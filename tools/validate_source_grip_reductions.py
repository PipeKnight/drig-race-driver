"""Independent Standard CSV reduction defaults joined to whole original 98db40.

Original source decoder/constructor linkage is static evidence. Original helper
receives independently converted source values; this does not execute that decoder.
"""
import argparse
import csv
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def reduction(text):
    w=word(float(text or '0'))
    return word(.5) if not w&0x7fffffff else w
def main():
    parser=argparse.ArgumentParser();parser.add_argument('game_root',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    previous=json.loads((ROOT/'local/behavior/native-body-prelude-validation/validation.json').read_text())
    datasets=[]
    for entry in previous['csv_files']:
        path=args.game_root/entry['source'];assert digest(path)==entry['sha256']
        rows=[r for r in csv.reader(path.read_text().splitlines()) if r];assert len(rows)==1
        row=rows[0];assert row[2]=='Standard' and len(row) in (360,361)
        datasets.append((path,row,True))
    assert len(datasets)==44
    for i,pair in enumerate([('',''),('0','-0'),('1e-46','-1e-46'),('1e-45','-1e-45'),
                            ('.100000001','.99999999'),('-.7','1.2'),('2','-2'),
                            ('0.5000000298023224','0.5000000298023225'),
                            ('1.1754942e-38','-1.1754942e-38'),('1.3','0.4')]):
        row=datasets[0][1].copy();row[178:180]=pair;path=out/f'authored-{i:02d}.csv'
        path.write_text(','.join(row)+'\r\n',encoding='ascii',newline='');datasets.append((path,row,False))
    probe=ROOT/'local/behavior/grip-history-probe.exe'
    assert digest(probe)=='1fe31f67e12e1a8bce07ee788913bf800e723afc68c107495ce2d80ef41ae8a9'
    native=ROOT/f'target/{args.profile}/grid-grip-history.exe'
    rng=random.Random(111);reports=[];total=0;differences=[];executions=[]
    for ordinal,(source,row,installed) in enumerate(datasets):
        expected=[reduction(row[c]) for c in (178,179)]
        original_rows=[];native_rows=[]
        for wheel in range(4):
            for flags in range(4):
                for sign,z in enumerate([-1.,-0.,1.]):
                    v=[float(wheel),float(flags&1),float(flags>>1),*[rng.uniform(0,1) for _ in range(3)],
                       rng.uniform(0,1),rng.uniform(.5,1.5),.1234,-.65,z,rng.uniform(-2,2)]
                    words=[word(x) for x in v];name=f'case_{wheel}_{flags}_{sign}'
                    native_rows.append(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n')
                    words[8:10]=expected
                    original_rows.append(name+'\t'+'\t'.join(f'{w:08x}' for w in words)+'\n')
        incoming=out/f'{ordinal:03d}-input.tsv';incoming.write_text(''.join(original_rows))
        native_input=out/f'{ordinal:03d}-native-input.tsv';native_input.write_text(''.join(native_rows))
        for kind,exe,arguments in [
            ('original',probe,[ROOT/'local/compatibility/grid-unpacked.exe',incoming,out/f'{ordinal:03d}-original.tsv']),
            ('native',native,[native_input,out/f'{ordinal:03d}-native.tsv',source])]:
            run=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(dataset=ordinal,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Source grip reductions run failed')
        a=(out/f'{ordinal:03d}-original.tsv').read_text().splitlines();b=(out/f'{ordinal:03d}-native.tsv').read_text().splitlines()
        assert len(a)==len(b)==48
        for reference,actual in zip(a,b):
            aa,bb=reference.split('\t'),actual.split('\t');assert aa[:2]==bb[:2] and len(aa)==len(bb)==4
            differences.extend(dict(dataset=ordinal,case=aa[1],field=i,original=x,native=y) for i,(x,y) in enumerate(zip(aa[2:],bb[2:])) if x!=y)
        total+=len(a);reports.append(dict(source=str(source),sha256=digest(source),installed=installed,lexemes=row[178:180],reduction_words=[f'{w:08x}' for w in expected],cases=len(a)))
    result=dict(profile=args.profile,datasets=len(datasets),installed_datasets=44,authored_datasets=10,cases=total,words=total*2,
                exact_cases=total-len({(d['dataset'],d['case']) for d in differences}),differences=differences,
                probe_sha256=digest(probe),native_sha256=digest(native),sources=reports,executions=executions,
                limits='Static original CSV column/default/constructor-copy provenance. Original whole 98db40 '
                'receives independently decoded source scalars; native reads actual CSV directly. '
                'Native request contains deliberately different placeholder reduction words. '
                'No original whole CSV decoder invocation, source flag, live values or complete force-path claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ('profile','datasets','cases','words','exact_cases')}));print('different_words='+str(len(differences)))
    assert not differences
if __name__=='__main__':main()
