"""Whole selected mutable handling, full owned records and carried branches."""
import argparse
import collections
import json
import random
import struct
from pathlib import Path
from validate_vehicle_propulsion import ROOT, REFERENCE, digest
import validate_vehicle_contact_force as execution
import validate_vehicle_publication as publication
CAR=0x21000; PHYSICAL=0x424; RECORD=4+CAR+PHYSICAL; M=0xcdf0
NATIVE=ROOT/'local/behavior/vehicle-handling-probe.exe'
BIN='grid-vehicle-handling'; MAGIC=b'VH01'; PUBLISH=False
def put(b,o,x):struct.pack_into('<f',b,o,x)
def word(b,o,x):struct.pack_into('<I',b,o,x)
def request(rows):return MAGIC+struct.pack('<I',len(rows))+b''.join(rows)
def cases():
    rng=random.Random(2008191);rows=[]
    for i in range(256):
        car=bytearray(rng.randbytes(CAR));physical=bytearray(rng.randbytes(PHYSICAL))
        for o in range(0,0x60,4):put(car,M+o,rng.uniform(-.2,1.5))
        for o in range(0x218,0x37c,4):put(car,M+o,rng.uniform(.01,1.8))
        for wheel in range(4):
            for j in range(4):
                put(car,M+0x60+wheel*0x30+j*4,rng.uniform(-2,2))
                put(car,0x1650+wheel*16+j*4,rng.uniform(-.2,.2))
            put(car,0x1790+wheel*4,rng.uniform(-.1,.1))
            put(car,0x1aa8+wheel*0x150,[-1,0,1,2,4,10][(i+wheel)%6])
            put(car,0x1ac8+wheel*0x150,[-1,0,10,30,100,2500][(i+wheel)%6])
        for o in range(0x1d0,0x230,4):put(physical,o,rng.uniform(-2,2))
        for o in range(0xc740,0xc750,4):put(car,o,rng.uniform(-.2,.2))
        for o,x in [(0x48,rng.uniform(0,1)),(0x4c,rng.uniform(0,1)),(0x254,[0,.3,.9,1.2][i%4]),(0x274,[.2,.65,1.2][i%3]),(0x278,.95),(0x280,[0,-0.,27][i%3]),(0x284,.45),(0x2f0,[1,999999][i%2]),(0x2f4,10),(0x3a0,rng.uniform(0,1)),(0x42c,[-.1,0,.001,.2][i%4]),(0x430,.8),(0x434,1.2),(0x438,.9),(0x368,.8)]:put(car,M+o,x)
        # Avoid unconsumed opaque words masquerading as owner pointers.
        word(car,0xdb0,0);word(car,M+0x214,0);word(car,0x1620,4)
        car[0xb13]=car[0xb14]=1;word(car,0x22e4,0)
        car[M+0x442]=0;word(car,M+0x444,7);car[M+0x440]=i%2
        car[0xc7ae]=i%2;put(car,0xeefc,.7)
        word(car,0x2118,i%2);word(car,0x211c,(i//2)%3);word(car,0x196c,i%5);word(car,M+0x428,(i+int(i%3==0))%5)
        put(car,0x18c4,[-1,0,.2,2][i%4]);put(physical,0x3f0,1140)
        put(physical,0x1e4,[-.1,-0.,0,.5,1][i%5])
        if i%4==0:
            for j,x in enumerate([.01,-.02,.03,-.04]):put(physical,0x210+j*4,x)
        if PUBLISH:
            publication.configure(car)
            put(car,0xc798,[-.1,0,.001,.1][i%4]);put(car,0xc790,[0,.001,.002,.5][i%4])
            for base in [0x15c0,0x15d0]:
                for j in range(3):put(car,base+j*4,rng.uniform(-.2,.2) if i%4==0 else rng.uniform(-2,2))
                word(car,base+12,[0,0x80000000,0x7fc00000,0x7f800000][i%4])
        rows.append(struct.pack('<f',[0,.001,.006,.016,.02,.04,.1][i%7])+car+physical)
    return rows
def compare(out,profile,branches,prefix):
    outputs={};runs=[]
    for kind,rows in branches.items():
        source=out/f'{prefix}-{kind}-input.bin';target=out/f'{prefix}-{kind}-output.bin';source.write_bytes(request(rows))
        exe=ROOT/f'target/{profile}/{BIN}.exe' if kind=='rust' else NATIVE
        run=execution.execute(exe,[source,target] if kind=='rust' else [REFERENCE,source,target],out,f'{prefix}-{kind}');runs.append(run)
        if kind!='rust':assert 'code_unchanged=1 globals_restored=1 error=0' in run['stdout']
        outputs[kind]=target.read_bytes();assert len(outputs[kind])==8+len(rows)*RECORD
    a,repeat,rust=[outputs[k] for k in ['original','repeat','rust']]
    differences=[dict(case=(o-8)//RECORD,offset=hex((o-8)%RECORD),original=a[o:o+4].hex(),rust=rust[o:o+4].hex()) for o in range(8,len(a),4) if a[o:o+4]!=rust[o:o+4]]
    coverage=collections.Counter()
    for i,row in enumerate(branches['original']):
        after=a[8+i*RECORD:8+(i+1)*RECORD]
        for name,o,n in [('wheel_levels',M+0x18,16),('wear_levels',M+0x38,16),('wear_rng',0x1644,4),('body_wear',M+0xc,4),('rollover',M+0x440,1),('gear_damage',M+0x14,4),('gear_history',M+0x428,8),('effects',M+0x3a0,0x88),('normalizations',M+0x218,0x140)]:coverage[name]+=row[4+o:4+o+n]!=after[4+o:4+o+n]
        delta=struct.unpack_from('<f',row)[0];coverage['zero_delta']+=delta==0;coverage['clamped_delta']+=delta>.02
        expected=bytearray(row[4+CAR:])
        if PUBLISH:
            for source,target in [(0x15c0,0x220),(0x15d0,0x210)]:
                expected[target:target+12]=row[4+source:4+source+12];word(expected,target+12,0)
                assert after[4+source+12:4+source+16]==bytes(4),'Publication failed to clear W'
            for name,o,n in [('timer',0xc798,4),('decay',0xc790,4),('published_velocity',CAR+0x210,32)]:coverage[name]+=row[4+o:4+o+n]!=after[4+o:4+o+n]
        assert after[4+CAR:]==expected, 'Physical body differs from the selected publication contract'
        assert after[4+M+0x214:4+M+0x218]==b'\0'*4,'Owned component token was not restored'
        assert after[4+M+0x442:4+M+0x448]==row[4+M+0x442:4+M+0x448],'Notification branch changed'
    report=dict(calls=len(branches['original']),record_bytes=len(a)-8,repeat_exact=a==repeat,differing_words=len(differences),differences=differences,coverage=dict(coverage),executions=runs)
    (out/f'{prefix}-validation.json').write_text(json.dumps(report,indent=2));assert a==repeat and not differences,dict(count=len(differences),first=differences[:20])
    return report,{k:[data[8+i*RECORD:8+(i+1)*RECORD] for i in range(len(branches[k]))] for k,data in outputs.items()}
def guards(out,profile,first):
    mutations=[('nan_delta',0,0x7fc00000),('negative_delta',0,0xbf800000),('large_delta',0,0x3e800000)]
    mutations += [(name,4+o,v) for name,o,v in [('foreign_wrapper',0xdb0,1),('foreign_component',M+0x214,1),('wheel_count',0x1620,5),('inactive_car',0xb10,0),('terminal_wheel',0x22e4,1),('notifications',M+0x440,0x10000),('status',M+0x444,8),('style_a',0x2118,2),('style_b',0x211c,3),('nan_level',M+0x18,0x7fc00000),('nan_parameter',M+0x318,0x7fc00000),('nan_deformation',0x1650,0x7fc00000),('nan_pose',CAR+0x1e4,0x7fc00000),('zero_mass',CAR+0x3f0,0),('active_collision',0x1ac8,0x49742400),('zero_wear_divisor',M+0x2f4,0),('negative_visual_limit',M+0x280,0xbf800000),('zero_visual_divisor',M+0x284,0)]]
    if PUBLISH:mutations += [(name,4+o,v) for name,o,v in [('render',0xdb8,1),('setup',0xc7f0,0x10000),('ui',0xc7a0,1),('nan_timer',0xc798,0x7fc00000),('nan_velocity',0x15d0,0x7fc00000)]]
    checks=[]
    for name,o,v in mutations:
        row=bytearray(first);struct.pack_into('<I',row,o,v)
        if name in ['active_collision','zero_wear_divisor']:put(row,0,.016)
        if name=='zero_wear_divisor':put(row,4+M+0x2f0,1);put(row,4+0x1aa8,10)
        if name=='zero_visual_divisor':put(row,4+M+0x280,27)
        for kind in ['original','rust']:
            source=out/f'guard-{name}-{kind}-input.bin';target=out/f'guard-{name}-{kind}-output.bin';source.write_bytes(request([row]));exe=ROOT/f'target/{profile}/{BIN}.exe' if kind=='rust' else NATIVE
            run=execution.execute(exe,[source,target] if kind=='rust' else [REFERENCE,source,target],out,f'guard-{name}-{kind}',True)
            passed=(0<run['exit']<=33 and not run['stderr'] and 'completed=0 code_unchanged=1 globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));assert all(x['passed'] for x in checks),[x for x in checks if not x['passed']];return checks
def framing(out,profile,first):
    valid=request([first]);checks=[]
    for name,data in [('tag',b'NOPE'+valid[4:]),('empty',MAGIC+bytes(4)),('count65',MAGIC+struct.pack('<I',65)),('truncated',valid[:-1]),('trailing',valid+b'!')]:
        for kind in ['original','rust']:
            source=out/f'framing-{name}-{kind}-input.bin';target=out/f'framing-{name}-{kind}-output.bin';source.write_bytes(data);exe=ROOT/f'target/{profile}/{BIN}.exe' if kind=='rust' else NATIVE
            run=execution.execute(exe,[source,target] if kind=='rust' else [REFERENCE,source,target],out,f'framing-{name}-{kind}',True)
            passed=(0<run['exit']<=33 and not run['stderr'] and 'code_unchanged=1 globals_restored=1' in run['stdout'] if kind=='original' else run['exit']==1) and not target.exists()
            checks.append(dict(name=name,kind=kind,passed=passed,execution=run))
    (out/'framing.json').write_text(json.dumps(checks,indent=2));assert all(x['passed'] for x in checks);return checks
def main():
    global PUBLISH,NATIVE,BIN,MAGIC
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--publication',action='store_true');a=p.parse_args();out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    if a.publication:
        PUBLISH=True;NATIVE=ROOT/'local/behavior/vehicle-handling-publication-probe.exe';BIN='grid-vehicle-handling-publication';MAGIC=b'VN01'
    rows=cases();reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),32):
        r,_=compare(out,a.profile,{k:rows[start:start+32] for k in ['original','repeat','rust']},f'initial-{start}');reports.append(r);coverage.update(r['coverage'])
    branches={k:rows[16:32] for k in ['original','repeat','rust']}
    for frame in range(8):
        r,branches=compare(out,a.profile,branches,f'carry-{frame}');reports.append(r);coverage.update(r['coverage'])
    controls=[]
    for row in rows[16:22]:
        for delta in [.02,.1]:controls.append(struct.pack('<f',delta)+row[4:])
    r,answers=compare(out,a.profile,{k:controls for k in ['original','repeat','rust']},'delta-cap');reports.append(r);coverage.update(r['coverage'])
    semantic=[dict(name='same_handling_rng_and_physical_at_02_and_10',case=i,passed=answers['original'][2*i][4+M:4+M+0x460]==answers['original'][2*i+1][4+M:4+M+0x460] and answers['original'][2*i][4+0x1644:4+0x1648]==answers['original'][2*i+1][4+0x1644:4+0x1648] and answers['original'][2*i][4+CAR:]==answers['original'][2*i+1][4+CAR:]) for i in range(6)]
    assert all(x['passed'] for x in semantic);(out/'semantic-checks.json').write_text(json.dumps(semantic,indent=2))
    scope='Whole unchanged9F3E80; four wheels, collision inputs below threshold, component mode0/query0, notification442false/status7/no terminal flags; physical unchanged.'
    if PUBLISH:scope='Whole unchanged A03220(delta,1) with whole9F3E80; stable setup/empty manager/render/UI; physical velocity setters precede mutable update; component mode0/query0, notification442false/status7, collision inputs below threshold.'
    checks=guards(out,a.profile,rows[1]);frames=framing(out,a.profile,rows[1]);result=dict(profile=a.profile,publication=PUBLISH,calls=sum(x['calls'] for x in reports),record_bytes=sum(x['record_bytes'] for x in reports),coverage=dict(coverage),guards=len(checks),framing=len(frames),semantic_checks=semantic,reports=reports,executables={str(p.relative_to(ROOT)):digest(p) for p in [NATIVE,ROOT/f'target/{a.profile}/{BIN}.exe']},scope=scope+' Supplied mutable/source state, not live CarJob or original constructor.')
    assert all(coverage[k]>0 for k in ['wheel_levels','wear_levels','wear_rng','body_wear','rollover','gear_damage','gear_history','effects','normalizations','zero_delta','clamped_delta'])
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k!='reports'}))
if __name__=='__main__':main()
