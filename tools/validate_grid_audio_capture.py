"""Inspect private process-loopback WAV and packet timing, without fidelity claims."""
import argparse
import csv
import hashlib
import json
import wave
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("wave", type=Path)
    parser.add_argument("log", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    packet_path = Path(str(args.wave) + ".packets.tsv")
    for path in (args.wave, args.log, args.output, packet_path):
        assert path.resolve().is_relative_to((ROOT / "local").resolve())
    assert not args.output.exists()
    with wave.open(str(args.wave), "rb") as audio:
        assert (audio.getframerate(), audio.getnchannels(), audio.getsampwidth()) == (48000, 2, 2)
        assert 0 < audio.getnframes() <= 65 * 48000
        pcm = np.frombuffer(audio.readframes(audio.getnframes()), "<i2").reshape(-1, 2).astype(np.float64) / 32768
    with packet_path.open() as stream:
        packets = list(csv.DictReader(stream, delimiter="\t"))
    assert sum(int(row["frames"]) for row in packets) == len(pcm)
    assert [int(row["packet"]) for row in packets] == list(range(len(packets)))
    assert all(int(row["frames"]) > 0 and int(row["flags"]) < 8 for row in packets)
    assert not any(int(row["flags"]) & 4 for row in packets), "capture timestamps invalid"
    assert not any(int(row["flags"]) & 1 for row in packets[1:]), "audio discontinuity after startup"
    errors = [(int(b["qpc_100ns"]) - int(a["qpc_100ns"])) / 1e7 - int(a["frames"]) / 48000 for a, b in zip(packets, packets[1:])]
    assert max(map(abs, errors), default=0) <= 0.0001, "packet timestamp gap"
    positions = [int(row["device_position"]) for row in packets]
    log = args.log.read_text()
    assert "event=completed" in log and "process_tree_only=1" in log
    report = dict(passed=True, frames=len(pcm), rate=48000, channels=2, seconds=len(pcm) / 48000,
                  packets=len(packets), rms=np.sqrt(np.mean(pcm * pcm, axis=0)).tolist(), peak=np.max(abs(pcm), axis=0).tolist(),
                  nonzero_signal=bool(np.any(pcm)), maximum_packet_qpc_error_seconds=max(map(abs, errors), default=0),
                  device_position_available=any(positions),
                  startup_discontinuity=bool(int(packets[0]["flags"]) & 1),
                  scope="Validated selected-process loopback output after Windows conversion to PCM16/48kHz. No microphone/whole-system fallback. Signal contents and original-vs-replacement fidelity require separate evidence.",
                  numpy_version=np.__version__)
    identities = [args.wave, packet_path, args.log, ROOT / "tools/capture_grid_audio.cpp", ROOT / "tools/build_grid_audio_capture.cmd", ROOT / "local/audio/capture-grid-audio.exe", Path(__file__)]
    report["identities"] = {str(path.resolve().relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in identities}
    with args.output.open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(json.dumps({k: v for k, v in report.items() if k != "identities"}))


if __name__ == "__main__":
    main()
