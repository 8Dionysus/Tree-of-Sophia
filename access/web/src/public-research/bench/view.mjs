// The workbench on the page. It draws the state the store keeps (model.mjs): the benches, the areas with their
// tabs, the borders between them, folded strips and the tray. Each window is drawn by its view (views/*.mjs) and
// lives as long as the window: moving it between areas carries the same element. The bench breathes
// (breathing.mjs): the window in which the work goes on is given its measure, after a short pause so that a click is
// not met by a jump. A border the hand moved keeps its place until a double click gives it back.

import './bench.css';
import {signMarkup} from '../space-glyphs.mjs';
import {FAMILY,activeBench,activateBench,activateWindow,addBench,applyPreset,areaOfWindow,areas,closeWindow,isArea,moveWindow,
  openSubject,releaseSizes,removeBench,renameBench,setBreathing,setCollapsed,setFocus,setSizes,setWindow,subjectKey,trayAdd,trayRemove,viewOf} from './model.mjs';
import {GUTTER,RAIL,combine,distribute,distributeColumn,measureOf,pinnedSizes} from './breathing.mjs';

const SETTLE=380,READING=1600;
const VIEW_SIGN={text:'chapter',fragment:'passage',word:'term',thought:'personal',card:'concept'};
const DRAG='application/x-tos-bench';
const PRESETS=['reading','parallel','analysis','compare'];

