//! Neutral, backend-agnostic decode vocabulary + the backend contract.
//!
//! Every server backend (live/test/eql) decodes its own wire format into these
//! shared types, so the daemon consuming them never learns which server it is
//! talking to. A backend maps its per-server structs (Live `Spawn` vs eql
//! `ZoneSpawn`, different heading conventions, …) into one `Event` shape; the
//! daemon just applies events.
//!
//! This crate holds NO wire-decode logic — only the vocabulary, the trait, and
//! shared neutral math — so a backend depending on it is never coupled to
//! another server's parsers.

/// Packet direction on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Server → client (spawns, zone, profile, …).
    ServerToClient,
    /// Client → server (e.g. the player's own position updates).
    ClientToServer,
}

/// A world position in EQ coordinates; heading already normalized to degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// 0..359, converted from the backend's raw heading bits via [`heading_deg`].
    pub heading_deg: u16,
}

/// Velocity components carried by one entity packet, in integer game-world
/// units. Each component is optional because the compact movement wire may
/// update only a subset. Absence is a protocol fact; compatibility projectors
/// decide whether their older public format should emit zero for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Velocity {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub z: Option<i32>,
}

/// One absolute vital value. Some packets carry only the current value, so a
/// maximum is optional rather than synthesized from a host convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VitalValue {
    pub current: i32,
    pub maximum: Option<i32>,
}

/// A partial update to the local player's combat-resource values.
///
/// `None` means that the packet did not carry that resource. Consumers merge
/// the present fields into their current player snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerVitals {
    pub health: Option<VitalValue>,
    pub mana: Option<VitalValue>,
    pub endurance: Option<VitalValue>,
}

impl PlayerVitals {
    pub const fn any(self) -> bool {
        self.health.is_some() || self.mana.is_some() || self.endurance.is_some()
    }
}

/// Current local-player identity fields. `spawn_id` is absent until the
/// ordered session has resolved the real moving spawn. In particular, an EQL
/// phantom-twin id is never exposed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerIdentity {
    pub spawn_id: Option<u32>,
    pub name: String,
    pub last_name: String,
    pub race: u32,
    pub class_: u32,
    pub deity: u32,
    pub level: u32,
    pub class_mask: u32,
}

/// A partial local-player appearance update.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlayerAppearance {
    pub race: Option<u32>,
    pub gender: Option<u8>,
    pub animation: Option<u32>,
}

/// A precise world-space point for packets whose wire coordinates are floats.
///
/// Host projectors may round these values for an older public contract. Rust
/// keeps the decoded coordinates so that compatibility policy does not become
/// shared game state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A spawn (NPC, PC, or corpse) entering the zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnInfo {
    pub id: u32,
    pub name: String,
    pub last_name: String,
    pub race: u32,
    pub class_: u32,
    pub deity: u32,
    pub level: u8,
    /// Nonzero for NPCs.
    pub npc: u8,
    pub cur_hp: u32,
    /// `None` when the spawn packet carries no max HP (Live — it arrives later
    /// via HP opcodes); `Some` for backends that ship it inline (eql).
    pub max_hp: Option<u32>,
    pub guild_id: u32,
    /// Guild ids are only unique within a guild server, so the pair is the key
    /// into the guild map built from [`Event::GuildsInZone`]. 0 on backends that
    /// don't send it.
    pub guild_server_id: u32,
    /// EQL multiclass bitmask (bit N = class N). 0 on non-multiclass wires.
    pub class_mask: u32,
    /// Present when the spawn packet carries position (eql); `None` when
    /// the backend cannot locate it reliably.
    pub pos: Option<Pos>,
    /// Initial per-axis velocity carried by the spawn packet. Live carries all
    /// three components; eql currently has no validated fields for them.
    pub velocity: Velocity,
    /// Initial heading delta carried by the spawn packet.
    pub delta_heading: Option<i16>,
    /// Initial movement or pose animation carried by the spawn packet.
    pub animation: Option<i16>,
    /// Nine visual equipment model ids in worn-slot order. `None` means that
    /// this backend did not decode equipment, while `Some([0; 9])` means the
    /// packet explicitly reported nine empty models.
    pub equipment_models: Option<[u32; 9]>,
}

