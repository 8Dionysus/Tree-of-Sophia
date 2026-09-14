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

test('unavailable legacy material keeps its status for the evidence boundary',async()=>{
  const data=createObservatoryData({fetcher:async()=>({ok:false,status:404})});
  await assert.rejects(data.queries.invoke('tos.epistemic.inspect',{mode:'philosophy',item_id:'missing'}),
    error=>error instanceof RequestError&&error.status===404&&error.message.includes('способе просмотра'));
});
