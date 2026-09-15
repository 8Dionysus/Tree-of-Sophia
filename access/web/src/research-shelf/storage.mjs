import {
  DEFAULT_DB_NAME,DEFAULT_PAGE_SIZE,MAX_PAGE_SIZE,MAX_RECORDS,MAX_COLLECTIONS,
  ResearchShelfError,createShelfCollection,createShelfRecord,decodeShelfCursor,
  encodeShelfCursor,makeShelfExport,importSuppliedResearchPacket,sameShelfValue,
  updateShelfCollection,updateShelfRecord,validateCollection,validateListOptions,
  validateMigrationOptions,validateShelfExport,validateShelfRecord,
} from './model.mjs';

export const RESEARCH_SHELF_DB_VERSION=1;
export const RESEARCH_SHELF_STORES=Object.freeze({records:'records',collections:'collections',meta:'meta'});
const META_KEY='state';

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const copy=value=>typeof structuredClone==='function'?structuredClone(value):JSON.parse(JSON.stringify(value));
const fail=(code,message,details)=>{throw new ResearchShelfError(code,message,details);};
const validExpected=value=>value===undefined||value===null||Number.isSafeInteger(value)&&value>=1;
const expectedRevision=(value,label='record')=>{
  if(!validExpected(value))fail('invalid-input',`The expected ${label} revision is invalid.`);
  return value;
};
const expectedGeneration=value=>{
  if(value!==undefined&&!Number.isSafeInteger(value)||value!==undefined&&value<0)fail('invalid-input','The expected shelf generation is invalid.');
  return value;
};
const timestamp=()=>new Date().toISOString();
const generatedId=(prefix)=>{
  const random=globalThis.crypto?.randomUUID;
  if(typeof random==='function')return random.call(globalThis.crypto);
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
};

function mapStorageError(error){
  if(error instanceof ResearchShelfError)return error;
  const name=error?.name;
  if(name==='QuotaExceededError')return new ResearchShelfError('quota','The research shelf storage quota was exceeded.',{cause:error});
  if(name==='VersionError'||name==='InvalidStateError'||name==='AbortError')return new ResearchShelfError('storage-unavailable','The research shelf storage is unavailable.',{cause:error});
  return new ResearchShelfError('storage-error',error?.message||'The research shelf storage operation failed.',{cause:error});
}

function checkState(value){
  if(!object(value)||!Number.isSafeInteger(value.generation)||value.generation<0||!(value.records instanceof Map)||!(value.collections instanceof Map))
    fail('invalid-input','The memory research shelf state is invalid.');
  return value;
}
function freshState(){return {generation:0,records:new Map(),collections:new Map()};}
export function createMemoryResearchShelfState(){return {state:freshState(),queue:Promise.resolve()};}
function sharedState(value){
  if(value===undefined)return createMemoryResearchShelfState();
  if(value?.state instanceof Object)return checkState(value.state),value;
  if(value instanceof Object&&value.records instanceof Map&&value.collections instanceof Map)return {state:value,queue:Promise.resolve()};
  fail('invalid-input','The memory research shelf state is invalid.');
}
function cloneState(state){return {generation:state.generation,records:new Map([...state.records].map(([id,v])=>[id,copy(v)])),collections:new Map([...state.collections].map(([id,v])=>[id,copy(v)]))};}
function enqueue(shared,task){
  const next=shared.queue.then(task);
  shared.queue=next.catch(()=>{});
  return next;
}
function sortDesc(left,right){return right.updatedAt.localeCompare(left.updatedAt)||right.id.localeCompare(left.id);}
function afterCursor(value,cursor){return value.updatedAt<cursor.updatedAt||(value.updatedAt===cursor.updatedAt&&value.id<cursor.id);}
function filterRecords(records,{type=null,collectionId=null}){
  return records.filter(record=>(type===null||record.type===type)&&(collectionId===null||record.collectionIds.includes(collectionId)));
}
function pageValues(values,options,generation){
  const cursor=decodeShelfCursor(options.cursor,{generation,type:options.type,collectionId:options.collectionId});
  const sorted=values.slice().sort(sortDesc),visible=cursor?sorted.filter(item=>afterCursor(item,cursor)):sorted;
  const items=visible.slice(0,options.limit).map(copy),last=items.at(-1);
  return {items,nextCursor:visible.length>options.limit&&last?encodeShelfCursor({generation,type:options.type,collectionId:options.collectionId,updatedAt:last.updatedAt,id:last.id}):null,generation};
}

