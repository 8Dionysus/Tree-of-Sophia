// Translate explicitly selected native candidates into the source-owner wire
// grammar. This module issues no grants, guesses no Tree identities and never
// turns a stand assessment into an admission.

const SCHEMA='tos_local_source_command_v1';
const clone=value=>structuredClone(value);

/** A single ordered source-owner preparation. Bindings and forms must already
 * be selected by the caller against the destination's source and grant. */
export function prepareSourceRequest(candidate){
  if(candidate?.kind==='record'){
    if(!candidate.record?.record_id||!Array.isArray(candidate.forms)||!candidate.forms.length)throw new TypeError('Record identity and source-copy forms are required.');
    return {schema_version:SCHEMA,operation:'prepare-create',record:clone(candidate.record),forms:clone(candidate.forms)};
  }
  if(candidate?.kind==='claims'){
    if(!Array.isArray(candidate.claims)||!candidate.claims.length||candidate.claims.length>32)throw new TypeError('A batch of 1–32 Claims is required.');
    if(candidate.claims.some(item=>item.review_status!=='unreviewed'))throw new TypeError('Initial Claims must be unreviewed.');
    return {schema_version:SCHEMA,operation:'prepare-create',claims:clone(candidate.claims)};
  }
  throw new TypeError('Select record or claims.');
}

/** Complete the exact write request from the native owner's own preview. Save
 * this complete request for exact retry; never manufacture preview digests. */
export function commitSourceRequest(candidate,preview,commandId){
  if(typeof commandId!=='string'||!commandId)throw new TypeError('A stable command ID is required.');
  const result=preview?.result??preview;
  if(!result?.owner_configuration||!result?.expected_dependencies)throw new TypeError('The native preview lacks required owner bindings.');
  const request=prepareSourceRequest(candidate);
  Object.assign(request,{operation:candidate.kind==='record'?'source.create':'claims.create',command_id:commandId,
    expected_configuration:result.owner_configuration,expected_dependencies:result.expected_dependencies,expected_revision:null});
  if(candidate.kind==='record')request.expected_source=null;
  else {
    if(!result.source_bindings)throw new TypeError('The native Claim preview lacks source bindings.');
    request.expected_inputs=clone(result.source_bindings);
  }
  return request;
}
