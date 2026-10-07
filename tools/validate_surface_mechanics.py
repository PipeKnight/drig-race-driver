"""Surface source fields/defaults against original XML views and loader.
Original numeric CRT conversion is replaced with host strtod; no complete original parsing claim.
"""
import argparse,hashlib,json,struct,subprocess,xml.etree.ElementTree as ET
from pathlib import Path
from validate_binxml import decode
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def word(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def encode(root):
    nodes=[root];records=[];strings=[''];ids={'':0};attributes=[]
    def sid(text):
        if text not in ids:ids[text]=len(strings);strings.append(text)
        return ids[text]
    index=0
    while index<len(nodes):
        node=nodes[index];index+=1
        ast=len(attributes)
        attributes.extend((sid(k),sid(v)) for k,v in node.attrib.items())
        start=len(nodes);nodes.extend(list(node))
        records.append((sid(node.tag),0,len(node.attrib),ast,len(node),start))
    blob=b'';offsets=[]
    for s in strings:offsets.append(len(blob));blob+=s.encode('ascii')+b'\0'
    def section(tag,data):return struct.pack('<II',tag,len(data))+data
    a=section(0x7252221d,blob);b=section(0x7252221e,struct.pack('<'+'I'*len(offsets),*offsets))
    c=section(0x7252221b,b''.join(struct.pack('<6I',*r) for r in records))
    d=section(0x7252221c,b''.join(struct.pack('<2I',*r) for r in attributes))
    payload=struct.pack('<II',0x72522217,len(a)+len(b))+a+b+c+d
    return struct.pack('<II',0x7252221a,len(payload))+payload

def expected(root,force=False):
    rows=[]
    for m in root.findall('MATERIAL'):
        code=int.from_bytes(m.get('name').encode(),'little');q=m.find('MECHANICS')
        if q is None:words=[0]*9;name=''
        else:
            a=q.attrib
            n=lambda key,default:struct.unpack('<f',struct.pack('<f',float(a.get(key,default))))[0]
            table=a.get('tyreTable','DEFA');table=table[:4] if len(table)>=4 else 'DEFA'
            real=min(1.,max(0.,n('bumpsrealproportion',0.)))
            words=[int(a.get('bumpstimebased','true').lower()=='true'),word(n('bumpswavelength',.05)),word(n('bumpsmagnitude',0.)),word(real),word(n('bumpsproportionCarDependent',0.)),word(n('depth',0.)),word(n('grip',1.)),word(n('hardness',1.)),int.from_bytes(table.encode(),'little')]
            name=a['name'].encode().hex()
        if force:
            if q is None:words += [0]*13
            else:
                words += [word(n(k,default)) for k,default in [
                    ('grip_per_ms',0.),('ms_for_max_effect',1.),('camera_factor',0.),('roughness',0.),
                    ('minimumSpeedForSlowdown',20.),('speedForMaximumSlowdown',40.),
                    ('slowdown',0.),('roll_induce',0.),('heat',1.),('wear',1.)]]
                words += [int(a.get(k,'false').lower()=='true') for k in ['curved','dirt_build_up','high_downforce']]
        rows.append(['M',f'{code:08x}',f'{int(q is not None):08x}',*[f'{v:08x}' for v in words],name])
    return rows

def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');parser.add_argument('--force',action='store_true');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/surface_materials.xml');sha='8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251';assert digest(source)==sha
    tree=ET.Element('MATERIALS')
    for i in range(24):
        m=ET.SubElement(tree,'MATERIAL',name=f'T{i:03d}')
        if i==23:continue
        q=ET.SubElement(m,'MECHANICS',name=f'AUTHORED{i:02d}')
        if i:
            q.set('bumpstimebased',['true','FALSE','TRUE','false'][i%4])
            q.set('bumpswavelength',str(.05+i*.01));q.set('bumpsmagnitude',str(i*.001))
            q.set('bumpsrealproportion',str([-2.,-.1,0.,.3,1.,2.][i%6]))
            q.set('bumpsproportionCarDependent',str(i*.1-1));q.set('depth',str(i*.01))
            q.set('tyreTable',['XX','TEST_more','ABCD'][i%3]);q.set('grip','1.0000000596046448' if i==1 else str(i*.2));q.set('hardness',str(i*.03))
            if args.force:
                for j,key in enumerate(['grip_per_ms','ms_for_max_effect','camera_factor','roughness',
                    'minimumSpeedForSlowdown','speedForMaximumSlowdown','slowdown','roll_induce','heat','wear']):
                    q.set(key,'1.0000000596046448' if i==1 else str((i-j)*.125))
                for j,key in enumerate(['curved','dirt_build_up','high_downforce']):
                    q.set(key,['true','FALSE','0','1','yes','TRUE','false'][(i+j)%7])
    synthetic=out/'authored-controls.bin';synthetic.write_bytes(encode(tree))
    image=ROOT/'local/compatibility/grid-unpacked.exe';executions=[];comparisons=[]
    for label,path in [('installed',source),('authored',synthetic)]:
        _,root,_=decode(path.read_bytes());reference=expected(root,args.force)
        for kind,exe,arguments in [
            ('original',ROOT/'local/behavior/surface-mechanics-probe.exe',[image,path,out/f'{label}-original.tsv']),
            ('native',ROOT/f'target/{args.profile}/grid-surface-mechanics.exe',[path,out/f'{label}-native.tsv'])]:
            if args.force:arguments=arguments+['force']
            result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(kind=kind,label=label,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
            if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Mechanics comparison failed')
        original=[line.split('\t') for line in (out/f'{label}-original.tsv').read_text().splitlines()]
        native=[line.split('\t') for line in (out/f'{label}-native.tsv').read_text().splitlines()]
        assert len(original)==len(native)==len(reference)
        differences=[dict(index=i,original=a,native=b,independent=c) for i,(a,b,c) in enumerate(zip(original,native,reference)) if not a==b==c]
        comparisons.append(dict(label=label,input_path=str(path),input_sha256=digest(path),materials=len(reference),mechanics=sum(row[2]=='00000001' for row in reference),exact=len(reference)-len(differences),differences=differences))
    assert digest(source)==sha
    report=dict(profile=args.profile,force=args.force,field_words_per_row=22 if args.force else 9,executions=executions,comparisons=comparisons,limits='Original mechanics initializer/consumer and binary-XML accessors, numeric CRT conversion replaced with host strtod; original CRT direct attempt faults c0000005. Missing mechanics output explicitly absent. No complete original parsing/other material sections/tyre simulation claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps([{k:v for k,v in r.items() if k in ['label','materials','mechanics','exact']} for r in comparisons]));assert all(not r['differences'] for r in comparisons)
if __name__=='__main__':main()
