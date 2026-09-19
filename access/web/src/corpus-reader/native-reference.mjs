// A native reference pins delivered source bytes. The graph origin is a way
// back to an owner-issued target, never permission to substitute newer text.
export const NATIVE_REFERENCE_SCHEMA='tos.corpus.reader.native-reference.v1';
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const fail=()=>{throw new TypeError('Invalid exact native text reference.');};
const string=value=>{if(typeof value!=='string'||!value.length||value.length>2048||!value.isWellFormed()||/[\u0000-\u001f\u007f]/u.test(value))fail();return value;};
const identifier=value=>{string(value);if(!/^tos\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/.test(value))fail();return value;};
const hash=value=>{if(typeof value!=='string'||!/^[a-f0-9]{64}$/.test(value))fail();return value;};
const integer=(value,min=0)=>{if(!Number.isSafeInteger(value)||value<min)fail();return value;};
const versioned=value=>{if(!object(value))fail();return {id:identifier(value.id),version:integer(value.version,1)};};
const selector=value=>{
  if(!object(value)||value.type!=='text_position'||value.positionUnit!=='unicode_code_point'||value.interval!=='half_open')fail();
  const start=integer(value.start),end=integer(value.end);if(end<start)fail();
  return {type:'text_position',start,end,positionUnit:'unicode_code_point',interval:'half_open'};
};

export function validateNativeReference(value){
  if(!object(value)||value.schemaVersion!==NATIVE_REFERENCE_SCHEMA||!object(value.target)||!object(value.origin))fail();
  const {target}=value;
  if(!['node','relation'].includes(value.origin.kind)||!['native_public_unit','native_local_unit'].includes(value.representation))fail();
  if(!object(target.record)||typeof target.record.digest!=='string'||!target.record.digest.startsWith('sha256:'))fail();
  const record={...versioned(target.record),digest:'sha256:'+hash(target.record.digest.slice(7))};
  const packet={...versioned(target.packet),sha256:hash(target.packet.sha256)};
  const segmentation=versioned(target.segmentation),unit=versioned(target.unit);
  const textLayer={...versioned(target.textLayer),recordSha256:hash(target.textLayer.recordSha256)};
  if(!object(target.span))fail();
  const span={anchorRef:identifier(target.span.anchorRef),start:integer(target.span.start),end:integer(target.span.end),exactSha256:hash(target.span.exactSha256)};
  if(span.end<span.start)fail();
  const selected=selector(value.selector);
  if(selected.start<span.start||selected.end>span.end)fail();
  return {schemaVersion:NATIVE_REFERENCE_SCHEMA,origin:{kind:value.origin.kind,id:string(value.origin.id)},representation:value.representation,
    target:{record,packet,segmentation,unit,textLayer,representationSha256:hash(target.representationSha256),span},selector:selected};
}

export function nativeReferenceKey(value){
  const checked=validateNativeReference(value);
  // Graph revision, navigation origin and transport/access mode do not own
  // the identity of a passage. The owner record and every text version do.
  return JSON.stringify([checked.schemaVersion,checked.target,checked.selector]);
}

export function nativeReferenceDocumentId(value){return validateNativeReference(value).target.textLayer.id;}
export function nativeReferenceVersionId(value){
  const {target}=validateNativeReference(value);
  return JSON.stringify([target.packet.id,target.packet.version,target.packet.sha256,target.segmentation.id,target.segmentation.version,target.textLayer.version]);
}

/** Input must be a successful result of the shared exact-source consumer. */
export function createNativeReference(result,spanIndex,{start,end}={}){
  if(result?.status!=='available'||!result.native_unit||!result.selection)fail();
  const unit=result.native_unit,span=unit.spans[spanIndex],summary=unit.summary;
  if(!span)fail();
  return validateNativeReference({schemaVersion:NATIVE_REFERENCE_SCHEMA,origin:{kind:result.selection.kind,id:result.selection.id},
    representation:unit.schema_version==='tos_native_local_unit_return_v1'?'native_local_unit':'native_public_unit',
    target:{record:result.record_ref,packet:unit.packet,
      segmentation:{id:summary.segmentation_id,version:summary.segmentation_version},unit:{id:summary.unit_id,version:summary.unit_version},
      textLayer:{id:summary.layer_id,version:summary.layer_version,recordSha256:unit.layer_record_sha256},
      representationSha256:unit.representation_sha256,
      span:{anchorRef:span.anchor_ref,start:span.selector.start,end:span.selector.end,exactSha256:span.exact_sha256}},
    selector:{type:'text_position',start:start??span.selector.start,end:end??span.selector.end,positionUnit:'unicode_code_point',interval:'half_open'}});
}

export function matchNativeReference(reference,result){
  const checked=validateNativeReference(reference);
  if(result?.status!=='available'||!result.native_unit)return {status:result?.status??'unavailable',spanIndex:null};
  const index=result.native_unit.spans.findIndex(span=>span.anchor_ref===checked.target.span.anchorRef);
  if(index<0)return {status:'stale',spanIndex:null};
  try{
    const current=createNativeReference(result,index,checked.selector);
    return nativeReferenceKey(current)===nativeReferenceKey(checked)?{status:'exact',spanIndex:index}:{status:'stale',spanIndex:null};
  }catch{return {status:'stale',spanIndex:null};}
}

export function nativeSelectedText(reference,text){
  const {target,selector}=validateNativeReference(reference),points=Array.from(text);
  if(points.length!==target.span.end-target.span.start)fail();
  return points.slice(selector.start-target.span.start,selector.end-target.span.start).join('');
}
