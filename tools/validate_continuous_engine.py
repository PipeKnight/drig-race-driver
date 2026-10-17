"""Repeatability, clipping and pause/reset checks; audible fidelity needs listening."""
import argparse
import hashlib
import json
import wave
from pathlib import Path
import numpy as np

ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('debug',type=Path);p.add_argument('release',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
    for path in (a.debug,a.release,a.output):assert path.resolve().is_relative_to((ROOT/'local').resolve())
    assert not a.output.exists()
    report={'scope':'Continuous twelve-voice PCM presentation; signal/profile repeatability, not native audible equivalence','files':[]}
    for name,frames in [('fixed-rpm.wav',576000),('source-drive.wav',768000)]:
        debug=a.debug/name;release=a.release/name
        assert debug.read_bytes()==release.read_bytes()
        with wave.open(str(release)) as w:
            assert (w.getnchannels(),w.getsampwidth(),w.getframerate(),w.getnframes())==(1,2,48000,frames)
            v=np.frombuffer(w.readframes(frames),'<i2').astype(np.float64)/32768
        assert np.isfinite(v).all() and .01<abs(v).max()<.98
        report['files'].append(dict(name=name,frames=frames,sha256=sha(release),debug_release_exact=True,rms=float(np.sqrt(np.mean(v*v))),peak=float(abs(v).max()),maximum_adjacent_step=float(abs(np.diff(v)).max())))
        if name=='fixed-rpm.wav':
            assert np.array_equal(v[:96000],v[480000:])
            assert not np.any(v[int(8.02*48000):480000])
            report['stages']=[]
            for sec,rpm in [(0.5,2200),(2.5,4500),(4.5,7000),(6.5,4500),(10.5,2200)]:
                x=v[int(sec*48000):int((sec+1)*48000)];freq=np.fft.rfftfreq(len(x),1/48000);power=abs(np.fft.rfft(x*np.hanning(len(x))));band=(freq>40)&(freq<800)
                report['stages'].append(dict(seconds=sec,rpm=rpm,rms=float(np.sqrt(np.mean(x*x))),dominant_hz=float(freq[band][power[band].argmax()]),maximum_adjacent_step=float(abs(np.diff(x)).max())))
            peaks=[x['dominant_hz'] for x in report['stages']]
            assert peaks[0]<peaks[1]<peaks[2] and abs(peaks[1]-peaks[3])<25 and peaks[0]==peaks[4]
        else:assert not np.any(v[:125*768])
    assert (a.debug/'telemetry.tsv').read_bytes()==(a.release/'telemetry.tsv').read_bytes()
    report['identities']={str(f):sha(f) for f in [Path(__file__),ROOT/'crates/grid-tools/src/engine_audio_continuous.rs',ROOT/'crates/grid-probes/src/bin/grid-engine-continuous.rs',ROOT/'target/debug/grid-engine-continuous.exe',ROOT/'target/release/grid-engine-continuous.exe']}
    with a.output.open('x') as f:json.dump(report,f,indent=2)
    print(json.dumps({k:v for k,v in report.items() if k!='identities'}))
if __name__=='__main__':main()
