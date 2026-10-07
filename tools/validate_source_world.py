"""Source-only Rust world against whole original query over independently built fixture."""
import argparse, hashlib, json, struct, subprocess
from pathlib import Path
from validate_grid_population import expected
from validate_packed_world import world
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/tracks/europe/donington/route_0/track.jpk')
    manifest=json.loads((ROOT/'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text());recorded=next(r['steam_sha256'] for r in manifest['resources'] if r['relative_path'].replace('\\','/').lower()=='tracks/europe/donington/route_0/track.jpk');assert digest(source)==recorded
    raw=source.read_bytes();names=[];sections=[]
    for i in range(struct.unpack_from('<I',raw,8)[0]):
        w=struct.unpack_from('<8I',raw,32+i*32);assert w[1]==w[3];names.append(raw[w[0]:raw.index(0,w[0])].decode());sections.append(raw[w[2]:w[2]+w[3]])
    _,cells,exponents=expected(names);bounds=struct.unpack_from('<6f',sections[0]);fixture=out/'world.private.bin';fixture.write_bytes(world([bounds[0],bounds[2],bounds[3],bounds[5]],list(zip(exponents,cells)),sections))
    retained=ROOT/'local/behavior/packed-world-20261007-02/captured/input.tsv';requests=out/'input.tsv';requests.write_bytes(retained.read_bytes())
    executions=[]
    for kind,exe,arguments in [('original',ROOT/'local/behavior/packed-world-probe.exe',[ROOT/'local/compatibility/grid-unpacked.exe',fixture,requests,out/'original.tsv']),('native',ROOT/f'target/{args.profile}/grid-track-query.exe',[source,requests,out/'native.tsv'])]:
        result=subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=60);executions.append(dict(kind=kind,path=str(exe),sha256=digest(exe),exit=result.returncode,stdout=result.stdout,stderr=result.stderr))
        if result.returncode:(out/'failure.json').write_text(json.dumps(executions,indent=2));raise RuntimeError('Source world comparison execution failed')
    original=(out/'original.tsv').read_text().splitlines();native=(out/'native.tsv').read_text().splitlines();assert len(original)==len(native)==1967;differences=[];words=hits=0
    for a,b in zip(original,native):
        words+=len(a.split('\t'))-2;hits+=a.startswith('H')
        if a!=b:differences.append(dict(case=a.split('\t')[1],original=a,native=b))
    assert digest(source)==recorded
    report=dict(profile=args.profile,cases=len(original),hits=hits,words=words,exact=len(original)-len(differences),source_sha256=recorded,source_chunks=len(sections),source_nonempty_cells=sum(bool(v) for c in cells for v in c),request_sha256=digest(requests),original_fixture_sha256=digest(fixture),executions=executions,differences=differences,limits='Native reads installed JPK directly, without pointer/cell captures. Whole original query runs over independently rebuilt source fixture. No original whole loader or synchronized live contacts/nonzero transforms/driving claim.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','hits','words','exact']}));assert not differences
if __name__=='__main__':main()
