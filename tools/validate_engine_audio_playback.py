"""Whole4371C0, including real restarts, with an explicit owned PCM-service boundary."""
import argparse
import json
import struct
import subprocess
from pathlib import Path
from validate_engine_audio_schedule import fixtures as waiting_fixtures
from validate_engine_audio_gain import sha

ROOT=Path(__file__).resolve().parents[1]
HEADER,RECORD,RESULT=16392,2448,2548

def fixtures(count=1024):
    waiting=waiting_fixtures(count);out=bytearray(waiting[:HEADER]);out[:4]=b'EP01'
    for case in range(count):
        row=bytearray(waiting[HEADER+case*2444:HEADER+(case+1)*2444])
        delta=struct.unpack_from('<d',row,2436)[0]
        for category in range(3):
            struct.pack_into('<f',row,4+0x134+category*4,(-1.,-.001,-0.,delta,1.)[(case+category)%5])
            row[4+0x14c+category]=(case+category)%2
            for bank in range(4):
                slot=bank*3+category
                struct.pack_into('<I',row,4+0x290+slot*4,32000)
                struct.pack_into('<I',row,4+0x2c0+slot*4,319292 if category==0 else 477036)
        for at,value in [(0x11c,1),(0x124,0),(0x22c,1)]+[(0x154+i*4,i) for i in range(6)]:struct.pack_into('<I',row,4+at,value)
        out+=row+struct.pack('<f',(.0625,.125,.3)[case%3])
    return out

def run(exe,args,log,success=True):
    r=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
    log.write_text(r.stdout+r.stderr)
    assert (r.returncode==0)==success and 'exception=' not in r.stderr,(exe,r.stdout,r.stderr)
    if exe.name=='engine-audio-playback-probe.exe':assert 'entire_restored_image_unchanged=1' in r.stdout

