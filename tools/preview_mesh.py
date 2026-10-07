"""Orthographic diagnostic plot of private OBJ sources, without scene assembly."""
import argparse
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont


def preview(source,output):
    vertices=[];triangles=[];group=-1
    for line in source.read_text().splitlines():
        f=line.split()
        if not f:continue
        if f[0]=='o':group+=1
        if f[0]=='v':vertices.append(tuple(map(float,f[1:])))
        if f[0]=='f':triangles.append(([int(v.split('/')[0])-1 for v in f[1:]],group))
    angle=math.radians(38);elevation=math.radians(19)
    right=(math.cos(angle),0,-math.sin(angle))
    up=(-math.sin(elevation)*math.sin(angle),math.cos(elevation),-math.sin(elevation)*math.cos(angle))
    forward=(math.cos(elevation)*math.sin(angle),math.sin(elevation),math.cos(elevation)*math.cos(angle))
    def dot(a,b):return sum(x*y for x,y in zip(a,b))
    projected=[(dot(v,right),dot(v,up),dot(v,forward)) for v in vertices]
    lo=[min(v[i] for v in projected) for i in range(2)];hi=[max(v[i] for v in projected) for i in range(2)]
    width,height=1200,900
    scale=min((width-120)/(hi[0]-lo[0]),(height-240)/(hi[1]-lo[1]))
    center=[(a+b)/2 for a,b in zip(lo,hi)]
    xy=[(width/2+(v[0]-center[0])*scale,height/2+30-(v[1]-center[1])*scale) for v in projected]
    image=Image.new('RGB',(width,height),(18,24,34));draw=ImageDraw.Draw(image)
    palette=[(140,170,200),(180,194,210),(78,114,152),(195,150,110)]
    group_count=group+1
    for indices,triangle_group in sorted(triangles,key=lambda t:sum(projected[i][2] for i in t[0])):
        a,b,c=[vertices[i] for i in indices]
        ab=[x-y for x,y in zip(b,a)];ac=[x-y for x,y in zip(c,a)]
        normal=(ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0])
        length=math.sqrt(dot(normal,normal))
        shade=0.35+0.65*abs(dot(normal,(-0.3,0.85,0.43)))/length if length else 0.35
        color=tuple(int(v*min(shade,1)) for v in palette[triangle_group%len(palette)])
        draw.polygon([xy[i] for i in indices],fill=color)
    try:
        title=ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf',30)
        caption=ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf',20)
    except OSError:title=caption=ImageFont.load_default()
    draw.text((40,28),'GRID Lacetti: decoded render-source geometry',font=title,fill=(230,237,247))
    draw.text((40,78),f'{group_count} source groups | {len(vertices):,} positions | {len(triangles):,} triangles',font=caption,fill=(150,174,200))
    draw.text((40,height-85),'Raw local buffers; instance transforms, skinning and materials are not applied.',font=caption,fill=(170,187,206))
    draw.text((40,height-50),'Diagnostic plot from the Rust OBJ export. Source units/scene assembly remain unverified.',font=caption,fill=(170,187,206))
    with output.open('xb') as f:image.save(f,format='PNG')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('source',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args()
    private=(Path(__file__).resolve().parent.parent/'local').resolve()
    if not all(p.resolve().is_relative_to(private) for p in [args.source,args.output]):raise SystemExit('Preview artifacts must remain under local/')
    preview(args.source,args.output)
