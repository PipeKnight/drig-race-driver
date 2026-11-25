"""List the worst frame_stats windows of one grid-app log with the render stage that held them and the log events around them.

Usage: py -I tools/frame_dips.py LOG [--top 8]
LOG is a grid-app stdout log with the fps log on (a run folder's stdout.log or a log a player sent). For the
--top windows with the longest frame (max_ms) it prints the window's fps and frame counts, the stages of the
worst render frame (rworst) of 1 ms and over, the camera lead segments (enc=) and the non-tick events logged
since the previous window and at the start of the next one. With GRID_PIPELINE_TRACE=1 it also counts
`pipeline_new` lines after `pipeline_warmup phase=Done` and names the camera view active when each appeared
(drive_camera event=commit lines), i.e. pipelines the warm-up missed (docs/PIPELINE_WARMUP.md 2.5).
Ends with one verdict line: the worst window, its dominant render stage and the missed pipeline count.
"""
import argparse
import re

ANSI = re.compile(r"\x1b\[[0-9;]*m")
STAGES = "render meshes views acquire queue sort prepare bind encode submit present cleanup".split()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("log")
    ap.add_argument("--top", type=int, default=8)
    a = ap.parse_args()
    with open(a.log, encoding="utf-8", errors="replace") as f:
        lines = [ANSI.sub("", l.rstrip("\n")) for l in f]
    wins, pending = [], []
    for i, l in enumerate(lines):
        if l.startswith("frame_stats"):
            wins.append((i, dict(re.findall(r"(\w+)=(\S+)", l)), pending))
            pending = []
        elif "event=tick" not in l:
            pending.append((i, l))
    rev = next((l for l in lines if "revision=" in l), "")
    print(f"{a.log}: {len(wins)} windows; {rev[:100]}")
    order = sorted(range(len(wins)), key=lambda k: -float(wins[k][1].get("max_ms", 0)))
    verdict = "no frame_stats lines"
    for rank, k in enumerate(order[: a.top]):
        i, kv, evs = wins[k]
        rw = dict(zip(STAGES, kv.get("rworst", "").split("/")))
        stages = {n: float(v) for n, v in rw.items() if n != "render" and v and float(v) >= 1.0}
        top_stage = max(stages, key=stages.get) if stages else "-"
        if rank == 0:
            verdict = f"worst window L{i+1} max_ms={kv.get('max_ms')} held by {top_stage}"
        print(f"\n#{rank+1} L{i+1} max_ms={kv.get('max_ms')} fps={kv.get('fps')} frames={kv.get('frames')} "
              f"over16={kv.get('over16')} render_ms={kv.get('render_ms')} main_ms={kv.get('main_ms')} "
              f"fixed_ms={kv.get('fixed_ms')} cpu_load={kv.get('cpu_load', '-')} faults={kv.get('faults', '-')} "
              f"ivcsw={kv.get('ivcsw', '-')}")
        print("   rworst: " + " ".join(f"{n}={v:g}" for n, v in stages.items()) + f" | enc={kv.get('enc', '-')[:120]}")
        for j, l in evs[-8:]:
            print(f"   L{j+1}: {l[:150]}")
        if k + 1 < len(wins):
            for j, l in wins[k + 1][2][:3]:
                print(f"   next L{j+1}: {l[:150]}")
    done = next((i for i, l in enumerate(lines) if l.startswith("pipeline_warmup phase=Done")), None)
    traced = any(l.startswith("pipeline_new") for l in lines)
    missed = "untraced"
    if traced and done is not None:
        views = [(i, re.search(r"view=(\S+)", l)) for i, l in enumerate(lines) if l.startswith("drive_camera event=commit")]
        late = [(i, l) for i, l in enumerate(lines) if l.startswith("pipeline_new") and i > done]
        by_view = {}
        for i, _ in late:
            seen = [m.group(1) for j, m in views if j < i and m]
            by_view[seen[-1] if seen else "start"] = by_view.get(seen[-1] if seen else "start", 0) + 1
        missed = f"{len(late)} " + (str(by_view) if by_view else "")
        print(f"\npipelines after warm-up: {missed}")
        for i, l in late[:12]:
            print(f"   L{i+1}: {l[:150]}")
    print(f"VERDICT: {verdict}; pipelines after warm-up: {missed}")


if __name__ == "__main__":
    main()
