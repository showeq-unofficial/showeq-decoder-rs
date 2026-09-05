use seq_events::{ActiveBuff, CastInterruptionReason, Event};
use seq_session::{
    BackendId, DecodeDisposition, Dir, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

const ACTION2: u16 = 0;
const ACTION: u16 = 1;
const BEGIN_CAST: u16 = 2;
const CAST_SPELL: u16 = 3;
const SIMPLE_MESSAGE: u16 = 4;
#[cfg(any(feature = "backend-live", feature = "backend-test"))]
const BUFF: u16 = 5;
#[cfg(feature = "backend-eql")]
const BUFF_LIST: u16 = 6;
const SELF_POS: u16 = 7;

#[derive(Debug, Clone, Copy)]
enum WireFamily {
    #[cfg(any(feature = "backend-live", feature = "backend-test"))]
    Live,
    #[cfg(feature = "backend-eql")]
    Eql,
}

impl WireFamily {
    fn resolves_self_from_client_update(self) -> bool {
        match self {
            #[cfg(any(feature = "backend-live", feature = "backend-test"))]
            Self::Live => true,
            #[cfg(feature = "backend-eql")]
            Self::Eql => false,
        }
    }
}

fn session(backend: BackendId, base: u16) -> Session {
    let names = [
        "OP_Action2",
        "OP_Action",
        "OP_BeginCast",
        "OP_CastSpell",
        "OP_SimpleMessage",
        "OP_Buff",
        "OP_BuffList",
        "OP_ClientUpdate",
    ];
    let catalog = names
        .iter()
        .enumerate()
        .map(|(offset, name)| {
            format!(
                "[[zone]]\nid='{:04x}'\nname='{name}'\n",
                base + offset as u16
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
    registry.replace_from_str(backend, &catalog).unwrap();
    Session::new(SessionConfig {
        backend,
        protocol_registry: registry,
    })
    .expect("backend linked")
}

fn decode(session: &mut Session, opcode: u16, direction: Dir, payload: &[u8]) -> Vec<Event> {
    let batch = session.decode(StreamKind::Zone, OpcodeId(opcode), direction, payload);
    assert_eq!(batch.disposition, DecodeDisposition::Decoded);
    batch.events
}

fn action2(source: u16, target: u16, damage: i32, spell: i32, kind: u8) -> [u8; 48] {
    let mut payload = [0; 48];
    payload[..2].copy_from_slice(&target.to_le_bytes());
    payload[2..4].copy_from_slice(&source.to_le_bytes());
    payload[8..12].copy_from_slice(&damage.to_le_bytes());
    payload[20..24].copy_from_slice(&spell.to_le_bytes());
    payload[40] = kind;
    payload
}

fn action(source: u16, target: u16, spell: u16, level: u8) -> [u8; 64] {
    let mut payload = [0; 64];
    payload[..2].copy_from_slice(&target.to_le_bytes());
    payload[2..4].copy_from_slice(&source.to_le_bytes());
    payload[4..6].copy_from_slice(&spell.to_le_bytes());
    payload[12] = level;
    payload[56] = 0xe7;
    payload
}

fn begin_cast(family: WireFamily, caster: u16, spell: u32, cast_time_ms: u16) -> Vec<u8> {
    let len = match family {
        #[cfg(any(feature = "backend-live", feature = "backend-test"))]
        WireFamily::Live => 15,
        #[cfg(feature = "backend-eql")]
        WireFamily::Eql => 19,
    };
    let mut payload = vec![0; len];
    payload[..4].copy_from_slice(&spell.to_le_bytes());
    payload[4..6].copy_from_slice(&caster.to_le_bytes());
    payload[6..8].copy_from_slice(&cast_time_ms.to_le_bytes());
    payload
}

fn cast_spell(family: WireFamily, slot: i32, spell: u32, target: u32) -> Vec<u8> {
    let len = match family {
        #[cfg(any(feature = "backend-live", feature = "backend-test"))]
        WireFamily::Live => 39,
        #[cfg(feature = "backend-eql")]
        WireFamily::Eql => 44,
    };
    let mut payload = vec![0; len];
    payload[..4].copy_from_slice(&slot.to_le_bytes());
    payload[4..8].copy_from_slice(&spell.to_le_bytes());
    payload[18..22].copy_from_slice(&target.to_le_bytes());
    payload
}

fn simple_message(format_id: u32) -> [u8; 12] {
    let mut payload = [0; 12];
    payload[..4].copy_from_slice(&format_id.to_le_bytes());
    payload
}

fn self_pos(spawn_id: u16) -> [u8; 42] {
    let mut payload = [0; 42];
    payload[2..4].copy_from_slice(&spawn_id.to_le_bytes());
    payload
}

fn semantic_damage(events: &[Event]) -> &Event {
    events
        .iter()
        .find(|event| matches!(event, Event::CombatDamage { .. }))
        .expect("semantic combat damage")
}

fn semantic_started(events: &[Event]) -> &Event {
    events
        .iter()
        .find(|event| matches!(event, Event::SpellCastStarted { .. }))
        .expect("semantic cast start")
}

fn exercise_combat(backend: BackendId, family: WireFamily, base: u16) {
    let mut session = session(backend, base);

    if family.resolves_self_from_client_update() {
        decode(
            &mut session,
            base + SELF_POS,
            Dir::ClientToServer,
            &self_pos(77),
        );
    }

    let requested = decode(
        &mut session,
        base + CAST_SPELL,
        Dir::ClientToServer,
        &cast_spell(family, 3, 70_001, 88),
    );
    assert!(matches!(
        semantic_started(&requested),
        Event::SpellCastStarted {
            caster_id,
            target_id: Some(88),
            spell_id: 70_001,
            cast_time_ms: None,
            slot: Some(3),
        } if *caster_id == family.resolves_self_from_client_update().then_some(77)
    ));

    let melee = decode(
        &mut session,
        base + ACTION2,
        Dir::ServerToClient,
        &action2(90, 91, 42, -1, 7),
    );
    assert!(matches!(
        semantic_damage(&melee),
        Event::CombatDamage {
            source_id: Some(90),
            target_id: Some(91),
            damage: 42,
            spell_id: None,
            ..
        }
    ));

    if family.resolves_self_from_client_update() {
        let outbound = decode(
            &mut session,
            base + ACTION2,
            Dir::ClientToServer,
            &action2(0, 91, 5, -1, 1),
        );
        assert!(matches!(
            semantic_damage(&outbound),
            Event::CombatDamage {
                source_id: Some(77),
                target_id: Some(91),
                spell_id: None,
                ..
            }
        ));
    }

    let begun = decode(
        &mut session,
        base + BEGIN_CAST,
        Dir::ServerToClient,
        &begin_cast(family, 77, 70_001, 2_500),
    );
    assert!(matches!(
        semantic_started(&begun),
        Event::SpellCastStarted {
            caster_id: Some(77),
            spell_id: 70_001,
            cast_time_ms: Some(2_500),
            ..
        }
    ));

    let spell_damage = decode(
        &mut session,
        base + ACTION2,
        Dir::ServerToClient,
        &action2(77, 88, 1_234, 70_001, 0xe7),
    );
    assert!(matches!(
        semantic_damage(&spell_damage),
        Event::CombatDamage {
            source_id: Some(77),
            target_id: Some(88),
            spell_id: Some(70_001),
            ..
        }
    ));

    let resolved = decode(
        &mut session,
        base + ACTION,
        Dir::ServerToClient,
        &action(77, 88, 65_000, 125),
    );
    assert!(resolved.iter().any(|event| matches!(
        event,
        Event::SpellActionResolved {
            source_id: Some(77),
            target_id: Some(88),
            spell_id: 65_000,
            caster_level: Some(125),
            kind: 0xe7,
        }
    )));
    let sentinel = decode(
        &mut session,
        base + ACTION,
        Dir::ServerToClient,
        &action(77, 88, u16::MAX, 125),
    );
    assert!(sentinel.iter().any(|event| matches!(
        event,
        Event::SpellAction {
            spell_id: 65_535,
            ..
        }
    )));
    assert!(sentinel
        .iter()
        .all(|event| !matches!(event, Event::SpellActionResolved { .. })));

    decode(
        &mut session,
        base + CAST_SPELL,
        Dir::ClientToServer,
        &cast_spell(family, 4, 99_999, 0),
    );
    let malformed = session.decode(
        StreamKind::Zone,
        OpcodeId(base + CAST_SPELL),
        Dir::ClientToServer,
        &[0; 2],
    );
    assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
    assert!(malformed.events.is_empty());
    let wrong_direction = session.decode(
        StreamKind::Zone,
        OpcodeId(base + SIMPLE_MESSAGE),
        Dir::ClientToServer,
        &simple_message(439),
    );
    assert!(matches!(
        wrong_direction.disposition,
        DecodeDisposition::Decoded | DecodeDisposition::Ignored
    ));
    assert!(wrong_direction
        .events
        .iter()
        .all(|event| !matches!(event, Event::SpellCastInterrupted { .. })));
    let interrupted = decode(
        &mut session,
        base + SIMPLE_MESSAGE,
        Dir::ServerToClient,
        &simple_message(439),
    );
    assert!(interrupted.iter().any(|event| matches!(
        event,
        Event::SpellCastInterrupted {
            spell_id: 99_999,
            reason: CastInterruptionReason::ServerMessage,
            ..
        }
    )));
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_combat_fixture() {
    exercise_combat(BackendId::Live, WireFamily::Live, 0x5100);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_combat_fixture() {
    exercise_combat(BackendId::Test, WireFamily::Live, 0x5200);
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_combat_fixture() {
    exercise_combat(BackendId::Eql, WireFamily::Eql, 0x5300);
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_buff(spawn_id: u32, spell_id: u32, form: u8, value: u32) -> Vec<u8> {
    let len = match form {
        0 => 13,
        1 => 30,
        2 => 34,
        _ => unreachable!("fixture form"),
    };
    let mut payload = vec![0; len];
    payload[..4].copy_from_slice(&spawn_id.to_le_bytes());
    payload[4..8].copy_from_slice(&spell_id.to_le_bytes());
    if form == 1 {
        payload[9] = value as u8;
    } else if form == 2 {
        payload[15..19].copy_from_slice(&value.to_le_bytes());
    }
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn exercise_live_buff(backend: BackendId, base: u16) {
    let mut session = session(backend, base);
    let added = decode(
        &mut session,
        base + BUFF,
        Dir::ServerToClient,
        &live_buff(77, 70_001, 1, 3),
    );
    assert!(added.iter().any(|event| matches!(
        event,
        Event::BuffAdded(ActiveBuff {
            owner_id: Some(77),
            spell_id: 70_001,
            remaining_ticks: None,
            slot: Some(3),
            ..
        })
    )));
    let updated = decode(
        &mut session,
        base + BUFF,
        Dir::ServerToClient,
        &live_buff(77, 70_001, 2, 600),
    );
    assert!(updated.iter().any(|event| matches!(
        event,
        Event::BuffUpdated(ActiveBuff {
            remaining_ticks: Some(600),
            ..
        })
    )));
    let removed = decode(
        &mut session,
        base + BUFF,
        Dir::ServerToClient,
        &live_buff(77, 70_001, 0, 0),
    );
    assert!(removed.iter().any(|event| matches!(
        event,
        Event::BuffRemoved {
            owner_id: Some(77),
            spell_id: 70_001,
            slot: Some(3),
        }
    )));

    decode(
        &mut session,
        base + CAST_SPELL,
        Dir::ClientToServer,
        &cast_spell(WireFamily::Live, 5, 80_001, 99),
    );
    decode(
        &mut session,
        base + BUFF,
        Dir::ServerToClient,
        &live_buff(77, 80_002, 1, 4),
    );
    let flushed = session.flush(seq_session::FlushReason::ReplayEnd);
    assert!(matches!(
        flushed.as_slice(),
        [
            Event::SpellCastInterrupted {
                spell_id: 80_001,
                reason: CastInterruptionReason::ReplayEnd,
                ..
            },
            Event::BuffRemoved {
                spell_id: 80_002,
                slot: Some(4),
                ..
            }
        ]
    ));
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_buff_fixture() {
    exercise_live_buff(BackendId::Live, 0x5400);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_buff_fixture() {
    exercise_live_buff(BackendId::Test, 0x5500);
}

#[cfg(feature = "backend-eql")]
fn eql_buff_list(owner: u32, entries: &[(u32, i32, &str, u16)]) -> Vec<u8> {
    let mut payload = vec![0; 15];
    payload[..4].copy_from_slice(&owner.to_le_bytes());
    payload[8] = 1;
    payload[9] = entries.len() as u8;
    for (index, &(spell_id, ticks, caster, slot)) in entries.iter().enumerate() {
        payload.extend_from_slice(&spell_id.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&ticks.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(caster.as_bytes());
        payload.push(0);
        if index + 1 == entries.len() {
            payload.extend_from_slice(&slot.to_le_bytes());
        } else {
            payload.extend_from_slice(&u32::from(slot).to_le_bytes());
        }
    }
    payload
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_buff_snapshot_orders_removals_before_additions() {
    let base = 0x5600;
    let mut session = session(BackendId::Eql, base);
    let first = eql_buff_list(
        88,
        &[
            (0, 10, "", 0),
            (70_001, 50, "Testchar", 1),
            (99_999, -1, "", 2),
            (u32::MAX, 10, "", 3),
        ],
    );
    let added = decode(&mut session, base + BUFF_LIST, Dir::ServerToClient, &first);
    assert_eq!(
        added
            .iter()
            .filter(|event| matches!(event, Event::BuffAdded(_)))
            .count(),
        2
    );

    let second = eql_buff_list(88, &[(70_002, 40, "Testchar", 1)]);
    let changed = decode(&mut session, base + BUFF_LIST, Dir::ServerToClient, &second);
    let semantic: Vec<_> = changed
        .iter()
        .filter(|event| matches!(event, Event::BuffAdded(_) | Event::BuffRemoved { .. }))
        .collect();
    assert!(matches!(
        semantic[0],
        Event::BuffRemoved {
            spell_id: 70_001,
            ..
        }
    ));
    assert!(matches!(
        semantic[1],
        Event::BuffRemoved {
            spell_id: 99_999,
            ..
        }
    ));
    assert!(matches!(
        semantic[2],
        Event::BuffAdded(ActiveBuff {
            spell_id: 70_002,
            ..
        })
    ));

    let malformed = session.decode(
        StreamKind::Zone,
        OpcodeId(base + BUFF_LIST),
        Dir::ServerToClient,
        &second[..10],
    );
    assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
    let duplicate = decode(&mut session, base + BUFF_LIST, Dir::ServerToClient, &second);
    assert!(duplicate.iter().all(|event| !matches!(
        event,
        Event::BuffAdded(_) | Event::BuffUpdated(_) | Event::BuffRemoved { .. }
    )));
}
