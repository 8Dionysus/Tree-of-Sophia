import {compareLosslessJson} from './lossless-json-compare.mjs';
import {frozenReference} from './frozen-reference.mjs';
import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync,mkdtempSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {DatabaseSync} from 'node:sqlite';
import {createHash,generateKeyPairSync,sign} from 'node:crypto';
import {Miniflare,convertV4MiniflareOptions} from 'miniflare';
import {knowledgeSearchD1,knowledgeSearchD1Indexed} from '../src/knowledge-store.ts';
import {nativePacketJson} from '../src/native-lens.ts';
import {sourceTextGatewayCapability,sourceTextSearchGatewayResponse} from '../src/source-text-gateway.ts';
import {publishedNodeFixtureWorker,publishedWorkerFixtureModules,publishedFixtureSearchControlsRuntime} from './native-lens-fixture.ts';

const reference=(referenceId, input) => JSON.parse(frozenReference(import.meta.url, referenceId, input===undefined?undefined:JSON.stringify(input)));
const sha=raw=>createHash('sha256').update(raw).digest('hex');
const fixture=reference("search/fixture");
const schema=`CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,title_text TEXT,search_text TEXT,json TEXT);
CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,source_graph TEXT,from_id TEXT,to_id TEXT,predicate_id TEXT,relation_type_id TEXT,label_text TEXT,search_text TEXT,json TEXT);
CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);
CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);
CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);
CREATE TABLE knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT,PRIMARY KEY(kind,id));
CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);
CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);
CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);
CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);
CREATE TABLE knowledge_search_documents(kind TEXT,position INTEGER,id TEXT,source_graph TEXT,kind_id TEXT,predicate_id TEXT,id_lower TEXT,native_id_lower TEXT,identity_values TEXT,visible_values TEXT,document_chars INTEGER,document_digest TEXT,PRIMARY KEY(kind,position));
CREATE TABLE knowledge_search_grams(kind TEXT,n INTEGER,gram TEXT,position INTEGER,PRIMARY KEY(kind,n,gram,position));
CREATE TABLE knowledge_search_gram_stats(kind TEXT,n INTEGER,gram TEXT,postings INTEGER,PRIMARY KEY(kind,n,gram));`;
const migration=readFileSync(new URL('../migrations/0001-exploration.sql',import.meta.url),'utf8');
function bindings(kind,row){const n=JSON.parse(row.raw);return kind==='nodes'?[n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,'',row.text,row.raw]:[n.id,n.native_id,n.source_graph,n.from_id,n.to_id,n.predicate_id,n.relation_type_id,'',row.text,row.raw];}
function database(version=9){
 const sqlite=new DatabaseSync(':memory:');sqlite.exec(schema);sqlite.exec(migration);
 for(const [key,raw]of Object.entries(fixture.metadata[version]))sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(key,raw);
 for(const kind of ['nodes','relations'])for(const row of fixture.rows[kind]){const args=bindings(kind,row);sqlite.prepare(`INSERT INTO knowledge_${kind} VALUES (${args.map(()=>'?').join(',')})`).run(...args);}
 for(const [table,key]of [['knowledge_search_documents','documents'],['knowledge_search_grams','grams'],['knowledge_search_gram_stats','stats']])for(const args of fixture[key])sqlite.prepare(`INSERT INTO ${table} VALUES (${args.map(()=>'?').join(',')})`).run(...args);
 const statements=[],hook={after:null};
 const db={prepare(sql){let args=[];return {bind(...values){args=values;return this;},async all(){const results=sqlite.prepare(sql).all(...args);statements.push({sql,args,results});hook.after?.(sql,args);return {results,meta:{rows_read:0}};},async first(){const result=sqlite.prepare(sql).get(...args)??null;statements.push({sql,args,results:result?[result]:[]});hook.after?.(sql,args);return result;}};}};
 return {db,sqlite,statements,hook,close(){sqlite.close();}};
}
const worker=publishedNodeFixtureWorker;
async function response(database,mode='legacy',query='',extra={},method='GET'){
 const params=new URLSearchParams({mode,query,...extra});
 return (await worker()).fetch(new Request('https://tos.test/api/knowledge/search?'+params,{method}),{DB:database.db,ASSETS:{fetch(){throw new Error('search must not fetch static or full-graph fallback');}}},{});
}
const options=(query='',extra={})=>({query,sources:null,kindIds:[],predicateIds:[],offset:0,limit:100,...extra});
async function direct(database,mode,query='',extra={}){return nativePacketJson(await (mode==='indexed'?knowledgeSearchD1Indexed:knowledgeSearchD1)(database.db,options(query,extra),publishedFixtureSearchControlsRuntime));}
function oracle(mode,query,extra={},graph=fixture.graph){return reference("search/indexed-pages",{graph,mode,query,extra});}
function assertPackets(actual, expected, indexed = false) {
  assert.deepEqual(compareLosslessJson(actual, expected, {indexed}), []);
}

test('Cloudflare explicitly refuses source-text search without a selected durable PostgreSQL consumer',async()=>{
 const response=await (await worker()).fetch(new Request('https://tos.test/api/source-text/search',{
  method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'alpha',limit:5}),
 }),{},{});
 assert.equal(response.status,501);
 assert.deepEqual(await response.json(),{
  error:'source-text search unavailable: an explicitly selected durable PostgreSQL consumer is required',
 });
});

const callerAccessIssuer='https://tos-example.cloudflareaccess.com';
const callerAccessAudience='tos-private-source-text-test-audience';
const callerAccessKeys=generateKeyPairSync('rsa',{modulusLength:2048,publicExponent:0x10001});
const callerAccessJwk={...callerAccessKeys.publicKey.export({format:'jwk'}),kid:'test-access-kid',kty:'RSA',alg:'RS256',use:'sig'};
const base64url=value=>Buffer.from(typeof value==='string'?value:JSON.stringify(value)).toString('base64url');
function callerAccessToken(claimOverrides={},headerOverrides={}){
 const now=Math.floor(Date.now()/1000);
 const header=base64url({alg:'RS256',kid:'test-access-kid',typ:'JWT',...headerOverrides});
 const claims=base64url({iss:callerAccessIssuer,aud:[callerAccessAudience],iat:now,exp:now+3600,type:'app',...claimOverrides});
 const signingInput=`${header}.${claims}`;
 return `${signingInput}.${sign('RSA-SHA256',Buffer.from(signingInput),callerAccessKeys.privateKey).toString('base64url')}`;
}
const validCallerAccessToken=callerAccessToken();
const callerJwksUrl=`${callerAccessIssuer}/cdn-cgi/access/certs`;
const callerJwksResponse=()=>new Response(JSON.stringify({keys:[callerAccessJwk]}),{
 status:200,headers:{'content-type':'application/json'},
});
const gatewayFetcher=nativeFetcher=>async(input,init)=>String(input)===callerJwksUrl
 ? callerJwksResponse()
 : nativeFetcher(input,init);

