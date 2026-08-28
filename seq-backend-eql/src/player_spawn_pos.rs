//! Parser for eql's 24-byte `playerSpawnPosStruct` (`OP_ClientUpdate`,
//! DIR_Server only — position broadcast for spawns other than the local player).
//!
//! **This is eql's OWN copy**; when eql and Live differ, only this copy changes
//! (Live's `parse_player_spawn_pos` lives in `seq-decode`, untouched).
//!
//! **Re-laid-out 2026-08-18** from upstream legends `1cd04be`
//! (`playerPosUpdateEQLStruct`). The 08/18 patch rearranged the body again at
//! the same 24-byte size — the third rearrangement in three patches, and again
//! one no size gate can catch. Every coordinate now sits in the **low** 19 bits
//! of its word (signed, ×8 fixed-point); the high-19 z of the 08/04 layout is
//! gone:
//!
//! ```text
//!   /*0000*/ u16  spawnId
//!   /*0002*/ u16  spawnId2         (0 in every pre-patch sample)
//!   /*0004*/ u32  unknown          (role TBD — carried x before this patch)
//!   /*0008*/ u32  { z:19 (low, signed) | deltaZ:13 }
//!   /*0012*/ u32  unknown          (role TBD)
//!   /*0016*/ u32  { x:19 (low, signed) | heading @bit19 | pad:1 }
//!   /*0020*/ u32  { y:19 (low, signed) | deltaY:13 }
//! ```
//!
//! Only the heading kept its home: it is still the field at bit 19 of the @16
//! word. What moved under it is the coordinate sharing that word — `y` before
//! this patch, `x` now.
//!
//! **UNVALIDATED LOCALLY — no post-patch capture exists yet.** The 08/04 layout
//! was pinned by scoring all 173 candidate 19-bit windows against the
//! `OP_MobUpdate` / `OP_NpcMoveUpdate` streams; that scan has not been re-run
//! for 08/18 because there is no recording from this wire. This layout is
//! upstream's derivation taken as data. Re-run the scan on the first post-patch
//! capture before treating any axis here as confirmed — upstream and we have
//! disagreed on a word index before (see the ZoneEntry `posData` note in
//! `lib.rs`), and a transposed x/y decodes into a plausible-looking map.
//!
//! Previous layouts, kept so a re-derivation can tell drift from a bad read:
//! 08/04–08/05 was x @4 low-19 / z @12 high-19 / y @16 low-19, heading @16 bit19;
//! before that a 28-byte body with `spawnId@0, z@4, x@8, y@12`.
//!
//! This parser surfaces the *raw* sign-extended coords and the daemon applies
//! `>> 3` (1/8-unit -> integer game world), matching the `EqlDispatch::mobUpdate`
//! path. Deltas/pitch/animation have no located field and read 0.

use crate::eqstructs::sign_extend;
use thiserror::Error;

pub const PAYLOAD_LEN: usize = 28;

