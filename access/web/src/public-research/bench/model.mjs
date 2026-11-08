// The workbench as data. A bench gathers the work on one question: the windows the reader keeps open, how they lie
// side by side, and the tray of what has been collected. A window is a view bound to a subject; the subject is an
// address (a star, a link, a chapter, a fragment, a word, a thought), never a copy of what the source says.
// Everything here is pure: an operation takes a state and returns the next one, so the layout can be tested,
// undone and saved without a page.

export const BENCH_SCHEMA='tos.public_research.bench.v1';
export const MAX_DEPTH=2,MAX_BENCHES=12,MAX_WINDOWS=24,MAX_TRAY=200,MAX_FRAGMENTS=2000,MAX_THOUGHTS=500,MAX_NOTES=200;
export const VIEWS=Object.freeze(['card','text','word','fragment','thought']);
// Each view belongs to one family, the step of interpretation it works on.
export const FAMILY=Object.freeze({card:'reading',text:'source',fragment:'source',word:'word',thought:'mine'});
export const NOTE_KINDS=Object.freeze(['observation','question','reading']);
// The verbs by which one place of the book answers another, as the bonsai's place relations name them.
export const PLACE_VERBS=Object.freeze(['repeat','variation','answer','contrast','echo']);
const SUBJECT_KINDS=new Set(['node','edge','chapter','fragment','word','thought']);
const EDGES=new Set(['left','right','top','bottom','center']);

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const text=(value,limit)=>typeof value==='string'?value.slice(0,limit):'';
const clone=value=>structuredClone(value);
let counter=0;
export const makeId=prefix=>`${prefix}-${globalThis.crypto?.randomUUID?.().slice(0,13)??`${Date.now().toString(36)}${(counter++).toString(36)}`}`;

/** The view that shows a subject. */
export function viewOf(subject){
  return {node:'card',edge:'card',chapter:'text',fragment:'fragment',word:'word',thought:'thought'}[subject?.kind]??null;
}
/** One key per subject: the same star or fragment is the same subject wherever it was taken from. */
export function subjectKey(subject){
  if(!object(subject))return '';
  if(subject.kind==='chapter')return `chapter:${subject.chapter}:${subject.language??''}`;
  if(subject.kind==='word')return `word:${subject.language??''}:${String(subject.form??'').toLocaleLowerCase()}`;
  return `${subject.kind}:${subject.id}`;
}
/** A subject as it may be kept: only its known fields, each bounded. */
export function cleanSubject(value){
  if(!object(value)||!SUBJECT_KINDS.has(value.kind))return null;
  if(value.kind==='chapter'){
    if(!text(value.chapter,16))return null;
    return {kind:'chapter',chapter:text(value.chapter,16),language:text(value.language,8)||'de',...(text(value.unit,200)?{unit:text(value.unit,200)}:{})};
  }
  if(value.kind==='word'){
    const form=text(value.form,80).trim();if(!form)return null;
    return {kind:'word',form,language:text(value.language,8)||'de',...(text(value.chapter,16)?{chapter:text(value.chapter,16)}:{}),...(text(value.unit,200)?{unit:text(value.unit,200)}:{}),
      ...(Number.isSafeInteger(value.at)&&value.at>=0?{at:value.at}:{})};
  }
  const id=text(value.id,400);if(!id)return null;
  return {kind:value.kind,id};
}

export function emptyBench(title=''){
  return {id:makeId('bench'),title:text(title,160),breathing:true,layout:null,windows:{},activeWindow:null,focus:null,tray:[]};
}
export function emptyState(){
  const bench=emptyBench();
  return {schema:BENCH_SCHEMA,revision:0,active:bench.id,benches:[bench],fragments:{},thoughts:{},viewDefaults:{}};
}

// ------------------------------------------------------------ the layout tree
// A layout is an area (tabs of windows) or a split (a row or a column of layouts with their shares).

