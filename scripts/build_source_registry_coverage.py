"""Project explicit registry-to-planting coverage; local custody is an opt-in live check."""
from __future__ import annotations
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import gzip
import json
import os
from pathlib import Path
from source_registry_common import ROOT, PACKET, digest, encoded, read, safe


def assess_target(root: Path, target: dict, plantings: dict, *, verify_local: bool = False) -> dict:
    ids, paths = target['ids'], target['paths']
    result = {'work_id': ids['work'], 'edition_id': ids['edition'], 'item_id': ids['item'],
              'title': target['title'], 'work_ref': paths['work'], 'item_manifest_ref': paths['item_root'] + '/item.manifest.json',
              'branch_planting_refs': [], 'file_count': 0, 'intake_evidence': 'not_installed',
              'status': 'prepared_version_not_installed', 'all_versions_or_corpus_complete': False}
    manifest_path = safe(root, result['item_manifest_ref'])
    if not manifest_path.is_file():
        return result
    for kind in ('work', 'expression', 'edition', 'item'):
        record = read(safe(root, paths[kind]))
        if record['record_id'] != ids[kind]:
            raise ValueError('installed source identity differs from the reviewed target')
        if kind == 'expression' and record['work_ref'] != ids['work']:
            raise ValueError('installed expression belongs to another work')
        if kind == 'edition' and ids['expression'] not in record['embodies_expression_refs']:
            raise ValueError('installed edition omits the selected expression')
    manifest = read(manifest_path)
    if manifest['item_id'] != ids['item'] or manifest['embodiment_ref'] != ids['edition']:
        raise ValueError('installed Item/Edition chain differs')
    if not manifest['payload_files'] or len(manifest['payload_files']) != len(target['files']):
        raise ValueError('declared file set is incomplete against preparation')
    expected = {f['basename']: f for f in target['files']}
    if {f['original_basename'] for f in manifest['payload_files']} != set(expected):
        raise ValueError('declared file names differ from the prepared exact file set')
    events = [json.loads(line) for line in safe(root, manifest['provenance_ref']).read_text().splitlines() if line]
    acquired = [event for event in events if event['event_id'] == manifest['acquisition_event_ref'] and event['event_type'] == 'acquisition' and event['status'] in ('completed', 'completed_with_warnings')]
    if len(acquired) != 1:
        raise ValueError('Item lacks one recorded completed acquisition event')
    output_files = {(output['ref'], output['sha256']) for output in acquired[0]['outputs'] if output.get('sha256')}
    local = []
    for f in manifest['payload_files']:
        if (f['file_id'], f['sha256']) not in output_files or f['byte_size'] != expected[f['original_basename']]['byte_size']:
            raise ValueError('File is not bound by prepared size and acquired output digest')
        if verify_local:
            path = safe(root, str(manifest_path.parent.relative_to(root) / f['relative_path']))
            state = 'missing_in_this_checkout' if not path.is_file() else 'verified' if digest(path.read_bytes()) == f['sha256'] else 'fixity_mismatch'
            local.append({'path': path.relative_to(root).as_posix(), 'state': state})
    result['file_count'] = len(manifest['payload_files'])
    result['intake_evidence'] = 'completed_acquisition_recorded'
    result['acquisition_event_ref'] = manifest['acquisition_event_ref']
    result['branch_planting_refs'] = sorted(ref for ref, p in plantings.get(ids['work'], [])
        if p['source_witness']['record_ref'] == paths['work'] and p['status'] == 'source_witness_planted')
    result['status'] = 'selected_version_planted' if result['branch_planting_refs'] else 'acquired_version_needs_branch'
    if verify_local:
        result['local_now'] = {'scope': 'this checkout only', 'state': 'verified' if all(f['state'] == 'verified' for f in local) else 'needs_attention', 'files': local}
    return result


