// Saved-draft policy uses bounded metadata; value custody stays in JS.
// With maxNodes=40, ordinary finite stored/link JSON arrays have at most 247
// spread slots and 254272 UTF-16 units across root strings. Valid root metadata
// is at most 8247 bytes (v1), 8246 bytes for v2 with defined conditions; the
// policy reply is at most 43 bytes. These are carrier bounds, not RSS claims.
// Native Set/spread retain custom iterators: repeated or unbounded yields can
// consume unbounded work/storage before cardinality admission or during spread.
let normalize;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const text=value=>typeof value==='string'?{kind:'string',units:value.length}:{kind:value===null?'null':'invalid'};
const list=(value,max)=>{
  if(!Array.isArray(value))return null;
  return {length:value.length,unique_count:value.length<=max?new Set(value).size:null,
    items:value.length<=max?Array.from({length:value.length},(_,index)=>{
    // `Set` counts a hole as undefined while `every` skips it. Preserve that
    // existing saved-draft rule, including rejection of two equal holes.
    if(!(index in value))return {kind:'hole'};
    return text(value[index]);
  }):[]};
};
const number=value=>typeof value==='number'?value:null;
const token=(value,max)=>typeof value==='string'?value.slice(0,max+1):null;
const request=(value,maxNodes)=>({present:Boolean(value),max_nodes:maxNodes,v:number(value?.v),
  name:{...text(value?.name),nonempty:typeof value?.name==='string'&&value.name.length<=64&&Boolean(value.name.trim())},
  scope:token(value?.scope,5),sources:list(value?.sources,7),node_ids:list(value?.nodeIds,maxNodes),
  kinds:list(value?.kinds,100),predicates:list(value?.predicates,100),query:text(value?.query),focus_id:text(value?.focusId),
  depth:number(value?.depth),direction:token(value?.direction,8),profile:token(value?.profile,8),
  limit:number(value?.limit),relations:typeof value?.relations==='boolean'?value.relations:null,
  conditions_defined:value?.conditions!==undefined,paths_defined:value?.paths!==undefined,
  paths_array:Array.isArray(value?.paths),paths_length:Array.isArray(value?.paths)?value.paths.length:null,
  retain_paths:value!=null&&Object.hasOwn(value,'paths')});

export function installDraftRules(runtime){
  if(typeof runtime?.normalize_observatory_draft_wasm_v1!=='function')throw new TypeError('Observatory draft WASM rule is unavailable');
  normalize=runtime.normalize_observatory_draft_wasm_v1;
}
export function normalizeDraft(value,maxNodes){
  if(!normalize)throw new TypeError('Observatory draft WASM rule is unavailable');
  return JSON.parse(decoder.decode(normalize(encoder.encode(JSON.stringify(request(value,maxNodes))))));
}
