import {validateReference} from '../corpus-reader/model.mjs';
import {readingKey} from '../observatory/reader-model.mjs';
import {validateReading} from '../observatory/reading-resume.mjs';
import {exactFormRef,FORM_ROLES} from '../observatory/human-forms.mjs';
import {validateDraft} from '../observatory/lens-model.mjs';
import {COPY_SCHEMA,validateWorkspaceCopy} from '../observatory/workspace-copy.mjs';
import {RESEARCH_WORKSPACE_SCHEMA,RESEARCH_WORKSPACE_VERSION,createResearchWorkspace} from '../research-workspace.ts';

// The shelf is a local, user-owned index of exact addresses. It never owns a
// source passage, an exploration page, or a command/runtime handle.
export const SHELF_SCHEMA='tos.research_shelf.v1';
export const SHELF_VERSION=1;
export const SHELF_EXPORT_SCHEMA='tos.research_shelf.export.v1';
export const DEFAULT_DB_NAME='tos-research-shelf-v1';
/**
 * Creation/update time for imported material and lens records whose owner
 * packet has no shelf-record chronology. This is unknown local chronology,
 * not the time the source material or export wrapper was produced.
 */
export const MIGRATION_RECORD_TIMESTAMP='1970-01-01T00:00:00.000Z';
export const DEFAULT_PAGE_SIZE=24;
export const MAX_PAGE_SIZE=100;
export const MAX_RECORDS=200_000;
export const MAX_COLLECTIONS=2_000;
export const MAX_COLLECTION_IDS=32;
export const MAX_ID_LENGTH=1_024;
export const MAX_TITLE_LENGTH=256;
export const MAX_COLLECTION_TITLE_LENGTH=128;
export const MAX_TARGET_BYTES=200_000;
export const MAX_OPTION_LIST=100;
export const EXPLORATION_PROFILES=Object.freeze(['overview','all']);
export const EXPLORATION_DIRECTIONS=Object.freeze(['either','incoming','outgoing']);
export const EXPLORATION_DEPTH={min:0,max:10};
/** Explicit owner packet names accepted by the supplied-packet importer. */
export const SUPPLIED_PACKET_SOURCES=Object.freeze({
  READING_RESUME:'reading-resume',
  RESEARCH_WORKSPACE:'research-workspace',
  WORKSPACE_COPY:'workspace-copy',
});

const RECORD_FIELDS=['id','title','type','target','collectionIds','createdAt','updatedAt','revision'];
const INPUT_FIELDS=['id','title','type','target','collectionIds','createdAt','updatedAt','revision'];
const COLLECTION_FIELDS=['id','title','createdAt','updatedAt','revision'];
const EXPORT_FIELDS=['schema','version','generation','exportedAt','records','collections'];
const TARGET_FIELDS=Object.freeze({
  material:['kind','id','sourceRevision','contentRevision','claimReference'],
  form:['material','role','form'],
  text:['reference'],
  lens:['draft'],
  route:['origin','options'],
});
const CLAIM_FIELDS=['claimId','pathId','relationType','nodeIds','relationIds','detailRelationIds','closureNodeIds'];
const ROUTE_OPTION_FIELDS=['profile','direction','max_depth','sources','predicate_ids'];
const DRAFT_FIELDS=['v','name','scope','sources','nodeIds','focusId','query','kinds','predicates','depth','direction','profile','limit','relations','conditions','paths'];

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
const jsonBytes=value=>{
  let text;
  try{text=JSON.stringify(value);}catch(error){fail('invalid-input','The shelf value is not JSON serializable.',{cause:error});}
  return new TextEncoder().encode(text).byteLength;
};
const exactKeys=(value,required,optional=[])=>{
  if(!object(value))fail('invalid-target','The shelf value must be an object.');
  const allowed=new Set([...required,...optional]);
  if(Object.keys(value).some(key=>!allowed.has(key)))fail('invalid-target','The shelf value contains an unknown field.',{fields:Object.keys(value)});
  for(const key of required)if(!own(value,key))fail('invalid-target',`The shelf value is missing ${key}.`);
};
const boundedString=(value,max,label)=>{
  if(typeof value!=='string'||value.length===0||value.length>max||!value.isWellFormed?.()||/[\u0000-\u001f\u007f]/u.test(value))
    fail('invalid-target',`The ${label} must be a bounded string.`);
  return value;
};
const boundedId=(value,label='id')=>boundedString(value,MAX_ID_LENGTH,label);
const digest=value=>{
  if(typeof value!=='string'||!/^[a-f0-9]{64}$/.test(value))fail('invalid-target','A revision must be a lowercase SHA-256 digest.');
  return value;
};
const safeInt=(value,min,max,label)=>{
  if(!Number.isSafeInteger(value)||value<min||value>max)fail('invalid-input',`The ${label} is outside its supported bounds.`);
  return value;
};
const timestamp=(value,label)=>{
  if(typeof value!=='string'||value.length>64||Number.isNaN(Date.parse(value))){
    fail('invalid-record',`The ${label} is invalid.`);
  }
  let iso;
  try{iso=new Date(value).toISOString();}catch{fail('invalid-record',`The ${label} is invalid.`);}
  if(iso!==value)fail('invalid-record',`The ${label} must be a canonical ISO timestamp.`);
  return value;
};
const canonicalTimestamp=()=>new Date().toISOString();
const uniqueStrings=(value,limit,label,maxLength=MAX_ID_LENGTH,{allowEmpty=false}={})=>{
  if(!Array.isArray(value)||value.length>limit||(!allowEmpty&&!value.length))fail('invalid-target',`The ${label} list is outside its bounds.`);
  const values=value.map(item=>boundedString(item,maxLength,label));
  if(new Set(values).size!==values.length)fail('invalid-target',`The ${label} list contains duplicates.`);
  return values;
};

