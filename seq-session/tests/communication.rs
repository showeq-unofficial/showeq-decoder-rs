use seq_events::{
    ChatMessageKind, DynamicZoneState, Event, GroupRosterState, GuildRosterState, Point3,
    SessionResetReason,
};
use seq_session::{
    BackendId, DecodeDisposition, Dir, OpcodeId, ProtocolRegistry, Session, SessionConfig,
    StreamKind,
};
use std::sync::Arc;

const ENTER_WORLD: u16 = 0;
const CHAT: u16 = 1;
const SIMPLE: u16 = 2;
const FORMATTED: u16 = 3;
const SPECIAL: u16 = 4;
const GROUP_LIST: u16 = 5;
const GROUP_FOLLOW: u16 = 6;
const GROUP_DISBAND: u16 = 7;
const GUILD_ROSTER: u16 = 8;
const GUILD_MOTD: u16 = 9;
const GUILD_RANK: u16 = 10;
#[cfg(any(feature = "backend-live", feature = "backend-test"))]
const GUILD_STATUS: u16 = 11;
const DZ_INFO: u16 = 12;
const DZ_SWITCH: u16 = 13;

fn session(backend: BackendId, base: u16) -> Session {
    let names = [
        "OP_EnterWorld",
        "OP_CommonMessage",
        "OP_SimpleMessage",
        "OP_FormattedMessage",
        "OP_SpecialMesg",
        "OP_GroupMemberList",
        "OP_GroupFollow",
        "OP_GroupDisband",
        "OP_GuildMemberList",
        "OP_GuildMOTD",
        "OP_ExpandedGuildInfo",
        "OP_GuildMemberUpdate",
        "OP_DzInfo",
        "OP_DzSwitchInfo",
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

fn decode(
    session: &mut Session,
    base: u16,
    offset: u16,
    direction: Dir,
    payload: &[u8],
) -> Vec<Event> {
    let batch = session.decode(
        StreamKind::Zone,
        OpcodeId(base + offset),
        direction,
        payload,
    );
    assert_eq!(batch.disposition, DecodeDisposition::Decoded);
    batch.events
}

fn enter_world(name: &str) -> [u8; 72] {
    let mut payload = [0; 72];
    payload[..name.len()].copy_from_slice(name.as_bytes());
    payload
}

fn common_message(sender: &[u8], target: &[u8], text: &[u8], channel: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(sender);
    payload.push(0);
    payload.extend_from_slice(target);
    payload.push(0);
    payload.extend_from_slice(&[0; 8]);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&channel.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.push(0);
    payload.extend_from_slice(&100u32.to_le_bytes());
    payload.extend_from_slice(text);
    payload.push(0);
    payload
}

fn formatted(format_id: u32, color: u32, args: &[&[u8]]) -> Vec<u8> {
    let mut payload = vec![0; 13];
    payload[5..9].copy_from_slice(&format_id.to_le_bytes());
    payload[9..13].copy_from_slice(&color.to_le_bytes());
    for arg in args {
        payload.extend_from_slice(&(arg.len() as u32).to_le_bytes());
        payload.extend_from_slice(arg);
    }
    payload
}

fn special(source: &[u8], text: &[u8], color: u32) -> Vec<u8> {
    let mut payload = vec![0; 11];
    payload[3..7].copy_from_slice(&color.to_le_bytes());
    payload.extend_from_slice(source);
    payload.push(0);
    payload.extend_from_slice(&[0; 12]);
    payload.extend_from_slice(text);
    payload.push(0);
    payload
}

fn group_list(group_id: u32, count: u32, names: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&group_id.to_le_bytes());
    payload.extend_from_slice(&count.to_le_bytes());
    for name in names {
        payload.extend_from_slice(name.as_bytes());
        payload.push(0);
    }
    payload
}

fn group_follow(backend: BackendId, name: &str, level: u32) -> Vec<u8> {
    let mut payload = vec![0; if backend == BackendId::Eql { 168 } else { 152 }];
    if backend == BackendId::Eql {
        payload[64..64 + name.len()].copy_from_slice(name.as_bytes());
        payload[132..136].copy_from_slice(&level.to_le_bytes());
    } else {
        payload[..name.len()].copy_from_slice(name.as_bytes());
    }
    payload
}

fn group_disband(backend: BackendId, your_name: &str, member_name: &str) -> Vec<u8> {
    let mut payload = vec![0; if backend == BackendId::Eql { 168 } else { 152 }];
    payload[..your_name.len()].copy_from_slice(your_name.as_bytes());
    payload[64..64 + member_name.len()].copy_from_slice(member_name.as_bytes());
    payload
}

fn dz_info(active: bool) -> Vec<u8> {
    let mut payload = vec![0; 212];
    payload[8] = u8::from(active);
    payload[12..16].copy_from_slice(&6u32.to_le_bytes());
    payload[16..26].copy_from_slice(b"Plane Raid");
    payload[144..149].copy_from_slice(b"Alice");
    payload
}

fn dz_switch() -> [u8; 32] {
    let mut payload = [0; 32];
    payload[8..10].copy_from_slice(&42u16.to_le_bytes());
    payload[10..12].copy_from_slice(&7u16.to_le_bytes());
    payload[12..16].copy_from_slice(&5u32.to_le_bytes());
    payload[20..24].copy_from_slice(&2.5f32.to_le_bytes());
    payload[24..28].copy_from_slice(&1.5f32.to_le_bytes());
    payload[28..32].copy_from_slice(&(-3.5f32).to_le_bytes());
    payload
}

fn lp(payload: &mut Vec<u8>, text: &str) {
    payload.extend_from_slice(&(text.len() as u32).to_le_bytes());
    payload.extend_from_slice(text.as_bytes());
}

fn live_guild_roster() -> Vec<u8> {
    let mut payload = Vec::new();
    lp(&mut payload, "Hero");
    payload.extend_from_slice(&15u32.to_le_bytes());
    payload.extend_from_slice(&180u32.to_le_bytes());
    payload.extend_from_slice(&[0; 2]);
    payload.extend_from_slice(&1u32.to_le_bytes());
    lp(&mut payload, "Alice");
    payload.extend_from_slice(&60u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&2u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&1234u32.to_le_bytes());
    payload.extend_from_slice(&[0; 2]);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.push(1);
    lp(&mut payload, "Main");
    payload.extend_from_slice(&[0; 6]);
    payload
}

fn eql_guild_roster() -> Vec<u8> {
    let mut payload = Vec::new();
    lp(&mut payload, "Hero");
    payload.extend_from_slice(&15u32.to_le_bytes());
    payload.extend_from_slice(&180u32.to_le_bytes());
    payload.extend_from_slice(&[0; 2]);
    payload.extend_from_slice(&1u32.to_le_bytes());
    lp(&mut payload, "Alice");
    payload.extend_from_slice(&60u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&0b110u32.to_le_bytes());
    payload.extend_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&1234u32.to_le_bytes());
    payload.extend_from_slice(&[0; 2]);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.push(1);
    lp(&mut payload, "Main");
    payload.extend_from_slice(&9u16.to_le_bytes());
    payload.extend_from_slice(&[0; 4]);
    payload
}

