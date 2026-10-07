"""Hash-guarded, bounded instruction evidence from the private analysis PE."""
import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'local/tools/python'))
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from inspect_installation import inspect_pe


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('ranges', nargs='+', help='hex-address:hex-length, at most 4096 bytes each')
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence output must stay private under local/')
    source = ROOT / 'local/compatibility/grid-unpacked.exe'
    raw = source.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514':
        raise ValueError('Analysis image identity mismatch')
    pe = inspect_pe(source)
    base = int(pe['image_base'], 16)
    result = {}
    for requested in args.ranges:
        address, length = (int(part, 16) for part in requested.split(':'))
        if not 0 < length <= 4096:
            raise ValueError('Instruction range outside limit')
        for section in pe['sections']:
            delta = address - base - section['rva']
            if 0 <= delta and delta + length <= section['raw_size']:
                offset = section['raw_offset'] + delta
                code = raw[offset:offset + length]
                result[hex(address)] = [
                    f'{ins.address:08x} {ins.mnemonic} {ins.op_str}'
                    for ins in Cs(CS_ARCH_X86, CS_MODE_32).disasm(code, address)
                ]
                break
        else:
            raise ValueError('Range is not entirely file backed')
    with output.open('x', encoding='utf-8') as handle:
        json.dump({'source_sha256': digest, 'instructions': result}, handle, indent=2)
    for address, instructions in result.items():
        print(address)
        print('\n'.join(instructions))


if __name__ == '__main__':
    main()
