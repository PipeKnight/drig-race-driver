"""Private telemetry-driven twelve-voice PCM experiment; not a native mixer port.

Uses observed phase/gain/pitch, with explicit linear resampling and mono summing.
It deliberately does not claim native device phase, spatial routing or filters.
"""
import argparse
import csv
import hashlib
import json
import struct
import wave
from pathlib import Path

import numpy as np

from inspect_grid_engine_trace import load, ROOT

NAMES = ("dn_en", "up_en", "up_en_load", "dn_ex", "up_ex", "up_ex_load") * 2


def read_wave(path, rate, channels):
    with wave.open(str(path), "rb") as stream:
        assert (stream.getframerate(), stream.getnchannels(), stream.getsampwidth()) == (rate, channels, 2)
        assert 0 < stream.getnframes() <= rate * 65
        data = np.frombuffer(stream.readframes(stream.getnframes()), "<i2").reshape(-1, channels).astype(np.float64) / 32768
    return data


def save_wave(path, samples):
    assert not path.exists() and np.all(np.isfinite(samples)) and np.max(np.abs(samples)) <= 1
    if samples.ndim == 1:
        samples = samples[:, None]
    pcm = np.clip(np.rint(samples * 32768), -32768, 32767).astype("<i2")
    with wave.open(str(path), "wb") as stream:
        stream.setnchannels(samples.shape[1]); stream.setsampwidth(2); stream.setframerate(48000)
        stream.writeframes(pcm.tobytes())
    return hashlib.sha256(path.read_bytes()).hexdigest()


