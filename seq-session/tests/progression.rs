#[cfg(any(feature = "backend-live", feature = "backend-test"))]
use seq_events::ItemLocation;
#[cfg(feature = "backend-eql")]
use seq_events::MoneyBalance;
use seq_events::{AlternateAdvancementProgress, Event, ExperienceProgress, SkillValue};
use seq_session::{
    BackendId, DecodeDisposition, Dir, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

const ITEM: u16 = 0;
const SKILL: u16 = 1;
const EXPERIENCE: u16 = 2;
const AA_EXPERIENCE: u16 = 3;
#[cfg(feature = "backend-eql")]
const MONEY: u16 = 4;
#[cfg(feature = "backend-eql")]
const AA_TABLE: u16 = 5;
#[cfg(feature = "backend-eql")]
const LEVEL: u16 = 6;

fn session(backend: BackendId, base: u16) -> Session {
    let names = [
        "OP_ItemPacket",
        "OP_SkillUpdate",
        "OP_ExpUpdate",
        "OP_AAExpUpdate",
        "OP_MoneyUpdate",
        "OP_SendAATable",
        "OP_LevelUpdate",
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
    registry
        .replace_from_str(backend, &catalog)
        .expect("canonical numeric progression catalog");
    Session::new(SessionConfig {
        backend,
        protocol_registry: registry,
    })
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

fn skill(skill_id: u32, value: u32) -> [u8; 12] {
    let mut payload = [0; 12];
    payload[..4].copy_from_slice(&skill_id.to_le_bytes());
    payload[4..8].copy_from_slice(&value.to_le_bytes());
    payload
}

fn experience(value: u32) -> [u8; 16] {
    let mut payload = [0; 16];
    payload[..4].copy_from_slice(&value.to_le_bytes());
    payload[8..12].copy_from_slice(&2u32.to_le_bytes());
    payload
}

fn aa_experience(value: u32, points: u32) -> [u8; 12] {
    let mut payload = [0; 12];
    payload[..4].copy_from_slice(&value.to_le_bytes());
    payload[4..8].copy_from_slice(&points.to_le_bytes());
    payload
}

fn assert_common_progression(backend: BackendId, base: u16) {
    let mut session = session(backend, base);

    assert_eq!(
        decode(&mut session, base + SKILL, &skill(30, 12)),
        vec![
            Event::SkillUpdate {
                skill_id: 30,
                value: 12,
            },
            Event::SkillValueUpdated(SkillValue {
                skill_id: 30,
                value: 12,
            }),
        ]
    );
    assert_eq!(
        decode(&mut session, base + SKILL, &skill(30, 12)),
        vec![Event::SkillUpdate {
            skill_id: 30,
            value: 12,
        }]
    );

    assert_eq!(
        decode(&mut session, base + EXPERIENCE, &experience(97_900)),
        vec![
            Event::Exp { exp: 97_900 },
            Event::ExperienceUpdated(ExperienceProgress {
                experience: 97_900,
                level: None,
                previous_level: None,
            }),
        ]
    );
    assert_eq!(
        decode(&mut session, base + EXPERIENCE, &experience(97_900)),
        vec![Event::Exp { exp: 97_900 }]
    );

    assert_eq!(
        decode(
            &mut session,
            base + AA_EXPERIENCE,
            &aa_experience(91_234, 7),
        ),
        vec![
            Event::AaExp {
                alt_exp: 91_234,
                aa_points: 7,
            },
            Event::AlternateAdvancementUpdated(AlternateAdvancementProgress {
                experience: 91_234,
                unspent_points: 7,
            }),
        ]
    );
    assert_eq!(
        decode(
            &mut session,
            base + AA_EXPERIENCE,
            &aa_experience(91_234, 7),
        ),
        vec![Event::AaExp {
            alt_exp: 91_234,
            aa_points: 7,
        }]
    );
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn live_item(serial: &[u8; 16], sub_slot: u16) -> Vec<u8> {
    let mut payload = vec![0; 21];
    payload[..4].copy_from_slice(&0x76u32.to_le_bytes());
    payload[4..20].copy_from_slice(serial);
    payload[20] = 0;
    payload.extend_from_slice(&7u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&sub_slot.to_le_bytes());
    payload.resize(72, b'Q');
    payload.push(0);
    payload.extend_from_slice(b"Gloomingdeep Lantern\0A lantern\0");
    let body = payload.len();
    payload.resize(body + 63, 0);
    payload[body + 8..body + 12].copy_from_slice(&9979u32.to_le_bytes());
    payload[body + 12..body + 16].copy_from_slice(&5u32.to_le_bytes());
    payload[body + 16..body + 20].copy_from_slice(&0x1234_5678u32.to_le_bytes());
    payload[body + 20..body + 24].copy_from_slice(&18432u32.to_le_bytes());
    payload[body + 34..body + 39].copy_from_slice(&[1, 2, 3, 4, 5]);
    payload[body + 39] = (-6i8) as u8;
    payload[body + 40..body + 47].copy_from_slice(&[7, 8, 9, 10, 11, 12, 13]);
    payload[body + 47..body + 51].copy_from_slice(&14u32.to_le_bytes());
    payload[body + 51..body + 55].copy_from_slice(&15u32.to_le_bytes());
    payload[body + 55..body + 59].copy_from_slice(&16u32.to_le_bytes());
    payload[body + 59..body + 63].copy_from_slice(&17u32.to_le_bytes());
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn assert_live_item_updates(backend: BackendId, base: u16) {
    let mut session = session(backend, base);
    let serial = *b"un000BG0001R0G00";
    let worn = live_item(&serial, 2);
    let first = decode(&mut session, base + ITEM, &worn);
    assert_eq!(first.len(), 3);
    let Event::InventoryItemUpdated {
        item,
        previous_location: None,
    } = &first[1]
    else {
        panic!("expected first inventory update");
    };
    assert_eq!(item.icon, None);
    assert_eq!(item.stack_count, Some(7));
    assert_eq!(item.weight_tenths, Some(5));
    assert_eq!(item.flags, Some(0x1234_5678));
    assert_eq!(item.corruption, Some(-6));
    assert_eq!(item.stats, vec![7, 8, 9, 10, 11, 12, 13]);
    assert_eq!(item.resists, vec![1, 2, 3, 4, 5]);
    assert!(matches!(
        &first[2],
        Event::EquipmentSlotUpdated { slot: 2, item: Some(item) }
            if item.item_id == 9979
    ));

    assert!(matches!(
        decode(&mut session, base + ITEM, &worn).as_slice(),
        [Event::ItemLearned { .. }]
    ));

    let carried = live_item(&serial, 24);
    let moved = decode(&mut session, base + ITEM, &carried);
    assert!(matches!(
        moved.as_slice(),
        [
            Event::ItemLearned { .. },
            Event::InventoryItemUpdated {
                previous_location: Some(ItemLocation {
                    container_id: 0,
                    container_slot: 2,
                    parent_slot: seq_events::TOP_LEVEL_SLOT,
                }),
                ..
            },
            Event::EquipmentSlotUpdated {
                slot: 2,
                item: None
            },
        ]
    ));
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_progression_fixture_is_ordered_and_deduplicated() {
    assert_common_progression(BackendId::Live, 0x1100);
    assert_live_item_updates(BackendId::Live, 0x1100);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_progression_fixture_is_ordered_and_deduplicated() {
    assert_common_progression(BackendId::Test, 0x2200);
    assert_live_item_updates(BackendId::Test, 0x2200);
}

#[cfg(feature = "backend-eql")]
fn eql_item(
    serial: &[u8; 16],
    name: &str,
    container: u32,
    parent: u16,
    slot: u16,
    item_id: u32,
    icon: u32,
) -> Vec<u8> {
    const NAME_OFFSET: usize = 123;
    const RECORD_CONTAINER: usize = 21;
    const RECORD_LOCATION: usize = 25;
    let mut record = Vec::new();
    record.extend_from_slice(serial);
    record.push(0);
    record.resize(NAME_OFFSET, 0);
    record[RECORD_CONTAINER..RECORD_CONTAINER + 4].copy_from_slice(&container.to_le_bytes());
    let location = (u32::from(parent) << 16) | u32::from(slot);
    record[RECORD_LOCATION..RECORD_LOCATION + 4].copy_from_slice(&location.to_le_bytes());
    record.extend_from_slice(name.as_bytes());
    record.push(0);
    record.extend_from_slice(format!("{name} lore").as_bytes());
    record.push(0);
    let tail = record.len();
    record.resize(tail + 112, 0);
    record[tail + 8..tail + 12].copy_from_slice(&item_id.to_le_bytes());
    record[tail + 20..tail + 24].copy_from_slice(&0x2000u32.to_le_bytes());
    record[tail + 28..tail + 32].copy_from_slice(&icon.to_le_bytes());
    let set_slot = |record: &mut Vec<u8>, index: usize, value: i16| {
        let offset = tail + index * 4 + 2;
        record[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    };
    for (index, value) in [1, 2, 3, 4, 5].into_iter().enumerate() {
        set_slot(&mut record, 8 + index, value);
    }
    for (index, value) in [6, 7, 8, 9, 10, 11, 12].into_iter().enumerate() {
        set_slot(&mut record, 14 + index, value);
    }
    set_slot(&mut record, 21, 13);
    set_slot(&mut record, 22, 14);
    set_slot(&mut record, 23, 15);
    set_slot(&mut record, 24, 16);
    record
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_fixture_emits_canonical_full_inventory_and_progression() {
    const BASE: u16 = 0x3300;
    let mut session = session(BackendId::Eql, BASE);
    assert_common_progression(BackendId::Eql, BASE);

    let duplicate = *b"iGS0000000000001";
    let second = *b"iGS0000000000002";
    let mut payload = 3u32.to_le_bytes().to_vec();
    payload.extend(eql_item(
        &duplicate,
        "Old Location",
        0,
        seq_events::TOP_LEVEL_SLOT,
        2,
        100,
        200,
    ));
    payload.extend(eql_item(
        &second,
        "Worn Item",
        0,
        seq_events::TOP_LEVEL_SLOT,
        5,
        101,
        201,
    ));
    payload.extend(eql_item(
        &duplicate,
        "Moved Item",
        0,
        seq_events::TOP_LEVEL_SLOT,
        24,
        100,
        202,
    ));

    let first = decode(&mut session, BASE + ITEM, &payload);
    assert_eq!(first.len(), 3);
    let Event::ItemSet { items: raw } = &first[0] else {
        panic!("expected legacy item set");
    };
    assert_eq!(raw.len(), 3);
    let Event::InventorySnapshot { items } = &first[1] else {
        panic!("expected inventory snapshot");
    };
    assert_eq!(items.len(), 2);
    let moved = items
        .iter()
        .find(|item| item.serial == "iGS0000000000001")
        .unwrap();
    assert_eq!(moved.name, "Moved Item");
    assert_eq!(moved.icon, Some(202));
    assert_eq!(moved.stack_count, None);
    assert_eq!(moved.weight_tenths, None);
    assert_eq!(moved.flags, None);
    assert_eq!(moved.corruption, None);
    assert_eq!(moved.stats, vec![6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(moved.resists, vec![1, 2, 3, 4, 5]);
    assert!(matches!(
        &first[2],
        Event::EquipmentSnapshot { items } if items.len() == 1
            && items[0].name == "Worn Item"
            && items[0].container_slot == 5
    ));

    assert!(matches!(
        decode(&mut session, BASE + ITEM, &payload).as_slice(),
        [Event::ItemSet { items }] if items.len() == 3
    ));

    let money = [
        11u32.to_le_bytes(),
        22u32.to_le_bytes(),
        33u32.to_le_bytes(),
        44u32.to_le_bytes(),
        0u32.to_le_bytes(),
    ]
    .concat();
    let money_events = decode(&mut session, BASE + MONEY, &money);
    assert_eq!(
        money_events.last(),
        Some(&Event::MoneyBalanceUpdated(MoneyBalance {
            platinum: 11,
            gold: 22,
            silver: 33,
            copper: 44,
        }))
    );
    assert!(matches!(
        decode(&mut session, BASE + MONEY, &money).as_slice(),
        [Event::Money { .. }]
    ));

    let mut aa_table = [0; 37];
    aa_table[..4].copy_from_slice(&501u32.to_le_bytes());
    aa_table[13..17].copy_from_slice(&601u32.to_le_bytes());
    let definitions = decode(&mut session, BASE + AA_TABLE, &aa_table);
    assert!(matches!(
        definitions.as_slice(),
        [
            Event::AaTable {
                desc_id: 501,
                title_sid: 601,
            },
            Event::AlternateAbilityDefined(definition),
        ] if definition.ability_id == 501 && definition.title_string_id == 601
    ));
    assert!(matches!(
        decode(&mut session, BASE + AA_TABLE, &aa_table).as_slice(),
        [Event::AaTable { .. }]
    ));

    let mut level = [0; 80];
    level[..4].copy_from_slice(&40u32.to_le_bytes());
    level[4..8].copy_from_slice(&39u32.to_le_bytes());
    level[8..12].copy_from_slice(&814u32.to_le_bytes());
    let level_events = decode(&mut session, BASE + LEVEL, &level);
    assert_eq!(
        level_events,
        vec![
            Event::LevelUpdate {
                level: 40,
                level_old: 39,
                exp: 814,
            },
            Event::ExperienceUpdated(ExperienceProgress {
                experience: 814,
                level: Some(40),
                previous_level: Some(39),
            }),
        ]
    );
    assert_eq!(
        decode(&mut session, BASE + EXPERIENCE, &experience(814)),
        vec![Event::Exp { exp: 814 }]
    );
}
