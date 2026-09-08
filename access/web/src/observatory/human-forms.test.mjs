import {test,expect} from 'vitest';
import {validateHumanForms,formView,formIdentity,contentLanguage,FormContractError,resolveClaimReading} from './human-forms.mjs';
import {KnowledgeClient,RevisionError} from './knowledge-client.mjs';
import {readingSnapshot,readingDocument,readingPositionKey,readingKey,createReadingShelf} from './reader-model.mjs';
import {validateReading} from './reading-resume.mjs';
import {nodePreview} from './graph-preview.mjs';
import {formNode,formLens,revision,content} from '../../fixtures/human-form-data.mjs';

test('every delivered role retains its exact wording, complete context, values and form version',()=>{
  const raw=formNode(),before=structuredClone(raw),view=formView(raw);
  expect(view.roles).toHaveLength(7);
  for(const role of view.roles){expect(role.packet.display_text.endsWith('Это НЕ подтверждение.')).toBe(true);expect(role.packet.context[0].value.unknown).toEqual({zero:0,false:false,null:null,empty:''});}
  const snapshot=readingSnapshot({packet:formLens([raw]),match:raw},'node');
  expect(readingDocument(snapshot).blocks).toEqual([]);expect(snapshot.raw.human_form_selection).toEqual(raw.human_form_selection);
  expect(raw).toEqual(before);raw.human_form_selection.roles.statement.packet.context[0].value.unknown.zero=7;
  expect(snapshot.raw.human_form_selection.roles.statement.packet.context[0].value.unknown.zero).toBe(0);
  expect(nodePreview(formLens([raw]),raw).body).not.toContain('Текст формы');
  expect(nodePreview(formLens([raw]),raw).body).toContain('контекст');
});

test.each([
  ['missing context',s=>delete s.roles.statement.packet.context],
  ['context silently made standalone',s=>s.roles.statement.packet.standalone_reading=true],
  ['mismatched form digest',s=>s.roles.statement.packet.form={...s.roles.statement.form,digest:'sha256:'+'e'.repeat(64)}],
  ['mismatched material revision',s=>s.content_revision='f'.repeat(64)],
  ['nonready wording leak',s=>s.roles.statement.state='unavailable'],
  ['false assessment authority',s=>s.performs_assessment=true],
  ['lost JSON value',s=>delete s.roles.statement.packet.context[0].value],
  ['missing role',s=>delete s.roles.technical],
  ['wrong candidate identity',s=>s.candidates.find(c=>c.role==='statement').form={...s.roles.statement.form,version:2}],
  ['unbounded envelope',s=>s.roles.statement.packet.context[0].value.extra='x'.repeat(20000)],
])('invalid delivery is rejected: %s',(_label,mutate)=>{const raw=formNode();mutate(raw.human_form_selection);expect(()=>validateHumanForms(raw)).toThrow(FormContractError);});

test('absence, selection states, matching diagnostics, unknown language and fallback remain distinct',()=>{
  const raw=formNode(),s=raw.human_form_selection;
  s.roles.name={state:'unavailable',reason:'no-ready-form',form:null,packet:null};s.candidates[0].state='stale';
  s.roles.hover={state:'ambiguous',reason:'multiple-forms',form:null,packet:null};
  s.roles.grounds={state:'over-budget',reason:'inspect-exact-form',form:s.roles.grounds.form,packet:null};
  s.requested_language='es';for(const selected of Object.values(s.roles))if(selected.packet)selected.reason='fallback';
  s.roles.caption.packet.language=null;s.candidates.find(c=>c.role==='caption').language=null;
  const view=formView(raw);expect(view.roles.find(r=>r.role==='name').candidates[0].state).toBe('stale');
  expect(view.roles.find(r=>r.role==='statement').candidates.every(c=>c.state==='ready')).toBe(true);
  expect(view.roles.find(r=>r.role==='caption').packet.language).toBeNull();expect(view.roles.find(r=>r.role==='hover').packet).toBeNull();
  expect(()=>validateHumanForms(raw,'ru')).toThrow();delete raw.human_form_selection;expect(validateHumanForms(raw)).toBeNull();
});

