//! Parser for `OP_NpcMoveUpdate` — variable-length 13..24 byte
//! payload using the legacy MSB-first `BitStream` packing (NOT the
//! C-struct `#[repr(C, packed)]` bitfield convention used elsewhere).
//!
//! Wire format mirrors `SpawnShell::npcMoveUpdate` / the daemon's
//! `BitStream` reader:
//!
//! ```text
//!   16 bits — spawnId (big-endian within the bit stream)
//!   16 bits — garbage / reserved
//!    6 bits — fieldSpecifier bitmask
//!   19 bits — y (signed, sign-magnitude — NOT two's complement)
//!   19 bits — x (signed)
//!   19 bits — z (signed)
//!   12 bits — heading (signed)
//!   [optional, in this order if the corresponding mask bit is set]
//!     0x01  → 12 bits pitch (read but unused by daemon)
//!     0x02  → 10 bits deltaHeading (signed)
//!     0x04  → 10 bits velocity / animation (signed)
//!     0x08  → 13 bits deltaY (signed)
//!     0x10  → 13 bits deltaX (signed)
//!     0x20  → 13 bits deltaZ (signed)
//! ```
//!
//! The daemon shifts y/x/z right by 3, and the deltas right by 2,
//! both for fixed-point conversion. We mirror those shifts on the
//! way out so the surfaced fields drop straight into `updateSpawn`.

use thiserror::Error;

const MASK_PITCH: u8 = 0x01;
const MASK_DELTA_HEADING: u8 = 0x02;
const MASK_ANIMATION: u8 = 0x04;
const MASK_DELTA_Y: u8 = 0x08;
const MASK_DELTA_X: u8 = 0x10;
const MASK_DELTA_Z: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcMoveUpdate {
    pub spawn_id: u16,
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub heading: i16,
    pub delta_x: i16,
    pub delta_y: i16,
    pub delta_z: i16,
    pub delta_heading: i8,
    pub animation: i16,
    pub has_delta_x: bool,
    pub has_delta_y: bool,
    pub has_delta_z: bool,
    pub has_delta_heading: bool,
    pub has_animation: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NpcMoveUpdateError {
    #[error("expected 13..=24 bytes, got {0}")]
    BadLength(usize),
    #[error("bit stream exhausted after {0} bits (payload too short for fieldSpecifier)")]
    Truncated(usize),
}

/// MSB-first bit reader matching the daemon's `BitStream`
/// implementation in `netstream.cpp`.
struct BitStream<'a> {
    data: &'a [u8],
    cur: usize,   // bit index of next bit to read
    total: usize, // bit length of the buffer
}

impl<'a> BitStream<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            cur: 0,
            total: data.len() * 8,
        }
    }

    /// Mirrors `BitStream::readUInt`: returns 0 on under-read so the
    /// caller's logic doesn't crash on malformed packets — same as
    /// the C++ daemon. Callers that care about the truncation surface
    /// it via [`Self::cur`]/[`Self::total`].
    fn read_uint(&mut self, bit_count: usize) -> u32 {
        if self.cur + bit_count > self.total {
            return 0;
        }
        let mut byte_idx = self.cur >> 3;
        let mut out: u32 = 0;

        let lead_partial = if self.cur % 8 == 0 {
            0
        } else {
            8 - (self.cur % 8)
        };

        if lead_partial > bit_count {
            // All bits live in the partial lead byte.
            let raw = (self.data[byte_idx] as u32) & ((1u32 << lead_partial) - 1);
            self.cur += bit_count;
            return raw >> (lead_partial - bit_count);
        }

        let middle = (bit_count - lead_partial) / 8;
        let tail_partial = bit_count - lead_partial - middle * 8;

        if lead_partial > 0 {
            out |= (self.data[byte_idx] as u32) & ((1u32 << lead_partial) - 1);
            byte_idx += 1;
        }

        for _ in 0..middle {
            out = (out << 8) | (self.data[byte_idx] as u32);
            byte_idx += 1;
        }

        if tail_partial > 0 {
            out = (out << tail_partial) | ((self.data[byte_idx] as u32) >> (8 - tail_partial));
        }

        self.cur += bit_count;
        out
    }

    /// Mirrors `BitStream::readInt`: 1 sign bit + (bit_count - 1)
    /// magnitude bits (sign-magnitude, not two's complement).
    fn read_int(&mut self, bit_count: usize) -> i32 {
        let sign = self.read_uint(1);
        let mag = self.read_uint(bit_count - 1) as i32;
        if sign != 0 {
            -mag
        } else {
            mag
        }
    }
}