function checkImportCapacity(state,packet){
  if(state.records.size+packet.records.filter(item=>!state.records.has(item.id)).length>MAX_RECORDS)fail('limit','The research shelf has reached its record limit.');
  if(state.collections.size+packet.collections.filter(item=>!state.collections.has(item.id)).length>MAX_COLLECTIONS)fail('limit','The research shelf has reached its collection limit.');
}
function compareIncoming(state,packet){
  for(const incoming of packet.records){
    const existing=state.records.get(incoming.id);
    if(existing&&!sameShelfValue(existing,incoming))fail('conflict','An imported shelf record conflicts with an existing record.',{id:incoming.id,kind:'record'});
  }
  for(const incoming of packet.collections){
    const existing=state.collections.get(incoming.id);
    if(existing&&!sameShelfValue(existing,incoming))fail('conflict','An imported collection conflicts with an existing collection.',{id:incoming.id,kind:'collection'});
  }
}
/**
 * Memory adapter used by tests and as the explicit degraded-mode fallback.
 * The adapter contract is asynchronous even though its implementation is not.
 */
export function createMemoryResearchShelfAdapter({memoryStore,state,now=timestamp,warning='memory-only'}={}){
  const shared=sharedState(memoryStore??state),status={adapter:'memory',persistent:false,warning,closed:false};
  const ready=()=>{if(status.closed)fail('closed','The research shelf is closed.');return Promise.resolve();};
  const read=task=>enqueue(shared,async()=>{await ready();return task(shared.state);});
  const mutate=(task)=>enqueue(shared,async()=>{await ready();const draft=cloneState(shared.state),value=await task(draft,shared.state);shared.state=draft;return value;});
  return {
    kind:'memory',persistent:false,warning,status:()=>({...status}),ready,
    async getMeta(){return read(stateValue=>({generation:stateValue.generation}));},
    async getRecord(id){return read(stateValue=>stateValue.records.has(id)?copy(stateValue.records.get(id)):null);},
    async listRecords(options){return read(stateValue=>pageValues(filterRecords([...stateValue.records.values()],options),options,stateValue.generation));},
    async saveRecord(record,expected){
      const value=validateShelfRecord(record);expectedRevision(expected);
      return mutate((draft,current)=>{
        const old=current.records.get(value.id),actual=old?.revision??null;
        if(expected===undefined){if(old)fail('conflict','A shelf record with this id already exists.',{id:value.id,expectedRevision:undefined,actualRecordRevision:actual});}
        else if(expected===null){if(old)fail('conflict','A shelf record with this id already exists.',{id:value.id,expectedRevision:null,actualRecordRevision:actual});}
        else if(!old||old.revision!==expected)fail('conflict','The shelf record changed in another context.',{id:value.id,expectedRevision:expected,actualRecordRevision:actual});
        const required=old?old.revision+1:1;if(value.revision!==required)fail('invalid-record','The shelf record revision does not match the CAS operation.');
        draft.records.set(value.id,copy(value));draft.generation=current.generation+1;
        return {item:copy(value),generation:draft.generation};
      });
    },
    async deleteRecord(id,expected){
      expectedRevision(expected);if(expected===null||expected===undefined)fail('invalid-input','Deleting a shelf record requires its current revision.');
      return mutate((draft,current)=>{const old=current.records.get(id);if(!old)fail('not-found','The shelf record was not found.',{id});if(old.revision!==expected)fail('conflict','The shelf record changed in another context.',{id,expectedRevision:expected,actualRecordRevision:old.revision});draft.records.delete(id);draft.generation=current.generation+1;return {id,generation:draft.generation};});
    },
    async getCollection(id){return read(stateValue=>stateValue.collections.has(id)?copy(stateValue.collections.get(id)):null);},
    async listCollections(){return read(stateValue=>({items:[...stateValue.collections.values()].sort(sortDesc).map(copy),generation:stateValue.generation}));},
    async saveCollection(collection,expected){
      const value=validateCollection(collection);expectedRevision(expected,'collection');
      return mutate((draft,current)=>{const old=current.collections.get(value.id),actual=old?.revision??null;
        if(expected===undefined){if(old)fail('conflict','A collection with this id already exists.',{id:value.id,expectedRevision:undefined,actualRecordRevision:actual});}
        else if(expected===null){if(old)fail('conflict','A collection with this id already exists.',{id:value.id,expectedRevision:null,actualRecordRevision:actual});}
        else if(!old||old.revision!==expected)fail('conflict','The collection changed in another context.',{id:value.id,expectedRevision:expected,actualRecordRevision:actual});
        const required=old?old.revision+1:1;if(value.revision!==required)fail('invalid-record','The collection revision does not match the CAS operation.');
        draft.collections.set(value.id,copy(value));draft.generation=current.generation+1;return {item:copy(value),generation:draft.generation};});
    },
    async deleteCollection(id,expected){
      expectedRevision(expected,'collection');if(expected===null||expected===undefined)fail('invalid-input','Deleting a collection requires its current revision.');
      return mutate((draft,current)=>{const old=current.collections.get(id);if(!old)fail('not-found','The collection was not found.',{id});if(old.revision!==expected)fail('conflict','The collection changed in another context.',{id,expectedRevision:expected,actualRecordRevision:old.revision});draft.collections.delete(id);draft.generation=current.generation+1;return {id,generation:draft.generation};});
    },
    async exportPacket(){return read(stateValue=>makeShelfExport({generation:stateValue.generation,records:[...stateValue.records.values()],collections:[...stateValue.collections.values()]}));},
    async importPacket(packet,expected){
      const parsed=validateShelfExport(packet);expectedGeneration(expected);
      return mutate((draft,current)=>{if(expected!==undefined&&expected!==current.generation)fail('conflict','The research shelf changed in another context.',{expectedGeneration:expected,actualGeneration:current.generation});
        // Check against a pre-commit clone so a later collision can never leave
        // earlier imported records behind.
        const plan=cloneState(current);const beforeRecords=plan.records.size,beforeCollections=plan.collections.size;compareIncoming(plan,parsed);checkImportCapacity(plan,parsed);
        for(const incoming of parsed.collections)if(!plan.collections.has(incoming.id))plan.collections.set(incoming.id,copy(incoming));
        for(const incoming of parsed.records)if(!plan.records.has(incoming.id))plan.records.set(incoming.id,copy(incoming));
        const changed=plan.records.size!==beforeRecords||plan.collections.size!==beforeCollections;
        plan.generation=changed?current.generation+1:current.generation;draft.generation=plan.generation;draft.records=plan.records;draft.collections=plan.collections;
        return {generation:plan.generation,counts:{records:plan.records.size-beforeRecords,collections:plan.collections.size-beforeCollections},changed};});
    },
    async close(){status.closed=true;},
  };
}

