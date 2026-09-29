// Only the bounded saved-filter shape crosses WASM; catalog and DOM stay here.
let normalize;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const scalar=value=>{
  if(value===null||typeof value==='boolean')return value;
  if(typeof value==='string')return value.length>1024?value.slice(0,1025):value;
  if(typeof value==='number'&&Number.isFinite(value))return value;
  return {};
};
const bounded=value=>Array.isArray(value)?value.slice(0,101).map(scalar):scalar(value);
const short=(value,max)=>typeof value==='string'&&value.length>max?value.slice(0,max+1):value;
const rule=value=>value&&typeof value==='object'?{selector:short(value.selector,32),id:short(value.id,256),
  op:short(value.op,32),value:bounded(value.value)}:value;
const entries=(value,max)=>Array.isArray(value)?value.slice(0,max).map(rule):value;
const request=(value,maxConditions)=>{
  const shape=value!==null&&typeof value==='object'&&!Array.isArray(value)?'object':'invalid';
  const cap=Number.isInteger(maxConditions)&&maxConditions>=0?maxConditions+1:1;
  return {shape,keys:shape==='object'?Object.keys(value).slice(0,3):[],max_conditions:maxConditions,
    nodes:entries(value?.nodes,cap),relations:entries(value?.relations,cap)};
};
const restore=(source,result)=>{
  for(const kind of ['nodes','relations'])result[kind].forEach((rule,index)=>{
    const original=source[kind][index]?.value;
    if(Object.is(original,-0))rule.value=-0;
    if(Array.isArray(original)&&Array.isArray(rule.value))original.forEach((item,part)=>{
      if(Object.is(item,-0))rule.value[part]=-0;
    });
    if(Array.isArray(original)&&Array.isArray(rule.value))for(let part=0;part<original.length;part++)
      if(!(part in original))delete rule.value[part];
  });
  return result;
};

export function installConditionRules(runtime){
  if(typeof runtime?.normalize_observatory_conditions_wasm_v1!=='function')throw new TypeError('Observatory conditions WASM rule is unavailable');
  normalize=runtime.normalize_observatory_conditions_wasm_v1;
}
export function normalizeConditions(value,maxConditions){
  if(!normalize)return null;
  const encoded=encoder.encode(JSON.stringify(request(value,maxConditions)));
  return restore(value,JSON.parse(decoder.decode(normalize(encoded))));
}