export function isArea(node){return object(node)&&typeof node.area==='string';}
export function areas(node,out=[]){if(!node)return out;if(isArea(node))out.push(node);else for(const child of node.children)areas(child,out);return out;}
/** The path of child indexes from the root to an area, and its parent chain. */
export function findArea(root,areaId){
  const walk=(node,path)=>{if(!node)return null;if(isArea(node))return node.area===areaId?path:null;for(const [i,child] of node.children.entries()){const found=walk(child,[...path,i]);if(found)return found;}return null;};
  return walk(root,[]);
}
export function nodeAt(root,path){let node=root;for(const i of path)node=node?.children?.[i];return node??null;}
export function areaOfWindow(bench,windowId){return areas(bench.layout).find(area=>area.tabs.includes(windowId))??null;}

function evenSizes(n){return Array.from({length:n},()=>1/n);}
function normalSizes(sizes,n){
  const list=Array.isArray(sizes)&&sizes.length===n?sizes.map(value=>Number.isFinite(value)&&value>0?value:0):evenSizes(n);
  const sum=list.reduce((a,b)=>a+b,0);if(!(sum>0))return evenSizes(n);
  return list.map(value=>Math.max(0.04,value/sum));
}
/** A layout made sound: empty areas leave, a split of one becomes its child, a split inside a split of its own way joins it. */
export function normalize(node){
  if(!node)return null;
  if(isArea(node)){
    if(!node.tabs.length)return null;
    if(!node.tabs.includes(node.active))node.active=node.tabs[0];
    return node;
  }
  const kept=[],sizes=[],given=normalSizes(node.sizes,node.children.length);
  node.children.forEach((child,i)=>{
    const sound=normalize(child);if(!sound)return;
    if(!isArea(sound)&&sound.split===node.split){const inner=normalSizes(sound.sizes,sound.children.length);sound.children.forEach((grand,k)=>{kept.push(grand);sizes.push(given[i]*inner[k]);});return;}
    kept.push(sound);sizes.push(given[i]);
  });
  if(!kept.length)return null;
  if(kept.length===1)return kept[0];
  node.children=kept;node.sizes=normalSizes(sizes,kept.length);
  return node;
}
function depthOf(root,path){return path.length;}

// ------------------------------------------------------------ benches

export function activeBench(state){return state.benches.find(bench=>bench.id===state.active)??state.benches[0];}
function withBench(state,benchId,change){
  const next=clone(state),bench=next.benches.find(item=>item.id===(benchId??next.active));if(!bench)return state;
  change(bench,next);bench.layout=normalize(bench.layout);
  for(const id of Object.keys(bench.windows))if(!areaOfWindow(bench,id))delete bench.windows[id];
  if(bench.activeWindow&&!bench.windows[bench.activeWindow])bench.activeWindow=areas(bench.layout)[0]?.active??null;
  if(bench.focus&&!bench.windows[bench.focus])bench.focus=null;
  return next;
}
export function addBench(state,{title=''}={}){
  if(state.benches.length>=MAX_BENCHES)return state;
  const next=clone(state),bench=emptyBench(title);next.benches.push(bench);next.active=bench.id;return next;
}
export function removeBench(state,benchId){
  const next=clone(state);next.benches=next.benches.filter(bench=>bench.id!==benchId);
  if(!next.benches.length)next.benches.push(emptyBench());
  if(!next.benches.some(bench=>bench.id===next.active))next.active=next.benches[0].id;
  return next;
}
export function renameBench(state,benchId,title){return withBench(state,benchId,bench=>{bench.title=text(title,160);});}
export function activateBench(state,benchId){if(!state.benches.some(bench=>bench.id===benchId))return state;const next=clone(state);next.active=benchId;return next;}
export function setBreathing(state,benchId,on){return withBench(state,benchId,bench=>{bench.breathing=!!on;});}

// ------------------------------------------------------------ windows

function familyOfArea(bench,area){const view=bench.windows[area.active]?.view;return view?FAMILY[view]:null;}
function hostArea(bench){return areaOfWindow(bench,bench.activeWindow)??areas(bench.layout)[0]??null;}