function requestResult(request,failRequest){
  return new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>{const error=mapStorageError(request.error);failRequest?.(error);reject(error);};});
}
function transactionPromise(db,stores,mode,setup){
  return new Promise((resolve,reject)=>{
    let tx,result,failed=false;
    try{tx=db.transaction(stores,mode);}catch(error){reject(mapStorageError(error));return;}
    const abort=error=>{if(failed)return;failed=true;try{tx.abort();}catch{};reject(error instanceof ResearchShelfError?error:mapStorageError(error));};
    tx.onerror=()=>abort(tx.error||new Error('IndexedDB transaction failed'));
    tx.onabort=()=>{if(!failed)abort(tx.error||new Error('IndexedDB transaction aborted'));};
    tx.oncomplete=()=>{if(!failed){try{resolve(typeof result==='function'?result():result);}catch(error){abort(error);}}};
    try{setup(tx,value=>{result=value;},abort);}catch(error){abort(error);}
  });
}
function openDatabase(indexedDB,dbName){
  return new Promise((resolve,reject)=>{
    let request;
    try{request=indexedDB.open(dbName,RESEARCH_SHELF_DB_VERSION);}catch(error){reject(mapStorageError(error));return;}
    request.onupgradeneeded=()=>{
      const db=request.result;
      const meta=db.objectStoreNames.contains(RESEARCH_SHELF_STORES.meta)?request.transaction.objectStore(RESEARCH_SHELF_STORES.meta):db.createObjectStore(RESEARCH_SHELF_STORES.meta,{keyPath:'key'});
      const records=db.objectStoreNames.contains(RESEARCH_SHELF_STORES.records)?request.transaction.objectStore(RESEARCH_SHELF_STORES.records):db.createObjectStore(RESEARCH_SHELF_STORES.records,{keyPath:'id'});
      if(!records.indexNames.contains('byUpdated'))records.createIndex('byUpdated',['updatedAt','id']);
      if(!records.indexNames.contains('byTypeUpdated'))records.createIndex('byTypeUpdated',['type','updatedAt','id']);
      if(!records.indexNames.contains('byCollection'))records.createIndex('byCollection','collectionIds',{multiEntry:true});
      const collections=db.objectStoreNames.contains(RESEARCH_SHELF_STORES.collections)?request.transaction.objectStore(RESEARCH_SHELF_STORES.collections):db.createObjectStore(RESEARCH_SHELF_STORES.collections,{keyPath:'id'});
      if(!collections.indexNames.contains('byUpdated'))collections.createIndex('byUpdated',['updatedAt','id']);
      meta.put({key:META_KEY,generation:0});
    };
    request.onerror=()=>reject(mapStorageError(request.error||new Error('IndexedDB could not open')));
    request.onblocked=()=>reject(new ResearchShelfError('storage-unavailable','Another tab is blocking the research shelf database upgrade.'));
    request.onsuccess=()=>{const db=request.result;db.onversionchange=()=>db.close();resolve(db);};
  });
}
function idbGet(db,store,id){return transactionPromise(db,[store],'readonly',(tx,set,abort)=>{const request=tx.objectStore(store).get(id);request.onsuccess=()=>set(request.result===undefined?null:copy(request.result));request.onerror=()=>abort(mapStorageError(request.error));});}
function metaFrom(value){return value&&Number.isSafeInteger(value.generation)&&value.generation>=0?value:{key:META_KEY,generation:0};}

