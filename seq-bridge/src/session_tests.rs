use super::*;
use seq_events::{
    BuffEntry, DoorInfo, Event, GroundItemInfo, GuildInZone, GuildRosterMember, ItemTemplate,
    LootItemInfo, PlayerAppearance, PlayerIdentity, PlayerVitals, Point3, Pos, ProfileInfo,
    SessionResetReason, SpawnInfo, Velocity, VitalValue, ZoneEnvironment, ZoneInfo, ZonePointInfo,
};

fn pos(seed: i32) -> Pos {
    Pos {
        x: seed,
        y: seed + 1,
        z: seed + 2,
        heading_deg: (seed + 3) as u16,
    }
}

fn point3(seed: f32) -> Point3 {
    Point3 {
        x: seed,
        y: seed + 0.25,
        z: seed + 0.5,
    }
}

fn velocity(seed: i32) -> Velocity {
    Velocity {
        x: Some(seed),
        y: None,
        z: Some(seed + 2),
    }
}

fn spawn() -> SpawnInfo {
    SpawnInfo {
        id: 1,
        name: "spawn".into(),
        last_name: "last".into(),
        race: 2,
        class_: 3,
        deity: 4,
        level: 5,
        npc: 6,
        cur_hp: 7,
        max_hp: Some(8),
        guild_id: 9,
        guild_server_id: 10,
        class_mask: 11,
        pos: Some(pos(12)),
        velocity: velocity(16),
        delta_heading: Some(19),
        animation: Some(20),
        equipment_models: Some([21, 22, 23, 24, 25, 26, 27, 28, 29]),
    }
}

fn profile() -> ProfileInfo {
    ProfileInfo {
        name: "player".into(),
        last_name: "surname".into(),
        class_: 20,
        level: 21,
        race: 22,
        deity: 23,
        cur_hp: 24,
        mana: 25,
        aa_ids: vec![26, 27],
        aa_values: vec![28, 29],
        aa_spent: 30,
        skills: vec![31, 32],
        class_mask: 33,
        str_: 34,
        sta: 35,
        cha: 36,
        dex: 37,
        int_: 38,
        agi: 39,
        wis: 40,
        platinum: 41,
        gold: 42,
        silver: 43,
        copper: 44,
    }
}

fn guild() -> GuildInZone {
    GuildInZone {
        guild_id: 50,
        server_id: 51,
        name: "guild".into(),
    }
}

fn member() -> GuildRosterMember {
    GuildRosterMember {
        name: "member".into(),
        level: 60,
        class: 61,
        class_mask: 62,
        rank: 63,
        last_on: 64,
        banker: true,
        alt: true,
        full_member: true,
        public_note: "note".into(),
        zone_id: 65,
    }
}

fn item() -> ItemTemplate {
    ItemTemplate {
        serial: "serial".into(),
        name: "item".into(),
        lore_name: "lore".into(),
        item_id: 70,
        icon: 71,
        slot_mask: 72,
        container_id: 73,
        container_slot: 74,
        parent_slot: 75,
        stats: vec![-76, 77],
        resists: vec![-78, 79],
        hp: -80,
        mana: 81,
        endurance: -82,
        ac: 83,
    }
}

fn door() -> DoorInfo {
    DoorInfo {
        id: 90,
        name: "door".into(),
        position: point3(-91.0),
        heading: 92.5,
        incline: 93,
        size: 94,
        open_type: 95,
        state: 96,
        invert_state: 97,
        zone_point_id: Some(98),
    }
}

fn loot_item() -> LootItemInfo {
    LootItemInfo {
        name: "loot".into(),
        icon: 100,
        item_id: 101,
    }
}

fn buff() -> BuffEntry {
    BuffEntry {
        spell_id: 110,
        remaining_ticks: -111,
        slot: 112,
        caster: "caster".into(),
    }
}

