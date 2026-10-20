#!/usr/bin/env python3
"""Independent disposable-candidate tests of the actual T0 CLI."""
import argparse
import copy
import hashlib
import json
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent
NAMES = ('entity-types.v1.json', 'relation-types.v1.json')

def load(p):
    return json.loads(p.read_text())

def write(p, value):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')

def stripped(v):
    if isinstance(v, dict):
        return {k: stripped(x) for k, x in v.items() if k not in ('labels', 'inverse_labels')}
    if isinstance(v, list):
        return [stripped(x) for x in v]
    return v

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--binary', type=pathlib.Path, required=True)
    p.add_argument('--test-executable', type=pathlib.Path)
    p.add_argument('--consumer-test-executable', type=pathlib.Path)
    p.add_argument('--output', type=pathlib.Path, required=True)
    args = p.parse_args()
    assert args.binary.is_absolute() and args.binary.is_file()
    oracle = load(ROOT / 'oracles/topology-acceptance-vectors.json')
    expected_digest = next(x['sha256'] for x in load(ROOT / 'oracles/MANIFEST.json')['files']
                           if x['path'] == 'topology-acceptance-vectors.json')
    assert hashlib.sha256((ROOT / 'oracles/topology-acceptance-vectors.json').read_bytes()).hexdigest() == expected_digest
    results = []
    def run(*cmd):
        proc = subprocess.run([str(x) for x in cmd], capture_output=True, text=True, timeout=30)
        return {'command': [str(x) for x in cmd], 'returncode': proc.returncode,
                'stdout': proc.stdout, 'stderr': proc.stderr}
    with tempfile.TemporaryDirectory(prefix='tos-rt3-topology-') as td:
        root = pathlib.Path(td)
        original = root / 'original'
        split = run(args.binary, 'split', ROOT / 'oracles' / NAMES[0], ROOT / 'oracles' / NAMES[1], original)
        if split['returncode']:
            write(args.output, {'status': 'HarnessError', 'split': split})
            return 1
        for case in oracle['cases']:
            row = {'id': case['id'], 'oracle': case, 'oracle_sha256': expected_digest,
                   'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest()}
            tree = root / case['id']
            shutil.copytree(original, tree)
            manifest_path = tree / 'manifest.json'
            m = load(manifest_path)
            def entry(entity):
                return next(e for e in m['entries'] if e['id'] == entity)
            def change(entity, key, value):
                path = tree / entry(entity)['path']
                record = load(path)
                record[key] = value
                write(path, record)
            try:
                number = int(case['id'].split('-')[1])
                reject = {2:'DuplicateCanonicalDefinition',3:'UnresolvedParent',4:'NavigationCycle',5:'SemanticEdgeLoss',8:'GeneratedOutputDivergence',9:'UnplacedDefinition'}
                if number == 2:
                    m['entries'].append(copy.deepcopy(m['entries'][0]))
                elif number == 3:
                    change('tos.entity.place', 'parent_type_ids', ['rt3-missing-parent'])
                elif number == 4:
                    a, b = m['entries'][:2]
                    a['navigation_parent'], b['navigation_parent'] = b['id'], a['id']
                elif number == 5:
                    change('tos.entity.place', 'parent_type_ids', ['tos.entity.spatial-object'])
                elif number == 6:
                    e = entry('tos.entity.place')
                    old = e['path']
                    new = str(pathlib.PurePosixPath(old).with_name('rt3-moved-place.json'))
                    (tree / old).rename(tree / new)
                    e['path'], e['previous_paths'] = new, [old]
                    row['mutation'] = {'from': old, 'to': new, 'id': e['id']}
                elif number == 7:
                    m['navigation_roots'].append('rt3-navigation-root')
                    m['navigation_groups']['navigation.relations'] = 'rt3-navigation-root'
                    m['navigation_roots'].remove('navigation.relations')
                elif number == 9:
                    m['entries'][0]['navigation_parent'] = 'rt3-missing-navigation-home'
                elif number == 10:
                    change('tos.entity.place', 'labels', {'default':'Δ arbitrary display', 'en':'Renamed 117'})
                elif number in (11,12):
                    binary = args.consumer_test_executable if number == 11 else args.test_executable
                    test = 'existing_schema_and_registry_consumers_accept_full_candidate_and_new_instance' if number == 11 else 't0_012_interrupted_write_does_not_publish'
                    if binary is None:
                        row.update(status='PreparedNotExecuted', dependency='explicit compiled '+test+' test executable required')
                    else:
                        observed = run(binary, '--exact', test)
                        passed = observed['returncode'] == 0 and '1 passed' in observed['stdout']
                        row.update(status='Pass' if passed else 'Fail', execution=observed,
                                   acceptance_kind='ActualOwnerTestExecution; independently selected original requirement; assertion code owned by T0')
                    results.append(row)
                    write(args.output, results)
                    continue
                write(manifest_path, m)
                output = root / (case['id'] + '-output')
                if number == 8:
                    assembled = run(args.binary, 'assemble', tree, output)
                    assert assembled['returncode'] == 0, assembled
                    (output / NAMES[0]).write_text('{}')
                    actual = run(args.binary, 'check-generated', tree, output)
                else:
                    actual = run(args.binary, 'check', tree)
                row['execution'] = actual
                if number in reject:
                    row['status'] = 'Pass' if actual['returncode'] != 0 and actual['stderr'].startswith(reject[number] + ':') else 'Fail'
                else:
                    assert actual['returncode'] == 0, actual
                    built = run(args.binary, 'assemble', tree, output)
                    row['assembly'] = built
                    assert built['returncode'] == 0, built
                    equality = {name: load(output / name) == load(ROOT / 'oracles' / name) for name in NAMES}
                    bytes_equal = {name: (output / name).read_bytes() == (ROOT / 'oracles' / name).read_bytes() for name in NAMES}
                    row.update(parsed_equal=equality, byte_equal=bytes_equal)
                    raw_retained = {name: (tree / m['baseline'][i]['path']).read_bytes() == (ROOT / 'oracles' / name).read_bytes()
                                    for i, name in enumerate(NAMES)}
                    second = root / (case['id'] + '-second-output')
                    repeated = run(args.binary, 'assemble', tree, second)
                    deterministic = repeated['returncode'] == 0 and all((output / name).read_bytes() == (second / name).read_bytes() for name in NAMES)
                    row.update(raw_baseline_retained=raw_retained, deterministic_output=deterministic,
                               serialization_gate='Byte identity with original reported separately; no supplied requirement mandates reserialization byte identity')
                    if number == 10:
                        ok = all(stripped(load(output / name)) == stripped(load(ROOT / 'oracles' / name)) for name in NAMES)
                    else:
                        ok = all(equality.values())
                    ok = ok and all(raw_retained.values()) and deterministic
                    row['status'] = 'Pass' if ok else 'Fail'
            except (AssertionError, KeyError, ValueError, OSError, subprocess.TimeoutExpired) as exc:
                row.update(status='HarnessError', error=str(exc))
            results.append(row)
            write(args.output, results)
    print(json.dumps({s:sum(r['status']==s for r in results) for s in sorted({r['status'] for r in results})}))
    return int(any(r['status'] != 'Pass' for r in results))

if __name__ == '__main__':
    raise SystemExit(main())