fn guild_roster(backend: BackendId) -> Vec<u8> {
    if backend == BackendId::Eql {
        eql_guild_roster()
    } else {
        live_guild_roster()
    }
}

fn guild_motd() -> Vec<u8> {
    let mut payload = vec![0; 656];
    payload[72..78].copy_from_slice(b"Setter");
    payload[140..149].copy_from_slice(b"Raid at 8");
    payload
}

fn guild_rank() -> Vec<u8> {
    let mut payload = vec![0; 192];
    payload[..4].copy_from_slice(&3u32.to_le_bytes());
    payload[8..12].copy_from_slice(&15u32.to_le_bytes());
    payload[88..92].copy_from_slice(&3u32.to_le_bytes());
    payload[92..99].copy_from_slice(b"Officer");
    payload
}

#[cfg(any(feature = "backend-live", feature = "backend-test"))]
fn guild_status() -> Vec<u8> {
    let mut payload = vec![0; 88];
    payload[8..13].copy_from_slice(b"Alice");
    payload[72..74].copy_from_slice(&22u16.to_le_bytes());
    payload[74..76].copy_from_slice(&2u16.to_le_bytes());
    payload[76..80].copy_from_slice(&5678u32.to_le_bytes());
    payload
}

fn assert_text_group_and_dz(backend: BackendId, base: u16) {
    let mut session = session(backend, base);
    decode(
        &mut session,
        base,
        ENTER_WORLD,
        Dir::ClientToServer,
        &enter_world("Hero"),
    );

    let chat = decode(
        &mut session,
        base,
        CHAT,
        Dir::ServerToClient,
        &common_message(b"Andr\xe9", b"", b"caf\xe9", 8),
    );
    assert!(matches!(
        chat.as_slice(),
        [Event::Chat { .. }, Event::ChatMessage(message)]
            if message.kind == ChatMessageKind::Common
                && message.from == "Andr\u{e9}"
                && message.text == "caf\u{e9}"
    ));

    let client_say = decode(
        &mut session,
        base,
        CHAT,
        Dir::ClientToServer,
        &common_message(b"Hero", b"", b"outbound", 8),
    );
    assert!(matches!(client_say.as_slice(), [Event::Chat { text, .. }] if text == "outbound"));
    assert!(
        client_say
            .iter()
            .all(|event| !matches!(event, Event::ChatMessage(_))),
        "client-side OP_CommonMessage must remain compatibility-only"
    );

    let server_say = decode(
        &mut session,
        base,
        CHAT,
        Dir::ServerToClient,
        &common_message(b"Hero", b"", b"authoritative", 8),
    );
    assert!(matches!(
        server_say.as_slice(),
        [Event::Chat { text, .. }, Event::ChatMessage(message)]
            if text == "authoritative" && message.text == "authoritative"
    ));

    let mut simple = [0; 12];
    simple[..4].copy_from_slice(&123u32.to_le_bytes());
    simple[4..8].copy_from_slice(&259u32.to_le_bytes());
    let messages = decode(&mut session, base, SIMPLE, Dir::ServerToClient, &simple);
    assert!(matches!(
        messages.as_slice(),
        [Event::SimpleMessage { .. }, Event::ChatMessage(message)]
            if message.kind == ChatMessageKind::Simple
                && message.channel == 0
                && message.format_id == Some(123)
    ));

    let messages = decode(
        &mut session,
        base,
        FORMATTED,
        Dir::ServerToClient,
        &formatted(456, 259, &[b"caf\xe9"]),
    );
    assert!(matches!(
        messages.as_slice(),
        [Event::FormattedMessage { .. }, Event::ChatMessage(message)]
            if message.args == ["caf\u{e9}"]
                && message.channel == if backend == BackendId::Eql { 19 } else { 0 }
    ));

    let messages = decode(
        &mut session,
        base,
        SPECIAL,
        Dir::ServerToClient,
        &special(b"Ren\xe9", b"voil\xe0", 256),
    );
    assert!(matches!(
        messages.as_slice(),
        [Event::SpecialMessage { .. }, Event::ChatMessage(message)]
            if message.from == "Ren\u{e9}" && message.text == "voil\u{e0}"
    ));

    let partial = decode(
        &mut session,
        base,
        GROUP_LIST,
        Dir::ServerToClient,
        &group_list(77, 3, &["Hero", "Alice"]),
    );
    assert!(matches!(
        partial.as_slice(),
        [Event::GroupRosterWire { complete: false, .. }, Event::GroupRosterUpdated(GroupRosterState { complete: false, members, .. })]
            if members.iter().map(|member| member.name.as_str()).collect::<Vec<_>>() == ["Alice"]
    ));
    let followed = decode(
        &mut session,
        base,
        GROUP_FOLLOW,
        Dir::ServerToClient,
        &group_follow(backend, "Bob", 55),
    );
    assert!(matches!(
        followed.as_slice(),
        [Event::GroupFollow { .. }, Event::GroupRosterUpdated(GroupRosterState { complete: false, members, .. })]
            if members.len() == 2
    ));
    let complete = decode(
        &mut session,
        base,
        GROUP_LIST,
        Dir::ServerToClient,
        &group_list(77, 3, &["Hero", "Alice", "Bob"]),
    );
    assert!(matches!(
        complete.as_slice(),
        [Event::GroupRosterWire { complete: true, .. }, Event::GroupRosterUpdated(GroupRosterState { group_id: Some(77), complete: true, members })]
            if members.len() == 2 && members[0].slot == 0 && members[1].slot == 1
    ));

    let left = decode(
        &mut session,
        base,
        GROUP_DISBAND,
        Dir::ServerToClient,
        &group_disband(backend, "Hero", "Alice"),
    );
    assert!(matches!(
        left.as_slice(),
        [Event::GroupDisband { .. }, Event::GroupRosterUpdated(GroupRosterState { members, .. })]
            if members.len() == 1 && members[0].name == "Bob" && members[0].slot == 1
    ));

    let first_dz = decode(
        &mut session,
        base,
        DZ_INFO,
        Dir::ServerToClient,
        &dz_info(true),
    );
    assert!(matches!(
        first_dz.as_slice(),
        [
            Event::DynamicZoneInfo { .. },
            Event::DynamicZoneUpdated(DynamicZoneState {
                active: true,
                complete: false,
                max_players: Some(6),
                ..
            })
        ]
    ));
    let second_dz = decode(
        &mut session,
        base,
        DZ_SWITCH,
        Dir::ServerToClient,
        &dz_switch(),
    );
    assert!(matches!(
        second_dz.as_slice(),
        [Event::DynamicZoneSwitch { .. }, Event::DynamicZoneUpdated(DynamicZoneState { active: true, complete: true, zone_id: Some(42), instance_id: Some(7), position: Some(Point3 { x, y, z }), .. })]
            if *x == 1.5 && *y == 2.5 && *z == -3.5
    ));

    let roster = decode(
        &mut session,
        base,
        GUILD_ROSTER,
        Dir::ServerToClient,
        &guild_roster(backend),
    );
    assert!(matches!(
        roster.as_slice(),
        [Event::GuildRoster { .. }, Event::GuildRosterWire { complete: true, .. }, Event::GuildRosterUpdated(GuildRosterState { guild_id: 15, complete: true, members })]
            if members.len() == 1 && members[0].name == "Alice"
    ));
    let motd = decode(
        &mut session,
        base,
        GUILD_MOTD,
        Dir::ServerToClient,
        &guild_motd(),
    );
    assert!(matches!(
        motd.as_slice(),
        [Event::GuildMotd { .. }, Event::GuildMotdUpdated(state)]
            if state.guild_id == 15 && state.sender == "Setter" && state.message == "Raid at 8"
    ));
    let rank = decode(
        &mut session,
        base,
        GUILD_RANK,
        Dir::ServerToClient,
        &guild_rank(),
    );
    assert!(matches!(
        rank.as_slice(),
        [Event::GuildRankName { .. }, Event::GuildRankNamesUpdated(state)]
            if state.guild_id == 15 && state.ranks.len() == 1 && state.ranks[0].rank_name == "Officer"
    ));

    #[cfg(any(feature = "backend-live", feature = "backend-test"))]
    {
        if backend != BackendId::Eql {
            let status = decode(
                &mut session,
                base,
                GUILD_STATUS,
                Dir::ServerToClient,
                &guild_status(),
            );
            assert!(matches!(
                status.as_slice(),
                [Event::GuildMemberStatus { instance_id: 2, .. }, Event::GuildRosterUpdated(GuildRosterState { members, .. })]
                    if members[0].zone_id == 22 && members[0].last_on == 5678
            ));
        }
    }

    let reconnect = decode(
        &mut session,
        base,
        ENTER_WORLD,
        Dir::ClientToServer,
        &enter_world("Other"),
    );
    assert!(matches!(
        reconnect.as_slice(),
        [Event::GroupRosterUpdated(GroupRosterState { complete: false, members, .. }), Event::GuildRosterUpdated(GuildRosterState { guild_id: 0, complete: false, members: guild_members }), Event::DynamicZoneUpdated(DynamicZoneState { active: false, complete: true, .. }), Event::SessionReset { reason: SessionResetReason::EnterWorld }, Event::EnterWorld { character_name }]
            if members.is_empty() && guild_members.is_empty() && character_name == "Other"
    ));

    let malformed = session.decode(
        StreamKind::Zone,
        OpcodeId(base + GROUP_LIST),
        Dir::ServerToClient,
        &[0; 7],
    );
    assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
    assert!(malformed.events.is_empty());

    let invalid_count = session.decode(
        StreamKind::Zone,
        OpcodeId(base + GROUP_LIST),
        Dir::ServerToClient,
        &group_list(77, 7, &["Hero", "Alice"]),
    );
    assert_eq!(invalid_count.disposition, DecodeDisposition::Ignored);
    assert!(invalid_count.events.is_empty());
}