/** Lays a new area beside an existing one; where the tree is already as deep as it may go, the window joins as a tab. */
function placeBeside(bench,targetId,area,edge){
  const path=findArea(bench.layout,targetId);if(!path)return false;
  const dir=edge==='left'||edge==='right'?'row':'column',after=edge==='right'||edge==='bottom';
  const parentPath=path.slice(0,-1),parent=parentPath.length||path.length?nodeAt(bench.layout,parentPath):null,index=path.at(-1);
  if(path.length&&parent&&!isArea(parent)&&parent.split===dir){
    const share=parent.sizes[index]/2;parent.sizes.splice(index,1,share);parent.sizes.splice(after?index+1:index,0,share);
    parent.children.splice(after?index+1:index,0,area);return true;
  }
  if(depthOf(bench.layout,path)>=MAX_DEPTH){
    // Too deep to split here: the grandparent of the way asked for takes the new area beside the branch.
    const up=path.slice(0,-1),grand=up.length?nodeAt(bench.layout,up.slice(0,-1)):null,at=up.at(-1);
    if(grand&&!isArea(grand)&&grand.split===dir){const share=grand.sizes[at]/2;grand.sizes.splice(at,1,share);grand.sizes.splice(after?at+1:at,0,share);grand.children.splice(after?at+1:at,0,area);return true;}
    return false;
  }
  const target=nodeAt(bench.layout,path),split={split:dir,sizes:[0.5,0.5],pinned:false,children:after?[target,area]:[area,target]};
  if(!path.length)bench.layout=split;else parent.children[index]=split;
  return true;
}
function newArea(windowId){return {area:makeId('area'),tabs:[windowId],active:windowId,collapsed:false};}
function addAsTab(area,windowId){if(!area.tabs.includes(windowId))area.tabs.push(windowId);area.active=windowId;area.collapsed=false;}

/**
 * Opens a subject on a bench. The same subject already open is brought forward. Otherwise a window joins the area
 * of its own family, or a new area beside the work in hand: the word beside the text it was read in, the reader's
 * own thought under the rest; with no room left it becomes a tab.
 */
