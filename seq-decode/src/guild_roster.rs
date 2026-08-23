//! Parser for `OP_GuildMemberList` — the full guild roster (Live/Test).
//!
//! Variable-length NetStream walk, ported 1:1 from legacy showeq's
//! `guildshell.cpp` (`upstream/master`, the current-Live reference — byte-for-byte
//! the same as the daemon's C++ copy). Live's own; eql keeps a separate parser in
//! `seq-backend-eql` (its wire diverges — a wider header + a multiclass mask +
//! a trailing zone id, none of which Live has).
//!
//! ```text
//! header  LPText requesterName, skip 4, skip 4, skip 2, u32 count
//! member  LPText name, u32 level, u32 banker, u32 class, u32 rank, u32 lastOn,
//!         u8 tributeOn, u8 trophyOn, u32 tributeDonated, u32 tributeLastDonation,
//!         u8 fullMember, LPText publicNote, skip 6
//! ```
//!
//! The header's pre-count field is `skip 2` on current Live — one byte wider than
//! legacy 6.4.25's `skip 1` (a byte was added in a later patch; verified against a
//! 13338-byte / 232-member live capture, which lands exactly on the payload end).
//! `count` is unreliable, so — like legacy — we IGNORE it and walk members until
//! the payload is consumed, rather than looping `count` times.
//!
//! `class` is a single class id (Live has no multiclass). `banker` packs two
//! flags: 0 none, 1 banker, 2 alt, 3 alt banker. Live does NOT carry a member's
//! zone in the roster (legacy reads none — the 6-byte tail is skipped), so there
//! is no online/offline state, unlike eql.

