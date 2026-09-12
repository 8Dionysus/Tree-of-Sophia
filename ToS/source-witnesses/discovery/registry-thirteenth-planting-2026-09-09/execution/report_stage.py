"""Separate preparation, observed custody and a reader route for this stage."""
from pathlib import Path
from datetime import datetime,timezone
import json,hashlib,os,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file())
BASE=Path(__file__).resolve().parents[1];REL=BASE.relative_to(ROOT).as_posix()
sys.path.insert(0,str(ROOT/'scripts'))
from acquire_registry_sources import verify_target
from source_registry_common import read

def ref(p):return {'path':p,'sha256':hashlib.sha256((ROOT/p).read_bytes()).hexdigest()}
def control(paths,rationale,status='ready',**extra):return {'status':status,'evidence_posture':'owner-reviewed','owner_refs':[ref(p) for p in paths],'rationale':rationale,**extra}
def write(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def report(done):
    m=read(BASE/'manifest.json');translated=m['targets'][0]['language']=='en';stage='English' if translated else 'Pali';now=datetime.now(timezone.utc).isoformat();plans={p['target_slug']:p for p in read(ROOT/m['branch_preparation_ref'])['targets']}
    if not done:
        absent=[]
        for t in m['targets']:
            files=[t['paths']['item_root']+'/payload/'+f['basename'] for f in t['files']]
            if any((ROOT/p).exists() for p in files):raise ValueError('preparation absence cannot overwrite custody history')
            x={'target_slug':t['slug'],'new_payload_paths':files,'new_payloads_present':False}
            if translated:x.update(existing_work_ref=ref(t['paths']['work']),parallel_Pali_payloads_present=all((ROOT/p).is_file() for p in t['parallel_source']['files']))
            absent.append(x)
        write(BASE/'local-presence.preparation.json',{'observed_at':now,'scope':stage+' stage before acquisition','targets':absent})
    snapshot=ROOT/m['source_registry_snapshot_ref'];docs={}
    for p in read(snapshot)['documents']:
        x=read(snapshot.parent/p);docs[x['corpus_id'],x['document_id']]=(snapshot.parent/p).relative_to(ROOT).as_posix()
    targets=[];rows=[]
    for i,t in enumerate(m['targets'],1):
        p=plans[t['slug']];branch=p['anchor_preparation']['branch_path'];planting=branch+'/sources/plantings/registry-'+t['slug']+'/source-planting.json';reading=str(Path(planting).with_name('README.md'));item=t['paths']['item_root'];files=[item+'/payload/'+f['basename'] for f in t['files']];discovery='ToS/source-witnesses/discovery/runs/registry-'+t['slug']+'.2026-09-09.v1.json'
        readiness={'version':control([REL+'/manifest.json',REL+'/SOURCE_AND_RIGHTS_REVIEW.md'],'Exact supplied UID, edition, source language or translation role and file list reviewed.'),
            'access':control([discovery] if done else [next(p for p in t['metadata_evidence_refs'] if p.endswith('.opening'))+'.receipt.json'],'Pinned complete file retained.' if done else 'Pinned incomplete opening accessible; complete transfer awaits checkpoint.'),
            'rights':control([item+'/rights.json'] if done else [REL+'/prepared-source-packages.jsonl',REL+'/SOURCE_AND_RIGHTS_REVIEW.md'],'Exact local preservation and reading scope reviewed; no AI derivative admission.',intended_uses=['local-preservation','research-analysis']),
            'file':control(files if done else [REL+'/local-presence.preparation.json'],'Exact files verified locally.' if done else 'New destination files absent.',status='present' if done else 'absent'),
            'branch':control([planting,reading] if done else [m['branch_preparation_ref']],'Source branch route reviewed for this exact constituent version.')}
        target={'target_kind':'exact-work-version','description':t['version_description'],'known_tos_refs':[branch],'create_record_refs':t['paths'],'planned_ids':t['ids'],'expected_coverage':t['coverage'],'expected_file_count':len(files),'expected_bytes':t['byte_size'],'scope_limits':t['limits']}
        if translated:
            target.update(known_tos_refs=[branch,t['paths']['work'],t['parallel_source']['expression_ref']],create_record_refs={k:v for k,v in t['paths'].items() if k!='work'},existing_record_refs={'work':t['paths']['work']})
        entry={'target_id':f'registry-thirteenth-{stage.lower()}.{i:03}.{t["slug"]}','preferred_label':t['title'],'source_record_refs':[{**ref(docs[s['corpus'],s['document_id']]),'corpus_id':s['corpus'],'document_id':s['document_id'],'record_id':s['entry_id']} for s in t['registry_sources']],'target':target,'readiness':readiness,'acquisition':{'source_urls':[f['url'] for f in t['files']],'destination_paths':files,'intended_uses':['local-preservation','research-analysis']}}
        if done:
            checked=verify_target(ROOT,t);owned=read(ROOT/planting)
            if owned['source_witness']['work_id']!=t['ids']['work'] or owned['discovery_ref']!=discovery:raise ValueError('branch identity differs')
            entry['execution']=control([planting,t['paths']['work'],t['paths']['expression'],item+'/item.manifest.json'],'Exact source acquired and branch-linked; no semantic admission.',status='completed')
            rows.append({**checked,'uid':t['coverage']['uid'],'title':t['title'],'work_title':t.get('work_title',t['title']),'language':t['language'],'expression_role':t['expression_role'],'new_work_created':not translated,'atlas_row':'A18','planting_ref':planting,'reading_route':reading,'parallel_source':t.get('parallel_source'),'observed_coverage':read(ROOT/item/'forensic-observations.json')['files'][0],'remaining_controls':owned['remaining_controls']})
        targets.append(entry)
    write(BASE/('readiness.post-acquisition.v1.json' if done else 'readiness.preparation.v1.json'),{'schema_version':'tos_open_work_readiness_plan_v1','reviewed_at':now,'reviewer_ref':'model:codex/session/01a08281-293a-7ff0-927a-0fd73dfacf7d','targets':targets})
    if not done:print(json.dumps({'prepared_targets':len(targets),'stage':stage}));return
    selection=read(BASE/'selection-review.json');forms=read(BASE/'human-form-companions.json');result={'schema_version':'tos_registry_first_planting_result_v1','completed_at':now,'preparation_commit':read(BASE/'preparation-checkpoint-receipt.json')['commit'],'normalization_snapshot':m['source_registry_snapshot_ref'],'planted':rows,'deferred':[],'reused_existing':[s for s in selection['candidates'] if s['decision']=='reuse-existing'],'blocked':[],'total_files':sum(x['files'] for x in rows),'total_bytes':sum(x['bytes'] for x in rows),'scope':stage+' stage of the 186-discourse Digha/Majjhima paired corpus','human_form_companions_ref':REL+'/human-form-companions.json','new_works_created':0 if translated else len(rows),'source_text_admitted':False,'payload_visibility':'local_only','public_or_remote_deployment':False}
    write(BASE/'batch-result.json',result)
    lines=['# Тринадцатая поставка: '+('английские переводы Суджато' if translated else 'палийские дискурсы'),' ',f"Добавлено {len(rows)} версий: {result['total_files']} файлов, {result['total_bytes']:,} байт.",'', '| UID | Название | Чтение |','| --- | --- | --- |']
    for x in rows:lines.append(f"| {x['uid']} | {x['title']} | [Открыть]({os.path.relpath(ROOT/x['reading_route'],BASE)}) |")
    lines+=['',f"Формы этой стадии: {forms['sets']} наборов, {forms['forms']} форм. История существующих форм сохранена; семантического принятия нет.",'','Пали и английский перевод имеют отдельные Expression, Edition и Item. Общий UID не доказывает качество перевода или семантическое выравнивание. Отсутствующие версии других редакций и Āgama-параллели не объявляются полученными.','','[Источник и права](SOURCE_AND_RIGHTS_REVIEW.md). [Полный результат](batch-result.json). [Формы](human-form-companions.json).','','Исходные файлы локальны и исключены из Git. CI, merge, публикация и принятие текста остаются отдельными состояниями.','']
    (BASE/'PALI_RESULTS.md').write_text('\n'.join(lines));(BASE/'README.md').write_text('# Thirteenth '+stage+' stage\n\n[Reading routes and result](PALI_RESULTS.md). [Source and rights review](SOURCE_AND_RIGHTS_REVIEW.md).\n')
    for branch in sorted({p['anchor_preparation']['branch_path'] for p in plans.values()}):
        for path in [ROOT/branch/'README.md',ROOT/branch/'sources/README.md']:
            prior=path.read_text();marker='## Registry planting: thirteenth '+stage+' stage (2026-09-09)'
            if marker not in prior:path.write_text(prior.rstrip()+'\n\n'+marker+'\n\n'+f'{len(rows)} exact versions. [Reading routes and evidence]({os.path.relpath(BASE/"PALI_RESULTS.md",path.parent)}).\n')
    print(json.dumps({'completed':len(rows),'stage':stage,'bytes':result['total_bytes']}))
if __name__=='__main__':
    if sys.argv[1:] not in (['prepare'],['complete']):raise SystemExit('choose prepare or complete')
    report(sys.argv[1]=='complete')
