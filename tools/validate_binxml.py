"""Validate selected native binary XML records against an independent decoder."""
import argparse
import hashlib
import json
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path


def decode(data):
    def u(p):
        return struct.unpack_from('<I', data, p)[0]
    assert u(0) == 0x7252221a and u(4) == len(data)-8 and u(8) == 0x72522217
    p = 16
    sections = []
    for magic in (0x7252221d, 0x7252221e, 0x7252221b, 0x7252221c):
        assert u(p) == magic
        size = u(p+4)
        sections.append(data[p+8:p+8+size])
        assert len(sections[-1]) == size
        p += 8+size
    assert p == len(data) and u(12) == len(sections[0])+len(sections[1])+16
    blob, offsets, raw_elements, raw_attributes = sections
    strings = []
    for (offset,) in struct.iter_unpack('<I', offsets):
        end = blob.index(0, offset)
        strings.append(blob[offset:end].decode('utf-8'))
    elements = list(struct.iter_unpack('<6I', raw_elements))
    attributes = list(struct.iter_unpack('<2I', raw_attributes))
    parents = [None]*len(elements)
    for i, (_, _, _, _, nc, start) in enumerate(elements):
        for child in range(start, start+nc):
            assert child != 0 and parents[child] is None
            parents[child] = i
    rows = []
    nodes = []
    for i, (name, value, na, ast, nc, cst) in enumerate(elements):
        rows.append(['E', str(i), strings[name].encode().hex(), strings[value].encode().hex() if value else '',
                     str(parents[i]) if parents[i] is not None else '', str(cst), str(cst+nc)])
        node = ET.Element(strings[name])
        node.text = strings[value] if value else None
        for key, val in attributes[ast:ast+na]:
            rows.append(['A', str(i), strings[key].encode().hex(), strings[val].encode().hex()])
            node.set(strings[key], strings[val])
        nodes.append(node)
    for i, element in enumerate(elements):
        for child in range(element[5], element[5]+element[4]):
            nodes[i].append(nodes[child])
    return rows, nodes[0], dict(strings=len(strings), elements=len(elements), attributes=len(attributes))


def entries(data):
    for i in range(struct.unpack_from('<I', data, 8)[0]):
        w = struct.unpack_from('<8I', data, 32+i*32)
        name = data[w[0]:data.index(0, w[0])].decode('ascii')
        yield name, data[w[2]:w[2]+w[3]], w[1] == w[3]


if __name__ == '__main__':
    a = argparse.ArgumentParser()
    a.add_argument('game_root', type=Path)
    a.add_argument('--output-dir', required=True, type=Path)
    args = a.parse_args()
    workspace = Path(__file__).resolve().parent.parent
    private = (workspace/'local').resolve()
    out = args.output_dir.resolve()
    if not out.is_relative_to(private):
        raise SystemExit('Evidence must stay beneath local/')
    out.mkdir(parents=True, exist_ok=False)
    baseline = json.loads((private/'reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    jobs = []
    skipped = []
    for r in baseline['resources']:
        p = args.game_root/r['relative_path']
        data = p.read_bytes()
        if data.startswith(bytes.fromhex('1a225272')):
            jobs.append((r['relative_path'], p, data))
        elif data.startswith(b'JPAK'):
            for name, content, raw in entries(data):
                if content.startswith(bytes.fromhex('1a225272')):
                    if not raw:
                        skipped.append(f'{p.name}#{name}: compressed')
                        continue
                    derivative = out/f'entry-{len(jobs):03}.binxml'
                    derivative.write_bytes(content)
                    jobs.append((r['relative_path']+'#'+name, derivative, content))
    binary = workspace/'target/debug/grid-binxml.exe'
    results = []
    for i, (source, path, data) in enumerate(jobs):
        report = out/f'fields-{i:03}.tsv'
        process = subprocess.run([str(binary), str(path), str(report)], capture_output=True, text=True)
        result = dict(source=source, sha256=hashlib.sha256(data).hexdigest(), native_exit=process.returncode,
                      stdout=process.stdout, stderr=process.stderr, exact=False)
        if process.returncode == 0:
            expected, root, counts = decode(data)
            actual = [line.split('\t') for line in report.read_text().splitlines()]
            result.update(counts, root=root.tag, exact=expected == actual, report=str(report))
            tree = ET.ElementTree(root)
            ET.indent(tree)
            tree.write(out/f'decoded-{i:03}.xml', encoding='utf-8', xml_declaration=True)
            if not result['exact']:
                result['mismatches'] = [[e, a] for e, a in zip(expected, actual) if e != a][:20]
        results.append(result)
    summary = dict(documents=len(results), all_exact=all(r['exact'] for r in results), skipped=skipped,
                   native_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), results=results)
    (out/'validation.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
    print(json.dumps(dict(documents=summary['documents'], all_exact=summary['all_exact'], skipped=skipped,
                          failed=[r['source'] for r in results if not r['exact']])))
    if not summary['all_exact']:
        raise SystemExit('Some selected binary XML documents failed native/independent validation')
