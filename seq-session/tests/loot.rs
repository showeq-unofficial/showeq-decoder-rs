#[cfg(feature = "backend-eql")]
use seq_events::{Event, LootAcquisition, SessionResetReason};
use seq_session::{
    BackendId, DecodeDisposition, Dir, FlushReason, OpcodeId, ProtocolRegistry, Session,
    SessionConfig, StreamKind,
};
use std::sync::Arc;

#[cfg(feature = "backend-eql")]
const ENTER_WORLD: u16 = 0;
const MESSAGE: u16 = 1;
const TRANSACTION: u16 = 2;
const DROPS: u16 = 3;

fn session(backend: BackendId, base: u16) -> Session {
    let names = [
        "OP_EnterWorld",
        "OP_LootMessage",
        "OP_LootTransaction",
        "OP_LootDrops",
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

#[cfg(feature = "backend-eql")]
fn decode_at(
    session: &mut Session,
    opcode: u16,
    direction: Dir,
    payload: &[u8],
    timestamp: i64,
) -> Vec<Event> {
    let batch = session.decode_at(
        StreamKind::Zone,
        OpcodeId(opcode),
        direction,
        payload,
        timestamp,
    );
    assert_eq!(batch.disposition, DecodeDisposition::Decoded);
    batch.events
}

fn message(text: &str) -> Vec<u8> {
    let mut payload = 286u32.to_le_bytes().to_vec();
    payload.extend_from_slice(text.as_bytes());
    payload.push(0);
    payload
}

fn confirmation(corpse_id: u32, item_id: u32, quantity: u32, coin: u32, sequence: u32) -> [u8; 36] {
    let mut payload = [0; 36];
    payload[..2].copy_from_slice(&7u16.to_le_bytes());
    payload[4..8].copy_from_slice(&item_id.to_le_bytes());
    payload[12..16].copy_from_slice(&corpse_id.to_le_bytes());
    payload[16..20].copy_from_slice(&quantity.to_le_bytes());
    payload[20..24].copy_from_slice(&sequence.to_le_bytes());
    payload[26..30].copy_from_slice(&coin.to_le_bytes());
    payload
}

#[cfg(feature = "backend-eql")]
fn corpse_coin(coin: u32) -> [u8; 16] {
    let mut payload = [0; 16];
    payload[..2].copy_from_slice(&5u16.to_le_bytes());
    payload[2] = 1;
    payload[3..7].copy_from_slice(&coin.to_le_bytes());
    payload[11] = 1;
    payload
}

fn drop_item(name: &str, item_id: u32, icon: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&item_id.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&icon.to_le_bytes());
    payload.extend_from_slice(name.as_bytes());
    payload.push(0);
    payload.push(0x12);
    let mut header = format!("{item_id:06X}").into_bytes();
    header.resize(197, b'0');
    payload.extend_from_slice(&header);
    payload.extend_from_slice(name.as_bytes());
    payload.push(0x12);
    payload
}

fn drops(corpse_id: u32, corpse_name: &str, items: &[(&str, u32, u32)]) -> Vec<u8> {
    let mut payload = vec![1, 0];
    payload.extend_from_slice(&900u32.to_le_bytes());
    payload.extend_from_slice(&corpse_id.to_le_bytes());
    payload.extend_from_slice(&(items.len() as u32).to_le_bytes());
    payload.extend_from_slice(corpse_name.as_bytes());
    payload.push(0);
    for &(name, item_id, icon) in items {
        payload.extend(drop_item(name, item_id, icon));
    }
    payload
}

#[cfg(feature = "backend-eql")]
fn acquisition(events: &[Event]) -> &LootAcquisition {
    events
        .iter()
        .find_map(|event| match event {
            Event::LootAcquired(acquisition) => Some(acquisition),
            _ => None,
        })
        .expect("semantic loot acquisition")
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_fixture_covers_ordering_dispositions_duplicates_and_boundaries() {
    let base = 0x4100;
    let mut session = session(BackendId::Eql, base);
    let mut enter_world = [0; 72];
    enter_world[..8].copy_from_slice(b"Testchar");
    decode_at(
        &mut session,
        base + ENTER_WORLD,
        Dir::ClientToServer,
        &enter_world,
        1,
    );

    let dropped = message("You looted a Rusty Sword from a goblin's corpse and dropped it.");
    decode_at(
        &mut session,
        base + MESSAGE,
        Dir::ServerToClient,
        &dropped,
        10,
    );
    let events = decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(100, 200, 1, 0, 1),
        11,
    );
    let acquired = acquisition(&events);
    assert_eq!(acquired.timestamp, 10);
    assert_eq!(acquired.disposition, "dropped");
    assert_eq!(acquired.corpse_id, Some(100));
    assert_eq!(acquired.sequence, Some(1));
    assert!(acquired.complete);

    let destroyed = message("You looted a Rusty Mace from an orc's corpse and destroyed it.");
    decode_at(
        &mut session,
        base + MESSAGE,
        Dir::ServerToClient,
        &destroyed,
        12,
    );
    let events = decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(101, 201, 1, 0, 2),
        13,
    );
    assert_eq!(acquisition(&events).disposition, "destroyed");

    let duplicate = decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(101, 201, 1, 0, 2),
        14,
    );
    assert!(duplicate
        .iter()
        .all(|event| !matches!(event, Event::LootAcquired(_))));

    decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(102, 202, 2, 50, 3),
        20,
    );
    let out_of_order =
        message("You looted 2 Bone Chips from a skeleton's corpse and stored it in your bank");
    let events = decode_at(
        &mut session,
        base + MESSAGE,
        Dir::ServerToClient,
        &out_of_order,
        21,
    );
    let acquired = acquisition(&events);
    assert_eq!(acquired.item_id, Some(202));
    assert_eq!(acquired.quantity, 2);
    assert_eq!(acquired.timestamp, 21);
    assert!(acquired.complete);

    let incomplete = message("--You have looted a Fine Steel Sword from a goblin's corpse.--");
    decode_at(
        &mut session,
        base + MESSAGE,
        Dir::ServerToClient,
        &incomplete,
        30,
    );
    let flushed = session.flush(FlushReason::ReplayEnd);
    let acquired = acquisition(&flushed);
    assert_eq!(acquired.timestamp, 30);
    assert!(!acquired.complete);

    decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(103, 203, 1, 0, 4),
        31,
    );
    let flushed = session.flush(FlushReason::Shutdown);
    let acquired = acquisition(&flushed);
    assert_eq!(acquired.item_id, Some(203));
    assert!(!acquired.complete);

    decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &confirmation(104, 204, 1, 0, 5),
        40,
    );
    let reset = session.flush(FlushReason::Reset);
    assert!(matches!(
        reset.as_slice(),
        [Event::LootAcquired(incomplete), Event::SessionReset { reason: SessionResetReason::Explicit }]
            if !incomplete.complete
    ));
    decode_at(
        &mut session,
        base + MESSAGE,
        Dir::ServerToClient,
        &message("--You have looted a Rusty Dagger from a goblin's corpse.--"),
        41,
    );
    let after_reset = session.flush(FlushReason::ReplayEnd);
    let acquired = acquisition(&after_reset);
    assert_eq!(
        acquired.item_id, None,
        "pre-reset confirmation must be gone"
    );
    assert!(!acquired.complete);
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_corpse_fixture_is_timestamped_deduplicated_and_compatible() {
    let base = 0x4200;
    let mut session = session(BackendId::Eql, base);
    let payload = drops(
        500,
        "an ice giant",
        &[("Diamond Dust", 16_884, 1075), ("Silk", 16_885, 1076)],
    );
    let first = decode_at(
        &mut session,
        base + DROPS,
        Dir::ServerToClient,
        &payload,
        1000,
    );
    let snapshot = first
        .iter()
        .find_map(|event| match event {
            Event::CorpseLootSnapshot(snapshot) => Some(snapshot),
            _ => None,
        })
        .expect("semantic corpse snapshot");
    assert_eq!(snapshot.timestamp, 1000);
    assert_eq!(snapshot.corpse_id, 500);
    assert_eq!(snapshot.corpse_name_normalized, "ice giant");
    assert_eq!(snapshot.items.len(), 2);
    assert!(
        matches!(first.first(), Some(Event::LootDrops { .. })),
        "low-level compatibility event stays additive"
    );

    let rows = session.take_loot_rows();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.complete));

    let repeated = decode_at(
        &mut session,
        base + DROPS,
        Dir::ServerToClient,
        &payload,
        1001,
    );
    assert!(repeated
        .iter()
        .all(|event| !matches!(event, Event::CorpseLootSnapshot(_))));
    assert!(session.take_loot_rows().is_empty());

    let emptied = decode_at(
        &mut session,
        base + DROPS,
        Dir::ServerToClient,
        &drops(500, "an ice giant", &[]),
        1002,
    );
    let emptied = emptied
        .iter()
        .find_map(|event| match event {
            Event::CorpseLootSnapshot(snapshot) => Some(snapshot),
            _ => None,
        })
        .expect("changed empty corpse snapshot");
    assert!(emptied.items.is_empty());

    let coin = decode_at(
        &mut session,
        base + TRANSACTION,
        Dir::ServerToClient,
        &corpse_coin(2881),
        1003,
    );
    let acquired = acquisition(&coin);
    assert_eq!(acquired.timestamp, 1003);
    assert_eq!(acquired.coin_copper, 2881);
    assert!(acquired.from_corpse);
    assert!(acquired.complete);
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_fixture_does_not_apply_eql_loot_semantics() {
    assert_non_eql_numeric_fixture(BackendId::Live, 0x5100);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_fixture_does_not_apply_eql_loot_semantics() {
    assert_non_eql_numeric_fixture(BackendId::Test, 0x6100);
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn assert_non_eql_numeric_fixture(backend: BackendId, base: u16) {
    let mut session = session(backend, base);
    for (offset, payload) in [
        (
            MESSAGE,
            message("--You have looted a Sword from a corpse's corpse.--"),
        ),
        (TRANSACTION, confirmation(1, 2, 1, 0, 1).to_vec()),
        (DROPS, drops(1, "a corpse", &[("Sword", 2, 3)])),
    ] {
        let batch = session.decode_at(
            StreamKind::Zone,
            OpcodeId(base + offset),
            Dir::ServerToClient,
            &payload,
            100,
        );
        assert_eq!(batch.disposition, DecodeDisposition::Unhandled);
        assert!(batch.events.is_empty());
    }
    assert!(session.flush(FlushReason::ReplayEnd).is_empty());
}
