//! EQ Legends UCS (Universal Chat Service) cross-zone / global chat decoder.
//!
//! UCS chat rides a separate SOE "chat" session (UDP 9877) obfuscated with
//! SOE's keyless self-synchronizing XOR: `plain[i] = cipher[i] ^ cipher[i-4]`
//! for `i >= 4` (bytes 0..4 are a per-session seed-masked header we leave
//! raw). After de-obfuscation each record is NUL-delimited:
//!
//! ```text
//! [4-byte masked header]<channel>\0 <server>.<sender>\0 <message>\0 SPAM:<score>:<id>\0
//! ```
//!
//! One packet may carry several records; we anchor on the `"SPAM:"` trailer
//! and walk the NUL fields backward (ported from the legacy client's
//! `MessageShell::ucsChatMessage`). Only the inbound (server->client) side
//! carries chat — the caller gates direction.
//!
//! The channel name's first char sits at byte 4 of the channel field (the tail
//! of the masked header) and so decodes garbled. We return it raw
//! (`channel_first`) plus the clean remainder (`channel_rest`); the caller
//! recovers the per-session mask from a crib (the auto-joined `General*`
//! channel, whose clean remainder is `"eneral"`) and repairs the first char
//! for every channel — no key and no `/list` needed. See `OPCODES_LEGENDS.md`.

/// One decoded UCS chat line. `channel_first` is the still-masked first byte of
/// the channel name; `channel_rest` is the clean remainder. `spam` is set when
/// the server's per-message spam score is non-zero.
pub struct UcsRecord {
    pub channel_first: u8,
    pub channel_rest: String,
    /// The channel field's whole trailing printable run (e.g. "^eneral",
    /// "General", "WewPlayers"). For the dominant framing this equals
    /// `[channel_first][channel_rest]`; when it does NOT (a data-dependent NUL
    /// in the masked header shifted the field boundary) the caller treats it as
    /// a framing outlier and resolves it by suffix-matching learned channels.
    pub channel_run: String,
    pub sender: String,
    pub message: String,
    pub spam: bool,
}

#[inline]
fn printable(c: u8) -> bool {
    (0x20..0x7f).contains(&c) || c >= 0xa0
}

#[inline]
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn mostly_printable(b: &[u8]) -> bool {
    if b.is_empty() {
        return false;
    }
    let p = b.iter().filter(|&&c| printable(c)).count();
    p * 100 >= b.len() * 85
}

