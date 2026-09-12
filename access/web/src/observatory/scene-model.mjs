import {claimPathClosure,resolveClaimReading} from './human-forms.mjs';

// The access client already caps LensResult delivery at these values. Keep the
// presentation adapter bounded as well when it is used directly by a fixture,
// a restored place, or a future consumer.
export const SCENE_LIMITS=Object.freeze({nodes:40,relations:80,vertices:40,arcs:80,paths:40});
// This separate consumer owns a retained, bounded local view. These limits do
// not change LensResult delivery and cannot be selected through caller numbers.
export const EXPLORATION_SCENE_LIMITS=Object.freeze({nodes:200,relations:600,vertices:200,arcs:600,paths:200});
const EXPLORATION_VIEW_SCHEMA='tos_browser_exploration_view_v1';

const SCENE_SCHEMA='tos_knowledge_scene_v1';
const SCENE_SCOPE='returned-packet-only';
const SCENE_IDENTITY='declared-tos-entity-id';
const SCENE_AUTHORITY='presentation-mapping-not-semantic-admission';
const COMPACT_RULE='explicit-claim-paths-v1';
const COMPACT_AUTHORITY='presentation-only-no-new-assertion';
const PROJECT_RELATION='tos.relation.projects';
const RETAINED_CLAIM_REASONS=new Set([
  'incomplete-claim-contract',
  'unmapped-claim-predicate',
  'claim-endpoint-identity-collision',
  'incomplete-or-ambiguous-path',
  'focus-claim',
  'focus-relation',
  'mixed-or-incomplete-claim-carriers',
  'incomplete-value-member-context',
  'nonfoldable-incident-relation',
  'focus-detail',
]);

const hasOwn=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const string=value=>typeof value==='string'&&value.length>0;
const declaredEntity=value=>typeof value==='string'&&value.startsWith('tos.');

export class SceneContractError extends Error {
  constructor(message='Сцена графа не соответствует контракту.'){
    super(message);
    this.name='SceneContractError';
  }
}

function requireScene(condition){
  if(!condition)throw new SceneContractError();
}

function exactKeys(value,keys,required=keys){
  requireScene(object(value));
  const allowed=new Set(keys);
  requireScene(Object.keys(value).every(key=>allowed.has(key))&&required.every(key=>hasOwn(value,key)));
}

function ids(value,{max=Infinity,min=0,unique=true}={}){
  requireScene(Array.isArray(value)&&value.length>=min&&value.length<=max&&value.every(string));
  if(unique)requireScene(new Set(value).size===value.length);
  return value;
}

function boundedPacket(packet,limits){
  requireScene(object(packet));
  requireScene(Array.isArray(packet.nodes)&&packet.nodes.length<=limits.nodes);
  requireScene(Array.isArray(packet.relations)&&packet.relations.length<=limits.relations);
  const rawNodesById=new Map(),rawRelationsById=new Map();
  for(const node of packet.nodes){
    requireScene(object(node)&&string(node.id)&&!rawNodesById.has(node.id));
    rawNodesById.set(node.id,node);
  }
  for(const relation of packet.relations){
    requireScene(object(relation)&&string(relation.id)&&!rawRelationsById.has(relation.id)
      &&string(relation.from_id)&&string(relation.to_id)
      &&rawNodesById.has(relation.from_id)&&rawNodesById.has(relation.to_id));
    rawRelationsById.set(relation.id,relation);
  }
  return {rawNodesById,rawRelationsById};
}

function focusBinding(packet,rawNodesById,rawRelationsById){
  const focus=packet.focus;
  if(focus===undefined||focus===null)return {nodeId:null,relationId:null};
  requireScene(object(focus));
  // LensResult v1 requires node_id. relation_id is accepted as the additive
  // local focus ABI used by scene producers to protect a selected edge.
  requireScene(string(focus.node_id)&&rawNodesById.has(focus.node_id));
  const relationId=focus.relation_id??focus.focus_relation_id??null;
  requireScene(relationId===null||(string(relationId)&&rawRelationsById.has(relationId)));
  return {nodeId:focus.node_id,relationId};
}

