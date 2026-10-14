"""Compare complete coupled original playback state with both Rust profiles."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAG = 0x31304345
CTRL = 0x340
RECORD = 4 + CTRL + 20
RESULT = 4 + CTRL + 96


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_curves():
    path = Path("C:/Program Files (x86)/Steam/steamapps/common/Grid/audio/cars/lacengine.ini")
    assert digest(path) == "cfa8887466c35ad5e9a2287c8e670a300952567f1c49e288ec57a1a63a7a04f8"
    sections = {}
    current = None
    for line in path.read_text().splitlines():
        if line.startswith("["):
            current = line.strip()[1:-1]
        elif "=" in line:
            name, value = line.split("=", 1)
            if name in ("RevsFromTime", "TimeFromRevs"):
                sections[current, name] = [float(v) for v in value.split(",")]
    values = [v for s in ("Idle", "Throttle") for k in ("RevsFromTime", "TimeFromRevs") for v in sections[s, k]]
    assert len(values) == 4096
    return struct.pack("<4096f", *values), path


def fixtures(curves, count=1280):
    rng = random.Random(275)
    records = []
    for case in range(count):
        ctrl = bytearray(rng.randbytes(CTRL))
        def put(at, value):
            struct.pack_into("<f", ctrl, at, value)
        put(0, (0., 0.5, 1., 1.5, 2.)[case % 5])
        put(0x28, 0.)
        put(0x38, (0., .1, .2, .5, .9, 1., 1.5, 2.)[case % 8])
        put(0xb0, (-2., -1., 0., .5, 1., 2.)[case % 6])
        put(0xb4, (-1., -.1, 0., .1, 1.)[case % 5])
        put(0xc8, .5)
        put(0x30c, (-1., -.2, 0., .15, 1.)[case % 5])
        for slot in range(12):
            length = (319292, 477036, 10000000, 1)[(case // 1024 + slot) % 4]
            rate = (8000, 32000, 44100, 48000, 192000)[(case + slot) % 5]
            struct.pack_into("<I", ctrl, 0x290 + slot * 4, rate)
            struct.pack_into("<I", ctrl, 0x2c0 + slot * 4, length)
            put(0x260 + slot * 4, (.5, .75, 1., 1.5, 2.)[(case + slot) % 5])
            if case < 1024:
                position = ((case + slot * 67) % 1024) / 1024 * length + .875
            else:
                position = (-1e9, -1.875, -0., 0., .875, length - .125, length, length * 1.5)[case % 8]
            put(0x230 + slot * 4, position)
        maximum = (600., 800., 1000.)[case % 3]
        idle = maximum * (.15, .2, .3, .5)[case % 4]
        speed = maximum * ((case % 1025) / 1024 * 2)
        delta = (0., .016, .1, 1.)[case % 4]
        records.append(struct.pack("<I", case) + ctrl + struct.pack("<3fd", speed, maximum, idle, delta))
    return struct.pack("<II", TAG, count) + curves + b"".join(records)


def execute(exe, args, log):
    result = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True, text=True, timeout=120)
    log.write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (exe, result.returncode, result.stdout, result.stderr)


def audit(request, output):
    raw, data = request.read_bytes(), output.read_bytes()
    count = struct.unpack_from("<I", raw, 4)[0]
    assert data[:8] == raw[:8] and len(data) == 8 + count * RESULT
    changes = [0x14, 0x38, 0x48, 0xbc, 0xc0, 0xc4, 0xc8]
    changes += [0x1a4 + i * 4 for i in range(12)] + [0x260 + i * 4 for i in range(12)]
    changes += [0x230 + i * 4 for i in range(12)]
    writable = {b for at in changes for b in range(at, at + 4)}
    writable.update(range(0x14c, 0x14f))
    lower = upper = 0
    for case in range(count):
        before = raw[8 + 16384 + case * RECORD:8 + 16384 + (case + 1) * RECORD]
        after = data[8 + case * RESULT:8 + (case + 1) * RESULT]
        assert before[:4] == after[:4]
        for byte in range(CTRL):
            if byte not in writable:
                assert before[4 + byte] == after[4 + byte], (case, hex(byte))
        pitches = struct.unpack_from("<12f", after, 4 + 0x1a4)
        assert pitches == struct.unpack_from("<12f", after, 4 + 0x260)
        assert all(.5 <= v <= 2 for v in pitches)
        lower += sum(v == .5 for v in pitches)
        upper += sum(v == 2 for v in pitches)
    return dict(cases=count, complete_controller_bytes=count * CTRL, queries=count * 24,
                lower_pitch_clamps=lower, upper_pitch_clamps=upper, opaque_bytes_preserved=True)


def guards(out, curves, native):
    base = fixtures(curves, 1)
    offset = 8 + 16384 + 4
    cases = []
    for name, at, fmt, value in [
        ("curve_nan", 8, "f", float("nan")),
        ("curve_zero", 8, "f", 0.),
        ("speed_nan", offset + CTRL, "f", float("nan")),
        ("idle_zero", offset + CTRL + 8, "f", 0.),
        ("delta_large", offset + CTRL + 12, "d", 1.01),
        ("last_length_zero", offset + 0x2ec, "I", 0),
        ("last_rate_zero", offset + 0x2bc, "I", 0),
        ("last_pitch_nan", offset + 0x28c, "f", float("nan")),
        ("wobble", offset + 0x28, "f", .1),
        ("negative_gain", offset, "f", -1.),
        ("negative_history", offset + 0x38, "f", -.01),
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
                                   ("debug", ROOT / "target/debug/grid-engine-audio-control.exe", []),
                                   ("release", ROOT / "target/release/grid-engine-audio-control.exe", [])):
            result = subprocess.run([str(exe), *map(str, [*prefix, request, out / f"guard-{name}-{label}.bin"])],
                                    cwd=ROOT, capture_output=True, text=True, timeout=30)
            (out / f"guard-{name}-{label}.txt").write_text(result.stdout + result.stderr)
            assert result.returncode != 0 and "exception=" not in result.stderr, (name, label)
            if label == "native":
                assert "entire_image_unchanged=1" in result.stdout
            rows.append(dict(name=name, profile=label, rejected=True, returncode=result.returncode))
    return rows


def source_drive(out, native):
    game = Path("C:/Program Files (x86)/Steam/steamapps/common/Grid")
    outputs = []
    for profile in ("debug", "release"):
        directory = out / f"source-{profile}"
        execute(ROOT / f"target/{profile}/grid-engine-audio-control-drive.exe", [game, directory], out / f"source-{profile}.txt")
        outputs.append(directory)
    assert (outputs[0] / "source.request.bin").read_bytes() == (outputs[1] / "source.request.bin").read_bytes()
    assert (outputs[0] / "source.expected.bin").read_bytes() == (outputs[1] / "source.expected.bin").read_bytes()
    request = outputs[0] / "source.request.bin"
    expected = (outputs[0] / "source.expected.bin").read_bytes()
    for repeat in range(2):
        output = out / f"source-native-{repeat}.bin"
        execute(native, [ROOT / "local/compatibility/grid-unpacked.exe", request, output], out / f"source-native-{repeat}.txt")
        assert output.read_bytes() == expected
    raw = request.read_bytes()
    for case in range(999):
        after = expected[8 + case * RESULT + 4:8 + case * RESULT + 4 + CTRL]
        before = raw[8 + 16384 + (case + 1) * RECORD + 4:8 + 16384 + (case + 1) * RECORD + 4 + CTRL]
        assert after == before, (case, "carried controller")
    return dict(**audit(request, outputs[0] / "source.expected.bin"), complete_controller_carried=True,
                source_physics_paired_no_feedback=True, output_sha256=hashlib.sha256(expected).hexdigest())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--source-drive", action="store_true")
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / "local").resolve()) and not out.exists()
    out.mkdir()
    source, ini = source_curves()
    authored = struct.pack("<4096f", *(
        [1 - .65 * i / 1023 for i in range(1024)] + [1 - i / 1023 for i in range(1024)] +
        [.35 + .65 * i / 1023 for i in range(1024)] + [i / 1023 for i in range(1024)]))
    native = ROOT / "local/audio/engine-audio-control-probe.exe"
    rows = []
    for name, curves in (("lacetti", source), ("authored", authored)):
        request = out / f"{name}.request.bin"
        request.write_bytes(fixtures(curves))
        outputs = []
        for label, exe, prefix in (("native", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                                   ("native-repeat", native, [ROOT / "local/compatibility/grid-unpacked.exe"]),
                                   ("debug", ROOT / "target/debug/grid-engine-audio-control.exe", []),
                                   ("release", ROOT / "target/release/grid-engine-audio-control.exe", [])):
            output = out / f"{name}-{label}.bin"
            execute(exe, [*prefix, request, output], out / f"{name}-{label}.txt")
            outputs.append(output)
        expected = outputs[0].read_bytes()
        for path in outputs[1:]:
            actual = path.read_bytes()
            if actual != expected:
                at = next(i for i, (a, b) in enumerate(zip(expected, actual)) if a != b)
                raise AssertionError((name, path.name, "first mismatch", at, "case", (at - 8) // RESULT,
                                      "record offset", (at - 8) % RESULT,
                                      expected[max(0, at-4):at+8].hex(), actual[max(0, at-4):at+8].hex()))
        rows.append(dict(name=name, **audit(request, outputs[0]), output_sha256=digest(outputs[0])))
    guard_rows = guards(out, source, native)
    source_row = source_drive(out, native) if args.source_drive else None
    files = [native, ROOT / "target/debug/grid-engine-audio-control.exe", ROOT / "target/release/grid-engine-audio-control.exe",
             ROOT / "tools/probe_engine_audio_control.c", ROOT / "tools/build_engine_audio_control_probe.cmd",
             ROOT / "crates/grid-tools/src/engine_audio_control.rs", ROOT / "crates/grid-tools/src/bin/grid-engine-audio-control.rs", Path(__file__), ini]
    if args.source_drive:
        files += [ROOT / f"target/{profile}/grid-engine-audio-control-drive.exe" for profile in ("debug", "release")]
        files += [ROOT / "crates/grid-tools/src/bin/grid-engine-audio-control-drive.rs"]
        files += [ROOT / "crates/grid-formats/src/audio_car_setup.rs", Path("C:/Program Files (x86)/Steam/steamapps/common/Grid/audio/carsetup.bin"), Path("C:/Program Files (x86)/Steam/steamapps/common/Grid/audio/CMSamples.dic")]
    files += list(out.rglob("*.bin")) + list(out.rglob("*.txt"))
    report = dict(passed=True, groups=rows, source_drive=source_row, guards=guard_rows, identities={str(p): digest(p) for p in files},
                  scope="Coupled unchanged 422A40, 4152A0, 415500 x3, 4153E0/415460 x12, original repeat and Rust debug/release complete output equality. Authored wrapper only; no 451770/backend/gear-wobble/gain/PCM/audio-fidelity equivalence.")
    (out / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "identities"}))


if __name__ == "__main__":
    main()
