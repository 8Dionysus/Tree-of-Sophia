import {test} from 'vitest';
import assert from 'node:assert/strict';
import {KnowledgeClient,ContractError,RevisionError,RequestError} from './knowledge-client.mjs';
import {exactSourceRepresentations,readExactSource,validateExactSourceTarget,SOURCE_READ_RESPONSE_BYTES} from './exact-source-read.mjs';

const R='a'.repeat(64),C='b'.repeat(64),D='sha256:'+'c'.repeat(64);
const selection={kind:'node',id:'source-claims:identity:tos.agent.fixture',source_revision:R,content_revision:C};
const capabilities=({available=true,sourceRevision=R,representations=['record','native_public_unit','native_local_unit'],authority={writes_to_source:false,grants_current_use:false},...rest}={})=>({
  schema_version:'tos_source_read_capabilities_v1',available,source_epoch:{source_revision:sourceRevision},representations,authority,...rest});
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
  return {inspection,capabilities:capabilities(),discovery,read};
}

test('source capabilities preserve advertised native public/local choices while filtering future representations',async()=>{
  const calls=[],packet=capabilities({representations:['future_unit','native_local_unit','record','native_public_unit','future_other']});
  const client={request:async(path,options)=>{calls.push({path,options});return packet;}};
  assert.deepEqual(await exactSourceRepresentations(client,R),['native_local_unit','native_public_unit']);
  assert.equal(calls.length,1);assert.equal(calls[0].path,'/api/source/capabilities');
});

test('unavailable source capabilities are an explicit empty representation set',async()=>{
  const packet=capabilities({available:false,sourceRevision:'0'.repeat(64),representations:[],authority:null});
  assert.deepEqual(await exactSourceRepresentations({request:async()=>packet},R),[]);
});

test('source capability revision and contract or authority mutations fail closed',async()=>{
  const stale=capabilities({sourceRevision:'0'.repeat(64)});
  await assert.rejects(exactSourceRepresentations({request:async()=>stale},R),RevisionError);
  for(const mutate of [
    packet=>packet.schema_version='tos_source_read_capabilities_v_future',
    packet=>packet.available='yes',
    packet=>packet.representations=['native_public_unit',17],
    packet=>packet.authority=null,
    packet=>packet.authority.writes_to_source=true,
    packet=>packet.authority.grants_current_use=true,
  ]){
    const packet=capabilities();mutate(packet);
    await assert.rejects(exactSourceRepresentations({request:async()=>packet},R),ContractError);
  }
});

test('source capability discovery forwards the caller signal and caps response work at 64 KiB',async()=>{
  const controller=new AbortController();let seen=null;
  const client={request:async(path,options)=>{seen={path,options};return capabilities();}};
  await exactSourceRepresentations(client,R,controller.signal);
  assert.equal(seen.path,'/api/source/capabilities');assert.equal(seen.options.signal,controller.signal);
  assert.equal(seen.options.maxResponseBytes,65536);
});

function harness(data=fixture(),decorate=packet=>new Response(JSON.stringify(packet)),options={}){
  const calls=[];
  const client=new KnowledgeClient({...options,fetcher:async(path,options)=>{
    calls.push({path,body:options.body?JSON.parse(options.body):null,signal:options.signal});
    return decorate(path.startsWith('/api/knowledge/')?data.inspection:path.endsWith('/capabilities')?data.capabilities:path.endsWith('/handles')?data.discovery:
      JSON.parse(options.body).representation!=='record'?data.unitRead:data.read,path);
  }});
  return {client,calls,data};
}

test('source preflight and exact reading honor a smaller selected client response budget',async()=>{
  const {client,calls}=harness(fixture(),undefined,{maxResponseBytes:8192});
  assert.deepEqual(await exactSourceRepresentations(client,R),['native_public_unit','native_local_unit']);
  assert.equal((await readExactSource(client,selection)).status,'available');
  assert.equal(calls.length,5);
});

