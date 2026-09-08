const own=(value,key)=>value!==null&&typeof value==='object'&&Object.hasOwn(value,key);

function resolvePointer(raw,pointer){
  if(typeof pointer!=='string'||pointer!==''&&!pointer.startsWith('/')||/~(?:[^01]|$)/.test(pointer))return {state:'unavailable'};
  let value=raw;
  for(const part of pointer===''?[]:pointer.slice(1).split('/')){
    const key=part.replace(/~1/g,'/').replace(/~0/g,'~');
    if(!own(value,key)||Array.isArray(value)&&!/^(0|[1-9]\d*)$/.test(key))return {state:'unavailable'};
    value=value[key];
  }
  return value===undefined?{state:'unavailable'}:{state:'available',value:structuredClone(value)};
}

// Only this response's declared pointers define its essential record context.
// Values, unknown fields and explicit nulls remain source-delivered data.
export function essentialContext(raw){
  const selection=raw?.display_selection;
  if(!selection||!Object.hasOwn(selection,'essential_context_pointers'))return {state:'not-declared',items:[]};
  if(selection.schema_version!=='tos_display_selection_v1'||selection.content_revision!==raw.content_revision
    ||!Array.isArray(selection.essential_context_pointers))return {state:'unavailable',items:[]};
  const items=selection.essential_context_pointers.map(pointer=>({pointer,...resolvePointer(raw,pointer)}));
  return {state:items.some(item=>item.state!=='available')?'incomplete':'available',items};
}
