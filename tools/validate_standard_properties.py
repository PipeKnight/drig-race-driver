"""Owned-cell whole Standard decoder reference; Rust equivalence is separate.

Original decoder/lookup/defaults/setup/math, host float/integer CRT adapters.
No original CSV tokenization/handle construction/full constructor claim.
"""
import argparse,csv,hashlib,io,json,random,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
GAME=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def controls(out):
    base=[r for r in csv.reader((GAME/'cars/lac/lac.csv').read_text().splitlines()) if r][0][:360]
    variations=[]
    for column in range(3,360):
        if column in [49,57,63,67,78]:continue
        try:float(base[column] or 0)
        except ValueError:continue
        r=base.copy();r[column]=['0','-0','1.0000000596046448','-1.25','1e-50'][column%5]
        if column==3:r[column]='1.25'
        if column in [147,149,276,262,263]:r[column]='1'
        variations.append((f'column-{column}',r))
    for drive in ['Front','Rear','Four','All','Unknown']:
        for diff in ['Full','Half','Simple','Custom','Unknown']:
            r=base.copy();r[67]=drive;r[78]=diff;r[262]='1535151';r[263]='303030';variations.append((f'differential-{drive}-{diff}',r))
    for mode in range(8):
        r=base.copy()
        for i in range(216,268):r[i]=str((i-240)*.125)
        if mode<4:r[[216,253,264,267][mode]]=''
        if mode==5:r[228]='-0';r[229]='0'
        if mode==6:r[232]='1.0000000596046448'
        if mode==7:r[264]='-1e-50';r[265]='1e-50'
        variations.append((f'custom-setup-{mode}',r))
    for i,value in enumerate(['0','-0','0.5','0.5000000000000001','-1e-50','1e-50']):
        r=base.copy()
        for column in [112,187,190,107,108,109,117,127,135,142,148,153,164,172,176,311,332,334,335]:r[column]=value
        variations.append((f'boolean-threshold-{i}',r))
    rng=random.Random(2008202)
    for sample in range(32):
        r=base.copy()
        for column in range(3,360):
            if column in [49,57,63,67,78]:continue
            r[column]=str(rng.uniform(-4,8))
        r[3]=str(rng.uniform(100,2500));r[12]=str(rng.uniform(.1,2))
        for column in [60,147,149,262,263,276]:r[column]=str(rng.randrange(0,1000))
        variations.append((f'combined-{sample}',r))
    fixtures=[]
    for i,(label,r) in enumerate(variations):
        r[0]=f'CONTROL{i:03}';buf=io.StringIO(newline='');csv.writer(buf,lineterminator='\r\n').writerow(r)
        path=out/f'control-{i:03}.csv';path.write_bytes(buf.getvalue().encode('ascii'))
        fixtures.append(dict(path=str(path),sha256=digest(path),label=label,authored=True))
    return fixtures
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release']);p.add_argument('--controls',action='store_true');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    baseline_path=ROOT/'local/behavior/wheel-car-manager-debug-20261011-final-01-source/validation.json'
    baseline=json.loads(baseline_path.read_text());assert baseline['source_files']==44
    exe=ROOT/'local/behavior/standard-properties-probe.exe';image=ROOT/'local/compatibility/grid-unpacked.exe'
    assert digest(image)=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    results=[];executions=[];differences=[]
    fixtures=baseline['files']+(controls(out) if a.controls else [])
    for i,f in enumerate(fixtures):
        source=GAME/f['path'];assert digest(source)==f['sha256']
        rows=[r for r in csv.reader(source.read_text().splitlines()) if r];assert len(rows)==1
        row=rows[0];assert len(row)==360 or len(row)==361 and not row[-1];row=row[:360]
        assert row[0] and row[2]=='Standard' and all(len(s.encode('ascii'))<64 for s in row)
        cells=out/f'{i:03}-cells.bin';cells.write_bytes(b''.join(s.encode('ascii').ljust(64,b'\0') for s in row))
        outputs=[]
        for kind in ['original','repeat']:
            target=out/f'{i:03}-{kind}.bin';r=subprocess.run([str(exe),str(image),str(cells),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            run=dict(source=f['path'],kind=kind,exit=r.returncode,stdout=r.stdout,stderr=r.stderr);executions.append(run)
            (out/'executions.json').write_text(json.dumps(executions,indent=2))
            assert r.returncode==0 and not r.stderr and 'data_restored=1 code_restored=1 error=0' in r.stdout,run
            data=target.read_bytes();assert len(data)==0x65c;outputs.append(data)
        assert outputs[0]==outputs[1]
        data=outputs[0];f32=lambda o:struct.unpack_from('<f',data,o)[0]
        # Independent checks on source subset already established by prior milestones.
        word=lambda n:struct.unpack('<I',struct.pack('<f',n))[0]
        assert struct.unpack_from('<I',data,0x130)[0]==word(float(row[3]))
        assert struct.unpack_from('<I',data,0x318)[0]==0
        assert [struct.unpack_from('<I',data,0x31c+4*j)[0] for j in range(8)]==[word(float(row[k] or 0)) for k in range(69,77)]
        assert struct.unpack_from('<I',data,0x33c)[0]==word(float(row[68] or 0))
        assert struct.unpack_from('<I',data,0x658)[0]==0x2008
        assert digest(source)==f['sha256']
        if a.profile:
            rust=ROOT/f'target/{a.profile}/grid-standard-properties.exe';target=out/f'{i:03}-rust.bin'
            r=subprocess.run([str(rust),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(source=f['path'],kind='rust',exit=r.returncode,stdout=r.stdout,stderr=r.stderr));(out/'executions.json').write_text(json.dumps(executions,indent=2))
            assert r.returncode==0 and not r.stderr,(f['path'],r.stderr)
            answer=target.read_bytes();assert len(answer)==len(data)
            differences.extend(dict(source=f['path'],offset=hex(o),original=data[o:o+4].hex(),rust=answer[o:o+4].hex()) for o in range(0,len(data),4) if data[o:o+4]!=answer[o:o+4])
        results.append(dict(source=f['path'],label=f.get('label','installed'),source_sha256=f['sha256'],cells_sha256=digest(cells),output_sha256=hashlib.sha256(data).hexdigest(),repeat_exact=True,properties_bytes=len(data),mass=f32(0x130)))
    report=dict(native_reference_passed=True,rust_equivalence=bool(a.profile) and not differences,profile=a.profile,differences=differences,sources=44,authored=len(fixtures)-44,properties_bytes=sum(q['properties_bytes'] for q in results),executions=executions,results=results,exe=str(exe.relative_to(ROOT)),exe_sha256=digest(exe),baseline_path=str(baseline_path.relative_to(ROOT)),baseline_sha256=digest(baseline_path),limits='Owned decoded 360x64 cells. Original whole9AEC60/991640/998860/998E60/math; two host numeric CRT adapters. Original tokenizer/handle/CRT parsing/full car constructor excluded.')
    if a.profile:report.update(rust_exe=str(rust.relative_to(ROOT)),rust_exe_sha256=digest(rust))
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['executions','results','differences']}));print('differences',len(differences),differences[:12])
    assert not differences
if __name__=='__main__':main()
