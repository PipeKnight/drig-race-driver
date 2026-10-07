"""Check native marker rectangles against original hierarchy and bounds bytes."""
import json
import math
import struct
from pathlib import Path
from survey_pssg_payloads import survey


def f(v):
    return struct.unpack('<f', struct.pack('<f', v))[0]


def word(v):
    return f'{struct.unpack("<I", struct.pack("<f", v))[0]:08x}'


def compose(a, b):
    result = []
    for i in range(16):
        value = 0.0
        for k in range(4):
            value = f(value+f(a[i//4*4+k]*b[k*4+i%4]))
        result.append(value)
    return result


def point(m, p):
    return [f(f(f(f(p[0]*m[i])+f(p[1]*m[4+i]))+f(p[2]*m[8+i]))+m[12+i]) for i in range(3)]


if __name__ == '__main__':
    root = Path(__file__).resolve().parent.parent
    source = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid\tracks\europe\donington\route_0\grids.pssg')
    report = survey(source)
    raw = source.read_bytes()
    nodes = report['nodes']
    native = {bytes.fromhex(row[1]).decode():row[2:] for line in (root/'local/behavior/grid-markers-native.tsv').read_text().splitlines() if (row:=line.split('\t'))[0]=='M'}

    def data(index):
        n = nodes[index]
        b = raw[n['payload_offset']:n['payload_offset']+n['payload_size']]
        return list(struct.unpack('>'+str(len(b)//4)+'f', b))

    results = []
    for i, n in enumerate(nodes):
        name = n['attributes'].get('id', {}).get('string', '')
        if n['name']!='NODE' or not name.startswith('slot_'):
            continue
        matrix = [1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]
        a = i
        while a is not None:
            transforms = [j for j, child in enumerate(nodes) if child['parent']==a and child['name']=='TRANSFORM']
            if transforms:
                assert len(transforms)==1
                matrix = compose(matrix, data(transforms[0]))
            a = nodes[a]['parent']
        boxes = [j for j, child in enumerate(nodes) if child['parent']==i and child['name']=='BOUNDINGBOX']
        assert len(boxes)==1
        lo_x,lo_y,lo_z,hi_x,hi_y,hi_z = data(boxes[0])
        y = f(lo_y+f(f(hi_y-lo_y)*0.5))
        corners = [point(matrix, p) for p in [[lo_x,y,lo_z],[hi_x,y,lo_z],[lo_x,y,hi_z]]]
        center = [f(corners[2][axis]+f(0.5*f(f(corners[0][axis]-corners[2][axis])+f(corners[1][axis]-corners[0][axis])))) for axis in range(3)]
        direction = [f(corners[0][axis]-corners[2][axis]) for axis in range(3)]
        length2 = 0.
        for d in direction:
            length2 = f(length2+f(d*d))
        length = f(math.sqrt(length2))
        expected = [word(v) for v in matrix+[v for p in corners for v in p]+center+[f(v/length) for v in direction]]
        actual = native.pop(name)
        result = dict(id=name, exact=expected==actual, raw_origin=matrix[12:15], center=center, forward=[f(v/length) for v in direction])
        results.append(result)
    assert not native
    output = dict(source_sha256=report['sha256'], markers=len(results), exact=all(r['exact'] for r in results), results=results)
    with (root/'local/behavior/grid-markers-validation.json').open('x') as out:
        json.dump(output, out, indent=2)
    print(json.dumps({k:v for k,v in output.items() if k!='results'}))
    assert output['exact']
