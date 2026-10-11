"""Audit complete configuration/reset and current cold constructor regression."""
import argparse,ast,json,struct
from pathlib import Path
import validate_handling_configuration as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',nargs=2,type=Path)
    p.add_argument('--cold-prefix',default='standard-constructor');p.add_argument('--source-prelude',action='store_true');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    reports=[];guards=[];cold=[]
    for path in a.reports:
        r=json.loads(path.read_text());profile=r['profile'];root=path.parent
        assert r['passed'] and r['settings_cases']==r['settings_bytes']//360
        assert r.get('source_prelude',False)==a.source_prelude
        assert r['joined_cases']==r['joined_bytes']//0x21000
        assert r['native_sha256']==v.v.digest(v.NATIVE) and r['image_sha256']==v.v.digest(v.IMAGE)
        assert r['layout_sha256']==v.v.digest(v.LAYOUT) and r['archive_sha256']==v.v.digest(v.v.GAME/'raceload.jpk')
        rust=v.ROOT/f'target/{profile}/grid-handling-configuration.exe';car_exe=v.ROOT/f'target/{profile}/grid-standard-car.exe'
        assert r['rust_settings_sha256']==v.v.digest(rust) and r['rust_car_sha256']==v.v.digest(car_exe)
        assert len(r['settings'])==r['settings_cases'] and len(r['joined'])==r['joined_cases']
        for u in r['upstream']:assert v.v.digest(Path(u['path']))==u['sha256']
        for case in r['settings']:
            i=case['index'];assert v.v.digest(root/f'{i:03}.hdm')==case['input_sha256'] and v.v.digest(root/f'{i:03}-xml.bin')==case['xml_sha256']
            for kind in ['original','repeat','rust']:
                file=root/f'{i:03}-{kind}.bin';assert file.stat().st_size==360 and v.v.digest(file)==case['output_sha256']
        for case in r['joined']:
            assert v.v.digest(v.v.GAME/case['car'])==case['source_sha256'] and v.v.digest(Path(case['ned']))==case['ned_sha256']
            assert v.v.digest(Path(case['cold']))==case['cold_sha256']
            i=case['hdm_index'];assert v.v.digest(root/f'{i:03}.hdm')==case['hdm_sha256'] and v.v.digest(root/f'{i:03}-xml.bin')==case['xml_sha256']
            for kind in ['original','repeat','rust']:
                file=root/f'joined-{case["index"]:04}-{kind}.bin';assert file.stat().st_size==0x21000 and v.v.digest(file)==case['output_sha256']
            assert all(e['exit']==0 and not e['stderr'] for e in case['executions'])
        reports.append(dict(path=str(path),sha256=v.v.digest(path),profile=profile,settings=r['settings_cases'],joined=r['joined_cases']))
        # Every existing F203 source/control/seed is replayed through CURRENT Rust.
        for seed in ['200679','0','1','0x80000000','0xffffffff']:
            upstream=v.ROOT/f'local/behavior/{a.cold_prefix}-{profile}-20261011-final-{seed}/validation.json';old=json.loads(upstream.read_text())
            assert old['native_reference_passed'] and old['rust_equivalence'] and not old['differences']
            assert old['exe_sha256']==v.v.digest(v.ROOT/'local/behavior/standard-constructor-probe.exe')
            for i,case in enumerate(old['results']):
                if case.get('skipped'):continue
                source=v.v.GAME/case['source'];assert v.v.digest(source)==case['source_sha256'] and v.v.digest(Path(case['ned']))==case['ned_sha256']
                original=upstream.parent/f'{i:03}-original.bin';assert v.v.digest(original)==case['output_sha256']
                target=out/f'cold-{profile}-{seed}-{i:03}.bin';e=v.run(car_exe,[source,case['ned'],f'{int(seed,0):08x}',target])
                assert e['exit']==0 and not e['stderr'] and v.v.digest(target)==case['output_sha256']
                cold.append(dict(profile=profile,seed=int(seed,0),case=i,upstream_sha256=v.v.digest(upstream),output_sha256=v.v.digest(target)))
        def reject(label,exe,args,target,original=False):
            e=v.run(exe,args);assert e['exit']!=0 and not target.exists(),(label,e)
            if original:assert not e['stderr'] and 'restored=1 error=' in e['stdout']
            else:assert 'grid-' in e['stderr']
            guards.append(dict(profile=profile,label=label,execution=e,no_output=True))
        malformed=[('empty',b''),('truncated',b'<VEHICLEHANDLINGDAMAGE>'),('wrong-root',b'<OTHER/>'),('duplicate-node',b'<VEHICLEHANDLINGDAMAGE><GENERAL/><GENERAL/></VEHICLEHANDLINGDAMAGE>'),('duplicate-strength',b'<VEHICLEHANDLINGDAMAGE><WHEEL><WHEELSTRENGTH/><WHEELSTRENGTH/></WHEEL></VEHICLEHANDLINGDAMAGE>'),('nonfinite',b'<VEHICLEHANDLINGDAMAGE><GENERAL terminalDamageSpeed="NaN"/></VEHICLEHANDLINGDAMAGE>'),('overflow',b'<VEHICLEHANDLINGDAMAGE><BRAKE maxForceToCauseDamage="1e50"/></VEHICLEHANDLINGDAMAGE>'),('entity',b'<VEHICLEHANDLINGDAMAGE><WHEEL x="&foo;"/></VEHICLEHANDLINGDAMAGE>'),('large',b' '*65537)]
        for label,data in malformed:
            source=out/f'{profile}-{label}.hdm';source.write_bytes(data);target=out/f'{profile}-{label}.bin'
            reject(label,rust,[source,target],target)
        first=r['joined'][0];i=first['hdm_index'];hdm=root/f'{i:03}.hdm';xml=root/f'{i:03}-xml.bin';source=v.v.GAME/first['car'];ned=first['ned'];seed=f'{first["seed"]:08x}'
        for label,text,active in [('reset-ramp',b'<VEHICLEHANDLINGDAMAGE><WHEEL damageToHaveFullEffect="0"/></VEHICLEHANDLINGDAMAGE>','1'),('reset-cap',b'<VEHICLEHANDLINGDAMAGE><SUSPENSION maxWheelLockScale="2"/></VEHICLEHANDLINGDAMAGE>','1'),('wheel-mode',hdm.read_bytes(),'2')]:
            source_hdm=out/f'{profile}-{label}.hdm';source_hdm.write_bytes(text);target=out/f'{profile}-{label}-car.bin'
            args=[source,ned,seed,target,source_hdm,active]
            if a.source_prelude:args.append('--source-prelude')
            reject(label,car_exe,args,target)
        data=xml.read_bytes()
        for label,wire in [('short',data[:30]),('extra',data+b'\0'),('tag',b'FAIL'+data[4:])]:
            bad=out/f'{profile}-native-{label}.bin';bad.write_bytes(wire);target=out/f'{profile}-native-{label}-output.bin'
            reject('native-'+label,v.NATIVE,[v.IMAGE,bad,target,'adapted'],target,True)
        bad=out/f'{profile}-bad-owner.bin';body=bytearray(Path(first['cold']).read_bytes());struct.pack_into('<I',body,0xcdf0+0x210,1);bad.write_bytes(body);target=out/f'{profile}-native-owner-output.bin'
        reject('native-owner',v.NATIVE,[v.IMAGE,xml,target,'adapted',bad,'00000001','1'],target,True)
    for name in ['validate_handling_configuration.py','audit_handling_configuration.py']:ast.parse((v.ROOT/'tools'/name).read_text())
    assert len(cold)==690
    result=dict(passed=True,reports=reports,cold_cases=len(cold),cold=cold,guards=guards,guard_count=len(guards),limits='Historical unchanged native constructor outputs rechecked against current Rust. Distinct malformed native framing/source/reset contracts; no malformed original XML/CRT equivalence.')
    result['source_prelude']=a.source_prelude
    (out/'audit.json').write_text(json.dumps(result,indent=2));print(json.dumps(dict(passed=True,reports=reports,cold_cases=len(cold),guards=len(guards))))
if __name__=='__main__':main()
