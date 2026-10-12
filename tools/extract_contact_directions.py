"""Prepare private collision sampling data from the owner's identified Steam PE.

This bounded static reader interprets only scalar/vector data moves in the
private analyzed initializer; scalar data comes from the untouched Steam PE.
It never maps or executes original instructions. This preparation still needs
the identified private analysis image; it is not an engine startup loader.
Neither the resulting table nor the source executable belongs in Git.
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT/'local/tools/python'))
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86_const import X86_OP_REG, X86_OP_MEM, X86_REG_ESP
from inspect_installation import inspect_pe

SOURCE = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid\grid.exe')
SOURCE_SHA = 'dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f'


def extract(source=SOURCE):
    raw = source.read_bytes(); assert hashlib.sha256(raw).hexdigest() == SOURCE_SHA
    pe = inspect_pe(source); base = int(pe['image_base'], 16)
    def read(address, size):
        section = next(s for s in pe['sections'] if 0 <= address-base-s['rva'] <= s['raw_size']-size)
        offset = section['raw_offset']+address-base-section['rva']
        return raw[offset:offset+size]
    decoder = Cs(CS_ARCH_X86, CS_MODE_32); decoder.detail = True
    image = ROOT/'local/compatibility/grid-unpacked.exe'; code_raw = image.read_bytes()
    code_sha = hashlib.sha256(code_raw).hexdigest()
    assert code_sha == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    code_pe = inspect_pe(image); code_base = int(code_pe['image_base'], 16)
    code_section = next(s for s in code_pe['sections'] if 0 <= 0xc7dae0-code_base-s['rva'] <= s['raw_size']-4096)
    code_offset = code_section['raw_offset']+0xc7dae0-code_base-code_section['rva']
    regs, stack, output, reads = {}, {}, {}, set()
    count = 0; returned = False
    prologue = {('push', 'ebp'), ('mov', 'ebp, esp'), ('and', 'esp, 0xfffffff0'),
                ('sub', 'esp, 0x10'), ('mov', 'esp, ebp'), ('pop', 'ebp')}
    for ins in decoder.disasm(code_raw[code_offset:code_offset+4096], 0xc7dae0):
        count += 1
        if ins.mnemonic == 'ret':
            returned = True; break
        if (ins.mnemonic, ins.op_str) in prologue: continue
        ops = ins.operands
        if ins.mnemonic == 'xorps' and len(ops) == 2 and ops[0].type == ops[1].type == X86_OP_REG and ops[0].reg == ops[1].reg:
            regs[ops[0].reg] = bytes(16); continue
        assert ins.mnemonic in ['movss', 'movaps'] and len(ops) == 2, (hex(ins.address), ins.mnemonic, ins.op_str)
        dst, src = ops; size = 4 if ins.mnemonic == 'movss' else 16
        if src.type == X86_OP_REG:
            value = regs[src.reg][:size]
        else:
            assert src.type == X86_OP_MEM and src.mem.index == 0
            if src.mem.base == X86_REG_ESP:
                assert src.mem.disp in stack; value = stack[src.mem.disp]
                if size == 16: value = b''.join(stack[i] for i in range(src.mem.disp, src.mem.disp+16, 4))
            else:
                assert src.mem.base == 0 and size == 4
                value = read(src.mem.disp, size); reads.add(src.mem.disp)
        if dst.type == X86_OP_REG:
            # A memory MOVSS clears the upper lanes; register MOVSS is absent in
            # this admitted initializer and is rejected rather than guessed.
            assert not (size == 4 and src.type == X86_OP_REG)
            regs[dst.reg] = value+bytes(16-size)
        else:
            assert dst.type == X86_OP_MEM and dst.mem.index == 0
            if dst.mem.base == X86_REG_ESP:
                assert size == 4 and dst.mem.disp in [0, 4, 8, 12]
                stack[dst.mem.disp] = value
            else:
                assert dst.mem.base == 0 and size == 16 and 0xf5e1b0 <= dst.mem.disp < 0xf5e470
                assert (dst.mem.disp-0xf5e1b0)%16 == 0 and dst.mem.disp not in output
                output[dst.mem.disp] = value
    assert returned and len(output) == 44
    table = b''.join(output[i] for i in range(0xf5e1b0, 0xf5e470, 16))
    for i in range(44):
        x, y, z, w = struct.unpack_from('<4f', table, i*16)
        assert w == 0. and abs(x*x+y*y+z*z-1.) < .001
    return b'CD01'+struct.pack('<I', 44)+table, dict(source_path=str(source), source_sha256=SOURCE_SHA,
        initializer='00c7dae0', initializer_image_sha256=code_sha,
        decoded_instructions=count, directions=44, scalar_source_words=len(reads),
        execution=False, scope='bounded data moves only; no original executable instructions run')


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path); a = p.parse_args()
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    table, report = extract(); target = out/'directions.bin'; target.write_bytes(table)
    report.update(passed=True, output_sha256=hashlib.sha256(table).hexdigest(), bytes=len(table),
                  script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(report))


if __name__ == '__main__': main()
