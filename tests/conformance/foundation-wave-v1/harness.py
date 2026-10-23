#!/usr/bin/env python3
"""Independent preparation and black-box execution; never sends expected to adapter."""
import argparse
import collections
import copy
import hashlib
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent
CATEGORIES = {
    'profile': None, 'entity': 'entities_and_nodes', 'perspective': 'knowledge_perspectives',
    'claim_stream': 'claim_streams', 'claim': 'claims', 'claim_revision': 'claim_revisions',
    'source': 'sources', 'snapshot': 'source_snapshots', 'fence': 'source_revision_fences',
    'chronology': 'chronology_versions', 'trs': 'temporal_reference_systems',
    'calendar': 'calendars', 'era': 'eras', 'zone': 'zones', 'policy': 'policies',
    'crosswalk': 'crosswalks', 'solver_profile': 'solver_profiles', 'rule': 'recurrence_rules',
    'revision_token': 'revision_tokens', 'principal': 'principal_contexts',
    'inline_fixture': 'inline_fixture_ids', 'inline_xml': 'inline_fixture_ids',
    'source_profile_fixture': 'source_profile_fixture_labels', 'tei_alias': 'tei_version_aliases',
}

def load(path):
    return json.loads(path.read_text())

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n')

def at_path(obj, path):
    for key, index in re.findall(r'([^.[\]]+)|\[(\d+)\]', path):
        if key.startswith('<key:') and key.endswith('>'):
            candidate = key[5:-1]
            if candidate not in obj:
                raise KeyError(candidate)
            return candidate
        obj = obj[int(index)] if index else obj[key]
    return obj

def prepare():
    manifest = load(ROOT / 'oracles/MANIFEST.json')
    verified = []
    for entry in manifest['files']:
        target = ROOT / 'oracles' / pathlib.Path(entry['path']).name
        if target.exists():
            if target.stat().st_size != entry['bytes'] or digest(target) != entry['sha256']:
                raise ValueError('Oracle digest mismatch: ' + entry['path'])
            verified.append({'path': entry['path'], 'sha256': entry['sha256']})
    temporal = load(ROOT / 'oracles/temporal-acceptance-vectors.json')
    topology = load(ROOT / 'oracles/topology-acceptance-vectors.json')
    resolved = missing = 0
    closure_errors = []
    unmanifested_refs = []
    rows = []
    ids = set()
    for case in temporal['vectors']:
        if case['id'] in ids:
            raise ValueError('Duplicate case ID: ' + case['id'])
        ids.add(case['id'])
        closure = case['reference_closure']
        if closure['ambient_dependencies']:
            closure_errors.append([case['id'], 'ambient dependencies'])
        for ref in closure['resolved_references']:
            actual = at_path(case, ref['path'])
            category = CATEGORIES[ref['kind']]
            registry = temporal['fixture_registry'][category] if category else temporal['profiles']
            if actual != ref['ref'] or actual not in registry:
                closure_errors.append([case['id'], ref, actual])
            resolved += 1
        for ref in closure['intentional_missing_references']:
            actual = at_path(case, ref['path'])
            if actual is not None:
                closure_errors.append([case['id'], ref, actual])
            missing += 1
        # Audit important reference slots independently of supplied closure lists.
        declared = {r['path'] for group in ('resolved_references', 'intentional_missing_references')
                    for r in closure[group]}
        key_kinds = {'trs_ref':'trs', 'comparison_axis':'trs', 'endpoint_axis':'trs',
                     'zone_ref':'zone', 'calendar_ref':'calendar', 'era_ref':'era',
                     'crosswalk_version':'crosswalk', 'target_trs':'trs', 'policy_ref':'policy'}
        def walk(value, path):
            if isinstance(value, dict):
                for key, child in value.items():
                    child_path = path + '.' + key
                    if key in key_kinds and child_path not in declared:
                        category = CATEGORIES[key_kinds[key]]
                        exists = isinstance(child, str) and child in temporal['fixture_registry'][category]
                        unmanifested_refs.append({'case':case['id'], 'path':child_path, 'ref':child,
                                                  'independently_resolved':exists})
                        if not exists:
                            closure_errors.append([case['id'], child_path, 'unmanifested unresolved ref'])
                    walk(child, child_path)
            elif isinstance(value, list):
                for index, child in enumerate(value):
                    walk(child, path + '[' + str(index) + ']')
        for section in ('input', 'context', 'expected'):
            walk(case.get(section, {}), section)
        rows.append({'suite': 'temporal', 'case': case, 'status': 'PreparedNotExecuted',
                     'reviewed_expected': reviewed_expected(case),
                     'gate': 'PostV1' if case['operation'] == 'expand_recurrence' else 'Core',
                     'source': 'temporal-acceptance-vectors.json#' + case['id']})
    for case in topology['cases']:
        rows.append({'suite': 'topology', 'case': case, 'status': 'PreparedNotExecuted',
                     'gate': 'Core', 'source': 'topology-acceptance-vectors.json#' + case['id']})
    entities = load(ROOT / 'oracles/entity-types.v1.json')
    relations = load(ROOT / 'oracles/relation-types.v1.json')
    source_pairs = [('entities', entities['types'], 'type_id', 'parent_type_ids'),
                    ('properties', entities['property_definitions'], 'property_id', None),
                    ('relations', relations['relations'], 'relation_type_id', 'parent_relation_type_ids')]
    baseline_errors = []
    for key, records, id_key, parents_key in source_pairs:
        indexed = {x[id_key]: x for x in records}
        if len(indexed) != len(records) or set(indexed) != {x['id'] for x in topology[key]}:
            baseline_errors.append([key, 'IDs differ or duplicated'])
        for summary in topology[key]:
            item = indexed[summary['id']]
            for field, value in summary.items():
                source_field = id_key if field == 'id' else parents_key if field == 'parents' else field
                if source_field not in item or item[source_field] != value:
                    baseline_errors.append([key, summary['id'], field])
    report = {'kind': 'FixtureAuditOnly', 'verified_files': verified,
              'resolved_reference_occurrences': resolved, 'intentional_missing_occurrences': missing,
              'closure_errors': closure_errors, 'baseline_summary_errors': baseline_errors,
              'unmanifested_reference_occurrences': unmanifested_refs,
              'case_counts': dict(collections.Counter(x['suite'] for x in rows)),
              'runtime_tests_executed': 0}
    write(ROOT / 'prepared/fixture-audit.json', report)
    write(ROOT / 'prepared/requirement-matrix.json', rows)
    if closure_errors or baseline_errors:
        raise ValueError('Fixture audit failed; see prepared/fixture-audit.json')
    return temporal, topology, report

