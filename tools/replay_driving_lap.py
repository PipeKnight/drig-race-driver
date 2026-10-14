"""Private observed-pose replay; interpolation is not vehicle resimulation."""
import argparse
import ast
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path
from summarize_driving_log import FIELDS,ROOT

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def number(word):return None if word=='none' else struct.unpack('<d',int(word,16).to_bytes(8,'little'))[0]

def main():
    p=argparse.ArgumentParser();p.add_argument('log',type=Path);p.add_argument('output',type=Path);p.add_argument('--segment',type=int,default=0);p.add_argument('--profile',choices=['debug','release'],default='release');p.add_argument('--all-progress-gates',action='store_true');p.add_argument('--game-root',type=Path,default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid'));args=p.parse_args()
    source=args.log.resolve();out=args.output.resolve();private=(ROOT/'local').resolve();assert source.is_relative_to(private) and out.is_relative_to(private) and source.stat().st_size<=128*1024*1024
    rows=[]
    for line in source.read_text(encoding='utf-8').splitlines():
        if not line.startswith('drive_sample '):continue
        r=dict(FIELDS.findall(line))
        if int(r['segment'])!=args.segment:continue
        assert r['version']=='1' and r['event']!='failure'
        tick=int(r['tick']);pose=ast.literal_eval(r['pose']);assert len(pose)==16 and all(math.isfinite(v) for v in pose)
        if r['event']=='tick' or (r['event']=='exit' and rows and tick>rows[-1][0]):rows.append((tick,pose[12:15]))
    assert rows and rows[0][0]==1 and all(a[0]<b[0] for a,b in zip(rows,rows[1:]))
    maximum_gap=max(b[0]-a[0] for a,b in zip(rows,rows[1:]));assert maximum_gap<=64
    frames=[rows[0]]
    for (a,old),(b,new) in zip(rows,rows[1:]):
        for tick in range(a+1,b+1):
            fraction=(tick-a)/(b-a);position=[struct.unpack('<f',struct.pack('<f',x+fraction*(y-x)))[0] for x,y in zip(old,new)];frames.append((tick,position))
    assert len(frames)==rows[-1][0]<=360000
    out.mkdir(exist_ok=False);request=out/'input.bin';request.write_bytes(b'LP01'+struct.pack('<I',len(frames))+b''.join(struct.pack('<Q3f',tick,*pos) for tick,pos in frames))
    exe=ROOT/f'target/{args.profile}/grid-lap-replay.exe';runs=[]
    for kind in ['first','repeat']:
        target=out/f'{kind}.tsv';run=subprocess.run([str(exe),str(args.game_root),str(request),str(target)]+(['--all-progress-gates'] if args.all_progress_gates else []),cwd=ROOT,capture_output=True,text=True,timeout=60);runs.append(dict(kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr));assert run.returncode==0 and not run.stderr,runs[-1]
    assert (out/'first.tsv').read_bytes()==(out/'repeat.tsv').read_bytes()
    events=[s.split('\t') for s in (out/'first.tsv').read_text().splitlines()];summary=events[-1];assert summary[0]=='S' and events[0][0]=='P'
    laps=[dict(tick=int(r[1]),number=int(r[3]),valid=r[4]=='1',seconds=number(r[5]),missed_gate=None if r[6]=='none' else int(r[6]),sectors=[number(w) for w in r[7].split(',')]) for r in events if len(r)>2 and r[2]=='LAP']
    report=dict(source=str(source),source_sha256=digest(source),script_sha256=digest(Path(__file__)),executable_sha256=digest(exe),progress_sha256=digest(args.game_root/'tracks/europe/donington/route_0/progress_track.xml'),profile=args.profile,all_progress_gates=args.all_progress_gates,required_gates=[int(g) for g in events[0][2].split(',')],segment=args.segment,observed_samples=len(rows),interpolated_ticks=len(frames),maximum_logged_gap_ticks=maximum_gap,repeat_exact=True,attempts=int(summary[1]),valid_laps=int(summary[2]),next_gate=int(summary[3]),passed_gates=int(summary[4]),best_seconds=number(summary[5]),current_elapsed=number(summary[6]),laps=laps,events=len(events)-2,executions=runs,files={q.name:dict(size=q.stat().st_size,sha256=digest(q)) for q in out.iterdir() if q.is_file()},scope='Earlier observed prototype pose path linearly interpolated between logged positions at the selected16ms clock. This validates current checkpoint policy on that sampled path, not vehicle resimulation, a new manual lap, original GRID timing or exact motion between samples.')
    report['implementation_sha256']={str(q.relative_to(ROOT)):digest(q) for q in [ROOT/'crates/grid-world/src/lap_timing.rs',ROOT/'crates/grid-tools/src/lap_progress.rs',ROOT/'crates/grid-tools/src/bin/grid-lap-replay.rs']}
    with (out/'validation.json').open('x') as f:json.dump(report,f,indent=2)
    print(json.dumps({k:report[k] for k in ['observed_samples','interpolated_ticks','maximum_logged_gap_ticks','repeat_exact','attempts','valid_laps','laps']}))
if __name__=='__main__':main()
