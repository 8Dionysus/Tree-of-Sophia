import {test,expect} from 'vitest';
import {knowledgeScene} from '../../../shared/knowledge-scene.ts';
import {encodeHumanFormSelection} from '../../../shared/human-form-selection-codec.ts';
import {compactFormLens} from '../../fixtures/human-form-data.mjs';
import {buildSceneModel,buildExplorationSceneModel,SceneContractError,EXPLORATION_SCENE_LIMITS} from './scene-model.mjs';
import {resolveClaimReading} from './human-forms.mjs';

const revision='a'.repeat(64);
const boundary={is_source:false,is_canon:false,writes_to_tree:false};
const node=id=>({id,entity_id:'tos.fixture.'+id,content_revision:revision,semantics:{},epistemic:{}});
const relation=(id,from_id,to_id,type='tos.relation.fixture')=>({id,from_id,to_id,relation_type_id:type,content_revision:revision});
function localView(nodes,relations=[],selection=null,originRow=nodes[0]){
  const relationOrigin=relations.includes(originRow);
  const endpoint=id=>{const raw=nodes.find(value=>value.id===id);return {node_id:raw.id,entity_id:raw.entity_id,content_revision:raw.content_revision};};
  const origin={kind:relationOrigin?'relation':'node',id:originRow.id,content_revision:originRow.content_revision,
    ...(relationOrigin?{endpoints:{from:endpoint(originRow.from_id),to:endpoint(originRow.to_id)}}:{})};
  const selected=selection??origin;
  const selectedRelation=selected.kind==='relation'?relations.find(raw=>raw.id===selected.id):null;
  const focus=selectedRelation?null:(selected.kind==='claim-path'?selected.claimId:selected.id);
  return {schema:'tos_browser_exploration_view_v1',source_revision:revision,snapshot_revision:'b'.repeat(64),
    execution_version:'tos-exploration-execution-v6',origin,selection,nodes,relations,
    scene:knowledgeScene(nodes,relations,focus,selectedRelation?.id??null),
    scope:'bounded-retained-exploration-carriers',writes_to_tree:false,authority_boundary:{...boundary}};
}
function claimView(version='v2',selection=null){
  const packet=compactFormLens(),claim=packet.nodes.find(raw=>raw.type_id==='tos.entity.claim');
  // The generic UI fixture shares the Claim's subject identity with an endpoint.
  // Give this independent endpoint its own explicit identity before composing.
  packet.nodes.find(raw=>raw.id==='fixture:subject').entity_id='tos.fixture.endpoint.subject';
  if(version==='v2')claim.human_form_selection=encodeHumanFormSelection(claim.human_form_selection);
  return localView(packet.nodes,packet.relations,selection);
}
const pathOf=view=>view.scene.compact.claim_paths[0];

test('separate local-view entry admits its fixed 200/600 boundary without widening LensResult',()=>{
  const nodes=Array.from({length:200},(_,i)=>node('n'+i));
  const relations=Array.from({length:600},(_,i)=>relation('r'+i,'n0','n'+(i%200)));
  const view=localView(nodes,relations);
  expect(EXPLORATION_SCENE_LIMITS).toEqual({nodes:200,relations:600,vertices:200,arcs:600,paths:200});
  expect(Object.isFrozen(EXPLORATION_SCENE_LIMITS)).toBe(true);
  for(const mode of ['raw','grouped','compact']){
    const model=buildExplorationSceneModel(view,{mode});
    expect(model.vertices).toHaveLength(200);expect(model.edges).toHaveLength(600);
    expect(()=>buildSceneModel(view,{mode,limits:EXPLORATION_SCENE_LIMITS})).toThrow(SceneContractError);
  }
  for(const oversized of [localView([...nodes,node('overflow')],relations),localView(nodes,[...relations,relation('overflow','n0','n1')])])
    expect(()=>buildExplorationSceneModel(oversized,{limits:{nodes:1000,relations:1000}})).toThrow(SceneContractError);
  expect(()=>buildSceneModel({nodes:nodes.slice(0,41),relations:[]},{limits:EXPLORATION_SCENE_LIMITS})).toThrow(SceneContractError);
  expect(()=>buildSceneModel({nodes:nodes.slice(0,2),relations:relations.slice(0,81).map(r=>({...r,to_id:'n1'}))},{limits:EXPLORATION_SCENE_LIMITS})).toThrow(SceneContractError);
  expect(()=>buildSceneModel(localView([node('small')]))).toThrow(SceneContractError);
});

test.each([
  ['schema',v=>v.schema='tos_lens_result_v1'],['scope',v=>v.scope='returned-packet-only'],
  ['write permission',v=>v.writes_to_tree=true],['authority',v=>v.authority_boundary.is_source=true],
  ['source revision',v=>v.source_revision+='a'],['snapshot revision',v=>delete v.snapshot_revision],
  ['execution',v=>v.execution_version='future'],['fake focus',v=>v.focus={node_id:'a'}],
  ['missing scene',v=>delete v.scene],['missing compact',v=>delete v.scene.compact],
  ['origin revision',v=>v.origin.content_revision='f'.repeat(64)],['origin kind',v=>v.origin.kind='claim-path'],
  ['origin foreign key',v=>v.origin.scene_id='tos-scene:entity:tos.fixture.a'],
  ['scene ID as selector',v=>v.selection={kind:'node',id:v.scene.vertices[0].id}],
  ['selection revision addition',v=>v.selection={kind:'node',id:'a',content_revision:revision}],
  ['selection focus mismatch',v=>v.selection={kind:'node',id:'b'}],
])('local-view envelope fails closed: %s',(_name,mutate)=>{
  const view=localView([node('a'),node('b')]);mutate(view);
  expect(()=>buildExplorationSceneModel(view)).toThrow(SceneContractError);
});

