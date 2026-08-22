# Protocol catalog maintenance

The TOML files under `data/` are the Rust-owned opcode lookup catalogs. They
contain only mapped IDs and stable opcode names, split into world and zone
streams. C++ payload type and size gates are not parser rules here.

During the host migration, compare the old scry-cpp copies with:

```sh
python3 seq-protocol-data/tools/import_host_catalogs.py --check ../scry-cpp/conf
```

After reviewing a patch-day host change, import it deterministically with
`--update`, inspect the diff, and commit the Rust files. Once both hosts load
these catalogs directly, generate any remaining compatibility copies from the
Rust files and reverse the check direction.

The current semantic SHA-256 hashes are:

- live: `8ba647ade8da0f99bea459e82dd6b47b2a0694b8ec131275d91536084c62577e`
- test: `0180d6c6c7baad7cc480a96723b99f962c08f9c72ea3998fd22873fc53c12fe2`
- eql: `e6f2f9fd46cc2f73d1f508abe217a93f0af0859195e75e1ea04d1c8125170044`

The hash covers backend, stream, numeric ID, and opcode name in sorted order.
It ignores TOML formatting and comments, which makes it suitable for host
packaging parity checks.
