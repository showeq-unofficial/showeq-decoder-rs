# Protocol catalog maintenance

The TOML files under `data/` are the Rust-owned opcode lookup catalogs. They
contain only mapped IDs and stable opcode names, split into world and zone
streams. Rows may also carry the diagnostic-only `priority`, `priority_note`,
`updated`, and `comment` fields. Unknown sections and row fields are rejected.
C++ payload type and size gates are not parser rules here and the Rust loader
rejects them.

During the host migration, compare payload-rich legacy scry-cpp files by their
semantic ID/name mappings with:

```sh
python3 seq-protocol-data/tools/import_host_catalogs.py --check ../scry-cpp/conf
```

Patch-day edits begin in `data/`, never in a host mirror. Generate minimal
compatibility mirrors into a packaging or staging directory with:

```sh
python3 seq-protocol-data/tools/import_host_catalogs.py --generate path/to/mirror/conf
```

`--generate` writes `opcodes.toml`, `test/opcodes.toml`, and
`eql/opcodes.toml` under the target. Do not point it at payload-rich legacy
files that still need host-only gates; use `--check` for those files and update
their semantic rows from the reviewed Rust change. The tool has no mode that
writes `data/`.

`ProtocolRegistry::from_directory` loads the same semantic schema and path
layout as `data/`. It does not load old payload-rich C++ configuration files.

The current semantic SHA-256 hashes are:

- live: `8ba647ade8da0f99bea459e82dd6b47b2a0694b8ec131275d91536084c62577e`
- test: `0180d6c6c7baad7cc480a96723b99f962c08f9c72ea3998fd22873fc53c12fe2`
- eql: `e6f2f9fd46cc2f73d1f508abe217a93f0af0859195e75e1ea04d1c8125170044`

The hash covers backend, stream, numeric ID, and opcode name in sorted order.
It ignores TOML formatting and comments, which makes it suitable for host
packaging parity checks.
