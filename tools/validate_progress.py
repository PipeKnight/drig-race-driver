"""Compare typed Rust progress values with independently reconstructed binary XML."""
import hashlib
import json
import struct
from pathlib import Path
from validate_binxml import decode


def native_bits(text):
    return f'{struct.unpack("<I", struct.pack("<f", float(text)))[0]:08x}'


def vector(node):
    assert node.attrib['format'] == 'float3'
    return [native_bits(value) for value in node.text.split()]


if __name__ == '__main__':
    workspace = Path(__file__).resolve().parent.parent
    source = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid\tracks\europe\donington\route_0\progress_track.xml')
    private = workspace/'local/binxml'
    raw = source.read_bytes()
    _, root, _ = decode(raw)
    report = private/'donington-progress-native.tsv'
    total = root.find('track').attrib['total_distance']
    expected = [['D', native_bits(total)]]
    gates = root.find('gates')
    points = root.find('points')
    for gate in gates:
        expected.append(['G', gate.attrib['id'], native_bits(gate.attrib['distance']),
                         *vector(gate.find('left')), *vector(gate.find('right'))])
    for point in points:
        expected.append(['P', point.attrib['id'], native_bits(point.attrib['distance']),
                         *vector(point.find('position'))])
    for route in root.find('routes'):
        for split in route:
            expected.append(['S', route.attrib['id'], split.attrib['id'],
                             split.attrib['type'].capitalize(), split.attrib['gate']])
    actual = [line.split('\t') for line in report.read_text().splitlines()]
    result = dict(source=str(source), sha256=hashlib.sha256(raw).hexdigest(), exact=expected == actual,
                  total_distance=total, gates=len(gates), points=len(points), rows=len(expected))
    with (private/'donington-progress-validation.json').open('x') as f:
        json.dump(result, f, indent=2)
    print(json.dumps(result))
    assert result['exact']
