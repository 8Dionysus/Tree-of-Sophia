import {ExplorationSession} from '../src/observatory/exploration-session.mjs';
import {buildExplorationSceneModel} from '../src/observatory/scene-model.mjs';
import {StableExplorationLayout} from './live-model.mjs';

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
export async function seekSearchPage(fetchPage,{cursor=null,window=SEARCH_SEEK_WINDOW,isCurrent=()=>true,now=()=>Date.now()}={}){
  if(typeof fetchPage!=='function'||typeof isCurrent!=='function'||typeof now!=='function')throw new TypeError('Invalid search-seek callbacks.');
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
    const found=(Array.isArray(page.nodes)&&page.nodes.length>0)||(Array.isArray(page.relations)&&page.relations.length>0);
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
  const layout=new StableExplorationLayout();
  let state={view:null,model:null,discovery:null,mode:'compact',language,loading:false,error:null,reading:null,readingError:null,comparison:[]};
  let generation=0,inspection=0,sourceGeneration=0,disposed=false;
  const emit=patch=>{state={...state,...patch};if(!disposed)onChange(state);};
  function present(view,{reset=false,frame=false}={}){
    const model=buildExplorationSceneModel(view,{mode:state.mode});
    if(reset)layout.reset();
    const projected=layout.project(view,model,{language:state.language});
    sky.update(projected,projected.labels);sky.select(projected.selectedNodeId);sky.selectEdge(projected.selectedEdgeId);
    if(frame)sky.frame();
    emit({view,model});
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
    const token=++inspection,target=state.view.selection;emit({reading:null,readingError:null});
    try{const result=await session.inspect(target,{language:state.language});
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
    start:()=>run(()=>session.discover(),discovery=>emit({discovery})),
    open(target,{replace=false,options={}}={}){
      const first=!state.view;
      return run(()=>session.open(target,{replace,options}),view=>{
        inspection++;session.cancelInspect(replace?undefined:'inspect');
        emit({reading:null,readingError:null,comparison:replace?[]:state.comparison});
        present(view,{reset:replace,frame:first||replace});
      });
    },
    continue:()=>run(()=>session.continue(),view=>present(view)),
    selectNode(vertexId){
      if(disposed)return;
      const vertex=state.model?.verticesById.get(vertexId);if(!vertex)return;
      present(session.select({kind:'node',id:vertex.representativeId}));void read();
    },
    selectEdge(edgeId){
      if(disposed)return;
      const edge=state.model?.edges.find(item=>item.id===edgeId);if(!edge)return;
      present(session.select({kind:edge.kind==='claim-path'?'claim-path':'relation',id:edge.kind==='claim-path'?edge.id:edge.rawId}));void read();
    },
    selectRaw(target){if(!disposed){present(session.select(target));void read();}},
    selectedTarget:()=>state.view?exact(state.view.selection):null,
    read,
    sourceDossier,
    async sourceRecord(snapshot){
      if(disposed)return null;
      const token=++sourceGeneration;
      try{const result=await session.sourceRecord(snapshot);return !disposed&&token===sourceGeneration?result:null;}
      catch(error){if(!disposed&&token===sourceGeneration)throw error;return null;}
    },
    cancelSourceRecord(){sourceGeneration++;session.cancelSourceRecord?.();},
    cancelSourceDossier(){sourceGeneration++;session.cancelSourceDossier?.();},
    closeReading(){inspection++;session.cancelInspect('inspect');emit({reading:null,readingError:null});},
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
      if(state.comparison.length>=2){emit({error:new Error('Only two exact reading cards can be compared.')});return null;}
      const target=structuredClone(state.view.selection),revision=state.view.source_revision;
      const slot=state.comparison.length,index=slot;
      // Reserve a slot before starting I/O; repeated clicks cannot append an
      // unbounded number of cards or race both requests into the same side.
      emit({comparison:[...state.comparison,{target,reading:null}]});
      const current=()=>!disposed&&state.view?.source_revision===revision&&state.comparison[index]?.target===target;
      try{const reading=await session.inspect(target,{language:state.language,slot:slot?'compare-right':'compare-left'});
        if(!current())return null;
        if(!reading)throw new Error('The exact comparison material is no longer available in this reading space.');
        const comparison=[...state.comparison];comparison[index]={target,reading};emit({comparison});return reading;
      }catch(error){if(current()){
        const comparison=[...state.comparison];comparison[index]={target,reading:null,error};emit({comparison,error});
      }return null;}
    },
    clearComparison(){session.cancelInspect('compare-left');session.cancelInspect('compare-right');emit({comparison:[]});},
    cancel(){generation++;session.cancelScene();emit({loading:false});},
    dispose(){disposed=true;generation++;inspection++;sourceGeneration++;session.dispose();sky.dispose();},
  };
}
