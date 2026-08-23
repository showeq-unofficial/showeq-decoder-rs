//! Live EQ backend — maps the shared `seq-decode` parsers into the neutral
//! [`seq_events`] vocabulary. Also serves Test today (Test's wire is
//! byte-identical to Live; a `seq-backend-test` sibling forks it when it diverges).
//!
//! Field/heading math mirrors the daemon and the scry NIF exactly so decoded
//! output stays byte-for-byte identical across the migration.

use seq_events::{
    heading_deg, Backend, Decoded, Dir, DoorInfo, Event, GroundItemInfo, GuildInZone,
    GuildRosterMember, ItemTemplate, Point3, Pos, ProfileInfo, SpawnInfo, Velocity,
    ZoneEnvironment, ZoneInfo, ZonePointInfo,
};

/// The Live/Test backend (shared `seq-decode` parsers).
pub struct LiveBackend;

impl Backend for LiveBackend {
    fn name(&self) -> &'static str {
        "live"
    }

    fn decode(&self, opcode: &str, dir: Dir, bytes: &[u8]) -> Decoded {
        match opcode {
            "OP_ZoneEntry" if dir == Dir::ServerToClient => spawn(bytes),
            "OP_ZoneEntry" => Decoded::Ignored,
            "OP_MobUpdate" => mob_update(bytes),
            "OP_NpcMoveUpdate" if dir == Dir::ServerToClient => npc_move_update(bytes),
            "OP_NpcMoveUpdate" => Decoded::Ignored,
            "OP_RemoveSpawn" => remove_spawn(bytes),
            "OP_DeleteSpawn" => delete_spawn(bytes),
            "OP_SpawnRename" if dir == Dir::ServerToClient => spawn_rename(bytes),
            "OP_SpawnRename" => Decoded::Ignored,
            "OP_HPUpdate" => hp_update(bytes),
            "OP_Death" => death(bytes),
            "OP_NewZone" if dir == Dir::ServerToClient => new_zone(bytes),
            "OP_NewZone" => Decoded::Ignored,
            "OP_PlayerProfile" if dir == Dir::ServerToClient => player_profile(bytes),
            "OP_PlayerProfile" => Decoded::Ignored,
            "OP_ZoneChange" => zone_change(bytes, dir),
            "OP_ClientUpdate" => self_pos(bytes),
            "OP_Illusion" => illusion(bytes),
            "OP_ManaChange" => mana_change(bytes),
            "OP_Stamina" => stamina(bytes),
            "OP_ItemPacket" => item_packet(bytes),
            "OP_SkillUpdate" => skill_update(bytes),
            "OP_Action2" => action2(bytes),
            "OP_TargetMouse" => target(bytes),
            "OP_Consider" => consider(bytes),
            "OP_CommonMessage" => chat(bytes, dir),
            "OP_SimpleMessage" => simple_message(bytes),
            "OP_FormattedMessage" => formatted_message(bytes),
            "OP_SpecialMesg" => special_message(bytes),
            "OP_GroundSpawn" if dir == Dir::ServerToClient => ground_item(bytes),
            "OP_GroundSpawn" => Decoded::Ignored,
            "OP_ClickObject" => click_object(dir, bytes),
            "OP_SpawnDoor" if dir == Dir::ServerToClient => doors(bytes),
            "OP_SpawnDoor" => Decoded::Ignored,
            "OP_CorpseLocResponse" if dir == Dir::ServerToClient => corpse_location(bytes),
            "OP_CorpseLocResponse" => Decoded::Ignored,
            "OP_SendZonePoints" if dir == Dir::ServerToClient => zone_points(bytes),
            "OP_SendZonePoints" => Decoded::Ignored,
            "OP_SpawnAppearance" => spawn_appearance(bytes),
            "OP_GuildsInZoneList" => guilds_in_zone_list(bytes),
            "OP_NewGuildInZone" => new_guild_in_zone(bytes),
            "OP_GuildMemberList" => guild_roster(bytes),
            "OP_GuildMOTD" => guild_motd(bytes),
            "OP_ExpandedGuildInfo" => expanded_guild_info(bytes),
            "OP_ExpUpdate" => exp(bytes),
            "OP_AAExpUpdate" => aa_exp(bytes),
            "OP_TimeOfDay" if dir == Dir::ServerToClient => time_of_day(bytes),
            "OP_TimeOfDay" => Decoded::Ignored,
            "OP_ZoneServerInfo" if dir == Dir::ServerToClient => zone_server_info(bytes),
            "OP_ZoneServerInfo" => Decoded::Ignored,
            "OP_GroupFollow" => group_follow(bytes),
            "OP_GroupDisband" | "OP_GroupDisband2" => group_disband(bytes),
            "OP_EnterWorld" if dir == Dir::ClientToServer => enter_world(bytes),
            "OP_EnterWorld" => Decoded::Ignored,
            _ => Decoded::Unhandled,
        }
    }
}

