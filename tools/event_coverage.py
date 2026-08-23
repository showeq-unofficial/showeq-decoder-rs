#!/usr/bin/env python3
"""Generate and check the Event contract coverage files.

The Rust Event enum supplies the names and their order. The adjacent TOML file
adds family and internal-only policy without duplicating payload definitions.
This script also checks the mechanical seq-bridge match and inventories the
legacy bridge calls that phase 11 will eventually remove.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVENT_SOURCE = ROOT / "seq-events/src/lib.rs"
METADATA_SOURCE = ROOT / "seq-events/event-metadata.toml"
MANIFEST = ROOT / "seq-events/event-coverage.json"
BRIDGE_SOURCE = ROOT / "seq-bridge/src/lib.rs"
INVENTORY = ROOT / "docs/phase11-source-inventory.json"

FAMILIES = {
    "lifecycle",
    "entities_spatial",
    "player",
    "items_progression",
    "loot",
    "combat_spells",
    "communication_social",
}
HOST_STATUSES = {"rust", "legacy", "not_applicable", "missing"}


class CoverageError(RuntimeError):
    pass


def mask_rust(source: str) -> str:
    """Mask comments and literals while preserving byte positions."""
    chars = list(source)
    i = 0
    block_depth = 0
    state = "normal"
    while i < len(chars):
        pair = source[i : i + 2]
        if state == "normal":
            if pair == "//":
                chars[i] = chars[i + 1] = " "
                i += 2
                state = "line"
                continue
            if pair == "/*":
                chars[i] = chars[i + 1] = " "
                i += 2
                block_depth = 1
                state = "block"
                continue
            if source[i] == '"':
                chars[i] = " "
                i += 1
                state = "string"
                continue
        elif state == "line":
            if source[i] == "\n":
                state = "normal"
            else:
                chars[i] = " "
            i += 1
            continue
        elif state == "block":
            if pair == "/*":
                chars[i] = chars[i + 1] = " "
                block_depth += 1
                i += 2
                continue
            if pair == "*/":
                chars[i] = chars[i + 1] = " "
                block_depth -= 1
                i += 2
                if block_depth == 0:
                    state = "normal"
                continue
            if source[i] != "\n":
                chars[i] = " "
            i += 1
            continue
        elif state == "string":
            if source[i] == "\\" and i + 1 < len(chars):
                chars[i] = chars[i + 1] = " "
                i += 2
                continue
            if source[i] == '"':
                state = "normal"
            chars[i] = " "
            i += 1
            continue
        i += 1
    return "".join(chars)


def braced_body(source: str, marker: str) -> str:
    masked = mask_rust(source)
    marker_at = masked.find(marker)
    if marker_at < 0:
        raise CoverageError(f"missing Rust item: {marker}")
    opening = masked.find("{", marker_at + len(marker))
    if opening < 0:
        raise CoverageError(f"missing opening brace after: {marker}")
    depth = 1
    i = opening + 1
    while i < len(masked) and depth:
        if masked[i] == "{":
            depth += 1
        elif masked[i] == "}":
            depth -= 1
        i += 1
    if depth:
        raise CoverageError(f"unclosed Rust item: {marker}")
    return masked[opening + 1 : i - 1]


def event_names() -> list[str]:
    body = braced_body(EVENT_SOURCE.read_text(), "pub enum Event")
    chunks: list[str] = []
    start = 0
    braces = parens = brackets = 0
    for i, char in enumerate(body):
        if char == "{":
            braces += 1
        elif char == "}":
            braces -= 1
        elif char == "(":
            parens += 1
        elif char == ")":
            parens -= 1
        elif char == "[":
            brackets += 1
        elif char == "]":
            brackets -= 1
        elif char == "," and braces == parens == brackets == 0:
            chunks.append(body[start:i])
            start = i + 1
    chunks.append(body[start:])

    names = []
    for chunk in chunks:
        match = re.search(r"\b([A-Z][A-Za-z0-9_]*)\b", chunk)
        if match:
            names.append(match.group(1))
    if not names:
        raise CoverageError("Event enum contained no variants")
    duplicates = sorted(name for name, count in Counter(names).items() if count != 1)
    if duplicates:
        raise CoverageError(f"duplicate Event variants: {', '.join(duplicates)}")
    return names


def metadata() -> dict[str, dict[str, object]]:
    raw = tomllib.loads(METADATA_SOURCE.read_text())
    rows = raw.get("events")
    if not isinstance(rows, list):
        raise CoverageError("event-metadata.toml must contain [[events]] rows")
    result: dict[str, dict[str, object]] = {}
    for row in rows:
        name = row.get("name")
        family = row.get("family")
        internal = row.get("internal_only")
        reason = row.get("internal_only_reason")
        if not isinstance(name, str) or not name:
            raise CoverageError("every metadata row needs a non-empty name")
        if name in result:
            raise CoverageError(f"duplicate metadata for Event::{name}")
        if family not in FAMILIES:
            raise CoverageError(f"Event::{name} has invalid family {family!r}")
        if not isinstance(internal, bool):
            raise CoverageError(f"Event::{name} needs boolean internal_only")
        if internal and (not isinstance(reason, str) or not reason.strip()):
            raise CoverageError(f"Event::{name} needs an internal_only_reason")
        if not internal and reason is not None:
            raise CoverageError(
                f"Event::{name} is host-visible but has internal_only_reason"
            )
        result[name] = {
            "family": family,
            "internal_only": internal,
            "internal_only_reason": reason if internal else None,
        }
    return result


def manifest_data() -> dict[str, object]:
    names = event_names()
    policy = metadata()
    missing = [name for name in names if name not in policy]
    extra = sorted(set(policy) - set(names))
    if missing or extra:
        lines = []
        if missing:
            lines.append("missing metadata: " + ", ".join(missing))
        if extra:
            lines.append("unknown metadata: " + ", ".join(extra))
        raise CoverageError("; ".join(lines))
    return {
        "schema_version": 1,
        "contract": "seq_events::Event",
        "source": "seq-events/src/lib.rs",
        "events": [
            {"name": name, **policy[name]}
            for name in names
        ],
    }


def bridge_event_names() -> list[str]:
    body = braced_body(BRIDGE_SOURCE.read_text(), "fn translate_event(")
    return re.findall(r"\bEvent::([A-Z][A-Za-z0-9_]*)", body)


def check_bridge(expected: list[str]) -> None:
    actual = bridge_event_names()
    counts = Counter(actual)
    duplicates = sorted(name for name, count in counts.items() if count != 1)
    missing = [name for name in expected if name not in counts]
    extra = sorted(set(actual) - set(expected))
    if duplicates or missing or extra:
        parts = []
        if missing:
            parts.append("missing mappings: " + ", ".join(missing))
        if extra:
            parts.append("unknown mappings: " + ", ".join(extra))
        if duplicates:
            parts.append("non-mechanical mappings: " + ", ".join(duplicates))
        raise CoverageError("seq-bridge translate_event is incomplete: " + "; ".join(parts))


def line_number(source: str, offset: int) -> int:
    return source.count("\n", 0, offset) + 1


def inventory_data() -> dict[str, object]:
    bridge = BRIDGE_SOURCE.read_text()
    bridge_masked = mask_rust(bridge)
    entrypoints = []
    extern_body = braced_body(bridge, "extern")
    decoder_names = sorted(
        set(re.findall(r"\bfn\s+(decode_[a-z0-9_]+)\s*\(", extern_body))
        - {"decode_ucs"}
    )
    for name in decoder_names:
        match = re.search(rf"\bfn\s+{name}\s*\(", bridge_masked)
        if match is None:
            raise CoverageError(f"missing bridge declaration for {name}")
        entrypoints.append(
            {
                "name": name,
                "declaration": "seq-bridge/src/lib.rs",
                "line": line_number(bridge, match.start()),
            }
        )

    tracker_specs = {
        "EqlSelfTracker": (
            "seq-backend-eql/src/self_track.rs",
            "type EqlLootTracker;",
        ),
        "EqlLootTracker": (
            "seq-backend-eql/src/loot_track.rs",
            "fn decode_loadout_swap(",
        ),
    }
    trackers = []
    for name, (implementation, end_marker) in tracker_specs.items():
        marker = re.search(rf"\btype\s+{name}\s*;", bridge_masked)
        if marker is None:
            raise CoverageError(f"missing standalone bridge tracker {name}")
        end = bridge_masked.find(end_marker, marker.end())
        if end < 0:
            raise CoverageError(f"missing end marker for standalone bridge tracker {name}")
        methods = re.findall(r"\bfn\s+([a-z][a-z0-9_]*)\s*\(", bridge_masked[marker.end() : end])
        trackers.append(
            {
                "name": name,
                "bridge_declaration": "seq-bridge/src/lib.rs",
                "line": line_number(bridge, marker.start()),
                "implementation": implementation,
                "methods": methods,
            }
        )

    modules = []
    for crate, backend in (
        ("seq-decode", "live_test"),
        ("seq-backend-eql", "eql"),
    ):
        lib_path = ROOT / crate / "src/lib.rs"
        source = mask_rust(lib_path.read_text())
        for name in re.findall(r"^pub\s+mod\s+([a-z][a-z0-9_]*)\s*;", source, re.MULTILINE):
            path = ROOT / crate / f"src/{name}.rs"
            modules.append(
                {
                    "backend": backend,
                    "module": name,
                    "source": str(path.relative_to(ROOT)),
                }
            )
    modules.sort(key=lambda row: (str(row["backend"]), str(row["module"])))
    return {
        "schema_version": 1,
        "purpose": "Phase 11 deletion inventory. Presence does not authorize deletion before host parity.",
        "legacy_bridge_decoder_entrypoints": entrypoints,
        "standalone_bridge_trackers": trackers,
        "backend_decoder_and_support_modules": modules,
    }


def json_text(value: object) -> str:
    return json.dumps(value, indent=2, ensure_ascii=False) + "\n"


def check_generated(path: Path, expected: object) -> None:
    actual = path.read_text() if path.exists() else ""
    wanted = json_text(expected)
    if actual != wanted:
        raise CoverageError(f"{path.relative_to(ROOT)} is stale; run generate")


def write_generated(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json_text(value))


def host_template(host: str, output: Path) -> None:
    data = manifest_data()
    lines = [
        "schema_version = 1",
        f'host = "{host}"',
        'event_manifest = "seq-events/event-coverage.json"',
        "",
        "# Replace missing values as each host path is audited.",
        "# Valid values: rust, legacy, not_applicable, missing.",
    ]
    for event in data["events"]:
        lines.extend(
            [
                "",
                "[[events]]",
                f'name = "{event["name"]}"',
                'projection = "not_applicable"' if event["internal_only"] else 'projection = "missing"',
                'state = "not_applicable"' if event["internal_only"] else 'state = "missing"',
                'persistence = "not_applicable"' if event["internal_only"] else 'persistence = "missing"',
                'notes = "Internal-only by contract."' if event["internal_only"] else 'notes = ""',
            ]
        )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines) + "\n")


def check_host(path: Path, strict: bool) -> None:
    manifest = manifest_data()
    expected = {event["name"]: event for event in manifest["events"]}
    raw = tomllib.loads(path.read_text())
    if raw.get("schema_version") != 1:
        raise CoverageError(f"{path}: unsupported schema_version")
    if not isinstance(raw.get("host"), str) or not raw["host"]:
        raise CoverageError(f"{path}: host must be a non-empty string")
    rows = raw.get("events")
    if not isinstance(rows, list):
        raise CoverageError(f"{path}: missing [[events]] rows")
    seen: set[str] = set()
    unfinished: set[str] = set()
    for row in rows:
        name = row.get("name")
        if name not in expected:
            raise CoverageError(f"{path}: unknown Event::{name}")
        if name in seen:
            raise CoverageError(f"{path}: duplicate Event::{name}")
        seen.add(name)
        statuses = [row.get(field) for field in ("projection", "state", "persistence")]
        if any(status not in HOST_STATUSES for status in statuses):
            raise CoverageError(f"{path}: Event::{name} has an invalid status")
        if expected[name]["internal_only"] and any(
            status != "not_applicable" for status in statuses
        ):
            raise CoverageError(
                f"{path}: internal-only Event::{name} must be not_applicable"
            )
        if strict and any(status in {"legacy", "missing"} for status in statuses):
            unfinished.add(name)
        if strict and not expected[name]["internal_only"] and "rust" not in statuses:
            unfinished.add(name)
    missing = [name for name in expected if name not in seen]
    if missing:
        raise CoverageError(f"{path}: missing Event rows: {', '.join(missing)}")
    if unfinished:
        raise CoverageError(
            f"{path}: strict coverage still has unfinished host-visible paths: "
            + ", ".join(sorted(unfinished))
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("generate")
    subparsers.add_parser("check")
    template = subparsers.add_parser("host-template")
    template.add_argument("host")
    template.add_argument("output", type=Path)
    host_check = subparsers.add_parser("check-host")
    host_check.add_argument("path", type=Path)
    host_check.add_argument("--strict", action="store_true")
    args = parser.parse_args()

    try:
        if args.command == "generate":
            manifest = manifest_data()
            check_bridge([event["name"] for event in manifest["events"]])
            write_generated(MANIFEST, manifest)
            write_generated(INVENTORY, inventory_data())
        elif args.command == "check":
            manifest = manifest_data()
            check_generated(MANIFEST, manifest)
            check_bridge([event["name"] for event in manifest["events"]])
            check_generated(INVENTORY, inventory_data())
        elif args.command == "host-template":
            host_template(args.host, args.output)
        elif args.command == "check-host":
            check_host(args.path, args.strict)
    except (CoverageError, OSError, tomllib.TOMLDecodeError) as error:
        print(f"event coverage: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
