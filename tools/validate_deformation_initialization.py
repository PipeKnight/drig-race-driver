"""Whole original/Rust reset-layout-mode-zero-edge source and authored proof."""
import argparse
import collections
import json
import struct
import subprocess
from pathlib import Path
import validate_deformation_layout as layout
from prepare_deformation_initialization import configuration, SETTINGS
from compare_source_deformation_layout import candidates, CAPTURES
from summarize_deformation_capture import load
from validate_deformation_volume import guard_checks

ROOT = layout.ROOT
PROBE = ROOT / 'local/behavior/deformation-initialization-probe.exe'
ANSWER = 3 * layout.SIZE + SETTINGS


def cases():
    params, flag = layout.parameters()
    rows, identities = [], {str(layout.IMAGE): layout.digest(layout.IMAGE)}
    for stem in ['lac', 'bmw']:
        settings, info = configuration(stem)
        identities[info['source']] = info['sha256']
        choices, sources = candidates(stem, 'highLOD'); identities.update(sources)
        choice = [c for c in choices if c['name'] == 'highLOD-LOD1_no-aerial']; assert len(choice) == 1
        base = dict(name=stem+'-source-candidate', bounds=[choice[0]['bounds']], parameters=params,
                    token=0, flag=flag, descriptor=bytes(layout.SIZE), settings=settings)
        rows.append(base)
        for change in ['zero_boxes', 'one_box', 'zero_damage', 'zero_crush', 'compression_half', 'damage_double']:
            data = bytearray(settings)
            if change in ['zero_boxes', 'one_box']: struct.pack_into('<I', data, 0x1000, int(change == 'one_box'))
            if change in ['zero_damage', 'zero_crush']:
                for i in range(info['boxes']): struct.pack_into('<f', data, i * 0x80 + (0x5c if change == 'zero_damage' else 0x50), 0.)
            if change in ['compression_half', 'damage_double']:
                struct.pack_into('<f', data, 0x1014 if change == 'compression_half' else 0x1018, .5 if change == 'compression_half' else 2.)
            rows.append(dict(base, name=stem+'-'+change, settings=bytes(data)))
        for scale in [.5, .75, 1.25, 1.5]:
            bounds = list(choice[0]['bounds'])
            for at in [0,1,2,4,5,6]: bounds[at] = layout.f32(bounds[at] * scale)
            rows.append(dict(base, name=stem+f'-bounds{scale}', bounds=[bounds]))
        for shift in [-.25, .25]:
            bounds = list(choice[0]['bounds'])
            for at in [0,1,2,4,5,6]: bounds[at] = layout.f32(bounds[at] + shift)
            rows.append(dict(base, name=stem+f'-shift{shift}', bounds=[bounds]))
        for margin in [0., .1, .25]:
            rows.append(dict(base, name=stem+f'-margin{margin}', parameters=[margin,*params[1:]]))
        for padding in [0x3e800000, 0xbf000000]:
            # Finite coordinates in node399 are consumed by terminal self edges.
            descriptor = bytearray(layout.SIZE)
            for at in range(0,layout.SIZE,4): struct.pack_into('<I',descriptor,at,padding)
            for at in [0xa410,0xa4f8]: struct.pack_into('<I',descriptor,at,0)
            rows.append(dict(base,name=stem+f'-padding{padding:x}',descriptor=bytes(descriptor)))
    return rows, identities


def request(rows):
    out = bytearray(b'GI01'+struct.pack('<I',len(rows)))
    for row in rows: out.extend(layout.request([row])[8:]); out.extend(row['settings'])
    return bytes(out)


