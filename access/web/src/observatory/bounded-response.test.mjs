import {afterEach,test,vi} from 'vitest';
import assert from 'node:assert/strict';
import {KnowledgeClient,ContractError,RequestError,RevisionError,RequestSlots} from './knowledge-client.mjs';
import {DEFAULT_RESPONSE_BYTES,MAX_RESPONSE_BYTES,readBoundedJSON} from './bounded-response.mjs';

const encode=value=>new TextEncoder().encode(value);
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no});return {promise,resolve,reject};};
const isStatus=status=>error=>error instanceof RequestError&&error.status===status;
const client=(response,options={})=>new KnowledgeClient({fetcher:async()=>response,...options});
function streamed(chunks,{length=null,close=true,cancel=()=>{}}={}){
  let index=0;
  const source=new ReadableStream({pull(controller){
    if(index<chunks.length)controller.enqueue(chunks[index++]);
    else if(close)controller.close();
  },cancel},{highWaterMark:0});
  const state={reads:0,cancels:0,releases:0,bodyCancels:0};
  const response={ok:true,status:200,headers:{get:()=>length},
    body:{cancel(reason){state.bodyCancels++;return source.cancel(reason);},getReader(){
      const reader=source.getReader();return {
        read(){state.reads++;return reader.read();},
        cancel(reason){state.cancels++;return reader.cancel(reason);},
        releaseLock(){state.releases++;return reader.releaseLock();},
      };
    }},json:vi.fn(()=>{throw new Error('A real body must never use json()');})};
  return {response,state};
}
afterEach(()=>vi.useRealTimers());

test('browser byte profile defaults to 4 MiB and accepts configurable limits only up to 8 MiB',()=>{
  assert.equal(new KnowledgeClient().maxResponseBytes,4*1024*1024);
  assert.equal(DEFAULT_RESPONSE_BYTES,4*1024*1024);assert.equal(MAX_RESPONSE_BYTES,8*1024*1024);
  for(const maxResponseBytes of [1,64,MAX_RESPONSE_BYTES])assert.equal(new KnowledgeClient({maxResponseBytes}).maxResponseBytes,maxResponseBytes);
  for(const maxResponseBytes of [0,-1,1.5,NaN,Infinity,MAX_RESPONSE_BYTES+1,'64',null])assert.throws(()=>new KnowledgeClient({maxResponseBytes}),RangeError);
});

test('native Response streams are read before parsing; json() is never used',async()=>{
  const packet={word:'λόγος 雪 🌌',flag:false,value:0,missing:null};
  const response=new Response(JSON.stringify(packet));response.json=vi.fn(()=>{throw new Error('unbounded parse');});
  assert.deepEqual(await client(response).request('/lens'),packet);assert.equal(response.json.mock.calls.length,0);
});

test('actual UTF-8 byte ceiling admits exact size and rejects one byte over in one chunk or byte-split chunks',async()=>{
  const packet={word:'雪🌌'},bytes=encode(JSON.stringify(packet));
  assert.ok(bytes.length>JSON.stringify(packet).length);
  for(const chunks of [[bytes],Array.from(bytes,byte=>new Uint8Array([byte]))]){
    const exact=streamed(chunks);assert.deepEqual(await client(exact.response,{maxResponseBytes:bytes.length}).request('/lens'),packet);
    assert.equal(exact.state.releases,1);assert.equal(exact.response.json.mock.calls.length,0);
    const excess=streamed(chunks);await assert.rejects(client(excess.response,{maxResponseBytes:bytes.length-1}).request('/lens'),isStatus(413));
    assert.equal(excess.state.cancels,1);assert.equal(excess.state.releases,1);assert.equal(excess.response.json.mock.calls.length,0);
  }
});

test('views with offsets, empty chunks and a growing buffer preserve the exact body',async()=>{
  const packet={text:'x'.repeat(70000)},bytes=encode(JSON.stringify(packet)),backing=new Uint8Array(bytes.length+20);
  backing.set(bytes,10);
  const {response,state}=streamed([new Uint8Array(),backing.subarray(10,1000),new Uint8Array(),backing.subarray(1000,10+bytes.length)]);
  assert.deepEqual(await client(response,{maxResponseBytes:bytes.length}).request('/lens'),packet);assert.equal(state.releases,1);
});

test('valid oversized Content-Length rejects before reader acquisition and cancels the body',async()=>{
  for(const length of ['100','000100',' \t100\t ','9'.repeat(400)]){
    const {response,state}=streamed([encode('{}')],{length});
    await assert.rejects(client(response,{maxResponseBytes:32}).request('/lens'),isStatus(413));
    assert.equal(state.reads,0);assert.equal(state.bodyCancels,1);assert.equal(state.releases,0);
  }
});

