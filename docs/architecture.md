# Architecture

How the workspace is put together and why. For commands and
behavior-changing gotchas, see [`../CLAUDE.md`](../CLAUDE.md).

## Two consumers, two surfaces

This workspace decodes EverQuest wire bytes for **two different daemons**,
and each gets its own FFI surface — they don't share one:

```
                         seq-decode (Live/Test parsers)
                        /                              \
   seq-structs-{live,test}                        seq-backend-live
   (generated struct mirrors)                      (wraps into Event)
              \                                          |
               seq-bridge (cxx)                     seq-events
              /    |                                     |
   backend-live/test   backend-eql                seq-backend-eql
              \         |                            /        \
               \        +---- (direct parse_* calls, no Event) /
                \                                              /
            scry-cpp (C++, via decode_*)          scry's native/scry_nif
                                                    (Elixir NIF, via Backend trait)
```

- **`seq-bridge`** is a `cxx` staticlib consumed by `scry-cpp`. Its
  `backend-live`/`backend-test` Cargo features link `seq-decode` (which
  `#[cfg]`-selects `seq-structs-live` or `seq-structs-test` as its binding
  crate); the `backend-eql` decoder edge links only `seq-backend-eql`, calling
  its `parse_*` functions directly. The session path also links the neutral
  `seq-events`, `seq-protocol-data`, and EQL-only `seq-session` features, but
  it has no `seq-decode` or `seq-backend-live` edge. The `decode_*` FFI names
  are identical across all three features; only the linked implementation
  differs (verify with `cargo tree -p seq-bridge --no-default-features
  --features backend-eql`).
- **`seq-events`** + **`seq-backend-live`** + **`seq-backend-eql`** form the
  neutral contract consumed by scry's Elixir NIF (`scry`'s
  `native/scry_nif`, a separate crate in the `scry` repo that path-deps on
  these three). `seq-events` is pure vocabulary (the `Event` enum, the
  `Backend` trait, `Decoded`) with no wire-decode code at all.
  `seq-backend-live` maps `seq-decode`'s parsers into that vocabulary (and
  also serves Test, since Test's wire is byte-identical to Live today — a
  `seq-backend-test` sibling would fork it if that changes).
  `seq-backend-eql` implements `Backend` directly against its own
  self-contained parsers.

**`seq-backend-eql` is the one crate that serves both surfaces at once** —
`seq-bridge` calls its `parse_*` functions directly (bypassing `Event`
entirely) for the C++ daemon, while it also implements the neutral
`Backend` trait (via its `seq-events` dependency) for the Elixir NIF. Both
call paths land on the exact same parser code, which is the whole point:
one eql decode implementation, not two.

## Stateful session path

`seq-protocol-data` embeds the mapped Live, Test, and EQL opcode catalogs. A
lookup always specifies `BackendId` and `StreamKind`, because world and zone
IDs can collide. Runtime reload parses and validates a complete backend file
before swapping it into `ProtocolRegistry`; a failed read or parse leaves the
previous generation active.

`seq-session` is the new host entry point. It accepts numeric opcodes, records
the protocol generation on every `DecodeBatch`, dispatches to the immutable
backend selected at construction, and owns EQL self and loot trackers. It also
turns backend wire events into player-family events. `PlayerMoved`, partial
`PlayerVitalsUpdated`, `PlayerDied`, and the player identity and appearance
events are final meanings. Other entities use `SpawnHealthUpdated`,
`SpawnIdentityUpdated`, and `SpawnDied`. Zero-valued killer ids become absent
values, and an unresolved player spawn id stays absent. The
existing name-based `Backend::decode`, opcode-specific C++ bridge, and public
standalone tracker APIs remain intact during shadow migration.

For C++, `seq-bridge` exposes `SessionProtocolRegistry` and `SessionResource`
as opaque cxx resources. `SessionDecodeBatch.events` contains ordered
`{SessionEventKind, payload_index}` references. Each Event payload has a typed
vector in the same batch. C++ switches on the tag, indexes the matching vector,
and constructs its `std::variant`; it never interprets a map or opcode name.
`self_stats` and `loot_rows` carry the phase-2 EQL shadow correlator output.

Lifecycle events add their state boundary to the same ordered batch. A
`SessionReset` precedes `EnterWorld`, `PlayerProfile`, or a confirmed Live/Test
`ZoneTransition`. `ZoneChanged` precedes `ZoneEnvironmentChanged` for one
`OP_NewZone`. Parsers validate lifecycle direction, payload size, names, time
ranges, and finite environment values before the session changes correlation
state.

