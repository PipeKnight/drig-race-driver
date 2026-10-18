"""Summarise a Bevy `trace_chrome` trace (docs/PERFORMANCE.md).

Streams the JSON line by line (traces of a minute are over a gigabyte), pairs
each thread's B/E events, and reports for a time window:
- per thread: busy time (top-level spans) per frame;
- the spans with the largest inclusive time per frame, overall and per thread.

A frame is one span whose name contains --frame-span (default the main
schedule). Only spans that end inside the window count.

usage: python -I tools/chrome_trace_summary.py TRACE.json
           [--skip-seconds S] [--seconds S] [--top N] [--frame-span TEXT]
"""
import argparse
import collections
import json
import re


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("trace")
    parser.add_argument("--skip-seconds", type=float, default=0.0,
                        help="window start, seconds after the first event")
    parser.add_argument("--seconds", type=float, default=1e9, help="window length")
    parser.add_argument("--top", type=int, default=40)
    parser.add_argument("--frame-span", default='schedule: name=Main')
    args = parser.parse_args()

    names = {}
    stacks = collections.defaultdict(list)  # tid -> [(name, ts)]
    inclusive = collections.Counter()  # (tid, name) -> us
    exclusive = collections.Counter()
    calls = collections.Counter()
    busy = collections.Counter()  # tid -> us of top-level spans
    child_time = collections.defaultdict(list)  # tid -> per-depth child us
    frames = 0
    first = None
    window = None
    with open(args.trace, encoding="utf-8") as f:
        for line in f:
            line = line.strip().rstrip(",")
            if not line.startswith("{"):
                continue
            event = json.loads(line)
            ph = event.get("ph")
            tid = event.get("tid")
            if ph == "M":
                names[tid] = event.get("args", {}).get("name", str(tid))
                continue
            ts = event.get("ts")
            if ts is None:
                continue
            if first is None:
                first = ts
                start = first + args.skip_seconds * 1e6
                window = (start, start + args.seconds * 1e6)
            name = re.sub(r"\s+", " ", event.get("name", "?"))
            stack = stacks[tid]
            children = child_time[tid]
            if ph == "B":
                stack.append((name, ts))
                children.append(0.0)
            elif ph == "E" and stack:
                opened, began = stack.pop()
                inner = children.pop()
                duration = ts - began
                if children:
                    children[-1] += duration
                if not (window[0] <= ts <= window[1]):
                    continue
                key = (tid, opened)
                inclusive[key] += duration
                exclusive[key] += duration - inner
                calls[key] += 1
                if not stack:
                    busy[tid] += duration
                if args.frame_span in opened and tid == 0:
                    frames += 1
    frames = max(frames, 1)
    print(f"frames={frames} (spans containing {args.frame_span!r} on thread 0)")
    print("\nbusy ms/frame per thread")
    for tid, us in busy.most_common():
        print(f"{us / frames / 1000:9.3f}  {names.get(tid, tid)} (tid {tid})")

    merged_inc = collections.Counter()
    merged_exc = collections.Counter()
    merged_calls = collections.Counter()
    for (tid, name), us in inclusive.items():
        merged_inc[name] += us
        merged_exc[name] += exclusive[(tid, name)]
        merged_calls[name] += calls[(tid, name)]
    print(f"\ntop {args.top} spans, all threads: inclusive ms/frame, exclusive ms/frame, calls/frame")
    for name, us in merged_inc.most_common(args.top):
        print(f"{us / frames / 1000:9.3f} {merged_exc[name] / frames / 1000:9.3f} "
              f"{merged_calls[name] / frames:7.2f}  {name[:150]}")
    print(f"\ntop {args.top} spans by exclusive time, all threads")
    for name, us in merged_exc.most_common(args.top):
        print(f"{us / frames / 1000:9.3f}  {name[:150]}")


if __name__ == "__main__":
    main()
