"""Compare native lexemes/numbers for all installed car-directory CSVs."""
import argparse
import collections
import csv
import hashlib
import json
import math
import struct
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('game_root',type=Path)
    args=parser.parse_args()
    workspace=Path(__file__).resolve().parent.parent
    out=workspace/'local/behavior/tuning-validation'
    out.mkdir(exist_ok=False)
    schema_path=workspace/'local/tools/ego-ctf-4.2/Ego CTF Editor/ctfSchemaGrid.xml'
    entries=ET.parse(schema_path).getroot().findall('entry')
    results=[]
    for ordinal,path in enumerate(sorted((args.game_root/'cars').rglob('*.csv'))):
        raw=path.read_bytes()
        text=raw.decode('utf-8')
        lines=text.splitlines()
        rows=[row for row in csv.reader(lines) if row]
        expected=[['B',str(sum(not line for line in lines))]]
        for i,row in enumerate(rows):
            expected.append(['R',str(i),str(len(row))])
            for j,value in enumerate(row):
                numeric=''
                if value:
                    try:
                        number=float(value)
                        numeric=f'{struct.unpack("<Q",struct.pack("<d",number))[0]:016x}' if math.isfinite(number) else 'nonnumeric'
                    except ValueError:
                        numeric='nonnumeric'
                expected.append(['C',str(i),str(j),value.encode().hex(),numeric])
        report=out/f'fields-{ordinal:03}.tsv'
        process=subprocess.run([str(workspace/'target/debug/grid-tuning.exe'),str(path),str(report)],capture_output=True,text=True)
        actual=[line.split('\t') for line in report.read_text().splitlines()] if process.returncode==0 else []
        fits_schema=len(rows)==1 and len(rows[0])>=len(entries) and all(not v for v in rows[0][len(entries):])
        mapping=[]
        if fits_schema:
            mapping=[dict(index=i,name=e.attrib['name'],type=e.attrib['type'],lexeme=rows[0][i]) for i,e in enumerate(entries)]
        results.append(dict(source=str(path.relative_to(args.game_root)),sha256=hashlib.sha256(raw).hexdigest(),
                            native_exit=process.returncode,stderr=process.stderr,exact=expected==actual,
                            row_field_counts=[len(row) for row in rows],schema_fits=fits_schema,schema_mapping=mapping))
    summary=dict(files=len(results),all_exact=all(r['exact'] for r in results),schema_fields=len(entries),
                 schema_sha256=hashlib.sha256(schema_path.read_bytes()).hexdigest(),schema_fits=sum(r['schema_fits'] for r in results),
                 exceptions=[dict(source=r['source'],row_field_counts=r['row_field_counts']) for r in results if not r['schema_fits']])
    with (out/'validation.json').open('x') as f:
        json.dump(dict(summary=summary,results=results),f,indent=2)
    print(json.dumps(summary))
    assert summary['all_exact']
