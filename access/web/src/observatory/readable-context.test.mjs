import {test,expect} from 'vitest';
import {createHash} from 'node:crypto';
import {verifyReadableContext,readableContextFor} from './readable-context.mjs';
import {FormContractError} from './human-forms.mjs';
import {KnowledgeClient} from './knowledge-client.mjs';
import {readingSnapshot} from './reader-model.mjs';
import {formLens} from '../../fixtures/human-form-data.mjs';

const hash=text=>'sha256:'+createHash('sha256').update(text).digest('hex');
const json='{"record_id":"tos.fixture.context","record_version":1,"negative":false,"unknown":null,"integer":9007199254740993,"float":1.0,"word":"Не переводить"}';
function fixture(){
  const record=JSON.parse(json),reference={id:record.record_id,version:1,digest:hash(json)},path='/attributes/source_record';
  const entries=Object.entries(record).map(([key,value])=>({key,category:key==='record_id'?'technical':key==='negative'?'governing':'unclassified',
    label:{ru:key,en:'Owner '+key},explanation:null,value_mode:key==='record_id'?'exact-reference':'source-value',
    ...(key==='record_id'?{}:{value}),value_label:null,language:null,script:null,
    binding:{kind:'record',record:reference,source_pointer:'/'+key},value_pointer:path+'/'+key}));
  return {id:'fixture:context',content_revision:'b'.repeat(64),kind_id:'work',source_refs:['ToS/fixture.json'],display:{title:{ru:'Материал'}},
    attributes:{source_record:record},readable_context:{schema_version:'tos_readable_context_v1',state:'complete',reason:'classified',
      vocabulary:{id:'tos.context-presentation.governing',version:1,source_ref:'ToS/doctrine/semantic-interchange/entity-types.v1.json',digest:hash('vocabulary')},
      contexts:[{origin_pointer:path,form:null,entries}],exact_materials:[{digest:hash(json),canonical_json:json,origin_pointers:[path]}],
      exact_context_pointers:[path],coverage:{input_contexts:1,returned_contexts:1,entries:entries.length,unclassified_entries:entries.length-2},
      performs_semantic_assessment:false,performs_translation:false}};
}

test('lossless values, unknowns, negation and owner labels survive a verified consumer snapshot',async()=>{
  const raw=fixture(),before=structuredClone(raw),view=await verifyReadableContext(raw),entries=view.contexts[0].entries;
  expect(entries.find(entry=>entry.key==='integer').display.text).toBe('9007199254740993');
  expect(entries.find(entry=>entry.key==='float').display.text).toBe('1.0');
  expect(entries.find(entry=>entry.key==='negative').display.text).toBe('false');
  expect(entries.find(entry=>entry.key==='unknown').display.text).toBe('null');
  expect(entries.find(entry=>entry.key==='word').display.text).toBe('Не переводить');
  expect(entries.find(entry=>entry.key==='word').label.en).toBe('Owner word');
  expect(raw).toEqual(before);
  const snapshot=readingSnapshot({packet:formLens([raw]),match:raw},'node');
  expect(snapshot.readableContext).toEqual(view);expect(snapshot.raw.attributes).toBeUndefined();
  view.contexts[0].entries[0].label.en='changed';expect(readableContextFor(raw)).not.toEqual(view);
});

test.each([
  ['digest',raw=>raw.readable_context.exact_materials[0].digest=hash('wrong')],
  ['source value',raw=>raw.attributes.source_record.negative=true],
  ['pointer',raw=>raw.readable_context.contexts[0].entries[0].value_pointer='/attributes/source_record/missing'],
  ['binding',raw=>raw.readable_context.contexts[0].entries[0].binding.source_pointer='/word'],
  ['lost entry',raw=>{raw.readable_context.contexts[0].entries.pop();raw.readable_context.coverage.entries--;raw.readable_context.coverage.unclassified_entries--;}],
  ['duplicate entry',raw=>raw.readable_context.contexts[0].entries[1]=structuredClone(raw.readable_context.contexts[0].entries[0])],
  ['lost context',raw=>{raw.readable_context.contexts=[];raw.readable_context.exact_context_pointers=[];Object.keys(raw.readable_context.coverage).forEach(key=>raw.readable_context.coverage[key]=0);}],
  ['invented authority',raw=>raw.readable_context.performs_semantic_assessment=true],
  ['unclassified count',raw=>raw.readable_context.coverage.unclassified_entries=0],
  ['ambiguous material',raw=>raw.readable_context.exact_materials.push({...raw.readable_context.exact_materials[0],digest:hash(json.replace('9007199254740993','9007199254740992')),canonical_json:json.replace('9007199254740993','9007199254740992')})],
])('refuses changed or incomplete sidecar before reading: %s',async(_label,mutate)=>{
  const raw=fixture();mutate(raw);await expect(verifyReadableContext(raw)).rejects.toThrow();
  expect(()=>readableContextFor(raw)).toThrow(FormContractError);
});

