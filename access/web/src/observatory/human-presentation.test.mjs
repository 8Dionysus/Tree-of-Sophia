import {test,expect} from 'vitest';
import {languageName,sourceLinkLabel,relationLabel,fileLabel,sourceTitle} from './human-presentation.mjs';
import {displayTitleForm} from './knowledge-client.mjs';

test('language and source labels are human without exposing paths',()=>{
  expect(languageName('en','ru')).toBe('английский');
  expect(languageName('de','ru')).toBe('немецкий');
  expect(sourceLinkLabel('https://archive.org/details/some-long-machine-id')).toBe('Internet Archive');
  expect(sourceLinkLabel('ToS/private/very/long/record.json')).toBe('Источник');
});
test('file cards retain the format and size needed to choose a source',()=>{
  expect(fileLabel({media_type:'application/pdf',byte_size:16228737},'ru')).toBe('PDF · 16,2 МБ');
  expect(fileLabel({media_type:'application/vnd.djvu+xml',byte_size:8882082},'ru')).toBe('DjVu XML · 8,9 МБ');
});
test('known predicate names are localized while source wording and future identities stay intact',()=>{
  expect(relationLabel({predicate_id:'transmits_to',display:{default:'transmits_to'}},'ru')).toBe('передаёт');
  expect(relationLabel({predicate_id:'contested_by',display:{label:{default:'contested_by'}}},'ru')).toBe('оспаривается');
  const navigation={has_expression:['Текст или перевод','Text or translation','Texto o traducción'],
    translated_by:['Переводчик','Translator','Traductor'],
    has_normalized_agent:['Участник','Participant','Participante'],
    has_normalized_place:['Место','Place','Lugar']};
  for(const [id,labels] of Object.entries(navigation))for(const [index,language] of ['ru','en','es'].entries())
    expect(relationLabel({predicate_id:id,display:{default:id}},language)).toBe(labels[index]);
  const raw={predicate_id:'future-type',display:{label:{ru:'Связь, описанная источником'}}},before=structuredClone(raw);
  expect(relationLabel(raw,'ru')).toBe('Связь, описанная источником');expect(raw).toEqual(before);
  expect(relationLabel({predicate_id:'future-type',display:{default:'future-type'}},'ru')).toBe('Связь');
});
test('source filenames and titles survive locale fallback while paths and dumps stay hidden',()=>{
  const file={kind_id:'file',display:{title:{default:'Ницше Так говорил Заратустра 1913.pdf',ru:null,en:null},kind_label:{ru:'Файл',en:'File',es:'Archivo'},provenance:{title:'projected-label',source_title_available:true}},attributes:{media_type:'application/pdf',byte_size:16228737}};
  const item={kind_id:'item',display:{title:{default:'Also sprach Zarathustra, 1913'},kind_label:{ru:'Материал',en:'Item',es:'Material'},provenance:{title:'projected-label'}}};
  for(const language of ['ru','en','es']){
    expect(displayTitleForm(file,language)).toMatchObject({text:file.display.title.default,key:'default',fallback:true});
    expect(displayTitleForm(item,language).text).toBe(item.display.title.default);
  }
  expect(sourceTitle(file)).toBe(file.display.title.default);
  for(const title of ['ToS/source-witnesses/file.pdf','/srv/data/record.json','source-navigation:tos.file.opaque','{"id":"tos.file.opaque"}']){
    const raw={kind_id:'file',display:{title:{default:title},kind_label:{ru:'Файл'},provenance:{title:'projected-path'}},attributes:{media_type:'application/pdf',byte_size:1024}};
    expect(displayTitleForm(raw,'ru').text).toBe('PDF · 1 КБ');
  }
});
test('seeded technical record titles do not leak opaque IDs and do not replace supplied names',()=>{
  let seed=915236;const kinds=['text-unit','anchor','item','file','link'];
  for(let index=0;index<100;index++){
    seed=(Math.imul(seed,1664525)+1013904223)>>>0;
    const raw={kind_id:kinds[seed%kinds.length],display:{title:{default:'tos.record.'+seed},kind_label:{ru:'Материал',en:'Material'},provenance:{title:'projected-label'}}},before=structuredClone(raw);
    expect(displayTitleForm(raw,'ru').text).not.toContain('tos.record.');expect(raw).toEqual(before);
    raw.display.title.ru='Название из источника '+index;expect(displayTitleForm(raw,'ru').text).toBe(raw.display.title.ru);
  }
});
test('generated claim navigation uses a short type label while authored names remain intact',()=>{
  for(const title of ['navigation-template','identifier-fallback']){
    const raw={kind_id:'claim',display:{title:{ru:'Запись утверждения · связь: содержит [исходная запись не оценена]'},kind_label:{ru:'Утверждение с доказательствами'},provenance:{title}}},before=structuredClone(raw);
    expect(displayTitleForm(raw,'ru').text).toBe('Утверждение');
    expect(displayTitleForm(raw,'en').text).toBe('Claim');expect(raw).toEqual(before);
    raw.display.provenance.title='source-bound-navigation';
    expect(displayTitleForm(raw,'ru').text).toBe(raw.display.title.ru);
  }
});
test('claims retain an owner-supplied readable title when source context is available',()=>{
  const raw={kind_id:'claim',display:{title:{default:'Перевод Так говорил Заратустра'},kind_label:{ru:'Утверждение'},provenance:{title:'navigation-template',source_title_available:true}},semantics:{claim:{source_predicate_id:'translated_by'}}};
  expect(displayTitleForm(raw,'ru').text).toBe(raw.display.title.default);
  expect(displayTitleForm({...raw,display:{...raw.display,provenance:{title:'navigation-template',source_title_available:false}}},'ru').text).toBe('Утверждение · Переводчик');
});
