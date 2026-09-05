//! Parser for `OP_SpecialMesg` — variable-length payload, two
//! embedded NUL-terminated strings separated by a 12-byte padding
//! block.
//!
//! Wire layout:
//!   offset 0  : unknown0000 [u8; 3]
//!   offset 3  : messageColor u32
//!   offset 7  : target u16
//!   offset 9  : padding u16
//!   offset 11 : source — NUL-terminated string
//!   then     : unknown0xxx [u32; 3] (12 bytes)
//!   then     : message — NUL-terminated string
//!
//! Daemon reads source as the speaker's name and message as the body.
//! Both are surfaced here as owned `String`s.

use thiserror::Error;

const HEADER_LEN: usize = 11;
const MID_PADDING: usize = 12; // [u32; 3]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialMessage {
    pub message_color: u32,
    pub target: u16,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SpecialMessageError {
    #[error("payload truncated at {0}, need at least {1} more bytes")]
    Truncated(usize, usize),
    #[error("source string not NUL-terminated within payload")]
    SourceUnterminated,
    #[error("message string not NUL-terminated within payload")]
    MessageUnterminated,
}

fn find_nul(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&b| b == 0)
}

pub fn parse_special_message(bytes: &[u8]) -> Result<SpecialMessage, SpecialMessageError> {
    if bytes.len() < HEADER_LEN {
        return Err(SpecialMessageError::Truncated(bytes.len(), HEADER_LEN));
    }
    let message_color = u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
    let target = u16::from_le_bytes([bytes[7], bytes[8]]);

    let source_start = HEADER_LEN;
    let source_end = source_start
        + find_nul(&bytes[source_start..]).ok_or(SpecialMessageError::SourceUnterminated)?;
    let source = bytes[source_start..source_end]
        .iter()
        .map(|&byte| char::from(byte))
        .collect();

    let message_start = source_end + 1 + MID_PADDING;
    if bytes.len() < message_start {
        return Err(SpecialMessageError::Truncated(
            bytes.len(),
            message_start - bytes.len(),
        ));
    }
    let message_end = message_start
        + find_nul(&bytes[message_start..]).ok_or(SpecialMessageError::MessageUnterminated)?;
    let message = bytes[message_start..message_end]
        .iter()
        .map(|&byte| char::from(byte))
        .collect();

    Ok(SpecialMessage {
        message_color,
        target,
        source,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(source: &[u8], message: &[u8], color: u32, target: u16) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_LEN];
        buf[3..7].copy_from_slice(&color.to_le_bytes());
        buf[7..9].copy_from_slice(&target.to_le_bytes());
        buf.extend_from_slice(source);
        buf.push(0);
        buf.extend(std::iter::repeat(0).take(MID_PADDING));
        buf.extend_from_slice(message);
        buf.push(0);
        buf
    }

    #[test]
    fn rejects_short_header() {
        assert!(parse_special_message(&[0; 10]).is_err());
    }

    #[test]
    fn parses_fields() {
        let buf = build(b"Soandso", b"hello world", 0x05, 42);
        let m = parse_special_message(&buf).unwrap();
        assert_eq!(m.message_color, 0x05);
        assert_eq!(m.target, 42);
        assert_eq!(m.source, "Soandso");
        assert_eq!(m.message, "hello world");
    }

    #[test]
    fn empty_source_and_message() {
        let buf = build(b"", b"", 0, 0);
        let m = parse_special_message(&buf).unwrap();
        assert_eq!(m.source, "");
        assert_eq!(m.message, "");
    }
}