test('cached verification cannot survive changed source data or sidecar',async()=>{
  const raw=fixture();await verifyReadableContext(raw);raw.attributes.source_record.negative=true;
  expect(()=>readableContextFor(raw)).toThrow(FormContractError);
  await expect(verifyReadableContext(raw)).rejects.toThrow();
  expect(()=>readableContextFor(raw)).toThrow(FormContractError);
});

test('a form can reference retained source fields without duplicating each exact material',async()=>{
  const raw=fixture(),source=raw.readable_context.contexts[0],form={id:'tos.form.fixture',version:1,digest:hash('form')};
  raw.attributes.human_forms=[{state:'ready',form,context:source.entries.map(entry=>({binding:{record:entry.binding.record,pointer:entry.binding.source_pointer},value:raw.attributes.source_record[entry.key]}))}];
  source.form=form;source.origin_pointer='/attributes/human_forms/0/context';raw.readable_context.exact_context_pointers=[source.origin_pointer];
  source.entries.forEach((entry,index)=>entry.value_pointer=source.origin_pointer+'/'+index+'/value');
  expect((await verifyReadableContext(raw)).contexts[0].entries.find(entry=>entry.key==='integer').display.text).toBe('9007199254740993');
  raw.attributes.human_forms[0].context[0].binding.record={...raw.attributes.human_forms[0].context[0].binding.record,version:2};
  await expect(verifyReadableContext(raw)).rejects.toThrow();
});

test('assertion qualifiers bind to the exact source field and cannot erase conflicts',async()=>{
  const raw=fixture(),path='/semantics/assertion_contexts/0',digest='a'.repeat(64);
  const assertion={schema_version:'tos_assertion_context_v1',source_record_digest:digest,fields:{negative:{value:false,source_pointer:'/qualifiers/negative'}},conflicts:['disputed']};
  raw.semantics={assertion_contexts:[assertion]};
  const entries=['negative','conflicts'].map(key=>({key,category:'unclassified',label:{en:key},explanation:null,
    value_mode:'source-value',value:key==='negative'?false:assertion.conflicts,value_label:null,language:null,script:null,
    binding:{kind:'assertion-context',source_record_digest:digest,source_pointer:key==='negative'?'/qualifiers/negative':''},
    value_pointer:path+(key==='negative'?'/fields/negative/value':'/conflicts')}));
  raw.readable_context.contexts.push({origin_pointer:path,form:null,entries});raw.readable_context.exact_context_pointers.push(path);
  raw.readable_context.exact_materials.push({digest:hash(JSON.stringify(assertion)),canonical_json:JSON.stringify(assertion),origin_pointers:[path]});
  Object.assign(raw.readable_context.coverage,{input_contexts:2,returned_contexts:2,entries:9,unclassified_entries:7});
  expect((await verifyReadableContext(raw)).contexts[1].entries[1].display.text).toBe('["disputed"]');
  entries[0].binding.source_pointer='/other';await expect(verifyReadableContext(raw)).rejects.toThrow();
});

test('over-budget is an explicit gap, never a complete empty context',async()=>{
  const raw=fixture();Object.assign(raw.readable_context,{state:'requires-exact-context',contexts:[],exact_materials:[],
    coverage:{input_contexts:1,returned_contexts:0,entries:0,unclassified_entries:0}});
  expect((await verifyReadableContext(raw)).state).toBe('requires-exact-context');
  raw.readable_context.state='complete';await expect(verifyReadableContext(raw)).rejects.toThrow();
});

test('cached gaps retain negative zero and overflowed JSON integer distinctions',async()=>{
  for(const [before,after]of [[-0,0],[Infinity,null]]){
    const raw=fixture();raw.attributes.source_record.integer=before;
    Object.assign(raw.readable_context,{state:'requires-exact-context',contexts:[],exact_materials:[],
      coverage:{input_contexts:1,returned_contexts:0,entries:0,unclassified_entries:0}});
    await verifyReadableContext(raw);raw.attributes.source_record.integer=after;
    expect(()=>readableContextFor(raw)).toThrow(FormContractError);
  }
});

test('the real KnowledgeClient material route verifies before returning to either reader',async()=>{
  const raw=fixture(),client=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>formLens([raw])})});
  const material=await client.readMaterial('node',raw.id,null,null,null);
  expect(readingSnapshot(material,'node').readableContext.state).toBe('complete');
  raw.readable_context.contexts[0].entries[0].binding.source_pointer='/word';
  await expect(client.readMaterial('node',raw.id,null,null,null)).rejects.toThrow();
});

test('older servers without a sidecar keep the existing reader contract',async()=>{
  const raw=fixture();delete raw.readable_context;
  expect(await verifyReadableContext(raw)).toBeNull();expect(readableContextFor(raw)).toBeNull();
});
