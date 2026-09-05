//! eql implementation of the neutral [`seq_events::Backend`] contract.
//!
//! Maps this crate's self-contained EQ Legends parsers into the neutral event
//! vocabulary. Depends only on `seq-events` (pure vocabulary, no Live decode
//! code), so it does not breach eql's isolation — no Live wire parser reaches
//! eql through it.
//!
//! Field/heading math mirrors the scry NIF exactly (eql spawn heading is 11-bit
//! h2048, self-pos is 13-bit, mob/npc updates are 12-bit like Live) so decoded
//! output stays byte-for-byte identical across the migration.

use seq_events::{
    heading_deg, Backend, BuffEntry, Decoded, Dir, DoorInfo, Event, GroundItemInfo,
    GuildRosterMember, ItemTemplate, LootItemInfo, Point3, Pos, ProfileInfo, SpawnInfo, Velocity,
    ZoneEnvironment, ZoneInfo, ZonePointInfo,
};

/// The EverQuest Legends backend (this crate's own parsers).
pub struct EqlBackend;

impl Backend for EqlBackend {
    fn name(&self) -> &'static str {
        "eql"
    }

    fn decode(&self, opcode: &str, dir: Dir, bytes: &[u8]) -> Decoded {
        // Server-message opcodes are server→client only; the client's own
        // outgoing sends echo back S→C, so decoding the C→S copy would
        // double-display (the daemon's DIR_Client guard). OP_CommonMessage is
        // filtered per-channel instead (see `chat`) — Say isn't echoed.
        if dir == Dir::ClientToServer
            && matches!(
                opcode,
                "OP_SimpleMessage" | "OP_FormattedMessage" | "OP_SpecialMesg" | "OP_LootMessage"
            )
        {
            return Decoded::Ignored;
        }

        match opcode {
            "OP_ZoneEntry" if dir == Dir::ServerToClient => spawn(bytes),
            "OP_ZoneEntry" => Decoded::Ignored,
            "OP_MobUpdate" if dir == Dir::ServerToClient => mob_update(bytes),
            "OP_MobUpdate" => Decoded::Ignored,
            "OP_NpcMoveUpdate" if dir == Dir::ServerToClient => npc_move_update(bytes),
            "OP_NpcMoveUpdate" => Decoded::Ignored,
            "OP_RemoveSpawn" => remove_spawn(bytes),
            "OP_DeleteSpawn" => delete_spawn(bytes),
            "OP_SpawnRename" if dir == Dir::ServerToClient => spawn_rename(bytes),
            "OP_SpawnRename" => Decoded::Ignored,
            "OP_Death" => death(bytes),
            "OP_HPUpdate" => hp_update(bytes),
            "OP_NewZone" if dir == Dir::ServerToClient => new_zone(bytes),
            "OP_NewZone" => Decoded::Ignored,
            "OP_GuildsInZoneList" => guilds_in_zone_list(bytes),
            "OP_NewGuildInZone" => new_guild_in_zone(bytes),
            "OP_PlayerProfile" if dir == Dir::ServerToClient => player_profile(bytes),
            "OP_PlayerProfile" => Decoded::Ignored,
            "OP_ZoneChange" if dir == Dir::ClientToServer => zone_change(bytes),
            "OP_ZoneChange" => Decoded::Ignored,
            "OP_LoadoutSwap" => loadout_swap(bytes),
            "OP_ClickObject" => click_object(dir, bytes),
            // eql's appearance event is the stock opcode carrying the WIDENED
            // 24-byte struct (upstream calls it spawnEventEQLStruct), so both
            // names reach the same decoder. Only OP_SpawnAppearance is mapped on
            // the current patch; before this, 26530 packets a capture arrived
            // under a name with no arm and were dropped outright.
            "OP_SpawnAppearance" | "OP_SpawnAppearance2" => spawn_appearance2(bytes),
            "OP_TimeOfDay" if dir == Dir::ServerToClient => time_of_day(bytes),
            "OP_TimeOfDay" => Decoded::Ignored,
            "OP_Stance" => stance(bytes),
            "OP_Invocation" => invocation(bytes),
            "OP_GuildMemberList" => guild_roster(bytes),
            "OP_ItemPacket" => item_packet(bytes),
            "OP_ZoneServerInfo" if dir == Dir::ServerToClient => zone_server_info(bytes),
            "OP_ZoneServerInfo" => Decoded::Ignored,
            "OP_GuildMOTD" => guild_motd(bytes),
            "OP_ExpandedGuildInfo" => expanded_guild_info(bytes),
            "OP_InspectAnswer" => inspect_answer(bytes),
            "OP_ClientUpdate" => self_pos(bytes),
            "OP_SelfPos" => self_pos_breadcrumb(bytes),
            "OP_Illusion" => illusion(bytes),
            "OP_Action" => action(bytes),
            "OP_Action2" => action2(bytes),
            "OP_BeginCast" if dir == Dir::ServerToClient => begin_cast(bytes),
            "OP_BeginCast" => Decoded::Ignored,
            "OP_CastSpell" if dir == Dir::ClientToServer => cast_spell(bytes),
            "OP_CastSpell" => Decoded::Ignored,
            "OP_Buff" if dir == Dir::ServerToClient => buff(bytes),
            "OP_Buff" => Decoded::Ignored,
            "OP_TargetMouse" => target(bytes),
            "OP_Consider" => consider(bytes),
            "OP_CommonMessage" => chat(bytes, dir),
            "OP_SimpleMessage" => simple_message(bytes),
            "OP_FormattedMessage" => formatted_message(bytes),
            "OP_SpecialMesg" => special_message(bytes),
            "OP_LootMessage" => loot_message(bytes),
            "OP_ExpUpdate" => exp(bytes),
            "OP_LevelUpdate" => level_update(bytes),
            "OP_AAExpUpdate" => aa_exp(bytes),
            "OP_ManaChange" => mana_change(bytes),
            "OP_Stamina" => stamina(bytes),
            "OP_SkillUpdate" => skill_update(bytes),
            "OP_LootTransaction" => loot_transaction(bytes),
            "OP_LootDrops" => loot_drops(bytes),
            "OP_MoneyUpdate" => money(bytes),
            "OP_SendAATable" => aa_table(bytes),
            "OP_BuffList" | "OP_BuffList2" | "OP_BuffList3" => buff_list(bytes),
            "OP_GroundSpawn" if dir == Dir::ServerToClient => ground_item(bytes),
            "OP_GroundSpawn" => Decoded::Ignored,
            "OP_SpawnDoor" if dir == Dir::ServerToClient => doors(bytes),
            "OP_SpawnDoor" => Decoded::Ignored,
            "OP_CorpseLocResponse" if dir == Dir::ServerToClient => corpse_location(bytes),
            "OP_CorpseLocResponse" => Decoded::Ignored,
            "OP_SendZonePoints" if dir == Dir::ServerToClient => zone_points(bytes),
            "OP_SendZonePoints" => Decoded::Ignored,
            "OP_GroupFollow" => group_follow(bytes),
            "OP_GroupDisband" | "OP_GroupDisband2" => group_disband(bytes),
            "OP_GroupMemberList" if dir == Dir::ServerToClient => group_member_list(bytes),
            "OP_GroupMemberList" => Decoded::Ignored,
            // OP_GroupUpdate is a fixed-168B status push (no roster); noop.
            "OP_GroupUpdate" => Decoded::Ignored,
            "OP_DzInfo" if dir == Dir::ServerToClient => dynamic_zone_info(bytes),
            "OP_DzInfo" => Decoded::Ignored,
            "OP_DzSwitchInfo" if dir == Dir::ServerToClient => dynamic_zone_switch(bytes),
            "OP_DzSwitchInfo" => Decoded::Ignored,
            "OP_EnterWorld" if dir == Dir::ClientToServer => enter_world(bytes),
            "OP_EnterWorld" => Decoded::Ignored,
            _ => Decoded::Unhandled,
        }
    }
}

