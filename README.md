# scry-decoder-rs

The Scry daemon's packet decoder. Rust is the **only** decoder — the C++
daemon (`scry-cpp`, expected as a sibling checkout) links `seq-bridge`
via Corrosion as a hard build dependency; there is no C++ fallback path and
no toggle. Every wire handler in the daemon decodes through the
`seq::rust::decode_*` FFI surface.

## Backends

One decoder workspace serves three build-time backends, selected by Cargo
features on `seq-bridge` (the daemon's `-DSEQ_TARGET=live|test|eql` maps 1:1):

| Feature        | Decode stack                                             |
|----------------|----------------------------------------------------------|
| `backend-live` | `seq-decode` + `seq-structs-live` (default)              |
| `backend-test` | `seq-decode` + `seq-structs-test`                        |
| `backend-eql`  | `seq-backend-eql` only — no `seq-decode` edge            |

The legacy FFI surface is uniform: `decode_*` names are identical for every
backend; the linked feature decides each function's implementation via a
module alias (`use seq_decode as backend` vs `use seq_backend_eql as backend`).
The same feature selects the numeric-opcode `SessionResource`. Exactly one
bridge backend feature must be enabled (`compile_error!` otherwise).

## Workspace

| Crate               | Purpose                                                    |
|---------------------|------------------------------------------------------------|
| `seq-decode`        | Shared backend-neutral Live parsers — pure `&[u8]` → typed struct, no I/O or global state. Live and Test share these. |
| `seq-structs-live`  | Generated Rust mirrors of `scry-cpp/src/backend/live/everquest.h` (via `tools/gen_eqstructs.py`, committed). |
| `seq-structs-test`  | Same for `backend/test/everquest.h`. Byte-identical to live today; forks when the Test server diverges. |
| `seq-backend-eql`   | Fully self-contained EQ Legends decode stack: vendored copies of the shared parsers, its own diverged parsers, eql-only decoders (stat-sync, buff-list, loadout-swap, UCS chat), a pinned `eqstructs` fork, and `size_overrides()` for the daemon's payload size table. |
| `seq-events`        | Backend-neutral event vocabulary and the existing name-based backend contract. |
| `seq-backend-live`  | Live/Test projection from parser output into `seq-events::Event`. |
| `seq-protocol-data` | Rust-owned Live, Test, and EQL opcode catalogs with stream-qualified lookup, validation, stable hashes, and atomic runtime reload. |
| `seq-session`       | Stateful ID-based backend dispatch. One session owns EQL self and loot correlation for one ordered packet stream. |
| `seq-trace`         | Versioned application-packet traces, deterministic session replay, and exact event goldens. |
| `seq-bridge`        | `cxx` FFI shim (staticlib): retained opcode calls plus opaque protocol/session resources and exhaustive typed Event batches. |

`seq-backend-eql` deliberately depends on nothing from `seq-decode`: eql is a
separate server, and riding Live's decoders meant a Live-only wire patch could
silently corrupt eql decode. The cost is vendored duplication — run
`tools/vendored_drift.py` to see which vendored modules have diverged from
their `seq-decode` twins and decide whether a shared fix should be ported.

New host integrations should construct a shared `ProtocolRegistry` and one
`seq_session::Session` per logical game session. The old name-based decoder
and standalone EQL tracker APIs remain available while hosts run the session
in shadow mode.

## Build & test

```sh
cargo build --workspace
cargo test --workspace
```

The integration check is daemon-side: rebuild the daemon (`cmake -B build`)
and run its `tests/replay/check.sh` tier-2 golden suite for the configured
`SEQ_TARGET`.

## Struct codegen (no bindgen)

`seq-structs-{live,test}/src/bindings.rs` are generated and committed —
regenerate after any `everquest.h` struct change:

```sh
python3 tools/gen_eqstructs.py all      # or `live` / `test`
```

No libclang/bindgen dependency. Layout tests in the generated file guard
sizes. eql's bindings (`seq-backend-eql/src/bindings.rs`) are a **pinned
fork** — `all` never touches them; regenerate on demand only via
`python3 tools/gen_eqstructs.py eql <path/to/everquest.h>`.

Bitfield-packed structs can't be code-generated: `spawnPositionUpdate` is a
hand-maintained special case, and structs like `playerSelfPosStruct` /
`playerSpawnPosStruct` are hand-rolled parsers directly on `&[u8]`.

See `CLAUDE.md` for the working notes (parser-derivation method, eql
gotchas, per-backend rules).

See [`docs/application-packet-traces.md`](docs/application-packet-traces.md)
for the capture-derived trace format and replay commands. The repository does
not contain packet captures.

See [`docs/event-coverage.md`](docs/event-coverage.md) for the exhaustive Event
manifest, host coverage declarations, and the phase 11 deletion inventory.

## License

GPL-2.0 — see [`LICENSE`](LICENSE). Matches legacy ShowEQ and `scry-cpp`.
