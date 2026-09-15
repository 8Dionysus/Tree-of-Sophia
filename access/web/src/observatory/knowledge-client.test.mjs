import {test} from 'vitest';
import assert from 'node:assert/strict';

import {validateLens,projectLens,focusSpec,KnowledgeClient,RequestSlots,ContractError,RevisionError,RequestError,displayTitle,displayTitleForm,sourceOriginalTitle,compileRouteCenter,isSourceDossierRef,SOURCE_DOSSIER_LIMIT} from './knowledge-client.mjs';
import {setUiLanguage} from './ui-i18n.mjs';

const node=id=>({id,entity_id:'tos.work.friedrich-nietzsche.also-sprach-zarathustra',kind_id:'work',
  content_revision:'b'.repeat(64),source_refs:['ToS/fixture/work.json'],display:{title:{ru:'Произведение'},kind_label:{ru:'Произведение'},summary:{ru:'Источник'}}});
const fixture={schema:'tos_lens_result_v1',source_revision:'a'.repeat(64),authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},
  nodes:[node('graph-a:work'),node('graph-b:work')],focus:{node_id:'graph-a:work'},
  relations:[{id:'relation:1',from_id:'graph-a:work',to_id:'graph-b:work',content_revision:'c'.repeat(64),source_refs:['ToS/fixture/relation.json'],display:{label:{ru:'Связано с'}}}]};
const clone=()=>structuredClone(fixture);
const emptySearchPacket=mode=>({schema:mode==='indexed'?'tos_knowledge_search_indexed_v2':'tos_knowledge_search_compressed_v3',
  source_revision:fixture.source_revision,authority_boundary:fixture.authority_boundary,nodes:[],relations:[],counts:{matching_nodes:null,matching_relations:null},
  page:{cursor:null,limit_per_kind:6,has_more:false,next_cursor:null}});

test('search selects an advertised engine and retains native cursor, schema and unknown counts',async()=>{
  for(const mode of ['indexed','compressed']){
    const calls=[],cursor=' opaque + / cursor ';
    const client=new KnowledgeClient({fetcher:async(url)=>{
      calls.push(url);const params=new URL(url,'http://fixture').searchParams;
      const packet=url.endsWith('/capabilities')?{modes:{[mode]:{available:true}}}:
        {schema:mode==='indexed'?'tos_knowledge_search_indexed_v2':'tos_knowledge_search_compressed_v3',
          source_revision:fixture.source_revision,authority_boundary:fixture.authority_boundary,
          nodes:[node('exact-node')],relations:[],counts:{matching_nodes:null,matching_relations:null},
          page:{cursor:params.get('cursor'),limit_per_kind:6,has_more:!params.has('cursor'),next_cursor:params.has('cursor')?null:cursor}};
      return {ok:true,json:async()=>packet};
    }});
    const first=await client.search('freedom');
    const second=await client.search('freedom',undefined,{cursor:first.page.next_cursor,search_mode:first.search_mode,source_revision:first.source_revision});
    assert.equal(first.search_mode,mode);assert.equal(second.schema,first.schema);
    assert.equal(second.page.cursor,cursor);assert.equal(second.counts.matching_nodes,null);
    for(const url of calls.filter(url=>!url.endsWith('/capabilities'))){const params=new URL(url,'http://fixture').searchParams;
      assert.equal(params.get('mode'),mode);assert.equal(params.has('offset'),false);}
  }
});

test('search refuses unavailable mode and rejected continuation without retry or fallback',async()=>{
  const calls=[];
  const client=new KnowledgeClient({fetcher:async(url)=>{calls.push(url);return url.endsWith('/capabilities')
    ?{ok:true,json:async()=>({modes:{compressed:{available:true},indexed:{available:false}}})}
    :{ok:false,status:409};}});
  await assert.rejects(client.search('freedom',undefined,{search_mode:'indexed'}),/mode unavailable/);
  assert.equal(calls.length,1);
  await assert.rejects(client.search('freedom',undefined,{cursor:'cursor',search_mode:'compressed'}),RevisionError);
  assert.equal(calls.filter(url=>!url.endsWith('/capabilities')).length,1);
  await assert.rejects(client.search('freedom',undefined,{cursor:'unbound'}),ContractError);
  assert.equal(calls.length,3);
});

