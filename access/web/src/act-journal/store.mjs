// Where the act journal's records live in this browser (tier 0). Records are stored whole and in sequence; the
// folds live in the Rust journal. The bridge (tier 1) offers the same four methods over its own store, so the
// journal changes its store, not its behaviour.

export const ACT_DB='tos-act-journal-v1';
const STORE='acts';

const request=value=>new Promise((resolve,reject)=>{value.onsuccess=()=>resolve(value.result);value.onerror=()=>reject(value.error);});

function openDatabase(indexedDB,name){
  return new Promise((resolve,reject)=>{
    let open;
    try{open=indexedDB.open(name,1);}catch(error){reject(error);return;}
    open.onupgradeneeded=()=>{
      const db=open.result;
      if(!db.objectStoreNames.contains(STORE)){const acts=db.createObjectStore(STORE,{keyPath:'seq'});acts.createIndex('byId','id',{unique:true});}
    };
    open.onerror=()=>reject(open.error);
    open.onblocked=()=>reject(new Error('Another tab is blocking the act journal upgrade.'));
    open.onsuccess=()=>{const db=open.result;db.onversionchange=()=>db.close();resolve(db);};
  });
}

/** Records kept in memory: for tests, for browsers without IndexedDB, and as the fallback when it fails. */
export function createMemoryActStore(entries=[]){
  const rows=[...entries];
  return {
    kind:'memory',persistent:false,
    async lastSeq(){return rows.length?rows[rows.length-1].seq:0;},
    async readFrom(seq,limit=1000){const start=rows.findIndex(row=>row.seq>=seq);return start<0?[]:rows.slice(start,start+limit);},
    async readBefore(seq,limit=100){return rows.filter(row=>row.seq<seq).slice(-limit).reverse();},
    async getById(id){return rows.find(row=>row.id===id)??null;},
    async put(items){
      let expected=(rows.length?rows[rows.length-1].seq:0)+1;
      for(const item of items){if(item.seq!==expected++)throw new Error('act_store_sequence');}
      rows.push(...items.map(item=>({seq:item.seq,id:item.id,record:item.record})));
    },
    close(){},
  };
}

function idbStore(db){
  const tx=(mode,task)=>new Promise((resolve,reject)=>{
    const transaction=db.transaction(STORE,mode),store=transaction.objectStore(STORE);let result;
    transaction.oncomplete=()=>resolve(result);transaction.onerror=()=>reject(transaction.error);transaction.onabort=()=>reject(transaction.error??new Error('act_store_aborted'));
    Promise.resolve(task(store)).then(value=>{result=value;},error=>{try{transaction.abort();}catch{}reject(error);});
  });
  return {
    kind:'indexeddb',persistent:true,
    lastSeq:()=>tx('readonly',async store=>{const cursor=await request(store.openCursor(null,'prev'));return cursor?cursor.key:0;}),
    readFrom:(seq,limit=1000)=>tx('readonly',store=>request(store.getAll(IDBKeyRange.lowerBound(seq),limit))),
    readBefore:(seq,limit=100)=>tx('readonly',store=>new Promise((resolve,reject)=>{
      const out=[],cursor=store.openCursor(IDBKeyRange.upperBound(seq,true),'prev');
      cursor.onsuccess=()=>{const at=cursor.result;if(!at||out.length>=limit){resolve(out);return;}out.push(at.value);at.continue();};
      cursor.onerror=()=>reject(cursor.error);
    })),
    getById:id=>tx('readonly',async store=>(await request(store.index('byId').get(id)))??null),
    put:items=>tx('readwrite',store=>{for(const item of items)store.add({seq:item.seq,id:item.id,record:item.record});}),
    close(){db.close();},
  };
}

/** The browser's act store, or memory with a warning when IndexedDB is unavailable. */
export async function openActStore({indexedDB=globalThis.indexedDB,name=ACT_DB}={}){
  if(!indexedDB||typeof indexedDB.open!=='function')return Object.assign(createMemoryActStore(),{warning:'storage-unavailable'});
  try{return idbStore(await openDatabase(indexedDB,name));}
  catch(error){return Object.assign(createMemoryActStore(),{warning:error?.message??'storage-unavailable'});}
}
