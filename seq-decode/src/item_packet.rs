//! Parser for Live's `OP_ItemPacket` — ONE item per fire, ~946-1011B.
//!
//! Unlike eql's bulk packet (every item in one ~300KB fire), Live delivers a
//! single item on each slot move and on zone-in pickup, so a consumer
//! ACCUMULATES rather than replacing its cache.
//!
//! ```text
//! wrapper  u32 packetType (0x74/0x76/0x78)
//!          char instanceId[16] + NUL      per-INSTANCE id
//!          u32 stackCount @21
//!          u32 mainSlot   @25   0 = top-level worn/inventory/cursor
//!          u16 subSlot    @29   with mainSlot==0, the worn/inventory slot
//! names    <name> NUL <lore name> NUL     located by scan, NOT a fixed offset
//! body     parsedItemTemplateStruct, offsets relative to its start
//! ```
//!
//! The names are found by scanning for the first uppercase ASCII letter
//! preceded by a NUL, because the header before them is jumbled binary of
//! variable length — a fixed offset does not survive. The body then starts
//! after the SECOND NUL, so its fields are located relative to the names, never
//! to the packet start. Ported from the daemon's `src/itempacket.cpp`.

use thiserror::Error;

/// Scan starts past the leading instance-id ASCII so it can't match that.
const HEADER_PROBE_START: usize = 0x40;
/// Body prefix we actually read, through `ac` at +59..63.
const BODY_MIN: usize = 63;
const RESIST_COUNT: usize = 5;
const STAT_COUNT: usize = 7;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveItem {
    pub packet_type: u32,
    pub instance_id: String,
    pub stack_count: u32,
    /// 0 = top-level (worn / inventory / cursor); otherwise the parent bag slot.
    pub main_slot: u32,
    /// With `main_slot == 0` this is the worn/inventory slot index.
    pub sub_slot: u16,
    pub name: String,
    pub lore_name: String,
    pub item_id: u32,
    pub weight: f32,
    /// Exact integer value carried by the wire. `weight` remains for the
    /// opcode-specific compatibility API.
    pub weight_tenths: u32,
    pub flags: u32,
    pub slot_mask: u32,
    pub resists: Vec<i32>,
    pub corruption: i32,
    pub stats: Vec<i32>,
    pub hp: i32,
    pub mana: i32,
    pub endurance: i32,
    pub ac: i32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ItemPacketError {
    #[error("payload too short: {0}")]
    Short(usize),
    #[error("instance-id NUL missing at +20")]
    BadWrapper,
    #[error("item name not found")]
    NoName,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// First uppercase ASCII letter preceded by a NUL — the item name's start.
fn find_name_start(b: &[u8], from: usize) -> Option<usize> {
    (from..b.len().saturating_sub(1)).find(|&i| i > 0 && b[i].is_ascii_uppercase() && b[i - 1] == 0)
}

pub fn parse_item_packet(b: &[u8]) -> Result<LiveItem, ItemPacketError> {
    if b.len() < HEADER_PROBE_START {
        return Err(ItemPacketError::Short(b.len()));
    }
    // The instance id is exactly 16 ASCII chars + NUL in every observed
    // sample. If that NUL is missing the wrapper isn't what we think, and no
    // downstream offset is trustworthy either — bail rather than guess.
    if b[20] != 0 {
        return Err(ItemPacketError::BadWrapper);
    }

    let name_start = find_name_start(b, HEADER_PROBE_START).ok_or(ItemPacketError::NoName)?;
    let first_nul = b[name_start..]
        .iter()
        .position(|&c| c == 0)
        .map(|p| name_start + p)
        .ok_or(ItemPacketError::NoName)?;
    let lore_start = first_nul + 1;
    let second_nul = b[lore_start..]
        .iter()
        .position(|&c| c == 0)
        .map(|p| lore_start + p)
        .ok_or(ItemPacketError::NoName)?;

    let body = second_nul + 1;
    if body + BODY_MIN > b.len() {
        return Err(ItemPacketError::Short(b.len()));
    }
    let p = &b[body..];

    Ok(LiveItem {
        packet_type: u32_at(b, 0),
        instance_id: latin1(&b[4..20]),
        stack_count: u32_at(b, 21),
        main_slot: u32_at(b, 25),
        sub_slot: u16::from_le_bytes([b[29], b[30]]),
        name: latin1(&b[name_start..first_nul]),
        lore_name: latin1(&b[lore_start..second_nul]),
        item_id: u32_at(p, 8),
        weight: u32_at(p, 12) as f32 / 10.0,
        weight_tenths: u32_at(p, 12),
        flags: u32_at(p, 16),
        slot_mask: u32_at(p, 20),
        resists: (0..RESIST_COUNT).map(|i| p[34 + i] as i8 as i32).collect(),
        corruption: p[39] as i8 as i32,
        stats: (0..STAT_COUNT).map(|i| p[40 + i] as i8 as i32).collect(),
        hp: u32_at(p, 47) as i32,
        mana: u32_at(p, 51) as i32,
        endurance: u32_at(p, 55) as i32,
        ac: u32_at(p, 59) as i32,
    })
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wrapper + jumbled header + two names + body, mirroring the wire.
    fn packet(name: &str, lore: &str, main: u32, sub: u16, id: u32) -> Vec<u8> {
        let mut b = vec![0u8; 21];
        b[0..4].copy_from_slice(&0x76u32.to_le_bytes());
        b[4..20].copy_from_slice(b"un000BG0001R0G00");
        b[20] = 0;
        b.extend_from_slice(&7u32.to_le_bytes()); // stack @21
        b.extend_from_slice(&main.to_le_bytes()); // @25
        b.extend_from_slice(&sub.to_le_bytes()); // @29
                                                 // Junk up to the probe start, deliberately including an uppercase
                                                 // letter NOT preceded by a NUL so the scan can't latch onto it.
        while b.len() < HEADER_PROBE_START + 8 {
            b.push(b'Q');
        }
        b.push(0); // the NUL the scan anchors on
        b.extend_from_slice(name.as_bytes());
        b.push(0);
        b.extend_from_slice(lore.as_bytes());
        b.push(0);
        let mut body = vec![0u8; BODY_MIN];
        body[8..12].copy_from_slice(&id.to_le_bytes());
        body[12..16].copy_from_slice(&5u32.to_le_bytes()); // weight 0.5
        body[20..24].copy_from_slice(&18432u32.to_le_bytes()); // slot mask
        body[40] = 3; // STR
        body[59..63].copy_from_slice(&11u32.to_le_bytes()); // ac
        b.extend_from_slice(&body);
        b
    }

    #[test]
    fn parses_wrapper_names_and_body() {
        let it =
            parse_item_packet(&packet("Gloomingdeep Lantern", "A lantern", 41, 3, 9979)).unwrap();
        assert_eq!(it.packet_type, 0x76);
        assert_eq!(it.instance_id, "un000BG0001R0G00");
        assert_eq!(it.stack_count, 7);
        assert_eq!((it.main_slot, it.sub_slot), (41, 3));
        assert_eq!(it.name, "Gloomingdeep Lantern");
        assert_eq!(it.lore_name, "A lantern");
        assert_eq!(it.item_id, 9979);
        assert_eq!(it.weight, 0.5);
        assert_eq!(it.weight_tenths, 5);
        assert_eq!(it.slot_mask, 18432);
        assert_eq!(it.stats[0], 3);
        assert_eq!(it.ac, 11);
        assert_eq!(it.resists.len(), RESIST_COUNT);
    }

    #[test]
    fn body_follows_the_names_not_a_fixed_offset() {
        // A much longer name must not shift the body fields.
        let long = parse_item_packet(&packet(
            "A Very Long Item Name That Pushes Everything Along",
            "Lore",
            0,
            5,
            42,
        ))
        .unwrap();
        assert_eq!(long.item_id, 42);
        assert_eq!(long.ac, 11);
        assert_eq!((long.main_slot, long.sub_slot), (0, 5));
    }

    #[test]
    fn a_missing_wrapper_nul_is_an_error_not_a_guess() {
        let mut p = packet("X", "X", 0, 0, 1);
        p[20] = b'!';
        assert_eq!(parse_item_packet(&p), Err(ItemPacketError::BadWrapper));
    }

    #[test]
    fn short_payloads_error() {
        assert!(matches!(
            parse_item_packet(&[0u8; 8]),
            Err(ItemPacketError::Short(8))
        ));
    }
}