fn spawn(bytes: &[u8]) -> Decoded {
    match crate::parse_spawn(bytes) {
        Ok(s) => Decoded::One(spawn_event(&s)),
        Err(_) => Decoded::Malformed,
    }
}

/// One decoded eql spawn record -> the neutral SpawnAdded event. Shared by
/// OP_ZoneEntry and OP_LoadoutSwap, whose embedded record is byte-identical.
fn spawn_event(s: &crate::ZoneSpawn) -> Event {
    Event::SpawnAdded(SpawnInfo {
        id: u32::from(s.id),
        name: s.name.clone(),
        last_name: s.last_name.clone(),
        race: s.race,
        class_: s.class_,
        deity: s.deity,
        level: s.level,
        npc: s.npc,
        cur_hp: u32::from(s.cur_hp),
        max_hp: Some(u32::from(s.max_hp)),
        guild_id: s.guild_id,
        guild_server_id: s.guild_server_id,
        class_mask: s.class_mask,
        // eql spawn carries position inline; heading is h2048 (11-bit).
        pos: Some(Pos {
            x: i32::from(s.x),
            y: i32::from(s.y),
            z: i32::from(s.z),
            heading_deg: heading_deg(s.heading, 11),
        }),
        velocity: Velocity::default(),
        delta_heading: None,
        animation: None,
        equipment_models: None,
    })
}

