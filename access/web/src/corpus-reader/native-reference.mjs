import {corpusReferenceRule} from './corpus-reference-rules.mjs';

// A native reference pins delivered source bytes. The graph origin is a way
// back to an owner-issued target, never permission to substitute newer text.
export const NATIVE_REFERENCE_SCHEMA='tos.corpus.reader.native-reference.v1';

export function validateNativeReference(value){
  return corpusReferenceRule('validate_native_reference',value);
}

export function nativeReferenceKey(value){
  return corpusReferenceRule('native_reference_key',value);
}

/** Input must be a successful result of the shared exact-source consumer. */
export function createNativeReference(result,spanIndex,{start,end}={}){
  return corpusReferenceRule('create_native_reference',{result,spanIndex,start,end});
}

export function matchNativeReference(reference,result){
  return corpusReferenceRule('match_native_reference',{reference,result});
}

export function nativeSelectedText(reference,text){
  return corpusReferenceRule('native_selected_text',{reference,text});
}
