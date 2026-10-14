"""Exercise process/output bounds without recording or changing game state."""
import argparse
import hashlib
import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("pid", type=int)
    parser.add_argument("start_filetime", type=int)
    parser.add_argument("existing_wave", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--controller", type=int)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / "local").resolve()) and not out.exists()
    existing = args.existing_wave.resolve()
    assert existing.is_relative_to((ROOT / "local").resolve()) and existing.is_file()
    packet = Path(str(existing) + ".packets.tsv")
    before = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (existing, packet)}
    exe = ROOT / "local/audio/capture-grid-audio.exe"
    out.mkdir()
    cases = [
        ("zero_pid", 0, args.start_filetime, 500, out / "zero.wav"),
        ("wrong_start", args.pid, args.start_filetime + 1, 500, out / "wrong-start.wav"),
        ("other_process", os.getpid(), args.start_filetime, 500, out / "other.wav"),
        ("short", args.pid, args.start_filetime, 499, out / "short.wav"),
        ("long", args.pid, args.start_filetime, 60001, out / "long.wav"),
        ("outside", args.pid, args.start_filetime, 500, ROOT / "capture-guard-must-not-exist.wav"),
        ("existing", args.pid, args.start_filetime, 500, existing),
        ("overflow_pid", 2**32, args.start_filetime, 500, out / "overflow.wav"),
    ]
    assert not (ROOT / "capture-guard-must-not-exist.wav").exists()
    rows = []
    for name, pid, start, duration, wave in cases:
        result = subprocess.run([str(exe), str(pid), str(start), str(duration), str(wave)], cwd=ROOT,
                                capture_output=True, text=True, timeout=10)
        (out / f"{name}.txt").write_text(result.stdout + result.stderr)
        assert result.returncode != 0, name
        assert "event=started" not in result.stdout, name
        if name != "existing":
            assert not wave.exists() and not Path(str(wave) + ".packets.tsv").exists(), name
        rows.append(dict(name=name, rejected=True, returncode=result.returncode))
    if args.controller is not None:
        extra = [("zero_controller", 0), ("unmapped_controller", 0x20000),
                 ("interior_controller", args.controller + 4), ("large_controller", 0x80000000)]
        for name, controller in extra:
            wave = out / (name + ".wav")
            result = subprocess.run([str(exe), str(args.pid), str(args.start_filetime), "500", str(wave), str(controller)],
                                    cwd=ROOT, capture_output=True, text=True, timeout=10)
            (out / f"{name}.txt").write_text(result.stdout + result.stderr)
            assert result.returncode != 0 and "event=started" not in result.stdout, name
            assert all(not Path(str(wave) + suffix).exists() for suffix in ("", ".packets.tsv", ".engine.bin"))
            rows.append(dict(name=name, rejected=True, returncode=result.returncode))
    assert before == {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in (existing, packet)}
    report = dict(passed=True, cases=rows, existing_preserved=before,
                  executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),
                  scope="All attempts reject before recording; existing capture preserved byte for byte.")
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(dict(passed=True, rejected=len(rows), existing_preserved=True)))


if __name__ == "__main__":
    main()