export function openSubject(state,subject,{place='auto',benchId=null,binding='pinned',settings={}}={}){
  const clean=cleanSubject(subject),view=viewOf(clean);if(!clean||!view)return {state,windowId:null};
  let windowId=null;
  const next=withBench(state,benchId,bench=>{
    const key=subjectKey(clean),existing=Object.entries(bench.windows).find(([,win])=>subjectKey(win.subject)===key)?.[0];
    if(existing&&place==='auto'){const area=areaOfWindow(bench,existing);area.active=existing;area.collapsed=false;bench.activeWindow=existing;windowId=existing;return;}
    if(Object.keys(bench.windows).length>=MAX_WINDOWS)return;
    windowId=makeId('win');bench.windows[windowId]={view,subject:clean,binding:binding==='follow'?'follow':'pinned',settings:object(settings)?clone(settings):{}};
    bench.activeWindow=windowId;bench.focus=null;
    if(!bench.layout){bench.layout=newArea(windowId);return;}
    if(object(place)&&place.area){
      const target=areas(bench.layout).find(area=>area.area===place.area);
      if(target&&(place.edge==='center'||!EDGES.has(place.edge)))return addAsTab(target,windowId);
      if(target&&placeBeside(bench,target.area,newArea(windowId),place.edge))return;
      if(target)return addAsTab(target,windowId);
    }
    const all=areas(bench.layout),host=hostArea(bench),family=FAMILY[view];
    if(place==='tab')return addAsTab(host,windowId);
    // A window joins an area showing the same view; a fragment otherwise lies beside the text it was taken from.
    const sameView=all.find(area=>bench.windows[area.active]?.view===view&&area.active!==windowId);
    if(sameView)return addAsTab(sameView,windowId);
    const same=all.find(area=>familyOfArea(bench,area)===family&&area.active!==windowId);
    if(same&&family!=='source')return addAsTab(same,windowId);
    if(same&&view==='fragment'){
      if(all.filter(area=>!area.collapsed).length<4&&placeBeside(bench,same.area,newArea(windowId),'right'))return;
      return addAsTab(same,windowId);
    }
    if(family==='mine'){
      // The reader's own thought lies under the work it grows from.
      if(isArea(bench.layout)||bench.layout.split!=='column'){bench.layout={split:'column',sizes:[0.7,0.3],pinned:false,children:[bench.layout,newArea(windowId)]};return;}
      if(placeBeside(bench,areas(bench.layout.children.at(-1)).at(-1).area,newArea(windowId),'right'))return;
      return addAsTab(host,windowId);
    }
    if(all.filter(area=>!area.collapsed).length>=4)return addAsTab(host,windowId);
    const beside=family==='word'?(all.find(area=>familyOfArea(bench,area)==='source')??host):host;
    if(placeBeside(bench,beside.area,newArea(windowId),'right'))return;
    addAsTab(host,windowId);
  });
  return {state:windowId?next:state,windowId};
}
export function closeWindow(state,windowId,benchId=null){
  return withBench(state,benchId,bench=>{
    const area=areaOfWindow(bench,windowId);if(!area)return;
    const at=area.tabs.indexOf(windowId);area.tabs.splice(at,1);if(area.active===windowId)area.active=area.tabs[Math.max(0,at-1)]??null;
    delete bench.windows[windowId];if(bench.activeWindow===windowId)bench.activeWindow=area.active??null;
  });
}
/** Moves a window into another area (as a tab) or beside it (to one of its edges). */
export function moveWindow(state,windowId,{area:targetId,edge='center'}={},benchId=null){
  return withBench(state,benchId,bench=>{
    const source=areaOfWindow(bench,windowId),target=areas(bench.layout).find(area=>area.area===targetId);
    if(!source||!target)return;
    if(source===target&&(edge==='center'||source.tabs.length===1))return;
    source.tabs.splice(source.tabs.indexOf(windowId),1);if(source.active===windowId)source.active=source.tabs[0]??null;
    bench.layout=normalize(bench.layout);
    if(edge==='center'||!EDGES.has(edge)){addAsTab(target,windowId);bench.activeWindow=windowId;return;}
    if(!placeBeside(bench,target.area,newArea(windowId),edge))addAsTab(target,windowId);
    bench.activeWindow=windowId;
  });
}
export function activateWindow(state,windowId,benchId=null){
  const bench=state.benches.find(item=>item.id===(benchId??state.active)),area=bench&&areaOfWindow(bench,windowId);
  if(!area||(area.active===windowId&&bench.activeWindow===windowId&&!area.collapsed))return state;
  return withBench(state,benchId,bench=>{const area=areaOfWindow(bench,windowId);area.active=windowId;area.collapsed=false;bench.activeWindow=windowId;});
}
export function setWindow(state,windowId,{subject,binding,settings}={},benchId=null){
  return withBench(state,benchId,bench=>{
    const win=bench.windows[windowId];if(!win)return;
    if(subject!==undefined){const clean=cleanSubject(subject);if(clean&&viewOf(clean)===win.view)win.subject=clean;}
    if(binding!==undefined)win.binding=binding==='follow'?'follow':'pinned';
    if(object(settings))win.settings={...win.settings,...clone(settings)};
  });
}
/** Shares of a split, taken by the hand: the split stays where the reader put it until it is given back to breathing. */
export function setSizes(state,path,sizes,{pinned=true,benchId=null}={}){
  return withBench(state,benchId,bench=>{const split=nodeAt(bench.layout,path);if(!split||isArea(split)||sizes.length!==split.children.length)return;split.sizes=normalSizes(sizes,sizes.length);split.pinned=!!pinned;});
}
export function releaseSizes(state,path,benchId=null){return withBench(state,benchId,bench=>{const split=nodeAt(bench.layout,path);if(split&&!isArea(split))split.pinned=false;});}
export function setCollapsed(state,areaId,collapsed,benchId=null){
  return withBench(state,benchId,bench=>{const area=areas(bench.layout).find(item=>item.area===areaId);if(area)area.collapsed=!!collapsed;});
}
export function setFocus(state,windowId,benchId=null){
  return withBench(state,benchId,bench=>{bench.focus=windowId&&bench.windows[windowId]?windowId:null;if(bench.focus)bench.activeWindow=windowId;});
}

// ------------------------------------------------------------ the tray

