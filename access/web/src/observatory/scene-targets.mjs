import {t} from './ui-i18n.mjs';
// Presentation targets retain exact raw identities. No scene ID is an API ID.
export function nodeTarget(model,id){
  if(!model?.rawNodesById.has(id))return null;
  return {kind:'node',nodeId:id,relationId:null,pathId:null};
}
export function relationTarget(model,id){
  const raw=model?.rawRelationsById.get(id);if(!raw)return null;
  return {kind:'relation',nodeId:raw.from_id,relationId:id,pathId:null};
}
export function pathTarget(model,id){
  const path=model?.pathsById.get(id);if(!path)return null;
  return {kind:'claim-path',nodeId:path.claim_node_id,relationId:null,pathId:id};
}
export function targetAnchor(model,target){
  if(!target)return null;
  const vertex=model.carrierToVertex.get(target.nodeId);
  if(model.verticesById.has(vertex))return vertex;
  const path=target.pathId?model.pathsById.get(target.pathId):[...model.pathsById.values()].find(p=>p.claim_node_id===target.nodeId);
  if(path&&model.verticesById.has(path.from_id))return path.from_id;
  if(target.relationId){
    const raw=model.rawRelationsById.get(target.relationId),other=model.carrierToVertex.get(raw?.to_id);
    if(model.verticesById.has(other))return other;
  }
  return null;
}
export function restoreTarget(model,value){
  if(!value)return null;
  if(value.pathId){const target=pathTarget(model,value.pathId);return target?.nodeId===value.nodeId&&!value.relationId?target:null;}
  return value.relationId?relationTarget(model,value.relationId):nodeTarget(model,value.nodeId);
}
export function requireRestorableTarget(model,value){
  const target=restoreTarget(model,value);
  if(value?.pathId&&!target)throw new Error(t('Сохранённый путь утверждения больше недоступен. Откройте запись заново.'));
  return target;
}
export function sceneConnections(model,nodeId){
  if(!model)return [];
  const vertex=model.carrierToVertex.get(nodeId),result=model.edges.filter(edge=>edge.fromId===vertex||edge.toId===vertex);
  // A folded Claim remains an inspectable source selection. Its own complete
  // path is a connection even though its carrier is no longer a visible star.
  for(const edge of model.edges)if(edge.kind==='claim-path'&&edge.path.claim_node_id===nodeId&&!result.includes(edge))result.push(edge);
  return result;
}
