"""Summarize bounded debugger entry/return evidence without printing raw captures."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def private(path):
    path = path.resolve()
    if not path.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private evidence required')
    return path


def blob(row, field, length):
    value = bytes.fromhex(row[field])
    if len(value) != length:
        raise ValueError(f'Unexpected {field} length')
    return value


def floats(value):
    values = struct.unpack(f'<{len(value) // 4}f', value)
    if not all(math.isfinite(x) for x in values):
        raise ValueError('Nonfinite selected float field')
    return values


def summarize(case):
    before = blob(case, 'descriptor_before_hex', 0xa500)
    after = blob(case, 'descriptor_after_hex', 0xa500)
    dims = struct.unpack_from('<3i', before, 0xa414)
    count = math.prod(dims)
    if not all(2 <= n <= 16 for n in dims) or not 1 <= count <= 400:
        raise ValueError('Invalid active dimensions')
    if case['return_registers'][:2] != [int(case['return_address'], 16), case['entry_registers'][1] + 4]:
        raise ValueError('Return ABI mismatch')
    contacts = []
    for contact in case['contacts']:
        a = blob(contact, 'before_hex', 0x80)
        b = blob(contact, 'after_hex', 0x80)
        contacts.append(dict(eligible_byte73=a[0x73], counterpart=contact['counterpart'],
                             field_3c=floats(a[0x3c:0x40])[0],
                             field_38=floats(a[0x38:0x3c])[0],
                             field_40=floats(a[0x40:0x44])[0],
                             field_44=floats(a[0x44:0x48])[0],
                             point50=floats(a[0x50:0x60]), direction60=floats(a[0x60:0x70]),
                             changed_word_offsets=[hex(i) for i in range(0, 0x80, 4) if a[i:i+4] != b[i:i+4]]))
    if len(contacts) != case['contact_count'] or not 1 <= len(contacts) <= 80:
        raise ValueError('Contact count mismatch')
    flag_changes = [i for i in range(count) if before[0xa280+i] != after[0xa280+i]]
    return dict(index=case['index'], body=case['body'], linked=case['linked'], dimensions=dims,
                contact_count=len(contacts), return_ax=case['return_registers'][2], contacts=contacts,
                scalar=floats(bytes.fromhex(case['scalar_word'])[::-1])[0],
                changed_descriptor_bytes=sum(x != y for x, y in zip(before, after)),
                changed_active_position_words=sum(before[i:i+4] != after[i:i+4]
                    for node in range(count) for i in range(0x2580+node*80, 0x2580+node*80+16, 4)),
                changed_active_flag_indices=flag_changes,
                active_flags_after=[after[0xa280+i] for i in flag_changes],
                counter_before=struct.unpack_from('<i', before, 0xa450)[0],
                counter_after=struct.unpack_from('<i', after, 0xa450)[0],
                dirty_flags_before=list(before[0xa4f0:0xa4f5]), dirty_flags_after=list(after[0xa4f0:0xa4f5]),
                corners_before=floats(before[0xa4b4:0xa4c4]), corners_after=floats(after[0xa4b4:0xa4c4]),
                response_before=floats(blob(case, 'response_before_hex', len(contacts)*4)) if int(case['response'],16) else None,
                response_after=floats(blob(case, 'response_after_hex', len(contacts)*4)) if int(case['response'],16) else None)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    source = private(args.capture)
    if source.stat().st_size > 8*1024*1024:
        raise ValueError('Capture exceeds bound')
    data = source.read_bytes()
    capture = json.loads(data)
    if capture['boundary'] != 'impact' or not capture['completed'] or not capture['code_exact'] or capture['debugger_exit'] != 0:
        raise ValueError('Incomplete impact capture')
    if not 1 <= len(capture['cases']) <= 32:
        raise ValueError('Case count outside scope')
    report = dict(capture_sha256=hashlib.sha256(data).hexdigest(), session=capture['session'],
                  debugger_sha256=capture['debugger_sha256'], hardware=capture['hardware_breakpoints'],
                  cases=[summarize(case) for case in capture['cases']],
                  limits='Actual entry/return records; debugger perturbs timing. External bounded macros are logged separately. No full simulation or replay equivalence claimed.')
    with private(args.output).open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
    print(json.dumps({k:report[k] for k in ('capture_sha256','hardware')}))
    print(json.dumps([dict(index=c['index'], contacts=c['contact_count'], changed_bytes=c['changed_descriptor_bytes'],
                           position_words=c['changed_active_position_words'], flag_changes=len(c['changed_active_flag_indices']),
                           response=c['response_after']) for c in report['cases']]))


if __name__ == '__main__':
    main()
