import {test} from 'vitest';
import assert from 'node:assert/strict';
import {StableExplorationLayout,liveLabel,livePredicateLabel} from './live-model.mjs';
import {ExplorationSceneCache} from '../src/observatory/exploration-cache.mjs';
import {buildExplorationSceneModel} from '../src/observatory/scene-model.mjs';
import {pageFixture,secondPage} from '../src/observatory/exploration-test-fixtures.mjs';

test('page growth, selection and projection changes preserve exact existing layout positions',()=>{
  const first=pageFixture(),cache=new ExplorationSceneCache(),ticket=cache.begin(first.query),layout=new StableExplorationLayout();
  let view=cache.accept(ticket,first),model=buildExplorationSceneModel(view),sky=layout.project(view,model),id=sky.nodes[0].id;
  layout.move(id,[12,34,56]);const before=JSON.stringify(view);
  sky=layout.project(view,model,{language:'en'});assert.deepEqual(sky.nodes[0].position,[12,34,56]);assert.equal(JSON.stringify(view),before);
  view=cache.accept(ticket,secondPage(first));model=buildExplorationSceneModel(view);sky=layout.project(view,model);
  assert.deepEqual(sky.nodes.find(node=>node.id===id).position,[12,34,56]);assert.equal(sky.nodes.length,3);
  layout.project(view,buildExplorationSceneModel(view,{mode:'raw'}));
  assert.deepEqual(layout.project(view,model).nodes.find(node=>node.id===id).position,[12,34,56]);
  const copy=layout.position(id);copy[0]=999;assert.deepEqual(layout.position(id),[12,34,56]);
  layout.reset();assert.equal(layout.position(id),null);
});

test('opaque IDs are not reconstructed into human names and unknown styles remain neutral',()=>{
  const raw={id:'tos.claim.secret-fact',display:{},content_revision:'a'.repeat(64)};
  assert.equal(liveLabel(raw).text,'Название не предоставлено');
  const cache=new ExplorationSceneCache(),packet=pageFixture(),ticket=cache.begin(packet.query),view=cache.accept(ticket,packet);
  const sky=new StableExplorationLayout().project(view,buildExplorationSceneModel(view));
  assert.ok(sky.nodes.every(node=>node.kind==='knowledge'&&node.color==='#bdd5ed'));
  assert.equal(sky.edges[0].label,'Учебное отношение');
});

test('layout moves admit only existing finite bounded coordinates',()=>{
  const layout=new StableExplorationLayout();assert.throws(()=>layout.move('absent',[0,0,0]),RangeError);
  const packet=pageFixture(),cache=new ExplorationSceneCache(),view=cache.accept(cache.begin(packet.query),packet),model=buildExplorationSceneModel(view);
  const id=layout.project(view,model).nodes[0].id;
  for(const pos of [[NaN,0,0],[Infinity,0,0],[0,0],[1e6,0,0]])assert.throws(()=>layout.move(id,pos),RangeError);
});
test('catalog predicate labels use the supplied direct language map, never opaque ID words',()=>{
  const predicate={predicate_id:'has_object',display:{ru:'имеет объект',en:'has object',default:'has object'}};
  assert.equal(livePredicateLabel(predicate,'ru'),'имеет объект');
  assert.equal(livePredicateLabel(predicate,'en'),'has object');
  assert.equal(livePredicateLabel({predicate_id:'has_object',display:{}},'ru'),'Название не предоставлено');
});
