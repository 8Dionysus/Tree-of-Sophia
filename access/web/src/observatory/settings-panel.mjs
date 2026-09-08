import {DEFAULT_INTERFACE} from './interface-model.mjs';
import {refreshIcons} from './icons';
import './settings.css';

const el=(tag,text='',className='')=>{const e=document.createElement(tag);e.textContent=text;e.className=className;return e;};
const button=(text,action)=>{const e=el('button',text);e.type='button';e.addEventListener('click',action);return e;};
export function createSettingsPanel(root,scene,panels,{studio,onUserAction=()=>{}}){
  const opener=button('',show);opener.className='sc-control sc-settings-open';opener.setAttribute('aria-label','Настройки');opener.setAttribute('aria-expanded','false');
  opener.dataset.tooltip='Чтение, управление пространством и расположение инструментов.';
  opener.innerHTML='<i data-lucide="settings" aria-hidden="true"></i>';root.querySelector('.sc-header').append(opener);
  const panel=el('section','','sc-panel sc-settings');panel.hidden=true;panel.setAttribute('aria-label','Настройки');panel.id='sc-settings';opener.setAttribute('aria-controls',panel.id);
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">НАСТРОЙКИ</span><button type="button" class="sc-icon sc-settings-close" aria-label="Закрыть настройки"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-settings-body"></div><p class="sc-settings-status" role="status"></p>';
  root.append(panel);const body=panel.querySelector('.sc-settings-body'),status=panel.querySelector('.sc-settings-status');
  panels.register('settings',panel,()=>opener.setAttribute('aria-expanded','false'),{anchor:'.sc-header'});panels.configure('settings',{onResume:render});
  function show(){onUserAction();panels.open('settings');opener.setAttribute('aria-expanded','true');render();panel.querySelector('.sc-settings-close').focus();}
  function close(){panels.close('settings');opener.focus();}
  panel.querySelector('.sc-settings-close').addEventListener('click',close);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  root.addEventListener('sophia-settings-open',show);
  root.addEventListener('sophia-controls-change',()=>{if(!panel.hidden)render();});
  function safe(action){onUserAction();try{const message=action();status.textContent=panels.storageError||(typeof message==='string'?message:'Сохранено в этом браузере.');}catch(error){status.textContent=error.message;}}
  function section(title){const group=el('fieldset');group.append(el('legend',title));body.append(group);return group;}
  function option(group,label,key,values,description){
    const field=el('label',label),select=el('select');select.setAttribute('aria-label',label);select.name=key;
    for(const [value,text]of values){const item=el('option',text);item.value=value;select.append(item);}select.value=panels.preferences[key];
    select.addEventListener('change',()=>safe(()=>panels.setPreferences({...panels.preferences,[key]:select.value})));field.append(select);
    if(description){const hint=el('small',description);hint.id='sc-setting-'+key+'-hint';field.append(hint);select.setAttribute('aria-describedby',hint.id);}group.append(field);
  }
  function render(){
    const top=body.scrollTop;body.replaceChildren();status.textContent=panels.storageError||'';
    const reading=section('Чтение и подписи');
    option(reading,'Размер текста','text',[['comfortable','Обычный'],['large','Крупнее']]);
    option(reading,'Подписи звёзд','labels',[['normal','Обычные'],['large','Крупнее']]);
    const controls=section('Управление пространством');
    option(controls,'Устройство','inputMode',[['trackpad','Тачпад'],['mouse','Мышь']],'Тачпад: два пальца — сдвиг, щипок — приближение. Мышь: колесо — масштаб, перетаскивание — вращение.');
    option(controls,'Движение звёзд','motion',[['system','По настройкам системы'],['paused','Приостановлено'],['running','Включено']],'При паузе пространство остаётся доступным для перемещения и чтения.');
    const layout=section('Окна и инструменты');
    option(layout,'Сторона окна','dock',[['auto','По свободному месту'],['left','Слева'],['right','Справа']]);
    layout.append(el('p','Закреплённые инструменты появятся в верхней панели. Остальные доступны в «Моём пространстве».'));
    const choices=el('div','','sc-settings-tools');layout.append(choices);
    const preferences=panels.preferences,available=panels.toolList(),ordered=[...preferences.pinned,...available.map(t=>t.id).filter(id=>!preferences.pinned.includes(id))];
    for(const id of ordered){
      const tool=available.find(t=>t.id===id);if(!tool)continue;
      const row=el('div','','sc-settings-tool'),label=el('label'),check=el('input');check.type='checkbox';check.checked=preferences.pinned.includes(id);check.setAttribute('aria-label','Закрепить: '+tool.title);check.dataset.toolId=id;
      label.append(check,el('span',tool.title));row.append(label);choices.append(row);
      const rerender=()=>{render();[...body.querySelectorAll('input')].find(e=>e.dataset.toolId===id)?.focus();};
      check.addEventListener('change',()=>safe(()=>{const current=panels.preferences;panels.setPreferences({...current,pinned:check.checked?[...current.pinned,id]:current.pinned.filter(i=>i!==id)});rerender();}));
      if(check.checked){const up=button('↑',()=>safe(()=>{const current=panels.preferences,index=current.pinned.indexOf(id);if(index>0){[current.pinned[index-1],current.pinned[index]]=[current.pinned[index],current.pinned[index-1]];panels.setPreferences(current);rerender();}}));
        up.setAttribute('aria-label','Выше: '+tool.title);up.disabled=preferences.pinned[0]===id;up.dataset.tooltip=up.disabled?'Этот инструмент уже первый.':'Передвинуть инструмент на одно место влево в верхней панели.';row.append(up);}
    }
    const reset=button('Сбросить оформление и управление',()=>safe(()=>{panels.setPreferences(structuredClone(DEFAULT_INTERFACE));render();body.querySelector('.sc-settings-reset').focus();}));
    reset.className='sc-settings-reset';reset.dataset.tooltip='Вернуть размеры окон, текст, управление и закреплённые кнопки к исходным значениям. Места и заметки сохранятся.';layout.append(reset);
    const local=section('Локальные данные');
    local.append(button('Сохранить и перенести исследование',()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy'))));
    local.append(button('История переходов',()=>root.dispatchEvent(new CustomEvent('sophia-history-open'))));
    const forget=button('Забыть последний вид',()=>safe(()=>{studio.forgetResume();return 'Автовозврат отключён до следующего открытия страницы.';}));
    forget.dataset.tooltip='Убрать автоматическое восстановление текущего вида. Сохранённые места, история и заметки останутся.';local.append(forget);
    local.append(el('p','Исследование и настройки сохраняются в этом браузере. Полную копию можно перенести файлом.'));
    body.scrollTop=top;scene.invalidate();
  }
  refreshIcons();return {show};
}
