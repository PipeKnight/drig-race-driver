"""Whole original mesh construction, source topology and complete owned bytes."""
import argparse,csv,hashlib,itertools,json,math,random,struct,subprocess
from pathlib import Path
from validate_binxml import decode
from validate_wheel_geometry import layout

ROOT=Path(__file__).resolve().parent.parent
IMAGE=ROOT/'local/compatibility/grid-unpacked.exe'
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def f32(x): return struct.unpack('<f',struct.pack('<f',x))[0]
def adjusted(points,parameters):
    if parameters is None:return points
    offset=parameters[:4];bottom,roof=parameters[4:];floor=min(p[1] for p in points)
    center=f32(offset[1]+f32(f32(roof+bottom)*.5))
    return [[f32(f32(offset[0]+p[0])-0.),f32(f32(offset[1]+max(p[1],floor))-center),f32(f32(offset[2]+p[2])-0.),1.] for p in points]

def request(points,faces,fill,parameters=None,shape=None):
    header=(b'CM03' if shape is not None else (b'CM01' if parameters is None else b'CM02'))+struct.pack('<III',len(points),len(faces),fill)
    if parameters is not None:header+=struct.pack('<6f',*parameters)
    if shape is not None:header+=struct.pack('<IfII',*shape)
    return header+b''.join(struct.pack('<4f',*p) for p in points)+b''.join(struct.pack('<3I',*t) for t in faces)
def run(exe,args):
    r=subprocess.run([str(exe),*map(str,args)],capture_output=True,text=True,cwd=ROOT,timeout=30)
    return dict(exit=r.returncode,stdout=r.stdout,stderr=r.stderr)
