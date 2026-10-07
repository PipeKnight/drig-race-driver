"""Match every captured grid wrapper to full original packed geometry.
Snapshot/source remain private; grid fixture is for isolated comparisons only.
"""
import argparse, collections, hashlib, json, struct
from pathlib import Path
from validate_vcqtc import f32
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser();parser.add_argument('snapshot',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args()
    private=(ROOT/'local').resolve();snapshot_path=args.snapshot.resolve();out=args.output.resolve()
    assert snapshot_path.is_relative_to(private) and out.is_relative_to(private);out.mkdir(exist_ok=False)
    snapshot=json.loads(snapshot_path.read_text(encoding='utf-8-sig'))
    assert snapshot['read_only'] and snapshot['live_numeric_code_exact'] and snapshot['full_grid_requested'] and snapshot['grid_header_rechecked'] and not snapshot['process_suspended']
    blocks={b['name']:b for b in snapshot['blocks']};assert len(blocks)==len(snapshot['blocks'])
    def block(name,length=None):
        b=blocks[name];assert b['success'] and b['length']==b['read'];data=bytes.fromhex(b['hex']);assert len(data)==b['length']
        if length is not None:assert length==len(data)
        return data
    def payload(label,size):return b''.join(block(f'{label}-part_{offset}',min(4096,size-offset)) for offset in range(0,size,4096))
    widths=struct.unpack('<1024I',block('candidate_tile_width_table',4096));exponents=struct.unpack('<1024I',block('candidate_tile_exponent_table',4096))
    bounds=struct.unpack_from('<6f',block('candidate_bounds_header',128));grid_bounds=[bounds[0],bounds[2],bounds[3],bounds[5]]
    fixture=b'GRC1'+struct.pack('<4f',*grid_bounds);cells=[];tile_ids=[]
    for i,(width,exponent) in enumerate(zip(widths,exponents)):
        assert 0<=exponent<=6 and width==2**exponent
        data=payload(f'grid_tile_{i}',width*width*4);ids=list(struct.unpack('<'+'I'*(width*width),data));tile_ids.append(ids);cells.extend(ids)
        fixture+=struct.pack('<II',exponent,width)+data
    (out/'grid.private.bin').write_bytes(fixture)
    source=Path('C:/Program Files (x86)/Steam/steamapps/common/Grid/tracks/europe/donington/route_0/track.jpk')
    manifest=json.loads((ROOT/'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    expected=next(r['steam_sha256'] for r in manifest['resources'] if r['relative_path'].replace('\\','/').lower()=='tracks/europe/donington/route_0/track.jpk');assert digest(source)==expected
    raw=source.read_bytes();source_chunks=[]
    for i in range(struct.unpack_from('<I',raw,8)[0]):
        w=struct.unpack_from('<8I',raw,32+i*32);assert w[1]==w[3];name=raw[w[0]:raw.index(0,w[0])].decode();data=raw[w[2]:w[2]+w[3]]
        signed=struct.unpack_from('<3i',data,24);nt,nv,nm=map(abs,signed);vo,no,to,ro=struct.unpack_from('<4I',data,36);assert vo==52+nm*4 and no==vo+nv*8 and ro==to+nt*4
        source_bounds=struct.unpack_from('<6f',data);scale=[f32(f32(source_bounds[k+3]-source_bounds[k])/(65536 if k==1 else 16777216)) for k in range(3)]
        source_chunks.append(dict(name=name,bounds=data[:24],counts=(nt,nv,nm),vertices=data[vo:no],triangles=data[to:ro],materials=[data[52+k*4:56+k*4] for k in range(nm)],scale=struct.pack('<3f',*scale),offset=data[:12]))
    captured_wrappers={int(s['wrapper'],16) for s in snapshot['grid_shapes']};nonzero=set(cells)-{0};assert captured_wrappers==nonzero
    results=[];tokens=collections.defaultdict(set);vertices=triangles=0
    for s in snapshot['grid_shapes']:
        label='grid_shape_'+s['wrapper'];shape=block(label+'-shape',0x280);header=block(label+'-header',128)
        counts=struct.unpack_from('<3I',header,24);assert counts==(s['triangles'],s['vertices'],s['materials']);assert struct.unpack_from('<I',shape,0x10)[0]==11
        v=payload(label+'-vertices',s['vertices']*8);t=payload(label+'-triangles',s['triangles']*4)
        matches=[c for c in source_chunks if c['bounds']==header[:24] and c['counts']==counts and c['vertices']==v and c['triangles']==t];assert len(matches)==1
        c=matches[0];assert shape[0x250:0x25c]==c['scale'] and shape[0x260:0x26c]==c['offset']
        vertices+=s['vertices'];triangles+=s['triangles'];material_tokens=struct.unpack_from('<'+'I'*s['materials'],header,0x34)
        for code,token in zip(c['materials'],material_tokens):tokens[token].add(code.hex())
        results.append(dict(wrapper=s['wrapper'],source_chunk=c['name'],type_index=11,vertices=s['vertices'],triangles=s['triangles'],materials=s['materials'],packed_vertices_sha256=hashlib.sha256(v).hexdigest(),packed_triangles_sha256=hashlib.sha256(t).hexdigest(),full_geometry_exact=True,scale_offset_xyz_exact=True,transform_pointer=f'{struct.unpack_from("<I",shape,0x170)[0]:08x}',material_codes_hex=[code.hex() for code in c['materials']],material_tokens_hex=[f'{token:08x}' for token in material_tokens]))
    assert len({r['source_chunk'] for r in results})==len(source_chunks)==snapshot['registry_count'];assert digest(source)==expected
    summary=dict(tiles=1024,width_counts=dict(collections.Counter(widths)),exponent_counts=dict(collections.Counter(exponents)),cells=len(cells),nonempty_cells=sum(bool(v) for v in cells),unique_wrappers=len(nonzero),registry_count=snapshot['registry_count'],source_chunks=len(source_chunks),matched=len(results),vertices=vertices,triangles=triangles,successful_blocks=len(blocks),all_shapes_type11=True,all_xyz_scale_offset_exact=True,nonzero_transform_pointers=sum(r['transform_pointer']!='00000000' for r in results))
    report=dict(summary=summary,snapshot_path=str(snapshot_path),snapshot_sha256=digest(snapshot_path),source_path=str(source),source_sha256=expected,grid_fixture_sha256=digest(out/'grid.private.bin'),grid_bounds=grid_bounds,results=results,material_token_codes={f'{k:08x}':sorted(v) for k,v in tokens.items()},limits='Full packed vertices/triangles and XYZ scales/offsets match captured wrappers; nodes/leaves and mechanics record ownership not captured here. Header recheck does not establish a synchronized simulation boundary or original whole-world query behavior.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps(summary))
if __name__=='__main__':main()
