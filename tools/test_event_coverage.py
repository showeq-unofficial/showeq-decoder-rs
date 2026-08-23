import tempfile
import unittest
from pathlib import Path

import event_coverage


class HostCoverageTests(unittest.TestCase):
    def write_host_declaration(
        self, replacements: dict[str, str]
    ) -> Path:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "coverage.toml"
        event_coverage.host_template("test-host", path)
        text = path.read_text()
        for source, replacement in replacements.items():
            text = text.replace(source, replacement)
        path.write_text(text)
        return path

    def test_strict_rejects_not_applicable_host_projection(self) -> None:
        path = self.write_host_declaration(
            {
                'projection = "missing"': 'projection = "not_applicable"',
                'state = "missing"': 'state = "rust"',
                'persistence = "missing"': 'persistence = "not_applicable"',
            }
        )
        with self.assertRaisesRegex(
            event_coverage.CoverageError,
            "strict coverage still has unfinished host-visible paths",
        ):
            event_coverage.check_host(path, strict=True)

    def test_strict_accepts_rust_projection_for_every_host_visible_event(self) -> None:
        path = self.write_host_declaration(
            {
                'projection = "missing"': 'projection = "rust"',
                'state = "missing"': 'state = "not_applicable"',
                'persistence = "missing"': 'persistence = "not_applicable"',
            }
        )
        event_coverage.check_host(path, strict=True)


class SourceInventoryTests(unittest.TestCase):
    def test_packet_support_entrypoints_are_in_deletion_inventory(self) -> None:
        inventory = event_coverage.inventory_data()
        names = {
            row["name"]
            for row in inventory["legacy_bridge_packet_support_entrypoints"]
        }
        self.assertEqual(names, {"struct_size_overrides", "door_stride"})


if __name__ == "__main__":
    unittest.main()
