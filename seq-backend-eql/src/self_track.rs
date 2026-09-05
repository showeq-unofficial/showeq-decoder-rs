//! Cross-packet EQL session identity: which spawn id is "us".
//!
//! Every other parser in this crate is a pure function over one packet. This
//! one is deliberately stateful, because the quirk it exists to absorb is not
//! expressible per-packet:
//!
//! eql announces the local player's `OP_ZoneEntry` TWICE per zone — a live copy
//! that moves and a static phantom a few ids higher — and it keys the player's
//! MOVEMENT to the first id but their PROFILE/BUFF/STAT data to the second.
//! Worse, the two records are not adjacent on the wire: the stat-sync packet
//! carrying the real HP/mana/endurance maxima can arrive BEFORE the phantom's
//! record does, and in some zone-ins it is the only packet that ever carries
//! them (every later one for that id is a stat-less keepalive). Matching on the
//! id alone therefore drops the player's maxima entirely, depending on nothing
//! more than wire ordering.
//!
//! So this tracker holds two things: the id pair, and the most recent wide
//! vitals for a plausible-but-not-yet-resolved twin id. When the phantom's
//! record finally lands and resolves that id as ours, the host drains the held
//! vitals via [`SelfTracker::take_pending_vitals`] and applies them.
//!
//! It lives here rather than in a host so that scry-cpp and scry inherit
//! the behaviour instead of each re-deriving it.

use crate::StatSync;

/// The live copy and its phantom twin are issued in one batch, so their ids sit
/// within a few of each other; a stale id from a previous zone is hundreds or
/// thousands off. Matches the window the daemon's `consumeSelfSpawn` used.
pub const SAME_BATCH: u32 = 16;

/// What a self-identifying packet means when NO zone-in was witnessed.
///
/// A host that attaches mid-session (sniffer started while already in a zone,
/// or restarted) never sees `OP_PlayerProfile` or the `OP_ZoneEntry` burst, so
/// [`SelfTracker::observe_spawn`] — which needs a name to match — can never
/// fire, and the player is invisible to it until they zone. These are the two
/// signals that keep arriving anyway and can only be the local player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SelfPosRouting {
    /// Nothing to do: no id, or the self is already resolved properly.
    Known = 0,
    /// First provisional adoption of `spawn_id` — the host has no record for
    /// this id, so it must synthesise one to show the player at all.
    Adopted = 1,
}

/// What a self-named `OP_ZoneEntry` record means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpawnRouting {
    /// Not the local player — hand it to the spawn list as normal.
    NotSelf = 0,
    /// The live/moving copy: adopt as the player's id.
    AdoptSelf = 1,
    /// The phantom twin (or a re-announce of the adopted id): swallow it, but
    /// remember the id — the player's stats are keyed to it.
    SelfTwin = 2,
}

/// One stat-sync packet's verdict. `is_self` false means the packet belongs to
/// some other spawn and the caller should route its HP normally; the `has_*`
/// flags are only meaningful when `is_self` is true.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfStat {
    pub is_self: bool,
    pub has_hp: bool,
    pub hp_cur: i64,
    pub hp_max: i64,
    pub has_mana: bool,
    pub mana_cur: i64,
    pub mana_max: i64,
    pub has_end: bool,
    pub end_cur: i64,
    pub end_max: i64,
}

impl SelfStat {
    /// True when at least one stat is carried. A wide packet with no stat bits
    /// set is the periodic keepalive and updates nothing.
    pub fn any(&self) -> bool {
        self.has_hp || self.has_mana || self.has_end
    }

    fn from_stat_sync(s: &StatSync, is_self: bool) -> Self {
        Self {
            is_self,
            has_hp: s.has_hp,
            hp_cur: s.hp_cur,
            hp_max: s.hp_max,
            has_mana: s.has_mana,
            mana_cur: s.mana_cur,
            mana_max: s.mana_max,
            has_end: s.has_end,
            end_cur: s.end_cur,
            end_max: s.end_max,
        }
    }
}

