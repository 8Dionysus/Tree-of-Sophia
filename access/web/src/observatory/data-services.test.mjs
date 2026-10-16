import './human-forms-wasm-test-runtime.mjs';
import {test} from 'vitest';
import assert from 'node:assert/strict';
import {createObservatoryData} from './data-services.mjs';
import {RequestError} from './knowledge-client.mjs';

test('knowledge and older research queries use the same injected connection',async()=>{
  const calls=[],controller=new AbortController();
  const data=createObservatoryData({fetcher:async(url,options)=>{calls.push({url,options});return {ok:true,json:async()=>({test:true})};}});
  assert.deepEqual(await data.client.request('/catalog',{signal:controller.signal}),{test:true});
  assert.deepEqual(await data.queries.invoke('tos.epistemic.inspect',{mode:'philosophy',item_id:'opaque:/relation'},{signal:controller.signal}),{test:true});
  assert.equal(calls[0].url,'/api/knowledge/catalog');
  assert.equal(decodeURIComponent(new URL('http://test'+calls[1].url).pathname),'/api/philosophy/query/epistemic/opaque:/relation');
  assert.ok(calls.every(call=>call.options.method==='GET'&&call.options.signal instanceof AbortSignal));
});

test('source-text query posts a JSON object through the existing selected transport',async()=>{
  const calls=[],controller=new AbortController();
  const page={operation_profile:'tos_source_text_search_v1',selection:{consumer_id:'reader',profile_id:'selected-v2',
    profile_sha256:'a'.repeat(64),selection_seq:7},hits:[],has_more:false,next_cursor:null};
  const data=createObservatoryData({fetcher:async(url,options)=>{
    calls.push({url,options});return {ok:true,json:async()=>page};
  }});
  const input={query:'  Über  ',document_types:['agent'],source_paths:['ToS/source-witnesses/agents'],limit:12,cursor:'opaque'};
  assert.strictEqual(await data.queries.invoke('tos.source-text.search',input,{signal:controller.signal}),page);
  assert.equal(calls.length,1);
  assert.equal(calls[0].url,'/api/source-text/search');
  assert.equal(calls[0].options.method,'POST');
  assert.equal(calls[0].options.headers['Content-Type'],'application/json');
  assert.ok(calls[0].options.signal instanceof AbortSignal);
  assert.deepEqual(JSON.parse(calls[0].options.body),input);
});

test('source-text query cancellation reaches the selected transport and keeps its reason',async()=>{
  let fetchSignal,started;
  const requestStarted=new Promise(resolve=>{started=resolve;});
  const controller=new AbortController();
  const data=createObservatoryData({timeoutMs:30000,fetcher:(_url,options)=>{
    fetchSignal=options.signal;
    started();
    // Deliberately uncooperative transport: KnowledgeClient must still finish
    // the request from its own abort signal without waiting for this promise.
    return new Promise(()=>{});
  }});
  const pending=data.queries.invoke('tos.source-text.search',{
    query:'fate',document_types:['agent'],source_paths:['ToS/source-witnesses/agents'],limit:12,cursor:null,
  },{signal:controller.signal});
  await requestStarted;
  assert.ok(fetchSignal instanceof AbortSignal);
  const reason=new DOMException('search cancelled','AbortError');
  controller.abort(reason);
  await assert.rejects(pending,error=>error===reason);
  assert.equal(fetchSignal.aborted,true);
  assert.strictEqual(fetchSignal.reason,reason);
});

test('unavailable legacy material keeps its status for the evidence boundary',async()=>{
  const data=createObservatoryData({fetcher:async()=>({ok:false,status:404})});
  await assert.rejects(data.queries.invoke('tos.epistemic.inspect',{mode:'philosophy',item_id:'missing'}),
    error=>error instanceof RequestError&&error.status===404&&error.message.includes('способе просмотра'));
});