test('search selects a compatible engine and rejects an explicit short indexed mode before /search',async()=>{
  const onlyIndexedCalls=[];
  const onlyIndexed=new KnowledgeClient({fetcher:async(url)=>{
    onlyIndexedCalls.push(url);
    if(url.endsWith('/capabilities'))return {ok:true,json:async()=>({modes:{indexed:{available:true,min_normalized_query_code_points:3},compressed:{available:false}}})};
    throw new Error('search must not be called');
  }});
  await assert.rejects(onlyIndexed.search('道'),/shorter than the minimum supported/);
  assert.equal(onlyIndexedCalls.filter(url=>!url.endsWith('/capabilities')).length,0);

  const fallbackCalls=[];
  const fallback=new KnowledgeClient({fetcher:async(url)=>{
    fallbackCalls.push(url);
    if(url.endsWith('/capabilities'))return {ok:true,json:async()=>({modes:{indexed:{available:true,min_normalized_query_code_points:3},compressed:{available:true}}})};
    return {ok:true,json:async()=>emptySearchPacket('compressed')};
  }});
  const selected=await fallback.search('道');
  assert.equal(selected.search_mode,'compressed');
  assert.equal(new URL(fallbackCalls[1],'http://fixture').searchParams.get('mode'),'compressed');

  const explicitCalls=[];
  const explicit=new KnowledgeClient({fetcher:async(url)=>{
    explicitCalls.push(url);
    if(url.endsWith('/capabilities'))return {ok:true,json:async()=>({modes:{indexed:{available:true,min_normalized_query_code_points:3},compressed:{available:true}}})};
    throw new Error('explicit indexed search must not be called');
  }});
  await assert.rejects(explicit.search('道',undefined,{search_mode:'indexed'}),/mode indexed requires at least 3 normalized/);
  assert.equal(explicitCalls.filter(url=>!url.endsWith('/capabilities')).length,0);
});

test('search query minimum counts native-lowered and edge-stripped Unicode code points',async()=>{
  const acceptedCalls=[];
  const accepted=new KnowledgeClient({fetcher:async(url)=>{
    acceptedCalls.push(url);
    if(url.endsWith('/capabilities'))return {ok:true,json:async()=>({modes:{indexed:{available:true,min_normalized_query_code_points:2},compressed:{available:false}}})};
    return {ok:true,json:async()=>emptySearchPacket('indexed')};
  }});
  const acceptedPacket=await accepted.search('  İ  ');
  assert.equal(acceptedPacket.search_mode,'indexed');
  assert.equal(new URL(acceptedCalls[1],'http://fixture').searchParams.get('query'),'  İ  ');

  const codePointCalls=[];
  const codePoint=new KnowledgeClient({fetcher:async(url)=>{
    codePointCalls.push(url);
    if(url.endsWith('/capabilities'))return {ok:true,json:async()=>({modes:{indexed:{available:true,min_normalized_query_code_points:3},compressed:{available:false}}})};
    throw new Error('UTF-16 length must not authorize indexed search');
  }});
  await assert.rejects(codePoint.search('😀a'),/shorter than the minimum supported/);
  assert.equal(codePointCalls.filter(url=>!url.endsWith('/capabilities')).length,0);
});

test('search refuses mismatched snapshot and malformed native pages',async()=>{
  for(const mutation of [p=>p.source_revision='c'.repeat(64),p=>p.page.cursor='wrong',p=>p.page.limit_per_kind=8,
    p=>p.schema='tos_knowledge_search_v1',p=>p.page.next_cursor=null]){
    const packet={schema:'tos_knowledge_search_compressed_v3',source_revision:fixture.source_revision,
      authority_boundary:fixture.authority_boundary,nodes:[],relations:[],
      page:{cursor:null,limit_per_kind:6,has_more:true,next_cursor:'next'}};
    mutation(packet);
    const client=new KnowledgeClient({fetcher:async(url)=>({ok:true,json:async()=>url.endsWith('/capabilities')
      ?{modes:{compressed:{available:true}}}:packet})});
    await assert.rejects(client.search('freedom',undefined,{source_revision:fixture.source_revision}));
  }
});
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no});return {promise,resolve,reject};};
const dossier=()=>({schema:'tos_source_dossier_v1',object_id:'tos.work.fixture',object:{node_id:'tos.work.fixture',node_kind:'work',label:'Fixture Work',properties:{}},
  agent_summary:{technical_access:'metadata_only',rights_posture:'unknown',human_review_required:true,can_conclude_legal_openness:false,
    availability_is_license:false,rights_scope_refs:['tos.work.fixture'],gaps:['no associated public rights record']},
  chain:{work:[{node_id:'tos.work.fixture',node_kind:'work'}],link:[]},tree_paths:[],relations:[],rights:[],source_refs:['ToS/source-witnesses/works/fixture/work.json'],truncated:false,
  authority_note:'ToS source_navigation remains authoritative.'});

