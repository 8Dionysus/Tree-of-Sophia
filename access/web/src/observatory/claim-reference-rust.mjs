// Only the seven bounded pointer fields cross into the shared Rust rule.
let validate;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const fields=['claimId','pathId','relationType','nodeIds','relationIds','detailRelationIds','closureNodeIds'];

export function installClaimReferenceRules(runtime){
  if(typeof runtime?.validate_claim_reference_wasm_v1!=='function')
    throw new TypeError('Claim reference WASM rule is unavailable');
  validate=runtime.validate_claim_reference_wasm_v1;
}

export function normalizeClaimReference(reference,claimId){
  if(!validate)return null;
  const projected=Object.fromEntries(fields.map(field=>[field,reference?.[field]]));
  const wire=JSON.stringify({claim_id:claimId,reference:projected});
  if(typeof wire!=='string'||wire.length>2_000_000)throw new Error('invalid_claim_reference');
  const bytes=encoder.encode(wire);
  if(bytes.length>2_000_000)throw new Error('invalid_claim_reference');
  return JSON.parse(decoder.decode(validate(bytes)));
}
