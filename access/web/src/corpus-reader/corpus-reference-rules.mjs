// The initialized shared Rust rule owns reference validation and identity.
// This module only frames bounded JSON requests and adapts WASM errors.
let rule;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const INPUT_BYTES=8*1024*1024;

export function installCorpusReferenceRules(runtime){
  if(typeof runtime?.corpus_reference_rule_wasm_v1!=='function')
    throw new TypeError('The corpus reference WASM rule is unavailable.');
  rule=runtime.corpus_reference_rule_wasm_v1;
}

function failure(error){
  const code=String(error).match(/reference_selector_range|invalid_native_reference|invalid_corpus_reference|invalid_input|invalid_reference_output/u)?.[0];
  if(code==='reference_selector_range')return new RangeError('reference.selector.end must not precede start');
  if(code==='invalid_native_reference')return new TypeError('Invalid exact native text reference.');
  if(code==='invalid_corpus_reference')return new TypeError('Invalid corpus text reference.');
  return new TypeError('Invalid corpus text reference.');
}

export function corpusReferenceRule(operation,value){
  if(!rule)throw new Error('The corpus reference Rust rule is not installed.');
  let json;
  try{json=JSON.stringify({operation,value});}catch(error){throw failure(error);}
  if(typeof json!=='string'||json.length>INPUT_BYTES)throw new TypeError('Invalid corpus text reference.');
  const bytes=encoder.encode(json);
  if(bytes.length>INPUT_BYTES)throw new TypeError('Invalid corpus text reference.');
  try{return JSON.parse(decoder.decode(rule(bytes)));}
  catch(error){throw failure(error);}
}
