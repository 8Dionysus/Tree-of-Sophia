import {DEFAULT_INTERFACE,INTERFACE_KEY,readInterface,validateInterface} from './interface-model.mjs';

// Registered tools are local presentation adapters. Preferences cannot add code,
// endpoints or actions, and never assign a camera pose.
export function createPanelHost(root,scene,{onUserAction=()=>{}}={}){
  const panels=new Map(),tools=new Map(),trail=[];let preferences=structuredClone(DEFAULT_INTERFACE),storage=null,storageError='',returning=false;
  try{storage=localStorage;preferences=readInterface(storage);}catch(error){storageError=error.message;}
  const key=()=>JSON.stringify([scene.port.packet?.source_revision,scene.port.packet?.fingerprint,scene.port.selection]);
  const title=id=>id==='inspector'?'К карточке':({evidence:'К основаниям',workspace:'К источникам',navigation:'К маршруту',builder:'К линзе',studio:'К рабочему месту',reader:'К чтению',history:'К истории',settings:'К настройкам'})[id]||'Назад';
  function current(){for(const [id,entry]of panels)if(!entry.element.hidden)return id;return null;}
  function save(){try{storage?.setItem(INTERFACE_KEY,JSON.stringify(preferences));storageError=storage?'':'Настройки действуют до закрытия страницы: хранилище недоступно.';}catch{storageError='Браузер не сохранил настройки. Они действуют до закрытия страницы.';}}
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
  const sizes=new ResizeObserver(()=>{scene.port.cardChanged();scene.invalidate();});
  function styleSize(id){const entry=panels.get(id),size=preferences.sizes[id];if(!entry)return;
    for(const axis of ['width','height']){if(size)entry.element.style.setProperty('--sc-panel-'+axis,size[axis]+'px');else entry.element.style.removeProperty('--sc-panel-'+axis);}
  }
  function resize(id,width,height){preferences.sizes[id]={width:Math.max(280,Math.min(760,width)),height:Math.max(240,Math.min(800,height))};styleSize(id);scene.invalidate();}
  function back(){
    onUserAction();const stamp=key();let previous;while(trail.length){const candidate=trail.pop();if(candidate.key===stamp&&candidate.id!==current()){previous=candidate;break;}}
    if(!previous){updateBack();return;}
    returning=true;show(previous.id,false);returning=false;
    panels.get(previous.id).onResume?.();
    const focus=previous.focus;if(focus?.isConnected&&!focus.closest('[hidden]'))focus.focus();else panels.get(previous.id).element.querySelector('button')?.focus();
  }
  function updateAvailability(){for(const tool of tools.values())if(tool.available){const available=tool.available()!==false;tool.opener.setAttribute('aria-disabled',String(!available));if(!available)tool.opener.dataset.tooltip='Сначала выберите звезду или связь.';else if(tool.hint)tool.opener.dataset.tooltip=tool.hint;}}
  function updateBack(){updateAvailability();for(const [id,entry]of panels){const last=[...trail].reverse().find(item=>item.key===key()&&item.id!==id);entry.back.hidden=!last;entry.back.textContent=last?'← '+title(last.id):'';}}
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
    const backButton=document.createElement('button');backButton.type='button';backButton.hidden=true;backButton.addEventListener('click',back);controls.append(backButton);
    element.querySelector('.sc-panel-top').after(controls);
    const size=document.createElement('button');size.type='button';size.className='sc-panel-size';size.textContent='↔';size.setAttribute('aria-label','Изменить размер окна');size.dataset.tooltip='Размер окна: обычный / просторный';
    size.addEventListener('click',()=>{onUserAction();const current=preferences.sizes[id];resize(id,current?.width>=560?420:640,current?.width>=560?440:620);save();});element.querySelector('.sc-panel-top').insertBefore(size,element.querySelector('.sc-panel-top').lastElementChild);
    const grip=document.createElement('button');grip.type='button';grip.className='sc-panel-resize';grip.textContent='⌟';grip.setAttribute('aria-label','Размер окна: стрелки меняют ширину и высоту');grip.dataset.tooltip='Потяните угол; стрелки меняют размер, Home сбрасывает';
    let drag=null;
    grip.addEventListener('pointerdown',e=>{if(e.button!==0)return;onUserAction();const rect=element.getBoundingClientRect();drag={id:e.pointerId,x:e.clientX,y:e.clientY,w:rect.width,h:rect.height};grip.setPointerCapture(e.pointerId);e.preventDefault();});
    grip.addEventListener('pointermove',e=>{if(drag?.id!==e.pointerId)return;resize(id,drag.w+(e.clientX-drag.x)*(element.dataset.dock==='right'?-1:1),drag.h+e.clientY-drag.y);});
    const release=()=>{if(drag){drag=null;save();}};grip.addEventListener('pointerup',release);grip.addEventListener('pointercancel',release);grip.addEventListener('lostpointercapture',release);
    grip.addEventListener('keydown',e=>{const delta={ArrowLeft:[-20,0],ArrowRight:[20,0],ArrowUp:[0,-20],ArrowDown:[0,20]}[e.key];if(e.key==='Home'){onUserAction();e.preventDefault();delete preferences.sizes[id];styleSize(id);save();}else if(delta){onUserAction();e.preventDefault();const box=element.getBoundingClientRect();resize(id,box.width+delta[0],box.height+delta[1]);save();}});element.append(grip);
    panels.set(id,{element,onHide,...options,back:backButton});styleSize(id);sizes.observe(element);
  }
  register('inspector',root.querySelector('.sc-inspector'),()=>{scene.ui.captureReading();scene.ui.cancelInspector();},{onResume:()=>scene.ui.restoreReading()});
  const observer=new MutationObserver(()=>{
    if(['.sc-inspector','.sc-search','.sc-lenses'].some(selector=>!root.querySelector(selector).hidden)){
      for(const [id,entry]of panels)if(id!=='inspector'&&!entry.element.hidden)close(id);
    }
    updateBack();
  });
  for(const selector of ['.sc-inspector','.sc-search','.sc-lenses'])observer.observe(root.querySelector(selector),{attributes:true,attributeFilter:['hidden']});
  observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision','data-selected','data-inspector-id','data-inspector-kind']});
  function applyPreferences(){
    root.dataset.readingSize=preferences.text;root.dataset.labelSize=preferences.labels;
    for(const [id,tool]of tools){tool.opener.hidden=!preferences.pinned.includes(id);if(preferences.pinned.includes(id))root.querySelector('.sc-header-actions').append(tool.opener);}
    for(const id of preferences.pinned){const tool=tools.get(id);if(tool)root.querySelector('.sc-header-actions').append(tool.opener);}
    const studio=root.querySelector('.sc-studio-open');if(studio)root.querySelector('.sc-header-actions').append(studio);
    for(const [id,entry]of panels){styleSize(id);if(id!=='inspector'&&!entry.element.hidden&&preferences.dock!=='auto')entry.element.dataset.dock=preferences.dock;}
    scene.port.setControls({inputMode:preferences.inputMode,motion:preferences.motion});
    scene.port.refreshTypography?.();scene.invalidate();
  }
  root.addEventListener('sophia-controls-change',event=>{preferences=validateInterface({...preferences,...event.detail});save();});
  applyPreferences();
  return {register,open:show,close,back,configure(id,options){Object.assign(panels.get(id),options);},
    get preferences(){return structuredClone(preferences);},get storageError(){return storageError;},
    setPreferences(value){preferences=validateInterface(value);save();applyPreferences();},
    addTool(id,tool){if(tools.has(id))throw new Error('Duplicate tool: '+id);tools.set(id,{...tool,hint:tool.opener.dataset.tooltip});applyPreferences();updateAvailability();},
    toolList:()=>[...tools].map(([id,tool])=>({id,title:tool.title,available:tool.available?.()!==false})),
    launch(id){const tool=tools.get(id);if(!tool||tool.available?.()===false)throw new Error('Сначала выберите звезду или связь.');tool.launch();},
  };
}
