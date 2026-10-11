"""Read installed Standard gear tables through Rust and audit source bytes privately."""
import csv
import json
import struct
import subprocess
from pathlib import Path
from validate_vehicle_propulsion import ROOT,digest

GAME=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')


def verify(out,profile):
    out.mkdir(exist_ok=False);tables=[];metadata=[]
    for path in sorted((GAME/'cars').rglob('*.csv')):
        raw=path.read_bytes();rows=[row for row in csv.reader(raw.decode('utf-8').splitlines()) if row]
        if not all((len(r)==360 or len(r)==361 and not r[-1]) and r[2]=='Standard' for r in rows):continue
        report=out/f'{len(tables):03}.tsv'
        exe=ROOT/f'target/{profile}/grid-body-tuning.exe'
        run=subprocess.run([str(exe),str(path),str(report),'gears'],cwd=ROOT,capture_output=True,text=True,timeout=30)
        assert run.returncode==0 and not run.stderr,(path,run.stderr)
        expected=[]
        for i,r in enumerate(rows):
            bits=lambda s:struct.unpack('<I',struct.pack('<f',float(s or 0)))[0]
            words=[bits(r[69])]+[bits(r[j]) for j in range(69,77)]+[bits(r[68])]
            expected.append(['G',str(i),*[f'{w:08x}' for w in words]])
            tables.append(dict(path=str(path.relative_to(GAME)),words=words))
        assert [line.split('\t') for line in report.read_text().splitlines()]==expected
        metadata.append(dict(path=str(path.relative_to(GAME)),sha256=digest(path),rows=len(rows),exact=True))
    assert len(metadata)==len(tables)==44
    counts=sorted(set(sum(struct.unpack('<f',struct.pack('<I',w))[0]>0 for w in t['words'][1:9]) for t in tables))
    result=dict(files=metadata,source_files=44,forward_counts=counts,unused_zero_slots=sum(w&0x7fffffff==0 for t in tables for w in t['words'][1:9]),tables=tables,cli_sha256=digest(ROOT/f'target/{profile}/grid-body-tuning.exe'),scope='Decoded ratio columns and constructor slot-zero alias only; no runtime setup scaling, shift thresholds or whole constructor claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));return tables
