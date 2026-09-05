# Application-packet traces

`seq-trace` records the ordered input to `seq_session::Session`. The transport
has already reassembled each record. A trace does not contain Ethernet, IP,
UDP, or reliable-channel framing.

The repository contains synthetic unit tests only. It contains no packet
captures and makes no capture-parity claim.

## Version 1 input

The trace is one UTF-8 JSON document. Readers reject unknown object fields,
unknown enum values, an incorrect format name or version, malformed payload
hex, decreasing timestamps, and catalog hash mismatches.

```json
{
  "format": "seq-app-packet-trace",
  "version": 1,
  "backend": "live",
  "catalog_hash": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "synthetic": true,
  "packets": [
    {
      "stream": "world",
      "opcode_id": 23129,
      "direction": "client_to_server",
      "payload": "00010203",
      "timestamp": 1700000000000
    }
  ]
}
```

`opcode_id` is an unsigned 16-bit JSON number. `payload` is lowercase hex with
two digits per byte and no prefix. `timestamp` is Unix epoch milliseconds.
Array order is packet order, and timestamps must not decrease. Equal
timestamps are valid.

`catalog_hash` is the semantic hash returned by the protocol registry for the
selected backend. Start a new trace if a catalog reload changes that hash. Do
not put packets decoded under two catalog generations in one file.

Set `synthetic` to `true` only for generated packets. Set it to `false` only
when the records came from a capture. A generated fixture must not be described
as capture evidence.

## Building a trace in a host

Rust callers can use `TraceBuilder::for_registry` with an explicit
`TraceOrigin`, then call `TraceBuilder::push`. The builder hex-encodes the
payload and rejects decreasing timestamps.

C++ and Elixir builders should write the same six top-level fields and the five
packet fields shown above. Record packets at the existing application-packet
callback, after reassembly and before the host or Rust decoder handles them.
Use one builder per logical session. Append exactly once for each call that
will reach `Session::decode_at`, including unmapped, malformed, and muted
packets.

The host must snapshot the backend and catalog hash when it opens the trace.
It must close the trace on a catalog reload rather than changing the header.
Write to a temporary file and rename it after the JSON document is complete so
an interrupted process does not leave a fixture that looks valid.

## Scrubbing names and text

Treat the unsanitized capture and first derived trace as sensitive. Keep both
outside the repository. Commit only the reviewed, scrubbed trace.

Scrubbing must not shift bytes. A parser can depend on fixed offsets, NUL
terminators, byte counts, and trailing records. Use these rules:

- Replace printable ASCII bytes in place. Keep the exact byte count and every
  NUL byte. `scrub_ascii_range` does this for a known field range and rejects a
  range containing binary data.
- For UTF-8 text, replace each byte with ASCII unless the packet format has a
  validated length update procedure. Similar-looking Unicode characters often
  use a different number of bytes.
- Do not scrub a guessed byte range. Find the packet struct or parser first.
  Binary ids and lengths can happen to look printable.
- Re-run `seq-trace validate`, replay, and the golden check after every scrub.
  A successful decode does not prove that every sensitive field was found, so
  inspect the payload layout separately.

If a variable-length packet stores a string length, either keep the original
encoded length or update every dependent offset and length from the documented
wire layout. In-place replacement is safer.

## Replay and goldens

```sh
cargo run -p seq-trace -- validate fixture.trace.json
cargo run -p seq-trace -- replay fixture.trace.json -o fixture.golden.json
cargo run -p seq-trace -- check fixture.trace.json fixture.golden.json
```

Use `--catalog-dir PATH` when replaying a historical catalog rather than the
embedded catalogs. The loader still requires its semantic hash to match the
trace.

A Test trace needs a Test-linked build:

```sh
cargo run -p seq-trace --no-default-features --features backend-test -- \
  check fixture.trace.json fixture.golden.json
```

The version 1 golden records one entry for every input packet, even when that
packet emits no events. Each entry contains `packet_index`,
`protocol_generation`, `disposition`, and the exact ordered JSON form of every
`seq_events::Event`. The final `replay_end` flush has its own ordered event
list. `check` reports the first differing JSON pointer and both values.

Event enums use Serde's externally tagged JSON representation. Changing a
variant or field in a way that changes its JSON requires a golden format
version review. Do not silently rewrite existing capture goldens.
