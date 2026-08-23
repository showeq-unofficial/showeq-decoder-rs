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

Entity events preserve native identities. `SpawnRenamed` carries an optional
spawn id resolved by the ordered session. `Doors` and `GroundItem` never use
spawn-id offsets or fabricated NPC types. Door zone-point sentinel values cross
the bridge as `has_zone_point_id = false`. Modern Test zone-point records carry
an actor name but no trigger or destination ids, so those ids remain optional
across the bridge. EQL ground items likewise use `has_heading = false` because
their wire record carries no heading. `CorpseLocated` and `ZonePoints` carry
float coordinates. A host projector may round them or create compatibility ids
when producing an older public format.

Player-family events carry final session meaning. `PlayerMoved` never exposes
EQL's phantom-twin id. Its `has_spawn_id` flag stays false during a cold attach
until the session finds the real moving spawn. `PlayerVitalsUpdated` is partial;
each resource and each maximum has its own presence flag. `PlayerDied` and
`SpawnDied` use `has_killer_id` instead of asking the host to interpret zero.
`PlayerIdentityUpdated` covers profiles, the real self spawn, and loadout
changes. `PlayerAppearanceUpdated` contains only the race, gender, or animation
fields carried by that packet. Other spawns use `SpawnHealthUpdated`,
`SpawnIdentityUpdated`, and `SpawnDied`.

The old `SelfPos`, `StatSync`, `SpawnHp`, `SpawnKilled`, and `LoadoutSwap`
variants remain in the mechanical bridge because the public name-based backend
API can still produce them. A numeric `SessionResource` translates those wire
events and does not return them to a host.

Loot correlation leaves the numeric session as `LootAcquired` and
`CorpseLootSnapshot`. Both carry the capture timestamp and normalized zone and
corpse context. `LootAcquired.complete` distinguishes a paired acquisition from
an unmatched narration or confirmation closed by `flush`; optional ids and the
optional request sequence use explicit presence flags. The session suppresses
duplicate confirmation sequences and repeated corpse-window items. Low-level
`LootMessage`, `LootTransaction`, and `LootDrops` events, plus `loot_rows`, stay
additive during the host selector cutover. A host must choose one persistence
path rather than write both the semantic event and compatibility row.

Combat-family consumers use `CombatDamage`, `SpellActionResolved`,
`SpellCastStarted`, `SpellCastInterrupted`, `BuffAdded`, `BuffUpdated`, and
`BuffRemoved`. Optional spawn and spell ids cross CXX as presence flags. In
particular, melee damage has `has_spell_id = false`; zero, `0xffff`, and
`0xffffffff` are never semantic spell ids. Unknown positive spell ids remain
numeric so each host can consult its own spell database without changing shared
state. Buff duration stays in server ticks. Spell names, icons, beneficial
flags, and level-scaled durations remain host projection data.

The session pairs an outbound cast request with a server begin-cast broadcast,
tracks replacement and server-message interruptions, and clears unresolved
casts on every flush or lifecycle reset. EQL buff lists are authoritative
snapshots. The session emits removals in prior slot order before additions for
a replacement snapshot. Live and Test variable buff packets emit incremental
add, update, and remove events. Hosts must apply those events in batch order and
must not rebuild buff diffs from the low-level `BuffList` or `BuffWire` values.
The low-level `Combat`, `SpellAction`, `SpellCastRequest`, `SpawnCast`,
`BuffList`, and `BuffWire` variants remain additive until both host family
selectors cut over.

Lifecycle batches have strict ordering. A reset caused by `OP_EnterWorld`, a
profile, or a confirmed Live/Test zone transition precedes the event that
caused it. `OP_NewZone` emits `ZoneChanged` followed by
`ZoneEnvironmentChanged`. The EQL `OP_ZoneChange` request has no destination
and does not reset session state. A malformed lifecycle packet emits no event
and changes no correlation state.

The batch also returns `protocol_generation`, `SessionDisposition`, and the
legacy EQL shadow correlation outputs `self_stats` and `loot_rows`. New code
uses the ordered player and loot events. Call `flush` at shutdown, zone
transition, and replay end. The returned event batch contains incomplete loot
meaning before any reset marker, and the same rows remain in the compatibility
drain.

The selected bridge backend is fixed at build time. `session_new` rejects a
different `SessionBackend`. Existing opcode-specific functions and standalone
EQL trackers remain available during shadow operation.
