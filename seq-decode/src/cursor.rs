//! Sequential little-endian reader for variable-length wire payloads.
//!
//! Mirrors the surface of the daemon's `NetStream` (see
//! `scry-cpp/src/netstream.{h,cpp}`) for the operations we
//! actually use in spawn parsing: u8 / u32 reads, fixed-size skips,
//! null-terminated strings. EQ wire format is little-endian on the
//! tested targets — `NetStream::readUInt32NC()` is a direct memcpy on
//! LE hosts (`packetcommon.h:123`), so we mirror that.
//!
//! Out-of-bounds reads return `Err(CursorError::Eof)` rather than
//! NetStream's "silently return 0 / advance to end" behavior — we
//! prefer explicit failure so the daemon can fall back to the C++
//! path on any anomaly. SZC_None dispatch means the daemon can't
//! pre-validate length the way SZC_Match does.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CursorError {
    #[error("read past end of buffer at offset {offset}, wanted {wanted} more bytes, have {have}")]
    Eof {
        offset: usize,
        wanted: usize,
        have: usize,
    },
}

#[derive(Clone, Copy)]
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    pub fn at_end(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn need(&self, n: usize) -> Result<(), CursorError> {
        if self.remaining() < n {
            return Err(CursorError::Eof {
                offset: self.pos,
                wanted: n,
                have: self.remaining(),
            });
        }
        Ok(())
    }

    pub fn skip(&mut self, n: usize) -> Result<(), CursorError> {
        self.need(n)?;
        self.pos += n;
        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8, CursorError> {
        self.need(1)?;
        let v = self.buf[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn read_u32_le(&mut self) -> Result<u32, CursorError> {
        self.need(4)?;
        let v = u32::from_le_bytes([
            self.buf[self.pos],
            self.buf[self.pos + 1],
            self.buf[self.pos + 2],
            self.buf[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    /// Reads a null-terminated string and returns a slice pointing at
    /// the bytes BEFORE the terminator. The terminator itself is
    /// consumed. Mirrors `NetStream::readText()` semantics exactly:
    /// scans forward until '\0' or end of buffer; if no terminator
    /// before end-of-buffer, the slice extends to the end and the
    /// cursor lands at end (not advanced past). The empty-string
    /// case (immediate '\0') returns an empty slice and consumes the
    /// terminator.
    pub fn read_cstr(&mut self) -> Result<&'a [u8], CursorError> {
        if self.at_end() {
            // Match NetStream's "no data left" branch — return empty.
            return Ok(&[]);
        }
        let start = self.pos;
        while self.pos < self.buf.len() && self.buf[self.pos] != 0 {
            self.pos += 1;
        }
        let slice = &self.buf[start..self.pos];
        // Skip the trailing NUL when present.
        if self.pos < self.buf.len() {
            self.pos += 1;
        }
        Ok(slice)
    }

    /// Length-prefixed text: a `u32` byte count followed by that many bytes (no
    /// terminator). Mirrors `NetStream::readLPText()`.
    pub fn read_lp_text(&mut self) -> Result<&'a [u8], CursorError> {
        let len = self.read_u32_le()? as usize;
        self.need(len)?;
        let s = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u8_round_trip() {
        let mut c = Cursor::new(&[0x12, 0x34]);
        assert_eq!(c.read_u8().unwrap(), 0x12);
        assert_eq!(c.read_u8().unwrap(), 0x34);
        assert!(c.read_u8().is_err());
    }

    #[test]
    fn read_u32_le_byte_order() {
        let mut c = Cursor::new(&[0xEF, 0xBE, 0xAD, 0xDE]);
        assert_eq!(c.read_u32_le().unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn skip_advances_position() {
        let mut c = Cursor::new(&[0; 10]);
        c.skip(3).unwrap();
        assert_eq!(c.pos(), 3);
        assert_eq!(c.remaining(), 7);
    }

    #[test]
    fn skip_past_end_errors() {
        let mut c = Cursor::new(&[0; 4]);
        assert!(c.skip(5).is_err());
        // Cursor must not have advanced on error.
        assert_eq!(c.pos(), 0);
    }

    #[test]
    fn read_cstr_consumes_terminator() {
        let buf = b"hello\0world\0";
        let mut c = Cursor::new(buf);
        assert_eq!(c.read_cstr().unwrap(), b"hello");
        assert_eq!(c.read_cstr().unwrap(), b"world");
        assert!(c.at_end());
    }

    #[test]
    fn read_cstr_empty_string() {
        let mut c = Cursor::new(b"\0tail");
        assert_eq!(c.read_cstr().unwrap(), b"");
        assert_eq!(c.read_cstr().unwrap(), b"tail");
    }

    #[test]
    fn read_cstr_unterminated_returns_remainder() {
        // Mirrors NetStream's behavior when no NUL is found before end.
        let mut c = Cursor::new(b"abc");
        assert_eq!(c.read_cstr().unwrap(), b"abc");
        assert!(c.at_end());
    }
}
