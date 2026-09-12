"""Twelfth planting: Latin source editions with exact reviewed identity carriers."""
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
PIN = '8668397510ca77fec6406f7d48f405e50d296764'
REPO = 'PerseusDL/canonical-latinLit'
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
    work=row['path'].split('/')[2]
    if work in ('phi005','phi011','phi013','phi035'):
        return 'Speech cycle: A29 source-witness owner must review constituent speech identities and collection relations before full acquisition.'
    if work == 'phi046':
        return 'Lucullus: preserve Roman Latin-edition deferral; A29 source-witness owner must resolve Academica redaction and part relations before a new Work or Expression.'
    if work == 'phi053':
        return 'Alternative De divinatione version: existing Work already retained; source-witness owner must prepare a version extension and legacy TEI.2 adapter review.'
    if work in ('phi056','phi057','phi058','phi059'):
        return 'Letter collection: source-witness owner must review collection and constituent letter identities before full acquisition.'
    if work == 'phi072':
        return 'Cicero Timaeus: source-witness owner must review Latin translation and adaptation identity against the existing Plato Work before a separate version intake.'
    if int(work[3:]) not in range(1,36):raise ValueError('unreviewed candidate')
    return None


def openings():
    """Retain small incomplete prefixes for source-visible language/extent review."""
    jobs=[r for r in candidate_files() if not selection_reason(r)]
    for row in jobs:
        stem=Path(row['path']).stem
        dest=BASE/'evidence'/(stem+'-opening.xml')
        if dest.exists(): continue
        header=(BASE/'evidence'/(stem+'-header.xml')).read_bytes()
        size=min(len(header)+4096,row['size']-1)
        start=stamp()
        with urlopen(Request(raw(row['path']),headers={'User-Agent':'Tree-of-Sophia-source-preparation'}),timeout=45) as response:
            body=response.read(size);status,final=response.status,response.url
        if len(body)!=size or not body.startswith(header):raise ValueError('source opening differs from retained header')
        dest.write_bytes(body)
        write(dest.with_name(dest.name+'.receipt.json'),{'url':raw(row['path']),'final_url':final,'http_status':status,
            'started_at':start,'ended_at':stamp(),'retained_ref':dest.relative_to(ROOT).as_posix(),
            'retained_sha256':sha(body),'retained_byte_size':len(body),'received_bytes':len(body),
            'corpus_payload_fetched':True,'acquisition_scope':'bounded incomplete opening only; not complete source custody',
            'response_digest_scope':'retained incomplete prefix'})
    print(json.dumps({'review_openings':len(jobs),'complete_files_retained':False}))


