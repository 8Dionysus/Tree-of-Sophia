/** Browser transport for local research shelf rules. The entry installs the
 * generated runtime after WASM initialization; no storage or DOM enters it. */
let runtime;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});

export function installResearchShelfRules(value){
  if(!value||typeof value.research_shelf_rule_wasm_v1!=='function'||typeof value.ResearchShelfPacketIndex!=='function')
    throw new TypeError('The research shelf WASM rule is unavailable.');
  runtime=value;
}

export function researchShelfRulesReady(){return Boolean(runtime);}

function ruleError(error){
  const code=String(error).match(/invalid-target|invalid-record|invalid-input|invalid-cursor|invalid-packet|migration-required|conflict|not-found|limit/u)?.[0]??'invalid-input';
  const failure=new Error(code);failure.name='ResearchShelfError';failure.code=code;failure.cause=error;
  return failure;
}

// Match the existing Rust parser's bounds before stringify/UTF-8 copies. Raw
// string/key units are a lower bound on their JSON byte representation. Visits
// include queued values so a huge invalid container is refused before enqueue.
const INPUT_BYTES=1_048_576,INPUT_VISITS=300_000,INPUT_DEPTH=64;
function encodeRuleRequest(value){
  const pending=[[value,0]];let visits=0,units=0;
  const text=value=>{units+=value.length;if(units>INPUT_BYTES)throw new Error('limit');};
  while(pending.length){
    const [item,depth]=pending.pop();
    if(++visits>INPUT_VISITS||depth>INPUT_DEPTH)throw new Error('limit');
    if(typeof item==='string'){text(item);continue;}
    if(!item||typeof item!=='object')continue;
    if(Array.isArray(item)){
      if(item.length>INPUT_VISITS-visits-pending.length)throw new Error('limit');
      for(const child of item)pending.push([child,depth+1]);
    }else{
      for(const key in item)if(Object.hasOwn(item,key)){
        text(key);
        if(visits+pending.length>=INPUT_VISITS)throw new Error('limit');
        pending.push([item[key],depth+1]);
      }
    }
  }
  const bytes=encoder.encode(JSON.stringify(value));
  if(bytes.byteLength>INPUT_BYTES)throw new Error('limit');
  return bytes;
}

export function researchShelfRule(operation,value){
  if(!runtime)return null;
  try{
    return JSON.parse(decoder.decode(runtime.research_shelf_rule_wasm_v1(encodeRuleRequest({operation,value}))));
  }catch(error){throw ruleError(error);}
}

/** Incremental exact-ID validation without carrying record bodies in WASM. */
export function validateResearchShelfPacketIndex(packet){
  if(!runtime)return null;
  let index;
  try{
    index=new runtime.ResearchShelfPacketIndex(encoder.encode(JSON.stringify({
      schema:packet.schema,version:packet.version,generation:packet.generation,
      recordCount:packet.records.length,collectionCount:packet.collections.length,
      ...(packet.exportedAt===undefined?{}:{exportedAt:packet.exportedAt}),
    })));
    for(const [values,method] of [[packet.records,'accept_records'],[packet.collections,'accept_collections']]){
      // Sort references to existing IDs. Rust checks strict UTF-16 order and
      // adjacency across chunks, retaining only the final ID of each stream.
      const ids=values.map(value=>value.id).sort();
      for(let start=0;start<ids.length;start+=256)
        index[method](encoder.encode(JSON.stringify(ids.slice(start,start+256))));
    }
    index.finish();
    return true;
  }catch(error){throw ruleError(error);}
  finally{index?.free?.();}
}
