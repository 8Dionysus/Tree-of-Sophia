import {test,expect} from 'vitest';
import {languageName,sourceLinkLabel,relationLabel,fileLabel} from './human-presentation.mjs';
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
  const raw={predicate_id:'future-type',display:{label:{ru:'Связь, описанная источником'}}},before=structuredClone(raw);
  expect(relationLabel(raw,'ru')).toBe('Связь, описанная источником');expect(raw).toEqual(before);
  expect(relationLabel({predicate_id:'future-type',display:{default:'future-type'}},'ru')).toBe('Связь');
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