The checked-in protocol TOML contains only stream, ID, and opcode name. See
[`seq-protocol-data/README.md`](../seq-protocol-data/README.md) for the
deterministic transitional drift check against scry-cpp. Host payload typename
and size gates are not copied into Rust protocol data.

## Backend isolation: why eql is a clean break

Rationale: eql is a separate server, and riding Live's decoders meant a
Live-only patch could silently corrupt eql. So **`seq-backend-eql` is fully
self-contained** — it vendors its own copies of every parser, every output
struct, and a PINNED `eqstructs` module (`src/bindings.rs` + `src/eqstructs.rs`,
a frozen fork of the live struct layouts). It depends on `seq-events` (pure
vocabulary, no Live code) and nothing else — no `seq-decode`, no
`seq-structs-live`. A Live wire patch literally cannot reach it (clean-break
refactor, 2026-07-09).

It vendors pinned copies of the ~40 shared parsers too (identical to Live
*today*, byte-verified by the eql tier-2 goldens); when eql and Live
diverge on an opcode, only eql's copy changes — see
[the "rewrite eql's own copy" rule](../CLAUDE.md#conventions) in `CLAUDE.md`.

The bridge's `backend` alias (`use seq_decode as backend` for live/test,
`use seq_backend_eql as backend` for eql) points every `decode_*` at the
active stack. Opcodes whose Legends wire diverges are served by eql's OWN
same-named parsers through that alias — `parse_player_self_pos` /
`parse_spawn` / `parse_consider` / `parse_new_zone` / `parse_player_profile`
/ `parse_player_spawn_pos` all share their Live counterpart's canonical
name, only the implementation differs (the old `parse_legends_*` /
`parse_zone_spawn` names were dropped — do not reintroduce them). eql-only
functions with no Live twin (`parse_stat_sync`, `size_overrides`) are called
explicitly via `seq_backend_eql::` under `#[cfg(feature = "backend-eql")]`.

## Struct-mirror codegen

`seq-structs-{live,test}/src/bindings.rs` are generated and committed —
regenerate after any `everquest.h` struct change with
`python3 tools/gen_eqstructs.py all` (or `live`/`test` for one; defaults to
`live` from the sibling `../scry-cpp` checkout). No libclang / bindgen
dependency (see [`../BINDGEN_REMOVAL.md`](../BINDGEN_REMOVAL.md)) — the
script trusts `everquest.h`'s per-field declarations and warns on
disagreement with any stale trailing `/*offset*/` marker.

**eql is NOT generated at all** (2026-08-03):
`seq-backend-eql/src/bindings.rs` is hand-maintained eql-owned source — edit
it directly; there is no `eql` codegen target, the script errors if asked
for one. Rationale: the only header the script could read is Live's, so
generating eql would re-import Live's layouts — exactly the coupling the
clean break above removed — and eql's wire has since diverged from any
header that exists (`startCastStruct` 44B vs. Live's 39B, `considerStruct`
24B vs. 32B, plus bitfield/variable-length records that were never
generatable). The file had also gone unregenerated since the clean break,
so its old `@generated / DO NOT EDIT` banner asserted a guarantee nothing
enforced *and* blocked the correct in-place fix when a struct diverged —
which is how a stale 39-byte `startCastStruct` silently size-dropped every
`OP_CastSpell`. When eql's wire diverges: change the struct in
`bindings.rs` directly and update its assertion in `__layout_tests`. Prefer
`size_of::<Struct>()` for `PAYLOAD_LEN` over a literal (28 of the 57 eql
parsers do) — literals belong only to genuinely unmodellable records
(bitfield packs, variable-length walks).

`gen_eqstructs.py` cannot parse multi-field bitfield groups on one offset
line (e.g. `pitch:12, y:19, padding:1`) — it raises `ValueError`. For
bitfield-laden structs, hand-roll the parser in `seq-decode` directly on
`&[u8]` without a `seq-structs` binding (see `player_self_pos.rs`,
`player_spawn_pos.rs`). `spawnPositionUpdate` is the one codegen
special-case: its bitfield pack can't be parsed generically, so
`parse_spawn_position_update()` and the `impl spawnPositionUpdate` accessor
block in `emit_rust()` are updated by hand when that struct's layout
changes.

## Bitfield conventions

Two distinct bitfield conventions live in the wire formats, and they don't
share extraction code — pick the right one per opcode:

- **(a) C-struct `#[repr(C, packed)]` LSB-first** within each storage unit
  — what `spawnPositionUpdate` and the `playerSelfPosStruct`/
  `playerSpawnPosStruct` parsers use.
- **(b) The legacy `BitStream` MSB-first sign-magnitude packing** from
  `netstream.cpp` — used by `OP_NpcMoveUpdate`, ported to
  `npc_move_update.rs`.