test('exact card inspection supplies the only source target; four bounded read-only requests preserve original wording',async()=>{
  const {client,calls,data}=harness();
  const result=await readExactSource(client,selection);
  assert.equal(result.status,'available');assert.deepEqual(result.record,data.read.record);
  assert.deepEqual(calls.map(row=>row.path),[
    '/api/knowledge/nodes/'+encodeURIComponent(selection.id)+'?relation_limit=0','/api/source/capabilities','/api/source/handles','/api/source/read']);
  assert.deepEqual(calls[2].body,{target:data.discovery.target});
  assert.deepEqual(calls[3].body,{handle:data.discovery.handle,representation:'record'});
  assert.deepEqual(result.selection,selection);assert.equal(result.access.rights_revalidated,false);
});

test('authored CSV has a separate owner, exact null/string cells and no path or metadata-handle fallback',async()=>{
  const data=fixture(),record={'10':'ten','2':'two',edge_id:'m001',note:'строка\nещё',missing:null,'\ue000':'bmp','😀':'astral'};
  const raw='{"10":"ten","2":"two","edge_id":"m001","missing":null,"note":"строка\\nещё","\ue000":"bmp","😀":"astral"}';
  const sha=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(raw))),b=>b.toString(16).padStart(2,'0')).join('');
  const target={layer:'authored_csv_record',pack_id:'canon/relations/fixture',edge_id:'m001',source_row:1,
    source_file_sha256:'1'.repeat(64),content_revision:'sha256:'+sha};
  data.discovery.target=target;data.discovery.content_revision=target.content_revision;
  data.discovery.handle.target=target;data.discovery.handle.issuer='Tree-of-Sophia/authored-corpus';
  data.discovery.access.scope='public-authored-csv-record';
  data.inspection.source_read_targets[selection.id].target=target;
  Object.assign(data.read,{layer:target.layer,record_kind:'authored_csv',record_ref:null,record,content_revision:target.content_revision,
    provenance:{source_row:1,source_file_sha256:target.source_file_sha256}});
  assert.deepEqual((await readExactSource(harness(data).client,selection)).record,record);
  for(const mutate of [d=>d.read.record.note='changed',d=>d.read.record.missing='',d=>d.read.record['2']=2,
    d=>d.discovery.handle.issuer='Tree-of-Sophia/source-witnesses',d=>d.discovery.access.scope='public-metadata-record',
    d=>d.read.provenance.source_row=2]){
    const bad=structuredClone(data);mutate(bad);
    await assert.rejects(readExactSource(harness(bad).client,selection),ContractError);
  }
  for(const bad of [{...target,path:'ToS/private'}, {...target,source_row:1.5},
    {...target,pack_id:'canon/relations/../hidden'}, {...target,pack_id:'candidate-intake/payload/x'},
    {...target,pack_id:'canon/relations/\ud800'}, {...target,edge_id:'\0'}])
    assert.throws(()=>validateExactSourceTarget(bad),ContractError);
});

