"""Read-only source/current-hash/complete-output audit of CQTC contact evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from validate_cqtc_decode import geometry, sha


def main():
    ap = argparse.ArgumentParser(); ap.add_argument('report', type=Path); ap.add_argument('output', type=Path); a = ap.parse_args()
    assert a.report.parts[0] == a.output.parts[0] == 'local'
    r = json.loads(a.report.read_text())
    assert r['passed'] and r['cases'] == len(r['rows']) == 1088 and len(r['guards']) == 30
    for path, digest in r['inputs'].items(): assert sha(path) == digest, path
    upstream = json.loads(Path('local/behavior/packed-mesh-contact-lacetti-20261012-final-01/validation.json').read_text())
    attachment = Path(upstream['attachment'])
    assert sha(attachment) == upstream['attachment_sha256']
    seed = (attachment.parent / f"{upstream['attachment_index']:03}-original.bin").read_bytes()
    assert hashlib.sha256(seed).hexdigest() == upstream['seed_sha256']
    files, bytes_total, contacts = 0, 0, 0
    sources = {}
    for row in r['rows']:
        source = Path(row['source'])
        if str(source) not in sources:
            raw = source.read_bytes(); decoded, counts = geometry(raw)
            sources[str(source)] = raw, decoded, counts
        raw, decoded, counts = sources[str(source)]
        assert sha(source) == row['source_sha256']
        request = Path(row['request']).read_bytes(); assert sha(row['request']) == row['request_sha256']
        nt = row['candidates']; ns = len(seed)
        assert struct.unpack_from('<6I', request) == (0x31304343, ns, len(raw), row['limit'], row['cache'], nt)
        assert struct.unpack_from('<I', request, 40)[0] == row['sheet']
        assert request[44:44 + ns] == seed and request[44 + ns:] == raw
        indices = struct.unpack_from('<3I', decoded, counts['vertices'] * 16 + row['triangle'] * 16)
        points = [struct.unpack_from('<4f', decoded, i * 16) for i in indices]
        center = [sum(p[j] for p in points) / 3 for j in range(3)]; center[1] += row['height']
        assert request[24:40] == struct.pack('<4f', *center, 1.)
        outputs = {}
        for kind, item in row['outputs'].items():
            p = Path(item['path']); assert sha(p) == item['sha256']; outputs[kind] = p.read_bytes(); files += 1
        original = outputs['original']; assert original == outputs['repeat']
        assert len(original) == row['complete_bytes'] == 0x44f8 + nt * 8
        # Independently select the complete cold cache, pair direction and arena.
        cache = original[0x21c8:0x4218]
        expected = cache[:32] + cache[44:52] + cache[68:8264] + original[0xfd0:0xfd4] + original[0x44f8:]
        expected += original[0xfb0:0xfb4] + original[0xf60:0xf70] + original[0xfe8:0x17e8]
        assert expected == outputs['debug'] == outputs['release'] and len(expected) == row['projected_bytes']
        assert struct.unpack_from('<I', original, 0x260)[0] == 10
        accepted, visited = struct.unpack_from('<2I', expected, 32)
        assert accepted == row['accepted'] <= nt and visited == row['visited'] <= nt
        to = struct.unpack_from('<I', raw, 44)[0]
        for start, used in [(0x44f8, accepted), (0x44f8 + nt * 4, visited)]:
            for i in range(used):
                pointer = struct.unpack_from('<I', original, start + i * 4)[0]
                offset = pointer - 0x100000 - to
                assert offset >= 0 and offset % 7 == 0 and offset // 7 < counts['triangles']
        count = struct.unpack_from('<I', original, 0xfb0)[0]
        assert count == row['contacts'] <= row['limit']; contacts += count
        for i in range(16):
            c = original[0xfe8 + i * 128:0x1068 + i * 128]
            assert struct.unpack_from('<I', c)[0] == 0x7200
            if i < count: assert c[0x73] == 1 and c[0x71] <= 1 and struct.unpack_from('<I', c, 0x2c)[0] == 0xc000
        files += 1; bytes_total += len(original)
    for label in r['guards']:
        kind, name = label.split(':')
        if name == 'existing': assert (a.report.parent / 'existing.bin').read_bytes() == b'KEEP'
        else:
            assert (a.report.parent / f'guard-{name}.input').exists()
            assert not (a.report.parent / f'guard-{name}-{kind}.bin').exists()
    assert contacts == r['contacts'] == 4212 and bytes_total == r['complete_bytes_per_native_run']
    result = dict(passed=True, report=str(a.report), report_sha256=sha(a.report), current_hashes=len(r['inputs']), source_files=len(sources), complete_files=files, complete_native_bytes_per_run=bytes_total, contacts=contacts, guards=len(r['guards']), script_sha256=sha(Path(__file__)))
    with a.output.open('x') as f: json.dump(result, f, indent=2)
    print(json.dumps(result))


if __name__ == '__main__': main()
