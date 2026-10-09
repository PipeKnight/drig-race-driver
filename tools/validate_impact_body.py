"""Replay whole original body-effect helper with controlled original-CRT seed."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from validate_impact_generation import blob
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='2855f8e00d88e702a7e25410f0b3feefead374ea681e0ab8cc068dad39ab8acb'


def from_live(case):
    row=case['body_effect'];result=row['random_result']
    if result is not None and not 0<=result<=32767:raise ValueError('Original rand result outside range')
    # Original seed was not captured. Choose a controlled predecessor which
    # yields the exact observed full15-bit result, rather than stubbing rand.
    seed=((result<<16)-2531011)*pow(214013,-1,2**32)%(2**32) if result is not None else 1
    return dict(linked=blob(row,'linked_before_hex',0x424),frame=blob(row,'frame_hex',64),a=blob(row,'prepared_a_before_hex',16),b=blob(row,'prepared_b_hex',16),
                center=blob(row,'center_hex',16),maximum=blob(row,'body_290_before_hex',40),extra=blob(row,'body_2e0_before_hex',4),mass=blob(row,'body_15a8_hex',4),
                globals=blob(row,'globals_hex',20),seed=seed,random_result=result)


def pack(rows):
    data=[struct.pack('<II',0x31524249,len(rows))]
    for r in rows:
        data.extend(r[k] for k in ('linked','frame','a','b','center','maximum','extra','mass','globals'))
        data.append(struct.pack('<I',r['seed']))
    return b''.join(data)


def expected(cases,rows):
    data=[struct.pack('<II',0x31524249,len(rows))]
    for case,r in zip(cases,rows):
        b=case['body_effect'];linked=blob(b,'linked_after_hex',0x424)
        seed=(r['seed']*214013+2531011)%(2**32) if r['random_result'] is not None else r['seed']
        data.extend([linked[0x210:0x230],blob(b,'prepared_a_after_hex',16),blob(b,'body_290_after_hex',40),blob(b,'body_2e0_after_hex',4),struct.pack('<I',seed)])
    return b''.join(data)


def compare(actual,wanted):
    if len(actual)!=len(wanted) or actual[:8]!=wanted[:8]:raise ValueError('Body output framing mismatch')
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_live_words=(len(actual)-8)//96*23,compared_controlled_seed_words=(len(actual)-8)//96,
                mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def pipeline_body(cases,out,profile):
    rows=[from_live(r) for r in cases]
    request=struct.pack('<II',0x31524649,len(rows))+b''.join(blob(r,'linked_before_hex',0x424)[0x1d0:0x210]+blob(r,'body_1630_hex',16) for r in cases)
    native=ROOT/f'target/{profile}/grid-impact-frame.exe';source=out/'pipeline-frame-input.bin';target=out/'pipeline-frame.bin';source.write_bytes(request)
    run=subprocess.run([str(native),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
    report=dict(frame_exit=run.returncode,frame_stdout=run.stdout,frame_stderr=run.stderr,frame_native_sha256=digest(native))
    if run.returncode:raise RuntimeError('Rust contact-frame stage failed')
    frames=target.read_bytes()
    if frames[:8]!=request[:8] or len(frames)!=8+len(rows)*64:raise ValueError('Frame output framing')
    differences=[]
    for i,(case,r) in enumerate(zip(cases,rows)):
        frame=frames[8+i*64:8+(i+1)*64]
        differences.extend(dict(case=i,offset=hex(j)) for j in range(0,64,4) if frame[j:j+4]!=r['frame'][j:j+4])
        r.update(frame=frame,linked=blob(case,'linked_before_hex',0x424),a=blob(case,'prepared_a_hex',16),b=blob(case,'prepared_b_hex',16))
    report.update(compared_frame_words=len(rows)*16,frame_mismatched_words=len(differences),first_frame_differences=differences[:20])
    source=out/'pipeline-body-input.bin';target=out/'pipeline-body.bin';source.write_bytes(pack(rows));native=ROOT/f'target/{profile}/grid-impact-body.exe'
    run=subprocess.run([str(native),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
    report.update(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,native_sha256=digest(native))
    if run.returncode:raise RuntimeError('Rust body pipeline stage failed')
    actual=target.read_bytes();report.update(compare(actual,expected(cases,rows)));report['passed'] &= not differences
    linked_states=[]
    for i,case in enumerate(cases):
        linked=bytearray(blob(case,'linked_before_hex',0x424));linked[0x210:0x230]=actual[8+i*96:8+i*96+32];linked_states.append(linked)
    return report,linked_states


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'));args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture size bound')
    c=json.loads(source.read_text(encoding='utf-8'))
    if not(c['completed'] and c['debugger_exit']==0 and c['hardware_breakpoints'] and c['code_exact'] and c['body_code_exact'] and c['session']['ExeSHA256']==REFERENCE and 1<=len(c['cases'])<=32):raise ValueError('Unverified body-effect capture')
    cases=[r for r in c['cases'] if r.get('body_effect')]
    if not cases:raise ValueError('No observed body helper calls')
    rows=[from_live(r) for r in cases];wanted=expected(cases,rows)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/impact-body-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Reference/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);(out/'input.bin').write_bytes(pack(rows))
    report=dict(capture_sha256=digest(source),probe_sha256=digest(probe),analysis_sha256=ANALYSIS,
                scope='Whole9D4210 and original CRT rand, real TLS APIs, controlled private thread seed yielding observed result. Actual original thread seed was not captured.',
                cases=len(rows),observed_random_calls=sum(r['random_result'] is not None for r in rows),runs={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)])]
    if args.profile:
        native=ROOT/f'target/{args.profile}/grid-impact-body.exe';report.update(profile=args.profile,native_sha256=digest(native));commands.append(('rust',[str(native)]))
    original=None
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(out/'input.bin'),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes();r.update(compare(actual,wanted))
            if original is None:original=actual
            r['repeat_exact']=actual==original
        report['runs'][label]=r
    report['passed']=all(r['exit']==0 and r.get('passed',False) and r['repeat_exact'] for r in report['runs'].values())
    if args.profile and report['passed'] and len(cases)==len(c['cases']) and all(r.get('solver') for r in cases):
        from validate_impact_response import pipeline,live_expected
        report['pipeline']=pipeline(cases,out,args.profile,live_expected(cases),body_stage=pipeline_body)
        report['passed'] &= report['pipeline']['passed']
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Body helper replay failed; inspect retained report')


if __name__=='__main__':main()
