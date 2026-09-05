//! Parser for `OP_FormattedMessage` (2026-07-14 rotation).
//!
//! The 07/14 patch rotated the id (3c0a -> 15d0) AND changed the layout to the
//! stock length-prefixed form (the old 3c0a decoder read a flat spellId@0/
//! formatId@9/caret@13 blob that no longer exists). Verified against the fight
//! capture + resolved through the EQL client's `eqstr_us.txt`:
//!
//! ```text
//!   u32 @0    always 0
//!   u8  @4    always 0
//!   u32 @5    formatId   — eqstr_us.txt format-string id
//!   u32 @9    msgType    — message type / chat colour
//!   @13       args       — length-prefixed [u32 len][len bytes] slots; unused
//!                          trailing slots carry len=0 (packet size lands exact)
//! ```
//!
//! The arg blob is the SAME length-prefixed form `EQStr::formatMessage` consumes
//! on Live, so `%N` interpolation stays daemon-side (EQStr owns the string DB).
//! Proven: fmt 9072 "%1 has taken %2 damage from your %3.%4" + ["Lady Vox","197",
//! "Blood of Pain"] = "Lady Vox has taken 197 damage from your Blood of Pain.";
//! fmt 447 "You have gained a level! Welcome to level 48!"; fmt 138 exp; etc.

use thiserror::Error;

/// Fixed header length; the length-prefixed arg blob starts here.
pub const HEADER_LEN: usize = 13;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedMessage {
    /// eqstr format-string id at @5.
    pub format_id: u32,
    /// Message type / chat colour at @9.
    pub msg_color: u32,
    /// Positional substitution args (%1..%N), in order, empty slots dropped and
    /// EQ `\x12`-wrapped links reduced to their readable name.
    pub args: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FormattedMessageError {
    #[error("expected at least {HEADER_LEN} bytes, got {0}")]
    BadLength(usize),
}

pub fn parse_formatted_message(bytes: &[u8]) -> Result<FormattedMessage, FormattedMessageError> {
    if bytes.len() < HEADER_LEN {
        return Err(FormattedMessageError::BadLength(bytes.len()));
    }
    let format_id = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
    let msg_color = u32::from_le_bytes(bytes[9..13].try_into().unwrap());
    let args = split_args(&bytes[HEADER_LEN..]);
    Ok(FormattedMessage {
        format_id,
        msg_color,
        args,
    })
}

/// Split the length-prefixed arg blob (`[u32 len][len bytes]`…) into positional
/// args, dropping empty (len=0) slots exactly as `EQStr::formatMessage` does, so
/// `%N` alignment matches the client. Links are cleaned to a readable name.
fn split_args(blob: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= blob.len() {
        let len = u32::from_le_bytes(blob[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if len == 0 {
            continue; // unused trailing slot
        }
        if pos + len > blob.len() {
            break; // truncated / corrupt
        }
        let text: String = blob[pos..pos + len]
            .iter()
            .map(|&byte| char::from(byte))
            .collect();
        out.push(crate::links::clean_links(&text));
        pos += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a 13-byte header (fmt@5, color@9) + a length-prefixed arg blob.
    fn pkt(fmt: u32, color: u32, args: &[&[u8]]) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b[5..9].copy_from_slice(&fmt.to_le_bytes());
        b[9..13].copy_from_slice(&color.to_le_bytes());
        for a in args {
            b.extend_from_slice(&(a.len() as u32).to_le_bytes());
            b.extend_from_slice(a);
        }
        b
    }

    #[test]
    fn rejects_short_payload() {
        assert_eq!(
            parse_formatted_message(&[0u8; 12]),
            Err(FormattedMessageError::BadLength(12))
        );
    }

    #[test]
    fn header_only_no_args() {
        // fmt 15603 "You receive no experience…" — all-empty arg slots.
        let m = parse_formatted_message(&pkt(15603, 334, &[b"", b"", b""])).unwrap();
        assert_eq!(m.format_id, 15603);
        assert_eq!(m.msg_color, 334);
        assert!(m.args.is_empty());
    }

    #[test]
    fn combat_damage_args() {
        // fmt 9072 "%1 has taken %2 damage from your %3.%4"; str[2] is a spell link.
        let link = b"\x1263^3686^0^1^'Blood of Pain\x12";
        let m = parse_formatted_message(&pkt(9072, 376, &[b"Lady Vox", b"197", link])).unwrap();
        assert_eq!(m.format_id, 9072);
        assert_eq!(m.args, vec!["Lady Vox", "197", "Blood of Pain"]);
    }

    #[test]
    fn drops_trailing_empty_slots() {
        let m = parse_formatted_message(&pkt(1, 2, &[b"a", b"b", b"", b"", b""])).unwrap();
        assert_eq!(m.args, vec!["a", "b"]);
    }
}
