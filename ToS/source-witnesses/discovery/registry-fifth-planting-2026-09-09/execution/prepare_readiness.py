"""Record absent exact destinations and reviewed readiness before acquisition."""
from pathlib import Path
from datetime import datetime,timezone
import json,hashlib,sys
ROOT=Path(__file__).resolve().parents[5];BASE=Path(__file__).resolve().parents[1];REL=BASE.relative_to(ROOT).as_posix()
sys.path.insert(0,str(ROOT/'scripts'))
from source_registry_common import read

def ref(path):return {'path':path,'sha256':hashlib.sha256((ROOT/path).read_bytes()).hexdigest()}
def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def control(paths,rationale,status='ready',**extra):return {'status':status,'evidence_posture':'owner-reviewed','owner_refs':[ref(path) for path in paths],'rationale':rationale,**extra}
manifest=read(BASE/'manifest.json');plans={p['target_slug']:p for p in read(ROOT/manifest['branch_preparation_ref'])['targets']};now=datetime.now(timezone.utc).isoformat()
observations=[]
for target in manifest['targets']:
 paths=[target['paths']['item_root']+'/payload/'+f['basename'] for f in target['files']]
 if any((ROOT/p).exists() for p in paths):raise ValueError('cannot replace pre-acquisition absence history after acquisition')
 observations.append({'target_slug':target['slug'],'new_payload_paths':paths,'present':False,'new_work_record_present':(ROOT/target['paths']['work']).exists()})
write(BASE/'local-presence.preparation.json',{'observed_at':now,'scope':'this checkout before fifth acquisition','targets':observations})
documents={};snapshot=ROOT/manifest['source_registry_snapshot_ref']
for path in read(snapshot)['documents']:
 p=snapshot.parent/path;doc=read(p);documents[(doc['corpus_id'],doc['document_id'])]=p.relative_to(ROOT).as_posix()
rows=[]
for index,target in enumerate(manifest['targets'],1):
 files=[target['paths']['item_root']+'/payload/'+f['basename'] for f in target['files']];branch=plans[target['slug']]['anchor_preparation']['branch_path']
 source_refs=[{**ref(documents[(s['corpus'],s['document_id'])]),'corpus_id':s['corpus'],'document_id':s['document_id'],'record_id':s['entry_id']} for s in target['registry_sources']]
 readiness={'version':control([REL+'/manifest.json',REL+'/SOURCE_AND_RIGHTS_REVIEW.md'],'Exact Latin edition, source bibliography including unknowns, and XML identity carrier reviewed.'),
 'access':control([next(p for p in target['metadata_evidence_refs'] if p.endswith('-opening.xml'))+'.receipt.json'],'Pinned incomplete source opening GET succeeded; complete acquisition remains pending.'),
 'rights':control([REL+'/prepared-source-packages.jsonl',REL+'/SOURCE_AND_RIGHTS_REVIEW.md'],'Positive supplier license reviewed for local preservation/research of exact digital layers.',intended_uses=['local-preservation','research-analysis']),
 'file':control([REL+'/local-presence.preparation.json'],'Exact destination paths inspected and absent before acquisition.',status='absent'),
 'branch':control([manifest['branch_preparation_ref']],'Explicit A29 corpus source anchor and scope reviewed.')}
 rows.append({'target_id':f'registry-fifth-planting.{index:02}.{target["slug"]}','preferred_label':target['title'],'source_record_refs':source_refs,
 'target':{'target_kind':'exact-work-version','description':target['version_description'],'known_tos_refs':[branch],'create_record_refs':target['paths'],'planned_ids':target['ids'],
 'expected_coverage':target['coverage'],'expected_file_count':len(files),'expected_bytes':target['byte_size'],'scope_limits':target['limits']},'readiness':readiness,
 'acquisition':{'source_urls':[f['url'] for f in target['files']],'destination_paths':files,'intended_uses':['local-preservation','research-analysis']}})
write(BASE/'readiness.preparation.v1.json',{'schema_version':'tos_open_work_readiness_plan_v1','reviewed_at':now,'reviewer_ref':'model:codex/session/01a08281-293a-7ff0-927a-0fd73dfacf7d','targets':rows})
print(json.dumps({'prepared':len(rows),'complete_files_acquired':False}))
