from pathlib import Path
from datetime import datetime,timezone
import sys,json,hashlib,os
root=Path(__file__).resolve().parents[5];sys.path.insert(0,str(root/'scripts'))
from acquire_registry_sources import verify_target
base=root/'ToS/source-witnesses/discovery/registry-first-planting-2026-09-08'
manifest=json.loads((base/'manifest.json').read_text());pre=json.loads((base/'readiness.pre-acquisition.v1.json').read_text())
branches={p['target_slug']:p for p in json.loads((root/'ToS/philosophy/source-planting-preparation/cross-branch-source-anchors-20260908.json').read_text())['targets']}
def ref(path):
 p=root/path;return {'path':path,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
rows=[]
for target,ready in zip(manifest['targets'],pre['targets'],strict=True):
 result=verify_target(root,target)
 correction_path=base/'oraec-scope-correction/current.json'
 correction=(json.loads(correction_path.read_text()).get('targets',{}).get(target['slug'],{}) if correction_path.exists() else {})
 if correction:ready['target']['description']=correction['corrected_version_description'];ready['target']['scope_limits']+=correction['limits']
 b=branches[target['branch_target_slug']]['anchor_preparation']['branch_path'];plant=b+'/sources/plantings/registry-'+target['slug']+'/source-planting.json';readme=str(Path(plant).with_name('README.md'))
 planting=json.loads((root/plant).read_text());assert planting['source_witness']['work_id']==target['ids']['work']
 ready['readiness']['file']={'status':'present','evidence_posture':'owner-reviewed','owner_refs':[ref(p) for p in ready['acquisition']['destination_paths']],'rationale':'Every immutable local file opened, parsed and matched exact prepared blob/size plus computed SHA-256; Item fixity closes.'}
 ready['readiness']['branch']['owner_refs']=[ref(plant),ref(readme)]
 ready['readiness']['branch']['rationale']='Actual branch planting and README resolve Work, exact version, rights, coverage and local source files.'
 ready['readiness']['access']['owner_refs']=[ref('ToS/source-witnesses/discovery/runs/registry-'+target['slug']+'.2026-09-08.v1.json')]
 ready['readiness']['access']['rationale']='Actual pinned GET transfer completed and local source bytes verified; discovery contains dated outcomes.'
 ready['readiness']['rights']['owner_refs']=[ref(target['paths']['item_root']+'/rights.json')]
 ready['execution']={'status':'completed','evidence_posture':'owner-reviewed','owner_refs':[ref(plant),ref(target['paths']['work'])],'rationale':'Source acquisition, exact version custody and branch-to-local-file route completed. Textual/semantic/canon acceptance remains outside this execution.'}
 rows.append({**result,'title':target['title'],'atlas_row':planting['atlas_row_id'],'version':target['repository']+'@'+target['pin'],'planting_ref':plant,'reading_route':readme,'coverage':target['coverage'],'remaining_controls':planting['remaining_controls']})
pre['reviewed_at']=datetime.now(timezone.utc).isoformat();(base/'readiness.post-acquisition.v1.json').write_text(json.dumps(pre,ensure_ascii=False,indent=2)+'\n')
summary={'schema_version':'tos_registry_first_planting_result_v1','completed_at':pre['reviewed_at'],'preparation_commit':'8861fecf8e328a874887e875b255876061c80ac9','normalization_snapshot':manifest['source_registry_snapshot_ref'],'planted':rows,'deferred':[],'blocked':[],'total_files':sum(r['files'] for r in rows),'total_bytes':sum(r['bytes'] for r in rows),'scope':'13 selected work versions; earlier unrelated backlog remains outside this completed batch','source_text_admitted':False,'payload_visibility':'local_only','public_or_remote_deployment':False}
(base/'batch-result.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n')
lines=['# Первая посадка из нормализованного реестра','',f"Посажено {len(rows)} конкретных версий: {summary['total_files']} исходных файлов, {summary['total_bytes']:,} байт. Все файлы реально существуют локально, открыты и сверены с закреплённой upstream-версией. В этой партии отложенных и заблокированных нет.",'','Метаданные и проверочные записи отслеживаются Git. Исходные тексты сохраняются локально в ignored Item payload; перенос одного Git-коммита не переносит эти файлы. Тексты не получили филологического, семантического или канонического принятия.','','| Произведение / выбранный фрагмент | Ветвь | Файлы | Локальный маршрут |','| --- | --- | ---: | --- |']
for r in rows:lines.append(f"| {r['title']} | {r['atlas_row']} | {r['files']} | [Открыть версию и файлы]({os.path.relpath(root/r['reading_route'],base)}) |")
lines+=['','[Машинная сводка: точные commits, покрытие и ограничения](batch-result.json). [Исходный отбор и права](SOURCE_AND_RIGHTS_REVIEW.md). [Нормализованный исследовательский корпус](../../../research-packets/source-registries/README.md).','','Притчи сначала прошли путь отдельно: 31 глава, 915 стихов, 6 984 OSIS word elements. После исправления служебных companions и успешного source-foundation validator продолжены остальные 12. Египетские позиции остаются двумя ограниченными частями EA 10684; египетский текст и немецкий перевод/глоссы имеют отдельные Expressions и адреса полей в общем исходном JSON.','']
(base/'RESULTS.md').write_text('\n'.join(lines));print(json.dumps({'planted':len(rows),'files':summary['total_files'],'bytes':summary['total_bytes']}))
