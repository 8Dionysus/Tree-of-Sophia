import {test} from 'vitest';
import assert from 'node:assert/strict';
import {createLiveResearch,seekSearchPage} from './live-research.mjs';
import {ExplorationSceneCache} from '../src/observatory/exploration-cache.mjs';
import {pageFixture,secondPage} from '../src/observatory/exploration-test-fixtures.mjs';

const deferred=()=>{let resolve;const promise=new Promise(done=>resolve=done);return {promise,resolve};};
function harness(){
  const cache=new ExplorationSceneCache(),first=pageFixture(),events=[],calls=[];
  let ticket;
  const session={discover:async()=>({catalog:{}}),snapshot:()=>cache.snapshot(),
    async open(target,{replace,options}){calls.push({target,replace,options});ticket=cache.begin(first.query,{replace});return cache.accept(ticket,first);},
    async continue(){return cache.accept(ticket,secondPage(first));},select:target=>cache.select(target),
    async inspect(target){return {target,raw:{id:target.id}};},cancelInspect(){},cancelScene(){},dispose(){calls.push('disposed');}};
  const sky={update:(state,labels)=>events.push({type:'update',state,labels}),select:id=>events.push({type:'select',id}),
    selectEdge:id=>events.push({type:'edge',id}),frame:()=>events.push({type:'frame'}),dispose:()=>events.push({type:'disposed'})};
  const controller=createLiveResearch({session,sky});return {controller,session,events,calls,first};
}
test('real cache pages drive the owner sky with stable positions, exact selection and no continuation framing',async()=>{
  const {controller:c,first,events}=harness();await c.start();await c.open(first.query.origin);
  const vertex=c.state().model.vertices[1];c.move(vertex.id,[11,22,33]);c.selectNode(vertex.id);
  await c.continue();assert.equal(c.state().view.nodes.length,3);assert.equal(c.state().view.selection.id,vertex.representativeId);
  assert.equal(events.filter(event=>event.type==='frame').length,1);
  const last=events.filter(event=>event.type==='update').at(-1);assert.deepEqual(last.state.nodes.find(item=>item.id===vertex.id).position,[11,22,33]);
  c.mode('raw');c.mode('compact');c.language('en');assert.equal(events.filter(event=>event.type==='frame').length,1);
  assert.deepEqual(c.selectedTarget(),{kind:'node',id:vertex.representativeId,content_revision:first.nodes[1].content_revision});
});
test('raw relation selection remains a relation and its exact target can become the next origin',async()=>{
  const {controller:c,first}=harness();await c.open(first.query.origin);
  c.selectEdge(c.state().model.edges[0].id);assert.equal(c.selectedTarget().kind,'relation');assert.equal(c.selectedTarget().id,first.relations[0].id);
});
test('failed new query keeps the last good scene and positions; explicit new field alone reframes',async()=>{
  const {controller:c,session,first,events}=harness();await c.open(first.query.origin);
  const old=c.state().view,open=session.open;session.open=async()=>{throw new Error('unavailable');};
  assert.equal(await c.open(first.query.origin,{replace:true}),null);assert.equal(c.state().view,old);assert.equal(c.state().error.message,'unavailable');
  assert.equal(events.filter(event=>event.type==='frame').length,1);session.open=open;
  await c.open(first.query.origin,{replace:true});assert.equal(events.filter(event=>event.type==='frame').length,2);
});
test('closing a card discards ignored-abort late reading; selection races cannot revive it',async()=>{
  const {controller:c,session,first}=harness();await c.open(first.query.origin);
  const pending=deferred();session.inspect=()=>pending.promise;const read=c.read();c.closeReading();pending.resolve({raw:{id:'old'}});
  await read;assert.equal(c.state().reading,null);
});
test('comparison has two reserved exact slots and clearing discards late cards',async()=>{
  const {controller:c,session,first}=harness();await c.open(first.query.origin);
  const pending=deferred();session.inspect=()=>pending.promise;const left=c.pin(),right=c.pin();
  assert.equal(c.state().comparison.length,2);assert.equal(await c.pin(),null);
  c.clearComparison();pending.resolve({raw:{id:'old'}});await Promise.all([left,right]);assert.deepEqual(c.state().comparison,[]);
});
test('dispose invalidates discovery, scene and card callbacks and disposes only its own session and sky',async()=>{
  const {controller:c,session,events,calls}=harness(),pending=deferred();session.discover=()=>pending.promise;
  const start=c.start();c.dispose();pending.resolve({catalog:{}});await start;
  assert.equal(c.state().discovery,null);assert.deepEqual(calls,['disposed']);assert.equal(events.at(-1).type,'disposed');
  assert.equal(await c.start(),null);
});
test('reading and comparison cancellation are independent, including retained expansion',async()=>{
  const {controller:c,session,first}=harness(),cancelled=[];session.cancelInspect=slot=>cancelled.push(slot);
  await c.open(first.query.origin);cancelled.length=0;
  c.closeReading();assert.deepEqual(cancelled,['inspect']);cancelled.length=0;
  c.clearComparison();assert.deepEqual(cancelled,['compare-left','compare-right']);cancelled.length=0;
  await c.open(first.query.origin);assert.deepEqual(cancelled,['inspect']);cancelled.length=0;
  await c.open(first.query.origin,{replace:true});assert.deepEqual(cancelled,[undefined]);
});
test('failed or invalidated comparison ends loading and cleared late failure cannot affect a new slot',async()=>{
  const {controller:c,session,first}=harness();await c.open(first.query.origin);
  session.inspect=async()=>{throw new Error('offline');};await c.pin();
  assert.equal(c.state().comparison[0].error.message,'offline');
  c.clearComparison();session.inspect=async()=>null;await c.pin();
  assert.match(c.state().comparison[0].error.message,/no longer available/);
  c.clearComparison();const pending=deferred();session.inspect=()=>pending.promise.then(()=>{throw new Error('old failure');});
  const old=c.pin();c.clearComparison();session.inspect=async()=>({raw:{id:'current'}});await c.pin();
  const error=c.state().error;pending.resolve();await old;
  assert.equal(c.state().comparison[0].reading.raw.id,'current');assert.equal(c.state().error,error);
});
test('failed reading has a terminal error and successful retry clears it',async()=>{
  const {controller:c,session,first}=harness();await c.open(first.query.origin);
  session.inspect=async()=>{throw new Error('offline');};await c.read();assert.equal(c.state().readingError.message,'offline');
  session.inspect=async()=>({raw:{id:'current'}});await c.read();assert.equal(c.state().readingError,null);
  assert.equal(c.state().reading.raw.id,'current');
});

