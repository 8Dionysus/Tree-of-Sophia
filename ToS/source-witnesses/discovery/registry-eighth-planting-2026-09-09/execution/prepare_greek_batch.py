"""Eighth planting: bounded source-visible metadata before exact acquisition."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from urllib.request import Request, urlopen
import hashlib
import copy
import json
import re
import sys
import time
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[5]
BASE = Path(__file__).resolve().parents[1]
REL = BASE.relative_to(ROOT).as_posix()
PIN = '341e309c821d5eca8c976bebca77c28b10bad58f'
REPO = 'PerseusDL/canonical-greekLit'
NS = {'t': 'http://www.tei-c.org/ns/1.0', 'c': 'http://chs.harvard.edu/xmlns/cts'}
sha = lambda b: hashlib.sha256(b).hexdigest()
stamp = lambda: datetime.now(timezone.utc).isoformat()
raw = lambda p: f'https://raw.githubusercontent.com/{REPO}/{PIN}/{p}'


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def candidate_files():
    return json.loads((BASE/'candidate-files.json').read_text())


def capture(job):
    name, upstream, header = job
    path = BASE/'evidence'/name
    receipt = path.with_name(path.name+'.receipt.json')
    url = raw(upstream)
    if path.exists():
        value = json.loads(receipt.read_text())
        if value['url'] != url or sha(path.read_bytes()) != value['retained_sha256']:
            raise ValueError('retained metadata differs from its source receipt')
        return
    started = stamp()
    for attempt in range(3):
        try:
            with urlopen(Request(url, headers={'User-Agent': 'Tree-of-Sophia-source-preparation'}), timeout=45) as response:
                body = b''
                if header:
                    while b'</teiHeader>' not in body and len(body) < 100000:
                        part = response.read(4096)
                        if not part: break
                        body += part
                    if b'</teiHeader>' not in body: raise ValueError('bounded TEI header absent')
                    received = len(body)
                    body = body.split(b'</teiHeader>', 1)[0] + b'</teiHeader>'
                else:
                    body = response.read(2_000_001)
                    received = len(body)
                    if received > 2_000_000: raise ValueError('metadata response too large')
                status, final = response.status, response.url
            break
        except (OSError, TimeoutError):
            if attempt == 2: raise
            time.sleep(attempt+1)
    path.write_bytes(body)
    write(receipt, {'url': url, 'final_url': final, 'http_status': status, 'started_at': started,
        'ended_at': stamp(), 'retained_ref': path.relative_to(ROOT).as_posix(),
        'retained_sha256': sha(body), 'retained_byte_size': len(body), 'received_bytes': received,
        'corpus_payload_fetched': header,
        'acquisition_scope': 'metadata prefix only; complete corpus file not retained' if header else 'metadata/license',
        'response_digest_scope': 'retained header prefix' if header else 'complete response'})


def metadata():
    rows = candidate_files()
    jobs = [('perseus-README.md', 'README.md', False), ('perseus-license.md', 'license.md', False)]
    for r in rows:
        if selection_reason(r): continue
        author, work = r['path'].split('/')[1:3]
        jobs += [(Path(r['path']).stem+'-header.xml', r['path'], True),
                 (author+'.'+work+'-cts.xml', f'data/{author}/{work}/__cts__.xml', False),
                 (author+'-cts.xml', f'data/{author}/__cts__.xml', False)]
    jobs = list(dict.fromkeys(jobs))
    with ThreadPoolExecutor(max_workers=4) as pool:
        list(pool.map(capture, jobs))
    print(json.dumps({'candidate_editions': len(rows), 'metadata_observations': len(jobs),
        'prepared_full_file_bytes': sum(r['size'] for r in rows)}))


def selection_reason(row):
    work = row['path'].split('/')[2]
    if work in ('tlg112', 'tlg114', 'tlg121', 'tlg131', 'tlg132'):
        return 'Defer unresolved aggregate/constituent or numbered-part identity: source-witnesses owner must prepare constituent/membership or part-to-whole identity before acquisition; no undifferentiated Work will stand in for that decision.'
    if work == 'tlg137':
        return 'Defer the Compendium: existing A26-R061 identifier/title conflict remains owned by source-registry review; do not resolve it through intake.'
    if work in ('tlg081', 'tlg082', 'tlg082a', 'tlg082b', 'tlg083', 'tlg084a', 'tlg084b', 'tlg085', 'tlg086', 'tlg087', 'tlg088'):
        return 'Defer paired editions and apophthegm/parallel narrative group: owner source-witnesses must review collection/member identity and exact version before a separate intake.'
    return None


def prepare():
    sys.path.insert(0, str(ROOT/'scripts'))
    from prepare_registry_sources import prepare_package
    from prepare_philosophy_source_planting import prepare_anchor
    from source_registry_common import read
    if not (BASE/'SOURCE_AND_RIGHTS_REVIEW.md').is_file():
        raise ValueError('source-visible identity, branch and rights review required')
    leads = read(BASE/'registry-candidate-leads.json')
    snapshot = ROOT/leads['snapshot_ref']
    template_ref = ROOT/'ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/prepared-source-packages.jsonl'
    rights_template = json.loads(template_ref.read_text().splitlines()[0])['rights']
    families = {'tlg0059': ('plato', 'A25', 14, 1, 'Scaife Viewer / Perseus', 'A25-R001'),
        'tlg0086': ('aristotle', 'A25', 14, 1, 'Scaife Viewer / Perseus', 'A25-R002'),
        'tlg0007': ('plutarch', 'A29', 14, 10, 'Плутарх', 'A29-R113'),
        'tlg0557': ('epictetus', 'A29', 14, 9, 'Эпиктет', 'A29-R107'),
        'tlg0562': ('marcus-aurelius', 'A29', 14, 11, 'Meditations', 'A29-R138'),
        'tlg0004': ('diogenes-laertius', 'A26', 12, 1, 'Diogenes Laertius, Lives Books 6–10', 'A26-R001')}
    targets, packages, plans, selections = [], [], [], []
    observed = stamp()
    for row in candidate_files():
        reason = selection_reason(row)
        selections.append({'upstream_path': row['path'], 'decision': 'defer' if reason else 'select',
            'reason': reason or 'Exact edition, source-visible licensed-use basis and existing philosophical corpus anchor reviewed.'})
        if reason: continue
        author, work = row['path'].split('/')[1:3]
        family, document, table, source_row, source_label, fallback = families[author]
        work_urn = 'urn:cts:greekLit:'+author+'.'+work
        urn = 'urn:cts:greekLit:'+Path(row['path']).stem
        cts_ref = f'{REL}/evidence/{author}.{work}-cts.xml'
        cts = ET.parse(ROOT/cts_ref).getroot()
        if cts.get('urn') != work_urn or not any(n.get('urn') == urn for n in cts):
            raise ValueError('CTS Work/Edition identity is not explicitly declared')
        title_node = cts.find('c:title', NS)
        title = ' '.join(title_node.itertext()).strip()
        title_language = {'eng': 'en', 'lat': 'la'}[title_node.get('{http://www.w3.org/XML/1998/namespace}lang')]
        slug = re.sub('[^a-z0-9]+', '-', title.lower()).strip('-')
        header_ref = f'{REL}/evidence/{Path(row["path"]).stem}-header.xml'
        header = (ROOT/header_ref).read_bytes()
        xml = ET.fromstring(header+b'\n</TEI>')
        supplied = lambda tag: ' '.join(' '.join(n.itertext()) for n in xml.findall('.//t:'+tag, NS)).split()
        source_description = ' '.join(supplied('sourceDesc'))
        contributors = ' '.join(supplied('titleStmt'))
        matches = [r for r in leads['records'] if r['document_id'] == document and
            r['reported_fields'].get('stable_identifier') in (work_urn, urn)]
        if not matches: matches = [r for r in leads['records'] if r['source_record_id'] == fallback]
        if not matches: raise ValueError('registry lead missing')
        sources = [{'corpus': 'table-i', 'document_id': document, 'original_source_id': r['source_record_id'],
            'entry_id': r['record_id'], 'source_locator': r['source']} for r in matches]
        exp = 'grc-perseus-'+author+'-'+work
        edition = 'perseus-'+PIN[:12]
        item = 'git-tei-xml'
        w = f'ToS/source-witnesses/works/{family}/{slug}'
        e = f'{w}/expressions/{exp}'; d = f'{e}/editions/{edition}'; it = f'{d}/items/{item}'
        identity = family+'.'+slug
        ids = {'work': 'tos.work.'+identity, 'expression': f'tos.expression.{identity}.{exp}',
            'edition': f'tos.edition.{identity}.{exp}.{edition}', 'item': f'tos.item.{identity}.{exp}.{edition}.{item}'}
        paths = {'work': w+'/work.json', 'expression': e+'/expression.json', 'edition': d+'/edition.json',
            'item': it+'/item.json', 'item_root': it}
        if any((ROOT/p).exists() for p in paths.values()): raise ValueError('target already present; no replacement allowed')
        limits = ['The exact supplied Greek file is a source-language edition, not a manuscript facsimile or ToS critical reconstruction.',
            'Repository author grouping and header attribution remain supplier assertions; no ancient author, chronology or doctrine is admitted.',
            'Exact file completeness does not establish critical completeness, correctness of readings or coverage of other editions/translations.',
            'Narrative, dramatic speakers, reported doctrines and polemical reports are not automatically positions of the named author.']
        if family == 'plato': limits += ['Authenticity remains open for disputed/spurious members of the transmitted Platonic corpus, including Alcibiades, Hipparchus, Lovers, Theages, Minos and Epinomis.']
        if family == 'aristotle': limits += ['Economics and On Virtues and Vices retain traditional/Pseudo-Aristotelian attribution; the provider grouping does not resolve authorship or editorial compilation.']
        if family == 'epictetus': limits += ['Arrian recording/editing/epitomizing and the earlier oral teaching remain distinct transmission layers.']
        if family == 'plutarch': limits += ['Reports of other schools are transmitting/polemical testimony, not accepted statements of those schools.', 'Traditional Plutarch attribution includes disputed and pseudonymous works; no authorship verdict is made, including De liberis educandis, Consolatio ad Apollonium and De fato.', 'The supplied Latin title is retained literally, including provider spellings; title normalization and person-name equivalence remain unreviewed.']
        if author == 'tlg0007' and work == 'tlg091': limits += ['Supplied print-volume discrepancy unresolved: CTS description says Moralia Vol II, while TEI sourceDesc says volume 3, both 1891. Exact digital CTS/Git identity is fixed; neither print-volume statement is adjudicated.']
        if author == 'tlg0007' and work == 'tlg122': limits += ['This transmitted Compendium is a distinct epitome witness; it is not the lost longer comparison and does not establish the wording or completeness of that earlier work.']
        if family == 'diogenes-laertius': limits += ['The entire ten-book supplied edition provides the selected Books 6–10 branch context; individual quotations, letters and school reports are not separately identified or admitted by this intake.']
        refs = [f'{REL}/manifest.json', f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md', cts_ref, header_ref,
            f'{REL}/evidence/{author}-cts.xml', f'{REL}/evidence/perseus-README.md', f'{REL}/evidence/perseus-license.md',
            f'https://github.com/{REPO}/tree/{PIN}']
        target = {'slug': slug, 'title': title, 'family': family, 'provider': 'perseus', 'repository': REPO, 'pin': PIN,
            'operation_date': '2026-09-09', 'language': 'grc', 'script': 'Grek', 'expression': exp, 'edition': edition, 'item': item,
            'registry_sources': sources, 'branch_target_slug': slug, 'ids': ids, 'paths': paths,
            'coverage': {'kind': 'perseus-tei-work', 'citation_scope': 'hierarchical_divisions', 'cts_urn': urn, 'header_prefix_sha256': sha(header)},
            'version_description': f'Perseus Ancient Greek TEI edition {urn}, Git {PIN}. Supplied print/source description: {source_description}',
            'responsibility': 'Supplied title and contributor statement: '+contributors+'. Preserve the exact source header; its claims remain reported.',
            'limits': limits, 'metadata_evidence_refs': [r for r in refs if r.startswith(REL+'/evidence/')],
            'files': [{'upstream_path': row['path'], 'basename': Path(row['path']).name, 'git_blob_sha1': row['sha'],
                'byte_size': row['size'], 'media_type': 'application/tei+xml', 'url': raw(row['path'])}], 'byte_size': row['size']}
        rights = copy.deepcopy(rights_template)
        rid = 'tos.rights.'+ids['item'].removeprefix('tos.item.')
        rights.update(rights_id=rid, scope_refs=[ids['item']], source_refs=refs+[rights['license_uri']], assessed_at=observed,
            review_refs=[f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md'], rationale='Positive provider CC BY-SA 4.0 grant, together with the separately read exact header and CTS metadata. No conflicting exception observed. Supplier authority and local operation only; no copyright-term inference or source-text acceptance.')
        for layer in rights['layer_assessments']:
            name = layer['layer_id'].rsplit('.', 1)[1]
            layer.update(layer_id=rid+'.layer.'+name, source_refs=rights['source_refs'], assessed_at=observed,
                scope_refs=[ids['expression' if name == 'digital-text' else 'edition' if name == 'edition' else 'item']])
        package = prepare_package(target, observed, evidence_refs=refs, rights_assessment=rights)
        package['records'][paths['work']]['field_languages']['preferred_label']['language'] = title_language
        for kind in ('work', 'expression', 'edition', 'item'):
            package['records'][paths[kind]]['external_identifiers'] = [{'scheme': 'CTS', 'value': work_urn if kind == 'work' else urn,
                'source_ref': cts_ref, 'status': 'verified'}]
        anchor = prepare_anchor(ROOT, atlas_row_id=document, source_table_index=table, source_row_index=source_row, source_label=source_label)
        plans.append({'target_slug': slug, 'registry_source_record_id': sources[0]['original_source_id'], 'anchor_preparation': anchor,
            'scope_relationship': 'bounded-constituent-work', 'scope_rationale': f'Exact {title} Greek edition within the reviewed {source_label} source need; the broader corpus and other versions stay open.',
            'remaining_controls': limits})
        targets.append(target); packages.append(package)
    if len(targets) != 45 or len({t['slug'] for t in targets}) != len(targets):
        raise ValueError('selected count/identity differs from reviewed 45-edition scope')
    content = b''.join((json.dumps(p, ensure_ascii=False, separators=(',', ':'))+'\n').encode() for p in packages)
    (BASE/'prepared-source-packages.jsonl').write_bytes(content)
    branch_ref = 'ToS/philosophy/source-planting-preparation/plutarch-moralia-greek-20260909.json'
    write(ROOT/branch_ref, {'schema_version': 'tos_source_planting_preparation_batch_v1', 'status': 'prepared-not-planted',
        'review_scope': 'Exact supplied edition, attribution limits and existing A25/A26/A29 corpus anchors', 'reviewer_ref': 'model:codex', 'targets': plans})
    observations = []
    for p in sorted((BASE/'evidence').glob('*.receipt.json')):
        value = read(p); body_path = p.with_name(p.name.removesuffix('.receipt.json'))
        value.setdefault('retained_ref', body_path.relative_to(ROOT).as_posix())
        value.setdefault('started_at', value.get('observed_at')); value.setdefault('ended_at', value['started_at'])
        if value.get('started_at') and value.get('ended_at'):
            value.setdefault('elapsed_seconds', (datetime.fromisoformat(value['ended_at'])-datetime.fromisoformat(value['started_at'])).total_seconds())
            value.setdefault('elapsed_seconds_basis', 'difference between the retained observation timestamps')
        observations.append(value)
    write(BASE/'selection-review.json', {'reviewer_ref': 'model:codex', 'observed_at': observed, 'candidates': selections,
        'selected': len(targets), 'deferred': sum(r['decision'] == 'defer' for r in selections), 'new_source_bodies_acquired': False})
    write(BASE/'manifest.json', {'schema_version': 'tos_registry_first_planting_preparation_v1', 'status': 'prepared-not-acquired',
        'source_registry_snapshot_ref': leads['snapshot_ref'], 'source_registry_snapshot_sha256': sha(snapshot.read_bytes()),
        'branch_preparation_ref': branch_ref, 'provider_pins': {'perseus': PIN}, 'metadata_observations': observations,
        'targets': targets, 'prepared_packages_ref': f'{REL}/prepared-source-packages.jsonl', 'prepared_packages_sha256': sha(content),
        'totals': {'works': len(targets), 'expressions': len(targets), 'payload_files': len(targets),
            'payload_bytes': sum(t['byte_size'] for t in targets), 'bibliographic_claims': 3*len(targets)},
        'authority_boundary': 'Preparation proves no local source custody, textual/semantic acceptance, canon or source-file publication.'})
    print(json.dumps({'selected': len(targets), 'deferred': len(selections)-len(targets), 'bytes': sum(t['byte_size'] for t in targets)}))


if __name__ == '__main__':
    if sys.argv[1:] == ['metadata']: metadata()
    elif sys.argv[1:] == ['prepare']: prepare()
    else: raise SystemExit('choose metadata or prepare')
