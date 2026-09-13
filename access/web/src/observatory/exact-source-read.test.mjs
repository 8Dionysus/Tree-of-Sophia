import {test} from 'vitest';
import assert from 'node:assert/strict';
import {KnowledgeClient,ContractError,RevisionError,RequestError} from './knowledge-client.mjs';
import {readExactSource,SOURCE_READ_RESPONSE_BYTES} from './exact-source-read.mjs';

const R='a'.repeat(64),C='b'.repeat(64),D='sha256:'+'c'.repeat(64);
const selection={kind:'node',id:'source-claims:identity:tos.agent.fixture',source_revision:R,content_revision:C};
function fixture(){
  const target={layer:'metadata_record',record_type:'agent',record_ref:{id:'tos.agent.fixture',version:2,digest:D},content_revision:D};
  const access={scope:'public-metadata-record',visibility:'public_metadata_only',visibility_verified:true,
    rights_revalidated:false,rights_scope:'metadata-disclosure-only',authority:'source-owner-public-metadata-contract'};
  const handle={schema_version:'tos_source_read_handle_v1',issuer:'Tree-of-Sophia/source-witnesses',
    epoch:{source_revision:R,catalog_root_sha256:'d'.repeat(64),catalog_namespace:'tos.fixture',
      source_publication:{protocol:'tos_selected_source_metadata_v1',token:'sha256:'+'e'.repeat(64),generation:2}},
    target,access,handle_digest:'sha256:'+'f'.repeat(64)};
  const flags={grants_current_use:false,performs_assessment:false,writes_to_source:false};
  const discovery={schema_version:'tos_source_handle_discovery_v1',status:'available',reason:'owner-issued-exact-source-handle',
    target,handle,source_revision:R,content_revision:D,provenance:{},access,...flags};
  const read={schema_version:'tos_source_read_result_v1',status:'available',reason:'owner-record-available',
    handle,source_revision:R,content_revision:D,layer:target.layer,record_ref:target.record_ref,
    record:{record_type:'agent',record_id:'tos.agent.fixture',record_version:2,preferred_label:'An agent',notes:'Exact source wording.'},
    provenance:{},access,...flags};
  const inspection={schema:'tos_knowledge_node_packet_v1',source_revision:R,
    matches:[{id:selection.id,content_revision:C,display:{title:{en:'An agent'}},source_refs:['ToS/fixture.json']}],
    source_read_targets:{[selection.id]:{source_revision:R,target}}};
  return {inspection,discovery,read};
}
function harness(data=fixture(),decorate=packet=>new Response(JSON.stringify(packet))){
  const calls=[];
  const client=new KnowledgeClient({fetcher:async(path,options)=>{
    calls.push({path,body:options.body?JSON.parse(options.body):null,signal:options.signal});
    return decorate(path.startsWith('/api/knowledge/')?data.inspection:path.endsWith('/handles')?data.discovery:data.read,path);
  }});
  return {client,calls,data};
}

test('exact card inspection supplies the only source target; three bounded read-only requests preserve original wording',async()=>{
  const {client,calls,data}=harness();
  const result=await readExactSource(client,selection);
  assert.equal(result.status,'available');assert.deepEqual(result.record,data.read.record);
  assert.deepEqual(calls.map(row=>row.path),[
    '/api/knowledge/nodes/'+encodeURIComponent(selection.id)+'?relation_limit=0','/api/source/handles','/api/source/read']);
  assert.deepEqual(calls[1].body,{target:data.discovery.target});
  assert.deepEqual(calls[2].body,{handle:data.discovery.handle,representation:'record'});
  assert.deepEqual(result.selection,selection);assert.equal(result.access.rights_revalidated,false);
});

test('missing source target never guesses from graph ID, native ID or local references',async()=>{
  const data=fixture();delete data.inspection.source_read_targets;
  const {client,calls}=harness(data);const result=await readExactSource(client,selection);
  assert.equal(result.status,'unsupported');assert.equal(result.record,null);assert.equal(calls.length,1);
});

