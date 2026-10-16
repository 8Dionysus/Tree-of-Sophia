// Rust owns saved-filter admissibility; catalog, DOM and value custody stay here.
let normalize,normalizePaths,conditionCatalog,compileRules,pathLimits,compilePaths,importPaths;
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
const project=(value,keys)=>Object.fromEntries(keys.map(key=>[key,value?.[key]]));
const projectProperties=value=>Array.isArray(value)?value.map(item=>({property_id:item?.property_id,
  value_type:item?.value_type,applies_to:item?.applies_to,operators:item?.operators})):value;
export function conditionContextRequest(context){
  const schema=context?.schema,catalog=context?.catalog,caps=catalog?.capabilities,defs=schema?.$defs;
  const properties=catalog?.semantic_registries?.properties;
  return {schema:{properties:{path_query:schema?.properties?.path_query},$defs:project(defs,
    ['pathCondition','nodeQuery','relationQuery','nodeFilter','relationFilter','nodeField','relationField'])},
    catalog:{capabilities:project(caps,['path_query','filter_operators','node_fields','relation_fields',
      'operator_value_contracts','property_filters']),
      node_kinds:Array.isArray(catalog?.node_kinds)?catalog.node_kinds.map(item=>({kind_id:item?.kind_id})):catalog?.node_kinds,
      predicates:Array.isArray(catalog?.predicates)?catalog.predicates.map(item=>({predicate_id:item?.predicate_id})):catalog?.predicates,
      semantic_registries:{properties:projectProperties(properties)}}};
}
const ruleList=value=>Array.isArray(value)?entries(value,value.length):null;
const encodeRequest=value=>encoder.encode(JSON.stringify(value));
const decodeResult=value=>JSON.parse(decoder.decode(value));
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
  if(typeof runtime?.normalize_observatory_paths_wasm_v1!=='function')throw new TypeError('Observatory path WASM rule is unavailable');
  if(typeof runtime?.observatory_condition_catalog_wasm_v1!=='function'
    ||typeof runtime?.compile_observatory_conditions_wasm_v1!=='function'
    ||typeof runtime?.observatory_path_limits_wasm_v1!=='function'
    ||typeof runtime?.compile_observatory_paths_wasm_v1!=='function'
    ||typeof runtime?.draft_observatory_paths_from_spec_wasm_v1!=='function')throw new TypeError('Observatory path compiler WASM rule is unavailable');
  normalize=runtime.normalize_observatory_conditions_wasm_v1;
  normalizePaths=runtime.normalize_observatory_paths_wasm_v1;
  conditionCatalog=runtime.observatory_condition_catalog_wasm_v1;
  compileRules=runtime.compile_observatory_conditions_wasm_v1;
  pathLimits=runtime.observatory_path_limits_wasm_v1;
  compilePaths=runtime.compile_observatory_paths_wasm_v1;
  importPaths=runtime.draft_observatory_paths_from_spec_wasm_v1;
}
export const conditionRequest=request;
export function normalizePathRule(value){
  if(!normalizePaths)throw new TypeError('Observatory path WASM rule is unavailable');
  return decodeResult(normalizePaths(encodeRequest(value)));
}
export function conditionCatalogRules(context,kind){
  if(!conditionCatalog)throw new TypeError('Observatory condition catalog WASM rule is unavailable');
  return decodeResult(conditionCatalog(encodeRequest({context:conditionContextRequest(context),kind})));
}
export function compileConditionRules(rules,context,kind,{reserveTypeSlot=true}={}){
  if(!compileRules)throw new TypeError('Observatory condition compiler WASM rule is unavailable');
  return decodeResult(compileRules(encodeRequest({context:conditionContextRequest(context),kind,
    reserve_type_slot:reserveTypeSlot,rules:ruleList(rules)})));
}
export function observatoryPathLimits(context){
  if(!pathLimits)throw new TypeError('Observatory path limits WASM rule is unavailable');
  return decodeResult(pathLimits(encodeRequest({context:conditionContextRequest(context)})));
}
export function compilePathRules(paths,context){
  if(!compilePaths)throw new TypeError('Observatory path compiler WASM rule is unavailable');
  return decodeResult(compilePaths(encodeRequest({paths,context:conditionContextRequest(context)})));
}
export function importPathDraftRules(spec){
  if(!importPaths)throw new TypeError('Observatory path import WASM rule is unavailable');
  return decodeResult(importPaths(encodeRequest(spec)));
}
export function attachConditionValues(value,normalized){
  const result={...normalized};
  // Copy each admitted value once, directly from its original JS carrier.
  // This preserves sparse lists, negative zero and exact UTF-16 code units.
  for(const kind of ['nodes','relations'])result[kind]=value[kind].map((original,index)=>({
    ...normalized[kind][index],value:structuredClone(original.value)}));
  return result;
}
export function normalizeConditions(value,maxConditions){
  if(!normalize)throw new TypeError('Observatory conditions WASM rule is unavailable');
  const encoded=encoder.encode(JSON.stringify(request(value,maxConditions)));
  const result=JSON.parse(decoder.decode(normalize(encoded)));
  return attachConditionValues(value,result);
}
