"""Report closed and open Fidelity Backlog items for the GitHub Actions summary."""

import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
COLUMNS = ("#", "area", "discrepancy", "evidence", "impact", "needs", "size", "owner")
CELL_SEPARATOR = re.compile(r"(?<!\\)\|")
CLOSED = re.compile(r"\*\*(?:closed(?: \([^*]+\))?|done)\*\*", re.IGNORECASE)


def cells(line):
    return tuple(part.strip() for part in CELL_SEPARATOR.split(line[1:-1]))


def backlog_counts(markdown):
    """Read only the numbered table, with each row counted once."""
    rows = {}
    in_table = False
    for number, line in enumerate(markdown.splitlines(), 1):
        line = line.strip()
        if not in_table:
            in_table = line.startswith("|") and line.endswith("|") and cells(line) == COLUMNS
            continue
        if not line.startswith("|"):
            break
        row = cells(line)
        if not line.endswith("|") or len(row) != len(COLUMNS):
            raise ValueError(f"line {number}: expected {len(COLUMNS)} table columns")
        if not rows and all(re.fullmatch(r":?-+:?", part) for part in row):
            continue
        if not re.fullmatch(r"[1-9][0-9]*", row[0]):
            raise ValueError(f"line {number}: expected a positive backlog item number")
        if row[0] in rows:
            raise ValueError(f"line {number}: duplicate backlog item {row[0]}")
        discrepancy = re.sub(r"^~~.*?~~\s*", "", row[2], count=1)
        rows[row[0]] = bool(CLOSED.match(discrepancy) or CLOSED.fullmatch(row[-1]))
    if not rows:
        raise ValueError("numbered fidelity backlog table is missing or empty")
    closed = sum(rows.values())
    return {
        "metric": "fidelity_backlog_completion",
        "total": len(rows),
        "open": len(rows) - closed,
        "closed": closed,
        "percent": round(100 * closed / len(rows), 2),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("backlog", nargs="?", type=Path, default=ROOT / "docs/FIDELITY_BACKLOG.md")
    parser.add_argument("--json", type=Path, help="write a machine-readable copy of the report")
    args = parser.parse_args()
    try:
        counts = backlog_counts(args.backlog.read_text(encoding="utf-8"))
        if args.json:
            args.json.write_text(json.dumps(counts, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError) as error:
        parser.exit(1, f"{args.backlog}: {error}\n")

    print("## Fidelity backlog completion\n")
    print("| Open | Closed | Total | Completion |")
    print("| ---: | ---: | ---: | ---: |")
    print("| {open} | {closed} | {total} | {percent:.2f}% |".format(**counts))
    print("\nCompletion is closed items divided by all numbered Fidelity Backlog items.")
    print("Game parity and matching decompilation require their own measurements.")


if __name__ == "__main__":
    main()