function canonicalClaimReference(value){
  exactKeys(value,CLAIM_FIELDS);
  for(const key of ['claimId','pathId','relationType'])boundedId(value[key],key);
  for(const key of ['nodeIds','relationIds','detailRelationIds','closureNodeIds']){
    const limit={nodeIds:3,relationIds:2,detailRelationIds:78,closureNodeIds:40}[key];
    uniqueStrings(value[key],limit,key,MAX_ID_LENGTH,{allowEmpty:key==='detailRelationIds'});
  }
  return clone(value);
}

/** Validate one graph material address, preserving an exact Claim binding. */
export function validateMaterialTarget(value){
  exactKeys(value,['kind','id','sourceRevision','contentRevision'],['claimReference']);
  if(!['node','relation'].includes(value.kind))fail('invalid-target','A material kind must be node or relation.');
  const result={kind:value.kind,id:boundedId(value.id),sourceRevision:digest(value.sourceRevision),contentRevision:digest(value.contentRevision)};
  if(own(value,'claimReference')){
    if(value.kind!=='node')fail('invalid-target','A Claim binding is only valid for a node material.');
    const claim=canonicalClaimReference(value.claimReference);
    // validateReading owns the compound reading contract. The wrapper is a
    // one-entry read so no local shelf rule can weaken its Claim checks.
    let reading;
    try{
      reading=validateReading({v:1,activeKey:readingKey(value.kind,value.id),entries:[{
        kind:value.kind,id:value.id,sourceRevision:value.sourceRevision,contentRevision:value.contentRevision,
        preferred:'default',positions:[],claimReference:claim,
      }]});
    }catch(error){fail('invalid-target','The material Claim binding is not a valid reading reference.',{cause:error});}
    result.claimReference=clone(reading.entries[0].claimReference);
  }
  if(jsonBytes(result)>MAX_TARGET_BYTES)fail('limit','The material target is too large.');
  return result;
}

export function validateFormTarget(value){
  exactKeys(value,['material','role','form']);
  if(!FORM_ROLES.includes(value.role))fail('invalid-target','The form role is not declared by the owner.');
  const material=validateMaterialTarget(value.material);
  if(!exactFormRef(value.form))fail('invalid-target','The form reference is not exact.');
  const form={id:boundedId(value.form.id,'form id'),version:safeInt(value.form.version,1,Number.MAX_SAFE_INTEGER,'form version'),digest:value.form.digest};
  if(typeof form.digest!=='string'||!/^sha256:[a-f0-9]{64}$/.test(form.digest))fail('invalid-target','The form digest is invalid.');
  return {material,role:value.role,form};
}

