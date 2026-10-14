"""Check native sin/cos/log10 contracts and reconstructed crossfade shaping."""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / "local").resolve()) and not out.exists()
    out.mkdir()
    native = ROOT / "local/audio/audio-gain-math-probe.exe"
    paths = []
    for label, exe, prefix in (("native", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                               ("native-repeat", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                               ("debug", ROOT / "target/debug/grid-audio-crossfade.exe", []),
                               ("release", ROOT / "target/release/grid-audio-crossfade.exe", [])):
        path = out / f"{label}.bin"
        result = subprocess.run([str(exe), *map(str, prefix), str(path)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        (out / f"{label}.txt").write_text(result.stdout + result.stderr)
        assert result.returncode == 0, (label, result.stdout, result.stderr)
        if label.startswith("native"):
            assert "entire_image_unchanged=1" in result.stdout and "completed=4097" in result.stdout
        paths.append(path)
    raw = paths[0].read_bytes()
    assert raw == paths[1].read_bytes() and struct.unpack_from("<II", raw) == (0x31304d47, 4097)
    rows = list(struct.iter_unpack("<6d2f", raw[8:]))
    assert len(rows) == 4097
    expected = b"GC01" + struct.pack("<I", len(rows))
    errors = [0.] * 5
    maximum_power_error = 0.
    for t, sine, cosine, log_t, log_sine, log_cosine, gain_cosine, gain_sine in rows:
        angle = f32(f32(t) * f32(1.570796012878418))
        wanted = (math.sin(angle), math.cos(angle), math.log10(t), math.log10(f32(sine)), math.log10(f32(cosine)))
        for i, (actual, value) in enumerate(zip((sine, cosine, log_t, log_sine, log_cosine), wanted)):
            errors[i] = max(errors[i], abs(actual - value))
        assert all(-48 <= value <= 0 for value in (gain_cosine, gain_sine))
        assert gain_cosine == max(-48., f32(f32(log_cosine) * 20.))
        assert gain_sine == max(-48., f32(f32(log_sine) * 20.))
        expected += struct.pack("<3f", t, gain_cosine, gain_sine)
        power = 10 ** (gain_cosine / 10) + 10 ** (gain_sine / 10)
        maximum_power_error = max(maximum_power_error, abs(power - 1))
    assert max(errors) < 2e-15
    assert maximum_power_error < 0.00005
    for path in paths[2:]:
        actual = path.read_bytes()
        if actual != expected:
            at = next(i for i, (a, b) in enumerate(zip(expected, actual)) if a != b)
            raise AssertionError((path.name, at, "case", (at - 8) // 12, expected[at:at+8].hex(), actual[at:at+8].hex()))
    files = [native, ROOT / "target/debug/grid-audio-crossfade.exe", ROOT / "target/release/grid-audio-crossfade.exe",
             ROOT / "tools/probe_audio_gain_math.c", ROOT / "tools/build_audio_gain_math_probe.cmd",
             ROOT / "crates/grid-tools/src/engine_audio_gain.rs", ROOT / "crates/grid-tools/src/bin/grid-audio-crossfade.rs", Path(__file__)]
    files += list(out.iterdir())
    report = dict(passed=True, cases=len(rows), original_repeat_exact=True, debug_release_gain_bits_exact=True,
                  maximum_absolute_math_contract_errors=errors, maximum_unit_power_error=maximum_power_error,
                  expected_gain_sha256=hashlib.sha256(expected).hexdigest(),
                  identities={str(p): sha(p) for p in files},
                  scope="Unchanged native sin/cos/log10 helpers and reconstructed 42DE60 crossfade shaping only. Whole 42DE60, gain/spatial/backend behavior and audible fidelity remain unverified.")
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "identities"}))


if __name__ == "__main__":
    main()