def classify_record(record: dict, selected: list[dict]) -> dict:
    planted = any(t['status'] == 'selected_version_planted' for t in selected)
    state = ('selected_versions_planted' if planted else 'selected_versions_pending' if selected else
             'possible_owner_correspondence' if record.get('owner_matches') else 'not_yet_reconciled')
    return {'record_id': record['record_id'], 'source_record_id': record['source_record_id'],
            'corpus_id': record['corpus_id'], 'document_id': record['document_id'], 'kind': record['kind'],
            'status': state, 'selected_targets': selected,
            'possible_owner_refs': [m['owner_ref'] for m in record.get('owner_matches', [])],
            'lead_scope_exhausted': False,
            'next_condition': 'Review remaining works/versions and gaps independently; a selected version never closes the whole lead.' if planted else
                'Review exact owner correspondence or finish the selected target; absence of this link is not proof that the work is absent from ToS.'}


def build(root: Path = ROOT, *, verify_local: bool = False) -> dict:
    packet = root / PACKET
    reconciliation_path = packet / 'reconciliation.current.json.gz'
    reconciliation = read(reconciliation_path)
    if reconciliation['snapshot_id'] != read(packet / 'current.json')['snapshot_id']:
        raise ValueError('reconciliation is not tied to the current registry snapshot')
    plantings = defaultdict(list)
    planting_count = 0
    for path in sorted((root / 'ToS/philosophy').rglob('source-planting.json')):
        p = read(path)
        planting_count += 1
        if p['source_witness'].get('work_id'):
            plantings[p['source_witness']['work_id']].append((path.relative_to(root).as_posix(), p))
    selected = defaultdict(dict)
    targets = {}
    for path in sorted((root / 'ToS/source-witnesses/discovery').glob('*/manifest.json')):
        manifest = read(path)
        if manifest.get('schema_version') != 'tos_registry_first_planting_preparation_v1':
            continue
        checkpoint = path.parent / 'preparation-checkpoint-receipt.json'
        if not checkpoint.is_file():
            continue
        receipt = read(checkpoint)
        if receipt.get('status') != 'passed' or receipt.get('manifest_sha256') != digest(path.read_bytes()) or not receipt.get('checkpoint_review_ref'):
            raise ValueError('selected manifest lacks its exact reviewed preparation receipt')
        for target in manifest['targets']:
            observed = assess_target(root, target, plantings, verify_local=verify_local)
            observed['preparation_ref'] = path.relative_to(root).as_posix()
            observed['review_ref'] = receipt['checkpoint_review_ref']
            key = target['ids']['item']
            if key in targets and targets[key] != observed:
                raise ValueError('same exact Item has conflicting reviewed target coverage')
            targets[key] = observed
            for lead in target['registry_sources']:
                selected[lead['entry_id']][key] = observed
    records = [classify_record(r, [selected[r['record_id']][k] for k in sorted(selected[r['record_id']])]) for r in reconciliation['records']]
    known = {r['record_id'] for r in records}
    if set(selected) - known:
        raise ValueError('reviewed target references an absent normalized registry record')
    statuses = Counter(r['status'] for r in records)
    work_count = len({r['record_id'] for r in reconciliation['owner_sources'] if r['record_type'] == 'work'})
    return {'schema_version': 'tos_registry_planting_coverage_v1', 'snapshot_id': reconciliation['snapshot_id'],
            'semantic_ceiling': 'derived navigation; no new identity, rights, textual acceptance or canon',
            'custody_scope': 'live file hashes in this checkout' if verify_local else 'recorded acquisition evidence; current local existence not asserted',
            'summary': {'registry_records': sum(r['kind'] == 'registry' for r in records), 'gap_records': sum(r['kind'] == 'gaps' for r in records),
                        'catalogued_works': work_count, 'all_branch_plantings': planting_count,
                        'selected_items': len(targets), 'selected_items_planted': sum(t['status'] == 'selected_version_planted' for t in targets.values()),
                        'selected_files_with_intake_evidence': sum(t['file_count'] for t in targets.values()), 'record_status_counts': dict(sorted(statuses.items()))},
            'targets': [targets[k] for k in sorted(targets)], 'records': records}


