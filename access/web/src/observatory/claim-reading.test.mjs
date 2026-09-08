import {test,expect} from 'vitest';
import {KnowledgeClient,RevisionError,RequestSlots} from './knowledge-client.mjs';
import * as inspector from './knowledge-ui.mjs';
import {compactFormLens,memberFormLens,formNode,formLens} from '../../fixtures/human-form-data.mjs';

const path=packet=>packet.scene.compact.claim_paths[0];
const claim=packet=>packet.nodes.find(n=>n.id==='fixture:claim');
function clientFor(answer){
  const sent=[];return {sent,client:new KnowledgeClient({fetcher:async(url,options)=>{
    const spec=JSON.parse(options.body);sent.push({url,spec});return {ok:true,json:async()=>typeof answer==='function'?answer(spec):answer};
  }})};
}
test('the inspector follows scene.compact and reads wording plus all context from one fresh language packet',async()=>{
  const scene=compactFormLens('ru'),fresh=compactFormLens('en'),before=structuredClone(scene),{client,sent}=clientFor(fresh);
  const value=await inspector.readInspectorMaterial({client,scene,kind:'node',raw:claim(scene),language:'en'});
  expect(sent).toHaveLength(1);const spec=sent[0].spec;
  expect(sent[0].url).toBe('/api/knowledge/lenses/compile');expect(spec.language).toBe('en');expect(spec.detail).toBe('full');
  expect(spec.traversal.depth).toBe(0);expect(spec.seed).toEqual({});
  expect(spec.node_query.filters[0].value.toSorted()).toEqual(fresh.nodes.map(n=>n.id).toSorted());
  expect(spec.relation_query.filters[0].value.toSorted()).toEqual(fresh.relations.map(r=>r.id).toSorted());
  expect(value.packet).toBe(fresh);expect(value.claimReading.wording).toEqual(claim(fresh).human_form_selection.roles.caption.packet);
  expect(value.claimReading.wording.language).toBe('en');expect(value.claimReading.wording.form.id).not.toBe(claim(scene).human_form_selection.roles.caption.form.id);
  expect(value.claimReading.semantics).toEqual(claim(fresh).semantics);expect(value.claimReading.relations).toEqual(fresh.relations);
  expect(scene).toEqual(before);
});
test('a card without a compact path retains the ordinary one-node material request',async()=>{
  const raw=formNode(),packet=formLens([raw]),{client,sent}=clientFor(packet);
  const value=await inspector.readInspectorMaterial({client,scene:packet,kind:'node',raw,language:'ru'});
  expect(value.claimReading).toBeUndefined();expect(sent[0].spec.lens_id).toBe('sophia-observatory-material');
  expect(sent[0].spec.limits).toEqual({nodes:1,relations:0,groups:1});
});
test.each([
  ['missing context relation',p=>p.relations.pop()],
  ['extra endpoint',p=>p.nodes.push(formNode('extra'))],
  ['missing endpoint',p=>p.nodes.pop()],
  ['missing returned path',p=>p.scene.compact.claim_paths=[]],
  ['wrong path node',p=>path(p).node_ids[2]='fixture:evidence'],
  ['unlisted relation context',p=>path(p).reading.relation_context_ids.pop()],
  ['returned old language',p=>{const raw=claim(p);raw.human_form_selection.requested_language='ru';}],
])('compact material rejects %s without a partial reading',async(_case,mutate)=>{
  const scene=compactFormLens(),fresh=compactFormLens('en');mutate(fresh);const {client}=clientFor(fresh);
  await expect(inspector.readInspectorMaterial({client,scene,kind:'node',raw:claim(scene),language:'en'})).rejects.toThrow();
});
test('a changed source or context record revision invalidates the compact material',async()=>{
  for(const mutate of [p=>p.source_revision='f'.repeat(64),p=>p.nodes[0].content_revision='f'.repeat(64),p=>p.relations[0].content_revision='f'.repeat(64)]){
    const scene=compactFormLens(),fresh=compactFormLens('en');mutate(fresh);const {client}=clientFor(fresh);
    await expect(client.readClaimMaterial(scene,path(scene),undefined,{language:'en'})).rejects.toBeInstanceOf(RevisionError);
  }
});
test('the declared context must fit the existing UI budget before any request',async()=>{
  const scene=compactFormLens();scene.nodes.push(...Array.from({length:37},(_,i)=>formNode('extra:'+i)));const {client,sent}=clientFor(scene);
  await expect(client.readClaimMaterial(scene,path(scene))).rejects.toThrow(/бюджет|предел/);expect(sent).toHaveLength(0);
});
test('canonical fallback and unavailable original never reselect a more convenient role or resurrect old wording',async()=>{
  const scene=compactFormLens('ru'),fallback=compactFormLens('ru');claim(fallback).human_form_selection.requested_language='es';
  for(const role of Object.values(claim(fallback).human_form_selection.roles))role.reason='fallback';
  const first=clientFor(fallback);const delivered=await inspector.readInspectorMaterial({client:first.client,scene,kind:'node',raw:claim(scene),language:'es'});
  expect(delivered.claimReading.wording.language).toBe('ru');expect(delivered.claimReading.reading.wording_pointer).toBe(path(fallback).reading.wording_pointer);
  const original=compactFormLens('original'),second=clientFor(original);
  const absent=await inspector.readInspectorMaterial({client:second.client,scene,kind:'node',raw:claim(scene),language:'original'});
  expect(absent.claimReading.wording).toEqual(claim(original).display_selection.fields.title);
  expect(absent.claimReading.wording.display_text).toBeUndefined();
});
test('a delayed compact language read cannot replace the current inspector packet',async()=>{
  const scene=compactFormLens('ru');let releaseOld;
  const pending=new Promise(resolve=>releaseOld=resolve),{client}=clientFor(spec=>spec.language==='ru'?pending:compactFormLens('en'));
  const slots=new RequestSlots(),read=language=>slots.run('inspect',signal=>inspector.readInspectorMaterial({client,scene,kind:'node',raw:claim(scene),language,signal}));
  const old=read('ru'),current=await read('en');releaseOld(compactFormLens('ru'));
  expect(current.current).toBe(true);expect(current.value.claimReading.wording.language).toBe('en');expect((await old).current).toBe(false);
});

