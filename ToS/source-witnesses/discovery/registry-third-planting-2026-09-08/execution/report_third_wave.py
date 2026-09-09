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
 readiness={'version':control([str((base/'manifest.json').relative_to(root)),str((base/'SOURCE_AND_RIGHTS_REVIEW.md').relative_to(root))],'Exact CTS edition, Git pin, supplied header and enumerated file set reviewed.'),'access':control(['ToS/source-witnesses/discovery/runs/registry-'+t['slug']+'.2026-09-08.v1.json'],'Dated pinned GET completed; discovery retains the actual outcome.'),'rights':control([item+'/rights.json'],'Exact provider license and supplied layers assessed for local preservation and analysis.',intended_uses=['local-preservation','research-analysis']),'file':control(files,'Original file opened and parsed; exact blob, byte size, header, CTS and local SHA-256 verified.',status='present'),'branch':control([plant,readme],'Actual selected branch route reaches the exact source records and locally retained file.')}
 refs=[]
 for source in t['registry_sources']:
  p=documents[(source['corpus'],source['document_id'])]
  refs.append({**ref(p.relative_to(root).as_posix()),'corpus_id':source['corpus'],'document_id':source['document_id'],'record_id':source['entry_id']})
 targets.append({'target_id':f'registry-third-planting.{i:02}.{t["slug"]}','preferred_label':t['title'],'source_record_refs':refs,'target':{'target_kind':'exact-work-version','description':t['version_description'],'known_tos_refs':[b],'create_record_refs':t['paths'],'planned_ids':t['ids'],'expected_coverage':t['coverage'],'expected_file_count':len(t['files']),'expected_bytes':t['byte_size'],'scope_limits':t['limits']},'readiness':readiness,'acquisition':{'source_urls':[f['url'] for f in t['files']],'destination_paths':files,'intended_uses':['local-preservation','research-analysis']},'execution':control([plant,t['paths']['work']],'Exact version acquisition and branch-to-file route completed; no textual/semantic/canon acceptance.',status='completed')})
 observation=read(root/item/'forensic-observations.json')
 rows.append({**checked,'title':t['title'],'atlas_row':branches[t['slug']]['anchor_preparation']['atlas_row_id'],'version':t['repository']+'@'+t['pin'],'planting_ref':plant,'reading_route':readme,'observed_coverage':observation['files'][0],'remaining_controls':read(root/plant)['remaining_controls']})
now=datetime.now(timezone.utc).isoformat();write=lambda p,v:p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
write(base/'readiness.post-acquisition.v1.json',{'schema_version':'tos_open_work_readiness_plan_v1','reviewed_at':now,'reviewer_ref':'model:codex/session/01a08281-293a-7ff0-927a-0fd73dfacf7d','targets':targets})
summary={'schema_version':'tos_registry_first_planting_result_v1','completed_at':now,'preparation_commit':read(base/'preparation-checkpoint-receipt.json')['commit'],'normalization_snapshot':manifest['source_registry_snapshot_ref'],'planted':rows,'deferred':[r for r in read(base/'selection-review.json')['candidates'] if r['decision']=='defer'],'blocked':[],'total_files':sum(r['files'] for r in rows),'total_bytes':sum(r['bytes'] for r in rows),'scope':'51 selected Greek editions in A25/A26/A29; other versions and broader corpus leads remain open.','source_text_admitted':False,'payload_visibility':'local_only','public_or_remote_deployment':False}
write(base/'batch-result.json',summary)
lines=['# Третья посадка: 51 греческое произведение','',f"Посажена {len(rows)} точная версия: {summary['total_files']} файл, {summary['total_bytes']:,} байт. Каждый файл открыт локально; проверены Git blob, размер, SHA-256, точный CTS, заголовок, греческий текст и адреса разделов.",'','23 произведения платоновского корпуса, 9 аристотелевского, 15 Плутарха, 2 Эпиктета, по одному Марка Аврелия и Диогена Лаэртского. Спорные атрибуции остаются явно не принятыми.','','| Произведение | Ветвь | Разделов в структуре | Открыть |','| --- | --- | ---: | --- |']
for r in rows:
 c=r['observed_coverage'];lines.append(f"| {r['title']} | {r['atlas_row']} | {c['division_count']} | [Версия и локальный файл]({os.path.relpath(root/r['reading_route'],base)}) |")
lines+=['','## Отложено','',f"{len(summary['deferred'])} метаданных кандидатов сохранены без приобретения полных файлов: 14 работ Ксенофонта требуют подходящего source anchor; письма Платона — структуры коллекции; 3 фрагментно-гномологических пакета Эпиктета — модели передачи/компиляции; краткий Compendium Плутарха удержан вместе с ошибкой идентичности A26-R061. Конкретные owner-условия сохранены в [отборе](selection-review.json).",'','В A26-R061 название «Об общих понятиях против стоиков» ошибочно связано с CTS tlg137. Проверенный текст — tlg138; он посажен через корректный корпусный источник A29. Исходная строка не исправлена молча и не помечена выполненной. [Разбор](SOURCE_AND_RIGHTS_REVIEW.md#registry-identity-correction).','','[Покрытие всего реестра](../../../research-packets/source-registries/COVERAGE.md). [Полный результат](batch-result.json). [Источник, права и границы](SOURCE_AND_RIGHTS_REVIEW.md).','','Исходные bytes остаются локально в ignored Item payload; Git переносит метаданные и проверочные записи. Посадка не принимает текст, перевод, интерпретацию или канон и не означает удалённую публикацию файлов.','']
(base/'RESULTS.md').write_text('\n'.join(lines))
(base/'README.md').write_text('# Registry third planting\n\n[Results and local reading routes](RESULTS.md). [Source/rights review](SOURCE_AND_RIGHTS_REVIEW.md). [Selection and deferrals](selection-review.json).\n\nFrozen preparation, actual acquisition and completed readiness-queue history are distinct. This batch continues the existing source-witnessing mechanic.\n')
for branch_path in sorted({p['anchor_preparation']['branch_path'] for p in branches.values()}):
 branch=root/branch_path
 entries=[r for r in rows if r['planting_ref'].startswith(branch_path+'/')]
 for path in [branch/'README.md',branch/'sources/README.md']:
  old=path.read_text() if path.exists() else '# Sources\n';marker='## Registry planting: third wave (2026-09-08)'
  if marker in old:continue
  addition='\n\n'+marker+'\n\n'+f"{len(entries)} exact Greek versions with verified local files. [Batch evidence and limits]({os.path.relpath(base/'RESULTS.md',path.parent)}).\n"
  if path.parent.name=='sources':
   addition+='\n'+'\n'.join('- ['+r['title']+']('+os.path.relpath(root/r['reading_route'],path.parent)+')' for r in entries)+'\n'
  path.write_text(old.rstrip()+addition)
print(json.dumps({'planted':len(rows),'deferred':len(summary['deferred']),'files':summary['total_files'],'bytes':summary['total_bytes']}))
