import {test,expect} from 'vitest';
import {buildSceneModel,SceneContractError,SCENE_LIMITS} from './scene-model.mjs';

const revision='a'.repeat(64);
const boundary={is_source:false,is_canon:false,writes_to_tree:false};

function node(id,entityId=null,{claim=false}={}){
  return {id,entity_id:entityId,type_id:claim?'tos.entity.claim':'tos.entity.fixture',content_revision:revision,
    semantics:claim?{claim:{subject_node_id:'subject',object_node_id:'object',relation_type_id:'tos.relation.fixture',predicate_mapping_status:'mapped'}}:{},
    epistemic:{review_posture:'unreviewed'},display:{title:{ru:id}},source_refs:['fixture']};
}
function relation(id,from_id,to_id,relation_type_id){
  return {id,from_id,to_id,relation_type_id,content_revision:revision,display:{label:{ru:id}},source_refs:['fixture']};
}

function pathPacket({focus='subject',focusRelation=null,includeCompact=true}={}){
  const subject=node('subject','tos.entity.subject'),claim=node('claim','tos.entity.claim',{claim:true}),objectNode=node('object','tos.entity.object'),support=node('support',null);
  const relations=[
    relation('leg-subject','claim','subject','tos.relation.has-subject'),
    relation('leg-object','claim','object','tos.relation.has-object'),
    relation('support','claim','support','tos.relation.claim-supported-by'),
  ];
  const packet={schema:'tos_lens_result_v1',nodes:[subject,claim,objectNode,support],relations,focus:focus===null?null:{node_id:focus,...(focusRelation?{relation_id:focusRelation}:{})}};
  const vertex=id=>`v:${id}`;
  const scene={schema_version:'tos_knowledge_scene_v1',vertices:packet.nodes.map(raw=>({id:vertex(raw.id),entity_id:raw.entity_id,node_ids:[raw.id],representative_node_id:raw.id})),
    arcs:relations.map(raw=>({relation_id:raw.id,from_id:vertex(raw.from_id),to_id:vertex(raw.to_id)})),collapsed_relation_ids:[],focus_vertex_id:focus===null?null:vertex(focus),
    scope:'returned-packet-only',identity_rule:'declared-tos-entity-id',authority:'presentation-mapping-not-semantic-admission'};
  const claimPath={id:'path:claim',from_id:vertex('subject'),to_id:vertex('object'),claim_node_id:'claim',relation_type_id:'tos.relation.fixture',node_ids:['subject','claim','object'],
    relation_ids:['leg-subject','leg-object'],detail_relation_ids:['support'],reading:{mode:'claim-with-mandatory-context',node_id:'claim',content_revision:revision,
      wording_pointer:null,wording_state:'missing',context_pointers:['/semantics','/epistemic'],relation_context_ids:['leg-subject','leg-object','support'],standalone:false}};
  if(includeCompact)scene.compact={rule:'explicit-claim-paths-v1',authority:'presentation-only-no-new-assertion',vertex_ids:[vertex('subject'),vertex('object')],
    relation_ids:[],claim_paths:[claimPath],folded_vertex_ids:[vertex('claim'),vertex('support')],retained_claims:[]};
  packet.scene=scene;
  return {packet,claimPath,subject,claim,relations};
}

function expectSceneError(work){
  expect(work).toThrow(SceneContractError);
  expect(work).toThrow(/Сцена/);
}

