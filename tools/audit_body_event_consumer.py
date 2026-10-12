"""Read-only current-input/source audit of the complete registered event proof."""
import argparse
import json
from pathlib import Path
import struct

from capture_reference_spawn import ANALYSIS, private
from validate_body_event_consumer import ROOT, PROBE_SHA, corpus, digest, expected, pack


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('evidence', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    evidence, output = private(args.evidence), private(args.output)
    report = json.loads((evidence / 'validation.json').read_text(encoding='utf-8'))
    rows = corpus()
    wanted, request = expected(rows), pack(rows)
    assert report['passed'] and report['cases'] == len(rows) == 2160
    assert digest(ROOT / 'local/compatibility/grid-unpacked.exe') == report['image_sha256'] == ANALYSIS
    assert digest(ROOT / 'local/behavior/body-event-consumer-probe.exe') == report['probe_sha256'] == PROBE_SHA
    assert digest(ROOT / 'tools/validate_body_event_consumer.py') == report['validator_sha256']
    for name, value in report['sources'].items():
        assert name in ('probe_body_event_consumer.c', 'build_body_event_consumer_probe.cmd', 'isolated_reference.h')
        assert digest(ROOT / 'tools' / name) == value
    assert len(report['sources']) == 3
    assert (evidence / 'input.bin').read_bytes() == request
    assert digest(evidence / 'input.bin') == report['request_sha256']
    assert (evidence / 'expected.bin').read_bytes() == wanted
    assert digest(evidence / 'expected.bin') == report['expected_sha256']
    assert report['compared_bytes_per_run'] == len(wanted) == 423368
    for label in ('original', 'repeat'):
        run = report['runs'][label]
        assert run['passed'] and run['exit'] == 0 and run['first_mismatch'] is None
        assert (evidence / f'{label}.bin').read_bytes() == wanted
        assert digest(evidence / f'{label}.bin') == run['actual_sha256']
        assert 'complete_image_unchanged=1 owned_contexts_unchanged=1 observer_error=0 no_imports=1' in run['stdout']
    counts = [struct.unpack_from('<I', wanted, 8+i*196)[0] for i in range(len(rows))]
    assert report['callback_counts'] == {str(n): counts.count(n) for n in range(3)}
    assert report['shared_mutation_cases'] == sum(n == 2 and bool(w[8]) for n, (_, w) in zip(counts, rows)) == 300
    guards = report['guards']
    assert len(guards) == 12 and len({g['label'] for g in guards}) == 12
    for guard in guards:
        assert guard['passed'] and guard['exit'] != 0
        if guard['label'] == 'existing-output':
            assert (evidence / 'existing.bin').read_bytes() == b'preserve existing evidence'
        elif guard['label'] == 'wrong-reference':
            assert (evidence / 'wrong-reference.bin').read_bytes() == bytes(64)
            assert not (evidence / 'wrong-reference-output.bin').exists()
        else:
            assert not (evidence / f"guard-{guard['label']}-output.bin").exists()
    result = dict(passed=True, cases=len(rows), complete_output_bytes=2*len(wanted), guards=len(guards),
                  report_sha256=digest(evidence / 'validation.json'), auditor_sha256=digest(Path(__file__)),
                  scope='Current identities and reconstructed corpus/oracle match both complete original outputs; '
                  'owned-context canaries/immutability and complete-image checks are native assertions, not recaptured memory.')
    with output.open('x', encoding='utf-8') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