test('native text is a separate gated request; bind exact IDs, span offsets and UTF-8 digest without normalizing',async()=>{
  const data=fixture(),binding={unit_id:'tos.text-unit.fixture',unit_version:1,
    segmentation_id:'tos.text-segmentation.fixture',segmentation_version:1,packet_id:'tos.packet.fixture',packet_version:1,
    packet_sha256:'1'.repeat(64),text_layer:{layer_id:'tos.text-layer.fixture',layer_version:1,record_sha256:'2'.repeat(64)},
    ordered_anchor_refs:['tos.anchor.fixture']};
  data.read.record.native_text_binding=binding;
  const text='cafe\u0301',sha=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text))),b=>b.toString(16).padStart(2,'0')).join('');
  const unit={schema_version:'tos_native_public_unit_return_v1',summary:{unit_id:binding.unit_id,unit_version:1,
    segmentation_id:binding.segmentation_id,segmentation_version:1,layer_id:binding.text_layer.layer_id,layer_version:1,
    content_verified:true,public_content_available:true,assessment_applied:false},packet:{id:binding.packet_id,version:1,sha256:binding.packet_sha256},
    layer_record_sha256:binding.text_layer.record_sha256,representation_sha256:'3'.repeat(64),closure_fingerprint:'sha256:'+'4'.repeat(64),
    spans:[{anchor_ref:'tos.anchor.fixture',selector:{type:'text_position',position_unit:'unicode_code_point',interval:'half_open',start:3,end:8},exact_sha256:sha,text}]};
  data.unitRead={...data.read,schema_version:'tos_source_native_unit_read_result_v1',record:null,native_unit:unit,
    text_access:{scope:'public-native-unit',recorded_rights_verified:true,conditional_rights:false,grants_current_use:false}};
  const options={representation:'native_public_unit'},h=harness(data);
  assert.equal((await readExactSource(h.client,selection,options)).native_unit.spans[0].text,text);
  assert.equal(h.calls.length,5);assert.deepEqual(h.calls[4].body,{handle:data.discovery.handle,representation:'native_public_unit'});
  for(const mutate of [d=>d.unitRead.native_unit.spans[0].text='café',d=>d.unitRead.native_unit.summary.unit_id='tos.text-unit.foreign',
    d=>d.unitRead.native_unit.spans[0].exact_sha256='0'.repeat(64),d=>d.unitRead.text_access.conditional_rights=true]){
    const wrong=structuredClone(data);mutate(wrong);
    await assert.rejects(readExactSource(harness(wrong).client,selection,options),ContractError);
  }
  const local=structuredClone(data),localOptions={representation:'native_local_unit'};
  local.unitRead.native_unit.schema_version='tos_native_local_unit_return_v1';
  local.unitRead.text_access={scope:'local-native-unit',recorded_rights_verified:true,conditional_rights:true,
    grants_current_use:false,external_publication_authorized:false};
  const noticeText='Synthetic notice\r\n',noticeHash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(noticeText))),b=>b.toString(16).padStart(2,'0')).join('');
  local.unitRead.native_unit.local_conditions={selection_sha256:'5'.repeat(64),expires_at:new Date(Date.now()+60000).toISOString(),
    condition_review:'Synthetic conditions only',notices:['license','attribution'].map(role=>({role,ref:role+'.md',sha256:noticeHash,text:noticeText}))};
  assert.deepEqual((await readExactSource(harness(local).client,selection,localOptions)).native_unit.local_conditions,
    local.unitRead.native_unit.local_conditions);
  for(const mutate of [d=>delete d.unitRead.native_unit.local_conditions,
    d=>d.unitRead.native_unit.local_conditions.notices.pop(),
    d=>d.unitRead.native_unit.local_conditions.notices[0].text='changed',
    d=>d.unitRead.native_unit.local_conditions.expires_at='2020-01-01T00:00:00Z',
    d=>d.unitRead.text_access.external_publication_authorized=true,
    d=>d.unitRead.native_unit.schema_version='tos_native_public_unit_return_v1']){
    const wrong=structuredClone(local);mutate(wrong);
    await assert.rejects(readExactSource(harness(wrong).client,selection,localOptions),ContractError);
  }
  await assert.rejects(readExactSource(harness(local).client,selection,options),ContractError);
  await assert.rejects(readExactSource(harness(data).client,selection,localOptions),ContractError);
  data.unitRead={...data.unitRead,status:'access-restricted',reason:'native-unit-public-rights-not-satisfied',native_unit:null,text_access:null};
  assert.equal((await readExactSource(harness(data).client,selection,options)).native_unit,null);
});

test('missing source target never guesses from graph ID, native ID or local references',async()=>{
  const data=fixture();delete data.inspection.source_read_targets;
  const {client,calls}=harness(data);const result=await readExactSource(client,selection);
  assert.equal(result.status,'unsupported');assert.equal(result.record,null);assert.equal(calls.length,1);
});

test('unavailable source owner preserves the selected source context and never posts a handle',async()=>{
  const data=fixture();
  data.capabilities=capabilities({available:false,representations:[],authority:{is_source:false,writes_to_source:false,grants_current_use:false,native_text_payload:false}});
  data.capabilities.source_epoch=null;
  const {client,calls}=harness(data);
  const result=await readExactSource(client,selection);
  assert.deepEqual(result,{status:'unsupported',reason:'source-owner-reader-not-configured',selection,record:null});
  assert.deepEqual(calls.map(row=>row.path),[
    '/api/knowledge/nodes/'+encodeURIComponent(selection.id)+'?relation_limit=0','/api/source/capabilities']);
  assert.equal(calls.some(row=>row.path.endsWith('/handles')||row.path.endsWith('/read')),false);
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
    assert.equal(result.status,status);assert.equal(result.record,null);assert.equal(calls.length,3);
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
