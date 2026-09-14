// User-owned reading state. Anchors address one supplied version's local
// paragraph order; they never assert a corpus text-unit or alignment identity.
const SCHEMA='tos_reader_notebook_v1',MAX_BYTES=1_500_000;
const identityFields=['documentId','version','sourceRevision','textSha256'];
const preferenceDefaults={fontSize:18,lineHeight:1.9,width:'comfortable',theme:'night',sidebar:true,inspector:false,mode:'single'};
const fail=code=>{throw new Error(code);};
const record=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const bounded=(value,min,max)=>Number.isFinite(value)&&value>=min&&value<=max;
const string=(value,max)=>typeof value==='string'&&value.trim().length>0&&value.length<=max;
function shape(value,fields){
  if(!record(value)||Object.keys(value).some(field=>!fields.includes(field))||fields.some(field=>!Object.hasOwn(value,field)))fail('invalid-input');
}
function identity(value){
  if(!record(value)||!string(value.documentId,2048)||!string(value.version,256)
    ||!string(value.sourceRevision,4096)||typeof value.textSha256!=='string'||!/^[a-f0-9]{64}$/.test(value.textSha256))fail('invalid-input');
  return Object.fromEntries(identityFields.map(field=>[field,value[field]]));
}
function anchor(value){
  shape(value,[...identityFields,'paragraph']);
  if(!Number.isSafeInteger(value.paragraph)||!bounded(value.paragraph,0,10_000_000))fail('invalid-input');
  return {...identity(value),paragraph:value.paragraph};
}
export function readerVersionKey(value){return JSON.stringify(Object.values(identity(value)));}
export function readerAnchorKey(value){const item=anchor(value);return JSON.stringify([...Object.values(identity(item)),item.paragraph]);}
function active(value){
  if(value===null)return null;
  shape(value,['documentId','version']);
  if(!string(value.documentId,2048)||!string(value.version,256))fail('invalid-input');
  return {...value};
}
function preferences(value){
  shape(value,Object.keys(preferenceDefaults));
  if(!bounded(value.fontSize,15,24)||!bounded(value.lineHeight,1.6,2.2)
    ||!['comfortable','wide'].includes(value.width)||!['night','paper'].includes(value.theme)
    ||!['single','parallel'].includes(value.mode)||typeof value.sidebar!=='boolean'||typeof value.inspector!=='boolean')fail('invalid-input');
  return {...value};
}
function timestamp(value){
  if(typeof value!=='string'||!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/.test(value)
    ||!Number.isFinite(Date.parse(value))||new Date(value).toISOString()!==value)fail('invalid-input');
  return value;
}
function noteText(value){
  if(typeof value!=='string'||!value.trim())fail('invalid-input');
  if(value.length>4000)fail('limit');
  return value;
}
const bytes=text=>new TextEncoder().encode(text).length;
function serialize(value){const text=JSON.stringify(value);if(bytes(text)>MAX_BYTES)fail('limit');return text;}
function validate(value){
  shape(value,['schema','active','preferences','positions','bookmarks','notes']);
  if(value.schema!==SCHEMA)fail('invalid-input');
  for(const [field,limit]of [['positions',64],['bookmarks',256],['notes',200]]){
    if(!Array.isArray(value[field]))fail('invalid-input');
    if(value[field].length>limit)fail('limit');
  }
  const positions=value.positions.map(item=>{
    shape(item,['anchor','fraction']);if(!bounded(item.fraction,0,1))fail('invalid-input');
    return {anchor:anchor(item.anchor),fraction:item.fraction};
  });
  const bookmarks=value.bookmarks.map(item=>{
    shape(item,['anchor','createdAt']);return {anchor:anchor(item.anchor),createdAt:timestamp(item.createdAt)};
  });
  const notes=value.notes.map(item=>{
    shape(item,['anchor','text','updatedAt']);return {anchor:anchor(item.anchor),text:noteText(item.text),updatedAt:timestamp(item.updatedAt)};
  });
  for(const [items,key]of [[positions,readerVersionKey],[bookmarks,readerAnchorKey],[notes,readerAnchorKey]]){
    if(new Set(items.map(item=>key(item.anchor))).size!==items.length)fail('invalid-input');
  }
  const result={schema:SCHEMA,active:active(value.active),preferences:preferences(value.preferences),positions,bookmarks,notes};
  serialize(result);return result;
}
function parse(text){
  if(typeof text!=='string')fail('invalid-input');
  if(bytes(text)>MAX_BYTES)fail('limit');
  let value;try{value=JSON.parse(text);}catch{fail('invalid-input');}
  return validate(value);
}
const empty=()=>({schema:SCHEMA,active:null,preferences:{...preferenceDefaults},positions:[],bookmarks:[],notes:[]});

