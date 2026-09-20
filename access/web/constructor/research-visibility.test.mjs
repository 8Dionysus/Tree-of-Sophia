import {test} from 'vitest';
import assert from 'node:assert/strict';
import {researchVisibility} from './research-visibility.mjs';

const catalog={semantic_registries:{entity_types:{entries:[
  {type_id:'service',object_role:'projection'},
  {type_id:'concept',object_role:'semantic'},
  {type_id:'candidate',object_role:'navigation'},
]}}};
function fixture(){
  const rawNodesById=new Map([
    ['s',{id:'s',type_id:'service'}],['c',{id:'c',type_id:'concept'}],
    ['n',{id:'n',type_id:'candidate'}],['u',{id:'u',type_id:'future'}],
    ['m',{id:'m',type_id:'service'}],['x',{id:'x',type_id:'concept'}],
  ]);
  const vertices=[['service',['s']],['concept',['c']],['candidate',['n']],['unknown',['u']],['mixed',['m','x']]]
    .map(([id,nodeIds])=>({id,nodeIds,representativeId:nodeIds[0]}));
  const edges=[{id:'cs',kind:'relation',rawId:'cs',fromId:'concept',toId:'service'},
    {id:'cu',kind:'relation',rawId:'cu',fromId:'concept',toId:'unknown'}];
  return {vertices,edges,verticesById:new Map(vertices.map(v=>[v.id,v])),rawNodesById,
    rawRelationsById:new Map([['cs',{id:'cs',from_id:'c',to_id:'s'}],['cu',{id:'cu',from_id:'c',to_id:'u'}]]),
    carrierToVertex:new Map(vertices.flatMap(v=>v.nodeIds.map(id=>[id,v.id]))),
    pathsById:new Map([['claim',{from_id:'concept',to_id:'service'}]])};
}
const ids=model=>model.vertices.map(v=>v.id).sort();
test('research hides declared service objects while retaining candidates, unknown types and mixed groups',()=>{
  const model=fixture(),view=researchVisibility(model,{catalog,selection:{kind:'node',id:'c'}});
  assert.deepEqual(ids(view),['candidate','concept','mixed','unknown']);
  assert.deepEqual(view.edges.map(e=>e.id),['cu']);
  assert.deepEqual(view.visibility,{hiddenObjects:1,hiddenRelations:1});
  for(const key of ['rawNodesById','rawRelationsById','carrierToVertex','pathsById'])assert.equal(view[key],model[key]);
  assert.equal(model.vertices.length,5);assert.equal(model.edges.length,2);
  assert.equal(view.verticesById.has('service'),false);
  assert.deepEqual(ids(researchVisibility(model,{catalog,mode:'grouped'})),ids(model));
  assert.deepEqual(ids(researchVisibility(model,{catalog,mode:'raw'})),ids(model));
});
test('direct node, relation and claim-path selections keep their displayed targets reachable',()=>{
  const model=fixture();
  for(const selection of [{kind:'node',id:'s'},{kind:'relation',id:'cs'},{kind:'claim-path',id:'claim',claimId:'folded'}]){
    const view=researchVisibility(model,{catalog,selection});
    assert.deepEqual(ids(view),ids(model));assert.equal(view.edges.length,2);
  }
});
test('missing or conflicting owner classification cannot hide a record',()=>{
  const model=fixture();assert.deepEqual(ids(researchVisibility(model)),ids(model));
  const conflicting=structuredClone(catalog);conflicting.semantic_registries.entity_types.entries.push({type_id:'service',object_role:'evidence'});
  assert.deepEqual(ids(researchVisibility(model,{catalog:conflicting})),ids(model));
});
test('randomized carrier and registry ordering preserves visible membership and selected endpoints',()=>{
  let seed=914071;const random=()=>((seed=(Math.imul(seed,1664525)+1013904223)>>>0)/2**32);
  const shuffle=rows=>{for(let i=rows.length-1;i>0;i--){const j=Math.floor(random()*(i+1));[rows[i],rows[j]]=[rows[j],rows[i]];}return rows;};
  for(let run=0;run<100;run++){
    const model=fixture(),registry=structuredClone(catalog);
    shuffle(model.vertices);shuffle(model.edges);for(const vertex of model.vertices)shuffle(vertex.nodeIds);
    shuffle(registry.semantic_registries.entity_types.entries);
    assert.deepEqual(ids(researchVisibility(model,{catalog:registry})),['candidate','concept','mixed','unknown']);
    const selection={kind:'relation',id:random()<.5?'cs':'cu'},view=researchVisibility(model,{catalog:registry,selection});
    const relation=model.rawRelationsById.get(selection.id);
    for(const id of [relation.from_id,relation.to_id])assert.ok(view.verticesById.has(model.carrierToVertex.get(id)));
  }
});
