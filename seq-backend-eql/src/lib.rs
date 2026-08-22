//! Self-contained EverQuest Legends decode surface.
//!
//! **eql depends on nothing from the Live decode stack.** EQ Legends is a
//! separate server that merely shares wire ancestry with Live *today*; to keep
//! a Live-only wire patch from silently corrupting eql, this crate vendors its
//! own copy of every parser + output struct (the modules below, forked from
//! `seq-decode`) and reads them through its own PINNED in-crate `eqstructs`
//! layouts. `seq-bridge`'s `backend-eql` feature routes every `decode_*` here —
//! there is no eql → seq-decode edge.
//!
//! Cite opcodes by NAME, never by id — EQL rotates ids nearly every patch, and
//! `scry-cpp/conf/eql/opcodes.toml` is the only map. Retired ids may be
//! named AS history.
//!
//! Two kinds of module live here:
//!   * eql's OWN byte-offset parsers for the opcodes whose Legends wire diverges
//!     from Live — `parse_spawn` / `parse_player_profile` / `parse_player_self_pos`
//!     / `parse_new_zone` / `parse_consider` below. Each takes the SAME canonical
//!     name as its Live counterpart in `seq-decode` (uniform surface — only the
//!     impl differs), so the bridge's `backend` alias routes to them with no
//!     per-opcode cfg. The lone exception is `decode_spawn`: eql's decoded output
//!     shape (x/y/z) differs from Live's raw `Spawn`, so its bridge fn keeps a
//!     cfg-split — both branches still call the backend's `parse_spawn`.
//!     /loc-confirmed; see `OPCODES_LEGENDS.md`.
//!   * pinned copies of the shared parsers (identical to Live *today*) that we
//!     now OWN — when eql and Live diverge, edit only the copy here.
//!
//! Field offsets/scales shuffle per patch; re-derive from captures, don't
//! memorize. **eql offsets below are the 2026-07-07 post-patch layout.**

use thiserror::Error;

/// eql's OWN pinned struct layouts (module `eqstructs`, a frozen fork of the
/// live bindings — see `eqstructs.rs`/`bindings.rs`). The vendored parser
/// modules reference `crate::eqstructs::<name>`; nothing here tracks Live's
/// generated bindings.
pub(crate) mod eqstructs;

// Vendored parser + output-struct modules (forked from seq-decode; eql-owned).
pub mod action;
pub mod action2;
pub mod action_alt;
pub mod backend;
pub mod buff;
pub mod channel_message;
pub mod click_object;
pub mod client_target;
pub mod consider;
pub mod corpse_loc;
pub mod cursor;
pub mod death;
pub mod delete_spawn;
pub mod dz_info;
pub mod dz_switch_info;
pub mod end_update;
pub mod exp_update;
pub mod formatted_message;
pub mod ground_spawn;
pub mod group_disband;
pub mod group_follow;
pub mod group_member_list;
pub mod group_roster;
pub mod guild_expanded_info;
pub mod guild_in_zone;
pub mod guild_motd;
pub mod guild_roster;
pub mod hp_update;
pub mod illusion;
pub mod item_packet;
pub mod level_update;
pub mod links;
pub mod loadout_swap;
pub mod loot_drops;
pub mod loot_message;
pub mod loot_track;
pub mod loot_transaction;
pub mod mana_change;
pub mod mob_health;
pub mod mob_update;
pub mod money_update;
pub mod new_zone;
pub mod npc_move_update;
pub mod player_profile;
pub mod player_self_pos;
pub mod player_spawn_pos;
pub mod remove_spawn;
pub mod self_pos_breadcrumb;
pub mod self_track;
pub mod simple_message;
pub mod skill_update;
pub mod spawn_appearance;
pub mod spawn_door;
pub mod spawn_rename;
pub mod special_message;
pub mod stamina;
pub mod start_cast;
pub mod ucs_chat;
pub mod wear_change;
pub mod zone_change;
pub mod zone_point;
pub mod zone_server_info;

// Full public-API mirror of seq-decode, so seq-bridge can alias this crate in
// place of seq-decode for the shared decoders (identical fn + struct names).
pub use action::{parse_action, Action, ActionError};
pub use action2::{parse_action2, Action2, Action2Error};
pub use action_alt::{parse_action_alt, ActionAlt, ActionAltError};
pub use buff::{parse_buff, Buff, BuffError};
pub use channel_message::{parse_channel_message, ChannelMessage, ChannelMessageError};
pub use click_object::{parse_click_object, ClickObject, ClickObjectError};
pub use client_target::{parse_client_target, ClientTarget, ClientTargetError};
// consider: eql provides the canonical `parse_consider` itself (below); the
// vendored Live parser stays available as `consider::parse_consider`.
pub use consider::{Consider, ConsiderError};
pub use corpse_loc::{parse_corpse_loc, CorpseLoc, CorpseLocError};
pub use death::{parse_death, Death, DeathError};
pub use delete_spawn::{
    parse_delete_spawn, DeleteSpawn, DeleteSpawnError, PAYLOAD_LEN as DELETE_SPAWN_LEN,
};
pub use dz_info::{parse_dz_info, DzInfo, DzInfoError};
pub use dz_switch_info::{parse_dz_switch_info, DzSwitch, DzSwitchError};
pub use end_update::{parse_end_update, EndUpdate, EndUpdateError};
pub use exp_update::{parse_exp_update, ExpUpdate, ExpUpdateError};
pub use formatted_message::{parse_formatted_message, FormattedMessage, FormattedMessageError};
pub use ground_spawn::{parse_ground_spawn, GroundSpawn, GroundSpawnError};
pub use group_disband::{parse_group_disband, GroupDisband, GroupDisbandError};
pub use group_follow::{parse_group_follow, GroupFollow, GroupFollowError};
pub use group_roster::{parse_group_roster, GroupRoster, GroupRosterError};
pub use loot_drops::{parse_loot_drops, LootDrops, LootDropsError};
pub use loot_message::{parse_loot_message, LootMessage, LootMessageError};
pub use loot_track::{LootRow, LootSource, LootTracker};
pub use loot_transaction::{parse_loot_transaction, LootTransaction, LootTransactionError};
pub use money_update::{parse_money_update, MoneyUpdate, MoneyUpdateError};
// Vendored 18B `hpNpcUpdateStruct` parser: eql never sees Live's fixed HP
// struct (OP_StatSync is the multiplexed stat channel decoded by `parse_stat_sync`
// below), so the shared `decode_hp_update` FFI is stubbed inert for eql in the
// bridge. Retained only so the module stays byte-identical to the Live fork.
pub use hp_update::{parse_hp_update, HpUpdate, HpUpdateError};
pub use illusion::{parse_illusion, Illusion, IllusionError};
pub use level_update::{parse_level_update, LevelUpdate, LevelUpdateError};
pub use loadout_swap::{parse_loadout_swap, LoadoutSwap, LoadoutSwapError};
pub use mana_change::{parse_mana_change, ManaChange, ManaChangeError};
pub use mob_health::{parse_mob_health, MobHealth, MobHealthError};
pub use mob_update::{parse_mob_update, MobUpdate, ParseError, PAYLOAD_LEN as MOB_UPDATE_LEN};
pub use new_zone::{NewZone, NewZoneError}; // eql owns canonical `parse_new_zone` (below)
pub use npc_move_update::{parse_npc_move_update, NpcMoveUpdate, NpcMoveUpdateError};
pub use player_profile::{PlayerProfile, PlayerProfileError}; // eql owns canonical `parse_player_profile` (below)
pub use player_self_pos::{parse_player_self_pos, PlayerSelfPos, PlayerSelfPosError}; // module owns the canonical parser (validates against its own PAYLOAD_LEN, same const the size override reads)
pub use player_spawn_pos::{parse_player_spawn_pos, PlayerSpawnPos, PlayerSpawnPosError};
pub use remove_spawn::{parse_remove_spawn, RemoveSpawn, RemoveSpawnError};
pub use self_pos_breadcrumb::{parse_self_pos_breadcrumb, BreadcrumbPoint, SelfPosBreadcrumb};
pub use self_track::{SelfPosRouting, SelfStat, SelfTracker, SpawnRouting, SAME_BATCH};
pub use simple_message::{parse_simple_message, SimpleMessage, SimpleMessageError};
pub use skill_update::{parse_skill_update, SkillUpdate, SkillUpdateError};
pub use spawn_appearance::{parse_spawn_appearance, SpawnAppearance, SpawnAppearanceError};
pub use spawn_door::{parse_door, Door, DoorError};
pub use spawn_rename::{parse_spawn_rename, SpawnRename, SpawnRenameError};
pub use special_message::{parse_special_message, SpecialMessage, SpecialMessageError};
pub use stamina::{parse_stamina, Stamina, StaminaError};
pub use start_cast::{parse_start_cast, StartCast, StartCastError};
pub use ucs_chat::{parse_ucs_channels, parse_ucs_chat, UcsRecord};
pub use wear_change::{parse_wear_change, WearChange, WearChangeError};
pub use zone_change::{parse_zone_change, ZoneChange, ZoneChangeError};
pub use zone_point::{parse_zone_point, ZonePoint, ZonePointError};

/// Decode a NUL-padded byte buffer into an owned `String`. The vendored parser
/// modules call this as `crate::cstr_field` (copied from seq-decode's helper).
pub(crate) fn cstr_field(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

// eql's own diverged parsers below return the vendored output structs, brought
// into scope by the `pub use` re-exports above: Consider, NewZone,
// PlayerProfile, PlayerSelfPos.

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("payload too short: {0} bytes")]
    Short(usize),
    #[error("unexpected payload length: {0} bytes")]
    BadLength(usize),
}

// Little-endian scalar reads at a byte offset. Callers length-guard first, so
// the fixed indexing below stays in bounds.
#[inline]
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
#[inline]
fn rd_i64(b: &[u8], o: usize) -> i64 {
    i64::from_le_bytes([
        b[o],
        b[o + 1],
        b[o + 2],
        b[o + 3],
        b[o + 4],
        b[o + 5],
        b[o + 6],
        b[o + 7],
    ])
}

