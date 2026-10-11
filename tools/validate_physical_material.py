"""Physical source loader: original accessors/defaults, explicitly adapted CRT.

Unmodified-code controls contain no numeric attributes. Adapted runs replace only
the two CRT numeric conversions; full original CRT text parsing is not claimed.
"""
import argparse, hashlib, json, struct, subprocess, xml.etree.ElementTree as ET
from pathlib import Path
from validate_surface_mechanics import encode
from validate_binxml import decode
ROOT = Path(__file__).resolve().parents[1]
INSTALL = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
SOURCES = {
    'surface_materials.xml': '8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251',
    'car_materials.xml': 'dfb9e93bfe6d9ebbf2bc639be244a567d2f4fffaa620601880e1b34cdede81e2',
}
FIELDS = [('restitution',0.),('friction',.4),('cfm',.01),('correctionVel',1.),
          ('minBounceVel',0.),('minSoftK',0.),('minSoftS',0.),('maxSoftK',0.),
          ('maxSoftS',0.),('speedForMaxSoftness',0.),('dynamicScale',0.),('dynamicThreshold',10.)]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def expected(root):
    rows=[]
    for m in root.findall('MATERIAL'):
        words=[int.from_bytes(m.get('name').encode('ascii'),'little')]
        words += [struct.unpack('<I',struct.pack('<f',float(m.get(k,d))))[0] for k,d in FIELDS]
        colors=[]
        for k,d in [('red',255),('green',255),('blue',255),('alpha',180)]:
            text=m.get(k,str(d)).lstrip();sign=-1 if text.startswith('-') else 1;text=text.lstrip('+-')
            radix=16 if text.lower().startswith('0x') else 8 if len(text)>1 and text.startswith('0') else 10
            colors.append(min(255,max(0,sign*int(text,radix))))
        words += [int.from_bytes(bytes(colors),'little'),int(m.get('sheet','').lower()=='true')]
        rows.append(['P',*[f'{w:08x}' for w in words]])
    return rows
def execute(exe,args):
    r=subprocess.run([str(exe),*map(str,args)],cwd=ROOT,capture_output=True,text=True,timeout=30)
    return dict(exe=str(exe),sha256=digest(exe),exit=r.returncode,stdout=r.stdout,stderr=r.stderr)
def controls():
    default=ET.Element('MATERIALS');authored=ET.Element('MATERIALS')
    for i,sheet in enumerate([None,'true','FALSE','TRUE','false','0','1','yes']):
        m=ET.SubElement(default,'MATERIAL',name=f'D{i:03d}')
        if sheet is not None:m.set('sheet',sheet)
    for i in range(40):
        m=ET.SubElement(authored,'MATERIAL',name=f'A{i:03d}')
        for j,(key,_) in enumerate(FIELDS):
            if i%5!=0:m.set(key,'1.0000000596046448' if i==1 else '-0.0' if i==2 else str((i-j)*.125))
        for j,key in enumerate(['red','green','blue','alpha']):
            if i%7:m.set(key,['-10','0','255','300','0xff','0377','+42','-0x1',' 123','2147483647','-2147483648'][(i+j)%11])
        m.set('sheet',['true','FALSE','TRUE','false','0','1','yes'][i%7])
    return default,authored
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],default='debug');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    default,authored=controls();default_path=out/'unchanged-default-controls.bin';default_path.write_bytes(encode(default))
    authored_path=out/'authored-controls.bin';authored_path.write_bytes(encode(authored))
    sources=[(name,INSTALL/name,'adapted') for name,sha in SOURCES.items() if digest(INSTALL/name)==sha]
    assert len(sources)==len(SOURCES)
    sources += [('defaults',default_path,'original'),('authored',authored_path,'adapted')]
    comparisons=[];executions=[]
    for label,path,mode in sources:
        _,root,_=decode(path.read_bytes());reference=expected(root);outputs=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{label}-{kind}.tsv'
            exe=ROOT/f'target/{a.profile}/grid-physical-material.exe' if kind=='rust' else ROOT/'local/behavior/physical-material-probe.exe'
            args=[path,target] if kind=='rust' else [ROOT/'local/compatibility/grid-unpacked.exe',path,target,mode]
            run=execute(exe,args);executions.append(dict(label=label,kind=kind,**run))
            (out/'executions.json').write_text(json.dumps(executions,indent=2))
            assert run['exit']==0 and not run['stderr'],run
            if kind!='rust':assert f'adapted={int(mode=="adapted")} restored=1 error=0' in run['stdout']
            outputs.append([r.split('\t') for r in target.read_text().splitlines()])
        assert all(len(q)==len(reference) for q in outputs)
        differences=[dict(index=i,original=x,repeat=y,rust=z,independent=r) for i,(x,y,z,r) in enumerate(zip(*outputs,reference,strict=True)) if not x==y==z==r]
        comparisons.append(dict(label=label,mode=mode,input=str(path),sha256=digest(path),materials=len(reference),words_per_material=15,differences=differences))
        (out/'comparisons.json').write_text(json.dumps(comparisons,indent=2));assert not differences,differences[:3]
    guards=[]
    for key,value in [('friction','NaN'),('cfm','inf'),('dynamicScale','1e50'),('red','2147483648'),('green','-2147483649'),('blue','oops')]:
        root=ET.Element('MATERIALS');ET.SubElement(root,'MATERIAL',{'name':'FAIL',key:value})
        path=out/f'guard-{key}.bin';path.write_bytes(encode(root));target=out/f'guard-{key}.tsv'
        run=execute(ROOT/f'target/{a.profile}/grid-physical-material.exe',[path,target]);guards.append(dict(key=key,value=value,passed=run['exit']==1 and not target.exists(),execution=run))
    assert all(q['passed'] for q in guards)
    assert all(digest(INSTALL/n)==sha for n,sha in SOURCES.items())
    report=dict(passed=True,profile=a.profile,comparisons=comparisons,executions=executions,guards=guards,
                limits='Original loader/accessors/defaults exercised. Numeric float/integer CRT conversions adapted for installed/authored inputs; original CRT parsing, malformed text equivalence, shape resource identity and full car initialization excluded. Strict Rust malformed/nonfinite guards are not original-equivalence claims.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(dict(passed=True,materials=sum(q['materials'] for q in comparisons),guards=len(guards),unchanged_code_materials=len(expected(default)))))
if __name__=='__main__':main()