test('native witness identities are exact without fabricated record fields',async()=>{
  for(const [schema,kind,field] of [['tos_artifact_source_witness_v1','artifact','artifact_id'],
    ['tos_artifact_source_witness_v2','artifact','artifact_id'],['tos_scholarly_composite_witness_v1','composite','composite_id']]){
    const data=fixture(),target=data.discovery.target;
    target.record_type=kind;target.record_ref.id='tos.'+kind+'.fixture';
    data.read.record={schema_version:schema,[field]:target.record_ref.id,record_version:2,visibility:'public_metadata_only',unknown:{preserved:true}};
    const result=await readExactSource(harness(data).client,selection);
    assert.deepEqual(result.record,data.read.record);
    for(const patch of [{record_id:target.record_ref.id},{record_type:kind},{[field]:'tos.agent.foreign'},{record_version:3}]){
      const wrong=structuredClone(data);Object.assign(wrong.read.record,patch);
      await assert.rejects(readExactSource(harness(wrong).client,selection),ContractError);
    }
  }
});

test('unknown source fields cannot silently round a large integer in an exact record',async()=>{
  const data=fixture();data.read.record.extension={values:['LARGE_INTEGER']};
  const {client}=harness(data,(packet,path)=>new Response(
    JSON.stringify(packet).replace('"LARGE_INTEGER"','9007199254740993')));
  await assert.rejects(readExactSource(client,selection),ContractError);
  data.read.record.extension={values:[Number.MAX_SAFE_INTEGER,-Number.MAX_SAFE_INTEGER,0,0.125,'9007199254740993']};
  const result=await readExactSource(harness(data).client,selection);
  assert.deepEqual(result.record.extension,data.read.record.extension);
});

test('nonavailable owner result stops at discovery without another request',async()=>{
  for(const status of ['missing','corrupt','access-restricted','over-budget','unsupported']){
    const data=fixture();data.discovery={...data.discovery,status,reason:'owner-unavailable',handle:null};
    const {client,calls}=harness(data);const result=await readExactSource(client,selection);
    assert.equal(result.status,status);assert.equal(result.record,null);assert.equal(calls.length,2);
  }
});

test('foreign source or content versions, swapped records and rights escalation fail closed',async()=>{
  for(const [mutate,ErrorType] of [
    [d=>d.inspection.source_read_targets[selection.id].source_revision='0'.repeat(64),RevisionError],
    [d=>d.inspection.matches[0].content_revision='0'.repeat(64),RevisionError],
    [d=>d.read.source_revision='0'.repeat(64),RevisionError],
    [d=>d.discovery.handle.access.rights_revalidated=true,ContractError],
    [d=>d.read.record.record_id='tos.agent.other',ContractError],
    [d=>d.read.record.record_version=3,ContractError],
    [d=>d.inspection.source_read_targets[selection.id].target={layer:'metadata_record',path:'/etc/passwd'},ContractError],
  ]){
    const data=fixture();mutate(data);await assert.rejects(readExactSource(harness(data).client,selection),ErrorType);
  }
});

test('source response budget cannot inherit or widen the larger graph budget',async()=>{
  const {client}=harness(fixture(),(packet,path)=>new Response(JSON.stringify(packet),{headers:
    path.endsWith('/read')?{'Content-Length':String(SOURCE_READ_RESPONSE_BYTES+1)}:{}}));
  await assert.rejects(readExactSource(client,selection),error=>error instanceof RequestError&&error.status===413);
  let called=false;const tight=new KnowledgeClient({maxResponseBytes:1024,fetcher:()=>{called=true;}});
  await assert.rejects(tight.request('/catalog',{maxResponseBytes:2048}),RangeError);assert.equal(called,false);
});

test('one overall deadline and explicit abort end uncooperative reads without retry',async()=>{
  let calls=0;const client={inspect:()=>{calls++;return new Promise(()=>{});}};
  await assert.rejects(readExactSource(client,selection,{timeoutMs:5}),error=>error instanceof RequestError&&error.status===504);
  assert.equal(calls,1);
  const controller=new AbortController();const pending=readExactSource(client,selection,{signal:controller.signal});
  controller.abort();await assert.rejects(pending);assert.equal(calls,2);
});