test('navigation language round trips preserve identities, positions and navigation-only provenance',()=>{
  const packet=clone();
  for(const raw of packet.nodes){raw.display.title={ru:'Запись утверждения',en:'Claim record',original:null};raw.display.provenance={title:'navigation-template',source_title_available:false};}
  const before=structuredClone(packet);let projected=projectLens(packet);
  projected[0].target=[17,28,-39];const slots=projected.map(n=>[n.id,n.slot]);
  try{for(const language of ['ru','en','ru']){
    setUiLanguage(language);projected=projectLens(packet,projected);
    assert.equal(projected[0].fullName,packet.nodes[0].display.title[language]);
    assert.equal(projected[0].original,'');assert.deepEqual(projected[0].target,[17,28,-39]);
    assert.deepEqual(projected.map(n=>[n.id,n.slot]),slots);
    assert.equal(displayTitleForm(packet.nodes[0],language).navigationOnly,true);
  }}finally{setUiLanguage('ru');}
  assert.deepEqual(packet,before);
});

test('material language overrides navigation language and only an actual original occupies the original slot',()=>{
  const raw=node('same-id');raw.display.title={ru:'Слово',en:'Word',original:'λόγος'};
  try{setUiLanguage('en');assert.equal(displayTitle(raw),'Word');assert.equal(displayTitle(raw,'','ru'),'Слово');
    assert.equal(sourceOriginalTitle(raw),'λόγος');
    delete raw.display.title.original;assert.equal(sourceOriginalTitle(raw),'');
    raw.display.title={default:'Owner fallback',ru:'Слово'};
    assert.deepEqual(displayTitleForm(raw,'en'),{text:'Owner fallback',key:'default',lang:null,fallback:true});
    raw.display.title.original='not source wording';raw.display.provenance={title:'navigation-template',source_title_available:false};
    assert.equal(sourceOriginalTitle(raw),'');
  }finally{setUiLanguage('ru');}
});

test('explicit identifier fallback is a missing title, retaining identity and positions through a supplied title repair',()=>{
  const packet=clone();
  packet.nodes=packet.nodes.map(raw=>({...raw,kind_id:'future-kind',display:{...raw.display,
    title:{default:'claim:tos claim translation identifier'},kind_label:{default:'Supplied kind'},provenance:{title:'identifier-fallback'}}}));
  const before=structuredClone(packet),presented=projectLens(packet);
  for(const raw of presented){assert.equal(raw.fullName,'Supplied kind · Нет читаемого названия');assert.equal(raw.name,'Нет читаемого названия');assert.equal(raw.original,'');}
  assert.deepEqual(packet,before);assert.deepEqual(presented.map(n=>n.id),packet.nodes.map(n=>n.id));
  presented[0].target=[17,28,-39];
  packet.nodes[0].display.title={ru:'Название, переданное владельцем'};packet.nodes[0].display.provenance.title='source-bound-navigation';
  const repaired=projectLens(packet,presented);
  assert.equal(repaired[0].fullName,'Название, переданное владельцем');assert.deepEqual(repaired[0].target,[17,28,-39]);
  assert.equal(repaired[0].slot,presented[0].slot);
});

