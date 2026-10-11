"""Whole HDM settings and joined cold car/source reset, explicit CRT adapter.

Original XML text tokenizer, resource acquisition and outer factory excluded.
"""
import argparse,copy,hashlib,json,re,struct,subprocess,xml.etree.ElementTree as ET
from pathlib import Path
from validate_surface_mechanics import encode
from validate_binxml import entries
import validate_standard_properties as v

ROOT=v.ROOT
NATIVE=ROOT/'local/behavior/handling-configuration-probe.exe'
IMAGE=ROOT/'local/compatibility/grid-unpacked.exe'
LAYOUT=ROOT/'local/behavior/handling-configuration-layout-20261011.json'
def run(exe,args):
    e=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
    return dict(exit=e.returncode,stdout=e.stdout,stderr=e.stderr)
def rng(seed,count):
    for _ in range(count):seed=(seed*0x41c64e6d+0x40584c0f)&0xffffffff
    return seed
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--join',action='store_true')
    p.add_argument('--constructor-prefix',default='standard-constructor');p.add_argument('--source-prelude',action='store_true');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    assert v.digest(LAYOUT)=='224c7041edb9158fdcc229c602e34f6a7a8bdf131250dcf09a928c3414a2e560'
    layout=json.loads(LAYOUT.read_text());archive=v.GAME/'raceload.jpk'
    assert v.digest(archive)=='58c62779d1915307784da7c095f1bae3832828abecb771bb4659a37999afcbc8'
    survey=json.loads((ROOT/'local/behavior/handling-damage-source-20261008-02/survey.json').read_text())
    sources=[]
    for name,data,raw in entries(archive.read_bytes()):
        if name not in ['standard.hdm','openwheeler.hdm']:continue
        assert raw and hashlib.sha256(data).hexdigest()==next(r['sha256'] for r in survey['reports'] if r['entry']==name)
        root=ET.fromstring(re.sub(rb'<![^>]*>',b'',data));sources.append((name,root,data,'adapted'))
    assert len(sources)==2
    controls=[('empty',ET.Element('VEHICLEHANDLINGDAMAGE'),'original')]
    nodes=list(dict.fromkeys(n for n,_,_ in layout['fields']))
    for node in nodes:
        root=ET.Element('VEHICLEHANDLINGDAMAGE');ET.SubElement(root,node);controls.append(('empty-'+node,root,'original'))
    for parent,node,_ in layout['strength']:
        root=ET.Element('VEHICLEHANDLINGDAMAGE');ET.SubElement(ET.SubElement(root,parent),node);controls.append(('empty-'+node,root,'original'))
    for node,key,_ in layout['fields']:
        root=ET.Element('VEHICLEHANDLINGDAMAGE');ET.SubElement(root,node,{key:'1.0000000596046448'});controls.append((node+'-'+key,root,'adapted'))
    for parent,node,_ in layout['strength']:
        for key in layout['strength_keys']:
            root=ET.Element('VEHICLEHANDLINGDAMAGE');ET.SubElement(ET.SubElement(root,parent),node,{key:'1.75'});controls.append((node+'-'+key,root,'adapted'))
    for value in ['-0.0','0','1e-50','-1e-50','.125','-1.75','1.0000000596046448','2.5']:
        root=ET.Element('VEHICLEHANDLINGDAMAGE');parents={node:ET.SubElement(root,node) for node in nodes}
        for node,key,_ in layout['fields']:parents[node].set(key,value)
        for parent,node,_ in layout['strength']:ET.SubElement(parents[parent],node,{key:value for key in layout['strength_keys']})
        controls.append(('all-'+value,root,'adapted'))
    for parent,node,_ in layout['strength']:
        for minor,major in [('.4','.6'),('.4000001','.6'),('2','3'),('-2','3')]:
            root=ET.Element('VEHICLEHANDLINGDAMAGE');ET.SubElement(ET.SubElement(root,parent),node,dict(chanceOfMinorDamage=minor,chanceOfMajorDamage=major));controls.append((node+'-sum-'+minor+'-'+major,root,'adapted'))
    fixtures=[]
    for label,root,data,mode in sources:fixtures.append((label,root,data,mode))
    for label,root,mode in controls:fixtures.append((label,root,ET.tostring(root,encoding='ascii'),mode))
    settings=[]
    for i,(label,root,text,mode) in enumerate(fixtures):
        source=out/f'{i:03}.hdm';source.write_bytes(text);binary=out/f'{i:03}-xml.bin';binary.write_bytes(encode(root));answers=[];executions=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{i:03}-{kind}.bin';exe=ROOT/f'target/{a.profile}/grid-handling-configuration.exe' if kind=='rust' else NATIVE
            args=[source,target] if kind=='rust' else [IMAGE,binary,target,mode]
            e=run(exe,args);executions.append(e);(out/'current-executions.json').write_text(json.dumps(executions,indent=2))
            assert e['exit']==0 and not e['stderr'],dict(label=label,kind=kind,execution=e)
            if kind!='rust':assert f'numeric_adapters={int(mode=="adapted")} restored=1 error=0' in e['stdout']
            answers.append(target.read_bytes());assert len(answers[-1])==360
        differences=[dict(offset=hex(o),original=answers[0][o:o+4].hex(),repeat=answers[1][o:o+4].hex(),rust=answers[2][o:o+4].hex()) for o in range(0,360,4) if len({b[o:o+4] for b in answers})!=1]
        (out/'current-differences.json').write_text(json.dumps(dict(label=label,differences=differences),indent=2));assert not differences
        settings.append(dict(label=label,index=i,mode=mode,input_sha256=v.digest(source),xml_sha256=v.digest(binary),output_sha256=v.digest(out/f'{i:03}-original.bin'),executions=executions))
    joined=[];upstream=[]
    if a.join:
        hdms=[(i,label,root) for i,(label,root,_,_) in enumerate(fixtures[:2])]
        # Valid configured controls preserve source ramp/finite reset semantics.
        base=next(root for label,root,_,_ in fixtures if label=='standard.hdm')
        extra=[]
        for node,key,value in [('WHEEL','maxMisalignAngle','0.5'),('WHEEL','maxMisalignAngle','-0.0'),('SUSPENSION','maxWheelLockScale','0'),('SUSPENSION','maxWheelLockScale','1'),('WHEEL','damageToStartBaseEffect','-0.1'),('WHEEL','damageToHaveFullEffect','0.5'),('WHEEL','damageMinEffect','0.5'),('WHEEL','maxSteerWobbleScale','0'),('SUSPENSION','damageToStartEffect','0'),('DRIVESHAFT','damageToStartEffect','-0.1'),('ENGINE','damageToStartBaseEffect','-0.1'),('GEARBOX','damageToStartEffect','-0.1')]:
            root=copy.deepcopy(base);root.find(node).set(key,value);extra.append((node+'-'+key+'='+value,root))
        for seed in ['200679','0','0xffffffff']:
            report_path=ROOT/f'local/behavior/{a.constructor_prefix}-debug-20261011-final-{seed}/validation.json';r=json.loads(report_path.read_text());assert r['rust_equivalence'] and not r['differences'] and r.get('source_prelude',False)==a.source_prelude;upstream.append(dict(path=str(report_path),sha256=v.digest(report_path)))
            for car_index,car in enumerate(r['results'][:44]):
                if car.get('skipped'):continue
                cold=report_path.parent/f'{car_index:03}-original.bin';assert v.digest(cold)==car['output_sha256']
                assert v.digest(v.GAME/car['source'])==car['source_sha256'] and v.digest(Path(car['ned']))==car['ned_sha256']
                selected=list(hdms)
                if Path(car['source']).stem=='lac':
                    for label,root in extra:
                        i=len(fixtures);fixtures.append((label,root,b'','adapted'));(out/f'{i:03}.hdm').write_bytes(ET.tostring(root,encoding='ascii'));(out/f'{i:03}-xml.bin').write_bytes(encode(root));selected.append((i,label,root))
                for hdm_index,label,root in selected:
                    for active in [0,1]:
                        index=len(joined);answers=[];executions=[]
                        for kind in ['original','repeat','rust']:
                            target=out/f'joined-{index:04}-{kind}.bin';exe=ROOT/f'target/{a.profile}/grid-standard-car.exe' if kind=='rust' else NATIVE
                            args=[v.GAME/car['source'],car['ned'],f'{int(seed,0):08x}',target,out/f'{hdm_index:03}.hdm',active] if kind=='rust' else [IMAGE,out/f'{hdm_index:03}-xml.bin',target,'adapted',cold,f'{rng(int(seed,0),12):08x}',active]
                            if kind=='rust' and a.source_prelude:args.append('--source-prelude')
                            e=run(exe,args);executions.append(e);(out/'current-joined.json').write_text(json.dumps(dict(index=index,car=car['source'],hdm=label,seed=seed,active=active,executions=executions),indent=2))
                            assert e['exit']==0 and not e['stderr'],executions
                            assert f'random_state={rng(int(seed,0),18):08x}' in e['stdout']
                            if kind!='rust':assert 'numeric_adapters=1 restored=1 error=0' in e['stdout']
                            answers.append(target.read_bytes());assert len(answers[-1])==0x21000
                        differences=[hex(o) for o in range(0,0x21000,4) if len({b[o:o+4] for b in answers})!=1]
                        (out/'current-joined-differences.json').write_text(json.dumps(differences));assert not differences,(index,differences[:12])
                        for body in answers:
                            assert all(__import__('math').isfinite(x) for x in struct.unpack_from('<9f',body,0xcdf0+0x3a0))
                        joined.append(dict(index=index,car=car['source'],source_sha256=car['source_sha256'],ned=car['ned'],ned_sha256=car['ned_sha256'],cold=str(cold),cold_sha256=v.digest(cold),hdm=label,hdm_index=hdm_index,hdm_sha256=v.digest(out/f'{hdm_index:03}.hdm'),xml_sha256=v.digest(out/f'{hdm_index:03}-xml.bin'),seed=int(seed,0),active=active,output_sha256=v.digest(out/f'joined-{index:04}-original.bin'),executions=executions))
    result=dict(passed=True,profile=a.profile,native_sha256=v.digest(NATIVE),image_sha256=v.digest(IMAGE),rust_settings_sha256=v.digest(ROOT/f'target/{a.profile}/grid-handling-configuration.exe'),rust_car_sha256=v.digest(ROOT/f'target/{a.profile}/grid-standard-car.exe'),layout_sha256=v.digest(LAYOUT),archive_sha256=v.digest(archive),settings=settings,joined=joined,upstream=upstream,settings_cases=len(settings),joined_cases=len(joined),settings_bytes=len(settings)*360,joined_bytes=len(joined)*0x21000,limits='Whole settings and reset on cold constructed owners. Numeric CRT conversion adapted; original text/resource/outer factory/physical owner/driving excluded. Owned optional wheel-view mode explicit, secondary/query objects absent.')
    result['source_prelude']=a.source_prelude
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k.endswith('cases') or k.endswith('bytes') or k=='passed'}))
if __name__=='__main__':main()
