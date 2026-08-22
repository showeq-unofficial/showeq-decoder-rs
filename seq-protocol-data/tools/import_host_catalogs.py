#!/usr/bin/env python3
"""Import or compare the transitional scry-cpp opcode catalogs.

Only mapped IDs and stable names cross the boundary. C++ payload type and size
rules remain host compatibility data and never enter the Rust catalog.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
import tomllib


BACKENDS = {
    "live": Path("opcodes.toml"),
    "test": Path("test/opcodes.toml"),
    "eql": Path("eql/opcodes.toml"),
}


def load(path: Path) -> dict[str, list[tuple[int, str]]]:
    with path.open("rb") as stream:
        parsed = tomllib.load(stream)
    result: dict[str, list[tuple[int, str]]] = {}
    for stream in ("world", "zone"):
        entries = []
        for row in parsed.get(stream, []):
            opcode = int(row["id"].removeprefix("0x"), 16)
            if opcode != 0xFFFF:
                entries.append((opcode, row["name"].strip()))
        result[stream] = sorted(entries)
    return result


def render(catalog: dict[str, list[tuple[int, str]]]) -> str:
    lines = [
        "# Canonical Rust opcode lookup data.",
        "# Generated with tools/import_host_catalogs.py during host migration.",
        "# Payload typename and size gates intentionally stay in the host.",
        "",
    ]
    for stream in ("world", "zone"):
        for opcode, name in catalog[stream]:
            lines.extend(
                [
                    f"[[{stream}]]",
                    f'id = "{opcode:04x}"',
                    f'name = "{name}"',
                    "",
                ]
            )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--update", action="store_true")
    parser.add_argument("host_conf", type=Path)
    args = parser.parse_args()

    data_dir = Path(__file__).resolve().parents[1] / "data"
    changed = []
    for backend, relative in BACKENDS.items():
        expected = render(load(args.host_conf / relative))
        destination = data_dir / f"{backend}.toml"
        if args.update:
            destination.write_text(expected, encoding="utf-8")
        elif not destination.exists() or destination.read_text(encoding="utf-8") != expected:
            changed.append(backend)

    if changed:
        print("Rust opcode catalogs differ from scry-cpp: " + ", ".join(changed), file=sys.stderr)
        print(
            "Review the host changes, then run this command with --update and commit the Rust data.",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
