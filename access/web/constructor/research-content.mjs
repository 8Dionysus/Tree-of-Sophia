const PROJECT_RELATION_TYPE='tos.relation.projects';
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);

// A type is safe to classify as a projection only when every declared entry
// for that exact type agrees on the same non-empty object role. Missing or
// conflicting declarations stay visible to the research surface.
export function projectionTypeIds(catalog){
  const roles=new Map();
  const entries=Array.isArray(catalog?.semantic_registries?.entity_types?.entries)
    ?catalog.semantic_registries.entity_types.entries:[];
  for(const entry of entries){
    if(!object(entry)||typeof entry.type_id!=='string'||!entry.type_id)continue;
    let values=roles.get(entry.type_id);
    if(!values){values=new Set();roles.set(entry.type_id,values);}
    values.add(typeof entry.object_role==='string'&&entry.object_role?entry.object_role:null);
  }
  return new Set([...roles].filter(([,values])=>values.size===1&&values.has('projection')).map(([typeId])=>typeId));
}

// Search results are a presentation list over the returned packet. They keep
// packet order and raw object identity; filtering never changes the packet.
export function researchSearchRows(page,catalog,{includeService=false}={}){
  const nodes=Array.isArray(page?.nodes)?page.nodes:[],relations=Array.isArray(page?.relations)?page.relations:[];
  if(includeService)return {rows:[...nodes.map(raw=>({kind:'node',raw})),...relations.map(raw=>({kind:'relation',raw}))],hiddenCount:0};
  const projectionTypes=projectionTypeIds(catalog),rows=[],hiddenNodes=nodes.filter(raw=>projectionTypes.has(raw?.type_id)).length;
  for(const raw of nodes)if(!projectionTypes.has(raw?.type_id))rows.push({kind:'node',raw});
  let hiddenRelations=0;
  for(const raw of relations){
    if(raw?.relation_type_id===PROJECT_RELATION_TYPE){hiddenRelations++;continue;}
    rows.push({kind:'relation',raw});
  }
  return {rows,hiddenCount:hiddenNodes+hiddenRelations};
}
