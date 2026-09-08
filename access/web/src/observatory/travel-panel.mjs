import {capturePlace,reopenPlace,pinHistoryPlace} from './place-model.mjs';
import {localized} from './knowledge-client.mjs';
import {createTravelStore,createTravelNavigation,historyLabel,travelKey,HISTORY_KEY,HISTORY_LIMIT} from './travel-model.mjs';
import './travel.css';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(text,action,className='')=>{const node=el('button',text,className);node.type='button';node.addEventListener('click',action);return node;};
export function createTravelPanel(root,scene,panels,{client,onUserAction=()=>{}}){
  let storage=null;try{storage=localStorage;}catch{}
  // Different mounted products/fixtures do not share a journey accidentally.
  const store=createTravelStore(storage,HISTORY_KEY+':'+location.pathname);
  const formatTime=new Intl.DateTimeFormat('ru',{day:'numeric',month:'short',hour:'2-digit',minute:'2-digit'});
  let started=false,applying=false,busy=false,pending=false,saveTimer=null,gestureTimer=null,baseline=null,lastPlace=null,message='',failure='',retryId=null,replacing=false;
  {const {entries,cursor}=store.state;lastPlace=entries[cursor]||null;}
  const back=root.querySelector('.sc-back');back.hidden=false;back.disabled=true;
  back.setAttribute('aria-label','Назад по истории');
  const forward=button('Вперёд →',()=>void step(1),'sc-travel-button sc-forward');forward.setAttribute('aria-label','Вперёд по истории');
  const opener=button('История ▾',()=>{onUserAction();if(!panel.hidden){close();return;}flush();panels.open('history');render();opener.setAttribute('aria-expanded','true');list.querySelector('[aria-current=step]')?.focus();},'sc-travel-button sc-history-open');
  opener.setAttribute('aria-expanded','false');opener.setAttribute('aria-controls','sc-history');
  const controls=el('nav','','sc-travel-controls');controls.setAttribute('aria-label','История пространства');back.before(controls);controls.append(back,forward,opener);
  const panel=el('section','','sc-panel sc-history');panel.id='sc-history';panel.hidden=true;panel.setAttribute('aria-label','История пространства');
  const top=el('div','','sc-panel-top');top.append(el('span','ИСТОРИЯ','sc-eyebrow'),button('×',close,'sc-icon sc-history-close'));top.lastChild.setAttribute('aria-label','Закрыть историю');
  const list=el('ol','','sc-history-list'),status=el('p','','sc-history-status');status.setAttribute('role','status');
  const retry=button('Повторить переход',()=>void go(retryId),'sc-history-retry');retry.hidden=true;
  const save=button('Повторить сохранение',()=>{store.save();render();},'sc-history-save');save.hidden=true;
  const clear=button('Очистить историю',()=>{onUserAction();navigator.cancel();clearTimeout(saveTimer);flush();store.clear(lastPlace);message='Оставлен только текущий вид.';failure='';retryId=null;render();},'sc-history-clear');
  panel.append(top,list,status,retry,save,el('p',`До ${HISTORY_LIMIT} переходов в этом браузере. Положение камеры запоминается внутри шага.`,'sc-history-note'),clear);root.append(panel);
  root.addEventListener('sophia-history-open',()=>opener.click());
  panels.register('history',panel,()=>{opener.setAttribute('aria-expanded','false');if(!applying)cancel();});
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  function close(){panels.close('history');opener.focus();}
  function render(){
    const {entries,cursor}=store.state;
    root.dataset.history=String(entries.length);root.dataset.historyCursor=String(cursor);root.dataset.historyBusy=String(busy);
    back.disabled=cursor<=0;forward.disabled=cursor<0||cursor>=entries.length-1;
    back.dataset.tooltip=back.disabled?'Это начало сохранённого пути.':'Предыдущий материал или область. Alt + ←.';
    forward.dataset.tooltip=forward.disabled?'Дальше сохранённых переходов нет.':'Следующий материал или область. Alt + →.';
    opener.textContent='История'+(entries.length?' · '+(cursor+1)+'/'+entries.length:'')+' ▾';
    opener.dataset.unsaved=String(Boolean(store.error));opener.dataset.tooltip=store.error||'Вернуться к материалу или области исследования.';
    panel.setAttribute('aria-busy',String(busy));status.textContent=failure||store.error||message;
    retry.hidden=!retryId||!failure;retry.disabled=busy;save.hidden=!store.error;clear.disabled=busy;
    if(panel.hidden)return;
    const focused=document.activeElement?.dataset.historyId,scroll=list.scrollTop;list.replaceChildren();
    for(let index=entries.length-1;index>=0;index--){const entry=entries[index],row=el('li');
      const open=button('',()=>void go(entry.id));open.dataset.historyId=entry.id;open.dataset.tooltip='Открыть этот материал и восстановить положение пространства.';
      if(index===cursor)open.setAttribute('aria-current','step');
      const time=el('time',formatTime.format(entry.savedAt));time.dateTime=new Date(entry.savedAt).toISOString();
      open.append(el('span',entry.name),time);row.append(open);list.append(row);
      const pin=button('☆ В места',()=>{
        onUserAction();try{if(!storage)throw new Error('Локальное хранилище недоступно.');const place=pinHistoryPlace(storage,entry);
          root.dispatchEvent(new CustomEvent('sophia-place-saved',{detail:place}));failure='';message='Место «'+place.name+'» сохранено. Название можно изменить в «Моём пространстве».';render();
        }catch(error){failure=error.message;render();}
      },'sc-history-pin');pin.dataset.tooltip='Сохранить область, линзу и положение камеры для возвращения. Название можно изменить в местах.';pin.setAttribute('aria-label','Сохранить место: '+entry.name);pin.disabled=busy;row.append(pin);
    }
    list.scrollTop=scroll;if(focused)[...list.querySelectorAll('button')].find(b=>b.dataset.historyId===focused)?.focus();scene.invalidate();
  }
  function capture(){
    const packet=scene.port.packet;if(!packet?.nodes.length)return null;
    const pose=scene.port.capturePlace(),raw=pose.relationId?scene.port.relation(pose.relationId):scene.port.node(pose.selectedId||packet.focus?.node_id);
    const title=localized(raw?.display?.title||raw?.display?.label,'область');
    const value=capturePlace(packet,pose,{name:'Шаг',id:crypto.randomUUID(),route:location.search});
    value.name=historyLabel(lastPlace,value,title);return value;
  }
  function saveLater(){if(replacing)return;clearTimeout(saveTimer);saveTimer=setTimeout(()=>{store.save();render();},700);}
  function flush(){
    if(!started||applying||busy)return;
    pending=false;
    try{const value=capture();if(!value)return;const key=travelKey(value);if(key===baseline&&value.name===store.state.entries[store.state.cursor]?.name)return;
      baseline=key;store.record(value);lastPlace=store.state.entries[store.state.cursor]||value;saveLater();render();
    }catch(error){failure=error.message;render();}
  }
  function observe(){if(!started||applying||busy||pending)return;pending=true;queueMicrotask(()=>{if(pending)flush();});}
  function cancel(){if(!busy)return;navigator.cancel();retryId=null;failure='';message='Переход прерван вашим действием.';render();}
  const navigator=createTravelNavigation({store,reopen:(entry,signal)=>reopenPlace(client,entry,signal),
    onBusy:value=>{busy=value;render();},apply:value=>{
      applying=true;try{panels.close('history');scene.ui.cancelPending();scene.port.setGraph(value.packet,{initial:true});scene.port.restorePlace(value.pose);lastPlace=capture();baseline=travelKey(lastPlace);}finally{applying=false;}
    }});
  async function go(id){
    if(!id)return;flush();onUserAction();scene.ui.cancelPending();panels.open('history');opener.setAttribute('aria-expanded','true');failure='';message='Возвращаюсь к шагу…';retryId=id;
    try{const result=await navigator.go(id);if(!result.current)return;
      retryId=null;const pose=result.value.pose,selection=scene.port.selection;
      message=(pose.selectedId&&selection.nodeId!==pose.selectedId||pose.relationId&&selection.relationId!==pose.relationId)?'Область открыта; прежний выбранный материал больше недоступен.':result.value.changed?'Шаг открыт. Данные обновились с прошлого посещения.':'Шаг открыт.';
      store.save();scene.port.announce(message);render();
    }catch(error){failure=error.message;message='';panels.open('history');opener.setAttribute('aria-expanded','true');render();}
  }
  async function step(delta){flush();const {entries,cursor}=store.state;await go(entries[cursor+delta]?.id);}
  // The scene calls beforeChange before a semantic mutation. The microtask
  // captures its final pose, after graph installation and focus have finished.
  scene.port.setHistory({beforeChange(){cancel();flush();observe();},back:()=>void step(-1)});
  root.addEventListener('click',event=>{if(!controls.contains(event.target)&&!panel.contains(event.target))observe();});
  root.addEventListener('pointerdown',event=>{if(!controls.contains(event.target)&&!panel.contains(event.target)){cancel();flush();}},{capture:true});
  root.addEventListener('pointerup',observe,{passive:true});
  root.addEventListener('wheel',event=>{if(event.target.closest('.sc-panel'))return;cancel();clearTimeout(gestureTimer);gestureTimer=setTimeout(observe,260);},{passive:true});
  root.addEventListener('gesturechange',event=>{if(event.target.closest('.sc-panel'))return;cancel();clearTimeout(gestureTimer);gestureTimer=setTimeout(observe,260);},{passive:true});
  root.addEventListener('keydown',event=>{
    if(event.target.closest('input,textarea,select,[contenteditable]'))return;
    if(event.altKey&&['ArrowLeft','ArrowRight'].includes(event.key)){event.preventDefault();event.stopPropagation();void step(event.key==='ArrowLeft'?-1:1);return;}
    if(!['Tab','Shift','Alt','Control','Meta'].includes(event.key)&&!panel.contains(event.target)){cancel();flush();observe();}
  },{capture:true});
  root.addEventListener('keyup',observe,{passive:true});
  root.addEventListener('sophia-workspace-replacing',()=>{replacing=true;started=false;pending=false;clearTimeout(saveTimer);clearTimeout(gestureTimer);navigator.cancel();});
  window.addEventListener('pagehide',()=>{if(!replacing){flush();store.save();}clearTimeout(saveTimer);clearTimeout(gestureTimer);navigator.cancel();});
  document.addEventListener('visibilitychange',()=>{if(document.hidden&&!replacing){flush();store.save();}});
  render();
  return {observe,exportState(){flush();return store.state;},flush(){flush();clearTimeout(saveTimer);store.save();},resume:route=>store.resume(route),start({restored=false}={}){
    if(restored){try{lastPlace=capture();baseline=lastPlace&&travelKey(lastPlace);}catch(error){failure=error.message;}}
    started=true;observe();render();
  }};
}