def markdown(value: dict, root: Path = ROOT) -> str:
    s = value['summary']; parent = root / PACKET
    link = lambda ref: os.path.relpath(root / ref, parent)
    lines = ['# Покрытие реестра посадками', '', 'Проекция по исходным owner-записям, точным подготовкам и связям ветвей. Перестраивается командой `python3 -B scripts/build_source_registry_coverage.py`.', '',
             f"Каталог: {s['catalogued_works']} Work. Связей посадки во всём Древе: {s['all_branch_plantings']}. В просмотренных партиях реестра: {s['selected_items_planted']} посаженных версий, {s['selected_files_with_intake_evidence']} файлов с записью о приобретении.", '',
             'Это отметка о сохранённом свидетельстве приобретения. Текущее наличие и SHA-256 файлов в данном checkout проверяет `python3 -B scripts/build_source_registry_coverage.py --verify-local`.', '',
             'Одна посаженная версия не закрывает весь корпус, все переводы или все издания. «Ещё не сопоставлено» означает отсутствие проверенной связи с этой строкой реестра; произведение может уже находиться в Древе под другой записью.', '',
             '## Посаженные выбранные версии', '', '| Произведение | Точная версия | Файлов | Ветвь |', '| --- | --- | ---: | --- |']
    for t in value['targets']:
        branch = ', '.join(f'[посадка]({link(ref)})' for ref in t['branch_planting_refs']) or 'ожидает связи'
        lines.append(f"| [{t['title']}]({link(t['work_ref'])}) | [{t['edition_id']}]({link(t['item_manifest_ref'])}) | {t['file_count']} | {branch} |")
    lines += ['', '## Где продолжать', '', 'Только строки без подтверждённой выбранной посадки: `python3 -B scripts/build_source_registry_coverage.py --remaining --document A25`. Без `--document` команда выводит все такие строки JSONL. Возможные совпадения требуют проверки идентичности; они не принимаются автоматически.', '',
              'Полное состояние всех строк и выбранных версий: `coverage.current.json.gz`. Пробелы исходного исследования (`gaps`) сохраняются отдельно от строк `registry`.', '',
              '| Досье | Строк реестра | С выбранными посадками | Возможные совпадения | Ещё не сопоставлено | Пробелов исследования |', '| --- | ---: | ---: | ---: | ---: | ---: |']
    by_doc = defaultdict(Counter)
    for r in value['records']:
        counts = by_doc[r['document_id']]
        counts[r['kind']] += 1
        if r['kind'] == 'registry': counts[r['status']] += 1
    for doc, c in sorted(by_doc.items()):
        lines.append(f"| {doc} | {c['registry']} | {c['selected_versions_planted']} | {c['possible_owner_correspondence']} | {c['not_yet_reconciled']} | {c['gaps']} |")
    return '\n'.join(lines) + '\n'


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--verify-local', action='store_true', help='Read current payload hashes; print observation without changing the portable projection')
    parser.add_argument('--remaining', action='store_true', help='Print unplanted registry rows as JSONL; this is not a proof of corpus absence')
    parser.add_argument('--document')
    args = parser.parse_args()
    value = build(verify_local=args.verify_local)
    if args.verify_local:
        print(json.dumps({'observed_at': datetime.now(timezone.utc).isoformat(), 'custody_scope': value['custody_scope'], 'targets': value['targets']}, ensure_ascii=False, indent=2))
        return int(any(t.get('local_now', {}).get('state') != 'verified' for t in value['targets']))
    if args.remaining or args.document:
        for r in value['records']:
            if r['kind'] == 'registry' and (not args.document or r['document_id'] == args.document) and (not args.remaining or r['status'] != 'selected_versions_planted'):
                print(json.dumps(r, ensure_ascii=False))
        return 0
    outputs = {'coverage.current.json.gz': gzip.compress(encoded(value), mtime=0), 'COVERAGE.md': markdown(value).encode()}
    for name, body in outputs.items():
        path = ROOT / PACKET / name
        if args.check:
            if not path.exists() or path.read_bytes() != body:
                raise ValueError('registry coverage projection is stale: ' + name)
        else: path.write_bytes(body)
    print(json.dumps(value['summary'], ensure_ascii=False))
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
