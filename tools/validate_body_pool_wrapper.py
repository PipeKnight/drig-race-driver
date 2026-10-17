"""Whole original registered wrapper on sequential solved source boundaries."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess

from capture_reference_spawn import ANALYSIS, private
from validate_standard_properties import GAME, ROOT

CAR=0x21000
PHYSICAL=0x424
RECORD=16+CAR+PHYSICAL+20*128
RESULT=12+CAR+PHYSICAL+20*128
SCENARIOS={'quarter':(0,.25),'180-left':(180,1.),'300-right':(300,-1.)}


def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def execute(command,success=True):
    process=subprocess.run(list(map(str,command)),capture_output=True,text=True,timeout=120)
    assert (process.returncode==0)==success,(command,process.returncode,process.stdout,process.stderr)
    return dict(exit=process.returncode,stdout=process.stdout,stderr=process.stderr)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);a=parser.parse_args()
    out=private(a.output);out.mkdir(parents=True,exist_ok=False)
    image=ROOT/'local/compatibility/grid-unpacked.exe';native=ROOT/'local/behavior/body-pool-wrapper-probe.exe'
    assert sha(image)==ANALYSIS
    directions=ROOT/'local/body-contact-directions.bin';parameters=ROOT/'local/body-impact-parameters.bin'
    files=[image,native,directions,parameters,Path(__file__),ROOT/'tools/probe_body_pool_wrapper.c',ROOT/'tools/probe_collision_damage.c',ROOT/'tools/isolated_reference.h',ROOT/'crates/grid-probes/src/bin/grid-body-pool-trace.rs',ROOT/'crates/grid-tools/src/source_vehicle.rs',ROOT/'crates/grid-world/src/body_impact.rs',ROOT/'crates/grid-world/src/vehicle_world.rs',ROOT/'Cargo.lock']
    for profile in ['debug','release']:files.append(ROOT/f'target/{profile}/grid-body-pool-trace.exe')
    pins={str(p):sha(p) for p in files}
    report=dict(scope='Whole original A727A0 registered dispatch/consumption on sequential '
                'solved source frames; original A726B0/A01910/9EF360 reached unchanged. '
                'Single car/static peers/empty notifications; owned one-step geometry carrier. '
                'Original AD8D70, geometry detection, factories and asynchronous scheduling are not executed by this probe.',
                pins=pins,scenarios={},batches=[],partition_controls=[],guards=[])
    best=None;first=None;total=contacts=external=changed=compared=0
    for name,(straight,steering) in SCENARIOS.items():
        profile_dirs={}
        for profile in ['debug','release']:
            directory=out/f'{name}-{profile}'
            execution=execute([ROOT/f'target/{profile}/grid-body-pool-trace.exe',GAME,directions,parameters,straight,steering,directory])
            profile_dirs[profile]=directory
            report['scenarios'][f'{name}-{profile}']=execution
        for index,inp in enumerate(sorted(profile_dirs['debug'].glob('*.input'))):
            request=inp.read_bytes();count=struct.unpack_from('<I',request,4)[0]
            assert request[:4]==b'BW01' and len(request)==8+count*RECORD
            wanted=inp.with_suffix('.expected').read_bytes()
            assert wanted[:8]==request[:8] and len(wanted)==8+count*RESULT
            for suffix in ['input','expected']:
                assert inp.with_suffix('.'+suffix).read_bytes()==(profile_dirs['release']/f'{index:03}.{suffix}').read_bytes()
            results={}
            for label in ['original','repeat']:
                target=out/f'{name}-{index:03}-{label}.bin';execution=execute([native,image,inp,target])
                data=target.read_bytes();difference=next((i for i,(x,y) in enumerate(zip(data,wanted)) if x!=y),None)
                assert data==wanted,(name,index,label,difference,None if difference is None else hex((difference-8)%RESULT))
                assert 'complete_image_unchanged=1 owned_contexts_checked=1 original_wrapper=1 code_replacements=0 error=0' in execution['stdout']
                results[label]=dict(path=str(target),sha256=sha(target),execution=execution)
            for i in range(count):
                r=request[8+i*RECORD:8+(i+1)*RECORD];w=wanted[8+i*RESULT:8+(i+1)*RESULT]
                seed,delta,used,prefix=struct.unpack_from('<4I',r)
                assert struct.unpack_from('<2I',w,4)==(0,used-prefix)
                assert w[12+CAR:]==r[16+CAR:]
                total+=1;contacts+=used;external+=prefix;changed+=struct.unpack_from('<I',w)[0]!=seed
                if first is None:first=(r,w)
                if best is None or used>struct.unpack_from('<I',best[0],8)[0]:best=(r,w)
            compared+=len(wanted)
            report['batches'].append(dict(name=name,index=index,cases=count,request=str(inp),request_sha256=sha(inp),expected_sha256=sha(inp.with_suffix('.expected')),bytes=len(wanted),runs=results))
        print(json.dumps(dict(scenario=name,complete_source_frames=total)),flush=True)
    assert total==1800 and changed>0 and contacts>0
    # Authored carrier splits reuse a complete solved source record. They test
    # traversal/consumption, not live wheel production or a persistent factory.
    r,w=best;used=struct.unpack_from('<I',r,8)[0]
    for prefix in sorted({0,1,min(4,used)}):
        modified=bytearray(r);struct.pack_into('<I',modified,12,prefix)
        expected=bytearray(w);struct.pack_into('<I',expected,8,used-prefix)
        request=b'BW01'+struct.pack('<I',1)+modified;wanted=b'BW01'+struct.pack('<I',1)+expected
        inp=out/f'partition-{prefix}.input';inp.write_bytes(request)
        for label in ['original','repeat']:
            target=out/f'partition-{prefix}-{label}.bin';execution=execute([native,image,inp,target]);assert target.read_bytes()==wanted
            report['partition_controls'].append(dict(prefix=prefix,used=used,label=label,request_sha256=sha(inp),result_sha256=sha(target),execution=execution))
    valid=b'BW01'+struct.pack('<I',1)+first[0]
    bad=[('short',valid[:-1]),('trailing',valid+b'x')]
    for name,offset,value in [('tag',0,0),('count',4,65),('used',8+8,21),('prefix',8+12,5),('delta',8+4,0),('owner-link',8+16+0xdb0,1),('dynamic',8+16+0x308,1),('mass',8+16+CAR+0x3f0,0)]:
        data=bytearray(valid);struct.pack_into('<I',data,offset,value);bad.append((name,data))
    active=b'BW01'+struct.pack('<I',1)+best[0]
    pool=8+16+CAR+PHYSICAL
    for name,offset,value in [('shape',pool+0x28,0),('point-nan',pool+0x50,0x7fc00000),('point-w',pool+0x5c,0),('impulse',pool+0x3c,0xbf800000)]:
        data=bytearray(active);struct.pack_into('<I',data,offset,value);bad.append((name,data))
    for name,data in bad:
        inp=out/f'guard-{name}.input';inp.write_bytes(data);target=out/f'guard-{name}.bin'
        execution=execute([native,image,inp,target],False);assert not target.exists()
        report['guards'].append(dict(name=name,execution=execution,preserved=True))
    inp=out/'guard-existing.input';inp.write_bytes(valid);target=out/'guard-existing.bin';target.write_bytes(b'KEEP')
    execution=execute([native,image,inp,target],False);assert target.read_bytes()==b'KEEP'
    report['guards'].append(dict(name='existing',execution=execution,preserved=True))
    # The generator rejects malformed source-trace controls without creating
    # its output directory; proprietary data is not copied to these controls.
    for name,straight,steering in [('extent',601,0),('nonfinite',0,'NaN'),('steering',0,2)]:
        target=out/f'guard-capture-{name}'
        execution=execute([ROOT/'target/release/grid-body-pool-trace.exe',GAME,directions,parameters,straight,steering,target],False)
        assert not target.exists();report['guards'].append(dict(name='capture-'+name,execution=execution,preserved=True))
    report.update(passed=True,cases=total,occupied_contacts=contacts,source_external_contacts=external,
                  event_rng_changed_frames=changed,complete_output_bytes_per_profile=compared,
                  partition_comparisons=len(report['partition_controls']),guard_count=len(report['guards']))
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps({k:report[k] for k in ('passed','cases','occupied_contacts','source_external_contacts','event_rng_changed_frames','complete_output_bytes_per_profile','partition_comparisons','guard_count')},indent=2))


if __name__=='__main__':main()
