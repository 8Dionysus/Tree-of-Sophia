import {test} from 'vitest';
import assert from 'node:assert/strict';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {lensDelta} from './lens-model.mjs';

const entity=(id,role,sources,kind)=>({type_id:id,object_role:role,source_mappings:sources.map(source_graph=>({source_graph,source_kind_id:kind}))});
const kind=(id,title,type,count=1)=>({kind_id:id,display:{ru:title},type_ids:[type],mapping_statuses:['mapped'],count});
const catalog={node_kinds:[kind('concept','Явление','meaning',5),kind('work','Альманах','work',3),kind('table','Таблица','projection',900),kind('future','Будущий тип','missing')],predicates:[],semantic_registries:{entity_types:{entries:[entity('meaning','semantic',['canon'],'concept'),entity('work','identity',['sources'],'work'),entity('projection','projection',['repository'],'table')]},relation_types:{entries:[]}}};

test('presentation follows registered roles and exact source mappings without classifying by names',()=>{
  const vocab=lensVocabulary(catalog,'kinds');
  assert.equal(vocab.find(i=>i.id==='table').group,'technical');
  assert.equal(vocab.find(i=>i.id==='future').group,'other');
  assert.equal(vocab.find(i=>i.id==='future').sourceKnown,false);
  const shown=vocabularyGroups(vocab,{sources:['canon']}).flatMap(g=>g.items.map(i=>i.id));
  assert.deepEqual(shown,['concept','future']);
  assert.equal(catalog.node_kinds.length,4);
});
test('source changes never silently drop chosen filters; missing registry entries remain removable',()=>{
  const groups=vocabularyGroups(lensVocabulary(catalog,'kinds'),{sources:['canon'],selected:['table','removed-type']});
  assert.equal(groups.find(g=>g.key==='unavailable').items[0].id,'table');
  assert.equal(groups.find(g=>g.key==='other').items.find(i=>i.id==='removed-type').selected,true);
  assert.equal(groups.flatMap(g=>g.items).filter(i=>i.id==='table').length,1);
});
test('all choices remain reachable, search crosses groups and sorting uses readable Russian labels',()=>{
  const items=[{id:'y',title:'Ясность',group:'meaning',sourceKnown:false,count:10},{id:'b',title:'Бытие',group:'meaning',sourceKnown:false,count:2},{id:'a',title:'Абсолют',group:'meaning',sourceKnown:false,count:5},...Array.from({length:90},(_,i)=>({id:'tech'+i,title:'Строка '+i,group:'technical',sourceKnown:false,count:i}))];
  const options={sources:['canon']};
  assert.deepEqual(vocabularyGroups(items,options)[0].items.map(i=>i.id),['a','b','y']);
  assert.deepEqual(vocabularyGroups(items,{...options,sort:'frequency'})[0].items.map(i=>i.id),['y','a','b']);
  assert.equal(vocabularyGroups(items,options).flatMap(g=>g.items).length,93);
  assert.equal(vocabularyGroups(items,{...options,query:'TECH89'})[0].items[0].id,'tech89');
  assert.equal(vocabularyGroups(items,{...options,selected:['y']})[0].items[0].id,'y');
});
test('relation groups retain authored meaning while derived structures stay separate',()=>{
  const c=structuredClone(catalog);c.predicates=[{predicate_id:'contains',display:{ru:'Содержит'},relation_type_ids:['derived'],mapping_statuses:['mapped']},{predicate_id:'author',display:{ru:'Автор'},relation_type_ids:['authored'],mapping_statuses:['mapped']}];
  c.semantic_registries.relation_types.entries=[{relation_type_id:'derived',assertion_mode:'derived-projection',source_mappings:[{source_graph:'repository',source_predicate_id:'contains',scope:'edge'}]},{relation_type_id:'authored',assertion_mode:'reified-claim',parent_relation_type_ids:['tos.relation.responsibility'],source_mappings:[{source_graph:'sources',source_predicate_id:'author',scope:'edge'},{source_graph:'claims',source_predicate_id:'author',scope:'claim-predicate'}]}];
  const vocab=lensVocabulary(c,'predicates');assert.equal(vocab[0].group,'technical');assert.equal(vocab[1].group,'authorship');
  assert.deepEqual(vocab[1].sources,['sources']);
});
test('a live lens reports changed identities even when counts stay equal',()=>{
  const a={nodes:[{id:'a'}],relations:[{id:'r1'}]},b={nodes:[{id:'b'}],relations:[{id:'r2'}]};
  assert.deepEqual(lensDelta(a,b),{nodes:{added:1,removed:1},relations:{added:1,removed:1}});
  assert.deepEqual(lensDelta(a,a),{nodes:{added:0,removed:0},relations:{added:0,removed:0}});
});