pub fn parse_npc_move_update(bytes: &[u8]) -> Result<NpcMoveUpdate, NpcMoveUpdateError> {
    if bytes.len() < 13 || bytes.len() > 24 {
        return Err(NpcMoveUpdateError::BadLength(bytes.len()));
    }
    let mut s = BitStream::new(bytes);

    let spawn_id = s.read_uint(16) as u16;
    let _garbage = s.read_uint(16);
    let field_specifier = s.read_uint(6) as u8;

    let y = (s.read_int(19) >> 3) as i16;
    let x = (s.read_int(19) >> 3) as i16;
    let z = (s.read_int(19) >> 3) as i16;
    let heading = s.read_int(12) as i16;

    let mut delta_x: i16 = 0;
    let mut delta_y: i16 = 0;
    let mut delta_z: i16 = 0;
    let mut delta_heading: i8 = 0;
    let mut animation: i16 = 0;

    if field_specifier & MASK_PITCH != 0 {
        let _pitch = s.read_int(12);
    }
    if field_specifier & MASK_DELTA_HEADING != 0 {
        delta_heading = (s.read_int(10) >> 2) as i8;
    }
    if field_specifier & MASK_ANIMATION != 0 {
        animation = (s.read_int(10) >> 2) as i16;
    }
    if field_specifier & MASK_DELTA_Y != 0 {
        delta_y = (s.read_int(13) >> 2) as i16;
    }
    if field_specifier & MASK_DELTA_X != 0 {
        delta_x = (s.read_int(13) >> 2) as i16;
    }
    if field_specifier & MASK_DELTA_Z != 0 {
        delta_z = (s.read_int(13) >> 2) as i16;
    }

    if s.cur > s.total {
        return Err(NpcMoveUpdateError::Truncated(s.cur));
    }

    Ok(NpcMoveUpdate {
        spawn_id,
        x,
        y,
        z,
        heading,
        delta_x,
        delta_y,
        delta_z,
        delta_heading,
        animation,
        has_delta_x: field_specifier & MASK_DELTA_X != 0,
        has_delta_y: field_specifier & MASK_DELTA_Y != 0,
        has_delta_z: field_specifier & MASK_DELTA_Z != 0,
        has_delta_heading: field_specifier & MASK_DELTA_HEADING != 0,
        has_animation: field_specifier & MASK_ANIMATION != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_length() {
        assert!(parse_npc_move_update(&[0; 12]).is_err());
        assert!(parse_npc_move_update(&[0; 25]).is_err());
    }

    #[test]
    fn read_uint_byte_aligned() {
        // 0x12 0x34 → readUInt(16) should be 0x1234 (big-endian).
        let mut s = BitStream::new(&[0x12, 0x34]);
        assert_eq!(s.read_uint(16), 0x1234);
    }

    #[test]
    fn read_uint_sub_byte() {
        // 0xF0 → readUInt(4) should be 0x0F (top nibble first).
        let mut s = BitStream::new(&[0xF0]);
        assert_eq!(s.read_uint(4), 0xF);
        assert_eq!(s.read_uint(4), 0x0);
    }

    #[test]
    fn read_int_sign_magnitude() {
        // sign=1, mag=5 in 4 bits → readInt(4) = -5
        // bits: 1 1 0 1 → 0b1101 → 0xD
        let mut s = BitStream::new(&[0xD0]);
        assert_eq!(s.read_int(4), -5);
    }

    #[test]
    fn parses_minimum_packet_no_optional_fields() {
        // Build 13 bytes: spawnId=0x4321, garbage=0, fs=0,
        // y=0, x=0, z=0, heading=0.
        // Total bits = 16+16+6+19+19+19+12 = 107 bits → 13.375 → 14 bytes
        // Hmm, 107/8 = 13.375 → 14 bytes minimum. Let me recompute:
        // 16+16+6+19+19+19+12 = 107. ceil(107/8) = 14.
        let mut buf = [0u8; 14];
        buf[0] = 0x43;
        buf[1] = 0x21;
        let r = parse_npc_move_update(&buf).unwrap();
        assert_eq!(r.spawn_id, 0x4321);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        assert_eq!(r.z, 0);
        assert_eq!(r.heading, 0);
        assert_eq!(r.delta_x, 0);
        assert!(!r.has_delta_x);
        assert!(!r.has_delta_y);
        assert!(!r.has_delta_z);
        assert!(!r.has_delta_heading);
        assert!(!r.has_animation);
    }
}