export function mountBench({host,store,views,t,ctxBase,onOpenChange=()=>{}}){
  const doc=host.ownerDocument;
  const el=(tag,cls,text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const btn=(label,cls,hint)=>{const node=el('button',cls,label);node.type='button';if(hint){node.setAttribute('aria-label',hint);node.title=hint;}return node;};
  const sign=kind=>{const span=el('span','bn-sign');span.innerHTML=signMarkup(kind)||'';return span;};

  const root=el('section','bn');root.hidden=true;root.setAttribute('aria-label',t('bench'));root.tabIndex=-1;
  const head=el('header','bn-head'),benchTabs=el('div','bn-benches'),title=el('input','bn-title'),tools=el('div','bn-tools');
  benchTabs.setAttribute('role','tablist');title.type='text';title.maxLength=160;
  const layoutButton=btn(t('layouts'),'bn-tool bn-layouts'),layoutMenu=el('div','bn-menu');layoutMenu.hidden=true;layoutMenu.setAttribute('role','menu');
  const breathe=btn(t('breathing'),'bn-tool bn-breathe'),undoButton=btn('↶','bn-tool bn-icon',t('undo')),redoButton=btn('↷','bn-tool bn-icon',t('redo'));
  const closeButton=btn('×','bn-tool bn-icon bn-close',t('closeBench'));
  tools.append(layoutButton,breathe,undoButton,redoButton,closeButton);head.append(benchTabs,title,tools);
  const board=el('div','bn-board'),tray=el('footer','bn-tray');
  const toast=el('p','bn-toast');toast.setAttribute('role','status');
  root.append(head,board,tray,layoutMenu,toast);host.append(root);

  const live=new Map(); // windowId → {element, body, instance, view, history:[], scroll}
  const touched=new Map();
  let shape='',pending=null,popover=null,ch=8.6,open=false,disposed=false,dragging=null,reading=ctxBase.readingLanguage?.()??'de';

  const state=()=>store.state;
  const bench=()=>activeBench(state());
  const commit=(op,options)=>store.update(op,options);

  // ------------------------------------------------------------ the context a view works with
  const ctx={
    ...ctxBase,t,
    state,bench,commit,
    async open(subject,options={}){
      const view=viewOf(subject);if(!view)return null;
      let id=null;await commit(s=>{const r=openSubject(s,subject,{...options,settings:{...(s.viewDefaults?.[view]??{}),...(options.settings??{})}});id=r.windowId;return r.state;});
      if(id){touched.set(id,Date.now());if(!open)show();}
      return id;
    },
    tray(subject,title){const already=bench().tray.some(item=>subjectKey(item.subject)===subjectKey(subject));commit(s=>trayAdd(s,subject,{title}));if(!already)say(t('onBench',[title||'']));},
    replace(windowId,subject){const entry=live.get(windowId),win=bench().windows[windowId];if(entry&&win)entry.history.push(win.subject);commit(s=>setWindow(s,windowId,{subject}),{history:false});},
    settings(windowId,patch){commit(s=>setWindow(s,windowId,{settings:patch}),{history:false});},
    effectiveSettings(windowId){const win=bench().windows[windowId];return {...(state().viewDefaults?.[win?.view]??{}),...(win?.settings??{})};},
    // A selection made in one window reaches the windows that follow it.
    broadcast(subject){
      const key=viewOf(subject);let any=false;
      for(const [id,win]of Object.entries(bench().windows))if(win.view===key&&win.binding==='follow'){any=true;if(subjectKey(win.subject)!==subjectKey(subject))commit(s=>setWindow(s,id,{subject}),{history:false});}
      return any;
    },
    windowsOf(view){return Object.entries(bench().windows).filter(([,win])=>win.view===view).map(([id,win])=>({id,win}));},
    say,
    changed(){if(open)draw();},
    compare(){commit(s=>applyPreset(s,'compare'));},
  };
  function say(message){toast.textContent=message;toast.dataset.on='true';clearTimeout(say.timer);say.timer=setTimeout(()=>{toast.dataset.on='false';},2600);}

  // ------------------------------------------------------------ windows
  function windowEntry(id,win){
    let entry=live.get(id);
    if(entry&&entry.view!==win.view){entry.instance?.dispose?.();entry.element.remove();live.delete(id);entry=null;}
    if(!entry){
      const element=el('div','bn-window'),body=el('div','bn-window-body');element.dataset.window=id;element.dataset.view=win.view;element.append(body);
      entry={element,body,view:win.view,history:[],instance:null,subject:''};live.set(id,entry);
      const factory=views[win.view];
      try{entry.instance=factory?.({root:body,windowId:id,ctx})??null;}catch(error){body.replaceChildren(el('p','bn-failed',`${t('unavailable')} ${error?.message??''}`));}
    }
    const key=subjectKey(win.subject)+JSON.stringify(win.settings??{})+JSON.stringify(state().viewDefaults?.[win.view]??{})+win.binding;
    try{
      if(entry.subject!==key){entry.subject=key;entry.instance?.update?.(win);}
      else entry.instance?.stateChanged?.(state());
    }catch(error){entry.body.replaceChildren(el('p','bn-failed',`${t('unavailable')} ${error?.message??''}`));}
    return entry;
  }
  const titleOf=(id,win)=>{try{return live.get(id)?.instance?.title?.(win)||t(`view_${win.view}`);}catch{return t(`view_${win.view}`);}};
  const signOf=(id,win)=>live.get(id)?.instance?.sign?.(win)??VIEW_SIGN[win.view];
  function forget(){
    const known=new Set(Object.keys(bench().windows));
    for(const [id,entry]of live)if(!known.has(id)){entry.instance?.dispose?.();entry.element.remove();live.delete(id);}
  }

  // ------------------------------------------------------------ drawing
  function draw(){
    if(disposed)return;
    const current=bench();
    renderHead(current);renderTray(current);
    for(const [id,win]of Object.entries(current.windows))windowEntry(id,win);
    forget();
    const nextShape=JSON.stringify([current.id,current.focus,shapeOf(current.layout)]);
    if(nextShape!==shape){shape=nextShape;build(current);}
    else refreshTabs(current);
    layout();
  }
  const shapeOf=node=>!node?null:isArea(node)?[node.area,node.tabs,node.active,node.collapsed]:[node.split,node.pinned,node.children.map(shapeOf)];

  function renderHead(current){
    benchTabs.replaceChildren();
    state().benches.forEach((item,i)=>{
      const tab=btn(item.title||t('benchName',[i+1]),'bn-bench-tab');tab.setAttribute('role','tab');tab.setAttribute('aria-selected',String(item.id===current.id));
      tab.addEventListener('click',()=>{if(item.id!==current.id)commit(s=>activateBench(s,item.id),{history:false});});
      benchTabs.append(tab);
      if(item.id===current.id&&state().benches.length>1){const remove=btn('×','bn-bench-remove',t('removeBench'));remove.addEventListener('click',event=>{event.stopPropagation();commit(s=>removeBench(s,item.id));});tab.append(remove);}
    });
    const add=btn('+','bn-bench-add',t('newBench'));add.addEventListener('click',()=>commit(s=>addBench(s)));benchTabs.append(add);
    if(doc.activeElement!==title)title.value=current.title;title.placeholder=t('question');title.setAttribute('aria-label',t('question'));
    breathe.setAttribute('aria-pressed',String(current.breathing));breathe.title=current.breathing?t('breathingOn'):t('breathingOff');
    undoButton.disabled=!store.canUndo();redoButton.disabled=!store.canRedo();
  }
  function renderTray(current){
    tray.replaceChildren();
    const label=el('span','bn-tray-label',t('tray'));tray.append(label);
    if(!current.tray.length){tray.append(el('span','bn-tray-empty',t('trayEmpty')));return;}
    const list=el('div','bn-chips');
    for(const item of current.tray){
      const chip=el('div','bn-chip');chip.draggable=true;chip.dataset.family=FAMILY[viewOf(item.subject)]??'';
      const go=btn('','bn-chip-open',t('trayOpen'));go.append(sign(VIEW_SIGN[viewOf(item.subject)]==='concept'?(ctxBase.kindOf?.(item.subject)??'concept'):VIEW_SIGN[viewOf(item.subject)]),el('span','bn-chip-title',item.title||ctxBase.titleOf?.(item.subject)||subjectKey(item.subject)));
      go.addEventListener('click',event=>ctx.open(item.subject,event.shiftKey?{place:'auto'}:{}));
      const remove=btn('×','bn-chip-remove',t('trayRemove'));remove.addEventListener('click',()=>commit(s=>trayRemove(s,subjectKey(item.subject))));
      chip.addEventListener('dragstart',event=>{event.dataTransfer.setData(DRAG,JSON.stringify({subject:item.subject,title:item.title}));event.dataTransfer.setData('text/plain',item.title||'');event.dataTransfer.effectAllowed='copyMove';});
      chip.append(go,remove);list.append(chip);
    }
    tray.append(list);
  }

  // The tree of areas as elements; window bodies move into their areas.
  function build(current){
    for(const entry of live.values())entry.scroll=entry.body.scrollTop;
    board.replaceChildren();
    if(!current.layout){board.append(emptyBench());return;}
    if(current.focus&&current.windows[current.focus]){
      const frame=el('div','bn-focus'),rails=el('div','bn-focus-rails');
      for(const area of areas(current.layout))for(const id of area.tabs)if(id!==current.focus)rails.append(railFor(id,current.windows[id],()=>commit(s=>setFocus(s,id),{history:false})));
      const area=areaElement({area:'focus',tabs:[current.focus],active:current.focus,collapsed:false},current,[]);area.classList.add('bn-focused');
      frame.append(rails,area);board.append(frame);
    }else board.append(nodeElement(current.layout,current,[]));
    for(const entry of live.values())if(entry.element.isConnected)entry.body.scrollTop=entry.scroll??0;
  }
  function nodeElement(node,current,path){
    if(isArea(node))return node.collapsed?railElement(node,current):areaElement(node,current,path);
    const split=el('div','bn-split');split.dataset.dir=node.split;split.dataset.pinned=String(!!node.pinned);split.dataset.path=path.join('.');
    node.children.forEach((child,i)=>{
      if(i){const gutter=el('div','bn-gutter');gutter.setAttribute('role','separator');gutter.setAttribute('aria-orientation',node.split==='row'?'vertical':'horizontal');gutter.tabIndex=0;gutter.title=node.pinned?t('pinnedSplit'):t('gutter');gutter.setAttribute('aria-label',t('gutter'));wireGutter(gutter,path,i);split.append(gutter);}
      const cell=el('div','bn-cell');cell.dataset.index=String(i);cell.append(nodeElement(child,current,[...path,i]));split.append(cell);
    });
    return split;
  }
  function railFor(id,win,action){
    const rail=btn('','bn-rail',titleOf(id,win));rail.dataset.family=FAMILY[win.view];
    rail.append(sign(signOf(id,win)),el('span','bn-rail-title',titleOf(id,win)));rail.addEventListener('click',action);return rail;
  }
  function railElement(area,current){
    const id=area.active,win=current.windows[id];
    const rail=railFor(id,win,()=>commit(s=>setCollapsed(s,area.area,false)));rail.dataset.area=area.area;wireDrop(rail,area.area);return rail;
  }
  function areaElement(area,current,path){
    const box=el('section','bn-area');box.dataset.area=area.area;box.dataset.family=FAMILY[current.windows[area.active]?.view]??'';
    const bar=el('div','bn-area-head'),tabs=el('div','bn-tabs'),tools=el('div','bn-area-tools'),body=el('div','bn-area-body');
    tabs.setAttribute('role','tablist');
    for(const id of area.tabs){
      const win=current.windows[id];if(!win)continue;
      const tab=el('div','bn-tab');tab.dataset.window=id;tab.draggable=true;tab.dataset.family=FAMILY[win.view];tab.setAttribute('aria-selected',String(id===area.active));
      const go=btn('','bn-tab-open');go.setAttribute('role','tab');go.append(sign(signOf(id,win)),el('span','bn-tab-title',titleOf(id,win)));go.title=titleOf(id,win);
      go.addEventListener('click',()=>{touched.set(id,Date.now());commit(s=>activateWindow(s,id),{history:false});});
      const close=btn('×','bn-tab-close',t('closeWindow'));close.addEventListener('click',event=>{event.stopPropagation();commit(s=>closeWindow(s,id));});
      tab.addEventListener('dragstart',event=>{event.dataTransfer.setData(DRAG,JSON.stringify({windowId:id}));event.dataTransfer.setData('text/plain',titleOf(id,win));event.dataTransfer.effectAllowed='move';dragging=id;});
      tab.addEventListener('dragend',()=>{dragging=null;});
      tab.append(go,close);tabs.append(tab);
    }
    const win=current.windows[area.active],entry=live.get(area.active);
    if(entry?.history.length){const back=btn('←','bn-area-tool',t('back'));back.addEventListener('click',()=>{const previous=entry.history.pop();if(previous)commit(s=>setWindow(s,area.active,{subject:previous}),{history:false});});tools.append(back);}
    if(win&&(win.view==='word'||win.view==='card')){
      const bind=btn(win.binding==='follow'?'◉':'○','bn-area-tool bn-bind',win.binding==='follow'?t('follow'):t('pinned'));bind.setAttribute('aria-pressed',String(win.binding==='follow'));
      bind.addEventListener('click',()=>commit(s=>setWindow(s,area.active,{binding:win.binding==='follow'?'pinned':'follow'}),{history:false}));tools.append(bind);
    }
    if(entry?.instance?.settings){const gear=btn('⚙','bn-area-tool',t('settings'));gear.addEventListener('click',event=>openSettings(event.currentTarget,area.active));tools.append(gear);}
    const focused=current.focus===area.active;
    const focus=btn(focused?'⤡':'⤢','bn-area-tool',focused?t('unfocus'):t('focus'));focus.addEventListener('click',()=>commit(s=>setFocus(s,focused?null:area.active),{history:false}));tools.append(focus);
    if(!focused&&areas(current.layout).length>1){const fold=btn('⟨','bn-area-tool',t('fold'));fold.addEventListener('click',()=>commit(s=>setCollapsed(s,area.area,true)));tools.append(fold);}
    bar.append(tabs,tools);
    for(const id of area.tabs){const item=live.get(id);if(!item)continue;item.element.hidden=id!==area.active;body.append(item.element);}
    box.append(bar,body);
    if(area.area!=='focus')wireDrop(box,area.area);
    return box;
  }
  function refreshTabs(current){
    for(const area of areas(current.layout)){
      const box=board.querySelector(`.bn-area[data-area="${CSS.escape(area.area)}"]`);if(!box)continue;
      box.dataset.family=FAMILY[current.windows[area.active]?.view]??'';
      for(const tab of box.querySelectorAll('.bn-tab')){const id=tab.dataset.window,win=current.windows[id];if(!win)continue;const name=titleOf(id,win);tab.querySelector('.bn-tab-title').textContent=name;tab.querySelector('.bn-tab-open').title=name;const mark=tab.querySelector('.bn-sign');mark.innerHTML=signMarkup(signOf(id,win))||'';}
    }
    for(const box of board.querySelectorAll('.bn-area'))box.dataset.active=String(box.querySelector(`.bn-window[data-window="${CSS.escape(current.activeWindow??'')}"]`)?.hidden===false);
  }
  function emptyBench(){
    const box=el('div','bn-empty'),signs=el('div','bn-empty-signs');
    for(const kind of ['chapter','term','concept','personal'])signs.append(sign(kind));
    box.append(signs,el('h2','bn-empty-title',t('emptyTitle')),el('p','bn-empty-hint',t('emptyHint')));
    const actions=el('div','bn-empty-actions');
    if(ctxBase.canRead){const read=btn(t('openText'),'bn-action bn-primary');read.addEventListener('click',()=>ctx.open({kind:'chapter',chapter:ctxBase.startChapter?.()??'I.01',language:ctxBase.readingLanguage?.()??'de'}));actions.append(read);}
    for(const name of PRESETS.filter(item=>item!=='compare'&&ctxBase.canRead)){const go=btn(t(`preset_${name}`),'bn-action');go.title=t(`presetHint_${name}`);go.addEventListener('click',()=>preset(name));actions.append(go);}
    box.append(actions);wireDrop(box,null);return box;
  }

  // ------------------------------------------------------------ breathing
  function measureChar(){
    const probe=el('span','bn-probe','abcdefghijklmnopqrstuvwxyz абвгдеёжзийклмнопрстуфхцчшщ');board.append(probe);
    const width=probe.getBoundingClientRect().width;probe.remove();
    if(width>0)ch=width/52;
  }
  function measureArea(area,current){
    if(area.collapsed)return {min:RAIL,comfort:RAIL,max:RAIL,collapsed:true};
    const win=current.windows[area.active],settings={...(state().viewDefaults?.[win?.view]??{}),...(win?.settings??{})};
    const m=live.get(area.active)?.instance?.measure?.(ch)??measureOf(win?.view,{ch,parallel:!!settings.parallel});
    return {...m,active:area.tabs.includes(current.activeWindow),touched:Math.max(0,...area.tabs.map(id=>touched.get(id)??0))};
  }
  function measureNode(node,current){
    if(isArea(node))return measureArea(node,current);
    const items=node.children.map(child=>measureNode(child,current));
    return {...combine(node.split,items),active:items.some(item=>item.active),touched:Math.max(0,...items.map(item=>item.touched??0)),fold:false};
  }
  function layout(){
    const current=bench();if(!current.layout||current.focus||!open)return;
    const split=board.firstElementChild;if(!split?.classList.contains('bn-split'))return;
    const box=board.getBoundingClientRect();
    place(current.layout,split,box.width,box.height,current);
  }
  function place(node,element,width,height,current){
    if(isArea(node)||!element)return;
    const items=node.children.map(child=>measureNode(child,current));
    const shares=node.sizes,row=node.split==='row';
    let sizes,folded=[];
    if(node.pinned||!current.breathing)sizes=pinnedSizes(row?width:height,items,shares);
    else if(row)({sizes,collapsed:folded}=distribute(width,items));
    else ({sizes}=distributeColumn(height,items,shares));
    const cells=[...element.children].filter(child=>child.classList.contains('bn-cell'));
    cells.forEach((cell,i)=>{
      cell.style.flexBasis=`${sizes[i]}px`;
      const child=node.children[i],inner=cell.firstElementChild;
      const fold=folded.includes(i)&&isArea(child);
      if(fold&&!inner?.classList.contains('bn-rail')){const rail=railFor(child.active,current.windows[child.active],()=>{touched.set(child.active,Date.now());commit(s=>activateWindow(s,child.active),{history:false});});rail.dataset.auto='true';cell.dataset.folded='true';cell.replaceChildren(rail);}
      else if(!fold&&cell.dataset.folded==='true'){delete cell.dataset.folded;cell.replaceChildren(nodeElement(child,current,[...(element.dataset.path?element.dataset.path.split('.').map(Number):[]),i]));}
      place(child,cell.firstElementChild,row?sizes[i]:width,row?height:sizes[i],current);
    });
  }
  // A click is met after a short pause, so the window does not jump under the pointer; reading takes longer to count.
  function activity(id,{delay=SETTLE}={}){
    if(!id||!bench().windows[id])return;
    touched.set(id,Date.now());
    if(bench().activeWindow===id){clearTimeout(pending?.timer);pending=null;return;}
    if(pending?.id===id)return;
    clearTimeout(pending?.timer);
    pending={id,timer:setTimeout(()=>{pending=null;if(bench().windows[id])commit(s=>activateWindow(s,id),{history:false});},delay)};
  }
  const windowOf=target=>target?.closest?.('.bn-window')?.dataset.window??null;
  board.addEventListener('pointerdown',event=>activity(windowOf(event.target)),true);
  board.addEventListener('focusin',event=>activity(windowOf(event.target)));
  board.addEventListener('keydown',event=>activity(windowOf(event.target),{delay:0}));
  let scrollStart=null;
  board.addEventListener('scroll',event=>{const id=windowOf(event.target);if(!id||id===bench().activeWindow)return;if(scrollStart?.id!==id)scrollStart={id,at:Date.now()};else if(Date.now()-scrollStart.at>READING){activity(id,{delay:0});scrollStart=null;}},true);

  // ------------------------------------------------------------ borders
  function wireGutter(gutter,path,index){
    gutter.addEventListener('pointerdown',event=>{
      if(event.button!==0)return;event.preventDefault();gutter.setPointerCapture(event.pointerId);
      const split=gutter.parentElement,row=split.dataset.dir==='row',cells=[...split.children].filter(child=>child.classList.contains('bn-cell'));
      const sizes=cells.map(cell=>row?cell.getBoundingClientRect().width:cell.getBoundingClientRect().height),start=row?event.clientX:event.clientY;
      root.dataset.dragging='true';
      const move=e=>{const delta=(row?e.clientX:e.clientY)-start,a=index-1,b=index,min=row?160:110;
        const da=Math.max(min-sizes[a],Math.min(sizes[b]-min,delta));cells[a].style.flexBasis=`${sizes[a]+da}px`;cells[b].style.flexBasis=`${sizes[b]-da}px`;};
      const up=e=>{gutter.removeEventListener('pointermove',move);gutter.removeEventListener('pointerup',up);gutter.removeEventListener('pointercancel',up);delete root.dataset.dragging;
        const now=cells.map(cell=>row?cell.getBoundingClientRect().width:cell.getBoundingClientRect().height);
        if(Math.abs((row?e.clientX:e.clientY)-start)>3)commit(s=>setSizes(s,path,now));};
      gutter.addEventListener('pointermove',move);gutter.addEventListener('pointerup',up);gutter.addEventListener('pointercancel',up);
    });
    gutter.addEventListener('dblclick',()=>commit(s=>releaseSizes(s,path)));
    gutter.addEventListener('keydown',event=>{
      const step={ArrowLeft:-24,ArrowUp:-24,ArrowRight:24,ArrowDown:24}[event.key];
      if(event.key==='Home'){event.preventDefault();commit(s=>releaseSizes(s,path));return;}
      if(!step)return;event.preventDefault();
      const split=gutter.parentElement,row=split.dataset.dir==='row',cells=[...split.children].filter(child=>child.classList.contains('bn-cell'));
      const sizes=cells.map(cell=>row?cell.getBoundingClientRect().width:cell.getBoundingClientRect().height);
      sizes[index-1]=Math.max(120,sizes[index-1]+step);sizes[index]=Math.max(120,sizes[index]-step);commit(s=>setSizes(s,path,sizes));
    });
  }

  // ------------------------------------------------------------ dragging windows and tray items
  const drop=el('div','bn-drop');drop.hidden=true;
  const zone=(box,event)=>{
    const r=box.getBoundingClientRect(),x=(event.clientX-r.left)/r.width,y=(event.clientY-r.top)/r.height,edge=0.24;
    if(x<edge)return 'left';if(x>1-edge)return 'right';if(y<edge)return 'top';if(y>1-edge)return 'bottom';return 'center';
  };
  function wireDrop(box,areaId){
    box.addEventListener('dragover',event=>{
      if(!event.dataTransfer.types.includes(DRAG))return;event.preventDefault();
      const where=areaId?zone(box,event):'center';drop.dataset.zone=where;
      const r=box.getBoundingClientRect(),b=root.getBoundingClientRect();
      // The outline shows where the window will lie: the whole area for a tab, its half for a new area beside it.
      let left=r.left-b.left,top=r.top-b.top,width=r.width,height=r.height;
      if(where==='left')width/=2;if(where==='right'){left+=width/2;width/=2;}
      if(where==='top')height/=2;if(where==='bottom'){top+=height/2;height/=2;}
      Object.assign(drop.style,{left:`${left}px`,top:`${top}px`,width:`${width}px`,height:`${height}px`});drop.hidden=false;
    });
    box.addEventListener('dragleave',event=>{if(!box.contains(event.relatedTarget))drop.hidden=true;});
    box.addEventListener('drop',event=>{
      const raw=event.dataTransfer.getData(DRAG);drop.hidden=true;if(!raw)return;event.preventDefault();
      let data;try{data=JSON.parse(raw);}catch{return;}
      const where=areaId?zone(box,event):'center';
      if(data.windowId&&areaId)commit(s=>moveWindow(s,data.windowId,{area:areaId,edge:where}));
      else if(data.subject)ctx.open(data.subject,areaId?{place:{area:areaId,edge:where}}:{});
    });
  }
  root.append(drop);

  // ------------------------------------------------------------ settings
  function closePopover(){popover?.remove();popover=null;}
  function openSettings(anchor,windowId){
    closePopover();
    const entry=live.get(windowId),win=bench().windows[windowId];if(!entry?.instance?.settings||!win)return;
    const box=el('div','bn-popover');box.setAttribute('role','dialog');box.setAttribute('aria-label',t('settings'));
    const values=entry.instance.values?.()??ctx.effectiveSettings(windowId);
    for(const control of entry.instance.settings(win,values)){
      const row=el('div','bn-setting');row.append(el('span','bn-setting-label',control.label));
      if(control.type==='toggle'){const toggle=btn(values[control.key]?'●':'○','bn-toggle');toggle.setAttribute('aria-pressed',String(!!values[control.key]));toggle.addEventListener('click',()=>{ctx.settings(windowId,{[control.key]:!values[control.key]});closePopover();});row.append(toggle);}
      else{const group=el('div','bn-choices');for(const option of control.options){const choice=btn(option.label,'bn-choice');choice.setAttribute('aria-pressed',String(values[control.key]===option.value));choice.addEventListener('click',()=>{ctx.settings(windowId,{[control.key]:option.value});closePopover();});group.append(choice);}row.append(group);}
      box.append(row);
    }
    const all=btn(t('makeDefault'),'bn-link');all.addEventListener('click',()=>{const view=win.view,settings=ctx.effectiveSettings(windowId);commit(s=>({...s,viewDefaults:{...(s.viewDefaults??{}),[view]:settings}}),{history:false});closePopover();});
    box.append(all);
    root.append(box);
    const a=anchor.getBoundingClientRect(),r=root.getBoundingClientRect();
    box.style.top=`${a.bottom-r.top+6}px`;box.style.left=`${Math.max(8,Math.min(r.width-box.offsetWidth-8,a.right-r.left-box.offsetWidth))}px`;
    popover=box;
  }

  // ------------------------------------------------------------ the head
  title.addEventListener('input',()=>{clearTimeout(title.timer);title.timer=setTimeout(()=>commit(s=>renameBench(s,null,title.value),{history:false}),350);});
  breathe.addEventListener('click',()=>commit(s=>setBreathing(s,null,!bench().breathing)));
  undoButton.addEventListener('click',()=>store.undo());redoButton.addEventListener('click',()=>store.redo());
  closeButton.addEventListener('click',()=>hide());
  layoutButton.addEventListener('click',()=>{
    layoutMenu.replaceChildren();
    for(const name of PRESETS){const item=btn('','bn-menu-item');item.setAttribute('role','menuitem');item.append(el('strong','',t(`preset_${name}`)),el('span','',t(`presetHint_${name}`)));item.addEventListener('click',()=>{layoutMenu.hidden=true;preset(name);});layoutMenu.append(item);}
    const r=layoutButton.getBoundingClientRect(),b=root.getBoundingClientRect();layoutMenu.style.top=`${r.bottom-b.top+6}px`;layoutMenu.style.right=`${b.right-r.right}px`;layoutMenu.hidden=!layoutMenu.hidden;
  });
  function preset(name){
    if(!ctxBase.canRead&&name!=='compare')return;
    commit(s=>applyPreset(s,name,{chapter:ctxBase.startChapter?.()??'I.01',language:ctxBase.readingLanguage?.()??'de'}));
  }
  doc.addEventListener('pointerdown',event=>{
    if(popover&&!popover.contains(event.target)&&!event.target.closest?.('.bn-area-tool'))closePopover();
    if(!layoutMenu.hidden&&!layoutMenu.contains(event.target)&&event.target!==layoutButton)layoutMenu.hidden=true;
  },true);
  const keys=event=>{
    if(!open||event.defaultPrevented)return;
    const typing=event.target.closest?.('input,textarea,select,[contenteditable="true"]');
    if(event.key==='Escape'){
      if(popover){event.preventDefault();closePopover();return;}
      // A selection's menu in a window leaves first, with the selection.
      const selectionMenu=root.querySelector('.bn-select-menu:not([hidden])');
      if(selectionMenu){event.preventDefault();selectionMenu.hidden=true;doc.getSelection()?.removeAllRanges();return;}
      if(!layoutMenu.hidden){event.preventDefault();layoutMenu.hidden=true;return;}
      if(typing){event.target.blur();return;}
      if(ctxBase.isBusy?.())return;
      event.preventDefault();hide();return;
    }
    if(typing)return;
    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='z'){event.preventDefault();event.shiftKey?store.redo():store.undo();return;}
    if(event.key==='F6'){
      event.preventDefault();const list=[...board.querySelectorAll('.bn-area,.bn-rail')];if(!list.length)return;
      const at=list.findIndex(item=>item.contains(doc.activeElement)),next=list[(at+(event.shiftKey?-1:1)+list.length)%list.length];
      (next.querySelector('.bn-tab[aria-selected=true] .bn-tab-open')??next).focus();
    }
    if(event.code==='KeyB'&&!event.ctrlKey&&!event.metaKey&&!event.altKey){event.preventDefault();hide();}
  };
  doc.addEventListener('keydown',keys);

  const resize=typeof ResizeObserver==='function'?new ResizeObserver(()=>{if(open)requestAnimationFrame(layout);}):null;
  resize?.observe(board);
  const unsubscribe=store.subscribe(()=>{if(open)draw();else renderHead(bench());});

  function show(){
    if(open)return;open=true;root.hidden=false;host.dataset.benchOpen='true';measureChar();shape='';draw();onOpenChange(true);
    requestAnimationFrame(()=>{(board.querySelector('.bn-tab[aria-selected=true] .bn-tab-open')??root).focus({preventScroll:true});});
  }
  function hide(){if(!open)return;open=false;closePopover();layoutMenu.hidden=true;root.hidden=true;delete host.dataset.benchOpen;onOpenChange(false);}

  return {
    root,ctx,
    isOpen:()=>open,show,hide,toggle:()=>open?hide():show(),
    count:()=>bench().tray.length+Object.keys(bench().windows).length,
    refresh(){if(open){shape='';draw();}},
    /** The reader's language changed: every word of the bench is drawn again in it, windows included. */
    relocale(){
      root.setAttribute('aria-label',t('bench'));layoutButton.textContent=t('layouts');breathe.textContent=t('breathing');
      for(const [button,key] of [[undoButton,'undo'],[redoButton,'redo'],[closeButton,'closeBench']]){button.setAttribute('aria-label',t(key));button.title=t(key);}
      closePopover();layoutMenu.hidden=true;
      // A text that reads in the reader's language by default (no witness chosen by hand) follows it to the new one.
      const before=reading,next=ctxBase.readingLanguage?.()??before;reading=next;
      if(next!==before)commit(s=>{let out=s;for(const [id,win]of Object.entries(activeBench(s).windows))
        if(win.view==='text'&&!win.settings?.witness&&win.subject.language===before)out=setWindow(out,id,{subject:{...win.subject,language:next}});return out;},{history:false});
      for(const entry of live.values()){entry.instance?.dispose?.();entry.element.remove();}
      live.clear();shape='';
      if(open)draw();
    },
    dispose(){disposed=true;unsubscribe();resize?.disconnect();doc.removeEventListener('keydown',keys);for(const entry of live.values())entry.instance?.dispose?.();live.clear();root.remove();},
  };
}