use crate::cursor::{Cursor, CursorError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildMemberRow {
    pub name: String,
    pub level: u32,
    /// Single class id (Live is single-class; `class_mask` is always 0).
    pub primary_class: u32,
    pub rank: u32,
    pub last_on: u32,
    pub banker: bool,
    pub alt: bool,
    pub full_member: bool,
    pub public_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildRoster {
    /// The player's own guild id, from the roster header's first u32 (legacy
    /// skips it as a patch-added field, but a live capture confirms it against
    /// the guild-in-zone list: roster header 15/180 == the OP_GuildsInZoneList
    /// entry for this guild).
    pub guild_id: u32,
    pub members: Vec<GuildMemberRow>,
    /// False when the parser reached a truncated trailing member. The complete
    /// prefix remains useful to a stateful session, but must not replace a
    /// previously complete roster.
    pub complete: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GuildRosterError {
    #[error("truncated header: {0}")]
    Truncated(#[from] CursorError),
}

fn latin1(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn read_member(c: &mut Cursor) -> Result<GuildMemberRow, CursorError> {
    let name = latin1(c.read_lp_text()?);
    let level = c.read_u32_le()?;
    let banker_flag = c.read_u32_le()?;
    let primary_class = c.read_u32_le()?;
    let rank = c.read_u32_le()?;
    let last_on = c.read_u32_le()?;
    let _tribute_on = c.read_u8()?;
    let _trophy_on = c.read_u8()?;
    let _tribute_donated = c.read_u32_le()?;
    let _tribute_last_donation = c.read_u32_le()?;
    let full_member = c.read_u8()? != 0;
    let public_note = latin1(c.read_lp_text()?);
    c.skip(6)?; // tail — legacy reads no zone/instance from it
    Ok(GuildMemberRow {
        name,
        level,
        primary_class,
        rank,
        last_on,
        banker: banker_flag % 2 != 0,
        alt: banker_flag > 1,
        full_member,
        public_note,
    })
}

pub fn parse_guild_member_list(bytes: &[u8]) -> Result<GuildRoster, GuildRosterError> {
    let mut c = Cursor::new(bytes);

    // Header: the requester's own name, three skipped fields (patch-added over
    // the years — the last is 2 bytes on current Live), then the member count.
    let _requester = c.read_lp_text()?;
    let guild_id = c.read_u32_le()?; // player's own guild id (legacy skips this)
    c.skip(4)?; // server id
    c.skip(2)?;
    let count = c.read_u32_le()? as usize; // unreliable — used only to pre-size

    // Walk members to the payload end (legacy ignores `count`). A member read
    // that runs short (truncation / trailing pad) ends the walk with what we
    // have, rather than failing the whole roster.
    let mut members = Vec::with_capacity(count.min(4096));
    while !c.at_end() {
        let mut trial = c;
        match read_member(&mut trial) {
            Ok(m) => {
                c = trial;
                members.push(m);
            }
            Err(_) => break,
        }
    }

    Ok(GuildRoster {
        guild_id,
        members,
        complete: c.at_end(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lp(buf: &mut Vec<u8>, s: &str) {
        buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
        buf.extend_from_slice(s.as_bytes());
    }

    fn member(buf: &mut Vec<u8>, name: &str, level: u32, class: u32, rank: u32, note: &str) {
        lp(buf, name);
        buf.extend_from_slice(&level.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // banker
        buf.extend_from_slice(&class.to_le_bytes());
        buf.extend_from_slice(&rank.to_le_bytes());
        buf.extend_from_slice(&0x6a5d_c783u32.to_le_bytes()); // lastOn
        buf.push(0); // tributeOn
        buf.push(0); // trophyOn
        buf.extend_from_slice(&0u32.to_le_bytes()); // tributeDonated
        buf.extend_from_slice(&0u32.to_le_bytes()); // tributeLastDonation
        buf.push(1); // fullMember
        lp(buf, note);
        buf.extend_from_slice(&[0u8; 6]); // tail
    }

    fn roster(members: &[(&str, u32, u32, u32, &str)]) -> Vec<u8> {
        let mut b = Vec::new();
        lp(&mut b, "Self");
        b.extend_from_slice(&15u32.to_le_bytes()); // guild id
        b.extend_from_slice(&180u32.to_le_bytes()); // server id
        b.extend_from_slice(&[0u8; 2]); // skip2 (current Live)
        b.extend_from_slice(&(members.len() as u32).to_le_bytes());
        for m in members {
            member(&mut b, m.0, m.1, m.2, m.3, m.4);
        }
        b
    }

    #[test]
    fn two_member_roster() {
        let b = roster(&[("Aaaa", 60, 1, 2, ""), ("Bbbbbb", 55, 3, 0, "alt of Aaaa")]);
        let r = parse_guild_member_list(&b).unwrap();
        assert_eq!(r.guild_id, 15);
        assert_eq!(r.members.len(), 2);
        assert_eq!(r.members[0].level, 60);
        assert_eq!(r.members[0].primary_class, 1);
        assert_eq!(r.members[0].rank, 2);
        assert!(r.members[0].full_member);
        assert_eq!(r.members[1].primary_class, 3);
        assert_eq!(r.members[1].public_note, "alt of Aaaa");
        assert!(r.complete);
    }

    #[test]
    fn truncated_tail_keeps_a_partial_prefix() {
        let mut b = roster(&[("Aaaa", 60, 1, 2, ""), ("Bbbbbb", 55, 3, 0, "alt")]);
        b.truncate(b.len() - 5);
        let r = parse_guild_member_list(&b).unwrap();
        assert_eq!(r.members.len(), 1);
        assert_eq!(r.members[0].name, "Aaaa");
        assert!(!r.complete);
    }

    #[test]
    fn banker_and_alt_flags() {
        let mut b = Vec::new();
        lp(&mut b, "Self");
        b.extend_from_slice(&[0u8; 10]); // skip4 + skip4 + skip2
        b.extend_from_slice(&1u32.to_le_bytes()); // count
                                                  // banker_flag = 3 -> banker + alt
        lp(&mut b, "X");
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&3u32.to_le_bytes()); // banker
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.push(0);
        b.push(0);
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.push(1);
        lp(&mut b, "");
        b.extend_from_slice(&[0u8; 6]);
        let r = parse_guild_member_list(&b).unwrap();
        assert!(r.members[0].banker);
        assert!(r.members[0].alt);
    }

    #[test]
    fn empty_roster() {
        assert!(parse_guild_member_list(&roster(&[]))
            .unwrap()
            .members
            .is_empty());
    }

    #[test]
    fn trailing_padding_is_tolerated() {
        // A short trailing pad after the last member is ignored (the walk ends
        // when a member read runs short), not treated as an error.
        let mut b = roster(&[("Aaaa", 60, 1, 0, "")]);
        b.extend_from_slice(&[0u8; 4]);
        assert_eq!(parse_guild_member_list(&b).unwrap().members.len(), 1);
    }

    #[test]
    fn truncated_member_is_dropped_not_fatal() {
        // A truncated final member ends the walk with the members read so far,
        // rather than failing the whole roster.
        let b = roster(&[("Aaaa", 60, 1, 0, ""), ("Bbbb", 55, 2, 1, "")]);
        let full = parse_guild_member_list(&b).unwrap().members.len();
        assert_eq!(full, 2);
        let mut cut = b.clone();
        cut.truncate(cut.len() - 4); // chop the second member's tail
        assert_eq!(parse_guild_member_list(&cut).unwrap().members.len(), 1);
    }
}
