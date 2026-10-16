import {INQUIRY_SOURCES_CONCEPTS} from './inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from './inquiry-readings-echoes.mjs';
import {RELATION_INQUIRY} from './relation-inquiry.mjs';
import {readingRule} from './reading-rules-rust.mjs';
export function checkGrounds(grounds,label){readingRule('grounds',{grounds,label});}
/** Authored commentary stays unchanged; Rust binds its traceability mechanics. */
export function bindInquiryLayer(materialIds,edgeIds,{sources=INQUIRY_SOURCES_CONCEPTS,readings=INQUIRY_READINGS_ECHOES,relations=RELATION_INQUIRY}={}){
 const bound=readingRule('bind',{materialIds,edgeIds,sources,readings,relations});
 return {material:id=>bound.nodes[id],relation:id=>bound.relations[id]};
}
export function checkInquiryTextReferences(nodes,catalog){readingRule('textReferences',{nodes,catalog});}
export function inquiryReadingContexts(nodes,routes){return new Map(Object.entries(readingRule('contexts',{nodes,routes})).map(([id,contexts])=>[id,new Map(Object.entries(contexts))]));}
export function routeGroundContext(route,ref){return readingRule('groundContext',{route,ref});}
