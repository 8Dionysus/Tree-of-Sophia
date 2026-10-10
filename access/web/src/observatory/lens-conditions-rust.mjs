// Rust owns saved-filter admissibility; catalog, DOM and value custody stay here.
let normalize;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const scalar=value=>{
  if(value===null)return {kind:'null'};
  switch(typeof value){
    case 'boolean':return {kind:'boolean'};
    case 'string':return {kind:'string',units:value.length};
    case 'number':return {kind:'number',finite:Number.isFinite(value)};
    default:return {kind:'invalid'};
  }
};
const valueShape=value=>Array.isArray(value)?{kind:'array',length:value.length,
  items:value.length<=100?Array.from({length:value.length},(_,index)=>index in value?scalar(value[index]):{kind:'hole'}):[]}:scalar(value);
const short=(value,max)=>typeof value==='string'?value.slice(0,max+1):null;
const rule=value=>value?{selector:short(value.selector,32),id:short(value.id,256),
  op:short(value.op,32),value_shape:valueShape(value.value)}:null;
const entries=(value,max)=>Array.isArray(value)?{length:value.length,
  items:Number.isInteger(max)&&max>=0&&value.length<=max
    ?Array.from({length:value.length},(_,index)=>index in value?rule(value[index]):{hole:true}):[]}:null;
const request=(value,maxConditions)=>{
  const shape=value!==null&&typeof value==='object'&&!Array.isArray(value)?'object':'invalid',keys=[];
  if(shape==='object')for(const key in value)if(Object.hasOwn(value,key)){
    keys.push(short(key,32));if(keys.length===3)break;
  }
  return {shape,keys,max_conditions:typeof maxConditions==='number'?maxConditions:null,
    nodes:entries(value?.nodes,maxConditions),relations:entries(value?.relations,maxConditions)};
};

export function installConditionRules(runtime){
  if(typeof runtime?.normalize_observatory_conditions_wasm_v1!=='function')throw new TypeError('Observatory conditions WASM rule is unavailable');
  normalize=runtime.normalize_observatory_conditions_wasm_v1;
}
export function normalizeConditions(value,maxConditions){
  if(!normalize)throw new TypeError('Observatory conditions WASM rule is unavailable');
  const encoded=encoder.encode(JSON.stringify(request(value,maxConditions)));
  const result=JSON.parse(decoder.decode(normalize(encoded)));
  // Copy each admitted value once, directly from its original JS carrier.
  // This preserves sparse lists, negative zero and exact UTF-16 code units;
  // no value payload is serialized, parsed, cloned or echoed through WASM.
  for(const kind of ['nodes','relations'])result[kind]=value[kind].map((original,index)=>({
    ...result[kind][index],value:structuredClone(original.value)}));
  return result;
}
