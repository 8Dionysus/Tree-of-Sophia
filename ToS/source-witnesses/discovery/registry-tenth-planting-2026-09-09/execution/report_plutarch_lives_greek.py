"""Keep prepared readiness and observed completion as separate dated records."""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import sys

ROOT = Path(__file__).resolve().parents[5]
BASE = Path(__file__).resolve().parents[1]
REL = BASE.relative_to(ROOT).as_posix()
sys.path.insert(0, str(ROOT / 'scripts'))
from acquire_registry_sources import verify_target
from source_registry_common import read


def ref(path):
    return {'path': path, 'sha256': hashlib.sha256((ROOT / path).read_bytes()).hexdigest()}


def control(paths, rationale, status='ready', **extra):
    return {'status': status, 'evidence_posture': 'owner-reviewed', 'owner_refs': [ref(path) for path in paths],
        'rationale': rationale, **extra}


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def report(completed):
    manifest = read(BASE / 'manifest.json')
    branches = {plan['target_slug']: plan for plan in read(ROOT / manifest['branch_preparation_ref'])['targets']}
    now = datetime.now(timezone.utc).isoformat()
    if not completed:
        observations = []
        for target in manifest['targets']:
            paths = [target['paths']['item_root'] + '/payload/' + entry['basename'] for entry in target['files']]
            if any((ROOT / path).exists() for path in paths):
                raise ValueError('initial absence inspection may not replace acquired file history')
            observations.append({'target_slug': target['slug'], 'new_payload_paths': paths, 'new_payloads_present': False})
        write(BASE / 'local-presence.preparation.json', {'observed_at': now, 'scope': 'this checkout before tenth acquisition', 'targets': observations})
    snapshot = ROOT / manifest['source_registry_snapshot_ref']
    documents = {}
    for relative in read(snapshot)['documents']:
        path = snapshot.parent / relative
        document = read(path)
        documents[(document['corpus_id'], document['document_id'])] = path.relative_to(ROOT).as_posix()
    targets, rows = [], []
    for index, target in enumerate(manifest['targets'], 1):
        plan = branches[target['branch_target_slug']]
        branch = plan['anchor_preparation']['branch_path']
        planting_ref = branch + '/sources/plantings/registry-' + target['slug'] + '/source-planting.json'
        reading_ref = str(Path(planting_ref).with_name('README.md'))
        item = target['paths']['item_root']
        files = [item + '/payload/' + entry['basename'] for entry in target['files']]
        discovery_ref = 'ToS/source-witnesses/discovery/runs/registry-' + target['slug'] + '.2026-09-09.v1.json'
        readiness = {
            'version': control([REL + '/manifest.json', REL + '/SOURCE_AND_RIGHTS_REVIEW.md'], 'Exact Greek edition, CTS Work identity, contributor statement and file list reviewed.'),
            'access': control([discovery_ref] if completed else [next(path for path in target['metadata_evidence_refs'] if path.endswith('-header.xml')) + '.receipt.json'],
                'Pinned source GET completed and retained.' if completed else 'Pinned source header GET succeeded; complete file transfer remains subject to exact acquisition checks.'),
            'rights': control([item + '/rights.json'] if completed else [REL + '/prepared-source-packages.jsonl', REL + '/SOURCE_AND_RIGHTS_REVIEW.md'],
                'Source and digital edition layers assessed for local preservation and research.', intended_uses=['local-preservation', 'research-analysis']),
            'file': control(files if completed else [REL + '/local-presence.preparation.json'],
                'Exact acquired files opened and verified locally.' if completed else 'Exact new version destinations were inspected and are absent before acquisition.', status='present' if completed else 'absent'),
            'branch': control([planting_ref, reading_ref] if completed else [manifest['branch_preparation_ref']],
                'Exact Greek edition branch and local reading routes are present.' if completed else 'Current branch and corpus anchor reviewed for this separately identified Work.'),
        }
        source_refs = [{**ref(documents[(source['corpus'], source['document_id'])]),
            'corpus_id': source['corpus'], 'document_id': source['document_id'], 'record_id': source['entry_id']}
            for source in target['registry_sources']]
        entry = {'target_id': f'registry-tenth-planting.{index:02}.{target["slug"]}', 'preferred_label': target['title'],
            'source_record_refs': source_refs,
            'target': {'target_kind': 'exact-work-version', 'description': target['version_description'],
                'known_tos_refs': [branch],
                'create_record_refs': target['paths'],
                'planned_ids': target['ids'], 'expected_coverage': target['coverage'],
                'expected_file_count': len(files), 'expected_bytes': target['byte_size'], 'scope_limits': target['limits']},
            'readiness': readiness,
            'acquisition': {'source_urls': [file['url'] for file in target['files']], 'destination_paths': files,
                'intended_uses': ['local-preservation', 'research-analysis']}}
        if completed:
            checked = verify_target(ROOT, target)
            planting = read(ROOT / planting_ref)
            if planting['source_witness']['work_id'] != target['ids']['work'] or planting['discovery_ref'] != discovery_ref:
                raise ValueError('branch route does not identify this exact Greek edition intake')
            entry['execution'] = control([planting_ref, target['paths']['work'], target['paths']['expression'], item + '/item.manifest.json'],
                'Exact Greek version acquired and branch-linked; no textual or semantic admission.', status='completed')
            rows.append({**checked, 'title': target['title'], 'language': 'grc',
                'expression_role': 'source-language-edition', 'new_work_created': True,
                'atlas_row': plan['anchor_preparation']['atlas_row_id'], 'planting_ref': planting_ref,
                'reading_route': reading_ref,
                'observed_coverage': read(ROOT / item / 'forensic-observations.json')['files'][0],
                'remaining_controls': planting['remaining_controls']})
        targets.append(entry)
    name = 'readiness.post-acquisition.v1.json' if completed else 'readiness.preparation.v1.json'
    write(BASE / name, {'schema_version': 'tos_open_work_readiness_plan_v1', 'reviewed_at': now,
        'reviewer_ref': 'model:codex/session/01a08281-293a-7ff0-927a-0fd73dfacf7d', 'targets': targets})
    if not completed:
        print(json.dumps({'prepared_readiness_targets': len(targets), 'acquired': False}))
        return
    selection = read(BASE / 'selection-review.json')
    result = {'schema_version': 'tos_registry_first_planting_result_v1', 'completed_at': now,
        'preparation_commit': read(BASE / 'preparation-checkpoint-receipt.json')['commit'],
        'normalization_snapshot': manifest['source_registry_snapshot_ref'], 'planted': rows,
        'deferred': [row for row in selection['candidates'] if row['decision'] == 'defer'], 'blocked': [],
        'total_files': sum(row['files'] for row in rows), 'total_bytes': sum(row['bytes'] for row in rows),
        'scope': '64 separately identified Greek Lives and comparisons, tlg0007.tlg001 through tlg066, through the A29 Plutarch anchor.',
        'human_form_companions_ref': REL + '/human-form-companions.json',
        'new_works_created': len(rows), 'source_text_admitted': False, 'payload_visibility': 'local_only', 'public_or_remote_deployment': False}
    write(BASE / 'batch-result.json', result)
    lines = ['# Десятая посадка: греческие жизнеописания и сопоставления Плутарха', '',
        f"Добавлено {len(rows)} произведений и точных греческих версий: {result['total_files']} файлов, {result['total_bytes']:,} байт.", '',
        '64 самостоятельных CTS-записи: жизнеописания и отдельные сопоставления. Исторические сообщения и авторские суждения остаются текстом источника, без принятия их истинности.', '',
        '| Произведение | Ветвь | Греческая версия |', '| --- | --- | --- |']
    for row in rows:
        lines.append(f"| {row['title']} | {row['atlas_row']} | [Открыть]({os.path.relpath(ROOT / row['reading_route'], BASE)}) |")
    lines.extend(['', '## Границы', '',
        'Сопоставления не объединены с парными жизнеописаниями; общая издательская серия не устанавливает тождество произведений.', '',
        'Для всех 256 новых записей созданы 512 форм метаданных через штатную команду владельца: точное копирование исходных полей, без семантического принятия. [Проверка форм](human-form-companions.json).', '',
        f"Отложено в пределах этой партии: {len(result['deferred'])} объединённых носителя — Agis and Cleomenes и Tiberius and Caius Gracchus. Требуется отдельный разбор состава и идентичности у владельца source-witnesses. Другие версии не входили в отбор.", '',
        'Названия, редакторы и даты сохраняют буквальную форму поставщика. Полнота файлов не доказывает полноту античного корпуса; точные ограничения перечислены в разборе источника.', '',
        '[Покрытие реестра](../../../research-packets/source-registries/COVERAGE.md). [Отбор](selection-review.json). [Полный результат](batch-result.json). [Разбор источника и прав](SOURCE_AND_RIGHTS_REVIEW.md).', '',
        'Исходные файлы локальны и исключены из Git; метаданные, fixity, права, происхождение и читательские маршруты отслеживаются. Принятие текста, интерпретации, канона, CI, merge и публикация остаются отдельными состояниями.', ''])
    (BASE / 'RESULTS.md').write_text('\n'.join(lines))
    (BASE / 'README.md').write_text('# Tenth registry planting\n\n[Greek reading routes and result](RESULTS.md). [Source and rights review](SOURCE_AND_RIGHTS_REVIEW.md). [Selection](selection-review.json).\n\nPrepared readiness and post-acquisition completion retain separate evidence.\n')
    for branch in sorted({plan['anchor_preparation']['branch_path'] for plan in branches.values()}):
        branch_root = ROOT / branch
        entries = [row for row in rows if row['planting_ref'].startswith(branch + '/')]
        for path in [branch_root / 'README.md', branch_root / 'sources/README.md']:
            prior = path.read_text() if path.exists() else '# Sources\n'
            marker = '## Registry planting: Plutarch Lives Greek (2026-09-09)'
            if marker in prior: continue
            added = '\n\n' + marker + '\n\n' + f'{len(entries)} exact Greek versions. [Batch reading routes and evidence]({os.path.relpath(BASE / "RESULTS.md", path.parent)}).\n'
            if path.parent.name == 'sources':
                added += '\n' + '\n'.join('- [' + row['title'] + '](' + os.path.relpath(ROOT / row['reading_route'], path.parent) + ')' for row in entries) + '\n'
            path.write_text(prior.rstrip() + added)
    print(json.dumps({'completed': len(rows), 'new_works': len(rows), 'files': result['total_files'], 'bytes': result['total_bytes']}))


if __name__ == '__main__':
    if sys.argv[1:] not in (['prepare'], ['complete']):
        raise SystemExit('expected prepare or complete')
    report(sys.argv[1] == 'complete')