test('the inspector delivers three exact value members with the complete current language form and context',async()=>{
  const scene=memberFormLens('ru'),fresh=memberFormLens('en'),before=structuredClone(scene),{client,sent}=clientFor(fresh);
  const value=await inspector.readInspectorMaterial({client,scene,kind:'node',raw:claim(scene),language:'en'});
  expect(sent[0].spec.limits).toEqual({nodes:7,relations:6,groups:7});
  expect(sent[0].spec.node_query.filters[0].value.toSorted()).toEqual(fresh.nodes.map(node=>node.id).toSorted());
  expect(sent[0].spec.relation_query.filters[0].value.toSorted()).toEqual(fresh.relations.map(relation=>relation.id).toSorted());
  expect(value.claimReading.semantics.claim.value_member_node_ids).toEqual(['fixture:text-unit:1','fixture:text-unit:2','fixture:text-unit:3']);
  expect(value.claimReading.relations.filter(relation=>relation.relation_type_id==='tos.relation.claim-value-member')).toHaveLength(3);
  expect(value.claimReading.wording).toEqual(claim(fresh).human_form_selection.roles.caption.packet);
  expect(value.claimReading.wording.language).toBe('en');expect(value.claimReading.wording.context[1].value.sign_judgment).toBeNull();
  expect(value.claimReading.wording.context[1].value.accepted_membership).toBe(false);expect(scene).toEqual(before);
});
test.each([
  ['missing third edge',p=>p.relations.pop()],
  ['third edge omitted from the whole declared path',p=>{p.relations.pop();path(p).detail_relation_ids.pop();path(p).reading.relation_context_ids.pop();}],
  ['member edge hidden outside the path context',p=>{claim(p).semantics.claim.value_member_node_ids.pop();path(p).detail_relation_ids.pop();path(p).reading.relation_context_ids.pop();}],
  ['undeclared member edges hidden outside path details',p=>{delete claim(p).semantics.claim.value_member_node_ids;path(p).detail_relation_ids.splice(1);path(p).reading.relation_context_ids.splice(3);}],
  ['missing third node',p=>p.nodes.pop()],
  ['duplicate member target',p=>p.relations.at(-1).to_id='fixture:text-unit:2'],
  ['wrong member target',p=>p.relations.at(-1).to_id='fixture:evidence'],
  ['wrong member origin',p=>p.relations.at(-1).from_id='fixture:subject'],
  ['duplicate declared member',p=>claim(p).semantics.claim.value_member_node_ids[2]='fixture:text-unit:2'],
  ['undeclared member edges',p=>delete claim(p).semantics.claim.value_member_node_ids],
  ['null member declaration',p=>claim(p).semantics.claim.value_member_node_ids=null],
  ['empty member declaration',p=>claim(p).semantics.claim.value_member_node_ids=[]],
  ['non-array member declaration',p=>claim(p).semantics.claim.value_member_node_ids='fixture:text-unit:1'],
  ['non-string declared member',p=>claim(p).semantics.claim.value_member_node_ids[2]=3],
])('member context rejects %s before fetching or returning a partial reading',async(_case,mutate)=>{
  const broken=memberFormLens();mutate(broken);const beforeRequest=clientFor(memberFormLens('en'));
  await expect(beforeRequest.client.readClaimMaterial(broken,path(broken),undefined,{language:'en'})).rejects.toThrow();
  expect(beforeRequest.sent).toHaveLength(0);
  const scene=memberFormLens(),response=memberFormLens('en');mutate(response);const afterRequest=clientFor(response);
  await expect(inspector.readInspectorMaterial({client:afterRequest.client,scene,kind:'node',raw:claim(scene),language:'en'})).rejects.toThrow();
});
test('a present empty member declaration is invalid even without member edges; an absent one preserves the two-leg path',async()=>{
  const ordinary=compactFormLens(),valid=clientFor(compactFormLens('en'));
  const result=await inspector.readInspectorMaterial({client:valid.client,scene:ordinary,kind:'node',raw:claim(ordinary),language:'en'});
  expect(result.claimReading.relations).toHaveLength(3);expect(valid.sent[0].spec.limits.nodes).toBe(4);
  claim(ordinary).semantics.claim.value_member_node_ids=[];const invalid=clientFor(compactFormLens('en'));
  await expect(invalid.client.readClaimMaterial(ordinary,path(ordinary),undefined,{language:'en'})).rejects.toThrow();
  expect(invalid.sent).toHaveLength(0);
});
test('member context uses only the normalized declaration and structural relation type',async()=>{
  const scene=memberFormLens(),fresh=memberFormLens('en');
  for(const packet of [scene,fresh])for(const node of packet.nodes.filter(node=>node.id.startsWith('fixture:text-unit:'))){
    delete node.type_id;node.kind_id='unknown';node.attributes={object:{members:['misleading']},kind:'Sign'};
  }
  const {client}=clientFor(fresh),value=await inspector.readInspectorMaterial({client,scene,kind:'node',raw:claim(scene),language:'en'});
  expect(value.claimReading.semantics.claim.value_member_node_ids).toHaveLength(3);
});
test('member node and member edge revisions remain pinned to the displayed scene',async()=>{
  for(const kind of ['nodes','relations']){
    const scene=memberFormLens(),fresh=memberFormLens('en');fresh[kind].at(-1).content_revision='e'.repeat(64);const {client}=clientFor(fresh);
    await expect(client.readClaimMaterial(scene,path(scene),undefined,{language:'en'})).rejects.toBeInstanceOf(RevisionError);
  }
});
