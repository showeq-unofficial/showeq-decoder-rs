# scry-decoder-rs

Rust EverQuest wire-format decoder, shared by two daemons: `scry-cpp` (C++,
via a `cxx` FFI bridge) and `scry` (Elixir, via a neutral event vocabulary
+ NIF). See [`docs/architecture.md`](docs/architecture.md) for how the two
consumer surfaces are built from the same crates and why eql is a clean
break from Live's decode stack.

## Stack

- Rust workspace, edition 2021, MSRV 1.75 (`rust-toolchain.toml`).
- `cxx` for the C++ FFI bridge (`seq-bridge`).
- No libclang/bindgen — struct-mirror codegen is a plain Python script
  parsing `everquest.h` text (see [`BINDGEN_REMOVAL.md`](BINDGEN_REMOVAL.md)).

## Structure

- `seq-decode/` — shared, backend-neutral Live/Test decoders
- `seq-structs-live/`, `seq-structs-test/` — generated struct mirrors of
  `../scry-cpp/src/backend/<t>/everquest.h`
- `seq-bridge/` — the `cxx` C++ FFI surface `scry-cpp` links
- `seq-events/` — neutral `Event` vocabulary + `Backend` trait (no decode
  logic of its own)
- `seq-backend-live/` — wraps `seq-decode`'s parsers into `seq-events`
  types (also serves Test)
- `seq-backend-eql/` — fully self-contained EverQuest Legends decode stack;
  serves both `seq-bridge` (direct `parse_*` calls) and the `Backend` trait
- `tools/gen_eqstructs.py` — struct-mirror codegen
- `scripts/` — misc dev scripts

## Commands

- Build: `cargo build --workspace`
- Test (does NOT cover the `test`/`eql` links — see Gotchas):
  `cargo test --workspace`
- Test the pairing `--workspace` never compiles:
  `cargo test -p seq-decode --no-default-features --features backend-test`
- Lint: `cargo clippy --workspace --all-targets` (per-backend in CI — see
  Gotchas), `cargo fmt --check`
- Regenerate struct bindings after an `everquest.h` change:
  `python3 tools/gen_eqstructs.py all` (or `live`/`test` for one)
- Integration check (daemon-side): rebuild `scry-cpp`
  (`cmake -B build` — Rust is unconditional) and run its
  `tests/replay/check.sh` against tier-2 fixtures.

## Conventions

- Never use an `EQL` suffix in a name we own (opcode or struct); cite
  upstream's `<name>EQLStruct` inside comments only. If upstream's
  EQL-suffixed name collides with a neutral name already in use, pick a
  neutral one from upstream's packet *description*.
