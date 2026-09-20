import {ExplorationSession,inspectSceneMaterial} from '../src/observatory/exploration-session.mjs';
import {buildExplorationSceneModel,buildSceneModel} from '../src/observatory/scene-model.mjs';
import {RequestSlots,validateLens,RevisionError} from '../src/observatory/knowledge-client.mjs';
import {readExactSource} from '../src/observatory/exact-source-read.mjs';
import {StableExplorationLayout} from './live-model.mjs';
import {validateSkyPose} from './sky-pose.mjs';
import {researchVisibility} from './research-visibility.mjs';

// Search pages are exact, source-revision-bound packets.  Seeking through an
// empty prefix is a convenience for the reader, never permission to drain the
// corpus: the caller gets a cursor and can explicitly continue after a bound.
// These are between-page limits.  A transport may finish one in-flight request
// after a time/byte threshold, so a valid non-empty page is never discarded
// merely because it crossed the window while arriving.
export const SEARCH_SEEK_WINDOW=Object.freeze({maxRequests:8,maxBytes:1024*1024,maxTimeMs:2000});
const positiveWindow=(value,name)=>{
  if(!Number.isSafeInteger(value)||value<1)throw new RangeError(`Invalid ${name} search-seek window.`);
  return value;
};
const jsonBytes=value=>{
  const text=JSON.stringify(value);
  return typeof TextEncoder==='function'?new TextEncoder().encode(text).byteLength:text.length;
};
export async function seekSearchPage(fetchPage,{cursor=null,window=SEARCH_SEEK_WINDOW,isCurrent=()=>true,now=()=>Date.now(),
  hasMatches=page=>(Array.isArray(page.nodes)&&page.nodes.length>0)||(Array.isArray(page.relations)&&page.relations.length>0)}={}){
  if([fetchPage,isCurrent,now,hasMatches].some(callback=>typeof callback!=='function'))throw new TypeError('Invalid search-seek callbacks.');
  const maxRequests=positiveWindow(window.maxRequests,'maxRequests'),maxBytes=positiveWindow(window.maxBytes,'maxBytes'),maxTimeMs=positiveWindow(window.maxTimeMs,'maxTimeMs');
  const limits=Object.freeze({maxRequests,maxBytes,maxTimeMs});
  const started=now();let nextCursor=cursor,requests=0,bytes=0,last=null;
  const result=(page,paused,reason,cancelled=false)=>{const elapsedMs=Math.max(0,now()-started);return {
    page,paused,reason,cancelled,requests,bytes,elapsedMs,limits,
    windowExceeded:{requests:requests>maxRequests,bytes:bytes>maxBytes,time:elapsedMs>maxTimeMs},
  };};
  while(isCurrent()){
    if(requests>=maxRequests)return result(last,true,'requests');
    if(Math.max(0,now()-started)>=maxTimeMs)return result(last,true,'time');
    const page=await fetchPage(nextCursor);
    if(!isCurrent())return result(null,false,'cancelled',true);
    if(page===null||page===undefined)return result(null,false,'cancelled',true);
    requests++;bytes+=jsonBytes(page);last=page;
    const found=hasMatches(page);
    // Preserve a useful match even when its response crossed the soft window;
    // no further page is fetched until the user explicitly continues.
    if(found)return result(page,false,'match');
    if(page.page?.has_more!==true)return result(page,false,'complete');
    if(bytes>=maxBytes)return result(page,true,'bytes');
    if(Math.max(0,now()-started)>=maxTimeMs)return result(page,true,'time');
    if(typeof page.page.next_cursor!=='string'||!page.page.next_cursor)return result(page,true,'contract');
    nextCursor=page.page.next_cursor;
  }
  return result(null,false,'cancelled',true);
}

