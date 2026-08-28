//! PINNED, eql-OWNED Rust mirrors of the EQ wire structs. Forked from
//! `seq-structs-live` at the 2026-07-07 layout so the EQ Legends backend does
//! NOT ride Live's struct bindings — a Live `everquest.h` change (and its
//! `gen_eqstructs.py` regen) must not be able to shift eql's decoders. This
//! `bindings.rs` is a frozen snapshot: `gen_eqstructs.py` does not touch it;
//! edit it by hand only when eql's own wire genuinely diverges.
//!
//! `spawnPositionUpdate`'s bitfield accessors (`y`, `z`, `x`, `heading`)
//! mask but don't sign-extend — the consumer applies [`sign_extend`] as
//! needed. The helper here covers the EQ-specific bit widths we hit.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

include!("bindings.rs");

/// Sign-extend the low `bits` of `v` into an `i32`. Used to recover the
/// sign of EQ's narrow signed bitfields (e.g. position coordinates are
/// 19-bit signed values stored in a packed `int64_t`).
#[inline]
pub fn sign_extend(v: u32, bits: u32) -> i32 {
    debug_assert!(bits > 0 && bits <= 32);
    let shift = 32 - bits;
    ((v << shift) as i32) >> shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_extend_19_bit() {
        assert_eq!(sign_extend(0, 19), 0);
        assert_eq!(sign_extend(1, 19), 1);
        assert_eq!(sign_extend(0x3_FFFF, 19), 0x3_FFFF);
        assert_eq!(sign_extend(0x4_0000, 19), -262_144);
        assert_eq!(sign_extend(0x7_FFFF, 19), -1);
    }

    #[test]
    fn spawn_position_update_layout() {
        // sanity: the pinned struct must match the 18-byte payload the EQ
        // wire format gives us (14 before the 08/25 patch widened unk1).
        assert_eq!(std::mem::size_of::<spawnPositionUpdate>(), 18);
    }

    #[test]
    fn delete_spawn_struct_layout() {
        // 4-byte fixed payload, single u32 spawnId.
        assert_eq!(std::mem::size_of::<deleteSpawnStruct>(), 4);
    }

    #[test]
    fn small_fixed_struct_layouts() {
        // Stage A+3 batch — guards the bindgen-derived sizes against
        // future struct edits in everquest.h. SZC_Match dispatch in
        // the daemon enforces these wire sizes today.
        assert_eq!(std::mem::size_of::<removeSpawnStruct>(), 5);
        assert_eq!(std::mem::size_of::<hpNpcUpdateStruct>(), 18);
        assert_eq!(std::mem::size_of::<mobHealthStruct>(), 6);
        assert_eq!(std::mem::size_of::<spawnAppearanceStruct>(), 8);
        assert_eq!(std::mem::size_of::<expUpdateStruct>(), 16);
        assert_eq!(std::mem::size_of::<levelUpUpdateStruct>(), 24);
        assert_eq!(std::mem::size_of::<skillIncStruct>(), 12);
    }

    #[test]
    fn stage_a4_struct_layouts() {
        assert_eq!(std::mem::size_of::<manaDecrementStruct>(), 20);
        assert_eq!(std::mem::size_of::<staminaStruct>(), 8);
        // endUpdateStruct: 2-byte spawn_id + two u32 — packed without
        // alignment padding (the daemon's struct lays out as 10).
        assert_eq!(std::mem::size_of::<endUpdateStruct>(), 10);
        assert_eq!(std::mem::size_of::<considerStruct>(), 24);
        assert_eq!(std::mem::size_of::<spawnRenameStruct>(), 195);
        assert_eq!(std::mem::size_of::<clientTargetStruct>(), 4);
        assert_eq!(std::mem::size_of::<newCorpseStruct>(), 40);
    }

    #[test]
    fn stage_a5_struct_layouts() {
        assert_eq!(std::mem::size_of::<remDropStruct>(), 12);
        // Struct's trailing /*0336*/ marker is wrong — actual byte
        // sum is 332 (4+64+4+1+1+1+1+4+4+248). bindgen agrees.
        assert_eq!(std::mem::size_of::<spawnIllusionStruct>(), 332);
        assert_eq!(std::mem::size_of::<buffStruct>(), 168);
        assert_eq!(std::mem::size_of::<action2Struct>(), 48);
    }
}