/// The local player's character profile (self identity + vitals).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    pub name: String,
    pub last_name: String,
    pub class_: u32,
    pub level: u8,
    pub race: u32,
    pub deity: u32,
    pub cur_hp: u32,
    pub mana: u32,
    /// Purchased-AA descIDs, paired index-for-index with `aa_values` (ranks).
    pub aa_ids: Vec<u32>,
    pub aa_values: Vec<u32>,
    /// Total AA points spent (the profile's `aa_spent`).
    pub aa_spent: u32,
    /// AA points assigned to abilities. This can differ from `aa_spent` on
    /// Live because spent points also include consumable abilities.
    pub aa_assigned: u32,
    /// AA points ready for the player to spend.
    pub aa_unspent: u32,
    /// Progress toward the next AA point, on the 0..100000 wire scale.
    pub aa_experience: u32,
    /// Learned-skill values, indexed by skill id (eql fills this; Live surfaces
    /// skills by another path, so it's empty there). `0xFFFFFFFF` = the skill is
    /// unavailable to this class; the consumer filters those (and 0) out.
    pub skills: Vec<u32>,
    /// EQL multiclass bitmask (bit N = class N). 0 on non-multiclass wires.
    pub class_mask: u32,
    /// Base stats — the loadout roll (race + primary + additional classes),
    /// not gear-inclusive totals. 0 where a backend doesn't surface them.
    pub str_: u32,
    pub sta: u32,
    pub cha: u32,
    pub dex: u32,
    pub int_: u32,
    pub agi: u32,
    pub wis: u32,
    /// On-hand carried coins (the base the OP_MoneyUpdate purse resyncs).
    pub platinum: u32,
    pub gold: u32,
    pub silver: u32,
    pub copper: u32,
}

/// Zone identity from OP_NewZone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneInfo {
    pub short_name: String,
    pub long_name: String,
}

/// Non-identity zone settings carried by `OP_NewZone`.
///
/// This is separate from [`ZoneInfo`] because clients can switch maps as soon as
/// the names arrive, while consumers that do not model safe points or experience
/// modifiers may explicitly ignore this event.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneEnvironment {
    pub zone_file: String,
    pub experience_multiplier: f32,
    pub safe_x: f32,
    pub safe_y: f32,
    pub safe_z: f32,
}

/// Why the session discarded state that cannot survive a lifecycle boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SessionResetReason {
    EnterWorld = 0,
    PlayerProfile = 1,
    ZoneTransition = 2,
    Explicit = 3,
}

/// One active-buff entry from an OP_BuffList (belongs to the list's owner spawn).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffEntry {
    pub spell_id: u32,
    /// Server-side remaining duration in ticks; `<= 0` = permanent.
    pub remaining_ticks: i32,
    /// Buff-window slot index.
    pub slot: u32,
    /// Who cast it, as spelled on the wire (empty when the wire's is). The
    /// owner's list mixes their own buffs with the ones the player cast on
    /// them; only the caster separates the two.
    pub caster: String,
}

/// One lootable item on a corpse (OP_LootDrops). `item_id` is parsed from the
/// item-link header; `icon` is the dragitem-atlas id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootItemInfo {
    pub name: String,
    pub icon: u32,
    pub item_id: u32,
}

/// A final item or coin acquisition after the ordered session has paired the
/// loot narration with its transaction confirmation.
///
/// `complete` is false at a reset, replay end, or shutdown when only one half
/// reached the session. Optional ids preserve that distinction without making
/// a host interpret zero sentinels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootAcquisition {
    pub timestamp: i64,
    pub item_name: String,
    pub item_id: Option<u32>,
    pub quantity: u32,
    pub corpse_name: String,
    pub corpse_name_normalized: String,
    pub corpse_id: Option<u32>,
    pub zone_short: String,
    pub zone_base: String,
    pub instance: String,
    pub sold: bool,
    pub coin_copper: u32,
    /// `inventory`, `sold`, `created`, `dropped`, `destroyed`, a named storage
    /// destination, or `corpse_coin`.
    pub disposition: String,
    pub looter: String,
    pub sequence: Option<u32>,
    pub from_corpse: bool,
    pub complete: bool,
}

/// Final meaning of one corpse-loot window after per-corpse duplicate
/// suppression. Reopening an unchanged corpse emits no second snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpseLootSnapshot {
    pub timestamp: i64,
    pub corpse_id: u32,
    pub corpse_name: String,
    pub corpse_name_normalized: String,
    pub zone_short: String,
    pub zone_base: String,
    pub instance: String,
    pub looter: String,
    pub items: Vec<LootItemInfo>,
}

/// One guild present in the zone, from the guild-in-zone opcodes. A spawn's
/// guild is on the wire only as the (guild_id, server_id) pair — these records
/// are the sole source of the NAME, so a consumer keys its guild map on the
/// pair. `server_id` is part of the key, not decoration: ids are only unique
/// within a guild server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInZone {
    pub guild_id: u32,
    pub server_id: u32,
    pub name: String,
}

/// `parent_slot` value meaning "not inside a bag".
pub const TOP_LEVEL_SLOT: u16 = 0xFFFF;
/// Highest worn slot index (Charm..Ammo); above this is inventory/cursor.
pub const MAX_WORN_SLOT: u16 = 22;