export function trayAdd(state,subject,{title='',benchId=null}={}){
  const clean=cleanSubject(subject);if(!clean)return state;
  return withBench(state,benchId,bench=>{
    const key=subjectKey(clean);if(bench.tray.some(item=>subjectKey(item.subject)===key))return;
    if(bench.tray.length>=MAX_TRAY)bench.tray.shift();
    bench.tray.push({subject:clean,title:text(title,200),at:new Date().toISOString()});
  });
}
export function trayRemove(state,key,benchId=null){return withBench(state,benchId,bench=>{bench.tray=bench.tray.filter(item=>subjectKey(item.subject)!==key);});}
export function inTray(state,subject,benchId=null){const bench=state.benches.find(item=>item.id===(benchId??state.active));const key=subjectKey(subject);return !!bench?.tray.some(item=>subjectKey(item.subject)===key);}

// ------------------------------------------------------------ fragments and thoughts

const cleanCite=value=>{
  if(!object(value)||!text(value.c,16)||!Array.isArray(value.u)||!value.u.length||value.u.some(unit=>!text(unit,200)))return null;
  const cite={w:text(value.w,120)||'also-sprach-zarathustra',c:text(value.c,16),u:value.u.slice(0,2).map(unit=>text(unit,200))};
  if(Number.isInteger(value.s)&&value.s>=0)cite.s=value.s;if(Number.isInteger(value.e)&&value.e>=0)cite.e=value.e;
  return cite;
};
function cleanNote(value){
  if(!object(value)||!NOTE_KINDS.includes(value.kind))return null;
  const body=text(value.body,4000).trim();if(!body)return null;
  return {id:text(value.id,80)||makeId('note'),kind:value.kind,body,at:text(value.at,40)||new Date().toISOString()};
}
function cleanLink(value){
  if(!object(value))return null;
  const target=cleanSubject(value.target);if(!target||!['node','edge','fragment'].includes(target.kind))return null;
  const verb=text(value.verb,120).trim();if(!verb)return null;
  return {id:text(value.id,80)||makeId('link'),verb,target,title:text(value.title,200),at:text(value.at,40)||new Date().toISOString()};
}
/**
 * A fragment: a place the reader marked in a witness. The pivot citation (`cite`) is the place in the original;
 * `witness` keeps the selection as it was made when it was made in a translation. The label is the reader's handle
 * for it; the words themselves stay with the source.
 */
export function cleanFragment(value){
  if(!object(value))return null;
  const cite=cleanCite(value.cite);if(!cite)return null;
  const witness=object(value.witness)&&text(value.witness.language,8)?{language:text(value.witness.language,8),...(cleanCite({...value.witness,c:cite.c})?{u:cleanCite({...value.witness,c:cite.c}).u,s:value.witness.s,e:value.witness.e}:{})}:{language:'de'};
  return {id:text(value.id,80)||makeId('frag'),cite,witness,label:text(value.label,160),fingerprint:text(value.fingerprint,80),
    notes:(Array.isArray(value.notes)?value.notes:[]).map(cleanNote).filter(Boolean).slice(0,MAX_NOTES),
    links:(Array.isArray(value.links)?value.links:[]).map(cleanLink).filter(Boolean).slice(0,MAX_NOTES),
    saved:text(value.saved,120)||null,at:text(value.at,40)||new Date().toISOString()};
}
export function putFragment(state,fragment){
  const clean=cleanFragment(fragment);if(!clean)return {state,id:null};
  const next=clone(state),same=Object.values(next.fragments).find(item=>item.cite.c===clean.cite.c&&item.cite.u.join()===clean.cite.u.join()&&item.cite.s===clean.cite.s&&item.cite.e===clean.cite.e&&item.witness.language===clean.witness.language);
  if(same)return {state,id:same.id};
  if(Object.keys(next.fragments).length>=MAX_FRAGMENTS)return {state,id:null};
  next.fragments[clean.id]=clean;return {state:next,id:clean.id};
}
export function updateFragment(state,id,change){
  const current=state.fragments[id];if(!current)return state;
  const clean=cleanFragment({...clone(current),...change(clone(current)),id});if(!clean)return state;
  const next=clone(state);next.fragments[id]=clean;return next;
}
export function cleanThought(value){
  if(!object(value))return null;
  return {id:text(value.id,80)||makeId('thought'),question:text(value.question,4000),thesis:text(value.thesis,4000),
    grounds:[...new Set((Array.isArray(value.grounds)?value.grounds:[]).map(item=>text(item,80)).filter(Boolean))].slice(0,40),
    saved:text(value.saved,120)||null,at:text(value.at,40)||new Date().toISOString()};
}
export function putThought(state,thought){
  const clean=cleanThought(thought);if(!clean)return {state,id:null};
  if(Object.keys(state.thoughts).length>=MAX_THOUGHTS&&!state.thoughts[clean.id])return {state,id:null};
  const next=clone(state);next.thoughts[clean.id]=clean;return {state:next,id:clean.id};
}
export function updateThought(state,id,change){
  const current=state.thoughts[id];if(!current)return state;
  return putThought(state,{...clone(current),...change(clone(current)),id}).state;
}