def prepare():
    import gzip
    sys.path.insert(0,str(ROOT/'scripts'))
    from prepare_registry_sources import prepare_package
    from prepare_philosophy_source_planting import prepare_anchor
    from source_registry_common import read
    if not (BASE/'SOURCE_AND_RIGHTS_REVIEW.md').is_file():raise ValueError('source-visible review required')
    source=ROOT/'ToS/research-packets/source-registries'
    snapshot=source/'snapshots'/read(source/'current.json')['snapshot_id']/'snapshot.json'
    leads=[]
    for document in ('A26','A29'):
        path=snapshot.parent/'documents/table-i'/(document+'.json.gz')
        for record in json.load(gzip.open(path,'rt'))['records']:
            if record['kind']!='registry':continue
            leads.append({'document_ref':path.relative_to(ROOT).as_posix(),'document_id':document,
                'source_record_id':record['source_record_id'],'record_id':record['record_id'],'source':record['source'],
                'reported_fields':{field['source_field']:field['value'] for field in record['reported_fields']}})
    write(BASE/'registry-candidate-leads.json',{'status':'reported-research-leads-not-source-admission',
        'snapshot_ref':snapshot.relative_to(ROOT).as_posix(),'records':leads})
    old=ROOT/'ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/prepared-source-packages.jsonl'
    rights_template=json.loads(old.read_text().splitlines()[0])['rights']
    families={'phi0474':('cicero',6,'Цицероновский корпус','A29-R027')}
    targets,packages,plans,selections=[],[],[],[];observed=stamp()
    for row in candidate_files():
        reason=selection_reason(row)
        selections.append({'upstream_path':row['path'],'decision':'defer' if reason else 'select',
            'owner':'ToS/source-witnesses with A29 branch source owner',
            'reason':reason or 'Exact Latin edition and philosophical/rhetorical source need reviewed; positive supplier license assessed separately.'})
        if reason:continue
        author,work=row['path'].split('/')[1:3];family,source_row,source_label,fallback=families[author]
        work_urn='urn:cts:latinLit:'+author+'.'+work;urn='urn:cts:latinLit:'+Path(row['path']).stem
        cts_ref=f'{REL}/evidence/{author}.{work}-cts.xml';cts=ET.parse(ROOT/cts_ref).getroot()
        editions=[n for n in cts if n.get('urn')==urn]
        if cts.get('urn')!=work_urn or len(editions)!=1 or editions[0].tag!='{'+NS['c']+'}edition':raise ValueError('explicit CTS Work/edition identity required')
        if editions[0].get('{http://www.w3.org/XML/1998/namespace}lang',cts.get('{http://www.w3.org/XML/1998/namespace}lang')) not in ('la','lat'):raise ValueError('CTS source language differs')
        node=cts.find('c:title',NS);title=' '.join(' '.join(node.itertext()).split());title_language={'lat':'la','eng':'en'}[node.get('{http://www.w3.org/XML/1998/namespace}lang')]
        work_slug=re.sub('[^a-z0-9]+','-',title.lower()).strip('-');slug=family+'-'+work_slug+'-latin';stem=Path(row['path']).stem
        header_ref=f'{REL}/evidence/{stem}-header.xml';header=(ROOT/header_ref).read_bytes();xml=ET.fromstring(header+b'</TEI>')
        supplied=lambda tag:' '.join(' '.join(' '.join(n.itertext()) for n in xml.findall('.//t:'+tag,NS)).split())
        source_description,contributors=supplied('sourceDesc'),supplied('titleStmt')
        opening_ref=f'{REL}/evidence/{stem}-opening.xml';opening=(ROOT/opening_ref).read_bytes()
        pull=ET.XMLPullParser(events=('start',));pull.feed(opening)
        nodes=[node for _,node in pull.read_events()]
        envelopes=[node for node in nodes if node.tag=='{'+NS['t']+'}div' and node.get('type') in ('edition','translation')]
        bodies=[node for node in nodes if node.tag=='{'+NS['t']+'}body']
        if len(envelopes)!=1 or len(bodies)!=1:raise ValueError('source must have one reviewed body/edition envelope')
        edition_identity=envelopes[0].get('n');body_identity=bodies[0].get('{http://www.w3.org/XML/1998/namespace}base')
        identity_anchor='edition_n' if edition_identity is not None else 'body_xml_base'
        if (edition_identity or body_identity)!=urn or any(value is not None and value!=urn for value in (edition_identity,body_identity)):raise ValueError('Latin source identity carrier missing or conflicting')
        if envelopes[0].get('type')!='edition' or envelopes[0].get('{http://www.w3.org/XML/1998/namespace}lang') not in ('la','lat'):raise ValueError('source-visible Latin language/role differs')
        matches=[r for r in leads if r['document_id']=='A29' and r['reported_fields'].get('stable_identifier') in (work_urn,urn)]
        if not matches:matches=[r for r in leads if r['source_record_id']==fallback]
        sources=[{'corpus':'table-i','document_id':r['document_id'],'original_source_id':r['source_record_id'],'entry_id':r['record_id'],'source_locator':r['source']} for r in matches]
        if not sources:raise ValueError('registry lead missing')
        exp='la-perseus-'+author+'-'+work;edition='perseus-'+PIN[:12];item='git-tei-xml'
        w=f'ToS/source-witnesses/works/{family}/{work_slug}';e=f'{w}/expressions/{exp}';d=f'{e}/editions/{edition}';it=f'{d}/items/{item}';identity=family+'.'+work_slug
        ids={'work':'tos.work.'+identity,'expression':f'tos.expression.{identity}.{exp}',
            'edition':f'tos.edition.{identity}.{exp}.{edition}','item':f'tos.item.{identity}.{exp}.{edition}.{item}'}
        paths={'work':w+'/work.json','expression':e+'/expression.json','edition':d+'/edition.json','item':it+'/item.json','item_root':it}
        if any((ROOT/p).exists() for p in paths.values()):raise ValueError('existing identity requires a distinct reviewed extension')
        limits=['The exact supplied Latin digital edition is not a manuscript facsimile or a ToS critical reconstruction.',
            'Supplier attribution, editor, date and language statements are retained without ancient authorship or chronology admission.',
            'Complete pinned file does not mean complete ancient work, critical completeness, correct readings or complete coverage of other versions.',
            'Greek or modern-language quotations/notes remain embedded source material, not separately admitted translations.',
            'Dramatic speakers, objections, reported schools, rhetorical techniques and political recommendations are not automatically the author\'s own doctrine.']
        limits += ['Forensic advocacy, accusations, praise and political argument remain the speech witness, not accepted historical fact or author doctrine.']
        if work=='phi018':limits+=['Cum Populo Gratias Egit metadata conflict: CTS description reports 1911; the exact TEI header sourceDesc reports 1909, volume 5. Both supplier statements are preserved without silently choosing a historical publication date.']
        if work=='phi004':limits+=['The body CTS identity is exact, but supplied descendant xml:base values append .xml; remote descendant CTS resolution is not established. Preserve bytes without correction.']
        if work in ('phi031','phi032','phi033','phi034'):limits+=['The header reports 1918 and an alternate typed 1901 date; preserve both supplier date roles without conflation.']
        refs=[f'{REL}/manifest.json',f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md',cts_ref,header_ref,opening_ref,
            f'{REL}/evidence/{author}-cts.xml',f'{REL}/evidence/perseus-README.md',f'{REL}/evidence/perseus-license.md',f'https://github.com/{REPO}/tree/{PIN}']
        target={'slug':slug,'work_slug':work_slug,'title':title,'family':family,'provider':'perseus','repository':REPO,'pin':PIN,'operation_date':'2026-09-09',
            'language':'la','script':'Latn','expression_role':'source_language','expression':exp,'edition':edition,'item':item,
            'registry_sources':sources,'branch_target_slug':slug,'ids':ids,'paths':paths,
            'coverage':{'kind':'perseus-tei-latin-work','citation_scope':'hierarchical_divisions','cts_urn':urn,'identity_anchor':identity_anchor,'header_prefix_sha256':sha(header),
                'reviewed_body_prefix_sha256':sha(opening),'reviewed_body_prefix_bytes':len(opening)},
            'version_description':f'Perseus Latin digital edition {urn}, Git {PIN}. Supplied source description: {source_description}',
            'responsibility':'Supplied title/contributor statement: '+contributors+'. Claims remain reported; preserve exact header.',
            'limits':limits,'metadata_evidence_refs':[r for r in refs if r.startswith(REL+'/evidence/')],
            'files':[{'upstream_path':row['path'],'basename':Path(row['path']).name,'git_blob_sha1':row['sha'],'byte_size':row['size'],
                'media_type':'application/tei+xml','url':raw(row['path'])}],'byte_size':row['size']}
        rights=copy.deepcopy(rights_template);rid='tos.rights.'+ids['item'].removeprefix('tos.item.')
        rights.update(rights_id=rid,scope_refs=[ids['item']],source_refs=refs+[rights['license_uri']],assessed_at=observed,
            review_refs=[f'{REL}/SOURCE_AND_RIGHTS_REVIEW.md'],rationale='Positive supplier CC BY-SA 4.0 grant assessed against exact CTS/header notices for local preservation and research; no conflicting exception observed. No age, expiry, print-exemplar or publication-authority inference.')
        for layer in rights['layer_assessments']:
            name=layer['layer_id'].rsplit('.',1)[1]
            layer.update(layer_id=rid+'.layer.'+name,source_refs=rights['source_refs'],rights_holder_refs=[f'https://github.com/{REPO}'],assessed_at=observed,
                scope_refs=[ids['expression' if name=='digital-text' else 'edition' if name=='edition' else 'item']])
        package=prepare_package(target,observed,evidence_refs=refs,rights_assessment=rights)
        package['records'][paths['work']]['field_languages']['preferred_label']['language']=title_language
        for kind in ('work','expression','edition','item'):
            package['records'][paths[kind]]['external_identifiers']=[{'scheme':'CTS','value':work_urn if kind=='work' else urn,'source_ref':cts_ref,'status':'verified'}]
        anchor=prepare_anchor(ROOT,atlas_row_id='A29',source_table_index=14,source_row_index=source_row,source_label=source_label)
        plans.append({'target_slug':slug,'registry_source_record_id':sources[0]['original_source_id'],'anchor_preparation':anchor,
            'scope_relationship':'bounded-constituent-work','scope_rationale':f'Exact {title} Latin edition within {source_label}; rhetoric belongs to A29\'s explicit philosophical-communication source need. Broader corpus, versions and meaning remain open.',
            'remaining_controls':limits})
        targets.append(target);packages.append(package)
    if len(targets)!=31 or len({t['slug'] for t in targets})!=31:raise ValueError('reviewed 31-target scope differs')
    content=b''.join((json.dumps(p,ensure_ascii=False,separators=(',',':'))+'\n').encode() for p in packages);(BASE/'prepared-source-packages.jsonl').write_bytes(content)
    branch_ref='ToS/philosophy/source-planting-preparation/cicero-forensic-speeches-latin-20260909.json'
    write(ROOT/branch_ref,{'schema_version':'tos_source_planting_preparation_batch_v1','status':'prepared-not-planted','review_scope':'31 separately identified Latin speeches at explicit A29 corpus anchors','reviewer_ref':'model:codex','targets':plans})
    observations=[]
    for path in sorted((BASE/'evidence').glob('*.receipt.json')):
        value=read(path);body_path=path.with_name(path.name.removesuffix('.receipt.json'))
        value.setdefault('retained_ref',body_path.relative_to(ROOT).as_posix());value.setdefault('retained_sha256',value.get('sha256'));value.setdefault('retained_byte_size',len(body_path.read_bytes()))
        value.setdefault('started_at',value.get('observed_at'));value.setdefault('ended_at',value['started_at'])
        if value.get('started_at') and value.get('ended_at'):
            value.setdefault('elapsed_seconds',(datetime.fromisoformat(value['ended_at'])-datetime.fromisoformat(value['started_at'])).total_seconds())
            value.setdefault('elapsed_seconds_basis','difference between retained observation timestamps; single-time metadata has no measured transport duration')
        observations.append(value)
    write(BASE/'selection-review.json',{'reviewer_ref':'model:codex','observed_at':observed,'candidates':selections,'selected':len(targets),'deferred':sum(r['decision']=='defer' for r in selections),'new_source_bodies_acquired':False})
    write(BASE/'manifest.json',{'schema_version':'tos_registry_first_planting_preparation_v1','status':'prepared-not-acquired',
        'source_registry_snapshot_ref':snapshot.relative_to(ROOT).as_posix(),'source_registry_snapshot_sha256':sha(snapshot.read_bytes()),'branch_preparation_ref':branch_ref,
        'provider_pins':{'perseus':PIN},'metadata_observations':observations,'targets':targets,'prepared_packages_ref':f'{REL}/prepared-source-packages.jsonl','prepared_packages_sha256':sha(content),
        'totals':{'works':len(targets),'expressions':len(targets),'payload_files':len(targets),'payload_bytes':sum(t['byte_size'] for t in targets),'bibliographic_claims':3*len(targets)},
        'authority_boundary':'Preparation proves no complete local source custody, textual/semantic acceptance, canon or source-file publication.'})
    print(json.dumps({'selected':len(targets),'deferred':sum(r['decision']=='defer' for r in selections),'bytes':sum(t['byte_size'] for t in targets)}))


if __name__=='__main__':
    if sys.argv[1:]==['metadata']:metadata()
    elif sys.argv[1:]==['openings']:openings()
    elif sys.argv[1:]==['prepare']:prepare()
    else:raise SystemExit('choose metadata, openings or prepare')
