import {validateReference,referenceKey,referenceDocumentId,referenceVersionId} from './model.mjs';

// The corpus notebook is deliberately separate from the bounded passage reader.
// It stores user records and exact references only; it never stores the source
// text or a disposable text window.
export const NOTEBOOK_SCHEMA='tos_corpus_reader_notebook_v1';
export const NOTEBOOK_VERSION=1;
export const DEFAULT_DB_NAME='tos-corpus-reader-v1';
const DB_VERSION=1;
const META_KEY='state';
const STORE_META='meta';
const STORE_NOTES='notes';
const STORE_READINGS='readings';
const STORE_PREFERENCES='preferences';
const STORES=[STORE_META,STORE_NOTES,STORE_READINGS,STORE_PREFERENCES];
const NOTE_LIMIT=200_000;
const NOTE_TEXT_LIMIT=64_000;
const QUOTE_LIMIT=64_000;
const PREFERENCE_LIMIT=256_000;
const CURSOR_LIMIT=16_384;
const DOCUMENT_ID_LIMIT=2_048;
const VERSION_ID_LIMIT=2_048;
const KEY_LIMIT=512;
const MAX_PAGE=200;
export const READING_LIMIT=10_000;
export const READING_SLOT_PREFIX='reading:';
const NOTE_FIELDS=['id','kind','reference','referenceKey','documentId','quote','text','createdAt','updatedAt','revision'];
const READING_FIELDS=['slot','documentId','versionId','reference','referenceKey','offset','createdAt','updatedAt','revision'];

export class CorpusNotebookError extends Error {
  constructor(code,message,details={}){
    super(message||code);
    this.name='CorpusNotebookError';
    this.code=code;
    Object.assign(this,details);
  }
}

const fail=(code,message,details)=>{throw new CorpusNotebookError(code,message,details);};
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const hasOwn=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const clone=value=>typeof structuredClone==='function'?structuredClone(value):JSON.parse(JSON.stringify(value));
function bytes(value){
  let serialized;
  try{serialized=JSON.stringify(value);}catch{fail('invalid-input','The value is not JSON serializable.');}
  return new TextEncoder().encode(serialized).byteLength;
}
const nonEmptyString=(value,max)=>typeof value==='string'&&value.trim().length>0&&value.length<=max;
const optionalString=(value,max)=>value===undefined||nonEmptyString(value,max);
const nullableString=(value,max)=>value===null||nonEmptyString(value,max);
const integer=(value,min,max)=>Number.isSafeInteger(value)&&value>=min&&value<=max;

function now(){return new Date().toISOString();}

