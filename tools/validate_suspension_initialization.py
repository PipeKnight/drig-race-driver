"""Source initialization versus whole wheel initializer and observed runtime words."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def word(value):
    return f'{struct.unpack("<I",struct.pack("<f",value))[0]:08x}'

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('game_root',type=Path)
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    previous=json.loads((ROOT/'local/behavior/native-body-prelude-validation/validation.json').read_text())
    expected_sources={v['source']:v['sha256'] for v in previous['csv_files']}
    assert len(expected_sources)==44
    sources=[]
    for path in sorted((args.game_root/'cars').rglob('*.csv')):
        name=str(path.relative_to(args.game_root))
        if name not in expected_sources:
            continue
        assert digest(path)==expected_sources[name]
        rows=[row for row in csv.reader(path.read_text().splitlines()) if row]
        assert len(rows)==1
        row=rows[0]
        assert (len(row)==360 or len(row)==361 and not row[-1]) and row[2]=='Standard'
        number=lambda column:f32(float(row[column] or '0'))
        rear=number(116) or 1.
        front=number(275) or rear
        inputs=[]
        expected=[]
        for columns,negative in [([96,94,80,130,213,215,88,90],rear),([95,93,79,137,212,214,87,89],front)]:
            values=[number(c) for c in columns[:6]]+[negative,number(columns[6]),number(columns[7])]
            inputs+=values
            expected.append([word(v) for v in values[:7]+[max(values[7],f32(.3)),0.,values[8],values[7]]])
        report=out/f'{len(sources):03d}.tsv'
        native=ROOT/f'target/{args.profile}/grid-suspension-tuning.exe'
        result=subprocess.run([str(native),str(path),str(report)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        assert result.returncode==0,result.stderr
        actual=[line.split('\t') for line in report.read_text().splitlines()]
        assert actual==[['S','0',str(i)]+values for i,values in enumerate(expected)]
        sources.append({'name':name,'sha256':digest(path),'inputs':inputs,'expected':expected,'row':row})
    assert len(sources)==44
    requests=out/'input.tsv'
    requests.write_text(''.join(f'file_{i:03d}\t'+'\t'.join(word(v) for v in source['inputs'])+'\n'
        for i,source in enumerate(sources)),encoding='ascii')
    probe=ROOT/'local/behavior/suspension-init-probe.exe'
    image=ROOT/'local/compatibility/grid-unpacked.exe'
    result=subprocess.run([str(probe),str(image),str(requests),str(out/'original.tsv')],
        cwd=ROOT,capture_output=True,text=True,timeout=30)
    assert result.returncode==0,result.stdout+result.stderr
    actual=[line.split('\t') for line in (out/'original.tsv').read_text().splitlines()]
    assert actual==[['P',f'file_{i:03d}']+sum(source['expected'],[]) for i,source in enumerate(sources)]
    observations=[]
    offsets=[0x5c,0x60,0x70,0x58,0x64,0x68,0x114,0x4c,0x54,0x48,0x50]
    names=['spring','damper','axle_coupling','bump','speed_threshold','high_speed_ratio',
        'negative_speed_scale','travel_denominator','preload','lower_stop','upper_stop']
    # Independently identify each current car from the earlier verified prelude;
    # do not infer identity from mass alone or reuse an unguarded index label.
    for file,expected_hash in [
        ('general-vehicle-manager-20261007T0837131885850Z.json','4dadad1abfa571391631abdd116f9e7b222571a8154aa295ba70412f0cfb47e4'),
        ('general-vehicle-manager-20261007T0837468621304Z.json','e41658a6ee2ef49372f31b3f744b529d0f90656859f9116559449cbbe847a8e5')]:
        path=ROOT/'local/behavior'/file
        assert digest(path)==expected_hash
        snapshot=json.loads(path.read_text(encoding='utf-8-sig'))
        blocks={b['name']:b for b in snapshot['blocks']}
        assert len(blocks)==len(snapshot['blocks']) and snapshot['read_only']
        for car in snapshot['managers'][0]['cars']:
            def read(name,length):
                b=blocks[f'car_{car["index"]}-{name}']
                assert b['success'] and b['length']==b['read']==length
                return bytes.fromhex(b['hex'])
            mass=read('mass_inertia_fields',0x30)
            wheels=read('wheel_records',4*0x150)
            body_words=[f'{struct.unpack_from("<I",mass,o)[0]:08x}' for o in (0,4,8,12,16)]
            candidates=[]
            for source in sources:
                row=source['row'];number=lambda c:f32(float(row[c]))
                expected_body=[word(number(3)),word(1./number(3))]+[word(number(c)) for c in (4,5,6)]
                columns=[[146,84,86,88,90,96,94,80],[145,83,85,87,89,95,93,79]]
                base_offsets=[0xe0,0x108,0x100,0x50,0x48,0x5c,0x60,0x70]
                if expected_body==body_words and all(
                    word(number(c))==f'{struct.unpack_from("<I",wheels,w*0x150+o)[0]:08x}'
                    for w in range(4) for c,o in zip(columns[w//2],base_offsets)):
                    candidates.append(source)
            assert len(candidates)==1
            source=candidates[0]
            differences=[]
            exact=0
            for w in range(4):
                for index,offset in enumerate(offsets):
                    actual_word=f'{struct.unpack_from("<I",wheels,w*0x150+offset)[0]:08x}'
                    expected_word=source['expected'][w//2][index]
                    if actual_word==expected_word:
                        exact+=1
                    else:
                        differences.append({'wheel':w,'field':names[index],'initial':expected_word,'runtime':actual_word})
            observations.append({'snapshot':file,'index':car['index'],'source':source['name'],
                'exact_words':exact,'differences':differences})
    summary={'source_files':44,'original_initializer_exact_words':44*22,
        'runtime_observations':len(observations),'runtime_compared_words':len(observations)*44,
        'runtime_exact_words':sum(o['exact_words'] for o in observations)}
    report={'summary':summary,'profile':args.profile,'source_files':[
        {'name':s['name'],'sha256':s['sha256']} for s in sources],
        'probe_sha256':digest(probe),'image_sha256':digest(image),'native_sha256':digest(native),
        'observations':observations,'limits':'Whole original wheel initializer/reset without stubs; supplied decoded coefficient record. '
        'Original CSV decoder is not executed; native source conversion/defaults independently checked against Python. '
        'Runtime observations are paused, not initialization boundaries; changed fields remain explicit. No production suspension preparation or driving claim.'}
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(summary))
    print('changed_runtime_fields='+str(sorted({d['field'] for o in observations for d in o['differences']})))

if __name__=='__main__':
    main()
