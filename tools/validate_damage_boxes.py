"""Whole named-region loader versus source-built Rust; numeric adapter explicit."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess
import xml.etree.ElementTree as ET
from validate_binxml import decode
from validate_surface_mechanics import encode

GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
SOURCE = GAME / 'cars/lac/lac.ned'
SNAPSHOT = Path('local/behavior/damage-region-20261012-draft-01/source.state.bin')
IMAGE = Path('local/compatibility/grid-unpacked.exe')
NAMES = ['GEARBOX', 'DRIVESHAFT', 'TURBO', 'RADIATOR', 'EXHAUST']
KEYS = ['m_centre_x', 'm_centre_y', 'm_centre_z', 'm_size_x', 'm_size_y', 'm_size_z']


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def expected(root, offset, fill):
    result = bytearray([fill] * 240)
    parent = root.find('HANDLINGDAMAGE')
    for i, name in enumerate(NAMES):
        region = parent.find(name) if parent is not None else None
        if region is None:
            continue
        values = [f32(float(region.get(key))) for key in KEYS]
        center = [f32(values[j] + offset[j]) for j in range(3)] + [1.]
        struct.pack_into('<8f', result, i*48, *center, *values[3:], 1.)
        result[i*48+32] = 1
    return bytes(result)


def fixtures():
    rng = random.Random(200680)
    raw = SOURCE.read_bytes()
    _, root, _ = decode(raw)
    assert sha(SOURCE) == 'f71ce1224b10454f9fa76ee20af791e5f8e25eea160b6e1f76ff2bdffb2a26ba'
    source_offset = struct.unpack_from('<4f', SNAPSHOT.read_bytes(), 4+0x1630)
    for index in range(64):
        offset = source_offset if index == 0 else (0.,0.,0.,0.) if index == 1 else tuple(f32(rng.uniform(-10,10)) for _ in range(3)) + (0.,)
        for fill in [0, 0xa5]:
            yield 'source', raw, root, offset, fill, 'adapted'
    for index in range(64):
        tree = ET.Element('DAMAGE'); parent = ET.SubElement(tree, 'HANDLINGDAMAGE')
        for j, name in enumerate(NAMES):
            if (index >> j) & 1:
                continue
            attrs = {key: format(rng.uniform(-10,10) if k<3 else rng.uniform(0,10), '.17g') for k,key in enumerate(KEYS)}
            if index == 0:
                attrs = dict(zip(KEYS, ['-0.0','0','1.0000000596046448','0','0.5','1']))
            ET.SubElement(parent, name, attrs)
        raw = encode(tree)
        for fill in [0, 0xa5]:
            offset = tuple(f32(rng.uniform(-10,10)) for _ in range(3)) + (0.,)
            yield 'authored', raw, tree, offset, fill, 'adapted'
    for index in range(8):
        tree = ET.Element('DAMAGE')
        if index % 2:
            ET.SubElement(tree,'HANDLINGDAMAGE')
        for fill in [0,0xa5]:
            yield 'empty-original',encode(tree),tree,(0.,0.,0.,0.),fill,'original'


def run(command, success=True):
    p = subprocess.run([str(v) for v in command], capture_output=True, text=True, timeout=60)
    assert (p.returncode == 0) == success, (command,p.returncode,p.stdout,p.stderr)
    return dict(exit=p.returncode,stdout=p.stdout,stderr=p.stderr)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.output.resolve().is_relative_to(Path('local').resolve())
    a.output.mkdir(exist_ok=False)
    native=Path('local/behavior/damage-boxes-probe.exe')
    debug=Path('target/debug/grid-damage-boxes.exe');release=Path('target/release/grid-damage-boxes.exe')
    pinned={str(p):sha(p) for p in [native,debug,release,IMAGE,SOURCE,SNAPSHOT,Path(__file__)]}
    rows=[]
    for index,(label,raw,root,offset,fill,mode) in enumerate(fixtures()):
        xml=a.output/f'{index:03}.ned';xml.write_bytes(raw)
        request=a.output/f'{index:03}.input';request.write_bytes(b'DB01'+struct.pack('<4fI',*offset,fill))
        answer=expected(root,offset,fill);outputs={}
        for name,exe in [('original',native),('repeat',native),('debug',debug),('release',release)]:
            out=a.output/f'{index:03}-{name}.bin'
            e=run([exe,IMAGE,xml,request,out,mode] if name in ['original','repeat'] else [exe,xml,request,out])
            assert out.read_bytes()==answer,(index,label,name)
            if name in ['original','repeat']:
                assert f'numeric_adapters={int(mode=="adapted")} restored=1 error=0' in e['stdout']
            outputs[name]=dict(path=str(out),sha256=sha(out),execution=e)
        rows.append(dict(index=index,label=label,mode=mode,offset=offset,fill=fill,xml=str(xml),xml_sha256=sha(xml),request=str(request),request_sha256=sha(request),outputs=outputs))
        if index%64==63:print(json.dumps(dict(cases_complete=index+1)),flush=True)
    # Complete-attribute admission prevents the original getter's uninitialized
    # destination from entering a probe. Invalid schemas never reach execution.
    root=ET.Element('DAMAGE');parent=ET.SubElement(root,'HANDLINGDAMAGE')
    region=ET.SubElement(parent,NAMES[0],dict(zip(KEYS,['0','0','0','1','1','1'])))
    request=b'DB01'+struct.pack('<4fI',0.,0.,0.,0.,0)
    variants=[('short-xml',b'',request),('wrong-root',encode(ET.Element('OTHER')),request)]
    for label,key,value in [('missing',KEYS[0],None),('nan',KEYS[0],'NaN'),('overflow',KEYS[0],'1e100'),('negative-half',KEYS[3],'-1')]:
        t=copy.deepcopy(root);r=t[0][0]
        if value is None:del r.attrib[key]
        else:r.set(key,value)
        variants.append((label,encode(t),request))
    t=copy.deepcopy(root);t[0].append(copy.deepcopy(t[0][0]));variants.append(('duplicate-region',encode(t),request))
    t=copy.deepcopy(root);t.append(copy.deepcopy(t[0]));variants.append(('duplicate-parent',encode(t),request))
    t=ET.Element('DAMAGE');wrapper=ET.SubElement(t,'WRAPPER');wrapper.append(copy.deepcopy(root[0]));variants.append(('nested-parent',encode(t),request))
    raw=encode(root)
    for label,o,v in [('tag',0,0),('nan-offset',4,0x7fc00000),('offset-w',16,0x3f800000),('fill',20,256)]:
        b=bytearray(request);struct.pack_into('<I',b,o,v);variants.append((label,raw,b))
    variants += [('short-request',raw,request[:-1]),('long-request',raw,request+b'\x00')]
    guards=[]
    for label,raw,req in variants:
        xml=a.output/f'guard-{label}.ned';xml.write_bytes(raw)
        inp=a.output/f'guard-{label}.input';inp.write_bytes(req)
        for name,exe in [('native',native),('debug',debug),('release',release)]:
            out=a.output/f'guard-{label}-{name}.bin'
            run([exe,IMAGE,xml,inp,out,'adapted'] if name=='native' else [exe,xml,inp,out],False)
            assert not out.exists();guards.append(name+':'+label)
    out=a.output/'existing.bin';out.write_bytes(b'KEEP')
    row=rows[0]
    for name,exe in [('native',native),('debug',debug),('release',release)]:
        run([exe,IMAGE,row['xml'],row['request'],out,'adapted'] if name=='native' else [exe,row['xml'],row['request'],out],False)
        assert out.read_bytes()==b'KEEP';guards.append(name+':existing')
    assert all(sha(p)==v for p,v in pinned.items())
    report=dict(passed=True,cases=len(rows),bytes_per_run=len(rows)*240,inputs=pinned,rows=rows,guards=guards,
                adapted_cases=sum(r['mode']=='adapted' for r in rows),original_numeric_cases=0,
                limitation='numeric conversion explicitly adapted to host strtod; whole original region/accessor dispatch retained; no resource acquisition/outer factory proof')
    (a.output/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:report[k] for k in ['passed','cases','bytes_per_run','adapted_cases']}))


if __name__=='__main__':main()
