"""Match bounded live collision headers/scales to original JPK chunk bytes."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
from validate_vcqtc import f32

if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('game_root',type=Path)
    parser.add_argument('snapshot',type=Path)
    args=parser.parse_args()
    root=Path(__file__).resolve().parent.parent
    private=(root/'local').resolve()
    snapshot_path=args.snapshot.resolve()
    assert snapshot_path.is_relative_to(private)
    snapshot=json.loads(snapshot_path.read_text(encoding='utf-8-sig'))
    assert snapshot['read_only'] and snapshot['live_numeric_code_exact']
    source=args.game_root/'tracks/europe/donington/route_0/track.jpk'
    raw=source.read_bytes()
    count=struct.unpack_from('<I',raw,8)[0]
    chunks=[]
    for i in range(count):
        words=struct.unpack_from('<8I',raw,32+i*32)
        name=raw[words[0]:raw.index(0,words[0])].decode()
        data=raw[words[2]:words[2]+words[3]]
        bounds=struct.unpack_from('<6f',data)
        signed=struct.unpack_from('<3i',data,24)
        scale=[f32(f32(bounds[i+3]-bounds[i])/(65536 if i==1 else 16777216)) for i in range(3)]
        vertex_offset,_,triangle_offset,_=struct.unpack_from('<4I',data,36)
        chunks.append(dict(name=name,bounds_bytes=data[:24],signed=signed,
                           vertex_prefix=data[vertex_offset:vertex_offset+64],triangle_prefix=data[triangle_offset:triangle_offset+64],
                           scale_bytes=struct.pack('<3f',*scale),offset_bytes=data[:12]))
    blocks={b['name']:bytes.fromhex(b['hex']) for b in snapshot['blocks'] if b['success'] and b['read']==b['length']}
    results=[]
    for candidate in snapshot['candidates']:
        header=blocks.get('packed_header_'+candidate['header'])
        shape=blocks.get('shape_'+candidate['shape'])
        if header is None or shape is None:
            results.append(dict(candidate=candidate,matched=False,error='missing captured header/shape'))
            continue
        signed=struct.unpack_from('<3i',header,24)
        vertices=blocks.get('vertices_'+candidate['header'])
        triangles=blocks.get('triangles_'+candidate['header'])
        matches=[c for c in chunks if c['bounds_bytes']==header[:24] and tuple(map(abs,c['signed']))==tuple(map(abs,signed))
                 and c['vertex_prefix']==vertices and c['triangle_prefix']==triangles]
        if len(matches)!=1:
            results.append(dict(candidate=candidate,matched=False,runtime_signed_counts=signed,matches=len(matches)))
            continue
        chunk=matches[0]
        scale_exact=shape[0x250:0x25c]==chunk['scale_bytes']
        offset_exact=shape[0x260:0x26c]==chunk['offset_bytes']
        results.append(dict(candidate=candidate,source_chunk=chunk['name'],matched=scale_exact and offset_exact,
                            runtime_signed_counts=signed,source_signed_counts=chunk['signed'],scale_exact=scale_exact,
                            offset_exact=offset_exact,geometry_prefix_exact=True,runtime_scale=struct.unpack_from('<3f',shape,0x250),
                            runtime_offset=struct.unpack_from('<3f',shape,0x260)))
    summary=dict(registry_count=snapshot['registry_count'],source_chunks=count,sampled=len(results),
                 matched=sum(r['matched'] for r in results),types=sorted({r['candidate']['type_index'] for r in results}),
                 live_numeric_code_exact=snapshot['live_numeric_code_exact'],process_suspended=snapshot['process_suspended'])
    output=snapshot_path.with_name(snapshot_path.stem+'-validation.json')
    with output.open('x') as f:
        json.dump(dict(summary=summary,track_sha256=hashlib.sha256(raw).hexdigest(),
                       snapshot_sha256=hashlib.sha256(snapshot_path.read_bytes()).hexdigest(),results=results),f,indent=2)
    print(json.dumps(summary))
    assert summary['matched']==summary['sampled'] and summary['sampled']>0
