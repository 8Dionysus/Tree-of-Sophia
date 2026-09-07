import {test} from 'vitest';
import assert from 'node:assert/strict';
import {lensContext} from '../../fixtures/lens-scenarios.mjs';
import {conditionCatalog,compileConditions,validateConditions} from './lens-conditions.mjs';
import {compileDraft,initialDraft,encodeDraft,decodeDraft} from './lens-model.mjs';
import {inclusionDescription} from './scene-feedback.mjs';
const condition=(id,op,value)=>({selector:'property_id',id:'tos.property.fixture-'+id,op,value});

test('semantic selectors require both advertised capability and executable schema, never a guessed field binding',()=>{
  const context=lensContext(),entries=conditionCatalog(context,'nodes');
  assert.equal(entries.filter(e=>e.selector==='property_id').length,4);
  assert.equal(conditionCatalog(context,'relations').some(e=>e.selector==='property_id'),false);
  assert.equal(conditionCatalog(lensContext({properties:false}),'nodes').some(e=>e.selector==='property_id'),false);
  delete context.schema.$defs.nodeFilter.properties.property_id;
  assert.equal(conditionCatalog(context,'nodes').some(e=>e.selector==='property_id'),false);
  assert.throws(()=>compileConditions([condition('title','eq','А')],context,'nodes'));
});
test('operator intersection fails closed for unadvertised value contracts and disappeared definitions',()=>{
  const context=lensContext();delete context.catalog.capabilities.operator_value_contracts.contains;
  assert.throws(()=>compileConditions([condition('title','contains','А')],context,'nodes'));
  context.catalog.semantic_registries.properties=[];
  assert.throws(()=>compileConditions([condition('number','gte',2)],context,'nodes'));
  assert.throws(()=>compileConditions([{selector:'field',id:'attributes.guessed',op:'eq',value:'A'}],context,'nodes'));
});
test('property conditions preserve codepoints, scalar types, false, zero and list values',()=>{
  const context=lensContext(),rules=[condition('title','eq',' А\u0301 '),condition('number','gte',0),condition('flag','eq',false),condition('tags','contains',['А','Б']),condition('title','exists',false)];
  const compiled=compileConditions(rules,context,'nodes');
  assert.deepEqual(compiled,rules.map(({id,op,value})=>({property_id:id,op,value})));
  for(const rule of [condition('number','gte','0'),condition('flag','eq','false'),condition('title','eq',['A']),condition('title','exists',0),condition('tags','in',[])])assert.throws(()=>compileConditions([rule],context,'nodes'));
});
test('conditions retain root/relationship scope and are never applied to the explicit center',()=>{
  const context=lensContext(),draft=initialDraft({nodes:[{id:'opaque:1'}],focus:{node_id:'opaque:1'}},context);
  draft.conditions.nodes=[condition('number','gte',3)];draft.conditions.relations=[{selector:'field',id:'display.label.default',op:'contains',value:'проверка'}];
  assert.deepEqual(compileDraft(draft,context).node_query.filters,[{property_id:'tos.property.fixture-number',op:'gte',value:3}]);
  const focused=compileDraft({...draft,scope:'focus'},context);assert.deepEqual(focused.node_query.filters,[]);assert.equal(focused.node_query.enabled,false);
  assert.equal(focused.relation_query.filters[0].field,'display.label.default');
  assert.deepEqual(compileDraft({...draft,relations:false},context).relation_query.filters,[]);
});
test('v2 definitions carry conditions through links; v1 migrates without silent loss',()=>{
  const draft=initialDraft(null,lensContext());draft.conditions.nodes=[condition('tags','in',['α','β'])];
  assert.deepEqual(decodeDraft(encodeDraft(draft)),draft);
  const legacy={...draft,v:1};delete legacy.conditions;
  assert.deepEqual(decodeDraft(JSON.stringify(legacy)).conditions,{nodes:[],relations:[]});
  assert.throws(()=>decodeDraft(JSON.stringify({...draft,v:1})));
  const oversized={...draft,conditions:{nodes:Array.from({length:8},()=>condition('title','eq','Ж'.repeat(1024))),relations:[]}};
  assert.throws(()=>encodeDraft(oversized));
  for(const value of [{nodes:Array(13).fill(condition('flag','eq',true)),relations:[]},{nodes:[condition('number','gte',Infinity)],relations:[]},{nodes:[],relations:[condition('flag','eq',true)]}])assert.throws(()=>validateConditions(value));
});
test('inclusion explanation names only the delivered traversal witness, without treating context as a match',()=>{
  const packet={inclusion:{authority:'query-execution-not-semantic-proof',nodes:{b:{kind:'traversal',via_node_id:'a',via_relation_id:'r',depth:1}}},nodes:[{id:'a',display:{title:{ru:'А'}}}],relations:[{id:'r',display:{label:{ru:'Проверочная связь'}}}]};
  assert.match(inclusionDescription(packet,'b'),/условия исходных узлов не обязаны выполняться/);
  assert.match(inclusionDescription(packet,'b'),/«Проверочная связь» с «А»/);
  assert.equal(inclusionDescription(packet,'missing'),'');
  packet.inclusion.authority='other';assert.equal(inclusionDescription(packet,'b'),'');
});
