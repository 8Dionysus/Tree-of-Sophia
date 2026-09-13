from pathlib import Path
import sys,json,hashlib,copy
from datetime import datetime,timezone
root=next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file())
sys.path.insert(0,str(root/'scripts'))
from acquire_registry_sources import event,validate_json
src=root/'ToS/source-witnesses'; base=Path(__file__).resolve().parents[1]
manifest=json.loads((base/'manifest.json').read_text())
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
for target in manifest['targets']:
 item=root/target['paths']['item_root']; mpath=item/'item.manifest.json'
 if not mpath.is_file():continue
 m=json.loads(mpath.read_text()); rights=json.loads((item/'rights.json').read_text()); now=datetime.now(timezone.utc).isoformat()
 planref='ToS/source-witnesses/server-import/plans/registry-'+target['slug']+'.server-import.json'; path=root/planref
 if path.exists():continue
 eventid='tos.event.server-import-plan.registry-20260912.'+target['slug']
 plan={'$schema':'https://tree-of-sophia.local/ToS/contracts/server-import-contract.schema.json','schema_version':'tos_server_import_contract_v1','server_import_id':'tos.server-import.registry-'+target['slug'],'item_ref':m['item_id'],'manifest':{'ref':str(mpath.relative_to(root)),'sha256':digest(mpath),'verified':True},'payload_files':[{'file_ref':f['file_id'],'relative_path':f['relative_path'],'byte_size':f['byte_size'],'sha256':f['sha256'],'verified':True} for f in m['payload_files']],'rights_policy':{'rights_record_ref':m['rights_ref'],'rights_record_sha256':digest(item/'rights.json'),'assessment_status':'open-licensed' if rights['assessment_status']=='licensed' else 'public-domain-reviewed','review_status':'unreviewed','jurisdictions_reviewed':rights['jurisdictions_reviewed'],'permission_or_license_refs':rights['source_refs'],'expires_at':None,'recheck_before_transfer':True},'access_class':'metadata-only','allowed_derivatives':{k:{'state':'prohibited','conditions':['This plan concerns future server use only. Local preservation/research permissions remain with the Item rights record; no server processing or source-content publication has been authorized.']} for k in ['ocr','transcription','page_images','snippets','lexical_index','embeddings','alignments','translations','annotations','search_projection','graph_projection']},'payload_transfer_authorized':False,'operator_transfer_approval':{'approved':False,'approved_by_real_human':False,'approved_at':None,'approval_ref':None},'server_import_status':'not-evaluated','publication_status':'not-published','server_receipt_refs':[],'takedown':{'public_contact_url':'https://github.com/8Dionysus/Tree-of-Sophia/issues','procedure_ref':'ToS/source-witnesses/server-import/SERVER_IMPORT_PROTOCOL.md','disable_supported':True,'delete_supported':True,'last_reviewed_at':now},'provenance_event_refs':[eventid],'server_copy_is_authority':False,'automatic_checkout_payload_discovery_allowed':False,'contract_version':1,'supersedes_server_import_ref':None}
 validate_json(plan,'server-import-contract',root);path.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
 e=event(eventid,'annotation',now,now,[{'ref':plan['manifest']['ref'],'sha256':plan['manifest']['sha256'],'role':'exact-local-item-manifest'},{'ref':m['rights_ref'],'sha256':plan['rights_policy']['rights_record_sha256'],'role':'local-acquisition-rights-assessment'}],[{'ref':planref,'sha256':digest(path),'role':'explicit-unapproved-server-boundary'}],name='retain-local-only-source-boundary',configuration={'server_transfer_authorized':False,'local_acquisition_is_not_server_approval':True},rights_ref=m['rights_ref'],receipts=[planref])
 validate_json(e,'provenance-event',root)
 with (src/'server-import/provenance.jsonl').open('a') as stream:stream.write(json.dumps(e,ensure_ascii=False,separators=(',',':'))+'\n')
 print(target['slug'],plan['server_import_status'])
