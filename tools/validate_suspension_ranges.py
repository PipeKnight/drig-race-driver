"""Original range/default-slider helpers versus the native source subset."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def word(value):return f'{struct.unpack("<I",struct.pack("<f",value))[0]:08x}'
def f32(value):return struct.unpack('<f',struct.pack('<f',value))[0]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('game_root',type=Path);parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    prior=json.loads((ROOT/'local/behavior/suspension-initialization-20261007-01/validation.json').read_text())
    indices=[4,5,6,7,8,9,10,11,12,28,29]
    columns={4:228,5:230,6:232,7:236,8:234,9:238,10:240,11:242,12:224,28:246,29:248}
    expected={};cases=[];sources=[];executions=[]
    def run(exe,arguments):
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(exe),'sha256':digest(exe),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Range comparison failed')
    for i,source in enumerate(prior['source_files']):
        path=args.game_root/source['name'];assert digest(path)==source['sha256']
        source_row=next(csv.reader(path.read_text().splitlines()))
        sources.append(source)
        for fallback in (False,True):
            name=f'{"fallback" if fallback else "explicit"}_{i:03d}'
            row=list(source_row)
            if fallback:row[216]=''
            stream=io.StringIO();csv.writer(stream,lineterminator='\n').writerow(row)
            private=out/f'{name}.csv';private.write_text(stream.getvalue(),encoding='ascii')
            report=out/f'{name}.tsv'
            run(ROOT/f'target/{args.profile}/grid-suspension-ranges.exe',[private,report])
            rows=[line.split('\t') for line in report.read_text().splitlines()]
            assert rows[0][0]=='B' and int(rows[0][2])==int(not fallback)
            assert [int(r[2]) for r in rows[1:]]==indices
            expected[name]=[v for r in rows[1:] for v in r[3:]]
            number=lambda c:f32(float(row[c] or '0'))
            rear=number(116) or 1.;front=number(275) or rear
            fields=[number(194),number(95),number(96),number(93),number(94),front,rear,
                    number(212),number(213),number(214),number(215),number(79),number(80)]
            ranges=[v for index in indices for v in [number(columns[index]),number(columns[index]+1)]]
            values=[float(not fallback)]+fields+ranges;assert len(values)==36
            cases.append((name,[word(v) for v in values]))
    requests=out/'input.tsv';requests.write_text(''.join(name+'\t'+'\t'.join(vs)+'\n' for name,vs in cases),encoding='ascii')
    run(ROOT/'local/behavior/suspension-ranges-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',requests,out/'original.tsv'])
    actual={}
    for line in (out/'original.tsv').read_text().splitlines():
        cells=line.split('\t');assert cells[0]=='P' and len(cells)==35 and cells[1] not in actual;actual[cells[1]]=cells[2:]
    assert set(actual)==set(expected)
    comparisons=[]
    for name in expected:
        differences=[{'field':i,'original':a,'native':b} for i,(a,b) in enumerate(zip(actual[name],expected[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    summary={'source_files':len(sources),'cases':len(cases),'exact_cases':sum(c['exact'] for c in comparisons),
             'compared_float_words':len(cases)*33,'exact_float_words':len(cases)*33-sum(len(c['differences']) for c in comparisons)}
    report={'summary':summary,'profile':args.profile,'cases':comparisons,'executions':executions,'source_files':sources,
            'limits':'Whole original fallback range builder 998e60 and default-slider builder 9c3c30 without stubs, supplied decoded suspension fields. '
            'Eleven suspension ranges/sliders checked. Explicit cases bypass fallback builder; all 44 fallback variants are private copies with field216 empty. '
            'Original text parser/custom-range-loader and remaining nineteen setup fields not validated.'}
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(summary))
    assert summary['exact_cases']==len(cases)
if __name__=='__main__':main()