def audit(request,result):
    count=struct.unpack_from('<I',request,4)[0]
    assert result[:8]==request[:8] and len(result)==8+count*RESULT
    pairs=toggles=0
    mutable=set(range(8,12))|set(range(0xc8,0xcc))|set(range(0x134,0x152))|set(range(0x1a4,0x1ec))|set(range(0x230,0x290))
    for case in range(count):
        before=request[HEADER+case*RECORD:HEADER+(case+1)*RECORD];after=result[8+case*RESULT:8+(case+1)*RESULT]
        assert before[:4]==after[:4] and after[2372:2376]==b'\0'*4
        calls=struct.unpack_from('<I',after,2376)[0];assert calls in (0,2,4,6);pairs+=calls//2
        changed=sum(before[4+0x14f+i]!=after[4+0x14f+i] for i in range(3));assert changed==calls//2;toggles+=changed
        for byte in range(832):
            if byte not in mutable:assert before[4+byte]==after[4+byte],(case,hex(byte))
        for call in range(calls):
            command=struct.unpack_from('<7I',after,2380+call*28)
            assert command[1]==1 and command[3]==1 and command[5]==command[6]
            assert command[2]==command[0]%6
        assert not any(after[2380+calls*28:])
    return dict(cases=count,complete_controller_voice_bytes=count*(832+1536),restarted_pairs=pairs,pair_toggles=toggles,opaque_controller_bytes_preserved=True)

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve()) and not out.exists();out.mkdir()
    native=ROOT/'local/audio/engine-audio-playback-probe.exe'
    profiles=[('native',native,[ROOT/'local/compatibility/grid-unpacked.exe']),('native-repeat',native,[ROOT/'local/compatibility/grid-unpacked.exe']),('debug',ROOT/'target/debug/grid-engine-audio-playback.exe',[]),('release',ROOT/'target/release/grid-engine-audio-playback.exe',[])]
    request=fixtures();q=out/'request.bin';q.write_bytes(request);outputs=[]
    for label,exe,prefix in profiles:
        dest=out/f'{label}.bin';run(exe,[*prefix,q,dest],out/f'{label}.txt');outputs.append(dest.read_bytes())
    for index,data in enumerate(outputs[1:]):
        if data!=outputs[0]:
            byte=next(i for i,(x,y) in enumerate(zip(data,outputs[0])) if x!=y)
            raise AssertionError((profiles[index+1][0],byte,(byte-8)//RESULT,(byte-8)%RESULT,outputs[0][byte:byte+8].hex(),data[byte:byte+8].hex()))
    summary=audit(request,outputs[0]);guards=[]
    for name,at,fmt,value in [('bad_pair_flag',HEADER+4+0x151,'B',2),('cooldown_nan',HEADER+4+0x13c,'f',float('nan')),('delta_nan',HEADER+2436,'d',float('nan')),('service_cooldown',HEADER+2444,'f',float('nan')),('last_length_zero',HEADER+4+0x2ec,'I',0),('last_treble_nan',HEADER+2428,'f',float('nan'))]:
        raw=bytearray(fixtures(1));struct.pack_into('<'+fmt,raw,at,value);q=out/f'guard-{name}.bin';q.write_bytes(raw)
        for label,exe,prefix in profiles:
            if label=='native-repeat':continue
            run(exe,[*prefix,q,out/f'guard-{name}-{label}.bin'],out/f'guard-{name}-{label}.txt',False);guards.append(dict(name=name,profile=label,rejected=True))
    source_outputs=[]
    for profile in ('debug','release'):
        directory=out/f'source-{profile}'
        run(ROOT/f'target/{profile}/grid-engine-audio-playback-drive.exe',[Path('C:/Program Files (x86)/Steam/steamapps/common/Grid'),directory],out/f'source-{profile}.txt')
        source_outputs.append(directory)
    for name in ('source.request.bin','source.expected.bin','source.playback.request.bin','source.playback.expected.bin'):
        assert (source_outputs[0]/name).read_bytes()==(source_outputs[1]/name).read_bytes(),name
    source_request=source_outputs[0]/'source.playback.request.bin'
    expected=(source_outputs[0]/'source.playback.expected.bin').read_bytes()
    for repeat in range(2):
        dest=out/f'source-native-{repeat}.bin'
        run(native,[ROOT/'local/compatibility/grid-unpacked.exe',source_request,dest],out/f'source-native-{repeat}.txt')
        assert dest.read_bytes()==expected
    source=audit(source_request.read_bytes(),expected)
    # Controller at the next physics frame is exactly the last full scheduler state.
    controls=(source_outputs[0]/'source.request.bin').read_bytes()
    records=source_request.read_bytes()
    for case in range(999):
        previous=expected[8+case*RESULT+4:8+case*RESULT+836]
        current=controls[HEADER+(case+1)*856+4:HEADER+(case+1)*856+836]
        assert previous==current
        a=8+case*RESULT+836;b=HEADER+(case+1)*RECORD+836
        assert expected[a:a+1536]==records[b:b+1536]
    source.update(carried_controller=True,carried_voices=True,paired_physics_no_feedback=True)
    # Also retain unchanged original RPM/phase/pitch proof for the carried source path.
    for repeat in range(2):
        dest=out/f'source-control-native-{repeat}.bin'
        r=subprocess.run([str(ROOT/'local/audio/engine-audio-control-probe.exe'),str(ROOT/'local/compatibility/grid-unpacked.exe'),str(source_outputs[0]/'source.request.bin'),str(dest)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        (out/f'source-control-native-{repeat}.txt').write_text(r.stdout+r.stderr)
        assert r.returncode==0 and 'entire_image_unchanged=1' in r.stdout and 'exception=' not in r.stderr
        assert dest.read_bytes()==(source_outputs[0]/'source.expected.bin').read_bytes()
    files=[native,profiles[2][1],profiles[3][1],ROOT/'target/debug/grid-engine-audio-playback-drive.exe',ROOT/'target/release/grid-engine-audio-playback-drive.exe',ROOT/'crates/grid-tools/src/bin/grid-engine-audio-playback-drive.rs',Path(__file__),ROOT/'tools/probe_engine_audio_playback.c',ROOT/'tools/build_engine_audio_playback_probe.cmd',ROOT/'crates/grid-tools/src/engine_audio_schedule.rs',ROOT/'crates/grid-tools/src/engine_audio_restart.rs',ROOT/'crates/grid-tools/src/bin/grid-engine-audio-playback.rs']+list(out.rglob('*.bin'))+list(out.rglob('*.txt'))
    report=dict(passed=True,summary=summary,source=source,guards=guards,scope='Whole unchanged4371C0/all real math, gain and restart helpers. Owned all-voices/camera3 graph; explicit virtual group/playback services retain requested seeks. Matched PCM rates/lengths within each category. Not original backend/451770/PCM rendering or audible equivalence.',identities={str(f):sha(f) for f in files})
    (out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='identities'}))
if __name__=='__main__':main()
