import {test} from 'vitest';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {BUDGET,ContractError} from './knowledge-client.mjs';
import {constructorCatalog,createConstructorCatalogLoader,initialDraft,compileDraft,encodeDraft,decodeDraft,summarizeLens,previewDraft,draftForPacket,readSaved,saveDraft,SAVED_LENSES_KEY} from './lens-model.mjs';

const schema=JSON.parse(readFileSync(new URL('../../../contracts/lens-spec.v1.schema.json',import.meta.url),'utf8'));
const boundary={is_source:false,is_canon:false,writes_to_tree:false};
const catalog={schema:'tos_knowledge_catalog_v1',source_revision:'a'.repeat(64),authority_boundary:boundary,
  node_kinds:[{kind_id:'work',display:{ru:'Произведение'}},{kind_id:'agent',display:{ru:'Действующее лицо'}}],predicates:[{predicate_id:'authored-by',display:{ru:'Автор'}}],
  capabilities:{sources:['philosophy','source-navigation'],filter_operators:['in'],node_fields:['kind_id'],relation_fields:['predicate_id'],
    maximums:{nodes:1000,relations:2000,groups:200,traversal_depth:5},neighborhood_profiles:[{profile:'all'},{profile:'overview'}],inclusion:{authority:'query-execution-not-semantic-proof'}}};
const context={catalog,schema},bundle={schema:'tos_knowledge_contract_bundle_v1',contracts:{lens_spec:schema},authority_boundary:boundary};
const raw={id:'source-navigation:opaque:work/с пробелом',content_revision:'b'.repeat(64),source_refs:['ToS/owned.json'],display:{title:{ru:'Произведение'}}};
const original={nodes:[raw],focus:{node_id:raw.id}};
const draft=()=>initialDraft(original,context);
const packet=()=>({schema:'tos_lens_result_v1',source_revision:catalog.source_revision,fingerprint:'c'.repeat(64),authority_boundary:boundary,nodes:[raw],relations:[],focus:null,
  counts:{nodes:1,relations:0,matched_nodes:3,eligible_relations:0,truncated_nodes:2,truncated_relations:0},inclusion:{authority:'query-execution-not-semantic-proof',nodes:{[raw.id]:{kind:'selector'}}}});
const freeze=value=>{if(value&&typeof value==='object'&&!Object.isFrozen(value)){Object.values(value).forEach(freeze);Object.freeze(value);}return value;};
const frozenCatalog=(source_revision=catalog.source_revision)=>freeze({...structuredClone(catalog),source_revision});

test('constructor reads both advertised vocabulary and the executable schema',async()=>{
  const paths=[];const result=await constructorCatalog({request:async path=>{paths.push(path);return path==='/catalog'?catalog:bundle;}});
  assert.deepEqual(paths.sort(),['/catalog','/contracts']);assert.equal(result.catalog,catalog);
  const supplied=frozenCatalog(),suppliedPaths=[];
  const suppliedResult=await constructorCatalog({request:async path=>{suppliedPaths.push(path);return bundle;}},undefined,{catalog:supplied});
  assert.deepEqual(suppliedPaths,['/contracts']);assert.equal(suppliedResult.catalog,supplied);
  const incompatible=structuredClone(bundle);incompatible.contracts.lens_spec.properties.schema_version.const='future';
  await assert.rejects(()=>constructorCatalog({request:async path=>path==='/catalog'?catalog:incompatible}),ContractError);
});

test('catalog loader reuses only the same frozen catalog identity and replaces revisions',async()=>{
  let supplied=frozenCatalog(),calls=[];
  const loader=createConstructorCatalogLoader({request:async path=>{calls.push(path);assert.equal(path,'/contracts');return bundle;}},()=>supplied);
  const first=await loader.load();assert.equal(first.catalog,supplied);
  assert.equal(await loader.load(),first);assert.deepEqual(calls,['/contracts']);
  supplied=frozenCatalog();const sameRevision=await loader.load();assert.notEqual(sameRevision,first);assert.deepEqual(calls,['/contracts','/contracts']);
  supplied=frozenCatalog('f'.repeat(64));const newRevision=await loader.load();assert.notEqual(newRevision.catalog.source_revision,sameRevision.catalog.source_revision);assert.equal(calls.length,3);
});

