import {test} from 'vitest';
import assert from 'node:assert/strict';
import {essentialContext} from './record-context.mjs';
import {materialDisplayForm} from './knowledge-client.mjs';
import {readingSnapshot,readingDocument,formLanguageNote,formLabel} from './reader-model.mjs';
import {setUiLanguage} from './ui-i18n.mjs';

const content='b'.repeat(64),revision='a'.repeat(64);
const raw=()=>({id:'opaque:version:old',kind_id:'arbitrary-future-kind',content_revision:content,source_refs:['ToS/source.json'],
  display:{title:{en:'Version 1'},summary:{default:'Старая запись',original:'Старая запись'},summary_state:'source-derived'},
  display_selection:{schema_version:'tos_display_selection_v1',content_revision:content,fields:{
    summary:{requested_language:'en',selected_key:'default',actual_language:null,text:'Старая запись',reason:'fallback',available_keys:['default','original'],content_available:true}},
    essential_context_pointers:['/attributes/a~1b/~0context/0']},
  attributes:{'a/b':{'~context':[{record:{version:1,notes:'Старая запись',unknown:{value:null,negated:false,alternatives:['a','b']}},source_ref:'ToS/old.json'}]}}});

test('language annotations follow interface switching while the supplied text and actual language remain fixed',()=>{
  const form={text:'Старая запись',key:'default',lang:'ru',fallback:true},before=structuredClone(form);
  const note=formLanguageNote(form),labels=[],headings=[],forms=[],document=readingDocument(readingSnapshot({match:raw(),packet:{source_revision:revision}},'node'),'en'),label=formLabel('original');
  try{for(const language of ['ru','en','es']){setUiLanguage(language);labels.push(String(note));headings.push(String(document.blocks[0].title));forms.push(String(label));}}
  finally{setUiLanguage('ru');}
  assert.equal(new Set(labels).size,3);assert.match(labels[0],/Выбранная форма/);assert.match(labels[1],/The selected form/);
  assert.equal(new Set(headings).size,3);assert.equal(new Set(forms).size,3);assert.equal(document.blocks[0].form.text,'Старая запись');
  assert.equal(String(formLabel('toString')),'toString');assert.equal(String(formLabel('zh-Hant')),'zh-Hant');
  assert.deepEqual(form,before);
});

test('declared context preserves complete opaque values, escaped pointers and explicit nulls',()=>{
  const record=raw(),before=structuredClone(record),value=record.attributes['a/b']['~context'][0];
  const result=essentialContext(record);
  assert.deepEqual(result,{state:'available',items:[{pointer:'/attributes/a~1b/~0context/0',state:'available',value}]});
  assert.deepEqual(record,before);
  value.record.unknown.value='later mutation';assert.equal(result.items[0].value.record.unknown.value,null);
  record.display_selection.essential_context_pointers=['/attributes/a~1b/~0context/0/record/unknown/value'];
  value.record.unknown.value=null;assert.equal(essentialContext(record).items[0].value,null);
});

test('missing or malformed pointers remain visible gaps without inferring a current record or following prototypes',()=>{
  const record=raw();record.attributes.current={notes:'NEW BODY MUST NOT BE SUBSTITUTED'};
  record.display_selection.essential_context_pointers=['/attributes/absent','/attributes/toString','/attributes/a~2b','relative','/attributes/a~1b/~0context/00',null];
  const context=essentialContext(record);
  assert.equal(context.state,'incomplete');assert.equal(context.items.length,6);
  assert.ok(context.items.every(item=>item.state==='unavailable'&&!Object.hasOwn(item,'value')));
  record.display_selection.content_revision='c'.repeat(64);
  assert.deepEqual(essentialContext(record),{state:'unavailable',items:[]});
  record.display_selection.content_revision=content;record.display_selection.essential_context_pointers={};
  assert.deepEqual(essentialContext(record),{state:'unavailable',items:[]});
});

test('legacy absence differs from an explicitly empty declaration',()=>{
  const record=raw();delete record.display_selection;
  assert.deepEqual(essentialContext(record),{state:'not-declared',items:[]});
  record.display_selection=raw().display_selection;record.display_selection.essential_context_pointers=[];
  assert.deepEqual(essentialContext(record),{state:'available',items:[]});
});

test('the bounded reading snapshot retains declared context outside its ordinary raw field allowlist',()=>{
  const record=raw(),snapshot=readingSnapshot({match:record,packet:{source_revision:revision}},'node');
  const context=essentialContext(record);assert.equal(snapshot.raw.attributes,undefined);
  record.attributes['a/b']['~context'][0].record.notes='CURRENT BODY';
  assert.deepEqual(readingDocument(snapshot,'en').essentialContext,context);
  assert.equal(readingDocument(snapshot,'en').essentialContext.items[0].value.record.notes,'Старая запись');
  assert.equal(readingDocument(snapshot,'en').blocks[0].form.lang,null);
});

test('material fields use only the same response revision and preserve declared actual language',()=>{
  const record=raw();record.display_selection.fields.summary.actual_language='ru';
  assert.deepEqual(materialDisplayForm(record,'summary','en'),{text:'Старая запись',key:'default',lang:'ru',fallback:true});
  const document=readingDocument(readingSnapshot({match:record,packet:{source_revision:revision}},'node'),'en');
  assert.equal(document.blocks[0].form.lang,'ru');
  record.display_selection.content_revision='c'.repeat(64);assert.equal(materialDisplayForm(record,'summary','en'),null);
  record.display_selection.content_revision=content;
  for(const invalid of [null,undefined,{},[],false]){
    record.display_selection.fields.summary=invalid;assert.equal(materialDisplayForm(record,'summary','en'),null);
  }
  delete record.display_selection;assert.equal(materialDisplayForm(record,'summary','en').lang,null);
});
