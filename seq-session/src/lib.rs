//! Stateful packet decoding keyed by backend, stream, and numeric opcode.
//!
//! Existing name-based backend decoders remain public for compatibility. New
//! hosts should keep one `Session` per ordered application-packet stream.

use seq_backend_eql::{backend::EqlBackend, LootTracker, SelfTracker};
use seq_backend_live::LiveBackend;
use seq_events::{Backend, Decoded};
use std::sync::Arc;

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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Live(LiveBackend),
    Test(LiveBackend),
    Eql(Box<EqlSession>),
}

struct EqlSession {
    decoder: EqlBackend,
    self_tracker: SelfTracker,
    loot_tracker: LootTracker,
    player_name: String,
    self_stats: Vec<SelfStat>,
    loot_rows: Vec<LootRow>,
}

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
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        let decoder = match config.backend {
            BackendId::Live => BackendSession::Live(LiveBackend),
            // Test currently shares Live's wire decoder. Its opcode catalog is
            // still independent, so a patch-day ID rotation cannot cross over.
            BackendId::Test => BackendSession::Test(LiveBackend),
            BackendId::Eql => BackendSession::Eql(Box::default()),
        };
        Self {
            backend: config.backend,
            protocol_registry: config.protocol_registry,
            decoder,
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
            BackendSession::Live(backend) | BackendSession::Test(backend) => {
                backend.decode(opcode_name, direction, payload)
            }
            BackendSession::Eql(state) => {
                let decoded = state.decoder.decode(opcode_name, direction, payload);
                state.observe(&decoded, opcode_name, direction, payload, timestamp);
                decoded
            }
        };
        batch(generation, decoded)
    }

    /// Close stateful correlators at a lifecycle boundary.
    ///
    /// Phase 2 still emits the existing low-level `Event` variants, so EQL loot
    /// rows are drained separately with `take_loot_rows` during shadow parity.
    pub fn flush(&mut self, _reason: FlushReason) -> Vec<Event> {
        if let BackendSession::Eql(state) = &mut self.decoder {
            state.loot_rows.extend(state.loot_tracker.flush());
            state.loot_tracker.reset();
            state.self_tracker.reset();
        }
        Vec::new()
    }

    /// Current EQL self-correlation state. Non-EQL sessions return zeros.
    pub fn self_identity(&self) -> SelfIdentity {
        match &self.decoder {
            BackendSession::Eql(state) => SelfIdentity {
                self_id: state.self_tracker.self_id(),
                alt_id: state.self_tracker.alt_id(),
                provisional_id: state.self_tracker.provisional_id(),
            },
            BackendSession::Live(_) | BackendSession::Test(_) => SelfIdentity::default(),
        }
    }

    /// Drain EQL self-vitals attributed by the session tracker.
    pub fn take_self_stats(&mut self) -> Vec<SelfStat> {
        match &mut self.decoder {
            BackendSession::Eql(state) => std::mem::take(&mut state.self_stats),
            BackendSession::Live(_) | BackendSession::Test(_) => Vec::new(),
        }
    }

    /// Drain EQL loot rows completed by the session tracker.
    pub fn take_loot_rows(&mut self) -> Vec<LootRow> {
        match &mut self.decoder {
            BackendSession::Eql(state) => std::mem::take(&mut state.loot_rows),
            BackendSession::Live(_) | BackendSession::Test(_) => Vec::new(),
        }
    }
}

impl EqlSession {
    fn observe(
        &mut self,
        decoded: &Decoded,
        opcode_name: &str,
        direction: Direction,
        payload: &[u8],
        timestamp: i64,
    ) {
        match decoded {
            Decoded::One(event) => {
                self.observe_event(event, opcode_name, direction, payload, timestamp)
            }
            Decoded::Many(events) => {
                for event in events {
                    self.observe_event(event, opcode_name, direction, payload, timestamp);
                }
            }
            Decoded::Ignored | Decoded::Unhandled | Decoded::Malformed => {}
        }
    }

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
                self.self_tracker.reset();
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
            Event::EnterWorld if direction == Dir::ClientToServer => {
                self.loot_rows.extend(self.loot_tracker.flush());
                self.loot_tracker.reset();
                self.self_tracker.reset();
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

#[cfg(test)]
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
            &[],
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
}
