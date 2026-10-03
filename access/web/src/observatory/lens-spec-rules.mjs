let focusDescriptor,relationDescriptor;
export function installLensSpecRules(runtime){
  if(typeof runtime?.focus_spec_descriptor_wasm_v1!=='function'||typeof runtime?.relation_spec_descriptor_wasm_v1!=='function')
    throw new TypeError('Generated browser lens recipes are unavailable');
  focusDescriptor=runtime.focus_spec_descriptor_wasm_v1;
  relationDescriptor=runtime.relation_spec_descriptor_wasm_v1;
}
export function browserFocusSpec(id,depth){
  if(!focusDescriptor)throw new Error('Browser lens recipes are not installed');
  // Parse only the fixed output container. Input values never cross JSON.
  const spec=JSON.parse(focusDescriptor());
  spec.seed.focus_node_id=id;spec.traversal.depth=depth;
  return spec;
}
export function browserRelationSpec(relation){
  const focusId=relation.from_id;
  if(!relationDescriptor)throw new Error('Browser lens recipes are not installed');
  const patch=JSON.parse(relationDescriptor()),spec=patch.focus;
  spec.seed.focus_node_id=focusId;
  patch.node_query.filters[0].value[0]=relation.from_id;
  patch.node_query.filters[0].value[1]=relation.to_id;
  spec.node_query=patch.node_query;spec.traversal.profile=patch.profile;
  patch.relation_query.filters[0].value=relation.id;
  spec.relation_query=patch.relation_query;spec.limits=patch.limits;
  return spec;
}
