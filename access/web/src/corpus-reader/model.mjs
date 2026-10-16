import {NATIVE_REFERENCE_SCHEMA} from './native-reference.mjs';
import {corpusReferenceRule} from './corpus-reference-rules.mjs';

/** Validate and normalize a durable text reference. */
export function validateReference(value){
  return corpusReferenceRule('validate_reference',value);
}

/** Construct a reference from provider source metadata and a unit selector. */
export function createReference({source,versionId,unitId,start=0,end=0}){
  return corpusReferenceRule('create_reference',{source,versionId,unitId,start,end});
}

/** Stable key for local stores, URL state, and provider-side resolve calls. */
export function referenceKey(value){
  return corpusReferenceRule('reference_key',value);
}

export function isReference(value){
  try{
    validateReference(value);
    return true;
  }catch{
    return false;
  }
}

export function referenceDocumentId(value){
  return corpusReferenceRule('reference_document_id',value);
}

export function referenceVersionId(value){
  return corpusReferenceRule('reference_version_id',value);
}

export {NATIVE_REFERENCE_SCHEMA};
