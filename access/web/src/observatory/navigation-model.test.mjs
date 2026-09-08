import {test,expect} from 'vitest';
import {ContractError,RevisionError,validateExploration,projectLens} from './knowledge-client.mjs';
import {pathAvailable,explorationQuery,validateNativePaths,nativePathSpec,bindPath,loadPaths} from './navigation-model.mjs';

const rev='a'.repeat(64),content='b'.repeat(64),boundary={is_source:false,is_canon:false,writes_to_tree:false};
const node=(id,native)=>({id,native_id:native,source_graph:'philosophy',content_revision:content,source_refs:['ToS/fixture.json'],display:{title:{ru:native}}});
const a=node('opaque/α:'+ 'long'.repeat(50),'native:a'),b=node('other/β','native:b');
const edge={...node('opaque/edge','native:e'),from_id:a.id,to_id:b.id,display:{label:{ru:'Связь'}}};
const legacyNode=n=>({node_id:n.native_id,source_refs:n.source_refs});
const legacyEdge={edge_id:edge.native_id,from_id:a.native_id,to_id:b.native_id,source_refs:edge.source_refs};
const path={node_ids:[a.native_id,b.native_id],edge_ids:[edge.native_id],nodes:[legacyNode(a),legacyNode(b)],edges:[legacyEdge],
  traversal:[{edge_id:edge.native_id,from_node_id:a.native_id,to_node_id:b.native_id,edge_direction:'forward'}]};
const lens={schema:'tos_lens_result_v1',source_revision:rev,authority_boundary:boundary,focus:{node_id:a.id},nodes:[a,b],relations:[edge]};
const options={direction:'either',maxDepth:6,alternativeLimit:3,excluded:[]};
const native={schema:'tos_philosophy_mcp_path_v2',from_id:a.native_id,to_id:b.native_id,direction:'either',max_depth:6,alternative_limit:3,
  found:true,path_count:1,paths:[path],excluded_edge_ids:[],exploration_truncated:false};
const exploration={schema:'tos_exploration_result_v1',source_revision:rev,snapshot_revision:'c'.repeat(64),authority_boundary:boundary,writes_to_tree:false,
  nodes:[a,b],relations:[edge],focus:{node_id:a.id},query:explorationQuery(a.id),status:'paused',limit_reason:null,
  page:{number:1,primary_node_ids:[a.id,b.id],context_node_ids:[],next_cursor:'d'.repeat(64),returned_nodes:2,returned_relations:1,scope:'resumable-neighborhood'},
  counts:{discovered_nodes:2,emitted_relations:1,scope:'cumulative-discovered-not-global-total'},inclusion:{authority:'query-execution-not-semantic-proof'}};

test('paths bind by explicit source/native identity, never by prefix or label',()=>{
  expect(pathAvailable({...a,id:'philosophy:fake',source_graph:'source-navigation'})).toBe(false);
  expect(pathAvailable({...a,native_id:undefined})).toBe(false);
  expect(validateNativePaths(native,a,b,options)).toBe(native);
  const bound=bindPath(path,lens,a,b);
  expect(bound.node_ids).toEqual([a.id,b.id]);expect(bound.edge_ids).toEqual([edge.id]);
  expect(bound.packet).toBe(lens);
  expect(nativePathSpec(path,a).node_query.filters[0].value).toEqual(path.node_ids);
  for(const change of [p=>p.nodes[1].source_graph='canon',p=>p.nodes[1].native_id=a.native_id,
    p=>p.nodes[1].source_refs=['unrelated'],p=>p.relations[0].to_id=a.id,p=>p.nodes.pop()]){
    const p=structuredClone(lens);change(p);expect(()=>bindPath(path,p,a,b)).toThrow(ContractError);
  }
});

