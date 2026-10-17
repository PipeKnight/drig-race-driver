"""Compare unchanged original 423210 with supplied playback-service replies."""
import argparse
import json
import random
import struct
import subprocess
from pathlib import Path
from validate_engine_audio_control import source_curves
from validate_engine_audio_gain import sha

ROOT=Path(__file__).resolve().parents[1]
HEADER, RECORD, RESULT=16392,856,896

def fixtures(count=1024):
    curves,_=source_curves(); values=struct.unpack("<4096f",curves)
    rng=random.Random(423210)
    rows=[]
    for case in range(count):
        ctrl=bytearray(rng.randbytes(832)); category=case%3
        def put(at,v):struct.pack_into('<f',ctrl,at,v)
        def word(at,v):struct.pack_into('<I',ctrl,at,v)
        put(0xc8,(0.,.05,.2,.5,.8,1.)[case%6])
        put(0x134+category*4,(-1.,-.001,-0.,0.,.125,1.)[(case//3)%6])
        ctrl[0x14c+category]=(case//18)%2
        ctrl[0x14f+category]=(case//36)%2
        outgoing=0 if (case//72)%2 else 2
        word(0x1d4+category*4,outgoing);word(0x1e0+category*4,2-outgoing)
        word(0x11c,1);word(0x124,0);word(0x22c,case%2)
        for slot in range(12):
            word(0x290+slot*4,(8000,32000,44100,48000,192000)[(case+slot)%5])
            word(0x2c0+slot*4,(319292,477036,1,10000000)[(case+slot)%4])
        for slot in range(6):word(0x154+slot*4,slot+20)
        replies=(0xffffffff,0,123456,999999,60000000)
        accepted=replies[(case//5)%5]
        if accepted==0xffffffff:
            slot=outgoing*3+category;target=struct.unpack_from('<f',ctrl,0xc8)[0];idx=min(1023,int(target*1024))
            accepted=int(struct.unpack_from('<I',ctrl,0x2c0+slot*4)[0]*values[(0 if category==0 else 2048)+1024+idx]/struct.unpack_from('<I',ctrl,0x290+slot*4)[0]*1000000)
        incoming=2-outgoing;slot=incoming*3+category
        rate=struct.unpack_from('<I',ctrl,0x290+slot*4)[0];length=struct.unpack_from('<I',ctrl,0x2c0+slot*4)[0]
        position=struct.unpack('<f',struct.pack('<f',accepted*struct.unpack('<f',struct.pack('<f',1e-6))[0]*rate))[0]
        time=struct.unpack('<f',struct.pack('<f',int(position)/length))[0]
        idx=min(1023,int(min(1,time)*1024))
        if struct.unpack_from('<f',ctrl,0xc8)[0]/values[(0 if category==0 else 2048)+idx]>1.5:put(0xc8,.5)
        rows.append(struct.pack('<I',case)+ctrl+struct.pack('<3IfI',category,replies[(case//5)%5],replies[(case//25)%5],(0.,.0625,.125,.3,1.)[case%5],99))
    return b'ER01'+struct.pack('<I',count)+curves+b''.join(rows)

def execute(exe,args,log,success=True):
    r=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=120)
    log.write_text(r.stdout+r.stderr)
    assert (r.returncode==0)==success and 'exception=' not in r.stderr,(exe,r.stdout,r.stderr)
    if exe.name=='engine-audio-restart-probe.exe':assert 'entire_restored_image_unchanged=1' in r.stdout

def audit(request,result):
    count=struct.unpack_from('<I',request,4)[0]
    assert result[:8]==request[:8] and len(result)==8+count*RESULT
    invoked=0;different_replies=0
    for case in range(count):
        before=request[HEADER+case*RECORD:HEADER+(case+1)*RECORD]
        after=result[8+case*RESULT:8+(case+1)*RESULT]
        category=struct.unpack_from('<I',before,836)[0]
        incoming=struct.unpack_from('<I',before,4+0x1e0+category*4)[0]
        calls=struct.unpack_from('<I',after,836)[0]
        should=before[4+0x14c+category]!=0 and struct.unpack_from('<f',before,4+0x134+category*4)[0]<0
        assert calls==2*should and before[:4]==after[:4]
        mutable=set()
        if calls:
            invoked+=1;slot=incoming*3+category
            for at in [0x134+category*4,0x230+slot*4,0x23c+slot*4,0x260+slot*4,0x26c+slot*4,0x1a4+slot*4,0x1b0+slot*4]:mutable.update(range(at,at+4))
            mutable|={0x14c+category,0x14f+category}
            assert after[4+0x14c+category]==0 and after[4+0x14f+category]==1-before[4+0x14f+category]
            for emitter in range(2):
                command=struct.unpack_from('<7I',after,840+emitter*28)
                assert command[0]==slot+emitter*3 and command[1]==99
                assert command[2]==20+category+emitter*3
                assert command[3]==case%2 and command[4]==(1 if emitter==0 else 0)^0x100
                different_replies+=command[5]!=command[6]
        else:assert not any(after[840:])
        for byte in range(832):
            if byte not in mutable:assert before[4+byte]==after[4+byte],(case,hex(byte))
    return dict(cases=count,invoked=invoked,skipped=count-invoked,different_accepted_seeks=different_replies,complete_controller_bytes=count*832,opaque_bytes_preserved=True)

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();out=a.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve()) and not out.exists();out.mkdir()
    native=ROOT/'local/audio/engine-audio-restart-probe.exe'
    profiles=[('native',native,[ROOT/'local/compatibility/grid-unpacked.exe']),('native-repeat',native,[ROOT/'local/compatibility/grid-unpacked.exe']),('debug',ROOT/'target/debug/grid-engine-audio-restart.exe',[]),('release',ROOT/'target/release/grid-engine-audio-restart.exe',[])]
    request=fixtures();q=out/'request.bin';q.write_bytes(request);outputs=[]
    for label,exe,prefix in profiles:
        dest=out/f'{label}.bin';execute(exe,[*prefix,q,dest],out/f'{label}.txt');outputs.append(dest.read_bytes())
    for index,data in enumerate(outputs[1:]):
        if data!=outputs[0]:
            byte=next(i for i,(x,y) in enumerate(zip(data,outputs[0])) if x!=y)
            raise AssertionError((profiles[index+1][0],byte,(byte-8)//RESULT,(byte-8)%RESULT,outputs[0][byte:byte+8].hex(),data[byte:byte+8].hex()))
    summary=audit(request,outputs[0]);guards=[]
    for name,at,fmt,value in [('category',HEADER+836,'I',3),('pair',HEADER+4+0x1e0,'I',1),('reply',HEADER+840,'I',60000001),('cooldown',HEADER+848,'f',float('nan')),('target',HEADER+4+0xc8,'f',float('nan')),('curve_zero',8,'f',0.),('last_rate',HEADER+4+0x290+9*4,'I',0)]:
        raw=bytearray(fixtures(1));struct.pack_into('<'+fmt,raw,at,value);q=out/f'guard-{name}.bin';q.write_bytes(raw)
        for label,exe,prefix in profiles:
            if label=='native-repeat':continue
            execute(exe,[*prefix,q,out/f'guard-{name}-{label}.bin'],out/f'guard-{name}-{label}.txt',False);guards.append(dict(name=name,profile=label,rejected=True))
    raw=bytearray(fixtures(59)[:HEADER])
    struct.pack_into('<I',raw,4,1)
    row=bytearray(fixtures(59)[HEADER+58*RECORD:HEADER+59*RECORD])
    struct.pack_into('<f',row,4+0xc8,1.);row[4+0x14d]=1;struct.pack_into('<f',row,4+0x138,-1.)
    raw+=row;q=out/'guard-logger.bin';q.write_bytes(raw)
    for label,exe,prefix in profiles:
        if label=='native-repeat':continue
        execute(exe,[*prefix,q,out/f'guard-logger-{label}.bin'],out/f'guard-logger-{label}.txt',False);guards.append(dict(name='logger-domain',profile=label,rejected=True))
    files=[native,profiles[2][1],profiles[3][1],Path(__file__),ROOT/'tools/probe_engine_audio_restart.c',ROOT/'tools/build_engine_audio_restart_probe.cmd',ROOT/'crates/grid-tools/src/engine_audio_restart.rs',ROOT/'crates/grid-probes/src/bin/grid-engine-audio-restart.rs']+list(out.glob('*.bin'))+list(out.glob('*.txt'))
    report=dict(passed=True,summary=summary,guards=guards,scope='Whole unchanged 423210 and real table helpers with supplied virtual playback/group services. Commands and accepted seeks compared; not the original backend implementation, real device or audible fidelity.',identities={str(f):sha(f) for f in files})
    (out/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='identities'}))
if __name__=='__main__':main()
