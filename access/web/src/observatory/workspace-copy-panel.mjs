import {uiComputed,uiLanguage,ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
import {COPY_SCHEMA,COPY_FILE_LIMIT,validateWorkspaceCopy,snapshotCopyStorage,commitWorkspaceCopy} from './workspace-copy.mjs';
import {readPlaces,readResume} from './place-model.mjs';
import {readSaved} from './lens-model.mjs';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
const button=(text,action)=>{const node=el('button',text);node.type='button';node.addEventListener('click',action);return node;};
function download(copy){const url=URL.createObjectURL(new Blob([JSON.stringify(copy,null,2)],{type:'application/json'}));
  const link=el('a');link.href=url;link.download='sophia-workspace-'+new Date().toISOString().slice(0,10)+'.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
const formatCopyDate=value=>uiComputed(()=>new Date(value).toLocaleString(uiLanguage()));
export function createWorkspaceCopyPanel(root,scene,panels,{workspace,reader,travel,studio,onUserAction=()=>{}}){
  let storage=null,pending=null,before=null,previous=null,readTurn=0,returnFocus=null;
  try{storage=localStorage;}catch{}
  const panel=el('section','','sc-panel sc-workspace-copy');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Копия исследования"));
  const top=el('div','','sc-panel-top');const close=button('×',()=>{panels.close('copy');(returnFocus||root.querySelector('.sc-studio-open'))?.focus();});close.className='sc-icon';uiAttribute(close, 'aria-label', ui("Закрыть копию исследования"));
  uiChildren(top, "append", el('span',ui("МОЁ ИССЛЕДОВАНИЕ"),'sc-eyebrow'), close);
  const heading=el('h3',ui("Сохранить и перенести")),description=el('p',ui("В копию входят история, места, сохранённые линзы, настройки, записи и пара материалов для чтения. Тексты материалов при открытии загружаются заново."));
  const output=el('div','','sc-copy-preview'),status=el('p','','sc-copy-status');uiAttribute(status, 'role', 'status');
  const upload=el('input');upload.type='file';upload.accept='.json,application/json';upload.hidden=true;uiAttribute(upload, 'aria-label', ui("Файл полной копии исследования"));
  function capture(){
    travel.flush();studio.flush();reader.flush();
    return validateWorkspaceCopy({schema:COPY_SCHEMA,v:1,exportedAt:new Date().toISOString(),history:travel.exportState(),places:storage?readPlaces(storage):[],resume:storage?readResume(storage,''):null,
      lenses:storage?readSaved(storage):[],preferences:panels.preferences,reading:reader.exportState(),research:JSON.parse(workspace.exportPacket())});
  }
  const exportButton=button(ui("Скачать текущую копию"),()=>{onUserAction();try{download(capture());uiText(status, ui("Копия подготовлена для скачивания."));}catch(error){uiText(status, error.message);}});
  const importButton=button(ui("Выбрать копию для импорта"),()=>{onUserAction();upload.value='';upload.click();});
  const actions=el('div','','sc-copy-actions');uiChildren(actions, "append", exportButton, importButton);
  const content=el('div','','sc-copy-content');uiChildren(content, "append", heading, description, actions, upload, output, status);uiChildren(panel, "append", top, content);uiChildren(root, "append", panel);
  function discard(){readTurn++;pending=null;before=null;previous=null;uiChildren(output, "replaceChildren");}
  panels.register('copy',panel,discard);
  function show(){const from=document.activeElement?.closest('[data-panel-id]')?.dataset.panelId;returnFocus=root.querySelector(from==='settings'?'.sc-settings-open':from==='workspace'?'.sc-workspace-open':'.sc-studio-open');onUserAction();discard();uiText(status, '');panels.open('copy');heading.tabIndex=-1;heading.focus();scene.invalidate();}
  root.addEventListener('sophia-workspace-copy',show);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close.click();}});
  upload.addEventListener('change',async()=>{
    discard();const file=upload.files?.[0];if(!file)return;const turn=readTurn;uiText(status, ui("Проверяю копию…"));
    try{
      if(file.size>COPY_FILE_LIMIT)throw new Error(ui("Копия превышает 12 МБ."));
      const text=await file.text();if(turn!==readTurn)return;pending=validateWorkspaceCopy(text);previous=capture();before=snapshotCopyStorage(storage,location.pathname);
      const counts=copy=>ui("Шаги: {0} · места: {1} · линзы: {2} · заметки: {3} · гипотезы: {4} · предложения: {5} · чтение: {6}", [copy.history.entries.length, copy.places.length, copy.lenses.length, copy.research.notes.length, copy.research.hypotheses.length, copy.research.proposals.length, copy.reading.entries.length]);
      uiChildren(output, "append", el('h4',ui("Копия от {0}", [formatCopyDate(pending.exportedAt)])), el('p',counts(pending)), el('p',ui("Сейчас: {0}", [counts(previous)])), el('p',ui("Импорт заменит текущее исследование в этом браузере. Сначала можно скачать прежнюю копию. После замены страница откроется заново.")));
      const apply=button(ui("Заменить исследование и открыть"),()=>{
        onUserAction();try{commitWorkspaceCopy(storage,location.pathname,pending,before);
          root.dispatchEvent(new CustomEvent('sophia-workspace-replacing'));apply.disabled=true;uiText(status, ui("Копия сохранена. Открываю исследование…"));
          // Reload through the normal owner readers. pagehide writers are
          // suspended so they cannot overwrite the imported state.
          location.assign(location.pathname);
        }catch(error){uiText(status, error.message);}
      });
      uiChildren(output, "append", button(ui("Скачать прежнюю копию"),()=>download(previous)), apply, button(ui("Отменить импорт"),()=>{discard();uiText(status, ui("Импорт отменён."));}));uiText(status, ui("Файл проверен. Данные пока не заменены."));scene.invalidate();
    }catch(error){pending=null;uiText(status, error.message);}
  });
  return {show};
}
