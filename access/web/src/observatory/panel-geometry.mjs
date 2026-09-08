import {ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
const clamp=(value,min,max)=>Math.max(min,Math.min(Math.max(min,max),value));
export function windowBounds(area,box){
  return {left:15,top:area.header+12,right:Math.max(15,area.width-box.width-15),bottom:Math.max(area.header+12,area.height-box.height-area.footer)};
}
export function windowPoint(position,bounds){return {x:bounds.left+(bounds.right-bounds.left)*position.x,y:bounds.top+(bounds.bottom-bounds.top)*position.y};}
export function windowFraction(point,bounds){return {x:clamp((point.x-bounds.left)/Math.max(1,bounds.right-bounds.left),0,1),y:clamp((point.y-bounds.top)/Math.max(1,bounds.bottom-bounds.top),0,1)};}

// Window geometry is local presentation state. Dragging never updates a query,
// graph selection, camera, reading position or source packet.
export function createPanelGeometry(root,{preferences,change,invalidate,onUserAction}){
  const windows=new Map();let gesture=null;
  const write=(element,key,value)=>{if(element.style.getPropertyValue(key)!==value)element.style.setProperty(key,value);};
  function area(){const box=root.getBoundingClientRect(),header=root.querySelector('.sc-header').getBoundingClientRect();return {width:root.clientWidth,height:root.clientHeight,header:header.bottom-box.top,footer:90};}
  function apply(id){
    const element=windows.get(id);if(!element)return;
    const current=preferences(),size=current.sizes[id],position=current.positions[id],space=area();
    write(element,'--sc-window-height-limit',Math.max(180,space.height-space.header-102)+'px');
    for(const axis of ['width','height']){if(size)write(element,'--sc-panel-'+axis,size[axis]+'px');else element.style.removeProperty('--sc-panel-'+axis);}
    element.dataset.windowSized=String(Boolean(size));element.dataset.floating=String(Boolean(position));
    if(position){const point=windowPoint(position,windowBounds(space,element.getBoundingClientRect()));write(element,'--sc-window-x',Math.round(point.x)+'px');write(element,'--sc-window-y',Math.round(point.y)+'px');}
    else{element.style.removeProperty('--sc-window-x');element.style.removeProperty('--sc-window-y');}
  }
  function refresh(){for(const id of windows.keys())apply(id);invalidate();}
  function updateSize(id,width,height){
    const space=area(),sizes={...preferences().sizes,[id]:{width:clamp(width,280,Math.min(760,space.width-30)),height:clamp(height,240,Math.min(800,space.height-space.header-102))}};
    change({sizes},false);apply(id);invalidate();
  }
  function updatePoint(id,x,y){const element=windows.get(id),position=windowFraction({x,y},windowBounds(area(),element.getBoundingClientRect()));change({positions:{...preferences().positions,[id]:position}},false);apply(id);invalidate();}
  function resetPosition(id){const positions={...preferences().positions};delete positions[id];change({positions});apply(id);invalidate();}
  function attach(id,element){
    windows.set(id,element);element.dataset.windowId=id;
    const top=element.querySelector('.sc-panel-top');let handle=top.querySelector('.sc-window-handle');
    if(!handle){handle=document.createElement('button');handle.type='button';handle.className='sc-window-handle sc-panel-move';uiText(handle, '⠿');uiChildren(top, "prepend", handle);}
    uiAttribute(handle, 'aria-label', ui("Переместить окно"));uiAttribute(handle, "data-tooltip", ui("Потяните заголовок. Стрелки перемещают окно; Home возвращает его на место."));
    const size=document.createElement('button');size.type='button';size.className='sc-panel-size';uiText(size, '↔');uiAttribute(size, 'aria-label', ui("Изменить размер окна"));uiAttribute(size, "data-tooltip", ui("Размер окна: обычный / просторный"));
    size.addEventListener('click',()=>{onUserAction();const current=preferences().sizes[id];updateSize(id,current?.width>=560?420:640,current?.width>=560?440:620);change({});});top.insertBefore(size,top.lastElementChild);
    handle.addEventListener('keydown',event=>{
      const direction={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[event.key];if(!direction&&event.key!=='Home')return;
      event.preventDefault();event.stopPropagation();onUserAction();
      if(event.key==='Home')resetPosition(id);else{const origin=root.getBoundingClientRect(),box=element.getBoundingClientRect(),step=event.shiftKey?40:16;updatePoint(id,box.left-origin.left+direction[0]*step,box.top-origin.top+direction[1]*step);change({});}
    });
    function begin(event,edge='move'){
      if(event.button!==0||gesture)return;
      if(edge==='move'&&event.target.closest('input,select,a,textarea,button')!==handle&&event.target.closest('input,select,a,textarea,button'))return;
      onUserAction();const box=element.getBoundingClientRect(),origin=root.getBoundingClientRect();gesture={id,eventId:event.pointerId,edge,x:event.clientX,y:event.clientY,left:box.left-origin.left,top:box.top-origin.top,width:box.width,height:box.height};
      element.setPointerCapture(event.pointerId);event.preventDefault();event.stopPropagation();
    }
    top.addEventListener('pointerdown',event=>begin(event));
    for(const edge of ['n','e','s','w','nw','ne','sw','se']){
      const grip=document.createElement(edge==='se'?'button':'div');grip.className='sc-window-edge sc-window-edge-'+edge;grip.dataset.edge=edge;
      if(edge==='se'){
        grip.type='button';grip.classList.add('sc-panel-resize');uiText(grip, '⌟');uiAttribute(grip, 'aria-label', ui("Размер окна: стрелки меняют ширину и высоту"));uiAttribute(grip, "data-tooltip", ui("Потяните край или угол. Стрелки меняют размер; Home сбрасывает."));
        grip.addEventListener('keydown',event=>{const delta={ArrowLeft:[-20,0],ArrowRight:[20,0],ArrowUp:[0,-20],ArrowDown:[0,20]}[event.key];if(!delta&&event.key!=='Home')return;event.preventDefault();event.stopPropagation();onUserAction();if(delta){const box=element.getBoundingClientRect();updateSize(id,box.width+delta[0],box.height+delta[1]);change({});}else{const sizes={...preferences().sizes};delete sizes[id];change({sizes});apply(id);invalidate();}});
      }else uiAttribute(grip, 'aria-hidden', 'true');
      grip.addEventListener('pointerdown',event=>begin(event,edge));uiChildren(element, "append", grip);
    }
    element.addEventListener('pointermove',event=>{
      const g=gesture;if(g?.id!==id||g.eventId!==event.pointerId)return;const dx=event.clientX-g.x,dy=event.clientY-g.y;
      if(g.edge==='move')updatePoint(id,g.left+dx,g.top+dy);
      else{
        const west=g.edge.includes('w'),north=g.edge.includes('n'),horizontal=west||g.edge.includes('e'),vertical=north||g.edge.includes('s');
        // Freeze the origin when resizing a docked window. Opposite edges stay
        // anchored, including after reaching the minimum or viewport bound.
        updatePoint(id,g.left,g.top);updateSize(id,g.width+(horizontal?dx*(west?-1:1):0),g.height+(vertical?dy*(north?-1:1):0));
        const box=element.getBoundingClientRect();updatePoint(id,g.left+(west?g.width-box.width:0),g.top+(north?g.height-box.height:0));
      }
      event.preventDefault();event.stopPropagation();
    });
    function release(event){if(gesture?.id!==id||gesture.eventId!==event.pointerId)return;gesture=null;if(element.hasPointerCapture(event.pointerId))element.releasePointerCapture(event.pointerId);change({});}
    element.addEventListener('pointerup',release);element.addEventListener('pointercancel',release);element.addEventListener('lostpointercapture',release);
    apply(id);
  }
  const observer=new ResizeObserver(refresh);observer.observe(root);
  return {attach,apply,refresh};
}
