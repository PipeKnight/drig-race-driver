"""Alongside squeeze capture for the original (FIRST_CORNER_LIVE section 13, rows 30/31).

    python -I tools/alongside_capture.py commands local/OUT.txt
    python -I tools/alongside_capture.py decode LOG local/OUT.tsv

`commands` writes the x32dbg command lines for a read-only, non-stopping hardware
breakpoint at the squeeze drive 0x799630 (thiscall: ECX behaviour, [esp+4] AI
context, [esp+8] alongside test object). Each call logs one `ALG` line of hex
words; the breakpoint is meant to be silent and fast-resuming.
Run it only with the owner's go, attached to the research copy, alongside
tools/capture_grid_field.py for the cars' positions. Tried live in F662: the
commands are accepted, but headless x32dbg does not honour FastResume and
pauses the game on the first hit, so a driver must clear the breakpoint and
detach at once (FIRST_CORNER_LIVE §13).

`decode` turns the ALG lines of a debugger log into a TSV of floats (the words
documented below), keeping the call order.

Fields per call (docs/AI_OVERTAKING_RESEARCH.md section 8.2, decompile 0x799630):
- behaviour (ECX), ctx ([esp+4]), test ([esp+8]), view ([ctx+0x18]), request ([ctx+4]);
- view +0xE0 planned offset at the look-ahead (the input `cur`), +0x1A0 car-link
  planned offset, +0x220/+0x224 racing rooms at the car, +0x230/+0x234 at the
  look-ahead, +0x170/+0x178/+0x17C the car point the gaps are measured from;
- test +0x0C/+0x10 gaps (positive/negative side), +0x14/+0x18 slot ids,
  +0x24/+0x25 strict-overlap flags;
- request +0x98 desired offset before this call (the previous output);
- asTargetBuffer [[ctx+0x10]+0x464]; minSpace/maxSpace [[ctx+0x28]+0x54]+8, +0x58]+8.
"""
import struct
import sys
from pathlib import Path

ADDRESS = 0x799630
CTX = '[esp+4]'
VIEW = f'[{CTX}+18]'
TEST = '[esp+8]'
REQUEST = f'[{CTX}+4]'
# (name, x32dbg expression, kind): kind f = float word, p = pointer, b = byte.
FIELDS = [
    ('behaviour', 'ecx', 'p'),
    ('ctx', CTX, 'p'),
    ('test', TEST, 'p'),
    ('view', VIEW, 'p'),
    ('offset_e0', f'dword:[{VIEW}+E0]', 'f'),
    ('link_1a0', f'dword:[{VIEW}+1A0]', 'f'),
    ('room_car_neg_220', f'dword:[{VIEW}+220]', 'f'),
    ('room_car_pos_224', f'dword:[{VIEW}+224]', 'f'),
    ('room_ahead_neg_230', f'dword:[{VIEW}+230]', 'f'),
    ('room_ahead_pos_234', f'dword:[{VIEW}+234]', 'f'),
    ('x_170', f'dword:[{VIEW}+170]', 'f'),
    ('y_178', f'dword:[{VIEW}+178]', 'f'),
    ('z_17c', f'dword:[{VIEW}+17C]', 'f'),
    ('gap_pos_0c', f'dword:[{TEST}+C]', 'f'),
    ('gap_neg_10', f'dword:[{TEST}+10]', 'f'),
    ('slot_14', f'dword:[{TEST}+14]', 'p'),
    ('slot_18', f'dword:[{TEST}+18]', 'p'),
    ('flag_24', f'byte:[{TEST}+24]', 'b'),
    ('flag_25', f'byte:[{TEST}+25]', 'b'),
    ('desired_98_before', f'dword:[{REQUEST}+98]', 'f'),
    ('target_buffer', f'dword:[[{CTX}+10]+464]', 'f'),
    ('min_space', f'dword:[[[{CTX}+28]+54]+8]', 'f'),
    ('max_space', f'dword:[[[{CTX}+28]+58]+8]', 'f'),
]


def commands():
    text = 'ALG ' + ' '.join('{x:' + expression + '}' for _, expression, _ in FIELDS)
    a = f'{ADDRESS:X}'
    return [
        f'bph {a},x,1',
        f'bphwlog {a},"{text}"',
        f'SetHardwareBreakpointSilent {a},1',
        f'SetHardwareBreakpointFastResume {a},1',
    ]


def decode_line(line):
    """The fields of one ALG log line, or None for any other line."""
    words = line.split()
    if not words or words[0] != 'ALG' or len(words) != len(FIELDS) + 1:
        return None
    out = []
    for (_, _, kind), word in zip(FIELDS, words[1:]):
        value = int(word, 16)
        if kind == 'f':
            out.append(f'{struct.unpack("<f", struct.pack("<I", value & 0xFFFFFFFF))[0]:.4f}')
        elif kind == 'b':
            out.append(str(value & 0xFF))
        else:
            out.append(f'{value:08x}')
    return out


def private(path):
    path = Path(path)
    if 'local' not in path.resolve().parts or path.exists():
        sys.exit('output must be a new file under local/')
    return path


def main():
    if len(sys.argv) == 3 and sys.argv[1] == 'commands':
        private(sys.argv[2]).write_text('\n'.join(commands()) + '\n', encoding='utf-8')
    elif len(sys.argv) == 4 and sys.argv[1] == 'decode':
        out = private(sys.argv[3])
        rows = [r for r in map(decode_line, Path(sys.argv[2]).read_text(errors='replace').splitlines()) if r]
        with out.open('x', encoding='utf-8') as f:
            f.write('\t'.join(['call'] + [name for name, _, _ in FIELDS]) + '\n')
            for i, r in enumerate(rows):
                f.write('\t'.join([str(i)] + r) + '\n')
        print(f'{len(rows)} calls -> {out}')
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
