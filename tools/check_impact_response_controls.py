"""Authored response-slice controls; these are separate from live verification."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess
from capture_reference_spawn import private
from validate_impact_response import from_live, pack, execute, ROOT


def controls(capture):
    rng=random.Random(20081009)
    rows=[]
    for i in range(192):
        row=dict(nodes=[[0.,0.,0.,0.],[2.,3.,-1.,0.5],[1.,-2.,4.,1.]],
                 contacts=[dict(enabled=1,reverse=0,baseline=.25,positive=1.,normal=[1.,0.,0.,7.]),
                           dict(enabled=1,reverse=0,baseline=.5,positive=1.,normal=[0.,1.,0.,-9.]),
                           dict(enabled=1,reverse=0,baseline=1.,positive=1.,normal=[0.,0.,1.,3.])],
                 records=[dict(direction=[1.,0.,0.,0.],anchor=[0.,0.,0.,0.],node=1,contact=0,weight=1.),
                          dict(direction=[0.,1.,0.,0.],anchor=[0.,0.,0.,0.],node=1,contact=1,weight=.5)],
                 present=1,policy=i%2,dirty=0,scale=1.,gain=.75,velocity=[3.,1.,.1,0.],accumulator=[.25,-.5,.75,-0.],response=[2.,-1.,.5])
        variant=i//2%24
        if variant==1:row['velocity']=[0.]*4
        elif variant==2:row['velocity']=[0.0009999,0.,0.,0.]
        elif variant==3:row['velocity']=[0.0010001,0.,0.,0.]
        elif variant==4:row['velocity']=[3.,1.,.1,4.]
        elif variant==5:row['velocity']=[-3.,-1.,-.1,-0.]
        elif variant==6:
            row['contacts'][0]['reverse']=1;row['velocity'][0]=-3.
        elif variant==7:
            for c in row['contacts']:c['reverse']=1
        elif variant==8:row['gain']=0.
        elif variant==9:row['gain']=-.5
        elif variant==10:row['scale']=0.
        elif variant==11:row['scale']=-2.
        elif variant==12:
            for c in row['contacts']:c['baseline']=-.125
        elif variant==13:
            for r in row['records']:r['weight']=0.
        elif variant==14:
            for r in row['records']:r['weight']=-1.
        elif variant==15:
            row['records'].insert(1,dict(direction=[0.,0.,1.,1.],anchor=[.25,.25,.25,-.5],node=2,contact=0,weight=-1.))
        elif variant==16:
            row['contacts'][1]['normal']=[1.,0.,0.,-9.];row['records'][1]['direction']=[1.,0.,0.,0.]
            row['contacts'][1]['baseline']=.25;row['records'][1]['weight']=1.
        elif variant==17:row['present']=0
        elif variant==18:row['records']=[]
        elif variant==19:
            row['records']=row['records'][1:];row['contacts'][0]['enabled']=0
        elif variant==20:
            row['records']=row['records'][:1];row['contacts'][1]['positive']=0.
        elif variant==21:row['dirty']=1
        elif variant==22:
            for c in row['contacts']:c['normal']=[0.,0.,0.,7.]
        elif variant==23:
            row['records']=[copy.deepcopy(row['records'][0]) for _ in range(400)]
            row['contacts'] += [copy.deepcopy(row['contacts'][2]) for _ in range(77)]
            row['response'] += [0.]*77
        if i>=48:
            row['gain']*=rng.uniform(.2,2.)
            row['scale']*=rng.uniform(.2,2.)
            row['accumulator']=[rng.uniform(-5.,5.) for _ in range(4)]
            row['response']=[rng.uniform(-3.,3.) for _ in row['contacts']]
            if i>=96:
                for r in row['records']:
                    r['direction']=[rng.uniform(-2.,2.) for _ in range(4)]
                    r['anchor']=[rng.uniform(-1.,1.) for _ in range(4)]
        rows.append(row)
    for i in range(64):
        row=from_live(capture['cases'][i%len(capture['cases'])])
        row['velocity']=[rng.uniform(-30.,30.) for _ in range(4)]
        row['accumulator']=[rng.uniform(-2.,2.) for _ in range(4)]
        row['gain']=rng.uniform(-1.,2.);row['scale']=rng.uniform(-2.,3.)
        row['policy']=i%2;row['dirty']=int(i%5==0)
        row['response']=[rng.uniform(-1.,1.) for _ in row['contacts']]
        rows.append(row)
    return rows


def guards(row):
    good=pack([row]);base=bytearray(good)
    nodes=len(row['nodes']);contacts=len(row['contacts']);contact_offset=8+64+nodes*16;record_offset=contact_offset+contacts*32
    cases={}
    def changed(name,offset,fmt,value):
        data=bytearray(base);struct.pack_into(fmt,data,offset,value);cases[name]=bytes(data)
    changed('node_index',record_offset+32,'<H',nodes)
    changed('contact_index',record_offset+34,'<H',contacts)
    changed('record_order',record_offset+34,'<H',2)
    changed('disabled_record_contact',contact_offset,'<I',0)
    changed('nan_weight',record_offset+36,'<I',0x7fc00000)
    changed('invalid_policy',8+16,'<I',2)
    changed('record_bound',8+8,'<I',401)
    cases['trailing']=good+b'\0';cases['truncated']=good[:-1]
    return cases


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    capture=json.loads(source.read_text(encoding='utf-8'));rows=controls(capture)
    report=execute(rows,out,args.profile)
    report.update(authored_cases=len(rows),seed=20081009,live_verification=False,guards={})
    for name,data in guards(rows[0]).items():
        request=out/f'guard-{name}.bin';request.write_bytes(data);checks={}
        for label,command in [('original',[str(ROOT/'local/behavior/impact-response-probe.exe'),str(ROOT/'local/compatibility/grid-unpacked.exe')]),('rust',[str(ROOT/f'target/{args.profile}/grid-impact-response.exe')])]:
            target=out/f'guard-{name}-{label}.bin'
            run=subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            checks[label]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,output_exists=target.exists(),passed=run.returncode>0 and (label=='original' or not target.exists()))
        report['guards'][name]=checks
    report['passed'] &= all(r['passed'] for g in report['guards'].values() for r in g.values())
    # Require the original controls to actually change the accumulator, rather than
    # treating a suite of inactive branches as correction coverage.
    report['passed'] &= report['runs']['original'].get('original',{}).get('accumulator_changed_cases',0)>0
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Authored response controls failed; inspect retained report')


if __name__=='__main__':main()
