import {validateReference} from '../corpus-reader/model.mjs';
import {readingKey} from '../observatory/reader-model.mjs';
import {validateReading} from '../observatory/reading-resume.mjs';
import {exactFormRef} from '../observatory/human-forms.mjs';
import {validateDraft} from '../observatory/lens-model.mjs';
import {validateWorkspaceCopy} from '../observatory/workspace-copy.mjs';
import {validateWorkspaceCopyLenses} from '../observatory/workspace-copy-rust.mjs';
import {RESEARCH_WORKSPACE_SCHEMA,RESEARCH_WORKSPACE_VERSION} from '../research-workspace.ts';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {researchShelfRule,validateResearchShelfPacketIndex} from './rules.mjs';

// The shelf is a local, user-owned index of exact addresses. It never owns a
// source passage, an exploration page, or a command/runtime handle.
export const SHELF_VERSION=1;
export const SHELF_EXPORT_SCHEMA='tos.research_shelf.export.v1';
export const DEFAULT_DB_NAME='tos-research-shelf-v1';
/**
 * Creation/update time for imported material and lens records whose owner
 * packet has no shelf-record chronology. This is unknown local chronology,
 * not the time the source material or export wrapper was produced.
 */
export const MIGRATION_RECORD_TIMESTAMP='1970-01-01T00:00:00.000Z';
/** Explicit owner packet names accepted by the supplied-packet importer. */
export const SUPPLIED_PACKET_SOURCES=Object.freeze({
  READING_RESUME:'reading-resume',
  RESEARCH_WORKSPACE:'research-workspace',
  WORKSPACE_COPY:'workspace-copy',
});

export class ResearchShelfError extends Error {
  constructor(code,message,details={}){
    super(message||code);
    this.name='ResearchShelfError';
    this.code=code;
    Object.assign(this,details);
  }
}

const fail=(code,message,details)=>{throw new ResearchShelfError(code,message,details);};
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const own=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const clone=value=>{
  try{return typeof structuredClone==='function'?structuredClone(value):JSON.parse(JSON.stringify(value));}
  catch(error){fail('invalid-input','The shelf value is not cloneable.',{cause:error});}
};
const exactKeys=(value,required,optional=[])=>{
  if(!object(value))fail('invalid-target','The shelf value must be an object.');
  const allowed=new Set([...required,...optional]);
  if(Object.keys(value).some(key=>!allowed.has(key)))fail('invalid-target','The shelf value contains an unknown field.',{fields:Object.keys(value)});
  for(const key of required)if(!own(value,key))fail('invalid-target',`The shelf value is missing ${key}.`);
};
const timestamp=(value,label)=>{
  try{return researchShelfRule('timestamp',{value});}
  catch(error){fail('invalid-record',`The ${label} is invalid.`,{cause:error});}
};
const canonicalTimestamp=()=>new Date().toISOString();

function checkedTarget(type,value){
  return researchShelfRule('validate_target',{type,target:value});
}

/** Validate one graph material address through Rust, then the reading owner. */
export function validateMaterialTarget(value){
  let result=checkedTarget('material',value);
  if(own(result,'claimReference')){
    let reading;
    try{
      reading=validateReading({v:1,activeKey:readingKey(result.kind,result.id),entries:[{
        kind:result.kind,id:result.id,sourceRevision:result.sourceRevision,contentRevision:result.contentRevision,
        preferred:'default',positions:[],claimReference:result.claimReference,
      }]});
    }catch(error){fail('invalid-target','The material Claim binding is not a valid reading reference.',{cause:error});}
    result=checkedTarget('material',{...result,claimReference:reading.entries[0].claimReference});
  }
  return result;
}

export function validateFormTarget(value){
  const checked=checkedTarget('form',value);
  const material=validateMaterialTarget(checked.material);
  if(!exactFormRef(checked.form))fail('invalid-target','The form reference is not exact.');
  return checkedTarget('form',{...checked,material});
}

export function validateTextTarget(value){
  const checked=checkedTarget('text',value);
  let reference;
  try{reference=validateReference(checked.reference);}catch(error){fail('invalid-target','The exact text reference is invalid.',{cause:error});}
  return checkedTarget('text',{reference});
}

export function validateLensTarget(value){
  const checked=checkedTarget('lens',value);
  let draft;
  try{draft=validateDraft(checked.draft);}catch(error){fail('invalid-target','The lens draft is invalid.',{cause:error});}
  return checkedTarget('lens',{draft});
}

