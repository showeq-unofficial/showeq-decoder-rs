//! Canonical opcode catalogs shared by every packet-decoding host.
//!
//! IDs are qualified by backend and stream. `0xffff` entries in the source
//! files are patch-day placeholders and never enter the lookup tables. The
//! schema rejects unknown sections and row fields; documented diagnostic
//! metadata is accepted but excluded from the semantic content hash.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thiserror::Error;

const UNMAPPED_ID: u16 = 0xffff;
const HASH_FORMAT: &[u8] = b"seq-protocol-data-v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackendId {
    Live,
    Test,
    Eql,
}

impl BackendId {
    pub const ALL: [Self; 3] = [Self::Live, Self::Test, Self::Eql];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Test => "test",
            Self::Eql => "eql",
        }
    }

    pub const fn relative_catalog_path(self) -> &'static str {
        match self {
            Self::Live => "opcodes.toml",
            Self::Test => "test/opcodes.toml",
            Self::Eql => "eql/opcodes.toml",
        }
    }
}

impl fmt::Display for BackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StreamKind {
    World,
    Zone,
}

impl StreamKind {
    const ALL: [Self; 2] = [Self::World, Self::Zone];

    const fn tag(self) -> u8 {
        match self {
            Self::World => 0,
            Self::Zone => 1,
        }
    }
}

impl fmt::Display for StreamKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::World => "world",
            Self::Zone => "zone",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OpcodeId(pub u16);

impl From<u16> for OpcodeId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl fmt::LowerHex for OpcodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ProtocolGeneration(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            use fmt::Write;
            write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
        }
        out
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    backend: BackendId,
    world: BTreeMap<OpcodeId, Arc<str>>,
    zone: BTreeMap<OpcodeId, Arc<str>>,
    content_hash: ContentHash,
}

impl Catalog {
    pub fn parse(backend: BackendId, source: &str) -> Result<Self, CatalogError> {
        let raw: RawCatalog =
            toml::from_str(source).map_err(|source| CatalogError::Malformed { backend, source })?;
        let world = validate_stream(backend, StreamKind::World, raw.world)?;
        let zone = validate_stream(backend, StreamKind::Zone, raw.zone)?;
        if world.is_empty() && zone.is_empty() {
            return Err(CatalogError::Empty { backend });
        }
        let content_hash = stable_hash(backend, &world, &zone);
        Ok(Self {
            backend,
            world,
            zone,
            content_hash,
        })
    }

    pub const fn backend(&self) -> BackendId {
        self.backend
    }

    pub const fn content_hash(&self) -> ContentHash {
        self.content_hash
    }

    pub fn lookup(&self, stream: StreamKind, opcode: OpcodeId) -> Option<&str> {
        self.table(stream).get(&opcode).map(AsRef::as_ref)
    }

    pub fn len(&self, stream: StreamKind) -> usize {
        self.table(stream).len()
    }