/// One item the character owns (see [`Event::ItemSet`]).
///
/// `serial` is a per-INSTANCE id, so two copies of the same item type share an
/// `item_id` but never a `serial` — key a cache on `item_id` for templates and
/// on `serial` only when you mean this exact copy.
///
/// The stat order was pinned against a real in-game tooltip, NOT inferred: six
/// of the seven land exactly, and the seventh (CHA) reads one lower because the
/// tooltip was displaying modified rather than base values. The five resists
/// keep slot order, since the tooltipped item carries the same value in all
/// five and nothing yet distinguishes them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTemplate {
    pub serial: String,
    pub name: String,
    /// Usually equal to `name`; a CONTAINER carries its description here.
    pub lore_name: String,
    pub item_id: u32,
    /// Drag-item atlas id. EQL carries it; the current Live item wrapper does
    /// not, so absence must not turn into a real icon id of zero.
    pub icon: Option<u32>,
    /// Stack size or remaining charges. Present on Live's wrapper and absent
    /// from the validated EQL bulk records.
    pub stack_count: Option<u32>,
    /// Item weight in tenths of a unit. Keeping the integer wire value avoids
    /// float equality and preserves the exact value through adapters.
    pub weight_tenths: Option<u32>,
    /// Live's decoded flag word. EQL has no validated equivalent yet.
    pub flags: Option<u32>,
    /// Live's corruption resist. EQL's five decoded resist slots do not expose
    /// a sixth value, so this remains absent there.
    pub corruption: Option<i32>,
    /// Standard EQ slot bitmask; 0 = not equippable. This is where the item
    /// COULD go — see `container_id` for where it IS.
    pub slot_mask: u32,
    /// Which container holds it (33 Exaltation, 37 activated key ring, 39
    /// equipment key ring, 1 carried inventory; 0 and 25 unidentified). Group by
    /// this to separate the storage spaces.
    pub container_id: u32,
    /// Slot index WITHIN `container_id`; unique per container. Combined with
    /// `parent_slot == TOP_LEVEL_SLOT` and `container_id == 0` this is the
    /// standard EQ slot enum: 0-22 worn (Charm..Ammo), 23-30 personal
    /// inventory, 35 cursor.
    pub container_slot: u16,
    /// Parent bag's slot when the item sits INSIDE a bag; [`TOP_LEVEL_SLOT`]
    /// otherwise. With `container_slot` this is Live's mainSlot/subSlot pair.
    pub parent_slot: u16,
    /// `[STR, STA, AGI, DEX, CHA, INT, WIS]`, signed. BASE values: an in-game
    /// tooltip showing "modified" numbers read one higher on CHA.
    pub stats: Vec<i32>,
    /// Five resists. Internal ORDER unverified — the one tooltipped item has 3
    /// in all five, so nothing distinguishes them. Do not relabel on a guess.
    pub resists: Vec<i32>,
    pub hp: i32,
    pub mana: i32,
    pub endurance: i32,
    pub ac: i32,
}

/// A normalized item location. It is copied out separately on move events so
/// a reducer can vacate the old equipment slot before applying the new item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemLocation {
    pub container_id: u32,
    pub container_slot: u16,
    pub parent_slot: u16,
}

impl ItemTemplate {
    pub const fn location(&self) -> ItemLocation {
        ItemLocation {
            container_id: self.container_id,
            container_slot: self.container_slot,
            parent_slot: self.parent_slot,
        }
    }
}

/// The carried purse without any host-specific total or display formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoneyBalance {
    pub platinum: u32,
    pub gold: u32,
    pub silver: u32,
    pub copper: u32,
}

/// One learned skill and its absolute value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillValue {
    pub skill_id: u32,
    pub value: u32,
}

/// The regular per-level experience bar and the level it belongs to when the
/// ordered session knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceProgress {
    pub experience: u32,
    pub level: Option<u32>,
    /// Present only when the same packet reports a level transition.
    pub previous_level: Option<u32>,
}

/// One purchased alternate-advancement ability and its absolute rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateAbilityRank {
    pub ability_id: u32,
    pub rank: u32,
}

/// Player AA state from an authoritative profile snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateAdvancementSnapshot {
    pub purchased: Vec<AlternateAbilityRank>,
    /// EQL does not carry these two counters independently.
    pub spent_points: Option<u32>,
    pub assigned_points: Option<u32>,
    pub unspent_points: u32,
    pub experience: u32,
}

/// Incremental AA bar and unspent-point state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateAdvancementProgress {
    pub experience: u32,
    pub unspent_points: u32,
}

/// One mapping from an AA rank id to its localized title string id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateAbilityDefinition {
    pub ability_id: u32,
    pub title_string_id: u32,
}

