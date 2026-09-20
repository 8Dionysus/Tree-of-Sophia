// The scene contract retains every carrier. This local view only sets aside
// objects explicitly classified as projections by the selected ToS catalog.
// Missing classifications and mixed groups remain visible.
export function researchVisibility(model,{catalog,selection,mode='compact'}={}){
  const roles=new Map();
  for(const entry of catalog?.semantic_registries?.entity_types?.entries??[]){
    if(!roles.has(entry.type_id))roles.set(entry.type_id,new Set());
    roles.get(entry.type_id).add(entry.object_role);
  }
  const protectedVertices=new Set();
  const protect=id=>{const vertex=model.carrierToVertex.get(id);if(vertex)protectedVertices.add(vertex);};
  if(selection?.kind==='node')protect(selection.id);
  if(selection?.kind==='relation'){
    const relation=model.rawRelationsById.get(selection.id);
    if(relation){protect(relation.from_id);protect(relation.to_id);}
  }
  if(selection?.kind==='claim-path'){
    protect(selection.claimId);
    const path=model.pathsById.get(selection.id);
    if(path){
      protectedVertices.add(path.from_id);protectedVertices.add(path.to_id);
      for(const id of path.node_ids??[])protect(id);
      for(const id of path.detail_relation_ids??[]){
        const relation=model.rawRelationsById.get(id);
        if(relation){protect(relation.from_id);protect(relation.to_id);}
      }
    }
  }
  const hidden=mode==='compact'?model.vertices.filter(vertex=>!protectedVertices.has(vertex.id)&&vertex.nodeIds.length>0&&vertex.nodeIds.every(id=>{
    const role=roles.get(model.rawNodesById.get(id)?.type_id);
    return role?.size===1&&role.has('projection');
  })):[];
  const hiddenIds=new Set(hidden.map(vertex=>vertex.id));
  const vertices=model.vertices.filter(vertex=>!hiddenIds.has(vertex.id));
  const edges=model.edges.filter(edge=>!hiddenIds.has(edge.fromId)&&!hiddenIds.has(edge.toId));
  return {...model,vertices,edges,verticesById:new Map(vertices.map(vertex=>[vertex.id,vertex])),
    visibility:{hiddenObjects:hidden.length,hiddenRelations:model.edges.length-edges.length}};
}
