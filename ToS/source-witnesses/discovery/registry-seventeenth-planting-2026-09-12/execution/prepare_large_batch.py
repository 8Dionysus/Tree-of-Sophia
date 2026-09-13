"""Freeze separately owned AN1–11 and SN36–56 Pali witnesses, then guarded English Sujato Work extensions."""
from pathlib import Path
from datetime import datetime, timezone
import copy,gzip,hashlib,json,re,sys
ROOT=Path(__file__).resolve().parents[5]
BATCH_ROOT=Path(__file__).resolve().parents[1]
PIN='d6d54741b7f2ddfeca82f02c3f95eb3990b4e351'
REPO='suttacentral/bilara-data'
sys.path.insert(0,str(ROOT/'scripts'))
from prepare_registry_sources import prepare_package
from prepare_philosophy_source_planting import prepare_anchor
from source_registry_common import read
sha=lambda b:hashlib.sha256(b).hexdigest()
def write(p,x):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def first_fields(body):
    decoder=json.JSONDecoder();s=body.decode('utf-8',errors='ignore');i=1;result={}
    while True:
        try:
            while s[i].isspace() or s[i]==',':i+=1
            k,i=decoder.raw_decode(s,i)
            while s[i].isspace():i+=1
            if s[i]!=':':raise ValueError('missing colon')
            i+=1
            while s[i].isspace():i+=1
            v,i=decoder.raw_decode(s,i)
            if k in result:raise ValueError('duplicate opening key')
            result[k]=v
        except (json.JSONDecodeError,IndexError):break
    return result

def classify_scope(rows):
    """Compare exact UID/Work/Expression/Item identity with waves 1–16."""
    source_root=ROOT/'ToS/source-witnesses'
    existing_works={};existing_expr=set();existing_items=set()
    for path in sorted((source_root/'works').glob('**/work.json')):
        try: record=read(path)
        except Exception: continue
        rid=record.get('record_id')
        if rid: existing_works[rid]=path.relative_to(ROOT).as_posix()
        for child in path.parent.glob('expressions/*/expression.json'):
            try: existing_expr.add(read(child).get('record_id'))
            except Exception: pass
        for child in path.parent.glob('expressions/*/editions/*/items/*/item.json'):
            try: existing_items.add(read(child).get('record_id'))
            except Exception: pass
    prior_uids=set();prior_refs=[]
    for manifest_path in sorted((source_root/'discovery').glob('registry-*/manifest.json')):
        if manifest_path.parent == BATCH_ROOT: continue
        try: prior=read(manifest_path)
        except Exception: continue
        prior_refs.append(manifest_path.relative_to(ROOT).as_posix())
        for target in prior.get('targets',[]):
            uid=target.get('coverage',{}).get('uid')
            if uid: prior_uids.add(uid)
    selected_uids=sorted({row['path'].rsplit('/',1)[1].split('_',1)[0] for row in rows})
    grouped=read(BATCH_ROOT/'deferred-files.json')
    grouped_uids=sorted({row['path'].rsplit('/',1)[1].split('_',1)[0] for row in grouped})
    decisions=[];counts={key:0 for key in ('candidate','new_work','existing_work_new_expression','already_present','deferred','ambiguity')}
    for uid in selected_uids:
        work_id=f'tos.work.pali-canon.{uid}';work_ref=existing_works.get(work_id);reasons=[];status='candidate'
        if uid in prior_uids: reasons.append('UID was already selected by a prior registry wave; exact owner closure is checked before reuse.')
        if work_ref is None:
            if uid in prior_uids: status='ambiguity';reasons.append('prior registry selection has no exact current Work record')
            else: status='new_work';reasons.append('no exact Work record or selected owner closure exists')
        else:
            record=read(ROOT/work_ref)
            if record.get('record_id') != work_id:
                status='ambiguity';reasons.append('existing Work path and record identity disagree')
            else:
                expr_ids={f'tos.expression.pali-canon.{uid}.pli-ms-bilara',f'tos.expression.pali-canon.{uid}.en-sujato-bilara'}
                item_ids={f'tos.item.pali-canon.{uid}.pli-ms-bilara.bilara-{PIN[:12]}.git-segment-json',f'tos.item.pali-canon.{uid}.en-sujato-bilara.bilara-{PIN[:12]}.git-segment-json'}
                if not (expr_ids-existing_expr) and not (item_ids-existing_items):
                    status='already_present';reasons.append('exact Work, both selected Expressions and both Items are already present')
                else:
                    status='existing_work_new_expression';reasons.append('exact Work exists but at least one selected Expression or Item is absent')
        counts[status]+=1;decisions.append({'uid':uid,'status':status,'work_id':work_id,'existing_work_ref':work_ref,'reasons':reasons})
    for _ in grouped_uids: counts['deferred']+=1
    write(BATCH_ROOT/'dedup-review.json',{
        'schema_version':'tos_registry_global_dedup_review_v1','reviewed_at':datetime.now(timezone.utc).isoformat(),
        'provider_pin':PIN,'scope':'AN1-11 and SN36-56','prior_wave_manifest_refs':prior_refs,
        'candidate_uids_before_global_dedup':len(selected_uids),'candidate_files_before_global_dedup':len(rows),
        'all_paired_uids':2171,'all_paired_files':4342,'deferred_grouped_uids':len(grouped_uids),'deferred_files':len(grouped),
        'existing_work_ids_checked':len(existing_works),'existing_expression_ids_checked':len(existing_expr),'existing_item_ids_checked':len(existing_items),
        'counts':counts,'decisions':decisions,
        'authority_boundary':'Exact identity classification only; no title-based merge, textual acceptance, rights certification, semantic admission, canon or publication.'
    })
    blocked=[x for x in decisions if x['status']=='ambiguity']
    if blocked: raise ValueError(f'identity or rights ambiguity requires owner return: {blocked[:3]}')
    return {x['uid']:x for x in decisions if x['status'] in {'new_work','existing_work_new_expression'}},counts

