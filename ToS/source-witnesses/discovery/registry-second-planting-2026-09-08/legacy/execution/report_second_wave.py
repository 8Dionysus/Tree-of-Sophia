"""Record observed batch completion and the existing readiness-queue lifecycle."""
from pathlib import Path
from datetime import datetime, timezone
import sys, json, hashlib, os
root=Path(__file__).resolve().parents[5];sys.path.insert(0,str(root/'scripts'))
from acquire_registry_sources import verify_target
from source_registry_common import read
base=Path(__file__).resolve().parents[1];manifest=read(base/'manifest.json');branches={p['target_slug']:p for p in read(root/manifest['branch_preparation_ref'])['targets']}
def ref(path):return {'path':path,'sha256':hashlib.sha256((root/path).read_bytes()).hexdigest()}
def control(paths,rationale,status='ready',**kwargs):return {'status':status,'evidence_posture':'owner-reviewed','owner_refs':[ref(p) for p in paths],'rationale':rationale,**kwargs}
rows=[];targets=[]
snapshot=root/manifest['source_registry_snapshot_ref'];documents={}
for document in read(snapshot)['documents']:
 p=snapshot.parent/document;d=read(p);documents[(d['corpus_id'],d['document_id'])]=p
for i,t in enumerate(manifest['targets'],1):
 checked=verify_target(root,t);b=branches[t['slug']]['anchor_preparation']['branch_path'];plant=b+'/sources/plantings/registry-'+t['slug']+'/source-planting.json';readme=str(Path(plant).with_name('README.md'));item=t['paths']['item_root']
 assert read(root/plant)['source_witness']['work_id']==t['ids']['work']
 files=[item+'/payload/'+f['basename'] for f in t['files']]
 readiness={'version':control([str((base/'manifest.json').relative_to(root)),str((base/'SOURCE_AND_RIGHTS_REVIEW.md').relative_to(root))],'Exact CTS edition, Git pin, supplied header and enumerated file set reviewed.'),'access':control(['ToS/source-witnesses/discovery/runs/registry-'+t['slug']+'.2026-09-08.v1.json'],'Dated pinned GET completed; discovery retains the actual outcome.'),'rights':control([item+'/rights.json'],'Exact provider license and supplied layers assessed for local preservation and analysis.',intended_uses=['local-preservation','research-analysis']),'file':control(files,'Original file opened and parsed; exact blob, byte size, header, CTS and local SHA-256 verified.',status='present'),'branch':control([plant,readme],'Actual A25 branch route reaches the exact source records and locally retained file.')}
 refs=[]
 for source in t['registry_sources']:
  p=documents[(source['corpus'],source['document_id'])]
  refs.append({**ref(p.relative_to(root).as_posix()),'corpus_id':source['corpus'],'document_id':source['document_id'],'record_id':source['entry_id']})
 targets.append({'target_id':f'registry-second-planting.{i:02}.{t["slug"]}','preferred_label':t['title'],'source_record_refs':refs,'target':{'target_kind':'exact-work-version','description':t['version_description'],'known_tos_refs':[b],'create_record_refs':t['paths'],'planned_ids':t['ids'],'expected_coverage':t['coverage'],'expected_file_count':len(t['files']),'expected_bytes':t['byte_size'],'scope_limits':t['limits']},'readiness':readiness,'acquisition':{'source_urls':[f['url'] for f in t['files']],'destination_paths':files,'intended_uses':['local-preservation','research-analysis']},'execution':control([plant,t['paths']['work']],'Exact version acquisition and branch-to-file route completed; no textual/semantic/canon acceptance.',status='completed')})
 observation=read(root/item/'forensic-observations.json')
 rows.append({**checked,'title':t['title'],'atlas_row':'A25','version':t['repository']+'@'+t['pin'],'planting_ref':plant,'reading_route':readme,'observed_coverage':observation['files'][0],'remaining_controls':read(root/plant)['remaining_controls']})
now=datetime.now(timezone.utc).isoformat();write=lambda p,v:p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
write(base/'readiness.post-acquisition.v1.json',{'schema_version':'tos_open_work_readiness_plan_v1','reviewed_at':now,'reviewer_ref':'model:codex/session/01a08281-293a-7ff0-927a-0fd73dfacf7d','targets':targets})
summary={'schema_version':'tos_registry_first_planting_result_v1','completed_at':now,'preparation_commit':'e2077017ef5671777005b12d5112e9ffb040a5b0','normalization_snapshot':manifest['source_registry_snapshot_ref'],'planted':rows,'deferred':[],'blocked':[],'total_files':sum(r['files'] for r in rows),'total_bytes':sum(r['bytes'] for r in rows),'scope':'Twelve selected Perseus Greek Plato editions in A25; other editions, translations and corpus members remain open.','source_text_admitted':False,'payload_visibility':'local_only','public_or_remote_deployment':False}
write(base/'batch-result.json',summary)
lines=['# Вторая посадка: двенадцать диалогов Платона','',f"Локально посажено {len(rows)} точных греческих версий Perseus: {summary['total_files']} файлов, {summary['total_bytes']:,} байт. Для каждого проверены Git blob, размер, SHA-256, просмотренный TEI-заголовок, CTS-идентификатор, греческий текст и структурные разделы. Все двенадцать файлов открыты локально.",'','Первым отдельно посажен «Евтифрон», затем остальные одиннадцать. Исходные байты сохранены без преобразования. Каждая версия имеет Work → Expression → Edition → Item → File, права, provenance, inventory и связь с A25.','','| Произведение | Разделов TEI | Первый — последний | Открыть |','| --- | ---: | --- | --- |']
for r in rows:
 c=r['observed_coverage'];lines.append(f"| {r['title']} | {c['section_count']} | {c['first_section']} — {c['last_section']} | [Версия и локальный файл]({os.path.relpath(root/r['reading_route'],base)}) |")
lines+=['','[Покрытие всего реестра](../../../research-packets/source-registries/COVERAGE.md): посаженные выбранные версии, возможные совпадения и ещё не разобранные строки. [Точный результат](batch-result.json). [Источник и права](SOURCE_AND_RIGHTS_REVIEW.md).','','Другие версии, переводы и оставшиеся произведения Платона и Аристотеля не помечаются завершёнными. Полнота закреплённого файла не равна критической полноте текста. Текст, интерпретации и канон не приняты этой операцией.','','Исходные файлы остаются в локальных ignored Item payload. Git переносит метаданные и проверочные записи, но не сами source-файлы. Публичный landing выполняется отдельным владельцем доставки.','']
(base/'RESULTS.md').write_text('\n'.join(lines))
(base/'README.md').write_text('# Registry second planting\n\n[Results and local reading routes](RESULTS.md). [Version and rights review](SOURCE_AND_RIGHTS_REVIEW.md).\n\nThe frozen preparation and completed acquisition records remain distinct. `readiness.post-acquisition.v1.json` marks only these twelve exact targets completed in the existing queue lifecycle. `execution/` retains the bounded preparation, planting and closeout operations; it is not a new queue or source authority.\n')
print(json.dumps({'planted':len(rows),'files':summary['total_files'],'bytes':summary['total_bytes']}))