**Position parsers name their fields in the MAP frame, always, every
backend.** Upstream's legends branch names EQL position bitfields in the
WIRE frame and transposes x/y at the call site
(`spawnPositionUpdateEQL` + `SpawnShell::updateSpawns`) — porting its
struct labels without that swap silently transposes the axes (how
`OP_MobUpdate` broke between 07/28 and 08/03). Read the consumer, not just
`everquest.h`, and convert to the map frame inside the parser; the
`decode_*` FFI contract is uniform across backends and must not shift
meaning per target.

Deriving a new eql (or post-patch) parser: if per-axis scale factors look
arbitrary (÷8 / ÷64 / unscaled) or a value wraps at a power of two, you're
reading truncated windows of a packed bitfield — check against Live's
packed layouts (19-bit ×8 coords in `spawnPositionUpdate` / `spawnStruct`
position words) before inventing byte offsets. Decisive cheap test:
sign-fill — the bits above a signed bitfield must equal its sign bit across
every captured packet; float/int32 scans cannot see byte-straddling
bitfields, so they prove nothing about their absence. eql default: try the
shared Live decoder FIRST — `OP_NpcMoveUpdate`, `OP_MobUpdate`,
`OP_TargetMouse`, and `OP_EnterWorld` all proved byte-identical to Live; an
offset parser in `seq-backend-eql` is the fallback, not the starting point.

## Self-identity tracking (`seq-backend-eql::SelfTracker`)

Self-identity has three tiers, and only `SelfTracker` (`self_track.rs`) ranks
them. `seq-session` applies its verdicts. Hosts receive final player events.

1. **`observe_spawn`** — a name-match on a self-named `OP_ZoneEntry` is
   authoritative and adopts the LIVE copy.
2. **`observe_self_pos`** — the id the client stamps on its own outbound
   `OP_ClientUpdate` (C>S, so ownership is proven by direction: the server
   never echoes your position back).
3. A wide stat-sync carrying mana or endurance, which ride that channel for
   the player ONLY.

Tiers 2 and 3 are provisional and handle mid-session attachment, where no
profile or zone-entry burst was observed. A provisional id is the phantom
twin's and never displaces a name match. The session reports the position as
`PlayerMoved { spawn_id: None, ... }` until it resolves the real moving id. It
does not create a synthetic spawn. Wide vitals received before the twin record
are held and emitted once the tracker confirms ownership. Zone boundaries and
player death clear every provisional and twin id.

## Entity identity and spatial state

The Rust session keeps a small entity-name index for `OP_SpawnRename`. A rename
gets a spawn id only when exactly one active spawn has the old name. Attachments
that start mid-zone and duplicate names produce `id: None`; they never invent a
sentinel id. Lifecycle resets clear the index before later events in the batch.

Doors and ground items keep their server ids in separate namespaces. The shared
events do not offset those ids or represent either object as a spawn. Door
`zonePoint == 0xffff_ffff` becomes `None`. Door, ground-item, corpse, and
zone-point coordinates stay as finite `f32` values. Modern Test zone points
retain their portal/object actor name and leave the absent trigger and
destination ids as `None`. Host projectors own any rounding, name resolution,
and synthetic ids required by seq.v1 compatibility.

Live ground items retain the full actor-definition string and their wire
heading. EQL ground records carry no heading, so the event uses `None` rather
than fabricating zero. A fixed-width host buffer owns its own truncation.

`OP_CorpseLocResponse` has an old PC-coordinate quirk. When the session knows
the entity is a player or player corpse, it swaps the two horizontal wire fields
before emitting `CorpseLocated`. Unknown mid-session corpses retain the parser's
map-frame reading rather than disappearing.

## Why quality gates are per-backend, not workspace-wide

`cargo test --workspace` builds every crate with its DEFAULT features, so
`seq-decode` is only ever compiled against `seq-structs-live`. A
struct-mirror or shared-FFI change can pass `--workspace` clean and still
break the `test` or `eql` link the daemon actually builds — the pairing
`--workspace` never compiles is
`cargo test -p seq-decode --no-default-features --features backend-test`.
CI runs one job per backend, plus a release `seq-bridge` build (release is
what Corrosion links: thin LTO + `codegen-units=1`), plus a standalone job
asserting `seq-decode` never appears in the `backend-eql` dep tree — the
2026-07-09 clean break is a convention nothing in the type system enforces.

`cargo clippy` and `cargo fmt` are both workspace-clean and gated (CI + the
pre-push hook). Clippy runs per-backend too, because the feature-gated FFI
arms in `seq-bridge` only get linted under the feature that enables them.