    fn table(&self, stream: StreamKind) -> &BTreeMap<OpcodeId, Arc<str>> {
        match stream {
            StreamKind::World => &self.world,
            StreamKind::Zone => &self.zone,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    #[serde(default)]
    world: Vec<RawOpcode>,
    #[serde(default)]
    zone: Vec<RawOpcode>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOpcode {
    id: String,
    name: String,
    /// Optional Rust-owned diagnostics. These do not affect lookup or the
    /// semantic content hash.
    #[serde(default, rename = "priority")]
    _priority: Option<i32>,
    #[serde(default, rename = "priority_note")]
    _priority_note: Option<String>,
    #[serde(default, rename = "updated")]
    _updated: Option<String>,
    #[serde(default, rename = "comment")]
    _comment: Option<String>,
}

fn validate_stream(
    backend: BackendId,
    stream: StreamKind,
    entries: Vec<RawOpcode>,
) -> Result<BTreeMap<OpcodeId, Arc<str>>, CatalogError> {
    let mut by_id: BTreeMap<OpcodeId, Arc<str>> = BTreeMap::new();
    let mut by_name = HashMap::new();
    for entry in entries {
        let RawOpcode {
            id: id_source,
            name,
            _priority: _,
            _priority_note: _,
            _updated: _,
            _comment: _,
        } = entry;
        let id_text = id_source.trim().trim_start_matches("0x");
        let id = u16::from_str_radix(id_text, 16).map_err(|_| CatalogError::InvalidId {
            backend,
            stream,
            id: id_source.clone(),
            name: name.clone(),
        })?;
        let name = name.trim();
        if name.is_empty() {
            return Err(CatalogError::EmptyName {
                backend,
                stream,
                id: id_source,
            });
        }

        // 0xffff means "not mapped on this patch" in the canonical host files.
        // Many named placeholders share it, so it is metadata, not a real ID.
        if id == UNMAPPED_ID {
            continue;
        }
        let id = OpcodeId(id);
        if let Some(previous) = by_id.insert(id, Arc::from(name)) {
            return Err(CatalogError::DuplicateId {
                backend,
                stream,
                id,
                first: previous.to_string(),
                second: name.to_string(),
            });
        }
        if let Some(previous) = by_name.insert(name.to_string(), id) {
            return Err(CatalogError::DuplicateName {
                backend,
                stream,
                name: name.to_string(),
                first: previous,
                second: id,
            });
        }
    }
    Ok(by_id)
}

fn stable_hash(
    backend: BackendId,
    world: &BTreeMap<OpcodeId, Arc<str>>,
    zone: &BTreeMap<OpcodeId, Arc<str>>,
) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(HASH_FORMAT);
    hasher.update([match backend {
        BackendId::Live => 0,
        BackendId::Test => 1,
        BackendId::Eql => 2,
    }]);
    for stream in StreamKind::ALL {
        let table = match stream {
            StreamKind::World => world,
            StreamKind::Zone => zone,
        };
        hasher.update([stream.tag()]);
        hasher.update((table.len() as u32).to_le_bytes());
        for (id, name) in table {
            hasher.update(id.0.to_le_bytes());
            hasher.update((name.len() as u32).to_le_bytes());
            hasher.update(name.as_bytes());
        }
    }
    ContentHash(hasher.finalize().into())
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("{backend} catalog is not valid TOML: {source}")]
    Malformed {
        backend: BackendId,
        #[source]
        source: toml::de::Error,
    },
    #[error("{backend} catalog contains no mapped opcodes")]
    Empty { backend: BackendId },
    #[error("{backend} {stream} opcode {name} has invalid hex id {id:?}")]
    InvalidId {
        backend: BackendId,
        stream: StreamKind,
        id: String,
        name: String,
    },
    #[error("{backend} {stream} opcode {id:?} has an empty name")]
    EmptyName {
        backend: BackendId,
        stream: StreamKind,
        id: String,
    },
    #[error("{backend} {stream} opcode id {id:04x} maps to both {first} and {second}")]
    DuplicateId {
        backend: BackendId,
        stream: StreamKind,
        id: OpcodeId,
        first: String,
        second: String,
    },
    #[error("{backend} {stream} opcode name {name} maps to both {first:04x} and {second:04x}")]
    DuplicateName {
        backend: BackendId,
        stream: StreamKind,
        name: String,
        first: OpcodeId,
        second: OpcodeId,
    },
}

#[derive(Debug, Error)]
pub enum ReloadError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Invalid(#[from] CatalogError),
}

#[derive(Debug, Clone)]
struct ActiveCatalog {
    generation: ProtocolGeneration,
    catalog: Arc<Catalog>,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshot(ActiveCatalog);

impl CatalogSnapshot {
    pub fn backend(&self) -> BackendId {
        self.0.catalog.backend()
    }

    pub const fn generation(&self) -> ProtocolGeneration {
        self.0.generation
    }

    pub fn content_hash(&self) -> ContentHash {
        self.0.catalog.content_hash()
    }

    pub fn lookup(&self, stream: StreamKind, opcode: OpcodeId) -> Option<&str> {
        self.0.catalog.lookup(stream, opcode)
    }

    pub fn len(&self, stream: StreamKind) -> usize {
        self.0.catalog.len(stream)
    }
}

/// Runtime registry. A write lock swaps one backend's complete catalog in one
/// operation. Parsing and validation happen before that lock is taken.
#[derive(Debug)]
pub struct ProtocolRegistry {
    catalogs: RwLock<HashMap<BackendId, ActiveCatalog>>,
}

impl ProtocolRegistry {
    pub fn embedded() -> Result<Self, CatalogError> {
        Ok(Self::new(
            Catalog::parse(BackendId::Live, include_str!("../data/live.toml"))?,
            Catalog::parse(BackendId::Test, include_str!("../data/test.toml"))?,
            Catalog::parse(BackendId::Eql, include_str!("../data/eql.toml"))?,
        ))
    }

