import {test,expect} from 'vitest';
import {KnowledgeClient,RevisionError,RequestSlots} from './knowledge-client.mjs';
import * as inspector from './knowledge-ui.mjs';
import {compactFormLens,formNode,formLens} from '../../fixtures/human-form-data.mjs';

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