const gatewayEnv=(overrides={})=>({
 TOS_SOURCE_TEXT_GATEWAY_URL:'https://native-source.example.net',
 TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_ID:'test-access-id',
 TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_SECRET:'test-access-secret',
 TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_ISSUER:callerAccessIssuer,
 TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_AUDIENCE:callerAccessAudience,
 ...overrides,
});
const sourceTextRequest=(body='{"query":"α","limit":5}',{url='https://tos.test/api/source-text/search?destination=ignored',token=validCallerAccessToken}={})=>new Request(url,{
 method:'POST',headers:{'content-type':'application/json','cookie':'caller=private','authorization':'Bearer caller-private',...(token?{'cf-access-jwt-assertion':token}:{})},body,
});

test('source-text gateway requires a cryptographically valid Access app token on alternate Worker hosts',async()=>{
 let nativeCalls=0,jwksCalls=0;
 const fetcher=async(input)=>{
  if(String(input)===callerJwksUrl){jwksCalls++;return callerJwksResponse();}
  nativeCalls++;return new Response('{"hits":[]}',{headers:{'content-type':'application/json'}});
 };
 const anonymous=await sourceTextSearchGatewayResponse(sourceTextRequest(undefined,{token:null}),gatewayEnv(),fetcher);
 assert.equal(anonymous.status,401);
 assert.equal(jwksCalls,0,'a missing assertion must fail before any key lookup');
 assert.equal(nativeCalls,0);

 const forgedParts=validCallerAccessToken.split('.');
 const forgedSignature=Buffer.from(forgedParts[2],'base64url');forgedSignature[0]^=1;
 const alternateHost=sourceTextRequest(undefined,{
  url:'https://tree-of-sophia.account.workers.dev/api/source-text/search',
  token:`${forgedParts[0]}.${forgedParts[1]}.${forgedSignature.toString('base64url')}`,
 });
 const forged=await sourceTextSearchGatewayResponse(alternateHost,gatewayEnv(),fetcher);
 assert.equal(forged.status,401);
 assert.equal(jwksCalls,1,'the alternate host must still perform signature validation');
 assert.equal(nativeCalls,0,'a caller-supplied assertion header cannot reach the native owner');
});

test('source-text gateway rejects invalid issuer, audience, expiry, token type, algorithm, and key id',async()=>{
 const rejected=[
  callerAccessToken({aud:['wrong-audience']}),
  callerAccessToken({iss:'https://other.cloudflareaccess.com'}),
  callerAccessToken({exp:Math.floor(Date.now()/1000)-1}),
  callerAccessToken({iat:undefined}),
  callerAccessToken({iat:Math.floor(Date.now()/1000)+120}),
  callerAccessToken({nbf:Math.floor(Date.now()/1000)+30}),
  callerAccessToken({type:'service'}),
  callerAccessToken({}, {alg:'HS256'}),
  callerAccessToken({}, {kid:'unknown-kid'}),
 ];
 let jwksCalls=0,nativeCalls=0;
 for(const token of rejected){
  const response=await sourceTextSearchGatewayResponse(sourceTextRequest(undefined,{token}),gatewayEnv(),async(input)=>{
   if(String(input)===callerJwksUrl){jwksCalls++;return callerJwksResponse();}
   nativeCalls++;return new Response('{"hits":[]}');
  });
  assert.equal(response.status,401);
 }
 assert.equal(jwksCalls,1,'only the unknown-key token needs a bounded JWKS lookup');
 assert.equal(nativeCalls,0,'claim/header rejection must precede native access');
});

test('source-text gateway fails closed when fixed Access JWKS is invalid or unavailable',async()=>{
 let nativeCalls=0;
 const malformed=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),async(input)=>{
  if(String(input)===callerJwksUrl)return new Response('{"keys":"invalid"}',{headers:{'content-type':'application/json'}});
  nativeCalls++;return new Response('{"hits":[]}');
 });
 assert.equal(malformed.status,503);
 assert.equal(nativeCalls,0);

 const redirected=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),async(input)=>{
  if(String(input)===callerJwksUrl)return new Response('',{status:302,headers:{location:'https://elsewhere.example.net/jwks','content-type':'application/json'}});
  nativeCalls++;return new Response('{"hits":[]}');
 });
 assert.equal(redirected.status,503);
 assert.equal(nativeCalls,0);

 const excessiveKeys=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),async(input)=>{
  if(String(input)===callerJwksUrl)return new Response(JSON.stringify({keys:Array(9).fill(callerAccessJwk)}),{headers:{'content-type':'application/json'}});
  nativeCalls++;return new Response('{"hits":[]}');
 });
 assert.equal(excessiveKeys.status,503);

 const oversizedKeys=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),async(input)=>{
  if(String(input)===callerJwksUrl)return new Response(' '.repeat(16*1024+1),{headers:{'content-type':'application/json'}});
  nativeCalls++;return new Response('{"hits":[]}');
 });
 assert.equal(oversizedKeys.status,503);
 assert.equal(nativeCalls,0);
});

test('source-text gateway binds the fixed native route, requires Access, and preserves only the native JSON response',async()=>{
 const caller=sourceTextRequest(),rawBody=await caller.clone().text();let captured,jwksInit;
 const response=await sourceTextSearchGatewayResponse(caller,gatewayEnv(),async(input,init)=>{
  if(String(input)===callerJwksUrl){jwksInit=init;return callerJwksResponse();}
  captured={url:String(input),init};
  return new Response('{"operation_profile":"tos_source_text_search_v1","query":"α","hits":[]}',{
   status:200,headers:{'content-type':'application/json; charset=utf-8','cache-control':'public, max-age=300','set-cookie':'native=private'},
  });
 });
 assert.equal(response.status,200);
 assert.equal(await response.text(),'{"operation_profile":"tos_source_text_search_v1","query":"α","hits":[]}');
 assert.equal(captured.url,'https://native-source.example.net/api/source-text/search');
 assert.equal(captured.init.method,'POST');
 assert.equal(captured.init.redirect,'manual');
 assert.ok(captured.init.signal instanceof AbortSignal);
 assert.notEqual(captured.init.signal,caller.signal,'the operation signal also enforces the gateway deadline');
 assert.deepEqual([...new Headers(captured.init.headers).keys()].sort(),[
  'accept','cf-access-client-id','cf-access-client-secret','content-type',
 ]);
 assert.equal(new Headers(captured.init.headers).get('cf-access-client-id'),'test-access-id');
 assert.equal(new Headers(captured.init.headers).get('cf-access-client-secret'),'test-access-secret');
 assert.equal(new Headers(captured.init.headers).get('cf-access-jwt-assertion'),null);
 assert.equal(new Headers(captured.init.headers).get('authorization'),null);
 assert.equal(new Headers(captured.init.headers).get('cookie'),null);
 assert.deepEqual([...new Headers(jwksInit.headers).keys()],['accept'],'caller JWKS fetch sends no caller or native service credentials');
 assert.equal(jwksInit.signal,captured.init.signal,'JWKS and native transport share one deadline signal');
 assert.equal(new TextDecoder().decode(captured.init.body),rawBody);
 assert.equal(response.headers.get('cache-control'),'no-store');
 assert.equal(response.headers.get('set-cookie'),null);
 assert.equal(response.headers.get('x-content-type-options'),'nosniff');

 const capability=sourceTextGatewayCapability(gatewayEnv(),caller.url);
 assert.equal(capability.status,'selected');
 assert.equal(capability.selected,true);
 assert.equal(capability.runtime_readiness,'not-probed');
 assert.equal(capability.endpoint_origin_exposed,false);
 assert.equal(JSON.stringify(capability).includes('native-source.example.net'),false);
 assert.equal(JSON.stringify(capability).includes(callerAccessAudience),false);
});

