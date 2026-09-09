import {ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
import './context-hints.css';

// One transient explanation, shared by tools and graph elements. It never
// changes selection, focuses a control, or captures a pointer gesture.
export function createContextHints(root,{delay=400}={}){
  const hint=document.createElement('div');hint.id='sc-context-hint';hint.className='sc-context-hint';uiAttribute(hint, 'role', 'tooltip');hint.hidden=true;uiChildren(root, "append", hint);
  let active=null,activeText='',pending=null,timer=null,leaveTimer=null,held=false,quietUntil=0,dismissed=null;
  const target=node=>node?.closest?.('[data-tooltip]');
  function hide(){
    clearTimeout(timer);clearTimeout(leaveTimer);timer=null;pending=null;
    if(active){const ids=(active.getAttribute('aria-describedby')||'').split(/\s+/).filter(id=>id&&id!==hint.id);if(ids.length)uiAttribute(active, 'aria-describedby', ids.join(' '));else active.removeAttribute('aria-describedby');}
    active=null;hint.hidden=true;
  }
  function show(anchor){
    if(held||performance.now()<quietUntil||anchor===dismissed||!anchor?.isConnected||anchor.closest('[hidden]')||!anchor.dataset.tooltip?.trim())return;
    hide();active=anchor;activeText=anchor.dataset.tooltip;uiChildren(hint, "replaceChildren");
    if(anchor.dataset.tooltipTitle){
      for(const key of ['kind','title','body']){const text=anchor.dataset['tooltip'+key[0].toUpperCase()+key.slice(1)];if(!text)continue;const part=document.createElement(key==='title'?'strong':'span');part.className='sc-hint-'+key;uiText(part, text);uiChildren(hint, "append", part);}
    }else uiText(hint, anchor.dataset.tooltip);
    hint.hidden=false;
    const ids=new Set((anchor.getAttribute('aria-describedby')||'').split(/\s+/).filter(Boolean));ids.add(hint.id);uiAttribute(anchor, 'aria-describedby', [...ids].join(' '));
    const area=root.getBoundingClientRect(),box=anchor.dataset.tooltipPoint?JSON.parse(anchor.dataset.tooltipPoint):anchor.getBoundingClientRect(),size=hint.getBoundingClientRect(),scaleX=area.width/root.offsetWidth||1,scaleY=area.height/root.offsetHeight||1;
    const left=Math.max(0,area.left)+12,right=Math.min(innerWidth,area.right)-12,top=Math.max(0,area.top)+12,bottom=Math.min(innerHeight,area.bottom)-12;
    const x=Math.max(left,Math.min(box.left+(box.width-size.width)/2,right-size.width));
    const y=box.bottom+10+size.height<=bottom?box.bottom+10:Math.max(top,box.top-size.height-10);
    hint.style.left=(x-area.left)/scaleX+'px';hint.style.top=(y-area.top)/scaleY+'px';
  }
  function schedule(anchor,immediate=false){
    clearTimeout(leaveTimer);if(!anchor||anchor===active||anchor===pending||anchor===dismissed||held||performance.now()<quietUntil)return;
    hide();pending=anchor;if(immediate)show(anchor);else timer=setTimeout(()=>show(anchor),delay);
  }
  root.addEventListener('sophia-context-target',event=>{dismissed=null;if(event.detail?.anchor)schedule(event.detail.anchor);else hide();});
  root.addEventListener('pointerover',event=>{if(event.pointerType==='touch')return;if(hint.contains(event.target)){clearTimeout(leaveTimer);return;}schedule(target(event.target));});
  root.addEventListener('pointerout',event=>{
    const anchor=target(event.target);if(anchor?.contains(event.relatedTarget)||hint.contains(event.relatedTarget))return;
    if(dismissed===anchor)dismissed=null;
    if(anchor===active||anchor===pending||hint.contains(event.target)){if(active?.contains(document.activeElement))return;clearTimeout(leaveTimer);leaveTimer=setTimeout(hide,160);}
  });
  root.addEventListener('focusin',event=>schedule(target(event.target),true));
  root.addEventListener('focusout',event=>{if(!active?.contains(event.relatedTarget)){dismissed=null;hide();}});
  root.addEventListener('pointerdown',()=>{held=true;hide();},{capture:true});
  window.addEventListener('pointerup',()=>held=false);window.addEventListener('pointercancel',()=>held=false);
  root.addEventListener('click',hide);
  root.addEventListener('wheel',()=>{quietUntil=performance.now()+500;hide();},{passive:true});
  root.addEventListener('gesturechange',()=>{quietUntil=performance.now()+500;hide();},{passive:true});
  root.addEventListener('scroll',hide,{capture:true,passive:true});
  window.addEventListener('keydown',event=>{if(event.key==='Escape'&&(active||pending)){dismissed=active||pending;hide();event.preventDefault();event.stopPropagation();}},{capture:true});
  window.addEventListener('resize',hide);window.addEventListener('blur',()=>{held=false;hide();});
  document.addEventListener('visibilitychange',()=>{if(document.hidden)hide();});
  const observer=new MutationObserver(()=>{if(active&&(!active.isConnected||active.closest('[hidden]')||active.dataset.tooltip!==activeText))hide();});
  observer.observe(root,{subtree:true,childList:true,attributes:true,attributeFilter:['hidden','data-tooltip']});
  window.addEventListener('pagehide',hide);return {hide};
}

export function describeControls(root){
  const descriptions={
    '.sc-search-open':ui("Найти материал по имени, названию или понятию. Клавиша / открывает поиск."),
    '.sc-lenses-open':ui("Выбрать расположение звёзд: созвездия, орбиты или плоскость."),
    '.sc-workspace-open':ui("Заметки, гипотезы и источники текущего исследования."),
    '.sc-navigation-open':ui("Найти путь между звёздами и исследовать их связи."),
    '.sc-reader-open':ui("Читать оставленные материалы рядом и возвращаться к месту остановки."),
    '.sc-reader-resume':ui("Вернуться к оставленной паре материалов и позициям чтения."),
    '.sc-studio-open':ui("Места, сохранённые для возвращения, и все инструменты исследования."),
    '.sc-open-neighborhood':ui("Загрузить область вокруг выбранной звезды и её связи."),
    '.sc-read-selected':ui("Оставить материал для чтения. Можно сопоставить два материала."),
    '.sc-focus':ui("Приблизить выбранную звезду и расположить рядом её карточку."),
    '.sc-overview':ui("Вернуться к общей композиции этой области."),
    '.sc-lens[data-lens=constellations]':ui("Расположить звёзды свободно в объёмном пространстве."),
    '.sc-lens[data-lens=orbits]':ui("Собрать окружение выбранной звезды в орбиты."),
    '.sc-lens[data-lens=plane]':ui("Развернуть звёзды на плоскости, чтобы легче читать связи."),
    '.sc-builder-open':ui("Собрать собственную область по материалам, условиям и связям."),
    '.sc-studio-copy':ui("Сохранить историю, места, линзы, заметки, настройки и состояние чтения в один файл."),
  };
  for(const [selector,text]of Object.entries(descriptions))for(const element of root.querySelectorAll(selector))uiAttribute(element, "data-tooltip", text);
}