#[cfg(feature = "backend-live")]
#[test]
fn live_numeric_text_group_and_dynamic_zone() {
    assert_text_group_and_dz(BackendId::Live, 0x6100);
}

#[cfg(feature = "backend-test")]
#[test]
fn test_numeric_text_group_and_dynamic_zone() {
    assert_text_group_and_dz(BackendId::Test, 0x6200);
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_numeric_text_group_and_dynamic_zone() {
    assert_text_group_and_dz(BackendId::Eql, 0x6300);
}

#[cfg(feature = "backend-eql")]
fn encode_ucs(plain: &[u8]) -> Vec<u8> {
    let mut encoded = plain.to_vec();
    for index in 4..plain.len() {
        encoded[index] = plain[index] ^ encoded[index - 4];
    }
    encoded
}

#[cfg(feature = "backend-eql")]
fn ucs_record(channel: &[u8], sender: &[u8], message: &[u8], spam: u8) -> Vec<u8> {
    let mut plain = vec![1, 2, 3, 4];
    plain.extend_from_slice(channel);
    plain.push(0);
    plain.extend_from_slice(b"Server.");
    plain.extend_from_slice(sender);
    plain.push(0);
    plain.extend_from_slice(message);
    plain.push(0);
    plain.extend_from_slice(format!("SPAM:{spam}:0").as_bytes());
    plain.push(0);
    encode_ucs(&plain)
}

#[cfg(feature = "backend-eql")]
#[test]
fn eql_ucs_uses_session_channel_state_and_reset() {
    let mut session = session(BackendId::Eql, 0x6400);
    let first = session.decode_ucs(
        Dir::ServerToClient,
        &ucs_record(b"General", b"Andr\xe9", b"caf\xe9", 0),
    );
    assert_eq!(first.disposition, DecodeDisposition::Decoded);
    assert!(matches!(
        first.events.as_slice(),
        [Event::UcsRecord { .. }, Event::ChatMessage(message)]
            if message.kind == ChatMessageKind::Ucs
                && message.channel_name == "General"
                && message.from == "Andr\u{e9}"
                && message.text == "caf\u{e9}"
    ));

    let spam = session.decode_ucs(
        Dir::ServerToClient,
        &ucs_record(b"General", b"Bot", b"buy now", 7),
    );
    assert!(matches!(
        spam.events.as_slice(),
        [Event::UcsRecord { spam: true, .. }, Event::ChatMessage(message)]
            if message.text == "(SPAM) buy now" && message.channel_name == "General"
    ));

    let outbound = session.decode_ucs(Dir::ClientToServer, &[0; 20]);
    assert_eq!(outbound.disposition, DecodeDisposition::Ignored);
    let malformed = session.decode_ucs(Dir::ServerToClient, &[0; 11]);
    assert_eq!(malformed.disposition, DecodeDisposition::Malformed);
}