test('title guard uses provenance alone and leaves supplied content and relation labels verbatim',()=>{
  const raw=node('claim:tos.claim.opaque');raw.display.title={ru:'claim:tos claim opaque'};
  for(const title of [undefined,'projected-label','source-bound-navigation']){
    raw.display.provenance={title};assert.equal(displayTitle(raw),raw.display.title.ru);
  }
  assert.equal(displayTitle(fixture.relations[0]),'Связано с');
  raw.display.provenance.title='identifier-fallback';
  raw.human_form_selection={roles:{caption:{wording:'Do not replace the missing name with this statement.'}}};
  assert.equal(displayTitle(raw),'Произведение · Нет читаемого названия');
});

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

test('route center resolves an opaque relation as the actual scene center and falls back only on relation 404',async()=>{
  const relation=fixture.relations[0],calls=[];
  const relationPacket={schema:'tos_knowledge_relation_packet_v1',source_revision:fixture.source_revision,
    matches:[relation],endpoints:fixture.nodes};
  const client=new KnowledgeClient({fetcher:async(url,options)=>{
    calls.push({url,options});
    if(url.includes('/relations/'))return {ok:true,json:async()=>relationPacket};
    return {ok:true,json:async()=>clone()};
  }});
  const resolved=await compileRouteCenter(client,relation.id);
  assert.equal(resolved.kind,'relation');assert.equal(resolved.relation.id,relation.id);
  assert.equal(resolved.packet.focus.node_id,relation.from_id);
  assert.deepEqual(resolved.packet.relations.map(item=>item.id),[relation.id]);
  const sent=JSON.parse(calls.find(call=>call.url.includes('/compile')).options.body);
  assert.deepEqual(sent.relation_query.filters,[{field:'id',op:'eq',value:relation.id}]);

  const fallbackCalls=[];
  const fallback=new KnowledgeClient({fetcher:async(url,options)=>{
    fallbackCalls.push({url,options});
    if(url.includes('/relations/'))return {ok:false,status:404,json:async()=>({})};
    return {ok:true,json:async()=>clone()};
  }});
  const node=await compileRouteCenter(fallback,'graph-a:work');
  assert.equal(node.kind,'node');assert.equal(node.packet.focus.node_id,'graph-a:work');
  assert.equal(fallbackCalls.filter(call=>call.url.includes('/compile')).length,1);
});

test('source dossier follows only an owner handle through the bounded source route',async()=>{
  const calls=[],client=new KnowledgeClient({fetcher:async(url,options)=>{calls.push({url,options});return {ok:true,json:async()=>dossier()};}});
  assert.equal(isSourceDossierRef('tos.work.fixture'),true);assert.equal(isSourceDossierRef('ToS/source-witnesses/work.json'),false);
  const result=await client.sourceDossier('tos.work.fixture',undefined,{limit:12});
  assert.equal(result.object_id,'tos.work.fixture');assert.equal(calls[0].url,'/api/source/dossiers/tos.work.fixture?limit=12');assert.equal(calls[0].options.method,'GET');
  for(const limit of [0,SOURCE_DOSSIER_LIMIT+1])await assert.rejects(client.sourceDossier('tos.work.fixture',undefined,{limit}),ContractError);
  await assert.rejects(client.sourceDossier('../private/file'),ContractError);assert.equal(calls.length,1);
  const forged=dossier();forged.agent_summary.availability_is_license=true;
  const guarded=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>forged})});await assert.rejects(guarded.sourceDossier('tos.work.fixture'),ContractError);
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

test('a normalizer change with identical input revision cannot enter the old card cache',async()=>{
  const raw=fixture.nodes[0],normalized={...raw,content_revision:'f'.repeat(64)};
  const client=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>({
    schema:'tos_knowledge_node_packet_v1',source_revision:fixture.source_revision,matches:[normalized]})})});
  await assert.rejects(client.inspect('node',raw.id,undefined,fixture.source_revision,raw.content_revision),RevisionError);
  assert.equal((await client.inspect('node',raw.id,undefined,fixture.source_revision,normalized.content_revision)).match,normalized);
});

test('network and malformed payload errors are readable without losing cancellation',async()=>{
  const network=new KnowledgeClient({fetcher:async()=>{throw new TypeError('Failed to fetch');}});
  await assert.rejects(network.capabilities(),e=>e instanceof RequestError&&e.status===0&&e.message.startsWith('Нет связи'));
  const malformed=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>{throw new SyntaxError('invalid');}})});
  await assert.rejects(malformed.capabilities(),ContractError);
});
