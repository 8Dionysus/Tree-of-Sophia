import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {RequestSlots,localized} from './knowledge-client.mjs';
import {capturePlace,readPlaces,savePlace,readResume,reopenPlace,PLACES_KEY,RESUME_KEY} from './place-model.mjs';
import {refreshIcons} from './icons';
const el=(tag,text='',className='')=>{const e=document.createElement(tag);uiText(e, text);e.className=className;return e;};
const button=(text,action)=>{const b=el('button',text);b.type='button';b.addEventListener('click',action);return b;};
export function createStudio(root,scene,panels,{data:{client},initialRoute,onUserAction}){
  const requests=new RequestSlots();let storage=null,entries=[],failure='',notice='',active='places',busy=false,applying=false,started=false,autoSave=true,timer=null,lastPacket=null,deleted=null,retry=null,restoreId=0;
  try{storage=localStorage;entries=readPlaces(storage);}catch(error){failure=error.message;}
  const opener=button('',()=>{onUserAction();show();});opener.className='sc-control sc-studio-open';uiAttribute(opener, 'aria-label', ui("Места и инструменты"));uiAttribute(opener, 'aria-expanded', 'false');uiHTML(opener, '<i data-lucide="bookmark" aria-hidden="true"></i><span>Моё пространство</span>');
  uiChildren(root.querySelector('.sc-header-actions'), "append", opener);
  const panel=el('section','','sc-panel sc-studio');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Места и инструменты"));
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">МОЁ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-studio-close" aria-label="Закрыть места и инструменты"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Места мысли</h3><div class="sc-studio-tabs" role="tablist" aria-label="Рабочее окружение"></div><div class="sc-studio-body" role="tabpanel" tabindex="0"></div><p class="sc-studio-status" role="status"></p>');uiChildren(root, "append", panel);
  const body=panel.querySelector('.sc-studio-body'),status=panel.querySelector('.sc-studio-status');body.id='sc-studio-content';
  const retryButton=button(ui("Повторить открытие места"),()=>{onUserAction();void restore(retry);});retryButton.className='sc-studio-retry';retryButton.hidden=true;status.after(retryButton);
  const copy=button(ui("Сохранить и перенести исследование"),()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy')));copy.className='sc-studio-copy';status.after(copy);
  function cancel(){restoreId++;requests.cancelAll();busy=false;}
  panels.register('studio',panel,()=>{cancel();uiAttribute(opener, 'aria-expanded', 'false');});
  panels.configure('studio',{onResume:render});
  const tabs=['places','tools'].map((id,i)=>{const b=button(i?ui("Инструменты"):ui("Места"),()=>{onUserAction();cancel();active=id;render();});b.id='sc-studio-'+id;uiAttribute(b, 'role', 'tab');uiAttribute(b, 'aria-controls', body.id);uiChildren(panel.querySelector('.sc-studio-tabs'), "append", b);
    b.addEventListener('keydown',event=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){event.preventDefault();const index=event.key==='Home'?0:event.key==='End'?1:1-i;tabs[index].click();tabs[index].focus();}});return b;});
  function close(){panels.close('studio');opener.focus();}
  panel.querySelector('.sc-studio-close').addEventListener('click',close);panel.addEventListener('keydown',e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}});
  function show(){try{if(storage)entries=readPlaces(storage);}catch(error){failure=error.message;}panels.open('studio');uiAttribute(opener, 'aria-expanded', 'true');render();tabs[active==='places'?0:1].focus();}
  function report(error){failure=error.message||ui("Не удалось выполнить действие.");renderStatus();}
  function safe(work){try{onUserAction();work();}catch(error){report(error);}}
  const areaName=()=>localized(scene.port.node(scene.port.selection.nodeId||scene.port.packet?.focus?.node_id)?.display.title,ui("Моё место"));
  function capture(name,id){if(!scene.port.packet?.nodes.length)throw new Error(ui("Сначала дождитесь загрузки области."));return capturePlace(scene.port.packet,scene.port.capturePlace(),{name:name.slice(0,64),id,route:location.search});}
  function persist(name,id){if(!storage)throw new Error(ui("Локальное хранилище недоступно."));entries=savePlace(storage,capture(name,id));failure='';notice=ui("Место сохранено в этом браузере.");render();}
  function saveResume(){
    if(!started||!autoSave||applying||busy||!storage||!scene.port.packet?.nodes.length)return;
    try{const text=JSON.stringify(capture(areaName(),'resume'));if(storage.getItem(RESUME_KEY)!==text)storage.setItem(RESUME_KEY,text);}catch(error){failure=ui("Не удалось сохранить последний вид. {0}", [error.message]);renderStatus();}
  }
  function schedule(){clearTimeout(timer);timer=setTimeout(saveResume,800);}
  async function restore(place,{initial=false}={}){
    cancel();const turn=restoreId;scene.ui.cancelPending();busy=true;retry=place;failure='';notice=ui("Возвращаюсь к месту…");renderStatus();
    try{
      const answer=await requests.run('place',signal=>reopenPlace(client,place,signal));if(!answer.current||turn!==restoreId)return false;
      applying=true;try{scene.port.setGraph(answer.value.packet,{initial});scene.port.restorePlace(answer.value.pose);}finally{applying=false;}
      notice=answer.value.changed?ui("Место открыто. Данные обновились с последнего посещения."):ui("Место открыто.");retry=null;autoSave=true;
      // Showing a saved card may close this tool through the panel host.
      busy=false;schedule();if(!initial&&!panel.hidden)render();scene.port.announce(notice);return true;
    }catch(error){if(turn===restoreId)report(error);return false;}finally{if(turn===restoreId){busy=false;renderStatus();}}
  }
  function renderStatus(){uiText(status, failure||notice||panels.storageError||'');uiAttribute(panel, 'aria-busy', String(busy));body.querySelectorAll('[data-place-action]').forEach(b=>b.disabled=busy);retryButton.hidden=active!=='places'||!retry||!failure;retryButton.disabled=busy;}
  function field(label,input){const wrap=el('label',label,'sc-studio-field');uiChildren(wrap, "append", input);return wrap;}
  function renderPlaces(){
    uiChildren(body, "append", el('p',ui("Сохраните область, линзу и ракурс. При возвращении данные проверяются заново."),'sc-studio-note'));
    const form=el('form'),input=el('input');input.type='text';input.maxLength=64;input.value=areaName().slice(0,64);input.required=true;uiAttribute(input, 'aria-label', ui("Название места"));
    const save=el('button',ui("Сохранить текущее место"));save.type='submit';save.dataset.placeAction='save';save.disabled=busy||!scene.port.packet;
    uiChildren(form, "append", field(ui("Название места"),input), save);form.addEventListener('submit',e=>{e.preventDefault();safe(()=>persist(input.value.trim(),crypto.randomUUID()));});uiChildren(body, "append", form);
    for(const place of entries){const row=el('article','','sc-place');const open=button(place.name,()=>{onUserAction();void restore(place);});open.className='sc-place-open';open.dataset.placeAction='open';uiChildren(row, "append", open, el('small',ui("{0} · {1} звёзд при сохранении", [new Date(place.savedAt).toLocaleDateString('ru',{day:'numeric',month:'long'}), place.pose.vertices.length])));
      const actions=el('div','','sc-place-actions');uiChildren(actions, "append", button(ui("Переименовать"),()=>{
        const form=el('form'),name=el('input');name.value=place.name;name.maxLength=64;name.required=true;uiAttribute(name, 'aria-label', ui("Новое название места"));
        const save=el('button',ui("Сохранить название"));save.type='submit';uiChildren(form, "append", name, save, button(ui("Отмена"),render));
        form.addEventListener('submit',event=>{event.preventDefault();safe(()=>{entries=savePlace(storage,{...place,name:name.value.trim()});notice=ui("Название места сохранено.");render();});});uiChildren(actions, "replaceChildren", form);name.focus();name.select();
      }), button(ui("Обновить этим видом"),()=>safe(()=>persist(place.name,place.id))), button(ui("Удалить"),()=>safe(()=>{deleted=place;entries=readPlaces(storage).filter(e=>e.id!==place.id);storage.setItem(PLACES_KEY,JSON.stringify(entries));notice=ui("Место удалено.");render();})));uiChildren(row, "append", actions);uiChildren(body, "append", row);}
    if(deleted)uiChildren(body, "append", button(ui("Вернуть удалённое место"),()=>safe(()=>{entries=savePlace(storage,deleted);deleted=null;notice=ui("Место восстановлено.");render();})));
    if(!entries.length)uiChildren(body, "append", el('p',ui("Здесь появятся места, к которым хочется вернуться."),'sc-studio-empty'));
    uiChildren(body, "append", el('p',ui("Последний вид запоминается автоматически в этом браузере."),'sc-studio-note'));
  }
  function renderTools(){
    uiChildren(body, "append", el('p',ui("Откройте нужный инструмент. Закрепление и порядок кнопок доступны в настройках."),'sc-studio-note'));
    for(const tool of panels.toolList()){
      const launch=button(tool.title,()=>safe(()=>panels.launch(tool.id)));uiAttribute(launch, 'aria-disabled', String(!tool.available));
      uiAttribute(launch, "data-tooltip", tool.available?ui("Открыть инструмент «{0}».", [tool.title]):ui("Сначала выберите звезду или связь."));
      const row=el('div','','sc-tool-choice');uiChildren(row, "append", launch);uiChildren(body, "append", row);
    }
    uiChildren(body, "append", button(ui("Настроить инструменты"),()=>root.dispatchEvent(new CustomEvent('sophia-settings-open'))));
  }
  function render(){const top=body.scrollTop;uiChildren(body, "replaceChildren");uiText(panel.querySelector('h3'), active==='places'?ui("Места мысли"):ui("Мои инструменты"));tabs.forEach((b,i)=>{const selected=active===(i?'tools':'places');uiAttribute(b, 'aria-selected', String(selected));b.tabIndex=selected?0:-1;});uiAttribute(body, 'aria-labelledby', 'sc-studio-'+active);if(active==='places')renderPlaces();else renderTools();body.scrollTop=top;renderStatus();scene.invalidate();}
  root.addEventListener('pointerdown',e=>{if(busy&&!panel.contains(e.target)){cancel();notice=ui("Возвращение прервано вашим действием.");renderStatus();}},{capture:true});
  root.addEventListener('wheel',e=>{if(!e.target.closest('.sc-panel')){if(busy){cancel();notice=ui("Возвращение прервано вашим действием.");renderStatus();}schedule();}},{passive:true});
  root.addEventListener('keydown',e=>{if(busy&&!panel.contains(e.target)&&!['Tab','Shift','Control','Meta','Alt'].includes(e.key)){cancel();notice=ui("Возвращение прервано вашим действием.");renderStatus();}},{capture:true});
  root.addEventListener('pointerup',schedule,{passive:true});root.addEventListener('keyup',schedule,{passive:true});
  root.addEventListener('sophia-place-saved',()=>{if(storage)entries=readPlaces(storage);if(!panel.hidden)render();});
  root.addEventListener('sophia-workspace-replacing',()=>{started=false;autoSave=false;clearTimeout(timer);cancel();});
  window.addEventListener('pagehide',()=>{clearTimeout(timer);saveResume();cancel();});document.addEventListener('visibilitychange',()=>{if(document.hidden)saveResume();});
  refreshIcons();
  return {
    forgetResume(){storage?.removeItem(RESUME_KEY);autoSave=false;},
    flush(){clearTimeout(timer);saveResume();},
    selectionChanged(){if(!applying&&scene.port.packet!==lastPacket){if(busy)cancel();lastPacket=scene.port.packet;}schedule();},
    async start(){
      let resume=null;try{if(storage)resume=readResume(storage,initialRoute);}catch(error){autoSave=false;report(error);}
      if(resume){const restored=await restore(resume,{initial:true});started=true;if(restored){void scene.ui.start({skipScene:true});schedule();return true;}autoSave=false;show();}
      started=true;void scene.ui.start();return false;
    },
  };
}
