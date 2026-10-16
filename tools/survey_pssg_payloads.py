"""Read-only exploratory PSSG survey. Raw values and metadata stay under local/."""
import argparse
import collections
import hashlib
import json
import math
import struct
from pathlib import Path

DATA_NAMES = {
    'BOUNDINGBOX', 'DATABLOCKDATA', 'DATABLOCKBUFFERED', 'INDEXSOURCEDATA',
    'INVERSEBINDMATRIX', 'MODIFIERNETWORKINSTANCEUNIQUEMODIFIERINPUT',
    'RENDERINTERFACEBOUNDBUFFERED', 'SHADERINPUT', 'TEXTUREIMAGEBLOCKDATA',
    'TRANSFORM', 'TEMPLATETRANSFORM', 'SHADERPROGRAMCODEBLOCK',
    'SHADERPROGRAMBINARYCODEBLOCK',
}


def survey(path):
    b = path.read_bytes()
    def u(p):
        return struct.unpack_from('>I', b, p)[0]
    def string(p):
        n = u(p)
        end = p + 4 + n
        if end > len(b):
            raise ValueError('String exceeds file')
        return b[p+4:end].decode('ascii'), end
    if b[:4] != b'PSSG' or u(4) != len(b)-8:
        raise ValueError('Unsupported framing')
    p = 16
    schema = {}
    attribute_names = {}
    for _ in range(u(12)):
        key = u(p)
        name, p = string(p+4)
        count = u(p)
        p += 4
        attrs = {}
        for _ in range(count):
            aid = u(p)
            aname, p = string(p+4)
            attrs[aid] = aname
            attribute_names[aid] = aname
        schema[key] = (name, attrs)
    nodes = []
    def node(p, limit, parent, depth):
        if depth > 128 or p+12 > limit:
            raise ValueError('Depth/header bounds')
        name, attrs = schema[u(p)]
        end = p+8+u(p+4)
        if end > limit:
            raise ValueError('Node exceeds parent')
        q = p+12
        aend = q+u(p+8)
        if aend > end:
            raise ValueError('Attributes exceed node')
        values = {}
        while q < aend:
            if q+8 > aend:
                raise ValueError('Truncated attribute')
            aid, n = u(q), u(q+4)
            q += 8
            if q+n > aend:
                raise ValueError('Attribute exceeds block')
            raw = b[q:q+n]
            value = {'hex':raw.hex()}
            if n == 4:
                value['u32be'] = struct.unpack('>I',raw)[0]
                preview = struct.unpack('>f',raw)[0]
                if math.isfinite(preview):
                    value['f32be'] = preview
            if n >= 4 and u(q) == n-4:
                value['string'] = raw[4:].decode('ascii',errors='backslashreplace')
            values[attribute_names[aid]] = value
            q += n
        index = len(nodes)
        data_node=name in DATA_NAMES or (name=='DATA' and parent is not None and nodes[parent]['name']=='PNSTRING')
        nodes.append(dict(name=name,offset=p,parent=parent,depth=depth,attributes=values,payload_offset=q,payload_size=end-q,data=data_node))
        if not data_node:
            while q < end:
                try:
                    q = node(q,end,index,depth+1)
                except (ValueError, KeyError, struct.error) as exc:
                    raise ValueError(f'{name} at 0x{p:x}, child 0x{q:x}: {exc}') from exc
        return end
    end = node(p,len(b),None,0)
    if end != len(b):
        raise ValueError('Trailing bytes')
    return dict(path=str(path),bytes=len(b),sha256=hashlib.sha256(b).hexdigest(),nodes=nodes,counts=dict(collections.Counter(n['name'] for n in nodes)))


if __name__ == '__main__':
    a=argparse.ArgumentParser()
    a.add_argument('source',type=Path)
    a.add_argument('--output',required=True,type=Path)
    args=a.parse_args()
    private=(Path(__file__).resolve().parent.parent/'local').resolve()
    if not args.output.resolve().is_relative_to(private):
        raise SystemExit('Survey evidence must remain under local/')
    report=survey(args.source)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    with args.output.open('x',encoding='utf-8') as f:
        json.dump(report,f,indent=2,allow_nan=False)
    print(json.dumps({'bytes':report['bytes'],'nodes':len(report['nodes']),'counts':report['counts']}))