test('source-text gateway deadline covers stalled request body, origin headers, and native response body',async()=>{
 let jwksSignal;
 const jwksTimeout=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),(input,init)=>{
  jwksSignal=init.signal;
  return new Promise((_,reject)=>init.signal.addEventListener('abort',()=>reject(init.signal.reason),{once:true}));
 },100);
 assert.equal(jwksTimeout.status,504);
 assert.equal(jwksSignal.aborted,true,'the shared deadline includes Access JWKS lookup');

 let requestBodyCancelled=false;
 const stalledRequestBody=new ReadableStream({
  start(){},
  cancel(){requestBodyCancelled=true;},
 });
 const request=new Request('https://tos.test/api/source-text/search',{
  method:'POST',headers:{'content-type':'application/json','cf-access-jwt-assertion':validCallerAccessToken},body:stalledRequestBody,duplex:'half',
 });
 const bodyTimeout=await sourceTextSearchGatewayResponse(request,gatewayEnv(),gatewayFetcher(async()=>{
  throw new Error('request body must time out before fetch');
 }),100);
 assert.equal(bodyTimeout.status,504);
 assert.equal(requestBodyCancelled,true,'deadline cancels the active inbound reader');

 let fetchSignal,fetchCancelled=false;
 const fetchTimeout=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher((_input,init)=>{
  fetchSignal=init.signal;
  return new Promise((_,reject)=>init.signal.addEventListener('abort',()=>{
   fetchCancelled=true;reject(init.signal.reason);
  },{once:true}));
 }),100);
 assert.equal(fetchTimeout.status,504);
 assert.equal(fetchSignal.aborted,true,'deadline aborts the native fetch');
 assert.equal(fetchCancelled,true,'stalled native fetch observes cancellation');

 let responseBodyCancelled=false;
 const stalledResponseBody=new ReadableStream({
  start(){},
  cancel(){responseBodyCancelled=true;},
 });
 const responseTimeout=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher(async()=>new Response(
  stalledResponseBody,{status:200,headers:{'content-type':'application/json'}},
 )),100);
 assert.equal(responseTimeout.status,504);
 assert.equal(responseBodyCancelled,true,'deadline cancels the active native response reader');
});

test('source-text gateway client cancellation stops an active native request and clears its deadline',async()=>{
 const clientController=new AbortController();
 const clientRequest=new Request('https://tos.test/api/source-text/search',{
  method:'POST',headers:{'content-type':'application/json','cf-access-jwt-assertion':validCallerAccessToken},body:'{"query":"α","limit":5}',signal:clientController.signal,
 });
 let fetchSignal,fetchCancelled=false,markFetchStarted;
 const fetchStarted=new Promise(resolve=>{markFetchStarted=resolve;});
 const responsePromise=sourceTextSearchGatewayResponse(clientRequest,gatewayEnv(),gatewayFetcher((_input,init)=>{
  fetchSignal=init.signal;
  markFetchStarted();
  return new Promise((_,reject)=>init.signal.addEventListener('abort',()=>{
   fetchCancelled=true;reject(init.signal.reason);
  },{once:true}));
 }),100);
 await new Promise((resolve,reject)=>{
  const timer=setTimeout(()=>reject(new Error('native fetch did not start')),1000);
  fetchStarted.then(()=>{clearTimeout(timer);resolve();});
 });
 clientController.abort();
 const cancelled=await responsePromise;
 assert.equal(cancelled.status,499);
 assert.equal(fetchSignal.aborted,true);
 assert.equal(fetchCancelled,true);

 let completedSignal;
 const completed=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher(async(_input,init)=>{
  completedSignal=init.signal;
  return new Response('{"hits":[]}',{headers:{'content-type':'application/json'}});
 }),100);
 assert.equal(completed.status,200);
 await new Promise(resolve=>setTimeout(resolve,120));
 assert.equal(completedSignal.aborted,false,'successful completion clears the deadline timer');
});

test('source-text gateway fails closed on partial, looping, oversized, and redirected routes',async()=>{
 const partial=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv({
  TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_SECRET:undefined,
 }),async()=>{throw new Error('must not fetch');});
 assert.equal(partial.status,503);

 const missingCaller=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv({
  TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_AUDIENCE:undefined,
 }),async()=>{throw new Error('must not fetch');});
 assert.equal(missingCaller.status,503);

 const invalidIssuer=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv({
  TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_ISSUER:'https://attacker.example/path',
 }),async()=>{throw new Error('must not fetch');});
 assert.equal(invalidIssuer.status,503);

 let called=false;
 const loop=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv({
  TOS_SOURCE_TEXT_GATEWAY_URL:'https://tos.test',
 }),async()=>{called=true;throw new Error('must not fetch');});
 assert.equal(loop.status,503);
 assert.equal(called,false);

 const pathBinding=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv({
  TOS_SOURCE_TEXT_GATEWAY_URL:'https://native-source.example.net/search?database=wrong',
 }),async()=>{called=true;throw new Error('must not fetch');});
 assert.equal(pathBinding.status,503);
 assert.equal(called,false);

 const oversized=await sourceTextSearchGatewayResponse(sourceTextRequest('x'.repeat(65*1024)),gatewayEnv(),gatewayFetcher(async()=>{
  throw new Error('must not fetch');
 }));
 assert.equal(oversized.status,413);

 const redirect=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher(async()=>new Response('redirect',{
  status:302,headers:{location:'https://elsewhere.example.net/api/source-text/search','content-type':'application/json'},
 })));
 assert.equal(redirect.status,502);
 assert.match((await redirect.json()).error,/invalid response/);

 const staleBody='{"error":"selected source-text consumer is stale"}';
 const stale=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher(async()=>new Response(staleBody,{
  status:503,headers:{'content-type':'application/json'},
 })));
 assert.equal(stale.status,503);
 assert.equal(await stale.text(),staleBody);

 const oversizedNative=await sourceTextSearchGatewayResponse(sourceTextRequest(),gatewayEnv(),gatewayFetcher(async()=>new Response(
  ' '.repeat(1024*1024+1),{status:200,headers:{'content-type':'application/json'}},
 )));
 assert.equal(oversizedNative.status,502);
});

