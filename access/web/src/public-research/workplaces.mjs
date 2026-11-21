// Workplaces (F3): the arrangement of one piece of work kept under a key. A workplace is the mode, the lens, the
// presence layer, the route and its step, the chosen star or link and the workbench (open, and which bench); Alt with
// a digit returns it, Alt and Shift with the digit keeps the present arrangement there. Nine places per stand, kept in
// this browser for this device; the work itself stays where it lives (the workspace, the bench, the act journal).
import './act-desk.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {activateBench} from './bench/model.mjs';

export const WORKPLACES_KEY='tos.public-research.workplaces.v1';
export const MAX_WORKPLACES=9;
const MODES=new Set(['original','reference','cognitive']);
const LAYERS=new Set(['reading','research','agents']);
const ID=/^[^\u0000-\u001f]{1,512}$/;

const str=value=>typeof value==='string'&&ID.test(value)?value:null;

/** A workplace as kept: every part checked, an unknown part left out. */
export function normalizeWorkplace(value){
  if(!value||typeof value!=='object')return null;
  const route=value.route&&str(value.route.id)?{id:value.route.id,step:Number.isSafeInteger(value.route.step)&&value.route.step>0?value.route.step:1}:null;
  const selection=value.selection&&str(value.selection.id)?{id:value.selection.id,kind:value.selection.kind==='relation'?'relation':'node'}:null;
  return {
    name:typeof value.name==='string'?value.name.trim().slice(0,80):'',
    at:typeof value.at==='string'?value.at.slice(0,40):'',
    mode:MODES.has(value.mode)?value.mode:'original',
    lens:str(value.lens),layer:LAYERS.has(value.layer)?value.layer:'reading',route,selection,
    bench:{open:value.bench?.open===true,active:str(value.bench?.active)},
  };
}

/** The present arrangement of the page. */
export function captureWorkplace({actionLayer,space,experience,name=''}){
  const view=actionLayer?.context?.()??{},chosen=space?.selection?.()??null,benchView=space?.bench?.()??null;
  return normalizeWorkplace({name,at:new Date().toISOString(),mode:experience?.getState?.().mode,lens:view.lens,layer:view.presence_layer,
    route:view.route?{id:view.route.id,step:view.route.step}:null,selection:chosen&&!view.route?{id:chosen.id,kind:chosen.kind}:null,
    bench:{open:Boolean(benchView?.isOpen?.()),active:benchView?.store?.state?.active??null}});
}

/** Return an arrangement: mode, lens, layer, then the route or the chosen subject, then the workbench. */
export async function applyWorkplace(place,{actionLayer,space,experience,registry=null}){
  const value=normalizeWorkplace(place);if(!value)return false;
  if(experience?.getState?.().mode!==value.mode)experience?.setMode?.(value.mode);
  if(value.lens&&space?.actions?.lens?.()!==value.lens)space?.actions?.setLens?.(value.lens);
  if(actionLayer?.layer?.()!==value.layer)actionLayer?.setLayer?.(value.layer);
  if(value.route)space?.actions?.startRoute?.(value.route.id,value.route.step);
  else if(value.selection){try{if(registry)await registry.invoke('tos.page.select',{item_id:value.selection.id});else experience?.open?.(value.selection.id);}catch{}}
  const benchView=space?.bench?.();
  if(benchView){
    if(value.bench.active&&benchView.store?.state?.active!==value.bench.active)await benchView.store.update(state=>activateBench(state,value.bench.active));
    if(value.bench.open&&!benchView.isOpen?.())space.openBench?.();else if(!value.bench.open&&benchView.isOpen?.())benchView.hide?.();
  }
  return true;
}

/** The nine places of a stand in this browser. */
export function createWorkplaceStore({storage=globalThis.localStorage,profile='zarathustra'}={}){
  const key=`${WORKPLACES_KEY}.${profile}`;
  const read=()=>{try{const value=JSON.parse(storage?.getItem(key)??'[]');return Array.from({length:MAX_WORKPLACES},(_,i)=>normalizeWorkplace(Array.isArray(value)?value[i]:null));}catch{return Array(MAX_WORKPLACES).fill(null);}};
  const write=list=>{try{storage?.setItem(key,JSON.stringify(list));return true;}catch{return false;}};
  return {
    list:read,
    get:slot=>read()[slot-1]??null,
    keep(slot,place){if(slot<1||slot>MAX_WORKPLACES)return false;const list=read();list[slot-1]=normalizeWorkplace(place);return write(list);},
    rename(slot,name){const list=read(),place=list[slot-1];if(!place)return false;list[slot-1]={...place,name:String(name).trim().slice(0,80)};return write(list);},
    clear(slot){const list=read();list[slot-1]=null;return write(list);},
  };
}

