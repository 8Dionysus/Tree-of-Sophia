import {test,expect} from 'vitest';
import {encodeHumanFormSelection,boundedFormCost,FORM_WIRE_BUDGET} from '../../../shared/human-form-selection-codec.ts';
import {validateHumanForms,formView,formIdentity,formLanguages,resolveClaimReading,claimPathClosure,FormContractError} from './human-forms.mjs';
import {KnowledgeClient,RevisionError,displayTitle,nodeLabels,claimMaterialReference} from './knowledge-client.mjs';
import {nodePreview} from './graph-preview.mjs';
import {essentialContext} from './record-context.mjs';
import {readingSnapshot,readingDocument,readingKey,readingPositionKey,createReadingShelf} from './reader-model.mjs';
import {validateReading} from './reading-resume.mjs';
import {formNode,formLens,compactFormLens,memberFormLens,revision,content} from '../../fixtures/human-form-data.mjs';

// Use the producer's codec, not an independently recreated v2 fixture format.
const sharedNode=raw=>({...structuredClone(raw),human_form_selection:encodeHumanFormSelection(raw.human_form_selection)});
function sharedLens(packet){
  const result=structuredClone(packet);
  result.nodes=result.nodes.map(raw=>raw.human_form_selection?sharedNode(raw):raw);
  for(const path of result.scene?.compact?.claim_paths??[]){
    if(/^\/human_form_selection\/roles\/(caption|statement|hover)\/packet$/.test(path.reading.wording_pointer??'')){
      path.reading.mode='claim-with-shared-form-context-v2';
      path.reading.wording_pointer=path.reading.wording_pointer.slice(0,-'/packet'.length);
    }
  }
  return result;
}
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));
const pathOf=packet=>packet.scene.compact.claim_paths[0];
const claimOf=packet=>packet.nodes.find(raw=>raw.id===pathOf(packet).claim_node_id);

test('v1 and v2 expose identical complete roles, labels, hover, identity and unknown context without mutating wire',()=>{
  const inline=formNode();
  for(const selected of Object.values(inline.human_form_selection.roles)){
    selected.packet.context[0].value={negative:true,disputed:true,alternatives:['one','two'],unknown:{zero:0,false:false,null:null,empty:'',array:[],object:{}}};
    selected.packet.admission={limits:['not publication permission','keep qualifier','not publication permission'],unknown:null};
  }
  const shared=sharedNode(inline),before=structuredClone(shared);
  expect(validateHumanForms(shared)).toEqual(validateHumanForms(inline));
  expect(formView(shared)).toEqual(formView(inline));
  expect(formView(shared).roles.find(role=>role.role==='hover').packet.context[0].value.disputed).toBe(true);
  expect(formLanguages(shared)).toEqual(formLanguages(inline));
  expect(formIdentity(shared)).toEqual(formIdentity(inline));
  expect(displayTitle(shared)).toEqual(displayTitle(inline));
  expect(nodeLabels(shared)).toEqual(nodeLabels(inline));
  expect(nodePreview(formLens([shared]),shared)).toEqual(nodePreview(formLens([inline]),inline));
  const decoded=validateHumanForms(shared);
  decoded.roles.name.packet.context[0].value.unknown.zero=7;
  expect(decoded.roles.hover.packet.context[0].value.unknown.zero).toBe(0);
  expect(shared).toEqual(before);
});

test('v2 wire below 16 KiB can reconstruct complete logical packets larger than the v1 wire limit',()=>{
  const inline=formNode();
  for(const selected of Object.values(inline.human_form_selection.roles))selected.packet.context[0].value.long='whole context; '.repeat(350);
  expect(()=>boundedFormCost(inline.human_form_selection,FORM_WIRE_BUDGET)).toThrow();
  const shared=sharedNode(inline);
  expect(boundedFormCost(shared.human_form_selection,FORM_WIRE_BUDGET)).toBeLessThanOrEqual(FORM_WIRE_BUDGET);
  expect(validateHumanForms(shared)).toEqual(inline.human_form_selection);
  const snapshot=readingSnapshot({packet:formLens([shared]),match:shared},'node');
  expect(snapshot.raw.human_form_selection).toEqual(shared.human_form_selection);
  expect(readingDocument(snapshot).humanForms).toEqual(inline.human_form_selection);
});