function intersectionGrams(query){
 const needle=query.toLowerCase(),grams=[];
 for(let index=0;index<=needle.length-3;index++){
  const gram=needle.slice(index,index+3);if(!grams.includes(gram))grams.push(gram);
 }
 return grams;
}
function intersectionFixture(query='common alpha metadata'){
 const d=database(),grams=intersectionGrams(query),stats=[];let nextPosition=100000;
 const stat=gram=>d.sqlite.prepare('SELECT postings FROM knowledge_search_gram_stats WHERE kind=? AND n=3 AND gram=?').get('nodes',gram)?.postings??0;
 for(const [queryOrder,gram]of grams.entries()){
  const postings=d.sqlite.prepare('SELECT postings FROM knowledge_search_gram_stats WHERE kind=? AND n=3 AND gram=?').get('nodes',gram);
  assert.ok(postings,`missing fixture stats for ${JSON.stringify(gram)}`);
  stats.push({gram,queryOrder,postings:postings.postings});
 }
 const target=Math.max(...stats.map(item=>item.postings))+2;
 assert.ok(target*3<50000,'synthetic posting closure must stay below the shared candidate budget');
 const addFalsePositive=(gram,documentChars)=>{
  const position=nextPosition++,id=`philosophy:intersection-false-positive-${position}`;
  d.sqlite.prepare('INSERT INTO knowledge_search_documents VALUES (?,?,?,?,?,?,?,?,?,?,?,?)')
   .run('nodes',position,id,'philosophy','','',id.toLowerCase(),id.toLowerCase(),'[]','[]',documentChars,'unused');
  d.sqlite.prepare('INSERT INTO knowledge_search_grams VALUES (?,?,?,?)').run('nodes',3,gram,position);
  d.sqlite.prepare('UPDATE knowledge_search_gram_stats SET postings=postings+1 WHERE kind=? AND n=3 AND gram=?').run('nodes',gram);
 };
 const primary=stats.reduce((best,item)=>item.postings<best.postings?item:best,stats[0]);
 // One non-deliverable row makes the original single-posting preflight exceed
 // 16 MiB. Its id has no knowledge_nodes carrier, so it cannot be selected or
 // delivered; only the numeric document budget observes it.
 addFalsePositive(primary.gram,16_000_001);
 for(const item of stats)if(item.gram!==primary.gram)for(let count=stat(item.gram);count<target;count++)addFalsePositive(item.gram,0);
 const ordered=grams.map((gram,queryOrder)=>({gram,queryOrder,postings:stat(gram)}))
  .sort((left,right)=>left.postings-right.postings||left.queryOrder-right.queryOrder);
 assert.equal(ordered[0].gram,primary.gram,'synthetic primary gram must remain the rarest');
 return {d,query,primary:ordered[0],secondary:ordered[1]};
}
function prefixVerificationFixture(){
 const d=database(),query='metadata',grams=intersectionGrams(query),insert=d.sqlite.prepare('INSERT INTO knowledge_search_documents VALUES (?,?,?,?,?,?,?,?,?,?,?,?)');
 for(const gram of grams)assert.ok(d.sqlite.prepare('SELECT postings FROM knowledge_search_gram_stats WHERE kind=? AND n=3 AND gram=?').get('nodes',gram),`missing fixture stats for ${JSON.stringify(gram)}`);
 const source=JSON.parse(fixture.rows.nodes.find(row=>JSON.parse(row.raw).id==='philosophy:A').raw);
 source.display.title={default:'False positive'};source.display.provenance.summary='synthetic';source.display.summary_state='synthetic';
 const searchableDocument=raw=>reference("search/prefix-fixture",{raw});
 for(const [suffix,position] of [[1,100000],[2,100001],[3,100002]]){
  const wide=JSON.parse(JSON.stringify(source));wide.id=`philosophy:aa-prefix-false-positive-${suffix}`;wide.entity_id=wide.id;wide.native_id=`aa-prefix-false-positive-${suffix}`;wide.source_ref='test:prefix-false-positive';
  wide.attributes.match='metXetaXtadXadaXdatXata'+'x'.repeat(7_999_000);
  const falsePositive=searchableDocument(JSON.stringify(wide));
  falsePositive.document[1]=position;
  assert.equal(falsePositive.text.includes(query),false,'false-positive source carrier must not contain the complete query');
  assert.ok(grams.every(gram=>falsePositive.grams.includes(gram)),'false-positive source carrier must retain every query gram');
  assert.ok(falsePositive.document[10]>7_900_000,'false-positive source carrier must be wide');
  d.sqlite.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').run(...bindings('nodes',{raw:falsePositive.raw,text:falsePositive.text}));
  d.sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(`knowledge_node_digest:${wide.id}`,JSON.stringify({sha256:sha(falsePositive.raw)}));
  insert.run(...falsePositive.document);
  for(const gram of falsePositive.grams){
   d.sqlite.prepare('INSERT INTO knowledge_search_grams VALUES (?,?,?,?)').run('nodes',3,gram,position);
   d.sqlite.prepare('INSERT OR IGNORE INTO knowledge_search_gram_stats VALUES (?,?,?,?)').run('nodes',3,gram,0);
   d.sqlite.prepare('UPDATE knowledge_search_gram_stats SET postings=postings+1 WHERE kind=? AND n=3 AND gram=?').run('nodes',gram);
  }
 }
 // Every false-positive row is a real native source/search carrier: its
 // document_chars and digest come from its actual searchable text. Each is
 // large enough to fit alone but not two at once after rank metadata.
 return {d,query};
}
function numericPostingAggregate(d,kind,grams){
 const parts=['knowledge_search_grams g0','CROSS JOIN knowledge_search_documents s ON s.kind=g0.kind AND s.position=g0.position'];
 for(const [index,gram]of grams.slice(1).entries())parts.push(`CROSS JOIN knowledge_search_grams g${index+1} ON g${index+1}.kind=g0.kind AND g${index+1}.n=g0.n AND g${index+1}.position=g0.position AND g${index+1}.gram=?`);
 const sql='SELECT COUNT(*) AS candidate_rows,COALESCE(SUM(s.document_chars),0) AS document_chars FROM '+parts.join(' ')+' WHERE g0.kind=? AND g0.n=? AND g0.gram=?';
 return d.sqlite.prepare(sql).get(...grams.slice(1),kind,3,grams[0]);
}

