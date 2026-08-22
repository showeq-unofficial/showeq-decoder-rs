//! Parser for the world-to-zone handoff packet.

use thiserror::Error;

pub const PAYLOAD_LEN: usize = 130;
const HOST_LEN: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneServerInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ZoneServerInfoError {
    #[error("expected {PAYLOAD_LEN} bytes, got {0}")]
    BadLength(usize),
    #[error("zone server host is empty or contains non-printable bytes")]
    InvalidHost,
    #[error("zone server port is zero")]
    InvalidPort,
}

pub fn parse_zone_server_info(bytes: &[u8]) -> Result<ZoneServerInfo, ZoneServerInfoError> {
    if bytes.len() != PAYLOAD_LEN {
        return Err(ZoneServerInfoError::BadLength(bytes.len()));
    }
    let host_end = bytes[..HOST_LEN]
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(HOST_LEN);
    let host_bytes = &bytes[..host_end];
    if host_bytes.is_empty() || !host_bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        return Err(ZoneServerInfoError::InvalidHost);
    }
    let port = u16::from_le_bytes([bytes[HOST_LEN], bytes[HOST_LEN + 1]]);
    if port == 0 {
        return Err(ZoneServerInfoError::InvalidPort);
    }
    Ok(ZoneServerInfo {
        host: String::from_utf8_lossy(host_bytes).into_owned(),
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(host: &[u8], port: u16) -> [u8; PAYLOAD_LEN] {
        let mut bytes = [0; PAYLOAD_LEN];
        bytes[..host.len()].copy_from_slice(host);
        bytes[HOST_LEN..].copy_from_slice(&port.to_le_bytes());
        bytes
    }

    #[test]
    fn parses_a_bounded_host_and_port() {
        assert_eq!(
            parse_zone_server_info(&payload(b"zone.example.test", 9000)).unwrap(),
            ZoneServerInfo {
                host: "zone.example.test".into(),
                port: 9000
            }
        );
    }

    #[test]
    fn rejects_bad_length_host_and_port() {
        assert!(matches!(
            parse_zone_server_info(&[0; PAYLOAD_LEN - 1]),
            Err(ZoneServerInfoError::BadLength(_))
        ));
        assert_eq!(
            parse_zone_server_info(&payload(b"", 9000)),
            Err(ZoneServerInfoError::InvalidHost)
        );
        assert_eq!(
            parse_zone_server_info(&payload(b"zone.example.test", 0)),
            Err(ZoneServerInfoError::InvalidPort)
        );
    }
}
