"""Audit private deterministic RPM mixer renders; not native mixing fidelity."""
import argparse
import csv
import hashlib
import json
import wave
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("debug", type=Path)
    parser.add_argument("release", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    for path in (args.debug, args.release, args.output):
        assert path.resolve().is_relative_to((ROOT / "local").resolve())
    assert not args.output.exists()
    report = {"scope": "Independent mixer, profile repeatability and offline signal checks; no native audio fidelity claim", "files": [], "stages": []}
    waves = {}
    for name, frames in (("fixed-rpm.wav", 576000), ("source-drive.wav", 768000)):
        debug, release = args.debug / name, args.release / name
        assert debug.read_bytes() == release.read_bytes(), name
        with wave.open(str(release), "rb") as audio:
            assert (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(), audio.getnframes()) == (1, 2, 48000, frames)
            pcm = np.frombuffer(audio.readframes(frames), "<i2")
        samples = pcm.astype(np.float64) / 32768
        assert np.isfinite(samples).all() and 0.01 < np.abs(samples).max() < 0.23
        waves[name] = pcm
        report["files"].append(dict(name=name, frames=frames, sha256=sha(release), debug_release_exact=True, peak=float(np.abs(samples).max()), rms=float(np.sqrt(np.mean(samples ** 2)))))
    fixed = waves["fixed-rpm.wav"]
    assert np.array_equal(fixed[:96000], fixed[480000:576000])
    assert not np.any(fixed[round(8.02 * 48000):480000])
    assert not np.any(waves["source-drive.wav"][:125 * 768])
    peaks = []
    for seconds, rpm in ((0.5, 2200), (2.5, 4500), (4.5, 7000), (6.5, 4500), (10.5, 2200)):
        samples = fixed[round(seconds * 48000):round((seconds + 1) * 48000)].astype(np.float64) / 32768
        spectrum = abs(np.fft.rfft(samples * np.hanning(len(samples))))
        frequency = np.fft.rfftfreq(len(samples), 1 / 48000)
        band = (frequency > 40) & (frequency < 600)
        peak = float(frequency[band][spectrum[band].argmax()])
        peaks.append(peak)
        report["stages"].append(dict(seconds=seconds, requested_rpm=rpm, dominant_frequency_hz=peak, rms=float(np.sqrt(np.mean(samples ** 2))), maximum_adjacent_step=float(abs(np.diff(samples)).max())))
    assert peaks[0] < peaks[1] < peaks[2]
    assert abs(peaks[1] - peaks[3]) < 25 and peaks[0] == peaks[4]
    assert (args.debug / "telemetry.tsv").read_bytes() == (args.release / "telemetry.tsv").read_bytes()
    with (args.release / "telemetry.tsv").open() as file:
        rows = list(csv.DictReader(file, delimiter="\t"))
    assert len(rows) == 1000 and [int(row["tick"]) for row in rows] == list(range(1, 1001))
    for i, row in enumerate(rows):
        assert float(row["throttle"]) == float(125 <= i < 625)
        for name in ("angular_speed", "normalized_rpm"):
            assert np.isfinite(float(row[name]))
    assert max(float(row["normalized_rpm"]) for row in rows) > 0.5
    report.update(pause_silent_after_20_ms=True, reset_exact=True, stopped_engine_silent=True, rpm_note_rises=True, source_telemetry_exact=True, source_ticks=1000)
    report["identities"] = {str(path.relative_to(ROOT)): sha(path) for path in (
        ROOT / "crates/grid-tools/src/engine_audio.rs",
        ROOT / "crates/grid-probes/src/bin/grid-engine-audio.rs",
        ROOT / "crates/grid-app/src/engine_sound.rs",
        ROOT / "crates/grid-app/src/main.rs",
        ROOT / "Cargo.lock",
        Path(__file__).resolve(),
        ROOT / "target/debug/grid-engine-audio.exe",
        ROOT / "target/release/grid-engine-audio.exe",
    )}
    report["numpy_version"] = np.__version__
    with args.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({key: value for key, value in report.items() if key != "identities"}))


if __name__ == "__main__":
    main()