/// Latin-1 → `String` (each byte is a codepoint), matching the daemon's
/// `QString::fromLatin1`.
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// The eql-relevant subset of a spawn. `Spawn` (the Live struct) can't derive
/// `Default` (it holds a `[u32; 45]` equipment array), so eql uses this small
/// struct and the uniform `decode_spawn` bridge maps it into `ffi::Spawn`
/// (decoded x/y/z + hp; Live's raw equipment/position arrays stay zero).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZoneSpawn {
    pub id: u16,
    pub name: String,
    pub last_name: String,
    pub title: String,
    pub suffix: String,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    /// h2048 heading (0..2047) — high 13 bits of the middle coord word.
    pub heading: u16,
    pub level: u8,
    pub cur_hp: u8,
    pub max_hp: u8,
    pub race: u32,
    pub class_: u32,
    pub class_mask: u32,
    pub deity: u32,
    pub guild_id: u32,
    pub guild_server_id: u32,
    pub pet_owner_id: u32,
    pub npc: u8,
    pub body_type: u32,
    pub holding: u8,
    pub state: u8,
    pub light: u8,
}

// Bounds-checked LE/BE reads at an absolute offset for the variable-length
// tail walk. Unlike the fixed `rd_*` readers above (callers length-guard those
// up front), these return `None` past the end so a truncated tail degrades to
// "identity + name only" instead of panicking.
#[inline]
fn opt_u8(b: &[u8], o: usize) -> Option<u8> {
    b.get(o).copied()
}
#[inline]
fn opt_u16_le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().unwrap()))
}
#[inline]
fn opt_u16_be(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(o..o + 2)?.try_into().unwrap()))
}
#[inline]
fn opt_u32_le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().unwrap()))
}
#[inline]
fn opt_f32_le(b: &[u8], o: usize) -> Option<f32> {
    Some(f32::from_le_bytes(b.get(o..o + 4)?.try_into().unwrap()))
}

/// NUL-terminated latin-1 out of a fixed-width name buffer (eql profile names
/// use latin-1, matching the daemon's `QString::fromLatin1`; distinct from the
/// crate-root utf8-lossy `cstr_field` the vendored modules use).
fn cstr_latin1(buf: &[u8]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    latin1(&buf[..end])
}

/// The name/lastname block is a reliable ABSOLUTE anchor inside the
/// (variable-length, ~40KB) profile: `u32 == 64` + 64-byte NUL-terminated name
/// buffer + `u32 == 32` + 32-byte NUL-terminated lastname buffer. Scanning for
/// that signature — instead of a fixed offset like the old 36047 — survives the
/// inventory/spellbook size drift that shifts the block per character and per
/// patch, and everything from the block onward matches the Live tail layout.
/// Returns the offset of the leading `u32 == 64`, or `None` if not found.
///
/// VALIDATE the candidate: a real first name is a capitalized, printable,
/// NUL-terminated run — binary that happens to carry the two length words
/// won't also spell a name, so this won't false-match.
fn find_profile_name_block(b: &[u8]) -> Option<usize> {
    if b.len() < 104 {
        return None;
    }
    for p in 0..=(b.len() - 104) {
        if b[p] != 0x40 || b[p + 1] != 0 || b[p + 2] != 0 || b[p + 3] != 0 {
            continue;
        }
        if b[p + 68] != 0x20 || b[p + 69] != 0 || b[p + 70] != 0 || b[p + 71] != 0 {
            continue;
        }
        let nb = &b[p + 4..p + 68];
        if !nb[0].is_ascii_uppercase() {
            continue;
        }
        let mut ok = false;
        for &c in nb {
            match c {
                0 => {
                    ok = true; // reached the terminator after >=1 printable byte
                    break;
                }
                0x20..=0x7e => {}
                _ => break,
            }
        }
        if ok {
            return Some(p);
        }
    }
    None
}

/// Parse name + lastname + the positionally-mapped tail (birthday, expansions,
/// languages, current zone/instance, position, guild, carried+bank money)
/// starting at the name block found by `find_profile_name_block`. Byte orders
/// match the Live tail (`fillProfileStruct`): zoneId/instance LE, standState/
/// anon BE, everything else LE. Degrades gracefully on a truncated tail.
fn read_profile_name_and_tail(b: &[u8], p0: usize, prof: &mut PlayerProfile) -> Option<()> {
    let mut p = p0;

    let name_len = opt_u32_le(b, p)? as usize; // == 64
    p += 4;
    prof.name = cstr_latin1(b.get(p..p + 64)?);
    p += name_len;

    let last_len = opt_u32_le(b, p)? as usize; // == 32
    p += 4;
    prof.last_name = cstr_latin1(b.get(p..p + 32)?);
    p += last_len;

    prof.birthday_time = opt_u32_le(b, p)?;
    p += 4;
    prof.account_create_date = opt_u32_le(b, p)?;
    p += 4;
    prof.last_save_time = opt_u32_le(b, p)?;
    p += 4;
    prof.time_played_min = opt_u32_le(b, p)?;
    p += 4;
    p += 4; // unknown
    prof.expansions = opt_u32_le(b, p)?;
    p += 4;
    p += 4; // unknown

    let lang_count = opt_u32_le(b, p)? as usize;
    p += 4;
    for _ in 0..lang_count {
        prof.languages.push(opt_u8(b, p)?);
        p += 1;
    }

    prof.zone_id = opt_u16_le(b, p)?;
    p += 2;
    prof.zone_instance = opt_u16_le(b, p)?;
    p += 2;

    // Position wire order is y, x, z, heading (f32 LE), same as Live.
    prof.y = opt_f32_le(b, p)?;
    p += 4;
    prof.x = opt_f32_le(b, p)?;
    p += 4;
    prof.z = opt_f32_le(b, p)?;
    p += 4;
    prof.heading = opt_f32_le(b, p)?;
    p += 4;

    // standState / anon are read BE (`readUInt16`), like Live.
    prof.stand_state = opt_u16_be(b, p)?;
    p += 2;
    prof.anon = opt_u16_be(b, p)?;
    p += 2;

    prof.guild_id = opt_u32_le(b, p)?;
    p += 4;
    prof.guild_server_id = opt_u32_le(b, p)?;

    // Money is NOT in this walk. The relative tail landed in the wrong place —
    // and its assumed layout was wrong too: carried is followed by CURSOR, not
    // bank. Read from fixed offsets in `parse_player_profile` instead.
    Some(())
}