def retained(original):
    final = original[8+2*layout.SIZE:8+3*layout.SIZE]
    settings = original[8+3*layout.SIZE:8+ANSWER]
    comparisons = []
    for name, sha in CAPTURES.items():
        path=ROOT/'local/behavior'/name;assert layout.digest(path)==sha
        _, blocks, cars=load(path);car=cars[0]
        prefix=f"car_{car['car']}-"
        def value(key):
            return blocks[prefix+key]
        nodes=value('deformation_grid_nodes')+value('deformation_grid_padding_nodes')
        actual=final[0x2580:0x2580+32000]
        edges=value('deformation_edges')
        differences=collections.Counter(hex((i%80)) for i in range(0,len(nodes),4) if nodes[i:i+4]!=actual[i:i+4])
        captured_settings=value('deformation_radius_thresholds')+value('deformation_strain_settings')+value('deformation_impact_settings')
        # These three adjacent captured regions begin at settings101C.
        item=dict(snapshot=name,sha256=sha,node_pool_bytes=len(nodes),node_pool_exact=nodes==actual,
                  differences_by_node_field=dict(differences),edge_bytes=len(edges),edges_exact=edges==final[:len(edges)],
                  settings_bytes=len(captured_settings),settings_exact=captured_settings==settings[0x101c:0x101c+len(captured_settings)])
        comparisons.append(item)
    assert comparisons[0]['node_pool_exact'] and comparisons[0]['edges_exact'] and comparisons[0]['settings_exact']
    return comparisons


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=['debug','release'],required=True)
    args=parser.parse_args();out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False)
    rows,identities=cases();reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),16):
        batch=rows[start:start+16];input_=out/f'{start}-input.bin';input_.write_bytes(request(batch));answers={};runs={}
        for kind in ['original','repeat','rust']:
            exe=PROBE if kind!='rust' else ROOT/f'target/{args.profile}/grid-deformation-initialization.exe'
            target=out/f'{start}-{kind}.bin';cmd=[str(exe),*([str(layout.IMAGE)] if kind!='rust' else []),str(input_),str(target)]
            run=subprocess.run(cmd,cwd=ROOT,capture_output=True,text=True,timeout=120)
            runs[kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,executable_sha256=layout.digest(exe))
            (out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run.returncode==0 and not run.stderr,runs[kind]
            if kind!='rust':assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run.stdout
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+len(batch)*ANSWER and a==answers['repeat']
        diffs=[dict(case=batch[i//ANSWER]['name'],offset=hex(i%ANSWER),original=a[8+i:12+i].hex(),rust=b[8+i:12+i].hex()) for i in range(0,len(a)-8,4) if a[8+i:12+i]!=b[8+i:12+i]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,differences=diffs,executions=runs)
        (out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item);assert not diffs,diffs[:12]
        if start==0:
            (out/'retained-comparison.json').write_text(json.dumps(retained(a),indent=2))
        for i in range(len(batch)):
            final=a[8+i*ANSWER+2*layout.SIZE:8+i*ANSWER+3*layout.SIZE]
            dims=struct.unpack_from('<3I',final,0xa414);coverage['active_nodes']+=dims[0]*dims[1]*dims[2]
            coverage['positive_markers']+=sum(struct.unpack_from('<f',final,0x25c0+j*80)[0]>0 for j in range(dims[0]*dims[1]*dims[2]))
            coverage['edges']+=struct.unpack_from('<I',final,0xa410)[0]
    settings_at=8+32+layout.SIZE+len(rows[0]['bounds'])*32
    mutations=[('bounds_count',8,33),('settings_token',12,1),('flag',16,256),('nan_margin',20,0x7fc00000),('edge_count',8+32+0xa410,1),('foreign_settings',8+32+0xa4f8,1),('box_count',settings_at+0x1000,33),('nan_matrix',settings_at,0x7fc00000),('zero_size',settings_at+0x40,0),('negative_crush',settings_at+0x50,0xbf800000),('zero_compression',settings_at+0x1014,0),('zero_damage_multiple',settings_at+0x1018,0),('nan_scalar',settings_at+0x103c,0x7fc00000)]
    mutations[0]=(*mutations[0],8) # Original count check is part of framing.
    guards=guard_checks(out,request([rows[0]]),mutations,args.profile,'grid-deformation-initialization',PROBE,8)
    report=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),names=[r['name'] for r in rows],identities=identities,coverage=dict(coverage),reports=reports,guards=guards,
                files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                scope='Complete reset/layout/mode-zero/edge initialization supplied by independent source readers; no original resource-factory selection, dirty rebuild or runtime integration.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