/// Tracks the local player's id pair for one session, and holds vitals that
/// arrived before the id carrying them could be resolved.
///
/// Reset it wherever the host severs the self-id — zone change, `OP_EnterWorld`
/// re-entry, and the player's own death all issue a fresh id.
#[derive(Debug, Default, Clone)]
pub struct SelfTracker {
    self_id: u32,
    alt_id: u32,
    /// `(spawn_id, vitals)` for a wide packet whose id was a plausible twin but
    /// was not yet known to be ours. At most one — the newest wins, since these
    /// are absolute cur/max snapshots rather than deltas.
    pending: Option<(u32, SelfStat)>,
    /// Self id recovered without a zone-in (see [`SelfPosRouting`]). Ranks
    /// BELOW `self_id`: it is the phantom twin's id, so it is only ever used
    /// when there is no name-matched live copy to prefer.
    provisional_id: u32,
    /// A provisional id that a real adoption has just superseded, waiting to be
    /// drained by the host so it can drop whatever it synthesised for it.
    retired: u32,
    /// Whether the stat channel's mid-session guess has already been spent this
    /// session. It is one-shot: re-arming it after a retraction just lets the
    /// next stranger take the slot, which measured WORSE than leaving it empty.
    guessed: bool,
}

impl SelfTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the session's identity. Keeps nothing: a new zone re-announces
    /// both records, and holding a previous zone's pending vitals could let
    /// them land on a recycled id.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The adopted (movement) id, or 0 if not yet resolved.
    pub fn self_id(&self) -> u32 {
        self.self_id
    }

    /// The phantom twin id that stats/profile/buffs are keyed to, or 0.
    pub fn alt_id(&self) -> u32 {
        self.alt_id
    }

    /// Either id counts as the player — as does a provisional id, which is the
    /// only handle a mid-session attach has.
    pub fn is_self(&self, id: u32) -> bool {
        id != 0 && (id == self.self_id || id == self.alt_id || id == self.provisional_id)
    }

    /// The id recovered without a zone-in, or 0. Never overrides [`Self::self_id`].
    pub fn provisional_id(&self) -> u32 {
        self.provisional_id
    }

    /// Observe the self id from the C>S position report; it is the phantom
    /// twin's id, so `observe_spawn`'s name match always outranks it.
    pub fn observe_self_pos(&mut self, spawn_id: u32) -> SelfPosRouting {
        if spawn_id == 0 {
            return SelfPosRouting::Known;
        }

        // Properly adopted already: the field still tells us which id is the
        // twin, which is what stats are keyed to — learn it, adopt nothing.
        if self.self_id != 0 {
            if spawn_id != self.self_id && self.is_twin_candidate(spawn_id) {
                self.alt_id = spawn_id;
            }
            return SelfPosRouting::Known;
        }

        self.adopt_provisional(spawn_id)
    }

    fn adopt_provisional(&mut self, spawn_id: u32) -> SelfPosRouting {
        if self.provisional_id == spawn_id {
            return SelfPosRouting::Known;
        }

        // A provisional id that changed (zoned while we were attached, without
        // ever seeing a profile) supersedes itself: retire the old record.
        if self.provisional_id != 0 {
            self.retired = self.provisional_id;
        }

        self.provisional_id = spawn_id;
        SelfPosRouting::Adopted
    }

    /// Drain a provisional id that has been superseded, or 0. The host drops
    /// whatever it synthesised for that id; anything real under it will be
    /// re-announced by the zone-in that superseded it.
    pub fn take_retired_provisional(&mut self) -> u32 {
        std::mem::replace(&mut self.retired, 0)
    }

    /// Close enough to the adopted id to be its twin, but not yet resolved.
    fn is_twin_candidate(&self, id: u32) -> bool {
        self.self_id != 0 && id != 0 && id.abs_diff(self.self_id) <= SAME_BATCH
    }

    /// Classify an `OP_ZoneEntry` record. `player_name` is the host's
    /// authoritative character name (from `OP_PlayerProfile`); an empty name on
    /// either side can never match, so records seen before the profile lands
    /// fall through to the spawn list.
    pub fn observe_spawn(
        &mut self,
        player_name: &str,
        spawn_name: &str,
        spawn_id: u32,
    ) -> SpawnRouting {
        if spawn_id == 0
            || player_name.is_empty()
            || spawn_name.is_empty()
            || spawn_name != player_name
        {
            return SpawnRouting::NotSelf;
        }

        // No id yet (fresh login / post-death sever), or an id that jumped
        // zones: this is the new live copy.
        if self.self_id == 0 || spawn_id.abs_diff(self.self_id) > SAME_BATCH {
            self.self_id = spawn_id;
            self.alt_id = 0;
            // A name match is authoritative, so any id guessed mid-session is
            // now superseded — hand it back so the host drops what it made up.
            if self.provisional_id != 0 && self.provisional_id != spawn_id {
                self.retired = self.provisional_id;
            }
            self.provisional_id = 0;
            // Anything held against the previous zone's ids is stale.
            if !matches!(self.pending, Some((id, _)) if id == spawn_id) {
                self.pending = None;
            }
            return SpawnRouting::AdoptSelf;
        }

        if spawn_id != self.self_id {
            self.alt_id = spawn_id;
        }
        SpawnRouting::SelfTwin
    }

    /// Classify a decoded stat-sync packet.
    ///
    /// When the id isn't (yet) known to be ours but is a plausible twin, the
    /// vitals are held for later. The packet is still reported as `is_self:
    /// false` so the caller routes its HP to the spawn list as usual — if the
    /// id turns out to be a real neighbouring spawn rather than our twin,
    /// nothing was swallowed, and if it turns out to be ours the spawn-list
    /// update was a no-op against an id that has no spawn entry.
    pub fn observe_stat_sync(&mut self, s: &StatSync) -> SelfStat {
        if self.is_self(s.spawn_id) {
            return SelfStat::from_stat_sync(s, true);
        }

        // Cold-attach guess from a wide mana/endurance packet: latches once per
        // session, or freely when it is the twin of the position-report id.
        let qualifies = if self.provisional_id != 0 {
            s.spawn_id.abs_diff(self.provisional_id) <= SAME_BATCH
        } else {
            !self.guessed
        };

        if self.self_id == 0
            && self.alt_id == 0
            && s.wide
            && (s.has_mana || s.has_end)
            && s.spawn_id != 0
            && qualifies
        {
            self.alt_id = s.spawn_id;
            if self.provisional_id == 0 {
                self.guessed = true;
            }
            return SelfStat::from_stat_sync(s, true);
        }

        if s.wide && self.is_twin_candidate(s.spawn_id) {
            let v = SelfStat::from_stat_sync(s, true);
            if v.any() {
                self.pending = Some((s.spawn_id, v));
            }
        }

        SelfStat::default()
    }

    /// Drain vitals held for an id that has since been resolved as ours.
    /// Returns an all-false value when there is nothing to apply.
    pub fn take_pending_vitals(&mut self) -> SelfStat {
        if let Some((id, v)) = self.pending {
            if self.is_self(id) {
                self.pending = None;
                return v;
            }
        }
        SelfStat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "Testchar";

    fn wide(spawn_id: u32, hp: (i64, i64), mana: (i64, i64), end: (i64, i64)) -> StatSync {
        StatSync {
            spawn_id,
            wide: true,
            has_hp: true,
            hp_cur: hp.0,
            hp_max: hp.1,
            has_mana: true,
            mana_cur: mana.0,
            mana_max: mana.1,
            has_end: true,
            end_cur: end.0,
            end_max: end.1,
        }
    }

    fn keepalive(spawn_id: u32) -> StatSync {
        StatSync {
            spawn_id,
            wide: true,
            ..StatSync::default()
        }
    }

    #[test]
    fn adopts_the_first_self_record_and_twins_the_second() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_spawn(ME, ME, 5893), SpawnRouting::AdoptSelf);
        assert_eq!(t.self_id(), 5893);
        assert_eq!(t.observe_spawn(ME, ME, 5906), SpawnRouting::SelfTwin);
        assert_eq!(
            t.self_id(),
            5893,
            "movement id must not be re-homed by the twin"
        );
        assert_eq!(t.alt_id(), 5906);
        assert!(t.is_self(5893) && t.is_self(5906));
    }

    #[test]
    fn other_spawns_are_not_self() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        assert_eq!(
            t.observe_spawn(ME, "Someoneelse", 5901),
            SpawnRouting::NotSelf
        );
        assert!(!t.is_self(5901));
    }

    #[test]
    fn a_distant_id_re_homes_rather_than_twinning() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        t.observe_spawn(ME, ME, 5906);
        // Next zone: ids jump far away.
        assert_eq!(t.observe_spawn(ME, ME, 12636), SpawnRouting::AdoptSelf);
        assert_eq!(t.self_id(), 12636);
        assert_eq!(t.alt_id(), 0, "the previous zone's twin must not linger");
        assert!(!t.is_self(5906));
    }

    #[test]
    fn stats_on_the_twin_id_are_ours() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        t.observe_spawn(ME, ME, 5906);
        let got = t.observe_stat_sync(&wide(5906, (4023, 4265), (1780, 4170), (1138, 2976)));
        assert!(got.is_self);
        assert_eq!((got.hp_max, got.mana_max, got.end_max), (4265, 4170, 2976));
    }

    /// The regression this module exists for: the only packet carrying the
    /// player's maxima arrives BEFORE the phantom record that identifies its
    /// id. Matching on the resolved ids alone loses it permanently.
    #[test]
    fn vitals_arriving_before_the_twin_record_are_replayed() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_spawn(ME, ME, 5893), SpawnRouting::AdoptSelf);

        // Stats land first, keyed to an id nothing has claimed yet.
        let early = t.observe_stat_sync(&wide(5906, (4023, 4265), (1780, 4170), (1138, 2976)));
        assert!(!early.is_self, "cannot be attributed yet");

        // Every later packet for that id is a stat-less keepalive.
        assert!(!t.observe_stat_sync(&keepalive(5906)).any());

        // The phantom's record finally lands.
        assert_eq!(t.observe_spawn(ME, ME, 5906), SpawnRouting::SelfTwin);

        let held = t.take_pending_vitals();
        assert!(held.is_self && held.any());
        assert_eq!((held.hp_cur, held.hp_max), (4023, 4265));
        assert_eq!((held.mana_cur, held.mana_max), (1780, 4170));
        assert_eq!((held.end_cur, held.end_max), (1138, 2976));

        // Draining is one-shot.
        assert!(!t.take_pending_vitals().any());
    }

    #[test]
    fn held_vitals_are_not_applied_to_an_unrelated_id() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        t.observe_stat_sync(&wide(5906, (10, 20), (30, 40), (50, 60)));
        // A neighbour resolves, but it is not us and not our twin.
        assert_eq!(
            t.observe_spawn(ME, "Neighbour", 5901),
            SpawnRouting::NotSelf
        );
        assert!(!t.take_pending_vitals().any());
    }

    #[test]
    fn distant_ids_are_never_held() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        // Well outside the batch window — a real unrelated spawn.
        t.observe_stat_sync(&wide(1002, (540, 668), (381, 410), (524, 524)));
        t.observe_spawn(ME, ME, 5906);
        assert!(!t.take_pending_vitals().any());
    }

    #[test]
    fn keepalives_never_overwrite_held_vitals() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        t.observe_stat_sync(&wide(5906, (4023, 4265), (1780, 4170), (1138, 2976)));
        t.observe_stat_sync(&keepalive(5906));
        t.observe_spawn(ME, ME, 5906);
        assert_eq!(t.take_pending_vitals().hp_max, 4265);
    }

    #[test]
    fn reset_clears_everything() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 5893);
        t.observe_stat_sync(&wide(5906, (1, 2), (3, 4), (5, 6)));
        t.reset();
        assert_eq!(t.self_id(), 0);
        assert_eq!(t.alt_id(), 0);
        assert!(!t.is_self(5893));
        t.observe_spawn(ME, ME, 5906);
        assert!(
            !t.take_pending_vitals().any(),
            "pre-reset vitals must not survive"
        );
    }

    #[test]
    fn nothing_matches_before_the_profile_names_us() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_spawn("", ME, 5893), SpawnRouting::NotSelf);
        assert_eq!(
            t.self_id(),
            0,
            "a name match is the only thing that sets self_id"
        );
    }

    // ── mid-session attach ────────────────────────────────────────────────
    // No profile and no zone-in burst were witnessed, so observe_spawn can
    // never fire: these are the two signals that still identify the player.

    #[test]
    fn self_pos_adopts_provisionally_when_no_zone_in_was_seen() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_self_pos(15707), SelfPosRouting::Adopted);
        assert_eq!(t.provisional_id(), 15707);
        assert!(t.is_self(15707));
        assert_eq!(t.self_id(), 0, "provisional is not a real adoption");
        // Every later report is the same id — the host already has its record.
        assert_eq!(t.observe_self_pos(15707), SelfPosRouting::Known);
    }

    #[test]
    fn wide_mana_identifies_us_while_standing_still() {
        let mut t = SelfTracker::new();
        // Mana/endurance ride this channel for the player only. No coordinates,
        // so it resolves the id for attribution without becoming drawable.
        let v = t.observe_stat_sync(&wide(5906, (4023, 4265), (1780, 4170), (1138, 2976)));
        assert!(v.is_self);
        assert_eq!(v.mana_max, 4170);
        assert!(t.is_self(5906));
        assert_eq!(
            t.provisional_id(),
            0,
            "no coordinates — nothing to synthesise"
        );
    }

    // The two mid-session signals carry different ids (pos = one record, stats
    // = its twin). Sharing one slot made each overwrite the other, so every
    // position report re-adopted and the host re-synthesised on a loop.
    #[test]
    fn the_two_signals_do_not_fight_over_the_id() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_self_pos(11715), SelfPosRouting::Adopted);

        for _ in 0..5 {
            t.observe_stat_sync(&wide(11719, (100, 200), (50, 60), (10, 20)));
            assert_eq!(
                t.observe_self_pos(11715),
                SelfPosRouting::Known,
                "adopt once"
            );
        }

        assert_eq!(t.provisional_id(), 11715);
        assert!(t.is_self(11719), "the twin still attributes stats");
        assert_eq!(t.take_retired_provisional(), 0, "nothing was superseded");
    }

    // The mid-session guess latches once. Before this, every neighbouring PC
    // reporting mana/endurance re-claimed the player's identity and overwrote
    // their maxima — 12 ids claiming a 3-id answer on a real 3-zone capture.
    #[test]
    fn the_mid_session_guess_latches_only_once() {
        let mut t = SelfTracker::new();

        let first = t.observe_stat_sync(&wide(5906, (4023, 4265), (1780, 4170), (1138, 2976)));
        assert!(first.is_self);
        assert_eq!(first.mana_max, 4170);

        // A different spawn reporting the same shape is NOT us.
        let other = t.observe_stat_sync(&wide(8412, (1, 2), (3, 9999), (5, 6)));
        assert!(!other.is_self, "a second id must not re-claim the player");
        assert!(!t.is_self(8412));

        // The latched id keeps attributing.
        assert!(
            t.observe_stat_sync(&wide(5906, (10, 4265), (20, 4170), (30, 2976)))
                .is_self
        );
        assert!(t.is_self(5906));
    }

    // With a provisional id in hand the stats id must be its twin. A far-off id
    // is another spawn, however player-shaped its packet looks.
    #[test]
    fn a_distant_id_cannot_claim_us_once_the_position_report_named_one() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_self_pos(27695), SelfPosRouting::Adopted);

        let far = t.observe_stat_sync(&wide(27607, (1, 2), (3, 4), (5, 6)));
        assert!(!far.is_self, "88 ids away — not our twin");
        assert!(!t.is_self(27607));

        let twin = t.observe_stat_sync(&wide(27699, (7, 8), (9, 10), (11, 12)));
        assert!(twin.is_self, "4 ids away — the twin");
        assert!(t.is_self(27699));
    }

    // The guess is only a guess: a name match replaces it wholesale.
    #[test]
    fn a_name_match_supersedes_a_wrongly_latched_guess() {
        let mut t = SelfTracker::new();
        assert!(
            t.observe_stat_sync(&wide(9000, (1, 2), (3, 4), (5, 6)))
                .is_self
        );
        assert!(t.is_self(9000));

        assert_eq!(t.observe_spawn(ME, ME, 4307), SpawnRouting::AdoptSelf);
        assert!(!t.is_self(9000), "the guess is dropped once we are named");
        assert!(t.is_self(4307));
    }

    #[test]
    fn hp_only_wide_packets_are_not_us() {
        let mut t = SelfTracker::new();
        let mut s = wide(1234, (500, 500), (0, 0), (0, 0));
        s.has_mana = false;
        s.has_end = false;
        assert!(!t.observe_stat_sync(&s).is_self, "HP alone is any mob");
        assert_eq!(t.provisional_id(), 0);
    }

    #[test]
    fn a_name_match_supersedes_the_provisional_and_hands_it_back() {
        let mut t = SelfTracker::new();
        t.observe_self_pos(15707); // the twin, guessed mid-session
        assert_eq!(t.observe_spawn(ME, ME, 15701), SpawnRouting::AdoptSelf);
        assert_eq!(t.self_id(), 15701, "the live copy wins");
        assert_eq!(t.provisional_id(), 0);
        assert_eq!(
            t.take_retired_provisional(),
            15707,
            "host must drop what it synthesised"
        );
        assert_eq!(t.take_retired_provisional(), 0, "drained once");
    }

    #[test]
    fn self_pos_never_outranks_a_name_matched_self() {
        let mut t = SelfTracker::new();
        t.observe_spawn(ME, ME, 15701);
        assert_eq!(t.observe_self_pos(15707), SelfPosRouting::Known);
        assert_eq!(t.self_id(), 15701, "still pinned to the live copy");
        assert_eq!(
            t.alt_id(),
            15707,
            "but the field told us which id is the twin"
        );
        assert_eq!(t.provisional_id(), 0);
    }

    #[test]
    fn zoning_while_still_provisional_retires_the_previous_id() {
        let mut t = SelfTracker::new();
        assert_eq!(t.observe_self_pos(15707), SelfPosRouting::Adopted);
        assert_eq!(t.observe_self_pos(20311), SelfPosRouting::Adopted);
        assert_eq!(t.take_retired_provisional(), 15707);
        assert_eq!(t.provisional_id(), 20311);
    }

    #[test]
    fn reset_clears_the_provisional_state_too() {
        let mut t = SelfTracker::new();
        t.observe_self_pos(15707);
        t.reset();
        assert_eq!(t.provisional_id(), 0);
        assert_eq!(t.take_retired_provisional(), 0);
        assert!(!t.is_self(15707));
    }
}
