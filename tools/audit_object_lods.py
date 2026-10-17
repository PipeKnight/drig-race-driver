"""Read-only bounds/indices comparison of one purchased object prototype's LOD draws.

All game-derived reports stay in this checkout's ignored local/ directory.
"""
import argparse
import collections
import hashlib
import json
import math
import struct
from pathlib import Path
from survey_pssg_payloads import survey

IDENTITY = [float(i % 5 == 0) for i in range(16)]


def compose(a, b):
    return [sum(a[(i//4)*4+k]*b[k*4+i%4] for k in range(4)) for i in range(16)]


def inspect(source, root_name, previews=None):
    if source.stat().st_size > 256*1024*1024:
        raise ValueError('resource exceeds 256MiB')
    report=survey(source); raw=source.read_bytes()
    if hashlib.sha256(raw).hexdigest()!=report['sha256']:
        raise ValueError('source changed during survey')
    ns=report['nodes']; children=collections.defaultdict(list)
    for i,n in enumerate(ns): children[n['parent']].append(i)
    roots=[i for i,n in enumerate(ns) if n['name']=='ROOTNODE' and n['attributes'].get('id',{}).get('string')==root_name]
    if len(roots)!=1: raise ValueError('expected one exact prototype root')
    root=roots[0]
    def string(i,k): return ns[i]['attributes'][k]['string']
    def integer(i,k): return ns[i]['attributes'][k]['u32be']
    def single(i,kind):
        values=[j for j in children[i] if ns[j]['name']==kind]
        if len(values)!=1: raise ValueError(f'expected one {kind}')
        return values[0]
    def find(kind,ident):
        values=[i for i,n in enumerate(ns) if n['name']==kind and n['attributes'].get('id',{}).get('string')==ident]
        if len(values)!=1: raise ValueError(f'ambiguous {kind} reference')
        return values[0]
    rows=[]; geometry=[]; decoded_vertices=0
    for i,n in enumerate(ns):
        if n['name']!='RENDERSTREAMINSTANCE':continue
        chain=[];parent=n['parent']
        while parent is not None:
            if len(chain)>128:raise ValueError('unbounded ancestry')
            chain.append(parent)
            if parent==root:break
            parent=ns[parent]['parent']
        if root not in chain:continue
        matrix=IDENTITY.copy()
        for parent in chain:
            transforms=[j for j in children[parent] if ns[j]['name']=='TRANSFORM']
            if len(transforms)>1:raise ValueError('ambiguous local transform')
            if transforms:
                node=ns[transforms[0]]
                if node['payload_size']!=64:raise ValueError('unexpected transform size')
                transform=list(struct.unpack_from('>16f',raw,node['payload_offset']))
                if not all(math.isfinite(x) for x in transform):raise ValueError('nonfinite transform')
                matrix=compose(matrix,transform)
        stage='lower' if any(ns[j]['name']=='LODRENDERINSTANCELIST' for j in chain) else ('closest' if ns[n['parent']]['name']=='LODVISIBLERENDERNODE' else 'other')
        if any(ns[j]['attributes'].get('id',{}).get('string','').startswith('rigidbody') for j in chain):stage='physics'
        refs=[j for j in children[i] if ns[j]['name']=='RENDERINSTANCESOURCE']
        if len(refs)!=1:raise ValueError('expected one static source')
        ref=string(refs[0],'source')
        if not ref.startswith('#'):raise ValueError('unsupported source reference')
        if string(i,'indices')!=ref:raise ValueError('cross-source index selection unsupported')
        if set(ns[refs[0]]['attributes'])!={'source'}:raise ValueError('partial source selection unsupported')
        mesh=find('RENDERDATASOURCE',ref[1:]); positions=None
        for stream in children[mesh]:
            if ns[stream]['name']!='RENDERSTREAM':continue
            block=find('DATABLOCK',string(stream,'dataBlock').removeprefix('#'))
            streams=[j for j in children[block] if ns[j]['name']=='DATABLOCKSTREAM']
            sub=integer(stream,'subStream')
            if sub>=len(streams):raise ValueError('stream index out of bounds')
            attr=streams[sub]
            if string(attr,'renderType')!='Vertex':continue
            if positions is not None or string(attr,'dataType')!='float3':raise ValueError('expected one float3 position stream')
            data=ns[single(block,'DATABLOCKDATA')];count=integer(block,'elementCount');stride=integer(attr,'stride');offset=integer(attr,'offset')
            if not 1<=count<=1_000_000 or offset+12>stride or count*stride!=data['payload_size']:raise ValueError('invalid position bounds/stride')
            positions=[struct.unpack_from('>3f',raw,data['payload_offset']+offset+k*stride) for k in range(count)]
        if positions is None or not all(math.isfinite(x) for p in positions for x in p):raise ValueError('missing/nonfinite positions')
        ix=single(mesh,'RENDERINDEXSOURCE');data=ns[single(ix,'INDEXSOURCEDATA')];count=integer(ix,'count')
        if string(ix,'format')!='ushort' or string(ix,'primitive')!='triangles' or not 0<count<=3_000_000 or count%3 or data['payload_size']!=count*2:raise ValueError('unsupported indices')
        indices=struct.unpack_from(f'>{count}H',raw,data['payload_offset'])
        if max(indices)>=len(positions):raise ValueError('index out of bounds')
        decoded_vertices+=len(positions)
        if decoded_vertices>2_000_000 or len(rows)>=64:raise ValueError('prototype exceeds diagnostic geometry bound')
        used=sorted(set(indices));selected=[positions[j] for j in used]
        transformed=[[sum(p[k]*matrix[k*4+axis] for k in range(3))+matrix[12+axis] for axis in range(3)] for p in positions]
        points=[transformed[j] for j in used]
        area=0.;positive=0.;negative=0.
        for t in range(0,len(indices),3):
            a,b,c=(transformed[j] for j in indices[t:t+3])
            signed=((b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]))*.5
            area+=abs(signed);positive+=max(0.,signed);negative+=max(0.,-signed)
        geometry.append((stage,string(i,'id'),transformed,indices))
        bounds=lambda ps:[[min(p[k] for p in ps) for k in range(3)],[max(p[k] for p in ps) for k in range(3)]]
        rows.append(dict(draw=string(i,'id'),stage=stage,source=ref[1:],shader=string(i,'shader'),vertices=len(positions),referenced_vertices=len(selected),triangles=count//3,xy_triangle_area_sum=area,xy_positive_area=positive,xy_negative_area=negative,local_bounds=bounds(selected),prototype_bounds=bounds(points),transform=matrix))
    if not rows:raise ValueError('prototype has no inspected draws')
    if previews is not None:
        from PIL import Image, ImageDraw
        previews.mkdir(exist_ok=False)
        lo=[min(r['prototype_bounds'][0][k] for r in rows) for k in (0,1)]
        hi=[max(r['prototype_bounds'][1][k] for r in rows) for k in (0,1)]
        if min(hi[k]-lo[k] for k in (0,1))<=1e-8:raise ValueError('degenerate XY projection')
        width=2048;height=max(64,min(2048,round(width*(hi[1]-lo[1])/(hi[0]-lo[0]))))
        sheet=Image.new('RGB',(width,height*len(rows)+30*len(rows)),(24,24,24))
        for row_index,((stage,ident,points,indices),row) in enumerate(zip(geometry,rows)):
            image=Image.new('L',(width,height),0);draw=ImageDraw.Draw(image)
            projected=[((p[0]-lo[0])/(hi[0]-lo[0])*(width-1),(hi[1]-p[1])/(hi[1]-lo[1])*(height-1)) for p in points]
            for t in range(0,len(indices),3):draw.polygon([projected[j] for j in indices[t:t+3]],fill=255)
            coverage=sum(image.histogram()[1:])/(width*height)
            row['xy_raster_union_coverage']=coverage
            image.save(previews/f'{row_index:02}-{stage}.png')
            sheet.paste(image,(0,row_index*(height+30)+30))
            ImageDraw.Draw(sheet).text((10,row_index*(height+30)+8),f'{stage}: {ident}; two-sided XY union; coverage {coverage:.4f}',fill='white')
        sheet.save(previews/'comparison.png')

    return dict(source=str(source),sha256=report['sha256'],prototype=root_name,draws=rows,scope='saved geometry only; no runtime LOD selection or placement visibility proof')


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('source',type=Path);p.add_argument('--root',required=True);p.add_argument('--output',required=True,type=Path);p.add_argument('--previews',type=Path);a=p.parse_args()
    private=(Path(__file__).resolve().parent.parent/'local').resolve()
    if not a.output.resolve().is_relative_to(private):p.error('report must stay under ignored local/')
    if a.previews is not None and not a.previews.resolve().is_relative_to(private):p.error('previews must stay under ignored local/')
    if a.output.exists() or (a.previews is not None and a.previews.exists()):raise FileExistsError('refusing existing evidence output')
    result=inspect(a.source,a.root,a.previews)
    with a.output.open('x',encoding='utf8') as f:json.dump(result,f,indent=2,allow_nan=False)
    for row in result['draws']:print(row['draw'],row['stage'],row['triangles'],'triangles',row['prototype_bounds'])


if __name__=='__main__':main()
