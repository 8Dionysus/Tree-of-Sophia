import {test} from 'vitest';
import assert from 'node:assert/strict';
import {evidenceRoute,validateEvidence,loadEvidence,compareEvidence} from './evidence-model.mjs';
import {ContractError,RevisionError,RequestError} from './knowledge-client.mjs';

const raw={id:'opaque:retain/this:identity',native_id:'native:a/b',source_graph:'philosophy',content_revision:'b'.repeat(64),source_refs:['ToS/fixture.json'],display:{label:{ru:'Прочтение'}}};
const relation=id=>({edge_id:id,predicate_id:'contested_by',from_id:'a',to_id:'b',properties:{comment:'Исходное замечание'},source_ref:'ToS/fixture.json'});
const packet=()=>({schema:'tos_evidence_lens_packet_v1',mode:'philosophy',item_id:raw.native_id,selection:relation(raw.native_id),
  authority_boundary:{is_source:false,is_canon:false,is_semantic_truth:false,is_rights_clearance:false},conclusion:{can_conclude:false},
  challenge_relations:[relation(raw.native_id),relation('another')],context_relations:[relation('context')],neighbor_nodes:[{node_id:'a',label:'А'},{node_id:'b',label:'Б'}],source_refs:['ToS/fixture.json'],routes:[],source_anchors:[],gaps:['Still unresolved'],coverage:{available_challenge_relations:9,returned_challenge_relations:2}});

test('only explicit projection identity binds the old evidence endpoint',()=>{
  assert.deepEqual(evidenceRoute(raw),{mode:'philosophy',item_id:'native:a/b'});
  for(const candidate of [{...raw,native_id:undefined},{...raw,source_graph:'source-navigation'},{...raw,source_graph:'source-claims'},{id:'philosophy:guess-me'}])assert.equal(evidenceRoute(candidate),null);
  assert.deepEqual(evidenceRoute({...raw,source_graph:'canon'}),{mode:'corpus',item_id:'native:a/b',view_id:'route-graph'});
});

test('evidence cannot cross identity, provenance, mode, kind or source-authority boundaries',()=>{
  assert.equal(validateEvidence(packet(),raw,'relation',evidenceRoute(raw)).item_id,raw.native_id);
  for(const change of [p=>p.item_id='other',p=>p.selection.edge_id='other',p=>p.selection={node_id:raw.native_id,source_ref:'ToS/fixture.json'},p=>p.selection.source_ref='unrelated',p=>p.mode='corpus',p=>p.authority_boundary.is_canon=true,p=>p.context_relations=null]){
    const p=packet();change(p);assert.throws(()=>validateEvidence(p,raw,'relation',evidenceRoute(raw)),ContractError);
  }
});

test('the selected relation is not its own competing reading; context and partial coverage stay distinct',()=>{
  const p=packet(),before=JSON.stringify(p),comparison=compareEvidence({packet:p,binding:{knowledge_id:raw.id}},{id:raw.id,kind:'edge'});
  assert.deepEqual(comparison.competing_readings.map(x=>x.id),['another']);assert.equal(comparison.competing_readings[0].route,'А → Б');
  assert.deepEqual(comparison.contextual_readings.map(x=>x.id),['context']);assert.equal(comparison.coverage.available_challenge_relations,9);
  assert.equal(comparison.can_conclude,false);assert.equal(comparison.selection.id,raw.id);assert.equal(JSON.stringify(p),before);
  p.challenge_relations=[relation(raw.native_id)];assert.equal(compareEvidence({packet:p},{id:raw.id}).competing_reading_count,0);
});

test('loading verifies the knowledge snapshot before and after the unversioned evidence call',async()=>{
  const calls=[],client={inspect:async(...args)=>{calls.push(args);return {match:raw};}},queries={invoke:async(op,input)=>{assert.equal(op,'tos.epistemic.inspect');assert.equal(input.item_id,raw.native_id);return packet();}};
  const result=await loadEvidence(raw,'relation','a'.repeat(64),{client,queries});
  assert.equal(calls.length,2);assert.equal(calls[1][0],'relation');assert.equal(calls[1][1],raw.id);assert.equal(calls[1][3],'a'.repeat(64));
  assert.equal(result.binding.knowledge_id,raw.id);assert.equal(result.availability,'available');
  client.inspect=async()=>({match:{...raw,content_revision:'c'.repeat(64)}});
  await assert.rejects(loadEvidence(raw,'relation','a'.repeat(64),{client,queries}),RevisionError);
  let count=0;client.inspect=async()=>({match:++count===1?raw:{...raw,content_revision:'c'.repeat(64)}});
  await assert.rejects(loadEvidence(raw,'relation','a'.repeat(64),{client,queries}),RevisionError);
});

test('unsupported layers and route-graph absence do not become absence of evidence; server failures stay failures',async()=>{
  let invoked=0;const candidate={...raw,source_graph:'source-navigation'},client={inspect:async()=>({match:candidate})},queries={invoke:async()=>{invoked++;throw new RequestError(404,'missing');}};
  assert.equal((await loadEvidence(candidate,'relation','revision',{client,queries})).availability,'not_connected');assert.equal(invoked,0);
  client.inspect=async()=>({match:raw});assert.equal((await loadEvidence(raw,'relation','revision',{client,queries})).availability,'outside_route');
  queries.invoke=async()=>{throw new RequestError(500,'server');};await assert.rejects(loadEvidence(raw,'relation','revision',{client,queries}),error=>error.status===500);
});

test('an ignored abort cannot return late evidence to the panel',async()=>{
  const controller=new AbortController(),client={inspect:async()=>({match:raw})},queries={invoke:async()=>{controller.abort();return packet();}};
  await assert.rejects(loadEvidence(raw,'relation','revision',{client,queries,signal:controller.signal}),error=>error.name==='AbortError');
});
