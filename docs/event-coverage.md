# Event coverage

The hosts can remove packet interpretation only after both account for every
`seq_events::Event` variant. The files in this repository make that audit
mechanical. They do not authorize removal of the legacy bridge or host
fallbacks before capture parity and soak testing.

## Contract files

- `seq-events/src/lib.rs` owns event names and ordering.
- `seq-events/event-metadata.toml` assigns each event to a stable family and
  documents wire-shaped variants that are internal to the migration adapter.
- `seq-events/event-coverage.json` is the generated host-facing manifest.
- `docs/legacy-source-inventory.json` lists the opcode-specific bridge calls,
  standalone EQL trackers, and backend decoder modules still in the tree.

Run the checks after changing the Event enum, the bridge match, or the legacy
decode API:

```sh
python3 tools/event_coverage.py generate
python3 tools/event_coverage.py check
```

`generate` fails if metadata is missing or duplicated. `check` also requires
`seq-bridge::translate_event` to map every event exactly once. Normal Rust
tests repeat the Event and bridge checks, so a new variant cannot enter CI with
an incomplete mechanical adapter.

## Host declarations

Each host keeps its declaration in its own repository. Generate the initial
file after updating that host's decoder pin:

```sh
# In scry-cpp
python3 path/to/scry-decoder-rs/tools/event_coverage.py \
  host-template cpp docs/event-coverage.toml

# In scry
python3 path/to/scry-decoder-rs/tools/event_coverage.py \
  host-template elixir docs/event-coverage.toml
```

Each event declares three independent paths:

- `projection` covers conversion to the public protobuf contract.
- `state` covers application to the host's runtime state.
- `persistence` covers database or durable-record effects.

The allowed values are `rust`, `legacy`, `not_applicable`, and `missing`.
Internal-only events must use `not_applicable` for all three fields. The
manifest supplies their reason. For host-visible events, replace the generated
`missing` values with the current implementation and add a useful note where
the choice is not obvious.

Validate a declaration with:

```sh
python3 path/to/scry-decoder-rs/tools/event_coverage.py \
  check-host docs/event-coverage.toml
```

Use `--strict` before deleting legacy code. Strict mode rejects every `legacy`
and `missing` value. It also requires a Rust-owned projection for every
host-visible event. Only variants documented as internal-only in the Event
manifest may omit projection, so `not_applicable` cannot hide a host projection
gap. Both hosts must pass strict mode, their capture corpus, and their protobuf
projection tests before deleting any source listed in the inventory.

## Reading the deletion inventory

The JSON inventory records source locations instead of guessing whether code
is safe to delete. For example:

```sh
jq '.legacy_bridge_decoder_entrypoints[].name' \
  docs/legacy-source-inventory.json
jq '.standalone_bridge_trackers[]' \
  docs/legacy-source-inventory.json
jq '.legacy_bridge_packet_support_entrypoints[]' \
  docs/legacy-source-inventory.json
```

Regenerate the inventory in the same commit that adds or removes a legacy
entry point. A smaller inventory is evidence of source removal, not evidence
that parity was met. The host declarations and capture results provide that
proof.
