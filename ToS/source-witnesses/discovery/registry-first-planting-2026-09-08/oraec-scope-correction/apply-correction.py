"""Recorded one-off post-acquisition scope correction; run from repository root."""
import hashlib
import json
from pathlib import Path
import sys
from datetime import datetime, timezone

root = Path.cwd().resolve()
sys.path.insert(0, str(root / 'scripts'))
from acquire_registry_sources import inspect_payloads, refresh_topology, validate_json

packet = Path(__file__).resolve().parent
base = packet.parent
manifest = json.loads((base / 'manifest.json').read_bytes())
current = packet / 'current.json'
if current.exists():
    raise SystemExit('Correction already recorded; do not rewrite its execution.')
started = datetime.now(timezone.utc).isoformat()

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def ref(path):
    return path.relative_to(root).as_posix()

def preserve(path):
    raw = path.read_bytes()
    saved = packet / 'before' / (sha(raw) + path.suffix)
    saved.parent.mkdir(parents=True, exist_ok=True)
    if saved.exists():
        assert saved.read_bytes() == raw
    else:
        saved.write_bytes(raw)
    return {'source_ref': ref(path), 'retained_ref': ref(saved), 'sha256': sha(raw), 'byte_size': len(raw)}

# Preserve the exact inspector code before the single field-recognition repair.
helper = root / 'scripts/acquire_registry_sources.py'
old_helper = helper.read_bytes().replace(b', "cotexttranslation"}', b'}')
old_path = packet / 'before' / (sha(old_helper) + '.py')
old_path.parent.mkdir(parents=True, exist_ok=True)
old_path.write_bytes(old_helper)

summary = {'schema_version': 'tos_oraec_observed_scope_correction_v1', 'status': 'source-visible-post-acquisition-correction',
           'prepared_evidence_changed': False, 'payload_bytes_changed': False, 'rights_changed': False,
           'reviewer_ref': 'model:codex', 'started_at': started, 'targets': {}}
work = []
for target in manifest['targets']:
    if target['provider'] != 'oraec':
        continue
    item = root / target['paths']['item_root']
    payloads = [(entry, (item / 'payload' / entry['basename']).read_bytes()) for entry in target['files']]
    original_digest = sha(payloads[0][1])
    parsed = json.loads(payloads[0][1])[target['coverage']['oraec_id']]
    sentences = parsed['sentences']
    tokens = [token for sentence in sentences for token in sentence['token']]
    assert not any(0x13000 <= ord(c) <= 0x143FF for c in payloads[0][1].decode())
    observed = inspect_payloads(target, payloads)
    slug = target['slug']
    event_id = 'tos.event.correction.registry-20260908.' + slug + '.observed-scope'
    paths = [item / 'component-witnesses.json', item / 'forensic-observations.json', item / 'forensic-report.md',
             root / target['paths']['expression'], root / target['paths']['translation_expression'], root / target['paths']['edition']]
    preimages = [preserve(path) for path in paths]
    preserve(item / 'provenance.jsonl')
    description = ('ORAEC scholarly JSON containing Egyptian written-form transliteration, German sentence translations '
                   'and token glosses, and lexical/morphological annotation, derived from AED-TEI and the January 2018 '
                   'database export; exact immutable ORAEC Git snapshot. No hieroglyphic representation field was observed '
                   'in these acquired bytes.')
    limits = ['Coverage is limited to ' + target['coverage']['carrier_region'] + ' of ' + target['coverage']['physical_carrier'] + '.',
              'Egyptian written-form transliteration, German sentence translation, token glosses and annotations remain separate supplied components.',
              'The broader ORAEC platform description does not establish a hieroglyphic representation in this exact raw JSON.',
              'No source characters, gaps or editorial signs were changed; no translation, semantic or canon admission follows.']
    summary['targets'][slug] = {'corrected_version_description': description, 'limits': limits, 'correction_event_ref': event_id,
        'observed_fields': {'title': parsed['title'], 'written_form_count': sum(bool(t.get('written_form', '').strip()) for t in tokens),
            'sentence_translation_count': sum(bool(s.get('translation', '').strip()) for s in sentences),
            'cotext_translation_count': sum(bool(t.get('cotext_translation', '').strip()) for t in tokens),
            'hieroglyphic_representation_field_observed': False}, 'file_sha256': original_digest,
        'prior_record_preimages': preimages}
    write(item / 'component-witnesses.json', {'schema_version': 'tos_observed_bundle_components_v1',
        'item_ref': target['ids']['item'], 'components': observed['component_witnesses']})
    write(item / 'forensic-observations.json', observed)
    for key in ('expression', 'translation_expression', 'edition'):
        path = root / target['paths'][key]
        record = json.loads(path.read_bytes())
        if key == 'expression':
            record['notes'] = (description + ' This Expression denotes only the Egyptian written-form/transliteration component. '
                'The modern German sentence translations and cotext_translation glosses have their own Expression and exact observed selectors. '
                + target['responsibility'])
        elif key == 'translation_expression':
            record['notes'] = ('Modern German sentence translations and token-level cotext_translation glosses supplied in the same immutable '
                'AED-TEI/ORAEC JSON Item. Both source selectors and observed counts are recorded in component-witnesses.json. '
                'This is not a translation generated by ToS or an accepted translation. ' + target['responsibility'])
        else:
            record['edition_statement'] = 'Immutable ' + target['repository'] + ' commit ' + target['pin'] + '; ' + description
            record['notes'] = 'Exact supplied digital edition; its Egyptian transliteration and German translation/gloss components remain separate. ' + target['responsibility']
        record['source_refs'].append(ref(current))
        record['record_version'] += 1
        validate_json(record, 'corpus-record', root)
        write(path, record)
    report = item / 'forensic-report.md'
    report.write_text(report.read_text() + '\n## Post-acquisition scope correction\n\n'
        + description + '\n\nObserved title: `' + parsed['title'] + '`.\n\n'
        + 'Observed Egyptian written forms: ' + str(summary['targets'][slug]['observed_fields']['written_form_count'])
        + '; German sentence translations: ' + str(summary['targets'][slug]['observed_fields']['sentence_translation_count'])
        + '; German token glosses: ' + str(summary['targets'][slug]['observed_fields']['cotext_translation_count']) + '.\n\n'
        + 'The initial prepared description and acquired record preimages remain retained in the discovery correction packet. '
        + 'This later observation corrects the current description; it does not rewrite source bytes or the preparation history.\n')
    assert sha((item / 'payload' / target['files'][0]['basename']).read_bytes()) == original_digest
    work.append((target, item, event_id, preimages, paths, original_digest))
