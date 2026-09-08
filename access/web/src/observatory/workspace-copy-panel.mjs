import {COPY_SCHEMA,COPY_FILE_LIMIT,validateWorkspaceCopy,snapshotCopyStorage,commitWorkspaceCopy} from './workspace-copy.mjs';
import {readPlaces,readResume} from './place-model.mjs';
import {readSaved} from './lens-model.mjs';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(text,action)=>{const node=el('button',text);node.type='button';node.addEventListener('click',action);return node;};
function download(copy){const url=URL.createObjectURL(new Blob([JSON.stringify(copy,null,2)],{type:'application/json'}));
  const link=el('a');link.href=url;link.download='sophia-workspace-'+new Date().toISOString().slice(0,10)+'.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
export function createWorkspaceCopyPanel(root,scene,panels,{workspace,reader,travel,studio,onUserAction=()=>{}}){
  let storage=null,pending=null,before=null,previous=null,readTurn=0,returnFocus=null;
  try{storage=localStorage;}catch{}
  const panel=el('section','','sc-panel sc-workspace-copy');panel.hidden=true;panel.setAttribute('aria-label','Копия исследования');
  const top=el('div','','sc-panel-top');const close=button('×',()=>{panels.close('copy');(returnFocus||root.querySelector('.sc-studio-open'))?.focus();});close.className='sc-icon';close.setAttribute('aria-label','Закрыть копию исследования');
  top.append(el('span','МОЁ ИССЛЕДОВАНИЕ','sc-eyebrow'),close);
  const heading=el('h3','Сохранить и перенести'),description=el('p','В копию входят история, места, сохранённые линзы, настройки, записи и пара материалов для чтения. Тексты материалов при открытии загружаются заново.');
  const output=el('div','','sc-copy-preview'),status=el('p','','sc-copy-status');status.setAttribute('role','status');
  const upload=el('input');upload.type='file';upload.accept='.json,application/json';upload.hidden=true;upload.setAttribute('aria-label','Файл полной копии исследования');
  function capture(){
    travel.flush();studio.flush();reader.flush();
    return validateWorkspaceCopy({schema:COPY_SCHEMA,v:1,exportedAt:new Date().toISOString(),history:travel.exportState(),places:storage?readPlaces(storage):[],resume:storage?readResume(storage,''):null,
      lenses:storage?readSaved(storage):[],preferences:panels.preferences,reading:reader.exportState(),research:JSON.parse(workspace.exportPacket())});
  }
  const exportButton=button('Скачать текущую копию',()=>{onUserAction();try{download(capture());status.textContent='Копия подготовлена для скачивания.';}catch(error){status.textContent=error.message;}});
  const importButton=button('Выбрать копию для импорта',()=>{onUserAction();upload.value='';upload.click();});
  const actions=el('div','','sc-copy-actions');actions.append(exportButton,importButton);
  const content=el('div','','sc-copy-content');content.append(heading,description,actions,upload,output,status);panel.append(top,content);root.append(panel);
  function discard(){readTurn++;pending=null;before=null;previous=null;output.replaceChildren();}
  panels.register('copy',panel,discard);
  function show(){const from=document.activeElement?.closest('[data-panel-id]')?.dataset.panelId;returnFocus=root.querySelector(from==='settings'?'.sc-settings-open':from==='workspace'?'.sc-workspace-open':'.sc-studio-open');onUserAction();discard();status.textContent='';panels.open('copy');heading.tabIndex=-1;heading.focus();scene.invalidate();}
  root.addEventListener('sophia-workspace-copy',show);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close.click();}});
  upload.addEventListener('change',async()=>{
    discard();const file=upload.files?.[0];if(!file)return;const turn=readTurn;status.textContent='Проверяю копию…';
    try{
      if(file.size>COPY_FILE_LIMIT)throw new Error('Копия превышает 12 МБ.');
      const text=await file.text();if(turn!==readTurn)return;pending=validateWorkspaceCopy(text);previous=capture();before=snapshotCopyStorage(storage,location.pathname);
      const counts=copy=>`Шаги: ${copy.history.entries.length} · места: ${copy.places.length} · линзы: ${copy.lenses.length} · заметки: ${copy.research.notes.length} · гипотезы: ${copy.research.hypotheses.length} · предложения: ${copy.research.proposals.length} · чтение: ${copy.reading.entries.length}`;
      output.append(el('h4','Копия от '+new Date(pending.exportedAt).toLocaleString('ru')),el('p',counts(pending)),el('p','Сейчас: '+counts(previous)),
        el('p','Импорт заменит текущее исследование в этом браузере. Сначала можно скачать прежнюю копию. После замены страница откроется заново.'));
      const apply=button('Заменить исследование и открыть',()=>{
        onUserAction();try{commitWorkspaceCopy(storage,location.pathname,pending,before);
          root.dispatchEvent(new CustomEvent('sophia-workspace-replacing'));apply.disabled=true;status.textContent='Копия сохранена. Открываю исследование…';
          // Reload through the normal owner readers. pagehide writers are
          // suspended so they cannot overwrite the imported state.
          location.assign(location.pathname);
        }catch(error){status.textContent=error.message;}
      });
      output.append(button('Скачать прежнюю копию',()=>download(previous)),apply,button('Отменить импорт',()=>{discard();status.textContent='Импорт отменён.';}));status.textContent='Файл проверен. Данные пока не заменены.';scene.invalidate();
    }catch(error){pending=null;status.textContent=error.message;}
  });
  return {show};
}
