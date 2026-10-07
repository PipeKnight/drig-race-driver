"""Compare native Rust exports to independent read-only PSSG surveys."""
import argparse
import collections
import hashlib
import json
import math
import struct
from pathlib import Path


def validate_mesh(survey_path, obj_path):
    report=json.loads(survey_path.read_text())
    source=Path(report['path']).read_bytes()
    assert hashlib.sha256(source).hexdigest()==report['sha256']
    nodes=report['nodes']
    children=collections.defaultdict(list)
    for i,n in enumerate(nodes):
        children[n['parent']].append(i)
    ids={n['attributes']['id']['string']:i for i,n in enumerate(nodes) if 'id' in n['attributes']}
    positions=[]; texcoords=[]; normals=[]; faces=[]
    for line in obj_path.read_text().splitlines():
        fields=line.split()
        if not fields: continue
        if fields[0]=='v':positions.append(tuple(map(float,fields[1:])))
        if fields[0]=='vt':texcoords.append(tuple(map(float,fields[1:])))
        if fields[0]=='vn':normals.append(tuple(map(float,fields[1:])))
        if fields[0]=='f':faces.append([tuple(int(v) if v else None for v in f.split('/')) for f in fields[1:]])
    expected_positions=[];expected_uvs=[];expected_normals=[];expected_faces=[]
    base=1;normal_base=1;uv_base=1;meshes=0;bad_normals=0
    for i,n in enumerate(nodes):
        if n['name']!='RENDERDATASOURCE':continue
        streams={}
        for j in children[i]:
            rs=nodes[j]
            if rs['name']!='RENDERSTREAM':continue
            a=rs['attributes']; bi=ids[a['dataBlock']['string'][1:]]
            block=nodes[bi]
            ds=[nodes[k] for k in children[bi] if nodes[k]['name']=='DATABLOCKSTREAM'][a['subStream']['u32be']]
            da=ds['attributes'];kind=da['renderType']['string']
            if kind not in ('Vertex','Normal','ST'):continue
            data=next(nodes[k] for k in children[bi] if nodes[k]['name']=='DATABLOCKDATA')
            count=block['attributes']['elementCount']['u32be']; components=2 if kind=='ST' else 3
            component_format='e' if da['dataType']['string']=='half2' else 'f'
            values=[struct.unpack_from('>'+str(components)+component_format,source,data['payload_offset']+da['offset']['u32be']+k*da['stride']['u32be']) for k in range(count)]
            if kind not in streams:streams[kind]=values
        normal=streams.get('Normal',[]);uv=streams.get('ST',[])
        bad=sum(any(not math.isfinite(v) for v in record) for record in normal)
        bad_normals+=bad
        ix=next(k for k in children[i] if nodes[k]['name']=='RENDERINDEXSOURCE')
        data=next(nodes[k] for k in children[ix] if nodes[k]['name']=='INDEXSOURCEDATA')
        indices=struct.unpack_from('>'+str(data['payload_size']//2)+'H',source,data['payload_offset'])
        for start in range(0,len(indices),3):
            face=[]
            for v in indices[start:start+3]:
                if uv and normal and not bad:face.append((base+v,uv_base+v,normal_base+v))
                elif uv:face.append((base+v,uv_base+v))
                elif normal and not bad:face.append((base+v,None,normal_base+v))
                else:face.append((base+v,))
            expected_faces.append(face)
        expected_positions.extend(streams['Vertex']);expected_uvs.extend(uv);uv_base+=len(uv)
        if not bad:expected_normals.extend(normal);normal_base+=len(normal)
        base+=len(streams['Vertex']);meshes+=1
    def f32bits(records):
        return b''.join(struct.pack('>f',v) for record in records for v in record)
    assert f32bits(positions)==f32bits(expected_positions), 'Position mismatch'
    assert f32bits(texcoords)==f32bits(expected_uvs), 'UV mismatch'
    assert f32bits(normals)==f32bits(expected_normals), 'Normal mismatch'
    assert faces==expected_faces,'Index/reference mismatch'
    return dict(meshes=meshes,vertices=len(positions),triangles=len(faces),source_nonfinite_normals=bad_normals,bit_exact_f32_roundtrip=True,obj_sha256=hashlib.sha256(obj_path.read_bytes()).hexdigest(),bounds=[[min(v[i] for v in positions) for i in range(3)],[max(v[i] for v in positions) for i in range(3)]])


def validate_textures(survey_path,directory):
    from PIL import Image
    report=json.loads(survey_path.read_text());source=Path(report['path']).read_bytes()
    assert hashlib.sha256(source).hexdigest()==report['sha256']
    nodes=report['nodes'];textures=[]
    for i,n in enumerate(nodes):
        if n['name']!='TEXTURE':continue
        a=n['attributes'];width=a['width']['u32be'];height=a['height']['u32be']
        size=width//4*(height//4)*(8 if a['texelFormat']['string']=='dxt1' else 16)
        block=next(j for j,x in enumerate(nodes) if x['parent']==i and x['name']=='TEXTUREIMAGEBLOCK')
        data=next(x for x in nodes if x['parent']==block and x['name']=='TEXTUREIMAGEBLOCKDATA')
        dds=directory/f'texture_{len(textures):03}.dds';raw=dds.read_bytes()
        assert len(raw)==128+size
        assert raw[128:]==source[data['payload_offset']:data['payload_offset']+size]
        with Image.open(dds) as image:
            image.load();assert image.size==(width,height)
            png=dds.with_suffix('.png')
            if png.exists():raise FileExistsError(png)
            image.save(png)
            decoded_hash=hashlib.sha256(image.tobytes()).hexdigest()
        textures.append(dict(id=a['id']['string'],width=width,height=height,format=a['texelFormat']['string'],base_bytes=size,raw_matches=True,pillow_decoded=True,decoded_pixels_sha256=decoded_hash,dds_sha256=hashlib.sha256(raw).hexdigest()))
    return textures


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--mesh-survey',type=Path,required=True)
    parser.add_argument('--obj',type=Path,required=True)
    parser.add_argument('--textures',type=Path,nargs=2,action='append',default=[])
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    private=(Path(__file__).resolve().parent.parent/'local').resolve()
    if not all(p.resolve().is_relative_to(private) for p in [args.mesh_survey,args.obj,args.output]+[p for pair in args.textures for p in pair]):
        raise SystemExit('Validation/export evidence must remain under local/')
    result={'mesh':validate_mesh(args.mesh_survey,args.obj),'textures':[]}
    for survey,directory in args.textures:result['textures'].extend(validate_textures(survey,directory))
    with args.output.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)
    print(json.dumps({'mesh':result['mesh'],'textures':len(result['textures']),'all_textures_decoded':all(t['pillow_decoded'] for t in result['textures'])}))