summary['ended_at'] = datetime.now(timezone.utc).isoformat()
write(current, summary)
for target, item, event_id, preimages, paths, original_digest in work:
    event = {'schema_version': 'tos_provenance_event_v1', 'event_id': event_id, 'event_type': 'correction',
        'started_at': started, 'ended_at': summary['ended_at'], 'agent_refs': ['model:codex'],
        'inputs': [{'ref': p['retained_ref'], 'role': 'exact-prior-acquired-record-bytes', 'sha256': p['sha256']} for p in preimages]
            + [{'ref': 'tos.file.sha256.' + original_digest, 'role': 'unchanged-acquired-json', 'sha256': original_digest},
               {'ref': ref(base / 'manifest.json'), 'role': 'unchanged-preparation', 'sha256': sha((base / 'manifest.json').read_bytes())}],
        'outputs': [{'ref': ref(p), 'role': 'corrected-current-scope-or-observation', 'sha256': sha(p.read_bytes())} for p in paths + [current]],
        'method': {'maker_type': 'mixed', 'name': 'source-visible-oraec-component-scope-correction', 'version': '1',
            'artifact_digest': sha(Path(__file__).read_bytes()), 'runtime': sys.version.split()[0],
            'configuration': {'script_ref': ref(Path(__file__).resolve()), 'source_bytes_changed': False,
                'rights_changed': False, 'prepared_evidence_changed': False, 'semantic_admission': False,
                'reason': 'Observed token glosses lacked selectors and platform-level hieroglyphic wording overstated these exact raw files.'}},
        'status': 'completed_with_warnings', 'warnings': ['Observed supplied fields and current scope correction only; no textual, translation, semantic, canon or publication acceptance.'],
        'receipt_refs': [ref(current), ref(item / 'forensic-report.md')], 'rights_basis_ref': ref(item / 'rights.json'),
        'event_version': 1, 'supersedes_event_ref': None}
    validate_json(event, 'provenance-event', root)
    with (item / 'provenance.jsonl').open('a') as stream:
        stream.write(json.dumps(event, ensure_ascii=False, separators=(',', ':')) + '\n')
refresh_topology(root, packet, summary['ended_at'])
print(json.dumps({'status': 'corrected', 'targets': list(summary['targets']), 'current_ref': ref(current)}))
