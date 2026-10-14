"""Validate private ET01 read-only engine observations and summarize their scope."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
MAX_RECORD = 5088


def load(path):
    assert 32 <= path.stat().st_size <= 32 + 6500 * MAX_RECORD
    data = path.read_bytes()
    assert len(data) >= 32 and data[:4] in (b"ET01", b"ET02")
    size, count, address, frequency, pid, reserved = struct.unpack_from("<IIIQII", data, 4)
    expected = 2440 if data[:4] == b"ET01" else 5088
    voice_bytes = 128 if data[:4] == b"ET01" else 224
    repeat_at = 2436 if data[:4] == b"ET01" else 4932
    assert size == expected and 0 < count <= 6500 and len(data) == 32 + count * size
    assert 0x20000 <= address <= 0x7ffe0000 and frequency > 0 and pid > 0 and reserved == 0
    rows = [data[32 + i * size:32 + (i + 1) * size] for i in range(count)]
    times = np.array([struct.unpack_from("<QQ", row) for row in rows], dtype=np.uint64)
    assert np.all(times[:, 1] >= times[:, 0]) and np.all(np.diff(times[:, 0].astype(np.int64)) > 0)
    f = lambda row, at: struct.unpack_from("<f", row, at)[0]
    u = lambda row, at: struct.unpack_from("<I", row, at)[0]
    for row in rows:
        ctrl = row[16:848]
        assert u(ctrl, 0x128) + 0x8f10 == address
        assert u(ctrl, 0x118) == u(ctrl, 0x128) + 0x1a40
        assert (u(ctrl, 0x328), u(ctrl, 0x32c)) == (477036, 319292)
        assert all(u(ctrl, 0x290 + i * 4) == 32000 for i in range(12))
        assert [u(ctrl, 0x2c0 + i * 4) for i in range(12)] == [319292, 477036, 477036] * 4
        assert u(row, repeat_at) in (0, 1)
        assert all(np.isfinite(f(row, at)) for at in [848, 852, 856, 864, *range(868, 900, 4)])
        assert 0 <= f(row, 848) <= 2 * f(row, 852) and 0 < f(row, 856) < f(row, 852)
        for i in range(12):
            voice = row[900 + i * voice_bytes:900 + (i + 1) * voice_bytes]
            assert all(np.isfinite(f(voice, at)) for at in [0x14, 0x18, 0x50, 0x54, 0x58, 0x5c])
            assert -48 <= f(voice, 0x14) <= 0 and .5 <= f(voice, 0x18) <= 2
            assert 0 < f(voice, 0x50) <= f(voice, 0x54)
            if voice_bytes == 224:
                assert all(np.isfinite(f(voice, at)) for at in (0xcc, 0xd0))
                assert f(voice, 0xcc) <= 0
                backend = row[3588 + i * 112:3700 + i * 112]
                if u(voice, 0x68) != 0xa000001:
                    assert 0 < u(backend, 0x2c) <= 96000 and np.isfinite(f(backend, 0x30)) and f(backend, 0x30) <= 0
    return rows, times, dict(address=f"{address:x}", qpc_frequency=frequency, pid=pid,
                            version=data[:4].decode(), voice_bytes=voice_bytes, repeat_at=repeat_at)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("trace", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    for p in (args.trace, args.output):
        assert p.resolve().is_relative_to((ROOT / "local").resolve())
    assert not args.output.exists()
    rows, times, identity = load(args.trace)
    f = lambda row, at: struct.unpack_from("<f", row, at)[0]
    values = lambda at: [f(row, at) for row in rows]
    extent = lambda values: dict(minimum=min(values), maximum=max(values))
    gains = [[f(row, 900 + i * identity['voice_bytes'] + 0x14) for row in rows] for i in range(12)]
    report = dict(
        passed=True, trace_sha256=hashlib.sha256(args.trace.read_bytes()).hexdigest(),
        identity=identity, records=len(rows),
        duration_seconds=int(times[-1, 0] - times[0, 0]) / identity["qpc_frequency"],
        sample_interval_seconds=extent(np.diff(times[:, 0].astype(np.int64)).astype(float).tolist()),
        read_duration_qpc=extent((times[:, 1] - times[:, 0]).tolist()),
        repeated_controller_exact=sum(struct.unpack_from("<I", row, identity['repeat_at'])[0] for row in rows),
        engine_speed=extent(values(848)), maximum=extent(values(852)), idle=extent(values(856)),
        camera_modes=sorted({struct.unpack_from("<I", row, 860)[0] for row in rows}),
        root_treble=extent(values(864)), voice_gains=[extent(v) for v in gains],
        audible_loaded_rows=sum(any(gains[i][n] > -48 for i in (2, 5, 8, 11)) for n in range(len(rows))),
        scope="Sequential read-only observations; controller repeat equality does not prove full-frame atomicity. No PCM, scheduler or audible equivalence claim."
    )
    if identity['version'] == 'ET02':
        report['post_mix_gains'] = [extent([f(row, 900 + i * 224 + 0xcc) for row in rows]) for i in range(12)]
        report['backend_volumes'] = [extent([f(row, 3588 + i * 112 + 0x30) for row in rows]) for i in range(12)]
        report['backend_effect_masks'] = [sorted({struct.unpack_from('<I', row, 3588 + i * 112 + 0x34)[0] for row in rows}) for i in range(12)]
    report["sample_interval_seconds"] = {k: v / identity["qpc_frequency"] for k, v in report["sample_interval_seconds"].items()}
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
