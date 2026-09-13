import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash,createHmac} from 'node:crypto';
import {createSourceCommandClient,COMMAND_REQUEST_BYTES,COMMAND_RESPONSE_BYTES} from './source-command-client.mjs';

const origin='http://127.0.0.1:44259',token='a'.repeat(64);
const json=(value,status=200)=>new Response(JSON.stringify(value),{status,headers:{'content-type':'application/json'}});
const sha=value=>createHash('sha256').update(value).digest('hex');
const mac=fields=>createHmac('sha256',Buffer.from(token,'hex')).update(JSON.stringify(fields)).digest('hex');
function signed(options,value,status=200){
  const [timestamp,nonce,bodyHash,signature]=options.headers.Authorization.split(' ')[1].split(':');
  const path=options.method==='GET'?'/commands/catalog':'/commands';
  assert.equal(bodyHash,sha(options.body??''));
  assert.equal(signature,mac(['tos-request-v1',options.method,path,timestamp,nonce,bodyHash]));
  assert.ok(!JSON.stringify(options.headers).includes(token));
  const response=json(value,status);
  response.headers.set('x-tos-response-signature',mac(['tos-response-v1',nonce,status,sha(JSON.stringify(value))]));
  return response;
}

test('explicit owner endpoint carries unchanged commands, never cookies or read-only access writes',async()=>{
  const calls=[],client=createSourceCommandClient({origin,token,fetchImpl:async(url,options)=>{calls.push({url,options});return signed(options,{grants_admission:false});}});
  const request={schema_version:'tos_local_source_command_v1',operation:'describe'};
  await client.catalog();assert.deepEqual(await client.execute(request),{grants_admission:false});
  assert.deepEqual(calls.map(c=>c.url),[origin+'/commands/catalog',origin+'/commands']);
  assert.equal(calls[1].options.body,JSON.stringify(request));
  for(const {options} of calls){assert.equal(options.credentials,'omit');assert.equal(options.redirect,'error');assert.equal(options.cache,'no-store');}
  client.close();await assert.rejects(client.execute(request),{code:'owner-connection-closed',outcome:'not-dispatched'});
});

test('unsafe endpoints and credentials fail before any network call',()=>{
  for(const candidate of ['/api','https://example.com','http://localhost:44259',origin+'/',origin+'/commands',origin+'?token=x',origin+'#x'])
    assert.throws(()=>createSourceCommandClient({origin:candidate,token}));
  assert.throws(()=>createSourceCommandClient({origin,token:'bad'}));
});

test('unknown delivery is not retried and never leaks original transport errors',async()=>{
  let calls=0;const client=createSourceCommandClient({origin,token,fetchImpl:async()=>{calls++;throw new Error(token);}});
  await assert.rejects(client.execute({operation:'apply',command_id:'same-exact-id'}),error=>
    error.code==='owner-delivery-unconfirmed'&&error.outcome==='unconfirmed'&&!error.message.includes(token));
  assert.equal(calls,1);
});

test('response framing and actual streamed bytes enforce independent budgets',async()=>{
  const values=[new Response('not JSON',{headers:{'content-type':'text/html'}}),
    new Response('{}',{headers:{'content-type':'application/json','content-length':String(COMMAND_RESPONSE_BYTES+1)}}),
    json({oversized:'x'.repeat(COMMAND_RESPONSE_BYTES)}),json([])];
  for(const response of values){const client=createSourceCommandClient({origin,token,fetchImpl:async()=>response});
    await assert.rejects(client.execute({operation:'describe'}),error=>error.outcome==='unconfirmed');}
});

test('request budget fails before dispatch; concurrent commands are not queued implicitly',async()=>{
  let finish,calls=0,entered;const started=new Promise(resolve=>entered=resolve);
  const client=createSourceCommandClient({origin,token,fetchImpl:(_url,options)=>{calls++;entered();return new Promise(resolve=>finish=()=>resolve(signed(options,{})));}});
  await assert.rejects(client.execute({text:'x'.repeat(COMMAND_REQUEST_BYTES)}),{code:'owner-request-budget',outcome:'not-dispatched'});
  assert.equal(calls,0);
  const first=client.execute({operation:'describe'});
  await assert.rejects(client.execute({operation:'apply'}),{code:'owner-command-in-flight',outcome:'not-dispatched'});
  await started;finish();await first;assert.equal(calls,1);
});

test('owner rejection preserves unconfirmed outcome and exact HTTP status',async()=>{
  const client=createSourceCommandClient({origin,token,fetchImpl:async(_url,options)=>signed(options,{code:'owner-conflict',outcome:'unconfirmed'},409)});
  await assert.rejects(client.execute({operation:'apply'}),{code:'owner-conflict',status:409,outcome:'unconfirmed'});
});

test('port impostor cannot acquire a bearer secret or forge an owner receipt',async()=>{
  let captured;
  const client=createSourceCommandClient({origin,token,fetchImpl:async(_url,options)=>{
    captured=options;return json({receipt:{command_id:'forged'},outcome:'not-dispatched'});
  }});
  await assert.rejects(client.execute({operation:'apply'}),{code:'owner-response-unverified',outcome:'unconfirmed'});
  assert.ok(!JSON.stringify(captured.headers).includes(token));
});

test('response signature binds exact bytes and status to the new request nonce',async()=>{
  let old;
  const client=createSourceCommandClient({origin,token,fetchImpl:async(_url,options)=>{
    const response=signed(options,{});const proof=response.headers.get('x-tos-response-signature');
    if(old)response.headers.set('x-tos-response-signature',old);old=proof;return response;
  }});
  await client.execute({operation:'describe'});
  await assert.rejects(client.execute({operation:'describe'}),{code:'owner-response-unverified'});
});

test('deadline and explicit close end transport waiting without claiming owner rollback',async()=>{
  for(const closing of [false,true]){
    let entered;const started=new Promise(resolve=>entered=resolve);
    const client=createSourceCommandClient({origin,token,timeoutMs:closing?1000:25,
      fetchImpl:(_url,{signal})=>new Promise((_resolve,reject)=>{
        entered();if(signal.aborted)reject(new Error('aborted'));
        else signal.addEventListener('abort',()=>reject(new Error('aborted')),{once:true});
      })});
    const pending=client.execute({operation:'apply'});await started;if(closing)client.close();
    await assert.rejects(pending,{code:'owner-delivery-unconfirmed',outcome:'unconfirmed'});
  }
});