test('source dossier transition delegates the unchanged owner handle and can be cancelled',async()=>{
  const {controller:c,session,first}=harness();await c.open(first.query.origin);const calls=[];
  session.sourceDossier=async(ref,options)=>{calls.push({ref,options});return {schema:'tos_source_dossier_v1',object_id:ref};};
  session.cancelSourceDossier=()=>calls.push('cancelled');
  const dossier=await c.sourceDossier('tos.work.fixture');assert.equal(dossier.object_id,'tos.work.fixture');assert.deepEqual(calls,[{ref:'tos.work.fixture',options:{}}]);
  c.cancelSourceDossier();assert.equal(calls.at(-1),'cancelled');
});

test('search seeking stops at the first non-empty page and reports bounded continuation state',async()=>{
  const page=(cursor,hasMore,next,nodes=[])=>({nodes,relations:[],page:{cursor,has_more:hasMore,next_cursor:hasMore?next:null}});
  const pages=[page(null,true,'cursor-1'),page('cursor-1',true,'cursor-2'),page('cursor-2',false,null,[{id:'hit'}])],calls=[];
  const result=await seekSearchPage(cursor=>{calls.push(cursor);return pages.shift();},{window:{maxRequests:5,maxBytes:100000,maxTimeMs:1000},now:()=>0});
  assert.deepEqual(calls,[null,'cursor-1','cursor-2']);assert.equal(result.page.nodes[0].id,'hit');assert.equal(result.paused,false);assert.equal(result.reason,'match');assert.equal(result.requests,3);
});

test('search seeking pauses at request, byte and time bounds and suppresses cancellation',async()=>{
  const empty=(cursor,next='next')=>({nodes:[],relations:[],page:{cursor,has_more:true,next_cursor:next}});
  const bounded=await seekSearchPage(async cursor=>empty(cursor),{window:{maxRequests:2,maxBytes:100000,maxTimeMs:1000},now:()=>0});
  assert.equal(bounded.paused,true);assert.equal(bounded.reason,'requests');assert.equal(bounded.requests,2);assert.equal(bounded.page.page.next_cursor,'next');
  const byBytes=await seekSearchPage(async cursor=>empty(cursor),{window:{maxRequests:8,maxBytes:1,maxTimeMs:1000},now:()=>0});
  assert.equal(byBytes.reason,'bytes');assert.equal(byBytes.paused,true);assert.equal(byBytes.requests,1);
  let now=0;const byTime=await seekSearchPage(async cursor=>{now=5;return empty(cursor);},{window:{maxRequests:8,maxBytes:100000,maxTimeMs:5},now:()=>now});
  assert.equal(byTime.reason,'time');assert.equal(byTime.requests,1);
  const pending=deferred();let current=true;const cancelled=seekSearchPage(()=>pending.promise,{isCurrent:()=>current,now:()=>0});current=false;pending.resolve(empty(null));
  assert.equal((await cancelled).cancelled,true);
});

test('seek preserves a valid match when an in-flight page crosses the soft window',async()=>{
  let now=0;
  const page={nodes:[{id:'hit',padding:'x'.repeat(128)}],relations:[],page:{cursor:null,has_more:true,next_cursor:'next'}};
  const result=await seekSearchPage(async()=>{now=10;return page;},{window:{maxRequests:1,maxBytes:8,maxTimeMs:5},now:()=>now});
  assert.equal(result.page.nodes[0].id,'hit');assert.equal(result.paused,false);assert.equal(result.reason,'match');
  assert.ok(result.bytes>8);assert.ok(result.elapsedMs>5);assert.equal(result.windowExceeded.bytes,true);assert.equal(result.windowExceeded.time,true);
});