def spectrum(samples):
    size = 8192
    assert len(samples) >= size
    channels = [samples[:, i] for i in range(samples.shape[1])] if samples.ndim == 2 else [samples]
    powers = []
    for channel in channels:
        frames = np.lib.stride_tricks.sliding_window_view(channel, size)[::size // 2]
        powers.append((np.abs(np.fft.rfft(frames * np.hanning(size), axis=1)) ** 2).mean(axis=0))
    power = np.mean(powers, axis=0)
    hz = np.fft.rfftfreq(size, 1 / 48000)
    total = power[(hz >= 40) & (hz < 12000)].sum()
    assert total > 0
    return {f"{low}-{high}Hz": float(power[(hz >= low) & (hz < high)].sum() / total)
            for low, high in [(40, 125), (125, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000), (8000, 12000)]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("native", type=Path)
    parser.add_argument("recordings", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--start", type=float, required=True)
    parser.add_argument("--end", type=float, required=True)
    parser.add_argument("--phase-mode", choices=("interpolate", "advance", "continuous"), default="interpolate")
    parser.add_argument("--gain-mode", choices=("input", "post-mix"), default="input")
    args = parser.parse_args()
    for p in (args.native, args.recordings, args.output):
        assert p.resolve().is_relative_to((ROOT / "local").resolve())
    assert not args.output.exists() and 0 <= args.start < args.end <= 60
    rows, stamps, identity = load(Path(str(args.native) + ".engine.bin"))
    assert args.gain_mode == 'input' or identity['version'] == 'ET02'
    native = read_wave(args.native, 48000, 2)
    packets = list(csv.DictReader(Path(str(args.native) + ".packets.tsv").open(), delimiter="\t"))
    origin = int(packets[0]["qpc_100ns"]) / 1e7
    times = stamps.astype(np.float64).mean(axis=1) / identity["qpc_frequency"] - origin
    assert args.end <= len(native) / 48000 and args.start >= times[0] and args.end <= times[-1]
    start, end = int(round(args.start * 48000)), int(round(args.end * 48000))
    output_times = np.arange(start, end, dtype=np.float64) / 48000
    pcm = {name: read_wave(args.recordings / (name + ".wav"), 32000, 1)[:, 0] for name in set(NAMES)}
    assert all(len(v) == (319292 if name.startswith("dn_") else 477036) for name, v in pcm.items())
    f = lambda row, at: struct.unpack_from("<f", row, at)[0]
    mixed = np.zeros(len(output_times), dtype=np.float64)
    diagnostics = []
    for slot, name in enumerate(NAMES):
        phases = np.array([f(row, 16 + 0x230 + slot * 4) for row in rows])
        pitches = np.array([f(row, 900 + slot * identity['voice_bytes'] + 0x18) for row in rows])
        db = np.array([f(row, 900 + slot * identity['voice_bytes'] + (0x14 if args.gain_mode == 'input' else 0xcc)) for row in rows])
        frequencies = (np.array([struct.unpack_from('<I', row, 3588 + slot * 112 + 0x2c)[0] for row in rows], dtype=np.float64)
                       if identity['version'] == 'ET02' else np.minimum(np.floor(pitches * 32000 + .5), 96000))
        assert np.all(np.isfinite(phases)) and np.all(np.abs(phases) <= 1e9)
        # Collapse repeated observations of a controller tick. Neither method
        # knows the backend's actual playback cursor or its delivery latency.
        changed = np.r_[True, (np.diff(phases) != 0) | (np.diff(frequencies) != 0) | (np.diff(db) != 0)]
        t, p, frequency = times[changed], phases[changed], frequencies[changed]
        assert len(t) >= 2
        segment = np.clip(np.searchsorted(t, output_times, side="right") - 1, 0, len(t) - 2)
        elapsed = output_times - t[segment]
        span = t[segment + 1] - t[segment]
        rounded_frequency = frequency[segment]
        expected = span * rounded_frequency
        discontinuity = np.abs(p[segment + 1] - p[segment] - expected) > 1024
        if args.phase_mode == "interpolate":
            position = p[segment] + np.where(discontinuity, elapsed * rounded_frequency,
                                             elapsed / span * (p[segment + 1] - p[segment]))
        elif args.phase_mode == 'advance':
            position = p[segment] + elapsed * rounded_frequency
        else:
            # Keep a continuous cursor between inferred seek events. Observed
            # frame positions are quantized by game ticks, not the sample clock.
            cursor = np.empty(len(t)); cursor[0] = p[0]
            seeks = np.abs(np.diff(p) - np.diff(t) * frequency[:-1]) > 1024
            for i in range(1, len(t)):
                cursor[i] = p[i] if seeks[i - 1] else cursor[i - 1] + (t[i] - t[i - 1]) * frequency[i - 1]
            position = cursor[segment] + elapsed * rounded_frequency
        volume_db = np.interp(output_times, times, db)
        # -48dB remains its mathematical amplitude here; native cutoff is unknown.
        volume = np.power(10., volume_db / 20)
        valid = (position >= 0) & (position < len(pcm[name]) - 1)
        clipped = np.clip(position, 0, len(pcm[name]) - 1.000001)
        integer = clipped.astype(np.int64)
        fraction = clipped - integer
        voice = (pcm[name][integer] * (1 - fraction) + pcm[name][integer + 1] * fraction) * volume * valid
        mixed += voice
        diagnostics.append(dict(slot=slot, recording=name, peak=float(np.max(np.abs(voice))),
                                rms=float(np.sqrt(np.mean(voice ** 2))), outside_recording_frames=int(np.sum(~valid)),
                                observed_discontinuities=int(np.sum(np.abs(np.diff(p) - np.diff(t) * frequency[:-1]) > 1024))))
    reference = native[start:end]
    native_rms = float(np.sqrt(np.mean(reference ** 2)))
    mix_rms = float(np.sqrt(np.mean(mixed ** 2)))
    assert mix_rms > 0 and native_rms > 0
    # A listening comparison uses RMS alignment, explicitly not a native gain fit.
    scale = native_rms / mix_rms
    peak = max(float(np.max(np.abs(mixed * scale))), float(np.max(np.abs(reference))))
    shared_gain = min(1., .95 / peak)
    args.output.mkdir()
    authored = save_wave(args.output / "observed-voices.wav", mixed * scale * shared_gain)
    captured = save_wave(args.output / "original-grid.wav", reference * shared_gain)
    report = dict(passed=True, start_seconds=args.start, end_seconds=args.end, phase_mode=args.phase_mode, gain_mode=args.gain_mode,
                  frequency_source='observed backend' if identity['version'] == 'ET02' else 'voice pitch rounded at source rate',
                  spectral_metric='mean per-channel power, Hann8192/4096-hop, fraction of40..12000Hz power',
                  native_rms=native_rms, unscaled_voice_sum_rms=mix_rms, comparison_rms_scale=scale,
                  comparison_shared_gain=shared_gain, voices=diagnostics,
                  native_spectrum=spectrum(reference), observed_voice_spectrum=spectrum(mixed),
                  outputs=dict(observed_voices_sha256=authored, original_grid_sha256=captured),
                  input_sha256={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in
                                [args.native, Path(str(args.native) + ".engine.bin"), *[args.recordings / (n + ".wav") for n in sorted(set(NAMES))]]},
                  scope="Telemetry-driven linear-resampling/mono-sum experiment. RMS alignment is for listening only. Native cursor, delivery latency, spatial routing, bus gains, filters and other game sounds are not reconstructed. No original-vs-replacement fidelity pass.")
    (args.output / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({k:v for k,v in report.items() if k not in ("input_sha256", "voices")}, indent=2))


if __name__ == "__main__":
    main()