test('legacy v1 keeps its existing UTF-8 wire boundary, independently of v2 conservative accounting',()=>{
  const raw=claimOf(memberFormLens()),wire=raw.human_form_selection;
  expect(new TextEncoder().encode(JSON.stringify(wire)).length).toBeLessThanOrEqual(FORM_WIRE_BUDGET);
  expect(()=>boundedFormCost(wire,FORM_WIRE_BUDGET)).toThrow();
  expect(validateHumanForms(raw)).toBe(wire);
  wire.extra='x'.repeat(FORM_WIRE_BUDGET);
  expect(()=>validateHumanForms(raw)).toThrow(FormContractError);
});

test('empty ready v2 delta, unavailable and ambiguous roles keep their distinct meanings',()=>{
  const raw=formNode();
  for(const [role,selected]of Object.entries(raw.human_form_selection.roles))if(role!=='caption'){
    selected.state=role==='hover'?'ambiguous':'unavailable';
    selected.reason=role==='hover'?'multiple-forms':'no-ready-form';selected.packet=null;selected.form=null;
  }
  const shared=sharedNode(raw);
  expect(shared.human_form_selection.roles.caption.packet_delta).toEqual({});
  expect(shared.human_form_selection.roles.hover.packet_delta).toBeNull();
  expect(validateHumanForms(shared)).toEqual(validateHumanForms(raw));
});

test.each([
  ['unknown version',s=>s.schema_version='tos_human_form_selection_v3'],
  ['missing base',s=>delete s.packet_base],
  ['overlapping base leaf',s=>s.roles.name.packet_delta.context=structuredClone(s.packet_base.context)],
  ['shadowed exact form',s=>s.packet_base.form=structuredClone(s.roles.name.form)],
  ['missing exact form',s=>s.roles.name.form=null],
  ['boolean pool index',s=>{s.shared_limits=['restriction'];s.packet_base.admission={limit_refs:[true]};}],
  ['unused pool value',s=>s.shared_limits=['orphaned restriction']],
  ['duplicate pool values',s=>{s.shared_limits=['restriction','restriction'];s.packet_base.admission={limit_refs:[0,1]};}],
  ['inline limits',s=>s.packet_base.admission={limits:[]}],
  ['missing mandatory packet context',s=>delete s.packet_base.context],
  ['missing context value',s=>delete s.packet_base.context[0].value],
  ['nonready wording',s=>s.roles.statement.state='unavailable'],
  ['carrier revision mismatch',s=>s.content_revision='f'.repeat(64)],
  ['subject ID mismatch',s=>s.packet_base.subject.id='tos.fixture.wrong'],
  ['subject version mismatch',s=>s.packet_base.subject.version=2],
  ['subject digest mismatch',s=>s.packet_base.subject.digest='sha256:'+'f'.repeat(64)],
  ['wire overflow',s=>s.extra='x'.repeat(16384)],
  ['expanded packet overflow',s=>{s.shared_limits=['x'.repeat(5000)];s.packet_base.admission={limit_refs:Array(16).fill(0)};}],
  ['excessive structural depth',s=>{let nested=null;for(let n=0;n<65;n++)nested={child:nested};s.extra=nested;}],
  ['unsafe key',s=>Object.defineProperty(s.packet_base,'__proto__',{value:{polluted:true},enumerable:true})],
])('v2 corruption fails closed before wording: %s',(_label,mutate)=>{
  const raw=sharedNode(formNode());mutate(raw.human_form_selection);
  expect(()=>validateHumanForms(raw)).toThrow(FormContractError);
  expect(()=>formView(raw)).toThrow(FormContractError);
  expect(()=>readingSnapshot({packet:formLens([raw]),match:raw},'node')).toThrow(FormContractError);
  expect(nodePreview(formLens([raw]),raw).body).not.toContain('Текст формы');
});