// ------------------------------------------------------------ prepared layouts

/**
 * The prepared layouts lay the bench out anew around what it holds: the text first, then a card, the word and the
 * reader's own thought. Windows already open keep their subjects.
 */
export function applyPreset(state,name,{chapter='I.01',language='de',benchId=null}={}){
  const bench=state.benches.find(item=>item.id===(benchId??state.active));if(!bench)return state;
  const wins=Object.entries(bench.windows),first=view=>wins.find(([,win])=>win.view===view)?.[0]??null;
  const textWin=first('text'),card=first('card'),word=first('word'),thought=first('thought'),fragment=first('fragment');
  const fresh=(view,subject)=>{const id=makeId('win');return [id,{view,subject,binding:view==='word'?'follow':'pinned',settings:{}}];};
  const made=new Map(wins);
  const ensure=(id,view,subject)=>{if(id)return id;const [nid,win]=fresh(view,subject);made.set(nid,win);return nid;};
  const rest=ids=>wins.map(([id])=>id).filter(id=>!ids.includes(id));
  let layout;
  if(name==='reading'){
    const t=ensure(textWin,'text',{kind:'chapter',chapter,language}),w=ensure(word,'word',{kind:'word',form:'',language});
    layout={split:'row',sizes:[0.68,0.32],pinned:false,children:[{area:makeId('area'),tabs:[t,...rest([t,w])],active:t,collapsed:false},{area:makeId('area'),tabs:[w],active:w,collapsed:false}]};
  }else if(name==='parallel'){
    const t=ensure(textWin,'text',{kind:'chapter',chapter,language}),w=ensure(word,'word',{kind:'word',form:'',language});
    made.set(t,{...made.get(t),settings:{...made.get(t).settings,parallel:true}});
    layout={split:'row',sizes:[0.74,0.26],pinned:false,children:[{area:makeId('area'),tabs:[t,...rest([t,w])],active:t,collapsed:false},{area:makeId('area'),tabs:[w],active:w,collapsed:false}]};
  }else if(name==='analysis'){
    const t=ensure(textWin,'text',{kind:'chapter',chapter,language}),w=ensure(word,'word',{kind:'word',form:'',language}),m=ensure(thought,'thought',{kind:'thought',id:''});
    const middle=card??fragment;
    const top=[{area:makeId('area'),tabs:[t],active:t,collapsed:false}];
    if(middle)top.push({area:makeId('area'),tabs:[middle,...rest([t,w,m,middle])],active:middle,collapsed:false});
    top.push({area:makeId('area'),tabs:[w],active:w,collapsed:false});
    const row={split:'row',sizes:middle?[0.42,0.36,0.22]:[0.7,0.3],pinned:false,children:top};
    if(!middle&&rest([t,w,m]).length)top[0].tabs.push(...rest([t,w,m]));
    layout={split:'column',sizes:[0.72,0.28],pinned:false,children:[row,{area:makeId('area'),tabs:[m],active:m,collapsed:false}]};
  }else if(name==='compare'){
    const cards=wins.filter(([,win])=>win.view==='card'||win.view==='fragment').map(([id])=>id);
    if(cards.length<2)return state;
    layout={split:'row',sizes:[0.5,0.5],pinned:false,children:[{area:makeId('area'),tabs:[cards[0]],active:cards[0],collapsed:false},{area:makeId('area'),tabs:[cards[1],...rest(cards.slice(0,2))],active:cards[1],collapsed:false}]};
  }else return state;
  const next=clone(state),target=next.benches.find(item=>item.id===bench.id);
  target.windows=Object.fromEntries([...made].filter(([id])=>areas(layout).some(area=>area.tabs.includes(id))));
  // A thought window made by the layout opens on a fresh thought.
  for(const [id,win]of Object.entries(target.windows))if(win.view==='thought'&&!win.subject.id){const t={id:makeId('thought'),question:target.title,thesis:'',grounds:[],saved:null,at:new Date().toISOString()};next.thoughts[t.id]=t;target.windows[id]={...win,subject:{kind:'thought',id:t.id}};}
  target.layout=normalize(layout);target.focus=null;target.activeWindow=areas(target.layout)[0]?.active??null;
  return next;
}