def differences(expected, actual, path='$'):
    # Declared oracle fields must match exactly; extra diagnostic/evidence fields may exist.
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            return [path + ': expected object']
        return [d for key, val in expected.items()
                for d in ([path + '.' + key + ': missing'] if key not in actual
                          else differences(val, actual[key], path + '.' + key))]
    if type(expected) is not type(actual) or expected != actual:
        return [path + ': expected ' + repr(expected) + ', got ' + repr(actual)]
    return []

def reviewed_expected(case):
    expected = copy.deepcopy(case['expected'])
    corrections = load(ROOT / 'oracle-corrections.json')
    if corrections['original_sha256'] != digest(ROOT / 'oracles/temporal-acceptance-vectors.json'):
        raise ValueError('Correction sidecar does not match pinned oracle')
    for correction in corrections['corrections']:
        if correction['id'] == case['id']:
            if expected[correction['field']] != correction['original']:
                raise ValueError('Correction source value changed')
            expected[correction['field']] = correction['reviewed']
    return expected

def execute(temporal, command, output, timeout, selected):
    rows = []
    for case in temporal['vectors']:
        if selected and case['id'] not in selected:
            continue
        request = {key: case[key] for key in ('operation', 'input', 'context', 'source_profile') if key in case}
        request['profiles'] = temporal['profiles']
        request['fixture_registry'] = temporal['fixture_registry']
        row = {'id': case['id'], 'request': request, 'expected': case['expected'],
               'reviewed_expected': reviewed_expected(case),
               'oracle_sha256': digest(ROOT / 'oracles/temporal-acceptance-vectors.json'),
               'adapter_command': command}
        row['adapter_file_digests'] = {arg: digest(pathlib.Path(arg)) for arg in command
                                       if pathlib.Path(arg).is_file()}
        try:
            proc = subprocess.run(command, input=json.dumps(request), text=True,
                                  capture_output=True, timeout=timeout, check=False)
            row.update(returncode=proc.returncode, stdout=proc.stdout, stderr=proc.stderr)
            if proc.returncode:
                row.update(status='HarnessError', differences=['adapter exited nonzero'])
            else:
                actual = json.loads(proc.stdout)
                original_delta = differences(case['expected'], actual)
                delta = differences(row['reviewed_expected'], actual)
                row.update(actual=actual, differences=delta, original_oracle_differences=original_delta,
                           status='Fail' if delta else 'Pass')
                if case['operation'] == 'check_network_consistency':
                    from network_checks import verify
                    row['independent_network_evidence'] = verify(case['input'],actual)
                    if not row['independent_network_evidence']['verified']:
                        row['status'] = 'Fail'
        except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError) as exc:
            row.update(status='HarnessError', error=str(exc))
        rows.append(row)
        write(output, {'kind': 'ActualAdapterExecution', 'cases': rows,
                       'counts': dict(collections.Counter(x['status'] for x in rows))})
    return any(x['status'] != 'Pass' for x in rows)

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--adapter', nargs='+')
    p.add_argument('--case', action='append', default=[])
    p.add_argument('--timeout', type=float, default=10)
    p.add_argument('--output', type=pathlib.Path, default=ROOT / 'evidence/actual.json')
    args = p.parse_args()
    temporal, _, report = prepare()
    if args.adapter:
        unknown = set(args.case) - {c['id'] for c in temporal['vectors']}
        if unknown:
            p.error('Unknown cases: ' + repr(unknown))
        return int(execute(temporal, args.adapter, args.output, args.timeout, set(args.case)))
    print(json.dumps(report, indent=2))
    return 0

if __name__ == '__main__':
    sys.exit(main())