test('indexed search intersects rare grams before verification without changing oracle paging',async()=>{
 const {d,query,primary,secondary}=intersectionFixture();
 try{
  const single=numericPostingAggregate(d,'nodes',[primary.gram]);
  const intersection=numericPostingAggregate(d,'nodes',[primary.gram,secondary.gram]);
  assert.ok(single.document_chars>16_000_000,JSON.stringify(single));
  assert.ok(intersection.document_chars<16_000_000,JSON.stringify(intersection));
  const expected=oracle('indexed',query,{limit:2});let cursor=null;
  for(const packet of expected){
   const result=await response(d,'indexed',query,{limit:'2',...(cursor?{cursor}:{})});
   const raw=await result.text();assert.equal(result.status,200,raw);assertPackets(raw,packet,true);cursor=JSON.parse(raw).page.next_cursor;
  }
  assert.equal(cursor,null);
  const preflight=d.statements.find(row=>row.args?.[0]==='nodes'&&row.sql.includes('invalid_budgets')&&row.sql.includes('intersection'));
  assert.ok(preflight,'indexed preflight must carry the bounded gram intersection');
  const plan=d.sqlite.prepare('EXPLAIN QUERY PLAN '+preflight.sql).all(...preflight.args).map(row=>row.detail);
  assert.ok(plan.some(detail=>/SEARCH intersection(?: EXISTS)? USING COVERING INDEX sqlite_autoindex_knowledge_search_grams_1/.test(detail)&&/position=\?/.test(detail)),JSON.stringify(plan));
 }finally{d.close();}
});

test('indexed search rejects a corrupt selected secondary posting closure instead of returning empty',async()=>{
 const {d,query,secondary}=intersectionFixture();
 try{
  d.sqlite.prepare('UPDATE knowledge_search_gram_stats SET postings=postings-1 WHERE kind=? AND n=3 AND gram=?').run('nodes',secondary.gram);
  const result=await response(d,'indexed',query,{limit:'2'}),raw=await result.text();
  assert.equal(result.status,503,raw);assert.match(raw,/posting closure is incomplete/);
 }finally{d.close();}
});

test('indexed prefix verification preserves exact rows across bounded false-positive progress pages',async()=>{
 const {d,query}=prefixVerificationFixture();
 try{
  const expectedPackets=oracle('indexed',query,{limit:1}).map(raw=>JSON.parse(raw));
  const expected={nodes:expectedPackets.flatMap(packet=>packet.nodes),relations:expectedPackets.flatMap(packet=>packet.relations)};
  const actual={nodes:[],relations:[]},pages=[];let cursor=null,emptyProgress=false;
  for(let pageNumber=0;pageNumber<100;pageNumber++){
   const result=await response(d,'indexed',query,{limit:'1',...(cursor?{cursor}:{})}),raw=await result.text();
   assert.equal(result.status,200,raw);
   const packet=JSON.parse(raw),rows=[...packet.nodes,...packet.relations];pages.push(packet);actual.nodes.push(...packet.nodes);actual.relations.push(...packet.relations);
   assert.ok(packet.work.nodes.verified_chars+ (packet.work.nodes.rank_chars??0)<=16_000_000,JSON.stringify(packet.work));
   assert.ok(packet.work.relations.verified_chars+ (packet.work.relations.rank_chars??0)<=16_000_000,JSON.stringify(packet.work));
   if(rows.length===0){assert.ok(packet.page.next_cursor,`empty page ${pageNumber} must make cursor progress`);emptyProgress=true;}
   const next=packet.page.next_cursor;
   if(next===null){cursor=null;break;}
   assert.notEqual(next,cursor,'each bounded prefix must advance the opaque cursor');cursor=next;
  }
  assert.equal(cursor,null);assert.ok(pages.length>=3,`expected multiple bounded prefixes, got ${pages.length}`);assert.equal(emptyProgress,true);
  assert.deepEqual(actual,expected);for(const rows of Object.values(actual))assert.equal(new Set(rows.map(row=>row.id)).size,rows.length);
  const aggregate=d.statements.find(row=>row.args?.[0]==='nodes'&&row.sql.includes('AS verified_chars'))?.results[0];
  assert.ok(aggregate&&aggregate.verified_chars>16_000_000,JSON.stringify(aggregate));
  assert.ok(aggregate.rank_chars<16_000_000,JSON.stringify(aggregate));
  assert.ok(d.statements.some(row=>row.sql.includes('selected_candidates AS MATERIALIZED')),'delivery must stay behind the verified prefix');
  const decode=value=>JSON.parse(Buffer.from(value,'base64url').toString('utf8')),encode=value=>Buffer.from(JSON.stringify(value)).toString('base64url');
  const seed=pages.find(packet=>packet.page.next_cursor)?.page.next_cursor;assert.ok(seed,'fixture must expose an outer continuation cursor');
  const outer=decode(seed),beyondOuter={...outer};
  for(const kind of ['nodes','relations']){
   if(typeof beyondOuter[kind]==='string'){beyondOuter[kind]=encode({...decode(beyondOuter[kind]),rank:3,id:'zzzz',position:Number.MAX_SAFE_INTEGER});beyondOuter[`${kind}_exhausted`]=false;}
   else {assert.equal(beyondOuter[kind],null);beyondOuter[`${kind}_exhausted`]=true;}
  }
  const beyond=encode(beyondOuter);
  const eof=await response(d,'indexed',query,{limit:'1',cursor:beyond}),eofRaw=await eof.text();assert.equal(eof.status,200,eofRaw);
  const eofPacket=JSON.parse(eofRaw);assert.deepEqual(eofPacket.nodes,[]);assert.deepEqual(eofPacket.relations,[]);assert.equal(eofPacket.page.next_cursor,null);
 }finally{d.close();}
});

test('indexed prefix verification rejects one oversized remaining document with 413',async()=>{
 const d=database();
 try{
  d.sqlite.prepare("UPDATE knowledge_search_documents SET document_chars=? WHERE kind='nodes' AND id=?").run(16_000_001,'philosophy:scalar');
  const result=await response(d,'indexed','scalar',{limit:'1'}),raw=await result.text();
  assert.equal(result.status,413,raw);assert.match(raw,/first remaining document exceeds verification budget/);
  const aggregate=d.statements.find(row=>row.args?.[0]==='nodes'&&row.sql.includes('AS verified_chars'))?.results[0];
  assert.ok(aggregate&&aggregate.verified_chars>16_000_000,JSON.stringify(aggregate));
  assert.ok(aggregate.rank_chars<16_000_000,JSON.stringify(aggregate));
 }finally{d.close();}
});