test('relation origin binds exact endpoints and selection keeps intra-entity projects visible',()=>{
  const a=node('a'),b={...node('b'),entity_id:a.entity_id},r=relation('project','a','b','tos.relation.projects');
  const view=localView([a,b],[r],null,r);
  expect(view.scene.collapsed_relation_ids).toEqual([]);
  expect(buildExplorationSceneModel(view).edges.map(edge=>edge.rawId)).toEqual(['project']);
  for(const mutate of [v=>v.origin.endpoints.from.node_id='b',v=>v.origin.endpoints.to.entity_id=null,
    v=>v.origin.endpoints.to.content_revision='f'.repeat(64),v=>delete v.origin.endpoints.from,
    v=>v.origin.endpoints.to.extra=true]){
    const damaged=structuredClone(view);mutate(damaged);
    expect(()=>buildExplorationSceneModel(damaged)).toThrow(SceneContractError);
  }
});

test('retained carriers preserve exact references and stable explicit identity across all projections',()=>{
  const a=node('a'),b={...node('b'),entity_id:a.entity_id},c=node('c');
  const first=localView([a,c]),second=localView([a,b,c],[relation('r','a','c')]);
  const identity=buildExplorationSceneModel(first).carrierToVertex.get('a'),before=JSON.stringify(second);
  for(const mode of ['raw','grouped','compact']){
    const model=buildExplorationSceneModel(second,{mode});
    expect(model.declared).toBe(second.scene);expect(model.rawNodesById.get('a')).toBe(a);
    expect(model.rawRelationsById.get('r')).toBe(second.relations[0]);
    expect(model.carrierToVertex.get('a')).toBe(mode==='raw'?'a':identity);
    expect(model.carrierToVertex.get('b')).toBe(mode==='raw'?'b':identity);
  }
  expect(JSON.stringify(second)).toBe(before);
});

test.each(['v1','v2'])('%s Claim paths keep complete reading context and exact raw references',version=>{
  const view=claimView(version),before=JSON.stringify(view),path=pathOf(view);
  expect(path).toBeDefined();
  const reading=resolveClaimReading(view,path.reading);
  expect(reading.wording.context[0].value.negation).toBe('НЕ доказано');
  expect(path.reading.wording_pointer).toBe('/human_form_selection/roles/caption'+(version==='v1'?'/packet':''));
  for(const mode of ['raw','grouped','compact']){
    const model=buildExplorationSceneModel(view,{mode});
    expect(model.pathsById.get(path.id)).toBe(path);
    expect(model.rawNodesById.get(path.claim_node_id)).toBe(view.nodes.find(raw=>raw.id===path.claim_node_id));
  }
  expect(JSON.stringify(view)).toBe(before);
});

test.each([
  ['old mode',v=>pathOf(v).reading.mode='claim-with-mandatory-context'],
  ['old packet pointer',v=>pathOf(v).reading.wording_pointer+='/packet'],
  ['standalone text pointer',v=>pathOf(v).reading.wording_pointer+='/display_text'],
  ['foreign revision',v=>pathOf(v).reading.content_revision='f'.repeat(64)],
  ['missing form context',v=>delete v.nodes.find(raw=>raw.type_id==='tos.entity.claim').human_form_selection.packet_base.context],
  ['missing relation context',v=>pathOf(v).reading.relation_context_ids.pop()],
])('v2 malformed path is never a readable scene: %s',(_name,mutate)=>{
  const view=claimView();mutate(view);
  expect(()=>buildExplorationSceneModel(view,{mode:'raw'})).toThrow(SceneContractError);
});

test.each(['fixture:claim-subject','fixture:claim-object','fixture:claim-evidence'])('selected raw Claim edge remains visible: %s',id=>{
  const view=claimView('v2',{kind:'relation',id}),model=buildExplorationSceneModel(view);
  expect(model.edges.some(edge=>edge.kind==='relation'&&edge.rawId===id)).toBe(true);
  expect(view.scene.compact.claim_paths).toEqual([]);
  expect(view.scene.compact.retained_claims).toContainEqual({node_id:'fixture:claim',reason:'focus-relation'});
  const malformed=claimView('v2',{kind:'node',id:'fixture:claim'});
  malformed.selection={kind:'relation',id};
  expect(()=>buildExplorationSceneModel(malformed)).toThrow(SceneContractError);
});

test('ordinary focused Claim and selected Claim path retain the vertex plus complete path',()=>{
  const selected={kind:'claim-path',id:'tos-scene:claim-path:fixture:claim',claimId:'fixture:claim'};
  for(const selection of [{kind:'node',id:selected.claimId},selected]){
    const view=claimView('v2',selection),model=buildExplorationSceneModel(view);
    expect(model.verticesById.has(model.carrierToVertex.get(selected.claimId))).toBe(true);
    expect(model.pathsById.has(selected.id)).toBe(true);
    expect(model.edges.some(edge=>edge.id===selected.id&&edge.kind==='claim-path')).toBe(true);
  }
  const mismatched=claimView('v2',selected);mismatched.selection={...selected,id:'missing-path'};
  expect(()=>buildExplorationSceneModel(mismatched)).toThrow(SceneContractError);
});
