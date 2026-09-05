//! Parser for the variable-length spawn payload carried by
//! `OP_ZoneEntry` (DIR_Server) — what scry-cpp historically
//! decoded via `SpawnShell::fillSpawnStruct` (see
//! `scry-cpp/src/spawnshell.cpp:633`).
//!
//! The wire format is a sequence of fixed-width fields punctuated by
//! null-terminated strings, with two conditional skip blocks (NPC vs
//! PC layout, full-equipment vs primary/secondary-only) keyed off
//! NPC class + race. The bitfield-packed `miscData` and `otherData`
//! are passed through as raw u32/u8; the daemon's existing
//! `spawnStruct` union exposes the bit accessors on the C++ side and
//! we don't reinterpret them here.
//!
//! On any read past end-of-buffer we return `SpawnError::Eof` so the
//! daemon can fall back to the C++ path. SZC_None dispatch means the
//! daemon doesn't pre-validate the payload size.

use crate::cursor::{Cursor, CursorError};
use crate::eqstructs::sign_extend;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpawnError {
    #[error("read past end of payload: {0}")]
    Eof(#[from] CursorError),
}

/// Mirror of the `spawnStruct` fields populated by `fillSpawnStruct`,
/// stored as plain data so the FFI bridge can hand the parsed result
/// back to C++ for assignment into the daemon's `spawnStruct`. Text
/// fields cross the FFI as owned strings; the daemon truncates them to
/// the fixed-size buffers in `everquest.h:1058+`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    pub bytes_consumed: u32,

    pub name: String,
    pub last_name: String,
    pub title: String,
    pub suffix: String,

    pub spawn_id: u32,
    pub misc_data: u32,
    pub body_type: u32,
    pub race: u32,
    pub deity: u32,
    pub guild_id: u32,
    pub guild_server_id: u32,
    pub class_: u32,
    pub pet_owner_id: u32,

    /// 9 equipment slots × 5 u32 fields each (itemId, equip3, equip2,
    /// equip1, equip0) — laid out to match `EquipStruct equipment[9]`
    /// on the C++ side so the daemon can assign with a single loop.
    /// Slots not populated by this payload (PC-only with the abridged
    /// equipment layout) stay zero.
    pub equip_data: [u32; 45],
    pub pos_data: [u32; 5],

    pub level: u8,
    pub npc: u8,
    pub other_data: u8,
    pub char_properties: u8,
    pub cur_hp: u8,
    pub holding: u8,
    pub state: u8,
    pub light: u8,
    pub is_mercenary: u8,
}

/// Semantic initial motion decoded from the Live spawn position block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnMotion {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub delta_x: i32,
    pub delta_y: i32,
    pub delta_z: i32,
    pub heading: u16,
    pub delta_heading: i16,
    pub animation: i16,
}

impl Spawn {
    /// Decode the five-word position block after the variable-length spawn
    /// body. Coordinates use 1/8 units and velocity uses 1/4 units, matching
    /// the legacy daemon's `Spawn::update` conversions.
    pub fn motion(&self) -> SpawnMotion {
        let [w0, w1, w2, w3, w4] = self.pos_data;
        SpawnMotion {
            z: sign_extend(w0 & 0x7_ffff, 19) >> 3,
            animation: sign_extend((w0 >> 19) & 0x3ff, 10) as i16,
            y: sign_extend(w1 & 0x7_ffff, 19) >> 3,
            heading: ((w1 >> 19) & 0xfff) as u16,
            delta_y: sign_extend(w2 & 0x1fff, 13) >> 2,
            x: sign_extend(w3 & 0x7_ffff, 19) >> 3,
            delta_x: sign_extend((w3 >> 19) & 0x1fff, 13) >> 2,
            delta_heading: sign_extend(w4 & 0x3ff, 10) as i16,
            delta_z: sign_extend((w4 >> 10) & 0x1fff, 13) >> 2,
        }
    }

