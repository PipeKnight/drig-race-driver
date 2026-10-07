"""Private bounded x86 instruction inspection before isolated numeric probes."""
import hashlib
import json
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent/'local/tools/python'))
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from inspect_installation import inspect_pe

if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent
    source = root/'local/compatibility/grid-unpacked.exe'
    raw = source.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    pe = inspect_pe(source)
    base = int(pe['image_base'],16)
    result = {}
    for address, length in [(0xadc290,0x400),(0x67c200,0x180)]:
        for section in pe['sections']:
            delta = address-base-section['rva']
            if 0 <= delta and delta+length <= section['raw_size']:
                code = raw[section['raw_offset']+delta:section['raw_offset']+delta+length]
                instructions = list(Cs(CS_ARCH_X86,CS_MODE_32).disasm(code,address))
                result[hex(address)] = [dict(address=hex(i.address),mnemonic=i.mnemonic,operands=i.op_str) for i in instructions]
                break
    with (root/'local/behavior/numeric-kernel-instructions.json').open('x') as f:
        json.dump(result,f,indent=2)
    for address,instructions in result.items():
        print(address)
        for instruction in instructions:
            if instruction['mnemonic'] in ('call','ret','cmp','test','je','jne','push','mov'):
                print(instruction)
            if instruction['mnemonic']=='ret':
                break
