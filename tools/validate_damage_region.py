"""Whole localized-damage helper versus source-only Rust and Python arithmetic.

Source-built startup boxes are observed disabled/zero; active boxes below are
authored controls, including separate joins to source HDM coefficients.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import subprocess
from validate_cqtc_decode import sha, f32

IMAGE = Path('local/compatibility/grid-unpacked.exe')
SOURCE = Path('local/behavior/damage-region-20261012-draft-01/source.state.bin')


def draw(state):
    state = (state * 0x41c64e6d + 0x40584c0f) & 0xffffffff
    signed = state if state < 0x80000000 else state - 0x100000000
    quotient = abs(signed) // 65535 * (-1 if signed < 0 else 1)
    return state, f32(((state + quotient) & 255) * struct.unpack('<f', bytes.fromhex('8180803b'))[0])


def record(previous=.2, point=(0.,0.,0.,1.), impact=.5, gain=1., center=(0.,0.,0.,1.), half=(1.,1.,1.,0.), coefficients=(.4,.2,.1), enabled=1, radius=1., factors=(2.,3.), state=0):
    r = bytearray(128)
    for offset, values in [(0,[previous,impact,gain]),(16,point),(32,center),(48,half),(64,coefficients),(96,[radius,*factors])]:
        struct.pack_into('<' + 'f' * len(values), r, offset, *values)
    struct.pack_into('<I', r, 12, enabled); struct.pack_into('<I', r, 112, state)
    return bytes(r)


def inputs():
    rng = random.Random(200679)
    for i in range(4096):
        center = [rng.uniform(-5,5) for _ in range(3)] + [1.]
        half = [rng.uniform(0,3) for _ in range(3)] + [float(i % 2)]
        point = [center[j] + rng.uniform(-1,1) * half[j] + rng.choice([-1,0,0,0,1]) * rng.uniform(0,2) for j in range(3)] + [1.]
        yield 'authored', record(previous=rng.choice([0.,-.0,.2,.99,1.,1.2]), impact=rng.choice([0.,-.0,.01,.5,1.]), gain=rng.choice([0.,-.0,.1,1.,2.]), center=center, half=half, point=point, coefficients=[rng.random() for _ in range(3)], enabled=i%7!=0, radius=rng.choice([.01,.1,1.,2.]), factors=[rng.uniform(0,3) for _ in range(2)], state=rng.getrandbits(32))
    for state in range(2048):
        _, sample = draw(state)
        for threshold in [max(0.,sample - 2**-23),sample,min(1.,sample + 2**-23)]:
            yield 'strict-probability', record(coefficients=(.4,0.,threshold), state=state)
    for axis in range(3):
        for side in [-1.,1.]:
            for distance in [0.,.5,1.,1.+2**-22,2.]:
                p = [0.,0.,0.,1.]; p[axis] = side * (1. + distance)
                yield 'falloff', record(point=p, state=0xffffffff)
    yield 'underflow-zero-radius', record(point=(2**-126,0.,0.,1.), half=(0.,0.,0.,0.), radius=0.)
    source = SOURCE.read_bytes(); assert source[:4] == b'DV01'
    m = 4 + 0xcdf0
    for box, coefficients in [(0x60,0x240),(0x90,0x240),(0xc0,0x240),(0xf0,0x240),(0x120,0x340),(0x150,0x218),(0x180,0x218),(0x1b0,0x304),(0x1e0,0x32c)]:
        center = struct.unpack_from('<4f',source,m+box); half = struct.unpack_from('<4f',source,m+box+16)
        enabled = source[m+box+32]; assert enabled == 0 and center == half == (0.,0.,0.,0.)
        coeff = struct.unpack_from('<3f',source,m+coefficients)
        radius, a, b = struct.unpack_from('<3f',source,m+0x370)
        gains = struct.unpack_from('<3f',source,m+0x430); gain = f32(f32(gains[0]*gains[1])*gains[2])
        for state in range(32):
            yield 'source-disabled-box', record(center=center,half=half,enabled=enabled,coefficients=coeff,radius=radius,factors=(a,b),gain=gain,state=state)
            # The coefficient/settings join has an explicitly authored active box
            # and gain; this is not recovered original car-region initialization.
            yield 'source-coefficients-authored-box', record(coefficients=coeff,radius=radius,factors=(a,b),gain=1.,state=state)


def expected(r):
    u = lambda o: struct.unpack_from('<I',r,o)[0]
    f = lambda o: struct.unpack_from('<f',r,o)[0]
    prior, state = f(0), u(112)
    if not u(12): return struct.pack('<fI',prior,state), False
    d = [f32(f(16+j*4)-f(32+j*4)) for j in range(4)]
    nearest = d[:]; outside=False
    for j in range(3):
        half=f(48+j*4)
        if abs(d[j])>half: outside=True;nearest[j]=half if d[j]>0 else -half
    attenuation=1.
    if outside:
        d=[f32(d[j]-nearest[j]) for j in range(4)]
        sq=[f32(x*x) for x in d]; square=f32(f32(sq[3]+sq[2])+f32(sq[1]+sq[0])); radius=f(96)
        if f32(radius*radius)<square or radius == 0: return struct.pack('<fI',prior,state), False
        attenuation=f32(1.-f32(f32(math.sqrt(square))/radius))
        if attenuation<=0: return struct.pack('<fI',prior,state), False
    amount=f32(f32(f32(attenuation*f(4))*f(64))*f(8))
    state,sample=draw(state)
    if f(72)>sample: amount=f32(amount*f(100))
    elif f32(f(68)+f(72))>sample: amount=f32(amount*f(104))
    level=f32(prior+amount)
    if level>1: level=1.
    return struct.pack('<fI',level,state), True


def execute(command, good=True):
    p=subprocess.run([str(x) for x in command],capture_output=True,text=True,timeout=60)
    assert (p.returncode==0)==good,(command,p.returncode,p.stdout,p.stderr)
    return dict(exit=p.returncode,stdout=p.stdout,stderr=p.stderr)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path)
    ap.add_argument('--native',type=Path,default=Path('local/behavior/damage-region-probe.exe'))
    ap.add_argument('--debug',type=Path,default=Path('target/debug/grid-damage-region.exe'))
    ap.add_argument('--release',type=Path,default=Path('target/release/grid-damage-region.exe'));a=ap.parse_args()
    assert a.output.resolve().is_relative_to(Path('local').resolve());a.output.mkdir(exist_ok=False)
    items=list(inputs());request=b'DM01'+struct.pack('<I',len(items))+b''.join(r for _,r in items)
    inp=a.output/'input.bin';inp.write_bytes(request)
    pinned={str(p):sha(p) for p in [a.native,a.debug,a.release,IMAGE,SOURCE,Path(__file__)]}
    answers={};executions={}
    for name,exe in [('original',a.native),('repeat',a.native),('debug',a.debug),('release',a.release)]:
        target=a.output/f'{name}.bin';command=[exe,IMAGE,inp,target] if name in ['original','repeat'] else [exe,inp,target]
        executions[name]=execute(command)
        if name in ['original','repeat']: assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in executions[name]['stdout']
        answers[name]=target.read_bytes()
    rebuilt=[expected(r) for _,r in items];independent=b''.join(v for v,_ in rebuilt)
    for name,data in answers.items():
        diff=next((i for i,(x,y) in enumerate(zip(data,independent)) if x!=y),None)
        assert data==independent,(name,diff,items[diff//8][0] if diff is not None else '',len(data),len(independent))
    mutants=[('short',request[:-1]),('long',request+b'\x00')]
    for label,offset,value in [('tag',0,0),('count',4,0),('nan',8,0x7fc00000),('enabled',20,2),('negative-half',8+48,0xbf800000),('probability',8+72,0x40000000),('reserved',8+120,1),('point-w',8+28,0),('center-w',8+44,0x40000000),('half-w',8+60,0x40000000)]:
        r=bytearray(request);struct.pack_into('<I',r,offset,value);mutants.append((label,r))
    guards=[]
    for label,r in mutants:
        target_in=a.output/f'guard-{label}.input';target_in.write_bytes(r)
        for name,exe in [('native',a.native),('debug',a.debug),('release',a.release)]:
            target=a.output/f'guard-{label}-{name}.bin';command=[exe,IMAGE,target_in,target] if name=='native' else [exe,target_in,target]
            execute(command,False);assert not target.exists();guards.append(name+':'+label)
    target=a.output/'existing.bin';target.write_bytes(b'KEEP')
    for name,exe in [('native',a.native),('debug',a.debug),('release',a.release)]:
        execute([exe,IMAGE,inp,target] if name=='native' else [exe,inp,target],False);assert target.read_bytes()==b'KEEP';guards.append(name+':existing')
    assert all(sha(p)==digest for p,digest in pinned.items())
    counts={name:sum(label==name for label,_ in items) for name in sorted({label for label,_ in items})}
    report=dict(passed=True,cases=len(items),observable_bytes_per_run=len(independent),random_draws=sum(draw for _,draw in rebuilt),labels=counts,inputs=pinned,request_sha256=sha(inp),outputs={name:sha(a.output/f'{name}.bin') for name in answers},executions=executions,guards=guards,source_box_observation='all nine selected prototype startup regions disabled/zero; source coefficients joined to separately authored active boxes',runtime_damage=False)
    (a.output/'validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ['passed','cases','observable_bytes_per_run','random_draws','labels']}))


if __name__=='__main__':main()