test('direction, endpoint order, exclusions and unique simple paths constrain every displayed hop',()=>{
  for(const change of [p=>p.from_id=b.native_id,p=>p.paths[0].node_ids.reverse(),p=>p.paths[0].traversal[0].edge_direction='reverse',
    p=>p.paths[0].edges[0].to_id='outside',p=>p.paths.push(p.paths[0]),p=>p.excluded_edge_ids=[edge.native_id],p=>p.found=false]){
    const p=structuredClone(native);change(p);expect(()=>validateNativePaths(p,a,b,options)).toThrow(ContractError);
  }
  const backwards=structuredClone(native);backwards.from_id=b.native_id;backwards.to_id=a.native_id;
  const back=backwards.paths[0];back.node_ids.reverse();back.nodes.reverse();back.traversal=[{edge_id:edge.native_id,from_node_id:b.native_id,to_node_id:a.native_id,edge_direction:'reverse'}];
  expect(validateNativePaths(backwards,b,a,options)).toBe(backwards);
  backwards.direction='outgoing';expect(()=>validateNativePaths(backwards,b,a,{...options,direction:'outgoing'})).toThrow(ContractError);
});

test('no-path and truncated search remain bounded findings without invented alternatives',()=>{
  const empty={...native,found:false,path_count:0,paths:[],exploration_truncated:true};
  expect(validateNativePaths(empty,a,b,options).exploration_truncated).toBe(true);
});

test('surrounding knowledge checks reject content drift and bind every route through backend compilation',async()=>{
  let checks=0,compiles=0;
  const client={inspect:async(kind,id,signal,revision,expected)=>{checks++;expect(revision).toBe(rev);expect(expected).toBe(content);},
    compile:async(spec,signal,revision)=>{compiles++;expect(spec.sources).toEqual(['philosophy']);expect(revision).toBe(rev);return lens;}};
  const queries={invoke:async(name,input)=>{expect(name).toBe('tos.path.find');expect(input.from_id).toBe(a.native_id);return native;}};
  const result=await loadPaths(a,b,rev,{client,queries});expect(checks).toBe(4);expect(compiles).toBe(1);expect(result.paths[0].node_ids[0]).toBe(a.id);
  client.inspect=async()=>{throw new RevisionError();};await expect(loadPaths(a,b,rev,{client,queries})).rejects.toThrow(RevisionError);
  await expect(loadPaths(a,{...b,source_graph:'canon'},rev,{client,queries})).rejects.toThrow('философского графа');
});

test('exploration preserves its schema and positions, enforcing closure and the frame budget',()=>{
  expect(validateExploration(exploration,rev)).toBe(exploration);
  const old=projectLens(lens);const next=projectLens(exploration,old);
  expect(next.map(n=>n.target)).toEqual(old.map(n=>n.target));expect(exploration.schema).toBe('tos_exploration_result_v1');
  const query=explorationQuery(a.id);expect(1+query.page_nodes+2*query.page_relations).toBeLessThanOrEqual(40);
  for(const change of [p=>p.relations[0].to_id='absent',p=>p.page.next_cursor=null,p=>p.writes_to_tree=true,
    p=>p.page.primary_node_ids=['absent'],p=>p.page.returned_nodes=3,p=>p.counts.scope='global-total',
    p=>p.nodes=Array.from({length:41},(_,i)=>({...a,id:String(i)}))]){
    const p=structuredClone(exploration);change(p);expect(()=>validateExploration(p)).toThrow(ContractError);
  }
});

test('resumed exploration requires the same snapshot, focus, query and next page',()=>{
  const next={...exploration,page:{...exploration.page,number:2,next_cursor:null},status:'complete'};
  expect(validateExploration(next,rev,exploration)).toBe(next);
  for(const change of [p=>p.snapshot_revision='e'.repeat(64),p=>p.page.number=4,p=>p.focus.node_id=b.id,p=>p.query.max_depth=3]){
    const p=structuredClone(next);change(p);expect(()=>validateExploration(p,rev,exploration)).toThrow(RevisionError);
  }
});