/// `OP_PlayerProfile` (Legends) S>C. Two parts:
///
/// 1. Identity header, fixed offsets (patch-VERIFIED against a known char —
///    race DarkElf=6 @21, level 12 @33): gender u8 @20, race u32 @21, class u32
///    @25, level u8 @33. Truncation to the daemon's u16 race / u8 class happens
///    on the C++ side (`setIdentity`), same as the Live path.
///
/// 2. Name/lastname + tail, via `find_profile_name_block`'s absolute anchor-scan
///    (replaces the old fragile fixed offset 36047). This yields the surname
///    and the current zone/instance, position, guild, and carried+bank money —
///    the eql box is named and placed from its own profile, authoritatively,
///    like Live, rather than from the own-spawn adoption fallback.
///
/// The EQ Legends multiclass bitmask sits at @29 (u32, inserted between class
/// and level — that's why `level` is @33 not @29). `class` @25 is the primary
/// of three simultaneous classes; surfacing all three needs a proto field and
/// is deferred (the neutral `setIdentity` carries a single class).
pub fn parse_player_profile(b: &[u8]) -> Result<PlayerProfile, DecodeError> {
    if b.len() < 34 {
        return Err(DecodeError::Short(b.len()));
    }
    let mut prof = PlayerProfile {
        gender: b[20],
        race: rd_u32(b, 21),
        class_: rd_u32(b, 25),
        class_mask: rd_u32(b, 29), // EQL multiclass bitmask (bit N = class N)
        level: b[33],
        ..Default::default()
    };
    // FIXED offset: 7 u32 in Live's charProfileStruct order, its 956 block
    // landing 6 bytes later. BASE stats (the loadout roll), not gear totals.
    if b.len() >= 990 {
        prof.str_ = rd_u32(b, 962);
        prof.sta = rd_u32(b, 966);
        prof.cha = rd_u32(b, 970);
        prof.dex = rd_u32(b, 974);
        prof.int_ = rd_u32(b, 978);
        prof.agi = rd_u32(b, 982);
        prof.wis = rd_u32(b, 986);
    }
    // Variable-length NetStream walk to the SKILL array (and the AA array en
    // route), mirroring ZoneMgr::fillProfileStructEQL (zonemgr.cpp). Populates
    // `skills`, `aa_ids`/`aa_values`, and `aa_spent`. Bounds-guarded: any
    // short-read leaves whatever it read so far — identity above + name below
    // are unaffected, and skills fall back to incremental OP_SkillUpdate.
    walk_profile_skills(b, &mut prof);

    // Name + tail via absolute anchor-scan. If the block isn't found (heavy
    // drift / truncation) the identity fields above still stand and the C++
    // side falls back to own-spawn name adoption.
    if let Some(p) = find_profile_name_block(b) {
        read_profile_name_and_tail(b, p, &mut prof);
    }
    // EQL active stance / invocation live at a FIXED offset (33777 / 33781): the
    // combat-state block sits in the profile's fixed prefix, verified byte-
    // identical across two chars of different class/level/server (the variable
    // netstream is all AFTER it). Bounds-guarded; the C++ side range-validates
    // the id via stanceName()/invocationName().
    if b.len() >= 33785 {
        prof.stance = rd_u32(b, 33777);
        prof.invocation = rd_u32(b, 33781);
    }
    // Money at FIXED offsets, same fixed-prefix region as stance above. Two
    // blocks exist; verified against a known purse across 10 captured profiles:
    //   33687 carried P/G/S/C   33703 cursor P/G/S/C
    //   36245 inventory mirror  36261 bank P/G/S/C
    // Carried is authoritative and sits BELOW stance (33777), so it is inside
    // the prefix already verified byte-identical across chars. Denominations are
    // NOT normalized on the wire (101 silver / 281 copper observed) — always sum
    // to copper rather than assuming each is < 10.
    if b.len() >= 33719 {
        prof.platinum = rd_u32(b, 33687);
        prof.gold = rd_u32(b, 33691);
        prof.silver = rd_u32(b, 33695);
        prof.copper = rd_u32(b, 33699);
        prof.platinum_cursor = rd_u32(b, 33703);
        prof.gold_cursor = rd_u32(b, 33707);
        prof.silver_cursor = rd_u32(b, 33711);
        prof.copper_cursor = rd_u32(b, 33715);
    }
    // Bank follows the inventory mirror of the carried quadruple, which sits
    // PAST the stance-verified prefix — locate it by signature instead of a raw
    // constant so it survives drift in the variable region. An all-zero purse
    // would match padding anywhere, so bank is left 0 in that case.
    if b.len() >= 36277 {
        let carried = [prof.platinum, prof.gold, prof.silver, prof.copper];
        if carried != [0; 4] {
            let mut sig = [0u8; 16];
            for (i, v) in carried.iter().enumerate() {
                sig[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            // Search starts past the carried block so we find the mirror, not it.
            if let Some(off) = b[33703..]
                .windows(16)
                .position(|w| w == sig)
                .map(|o| o + 33703)
                .filter(|o| o + 32 <= b.len())
            {
                prof.platinum_inventory = rd_u32(b, off);
                prof.gold_inventory = rd_u32(b, off + 4);
                prof.silver_inventory = rd_u32(b, off + 8);
                prof.copper_inventory = rd_u32(b, off + 12);
                prof.platinum_bank = rd_u32(b, off + 16);
                prof.gold_bank = rd_u32(b, off + 20);
                prof.silver_bank = rd_u32(b, off + 24);
                prof.copper_bank = rd_u32(b, off + 28);
            }
        }
    }
    Ok(prof)
}

/// Walk the eql OP_PlayerProfile from the identity header (cursor @35, one byte
/// past `level1`) through every count-prefixed section to the SKILL array,
/// filling `skills`, `aa_ids`/`aa_values`, and `aa_spent` (= Σ aa value). Direct
/// port of ZoneMgr::fillProfileStructEQL: little-endian scalars, sequential
/// cursor. Every section count varies per character, so there are no fixed
/// offsets past `level1` — the cursor must be walked. Bounds-guarded: any
/// underflow returns early, leaving the fields populated so far untouched.
fn walk_profile_skills(b: &[u8], prof: &mut PlayerProfile) {
    // MAX_KNOWN_SKILLS in the daemon headers (src/backend/*/everquest.h).
    const MAX_KNOWN_SKILLS: usize = 100;
    let len = b.len();
    // checksum@0, skip 16, gender@20, race@21, class@25, classMask@29,
    // level@33, level1@34 => walk resumes at 35.
    let mut p: usize = 35;

    // Read a LE u32 and advance; bail out of the whole walk on underflow.
    macro_rules! next_u32 {
        () => {{
            if p + 4 > len {
                return;
            }
            let v = rd_u32(b, p);
            p += 4;
            v
        }};
    }
    // Skip `n` bytes; bail on underflow.
    macro_rules! skip {
        ($n:expr) => {{
            let n = $n;
            if p.checked_add(n).map_or(true, |e| e > len) {
                return;
            }
            p += n;
        }};
    }

    // bind points: u32 count, then count * 20 bytes.
    let bind_count = next_u32!() as usize;
    skip!(bind_count.saturating_mul(20));

    // deity, intoxication (2 u32).
    let _deity = next_u32!();
    let _intoxication = next_u32!();

    // spell-slot refresh: u32 count, then count * u32.
    let refresh_count = next_u32!() as usize;
    skip!(refresh_count.saturating_mul(4));

    // equipment: u32 count, then count * 20 bytes.
    let equip_count = next_u32!() as usize;
    skip!(equip_count.saturating_mul(20));

    // three unknown count-prefixed arrays: 20B, 4B, 4B entries.
    let s0 = next_u32!() as usize;
    skip!(s0.saturating_mul(20));
    let s1 = next_u32!() as usize;
    skip!(s1.saturating_mul(4));
    let s2 = next_u32!() as usize;
    skip!(s2.saturating_mul(4));

    // face / hair / beard / eyes / etc.
    skip!(51);

    // points, MANA, curHp, STR, STA, CHA, DEX, INT, AGI, WIS (10 u32).
    skip!(40);

    // unknown padding.
    skip!(28);

    // AA array: u32 count, then count * {u32 id, u32 value, u32 unk}. Guard the
    // whole block before the loop so a corrupt count can't over-allocate.
    let aa_count = next_u32!() as usize;
    let aa_bytes = aa_count.saturating_mul(12);
    if p.checked_add(aa_bytes).map_or(true, |e| e > len) {
        return;
    }
    let mut aa_spent: u32 = 0;
    prof.aa_ids.reserve(aa_count);
    prof.aa_values.reserve(aa_count);
    for _ in 0..aa_count {
        let id = rd_u32(b, p);
        let val = rd_u32(b, p + 4);
        // unknown at p + 8.
        p += 12;
        aa_spent = aa_spent.wrapping_add(val);
        prof.aa_ids.push(id);
        prof.aa_values.push(val);
    }
    prof.aa_spent = aa_spent;

    // SKILLS: u32 count, then count * u32 (store up to MAX_KNOWN_SKILLS).
    let skill_count = next_u32!() as usize;
    if p.checked_add(skill_count.saturating_mul(4))
        .map_or(true, |e| e > len)
    {
        return;
    }
    let stored = skill_count.min(MAX_KNOWN_SKILLS);
    prof.skills.reserve(stored);
    for i in 0..skill_count {
        let v = rd_u32(b, p);
        p += 4;
        if i < MAX_KNOWN_SKILLS {
            prof.skills.push(v);
        }
    }
}

// `OP_ClientUpdate` (Legends) C>S: IEEE-float position + heading.
// post-2026-07-29 layout (42B; see `player_self_pos.rs` for the derivation
// and its evidence): spawnId u16 @2, gameY@10, gameX@22, gameZ@34, heading the
// low 11 bits at @26. The velocities are not located this patch and read 0.
//
// Worth knowing when a future patch rotates this again: the 07/29 body is close
// to the pre-07/14 42B form, which also carried spawnId@2 and gameY@10 (it
// had gameX@18 / z@30, i.e. those two sit 4 bytes later now). The 38B bodies
// that ran between 07/14 and 07/29 were the outlier — they dropped the spawnId
// entirely. So on the next rotation, check the older 42B layout before assuming
// a from-scratch rearrangement.
// `parse_player_self_pos` lives in `player_self_pos.rs` (re-exported above) — one
// parser that validates against its own `PAYLOAD_LEN`, the SAME const the
// `playerSelfPosStruct` size override reads, so the SZC gate and the parser can
// never disagree on the size (they diverged once — 42 vs 38 — and silently masked
// the self-position, see OPCODES_LEGENDS.md 2026-07-14).

/// `OP_NewZone` (Legends) S>C, ~340B, once per zone-in. Carries the
/// CURRENT zone as packed NUL-terminated text — `short_name` then `long_name`
/// (then a zonefile repeat + environment tail). The daemon drives
/// `ZoneMgr::setZoneByName(short, long)` directly, so no classic-id table is
/// needed. Confirmed 3-way (2026-07-08): guktop / "The City of Guk",
/// nektulos / "Nektulos Forest", unrest / "The Estate of Unrest" — each the
/// correct current zone, each a different length (packed C-strings, not
/// fixed-width arrays, so the fields sit at zone-dependent offsets).
///
/// The pre-2026-07-08 mapping pointed OP_NewZone at 0x4bc8, whose `u32@6` is the
/// BIND zone (identical across zones); that opcode is not OP_NewZone and is no
/// longer decoded here.
pub fn parse_new_zone(b: &[u8]) -> Result<NewZone, DecodeError> {
    // Keep this public compatibility entry point, but make the module parser
    // the sole implementation used by both the Event backend and direct FFI.
    new_zone::parse_new_zone(b).map_err(|_| DecodeError::Short(b.len()))
}

// NOTE: eql has NO local OP_MobUpdate parser — the Legends wire turned out to
// be byte-identical to Live's `spawnPositionUpdate` (14B, packed
// y:19/z:19/u3:7/x:19/heading:12, fixed-point ×8), so `seq-bridge` routes
// eql's `decode_mob_update` to the shared `seq_decode::parse_mob_update`.
// The earlier per-offset i16 parser here (X@10, Y@4/8, Z@6/64) was reading
// truncated windows of those bitfields — correct inside ±4095 game units but
// wrapping by 8192 (Y) / 1024 (Z) beyond, the "north<->south teleport" bug.

/// `OP_ZoneSpawns` (Legends) S>C: null-terminated name, then a variable-length
/// block. Header fields stay at the front: spawnId u32 @0, level u8 @4,
/// curHpPct u8 @44, maxHpPct u8 @45. **post-2026-07-07 layout**: position sits
/// at a FIXED offset from the END of the block (block grew 326→330 NPC / 486
/// rich, but the position triple stays anchored to the tail). **post-2026-07-29
/// layout**: three consecutive u32 words in packet order Z @(len-103),
/// X @(len-99), Y @(len-95), each a **signed 19-bit fixed-point (×8) coordinate
/// in the word's low bits** (same packing as Live's `spawnStruct` position
/// words; the upper 13 bits carry other subfields). The 19-bit width (not i16)
/// was confirmed by sign-fill analysis 2026-07-08 — an i16 read wraps
/// coordinates past ±4095 by 8192 game units. The walk below reaches these
/// sequentially rather than from the tail, because the record's tail length
/// varies with the title/suffix string block.
/// Sequential reader mirroring the daemon's `NetStream` (LE `readUInt*NC`,
/// NUL-terminated `readText`), bounds-checked: any overrun ends the walk with
/// `BadLength` rather than panicking (a dropped spawn, not a crash).
struct Walk<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Walk<'a> {
    fn new(b: &'a [u8]) -> Self {
        Walk { b, p: 0 }
    }
    fn pos(&self) -> usize {
        self.p
    }
    fn need(&self, n: usize) -> Result<(), DecodeError> {
        if self.p + n > self.b.len() {
            Err(DecodeError::BadLength(self.b.len()))
        } else {
            Ok(())
        }
    }
    fn skip(&mut self, n: usize) -> Result<(), DecodeError> {
        self.need(n)?;
        self.p += n;
        Ok(())
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        self.need(1)?;
        let v = self.b[self.p];
        self.p += 1;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        self.need(4)?;
        let v = rd_u32(self.b, self.p);
        self.p += 4;
        Ok(v)
    }
    /// NUL-terminated latin-1 string; advances past the NUL.
    fn text(&mut self) -> Result<String, DecodeError> {
        let start = self.p;
        while self.p < self.b.len() && self.b[self.p] != 0 {
            self.p += 1;
        }
        if self.p >= self.b.len() {
            return Err(DecodeError::BadLength(self.b.len()));
        }
        let s = latin1(&self.b[start..self.p]);
        self.p += 1; // consume NUL
        Ok(s)
    }
}

/// Decode a signed 19-bit ×8 fixed-point coordinate out of a full position word
/// (low 19 bits; upper 13 carry unrelated subfields). Same packing as
/// `rd_pos19`, but taking the whole word.
#[inline]
fn pos19_word(w: u32) -> i16 {
    let v = w & 0x7FFFF;
    let raw = if v & 0x4_0000 != 0 {
        (v as i32) - (1 << 19)
    } else {
        v as i32
    };
    (raw >> 3) as i16
}

/// `OP_ZoneSpawns` (Legends) S>C, one per spawn. Full front walk,
/// ported 1:1 from the community patch's `SpawnShell::fillSpawnStruct` (verified
/// against a 1617-record eql corpus). Supersedes the old tail-anchored partial,
/// which assumed a fixed 95-byte tail and so mis-read position and dropped
/// titles on any spawn carrying a title/suffix string block.
pub fn parse_spawn(b: &[u8]) -> Result<ZoneSpawn, DecodeError> {
    let mut w = Walk::new(b);

    let name = w.text()?;
    if name.is_empty() {
        return Err(DecodeError::Short(b.len()));
    }
    let id = w.u32()? as u16;
    let level = w.u8()?;
    w.skip(16)?;
    let npc = w.u8()?;
    let _misc_data = w.u32()?;
    let _other_data = w.u8()?;
    w.skip(8)?; // unknown3, unknown4
                // (EQ Legends aura-flagged spawns carry no aura block on the wire.)

    // bodytype: `charProperties` count-prefixed u32s; the first is the bodytype.
    let char_properties = w.u8()?;
    let mut body_type = 0u32;
    if char_properties != 0 {
        for i in 0..char_properties {
            let n = w.u32()?;
            if i == 0 {
                body_type = n;
            }
        }
    }

    // EQ Legends: the HP percent sits 8 bytes past Live's slot (Live's slot
    // reads 0 for every eql spawn).
    w.skip(3)?;
    let appearance_count = w.u8()? as usize;
    w.skip(4)?;
    let cur_hp = w.u8()?;
    w.skip(33 + 4 * appearance_count)?;

    let race = w.u32()?;
    let holding = w.u8()?;
    let deity = w.u32()?;
    let guild_id = w.u32()?;
    let guild_server_id = w.u32()?;
    let class_ = w.u32()?;
    let class_mask = w.u32()?; // EQL multiclass bitmask (bit N = class N)
    w.skip(1)?;
    let state = w.u8()?;
    let light = w.u8()?;
    w.skip(1)?;

    let last_name = w.text()?;
    w.skip(2)?;
    let pet_owner_id = w.u32()?;

    // 12 extra bytes on NPCs (added 2013-06-19).
    w.skip(if npc == 1 { 49 } else { 37 })?;

    // Equipment block (skipped — not surfaced). The client's own read gate:
    // full 9-slot layout for PCs + a few humanoid NPC races, 2-slot otherwise.
    if npc == 0 || race <= 12 || race == 128 || race == 130 || race == 330 || race == 522 {
        w.skip(36 + 9 * 5 * 4)?;
    } else {
        w.skip(20 + 2 * 5 * 4)?;
    }

    // EQL spawn position block — six 32-bit words follow the equipment. The
    // 2026-07-14 patch moved Z into the HIGH bits of the 2nd word (NOT the low-19
    // of the 3rd word the pre-patch layout used, which read garbage Z ~±30000 and
    // — via the map's z-depth filter — hid nearly every mob, "barely any coming in"):
    //   word0            velocity / flags
    //   word1 (z-word)   Z = (w >> 13) & 0x7FFFF, signed 19-bit ×8  — the SAME
    //                    high-bit packing as OP_ClientUpdate's Z
    //   word2            flag (~0xa8000000)
    //   word3 (y-word)   Y in the low 19 bits (+ h2048 heading in bits 19..31)
    //   word4 (x-word)   X in the low 19 bits
    //   word5            delta / animation
    // Re-derived 2026-07-14 by matching ZoneEntry Z to the OP_MobUpdate Z of the
    // same spawns (35/36 exact, worst 1 unit).
    // ---- 2026-07-28 patch: the position block was rearranged. ----
    // The words are still six and still tail-anchored, but the coordinates
    // moved and one of them is now a PAD that always reads 0 — which is why
    // the pre-patch read produced a wall of spawns sitting at y=0.
    //
    // Located against the only position we can trust: the player's own. Their
    // breadcrumb (OP_SelfPos) gives verified coordinates, and their own spawn
    // arrives as a ZoneEntry record, so the record's words can be matched
    // straight to known values — 30 own-records agreeing on
    //     Y @(len-95)   Z @(len-91)   pad @(len-87)   X @(len-83)
    //
    // The x/y assignment is MobUpdate's, established by tail-scanning 3216 NPC
    // records against their MobUpdate position (id-verified at 100%, fixed
    // layout). An earlier revision had these swapped because it was pinned to
    // the breadcrumb, which reports in /loc order (y first) — self-consistent
    // with the breadcrumb and transposed against every other position source.
    // i.e. in packet order: one lead word, then X, Z, pad, Y, one trailing
    // word. Read sequentially rather than from the tail — the record's tail
    // length varies with the title/suffix string block.
    // Upstream's struct (legends 7612d72) independently describes the same
    // 5-word shape including the pad; it labels the two horizontal words the
    // other way round, which is the transpose it flags as ambiguous. Ours is
    // pinned to the same frame the breadcrumb and heading were verified in, so
    // it stays consistent with the self-position path.
    //
    // ---- 2026-08-06: re-derived against upstream's struct + measurement. ----
    // The previous read took the coordinates as the first THREE consecutive
    // words (Z, X, Y, all low-19). That is not the block's shape, and the
    // symptom was unmissable once plotted: word1 is a PAD that reads 0 in
    // 479 of 952 id-paired records, so half of every zone's spawns rendered
    // in a straight line at x = 0.
    //
    // Upstream's `posData[5]` union (legends everquest.h) describes the real
    // shape, and its consumer — `spawn.cpp: setPos(s->y >> 3, s->x >> 3,
    // s->z >> 3)` — supplies the EQL transpose: their `y` field is world X and
    // their `x` field is world Y. Reading their STRUCT without their CALL SITE
    // transposes the map; reading the call site without the struct misses that
    // Z lives in the HIGH bits. Both halves or neither.
    //
    // Word roles pinned by scoring every (word, half) against the untouched
    // OP_MobUpdate stream over 952 id-paired records — median absolute error,
    // best vs runner-up:
    //     X = w0 low-19   161 vs 554     (upstream's `y` field)
    //     Y = w3 low-19   182 vs 1441    (upstream's `x` field)
    //     Z = w4 high-19    9 vs 43
    // X/Y carry more error than Z because ZoneEntry reports a spawn-time
    // position while MobUpdate reports the current one — mobs walk, terrain
    // does not. Z's 9-unit median is what confirms the alignment.
    //
    // w1 is the pad (479/952 zero), w5 is always 0 (952/952). Upstream labels
    // w4 `heading:12 | padding00:20`, but 12 + 1 + 19 = 32 and Z measurably
    // lives in that "padding" — so their Z word index is off by two while
    // their X/Y ones are right. Heading is still NOT recoverable: no word at
    // 11- or 12-bit width scores better than noise against MobUpdate's facing,
    // so it stays zeroed rather than pointed in a direction from the wrong bits.
    //
    // ---- 2026-08-18: the block was rearranged again. ----
    // Ported from upstream legends `1cd04be` (`spawnStruct` posData) plus their
    // call site, which still transposes — `spawn.cpp: setPos(s->y >> 3,
    // s->x >> 3, s->z >> 3)`, so their `y` field is world X and their `x` field
    // is world Y. Both halves or neither, same as the 08/06 port.
    //
    //     w0  pad
    //     w1  { z:19 low | deltaZ:13 }              -> world Z
    //     w2  { heading:12 low | padding:20 }
    //     w3  { y:19 low | deltaY:13 }  (upstream's `y`)  -> world X
    //     w4  { x:19 low | deltaX:13 }  (upstream's `x`)  -> world Y
    //     w5  trailing
    //
    // Every coordinate is now in the LOW 19 bits of its word; the high-19 z of
    // the previous layout is gone, and the pad moved from w1 to w0.
    //
    // UNVALIDATED LOCALLY — no post-patch capture exists yet, so the 952-record
    // scoring run against OP_MobUpdate that pinned the 08/06 layout has not been
    // re-run. IF Z COMES OUT WRONG, test w2's HIGH 19 bits (`w2 >> 13`) first:
    // that is precisely the disagreement we already found once, upstream naming
    // a separate z field while Z actually rode the top of the heading word.
    let _pad = w.u32()?;
    let w1 = w.u32()?;
    let _w2 = w.u32()?;
    let w3 = w.u32()?;
    let w4 = w.u32()?;
    let _trail = w.u32()?;
    let x = pos19_word(w3);
    let y = pos19_word(w4);
    let z = pos19_word(w1);
    // Upstream gives the facing its own word again (w2, 12 bits low) after a
    // patch with no home for it. Left at 0 until it is measured: the last time
    // no window scored better than noise against MobUpdate's facing, and a
    // wrong heading points every spawn in the zone the wrong way, where a zero
    // one just points them all north.
    let heading = 0u16;

    // Title/suffix string block: 4 strings on ordinary spawns, 6 (title, suffix,
    // then the 4) on titled ones — no reliable presence flag. Anchor on the
    // tail: the record ends with 4 fixed bytes, u8 isMercenary, an ASCII digit
    // string ('0' run), then 53 fixed bytes. Read strings up to that anchor;
    // the first two non-empty are title then suffix.
    let mut title = String::new();
    let mut suffix = String::new();
    if b.len() >= 55 {
        let d_end = b.len() - 54; // digit-string NUL sits 54 bytes from the end
        let mut d_start = d_end;
        while d_start > 0 && b[d_start - 1] == b'0' {
            d_start -= 1;
        }
        if d_start >= 5 {
            let text_end = d_start - 1 /*isMercenary*/ - 4 /*fixed*/;
            let mut str_index = 0;
            while w.pos() < text_end {
                let s = w.text()?;
                match str_index {
                    0 => title = s,
                    1 => suffix = s,
                    _ => {}
                }
                str_index += 1;
                if str_index > 6 {
                    break; // safety: never more than 6 strings
                }
            }
        }
    }

    Ok(ZoneSpawn {
        id,
        name,
        last_name,
        title,
        suffix,
        x,
        y,
        z,
        heading,
        level,
        cur_hp,
        max_hp: 100, // curHp is a percentage; base is 100
        race,
        class_,
        class_mask,
        deity,
        guild_id,
        guild_server_id,
        pet_owner_id,
        npc,
        body_type,
        holding,
        state,
        light,
    })
}

/// `OP_Consider` (Legends) 24B: `{u32 self, u32 target, u32 faction, u32 =7,
/// pad, pad}`. C>S request has faction=0; the S>C reply fills faction (observed
/// 2=warmly, 4=amiably — the friendliness word; **level is NOT here**, the
/// client reads it from the spawn). Maps to the shared `Consider` (level=0) so
/// the daemon's `SpawnShell::consMessage` path is uniform with Live.
pub fn parse_consider(b: &[u8]) -> Result<Consider, DecodeError> {
    if b.len() != 24 {
        return Err(DecodeError::BadLength(b.len()));
    }
    Ok(Consider {
        player_id: rd_u32(b, 0),
        target_id: rd_u32(b, 4),
        faction: rd_u32(b, 8) as i32,
        level: 0,
    })
}

/// Payload size overrides for the daemon's `SZC_Match` size registry: toml
/// `typename`s whose eql wire size diverges from the daemon's compiled (Live)
/// `everquest.h` `sizeof`. The daemon applies these over its C++ size table so a
/// diverged payload keeps its real struct NAME and size-gates on eql's real
/// size — not a hardcoded Live `sizeof`, and not a `uint8_t`/`none` placeholder.
/// Sourced from the pinned `eqstructs` sizes so a size and its decoder move
/// together. (live/test diverge from nothing; the bridge ships them an empty
/// list.)
/// eql `OP_BeginCast` (S>C, 19B): a spawn started casting a spell.
/// Wire layout validated across 3127 fight-capture packets: spellId u32@0,
/// casterSpawnId u16@4, castTime_ms u16@6 (cast times 0/2010/4500/5000ms;
/// spell ids include 74023 > u16, so spellId MUST be read as u32). The stock
/// 15B beginCastStruct covers only the first 8B; the trailing 11B are flags.
pub struct BeginCast {
    pub caster_id: u32,
    pub spell_id: u32,
    pub cast_time_ms: u32,
}

pub fn parse_begin_cast(b: &[u8]) -> Result<BeginCast, DecodeError> {
    if b.len() < 8 {
        return Err(DecodeError::Short(b.len()));
    }
    Ok(BeginCast {
        spell_id: rd_u32(b, 0),
        caster_id: rd_u16(b, 4) as u32,
        cast_time_ms: rd_u16(b, 6) as u32,
    })
}

/// eql `OP_SendAATable` (S>C): one AA ability-rank definition, burst at
/// zone-in (one record per packet; variable length 130..346B = a 37B fixed
/// head + a variable prereq/effect tail). Only the fixed head is needed.
/// Layout matches Live's `aaInfoStruct`: `descID`@0 (== the profile's
/// `aa_array[].AA`, the per-rank id) and `titleSID`@13 (a dbstr type-1 id
/// shared by every rank of an AA — the daemon resolves it to the display
/// name). Verified against 15/15 owned AAs in the fight capture (titleSID@13 →
/// dbstr type-1 gives the correct names: Packrat, Fury of Magic, Destructive
/// Fury, Mnemonic Retention, …).
pub struct AaTableEntry {
    pub desc_id: u32,
    pub title_sid: u32,
}

pub fn parse_aa_table_entry(b: &[u8]) -> Result<AaTableEntry, DecodeError> {
    // Guard the whole fixed head (37B through the record's rank field); titleSID
    // sits at 13, well before the variable tail, so a fixed read reaches it.
    if b.len() < 37 {
        return Err(DecodeError::Short(b.len()));
    }
    Ok(AaTableEntry {
        desc_id: rd_u32(b, 0),
        title_sid: rd_u32(b, 13),
    })
}

/// eql OP_Stance / OP_Invocation, S>C echo (authoritative): a
/// swappable stance or invocation was activated. 4B payload = a single u32
/// ability id @0 (a stable client enum; the daemon resolves it to a name). The
/// opcode id distinguishes stance from invocation; the payload shape is shared.
pub fn parse_activate_ability(b: &[u8]) -> Result<u32, DecodeError> {
    if b.len() < 4 {
        Err(DecodeError::Short(b.len()))
    } else {
        Ok(rd_u32(b, 0))
    }
}

pub fn size_overrides() -> Vec<(&'static str, u32)> {
    vec![
        // eql /consider is 24B both ways; Live's considerStruct is 32B.
        (
            "considerStruct",
            core::mem::size_of::<eqstructs::considerStruct>() as u32,
        ),
        // eql OP_CastSpell is a fixed 44B (validated locally 2026-08-03; the gate
        // was pinned at 40 from the Live struct, so every packet was size-dropped).
        // slot@0/spellId@4/targetId@18 kept their offsets — the record grew at the
        // tail. Size comes from that parser, not from Live's 39B sizeof.
        ("startCastStruct", start_cast::PAYLOAD_LEN as u32),
        // eql OP_ClientUpdate S>C other-spawn position broadcast: 19-bit ×8 packed,
        // x/y in the LOW bits of the @4/@16 words and z in the HIGH bits of @12.
        // The 2026-08-04 rotation shrank it 28B -> 24B and rearranged the body;
        // decoded by this crate's own parse_player_spawn_pos, re-derived against
        // the untouched OP_MobUpdate / OP_NpcMoveUpdate streams — see that module
        // and OPCODES_LEGENDS.md.
        ("playerSpawnPosStruct", player_spawn_pos::PAYLOAD_LEN as u32),
        // --- De-piggyback (2026-07-10): eql OWNS every mapped SZC_Match gate size ---
        // The daemon's compiled size table is Live's `everquest.h` sizeof. Before
        // this block, any mapped eql SZC_Match opcode NOT listed above silently
        // inherited that Live sizeof as its packet gate — fragile (a Live struct
        // change moves the eql gate) and the source of the WearChange/SpawnUpdateStruct
        // 32B collision (a wrong-handler binding that size-matched by coincidence).
        // So EVERY mapped SZC_Match eql opcode declares its gate size HERE, making the
        // size eql-owned and severing the Live dependency. `packetinfo.cpp` warns at
        // load if a mapped SZC_Match eql opcode is missing from this list, so the next
        // opcode-map can't reintroduce the footgun silently. Sizes equal Live's TODAY
        // (eql goldens verify byte-for-byte); each is sourced from THIS crate's own
        // pinned `eqstructs` where a binding exists, so if eql's copy later diverges the
        // gate tracks it — never Live's. The handful with no pinned binding (eql reuses
        // the shared decode) carry the capture-confirmed eql size as a literal.
        (
            "spawnPositionUpdate",
            core::mem::size_of::<eqstructs::spawnPositionUpdate>() as u32,
        ),
        (
            "clientTargetStruct",
            core::mem::size_of::<eqstructs::clientTargetStruct>() as u32,
        ),
        (
            "manaDecrementStruct",
            core::mem::size_of::<eqstructs::manaDecrementStruct>() as u32,
        ),
        (
            "expUpdateStruct",
            core::mem::size_of::<eqstructs::expUpdateStruct>() as u32,
        ),
        (
            "skillIncStruct",
            core::mem::size_of::<eqstructs::skillIncStruct>() as u32,
        ),
        // OP_Stamina (07/14): stock 8B staminaStruct {u32 food, u32 water};
        // capture-verified food/water tick down together. Pinned so the gate is eql-owned.
        (
            "staminaStruct",
            core::mem::size_of::<eqstructs::staminaStruct>() as u32,
        ),
        // OP_Illusion (07/14): 332B spawnIllusionStruct — the /*0336*/
        // offset marker in everquest.h is stale; eql's pinned copy is 332 (fires
        // 332x33 in the fight capture). parse_illusion.
        (
            "spawnIllusionStruct",
            core::mem::size_of::<eqstructs::spawnIllusionStruct>() as u32,
        ),
        // OP_TimeOfDay: 8B timeOfDayStruct {u8 hour/min/day/month,u16 year};
        // no pinned eql binding, literal capture-confirmed size (fires 8x12).
        ("timeOfDayStruct", 8),
        // OP_InspectAnswer: 1956B inspectDataStruct; not seen in the fight
        // capture (inspect is passive) — size from the struct def.
        ("inspectDataStruct", 1956),
        (
            "deleteSpawnStruct",
            core::mem::size_of::<eqstructs::deleteSpawnStruct>() as u32,
        ),
        (
            "newCorpseStruct",
            core::mem::size_of::<eqstructs::newCorpseStruct>() as u32,
        ),
        (
            "remDropStruct",
            core::mem::size_of::<eqstructs::remDropStruct>() as u32,
        ),
        (
            "action2Struct",
            core::mem::size_of::<eqstructs::action2Struct>() as u32,
        ),
        (
            "actionStruct",
            core::mem::size_of::<eqstructs::actionStruct>() as u32,
        ),
        (
            "actionAltStruct",
            core::mem::size_of::<eqstructs::actionAltStruct>() as u32,
        ),
        // OP_SimpleMessage (07/12 rotation): stock 12B {u32 eqstrId, u32
        // color, u32 0}; pinned binding so the gate tracks eql's own copy.
        (
            "simpleMessageStruct",
            core::mem::size_of::<eqstructs::simpleMessageStruct>() as u32,
        ),
        // No pinned eql binding (eql reuses the shared decode) — capture-confirmed size:
        ("playerSelfPosStruct", player_self_pos::PAYLOAD_LEN as u32), // OP_ClientUpdate C>S self-pos: 42B post-07/29 (was 38B); floats Y@10/X@22/Z@34, spawnId@2, heading@26
        ("altExpUpdateStruct", 12), // OP_AAExpUpdate: u32 altexp, u32 aaUnspent, u32 tail
        // eql door rows are 132B (Live doorStruct is 136B); OP_SpawnDoor gates
        // SZC_Modulus on this and newDoorSpawns strides via door_stride().
        ("doorStruct", spawn_door::PAYLOAD_LEN as u32),
        ("timeOfDayStruct", 8),         // OP_TimeOfDay
        ("zoneServerInfoStruct", 130),  // OP_ZoneServerInfo (world)
        ("spawnAppearance2Struct", 24), // OP_SpawnAppearance2
        // OP_SpawnAppearance. eql's payload is the WIDE 24B record ({u32 spawnId,
        // u32 type, u32 value} + 12B of zeros), not Live's 8B struct — eql has one
        // appearance opcode where Live has two. This entry was missing, so a
        // mapped SZC_Match opcode silently inherited the compiled Live sizeof of 8
        // and the gate dropped every packet; --strict-gate-sizes flags exactly
        // this class. The size comes from eql's own parser, never from Live.
        (
            "spawnAppearanceStruct",
            spawn_appearance::PAYLOAD_LEN as u32,
        ),
        ("inspectDataStruct", 1956), // OP_InspectAnswer
        // Stock-struct reuse (eql size == Live's today, stock layout): declared
        // so the gate is eql-owned, not silently inherited from everquest.h.
        ("randomStruct", 76), // OP_RandomReply (76B, l-patch addendum 3)
        ("tradeSpellBookSlotsStruct", 8), // OP_SwapSpell
        ("buffWindowSlotStruct", 12), // OP_BuffWindow
        // 07/14 remap SZC_Match opcodes; no pinned eql binding, gate = eql WIRE size.
        // OP_BeginCast: eql wire is 19B (spellId u16@0, spawnId u16@4, castTime u16@6 —
        // Xerxes-confirmed), NOT the stock 15B beginCastStruct; gate at 19 so SZC_Match passes.
        ("beginCastStruct", 19),        // OP_BeginCast
        ("consentResponseStruct", 193), // OP_ConsentResponse + OP_DenyResponse
        ("GuildMemberUpdate", 88),      // OP_GuildMemberUpdate
        // OP_Stance + OP_Invocation: 4B {u32 abilityId}. Same
        // struct/size for both; the opcode id picks stance vs invocation.
        ("activateAbilityStruct", 4),
    ]
}

/// eql `OP_HPUpdate` — the multiplexed stat-sync channel, fully
/// decoded from the community f-patch's `SpawnShell::spawnStatEQL` (6378 wide
/// packets across the pcap library, zero layout exceptions).
///
/// Layout: `u32 spawnId | u8 flags | per-stat payload | [optional u32 tail]`.
/// `flags`: bit0 = wide form; bit1 = HP; bit2 = mana; bit3 = stamina/endurance;
/// bits4-5 = reason (event/refresh/tick, ignored). The per-stat payload appears
/// in bit order (HP, then mana, then endurance): the wide form carries
/// `{i64 cur, i64 max}` per set stat bit; the narrow form carries one `u8
/// percent` per stat (`max` = 100). Some packets append a trailing `u32`
/// (purpose unknown). `flags 0x31` with no stat bits is the 6s keepalive.
///
/// Consumers (see `EqlDispatch::statSync`): HP → spawn cur/max (the wide form
/// supersedes the old percent-only feed); the player's mana → `Player::setMana`
/// and endurance → `Player::setEndurance` (real cur/max, player-only, wide form
/// only — Legends has no standalone OP_EndUpdate, so this channel is the sole
/// endurance feed). Food/water never ride this channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatSync {
    pub spawn_id: u32,
    pub wide: bool,
    pub has_hp: bool,
    pub hp_cur: i64,
    pub hp_max: i64,
    pub has_mana: bool,
    pub mana_cur: i64,
    pub mana_max: i64,
    pub has_end: bool,
    pub end_cur: i64,
    pub end_max: i64,
}

