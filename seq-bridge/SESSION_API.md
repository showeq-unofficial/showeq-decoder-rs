# C++ session API

`seq-bridge` generates these resources in namespace `seq::rust`:

```cpp
auto protocols = session_protocol_registry_new(protocol_dir);
auto session = session_new(*protocols, SessionBackend::Eql);
auto batch = session->decode(
    SessionStream::Zone,
    opcode_id,
    SessionDirection::ServerToClient,
    payload,
    timestamp);
```

An empty `protocol_dir` selects the embedded catalogs. A non-empty directory
uses the semantic Rust catalog layout: `opcodes.toml`, `test/opcodes.toml`, and
`eql/opcodes.toml`. These files may contain documented diagnostic metadata but
not C++ payload typename or size gates. `SessionProtocolRegistry::reload`
validates a complete backend file before swapping it. It throws a Rust error on
failure and leaves the prior generation active. `content_hash` returns the
semantic SHA-256.

`SessionDecodeBatch.events` preserves Rust event order. Each entry has a
`SessionEventKind` and `payload_index`. Read the index from the typed vector
named for that event. `Stance` and `Invocation` share `named`; `Targeted` and
`Considered` share `spawn_id`. Lifecycle events use `session_reset`,
`zone_transition`, `zone_environment_changed`, and `enter_world`. All other
tags have a same-named payload vector. This is the cxx-compatible
tagged form from which the host can build an exhaustive `std::variant`.

Lifecycle batches have strict ordering. A reset caused by `OP_EnterWorld`, a
profile, or a confirmed Live/Test zone transition precedes the event that
caused it. `OP_NewZone` emits `ZoneChanged` followed by
`ZoneEnvironmentChanged`. The EQL `OP_ZoneChange` request has no destination
and does not reset session state. A malformed lifecycle packet emits no event
and changes no correlation state.

The batch also returns `protocol_generation`, `SessionDisposition`, and the
EQL shadow correlation outputs `self_stats` and `loot_rows`. Call `flush` at
shutdown, zone transition, and replay end, then consume its correlation rows.

The selected bridge backend is fixed at build time. `session_new` rejects a
different `SessionBackend`. Existing opcode-specific functions and standalone
EQL trackers remain available during shadow operation.
