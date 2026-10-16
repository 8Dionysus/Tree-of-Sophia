import {validateTarget,validateMaterialTarget} from '../src/research-shelf/model.mjs';
import {draftForPacket} from '../src/observatory/lens-model.mjs';
import {StableExplorationLayout} from './live-model.mjs';
import {validateSkyPose} from './sky-pose.mjs';
import {claimMaterialReference} from '../src/observatory/knowledge-client.mjs';
import {claimPathFor} from '../src/observatory/human-forms.mjs';

export const LIVE_RESUME_SCHEMA='tos.live.resume.v1';
const clone=value=>structuredClone(value);
const fail=()=>{throw new TypeError('The saved research view is incomplete or exceeds its bounds.');};
const keys=(value,allowed)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(key=>allowed.includes(key));
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
let validateRule,rebindRule;

export function installLiveResumeRules(runtime){
  if(typeof runtime?.validate_live_resume_wasm_v1!=='function'||typeof runtime?.rebind_live_resume_wasm_v1!=='function')
    throw new TypeError('Live resume WASM rules are unavailable.');
  validateRule=runtime.validate_live_resume_wasm_v1;
  rebindRule=runtime.rebind_live_resume_wasm_v1;
}
const requireRules=()=>{
  if(!validateRule||!rebindRule)throw new TypeError('Live resume WASM rules are unavailable.');
};
const ruled=(rule,value)=>{
  const wire=JSON.stringify(value);if(wire.length>1_000_000)fail();
  const bytes=encoder.encode(wire);if(bytes.length>1_000_000)fail();
  return JSON.parse(decoder.decode(rule(bytes)));
};

// Resume keeps one query, exact selection and geometry. It never persists a
// source response, continuation token, text cache, note, or command receipt.
export function validateLiveResume(value){
  requireRules();
  // Cheap host shape/leaf preflight prevents unknown carriers and unbounded
  // envelope strings reaching serialization. Rust owns schema/revision/mode.
  if(!keys(value,['schema','sourceRevision','area','selection','presentation'])
    ||!keys(value.area,['type','target'])||!keys(value.presentation,['layout','pose','mode'])
    ||typeof value.schema!=='string'||value.schema.length!==LIVE_RESUME_SCHEMA.length
    ||typeof value.sourceRevision!=='string'||value.sourceRevision.length!==64
    ||typeof value.presentation.mode!=='string'||value.presentation.mode.length>7)fail();
  const area={type:value.area.type,target:validateTarget(value.area.type,value.area.target)};
  const selection=validateMaterialTarget(value.selection);
  const mode=value.presentation.mode;
  const layout=new StableExplorationLayout();layout.restore(value.presentation.layout);
  const result={schema:value.schema,sourceRevision:value.sourceRevision,area,selection,
    presentation:{layout:layout.capture(),pose:validateSkyPose(value.presentation.pose),mode}};
  const wire=JSON.stringify(result);if(wire.length>128*1024)fail();
  const bytes=encoder.encode(wire);if(bytes.length>128*1024)fail();
  try{validateRule(bytes);return result;}catch{fail();}
}

export function makeLiveResume(state,presentation){
  requireRules();
  if(!state.view||!state.selection||!presentation.pose)return null;
  const kind=state.selection.kind==='relation'?'relation':'node';
  const id=state.selection.kind==='claim-path'?state.selection.claimId:state.selection.id;
  const raw=state.view[kind==='node'?'nodes':'relations'].find(item=>item.id===id);if(!raw)return null;
  const selection={kind,id,sourceRevision:state.view.source_revision,contentRevision:raw.content_revision};
  if(state.selection.kind==='claim-path'){
    const path=claimPathFor(state.view,id);if(!path||path.id!==state.selection.id)return null;
    selection.claimReference=claimMaterialReference(state.view,path);
  }
  let area;
  if(state.areaKind==='lens'){
    const draft=draftForPacket(state.view);if(!draft)return null;
    area={type:'lens',target:{draft}};
  }else{
    const query=state.view.contexts.at(-1)?.query;if(!query)return null;
    const {profile,direction,max_depth,sources,predicate_ids}=query;
    area={type:'route',target:{origin:{kind:query.origin.kind,id:query.origin.id,contentRevision:query.origin.content_revision,
      sourceRevision:state.view.source_revision},options:{profile,direction,max_depth,sources,predicate_ids}}};
  }
  return validateLiveResume({schema:LIVE_RESUME_SCHEMA,sourceRevision:state.view.source_revision,area,selection,presentation});
}

// Requerying an area supplies the path again. Saved selectors cannot recreate
// missing wording or turn a compound Claim reading into a plain node reading.
export function resolveLiveResumeSelection(value,view){
  requireRules();
  const saved=validateMaterialTarget(value);
  if(!view)return null;
  const raw=view[saved.kind==='node'?'nodes':'relations'].find(item=>item.id===saved.id);
  if(!raw)return null;
  let currentClaimReference=null;
  if(saved.claimReference&&view.source_revision===saved.sourceRevision&&raw.content_revision===saved.contentRevision){
    const path=claimPathFor(view,saved.id);
    if(path)currentClaimReference=claimMaterialReference(view,path);
  }
  try{return ruled(rebindRule,{saved,sourceRevision:view.source_revision??null,
    contentRevision:raw.content_revision??null,currentClaimReference});}catch{fail();}
}

export function createLiveResumeStore({indexedDB=globalThis.indexedDB,dbName='tos-real-ui-view-v1',profile='constructor-live'}={}){
  if(!['constructor-live','observatory'].includes(profile))throw new TypeError('Unknown research view profile.');
  let pending=null,db=null,closed=false;
  const open=()=>pending??=(async()=>{
    if(!indexedDB)throw new Error('Persistent view storage is unavailable.');
    db=await new Promise((resolve,reject)=>{
      const request=indexedDB.open(dbName,1);
      request.onupgradeneeded=()=>request.result.createObjectStore('views');
      request.onerror=()=>reject(request.error);request.onblocked=()=>reject(new Error('The research view database is blocked by another tab.'));
      request.onsuccess=()=>resolve(request.result);
    });
    db.onversionchange=()=>{db.close();closed=true;};
    if(closed){db.close();throw new Error('The research view store is closed.');}return db;
  })();
  async function access(mode,work){
    if(closed)throw new Error('The research view store is closed.');
    const connection=await open();
    return new Promise((resolve,reject)=>{
      const tx=connection.transaction('views',mode),request=work(tx.objectStore('views'));
      tx.oncomplete=()=>resolve(request.result);tx.onerror=()=>reject(tx.error);tx.onabort=()=>reject(tx.error??new Error('The view write was cancelled.'));
    });
  }
  return {
    async load(){const value=await access('readonly',store=>store.get(profile));return value===undefined?null:validateLiveResume(value);},
    async save(value){const checked=validateLiveResume(value);await access('readwrite',store=>store.put(clone(checked),profile));return checked;},
    close(){closed=true;db?.close();},
  };
}
