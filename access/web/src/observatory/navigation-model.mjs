import {ContractError,RevisionError} from './knowledge-client.mjs';
import {sourceRefs} from './evidence-model.mjs';

export const pathAvailable=raw=>raw?.source_graph==='philosophy'&&typeof raw.native_id==='string'&&Boolean(raw.native_id);
const fail=()=>{throw new ContractError('Маршрут не удалось связать с текущими данными. Обновите область.');};
export function explorationQuery(id,{depth=2,direction='either',profile='overview'}={}){
  // Focus + primary nodes + both endpoints of every edge fit the 40-node scene.
  return {focus_node_id:id,max_depth:depth,direction,profile,page_nodes:10,page_relations:14};
}
export function pathSearchSpec(query){
  return {schema_version:'tos_lens_spec_v1',lens_id:'observatory-path-search',sources:['philosophy'],language:'ru',detail:'compact',
    seed:{text_query:query},traversal:{depth:0,profile:'all'},relation_query:{enabled:false},limits:{nodes:6,relations:0,groups:1}};
}
export function validateNativePaths(packet,start,end,{direction,maxDepth,alternativeLimit,excluded}){
  if(packet?.schema!=='tos_philosophy_mcp_path_v2'||packet.from_id!==start.native_id||packet.to_id!==end.native_id
    ||packet.direction!==direction||packet.max_depth!==maxDepth||packet.alternative_limit!==alternativeLimit
    ||!Array.isArray(packet.paths)||packet.paths.length>alternativeLimit||packet.path_count!==packet.paths.length
    ||packet.found!==(packet.paths.length>0)||typeof packet.exploration_truncated!=='boolean'
    ||JSON.stringify([...(packet.excluded_edge_ids||[])].sort())!==JSON.stringify(excluded.map(r=>r.native_id).sort()))fail();
  const seen=new Set();
  for(const path of packet.paths){
    const ids=path.node_ids,edgeIds=path.edge_ids;
    if(!Array.isArray(ids)||!Array.isArray(edgeIds)||!Array.isArray(path.nodes)||!Array.isArray(path.edges)||!Array.isArray(path.traversal)
      ||ids[0]!==start.native_id||ids.at(-1)!==end.native_id||ids.length!==edgeIds.length+1||edgeIds.length>maxDepth
      ||new Set(ids).size!==ids.length||new Set(edgeIds).size!==edgeIds.length
      ||path.nodes.length!==ids.length||path.edges.length!==edgeIds.length||path.traversal.length!==edgeIds.length)fail();
    const key=JSON.stringify(edgeIds);if(seen.has(key))fail();seen.add(key);
    for(let i=0;i<ids.length;i++)if(path.nodes[i].node_id!==ids[i])fail();
    for(let i=0;i<edgeIds.length;i++){
      const edge=path.edges[i],step=path.traversal[i],forward=edge.from_id===ids[i]&&edge.to_id===ids[i+1],reverse=edge.to_id===ids[i]&&edge.from_id===ids[i+1];
      if(edge.edge_id!==edgeIds[i]||excluded.some(r=>r.native_id===edgeIds[i])||(!forward&&!reverse)
        ||(direction==='outgoing'&&!forward)||(direction==='incoming'&&!reverse)
        ||step.edge_id!==edgeIds[i]||step.from_node_id!==ids[i]||step.to_node_id!==ids[i+1]
        ||step.edge_direction!==(forward?'forward':'reverse'))fail();
    }
  }
  return packet;
}
export function nativePathSpec(path,start){
  return {schema_version:'tos_lens_spec_v1',lens_id:'observatory-path',sources:['philosophy'],language:'ru',detail:'compact',
    seed:{focus_node_id:start.id},node_query:{filters:[{field:'native_id',op:'in',value:path.node_ids}]},
    relation_query:{filters:[{field:'native_id',op:'in',value:path.edge_ids}]},
    traversal:{depth:0,profile:'all'},limits:{nodes:9,relations:8,groups:1}};
}
export function bindPath(path,packet,start,end){
  function bind(records,legacy,kind){
    if(records.length!==legacy.length)fail();
    const result=new Map();
    for(const raw of records){
      const original=legacy.find(item=>item[kind==='node'?'node_id':'edge_id']===raw.native_id);
      if(!pathAvailable(raw)||result.has(raw.native_id)||!original||!sourceRefs(original).some(ref=>sourceRefs(raw).includes(ref)))fail();
      result.set(raw.native_id,raw);
    }
    return result;
  }
  const nodes=bind(packet.nodes,path.nodes,'node'),edges=bind(packet.relations,path.edges,'relation');
  if(nodes.get(start.native_id)?.id!==start.id||nodes.get(end.native_id)?.id!==end.id)fail();
  for(const edge of path.edges){
    const raw=edges.get(edge.edge_id);
    if(raw.from_id!==nodes.get(edge.from_id)?.id||raw.to_id!==nodes.get(edge.to_id)?.id)fail();
  }
  return {packet,node_ids:path.node_ids.map(id=>nodes.get(id).id),edge_ids:path.edge_ids.map(id=>edges.get(id).id),
    nodes:path.node_ids.map(id=>nodes.get(id)),edges:path.edge_ids.map(id=>edges.get(id)),
    traversal:path.traversal.map(step=>({edge_id:edges.get(step.edge_id).id,
      from_node_id:nodes.get(step.from_node_id).id,to_node_id:nodes.get(step.to_node_id).id,edge_direction:step.edge_direction}))};
}
export async function loadPaths(start,end,revision,{client,queries,signal,direction='either',maxDepth=6,alternativeLimit=3,excluded=[]}){
  if(!pathAvailable(start)||!pathAvailable(end)||excluded.some(r=>!pathAvailable(r)))throw new Error('Маршруты пока доступны между объектами философского графа.');
  if(start.id===end.id)throw new Error('Выберите две разные звезды.');
  if(!['outgoing','incoming','either'].includes(direction)||!Number.isInteger(maxDepth)||maxDepth<1||maxDepth>8
    ||!Number.isInteger(alternativeLimit)||alternativeLimit<1||alternativeLimit>5||excluded.length>64)throw new Error('Выберите глубину от 1 до 8 и до 5 вариантов.');
  // The older endpoint represents exclusion lists with commas, not opaque arrays.
  if(excluded.some(r=>r.native_id.includes(',')))throw new Error('Эту связь пока нельзя исключить через доступный маршрут поиска.');
  const subjects=[[start,'node'],[end,'node'],...excluded.map(r=>[r,'relation'])];
  const check=()=>Promise.all(subjects.map(([raw,kind])=>client.inspect(kind,raw.id,signal,revision,raw.content_revision)));
  await check();signal?.throwIfAborted();
  const options={direction,maxDepth,alternativeLimit,excluded};
  const native=validateNativePaths(await queries.invoke('tos.path.find',{from_id:start.native_id,to_id:end.native_id,
    direction,max_depth:maxDepth,alternative_limit:alternativeLimit,excluded_edge_ids:excluded.map(r=>r.native_id)},{signal}),start,end,options);
  const paths=await Promise.all(native.paths.map(async path=>bindPath(path,await client.compile(nativePathSpec(path,start),signal,revision),start,end)));
  await check();signal?.throwIfAborted();
  // No atomic revision token exists on the legacy path endpoint. Every displayed
  // hop is nevertheless rebound through exact native IDs, provenance and endpoints.
  for(const path of paths)for(const raw of [start,end])if(path.nodes.find(n=>n.id===raw.id)?.content_revision!==raw.content_revision)throw new RevisionError();
  return {schema:'tos_observatory_paths_v1',from_id:start.id,to_id:end.id,source_revision:revision,
    found:paths.length>0,path_count:paths.length,paths,direction,max_depth:maxDepth,alternative_limit:alternativeLimit,
    excluded_edge_ids:excluded.map(r=>r.id),exploration_truncated:native.exploration_truncated,
    next_actions:paths.length?['inspect a route node or relation','try excluding a relation']:['change direction or depth','restore an excluded relation']};
}