test('isolated reading uses a real full LensResult and rejects extra objects or another revision',async()=>{
  const raw=formNode(),packet=formLens([raw]);let sent;
  const client=new KnowledgeClient({fetcher:async(url,options)=>{sent={url,spec:JSON.parse(options.body)};return {ok:true,json:async()=>packet};}});
  const value=await client.readMaterial('node',raw.id,undefined,revision,content,{language:'ru'});
  expect(sent.url).toBe('/api/knowledge/lenses/compile');expect(sent.spec.detail).toBe('full');expect(sent.spec.traversal.depth).toBe(0);
  expect(sent.spec.seed.focus_node_id).toBe(raw.id);expect(value.packet.schema).toBe('tos_lens_result_v1');expect(value.packet).toBe(packet);
  await expect(client.readMaterial('node',raw.id,undefined,'f'.repeat(64))).rejects.toBeInstanceOf(RevisionError);
  packet.nodes.push({...raw,id:'other'});await expect(client.readMaterial('node',raw.id)).rejects.toThrow();
});

test('compact Claim reading resolves the full packet and all context, never a wording substring',()=>{
  const raw=formNode(),packet=formLens([raw],[{id:'context-relation',qualifiers:{negation:false}}]);
  const reading={mode:'claim-with-mandatory-context',node_id:raw.id,content_revision:content,
    wording_pointer:'/human_form_selection/roles/statement/packet',wording_state:'available',context_pointers:['/semantics','/epistemic'],relation_context_ids:['context-relation'],standalone:false};
  const resolved=resolveClaimReading(packet,reading);expect(resolved.wording).toEqual(raw.human_form_selection.roles.statement.packet);
  expect(resolved.relations[0].qualifiers.negation).toBe(false);
  expect(()=>resolveClaimReading(packet,{...reading,wording_pointer:reading.wording_pointer+'/display_text'})).toThrow();
  expect(()=>resolveClaimReading({...packet,relations:[]},reading)).toThrow();
  expect(()=>resolveClaimReading(packet,{...reading,content_revision:'f'.repeat(64)})).toThrow();
});

test('reading persistence retains exact form references and anchors without source wording',()=>{
  const raw=formNode(),snapshot=readingSnapshot({packet:formLens([raw]),match:raw},'node'),key=readingKey('node',raw.id);
  const saved=validateReading({v:1,activeKey:key,entries:[{kind:'node',id:raw.id,sourceRevision:revision,contentRevision:content,preferred:'ru',
    positions:[[readingPositionKey(key,snapshot,'ru'),{top:100,details:[],anchor:{key:'form:statement:context:0',offset:2}}]]}]});
  expect(JSON.stringify(saved)).not.toContain('Текст формы');expect(JSON.stringify(saved)).not.toContain('НЕ доказано');
  expect(JSON.stringify(saved)).toContain(raw.human_form_selection.roles.statement.form.digest);
  const other=formNode();other.human_form_selection.roles.statement.form.version=2;
  expect(formIdentity(other)).not.toBe(formIdentity(raw));
});

test('a late language response cannot replace the newer choice and malformed refresh clears forms',async()=>{
  let resolveOld;const pending=new Promise(resolve=>resolveOld=resolve);let calls=0;
  const answer=raw=>({packet:formLens([raw]),match:raw});
  const client={readMaterial:async(_kind,_id,_signal,_revision,_content,{language})=>{
    if(++calls===1)return pending;
    if(calls===3)throw new FormContractError();
    return answer(formNode('fixture:forms',language));
  }};
  const shelf=createReadingShelf({client}),raw=formNode(),{key}=shelf.pin({raw,kind:'node',sourceRevision:revision});
  await shelf.language(key,'en');resolveOld(answer(raw));await new Promise(resolve=>setTimeout(resolve,0));
  expect(shelf.entries[0].snapshot.raw.human_form_selection.requested_language).toBe('en');
  await shelf.refresh(key);expect(shelf.entries[0].snapshot).toBeNull();
});


test('language syntax and reported language choice cannot relabel source wording',()=>{
  expect(contentLanguage('zh-Hant')).toBe(true);expect(contentLanguage('x-source')).toBe(true);
  expect(contentLanguage('ru\n')).toBe(false);expect(contentLanguage(' ru')).toBe(false);
  const raw=formNode();raw.human_form_selection.requested_language='en';
  expect(()=>validateHumanForms(raw)).toThrow();
  const original=formNode();original.human_form_selection.requested_language='original';
  for(const selected of Object.values(original.human_form_selection.roles))selected.reason='original';
  expect(()=>validateHumanForms(original)).toThrow();
});
