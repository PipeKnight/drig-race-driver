"""Hash-guarded source/setup/cap chain replay against retained runtime fields."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def f32(value):return struct.unpack('<f',struct.pack('<f',value))[0]
def word(value):return f'{struct.unpack("<I",struct.pack("<f",value))[0]:08x}'
def value(word):return struct.unpack('<f',bytes.fromhex(word)[::-1])[0]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('game_root',type=Path)
    parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):raise ValueError('Private output only')
    out.mkdir(exist_ok=False)
    prior=json.loads((ROOT/'local/behavior/suspension-initialization-20261007-01/validation.json').read_text())
    source_results={};executions=[]
    def run(exe,arguments):
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        executions.append({'path':str(exe),'sha256':digest(exe),'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
        if result.returncode:
            (out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Runtime validation failed')
    indices=[4,5,6,7,8,9,10,11,12,28,29]
    custom={4:228,5:230,6:232,7:236,8:234,9:238,10:240,11:242,12:224,28:246,29:248}
    for i,s in enumerate(prior['source_files']):
        path=args.game_root/s['name'];assert digest(path)==s['sha256']
        row=next(csv.reader(path.read_text().splitlines()));number=lambda c:f32(float(row[c] or '0'))
        report=out/f'{i:03d}.tsv'
        run(ROOT/f'target/{args.profile}/grid-suspension-ranges.exe',[path,report])
        rows=[line.split('\t') for line in report.read_text().splitlines()]
        explicit=all(row[c] for c in [*range(216,254),*range(264,268)])
        damper=[number(93),number(94)];threshold=[number(212),number(213)];ratio=[number(214),number(215)]
        neg_rear=number(116) or 1.;neg_front=number(275) or neg_rear
        high=[f32(ratio[0]*damper[0]),f32(damper[1]*ratio[1])];height=number(194)
        defaults={4:number(95),5:number(96),6:damper[0],7:f32(neg_front*damper[0]),8:damper[1],9:f32(damper[1]*neg_rear),
                  10:f32(f32(high[0]+high[1])*.5),11:f32(f32(threshold[0]+threshold[1])*.5),12:height,28:number(79),29:number(80)}
        expected=[['B','0',str(int(explicit)),*[word(v) for v in [height,*damper,*threshold,*ratio]]]]
        upper=[1.]*30;lower=[0.]*30;sliders=[0.]*30
        for index in indices:
            base=defaults[index]
            if explicit:lo,hi=number(custom[index]),number(custom[index]+1)
            elif index==12:lo,hi=f32(height-.5),f32(height+.5)
            elif index in (10,11):
                pair=high if index==10 else threshold;lo,hi=f32(min(pair)*f32(.8)),f32(max(pair)*f32(1.2))
            else:lo,hi=f32(base*f32(.8)),f32(base*f32(1.2))
            slider=f32(f32(min(max(base,lo),hi)-lo)/f32(hi-lo)) if lo<hi else 0.
            lower[index],upper[index],sliders[index]=lo,hi,slider
            expected.append(['R','0',str(index),word(lo),word(hi),word(slider)])
        assert rows==expected,s['name']
        source_results[s['name']]={'upper':upper,'lower':lower,'sliders':sliders,'base':damper+threshold+ratio+[height],
                                  'row':row,'sha256':s['sha256'],'explicit':explicit}
    cases=[];observed={};snapshots=[]
    for file,sha in [('general-vehicle-manager-20261007T0837131885850Z.json','4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
                     ('general-vehicle-manager-20261007T0837468621304Z.json','e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5')]:
        path=ROOT/'local/behavior'/file;assert digest(path)==sha
        snapshot=json.loads(path.read_text(encoding='utf-8-sig'));assert snapshot['read_only']
        blocks={b['name']:b for b in snapshot['blocks']};assert len(blocks)==len(snapshot['blocks'])
        snapshots.append({'file':file,'sha256':sha})
        for observation in [o for o in prior['observations'] if o['snapshot']==file]:
            index=observation['index'];source=source_results[observation['source']]
            def read(name,length):
                block=blocks[f'car_{index}-{name}'];assert block['success'] and block['read']==block['length']==length
                return bytes.fromhex(block['hex'])
            wheels=read('wheel_records',4*0x150);mass=struct.unpack_from('<f',read('mass_inertia_fields',0x30))[0]
            dimension=struct.unpack_from('<f',read('dimension_fields',0x40),4)[0]
            at=lambda w,o:struct.unpack_from('<f',wheels,w*0x150+o)[0]
            row=source['row'];number=lambda c:f32(float(row[c] or '0'))
            denominator=[max(number([88,88,87,87][w]),f32(.3)) for w in range(4)]
            # Replay stored geometry; this is not an independent deformation initialization.
            values=source['upper']+source['lower']+source['sliders']+source['base']+denominator+[at(w,4) for w in range(4)]+[at(w,8) for w in range(4)]+[mass,f32(9.81)]
            name=f'car_{len(cases):02d}';assert len(values)==111
            cases.append((name,values,dimension))
            observed[name]=[word(source['base'][-1])]+[word(at(w,o)) for w in range(4) for o in [0x5c,0x60,0x64,0x68,0x114,0x70,0x4c,4,0x54]]
    for name,values in [('input.tsv',lambda v,d:v),('capped-input.tsv',lambda v,d:v+[d])]:
        (out/name).write_text(''.join(case+'\t'+'\t'.join(word(v) for v in values(vs,dim))+'\n' for case,vs,dim in cases),encoding='ascii')
    run(ROOT/'local/behavior/suspension-prepare-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',out/'input.tsv',out/'original.tsv'])
    run(ROOT/f'target/{args.profile}/grid-suspension-prepare.exe',[out/'capped-input.tsv',out/'native.tsv'])
    def outputs(name):
        result={}
        for line in (out/name).read_text().splitlines():
            cells=line.split('\t');assert cells[0]=='P' and len(cells)==39 and cells[1] not in result;result[cells[1]]=cells[2:]
        return result
    original,native=outputs('original.tsv'),outputs('native.tsv');assert set(original)==set(native)==set(observed)
    comparisons=[]
    for name,_,dim in cases:
        expected=list(original[name]);cap=f32(dim+f32(.05))
        for w in range(4):
            offset=1+w*9+6;expected[offset]=word(min(value(expected[offset]),cap))
        assert native[name]==expected,'native differs from original setup plus inspected cap'
        differences=[{'field':i,'computed':a,'retained':b} for i,(a,b) in enumerate(zip(native[name],observed[name])) if a!=b]
        comparisons.append({'name':name,'exact':not differences,'differences':differences})
    summary={'source_files':len(source_results),'explicit_range_sources':sum(s['explicit'] for s in source_results.values()),
             'observations':len(cases),'exact_observations':sum(c['exact'] for c in comparisons),'compared_float_words':len(cases)*37,
             'exact_float_words':len(cases)*37-sum(len(c['differences']) for c in comparisons)}
    report={'summary':summary,'profile':args.profile,'executions':executions,'snapshots':snapshots,'cases':comparisons,
            'limits':'Hash-guarded source subset/default sliders independently calculated; original CSV parser/default-slider helper not executed. '
            'Whole setup consumer compared for 37 suspension/height words; constructor cap is instruction-derived plus recorded dimension operand. '
            'Geometry replayed from retained runtime fields, not source-only deformation. Source identity references prior unique 37-word matching. '
            'No synchronized setup or production preparation/driving claim.'}
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(summary))

if __name__=='__main__':main()