function explorationFocusBinding(view,rawNodesById,rawRelationsById){
  const revision=value=>typeof value==='string'&&value.length===64&&/^[a-f0-9]+$/.test(value);
  requireScene(view.schema===EXPLORATION_VIEW_SCHEMA&&view.scope==='bounded-retained-exploration-carriers'
    &&view.writes_to_tree===false&&view.authority_boundary?.is_source===false
    &&view.authority_boundary?.is_canon===false&&view.authority_boundary?.writes_to_tree===false
    &&revision(view.source_revision)&&revision(view.snapshot_revision)
    &&['tos-exploration-execution-v6','tos-exploration-d1-execution-v6'].includes(view.execution_version)
    &&!hasOwn(view,'focus'));
  const origin=view.origin;
  requireScene(object(origin)&&['node','relation'].includes(origin.kind));
  exactKeys(origin,origin.kind==='relation'?['kind','id','content_revision','endpoints']:['kind','id','content_revision']);
  requireScene(string(origin.id)&&revision(origin.content_revision));
  const originRow=(origin.kind==='node'?rawNodesById:rawRelationsById).get(origin.id);
  requireScene(originRow&&originRow.content_revision===origin.content_revision);
  if(origin.kind==='relation'){
    exactKeys(origin.endpoints,['from','to']);
    for(const side of ['from','to']){
      const endpoint=origin.endpoints[side],node=rawNodesById.get(originRow[`${side}_id`]);
      exactKeys(endpoint,['node_id','entity_id','content_revision']);
      requireScene(node&&endpoint.node_id===node.id&&endpoint.entity_id===node.entity_id
        &&revision(endpoint.content_revision)&&endpoint.content_revision===node.content_revision);
    }
  }
  // Resolve private validation focus from exact raw selectors. Do not add a
  // fake LensResult focus or replace the local view's own schema/origin.
  const selection=view.selection??{kind:origin.kind,id:origin.id};
  requireScene(object(selection)&&['node','relation','claim-path'].includes(selection.kind));
  exactKeys(selection,selection.kind==='claim-path'?['kind','id','claimId']:['kind','id']);
  requireScene(string(selection.id));
  if(selection.kind==='relation'){
    const relation=rawRelationsById.get(selection.id);requireScene(relation);
    return {nodeId:relation.from_id,relationId:relation.id,pathId:null};
  }
  const nodeId=selection.kind==='claim-path'?selection.claimId:selection.id;
  requireScene(string(nodeId)&&rawNodesById.has(nodeId));
  return {nodeId,relationId:null,pathId:selection.kind==='claim-path'?selection.id:null};
}

// Raw mode intentionally exposes each carrier under its own API node ID. It
// has no declared scene identity to prefix or reinterpret.
function rawCarrierVertex(nodeId){return nodeId;}

function relationEdge(relation,fromId,toId){
  return {id:relation.id,kind:'relation',fromId,toId,rawId:relation.id};
}

function rawProjection(packet,rawNodesById,rawRelationsById){
  const carrierToVertex=new Map(),vertices=[],verticesById=new Map();
  for(const node of packet.nodes){
    const id=rawCarrierVertex(node.id),vertex={id,nodeIds:[node.id],representativeId:node.id};
    carrierToVertex.set(node.id,id);vertices.push(vertex);verticesById.set(id,vertex);
  }
  const edges=packet.relations.map(relation=>relationEdge(relation,carrierToVertex.get(relation.from_id),carrierToVertex.get(relation.to_id)));
  return {mode:'raw',declared:null,vertices,edges,rawNodesById,rawRelationsById,carrierToVertex,verticesById,pathsById:new Map()};
}

function sceneVertex(value,limits){
  exactKeys(value,['id','entity_id','node_ids','representative_node_id']);
  requireScene(string(value.id)&&(value.entity_id===null||declaredEntity(value.entity_id)));
  ids(value.node_ids,{max:limits.nodes,min:1});
  requireScene(value.node_ids.includes(value.representative_node_id));
}

function validateReadingShape(reading,limits){
  exactKeys(reading,['mode','node_id','content_revision','wording_pointer','wording_state','context_pointers','relation_context_ids','standalone']);
  requireScene(string(reading.node_id)&&/^[a-f0-9]{64}$/.test(reading.content_revision));
  requireScene(reading.wording_pointer===null||string(reading.wording_pointer));
  requireScene(['available','missing'].includes(reading.wording_state));
  requireScene(Array.isArray(reading.context_pointers)&&reading.context_pointers.length===2
    &&reading.context_pointers[0]==='/semantics'&&reading.context_pointers[1]==='/epistemic');
  ids(reading.relation_context_ids,{max:limits.relations});
  requireScene(['claim-with-mandatory-context','claim-with-shared-form-context-v2'].includes(reading.mode)&&reading.standalone===false);
  requireScene(reading.wording_state==='missing'?reading.wording_pointer===null:reading.wording_pointer!==null);
}