test.each(['v1','v2'])('%s mandatory carrier pointers keep negative/disputed values and reject missing context atomically',version=>{
  let raw=formNode();
  raw.semantics.assertion_contexts=[{negative:true,review:'disputed',confidence:null,unknown:{zero:0,false:false,empty:''}}];
  raw.display_selection={schema_version:'tos_display_selection_v1',content_revision:content,fields:{},
    essential_context_pointers:['/semantics/assertion_contexts/0']};
  if(version==='v2')raw=sharedNode(raw);
  const snapshot=readingSnapshot({packet:formLens([raw]),match:raw},'node');
  expect(readingDocument(snapshot).essentialContext).toEqual(essentialContext(raw));
  expect(readingDocument(snapshot).essentialContext.items[0].value.negative).toBe(true);
  for(const mutate of [
    value=>delete value.semantics.assertion_contexts,
    value=>value.display_selection.essential_context_pointers=['/semantics/assertion_contexts/1'],
    value=>value.display_selection.essential_context_pointers=['/semantics/assertion_contexts/00'],
    value=>value.display_selection.content_revision='f'.repeat(64),
    value=>value.display_selection.essential_context_pointers={},
  ]){
    const damaged=structuredClone(raw);mutate(damaged);
    expect(()=>validateHumanForms(damaged)).toThrow(FormContractError);
    expect(()=>readingSnapshot({packet:formLens([damaged]),match:damaged},'node')).toThrow(FormContractError);
  }
});

test('v2 compact Claim role pointers decode full wording together with exact semantic, epistemic and relation context',()=>{
  for(const fixture of [compactFormLens,memberFormLens]){
    const inline=fixture();
    claimOf(inline).semantics.assertion_contexts=[{negative:true,scope:{unknown:null},alternatives:['a','b']}];
    claimOf(inline).epistemic={review_posture:'disputed',confidence:null};
    inline.relations[0].qualifiers={negative:true,unknown:false};
    const shared=sharedLens(inline),before=structuredClone(shared),path=pathOf(shared);
    const expected=resolveClaimReading(inline,pathOf(inline).reading),actual=resolveClaimReading(shared,path.reading);
    expect({...actual,reading:expected.reading}).toEqual(expected);
    const closure=claimPathClosure(shared,path),oldClosure=claimPathClosure(inline,pathOf(inline));
    expect(closure.nodeIds).toEqual(oldClosure.nodeIds);expect(closure.relationIds).toEqual(oldClosure.relationIds);
    expect(closure.node).toBe(claimOf(shared));
    expect(actual.wording).toHaveProperty('context');expect(actual.wording).not.toHaveProperty('packet_delta');
    expect(shared).toEqual(before);
    for(const mutate of [
      reading=>reading.mode='claim-with-mandatory-context',
      reading=>reading.wording_pointer+='/packet',
      reading=>reading.wording_pointer+='/packet_delta',
      reading=>reading.wording_pointer+='/display_text',
      reading=>reading.content_revision='f'.repeat(64),
      reading=>reading.context_pointers=['/semantics'],
      reading=>reading.relation_context_ids.push('missing-relation'),
      reading=>reading.standalone=true,
    ]){const reading=structuredClone(path.reading);mutate(reading);expect(()=>resolveClaimReading(shared,reading)).toThrow(FormContractError);}
    expect(()=>resolveClaimReading(inline,path.reading)).toThrow(FormContractError);
    const damaged=structuredClone(shared);delete claimOf(damaged).epistemic;
    expect(()=>resolveClaimReading(damaged,pathOf(damaged).reading)).toThrow(FormContractError);
  }
});

