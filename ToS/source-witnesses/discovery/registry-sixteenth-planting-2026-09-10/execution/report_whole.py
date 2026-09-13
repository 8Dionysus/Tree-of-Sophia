"""Close the two stages without replacing either preparation or custody record."""
from pathlib import Path
from datetime import datetime,timezone
import json,os,sys
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'scripts/acquire_registry_sources.py').is_file())
BASE=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from acquire_registry_sources import verify_target
from source_registry_common import read
pali=read(BASE/'manifest.json');english=read(BASE/'translations/manifest.json');result=read(BASE/'translations/batch-result.json')
roots={t['coverage']['uid']:t for t in pali['targets']}
expected={row['path'].split('_')[0] for row in read(BASE/'candidate-files.json') if row['language']=='pli'}
assert len(expected)==329
if set(roots)!=expected or {t['coverage']['uid'] for t in english['targets']}!=expected:raise ValueError('paired corpus UID closure differs')
rows=[];by_slug={row['target_slug']:row for row in result['planted']}
for t in english['targets']:
    uid=t['coverage']['uid'];old=roots[uid]
    if t['ids']['work']!=old['ids']['work']:raise ValueError('parallel versions do not share the reviewed Work')
    a=verify_target(ROOT,old);b=verify_target(ROOT,t);reading=by_slug[t['slug']]['reading_route']
    rows.append({'uid':uid,'work_id':t['ids']['work'],'work_ref':t['paths']['work'],'pali_title':old['title'],'english_title':t['title'],'pali_item_manifest_ref':a['item_manifest_ref'],'english_item_manifest_ref':b['item_manifest_ref'],'pali_preexisting':False,'reading_route':reading})
rows.sort(key=lambda row:tuple(map(int,row['uid'][2:].split('.'))))
new_bytes=sum(t['byte_size'] for m in (pali,english) for t in m['targets'])
assert len(rows)==329 and new_bytes==1185706
whole={'completed_at':datetime.now(timezone.utc).isoformat(),'scope':'Two stages of the sixteenth planting: 329 new Pali versions and 329 English Sujato translations, sharing 329 newly created Work identities','stage_results':['batch-result.json','translations/batch-result.json'],'new_works':329,'new_versions':658,'new_files':658,'new_bytes':new_bytes,'paired_works':329,'reused_Pali_versions':0,'pairs':rows,'deferred_files_ref':'deferred-files.json','source_text_admitted':False,'translation_quality_assessed':False,'semantic_alignment_admitted':False,'public_payload_delivery':False,'AI_derivative_use_admitted':False}
(BASE/'whole-batch-result.json').write_text(json.dumps(whole,ensure_ascii=False,indent=2)+'\n')
lines=['# Шестнадцатая поставка: Самъютта-никая SN 23–35','','**658 новые версии, 658 файла, 1 185 706 байт.** Добавлено 329 новое произведение. Каждое произведение представлено на пали и в английском переводе Суджато.','','Это 329 отдельно обозначенная сутта из SN 23–35 выбранной цифровой редакции. 49 групповых UID (98 файлов двух языков) отложены до отдельного описания состава; полнота SN 23–35 не заявлена. Оно не означает полноту древней устной традиции, всех редакций или Āgama-параллелей.','','| UID | Палийское название | Английское название | Обе версии |','| --- | --- | --- | --- |']
for row in rows:lines.append(f"| {row['uid']} | {row['pali_title']} | {row['english_title']} | [Читать]({os.path.relpath(ROOT/row['reading_route'],BASE)}) |")
lines+=['','[Палийский этап](PALI_RESULTS.md) · [Английский этап](translations/RESULTS.md) · [Полный результат](whole-batch-result.json) · [Источники и права](SOURCE_AND_RIGHTS_REVIEW.md)','','Исходные файлы сохранены без изменений и исключены из Git. Метаданные, права, provenance, SHA-256 и маршруты чтения отслеживаются. Переводы и палийские тексты имеют отдельные Expression, Edition и Item; история существующих Work и форм сохранена. Общие segment IDs не объявлены принятым выравниванием.','','CC0 переводов и отдельная просьба Суджато об ограничении использования в ИИ сохранены раздельно. Поставка предназначена для локального хранения и чтения; публикация текстов, производные ИИ-материалы, качество перевода, интерпретация и канон не приняты. CI и merge подтверждаются отдельно.','']
(BASE/'RESULTS.md').write_text('\n'.join(lines));(BASE/'README.md').write_text('# Sixteenth registry planting\n\n[329 paired reading routes and whole result](RESULTS.md). [Source and rights review](SOURCE_AND_RIGHTS_REVIEW.md).\n\n[329 new Pali versions](PALI_RESULTS.md) precede [329 English Work extensions](translations/RESULTS.md). Each stage retains its original preparation, checkpoint, custody and form history.\n')
print(json.dumps({'new_versions':658,'new_works':329,'paired_works':329,'bytes':new_bytes}))
