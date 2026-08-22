#!/usr/bin/env python3
"""Check or generate transitional host mirrors from Rust-owned catalogs.

Rust files under ``seq-protocol-data/data`` are always the source. Check mode
compares only backend/stream/id/name semantics, so it can read legacy C++ files
that still carry payload gates. Generate mode writes minimal semantic mirrors;
it never modifies the Rust catalogs.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
import tomllib


BACKENDS = {
    "live": Path("opcodes.toml"),
    "test": Path("test/opcodes.toml"),
    "eql": Path("eql/opcodes.toml"),
}
STREAMS = ("world", "zone")
RUST_ROW_FIELDS = {"id", "name", "priority", "priority_note", "updated", "comment"}
UNMAPPED_ID = 0xFFFF


Catalog = dict[str, list[tuple[int, str]]]


def parse_id(value: object, path: Path, stream: str) -> int:
    if not isinstance(value, str):
        raise ValueError(f"{path}: {stream} opcode id must be a string")
    text = value.strip()
    if text.startswith("0x"):
        text = text[2:]
    try:
        opcode = int(text, 16)
    except ValueError as error:
        raise ValueError(f"{path}: invalid {stream} opcode id {value!r}") from error
    if not 0 <= opcode <= 0xFFFF:
        raise ValueError(f"{path}: {stream} opcode id {value!r} exceeds u16")
    return opcode


def load(path: Path, *, strict_rust: bool) -> Catalog:
    with path.open("rb") as file:
        parsed = tomllib.load(file)

    if strict_rust:
        unknown_sections = set(parsed) - set(STREAMS)
        if unknown_sections:
            names = ", ".join(sorted(unknown_sections))
            raise ValueError(f"{path}: unknown top-level section(s): {names}")

    result: Catalog = {}
    for stream in STREAMS:
        entries: list[tuple[int, str]] = []
        seen_ids: dict[int, str] = {}
        seen_names: dict[str, int] = {}
        rows = parsed.get(stream, [])
        if not isinstance(rows, list):
            raise ValueError(f"{path}: {stream} must be an array of tables")
        for row in rows:
            if not isinstance(row, dict):
                raise ValueError(f"{path}: {stream} row must be a table")
            if strict_rust:
                unknown_fields = set(row) - RUST_ROW_FIELDS
                if unknown_fields:
                    names = ", ".join(sorted(unknown_fields))
                    raise ValueError(f"{path}: unknown {stream} row field(s): {names}")
                priority = row.get("priority")
                if priority is not None and (
                    not isinstance(priority, int)
                    or isinstance(priority, bool)
                    or not -(2**31) <= priority < 2**31
                ):
                    raise ValueError(f"{path}: {stream} priority must be an i32")
                for field in ("priority_note", "updated", "comment"):
                    value = row.get(field)
                    if value is not None and not isinstance(value, str):
                        raise ValueError(f"{path}: {stream} {field} must be a string")
            try:
                raw_id = row["id"]
                raw_name = row["name"]
            except KeyError as error:
                raise ValueError(f"{path}: {stream} row is missing {error.args[0]!r}") from error
            opcode = parse_id(raw_id, path, stream)
            if not isinstance(raw_name, str) or not raw_name.strip():
                raise ValueError(f"{path}: {stream} opcode {raw_id!r} has an empty name")
            name = raw_name.strip()
            if opcode == UNMAPPED_ID:
                continue
            if opcode in seen_ids:
                raise ValueError(
                    f"{path}: {stream} id {opcode:04x} maps to both "
                    f"{seen_ids[opcode]} and {name}"
                )
            if name in seen_names:
                raise ValueError(
                    f"{path}: {stream} name {name} maps to both "
                    f"{seen_names[name]:04x} and {opcode:04x}"
                )
            seen_ids[opcode] = name
            seen_names[name] = opcode
            entries.append((opcode, name))
        result[stream] = sorted(entries)
    return result


def render(backend: str, catalog: Catalog) -> str:
    lines = [
        "# Generated compatibility mirror. Do not edit.",
        f"# Source: seq-protocol-data/data/{backend}.toml",
        "# Payload typename and size gates belong to the host and are not included.",
        "",
    ]
    for stream in STREAMS:
        for opcode, name in catalog[stream]:
            lines.extend(
                [
                    f"[[{stream}]]",
                    f'id = "{opcode:04x}"',
                    f"name = {json.dumps(name)}",
                    "",
                ]
            )
    return "\n".join(lines)


def check(rust_data: Path, host_conf: Path) -> int:
    changed: list[str] = []
    for backend, relative in BACKENDS.items():
        expected = load(rust_data / f"{backend}.toml", strict_rust=True)
        try:
            actual = load(host_conf / relative, strict_rust=False)
        except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
            print(error, file=sys.stderr)
            changed.append(backend)
            continue
        if actual != expected:
            changed.append(backend)

    if changed:
        print(
            "Host opcode mirrors differ semantically from Rust: " + ", ".join(changed),
            file=sys.stderr,
        )
        print(
            "Update seq-protocol-data/data first, then regenerate or update host compatibility data.",
            file=sys.stderr,
        )
        return 1
    return 0


def generate(rust_data: Path, host_conf: Path) -> int:
    for backend, relative in BACKENDS.items():
        catalog = load(rust_data / f"{backend}.toml", strict_rust=True)
        destination = host_conf / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(render(backend, catalog), encoding="utf-8")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--check",
        action="store_true",
        help="compare a host directory semantically with the Rust catalogs",
    )
    mode.add_argument(
        "--generate",
        action="store_true",
        help="write minimal host mirrors from the Rust catalogs",
    )
    parser.add_argument("host_conf", type=Path)
    args = parser.parse_args()

    rust_data = Path(__file__).resolve().parents[1] / "data"
    try:
        if args.check:
            return check(rust_data, args.host_conf)
        return generate(rust_data, args.host_conf)
    except (OSError, tomllib.TOMLDecodeError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
