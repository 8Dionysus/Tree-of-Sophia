import {t} from './ui-i18n.mjs';
import {validatePlace} from './place-model.mjs';

export const HISTORY_KEY='tos-observatory-history-v1';
export const HISTORY_LIMIT=100;
const MAX_TEXT=1200000;
const empty=()=>({v:1,entries:[],cursor:-1});
const bad=()=>{throw new Error(t("Историю не удалось прочитать. Сохранённая запись осталась в браузере."));};
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
// A journey follows materials and queries. Camera, card tabs and refreshed
// data belong to the current stop and must not consume the forward branch.
export function journeyKey(place){
  return JSON.stringify([place.spec,place.draft,place.pose.lens,place.pose.selectedId,place.pose.relationId]);
}
const legacyName=name=>/^(Измен[её]н ракурс|Открыт смысл|Открыты связи|Обновлена область)$/.test(name);
function fallbackName(entry){
  const name=entry.draft?.name||entry.spec.title;
  return (typeof name==='string'&&name.trim()?name:t("Область исследования")).slice(0,64);
}
export function compactHistory(value){
  const state=validateHistory(value),entries=[];let cursor=-1;
  for(let i=0;i<state.entries.length;i++){
    const entry=state.entries[i],previous=entries.at(-1);
    if(previous&&journeyKey(previous)===journeyKey(entry)){
      // Keep the cursor's exact pose if it lies inside a collapsed run.
      if(i<=state.cursor||cursor!==entries.length-1)entries[entries.length-1]={...entry,id:previous.id,savedAt:previous.savedAt,name:previous.name};
    }else entries.push({...entry,name:legacyName(entry.name)?fallbackName(entry):entry.name.replace(/^(Начало|Выбрано|Открыта область): /,'')||entry.name});
    if(i===state.cursor)cursor=entries.length-1;
  }
  return {v:1,entries,cursor};
}
export function historyLabel(previous,next,title){
  const short=String(title||t("Область исследования")).slice(0,64);
  if(!previous)return short;
  if(JSON.stringify(previous.draft)!==JSON.stringify(next.draft))return (t("Линза: {0}", [(next.draft?.name||short)])).slice(0,64);
  if(JSON.stringify(previous.spec)!==JSON.stringify(next.spec))return short;
  if(next.pose.relationId!==previous.pose.relationId&&next.pose.relationId)return (t("Связь: {0}", [short])).slice(0,64);
  if(next.pose.selectedId!==previous.pose.selectedId)return next.pose.selectedId?short:t("Общий вид");
  if(next.pose.lens!==previous.pose.lens)return t("Линза: {0}", [({plane:t("Карта связей"),orbits:t("Орбиты мысли"),constellations:t("Созвездия мысли")})[next.pose.lens]]);
  return previous.name==='Область исследования'||legacyName(previous.name)?short:previous.name;
}
export function createTravelStore(storage,key=HISTORY_KEY){
  let state=empty(),baseline=null,error='',writable=true;
  try{baseline=storage?.getItem(key)||null;if(baseline){if(baseline.length>MAX_TEXT)bad();state=compactHistory(JSON.parse(baseline));}}
  catch(cause){error=cause.message;writable=false;}
  function save(){
    if(!storage){error=t("История действует до закрытия страницы: хранилище недоступно.");return false;}
    if(!writable)return false;
    try{
      if(storage.getItem(key)!==baseline){writable=false;throw new Error(t("История изменилась в другой вкладке. Новые шаги этой вкладки пока не сохранены; откройте страницу заново, чтобы загрузить общую историю."));}
      const text=JSON.stringify(state);storage.setItem(key,text);baseline=text;error='';return true;
    }catch(cause){error=cause.message||t("Браузер не сохранил историю.");return false;}
  }
  function trim(){
    while(state.entries.length>HISTORY_LIMIT||JSON.stringify(state).length>MAX_TEXT){
      if(state.cursor>0){state.entries.shift();state.cursor--;}else state.entries.pop();
    }
  }
  if(baseline&&writable&&JSON.stringify(state)!==baseline)save();
  return {
    get state(){return structuredClone(state);},get error(){return error;},save,
    record(place){
      const entry=historyEntry(place),current=state.entries[state.cursor];
      if(current&&travelKey(current)===travelKey(entry)&&current.name===entry.name)return false;
      if(current&&journeyKey(current)===journeyKey(entry)){
        state.entries[state.cursor]={...entry,id:current.id,savedAt:current.savedAt,name:entry.name};trim();return true;
      }
      state.entries=state.entries.slice(0,state.cursor+1);state.entries.push(entry);state.cursor=state.entries.length-1;
      trim();
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
