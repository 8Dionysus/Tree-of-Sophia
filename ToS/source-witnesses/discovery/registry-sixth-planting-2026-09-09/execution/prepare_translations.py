"""Sixth planting: review translation metadata before acquiring complete files."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
import hashlib
import copy
import gzip
import json
import sys
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[5]
BASE = Path(__file__).resolve().parents[1]
REL = BASE.relative_to(ROOT).as_posix()
PIN = '8668397510ca77fec6406f7d48f405e50d296764'
REPOSITORY = 'PerseusDL/canonical-latinLit'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def capture(job):
    name, upstream, header_only = job
    path = BASE / 'evidence' / name
    receipt_path = path.with_name(path.name + '.receipt.json')
    url = f'https://raw.githubusercontent.com/{REPOSITORY}/{PIN}/{upstream}'
    if path.exists():
        receipt = json.loads(receipt_path.read_text())
        if receipt['url'] != url or receipt['retained_sha256'] != digest(path.read_bytes()):
            raise ValueError('retained source metadata does not match its receipt')
        return
    started = datetime.now(timezone.utc).isoformat()
    start_clock = time.monotonic()
    with urlopen(Request(url, headers={'User-Agent': 'Tree-of-Sophia-source-preparation'}), timeout=60) as response:
        if header_only:
            body = b''
            while b'</teiHeader>' not in body and len(body) < 100000:
                part = response.read(2048)
                if not part:
                    break
                body += part
            if b'</teiHeader>' not in body:
                raise ValueError('bounded complete TEI header is unavailable')
            received = len(body)
            body = body.split(b'</teiHeader>', 1)[0] + b'</teiHeader>'
        else:
            body = response.read(2000001)
            received = len(body)
            if received > 2000000:
                raise ValueError('metadata exceeds bounded allowance')
        status, final_url = response.status, response.url
    path.write_bytes(body)
    write(receipt_path, {'url': url, 'final_url': final_url, 'http_status': status,
        'started_at': started, 'ended_at': datetime.now(timezone.utc).isoformat(),
        'elapsed_seconds': time.monotonic() - start_clock,
        'retained_ref': path.relative_to(ROOT).as_posix(), 'retained_sha256': digest(body),
        'retained_byte_size': len(body), 'received_bytes': received,
        'corpus_payload_fetched': header_only,
        'acquisition_scope': 'bounded metadata prefix; complete source file not retained' if header_only else 'metadata/license',
        'response_digest_scope': 'retained header prefix' if header_only else 'complete response'})


def metadata():
    rows = json.loads((BASE / 'translation-candidate-tree.json').read_text())
    jobs = [('perseus-README.md', 'README.md', False), ('perseus-license.md', 'license.md', False)]
    for row in rows:
        author, work = row['path'].split('/')[1:3]
        jobs.extend([(Path(row['path']).stem + '-header.xml', row['path'], True),
            (author + '.' + work + '-cts.xml', f'data/{author}/{work}/__cts__.xml', False),
            (author + '-cts.xml', f'data/{author}/__cts__.xml', False)])
    jobs = list(dict.fromkeys(jobs))
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(capture, jobs))
    print(json.dumps({'candidate_versions': len(rows), 'metadata_observations': len(jobs),
        'complete_source_files_acquired': 0}))


def prepare():
    sys.path.insert(0, str(ROOT / 'scripts'))
    from prepare_registry_sources import prepare_package
    from prepare_philosophy_source_planting import prepare_anchor
    if not (BASE / 'SOURCE_AND_RIGHTS_REVIEW.md').is_file():
        raise ValueError('source-visible translation and rights review required')
    source = ROOT / 'ToS/research-packets/source-registries'
    snapshot_id = json.loads((source / 'current.json').read_text())['snapshot_id']
    snapshot = source / 'snapshots' / snapshot_id / 'snapshot.json'
    snapshot_ref = snapshot.relative_to(ROOT).as_posix()
    leads = []
    for document in ('A25', 'A26', 'A29'):
        path = snapshot.parent / 'documents/table-i' / (document + '.json.gz')
        for record in json.load(gzip.open(path, 'rt'))['records']:
            if record['kind'] != 'registry':
                continue
            leads.append({'document_ref': path.relative_to(ROOT).as_posix(), 'document_id': document,
                'source_record_id': record['source_record_id'], 'record_id': record['record_id'],
                'source': record['source'], 'reported_fields': {field['source_field']: field['value'] for field in record['reported_fields']}})
    write(BASE / 'registry-candidate-leads.json', {'status': 'reported-research-leads-not-source-admission',
        'snapshot_ref': snapshot_ref, 'records': leads})
    prior_targets = {}
    prior_plans = {}
    for batch, plan_name in [('registry-fifth-planting-2026-09-09', 'roman-philosophical-latin-editions-20260909.json')]:
        old = json.loads((BASE.parent / batch / 'manifest.json').read_text())
        plans = json.loads((ROOT / 'ToS/philosophy/source-planting-preparation' / plan_name).read_text())
        prior_plans.update({plan['target_slug']: plan for plan in plans['targets']})
        for target in old['targets']:
            work_urn = target['coverage']['cts_urn'].rsplit('.', 1)[0]
            prior_targets[work_urn] = target
    candidates = json.loads((BASE / 'translation-candidate-tree.json').read_text())
    selected, alternatives, groups = [], [], {}
    for row in candidates:
        work_urn = 'urn:cts:latinLit:' + '.'.join(Path(row['path']).name.split('.')[:2])
        groups.setdefault(work_urn, []).append(row)
    # One supplied English version exists for each of these five prior Works.
    for work_urn, rows in sorted(groups.items()):
        ordered = sorted(rows, key=lambda row: row['path'])
        selected.append((work_urn, ordered[0]))
        for row in ordered[1:]:
            alternatives.append({'upstream_path': row['path'], 'decision': 'defer',
                'owner': 'ToS/source-witnesses/discovery',
                'reason': 'A separate English translation of the same Work is retained as metadata; this pass adds one parallel reading version per Work.',
                'next_condition': 'Review the additional translation as a distinct Expression/Edition in a future version-selection pass.'})
    missing = [{'work_id': old['ids']['work'], 'work_ref': old['paths']['work'], 'cts_work': urn,
        'status': 'no-English-file-in-this-pinned-repository-view',
        'next_owner': 'ToS/source-witnesses/discovery',
        'next_condition': 'Query other exact providers or a later reviewed snapshot; do not infer global absence.'}
        for urn, old in prior_targets.items() if urn not in groups]
    template = BASE.parent / 'registry-third-planting-2026-09-08/prepared-source-packages.jsonl'
    rights_template = json.loads(template.read_text().splitlines()[0])['rights']
    ns = {'t': 'http://www.tei-c.org/ns/1.0', 'c': 'http://chs.harvard.edu/xmlns/cts'}
    xml_lang = '{http://www.w3.org/XML/1998/namespace}lang'
    observed = datetime.now(timezone.utc).isoformat()
    targets, packages, plans, reviews = [], [], [], []
    for work_urn, row in selected:
        old = prior_targets[work_urn]
        work_slug, family = old['slug'], old['family']
        slug = work_slug + '-english'
        author, work_code = row['path'].split('/')[1:3]
        urn = 'urn:cts:latinLit:' + Path(row['path']).stem
        cts_ref = f'{REL}/evidence/{author}.{work_code}-cts.xml'
        cts = ET.parse(ROOT / cts_ref).getroot()
        translations = [node for node in cts if node.get('urn') == urn]
        if cts.get('urn') != work_urn or len(translations) != 1:
            raise ValueError('translation does not resolve to the existing exact CTS Work')
        translation = translations[0]
        if translation.tag != '{http://chs.harvard.edu/xmlns/cts}translation' or translation.get('workUrn') != work_urn:
            raise ValueError('CTS does not declare this version as an English translation of this Work')
        language_assessment = None
        if translation.get(xml_lang) != 'eng':
            assessments = json.loads((BASE / 'metadata-language-assessments.json').read_text())['assessments']
            matching = [assessment for assessment in assessments if assessment['translation_urn'] == urn]
            if len(matching) != 1 or matching[0]['status'] != 'source-visible-language-reviewed':
                raise ValueError('conflicting CTS language requires an exact source-visible assessment')
            language_assessment = matching[0]
            prefix = (ROOT / language_assessment['body_prefix_ref']).read_bytes()
            if digest(prefix) != language_assessment['body_prefix_sha256'] or language_assessment['reported_cts_language'] != translation.get(xml_lang):
                raise ValueError('language assessment evidence no longer matches')
            parser = ET.XMLPullParser(['start'])
            parser.feed(prefix)
            divisions = [element for _, element in parser.read_events() if element.tag == '{http://www.tei-c.org/ns/1.0}div'
                and element.get('type') in ('edition', 'translation')]
            if not divisions or divisions[0].get('n') != urn or divisions[0].get('type') != 'translation' or divisions[0].get(xml_lang) != 'eng':
                raise ValueError('source envelope does not support the reviewed English translation language')
        title = ' '.join(translation.find('c:label', ns).itertext()).strip()
        description = ' '.join(translation.find('c:description', ns).itertext()).strip()
        header_ref = f'{REL}/evidence/{Path(row["path"]).stem}-header.xml'
        header = (ROOT / header_ref).read_bytes()
        xml = ET.fromstring(header + b'\n</TEI>')
        text = lambda tag: ' '.join(' '.join(' '.join(node.itertext()).split()) for node in xml.findall('.//t:' + tag, ns))
        source_description, responsibility = text('sourceDesc'), text('titleStmt')
        opening_ref = f'{REL}/evidence/{Path(row["path"]).stem}-opening.xml'
        opening = (ROOT / opening_ref).read_bytes()
        pull = ET.XMLPullParser(['start']); pull.feed(opening)
        elements = [node for _, node in pull.read_events()]
        envelopes = [node for node in elements if node.tag == '{'+ns['t']+'}div' and node.get('type') in ('edition', 'translation')]
        bodies = [node for node in elements if node.tag == '{'+ns['t']+'}body']
        if len(envelopes) != 1 or len(bodies) != 1 or envelopes[0].get('type') != 'translation' or envelopes[0].get(xml_lang) != 'eng':
            raise ValueError('reviewed English body envelope required')
        if bodies[0].get('{http://www.w3.org/XML/1998/namespace}base') != urn or envelopes[0].get('n') not in (None, urn):
            raise ValueError('reviewed translation XML identity differs')
        work_ref = old['paths']['work']
        before = (ROOT / work_ref).read_bytes()
        work_record = json.loads(before)
        if work_record['record_id'] != old['ids']['work'] or not any(
                identity['scheme'] == 'CTS' and identity['value'] == work_urn
                for identity in work_record['external_identifiers']):
            raise ValueError('existing owner Work does not carry the reviewed exact CTS identity')
        expression = 'en-perseus-' + Path(row['path']).stem.replace('.', '-')
        edition = 'perseus-' + PIN[:12]
        item = 'git-tei-xml'
        work_root = str(Path(work_ref).parent)
        expression_root = f'{work_root}/expressions/{expression}'
        edition_root = f'{expression_root}/editions/{edition}'
        item_root = f'{edition_root}/items/{item}'
        identity = family + '.' + work_slug
        ids = {'work': old['ids']['work'], 'expression': f'tos.expression.{identity}.{expression}',
            'edition': f'tos.edition.{identity}.{expression}.{edition}',
            'item': f'tos.item.{identity}.{expression}.{edition}.{item}'}
        paths = {'work': work_ref, 'expression': expression_root + '/expression.json',
            'edition': edition_root + '/edition.json', 'item': item_root + '/item.json', 'item_root': item_root}
        if any((ROOT / path).exists() for key, path in paths.items() if key != 'work'):
            raise ValueError('English target is already present; no replacement is allowed')
        prior_plan = prior_plans[work_slug]
        old_anchor = prior_plan['anchor_preparation']
        document = old_anchor['atlas_row_id']
        candidates_for_work = [lead for lead in leads if lead['document_id'] == document and
            lead['reported_fields'].get('stable_identifier') in (urn, work_urn) and
            any(value.get('language_code') == 'en' for value in (lead['reported_fields'].get('language') or []) if isinstance(value, dict))]
        if not candidates_for_work:
            fallback = {'cicero': 'A29-R027', 'lucretius': 'A29-R041'}[family]
            candidates_for_work = [lead for lead in leads if lead['source_record_id'] == fallback]
        if not candidates_for_work:
            raise ValueError('no language-compatible source-registry anchor')
        sources = [{'corpus': 'table-i', 'document_id': document, 'original_source_id': lead['source_record_id'],
            'entry_id': lead['record_id'], 'source_locator': lead['source']} for lead in candidates_for_work]
        limits = ['The acquired file is an English translation, distinct from the previously retained Latin Expression and Edition.',
            'Shared CTS Work identity is not evidence that this translation used the particular Latin edition held by ToS; no exact translation-from or passage alignment is asserted.',
            'Translator, editor, printing dates and ancillary language statements remain the literal supplied edition metadata.',
            'Complete pinned supplied file only; no independent completeness, accuracy, ancient-authorship or critical-edition judgment.',
            'Latin and other-language quotations, introductions and notes inside the file remain distinct supplied editorial layers.',
            'Local custody, branch routes and parallel reading do not admit translation quality, philosophical meaning or canon.',
            'The parallel Latin version retains its own source uncertainties; those are not transferred to this translation bibliography.']
        refs = [f'{REL}/manifest.json', f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md', cts_ref, header_ref, opening_ref,
            f'{REL}/evidence/{author}-cts.xml', f'{REL}/evidence/perseus-README.md',
            f'{REL}/evidence/perseus-license.md', f'https://github.com/{REPOSITORY}/tree/{PIN}']
        if language_assessment is not None:
            refs.extend([f'{REL}/metadata-language-assessments.json', language_assessment['body_prefix_ref']])
            limits.append('Upstream CTS metadata labels this English translation grc; the contradictory label remains retained, while the actual TEI translation envelope and source-visible English opening support the en Expression. No upstream bytes were corrected.')
        target = {'slug': slug, 'work_slug': work_slug, 'title': title, 'work_title': work_record['preferred_label'],
            'family': family, 'provider': 'perseus', 'repository': REPOSITORY, 'pin': PIN,
            'language': 'en', 'script': 'Latn', 'expression_role': 'translation', 'operation_date': '2026-09-09',
            'expression': expression, 'edition': edition, 'item': item, 'registry_sources': sources,
            'branch_target_slug': slug, 'ids': ids, 'paths': paths,
            'parallel_source': {'work_ref': work_ref, 'expression_ref': old['paths']['expression'],
                'edition_ref': old['paths']['edition'], 'item_manifest_ref': old['paths']['item_root'] + '/item.manifest.json',
                'files': [old['paths']['item_root'] + '/payload/' + entry['basename'] for entry in old['files']],
                'relationship_limit': 'Parallel versions of the same CTS Work; no exact translation-source or segment-alignment claim.'},
            'coverage': {'kind': 'perseus-tei-translation', 'citation_scope': 'hierarchical_divisions_and_section_milestones',
                'cts_urn': urn, 'identity_anchor': 'body_xml_base', 'header_prefix_sha256': digest(header), 'reviewed_body_prefix_sha256': digest(opening), 'reviewed_body_prefix_bytes': len(opening)},
            'version_description': f'English translation {urn}. {description} Pinned digital file: {REPOSITORY}@{PIN}. Supplied source description: {source_description}',
            'responsibility': 'Supplied title/contributor statement: ' + responsibility + '. CTS translation description: ' + description,
            'limits': limits, 'metadata_evidence_refs': [ref for ref in refs if ref.startswith(REL + '/evidence/')],
            'files': [{'upstream_path': row['path'], 'basename': Path(row['path']).name,
                'git_blob_sha1': row['sha'], 'byte_size': row['size'], 'media_type': 'application/tei+xml',
                'url': f'https://raw.githubusercontent.com/{REPOSITORY}/{PIN}/{row["path"]}'}], 'byte_size': row['size']}
        if language_assessment is not None:
            target['language_assessment_ref'] = f'{REL}/metadata-language-assessments.json'
            target['coverage'].update(reviewed_body_prefix_sha256=language_assessment['body_prefix_sha256'],
                reviewed_body_prefix_bytes=len(prefix))
        rights = copy.deepcopy(rights_template)
        rights_id = 'tos.rights.' + ids['item'].removeprefix('tos.item.')
        rights.update(rights_id=rights_id, scope_refs=[ids['item']], source_refs=refs + [rights['license_uri']],
            assessed_at=observed, review_refs=[f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md'],
            rationale='Positive repository CC BY-SA 4.0 grant, assessed against this exact translation header and CTS declaration. No conflicting exception observed. Local acquisition relies on the licensed digital text and translation layers, not age of the ancient Work or a presumed translation copyright expiry. Supplier authority remains the limit.')
        for layer in rights['layer_assessments']:
            name = layer['layer_id'].rsplit('.', 1)[1]
            layer.update(layer_id=rights_id + '.layer.' + name, source_refs=rights['source_refs'], assessed_at=observed,
                scope_refs=[ids['expression' if name == 'digital-text' else 'edition' if name == 'edition' else 'item']])
            if name == 'digital-text':
                layer['layer_role'] = 'translation'
                layer['rationale'] = 'This supplied English translation layer is included in the exact digital text license; it is not treated as the ancient Latin Work or as public domain by age.'
        package = prepare_package(target, observed, evidence_refs=refs, rights_assessment=rights)
        additions = package['records'][work_ref]['expression_claim_refs']
        expected = copy.deepcopy(work_record)
        expected['expression_claim_refs'] += additions
        expected['record_version'] += 1
        package['records'][work_ref] = expected
        before_ref = f'{REL}/work-before/{digest(before)}.json'
        before_path = ROOT / before_ref
        before_path.parent.mkdir(parents=True, exist_ok=True)
        if before_path.exists() and before_path.read_bytes() != before:
            raise ValueError('immutable Work preimage collision')
        before_path.write_bytes(before)
        package['existing_work'] = {'record_ref': work_ref, 'preimage_ref': before_ref, 'sha256': digest(before)}
        for kind in ('expression', 'edition', 'item'):
            package['records'][paths[kind]]['external_identifiers'] = [{'scheme': 'CTS', 'value': urn,
                'source_ref': cts_ref, 'status': 'verified'}]
        anchor_source = old_anchor['source_backlog_anchor']
        anchor = prepare_anchor(ROOT, atlas_row_id=document, source_table_index=anchor_source['source_table_index'],
            source_row_index=anchor_source['source_row_index'], source_label=anchor_source['source_label'])
        plans.append({'target_slug': slug, 'registry_source_record_id': sources[0]['original_source_id'],
            'anchor_preparation': anchor, 'scope_relationship': 'parallel-translation-of-existing-work',
            'scope_rationale': f'English reading version of {work_record["preferred_label"]} alongside the already retained Latin version within the {anchor_source["source_label"]} source need. The corpus lead and other versions remain open.',
            'remaining_controls': limits})
        reviews.append({'upstream_path': row['path'], 'decision': 'select', 'existing_work_ref': work_ref,
            'translation_urn': urn, 'cts_description': description, 'header_publication_statement': text('publicationStmt'),
            'header_notes': text('notesStmt'), 'ancillary_language_statement': text('langUsage'),
            'reason': 'Exact CTS translation-to-Work declaration, separately retained translator/editor metadata, licensed local-use basis, current Work preimage and compatible branch/registry anchor reviewed.'})
        targets.append(target)
        packages.append(package)
    if len(targets) != 5 or len({target['ids']['work'] for target in targets}) != 5:
        raise ValueError('reviewed scope must be 5 distinct existing Works and 5 new English versions')
    payload = b''.join((json.dumps(package, ensure_ascii=False, separators=(',', ':')) + '\n').encode() for package in packages)
    (BASE / 'prepared-source-packages.jsonl').write_bytes(payload)
    branch_ref = 'ToS/philosophy/source-planting-preparation/roman-philosophical-english-translations-20260909.json'
    write(ROOT / branch_ref, {'schema_version': 'tos_source_planting_preparation_batch_v1',
        'status': 'prepared-not-planted', 'review_scope': '5 English translations attached to existing Works and A29 branches',
        'reviewer_ref': 'model:codex', 'targets': plans})
    observations = [json.loads(path.read_text()) for path in sorted((BASE / 'evidence').glob('*.receipt.json'))]
    write(BASE / 'selection-review.json', {'reviewer_ref': 'model:codex', 'observed_at': observed,
        'candidates': reviews + alternatives, 'selected': len(targets), 'deferred': len(alternatives),
        'no-English-file-in-pinned-view': missing, 'complete_source_files_acquired': False})
    write(BASE / 'manifest.json', {'schema_version': 'tos_registry_first_planting_preparation_v1',
        'status': 'prepared-not-acquired', 'source_registry_snapshot_ref': snapshot_ref,
        'source_registry_snapshot_sha256': digest(snapshot.read_bytes()), 'branch_preparation_ref': branch_ref,
        'provider_pins': {'perseus': PIN}, 'metadata_observations': observations, 'targets': targets,
        'prepared_packages_ref': f'{REL}/prepared-source-packages.jsonl', 'prepared_packages_sha256': digest(payload),
        'totals': {'works': 0, 'existing_works_extended': len(targets), 'expressions': len(targets),
            'payload_files': len(targets), 'payload_bytes': sum(target['byte_size'] for target in targets),
            'bibliographic_claims': 3 * len(targets)},
        'authority_boundary': 'Preparation is not custody, translation acceptance, source-file publication or canon. Existing Work fields and prior source versions are preserved.'})
    print(json.dumps({'new_works': 0, 'existing_works': len(targets), 'new_English_versions': len(targets),
        'deferred_alternatives': len(alternatives), 'bytes': sum(target['byte_size'] for target in targets)}))


if __name__ == '__main__':
    if sys.argv[1:] == ['metadata']:
        metadata()
    elif sys.argv[1:] == ['prepare']:
        prepare()
    else:
        raise SystemExit('expected metadata or prepare')
