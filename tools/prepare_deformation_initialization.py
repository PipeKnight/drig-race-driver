"""Prepare a bounded source-derived player candidate for whole original setup.

Selected graphics bounds are still an explicit investigative policy. This does
not execute the original XML/resource factory or port the node-limit algorithm.
"""
import argparse
import json
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode
from inspect_installation import inspect_pe
import validate_deformation_layout as layout
from compare_source_deformation_layout import candidates, STEAM

ROOT = layout.ROOT
SETTINGS = 0x1054


def configuration():
    raw, pe = layout.IMAGE.read_bytes(), inspect_pe(layout.IMAGE)
    def static(address):
        for s in pe['sections']:
            at = address - 0x400000 - s['rva']
            if 0 <= at <= s['raw_size'] - 4:
                return struct.unpack_from('<f', raw, s['raw_offset'] + at)[0]
        raise AssertionError(hex(address))
    path = STEAM / 'cars/lac/lac.ned'
    tree = decode(path.read_bytes())[1]
    fems = [e for e in tree.iter() if e.tag == 'FEMS']
    assert len(fems) == 1
    fems = fems[0]
    geometries = [e for e in fems if e.tag == 'GEOMETRY' and e.get('id') == 'strength_main']
    assert len(geometries) == 1 and len(geometries[0]) <= 32
    out = bytearray(SETTINGS)
    def put(at, x):
        struct.pack_into('<f', out, at, layout.f32(x))
    identity = [1. if i in (0, 5, 10, 15) else 0. for i in range(16)]
    for i, box in enumerate(geometries[0]):
        assert box.tag == 'BOX'
        matrix = [layout.f32(float(v)) for v in box.get('transform', ' '.join(map(str, identity))).split()]
        size = [layout.f32(float(v)) for v in box.get('size', ' '.join([str(static(0xcafcd4))] * 3)).split()]
        assert len(matrix) == 16 and len(size) == 3
        struct.pack_into('<16f', out, i * 0x80, *matrix)
        struct.pack_into('<4f', out, i * 0x80 + 0x40, *size, 0.)
        for at, name, default in [(0x50, 'crush', static(0xcafcd4)), (0x54, 'crY', 1.),
                                  (0x58, 'crZ', 1.), (0x5c, 'damage', 1.)]:
            put(i * 0x80 + at, float(box.get(name, str(default))))
    struct.pack_into('<I', out, 0x1000, len(geometries[0]))
    for at in [0x1014, 0x1018, 0x1030, 0x103c, 0x1040]:
        put(at, 1.)
    for at, address in [(0x101c, 0xcf7f14), (0x1020, 0xcf7f14), (0x1028, 0xcafbd8),
                        (0x102c, 0xcafe60), (0x1034, 0xcb0db0), (0x1038, 0xcb22b4),
                        (0x1048, 0xcb870c), (0x104c, 0xcb0f24)]:
        put(at, static(address))
    mappings = [(0x1018, 'm_damage_multiple'), (0x1014, 'max_compression_factor'),
                (0x1040, 'm_wing_pushout_multiple'), (0x103c, 'm_bonnet_uppush_multiple'),
                (0x1044, 'm_bonnet_uppush_z'), (0x1048, 'm_deformation_linear_threshold'),
                (0x104c, 'm_deformation_multiple_slow'), (0x1050, 'm_deformation_multiple_very_slow'),
                (0x101c, 'm_wheel_burst_dist_f'), (0x1020, 'm_wheel_burst_dist_r'),
                (0x102c, 'm_normalMapDistance'), (0x102c, 'm_normalMapDistanceFull'),
                (0x1024, 'm_normalMapDistanceStart'), (0x1030, 'm_scratchMapVelThreshold'),
                (0x1034, 'm_scratchMapVelScale'), (0x1038, 'm_scratchMapImpactScale')]
    for at, name in mappings:
        if name in fems.attrib:
            put(at, float(fems.get(name)))
    if float(fems.get('m_enableEditorExtraParameters', '0')) < .5:
        put(0x1014, 1.); put(0x1018, 1.)
    put(0x1040, struct.unpack_from('<f', out, 0x1040)[0] * .5)
    span = layout.f32(struct.unpack_from('<f', out, 0x102c)[0] - struct.unpack_from('<f', out, 0x1024)[0])
    assert span > 0
    put(0x1028, 1. / span)
    return bytes(out), dict(source=str(path), sha256=layout.digest(path), boxes=len(geometries[0]),
                            scope='Independent source reader guided by aligned loader fields; original tokenizer/loader unexecuted')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    choices, identities = candidates('lac', 'highLOD')
    selected = [c for c in choices if c['name'] == 'highLOD-LOD1_no-aerial']
    assert len(selected) == 1
    settings, source_info = configuration()
    p, flag = layout.parameters()
    row = dict(name='player-source-candidate', bounds=[selected[0]['bounds']], parameters=p,
               token=0, flag=flag, descriptor=bytes(layout.SIZE))
    request = b'GI01' + layout.request([row])[4:] + settings
    input_ = out / 'input.bin'
    input_.write_bytes(request)
    runs, results = {}, {}
    probe = ROOT / 'local/behavior/deformation-initialization-probe.exe'
    for kind in ['original', 'repeat']:
        target = out / (kind + '.bin')
        run = subprocess.run([str(probe), str(layout.IMAGE), str(input_), str(target)],
                             cwd=ROOT, capture_output=True, text=True, timeout=120)
        runs[kind] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr,
                          executable_sha256=layout.digest(probe))
        (out / 'execution.json').write_text(json.dumps(runs, indent=2))
        assert run.returncode == 0 and not run.stderr, runs[kind]
        assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run.stdout
        results[kind] = target.read_bytes()
    assert results['original'] == results['repeat']
    data = results['original']
    assert len(data) == 8 + 3 * layout.SIZE + SETTINGS
    final = data[8 + 2 * layout.SIZE:8 + 3 * layout.SIZE]
    report = dict(executions=runs, inputs=identities, source_configuration=source_info,
                  selected_candidate=selected[0], input_sha256=layout.digest(input_),
                  output_sha256=layout.digest(out / 'original.bin'), repeat_exact=True,
                  dimensions=struct.unpack_from('<3I', final, 0xa414),
                  edge_count=struct.unpack_from('<I', final, 0xa410)[0],
                  positive_node_markers=sum(struct.unpack_from('<f', final, 0x25c0 + i * 80)[0] > 0 for i in range(400)),
                  settings_unchanged=data[-SETTINGS:] == settings,
                  scope='Whole original reset/layout/mode-zero/edges on independent source candidate. '
                  'No Rust node-limit/edge initializer, original resource factory or prototype integration.')
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['dimensions', 'edge_count', 'positive_node_markers', 'settings_unchanged', 'repeat_exact']}))


if __name__ == '__main__':
    main()
