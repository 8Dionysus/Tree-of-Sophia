import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {DEFAULT_INTERFACE} from './interface-model.mjs';
import {refreshIcons} from './icons';
import './settings.css';

const el=(tag,text='',className='')=>{const e=document.createElement(tag);uiText(e, text);e.className=className;return e;};
const button=(text,action)=>{const e=el('button',text);e.type='button';e.addEventListener('click',action);return e;};
export function createSettingsPanel(root,scene,panels,{studio,onUserAction=()=>{}}){
  const opener=button('',show);opener.className='sc-control sc-settings-open';uiAttribute(opener, 'aria-label', ui("Настройки"));uiAttribute(opener, 'aria-expanded', 'false');
  uiAttribute(opener, "data-tooltip", ui("Чтение, управление пространством и расположение инструментов."));
  uiHTML(opener, '<i data-lucide="settings" aria-hidden="true"></i>');uiChildren(root.querySelector('.sc-header'), "append", opener);
  const panel=el('section','','sc-panel sc-settings');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Настройки"));panel.id='sc-settings';uiAttribute(opener, 'aria-controls', panel.id);
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">НАСТРОЙКИ</span><button type="button" class="sc-icon sc-settings-close" aria-label="Закрыть настройки"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-settings-body"></div><p class="sc-settings-status" role="status"></p>');
  uiChildren(root, "append", panel);const body=panel.querySelector('.sc-settings-body'),status=panel.querySelector('.sc-settings-status');
  panels.register('settings',panel,()=>uiAttribute(opener, 'aria-expanded', 'false'),{anchor:'.sc-header'});panels.configure('settings',{onResume:render});
  function show(){onUserAction();panels.open('settings');uiAttribute(opener, 'aria-expanded', 'true');render();panel.querySelector('.sc-settings-close').focus();}
  function close(){panels.close('settings');opener.focus();}
  panel.querySelector('.sc-settings-close').addEventListener('click',close);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  root.addEventListener('sophia-settings-open',show);
  root.addEventListener('sophia-controls-change',()=>{if(!panel.hidden)render();});
  function safe(action){onUserAction();try{const message=action();uiText(status, panels.storageError||(typeof message==='string'||message instanceof String?message:ui("Сохранено в этом браузере.")));}catch(error){uiText(status, error.message);}}
  function section(title){const group=el('fieldset');uiChildren(group, "append", el('legend',title));uiChildren(body, "append", group);return group;}
  function option(group,label,key,values,description){
    const field=el('label',label),select=el('select');uiAttribute(select, 'aria-label', label);select.name=key;
    for(const [value,text]of values){const item=el('option',text);item.value=value;uiChildren(select, "append", item);}select.value=panels.preferences[key];
    select.addEventListener('change',()=>safe(()=>panels.setPreferences({...panels.preferences,[key]:select.value})));uiChildren(field, "append", select);
    if(description){const hint=el('small',description);hint.id='sc-setting-'+key+'-hint';uiChildren(field, "append", hint);uiAttribute(select, 'aria-describedby', hint.id);}uiChildren(group, "append", field);
  }
  function render(){
    const top=body.scrollTop;uiChildren(body, "replaceChildren");uiText(status, panels.storageError||'');
    const language=section(ui("Язык"));
    option(language,ui("Язык интерфейса"),'uiLanguage',[['ru',ui("Русский")],['en','English'],['es','Español']],ui("Язык кнопок и окон. Язык материалов выбирается отдельно при чтении — из доступных версий."));
    const appearance=section(ui("Оформление"));
    option(appearance,ui("Тема интерфейса"),'theme',[['dark',ui("Тёмная")],['light',ui("Светлая")]],ui("Тема меняет панели и кнопки. Звёздное пространство остаётся тёмным."));
    const reading=section(ui("Чтение и подписи"));
    option(reading,ui("Размер текста"),'text',[['comfortable',ui("Обычный")],['large',ui("Крупнее")]]);
    option(reading,ui("Подписи звёзд"),'labels',[['normal',ui("Обычные")],['large',ui("Крупнее")]]);
    const controls=section(ui("Управление пространством"));
    option(controls,ui("Перетаскивание"),'dragAction',[['rotate',ui("Вращать пространство")],['pan',ui("Сдвигать пространство")]],ui("Shift временно меняет действие на противоположное."));
    option(controls,ui("Прокрутка"),'scrollAction',[['auto',ui("Автоматически")],['pan',ui("Сдвигать пространство")],['zoom',ui("Приближать и отдалять")]],ui("Автоматически: плавный жест сдвигает пространство, шаги колеса меняют масштаб. Щипок всегда приближает. Для плавного колеса можно выбрать действие вручную."));
    option(controls,ui("Чувствительность"),'sensitivity',[['gentle',ui("Мягкая")],['normal',ui("Обычная")],['fast',ui("Быстрая")]]);
    const layout=section(ui("Окна и инструменты"));
    option(layout,ui("Сторона окна"),'dock',[['auto',ui("По свободному месту")],['left',ui("Слева")],['right',ui("Справа")]]);
    uiChildren(layout, "append", el('p',ui("Закреплённые инструменты появятся в верхней панели. Остальные доступны в «Моём пространстве».")));
    const choices=el('div','','sc-settings-tools');uiChildren(layout, "append", choices);
    const preferences=panels.preferences,available=panels.toolList(),ordered=[...preferences.pinned,...available.map(t=>t.id).filter(id=>!preferences.pinned.includes(id))];
    for(const id of ordered){
      const tool=available.find(t=>t.id===id);if(!tool)continue;
      const row=el('div','','sc-settings-tool'),label=el('label'),check=el('input');check.type='checkbox';check.checked=preferences.pinned.includes(id);uiAttribute(check, 'aria-label', ui("Закрепить: {0}", [tool.title]));check.dataset.toolId=id;
      uiChildren(label, "append", check, el('span',tool.title));uiChildren(row, "append", label);uiChildren(choices, "append", row);
      const rerender=()=>{render();[...body.querySelectorAll('input')].find(e=>e.dataset.toolId===id)?.focus();};
      check.addEventListener('change',()=>safe(()=>{const current=panels.preferences;panels.setPreferences({...current,pinned:check.checked?[...current.pinned,id]:current.pinned.filter(i=>i!==id)});rerender();}));
      if(check.checked){const up=button('↑',()=>safe(()=>{const current=panels.preferences,index=current.pinned.indexOf(id);if(index>0){[current.pinned[index-1],current.pinned[index]]=[current.pinned[index],current.pinned[index-1]];panels.setPreferences(current);rerender();}}));
        uiAttribute(up, 'aria-label', ui("Выше: {0}", [tool.title]));up.disabled=preferences.pinned[0]===id;uiAttribute(up, "data-tooltip", up.disabled?ui("Этот инструмент уже первый."):ui("Передвинуть инструмент на одно место влево в верхней панели."));uiChildren(row, "append", up);}
    }
    const reset=button(ui("Сбросить оформление и управление"),()=>safe(()=>{panels.setPreferences({...structuredClone(DEFAULT_INTERFACE),uiLanguage:panels.preferences.uiLanguage});render();body.querySelector('.sc-settings-reset').focus();}));
    reset.className='sc-settings-reset';uiAttribute(reset, "data-tooltip", ui("Вернуть размеры окон, текст, управление и закреплённые кнопки к исходным значениям. Места и заметки сохранятся."));uiChildren(layout, "append", reset);
    const local=section(ui("Локальные данные"));
    uiChildren(local, "append", button(ui("Сохранить и перенести исследование"),()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy'))));
    uiChildren(local, "append", button(ui("История переходов"),()=>root.dispatchEvent(new CustomEvent('sophia-history-open'))));
    const forget=button(ui("Забыть последний вид"),()=>safe(()=>{studio.forgetResume();return ui("Автовозврат отключён до следующего открытия страницы.");}));
    uiAttribute(forget, "data-tooltip", ui("Убрать автоматическое восстановление текущего вида. Сохранённые места, история и заметки останутся."));uiChildren(local, "append", forget);
    uiChildren(local, "append", el('p',ui("Исследование и настройки сохраняются в этом браузере. Полную копию можно перенести файлом.")));
    body.scrollTop=top;scene.invalidate();
  }
  refreshIcons();return {show};
}
