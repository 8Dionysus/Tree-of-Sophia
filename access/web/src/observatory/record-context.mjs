const own=(value,key)=>value!==null&&typeof value==='object'&&Object.hasOwn(value,key);
let installedRuntime;
export function installRecordContextRules(runtime){
  if(typeof runtime?.RecordContextSession!=='function')throw new TypeError('Generated record-context Rust rule export is incomplete');
  installedRuntime=runtime;
}
const units=value=>Uint16Array.from({length:value.length},(_,index)=>value.charCodeAt(index));
function text(units){let value='';for(const unit of units)value+=String.fromCharCode(unit);return value;}

// Rust owns declaration, pointer grammar, array keys and result aggregation.
// Host observations preserve JS identity; values never pass through JSON/WASM.
export function essentialContext(raw){
  if(!installedRuntime)throw new Error('Record-context Rust rules are not installed');
  const selection=raw?.display_selection;
  const session=new installedRuntime.RecordContextSession(
    Boolean(selection)&&Object.hasOwn(selection,'essential_context_pointers'),
    selection?.schema_version==='tos_display_selection_v1',
    selection?.content_revision===raw?.content_revision,
    Array.isArray(selection?.essential_context_pointers),
  );
  try{
    if(session.state()!=='available')return {state:session.state(),items:[]};
    const items=selection.essential_context_pointers.map(pointer=>{
      let value=raw,available=session.pointer(typeof pointer==='string',typeof pointer==='string'?units(pointer):new Uint16Array());
      for(let index=0;available&&index<session.token_count();index++){
        const key=text(session.token(index));
        if(!own(value,key)||Array.isArray(value)&&!session.array_index(index))available=false;
        else value=value[key];
      }
      if(!available||value===undefined){session.unavailable();return {pointer,state:'unavailable'};}
      return {pointer,state:'available',value:structuredClone(value)};
    });
    return {state:session.state(),items};
  }finally{session.free();}
}
