"""Source raw-key lookup joined to whole original four-wheel slowdown consumer."""
import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml')
    assert digest(source)=='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'
    _,root,_=decode(source.read_bytes());records=[];keys=[];missing=[]
    for i,m in enumerate(root.findall('MATERIAL')):
        key=m.attrib['name'].encode('ascii');assert len(key)==4;mechanics=m.find('MECHANICS')
        if mechanics is None:coeffs=[20.,40.,0.];missing.append((i,key))
        else:
            keys.append(key);a=mechanics.attrib;coeffs=[float(a.get(k,d)) for k,d in [('minimumSpeedForSlowdown',20),('speedForMaximumSlowdown',40),('slowdown',0)]]
        records.append((key,int(mechanics is not None),coeffs))
    assert len(records)==89 and len(keys)==84 and len(missing)==5
    fixture=out/'source.drg1';fixture.write_bytes(b'DRG1'+struct.pack('<I',len(records))+b''.join(key+struct.pack('<I3f',present,*coeffs) for key,present,coeffs in records))
    retained=ROOT/'local/behavior/surface-drag-20261008-01/input.tsv'
    assert digest(retained)=='17e095c73ab5fc33eddc99f0c29c0dd26c3855154c51628e68825bdd863c954b'
    names=[];request=[];coverage=set();fallbacks=0;expected_indices={}
    choices=keys+[b'MUD\0',b'MISS',b'NONE',b'\0\0\0\0']
    for i,line in enumerate(retained.read_text().splitlines()):
        cells=line.split('\t');assert len(cells)==79;selected=[];indices=[]
        for w in range(4):
            code=choices[(i*7+w*13)%len(choices)];selected.append(int.from_bytes(code,'little'))
            present=struct.unpack('<f',struct.pack('<I',int(cells[38+w*12],16)))[0]!=0.
            if present:
                matching=[j for j,(key,_,_) in enumerate(records) if code==key]
                index=matching[0] if matching else 0;assert records[index][1]
                coverage.add(index);fallbacks+=int(not matching)
                indices.append(index)
            else:indices.append(0xffffffff)
        expected_indices[cells[0]]=indices
        names.append(cells[0]);request.append(line+'\t'+'\t'.join(f'{x:08x}' for x in selected)+'\n')
    requests=out/'input.tsv';requests.write_text(''.join(request));executions=[]
    probe=ROOT/'local/behavior/surface-drag-probe.exe';native=ROOT/f'target/{args.profile}/grid-surface-drag.exe'
    for kind,exe,arguments in [
        ('original',probe,[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv',fixture]),
        ('native',native,[requests,out/'native.tsv',source])]:
        run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
        if run.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Source surface drag run failed')
    def rows(path):
        result={}
        for line in path.read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==18 and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native_rows=rows(out/'original.tsv'),rows(out/'native.tsv');assert original.keys()==native_rows.keys()==set(names)
    assert all(original[name][12:]==[f'{i:08x}' for i in expected_indices[name]] for name in names)
    differences=[dict(case=name,field=i,original=a,native=b) for name in original for i,(a,b) in enumerate(zip(original[name],native_rows[name])) if a!=b]
    guards=[]
    for index,key in missing:
        cells=request[0].strip().split('\t');cells[38]='3f800000';cells[79]=f'{int.from_bytes(key,"little"):08x}'
        guard_input=out/f'guard-{index}-input.tsv';guard_input.write_text('\t'.join(cells)+'\n')
        for kind,exe,arguments in [('original',probe,[ROOT/'local/compatibility/grid-unpacked.exe',guard_input,out/f'guard-{index}-original.tsv',fixture]),('native',native,[guard_input,out/f'guard-{index}-native.tsv',source])]:
            run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards.append(dict(index=index,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
            assert run.returncode==(20 if kind=='original' else 1)
    result=dict(profile=args.profile,executions=executions,cases=len(names),words=len(names)*16,
                exact_cases=len(names)-len({d['case'] for d in differences}),differences=differences,
                source_sha256=digest(source),retained_input_sha256=digest(retained),fixture_sha256=digest(fixture),
                material_indices=sorted(coverage),materials=len(coverage),fallback_contacts=fallbacks,missing_mechanics_guards=guards,
                limits='Whole original b3b0c0 resolver and 9dfc50 slowdown, original cross/impulse helpers; no code patches/stubs/live injection. '
                'Private lazy-normalization data page writable/restored. Original receives independently decoded source fixture; native reads source XML directly. '
                'Source flag73 initializer false. Supplied contacts/relative levers/body/speed/additive scalars, not actual query-to-force synchronization. '
                'Missing MECH guards avoid undefined original dereference; no original XML loader or complete live driving/cadence claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','cases','words','exact_cases','materials','fallback_contacts')}));print('different_words='+str(len(differences)))
    assert not differences and len(coverage)==84
if __name__=='__main__':main()
