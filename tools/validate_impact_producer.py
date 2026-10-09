"""Whole-original collision producer versus live return and independent Rust stages."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from validate_impact_generation import blob,pack as generation_pack
from validate_impact_body import from_live,pipeline_body
from validate_impact_response import pipeline,live_expected
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='9b7961b8c49ac611d7c8ea7da1a4d2751078e135fd26f5b34328429dbc1d0c38'


def pack(cases):
    data=[struct.pack('<II',0x31574349,len(cases))]
    for case in cases:
        body=from_live(case)
        data.append(generation_pack([case])[8:])
        data.extend(body[k] for k in ('center','maximum','extra','mass','globals'))
        data.extend([struct.pack('<I',body['seed']),blob(case,'prepared_a_hex',16),blob(case,'prepared_b_hex',16),
                     blob(case,'response_scale_hex',4),blob(case,'solver_globals_hex',24),struct.pack('<II',blob(case,'solver_clip_hex',1)[0],int(int(case['response'],16)!=0)),
                     blob(case,'response_before_hex',case['contact_count']*4) if int(case['response'],16) else bytes(case['contact_count']*4)])
    return b''.join(data)


def expected(cases):
    data=[struct.pack('<II',0x31574349,len(cases))]
    for case in cases:
        body=case['body_effect'];r=from_live(case)
        d=bytearray(blob(case,'descriptor_after_hex',0xa500));d[0xa4f8:0xa4fc]=bytes(4)
        seed=(r['seed']*214013+2531011)%(2**32) if r['random_result'] is not None else r['seed']
        data.extend([d,blob(case,'linked_after_hex',0x424),blob(body,'body_290_after_hex',40),blob(body,'body_2e0_after_hex',4),struct.pack('<I',seed),
                     blob(case,'response_after_hex',case['contact_count']*4) if int(case['response'],16) else bytes(case['contact_count']*4)])
    return b''.join(data)


def compare(actual,wanted,count):
    if len(actual)!=len(wanted) or actual[:8]!=wanted[:8]:raise ValueError('Producer output framing mismatch')
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_live_words=(len(actual)-8)//4-count,compared_controlled_seed_words=count,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def rust_output(cases,out):
    solved=(out/'pipeline-solver.bin').read_bytes();body=(out/'pipeline-body.bin').read_bytes();response=(out/'pipeline-response.bin').read_bytes()
    data=[struct.pack('<II',0x31574349,len(cases))];cursor=8
    for i,case in enumerate(cases):
        d=solved[8+i*0xa500:8+(i+1)*0xa500];b=body[8+i*96:8+(i+1)*96]
        linked=bytearray(blob(case,'linked_before_hex',0x424));linked[0x210:0x230]=b[:32];linked[0x380:0x390]=response[cursor:cursor+16]
        dirty,=struct.unpack_from('<I',response,cursor+16)
        if dirty>1:raise ValueError('Dirty boolean out of scope')
        linked[0x390]=dirty;cursor+=20;values=response[cursor:cursor+case['contact_count']*4];cursor+=len(values)
        data.extend([d,linked,b[48:],values])
    if cursor!=len(response):raise ValueError('Trailing Rust response data')
    return b''.join(data)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture size bound')
    c=json.loads(source.read_text(encoding='utf-8'));cases=c['cases']
    if not(c['completed'] and c['debugger_exit']==0 and c['hardware_breakpoints'] and c['code_exact'] and c['body_code_exact'] and c['solver_code_exact'] and c['session']['ExeSHA256']==REFERENCE and 1<=len(cases)<=32 and all(r.get('body_effect') and r.get('solver') for r in cases)):raise ValueError('Unverified producer capture')
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/impact-producer-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Reference/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);(out/'input.bin').write_bytes(pack(cases));wanted=expected(cases);original=None
    report=dict(scope='Whole original9EA6E0, no skipped instructions/helper stubs/code patches. Seven real clock/TLS imports; controlled private CRT seed produces observed result. Helper-captured history/mass/center/globals supplied as fixture inputs.',capture_sha256=digest(source),probe_sha256=digest(probe),analysis_sha256=ANALYSIS,profile=args.profile,runs={})
    for label in ('original','repeat'):
        target=out/f'{label}.bin';run=subprocess.run([str(probe),str(image),str(out/'input.bin'),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes();r.update(compare(actual,wanted,len(cases)))
            if original is None:original=actual
            r['repeat_exact']=actual==original
        report['runs'][label]=r
    report['passed']=all(r.get('passed',False) and r.get('repeat_exact',False) for r in report['runs'].values())
    if report['passed']:
        report['pipeline']=pipeline(cases,out,args.profile,live_expected(cases),body_stage=pipeline_body)
        actual=rust_output(cases,out);(out/'rust-complete.bin').write_bytes(actual)
        report['rust_complete']=compare(actual,wanted,len(cases));report['rust_complete']['original_exact']=actual==original
        report['passed'] &= report['pipeline']['passed'] and report['rust_complete']['passed'] and report['rust_complete']['original_exact']
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole producer replay failed; inspect private report')


if __name__=='__main__':main()
