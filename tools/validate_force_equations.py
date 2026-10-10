"""Whole-original joined sparse equation lifecycle vs independent Rust.

Authored closed groups only; no live vehicle row-production/limit/event claim.
"""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def word(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]


def add(terms, rhs):
    return [0, len(terms)] + [v for i, c in terms for v in (i, word(c))] + [word(rhs)]


def fix(i, value):
    return [1, i, word(value)]


def select(rows, variables):
    assert len(rows) == len(variables)
    return [2, len(rows)] + rows + variables


def cases():
    result = []

    def append(name, n, operations):
        result.append((name, [n, len(operations)] + sum(operations, [])))

    append('empty', 0, [[3]])
    append('all_fixed', 2, [fix(0, -0.), fix(1, 2.), add([(0, -0.), (1, 3.)], 6.), [3]])
    append('swap_last', 3, [add([(0, 2.), (1, 3.), (2, 1.)], 12.),
                          add([(1, 2.), (2, -1.)], 3.), fix(0, 2.), [3], [3]])
    append('first_zero_pivot', 2, [add([(1, 1.)], 2.), add([(1, 2.)], 3.), [3]])
    append('last_zero_pivot', 2, [add([(0, 1.), (1, 1.)], 2.), add([(0, 2.), (1, 2.)], 4.), [3]])
    append('one_zero_pivot', 1, [add([], 1.), select([0], [0]), [3]])
    append('duplicate_assignment', 2, [add([(0, 9.), (0, 2.), (1, 1.)], 5.),
                                      add([(0, 1.), (1, -1.)], 1.), [3]])
    append('duplicate_fix_first_only', 1, [add([(0, 2.), (0, 3.)], 9.), fix(0, 1.),
                                         select([0], [0]), [3]])
    append('overdetermined_first_rows', 1, [add([(0, 2.)], 6.), add([(0, 3.)], 12.), [3]])
    append('failed_selected_then_cleanup', 2, [add([(1, 1.)], 2.), add([(1, 2.)], 3.),
                                             add([(1, 3.)], 4.), add([(1, 4.)], 5.),
                                             select([0, 1], [0, 1]), [3]])

    rng = random.Random(2008164)
    for n in range(1, 41):
        for variant in range(6):
            perm = list(range(n))
            rng.shuffle(perm)
            pre = perm[:min(n - 1, variant % 3)]
            unknown = perm[len(pre):]
            post = unknown[0] if variant >= 3 and len(unknown) > 1 else None
            ops = [fix(i, rng.uniform(-3, 3)) for i in pre]
            for row, i in enumerate(unknown):
                ids = [i] if i == post else [i] + rng.sample([j for j in unknown if j != i], min(len(unknown)-1, 4))
                terms = [(j, (5. + rng.random() if j == i else rng.uniform(-.5, .5))) for j in ids]
                terms += [(j, rng.uniform(-2, 2)) for j in pre]
                if len(terms) < 8:
                    terms.append((i, -0. if variant % 2 else 0.))
                # Zero unfixed duplicate must be omitted, not overwrite the diagonal.
                rng.shuffle(terms)
                ops.append(add(terms, rng.uniform(-10, 10)))
            if post is not None:
                ops.append(fix(post, rng.uniform(-1, 1)))
            ops += [[3], [3]]
            append(f'band_{n:02d}_{variant}', n, ops)

    # Explicit small group solves followed by remaining solve, with arbitrary ID ordering.
    for index in range(96):
        sizes = [1 + index % 8, 1 + (index // 8) % 8, 1 + (index // 3) % 8]
        n = sum(sizes)
        variables = list(range(n))
        rng.shuffle(variables)
        ops, groups, offset = [], [], 0
        for size in sizes:
            group = variables[offset:offset+size]
            rows = list(range(offset, offset+size))
            for i in group:
                terms = [(j, rng.uniform(-1, 1) if i != j else 9. + rng.random()) for j in group]
                rng.shuffle(terms)
                ops.append(add(terms, rng.uniform(-20, 20)))
            rng.shuffle(rows)
            rng.shuffle(group)
            groups.append((rows, group))
            offset += size
        ops.append(select(*groups[index % 3]))
        ops.append(select(*groups[(index + 1) % 3]))
        ops += [[3], [3]]
        append(f'groups_{index:03d}', n, ops)

    # Pivot ties, row scaling, signed zeros and wide finite coefficient magnitudes.
    for index in range(48):
        n = 2 + index % 7
        scale = [1., 1e-10, 1e10, .125][index % 4]
        ops = []
        for i in range(n):
            terms = [(j, scale * (2. if i == j else (-1. if (i+j) % 2 else .25))) for j in range(n)]
            ops.append(add(terms, scale * (i - 2.5)))
        ops += [select(list(reversed(range(n))), list(range(n))), [3]]
        append(f'scaled_{index:02d}', n, ops)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT/'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(exist_ok=False)
    data = cases()
    request = out/'input.tsv'
    request.write_text(''.join(name+'\t'+'\t'.join(f'{v:08x}' for v in values)+'\n' for name, values in data), encoding='ascii')
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    executions = []
    for exe, arguments in [
        (ROOT/'local/behavior/force-equations-probe.exe', [image, request, out/'original.tsv']),
        (ROOT/'local/behavior/force-equations-probe.exe', [image, request, out/'repeat.tsv']),
        (ROOT/f'target/{args.profile}/grid-force-equations.exe', [request, out/'rust.tsv'])]:
        run = subprocess.run([str(exe), *map(str, arguments)], cwd=ROOT, capture_output=True, text=True, timeout=120)
        executions.append({'path': str(exe), 'sha256': digest(exe), 'exit': run.returncode, 'stdout': run.stdout, 'stderr': run.stderr})
        if run.returncode:
            (out/'failure.json').write_text(json.dumps(executions, indent=2))
            raise RuntimeError('Equation comparison failed; evidence retained')

    def rows(path):
        result = {}
        with path.open() as source:
            for line in source:
                row = line.rstrip().split('\t')
                assert row[0] == 'P' and len(row) == 888
                key = row[1], int(row[2])
                assert key not in result
                result[key] = row[3:]
        return result

    original, repeat, rust = [rows(out/name) for name in ('original.tsv', 'repeat.tsv', 'rust.tsv')]
    expected = {(name, operation) for name, values in data for operation in range(values[1])}
    assert set(original) == set(repeat) == set(rust) == expected
    diffs = []
    for key in sorted(expected):
        bad = [{'field': i, 'original': a, 'repeat': b, 'rust': c}
               for i, (a, b, c) in enumerate(zip(original[key], repeat[key], rust[key])) if a != b or a != c]
        if bad:
            diffs.append({'case': key[0], 'operation': key[1], 'differences': bad})
    (out/'differences.json').write_text(json.dumps(diffs, indent=2))
    if diffs:
        raise AssertionError(f'{len(diffs)} mismatching operation snapshots; evidence retained')

    # Independent behavioral checks beyond byte comparison.
    swap = original['swap_last', 2][1:]
    selected = original['failed_selected_then_cleanup', 4][1:]
    cleared = original['failed_selected_then_cleanup', 5][1:]
    semantics = {
        'empty_remaining_success': original['empty', 0][0] == '00000001',
        'elimination_pivot_failure': original['first_zero_pivot', 2][0] == '00000000',
        'last_pivot_unchecked': original['last_zero_pivot', 2][0] == '00000001',
        'one_zero_infinity': original['one_zero_pivot', 1][5] == '7f800000',
        'swap_last_order': swap[84] == '00000002' and swap[84+9:84+11] == ['00000002', '00000001'],
        'stale_removed_tail': swap[84+11] == '00000002',
        'failed_selection_marks': selected[84+18] == selected[103+18] == '00000001',
        'failed_remaining_cleanup': cleared[84+18] == cleared[103+18] == '00000000',
    }
    assert all(semantics.values())

    invalid = [
        ('capacity', [41, 1, 3]), ('operations_zero', [1, 0]), ('unknown_op', [1, 1, 4]),
        ('bad_id', [1, 1]+add([(1, 1.)], 1.)),
        ('nan_rhs', [1, 1, 0, 0, 0x7fc00000]),
        ('nonfinite_fix', [1, 1, 1, 0, 0x7f800000]),
        ('too_many_terms', [1, 1, 0, 9]+[0, word(1.)]*9+[word(1.)]),
        ('empty_selection', [1, 1, 2, 0]),
        ('underdetermined', [1, 1, 3]),
        ('unclosed', [2, 2]+add([(0, 1.), (1, 1.)], 2.)+select([0], [0])),
        ('duplicate_selected_vars', [2, 3]+add([(0, 1.)], 1.)+add([(1, 1.)], 2.)+select([0, 1], [0, 0])),
        ('duplicate_selected_rows', [2, 3]+add([(0, 1.)], 1.)+add([(1, 1.)], 2.)+select([0, 0], [0, 1])),
        ('truncated', [1, 1, 1, 0]), ('trailing', [0, 1, 3, 0]),
        ('row_capacity', [1, 41]+sum([add([(0, 1.)], 1.) for _ in range(41)], [])),
    ]
    malformed = []
    exe = ROOT/f'target/{args.profile}/grid-force-equations.exe'
    for name, values in invalid:
        path, target = out/(name+'.tsv'), out/(name+'-result.tsv')
        path.write_text('bad\t'+'\t'.join(f'{v:08x}' for v in values)+'\n', encoding='ascii')
        run = subprocess.run([str(exe), str(path), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=15)
        malformed.append({'name': name, 'exit': run.returncode, 'stderr': run.stderr, 'no_output': not target.exists()})
        assert run.returncode and not target.exists()
    summary = {'cases': len(data), 'operation_snapshots': len(expected), 'compared_words': len(expected)*885,
               'exact_words': len(expected)*885, 'semantic_checks': len(semantics), 'malformed_guards': len(malformed),
               'failure_snapshots': sum(row[0] == '00000000' for row in original.values()),
               'variable_counts': sorted({v[0] for _, v in data})}
    report = {'profile': args.profile, 'summary': summary, 'executions': executions,
              'image_sha256': digest(image), 'input_sha256': digest(request), 'malformed': malformed, 'semantic_checks': semantics,
              'limits': 'Whole original 992320/9930A0/9925E0/992FD0 and actual 991920 dependency. '
                        'Authored closed bounded groups; no whole 9B5370 selection policy, vehicle row production, '
                        'active force limits, event-time driver, live vehicle snapshot or driving equivalence.'}
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({'profile': args.profile, **summary}))


if __name__ == '__main__':
    main()
