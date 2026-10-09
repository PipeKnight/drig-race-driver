"""Whole original serial manager versus Rust plan; actual callbacks guard off."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from validate_response_consumption import ROOT, ANALYSIS, digest

PROBE = '8a2fcfff6f0a5c910fd0961b1675de55c8bac19ae7b911ec5a114307eacc1331'


def pack(rows):
    data = [struct.pack('<II', 0x314d4353, len(rows))]
    for r in rows:
        data.append(struct.pack('<5I', len(r['owners']), len(r['pools'][0]), len(r['pools'][1]), r['reverse'], r['passes']))
        data.extend(struct.pack('<2I', *o) for o in r['owners'])
        data.extend(struct.pack('<6I', *c) for pool in r['pools'] for c in pool)
    return b''.join(data)


def controls():
    def row(owners, a, b, reverse=0, passes=4): return dict(owners=owners, pools=[a,b], reverse=reverse, passes=passes)
    rows = [row([],[],[],255), row([[1,1]],[[10,1,0,1,0,0]],[],255),
            row([[1,1],[2,1]],[[10,1,2,0,0,0],[11,2,1,0,1,0]],[]),
            row([[1,1],[2,1],[3,1]],[],[[10,1,2,0,0,0],[11,3,1,0,0,0]]),
            row([[1,1]],[[10+i,1,0,1,0,0] for i in range(120)],[]),
            row([[i+1,1] for i in range(32)],[[10+i,i%32+1,0,1,0,0] for i in range(512)],[]),
            row([[1,0],[2,0]],[],[[10,1,2,0,0,0]],7)]
    rng = random.Random(20081010)
    while len(rows)<128:
        n=rng.randrange(1,33);owners=[[i+1,rng.randrange(2)] for i in range(n)]
        a=[[10+i,rng.randrange(1,n+1),rng.randrange(n+1),rng.randrange(2),rng.randrange(2),rng.randrange(2)] for i in range(rng.randrange(129))]
        b=[[200+i,rng.randrange(1,n+1),rng.randrange(1,n+1),0,rng.randrange(2),rng.randrange(2)] for i in range(rng.randrange(129))]
        rows.append(row(owners,a,b,rng.choice([0,1,7,255]),rng.randrange(1,5)))
    return rows


def expected(rows):
    data=[struct.pack('<II',0x314d4353,len(rows))]; metrics=dict(callback_calls=0,selected_contacts=0,reversals=0,unchanged_small_lists=0)
    for r in rows:
        flags=dict(r['owners']);eligible=[]
        for id in [c[1] for c in r['pools'][0]]+[id for c in r['pools'][1] for id in c[1:3]]:
            if flags[id] and id not in eligible:eligible.append(id)
        reverse=r['reverse'];data.append(struct.pack('<I',r['passes']))
        for _ in range(r['passes']):
            if len(eligible)>1:reverse=int(reverse==0)
            else:metrics['unchanged_small_lists']+=1
            order=eligible[::-1] if len(eligible)>1 and reverse else eligible
            metrics['reversals']+=int(len(eligible)>1 and reverse!=0);calls=[]
            for owner in order:
                contacts=[c[0] for c in r['pools'][0] if c[1]==owner and not any(c[4:6])]
                contacts += [c[0] for c in r['pools'][1] if (c[1]==owner or c[2]==owner) and not any(c[4:6])]
                if contacts:calls.append((owner,contacts[:80]))
            data.append(struct.pack('<3I',reverse,len(calls),0))
            for owner,contacts in calls:
                metrics['callback_calls']+=1;metrics['selected_contacts']+=len(contacts)
                data.append(struct.pack('<'+'I'*(len(contacts)+2),owner,len(contacts),*contacts))
    return b''.join(data),metrics


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);p.add_argument('--profile',choices=('debug','release'),required=True);args=p.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):raise ValueError('Private output required')
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/serial-collision-dispatch-probe.exe';rust=ROOT/f'target/{args.profile}/grid-serial-collision-dispatch.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE:raise ValueError('Reference/probe identity mismatch')
    rows=controls();wanted,metrics=expected(rows);out.mkdir(parents=True,exist_ok=False);source=out/'input.bin';source.write_bytes(pack(rows))
    report=dict(live_verification=False,scope='Whole B78710 serial branch with original B77770/B775D0/B784B0/A064A0 guard. Hardware observes callback arguments and any consumer invocation; no code changes/stubs. Rust dispatch plan only, not active callback/body simulation.',profile=args.profile,cases=len(rows),passes=sum(r['passes'] for r in rows),seed=20081010,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),activity=metrics,runs={},guards={})
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    for label,command in commands:
        target=out/f'{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60);data=target.read_bytes() if target.exists() else b''
        different=[i for i in range(8,min(len(data),len(wanted)),4) if data[i:i+4]!=wanted[i:i+4]]
        report['runs'][label]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,compared_words=(len(wanted)-8)//4,mismatched_words=len(different),first_differences=different[:20],passed=run.returncode==0 and data==wanted)
    base=rows[2];good=pack([base]);bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:])
    for name,change in [('invalid_toggle',lambda r:r.__setitem__('reverse',256)),('zero_passes',lambda r:r.__setitem__('passes',0)),('invalid_owner',lambda r:r['owners'][0].__setitem__(0,0)),('invalid_flag',lambda r:r['owners'][0].__setitem__(1,2)),('zero_contact',lambda r:r['pools'][0][0].__setitem__(0,0)),('null_first',lambda r:r['pools'][0][0].__setitem__(1,0)),('invalid_kind',lambda r:r['pools'][0][0].__setitem__(3,2))]:
        r=copy.deepcopy(base);change(r);bad[name]=pack([r])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);report['guards'][name]={}
        for label,command in (commands[0],commands[2]):
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=15)
            report['guards'][name][label]=dict(exit=run.returncode,passed=0<run.returncode<256 and (label=='original' or not target.exists()))
    report['passed']=all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Serial dispatcher comparison failed; inspect private report')


if __name__=='__main__':main()