test('absent, malformed, duplicate and low Content-Length never replaces actual byte counting',async()=>{
  for(const length of [null,'','abc','-100','+100','1e3','100,100','100\n','100.0','0','2','0002']){
    const small=streamed([encode('{}')],{length});assert.deepEqual(await client(small.response,{maxResponseBytes:32}).request('/lens'),{});
    const large=streamed([encode(JSON.stringify({x:'雪'.repeat(20)}))],{length});
    await assert.rejects(client(large.response,{maxResponseBytes:32}).request('/lens'),isStatus(413));
    assert.ok(large.state.reads>0);assert.equal(large.state.cancels,1);
  }
});

test('an oversized stream is cancelled without reading its tail or waiting for cancellation to settle',async()=>{
  const cancellation=deferred(),{response,state}=streamed([encode('x'.repeat(33)),encode('never consumed')],{close:false,cancel:()=>cancellation.promise});
  await assert.rejects(client(response,{maxResponseBytes:32}).request('/lens'),isStatus(413));
  assert.equal(state.reads,1);assert.equal(state.cancels,1);assert.equal(state.releases,1);cancellation.resolve();
});

test('malformed JSON and UTF-8 produce readable contract errors with reader cleanup',async()=>{
  for(const bytes of [encode('{'),new Uint8Array([123,34,120,34,58,34,255,34,125])]){
    const {response,state}=streamed([bytes]);
    await assert.rejects(client(response).request('/lens'),ContractError);
    assert.equal(state.cancels,1);assert.equal(state.releases,1);assert.equal(response.json.mock.calls.length,0);
  }
});

test('only object packets are admitted, never scalar, null or array roots',async()=>{
  for(const raw of ['null','[]','1','true','"text"'])await assert.rejects(client(new Response(raw)).request('/lens'),ContractError);
});

test('non-byte chunks fail closed without json-only fallback',async()=>{
  const {response,state}=streamed(['{}']);await assert.rejects(client(response).request('/lens'),ContractError);
  assert.equal(state.cancels,1);assert.equal(state.releases,1);assert.equal(response.json.mock.calls.length,0);
});

test('a non-null body without a byte reader cannot bypass stream admission',async()=>{
  const cancel=vi.fn(),json=vi.fn(async()=>({}));
  await assert.rejects(client({ok:true,body:{cancel},json}).request('/lens'),ContractError);
  assert.equal(cancel.mock.calls.length,1);assert.equal(json.mock.calls.length,0);
});

test('reader acquisition and reading failures cancel their owned body or reader',async()=>{
  const error=new TypeError('broken source'),cancel=vi.fn(),json=vi.fn(async()=>({}));
  await assert.rejects(client({ok:true,body:{getReader(){throw error;},cancel},json}).request('/lens'),isStatus(0));
  assert.equal(cancel.mock.calls.length,1);assert.equal(json.mock.calls.length,0);
  const reader={read:()=>Promise.reject(error),cancel:vi.fn(()=>Promise.reject(new Error('cancel failed'))),releaseLock:vi.fn()};
  await assert.rejects(client({ok:true,body:{getReader:()=>reader},json}).request('/lens'),isStatus(0));
  assert.equal(reader.cancel.mock.calls.length,1);assert.equal(reader.releaseLock.mock.calls.length,1);
});

test('HTTP errors preserve status mapping and cancel unread response bodies',async()=>{
  for(const status of [400,403,404,409,410,413,503]){
    const {response,state}=streamed([encode('{}')]);response.ok=false;response.status=status;
    await assert.rejects(client(response).request('/lens'),status===409?RevisionError:isStatus(status));
    assert.equal(state.bodyCancels,1);assert.equal(state.reads,0);
  }
});

test('json-only in-memory transports retain object identity with a post-parse UTF-8 size check',async()=>{
  const packet={word:'雪🌌'},size=encode(JSON.stringify(packet)).length,json=vi.fn(async()=>packet);
  assert.equal(await client({ok:true,json},{maxResponseBytes:size}).request('/lens'),packet);
  await assert.rejects(client({ok:true,json},{maxResponseBytes:size-1}).request('/lens'),isStatus(413));
  assert.equal(json.mock.calls.length,2);
});

