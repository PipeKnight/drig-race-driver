"""Bounded isolated x86 probe of the pinned private GRID lighting calculation.

Never loads the executable into Windows, attaches to a process or invokes imports.
The sole engine-environment getter is explicitly stubbed; runtime fidelity is not
claimed. Outputs containing game-derived values must stay under ignored local/.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'local/tools/graphics-python'))
import unicorn
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_EIP,
                               UC_X86_REG_ESP, UC_X86_REG_MXCSR, UC_X86_REG_CR4)

HASH = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
ENTRY = 0x730f90


def probe(data, height, rotation):
    u16 = lambda off: struct.unpack_from('<H', data, off)[0]
    u32 = lambda off: struct.unpack_from('<I', data, off)[0]
    if data[:2] != b'MZ':
        raise ValueError('not PE')
    nt = u32(60)
    if data[nt:nt+4] != b'PE\0\0' or u16(nt+4) != 0x14c or u16(nt+24) != 0x10b:
        raise ValueError('expected x86 PE32')
    base, size, header = u32(nt+52), u32(nt+80), u32(nt+84)
    if base != 0x400000 or not 0 < size <= 64*1024*1024:
        raise ValueError('unsupported image mapping')
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(base, (size+4095) & ~4095)
    uc.mem_write(base, data[:header])
    sh = nt+24+u16(nt+20)
    for i in range(u16(nt+6)):
        _, virtual_size, rva, raw_size, raw = struct.unpack_from('<8sIIII',data,sh+40*i)
        if rva+max(virtual_size,raw_size)>size or raw+raw_size>len(data):
            raise ValueError('invalid section bounds')
        if raw_size: uc.mem_write(base+rva,data[raw:raw+raw_size])
    obj, env, stack, stop = 0x20000000, 0x21000000, 0x30000000, 0x30020000
    uc.mem_map(obj, 0x4000); uc.mem_map(env,0x10000); uc.mem_map(stack,0x30000)
    def scalar(off,v): uc.mem_write(obj+off,struct.pack('<f',v))
    for off,v in ((0x278,height),(0x27c,rotation),(0x280,height),(0x284,rotation),
                  (0x2c0,500.),(0x2c4,0.4)):
        scalar(off,v)
    sp=stack+0xfffc
    uc.mem_write(sp,struct.pack('<I',stop)); uc.reg_write(UC_X86_REG_ESP,sp)
    uc.reg_write(UC_X86_REG_ECX,obj); uc.reg_write(UC_X86_REG_MXCSR,0x1f80)
    uc.reg_write(UC_X86_REG_CR4,uc.reg_read(UC_X86_REG_CR4)|0x600)
    state={'instructions':0,'stubbed_environment_calls':0}
    def hook(cpu, address, size, _):
        state['instructions']+=1
        if address==0x98ce10:
            state['stubbed_environment_calls']+=1
            esp=cpu.reg_read(UC_X86_REG_ESP)
            ret=struct.unpack('<I',cpu.mem_read(esp,4))[0]
            cpu.reg_write(UC_X86_REG_EAX,env); cpu.reg_write(UC_X86_REG_ESP,esp+4)
            cpu.reg_write(UC_X86_REG_EIP,ret)
        elif not ENTRY<=address<0x732967:
            raise RuntimeError(f'unexpected instruction address {address:x}')
    uc.hook_add(UC_HOOK_CODE,hook)
    uc.emu_start(ENTRY,stop,timeout=1_000_000,count=100_000)
    if uc.reg_read(UC_X86_REG_EIP)!=stop:
        raise RuntimeError('probe did not return within limits')
    matrix=list(struct.unpack('<16f',uc.mem_read(obj,64)))
    if not all(math.isfinite(v) for v in matrix):raise ValueError('nonfinite matrix')
    return dict(height=height,rotation=rotation,matrix=matrix,**state)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path); p.add_argument('--output',required=True,type=Path)
    a=p.parse_args(); output=a.output.resolve()
    if not output.is_relative_to(ROOT/'local') or output.exists():
        p.error('new private local/ output required')
    if not 0<a.source.stat().st_size<=64*1024*1024:raise ValueError('PE size bound')
    data=a.source.read_bytes()
    if hashlib.sha256(data).hexdigest()!=HASH:raise ValueError('private PE identity mismatch')
    cases=[(0.,0.),(90.,0.),(0.,90.),(144.9,163.6)]
    report=dict(source_sha256=HASH,entry=hex(ENTRY),emulator_version=unicorn.__version__,python=sys.version,
                scope='synthetic inputs, environment getter stub returns zero-mode; not runtime capture')
    try:
        report['cases']=[probe(data,*v) for v in cases]
        for case in report['cases']:
            h, r = math.radians(case['height']), math.radians(case['rotation'])
            ch, sh, cr, sr = math.cos(h), math.sin(h), math.cos(r), math.sin(r)
            expected = [cr,0.,sr,0., sr*sh,ch,-cr*sh,0., -sr*ch,sh,cr*ch,0., 0.,0.,0.,1.]
            error=max(abs(a-b) for a,b in zip(case['matrix'],expected))
            if error>2e-6:raise ValueError('Euler composition does not match isolated routine')
            case['ry_negative_rotation_rx_negative_height_max_error']=error
        report['status']='returned; synthetic Euler composition checked' 
    except Exception as e:
        report.update(status='failed',error=str(e))
        with output.open('x') as f:json.dump(report,f,indent=2)
        raise
    with output.open('x') as f:json.dump(report,f,indent=2,allow_nan=False)
    print(json.dumps(report))

if __name__=='__main__':main()
