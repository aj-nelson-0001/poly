#!/usr/bin/env python3
"""Tests for the release scripts that edit files or gate the release.

`prepare_release.py` rewrites the wasm size quoted in the preview notes and the
driver refuses to tag when that number disagrees with the built module. Both are
one-off code paths that only run during a release, so a mistake there is found
after the release has already started; these tests exercise them on temp files
instead. Run with `python3 -m unittest discover -s scripts/tests`.
"""

from __future__ import annotations

import contextlib
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))

import prepare_release  # noqa: E402  (import needs the path above)
import release  # noqa: E402


class SyncNotesWasmSizeTest(unittest.TestCase):
    """`sync_notes_wasm_size` rewrites exactly one evidence line."""

    SIZE = 1_234_567

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.notes = root / "POLY_V2_PREVIEW_RELEASE_NOTES.md"
        self.wasm = root / "poly.wasm"
        self.wasm.write_bytes(b"\0" * self.SIZE)
        for name, value in (("RELEASE_NOTES", self.notes), ("PLAYGROUND_WASM", self.wasm)):
            patcher = mock.patch.object(prepare_release, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def run_sync(self, text: str) -> str:
        """Write `text`, run the sync, and return the resulting file body."""
        self.notes.write_text(text, encoding="utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            prepare_release.sync_notes_wasm_size()
        return self.notes.read_text(encoding="utf-8")

    def test_plain_number_is_replaced(self) -> None:
        updated = self.run_sync("evidence: `poly.wasm: 547733 bytes (budget: 660000)`\n")
        self.assertIn(f"poly.wasm: {self.SIZE} bytes (budget: 660000)", updated)
        self.assertNotIn("547733", updated)

    def test_comma_grouping_is_preserved(self) -> None:
        updated = self.run_sync("evidence: `poly.wasm: 547,733 bytes`\n")
        self.assertIn("poly.wasm: 1,234,567 bytes", updated)

    def test_already_correct_number_is_untouched(self) -> None:
        text = f"evidence: `poly.wasm: {self.SIZE} bytes`\n"
        self.notes.write_text(text, encoding="utf-8")
        before = self.notes.stat().st_mtime_ns
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            prepare_release.sync_notes_wasm_size()
        self.assertEqual(self.notes.read_text(encoding="utf-8"), text)
        self.assertIn(f"already {self.SIZE} bytes", output.getvalue())
        # An unchanged file must not be rewritten, so its mtime stays put.
        self.assertEqual(self.notes.stat().st_mtime_ns, before)

    def test_placeholder_is_not_matched(self) -> None:
        # The notes' prose line says `poly.wasm: <n> bytes`; only the real
        # evidence line may be rewritten.
        updated = self.run_sync(
            "The notes quote `poly.wasm: <n> bytes`.\n"
            "evidence: `poly.wasm: 999 bytes`\n"
        )
        self.assertIn("poly.wasm: <n> bytes", updated)
        self.assertIn(f"poly.wasm: {self.SIZE} bytes", updated)

    def test_two_matches_abort(self) -> None:
        with self.assertRaises(SystemExit) as caught:
            self.run_sync("a: poly.wasm: 1 bytes\nb: poly.wasm: 2 bytes\n")
        self.assertIn("expected exactly one", str(caught.exception))

    def test_missing_file_aborts(self) -> None:
        with self.assertRaises(SystemExit) as caught:
            prepare_release.sync_notes_wasm_size()
        self.assertIn("is missing", str(caught.exception))

    def test_only_placeholder_aborts(self) -> None:
        with self.assertRaises(SystemExit) as caught:
            self.run_sync("The notes quote `poly.wasm: <n> bytes`.\n")
        self.assertIn("expected exactly one", str(caught.exception))


class VersionPatternTest(unittest.TestCase):
    """Both scripts locate the workspace version with their own pattern."""

    def test_prepare_release_reads_workspace_version(self) -> None:
        manifest = (
            "[workspace.package]\n"
            'version = "2.0.0-preview.23"\n'
            'edition = "2021"\n'
        )
        match = prepare_release.VERSION_RE.search(manifest)
        self.assertIsNotNone(match)
        self.assertEqual(match.group(2), "2.0.0-preview.23")

    def test_prepare_release_ignores_non_version_lines(self) -> None:
        manifest = 'rust-version = "1.80"\n'
        self.assertIsNone(prepare_release.VERSION_RE.search(manifest))

    def test_driver_accepts_release_versions(self) -> None:
        for version in ("2.0.0-preview.23", "2.0.0", "1.7.3"):
            self.assertIsNotNone(release.VERSION_RE.match(version), version)

    def test_driver_rejects_tags_and_partial_versions(self) -> None:
        for version in ("v2.0.0-preview.23", "2.0", "2.0.0 preview.23", ""):
            self.assertIsNone(release.VERSION_RE.match(version), version)


class DriverArgumentTest(unittest.TestCase):
    """The split wait limits replaced one shared `--timeout` flag."""

    def test_defaults_are_separate_limits(self) -> None:
        self.assertEqual(release.DEFAULT_CI_TIMEOUT_MINUTES, 15)
        self.assertEqual(release.DEFAULT_RELEASE_TIMEOUT_MINUTES, 5)

    def test_help_lists_both_limits_and_no_shared_timeout(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPTS_DIR / "release.py"), "--help"],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertIn("default 15", result.stdout)
        self.assertIn("default 5", result.stdout)
        self.assertIn("--ci-timeout", result.stdout)
        self.assertIn("--release-timeout", result.stdout)
        self.assertNotIn("--timeout", result.stdout)


if __name__ == "__main__":
    unittest.main()