function pathShape(path,limits){
  exactKeys(path,['id','from_id','to_id','claim_node_id','relation_type_id','node_ids','relation_ids','detail_relation_ids','reading']);
  requireScene(string(path.id)&&string(path.from_id)&&string(path.to_id)&&string(path.claim_node_id)&&string(path.relation_type_id));
  ids(path.node_ids,{max:3,min:3});
  ids(path.relation_ids,{max:2,min:2});
  ids(path.detail_relation_ids,{max:limits.relations});
  validateReadingShape(path.reading,limits);
}

function validateScene(packet,rawNodesById,rawRelationsById,focus,limits){
  const scene=packet.scene;
  exactKeys(scene,['schema_version','vertices','arcs','collapsed_relation_ids','focus_vertex_id','compact','scope','identity_rule','authority'],
    ['schema_version','vertices','arcs','collapsed_relation_ids','focus_vertex_id','scope','identity_rule','authority']);
  requireScene(scene.schema_version===SCENE_SCHEMA&&scene.scope===SCENE_SCOPE
    &&scene.identity_rule===SCENE_IDENTITY&&scene.authority===SCENE_AUTHORITY);
  requireScene(Array.isArray(scene.vertices)&&scene.vertices.length<=limits.vertices);
  requireScene(Array.isArray(scene.arcs)&&scene.arcs.length<=limits.arcs);
  ids(scene.collapsed_relation_ids,{max:limits.relations});
  requireScene(scene.focus_vertex_id===null||string(scene.focus_vertex_id));

  const verticesById=new Map(),carrierToSceneVertex=new Map(),entityToVertex=new Map();
  for(const vertex of scene.vertices){
    sceneVertex(vertex,limits);
    requireScene(!verticesById.has(vertex.id));
    verticesById.set(vertex.id,vertex);
    if(vertex.entity_id!==null){
      requireScene(!entityToVertex.has(vertex.entity_id));
      entityToVertex.set(vertex.entity_id,vertex.id);
    }else {
      // A null entity is an explicit absence of identity. Grouping such
      // carriers would manufacture identity from their presentation shape.
      requireScene(vertex.node_ids.length===1);
    }
    for(const nodeId of vertex.node_ids){
      requireScene(rawNodesById.has(nodeId)&&!carrierToSceneVertex.has(nodeId));
      const nodeEntity=declaredEntity(rawNodesById.get(nodeId).entity_id)?rawNodesById.get(nodeId).entity_id:null;
      requireScene(nodeEntity===vertex.entity_id);
      carrierToSceneVertex.set(nodeId,vertex.id);
    }
  }
  requireScene(carrierToSceneVertex.size===rawNodesById.size);
  requireScene(scene.focus_vertex_id===null?focus.nodeId===null:verticesById.has(scene.focus_vertex_id)
    &&scene.focus_vertex_id===carrierToSceneVertex.get(focus.nodeId));

  const arcs=[],arcById=new Map();
  for(const arc of scene.arcs){
    exactKeys(arc,['relation_id','from_id','to_id']);
    requireScene(string(arc.relation_id)&&string(arc.from_id)&&string(arc.to_id)
      &&rawRelationsById.has(arc.relation_id)&&verticesById.has(arc.from_id)&&verticesById.has(arc.to_id)
      &&!arcById.has(arc.relation_id));
    const relation=rawRelationsById.get(arc.relation_id);
    requireScene(carrierToSceneVertex.get(relation.from_id)===arc.from_id
      &&carrierToSceneVertex.get(relation.to_id)===arc.to_id);
    const value={id:arc.relation_id,fromId:arc.from_id,toId:arc.to_id,relation};
    arcs.push(value);arcById.set(value.id,value);
  }
  const collapsed=new Set(scene.collapsed_relation_ids);
  for(const relationId of collapsed){
    requireScene(rawRelationsById.has(relationId)&&!arcById.has(relationId));
    const relation=rawRelationsById.get(relationId),from=carrierToSceneVertex.get(relation.from_id),to=carrierToSceneVertex.get(relation.to_id);
    requireScene(from===to&&relation.relation_type_id===PROJECT_RELATION&&relationId!==focus.relationId);
  }
  requireScene(arcById.size+collapsed.size===rawRelationsById.size);
  return {scene,verticesById,carrierToSceneVertex,arcs,arcById,collapsed,entityToVertex};
}

