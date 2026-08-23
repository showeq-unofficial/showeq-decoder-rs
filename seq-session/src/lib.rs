//! Stateful packet decoding keyed by backend, stream, and numeric opcode.
//!
//! Existing name-based backend decoders remain public for compatibility. New
//! hosts should keep one `Session` per ordered application-packet stream.

#[cfg(not(any(
    feature = "backend-live",
    feature = "backend-test",
    feature = "backend-eql"
)))]
compile_error!("seq-session: enable at least one backend feature");
#[cfg(all(feature = "backend-live", feature = "backend-test"))]
compile_error!("seq-session: backend-live and backend-test use different struct mirrors");

#[cfg(feature = "backend-eql")]
use seq_backend_eql::{backend::EqlBackend, LootTracker, SelfTracker};
#[cfg(any(feature = "backend-live", feature = "backend-test"))]
use seq_backend_live::LiveBackend;
use seq_events::{
    AlternateAbilityDefinition, AlternateAbilityRank, AlternateAdvancementProgress,
    AlternateAdvancementSnapshot, Backend, Decoded, ExperienceProgress, ItemTemplate, MoneyBalance,
    PlayerAppearance, PlayerIdentity, PlayerVitals, SessionResetReason, SkillValue, VitalValue,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

#[cfg(feature = "backend-eql")]
pub use seq_backend_eql::{LootRow, SelfStat};
pub use seq_events::{Dir, Dir as Direction, Event};
pub use seq_protocol_data::{
    BackendId, ContentHash, OpcodeId, ProtocolGeneration, ProtocolRegistry, StreamKind,
};

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub backend: BackendId,
    pub protocol_registry: Arc<ProtocolRegistry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeDisposition {
    Decoded,
    Ignored,
    Unhandled,
    Malformed,
    Unmapped,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodeBatch {
    pub protocol_generation: ProtocolGeneration,
    pub disposition: DecodeDisposition,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushReason {
    Shutdown,
    ZoneTransition,
    ReplayEnd,
    Reset,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfIdentity {
    pub self_id: u32,
    pub alt_id: u32,
    pub provisional_id: u32,
}

enum BackendSession {
    #[cfg(any(feature = "backend-live", feature = "backend-test"))]
    Live(LiveBackend),
    #[cfg(any(feature = "backend-live", feature = "backend-test"))]
    Test(LiveBackend),
    #[cfg(feature = "backend-eql")]
    Eql(Box<EqlSession>),
}

impl BackendSession {
    fn observe_event(
        &mut self,
        event: &Event,
        opcode_name: &str,
        direction: Direction,
        payload: &[u8],
        timestamp: i64,
    ) {
        match self {
            #[cfg(feature = "backend-eql")]
            Self::Eql(state) => {
                state.observe_event(event, opcode_name, direction, payload, timestamp);
            }
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            Self::Live(_) | Self::Test(_) => {
                let _ = (event, opcode_name, direction, payload, timestamp);
            }
        }
    }
}

#[cfg(feature = "backend-eql")]
struct EqlSession {
    decoder: EqlBackend,
    self_tracker: SelfTracker,
    loot_tracker: LootTracker,
    player_name: String,
    self_stats: Vec<SelfStat>,
    loot_rows: Vec<LootRow>,
}

#[cfg(feature = "backend-eql")]
impl Default for EqlSession {
    fn default() -> Self {
        Self {
            decoder: EqlBackend,
            self_tracker: SelfTracker::new(),
            loot_tracker: LootTracker::new(),
            player_name: String::new(),
            self_stats: Vec::new(),
            loot_rows: Vec::new(),
        }
    }
}

/// One ordered logical game session. Its backend cannot change after creation.
pub struct Session {
    backend: BackendId,
    protocol_registry: Arc<ProtocolRegistry>,
    decoder: BackendSession,
    entities: EntityIndex,
    player_name: String,
    player_id: Option<u32>,
    player_identity: Option<PlayerIdentity>,
    progression: ProgressionState,
}

#[derive(Default)]
struct ProgressionState {
    inventory: HashMap<String, ItemTemplate>,
    equipment: BTreeMap<u16, String>,
    money: Option<MoneyBalance>,
    skills: BTreeMap<u32, u32>,
    experience: Option<u32>,
    level: Option<u32>,
    aa_progress: Option<AlternateAdvancementProgress>,
    aa_definitions: HashMap<u32, u32>,
}

#[derive(Default)]
struct EntityIndex {
    names_by_id: HashMap<u32, String>,
    ids_by_name: HashMap<String, BTreeSet<u32>>,
    kinds_by_id: HashMap<u32, u8>,
}

impl EntityIndex {
    fn add(&mut self, id: u32, name: &str, kind: u8) {
        self.remove(id);
        self.names_by_id.insert(id, name.to_owned());
        self.kinds_by_id.insert(id, kind);
        self.ids_by_name
            .entry(name.to_owned())
            .or_default()
            .insert(id);
    }

    fn remove(&mut self, id: u32) {
        self.kinds_by_id.remove(&id);
        let Some(name) = self.names_by_id.remove(&id) else {
            return;
        };
        let Some(ids) = self.ids_by_name.get_mut(&name) else {
            return;
        };
        ids.remove(&id);
        if ids.is_empty() {
            self.ids_by_name.remove(&name);
        }
    }

    fn unique_id(&self, name: &str) -> Option<u32> {
        let ids = self.ids_by_name.get(name)?;
        (ids.len() == 1).then(|| *ids.first().expect("one name-index entry"))
    }

    #[cfg(feature = "backend-eql")]
    fn contains(&self, id: u32) -> bool {
        self.names_by_id.contains_key(&id)
    }

    fn rename(&mut self, id: u32, new_name: &str) {
        let kind = self.kinds_by_id.get(&id).copied().unwrap_or_default();
        self.remove(id);
        self.add(id, new_name, kind);
    }

    fn corpse_position(&self, id: u32, position: &mut seq_events::Point3) {
        if matches!(self.kinds_by_id.get(&id), Some(0 | 2 | 10)) {
            std::mem::swap(&mut position.x, &mut position.y);
        }
    }

    fn clear(&mut self) {
        self.names_by_id.clear();
        self.ids_by_name.clear();
        self.kinds_by_id.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerSpawnRouting {
    Other,
    Player,
    Phantom,
}

const fn nonzero(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

fn u32_to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(feature = "backend-eql")]
fn player_vitals(stat: SelfStat) -> PlayerVitals {
    let health = (stat.has_hp && stat.hp_max > 0).then_some(VitalValue {
        current: i64_to_i32(stat.hp_cur),
        maximum: Some(i64_to_i32(stat.hp_max)),
    });
    let mana = stat.has_mana.then_some(VitalValue {
        current: i64_to_i32(stat.mana_cur),
        maximum: Some(i64_to_i32(stat.mana_max)),
    });
    let endurance = stat.has_end.then_some(VitalValue {
        current: i64_to_i32(stat.end_cur),
        maximum: Some(i64_to_i32(stat.end_max)),
    });
    PlayerVitals {
        health,
        mana,
        endurance,
    }
}

#[cfg(feature = "backend-eql")]
fn i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        let decoder = match config.backend {
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendId::Live => BackendSession::Live(LiveBackend),
            // Test currently shares Live's wire decoder. Its opcode catalog is
            // still independent, so a patch-day ID rotation cannot cross over.
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendId::Test => BackendSession::Test(LiveBackend),
            #[cfg(feature = "backend-eql")]
            BackendId::Eql => BackendSession::Eql(Box::default()),
            #[allow(unreachable_patterns)]
            unsupported => panic!("backend {unsupported} is not linked into seq-session"),
        };
        Self {
            backend: config.backend,
            protocol_registry: config.protocol_registry,
            decoder,
            entities: EntityIndex::default(),
            player_name: String::new(),
            player_id: None,
            player_identity: None,
            progression: ProgressionState::default(),
        }
    }

    pub const fn backend(&self) -> BackendId {
        self.backend
    }

    pub fn decode(
        &mut self,
        stream: StreamKind,
        opcode_id: OpcodeId,
        direction: Direction,
        payload: &[u8],
    ) -> DecodeBatch {
        self.decode_at(stream, opcode_id, direction, payload, 0)
    }

    /// Decode with the capture timestamp used by stateful loot correlation.
    /// `decode` remains the clock-free API from the migration plan.
    pub fn decode_at(
        &mut self,
        stream: StreamKind,
        opcode_id: OpcodeId,
        direction: Direction,
        payload: &[u8],
        _timestamp: i64,
    ) -> DecodeBatch {
        let catalog = self.protocol_registry.snapshot(self.backend);
        let generation = catalog.generation();
        let Some(opcode_name) = catalog.lookup(stream, opcode_id) else {
            return DecodeBatch {
                protocol_generation: generation,
                disposition: DecodeDisposition::Unmapped,
                events: Vec::new(),
            };
        };

        let decoded = match &mut self.decoder {
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(backend) | BackendSession::Test(backend) => {
                backend.decode(opcode_name, direction, payload)
            }
            #[cfg(feature = "backend-eql")]
            BackendSession::Eql(state) => state.decoder.decode(opcode_name, direction, payload),
        };
        let decoded =
            self.apply_session_semantics(decoded, opcode_name, direction, payload, _timestamp);
        batch(generation, decoded)
    }

    /// Close stateful correlators at a lifecycle boundary.
    ///
    /// EQL loot rows remain a separate shadow output until the loot-family
    /// migration. Zone-transition and explicit resets emit a reset marker;
    /// terminal flushes only close correlators.
    pub fn flush(&mut self, reason: FlushReason) -> Vec<Event> {
        self.reset_correlations();
        match reason {
            FlushReason::ZoneTransition => vec![Event::SessionReset {
                reason: SessionResetReason::ZoneTransition,
            }],
            FlushReason::Reset => vec![Event::SessionReset {
                reason: SessionResetReason::Explicit,
            }],
            FlushReason::Shutdown | FlushReason::ReplayEnd => Vec::new(),
        }
    }

    /// Current EQL self-correlation state. Non-EQL sessions return zeros.
    pub fn self_identity(&self) -> SelfIdentity {
        #[allow(unreachable_patterns)]
        match &self.decoder {
            #[cfg(feature = "backend-eql")]
            BackendSession::Eql(state) => SelfIdentity {
                self_id: state.self_tracker.self_id(),
                alt_id: state.self_tracker.alt_id(),
                provisional_id: state.self_tracker.provisional_id(),
            },
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(_) | BackendSession::Test(_) => SelfIdentity::default(),
            _ => SelfIdentity::default(),
        }
    }

    /// Drain EQL self-vitals attributed by the session tracker.
    #[cfg(feature = "backend-eql")]
    pub fn take_self_stats(&mut self) -> Vec<SelfStat> {
        match &mut self.decoder {
            BackendSession::Eql(state) => std::mem::take(&mut state.self_stats),
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(_) | BackendSession::Test(_) => Vec::new(),
        }
    }

    /// Drain EQL loot rows completed by the session tracker.
    #[cfg(feature = "backend-eql")]
    pub fn take_loot_rows(&mut self) -> Vec<LootRow> {
        match &mut self.decoder {
            BackendSession::Eql(state) => std::mem::take(&mut state.loot_rows),
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(_) | BackendSession::Test(_) => Vec::new(),
        }
    }

    fn apply_session_semantics(
        &mut self,
        decoded: Decoded,
        opcode_name: &str,
        direction: Direction,
        payload: &[u8],
        timestamp: i64,
    ) -> Decoded {
        let events = match decoded {
            Decoded::One(event) => vec![event],
            Decoded::Many(events) => events,
            other => return other,
        };

        let mut output = Vec::with_capacity(events.len() + 1);
        for event in events {
            let reset = match &event {
                Event::EnterWorld { .. } => Some(SessionResetReason::EnterWorld),
                Event::PlayerProfile(_) => Some(SessionResetReason::PlayerProfile),
                Event::ZoneTransition {
                    confirmed: true, ..
                } => Some(SessionResetReason::ZoneTransition),
                _ => None,
            };
            if let Some(reason) = reset {
                self.reset_correlations();
                output.push(Event::SessionReset { reason });
            }

            for event in self.apply_player_semantics(event, opcode_name, direction) {
                for mut event in self.apply_progression_semantics(event) {
                    self.apply_entity_semantics(&mut event);
                    self.decoder
                        .observe_event(&event, opcode_name, direction, payload, timestamp);
                    output.push(event);
                }
            }
        }
        Decoded::Many(output)
    }

    #[allow(clippy::too_many_lines, irrefutable_let_patterns)]
    fn apply_player_semantics(
        &mut self,
        event: Event,
        opcode_name: &str,
        direction: Direction,
    ) -> Vec<Event> {
        match event {
            Event::EnterWorld { character_name } => {
                if self.player_name != character_name {
                    self.player_identity = None;
                }
                self.player_name.clone_from(&character_name);
                vec![Event::EnterWorld { character_name }]
            }
            Event::PlayerProfile(profile) => {
                self.player_name.clone_from(&profile.name);
                let identity = PlayerIdentity {
                    spawn_id: None,
                    name: profile.name.clone(),
                    last_name: profile.last_name.clone(),
                    race: profile.race,
                    class_: profile.class_,
                    deity: profile.deity,
                    level: u32::from(profile.level),
                    class_mask: profile.class_mask,
                };
                self.player_identity = Some(identity.clone());
                let vitals = PlayerVitals {
                    health: Some(VitalValue {
                        current: u32_to_i32(profile.cur_hp),
                        maximum: None,
                    }),
                    mana: Some(VitalValue {
                        current: u32_to_i32(profile.mana),
                        maximum: None,
                    }),
                    endurance: None,
                };
                vec![
                    Event::PlayerProfile(profile),
                    Event::PlayerIdentityUpdated(identity),
                    Event::PlayerVitalsUpdated(vitals),
                ]
            }
            Event::SpawnAdded(spawn) => self.apply_spawn_identity(spawn, opcode_name),
            Event::SelfPos {
                pos,
                spawn_id,
                velocity,
                delta_heading,
                animation,
            } => {
                if direction != Dir::ClientToServer {
                    return vec![Event::SpawnMoved {
                        id: spawn_id,
                        pos,
                        velocity,
                        delta_heading,
                        animation,
                    }];
                }

                #[cfg(feature = "backend-eql")]
                if let BackendSession::Eql(state) = &mut self.decoder {
                    state.self_tracker.observe_self_pos(spawn_id);
                    let resolved = nonzero(state.self_tracker.self_id());
                    self.player_id = resolved;
                    return vec![Event::PlayerMoved {
                        spawn_id: resolved,
                        pos,
                    }];
                }

                self.player_id = nonzero(spawn_id);
                self.update_identity_id();
                vec![Event::PlayerMoved {
                    spawn_id: self.player_id,
                    pos,
                }]
            }
            Event::SpawnHp { id, cur, max } => {
                if self.is_player_id(id) {
                    vec![Event::PlayerVitalsUpdated(PlayerVitals {
                        health: Some(VitalValue {
                            current: cur,
                            maximum: Some(max),
                        }),
                        ..PlayerVitals::default()
                    })]
                } else {
                    vec![Event::SpawnHealthUpdated {
                        id,
                        current: cur,
                        maximum: max,
                    }]
                }
            }
            Event::StatSync {
                spawn_id,
                wide,
                has_hp,
                hp_cur,
                hp_max,
                has_mana,
                mana_cur,
                mana_max,
                has_end,
                end_cur,
                end_max,
            } => self.apply_stat_sync(
                spawn_id, wide, has_hp, hp_cur, hp_max, has_mana, mana_cur, mana_max, has_end,
                end_cur, end_max,
            ),
            Event::ManaUpdate { mana } => vec![Event::PlayerVitalsUpdated(PlayerVitals {
                mana: Some(VitalValue {
                    current: u32_to_i32(mana),
                    maximum: None,
                }),
                ..PlayerVitals::default()
            })],
            Event::SpawnKilled {
                deceased_id,
                killer_id,
            } => {
                let killer_id = nonzero(killer_id);
                if self.is_player_id(deceased_id) {
                    self.clear_player_id();
                    vec![Event::PlayerDied { killer_id }]
                } else {
                    vec![Event::SpawnDied {
                        id: deceased_id,
                        killer_id,
                    }]
                }
            }
            Event::LoadoutSwap {
                spawn_id,
                level,
                class,
                race,
            } => {
                if self.is_player_id(spawn_id) {
                    let identity = self.update_player_loadout(level, class, race);
                    vec![Event::PlayerIdentityUpdated(identity)]
                } else {
                    vec![Event::SpawnIdentityUpdated {
                        id: spawn_id,
                        level,
                        class_: class,
                        race,
                    }]
                }
            }
            Event::SpawnAnimation {
                spawn_id,
                animation,
            } if self.is_player_id(spawn_id) => {
                vec![Event::PlayerAppearanceUpdated(PlayerAppearance {
                    animation: Some(animation),
                    ..PlayerAppearance::default()
                })]
            }
            Event::SpawnIllusion {
                spawn_id,
                race,
                gender,
            } if self.is_player_id(spawn_id) => {
                if let Some(identity) = &mut self.player_identity {
                    identity.race = race;
                }
                vec![Event::PlayerAppearanceUpdated(PlayerAppearance {
                    race: Some(race),
                    gender: Some(gender),
                    animation: None,
                })]
            }
            other => vec![other],
        }
    }

    fn apply_spawn_identity(
        &mut self,
        spawn: seq_events::SpawnInfo,
        opcode_name: &str,
    ) -> Vec<Event> {
        let routing = self.classify_spawn(&spawn);
        if routing == PlayerSpawnRouting::Other {
            return vec![Event::SpawnAdded(spawn)];
        }

        if routing == PlayerSpawnRouting::Phantom {
            return self.take_pending_player_vitals();
        }

        self.player_id = Some(spawn.id);
        let identity = PlayerIdentity {
            spawn_id: Some(spawn.id),
            name: spawn.name,
            last_name: spawn.last_name,
            race: spawn.race,
            class_: spawn.class_,
            deity: spawn.deity,
            level: u32::from(spawn.level),
            class_mask: spawn.class_mask,
        };
        self.player_identity = Some(identity.clone());

        let mut output = Vec::with_capacity(4);
        if opcode_name != "OP_LoadoutSwap" {
            output.push(Event::PlayerIdentityUpdated(identity));
        }
        if let Some(pos) = spawn.pos {
            output.push(Event::PlayerMoved {
                spawn_id: self.player_id,
                pos,
            });
        }
        if let Some(maximum) = spawn.max_hp {
            output.push(Event::PlayerVitalsUpdated(PlayerVitals {
                health: Some(VitalValue {
                    current: u32_to_i32(spawn.cur_hp),
                    maximum: Some(u32_to_i32(maximum)),
                }),
                ..PlayerVitals::default()
            }));
        }
        output.extend(self.take_pending_player_vitals());
        output
    }

    #[allow(irrefutable_let_patterns)]
    fn classify_spawn(&mut self, spawn: &seq_events::SpawnInfo) -> PlayerSpawnRouting {
        #[cfg(feature = "backend-eql")]
        if let BackendSession::Eql(state) = &mut self.decoder {
            return match state
                .self_tracker
                .observe_spawn(&self.player_name, &spawn.name, spawn.id)
            {
                seq_backend_eql::SpawnRouting::NotSelf => PlayerSpawnRouting::Other,
                seq_backend_eql::SpawnRouting::AdoptSelf => PlayerSpawnRouting::Player,
                seq_backend_eql::SpawnRouting::SelfTwin => PlayerSpawnRouting::Phantom,
            };
        }

        if !self.player_name.is_empty() && spawn.name == self.player_name {
            PlayerSpawnRouting::Player
        } else {
            PlayerSpawnRouting::Other
        }
    }

    #[allow(clippy::too_many_arguments, irrefutable_let_patterns)]
    fn apply_stat_sync(
        &mut self,
        spawn_id: u32,
        wide: bool,
        has_hp: bool,
        hp_cur: i32,
        hp_max: i32,
        has_mana: bool,
        mana_cur: i32,
        mana_max: i32,
        has_end: bool,
        end_cur: i32,
        end_max: i32,
    ) -> Vec<Event> {
        #[cfg(feature = "backend-eql")]
        if let BackendSession::Eql(state) = &mut self.decoder {
            let stat = seq_backend_eql::StatSync {
                spawn_id,
                wide,
                has_hp,
                hp_cur: i64::from(hp_cur),
                hp_max: i64::from(hp_max),
                has_mana: wide && has_mana,
                mana_cur: i64::from(mana_cur),
                mana_max: i64::from(mana_max),
                has_end: wide && has_end,
                end_cur: i64::from(end_cur),
                end_max: i64::from(end_max),
            };
            let routed = state.self_tracker.observe_stat_sync(&stat);
            if routed.is_self {
                if routed.any() {
                    state.self_stats.push(routed);
                }
                let vitals = player_vitals(routed);
                return vitals
                    .any()
                    .then_some(Event::PlayerVitalsUpdated(vitals))
                    .into_iter()
                    .collect();
            }
            if has_hp && hp_max > 0 && self.entities.contains(spawn_id) {
                return vec![Event::SpawnHealthUpdated {
                    id: spawn_id,
                    current: hp_cur,
                    maximum: hp_max,
                }];
            }
            return Vec::new();
        }

        #[cfg(not(feature = "backend-eql"))]
        let _ = (
            wide, has_mana, mana_cur, mana_max, has_end, end_cur, end_max,
        );

        if has_hp && hp_max > 0 {
            vec![Event::SpawnHealthUpdated {
                id: spawn_id,
                current: hp_cur,
                maximum: hp_max,
            }]
        } else {
            Vec::new()
        }
    }

    #[allow(irrefutable_let_patterns)]
    fn take_pending_player_vitals(&mut self) -> Vec<Event> {
        #[cfg(feature = "backend-eql")]
        if let BackendSession::Eql(state) = &mut self.decoder {
            let pending = state.self_tracker.take_pending_vitals();
            if pending.any() {
                state.self_stats.push(pending);
            }
            let vitals = player_vitals(pending);
            return vitals
                .any()
                .then_some(Event::PlayerVitalsUpdated(vitals))
                .into_iter()
                .collect();
        }
        Vec::new()
    }

    #[allow(irrefutable_let_patterns)]
    fn is_player_id(&self, id: u32) -> bool {
        if self.player_id == Some(id) {
            return true;
        }
        #[cfg(feature = "backend-eql")]
        if let BackendSession::Eql(state) = &self.decoder {
            return state.self_tracker.is_self(id);
        }
        false
    }

    fn update_identity_id(&mut self) {
        if let Some(identity) = &mut self.player_identity {
            identity.spawn_id = self.player_id;
        }
    }

    #[allow(irrefutable_let_patterns)]
    fn clear_player_id(&mut self) {
        self.player_id = None;
        self.update_identity_id();
        #[cfg(feature = "backend-eql")]
        if let BackendSession::Eql(state) = &mut self.decoder {
            state.self_tracker.reset();
        }
    }

    fn update_player_loadout(&mut self, level: u32, class_: u32, race: u32) -> PlayerIdentity {
        let identity = self.player_identity.get_or_insert_with(|| PlayerIdentity {
            spawn_id: self.player_id,
            name: self.player_name.clone(),
            last_name: String::new(),
            race,
            class_,
            deity: 0,
            level,
            class_mask: 0,
        });
        identity.spawn_id = self.player_id;
        identity.level = level;
        identity.class_ = class_;
        identity.race = race;
        identity.clone()
    }

    fn apply_entity_semantics(&mut self, event: &mut Event) {
        match event {
            Event::SpawnAdded(spawn) => self.entities.add(spawn.id, &spawn.name, spawn.npc),
            Event::SpawnRemoved { id } => self.entities.remove(*id),
            Event::SpawnRenamed {
                id,
                old_name,
                new_name,
            } => {
                let resolved = self.entities.unique_id(old_name);
                *id = resolved;
                if let Some(resolved) = resolved {
                    self.entities.rename(resolved, new_name);
                }
            }
            Event::CorpseLocated { id, position } => {
                self.entities.corpse_position(*id, position);
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_progression_semantics(&mut self, event: Event) -> Vec<Event> {
        match event {
            Event::PlayerProfile(profile) => {
                self.progression.level = Some(u32::from(profile.level));
                let money = MoneyBalance {
                    platinum: profile.platinum,
                    gold: profile.gold,
                    silver: profile.silver,
                    copper: profile.copper,
                };
                self.progression.money = Some(money);

                self.progression.skills = learned_skills(&profile.skills);
                let skills = self
                    .progression
                    .skills
                    .iter()
                    .map(|(&skill_id, &value)| SkillValue { skill_id, value })
                    .collect();

                let aa_progress = AlternateAdvancementProgress {
                    experience: profile.aa_experience,
                    unspent_points: profile.aa_unspent,
                };
                self.progression.aa_progress = Some(aa_progress);
                let aa = AlternateAdvancementSnapshot {
                    purchased: purchased_aa(&profile.aa_ids, &profile.aa_values),
                    spent_points: (self.backend != BackendId::Eql).then_some(profile.aa_spent),
                    assigned_points: (self.backend != BackendId::Eql)
                        .then_some(profile.aa_assigned),
                    unspent_points: profile.aa_unspent,
                    experience: profile.aa_experience,
                };

                vec![
                    Event::PlayerProfile(profile),
                    Event::MoneyBalanceUpdated(money),
                    Event::SkillsSnapshot { skills },
                    Event::AlternateAdvancementSnapshot(aa),
                ]
            }
            Event::ItemSet { items } => {
                let canonical = canonical_inventory(items.clone());
                let inventory = inventory_map(&canonical);
                let equipment = equipment_map(&canonical);
                let changed = inventory != self.progression.inventory;
                self.progression.inventory = inventory;
                self.progression.equipment = equipment;

                let mut output = vec![Event::ItemSet { items }];
                if changed {
                    let worn = equipment_items(&canonical);
                    output.push(Event::InventorySnapshot { items: canonical });
                    output.push(Event::EquipmentSnapshot { items: worn });
                }
                output
            }
            Event::ItemLearned { item } => self.apply_item_update(item),
            Event::Money {
                platinum,
                gold,
                silver,
                copper,
            } => {
                let balance = MoneyBalance {
                    platinum,
                    gold,
                    silver,
                    copper,
                };
                let mut output = vec![Event::Money {
                    platinum,
                    gold,
                    silver,
                    copper,
                }];
                if self.progression.money != Some(balance) {
                    self.progression.money = Some(balance);
                    output.push(Event::MoneyBalanceUpdated(balance));
                }
                output
            }
            Event::SkillUpdate { skill_id, value } => {
                let mut output = vec![Event::SkillUpdate { skill_id, value }];
                if self.progression.skills.get(&skill_id).copied() != Some(value) {
                    self.progression.skills.insert(skill_id, value);
                    output.push(Event::SkillValueUpdated(SkillValue { skill_id, value }));
                }
                output
            }
            Event::Exp { exp } => {
                let mut output = vec![Event::Exp { exp }];
                if self.progression.experience != Some(exp) {
                    self.progression.experience = Some(exp);
                    output.push(Event::ExperienceUpdated(ExperienceProgress {
                        experience: exp,
                        level: self.progression.level,
                        previous_level: None,
                    }));
                }
                output
            }
            Event::LevelUpdate {
                level,
                level_old,
                exp,
            } => {
                let mut output = vec![Event::LevelUpdate {
                    level,
                    level_old,
                    exp,
                }];
                let changed = self.progression.level != Some(level)
                    || self.progression.experience != Some(exp);
                self.progression.level = Some(level);
                self.progression.experience = Some(exp);
                if let Some(identity) = &mut self.player_identity {
                    if identity.level != level {
                        identity.level = level;
                        output.push(Event::PlayerIdentityUpdated(identity.clone()));
                    }
                }
                if changed {
                    output.push(Event::ExperienceUpdated(ExperienceProgress {
                        experience: exp,
                        level: Some(level),
                        previous_level: Some(level_old),
                    }));
                }
                output
            }
            Event::AaExp { alt_exp, aa_points } => {
                let progress = AlternateAdvancementProgress {
                    experience: alt_exp,
                    unspent_points: aa_points,
                };
                let mut output = vec![Event::AaExp { alt_exp, aa_points }];
                if self.progression.aa_progress != Some(progress) {
                    self.progression.aa_progress = Some(progress);
                    output.push(Event::AlternateAdvancementUpdated(progress));
                }
                output
            }
            Event::AaTable { desc_id, title_sid } => {
                let mut output = vec![Event::AaTable { desc_id, title_sid }];
                if self.progression.aa_definitions.get(&desc_id).copied() != Some(title_sid) {
                    self.progression.aa_definitions.insert(desc_id, title_sid);
                    output.push(Event::AlternateAbilityDefined(AlternateAbilityDefinition {
                        ability_id: desc_id,
                        title_string_id: title_sid,
                    }));
                }
                output
            }
            other => vec![other],
        }
    }

    fn apply_item_update(&mut self, item: ItemTemplate) -> Vec<Event> {
        let key = inventory_key(&item);
        let previous = self.progression.inventory.get(&key).cloned();
        let mut output = vec![Event::ItemLearned { item: item.clone() }];
        if previous.as_ref() == Some(&item) {
            return output;
        }

        let previous_location = previous.as_ref().map(ItemTemplate::location);
        let old_worn = previous.as_ref().and_then(|old| {
            old.is_worn()
                .then_some((old.container_slot, inventory_key(old)))
        });
        self.progression.inventory.insert(key.clone(), item.clone());
        output.push(Event::InventoryItemUpdated {
            item: item.clone(),
            previous_location,
        });

        if let Some((slot, old_key)) = old_worn {
            if (!item.is_worn() || item.container_slot != slot)
                && self.progression.equipment.get(&slot) == Some(&old_key)
            {
                self.progression.equipment.remove(&slot);
                output.push(Event::EquipmentSlotUpdated { slot, item: None });
            }
        }
        if item.is_worn() {
            self.progression.equipment.insert(item.container_slot, key);
            output.push(Event::EquipmentSlotUpdated {
                slot: item.container_slot,
                item: Some(item),
            });
        }
        output
    }

    fn reset_correlations(&mut self) {
        self.entities.clear();
        self.player_id = None;
        self.update_identity_id();
        self.progression = ProgressionState::default();
        match &mut self.decoder {
            #[cfg(feature = "backend-eql")]
            BackendSession::Eql(state) => {
                state.loot_rows.extend(state.loot_tracker.flush());
                state.loot_tracker.reset();
                state.self_tracker.reset();
                state.self_stats.clear();
            }
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(_) | BackendSession::Test(_) => {}
        }
    }
}

fn inventory_key(item: &ItemTemplate) -> String {
    if item.serial.is_empty() {
        format!(
            "\0{}:{}:{}:{}",
            item.item_id, item.container_id, item.container_slot, item.parent_slot
        )
    } else {
        item.serial.clone()
    }
}

fn canonical_inventory(items: Vec<ItemTemplate>) -> Vec<ItemTemplate> {
    let mut by_key = HashMap::with_capacity(items.len());
    for item in items {
        by_key.insert(inventory_key(&item), item);
    }
    let mut items: Vec<_> = by_key.into_values().collect();
    items.sort_by(|left, right| {
        (
            left.container_id,
            left.parent_slot,
            left.container_slot,
            &left.serial,
        )
            .cmp(&(
                right.container_id,
                right.parent_slot,
                right.container_slot,
                &right.serial,
            ))
    });
    items
}

fn inventory_map(items: &[ItemTemplate]) -> HashMap<String, ItemTemplate> {
    items
        .iter()
        .cloned()
        .map(|item| (inventory_key(&item), item))
        .collect()
}

fn equipment_map(items: &[ItemTemplate]) -> BTreeMap<u16, String> {
    items
        .iter()
        .filter(|item| item.is_worn())
        .map(|item| (item.container_slot, inventory_key(item)))
        .collect()
}

fn equipment_items(items: &[ItemTemplate]) -> Vec<ItemTemplate> {
    let by_key = inventory_map(items);
    equipment_map(items)
        .into_values()
        .filter_map(|key| by_key.get(&key).cloned())
        .collect()
}

fn learned_skills(values: &[u32]) -> BTreeMap<u32, u32> {
    values
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, value)| *value != 0 && *value != u32::MAX)
        .map(|(skill_id, value)| (skill_id as u32, value))
        .collect()
}

fn purchased_aa(ids: &[u32], ranks: &[u32]) -> Vec<AlternateAbilityRank> {
    let mut purchased = BTreeMap::new();
    for (&ability_id, &rank) in ids.iter().zip(ranks) {
        if ability_id != 0 && rank != 0 {
            purchased.insert(ability_id, rank);
        }
    }
    purchased
        .into_iter()
        .map(|(ability_id, rank)| AlternateAbilityRank { ability_id, rank })
        .collect()
}

#[cfg(feature = "backend-eql")]
impl EqlSession {
    fn observe_event(
        &mut self,
        event: &Event,
        opcode_name: &str,
        direction: Direction,
        payload: &[u8],
        timestamp: i64,
    ) {
        match event {
            Event::PlayerProfile(profile) => {
                self.player_name.clone_from(&profile.name);
                self.loot_tracker.set_looter(&profile.name);
            }
            Event::ZoneChanged(zone) => {
                self.loot_rows
                    .extend(self.loot_tracker.set_zone(&zone.short_name));
            }
            Event::LootMessage {
                color,
                text,
                item_id,
                item_name,
            } => {
                self.loot_rows.extend(
                    self.loot_tracker
                        .on_loot_message(*color, text, *item_id, item_name, timestamp),
                );
            }
            Event::LootTransaction {
                corpse_id,
                item_id,
                quantity,
                coin_copper,
                from_corpse,
            } => {
                let sequence = if opcode_name == "OP_LootTransaction" {
                    seq_backend_eql::parse_loot_transaction(payload)
                        .map(|transaction| transaction.sequence)
                        .unwrap_or(0)
                } else {
                    0
                };
                self.loot_rows.extend(self.loot_tracker.on_loot_transaction(
                    *corpse_id,
                    *item_id,
                    *quantity,
                    *coin_copper,
                    *from_corpse,
                    sequence,
                    timestamp,
                ));
            }
            Event::LootDrops {
                corpse_id,
                corpse_name,
                items,
            } => {
                for item in items {
                    self.loot_rows.extend(self.loot_tracker.on_loot_drop_item(
                        *corpse_id,
                        corpse_name,
                        &item.name,
                        item.icon,
                        item.item_id,
                        timestamp,
                    ));
                }
            }
            Event::EnterWorld { character_name } if direction == Dir::ClientToServer => {
                self.player_name.clone_from(character_name);
                self.loot_tracker.set_looter(character_name);
            }
            _ => {}
        }
    }
}

fn batch(generation: ProtocolGeneration, decoded: Decoded) -> DecodeBatch {
    let (disposition, events) = match decoded {
        Decoded::One(event) => (DecodeDisposition::Decoded, vec![event]),
        Decoded::Many(events) => (DecodeDisposition::Decoded, events),
        Decoded::Ignored => (DecodeDisposition::Ignored, Vec::new()),
        Decoded::Unhandled => (DecodeDisposition::Unhandled, Vec::new()),
        Decoded::Malformed => (DecodeDisposition::Malformed, Vec::new()),
    };
    DecodeBatch {
        protocol_generation: generation,
        disposition,
        events,
    }
}

#[cfg(all(test, feature = "backend-eql"))]
mod tests {
    use super::*;

    fn eql_session(registry: Arc<ProtocolRegistry>) -> Session {
        Session::new(SessionConfig {
            backend: BackendId::Eql,
            protocol_registry: registry,
        })
    }

    fn self_pos(spawn_id: u16) -> [u8; seq_backend_eql::player_self_pos::PAYLOAD_LEN] {
        let mut payload = [0; seq_backend_eql::player_self_pos::PAYLOAD_LEN];
        payload[2..4].copy_from_slice(&spawn_id.to_le_bytes());
        payload
    }

    fn enter_world(name: &str) -> [u8; 72] {
        let mut payload = [0; 72];
        payload[..name.len()].copy_from_slice(name.as_bytes());
        payload
    }

    fn profile(name: &str) -> seq_events::ProfileInfo {
        seq_events::ProfileInfo {
            name: name.into(),
            last_name: String::new(),
            class_: 1,
            level: 2,
            race: 3,
            deity: 4,
            cur_hp: 5,
            mana: 6,
            aa_ids: Vec::new(),
            aa_values: Vec::new(),
            aa_spent: 0,
            aa_assigned: 0,
            aa_unspent: 0,
            aa_experience: 0,
            skills: Vec::new(),
            class_mask: 0,
            str_: 0,
            sta: 0,
            cha: 0,
            dex: 0,
            int_: 0,
            agi: 0,
            wis: 0,
            platinum: 0,
            gold: 0,
            silver: 0,
            copper: 0,
        }
    }

    fn new_zone() -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"qeynos\0South Qeynos\0");
        payload.extend_from_slice(&[0; 2]);
        payload.extend_from_slice(b"qeynos.eqg\0");
        payload.extend_from_slice(&[0; 90]);
        payload.extend_from_slice(&1.25f32.to_le_bytes());
        payload.extend_from_slice(&[0; 28]);
        payload.extend_from_slice(&20.5f32.to_le_bytes());
        payload.extend_from_slice(&10.25f32.to_le_bytes());
        payload.extend_from_slice(&30.75f32.to_le_bytes());
        payload
    }

    #[test]
    fn numeric_lookup_dispatches_and_reports_diagnostics() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        let batch = session.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(77),
        );
        assert_eq!(batch.protocol_generation, ProtocolGeneration(1));
        assert_eq!(batch.disposition, DecodeDisposition::Decoded);
        assert!(matches!(
            batch.events.as_slice(),
            [Event::PlayerMoved { spawn_id: None, .. }]
        ));
        assert_eq!(session.self_identity().provisional_id, 77);

        let unmapped = session.decode(
            StreamKind::World,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &[],
        );
        assert_eq!(unmapped.disposition, DecodeDisposition::Unmapped);

        let malformed =
            session.decode(StreamKind::Zone, OpcodeId(0x6987), Dir::ClientToServer, &[]);
        assert_eq!(malformed.disposition, DecodeDisposition::Malformed);

        session.decode(
            StreamKind::World,
            OpcodeId(0x0935),
            Dir::ServerToClient,
            &[],
        );
        assert_eq!(session.self_identity().provisional_id, 77);
        session.decode(
            StreamKind::World,
            OpcodeId(0x0935),
            Dir::ClientToServer,
            &enter_world("Firona"),
        );
        assert_eq!(session.self_identity(), SelfIdentity::default());
    }

    #[test]
    fn enter_world_resets_before_the_identity_event_and_malformed_does_not_reset() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        session.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(77),
        );

        let malformed = session.decode(
            StreamKind::World,
            OpcodeId(0x0935),
            Dir::ClientToServer,
            &[],
        );
        assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
        assert_eq!(session.self_identity().provisional_id, 77);

        let entered = session.decode(
            StreamKind::World,
            OpcodeId(0x0935),
            Dir::ClientToServer,
            &enter_world("Firona"),
        );
        assert_eq!(
            entered.events,
            vec![
                Event::SessionReset {
                    reason: SessionResetReason::EnterWorld,
                },
                Event::EnterWorld {
                    character_name: "Firona".into(),
                },
            ]
        );
        assert_eq!(session.self_identity(), SelfIdentity::default());
    }

    #[test]
    fn profile_reset_precedes_profile_observation() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        session.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(88),
        );
        let profile = profile("Firona");
        let decoded = session.apply_session_semantics(
            Decoded::One(Event::PlayerProfile(profile.clone())),
            "OP_PlayerProfile",
            Dir::ServerToClient,
            &[],
            0,
        );
        assert_eq!(
            decoded,
            Decoded::Many(vec![
                Event::SessionReset {
                    reason: SessionResetReason::PlayerProfile,
                },
                Event::PlayerProfile(profile.clone()),
                Event::MoneyBalanceUpdated(MoneyBalance {
                    platinum: 0,
                    gold: 0,
                    silver: 0,
                    copper: 0,
                }),
                Event::SkillsSnapshot { skills: Vec::new() },
                Event::AlternateAdvancementSnapshot(AlternateAdvancementSnapshot {
                    purchased: Vec::new(),
                    spent_points: None,
                    assigned_points: None,
                    unspent_points: 0,
                    experience: 0,
                }),
                Event::PlayerIdentityUpdated(PlayerIdentity {
                    spawn_id: None,
                    name: "Firona".into(),
                    last_name: String::new(),
                    race: 3,
                    class_: 1,
                    deity: 4,
                    level: 2,
                    class_mask: 0,
                }),
                Event::PlayerVitalsUpdated(PlayerVitals {
                    health: Some(VitalValue {
                        current: 5,
                        maximum: None,
                    }),
                    mana: Some(VitalValue {
                        current: 6,
                        maximum: None,
                    }),
                    endurance: None,
                }),
            ])
        );
        assert_eq!(session.self_identity(), SelfIdentity::default());
    }

    #[test]
    fn profile_progression_snapshot_pairs_and_filters_wire_arrays() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        let mut profile = profile("Firona");
        profile.aa_ids = vec![700, 0, 701, 700];
        profile.aa_values = vec![2, 9, 0, 3];
        profile.aa_spent = 99;
        profile.aa_assigned = 88;
        profile.aa_unspent = 7;
        profile.aa_experience = 6_543;
        profile.skills = vec![0, 12, u32::MAX, 34];
        profile.platinum = 11;
        profile.gold = 22;
        profile.silver = 33;
        profile.copper = 44;

        assert_eq!(
            session.apply_progression_semantics(Event::PlayerProfile(profile.clone())),
            vec![
                Event::PlayerProfile(profile),
                Event::MoneyBalanceUpdated(MoneyBalance {
                    platinum: 11,
                    gold: 22,
                    silver: 33,
                    copper: 44,
                }),
                Event::SkillsSnapshot {
                    skills: vec![
                        SkillValue {
                            skill_id: 1,
                            value: 12,
                        },
                        SkillValue {
                            skill_id: 3,
                            value: 34,
                        },
                    ],
                },
                Event::AlternateAdvancementSnapshot(AlternateAdvancementSnapshot {
                    purchased: vec![AlternateAbilityRank {
                        ability_id: 700,
                        rank: 3,
                    }],
                    spent_points: None,
                    assigned_points: None,
                    unspent_points: 7,
                    experience: 6_543,
                }),
            ]
        );
    }

    #[test]
    fn new_zone_emits_identity_then_environment_in_wire_order() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        let decoded = session.decode(
            StreamKind::Zone,
            OpcodeId(0x15e1),
            Dir::ServerToClient,
            &new_zone(),
        );
        assert_eq!(
            decoded.events,
            vec![
                Event::ZoneChanged(seq_events::ZoneInfo {
                    short_name: "qeynos".into(),
                    long_name: "South Qeynos".into(),
                }),
                Event::ZoneEnvironmentChanged(seq_events::ZoneEnvironment {
                    zone_file: "qeynos.eqg".into(),
                    experience_multiplier: 1.25,
                    safe_x: 10.25,
                    safe_y: 20.5,
                    safe_z: 30.75,
                }),
            ]
        );
    }

    #[test]
    fn eql_transition_request_does_not_reset_until_a_real_boundary() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        session.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(99),
        );
        let decoded = session.decode(
            StreamKind::Zone,
            OpcodeId(0x2960),
            Dir::ClientToServer,
            &[0; 484],
        );
        assert_eq!(
            decoded.events,
            vec![Event::ZoneTransition {
                character_name: String::new(),
                zone_id: None,
                instance_id: None,
                confirmed: false,
            }]
        );
        assert_eq!(session.self_identity().provisional_id, 99);

        assert_eq!(
            session.flush(FlushReason::ZoneTransition),
            vec![Event::SessionReset {
                reason: SessionResetReason::ZoneTransition,
            }]
        );
        assert_eq!(session.self_identity(), SelfIdentity::default());
    }

    #[test]
    fn interleaved_sessions_do_not_share_correlation() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut first = eql_session(Arc::clone(&registry));
        let mut second = eql_session(registry);
        first.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(101),
        );
        second.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(202),
        );
        assert_eq!(first.self_identity().provisional_id, 101);
        assert_eq!(second.self_identity().provisional_id, 202);
    }

    #[test]
    fn catalog_swap_preserves_session_correlation_state() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(Arc::clone(&registry));
        session.decode(
            StreamKind::Zone,
            OpcodeId(0x6987),
            Dir::ClientToServer,
            &self_pos(10),
        );
        registry
            .replace_from_str(
                BackendId::Eql,
                "[[zone]]\nid='1234'\nname='OP_ClientUpdate'\n",
            )
            .unwrap();
        let batch = session.decode(
            StreamKind::Zone,
            OpcodeId(0x1234),
            Dir::ClientToServer,
            &self_pos(11),
        );
        assert_eq!(batch.protocol_generation, ProtocolGeneration(2));
        assert_eq!(session.self_identity().provisional_id, 11);
    }

    #[test]
    fn session_owns_loot_pairing_and_preserves_confirmation_sequence() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        registry
            .replace_from_str(
                BackendId::Eql,
                "[[zone]]\nid='0001'\nname='OP_LootMessage'\n\n[[zone]]\nid='0002'\nname='OP_LootTransaction'\n",
            )
            .unwrap();
        let mut session = eql_session(registry);

        let mut message = 286u32.to_le_bytes().to_vec();
        message
            .extend_from_slice(b"--You have looted a Fine Steel Sword from a goblin's corpse.--\0");
        assert_eq!(
            session
                .decode_at(
                    StreamKind::Zone,
                    OpcodeId(1),
                    Dir::ServerToClient,
                    &message,
                    123,
                )
                .disposition,
            DecodeDisposition::Decoded
        );

        let mut transaction = [0u8; 36];
        transaction[0..2].copy_from_slice(&7u16.to_le_bytes());
        transaction[4..8].copy_from_slice(&1004u32.to_le_bytes());
        transaction[12..16].copy_from_slice(&900u32.to_le_bytes());
        transaction[16..20].copy_from_slice(&1u32.to_le_bytes());
        transaction[20..24].copy_from_slice(&9u32.to_le_bytes());
        session.decode_at(
            StreamKind::Zone,
            OpcodeId(2),
            Dir::ServerToClient,
            &transaction,
            124,
        );
        let rows = session.take_loot_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ts, 123);
        assert_eq!(rows[0].item_name, "Fine Steel Sword");
        assert_eq!(rows[0].corpse_id, 900);
        assert_eq!(rows[0].sequence, 9);
    }

    #[test]
    fn flush_closes_pending_loot() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        registry
            .replace_from_str(
                BackendId::Eql,
                "[[zone]]\nid='0001'\nname='OP_LootMessage'\n",
            )
            .unwrap();
        let mut session = eql_session(registry);
        let mut message = 286u32.to_le_bytes().to_vec();
        message
            .extend_from_slice(b"--You have looted a Fine Steel Sword from a goblin's corpse.--\0");
        session.decode_at(
            StreamKind::Zone,
            OpcodeId(1),
            Dir::ServerToClient,
            &message,
            123,
        );
        assert!(session.take_loot_rows().is_empty());
        assert!(session.flush(FlushReason::ReplayEnd).is_empty());
        assert_eq!(session.take_loot_rows().len(), 1);
    }

    #[test]
    fn corpse_coordinates_follow_entity_kind() {
        for kind in [0, 2, 10] {
            let mut entities = EntityIndex::default();
            entities.add(99, "Firona", kind);
            let mut position = seq_events::Point3 {
                x: 4.25,
                y: 5.5,
                z: 6.75,
            };
            entities.corpse_position(99, &mut position);
            assert_eq!(
                position,
                seq_events::Point3 {
                    x: 5.5,
                    y: 4.25,
                    z: 6.75,
                }
            );
        }

        let mut entities = EntityIndex::default();
        entities.add(100, "a rat", 1);
        let mut position = seq_events::Point3 {
            x: 4.25,
            y: 5.5,
            z: 6.75,
        };
        entities.corpse_position(100, &mut position);
        assert_eq!(position.x, 4.25);
        assert_eq!(position.y, 5.5);
    }

    #[test]
    fn loadout_and_zone_boundaries_keep_identity_semantic() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let mut session = eql_session(registry);
        let profile = profile("Firona");
        session.apply_session_semantics(
            Decoded::One(Event::PlayerProfile(profile)),
            "OP_PlayerProfile",
            Dir::ServerToClient,
            &[],
            0,
        );
        let spawn = seq_events::SpawnInfo {
            id: 500,
            name: "Firona".into(),
            last_name: "Vie".into(),
            race: 1,
            class_: 2,
            deity: 3,
            level: 4,
            npc: 0,
            cur_hp: 90,
            max_hp: Some(100),
            guild_id: 0,
            guild_server_id: 0,
            class_mask: 4,
            pos: None,
            velocity: seq_events::Velocity::default(),
            delta_heading: None,
            animation: None,
            equipment_models: None,
        };
        session.apply_session_semantics(
            Decoded::One(Event::SpawnAdded(spawn)),
            "OP_ZoneEntry",
            Dir::ServerToClient,
            &[],
            0,
        );

        let changed = session.apply_session_semantics(
            Decoded::One(Event::LoadoutSwap {
                spawn_id: 500,
                level: 60,
                class: 8,
                race: 9,
            }),
            "OP_LoadoutSwap",
            Dir::ServerToClient,
            &[],
            0,
        );
        assert!(matches!(
            changed,
            Decoded::Many(events)
                if matches!(events.as_slice(), [Event::PlayerIdentityUpdated(identity)]
                    if identity.spawn_id == Some(500)
                        && identity.level == 60
                        && identity.class_ == 8
                        && identity.race == 9)
        ));

        assert_eq!(
            session.flush(FlushReason::ZoneTransition),
            vec![Event::SessionReset {
                reason: SessionResetReason::ZoneTransition,
            }]
        );
        assert_eq!(session.player_id, None);
        assert_eq!(
            session
                .player_identity
                .as_ref()
                .and_then(|identity| identity.spawn_id),
            None
        );
    }
}