const labels={
  places:['Рабочие места','Workplaces'],
  lead:['Расстановка работы под клавишей: режим, линза, слой, маршрут и шаг, выбранное и верстак. Alt и цифра возвращают место, Alt, Shift и цифра записывают в него нынешнюю расстановку.','An arrangement of work under a key: the mode, lens, layer, route and step, what is chosen and the workbench. Alt and a digit return a place; Alt, Shift and the digit keep the present arrangement there.'],
  place:['Место','Place'],
  empty:['пусто','empty'],
  restore:['Вернуть','Return'],
  keep:['Записать сюда','Keep here'],
  clear:['Стереть','Clear'],
  name:['Название','Name'],
  returned:['Рабочее место возвращено','Workplace returned'],
  kept:['Рабочее место записано','Workplace kept'],
  benchOpen:['верстак открыт','workbench open'],
  close:['Закрыть','Close'],
};
const modeLabels={original:['Оригинальный','Original'],reference:['Справочный','Reference'],cognitive:['Познавательный','Learning']};
const layerLabels={reading:['Чтение','Reading'],research:['Исследование','Research'],agents:['С агентами','With agents']};

export function mountWorkplaces({shell,actionLayer,space,experience=null,registry=null,locale=()=> 'ru',profile='zarathustra',storage=globalThis.localStorage}){
  const store=createWorkplaceStore({storage,profile});
  const ru=()=>resolveLocale(locale())==='ru';
  const pick=pair=>ru()?pair[0]:localeText(pair[1],locale());
  const t=key=>labels[key]?pick(labels[key]):key;
  const doc=shell.ownerDocument,view=doc.defaultView;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const context={actionLayer,space,experience,registry};
  const named=(slot,place)=>place?.name||`${t('place')} ${slot}`;
  function summary(place){
    const lens=space?.actions?.lenses?.().find(item=>item.id===place.lens);
    const title=id=>{const item=space?.index?.get?.(id);return item?localeValue(item.title,locale()):id;};
    const route=place.route?space?.actions?.routes?.().find(item=>item.id===place.route.id):null;
    return [pick(modeLabels[place.mode]),lens?localeValue(lens.title,locale()):'',pick(layerLabels[place.layer]),
      route?`${localeValue(route.title,locale())} · ${place.route.step}`:'',place.selection?title(place.selection.id):'',place.bench.open?t('benchOpen'):''].filter(Boolean).join(' · ');
  }
  async function recall(slot){const place=store.get(slot);if(!place)return false;await applyWorkplace(place,context);actionLayer?.say?.(`${t('returned')}: ${named(slot,place)}`);return true;}
  function keep(slot,name=null){const before=store.get(slot);const place=captureWorkplace({...context,name:name??before?.name??''});store.keep(slot,place);actionLayer?.say?.(`${t('kept')}: ${named(slot,place)}`);return place;}

  const dialogs=[];
  function open(){
    const dialog=el('dialog','ad-sheet wp-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',t('places')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    const paint=()=>{
      body.replaceChildren(el('p','ad-muted',t('lead')));
      const list=store.list();
      for(let slot=1;slot<=MAX_WORKPLACES;slot++){
        const place=list[slot-1],row=el('section','ad-group wp-row');row.dataset.slot=String(slot);
        const top=el('div','ad-row');top.append(el('kbd','wp-key',`Alt ${slot}`));
        const name=el('input','ad-input wp-name');name.value=place?.name??'';name.placeholder=`${t('place')} ${slot}`;name.setAttribute('aria-label',`${t('name')} · ${slot}`);
        name.addEventListener('change',()=>{if(store.get(slot))store.rename(slot,name.value);});
        top.append(name);row.append(top,el('p','ad-muted',place?summary(place):t('empty')));
        const actions=el('div','ad-row');
        if(place){const back=button(t('restore'),'primary');back.addEventListener('click',()=>{dialog.close();void recall(slot);});actions.append(back);}
        const here=button(t('keep'),'text-button');here.addEventListener('click',()=>{keep(slot,name.value);paint();});actions.append(here);
        if(place){const wipe=button(t('clear'),'text-button');wipe.addEventListener('click',()=>{store.clear(slot);paint();});actions.append(wipe);}
        row.append(actions);body.append(row);
      }
    };
    paint();dialog.showModal?.();return dialog;
  }

  // Alt with a digit returns a place; with Shift it keeps the present arrangement there; Alt and 0 lists them.
  const onKey=event=>{
    if(!event.altKey||event.ctrlKey||event.metaKey||event.defaultPrevented)return;
    if(event.target?.closest?.('input,textarea,select,[contenteditable="true"]'))return;
    const match=/^Digit([0-9])$/.exec(event.code??'');if(!match)return;
    const slot=Number(match[1]);event.preventDefault();
    if(slot===0){open();return;}
    if(event.shiftKey)keep(slot);else void recall(slot);
  };
  view?.addEventListener('keydown',onKey,{capture:true});
  const remove=actionLayer?.addActions?.([
    {id:'workplaces',text:()=>t('places'),key:()=> 'Alt 0',for:['any'],run:()=>open()},
    ...Array.from({length:MAX_WORKPLACES},(_,i)=>({id:`workplace-${i+1}`,text:()=>`${t('place')} ${i+1}: ${named(i+1,store.get(i+1))}`,key:()=>`Alt ${i+1}`,for:['any'],when:()=>Boolean(store.get(i+1)),run:()=>recall(i+1)})),
  ]);
  return {open,recall,keep,store,destroy(){view?.removeEventListener('keydown',onKey,{capture:true});remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
