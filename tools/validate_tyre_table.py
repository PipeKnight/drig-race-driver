"""Whole original tyre CSV loader/tokenizer versus independent native reader.

The isolated original uses host allocator methods and CRT numeric/handle adapters.
"""
import argparse
import csv
import hashlib
import io
import json
import random
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ROW_CODES = ['LIGR','MDGR','HEGR','DUGR','LPGR','PEGR','WTGR','INGR','MUDD','SMTM','ROTM','WTTM','INTM']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def code(value):
    return int.from_bytes(value[:4].encode('ascii'),'little') if len(value)>=4 else 0xffffffff


def word(value):
    return struct.unpack('<I',struct.pack('<f',value))[0]


def expected(raw):
    records = list(csv.reader(io.StringIO(raw.decode('ascii'),newline='')))
    columns = []
    for value in records[0][2:]:
        if not value:break
        columns.append(code(value))
    rows = [None]*13
    if columns:
        for record in records[2:]:
            if not record or not record[0]:break
            token = record[0][:4]
            if len(token)<4 or token not in ROW_CODES:continue
            slot = ROW_CODES.index(token)
            if rows[slot] is not None:continue
            values = record[2:2+len(columns)]+['']*max(0,len(columns)-max(0,len(record)-2))
            rows[slot] = [word(float(v or '0')*struct.unpack('<f',struct.pack('<I',0x3c23d70a))[0]) for v in values]
    lines = [f'T\t{int(bool(columns)):08x}\t{len(columns):08x}']
    lines += [f'K\t{i:08x}\t{k:08x}' for i,k in enumerate(columns)]
    lines += [f'R\t{i:08x}\t{int(row is not None):08x}'+('' if row is None else ''.join(f'\t{v:08x}' for v in row)) for i,row in enumerate(rows)]
    return lines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    parser.add_argument('--original-crt-check',action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    source = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/cars/tyreGrip.csv')
    source_hash = '19383cb996b1d534fd3e4643ebcdb99edb8071ffd85ec21a469a30214e672a5c'
    assert digest(source)==source_hash
    rng = random.Random(106)
    datasets = [('installed',source)]
    for i in range(40):
        count = i%11
        keys = ['TEST_more','XX','MUD ','MUD\0','DEFA','TEST','ROAD','TARM','DIRT','GRAS'][:count]
        # Embedded NUL is unsafe framing, so use distinct printable padding controls.
        keys = ['MUD_' if k=='MUD\0' else k for k in keys]
        records = [['header','description',*keys,'','ignored'],['labels','ignored']]
        order = ROW_CODES.copy();rng.shuffle(order)
        for j, token in enumerate(order):
            if i%3==0 and j%2:continue
            values = ['1.0000000596046448' if j==0 else str(rng.uniform(-200,200)) for _ in keys]
            if i%4==0 and j%3==0:values = values[:len(values)//2]
            if i%5==0:values = ['' if k%2 else v for k,v in enumerate(values)]
            records.append([token+'_suffix','ignored',*values])
            if j%3==0:records.append([token,'duplicate',*['9999']*len(keys)])
            if j==4:records.append(['ZZZZ','unknown',*['8888']*len(keys)])
            if i%7==0 and j==6:records.append([])
        records += [[],['LIGR','after-empty',*['7777']*len(keys)]]
        buffer = io.StringIO(newline='');csv.writer(buffer,lineterminator='\r\n').writerows(records)
        path = out/f'authored-{i:02d}.csv';path.write_bytes(buffer.getvalue().encode('ascii'))
        datasets.append((f'authored-{i:02d}',path))
    probe = ROOT/'local/behavior/tyre-table-probe.exe'
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions,comparisons = [],[]
    if args.original_crt_check:
        run = subprocess.run([str(probe),str(image),str(source),str(out/'original-crt.tsv'),'original'],
                             cwd=ROOT,capture_output=True,text=True,timeout=30)
        diagnostic = dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,sha256=digest(probe),
                          limits='Diagnostic only; no installed-game compatibility claim.')
        (out/'original-crt.json').write_text(json.dumps(diagnostic,indent=2))
    for label,path in datasets:
        reference = expected(path.read_bytes())
        for kind,exe,arguments in [
            ('original',probe,[image,path,out/f'{label}-original.tsv']),
            ('native',ROOT/f'target/{args.profile}/grid-tyre-table.exe',[path,out/f'{label}-native.tsv']),
        ]:
            run = subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            executions.append(dict(label=label,kind=kind,sha256=digest(exe),exit=run.returncode,stdout=run.stdout,stderr=run.stderr))
            if run.returncode:
                (out/'failure.json').write_text(json.dumps(executions,indent=2))
                raise RuntimeError('Tyre table run failed; evidence retained')
        original = (out/f'{label}-original.tsv').read_text().splitlines()
        native = (out/f'{label}-native.tsv').read_text().splitlines()
        differences = [] if original==native==reference else [dict(original=original,native=native,independent=reference)]
        words = sum(len(line.split('\t'))-1 for line in reference)
        comparisons.append(dict(label=label,input_sha256=digest(path),report_words=words,
                                exact=not differences,differences=differences))
    assert digest(source)==source_hash
    result = dict(profile=args.profile,source_sha256=source_hash,executions=executions,comparisons=comparisons,
                  datasets=len(datasets),exact_datasets=sum(c['exact'] for c in comparisons),
                  report_words=sum(c['report_words'] for c in comparisons),
                  limits='Whole original 9c2dd0/9917f0/991240/991510/cleanup with supplied host allocator methods. '
                         'Numeric strtod and CRT generation adapters; no original numeric CRT or handle-generation claim. '
                         'Private CSV globals writable/restored; original loader/tokenizer code unmodified. '
                         'Not file/resource manager loading, body row selection or full tyre forces.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:v for k,v in result.items() if k in ('profile','datasets','exact_datasets','report_words')}))
    assert all(c['exact'] for c in comparisons)


if __name__=='__main__':
    main()
