import {validateTarget,validateMaterialTarget} from '../src/research-shelf/model.mjs';
import {draftForPacket} from '../src/observatory/lens-model.mjs';
import {StableExplorationLayout} from './live-model.mjs';
import {validateSkyPose} from './sky-pose.mjs';

export const LIVE_RESUME_SCHEMA='tos.live.resume.v1';
const clone=value=>structuredClone(value);
const fail=()=>{throw new TypeError('The saved research view is incomplete or exceeds its bounds.');};
const keys=(value,allowed)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).every(key=>allowed.includes(key));

// Resume keeps one query, exact selection and geometry. It never persists a
// source response, continuation token, text cache, note, or command receipt.
export function validateLiveResume(value){
  if(!keys(value,['schema','sourceRevision','area','selection','presentation'])||value.schema!==LIVE_RESUME_SCHEMA
    ||!/^[a-f0-9]{64}$/.test(value.sourceRevision||'')||!keys(value.area,['type','target'])
    ||!['route','lens'].includes(value.area.type)||!keys(value.presentation,['layout','pose','mode']))fail();
  const area={type:value.area.type,target:validateTarget(value.area.type,value.area.target)};
  const selection=validateMaterialTarget(value.selection);
  if(selection.sourceRevision!==value.sourceRevision||(area.type==='route'&&area.target.origin.sourceRevision!==value.sourceRevision))fail();
  const mode=value.presentation.mode;if(!['compact','grouped','raw'].includes(mode))fail();
  const layout=new StableExplorationLayout();layout.restore(value.presentation.layout);
  const result={schema:LIVE_RESUME_SCHEMA,sourceRevision:value.sourceRevision,area,selection,
    presentation:{layout:layout.capture(),pose:validateSkyPose(value.presentation.pose),mode}};
  if(new TextEncoder().encode(JSON.stringify(result)).length>128*1024)fail();
  return result;
}

export function makeLiveResume(state,presentation){
  if(!state.view||!state.selection||!presentation.pose)return null;
  const kind=state.selection.kind==='relation'?'relation':'node';
  const id=state.selection.kind==='claim-path'?state.selection.claimId:state.selection.id;
  const raw=state.view[kind==='node'?'nodes':'relations'].find(item=>item.id===id);if(!raw)return null;
  const selection={kind,id,sourceRevision:state.view.source_revision,contentRevision:raw.content_revision};
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
