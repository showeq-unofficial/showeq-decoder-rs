#[cfg(any(feature = "backend-live", feature = "backend-test"))]
use seq_events::Velocity;
use seq_events::{Event, PlayerAppearance, PlayerVitals, Pos, VitalValue};
use seq_session::{
    BackendId, DecodeDisposition, Dir, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

const ENTER_WORLD: u16 = 1;
const ZONE_ENTRY: u16 = 2;
const CLIENT_UPDATE: u16 = 3;
const HP_UPDATE: u16 = 4;
const DEATH: u16 = 5;
const ILLUSION: u16 = 6;
#[cfg(feature = "backend-eql")]
const APPEARANCE: u16 = 7;
const MANA_CHANGE: u16 = 8;

fn session(backend: BackendId) -> Session {
    let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
    registry
        .replace_from_str(
            backend,
            "[[world]]\nid='0001'\nname='OP_EnterWorld'\n\
             [[zone]]\nid='0002'\nname='OP_ZoneEntry'\n\
             [[zone]]\nid='0003'\nname='OP_ClientUpdate'\n\
             [[zone]]\nid='0004'\nname='OP_HPUpdate'\n\
             [[zone]]\nid='0005'\nname='OP_Death'\n\
             [[zone]]\nid='0006'\nname='OP_Illusion'\n\
             [[zone]]\nid='0007'\nname='OP_SpawnAppearance'\n\
             [[zone]]\nid='0008'\nname='OP_ManaChange'\n",
        )
        .unwrap();
    Session::new(SessionConfig {
        backend,
        protocol_registry: registry,
    })
}

fn enter_world(name: &str) -> [u8; 72] {
    let mut payload = [0; 72];
    payload[..name.len()].copy_from_slice(name.as_bytes());
    payload
}

fn decode(
    session: &mut Session,
    stream: StreamKind,
    opcode: u16,
    direction: Dir,
    payload: &[u8],
) -> Vec<Event> {
    let batch = session.decode(stream, OpcodeId(opcode), direction, payload);
    assert_eq!(batch.disposition, DecodeDisposition::Decoded);
    batch.events
}

fn death(deceased: u32, killer: u32) -> [u8; 40] {
    let mut payload = [0; 40];
    payload[..4].copy_from_slice(&deceased.to_le_bytes());
    payload[4..8].copy_from_slice(&killer.to_le_bytes());
    payload
}

fn illusion(id: u32, race: u32, gender: u8) -> Vec<u8> {
    let mut payload = vec![0; 332];
    payload[..4].copy_from_slice(&id.to_le_bytes());
    payload[68..72].copy_from_slice(&race.to_le_bytes());
    payload[72] = gender;
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_spawn(name: &str, id: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(name.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.push(40);
    bytes.extend_from_slice(&[0; 16]);
    bytes.push(1);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[0; 8]);
    bytes.push(0);
    bytes.push(95);
    bytes.extend_from_slice(&[0; 35]);
    bytes.extend_from_slice(&50u32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 49]);
    bytes.extend_from_slice(&[0; 60]);
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&[0; 8]);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 66]);
    bytes
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_self_pos(id: u16, x: f32, y: f32, z: f32) -> [u8; 42] {
    let mut payload = [0; 42];
    payload[2..4].copy_from_slice(&id.to_le_bytes());
    payload[10..14].copy_from_slice(&y.to_le_bytes());
    payload[18..22].copy_from_slice(&z.to_le_bytes());
    payload[30..34].copy_from_slice(&x.to_le_bytes());
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_controlled_spawn_pos(id: u16) -> [u8; 42] {
    let mut payload = live_self_pos(id, 10.0, 20.0, 30.0);
    payload[6..10].copy_from_slice(&12u32.to_le_bytes());
    payload[14..18].copy_from_slice(&4.75f32.to_le_bytes());
    payload[22..26].copy_from_slice(&(0x3ffu32 - 6).to_le_bytes());
    payload[26..30].copy_from_slice(&9.5f32.to_le_bytes());
    payload[34..38].copy_from_slice(&(-5.25f32).to_le_bytes());
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_hp(id: u16, current: i32, maximum: i32) -> [u8; 18] {
    let mut payload = [0; 18];
    payload[..2].copy_from_slice(&id.to_le_bytes());
    payload[2..6].copy_from_slice(&current.to_le_bytes());
    payload[10..14].copy_from_slice(&maximum.to_le_bytes());
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn assert_live_trace(backend: BackendId) {
    let mut session = session(backend);
    decode(
        &mut session,
        StreamKind::World,
        ENTER_WORLD,
        Dir::ClientToServer,
        &enter_world("Firona"),
    );

    let identity = decode(
        &mut session,
        StreamKind::Zone,
        ZONE_ENTRY,
        Dir::ServerToClient,
        &live_spawn("Firona", 100),
    );
    assert!(matches!(
        identity.as_slice(),
        [Event::PlayerIdentityUpdated(i), Event::PlayerMoved { spawn_id: Some(100), pos }]
            if i.spawn_id == Some(100) && *pos == Pos {
                x: 0,
                y: 0,
                z: 0,
                heading_deg: 0,
            }
    ));

    assert!(matches!(
        decode(
            &mut session,
            StreamKind::Zone,
            ZONE_ENTRY,
            Dir::ServerToClient,
            &live_spawn("a rat", 200),
        )
        .as_slice(),
        [Event::SpawnAdded(spawn)] if spawn.id == 200
    ));

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            CLIENT_UPDATE,
            Dir::ServerToClient,
            &live_controlled_spawn_pos(200),
        ),
        vec![Event::SpawnMoved {
            id: 200,
            pos: Pos {
                x: 10,
                y: 20,
                z: 30,
                heading_deg: 0,
            },
            velocity: Velocity {
                x: Some(4),
                y: Some(-5),
                z: Some(9),
            },
            delta_heading: Some(-7),
            animation: Some(12),
        }]
    );

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            CLIENT_UPDATE,
            Dir::ClientToServer,
            &live_self_pos(100, 10.0, 20.0, 30.0),
        ),
        vec![Event::PlayerMoved {
            spawn_id: Some(100),
            pos: Pos {
                x: 10,
                y: 20,
                z: 30,
                heading_deg: 0,
            },
        }]
    );

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            HP_UPDATE,
            Dir::ServerToClient,
            &live_hp(100, 40, 50),
        ),
        vec![Event::PlayerVitalsUpdated(PlayerVitals {
            health: Some(VitalValue {
                current: 40,
                maximum: Some(50),
            }),
            ..PlayerVitals::default()
        })]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            HP_UPDATE,
            Dir::ServerToClient,
            &live_hp(200, 75, 100),
        ),
        vec![Event::SpawnHealthUpdated {
            id: 200,
            current: 75,
            maximum: 100,
        }]
    );

    let mut mana = [0; 20];
    mana[..4].copy_from_slice(&37i32.to_le_bytes());
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            MANA_CHANGE,
            Dir::ServerToClient,
            &mana,
        ),
        vec![Event::PlayerVitalsUpdated(PlayerVitals {
            mana: Some(VitalValue {
                current: 37,
                maximum: None,
            }),
            ..PlayerVitals::default()
        })]
    );

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            ILLUSION,
            Dir::ServerToClient,
            &illusion(100, 75, 1),
        ),
        vec![Event::PlayerAppearanceUpdated(PlayerAppearance {
            race: Some(75),
            gender: Some(1),
            animation: None,
        })]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            DEATH,
            Dir::ServerToClient,
            &death(200, 100),
        ),
        vec![Event::SpawnDied {
            id: 200,
            killer_id: Some(100),
        }]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            DEATH,
            Dir::ServerToClient,
            &death(100, 0),
        ),
        vec![Event::PlayerDied { killer_id: None }]
    );
}

