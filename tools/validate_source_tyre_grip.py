"""Source CSV -> original decoded table -> original grip versus native source grip.

Present rows are consumed; absent rows only receive unresolved misses/inactive
queries. The private query fixture's unused absent rows contain zero padding.
"""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_tyre_table import expected

ROOT=Path(__file__).resolve().parents[1]


def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def value(w):return struct.unpack('<f',struct.pack('<I',w))[0]


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    previous_path=ROOT/'local/behavior/tyre-table-20261008-01/validation.json'
    assert digest(previous_path)=='f0338189eff5f328404eb907d3c55d937102cf065fe0f17363775a523d54a34b'
    previous=json.loads(previous_path.read_text());assert previous['exact_datasets']==41
    table_probe=ROOT/'local/behavior/tyre-table-probe.exe'
    assert digest(table_probe)=='3ac045602e0b461d868187bcb3177b5dea290d2ccdbb60272ca56fabf1e2f893'
    probe=ROOT/'local/behavior/tyre-grip-probe.exe'
    assert digest(probe)=='f07bcf8de1d1344d23bf63bd97337d8e2e53682e729107ccf2064290d53d9564'
    image=ROOT/'local/compatibility/grid-unpacked.exe';executions=[];comparisons=[];rng=random.Random(107)
    for entry in previous['comparisons']:
        label=entry['label'];assert entry['exact']
        source=(Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/cars/tyreGrip.csv') if label=='installed'
                else previous_path.parent/f'{label}.csv')
        assert digest(source)==entry['input_sha256']
        loaded=out/f'{label}-loaded.tsv'
        result=subprocess.run([str(table_probe),str(image),str(source),str(loaded)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append(dict(label=label,kind='original_loader',exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Source loader failed')
        report=loaded.read_text().splitlines();assert report==expected(source.read_bytes())
        cells=[r.split('\t') for r in report];active,count=[int(w,16) for w in cells[0][1:]]
        keys=[int(r[2],16) for r in cells[1:1+count]]
        rows=[]
        for i,r in enumerate(cells[1+count:]):
            assert r[0]=='R' and int(r[1],16)==i
            rows.append([int(w,16) for w in r[3:]] if int(r[2],16) else None)
        assert len(rows)==13
        fixture=out/f'{label}.private.bin'
        fixture.write_bytes(b'TYG1'+struct.pack('<I',count)+b''.join(struct.pack('<I',k) for k in keys)+
                            b''.join(struct.pack('<I',w) for row in rows for w in (row if row is not None else [0]*count)))
        cases=[]
        def add(name,flag,key,pending,row,speed):
            coefficients=[rng.uniform(-1,1),rng.uniform(1,4),rng.uniform(.1,2),speed]
            cases.append((name,[flag,key,pending,row]+[word(v) for v in coefficients]))
        for row,values in enumerate(rows):
            add(f'miss_{row}',1,0xdeadbeef,1,row,2.)
            add(f'inactive_{row}',0,99,0,row,-1.)
            if values is None:continue
            for i,key in enumerate(keys):
                for pending in [0,1]:
                    for flag in [0,1]:
                        for j,speed in enumerate([-1.,rng.uniform(0,5)]):
                            add(f'r{row}_k{i}_p{pending}_a{flag}_s{j}',flag,key if pending else i,pending,row,speed)
        requests=out/f'{label}-input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n' for name,words in cases))
        independent=[]
        for name,words in cases:
            flag,key,pending,row=words[:4];factor=1.
            if flag and active:
                if pending and key in keys:key,pending=keys.index(key),0
                if not pending:
                    assert rows[row] is not None
                    factor=value(rows[row][key])
            slope,maximum,base,speed=map(value,words[4:]);speed=0. if speed<0 else maximum if speed>maximum else speed
            independent.append(f'P\t{name}\t{word((slope*speed+base)*factor):08x}\t{key:08x}\t{pending:08x}')
        for kind,exe,arguments in [
            ('original_grip',probe,[image,fixture,requests,out/f'{label}-original.tsv']),
            ('native_source',ROOT/f'target/{args.profile}/grid-tyre-grip.exe',[source,requests,out/f'{label}-native.tsv','csv'])]:
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(label=label,kind=kind,exit=result.returncode,stdout=result.stdout,stderr=result.stderr,sha256=digest(exe)))
            if result.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Source grip failed')
        original=(out/f'{label}-original.tsv').read_text().splitlines();native=(out/f'{label}-native.tsv').read_text().splitlines()
        assert len(original)==len(native)==len(independent)
        differences=[dict(case=cases[i][0],original=a,native=b,independent=c) for i,(a,b,c) in enumerate(zip(original,native,independent)) if not a==b==c]
        comparisons.append(dict(label=label,source_sha256=digest(source),columns=count,present_rows=sum(r is not None for r in rows),
                                cases=len(cases),exact=len(cases)-len(differences),differences=differences))
    result=dict(profile=args.profile,comparisons=comparisons,executions=executions,
                datasets=len(comparisons),cases=sum(c['cases'] for c in comparisons),exact=sum(c['exact'] for c in comparisons),
                limits='Original CSV loader with F106 CRT adapters -> captured table -> whole F105 grip query; '
                       'native reads source CSV directly. Only present rows are consumed; absent fixture rows are unused padding. '
                       'Unknown codes do not access a row. Native matched/cached absent-row guard independently tested. '
                       'Caller supplies row selector, mechanics coefficients and speed. No production body selector/full tyre force claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','datasets','cases','exact')}))
    assert all(not c['differences'] for c in comparisons)


if __name__=='__main__':main()