export function validateRouteTarget(value){
  const checked=checkedTarget('route',value);
  const origin=validateMaterialTarget(checked.origin);
  return checkedTarget('route',{...checked,origin});
}

export function validateTarget(type,value){
  switch(type){
    case 'material':return validateMaterialTarget(value);
    case 'form':return validateFormTarget(value);
    case 'text':return validateTextTarget(value);
    case 'lens':return validateLensTarget(value);
    case 'route':return validateRouteTarget(value);
    default:return checkedTarget(type,value);
  }
}

/** Validate a complete stored record; Rust owns its fields, bounds and normalization. */
export function validateShelfRecord(value){
  const target=validateTarget(value?.type,value?.target);
  return researchShelfRule('validate_record',{...value,target});
}

/** Build a new record; only clock and ID generation stay in the host. */
export function createShelfRecord(input,{id,now=canonicalTimestamp}={}){
  const target=input?.target===undefined?undefined:validateTarget(input?.type,input.target);
  const recordId=input?.id===undefined?(typeof id==='function'?id():`shelf-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`):input.id;
  const prepared=target===undefined?input:{...input,target};
  return researchShelfRule('create_record',{input:prepared,id:recordId,now:now()});
}

/** Normalize editable fields while Rust preserves the stored address and chronology. */
export function updateShelfRecord(existing,input,{now=canonicalTimestamp}={}){
  const old=validateShelfRecord(existing);
  const target=input?.target===undefined?undefined:validateTarget(old.type,input.target);
  const prepared=target===undefined?input:{...input,target};
  return researchShelfRule('update_record',{old,input:prepared,now:now()});
}

export function validateCollection(value){
  return researchShelfRule('validate_collection',value);
}

export function createShelfCollection(input,{id,now=canonicalTimestamp}={}){
  const collectionId=input?.id??(typeof id==='function'?id():`collection-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`);
  return researchShelfRule('create_collection',{input,id:collectionId,now:now()});
}

export function updateShelfCollection(existing,input,{now=canonicalTimestamp}={}){
  const old=validateCollection(existing);
  return researchShelfRule('update_collection',{old,input,now:now()});
}
export const sameShelfValue=(left,right)=>researchShelfRule('same_value',{left,right});

function encodeBase64(value){
  const text=JSON.stringify(value);
  if(typeof btoa==='function')return btoa(unescape(encodeURIComponent(text))).replaceAll('+','-').replaceAll('/','_').replace(/=+$/,'');
  return Buffer.from(text,'utf8').toString('base64url');
}
function decodeBase64(value){
  try{
    if(typeof atob==='function'){
      const base64=value.replaceAll('-','+').replaceAll('_','/').padEnd(value.length+((4-value.length%4)%4),'=');
      return decodeURIComponent(escape(atob(base64)));
    }
    return Buffer.from(value,'base64url').toString('utf8');
  }catch(error){fail('invalid-cursor','The shelf cursor is invalid.',{cause:error});}
}

export function validateListOptions(options={}){
  return researchShelfRule('list_options',options);
}

export function encodeShelfCursor({generation,type=null,collectionId=null,updatedAt,id}={}){
  const decoded=researchShelfRule('cursor',{decoded:{v:1,generation,filter:{type,collectionId},updatedAt,id},generation,type,collectionId});
  return encodeBase64(decoded);
}

export function decodeShelfCursor(cursor,{generation,type=null,collectionId=null}={}){
  if(cursor===null||cursor===undefined)return null;
  if(typeof cursor!=='string'||cursor.length>16_384)fail('invalid-cursor','The shelf cursor is invalid.');
  let value;
  try{value=JSON.parse(decodeBase64(cursor));}catch(error){fail('invalid-cursor','The shelf cursor is invalid.',{cause:error});}
  const checked=researchShelfRule('cursor',{decoded:value,generation,type,collectionId});
  return {generation:checked.generation,filter:checked.filter,updatedAt:checked.updatedAt,id:checked.id};
}

// Short aliases keep the model convenient for adapters and callers that use
// the noun without the storage-specific `Shelf` prefix.