#[allow(clippy::too_many_lines)]
fn canonical_events() -> Vec<Event> {
    vec![
        Event::SessionReset {
            reason: SessionResetReason::PlayerProfile,
        },
        Event::PlayerIdentityUpdated(PlayerIdentity {
            spawn_id: Some(101),
            name: "player".into(),
            last_name: "surname".into(),
            race: 102,
            class_: 103,
            deity: 104,
            level: 105,
            class_mask: 106,
        }),
        Event::PlayerMoved {
            spawn_id: None,
            pos: pos(107),
        },
        Event::PlayerVitalsUpdated(PlayerVitals {
            health: Some(VitalValue {
                current: -111,
                maximum: Some(112),
            }),
            mana: Some(VitalValue {
                current: 113,
                maximum: None,
            }),
            endurance: None,
        }),
        Event::SpawnHealthUpdated {
            id: 114,
            current: 115,
            maximum: 116,
        },
        Event::PlayerDied { killer_id: None },
        Event::SpawnDied {
            id: 117,
            killer_id: Some(118),
        },
        Event::SpawnIdentityUpdated {
            id: 119,
            level: 120,
            class_: 121,
            race: 122,
        },
        Event::PlayerAppearanceUpdated(PlayerAppearance {
            race: Some(123),
            gender: None,
            animation: Some(124),
        }),
        Event::SpawnAdded(spawn()),
        Event::SpawnAdded(SpawnInfo {
            max_hp: None,
            pos: None,
            velocity: Velocity::default(),
            delta_heading: None,
            animation: None,
            equipment_models: None,
            ..spawn()
        }),
        Event::SpawnMoved {
            id: 120,
            pos: pos(121),
            velocity: velocity(125),
            delta_heading: Some(128),
            animation: None,
        },
        Event::SpawnRemoved { id: 130 },
        Event::SpawnRenamed {
            id: Some(130),
            old_name: "old spawn".into(),
            new_name: "new spawn".into(),
        },
        Event::SpawnKilled {
            deceased_id: 131,
            killer_id: 132,
        },
        Event::SpawnHp {
            id: 133,
            cur: -134,
            max: 135,
        },
        Event::StatSync {
            spawn_id: 140,
            wide: true,
            has_hp: true,
            hp_cur: -141,
            hp_max: 142,
            has_mana: true,
            mana_cur: -143,
            mana_max: 144,
            has_end: true,
            end_cur: -145,
            end_max: 146,
        },
        Event::SelfPos {
            pos: pos(150),
            spawn_id: 154,
            velocity: velocity(155),
            delta_heading: Some(158),
            animation: None,
        },
        Event::SpawnAnimation {
            spawn_id: 160,
            animation: 161,
        },
        Event::SpawnIllusion {
            spawn_id: 162,
            race: 163,
            gender: 164,
        },
        Event::GuildsInZone {
            guilds: vec![guild()],
        },
        Event::TimeOfDay {
            year: 170,
            month: 171,
            day: 172,
            hour: 173,
            minute: 174,
        },
        Event::ZoneTransition {
            character_name: "player".into(),
            zone_id: Some(175),
            instance_id: None,
            confirmed: true,
        },
        Event::ZoneChanged(ZoneInfo {
            short_name: "short".into(),
            long_name: "long".into(),
        }),
        Event::ZoneEnvironmentChanged(ZoneEnvironment {
            zone_file: "short.eqg".into(),
            experience_multiplier: 1.5,
            safe_x: 176.25,
            safe_y: -177.5,
            safe_z: 178.75,
        }),
        Event::PlayerProfile(profile()),
        Event::Stance {
            name: "stance".into(),
        },
        Event::Invocation {
            name: "invocation".into(),
        },
        Event::InspectAnswer {
            spawn_id: 180,
            item_names: vec!["first".into(), "second".into()],
            bio: "bio".into(),
        },
        Event::GuildRoster {
            guild_id: 181,
            members: vec![member()],
        },
        Event::ZoneServerInfo {
            host: "host".into(),
            port: 182,
        },
        Event::ItemSet {
            items: vec![item()],
        },
        Event::ItemLearned { item: item() },
        Event::GuildMotd {
            message: "motd".into(),
            sender: "sender".into(),
        },
        Event::GuildRankName {
            guild_id: 190,
            rank_index: 191,
            rank_name: "rank".into(),
        },
        Event::LoadoutSwap {
            spawn_id: 192,
            level: 193,
            class: 194,
            race: 195,
        },
        Event::Doors(vec![door()]),
        Event::GroundItemRemoved { drop_id: 200 },
        Event::GroundItem(GroundItemInfo {
            id: 201,
            actor_definition: "IT1_ACTORDEF".into(),
            position: point3(-202.0),
            heading: Some(203.5),
        }),
        Event::CorpseLocated {
            id: 204,
            position: point3(-205.0),
        },
        Event::ZonePoints(vec![
            ZonePointInfo {
                trigger_id: Some(206),
                actor_definition: Some("POKCABPORT500".into()),
                position: point3(207.0),
                heading: 208.5,
                destination_zone_id: Some(209),
                destination_instance_id: Some(210),
            },
            ZonePointInfo {
                trigger_id: None,
                actor_definition: None,
                position: point3(211.0),
                heading: 212.5,
                destination_zone_id: None,
                destination_instance_id: None,
            },
        ]),
        Event::Combat {
            source: 210,
            target: 211,
            kind: 212,
            damage: -213,
            spell_id: 214,
        },
        Event::SpawnCast {
            caster_id: 215,
            spell_id: 216,
            cast_time_ms: 217,
        },
        Event::Targeted { spawn_id: 218 },
        Event::Considered { spawn_id: 219 },
        Event::AaTable {
            desc_id: 220,
            title_sid: 221,
        },
        Event::Exp { exp: 222 },
        Event::AaExp {
            alt_exp: 223,
            aa_points: 224,
        },
        Event::Stamina {
            food: 225,
            water: 226,
        },
        Event::ManaUpdate { mana: 227 },
        Event::SkillUpdate {
            skill_id: 228,
            value: 229,
        },
        Event::LootTransaction {
            corpse_id: 230,
            item_id: 231,
            quantity: 232,
            coin_copper: 233,
            from_corpse: true,
        },
        Event::LootDrops {
            corpse_id: 234,
            corpse_name: "corpse".into(),
            items: vec![loot_item()],
        },
        Event::Money {
            platinum: 235,
            gold: 236,
            silver: 237,
            copper: 238,
        },
        Event::SimpleMessage {
            format_id: 240,
            color: 241,
        },
        Event::FormattedMessage {
            format_id: 242,
            color: 243,
            args: vec!["arg1".into(), "arg2".into()],
        },
        Event::SpecialMessage {
            color: 244,
            target: 245,
            source: "source".into(),
            message: "special".into(),
        },
        Event::LootMessage {
            color: 246,
            text: "loot text".into(),
            item_id: 247,
            item_name: "loot name".into(),
        },
        Event::Chat {
            channel: 248,
            from: "from".into(),
            target: "target".into(),
            text: "chat".into(),
            chat_color: 249,
            channel_name: "channel".into(),
        },
        Event::BuffList {
            owner: 250,
            entries: vec![buff()],
        },
        Event::GroupFollow {
            name: "follow".into(),
            level: 251,
        },
        Event::GroupDisband {
            yourname: "you".into(),
            membername: "member".into(),
        },
        Event::LevelUpdate {
            level: 252,
            level_old: 253,
            exp: 254,
        },
        Event::EnterWorld {
            character_name: "player".into(),
        },
    ]
}