test('json-only compatibility still rejects non-JSON values and bad root shapes',async()=>{
  const cyclic={};cyclic.self=cyclic;
  for(const packet of [undefined,cyclic,{x:1n},null,[],1])await assert.rejects(client({ok:true,json:async()=>packet}).request('/lens'),ContractError);
});

test('the hard ceiling is checked at read admission even if public configuration was mutated',async()=>{
  const response=new Response('{}'),instance=client(response);instance.maxResponseBytes=MAX_RESPONSE_BYTES+1;
  await assert.rejects(instance.request('/lens'),RangeError);
  await assert.rejects(readBoundedJSON(response,Infinity,new AbortController().signal),RangeError);
});

test('pre-aborted callers never start a fetch',async()=>{
  for(const reason of [new Error('caller cancelled'),new SyntaxError('caller reason'),new TypeError('caller reason')]){
    const controller=new AbortController(),fetcher=vi.fn();controller.abort(reason);
    await assert.rejects(new KnowledgeClient({fetcher}).request('/lens',{signal:controller.signal}),error=>error===reason);
    assert.equal(fetcher.mock.calls.length,0);
  }
});

test('a timeout rejects promptly when fetch ignores abort and cancels a late response',async()=>{
  vi.useFakeTimers();const pending=deferred(),instance=new KnowledgeClient({fetcher:()=>pending.promise,timeoutMs:25});
  const request=assert.rejects(instance.request('/lens'),isStatus(504));await vi.advanceTimersByTimeAsync(25);await request;
  const {response,state}=streamed([encode('{}')]);pending.resolve(response);await vi.advanceTimersByTimeAsync(0);
  assert.equal(state.bodyCancels,1);assert.equal(state.reads,0);assert.equal(vi.getTimerCount(),0);
});

test('a timeout during a pending stream read cancels and releases without waiting for the source',async()=>{
  vi.useFakeTimers();const cancellation=deferred(),{response,state}=streamed([encode('{')],{close:false,cancel:()=>cancellation.promise});
  const request=assert.rejects(client(response,{timeoutMs:25}).request('/lens'),isStatus(504));
  await vi.advanceTimersByTimeAsync(25);await request;
  assert.equal(state.reads,2);assert.equal(state.cancels,1);assert.equal(state.releases,1);assert.equal(vi.getTimerCount(),0);cancellation.resolve();
});

test('caller abort during a stream read preserves its reason and cleans up',async()=>{
  vi.useFakeTimers();const controller=new AbortController(),reason=new Error('superseded'),{response,state}=streamed([encode('{')],{close:false});
  const request=assert.rejects(client(response).request('/lens',{signal:controller.signal}),error=>error===reason);
  await vi.advanceTimersByTimeAsync(0);controller.abort(reason);await request;
  assert.equal(state.cancels,1);assert.equal(state.releases,1);assert.equal(vi.getTimerCount(),0);
});

test('a json-only transport ignoring abort cannot return a late packet after timeout',async()=>{
  vi.useFakeTimers();const pending=deferred(),json=vi.fn(()=>pending.promise);
  const request=assert.rejects(client({ok:true,json},{timeoutMs:25}).request('/lens'),isStatus(504));
  await vi.advanceTimersByTimeAsync(25);await request;pending.resolve({late:true});await vi.advanceTimersByTimeAsync(0);
  assert.equal(json.mock.calls.length,1);assert.equal(vi.getTimerCount(),0);
});

test('late rejections from an aborted custom fetch are consumed and never replace caller cancellation',async()=>{
  vi.useFakeTimers();const pending=deferred(),controller=new AbortController(),instance=new KnowledgeClient({fetcher:()=>pending.promise});
  const request=assert.rejects(instance.request('/lens',{signal:controller.signal}),error=>error.name==='AbortError');controller.abort();await request;
  pending.reject(new TypeError('late network failure'));await vi.advanceTimersByTimeAsync(0);assert.equal(vi.getTimerCount(),0);
});

test('RequestSlots suppression still rejects stale transport ownership without accepting a late packet',async()=>{
  vi.useFakeTimers();const pending=deferred(),slots=new RequestSlots(),instance=new KnowledgeClient({fetcher:()=>pending.promise});
  const request=slots.run('scene',signal=>instance.request('/lens',{signal}));slots.cancel('scene');
  assert.deepEqual(await request,{current:false});
  const {response,state}=streamed([encode('{}')]);pending.resolve(response);await vi.advanceTimersByTimeAsync(0);
  assert.equal(state.bodyCancels,1);assert.equal(state.reads,0);assert.equal(vi.getTimerCount(),0);
});