test('catalog loader preserves the uncached no-supplied and mutable paths',async()=>{
  const paths=[];
  const client={request:async path=>{paths.push(path);return path==='/catalog'?catalog:bundle;}};
  const noCatalog=createConstructorCatalogLoader(client,()=>null);await noCatalog.load();await noCatalog.load();
  assert.deepEqual(paths,['/catalog','/contracts','/catalog','/contracts']);
  paths.length=0;const mutable=structuredClone(catalog),mutableLoader=createConstructorCatalogLoader({request:async path=>{paths.push(path);return bundle;}},()=>mutable);
  await mutableLoader.load();await mutableLoader.load();assert.deepEqual(paths,['/contracts','/contracts']);
});

test('catalog loader never caches aborted, invalid or cleared in-flight responses',async()=>{
  let resolveContracts,calls=0;const response=new Promise(resolve=>{resolveContracts=resolve;});
  const supplied=frozenCatalog(),client={request:async path=>{assert.equal(path,'/contracts');calls++;return calls===1?response:bundle;}};
  const loader=createConstructorCatalogLoader(client,()=>supplied),controller=new AbortController();
  const cancelled=loader.load(controller.signal);controller.abort();resolveContracts(bundle);
  await assert.rejects(cancelled,error=>error?.name==='AbortError');await loader.load();assert.equal(calls,2);

  const bad=structuredClone(bundle);bad.contracts.lens_spec.properties.schema_version.const='future';let valid=false;
  const retryLoader=createConstructorCatalogLoader({request:async path=>{assert.equal(path,'/contracts');return valid?bundle:bad;}},()=>supplied);
  await assert.rejects(retryLoader.load(),ContractError);valid=true;await retryLoader.load();

  let resolveCleared;const clearedResponse=new Promise(resolve=>{resolveCleared=resolve;});let clearedCalls=0;
  const clearLoader=createConstructorCatalogLoader({request:async path=>{assert.equal(path,'/contracts');clearedCalls++;return clearedCalls===1?clearedResponse:bundle;}},()=>supplied);
  const inFlight=clearLoader.load();clearLoader.clear();resolveCleared(bundle);assert.equal(await inFlight,null);await clearLoader.load();assert.equal(clearedCalls,2);
});

test('a superseded catalog load cannot return or overwrite the newer context',async()=>{
  const supplied=frozenCatalog();let resolveFirst,calls=0;
  const firstResponse=new Promise(resolve=>{resolveFirst=resolve;});
  const loader=createConstructorCatalogLoader({request:async()=>++calls===1?firstResponse:bundle},()=>supplied);
  const pending=loader.load(),current=await loader.load();resolveFirst(bundle);
  assert.equal(await pending,null);assert.equal(await loader.load(),current);assert.equal(calls,2);
});

test('a failed refresh discards the previous cached context so the next open retries',async()=>{
  const supplied=frozenCatalog(),bad=structuredClone(bundle);bad.contracts.lens_spec.properties.schema_version.const='future';
  let valid=true,calls=0;const loader=createConstructorCatalogLoader({request:async path=>{assert.equal(path,'/contracts');calls++;return valid?bundle:bad;}},()=>supplied);
  const first=await loader.load();valid=false;await assert.rejects(loader.load(undefined,{refresh:true}),ContractError);
  valid=true;const retried=await loader.load();assert.notEqual(retried,first);assert.equal(calls,3);assert.equal(Object.isFrozen(retried),true);assert.equal(Object.isFrozen(retried.schema.properties.schema_version),true);
});

test('a bound catalog replacement during fetch returns no stale context and does not cache it',async()=>{
  let supplied=frozenCatalog(),resolveContracts,calls=0;const replacement=frozenCatalog('f'.repeat(64));
  const response=new Promise(resolve=>{resolveContracts=resolve;});
  const loader=createConstructorCatalogLoader({request:async path=>{assert.equal(path,'/contracts');calls++;return calls===1?response:bundle;}},()=>supplied);
  const pending=loader.load();supplied=replacement;resolveContracts(bundle);assert.equal(await pending,null);
  const current=await loader.load();assert.equal(current.catalog,replacement);assert.equal(calls,2);
});

