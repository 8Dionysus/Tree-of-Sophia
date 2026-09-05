import {test} from 'vitest';
import assert from 'node:assert/strict';

import {validateLens,projectLens,focusSpec,KnowledgeClient,RequestSlots,ContractError,RevisionError,RequestError} from './knowledge-client.mjs';

const node=id=>({id,entity_id:'tos.work.friedrich-nietzsche.also-sprach-zarathustra',kind_id:'work',
  content_revision:'b'.repeat(64),source_refs:['ToS/fixture/work.json'],display:{title:{ru:'Произведение'},kind_label:{ru:'Произведение'},summary:{ru:'Источник'}}});
const fixture={schema:'tos_lens_result_v1',source_revision:'a'.repeat(64),authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},
  nodes:[node('graph-a:work'),node('graph-b:work')],focus:{node_id:'graph-a:work'},
  relations:[{id:'relation:1',from_id:'graph-a:work',to_id:'graph-b:work',content_revision:'c'.repeat(64),source_refs:['ToS/fixture/relation.json'],display:{label:{ru:'Связано с'}}}]};
const clone=()=>structuredClone(fixture);
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no});return {promise,resolve,reject};};

test('access LensResult keeps source authority and exact opaque identities',()=>{
  assert.equal(validateLens(fixture),fixture);
  const before=JSON.stringify(fixture),nodes=projectLens(fixture);
  assert.equal(nodes.length,fixture.nodes.length);
  assert.equal(nodes[0].id,fixture.focus.node_id);
  const identities=nodes.filter(n=>n.raw.entity_id==='tos.work.friedrich-nietzsche.also-sprach-zarathustra');
  assert.ok(identities.length>=2);
  assert.equal(new Set(identities.map(n=>n.id)).size,identities.length);
  assert.equal(JSON.stringify(fixture),before);
});

test('rejects mismatched revisions, invented authority, duplicates, incomplete edges and excess data',()=>{
  assert.throws(()=>validateLens(fixture,'0'.repeat(64)),RevisionError);
  for(const mutate of [
    p=>p.authority_boundary.is_canon=true,
    p=>p.nodes.push(p.nodes[0]),
    p=>p.relations[0].to_id='absent',
    p=>p.nodes[0].content_revision='unstable',
    p=>p.nodes[0].source_refs=[],
    p=>p.nodes=Array.from({length:41},(_,i)=>({...p.nodes[0],id:String(i)})),
  ]){const packet=clone();mutate(packet);assert.throws(()=>validateLens(packet),ContractError);}
});

test('membership refresh preserves surviving positions without reserving vanished slots',()=>{
  const before=projectLens(fixture),saved=JSON.stringify(before);
  before[0].target=[17,28,-39];
  const refreshed=projectLens({...fixture,nodes:fixture.nodes.slice().reverse()},before);
  for(const node of refreshed){const old=before.find(n=>n.id===node.id);assert.deepEqual(node.target,old.target);assert.equal(node.slot,old.slot);assert.notEqual(node.target,old.target);}
  const packet=clone();packet.nodes=[packet.nodes[0]];packet.focus={node_id:packet.nodes[0].id};packet.relations=[];
  const unrelated=before.map(n=>({...n,id:'gone:'+n.id}));
  assert.equal(projectLens(packet,unrelated)[0].slot,0);
  assert.notEqual(JSON.stringify(before),saved); // Only the explicit camera fixture mutation above.
  assert.deepEqual(before[0].target,[17,28,-39]);
});

test('superseded responses cannot replace the active scene even if transport ignores abort',async()=>{
  const slots=new RequestSlots(),first=deferred(),second=deferred();let firstSignal;
  const a=slots.run('scene',signal=>{firstSignal=signal;return first.promise;});
  const b=slots.run('scene',()=>second.promise);
  assert.equal(firstSignal.aborted,true);
  second.resolve('new');assert.deepEqual(await b,{current:true,value:'new'});
  first.resolve('old');assert.deepEqual(await a,{current:false});
});

test('search and inspector cancel independently; cancelled errors remain silent',async()=>{
  const slots=new RequestSlots(),search=deferred(),inspect=deferred();
  const a=slots.run('search',()=>search.promise),b=slots.run('inspect',()=>inspect.promise);
  slots.cancel('inspect');inspect.reject(new Error('late failure'));
  search.resolve('kept');assert.deepEqual(await a,{current:true,value:'kept'});assert.deepEqual(await b,{current:false});
  const one=deferred(),two=deferred();const x=slots.run('scene',()=>one.promise),y=slots.run('inspect',()=>two.promise);
  slots.cancelAll();one.resolve(1);two.resolve(2);assert.deepEqual(await x,{current:false});assert.deepEqual(await y,{current:false});
});

test('HTTP adapter sends the compact bounded contract and encodes opaque IDs',async()=>{
  const calls=[];const raw=fixture.nodes[0];
  const client=new KnowledgeClient({fetcher:async(url,options)=>{
    calls.push({url,options});return {ok:true,json:async()=>url.includes('compile')?clone():{
      schema:'tos_knowledge_node_packet_v1',source_revision:fixture.source_revision,matches:[raw]}};
  }});
  const spec=focusSpec(raw.id);await client.compile(spec,undefined,fixture.source_revision);
  const inspected=await client.inspect('node',raw.id,undefined,fixture.source_revision);
  assert.equal(calls[0].url,'/api/knowledge/lenses/compile');assert.equal(calls[0].options.method,'POST');
  const sent=JSON.parse(calls[0].options.body);assert.equal(sent.detail,'compact');assert.equal(sent.traversal.profile,'overview');assert.deepEqual(sent.limits,{nodes:40,relations:80,groups:8});
  assert.equal(calls[1].url,'/api/knowledge/nodes/'+encodeURIComponent(raw.id)+'?relation_limit=0');
  assert.equal(inspected.match.id,raw.id);
  await assert.rejects(client.inspect('node','different/id',undefined,fixture.source_revision),ContractError);
});

test('timeouts fail visibly and user cancellation remains cancellation',async()=>{
  const fetcher=(_url,{signal})=>new Promise((_resolve,reject)=>{
    if(signal.aborted)reject(signal.reason);else signal.addEventListener('abort',()=>reject(signal.reason),{once:true});
  });
  const client=new KnowledgeClient({fetcher,timeoutMs:15});
  await assert.rejects(client.capabilities(),error=>error instanceof RequestError&&error.status===504);
  const slots=new RequestSlots();const call=slots.run('scene',signal=>client.capabilities(signal));slots.cancel('scene');
  assert.deepEqual(await call,{current:false});
});

test('network and malformed payload errors are readable without losing cancellation',async()=>{
  const network=new KnowledgeClient({fetcher:async()=>{throw new TypeError('Failed to fetch');}});
  await assert.rejects(network.capabilities(),e=>e instanceof RequestError&&e.status===0&&e.message.startsWith('Нет связи'));
  const malformed=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>{throw new SyntaxError('invalid');}})});
  await assert.rejects(malformed.capabilities(),ContractError);
});
