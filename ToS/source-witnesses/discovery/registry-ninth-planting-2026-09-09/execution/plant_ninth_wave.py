import sys, json, os
from pathlib import Path
from datetime import datetime, timezone
root=Path(__file__).resolve().parents[5]
sys.path.insert(0,str(root/'scripts'))
from acquire_registry_sources import verify_target, validate_json
from prepare_philosophy_source_planting import prepare_anchor
base=root/'ToS/source-witnesses/discovery/registry-ninth-planting-2026-09-09'
manifest=json.loads((base/'manifest.json').read_text())
plans={x['target_slug']:x for x in json.loads((root/'ToS/philosophy/source-planting-preparation/ninth-wave-20260909.json').read_text())['targets']}
for target in manifest['targets']:
 if sys.argv[1:] and target['slug'] not in sys.argv[1:]: continue
 checked=verify_target(root,target)
 correction_path=base/'oraec-scope-correction/current.json'
 correction=(json.loads(correction_path.read_text()).get('targets',{}).get(target['slug'],{}) if correction_path.exists() else {})
 version_description=correction.get('corrected_version_description',target['version_description'])
 extra_limits=correction.get('limits',[])
 plan=plans[target['branch_target_slug']]; anchor=plan['anchor_preparation']; a=anchor['source_backlog_anchor']
 current=prepare_anchor(root,atlas_row_id=anchor['atlas_row_id'],source_table_index=a['source_table_index'],source_row_index=a['source_row_index'],source_label=a['source_label'])
 if current['backlog_row_sha256']!=anchor['backlog_row_sha256']: raise ValueError('backlog changed')
 branch=root/anchor['branch_path']; out=branch/'sources/plantings'/('registry-'+target['slug']); out.mkdir(parents=True,exist_ok=True)
 record={'$schema':'https://tree-of-sophia.local/ToS/contracts/philosophy-source-planting.schema.json','schema_version':'tos_philosophy_source_planting_v1','planting_id':'tos.planting.'+anchor['atlas_row_id'].lower()+'.registry-'+target['slug'],'atlas_row_id':anchor['atlas_row_id'],'dossier_id':anchor['dossier_id'],'branch_path':anchor['branch_path'],'source_backlog_anchor':a,'source_witness':{'work_id':target['ids']['work'],'record_ref':target['paths']['work'],'relationship':'grounds_source_backlog_anchor'},'discovery_ref':'ToS/source-witnesses/discovery/runs/registry-'+target['slug']+'.2026-09-09.v1.json','provenance_event_ref':'tos.event.discovery.registry-20260909.'+target['slug'],'research_ref':str((base/'SOURCE_AND_RIGHTS_REVIEW.md').relative_to(root)),'fulfilled_source_needs':[plan['scope_rationale'],'Exact prepared version is locally retained: '+str(checked['files'])+' immutable files; Item manifest fixes SHA-256 and resource inventory records observed coverage.'],'remaining_controls':list(dict.fromkeys(plan['remaining_controls']+target['limits']+extra_limits+['Local file custody and branch route do not admit text, translation, semantics or canon.'])),'research_order':['official_and_classical_documentation','established_top_scholarship','fresh_current_relevance_check'],'authority':{'source_status':'metadata_witness_planted','review_status':'unreviewed','source_text_admitted':False,'semantic_status':'not_started','graph_status':'not_promoted','canon_status':'not_canon','human_task_created':False},'status':'source_witness_planted','maker':{'maker_type':'model','agent_ref':'model:codex','human_review_performed':False},'created_at':datetime.now(timezone.utc).isoformat(),'record_version':1}
 validate_json(record,'philosophy-source-planting',root)
 path=out/'source-planting.json'
 if path.exists():
  old=json.loads(path.read_text()); record['created_at']=old['created_at']
  if record!=old: raise ValueError('existing planting differs')
 else: path.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
 def link(ref): return os.path.relpath(root/ref,out)
 item=target['paths']['item_root']
 lines=['# '+target['title'],'',version_description,'',plan['scope_rationale'],'','## Local source files','']
 for f in target['files']: lines.append('- ['+f['basename']+']('+link(item+'/payload/'+f['basename'])+')')
 lines+=['','## Exact owner records','',*['- ['+label+']('+link(ref)+')' for label,ref in [('Work',target['paths']['work']),('Expression',target['paths']['expression']),('Edition',target['paths']['edition']),('Item and SHA-256',item+'/item.manifest.json'),('Rights',item+'/rights.json'),('Provenance',item+'/provenance.jsonl'),('Observed coverage',item+'/forensic-observations.json')]],'','## Limits','',*['- '+x for x in record['remaining_controls']],'']
 parallel=dict(target['parallel_source']); parallel['reading_route']=target['parallel_source']['reading_route']
 lines+=['## Parallel Greek version','', '- [Greek version and owner records]('+link(parallel['reading_route'])+')','']
 for p in parallel['files']: lines.append('- [Greek source file]('+link(p)+')')
 lines+=['','Shared Work identity does not establish exact edition dependence or textual alignment.','']
 (out/'README.md').write_text('\n'.join(lines))
 greek=root/parallel['reading_route']; prior=greek.read_text(); marker='## Additional English version: '+target['slug']
 if marker not in prior:
  greek.write_text(prior.rstrip()+'\n\n'+marker+'\n\n[English version and source file]('+os.path.relpath(out/'README.md',greek.parent)+'). Shared Work identity does not establish edition dependence or alignment.\n')
 for ref,key,count in [('branch.manifest.json','source_planting_refs','source_planting_count'),('sources/branch.manifest.json','planting_refs','planting_count')]:
  p=branch/ref; obj=json.loads(p.read_text()); refs=sorted(str(x.relative_to(root)) for x in (branch/'sources/plantings').glob('*/source-planting.json')); obj[key]=refs; obj[count]=len(refs); p.write_text(json.dumps(obj,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps({'slug':target['slug'],'planting_ref':str(path.relative_to(root)),'local_readme':str((out/'README.md').relative_to(root)),**checked}),flush=True)