export function createReaderNotebook({storage,key='tos-reader-notebook-v1',onChange=()=>{}}={}){
  if(!string(key,2048)||typeof onChange!=='function')fail('invalid-input');
  let state=empty(),saved=null,writable=true,error=null;
  try{
    storage=storage===undefined?globalThis.localStorage:storage;
    if(!storage||typeof storage.getItem!=='function'||typeof storage.setItem!=='function')throw Error();
    saved=storage.getItem(key);
    if(saved!==null){
      try{state=parse(saved);}catch{writable=false;error='invalid-storage';}
    }
  }catch{writable=false;error='unavailable';}
  const getState=()=>structuredClone(state),status=()=>({writable,error});
  function persist(){
    if(!writable)return;
    try{
      if(storage.getItem(key)!==saved){writable=false;error='conflict';return;}
      const text=serialize(state);storage.setItem(key,text);saved=text;
    }catch{writable=false;error='unavailable';}
  }
  function commit(next){
    const checked=validate(next);state=checked;persist();onChange(getState(),status());return getState();
  }
  function update(change){const next=getState();change(next);return commit(next);}
  return {
    getState,status,
    setActive(value){const checked=active(value);return update(next=>{next.active=checked;});},
    setPreferences(patch){
      if(!record(patch)||Object.keys(patch).some(field=>!Object.hasOwn(preferenceDefaults,field)))fail('invalid-input');
      const checked=preferences({...state.preferences,...patch});return update(next=>{next.preferences=checked;});
    },
    savePosition(value,fraction=0){
      const checked=anchor(value);if(!bounded(fraction,0,1))fail('invalid-input');
      const key=readerVersionKey(checked);
      return update(next=>{
        next.positions=next.positions.filter(item=>readerVersionKey(item.anchor)!==key);
        next.positions.push({anchor:checked,fraction});next.positions=next.positions.slice(-64);
      });
    },
    positionFor(value){
      const key=readerVersionKey(value),item=state.positions.find(item=>readerVersionKey(item.anchor)===key);
      return item?structuredClone(item):null;
    },
    toggleBookmark(value){
      const checked=anchor(value),key=readerAnchorKey(checked);
      return update(next=>{
        const index=next.bookmarks.findIndex(item=>readerAnchorKey(item.anchor)===key);
        if(index>=0)next.bookmarks.splice(index,1);
        else{if(next.bookmarks.length>=256)fail('limit');next.bookmarks.push({anchor:checked,createdAt:new Date().toISOString()});}
      });
    },
    saveNote(value,text){
      const checked=anchor(value),key=readerAnchorKey(checked),wording=noteText(text);
      return update(next=>{
        const index=next.notes.findIndex(item=>readerAnchorKey(item.anchor)===key);
        const item={anchor:checked,text:wording,updatedAt:new Date().toISOString()};
        if(index>=0)next.notes[index]=item;
        else{if(next.notes.length>=200)fail('limit');next.notes.push(item);}
      });
    },
    deleteNote(value){
      const key=readerAnchorKey(value);return update(next=>{next.notes=next.notes.filter(item=>readerAnchorKey(item.anchor)!==key);});
    },
    exportData(){return serialize(state);},
    importData(text){return commit(parse(text));},
  };
}