export function validateShelfExport(value){
  let packet=value;
  if(typeof value==='string'){
    try{packet=JSON.parse(value);}catch(error){fail('invalid-packet','The shelf export is not valid JSON.',{cause:error});}
  }
  validateResearchShelfPacketIndex(packet);
  const records=packet.records.map(validateShelfRecord),collections=packet.collections.map(validateCollection);
  return {schema:SHELF_EXPORT_SCHEMA,version:SHELF_VERSION,generation:packet.generation,records,collections,
    ...(packet.exportedAt===undefined?{}:{exportedAt:packet.exportedAt})};
}

export function makeShelfExport({generation,records,collections,exportedAt=canonicalTimestamp()}){
  return validateShelfExport({schema:SHELF_EXPORT_SCHEMA,version:SHELF_VERSION,generation,exportedAt,records,collections});
}

function parseSuppliedPacket(value,label){
  let packet;
  if(typeof value==='string'){
    if(value.length>4_000_000)fail('limit',`The supplied ${label} packet is too large.`);
    try{packet=JSON.parse(value);}catch(error){fail('invalid-packet',`The supplied ${label} packet is not valid JSON.`,{cause:error});}
  }else packet=clone(value);
  if(!object(packet))fail('invalid-packet',`The supplied ${label} packet must be an object.`);
  return packet;
}

function migrationOptions(options={}){
  if(!object(options))fail('invalid-input','Supplied packet migration options must be an object.');
  const allowed=['source','lenses','now'];
  if(Object.keys(options).some(key=>!allowed.includes(key)))fail('invalid-input','Supplied packet migration options contain an unknown field.');
  if(!Object.values(SUPPLIED_PACKET_SOURCES).includes(options.source))
    fail('migration-required','Choose an explicit reading-resume, research-workspace, or workspace-copy packet source.');
  if(options.now!==undefined&&typeof options.now!=='function')fail('invalid-input','The migration clock must be a function.');
  if(options.lenses!==undefined&&!Array.isArray(options.lenses))fail('invalid-packet','Supplied lenses must be an array.');
  return {source:options.source,lenses:options.lenses,now:options.now};
}

function migrationTimes(now){
  if(now===undefined)return {recordAt:MIGRATION_RECORD_TIMESTAMP,exportedAt:canonicalTimestamp()};
  const value=timestamp(now(),'migration timestamp');
  return {recordAt:value,exportedAt:value};
}

function checkedReadingPacket(value){
  const packet=parseSuppliedPacket(value,'reading-resume');
  exactKeys(packet,['v','activeKey','entries']);
  try{return validateReading(packet);}catch(error){fail('invalid-packet','The supplied reading-resume packet is invalid.',{cause:error});}
}

function checkedResearchWorkspacePacket(value){
  const packet=parseSuppliedPacket(value,'research-workspace');
  if(packet.schema!==RESEARCH_WORKSPACE_SCHEMA||packet.version!==RESEARCH_WORKSPACE_VERSION)
    fail('invalid-packet','The supplied research-workspace packet has an unsupported owner schema.');
  try{
    // The owner parser remains authoritative for exact field and posture
    // checks. Exporting immediately gives the importer a detached canonical
    // packet without touching the browser persistence namespace.
    const workspace=createBrowserResearchWorkspace({persistence:false});
    try{workspace.importPacket(JSON.stringify(packet));return JSON.parse(workspace.exportPacket());}
    finally{if('dispose' in workspace)workspace.dispose();}
  }catch(error){fail('invalid-packet','The supplied research-workspace packet is invalid.',{cause:error});}
}

function checkedWorkspaceCopy(value){
  const packet=parseSuppliedPacket(value,'workspace-copy');
  // `validateWorkspaceCopy` owns the full copy contract. Check lens keys
  // before it normalizes the saved-lens section so an unknown field cannot be
  // silently dropped on this import boundary.
  preflightWorkspaceCopyLensFields(packet.lenses);
  try{return validateWorkspaceCopy(packet);}catch(error){fail('invalid-packet','The supplied workspace-copy packet is invalid.',{cause:error});}
}

function preflightWorkspaceCopyLensFields(value){
  if(value===undefined)return;
  if(!Array.isArray(value))fail('invalid-packet','Supplied lenses must be an array.');
  if(value.length>12)fail('limit','The supplied lens list is outside its bounds.');
  value.forEach((item,index)=>{
    try{researchShelfRule('validate_target',{type:'lens',target:{draft:item}});}
    catch(error){fail('invalid-packet',`The supplied lens at index ${index} has an invalid field shape.`,{cause:error});}
  });
}

