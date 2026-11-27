"""Run the versioned gameplay scenario suite and report its results."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import NamedTuple

ROOT = Path(__file__).resolve().parents[1]
SUITE_VERSION = 1


class Scenario(NamedTuple):
    id: str
    description: str
    test: str | None
    requires_game_data: bool = False


SCENARIOS = (
    Scenario("lap-timing", "Three laps, best lap and finish time",
             "race::tests::lap_times_and_best_lap_follow_the_line_crossings"),
    Scenario("lapped-opponent", "A lapped opponent still owes the full race distance",
             "race::tests::lapped_cars_must_complete_the_full_distance"),
    Scenario("timed-race", "A timed race ends at the limit and ranks by progress",
             "race::tests::a_time_limited_race_ends_at_the_limit_and_ranks_by_progress"),
    Scenario("flashback-limit", "Jumping in uses one flashback until the allowance runs out",
             "flashback::tests::each_jump_in_uses_one_until_none_remain"),
    Scenario("drift-scoring", "A drift accumulates score, changes rating and banks points",
             "drift::tests::a_single_drift_scores_steps_ratings_and_banks_after_the_combo_time"),
    Scenario("career-payout", "An event pays prize money, reputation and sponsors",
             "career::tests::an_event_pays_prize_reputation_sponsors_and_drains_the_pot"),
    Scenario("bmw-tour-grid", "BMW Si Tour layout order, lap counts and starting field",
             "career::tests::bmw_si_tour_draw_and_field_follow_the_original", True),
    Scenario("le-mans-payout", "Le Mans pays by class out of the event prize pool",
             "career::tests::routine_b_pays_the_le_mans_24_hours_by_class_from_the_whole_pot", True),
    Scenario("rookie-career", "Driver offers lead to forming a team with the shipped career tables",
             "career::tests::a_rookie_career_runs_on_the_shipped_data", True),
    Scenario("drift-settings", "The shipped drift settings match the decoded values",
             "drift::tests::shipped_drift_xml_matches_the_decoded_values", True),
    Scenario("traffic-jarama", "Jarama route 0, quick seed 22: 5,000 ticks with no soak failure",
             None, True),
    Scenario("donington-race", "Donington route 0: 12 AI Lacettis, one lap, all cars finish "
             "within 12,000 ticks without physics failure", None, True),
)


def build_harness():
    command = ["cargo", "test", "--locked", "--release", "-p", "grid-tools", "--lib",
               "--no-run", "--message-format=json-render-diagnostics"]
    build = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, text=True,
                           check=True, timeout=900)
    executables = []
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if (message["reason"] == "compiler-artifact" and message.get("executable")
                and message["target"]["name"] == "grid_tools" and message["profile"]["test"]):
            executables.append(message["executable"])
    if len(executables) != 1:
        raise ValueError("expected exactly one grid-tools unit test executable")
    return executables[0]


def run_scenario(executable, scenario, game_root):
    if scenario.test is None or (scenario.requires_game_data and game_root is None):
        return "not_run"

    command = [executable, "--exact", scenario.test, "--color", "never", "--format", "pretty"]
    env = os.environ.copy()
    env.pop("GRID_GAME_ROOT", None)
    if scenario.requires_game_data:
        env["GRID_GAME_ROOT"] = str(game_root)
        command.append("--ignored")
    try:
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True,
                                timeout=120)
    except subprocess.TimeoutExpired:
        print(f"{scenario.id}: timed out", file=sys.stderr)
        return "failed"

    if result.returncode == 0 and f"test {scenario.test} ... ok" in result.stdout.splitlines():
        return "passed"
    print(f"{scenario.id}: {scenario.test} did not pass", file=sys.stderr)
    print(result.stdout + result.stderr, file=sys.stderr)
    return "failed"


def counts(results):
    total = len(results)
    passed = sum(case["status"] == "passed" for case in results)
    return {
        "total": total,
        "passed": passed,
        "failed": sum(case["status"] == "failed" for case in results),
        "not_run": sum(case["status"] == "not_run" for case in results),
        "percent": round(100 * passed / total, 2),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-root", type=Path, help="include checks requiring a local copy of GRID")
    parser.add_argument("--json", type=Path, help="write a machine-readable copy of the report")
    args = parser.parse_args()
    if args.game_root is not None and not args.game_root.is_dir():
        parser.error("--game-root must point to the original GRID data directory")
    game_root = args.game_root.resolve() if args.game_root is not None else None

    try:
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        executable = build_harness()
        results = [
            {**case._asdict(), "status": run_scenario(executable, case, game_root)}
            for case in SCENARIOS
        ]
        report = {
            "metric": "fixed_gameplay_scenarios",
            "suite_version": SUITE_VERSION,
            "revision": revision,
            **counts(results),
            "without_game_data": counts([case for case in results if not case["requires_game_data"]]),
            "with_game_data": counts([case for case in results if case["requires_game_data"]]),
            "scenarios": results,
        }
        if args.json:
            args.json.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")

    print(f"## Fixed gameplay scenarios v{SUITE_VERSION}\n")
    print(f"Commit: `{revision}`\n")
    print("| Scope | Passed | Failed | Not run | Total | Verified |")
    print("| --- | ---: | ---: | ---: | ---: | ---: |")
    for label, row in (
        ("All scenarios", report),
        ("Without game data", report["without_game_data"]),
        ("With game data", report["with_game_data"]),
    ):
        print(f"| {label} | {row['passed']} | {row['failed']} | {row['not_run']} | "
              f"{row['total']} | {row['percent']:.2f}% |")
    print("\n| Scenario | Check | Result |")
    print("| --- | --- | --- |")
    for case in results:
        status = case["status"].replace("_", " ")
        if case["status"] == "not_run":
            status += " (no automated check)" if case["test"] is None else " (needs game data)"
        print(f"| `{case['id']}` | {case['description']} | {status} |")
    print("\nVerified percentage is passed scenarios divided by all scenarios in the group.")
    return int(report["failed"] > 0)


if __name__ == "__main__":
    sys.exit(main())