test('v2 unavailable roles still use the existing explicit display-field fallback and missing state',()=>{
  const inline=compactFormLens('original'),shared=sharedLens(inline);
  expect(pathOf(shared).reading.mode).toBe('claim-with-mandatory-context');
  expect(resolveClaimReading(shared,pathOf(shared).reading)).toEqual(resolveClaimReading(inline,pathOf(inline).reading));
  const reading={...pathOf(shared).reading,mode:'claim-with-shared-form-context-v2',wording_pointer:null,wording_state:'missing'};
  expect(resolveClaimReading(shared,reading).wording).toBeNull();
  expect(()=>resolveClaimReading(shared,{...reading,wording_state:'available'})).toThrow(FormContractError);
});

test('saved reading positions keep v1 identities across v2 wire migration without persisting wording',()=>{
  const inline=formNode(),shared=sharedNode(inline),key=readingKey('node',inline.id);
  const oldSnapshot=readingSnapshot({packet:formLens([inline]),match:inline},'node');
  const snapshot=readingSnapshot({packet:formLens([shared]),match:shared},'node');
  expect(readingPositionKey(key,snapshot,'ru')).toBe(readingPositionKey(key,oldSnapshot,'ru'));
  const saved=validateReading({v:1,activeKey:key,entries:[{kind:'node',id:inline.id,sourceRevision:revision,contentRevision:content,preferred:'ru',
    positions:[[readingPositionKey(key,snapshot,'ru'),{top:100,details:[],anchor:{key:'form:hover:context:0',offset:2}}]]}]});
  expect(validateReading(JSON.parse(JSON.stringify(saved)))).toEqual(saved);
  expect(JSON.stringify(saved)).not.toContain('Текст формы');expect(JSON.stringify(saved)).not.toContain('НЕ доказано');
  expect(snapshot.raw.human_form_selection.schema_version).toBe('tos_human_form_selection_v2');
});

test('current v2 compound reads and restored v1 selectors re-query the full closure, reject revision changes and clear damaged context',async()=>{
  const inline=memberFormLens(),scene=sharedLens(inline);let fresh=sharedLens(memberFormLens());
  const client=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>fresh})});
  const shelf=createReadingShelf({client}),raw=claimOf(scene),{key}=shelf.pin({raw,kind:'node',sourceRevision:revision,bookmark:{graph:{packet:scene}},preferred:'ru'});
  await tick();expect(shelf.entries[0].snapshot.claimReading.wording.context).toHaveLength(2);
  const reference=claimMaterialReference(inline,pathOf(inline));
  expect(shelf.entries[0].claimReference).toEqual(reference);
  shelf.restore([{kind:'node',id:raw.id,sourceRevision:revision,contentRevision:content,preferred:'ru',claimReference:reference}]);
  await tick();expect(shelf.entries[0].snapshot.raw.human_form_selection.schema_version).toBe('tos_human_form_selection_v2');
  fresh.source_revision='f'.repeat(64);
  await expect(client.readClaimMaterial(scene,pathOf(scene),undefined,{language:'ru'})).rejects.toBeInstanceOf(RevisionError);
  fresh=sharedLens(memberFormLens());delete claimOf(fresh).human_form_selection.packet_base.context;
  await shelf.refresh(key);expect(shelf.entries[0].snapshot).toBeNull();expect(shelf.entries[0].failure).toBe('contract');
});

test('late v2 language responses cannot restore obsolete wording over a newer choice',async()=>{
  let resolveOld,calls=0;const pending=new Promise(resolve=>resolveOld=resolve);
  const answer=raw=>({packet:formLens([raw]),match:raw});
  const client={readMaterial:async(_kind,_id,_signal,_revision,_content,{language})=>++calls===1?pending:answer(sharedNode(formNode('fixture:forms',language)))};
  const shelf=createReadingShelf({client}),raw=sharedNode(formNode()),{key}=shelf.pin({raw,kind:'node',sourceRevision:revision});
  await shelf.language(key,'en');resolveOld(answer(raw));await tick();
  expect(shelf.entries[0].snapshot.raw.human_form_selection.requested_language).toBe('en');
  expect(validateHumanForms(shelf.entries[0].snapshot.raw).roles.statement.packet.language).toBe('en');
});