// ------------------------------------------------------------ reading a stored state

/** A stored state made sound: unknown fields leave, broken windows and areas leave, the rest stays as it was. */
export function validateState(value){
  if(!object(value)||value.schema!==BENCH_SCHEMA||!Array.isArray(value.benches))throw new TypeError('The workbench state is not a tos.public_research.bench.v1 packet.');
  const fragments={},thoughts={};
  for(const item of Object.values(object(value.fragments)?value.fragments:{}).slice(0,MAX_FRAGMENTS)){const clean=cleanFragment(item);if(clean)fragments[clean.id]=clean;}
  for(const item of Object.values(object(value.thoughts)?value.thoughts:{}).slice(0,MAX_THOUGHTS)){const clean=cleanThought(item);if(clean)thoughts[clean.id]=clean;}
  const benches=[];
  for(const raw of value.benches.slice(0,MAX_BENCHES)){
    if(!object(raw)||!text(raw.id,80))continue;
    const windows={};
    for(const [id,win]of Object.entries(object(raw.windows)?raw.windows:{}).slice(0,MAX_WINDOWS)){
      const subject=cleanSubject(win?.subject);if(!subject||viewOf(subject)!==win.view)continue;
      windows[text(id,80)]={view:win.view,subject,binding:win.binding==='follow'?'follow':'pinned',settings:object(win.settings)?clone(win.settings):{}};
    }
    const seen=new Set();
    const read=(node,depth)=>{
      if(!object(node)||depth>MAX_DEPTH)return null;
      if(typeof node.area==='string'){
        const tabs=(Array.isArray(node.tabs)?node.tabs:[]).filter(id=>windows[id]&&!seen.has(id));tabs.forEach(id=>seen.add(id));
        return tabs.length?{area:text(node.area,80),tabs,active:tabs.includes(node.active)?node.active:tabs[0],collapsed:!!node.collapsed}:null;
      }
      if(!['row','column'].includes(node.split)||!Array.isArray(node.children))return null;
      const kept=[],sizes=[];node.children.forEach((child,i)=>{const sound=read(child,depth+1);if(sound){kept.push(sound);sizes.push(node.sizes?.[i]);}});
      return {split:node.split,sizes,pinned:!!node.pinned,children:kept};
    };
    const layout=normalize(read(raw.layout,0));
    for(const id of Object.keys(windows))if(!seen.has(id))delete windows[id];
    const tray=(Array.isArray(raw.tray)?raw.tray:[]).slice(-MAX_TRAY).map(item=>{const subject=cleanSubject(item?.subject);return subject?{subject,title:text(item.title,200),at:text(item.at,40)}:null;}).filter(Boolean);
    benches.push({id:text(raw.id,80),title:text(raw.title,160),breathing:raw.breathing!==false,layout,windows,
      activeWindow:windows[raw.activeWindow]?raw.activeWindow:areas(layout)[0]?.active??null,focus:windows[raw.focus]?raw.focus:null,tray});
  }
  if(!benches.length)benches.push(emptyBench());
  const viewDefaults={};
  for(const [view,settings]of Object.entries(object(value.viewDefaults)?value.viewDefaults:{}))if(VIEWS.includes(view)&&object(settings))viewDefaults[view]=Object.fromEntries(Object.entries(settings).filter(([,v])=>['string','number','boolean'].includes(typeof v)).slice(0,24));
  return {schema:BENCH_SCHEMA,viewDefaults,revision:Number.isInteger(value.revision)&&value.revision>=0?value.revision:0,
    active:benches.some(bench=>bench.id===value.active)?value.active:benches[0].id,benches,fragments,thoughts};
}
