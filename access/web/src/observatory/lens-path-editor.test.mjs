import {test,expect} from 'vitest';
import assert from 'node:assert/strict';
import {lensContext} from '../../fixtures/lens-scenarios.mjs';
import {ContractError} from './knowledge-client.mjs';
import {compileDraft,decodeDraft,encodeDraft,initialDraft} from './lens-model.mjs';
import {compilePathQuery,draftPathsFromSpec,pathLimits,validatePathDraft} from './lens-path-editor.mjs';

const context=lensContext();
const query=(conditions=[],extra={})=>({enabled:true,match:'all',conditions,...extra});
const step=(patch={})=>({direction:'outgoing',kinds:[],predicates:[],nodeQuery:query(),relationQuery:query(),...patch});
const path=(patch={})=>({pathId:'author',quantifier:'exists',steps:[step()],...patch});
const condition=(id,op,value)=>({selector:'property_id',id:'tos.property.fixture-'+id,op,value});

test('path draft compiles to the exact schema shape and keeps query intent in links',()=>{
  const draft=initialDraft(null,context);
  draft.paths=[path({quantifier:'not_exists',steps:[step({direction:'incoming',kinds:['fixture-material'],predicates:['fixture-related'],nodeQuery:query([condition('title','eq','А')],{match:'any'}),relationQuery:query([], {enabled:false})})]})];
  const spec=compileDraft(draft,context);
  expect(spec.path_query).toEqual([{path_id:'author',quantifier:'not_exists',steps:[{direction:'incoming',
    node_query:{enabled:true,match:'any',filters:[{field:'kind_id',op:'in',value:['fixture-material']},{property_id:'tos.property.fixture-title',op:'eq',value:'А'}]},
    relation_query:{enabled:false,match:'all',filters:[{field:'predicate_id',op:'in',value:['fixture-related']}]}}]}]);
  expect(decodeDraft(encodeDraft(draft))).toEqual(draft);
  expect(spec).not.toHaveProperty('cursor');expect(spec).not.toHaveProperty('exploration');
});

test('path editor honors the live schema bounds',()=>{
  const limited=structuredClone(context);limited.schema.properties.path_query.maxItems=2;
  limited.schema.$defs.pathCondition.properties.steps.maxItems=3;
  limited.schema.$defs.pathCondition.properties.quantifier.enum=['exists'];
  limited.catalog.capabilities.path_query={conditions:2,steps_per_condition:3,quantifiers:['exists'],combination:'all',scope:'node-selector-roots-and-selected-sources'};
  expect(pathLimits(limited)).toMatchObject({conditions:2,steps:3,quantifiers:['exists']});
  const wrongScope=structuredClone(context);wrongScope.catalog.capabilities.path_query={conditions:4,steps_per_condition:4,quantifiers:['exists','not_exists'],combination:'all',scope:'global'};
  assert.throws(()=>pathLimits(wrongScope),ContractError);
  assert.throws(()=>compilePathQuery([path({pathId:'a'}),path({pathId:'b'}),path({pathId:'c'})],limited),ContractError);
  assert.throws(()=>compilePathQuery([path({steps:[step(),step(),step(),step(),step()]})],context),ContractError);
  assert.throws(()=>compilePathQuery([path({pathId:'bad id'})],context),ContractError);
  assert.throws(()=>compilePathQuery([path({pathId:'bad\n'})],context),ContractError);
  assert.throws(()=>compilePathQuery([path({pathId:'a'}),path({pathId:'a'})],context),ContractError);
  const noAny=structuredClone(context);noAny.schema.$defs.nodeQuery.properties.match.enum=['all'];
  assert.throws(()=>compilePathQuery([path({steps:[step({nodeQuery:query([],{match:'any'})})]})],noAny),ContractError);
});

test('invalid nested query state is retained as an explicit failure',()=>{
  const invalid=path({steps:[step({direction:'sideways'})]});
  assert.throws(()=>validatePathDraft([invalid]),ContractError);
  assert.throws(()=>compilePathQuery([path({steps:[step({nodeQuery:{enabled:true,match:'all',conditions:[condition('title','invented','x')]}})]})],context),ContractError);
  assert.throws(()=>compilePathQuery([path({steps:[step({relationQuery:{enabled:true,match:'all',conditions:[{selector:'property_id',id:'tos.property.fixture-title',op:'eq',value:'x'}]}})]})],context),ContractError);
});

test('step filters follow the schema maximum when no type selector consumes a slot',()=>{
  const conditions=Array.from({length:32},()=>condition('title','eq','x'));
  expect(compilePathQuery([path({steps:[step({nodeQuery:query(conditions)})]})],context)[0].steps[0].node_query.filters).toHaveLength(32);
  assert.throws(()=>compilePathQuery([path({steps:[step({kinds:['fixture-material'],nodeQuery:query(conditions)})]})],context),ContractError);
});

test('wire path import round-trips scoped absence and preserves non-type filters',()=>{
  const wire=[{path_id:'typed',quantifier:'not_exists',steps:[{direction:'either',node_query:{enabled:false,match:'any',filters:[
    {field:'epistemic.review_posture',op:'prefix',value:'fixture-'},
  ]},relation_query:{filters:[{field:'predicate_id',op:'in',value:['fixture-related']},{field:'display.label.default',op:'contains',value:'связь'}]}}]}];
  const draft=draftPathsFromSpec(wire);
  expect(draft).toEqual([{pathId:'typed',quantifier:'not_exists',steps:[{direction:'either',kinds:[],predicates:['fixture-related'],
    nodeQuery:{enabled:false,match:'any',conditions:[{selector:'field',id:'epistemic.review_posture',op:'prefix',value:'fixture-'}]},
    relationQuery:{enabled:true,match:'all',conditions:[{selector:'field',id:'display.label.default',op:'contains',value:'связь'}]}}]}]);
  expect(compilePathQuery(draft,context)).toEqual([{path_id:'typed',quantifier:'not_exists',steps:[{direction:'either',
    node_query:{enabled:false,match:'any',filters:[{field:'epistemic.review_posture',op:'prefix',value:'fixture-'}]},
    relation_query:{enabled:true,match:'all',filters:[{field:'predicate_id',op:'in',value:['fixture-related']},{field:'display.label.default',op:'contains',value:'связь'}]}}]}]);
});

test('wire import does not merge repeated type filters with different boolean meaning',()=>{
  const wire=[{path_id:'repeated',steps:[{node_query:{match:'all',filters:[
    {field:'kind_id',op:'in',value:['fixture-material']},{field:'kind_id',op:'in',value:['another-kind']},
  ]}}]}];
  const draft=draftPathsFromSpec(wire);
  expect(draft[0].steps[0].kinds).toEqual(['fixture-material']);
  expect(draft[0].steps[0].nodeQuery.conditions).toEqual([{selector:'field',id:'kind_id',op:'in',value:['another-kind']}]);
});

test('no path keeps the former query semantics',()=>{
  const draft=initialDraft(null,context),withEmpty=compileDraft(draft,context),without=structuredClone(draft);delete without.paths;
  expect(withEmpty.path_query).toEqual([]);expect(compileDraft(without,context).path_query).toEqual([]);
  expect(withEmpty.seed).toEqual(compileDraft(without,context).seed);
  expect(withEmpty.node_query).toEqual(compileDraft(without,context).node_query);
  expect(withEmpty.relation_query).toEqual(compileDraft(without,context).relation_query);
  const oldSchema=structuredClone(context);delete oldSchema.schema.properties.path_query;delete oldSchema.schema.$defs.pathCondition;
  expect(compileDraft(draft,oldSchema).path_query).toBeUndefined();
});
