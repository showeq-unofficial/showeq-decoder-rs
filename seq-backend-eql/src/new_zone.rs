//! Parser for `OP_NewZone`. The modern wire format is a NetStream walk
//! over three NUL-terminated text fields plus the exp multiplier and
//! safe-point coords — the legacy `newZoneStruct` layout in
//! `everquest.h` is fixed-size and no longer matches what the client
//! actually receives.
//!
//! Layout from `ZoneMgr::zoneNew`:
//!   short_name : NUL-terminated text
//!   long_name  : NUL-terminated text
//!   skip       : 2 bytes
//!   zonefile   : NUL-terminated text
//!   skip       : 90 bytes
//!   exp_mult   : f32 LE
//!   skip       : 28 bytes
//!   safe_y     : f32 LE
//!   safe_x     : f32 LE
//!   safe_z     : f32 LE

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
    /// Classic zone id. Unused by the current parsers — both Live and the eql
    /// backend name the zone via short_name/long_name (eql's OP_NewZone carries the
    /// name as text, not an id). Retained for wire/FFI compatibility.
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

/// Does this read like a zone name, or like whatever bytes happened to be there?
///
/// The walk starts at offset 0 and takes everything up to the first NUL, so a payload that
/// is not an `OP_NewZone` yields a "name" made of raw bytes — and the consumer LATCHES it:
/// zone identity is held in World and re-sent in every snapshot, so one bad decode renames
/// the zone for the rest of the session and takes the map with it. Observed live as
/// `zoneShort = "X\u{fb}f"`.
///
/// Zone names are ASCII (`airplane_eqlsolo`, `The Plane of Sky`), so the test is deliberately
/// blunt: printable ASCII only, and bounded. It cannot save a wrong payload that happens to
/// contain plausible text, but it turns the common case from silent corruption into a
/// rejected packet.
pub(crate) fn plausible(name: &str, max: usize) -> bool {
    // Empty is structurally legal and harmless — a well-formed packet may carry no text,
    // and an empty zone name is not a zone RENAMED to nonsense. Only text that is present
    // and made of non-printable bytes is the failure this rejects.
    name.is_empty() || (name.len() <= max && name.bytes().all(|b| (0x20..=0x7e).contains(&b)))
}

struct R<'a> {
    bytes: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn need(&self, n: usize) -> Result<(), NewZoneError> {
        if self.bytes.len() < self.p + n {
            Err(NewZoneError::Truncated(self.bytes.len(), n))
        } else {
            Ok(())
        }
    }
    fn skip(&mut self, n: usize) -> Result<(), NewZoneError> {
        self.need(n)?;
        self.p += n;
        Ok(())
    }
    fn f32(&mut self) -> Result<f32, NewZoneError> {
        self.need(4)?;
        let v = f32::from_le_bytes(self.bytes[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        Ok(v)
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
    r.skip(2)?;
    let zonefile = r.text("zonefile")?;
    r.skip(90)?;
    let zone_exp_multiplier = r.f32()?;
    r.skip(28)?;
    let safe_y = r.f32()?;
    let safe_x = r.f32()?;
    let safe_z = r.f32()?;
    Ok(NewZone {
        short_name,
        long_name,
        zonefile,
        zone_exp_multiplier,
        safe_y,
        safe_x,
        safe_z,
        zone_id: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(
        short_name: &[u8],
        long_name: &[u8],
        zonefile: &[u8],
        exp_mult: f32,
        safe_y: f32,
        safe_x: f32,
        safe_z: f32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(short_name);
        buf.push(0);
        buf.extend_from_slice(long_name);
        buf.push(0);
        buf.extend_from_slice(&[0u8; 2]);
        buf.extend_from_slice(zonefile);
        buf.push(0);
        buf.extend_from_slice(&[0u8; 90]);
        buf.extend_from_slice(&exp_mult.to_le_bytes());
        buf.extend_from_slice(&[0u8; 28]);
        buf.extend_from_slice(&safe_y.to_le_bytes());
        buf.extend_from_slice(&safe_x.to_le_bytes());
        buf.extend_from_slice(&safe_z.to_le_bytes());
        buf
    }

    #[test]
    fn parses_fields() {
        let buf = build(
            b"ecommons",
            b"East Commonlands",
            b"ecommons",
            1.0,
            -100.0,
            200.0,
            -50.0,
        );
        let z = parse_new_zone(&buf).unwrap();
        assert_eq!(z.short_name, "ecommons");
        assert_eq!(z.long_name, "East Commonlands");
        assert_eq!(z.zonefile, "ecommons");
        assert_eq!(z.zone_exp_multiplier, 1.0);
        assert_eq!(z.safe_y, -100.0);
        assert_eq!(z.safe_x, 200.0);
        assert_eq!(z.safe_z, -50.0);
    }

    #[test]
    fn rejects_truncated() {
        assert!(parse_new_zone(b"short\0long\0").is_err());
    }

    #[test]
    fn empty_strings_are_legal() {
        let buf = build(b"", b"", b"", 0.0, 0.0, 0.0, 0.0);
        let z = parse_new_zone(&buf).unwrap();
        assert_eq!(z.short_name, "");
        assert_eq!(z.long_name, "");
        assert_eq!(z.zonefile, "");
    }
}

#[cfg(test)]
mod reject_tests {
    use super::*;

    /// The live regression: a non-OP_NewZone payload walked as one produced
    /// `zoneShort = "X\u{fb}f"`, which World then latched into every snapshot.
    #[test]
    fn rejects_a_short_name_of_raw_bytes() {
        let mut buf = vec![b'X', 0xfb, b'f', 0];
        buf.extend_from_slice(b"garbage\0");
        buf.extend_from_slice(&[0u8; 200]);
        assert_eq!(
            parse_new_zone(&buf),
            Err(NewZoneError::ImplausibleName("short_name"))
        );
    }

    /// A garbage LONG name is rejected too — it is what the zone is displayed as.
    #[test]
    fn rejects_a_long_name_of_raw_bytes() {
        let mut buf = Vec::from(&b"airplane\0"[..]);
        buf.extend_from_slice(&[0x12, 0x21, 0xdd, 0x71, 0]);
        buf.extend_from_slice(&[0u8; 200]);
        assert_eq!(
            parse_new_zone(&buf),
            Err(NewZoneError::ImplausibleName("long_name"))
        );
    }
}