impl ItemTemplate {
    /// Is this item EQUIPPED? Worn gear is top-level in the possessions
    /// container at a slot inside the worn range — inventory and cursor share
    /// that container at higher indices, and every other container is storage.
    pub fn is_worn(&self) -> bool {
        self.container_id == 0
            && self.parent_slot == TOP_LEVEL_SLOT
            && self.container_slot <= MAX_WORN_SLOT
    }
}

/// One row of the guild roster (see [`Event::GuildRoster`]). `class` is the
/// primary (lowest set bit of `class_mask`); `banker`/`alt` are the two flags
/// split from the wire's packed field. `zone_id` 0 = offline; `last_on` is unix
/// seconds (0 = never).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildRosterMember {
    pub name: String,
    pub level: u32,
    pub class: u32,
    pub class_mask: u32,
    pub rank: u32,
    pub last_on: u32,
    pub banker: bool,
    pub alt: bool,
    pub full_member: bool,
    pub public_note: String,
    pub zone_id: u32,
}

/// A single door / static object row from OP_SpawnDoor.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorInfo {
    pub id: u32,
    pub name: String,
    pub position: Point3,
    /// Native door heading. Unlike mobile-spawn headings, this field is already
    /// a float and has not been proven to use the same bit-scaled convention.
    pub heading: f32,
    pub incline: u32,
    pub size: u32,
    pub open_type: u8,
    pub state: u8,
    pub invert_state: u8,
    /// `None` replaces the wire's `0xffff_ffff` "not a zone line" sentinel.
    pub zone_point_id: Option<u32>,
}

/// A ground object or dropped item in its native identity namespace.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundItemInfo {
    pub id: u32,
    /// Actor-definition model name. Resolving it to an item display name needs
    /// the item database and belongs in a host projection or later correlation.
    pub actor_definition: String,
    pub position: Point3,
    /// Native heading when present. EQL ground records carry no heading.
    pub heading: Option<f32>,
}

/// One destination trigger from `OP_SendZonePoints`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonePointInfo {
    /// Wire trigger id. Modern Test records identify the portal by actor name
    /// instead and therefore leave this absent.
    pub trigger_id: Option<u32>,
    /// Test portal/object actor name. Live and EQL counted records do not carry
    /// one.
    pub actor_definition: Option<String>,
    pub position: Point3,
    pub heading: f32,
    /// Destination ids are absent from modern Test records. A compatibility
    /// projector may resolve the actor name, but the shared event does not
    /// invent zero-valued ids.
    pub destination_zone_id: Option<u16>,
    pub destination_instance_id: Option<u16>,
}

