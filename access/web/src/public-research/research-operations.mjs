// Research operations run on the caller's explicitly selected, versioned data.
// The native query owner supplies every rule; the browser only frames requests.
let kernel;
const encoder=new TextEncoder(),decoder=new TextDecoder();
export function configureResearchOperations(runtime){
  if(typeof runtime?.research_operation_wasm_v1!=='function')throw new TypeError('Research operation WASM rule is unavailable');
  kernel=runtime.research_operation_wasm_v1;
}
export function runResearchOperation(request){
  if(!kernel)throw new Error('Research operation WASM rule is not configured');
  return JSON.parse(decoder.decode(kernel(encoder.encode(JSON.stringify(request)))));
}
