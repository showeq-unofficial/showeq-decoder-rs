use seq_session::{
    BackendId, DecodeDisposition, Dir, Event, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

fn registry(backend: BackendId) -> Arc<ProtocolRegistry> {
    let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
    registry
        .replace_from_str(
            backend,
            "[[world]]\nid='0001'\nname='OP_EnterWorld'\n\n[[world]]\nid='0002'\nname='OP_ZoneServerInfo'\n\n[[zone]]\nid='0003'\nname='OP_ZoneChange'\n\n[[zone]]\nid='0004'\nname='OP_NewZone'\n\n[[zone]]\nid='0005'\nname='OP_TimeOfDay'\n",
        )
        .unwrap();
    registry
}

fn session(backend: BackendId) -> Session {
    Session::new(SessionConfig {
        backend,
        protocol_registry: registry(backend),
    })
    .expect("backend linked")
}

fn enter_world(name: &str) -> [u8; 72] {
    let mut payload = [0; 72];
    payload[..name.len()].copy_from_slice(name.as_bytes());
    payload
}

fn zone_server(host: &str, port: u16) -> [u8; 130] {
    let mut payload = [0; 130];
    payload[..host.len()].copy_from_slice(host.as_bytes());
    payload[128..].copy_from_slice(&port.to_le_bytes());
    payload
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

fn assert_common_lifecycle(backend: BackendId) {
    let mut session = session(backend);

    let malformed = session.decode(StreamKind::World, OpcodeId(1), Dir::ClientToServer, &[]);
    assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
    assert!(malformed.events.is_empty());

    let entered = session.decode(
        StreamKind::World,
        OpcodeId(1),
        Dir::ClientToServer,
        &enter_world("Testchar"),
    );
    assert_eq!(
        entered.events,
        vec![
            Event::SessionReset {
                reason: seq_events::SessionResetReason::EnterWorld,
            },
            Event::EnterWorld {
                character_name: "Testchar".into(),
            },
        ]
    );

    let handoff = session.decode(
        StreamKind::World,
        OpcodeId(2),
        Dir::ServerToClient,
        &zone_server("zone.example.test", 9000),
    );
    assert_eq!(
        handoff.events,
        vec![Event::ZoneServerInfo {
            host: "zone.example.test".into(),
            port: 9000,
        }]
    );

    let zone = session.decode(
        StreamKind::Zone,
        OpcodeId(4),
        Dir::ServerToClient,
        &new_zone(),
    );
    assert_eq!(
        zone.events,
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

    let clock = session.decode(
        StreamKind::Zone,
        OpcodeId(5),
        Dir::ServerToClient,
        &[13, 42, 27, 11, 0xcd, 0x0e, 0, 0],
    );
    assert_eq!(
        clock.events,
        vec![Event::TimeOfDay {
            year: 3789,
            month: 11,
            day: 27,
            hour: 13,
            minute: 42,
        }]
    );
}

#[cfg(feature = "backend-live")]
#[test]
fn live_lifecycle_trace_has_exact_ordered_batches() {
    assert_common_lifecycle(BackendId::Live);
    let mut session = session(BackendId::Live);
    let mut transition = [0; 100];
    transition[..9].copy_from_slice(b"Testchar\0");
    transition[64..66].copy_from_slice(&57u16.to_le_bytes());
    transition[66..68].copy_from_slice(&3u16.to_le_bytes());
    let batch = session.decode(
        StreamKind::Zone,
        OpcodeId(3),
        Dir::ServerToClient,
        &transition,
    );
    assert_eq!(
        batch.events,
        vec![
            Event::SessionReset {
                reason: seq_events::SessionResetReason::ZoneTransition,
            },
            Event::ZoneTransition {
                character_name: "Testchar".into(),
                zone_id: Some(57),
                instance_id: Some(3),
                confirmed: true,
            },
        ]
    );
}

#[cfg(feature = "backend-test")]
#[test]
fn test_backend_lifecycle_trace_has_exact_ordered_batches() {
    assert_common_lifecycle(BackendId::Test);
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_lifecycle_trace_has_exact_ordered_batches() {
    assert_common_lifecycle(BackendId::Eql);
    let mut session = session(BackendId::Eql);
    let batch = session.decode(
        StreamKind::Zone,
        OpcodeId(3),
        Dir::ClientToServer,
        &[0; 484],
    );
    assert_eq!(
        batch.events,
        vec![Event::ZoneTransition {
            character_name: String::new(),
            zone_id: None,
            instance_id: None,
            confirmed: false,
        }]
    );
}