test('compact, grouped, and raw projections keep presentation identities typed',()=>{
  const {packet,claimPath,claim,relations}=pathPacket();
  const compact=buildSceneModel(packet);
  expect(compact.mode).toBe('compact');
  expect(compact.declared).toBe(packet.scene);
  expect(compact.vertices.map(value=>value.id)).toEqual(['v:subject','v:object']);
  expect(compact.edges).toEqual([{id:claimPath.id,kind:'claim-path',fromId:'v:subject',toId:'v:object',path:claimPath}]);
  expect(compact.pathsById.get(claimPath.id)).toBe(claimPath);
  expect(compact.carrierToVertex.get('claim')).toBe('v:claim');
  expect(compact.verticesById.has('v:claim')).toBe(false);
  expect(compact.rawNodesById.get('claim')).toBe(claim);
  expect(compact.rawRelationsById.get(relations[0].id)).toBe(relations[0]);

  const grouped=buildSceneModel(packet,{mode:'grouped'});
  expect(grouped.vertices).toHaveLength(4);expect(grouped.edges.map(edge=>edge.kind)).toEqual(['relation','relation','relation']);
  expect(grouped.pathsById.get(claimPath.id)).toBe(claimPath);
  const raw=buildSceneModel(packet,{mode:'raw'});
  expect(raw.vertices).toHaveLength(4);expect(raw.edges.map(edge=>edge.rawId)).toEqual(relations.map(value=>value.id));
  expect(raw.carrierToVertex.get('claim')).toBe('claim');
  expect(raw.pathsById.get(claimPath.id)).toBe(claimPath);
});

test('declared grouping requires explicit entity identity and keeps null carriers separate',()=>{
  const first=node('same-a','tos.entity.same'),second=node('same-b','tos.entity.same'),third=node('same-label',null);
  first.display.title=second.display.title=third.display.title={ru:'Одинаковая подпись'};
  const packet={nodes:[first,second,third],relations:[],focus:null,scene:{schema_version:'tos_knowledge_scene_v1',vertices:[
    {id:'same',entity_id:'tos.entity.same',node_ids:[first.id,second.id],representative_node_id:first.id},
    {id:'carrier',entity_id:null,node_ids:[third.id],representative_node_id:third.id},
  ],arcs:[],collapsed_relation_ids:[],focus_vertex_id:null,scope:'returned-packet-only',identity_rule:'declared-tos-entity-id',authority:'presentation-mapping-not-semantic-admission',
    compact:{rule:'explicit-claim-paths-v1',authority:'presentation-only-no-new-assertion',vertex_ids:['same','carrier'],relation_ids:[],claim_paths:[],folded_vertex_ids:[],retained_claims:[]}}};
  const model=buildSceneModel(packet);
  expect(model.vertices).toEqual([{id:'same',nodeIds:['same-a','same-b'],representativeId:'same-a'},{id:'carrier',nodeIds:['same-label'],representativeId:'same-label'}]);
  expect(model.carrierToVertex.get(first.id)).toBe('same');expect(model.carrierToVertex.get(second.id)).toBe('same');expect(model.carrierToVertex.get(third.id)).toBe('carrier');
});

test('raw fallback is available for legacy packets without a declared scene',()=>{
  const first=node('opaque/a'),second=node('opaque/b');const edge=relation('edge','opaque/a','opaque/b','tos.relation.fixture');
  const packet={nodes:[first,second],relations:[edge],focus:{node_id:first.id}};const model=buildSceneModel(packet,{mode:'compact'});
  expect(model.mode).toBe('raw');expect(model.declared).toBeNull();expect(model.vertices.map(value=>value.id)).toEqual(['opaque/a','opaque/b']);
  expect(model.edges).toEqual([{id:'edge',kind:'relation',fromId:'opaque/a',toId:'opaque/b',rawId:'edge'}]);
  expect(model.rawNodesById.get(first.id)).toBe(first);expect(model.rawRelationsById.get(edge.id)).toBe(edge);
});

