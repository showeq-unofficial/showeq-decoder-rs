//! Parser for EQ Legends `OP_NewZone`: three NUL-terminated names (short, long, zone
//! file) followed by a fixed 306-byte environment tail. Derived from the 08/25 wire;
//! Live's `newZoneStruct` walk does not apply here.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewZone {
    pub short_name: String,
    pub long_name: String,
    pub zonefile: String,
    pub zone_exp_multiplier: f32,
    pub safe_y: f32,
    pub safe_x: f32,
    pub safe_z: f32,
    /// Classic zone id (fearplane 72, guktop 65, gukbottom 66 on the 08/25 wire).
    pub zone_id: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NewZoneError {
    #[error("payload truncated at {0}, need at least {1} more bytes")]
    Truncated(usize, usize),
    #[error("{0} not NUL-terminated within payload")]
    UnterminatedText(&'static str),
    #[error("{0} is not a plausible zone name")]
    ImplausibleName(&'static str),
}

/// Bytes between the long name's terminator and the zone file.
pub const NAME_GAP: usize = 3;
/// Fixed environment block after the zone file's terminator.
pub const TAIL_LEN: usize = 306;
const ZONE_ID: usize = 5;
const EXP_MULT: usize = 9;
const SAFE_Y: usize = 123;

/// Empty is legal; text that is present must be printable ASCII and bounded.
pub(crate) fn plausible(name: &str, max: usize) -> bool {
    name.is_empty() || (name.len() <= max && name.bytes().all(|b| (0x20..=0x7e).contains(&b)))
}

struct R<'a> {
    bytes: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn need(&self, n: usize) -> Result<(), NewZoneError> {
        if self.bytes.len() < self.p + n {
            Err(NewZoneError::Truncated(
                self.bytes.len(),
                self.p + n - self.bytes.len(),
            ))
        } else {
            Ok(())
        }
    }
    fn skip(&mut self, n: usize) -> Result<(), NewZoneError> {
        self.need(n)?;
        self.p += n;
        Ok(())
    }
    fn text(&mut self, which: &'static str) -> Result<String, NewZoneError> {
        let end = self.bytes[self.p..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(NewZoneError::UnterminatedText(which))?;
        let s = String::from_utf8_lossy(&self.bytes[self.p..self.p + end]).into_owned();
        self.p += end + 1;
        Ok(s)
    }
    fn tail(&self) -> Result<&'a [u8], NewZoneError> {
        self.need(TAIL_LEN)?;
        Ok(&self.bytes[self.p..])
    }
}

fn f32_at(tail: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(tail[at..at + 4].try_into().unwrap())
}

pub fn parse_new_zone(bytes: &[u8]) -> Result<NewZone, NewZoneError> {
    let mut r = R { bytes, p: 0 };
    let short_name = r.text("short_name")?;
    let long_name = r.text("long_name")?;
    if !plausible(&short_name, 64) {
        return Err(NewZoneError::ImplausibleName("short_name"));
    }
    if !plausible(&long_name, 128) {
        return Err(NewZoneError::ImplausibleName("long_name"));
    }
    r.skip(NAME_GAP)?;
    let zonefile = r.text("zonefile")?;
    if !plausible(&zonefile, 128) {
        return Err(NewZoneError::ImplausibleName("zonefile"));
    }
    let tail = r.tail()?;
    Ok(NewZone {
        short_name,
        long_name,
        zonefile,
        zone_exp_multiplier: f32_at(tail, EXP_MULT),
        safe_y: f32_at(tail, SAFE_Y),
        safe_x: f32_at(tail, SAFE_Y + 4),
        safe_z: f32_at(tail, SAFE_Y + 8),
        zone_id: u32::from_le_bytes(tail[ZONE_ID..ZONE_ID + 4].try_into().unwrap()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn build(
        short: &[u8],
        long: &[u8],
        zonefile: &[u8],
        zone_id: u32,
        exp_mult: f32,
        safe_y: f32,
        safe_x: f32,
        safe_z: f32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(short);
        buf.push(0);
        buf.extend_from_slice(long);
        buf.push(0);
        buf.extend_from_slice(&[0u8; NAME_GAP]);
        buf.extend_from_slice(zonefile);
        buf.push(0);
        let mut tail = [0u8; TAIL_LEN];
        tail[ZONE_ID..ZONE_ID + 4].copy_from_slice(&zone_id.to_le_bytes());
        tail[EXP_MULT..EXP_MULT + 4].copy_from_slice(&exp_mult.to_le_bytes());
        tail[SAFE_Y..SAFE_Y + 4].copy_from_slice(&safe_y.to_le_bytes());
        tail[SAFE_Y + 4..SAFE_Y + 8].copy_from_slice(&safe_x.to_le_bytes());
        tail[SAFE_Y + 8..SAFE_Y + 12].copy_from_slice(&safe_z.to_le_bytes());
        buf.extend_from_slice(&tail);
        buf
    }

    #[test]
    fn parses_fields() {
        let buf = build(
            b"guktop",
            b"The City of Guk",
            b"guktop",
            65,
            1.0,
            -36.0,
            7.0,
            4.0,
        );
        assert_eq!(buf.len(), 339);
        let z = parse_new_zone(&buf).unwrap();
        assert_eq!(z.short_name, "guktop");
        assert_eq!(z.long_name, "The City of Guk");
        assert_eq!(z.zonefile, "guktop");
        assert_eq!(z.zone_id, 65);
        assert_eq!(z.zone_exp_multiplier, 1.0);
        assert_eq!((z.safe_y, z.safe_x, z.safe_z), (-36.0, 7.0, 4.0));
    }

    #[test]
    fn wire_shape_has_three_nuls_before_the_zone_file() {
        let buf = build(
            b"fearplane",
            b"The Plane of Fear",
            b"fearplane",
            72,
            1.0,
            0.0,
            0.0,
            0.0,
        );
        assert_eq!(&buf[27..32], b"\0\0\0\0f");
        assert_eq!(parse_new_zone(&buf).unwrap().zone_id, 72);
    }

    #[test]
    fn rejects_truncated() {
        assert!(parse_new_zone(b"short\0long\0").is_err());
        let mut buf = build(b"qeynos", b"South Qeynos", b"qeynos", 1, 1.0, 0.0, 0.0, 0.0);
        buf.pop();
        assert!(matches!(
            parse_new_zone(&buf),
            Err(NewZoneError::Truncated(_, 1))
        ));
    }

    #[test]
    fn rejects_names_of_raw_bytes() {
        let mut buf = vec![0x5c, 0xfa, 0x27, 0x1b, 0x00];
        buf.extend_from_slice(b"The Plane of Hate\0");
        assert_eq!(
            parse_new_zone(&buf),
            Err(NewZoneError::ImplausibleName("short_name"))
        );
    }

    #[test]
    fn empty_strings_are_legal() {
        let buf = build(b"", b"", b"", 0, 0.0, 0.0, 0.0, 0.0);
        let z = parse_new_zone(&buf).unwrap();
        assert_eq!(
            (
                z.short_name.as_str(),
                z.long_name.as_str(),
                z.zonefile.as_str()
            ),
            ("", "", "")
        );
    }
}
