// Host observations preserve strict source identity. Only the selected Claim's
// IDs, type metadata and scalar observations cross WASM; wording stays here.
let validate;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const own=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const scalar=value=>typeof value==='string'?value:null;
const strings=value=>Array.isArray(value)&&value.every(item=>typeof item==='string');
function ids(value){
  if(!Array.isArray(value))return null;
  return Array.from({length:value.length},(_,i)=>i in value?scalar(value[i]):{hole:true});
}
function rule(op,input){
  if(!validate)throw new Error('Claim reading Rust rules are not installed');
  const wire=JSON.stringify({op,...input});
  const raw=encoder.encode(wire);
  return JSON.parse(decoder.decode(validate(raw)));
}
export function installClaimReadingRules(runtime){
  if(typeof runtime?.validate_claim_reading_wasm_v1!=='function')throw new TypeError('Claim reading WASM rule is unavailable');
  validate=runtime.validate_claim_reading_wasm_v1;
}
export function selectedClaimContext(packet,reading){
  // These scans observe original values. They do not serialize the graph.
  const node=packet.nodes?.find(item=>item.id===reading?.node_id);
  const relationIds=reading?.relation_context_ids;
  const relations=Array.isArray(relationIds)?relationIds.map(id=>packet.relations?.find(item=>item.id===id)):[];
  rule('context',{
    reading_object:object(reading),mode:scalar(reading?.mode),standalone_false:reading?.standalone===false,
    wording_state:scalar(reading?.wording_state),context_pointers_exact:JSON.stringify(reading?.context_pointers)===JSON.stringify(['/semantics','/epistemic']),
    relation_context_ids:strings(relationIds)?ids(relationIds):null,node_found:Boolean(node),
    revision_equal:Boolean(node)&&node.content_revision===reading?.content_revision,
    semantics_object:object(node?.semantics),epistemic_object:object(node?.epistemic),relations_found:relations.every(Boolean),
  });
  return {node,relations};
}
export function selectedClaimPath(packet,nodeId){
  const scene=packet?.scene,compact=scene?.compact,paths=compact?.claim_paths;
  const matches=[];
  if(Array.isArray(paths))for(let i=0;i<paths.length;i++)if(object(paths[i])&&paths[i].claim_node_id===nodeId)matches.push(i);
  const index=rule('path',{compact_absent:compact===undefined,scene_schema:scalar(scene?.schema_version),compact_object:object(compact),
    rule:scalar(compact?.rule),authority:scalar(compact?.authority),paths_array:Array.isArray(paths),matches});
  return index===null?null:paths[index];
}
export function selectedClaimClosure(packet,path,node,relations,owned){
  const claim=node.semantics.claim,primary=path.relation_ids,detail=path.detail_relation_ids;
  const relationIds=Array.isArray(primary)&&Array.isArray(detail)?[...primary,...detail]:[];
  const nodeIds=[...new Set([...(Array.isArray(path.node_ids)?path.node_ids:[]),...relations.flatMap(r=>[r.from_id,r.to_id])])];
  const allMembers=packet.relations.filter(r=>r.from_id===node.id&&r.relation_type_id==='tos.relation.claim-value-member');
  const memberRelations=Array.isArray(detail)?detail.map(id=>relations.find(r=>r.id===id)).filter(r=>r?.relation_type_id==='tos.relation.claim-value-member'):[];
  const descriptor=r=>r?{to_id:scalar(r.to_id),relation_type_id:scalar(r.relation_type_id),from_claim:r.from_id===node.id}:null;
  rule('closure',{
    path_object:object(path),path_owned:owned,id_nonempty:typeof path.id==='string'&&Boolean(path.id),relation_type_string:typeof path.relation_type_id==='string',
    node_ids:strings(path.node_ids)?ids(path.node_ids):null,relation_ids:strings(primary)?ids(primary):null,detail_relation_ids:strings(detail)?ids(detail):null,
    middle_equal:path.node_ids?.[1]===path.claim_node_id,reading_node_equal:path.reading?.node_id===path.claim_node_id,
    claim_object:object(claim),mapping_status:scalar(claim?.predicate_mapping_status),predicate_equal:claim?.relation_type_id===path.relation_type_id,
    subject_equal:claim?.subject_node_id===path.node_ids?.[0],object_equal:claim?.object_node_id===path.node_ids?.[2],
    endpoints_different:path.node_ids?.[0]!==node.id&&path.node_ids?.[2]!==node.id,
    relations:relations.map(descriptor),relation_set_complete:relations.every(r=>relationIds.includes(r.id)),
    primary_selected:Array.isArray(primary)?Array.from(primary,id=>descriptor(relations.find(r=>r.id===id))):null,
    detail_selected:Array.isArray(detail)?Array.from(detail,id=>descriptor(relations.find(r=>r.id===id))):null,
    has_member_declaration:object(claim)&&own(claim,'value_member_node_ids'),has_member_edges:allMembers.length>0,
    member_ids:strings(claim?.value_member_node_ids)?ids(claim.value_member_node_ids):null,
    member_count_complete:memberRelations.length===allMembers.length,
    closure_nodes_present:nodeIds.every(id=>typeof id==='string'&&packet.nodes.some(n=>n.id===id)),
  });
  return {nodeIds,relationIds,node};
}
export function selectedClaimWording(node,reading,forms){
  const pointer=reading.wording_pointer,field=typeof pointer==='string'?pointer.split('/')[3]:undefined;
  const display=node.display_selection?.fields?.[field];
  const selector=rule('wording',{
    mode:scalar(reading.mode),wording_state:scalar(reading.wording_state),pointer:scalar(pointer),pointer_null:pointer===null,
    selection_schema:scalar(node.human_form_selection?.schema_version),
    role_states:Object.fromEntries(['caption','statement','hover'].map(role=>[role,scalar(forms?.roles[role]?.state)])),
    role_selectors:{caption:'caption',statement:'statement',hover:'hover'},display_selector:'display',
    display_object:object(display),display_available:display?.content_available===true,
  });
  return selector===null?null:selector==='display'?display:forms.roles[selector].packet;
}