fn unpos(pos: &ffi::EventPos) -> Pos {
    Pos {
        x: pos.x,
        y: pos.y,
        z: pos.z,
        heading_deg: pos.heading_deg,
    }
}

fn unpoint3(point: &ffi::EventPoint3) -> Point3 {
    Point3 {
        x: point.x,
        y: point.y,
        z: point.z,
    }
}

fn unvital(value: &ffi::EventVitalValue) -> VitalValue {
    VitalValue {
        current: value.current,
        maximum: value.has_maximum.then_some(value.maximum),
    }
}

fn unidentity(identity: &ffi::EventPlayerIdentity) -> PlayerIdentity {
    PlayerIdentity {
        spawn_id: identity.has_spawn_id.then_some(identity.spawn_id),
        name: identity.name.clone(),
        last_name: identity.last_name.clone(),
        race: identity.race,
        class_: identity.class_,
        deity: identity.deity,
        level: identity.level,
        class_mask: identity.class_mask,
    }
}

fn unspawn(spawn: &ffi::EventSpawnInfo) -> SpawnInfo {
    SpawnInfo {
        id: spawn.id,
        name: spawn.name.clone(),
        last_name: spawn.last_name.clone(),
        race: spawn.race,
        class_: spawn.class_,
        deity: spawn.deity,
        level: spawn.level,
        npc: spawn.npc,
        cur_hp: spawn.cur_hp,
        max_hp: spawn.has_max_hp.then_some(spawn.max_hp),
        guild_id: spawn.guild_id,
        guild_server_id: spawn.guild_server_id,
        class_mask: spawn.class_mask,
        pos: spawn.has_pos.then(|| unpos(&spawn.pos)),
        velocity: unvelocity(&spawn.velocity),
        delta_heading: spawn.has_delta_heading.then_some(spawn.delta_heading),
        animation: spawn.has_animation.then_some(spawn.animation),
        equipment_models: spawn.has_equipment_models.then(|| {
            spawn
                .equipment_models
                .as_slice()
                .try_into()
                .expect("CXX equipment model count")
        }),
    }
}

fn unvelocity(velocity: &ffi::EventVelocity) -> Velocity {
    Velocity {
        x: velocity.has_x.then_some(velocity.x),
        y: velocity.has_y.then_some(velocity.y),
        z: velocity.has_z.then_some(velocity.z),
    }
}

fn unprofile(profile: &ffi::EventProfileInfo) -> ProfileInfo {
    ProfileInfo {
        name: profile.name.clone(),
        last_name: profile.last_name.clone(),
        class_: profile.class_,
        level: profile.level,
        race: profile.race,
        deity: profile.deity,
        cur_hp: profile.cur_hp,
        mana: profile.mana,
        aa_ids: profile.aa_ids.clone(),
        aa_values: profile.aa_values.clone(),
        aa_spent: profile.aa_spent,
        skills: profile.skills.clone(),
        class_mask: profile.class_mask,
        str_: profile.str_,
        sta: profile.sta,
        cha: profile.cha,
        dex: profile.dex,
        int_: profile.int_,
        agi: profile.agi,
        wis: profile.wis,
        platinum: profile.platinum,
        gold: profile.gold,
        silver: profile.silver,
        copper: profile.copper,
    }
}

fn unguild(guild: &ffi::EventGuildInZone) -> GuildInZone {
    GuildInZone {
        guild_id: guild.guild_id,
        server_id: guild.server_id,
        name: guild.name.clone(),
    }
}