def check(data,points,faces,fill,shape=None):
    if shape is not None:
        expected=bytearray([fill]*0x250);span,scalar,layer,flag=shape
        expected[0x19]=layer;expected[0xc0]=flag
        struct.pack_into('<f',expected,0x220,scalar);struct.pack_into('<II',expected,0x240,0x250,(span+15)&~15)
        assert data[:0x250]==expected
        data=data[0x250:]
    p=len(points);t=len(faces);a16=lambda n:(n+15)&~15
    vertices=0x60;aux=vertices+32*p;planes=aux+16*p;neighbors=planes+32*t;incidents=neighbors+a16(40*p)
    u=lambda o:struct.unpack_from('<I',data,o)[0]
    assert len(data)==incidents+a16(12*t)
    assert [u(o) for o in [0,4,8,12,0x50,0x54,0x58]]==[p,t,1,1,0x3f800000,vertices,planes]
    assert struct.unpack_from('<16f',data,0x10)==tuple(1. if i%5==0 else 0. for i in range(16))
    extrema=[min(v[j] for v in points) for j in range(3)]+[max(v[j] for v in points) for j in range(3)]
    assert struct.unpack_from('<f',data,0x5c)[0]==min(map(abs,extrema))
    edge_cursor=face_cursor=0
    for i,point in enumerate(points):
        v=vertices+i*32;a=aux+i*16;adj=[];inc=[]
        for j,face in enumerate(faces):
            if i not in face:continue
            inc.append(j)
            for k in face:
                if k!=i and k not in adj:adj.append(k)
        assert data[v:v+16]==struct.pack('<4f',*point)
        assert [u(v+o) for o in [0x10,0x14,0x18]]==[a,len(inc),incidents+face_cursor*4]
        assert [u(a+o) for o in [0,4]]==[len(adj),neighbors+edge_cursor*4]
        assert [u(neighbors+(edge_cursor+j)*4) for j in range(len(adj))]==[vertices+j*32 for j in adj]
        assert [u(incidents+(face_cursor+j)*4) for j in range(len(inc))]==[planes+j*32 for j in inc]
        assert data[v+28:v+32]==bytes([fill])*4 and data[a+8:a+16]==bytes([fill])*8
        edge_cursor+=len(adj);face_cursor+=len(inc)
    assert face_cursor==3*t and edge_cursor<=p*10
    assert data[neighbors+edge_cursor*4:incidents]==bytes([fill])*(incidents-neighbors-edge_cursor*4)
    assert data[incidents+face_cursor*4:]==bytes([fill])*(len(data)-incidents-face_cursor*4)
    for i,face in enumerate(faces):
        o=planes+i*32;n=struct.unpack_from('<4f',data,o)
        assert all(math.isfinite(x) for x in n) and abs(sum(x*x for x in n)-1)<1e-6 and n[3]==0
        assert struct.unpack_from('<3I',data,o+16)==tuple(face) and data[o+28:o+32]==bytes([fill])*4
        a,b,c=[points[j] for j in face];ab=[b[j]-a[j] for j in range(3)];ac=[c[j]-a[j] for j in range(3)]
        cross=[ab[1]*ac[2]-ab[2]*ac[1],ac[0]*ab[2]-ac[2]*ab[0],ac[1]*ab[0]-ac[0]*ab[1]]
        if shape is None:assert sum(x*y for x,y in zip(n,cross))<0 # Initial builder stores opposite winding.
        else:
            center=[sum(p[j] for p in points)/len(points) for j in range(3)]
            assert sum(n[j]*(a[j]-center[j]) for j in range(3))>=-1e-6 # Shape planes point away from average vertex.
            assert abs(sum(x*y for x,y in zip(n,cross)))>0
    return dict(directed_edges=edge_cursor,incidences=face_cursor)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',required=True,choices=['debug','release']);parser.add_argument('--source-adjustment',action='store_true');parser.add_argument('--shape',action='store_true');args=parser.parse_args()
    if args.shape:args.source_adjustment=True
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    game=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid');native=ROOT/('local/behavior/mesh-shape-probe.exe' if args.shape else ('local/behavior/source-collision-mesh-probe.exe' if args.source_adjustment else 'local/behavior/collision-mesh-probe.exe'));rust=ROOT/f'target/{args.profile}'/('grid-mesh-shape.exe' if args.shape else 'grid-collision-mesh.exe')
    source_cases=[];skipped=[]
    for csvpath in sorted((game/'cars').rglob('*.csv')):
        rows=[row for row in csv.reader(csvpath.read_text().splitlines()) if row]
        if not rows or any(len(row)<360 or row[2]!='Standard' for row in rows):continue
        ned=csvpath.with_suffix('.ned')
        if not ned.is_file():skipped.append(str(csvpath.relative_to(game)));continue
        tree=decode(ned.read_bytes())[1];collisions=list(tree.iter('COLLISION'));assert len(collisions)==1
        geometry=[e for e in collisions[0] if e.tag=='GEOMETRY' and e.get('id')=='collision_main'];assert len(geometry)==1
        points=[[*map(f32,map(float,e.get('pos').split())),1.] for e in geometry[0] if e.tag=='POINT']
        faces=[list(map(int,e.get('points').split())) for e in geometry[0] if e.tag=='TRIANGLE']
        metadata=dict(kind='source',source=str(ned.relative_to(game)),source_sha256=digest(ned))
        parameters=None
        if args.source_adjustment:
            wheels=list(tree.iter('WHEELS'));assert len(wheels)==1
            parameters=layout(wheels[0].attrib,rows[0])[2]+[min(p[1] for p in points),max(p[1] for p in points)]
            metadata.update(csv=str(csvpath.relative_to(game)),csv_sha256=digest(csvpath),row=0,source_parameters=parameters)
        source_cases.append((metadata,points,faces,parameters))
    assert len(source_cases)==43 and len(skipped)==1
    source_rejections=[];admitted=[]
    for metadata,points,faces,parameters in source_cases:
        degenerate=[]
        for i,tri in enumerate(faces):
            a,b,c=[adjusted(points,parameters)[k] for k in tri];ab=[f32(b[j]-a[j]) for j in range(3)];ac=[f32(c[j]-a[j]) for j in range(3)]
            n=[f32(f32(ab[1]*ac[2])-f32(ab[2]*ac[1])),f32(f32(ac[0]*ab[2])-f32(ac[2]*ab[0])),f32(f32(ac[1]*ab[0])-f32(ac[0]*ab[1]))]
            sq=f32(f32(f32(n[0]*n[0])+f32(n[1]*n[1]))+f32(n[2]*n[2]))
            if not math.isfinite(sq) or not 1e-20<=sq<=1e20:degenerate.append(i)
        if degenerate:
            source_rejections.append((dict(**metadata,degenerate_faces=degenerate),points,faces,parameters))
        else:admitted.append((metadata,points,faces,parameters))
    assert len(admitted)==42 and len(source_rejections)==1
    assert source_rejections[0][0]['source']==str(Path('cars/bos/bos.ned')) and source_rejections[0][0]['degenerate_faces']==[11,47]
    cases=[(metadata,p,t,fill,parameters) for metadata,p,t,parameters in admitted for fill in [0,0xcd]]
    cube=[[-1.,-1.,-1.,1.],[1.,-1.,-1.,1.],[1.,1.,-1.,1.],[-1.,1.,-1.,1.],[-1.,-1.,1.,1.],[1.,-1.,1.,1.],[1.,1.,1.,1.],[-1.,1.,1.,1.]]
    triangles=[[0,1,2],[0,2,3],[4,6,5],[4,7,6],[0,4,5],[0,5,1],[3,2,6],[3,6,7],[0,3,7],[0,7,4],[1,5,6],[1,6,2]]
    rng=random.Random(2008212)
    for i in range(96):
        scales=[rng.uniform(.2,4) for _ in range(3)];offset=[rng.uniform(-4,4) for _ in range(3)]
        points=[[f32(p[j]*scales[j]+offset[j]) for j in range(3)]+[1.] for p in cube]
        faces=[list(reversed(t)) if i%2 else t[:] for t in triangles]
        rng.shuffle(faces)
        parameters=[*[f32(rng.uniform(-10,10)) for _ in range(3)],0.,f32(rng.uniform(-5,0)),f32(rng.uniform(0,5))] if args.source_adjustment else None
        cases.append((dict(kind='authored',control=i,source_parameters=parameters),points,faces,[0,0xcd][i%2],parameters))
    def encode(points,faces,fill,parameters=None,shape=None):
        if args.shape and shape is None:shape=[1,.01,7,1]
        return request(points,faces,fill,parameters,shape)
    results=[]
    for i,(metadata,points,faces,fill,parameters) in enumerate(cases):
        shape=None
        if args.shape:
            span=[1,15,16,17,8192,16000][i%6];scalar=[-0.,.01,-.125,1.5][i%4]
            shape=[span,scalar,(i*17)%256,i%2];metadata=dict(**metadata,shape_settings=shape)
        inp=out/f'{i:03}-input.bin';inp.write_bytes(encode(points,faces,fill,parameters,shape));executions=[];answers=[]
        for kind in ['original','repeat','rust']:
            output=out/f'{i:03}-{kind}.bin';e=run(rust if kind=='rust' else native,[inp,output] if kind=='rust' else [IMAGE,inp,output]);executions.append(e)
            (out/'current-executions.json').write_text(json.dumps(dict(index=i,executions=executions),indent=2));assert e['exit']==0 and not e['stderr'],(i,e)
            if kind!='rust':assert 'code_unchanged=1 code_replacements=0 globals_restored=1 error=0' in e['stdout']
            answers.append(output.read_bytes())
        diff=[dict(offset=hex(j),original=answers[0][j:j+4].hex(),repeat=answers[1][j:j+4].hex(),rust=answers[2][j:j+4].hex()) for j in range(0,len(answers[0]),4) if len({a[j:j+4] for a in answers})!=1]
        (out/'current-differences.json').write_text(json.dumps(dict(index=i,differences=diff),indent=2));assert not diff,(i,diff[:12])
        checked=check(answers[0],adjusted(points,parameters),faces,fill,shape)
        results.append(dict(index=i,**metadata,points=len(points),faces=len(faces),fill=fill,bytes=len(answers[0]),input_sha256=digest(inp),output_sha256=digest(out/f'{i:03}-original.bin'),**checked,executions=executions))
    parameters=[0.,0.,0.,0.,-1.,1.] if args.source_adjustment else None
    extra=40 if args.shape else (24 if args.source_adjustment else 0)
    sample=encode(cube,triangles,0,parameters);variants=[('short',sample[:-1]),('long',sample+b'\0')]
    for label,o,value in [('tag',0,0),('points-low',4,3),('points-high',4,81),('faces-low',8,3),('faces-high',8,111),('fill',12,1),('point-nan',16,0x7fc00000),('point-inf',20,0x7f800000),('point-bound',24,struct.unpack('<I',struct.pack('<f',100001.))[0]),('point-w',28,0),('index',16+8*16,8),('duplicate-index',16+8*16+4,0)]:
        data=bytearray(sample);struct.pack_into('<I',data,o+(extra if o>=16 else 0),value);variants.append((label,data))
    points=[v[:] for v in cube];points[1]=points[0][:];variants.append(('coincident',encode(points,triangles,0,parameters)))
    points=[[f32(v*.0000001) for v in p[:3]]+[1.] for p in cube];variants.append(('tiny-normal',encode(points,triangles,0,parameters)))
    points=[[rng.uniform(-2,2) for _ in range(3)]+[1.] for _ in range(12)];faces=list(itertools.islice(itertools.combinations(range(12),3),110));variants.append(('adjacency-capacity',encode(points,faces,0,parameters)))
    if args.source_adjustment:
        for label,o,value in [('offset-nan',16,0x7fc00000),('offset-inf',20,0x7f800000),('offset-bound',24,struct.unpack('<I',struct.pack('<f',10001.))[0]),('offset-w',28,0x3f800000),('extents-nan',32,0x7fc00000),('extents-reversed',32,0x40000000)]:
            data=bytearray(sample);struct.pack_into('<I',data,o,value);variants.append((label,data))
        points=[[f32(p[0]+99999.),p[1],p[2],1.] for p in cube]
        variants.append(('adjusted-bound',encode(points,triangles,0,[10.,0.,0.,0.,-1.,1.])))
    if args.shape:
        for label,o,value in [('span-zero',40,0),('span-high',40,16001),('scalar-nan',44,0x7fc00000),('scalar-inf',44,0x7f800000),('scalar-bound',44,struct.unpack('<I',struct.pack('<f',1000001.))[0]),('layer',48,256),('flag-c0',52,2)]:
            data=bytearray(sample);struct.pack_into('<I',data,o,value);variants.append((label,data))
    source_rejected=[]
    for metadata,points,faces,parameters in source_rejections:
        for fill in [0,0xcd]:
            inp=out/f'source-rejected-{fill}.bin';inp.write_bytes(encode(points,faces,fill,parameters))
            for kind,exe in [('native',native),('rust',rust)]:
                output=out/f'source-rejected-{fill}-{kind}.bin';e=run(exe,[IMAGE,inp,output] if kind=='native' else [inp,output]);assert 0<e['exit']<256 and not output.exists()
                if kind=='native':assert not e['stderr'] and 'error=8' in e['stdout']
                source_rejected.append(dict(**metadata,fill=fill,implementation=kind,execution=e,no_output=True,input_sha256=digest(inp)))
    guards=[]
    for label,data in variants:
        inp=out/f'guard-{label}.bin';inp.write_bytes(data)
        for kind,exe in [('native',native),('rust',rust)]:
            output=out/f'guard-{label}-{kind}.bin';e=run(exe,[IMAGE,inp,output] if kind=='native' else [inp,output]);assert 0<e['exit']<256 and not output.exists(),(label,kind,e)
            if kind=='native':assert not e['stderr'] and 'code_unchanged=1' in e['stdout'] and 'globals_restored=1' in e['stdout']
            guards.append(dict(label=label,kind=kind,execution=e,no_output=True))
    assert digest(IMAGE)=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    for metadata,_,_,_ in source_cases:
        assert digest(game/metadata['source'])==metadata['source_sha256']
        if args.source_adjustment:assert digest(game/metadata['csv'])==metadata['csv_sha256']
    for kind,exe in [('native',native),('rust',rust)]:
        existing=out/f'existing-{kind}.bin';existing.write_bytes(b'preserve existing evidence');before=digest(existing)
        inp=out/'000-input.bin';e=run(exe,[IMAGE,inp,existing] if kind=='native' else [inp,existing]);assert 0<e['exit']<256 and digest(existing)==before
        if kind=='native':assert not e['stderr'] and 'error=19' in e['stdout']
        guards.append(dict(label='existing-output',kind=kind,execution=e,existing_output_preserved=True))
    report=dict(passed=True,profile=args.profile,cases=len(results),examined_source_meshes=43,admitted_source_meshes=42,source_cases=84,authored_controls=96,observable_bytes=sum(r['bytes'] for r in results),results=results,source_rejected=source_rejected,guards=guards,guard_count=len(guards),skipped=skipped,executables={str(p.relative_to(ROOT)):digest(p) for p in [native,rust,IMAGE]},source_adjustment=args.source_adjustment,shape_update=args.shape,scope=('Whole B444D0 supplied owned shape update after the following stages; mesh planes oriented away from average vertex. Base constructor/physical registration excluded. ' if args.shape else '')+('Whole 9B8B70/9B8890 deformation-off adjustment then whole B44E80. Source offset from CSV/NED wheel layout, raw min/max Y supplied as bottom/roof; owned car and raw inputs unchanged. ' if args.source_adjustment else 'Whole supplied B44E80. ')+ 'Complete normal/extent/topology and owned relative pointers. BOS zero-area faces explicitly rejected. Not whole outer source factory, deformation, convexification, allocation, shape registration or contact generation.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['passed','cases','observable_bytes','guard_count']}))
if __name__=='__main__':main()
