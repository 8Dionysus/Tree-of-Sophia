import {RequestSlots,localized} from './knowledge-client.mjs';
import {constructorCatalog,initialDraft,compileDraft,previewDraft,summarizeLens,lensDelta,encodeDraft,draftForPacket,readSaved,saveDraft} from './lens-model.mjs';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {createConditionEditor} from './lens-condition-editor.mjs';
import {conditionCatalog,conditionText} from './lens-conditions.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(label,action,className='sc-builder-button')=>{const b=el('button',label,className);b.type='button';b.addEventListener('click',action);return b;};
const plural=new Intl.PluralRules('ru');
const count=(n,one,few,many)=>`${n} ${{one,few,many}[plural.select(n)]||many}`;
const sourceNames={'philosophy':'Философский атлас','canon':'Канон','candidate-intake':'Исследовательские кандидаты','source-navigation':'Произведения и источники','source-claims':'Утверждения источников','semantic-interchange':'Понятия и типы','repository':'Карта проекта'};

export function createLensPanel(root,scene,panels,{data:{client},onUserAction}){
  const requests=new RequestSlots();
  let context=null,draft=null,preview=null,bookmark=null,origin=null,basePacket=null,busy=false,failure='',storageError='',applied=false,applying=false;
  let saved=[],returnFocus=null,generation=0,stale=false,notice='',timer=null,scheduled=false,delta=null;
  const choiceViews=new Map(),choiceUpdates=new Map();
  const panel=el('section','','sc-panel sc-builder');panel.hidden=true;panel.setAttribute('aria-label','Конструктор линз');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ОПТИКА МЫСЛИ</span><button class="sc-icon sc-builder-close" type="button" aria-label="Закрыть конструктор"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-builder-heading"><span aria-hidden="true">◈</span><div><h3>Собрать линзу</h3><p>Настройки сразу меняют пространство.</p></div></div><div class="sc-builder-body"></div><div class="sc-builder-result" aria-live="polite"></div><div class="sc-builder-status" role="status"></div><div class="sc-builder-footer"></div>';
  root.append(panel);
  const body=panel.querySelector('.sc-builder-body'),result=panel.querySelector('.sc-builder-result'),status=panel.querySelector('.sc-builder-status'),footer=panel.querySelector('.sc-builder-footer');
  const opener=button('Конструктор линз',()=>{onUserAction();void open();},'sc-builder-open');
  root.querySelector('.sc-lenses').append(opener);
  const headerOpener=root.querySelector('.sc-lenses-open');
  const activeQuery=button('',()=>{onUserAction();void open();},'sc-active-query');activeQuery.hidden=true;
  activeQuery.setAttribute('aria-label','Условия отображённой области');root.querySelector('.sc-context').append(activeQuery);
  function queryLines(value,lookup=context){
    const nodeEntries=lookup?conditionCatalog(lookup,'nodes'):[],relationEntries=lookup?conditionCatalog(lookup,'relations'):[];
    const lines=[value.scope==='area'?`Из исходной области · ${value.nodeIds.length}`:value.scope==='focus'?'От выбранной звезды':'По всему древу'];
    lines.push('Источники: '+value.sources.map(id=>sourceNames[id]||id).join(', '));
    if(value.scope!=='focus'){
      if(value.query)lines.push('Поиск: «'+value.query+'»');
      if(value.kinds.length)lines.push('Типы узлов: '+value.kinds.map(id=>localized(lookup?.catalog.node_kinds.find(k=>k.kind_id===id)?.display,id)).join(', '));
      for(const rule of value.conditions.nodes)lines.push(conditionText(rule,nodeEntries));
    }
    if(value.relations){
      if(value.predicates.length)lines.push('Типы связей: '+value.predicates.map(id=>localized(lookup?.catalog.predicates.find(k=>k.predicate_id===id)?.display,id)).join(', '));
      for(const rule of value.conditions.relations)lines.push('Связь: '+conditionText(rule,relationEntries));
      lines.push(`Окружение: ${value.depth} · ${({either:'в обе стороны',outgoing:'по связям',incoming:'против связей'})[value.direction]} · ${value.profile==='all'?'все типы':'обзор'}`);
    }else lines.push('Без связей');
    lines.push('До '+value.limit+' звёзд');return lines;
  }
  function updateActiveQuery(){
    const value=draftForPacket(scene.port.packet);activeQuery.hidden=!value;
    if(value){activeQuery.textContent='Линза: '+value.name+' · условия';activeQuery.dataset.tooltip=queryLines(value,context?.catalog.source_revision===scene.port.packet.source_revision?context:null).slice(0,2).join(' · ').slice(0,250);}
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
    try{saved=readSaved(localStorage);storageError='';}catch(error){saved=[];storageError=error.message||'Локальное хранилище недоступно.';}
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
    }catch(error){if(token===generation)failure=error.message||'Не удалось загрузить словарь данных.';}
    finally{if(token===generation){busy=false;render();}}
  }
  function touch({composing=false}={}){
    onUserAction();stop();failure='';notice='';preview=null;applied=false;delta=null;
    if(context&&!stale&&!composing){scheduled=true;timer=setTimeout(()=>{timer=null;void run();},400);}
    renderResult();
  }
  function field(label,input){const wrap=el('label','','sc-builder-field');wrap.append(el('span',label),input);return wrap;}
  function select(label,key,choices){
    if(!choices.some(([value])=>String(value)===String(draft[key])))choices=[...choices,[draft[key],String(draft[key])]];
    const input=el('select');for(const [value,title]of choices){const option=el('option',title);option.value=String(value);input.append(option);}input.value=String(draft[key]);
    input.addEventListener('change',()=>{draft[key]=['limit','depth'].includes(key)?Number(input.value):input.value;touch();if(key==='scope')render();});return field(label,input);
  }
  function choices(title,key){
    const state=choiceViews.get(key)||{open:false,query:'',sort:'alphabet',expanded:new Map(),limits:new Map()};choiceViews.set(key,state);
    const details=el('details','','sc-builder-choices'),summary=el('summary'),list=el('div','','sc-builder-options'),search=el('input');
    details.open=state.open;details.addEventListener('toggle',()=>state.open=details.open);
    search.type='search';search.placeholder='Найти в списке…';search.setAttribute('aria-label','Найти: '+title);search.value=state.query;
    const sorting=el('select');sorting.setAttribute('aria-label','Порядок: '+title);
    for(const [id,name]of [['alphabet','По алфавиту'],['frequency','Сначала частые']]){const option=el('option',name);option.value=id;sorting.append(option);}sorting.value=state.sort;
    const vocabulary=lensVocabulary(context.catalog,key),titleCounts=new Map();
    for(const item of vocabulary)titleCounts.set(item.title,(titleCounts.get(item.title)||0)+1);
    const caption=()=>summary.textContent=title+' · '+(draft[key].length?draft[key].length+' выбрано':'любые');caption();
    function redraw(){
      const scroll=list.scrollTop;list.replaceChildren();
      const groups=vocabularyGroups(vocabulary,{sources:draft.sources,selected:draft[key],query:state.query,sort:state.sort});
      for(const group of groups){
        const section=el('details','','sc-builder-choice-group'),heading=el('summary',group.title+' · '+group.items.length);
        section.dataset.group=group.key;
        section.open=state.query.trim()!==''||group.items.some(item=>item.selected)||state.expanded.get(group.key)===true;
        section.addEventListener('toggle',()=>state.expanded.set(group.key,section.open));section.append(heading);
        if(group.key==='unavailable')section.append(el('p','Эти условия сохраняются. Снимите их или включите соответствующий источник.','sc-builder-note'));
        const limit=state.limits.get(group.key)||40;
        for(const item of group.items.slice(0,limit)){
          const input=el('input');input.type='checkbox';input.value=item.id;input.checked=item.selected;
          input.addEventListener('change',()=>{draft[key]=input.checked?[...draft[key],item.id]:draft[key].filter(id=>id!==item.id);caption();touch();});
          const label=el('label','','sc-builder-choice'),name=el('span',item.title);
          if(titleCounts.get(item.title)>1)name.append(el('small',item.id));
          label.append(input,name);section.append(label);
        }
        if(group.items.length>limit)section.append(button('Ещё варианты · '+(group.items.length-limit),()=>{state.limits.set(group.key,limit+40);redraw();},'sc-builder-link'));
        list.append(section);
      }
      if(!groups.length)list.append(el('p','Нет вариантов для этих источников и поиска.','sc-builder-note'));
      list.scrollTop=scroll;
    }
    search.addEventListener('input',()=>{state.query=search.value;redraw();});
    sorting.addEventListener('change',()=>{state.sort=sorting.value;redraw();});
    choiceUpdates.set(key,redraw);redraw();
    const controls=el('div','','sc-builder-choice-controls');controls.append(search,sorting);
    details.append(summary,controls,list,el('p','Типы для выбранных источников. Технические — в отдельной группе.','sc-builder-note'),button('Сбросить выбор',()=>{draft[key]=[];caption();redraw();touch();},'sc-builder-link'));return details;
  }
  function render(){
    const scroll=body.scrollTop;body.replaceChildren();choiceUpdates.clear();
    if(!context){body.append(el('p',busy?'Загружаю доступные источники и условия…':'Словарь данных пока недоступен.','sc-builder-note'));if(!busy)body.append(button('Повторить загрузку',()=>void loadCatalog()));renderResult();return;}
    const nameInput=el('input');nameInput.type='text';nameInput.maxLength=64;nameInput.value=draft.name;nameInput.addEventListener('input',()=>{draft.name=nameInput.value;touch();});body.append(field('Название линзы',nameInput));
    if(saved.length){const chooser=el('select');chooser.append(el('option','Выбрать сохранённую…'));for(const item of saved){const opt=el('option',item.name);opt.value=item.name;chooser.append(opt);}chooser.addEventListener('change',()=>{const found=saved.find(s=>s.name===chooser.value);if(found){draft=structuredClone(found);touch();render();}});body.append(field('Мои линзы',chooser));}
    body.append(select('Отправная точка','scope',[['area','Из исходной области'],['focus','От выбранной звезды'],['all','По всему древу']]));
    if(draft.scope==='area')body.append(el('p',`Исходная область: ${count(draft.nodeIds.length,'звезда','звезды','звёзд')}. Фильтры выбирают начало; глубина добавляет окружение.`,'sc-builder-note'));
    if(draft.scope==='focus'){
      const raw=scene.port.node(draft.focusId)||origin?.nodes.find(n=>n.id===draft.focusId);body.append(el('p',raw?localized(raw.display.title):draft.focusId?'Звезда из сохранённой линзы.':'Сначала выберите звезду в пространстве.','sc-builder-focus'));
      const selected=scene.port.selection.nodeId;if(selected&&selected!==draft.focusId)body.append(button('Взять выбранную звезду',()=>{draft.focusId=selected;touch();render();},'sc-builder-link'));
      body.append(el('p','Центр остаётся в области. Условия ниже выбирают связи вокруг него.','sc-builder-note'));
    }
    const sources=el('fieldset','','sc-builder-sources');sources.append(el('legend','Источники'));
    for(const id of context.catalog.capabilities.sources){const input=el('input');input.type='checkbox';input.checked=draft.sources.includes(id);input.addEventListener('change',()=>{draft.sources=input.checked?[...draft.sources,id]:draft.sources.filter(s=>s!==id);touch();for(const update of choiceUpdates.values())update();});const label=el('label');label.append(input,el('span',sourceNames[id]||id));sources.append(label);}body.append(sources);
    if(draft.scope!=='focus'){
      const query=el('input');query.type='search';query.maxLength=256;query.value=draft.query;query.placeholder='Имя, произведение, понятие…';query.addEventListener('input',()=>{draft.query=query.value;touch();});body.append(field('Слова в исходных узлах',query));
      body.append(choices('Типы узлов','kinds'));
    }
    if(draft.scope!=='focus'||draft.conditions.nodes.length)body.append(createConditionEditor({draft,context,kind:'nodes',onChange:touch}));
    const relations=el('input');relations.type='checkbox';relations.checked=draft.relations;relations.addEventListener('change',()=>{draft.relations=relations.checked;touch();render();});const withRelations=el('label','','sc-builder-toggle');withRelations.append(relations,el('span','Показывать связи и окружение'));body.append(withRelations);
    if(draft.relations)body.append(choices('Типы связей','predicates'));
    if(draft.relations||draft.conditions.relations.length)body.append(createConditionEditor({draft,context,kind:'relations',onChange:touch}));
    const options=el('div','','sc-builder-grid');
    if(draft.relations){options.append(select('Глубина','depth',[[0,'Только исходные'],[1,'1 шаг'],[2,'2 шага'],[3,'3 шага']]),select('Направление','direction',[['either','В обе стороны'],['outgoing','По связям →'],['incoming','Против связей ←']]),select('Подробность связей','profile',[['all','Все типы, включая текст'],['overview','Обзор без структуры текста']]));}
    options.append(select('Звёзд в области','limit',[[10,'До 10'],[20,'До 20'],[40,'До 40']]));body.append(options);
    body.append(el('p','Линза меняет способ просмотра. Фильтры не меняют источники, связи или их статус.','sc-builder-note'));
    renderResult();body.scrollTop=scroll;
  }
  function renderResult(){
    if(result.parentElement!==body)body.append(result);
    result.replaceChildren();footer.replaceChildren();status.textContent=stale?'Область изменилась. Откройте конструктор заново для нового вида.':failure||storageError||notice||(scheduled?'Обновлю пространство…':busy?'Обновляю пространство…':applied?(delta&&Object.values(delta).every(d=>d.added===0&&d.removed===0)?'Условия применены. Состав этой области совпал.':`В пространстве: ${count(preview.nodes.length,'звезда','звезды','звёзд')} · ${count(preview.relations.length,'связь','связи','связей')}`):preview&&!preview.nodes.length?'Ничего не найдено. Предыдущий вид сохранён.':'Измените условие — результат появится в пространстве.');
    if(context){
      const edited=el('details','','sc-query-description');edited.append(el('summary','Редактируемые условия'));
      const list=el('ul');for(const line of queryLines(draft))list.append(el('li',line));edited.append(list);result.append(edited);
      const current=draftForPacket(scene.port.packet);
      if(current&&(JSON.stringify(current)!==JSON.stringify(draft)||context.catalog.source_revision!==scene.port.packet.source_revision)){
        const shown=el('details','','sc-query-description');shown.append(el('summary','Сейчас в пространстве: '+current.name));
        const sameRevision=context.catalog.source_revision===scene.port.packet.source_revision;
        if(!sameRevision)shown.append(el('p','Эта область получена из предыдущего снимка. Названия свойств из нового каталога к ней не применяются.'));
        const lines=el('ul');for(const line of queryLines(current,sameRevision?context:null))lines.append(el('li',line));shown.append(lines);result.append(shown);
      }
      if(preview){const summary=summarizeLens(preview);result.append(el('strong',`${count(summary.nodes,'звезда','звезды','звёзд')} · ${count(summary.relations,'связь','связи','связей')}`));
        result.append(el('p',`Условиями выбрано: ${summary.matched}. Из показанных узлов добавлено окружением: ${summary.context}.`));
        if(summary.limited)result.append(el('p','Результат ограничен размером области. Для другого среза уточните условия.','sc-builder-warning'));
        if(delta)result.append(el('p',`Изменение области: звёзды +${delta.nodes.added} / −${delta.nodes.removed}; связи +${delta.relations.added} / −${delta.relations.removed}.`));
        if(!summary.nodes){
          result.append(el('p','В выбранных источниках и области совпадений нет. Это не означает, что таких материалов нет во всём древе.'));
          const help=el('div','','sc-empty-actions');
          if(draft.scope==='area')help.append(button('Искать по всему древу',()=>{draft.scope='all';touch();render();}));
          if(draft.query)help.append(button('Убрать текстовый поиск',()=>{draft.query='';touch();render();}));
          const kind=draft.scope!=='focus'&&draft.conditions.nodes.length?'nodes':draft.relations&&draft.conditions.relations.length?'relations':null;
          if(kind)help.append(button('Убрать последнее условие '+(kind==='nodes'?'узла':'связи'),()=>{draft.conditions[kind].pop();touch();render();}));
          result.append(help);
        }
      }else result.append(el('p','Изменения применяются автоматически; исходный вид можно вернуть.'));
      const apply=button(busy?'Обновляю…':'Обновить пространство',()=>void run(),'sc-builder-primary');apply.disabled=busy||stale;footer.append(apply);
      const save=button('Сохранить линзу',()=>{onUserAction();try{compileDraft(draft,context);saved=saveDraft(localStorage,draft);storageError='';failure='';notice='Линза сохранена в этом браузере.';render();}catch(error){failure=error.message||'Не удалось сохранить линзу.';renderResult();}});save.disabled=busy||scheduled||stale;footer.append(save);
      if(failure)footer.append(button('Обновить каталог',()=>void loadCatalog(),'sc-builder-link'));
    }
    const previous=button('← Предыдущий вид',()=>{onUserAction();stop();root.querySelector('.sc-back').click();resetForArea();close();},'sc-builder-link');previous.disabled=root.dataset.history==='0';footer.append(previous);
    const back=button('↶ К исходному виду',()=>{onUserAction();stop();applying=true;try{if(bookmark)scene.port.restoreView(bookmark);resetForArea();}finally{applying=false;}close();},'sc-builder-link');back.disabled=!bookmark;footer.append(back);
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
    }catch(error){if(token===generation){failure=error.message||'Не удалось собрать линзу.';preview=null;}}
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
