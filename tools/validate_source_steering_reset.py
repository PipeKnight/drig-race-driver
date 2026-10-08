"""Native HDM source reader joined to whole original reset with independently supplied fields."""
import argparse
import hashlib
import json
import random
import re
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from validate_binxml import entries
from validate_steering_reset import PROBE_SHA, PE_SHA

ROOT = Path(__file__).resolve().parents[1]
FIELDS = [('WHEEL','maxMisalignAngle',0.),('SUSPENSION','maxWheelLockScale',0.),
          ('WHEEL','maxSteerWobbleScale',0.),('WHEEL','damageToStartBaseEffect',0.),
          ('WHEEL','damageToHaveFullEffect',struct.unpack('<f',bytes.fromhex('3333733f'))[0]),
          ('WHEEL','damageMinEffect',struct.unpack('<f',bytes.fromhex('cdcccc3d'))[0]),
          ('SUSPENSION','damageToStartEffect',0.)]


def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v): return f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    probe = ROOT/'local/behavior/steering-reset-probe.exe'
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    native = ROOT/f'target/{args.profile}/grid-steering-reset.exe'
    assert digest(probe) == PROBE_SHA and digest(image) == PE_SHA
    baseline = json.loads((ROOT/'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    records = [r for r in baseline['resources'] if Path(r['relative_path']).name == 'raceload.jpk']
    assert len(records)==1
    record = records[0]
    archive = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')/record['relative_path']
    assert digest(archive) == record['steam_sha256']
    datasets = []
    for name,payload,raw in entries(archive.read_bytes()):
        if name not in ['standard.hdm','openwheeler.hdm']: continue
        assert raw
        source = out/name
        source.write_bytes(payload)
        root = ET.fromstring(re.sub(rb'<![^>]*>',b'',payload))
        datasets.append((source,root,True))
    assert len(datasets)==2
    controls = ['', '0','-0','1e-46','-1e-46','1e-43','1e-30',
                '0.100000004','0.49999999999999994','0.5000000000000001',
                '1','-1','2','-2','0.95','0.9500000178813934','3','-3']
    for i,text in enumerate(controls):
        root = ET.Element('VEHICLEHANDLINGDAMAGE')
        ET.SubElement(root,'SUSPENSION',{'maxWheelLockScale': ['0','.8','1'][i%3], 'damageToStartEffect':['-1','0','1'][i%3]})
        attributes = {'maxMisalignAngle':text or '0','maxSteerWobbleScale':text or '0',
                      'damageToStartBaseEffect':['0','.65','-.2'][i%3],
                      'damageToStartSteeringWobble':'999'}
        if i%2: attributes.update(damageToHaveFullEffect='1.0000000000000001',damageMinEffect=text)
        ET.SubElement(root,'WHEEL',attributes)
        source = out/f'authored-{i}.hdm'
        source.write_bytes(ET.tostring(root))
        datasets.append((source,root,False))
    rng = random.Random(129)
    seeds = [0,1,65534,65535,0x7fffffff,0x80000000,0xfffffffe,0xffffffff]+[rng.getrandbits(32) for _ in range(120)]
    reports,executions,differences = [],[],[]
    for ordinal,(source,root,installed) in enumerate(datasets):
        parameters = []
        for node,key,default in FIELDS:
            selected = root.findall(node)
            assert len(selected)==1
            parameters.append(word(float(selected[0].get(key,str(default)))))
        original_input,native_input = out/f'{ordinal}-original-input.tsv',out/f'{ordinal}-native-input.tsv'
        original_input.write_text(''.join(f'case_{i}\t{seed:08x}\t'+'\t'.join(parameters+[word(float(i%2))])+'\n' for i,seed in enumerate(seeds)))
        native_input.write_text(''.join(f'case_{i}\t{seed:08x}\t'+'\t'.join([word(.333333)]*7+[word(float(i%2))])+'\n' for i,seed in enumerate(seeds)))
        original_output,native_output = out/f'{ordinal}-original.tsv',out/f'{ordinal}-native.tsv'
        for kind,exe,argv in [('original',probe,[image,original_input,original_output]),
                              ('native',native,[native_input,native_output,'--source',source])]:
            run = subprocess.run([str(exe),*map(str,argv)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(dataset=ordinal,kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2))
                raise RuntimeError('Source steering reset execution failed')
        a,b = original_output.read_text().splitlines(),native_output.read_text().splitlines()
        assert len(a)==len(b)==128
        for i,(a,b) in enumerate(zip(a,b)):
            a,b = a.split('\t'),b.split('\t')
            assert len(a)==20 and len(b)==27 and a[:2]==b[:2]
            expected = parameters+a[2:]
            differences += [dict(dataset=ordinal,case=i,field=j,original=x,native=y) for j,(x,y) in enumerate(zip(expected,b[2:])) if x!=y]
        reports.append(dict(source=source.name,sha256=digest(source),installed=installed,parameters=parameters,
                            original_input_sha256=digest(original_input),native_input_sha256=digest(native_input)))
    guards = []
    for name,replacement in [('unordered','<WHEEL damageToStartBaseEffect="0" damageToHaveFullEffect="0"/>'),
                             ('nonfinite','<WHEEL maxMisalignAngle="NaN"/>'),('overflow','<WHEEL maxSteerWobbleScale="1e300"/>'),
                             ('duplicate','<WHEEL/><WHEEL/>'),('description_in_attribute','<WHEEL a="<!note>"/>'),
                             ('entity','<!DOCTYPE x><WHEEL/>'),('missing','')]:
        source = out/f'guard-{name}.hdm'
        source.write_text(f'<VEHICLEHANDLINGDAMAGE><SUSPENSION/>{replacement}</VEHICLEHANDLINGDAMAGE>')
        output = out/f'guard-{name}.tsv'
        run = subprocess.run([str(native),str(native_input),str(output),'--source',str(source)],cwd=ROOT,capture_output=True,text=True,timeout=30)
        guards.append(dict(name=name,exit=run.returncode,stderr=run.stderr,output_created=output.exists()))
        assert run.returncode and not output.exists()
    result = dict(profile=args.profile,datasets=len(datasets),cases=len(datasets)*128,fields=len(datasets)*128*25,
                  datasets_report=reports,executions=executions,differences=differences,guards=guards,
                  archive_sha256=digest(archive), analysis_sha256=PE_SHA,
                  limits='Native legacy HDM source reader versus independent Python conversions supplied to whole original9e6070->9c5b90. '
                         'Original XML loader/configuration reader/numeric converter/full constructor unexecuted; static mapping only. '
                         'Supplied RNG seed, secondary component absent, wheel component queries inactive. No live RNG/cadence, '
                         'per-vehicle override selection, full damage or driving claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ('profile','datasets','cases','fields')}))
    print('different_fields='+str(len(differences)))
    assert not differences


if __name__ == '__main__': main()