pub fn parse_stat_sync(b: &[u8]) -> Result<StatSync, DecodeError> {
    if b.len() < 5 {
        return Err(DecodeError::Short(b.len()));
    }
    let flags = b[4];
    let wide = flags & 0x01 != 0;
    let nstats = ((flags & 0x02) != 0) as usize
        + ((flags & 0x04) != 0) as usize
        + ((flags & 0x08) != 0) as usize;
    let stat_sz = if wide { 16 } else { 1 };
    let expect = 5 + stat_sz * nstats;
    // Structural canary: exact size, optionally +4 (trailing u32). A mismatch
    // means the wire layout shifted — reject rather than misread the stats.
    if b.len() != expect && b.len() != expect + 4 {
        return Err(DecodeError::BadLength(b.len()));
    }

    let mut out = StatSync {
        spawn_id: rd_u32(b, 0),
        wide,
        ..StatSync::default()
    };

    let mut pos = 5;
    // bit1 = HP, bit2 = mana, bit3 = endurance, read in that fixed order.
    for bit in 1..=3u8 {
        if flags & (1 << bit) == 0 {
            continue;
        }
        let (cur, max) = if wide {
            let pair = (rd_i64(b, pos), rd_i64(b, pos + 8));
            pos += 16;
            pair
        } else {
            let c = b[pos] as i64;
            pos += 1;
            // A narrow percent above 100 signals a layout change; drop that stat
            // (matching the f-patch's warn+skip) but keep parsing the rest.
            if c > 100 {
                continue;
            }
            (c, 100)
        };
        match bit {
            1 => {
                out.has_hp = true;
                out.hp_cur = cur;
                out.hp_max = max;
            }
            2 => {
                out.has_mana = true;
                out.mana_cur = cur;
                out.mana_max = max;
            }
            _ => {
                out.has_end = true;
                out.end_cur = cur;
                out.end_max = max;
            }
        }
    }
    Ok(out)
}

