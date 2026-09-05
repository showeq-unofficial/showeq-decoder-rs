//! Parser for `OP_GroundSpawn` — variable-length payload built by
//! the modern client around a single dropped item. Mirrors
//! `SpawnShell::newGroundItem`'s NetStream-style read order:
//! dropId u32, NUL-terminated idFile text, three skipped u32s,
//! heading f32, three skipped u32s, then y/x/z f32 triplet.
//!
//! Total fixed size: 44 bytes around the variable text. The text
//! field corresponds to the legacy `makeDropStruct.idFile[30]` actor-id string.
//! The parser keeps the full wire value; a legacy host projector owns any
//! truncation needed for its fixed buffer.

use thiserror::Error;

pub const ID_FILE_LEN: usize = 30;
const FIXED_AROUND_TEXT: usize = 4 + 4 * 3 + 4 + 4 * 3 + 4 * 3; // 44

#[derive(Debug, Clone)]
pub struct GroundSpawn {
    pub drop_id: u32,
    /// Full NUL-terminated idFile text from the payload.
    pub id_file: String,
    pub heading: f32,
    pub y: f32,
    pub x: f32,
    pub z: f32,
    /// Bytes consumed from the input. The C++ NetStream stops after
    /// reading the trailing z float; surplus payload is ignored.
    pub bytes_consumed: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GroundSpawnError {
    #[error("payload truncated at {0}, need at least {1} more bytes")]
    Truncated(usize, usize),
    #[error("idFile not NUL-terminated within payload")]
    UnterminatedText,
}

fn read_u32_le(bytes: &[u8], at: usize) -> Result<u32, GroundSpawnError> {
    bytes
        .get(at..at + 4)
        .ok_or(GroundSpawnError::Truncated(at, 4))
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_f32_le(bytes: &[u8], at: usize) -> Result<f32, GroundSpawnError> {
    read_u32_le(bytes, at).map(f32::from_bits)
}

pub fn parse_ground_spawn(bytes: &[u8]) -> Result<GroundSpawn, GroundSpawnError> {
    if bytes.len() < 4 {
        return Err(GroundSpawnError::Truncated(bytes.len(), 4));
    }
    let drop_id = read_u32_le(bytes, 0)?;

    // Variable-length NUL-terminated text starting at offset 4.
    let text_start = 4usize;
    let mut p = text_start;
    while p < bytes.len() && bytes[p] != 0 {
        p += 1;
    }
    if p >= bytes.len() {
        return Err(GroundSpawnError::UnterminatedText);
    }
    let text_len = p - text_start;
    let text_end = p + 1; // skip the NUL

    let id_file = String::from_utf8_lossy(&bytes[text_start..text_start + text_len]).into_owned();

    // After the text, the netstream layout is:
    //   3× u32 skip (zoneId, zoneInstance, unknown)
    //   f32 heading
    //   3× u32 skip
    //   f32 y, f32 x, f32 z
    let after_text = text_end;
    let need = FIXED_AROUND_TEXT - 4;
    if bytes.len() < after_text + need {
        return Err(GroundSpawnError::Truncated(bytes.len(), need));
    }

    let heading = read_f32_le(bytes, after_text + 4 * 3)?;
    let y = read_f32_le(bytes, after_text + 4 * 3 + 4 + 4 * 3)?;
    let x = read_f32_le(bytes, after_text + 4 * 3 + 4 + 4 * 3 + 4)?;
    let z = read_f32_le(bytes, after_text + 4 * 3 + 4 + 4 * 3 + 4 + 4)?;

    let bytes_consumed = (after_text + need) as u32;
    Ok(GroundSpawn {
        drop_id,
        id_file,
        heading,
        y,
        x,
        z,
        bytes_consumed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_payload() {
        assert!(parse_ground_spawn(&[0; 3]).is_err());
    }

    #[test]
    fn rejects_unterminated_text() {
        let mut buf = vec![0u8; 4];
        buf.extend_from_slice(b"NoNullEver"); // no NUL byte
        assert!(matches!(
            parse_ground_spawn(&buf),
            Err(GroundSpawnError::UnterminatedText)
        ));
    }

    #[test]
    fn parses_fields() {
        // Build a payload: dropId=42, idFile="IT63_ACTORDEF",
        // skip 3×u32, heading=90.0, skip 3×u32, y=1.0, x=2.0, z=3.0.
        let mut buf = Vec::new();
        buf.extend_from_slice(&42u32.to_le_bytes());
        buf.extend_from_slice(b"IT63_ACTORDEF");
        buf.push(0); // NUL terminator
                     // 3 u32 placeholders for zoneId / zoneInstance / unknown
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(&90.0f32.to_le_bytes());
        // 3 u32 unknowns
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(&1.0f32.to_le_bytes());
        buf.extend_from_slice(&2.0f32.to_le_bytes());
        buf.extend_from_slice(&3.0f32.to_le_bytes());

        let g = parse_ground_spawn(&buf).unwrap();
        assert_eq!(g.drop_id, 42);
        assert_eq!(g.id_file, "IT63_ACTORDEF");
        assert_eq!(g.heading, 90.0);
        assert_eq!(g.y, 1.0);
        assert_eq!(g.x, 2.0);
        assert_eq!(g.z, 3.0);
        assert_eq!(g.bytes_consumed, buf.len() as u32);
    }

    #[test]
    fn preserves_long_id_file_for_host_projection() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u32.to_le_bytes());
        // 35-char text — longer than ID_FILE_LEN(30).
        let long_name = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ_xxxxx_yz";
        assert!(long_name.len() > ID_FILE_LEN);
        buf.extend_from_slice(long_name);
        buf.push(0);
        buf.extend_from_slice(&[0u8; FIXED_AROUND_TEXT - 4]);
        let g = parse_ground_spawn(&buf).unwrap();
        assert_eq!(g.id_file.as_bytes(), long_name);
    }
}