fn spawn(bytes: &[u8]) -> Decoded {
    match seq_decode::spawn::parse_spawn(bytes) {
        Ok(s) => {
            let motion = s.motion();
            let equipment_models = s.equipment_models();
            Decoded::One(Event::SpawnAdded(SpawnInfo {
                id: s.spawn_id,
                name: s.name,
                last_name: s.last_name,
                race: s.race,
                class_: s.class_,
                deity: s.deity,
                level: s.level,
                npc: s.npc,
                cur_hp: u32::from(s.cur_hp),
                max_hp: None, // Live spawn carries no max HP; arrives via HP opcodes.
                guild_id: s.guild_id,
                // Live has no guild-in-zone name feed wired, so the pair is unused
                // here; 0 keeps the key inert rather than colliding on server 0.
                guild_server_id: 0,
                class_mask: 0, // live isn't multiclass
                pos: Some(Pos {
                    x: motion.x,
                    y: motion.y,
                    z: motion.z,
                    heading_deg: heading_deg(motion.heading, 12),
                }),
                velocity: Velocity {
                    x: Some(motion.delta_x),
                    y: Some(motion.delta_y),
                    z: Some(motion.delta_z),
                },
                delta_heading: Some(motion.delta_heading),
                animation: Some(motion.animation),
                equipment_models: Some(equipment_models),
            }))
        }
        Err(_) => Decoded::Malformed,
    }
}