/// One decoded record from `OP_BuffList`. `remaining_ticks <= 0` means
/// a permanent buff (no timer). `slot` is the buff-window slot index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffListEntry {
    pub spell_id: u32,
    pub remaining_ticks: i32,
    pub slot: u32,
    /// Who cast it, as spelled on the wire (empty when the name field is).
    /// Consumers key ownership off this: a non-self owner's list mixes the
    /// spawn's own buffs with the ones the player put on it, and only the
    /// caster tells them apart.
    pub caster: String,
}

/// A decoded `OP_BuffList`: the authoritative active-buff list for one
/// spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffList {
    pub spawn_id: u32,
    pub entries: Vec<BuffListEntry>,
}

/// eql `OP_BuffList` — a per-spawn active-buff list, sent at zone-in
/// and on every buff change, with real server-side remaining durations (the
/// patch calls it "the one that makes it work"). Layout (validated 26/26 across
/// the upperguk capture, cursor lands exactly on the packet end):
///
/// ```text
/// u32 spawnId | u32 (seq/timestamp) | u8 flag=1 | u8 count | u8[5] pad
/// then `count` records, each:
///   { u32 spellId, u32 =1, i32 remainingTicks, u32 =0 }   (16 fixed bytes)
///   + NUL-terminated caster name (latin1; empty for self-cast)
///   + slot: u32 BETWEEN records, u16 on the FINAL record.
/// ```
///
/// `remainingTicks <= 0` is permanent. The cursor-lands-on-end check is the
/// structural canary (a layout drift rejects the whole packet).
pub fn parse_buff_list(b: &[u8]) -> Result<BuffList, DecodeError> {
    if b.len() < 15 {
        return Err(DecodeError::Short(b.len()));
    }
    let spawn_id = rd_u32(b, 0);
    let count = b[9] as usize;
    let mut pos = 15;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        if pos + 16 > b.len() {
            return Err(DecodeError::BadLength(b.len()));
        }
        let spell_id = rd_u32(b, pos);
        let remaining_ticks = rd_u32(b, pos + 8) as i32;
        pos += 16;
        // NUL-terminated caster name.
        let caster = match b[pos..].iter().position(|&c| c == 0) {
            Some(off) => {
                let s = String::from_utf8_lossy(&b[pos..pos + off]).into_owned();
                pos += off + 1;
                s
            }
            None => return Err(DecodeError::BadLength(b.len())),
        };
        // slot: u32 between records, u16 on the final record.
        let slot = if i + 1 == count {
            if pos + 2 > b.len() {
                return Err(DecodeError::BadLength(b.len()));
            }
            let s = rd_u16(b, pos) as u32;
            pos += 2;
            s
        } else {
            if pos + 4 > b.len() {
                return Err(DecodeError::BadLength(b.len()));
            }
            let s = rd_u32(b, pos);
            pos += 4;
            s
        };
        entries.push(BuffListEntry {
            spell_id,
            remaining_ticks,
            slot,
            caster,
        });
    }
    // Structural canary: the parse must consume the packet exactly.
    if pos != b.len() {
        return Err(DecodeError::BadLength(b.len()));
    }
    Ok(BuffList { spawn_id, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_reads_identity() {
        let mut b = [0u8; 34];
        b[21..25].copy_from_slice(&6u32.to_le_bytes()); // race
        b[25..29].copy_from_slice(&5u32.to_le_bytes()); // class
        b[33] = 42; // level
        let p = parse_player_profile(&b).unwrap();
        assert_eq!(p.race, 6);
        assert_eq!(p.class_, 5);
        assert_eq!(p.level, 42);
        // 34-byte identity-only buffer has no name slot → empty, no panic.
        assert_eq!(p.name, "");
    }

    #[test]
    fn profile_reads_base_stats() {
        // Seven DISTINCT values, so a permuted field mapping cannot pass.
        let mut b = vec![0u8; 990];
        for (i, v) in [11u32, 22, 33, 44, 55, 66, 77].iter().enumerate() {
            b[962 + 4 * i..966 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        let p = parse_player_profile(&b).unwrap();
        // Wire order is STR/STA/CHA/DEX/INT/AGI/WIS.
        assert_eq!(
            (p.str_, p.sta, p.cha, p.dex, p.int_, p.agi, p.wis),
            (11, 22, 33, 44, 55, 66, 77)
        );
    }

    #[test]
    fn profile_short_buffer_leaves_base_stats_zero() {
        // One byte short of the block: read is skipped, not partial, no panic.
        let p = parse_player_profile(&vec![0u8; 989]).unwrap();
        assert_eq!(
            (p.str_, p.sta, p.cha, p.dex, p.int_, p.agi, p.wis),
            (0, 0, 0, 0, 0, 0, 0)
        );
    }

    /// Synthetic profile: 34-byte identity header, then unmapped-middle junk
    /// (zeros — no 0x40, so no false name-block match), then the name block
    /// (`u32 64` + 64B name + `u32 32` + 32B lastname) and the positional tail.
    fn profile_with_name_block(name: &str, last: &str) -> Vec<u8> {
        let mut b = vec![0u8; 34];
        b[20] = 1; // gender
        b[21..25].copy_from_slice(&6u32.to_le_bytes()); // race
        b[25..29].copy_from_slice(&5u32.to_le_bytes()); // class (primary)
        b[29..33].copy_from_slice(&0b111u32.to_le_bytes()); // classMask (not surfaced)
        b[33] = 12; // level
        b.extend_from_slice(&[0u8; 200]); // unmapped middle

        // name block
        b.extend_from_slice(&64u32.to_le_bytes());
        let mut nbuf = [0u8; 64];
        nbuf[..name.len()].copy_from_slice(name.as_bytes());
        b.extend_from_slice(&nbuf);
        b.extend_from_slice(&32u32.to_le_bytes());
        let mut lbuf = [0u8; 32];
        lbuf[..last.len()].copy_from_slice(last.as_bytes());
        b.extend_from_slice(&lbuf);

        // tail
        for v in [111u32, 222, 333, 444] {
            b.extend_from_slice(&v.to_le_bytes()); // birthday/create/save/played
        }
        b.extend_from_slice(&[0u8; 4]); // unknown
        b.extend_from_slice(&0xFFu32.to_le_bytes()); // expansions
        b.extend_from_slice(&[0u8; 4]); // unknown
        b.extend_from_slice(&1u32.to_le_bytes()); // langCount
        b.push(100); // one language
        b.extend_from_slice(&55u16.to_le_bytes()); // zoneId (LE)
        b.extend_from_slice(&0u16.to_le_bytes()); // zoneInstance (LE)
        b.extend_from_slice(&(-12.5f32).to_le_bytes()); // y
        b.extend_from_slice(&34.5f32.to_le_bytes()); // x
        b.extend_from_slice(&7.0f32.to_le_bytes()); // z
        b.extend_from_slice(&90.0f32.to_le_bytes()); // heading
        b.extend_from_slice(&100u16.to_be_bytes()); // standState (BE)
        b.extend_from_slice(&0u16.to_be_bytes()); // anon (BE)
        b.extend_from_slice(&999u32.to_le_bytes()); // guildID
        b.extend_from_slice(&1u32.to_le_bytes()); // guildServerID
        b.extend_from_slice(&[0u8; 2]); // 2 unknown
        for v in [10u32, 20, 30, 40] {
            b.extend_from_slice(&v.to_le_bytes()); // carried P/G/S/C
        }
        for v in [50u32, 60, 70, 80] {
            b.extend_from_slice(&v.to_le_bytes()); // bank P/G/S/C
        }
        b
    }

    #[test]
    fn profile_anchor_scans_name_surname_and_tail() {
        let b = profile_with_name_block("Testchar", "Surname");
        let p = parse_player_profile(&b).unwrap();
        // identity header (fixed offsets)
        assert_eq!(p.gender, 1);
        assert_eq!(p.race, 6);
        assert_eq!(p.class_, 5);
        assert_eq!(p.level, 12);
        // name block reached by the absolute anchor-scan, not a fixed offset
        assert_eq!(p.name, "Testchar");
        assert_eq!(p.last_name, "Surname");
        // positional tail
        assert_eq!(p.expansions, 0xFF);
        assert_eq!(p.languages, vec![100]);
        assert_eq!(p.zone_id, 55);
        assert_eq!(p.x, 34.5);
        assert_eq!(p.y, -12.5);
        assert_eq!(p.z, 7.0);
        assert_eq!(p.heading, 90.0);
        assert_eq!(p.stand_state, 100);
        assert_eq!(p.guild_id, 999);
        assert_eq!(p.guild_server_id, 1);
        // Money is deliberately NOT part of this walk any more — it is read from
        // fixed offsets instead (see money_at_fixed_offsets below), so a short
        // synthetic profile like this one carries none.
        assert_eq!(p.platinum, 0);
        assert_eq!(p.copper, 0);
    }

    #[test]
    fn money_at_fixed_offsets() {
        // Carried coin sits at 33687 and cursor at 33703, both inside the
        // fixed prefix; the bank follows the inventory MIRROR of the carried
        // quadruple, located by signature rather than a constant.
        let mut b = vec![0u8; 40000];
        let put = |b: &mut Vec<u8>, off: usize, v: u32| {
            b[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        for (i, v) in [9275u32, 10, 25, 47].iter().enumerate() {
            put(&mut b, 33687 + i * 4, *v); // carried
            put(&mut b, 36245 + i * 4, *v); // inventory mirror
        }
        for (i, v) in [1234u32, 5, 3, 5].iter().enumerate() {
            put(&mut b, 36261 + i * 4, *v); // bank, right after the mirror
        }
        let p = parse_player_profile(&b).unwrap();
        assert_eq!((p.platinum, p.gold, p.silver, p.copper), (9275, 10, 25, 47));
        assert_eq!(
            (p.platinum_bank, p.gold_bank, p.silver_bank, p.copper_bank),
            (1234, 5, 3, 5)
        );
        // Nothing on the cursor in this fixture.
        assert_eq!((p.platinum_cursor, p.copper_cursor), (0, 0));
    }

    #[test]
    fn empty_purse_leaves_bank_unread() {
        // An all-zero carried quadruple would match padding anywhere, so the
        // mirror scan is skipped rather than latching onto the first zeros.
        let b = vec![0u8; 40000];
        let p = parse_player_profile(&b).unwrap();
        assert_eq!((p.platinum, p.platinum_bank), (0, 0));
    }

    #[test]
    fn profile_without_name_block_keeps_identity_only() {
        // No name-block signature anywhere → name stays empty, identity stands,
        // and the daemon falls back to own-spawn name adoption.
        let mut b = vec![0u8; 500];
        b[21..25].copy_from_slice(&6u32.to_le_bytes());
        b[33] = 12;
        let p = parse_player_profile(&b).unwrap();
        assert_eq!(p.name, "");
        assert_eq!(p.last_name, "");
        assert_eq!(p.race, 6);
        assert_eq!(p.level, 12);
    }

    #[test]
    fn profile_truncated_tail_still_yields_name() {
        // Name block intact but the tail is cut off: name/lastname still land,
        // the truncated tail fields degrade to Default without panicking.
        let mut b = profile_with_name_block("Halfway", "");
        b.truncate(b.len() - 30);
        let p = parse_player_profile(&b).unwrap();
        assert_eq!(p.name, "Halfway");
    }

    #[test]
    fn new_zone_reads_packed_names() {
        // OP_NewZone layout: short, long, zone file, then environment.
        let mut b = Vec::new();
        b.extend_from_slice(b"guktop\0");
        b.extend_from_slice(b"The City of Guk\0");
        b.extend_from_slice(&[0u8; 2]);
        b.extend_from_slice(b"guktop.eqg\0");
        b.extend_from_slice(&[0u8; 90]);
        b.extend_from_slice(&1.0f32.to_le_bytes());
        b.extend_from_slice(&[0u8; 28]);
        b.extend_from_slice(&2.0f32.to_le_bytes());
        b.extend_from_slice(&1.0f32.to_le_bytes());
        b.extend_from_slice(&3.0f32.to_le_bytes());
        let z = parse_new_zone(&b).unwrap();
        assert_eq!(z.short_name, "guktop");
        assert_eq!(z.long_name, "The City of Guk");
        assert_eq!(z.zonefile, "guktop.eqg");
    }

    #[test]
    fn new_zone_rejects_unterminated() {
        assert!(parse_new_zone(b"noterminator").is_err());
        assert!(parse_new_zone(b"short\0").is_err()); // no long name
    }

    // parse_player_self_pos tests live in player_self_pos.rs (the module now owns
    // the canonical parser; these lib.rs copies tested the retired 42B layout).

    /// Encode a game-unit coordinate as a wire position word: signed 19-bit
    /// ×8 fixed-point in the low bits.
    fn pos_word(game_units: i32) -> [u8; 4] {
        (((game_units * 8) as u32) & 0x7FFFF).to_le_bytes()
    }

    /// Assemble a full eql zone-spawn payload matching the `fillSpawnStruct`
    /// walk. Uses the NPC / non-humanoid path (npc=1, race>12 → the 2-slot
    /// equipment branch); the 9-slot humanoid branch is exercised by the goldens.
    #[allow(clippy::too_many_arguments)]
    fn build_spawn(
        name: &str,
        id: u32,
        level: u8,
        cur_hp: u8,
        race: u32,
        deity: u32,
        class_: u32,
        z: i32,
        y: i32,
        x: i32,
        heading: u16,
        last: &str,
        title: &str,
        suffix: &str,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        let text = |b: &mut Vec<u8>, s: &str| {
            b.extend_from_slice(s.as_bytes());
            b.push(0);
        };
        let u32le = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        text(&mut b, name);
        u32le(&mut b, id);
        b.push(level);
        b.extend_from_slice(&[0u8; 16]);
        b.push(1); // npc
        u32le(&mut b, 0); // miscData
        b.push(0); // otherData
        b.extend_from_slice(&[0u8; 8]);
        b.push(0); // charProperties = 0 (no bodytype loop)
        b.extend_from_slice(&[0u8; 3]);
        b.push(0); // appearanceCount = 0
        b.extend_from_slice(&[0u8; 4]);
        b.push(cur_hp);
        b.extend_from_slice(&[0u8; 33]);
        u32le(&mut b, race);
        b.push(0); // holding
        u32le(&mut b, deity);
        u32le(&mut b, 0); // guildID
        u32le(&mut b, 0); // guildServerID
        u32le(&mut b, class_);
        u32le(&mut b, 0); // classMask
        b.push(0); // skip1
        b.push(0); // state
        b.push(0); // light
        b.push(0); // skip1
        text(&mut b, last); // lastName
        b.extend_from_slice(&[0u8; 2]);
        u32le(&mut b, 0); // petOwnerId
        b.extend_from_slice(&[0u8; 49]); // npc==1 extra
        b.extend_from_slice(&[0u8; 60]); // equipment (else branch: 20 + 2*5*4)
                                         // 2026-08-18 position block (upstream `posData` shape + their
                                         // transposing call site): w0 = pad, w1 low-19 = Z, w2 = heading
                                         // word, w3 low-19 = X, w4 low-19 = Y, w5 = 0. Building the pad
                                         // and the heading word as NON-zero values here is deliberate: an
                                         // earlier bug read a coordinate out of the pad, and a zero-filled
                                         // one would have let that reading pass.
        b.extend_from_slice(&(0x1234_5678u32).to_le_bytes()); // w0: pad — deliberately not zero
        b.extend_from_slice(&pos_word(z)); // w1: Z in the low 19
        u32le(&mut b, u32::from(heading) & 0xFFF); // w2: heading word (not decoded yet)
        b.extend_from_slice(&pos_word(x)); // w3: X in the low 19 (upstream's `y`)
        b.extend_from_slice(&pos_word(y)); // w4: Y in the low 19 (upstream's `x`)
        u32le(&mut b, 0); // w5 (always 0 on the wire)
        text(&mut b, title);
        text(&mut b, suffix);
        text(&mut b, ""); // string 3
        text(&mut b, ""); // string 4
        b.extend_from_slice(&[0u8; 4]); // 4 fixed
        b.push(0); // isMercenary
        text(&mut b, "0"); // ASCII digit string
        b.extend_from_slice(&[0u8; 53]); // 53 fixed tail
        b
    }

    #[test]
    fn spawn_full_walk_reads_all_fields() {
        let b = build_spawn(
            "a guard",
            4242,
            55,
            90,
            14,
            396,
            3,
            80,
            -15,
            10,
            1234,
            "",
            "Protector",
            "of Qeynos",
        );
        let s = parse_spawn(&b).unwrap();
        assert_eq!(s.id, 4242);
        assert_eq!(s.name, "a guard");
        assert_eq!(s.level, 55);
        assert_eq!(s.cur_hp, 90);
        assert_eq!(s.max_hp, 100);
        assert_eq!(s.race, 14);
        assert_eq!(s.deity, 396);
        assert_eq!(s.class_, 3);
        assert_eq!(s.npc, 1);
        assert_eq!(s.x, 10);
        assert_eq!(s.y, -15);
        assert_eq!(s.z, 80);
        // The facing no longer survives the 07/28 position block; it is
        // reported as 0 rather than read from the bits that now hold Y.
        assert_eq!(s.heading, 0);
        // titled spawn: title then suffix out of the tail-anchored string block
        assert_eq!(s.title, "Protector");
        assert_eq!(s.suffix, "of Qeynos");
    }

    #[test]
    fn spawn_last_name_and_position_past_i16_window() {
        // far spawn (|y·8| > i16::MAX) must not wrap; surname decodes; no title.
        let b = build_spawn(
            "Grarf",
            7,
            60,
            100,
            14,
            0,
            5,
            12,
            -4700,
            5200,
            0,
            "Ironforge",
            "",
            "",
        );
        let s = parse_spawn(&b).unwrap();
        assert_eq!(s.y, -4700);
        assert_eq!(s.x, 5200);
        assert_eq!(s.last_name, "Ironforge");
        assert_eq!(s.title, "");
        assert_eq!(s.suffix, "");
    }

    #[test]
    fn spawn_rejects_truncated() {
        let mut b = Vec::new();
        b.extend_from_slice(b"orc\0");
        b.extend_from_slice(&[0u8; 40]); // walk overruns the header
        assert!(parse_spawn(&b).is_err());
    }

    #[test]
    fn consider_reads_self_target_faction() {
        let mut b = [0u8; 24];
        b[0..4].copy_from_slice(&27090u32.to_le_bytes()); // self
        b[4..8].copy_from_slice(&11626u32.to_le_bytes()); // target
        b[8..12].copy_from_slice(&4u32.to_le_bytes()); // faction (amiably)
        let c = parse_consider(&b).unwrap();
        assert_eq!(c.player_id, 27090);
        assert_eq!(c.target_id, 11626);
        assert_eq!(c.faction, 4);
        assert_eq!(c.level, 0);
        assert!(parse_consider(&[0u8; 23]).is_err());
    }

    #[test]
    fn stat_sync_narrow_hp_percent() {
        // 6-byte narrow HP feed: u32 id@0, flags@4=0x02 (HP, narrow), percent@5.
        let mut b = [0u8; 6];
        b[0..4].copy_from_slice(&11744u32.to_le_bytes());
        b[4] = 0x02;
        b[5] = 73;
        let s = parse_stat_sync(&b).unwrap();
        assert_eq!(s.spawn_id, 11744);
        assert!(!s.wide);
        assert!(s.has_hp);
        assert_eq!(s.hp_cur, 73);
        assert_eq!(s.hp_max, 100);
        assert!(!s.has_mana && !s.has_end);
    }

    #[test]
    fn stat_sync_wide_hp_mana() {
        // 37-byte wide HP+mana: flags 0x07 (wide|HP|mana), two {i64 cur,max} pairs.
        let mut b = [0u8; 37];
        b[0..4].copy_from_slice(&42u32.to_le_bytes());
        b[4] = 0x07;
        b[5..13].copy_from_slice(&1500i64.to_le_bytes()); // hp cur
        b[13..21].copy_from_slice(&2000i64.to_le_bytes()); // hp max
        b[21..29].copy_from_slice(&300i64.to_le_bytes()); // mana cur
        b[29..37].copy_from_slice(&450i64.to_le_bytes()); // mana max
        let s = parse_stat_sync(&b).unwrap();
        assert!(s.wide);
        assert_eq!(s.spawn_id, 42);
        assert!(s.has_hp && s.hp_cur == 1500 && s.hp_max == 2000);
        assert!(s.has_mana && s.mana_cur == 300 && s.mana_max == 450);
        assert!(!s.has_end);
    }

    #[test]
    fn stat_sync_wide_three_stats_with_tail() {
        // 53+4-byte wide HP+mana+endurance with the optional trailing u32.
        let mut b = [0u8; 57];
        b[0..4].copy_from_slice(&7u32.to_le_bytes());
        b[4] = 0x0f; // wide | HP | mana | endurance
        b[5..13].copy_from_slice(&10i64.to_le_bytes()); // hp cur
        b[13..21].copy_from_slice(&20i64.to_le_bytes()); // hp max
        b[21..29].copy_from_slice(&30i64.to_le_bytes()); // mana cur
        b[29..37].copy_from_slice(&40i64.to_le_bytes()); // mana max
        b[37..45].copy_from_slice(&50i64.to_le_bytes()); // end cur
        b[45..53].copy_from_slice(&60i64.to_le_bytes()); // end max
                                                         // bytes 53..57 = trailing u32, ignored.
        let s = parse_stat_sync(&b).unwrap();
        assert!(s.has_hp && s.hp_cur == 10 && s.hp_max == 20);
        assert!(s.has_mana && s.mana_cur == 30 && s.mana_max == 40);
        assert!(s.has_end && s.end_cur == 50 && s.end_max == 60);
    }

    #[test]
    fn stat_sync_keepalive_narrow_over_100_and_canary() {
        // flags 0x31 = wide|reason, no stat bits → 5-byte keepalive, decodes empty.
        let mut ka = [0u8; 5];
        ka[4] = 0x31;
        let s = parse_stat_sync(&ka).unwrap();
        assert!(!s.has_hp && !s.has_mana && !s.has_end);
        // A narrow percent above 100 drops that stat.
        let mut hot = [0u8; 6];
        hot[4] = 0x02;
        hot[5] = 200;
        assert!(!parse_stat_sync(&hot).unwrap().has_hp);
        // Wrong size for the declared flags is rejected (structural canary).
        let mut bad = [0u8; 8];
        bad[4] = 0x02; // narrow HP → expect 6 or 10 bytes, not 8
        assert!(parse_stat_sync(&bad).is_err());
        // Runt.
        assert!(parse_stat_sync(&[0u8; 4]).is_err());
    }

    #[test]
    fn buff_list_parses_records_and_permanents() {
        let mut b = Vec::new();
        b.extend_from_slice(&100u32.to_le_bytes()); // spawnId
        b.extend_from_slice(&0u32.to_le_bytes()); // @4
        b.push(1); // flag
        b.push(2); // count
        b.extend_from_slice(&[0u8; 5]); // pad
                                        // record 1: spell 278, ticks 5, empty caster, slot 1 (u32 — not final)
        b.extend_from_slice(&278u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&5i32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.push(0); // empty name
        b.extend_from_slice(&1u32.to_le_bytes()); // slot
                                                  // record 2 (final): spell 515, ticks -1 (permanent), caster "X", slot 7 (u16)
        b.extend_from_slice(&515u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&(-1i32).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(b"X\0");
        b.extend_from_slice(&7u16.to_le_bytes()); // final slot is u16

        let list = parse_buff_list(&b).unwrap();
        assert_eq!(list.spawn_id, 100);
        assert_eq!(list.entries.len(), 2);
        assert_eq!(
            list.entries[0],
            BuffListEntry {
                spell_id: 278,
                remaining_ticks: 5,
                slot: 1,
                caster: String::new()
            }
        );
        assert_eq!(
            list.entries[1],
            BuffListEntry {
                spell_id: 515,
                remaining_ticks: -1,
                slot: 7,
                caster: "X".into()
            }
        );
        // Truncation / trailing-garbage both fail the cursor-lands-on-end canary.
        assert!(parse_buff_list(&b[..b.len() - 1]).is_err());
        let mut extra = b.clone();
        extra.push(0xAA);
        assert!(parse_buff_list(&extra).is_err());
    }
}
