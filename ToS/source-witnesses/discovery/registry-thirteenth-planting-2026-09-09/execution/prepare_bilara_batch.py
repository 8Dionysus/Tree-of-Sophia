"""Freeze separately owned Pali witnesses, then guarded English Work extensions."""
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

def prepare(translated):
    base=BATCH_ROOT/'translations' if translated else BATCH_ROOT;rel=base.relative_to(ROOT).as_posix();shared=BATCH_ROOT.relative_to(ROOT).as_posix();observed=datetime.now(timezone.utc).isoformat()
    if not (base/'SOURCE_AND_RIGHTS_REVIEW.md').is_file():raise ValueError('source-visible review required')
    rows=read(BATCH_ROOT/'candidate-files.json');first=read(ROOT/'ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/manifest.json')
    existing={t['coverage']['uid']:t for t in first['targets'] if t['coverage']['kind']=='bilara-root' and re.fullmatch(r'(dn|mn)\d+',t['coverage']['uid'])}
    if set(existing)!={'dn2','mn9','mn56'}:raise ValueError('first-wave UID closure changed')
    if translated:
        prior=read(BATCH_ROOT/'manifest.json');existing.update({t['coverage']['uid']:t for t in prior['targets']})
        if len(existing)!=186:raise ValueError('Pali Work closure incomplete')
    source=ROOT/'ToS/research-packets/source-registries';snapshot=source/'snapshots'/read(source/'current.json')['snapshot_id']/'snapshot.json';leads=[]
    for document in ('A18','A21'):
        p=snapshot.parent/'documents/table-i'/(document+'.json.gz')
        for r in json.load(gzip.open(p,'rt'))['records']:
            if r['kind']=='registry':leads.append(r)
    oldpackages=[json.loads(line) for line in (ROOT/'ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/prepared-source-packages.jsonl').read_text().splitlines()]
    template=next(p['rights'] for p in oldpackages if p['target_slug']=='samannaphala-sutta')
    shared_refs=[shared+'/evidence/bilara-'+name for name in ('README.md','LICENSE.md','_author.json','_edition.json','_publication.json','_publication-v2.json')]
    oldbase='ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/evidence/'
    old_refs=[oldbase+'suttacentral-root-edition.json',oldbase+'suttacentral-licensing.json']
    targets=[];packages=[];plans=[];selections=[]
    for row in rows:
        if row['language']!=('en' if translated else 'pli'):continue
        uid=row['path'].split('_')[0];opening_ref=shared+'/evidence/'+row['path']+'.opening';opening=(ROOT/opening_ref).read_bytes();fields=first_fields(opening)
        title=fields.get(uid+':0.2','').strip()
        if not title or any(k.split(':')[0]!=uid for k in fields) or len(fields)<3:raise ValueError('opening UID/title evidence incomplete')
        if not translated and uid in existing:
            selections.append({'uid':uid,'upstream_path':row['upstream_path'],'decision':'reuse-existing','existing_work_ref':existing[uid]['paths']['work'],'reason':'Existing exact Pali Item at the same provider pin; no duplicate Work or file acquisition.'});continue
        language='en' if translated else 'pli';slug='pali-'+uid+('-sujato-english' if translated else '-root')
        family='pali-canon';work_slug=uid;work_id='tos.work.'+family+'.'+uid;work_ref=f'ToS/source-witnesses/works/{family}/{uid}/work.json'
        before=None
        if translated:
            old=existing[uid];work_ref=old['paths']['work'];work_id=old['ids']['work'];before=(ROOT/work_ref).read_bytes();work_record=json.loads(before)
            if work_record['record_id']!=work_id:raise ValueError('existing Work identity differs')
            work_slug=Path(work_ref).parent.name
        exp='en-sujato-bilara' if translated else 'pli-ms-bilara';edition='bilara-'+PIN[:12];item='git-segment-json';w=Path(work_ref).parent.as_posix();e=w+'/expressions/'+exp;d=e+'/editions/'+edition;it=d+'/items/'+item;identity=work_id.removeprefix('tos.work.')
        ids={'work':work_id,'expression':f'tos.expression.{identity}.{exp}','edition':f'tos.edition.{identity}.{exp}.{edition}','item':f'tos.item.{identity}.{exp}.{edition}.{item}'}
        paths={'work':work_ref,'expression':e+'/expression.json','edition':d+'/edition.json','item':it+'/item.json','item_root':it}
        if any((ROOT/p).exists() for k,p in paths.items() if not (translated and k=='work')):raise ValueError('existing target requires reviewed resume rather than reprepare')
        record_ids=['A18-R002' if row['district']=='dn' else 'A18-R003']
        if translated:record_ids+=['A21-R042' if row['district']=='dn' else 'A21-R043']
        sources=[{'corpus':'table-i','document_id':v.split('-')[0],'original_source_id':v,'entry_id':r['record_id'],'source_locator':r['source']} for v in record_ids for r in leads if r['source_record_id']==v]
        if len(sources)!=len(record_ids):raise ValueError('exact registry source unavailable')
        limits=['Exact supplied SuttaCentral UID identifies this separately retained discourse; a title match never merges different UIDs.',
            'Complete pinned JSON is supplied digital-file coverage, not complete ancient recitation, textual correctness, historical truth or canon admission.',
            'Pali edition lineage, modern segment IDs and English translation remain distinct. Shared keys do not establish accepted semantic alignment.',
            'DN segment numbering follows PTS and MN numbering follows Nanamoli per the supplier edition note; it is not the Mahasangiti printed paragraph numbering.',
            'Nikaya corpus leads remain broader than this constituent discourse; Agama parallels and other recensions are not supplied by this intake.']
        if translated:limits+=['Bhikkhu Sujato is the modern English translator; reported 2016–2018 translation work and first publication 2018 do not date the ancient discourse.',
            'The pinned current publication metadata retains CC0 and separately requests no AI training, model/algorithm development or AI-derived technologies. Preserve that request; this intake assesses local preservation and reading, not AI derivative use or translation quality.']
        tree_ref=shared+'/evidence/bilara-'+row['district']+('-en' if translated else '')+'-tree.json'
        meta_refs=[tree_ref,opening_ref]+shared_refs+old_refs
        refs=[rel+'/manifest.json',rel+'/SOURCE_AND_RIGHTS_REVIEW.md',shared+'/SOURCE_AND_RIGHTS_REVIEW.md',*meta_refs,f'https://github.com/{REPO}/tree/{PIN}'];refs=list(dict.fromkeys(refs))
        coverage={'kind':'bilara-translation' if translated else 'bilara-root','uid':uid,'file_count':1,'reviewed_body_prefix_sha256':sha(opening),'reviewed_body_prefix_bytes':len(opening)}
        target={'slug':slug,'work_slug':work_slug,'title':title,'family':family,'provider':'bilara','repository':REPO,'pin':PIN,'operation_date':'2026-09-09','language':language,'script':'Latn','expression_role':'translation' if translated else 'source_language','expression':exp,'edition':edition,'item':item,'registry_sources':sources,'branch_target_slug':slug,'ids':ids,'paths':paths,'coverage':coverage,
            'version_description':f'Exact {uid} '+('English translation by Bhikkhu Sujato' if translated else 'Pali root text of the Mahasangiti edition')+f' in Bilara Git {PIN}; source filename {row["path"]}.',
            'responsibility':'Bhikkhu Sujato, English translator; SuttaCentral digital publication and revisions.' if translated else 'Mahasangiti Tipitaka Buddhavasse 2500; Dhamma Society digital edition and SuttaCentral contributions. Ancient authorship is not inferred.',
            'limits':limits,'metadata_evidence_refs':meta_refs,'files':[{'upstream_path':row['upstream_path'],'basename':row['path'],'git_blob_sha1':row['sha'],'byte_size':row['size'],'media_type':'application/json','url':f'https://raw.githubusercontent.com/{REPO}/{PIN}/'+row['upstream_path']}],'byte_size':row['size']}
        if translated:
            target['existing_work_ref']=work_ref;target['work_title']=work_record['preferred_label'];old=existing[uid]
            target['parallel_source']={'work_ref':old['paths']['work'],'expression_ref':old['paths']['expression'],'edition_ref':old['paths']['edition'],'item_ref':old['paths']['item'],'item_manifest_ref':old['paths']['item_root']+'/item.manifest.json','files':[old['paths']['item_root']+'/payload/'+f['basename'] for f in old['files']],'uid':uid}
        rights=copy.deepcopy(template);rid='tos.rights.'+ids['item'].removeprefix('tos.item.')
        rights.update(rights_id=rid,scope_refs=[ids['item']],source_refs=refs+[rights['license_uri']],assessed_at=observed,review_refs=[rel+'/SOURCE_AND_RIGHTS_REVIEW.md'],permissions=['Acquire and preserve these exact files locally.','Read the supplied version for local research with provenance preserved.'],derivative_posture='local_research_only',redistribution_posture='metadata_only',rationale='Supplier original-language public-domain statement and CC0 contributions are separately assessed for Pali; explicit Sujato publication CC0 applies to English. Local preservation and reading only; no publication, translation-quality or AI derivative admission.')
        for layer in rights['layer_assessments']:
            name=layer['layer_id'].rsplit('.',1)[1];layer.update(layer_id=rid+'.layer.'+name,source_refs=rights['source_refs'],assessed_at=observed,scope_refs=[ids['expression' if name=='pali-root' else 'edition' if name=='suttacentral-contribution' else 'item']],permissions=rights['permissions'],derivative_posture='local_research_only',redistribution_posture='metadata_only',server_processing_posture='not_authorized')
            if translated and name=='pali-root':
                layer.update(layer_id=rid+'.layer.english-translation',layer_role='translation',assessment_status='licensed',assessment_basis='license',license_uri=rights['license_uri'],rights_holder_refs=[shared+'/evidence/bilara-_publication-v2.json'],rationale='Exact English Sujato translation has explicit publication CC0; not an ancient Pali layer. Current publication record additionally retains a creator request against AI uses.',term={'calculation_status':'not_applicable','basis':'Positive CC0 dedication, not copyright expiry.','starts_on':None,'ends_on':None,'uncertainty':'Creator usage request is retained separately; AI derivatives are outside this intake.'})
        package=prepare_package(target,observed,evidence_refs=refs,rights_assessment=rights)
        if translated:
            expected=copy.deepcopy(work_record);expected['expression_claim_refs']+=package['records'][work_ref]['expression_claim_refs'];expected['record_version']+=1;package['records'][work_ref]=expected;preimage=rel+'/work-before/'+sha(before)+'.json'
            before_path=ROOT/preimage;before_path.parent.mkdir(parents=True,exist_ok=True)
            if before_path.exists() and before_path.read_bytes()!=before:raise ValueError('immutable Work preimage collision')
            before_path.write_bytes(before);package['existing_work']={'record_ref':work_ref,'preimage_ref':preimage,'sha256':sha(before)}
        else:package['records'][work_ref]['field_languages']['preferred_label']['language']='pli'
        for kind in (('expression','edition','item') if translated else ('work','expression','edition','item')):
            package['records'][paths[kind]]['external_identifiers']=[{'scheme':'SuttaCentral','value':uid if kind=='work' else uid+('/en/sujato' if translated else '/pli/ms'),'source_ref':opening_ref,'status':'verified'}]
        anchor=prepare_anchor(ROOT,atlas_row_id='A18',source_table_index=12,source_row_index=1,source_label='Nikāya / Āgama-параллели')
        plans.append({'target_slug':slug,'registry_source_record_id':sources[0]['original_source_id'],'anchor_preparation':anchor,'scope_relationship':'parallel-translation-of-existing-work' if translated else 'bounded-constituent-work','scope_rationale':f'Exact {uid} '+('English reading version alongside the retained Pali witness' if translated else 'Pali discourse witness')+' within the early Buddhist Nikaya source need; no Agama parallel or entire recension admission.','remaining_controls':limits})
        selections.append({'uid':uid,'upstream_path':row['upstream_path'],'decision':'select','title':title,'owner':'A18/source-witnesses','reason':'Pinned UID, opening title and language, exact provider edition and local rights basis reviewed.'})
        targets.append(target);packages.append(package)
    expected=186 if translated else 183
    if len(targets)!=expected or len({t['ids']['work'] for t in targets})!=expected:raise ValueError('reviewed distinct Work scope differs')
    payload=b''.join((json.dumps(p,ensure_ascii=False,separators=(',',':'))+'\n').encode() for p in packages);(base/'prepared-source-packages.jsonl').write_bytes(payload)
    branch_ref='ToS/philosophy/source-planting-preparation/thirteenth-'+('translations' if translated else 'pali')+'-wave-20260909.json'
    write(ROOT/branch_ref,{'schema_version':'tos_source_planting_preparation_batch_v1','status':'prepared-not-planted','review_scope':f'{expected} exact Bilara '+('English versions attached to existing Works' if translated else 'new Pali discourse witnesses'),'reviewer_ref':'model:codex','targets':plans})
    observations=[read(p) for p in sorted((BATCH_ROOT/'evidence').glob('*.receipt.json'))]+[read(ROOT/(p+'.receipt.json')) for p in old_refs]
    write(base/'selection-review.json',{'reviewer_ref':'model:codex','observed_at':observed,'candidates':selections,'selected':expected,'deferred':0,'reused_existing':0 if translated else 3,'new_source_bodies_acquired':False})
    write(base/'manifest.json',{'schema_version':'tos_registry_first_planting_preparation_v1','status':'prepared-not-acquired','source_registry_snapshot_ref':snapshot.relative_to(ROOT).as_posix(),'source_registry_snapshot_sha256':sha(snapshot.read_bytes()),'branch_preparation_ref':branch_ref,'provider_pins':{'bilara':PIN},'metadata_observations':observations,'targets':targets,'prepared_packages_ref':rel+'/prepared-source-packages.jsonl','prepared_packages_sha256':sha(payload),'totals':{'works':0 if translated else expected,'existing_works_extended':expected if translated else 0,'expressions':expected,'payload_files':expected,'payload_bytes':sum(t['byte_size'] for t in targets),'bibliographic_claims':3*expected},'authority_boundary':'Preparation is not complete file custody, semantic acceptance, canon, translation-quality admission or public source distribution.'})
    print(json.dumps({'stage':'English' if translated else 'Pali','targets':expected,'bytes':sum(t['byte_size'] for t in targets)}))
if __name__=='__main__':
    if sys.argv[1:] not in (['pali'],['translations']):raise SystemExit('choose pali or translations')
    prepare(sys.argv[1]=='translations')
