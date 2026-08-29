//! Capture-derived application-packet traces and deterministic session replay.
//!
//! The input format starts after transport reassembly. It records the exact
//! ordered application packets passed to [`seq_session::Session`].

use seq_events::{Dir, Event};
use seq_protocol_data::{BackendId, ContentHash, OpcodeId, ProtocolRegistry, StreamKind};
use seq_session::{DecodeDisposition, FlushReason, Session, SessionConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

pub const TRACE_FORMAT: &str = "seq-app-packet-trace";
pub const TRACE_VERSION: u32 = 1;
pub const GOLDEN_FORMAT: &str = "seq-session-event-golden";
pub const GOLDEN_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceBackend {
    Live,
    Test,
    Eql,
}

impl From<TraceBackend> for BackendId {
    fn from(value: TraceBackend) -> Self {
        match value {
            TraceBackend::Live => Self::Live,
            TraceBackend::Test => Self::Test,
            TraceBackend::Eql => Self::Eql,
        }
    }
}

impl From<BackendId> for TraceBackend {
    fn from(value: BackendId) -> Self {
        match value {
            BackendId::Live => Self::Live,
            BackendId::Test => Self::Test,
            BackendId::Eql => Self::Eql,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStream {
    World,
    Zone,
}

impl From<TraceStream> for StreamKind {
    fn from(value: TraceStream) -> Self {
        match value {
            TraceStream::World => Self::World,
            TraceStream::Zone => Self::Zone,
        }
    }
}

impl From<StreamKind> for TraceStream {
    fn from(value: StreamKind) -> Self {
        match value {
            StreamKind::World => Self::World,
            StreamKind::Zone => Self::Zone,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceDirection {
    ServerToClient,
    ClientToServer,
}

impl From<TraceDirection> for Dir {
    fn from(value: TraceDirection) -> Self {
        match value {
            TraceDirection::ServerToClient => Self::ServerToClient,
            TraceDirection::ClientToServer => Self::ClientToServer,
        }
    }
}

impl From<Dir> for TraceDirection {
    fn from(value: Dir) -> Self {
        match value {
            Dir::ServerToClient => Self::ServerToClient,
            Dir::ClientToServer => Self::ClientToServer,
        }
    }
}

/// One application packet after transport framing and reassembly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TracePacket {
    pub stream: TraceStream,
    pub opcode_id: u16,
    pub direction: TraceDirection,
    /// Lowercase hexadecimal bytes with no prefix or separators.
    pub payload: String,
    /// Capture time in Unix epoch milliseconds.
    pub timestamp: i64,
}

impl TracePacket {
    pub fn from_bytes(
        stream: StreamKind,
        opcode_id: OpcodeId,
        direction: Dir,
        payload: &[u8],
        timestamp: i64,
    ) -> Self {
        Self {
            stream: stream.into(),
            opcode_id: opcode_id.0,
            direction: direction.into(),
            payload: encode_hex(payload),
            timestamp,
        }
    }

    pub fn payload_bytes(&self) -> Result<Vec<u8>, TraceError> {
        decode_hex(&self.payload)
    }
}

/// Version 1 trace document. `synthetic` must be true for generated packets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceFile {
    pub format: String,
    pub version: u32,
    pub backend: TraceBackend,
    pub catalog_hash: String,
    pub synthetic: bool,
    pub packets: Vec<TracePacket>,
}

impl TraceFile {
    pub fn validate(&self) -> Result<(), TraceError> {
        if self.format != TRACE_FORMAT {
            return Err(TraceError::WrongFormat {
                expected: TRACE_FORMAT,
                actual: self.format.clone(),
            });
        }
        if self.version != TRACE_VERSION {
            return Err(TraceError::WrongVersion {
                expected: TRACE_VERSION,
                actual: self.version,
            });
        }
        validate_hash(&self.catalog_hash)?;
        let mut previous_timestamp = None;
        for (index, packet) in self.packets.iter().enumerate() {
            packet
                .payload_bytes()
                .map_err(|source| TraceError::Packet {
                    index,
                    source: Box::new(source),
                })?;
            if let Some(previous) = previous_timestamp {
                if packet.timestamp < previous {
                    return Err(TraceError::TimestampOrder {
                        index,
                        previous,
                        actual: packet.timestamp,
                    });
                }
            }
            previous_timestamp = Some(packet.timestamp);
        }
        Ok(())
    }

    pub fn validate_for_registry(&self, registry: &ProtocolRegistry) -> Result<(), TraceError> {
        self.validate()?;
        let backend = BackendId::from(self.backend);
        ensure_backend_linked(backend)?;
        let actual = registry.snapshot(backend).content_hash().to_hex();
        if actual != self.catalog_hash {
            return Err(TraceError::CatalogMismatch {
                backend,
                trace: self.catalog_hash.clone(),
                loaded: actual,
            });
        }
        Ok(())
    }
}

/// Builds traces from host application-packet callbacks.
pub struct TraceBuilder {
    trace: TraceFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceOrigin {
    Captured,
    Synthetic,
}

impl TraceBuilder {
    pub fn new(backend: BackendId, catalog_hash: ContentHash, origin: TraceOrigin) -> Self {
        Self {
            trace: TraceFile {
                format: TRACE_FORMAT.to_owned(),
                version: TRACE_VERSION,
                backend: backend.into(),
                catalog_hash: catalog_hash.to_hex(),
                synthetic: origin == TraceOrigin::Synthetic,
                packets: Vec::new(),
            },
        }
    }

    pub fn for_registry(
        registry: &ProtocolRegistry,
        backend: BackendId,
        origin: TraceOrigin,
    ) -> Self {
        Self::new(backend, registry.snapshot(backend).content_hash(), origin)
    }

    pub fn push(
        &mut self,
        stream: StreamKind,
        opcode_id: OpcodeId,
        direction: Dir,
        payload: &[u8],
        timestamp: i64,
    ) -> Result<&mut Self, TraceError> {
        if let Some(previous) = self.trace.packets.last() {
            if timestamp < previous.timestamp {
                return Err(TraceError::TimestampOrder {
                    index: self.trace.packets.len(),
                    previous: previous.timestamp,
                    actual: timestamp,
                });
            }
        }
        self.trace.packets.push(TracePacket::from_bytes(
            stream, opcode_id, direction, payload, timestamp,
        ));
        Ok(self)
    }

    pub fn finish(self) -> TraceFile {
        self.trace
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayBatch {
    pub packet_index: usize,
    pub protocol_generation: u64,
    pub disposition: DecodeDisposition,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayResult {
    pub batches: Vec<ReplayBatch>,
    pub flush_events: Vec<Event>,
}

/// Replays every packet through one new session and flushes at replay end.
pub fn replay(
    trace: &TraceFile,
    registry: Arc<ProtocolRegistry>,
) -> Result<ReplayResult, TraceError> {
    trace.validate_for_registry(&registry)?;
    let backend = BackendId::from(trace.backend);
    let mut session = Session::new(SessionConfig {
        backend,
        protocol_registry: registry,
    })?;
    let mut batches = Vec::with_capacity(trace.packets.len());
    for (packet_index, packet) in trace.packets.iter().enumerate() {
        let payload = packet.payload_bytes()?;
        let batch = session.decode_at(
            packet.stream.into(),
            OpcodeId(packet.opcode_id),
            packet.direction.into(),
            &payload,
            packet.timestamp,
        );
        batches.push(ReplayBatch {
            packet_index,
            protocol_generation: batch.protocol_generation.0,
            disposition: batch.disposition,
            events: batch.events,
        });
    }
    Ok(ReplayResult {
        batches,
        flush_events: session.flush(FlushReason::ReplayEnd),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoldenDisposition {
    Decoded,
    Ignored,
    Unhandled,
    Malformed,
    Unmapped,
}

impl From<DecodeDisposition> for GoldenDisposition {
    fn from(value: DecodeDisposition) -> Self {
        match value {
            DecodeDisposition::Decoded => Self::Decoded,
            DecodeDisposition::Ignored => Self::Ignored,
            DecodeDisposition::Unhandled => Self::Unhandled,
            DecodeDisposition::Malformed => Self::Malformed,
            DecodeDisposition::Unmapped => Self::Unmapped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenBatch {
    pub packet_index: usize,
    pub protocol_generation: u64,
    pub disposition: GoldenDisposition,
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenFlush {
    pub reason: String,
    pub events: Vec<Value>,
}

/// Exact, ordered JSON representation of every decode batch and replay flush.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenFile {
    pub format: String,
    pub version: u32,
    pub backend: TraceBackend,
    pub catalog_hash: String,
    pub synthetic: bool,
    pub batches: Vec<GoldenBatch>,
    pub flush: GoldenFlush,
}

impl GoldenFile {
    pub fn from_replay(trace: &TraceFile, replay: &ReplayResult) -> Result<Self, TraceError> {
        let batches = replay
            .batches
            .iter()
            .map(|batch| {
                Ok(GoldenBatch {
                    packet_index: batch.packet_index,
                    protocol_generation: batch.protocol_generation,
                    disposition: batch.disposition.into(),
                    events: serialize_events(&batch.events)?,
                })
            })
            .collect::<Result<Vec<_>, TraceError>>()?;
        Ok(Self {
            format: GOLDEN_FORMAT.to_owned(),
            version: GOLDEN_VERSION,
            backend: trace.backend,
            catalog_hash: trace.catalog_hash.clone(),
            synthetic: trace.synthetic,
            batches,
            flush: GoldenFlush {
                reason: "replay_end".to_owned(),
                events: serialize_events(&replay.flush_events)?,
            },
        })
    }

    pub fn validate(&self) -> Result<(), TraceError> {
        if self.format != GOLDEN_FORMAT {
            return Err(TraceError::WrongFormat {
                expected: GOLDEN_FORMAT,
                actual: self.format.clone(),
            });
        }
        if self.version != GOLDEN_VERSION {
            return Err(TraceError::WrongVersion {
                expected: GOLDEN_VERSION,
                actual: self.version,
            });
        }
        validate_hash(&self.catalog_hash)?;
        if self.flush.reason != "replay_end" {
            return Err(TraceError::InvalidFlushReason(self.flush.reason.clone()));
        }
        for (expected, batch) in self.batches.iter().enumerate() {
            if batch.packet_index != expected {
                return Err(TraceError::GoldenPacketIndex {
                    position: expected,
                    actual: batch.packet_index,
                });
            }
        }
        Ok(())
    }
}

fn serialize_events(events: &[Event]) -> Result<Vec<Value>, TraceError> {
    events
        .iter()
        .map(|event| serde_json::to_value(event).map_err(TraceError::Json))
        .collect()
}

pub fn load_trace(path: impl AsRef<Path>) -> Result<TraceFile, TraceError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| TraceError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let trace: TraceFile = serde_json::from_str(&source).map_err(TraceError::Json)?;
    trace.validate()?;
    Ok(trace)
}

pub fn load_trace_reader(mut reader: impl Read) -> Result<TraceFile, TraceError> {
    let mut source = String::new();
    reader.read_to_string(&mut source).map_err(TraceError::Io)?;
    let trace: TraceFile = serde_json::from_str(&source).map_err(TraceError::Json)?;
    trace.validate()?;
    Ok(trace)
}

pub fn load_golden(path: impl AsRef<Path>) -> Result<GoldenFile, TraceError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| TraceError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let golden: GoldenFile = serde_json::from_str(&source).map_err(TraceError::Json)?;
    golden.validate()?;
    Ok(golden)
}

pub fn write_trace(trace: &TraceFile, mut writer: impl Write) -> Result<(), TraceError> {
    trace.validate()?;
    serde_json::to_writer_pretty(&mut writer, trace).map_err(TraceError::Json)?;
    writer.write_all(b"\n").map_err(TraceError::Io)
}

pub fn write_golden(golden: &GoldenFile, mut writer: impl Write) -> Result<(), TraceError> {
    golden.validate()?;
    serde_json::to_writer_pretty(&mut writer, golden).map_err(TraceError::Json)?;
    writer.write_all(b"\n").map_err(TraceError::Io)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenMismatch {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for GoldenMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "golden differs at {}", self.path)?;
        writeln!(f, "expected: {}", self.expected)?;
        write!(f, "actual:   {}", self.actual)
    }
}

pub fn compare_golden(expected: &GoldenFile, actual: &GoldenFile) -> Result<(), GoldenMismatch> {
    if expected == actual {
        return Ok(());
    }
    let expected_value =
        serde_json::to_value(expected).expect("golden serialization is infallible");
    let actual_value = serde_json::to_value(actual).expect("golden serialization is infallible");
    let mismatch = first_mismatch("", &expected_value, &actual_value)
        .expect("unequal JSON documents have a first mismatch");
    Err(mismatch)
}

fn first_mismatch(path: &str, expected: &Value, actual: &Value) -> Option<GoldenMismatch> {
    match (expected, actual) {
        (Value::Array(left), Value::Array(right)) => {
            for index in 0..left.len().min(right.len()) {
                let child = format!("{path}/{index}");
                if let Some(found) = first_mismatch(&child, &left[index], &right[index]) {
                    return Some(found);
                }
            }
            (left.len() != right.len()).then(|| GoldenMismatch {
                path: path_or_root(path),
                expected: format!("array length {}", left.len()),
                actual: format!("array length {}", right.len()),
            })
        }
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys().chain(right.keys()) {
                let child = format!("{path}/{}", escape_pointer(key));
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(found) = first_mismatch(&child, left, right) {
                            return Some(found);
                        }
                    }
                    (Some(value), None) => {
                        return Some(GoldenMismatch {
                            path: child,
                            expected: short_json(value),
                            actual: "<missing>".to_owned(),
                        });
                    }
                    (None, Some(value)) => {
                        return Some(GoldenMismatch {
                            path: child,
                            expected: "<missing>".to_owned(),
                            actual: short_json(value),
                        });
                    }
                    (None, None) => {}
                }
            }
            None
        }
        _ if expected == actual => None,
        _ => Some(GoldenMismatch {
            path: path_or_root(path),
            expected: short_json(expected),
            actual: short_json(actual),
        }),
    }
}

fn path_or_root(path: &str) -> String {
    if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn short_json(value: &Value) -> String {
    const LIMIT: usize = 400;
    let text = serde_json::to_string(value).expect("JSON value serialization is infallible");
    if text.len() <= LIMIT {
        text
    } else {
        let prefix = text.chars().take(LIMIT).collect::<String>();
        format!("{prefix}... <{} bytes>", text.len())
    }
}

/// Replaces printable ASCII bytes while keeping the field width and NUL bytes.
/// This rejects binary fields so a mistaken offset cannot silently corrupt a fixture.
pub fn scrub_ascii_range(
    payload: &mut [u8],
    range: Range<usize>,
    replacement: u8,
) -> Result<(), TraceError> {
    if !replacement.is_ascii_graphic() {
        return Err(TraceError::InvalidScrubReplacement(replacement));
    }
    let Some(field) = payload.get_mut(range.clone()) else {
        return Err(TraceError::ScrubRange {
            start: range.start,
            end: range.end,
            payload_len: payload.len(),
        });
    };
    for (offset, byte) in field.iter_mut().enumerate() {
        if *byte == 0 {
            continue;
        }
        if !byte.is_ascii_graphic() && *byte != b' ' {
            return Err(TraceError::NonAsciiScrubByte {
                offset: range.start + offset,
                byte: *byte,
            });
        }
        *byte = replacement;
    }
    Ok(())
}

fn ensure_backend_linked(backend: BackendId) -> Result<(), TraceError> {
    let linked = match backend {
        BackendId::Live => cfg!(feature = "backend-live"),
        BackendId::Test => cfg!(feature = "backend-test"),
        BackendId::Eql => cfg!(feature = "backend-eql"),
    };
    if linked {
        Ok(())
    } else {
        Err(TraceError::BackendNotLinked(backend))
    }
}

fn validate_hash(value: &str) -> Result<(), TraceError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(TraceError::InvalidCatalogHash(value.to_owned()));
    }
    Ok(())
}

fn encode_hex(payload: &[u8]) -> String {
    let mut out = String::with_capacity(payload.len() * 2);
    for byte in payload {
        use fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn decode_hex(source: &str) -> Result<Vec<u8>, TraceError> {
    if source.len() % 2 != 0 {
        return Err(TraceError::InvalidPayloadHex(
            "payload has an odd number of hexadecimal digits".to_owned(),
        ));
    }
    if !source
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(TraceError::InvalidPayloadHex(
            "payload must contain lowercase hexadecimal digits only".to_owned(),
        ));
    }
    source
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hexadecimal digits are ASCII");
            u8::from_str_radix(text, 16).map_err(|error| {
                TraceError::InvalidPayloadHex(format!("invalid byte {text:?}: {error}"))
            })
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum TraceError {
    #[error(transparent)]
    Session(#[from] seq_session::SessionError),
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("I/O error: {0}")]
    Io(#[source] io::Error),
    #[error("JSON error: {0}")]
    Json(#[source] serde_json::Error),
    #[error("expected format {expected:?}, found {actual:?}")]
    WrongFormat {
        expected: &'static str,
        actual: String,
    },
    #[error("expected format version {expected}, found {actual}")]
    WrongVersion { expected: u32, actual: u32 },
    #[error("catalog_hash must be exactly 64 lowercase hexadecimal digits, found {0:?}")]
    InvalidCatalogHash(String),
    #[error("packet {index}: {source}")]
    Packet {
        index: usize,
        #[source]
        source: Box<TraceError>,
    },
    #[error("invalid payload hex: {0}")]
    InvalidPayloadHex(String),
    #[error("packet {index} timestamp {actual} precedes the previous packet timestamp {previous}")]
    TimestampOrder {
        index: usize,
        previous: i64,
        actual: i64,
    },
    #[error("{backend} catalog hash mismatch: trace records {trace}, loaded catalog is {loaded}")]
    CatalogMismatch {
        backend: BackendId,
        trace: String,
        loaded: String,
    },
    #[error("this seq-trace build does not link the {0} backend")]
    BackendNotLinked(BackendId),
    #[error("golden flush reason must be replay_end, found {0:?}")]
    InvalidFlushReason(String),
    #[error("golden batch at position {position} has packet_index {actual}")]
    GoldenPacketIndex { position: usize, actual: usize },
    #[error("scrub replacement byte 0x{0:02x} is not printable ASCII")]
    InvalidScrubReplacement(u8),
    #[error("scrub range {start}..{end} exceeds payload length {payload_len}")]
    ScrubRange {
        start: usize,
        end: usize,
        payload_len: usize,
    },
    #[error("scrub range contains non-ASCII byte 0x{byte:02x} at payload offset {offset}")]
    NonAsciiScrubByte { offset: usize, byte: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_trace() -> (Arc<ProtocolRegistry>, TraceFile) {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        #[cfg(feature = "backend-live")]
        let (backend, opcode) = (BackendId::Live, OpcodeId(0x5a59));
        #[cfg(all(not(feature = "backend-live"), feature = "backend-test"))]
        let (backend, opcode) = (BackendId::Test, OpcodeId(0xf31f));
        #[cfg(all(
            not(feature = "backend-live"),
            not(feature = "backend-test"),
            feature = "backend-eql"
        ))]
        let (backend, opcode) = (BackendId::Eql, OpcodeId(0x0935));
        let mut builder = TraceBuilder::for_registry(&registry, backend, TraceOrigin::Synthetic);
        let mut enter_world = [0_u8; 72];
        enter_world[..6].copy_from_slice(b"Tester");
        builder
            .push(
                StreamKind::World,
                opcode,
                Dir::ClientToServer,
                &enter_world,
                1_700_000_000_000,
            )
            .unwrap();
        (registry, builder.finish())
    }

    #[test]
    fn synthetic_trace_round_trips_and_replays() {
        let (registry, trace) = synthetic_trace();
        let mut json = Vec::new();
        write_trace(&trace, &mut json).unwrap();
        let loaded = load_trace_reader(json.as_slice()).unwrap();
        assert_eq!(loaded, trace);

        let result = replay(&loaded, registry).unwrap();
        assert_eq!(result.batches.len(), 1);
        assert_eq!(result.batches[0].disposition, DecodeDisposition::Decoded);
        assert_eq!(result.batches[0].events.len(), 2);

        let first = GoldenFile::from_replay(&loaded, &result).unwrap();
        let second = GoldenFile::from_replay(
            &loaded,
            &replay(&loaded, Arc::new(ProtocolRegistry::embedded().unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert!(first.synthetic);
        assert_eq!(
            first.batches[0].events,
            vec![
                serde_json::json!({"SessionReset": {"reason": "EnterWorld"}}),
                serde_json::json!({"EnterWorld": {"character_name": "Tester"}})
            ]
        );
    }

    #[test]
    fn synthetic_validation_rejects_catalog_and_packet_drift() {
        let (registry, mut trace) = synthetic_trace();
        let wrong_digit = if trace.catalog_hash.starts_with('0') {
            "1"
        } else {
            "0"
        };
        trace.catalog_hash.replace_range(..1, wrong_digit);
        assert!(matches!(
            trace.validate_for_registry(&registry),
            Err(TraceError::CatalogMismatch { .. })
        ));

        trace.catalog_hash = registry
            .snapshot(trace.backend.into())
            .content_hash()
            .to_hex();
        trace.packets[0].payload = "AA".to_owned();
        assert!(matches!(trace.validate(), Err(TraceError::Packet { .. })));
    }

    #[test]
    fn synthetic_diff_names_the_first_event_field() {
        let (registry, trace) = synthetic_trace();
        let replay = replay(&trace, registry).unwrap();
        let actual = GoldenFile::from_replay(&trace, &replay).unwrap();
        let mut expected = actual.clone();
        expected.batches[0].events[1]["EnterWorld"]["character_name"] =
            Value::String("Redacted".to_owned());

        let mismatch = compare_golden(&expected, &actual).unwrap_err();
        assert_eq!(
            mismatch.path,
            "/batches/0/events/1/EnterWorld/character_name"
        );
        assert!(mismatch.to_string().contains("Redacted"));
        assert!(mismatch.to_string().contains("Tester"));
    }

    #[test]
    fn synthetic_scrubber_preserves_width_and_nuls() {
        let mut payload = b"Alice\0hello world\0".to_vec();
        let original_len = payload.len();
        scrub_ascii_range(&mut payload, 0..18, b'x').unwrap();
        assert_eq!(payload.len(), original_len);
        assert_eq!(&payload, b"xxxxx\0xxxxxxxxxxx\0");
    }

    #[test]
    fn synthetic_loader_rejects_unknown_fields_and_decreasing_time() {
        let (_, trace) = synthetic_trace();
        let mut value = serde_json::to_value(&trace).unwrap();
        value["unexpected"] = Value::Bool(true);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(matches!(
            load_trace_reader(bytes.as_slice()),
            Err(TraceError::Json(_))
        ));

        let mut builder = TraceBuilder::new(
            trace.backend.into(),
            ProtocolRegistry::embedded()
                .unwrap()
                .snapshot(trace.backend.into())
                .content_hash(),
            TraceOrigin::Synthetic,
        );
        builder
            .push(StreamKind::World, OpcodeId(1), Dir::ClientToServer, &[], 20)
            .unwrap();
        assert!(matches!(
            builder.push(StreamKind::World, OpcodeId(2), Dir::ClientToServer, &[], 19,),
            Err(TraceError::TimestampOrder { .. })
        ));
    }
}