function checkedLensList(value){
  if(value===undefined)return [];
  if(!Array.isArray(value))fail('invalid-packet','Supplied lenses must be an array.');
  if(value.length>12)fail('limit','The supplied lens list is outside its bounds.');
  const drafts=value.map((item,index)=>{
    try{return validateLensTarget({draft:item}).draft;}
    catch(error){fail('invalid-packet',`The supplied lens at index ${index} is invalid.`,{cause:error});}
  });
  try{validateWorkspaceCopyLenses(drafts);}
  catch(error){fail('invalid-packet','The supplied lens list contains duplicate names or is invalid.',{cause:error});}
  return drafts;
}

function addMaterial(records,retained,skipped,entry,source,index,at){
  const target={kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,contentRevision:entry.contentRevision,
    ...(entry.claimReference===undefined?{}:{claimReference:entry.claimReference})};
  const result=researchShelfRule('migration_record',{source,section:'entries',index,target:validateMaterialTarget(target),now:at,hasPositions:entry.positions.length>0});
  records.push(result.record);retained.push(result.retained);for(const item of result.skipped)skipped.push(item);
}

function addLens(records,retained,draft,source,index,at){
  const target={draft};
  const result=researchShelfRule('migration_record',{source,section:'lenses',index,target:validateLensTarget(target),now:at});
  records.push(result.record);retained.push(result.retained);
}

// Array traversal belongs to the host, but report layout/posture belongs to
// Rust. One bounded owner section enters; full packet bodies are never copied.
function skipSection(skipped,source,section,count,{single=false}={}){
  if(count===0)return;
  const result=researchShelfRule('migration_skips',{source,section,count,single});
  for(const item of result)skipped.push(item);
}

function carryReading(records,retained,skipped,reading,source,at){
  reading.entries.forEach((entry,index)=>{
    addMaterial(records,retained,skipped,entry,source,index,at);
  });
}

function carryResearchWorkspace(records,retained,skipped,workspace,source,at,lenses){
  skipSection(skipped,source,'selected_lens',workspace.selected_lens?1:0,{single:true});
  skipSection(skipped,source,'excluded_edge_ids',workspace.excluded_edge_ids.length);
  skipSection(skipped,source,'route_snapshots',workspace.route_snapshots.length);
  skipSection(skipped,source,'hypotheses',workspace.hypotheses.length);
  skipSection(skipped,source,'proposals',workspace.proposals.length);
  skipSection(skipped,source,'notes',workspace.notes.length);
  checkedLensList(lenses).forEach((draft,index)=>addLens(records,retained,draft,source,index,at));
}

/**
 * Prepare an additive shelf packet from an explicitly supplied owner export.
 * The owner source is mandatory and the packet is detached before validation;
 * no localStorage namespace is discovered, read, rewritten, or removed.
 * Only exact material addresses and separately supplied exact lens drafts are
 * retained. Session notes, graph poses and selections remain skipped with a
 * machine-readable reason.
 */
export function importSuppliedResearchPacket(value,options={}){
  const checked=migrationOptions(options),source=checked.source,times=migrationTimes(checked.now),at=times.recordAt;
  const records=[],retained=[],skipped=[];
  if(source===SUPPLIED_PACKET_SOURCES.READING_RESUME){
    carryReading(records,retained,skipped,checkedReadingPacket(value),source,at);
  }else if(source===SUPPLIED_PACKET_SOURCES.RESEARCH_WORKSPACE){
    carryResearchWorkspace(records,retained,skipped,checkedResearchWorkspacePacket(value),source,at,checked.lenses);
  }else{
    const copyPacket=checkedWorkspaceCopy(value);
    carryReading(records,retained,skipped,copyPacket.reading,source,at);
    copyPacket.lenses.forEach((draft,index)=>addLens(records,retained,draft,source,index,at));
    skipSection(skipped,source,'history',copyPacket.history?.entries?.length??0);
    skipSection(skipped,source,'places',copyPacket.places.length);
    skipSection(skipped,source,'resume',copyPacket.resume?1:0,{single:true});
    carryResearchWorkspace(records,retained,skipped,copyPacket.research,source,at);
  }
  return {source,packet:makeShelfExport({generation:0,records,collections:[],exportedAt:times.exportedAt}),retained,skipped};
}

export function validateMigrationOptions(options={}){return migrationOptions(options);}