test('focused intra-carrier projects relation remains an edge while ordinary projects may collapse',()=>{
  const first=node('first','tos.entity.same'),second=node('second','tos.entity.same'),third=node('third','tos.entity.other');
  const ordinary=relation('ordinary-project','first','second','tos.relation.projects'),focused=relation('focused-project','first','second','tos.relation.projects'),cross=relation('cross','first','third','tos.relation.fixture');
  const make=(focusRelation,collapsed)=>({nodes:[first,second,third],relations:[ordinary,focused,cross],focus:{node_id:'first',relation_id:focusRelation},scene:{schema_version:'tos_knowledge_scene_v1',vertices:[
    {id:'same',entity_id:'tos.entity.same',node_ids:['first','second'],representative_node_id:'first'},
    {id:'other',entity_id:'tos.entity.other',node_ids:['third'],representative_node_id:'third'},
  ],arcs:[...((collapsed?['focused-project','cross']:['ordinary-project','focused-project','cross']).map(id=>{const raw={ordinary:ordinary,focused:focused,cross}[id];return {relation_id:id,from_id:id==='cross'?'same':'same',to_id:id==='cross'?'other':'same'};}))],
    collapsed_relation_ids:collapsed?['ordinary-project']:[],focus_vertex_id:'same',scope:'returned-packet-only',identity_rule:'declared-tos-entity-id',authority:'presentation-mapping-not-semantic-admission',
    compact:{rule:'explicit-claim-paths-v1',authority:'presentation-only-no-new-assertion',vertex_ids:['same','other'],relation_ids:collapsed?['focused-project','cross']:['ordinary-project','focused-project','cross'],claim_paths:[],folded_vertex_ids:[],retained_claims:[]}}});
  const ordinaryModel=buildSceneModel(make(null,true),{mode:'grouped'});
  expect(ordinaryModel.edges.map(edge=>edge.rawId)).toEqual(['focused-project','cross']);
  const focusedModel=buildSceneModel(make('focused-project',false),{mode:'grouped'});
  expect(focusedModel.edges.map(edge=>edge.rawId)).toEqual(['ordinary-project','focused-project','cross']);
  expect(()=>buildSceneModel(make('ordinary-project',true))).toThrow(SceneContractError);
});

test.each([
  ['unpartitioned node',packet=>packet.scene.vertices[0].node_ids.pop()],
  ['duplicate node membership',packet=>packet.scene.vertices[1].node_ids.push('subject')],
  ['entity mismatch',packet=>packet.scene.vertices[0].entity_id='tos.entity.changed'],
  ['unknown arc relation',packet=>packet.scene.arcs[0].relation_id='missing'],
  ['wrong arc endpoint',packet=>packet.scene.arcs[0].from_id='v:object'],
  ['relation omitted from scene accounting',packet=>packet.scene.arcs.pop()],
  ['compact vertex overlap',packet=>packet.scene.compact.folded_vertex_ids.push('v:subject')],
  ['compact relation omitted',packet=>{packet.scene.compact.relation_ids=['support'];}],
  ['path endpoint mismatch',packet=>packet.scene.compact.claim_paths[0].to_id='v:support'],
  ['path reading mismatch',packet=>packet.scene.compact.claim_paths[0].reading.content_revision='b'.repeat(64)],
])('malformed declared scene fails closed: %s',(_name,mutate)=>{
  const {packet}=pathPacket();mutate(packet);expectSceneError(()=>buildSceneModel(packet,{mode:'raw'}));
});

test('raw records and declared scene stay unchanged while adapter exposes exact references',()=>{
  const {packet,claimPath}=pathPacket(),before=JSON.stringify(packet),model=buildSceneModel(packet);
  expect(JSON.stringify(packet)).toBe(before);
  expect(model.pathsById.get(claimPath.id)).toBe(claimPath);
  expect([...model.rawNodesById.values()]).toEqual(packet.nodes);
  expect([...model.rawRelationsById.values()]).toEqual(packet.relations);
});

test('invalid limits do not trigger unbounded graph work',()=>{
  const nodes=Array.from({length:SCENE_LIMITS.nodes+1},(_,index)=>node('n:'+index));
  expectSceneError(()=>buildSceneModel({nodes,relations:[]}));
  const small=pathPacket().packet;small.relations=Array.from({length:SCENE_LIMITS.relations+1},(_,index)=>relation('r:'+index,'subject','subject','tos.relation.fixture'));
  expectSceneError(()=>buildSceneModel(small));
});