#[cfg(feature = "backend-live")]
#[test]
fn live_interleaves_self_and_other_final_events() {
    assert_live_trace(BackendId::Live);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_interleaves_self_and_other_final_events() {
    assert_live_trace(BackendId::Test);
}

#[cfg(feature = "backend-eql")]
fn eql_spawn(name: &str, id: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let text = |bytes: &mut Vec<u8>, value: &str| {
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
    };
    text(&mut bytes, name);
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.push(40);
    bytes.extend_from_slice(&[0; 16]);
    bytes.push(1);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&[0; 8]);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 3]);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 4]);
    bytes.push(95);
    bytes.extend_from_slice(&[0; 33]);
    bytes.extend_from_slice(&50u32.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    text(&mut bytes, "");
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 49]);
    bytes.extend_from_slice(&[0; 60]);
    bytes.extend_from_slice(&[0; 24]);
    for _ in 0..4 {
        text(&mut bytes, "");
    }
    bytes.extend_from_slice(&[0; 4]);
    bytes.push(0);
    text(&mut bytes, "0");
    bytes.extend_from_slice(&[0; 53]);
    bytes
}

#[cfg(feature = "backend-eql")]
fn eql_self_pos(id: u16, x: f32, y: f32, z: f32) -> [u8; 42] {
    let mut payload = [0; 42];
    payload[2..4].copy_from_slice(&id.to_le_bytes());
    payload[10..14].copy_from_slice(&x.to_le_bytes());
    payload[30..34].copy_from_slice(&z.to_le_bytes());
    payload[38..42].copy_from_slice(&y.to_le_bytes());
    payload
}