test('catalog loader refresh revalidates contracts and bound disappearance drops the cache',async()=>{
  let supplied=frozenCatalog(),calls=[];
  const loader=createConstructorCatalogLoader({request:async path=>{calls.push(path);return path==='/catalog'?catalog:bundle;}},()=>supplied);
  const first=await loader.load();await loader.load(undefined,{refresh:true});assert.deepEqual(calls,['/contracts','/contracts']);
  supplied=null;await loader.load();assert.deepEqual(calls,['/contracts','/contracts','/catalog','/contracts']);
  supplied=frozenCatalog();await loader.load();assert.equal(calls.length,5);
  assert.equal(first.catalog.source_revision,catalog.source_revision);
});
test('area filtering cannot silently become an unbounded global selector; opaque IDs survive',()=>{
  const value=draft();value.kinds=['work'];value.query='Ницше';
  const spec=compileDraft(value,context);
  assert.deepEqual(spec.seed.node_ids,[raw.id]);assert.equal(spec.seed.text_query,'Ницше');
  assert.deepEqual(spec.node_query.filters,[{field:'kind_id',op:'in',value:['work']}]);
  assert.throws(()=>compileDraft({...value,nodeIds:[]},context),ContractError);
  const global=compileDraft({...value,scope:'all'},context);assert.equal(global.seed.node_ids,undefined);
  assert.equal(original.nodes[0],raw);
});
test('focus excludes dormant root filters while relation filtering and direction stay explicit',()=>{
  const value={...draft(),scope:'focus',kinds:['work'],query:'ignored root query',predicates:['authored-by'],depth:2,direction:'incoming'};
  const spec=compileDraft(value,context);
  assert.deepEqual(spec.seed,{focus_node_id:raw.id});assert.equal(spec.node_query.enabled,false);assert.deepEqual(spec.node_query.filters,[]);
  assert.deepEqual(spec.relation_query.filters,[{field:'predicate_id',op:'in',value:['authored-by']}]);assert.equal(spec.traversal.direction,'incoming');
  assert.throws(()=>compileDraft({...value,focusId:null},context),ContractError);
});
test('all builder routes keep the scene budget and reject vanished vocabulary',()=>{
  for(const scope of ['area','focus','all'])for(const limit of [10,20,40])for(const depth of [0,1,3]){
    const spec=compileDraft({...draft(),scope,limit,depth},context);
    assert.ok(spec.limits.nodes<=BUDGET.nodes);assert.ok(spec.limits.relations<=BUDGET.relations);assert.equal(spec.detail,'compact');assert.equal(spec.composition.endpoint_policy,'both');
  }
  for(const patch of [{sources:[]},{sources:['invented']},{kinds:['missing']},{predicates:['missing']},{limit:41},{depth:4}])assert.throws(()=>compileDraft({...draft(),...patch},context),ContractError);
  assert.equal(compileDraft({...draft(),relations:false},context).limits.relations,0);
});
test('links round-trip the owned definition and reject malformed or excessive carriers',()=>{
  const value={...draft(),name:'Линза «источники»'};
  assert.deepEqual(decodeDraft(encodeDraft(value)),value);
  for(const text of ['{','null',JSON.stringify({...value,v:3}),JSON.stringify({...value,nodeIds:[raw.id,raw.id]}),'x'.repeat(12001)])assert.throws(()=>decodeDraft(text),ContractError);
  assert.equal(decodeDraft(JSON.stringify({...value,authority:'canon'})).authority,undefined);
});
test('preview reports selector scope, added context and limits without claiming corpus totals',()=>{
  const p=packet();assert.deepEqual(summarizeLens(p),{nodes:1,relations:0,matched:3,context:0,limited:true});
  p.inclusion.nodes[raw.id].kind='endpoint';assert.equal(summarizeLens(p).context,1);
  p.counts.nodes=2;assert.throws(()=>summarizeLens(p),ContractError);
});
test('live compile binds the current snapshot and retains the exact owned definition for links',async()=>{
  const calls=[],current=packet();const client={compile:async(spec,signal,revision)=>{calls.push({spec,signal,revision});return current;}};
  const value=draft(),preview=await previewDraft(client,value,context);
  assert.deepEqual(draftForPacket(preview),value);assert.equal(calls[0].revision,catalog.source_revision);
  assert.deepEqual(calls[0].spec,compileDraft(value,context));
  value.name='Later edit';assert.notEqual(draftForPacket(preview).name,value.name);
  assert.deepEqual(decodeDraft(encodeDraft(draftForPacket(preview))),draft());
});
test('local definitions update by name without touching another storage key or corrupt data',()=>{
  const data=new Map(),storage={getItem:key=>data.get(key)||null,setItem:(key,value)=>data.set(key,value)};
  saveDraft(storage,draft());saveDraft(storage,{...draft(),limit:20});
  assert.equal(data.size,1);assert.equal(readSaved(storage).length,1);assert.equal(readSaved(storage)[0].limit,20);
  data.set(SAVED_LENSES_KEY,'broken');assert.throws(()=>saveDraft(storage,draft()),ContractError);assert.equal(data.get(SAVED_LENSES_KEY),'broken');
});
