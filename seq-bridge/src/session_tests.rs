use super::*;
use seq_events::{
    ActiveBuff, AlternateAbilityDefinition, AlternateAbilityRank, AlternateAdvancementProgress,
    AlternateAdvancementSnapshot, BuffEntry, CastInterruptionReason, ChatMessage, ChatMessageKind,
    CorpseLootSnapshot, DoorInfo, DynamicZoneState, Event, ExperienceProgress, GroundItemInfo,
    GroupMember, GroupRosterState, GuildInZone, GuildMotdState, GuildRankNameEntry,
    GuildRankNamesState, GuildRosterMember, GuildRosterState, ItemLocation, ItemTemplate,
    LootAcquisition, LootItemInfo, MoneyBalance, PlayerAppearance, PlayerIdentity, PlayerVitals,
    Point3, Pos, ProfileInfo, SessionResetReason, SkillValue, SpawnInfo, Velocity, VitalValue,
    ZoneEnvironment, ZoneInfo, ZonePointInfo,
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
        aa_assigned: 31,
        aa_unspent: 32,
        aa_experience: 33,
        skills: vec![34, 35],
        class_mask: 36,
        str_: 37,
        sta: 38,
        cha: 39,
        dex: 40,
        int_: 41,
        agi: 42,
        wis: 43,
        platinum: 44,
        gold: 45,
        silver: 46,
        copper: 47,
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
        icon: Some(71),
        stack_count: Some(72),
        weight_tenths: Some(73),
        flags: Some(74),
        corruption: Some(-75),
        slot_mask: 76,
        container_id: 77,
        container_slot: 78,
        parent_slot: 79,
        stats: vec![-80, 81],
        resists: vec![-82, 83],
        hp: -84,
        mana: 85,
        endurance: -86,
        ac: 87,
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
        Event::GuildRosterWire {
            guild_id: 181,
            members: vec![member()],
            complete: false,
        },
        Event::ZoneServerInfo {
            host: "host".into(),
            port: 182,
        },
        Event::ItemSet {
            items: vec![item()],
        },
        Event::ItemLearned { item: item() },
        Event::InventorySnapshot {
            items: vec![item()],
        },
        Event::InventoryItemUpdated {
            item: item(),
            previous_location: Some(ItemLocation {
                container_id: 88,
                container_slot: 89,
                parent_slot: 90,
            }),
        },
        Event::InventoryItemUpdated {
            item: ItemTemplate {
                icon: None,
                stack_count: None,
                weight_tenths: None,
                flags: None,
                corruption: None,
                ..item()
            },
            previous_location: None,
        },
        Event::EquipmentSnapshot {
            items: vec![item()],
        },
        Event::EquipmentSlotUpdated {
            slot: 91,
            item: Some(item()),
        },
        Event::EquipmentSlotUpdated {
            slot: 92,
            item: None,
        },
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
        Event::CombatDamage {
            source_id: Some(210),
            target_id: None,
            kind: 212,
            damage: -213,
            spell_id: Some(214),
        },
        Event::SpellAction {
            source: 211,
            target: 212,
            spell_id: 213,
            caster_level: 65,
            kind: 0xe7,
        },
        Event::SpellActionResolved {
            source_id: None,
            target_id: Some(212),
            spell_id: 213,
            caster_level: Some(65),
            kind: 0xe7,
        },
        Event::SpellCastRequest {
            slot: -1,
            spell_id: 214,
            target_id: 0,
        },
        Event::SpawnCast {
            caster_id: 215,
            spell_id: 216,
            cast_time_ms: 217,
        },
        Event::SpellCastStarted {
            caster_id: Some(215),
            target_id: None,
            spell_id: 216,
            cast_time_ms: Some(217),
            slot: None,
        },
        Event::SpellCastInterrupted {
            caster_id: None,
            target_id: Some(218),
            spell_id: 219,
            reason: CastInterruptionReason::ReplayEnd,
        },
        Event::Targeted { spawn_id: 218 },
        Event::Considered { spawn_id: 219 },
        Event::AaTable {
            desc_id: 220,
            title_sid: 221,
        },
        Event::AlternateAbilityDefined(AlternateAbilityDefinition {
            ability_id: 222,
            title_string_id: 223,
        }),
        Event::Exp { exp: 222 },
        Event::ExperienceUpdated(ExperienceProgress {
            experience: 224,
            level: Some(225),
            previous_level: None,
        }),
        Event::ExperienceUpdated(ExperienceProgress {
            experience: 226,
            level: None,
            previous_level: Some(227),
        }),
        Event::AaExp {
            alt_exp: 223,
            aa_points: 224,
        },
        Event::AlternateAdvancementSnapshot(AlternateAdvancementSnapshot {
            purchased: vec![AlternateAbilityRank {
                ability_id: 228,
                rank: 229,
            }],
            spent_points: Some(230),
            assigned_points: None,
            unspent_points: 231,
            experience: 232,
        }),
        Event::AlternateAdvancementUpdated(AlternateAdvancementProgress {
            experience: 233,
            unspent_points: 234,
        }),
        Event::Stamina {
            food: 225,
            water: 226,
        },
        Event::ManaUpdate { mana: 227 },
        Event::SkillUpdate {
            skill_id: 228,
            value: 229,
        },
        Event::SkillsSnapshot {
            skills: vec![SkillValue {
                skill_id: 235,
                value: 236,
            }],
        },
        Event::SkillValueUpdated(SkillValue {
            skill_id: 237,
            value: 238,
        }),
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
        Event::CorpseLootSnapshot(Box::new(CorpseLootSnapshot {
            timestamp: 235,
            corpse_id: 236,
            corpse_name: "corpse snapshot".into(),
            corpse_name_normalized: "corpse snapshot".into(),
            zone_short: "zone_multi".into(),
            zone_base: "zone".into(),
            instance: "multi".into(),
            looter: "player".into(),
            items: vec![loot_item()],
        })),
        Event::LootAcquired(Box::new(LootAcquisition {
            timestamp: 237,
            item_name: "acquired item".into(),
            item_id: Some(238),
            quantity: 239,
            corpse_name: "a corpse".into(),
            corpse_name_normalized: "corpse".into(),
            corpse_id: None,
            zone_short: "zone_solo".into(),
            zone_base: "zone".into(),
            instance: "solo".into(),
            sold: true,
            coin_copper: 240,
            disposition: "sold".into(),
            looter: "player".into(),
            sequence: Some(241),
            from_corpse: false,
            complete: false,
        })),
        Event::LootAcquired(Box::new(LootAcquisition {
            timestamp: 242,
            item_name: "Coin".into(),
            item_id: None,
            quantity: 1,
            corpse_name: String::new(),
            corpse_name_normalized: String::new(),
            corpse_id: Some(243),
            zone_short: String::new(),
            zone_base: String::new(),
            instance: String::new(),
            sold: false,
            coin_copper: 244,
            disposition: "corpse_coin".into(),
            looter: String::new(),
            sequence: None,
            from_corpse: true,
            complete: true,
        })),
        Event::Money {
            platinum: 235,
            gold: 236,
            silver: 237,
            copper: 238,
        },
        Event::MoneyBalanceUpdated(MoneyBalance {
            platinum: 239,
            gold: 240,
            silver: 241,
            copper: 242,
        }),
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
        Event::BuffWire {
            spawn_id: 251,
            spell_id: 252,
            form: 2,
            slot: u8::MAX,
            duration_ticks: 253,
            change_type: 4,
        },
        Event::BuffAdded(ActiveBuff {
            owner_id: Some(254),
            spell_id: 255,
            remaining_ticks: Some(-1),
            slot: Some(3),
            caster_id: None,
            caster_name: Some("caster".into()),
        }),
        Event::BuffUpdated(ActiveBuff {
            owner_id: None,
            spell_id: 256,
            remaining_ticks: None,
            slot: None,
            caster_id: Some(257),
            caster_name: None,
        }),
        Event::BuffRemoved {
            owner_id: Some(258),
            spell_id: 259,
            slot: None,
        },
        Event::GroupFollow {
            name: "follow".into(),
            level: 251,
        },
        Event::GroupDisband {
            yourname: "you".into(),
            membername: "member".into(),
        },
        Event::ChatMessage(ChatMessage {
            kind: ChatMessageKind::Formatted,
            channel: 19,
            from: "speaker".into(),
            target: "listener".into(),
            text: "text".into(),
            chat_color: 320,
            channel_name: "General".into(),
            format_id: Some(1234),
            args: vec!["one".into(), "two".into()],
        }),
        Event::UcsRecord {
            channel_first: b'G',
            channel_rest: "eneral".into(),
            channel_run: "General".into(),
            sender: "sender".into(),
            message: "ucs".into(),
            spam: true,
        },
        Event::GroupRosterWire {
            group_id: 260,
            member_count: 2,
            names: vec!["self".into(), "peer".into()],
            complete: true,
        },
        Event::GroupRosterUpdated(GroupRosterState {
            group_id: Some(260),
            members: vec![GroupMember {
                slot: 2,
                name: "peer".into(),
                level: Some(65),
            }],
            complete: false,
        }),
        Event::GuildRosterUpdated(GuildRosterState {
            guild_id: 261,
            members: vec![member()],
            complete: true,
        }),
        Event::GuildMemberStatus {
            name: "guildmate".into(),
            zone_id: 262,
            instance_id: 263,
            last_on: 264,
        },
        Event::GuildMotdUpdated(GuildMotdState {
            guild_id: 265,
            message: "motd state".into(),
            sender: "setter".into(),
        }),
        Event::GuildRankNamesUpdated(GuildRankNamesState {
            guild_id: 266,
            ranks: vec![GuildRankNameEntry {
                rank_index: 3,
                rank_name: "Officer".into(),
            }],
        }),
        Event::DynamicZoneInfo {
            active: true,
            max_players: 54,
            expedition_name: "expedition".into(),
            leader_name: "leader".into(),
        },
        Event::DynamicZoneSwitch {
            active: true,
            zone_id: Some(267),
            instance_id: None,
            kind: Some(5),
            position: Some(point3(268.0)),
        },
        Event::DynamicZoneUpdated(DynamicZoneState {
            active: true,
            zone_id: Some(269),
            instance_id: Some(7),
            kind: None,
            position: None,
            max_players: Some(6),
            expedition_name: "dz".into(),
            leader_name: "lead".into(),
            complete: true,
        }),
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
        aa_assigned: profile.aa_assigned,
        aa_unspent: profile.aa_unspent,
        aa_experience: profile.aa_experience,
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
        icon: item.has_icon.then_some(item.icon),
        stack_count: item.has_stack_count.then_some(item.stack_count),
        weight_tenths: item.has_weight_tenths.then_some(item.weight_tenths),
        flags: item.has_flags.then_some(item.flags),
        corruption: item.has_corruption.then_some(item.corruption),
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
                ffi::SessionEventKind::InventorySnapshot => Event::InventorySnapshot {
                    items: batch.inventory_snapshot[i]
                        .items
                        .iter()
                        .map(unitem)
                        .collect(),
                },
                ffi::SessionEventKind::InventoryItemUpdated => {
                    let update = &batch.inventory_item_updated[i];
                    Event::InventoryItemUpdated {
                        item: unitem(&update.item),
                        previous_location: update.has_previous_location.then_some(ItemLocation {
                            container_id: update.previous_location.container_id,
                            container_slot: update.previous_location.container_slot,
                            parent_slot: update.previous_location.parent_slot,
                        }),
                    }
                }
                ffi::SessionEventKind::EquipmentSnapshot => Event::EquipmentSnapshot {
                    items: batch.equipment_snapshot[i]
                        .items
                        .iter()
                        .map(unitem)
                        .collect(),
                },
                ffi::SessionEventKind::EquipmentSlotUpdated => {
                    let update = &batch.equipment_slot_updated[i];
                    Event::EquipmentSlotUpdated {
                        slot: update.slot,
                        item: update.has_item.then(|| unitem(&update.item)),
                    }
                }
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
                ffi::SessionEventKind::CombatDamage => {
                    let p = &batch.combat_damage[i];
                    Event::CombatDamage {
                        source_id: p.has_source_id.then_some(p.source_id),
                        target_id: p.has_target_id.then_some(p.target_id),
                        kind: p.kind,
                        damage: p.damage,
                        spell_id: p.has_spell_id.then_some(p.spell_id),
                    }
                }
                ffi::SessionEventKind::SpellAction => {
                    let p = &batch.spell_action[i];
                    Event::SpellAction {
                        source: p.source,
                        target: p.target,
                        spell_id: p.spell_id,
                        caster_level: p.caster_level,
                        kind: p.kind,
                    }
                }
                ffi::SessionEventKind::SpellActionResolved => {
                    let p = &batch.spell_action_resolved[i];
                    Event::SpellActionResolved {
                        source_id: p.has_source_id.then_some(p.source_id),
                        target_id: p.has_target_id.then_some(p.target_id),
                        spell_id: p.spell_id,
                        caster_level: p.has_caster_level.then_some(p.caster_level),
                        kind: p.kind,
                    }
                }
                ffi::SessionEventKind::SpellCastRequest => {
                    let p = &batch.spell_cast_request[i];
                    Event::SpellCastRequest {
                        slot: p.slot,
                        spell_id: p.spell_id,
                        target_id: p.target_id,
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
                ffi::SessionEventKind::SpellCastStarted => {
                    let p = &batch.spell_cast_started[i];
                    Event::SpellCastStarted {
                        caster_id: p.has_caster_id.then_some(p.caster_id),
                        target_id: p.has_target_id.then_some(p.target_id),
                        spell_id: p.spell_id,
                        cast_time_ms: p.has_cast_time_ms.then_some(p.cast_time_ms),
                        slot: p.has_slot.then_some(p.slot),
                    }
                }
                ffi::SessionEventKind::SpellCastInterrupted => {
                    let p = &batch.spell_cast_interrupted[i];
                    Event::SpellCastInterrupted {
                        caster_id: p.has_caster_id.then_some(p.caster_id),
                        target_id: p.has_target_id.then_some(p.target_id),
                        spell_id: p.spell_id,
                        reason: match p.reason {
                            ffi::EventCastInterruptionReason::ServerMessage => {
                                CastInterruptionReason::ServerMessage
                            }
                            ffi::EventCastInterruptionReason::Superseded => {
                                CastInterruptionReason::Superseded
                            }
                            ffi::EventCastInterruptionReason::SessionReset => {
                                CastInterruptionReason::SessionReset
                            }
                            ffi::EventCastInterruptionReason::ReplayEnd => {
                                CastInterruptionReason::ReplayEnd
                            }
                            ffi::EventCastInterruptionReason::Shutdown => {
                                CastInterruptionReason::Shutdown
                            }
                            _ => unreachable!("known interruption reason"),
                        },
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
                ffi::SessionEventKind::AlternateAbilityDefined => {
                    let definition = &batch.alternate_ability_defined[i];
                    Event::AlternateAbilityDefined(AlternateAbilityDefinition {
                        ability_id: definition.ability_id,
                        title_string_id: definition.title_string_id,
                    })
                }
                ffi::SessionEventKind::Exp => Event::Exp {
                    exp: batch.exp[i].exp,
                },
                ffi::SessionEventKind::ExperienceUpdated => {
                    let progress = &batch.experience_updated[i];
                    Event::ExperienceUpdated(ExperienceProgress {
                        experience: progress.experience,
                        level: progress.has_level.then_some(progress.level),
                        previous_level: progress
                            .has_previous_level
                            .then_some(progress.previous_level),
                    })
                }
                ffi::SessionEventKind::AaExp => {
                    let p = &batch.aa_exp[i];
                    Event::AaExp {
                        alt_exp: p.alt_exp,
                        aa_points: p.aa_points,
                    }
                }
                ffi::SessionEventKind::AlternateAdvancementSnapshot => {
                    let snapshot = &batch.alternate_advancement_snapshot[i];
                    Event::AlternateAdvancementSnapshot(AlternateAdvancementSnapshot {
                        purchased: snapshot
                            .purchased
                            .iter()
                            .map(|rank| AlternateAbilityRank {
                                ability_id: rank.ability_id,
                                rank: rank.rank,
                            })
                            .collect(),
                        spent_points: snapshot.has_spent_points.then_some(snapshot.spent_points),
                        assigned_points: snapshot
                            .has_assigned_points
                            .then_some(snapshot.assigned_points),
                        unspent_points: snapshot.unspent_points,
                        experience: snapshot.experience,
                    })
                }
                ffi::SessionEventKind::AlternateAdvancementUpdated => {
                    let progress = &batch.alternate_advancement_updated[i];
                    Event::AlternateAdvancementUpdated(AlternateAdvancementProgress {
                        experience: progress.experience,
                        unspent_points: progress.unspent_points,
                    })
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
                ffi::SessionEventKind::SkillsSnapshot => Event::SkillsSnapshot {
                    skills: batch.skills_snapshot[i]
                        .skills
                        .iter()
                        .map(|skill| SkillValue {
                            skill_id: skill.skill_id,
                            value: skill.value,
                        })
                        .collect(),
                },
                ffi::SessionEventKind::SkillValueUpdated => {
                    let skill = &batch.skill_value_updated[i];
                    Event::SkillValueUpdated(SkillValue {
                        skill_id: skill.skill_id,
                        value: skill.value,
                    })
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
                ffi::SessionEventKind::CorpseLootSnapshot => {
                    let p = &batch.corpse_loot_snapshot[i];
                    Event::CorpseLootSnapshot(Box::new(CorpseLootSnapshot {
                        timestamp: p.timestamp,
                        corpse_id: p.corpse_id,
                        corpse_name: p.corpse_name.clone(),
                        corpse_name_normalized: p.corpse_name_normalized.clone(),
                        zone_short: p.zone_short.clone(),
                        zone_base: p.zone_base.clone(),
                        instance: p.instance.clone(),
                        looter: p.looter.clone(),
                        items: p
                            .items
                            .iter()
                            .map(|item| LootItemInfo {
                                name: item.name.clone(),
                                icon: item.icon,
                                item_id: item.item_id,
                            })
                            .collect(),
                    }))
                }
                ffi::SessionEventKind::LootAcquired => {
                    let p = &batch.loot_acquired[i];
                    Event::LootAcquired(Box::new(LootAcquisition {
                        timestamp: p.timestamp,
                        item_name: p.item_name.clone(),
                        item_id: p.has_item_id.then_some(p.item_id),
                        quantity: p.quantity,
                        corpse_name: p.corpse_name.clone(),
                        corpse_name_normalized: p.corpse_name_normalized.clone(),
                        corpse_id: p.has_corpse_id.then_some(p.corpse_id),
                        zone_short: p.zone_short.clone(),
                        zone_base: p.zone_base.clone(),
                        instance: p.instance.clone(),
                        sold: p.sold,
                        coin_copper: p.coin_copper,
                        disposition: p.disposition.clone(),
                        looter: p.looter.clone(),
                        sequence: p.has_sequence.then_some(p.sequence),
                        from_corpse: p.from_corpse,
                        complete: p.complete,
                    }))
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
                ffi::SessionEventKind::MoneyBalanceUpdated => {
                    let balance = &batch.money_balance_updated[i];
                    Event::MoneyBalanceUpdated(MoneyBalance {
                        platinum: balance.platinum,
                        gold: balance.gold,
                        silver: balance.silver,
                        copper: balance.copper,
                    })
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
                ffi::SessionEventKind::BuffWire => {
                    let p = &batch.buff_wire[i];
                    Event::BuffWire {
                        spawn_id: p.spawn_id,
                        spell_id: p.spell_id,
                        form: p.form,
                        slot: p.slot,
                        duration_ticks: p.duration_ticks,
                        change_type: p.change_type,
                    }
                }
                ffi::SessionEventKind::BuffAdded | ffi::SessionEventKind::BuffUpdated => {
                    let p = if reference.kind == ffi::SessionEventKind::BuffAdded {
                        &batch.buff_added[i]
                    } else {
                        &batch.buff_updated[i]
                    };
                    let buff = ActiveBuff {
                        owner_id: p.has_owner_id.then_some(p.owner_id),
                        spell_id: p.spell_id,
                        remaining_ticks: p.has_remaining_ticks.then_some(p.remaining_ticks),
                        slot: p.has_slot.then_some(p.slot),
                        caster_id: p.has_caster_id.then_some(p.caster_id),
                        caster_name: p.has_caster_name.then(|| p.caster_name.clone()),
                    };
                    if reference.kind == ffi::SessionEventKind::BuffAdded {
                        Event::BuffAdded(buff)
                    } else {
                        Event::BuffUpdated(buff)
                    }
                }
                ffi::SessionEventKind::BuffRemoved => {
                    let p = &batch.buff_removed[i];
                    Event::BuffRemoved {
                        owner_id: p.has_owner_id.then_some(p.owner_id),
                        spell_id: p.spell_id,
                        slot: p.has_slot.then_some(p.slot),
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
                ffi::SessionEventKind::ChatMessage => {
                    let p = &batch.chat_message[i];
                    let kind = match p.kind {
                        ffi::EventChatMessageKind::Common => ChatMessageKind::Common,
                        ffi::EventChatMessageKind::Simple => ChatMessageKind::Simple,
                        ffi::EventChatMessageKind::Formatted => ChatMessageKind::Formatted,
                        ffi::EventChatMessageKind::Special => ChatMessageKind::Special,
                        ffi::EventChatMessageKind::Loot => ChatMessageKind::Loot,
                        ffi::EventChatMessageKind::Ucs => ChatMessageKind::Ucs,
                        _ => unreachable!(),
                    };
                    Event::ChatMessage(ChatMessage {
                        kind,
                        channel: p.channel,
                        from: p.from.clone(),
                        target: p.target.clone(),
                        text: p.text.clone(),
                        chat_color: p.chat_color,
                        channel_name: p.channel_name.clone(),
                        format_id: p.has_format_id.then_some(p.format_id),
                        args: p.args.clone(),
                    })
                }
                ffi::SessionEventKind::UcsRecord => {
                    let p = &batch.ucs_record[i];
                    Event::UcsRecord {
                        channel_first: p.channel_first,
                        channel_rest: p.channel_rest.clone(),
                        channel_run: p.channel_run.clone(),
                        sender: p.sender.clone(),
                        message: p.message.clone(),
                        spam: p.spam,
                    }
                }
                ffi::SessionEventKind::GroupRosterWire => {
                    let p = &batch.group_roster_wire[i];
                    Event::GroupRosterWire {
                        group_id: p.group_id,
                        member_count: p.member_count,
                        names: p.names.clone(),
                        complete: p.complete,
                    }
                }
                ffi::SessionEventKind::GroupRosterUpdated => {
                    let p = &batch.group_roster_updated[i];
                    Event::GroupRosterUpdated(GroupRosterState {
                        group_id: p.has_group_id.then_some(p.group_id),
                        members: p
                            .members
                            .iter()
                            .map(|member| GroupMember {
                                slot: member.slot,
                                name: member.name.clone(),
                                level: member.has_level.then_some(member.level),
                            })
                            .collect(),
                        complete: p.complete,
                    })
                }
                ffi::SessionEventKind::GuildRosterUpdated => {
                    let p = &batch.guild_roster_updated[i];
                    Event::GuildRosterUpdated(GuildRosterState {
                        guild_id: p.guild_id,
                        members: p.members.iter().map(unmember).collect(),
                        complete: p.complete,
                    })
                }
                ffi::SessionEventKind::GuildRosterWire => {
                    let p = &batch.guild_roster_wire[i];
                    Event::GuildRosterWire {
                        guild_id: p.guild_id,
                        members: p.members.iter().map(unmember).collect(),
                        complete: p.complete,
                    }
                }
                ffi::SessionEventKind::GuildMemberStatus => {
                    let p = &batch.guild_member_status[i];
                    Event::GuildMemberStatus {
                        name: p.name.clone(),
                        zone_id: p.zone_id,
                        instance_id: p.instance_id,
                        last_on: p.last_on,
                    }
                }
                ffi::SessionEventKind::GuildMotdUpdated => {
                    let p = &batch.guild_motd_updated[i];
                    Event::GuildMotdUpdated(GuildMotdState {
                        guild_id: p.guild_id,
                        message: p.message.clone(),
                        sender: p.sender.clone(),
                    })
                }
                ffi::SessionEventKind::GuildRankNamesUpdated => {
                    let p = &batch.guild_rank_names_updated[i];
                    Event::GuildRankNamesUpdated(GuildRankNamesState {
                        guild_id: p.guild_id,
                        ranks: p
                            .ranks
                            .iter()
                            .map(|rank| GuildRankNameEntry {
                                rank_index: rank.rank_index,
                                rank_name: rank.rank_name.clone(),
                            })
                            .collect(),
                    })
                }
                ffi::SessionEventKind::DynamicZoneInfo => {
                    let p = &batch.dynamic_zone_info[i];
                    Event::DynamicZoneInfo {
                        active: p.active,
                        max_players: p.max_players,
                        expedition_name: p.expedition_name.clone(),
                        leader_name: p.leader_name.clone(),
                    }
                }
                ffi::SessionEventKind::DynamicZoneSwitch => {
                    let p = &batch.dynamic_zone_switch[i];
                    Event::DynamicZoneSwitch {
                        active: p.active,
                        zone_id: p.has_zone_id.then_some(p.zone_id),
                        instance_id: p.has_instance_id.then_some(p.instance_id),
                        kind: p.has_kind.then_some(p.kind),
                        position: p.has_position.then(|| unpoint3(&p.position)),
                    }
                }
                ffi::SessionEventKind::DynamicZoneUpdated => {
                    let p = &batch.dynamic_zone_updated[i];
                    Event::DynamicZoneUpdated(DynamicZoneState {
                        active: p.active,
                        zone_id: p.has_zone_id.then_some(p.zone_id),
                        instance_id: p.has_instance_id.then_some(p.instance_id),
                        kind: p.has_kind.then_some(p.kind),
                        position: p.has_position.then(|| unpoint3(&p.position)),
                        max_players: p.has_max_players.then_some(p.max_players),
                        expedition_name: p.expedition_name.clone(),
                        leader_name: p.leader_name.clone(),
                        complete: p.complete,
                    })
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
fn encode_ucs(plain: &[u8]) -> Vec<u8> {
    let mut encoded = plain.to_vec();
    for index in 4..plain.len() {
        encoded[index] = plain[index] ^ encoded[index - 4];
    }
    encoded
}

#[cfg(feature = "backend-eql")]
#[test]
fn stateful_bridge_owns_ucs_channel_recovery() {
    let registry = session_protocol_registry_new("").unwrap();
    let mut session = session_new(&registry, ffi::SessionBackend::Eql).unwrap();
    let mut plain = vec![1, 2, 3, 4];
    plain.extend_from_slice(b"General\0Server.Alice\0hello\0SPAM:7:0\0");
    let batch = session.decode_ucs(ffi::SessionDirection::ServerToClient, &encode_ucs(&plain));
    assert!(batch.disposition == ffi::SessionDisposition::Decoded);
    assert_eq!(batch.events.len(), 2);
    assert!(batch.events[0].kind == ffi::SessionEventKind::UcsRecord);
    assert!(batch.events[1].kind == ffi::SessionEventKind::ChatMessage);
    assert!(batch.ucs_record[0].spam);
    assert!(batch.chat_message[0].kind == ffi::EventChatMessageKind::Ucs);
    assert_eq!(batch.chat_message[0].channel_name, "General");
    assert_eq!(batch.chat_message[0].text, "(SPAM) hello");

    let outbound = session.decode_ucs(ffi::SessionDirection::ClientToServer, &[0; 20]);
    assert!(outbound.disposition == ffi::SessionDisposition::Ignored);
    assert!(outbound.events.is_empty());
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
    assert!(second_confirmation.loot_acquired.is_empty());
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
    assert!(first_confirmation.loot_rows[0].complete);
    assert_eq!(first_confirmation.loot_acquired.len(), 1);
    let acquired = &first_confirmation.loot_acquired[0];
    assert_eq!(acquired.timestamp, 20);
    assert_eq!(acquired.item_name, "Sword");
    assert!(acquired.has_item_id);
    assert_eq!(acquired.item_id, 300);
    assert!(acquired.has_corpse_id);
    assert_eq!(acquired.corpse_id, 301);
    assert!(acquired.has_sequence);
    assert_eq!(acquired.sequence, 302);
    assert!(acquired.complete);

    let orphan = second.flush(ffi::SessionFlushReason::ReplayEnd);
    assert_eq!(orphan.loot_acquired.len(), 1);
    assert!(!orphan.loot_acquired[0].complete);
    assert_eq!(orphan.loot_acquired[0].timestamp, 21);
}
