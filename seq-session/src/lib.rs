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
    ActiveBuff, AlternateAbilityDefinition, AlternateAbilityRank, AlternateAdvancementProgress,
    AlternateAdvancementSnapshot, Backend, CastInterruptionReason, ChatMessage, ChatMessageKind,
    Decoded, DynamicZoneState, ExperienceProgress, GroupMember, GroupRosterState, GuildMotdState,
    GuildRankNameEntry, GuildRankNamesState, GuildRosterMember, GuildRosterState, ItemTemplate,
    MoneyBalance, PlayerAppearance, PlayerIdentity, PlayerVitals, SessionResetReason, SkillValue,
    VitalValue,
};
#[cfg(feature = "backend-eql")]
use seq_events::{CorpseLootSnapshot, LootAcquisition};
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
    ) -> Vec<Event> {
        match self {
            #[cfg(feature = "backend-eql")]
            Self::Eql(state) => {
                state.observe_event(event, opcode_name, direction, payload, timestamp)
            }
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            Self::Live(_) | Self::Test(_) => {
                let _ = (event, opcode_name, direction, payload, timestamp);
                Vec::new()
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
    combat: CombatState,
    communication: CommunicationState,
}

const MAX_GROUP_PEERS: usize = 5;

#[derive(Default)]
struct CommunicationState {
    group_id: Option<u32>,
    group_slots: Vec<Option<GroupMember>>,
    group_complete: bool,
    guild_id: u32,
    guild_members: Vec<GuildRosterMember>,
    guild_complete: bool,
    guild_status: BTreeMap<String, GuildMemberStatus>,
    guild_rank_names: BTreeMap<u32, String>,
    dynamic_zone: DynamicZoneCorrelation,
    #[cfg(feature = "backend-eql")]
    ucs_mask: Option<u8>,
    #[cfg(feature = "backend-eql")]
    ucs_known_channels: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GuildMemberStatus {
    zone_id: u32,
    last_on: u32,
}

#[derive(Default)]
struct DynamicZoneCorrelation {
    state: Option<DynamicZoneState>,
    saw_info: bool,
    saw_switch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingCast {
    caster_id: Option<u32>,
    target_id: Option<u32>,
    spell_id: u32,
    cast_time_ms: Option<u32>,
    slot: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BuffKey {
    Slot(u32),
    Spell(u32),
}

#[derive(Default)]
struct CombatState {
    casts: BTreeMap<Option<u32>, PendingCast>,
    buffs: BTreeMap<(Option<u32>, BuffKey), ActiveBuff>,
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
            combat: CombatState::default(),
            communication: CommunicationState {
                group_slots: vec![None; MAX_GROUP_PEERS],
                ..CommunicationState::default()
            },
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
        timestamp: i64,
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
            self.apply_session_semantics(decoded, opcode_name, direction, payload, timestamp);
        batch(generation, decoded)
    }

    /// Decode one raw UCS port-9877 payload through this session.
    ///
    /// UCS has no application opcode and does not use the world/zone catalog,
    /// so hosts call this beside the numeric `decode` entry point. It does not
    /// change remote SEQA framing. Only the EQL backend handles this stream.
    pub fn decode_ucs(&mut self, direction: Direction, payload: &[u8]) -> DecodeBatch {
        let generation = self.protocol_registry.snapshot(self.backend).generation();
        if direction != Dir::ServerToClient {
            return DecodeBatch {
                protocol_generation: generation,
                disposition: DecodeDisposition::Ignored,
                events: Vec::new(),
            };
        }
        if payload.len() < 12 {
            return DecodeBatch {
                protocol_generation: generation,
                disposition: DecodeDisposition::Malformed,
                events: Vec::new(),
            };
        }

        #[cfg(feature = "backend-eql")]
        if self.backend == BackendId::Eql {
            for channel in seq_backend_eql::parse_ucs_channels(payload) {
                self.communication.ucs_known_channels.insert(channel);
            }
            let records = seq_backend_eql::parse_ucs_chat(payload);
            let mut events = Vec::with_capacity(records.len() * 2);
            for record in records {
                let compatibility = Event::UcsRecord {
                    channel_first: record.channel_first,
                    channel_rest: record.channel_rest.clone(),
                    channel_run: record.channel_run.clone(),
                    sender: record.sender.clone(),
                    message: record.message.clone(),
                    spam: record.spam,
                };
                let semantic = self.ucs_chat_message(
                    record.channel_first,
                    &record.channel_rest,
                    &record.channel_run,
                    record.sender,
                    record.message,
                    record.spam,
                );
                events.push(compatibility);
                events.push(Event::ChatMessage(semantic));
            }
            return DecodeBatch {
                protocol_generation: generation,
                disposition: if events.is_empty() {
                    DecodeDisposition::Ignored
                } else {
                    DecodeDisposition::Decoded
                },
                events,
            };
        }

        DecodeBatch {
            protocol_generation: generation,
            disposition: DecodeDisposition::Unhandled,
            events: Vec::new(),
        }
    }

    /// Close stateful correlators at a lifecycle boundary.
    ///
    /// The returned batch carries any incomplete loot acquisition before the
    /// reset marker. Compatibility loot rows remain available through the
    /// separate drain while hosts cut over.
    pub fn flush(&mut self, reason: FlushReason) -> Vec<Event> {
        let interruption_reason = match reason {
            FlushReason::Shutdown => CastInterruptionReason::Shutdown,
            FlushReason::ReplayEnd => CastInterruptionReason::ReplayEnd,
            FlushReason::ZoneTransition | FlushReason::Reset => {
                CastInterruptionReason::SessionReset
            }
        };
        let mut events = self.reset_correlations(interruption_reason);
        match reason {
            FlushReason::ZoneTransition => events.push(Event::SessionReset {
                reason: SessionResetReason::ZoneTransition,
            }),
            FlushReason::Reset => events.push(Event::SessionReset {
                reason: SessionResetReason::Explicit,
            }),
            FlushReason::Shutdown | FlushReason::ReplayEnd => {}
        }
        events
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
                output.extend(self.reset_correlations(CastInterruptionReason::SessionReset));
                output.push(Event::SessionReset { reason });
            }

            for event in self.apply_player_semantics(event, opcode_name, direction) {
                for mut event in self.apply_progression_semantics(event) {
                    self.apply_entity_semantics(&mut event);
                    for event in self.apply_combat_semantics(event, direction) {
                        for event in self.apply_communication_semantics(event, direction) {
                            let semantic_loot = self.decoder.observe_event(
                                &event,
                                opcode_name,
                                direction,
                                payload,
                                timestamp,
                            );
                            output.push(event);
                            output.extend(semantic_loot);
                        }
                    }
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

    fn apply_combat_semantics(&mut self, event: Event, direction: Direction) -> Vec<Event> {
        match event {
            Event::Combat {
                source,
                target,
                kind,
                damage,
                spell_id,
            } => {
                let source_id = self.directional_id(source, direction);
                let target_id = nonzero(target);
                let wire_spell_id = spell_id;
                let spell_id = valid_spell_id(wire_spell_id);
                if let Some(spell_id) = spell_id {
                    self.finish_matching_cast(source_id, spell_id);
                }
                vec![
                    Event::Combat {
                        source,
                        target,
                        kind,
                        damage,
                        spell_id: wire_spell_id,
                    },
                    Event::CombatDamage {
                        source_id,
                        target_id,
                        kind,
                        damage,
                        spell_id,
                    },
                ]
            }
            Event::SpellAction {
                source,
                target,
                spell_id,
                caster_level,
                kind,
            } => {
                let source_id = self.directional_id(source, direction);
                let target_id = nonzero(target);
                let wire_spell_id = spell_id;
                let Some(spell_id) = valid_spell_id(wire_spell_id) else {
                    return vec![Event::SpellAction {
                        source,
                        target,
                        spell_id: wire_spell_id,
                        caster_level,
                        kind,
                    }];
                };
                self.finish_matching_cast(source_id, spell_id);
                vec![
                    Event::SpellAction {
                        source,
                        target,
                        spell_id,
                        caster_level,
                        kind,
                    },
                    Event::SpellActionResolved {
                        source_id,
                        target_id,
                        spell_id,
                        caster_level: Some(caster_level),
                        kind: u32::from(kind),
                    },
                ]
            }
            Event::SpellCastRequest {
                slot,
                spell_id,
                target_id,
            } => {
                let wire_spell_id = spell_id;
                let Some(spell_id) = valid_spell_id(wire_spell_id) else {
                    return vec![Event::SpellCastRequest {
                        slot,
                        spell_id: wire_spell_id,
                        target_id,
                    }];
                };
                let cast = PendingCast {
                    caster_id: self.player_id,
                    target_id: nonzero(target_id),
                    spell_id,
                    cast_time_ms: None,
                    slot: (slot >= 0).then_some(slot),
                };
                let mut output = vec![Event::SpellCastRequest {
                    slot,
                    spell_id,
                    target_id,
                }];
                output.extend(self.start_cast(cast));
                output
            }
            Event::SpawnCast {
                caster_id,
                spell_id,
                cast_time_ms,
            } => {
                let wire_spell_id = spell_id;
                let Some(spell_id) = valid_spell_id(wire_spell_id) else {
                    return vec![Event::SpawnCast {
                        caster_id,
                        spell_id: wire_spell_id,
                        cast_time_ms,
                    }];
                };
                let caster_id = nonzero(caster_id);
                let previous = self.combat.casts.get(&caster_id).cloned().or_else(|| {
                    (caster_id.is_some()
                        && self
                            .combat
                            .casts
                            .get(&None)
                            .is_some_and(|cast| cast.spell_id == spell_id))
                    .then(|| self.combat.casts.remove(&None))
                    .flatten()
                });
                let target_id = previous
                    .as_ref()
                    .filter(|cast| cast.spell_id == spell_id)
                    .and_then(|cast| cast.target_id);
                let slot = previous
                    .as_ref()
                    .filter(|cast| cast.spell_id == spell_id)
                    .and_then(|cast| cast.slot);
                let cast = PendingCast {
                    caster_id,
                    target_id,
                    spell_id,
                    cast_time_ms: Some(cast_time_ms),
                    slot,
                };
                let mut output = vec![Event::SpawnCast {
                    caster_id: caster_id.unwrap_or_default(),
                    spell_id,
                    cast_time_ms,
                }];
                output.extend(self.start_cast(cast));
                output
            }
            Event::SimpleMessage { format_id, color } => {
                let mut output = vec![Event::SimpleMessage { format_id, color }];
                if direction == Dir::ServerToClient && interrupts_cast(format_id) {
                    output
                        .extend(self.interrupt_player_cast(CastInterruptionReason::ServerMessage));
                }
                output
            }
            Event::BuffList { owner, entries } => {
                let compatibility = Event::BuffList {
                    owner,
                    entries: entries.clone(),
                };
                let mut output = vec![compatibility];
                output.extend(self.apply_buff_snapshot(owner, entries));
                output
            }
            Event::BuffWire {
                spawn_id,
                spell_id,
                form,
                slot,
                duration_ticks,
                change_type,
            } => {
                let compatibility = Event::BuffWire {
                    spawn_id,
                    spell_id,
                    form,
                    slot,
                    duration_ticks,
                    change_type,
                };
                let mut output = vec![compatibility];
                output.extend(self.apply_buff_wire(spawn_id, spell_id, form, slot, duration_ticks));
                output
            }
            other => vec![other],
        }
    }

    fn directional_id(&self, wire_id: u32, direction: Direction) -> Option<u32> {
        nonzero(wire_id).or_else(|| {
            (direction == Dir::ClientToServer)
                .then_some(self.player_id)
                .flatten()
        })
    }

    fn start_cast(&mut self, mut cast: PendingCast) -> Vec<Event> {
        let key = cast.caster_id;
        let mut output = Vec::with_capacity(2);
        if let Some(previous) = self.combat.casts.remove(&key) {
            if previous.spell_id == cast.spell_id {
                cast.target_id = cast.target_id.or(previous.target_id);
                cast.cast_time_ms = cast.cast_time_ms.or(previous.cast_time_ms);
                cast.slot = cast.slot.or(previous.slot);
            } else {
                output.push(interrupted_event(
                    previous,
                    CastInterruptionReason::Superseded,
                ));
            }
        }
        self.combat.casts.insert(key, cast.clone());
        output.push(Event::SpellCastStarted {
            caster_id: cast.caster_id,
            target_id: cast.target_id,
            spell_id: cast.spell_id,
            cast_time_ms: cast.cast_time_ms,
            slot: cast.slot,
        });
        output
    }

    fn finish_matching_cast(&mut self, source_id: Option<u32>, spell_id: u32) {
        if self
            .combat
            .casts
            .get(&source_id)
            .is_some_and(|cast| cast.spell_id == spell_id)
        {
            self.combat.casts.remove(&source_id);
            return;
        }
        if source_id == self.player_id {
            let unknown_key = None;
            if self
                .combat
                .casts
                .get(&unknown_key)
                .is_some_and(|cast| cast.spell_id == spell_id)
            {
                self.combat.casts.remove(&unknown_key);
            }
        }
    }

    fn interrupt_player_cast(&mut self, reason: CastInterruptionReason) -> Vec<Event> {
        let mut keys = Vec::with_capacity(2);
        keys.push(self.player_id);
        if self.player_id.is_some() {
            keys.push(None);
        }
        keys.into_iter()
            .filter_map(|key| self.combat.casts.remove(&key))
            .map(|cast| interrupted_event(cast, reason))
            .collect()
    }

    fn apply_buff_snapshot(
        &mut self,
        owner: u32,
        entries: Vec<seq_events::BuffEntry>,
    ) -> Vec<Event> {
        let owner_id = self.buff_owner(owner);
        let mut incoming = BTreeMap::new();
        for entry in entries {
            let Some(spell_id) = valid_spell_id(entry.spell_id) else {
                continue;
            };
            let caster_name = (!entry.caster.is_empty()).then_some(entry.caster);
            let caster_id = caster_name
                .as_deref()
                .and_then(|name| self.resolve_caster_name(name));
            let buff = ActiveBuff {
                owner_id,
                spell_id,
                remaining_ticks: Some(entry.remaining_ticks),
                slot: Some(entry.slot),
                caster_id,
                caster_name,
            };
            incoming.insert(BuffKey::Slot(entry.slot), buff);
        }

        let old_keys: Vec<_> = self
            .combat
            .buffs
            .keys()
            .filter(|(known_owner, _)| *known_owner == owner_id)
            .cloned()
            .collect();
        let mut output = Vec::new();
        for key in old_keys {
            let (_, buff_key) = key;
            let remove = match (self.combat.buffs.get(&key), incoming.get(&buff_key)) {
                (Some(old), Some(new)) => old.spell_id != new.spell_id,
                (Some(_), None) => true,
                _ => false,
            };
            if remove {
                if let Some(old) = self.combat.buffs.remove(&key) {
                    output.push(buff_removed(old));
                }
            }
        }

        for (buff_key, buff) in incoming {
            let key = (owner_id, buff_key);
            match self.combat.buffs.get(&key) {
                Some(current) if current == &buff => {}
                Some(_) => {
                    self.combat.buffs.insert(key, buff.clone());
                    output.push(Event::BuffUpdated(buff));
                }
                None => {
                    self.combat.buffs.insert(key, buff.clone());
                    output.push(Event::BuffAdded(buff));
                }
            }
        }
        output
    }

    fn apply_buff_wire(
        &mut self,
        spawn_id: u32,
        spell_id: u32,
        form: u8,
        slot: u8,
        duration_ticks: u32,
    ) -> Vec<Event> {
        let Some(spell_id) = valid_spell_id(spell_id) else {
            return Vec::new();
        };
        let owner_id = self.buff_owner(spawn_id);
        if form == 0 {
            let keys: Vec<_> = self
                .combat
                .buffs
                .iter()
                .filter(|((owner, _), buff)| *owner == owner_id && buff.spell_id == spell_id)
                .map(|(key, _)| *key)
                .collect();
            return keys
                .into_iter()
                .filter_map(|key| self.combat.buffs.remove(&key))
                .map(buff_removed)
                .collect();
        }
        if !matches!(form, 1 | 2) {
            return Vec::new();
        }

        let explicit_slot = (form == 1 && slot != u8::MAX).then_some(u32::from(slot));
        let key = explicit_slot
            .map(BuffKey::Slot)
            .or_else(|| {
                self.combat
                    .buffs
                    .iter()
                    .find(|((owner, _), buff)| *owner == owner_id && buff.spell_id == spell_id)
                    .map(|((_, key), _)| *key)
            })
            .unwrap_or(BuffKey::Spell(spell_id));
        let old = self.combat.buffs.get(&(owner_id, key));
        let buff = ActiveBuff {
            owner_id,
            spell_id,
            remaining_ticks: (form == 2).then_some(u32_to_i32(duration_ticks)),
            slot: explicit_slot.or_else(|| old.and_then(|buff| buff.slot)),
            caster_id: old.and_then(|buff| buff.caster_id),
            caster_name: old.and_then(|buff| buff.caster_name.clone()),
        };
        match old {
            Some(current) if current == &buff => Vec::new(),
            Some(_) => {
                self.combat.buffs.insert((owner_id, key), buff.clone());
                vec![Event::BuffUpdated(buff)]
            }
            None => {
                self.combat.buffs.insert((owner_id, key), buff.clone());
                vec![Event::BuffAdded(buff)]
            }
        }
    }

    fn buff_owner(&self, wire_owner: u32) -> Option<u32> {
        if wire_owner == 0 || self.is_player_id(wire_owner) {
            self.player_id
        } else {
            Some(wire_owner)
        }
    }

    fn resolve_caster_name(&self, name: &str) -> Option<u32> {
        if name == self.player_name {
            self.player_id
        } else {
            self.entities.unique_id(name)
        }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_communication_semantics(&mut self, event: Event, direction: Direction) -> Vec<Event> {
        match event {
            Event::Chat {
                channel,
                from,
                target,
                text,
                chat_color,
                channel_name,
            } => {
                let compatibility = Event::Chat {
                    channel,
                    from: from.clone(),
                    target: target.clone(),
                    text: text.clone(),
                    chat_color,
                    channel_name: channel_name.clone(),
                };
                if direction == Dir::ClientToServer {
                    vec![compatibility]
                } else {
                    vec![
                        compatibility,
                        Event::ChatMessage(ChatMessage {
                            kind: ChatMessageKind::Common,
                            channel,
                            from,
                            target,
                            text: clean_links(&text),
                            chat_color,
                            channel_name,
                            format_id: None,
                            args: Vec::new(),
                        }),
                    ]
                }
            }
            Event::SimpleMessage { format_id, color } => vec![
                Event::SimpleMessage { format_id, color },
                Event::ChatMessage(ChatMessage {
                    kind: ChatMessageKind::Simple,
                    channel: chat_color_channel(color),
                    from: String::new(),
                    target: String::new(),
                    text: String::new(),
                    chat_color: color,
                    channel_name: String::new(),
                    format_id: Some(format_id),
                    args: Vec::new(),
                }),
            ],
            Event::FormattedMessage {
                format_id,
                color,
                args,
            } => {
                let channel = if self.backend == BackendId::Eql {
                    19
                } else {
                    chat_color_channel(color)
                };
                vec![
                    Event::FormattedMessage {
                        format_id,
                        color,
                        args: args.clone(),
                    },
                    Event::ChatMessage(ChatMessage {
                        kind: ChatMessageKind::Formatted,
                        channel,
                        from: String::new(),
                        target: String::new(),
                        text: String::new(),
                        chat_color: color,
                        channel_name: String::new(),
                        format_id: Some(format_id),
                        args,
                    }),
                ]
            }
            Event::SpecialMessage {
                color,
                target,
                source,
                message,
            } => {
                let target_name = self
                    .entities
                    .names_by_id
                    .get(&target)
                    .cloned()
                    .unwrap_or_default();
                vec![
                    Event::SpecialMessage {
                        color,
                        target,
                        source: source.clone(),
                        message: message.clone(),
                    },
                    Event::ChatMessage(ChatMessage {
                        kind: ChatMessageKind::Special,
                        channel: chat_color_channel(color),
                        from: source,
                        target: target_name,
                        text: clean_links(&message),
                        chat_color: color,
                        channel_name: String::new(),
                        format_id: None,
                        args: Vec::new(),
                    }),
                ]
            }
            Event::LootMessage {
                color,
                text,
                item_id,
                item_name,
            } => {
                let compatibility = Event::LootMessage {
                    color,
                    text: text.clone(),
                    item_id,
                    item_name,
                };
                if text.is_empty() {
                    vec![compatibility]
                } else {
                    vec![
                        compatibility,
                        Event::ChatMessage(ChatMessage {
                            kind: ChatMessageKind::Loot,
                            channel: 19,
                            from: String::new(),
                            target: String::new(),
                            text,
                            chat_color: color,
                            channel_name: String::new(),
                            format_id: None,
                            args: Vec::new(),
                        }),
                    ]
                }
            }
            Event::GroupRosterWire {
                group_id,
                member_count,
                names,
                complete,
            } => {
                let compatibility = Event::GroupRosterWire {
                    group_id,
                    member_count,
                    names: names.clone(),
                    complete,
                };
                self.apply_group_roster(group_id, &names, complete);
                vec![compatibility, Event::GroupRosterUpdated(self.group_state())]
            }
            Event::GroupFollow { name, level } => {
                let compatibility = Event::GroupFollow {
                    name: name.clone(),
                    level,
                };
                if self.add_group_member(name, (level != 0).then_some(level)) {
                    vec![compatibility, Event::GroupRosterUpdated(self.group_state())]
                } else {
                    vec![compatibility]
                }
            }
            Event::GroupDisband {
                yourname,
                membername,
            } => {
                let compatibility = Event::GroupDisband {
                    yourname,
                    membername: membername.clone(),
                };
                if membername == self.player_name {
                    self.communication.group_slots.fill(None);
                    self.communication.group_complete = true;
                } else {
                    for slot in &mut self.communication.group_slots {
                        if slot
                            .as_ref()
                            .is_some_and(|member| member.name == membername)
                        {
                            *slot = None;
                        }
                    }
                }
                vec![compatibility, Event::GroupRosterUpdated(self.group_state())]
            }
            Event::GuildRoster { guild_id, members } => {
                vec![Event::GuildRoster { guild_id, members }]
            }
            Event::GuildRosterWire {
                guild_id,
                members,
                complete,
            } => {
                let compatibility = Event::GuildRosterWire {
                    guild_id,
                    members: members.clone(),
                    complete,
                };
                if self.communication.guild_id != guild_id {
                    self.communication.guild_rank_names.clear();
                    self.communication.guild_members.clear();
                    self.communication.guild_complete = false;
                }
                self.communication.guild_id = guild_id;
                if complete {
                    self.communication.guild_members = members;
                    self.communication.guild_complete = true;
                } else {
                    for member in members {
                        if let Some(current) = self
                            .communication
                            .guild_members
                            .iter_mut()
                            .find(|current| current.name == member.name)
                        {
                            *current = member;
                        } else {
                            self.communication.guild_members.push(member);
                        }
                    }
                }
                for member in &mut self.communication.guild_members {
                    if let Some(status) = self.communication.guild_status.get(&member.name) {
                        member.zone_id = status.zone_id;
                        member.last_on = status.last_on;
                    }
                }
                vec![compatibility, Event::GuildRosterUpdated(self.guild_state())]
            }
            Event::GuildMemberStatus {
                name,
                zone_id,
                instance_id,
                last_on,
            } => {
                let compatibility = Event::GuildMemberStatus {
                    name: name.clone(),
                    zone_id,
                    instance_id,
                    last_on,
                };
                self.communication
                    .guild_status
                    .insert(name.clone(), GuildMemberStatus { zone_id, last_on });
                if let Some(member) = self
                    .communication
                    .guild_members
                    .iter_mut()
                    .find(|member| member.name == name)
                {
                    member.zone_id = zone_id;
                    member.last_on = last_on;
                    vec![compatibility, Event::GuildRosterUpdated(self.guild_state())]
                } else {
                    self.communication.guild_complete = false;
                    vec![compatibility]
                }
            }
            Event::GuildMotd { message, sender } => vec![
                Event::GuildMotd {
                    message: message.clone(),
                    sender: sender.clone(),
                },
                Event::GuildMotdUpdated(GuildMotdState {
                    guild_id: self.communication.guild_id,
                    message,
                    sender,
                }),
            ],
            Event::GuildRankName {
                guild_id,
                rank_index,
                rank_name,
            } => {
                let compatibility = Event::GuildRankName {
                    guild_id,
                    rank_index,
                    rank_name: rank_name.clone(),
                };
                if guild_id != 0 && self.communication.guild_id != guild_id {
                    self.communication.guild_id = guild_id;
                    self.communication.guild_rank_names.clear();
                }
                self.communication
                    .guild_rank_names
                    .insert(rank_index, rank_name);
                vec![
                    compatibility,
                    Event::GuildRankNamesUpdated(self.guild_ranks()),
                ]
            }
            Event::DynamicZoneInfo {
                active,
                max_players,
                expedition_name,
                leader_name,
            } => {
                let compatibility = Event::DynamicZoneInfo {
                    active,
                    max_players,
                    expedition_name: expedition_name.clone(),
                    leader_name: leader_name.clone(),
                };
                self.apply_dynamic_zone_info(active, max_players, expedition_name, leader_name);
                vec![
                    compatibility,
                    Event::DynamicZoneUpdated(self.dynamic_zone_state()),
                ]
            }
            Event::DynamicZoneSwitch {
                active,
                zone_id,
                instance_id,
                kind,
                position,
            } => {
                let compatibility = Event::DynamicZoneSwitch {
                    active,
                    zone_id,
                    instance_id,
                    kind,
                    position,
                };
                self.apply_dynamic_zone_switch(active, zone_id, instance_id, kind, position);
                vec![
                    compatibility,
                    Event::DynamicZoneUpdated(self.dynamic_zone_state()),
                ]
            }
            other => vec![other],
        }
    }

    fn apply_group_roster(&mut self, group_id: u32, names: &[String], complete: bool) {
        let mut incoming = Vec::new();
        let mut seen = BTreeSet::new();
        for name in names {
            if name.is_empty() || name == &self.player_name || !seen.insert(name.clone()) {
                continue;
            }
            incoming.push(name.clone());
        }
        if complete {
            for slot in &mut self.communication.group_slots {
                if slot
                    .as_ref()
                    .is_some_and(|member| !incoming.contains(&member.name))
                {
                    *slot = None;
                }
            }
        }
        for name in incoming {
            self.add_group_member(name, None);
        }
        self.communication.group_id = nonzero(group_id);
        self.communication.group_complete = complete && !self.player_name.is_empty();
    }

    fn add_group_member(&mut self, name: String, level: Option<u32>) -> bool {
        if name.is_empty() || name == self.player_name {
            return false;
        }
        if let Some(member) = self
            .communication
            .group_slots
            .iter_mut()
            .flatten()
            .find(|member| member.name == name)
        {
            if level.is_some() && member.level != level {
                member.level = level;
                return true;
            }
            return false;
        }
        let Some((slot, target)) = self
            .communication
            .group_slots
            .iter_mut()
            .enumerate()
            .find(|(_, member)| member.is_none())
        else {
            return false;
        };
        *target = Some(GroupMember {
            slot: slot as u8,
            name,
            level,
        });
        true
    }

    fn group_state(&self) -> GroupRosterState {
        GroupRosterState {
            group_id: self.communication.group_id,
            members: self
                .communication
                .group_slots
                .iter()
                .flatten()
                .cloned()
                .collect(),
            complete: self.communication.group_complete,
        }
    }

    fn guild_state(&self) -> GuildRosterState {
        GuildRosterState {
            guild_id: self.communication.guild_id,
            members: self.communication.guild_members.clone(),
            complete: self.communication.guild_complete,
        }
    }

    fn guild_ranks(&self) -> GuildRankNamesState {
        GuildRankNamesState {
            guild_id: self.communication.guild_id,
            ranks: self
                .communication
                .guild_rank_names
                .iter()
                .map(|(&rank_index, rank_name)| GuildRankNameEntry {
                    rank_index,
                    rank_name: rank_name.clone(),
                })
                .collect(),
        }
    }

    fn apply_dynamic_zone_info(
        &mut self,
        active: bool,
        max_players: u32,
        expedition_name: String,
        leader_name: String,
    ) {
        if !active {
            self.communication.dynamic_zone = DynamicZoneCorrelation {
                state: Some(inactive_dynamic_zone()),
                saw_info: true,
                saw_switch: true,
            };
            return;
        }
        let state = self
            .communication
            .dynamic_zone
            .state
            .get_or_insert_with(empty_active_dynamic_zone);
        state.active = true;
        state.max_players = Some(max_players);
        state.expedition_name = expedition_name;
        state.leader_name = leader_name;
        self.communication.dynamic_zone.saw_info = true;
        state.complete = self.communication.dynamic_zone.saw_switch;
    }

    fn apply_dynamic_zone_switch(
        &mut self,
        active: bool,
        zone_id: Option<u16>,
        instance_id: Option<u16>,
        kind: Option<u32>,
        position: Option<seq_events::Point3>,
    ) {
        if !active {
            self.communication.dynamic_zone = DynamicZoneCorrelation {
                state: Some(inactive_dynamic_zone()),
                saw_info: true,
                saw_switch: true,
            };
            return;
        }
        let state = self
            .communication
            .dynamic_zone
            .state
            .get_or_insert_with(empty_active_dynamic_zone);
        state.active = true;
        state.zone_id = zone_id;
        state.instance_id = instance_id;
        state.kind = kind;
        state.position = position;
        self.communication.dynamic_zone.saw_switch = true;
        state.complete = self.communication.dynamic_zone.saw_info;
    }

    fn dynamic_zone_state(&self) -> DynamicZoneState {
        self.communication
            .dynamic_zone
            .state
            .clone()
            .unwrap_or_else(inactive_dynamic_zone)
    }

    #[cfg(feature = "backend-eql")]
    fn ucs_chat_message(
        &mut self,
        channel_first: u8,
        channel_rest: &str,
        channel_run: &str,
        sender: String,
        message: String,
        spam: bool,
    ) -> ChatMessage {
        if channel_rest == "eneral" {
            self.communication.ucs_mask = Some(channel_first ^ b'G');
        }
        let channel_name = resolve_ucs_channel(
            channel_first,
            channel_rest,
            channel_run,
            self.communication.ucs_mask,
            &mut self.communication.ucs_known_channels,
        );
        ChatMessage {
            kind: ChatMessageKind::Ucs,
            channel: 19,
            from: sender,
            target: String::new(),
            text: if spam {
                format!("(SPAM) {message}")
            } else {
                message
            },
            chat_color: 0,
            channel_name,
            format_id: None,
            args: Vec::new(),
        }
    }

    fn reset_combat(&mut self, reason: CastInterruptionReason) -> Vec<Event> {
        let mut events: Vec<_> = std::mem::take(&mut self.combat.casts)
            .into_values()
            .map(|cast| interrupted_event(cast, reason))
            .collect();
        events.extend(
            std::mem::take(&mut self.combat.buffs)
                .into_values()
                .map(buff_removed),
        );
        events
    }

    fn reset_correlations(&mut self, cast_reason: CastInterruptionReason) -> Vec<Event> {
        let mut events = self.reset_combat(cast_reason);
        events.extend(self.reset_communication());
        self.entities.clear();
        self.player_id = None;
        self.update_identity_id();
        self.progression = ProgressionState::default();
        let decoder_events = match &mut self.decoder {
            #[cfg(feature = "backend-eql")]
            BackendSession::Eql(state) => {
                let rows = state.loot_tracker.flush();
                state.loot_tracker.reset();
                state.self_tracker.reset();
                state.self_stats.clear();
                state.finish_loot_rows(rows)
            }
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            BackendSession::Live(_) | BackendSession::Test(_) => Vec::new(),
        };
        events.extend(decoder_events);
        events
    }

    fn reset_communication(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        if self.communication.group_id.is_some()
            || self.communication.group_slots.iter().any(Option::is_some)
        {
            events.push(Event::GroupRosterUpdated(GroupRosterState {
                group_id: None,
                members: Vec::new(),
                complete: false,
            }));
        }
        if self.communication.guild_id != 0 || !self.communication.guild_members.is_empty() {
            events.push(Event::GuildRosterUpdated(GuildRosterState {
                guild_id: 0,
                members: Vec::new(),
                complete: false,
            }));
        }
        if self
            .communication
            .dynamic_zone
            .state
            .as_ref()
            .is_some_and(|state| state.active)
        {
            events.push(Event::DynamicZoneUpdated(inactive_dynamic_zone()));
        }
        self.communication = CommunicationState {
            group_slots: vec![None; MAX_GROUP_PEERS],
            ..CommunicationState::default()
        };
        events
    }
}

const fn empty_active_dynamic_zone() -> DynamicZoneState {
    DynamicZoneState {
        active: true,
        zone_id: None,
        instance_id: None,
        kind: None,
        position: None,
        max_players: None,
        expedition_name: String::new(),
        leader_name: String::new(),
        complete: false,
    }
}

const fn inactive_dynamic_zone() -> DynamicZoneState {
    DynamicZoneState {
        active: false,
        zone_id: None,
        instance_id: None,
        kind: None,
        position: None,
        max_players: None,
        expedition_name: String::new(),
        leader_name: String::new(),
        complete: true,
    }
}

fn chat_color_channel(color: u32) -> u32 {
    match color {
        256 | 307 => 8,
        257 | 308 => 7,
        258 | 309 => 2,
        259 | 310 => 0,
        260 | 311 => 5,
        261 | 312 => 4,
        262 | 313 => 3,
        263 | 314 => 24,
        327 => 15,
        264 | 284 | 288 | 289 | 302 => 26,
        285 => 22,
        287 => 23,
        _ => 19,
    }
}

fn clean_links(text: &str) -> String {
    const ITEM_LINK_HEX: usize = 197;
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('\u{12}') {
        output.push_str(&rest[..open]);
        let body = &rest[open + 1..];
        let Some(close) = body.find('\u{12}') else {
            output.push_str(body);
            return output;
        };
        let link = &body[..close];
        if let Some(caret) = link.rfind('^') {
            output.push_str(link[caret + 1..].trim_start_matches('\''));
        } else {
            output.push_str(link.get(ITEM_LINK_HEX..).unwrap_or_default());
        }
        rest = &body[close + 1..];
    }
    output.push_str(rest);
    output
}

#[cfg(feature = "backend-eql")]
fn resolve_ucs_channel(
    first: u8,
    rest: &str,
    run: &str,
    mask: Option<u8>,
    known: &mut BTreeSet<String>,
) -> String {
    if let Some(name) = known
        .iter()
        .filter_map(|name| {
            let suffix = shared_suffix_len(run.as_bytes(), name.as_bytes());
            (suffix >= 5).then_some((suffix, name))
        })
        .max_by_key(|(suffix, _)| *suffix)
        .map(|(_, name)| name.clone())
    {
        return name;
    }

    let dominant = run == rest || (run.len() == rest.len() + 1 && run.get(1..) == Some(rest));
    if !dominant {
        return run.to_owned();
    }
    let name = match mask.map(|mask| first ^ mask) {
        Some(repaired) if repaired.is_ascii_graphic() => {
            format!("{}{rest}", char::from(repaired))
        }
        _ => rest.to_owned(),
    };
    if name.len() >= 2 && name.as_bytes()[0].is_ascii_uppercase() {
        known.insert(name.clone());
    }
    name
}

#[cfg(feature = "backend-eql")]
fn shared_suffix_len(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
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

fn valid_spell_id(spell_id: u32) -> Option<u32> {
    (!matches!(spell_id, 0 | 0xffff | u32::MAX)).then_some(spell_id)
}

fn interrupts_cast(format_id: u32) -> bool {
    matches!(
        format_id,
        191 | 239
            | 240
            | 242
            | 243
            | 244
            | 245
            | 248
            | 251
            | 253
            | 255
            | 263
            | 264
            | 268
            | 269
            | 271
            | 272
            | 439
            | 3_285
            | 9_035
            | 9_036
    )
}

fn interrupted_event(cast: PendingCast, reason: CastInterruptionReason) -> Event {
    Event::SpellCastInterrupted {
        caster_id: cast.caster_id,
        target_id: cast.target_id,
        spell_id: cast.spell_id,
        reason,
    }
}

fn buff_removed(buff: ActiveBuff) -> Event {
    Event::BuffRemoved {
        owner_id: buff.owner_id,
        spell_id: buff.spell_id,
        slot: buff.slot,
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
    ) -> Vec<Event> {
        match event {
            Event::PlayerProfile(profile) => {
                self.player_name.clone_from(&profile.name);
                self.loot_tracker.set_looter(&profile.name);
                Vec::new()
            }
            Event::ZoneChanged(zone) => {
                let rows = self.loot_tracker.set_zone(&zone.short_name);
                self.finish_loot_rows(rows)
            }
            Event::LootMessage {
                color,
                text,
                item_id,
                item_name,
            } => {
                let rows = self
                    .loot_tracker
                    .on_loot_message(*color, text, *item_id, item_name, timestamp);
                self.finish_loot_rows(rows)
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
                let rows = self.loot_tracker.on_loot_transaction(
                    *corpse_id,
                    *item_id,
                    *quantity,
                    *coin_copper,
                    *from_corpse,
                    sequence,
                    timestamp,
                );
                self.finish_loot_rows(rows)
            }
            Event::LootDrops {
                corpse_id,
                corpse_name,
                items,
            } => {
                let emit_snapshot =
                    self.loot_tracker
                        .observe_window_snapshot(*corpse_id, corpse_name, items);
                let mut rows = Vec::new();
                for item in items {
                    let item_rows = self.loot_tracker.on_loot_drop_item(
                        *corpse_id,
                        corpse_name,
                        &item.name,
                        item.icon,
                        item.item_id,
                        timestamp,
                    );
                    if !item_rows.is_empty() {
                        rows.extend(item_rows);
                    }
                }
                self.loot_rows.extend(rows);
                if !emit_snapshot {
                    Vec::new()
                } else {
                    let (zone_base, instance) =
                        seq_backend_eql::loot_track::split_zone_instance(self.loot_tracker.zone());
                    vec![Event::CorpseLootSnapshot(Box::new(CorpseLootSnapshot {
                        timestamp,
                        corpse_id: *corpse_id,
                        corpse_name: corpse_name.clone(),
                        corpse_name_normalized: seq_backend_eql::loot_track::normalize_mob(
                            corpse_name,
                        ),
                        zone_short: self.loot_tracker.zone().to_owned(),
                        zone_base,
                        instance,
                        looter: self.player_name.clone(),
                        items: items.clone(),
                    }))]
                }
            }
            Event::EnterWorld { character_name } if direction == Dir::ClientToServer => {
                self.player_name.clone_from(character_name);
                self.loot_tracker.set_looter(character_name);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn finish_loot_rows(&mut self, rows: Vec<LootRow>) -> Vec<Event> {
        let events = rows
            .iter()
            .filter(|row| row.source != seq_backend_eql::loot_track::LootSource::Window)
            .cloned()
            .map(loot_acquisition)
            .map(|acquisition| Event::LootAcquired(Box::new(acquisition)))
            .collect();
        self.loot_rows.extend(rows);
        events
    }
}

#[cfg(feature = "backend-eql")]
fn loot_acquisition(row: LootRow) -> LootAcquisition {
    LootAcquisition {
        timestamp: row.ts,
        item_name: row.item_name,
        item_id: nonzero(row.item_id),
        quantity: row.qty,
        corpse_name: row.mob_name,
        corpse_name_normalized: row.mob_norm,
        corpse_id: nonzero(row.corpse_id),
        zone_short: row.zone_short,
        zone_base: row.zone_base,
        instance: row.instance,
        sold: row.sold,
        coin_copper: row.money_copper,
        disposition: row.disposition,
        looter: row.looter,
        sequence: nonzero(row.sequence),
        from_corpse: row.source == seq_backend_eql::loot_track::LootSource::Coin,
        complete: row.complete,
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
            OpcodeId(0x1c18),
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
            OpcodeId(0x1c18),
            Dir::ClientToServer,
            &[],
        );
        assert_eq!(unmapped.disposition, DecodeDisposition::Unmapped);

        let malformed =
            session.decode(StreamKind::Zone, OpcodeId(0x1c18), Dir::ClientToServer, &[]);
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
            OpcodeId(0x1c18),
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
            OpcodeId(0x1c18),
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
            OpcodeId(0x514a),
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
            OpcodeId(0x1c18),
            Dir::ClientToServer,
            &self_pos(99),
        );
        let decoded = session.decode(
            StreamKind::Zone,
            OpcodeId(0x7560),
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
            OpcodeId(0x1c18),
            Dir::ClientToServer,
            &self_pos(101),
        );
        second.decode(
            StreamKind::Zone,
            OpcodeId(0x1c18),
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
            OpcodeId(0x1c18),
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
        let events = session.flush(FlushReason::ReplayEnd);
        assert!(matches!(
            events.as_slice(),
            [Event::LootAcquired(acquisition)]
                if acquisition.timestamp == 123 && !acquisition.complete
        ));
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