#[cfg(feature = "backend-eql")]
fn eql_stats(id: u32, health: (i64, i64), mana: Option<(i64, i64)>) -> Vec<u8> {
    let mut payload = id.to_le_bytes().to_vec();
    payload.push(0x03 | if mana.is_some() { 0x04 } else { 0 });
    payload.extend_from_slice(&health.0.to_le_bytes());
    payload.extend_from_slice(&health.1.to_le_bytes());
    if let Some((current, maximum)) = mana {
        payload.extend_from_slice(&current.to_le_bytes());
        payload.extend_from_slice(&maximum.to_le_bytes());
    }
    payload
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_keeps_the_phantom_internal_and_routes_partial_vitals() {
    let mut session = session(BackendId::Eql);
    decode(
        &mut session,
        StreamKind::World,
        ENTER_WORLD,
        Dir::ClientToServer,
        &enter_world("Firona"),
    );
    decode(
        &mut session,
        StreamKind::Zone,
        ZONE_ENTRY,
        Dir::ServerToClient,
        &eql_spawn("Firona", 100),
    );
    assert!(decode(
        &mut session,
        StreamKind::Zone,
        HP_UPDATE,
        Dir::ServerToClient,
        &eql_stats(105, (400, 500), Some((300, 600))),
    )
    .is_empty());
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            ZONE_ENTRY,
            Dir::ServerToClient,
            &eql_spawn("Firona", 105),
        ),
        vec![Event::PlayerVitalsUpdated(PlayerVitals {
            health: Some(VitalValue {
                current: 400,
                maximum: Some(500),
            }),
            mana: Some(VitalValue {
                current: 300,
                maximum: Some(600),
            }),
            endurance: None,
        })]
    );
    assert!(decode(
        &mut session,
        StreamKind::Zone,
        ZONE_ENTRY,
        Dir::ServerToClient,
        &eql_spawn("a rat", 200),
    )
    .iter()
    .any(|event| matches!(event, Event::SpawnAdded(spawn) if spawn.id == 200)));

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            CLIENT_UPDATE,
            Dir::ClientToServer,
            &eql_self_pos(105, 10.0, 20.0, 30.0),
        ),
        vec![Event::PlayerMoved {
            spawn_id: Some(100),
            pos: Pos {
                x: 10,
                y: 20,
                z: 30,
                heading_deg: 0,
            },
        }]
    );

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            HP_UPDATE,
            Dir::ServerToClient,
            &eql_stats(105, (390, 500), None),
        ),
        vec![Event::PlayerVitalsUpdated(PlayerVitals {
            health: Some(VitalValue {
                current: 390,
                maximum: Some(500),
            }),
            mana: None,
            endurance: None,
        })]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            HP_UPDATE,
            Dir::ServerToClient,
            &eql_stats(200, (80, 100), None),
        ),
        vec![Event::SpawnHealthUpdated {
            id: 200,
            current: 80,
            maximum: 100,
        }]
    );

    let mut mana = [0; 20];
    mana[..4].copy_from_slice(&275i32.to_le_bytes());
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            MANA_CHANGE,
            Dir::ServerToClient,
            &mana,
        ),
        vec![Event::PlayerVitalsUpdated(PlayerVitals {
            mana: Some(VitalValue {
                current: 275,
                maximum: None,
            }),
            ..PlayerVitals::default()
        })]
    );

    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            ILLUSION,
            Dir::ServerToClient,
            &illusion(105, 75, 1),
        ),
        vec![Event::PlayerAppearanceUpdated(PlayerAppearance {
            race: Some(75),
            gender: Some(1),
            animation: None,
        })]
    );

    let mut pose = [0; 24];
    pose[..4].copy_from_slice(&105u32.to_le_bytes());
    pose[4..8].copy_from_slice(&6u32.to_le_bytes());
    pose[8..12].copy_from_slice(&110u32.to_le_bytes());
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            APPEARANCE,
            Dir::ServerToClient,
            &pose,
        ),
        vec![Event::PlayerAppearanceUpdated(PlayerAppearance {
            animation: Some(110),
            ..PlayerAppearance::default()
        })]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            DEATH,
            Dir::ServerToClient,
            &death(200, 100),
        ),
        vec![Event::SpawnDied {
            id: 200,
            killer_id: Some(100),
        }]
    );
    assert_eq!(
        decode(
            &mut session,
            StreamKind::Zone,
            DEATH,
            Dir::ServerToClient,
            &death(105, 0),
        ),
        vec![Event::PlayerDied { killer_id: None }]
    );
}