function idbListRecords(db,options){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.records],'readonly',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY);metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));
    metaRequest.onsuccess=()=>{
      try{
        const generation=metaFrom(metaRequest.result).generation,cursor=decodeShelfCursor(options.cursor,{generation,type:options.type,collectionId:options.collectionId});
        const index=tx.objectStore(RESEARCH_SHELF_STORES.records).index('byUpdated');
        const range=cursor?IDBKeyRange.upperBound([cursor.updatedAt,cursor.id],true):undefined;
        const request=index.openCursor(range,'prev'),items=[];
        request.onerror=()=>abort(mapStorageError(request.error));
        request.onsuccess=()=>{
          const current=request.result;
          if(!current){const last=items.at(-1);set({items:items.map(copy),nextCursor:null,generation});return;}
          const value=current.value;
          if((options.type===null||value.type===options.type)&&(options.collectionId===null||value.collectionIds.includes(options.collectionId)))items.push(value);
          if(items.length>options.limit){const page=items.slice(0,options.limit),last=page.at(-1);set({items:page.map(copy),nextCursor:encodeShelfCursor({generation,type:options.type,collectionId:options.collectionId,updatedAt:last.updatedAt,id:last.id}),generation});return;}
          current.continue();
        };
      }catch(error){abort(error);}
    };
  });
}
function idbListCollections(db){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.collections],'readonly',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),request=tx.objectStore(RESEARCH_SHELF_STORES.collections).getAll();
    metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));request.onerror=()=>abort(mapStorageError(request.error));
    let generation,items;const finish=()=>{if(generation===undefined||items===undefined)return;set({items:items.sort(sortDesc).map(copy),generation});};
    metaRequest.onsuccess=()=>{generation=metaFrom(metaRequest.result).generation;finish();};request.onsuccess=()=>{items=request.result;finish();};
  });
}
function idbSaveRecord(db,record,expected){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.records],'readwrite',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),recordRequest=tx.objectStore(RESEARCH_SHELF_STORES.records).get(record.id);let meta,old;
    const finish=()=>{
      if(meta===undefined||old===undefined)return;
      try{
        const generation=metaFrom(meta).generation,actual=old?.revision??null;
        if(expected===undefined){if(old)fail('conflict','A shelf record with this id already exists.',{id:record.id,expectedRevision:undefined,actualRecordRevision:actual});}
        else if(expected===null){if(old)fail('conflict','A shelf record with this id already exists.',{id:record.id,expectedRevision:null,actualRecordRevision:actual});}
        else if(!old||old.revision!==expected)fail('conflict','The shelf record changed in another context.',{id:record.id,expectedRevision:expected,actualRecordRevision:actual});
        const required=old?old.revision+1:1;if(record.revision!==required)fail('invalid-record','The shelf record revision does not match the CAS operation.');
        tx.objectStore(RESEARCH_SHELF_STORES.records).put(copy(record));tx.objectStore(RESEARCH_SHELF_STORES.meta).put({key:META_KEY,generation:generation+1});set({item:copy(record),generation:generation+1});
      }catch(error){abort(error);}
    };
    metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));recordRequest.onerror=()=>abort(mapStorageError(recordRequest.error));
    metaRequest.onsuccess=()=>{meta=metaRequest.result;finish();};recordRequest.onsuccess=()=>{old=recordRequest.result===undefined?null:recordRequest.result;finish();};
  });
}
function idbDeleteRecord(db,id,expected){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.records],'readwrite',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),recordRequest=tx.objectStore(RESEARCH_SHELF_STORES.records).get(id);let meta,old;
    const finish=()=>{if(meta===undefined||old===undefined)return;try{const generation=metaFrom(meta).generation;if(!old)fail('not-found','The shelf record was not found.',{id});if(old.revision!==expected)fail('conflict','The shelf record changed in another context.',{id,expectedRevision:expected,actualRecordRevision:old.revision});tx.objectStore(RESEARCH_SHELF_STORES.records).delete(id);tx.objectStore(RESEARCH_SHELF_STORES.meta).put({key:META_KEY,generation:generation+1});set({id,generation:generation+1});}catch(error){abort(error);}};
    metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));recordRequest.onerror=()=>abort(mapStorageError(recordRequest.error));metaRequest.onsuccess=()=>{meta=metaRequest.result;finish();};recordRequest.onsuccess=()=>{old=recordRequest.result===undefined?null:recordRequest.result;finish();};
  });
}
function idbSaveCollection(db,collection,expected){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.collections],'readwrite',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),collectionRequest=tx.objectStore(RESEARCH_SHELF_STORES.collections).get(collection.id);let meta,old;
    const finish=()=>{if(meta===undefined||old===undefined)return;try{const generation=metaFrom(meta).generation,actual=old?.revision??null;if(expected===undefined){if(old)fail('conflict','A collection with this id already exists.',{id:collection.id,expectedRevision:undefined,actualRecordRevision:actual});}else if(expected===null){if(old)fail('conflict','A collection with this id already exists.',{id:collection.id,expectedRevision:null,actualRecordRevision:actual});}else if(!old||old.revision!==expected)fail('conflict','The collection changed in another context.',{id:collection.id,expectedRevision:expected,actualRecordRevision:actual});const required=old?old.revision+1:1;if(collection.revision!==required)fail('invalid-record','The collection revision does not match the CAS operation.');tx.objectStore(RESEARCH_SHELF_STORES.collections).put(copy(collection));tx.objectStore(RESEARCH_SHELF_STORES.meta).put({key:META_KEY,generation:generation+1});set({item:copy(collection),generation:generation+1});}catch(error){abort(error);}};
    metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));collectionRequest.onerror=()=>abort(mapStorageError(collectionRequest.error));metaRequest.onsuccess=()=>{meta=metaRequest.result;finish();};collectionRequest.onsuccess=()=>{old=collectionRequest.result===undefined?null:collectionRequest.result;finish();};
  });
}
function idbDeleteCollection(db,id,expected){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.collections],'readwrite',(tx,set,abort)=>{const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),collectionRequest=tx.objectStore(RESEARCH_SHELF_STORES.collections).get(id);let meta,old;const finish=()=>{if(meta===undefined||old===undefined)return;try{const generation=metaFrom(meta).generation;if(!old)fail('not-found','The collection was not found.',{id});if(old.revision!==expected)fail('conflict','The collection changed in another context.',{id,expectedRevision:expected,actualRecordRevision:old.revision});tx.objectStore(RESEARCH_SHELF_STORES.collections).delete(id);tx.objectStore(RESEARCH_SHELF_STORES.meta).put({key:META_KEY,generation:generation+1});set({id,generation:generation+1});}catch(error){abort(error);}};metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));collectionRequest.onerror=()=>abort(mapStorageError(collectionRequest.error));metaRequest.onsuccess=()=>{meta=metaRequest.result;finish();};collectionRequest.onsuccess=()=>{old=collectionRequest.result===undefined?null:collectionRequest.result;finish();};});
}
function idbExport(db){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.records,RESEARCH_SHELF_STORES.collections],'readonly',(tx,set,abort)=>{const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY),recordsRequest=tx.objectStore(RESEARCH_SHELF_STORES.records).getAll(),collectionsRequest=tx.objectStore(RESEARCH_SHELF_STORES.collections).getAll();let meta,records,collections;const finish=()=>{if(meta===undefined||records===undefined||collections===undefined)return;set(makeShelfExport({generation:metaFrom(meta).generation,records,collections}));};for(const request of [metaRequest,recordsRequest,collectionsRequest])request.onerror=()=>abort(mapStorageError(request.error));metaRequest.onsuccess=()=>{meta=metaRequest.result;finish();};recordsRequest.onsuccess=()=>{records=recordsRequest.result;finish();};collectionsRequest.onsuccess=()=>{collections=collectionsRequest.result;finish();};});
}
function idbImport(db,packet,expected){
  return transactionPromise(db,[RESEARCH_SHELF_STORES.meta,RESEARCH_SHELF_STORES.records,RESEARCH_SHELF_STORES.collections],'readwrite',(tx,set,abort)=>{
    const metaRequest=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY);
    metaRequest.onerror=()=>abort(mapStorageError(metaRequest.error));
    metaRequest.onsuccess=()=>{
      try{
        const current=metaFrom(metaRequest.result);
        if(expected!==undefined&&expected!==current.generation)
          fail('conflict','The research shelf changed in another context.',{expectedGeneration:expected,actualGeneration:current.generation});
        const additions={records:[],collections:[]};
        let recordIndex=0,collectionIndex=0,recordTotal,collectionTotal;
        const finish=()=>{
          try{
            if(current.generation===undefined)fail('storage-error','The shelf metadata is invalid.');
            // IndexedDB reads above are all in this transaction. No put is
            // issued until every incoming id has been checked, preserving
            // additive import atomicity when the final id conflicts.
            if(recordTotal+additions.records.length>MAX_RECORDS||collectionTotal+additions.collections.length>MAX_COLLECTIONS)
              fail('limit','The research shelf import is outside its bounds.');
            const generation=additions.records.length||additions.collections.length?current.generation+1:current.generation;
            for(const item of additions.collections)tx.objectStore(RESEARCH_SHELF_STORES.collections).put(copy(item));
            for(const item of additions.records)tx.objectStore(RESEARCH_SHELF_STORES.records).put(copy(item));
            if(additions.records.length||additions.collections.length)
              tx.objectStore(RESEARCH_SHELF_STORES.meta).put({key:META_KEY,generation});
            set({generation,counts:{records:additions.records.length,collections:additions.collections.length},changed:Boolean(additions.records.length||additions.collections.length)});
          }catch(error){abort(error);}
        };
        const nextRecord=()=>{
          if(recordIndex>=packet.records.length)return finish();
          const incoming=packet.records[recordIndex++],request=tx.objectStore(RESEARCH_SHELF_STORES.records).get(incoming.id);
          request.onerror=()=>abort(mapStorageError(request.error));
          request.onsuccess=()=>{
            try{
              const existing=request.result;
              if(existing&&!sameShelfValue(existing,incoming))fail('conflict','An imported shelf record conflicts with an existing record.',{id:incoming.id,kind:'record'});
              if(!existing)additions.records.push(incoming);
              nextRecord();
            }catch(error){abort(error);}
          };
        };
        const nextCollection=()=>{
          if(collectionIndex>=packet.collections.length)return nextRecord();
          const incoming=packet.collections[collectionIndex++],request=tx.objectStore(RESEARCH_SHELF_STORES.collections).get(incoming.id);
          request.onerror=()=>abort(mapStorageError(request.error));
          request.onsuccess=()=>{
            try{
              const existing=request.result;
              if(existing&&!sameShelfValue(existing,incoming))fail('conflict','An imported collection conflicts with an existing collection.',{id:incoming.id,kind:'collection'});
              if(!existing)additions.collections.push(incoming);
              nextCollection();
            }catch(error){abort(error);}
          };
        };
        const recordsCountRequest=tx.objectStore(RESEARCH_SHELF_STORES.records).count();
        const collectionsCountRequest=tx.objectStore(RESEARCH_SHELF_STORES.collections).count();
        recordsCountRequest.onerror=()=>abort(mapStorageError(recordsCountRequest.error));
        collectionsCountRequest.onerror=()=>abort(mapStorageError(collectionsCountRequest.error));
        let countsReady=0;
        const begin=()=>{if(++countsReady===2)nextCollection();};
        recordsCountRequest.onsuccess=()=>{recordTotal=recordsCountRequest.result;begin();};
        collectionsCountRequest.onsuccess=()=>{collectionTotal=collectionsCountRequest.result;begin();};
      }catch(error){abort(error);}
    };
  });
}

