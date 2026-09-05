import tempfile
import unittest
from pathlib import Path
from unittest import mock

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

    def test_event_without_projection_contract_rejects_rust_projection(self) -> None:
        path = self.write_host_declaration(
            {
                'projection = "missing"': 'projection = "rust"',
                'state = "missing"': 'state = "not_applicable"',
                'persistence = "missing"': 'persistence = "not_applicable"',
                'name = "SessionReset"\nprojection = "not_applicable"': (
                    'name = "SessionReset"\nprojection = "rust"'
                ),
            }
        )
        with self.assertRaisesRegex(
            event_coverage.CoverageError,
            "has no standalone projection contract",
        ):
            event_coverage.check_host(path, strict=True)

    def test_projection_contract_defaults_and_explicit_exception(self) -> None:
        policy = event_coverage.metadata()
        self.assertTrue(policy["PlayerMoved"]["projection_required"])
        self.assertFalse(policy["SessionReset"]["projection_required"])
        self.assertTrue(
            policy["SessionReset"]["projection_not_applicable_reason"]
        )
        self.assertFalse(policy["SpawnKilled"]["projection_required"])

    def test_projection_exception_requires_a_reason(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        path = Path(directory.name) / "event-metadata.toml"
        text = event_coverage.METADATA_SOURCE.read_text().replace(
            'projection_not_applicable_reason = "Session reset controls host state and has no standalone seq.v1 payload."\n',
            "",
            1,
        )
        path.write_text(text)
        with mock.patch.object(event_coverage, "METADATA_SOURCE", path):
            with self.assertRaisesRegex(
                event_coverage.CoverageError,
                "needs a projection_not_applicable_reason",
            ):
                event_coverage.metadata()


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
