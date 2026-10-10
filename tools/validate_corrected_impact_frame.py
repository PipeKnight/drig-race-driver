"""Whole outer impact frame with main and optional correction joined."""
import argparse
import copy
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private, ANALYSIS
from validate_contact_bias import compare, execute
from validate_impact_frame import pack as frame_pack, decode as frame_decode, layouts, PROBE as FRAME_PROBE
from validate_main_contact_response import fput, uput, fget, uget
from validate_steering_control import digest

ROOT=Path(__file__).resolve().parents[1]
MAGIC=0x32524650
PROBE='d5061ab307ece7dae07d97e3a2915962facc6f5867f692a4377a7ce9e01acfbe'


def pack(cases):
    data=bytearray(struct.pack('<II',MAGIC,len(cases)))
    for c in cases:
        raw=frame_pack([c])[8:]
        data+=raw[:144]+struct.pack('<I4f',*c['correction'])+raw[144:]
    return data


def decode(data,cases):
    raw=bytearray(data);uput(raw,0,0x31524650);return frame_decode(raw,cases)


def semantic(cases,baseline,decoded):
    checks=dict(bypasses_retained=True,main_contacts_retained=True,tangents_retained=True,cache_properties_retained=True,optional_span_reset=True,optional_row_reset=True,finished_force_cleanup=True)
    activity=dict(invoked=0,active=0,inactive=0,disabled_by_iterations=0,disabled_by_relaxation=0,changed_poses=0,normal_rows=0,friction_rows=0,empty_contacts=0)
    for c,b,o in zip(cases,baseline,decoded):
        iterations,softness,scale,threshold,relaxation=c['correction']
        invoked=bool(o['started'] and o['status']==0 and iterations and relaxation>0.)
        active=invoked and any(not raw[0xe] and (fget(raw,0x38)-threshold)*scale>=0. for _,raw in c['contacts'])
        activity['invoked']+=invoked;activity['active']+=bool(active);activity['inactive']+=invoked and not active
        activity['disabled_by_iterations']+=iterations==0;activity['disabled_by_relaxation']+=relaxation<=0.;activity['empty_contacts']+=not c['contacts']
        checks['main_contacts_retained'] &= o['contacts']==b['contacts']
        checks['cache_properties_retained'] &= o['cache']==b['cache'] and o['properties']==b['properties']
        if not invoked:checks['bypasses_retained'] &= o==b
        if o['started'] and o['status']==0 and c['contacts']:
            n=b['ends'][1];total=b['ends'][3];activity['normal_rows']+=n;activity['friction_rows']+=total-n
            checks['tangents_retained'] &= o['rows'][n:]==b['rows'][n:]
            if invoked:
                checks['optional_span_reset'] &= o['ends'][2:]==(n,n)
                checks['optional_row_reset'] &= all(row[0x80:0x84]==bytes(4) and row[0x84:0x88]==struct.pack('<f',softness) for row in o['rows'][:n])
        activity['changed_poses']+=any(x[0x190:0x210]!=y[0x190:0x210] or x[0xd0:0x150]!=y[0xd0:0x150] for x,y in zip(b['bodies'],o['bodies']))
        if o['started'] and o['status']==0:
            checks['finished_force_cleanup'] &= all(body[0x250:0x270]==bytes(32) and not body[0x40d] and not body[0x40e] for body in o['bodies'])
    return checks,activity


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/corrected-impact-frame-probe.exe';rust=ROOT/f'target/{args.profile}/grid-corrected-physical-frame.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE:raise ValueError('Image/probe identity')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    prior=ROOT/f'local/behavior/impact-frame-20261010-{args.profile}-final-04';upstream=prior/'input.bin';baseline=(prior/'original.bin').read_bytes()
    if digest(upstream)!='b53b3b376e88643e592f3fe4194de5af307c5f80266155b6c376cc67654cfe1e' or any((prior/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust']):raise ValueError('Prior frame identity/agreement')
    frame_probe=ROOT/'local/behavior/impact-frame-probe.exe'
    if digest(frame_probe)!=FRAME_PROBE:raise ValueError('Prior frame probe identity')
    upstream_runs={}
    for label,command in [('original',[str(frame_probe),str(image)]),('rust',[str(ROOT/f'target/{args.profile}/grid-physical-frame.exe')])]:
        data,run=execute(command,upstream,out/f'upstream-{label}.bin');run.update(compare(data,baseline));upstream_runs[label]=run
        if not run['passed']:raise ValueError('Fresh prior frame mismatch')
    cases=layouts(upstream.read_bytes())
    for i,c in enumerate(cases):
        c['correction']=[[0,1,2,4,8,16,32][i%7],[.01,.1,1.][i%3],[.0001,0.,-.0001][(i//3)%3],0. if i%2 else 1.,[0.,.5,1.][(i//9)%3]]
    source=out/'input.bin';source.write_bytes(pack(cases));report=dict(profile=args.profile,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),input_sha256=digest(source),cases=len(cases),upstream_runs=upstream_runs,
        scope='Whole B7B850 impact-only main+optional correction, natural no-owner guards, no body links/shapes/wakeup; authored optional settings; no live frame.',runs={},guards={},live_verification=False)
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])];original=None
    for label,command in commands:
        data,run=execute(command,source,out/f'{label}.bin')
        if original is None:original=data
        run.update(compare(data,original));report['runs'][label]=run
    checks,activity=semantic(cases,frame_decode(baseline,cases),decode(original,cases));report.update(semantic_checks=checks,activity=activity)
    base=copy.deepcopy(cases[3]);good=pack([base]);bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:])
    for name,mutate in [('excess_correction_iterations',lambda c:c['correction'].__setitem__(0,33)),('nan_softness',lambda c:c['correction'].__setitem__(1,float('nan'))),
                        ('nan_scale',lambda c:c['correction'].__setitem__(2,float('nan'))),('nan_threshold',lambda c:c['correction'].__setitem__(3,float('nan'))),('nan_relaxation',lambda c:c['correction'].__setitem__(4,float('nan'))),
                        ('body_callback',lambda c:uput(c['bodies'][0]['raw'],0x418,1)),('body_links',lambda c:uput(c['bodies'][0]['raw'],0x324,1)),('stale_metadata',lambda c:c['contacts'][0][1].__setitem__(0x70,1))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,request in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(request);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,passed=0<run.returncode<256 and 'corrected_impact_frame_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for r in upstream_runs.values()) and all(r['passed'] for guard in report['guards'].values() for r in guard.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Corrected whole frame comparison failed; inspect private evidence')


if __name__=='__main__':main()
