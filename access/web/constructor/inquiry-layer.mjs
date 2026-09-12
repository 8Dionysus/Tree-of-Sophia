import {INQUIRY_SOURCES_CONCEPTS} from './inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from './inquiry-readings-echoes.mjs';
import {RELATION_INQUIRY} from './relation-inquiry.mjs';
import {sourceReference} from './source-references.mjs';

const bilingual=(value,label)=>{
 for(const code of ['ru','en'])if(typeof value?.[code]!=='string'||!value[code].trim())throw Error('Missing '+code+' inquiry: '+label);
};
const coverage=(actual,expected,label)=>{
 const ids=new Set(expected);
 if(ids.size!==expected.length||actual.length!==ids.size||actual.some(id=>!ids.has(id)))throw Error('Inquiry coverage differs from prepared '+label);
};

/** Checks traceability mechanics, never whether a passage warrants a reading. */
export function checkGrounds(grounds,label){
 if(!Array.isArray(grounds)||!grounds.length)throw Error('Missing textual grounds: '+label);
 const seen=new Set();
 for(const ground of grounds){
  if(typeof ground.ref!=='string'||seen.has(ground.ref))throw Error('Invalid or duplicate textual ground: '+label);
  sourceReference(ground.ref);bilingual(ground.focus,label+' source focus');seen.add(ground.ref);
 }
}

function readerAnchors(item){
 const anchors=new Map();
 for(const ground of [...item.grounds,...(item.counterReading?.grounds??[])]){
  const passageId=sourceReference(ground.ref).passageId;
  if(passageId&&!anchors.has(passageId))anchors.set(passageId,{passageId,focus:ground.focus});
 }
 return [...anchors.values()];
}

/** Bind authored demonstration commentary without replacing source fields. */
export function bindInquiryLayer(materialIds,edgeIds,{sources=INQUIRY_SOURCES_CONCEPTS,readings=INQUIRY_READINGS_ECHOES,relations=RELATION_INQUIRY}={}){
 const nodeEntries=[...Object.entries(sources),...Object.entries(readings)];
 coverage(nodeEntries.map(([id])=>id),materialIds,'materials');
 if(new Set(nodeEntries.map(([id])=>id)).size!==nodeEntries.length)throw Error('Duplicate authored material inquiry');
 coverage(Object.keys(relations),edgeIds,'relations');
 const nodes=new Map(nodeEntries);
 for(const [id,item]of nodes){
  bilingual(item.argument,id+' argument');checkGrounds(item.grounds,id);
  if('counterpoint'in item)throw Error('Unattributed counterpoint is not a reading: '+id);
  if(item.question)bilingual(item.question,id+' question');
  if(item.experiment)for(const key of ['setup','question'])bilingual(item.experiment[key],id+' experiment '+key);
  if(item.counterReading){bilingual(item.counterReading.text,id+' alternative reading');checkGrounds(item.counterReading.grounds,id+' alternative reading');}
  nodes.set(id,{...item,anchors:readerAnchors(item)});
 }
 for(const [id,item]of Object.entries(relations)){
  bilingual(item.warrant,id+' warrant');checkGrounds(item.grounds,id);
  if('challenge'in item)throw Error('Unattributed relation challenge: '+id);
  for(const key of ['limit','question'])if(item[key])bilingual(item[key],id+' '+key);
 }
 return {material:id=>nodes.get(id),relation:id=>relations[id]};
}

/** Called only after the separate text catalog has passed its source checks. */
export function checkInquiryTextReferences(nodes,catalog){
 const documents=new Map(catalog.passages.map(item=>[item.id,item]));
 for(const node of nodes)for(const anchor of node.inquiry?.anchors??[])if(documents.get(anchor.passageId)?.status!=='available')throw Error('Inquiry text reference is unavailable: '+node.id+' → '+anchor.passageId);
}

/** A route may read a passage that the stop's own introduction does not use. */
export function inquiryReadingContexts(nodes,routes){
 const result=new Map(nodes.map(node=>[node.id,new Map((node.inquiry?.anchors??[]).map(anchor=>[anchor.passageId,anchor.focus]))]));
 const add=(id,grounds)=>{const contexts=result.get(id);if(!contexts)throw Error('Unknown reading context: '+id);for(const ground of grounds??[]){const passageId=sourceReference(ground.ref).passageId;if(passageId&&!contexts.has(passageId))contexts.set(passageId,ground.focus);}};
 for(const route of routes){for(const step of route.steps)add(step.nodeId,step.grounds);add(route.steps.at(-1).nodeId,route.grounds);}
 return result;
}

export function routeGroundContext(route,ref){
 return route.steps.find(step=>step.grounds.some(ground=>ground.ref===ref))?.nodeId??route.steps.at(-1).nodeId;
}
