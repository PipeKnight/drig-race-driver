"""Regression tests for the fidelity backlog report and its status conventions."""

from pathlib import Path
import runpy
import unittest

TOOL = Path(__file__).resolve().parents[1] / "fidelity_progress.py"
backlog_counts = runpy.run_path(str(TOOL))["backlog_counts"]
HEADER = "| # | area | discrepancy | evidence | impact | needs | size | owner |\n"
SEPARATOR = "|---|---|---|---|---|---|---|---|\n"


def row(number, discrepancy, owner="team"):
    return f"| {number} | rendering | {discrepancy} | F123 | M | O | S | {owner} |\n"


class FidelityProgressTests(unittest.TestCase):
    def test_counts_only_the_numbered_table_and_explicit_closures(self):
        text = (
            "1. A summary mentioning a closed item.\n\n"
            + HEADER + SEPARATOR
            + row(1, "**closed** F123. Fixed.")
            + row(2, "Old discrepancy.", "**closed (F123)**")
            + row(3, "~~Original symptom~~ **closed** F123.")
            + row(4, "~~One symptom~~ fixed. Another is open.")
            + row(5, "**fixed (owner check pending)** F123.")
            + row(6, "**done on Windows**. Linux is pending.")
            + row(7, "**done** F123. Verified.")
            + row(8, r"Escaped \| stays inside the discrepancy.")
            + "\n## Closed in this sweep\n\n- Fixed item 1.\n"
        )
        report = backlog_counts(text)
        self.assertEqual((report["total"], report["open"], report["closed"], report["percent"]),
                         (8, 4, 4, 50.0))

    def test_rejects_duplicate_item(self):
        with self.assertRaisesRegex(ValueError, "duplicate backlog item 1"):
            backlog_counts(HEADER + SEPARATOR + row(1, "One") + row(1, "Two"))

    def test_rejects_missing_or_malformed_table(self):
        with self.assertRaisesRegex(ValueError, "table is missing or empty"):
            backlog_counts(HEADER + SEPARATOR)
        with self.assertRaisesRegex(ValueError, "expected 8 table columns"):
            backlog_counts(HEADER + SEPARATOR + "| 1 | graphics | Incomplete. |\n")