export function validateTextTarget(value){
  exactKeys(value,['reference']);
  let reference;
  try{reference=validateReference(value.reference);}catch(error){fail('invalid-target','The exact text reference is invalid.',{cause:error});}
  if(jsonBytes(reference)>MAX_TARGET_BYTES)fail('limit','The text target is too large.');
  return {reference:clone(reference)};
}

export function validateLensTarget(value){
  exactKeys(value,['draft']);
  if(!object(value.draft))fail('invalid-target','The lens draft must be an object.');
  if(Object.keys(value.draft).some(key=>!DRAFT_FIELDS.includes(key)))fail('invalid-target','The lens draft contains an unknown field.');
  let draft;
  try{draft=validateDraft(value.draft);}catch(error){fail('invalid-target','The lens draft is invalid.',{cause:error});}
  if(jsonBytes(draft)>MAX_TARGET_BYTES)fail('limit','The lens target is too large.');
  return {draft:clone(draft)};
}

export function validateRouteTarget(value){
  exactKeys(value,['origin','options']);
  const origin=validateMaterialTarget(value.origin);
  exactKeys(value.options,ROUTE_OPTION_FIELDS);
  if(!EXPLORATION_PROFILES.includes(value.options.profile))fail('invalid-target','The route profile is unsupported.');
  if(!EXPLORATION_DIRECTIONS.includes(value.options.direction))fail('invalid-target','The route direction is unsupported.');
  const max_depth=safeInt(value.options.max_depth,EXPLORATION_DEPTH.min,EXPLORATION_DEPTH.max,'route depth');
  const sources=uniqueStrings(value.options.sources,MAX_OPTION_LIST,'route sources');
  const predicate_ids=uniqueStrings(value.options.predicate_ids,MAX_OPTION_LIST,'route predicates',MAX_ID_LENGTH,{allowEmpty:true});
  return {origin,options:{profile:value.options.profile,direction:value.options.direction,max_depth,sources,predicate_ids}};
}

export function validateTarget(type,value){
  if(typeof type!=='string'||!Object.hasOwn(TARGET_FIELDS,type))fail('invalid-target','The shelf target type is unsupported.');
  switch(type){
    case 'material':return validateMaterialTarget(value);
    case 'form':return validateFormTarget(value);
    case 'text':return validateTextTarget(value);
    case 'lens':return validateLensTarget(value);
    case 'route':return validateRouteTarget(value);
    default:fail('invalid-target','The shelf target type is unsupported.');
  }
}

function checkCollections(value){
  if(value===undefined)return [];
  return uniqueStrings(value,MAX_COLLECTION_IDS,'collection ids',256,{allowEmpty:true});
}

/** Validate a complete stored record; all fields are retained in fixed order. */
export function validateShelfRecord(value){
  exactKeys(value,RECORD_FIELDS);
  const id=boundedId(value.id),title=boundedString(value.title,MAX_TITLE_LENGTH,'title').trim();
  if(!title)fail('invalid-record','The shelf title must not be blank.');
  if(!['material','form','text','lens','route'].includes(value.type))fail('invalid-record','The shelf record type is unsupported.');
  const target=validateTarget(value.type,value.target),collectionIds=checkCollections(value.collectionIds);
  const createdAt=timestamp(value.createdAt,'createdAt'),updatedAt=timestamp(value.updatedAt,'updatedAt');
  if(Date.parse(updatedAt)<Date.parse(createdAt))fail('invalid-record','updatedAt must not precede createdAt.');
  const revision=safeInt(value.revision,1,Number.MAX_SAFE_INTEGER,'record revision');
  const result={id,title,type:value.type,target,collectionIds,createdAt,updatedAt,revision};
  if(jsonBytes(result)>MAX_TARGET_BYTES+8_192)fail('limit','The shelf record is too large.');
  return result;
}

/** Build a new record from a user input; timestamps and revision are local. */
export function createShelfRecord(input,{id,now=canonicalTimestamp}={}){
  if(!object(input))fail('invalid-input','A shelf record input object is required.');
  if(Object.keys(input).some(key=>!INPUT_FIELDS.includes(key)))fail('invalid-input','The shelf input contains an unknown field.');
  const recordId=input.id===undefined?(typeof id==='function'?id():`shelf-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`):input.id;
  boundedId(recordId??'generated id');
  if(input.createdAt!==undefined||input.updatedAt!==undefined||input.revision!==undefined)
    fail('invalid-input','Stored timestamps and revisions cannot be supplied for a new shelf record.');
  const createdAt=timestamp(now(),'createdAt');
  const base={id:recordId,title:input.title,type:input.type,target:input.target,collectionIds:input.collectionIds??[],createdAt,updatedAt:createdAt,revision:1};
  return validateShelfRecord(base);
}

