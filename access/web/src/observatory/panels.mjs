import {setUiLanguage,ui,uiAttribute,uiChildren,uiComputed,uiText} from './ui-i18n.mjs';
import {createPanelGeometry} from './panel-geometry.mjs';
import {DEFAULT_INTERFACE,INTERFACE_KEY,readInterface,validateInterface} from './interface-model.mjs';

// Registered tools are local presentation adapters. Preferences cannot add code,
// endpoints or actions, and never assign a camera pose.
export function createPanelHost(root,scene,{onUserAction=()=>{}}={}){
  const panels=new Map(),tools=new Map(),trail=[];let preferences=structuredClone(DEFAULT_INTERFACE),storage=null,storageError='',returning=false;
  const toolBar=root.querySelector('.sc-header-actions');
  function revealToolFocus(target=document.activeElement){
    const control=target?.closest?.('.sc-control');if(!control||!toolBar.contains(control))return;
    const bar=toolBar.getBoundingClientRect(),box=control.getBoundingClientRect();
    const left=bar.left+toolBar.clientLeft,right=left+toolBar.clientWidth;
    toolBar.scrollLeft+=box.left<left?box.left-left:box.right>right?box.right-right:0;
  }
  toolBar.addEventListener('focusin',event=>revealToolFocus(event.target));
  try{storage=localStorage;preferences=readInterface(storage);}catch(error){storageError=error.message;}
  const key=()=>JSON.stringify([scene.port.packet?.source_revision,scene.port.packet?.fingerprint,scene.port.selection]);
  const title=id=>id==='inspector'?ui("К карточке"):({evidence:ui("К основаниям"),workspace:ui("К источникам"),navigation:ui("К маршруту"),builder:ui("К линзе"),studio:ui("К рабочему месту"),reader:ui("К чтению"),history:ui("К истории"),settings:ui("К настройкам")})[id]||ui("Назад");
  function current(){for(const [id,entry]of panels)if(!entry.element.hidden)return id;return null;}
  function save(){try{storage?.setItem(INTERFACE_KEY,JSON.stringify(preferences));storageError=storage?'':ui("Настройки действуют до закрытия страницы: хранилище недоступно.");}catch{storageError=ui("Браузер не сохранил настройки. Они действуют до закрытия страницы.");}}
  function preferredDock(){
    if(preferences.dock!=='auto')return preferences.dock;
    const bounds=root.getBoundingClientRect(),middle=bounds.left+bounds.width/2,inspector=root.querySelector('.sc-inspector');
    if(!inspector.hidden&&bounds.width>650){const box=inspector.getBoundingClientRect();return box.left+box.width/2<middle?'left':'right';}
    for(const {element}of panels.values())if(!element.hidden&&element.dataset.dock)return element.dataset.dock;
    const anchor=[...root.querySelectorAll('.sc-node')].find(node=>node.dataset.id===root.dataset.selected&&!node.hidden);
    if(anchor){const box=anchor.getBoundingClientRect();if(box.width)return box.left+box.width/2>middle?'left':'right';}
    return 'right';
  }
  function close(id){const entry=panels.get(id);if(!entry||entry.element.hidden)return;entry.onHide();entry.element.hidden=true;scene.invalidate();}
  const geometry=createPanelGeometry(root,{preferences:()=>preferences,onUserAction,
    change(delta,persist=true){preferences=validateInterface({...preferences,...delta});if(persist)save();},
    invalidate(){scene.port.cardChanged();scene.invalidate();}});
  const sizes=new ResizeObserver(()=>{geometry.refresh();revealToolFocus();});sizes.observe(toolBar);
  const styleSize=id=>geometry.apply(id);
  function back(){
    onUserAction();const stamp=key();let previous;while(trail.length){const candidate=trail.pop();if(candidate.key===stamp&&candidate.id!==current()){previous=candidate;break;}}
    if(!previous){updateBack();return;}
    returning=true;show(previous.id,false);returning=false;
    panels.get(previous.id).onResume?.();
    const focus=previous.focus;if(focus?.isConnected&&!focus.closest('[hidden]'))focus.focus();else panels.get(previous.id).element.querySelector('button')?.focus();
  }
  function updateAvailability(){for(const tool of tools.values())if(tool.available){const available=tool.available()!==false;uiAttribute(tool.opener, 'aria-disabled', String(!available));if(!available)uiAttribute(tool.opener, "data-tooltip", ui("Сначала выберите звезду или связь."));else if(tool.hint)uiAttribute(tool.opener, "data-tooltip", tool.hint);}}
  function updateBack(){updateAvailability();for(const [id,entry]of panels){const last=[...trail].reverse().find(item=>item.key===key()&&item.id!==id);entry.back.hidden=!last;uiText(entry.back, last?uiComputed(()=>'← '+title(last.id)):'');}}
  function show(id,record=true){
    if(!panels.has(id))throw new Error('Unknown panel: '+id);
    const previous=current(),dock=preferredDock();
    if(record&&!returning&&previous&&previous!==id){trail.push({id:previous,key:key(),focus:document.activeElement});if(trail.length>8)trail.shift();}
    for(const other of panels.keys())if(other!==id)close(other);
    if(id!=='inspector'){scene.closeInspector(false);root.querySelector('.sc-search-close').click();root.querySelector('.sc-lenses-close').click();}
    const element=panels.get(id).element;
    if(id!=='inspector'){const context=root.querySelector(panels.get(id).anchor||'.sc-context').getBoundingClientRect();element.style.setProperty('--sc-tool-top',`${context.bottom-root.getBoundingClientRect().top+18}px`);element.dataset.dock=dock;}
    element.hidden=false;styleSize(id);updateBack();scene.invalidate();
  }
  function register(id,element,onHide=()=>{},options={}){
    if(panels.has(id))throw new Error('Duplicate panel: '+id);
    element.dataset.panelId=id;const controls=document.createElement('div');controls.className='sc-reading-return';
    const backButton=document.createElement('button');backButton.type='button';backButton.hidden=true;backButton.addEventListener('click',back);uiChildren(controls, "append", backButton);
    element.querySelector('.sc-panel-top').after(controls);
    geometry.attach(id,element);
    panels.set(id,{element,onHide,...options,back:backButton});styleSize(id);sizes.observe(element);
  }
  register('inspector',root.querySelector('.sc-inspector'),()=>{scene.ui.captureReading();scene.ui.cancelInspector();},{onResume:()=>scene.ui.restoreReading()});
  for(const id of ['search','lenses'])geometry.attach(id,root.querySelector('.sc-'+id));
  const observer=new MutationObserver(()=>{
    if(['.sc-inspector','.sc-search','.sc-lenses'].some(selector=>!root.querySelector(selector).hidden)){
      for(const [id,entry]of panels)if(id!=='inspector'&&!entry.element.hidden)close(id);
    }
    geometry.refresh();updateBack();
  });
  for(const selector of ['.sc-inspector','.sc-search','.sc-lenses'])observer.observe(root.querySelector(selector),{attributes:true,attributeFilter:['hidden']});
  observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision','data-selected','data-inspector-id','data-inspector-kind']});
  function applyPreferences(){
    setUiLanguage(preferences.uiLanguage);
    root.dataset.theme=preferences.theme;root.dataset.readingSize=preferences.text;root.dataset.labelSize=preferences.labels;
    for(const [id,tool]of tools){tool.opener.hidden=!preferences.pinned.includes(id);if(preferences.pinned.includes(id))uiChildren(root.querySelector('.sc-header-actions'), "append", tool.opener);}
    for(const id of preferences.pinned){const tool=tools.get(id);if(tool)uiChildren(root.querySelector('.sc-header-actions'), "append", tool.opener);}
    const studio=root.querySelector('.sc-studio-open');if(studio)uiChildren(root.querySelector('.sc-header-actions'), "append", studio);
    for(const [id,entry]of panels){styleSize(id);if(id!=='inspector'&&!entry.element.hidden&&preferences.dock!=='auto')entry.element.dataset.dock=preferences.dock;}
    geometry.refresh();scene.port.setControls({scrollAction:preferences.scrollAction,dragAction:preferences.dragAction,sensitivity:preferences.sensitivity,motion:preferences.motion});
    scene.port.refreshTypography?.();scene.invalidate();
  }
  root.addEventListener('sophia-controls-change',event=>{preferences=validateInterface({...preferences,...event.detail});save();});
  applyPreferences();
  return {register,open:show,close,back,configure(id,options){Object.assign(panels.get(id),options);},
    get preferences(){return structuredClone(preferences);},get storageError(){return storageError;},
    setPreferences(value){preferences=validateInterface(value);save();applyPreferences();},
    addTool(id,tool){if(tools.has(id))throw new Error('Duplicate tool: '+id);tools.set(id,{...tool,hint:tool.opener.dataset.tooltip});applyPreferences();updateAvailability();},
    toolList:()=>[...tools].map(([id,tool])=>({id,title:tool.title,available:tool.available?.()!==false})),
    launch(id){const tool=tools.get(id);if(!tool||tool.available?.()===false)throw new Error(ui("Сначала выберите звезду или связь."));tool.launch();},
  };
}