fn unmember(member: &ffi::EventGuildRosterMember) -> GuildRosterMember {
    GuildRosterMember {
        name: member.name.clone(),
        level: member.level,
        class: member.class_,
        class_mask: member.class_mask,
        rank: member.rank,
        last_on: member.last_on,
        banker: member.banker,
        alt: member.alt,
        full_member: member.full_member,
        public_note: member.public_note.clone(),
        zone_id: member.zone_id,
    }
}

fn unitem(item: &ffi::EventItemTemplate) -> ItemTemplate {
    ItemTemplate {
        serial: item.serial.clone(),
        name: item.name.clone(),
        lore_name: item.lore_name.clone(),
        item_id: item.item_id,
        icon: item.icon,
        slot_mask: item.slot_mask,
        container_id: item.container_id,
        container_slot: item.container_slot,
        parent_slot: item.parent_slot,
        stats: item.stats.clone(),
        resists: item.resists.clone(),
        hp: item.hp,
        mana: item.mana,
        endurance: item.endurance,
        ac: item.ac,
    }
}

#[allow(clippy::too_many_lines)]
fn reconstruct(batch: &ffi::SessionDecodeBatch) -> Vec<Event> {
    batch
        .events
        .iter()
        .map(|reference| {
            let i = reference.payload_index as usize;
            match reference.kind {
                ffi::SessionEventKind::SessionReset => Event::SessionReset {
                    reason: match batch.session_reset[i].reason {
                        ffi::EventSessionResetReason::EnterWorld => SessionResetReason::EnterWorld,
                        ffi::EventSessionResetReason::PlayerProfile => {
                            SessionResetReason::PlayerProfile
                        }
                        ffi::EventSessionResetReason::ZoneTransition => {
                            SessionResetReason::ZoneTransition
                        }
                        ffi::EventSessionResetReason::Explicit => SessionResetReason::Explicit,
                        _ => unreachable!("known reset reason"),
                    },
                },
                ffi::SessionEventKind::PlayerIdentityUpdated => {
                    Event::PlayerIdentityUpdated(unidentity(&batch.player_identity_updated[i]))
                }
                ffi::SessionEventKind::PlayerMoved => {
                    let p = &batch.player_moved[i];
                    Event::PlayerMoved {
                        spawn_id: p.has_spawn_id.then_some(p.spawn_id),
                        pos: unpos(&p.pos),
                    }
                }
                ffi::SessionEventKind::PlayerVitalsUpdated => {
                    let p = &batch.player_vitals_updated[i];
                    Event::PlayerVitalsUpdated(PlayerVitals {
                        health: p.has_health.then(|| unvital(&p.health)),
                        mana: p.has_mana.then(|| unvital(&p.mana)),
                        endurance: p.has_endurance.then(|| unvital(&p.endurance)),
                    })
                }
                ffi::SessionEventKind::SpawnHealthUpdated => {
                    let p = &batch.spawn_health_updated[i];
                    Event::SpawnHealthUpdated {
                        id: p.id,
                        current: p.current,
                        maximum: p.maximum,
                    }
                }
                ffi::SessionEventKind::PlayerDied => {
                    let p = &batch.player_died[i];
                    Event::PlayerDied {
                        killer_id: p.has_killer_id.then_some(p.killer_id),
                    }
                }
                ffi::SessionEventKind::SpawnDied => {
                    let p = &batch.spawn_died[i];
                    Event::SpawnDied {
                        id: p.id,
                        killer_id: p.has_killer_id.then_some(p.killer_id),
                    }
                }
                ffi::SessionEventKind::SpawnIdentityUpdated => {
                    let p = &batch.spawn_identity_updated[i];
                    Event::SpawnIdentityUpdated {
                        id: p.id,
                        level: p.level,
                        class_: p.class_,
                        race: p.race,
                    }
                }
                ffi::SessionEventKind::PlayerAppearanceUpdated => {
                    let p = &batch.player_appearance_updated[i];
                    Event::PlayerAppearanceUpdated(PlayerAppearance {
                        race: p.has_race.then_some(p.race),
                        gender: p.has_gender.then_some(p.gender),
                        animation: p.has_animation.then_some(p.animation),
                    })
                }
                ffi::SessionEventKind::SpawnAdded => {
                    Event::SpawnAdded(unspawn(&batch.spawn_added[i]))
                }
                ffi::SessionEventKind::SpawnMoved => {
                    let p = &batch.spawn_moved[i];
                    Event::SpawnMoved {
                        id: p.id,
                        pos: unpos(&p.pos),
                        velocity: unvelocity(&p.velocity),
                        delta_heading: p.has_delta_heading.then_some(p.delta_heading),
                        animation: p.has_animation.then_some(p.animation),
                    }
                }
                ffi::SessionEventKind::SpawnRemoved => Event::SpawnRemoved {
                    id: batch.spawn_removed[i].id,
                },
                ffi::SessionEventKind::SpawnRenamed => {
                    let p = &batch.spawn_renamed[i];
                    Event::SpawnRenamed {
                        id: p.has_id.then_some(p.id),
                        old_name: p.old_name.clone(),
                        new_name: p.new_name.clone(),
                    }
                }
                ffi::SessionEventKind::SpawnKilled => {
                    let p = &batch.spawn_killed[i];
                    Event::SpawnKilled {
                        deceased_id: p.deceased_id,
                        killer_id: p.killer_id,
                    }
                }
                ffi::SessionEventKind::SpawnHp => {
                    let p = &batch.spawn_hp[i];
                    Event::SpawnHp {
                        id: p.id,
                        cur: p.cur,
                        max: p.max,
                    }
                }
                ffi::SessionEventKind::StatSync => {
                    let p = &batch.stat_sync[i];
                    Event::StatSync {
                        spawn_id: p.spawn_id,
                        wide: p.wide,
                        has_hp: p.has_hp,
                        hp_cur: p.hp_cur,
                        hp_max: p.hp_max,
                        has_mana: p.has_mana,
                        mana_cur: p.mana_cur,
                        mana_max: p.mana_max,
                        has_end: p.has_end,
                        end_cur: p.end_cur,
                        end_max: p.end_max,
                    }
                }
                ffi::SessionEventKind::SelfPos => {
                    let p = &batch.self_pos[i];
                    Event::SelfPos {
                        pos: unpos(&p.pos),
                        spawn_id: p.spawn_id,
                        velocity: unvelocity(&p.velocity),
                        delta_heading: p.has_delta_heading.then_some(p.delta_heading),
                        animation: p.has_animation.then_some(p.animation),
                    }
                }
                ffi::SessionEventKind::SpawnAnimation => {
                    let p = &batch.spawn_animation[i];
                    Event::SpawnAnimation {
                        spawn_id: p.spawn_id,
                        animation: p.animation,
                    }
                }
                ffi::SessionEventKind::SpawnIllusion => {
                    let p = &batch.spawn_illusion[i];
                    Event::SpawnIllusion {
                        spawn_id: p.spawn_id,
                        race: p.race,
                        gender: p.gender,
                    }
                }
                ffi::SessionEventKind::GuildsInZone => Event::GuildsInZone {
                    guilds: batch.guilds_in_zone[i].guilds.iter().map(unguild).collect(),
                },
                ffi::SessionEventKind::TimeOfDay => {
                    let p = &batch.time_of_day[i];
                    Event::TimeOfDay {
                        year: p.year,
                        month: p.month,
                        day: p.day,
                        hour: p.hour,
                        minute: p.minute,
                    }
                }
                ffi::SessionEventKind::ZoneTransition => {
                    let p = &batch.zone_transition[i];
                    Event::ZoneTransition {
                        character_name: p.character_name.clone(),
                        zone_id: p.has_zone_id.then_some(p.zone_id),
                        instance_id: p.has_instance_id.then_some(p.instance_id),
                        confirmed: p.confirmed,
                    }
                }
                ffi::SessionEventKind::ZoneChanged => {
                    let p = &batch.zone_changed[i];
                    Event::ZoneChanged(ZoneInfo {
                        short_name: p.short_name.clone(),
                        long_name: p.long_name.clone(),
                    })
                }
                ffi::SessionEventKind::ZoneEnvironmentChanged => {
                    let p = &batch.zone_environment_changed[i];
                    Event::ZoneEnvironmentChanged(ZoneEnvironment {
                        zone_file: p.zone_file.clone(),
                        experience_multiplier: p.experience_multiplier,
                        safe_x: p.safe_x,
                        safe_y: p.safe_y,
                        safe_z: p.safe_z,
                    })
                }
                ffi::SessionEventKind::PlayerProfile => {
                    Event::PlayerProfile(unprofile(&batch.player_profile[i]))
                }
                ffi::SessionEventKind::Stance => Event::Stance {
                    name: batch.named[i].name.clone(),
                },
                ffi::SessionEventKind::Invocation => Event::Invocation {
                    name: batch.named[i].name.clone(),
                },
                ffi::SessionEventKind::InspectAnswer => {
                    let p = &batch.inspect_answer[i];
                    Event::InspectAnswer {
                        spawn_id: p.spawn_id,
                        item_names: p.item_names.clone(),
                        bio: p.bio.clone(),
                    }
                }
                ffi::SessionEventKind::GuildRoster => {
                    let p = &batch.guild_roster[i];
                    Event::GuildRoster {
                        guild_id: p.guild_id,
                        members: p.members.iter().map(unmember).collect(),
                    }
                }
                ffi::SessionEventKind::ZoneServerInfo => {
                    let p = &batch.zone_server_info[i];
                    Event::ZoneServerInfo {
                        host: p.host.clone(),
                        port: p.port,
                    }
                }
                ffi::SessionEventKind::ItemSet => Event::ItemSet {
                    items: batch.item_set[i].items.iter().map(unitem).collect(),
                },
                ffi::SessionEventKind::ItemLearned => Event::ItemLearned {
                    item: unitem(&batch.item_learned[i].item),
                },
                ffi::SessionEventKind::GuildMotd => {
                    let p = &batch.guild_motd[i];
                    Event::GuildMotd {
                        message: p.message.clone(),
                        sender: p.sender.clone(),
                    }
                }
                ffi::SessionEventKind::GuildRankName => {
                    let p = &batch.guild_rank_name[i];
                    Event::GuildRankName {
                        guild_id: p.guild_id,
                        rank_index: p.rank_index,
                        rank_name: p.rank_name.clone(),
                    }
                }
                ffi::SessionEventKind::LoadoutSwap => {
                    let p = &batch.loadout_swap[i];
                    Event::LoadoutSwap {
                        spawn_id: p.spawn_id,
                        level: p.level,
                        class: p.class_,
                        race: p.race,
                    }
                }
                ffi::SessionEventKind::Doors => Event::Doors(
                    batch.doors[i]
                        .doors
                        .iter()
                        .map(|p| DoorInfo {
                            id: p.id,
                            name: p.name.clone(),
                            position: unpoint3(&p.position),
                            heading: p.heading,
                            incline: p.incline,
                            size: p.size,
                            open_type: p.open_type,
                            state: p.state,
                            invert_state: p.invert_state,
                            zone_point_id: p.has_zone_point_id.then_some(p.zone_point_id),
                        })
                        .collect(),
                ),
                ffi::SessionEventKind::GroundItemRemoved => Event::GroundItemRemoved {
                    drop_id: batch.ground_item_removed[i].drop_id,
                },
                ffi::SessionEventKind::GroundItem => {
                    let p = &batch.ground_item[i];
                    Event::GroundItem(GroundItemInfo {
                        id: p.id,
                        actor_definition: p.actor_definition.clone(),
                        position: unpoint3(&p.position),
                        heading: p.has_heading.then_some(p.heading),
                    })
                }
                ffi::SessionEventKind::CorpseLocated => {
                    let p = &batch.corpse_located[i];
                    Event::CorpseLocated {
                        id: p.id,
                        position: unpoint3(&p.position),
                    }
                }
                ffi::SessionEventKind::ZonePoints => Event::ZonePoints(
                    batch.zone_points[i]
                        .points
                        .iter()
                        .map(|p| ZonePointInfo {
                            trigger_id: p.has_trigger_id.then_some(p.trigger_id),
                            actor_definition: p
                                .has_actor_definition
                                .then(|| p.actor_definition.clone()),
                            position: unpoint3(&p.position),
                            heading: p.heading,
                            destination_zone_id: p
                                .has_destination_zone_id
                                .then_some(p.destination_zone_id),
                            destination_instance_id: p
                                .has_destination_instance_id
                                .then_some(p.destination_instance_id),
                        })
                        .collect(),
                ),
                ffi::SessionEventKind::Combat => {
                    let p = &batch.combat[i];
                    Event::Combat {
                        source: p.source,
                        target: p.target,
                        kind: p.kind,
                        damage: p.damage,
                        spell_id: p.spell_id,
                    }
                }
                ffi::SessionEventKind::SpawnCast => {
                    let p = &batch.spawn_cast[i];
                    Event::SpawnCast {
                        caster_id: p.caster_id,
                        spell_id: p.spell_id,
                        cast_time_ms: p.cast_time_ms,
                    }
                }
                ffi::SessionEventKind::Targeted => Event::Targeted {
                    spawn_id: batch.spawn_id[i].id,
                },
                ffi::SessionEventKind::Considered => Event::Considered {
                    spawn_id: batch.spawn_id[i].id,
                },
                ffi::SessionEventKind::AaTable => {
                    let p = &batch.aa_table[i];
                    Event::AaTable {
                        desc_id: p.desc_id,
                        title_sid: p.title_sid,
                    }
                }
                ffi::SessionEventKind::Exp => Event::Exp {
                    exp: batch.exp[i].exp,
                },
                ffi::SessionEventKind::AaExp => {
                    let p = &batch.aa_exp[i];
                    Event::AaExp {
                        alt_exp: p.alt_exp,
                        aa_points: p.aa_points,
                    }
                }
                ffi::SessionEventKind::Stamina => {
                    let p = &batch.stamina[i];
                    Event::Stamina {
                        food: p.food,
                        water: p.water,
                    }
                }
                ffi::SessionEventKind::ManaUpdate => Event::ManaUpdate {
                    mana: batch.mana_update[i].mana,
                },
                ffi::SessionEventKind::SkillUpdate => {
                    let p = &batch.skill_update[i];
                    Event::SkillUpdate {
                        skill_id: p.skill_id,
                        value: p.value,
                    }
                }
                ffi::SessionEventKind::LootTransaction => {
                    let p = &batch.loot_transaction[i];
                    Event::LootTransaction {
                        corpse_id: p.corpse_id,
                        item_id: p.item_id,
                        quantity: p.quantity,
                        coin_copper: p.coin_copper,
                        from_corpse: p.from_corpse,
                    }
                }
                ffi::SessionEventKind::LootDrops => {
                    let p = &batch.loot_drops[i];
                    Event::LootDrops {
                        corpse_id: p.corpse_id,
                        corpse_name: p.corpse_name.clone(),
                        items: p
                            .items
                            .iter()
                            .map(|item| LootItemInfo {
                                name: item.name.clone(),
                                icon: item.icon,
                                item_id: item.item_id,
                            })
                            .collect(),
                    }
                }
                ffi::SessionEventKind::Money => {
                    let p = &batch.money[i];
                    Event::Money {
                        platinum: p.platinum,
                        gold: p.gold,
                        silver: p.silver,
                        copper: p.copper,
                    }
                }
                ffi::SessionEventKind::SimpleMessage => {
                    let p = &batch.simple_message[i];
                    Event::SimpleMessage {
                        format_id: p.format_id,
                        color: p.color,
                    }
                }
                ffi::SessionEventKind::FormattedMessage => {
                    let p = &batch.formatted_message[i];
                    Event::FormattedMessage {
                        format_id: p.format_id,
                        color: p.color,
                        args: p.args.clone(),
                    }
                }
                ffi::SessionEventKind::SpecialMessage => {
                    let p = &batch.special_message[i];
                    Event::SpecialMessage {
                        color: p.color,
                        target: p.target,
                        source: p.source.clone(),
                        message: p.message.clone(),
                    }
                }
                ffi::SessionEventKind::LootMessage => {
                    let p = &batch.loot_message[i];
                    Event::LootMessage {
                        color: p.color,
                        text: p.text.clone(),
                        item_id: p.item_id,
                        item_name: p.item_name.clone(),
                    }
                }
                ffi::SessionEventKind::Chat => {
                    let p = &batch.chat[i];
                    Event::Chat {
                        channel: p.channel,
                        from: p.from.clone(),
                        target: p.target.clone(),
                        text: p.text.clone(),
                        chat_color: p.chat_color,
                        channel_name: p.channel_name.clone(),
                    }
                }
                ffi::SessionEventKind::BuffList => {
                    let p = &batch.buff_list[i];
                    Event::BuffList {
                        owner: p.owner,
                        entries: p
                            .entries
                            .iter()
                            .map(|entry| BuffEntry {
                                spell_id: entry.spell_id,
                                remaining_ticks: entry.remaining_ticks,
                                slot: entry.slot,
                                caster: entry.caster.clone(),
                            })
                            .collect(),
                    }
                }
                ffi::SessionEventKind::GroupFollow => {
                    let p = &batch.group_follow[i];
                    Event::GroupFollow {
                        name: p.name.clone(),
                        level: p.level,
                    }
                }
                ffi::SessionEventKind::GroupDisband => {
                    let p = &batch.group_disband[i];
                    Event::GroupDisband {
                        yourname: p.yourname.clone(),
                        membername: p.membername.clone(),
                    }
                }
                ffi::SessionEventKind::LevelUpdate => {
                    let p = &batch.level_update[i];
                    Event::LevelUpdate {
                        level: p.level,
                        level_old: p.level_old,
                        exp: p.exp,
                    }
                }
                ffi::SessionEventKind::EnterWorld => Event::EnterWorld {
                    character_name: batch.enter_world[i].character_name.clone(),
                },
                _ => unreachable!("canonical fixture uses known SessionEventKind values"),
            }
        })
        .collect()
}

