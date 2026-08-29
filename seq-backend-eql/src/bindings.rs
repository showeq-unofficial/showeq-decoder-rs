// eql-OWNED wire structs — HAND-MAINTAINED. Edit this file directly.
//
// Originally emitted by tools/gen_eqstructs.py, but eql is no longer
// generated and this is no longer a generated file (2026-08-03). The
// generator's only possible input is Live's everquest.h, and there is no eql
// fork header — so "regenerating" would import Live's layouts into eql, the
// exact coupling the 2026-07-09 clean break removed. It had also not been
// regenerated since that break, while eql's wire kept diverging, so the
// @generated banner was claiming a guarantee nothing enforced and blocking
// the correct in-place fix when a struct diverged.
//
// Keep every struct paired with a size assertion in __layout_tests below:
// that is what actually guards these layouts, and it works the same whether
// the file is generated or hand-written. When eql's wire diverges, change the
// struct HERE and update its assertion — do not model the record somewhere
// else and leave a wrong struct standing.
//
// Live and test bindings ARE still generated (seq-structs-{live,test}); the
// generator and the daemon's pre-push freshness check both cover only those.
//
// Lints (#![allow(non_camel_case_types)] etc.) are applied at the crate
// root in lib.rs since this file is included via include!() and cannot
// carry inner attributes itself.

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct spawnPositionUpdate {
    /// int16_t spawnId
    pub spawnId: i16,
    /// uint8_t unk1[6] — grew 2 -> 6 on the 08/25 patch, moving the block to byte 8
    pub unk1: [u8; 6],
    /// packed bitfield: y:19 z:19 u3:7 x:19 unused2:4 heading:12
    pub _bits: [u8; 10],
}

