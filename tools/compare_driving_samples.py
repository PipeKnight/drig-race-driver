"""Read-only private replay comparison for presentation-only prototype changes."""
import argparse
import hashlib
import json
from pathlib import Path
from summarize_driving_log import FIELDS,ROOT

def read(path):
    path=path.resolve();assert path.is_relative_to((ROOT/'local').resolve()) and path.stat().st_size<=128*1024*1024
    raw=path.read_bytes();lines=raw.decode('utf-8').splitlines();rows=[];updates=[]
    for line in lines:
        if line.startswith(('drive_sample ','drive_deformation_sample ')):
            row=dict(FIELDS.findall(line));assert row['version']=='1' and row['event']!='failure'
            row.pop('wall_seconds',None);rows.append((line.split(' ')[0],row))
        elif line.startswith('drive_body_projection '):updates.append(dict(FIELDS.findall(line)))
    assert rows and rows[-1][1]['event']=='exit'
    return rows,updates,dict(path=str(path),sha256=hashlib.sha256(raw).hexdigest(),build_revision=next((s.split('revision=',1)[1] for s in lines if s.startswith('prototype_build revision=')),None))

def main():
    p=argparse.ArgumentParser();p.add_argument('baseline',type=Path);p.add_argument('current',type=Path);p.add_argument('output',type=Path);args=p.parse_args()
    before,_,old=read(args.baseline);after,updates,new=read(args.current)
    assert before==after,'Logged physical/deformation fields differ'
    result=dict(physical_and_deformation_samples_exact=True,compared_records=len(before),inputs=[old,new],script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),mesh_updates=len(updates),first_mesh_update=updates[0] if updates else None,first_changed_mesh_update=next((r for r in updates if int(r['changed_meshes'])>0),None),last_mesh_update=updates[-1] if updates else None,scope='All logged physical/deformation fields exact excluding wall time; sampled evidence, not every authoritative byte or original rendering equivalence.')
    target=args.output.resolve();assert target.is_relative_to((ROOT/'local').resolve())
    with target.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)
    print(json.dumps(result))
if __name__=='__main__':main()
