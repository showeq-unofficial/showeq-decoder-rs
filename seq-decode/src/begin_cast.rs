//! Parser for the Live/Test 15-byte `OP_BeginCast` broadcast.

use thiserror::Error;

pub const PAYLOAD_LEN: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginCast {
    pub caster_id: u32,
    pub spell_id: u32,
    pub cast_time_ms: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BeginCastError {
    #[error("expected {PAYLOAD_LEN} bytes, got {0}")]
    BadLength(usize),
}

pub fn parse_begin_cast(bytes: &[u8]) -> Result<BeginCast, BeginCastError> {
    if bytes.len() != PAYLOAD_LEN {
        return Err(BeginCastError::BadLength(bytes.len()));
    }
    Ok(BeginCast {
        spell_id: u32::from_le_bytes(bytes[0..4].try_into().expect("four bytes")),
        caster_id: u32::from(u16::from_le_bytes(
            bytes[4..6].try_into().expect("two bytes"),
        )),
        cast_time_ms: u32::from(u16::from_le_bytes(
            bytes[6..8].try_into().expect("two bytes"),
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fields_and_rejects_partial_records() {
        let mut payload = [0; PAYLOAD_LEN];
        payload[..4].copy_from_slice(&74_023u32.to_le_bytes());
        payload[4..6].copy_from_slice(&321u16.to_le_bytes());
        payload[6..8].copy_from_slice(&4_500u16.to_le_bytes());
        assert_eq!(
            parse_begin_cast(&payload).unwrap(),
            BeginCast {
                caster_id: 321,
                spell_id: 74_023,
                cast_time_ms: 4_500,
            }
        );
        assert!(parse_begin_cast(&payload[..14]).is_err());
    }
}
