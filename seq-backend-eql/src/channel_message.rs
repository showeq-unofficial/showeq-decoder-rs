//! Parser for `OP_CommonMessage` (channel chat). Wire format is
//! NetStream-style — two NUL-terminated strings, a 8-byte skip, three
//! u32s (with a u32 + u8 skip in between for unknowns), another
//! NUL-terminated string for the message body.
//!
//! Walked from `MessageShell::channelMessage`:
//!   sender   : NUL-terminated text
//!   target   : NUL-terminated text
//!   skip     : 8 bytes (unknown)
//!   language : u32 LE
//!   chan_num : u32 LE
//!   skip     : 4 bytes (unknown u32)
//!   skip     : 1 byte  (unknown u8)
//!   skill    : u32 LE (skillInLanguage)
//!   message  : NUL-terminated text

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessage {
    pub sender: String,
    pub target: String,
    pub language: u32,
    pub chan_num: u32,
    pub skill_in_language: u32,
    pub message: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChannelMessageError {
    #[error("payload truncated at {0}, need at least {1} more bytes")]
    Truncated(usize, usize),
    #[error("{0} string not NUL-terminated within payload")]
    UnterminatedText(&'static str),
}

struct R<'a> {
    bytes: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn need(&self, n: usize) -> Result<(), ChannelMessageError> {
        if self.bytes.len() < self.p + n {
            Err(ChannelMessageError::Truncated(self.bytes.len(), n))
        } else {
            Ok(())
        }
    }
    fn skip(&mut self, n: usize) -> Result<(), ChannelMessageError> {
        self.need(n)?;
        self.p += n;
        Ok(())
    }
    fn u32(&mut self) -> Result<u32, ChannelMessageError> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.bytes[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        Ok(v)
    }
    fn text(&mut self, which: &'static str) -> Result<String, ChannelMessageError> {
        let end = self.bytes[self.p..]
            .iter()
            .position(|&b| b == 0)
            .ok_or(ChannelMessageError::UnterminatedText(which))?;
        let s = self.bytes[self.p..self.p + end]
            .iter()
            .map(|&byte| char::from(byte))
            .collect();
        self.p += end + 1;
        Ok(s)
    }
}

pub fn parse_channel_message(bytes: &[u8]) -> Result<ChannelMessage, ChannelMessageError> {
    let mut r = R { bytes, p: 0 };
    let sender = r.text("sender")?;
    let target = r.text("target")?;
    r.skip(8)?; // unknown
    let language = r.u32()?;
    let chan_num = r.u32()?;
    r.skip(4)?; // unknown u32
    r.skip(1)?; // unknown u8
    let skill_in_language = r.u32()?;
    let message = crate::links::clean_links(&r.text("message")?); // players link items in chat
    Ok(ChannelMessage {
        sender,
        target,
        language,
        chan_num,
        skill_in_language,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(
        sender: &[u8],
        target: &[u8],
        lang: u32,
        chan: u32,
        skill: u32,
        message: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(sender);
        buf.push(0);
        buf.extend_from_slice(target);
        buf.push(0);
        buf.extend_from_slice(&[0u8; 8]);
        buf.extend_from_slice(&lang.to_le_bytes());
        buf.extend_from_slice(&chan.to_le_bytes());
        buf.extend_from_slice(&[0u8; 4]);
        buf.push(0);
        buf.extend_from_slice(&skill.to_le_bytes());
        buf.extend_from_slice(message);
        buf.push(0);
        buf
    }

    #[test]
    fn parses_say() {
        let buf = build(b"Soandso", b"", 0, 6, 100, b"hi there");
        let m = parse_channel_message(&buf).unwrap();
        assert_eq!(m.sender, "Soandso");
        assert_eq!(m.target, "");
        assert_eq!(m.chan_num, 6);
        assert_eq!(m.skill_in_language, 100);
        assert_eq!(m.message, "hi there");
    }

    #[test]
    fn parses_tell() {
        let buf = build(b"Alice", b"Bob", 0, 14, 0, b"private msg");
        let m = parse_channel_message(&buf).unwrap();
        assert_eq!(m.sender, "Alice");
        assert_eq!(m.target, "Bob");
        assert_eq!(m.message, "private msg");
    }

    #[test]
    fn rejects_truncated() {
        assert!(parse_channel_message(b"Alice").is_err());
    }
}