/** Normalize editable fields while preserving the stored address and creation time. */
export function updateShelfRecord(existing,input,{now=canonicalTimestamp}={}){
  const old=validateShelfRecord(existing);
  if(!object(input))fail('invalid-input','A shelf record input object is required.');
  if(Object.keys(input).some(key=>!INPUT_FIELDS.includes(key)))fail('invalid-input','The shelf input contains an unknown field.');
  if(input.id!==undefined&&input.id!==old.id)fail('invalid-input','A record update cannot change its id.');
  if(input.type!==undefined&&input.type!==old.type)fail('invalid-input','A record update cannot change its type.');
  if(input.target!==undefined&&JSON.stringify(validateTarget(old.type,input.target))!==JSON.stringify(old.target))
    fail('invalid-input','A record update cannot change its exact target.');
  if(input.createdAt!==undefined&&input.createdAt!==old.createdAt)fail('invalid-input','A record update cannot change createdAt.');
  const updatedAt=timestamp(now(),'updatedAt');
  return validateShelfRecord({id:old.id,title:input.title??old.title,type:old.type,target:old.target,
    collectionIds:input.collectionIds??old.collectionIds,createdAt:old.createdAt,updatedAt,revision:old.revision+1});
}

export function validateCollection(value){
  exactKeys(value,COLLECTION_FIELDS);
  const id=boundedString(value.id,256,'collection id'),title=boundedString(value.title,MAX_COLLECTION_TITLE_LENGTH,'collection title').trim();
  if(!title)fail('invalid-record','The collection title must not be blank.');
  const createdAt=timestamp(value.createdAt,'collection.createdAt'),updatedAt=timestamp(value.updatedAt,'collection.updatedAt');
  if(Date.parse(updatedAt)<Date.parse(createdAt))fail('invalid-record','collection.updatedAt must not precede createdAt.');
  return {id,title,createdAt,updatedAt,revision:safeInt(value.revision,1,Number.MAX_SAFE_INTEGER,'collection revision')};
}

export function createShelfCollection(input,{id,now=canonicalTimestamp}={}){
  if(!object(input))fail('invalid-input','A collection input object is required.');
  if(Object.keys(input).some(key=>!['id','title'].includes(key)))fail('invalid-input','The collection input contains an unknown field.');
  const collectionId=input.id??(typeof id==='function'?id():`collection-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`);
  boundedString(collectionId??'generated id',256,'collection id');
  const at=timestamp(now(),'collection.createdAt');
  return validateCollection({id:collectionId,title:input.title,createdAt:at,updatedAt:at,revision:1});
}

export function updateShelfCollection(existing,input,{now=canonicalTimestamp}={}){
  const old=validateCollection(existing);
  if(!object(input)||Object.keys(input).some(key=>!['id','title'].includes(key)))fail('invalid-input','The collection input contains an unknown field.');
  if(input.id!==undefined&&input.id!==old.id)fail('invalid-input','A collection update cannot change its id.');
  const at=timestamp(now(),'collection.updatedAt');
  return validateCollection({id:old.id,title:input.title??old.title,createdAt:old.createdAt,updatedAt:at,revision:old.revision+1});
}

function stable(value){
  if(Array.isArray(value))return `[${value.map(stable).join(',')}]`;
  if(object(value))return `{${Object.keys(value).sort().map(key=>`${JSON.stringify(key)}:${stable(value[key])}`).join(',')}}`;
  return JSON.stringify(value);
}
export const sameShelfValue=(left,right)=>stable(left)===stable(right);

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
  if(!object(options))fail('invalid-input','Shelf list options must be an object.');
  if(Object.keys(options).some(key=>!['limit','cursor','type','collectionId'].includes(key)))fail('invalid-input','Shelf list options contain an unknown field.');
  const limit=options.limit===undefined?DEFAULT_PAGE_SIZE:safeInt(options.limit,1,MAX_PAGE_SIZE,'page size');
  const type=options.type===undefined||options.type===null?null:options.type;
  if(type!==null&&!['material','form','text','lens','route'].includes(type))fail('invalid-input','The shelf type filter is unsupported.');
  const collectionId=options.collectionId===undefined||options.collectionId===null?null:boundedString(options.collectionId,256,'collection filter');
  const cursor=options.cursor===undefined||options.cursor===null?null:boundedString(options.cursor,16_384,'cursor');
  return {limit,type,collectionId,cursor};
}

