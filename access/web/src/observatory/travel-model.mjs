import {validatePlace} from './place-model.mjs';

export const HISTORY_KEY='tos-observatory-history-v1';
export const HISTORY_LIMIT=100;
const MAX_TEXT=1200000;
const empty=()=>({v:1,entries:[],cursor:-1});
const bad=()=>{throw new Error('Историю не удалось прочитать. Сохранённая запись осталась в браузере.');};
function historyEntry(value){const entry=validatePlace(value);if(!Number.isFinite(new Date(entry.savedAt).getTime()))bad();return entry;}
export function validateHistory(value){
  if(value?.v!==1||!Array.isArray(value.entries)||value.entries.length>HISTORY_LIMIT||!Number.isInteger(value.cursor)
    ||value.cursor<(value.entries.length?0:-1)||value.cursor>=value.entries.length)bad();
  const entries=value.entries.map(historyEntry);
  if(new Set(entries.map(e=>e.id)).size!==entries.length)bad();
  const result={v:1,entries,cursor:value.cursor};if(JSON.stringify(result).length>MAX_TEXT)bad();return result;
}
// Compare only a bounded request and local pose. Panel visibility is transient;
// opening a tool must not manufacture a new journey step.
export function travelKey(place){
  const {panelOpen,...pose}=place.pose;
  return JSON.stringify([place.sourceRevision,place.spec,place.draft,pose]);
}
export function historyLabel(previous,next,title){
  const short=String(title||'область').slice(0,42);
  if(!previous)return ('Начало: '+short).slice(0,64);
  if(JSON.stringify(previous.draft)!==JSON.stringify(next.draft))return 'Изменена линза';
  if(JSON.stringify(previous.spec)!==JSON.stringify(next.spec))return ('Открыта область: '+short).slice(0,64);
  if(next.pose.relationId!==previous.pose.relationId&&next.pose.relationId)return ('Выбрана связь: '+short).slice(0,64);
  if(next.pose.selectedId!==previous.pose.selectedId)return next.pose.selectedId?('Выбрано: '+short).slice(0,64):'Общий вид';
  if(next.pose.lens!==previous.pose.lens)return 'Линза: '+({plane:'Карта связей',orbits:'Орбиты мысли',constellations:'Созвездия мысли'})[next.pose.lens];
  if(next.pose.cardTab!==previous.pose.cardTab)return next.pose.cardTab==='relations'?'Открыты связи':'Открыт смысл';
  if(next.sourceRevision!==previous.sourceRevision)return 'Обновлена область';
  return 'Изменён ракурс';
}
export function createTravelStore(storage,key=HISTORY_KEY){
  let state=empty(),baseline=null,error='',writable=true;
  try{baseline=storage?.getItem(key)||null;if(baseline){if(baseline.length>MAX_TEXT)bad();state=validateHistory(JSON.parse(baseline));}}
  catch(cause){error=cause.message;writable=false;}
  function save(){
    if(!storage){error='История действует до закрытия страницы: хранилище недоступно.';return false;}
    if(!writable)return false;
    try{
      if(storage.getItem(key)!==baseline){writable=false;throw new Error('История изменилась в другой вкладке. Новые шаги этой вкладки пока не сохранены; откройте страницу заново, чтобы загрузить общую историю.');}
      const text=JSON.stringify(state);storage.setItem(key,text);baseline=text;error='';return true;
    }catch(cause){error=cause.message||'Браузер не сохранил историю.';return false;}
  }
  return {
    get state(){return structuredClone(state);},get error(){return error;},save,
    record(place){
      const entry=historyEntry(place),current=state.entries[state.cursor];
      if(current&&travelKey(current)===travelKey(entry))return false;
      state.entries=state.entries.slice(0,state.cursor+1);state.entries.push(entry);state.cursor=state.entries.length-1;
      while(state.entries.length>HISTORY_LIMIT||JSON.stringify(state).length>MAX_TEXT){state.entries.shift();state.cursor--;}
      return true;
    },
    move(id){const index=state.entries.findIndex(e=>e.id===id);if(index<0)return false;state.cursor=index;return true;},
    clear(place){state=empty();if(place)state={v:1,entries:[historyEntry(place)],cursor:0};writable=true;try{baseline=storage?.getItem(key)||null;}catch{writable=false;}return save();},
    resume(route){const entry=state.entries[state.cursor];return entry&&(!route||route===entry.route)?structuredClone(entry):null;},
  };
}

// Navigation commits both the view and cursor only after a current successful
// request. A newer intent invalidates even a transport that ignores abort.
export function createTravelNavigation({store,reopen,apply,onBusy=()=>{}}){
  let generation=0,controller=null;
  function cancel(){generation++;controller?.abort();controller=null;onBusy(false);}
  return {cancel,async go(id){
    cancel();const entry=store.state.entries.find(e=>e.id===id);if(!entry)return {current:false};
    const turn=generation;controller=new AbortController();onBusy(true);
    try{
      const value=await reopen(entry,controller.signal);
      if(turn!==generation)return {current:false};
      apply(value);store.move(id);return {current:true,value};
    }catch(error){if(turn!==generation)return {current:false};throw error;}
    finally{if(turn===generation){controller=null;onBusy(false);}}
  }};
}
