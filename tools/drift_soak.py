"""Headless Drift Battle soak on every Drift Battle layout: score and AI-quality summary.

Runs target/release/grid-opponent-race --discipline drift_battle once per route of the
original's Drift Battle free-race pool (virtual_race::DRIFT_POOLS, 24 layouts; DRIG loads
those of the supported tracks) and prints one line per route plus the field totals:
laps covered, spins and barrier hits per lap, stalled seconds, drift points that the scorer
earned (estimated zero scores excluded), the best car, cars estimated / finished, failures.
The full output of every run stays in OUT/<route>.txt; OUT/summary.tsv holds the table.
The measure lines (`drift_quality`, `drift car`) are the tool's own (docs/DISCIPLINES.md §4.5).

Usage:
  py -I tools/drift_soak.py OUT [--ticks 16000] [--cars 8] [--field STEM,...] [--jobs 2]
                             [--routes SUBSTR,...] [--exe PATH]
Environment: GRID_GAME_ROOT (default the Steam path); private tables from local/ of this
checkout, else of the main checkout (body-contact-directions.bin, body-impact-parameters.bin,
deformation-parameters.bin, ai-driver-characteristics.tsv), all set as in the windowed race.
Without --field each car keeps the tool's default model order.
"""
import argparse
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROUTES = [
    "tracks/europe/istanbul_park/route_1", "tracks/europe/istanbul_park/route_3",
    "tracks/europe/jarama/route_0", "tracks/europe/jarama/route_1",
    "tracks/europe/nurburgring/route_1", "tracks/europe/nurburgring/route_3",
    "tracks/usa/long_beach/route_1", "tracks/usa/long_beach/route_4",
    "tracks/usa/washington/route_2", "tracks/usa/washington/route_3",
    "tracks/usa/washington/route_7", "tracks/usa/washington/route_8",
    "tracks/japan/okutama/route_2", "tracks/japan/okutama/route_5",
    "tracks/japan/yokohama_docks/route_0", "tracks/japan/yokohama_docks/route_1",
    "tracks/japan/yokohama_docks/route_2", "tracks/japan/yokohama_docks/route_7",
    "tracks/japan/yokohama_docks/route_8", "tracks/japan/yokohama_docks/route_9",
    "tracks/japan/shibuya/route_0", "tracks/japan/shibuya/route_1",
    "tracks/japan/shibuya/route_2", "tracks/japan/shibuya/route_3",
]


def private_dir(repo: Path) -> Path:
    own = repo / "local"
    if (own / "body-contact-directions.bin").is_file():
        return own
    common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
                            cwd=repo, capture_output=True, text=True, check=True).stdout.strip()
    return Path(common).parent / "local"


def parse(text: str) -> dict:
    cars = {}
    for m in re.finditer(r"^drift_quality car(\d+) laps_covered=([\d.]+) spins=(\d+) "
                         r"barrier_hits=(\d+) stalled_s=([\d.]+)", text, re.M):
        cars.setdefault(int(m[1]), {}).update(laps=float(m[2]), spins=int(m[3]),
                                               hits=int(m[4]), stalled=float(m[5]))
    for m in re.finditer(r"^drift car(\d+) score=(-?\d+) estimated=(\w+)", text, re.M):
        cars.setdefault(int(m[1]), {}).update(score=int(m[2]), estimated=m[3] == "true")
    finished = len(re.findall(r"^car\d+ valid_laps=", text, re.M))
    return {"cars": cars, "car_lines": finished}