/// Start index of the NUL-delimited field ending at `end` (exclusive).
fn field_start(buf: &[u8], end: usize) -> usize {
    let mut s = end;
    while s > 0 && buf[s - 1] != 0 {
        s -= 1;
    }
    s
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Decode all UCS chat records from one server->client UDP payload.
/// Returns an empty Vec if the payload is too short or carries no chat
/// (SOE ACKs / keepalives decode to noise and produce no `SPAM:` anchor).
pub fn parse_ucs_chat(payload: &[u8]) -> Vec<UcsRecord> {
    if payload.len() < 12 {
        return Vec::new();
    }
    // Self-synchronizing XOR. Read the ORIGINAL ciphertext at i-4 (from the
    // pristine input), NOT the already-decoded byte.
    let mut buf = payload.to_vec();
    for i in 4..payload.len() {
        buf[i] = payload[i] ^ payload[i - 4];
    }

    let n = buf.len();
    let mut out = Vec::new();
    let mut scan = 0usize;
    while scan + 5 <= n {
        let p = match find(&buf[scan..], b"SPAM:") {
            Some(i) => scan + i,
            None => break,
        };
        scan = p + 5;
        if p < 2 || buf[p - 1] != 0 {
            continue; // "SPAM:" must follow a NUL
        }
        // message = field ending at the NUL just before "SPAM:".
        let msg_end = p - 1;
        let msg_start = field_start(&buf, msg_end);
        if msg_start >= msg_end || msg_start < 2 {
            continue;
        }
        // <server>.<sender> = field before the message.
        let ss_end = msg_start - 1;
        let ss_start = field_start(&buf, ss_end);
        if ss_start >= ss_end || ss_start < 2 {
            continue;
        }
        // channel field = [4-byte masked header][channel name] before that.
        let ce_end = ss_start - 1;
        let ce_start = field_start(&buf, ce_end);
        let field = &buf[ce_start..ce_end];
        if field.len() < 5 {
            continue;
        }

        let message = crate::links::clean_links(&latin1(&buf[msg_start..msg_end]));
        let server_sender = latin1(&buf[ss_start..ss_end]);
        let sender = match server_sender.rfind('.') {
            Some(d) => server_sender[d + 1..].to_string(),
            None => server_sender.clone(),
        };
        if !mostly_printable(sender.as_bytes())
            || !mostly_printable(message.as_bytes())
            || sender.len() > 32
        {
            continue;
        }

        // spam score = digits right after "SPAM:".
        let mut q = p + 5;
        let mut score: u64 = 0;
        let mut have = false;
        while q < n && buf[q].is_ascii_digit() {
            score = score.saturating_mul(10) + (buf[q] - b'0') as u64;
            have = true;
            q += 1;
        }
        let spam = have && score > 0;

        // channel_first = masked first char (byte 4); channel_rest = clean
        // remainder (byte 5 onward, up to the first non-printable for safety).
        let channel_first = field[4];
        let rest = &field[5..];
        let k = rest
            .iter()
            .position(|&c| !printable(c))
            .unwrap_or(rest.len());
        let channel_rest = latin1(&rest[..k]);

        // Whole trailing printable run of the field (name recovery for outliers).
        let mut rs = field.len();
        while rs > 0 && printable(field[rs - 1]) {
            rs -= 1;
        }
        let channel_run = latin1(&field[rs..]);

        out.push(UcsRecord {
            channel_first,
            channel_rest,
            channel_run,
            sender,
            message,
            spam,
        });
    }
    out
}

/// Learn full channel NAMES from one server->client UDP payload's un-masked
/// text — `/list` rosters ("Channels: 1=General2(400), 2=myraid(1), ...") and
/// channel join/invite notices ("channel <name>" / "joined <name>"). These
/// carry the name in the clear (unlike chat, whose first char is seed-masked),
/// so they let the caller resolve every channel — including the first chat line
/// and framing outliers — without the General crib. Ported from the legacy
/// client's `MessageShell::ucsLearnChannels`.
pub fn parse_ucs_channels(payload: &[u8]) -> Vec<String> {
    if payload.len() < 12 {
        return Vec::new();
    }
    let mut buf = payload.to_vec();
    for i in 4..payload.len() {
        buf[i] = payload[i] ^ payload[i - 4];
    }
    let n = buf.len();
    let mut out: Vec<String> = Vec::new();

    let learn = |name: &[u8], out: &mut Vec<String>| {
        if name.len() >= 2 {
            let s = latin1(name);
            if !out.contains(&s) {
                out.push(s);
            }
        }
    };

    // "<n>=<name>(" roster entries from /list output.
    let mut i = 0;
    while i < n {
        if buf[i] == b'=' {
            let start = i + 1;
            let mut j = start;
            while j < n && name_char(buf[j]) {
                j += 1;
            }
            if j < n && buf[j] == b'(' && j > start {
                learn(&buf[start..j], &mut out);
            }
            i = if j > i { j } else { i + 1 };
        } else {
            i += 1;
        }
    }

    // "channel <name>" / "joined <name>" join/invite notices.
    for kw in [b"channel ".as_slice(), b"joined ".as_slice()] {
        if let Some(p) = find(&buf, kw) {
            let start = p + kw.len();
            let mut j = start;
            while j < n && name_char(buf[j]) {
                j += 1;
            }
            if j > start {
                learn(&buf[start..j], &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// XOR-encode a plaintext UCS payload the way the server does (inverse of
    /// the decode): cipher[0..4] = plain[0..4] (seed 0), cipher[i] = plain[i] ^
    /// cipher[i-4].
    fn encode(plain: &[u8]) -> Vec<u8> {
        let mut c = plain.to_vec();
        for i in 4..plain.len() {
            c[i] = plain[i] ^ c[i - 4];
        }
        c
    }

    fn build(
        header: &[u8; 4],
        channel: &[u8],
        server_sender: &[u8],
        message: &[u8],
        score: &[u8],
    ) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(header); // masked 4-byte header
        p.extend_from_slice(channel);
        p.push(0);
        p.extend_from_slice(server_sender);
        p.push(0);
        p.extend_from_slice(message);
        p.push(0);
        p.extend_from_slice(b"SPAM:");
        p.extend_from_slice(score);
        p.push(b':');
        p.push(0);
        p
    }

    #[test]
    fn decodes_one_record() {
        let plain = build(
            b"\x01\x02\x03\x04",
            b"General",
            b"Rivervale.Alice",
            b"hey anyone selling",
            b"0",
        );
        let recs = parse_ucs_chat(&encode(&plain));
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r.channel_first, b'G'); // byte 4 of the channel field
        assert_eq!(r.channel_rest, "eneral");
        assert_eq!(r.channel_run, "General"); // trailing printable run of the field
        assert_eq!(r.sender, "Alice"); // after the last '.'
        assert_eq!(r.message, "hey anyone selling");
        assert!(!r.spam);
    }

    #[test]
    fn flags_spam_and_multiple_records() {
        let mut plain = build(
            b"\xaa\xbb\xcc\xdd",
            b"NewPlayers",
            b"Rivervale.Bob",
            b"clean line",
            b"0",
        );
        plain.extend_from_slice(&build(
            b"\x00\x00\x00\x00",
            b"General",
            b"Rivervale.Eve",
            b"spammy",
            b"7",
        ));
        let recs = parse_ucs_chat(&encode(&plain));
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].sender, "Bob");
        assert_eq!(recs[0].channel_rest, "ewPlayers");
        assert!(!recs[0].spam);
        assert_eq!(recs[1].sender, "Eve");
        assert!(recs[1].spam);
    }

    #[test]
    fn rejects_short_and_noise() {
        assert!(parse_ucs_chat(b"\x00\x15\x00\x00").is_empty()); // too short
        assert!(parse_ucs_chat(&[0u8; 64]).is_empty()); // no SPAM anchor
    }

    #[test]
    fn learns_channel_names() {
        let mut plain = b"\x01\x02\x03\x04".to_vec();
        plain.extend_from_slice(b"Channels: 1=General2(400), 2=myraid(1)\0");
        plain.extend_from_slice(b"You have joined raidchat.\0");
        let names = parse_ucs_channels(&encode(&plain));
        assert!(names.contains(&"General2".to_string())); // /list roster
        assert!(names.contains(&"myraid".to_string()));
        assert!(names.contains(&"raidchat".to_string())); // "joined <name>"
                                                          // No SPAM anchor -> no chat records.
        assert!(parse_ucs_chat(&encode(&plain)).is_empty());
    }
}