function id(){
  const random=globalThis.crypto?.randomUUID;
  if(typeof random==='function')return random.call(globalThis.crypto);
  return `note-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

function documentIdOf(reference){
  try{return referenceDocumentId(reference);}catch{}
  const values=[
    reference?.documentId,
    reference?.document_id,
    reference?.document?.id,
    reference?.document?.documentId,
    reference?.target?.documentId,
    reference?.target?.document_id,
    reference?.target?.workId,
  ];
  const found=values.find(value=>nonEmptyString(value,DOCUMENT_ID_LIMIT));
  return found??null;
}

function checkedReference(value){
  let checked;
  try{checked=validateReference(value);}catch(error){
    if(error instanceof CorpusNotebookError)throw error;
    fail('invalid-reference',error?.message||'The exact reference is invalid.');
  }
  if(!object(checked))fail('invalid-reference','The exact reference validator returned no object.');
  let key;
  try{key=referenceKey(checked);}catch(error){
    fail('invalid-reference',error?.message||'The exact reference has no stable key.');
  }
  if(!nonEmptyString(key,16_384))fail('invalid-reference','The exact reference has no stable key.');
  return {value:clone(checked),key};
}

function checkId(value){if(!nonEmptyString(value,2048))fail('invalid-input','The note id must be a non-empty string.');return value;}
function checkDocumentId(value){if(!nonEmptyString(value,DOCUMENT_ID_LIMIT))fail('invalid-input','The document id must be a non-empty string.');return value;}
function checkVersionId(value){if(!nonEmptyString(value,VERSION_ID_LIMIT))fail('invalid-input','The version id must be a non-empty string.');return value;}
function canonicalReadingSlot(workId,versionId){return `${READING_SLOT_PREFIX}${encodeURIComponent(workId)}:${encodeURIComponent(versionId)}`;}
function parsedReadingSlot(value){
  if(typeof value!=='string'||!value.startsWith(READING_SLOT_PREFIX))return null;
  const encoded=value.slice(READING_SLOT_PREFIX.length),separator=encoded.indexOf(':');
  if(separator<=0||encoded.indexOf(':',separator+1)!==-1)return null;
  let workId,versionId;
  try{workId=decodeURIComponent(encoded.slice(0,separator));versionId=decodeURIComponent(encoded.slice(separator+1));}catch{return null;}
  if(!nonEmptyString(workId,DOCUMENT_ID_LIMIT)||!nonEmptyString(versionId,VERSION_ID_LIMIT))return null;
  return canonicalReadingSlot(workId,versionId)===value?{workId,versionId}:null;
}
function checkSlot(value){
  if(value==='primary'||value==='secondary')return value;
  if(parsedReadingSlot(value))return value;
  fail('invalid-input','The reading slot must be primary, secondary, or a canonical version slot.');
}
/** Return the durable per-version reading key for an exact corpus reference. */
export function readingSlot(reference){
  const checked=checkedReference(reference).value;
  return canonicalReadingSlot(referenceDocumentId(checked),referenceVersionId(checked));
}
function checkKey(value){if(!nonEmptyString(value,KEY_LIMIT))fail('invalid-input','The preference key must be a non-empty string.');return value;}
function checkExpectedRevision(value){if(value!==undefined&&!integer(value,0,Number.MAX_SAFE_INTEGER))fail('invalid-input','The expected revision must be a non-negative safe integer.');return value;}
function checkExpectedRecordRevision(value){if(value!==undefined&&value!==null&&!integer(value,1,Number.MAX_SAFE_INTEGER))fail('invalid-input','The expected note revision must be a positive safe integer or null.');return value;}
function checkLimit(value){if(!integer(value,1,MAX_PAGE))fail('invalid-input',`The page limit must be an integer from 1 to ${MAX_PAGE}.`);return value;}
function checkText(value,max,label){if(value!==undefined&&(!nonEmptyString(value,max)||bytes(value)>max*4))fail('invalid-input',`The ${label} must be a bounded non-empty string.`);return value;}
function checkPreference(value){
  if(value===undefined)fail('invalid-input','A preference value is required.');
  let checked;
  try{checked=clone(value);}catch{fail('invalid-input','The preference value is not cloneable.');}
  if(bytes(checked)>PREFERENCE_LIMIT)fail('limit','The preference is too large.');
  return checked;
}

function checkTimestamp(value,label='timestamp'){
  if(!nonEmptyString(value,64)||!Number.isFinite(Date.parse(value))||new Date(value).toISOString()!==value)fail('invalid-packet',`The ${label} is invalid.`);
  return value;
}

function noteInput(input){
  if(!object(input))fail('invalid-input','A note input object is required.');
  const fields=['id','reference','quote','text','kind','expectedRevision','expectedRecordRevision'];
  if(Object.keys(input).some(key=>!fields.includes(key)))fail('invalid-input','The note input contains an unknown field.');
  if(input.id!==undefined)checkId(input.id);
  if(input.kind!=='note'&&input.kind!=='bookmark')fail('invalid-input','The note kind must be note or bookmark.');
  const reference=checkedReference(input.reference);
  checkText(input.quote,QUOTE_LIMIT,'quote');
  checkText(input.text,NOTE_TEXT_LIMIT,'note text');
  const expectedRevision=checkExpectedRevision(input.expectedRevision);
  const expectedRecordRevision=checkExpectedRecordRevision(input.expectedRecordRevision);
  if(input.id===undefined&&expectedRecordRevision!==undefined&&expectedRecordRevision!==null)fail('invalid-input','An expected note revision requires an existing note id.');
  return {
    id:input.id??id(),
    kind:input.kind,
    reference:reference.value,
    referenceKey:reference.key,
    documentId:documentIdOf(reference.value),
    ...(input.quote===undefined?{}:{quote:input.quote}),
    ...(input.text===undefined?{}:{text:input.text}),
    expectedRevision,expectedRecordRevision,
  };
}

function readingInput(input){
  if(!object(input))fail('invalid-input','A reading input object is required.');
  const fields=['slot','documentId','versionId','reference','offset'];
  if(Object.keys(input).some(key=>!fields.includes(key)))fail('invalid-input','The reading input contains an unknown field.');
  const slot=checkSlot(input.slot);
  const documentId=checkDocumentId(input.documentId);
  const versionId=checkVersionId(input.versionId);
  const reference=checkedReference(input.reference);
  const derivedDocumentId=documentIdOf(reference.value);
  if(derivedDocumentId!==null&&derivedDocumentId!==documentId)fail('invalid-input','The reading document id does not match the exact reference.');
  if(parsedReadingSlot(slot)&&(slot!==readingSlot(reference.value)||versionId!==referenceVersionId(reference.value)))fail('invalid-input','The canonical reading slot must match the exact work and version.');
  if(input.offset!==undefined&&!integer(input.offset,0,Number.MAX_SAFE_INTEGER))fail('invalid-input','The reading offset must be a non-negative safe integer.');
  return {
    slot,documentId,versionId,reference:reference.value,referenceKey:reference.key,
    ...(input.offset===undefined?{}:{offset:input.offset}),
  };
}

function storedNote(input,revision,createdAt=now(),updatedAt=createdAt){
  return {id:input.id,kind:input.kind,reference:clone(input.reference),referenceKey:input.referenceKey,
    documentId:input.documentId,...(input.quote===undefined?{}:{quote:input.quote}),
    ...(input.text===undefined?{}:{text:input.text}),createdAt,updatedAt,revision};
}

function storedReading(input,revision,createdAt=now(),updatedAt=createdAt){
  return {slot:input.slot,documentId:input.documentId,versionId:input.versionId,
    reference:clone(input.reference),referenceKey:input.referenceKey,
    ...(input.offset===undefined?{}:{offset:input.offset}),createdAt,updatedAt,revision};
}

function outputNote(value){return clone(value);}
function outputReading(value){return value?clone(value):null;}
function checkNoteRecordRevision(note,expected,id){
  if(expected===undefined)return;
  const actual=note?.revision??null;
  if((expected===null?actual!==null:actual!==expected))fail('conflict','The note changed in another context.',{id,expectedRecordRevision:expected,actualRecordRevision:actual});
}

function checkCursor(cursor,documentId){
  if(cursor===undefined||cursor===null)return null;
  if(!nonEmptyString(cursor,CURSOR_LIMIT))fail('invalid-cursor','The notebook cursor is invalid.');
  let value;
  try{value=JSON.parse(decodeBase64(cursor));}catch{fail('invalid-cursor','The notebook cursor is invalid.');}
  if(!object(value)||value.v!==1||!(value.documentId===null||nonEmptyString(value.documentId,DOCUMENT_ID_LIMIT))
    ||!nonEmptyString(value.updatedAt,64)||!nonEmptyString(value.id,2048))fail('invalid-cursor','The notebook cursor is invalid.');
  if((documentId??null)!==value.documentId)fail('invalid-cursor','The notebook cursor belongs to another document filter.');
  return value;
}

function encodeBase64(value){
  const text=JSON.stringify(value);
  if(typeof btoa==='function')return btoa(unescape(encodeURIComponent(text))).replaceAll('+','-').replaceAll('/','_').replace(/=+$/,'');
  return Buffer.from(text,'utf8').toString('base64url');
}
function decodeBase64(value){
  if(typeof atob==='function'){
    const base64=value.replaceAll('-','+').replaceAll('_','/');
    return decodeURIComponent(escape(atob(base64.padEnd(base64.length+((4-base64.length%4)%4),'='))));
  }
  return Buffer.from(value,'base64url').toString('utf8');
}
function cursorFor(note,documentId=null){return encodeBase64({v:1,documentId:documentId??null,updatedAt:note.updatedAt,id:note.id});}
function sortNotes(a,b){return b.updatedAt.localeCompare(a.updatedAt)||b.id.localeCompare(a.id);}
function afterCursor(note,cursor){return note.updatedAt<cursor.updatedAt||(note.updatedAt===cursor.updatedAt&&note.id<cursor.id);}

function mapStorageError(error){
  if(error instanceof CorpusNotebookError)return error;
  const name=error?.name;
  if(name==='QuotaExceededError')return new CorpusNotebookError('quota','The notebook storage quota was exceeded.',{cause:error});
  if(name==='VersionError'||name==='InvalidStateError')return new CorpusNotebookError('storage-unavailable','The notebook storage is unavailable.',{cause:error});
  return new CorpusNotebookError('storage-error',error?.message||'The notebook storage operation failed.',{cause:error});
}

function requestResult(request){return new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(mapStorageError(request.error));});}

function txPromise(db,stores,mode,setup){
  return new Promise((resolve,reject)=>{
    let tx,result,failed=false;
    try{tx=db.transaction(stores,mode);}catch(error){reject(mapStorageError(error));return;}
    const abort=error=>{if(failed)return;failed=true;try{tx.abort();}catch{}reject(error instanceof CorpusNotebookError?error:mapStorageError(error));};
    tx.onerror=()=>abort(tx.error||new Error('IndexedDB transaction failed'));
    tx.onabort=()=>{if(!failed)abort(tx.error||new Error('IndexedDB transaction aborted'));};
    tx.oncomplete=()=>{if(!failed){try{resolve(typeof result==='function'?result():result);}catch(error){reject(error);}}};
    try{result=setup(tx,abort);}catch(error){abort(error);}
  });
}

function openDatabase(indexedDB,dbName){
  return new Promise((resolve,reject)=>{
    let request;
    try{request=indexedDB.open(dbName,DB_VERSION);}catch(error){reject(mapStorageError(error));return;}
    request.onupgradeneeded=()=>{
      const db=request.result;
      const meta=db.objectStoreNames.contains(STORE_META)?request.transaction.objectStore(STORE_META):db.createObjectStore(STORE_META,{keyPath:'key'});
      if(!meta.indexNames.contains('byKey'))meta.createIndex('byKey','key',{unique:true});
      const notes=db.objectStoreNames.contains(STORE_NOTES)?request.transaction.objectStore(STORE_NOTES):db.createObjectStore(STORE_NOTES,{keyPath:'id'});
      if(!notes.indexNames.contains('byDocumentUpdated'))notes.createIndex('byDocumentUpdated',['documentId','updatedAt','id']);
      if(!notes.indexNames.contains('byUpdated'))notes.createIndex('byUpdated',['updatedAt','id']);
      if(!notes.indexNames.contains('byReference'))notes.createIndex('byReference','referenceKey');
      const readings=db.objectStoreNames.contains(STORE_READINGS)?request.transaction.objectStore(STORE_READINGS):db.createObjectStore(STORE_READINGS,{keyPath:'slot'});
      if(!readings.indexNames.contains('byDocument'))readings.createIndex('byDocument','documentId');
      const preferences=db.objectStoreNames.contains(STORE_PREFERENCES)?request.transaction.objectStore(STORE_PREFERENCES):db.createObjectStore(STORE_PREFERENCES,{keyPath:'key'});
      if(!preferences.indexNames.contains('byKey'))preferences.createIndex('byKey','key',{unique:true});
    };
    request.onerror=()=>reject(mapStorageError(request.error||new Error('IndexedDB could not open')));
    request.onblocked=()=>reject(new CorpusNotebookError('storage-unavailable','Another tab is blocking the notebook database upgrade.'));
    request.onsuccess=()=>{const db=request.result;db.onversionchange=()=>db.close();resolve(db);};
  });
}

function initialState(){return {revision:0,notes:new Map(),readings:new Map(),preferences:new Map()};}
function memoryCloneState(state){return {revision:state.revision,notes:new Map([...state.notes].map(([key,value])=>[key,clone(value)])),readings:new Map([...state.readings].map(([key,value])=>[key,clone(value)])),preferences:new Map([...state.preferences].map(([key,value])=>[key,clone(value)]))};}
function sharedMemoryState(value){
  if(value===undefined)return {state:initialState(),queue:Promise.resolve()};
  if(!object(value)||!(value.state?.notes instanceof Map)||!(value.state?.readings instanceof Map)||!(value.state?.preferences instanceof Map))fail('invalid-input','The memory adapter state is invalid.');
  return value;
}

function createMemoryBackend(shared,warning='memory-only'){
  const holder=sharedMemoryState(shared);
  const enqueue=task=>{const next=holder.queue.then(task);holder.queue=next.catch(()=>{});return next;};
  const backend={kind:'memory',persistent:false,warning,close(){return enqueue(()=>undefined)},
    read(task){return enqueue(()=>task(holder.state));},
    mutate(expectedRevision,task){return enqueue(()=>{if(expectedRevision!==undefined&&expectedRevision!==holder.state.revision)fail('conflict','The notebook changed in another context.',{expectedRevision,actualRevision:holder.state.revision});const actual=holder.state.revision;const draft=memoryCloneState(holder.state);const value=task(draft,actual,()=>{});draft.revision++;holder.state=draft;return {value,revision:draft.revision};});},
    merge(expectedRevision,packet){return enqueue(()=>{if(expectedRevision!==undefined&&expectedRevision!==holder.state.revision)fail('conflict','The notebook changed in another context.',{expectedRevision,actualRevision:holder.state.revision});const actual=holder.state.revision;const draft=memoryCloneState(holder.state);const plan=mergePlan(draft,packet);for(const item of plan.notes)draft.notes.set(item.id,clone(item));for(const item of plan.readings)draft.readings.set(item.slot,clone(item));for(const item of plan.preferences)draft.preferences.set(item.key,clone(item.value));const revision=plan.changed?actual+1:actual;if(plan.changed){draft.revision=revision;holder.state=draft;}return {value:plan.counts,revision};});},
  };
  return backend;
}

function metaRecord(revision){return {key:META_KEY,revision};}
function getMeta(tx){return tx.objectStore(STORE_META).get(META_KEY);}
function checkMeta(meta){return meta&&integer(meta.revision,0,Number.MAX_SAFE_INTEGER)?meta:{key:META_KEY,revision:0};}
function setRevision(tx,revision){tx.objectStore(STORE_META).put(metaRecord(revision));}

function createIdbBackend(db){
  const backend={kind:'indexeddb',persistent:true,warning:null,db,
    close(){db.close();return Promise.resolve();},
    read(task){
      return txPromise(db,STORES,'readonly',(tx,abort)=>{
        const result=task(tx,abort);return result;
      });
    },
    mutate(expectedRevision,task){
      return txPromise(db,[STORE_META,STORE_NOTES,STORE_READINGS,STORE_PREFERENCES],'readwrite',(tx,abort)=>{
        const metaRequest=getMeta(tx);let output;
        metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));
        metaRequest.onsuccess=()=>{
          try{
            const meta=checkMeta(metaRequest.result),actual=meta.revision;
            if(expectedRevision!==undefined&&expectedRevision!==actual)fail('conflict','The notebook changed in another context.',{expectedRevision,actualRevision:actual});
            output=task(tx,actual,abort);
            const next=actual+1;setRevision(tx,next);output={value:output,revision:next};
          }catch(error){abort(error);}
        };
        return ()=>({value:typeof output?.value==='function'?output.value():output?.value,revision:output?.revision});
      });
    },
    merge(expectedRevision,packet){
      return txPromise(db,STORES,'readwrite',(tx,abort)=>{
        const metaRequest=getMeta(tx);let output;
        metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));
        metaRequest.onsuccess=()=>{
          try{
            const meta=checkMeta(metaRequest.result),actual=meta.revision;
            if(expectedRevision!==undefined&&expectedRevision!==actual)fail('conflict','The notebook changed in another context.',{expectedRevision,actualRevision:actual});
            const noteRequest=withRequest(tx.objectStore(STORE_NOTES).getAll(),abort);
            const readingRequest=withRequest(tx.objectStore(STORE_READINGS).getAll(),abort);
            const preferenceRequest=withRequest(tx.objectStore(STORE_PREFERENCES).getAll(),abort);
            const values={notes:null,readings:null,preferences:null};let remaining=3;
            const finish=()=>{
              remaining-=1;if(remaining!==0)return;
              try{
                const current={notes:new Map(values.notes.map(item=>[item.id,item])),readings:new Map(values.readings.map(item=>[item.slot,item])),preferences:new Map(values.preferences.map(item=>[item.key,item.value]))};
                const plan=mergePlan(current,packet);
                for(const item of plan.notes)putRequest(tx,STORE_NOTES,clone(item),abort);
                for(const item of plan.readings)putRequest(tx,STORE_READINGS,clone(item),abort);
                for(const item of plan.preferences)putRequest(tx,STORE_PREFERENCES,{key:item.key,value:clone(item.value)},abort);
                const revision=plan.changed?actual+1:actual;if(plan.changed)setRevision(tx,revision);
                output={value:plan.counts,revision};
              }catch(error){abort(error);}
            };
            noteRequest.onsuccess=()=>{values.notes=noteRequest.result;finish();};
            readingRequest.onsuccess=()=>{values.readings=readingRequest.result;finish();};
            preferenceRequest.onsuccess=()=>{values.preferences=preferenceRequest.result;finish();};
          }catch(error){abort(error);}
        };
        return ()=>({value:output?.value,revision:output?.revision});
      });
    },
  };
  return backend;
}

function withRequest(request,abort){request.onerror=()=>abort(mapStorageError(request.error));return request;}
function putRequest(tx,store,value,abort){return withRequest(tx.objectStore(store).put(value),abort);}
function deleteRequest(tx,store,key,abort){return withRequest(tx.objectStore(store).delete(key),abort);}

function packetShape(packet){
  let value=packet;
  if(typeof packet==='string'){try{value=JSON.parse(packet);}catch{fail('invalid-packet','The notebook packet is not valid JSON.');}}
  if(!object(value)||value.schema!==NOTEBOOK_SCHEMA||value.version!==NOTEBOOK_VERSION
    ||!integer(value.revision,0,Number.MAX_SAFE_INTEGER)||!Array.isArray(value.notes)
    ||!Array.isArray(value.readings)||!Array.isArray(value.preferences))fail('invalid-packet','The notebook packet header is invalid.');
  if(Object.keys(value).some(key=>!['schema','version','revision','exportedAt','notes','readings','preferences'].includes(key)))fail('invalid-packet','The notebook packet contains an unknown field.');
  if(value.notes.length>NOTE_LIMIT||value.readings.length>READING_LIMIT||value.preferences.length>NOTE_LIMIT)fail('limit','The notebook packet has too many records.');
  if(value.exportedAt!==undefined)checkTimestamp(value.exportedAt,'exportedAt');
  const notes=new Map();
  for(const item of value.notes){
    if(!object(item)||!nonEmptyString(item.id,2048)||notes.has(item.id)||
      Object.keys(item).some(key=>!NOTE_FIELDS.includes(key))||
      (item.kind!=='note'&&item.kind!=='bookmark')||!object(item.reference)||!nonEmptyString(item.referenceKey,16_384)
      ||!nullableString(item.documentId,DOCUMENT_ID_LIMIT)||!nonEmptyString(item.createdAt,64)||!nonEmptyString(item.updatedAt,64)
      ||!integer(item.revision,1,Number.MAX_SAFE_INTEGER))fail('invalid-packet','A notebook note record is invalid.');
    let reference;
    try{reference=checkedReference(item.reference);}catch(error){fail('invalid-packet',error?.message||'A notebook note reference is invalid.');}
    if(reference.key!==item.referenceKey)fail('invalid-packet','A notebook note reference key is invalid.');
    if((item.documentId??null)!==documentIdOf(reference.value))fail('invalid-packet','A notebook note document id is invalid.');
    checkTimestamp(item.createdAt,'note.createdAt');checkTimestamp(item.updatedAt,'note.updatedAt');
    try{checkText(item.quote,QUOTE_LIMIT,'quote');checkText(item.text,NOTE_TEXT_LIMIT,'note text');}catch(error){fail('invalid-packet',error?.message||'A notebook note text field is invalid.');}
    const expected={id:item.id,kind:item.kind,reference:reference.value,referenceKey:reference.key,
      documentId:item.documentId??documentIdOf(reference.value),...(item.quote===undefined?{}:{quote:item.quote}),
      ...(item.text===undefined?{}:{text:item.text}),createdAt:item.createdAt,updatedAt:item.updatedAt,revision:item.revision};
    notes.set(expected.id,expected);
  }
  const readings=new Map();
  for(const item of value.readings){
    const validSlot=object(item)&&(()=>{try{return checkSlot(item.slot);}catch{return null;}})();
    if(!object(item)||!validSlot||readings.has(item.slot)
      ||Object.keys(item).some(key=>!READING_FIELDS.includes(key))
      ||!nonEmptyString(item.documentId,DOCUMENT_ID_LIMIT)||!nonEmptyString(item.versionId,VERSION_ID_LIMIT)
      ||!object(item.reference)||!nonEmptyString(item.referenceKey,16_384)||!nonEmptyString(item.createdAt,64)
      ||!nonEmptyString(item.updatedAt,64)||!integer(item.revision,1,Number.MAX_SAFE_INTEGER)
      ||(item.offset!==undefined&&!integer(item.offset,0,Number.MAX_SAFE_INTEGER)))fail('invalid-packet','A notebook reading record is invalid.');
    let reference;
    try{reference=checkedReference(item.reference);}catch(error){fail('invalid-packet',error?.message||'A notebook reading reference is invalid.');}
    if(reference.key!==item.referenceKey)fail('invalid-packet','A notebook reading reference key is invalid.');
    if(documentIdOf(reference.value)!==item.documentId)fail('invalid-packet','A notebook reading document id is invalid.');
    if(parsedReadingSlot(item.slot)&&(item.slot!==readingSlot(reference.value)||item.versionId!==referenceVersionId(reference.value)))fail('invalid-packet','A canonical reading slot does not match its exact work and version.');
    checkTimestamp(item.createdAt,'reading.createdAt');checkTimestamp(item.updatedAt,'reading.updatedAt');
    readings.set(item.slot,{slot:validSlot,documentId:item.documentId,versionId:item.versionId,reference:reference.value,referenceKey:reference.key,
      ...(item.offset===undefined?{}:{offset:item.offset}),createdAt:item.createdAt,updatedAt:item.updatedAt,revision:item.revision});
  }
  const preferences=new Map();
  for(const item of value.preferences){
    if(!object(item)||!nonEmptyString(item.key,KEY_LIMIT)||preferences.has(item.key)||!hasOwn(item,'value'))fail('invalid-packet','A notebook preference record is invalid.');
    try{preferences.set(item.key,checkPreference(item.value));}catch(error){fail('invalid-packet',error?.message||'A notebook preference value is invalid.');}
  }
  return {revision:value.revision,notes,readings,preferences};
}

function sameRecord(left,right){
  try{return JSON.stringify(left)===JSON.stringify(right);}catch{return false;}
}

function mergePlan(current,packet){
  const additions={notes:[],readings:[],preferences:[]};
  for(const [id,incoming] of packet.notes){
    const existing=current.notes.get(id);
    if(existing&&!sameRecord(existing,incoming))fail('conflict','An imported note conflicts with an existing note.',{id,kind:'note'});
    if(!existing)additions.notes.push(incoming);
  }
  for(const [slot,incoming] of packet.readings){if(!current.readings.has(slot))additions.readings.push(incoming);}
  if(current.readings.size+additions.readings.length>READING_LIMIT)fail('limit','The notebook has reached its reading position limit.');
  for(const [key,value] of packet.preferences){if(!current.preferences.has(key))additions.preferences.push({key,value});}
  return {...additions,changed:Object.values(additions).some(value=>Array.isArray(value)&&value.length>0),counts:{
    notes:additions.notes.length,readings:additions.readings.length,preferences:additions.preferences.length,
  }};
}

function makePacket({revision,notes,readings,preferences}){
  return {
    schema:NOTEBOOK_SCHEMA,version:NOTEBOOK_VERSION,revision,exportedAt:now(),
    notes:[...notes].map(([,value])=>outputNote(value)),
    readings:[...readings].map(([,value])=>outputReading(value)),
    preferences:[...preferences].map(([key,value])=>({key,value:clone(value)})),
  };
}

export function createMemoryCorpusNotebookState(){return {state:initialState(),queue:Promise.resolve()};}

export function createCorpusNotebook({indexedDB=globalThis.indexedDB,dbName=DEFAULT_DB_NAME,adapter='indexeddb',memoryStore}={}){
  if(!nonEmptyString(dbName,512))fail('invalid-input','The notebook database name must be a non-empty string.');
  const forceMemory=adapter==='memory'||adapter==='ephemeral'||indexedDB===null;
  let closed=false;
  let backendPromise;
  let backend;
  const status={adapter:'pending',persistent:false,warning:null,dbName,closed:false};
  if(forceMemory){
    backendPromise=Promise.resolve(createMemoryBackend(memoryStore,'memory-only'));
  }else if(!indexedDB||typeof indexedDB.open!=='function'){
    backendPromise=Promise.resolve(createMemoryBackend(memoryStore,'storage-unavailable'));
  }else{
    backendPromise=openDatabase(indexedDB,dbName).then(db=>createIdbBackend(db)).catch(error=>createMemoryBackend(memoryStore,error.code||'storage-unavailable'));
  }
  backendPromise=backendPromise.then(value=>{backend=value;status.adapter=value.kind;status.persistent=value.persistent;status.warning=value.warning;return value;});
  const ready=async()=>{if(closed)fail('closed','The notebook is closed.');const value=await backendPromise;if(closed)fail('closed','The notebook is closed.');return value;};
  const read=task=>ready().then(value=>value.read(task));
  const mutate=(expected,task)=>ready().then(value=>value.mutate(expected,task));
  const merge=(expected,packet)=>ready().then(value=>value.merge(expected,packet));
  const currentNoteRecord=(input,old,revision)=>storedNote(input,revision,old?.createdAt??now(),now());

  const api={
    status:()=>({...status,closed}),
    async putNote(input){
      const value=noteInput(input);const result=await mutate(value.expectedRevision,(draftOrTx,actual,abort)=>{
        if(draftOrTx instanceof Object&&draftOrTx.notes instanceof Map){
          const old=draftOrTx.notes.get(value.id);checkNoteRecordRevision(old,value.expectedRecordRevision,value.id);const record=currentNoteRecord(value,old,actual+1);draftOrTx.notes.set(value.id,record);return outputNote(record);
        }
        const tx=draftOrTx,store=tx.objectStore(STORE_NOTES);let old,record;
        const request=withRequest(store.get(value.id),abort);
        request.onsuccess=()=>{try{old=request.result;checkNoteRecordRevision(old,value.expectedRecordRevision,value.id);record=currentNoteRecord(value,old,actual+1);putRequest(tx,STORE_NOTES,record,abort);}catch(error){abort(error);}};
        return ()=>outputNote(record);
      });
      return {item:outputNote(result.value),revision:result.revision};
    },
    async deleteNote(noteId,expectedRevision,expectedRecordRevision){
      const idValue=checkId(noteId),expected=checkExpectedRevision(expectedRevision),expectedRecord=checkExpectedRecordRevision(expectedRecordRevision);
      const result=await mutate(expected,(draftOrTx,actual,abort)=>{
        if(draftOrTx instanceof Object&&draftOrTx.notes instanceof Map){
          const old=draftOrTx.notes.get(idValue);checkNoteRecordRevision(old,expectedRecord,idValue);const existed=draftOrTx.notes.delete(idValue);if(!existed)fail('not-found','The notebook note was not found.');return {id:idValue};
        }
        const request=withRequest(draftOrTx.objectStore(STORE_NOTES).get(idValue),abort);
        request.onsuccess=()=>{try{checkNoteRecordRevision(request.result,expectedRecord);if(!request.result)fail('not-found','The notebook note was not found.');else deleteRequest(draftOrTx,STORE_NOTES,idValue,abort);}catch(error){abort(error);}};
        return ()=>({id:idValue});
      });
      return {id:idValue,revision:result.revision};
    },
    async listNotes(options={}){
      if(!object(options))fail('invalid-input','The list options must be an object.');
      const {documentId,limit=50,cursor=null}=options;
      if(documentId!==undefined&&documentId!==null)checkDocumentId(documentId);const filterDocumentId=documentId??null;const pageLimit=checkLimit(limit);const checkedCursor=checkCursor(cursor,filterDocumentId);
      return read((stateOrTx)=>{
        if(stateOrTx instanceof Object&&stateOrTx.notes instanceof Map){
          let values=[...stateOrTx.notes.values()];if(filterDocumentId!==null)values=values.filter(item=>item.documentId===filterDocumentId);values.sort(sortNotes);if(checkedCursor)values=values.filter(item=>afterCursor(item,checkedCursor));const more=values.length>pageLimit;const items=values.slice(0,pageLimit).map(outputNote);return {items,nextCursor:more?cursorFor(values[pageLimit-1],filterDocumentId):null};
        }
        const index=filterDocumentId!==null?stateOrTx.objectStore(STORE_NOTES).index('byDocumentUpdated'):stateOrTx.objectStore(STORE_NOTES).index('byUpdated');
        let range;
        if(filterDocumentId!==null){const lower=[filterDocumentId,'',''];const upper=checkedCursor?[filterDocumentId,checkedCursor.updatedAt,checkedCursor.id]:[filterDocumentId,'\uffff','\uffff'];range=IDBKeyRange.bound(lower,upper,false,Boolean(checkedCursor));}
        else if(checkedCursor)range=IDBKeyRange.upperBound([checkedCursor.updatedAt,checkedCursor.id],true);
        const request=index.openCursor(range,'prev'),items=[];
        return new Promise((resolve,reject)=>{
          request.onerror=()=>reject(mapStorageError(request.error));
          request.onsuccess=()=>{
            const current=request.result;
            if(!current){const values=items.slice(0,pageLimit);resolve({items:values.map(outputNote),nextCursor:null});return;}
            items.push(current.value);
            if(items.length>pageLimit){const values=items.slice(0,pageLimit);resolve({items:values.map(outputNote),nextCursor:cursorFor(values.at(-1),filterDocumentId)});return;}
            current.continue();
          };
        });
      });
    },
    async saveReading(input){
      const value=readingInput(input);const result=await mutate(undefined,(draftOrTx,actual,abort)=>{
        if(draftOrTx instanceof Object&&draftOrTx.readings instanceof Map){const old=draftOrTx.readings.get(value.slot);if(!old&&draftOrTx.readings.size>=READING_LIMIT)fail('limit','The notebook has reached its reading position limit.');const record=storedReading(value,actual+1,old?.createdAt??now(),now());draftOrTx.readings.set(value.slot,record);return outputReading(record);}
        let record,count,old,remaining=2;
        const finish=()=>{if(--remaining!==0)return;try{if(!old&&count>=READING_LIMIT)fail('limit','The notebook has reached its reading position limit.');record=storedReading(value,actual+1,old?.createdAt??now(),now());putRequest(draftOrTx,STORE_READINGS,record,abort);}catch(error){abort(error);}};
        const countRequest=withRequest(draftOrTx.objectStore(STORE_READINGS).count(),abort);
        const request=withRequest(draftOrTx.objectStore(STORE_READINGS).get(value.slot),abort);
        countRequest.onsuccess=()=>{count=countRequest.result;finish();};
        request.onsuccess=()=>{old=request.result;finish();};
        return ()=>outputReading(record);
      });
      return {reading:result.value,revision:result.revision};
    },
    async loadReading(slot){const checked=checkSlot(slot);return read((stateOrTx)=>stateOrTx instanceof Object&&stateOrTx.readings instanceof Map?outputReading(stateOrTx.readings.get(checked)):requestResult(stateOrTx.objectStore(STORE_READINGS).get(checked)).then(outputReading));},
    async getPreference(key){const checked=checkKey(key);return read(stateOrTx=>stateOrTx instanceof Object&&stateOrTx.preferences instanceof Map?clone(stateOrTx.preferences.get(checked)):requestResult(stateOrTx.objectStore(STORE_PREFERENCES).get(checked)).then(value=>value===undefined?undefined:clone(value.value)));},
    async setPreference(key,value){const checked=checkKey(key),checkedValue=checkPreference(value);const result=await mutate(undefined,(draftOrTx,actual,abort)=>{if(draftOrTx instanceof Object&&draftOrTx.preferences instanceof Map){draftOrTx.preferences.set(checked,clone(checkedValue));return clone(checkedValue);}putRequest(draftOrTx,STORE_PREFERENCES,{key:checked,value:clone(checkedValue)},abort);return ()=>clone(checkedValue);});return {key:checked,value:result.value,revision:result.revision};},
    async exportPacket(){return read(stateOrTx=>{if(stateOrTx instanceof Object&&stateOrTx.notes instanceof Map)return makePacket(stateOrTx);const meta=stateOrTx.objectStore(STORE_META),notes=stateOrTx.objectStore(STORE_NOTES),readings=stateOrTx.objectStore(STORE_READINGS),preferences=stateOrTx.objectStore(STORE_PREFERENCES);return Promise.all([requestResult(meta.get(META_KEY)),requestResult(notes.getAll()),requestResult(readings.getAll()),requestResult(preferences.getAll())]).then(([m,n,r,p])=>makePacket({revision:checkMeta(m).revision,notes:new Map(n.map(v=>[v.id,v])),readings:new Map(r.map(v=>[v.slot,v])),preferences:new Map(p.map(v=>[v.key,v.value]))}));});},
    async importPacket(packet,options={}){
      if(!object(options)||Object.keys(options).some(key=>key!=='expectedRevision'))fail('invalid-input','The import options are invalid.');
      const expected=checkExpectedRevision(options.expectedRevision),parsed=packetShape(packet),result=await merge(expected,parsed);
      return {revision:result.revision};
    },
    async close(){if(closed)return;closed=true;status.closed=true;const value=backend??await backendPromise.catch(()=>null);if(value)await value.close();},
  };
  return api;
}
