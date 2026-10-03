// Bounded transport for the durable reading selector rule. Browser JSON owns
// canonical key spelling and LocalStorage; Rust owns its retained shape.
let normalize;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const claimFields=['claimId','pathId','relationType','nodeIds','relationIds','detailRelationIds','closureNodeIds'];
const claim=value=>Object.fromEntries(claimFields.map(field=>[field,value?.[field]]));
const detail=value=>Array.isArray(value)?[value[0],value[1]]:value;
const position=value=>Array.isArray(value)?[value[0],{
  top:value[1]?.top,details:Array.isArray(value[1]?.details)?value[1].details.map(detail):value[1]?.details,
  anchor:value[1]?.anchor===null?null:{key:value[1]?.anchor?.key,offset:value[1]?.anchor?.offset},
}]:value;
function entry(value){
  if(Array.isArray(value?.positions)&&value.positions.length>8)throw new Error('invalid_reading');
  return {kind:value?.kind,id:value?.id,_key:JSON.stringify([value?.kind,value?.id]),
    sourceRevision:value?.sourceRevision,contentRevision:value?.contentRevision,
    preferred:value?.preferred,positions:Array.isArray(value?.positions)?value.positions.map(position):value?.positions,
    ...(Object.hasOwn(value??{},'claimReference')?{claimReference:value.claimReference===undefined?null:claim(value.claimReference)}:{})};
}
export function installReadingRules(runtime){
  if(typeof runtime?.normalize_reading_resume_wasm_v1!=='function')throw new TypeError('Reading resume WASM rule is unavailable');
  normalize=runtime.normalize_reading_resume_wasm_v1;
}
export function normalizeReadingResume(value){
  if(!normalize)return null;
  if(Array.isArray(value?.entries)&&value.entries.length>2)throw new Error('invalid_reading');
  const projected={v:value?.v,activeKey:value?.activeKey,
    entries:Array.isArray(value?.entries)?value.entries.map(entry):value?.entries};
  const wire=JSON.stringify(projected);
  if(typeof wire!=='string'||wire.length>4_000_000)throw new Error('invalid_reading');
  const bytes=encoder.encode(wire);
  if(bytes.length>4_000_000)throw new Error('invalid_reading');
  const result=JSON.parse(decoder.decode(normalize(bytes)));
  // JSON.stringify collapses -0 to 0. The maintained in-memory reader kept
  // the numeric sign, though persisted JSON does not.
  for(let i=0;i<result.entries.length;i++)for(let j=0;j<result.entries[i].positions.length;j++){
    const original=value.entries[i].positions[j]?.[1],restored=result.entries[i].positions[j][1];
    if(Object.is(original?.top,-0))restored.top=-0;
    if(Object.is(original?.anchor?.offset,-0))restored.anchor.offset=-0;
  }
  if(JSON.stringify(result).length>200_000)throw new Error('invalid_reading');
  return result;
}