/// A decoded, backend-neutral world event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Stateful correlators were reset before the next event in this batch was
    /// observed. Hosts must apply this before later events in the batch.
    SessionReset { reason: SessionResetReason },
    /// The session resolved or changed the local player's identity. This is
    /// emitted from profiles, the real self spawn, and EQL loadout swaps.
    PlayerIdentityUpdated(PlayerIdentity),
    /// The local player moved. `spawn_id` is absent on an EQL cold attach until
    /// a name-matched moving spawn appears. The wire's phantom id is internal.
    PlayerMoved { spawn_id: Option<u32>, pos: Pos },
    /// One packet changed one or more local-player vital values.
    PlayerVitalsUpdated(PlayerVitals),
    /// One non-player spawn's health changed.
    SpawnHealthUpdated { id: u32, current: i32, maximum: i32 },
    /// The local player died. A zero wire killer id is represented as absent.
    PlayerDied { killer_id: Option<u32> },
    /// A non-player spawn died. The entity remains present as a corpse.
    SpawnDied { id: u32, killer_id: Option<u32> },
    /// A non-player spawn changed class, race, or level without changing id.
    SpawnIdentityUpdated {
        id: u32,
        level: u32,
        class_: u32,
        race: u32,
    },
    /// The local player's visible race, gender, or pose changed.
    PlayerAppearanceUpdated(PlayerAppearance),
    /// A spawn entered the zone (OP_ZoneEntry).
    SpawnAdded(SpawnInfo),
    /// A spawn moved (OP_MobUpdate / OP_NpcMoveUpdate).
    SpawnMoved {
        id: u32,
        pos: Pos,
        /// Per-axis velocity values present on this movement wire.
        velocity: Velocity,
        /// Heading delta present on this movement wire.
        delta_heading: Option<i16>,
        /// Movement or pose animation present on this movement wire.
        animation: Option<i16>,
    },
    /// A spawn left the zone (OP_RemoveSpawn / OP_DeleteSpawn).
    SpawnRemoved { id: u32 },
    /// A spawn changed its server-provided display name. `id` is present when
    /// the ordered session saw one unambiguous matching spawn first. A
    /// mid-session attachment or duplicate old names leave it absent rather
    /// than inventing an id.
    SpawnRenamed {
        id: Option<u32>,
        old_name: String,
        new_name: String,
    },
    /// Low-level OP_Death result retained for direct backend callers. A
    /// stateful session converts it to [`Event::PlayerDied`] or
    /// [`Event::SpawnDied`] and converts a zero killer id to `None`.
    SpawnKilled { deceased_id: u32, killer_id: u32 },
    /// Low-level OP_HPUpdate result retained for direct backend callers. A
    /// stateful session emits player or spawn health with final ownership.
    SpawnHp { id: u32, cur: i32, max: i32 },
    /// One packet of the multiplexed stat-sync channel (eql OP_HPUpdate), which
    /// carries spawn HP plus the local player's mana/endurance together. Kept as
    /// ONE event per packet on purpose: splitting it into per-stat events makes a
    /// consumer emit several near-identical player snapshots for a single packet.
    ///
    /// A stateful session owns the self/other split. Direct backend callers may
    /// still inspect this wire-shaped compatibility variant. Routing rules:
    ///   * HP is meaningful only when `has_hp && hp_max > 0`. For the self it is
    ///     real cur/max; for other spawns the narrow form is a percentage.
    ///   * mana/endurance are the local player's only, and only when `wide` —
    ///     the narrow form is a u8 percent with a synthesized max of 100, which
    ///     is useless as a max.
    ///
    /// eql has no standalone endurance opcode, so this is its sole endurance feed.
    StatSync {
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
    },
    /// Low-level OP_ClientUpdate result retained for direct backend callers. A
    /// stateful session emits [`Event::PlayerMoved`].
    ///
    /// `spawn_id` is the id the client stamps on its own outbound report. It is
    /// carried here because it is the only self-identifying field that keeps
    /// arriving mid-session. On EQL it names the phantom twin, so only the
    /// session may adopt it.
    SelfPos {
        pos: Pos,
        spawn_id: u32,
        velocity: Velocity,
        delta_heading: Option<i16>,
        animation: Option<i16>,
    },
    /// A spawn changed pose/animation (OP_SpawnAppearance2 type 6: 110=sit,
    /// 100=stand, 111=duck). Only the pose subtype is surfaced — other
    /// appearance types carry no spawn field. The consumer updates the tracked
    /// spawn's animation and re-emits it (ignores an unknown spawn).
    SpawnAnimation { spawn_id: u32, animation: u32 },
    /// A spawn changed race/model via an illusion (OP_Illusion). The consumer
    /// merges the new race into the tracked spawn and re-renders it; the daemon
    /// ignores it for an unknown spawn (the spawn arrives already illusioned).
    SpawnIllusion {
        spawn_id: u32,
        race: u32,
        gender: u8,
    },
    /// Guilds present in the current zone, resolving guild ids to names
    /// (OP_GuildsInZoneList on zone-in, OP_NewGuildInZone as guilded players
    /// arrive — the latter is just a one-element list, so both map here).
    ///
    /// The consumer accumulates these into a guild map and back-fills spawns:
    /// a spawn can arrive before its guild is named, so tagging only on receipt
    /// would permanently miss those.
    GuildsInZone { guilds: Vec<GuildInZone> },
    /// A Norrath time sync (OP_TimeOfDay). The consumer surfaces it as a time
    /// sync-point (standalone + in its snapshot) so the client can track the
    /// game clock. `day` 1..28, `month` 1..12, `hour` 1..24, `minute` 0..59.
    TimeOfDay {
        year: u32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    },
    /// A zone transition started or was confirmed. The eql request has no
    /// destination fields, so those values are absent for that backend.
    ZoneTransition {
        character_name: String,
        zone_id: Option<u32>,
        instance_id: Option<u32>,
        confirmed: bool,
    },
    /// Zone changed (OP_NewZone).
    ZoneChanged(ZoneInfo),
    /// Safe point and other environment settings from the same OP_NewZone.
    /// This immediately follows [`Event::ZoneChanged`] in the decode batch.
    ZoneEnvironmentChanged(ZoneEnvironment),
    /// The local player's profile (OP_PlayerProfile).
    PlayerProfile(ProfileInfo),
    /// The player's active STANCE changed (eql OP_Stance echo). `name` is the
    /// resolved display name (e.g. "Defense"), or "#<id>" for an unknown id —
    /// ready to show. The consumer stores it on the player and re-emits stats.
    Stance { name: String },
    /// The player's active INVOCATION changed (eql OP_Invocation echo). `name`
    /// is the resolved display name (e.g. "Recover"), or "#<id>" if unknown.
    Invocation { name: String },
    /// A spawn-inspect result (OP_InspectAnswer): the 23 worn-slot item names
    /// (empty string = empty slot) + the inspected player's bio text. Icons on
    /// the wire are dropped (no home). The consumer forwards it to whoever
    /// requested the inspect.
    InspectAnswer {
        spawn_id: u32,
        item_names: Vec<String>,
        bio: String,
    },
    /// The full guild roster (OP_GuildMemberList), authoritative and replacing.
    /// The consumer replaces the whole roster; an empty `members` means the
    /// parse failed its canary (or the guild is empty) and should be ignored,
    /// not wiped over.
    GuildRoster {
        guild_id: u32,
        members: Vec<GuildRosterMember>,
    },
    /// The world->zone handoff (OP_ZoneServerInfo): which zone server the client
    /// was just told to connect to.
    ///
    /// A REPORT, not a routing input. scry feeds every UDP flow to the SOE layer
    /// and lets each decode on its own merits, so it needs no port binding —
    /// consuming this to reintroduce one would give that up. Surface it, do not
    /// route on it.
    ZoneServerInfo { host: String, port: u32 },
    /// Every item the character owns, with its template data (OP_ItemPacket).
    ///
    /// Authoritative and REPLACING, like [`Event::GuildRoster`]: the server
    /// answers a request with the whole set, so a consumer replaces its cache
    /// rather than merging. An empty `items` means the parse found no records
    /// and should be ignored, not treated as "you own nothing".
    ///
    /// This is the only source of an item's stats — loot events carry name and
    /// icon but nothing more, and the player profile carries no item data at
    /// all.
    ItemSet { items: Vec<ItemTemplate> },
    /// ONE item, learned incrementally (Live's per-item OP_ItemPacket, which
    /// fires on each slot move and zone-in pickup). The consumer ACCUMULATES
    /// these — unlike [`Event::ItemSet`], which replaces the whole set.
    ItemLearned { item: ItemTemplate },
    /// Final authoritative inventory snapshot. The ordered session removes
    /// duplicate serials before emitting it. An empty snapshot is never
    /// synthesized from a malformed or request-side packet.
    InventorySnapshot { items: Vec<ItemTemplate> },
    /// One changed inventory instance. `previous_location` is present when the
    /// session already knew the serial, including moves between inventory and
    /// worn slots. Byte-for-byte duplicate updates do not emit this event.
    InventoryItemUpdated {
        item: ItemTemplate,
        previous_location: Option<ItemLocation>,
    },
    /// Authoritative worn-item view derived from an inventory snapshot. Items
    /// are sorted by worn slot and keep their complete optional fields.
    EquipmentSnapshot { items: Vec<ItemTemplate> },
    /// One worn slot changed. `item: None` vacates the slot. A move between two
    /// worn slots therefore emits the old-slot removal before the new value.
    EquipmentSlotUpdated {
        slot: u16,
        item: Option<ItemTemplate>,
    },
    /// The guild message of the day (OP_GuildMOTD). `message`/`sender` are empty
    /// when the guild has none set. The wire carries no guild id — the MOTD is
    /// implicitly the local player's guild — so the consumer stamps it from the
    /// roster it tracks (0 if none has arrived).
    GuildMotd { message: String, sender: String },
    /// One entry of the guild's rank-name table (OP_ExpandedGuildInfo). Guilds
    /// rename their ranks freely, so a `GuildRosterMember.rank` only means
    /// something against this table. One arrives per rank (right after the
    /// roster); the consumer accumulates them into a `rank -> name` map keyed by
    /// `rank_index` (1-based, matching the member rank field).
    GuildRankName {
        guild_id: u32,
        rank_index: u32,
        rank_name: String,
    },
    /// Low-level eql OP_LoadoutSwap result retained for direct backend callers.
    /// A stateful session emits a player or spawn identity update, changing
    /// their class + level. eql sends no OP_PlayerProfile on a swap, so this is
    /// the sole source of the new identity. The session owns the self/other
    /// split.
    /// `class` is the single resolved class, not the multiclass mask.
    LoadoutSwap {
        spawn_id: u32,
        level: u32,
        class: u32,
        race: u32,
    },
    /// The authoritative door/static-object set from OP_SpawnDoor. Door ids
    /// remain in their own namespace. A host projector creates any compatibility
    /// id used to merge them into a protobuf spawn list.
    Doors(Vec<DoorInfo>),
    /// A ground item was picked up / removed (OP_ClickObject, S>C removal side —
    /// the C>S click request is ignored). `drop_id` matches the GroundItem's
    /// drop_id; the consumer removes the drop it rendered for that id.
    GroundItemRemoved { drop_id: u32 },
    /// A ground item or static placeable (OP_GroundSpawn). It remains a ground
    /// entity here, not a fabricated spawn with a host-specific NPC type.
    GroundItem(GroundItemInfo),
    /// A corpse-location response. This is distinct from ordinary movement:
    /// the packet confirms the entity is a stationary corpse.
    CorpseLocated { id: u32, position: Point3 },
    /// The authoritative zone-point set from OP_SendZonePoints.
    ZonePoints(Vec<ZonePointInfo>),
    /// A damage event (OP_Action2). Ids only; the consumer resolves names from
    /// its spawn map. `kind` is the wire damage type; `spell_id` 0 = melee.
    Combat {
        source: u32,
        target: u32,
        kind: u32,
        damage: i32,
        spell_id: u32,
    },
    /// A spawn started casting a spell (OP_BeginCast). Ids only; the consumer
    /// resolves the caster name from its spawn map and the spell name from its
    /// spell DB. `cast_time_ms` is the wire cast time (0 = instant).
    SpawnCast {
        caster_id: u32,
        spell_id: u32,
        cast_time_ms: u32,
    },
    /// The player selected a target (OP_TargetMouse). `spawn_id` 0 = cleared.
    Targeted { spawn_id: u32 },
    /// The player considered a spawn (OP_Consider) — `spawn_id` is the target.
    Considered { spawn_id: u32 },
    /// One AA definition from the OP_SendAATable burst: maps a purchased AA's
    /// `desc_id` to a `title_sid` (a dbstring type-1 id → the AA's display name).
    AaTable { desc_id: u32, title_sid: u32 },
    /// Final AA title mapping. Repeated identical table rows are suppressed by
    /// the ordered session.
    AlternateAbilityDefined(AlternateAbilityDefinition),
    /// The regular experience bar (OP_ExpUpdate), 0..100000 within a level. On
    /// eql there is no discrete level packet — a wrap (decrease) is a ding.
    Exp { exp: u32 },
    /// Final regular experience state. The session merges level and exp
    /// packets and suppresses duplicate absolute values.
    ExperienceUpdated(ExperienceProgress),
    /// AA experience (OP_AAExpUpdate): `alt_exp` 0..100000 toward the next point,
    /// `aa_points` = unspent points.
    AaExp { alt_exp: u32, aa_points: u32 },
    /// Final incremental AA bar and unspent-point state.
    AlternateAdvancementUpdated(AlternateAdvancementProgress),
    /// Final authoritative purchased-AA and point snapshot from the profile.
    AlternateAdvancementSnapshot(AlternateAdvancementSnapshot),
    /// Hunger / thirst (OP_Stamina), in ticks till the next eat/drink. NOT the
    /// run/jump endurance bar — that is OP_EndUpdate.
    Stamina { food: u32, water: u32 },
    /// The player's current mana (OP_ManaChange). eql sends no max on the wire —
    /// the consumer tracks the observed high-water mark, like the daemon.
    ManaUpdate { mana: u32 },
    /// A single skill's new value (OP_SkillUpdate) — the consumer updates that
    /// skill id in the player's skill map.
    SkillUpdate { skill_id: u32, value: u32 },
    /// Final authoritative learned-skill snapshot. Invalid and zero values are
    /// absent; entries are sorted by skill id.
    SkillsSnapshot { skills: Vec<SkillValue> },
    /// Final absolute value for one learned skill. Duplicate values do not emit
    /// another semantic event.
    SkillValueUpdated(SkillValue),
    /// A corpse-loot event (OP_LootTransaction): an item confirmation carrying
    /// auto-sale proceeds, or the corpse's coin pile (`from_corpse`, item
    /// fields 0). Both are acquired coin — add `coin_copper` to the running
    /// total either way, like the daemon's adjustMoney.
    LootTransaction {
        corpse_id: u32,
        item_id: u32,
        quantity: u32,
        coin_copper: u32,
        from_corpse: bool,
    },
    /// A corpse's loot window (OP_LootDrops) — the lootable items on a corpse.
    LootDrops {
        corpse_id: u32,
        corpse_name: String,
        items: Vec<LootItemInfo>,
    },
    /// A deduplicated corpse window with session-owned timestamp and zone
    /// context. Direct backend callers still receive [`Event::LootDrops`].
    CorpseLootSnapshot(Box<CorpseLootSnapshot>),
    /// A paired item acquisition, corpse coin pile, or an explicitly incomplete
    /// half closed by a session boundary.
    LootAcquired(Box<LootAcquisition>),
    /// The carried purse (OP_MoneyUpdate, 0x6414). Denominations are NOT
    /// normalized on the wire — the consumer sums to total copper.
    Money {
        platinum: u32,
        gold: u32,
        silver: u32,
        copper: u32,
    },
    /// Final carried-purse state. Duplicate purse packets are suppressed.
    MoneyBalanceUpdated(MoneyBalance),
    /// A string-id server message (OP_SimpleMessage): `format_id` resolves to
    /// text via the eqstr DB (no args); `color` is the wire ChatColor.
    SimpleMessage { format_id: u32, color: u32 },
    /// A formatted server message (OP_FormattedMessage): `format_id` + `args`
    /// interpolate through the eqstr template; `color` is the wire ChatColor.
    FormattedMessage {
        format_id: u32,
        color: u32,
        args: Vec<String>,
    },
    /// A special server message (OP_SpecialMesg): carries `message` text
    /// directly + a `source` sender and a `target` spawn id (0 = none).
    SpecialMessage {
        color: u32,
        target: u32,
        source: String,
        message: String,
    },
    /// Auto-loot / sell narration (OP_LootMessage), e.g. "You looted a …" —
    /// `text` is already link-cleaned; the consumer shows it as general chat.
    /// `item_id`/`item_name` come off the link header, 0/empty when the line
    /// carries no item link — authoritative, so a consumer recording loot never
    /// has to recover the item from the prose.
    LootMessage {
        color: u32,
        text: String,
        item_id: u32,
        item_name: String,
    },
    /// A player chat message (OP_CommonMessage). `channel` is the MessageType
    /// (0=Guild 2=Group 3=Shout 4=Auction 5=OOC 7=Tell 8=Say 15=Raid). `target`
    /// is meaningful only for tells; `chat_color`/`channel_name` are 0/empty for
    /// channel messages (set by the formatted/UCS paths).
    Chat {
        channel: u32,
        from: String,
        target: String,
        text: String,
        chat_color: u32,
        channel_name: String,
    },
    /// The authoritative active-buff list for one spawn (eql OP_BuffList), sent
    /// at zone-in and on every buff change. A full snapshot: the consumer
    /// REPLACES that owner's buffs. `owner` == the player → the buff panel; a
    /// mob → that mob's effects. `remaining_ticks <= 0` on an entry = permanent.
    BuffList { owner: u32, entries: Vec<BuffEntry> },
    /// A member joined the group (OP_GroupFollow): `name` (the invitee) is added
    /// to the roster. `level` is the member's wire level (0 if absent).
    GroupFollow { name: String, level: u32 },
    /// A group departure (OP_GroupDisband / OP_GroupDisband2): `membername`
    /// leaves; `membername == yourname` means the whole group disbanded.
    GroupDisband {
        yourname: String,
        membername: String,
    },
    /// The player levelled (OP_LevelUpdate). `level` is absolute, not a delta —
    /// consumers should assign it rather than increment. `exp` is the post-ding
    /// exp value, which cross-references the next Exp event.
    LevelUpdate {
        level: u32,
        level_old: u32,
        exp: u32,
    },
    /// The client entered the world with a character. A
    /// [`Event::SessionReset`] immediately precedes this event.
    EnterWorld { character_name: String },
}