export function encodeShelfCursor({generation,type=null,collectionId=null,updatedAt,id}){
  safeInt(generation,0,Number.MAX_SAFE_INTEGER,'shelf generation');
  timestamp(updatedAt,'cursor timestamp');boundedId(id);
  return encodeBase64({v:1,generation,filter:{type,collectionId},updatedAt,id});
}

export function decodeShelfCursor(cursor,{generation,type=null,collectionId=null}={}){
  if(cursor===null||cursor===undefined)return null;
  boundedString(cursor,16_384,'cursor');
  let value;
  try{value=JSON.parse(decodeBase64(cursor));}catch(error){fail('invalid-cursor','The shelf cursor is invalid.',{cause:error});}
  if(!object(value)||Object.keys(value).some(key=>!['v','generation','filter','updatedAt','id'].includes(key))||value.v!==1||!Number.isSafeInteger(value.generation)||value.generation<0||!object(value.filter)
    ||Object.keys(value.filter).some(key=>!['type','collectionId'].includes(key))
    ||(value.filter.type??null)!==(type??null)||(value.filter.collectionId??null)!==(collectionId??null)
    ||value.generation!==generation)fail('invalid-cursor','The shelf cursor belongs to another list generation or filter.');
  timestamp(value.updatedAt,'cursor timestamp');boundedId(value.id);
  return {generation:value.generation,filter:{type:value.filter.type??null,collectionId:value.filter.collectionId??null},updatedAt:value.updatedAt,id:value.id};
}

// Short aliases keep the model convenient for adapters and callers that use
// the noun without the storage-specific `Shelf` prefix.
export const validateRecord=validateShelfRecord;
export const validateCollectionRecord=validateCollection;

export function validateShelfExport(value){
  let packet=value;
  if(typeof value==='string'){
    try{packet=JSON.parse(value);}catch(error){fail('invalid-packet','The shelf export is not valid JSON.',{cause:error});}
  }
  exactKeys(packet,['schema','version','generation','records','collections'],['exportedAt']);
  if(packet.schema!==SHELF_EXPORT_SCHEMA||packet.version!==SHELF_VERSION)fail('invalid-packet','The shelf export version is unsupported.');
  const generation=safeInt(packet.generation,0,Number.MAX_SAFE_INTEGER,'export generation');
  if(!Array.isArray(packet.records)||packet.records.length>MAX_RECORDS||!Array.isArray(packet.collections)||packet.collections.length>MAX_COLLECTIONS)
    fail('limit','The shelf export is outside its record bounds.');
  if(packet.exportedAt!==undefined)timestamp(packet.exportedAt,'exportedAt');
  const records=packet.records.map(validateShelfRecord),collections=packet.collections.map(validateCollection);
  if(new Set(records.map(item=>item.id)).size!==records.length)fail('invalid-packet','The shelf export contains duplicate record ids.');
  if(new Set(collections.map(item=>item.id)).size!==collections.length)fail('invalid-packet','The shelf export contains duplicate collection ids.');
  return {schema:SHELF_EXPORT_SCHEMA,version:SHELF_VERSION,generation,records:records.map(clone),collections:collections.map(clone),
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

// Deterministic local identity keeps re-imports additive and idempotent while
// retaining no source wording or runtime state. It is a trace key, not a
// cryptographic digest.
function migrationHash(value){
  let hash=0xcbf29ce484222325n;
  for(const byte of new TextEncoder().encode(stable(value))){hash^=BigInt(byte);hash=BigInt.asUintN(64,hash*0x100000001b3n);}
  return hash.toString(16).padStart(16,'0');
}
function migrationRecordId(type,target){return `import:${type}:${migrationHash({target})}`;}

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
    const workspace=createResearchWorkspace({persistence:false});
    workspace.importPacket(JSON.stringify(packet));
    return JSON.parse(workspace.exportPacket());
  }catch(error){fail('invalid-packet','The supplied research-workspace packet is invalid.',{cause:error});}
}

