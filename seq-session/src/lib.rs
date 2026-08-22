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
use seq_events::{Backend, Decoded, SessionResetReason};
use std::{
    collections::{BTreeSet, HashMap},
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
        for mut event in events {
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

            self.apply_entity_semantics(&mut event);

            self.decoder
                .observe_event(&event, opcode_name, direction, payload, timestamp);
            output.push(event);
        }
        Decoded::Many(output)
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

    fn reset_correlations(&mut self) {
        self.entities.clear();
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
            Event::SpawnAdded(spawn) => {
                self.self_tracker
                    .observe_spawn(&self.player_name, &spawn.name, spawn.id);
                let pending = self.self_tracker.take_pending_vitals();
                if pending.any() {
                    self.self_stats.push(pending);
                }
            }
            Event::SelfPos { spawn_id, .. } if direction == Dir::ClientToServer => {
                self.self_tracker.observe_self_pos(*spawn_id);
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
            } => {
                let stat = seq_backend_eql::StatSync {
                    spawn_id: *spawn_id,
                    wide: *wide,
                    has_hp: *has_hp,
                    hp_cur: i64::from(*hp_cur),
                    hp_max: i64::from(*hp_max),
                    has_mana: *has_mana,
                    mana_cur: i64::from(*mana_cur),
                    mana_max: i64::from(*mana_max),
                    has_end: *has_end,
                    end_cur: i64::from(*end_cur),
                    end_max: i64::from(*end_max),
                };
                let routed = self.self_tracker.observe_stat_sync(&stat);
                if routed.is_self && routed.any() {
                    self.self_stats.push(routed);
                }
            }
            Event::SpawnKilled { deceased_id, .. } if self.self_tracker.is_self(*deceased_id) => {
                self.self_tracker.reset();
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
            [Event::SelfPos { spawn_id: 77, .. }]
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
                Event::PlayerProfile(profile),
            ])
        );
        assert_eq!(session.self_identity(), SelfIdentity::default());
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
}