function validateCompact(packet,sceneData,rawNodesById,rawRelationsById,focus,limits){
  const hasCompact=hasOwn(sceneData.scene,'compact'),compact=sceneData.scene.compact;
  if(!hasCompact)return null;
  requireScene(object(compact));
  exactKeys(compact,['rule','vertex_ids','relation_ids','claim_paths','folded_vertex_ids','retained_claims','authority']);
  requireScene(compact.rule===COMPACT_RULE&&compact.authority===COMPACT_AUTHORITY);
  ids(compact.vertex_ids,{max:limits.vertices});
  ids(compact.folded_vertex_ids,{max:limits.vertices});
  ids(compact.relation_ids,{max:limits.relations});
  requireScene(Array.isArray(compact.claim_paths)&&compact.claim_paths.length<=limits.paths);
  requireScene(Array.isArray(compact.retained_claims)&&compact.retained_claims.length<=limits.nodes);

  const allVertexIds=new Set(sceneData.verticesById.keys()),visibleIds=new Set(compact.vertex_ids),foldedIds=new Set(compact.folded_vertex_ids);
  requireScene([...visibleIds,...foldedIds].every(id=>allVertexIds.has(id))
    &&visibleIds.size+foldedIds.size===allVertexIds.size
    &&[...visibleIds].every(id=>!foldedIds.has(id)));
  const arcIds=new Set(sceneData.arcById.keys()),relationIds=new Set(compact.relation_ids);
  requireScene([...relationIds].every(id=>arcIds.has(id)));
  // A selected raw relation cannot disappear into a Claim path either. The
  // producer retains it; this consumer rejects rather than repairs a mismatch.
  requireScene(focus.relationId===null||relationIds.has(focus.relationId));

  const pathsById=new Map(),pathRelationIds=new Set(),pathClaimIds=new Set();
  for(const path of compact.claim_paths){
    pathShape(path,limits);
    requireScene(!pathsById.has(path.id)&&!pathClaimIds.has(path.claim_node_id)
      &&rawNodesById.has(path.claim_node_id));
    requireScene(path.node_ids[1]===path.claim_node_id);
    const from=sceneData.carrierToSceneVertex.get(path.node_ids[0]),to=sceneData.carrierToSceneVertex.get(path.node_ids[2]);
    requireScene(from===path.from_id&&to===path.to_id&&visibleIds.has(path.from_id)&&visibleIds.has(path.to_id));
    try {
      // These owner helpers validate the complete exact path and its reading
      // envelope. Their return values are intentionally discarded: the
      // packet's path and records remain the references exposed by this model.
      claimPathClosure(packet,path);
      resolveClaimReading(packet,path.reading);
    } catch(error) {
      if(error instanceof SceneContractError)throw error;
      throw new SceneContractError();
    }
    for(const relationId of [...path.relation_ids,...path.detail_relation_ids]){
      requireScene(arcIds.has(relationId)&&!pathRelationIds.has(relationId));
      pathRelationIds.add(relationId);
    }
    pathsById.set(path.id,path);pathClaimIds.add(path.claim_node_id);
  }

  for(const retained of compact.retained_claims){
    exactKeys(retained,['node_id','reason']);
    requireScene(string(retained.node_id)&&rawNodesById.has(retained.node_id)
      &&typeof retained.reason==='string'&&RETAINED_CLAIM_REASONS.has(retained.reason));
    const vertex=sceneData.carrierToSceneVertex.get(retained.node_id);
    requireScene(visibleIds.has(vertex));
  }
  const retainedIds=new Set(compact.retained_claims.map(value=>value.node_id));
  requireScene(retainedIds.size===compact.retained_claims.length);

  // Every retained arc is either displayed directly or consumed by exactly one
  // complete path. This is the relation-level no-silent-drop boundary.
  requireScene([...relationIds].every(id=>!pathRelationIds.has(id))
    &&relationIds.size+pathRelationIds.size===arcIds.size
    &&[...arcIds].every(id=>relationIds.has(id)||pathRelationIds.has(id)));

  // A folded Claim must remain reachable through its exact path. The claim
  // test uses only explicit source type markers; it does not classify labels.
  for(const node of rawNodesById.values()){
    const isClaim=node.type_id==='tos.entity.claim'||node.semantics?.type_ancestors?.includes?.('tos.entity.claim');
    const vertex=sceneData.carrierToSceneVertex.get(node.id);
    if(isClaim&&!visibleIds.has(vertex))requireScene(pathClaimIds.has(node.id));
  }
  return {compact,visibleIds,foldedIds,relationIds,pathsById,pathRelationIds,pathClaimIds,retainedIds};
}