#[test]
fn canonical_fixture_preserves_every_event_variant_and_field() {
    let expected = canonical_events();
    let batch = translate_events(77, ffi::SessionDisposition::Decoded, expected.clone());
    assert_eq!(batch.protocol_generation, 77);
    assert!(batch.disposition == ffi::SessionDisposition::Decoded);
    assert_eq!(batch.events.len(), expected.len());
    assert_eq!(reconstruct(&batch), expected);
}

#[test]
fn session_resource_reports_unmapped_and_malformed_without_events() {
    let registry = session_protocol_registry_new("").unwrap();
    let mut session = session_new(&registry, linked_session_backend()).unwrap();
    let unmapped = session.decode(
        ffi::SessionStream::Zone,
        0,
        ffi::SessionDirection::ServerToClient,
        &[],
        1,
    );
    assert!(unmapped.disposition == ffi::SessionDisposition::Unmapped);
    assert!(unmapped.events.is_empty());

    let (opcode, direction) = malformed_fixture();
    let malformed = session.decode(ffi::SessionStream::Zone, opcode, direction, &[], 2);
    assert!(malformed.disposition == ffi::SessionDisposition::Malformed);
    assert!(malformed.events.is_empty());

    registry
        .0
        .replace_from_str(
            linked_backend(),
            "[[zone]]\nid='0001'\nname='OP_DoesNotExist'\n",
        )
        .unwrap();
    let unhandled = session.decode(
        ffi::SessionStream::Zone,
        1,
        ffi::SessionDirection::ServerToClient,
        &[],
        3,
    );
    assert!(unhandled.disposition == ffi::SessionDisposition::Unhandled);
    assert!(unhandled.events.is_empty());

    let wrong_backend = match linked_session_backend() {
        ffi::SessionBackend::Live => ffi::SessionBackend::Eql,
        ffi::SessionBackend::Test | ffi::SessionBackend::Eql => ffi::SessionBackend::Live,
        _ => unreachable!(),
    };
    assert!(session_new(&registry, wrong_backend).is_err());
}

