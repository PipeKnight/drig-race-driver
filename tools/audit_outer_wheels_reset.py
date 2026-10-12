"""Read-only provenance/full-output audit of the selected SC05/HDM join."""
import argparse
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from validate_outer_wheels_reset import advance
from validate_standard_properties import GAME, ROOT, digest
from validate_binxml import entries
from validate_surface_mechanics import encode


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('report', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    assert args.output.resolve().is_relative_to((ROOT / 'local').resolve())
    report = json.loads(args.report.read_text())
    assert report['passed'] and report['cases'] == 24
    for path, expected in report['inputs'].items():
        assert digest(Path(path)) == expected, path
    hdms = {name: data for name, data, raw in entries((GAME / 'raceload.jpk').read_bytes())
            if raw and name in ['standard.hdm', 'openwheeler.hdm']}
    assert len(hdms) == 2
    combinations = set()
    rows = []
    for row in report['rows']:
        key = (row['profile'], row['seed'], row['hdm'], row['active'])
        assert key not in combinations
        combinations.add(key)
        hdm = args.report.parent / row['hdm']
        xml = args.report.parent / (row['hdm'] + '.bin')
        assert hdm.read_bytes() == hdms[row['hdm']]
        assert digest(hdm) == row['hdm_sha256']
        expected_xml = encode(ET.fromstring(re.sub(rb'<![^>]*>', b'', hdms[row['hdm']])))
        assert xml.read_bytes() == expected_xml and digest(xml) == row['xml_sha256']
        cold = Path(row['cold'])
        assert str(cold) in report['inputs']
        before = cold.read_bytes()
        outputs = []
        assert set(row['outputs']) == set(row['executions']) == {'original', 'repeat', 'rust'}
        for kind, output in row['outputs'].items():
            path = Path(output['path'])
            assert digest(path) == output['sha256']
            body = path.read_bytes()
            assert len(body) == len(before) == 0x21000
            # The selected reset may change only the mutable owner. In particular
            # all four wheel boxes and the two canonical padding bytes persist.
            assert body[:0xcdf0] == before[:0xcdf0]
            assert body[0xd250:] == before[0xd250:]
            assert body[0xce50:0xcf10] == before[0xce50:0xcf10]
            execution = row['executions'][kind]
            assert execution['exit'] == 0 and not execution['stderr']
            assert f'random_state={advance(row["seed"], 24):08x}' in execution['stdout']
            if kind != 'rust':
                assert 'numeric_adapters=1 restored=1 error=0' in execution['stdout']
            outputs.append(body)
        assert outputs[0] == outputs[1] == outputs[2]
        rows.append(dict(profile=row['profile'], seed=row['seed'], hdm=row['hdm'],
                         active=row['active'], complete_bytes=len(outputs[0]),
                         sha256=row['outputs']['original']['sha256']))
    expected = {(profile, seed, hdm, active) for profile in ['debug', 'release']
                for seed in [200679, 0, 0xffffffff] for hdm in hdms for active in ['0', '1']}
    assert combinations == expected and sum(row['complete_bytes'] for row in rows) == report['complete_bytes']
    result = dict(passed=True, cases=len(rows), checked_outputs=3 * len(rows),
                  report_sha256=digest(args.report), auditor_sha256=digest(Path(__file__)),
                  inputs=report['inputs'], rows=rows,
                  scope='Read-only current hashes, source HDM/XML reconstruction, all complete '
                        'outputs, mutable-owner containment, four wheel regions and 24-draw RNG; '
                        'not a second independent original-function oracle or whole factory proof.')
    with args.output.open('x') as output:
        json.dump(result, output, indent=2)
    print(json.dumps({k: v for k, v in result.items() if k not in ['inputs', 'rows']}))


if __name__ == '__main__':
    main()
