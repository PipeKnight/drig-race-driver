"""Complete registered callback replay; live boundary and authored cases separate."""
import argparse
import copy
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private,REFERENCE,ANALYSIS
from validate_impact_generation import blob
from validate_impact_producer import pack as producer_pack
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
PROBE_SHA='26151a2684b60976361fb467a509a9e75681d447d531498864a281b93117de8b'


def bodies(cases):
    rows=[]
    for c in cases:
        b=c['callback_body_state'];nested=c.get('body_effect');result=nested['random_result'] if nested else None
        if result is not None and not 0<=result<=32767:raise ValueError('Original rand result bound')
        seed=((result<<16)-2531011)*pow(214013,-1,2**32)%(2**32) if result is not None else 1
        rows.append(dict(center=blob(b,'center_hex',16),maximum=blob(b,'body_290_before_hex',40),extra=blob(b,'body_2e0_before_hex',4),mass=blob(b,'body_15a8_hex',4),globals=blob(b,'globals_hex',20),seed=seed,random_result=result))
    return rows


def pack(cases,body_rows):
    parts=[struct.pack('<II',0x31434349,len(cases))]
    for c,b in zip(cases,body_rows):
        r=copy.deepcopy(c);r['prepared_a_hex']=r['prepared_b_hex']=bytes(16).hex()
        parts.append(producer_pack([r],[b])[8:])
        parts.extend(blob(c,k,n) for k,n in [('body_d16c_hex',4),('body_d22c_hex',4),('global_d1676c_hex',4),('preparation_scale_hex',4),('preparation_table_hex',64)])
        parts.append(struct.pack('<I',c['callback_global_disable']))
    return b''.join(parts)


def expected(cases,body_rows):
    data=[struct.pack('<II',0x31434349,len(cases))]
    for c,b in zip(cases,body_rows):
        d=bytearray(blob(c,'descriptor_after_hex',0xa500));d[0xa4f8:0xa4fc]=bytes(4);state=c['callback_body_state']
        seed=(b['seed']*214013+2531011)%(2**32) if b['random_result'] is not None else b['seed']
        data.extend([d,blob(c,'linked_after_hex',0x424),blob(state,'body_290_after_hex',40),blob(state,'body_2e0_after_hex',4),struct.pack('<I',seed),blob(c,'response_after_hex',c['contact_count']*4) if int(c['response'],16) else bytes(c['contact_count']*4),struct.pack('<I',c['callback_result'])])
    return b''.join(data)


def compare(actual,wanted,count):
    if len(actual)!=len(wanted) or actual[:8]!=struct.pack('<II',0x31434349,count) or actual[:8]!=wanted[:8]:raise ValueError('Callback output framing mismatch')
    differences=[dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=wanted[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=wanted[i:i+4]]
    return dict(compared_buffer_and_return_words=(len(actual)-8)//4-count,compared_controlled_seed_words=count,mismatched_words=len(differences),first_differences=differences[:20],passed=not differences)


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'));args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture bound')
    capture=json.loads(source.read_text(encoding='utf-8'));cases=capture['cases']
    if not(capture['completed'] and capture['debugger_exit']==0 and capture['code_exact'] and capture['hardware_breakpoints'] and capture['boundary']=='callback' and capture['session']['ExeSHA256']==REFERENCE and 1<=len(cases)<=32):raise ValueError('Unverified callback capture')
    body_rows=bodies(cases);wanted=expected(cases,body_rows)
    image,probe=ROOT/'local/compatibility/grid-unpacked.exe',ROOT/'local/behavior/collision-callback-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:raise ValueError('Analysis/probe identity changed')
    out.mkdir(parents=True,exist_ok=False);request=out/'input.bin';request.write_bytes(pack(cases,body_rows));original=None
    report=dict(scope='Complete originalA064A0 callback and original helpers/CRT/solver, seven real Windows imports, no stubs/skips/code patches; controlled private seed when live random occurs. Geometric contacts supplied.',cases=len(cases),capture_sha256=digest(source),probe_sha256=digest(probe),profile=args.profile,observed_positive_record_calls=sum(bool(c.get('solver')) for c in cases),runs={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)])]
    if args.profile:
        native=ROOT/f'target/{args.profile}/grid-collision-callback.exe';report['native_sha256']=digest(native);commands.append(('rust',[str(native)]))
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);r=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if original is None:original=actual
            r.update(compare(actual,wanted,len(cases)));r['repeat_exact']=actual==original;r['passed'] &=r['repeat_exact']
        report['runs'][label]=r
    report['passed']=all(r.get('passed',False) for r in report['runs'].values())
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Whole callback replay failed; inspect private report')


if __name__=='__main__':main()
