"""Direct Rust NED/PSSG reader versus independently supplied whole original setup."""
import argparse
import json
import struct
import subprocess
from pathlib import Path
import validate_deformation_layout as layout
import validate_deformation_initialization as initialization
from compare_source_deformation_layout import candidates,STEAM


def source_rows():
    rows,identities=initialization.cases()
    return [r for r in rows if r['name'] in ['lac-source-candidate','bmw-source-candidate']],identities


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],required=True)
    args=parser.parse_args();out=args.output.resolve();assert out.is_relative_to((layout.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows,identities=source_rows();source=out/'original-input.bin';source.write_bytes(initialization.request(rows));answers={};runs={}
    for kind in ['original','repeat']:
        target=out/f'{kind}.bin';run=subprocess.run([str(initialization.PROBE),str(layout.IMAGE),str(source),str(target)],cwd=layout.ROOT,capture_output=True,text=True,timeout=120)
        runs[kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,executable_sha256=layout.digest(initialization.PROBE));assert run.returncode==0 and not run.stderr
        assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run.stdout;answers[kind]=target.read_bytes()
    assert answers['original']==answers['repeat'] and len(answers['original'])==8+len(rows)*initialization.ANSWER
    observations=[];exe=layout.ROOT/f'target/{args.profile}/grid-deformation-source.exe'
    for i,row in enumerate(rows):
        stem=row['name'].split('-')[0];choices,_=candidates(stem,'highLOD');choice=[c for c in choices if c['name']=='highLOD-LOD1_no-aerial'][0]
        expected=b'DS01'+struct.pack('<II',len(choice['selected_joints']),len(choice['excluded_joints']))+struct.pack('<8f',*choice['bounds'])+answers['original'][8+i*initialization.ANSWER:8+(i+1)*initialization.ANSWER]
        for kind in ['rust','rust-repeat']:
            target=out/f'{stem}-{kind}.bin';run=subprocess.run([str(exe),str(STEAM),stem,str(target)],cwd=layout.ROOT,capture_output=True,text=True,timeout=120)
            runs[stem+'-'+kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,executable_sha256=layout.digest(exe));assert run.returncode==0 and not run.stderr
            actual=target.read_bytes();assert actual==expected,(stem,kind,[(hex(j),actual[j:j+4].hex(),expected[j:j+4].hex()) for j in range(0,len(expected),4) if actual[j:j+4]!=expected[j:j+4]][:12])
        observations.append(dict(model=stem,complete_bytes=len(expected),selected_joints=len(choice['selected_joints']),excluded_aerials=len(choice['excluded_joints']),exact=True))
    checks=[]
    for name,root,stem in [('unknown-model',STEAM,'unknown'),('missing-source',out,'lac')]:
        target=out/f'guard-{name}.bin';run=subprocess.run([str(exe),str(root),stem,str(target)],cwd=layout.ROOT,capture_output=True,text=True,timeout=120)
        assert run.returncode==1 and not target.exists();checks.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_exists=False))
    target=out/'existing.bin';target.write_bytes(b'owned sentinel');run=subprocess.run([str(exe),str(STEAM),'lac',str(target)],cwd=layout.ROOT,capture_output=True,text=True,timeout=120)
    assert run.returncode==1 and target.read_bytes()==b'owned sentinel';checks.append(dict(name='existing',exit=run.returncode,preserved=True,stderr=run.stderr))
    report=dict(profile=args.profile,observations=observations,identities=identities,executions=runs,guards=checks,
                files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                scope='Direct Rust purchased NED/PSSG->whole cold deformation constructor versus independent Python reader->complete original initializer. Explicit LOD1_without_aerial prototype policy; no original XML/resource factory, dirty vehicle rebuild or active driving deformation.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(observations))


if __name__=='__main__':main()