    /// The model id is the first word of each five-word equipment record.
    pub fn equipment_models(&self) -> [u32; 9] {
        std::array::from_fn(|slot| self.equip_data[slot * 5])
    }
}

impl Default for Spawn {
    fn default() -> Self {
        Self {
            bytes_consumed: 0,
            name: String::new(),
            last_name: String::new(),
            title: String::new(),
            suffix: String::new(),
            spawn_id: 0,
            misc_data: 0,
            body_type: 0,
            race: 0,
            deity: 0,
            guild_id: 0,
            guild_server_id: 0,
            class_: 0,
            pet_owner_id: 0,
            equip_data: [0; 45],
            pos_data: [0; 5],
            level: 0,
            npc: 0,
            other_data: 0,
            char_properties: 0,
            cur_hp: 0,
            holding: 0,
            state: 0,
            light: 0,
            is_mercenary: 0,
        }
    }
}

/// Bits in `otherData` — needed to drive the two conditional reads
/// (aura strings, optional title/suffix). Mirrors
/// `everquest.h:1093-1100`.
const OTHER_DATA_AURA: u8 = 1 << 2;
const OTHER_DATA_HAS_TITLE: u8 = 1 << 4;
const OTHER_DATA_HAS_SUFFIX: u8 = 1 << 5;

fn bytes_to_string(src: &[u8]) -> String {
    String::from_utf8_lossy(src).into_owned()
}