    pub fn from_directory(path: impl AsRef<Path>) -> Result<Self, ReloadError> {
        let root = path.as_ref();
        let live = read_catalog(root, BackendId::Live)?;
        let test = read_catalog(root, BackendId::Test)?;
        let eql = read_catalog(root, BackendId::Eql)?;
        Ok(Self::new(live, test, eql))
    }

    pub fn new(live: Catalog, test: Catalog, eql: Catalog) -> Self {
        assert_eq!(live.backend(), BackendId::Live);
        assert_eq!(test.backend(), BackendId::Test);
        assert_eq!(eql.backend(), BackendId::Eql);
        Self {
            catalogs: RwLock::new(HashMap::from([
                (
                    BackendId::Live,
                    ActiveCatalog {
                        generation: ProtocolGeneration(1),
                        catalog: Arc::new(live),
                    },
                ),
                (
                    BackendId::Test,
                    ActiveCatalog {
                        generation: ProtocolGeneration(1),
                        catalog: Arc::new(test),
                    },
                ),
                (
                    BackendId::Eql,
                    ActiveCatalog {
                        generation: ProtocolGeneration(1),
                        catalog: Arc::new(eql),
                    },
                ),
            ])),
        }
    }

    pub fn snapshot(&self, backend: BackendId) -> CatalogSnapshot {
        let catalogs = read_lock(&self.catalogs);
        CatalogSnapshot(
            catalogs
                .get(&backend)
                .expect("all backend catalogs are installed at construction")
                .clone(),
        )
    }

    pub fn reload_backend_from_directory(
        &self,
        root: impl AsRef<Path>,
        backend: BackendId,
    ) -> Result<ProtocolGeneration, ReloadError> {
        let catalog = read_catalog(root.as_ref(), backend)?;
        Ok(self.replace(catalog))
    }

    pub fn replace_from_str(
        &self,
        backend: BackendId,
        source: &str,
    ) -> Result<ProtocolGeneration, CatalogError> {
        let catalog = Catalog::parse(backend, source)?;
        Ok(self.replace(catalog))
    }

    pub fn replace(&self, catalog: Catalog) -> ProtocolGeneration {
        let backend = catalog.backend();
        let mut catalogs = write_lock(&self.catalogs);
        let old = catalogs
            .get(&backend)
            .expect("all backend catalogs are installed at construction");
        let generation = ProtocolGeneration(old.generation.0.saturating_add(1));
        catalogs.insert(
            backend,
            ActiveCatalog {
                generation,
                catalog: Arc::new(catalog),
            },
        );
        generation
    }
}

fn read_catalog(root: &Path, backend: BackendId) -> Result<Catalog, ReloadError> {
    let path = root.join(backend.relative_catalog_path());
    let source = fs::read_to_string(&path).map_err(|source| ReloadError::Read {
        path: path.clone(),
        source,
    })?;
    Ok(Catalog::parse(backend, &source)?)
}

fn read_lock<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Barrier;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempCatalogDir(PathBuf);

