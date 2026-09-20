import {test,expect} from 'vitest';
import {projectionTypeIds,researchSearchRows} from './research-content.mjs';

const catalog={semantic_registries:{entity_types:{entries:[
  {type_id:'service',object_role:'projection'},
  {type_id:'service',object_role:'projection'},
  {type_id:'useful',object_role:'semantic'},
  {type_id:'conflict',object_role:'projection'},
  {type_id:'conflict',object_role:'semantic'},
  {type_id:'incomplete',object_role:'projection'},
  {type_id:'incomplete'},
  {object_role:'projection'},
  {type_id:42,object_role:'projection'},
]}}};
const node=(id,type_id)=>({id,type_id,payload:{id}});
const relation=(id,relation_type_id)=>({id,relation_type_id,payload:{id}});
const page=()=>({nodes:[node('service-node','service'),node('useful-node','useful'),node('unknown-node','unknown'),node('conflict-node','conflict'),node('incomplete-node','incomplete')],relations:[
  relation('hidden-project','tos.relation.projects'),relation('kept-derived','tos.relation.projection-pressure'),relation('kept-unknown','tos.relation.unknown'),relation('kept-missing',undefined),
]});

test('projectionTypeIds admits only unique, non-conflicting projection roles',()=>{
  expect([...projectionTypeIds(catalog)]).toEqual(['service']);
  expect(projectionTypeIds({semantic_registries:{entity_types:{entries:{}}}})).toEqual(new Set());
  expect(projectionTypeIds({semantic_registries:{entity_types:{entries:[{type_id:'x',object_role:'projection'},{type_id:'x',object_role:'projection'}]}}})).toEqual(new Set(['x']));
  expect(projectionTypeIds({semantic_registries:{entity_types:{entries:[{type_id:'x',object_role:'projection'},{type_id:'x',object_role:null}]}}})).toEqual(new Set());
});

test('research keeps useful, unknown and conflicting records and preserves packet order',()=>{
  const input=page(),before=structuredClone(input),result=researchSearchRows(input,catalog);
  expect(result.rows.map(row=>[row.kind,row.raw.id])).toEqual([
    ['node','useful-node'],['node','unknown-node'],['node','conflict-node'],['node','incomplete-node'],
    ['relation','kept-derived'],['relation','kept-unknown'],['relation','kept-missing'],
  ]);
  expect(result.hiddenCount).toBe(2);
  expect(result.rows[0].raw).toBe(input.nodes[1]);expect(result.rows.at(-1).raw).toBe(input.relations[3]);
  expect(input).toEqual(before);
});

test('all mode restores every raw record and does not rewrite the packet',()=>{
  const input=page(),before=structuredClone(input),result=researchSearchRows(input,catalog,{includeService:true});
  expect(result.rows.map(row=>row.raw.id)).toEqual([...input.nodes,...input.relations].map(row=>row.id));
  expect(result.rows.map(row=>row.kind)).toEqual([...input.nodes.map(()=> 'node'),...input.relations.map(()=> 'relation')]);
  expect(result.hiddenCount).toBe(0);expect(result.rows[0].raw).toBe(input.nodes[0]);expect(result.rows.at(-1).raw).toBe(input.relations.at(-1));
  expect(input).toEqual(before);
});

test('a page containing only service records has no research rows but reports hidden records',()=>{
  const input={nodes:[node('service-a','service'),node('service-b','service')],relations:[relation('project','tos.relation.projects')]};
  expect(researchSearchRows(input,catalog)).toEqual({rows:[],hiddenCount:3});
  expect(researchSearchRows(input,catalog,{includeService:true}).rows.map(row=>row.raw.id)).toEqual(['service-a','service-b','project']);
});

test('seeded page permutations preserve each source order and filtering boundary',()=>{
  let seed=0x6d2b79f5;const random=()=>((seed=Math.imul(seed,1664525)+1013904223)>>>0)/2**32;
  const shuffle=value=>{const copy=[...value];for(let index=copy.length-1;index>0;index--){const other=Math.floor(random()*(index+1));[copy[index],copy[other]]=[copy[other],copy[index]];}return copy;};
  const nodes=[node('s1','service'),node('u1','useful'),node('x1','unknown'),node('s2','service'),node('c1','conflict')];
  const relations=[relation('p1','tos.relation.projects'),relation('r1','tos.relation.projection-pressure'),relation('r2','tos.relation.other')];
  for(let run=0;run<100;run++){
    const input={nodes:shuffle(nodes),relations:shuffle(relations)},before=structuredClone(input),result=researchSearchRows(input,catalog);
    expect(result.rows.map(row=>row.raw.id)).toEqual([
      ...input.nodes.filter(raw=>raw.type_id!=='service').map(raw=>raw.id),
      ...input.relations.filter(raw=>raw.relation_type_id!=='tos.relation.projects').map(raw=>raw.id),
    ]);
    expect(result.hiddenCount).toBe(input.nodes.filter(raw=>raw.type_id==='service').length+input.relations.filter(raw=>raw.relation_type_id==='tos.relation.projects').length);
    expect(input).toEqual(before);
  }
});