fn linked_session_backend() -> ffi::SessionBackend {
    #[cfg(feature = "backend-live")]
    return ffi::SessionBackend::Live;
    #[cfg(feature = "backend-test")]
    return ffi::SessionBackend::Test;
    #[cfg(feature = "backend-eql")]
    return ffi::SessionBackend::Eql;
}

fn malformed_fixture() -> (u16, ffi::SessionDirection) {
    #[cfg(feature = "backend-live")]
    return (0x3635, ffi::SessionDirection::ServerToClient);
    #[cfg(feature = "backend-test")]
    return (0xaaed, ffi::SessionDirection::ServerToClient);
    #[cfg(feature = "backend-eql")]
    return (0x6afc, ffi::SessionDirection::ServerToClient);
}

#[cfg(feature = "backend-eql")]
#[test]
fn stateful_bridge_sessions_are_isolated_when_interleaved() {
    let registry = session_protocol_registry_new("").unwrap();
    let mut first = session_new(&registry, ffi::SessionBackend::Eql).unwrap();
    let mut second = session_new(&registry, ffi::SessionBackend::Eql).unwrap();
    let mut first_payload = [0u8; seq_backend_eql::player_self_pos::PAYLOAD_LEN];
    first_payload[2..4].copy_from_slice(&101u16.to_le_bytes());
    first_payload[10..14].copy_from_slice(&101.0f32.to_le_bytes());
    let mut second_payload = [0u8; seq_backend_eql::player_self_pos::PAYLOAD_LEN];
    second_payload[2..4].copy_from_slice(&202u16.to_le_bytes());
    second_payload[10..14].copy_from_slice(&202.0f32.to_le_bytes());

    let first_batch = first.decode(
        ffi::SessionStream::Zone,
        0x6987,
        ffi::SessionDirection::ClientToServer,
        &first_payload,
        10,
    );
    let second_batch = second.decode(
        ffi::SessionStream::Zone,
        0x6987,
        ffi::SessionDirection::ClientToServer,
        &second_payload,
        11,
    );
    assert!(!first_batch.player_moved[0].has_spawn_id);
    assert_eq!(first_batch.player_moved[0].pos.x, 101);
    assert!(!second_batch.player_moved[0].has_spawn_id);
    assert_eq!(second_batch.player_moved[0].pos.x, 202);

    first.flush(ffi::SessionFlushReason::Reset);
    let second_again = second.decode(
        ffi::SessionStream::Zone,
        0x6987,
        ffi::SessionDirection::ClientToServer,
        &second_payload,
        12,
    );
    assert!(!second_again.player_moved[0].has_spawn_id);
    assert_eq!(second_again.player_moved[0].pos.x, 202);

    registry
        .0
        .replace_from_str(
            seq_protocol_data::BackendId::Eql,
            "[[zone]]\nid='0001'\nname='OP_LootMessage'\n\n[[zone]]\nid='0002'\nname='OP_LootTransaction'\n",
        )
        .unwrap();
    let mut first = session_new(&registry, ffi::SessionBackend::Eql).unwrap();
    let mut second = session_new(&registry, ffi::SessionBackend::Eql).unwrap();
    let mut message = 286u32.to_le_bytes().to_vec();
    message.extend_from_slice(b"--You have looted a Sword from a goblin's corpse.--\0");
    first.decode(
        ffi::SessionStream::Zone,
        1,
        ffi::SessionDirection::ServerToClient,
        &message,
        20,
    );
    let mut confirmation = [0u8; 36];
    confirmation[0..2].copy_from_slice(&7u16.to_le_bytes());
    confirmation[4..8].copy_from_slice(&300u32.to_le_bytes());
    confirmation[12..16].copy_from_slice(&301u32.to_le_bytes());
    confirmation[16..20].copy_from_slice(&1u32.to_le_bytes());
    confirmation[20..24].copy_from_slice(&302u32.to_le_bytes());

    let second_confirmation = second.decode(
        ffi::SessionStream::Zone,
        2,
        ffi::SessionDirection::ServerToClient,
        &confirmation,
        21,
    );
    assert!(second_confirmation.loot_rows.is_empty());
    let first_confirmation = first.decode(
        ffi::SessionStream::Zone,
        2,
        ffi::SessionDirection::ServerToClient,
        &confirmation,
        22,
    );
    assert_eq!(first_confirmation.loot_rows.len(), 1);
    assert_eq!(first_confirmation.loot_rows[0].item_name, "Sword");
    assert_eq!(first_confirmation.loot_rows[0].sequence, 302);
}
