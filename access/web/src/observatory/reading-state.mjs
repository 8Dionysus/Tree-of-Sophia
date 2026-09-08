// Reading positions belong to a particular object, source revision and section.
// Text and source payloads are never copied into durable browser storage here.
export function createReadingMemory(body,{limit=48,onCapture=()=>{}}={}){
  const views=new Map();let key=null,restoring=false;
  const identity=(element,index)=>element.dataset.readingKey||element.querySelector('summary')?.textContent?.trim()||String(index);
  function capture(){
    if(!key||restoring||body.getAttribute('aria-busy')==='true'||!body.isConnected||!body.getClientRects().length)return;
    const details=[...body.querySelectorAll('details')].map((el,i)=>[identity(el,i),el.open]);
    const rect=body.getBoundingClientRect(),anchors=[...body.querySelectorAll('h4,article,p,[data-reading-anchor]')];
    const anchor=anchors.find(el=>el.getBoundingClientRect().bottom>rect.top+4);
    views.delete(key);views.set(key,{top:body.scrollTop,details,anchor:anchor?{key:anchor.dataset.readingAnchor||null,text:anchor.textContent.slice(0,180),offset:anchor.getBoundingClientRect().top-rect.top}:null});
    if(views.size>limit)views.delete(views.keys().next().value);
    onCapture();
  }
  function restore(){
    const state=views.get(key);if(!state||!body.getClientRects().length)return;
    restoring=true;const details=new Map(state.details);
    [...body.querySelectorAll('details')].forEach((el,i)=>{if(details.has(identity(el,i)))el.open=details.get(identity(el,i));});
    body.scrollTop=state.top;
    if(state.anchor){const anchor=[...body.querySelectorAll('h4,article,p,[data-reading-anchor]')].find(el=>state.anchor.key?el.dataset.readingAnchor===state.anchor.key:el.textContent.slice(0,180)===state.anchor.text);if(anchor)body.scrollTop+=anchor.getBoundingClientRect().top-body.getBoundingClientRect().top-state.anchor.offset;}
    restoring=false;
  }
  body.addEventListener('scroll',capture,{passive:true});
  return {capture,restore,enter(next){if(next!==key){capture();key=next;}},get key(){return key;},
    // Only explicit local anchors and details IDs can leave page memory.
    exportPositions(){return [...views].map(([id,state])=>[id,{top:state.top,details:state.details.filter(([name])=>['sources','identity'].includes(name)),
      anchor:state.anchor?.key?{key:state.anchor.key,offset:state.anchor.offset}:null}]);},
    importPositions(positions){views.clear();for(const [id,state]of positions.slice(-limit))views.set(id,structuredClone(state));},
  };
}
