"""SC05 source Lacetti plus complete source HDM reset; explicit numeric adapter."""
import argparse,json,re,subprocess
from pathlib import Path
import xml.etree.ElementTree as ET
from validate_standard_properties import GAME,digest,ROOT
from validate_binxml import entries
from validate_surface_mechanics import encode


def advance(seed,n):
    for _ in range(n):seed=(seed*0x41c64e6d+0x40584c0f)&0xffffffff
    return seed


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.output.resolve().is_relative_to((ROOT/'local').resolve());a.output.mkdir(exist_ok=False)
    archive=GAME/'raceload.jpk';assert digest(archive)=='58c62779d1915307784da7c095f1bae3832828abecb771bb4659a37999afcbc8'
    hdms=[]
    for name,data,raw in entries(archive.read_bytes()):
        if name in ['standard.hdm','openwheeler.hdm']:
            assert raw;path=a.output/name;path.write_bytes(data)
            xml=a.output/(name+'.bin');xml.write_bytes(encode(ET.fromstring(re.sub(rb'<![^>]*>',b'',data))))
            hdms.append((name,path,xml))
    assert len(hdms)==2
    native=ROOT/'local/behavior/handling-configuration-probe.exe';image=ROOT/'local/compatibility/grid-unpacked.exe'
    pinned={str(p):digest(p) for p in [archive,native,image,Path(__file__),ROOT/'tools/validate_surface_mechanics.py',ROOT/'tools/validate_binxml.py']}
    rows=[]
    for profile in ['debug','release']:
        rust=ROOT/f'target/{profile}/grid-standard-car.exe';pinned[str(rust)]=digest(rust)
        for seed in [200679,0,0xffffffff]:
            text=str(seed) if seed!=0xffffffff else '0xffffffff'
            report=ROOT/f'local/behavior/outer-wheels-standard-debug-20261013-final-{text}/validation.json';pinned[str(report)]=digest(report)
            r=json.loads(report.read_text());assert r['outer_wheels'] and r['rust_equivalence'] and not r['differences']
            index,source=next((i,s) for i,s in enumerate(r['results']) if s['source'].replace('\\','/')=='cars/lac/lac.csv')
            cold=report.parent/f'{index:03}-original.bin';assert digest(cold)==source['output_sha256'];pinned[str(cold)]=digest(cold)
            for label,hdm,xml in hdms:
                for active in ['0','1']:
                    outputs={};executions={}
                    for kind in ['original','repeat','rust']:
                        out=a.output/f'{len(rows):03}-{kind}.bin'
                        command=[rust,GAME/source['source'],source['ned'],f'{seed:08x}',out,hdm,active,'--outer-wheels'] if kind=='rust' else [native,image,xml,out,'adapted',cold,f'{advance(seed,18):08x}',active]
                        e=subprocess.run([str(v) for v in command],capture_output=True,text=True,timeout=30)
                        assert e.returncode==0 and not e.stderr,(kind,e.returncode,e.stdout,e.stderr)
                        assert f'random_state={advance(seed,24):08x}' in e.stdout
                        if kind!='rust':assert 'numeric_adapters=1 restored=1 error=0' in e.stdout
                        outputs[kind]=dict(path=str(out),sha256=digest(out));executions[kind]=dict(exit=e.returncode,stdout=e.stdout,stderr=e.stderr)
                    answers=[Path(q['path']).read_bytes() for q in outputs.values()];assert all(len(v)==0x21000 and v==answers[0] for v in answers)
                    rows.append(dict(profile=profile,seed=seed,hdm=label,hdm_sha256=digest(hdm),xml_sha256=digest(xml),active=active,cold=str(cold),outputs=outputs,executions=executions))
    result=dict(passed=True,cases=len(rows),complete_bytes=24*0x21000,inputs=pinned,rows=rows,scope='SC05 selected source Lacetti cold state plus whole9C5580/9E6070 with one numeric CRT adapter; complete state and24-draw RNG. Five NED regions and original text/resource factory excluded.')
    assert len(rows)==24
    with (a.output/'validation.json').open('x') as file:json.dump(result,file,indent=2)
    print(json.dumps({k:v for k,v in result.items() if k not in ['inputs','rows']}))


if __name__=='__main__':main()
