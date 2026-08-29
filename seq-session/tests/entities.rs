use seq_events::{Event, Point3, Pos, Velocity};
#[cfg(feature = "backend-live")]
use seq_session::FlushReason;
use seq_session::{
    BackendId, DecodeDisposition, Dir, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

const NAMES: [&str; 9] = [
    "OP_ZoneEntry",
    "OP_MobUpdate",
    "OP_RemoveSpawn",
    "OP_SpawnRename",
    "OP_SpawnDoor",
    "OP_GroundSpawn",
    "OP_ClickObject",
    "OP_CorpseLocResponse",
    "OP_SendZonePoints",
];

fn session(backend: BackendId, first_opcode: u16) -> Session {
    let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
    let catalog = NAMES
        .iter()
        .enumerate()
        .map(|(offset, name)| {
            format!(
                "[[zone]]\nid='{:04x}'\nname='{name}'\n",
                first_opcode + offset as u16
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    registry
        .replace_from_str(backend, &catalog)
        .expect("canonical numeric entity catalog");
    Session::new(SessionConfig {
        backend,
        protocol_registry: registry,
    })
    .expect("backend linked")
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
    bytes.extend_from_slice(&[0; 20]);
    for value in [700u32, 0, 0, 0, 0, 800, 0, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let signed = |value: i32, bits: u32| (value as u32) & ((1 << bits) - 1);
    for value in [
        signed(24, 19) | (signed(7, 10) << 19),
        signed(80, 19) | (1024 << 19),
        signed(20, 13),
        signed(160, 19) | (signed(-28, 13) << 19),
        signed(9, 10) | (signed(36, 13) << 10),
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[0; 8]);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 66]);
    bytes
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

fn rename(old_name: &str, new_name: &str) -> [u8; 195] {
    let mut bytes = [0; 195];
    bytes[..old_name.len()].copy_from_slice(old_name.as_bytes());
    bytes[64..64 + old_name.len()].copy_from_slice(old_name.as_bytes());
    bytes[128..128 + new_name.len()].copy_from_slice(new_name.as_bytes());
    bytes
}

fn door(row_len: usize) -> Vec<u8> {
    let mut bytes = vec![0; row_len];
    bytes[..4].copy_from_slice(b"OAK1");
    bytes[32..36].copy_from_slice(&1.25f32.to_le_bytes());
    bytes[36..40].copy_from_slice(&2.5f32.to_le_bytes());
    bytes[40..44].copy_from_slice(&3.75f32.to_le_bytes());
    bytes[44..48].copy_from_slice(&128.5f32.to_le_bytes());
    bytes[48..52].copy_from_slice(&7u32.to_le_bytes());
    bytes[72..76].copy_from_slice(&100u32.to_le_bytes());
    bytes[80] = 9;
    bytes[81] = 40;
    bytes[82] = 1;
    bytes[83] = 2;
    bytes[84..88].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_ground() -> Vec<u8> {
    let mut bytes = 77u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"IT63_ACTORDEF\0");
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&90.5f32.to_le_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&11.25f32.to_le_bytes());
    bytes.extend_from_slice(&22.5f32.to_le_bytes());
    bytes.extend_from_slice(&33.75f32.to_le_bytes());
    bytes
}

#[cfg(feature = "backend-eql")]
fn eql_ground() -> Vec<u8> {
    let mut bytes = 77u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"IT63_ACTORDEF\0");
    bytes.extend_from_slice(&[0; 28]);
    bytes.extend_from_slice(&11.25f32.to_le_bytes());
    bytes.extend_from_slice(&22.5f32.to_le_bytes());
    bytes.extend_from_slice(&33.75f32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
}

#[cfg(any(feature = "backend-live", feature = "backend-eql"))]
fn zone_points() -> Vec<u8> {
    let mut bytes = 1u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&7u32.to_le_bytes());
    bytes.extend_from_slice(&1.25f32.to_le_bytes());
    bytes.extend_from_slice(&2.5f32.to_le_bytes());
    bytes.extend_from_slice(&3.75f32.to_le_bytes());
    bytes.extend_from_slice(&90.5f32.to_le_bytes());
    bytes.extend_from_slice(&57u16.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 24]);
    bytes
}

#[cfg(feature = "backend-test")]
fn test_zone_points() -> Vec<u8> {
    let mut bytes = vec![0; 136];
    bytes[..13].copy_from_slice(b"POKCABPORT500");
    bytes[0x20..0x24].copy_from_slice(&1.25f32.to_le_bytes());
    bytes[0x24..0x28].copy_from_slice(&2.5f32.to_le_bytes());
    bytes[0x28..0x2c].copy_from_slice(&3.75f32.to_le_bytes());
    bytes[0x2c..0x30].copy_from_slice(&90.5f32.to_le_bytes());
    bytes
}

fn decode(session: &mut Session, opcode: u16, payload: &[u8]) -> Vec<Event> {
    let batch = session.decode(
        StreamKind::Zone,
        OpcodeId(opcode),
        Dir::ServerToClient,
        payload,
    );
    assert_eq!(batch.disposition, DecodeDisposition::Decoded);
    batch.events
}

fn assert_entity_trace(
    backend: BackendId,
    first_opcode: u16,
    spawn_payload: &[u8],
    door_len: usize,
    ground_payload: &[u8],
    zone_points_payload: &[u8],
) {
    let mut session = session(backend, first_opcode);
    let spawn_id = 4_242;
    let added = decode(&mut session, first_opcode, spawn_payload);
    let [Event::SpawnAdded(spawn)] = added.as_slice() else {
        panic!("expected one spawn-added event");
    };
    assert_eq!(spawn.id, spawn_id);
    assert_eq!(spawn.name, "a guard");
    match backend {
        BackendId::Live | BackendId::Test => {
            assert_eq!(
                spawn.pos,
                Some(Pos {
                    x: 20,
                    y: 10,
                    z: 3,
                    heading_deg: 270,
                })
            );
            assert_eq!(
                spawn.velocity,
                Velocity {
                    x: Some(-7),
                    y: Some(5),
                    z: Some(9),
                }
            );
            assert_eq!(spawn.delta_heading, Some(9));
            assert_eq!(spawn.animation, Some(7));
            assert_eq!(
                spawn.equipment_models,
                Some([0, 0, 0, 0, 0, 0, 0, 700, 800])
            );
        }
        BackendId::Eql => {
            assert_eq!(spawn.velocity, Velocity::default());
            assert_eq!(spawn.delta_heading, None);
            assert_eq!(spawn.animation, None);
            assert_eq!(spawn.equipment_models, None);
        }
    }

    let mut movement = vec![0; if backend == BackendId::Eql { 18 } else { 14 }];
    movement[..4].copy_from_slice(&spawn_id.to_le_bytes());
    assert!(matches!(
        decode(&mut session, first_opcode + 1, &movement).as_slice(),
        [Event::SpawnMoved { id, velocity, delta_heading: None, animation: None, .. }]
            if *id == spawn_id && *velocity == Velocity::default()
    ));

    assert_eq!(
        decode(
            &mut session,
            first_opcode + 3,
            &rename("a guard", "Captain Guard")
        ),
        vec![Event::SpawnRenamed {
            id: Some(spawn_id),
            old_name: "a guard".into(),
            new_name: "Captain Guard".into(),
        }]
    );

    let doors = decode(&mut session, first_opcode + 4, &door(door_len));
    let [Event::Doors(doors)] = doors.as_slice() else {
        panic!("expected door set");
    };
    assert_eq!(doors[0].id, 9);
    assert_eq!(
        doors[0].position,
        Point3 {
            x: 2.5,
            y: 1.25,
            z: 3.75
        }
    );
    assert_eq!(doors[0].zone_point_id, None);

    let ground = decode(&mut session, first_opcode + 5, ground_payload);
    let [Event::GroundItem(ground)] = ground.as_slice() else {
        panic!("expected ground item");
    };
    assert_eq!(ground.id, 77);
    assert_eq!(ground.actor_definition, "IT63_ACTORDEF");
    assert_eq!(
        ground.position,
        Point3 {
            x: 22.5,
            y: 11.25,
            z: 33.75
        }
    );
    if backend == BackendId::Eql {
        assert_eq!(ground.heading, None);
    } else {
        assert_eq!(ground.heading, Some(90.5));
    }

    let mut corpse = [0; 16];
    corpse[..4].copy_from_slice(&spawn_id.to_le_bytes());
    corpse[4..8].copy_from_slice(&4.25f32.to_le_bytes());
    corpse[8..12].copy_from_slice(&5.5f32.to_le_bytes());
    corpse[12..16].copy_from_slice(&6.75f32.to_le_bytes());
    assert_eq!(
        decode(&mut session, first_opcode + 7, &corpse),
        vec![Event::CorpseLocated {
            id: spawn_id,
            position: Point3 {
                x: 4.25,
                y: 5.5,
                z: 6.75
            },
        }]
    );

    let points = decode(&mut session, first_opcode + 8, zone_points_payload);
    let [Event::ZonePoints(points)] = points.as_slice() else {
        panic!("expected zone points");
    };
    assert_eq!(points.len(), 1);
    if backend == BackendId::Test {
        assert_eq!(points[0].trigger_id, None);
        assert_eq!(points[0].actor_definition.as_deref(), Some("POKCABPORT500"));
        assert_eq!(points[0].destination_zone_id, None);
    } else {
        assert_eq!(points[0].trigger_id, Some(7));
        assert_eq!(points[0].actor_definition, None);
        assert_eq!(points[0].destination_zone_id, Some(57));
    }

    let mut removed = [0; 5];
    removed[..4].copy_from_slice(&spawn_id.to_le_bytes());
    removed[4] = 1;
    assert_eq!(
        decode(&mut session, first_opcode + 2, &removed),
        vec![Event::SpawnRemoved { id: spawn_id }]
    );

    let mut click = [0; 12];
    click[..2].copy_from_slice(&77u16.to_le_bytes());
    assert_eq!(
        decode(&mut session, first_opcode + 6, &click),
        vec![Event::GroundItemRemoved { drop_id: 77 }]
    );
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_entity_fixture_has_final_meaning() {
    assert_entity_trace(
        BackendId::Live,
        0x5101,
        &live_spawn("a guard", 4_242),
        136,
        &live_ground(),
        &zone_points(),
    );
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_entity_fixture_has_final_meaning() {
    assert_entity_trace(
        BackendId::Test,
        0x5201,
        &live_spawn("a guard", 4_242),
        136,
        &live_ground(),
        &test_zone_points(),
    );
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_entity_fixture_has_final_meaning() {
    assert_entity_trace(
        BackendId::Eql,
        0x5301,
        &eql_spawn("a guard", 4_242),
        132,
        &eql_ground(),
        &zone_points(),
    );
}

#[cfg(feature = "backend-live")]
#[test]
fn ambiguous_rename_never_invents_an_entity_id() {
    let first = 0x5401;
    let mut session = session(BackendId::Live, first);
    decode(&mut session, first, &live_spawn("a guard", 1));
    decode(&mut session, first, &live_spawn("a guard", 2));
    assert_eq!(
        decode(&mut session, first + 3, &rename("a guard", "Captain Guard")),
        vec![Event::SpawnRenamed {
            id: None,
            old_name: "a guard".into(),
            new_name: "Captain Guard".into(),
        }]
    );
}

#[cfg(feature = "backend-live")]
#[test]
fn rename_index_tracks_removals_and_resets() {
    let first = 0x5601;
    let mut session = session(BackendId::Live, first);
    decode(&mut session, first, &live_spawn("a guard", 1));
    decode(&mut session, first, &live_spawn("a guard", 2));
    let mut removed = [0; 5];
    removed[..4].copy_from_slice(&1u32.to_le_bytes());
    decode(&mut session, first + 2, &removed);
    assert!(matches!(
        decode(&mut session, first + 3, &rename("a guard", "Captain Guard")).as_slice(),
        [Event::SpawnRenamed { id: Some(2), .. }]
    ));

    assert_eq!(
        session.flush(FlushReason::Reset),
        vec![Event::SessionReset {
            reason: seq_events::SessionResetReason::Explicit,
        }]
    );
    assert!(matches!(
        decode(
            &mut session,
            first + 3,
            &rename("Captain Guard", "Guard Captain")
        )
        .as_slice(),
        [Event::SpawnRenamed { id: None, .. }]
    ));
}

#[cfg(feature = "backend-test")]
#[test]
fn test_zone_point_rows_reject_bad_framing_and_values() {
    let first = 0x5801;
    let mut session = session(BackendId::Test, first);
    let mut unterminated_name = vec![0; 136];
    unterminated_name[..32].fill(b'A');
    for payload in [vec![0; 135], vec![0; 136], unterminated_name] {
        let batch = session.decode(
            StreamKind::Zone,
            OpcodeId(first + 8),
            Dir::ServerToClient,
            &payload,
        );
        assert_eq!(batch.disposition, DecodeDisposition::Malformed);
        assert!(batch.events.is_empty());
    }

    let mut non_finite = test_zone_points();
    non_finite[0x20..0x24].copy_from_slice(&f32::NAN.to_le_bytes());
    let batch = session.decode(
        StreamKind::Zone,
        OpcodeId(first + 8),
        Dir::ServerToClient,
        &non_finite,
    );
    assert_eq!(batch.disposition, DecodeDisposition::Malformed);
    assert!(batch.events.is_empty());
}

#[cfg(feature = "backend-live")]
#[test]
fn malformed_entity_batches_do_not_emit_partial_state() {
    let first = 0x5501;
    let mut session = session(BackendId::Live, first);
    for (opcode, payload) in [
        (first + 3, vec![0; 194]),
        (first + 4, vec![0; 135]),
        (first + 7, vec![0; 15]),
        (first + 8, vec![1, 0, 0, 0]),
    ] {
        let batch = session.decode(
            StreamKind::Zone,
            OpcodeId(opcode),
            Dir::ServerToClient,
            &payload,
        );
        assert_eq!(batch.disposition, DecodeDisposition::Malformed);
        assert!(batch.events.is_empty());
    }
}