impl Default for spawnPositionUpdate {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct deleteSpawnStruct {
    /// uint32_t spawnId
    pub spawnId: u32,
}

impl Default for deleteSpawnStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct removeSpawnStruct {
    /// uint32_t spawnId
    pub spawnId: u32,
    /// uint8_t removeSpawn
    pub removeSpawn: u8,
}

impl Default for removeSpawnStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct hpNpcUpdateStruct {
    /// uint16_t spawnId
    pub spawnId: u16,
    /// int32_t curHP
    pub curHP: i32,
    /// uint32_t unknown0006
    pub unknown0006: u32,
    /// int32_t maxHP
    pub maxHP: i32,
    /// uint32_t unknown0014
    pub unknown0014: u32,
}

impl Default for hpNpcUpdateStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct mobHealthStruct {
    /// uint16_t spawnId
    pub spawnId: u16,
    /// int32_t hpPercent
    pub hpPercent: i32,
}

impl Default for mobHealthStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct spawnAppearanceStruct {
    /// uint16_t spawnId
    pub spawnId: u16,
    /// uint16_t type
    pub type_: u16,
    /// uint32_t parameter
    pub parameter: u32,
}

impl Default for spawnAppearanceStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct expUpdateStruct {
    /// uint32_t exp
    pub exp: u32,
    /// uint32_t unknown0004
    pub unknown0004: u32,
    /// uint32_t type
    pub type_: u32,
    /// uint32_t unknown0012
    pub unknown0012: u32,
}

impl Default for expUpdateStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct levelUpUpdateStruct {
    /// uint32_t level
    pub level: u32,
    /// uint32_t levelOld
    pub levelOld: u32,
    /// uint32_t exp
    pub exp: u32,
    /// uint32_t unknown0012
    pub unknown0012: u32,
    /// uint32_t unknown0016
    pub unknown0016: u32,
    /// uint32_t unknown0020
    pub unknown0020: u32,
}

impl Default for levelUpUpdateStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct skillIncStruct {
    /// uint32_t skillId
    pub skillId: u32,
    /// int32_t value
    pub value: i32,
    /// uint8_t unknown0008[4]
    pub unknown0008: [u8; 4],
}

impl Default for skillIncStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct manaDecrementStruct {
    /// int32_t newMana
    pub newMana: i32,
    /// int32_t curEndurance
    pub curEndurance: i32,
    /// int32_t spellId
    pub spellId: i32,
    /// uint8_t unknown0012[4]
    pub unknown0012: [u8; 4],
    /// uint8_t unknown0016[4]
    pub unknown0016: [u8; 4],
}

impl Default for manaDecrementStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct staminaStruct {
    /// uint32_t food
    pub food: u32,
    /// uint32_t water
    pub water: u32,
}

impl Default for staminaStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct endUpdateStruct {
    /// uint16_t spawn_id
    pub spawn_id: u16,
    /// uint32_t cur
    pub cur: u32,
    /// uint32_t max
    pub max: u32,
}

impl Default for endUpdateStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

// eql /consider is 24B in BOTH directions, NOT Live's 32B considerStruct. This
// pinned fork is hand-edited to eql's real layout (clean-break rule: eql owns
// its structs; edit by hand when the eql wire genuinely diverges). Decoded by
// `parse_consider`; this struct's size is what the daemon `SZC_Match`-gates on,
// surfaced via seq-bridge `struct_size_overrides`.
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct considerStruct {
    /// uint32_t playerid
    pub playerid: u32,
    /// uint32_t targetid
    pub targetid: u32,
    /// int32_t faction — 0 on the C>S request; friendliness word on the S>C
    /// reply (2=warmly, 4=amiably). level is NOT here (read from the spawn).
    pub faction: i32,
    /// int32_t unknown0012 (observed = 7)
    pub unknown0012: i32,
    /// int32_t unknown0016
    pub unknown0016: i32,
    /// int32_t unknown0020
    pub unknown0020: i32,
}

impl Default for considerStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct spawnRenameStruct {
    /// char old_name[64]
    pub old_name: [u8; 64],
    /// char old_name_again[64]
    pub old_name_again: [u8; 64],
    /// char new_name[64]
    pub new_name: [u8; 64],
    /// uint8_t unknown0192[3]
    pub unknown0192: [u8; 3],
}

impl Default for spawnRenameStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct clientTargetStruct {
    /// uint32_t newTarget
    pub newTarget: u32,
}

impl Default for clientTargetStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct newCorpseStruct {
    /// uint32_t spawnId
    pub spawnId: u32,
    /// uint32_t killerId
    pub killerId: u32,
    /// uint32_t corpseid
    pub corpseid: u32,
    /// int32_t type
    pub type_: i32,
    /// uint32_t spellId
    pub spellId: u32,
    /// uint16_t zoneId
    pub zoneId: u16,
    /// uint16_t zoneInstance
    pub zoneInstance: u16,
    /// uint32_t damage
    pub damage: u32,
    /// uint8_t unknown0028[12]
    pub unknown0028: [u8; 12],
}

impl Default for newCorpseStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct remDropStruct {
    /// uint16_t dropId
    pub dropId: u16,
    /// uint8_t unknown0002[2]
    pub unknown0002: [u8; 2],
    /// uint16_t spawnId
    pub spawnId: u16,
    /// uint8_t unknown0006[2]
    pub unknown0006: [u8; 2],
    /// uint8_t unknown0008[4]
    pub unknown0008: [u8; 4],
}

impl Default for remDropStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct spawnIllusionStruct {
    /// uint32_t spawnId
    pub spawnId: u32,
    /// char name[64]
    pub name: [u8; 64],
    /// uint32_t race
    pub race: u32,
    /// uint8_t gender
    pub gender: u8,
    /// uint8_t texture
    pub texture: u8,
    /// uint8_t helm
    pub helm: u8,
    /// uint8_t unknown0075
    pub unknown0075: u8,
    /// uint32_t unknown0076
    pub unknown0076: u32,
    /// uint32_t face
    pub face: u32,
    /// uint8_t unknown0084[248]
    pub unknown0084: [u8; 248],
}

impl Default for spawnIllusionStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct buffStruct {
    /// uint32_t spawnid
    pub spawnid: u32,
    /// uint8_t unknown0004[4]
    pub unknown0004: [u8; 4],
    /// uint8_t unknown0008[4]
    pub unknown0008: [u8; 4],
    /// uint8_t unknown0012[104]
    pub unknown0012: [u8; 104],
    /// uint32_t spellid
    pub spellid: u32,
    /// uint32_t duration
    pub duration: u32,
    /// uint32_t initialDuration
    pub initialDuration: u32,
    /// uint8_t unknown0128[8]
    pub unknown0128: [u8; 8],
    /// float unknown0136
    pub unknown0136: f32,
    /// uint8_t unknown0140[12]
    pub unknown0140: [u8; 12],
    /// uint8_t level
    pub level: u8,
    /// uint8_t unknown0153[7]
    pub unknown0153: [u8; 7],
    /// uint32_t spellslot
    pub spellslot: u32,
    /// uint32_t changetype
    pub changetype: u32,
}

impl Default for buffStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct action2Struct {
    /// uint16_t target
    pub target: u16,
    /// uint16_t source
    pub source: u16,
    /// uint8_t unknown0004[4]
    pub unknown0004: [u8; 4],
    /// int32_t damage
    pub damage: i32,
    /// int8_t unknown0012[8]
    pub unknown0012: [i8; 8],
    /// int32_t spell
    pub spell: i32,
    /// uint8_t uknown0024[16]
    pub uknown0024: [u8; 16],
    /// uint8_t type
    pub type_: u8,
    /// uint8_t unknown0042[7]
    pub unknown0042: [u8; 7],
}

impl Default for action2Struct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct SpawnUpdateStruct {
    /// uint16_t spawnId
    pub spawnId: u16,
    /// uint16_t subcommand
    pub subcommand: u16,
    /// int16_t arg1
    pub arg1: i16,
    /// int16_t arg2
    pub arg2: i16,
    /// uint8_t arg3
    pub arg3: u8,
    /// uint8_t unknown0009[23]
    pub unknown0009: [u8; 23],
}

impl Default for SpawnUpdateStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct zoneChangeStruct {
    /// char name[64]
    pub name: [u8; 64],
    /// uint16_t zoneId
    pub zoneId: u16,
    /// uint16_t zoneInstance
    pub zoneInstance: u16,
    /// uint8_t unknown0068[8]
    pub unknown0068: [u8; 8],
    /// uint8_t unknown0076[12]
    pub unknown0076: [u8; 12],
    /// uint8_t unknown0088[4]
    pub unknown0088: [u8; 4],
    /// uint8_t unknown0092[8]
    pub unknown0092: [u8; 8],
}

impl Default for zoneChangeStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct dzInfo {
    /// uint32_t unknown0000
    pub unknown0000: u32,
    /// uint32_t unknown0004
    pub unknown0004: u32,
    /// uint8_t newDZ
    pub newDZ: u8,
    /// uint8_t padding0009[3]
    pub padding0009: [u8; 3],
    /// uint32_t maxPlayers
    pub maxPlayers: u32,
    /// char dzName[128]
    pub dzName: [u8; 128],
    /// char name[64]
    pub name: [u8; 64],
    /// uint32_t unknown0208
    pub unknown0208: u32,
}

impl Default for dzInfo {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct dzSwitchInfo {
    /// uint32_t unknown0000
    pub unknown0000: u32,
    /// uint32_t show
    pub show: u32,
    /// uint16_t zoneID
    pub zoneID: u16,
    /// uint16_t instanceID
    pub instanceID: u16,
    /// uint32_t type
    pub type_: u32,
    /// uint32_t unknown0016
    pub unknown0016: u32,
    /// float y
    pub y: f32,
    /// float x
    pub x: f32,
    /// float z
    pub z: f32,
}

impl Default for dzSwitchInfo {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
/// DIVERGED from Live 2026-08-03: eql's record is 44 bytes, Live's is 39. The
/// three consumed fields kept their offsets; the tail grew. Validated on 10
/// captured casts (see start_cast.rs).
pub struct startCastStruct {
    /// int32_t slot
    pub slot: i32,
    /// uint32_t spellId
    pub spellId: u32,
    /// uint8_t unknown0008[10] — reads 0xff on every capture
    pub unknown0008: [u8; 10],
    /// uint32_t targetId — 0 when cast with no target
    pub targetId: u32,
    /// uint32_t unknown0022 — per-spell constant, role unmapped
    pub unknown0022: u32,
    /// uint8_t unknown0026[18] — zero except a 1 at offset 31
    pub unknown0026: [u8; 18],
}

impl Default for startCastStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct actionStruct {
    /// uint16_t target
    pub target: u16,
    /// uint16_t source
    pub source: u16,
    /// uint16_t spell
    pub spell: u16,
    /// uint8_t unknown0006[6]
    pub unknown0006: [u8; 6],
    /// uint8_t level
    pub level: u8,
    /// uint8_t unknown0013[43]
    pub unknown0013: [u8; 43],
    /// uint8_t type
    pub type_: u8,
    /// uint8_t unknown0057[7]
    pub unknown0057: [u8; 7],
}

impl Default for actionStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct actionAltStruct {
    /// uint16_t target
    pub target: u16,
    /// uint16_t source
    pub source: u16,
    /// uint16_t spell
    pub spell: u16,
    /// uint8_t unknown0006[6]
    pub unknown0006: [u8; 6],
    /// uint8_t level
    pub level: u8,
    /// uint8_t unknown0013[43]
    pub unknown0013: [u8; 43],
    /// uint8_t type
    pub type_: u8,
    /// uint8_t unknown0057[31]
    pub unknown0057: [u8; 31],
}

impl Default for actionAltStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct groupDisbandStruct {
    /// char yourname[64]
    pub yourname: [u8; 64],
    /// char membername[64]
    pub membername: [u8; 64],
    /// uint8_t unknown0128[24]
    pub unknown0128: [u8; 24],
}

impl Default for groupDisbandStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct corpseLocStruct {
    /// uint32_t spawnId
    pub spawnId: u32,
    /// float x
    pub x: f32,
    /// float y
    pub y: f32,
    /// float z
    pub z: f32,
}

impl Default for corpseLocStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
/// DIVERGED from Live 2026-07-13: eql's door row is 132 bytes, Live's is 136.
/// The first 88 bytes are byte-identical; the trailing unknown region is 44
/// instead of 48. Rows are iterated by the daemon at this stride.
pub struct doorStruct {
    /// char name[32]
    pub name: [u8; 32],
    /// float y
    pub y: f32,
    /// float x
    pub x: f32,
    /// float z
    pub z: f32,
    /// float heading
    pub heading: f32,
    /// uint32_t incline
    pub incline: u32,
    /// uint8_t unknown0048[20]
    pub unknown0048: [u8; 20],
    /// uint32_t size
    pub size: u32,
    /// uint8_t unknown0056[4]
    pub unknown0056: [u8; 4],
    /// uint8_t doorId
    pub doorId: u8,
    /// uint8_t opentype
    pub opentype: u8,
    /// uint8_t spawnstate
    pub spawnstate: u8,
    /// uint8_t invertstate
    pub invertstate: u8,
    /// uint32_t zonePoint
    pub zonePoint: u32,
    /// uint8_t unknown068[44] — eql's trailing region is 44, not Live's 28+20.
    /// Everything above is byte-identical to Live; only this tail differs.
    pub unknown068: [u8; 44],
}

impl Default for doorStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct zonePointStruct {
    /// uint32_t zoneTrigger
    pub zoneTrigger: u32,
    /// float y
    pub y: f32,
    /// float x
    pub x: f32,
    /// float z
    pub z: f32,
    /// float heading
    pub heading: f32,
    /// uint16_t zoneId
    pub zoneId: u16,
    /// uint16_t zoneInstance
    pub zoneInstance: u16,
}

impl Default for zonePointStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct simpleMessageStruct {
    /// uint32_t messageFormat
    pub messageFormat: u32,
    /// ChatColor messageColor
    pub messageColor: u32,
    /// uint32_t unknown
    pub unknown: u32,
}

impl Default for simpleMessageStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct formattedMessageStruct {
    /// uint8_t unknown0000
    pub unknown0000: u8,
    /// uint8_t unknown0001[4]
    pub unknown0001: [u8; 4],
    /// uint32_t messageFormat
    pub messageFormat: u32,
    /// ChatColor messageColor
    pub messageColor: u32,
    /// char messages[0]
    pub messages: [u8; 0],
}

impl Default for formattedMessageStruct {
    fn default() -> Self {
        // SAFETY: all fields are POD and zero-bit-pattern is valid.
        unsafe { core::mem::zeroed() }
    }
}

impl spawnPositionUpdate {
    #[inline]
    fn pack(&self) -> [u8; 10] {
        // Copy out the packed bytes via addr_of! — the field is on a #[repr(C, packed)]
        // struct and may be unaligned.
        unsafe { core::ptr::addr_of!(self._bits).read_unaligned() }
    }
    #[inline]
    fn lo64(&self) -> u64 {
        let p = self.pack();
        u64::from_le_bytes([p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]])
    }
    #[inline]
    fn hi16(&self) -> u16 {
        let p = self.pack();
        u16::from_le_bytes([p[8], p[9]])
    }
    /// Low 19 bits of the packed int64 (offset 4) — Y coordinate.
    #[inline]
    pub fn y(&self) -> u64 { self.lo64() & ((1 << 19) - 1) }
    /// Bits 19..38 of the packed int64 — Z coordinate.
    #[inline]
    pub fn z(&self) -> u64 { (self.lo64() >> 19) & ((1 << 19) - 1) }
    /// Bits 38..45 of the packed int64 — unknown 7-bit field.
    #[inline]
    pub fn u3(&self) -> u64 { (self.lo64() >> 38) & ((1 << 7) - 1) }
    /// Bits 45..64 of the packed int64 — X coordinate.
    #[inline]
    pub fn x(&self) -> u64 { (self.lo64() >> 45) & ((1 << 19) - 1) }
    /// High 12 bits of the trailing u16 — heading. The 4 unused bits lead;
    /// upstream declares them trailing, which reads the facing 4 bits low.
    #[inline]
    pub fn heading(&self) -> u64 { ((self.hi16() >> 4) & 0xFFF) as u64 }
    /// High 4 bits of the trailing u16 — signed 4-bit unused field.
    #[inline]
    pub fn unused2(&self) -> i64 {
        let v = ((self.hi16() >> 12) & 0xF) as i64;
        if v & 0x8 != 0 { v - 0x10 } else { v }
    }
}

#[cfg(test)]
mod __layout_tests {
    use super::*;
    #[test] fn spawnPositionUpdate_size() { assert_eq!(core::mem::size_of::<spawnPositionUpdate>(), 18); }
    #[test] fn deleteSpawnStruct_size() { assert_eq!(core::mem::size_of::<deleteSpawnStruct>(), 4); }
    #[test] fn removeSpawnStruct_size() { assert_eq!(core::mem::size_of::<removeSpawnStruct>(), 5); }
    #[test] fn hpNpcUpdateStruct_size() { assert_eq!(core::mem::size_of::<hpNpcUpdateStruct>(), 18); }
    #[test] fn mobHealthStruct_size() { assert_eq!(core::mem::size_of::<mobHealthStruct>(), 6); }
    #[test] fn spawnAppearanceStruct_size() { assert_eq!(core::mem::size_of::<spawnAppearanceStruct>(), 8); }
    #[test] fn expUpdateStruct_size() { assert_eq!(core::mem::size_of::<expUpdateStruct>(), 16); }
    #[test] fn levelUpUpdateStruct_size() { assert_eq!(core::mem::size_of::<levelUpUpdateStruct>(), 24); }
    #[test] fn skillIncStruct_size() { assert_eq!(core::mem::size_of::<skillIncStruct>(), 12); }
    #[test] fn manaDecrementStruct_size() { assert_eq!(core::mem::size_of::<manaDecrementStruct>(), 20); }
    #[test] fn staminaStruct_size() { assert_eq!(core::mem::size_of::<staminaStruct>(), 8); }
    #[test] fn endUpdateStruct_size() { assert_eq!(core::mem::size_of::<endUpdateStruct>(), 10); }
    #[test] fn considerStruct_size() { assert_eq!(core::mem::size_of::<considerStruct>(), 24); }
    #[test] fn spawnRenameStruct_size() { assert_eq!(core::mem::size_of::<spawnRenameStruct>(), 195); }
    #[test] fn clientTargetStruct_size() { assert_eq!(core::mem::size_of::<clientTargetStruct>(), 4); }
    #[test] fn newCorpseStruct_size() { assert_eq!(core::mem::size_of::<newCorpseStruct>(), 40); }
    #[test] fn remDropStruct_size() { assert_eq!(core::mem::size_of::<remDropStruct>(), 12); }
    #[test] fn spawnIllusionStruct_size() { assert_eq!(core::mem::size_of::<spawnIllusionStruct>(), 332); }
    #[test] fn buffStruct_size() { assert_eq!(core::mem::size_of::<buffStruct>(), 168); }
    #[test] fn action2Struct_size() { assert_eq!(core::mem::size_of::<action2Struct>(), 48); }
    #[test] fn SpawnUpdateStruct_size() { assert_eq!(core::mem::size_of::<SpawnUpdateStruct>(), 32); }
    #[test] fn zoneChangeStruct_size() { assert_eq!(core::mem::size_of::<zoneChangeStruct>(), 100); }
    #[test] fn dzInfo_size() { assert_eq!(core::mem::size_of::<dzInfo>(), 212); }
    #[test] fn dzSwitchInfo_size() { assert_eq!(core::mem::size_of::<dzSwitchInfo>(), 32); }
    #[test] fn startCastStruct_size() { assert_eq!(core::mem::size_of::<startCastStruct>(), 44); }  // eql-diverged; Live is 39
    #[test] fn actionStruct_size() { assert_eq!(core::mem::size_of::<actionStruct>(), 64); }
    #[test] fn actionAltStruct_size() { assert_eq!(core::mem::size_of::<actionAltStruct>(), 88); }
    #[test] fn groupDisbandStruct_size() { assert_eq!(core::mem::size_of::<groupDisbandStruct>(), 152); }
    #[test] fn corpseLocStruct_size() { assert_eq!(core::mem::size_of::<corpseLocStruct>(), 16); }
    #[test] fn doorStruct_size() { assert_eq!(core::mem::size_of::<doorStruct>(), 132); }  // eql-diverged; Live is 136
    #[test] fn zonePointStruct_size() { assert_eq!(core::mem::size_of::<zonePointStruct>(), 24); }
    #[test] fn simpleMessageStruct_size() { assert_eq!(core::mem::size_of::<simpleMessageStruct>(), 12); }
    #[test] fn formattedMessageStruct_size() { assert_eq!(core::mem::size_of::<formattedMessageStruct>(), 13); }
}
