"""Independent private gate-plane oracle for sampled-pose replay investigations."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path
from validate_binxml import decode
from summarize_driving_log import ROOT

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('input',type=Path);p.add_argument('output',type=Path);p.add_argument('--game-root',type=Path,default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid'));args=p.parse_args()
    source=args.input.resolve();output=args.output.resolve();private=(ROOT/'local').resolve();assert source.is_relative_to(private) and output.is_relative_to(private) and source.stat().st_size<=16*1024*1024
    raw=source.read_bytes();assert raw[:4]==b'LP01';count=struct.unpack_from('<I',raw,4)[0];assert len(raw)==8+count*20
    frames=list(struct.iter_unpack('<Q3f',raw[8:]));assert all(t==i+1 and all(math.isfinite(v) for v in xyz) for i,(t,*xyz) in enumerate(frames))
    path=args.game_root/'tracks/europe/donington/route_0/progress_track.xml';_,root,_=decode(path.read_bytes())
    gates=[[[struct.unpack('<f',struct.pack('<f',float(v)))[0] for v in g.find(side).text.split()] for side in ['left','right']] for g in root.find('gates')]
    centers=[[(a+b)*.5 for a,b in zip(left,right)] for left,right in gates];crossings=[]
    for gate,(left,right) in enumerate(gates):
        edge=[b-a for a,b in zip(left,right)];width_squared=edge[0]**2+edge[2]**2;normal=[edge[2]/math.sqrt(width_squared),-edge[0]/math.sqrt(width_squared)]
        direction=sum(n*(centers[(gate+1)%len(gates)][axis]-centers[gate][axis]) for n,axis in zip(normal,[0,2]))
        if direction<0:normal=[-v for v in normal]
        for previous,current in zip(frames,frames[1:]):
            a=sum(n*(previous[axis+1]-centers[gate][axis]) for n,axis in zip(normal,[0,2]));b=sum(n*(current[axis+1]-centers[gate][axis]) for n,axis in zip(normal,[0,2]))
            forward=a<0<=b;backward=a>0>=b
            if not(forward or backward):continue
            fraction=-a/(b-a);position=[x+fraction*(y-x) for x,y in zip(previous[1:],current[1:])]
            along=sum((position[axis]-left[axis])*edge[axis] for axis in [0,2])/width_squared;height=position[1]-(left[1]+along*edge[1])
            crossings.append(dict(gate=gate,tick=current[0],forward=forward,fraction=fraction,along=along,height_offset=height,within_span=-1e-9<=along<=1+1e-9,within_height=abs(height)<=3.))
    accepted=[r for r in crossings if r['forward'] and r['within_span'] and r['within_height']]
    required=[int(s.get('gate')) for s in root.find('routes')[0]]
    report=dict(input_sha256=digest(source),progress_sha256=digest(path),script_sha256=digest(Path(__file__)),ticks=count,gates=len(gates),source_split_gates=required,accepted_forward_gates=sorted({r['gate'] for r in accepted}),missing_progress_gates=sorted(set(range(len(gates)))-{r['gate'] for r in accepted}),accepted_source_split_crossings=[r for r in accepted if r['gate'] in required],crossings=crossings,scope='Independent plane/width/height calculation on linearly interpolated earlier observed positions. Progress gates and explicit source route splits are distinguished; no original race-rule claim.')
    with output.open('x') as f:json.dump(report,f,indent=2)
    print(json.dumps({k:report[k] for k in ['ticks','gates','source_split_gates','missing_progress_gates','accepted_source_split_crossings']}))
if __name__=='__main__':main()
