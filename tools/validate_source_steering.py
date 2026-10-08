"""Independent CSV angle decoding joined to whole original steering routines.

Original whole CSV decoder is not invoked; its linkage is static evidence.
"""
import argparse
import csv
import hashlib
import json
import struct
import subprocess
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    manifest=json.loads((ROOT/'local/behavior/native-body-prelude-validation/validation.json').read_text())
    game=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
    datasets=[]
    for entry in manifest['csv_files']:
        path=game/entry['source']
        assert digest(path)==entry['sha256']
        rows=[r for r in csv.reader(path.read_text().splitlines()) if r]
        assert len(rows)==1 and rows[0][2]=='Standard'
        datasets.append((path,rows[0],True))
    assert len(datasets)==44
    for i,text in enumerate(['','0','-0','1e-46','-1e-46','1e-43','1e-30',
                             '0.1000000007','0.100000004','30.000000953674315','30.00000095367432','45']):
        row=datasets[0][1].copy()
        row[77]=text
        path=out/f'authored-{i:02d}.csv'
        path.write_text(','.join(row)+'\r\n',encoding='ascii',newline='')
        datasets.append((path,row,False))
    seed_dir=ROOT/'local/behavior/steering-pipeline-20261008-01'
    seed_report=json.loads((seed_dir/'validation.json').read_text())
    assert seed_report['cases']==1833 and not seed_report['differences']
    seed_input=seed_dir/'input.tsv'
    assert digest(seed_input)==seed_report['input_sha256']
    seeds=[line.split('\t')[1:] for line in seed_input.read_text().splitlines()]
    probe=ROOT/'local/behavior/steering-coefficients-probe.exe'
    assert digest(probe)=='239af3cc2ee916c81e2cbec2e428a2c6508bac67a0d2e6b0cbeae5132e8219a4'
    native=ROOT/f'target/{args.profile}/grid-steering-coefficients.exe'
    scale=f32(struct.unpack('<f',bytes.fromhex('35fa8e3c'))[0])
    reports,executions,differences=[],[],[]
    for ordinal,(source,row,installed) in enumerate(datasets):
        expected=word(float(row[77] or '0')*scale)
        early=word(f32(float(row[77] or '0'))*scale)
        original_rows,native_rows=[],[]
        for i in range(48):
            words=seeds[(i*37+ordinal*7)%len(seeds)].copy()
            # Source mode must ignore the angle placeholder. Other state stays supplied.
            words[4]='3eaaaaab'
            name=f'condition_{i}'
            native_rows.append(name+'\t'+'\t'.join(words)+'\n')
            words[4]=f'{expected:08x}'
            original_rows.append(name+'\t'+'\t'.join(words)+'\n')
        incoming=out/f'{ordinal:03d}-original-input.tsv'
        native_input=out/f'{ordinal:03d}-native-input.tsv'
        incoming.write_text(''.join(original_rows))
        native_input.write_text(''.join(native_rows))
        for kind,exe,argv in [
            ('original',probe,[ROOT/'local/compatibility/grid-unpacked.exe',incoming,out/f'{ordinal:03d}-original.tsv','--geometry']),
            ('native',native,[native_input,out/f'{ordinal:03d}-native.tsv','--geometry',source])]:
            run=subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(dataset=ordinal,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2))
                raise RuntimeError('Source steering execution failed')
        original=(out/f'{ordinal:03d}-original.tsv').read_text().splitlines()
        actual=(out/f'{ordinal:03d}-native.tsv').read_text().splitlines()
        assert len(original)==len(actual)==48
        for a,b in zip(original,actual):
            aa,bb=a.split('\t'),b.split('\t')
            assert aa[:2]==bb[:2] and len(aa)==len(bb)==27 and aa[2]==f'{expected:08x}'
            differences.extend(dict(dataset=ordinal,case=aa[1],field=i,original=x,native=y) for i,(x,y) in enumerate(zip(aa[2:],bb[2:])) if x!=y)
        reports.append(dict(source=str(source),sha256=digest(source),installed=installed,lexeme=row[77],
                            angle_word=f'{expected:08x}',premature_f32_word=f'{early:08x}',cases=48))
    guards=[]
    for name,value,multiple,style in [('overflow','1e300',False,'Standard'),('nonfinite','NaN',False,'Standard'),
                                      ('multiple','33',True,'Standard'),('unsupported','33',False,'Other')]:
        row=datasets[0][1].copy()
        row[77],row[2]=value,style
        source=out/f'guard-{name}.csv'
        source.write_text((','.join(row)+'\n')*(2 if multiple else 1))
        output=out/f'guard-{name}.tsv'
        run=subprocess.run([str(native),str(native_input),str(output),'--geometry',str(source)],cwd=ROOT,
                           capture_output=True,text=True,timeout=30)
        guards.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_exists=output.exists()))
        assert run.returncode!=0 and not output.exists()
    count=len(datasets)*48
    result=dict(profile=args.profile,datasets=len(datasets),installed_datasets=44,authored_datasets=12,cases=count,fields=count*25,
                exact_cases=count-len({(d['dataset'],d['case']) for d in differences}),differences=differences,
                premature_f32_datasets=sum(r['angle_word']!=r['premature_f32_word'] for r in reports),sources=reports,executions=executions,guards=guards,
                probe_sha256=digest(probe),native_sha256=digest(native),input_fixture_sha256=digest(seed_input),
                limits='Static original decoder/column/conversion linkage. Original whole98ebf0->9b3110/98f790 receives independently converted angle; native reads CSV. '
                       'No original wholeCSVdecoder, assistance, actual device input or source-matched complete car state claim. Geometry and other tuning/state/delta remain supplied.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','datasets','cases','fields','exact_cases','premature_f32_datasets')}))
    print('different_fields='+str(len(differences)))
    assert not differences
if __name__=='__main__':main()