function descriptor(vertex){
  return {id:vertex.id,nodeIds:vertex.node_ids.slice(),representativeId:vertex.representative_node_id};
}

function sceneProjection(packet,sceneData,compactData,rawNodesById,rawRelationsById,focus,mode){
  const scene=sceneData.scene,compact=compactData?.compact;
  const effectiveMode=mode==='compact'&&compact===undefined?'grouped':mode;
  const carrierToVertex=new Map(),vertices=[],verticesById=new Map();
  let pathsById=compactData?.pathsById||new Map();
  if(effectiveMode==='raw'){
    const raw=rawProjection(packet,rawNodesById,rawRelationsById);
    return {...raw,declared:scene,pathsById};
  }
  for(const vertex of scene.vertices){
    for(const nodeId of vertex.node_ids)carrierToVertex.set(nodeId,vertex.id);
  }
  const visible=effectiveMode==='compact'?compactData.visibleIds:new Set(scene.vertices.map(vertex=>vertex.id));
  for(const vertex of scene.vertices)if(visible.has(vertex.id)){
    const value=descriptor(vertex);vertices.push(value);verticesById.set(value.id,value);
  }
  const relationEdgeById=sceneData.arcById;
  const edges=[];
  const relationIds=effectiveMode==='compact'?[...compactData.relationIds]:sceneData.arcs.map(arc=>arc.id);
  for(const relationId of relationIds){
    const arc=relationEdgeById.get(relationId);requireScene(arc&&verticesById.has(arc.fromId)&&verticesById.has(arc.toId));
    edges.push(relationEdge(arc.relation,arc.fromId,arc.toId));
  }
  if(effectiveMode==='compact')for(const path of compactData.pathsById.values()){
    requireScene(verticesById.has(path.from_id)&&verticesById.has(path.to_id));
    edges.push({id:path.id,kind:'claim-path',fromId:path.from_id,toId:path.to_id,path});
  }
  return {mode:effectiveMode,declared:scene,vertices,edges,rawNodesById,rawRelationsById,carrierToVertex,verticesById,pathsById};
}

function buildModel(packet,mode,limits,exploration){
  try {
    requireScene(['compact','grouped','raw'].includes(mode));
    const {rawNodesById,rawRelationsById}=boundedPacket(packet,limits);
    requireScene(exploration||packet.schema!==EXPLORATION_VIEW_SCHEMA);
    const focus=(exploration?explorationFocusBinding:focusBinding)(packet,rawNodesById,rawRelationsById);
    if(!hasOwn(packet,'scene')){
      requireScene(!exploration);
      return rawProjection(packet,rawNodesById,rawRelationsById);
    }
    requireScene(object(packet.scene));
    const sceneData=validateScene(packet,rawNodesById,rawRelationsById,focus,limits);
    const compactData=validateCompact(packet,sceneData,rawNodesById,rawRelationsById,focus,limits);
    if(exploration){
      requireScene(compactData&&compactData.visibleIds.has(sceneData.carrierToSceneVertex.get(focus.nodeId)));
      requireScene(focus.pathId===null||compactData.pathsById.get(focus.pathId)?.claim_node_id===focus.nodeId);
    }
    return sceneProjection(packet,sceneData,compactData,rawNodesById,rawRelationsById,focus,mode);
  } catch(error) {
    if(error instanceof SceneContractError)throw error;
    throw new SceneContractError();
  }
}

export function buildSceneModel(packet,{mode='compact'}={}){
  return buildModel(packet,mode,SCENE_LIMITS,false);
}

export function buildExplorationSceneModel(localView,{mode='compact'}={}){
  return buildModel(localView,mode,EXPLORATION_SCENE_LIMITS,true);
}