/// Outcome of decoding one app packet.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// One neutral event.
    One(Event),
    /// Several events from one packet.
    Many(Vec<Event>),
    /// The opcode was recognized and parsed, but carried nothing to surface
    /// (e.g. an eql stat-sync packet with only player mana/endurance).
    Ignored,
    /// This backend has no decoder for the opcode (caller may still count it).
    Unhandled,
    /// The opcode is handled but its payload failed to parse.
    Malformed,
}

/// The contract every server backend implements. The daemon holds a
/// `Box<dyn Backend>` and never branches on live/test/eql.
pub trait Backend: Send + Sync {
    /// Stable backend identifier (`"live"`, `"eql"`, …).
    fn name(&self) -> &'static str;

    /// Decode one app packet, keyed on the opcode's stable NAME. Patch-day id
    /// rotations are the caller's opcode-table concern (id→name), not the
    /// backend's — names stay stable across remaps.
    fn decode(&self, opcode: &str, dir: Dir, bytes: &[u8]) -> Decoded;
}

/// Legacy heading (`0..2^bits`, N per circle) → compass degrees `0..359`,
/// matching the daemon's `360 - ((raw * 360) >> bits)`.
pub fn heading_deg(raw: u16, bits: u32) -> u16 {
    let d = 360i32 - ((i32::from(raw) * 360) >> bits);
    (((d % 360) + 360) % 360) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_zero_is_zero() {
        assert_eq!(heading_deg(0, 12), 0);
    }

    #[test]
    fn heading_wraps_into_0_359() {
        for bits in [11u32, 12, 13] {
            let max = 1u16 << bits;
            for raw in [1u16, max / 4, max / 2, max - 1] {
                let d = heading_deg(raw, bits);
                assert!(d < 360, "raw={raw} bits={bits} -> {d}");
            }
        }
    }
}