/// Full circle in wire units for [`PlayerSpawnPos::heading`].
///
/// The 08/25 patch moved the facing to bit 160. Upstream declares it
/// `heading:8` on a 256-step circle and consumes it as `pu->heading & 0xff`;
/// that is wrong. Measured against travel bearing over 3204 legs from the
/// 08/25 capture, an 11-bit field on a 2048-step circle scores a **0.63
/// degree** median, where upstream's 8-bit/256 read scores 95.61 — noise —
/// and a 12-bit/4096 read scores 69.50. Width and scale are independent:
/// keep 11 bits at 2048. The field sits in the gap between z and y (bits
/// 147..171), so 11 bits fit with one spare bit before y at 172.
pub const HEADING_UNITS: u16 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSpawnPos {
    pub spawn_id: u16,
    pub spawn_id2: u16,
    /// Raw 19-bit signed; daemon applies `>> 3` for fixed-point conv.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// No located field on eql's 28B wire — surfaced as 0.
    pub delta_x: i32,
    pub delta_y: i32,
    pub delta_z: i32,
    /// Compass value (0..2047, see [`HEADING_UNITS`]); 0 = N, increasing
    /// clockwise, NOT inverted. `SpawnShell::moveSpawn` takes no heading, so the
    /// daemon currently ignores this; it is decoded so callers that want a
    /// facing don't have to re-derive it.
    pub heading: u16,
    /// Not carried on eql's 28B wire — surfaced as 0.
    pub delta_heading: i16,
    pub animation: i16,
    pub pitch: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlayerSpawnPosError {
    #[error("expected {PAYLOAD_LEN} bytes, got {0}")]
    BadLength(usize),
}

fn read_u16_le(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u64_le(bytes: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(b)
}

pub fn parse_player_spawn_pos(bytes: &[u8]) -> Result<PlayerSpawnPos, PlayerSpawnPosError> {
    if bytes.len() != PAYLOAD_LEN {
        return Err(PlayerSpawnPosError::BadLength(bytes.len()));
    }

    let spawn_id = read_u16_le(bytes, 0);
    let spawn_id2 = read_u16_le(bytes, 2);

    // 08/25: the record grew 24 -> 28B and the fields are no longer word
    // aligned. Map frame, scored against OP_MobUpdate over 423 time-paired
    // records (median abs error, best vs next-best window):
    //     map X  bit 74    13.50 vs 106.75
    //     Z      bit 128    1.50 vs   9.50
    //     map Y  bit 172   11.38 vs 852.00
    // Upstream's field named `x` is map X here and their `y` is map Y — this
    // is the one position struct they do NOT transpose at their call site.
    let hi = read_u64_le(bytes, 16);
    let x = sign_extend(((read_u64_le(bytes, 8) >> 10) & 0x7_FFFF) as u32, 19);
    let z = sign_extend((hi & 0x7_FFFF) as u32, 19);
    let y = sign_extend(((hi >> 44) & 0x7_FFFF) as u32, 19);

    let heading = ((hi >> 32) & 0x7FF) as u16;

    Ok(PlayerSpawnPos {
        spawn_id,
        spawn_id2,
        x,
        y,
        z,
        delta_x: 0,
        delta_y: 0,
        delta_z: 0,
        heading,
        delta_heading: 0,
        animation: 0,
        pitch: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_player_spawn_pos(&[0; 24]).is_err()); // the pre-08/25 size is rejected
        assert!(parse_player_spawn_pos(&[0; 27]).is_err());
        assert!(parse_player_spawn_pos(&[0; 29]).is_err());
    }

    #[test]
    fn zero_payload_is_zero() {
        let p = parse_player_spawn_pos(&[0u8; PAYLOAD_LEN]).unwrap();
        assert_eq!(p.spawn_id, 0);
        assert_eq!((p.x, p.y, p.z), (0, 0, 0));
        assert_eq!(p.heading, 0);
    }

    // Each axis at its own offset, with the neighbouring bits set, so a word
    // that shifts under a future rearrangement fails loudly instead of reading
    // a plausible number out of the wrong field.
    #[test]
    fn each_coordinate_reads_from_its_own_field() {
        let mut buf = [0u8; PAYLOAD_LEN];
        buf[0..2].copy_from_slice(&0x1151u16.to_le_bytes()); // spawnId 4433

        // Every bit outside the three coordinate fields is set, so a parser
        // that slips by even one bit reads a wrong value instead of a
        // plausible 0. x = bit 74, z = bit 128, y = bit 172.
        let lo = !(0x7_FFFFu64 << 10);
        buf[8..16].copy_from_slice(&(lo | (300u64 << 10)).to_le_bytes()); // x = 300
        let hi = !(0x7_FFFFu64 | (0x7_FFFFu64 << 44));
        buf[16..24].copy_from_slice(&(hi | 42u64 | (0x0004_0000u64 << 44)).to_le_bytes());
        let p = parse_player_spawn_pos(&buf).unwrap();
        assert_eq!(p.spawn_id, 0x1151);
        assert_eq!(p.x, 300);
        assert_eq!(p.z, 42);
        assert_eq!(p.y, -262_144); // 19-bit signed minimum
    }

    // A real 08/25 broadcast whose spawn also appears in OP_MobUpdate at the
    // same moment. That stream packs the same position in an unrelated layout
    // and agrees exactly on x, y, z AND heading — which is what pins the
    // 11-bit heading here: upstream's 8-bit read truncates 1512 to 232.
    #[test]
    fn decodes_a_captured_broadcast() {
        let bytes: [u8; PAYLOAD_LEN] = [
            0x92, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xB0, 0x58, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x51, 0xFF, 0x87, 0x07, 0xE8, 0xC5, 0xEB, 0x7D, 0x00, 0x00, 0x00, 0x00,
        ];
        let p = parse_player_spawn_pos(&bytes).unwrap();
        assert_eq!(p.spawn_id, 25490);
        // the parser surfaces raw 19-bit values; the daemon applies >> 3
        assert_eq!((p.x >> 3, p.y >> 3, p.z >> 3), (709, -1065, -22));
        assert_eq!(p.heading, 1512);
    }

    #[test]
    fn heading_is_eleven_bits_between_z_and_y() {
        let mut buf = [0u8; PAYLOAD_LEN];
        // A quarter-circle heading at bit 160 with every neighbouring bit set:
        // upstream's 8-bit read would truncate it, and a 12-bit read would
        // borrow y's low bit. Both fail here.
        let quarter = u64::from(HEADING_UNITS) / 4;
        let hi = !(0x7FFu64 << 32) | (quarter << 32);
        buf[16..24].copy_from_slice(&hi.to_le_bytes());
        let p = parse_player_spawn_pos(&buf).unwrap();
        assert_eq!(p.heading, HEADING_UNITS / 4);
        assert!(p.heading < HEADING_UNITS);
    }
}