test('selected search delivery drives exact address seeks even with publisher identity indexes',async()=>{
 const d=database();try{
  d.sqlite.exec('CREATE UNIQUE INDEX knowledge_search_address_id_idx ON knowledge_search_documents(kind,id)');
  d.sqlite.exec('CREATE INDEX knowledge_search_address_tie_idx ON knowledge_search_documents(kind,id_lower,position)');
  const insert=d.sqlite.prepare('INSERT INTO knowledge_search_documents VALUES (?,?,?,?,?,?,?,?,?,?,?,?)');
  d.sqlite.exec('BEGIN');
  for(let i=0;i<10000;i++)insert.run('nodes',100000+i,'unrelated-'+i,'philosophy','','','unrelated-'+i,'unrelated-'+i,'[]','[]',0,'unused');
  d.sqlite.exec('COMMIT');
  const raw=await direct(d,'indexed','αβγ',{limit:1});
  assertPackets(raw,oracle('indexed','αβγ',{limit:1})[0],true);
  const deliveries=d.statements.filter(row=>row.sql.includes('FROM json_each(?) wanted')&&row.sql.includes('knowledge_search_documents s'));
  assert.ok(deliveries.length>0);
  for(const {sql,args}of deliveries){
   const plan=d.sqlite.prepare('EXPLAIN QUERY PLAN '+sql).all(...args).map(row=>row.detail);
   assert.ok(plan.some(detail=>/SCAN wanted VIRTUAL TABLE/.test(detail)),JSON.stringify(plan));
   assert.ok(plan.some(detail=>/SEARCH s .*\(kind=\? AND position=\?\)/.test(detail)),JSON.stringify(plan));
   assert.equal(plan.some(detail=>/SCAN s\b|SEARCH s .*\(kind=\?\)$/.test(detail)),false);
  }
 }finally{d.close();}
});