- Position parsers name fields in the MAP frame, always, every backend —
  never the wire frame. See
  [`docs/architecture.md`](docs/architecture.md#bitfield-conventions).
- When eql diverges on an opcode that already has a shared `decode_*` FFI,
  **rewrite eql's own parser copy** behind that FFI instead of adding new
  bridge surface — Live's `seq-decode` copy stays untouched, so live/test
  stay byte-identical (regression-free).
- Before reusing any `parse_*`/`decode_*` in eql code, reuse the exact one
  the eql `decode_*` bridge impl calls — never vendor a second copy.
- Doc comments describing wire layout: use a ```` ```text ```` fence rather
  than an indented list (rustdoc's `doc_lazy_continuation`/
  `doc_overindented_list_items` lints fire hard on layout diagrams that look
  like markdown lists).
- Where a literal's ROW STRUCTURE carries wire meaning (one field per row),
  mark it `#[rustfmt::skip]` rather than letting rustfmt fill to the margin
  — see `level_update.rs`.

## Gotchas

- **`cargo test --workspace` is NOT full coverage.** It builds every crate
  with its DEFAULT features, so `seq-decode` is only ever compiled against
  `seq-structs-live`. A struct-mirror or shared-FFI change can pass
  `--workspace` clean and still break the `test` or `eql` link the daemon
  actually builds. Run the `test`-feature pairing from Commands before
  trusting a change.
- **The `seq-bridge` `decode_*` FFI is shared but maps per-backend struct
  fields.** Adding a field to `seq_backend_eql::X` without
  `seq_decode::X` (or vice versa) compiles here and breaks the other
  backend's link in CI (`E0609 no field`). After any change to
  `seq-bridge` or a struct it maps, build all three:
  `cargo build -p seq-bridge`, `--no-default-features --features
  backend-test`, `--no-default-features --features backend-eql`. Live
  carries the field as `0` when it has no wire source (see `class_mask`).
- **`gen_eqstructs.py all` regenerates live+test only.** eql owns a
  pinned `bindings.rs` fork; a struct added to `everquest.h` reaches it
  only via an explicit `gen_eqstructs.py eql <path>`. Structs used purely
  for C++ size-gating stay out of the ALLOWLIST.
- **`spawnStruct` equipment field order differs between wire and memory**
  (wire `[itemId, equip3, equip2, equip1, equip0]` vs `EquipStruct
  {equip0, equip1, equip2, itemId, equip3}`) — assign field-by-field,
  never memcpy.
- **Hand-written variable-length parsers can drift silently on a patch** —
  bindings-sync only covers struct-based parsers.
- **`gen_eqstructs.py` can't parse multi-field bitfield groups** on one
  offset line (e.g. `pitch:12, y:19, padding:1`) — it raises `ValueError`.
  Hand-roll those parsers directly on `&[u8]` instead of adding a
  `seq-structs` binding (see `player_self_pos.rs`).
- **eql gate sizes are backend-owned**: a mapped `SZC_Match` payload with no
  `size_overrides()` entry silently inherits Live's compiled `sizeof`. Fix
  by declaring the size from that parser's own `PAYLOAD_LEN` — never by
  re-pointing the payload at a Live struct name that happens to fit. A
  wrong gate usually hides a second bug: eql's vendored parser still
  reading the pinned Live layout.
- **`seq-backend-eql` must contain exactly ONE `parse_spawn`.** It once
  carried a second, vendored Live-copy `parse_spawn` — running that
  Live-format parser on an eql record silently returned garbage (`ok=true`,
  race ≈ 2.2e9, class 0, no error) — deleted 2026-07-11. Never re-vendor a
  Live-copy of any eql-diverged opcode into `seq-backend-eql`; it would
  compile but silently mis-decode.
- **Stale trailing `/*offset*/` comments in `everquest.h`** (even on
  commented-out C lines) are still parsed by `gen_eqstructs.py` and trip its
  size cross-check — fix `everquest.h`, don't work around it in the script.
- When a clippy lint fires on deliberately explicit wire code, `#[allow]`
  it with a reason rather than taking clippy's rewrite — e.g.
  `mob_update.rs` keeps bit-position literals under
  `#[allow(clippy::identity_op)]` because `<< 0` documents the first
  field's bit position and asserts below cite those ranges.

## Before Committing

- `cargo fmt --check` and `cargo clippy` (workspace lints deny warnings —
  an unclean tree breaks clippy for everyone, not just CI).
- `cargo test -p seq-decode --no-default-features --features backend-test`
  in addition to `--workspace` (see the Gotchas entry above).
- Pre-push hook (`scripts/hooks/pre-push`, `git config core.hooksPath
  scripts/hooks` once per clone) mirrors CI; bypass with `--no-verify`.
- If `everquest.h` changed: regenerate bindings
  (`python3 tools/gen_eqstructs.py all`) before committing — the daemon's
  own pre-push hook checks these are fresh.

## Documentation

- [`docs/architecture.md`](docs/architecture.md) — the two-consumer FFI
  design, backend isolation rationale, struct codegen, bitfield
  conventions, self-identity tracking.
- [`BINDGEN_REMOVAL.md`](BINDGEN_REMOVAL.md) — why codegen is a plain
  Python script instead of bindgen.
- For adding/changing an opcode on the C++ daemon side, see
  `scry-cpp`'s own `CLAUDE.md` and `docs/architecture.md`.
