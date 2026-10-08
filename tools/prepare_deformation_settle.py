"""Execute isolated original settling against a bounded retained node pool."""
import argparse
import json
import struct
import subprocess
from pathlib import Path
from validate_steering_control import digest, word, PE_SHA
from summarize_deformation_capture import load

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = 'a62bfd35a8428f865bd3869be0b2a6df66831f878d00f0fbdceda637d35c1b0e'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('snapshot', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve())
    capture, blocks, summary = load(args.snapshot)
    out.mkdir(exist_ok=False)
    image, probe = ROOT / 'local/compatibility/grid-unpacked.exe', ROOT / 'local/behavior/deformation-settle-probe.exe'
    assert digest(image) == PE_SHA and digest(probe) == PROBE_SHA
    inputs, prior = [], {}
    for row in summary:
        i = row['car']
        prefix = f'car_{i}-'
        descriptor = blocks[prefix + 'deformation_grid_descriptor']
        nodes = blocks[prefix + 'deformation_grid_nodes'] + blocks.get(prefix + 'deformation_grid_padding_nodes', b'')
        edges = list(struct.iter_unpack('<hhf', blocks[prefix + 'deformation_edges']))
        values = list(map(float, row['dimensions'])) + [struct.unpack_from('<f', descriptor, 12)[0], float(row['mode'])]
        values += list(row['strain_settings']) + [float(len(edges))]
        values += [v for offset in range(0, len(nodes), 80) for v in struct.unpack_from('<12f', nodes, offset)]
        values += [v for edge in edges for v in edge]
        name = f'car_{i}'
        inputs.append(name + '\t' + '\t'.join(f'{word(v):08x}' for v in values) + '\n')
        prior[name] = [f'{word(v):08x}' for offset in range(0, len(nodes), 80) for v in struct.unpack_from('<4f', nodes, offset)]
        prior[name] += [f'{word(edge[2]):08x}' for edge in edges]
    request, target = out / 'input.tsv', out / 'original.tsv'
    request.write_text(''.join(inputs))
    run = subprocess.run([str(probe), str(image), str(request), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
    report = dict(snapshot_sha256=digest(args.snapshot), analysis_sha256=PE_SHA, probe_sha256=PROBE_SHA,
                  input_sha256=digest(request), cases=len(inputs), execution=dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr),
                  limits='Whole original9a26f0 including real9a2110, supplied400-node pool/graph/settings, codeRX and restored privateE58000data page. No game writes, stubs or same-tick claim.')
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    assert run.returncode == 0, report['execution']
    results = {}
    for line in target.read_text().splitlines():
        cells = line.split('\t')
        assert cells[0] == 'P' and cells[1] not in results
        results[cells[1]] = cells[2:]
    assert results.keys() == prior.keys()
    assert all(len(results[name]) == len(prior[name]) for name in prior)
    differences = [dict(case=name, field=i, incoming=a, replay=b) for name in results
                   for i, (a, b) in enumerate(zip(prior[name], results[name])) if a != b]
    report.update(output_sha256=digest(target), retained_fields=sum(map(len, prior.values())), retained_differences=differences)
    (out / 'execution.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(cases=len(results), retained_fields=report['retained_fields'], differences=len(differences), exit=run.returncode)))


if __name__ == '__main__':
    main()