/** An IndexedDB adapter with the same async seam as the memory adapter. */
export function createIndexedDBResearchShelfAdapter({indexedDB=globalThis.indexedDB,dbName=DEFAULT_DB_NAME}={}){
  if(!indexedDB||typeof indexedDB.open!=='function')fail('storage-unavailable','IndexedDB is unavailable.');
  if(typeof dbName!=='string'||!dbName.trim()||dbName.length>256)fail('invalid-input','The shelf database name is invalid.');
  const dbPromise=openDatabase(indexedDB,dbName),status={adapter:'indexeddb',persistent:true,warning:null,closed:false};
  const ready=()=>{if(status.closed)fail('closed','The research shelf is closed.');return dbPromise;};
  return {kind:'indexeddb',persistent:true,warning:null,status:()=>({...status}),ready,
    async getMeta(){return ready().then(db=>transactionPromise(db,[RESEARCH_SHELF_STORES.meta],'readonly',(tx,set,abort)=>{const request=tx.objectStore(RESEARCH_SHELF_STORES.meta).get(META_KEY);request.onerror=()=>abort(mapStorageError(request.error));request.onsuccess=()=>set({generation:metaFrom(request.result).generation});}));},
    async getRecord(id){return ready().then(db=>idbGet(db,RESEARCH_SHELF_STORES.records,id));},
    async listRecords(options){return ready().then(db=>idbListRecords(db,validateListOptions(options)));},
    async saveRecord(record,expected){expectedRevision(expected);return ready().then(db=>idbSaveRecord(db,validateShelfRecord(record),expected));},
    async deleteRecord(id,expected){expectedRevision(expected);if(expected===null||expected===undefined)fail('invalid-input','Deleting a shelf record requires its current revision.');return ready().then(db=>idbDeleteRecord(db,id,expected));},
    async getCollection(id){return ready().then(db=>idbGet(db,RESEARCH_SHELF_STORES.collections,id));},
    async listCollections(){return ready().then(db=>idbListCollections(db));},
    async saveCollection(collection,expected){expectedRevision(expected,'collection');return ready().then(db=>idbSaveCollection(db,validateCollection(collection),expected));},
    async deleteCollection(id,expected){expectedRevision(expected,'collection');if(expected===null||expected===undefined)fail('invalid-input','Deleting a collection requires its current revision.');return ready().then(db=>idbDeleteCollection(db,id,expected));},
    async exportPacket(){return ready().then(db=>idbExport(db));},
    async importPacket(packet,expected){expectedGeneration(expected);const parsed=validateShelfExport(packet);return ready().then(db=>idbImport(db,parsed,expected));},
    async close(){if(status.closed)return;status.closed=true;const db=await dbPromise.catch(()=>null);db?.close();},
  };
}

