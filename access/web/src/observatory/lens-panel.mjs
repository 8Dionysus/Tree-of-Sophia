import {uiComputed,uiLanguage,ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {RequestSlots,localized} from './knowledge-client.mjs';
import {constructorCatalog,initialDraft,compileDraft,previewDraft,summarizeLens,lensDelta,encodeDraft,draftForPacket,readSaved,saveDraft} from './lens-model.mjs';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {createConditionEditor} from './lens-condition-editor.mjs';
import {conditionCatalog,conditionText} from './lens-conditions.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
const button=(label,action,className='sc-builder-button')=>{const b=el('button',label,className);b.type='button';b.addEventListener('click',action);return b;};
const count=(n,one,few,many)=>uiComputed(()=>`${n} ${{one,few,many}[new Intl.PluralRules(uiLanguage()).select(n)]||many}`);
const sourceNames={'philosophy':ui("Философский атлас"),'canon':ui("Канон"),'candidate-intake':ui("Исследовательские кандидаты"),'source-navigation':ui("Произведения и источники"),'source-claims':ui("Утверждения источников"),'semantic-interchange':ui("Понятия и типы"),'repository':ui("Карта проекта")};

export function createLensPanel(root,scene,panels,{data:{client},onUserAction}){
  const requests=new RequestSlots();
  let context=null,draft=null,preview=null,bookmark=null,origin=null,basePacket=null,busy=false,failure='',storageError='',applied=false,applying=false;
  let saved=[],returnFocus=null,generation=0,stale=false,notice='',timer=null,scheduled=false,delta=null;
  const choiceViews=new Map(),choiceUpdates=new Map();
  const panel=el('section','','sc-panel sc-builder');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Конструктор линз"));
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">ОПТИКА МЫСЛИ</span><button class="sc-icon sc-builder-close" type="button" aria-label="Закрыть конструктор"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-builder-heading"><span aria-hidden="true">◈</span><div><h3>Собрать линзу</h3><p>Настройки сразу меняют пространство.</p></div></div><div class="sc-builder-body"></div><div class="sc-builder-result" aria-live="polite"></div><div class="sc-builder-status" role="status"></div><div class="sc-builder-footer"></div>');
  uiChildren(root, "append", panel);
  const body=panel.querySelector('.sc-builder-body'),result=panel.querySelector('.sc-builder-result'),status=panel.querySelector('.sc-builder-status'),footer=panel.querySelector('.sc-builder-footer');
  const opener=button(ui("Конструктор линз"),()=>{onUserAction();void open();},'sc-builder-open');
  uiChildren(root.querySelector('.sc-lenses'), "append", opener);
  const headerOpener=root.querySelector('.sc-lenses-open');
  const activeQuery=button('',()=>{onUserAction();void open();},'sc-active-query');activeQuery.hidden=true;
  uiAttribute(activeQuery, 'aria-label', ui("Условия отображённой области"));uiChildren(root.querySelector('.sc-context'), "append", activeQuery);
  function queryLines(value,lookup=context){
    const nodeEntries=lookup?conditionCatalog(lookup,'nodes'):[],relationEntries=lookup?conditionCatalog(lookup,'relations'):[];
    const lines=[value.scope==='area'?ui("Из исходной области · {0}", [value.nodeIds.length]):value.scope==='focus'?ui("От выбранной звезды"):ui("По всему древу")];
    lines.push(ui("Источники: {0}", [value.sources.map(id=>sourceNames[id]||id).join(', ')]));
    if(value.scope!=='focus'){
      if(value.query)lines.push(ui("Поиск: «{0}»", [value.query]));
      if(value.kinds.length)lines.push(ui("Типы узлов: {0}", [value.kinds.map(id=>localized(lookup?.catalog.node_kinds.find(k=>k.kind_id===id)?.display,id)).join(', ')]));
      for(const rule of value.conditions.nodes)lines.push(conditionText(rule,nodeEntries));
    }
    if(value.relations){
      if(value.predicates.length)lines.push(ui("Типы связей: {0}", [value.predicates.map(id=>localized(lookup?.catalog.predicates.find(k=>k.predicate_id===id)?.display,id)).join(', ')]));
      for(const rule of value.conditions.relations)lines.push(ui("Связь: {0}", [conditionText(rule,relationEntries)]));
      lines.push(ui("Окружение: {0} · {1} · {2}", [value.depth, ({either:ui("в обе стороны"),outgoing:ui("по связям"),incoming:ui("против связей")})[value.direction], value.profile==='all'?ui("все типы"):ui("обзор")]));
    }else lines.push(ui("Без связей"));
    lines.push(ui("До {0} звёзд", [value.limit]));return lines;
  }
  function updateActiveQuery(){
    const value=draftForPacket(scene.port.packet);activeQuery.hidden=!value;
    if(value){uiText(activeQuery, ui("Линза: {0} · условия", [value.name]));uiAttribute(activeQuery, "data-tooltip", queryLines(value,context?.catalog.source_revision===scene.port.packet.source_revision?context:null).slice(0,2).join(' · ').slice(0,250));}
  }
  const stop=()=>{generation++;clearTimeout(timer);timer=null;scheduled=false;requests.cancelAll();busy=false;};
  panels.register('builder',panel,stop);
  function close(){panels.close('builder');(returnFocus?.isConnected&&!returnFocus.closest('[hidden]')?returnFocus:headerOpener).focus();}
  panel.querySelector('.sc-builder-close').addEventListener('click',()=>{onUserAction();close();});
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.stopPropagation();event.preventDefault();onUserAction();close();}});
  function resetForArea(){origin=scene.port.packet;basePacket=origin;bookmark=origin?scene.port.captureView():null;preview=null;applied=false;draft=null;stale=false;delta=null;choiceViews.clear();}
  async function open(){
    returnFocus=document.activeElement;
    if(stale||!draft||scene.port.packet!==basePacket)resetForArea();
    panels.open('builder');failure='';notice='';
    try{saved=readSaved(localStorage);storageError='';}catch(error){saved=[];storageError=error.message||ui("Локальное хранилище недоступно.");}
    if(context&&context.catalog.source_revision===scene.port.packet?.source_revision){prepareDraft();render();return;}
    await loadCatalog();
  }
  function prepareDraft(){
    if(!draft)draft=structuredClone(draftForPacket(scene.port.packet)||initialDraft(origin,context));
    const selected=scene.port.selection.nodeId;
    if(draft.scope==='area'&&selected&&origin?.nodes.some(n=>n.id===selected))draft.focusId=selected;
  }
  async function loadCatalog(){
    onUserAction();stop();const token=generation;busy=true;context=null;notice='';render();
    try{const answer=await requests.run('catalog',signal=>constructorCatalog(client,signal));
      if(!answer.current||token!==generation||panel.hidden)return;context=answer.value;prepareDraft();failure='';
    }catch(error){if(token===generation)failure=error.message||ui("Не удалось загрузить словарь данных.");}
    finally{if(token===generation){busy=false;render();}}
  }
  function touch({composing=false}={}){
    onUserAction();stop();failure='';notice='';preview=null;applied=false;delta=null;
    if(context&&!stale&&!composing){scheduled=true;timer=setTimeout(()=>{timer=null;void run();},400);}
    renderResult();
  }
  function field(label,input){const wrap=el('label','','sc-builder-field');uiChildren(wrap, "append", el('span',label), input);return wrap;}
  function select(label,key,choices){
    if(!choices.some(([value])=>String(value)===String(draft[key])))choices=[...choices,[draft[key],String(draft[key])]];
    const input=el('select');for(const [value,title]of choices){const option=el('option',title);option.value=String(value);uiChildren(input, "append", option);}input.value=String(draft[key]);
    input.addEventListener('change',()=>{draft[key]=['limit','depth'].includes(key)?Number(input.value):input.value;touch();if(key==='scope')render();});return field(label,input);
  }
  function choices(title,key){
    const state=choiceViews.get(key)||{open:false,query:'',sort:'alphabet',expanded:new Map(),limits:new Map()};choiceViews.set(key,state);
    const details=el('details','','sc-builder-choices'),summary=el('summary'),list=el('div','','sc-builder-options'),search=el('input');
    details.open=state.open;details.addEventListener('toggle',()=>state.open=details.open);
    search.type='search';uiAttribute(search, "placeholder", ui("Найти в списке…"));uiAttribute(search, 'aria-label', ui("Найти: {0}", [title]));search.value=state.query;
    const sorting=el('select');uiAttribute(sorting, 'aria-label', ui("Порядок: {0}", [title]));
    for(const [id,name]of [['alphabet',ui("По алфавиту")],['frequency',ui("Сначала частые")]]){const option=el('option',name);option.value=id;uiChildren(sorting, "append", option);}sorting.value=state.sort;
    const vocabulary=lensVocabulary(context.catalog,key),titleCounts=new Map();
    for(const item of vocabulary)titleCounts.set(item.title,(titleCounts.get(item.title)||0)+1);
    const caption=()=>uiText(summary, title+' · '+(draft[key].length?ui("{0} выбрано", [draft[key].length]):ui("любые")));caption();
    function redraw(){
      const scroll=list.scrollTop;uiChildren(list, "replaceChildren");
      const groups=vocabularyGroups(vocabulary,{sources:draft.sources,selected:draft[key],query:state.query,sort:state.sort});
      for(const group of groups){
        const section=el('details','','sc-builder-choice-group'),heading=el('summary',group.title+' · '+group.items.length);
        section.dataset.group=group.key;
        section.open=state.query.trim()!==''||group.items.some(item=>item.selected)||state.expanded.get(group.key)===true;
        section.addEventListener('toggle',()=>state.expanded.set(group.key,section.open));uiChildren(section, "append", heading);
        if(group.key==='unavailable')uiChildren(section, "append", el('p',ui("Эти условия сохраняются. Снимите их или включите соответствующий источник."),'sc-builder-note'));
        const limit=state.limits.get(group.key)||40;
        for(const item of group.items.slice(0,limit)){
          const input=el('input');input.type='checkbox';input.value=item.id;input.checked=item.selected;
          input.addEventListener('change',()=>{draft[key]=input.checked?[...draft[key],item.id]:draft[key].filter(id=>id!==item.id);caption();touch();});
          const label=el('label','','sc-builder-choice'),name=el('span',item.title);
          if(titleCounts.get(item.title)>1)uiChildren(name, "append", el('small',item.id));
          uiChildren(label, "append", input, name);uiChildren(section, "append", label);
        }
        if(group.items.length>limit)uiChildren(section, "append", button(ui("Ещё варианты · {0}", [(group.items.length-limit)]),()=>{state.limits.set(group.key,limit+40);redraw();},'sc-builder-link'));
        uiChildren(list, "append", section);
      }
      if(!groups.length)uiChildren(list, "append", el('p',ui("Нет вариантов для этих источников и поиска."),'sc-builder-note'));
      list.scrollTop=scroll;
    }
    search.addEventListener('input',()=>{state.query=search.value;redraw();});
    sorting.addEventListener('change',()=>{state.sort=sorting.value;redraw();});
    choiceUpdates.set(key,redraw);redraw();
    const controls=el('div','','sc-builder-choice-controls');uiChildren(controls, "append", search, sorting);
    uiChildren(details, "append", summary, controls, list, el('p',ui("Типы для выбранных источников. Технические — в отдельной группе."),'sc-builder-note'), button(ui("Сбросить выбор"),()=>{draft[key]=[];caption();redraw();touch();},'sc-builder-link'));return details;
  }
  function render(){
    const scroll=body.scrollTop;uiChildren(body, "replaceChildren");choiceUpdates.clear();
    if(!context){uiChildren(body, "append", el('p',busy?ui("Загружаю доступные источники и условия…"):ui("Словарь данных пока недоступен."),'sc-builder-note'));if(!busy)uiChildren(body, "append", button(ui("Повторить загрузку"),()=>void loadCatalog()));renderResult();return;}
    const nameInput=el('input');nameInput.type='text';nameInput.maxLength=64;nameInput.value=draft.name;nameInput.addEventListener('input',()=>{draft.name=nameInput.value;touch();});uiChildren(body, "append", field(ui("Название линзы"),nameInput));
    if(saved.length){const chooser=el('select');uiChildren(chooser, "append", el('option',ui("Выбрать сохранённую…")));for(const item of saved){const opt=el('option',item.name);opt.value=item.name;uiChildren(chooser, "append", opt);}chooser.addEventListener('change',()=>{const found=saved.find(s=>s.name===chooser.value);if(found){draft=structuredClone(found);touch();render();}});uiChildren(body, "append", field(ui("Мои линзы"),chooser));}
    uiChildren(body, "append", select(ui("Отправная точка"),'scope',[['area',ui("Из исходной области")],['focus',ui("От выбранной звезды")],['all',ui("По всему древу")]]));
    if(draft.scope==='area')uiChildren(body, "append", el('p',ui("Исходная область: {0}. Фильтры выбирают начало; глубина добавляет окружение.", [count(draft.nodeIds.length,ui("звезда"),ui("звезды"),ui("звёзд"))]),'sc-builder-note'));
    if(draft.scope==='focus'){
      const raw=scene.port.node(draft.focusId)||origin?.nodes.find(n=>n.id===draft.focusId);uiChildren(body, "append", el('p',raw?localized(raw.display.title):draft.focusId?ui("Звезда из сохранённой линзы."):ui("Сначала выберите звезду в пространстве."),'sc-builder-focus'));
      const selected=scene.port.selection.nodeId;if(selected&&selected!==draft.focusId)uiChildren(body, "append", button(ui("Взять выбранную звезду"),()=>{draft.focusId=selected;touch();render();},'sc-builder-link'));
      uiChildren(body, "append", el('p',ui("Центр остаётся в области. Условия ниже выбирают связи вокруг него."),'sc-builder-note'));
    }
    const sources=el('fieldset','','sc-builder-sources');uiChildren(sources, "append", el('legend',ui("Источники")));
    for(const id of context.catalog.capabilities.sources){const input=el('input');input.type='checkbox';input.checked=draft.sources.includes(id);input.addEventListener('change',()=>{draft.sources=input.checked?[...draft.sources,id]:draft.sources.filter(s=>s!==id);touch();for(const update of choiceUpdates.values())update();});const label=el('label');uiChildren(label, "append", input, el('span',sourceNames[id]||id));uiChildren(sources, "append", label);}uiChildren(body, "append", sources);
    if(draft.scope!=='focus'){
      const query=el('input');query.type='search';query.maxLength=256;query.value=draft.query;uiAttribute(query, "placeholder", ui("Имя, произведение, понятие…"));query.addEventListener('input',()=>{draft.query=query.value;touch();});uiChildren(body, "append", field(ui("Слова в исходных узлах"),query));
      uiChildren(body, "append", choices(ui("Типы узлов"),'kinds'));
    }
    if(draft.scope!=='focus'||draft.conditions.nodes.length)uiChildren(body, "append", createConditionEditor({draft,context,kind:'nodes',onChange:touch}));
    const relations=el('input');relations.type='checkbox';relations.checked=draft.relations;relations.addEventListener('change',()=>{draft.relations=relations.checked;touch();render();});const withRelations=el('label','','sc-builder-toggle');uiChildren(withRelations, "append", relations, el('span',ui("Показывать связи и окружение")));uiChildren(body, "append", withRelations);
    if(draft.relations)uiChildren(body, "append", choices(ui("Типы связей"),'predicates'));
    if(draft.relations||draft.conditions.relations.length)uiChildren(body, "append", createConditionEditor({draft,context,kind:'relations',onChange:touch}));
    const options=el('div','','sc-builder-grid');
    if(draft.relations){uiChildren(options, "append", select(ui("Глубина"),'depth',[[0,ui("Только исходные")],[1,ui("1 шаг")],[2,ui("2 шага")],[3,ui("3 шага")]]), select(ui("Направление"),'direction',[['either',ui("В обе стороны")],['outgoing',ui("По связям →")],['incoming',ui("Против связей ←")]]), select(ui("Подробность связей"),'profile',[['all',ui("Все типы, включая текст")],['overview',ui("Обзор без структуры текста")]]));}
    uiChildren(options, "append", select(ui("Звёзд в области"),'limit',[[10,ui("До 10")],[20,ui("До 20")],[40,ui("До 40")]]));uiChildren(body, "append", options);
    uiChildren(body, "append", el('p',ui("Линза меняет способ просмотра. Фильтры не меняют источники, связи или их статус."),'sc-builder-note'));
    renderResult();body.scrollTop=scroll;
  }
  function renderResult(){
    if(result.parentElement!==body)uiChildren(body, "append", result);
    uiChildren(result, "replaceChildren");uiChildren(footer, "replaceChildren");uiText(status, stale?ui("Область изменилась. Откройте конструктор заново для нового вида."):failure||storageError||notice||(scheduled?ui("Обновлю пространство…"):busy?ui("Обновляю пространство…"):applied?(delta&&Object.values(delta).every(d=>d.added===0&&d.removed===0)?ui("Условия применены. Состав этой области совпал."):ui("В пространстве: {0} · {1}", [count(preview.nodes.length,ui("звезда"),ui("звезды"),ui("звёзд")), count(preview.relations.length,ui("связь"),ui("связи"),ui("связей"))])):preview&&!preview.nodes.length?ui("Ничего не найдено. Предыдущий вид сохранён."):ui("Измените условие — результат появится в пространстве.")));
    if(context){
      const edited=el('details','','sc-query-description');uiChildren(edited, "append", el('summary',ui("Редактируемые условия")));
      const list=el('ul');for(const line of queryLines(draft))uiChildren(list, "append", el('li',line));uiChildren(edited, "append", list);uiChildren(result, "append", edited);
      const current=draftForPacket(scene.port.packet);
      if(current&&(JSON.stringify(current)!==JSON.stringify(draft)||context.catalog.source_revision!==scene.port.packet.source_revision)){
        const shown=el('details','','sc-query-description');uiChildren(shown, "append", el('summary',ui("Сейчас в пространстве: {0}", [current.name])));
        const sameRevision=context.catalog.source_revision===scene.port.packet.source_revision;
        if(!sameRevision)uiChildren(shown, "append", el('p',ui("Эта область получена из предыдущего снимка. Названия свойств из нового каталога к ней не применяются.")));
        const lines=el('ul');for(const line of queryLines(current,sameRevision?context:null))uiChildren(lines, "append", el('li',line));uiChildren(shown, "append", lines);uiChildren(result, "append", shown);
      }
      if(preview){const summary=summarizeLens(preview);uiChildren(result, "append", el('strong',`${count(summary.nodes,ui("звезда"),ui("звезды"),ui("звёзд"))} · ${count(summary.relations,ui("связь"),ui("связи"),ui("связей"))}`));
        uiChildren(result, "append", el('p',ui("Условиями выбрано: {0}. Из показанных узлов добавлено окружением: {1}.", [summary.matched, summary.context])));
        if(summary.limited)uiChildren(result, "append", el('p',ui("Результат ограничен размером области. Для другого среза уточните условия."),'sc-builder-warning'));
        if(delta)uiChildren(result, "append", el('p',ui("Изменение области: звёзды +{0} / −{1}; связи +{2} / −{3}.", [delta.nodes.added, delta.nodes.removed, delta.relations.added, delta.relations.removed])));
        if(!summary.nodes){
          uiChildren(result, "append", el('p',ui("В выбранных источниках и области совпадений нет. Это не означает, что таких материалов нет во всём древе.")));
          const help=el('div','','sc-empty-actions');
          if(draft.scope==='area')uiChildren(help, "append", button(ui("Искать по всему древу"),()=>{draft.scope='all';touch();render();}));
          if(draft.query)uiChildren(help, "append", button(ui("Убрать текстовый поиск"),()=>{draft.query='';touch();render();}));
          const kind=draft.scope!=='focus'&&draft.conditions.nodes.length?'nodes':draft.relations&&draft.conditions.relations.length?'relations':null;
          if(kind)uiChildren(help, "append", button(ui("Убрать последнее условие {0}", [(kind==='nodes'?ui("узла"):ui("связи"))]),()=>{draft.conditions[kind].pop();touch();render();}));
          uiChildren(result, "append", help);
        }
      }else uiChildren(result, "append", el('p',ui("Изменения применяются автоматически; исходный вид можно вернуть.")));
      const apply=button(busy?ui("Обновляю…"):ui("Обновить пространство"),()=>void run(),'sc-builder-primary');apply.disabled=busy||stale;uiChildren(footer, "append", apply);
      const save=button(ui("Сохранить линзу"),()=>{onUserAction();try{compileDraft(draft,context);saved=saveDraft(localStorage,draft);storageError='';failure='';notice=ui("Линза сохранена в этом браузере.");render();}catch(error){failure=error.message||ui("Не удалось сохранить линзу.");renderResult();}});save.disabled=busy||scheduled||stale;uiChildren(footer, "append", save);
      if(failure)uiChildren(footer, "append", button(ui("Обновить каталог"),()=>void loadCatalog(),'sc-builder-link'));
    }
    const previous=button(ui("← Предыдущий вид"),()=>{onUserAction();stop();root.querySelector('.sc-back').click();resetForArea();close();},'sc-builder-link');previous.disabled=root.dataset.history==='0';uiChildren(footer, "append", previous);
    const back=button(ui("↶ К исходному виду"),()=>{onUserAction();stop();applying=true;try{if(bookmark)scene.port.restoreView(bookmark);resetForArea();}finally{applying=false;}close();},'sc-builder-link');back.disabled=!bookmark;uiChildren(footer, "append", back);
    updateActiveQuery();
    scene.invalidate();
  }
  async function run(){
    if(stale)return;onUserAction();stop();scene.ui.cancelPending();failure='';notice='';applied=false;delta=null;const token=generation;busy=true;renderResult();
    try{
      encodeDraft(draft);const pending=structuredClone(draft);
      const answer=await requests.run('compile',signal=>previewDraft(client,pending,context,signal));
      if(!answer.current||token!==generation||panel.hidden)return;
      preview=answer.value;
      if(preview.nodes.length){delta=lensDelta(scene.port.packet,preview);applying=true;try{scene.port.setGraph(preview);basePacket=scene.port.packet;applied=true;}finally{applying=false;}}
    }catch(error){if(token===generation){failure=error.message||ui("Не удалось собрать линзу.");preview=null;}}
    finally{if(token===generation){busy=false;renderResult();scene.invalidate();}}
  }
  function selectionChanged(){
    updateActiveQuery();
    if(applying||panel.hidden||scene.port.packet===basePacket)return;
    stop();preview=null;stale=true;renderResult();
  }
  panels.configure('builder',{onResume:()=>{if(!context)void loadCatalog();else renderResult();}});
  addEventListener('pagehide',stop);refreshIcons();return {selectionChanged};
}