test('indexed candidate budgets and ranking start from the rare gram before source filters',async()=>{
 const d=database();try{
  d.sqlite.exec('CREATE INDEX search_filter_plan ON knowledge_search_documents(kind,source_graph,kind_id,position)');
  for(const sources of [null,['philosophy']]){
   const raw=await direct(d,'indexed','αβγ',{limit:1,sources});
   assertPackets(raw,oracle('indexed','αβγ',{limit:1,...(sources?{sources}:{})})[0],true);
  }
  const selections=d.statements.filter(row=>row.sql.includes('FROM knowledge_search_grams g')&&
   (row.sql.includes('invalid_budgets')||row.sql.includes('search_rank')));
  assert.ok(selections.length>=2);
  for(const {sql,args}of selections){
   const plan=d.sqlite.prepare('EXPLAIN QUERY PLAN '+sql).all(...args).map(row=>row.detail);
   assert.ok(plan.some(detail=>/SEARCH g .*\(kind=\? AND n=\? AND gram=\?\)/.test(detail)),JSON.stringify(plan));
   assert.ok(plan.some(detail=>/SEARCH s .*\(kind=\? AND position=\?\)/.test(detail)),JSON.stringify(plan));
   assert.equal(plan.some(detail=>/SCAN s\b|SEARCH s .*\(kind=\? AND source_graph=\?/.test(detail)),false);
  }
 }finally{d.close();}
});

test('addressed case-tie relocation preserves Worker search and continuation source order',async()=>{
 const change=reference("search/tie-relocation",{graph:fixture.graph,documents:fixture.documents});
 const d=database();try{
  const unrelated=d.sqlite.prepare("SELECT * FROM knowledge_search_grams WHERE kind='nodes' AND position NOT IN (?,?) ORDER BY gram,position")
   .all(...Object.values(change.plan.before));
  d.sqlite.exec('BEGIN');
  for(const id of change.plan.changed_ids){const before=change.plan.before[id],after=change.plan.after[id];
   d.sqlite.prepare('UPDATE knowledge_search_documents SET position=? WHERE kind=? AND position=?').run(after,'nodes',before);
   d.sqlite.prepare('UPDATE knowledge_search_grams SET position=? WHERE kind=? AND position=?').run(after,'nodes',before);
  }
  d.sqlite.exec('COMMIT');
  assert.deepEqual(d.sqlite.prepare("SELECT * FROM knowledge_search_grams WHERE kind='nodes' AND position NOT IN (?,?) ORDER BY gram,position")
   .all(...Object.values(change.plan.after)),unrelated);
  assertPackets(await direct(d,'legacy','alpha'),oracle('legacy','alpha',{limit:100},change.graph)[0]);
  const expected=oracle('indexed','alpha',{limit:1},change.graph);let cursor=null;
  for(const packet of expected){const responseValue=await response(d,'indexed','alpha',{limit:'1',...(cursor?{cursor}:{})});
   assert.equal(responseValue.status,200);const raw=await responseValue.text();assertPackets(raw,packet,true);cursor=JSON.parse(raw).page.next_cursor;}
  assert.equal(cursor,null);
 }finally{d.close();}
});

for(const version of [8,9])test(`v${version} full search rows retain Python kinds, ordered keys, metadata and Unicode rank`,async()=>{
 const d=database(version);try{
  for(const query of ['', 'αβγ','\u0085ΑΒΓ\u0085','\ufeffαβγ','scalar','straße','STRASSE','école','e\u0301cole','😀'.repeat(256)]){
   const raw=await direct(d,'legacy',query);assertPackets(raw,oracle('legacy',query,{limit:100})[0]);
  }
  assert.equal(d.statements.some(row=>row.sql.includes('knowledge_lens_top')),false);
  assert.equal(d.statements.some(row=>row.sql.includes('SELECT json FROM knowledge_')),false);
 }finally{d.close();}
});
test('indexed native first and continuation packets match captured core/read-model packets without restarting exhausted kinds',async()=>{
 const d=database();try{
  for(const query of ['alpha','αβγ','\u0085ΑΒΓ\u0085','straße','STRASSE','😀'.repeat(256)]){
   const expected=oracle('indexed',query,{limit:2});let cursor=null;
   for(const page of expected){const raw=await direct(d,'indexed',query,{limit:2,cursor});assertPackets(raw,page,true);assert.equal(await direct(d,'indexed',query,{limit:2,cursor}),raw);cursor=JSON.parse(raw).page.next_cursor;}
   assert.equal(cursor,null);
  }
 }finally{d.close();}
});
test('actual HTTP GET/HEAD serialize full native source only once and replay identical queries',async()=>{
 const d=database();try{for(const mode of ['legacy','indexed']){
  const first=await response(d,mode,'αβγ',{limit:'2'});assert.equal(first.status,200);const raw=await first.text();assertPackets(raw,oracle(mode,'αβγ',{limit:2})[0],mode==='indexed');
  assert.equal(await (await response(d,mode,'αβγ',{limit:'2'})).text(),raw);
  const head=await response(d,mode,'αβγ',{limit:'2'},'HEAD');assert.equal(head.status,200);assert.equal(await head.text(),'');
 }}finally{d.close();}
});

test('Python mode-specific whitespace, code-point lengths and bounded filters fail before selected delivery',async()=>{
 const d=database();try{
  for(const [mode,query,status]of [['legacy',' '+ 'x'.repeat(256)+' ',200],['indexed',' '+'x'.repeat(256)+' ',400],
   ['legacy','İ'.repeat(129),200],['indexed','İ'.repeat(129),400],['legacy','a',200],['indexed','a',400],
   ['legacy','x'.repeat(257),400],['indexed','😀'.repeat(257),400]]){
   const start=d.statements.length;assert.equal((await response(d,mode,query)).status,status);
   if(status===400)assert.equal(d.statements.slice(start).some(row=>row.sql.includes('json_bytes')),false);
  }
  for(const mode of ['legacy','indexed']){
   assert.equal((await response(d,mode,'alpha',{sources:'unknown'})).status,400);
   assert.equal((await response(d,mode,'alpha',{sources:'\u0085philosophy\u0085'})).status,400);
   assert.equal((await response(d,mode,'alpha',{sources:'\ufeffphilosophy'})).status,400);
  }
  assert.equal((await response(d,'indexed','alpha',{kind_ids:'x'.repeat(257)})).status,400);
 }finally{d.close();}
});
test('search HTTP numeric parsing follows Python whole integers, Unicode decimal digits and default/clamp',async()=>{
 const d=database();try{
  const values=['1','1.0','1suffix','1_0','١٢','\u00852\u0085','\u001c2','\ufeff2','-100','999999999999999999999','9'.repeat(4301)];
  const expected=reference("search/http-integers",values);
  for(const mode of ['legacy','indexed'])for(const [at,value]of values.entries()){
   const result=await response(d,mode,'alpha',{limit:value});assert.equal(result.status,200);assert.equal((await result.json()).page.limit_per_kind,expected[at],mode+': '+value.slice(0,20));
  }
 }finally{d.close();}
});
test('search HTTP blank/repeated parameters follow Python parse_qs before first-value selection',async()=>{
 const d=database();try{
  const result=await (await worker()).fetch(new Request('https://tos.test/api/knowledge/search?mode=&mode=indexed&query=&query=alpha&limit=&limit=1&cursor='),{DB:d.db,ASSETS:{fetch(){throw new Error('no fallback');}}},{});
  assert.equal(result.status,200);assertPackets(await result.text(),oracle('indexed','alpha',{limit:1})[0],true);
 }finally{d.close();}
});
test('private native cursor migration, integer kinds, filters and epochs never silently coerce or restart',async()=>{
 const d=database();try{
  const first=JSON.parse(await direct(d,'indexed','alpha',{limit:1})),cursor=first.page.next_cursor;
  await assert.rejects(()=>knowledgeSearchD1Indexed(d.db,options('alpha',{cursor:''}),publishedFixtureSearchControlsRuntime),error=>error.status===400);
  const decode=value=>JSON.parse(Buffer.from(value,'base64url').toString('utf8')),encode=value=>Buffer.from(JSON.stringify(value)).toString('base64url');
  const outer=decode(cursor);
  const reordered={...outer,filters:{predicate_ids:outer.filters.predicate_ids,kind_ids:outer.filters.kind_ids,sources:outer.filters.sources}};
  assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:encode(reordered)})).status,200);
  const duplicate=Buffer.from('{"schema":"superseded",'+JSON.stringify(outer).slice(1)).toString('base64url');
  assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:duplicate})).status,200);
  for(const [schema,status]of [['tos_knowledge_search_indexed_cursor_v2',409],['tos_knowledge_search_indexed_cursor_v1',400],['unknown',400]]){
   assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:encode({...outer,schema})})).status,status);
  }
  const floatOuter=Buffer.from(JSON.stringify(outer).replace(/"snapshot_epoch":(\d+)/,'"snapshot_epoch":$1.0')).toString('base64url');
  assert.equal((await response(d,'indexed','alpha',{cursor:floatOuter})).status,400);
  const innerRaw=Buffer.from(outer.nodes,'base64url').toString('utf8').replace(/"rank":(\d+)/,'"rank":$1.0');
  assert.equal((await response(d,'indexed','alpha',{cursor:encode({...outer,nodes:Buffer.from(innerRaw).toString('base64url')})})).status,400);
  assert.equal((await response(d,'indexed','alpha',{cursor,kind_ids:'different'})).status,409);
  assert.equal((await response(d,'indexed','alpha',{cursor:'not-a-cursor'})).status,400);
  d.sqlite.exec('UPDATE knowledge_exploration_clock SET epoch=epoch+2');
  assert.equal((await response(d,'indexed','alpha',{cursor})).status,409);
 }finally{d.close();}
});
test('generated continuation fits its own private decode budget or refuses before returning a page',async()=>{
 const d=database();try{
  const kind=JSON.parse(fixture.rows.nodes[0].raw).kind_id;
  const filters=[kind,...Array.from({length:40},(_,at)=>String(at)+'x'.repeat(100))].join(',');
  const result=await response(d,'indexed','alpha',{limit:'1',kind_ids:filters});assert.equal(result.status,413);assert.match(await result.text(),/cursor byte budget/);
 }finally{d.close();}
});
test('missing or damaged required source/header/search carriers fail closed without fallback',async()=>{
 const changes=[
  "DELETE FROM edge_meta WHERE key='knowledge_reader_top'",
  "DELETE FROM edge_meta WHERE key='knowledge_node_digest:philosophy:z'",
  "UPDATE knowledge_nodes SET json=replace(json,'9007199254740993','9007199254740994') WHERE id='philosophy:z'",
  "UPDATE knowledge_nodes SET native_id='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET document_digest='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET visible_values='[]' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET identity_values='not-json' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET document_chars='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET id_lower='wrong' WHERE id='philosophy:z'",
  "DROP TABLE knowledge_search_documents",
 ];
 for(const mode of ['legacy','indexed'])for(const sql of changes){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,mode,'philosophy:z')).status,503,mode+': '+sql);}finally{d.close();}}
 const d=database();try{d.sqlite.exec("DELETE FROM knowledge_search_documents WHERE kind='nodes'");assert.equal((await response(d,'legacy','')).status,503);}finally{d.close();}
 for(const sql of ["DELETE FROM knowledge_search_documents WHERE kind='nodes'","DELETE FROM knowledge_search_gram_stats WHERE kind='nodes'"]){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,'indexed','αβγ')).status,503);}finally{d.close();}}
});
test('row/header byte limits and indexed preflight budgets reject before oversize text/rank evaluation',async()=>{
 for(const mode of ['legacy','indexed'])for(const kind of ['row','header']){const d=database();try{
  const oversized='x'.repeat(1048577);
  if(kind==='row')d.sqlite.prepare("UPDATE knowledge_nodes SET json=? WHERE id='philosophy:z'").run(oversized);
  else d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(oversized);
  assert.equal((await response(d,mode,'philosophy:z')).status,413);
  assert.equal(d.statements.some(entry=>entry.results.some(row=>Object.values(row).some(value=>typeof value==='string'&&Buffer.byteLength(value)>1048576))),false);
 }finally{d.close();}}
 for(const [sql,status]of [
  ["UPDATE knowledge_search_documents SET document_chars=16000001,identity_values='not-json' WHERE id='philosophy:z'",413],
  ["UPDATE knowledge_search_gram_stats SET postings=50001",413],
  ["UPDATE knowledge_search_gram_stats SET postings=-1",503],
  ["UPDATE knowledge_search_gram_stats SET postings=zeroblob(1048577) WHERE kind='nodes' AND gram='phi'",503],
  ["UPDATE knowledge_search_documents SET document_chars=-1 WHERE id='philosophy:z'",503],
 ]){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,'indexed','philosophy:z')).status,status);}finally{d.close();}}
});
test('retained invalid Unicode strings and keys refuse the Python UTF-8 response boundary',async()=>{
 for(const mode of ['legacy','indexed'])for(const key of [false,true]){const d=database();try{
  const row=d.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get();
  const malformed=key?'"\\ud800":"bad",':'"bad":"\\ud800",';
  d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(row.json_chunk.replace('"unknown":',malformed+'"unknown":'));
  const result=await response(d,mode,'alpha');assert.equal(result.status,503);assert.match(await result.text(),/invalid Unicode/);
 }finally{d.close();}}
 assert.equal(reference("search/invalid-utf8"),true);
});
test('aggregate selected ID budget masks oversized headers in SQL before source bodies are read',async()=>{
 const d=database();try{
  for(const [index,row]of fixture.rows.nodes.entries()){
   const id=JSON.parse(row.raw).id,big=String(index).padStart(2,'0')+'x'.repeat(900000);
   d.sqlite.prepare('UPDATE knowledge_nodes SET id=? WHERE id=?').run(big,id);
   d.sqlite.prepare('UPDATE knowledge_search_documents SET id=?,id_lower=? WHERE id=?').run(big,big,id);
  }
  assert.equal((await response(d,'legacy','',{limit:'100'})).status,413);
  assert.equal(d.statements.some(row=>row.sql.includes('json_bytes')&&row.sql.includes('FROM knowledge_nodes')),false);
  const selection=d.statements.find(row=>row.sql.includes('AS _native_valid FROM selected'));
  assert.ok(selection);assert.ok(selection.results.some(row=>row._native_bytes>16*1024*1024&&row.id===null&&row.id_lower===null));
 }finally{d.close();}
});
test('publication A-to-B-to-A during either search mode is rejected at final read boundary',async()=>{
 for(const mode of ['legacy','indexed']){const d=database();try{
  let changed=false;d.hook.after=(sql)=>{if(!changed&&sql.includes('AS _native_valid FROM selected')){changed=true;
   d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'b'.repeat(64)}));
   d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'d'.repeat(64)}));
  }};
  assert.equal((await response(d,mode,'αβγ')).status,409);assert.equal(changed,true);
 }finally{d.close();}}
});
test('selection plans retain linear legacy carrier scan plus primary-key lookup and indexed posting seeks',async()=>{
 const d=database();try{
  await direct(d,'legacy','alpha');await direct(d,'indexed','alpha');
  const plans=d.statements.filter(row=>row.sql.includes('AS _native_valid FROM selected')).map(row=>d.sqlite.prepare('EXPLAIN QUERY PLAN '+row.sql).all(...row.args).map(step=>step.detail));
  assert.equal(plans.length,4);
  for(const [index,plan]of plans.entries()){
   assert.ok(plan.some(line=>/SEARCH (n|r|b) USING INDEX sqlite_autoindex_knowledge_(nodes|relations)_1 \(id=\?\)/.test(line)),JSON.stringify(plan));
   if(index<2)assert.ok(plan.some(line=>/SEARCH s USING INDEX sqlite_autoindex_knowledge_search_documents_1 \(kind=\?\)/.test(line)),JSON.stringify(plan));
   else assert.ok(plan.some(line=>/SEARCH g USING COVERING INDEX sqlite_autoindex_knowledge_search_grams_1/.test(line)),JSON.stringify(plan));
  }
 }finally{d.close();}
});
test('real D1 Worker first/continuation native bytes survive isolate restart without cache or schema writes',async()=>{
 const workerModules=await publishedWorkerFixtureModules();
 const directory=mkdtempSync(join(tmpdir(),'tos-native-search-d1-'));let firstRaw,cursor;
 const expected=oracle('indexed','αβγ',{limit:1});
 try{for(let launch=0;launch<2;launch++){
  const mf=new Miniflare(convertV4MiniflareOptions({...workerModules,d1Databases:['DB'],resourcePersistencePath:directory}));
  try{const db=await mf.getD1Database('DB');
   if(launch===0){const statements=[...schema.split(';').filter(value=>value.trim()).map(sql=>db.prepare(sql)),
    ...migration.replace(/^--.*$/gm,'').trim().split(/\n(?=CREATE |INSERT )/).map(sql=>db.prepare(sql)),
    ...Object.entries(fixture.metadata[9]).map(([key,raw])=>db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw))];
    for(const kind of ['nodes','relations'])for(const row of fixture.rows[kind]){const args=bindings(kind,row);statements.push(db.prepare(`INSERT INTO knowledge_${kind} VALUES (${args.map(()=>'?').join(',')})`).bind(...args));}
    for(const [table,key]of [['knowledge_search_documents','documents'],['knowledge_search_grams','grams'],['knowledge_search_gram_stats','stats']]){
     const values=fixture[key],size=Math.floor(96/values[0].length);
     for(let at=0;at<values.length;at+=size){const page=values.slice(at,at+size);statements.push(db.prepare(`INSERT INTO ${table} VALUES `+page.map(args=>'('+args.map(()=>'?').join(',')+')').join(',')).bind(...page.flat()));}
    }
    for(let at=0;at<statements.length;at+=128)await db.batch(statements.slice(at,at+128));
   }
   const first=await mf.dispatchFetch('https://tos.test/api/knowledge/search?mode=indexed&query='+encodeURIComponent('αβγ')+'&limit=1');const raw=await first.text();assert.equal(first.status,200,raw);assertPackets(raw,expected[0],true);
   if(launch===0){firstRaw=raw;cursor=JSON.parse(raw).page.next_cursor;}else assert.equal(raw,firstRaw);
   const next=await mf.dispatchFetch('https://tos.test/api/knowledge/search?mode=indexed&query='+encodeURIComponent('αβγ')+'&limit=1&cursor='+encodeURIComponent(cursor));const nextRaw=await next.text();assert.equal(next.status,200,nextRaw);assertPackets(nextRaw,expected[1],true);
  }finally{await mf.dispose();}
 }}finally{rmSync(directory,{recursive:true,force:true});}
});