function checkedWorkspaceCopy(value){
  const packet=parseSuppliedPacket(value,'workspace-copy');
  // `validateWorkspaceCopy` owns the full copy contract. Check lens keys
  // before it normalizes the saved-lens section so an unknown field cannot be
  // silently dropped on this import boundary.
  checkedLensList(packet.lenses);
  try{return validateWorkspaceCopy(packet);}catch(error){fail('invalid-packet','The supplied workspace-copy packet is invalid.',{cause:error});}
}

function checkedLensList(value){
  if(value===undefined)return [];
  if(!Array.isArray(value))fail('invalid-packet','Supplied lenses must be an array.');
  if(value.length>12)fail('limit','The supplied lens list is outside its bounds.');
  const drafts=value.map((item,index)=>{
    if(!object(item)||Object.keys(item).some(key=>!DRAFT_FIELDS.includes(key)))
      fail('invalid-packet',`The supplied lens at index ${index} contains an unknown field.`);
    try{return validateDraft(item);}catch(error){fail('invalid-packet',`The supplied lens at index ${index} is invalid.`,{cause:error});}
  });
  if(new Set(drafts.map(item=>item.name)).size!==drafts.length)fail('invalid-packet','The supplied lens list contains duplicate names.');
  return drafts;
}

function addMaterial(records,retained,entry,source,index,at){
  const target={kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,contentRevision:entry.contentRevision,
    ...(entry.claimReference===undefined?{}:{claimReference:entry.claimReference})};
  const record=createShelfRecord({id:migrationRecordId('material',target),title:'Материал',type:'material',target,collectionIds:[]},{now:()=>at});
  records.push(record);retained.push({source,section:'entries',index,type:'material',recordId:record.id});
}

function addLens(records,retained,draft,source,index,at){
  const target={draft};
  const record=createShelfRecord({id:migrationRecordId('lens',target),title:draft.name,type:'lens',target,collectionIds:[]},{now:()=>at});
  records.push(record);retained.push({source,section:'lenses',index,type:'lens',recordId:record.id});
}

function skip(skipped,source,section,index,reason){skipped.push({source,section,index,reason});}

function carryReading(records,retained,skipped,reading,source,at){
  reading.entries.forEach((entry,index)=>{
    addMaterial(records,retained,entry,source,index,at);
    if(entry.positions.length)skip(skipped,source,'entries',index,'non-portable-reading-position');
  });
}

function carryResearchWorkspace(records,retained,skipped,workspace,source,at,lenses){
  if(workspace.selected_lens)skip(skipped,source,'selected_lens',null,'non-portable-lens-selection');
  workspace.excluded_edge_ids.forEach((_,index)=>skip(skipped,source,'excluded_edge_ids',index,'legacy-graph-pose'));
  workspace.route_snapshots.forEach((_,index)=>skip(skipped,source,'route_snapshots',index,'legacy-graph-pose'));
  workspace.hypotheses.forEach((_,index)=>skip(skipped,source,'hypotheses',index,'non-portable-research-record'));
  workspace.proposals.forEach((_,index)=>skip(skipped,source,'proposals',index,'non-portable-research-record'));
  workspace.notes.forEach((_,index)=>skip(skipped,source,'notes',index,'non-portable-note'));
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
    copyPacket.history?.entries?.forEach((_,index)=>skip(skipped,source,'history',index,'legacy-graph-pose'));
    copyPacket.places.forEach((_,index)=>skip(skipped,source,'places',index,'legacy-graph-pose'));
    if(copyPacket.resume)skip(skipped,source,'resume',null,'legacy-graph-pose');
    carryResearchWorkspace(records,retained,skipped,copyPacket.research,source,at);
  }
  return {source,packet:makeShelfExport({generation:0,records,collections:[],exportedAt:times.exportedAt}),retained,skipped};
}

export function validateMigrationOptions(options={}){return migrationOptions(options);}

// Kept as a semantic alias for callers migrating from the old method name;
// there is no synthetic shelf-export v0 schema or implicit history here.
export const migrateResearchShelfExport=importSuppliedResearchPacket;

export const canonicalNow=canonicalTimestamp;
export const validateShelfTarget=validateTarget;
export const migrateShelfExport=importSuppliedResearchPacket;
