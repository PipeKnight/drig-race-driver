"""Whole 42DE60 owned voice publication against both Rust profiles and driving."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CTRL, VOICE, REQUEST, RESULT = 0x340, 0x80, 2436, 2376


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixtures(count=512):
    rng = random.Random(27542)
    records = []
    for case in range(count):
        ctrl = bytearray(rng.randbytes(CTRL))
        def put(at, value):
            struct.pack_into("<f", ctrl, at, value)
        put(0xcc, (0., .01, .5, .99, 1.)[case % 5])
        for i in range(3):
            put(0x140 + i * 4, (-1., -.9998, -.5, 0., .25, .5, .9998, 1.)[(case + i) % 8])
        put(0x1a0, (-1., -.5, 0., .5, 1.)[(case // 3) % 5])
        put(0x2f8, (.5, .75, 1., 1.25, 2.)[(case // 5) % 5])
        put(0x330, (-48., -3., 0., 3., 48.)[(case // 7) % 5])
        put(0xc8, (0., .05, .2, .5, .9, 1.)[(case // 11) % 6])
        ctrl[0xd8] = case % 2
        for slot in range(12):
            put(0x1a4 + slot * 4, (.5, .75, 1., 1.25, 1.5, 2.)[(case + slot) % 6])
        for at in (0xe0, 0xf0):
            for axis in range(4):
                put(at + axis * 4, rng.uniform(-100, 100))
        motion = [(0., 0., 0., 0.), (.5, .3, .25, 0.), (1., 0., 0., 0.), (2., 9., 0., 0.),
                  (3., 0., 4., 0.), (0., 100., 0., 0.), (0., 0., 0., .5), (1000., 0., 1000., 1000.)][case % 8]
        linear = [rng.uniform(-100, 100) for _ in range(4)]
        ranges = (1., 50., 2., 60.)
        trims = ((0., 0.), (-1., -6.), (-48., -48.), (24., 48.))[case % 4]
        treble = (-48., -12., -3., 0., 3.)[case % 5]
        records.append(struct.pack("<I", case) + ctrl + rng.randbytes(12 * VOICE) +
                       struct.pack("<8f4f2ffI", *motion, *linear, *ranges, *trims, treble, case % 2))
    return b"EG01" + struct.pack("<I", count) + b"".join(records)


def execute(exe, args, log):
    result = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True, text=True, timeout=120)
    log.write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (exe, result.stdout, result.stderr)
    if exe.name == "engine-audio-gain-probe.exe":
        assert "entire_restored_image_unchanged=1" in result.stdout


def compare(paths):
    wanted = paths[0].read_bytes()
    for path in paths[1:]:
        actual = path.read_bytes()
        if actual != wanted:
            at = next(i for i, (a, b) in enumerate(zip(actual, wanted)) if a != b)
            raise AssertionError((path.name, at, "case", (at - 8) // RESULT, "offset", (at - 8) % RESULT,
                                  wanted[at:at+8].hex(), actual[at:at+8].hex()))
    return wanted


def audit(request, result):
    raw, data = request.read_bytes(), result.read_bytes()
    count = struct.unpack_from("<I", raw, 4)[0]
    assert data[:8] == raw[:8] and len(data) == 8 + count * RESULT
    mutable = set(range(0x14, 0x3c)) | set(range(0x50, 0x60)) | {0x78}
    lower = upper = 0
    category_volumes=[[] for _ in range(3)]
    for case in range(count):
        before = raw[8 + case * REQUEST:8 + (case + 1) * REQUEST]
        after = data[8 + case * RESULT:8 + (case + 1) * RESULT]
        assert before[:4] == after[:4] and after[-4:] == b"\0" * 4
        for byte in range(CTRL):
            if byte not in range(8, 12):
                assert before[4 + byte] == after[4 + byte], (case, "controller", byte)
        for slot in range(12):
            start = 4 + CTRL + slot * VOICE
            a, b = before[start:start + VOICE], after[start:start + VOICE]
            for byte in range(VOICE):
                if byte not in mutable:
                    assert a[byte] == b[byte], (case, slot, "opaque", byte)
            volume, pitch = struct.unpack_from("<2f", b, 0x14)
            category_volumes[slot%3].append(volume)
            assert -48 <= volume <= 0 and .5 <= pitch <= 2 and b[0x78] == 1
            assert struct.unpack_from("<f", b, 0x28)[0] == 1 and struct.unpack_from("<f", b, 0x38)[0] == 1
            lower += pitch == .5
            upper += pitch == 2
    return dict(cases=count, complete_controller_and_voice_bytes=count * (CTRL + 12 * VOICE),
                lower_pitch_clamps=lower, upper_pitch_clamps=upper, opaque_bytes_preserved=True,
                category_volume_ranges=[[min(v),max(v)] for v in category_volumes])


def guards(out, native):
    base = fixtures(1)
    c = 8 + 4
    env = c + CTRL + 12 * VOICE
    cases = []
    for name, at, fmt, value in [
        ("fade_nan", c + 0x140, "f", float("nan")),
        ("last_pitch_nan", c + 0x1d0, "f", float("nan")),
        ("bad_off_flag", c + 0xd8, "B", 2),
        ("motion_nan", env + 28, "f", float("nan")),
        ("subnormal_motion", env, "f", 1e-20),
        ("last_range_zero", env + 44, "f", 0.),
        ("treble_nan", env + 56, "f", float("nan")),
        ("bad_player", env + 60, "I", 2),
    ]:
        request = bytearray(base)
        struct.pack_into("<" + fmt, request, at, value)
        cases.append((name, request))
    cases += [("truncated", base[:-1]), ("trailing", base + b"x")]
    rows = []
    for name, data in cases:
        request = out / f"guard-{name}.request.bin"
        request.write_bytes(data)
        for label, exe, prefix in (("native", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                                   ("debug", ROOT / "target/debug/grid-engine-audio-gain.exe", []),
                                   ("release", ROOT / "target/release/grid-engine-audio-gain.exe", [])):
            result = subprocess.run([str(exe), *map(str, [*prefix, request, out / f"guard-{name}-{label}.bin"])],
                                    cwd=ROOT, capture_output=True, text=True, timeout=30)
            (out / f"guard-{name}-{label}.txt").write_text(result.stdout + result.stderr)
            assert result.returncode != 0 and "exception=" not in result.stderr, (name, label)
            if label == "native":
                assert "entire_restored_image_unchanged=1" in result.stdout
            rows.append(dict(name=name, profile=label, rejected=True))
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / "local").resolve()) and not out.exists()
    out.mkdir()
    native = ROOT / "local/audio/engine-audio-gain-probe.exe"
    request = out / "authored.request.bin"
    request.write_bytes(fixtures())
    outputs = []
    for label, exe, prefix in (("native", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                               ("native-repeat", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                               ("debug", ROOT / "target/debug/grid-engine-audio-gain.exe", []),
                               ("release", ROOT / "target/release/grid-engine-audio-gain.exe", [])):
        output = out / f"authored-{label}.bin"
        execute(exe, [*prefix, request, output], out / f"authored-{label}.txt")
        outputs.append(output)
    compare(outputs)
    authored = audit(request, outputs[0])
    directories = []
    for profile in ("debug", "release"):
        directory = out / f"source-{profile}"
        execute(ROOT / f"target/{profile}/grid-engine-audio-control-drive.exe",
                [Path("C:/Program Files (x86)/Steam/steamapps/common/Grid"), directory], out / f"source-{profile}.txt")
        directories.append(directory)
    for name in ("source.gain.request.bin", "source.gain.expected.bin"):
        assert (directories[0] / name).read_bytes() == (directories[1] / name).read_bytes()
    request = directories[0] / "source.gain.request.bin"
    outputs = [directories[0] / "source.gain.expected.bin"]
    for repeat in range(2):
        output = out / f"source-native-{repeat}.bin"
        execute(native, [ROOT / "local/compatibility/grid-unpacked.exe", request, output], out / f"source-native-{repeat}.txt")
        outputs.append(output)
    compare(outputs)
    source = audit(request, outputs[0])
    raw, expected = request.read_bytes(), outputs[0].read_bytes()
    for case in range(999):
        a = 8 + case * RESULT + 4 + CTRL
        b = 8 + (case + 1) * REQUEST + 4 + CTRL
        assert expected[a:a + 12 * VOICE] == raw[b:b + 12 * VOICE]
    source.update(voices_carried=True, paired_source_physics_no_feedback=True)
    guard_rows = guards(out, native)
    files = [native, ROOT / "target/debug/grid-engine-audio-gain.exe", ROOT / "target/release/grid-engine-audio-gain.exe",
             ROOT / "target/debug/grid-engine-audio-control-drive.exe", ROOT / "target/release/grid-engine-audio-control-drive.exe",
             ROOT / "tools/probe_engine_audio_gain.c", ROOT / "tools/build_engine_audio_gain_probe.cmd",
             ROOT / "crates/grid-tools/src/engine_audio_publication.rs", ROOT / "crates/grid-probes/src/bin/grid-engine-audio-gain.rs",
             ROOT / "crates/grid-probes/src/bin/grid-engine-audio-control-drive.rs", Path(__file__)]
    files += list(out.rglob("*.bin")) + list(out.rglob("*.txt"))
    report = dict(passed=True, authored=authored, source=source, guards=guard_rows,
                  identities={str(p): sha(p) for p in files},
                  scope="Whole unchanged 42DE60 including real helpers and observed descriptor dispatch, owned camera3/all-voices context. Original repeat/Rust debug/release complete-state equality. Source motion/poses/metadata plus authored fades/ranges/Treble fixture; no factory, active device/filter/PCM or audible-fidelity claim.")
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "identities"}))


if __name__ == "__main__":
    main()