    impl TempCatalogDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("seq-protocol-data-{}-{unique}", std::process::id()));
            fs::create_dir_all(path.join("test")).unwrap();
            fs::create_dir_all(path.join("eql")).unwrap();
            Self(path)
        }
    }

    impl Drop for TempCatalogDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn source(zone: &[(u16, &str)], world: &[(u16, &str)]) -> String {
        let mut out = String::new();
        for (id, name) in zone {
            out.push_str(&format!("[[zone]]\nid = \"{id:04x}\"\nname = \"{name}\"\n"));
        }
        for (id, name) in world {
            out.push_str(&format!(
                "[[world]]\nid = \"{id:04x}\"\nname = \"{name}\"\n"
            ));
        }
        out
    }

    #[test]
    fn embedded_catalogs_use_the_canonical_real_data() {
        let registry = ProtocolRegistry::embedded().unwrap();
        assert!(registry.snapshot(BackendId::Live).len(StreamKind::Zone) > 25);
        assert!(registry.snapshot(BackendId::Test).len(StreamKind::Zone) > 70);
        assert!(registry.snapshot(BackendId::Eql).len(StreamKind::Zone) > 60);
        assert_eq!(
            registry
                .snapshot(BackendId::Eql)
                .lookup(StreamKind::Zone, OpcodeId(0x206a)),
            Some("OP_PlayerProfile")
        );
        assert_eq!(
            registry.snapshot(BackendId::Live).content_hash().to_hex(),
            "8ba647ade8da0f99bea459e82dd6b47b2a0694b8ec131275d91536084c62577e"
        );
        assert_eq!(
            registry.snapshot(BackendId::Test).content_hash().to_hex(),
            "0180d6c6c7baad7cc480a96723b99f962c08f9c72ea3998fd22873fc53c12fe2"
        );
        assert_eq!(
            registry.snapshot(BackendId::Eql).content_hash().to_hex(),
            "4a71bf832ff4593be648812144aa6b97b002e30325a14c15ed1a0dcae9516fc7"
        );
    }

    #[test]
    fn rejects_duplicate_mapped_ids_and_malformed_files() {
        let duplicate = source(&[(0x1234, "OP_A"), (0x1234, "OP_B")], &[]);
        assert!(matches!(
            Catalog::parse(BackendId::Live, &duplicate),
            Err(CatalogError::DuplicateId { .. })
        ));
        assert!(matches!(
            Catalog::parse(BackendId::Live, "[[zone]\nid = nope"),
            Err(CatalogError::Malformed { .. })
        ));
    }

    #[test]
    fn rejects_unknown_sections_and_row_fields() {
        let unknown_section =
            "[[world]]\nid='0001'\nname='OP_World'\n\n[[znoe]]\nid='0002'\nname='OP_Zone'\n";
        assert!(matches!(
            Catalog::parse(BackendId::Live, unknown_section),
            Err(CatalogError::Malformed { .. })
        ));

        let unknown_field = "[[zone]]\nid='0001'\nname='OP_Zone'\n\n[[zone.payloads]]\ndir='server'\ntypename='ZonePacket'\nsizechecktype='exact'\n";
        assert!(matches!(
            Catalog::parse(BackendId::Live, unknown_field),
            Err(CatalogError::Malformed { .. })
        ));
    }

    #[test]
    fn accepts_documented_diagnostic_metadata_without_hashing_it() {
        let plain = "[[zone]]\nid='0001'\nname='OP_Zone'\n";
        let annotated = "[[zone]]\nid='0001'\nname='OP_Zone'\npriority=7\npriority_note='core'\nupdated='2026-08-21'\ncomment='diagnostic only'\n";
        let plain = Catalog::parse(BackendId::Live, plain).unwrap();
        let annotated = Catalog::parse(BackendId::Live, annotated).unwrap();
        assert_eq!(plain.content_hash(), annotated.content_hash());
    }

    #[test]
    fn unmapped_placeholders_may_repeat_but_are_not_looked_up() {
        let input = source(&[(0xffff, "OP_A"), (0xffff, "OP_B"), (1, "OP_C")], &[]);
        let catalog = Catalog::parse(BackendId::Live, &input).unwrap();
        assert_eq!(catalog.len(StreamKind::Zone), 1);
        assert_eq!(catalog.lookup(StreamKind::Zone, OpcodeId(0xffff)), None);
    }

    #[test]
    fn overlapping_stream_ids_stay_separate() {
        let input = source(&[(0x1234, "OP_ZoneThing")], &[(0x1234, "OP_WorldThing")]);
        let catalog = Catalog::parse(BackendId::Live, &input).unwrap();
        assert_eq!(
            catalog.lookup(StreamKind::Zone, OpcodeId(0x1234)),
            Some("OP_ZoneThing")
        );
        assert_eq!(
            catalog.lookup(StreamKind::World, OpcodeId(0x1234)),
            Some("OP_WorldThing")
        );
    }

    #[test]
    fn semantic_hash_ignores_formatting_and_comments() {
        let compact = source(&[(1, "OP_A")], &[(2, "OP_B")]);
        let reordered =
            "# comment\n[[world]]\nname='OP_B'\nid='0002'\n\n[[zone]]\nname='OP_A'\nid='0001'\n";
        let first = Catalog::parse(BackendId::Live, &compact).unwrap();
        let second = Catalog::parse(BackendId::Live, reordered).unwrap();
        assert_eq!(first.content_hash(), second.content_hash());
        assert_eq!(first.content_hash().to_hex().len(), 64);
    }

    #[test]
    fn replacement_is_atomic_and_failure_keeps_the_last_good_catalog() {
        let registry = ProtocolRegistry::embedded().unwrap();
        let before = registry.snapshot(BackendId::Live);
        let replacement = source(&[(0x1234, "OP_New")], &[]);
        assert_eq!(
            registry
                .replace_from_str(BackendId::Live, &replacement)
                .unwrap(),
            ProtocolGeneration(2)
        );
        let after = registry.snapshot(BackendId::Live);
        assert_eq!(
            after.lookup(StreamKind::Zone, OpcodeId(0x1234)),
            Some("OP_New")
        );
        assert_eq!(before.lookup(StreamKind::Zone, OpcodeId(0x1234)), None);

        assert!(registry.replace_from_str(BackendId::Live, "bad").is_err());
        let last_good = registry.snapshot(BackendId::Live);
        assert_eq!(last_good.generation(), ProtocolGeneration(2));
        assert_eq!(last_good.content_hash(), after.content_hash());
    }

    #[test]
    fn typo_reload_keeps_the_last_good_catalog() {
        let registry = ProtocolRegistry::embedded().unwrap();
        let before = registry.snapshot(BackendId::Eql);

        for invalid in [
            "[[znoe]]\nid='0001'\nname='OP_Typo'\n",
            "[[zone]]\nid='0001'\nname='OP_Typo'\nprioroty=9\n",
        ] {
            assert!(registry.replace_from_str(BackendId::Eql, invalid).is_err());
            let after = registry.snapshot(BackendId::Eql);
            assert_eq!(after.generation(), before.generation());
            assert_eq!(after.content_hash(), before.content_hash());
            assert_eq!(
                after.lookup(StreamKind::Zone, OpcodeId(0x206a)),
                Some("OP_PlayerProfile")
            );
        }
    }

    #[test]
    fn directory_reload_uses_semantic_layout_and_keeps_last_good_on_io_or_parse_error() {
        let root = TempCatalogDir::new();
        fs::write(root.0.join("opcodes.toml"), source(&[(1, "OP_Live")], &[])).unwrap();
        fs::write(
            root.0.join("test/opcodes.toml"),
            source(&[(2, "OP_Test")], &[]),
        )
        .unwrap();
        fs::write(
            root.0.join("eql/opcodes.toml"),
            source(&[(3, "OP_Eql")], &[]),
        )
        .unwrap();

        let registry = ProtocolRegistry::from_directory(&root.0).unwrap();
        assert_eq!(
            registry
                .snapshot(BackendId::Test)
                .lookup(StreamKind::Zone, OpcodeId(2)),
            Some("OP_Test")
        );

        fs::write(root.0.join("eql/opcodes.toml"), "not toml").unwrap();
        assert!(registry
            .reload_backend_from_directory(&root.0, BackendId::Eql)
            .is_err());
        let last_good = registry.snapshot(BackendId::Eql);
        assert_eq!(last_good.generation(), ProtocolGeneration(1));
        assert_eq!(
            last_good.lookup(StreamKind::Zone, OpcodeId(3)),
            Some("OP_Eql")
        );

        fs::remove_file(root.0.join("eql/opcodes.toml")).unwrap();
        assert!(registry
            .reload_backend_from_directory(&root.0, BackendId::Eql)
            .is_err());
        assert_eq!(
            registry.snapshot(BackendId::Eql).content_hash(),
            last_good.content_hash()
        );
    }

    #[test]
    fn live_and_eql_are_safe_to_lookup_concurrently() {
        let registry = Arc::new(ProtocolRegistry::embedded().unwrap());
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for (backend, id, expected) in [
            (BackendId::Live, 0x3635, "OP_PlayerProfile"),
            (BackendId::Eql, 0x206a, "OP_PlayerProfile"),
        ] {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            threads.push(thread::spawn(move || {
                barrier.wait();
                for _ in 0..1_000 {
                    assert_eq!(
                        registry
                            .snapshot(backend)
                            .lookup(StreamKind::Zone, OpcodeId(id)),
                        Some(expected)
                    );
                }
            }));
        }
        barrier.wait();
        for thread in threads {
            thread.join().unwrap();
        }
    }
}