const requiredAdapterMethods=['getMeta','getRecord','listRecords','saveRecord','deleteRecord','getCollection','listCollections','saveCollection','deleteCollection','exportPacket','importPacket','close'];
function suppliedAdapter(value){
  if(!value||typeof value!=='object'||requiredAdapterMethods.some(key=>typeof value[key]!=='function'))fail('invalid-input','The supplied research shelf adapter is incomplete.');
  return value;
}

/**
 * Store facade. Adapter methods are deliberately small and async so a future
 * sync adapter can be supplied without changing the UI or typed model.
 */
export function createResearchShelfStore(options={}){
  if(!object(options))fail('invalid-input','Research shelf options must be an object.');
  const dbName=options.dbName??DEFAULT_DB_NAME;
  let backendPromise;
  const supplied=options.storage??(typeof options.adapter==='object'?options.adapter:null);
  if(supplied)backendPromise=Promise.resolve(suppliedAdapter(supplied));
  else if(options.adapter==='memory'||options.adapter==='ephemeral'||options.indexedDB===null)
    backendPromise=Promise.resolve(createMemoryResearchShelfAdapter({memoryStore:options.memoryStore,state:options.state,now:options.now,warning:'memory-only'}));
  else if((Object.hasOwn(options,'indexedDB')?options.indexedDB:globalThis.indexedDB)===undefined)
    backendPromise=Promise.resolve(createMemoryResearchShelfAdapter({memoryStore:options.memoryStore,state:options.state,now:options.now,warning:'storage-unavailable'}));
  else{
    const availableIndexedDB=Object.hasOwn(options,'indexedDB')?options.indexedDB:globalThis.indexedDB;
    let candidate;
    try{candidate=createIndexedDBResearchShelfAdapter({indexedDB:availableIndexedDB,dbName});}
    catch(error){candidate=null;backendPromise=Promise.resolve(createMemoryResearchShelfAdapter({memoryStore:options.memoryStore,state:options.state,now:options.now,warning:error?.code||'storage-unavailable'}));}
    if(candidate)backendPromise=candidate.ready().then(()=>candidate).catch(error=>createMemoryResearchShelfAdapter({memoryStore:options.memoryStore,state:options.state,now:options.now,warning:error?.code||'storage-unavailable'}));
  }
  let backend=null,closed=false;
  const status={adapter:'pending',persistent:false,warning:null,dbName,closed:false};
  const ready=async()=>{if(closed)fail('closed','The research shelf is closed.');const value=await backendPromise;if(closed)fail('closed','The research shelf is closed.');if(!backend){backend=value;status.adapter=value.kind??'custom';status.persistent=value.persistent===true;status.warning=value.warning??null;}return value;};
  const call=(method,...args)=>ready().then(value=>value[method](...args));
  const clock=typeof options.now==='function'?options.now:timestamp;
  const api={
    status:()=>({...status,closed}),
    async ready(){await ready();return {...status,closed};},
    async get(id){if(typeof id!=='string'||!id.length||id.length>1024)fail('invalid-input','The shelf record id is invalid.');return call('getRecord',id);},
    async list(options={}){return call('listRecords',validateListOptions(options));},
    async save(input,options={}){
      if(!object(options)||Object.keys(options).some(key=>!['expectedRevision'].includes(key)))fail('invalid-input','Shelf save options contain an unknown field.');
      const expected=expectedRevision(options.expectedRevision),existing=input?.id===undefined?null:await api.get(input.id);
      let value;
      if(existing){
        const chosen=options.expectedRevision===undefined?input?.revision:expected;
        if(chosen===undefined)fail('conflict','Updating a shelf record requires its current revision.',{id:existing.id,actualRecordRevision:existing.revision});
        if(chosen!==null&&chosen!==existing.revision)fail('conflict','The shelf record changed in another context.',{id:existing.id,expectedRevision:chosen,actualRecordRevision:existing.revision});
        value=updateShelfRecord(existing,input,{now:clock});
        return call('saveRecord',value,chosen);
      }
      if(input?.id!==undefined&&options.expectedRevision!==undefined&&expected!==null)fail('conflict','The shelf record was not found for the requested update.',{id:input.id,expectedRevision:expected,actualRecordRevision:null});
      value=createShelfRecord(input,{id:()=>generatedId('shelf'),now:clock});
      return call('saveRecord',value,options.expectedRevision===undefined?undefined:expected);
    },
    async update(id,input,options={}){return api.save({...input,id},{...options,expectedRevision:options.expectedRevision});},
    async remove(id,expected){if(typeof id!=='string'||!id.length||id.length>1024)fail('invalid-input','The shelf record id is invalid.');const chosen=object(expected)?expected.expectedRevision:expected;return call('deleteRecord',id,expectedRevision(chosen));},
    async listCollections(){return call('listCollections');},
    async saveCollection(input,options={}){
      if(!object(options)||Object.keys(options).some(key=>key!=='expectedRevision'))fail('invalid-input','Collection save options contain an unknown field.');
      const expected=expectedRevision(options.expectedRevision,'collection'),existing=input?.id===undefined?null:await call('getCollection',input.id);let value;
      if(existing){const chosen=options.expectedRevision===undefined?input?.revision:expected;if(chosen===undefined)fail('conflict','Updating a collection requires its current revision.',{id:existing.id,actualRecordRevision:existing.revision});if(chosen!==existing.revision)fail('conflict','The collection changed in another context.',{id:existing.id,expectedRevision:chosen,actualRecordRevision:existing.revision});value=updateShelfCollection(existing,input,{now:clock});return call('saveCollection',value,chosen);}
      value=createShelfCollection(input,{id:()=>generatedId('collection'),now:clock});return call('saveCollection',value,options.expectedRevision===undefined?undefined:expected);
    },
    async removeCollection(id,expected){if(typeof id!=='string'||!id.length||id.length>256)fail('invalid-input','The collection id is invalid.');const chosen=object(expected)?expected.expectedRevision:expected;return call('deleteCollection',id,expectedRevision(chosen,'collection'));},
    async export(){return call('exportPacket');},
    async exportPacket(){return api.export();},
    async import(packet,options={}){if(!object(options)||Object.keys(options).some(key=>key!=='expectedGeneration'))fail('invalid-input','Shelf import options contain an unknown field.');const parsed=validateShelfExport(packet);return call('importPacket',parsed,expectedGeneration(options.expectedGeneration));},
    async importPacket(packet,options={}){return api.import(packet,options);},
    async migrate(packet,options={}){
      if(!object(options)||Object.keys(options).some(key=>!['source','lenses','now','expectedGeneration'].includes(key)))
        fail('invalid-input','Supplied packet migration options contain an unknown field.');
      const checked=validateMigrationOptions({source:options.source,lenses:options.lenses,now:options.now}),migrated=importSuppliedResearchPacket(packet,checked);
      const imported=await api.import(migrated.packet,{expectedGeneration:options.expectedGeneration});
      return {...imported,source:migrated.source,retained:migrated.retained,skipped:migrated.skipped};
    },
    async close(){if(closed)return;closed=true;status.closed=true;const value=backend??await backendPromise.catch(()=>null);if(value)await value.close();},
    async destroy(){return api.close();},
  };
  api.delete=api.remove;
  api.getCollection=id=>call('getCollection',id);
  api.deleteCollection=api.removeCollection;
  api.importSupplied=api.migrate;
  api.importSuppliedPacket=api.migrate;
  api.migrateExport=api.migrate;
  return api;
}

export const createMemoryResearchShelfStore=options=>createResearchShelfStore({...options,adapter:'memory'});
export const createResearchShelf=createResearchShelfStore;
export const createMemoryResearchShelf=createMemoryResearchShelfStore;