fn mob_update(bytes: &[u8]) -> Decoded {
    match crate::mob_update::parse_mob_update(bytes) {
        Ok(s) => Decoded::One(Event::SpawnMoved {
            id: u32::from(s.spawn_id),
            pos: Pos {
                x: s.x,
                y: s.y,
                z: s.z,
                heading_deg: heading_deg(s.heading, 12),
            },
            velocity: Velocity::default(),
            delta_heading: None,
            animation: None,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn npc_move_update(bytes: &[u8]) -> Decoded {
    match crate::npc_move_update::parse_npc_move_update(bytes) {
        Ok(s) => Decoded::One(Event::SpawnMoved {
            id: u32::from(s.spawn_id),
            pos: Pos {
                x: i32::from(s.x),
                y: i32::from(s.y),
                z: i32::from(s.z),
                heading_deg: heading_deg(s.heading as u16, 12),
            },
            velocity: Velocity {
                x: s.has_delta_x.then_some(i32::from(s.delta_x)),
                y: s.has_delta_y.then_some(i32::from(s.delta_y)),
                z: s.has_delta_z.then_some(i32::from(s.delta_z)),
            },
            delta_heading: s.has_delta_heading.then_some(i16::from(s.delta_heading)),
            animation: s.has_animation.then_some(s.animation),
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn remove_spawn(bytes: &[u8]) -> Decoded {
    match crate::remove_spawn::parse_remove_spawn(bytes) {
        Ok(s) => Decoded::One(Event::SpawnRemoved { id: s.spawn_id }),
        Err(_) => Decoded::Malformed,
    }
}

fn delete_spawn(bytes: &[u8]) -> Decoded {
    match crate::delete_spawn::parse_delete_spawn(bytes) {
        Ok(s) => Decoded::One(Event::SpawnRemoved { id: s.spawn_id }),
        Err(_) => Decoded::Malformed,
    }
}

fn spawn_rename(bytes: &[u8]) -> Decoded {
    match crate::spawn_rename::parse_spawn_rename(bytes) {
        Ok(rename)
            if !rename.old_name.is_empty()
                && rename.old_name == rename.old_name_again
                && !rename.new_name.is_empty() =>
        {
            Decoded::One(Event::SpawnRenamed {
                id: None,
                old_name: rename.old_name,
                new_name: rename.new_name,
            })
        }
        Ok(_) | Err(_) => Decoded::Malformed,
    }
}

// OP_Death (newCorpseStruct): the deceased becomes a corpse, not a removal.
// seq-session resolves player ownership; direct backend callers retain this
// low-level result during migration.
fn death(bytes: &[u8]) -> Decoded {
    match crate::death::parse_death(bytes) {
        Ok(d) => Decoded::One(Event::SpawnKilled {
            deceased_id: d.spawn_id,
            killer_id: d.killer_id,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// eql OP_HPUpdate is the multiplexed stat-sync channel: spawn HP (real for the
// self, percent for others) plus the player's mana/endurance, all in one packet.
// Surface it whole as StatSync and let seq-session split self from other.
// Emitting one event per packet
// (rather than one per stat) is deliberate: it keeps a single wire packet from
// fanning out into several near-identical player snapshots downstream.
fn hp_update(bytes: &[u8]) -> Decoded {
    match crate::parse_stat_sync(bytes) {
        // The keepalive (flags 0x31, no stat bits) carries nothing to report.
        Ok(s) if !s.has_hp && !s.has_mana && !s.has_end => Decoded::Ignored,
        Ok(s) => Decoded::One(Event::StatSync {
            spawn_id: s.spawn_id,
            wide: s.wide,
            has_hp: s.has_hp,
            hp_cur: s.hp_cur as i32,
            hp_max: s.hp_max as i32,
            has_mana: s.has_mana,
            mana_cur: s.mana_cur as i32,
            mana_max: s.mana_max as i32,
            has_end: s.has_end,
            end_cur: s.end_cur as i32,
            end_max: s.end_max as i32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// Both guild-in-zone opcodes resolve guild ids to names and differ only in
// cardinality, so a single-guild arrival is just a one-element list.
fn guilds_in_zone_list(bytes: &[u8]) -> Decoded {
    match crate::guild_in_zone::parse_guilds_in_zone_list(bytes) {
        // An empty list is normal (an unguilded zone) and carries nothing.
        Ok(guilds) if guilds.is_empty() => Decoded::Ignored,
        Ok(guilds) => Decoded::One(Event::GuildsInZone { guilds }),
        Err(_) => Decoded::Malformed,
    }
}

fn new_guild_in_zone(bytes: &[u8]) -> Decoded {
    match crate::guild_in_zone::parse_new_guild_in_zone(bytes) {
        Ok(g) => Decoded::One(Event::GuildsInZone { guilds: vec![g] }),
        Err(_) => Decoded::Malformed,
    }
}

fn self_pos(bytes: &[u8]) -> Decoded {
    match crate::player_self_pos::parse_player_self_pos(bytes) {
        // eql self heading is an 11-bit COMPASS value (2048 per circle, 0 = N,
        // increasing clockwise), so it converts straight to degrees — unlike the
        // spawn headings above it is NOT inverted. Field boundaries from
        // upstream's struct, sense calibrated against travel direction; see
        // player_self_pos::HEADING_UNITS.
        Ok(s) => Decoded::One(Event::SelfPos {
            pos: Pos {
                x: s.x.round() as i32,
                y: s.y.round() as i32,
                z: s.z.round() as i32,
                heading_deg: heading_deg(s.heading, 11),
            },
            // The phantom twin's id (see player_self_pos) — the host feeds it
            // to SelfTracker, which is the only thing allowed to act on it.
            spawn_id: u32::from(s.spawn_id),
            velocity: Velocity::default(),
            delta_heading: None,
            animation: None,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_SelfPos = the eql self-pos breadcrumb (a position-history trail, N×17B).
// Wired but INERT: it decodes (so the path is live and validated) yet emits
// nothing — the trail is redundant with the OP_ClientUpdate self-pos and carries
// no heading. Return `One(Event::SelfPos ...)` from the last point here if we
// ever surface the trail.
fn self_pos_breadcrumb(bytes: &[u8]) -> Decoded {
    let _ = crate::self_pos_breadcrumb::parse_self_pos_breadcrumb(bytes);
    Decoded::Ignored
}

// OP_TargetMouse = target select (byte-identical to Live's clientTargetStruct).
fn target(bytes: &[u8]) -> Decoded {
    match crate::client_target::parse_client_target(bytes) {
        Ok(t) => Decoded::One(Event::Targeted {
            spawn_id: t.new_target,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_Consider = con result; the considered spawn is the target (eql's own 24B).
fn consider(bytes: &[u8]) -> Decoded {
    match crate::consider::parse_consider(bytes) {
        Ok(c) => Decoded::One(Event::Considered {
            spawn_id: c.target_id,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_CommonMessage = player chat; keep only the player channels (drop system
// noise), matching MessageShell::channelMessage.
fn chat(bytes: &[u8], dir: Dir) -> Decoded {
    match crate::channel_message::parse_channel_message(bytes) {
        // The server echoes tells/group/guild/etc. back, so drop the C→S copy of
        // those (matches MessageShell::channelMessage); Say is not echoed — keep
        // its C→S copy.
        Ok(c) if dir == Dir::ClientToServer && is_echoed_channel(c.chan_num) => Decoded::Ignored,
        Ok(c) if is_player_channel(c.chan_num) => Decoded::One(Event::Chat {
            channel: c.chan_num,
            from: c.sender,
            target: c.target,
            text: c.message,
            chat_color: 0,
            channel_name: String::new(),
        }),
        Ok(_) => Decoded::Ignored,
        Err(_) => Decoded::Malformed,
    }
}

// OP_SimpleMessage: a string-id message + colour (the consumer resolves text
// from the eqstr DB).
fn simple_message(bytes: &[u8]) -> Decoded {
    match crate::simple_message::parse_simple_message(bytes) {
        Ok(m) => Decoded::One(Event::SimpleMessage {
            format_id: m.message_format,
            color: m.message_color,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_FormattedMessage: eqstr template id + positional args + colour.
fn formatted_message(bytes: &[u8]) -> Decoded {
    match crate::formatted_message::parse_formatted_message(bytes) {
        Ok(m) => Decoded::One(Event::FormattedMessage {
            format_id: m.format_id,
            color: m.msg_color,
            args: m.args,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_SpecialMesg: message text carried inline + sender + target spawn id.
fn special_message(bytes: &[u8]) -> Decoded {
    match crate::special_message::parse_special_message(bytes) {
        Ok(m) => Decoded::One(Event::SpecialMessage {
            color: m.message_color,
            target: u32::from(m.target),
            source: m.source,
            message: m.message,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_GroupFollow: one member joined (invitee name @64, level @132).
fn group_follow(bytes: &[u8]) -> Decoded {
    match crate::group_follow::parse_group_follow(bytes) {
        Ok(g) => Decoded::One(Event::GroupFollow {
            name: g.name,
            level: g.level,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_GroupDisband / OP_GroupDisband2: a member left (self = whole disband).
fn group_disband(bytes: &[u8]) -> Decoded {
    match crate::group_disband::parse_group_disband(bytes) {
        Ok(g) => Decoded::One(Event::GroupDisband {
            yourname: g.yourname,
            membername: g.membername,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn group_member_list(bytes: &[u8]) -> Decoded {
    match crate::group_member_list::parse_group_member_list(bytes) {
        Ok(roster) if roster.member_count == 0 || roster.member_count > 6 => Decoded::Ignored,
        Ok(roster) => {
            let unique: std::collections::BTreeSet<_> = roster
                .names
                .iter()
                .map(String::as_str)
                .filter(|name| !name.is_empty())
                .collect();
            let complete = unique.len() >= roster.member_count as usize;
            Decoded::One(Event::GroupRosterWire {
                group_id: roster.group_id,
                member_count: roster.member_count,
                names: roster.names,
                complete,
            })
        }
        Err(_) => Decoded::Malformed,
    }
}

fn dynamic_zone_info(bytes: &[u8]) -> Decoded {
    match crate::dz_info::parse_dz_info(bytes) {
        Ok(info) => Decoded::One(Event::DynamicZoneInfo {
            active: info.new_dz != 0,
            max_players: info.max_players,
            expedition_name: info.dz_name,
            leader_name: info.name,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn dynamic_zone_switch(bytes: &[u8]) -> Decoded {
    if bytes.len() == 8 {
        return Decoded::One(Event::DynamicZoneSwitch {
            active: false,
            zone_id: None,
            instance_id: None,
            kind: None,
            position: None,
        });
    }
    match crate::dz_switch_info::parse_dz_switch_info(bytes) {
        Ok(switch) => Decoded::One(Event::DynamicZoneSwitch {
            active: true,
            zone_id: Some(switch.zone_id),
            instance_id: Some(switch.instance_id),
            kind: Some(switch.kind),
            position: Some(Point3 {
                x: switch.x,
                y: switch.y,
                z: switch.z,
            }),
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_LootMessage: auto-loot / sell narration (text already link-cleaned).
fn loot_message(bytes: &[u8]) -> Decoded {
    match crate::loot_message::parse_loot_message(bytes) {
        Ok(m) => Decoded::One(Event::LootMessage {
            color: m.color,
            text: m.text,
            item_id: m.item_id,
            item_name: m.item_name,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// Guild/Group/Shout/Auction/OOC/Tell/Say/Raid (MessageType enum).
fn is_player_channel(c: u32) -> bool {
    matches!(c, 0 | 2 | 3 | 4 | 5 | 7 | 8 | 15)
}

// Player channels the server echoes back (so the C→S copy is dropped): all of
// them except Say (8), which is not echoed.
fn is_echoed_channel(c: u32) -> bool {
    matches!(c, 0 | 2 | 3 | 4 | 5 | 7 | 15)
}

// OP_LevelUpdate: the eql packet is an 80B widened container whose HEAD is the
// stock levelUpUpdateStruct, so feed the parser exactly that head — it length-
// checks exactly and would otherwise reject the whole packet.
fn level_update(bytes: &[u8]) -> Decoded {
    let n = crate::level_update::PAYLOAD_LEN;
    if bytes.len() < n {
        return Decoded::Malformed;
    }
    match crate::level_update::parse_level_update(&bytes[..n]) {
        Ok(l) => Decoded::One(Event::LevelUpdate {
            level: l.level,
            level_old: l.level_old,
            exp: l.exp,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_ExpUpdate = the regular exp bar (0..100000). Shared expUpdateStruct.
fn exp(bytes: &[u8]) -> Decoded {
    match crate::exp_update::parse_exp_update(bytes) {
        Ok(e) => Decoded::One(Event::Exp { exp: e.exp }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_AAExpUpdate = altExpUpdateStruct {u32 altexp@0, u32 aapoints@4, u32 tail};
// the daemon reads it directly (no dedicated parser), so decode the two fields.
fn aa_exp(bytes: &[u8]) -> Decoded {
    if bytes.len() < 8 {
        return Decoded::Malformed;
    }
    let rd = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    Decoded::One(Event::AaExp {
        alt_exp: rd(0),
        aa_points: rd(4),
    })
}

// OP_MoneyUpdate = the authoritative carried purse (un-normalized coins).
fn money(bytes: &[u8]) -> Decoded {
    match crate::money_update::parse_money_update(bytes) {
        Ok(m) => Decoded::One(Event::Money {
            platinum: m.platinum,
            gold: m.gold,
            silver: m.silver,
            copper: m.copper,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_ManaChange: the player's current mana (newMana); no max on the wire.
fn stamina(bytes: &[u8]) -> Decoded {
    match crate::stamina::parse_stamina(bytes) {
        Ok(s) => Decoded::One(Event::Stamina {
            food: s.food,
            water: s.water,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn mana_change(bytes: &[u8]) -> Decoded {
    match crate::mana_change::parse_mana_change(bytes) {
        Ok(m) => Decoded::One(Event::ManaUpdate {
            mana: m.new_mana.max(0) as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_SkillUpdate: one skill's new value (skillIncStruct).
fn skill_update(bytes: &[u8]) -> Decoded {
    match crate::skill_update::parse_skill_update(bytes) {
        Ok(s) => Decoded::One(Event::SkillUpdate {
            skill_id: s.skill_id,
            value: s.value.max(0) as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_LootDrops: the lootable items on a corpse (name/icon/item-id per item).
fn loot_drops(bytes: &[u8]) -> Decoded {
    match crate::loot_drops::parse_loot_drops(bytes) {
        Ok(d) => Decoded::One(Event::LootDrops {
            corpse_id: d.corpse_id,
            corpse_name: d.corpse_name,
            items: d
                .items
                .into_iter()
                .map(|i| LootItemInfo {
                    name: i.name,
                    icon: i.icon,
                    item_id: i.item_id,
                })
                .collect(),
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_LootTransaction: the subcode-7 confirmation carries an item's sale coin
// and the subcode-5 record the corpse's coin pile; the request/ack subcodes
// (3/6) ride the same id but surface nothing.
fn loot_transaction(bytes: &[u8]) -> Decoded {
    use crate::loot_transaction::LootTransactionError::Unhandled;
    match crate::loot_transaction::parse_loot_transaction(bytes) {
        Ok(t) => Decoded::One(Event::LootTransaction {
            corpse_id: t.corpse_id,
            item_id: t.item_id,
            quantity: t.quantity,
            coin_copper: t.coin_copper,
            from_corpse: t.from_corpse,
        }),
        Err(Unhandled(_)) => Decoded::Ignored,
        Err(_) => Decoded::Malformed,
    }
}

// eql OP_SendAATable = one AA definition (descID -> titleSID) per packet.
fn aa_table(bytes: &[u8]) -> Decoded {
    match crate::parse_aa_table_entry(bytes) {
        Ok(a) => Decoded::One(Event::AaTable {
            desc_id: a.desc_id,
            title_sid: a.title_sid,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// eql OP_BuffList = the authoritative per-spawn active-buff snapshot.
fn buff_list(bytes: &[u8]) -> Decoded {
    match crate::parse_buff_list(bytes) {
        Ok(bl) => Decoded::One(Event::BuffList {
            owner: bl.spawn_id,
            entries: bl
                .entries
                .into_iter()
                .map(|e| BuffEntry {
                    spell_id: e.spell_id,
                    remaining_ticks: e.remaining_ticks,
                    slot: e.slot,
                    caster: e.caster,
                })
                .collect(),
        }),
        Err(_) => Decoded::Malformed,
    }
}

// eql reuses Live's action2Struct byte-identically (OP_Action2 = damage).
fn action2(bytes: &[u8]) -> Decoded {
    match crate::action2::parse_action2(bytes) {
        // The wire marks "no spell" (a melee swing) as -1, which the parser
        // faithfully keeps as i32 — but casting that to u32 turns it into
        // 4294967295 and the neutral contract says 0 = melee, so a consumer
        // then looks up a spell that cannot exist. Normalise here.
        Ok(a) => Decoded::One(Event::Combat {
            source: u32::from(a.source),
            target: u32::from(a.target),
            kind: u32::from(a.kind),
            damage: a.damage,
            spell_id: if a.spell < 0 { 0 } else { a.spell as u32 },
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn action(bytes: &[u8]) -> Decoded {
    let parsed = if bytes.len() == crate::action::PAYLOAD_LEN {
        crate::action::parse_action(bytes).ok().map(|action| {
            (
                action.source,
                action.target,
                action.spell,
                action.level,
                action.kind,
            )
        })
    } else {
        crate::action_alt::parse_action_alt(bytes)
            .ok()
            .map(|action| {
                (
                    action.source,
                    action.target,
                    action.spell,
                    action.level,
                    action.kind,
                )
            })
    };
    match parsed {
        Some((source, target, spell_id, caster_level, kind)) => Decoded::One(Event::SpellAction {
            source: u32::from(source),
            target: u32::from(target),
            spell_id: u32::from(spell_id),
            caster_level,
            kind,
        }),
        None => Decoded::Malformed,
    }
}

fn cast_spell(bytes: &[u8]) -> Decoded {
    match crate::start_cast::parse_start_cast(bytes) {
        Ok(cast) => Decoded::One(Event::SpellCastRequest {
            slot: cast.slot,
            spell_id: cast.spell_id,
            target_id: cast.target_id,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn buff(bytes: &[u8]) -> Decoded {
    match crate::buff::parse_buff(bytes) {
        Ok(buff) => Decoded::One(Event::BuffWire {
            spawn_id: buff.spawn_id,
            spell_id: buff.spell_id,
            form: buff.form,
            slot: buff.slot,
            duration_ticks: buff.dur_ticks,
            change_type: buff.change_type,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_BeginCast: a spawn started casting. The daemon surfaces this (a transient
// cast indicator), NOT OP_CastSpell — cast-start buff insertion was noise, buffs
// ride OP_BuffList.
fn begin_cast(bytes: &[u8]) -> Decoded {
    match crate::parse_begin_cast(bytes) {
        Ok(c) => Decoded::One(Event::SpawnCast {
            caster_id: c.caster_id,
            spell_id: c.spell_id,
            cast_time_ms: c.cast_time_ms,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn new_zone(bytes: &[u8]) -> Decoded {
    match crate::parse_new_zone(bytes) {
        Ok(z) => Decoded::Many(vec![
            Event::ZoneChanged(ZoneInfo {
                short_name: z.short_name,
                long_name: z.long_name,
            }),
            Event::ZoneEnvironmentChanged(ZoneEnvironment {
                zone_file: z.zonefile,
                experience_multiplier: z.zone_exp_multiplier,
                safe_x: z.safe_x,
                safe_y: z.safe_y,
                safe_z: z.safe_z,
            }),
        ]),
        Err(_) => Decoded::Malformed,
    }
}

fn zone_change(bytes: &[u8]) -> Decoded {
    // Legends carries position but no destination in this 484-byte request.
    if bytes.len() != 484 {
        return Decoded::Malformed;
    }
    Decoded::One(Event::ZoneTransition {
        character_name: String::new(),
        zone_id: None,
        instance_id: None,
        confirmed: false,
    })
}

fn enter_world(bytes: &[u8]) -> Decoded {
    if bytes.len() != 72 {
        return Decoded::Malformed;
    }
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let name = &bytes[..end];
    if name.is_empty() || name.len() > 63 || !name.iter().all(|byte| byte.is_ascii_graphic()) {
        return Decoded::Malformed;
    }
    Decoded::One(Event::EnterWorld {
        character_name: String::from_utf8_lossy(name).into_owned(),
    })
}

fn player_profile(bytes: &[u8]) -> Decoded {
    match crate::parse_player_profile(bytes) {
        Ok(p) => Decoded::One(Event::PlayerProfile(ProfileInfo {
            name: p.name,
            last_name: p.last_name,
            class_: p.class_,
            level: p.level,
            race: p.race,
            deity: p.deity,
            cur_hp: p.cur_hp,
            mana: p.mana,
            aa_ids: p.aa_ids,
            aa_values: p.aa_values,
            aa_spent: p.aa_spent,
            aa_assigned: p.aa_assigned,
            aa_unspent: p.aa_unspent,
            aa_experience: p.exp_aa,
            skills: p.skills,
            class_mask: p.class_mask,
            str_: p.str_,
            sta: p.sta,
            cha: p.cha,
            dex: p.dex,
            int_: p.int_,
            agi: p.agi,
            wis: p.wis,
            platinum: p.platinum,
            gold: p.gold,
            silver: p.silver,
            copper: p.copper,
        })),
        Err(_) => Decoded::Malformed,
    }
}

fn doors(bytes: &[u8]) -> Decoded {
    if bytes.len() % crate::spawn_door::PAYLOAD_LEN != 0 {
        return Decoded::Malformed;
    }
    let parsed = bytes
        .chunks_exact(crate::spawn_door::PAYLOAD_LEN)
        .map(crate::spawn_door::parse_door)
        .collect::<Result<Vec<_>, _>>()
        .expect("validated fixed-size door rows");
    if parsed.iter().any(|door| {
        ![door.x, door.y, door.z, door.heading]
            .into_iter()
            .all(f32::is_finite)
    }) {
        return Decoded::Malformed;
    }
    let doors: Vec<DoorInfo> = parsed
        .into_iter()
        .map(|d| DoorInfo {
            id: u32::from(d.door_id),
            name: d.name,
            position: Point3 {
                x: d.x,
                y: d.y,
                z: d.z,
            },
            heading: d.heading,
            incline: d.incline,
            size: d.size,
            open_type: d.opentype,
            state: d.spawnstate,
            invert_state: d.invertstate,
            zone_point_id: (d.zone_point != u32::MAX).then_some(d.zone_point),
        })
        .collect();
    Decoded::One(Event::Doors(doors))
}

// OP_Illusion: a spawn changed race/model (id + new race/gender).
// OP_Stance / OP_Invocation are both 4B {u32 abilityId}; resolve the id to its
// display name (stable eqgame.exe GetAbilityName enum), "#<id>" if unknown —
// matching the daemon's stanceName/invocationName + fallback.
fn stance_name(id: u32) -> Option<&'static str> {
    Some(match id {
        117 => "Offense",
        118 => "Defense",
        119 => "Evasive",
        120 => "Balanced",
        121 => "Mage Hunter",
        122 => "Striker",
        123 => "Berserker",
        124 => "Ranged",
        135 => "Channeler",
        _ => return None,
    })
}
fn invocation_name(id: u32) -> Option<&'static str> {
    Some(match id {
        125 => "Recover",
        126 => "Empower",
        127 => "Inversion",
        128 => "Spell Blade",
        129 => "Over Channel",
        130 => "Inviolable",
        131 => "Divine",
        132 => "Chained",
        133 => "Arcane Mastery",
        134 => "Unyielding",
        _ => return None,
    })
}
fn resolve_ability(bytes: &[u8], name_of: fn(u32) -> Option<&'static str>) -> Option<String> {
    let id = crate::parse_activate_ability(bytes).ok()?;
    Some(
        name_of(id)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{id}")),
    )
}
fn inspect_answer(bytes: &[u8]) -> Decoded {
    // 1956B inspectDataStruct: pad[4], spawnId@4, itemNames[23][64]@8,
    // icons[23]@1480 (dropped — no proto home), mytext[200]@1572, pad[184].
    // Read through mytext; each name/bio is NUL-terminated latin1 (like strnlen).
    const NAMES_OFF: usize = 8;
    const NAME_LEN: usize = 64;
    const BIO_OFF: usize = 1572;
    const BIO_LEN: usize = 200;
    if bytes.len() < BIO_OFF + BIO_LEN {
        return Decoded::Malformed;
    }
    let spawn_id = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let item_names = (0..23)
        .map(|i| {
            crate::cstr_latin1(&bytes[NAMES_OFF + i * NAME_LEN..NAMES_OFF + (i + 1) * NAME_LEN])
        })
        .collect();
    let bio = crate::cstr_latin1(&bytes[BIO_OFF..BIO_OFF + BIO_LEN]);
    Decoded::One(Event::InspectAnswer {
        spawn_id,
        item_names,
        bio,
    })
}
fn guild_motd(bytes: &[u8]) -> Decoded {
    // Fixed layout; the parser is shared with the bridge's decode_guild_motd.
    // No guild id on the wire — the consumer stamps it from the roster.
    match crate::guild_motd::parse_guild_motd(bytes) {
        Ok(m) => Decoded::One(Event::GuildMotd {
            message: m.message,
            sender: m.sender,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn zone_server_info(bytes: &[u8]) -> Decoded {
    match crate::zone_server_info::parse_zone_server_info(bytes) {
        Ok(z) => Decoded::One(Event::ZoneServerInfo {
            host: z.host,
            port: u32::from(z.port),
        }),
        Err(_) => Decoded::Malformed,
    }
}

/// Shared by OP_ItemPacket and the loadout-swap tail — one construction, so the
/// two can never drift into describing items differently.
fn item_set_event(set: crate::item_packet::ItemSet) -> Event {
    Event::ItemSet {
        items: set
            .items
            .into_iter()
            .map(|i| ItemTemplate {
                serial: i.serial,
                name: i.name,
                lore_name: i.lore_name,
                item_id: i.item_id,
                icon: Some(i.icon),
                stack_count: None,
                weight_tenths: None,
                flags: None,
                corruption: None,
                slot_mask: i.slot_mask,
                container_id: i.container_id,
                container_slot: i.container_slot,
                parent_slot: i.parent_slot,
                stats: i.stats,
                resists: i.resists,
                hp: i.hp,
                mana: i.mana,
                endurance: i.endurance,
                ac: i.ac,
            })
            .collect(),
    }
}

fn item_packet(bytes: &[u8]) -> Decoded {
    // The C>S half is a 0-byte REQUEST that triggers the bulk reply; it carries
    // nothing to decode, so let it fall through as Malformed rather than
    // emitting an empty ItemSet a consumer would apply as "you own nothing".
    match crate::item_packet::parse_item_packet(bytes) {
        Ok(set) if !set.items.is_empty() => Decoded::One(item_set_event(set)),
        _ => Decoded::Malformed,
    }
}

fn guild_roster(bytes: &[u8]) -> Decoded {
    // eql wire diverges from the stock struct (wider header, multiclass mask in
    // the class slot, a rank field, a trailing zone id). The bridge's cxx path
    // and this share the one parser; the flag/primary-class derivation mirrors
    // decode_guild_roster in seq-bridge.
    match crate::guild_roster::parse_guild_member_list(bytes) {
        Ok(r) => {
            let members: Vec<GuildRosterMember> = r
                .members
                .into_iter()
                .map(|m| GuildRosterMember {
                    class: crate::guild_roster::primary_class(m.class_mask) as u32,
                    name: m.name,
                    level: m.level,
                    class_mask: m.class_mask,
                    rank: m.rank,
                    last_on: m.last_on,
                    // Wire packs both flags into one field: 0 none, 1 banker, 2 alt, 3 both.
                    banker: m.banker_flag % 2 != 0,
                    alt: m.banker_flag > 1,
                    full_member: m.full_member != 0,
                    public_note: m.public_note,
                    zone_id: m.zone_id as u32,
                })
                .collect();
            Decoded::Many(vec![
                Event::GuildRoster {
                    guild_id: r.guild_id,
                    members: members.clone(),
                },
                Event::GuildRosterWire {
                    guild_id: r.guild_id,
                    members,
                    complete: true,
                },
            ])
        }
        Err(_) => Decoded::Malformed,
    }
}

fn expanded_guild_info(bytes: &[u8]) -> Decoded {
    // Tagged union; only the rank-name action carries a rank-table entry. eql's
    // wire is byte-identical to Live's here (both dumped from captures). One
    // entry per packet — the consumer accumulates the rank -> name table.
    let i = crate::guild_expanded_info::parse_expanded_guild_info(bytes);
    if i.rank_index == 0 || i.rank_name.is_empty() {
        return Decoded::Ignored; // not the rank-name action (misc guild config)
    }
    Decoded::One(Event::GuildRankName {
        guild_id: i.guild_id,
        rank_index: i.rank_index,
        rank_name: i.rank_name,
    })
}

fn stance(bytes: &[u8]) -> Decoded {
    match resolve_ability(bytes, stance_name) {
        Some(name) => Decoded::One(Event::Stance { name }),
        None => Decoded::Malformed,
    }
}
fn invocation(bytes: &[u8]) -> Decoded {
    match resolve_ability(bytes, invocation_name) {
        Some(name) => Decoded::One(Event::Invocation { name }),
        None => Decoded::Malformed,
    }
}
fn time_of_day(bytes: &[u8]) -> Decoded {
    // 8B timeOfDayStruct: hour@0 u8, minute@1 u8, day@2 u8, month@3 u8,
    // year@4 u16 (+ 2B pad). Read the 6 meaningful bytes; tolerate the pad.
    if bytes.len() != 8 {
        return Decoded::Malformed;
    }
    if !(1..=24).contains(&bytes[0])
        || bytes[1] > 59
        || !(1..=28).contains(&bytes[2])
        || !(1..=12).contains(&bytes[3])
    {
        return Decoded::Malformed;
    }
    Decoded::One(Event::TimeOfDay {
        hour: bytes[0] as u32,
        minute: bytes[1] as u32,
        day: bytes[2] as u32,
        month: bytes[3] as u32,
        year: u16::from_le_bytes([bytes[4], bytes[5]]) as u32,
    })
}
fn spawn_appearance2(bytes: &[u8]) -> Decoded {
    // 24B {u32 spawnId, u32 type, u32 value, u8[12]}. Only type 6 (pose:
    // 110=sit / 100=stand / 111=duck) carries a spawn field; every other type
    // (periodic ticks, timestamps, mob-lock 0x2c, …) is consumed silently,
    // matching the daemon's EqlDispatch::spawnAppearance. Guard on >= 12 like
    // the daemon (only the first 12 bytes are read).
    if bytes.len() < 12 {
        return Decoded::Malformed;
    }
    let spawn_id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let kind = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let value = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if kind != 6 {
        return Decoded::Ignored;
    }
    Decoded::One(Event::SpawnAnimation {
        spawn_id,
        animation: value,
    })
}
fn click_object(dir: Dir, bytes: &[u8]) -> Decoded {
    // Dual-direction: the C>S side is the client's click REQUEST (16B, layout
    // unmapped) which nobody decodes — ignore it, like the daemon (S>C only).
    // The S>C side is the 12B remDropStruct removal of a ground item.
    if dir != Dir::ServerToClient {
        return Decoded::Ignored;
    }
    match crate::click_object::parse_click_object(bytes) {
        Ok(c) => Decoded::One(Event::GroundItemRemoved {
            drop_id: c.drop_id as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}
fn loadout_swap(bytes: &[u8]) -> Decoded {
    match crate::loadout_swap::parse_loadout_swap(bytes) {
        // Legends does delete-then-readd on a loadout/appearance change: a
        // paired OP_DeleteSpawn removes the id moments before this arrives, so
        // the embedded record IS the re-add. Emit it as a spawn FIRST so the
        // consumer re-creates a spawn it may have just dropped — otherwise the
        // next position update resurrects the id as an "Unknown" placeholder.
        // Consumers upsert on SpawnAdded, so this is idempotent when the spawn
        // is still tracked. Matches upstream's fix (legends 7612d72).
        Ok(l) => {
            let mut out = vec![
                spawn_event(&l.record),
                Event::LoadoutSwap {
                    spawn_id: l.spawn_id,
                    level: l.level as u32,
                    class: l.class_,
                    race: l.race,
                },
            ];

            // The SELF variant carries a serialized inventory tail in the same
            // record format OP_ItemPacket uses — confirmed on a captured swap:
            // 307705 bytes holding 234 items, parsed by the same walk. A
            // broadcast has no tail (tail_len 0), so nearby players' swaps add
            // nothing here. No swap follows with an OP_ItemPacket, so without
            // this a self swap would leave the item cache describing the
            // PREVIOUS loadout.
            let tail = crate::loadout_swap::tail_of(bytes);

            if !tail.is_empty() {
                if let Ok(set) = crate::item_packet::parse_item_packet(tail) {
                    if !set.items.is_empty() {
                        out.push(item_set_event(set));
                    }
                }
            }

            Decoded::Many(out)
        }
        Err(_) => Decoded::Malformed,
    }
}
fn illusion(bytes: &[u8]) -> Decoded {
    match crate::illusion::parse_illusion(bytes) {
        Ok(i) => Decoded::One(Event::SpawnIllusion {
            spawn_id: i.spawn_id,
            race: i.race,
            gender: i.gender,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn ground_item(bytes: &[u8]) -> Decoded {
    match crate::ground_spawn::parse_ground_spawn(bytes) {
        Ok(g) if [g.x, g.y, g.z, g.heading].into_iter().all(f32::is_finite) => {
            Decoded::One(Event::GroundItem(GroundItemInfo {
                id: g.drop_id,
                actor_definition: g.id_file,
                position: Point3 {
                    x: g.x,
                    y: g.y,
                    z: g.z,
                },
                heading: None,
            }))
        }
        Ok(_) | Err(_) => Decoded::Malformed,
    }
}

fn corpse_location(bytes: &[u8]) -> Decoded {
    match crate::corpse_loc::parse_corpse_loc(bytes) {
        Ok(corpse)
            if [corpse.x, corpse.y, corpse.z]
                .into_iter()
                .all(f32::is_finite) =>
        {
            Decoded::One(Event::CorpseLocated {
                id: corpse.spawn_id,
                position: Point3 {
                    x: corpse.x,
                    y: corpse.y,
                    z: corpse.z,
                },
            })
        }
        Ok(_) | Err(_) => Decoded::Malformed,
    }
}

fn zone_points(bytes: &[u8]) -> Decoded {
    let Some(count_bytes) = bytes.get(..4) else {
        return Decoded::Malformed;
    };
    let count = u32::from_le_bytes(count_bytes.try_into().expect("four-byte slice")) as usize;
    let Some(rows_len) = count.checked_mul(crate::zone_point::PAYLOAD_LEN) else {
        return Decoded::Malformed;
    };
    let Some(expected_len) = 4usize.checked_add(rows_len).and_then(|n| n.checked_add(24)) else {
        return Decoded::Malformed;
    };
    if bytes.len() != expected_len {
        return Decoded::Malformed;
    }
    let rows = &bytes[4..4 + rows_len];
    let parsed = rows
        .chunks_exact(crate::zone_point::PAYLOAD_LEN)
        .map(crate::zone_point::parse_zone_point)
        .collect::<Result<Vec<_>, _>>()
        .expect("validated fixed-size zone-point rows");
    if parsed.iter().any(|point| {
        ![point.x, point.y, point.z, point.heading]
            .into_iter()
            .all(f32::is_finite)
    }) {
        return Decoded::Malformed;
    }
    let points = parsed
        .into_iter()
        .map(|point| ZonePointInfo {
            trigger_id: Some(point.zone_trigger),
            actor_definition: None,
            position: Point3 {
                x: point.x,
                y: point.y,
                z: point.z,
            },
            heading: point.heading,
            destination_zone_id: Some(point.zone_id),
            destination_instance_id: Some(point.zone_instance),
        })
        .collect();
    Decoded::One(Event::ZonePoints(points))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_opcode_is_unhandled() {
        let d = EqlBackend.decode("OP_DoesNotExist", Dir::ServerToClient, &[]);
        assert_eq!(d, Decoded::Unhandled);
    }

    #[test]
    fn entity_broadcasts_validate_direction_before_payload() {
        let backend = EqlBackend;
        for opcode in [
            "OP_ZoneEntry",
            "OP_MobUpdate",
            "OP_NpcMoveUpdate",
            "OP_SpawnDoor",
            "OP_SendZonePoints",
        ] {
            assert_eq!(
                backend.decode(opcode, Dir::ClientToServer, &[]),
                Decoded::Ignored,
                "{opcode}"
            );
        }
    }

    // The parser returns the raw field; the sense is applied here.
    #[test]
    fn self_heading_is_inverted_like_every_other_heading() {
        let mut b = [0u8; crate::player_self_pos::PAYLOAD_LEN];
        b[22..24].copy_from_slice(&512u16.to_le_bytes()); // quarter of 2048; @22 as of 08/25

        let Decoded::One(Event::SelfPos { pos, .. }) =
            EqlBackend.decode("OP_ClientUpdate", Dir::ClientToServer, &b)
        else {
            panic!("expected a SelfPos event");
        };

        assert_eq!(pos.heading_deg, 270, "quarter turn must read 270, not 90");
        assert_eq!(pos.heading_deg, heading_deg(512, 11));
    }

    #[test]
    fn truncated_spawn_is_malformed_not_panic() {
        let d = EqlBackend.decode("OP_ZoneEntry", Dir::ServerToClient, &[0u8; 2]);
        assert_eq!(d, Decoded::Malformed);
    }

    #[test]
    fn enter_world_validates_direction_and_identity() {
        let mut payload = [0; 72];
        payload[..8].copy_from_slice(b"Testchar");
        assert_eq!(
            EqlBackend.decode("OP_EnterWorld", Dir::ClientToServer, &payload),
            Decoded::One(Event::EnterWorld {
                character_name: "Testchar".into()
            })
        );
        assert_eq!(
            EqlBackend.decode("OP_EnterWorld", Dir::ServerToClient, &payload),
            Decoded::Ignored
        );
        assert_eq!(
            EqlBackend.decode("OP_EnterWorld", Dir::ClientToServer, &[]),
            Decoded::Malformed
        );
    }

    #[test]
    fn loadout_swap_is_routed() {
        // A truncated payload must reach the parser (Malformed), proving the
        // opcode is wired — not fall through to Unhandled.
        let d = EqlBackend.decode("OP_LoadoutSwap", Dir::ServerToClient, &[0u8; 4]);
        assert_eq!(d, Decoded::Malformed);
    }

    #[test]
    fn click_object_s2c_removes_a_ground_item() {
        let mut buf = [0u8; 12]; // remDropStruct: dropId@0, spawnId@4
        buf[0..2].copy_from_slice(&0x1234u16.to_le_bytes());
        let d = EqlBackend.decode("OP_ClickObject", Dir::ServerToClient, &buf);
        assert_eq!(
            d,
            Decoded::One(Event::GroundItemRemoved { drop_id: 0x1234 })
        );
    }

    #[test]
    fn click_object_c2s_request_is_ignored() {
        // The client's 16B click request carries nothing we surface.
        let d = EqlBackend.decode("OP_ClickObject", Dir::ClientToServer, &[0u8; 16]);
        assert_eq!(d, Decoded::Ignored);
    }

    #[test]
    fn spawn_appearance2_type6_is_a_pose() {
        let mut b = [0u8; 24];
        b[0..4].copy_from_slice(&1234u32.to_le_bytes()); // spawnId
        b[4..8].copy_from_slice(&6u32.to_le_bytes()); // type 6 = pose
        b[8..12].copy_from_slice(&110u32.to_le_bytes()); // 110 = sit
        let d = EqlBackend.decode("OP_SpawnAppearance2", Dir::ServerToClient, &b);
        assert_eq!(
            d,
            Decoded::One(Event::SpawnAnimation {
                spawn_id: 1234,
                animation: 110
            })
        );
    }

    #[test]
    fn spawn_appearance2_other_types_are_ignored() {
        let mut b = [0u8; 24];
        b[4..8].copy_from_slice(&22u32.to_le_bytes()); // type 22 = periodic tick
        let d = EqlBackend.decode("OP_SpawnAppearance2", Dir::ServerToClient, &b);
        assert_eq!(d, Decoded::Ignored);
    }

    #[test]
    fn stance_resolves_known_and_unknown() {
        let known = EqlBackend.decode("OP_Stance", Dir::ServerToClient, &118u32.to_le_bytes());
        assert_eq!(
            known,
            Decoded::One(Event::Stance {
                name: "Defense".into()
            })
        );
        let unknown = EqlBackend.decode("OP_Stance", Dir::ServerToClient, &999u32.to_le_bytes());
        assert_eq!(
            unknown,
            Decoded::One(Event::Stance {
                name: "#999".into()
            })
        );
    }

    #[test]
    fn invocation_resolves() {
        let d = EqlBackend.decode("OP_Invocation", Dir::ServerToClient, &125u32.to_le_bytes());
        assert_eq!(
            d,
            Decoded::One(Event::Invocation {
                name: "Recover".into()
            })
        );
    }

    #[test]
    fn inspect_answer_decodes_names_and_bio() {
        let mut b = vec![0u8; 1956];
        b[4..8].copy_from_slice(&77u32.to_le_bytes()); // spawnId
        b[8..8 + 5].copy_from_slice(b"Sword"); // itemNames[0]
        b[1572..1572 + 3].copy_from_slice(b"Hi!"); // mytext (bio)
        let d = EqlBackend.decode("OP_InspectAnswer", Dir::ServerToClient, &b);
        match d {
            Decoded::One(Event::InspectAnswer {
                spawn_id,
                item_names,
                bio,
            }) => {
                assert_eq!(spawn_id, 77);
                assert_eq!(item_names.len(), 23);
                assert_eq!(item_names[0], "Sword");
                assert_eq!(item_names[1], "");
                assert_eq!(bio, "Hi!");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn inspect_answer_truncated_is_malformed() {
        assert_eq!(
            EqlBackend.decode("OP_InspectAnswer", Dir::ServerToClient, &[0u8; 100]),
            Decoded::Malformed
        );
    }

    #[test]
    fn zone_server_info_is_routed() {
        let mut p = vec![0u8; 130];
        let host = b"lvseqns-livz07.everquestlegends.com";
        p[..host.len()].copy_from_slice(host);
        p[128..].copy_from_slice(&3229u16.to_le_bytes());
        assert_eq!(
            EqlBackend.decode("OP_ZoneServerInfo", Dir::ServerToClient, &p),
            Decoded::One(Event::ZoneServerInfo {
                host: "lvseqns-livz07.everquestlegends.com".into(),
                port: 3229,
            })
        );
        // Short payload reaches the parser rather than falling through.
        assert_eq!(
            EqlBackend.decode("OP_ZoneServerInfo", Dir::ServerToClient, &[0u8; 4]),
            Decoded::Malformed
        );
    }

    #[test]
    fn a_self_loadout_swap_also_yields_its_item_set() {
        // Header + a minimal ZoneEntry-ish record will not parse, so this pins
        // the SHAPE via tail_of: a payload longer than innerLen has a tail, a
        // broadcast does not. The end-to-end proof is the captured swap (234
        // items out of a 307705-byte tail), which cannot ship as a fixture.
        let mut p = vec![0u8; 64];
        p[5..7].copy_from_slice(&64u16.to_le_bytes());
        assert!(
            crate::loadout_swap::tail_of(&p).is_empty(),
            "broadcast: no tail"
        );

        let mut p2 = vec![0u8; 200];
        p2[5..7].copy_from_slice(&64u16.to_le_bytes());
        assert_eq!(
            crate::loadout_swap::tail_of(&p2).len(),
            136,
            "self: tail present"
        );
    }

    #[test]
    fn item_packet_is_routed() {
        // A truncated payload reaches the parser (Malformed), proving the arm is
        // wired rather than falling through to Unhandled.
        assert_eq!(
            EqlBackend.decode("OP_ItemPacket", Dir::ServerToClient, &[0u8; 4]),
            Decoded::Malformed
        );
    }

    #[test]
    fn item_packet_request_does_not_wipe_the_cache() {
        // The C>S half is a 0-byte REQUEST. It must NOT decode to an empty
        // ItemSet: the event is authoritative-replacing, so a consumer would
        // apply that as "you own nothing".
        assert_eq!(
            EqlBackend.decode("OP_ItemPacket", Dir::ClientToServer, &[]),
            Decoded::Malformed
        );
    }

    #[test]
    fn guild_roster_is_routed() {
        // A truncated payload reaches the parser (Malformed), proving the arm is
        // wired rather than falling through to Unhandled.
        let d = EqlBackend.decode("OP_GuildMemberList", Dir::ServerToClient, &[0u8; 4]);
        assert_eq!(d, Decoded::Malformed);
    }

    #[test]
    fn guild_motd_is_routed() {
        // A full-size empty MOTD routes to GuildMotd with empty fields; a short
        // one is Malformed — either way it reached the parser, not Unhandled.
        let d = EqlBackend.decode("OP_GuildMOTD", Dir::ServerToClient, &[0u8; 656]);
        assert_eq!(
            d,
            Decoded::One(Event::GuildMotd {
                message: String::new(),
                sender: String::new(),
            })
        );
        assert_eq!(
            EqlBackend.decode("OP_GuildMOTD", Dir::ServerToClient, &[0u8; 4]),
            Decoded::Malformed
        );
    }

    #[test]
    fn time_of_day_decodes() {
        // hour=13, minute=45, day=7, month=3, year=3521
        let b = [13u8, 45, 7, 3, 0xC1, 0x0D, 0, 0]; // 0x0DC1 = 3521
        let d = EqlBackend.decode("OP_TimeOfDay", Dir::ServerToClient, &b);
        assert_eq!(
            d,
            Decoded::One(Event::TimeOfDay {
                year: 3521,
                month: 3,
                day: 7,
                hour: 13,
                minute: 45
            })
        );
    }

    #[test]
    fn self_pos_is_wired_but_inert() {
        // Recognized (not Unhandled) so it leaves the gap report, but emits
        // nothing — the wired-but-inert breadcrumb path.
        let d = EqlBackend.decode("OP_SelfPos", Dir::ServerToClient, &[0u8; 18]);
        assert_eq!(d, Decoded::Ignored);
    }

    #[test]
    fn empty_door_batch_is_empty_vec() {
        let d = EqlBackend.decode("OP_SpawnDoor", Dir::ServerToClient, &[]);
        assert_eq!(d, Decoded::One(Event::Doors(vec![])));
    }
}

#[cfg(test)]
mod level_update_tests {
    use super::*;

    // The eql packet is far wider than levelUpUpdateStruct. Reading the whole
    // thing rejects it on the exact-length check, which is how these went
    // undecoded: no error surfaced, the packet simply vanished.
    #[test]
    fn decodes_the_wide_container_by_slicing_its_head() {
        let n = crate::level_update::PAYLOAD_LEN;
        let mut b = vec![0u8; 80];
        b[0..4].copy_from_slice(&6u32.to_le_bytes());
        b[4..8].copy_from_slice(&5u32.to_le_bytes());
        b[8..12].copy_from_slice(&1530u32.to_le_bytes());
        assert!(b.len() > n, "fixture must be wider than the struct");
        assert!(crate::level_update::parse_level_update(&b).is_err());
        assert_eq!(
            level_update(&b),
            Decoded::One(Event::LevelUpdate {
                level: 6,
                level_old: 5,
                exp: 1530
            })
        );
    }

    #[test]
    fn rejects_a_runt() {
        assert_eq!(level_update(&[0u8; 4]), Decoded::Malformed);
    }
}
