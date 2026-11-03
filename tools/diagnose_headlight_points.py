"""Report source headlight point selection from saved per-model data (CPU only).

Reuses the existing binxml/PSSG readers; --self-test needs no game assets.
For purchased data pass an absolute --game-root and repeat --model STEM.
--output must be a new JSON file under ignored local/. Canonical LOD0 point
selection does not prove runtime slot ordering, world pose or rendered optics.
See docs/TRACK_ENVIRONMENT.md, source headlight preparation diagnostic.
"""
import argparse
import collections
import copy
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import sys
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
REPO = next(p for p in Path(__file__).resolve().parents
            if (p / 'tools/survey_pssg_payloads.py').is_file())
sys.path.insert(0, str(REPO / 'tools'))
from survey_pssg_payloads import survey
from validate_binxml import decode

F32 = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
DEFAULT_PAIR = [[.75, F32(.6), F32(2.2)], [-.75, F32(.6), F32(2.2)]]
PAIRS = [('glass', ['x0_light_glass_fl', 'x0_light_glass_fr'], [108, 114]),
         ('pod', ['x0_light_pod_fl', 'x0_light_pod_fr'], [107, 113])]
QUALIFICATION = {
    'scope': 'canonical LOD0 saved-data diagnostic; not original runtime execution',
    'runtime_first_of_eight_slot_to_LOD_order_proven': False,
    'current_Car_scene_model_offset_join_proven': False,
    'world_pose_evaluated': False,
    'cone_range_intensity_or_physical_projector_count_proven': False,
    'numeric_policy': 'finite strict Python float->f32 attributes, f64 midpoint; not CRT/SM3/f32 parity',
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def validate_offset_rows(rows):
    """Keep consumed-attribute ambiguity visible before ElementTree collapses it."""
    parts = {row[1] for row in rows
             if row[0] == 'E' and row[2] == b'PARTS'.hex()}
    consumed = {key.encode().hex() for key in
                ('m_light_ray_z_offset', 'm_light_ray_y_offset')}
    seen = set()
    for row in rows:
        if row[0] != 'A' or row[1] not in parts or row[2] not in consumed:
            continue
        key = (row[1], row[2])
        if key in seen:
            raise ValueError('ambiguous duplicate consumed PARTS offset attribute')
        seen.add(key)


def offsets(root):
    parts = list(root.iter('PARTS'))
    if len(parts) > 1:
        raise ValueError('ambiguous PARTS nodes')
    result = {'PARTS_present': bool(parts), 'fields': {}}
    for axis, key, default in [('z', 'm_light_ray_z_offset', F32(.2)),
                               ('y', 'm_light_ray_y_offset', 0.)]:
        text = parts[0].get(key) if parts else None
        if text is not None:
            try:
                value = F32(float(text))
            except (ValueError, OverflowError) as exc:
                raise ValueError(f'invalid finite f32 {key}') from exc
            if not math.isfinite(value):
                raise ValueError(f'nonfinite {key}')
            origin = 'saved-attribute'
        else:
            value = default if parts else 0.
            origin = 'PARTS-present-source-default' if parts else 'PARTS-absent-source-zero-initialization'
        result['fields'][axis] = {'name': key, 'value': value, 'saved_text': text, 'origin': origin}
    return result


def validate_selected(node):
    bounds, matrix = node['bounds'], node['transform']
    if len(bounds) != 6 or len(matrix) != 16 or not all(math.isfinite(v) for v in bounds + matrix):
        raise ValueError('selected node has invalid finite bounds/transform')
    if any(bounds[i] > bounds[i + 3] for i in range(3)):
        raise ValueError('selected node bounds are inverted')
    return [(bounds[i] + bounds[i + 3]) * .5 + matrix[i + 12] for i in range(3)]


def prepare(nodes, part_offsets):
    """Exact atomic pair/default rule, consumed by both reports and controls."""
    selected = []
    policy = 'source-original-default-pair'
    for kind, names, codes in PAIRS:
        if all(nodes.get(name) for name in names):
            for name, code in zip(names, codes):
                candidates = nodes[name]
                if len(candidates) != 1:
                    raise ValueError(f'ambiguous selected canonical node {name}')
                node = candidates[0]
                selected.append({'name': name, 'part_code': code, **node,
                                 'center_plus_translation_xyz': validate_selected(node)})
            policy = f'canonical-LOD0-{kind}-pair'
            break
    points = [n['center_plus_translation_xyz'] for n in selected] or copy.deepcopy(DEFAULT_PAIR)
    raw_c = [(points[0][i] + points[1][i]) * .5 for i in range(3)]
    source_c = raw_c.copy()
    source_c[1] += part_offsets['fields']['y']['value']
    source_c[2] += part_offsets['fields']['z']['value']
    if not all(math.isfinite(v) for v in raw_c + source_c):
        raise ValueError('nonfinite prepared source point')
    return {'selection': policy, 'selected_nodes': selected, 'selected_or_default_points_xyz': points,
            'missing_canonical_names': [n for _, names, _ in PAIRS for n in names if not nodes.get(n)],
            'offsets': part_offsets, 'raw_C_midpoint_xyz': raw_c,
            'C_plus_NED_offset_xyz': source_c, 'qualification': QUALIFICATION}


def saved_nodes(tree, raw):
    """Reuse inventory03's schema selection, retaining saved bounds rather than vertex AABBs."""
    nodes = tree['nodes']
    children = collections.defaultdict(list)
    for i, node in enumerate(nodes):
        children[node['parent']].append(i)
    def name(node):
        return node['attributes'].get('id', {}).get('string')
    bundles = [i for i, n in enumerate(nodes)
               if n['name'] == 'MATRIXPALETTEBUNDLENODE' and name(n) == 'LOD0_']
    if len(bundles) != 1:
        raise ValueError('expected one canonical LOD0_ bundle')
    wanted = {n for _, names, _ in PAIRS for n in names}
    result = collections.defaultdict(list)
    for i in children[bundles[0]]:
        node = nodes[i]
        if node['name'] != 'MATRIXPALETTEJOINTNODE' or name(node) not in wanted:
            continue
        payloads = {}
        for kind, count in [('BOUNDINGBOX', 6), ('TRANSFORM', 16)]:
            payload = [nodes[c] for c in children[i] if nodes[c]['name'] == kind]
            if len(payload) != 1 or payload[0]['payload_size'] != count * 4:
                # Preserve malformed candidates until AFTER atomic selection.
                payloads[kind] = []
            else:
                payloads[kind] = list(struct.unpack_from('>' + str(count) + 'f', raw,
                                                       payload[0]['payload_offset']))
        result[name(node)].append({'node_offset': node['offset'], 'bounds': payloads['BOUNDINGBOX'],
                                   'transform': payloads['TRANSFORM'],
                                   'joint_id': node['attributes'].get('jointID', {}).get('u32be')})
    return dict(result)


def report_model(game_root, model):
    if not re.fullmatch(r'[a-z0-9]{2,8}', model):
        raise ValueError('model must match existing CarModel stem syntax')
    ned = game_root / 'cars' / model / (model + '.ned')
    mesh = game_root / 'cars' / model / (model + '_highLOD.pssg')
    if ned.stat().st_size > 32 * 1024 * 1024 or mesh.stat().st_size > 256 * 1024 * 1024:
        raise ValueError('selected resource exceeds diagnostic bound')
    ned_raw = ned.read_bytes()
    rows, decoded, _ = decode(ned_raw)
    validate_offset_rows(rows)
    ned_offsets = offsets(decoded)
    tree = survey(mesh)
    mesh_raw = mesh.read_bytes()
    if digest(mesh_raw) != tree['sha256']:
        raise ValueError('mesh changed during survey')
    descriptor = prepare(saved_nodes(tree, mesh_raw), ned_offsets)
    if digest(ned.read_bytes()) != digest(ned_raw) or digest(mesh.read_bytes()) != tree['sha256']:
        raise ValueError('selected source changed during read')
    return {'model': model, 'inputs': [
        {'path': str(ned), 'sha256': digest(ned_raw), 'bytes': len(ned_raw)},
        {'path': str(mesh), 'sha256': tree['sha256'], 'bytes': len(mesh_raw)}],
        'descriptor': descriptor, 'input_stability': 'PASS'}


def self_test():
    """Authored controls call the exact consumed preparation, never a copied formula."""
    def node(lo, hi, translation=(0., 0., 0.)):
        m = [1. if i in [0, 5, 10, 15] else 0. for i in range(16)]
        m[12:15] = translation
        return {'bounds': list(lo) + list(hi), 'transform': m}
    glass = {'x0_light_glass_fl': [node([0, 0, 0], [2, 4, 6])],
             'x0_light_glass_fr': [node([4, 4, 4], [6, 8, 10])]}
    pod = {'x0_light_pod_fl': [node([9, 19, 29], [11, 21, 31])],
           'x0_light_pod_fr': [node([13, 23, 33], [15, 25, 35])]}
    present = offsets(ET.fromstring('<CAR><PARTS/></CAR>'))
    absent = offsets(ET.fromstring('<CAR/>'))
    passed = []
    both = prepare(glass | pod, present)
    assert both['selection'] == 'canonical-LOD0-glass-pair' and both['raw_C_midpoint_xyz'] == [3, 4, 5]
    passed.append('complete-glass-pair-preferred')
    missing_glass = dict(glass | pod); del missing_glass['x0_light_glass_fr']
    selected = prepare(missing_glass, present)
    assert selected['selection'] == 'canonical-LOD0-pod-pair' and selected['raw_C_midpoint_xyz'] == [12, 22, 32]
    passed.append('one-glass-missing-selects-whole-pod-pair')
    del missing_glass['x0_light_pod_fr']
    selected = prepare(missing_glass, present)
    assert selected['selection'] == 'source-original-default-pair'
    assert selected['selected_or_default_points_xyz'] == DEFAULT_PAIR
    assert not selected['selected_nodes']
    passed.append('incomplete-fallback-retains-whole-original-default-pair')
    assert prepare({}, present)['selected_or_default_points_xyz'] == DEFAULT_PAIR
    passed.append('both-fallback-parts-absent-use-original-default-pair')
    zero = prepare(glass, absent)
    assert zero['C_plus_NED_offset_xyz'] == [3, 4, 5]
    assert all(f['origin'] == 'PARTS-absent-source-zero-initialization' for f in absent['fields'].values())
    assert present['fields']['z']['value'] == F32(.2) and absent['fields']['z']['value'] == 0
    passed.append('absent-PARTS-is-zero-initialization')
    explicit = offsets(ET.fromstring('<CAR><PARTS m_light_ray_z_offset="0" m_light_ray_y_offset=".5"/></CAR>'))
    assert explicit['fields']['z']['value'] == 0 and explicit['fields']['z']['origin'] == 'saved-attribute'
    assert present['fields']['z']['origin'] == 'PARTS-present-source-default'
    assert prepare(glass, explicit)['C_plus_NED_offset_xyz'] == [3, 4.5, 5]
    passed.append('missing-attribute-default-distinct-from-explicit-zero')
    try: offsets(ET.fromstring('<CAR><PARTS m_light_ray_z_offset="nan"/></CAR>'))
    except ValueError: pass
    else: raise AssertionError('nonfinite selected NED offset must reject')
    passed.append('nonfinite-NED-offset-rejects')
    for case, bounds in [('nonfinite', [math.nan, 0, 0, 2, 4, 6]), ('inverted', [3, 0, 0, 2, 4, 6])]:
        bad = copy.deepcopy(glass | pod); bad['x0_light_glass_fl'][0]['bounds'] = bounds
        try: prepare(bad, present)
        except ValueError: pass
        else: raise AssertionError('malformed SELECTED glass must reject, never fall through to valid pods')
        passed.append(case + '-selected-bounds-reject-no-semantic-fallback')
    bad_unused = copy.deepcopy(glass | pod); bad_unused['x0_light_pod_fl'][0]['bounds'] = []
    assert prepare(bad_unused, present)['selection'] == 'canonical-LOD0-glass-pair'
    passed.append('unused-pod-data-does-not-block-selected-glass')
    rotated = copy.deepcopy(glass)
    rotated['x0_light_glass_fl'][0]['transform'] = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 2, 4, 6, 1]
    assert prepare(rotated, absent)['raw_C_midpoint_xyz'] == [4, 6, 8]
    passed.append('source-rule-adds-translation-without-invented-bound-rotation')
    # Binary XML preserves duplicate attributes in rows, unlike ElementTree.
    # Only the consumed offsets are ambiguous for this diagnostic.
    row = ['E', '0', b'PARTS'.hex(), '', '', '0', '0']
    for axis in ('z', 'y'):
        attribute = ['A', '0', ('m_light_ray_' + axis + '_offset').encode().hex(), b'0'.hex()]
        try: validate_offset_rows([row, attribute, attribute])
        except ValueError: pass
        else: raise AssertionError('duplicate consumed offset must reject')
        passed.append('duplicate-consumed-' + axis + '-offset-rejects')
    attribute = ['A', '0', b'unrelated'.hex(), b'0'.hex()]
    validate_offset_rows([row, attribute, attribute])
    passed.append('duplicate-unrelated-attribute-not-reinterpreted')
    return {'status': 'PASS', 'count': len(passed), 'controls': passed,
            'scope': 'authored CPU policy/domain controls, not original execution or renderer validation'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--game-root', type=Path)
    parser.add_argument('--model', action='append', default=[], help='repeatable generic CarModel stem')
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        parser.error('optimized Python is unsupported: diagnostic and reader assertions must execute')
    output = args.output.resolve()
    if not output.is_relative_to((REPO / 'local').resolve()):
        parser.error('output must remain under ignored local/')
    if output.exists():
        raise FileExistsError(output)
    if len(args.model) > 32 or len(set(args.model)) != len(args.model):
        parser.error('need at most32 distinct selected models')
    if bool(args.model) != bool(args.game_root):
        parser.error('--game-root and one or more --model must be supplied together')
    if not args.model and not args.self_test:
        parser.error('select models or --self-test')
    if args.game_root and (not args.game_root.is_absolute() or not args.game_root.is_dir()):
        parser.error('--game-root must be an existing absolute resource root')
    report = {'status': 'COMPLETED_CPU_DATA_DIAGNOSTIC', 'qualification': QUALIFICATION,
              'self_test': self_test() if args.self_test else 'NOT_REQUESTED',
              'models': [report_model(args.game_root.resolve(), m) for m in args.model],
              'readers': {p: digest((REPO / p).read_bytes()) for p in
                          ['tools/survey_pssg_payloads.py', 'tools/validate_binxml.py']},
              'script_sha256': digest(Path(__file__).read_bytes())}
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('x', encoding='utf-8', newline='\n') as file:
        json.dump(report, file, indent=2, allow_nan=False)
        file.write('\n')
    print(json.dumps({'status': report['status'], 'controls': report['self_test'],
                      'models': [{'model': row['model'], 'selection': row['descriptor']['selection'],
                                  'point': row['descriptor']['C_plus_NED_offset_xyz']} for row in report['models']],
                      'output': str(output)}))


if __name__ == '__main__':
    main()