fn mob_update(bytes: &[u8]) -> Decoded {
    match seq_decode::mob_update::parse_mob_update(bytes) {
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
    match seq_decode::npc_move_update::parse_npc_move_update(bytes) {
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
    match seq_decode::remove_spawn::parse_remove_spawn(bytes) {
        Ok(s) => Decoded::One(Event::SpawnRemoved { id: s.spawn_id }),
        Err(_) => Decoded::Malformed,
    }
}

fn delete_spawn(bytes: &[u8]) -> Decoded {
    match seq_decode::delete_spawn::parse_delete_spawn(bytes) {
        Ok(s) => Decoded::One(Event::SpawnRemoved { id: s.spawn_id }),
        Err(_) => Decoded::Malformed,
    }
}

fn spawn_rename(bytes: &[u8]) -> Decoded {
    match seq_decode::spawn_rename::parse_spawn_rename(bytes) {
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

fn hp_update(bytes: &[u8]) -> Decoded {
    match seq_decode::hp_update::parse_hp_update(bytes) {
        Ok(s) => Decoded::One(Event::SpawnHp {
            id: u32::from(s.spawn_id),
            cur: s.cur_hp,
            max: s.max_hp,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn self_pos(bytes: &[u8]) -> Decoded {
    match seq_decode::player_self_pos::parse_player_self_pos(bytes) {
        Ok(s) => Decoded::One(Event::SelfPos {
            pos: Pos {
                x: s.x.round() as i32,
                y: s.y.round() as i32,
                z: s.z.round() as i32,
                heading_deg: heading_deg(s.heading, 12),
            },
            spawn_id: u32::from(s.spawn_id),
            velocity: Velocity {
                x: Some(s.delta_x as i32),
                y: Some(s.delta_y as i32),
                z: Some(s.delta_z as i32),
            },
            delta_heading: Some(s.delta_heading),
            animation: Some(s.animation),
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_TargetMouse = the player's target selection (0 = cleared).
fn target(bytes: &[u8]) -> Decoded {
    match seq_decode::client_target::parse_client_target(bytes) {
        Ok(t) => Decoded::One(Event::Targeted {
            spawn_id: t.new_target,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_Consider = the player conned a spawn; the target is the considered spawn.
fn consider(bytes: &[u8]) -> Decoded {
    match seq_decode::consider::parse_consider(bytes) {
        Ok(c) => Decoded::One(Event::Considered {
            spawn_id: c.target_id,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_CommonMessage = player chat; keep only the player channels (drop system
// noise), matching MessageShell::channelMessage.
fn chat(bytes: &[u8], dir: Dir) -> Decoded {
    match seq_decode::channel_message::parse_channel_message(bytes) {
        // Drop the client's C→S copy of the echoed channels (tells/group/…), but
        // keep C→S Say — matches MessageShell::channelMessage.
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

// Player channels the server echoes back (drop the C→S copy): all except Say.
fn is_echoed_channel(c: u32) -> bool {
    matches!(c, 0 | 2 | 3 | 4 | 5 | 7 | 15)
}

// Guild/Group/Shout/Auction/OOC/Tell/Say/Raid (MessageType enum).
fn is_player_channel(c: u32) -> bool {
    matches!(c, 0 | 2 | 3 | 4 | 5 | 7 | 8 | 15)
}

// OP_Action2 = a damage event; matches the daemon's CombatRouter::action2.
fn action2(bytes: &[u8]) -> Decoded {
    match seq_decode::action2::parse_action2(bytes) {
        Ok(a) => Decoded::One(Event::Combat {
            source: u32::from(a.source),
            target: u32::from(a.target),
            kind: u32::from(a.kind),
            damage: a.damage,
            spell_id: a.spell as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_Death (newCorpseStruct): a death leaves a corpse, not a removal.
// seq-session resolves player ownership; direct backend callers retain this
// low-level result during migration.
fn death(bytes: &[u8]) -> Decoded {
    match seq_decode::death::parse_death(bytes) {
        Ok(d) => Decoded::One(Event::SpawnKilled {
            deceased_id: d.spawn_id,
            killer_id: d.killer_id,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn new_zone(bytes: &[u8]) -> Decoded {
    match seq_decode::new_zone::parse_new_zone(bytes) {
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

fn zone_change(bytes: &[u8], dir: Dir) -> Decoded {
    match seq_decode::zone_change::parse_zone_change(bytes) {
        Ok(zone) => Decoded::One(Event::ZoneTransition {
            character_name: zone.name,
            zone_id: Some(u32::from(zone.zone_id)),
            instance_id: Some(u32::from(zone.zone_instance)),
            confirmed: dir == Dir::ServerToClient,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn zone_server_info(bytes: &[u8]) -> Decoded {
    match seq_decode::zone_server_info::parse_zone_server_info(bytes) {
        Ok(info) => Decoded::One(Event::ZoneServerInfo {
            host: info.host,
            port: u32::from(info.port),
        }),
        Err(_) => Decoded::Malformed,
    }
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
    match seq_decode::player_profile::parse_player_profile(bytes) {
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
    if bytes.len() % seq_decode::spawn_door::PAYLOAD_LEN != 0 {
        return Decoded::Malformed;
    }
    let parsed = bytes
        .chunks_exact(seq_decode::spawn_door::PAYLOAD_LEN)
        .map(seq_decode::spawn_door::parse_door)
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

// OP_ManaChange: the player's current mana (newMana); no max on the wire.
fn item_packet(bytes: &[u8]) -> Decoded {
    match seq_decode::item_packet::parse_item_packet(bytes) {
        Ok(i) => Decoded::One(Event::ItemLearned {
            item: ItemTemplate {
                serial: i.instance_id,
                name: i.name,
                lore_name: i.lore_name,
                item_id: i.item_id,
                icon: None,
                stack_count: Some(i.stack_count),
                weight_tenths: Some(i.weight_tenths),
                flags: Some(i.flags),
                corruption: Some(i.corruption),
                slot_mask: i.slot_mask,
                // Live addresses possessions only; its mainSlot/subSlot map
                // onto the neutral pair, with mainSlot 0 meaning "not in a bag"
                // — the same thing eql spells 0xFFFF. Normalising here is what
                // lets `is_worn` be shared rather than reimplemented per wire.
                container_id: 0,
                container_slot: i.sub_slot,
                parent_slot: if i.main_slot == 0 {
                    seq_events::TOP_LEVEL_SLOT
                } else {
                    i.main_slot as u16
                },
                stats: i.stats,
                resists: i.resists,
                hp: i.hp,
                mana: i.mana,
                endurance: i.endurance,
                ac: i.ac,
            },
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn stamina(bytes: &[u8]) -> Decoded {
    match seq_decode::stamina::parse_stamina(bytes) {
        Ok(s) => Decoded::One(Event::Stamina {
            food: s.food,
            water: s.water,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn mana_change(bytes: &[u8]) -> Decoded {
    match seq_decode::mana_change::parse_mana_change(bytes) {
        Ok(m) => Decoded::One(Event::ManaUpdate {
            mana: m.new_mana.max(0) as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_SkillUpdate: one skill's new value (skillIncStruct).
fn skill_update(bytes: &[u8]) -> Decoded {
    match seq_decode::skill_update::parse_skill_update(bytes) {
        Ok(s) => Decoded::One(Event::SkillUpdate {
            skill_id: s.skill_id,
            value: s.value.max(0) as u32,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_Illusion: a spawn changed race/model (id + new race/gender).
fn illusion(bytes: &[u8]) -> Decoded {
    match seq_decode::illusion::parse_illusion(bytes) {
        Ok(i) => Decoded::One(Event::SpawnIllusion {
            spawn_id: i.spawn_id,
            race: i.race,
            gender: i.gender,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn ground_item(bytes: &[u8]) -> Decoded {
    match seq_decode::ground_spawn::parse_ground_spawn(bytes) {
        Ok(g) if [g.x, g.y, g.z, g.heading].into_iter().all(f32::is_finite) => {
            Decoded::One(Event::GroundItem(GroundItemInfo {
                id: g.drop_id,
                actor_definition: g.id_file,
                position: Point3 {
                    x: g.x,
                    y: g.y,
                    z: g.z,
                },
                heading: Some(g.heading),
            }))
        }
        Ok(_) | Err(_) => Decoded::Malformed,
    }
}

fn corpse_location(bytes: &[u8]) -> Decoded {
    match seq_decode::corpse_loc::parse_corpse_loc(bytes) {
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

#[cfg(feature = "backend-live")]
fn zone_points(bytes: &[u8]) -> Decoded {
    let Some(count_bytes) = bytes.get(..4) else {
        return Decoded::Malformed;
    };
    let count = u32::from_le_bytes(count_bytes.try_into().expect("four-byte slice")) as usize;
    let Some(rows_len) = count.checked_mul(seq_decode::zone_point::PAYLOAD_LEN) else {
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
        .chunks_exact(seq_decode::zone_point::PAYLOAD_LEN)
        .map(seq_decode::zone_point::parse_zone_point)
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

#[cfg(feature = "backend-test")]
fn zone_points(bytes: &[u8]) -> Decoded {
    const RECORD_LEN: usize = 136;
    const NAME_LEN: usize = 32;

    if bytes.is_empty() || bytes.len() % RECORD_LEN != 0 {
        return Decoded::Malformed;
    }

    let mut points = Vec::with_capacity(bytes.len() / RECORD_LEN);
    for row in bytes.chunks_exact(RECORD_LEN) {
        let name_bytes = &row[..NAME_LEN];
        let Some(name_len) = name_bytes.iter().position(|byte| *byte == 0) else {
            return Decoded::Malformed;
        };
        let name = &name_bytes[..name_len];
        if name.is_empty() || !name.iter().all(|byte| byte.is_ascii_graphic()) {
            return Decoded::Malformed;
        }

        let read_float =
            |at: usize| f32::from_le_bytes(row[at..at + 4].try_into().expect("four-byte float"));
        // The Test record retains the legacy map-frame y/x/z wire order.
        let position = Point3 {
            x: read_float(0x24),
            y: read_float(0x20),
            z: read_float(0x28),
        };
        let heading = read_float(0x2c);
        if ![position.x, position.y, position.z, heading]
            .into_iter()
            .all(f32::is_finite)
        {
            return Decoded::Malformed;
        }

        points.push(ZonePointInfo {
            trigger_id: None,
            actor_definition: Some(String::from_utf8_lossy(name).into_owned()),
            position,
            heading,
            destination_zone_id: None,
            destination_instance_id: None,
        });
    }
    Decoded::One(Event::ZonePoints(points))
}

// OP_ClickObject: dual-direction. The C>S side is the client's click request
// (nobody decodes it); the S>C side is the remDropStruct removal of a ground
// item, matching the daemon's server-only wiring.
fn click_object(dir: Dir, bytes: &[u8]) -> Decoded {
    if dir != Dir::ServerToClient {
        return Decoded::Ignored;
    }
    match seq_decode::click_object::parse_click_object(bytes) {
        Ok(c) => Decoded::One(Event::GroundItemRemoved {
            drop_id: u32::from(c.drop_id),
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_SpawnAppearance is a subcommand carrier and the current-patch wire carries
// no value field, so a subcommand is a bare signal. Its numbering is NOT the
// legacy one (that assumed `type` at offset 2; see the struct's re-derivation)
// and no current type has confirmed semantics — 4 / 32 / 64 are all that two
// live captures show. Parsed for length-validation, mapped to nothing until a
// type is pinned in-game: a wrong guess here writes wrong spawn state.
fn spawn_appearance(bytes: &[u8]) -> Decoded {
    match seq_decode::spawn_appearance::parse_spawn_appearance(bytes) {
        Ok(_) => Decoded::Ignored,
        Err(_) => Decoded::Malformed,
    }
}

fn guilds_in_zone_list(bytes: &[u8]) -> Decoded {
    match seq_decode::guild_in_zone::parse_guilds_in_zone_list(bytes) {
        // An empty list is normal (an unguilded zone) and carries nothing.
        Ok(guilds) if guilds.is_empty() => Decoded::Ignored,
        Ok(guilds) => Decoded::One(Event::GuildsInZone {
            guilds: guilds.into_iter().map(guild_in_zone).collect(),
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn new_guild_in_zone(bytes: &[u8]) -> Decoded {
    match seq_decode::guild_in_zone::parse_new_guild_in_zone(bytes) {
        Ok(g) => Decoded::One(Event::GuildsInZone {
            guilds: vec![guild_in_zone(g)],
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn guild_in_zone(g: seq_decode::guild_in_zone::GuildInZone) -> GuildInZone {
    GuildInZone {
        guild_id: g.guild_id,
        server_id: g.server_id,
        name: g.name,
    }
}

// OP_GuildMemberList: the full roster, authoritative and replacing. Live is
// single-class, so `class_mask` stays 0; the roster carries no member zone
// (`zone_id` 0) — that arrives separately via OP_GuildMemberUpdate.
fn guild_roster(bytes: &[u8]) -> Decoded {
    match seq_decode::guild_roster::parse_guild_member_list(bytes) {
        Ok(r) => {
            let members = r
                .members
                .into_iter()
                .map(|m| GuildRosterMember {
                    name: m.name,
                    level: m.level,
                    class: m.primary_class,
                    class_mask: 0,
                    rank: m.rank,
                    last_on: m.last_on,
                    banker: m.banker,
                    alt: m.alt,
                    full_member: m.full_member,
                    public_note: m.public_note,
                    zone_id: 0,
                })
                .collect();
            Decoded::One(Event::GuildRoster {
                guild_id: r.guild_id,
                members,
            })
        }
        Err(_) => Decoded::Malformed,
    }
}

fn guild_motd(bytes: &[u8]) -> Decoded {
    match seq_decode::guild_motd::parse_guild_motd(bytes) {
        Ok(m) => Decoded::One(Event::GuildMotd {
            message: m.message,
            sender: m.sender,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_ExpandedGuildInfo is a tagged union; only the rank-name action carries a
// rank-table entry (one per packet — the consumer accumulates the table).
fn expanded_guild_info(bytes: &[u8]) -> Decoded {
    let i = seq_decode::guild_expanded_info::parse_expanded_guild_info(bytes);
    if i.rank_index == 0 || i.rank_name.is_empty() {
        return Decoded::Ignored; // misc guild config, not the rank table
    }
    Decoded::One(Event::GuildRankName {
        guild_id: i.guild_id,
        rank_index: i.rank_index,
        rank_name: i.rank_name,
    })
}

fn simple_message(bytes: &[u8]) -> Decoded {
    match seq_decode::simple_message::parse_simple_message(bytes) {
        Ok(m) => Decoded::One(Event::SimpleMessage {
            format_id: m.message_format,
            color: m.message_color,
        }),
        Err(_) => Decoded::Malformed,
    }
}

// OP_FormattedMessage: header + the `{u32 len, bytes}` substitution blob the
// consumer interpolates into the eqstr template.
fn formatted_message(bytes: &[u8]) -> Decoded {
    match seq_decode::formatted_message::parse_formatted_message(bytes) {
        Ok(m) => Decoded::One(Event::FormattedMessage {
            format_id: m.message_format,
            color: m.message_color,
            args: seq_decode::formatted_message::parse_formatted_message_args(bytes),
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn special_message(bytes: &[u8]) -> Decoded {
    match seq_decode::special_message::parse_special_message(bytes) {
        Ok(m) => Decoded::One(Event::SpecialMessage {
            color: m.message_color,
            target: u32::from(m.target),
            source: m.source,
            message: m.message,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn exp(bytes: &[u8]) -> Decoded {
    match seq_decode::exp_update::parse_exp_update(bytes) {
        Ok(e) => Decoded::One(Event::Exp { exp: e.exp }),
        Err(_) => Decoded::Malformed,
    }
}

fn aa_exp(bytes: &[u8]) -> Decoded {
    match seq_decode::alt_exp_update::parse_alt_exp_update(bytes) {
        Ok(a) => Decoded::One(Event::AaExp {
            alt_exp: a.alt_exp,
            aa_points: a.aa_points,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn time_of_day(bytes: &[u8]) -> Decoded {
    match seq_decode::time_of_day::parse_time_of_day(bytes) {
        Ok(t) => Decoded::One(Event::TimeOfDay {
            year: u32::from(t.year),
            month: u32::from(t.month),
            day: u32::from(t.day),
            hour: u32::from(t.hour),
            minute: u32::from(t.minute),
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn group_follow(bytes: &[u8]) -> Decoded {
    match seq_decode::group_follow::parse_group_follow(bytes) {
        // Live's groupFollowStruct carries no level (an eql addition) — 0 is
        // the contract's "absent".
        Ok(g) => Decoded::One(Event::GroupFollow {
            name: g.name,
            level: 0,
        }),
        Err(_) => Decoded::Malformed,
    }
}

fn group_disband(bytes: &[u8]) -> Decoded {
    match seq_decode::group_disband::parse_group_disband(bytes) {
        Ok(g) => Decoded::One(Event::GroupDisband {
            yourname: g.yourname,
            membername: g.membername,
        }),
        Err(_) => Decoded::Malformed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_opcode_is_unhandled() {
        let d = LiveBackend.decode("OP_DoesNotExist", Dir::ServerToClient, &[]);
        assert_eq!(d, Decoded::Unhandled);
    }

    #[test]
    fn entity_broadcasts_validate_direction_before_payload() {
        let backend = LiveBackend;
        assert_eq!(
            backend.decode("OP_ZoneEntry", Dir::ClientToServer, &[]),
            Decoded::Ignored
        );
        assert_eq!(
            backend.decode("OP_NpcMoveUpdate", Dir::ClientToServer, &[]),
            Decoded::Ignored
        );
        assert_eq!(
            backend.decode("OP_SpawnDoor", Dir::ClientToServer, &[]),
            Decoded::Ignored
        );
        assert_eq!(
            backend.decode("OP_SendZonePoints", Dir::ClientToServer, &[]),
            Decoded::Ignored
        );
    }

    #[test]
    fn truncated_spawn_is_malformed_not_panic() {
        let d = LiveBackend.decode("OP_ZoneEntry", Dir::ServerToClient, &[0u8; 2]);
        assert_eq!(d, Decoded::Malformed);
    }

    #[test]
    fn enter_world_validates_direction_and_identity() {
        let mut payload = [0; 72];
        payload[..6].copy_from_slice(b"Firona");
        assert_eq!(
            LiveBackend.decode("OP_EnterWorld", Dir::ClientToServer, &payload),
            Decoded::One(Event::EnterWorld {
                character_name: "Firona".into()
            })
        );
        assert_eq!(
            LiveBackend.decode("OP_EnterWorld", Dir::ServerToClient, &payload),
            Decoded::Ignored
        );
        assert_eq!(
            LiveBackend.decode("OP_EnterWorld", Dir::ClientToServer, &[]),
            Decoded::Malformed
        );
    }

    #[test]
    fn empty_door_batch_is_empty_vec() {
        let d = LiveBackend.decode("OP_SpawnDoor", Dir::ServerToClient, &[]);
        assert_eq!(d, Decoded::One(Event::Doors(vec![])));
    }

    // Every current-patch subcommand parses and maps to nothing (semantics
    // unpinned); a wrong length is still reported.
    #[test]
    fn spawn_appearance_parses_but_surfaces_no_event_yet() {
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&25_049u32.to_le_bytes());
        b[4..8].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(
            LiveBackend.decode("OP_SpawnAppearance", Dir::ServerToClient, &b),
            Decoded::Ignored
        );
        assert_eq!(
            LiveBackend.decode("OP_SpawnAppearance", Dir::ServerToClient, &[0u8; 7]),
            Decoded::Malformed
        );
    }

    #[test]
    fn formatted_message_carries_its_interpolation_args() {
        let header = seq_decode::formatted_message::HEADER_LEN;
        let mut b = vec![0u8; header];
        b[5..9].copy_from_slice(&11_355u32.to_le_bytes()); // messageFormat
        b[9..13].copy_from_slice(&335u32.to_le_bytes()); // messageColor
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(b"15");

        assert_eq!(
            LiveBackend.decode("OP_FormattedMessage", Dir::ServerToClient, &b),
            Decoded::One(Event::FormattedMessage {
                format_id: 11_355,
                color: 335,
                args: vec!["15".to_string()],
            })
        );
    }

    // The rank-name action is one entry per packet; every other action is misc
    // guild config with no rank table in it.
    #[test]
    fn expanded_guild_info_surfaces_only_the_rank_name_action() {
        let mut b = vec![0u8; 192];
        b[0..4].copy_from_slice(&3u32.to_le_bytes()); // action == rank name
        b[8..12].copy_from_slice(&15u32.to_le_bytes()); // guild id
        b[88..92].copy_from_slice(&1u32.to_le_bytes()); // rank index
        b[92..98].copy_from_slice(b"Leader");

        assert_eq!(
            LiveBackend.decode("OP_ExpandedGuildInfo", Dir::ServerToClient, &b),
            Decoded::One(Event::GuildRankName {
                guild_id: 15,
                rank_index: 1,
                rank_name: "Leader".to_string(),
            })
        );

        b[0..4].copy_from_slice(&1u32.to_le_bytes());
        b[88..92].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            LiveBackend.decode("OP_ExpandedGuildInfo", Dir::ServerToClient, &b),
            Decoded::Ignored
        );
    }

    #[test]
    fn guilds_in_zone_list_maps_into_the_neutral_rows() {
        let mut b = Vec::new();
        b.extend_from_slice(&4u32.to_le_bytes()); // requester name length
        b.extend_from_slice(b"Name");
        b.extend_from_slice(&1u32.to_le_bytes()); // count
        b.extend_from_slice(&15u32.to_le_bytes());
        b.extend_from_slice(&180u32.to_le_bytes());
        b.extend_from_slice(b"A Guild\0");

        assert_eq!(
            LiveBackend.decode("OP_GuildsInZoneList", Dir::ServerToClient, &b),
            Decoded::One(Event::GuildsInZone {
                guilds: vec![GuildInZone {
                    guild_id: 15,
                    server_id: 180,
                    name: "A Guild".to_string(),
                }],
            })
        );
    }

    #[test]
    fn time_of_day_and_aa_exp_map_their_fixed_structs() {
        let mut t = [0u8; 8];
        t[0] = 6; // hour
        t[1] = 35; // minute
        t[2] = 28; // day
        t[3] = 6; // month
        t[4..6].copy_from_slice(&3789u16.to_le_bytes());
        assert_eq!(
            LiveBackend.decode("OP_TimeOfDay", Dir::ServerToClient, &t),
            Decoded::One(Event::TimeOfDay {
                year: 3789,
                month: 6,
                day: 28,
                hour: 6,
                minute: 35
            })
        );

        let mut a = [0u8; 12];
        a[0..4].copy_from_slice(&91_234u32.to_le_bytes());
        a[4..8].copy_from_slice(&317u32.to_le_bytes());
        assert_eq!(
            LiveBackend.decode("OP_AAExpUpdate", Dir::ServerToClient, &a),
            Decoded::One(Event::AaExp {
                alt_exp: 91_234,
                aa_points: 317
            })
        );
    }

    // The client's click request is not the removal; only S>C removes a drop.
    #[test]
    fn click_object_ignores_the_client_request() {
        assert_eq!(
            LiveBackend.decode("OP_ClickObject", Dir::ClientToServer, &[0u8; 16]),
            Decoded::Ignored
        );
    }
}
