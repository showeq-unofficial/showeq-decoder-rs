//! Parser for `OP_ZoneServerInfo` (eql `0x2ecf`) — the world→zone handoff.
//!
//! ```text
//! /*0000*/ char     host[128]   NUL-terminated zone server hostname
//! /*0128*/ uint16_t port        little-endian UDP port
//! /*0130*/
//! ```
//!
//! Byte-identical to Live's `zoneServerInfoStruct`, confirmed by content across
//! four eql captures: `lvseqns-livz05/06/07.everquestlegends.com` on ports
//! 1499/1547/2824/3229, one fire per zone-in.
//!
//! **This is a REPORT, not a routing input.** The daemon binds its zone session
//! to the announced port; scry does not — it feeds every UDP flow to the SOE
//! layer and lets each decode on its own merits, which is why an unmapped
//! OP_ZoneServerInfo costs scry nothing. Consuming this to re-introduce port
//! binding would give up that property. It exists so a client can display where
//! the session went.
//!
//! The host is fixed-width and NUL-padded, so trailing garbage after the
//! terminator is normal — never read the field as 128 bytes of string.

use thiserror::Error;

/// `char host[128]` + `uint16 port`.
pub const PAYLOAD_LEN: usize = 130;
const PORT_OFFSET: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZoneServerInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ZoneServerInfoError {
    #[error("expected {PAYLOAD_LEN} bytes, got {0}")]
    BadLen(usize),
    #[error("zone server host is empty or contains non-printable bytes")]
    InvalidHost,
    #[error("zone server port is zero")]
    InvalidPort,
}

pub fn parse_zone_server_info(b: &[u8]) -> Result<ZoneServerInfo, ZoneServerInfoError> {
    if b.len() != PAYLOAD_LEN {
        return Err(ZoneServerInfoError::BadLen(b.len()));
    }

    // Stop at the terminator; the rest of the fixed field is padding.
    let host_bytes = &b[..PORT_OFFSET];
    let end = host_bytes
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(PORT_OFFSET);

    let host = &host_bytes[..end];
    if host.is_empty() || !host.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        return Err(ZoneServerInfoError::InvalidHost);
    }
    let port = u16::from_le_bytes([b[PORT_OFFSET], b[PORT_OFFSET + 1]]);
    if port == 0 {
        return Err(ZoneServerInfoError::InvalidPort);
    }
    Ok(ZoneServerInfo {
        host: String::from_utf8_lossy(host).into_owned(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(host: &str, port: u16) -> Vec<u8> {
        let mut b = vec![0u8; PAYLOAD_LEN];
        b[..host.len()].copy_from_slice(host.as_bytes());
        b[PORT_OFFSET..].copy_from_slice(&port.to_le_bytes());
        b
    }

    #[test]
    fn parses_host_and_port() {
        let p = payload("lvseqns-livz07.everquestlegends.com", 3229);
        assert_eq!(
            parse_zone_server_info(&p).unwrap(),
            ZoneServerInfo {
                host: "lvseqns-livz07.everquestlegends.com".into(),
                port: 3229,
            }
        );
    }

    /// The field is fixed-width and NUL-padded. Reading all 128 bytes would
    /// append the padding (and anything left over from a previous message) to
    /// the hostname.
    #[test]
    fn stops_at_the_terminator_not_the_field_width() {
        let mut p = payload("host.example.com", 1499);
        p[40] = b'X'; // garbage after the NUL
        let z = parse_zone_server_info(&p).unwrap();
        assert_eq!(z.host, "host.example.com");
        assert!(!z.host.contains('X'));
    }

    #[test]
    fn a_short_payload_is_an_error_not_a_panic() {
        assert_eq!(
            parse_zone_server_info(&[0u8; 129]).unwrap_err(),
            ZoneServerInfoError::BadLen(129)
        );
        assert!(parse_zone_server_info(&[]).is_err());
    }

    /// An unterminated 128-byte host must not run into the port field.
    #[test]
    fn an_unterminated_host_is_bounded_by_the_field() {
        let mut p = vec![b'a'; PAYLOAD_LEN];
        p[PORT_OFFSET..].copy_from_slice(&7u16.to_le_bytes());
        let z = parse_zone_server_info(&p).unwrap();
        assert_eq!(z.host.len(), PORT_OFFSET);
        assert_eq!(z.port, 7);
    }
}