pub fn parse_spawn(bytes: &[u8]) -> Result<Spawn, SpawnError> {
    let mut c = Cursor::new(bytes);
    let mut out = Spawn::default();

    let name = c.read_cstr()?;
    out.name = bytes_to_string(name);

    out.spawn_id = c.read_u32_le()?;
    out.level = c.read_u8()?;
    c.skip(16)?;
    out.npc = c.read_u8()?;
    out.misc_data = c.read_u32_le()?;
    out.other_data = c.read_u8()?;
    c.skip(8)?; // unknown3, unknown4

    // The "chest / untargetable" branch keyed off otherData & 1 was
    // disabled in the daemon (see comment block at spawnshell.cpp:670).
    // Mirror that — don't read those bytes.

    if (out.other_data & OTHER_DATA_AURA) != 0 {
        // 3 variable-length strings + 50 static bytes
        c.read_cstr()?;
        c.read_cstr()?;
        c.read_cstr()?;
        c.skip(50)?;
    }

    out.char_properties = c.read_u8()?;
    if out.char_properties == 0 {
        // bodytype stays 0 — observed since the 2013-01-16 patch in
        // Field of Scale. See spawnshell.cpp:717.
        out.body_type = 0;
    } else {
        // Read N u32s; the FIRST is bodytype, the rest are discarded
        // (the `if(i == spawn->charProperties)` check fires only on
        // the first iteration of the descending loop).
        out.body_type = c.read_u32_le()?;
        for _ in 1..out.char_properties {
            let _ = c.read_u32_le()?;
        }
    }

    out.cur_hp = c.read_u8()?;
    c.skip(35)?; // facestyle, walk/run speeds, unknown5

    out.race = c.read_u32_le()?;
    out.holding = c.read_u8()?;
    out.deity = c.read_u32_le()?;
    out.guild_id = c.read_u32_le()?;
    out.guild_server_id = c.read_u32_le()?;
    // guildstatus disappeared 2018-11-14; the daemon hard-codes 0.
    out.class_ = c.read_u32_le()?;

    c.skip(1)?;
    out.state = c.read_u8()?;
    out.light = c.read_u8()?;
    c.skip(1)?;

    let last_name = c.read_cstr()?;
    // Daemon's strict check: only surface when the name fits the
    // legacy 32-byte buffer (room for NUL). Longer names silently
    // dropped; the daemon's strncpy on the other side would otherwise
    // overflow.
    if !last_name.is_empty() && last_name.len() < 32 {
        out.last_name = bytes_to_string(last_name);
    }

    c.skip(2)?;
    out.pet_owner_id = c.read_u32_le()?;

    // 12-byte NPC-only block added 2013-06-19 — daemon skips 49 vs
    // 37 based on NPC type.
    c.skip(if out.npc == 1 { 49 } else { 37 })?;

    let race = out.race;
    let read_full_equip =
        out.npc == 0 || race <= 12 || race == 128 || race == 130 || race == 330 || race == 522;
    if read_full_equip {
        c.skip(36)?; // equipment colors
        for slot in 0..9 {
            let base = slot * 5;
            out.equip_data[base] = c.read_u32_le()?; // itemId
            out.equip_data[base + 1] = c.read_u32_le()?; // equip3
            out.equip_data[base + 2] = c.read_u32_le()?; // equip2
            out.equip_data[base + 3] = c.read_u32_le()?; // equip1
            out.equip_data[base + 4] = c.read_u32_le()?; // equip0
        }
    } else {
        c.skip(20)?;
        for slot in [7usize, 8] {
            let base = slot * 5;
            out.equip_data[base] = c.read_u32_le()?;
            out.equip_data[base + 1] = c.read_u32_le()?;
            out.equip_data[base + 2] = c.read_u32_le()?;
            out.equip_data[base + 3] = c.read_u32_le()?;
            out.equip_data[base + 4] = c.read_u32_le()?;
        }
    }

    // Position block: 5 raw u32 (the bitfield packing is decoded C++-side via
    // spawnStruct's position union). Was 6 in feat's era — a post-feat patch
    // dropped one u32 (posData[6] -> posData[5]).
    for slot in 0..5 {
        out.pos_data[slot] = c.read_u32_le()?;
    }

    if (out.other_data & OTHER_DATA_HAS_TITLE) != 0 {
        out.title = bytes_to_string(c.read_cstr()?);
    }
    if (out.other_data & OTHER_DATA_HAS_SUFFIX) != 0 {
        out.suffix = bytes_to_string(c.read_cstr()?);
    }

    c.skip(8)?; // unknowns
    out.is_mercenary = c.read_u8()?;
    c.skip(66)?; // unknowns — end of payload

    out.bytes_consumed = c.pos() as u32;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_errors_immediately() {
        // First op is read_cstr, which tolerates empty (returns empty),
        // but subsequent read_u32_le hits EOF.
        assert!(parse_spawn(&[]).is_err());
    }

    #[test]
    fn truncated_after_name_errors() {
        let mut buf = b"NPCName\0".to_vec();
        // No spawnId / level / etc after the name.
        assert!(parse_spawn(&buf).is_err());
        // Add some bytes but stop before all 16 mid-skip bytes are present.
        buf.extend_from_slice(&[0x42; 4]); // spawnId
        buf.push(50); // level
        buf.extend_from_slice(&[0; 10]); // partial 16-byte skip
        assert!(parse_spawn(&buf).is_err());
    }

    /// Synthesize a minimal NPC payload (no title/suffix/aura, NPC=1
    /// so the 49-byte skip applies, race that triggers the abridged
    /// equipment block) and verify field-by-field decode.
    #[test]
    fn happy_path_npc_no_aura_no_title() {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"a goblin\0");
        buf.extend_from_slice(&123u32.to_le_bytes()); // spawnId
        buf.push(40); // level
        buf.extend_from_slice(&[0; 16]); // skip
        buf.push(1); // NPC
        buf.extend_from_slice(&0xCAFEBABEu32.to_le_bytes()); // miscData
        buf.push(0); // otherData (no aura/title/suffix)
        buf.extend_from_slice(&[0; 8]); // unknown3/4
        buf.push(2); // charProperties
                     // 2 u32 reads — first is bodytype, second discarded
        buf.extend_from_slice(&21u32.to_le_bytes());
        buf.extend_from_slice(&99u32.to_le_bytes());
        buf.push(95); // curHp
        buf.extend_from_slice(&[0; 35]); // facestyle/speeds/unknown5
        buf.extend_from_slice(&50u32.to_le_bytes()); // race (>12, not in special list)
        buf.push(7); // holding
        buf.extend_from_slice(&3u32.to_le_bytes()); // deity
        buf.extend_from_slice(&111u32.to_le_bytes()); // guildID
        buf.extend_from_slice(&222u32.to_le_bytes()); // guildServerID
        buf.extend_from_slice(&5u32.to_le_bytes()); // class_
        buf.push(0); // skip 1
        buf.push(11); // state
        buf.push(2); // light
        buf.push(0); // skip 1
        buf.extend_from_slice(b"\0"); // empty lastName
        buf.extend_from_slice(&[0; 2]); // skip 2
        buf.extend_from_slice(&777u32.to_le_bytes()); // petOwnerId
        buf.extend_from_slice(&[0; 49]); // NPC=1 skip
                                         // Abridged equipment (race=50 NPC=1, not in special list).
        buf.extend_from_slice(&[0; 20]); // skip 20
                                         // slot 7 + slot 8 = 10 u32s
        for v in [70u32, 71, 72, 73, 74, 80, 81, 82, 83, 84] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        // posData[5]
        for v in [1u32, 2, 3, 4, 5] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&[0; 8]); // unknowns
        buf.push(1); // isMercenary
        buf.extend_from_slice(&[0; 66]); // tail unknowns

        let s = parse_spawn(&buf).unwrap();
        assert_eq!(s.name, "a goblin");
        assert_eq!(s.spawn_id, 123);
        assert_eq!(s.level, 40);
        assert_eq!(s.npc, 1);
        assert_eq!(s.misc_data, 0xCAFEBABE);
        assert_eq!(s.other_data, 0);
        assert_eq!(s.char_properties, 2);
        assert_eq!(s.body_type, 21);
        assert_eq!(s.cur_hp, 95);
        assert_eq!(s.race, 50);
        assert_eq!(s.holding, 7);
        assert_eq!(s.deity, 3);
        assert_eq!(s.guild_id, 111);
        assert_eq!(s.guild_server_id, 222);
        assert_eq!(s.class_, 5);
        assert_eq!(s.state, 11);
        assert_eq!(s.light, 2);
        assert_eq!(s.pet_owner_id, 777);
        // Abridged equip: slots 0..6 zero, slot 7 + 8 populated.
        for i in 0..7 {
            for k in 0..5 {
                assert_eq!(s.equip_data[i * 5 + k], 0, "slot {} field {}", i, k);
            }
        }
        assert_eq!(
            &s.equip_data[35..45],
            &[70, 71, 72, 73, 74, 80, 81, 82, 83, 84]
        );
        assert_eq!(s.equipment_models(), [0, 0, 0, 0, 0, 0, 0, 70, 80]);
        assert_eq!(s.pos_data, [1, 2, 3, 4, 5]);
        assert_eq!(s.is_mercenary, 1);
        assert_eq!(s.bytes_consumed as usize, buf.len());
    }

    #[test]
    fn position_block_decodes_initial_motion_without_host_defaults() {
        let signed = |value: i32, bits: u32| (value as u32) & ((1 << bits) - 1);
        let spawn = Spawn {
            pos_data: [
                signed(-24, 19) | (signed(-7, 10) << 19),
                signed(80, 19) | (3072 << 19),
                signed(-20, 13),
                signed(160, 19) | (signed(28, 13) << 19),
                signed(-9, 10) | (signed(36, 13) << 10),
            ],
            ..Spawn::default()
        };

        assert_eq!(
            spawn.motion(),
            SpawnMotion {
                x: 20,
                y: 10,
                z: -3,
                delta_x: 7,
                delta_y: -5,
                delta_z: 9,
                heading: 3072,
                delta_heading: -9,
                animation: -7,
            }
        );
    }

    #[test]
    fn full_equipment_branch_via_pc() {
        // NPC=0 forces the full-equipment branch (skip 36 + 9 slots).
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"\0"); // empty name
        buf.extend_from_slice(&1u32.to_le_bytes()); // spawnId
        buf.push(60); // level
        buf.extend_from_slice(&[0; 16]);
        buf.push(0); // NPC=0 (player)
        buf.extend_from_slice(&[0; 4]); // miscData
        buf.push(OTHER_DATA_HAS_TITLE | OTHER_DATA_HAS_SUFFIX);
        buf.extend_from_slice(&[0; 8]);
        buf.push(0); // charProperties (=0 so no bodytype loop)
        buf.push(100); // curHp
        buf.extend_from_slice(&[0; 35]);
        buf.extend_from_slice(&12u32.to_le_bytes()); // race (<=12 → would match anyway)
        buf.push(0); // holding
        buf.extend_from_slice(&[0; 16]); // deity, guildID, guildServerID, class_
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.extend_from_slice(b"\0"); // empty lastName
        buf.extend_from_slice(&[0; 2]);
        buf.extend_from_slice(&[0; 4]); // petOwnerId
        buf.extend_from_slice(&[0; 37]); // NPC!=1 → skip 37
                                         // Full equip: 36 skip + 45 u32
        buf.extend_from_slice(&[0; 36]);
        for slot in 0..9 {
            for k in 0..5 {
                let v = (slot as u32) * 100 + k as u32;
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in [10u32, 20, 30, 40, 50] {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(b"My Title\0");
        buf.extend_from_slice(b"the Suffixed\0");
        buf.extend_from_slice(&[0; 8]);
        buf.push(0);
        buf.extend_from_slice(&[0; 66]);

        let s = parse_spawn(&buf).unwrap();
        assert_eq!(s.npc, 0);
        assert_eq!(s.body_type, 0);
        // First slot fields = (0, 1, 2, 3, 4); slot 8 = (800..804).
        assert_eq!(&s.equip_data[..5], &[0, 1, 2, 3, 4]);
        assert_eq!(&s.equip_data[40..45], &[800, 801, 802, 803, 804]);
        assert_eq!(s.title, "My Title");
        assert_eq!(s.suffix, "the Suffixed");
    }

    #[test]
    fn aura_branch_consumes_three_strings_plus_50() {
        // Build a payload up through otherData=4 (aura bit), then 3
        // strings + 50 bytes, then enough remaining tail to satisfy
        // the rest of the parser. Easier: verify the cursor advanced
        // the right amount in isolation.
        let prefix_len = 1 + 4 + 1 + 16 + 1 + 4 + 1 + 8;
        let mut buf: Vec<u8> = vec![0; prefix_len];
        buf[0] = 0; // empty name (single NUL)
        buf[1 + 4 + 1 + 16] = 0; // NPC
                                 // otherData byte sits at offset (1+4+1+16+1+4) = 27
        buf[27] = OTHER_DATA_AURA;
        // Append aura's 3 strings + 50 bytes
        buf.extend_from_slice(b"a\0b\0c\0");
        buf.extend_from_slice(&[0; 50]);
        // Append minimal valid tail so the parser doesn't EOF on
        // unrelated reads.
        buf.push(0); // charProperties
        buf.push(0); // curHp
        buf.extend_from_slice(&[0; 35]);
        buf.extend_from_slice(&0u32.to_le_bytes()); // race
        buf.push(0);
        buf.extend_from_slice(&[0; 16]);
        buf.extend_from_slice(&[0; 4]);
        buf.extend_from_slice(b"\0");
        buf.extend_from_slice(&[0; 2]);
        buf.extend_from_slice(&[0; 4]);
        buf.extend_from_slice(&[0; 37]); // NPC=0 → 37
                                         // race=0 ⇒ <=12 ⇒ full equip
        buf.extend_from_slice(&[0; 36 + 45 * 4]);
        buf.extend_from_slice(&[0; 6 * 4]);
        buf.extend_from_slice(&[0; 8]);
        buf.push(0);
        buf.extend_from_slice(&[0; 66]);

        let s = parse_spawn(&buf).unwrap();
        assert_eq!(s.other_data, OTHER_DATA_AURA);
    }
}