def prepare(translated):
    base=BATCH_ROOT/'translations' if translated else BATCH_ROOT;rel=base.relative_to(ROOT).as_posix();shared=BATCH_ROOT.relative_to(ROOT).as_posix();observed=datetime.now(timezone.utc).isoformat()
    if not (base/'SOURCE_AND_RIGHTS_REVIEW.md').is_file():raise ValueError('source-visible review required')
    rows=read(BATCH_ROOT/'candidate-files.json');existing={}
    if translated:
        prior=read(BATCH_ROOT/'manifest.json');existing.update({t['coverage']['uid']:t for t in prior['targets']})
        if len(existing)!=1997:raise ValueError('Pali Work closure incomplete')
    source=ROOT/'ToS/research-packets/source-registries';snapshot=source/'snapshots'/read(source/'current.json')['snapshot_id']/'snapshot.json';leads=[]
    for document in ('A18','A21'):
        p=snapshot.parent/'documents/table-i'/(document+'.json.gz')
        for r in json.load(gzip.open(p,'rt'))['records']:
            if r['kind']=='registry':leads.append(r)
    oldpackages=[json.loads(line) for line in (ROOT/'ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/prepared-source-packages.jsonl').read_text().splitlines()]
    template=next(p['rights'] for p in oldpackages if p['target_slug']=='samannaphala-sutta')
    prior_shared='ToS/source-witnesses/discovery/registry-thirteenth-planting-2026-09-09'
    shared_refs=[prior_shared+'/evidence/bilara-'+name for name in ('README.md','LICENSE.md','_author.json','_edition.json','_publication.json','_publication-v2.json')]
    oldbase='ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/evidence/'
    old_refs=[oldbase+'suttacentral-root-edition.json',oldbase+'suttacentral-licensing.json']
    dedup,dedup_counts=classify_scope(rows)
    anchor=prepare_anchor(ROOT,atlas_row_id='A18',source_table_index=12,source_row_index=1,source_label='Nikāya / Āgama-параллели')
    targets=[];packages=[];plans=[];selections=[]
    for row in rows:
        if row['language']!=('en' if translated else 'pli'):continue
        basename=Path(row['path']).name
        uid=basename.split('_')[0]
        if not re.fullmatch(r'(?:an|sn)[1-9][0-9]*\.[1-9][0-9]*',uid) or uid.split('.')[0]!=row['district']:raise ValueError('UID/district mismatch')
        if uid not in dedup: continue
        opening_ref=shared+'/evidence/'+row['path']+'.opening';opening=(ROOT/opening_ref).read_bytes();fields=first_fields(opening)
        title=fields.get(uid+':0.3','').strip()
        if not title or any(k.split(':')[0]!=uid for k in fields) or len(fields)<3:raise ValueError('opening UID/title evidence incomplete')
        language='en' if translated else 'pli';slug='pali-'+uid.replace('.','-')+('-sujato-english' if translated else '-root')
        family='pali-canon';work_slug=uid.replace('.','-');work_id='tos.work.'+family+'.'+uid;work_ref=f'ToS/source-witnesses/works/{family}/{work_slug}/work.json'
        before=None
        if translated:
            old=existing[uid];work_ref=old['paths']['work'];work_id=old['ids']['work'];before=(ROOT/work_ref).read_bytes();work_record=json.loads(before)
            if work_record['record_id']!=work_id:raise ValueError('existing Work identity differs')
            work_slug=Path(work_ref).parent.name
        exp='en-sujato-bilara' if translated else 'pli-ms-bilara';edition='bilara-'+PIN[:12];item='git-segment-json';w=Path(work_ref).parent.as_posix();e=w+'/expressions/'+exp;d=e+'/editions/'+edition;it=d+'/items/'+item;identity=work_id.removeprefix('tos.work.')
        ids={'work':work_id,'expression':f'tos.expression.{identity}.{exp}','edition':f'tos.edition.{identity}.{exp}.{edition}','item':f'tos.item.{identity}.{exp}.{edition}.{item}'}
        paths={'work':work_ref,'expression':e+'/expression.json','edition':d+'/edition.json','item':it+'/item.json','item_root':it}
        if any((ROOT/p).exists() for k,p in paths.items() if not (translated and k=='work')):raise ValueError('existing target requires reviewed resume rather than reprepare')
        record_ids=['A18-R004']
        if translated:record_ids+=['A21-R044']
        sources=[{'corpus':'table-i','document_id':v.split('-')[0],'original_source_id':v,'entry_id':r['record_id'],'source_locator':r['source']} for v in record_ids for r in leads if r['source_record_id']==v]
        if len(sources)!=len(record_ids):raise ValueError('exact registry source unavailable')
        limits=['Grouped UID carriers in AN1–11 and SN36–56 are explicitly deferred; this batch preserves each original aggregate carrier and does not split it into individual Works.',
            'Exact supplied SuttaCentral UID identifies this separately retained discourse; a title match never merges different UIDs.',
            'Complete pinned JSON is supplied digital-file coverage, not complete ancient recitation, textual correctness, historical truth or canon admission.',
            'Pali edition lineage, modern segment IDs and English translation remain distinct. Shared keys do not establish accepted semantic alignment.',
            'SN UID, vagga heading and segment numbering are preserved as supplied digital coordinates; no equivalence to printed paragraph numbering or another recension is inferred.',
            'Nikaya corpus leads remain broader than this constituent discourse; Agama parallels and other recensions are not supplied by this intake.']
        if translated:limits+=['Bhikkhu Sujato is the modern English translator; reported 2016–2018 translation work and first publication 2018 do not date the ancient discourse.',
            'The pinned current publication metadata retains CC0 and separately requests no AI training, model/algorithm development or AI-derived technologies. Preserve that request; this intake assesses local preservation and reading, not AI derivative use or translation quality.']
        tree_ref=shared+'/evidence/bilara-'+('en' if translated else 'pli')+'-selected-tree.json'
        meta_refs=[tree_ref,opening_ref]+shared_refs+old_refs
        observations=read(BATCH_ROOT/'source-observations.json')
        limits+=observations.get(uid,[])
        refs=[shared+'/source-observations.json',shared+'/batch-scope.json',shared+'/deferred-files.json',shared+'/storage-route-receipt.json',shared+'/dedup-review.json',rel+'/manifest.json',rel+'/SOURCE_AND_RIGHTS_REVIEW.md',shared+'/SOURCE_AND_RIGHTS_REVIEW.md',*meta_refs,f'https://github.com/{REPO}/tree/{PIN}'];refs=list(dict.fromkeys(refs))
        coverage={'kind':'bilara-translation' if translated else 'bilara-root','uid':uid,'file_count':1,'reviewed_body_prefix_sha256':sha(opening),'reviewed_body_prefix_bytes':len(opening)}
        target={'slug':slug,'work_slug':work_slug,'title':title,'family':family,'provider':'bilara','repository':REPO,'pin':PIN,'operation_date':'2026-09-12','language':language,'script':'Latn','expression_role':'translation' if translated else 'source_language','expression':exp,'edition':edition,'item':item,'registry_sources':sources,'branch_target_slug':slug,'ids':ids,'paths':paths,'coverage':coverage,
            'version_description':f'Exact {uid} '+('English translation by Bhikkhu Sujato' if translated else 'Pali root text of the Mahasangiti edition')+f' in Bilara Git {PIN}; source filename {basename}.',
            'responsibility':'Bhikkhu Sujato, English translator; SuttaCentral digital publication and revisions.' if translated else 'Mahasangiti Tipitaka Buddhavasse 2500; Dhamma Society digital edition and SuttaCentral contributions. Ancient authorship is not inferred.',
            'limits':limits,'metadata_evidence_refs':meta_refs,'files':[{'upstream_path':row['upstream_path'],'basename':basename,'git_blob_sha1':row['sha'],'byte_size':row['size'],'media_type':'application/json','url':f'https://raw.githubusercontent.com/{REPO}/{PIN}/'+row['upstream_path']}],'byte_size':row['size']}
        if translated:
            target['existing_work_ref']=work_ref;target['work_title']=work_record['preferred_label'];old=existing[uid]
            target['parallel_source']={'work_ref':old['paths']['work'],'expression_ref':old['paths']['expression'],'edition_ref':old['paths']['edition'],'item_ref':old['paths']['item'],'item_manifest_ref':old['paths']['item_root']+'/item.manifest.json','files':[old['paths']['item_root']+'/payload/'+f['basename'] for f in old['files']],'uid':uid}
        rights=copy.deepcopy(template);rid='tos.rights.'+ids['item'].removeprefix('tos.item.')
        rights.update(rights_id=rid,scope_refs=[ids['item']],source_refs=refs+[rights['license_uri']],assessed_at=observed,review_refs=[rel+'/SOURCE_AND_RIGHTS_REVIEW.md'],permissions=['Acquire and preserve these exact files locally.','Read the supplied version for local research with provenance preserved.'],derivative_posture='local_research_only',redistribution_posture='metadata_only',rationale='Supplier original-language public-domain statement and CC0 contributions are separately assessed for Pali; explicit Sujato publication CC0 applies to English. Local preservation and reading only; no publication, translation-quality or AI derivative admission.')
        for layer in rights['layer_assessments']:
            name=layer['layer_id'].rsplit('.',1)[1];layer.update(layer_id=rid+'.layer.'+name,source_refs=rights['source_refs'],assessed_at=observed,scope_refs=[ids['expression' if name=='pali-root' else 'edition' if name=='suttacentral-contribution' else 'item']],permissions=rights['permissions'],derivative_posture='local_research_only',redistribution_posture='metadata_only',server_processing_posture='not_authorized')
            if translated and name=='pali-root':
                layer.update(layer_id=rid+'.layer.english-translation',layer_role='translation',assessment_status='licensed',assessment_basis='license',license_uri=rights['license_uri'],rights_holder_refs=[prior_shared+'/evidence/bilara-_publication-v2.json'],rationale='Exact English Sujato translation has explicit publication CC0; not an ancient Pali layer. Current publication record additionally retains a creator request against AI uses.',term={'calculation_status':'not_applicable','basis':'Positive CC0 dedication, not copyright expiry.','starts_on':None,'ends_on':None,'uncertainty':'Creator usage request is retained separately; AI derivatives are outside this intake.'})
        package=prepare_package(target,observed,evidence_refs=refs,rights_assessment=rights)
        if translated:
            expected=copy.deepcopy(work_record);expected['expression_claim_refs']+=package['records'][work_ref]['expression_claim_refs'];expected['record_version']+=1;package['records'][work_ref]=expected;preimage=rel+'/work-before/'+sha(before)+'.json'
            before_path=ROOT/preimage;before_path.parent.mkdir(parents=True,exist_ok=True)
            if before_path.exists() and before_path.read_bytes()!=before:raise ValueError('immutable Work preimage collision')
            before_path.write_bytes(before);package['existing_work']={'record_ref':work_ref,'preimage_ref':preimage,'sha256':sha(before)}
        else:package['records'][work_ref]['field_languages']['preferred_label']['language']='pli'
        for kind in (('expression','edition','item') if translated else ('work','expression','edition','item')):
            package['records'][paths[kind]]['external_identifiers']=[{'scheme':'SuttaCentral','value':uid if kind=='work' else uid+('/en/sujato' if translated else '/pli/ms'),'source_ref':opening_ref,'status':'verified'}]
        plans.append({'target_slug':slug,'registry_source_record_id':sources[0]['original_source_id'],'anchor_preparation':anchor,'scope_relationship':'parallel-translation-of-existing-work' if translated else 'bounded-constituent-work','scope_rationale':f'Exact {uid} '+('English reading version alongside the retained Pali witness' if translated else 'Pali discourse witness')+' within the early Buddhist Nikaya source need; no Agama parallel or entire recension admission.','remaining_controls':limits})
        selections.append({'uid':uid,'upstream_path':row['upstream_path'],'decision':'select','title':title,'owner':'A18/source-witnesses','reason':'Pinned UID, opening title and language, exact provider edition and local rights basis reviewed.'})
        targets.append(target);packages.append(package)
    expected=1997
    if len(targets)!=expected or len({t['ids']['work'] for t in targets})!=expected:raise ValueError('reviewed distinct Work scope differs')
    payload=b''.join((json.dumps(p,ensure_ascii=False,separators=(',',':'))+'\n').encode() for p in packages);(base/'prepared-source-packages.jsonl').write_bytes(payload)
    branch_ref='ToS/philosophy/source-planting-preparation/early-buddhist-an1-11-sn36-56-'+('translations' if translated else 'pali')+'-20260912.json'
    write(ROOT/branch_ref,{'schema_version':'tos_source_planting_preparation_batch_v1','status':'prepared-not-planted','review_scope':f'{expected} exact Bilara '+('English versions attached to existing Works' if translated else 'new Pali discourse witnesses'),'reviewer_ref':'model:codex','targets':plans})
    observations=[read(p) for p in sorted((BATCH_ROOT/'evidence').rglob('*.receipt.json'))]+[read(ROOT/(p+'.receipt.json')) for p in read(BATCH_ROOT/'inherited-metadata-refs.json')]
    write(base/'selection-review.json',{'reviewer_ref':'model:codex','observed_at':observed,'candidates':selections,'selected':expected,'deferred':174,'reused_existing':dedup_counts.get('already_present',0),'new_work_candidates':dedup_counts.get('new_work',0),'existing_work_new_expression':dedup_counts.get('existing_work_new_expression',0),'ambiguity_returned':dedup_counts.get('ambiguity',0),'new_source_bodies_acquired':False,'deferred_files_ref':shared+'/deferred-files.json','global_dedup_ref':shared+'/dedup-review.json'})
    write(base/'manifest.json',{'schema_version':'tos_registry_first_planting_preparation_v1','status':'prepared-not-acquired','source_registry_snapshot_ref':snapshot.relative_to(ROOT).as_posix(),'source_registry_snapshot_sha256':sha(snapshot.read_bytes()),'branch_preparation_ref':branch_ref,'provider_pins':{'bilara':PIN},'metadata_observations':observations,'targets':targets,'prepared_packages_ref':rel+'/prepared-source-packages.jsonl','prepared_packages_sha256':sha(payload),'totals':{'works':0 if translated else expected,'existing_works_extended':expected if translated else 0,'expressions':expected,'payload_files':expected,'payload_bytes':sum(t['byte_size'] for t in targets),'bibliographic_claims':3*expected},'global_dedup_ref':shared+'/dedup-review.json','storage_route_receipt_ref':shared+'/storage-route-receipt.json','authority_boundary':'Preparation is not complete file custody, semantic acceptance, canon, translation-quality admission or public source distribution.'})
    print(json.dumps({'stage':'English' if translated else 'Pali','targets':expected,'bytes':sum(t['byte_size'] for t in targets)}))
if __name__=='__main__':
    if sys.argv[1:] not in (['pali'],['translations']):raise SystemExit('choose pali or translations')
    prepare(sys.argv[1]=='translations')