def run(args, route: str, env: dict, out: Path) -> tuple:
    name = route.removeprefix("tracks/").replace("/", "_")
    cmd = [args.exe, "--route", route, "--discipline", "drift_battle"]
    if args.field:
        cmd += ["--cars", args.field]
    cmd += [env["GRID_GAME_ROOT"], env["CD"], env["BP"], str(args.cars), str(args.ticks), "ai"]
    p = subprocess.run(cmd, capture_output=True, text=True, env=env["env"])
    (out / f"{name}.txt").write_text(p.stdout + "\n--- stderr\n" + p.stderr, encoding="utf-8")
    return name, p.returncode, parse(p.stdout), (p.stderr.strip().splitlines() or [""])[-1][:120]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--ticks", type=int, default=16000)
    ap.add_argument("--cars", type=int, default=8)
    ap.add_argument("--field", default="")
    ap.add_argument("--jobs", type=int, default=2)
    ap.add_argument("--routes", default="")
    ap.add_argument("--exe", default="")
    args = ap.parse_args()
    repo = Path(__file__).resolve().parent.parent
    args.exe = str(Path(args.exe).resolve()) if args.exe else str(repo / "target/release/grid-opponent-race.exe")
    priv = private_dir(repo)
    env = dict(os.environ)
    env["GRID_DEFORMATION_PARAMETERS"] = str(priv / "deformation-parameters.bin")
    env["GRID_AI_CHARACTERISTICS"] = str(priv / "ai-driver-characteristics.tsv")
    ctx = {"env": env, "CD": str(priv / "body-contact-directions.bin"),
           "BP": str(priv / "body-impact-parameters.bin"),
           "GRID_GAME_ROOT": os.environ.get(
               "GRID_GAME_ROOT", r"C:\Program Files (x86)\Steam\steamapps\common\Grid")}
    routes = [r for r in ROUTES
              if not args.routes or any(s in r for s in args.routes.split(","))]
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(lambda r: run(args, r, ctx, out), routes))
    rows = ["route\texit\tlaps\tspins_per_lap\thits_per_lap\tstalled_s\tscored_sum\tbest"
            "\tscored_cars\testimated\tlaps_ge_2"]
    tot = dict(laps=0.0, spins=0, hits=0, stalled=0.0, score=0, n=0, est=0, done=0, cars=0)
    scored_all = []
    for name, code, r, err in results:
        cars = r["cars"].values()
        if code != 0 or not cars:
            rows.append(f"{name}\t{code}\t-\t-\t-\t-\t-\t-\t-\t-\t-\t# {err}")
            continue
        laps = sum(c.get("laps", 0) for c in cars)
        spins = sum(c.get("spins", 0) for c in cars)
        hits = sum(c.get("hits", 0) for c in cars)
        stalled = sum(c.get("stalled", 0) for c in cars)
        scored = [c["score"] for c in cars if "score" in c and not c["estimated"]]
        est = sum(1 for c in cars if c.get("estimated"))
        done = sum(1 for c in cars if c.get("laps", 0) >= 2)
        scored_all += scored
        for k, v in (("laps", laps), ("spins", spins), ("hits", hits), ("stalled", stalled),
                     ("score", sum(scored)), ("n", len(scored)), ("est", est), ("done", done),
                     ("cars", len(cars))):
            tot[k] += v
        per = max(laps, 1e-9)
        rows.append(f"{name}\t0\t{laps:.1f}\t{spins / per:.1f}\t{hits / per:.1f}\t{stalled:.0f}"
                    f"\t{sum(scored)}\t{max(scored, default=0)}\t{len(scored)}\t{est}\t{done}")
    scored_all.sort()
    med = scored_all[len(scored_all) // 2] if scored_all else 0
    per = max(tot["laps"], 1e-9)
    rows.append(f"TOTAL\t-\t{tot['laps']:.1f}\t{tot['spins'] / per:.1f}\t{tot['hits'] / per:.1f}"
                f"\t{tot['stalled']:.0f}\t{tot['score']}\t{max(scored_all, default=0)}"
                f"\t{tot['n']}\t{tot['est']}\t{tot['done']}")
    rows.append(f"# cars={tot['cars']} scored median={med} "
                f"p90={scored_all[int(len(scored_all) * 0.9)] if scored_all else 0}")
    (out / "summary.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")
    print("\n".join(rows))
    print(f"full logs: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