// Coordinates and open cards belong to the local reader, not ToS. The session
// alone admits exact, bounded backend pages; the sky never changes membership.
export function createLiveResearch({session=new ExplorationSession(),sky,onChange=()=>{},language='ru'}={}){
  const layout=new StableExplorationLayout(),requests=new RequestSlots(),history=[];
  let state={view:null,model:null,areaKind:'exploration',selection:null,historyDepth:0,discovery:null,mode:'compact',language,loading:false,error:null,reading:null,readingError:null,comparison:[]};
  let generation=0,inspection=0,sourceGeneration=0,disposed=false;
  const emit=patch=>{state={...state,...patch};if(!disposed)onChange(state);};
  function present(view,{reset=false,frame=false,areaKind=state.areaKind,selection=areaKind==='exploration'?view.selection:state.selection}={}){
    const scene=(areaKind==='exploration'?buildExplorationSceneModel:buildSceneModel)(view,{mode:state.mode});
    const model=researchVisibility(scene,{catalog:state.discovery?.catalog,selection,mode:state.mode});
    if(reset)layout.reset();
    const projected=layout.project(view,model,{language:state.language,selection,catalog:state.discovery?.catalog});
    sky.update(projected,projected.labels);sky.select(projected.selectedNodeId);sky.selectEdge(projected.selectedEdgeId);
    if(frame)sky.frame();
    emit({view,model,areaKind,selection,historyDepth:history.length});
  }
  function capture(){return state.view?{view:state.view,areaKind:state.areaKind,selection:state.selection,mode:state.mode,
    layout:layout.capture(),pose:sky.capturePose?.()??null,local:state.areaKind==='exploration'?session.captureLocal?.():null,
    comparison:state.comparison}:null;}
  function remember(saved){if(saved){history.push(saved);if(history.length>2)history.shift();}}
  function cancelReadings(slot){session.cancelInspect(slot);if(slot)requests.cancel(slot);else for(const name of ['inspect','compare-left','compare-right'])requests.cancel(name);}
  async function inspect(target,slot='inspect'){
    if(state.areaKind==='exploration')return session.inspect(target,{language:state.language,slot});
    const view=state.view;
    const result=await requests.run(slot,signal=>inspectSceneMaterial(session.client,view,target,{language:state.language,signal}));
    return result.current&&!disposed&&state.view===view?result.value:null;
  }
  function select(target){
    if(!target||!['node','relation','claim-path'].includes(target.kind))throw new TypeError('Unknown exact selection kind.');
    if(state.areaKind==='exploration')present(session.select(target));
    else {
      if(target.kind==='claim-path'){
        const path=state.model.pathsById.get(target.id);if(!path)throw new Error('Unknown exact Claim path.');
        target={kind:'claim-path',id:target.id,claimId:path.claim_node_id};
      }else {
        if(!state.view[target.kind==='relation'?'relations':'nodes'].some(item=>item.id===target.id))throw new Error('Unknown exact material.');
        target={kind:target.kind,id:target.id};
      }
      inspection++;requests.cancel('inspect');requests.cancel('source-record');
      present(state.view,{selection:target});
    }
    void read();
  }
  async function run(work,accept){
    if(disposed)return null;
    const token=++generation;emit({loading:true,error:null});
    try{const result=await work();if(disposed||token!==generation||result===null)return null;accept(result);return result;}
    catch(error){if(!disposed&&token===generation)emit({error});return null;}
    finally{if(!disposed&&token===generation)emit({loading:false});}
  }
  const exact=target=>{
    const kind=target.kind==='claim-path'?'node':target.kind;
    const id=target.kind==='claim-path'?state.model.pathsById.get(target.id)?.claim_node_id:target.id;
    const raw=state.view?.[kind==='relation'?'relations':'nodes'].find(row=>row.id===id);
    if(!raw)throw new Error('The selected material is not in this reading space.');
    return {kind,id,content_revision:raw.content_revision};
  };
  async function read(){
    if(disposed||!state.view)return null;
    const token=++inspection,target=state.selection;emit({reading:null,readingError:null});
    try{const result=await inspect(target);
      if(!disposed&&token===inspection&&result){emit({reading:result});return result;}
    }catch(error){if(!disposed&&token===inspection)emit({error,readingError:error});}return null;
  }
  async function sourceDossier(objectId,{limit}={}){
    if(disposed)return null;
    const token=++sourceGeneration;
    try{
      const result=await session.sourceDossier(objectId,{...(limit===undefined?{}:{limit})});
      return !disposed&&token===sourceGeneration?result:null;
    }catch(error){if(!disposed&&token===sourceGeneration)throw error;return null;}
  }
  return {
    state:()=>state,
    start:()=>run(()=>session.discover(),discovery=>{emit({discovery});if(state.view)present(state.view);}),
    open(target,{replace=false,options={}}={}){
      const first=!state.view,replacing=replace||state.areaKind==='lens',saved=replacing?capture():null;
      return run(()=>session.open(target,{replace:replacing,options}),view=>{
        inspection++;cancelReadings(replacing?undefined:'inspect');remember(saved);
        emit({reading:null,readingError:null,comparison:replace?[]:state.comparison});
        present(view,{areaKind:'exploration',reset:replacing,frame:first||replacing});
      });
    },
    continue:()=>state.areaKind==='exploration'?run(()=>session.continue(),view=>present(view)):Promise.resolve(null),
    showLens(packet){
      if(disposed)return null;
      validateLens(packet,state.discovery?.catalog.source_revision);
      // Validate geometry before taking ownership of the new view.
      buildSceneModel(packet,{mode:state.mode});
      if(!packet.nodes.length)return null;
      const saved=capture();generation++;inspection++;sourceGeneration++;session.cancelScene();cancelReadings();requests.cancel('source-record');
      remember(saved);emit({reading:null,readingError:null,error:null,loading:false,comparison:[]});
      present(packet,{areaKind:'lens',reset:true,frame:true,selection:{kind:'node',id:packet.focus?.node_id??packet.nodes[0].id}});
      return packet;
    },
    back(){
      if(disposed||!history.length)return false;
      const saved=history.at(-1);
      if(saved.areaKind==='exploration'&&!saved.local)throw new Error('The previous reading area is no longer retained.');
      generation++;inspection++;sourceGeneration++;session.cancelScene();cancelReadings();requests.cancel('source-record');
      const view=saved.areaKind==='exploration'?session.restoreLocal(saved.local):saved.view;
      layout.restore(saved.layout);history.pop();
      emit({mode:saved.mode,reading:null,readingError:null,error:null,loading:false,comparison:saved.comparison});
      present(view,{areaKind:saved.areaKind,selection:saved.selection});if(saved.pose)sky.restorePose?.(saved.pose);
      void read();return true;
    },
    capturePresentation:()=>({layout:layout.capture(),pose:sky.capturePose?.()??null,mode:state.mode}),
    restorePresentation(value){
      const probe=new StableExplorationLayout();probe.restore(value.layout);
      if(!['compact','grouped','raw'].includes(value.mode))throw new TypeError('Unknown scene mode.');
      const pose=value.pose?validateSkyPose(value.pose):null;
      layout.restore(value.layout);emit({mode:value.mode});if(state.view)present(state.view);if(pose)sky.restorePose?.(pose);
    },
    selectNode(vertexId){
      if(disposed)return;
      const vertex=state.model?.verticesById.get(vertexId);if(!vertex)return;
      select({kind:'node',id:vertex.representativeId});
    },
    selectEdge(edgeId){
      if(disposed)return;
      const edge=state.model?.edges.find(item=>item.id===edgeId);if(!edge)return;
      select({kind:edge.kind==='claim-path'?'claim-path':'relation',id:edge.kind==='claim-path'?edge.id:edge.rawId});
    },
    selectRaw(target){if(!disposed)select(target);},
    selectedTarget:()=>state.view?exact(state.selection):null,
    read,
    sourceDossier,
    async sourceRecord(snapshot,options){
      if(disposed)return null;
      const token=++sourceGeneration;
      try{
        let result;
        if(state.areaKind==='exploration')result=await session.sourceRecord(snapshot,options);
        else {
          const row=state.view[snapshot.kind==='node'?'nodes':'relations'].find(item=>item.id===snapshot.raw.id);
          if(snapshot.sourceRevision!==state.view.source_revision||row?.content_revision!==snapshot.raw.content_revision)throw new RevisionError();
          const answer=await requests.run('source-record',signal=>readExactSource(session.client,{kind:snapshot.kind,id:row.id,
            source_revision:state.view.source_revision,content_revision:row.content_revision},{...options,signal}));
          result=answer.current?answer.value:null;
        }
        return !disposed&&token===sourceGeneration?result:null;
      }
      catch(error){if(!disposed&&token===sourceGeneration)throw error;return null;}
    },
    cancelSourceRecord(){sourceGeneration++;session.cancelSourceRecord?.();requests.cancel('source-record');},
    cancelSourceDossier(){sourceGeneration++;session.cancelSourceDossier?.();},
    closeReading(){inspection++;cancelReadings('inspect');emit({reading:null,readingError:null});},
    mode(mode){
      if(disposed)return;if(!['compact','grouped','raw'].includes(mode))throw new TypeError('Unknown scene mode.');
      emit({mode});if(state.view)present(state.view);
    },
    language(value){
      if(disposed)return;if(!['ru','en','es'].includes(value))throw new TypeError('Unknown interface language.');
      emit({language:value});if(state.view){present(state.view);if(state.reading)void read();}
    },
    move(id,position){if(!disposed){layout.move(id,position);}},
    async pin(){
      if(disposed||!state.view)return null;
      if(state.comparison.some(item=>item.target.kind===state.selection?.kind&&item.target.id===state.selection?.id))return null;
      if(state.comparison.length>=2){emit({error:new Error('Only two exact reading cards can be compared.')});return null;}
      const target=structuredClone(state.selection),revision=state.view.source_revision;
      const slot=state.comparison.length,index=slot;
      // Reserve a slot before starting I/O; repeated clicks cannot append an
      // unbounded number of cards or race both requests into the same side.
      emit({comparison:[...state.comparison,{target,reading:null}]});
      const current=()=>!disposed&&state.view?.source_revision===revision&&state.comparison[index]?.target===target;
      try{const reading=await inspect(target,slot?'compare-right':'compare-left');
        if(!current())return null;
        if(!reading)throw new Error('The exact comparison material is no longer available in this reading space.');
        const comparison=[...state.comparison];comparison[index]={target,reading};emit({comparison});return reading;
      }catch(error){if(current()){
        const comparison=[...state.comparison];comparison[index]={target,reading:null,error};emit({comparison,error});
      }return null;}
    },
    clearComparison(){cancelReadings('compare-left');cancelReadings('compare-right');emit({comparison:[]});},
    cancel(){generation++;session.cancelScene();emit({loading:false});},
    dispose(){disposed=true;generation++;inspection++;sourceGeneration++;history.length=0;requests.cancelAll();session.dispose();sky.dispose();},
  };
}
