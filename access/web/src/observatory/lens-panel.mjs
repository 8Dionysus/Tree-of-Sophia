import {KnowledgeClient,RequestSlots,localized} from './knowledge-client.mjs';
import {constructorCatalog,initialDraft,compileDraft,previewDraft,confirmDraft,summarizeLens,encodeDraft,draftForPacket,readSaved,saveDraft} from './lens-model.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(label,action,className='sc-builder-button')=>{const b=el('button',label,className);b.type='button';b.addEventListener('click',action);return b;};
const plural=new Intl.PluralRules('ru');
const count=(n,one,few,many)=>`${n} ${{one,few,many}[plural.select(n)]||many}`;
const sourceNames={'philosophy':'Философский атлас','canon':'Канон','candidate-intake':'Исследовательские кандидаты','source-navigation':'Произведения и источники','source-claims':'Утверждения источников','semantic-interchange':'Понятия и типы','repository':'Карта проекта'};

export function createLensPanel(root,scene,panels,{onUserAction}){
  const client=new KnowledgeClient(),requests=new RequestSlots();
  let context=null,draft=null,preview=null,bookmark=null,origin=null,basePacket=null,busy=false,failure='',storageError='',applied=false,applying=false;
  let saved=[],returnFocus=null,generation=0,stale=false,notice='';
  const panel=el('section','','sc-panel sc-builder');panel.hidden=true;panel.setAttribute('aria-label','Конструктор линз');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ОПТИКА МЫСЛИ</span><button class="sc-icon sc-builder-close" type="button" aria-label="Закрыть конструктор"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-builder-heading"><span aria-hidden="true">◈</span><div><h3>Собрать линзу</h3><p>Настройте, что видно в пространстве.</p></div></div><div class="sc-builder-body"></div><div class="sc-builder-result" aria-live="polite"></div><div class="sc-builder-status" role="status"></div><div class="sc-builder-footer"></div>';
  root.append(panel);
  const body=panel.querySelector('.sc-builder-body'),result=panel.querySelector('.sc-builder-result'),status=panel.querySelector('.sc-builder-status'),footer=panel.querySelector('.sc-builder-footer');
  const opener=button('Конструктор линз',()=>{onUserAction();void open();},'sc-builder-open');
  root.querySelector('.sc-lenses').append(opener);
  const headerOpener=root.querySelector('.sc-lenses-open');
  const stop=()=>{generation++;requests.cancelAll();busy=false;};
  panels.register('builder',panel,stop);
  function close(){panels.close('builder');(returnFocus?.isConnected&&!returnFocus.closest('[hidden]')?returnFocus:headerOpener).focus();}
  panel.querySelector('.sc-builder-close').addEventListener('click',()=>{onUserAction();close();});
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.stopPropagation();event.preventDefault();onUserAction();close();}});
  function resetForArea(){origin=scene.port.packet;basePacket=origin;bookmark=origin?scene.port.captureView():null;preview=null;applied=false;draft=null;stale=false;}
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
  function touch(){onUserAction();stop();failure='';notice='';preview=null;applied=false;renderResult();}
  function field(label,input){const wrap=el('label','','sc-builder-field');wrap.append(el('span',label),input);return wrap;}
  function select(label,key,choices){
    if(!choices.some(([value])=>String(value)===String(draft[key])))choices=[...choices,[draft[key],String(draft[key])]];
    const input=el('select');for(const [value,title]of choices){const option=el('option',title);option.value=String(value);input.append(option);}input.value=String(draft[key]);
    input.addEventListener('change',()=>{draft[key]=['limit','depth'].includes(key)?Number(input.value):input.value;touch();if(key==='scope')render();});return field(label,input);
  }
  function choices(title,key,items){
    const details=el('details','','sc-builder-choices'),summary=el('summary'),list=el('div','','sc-builder-options'),search=el('input');search.type='search';search.placeholder='Найти в списке…';search.setAttribute('aria-label','Найти: '+title);
    const titleCounts=new Map();for(const item of items)titleCounts.set(item.title,(titleCounts.get(item.title)||0)+1);
    const caption=()=>summary.textContent=title+' · '+(draft[key].length||'любые');caption();
    function redraw(){
      list.replaceChildren();const query=search.value.toLocaleLowerCase();const matches=items.filter(item=>(item.title+' '+item.id).toLocaleLowerCase().includes(query)).sort((a,b)=>Number(draft[key].includes(b.id))-Number(draft[key].includes(a.id)));
      for(const item of matches.slice(0,60)){
        const input=el('input');input.type='checkbox';input.value=item.id;input.checked=draft[key].includes(item.id);
        input.addEventListener('change',()=>{draft[key]=input.checked?[...draft[key],item.id]:draft[key].filter(id=>id!==item.id);caption();touch();});
        const label=el('label','','sc-builder-choice'),caption=el('span',item.title);if(titleCounts.get(item.title)>1)caption.append(el('small',item.id));label.append(input,caption);list.append(label);
      }
      if(!matches.length)list.append(el('p','Нет подходящих вариантов.','sc-builder-note'));
      if(matches.length>60)list.append(el('p','Показаны первые 60. Уточните название.','sc-builder-note'));
    }
    search.addEventListener('input',redraw);redraw();
    details.append(summary,search,list,button('Сбросить выбор',()=>{draft[key]=[];caption();redraw();touch();},'sc-builder-link'));return details;
  }
  function render(){
    body.replaceChildren();
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
    for(const id of context.catalog.capabilities.sources){const input=el('input');input.type='checkbox';input.checked=draft.sources.includes(id);input.addEventListener('change',()=>{draft.sources=input.checked?[...draft.sources,id]:draft.sources.filter(s=>s!==id);touch();});const label=el('label');label.append(input,el('span',sourceNames[id]||id));sources.append(label);}body.append(sources);
    if(draft.scope!=='focus'){
      const query=el('input');query.type='search';query.maxLength=256;query.value=draft.query;query.placeholder='Имя, произведение, понятие…';query.addEventListener('input',()=>{draft.query=query.value;touch();});body.append(field('Слова в исходных узлах',query));
      body.append(choices('Типы узлов','kinds',context.catalog.node_kinds.map(k=>({id:k.kind_id,title:localized(k.display,k.kind_id)}))));
    }
    const relations=el('input');relations.type='checkbox';relations.checked=draft.relations;relations.addEventListener('change',()=>{draft.relations=relations.checked;touch();render();});const withRelations=el('label','','sc-builder-toggle');withRelations.append(relations,el('span','Показывать связи и окружение'));body.append(withRelations);
    if(draft.relations)body.append(choices('Типы связей','predicates',context.catalog.predicates.map(p=>({id:p.predicate_id,title:localized(p.display,p.predicate_id)}))));
    const options=el('div','','sc-builder-grid');
    if(draft.relations){options.append(select('Глубина','depth',[[0,'Только исходные'],[1,'1 шаг'],[2,'2 шага'],[3,'3 шага']]),select('Направление','direction',[['either','В обе стороны'],['outgoing','По связям →'],['incoming','Против связей ←']]),select('Подробность связей','profile',[['all','Все типы, включая текст'],['overview','Обзор без структуры текста']]));}
    options.append(select('Звёзд в области','limit',[[10,'До 10'],[20,'До 20'],[40,'До 40']]));body.append(options);
    body.append(el('p','Линза меняет способ просмотра. Фильтры не меняют источники, связи или их статус.','sc-builder-note'));
    renderResult();
  }
  function renderResult(){
    if(result.parentElement!==body)body.append(result);
    result.replaceChildren();footer.replaceChildren();status.textContent=stale?'Область изменилась. Откройте конструктор заново для нового вида.':failure||storageError||notice;
    if(context){
      if(preview){const summary=summarizeLens(preview);result.append(el('strong',`${count(summary.nodes,'звезда','звезды','звёзд')} · ${count(summary.relations,'связь','связи','связей')}`));
        result.append(el('p',`Условиями выбрано: ${summary.matched}. Из показанных узлов добавлено окружением: ${summary.context}.`));
        if(summary.limited)result.append(el('p','Результат ограничен размером области. Для другого среза уточните условия.','sc-builder-warning'));
        if(!summary.nodes)result.append(el('p','Ничего не найдено. Пространство сохранено. Измените условия.'));
      }else result.append(el('p',busy?'Проверяю состав…':'Измените условия и посмотрите состав перед применением.'));
      const inspect=button(busy?'Проверяю…':'Посмотреть состав',()=>void run(false));inspect.disabled=busy||stale;footer.append(inspect);
      const apply=button(applied?'Линза применена':'Показать в пространстве',()=>void run(true),'sc-builder-primary');apply.disabled=busy||stale||!preview?.nodes.length||applied;footer.append(apply);
      const save=button('Сохранить линзу',()=>{onUserAction();try{compileDraft(draft,context);saved=saveDraft(localStorage,draft);storageError='';failure='';notice='Линза сохранена в этом браузере.';render();}catch(error){failure=error.message||'Не удалось сохранить линзу.';renderResult();}});save.disabled=busy||stale;footer.append(save);
      if(failure)footer.append(button('Обновить каталог',()=>void loadCatalog(),'sc-builder-link'));
    }
    const back=button('↶ К исходному виду',()=>{onUserAction();stop();applying=true;try{if(bookmark)scene.port.restoreView(bookmark);basePacket=scene.port.packet;applied=false;}finally{applying=false;}close();},'sc-builder-link');back.disabled=!bookmark;footer.append(back);
    scene.invalidate();
  }
  async function run(apply){
    if(stale)return;onUserAction();stop();scene.ui.cancelPending();failure='';notice='';const token=generation;busy=true;renderResult();
    try{
      encodeDraft(draft);const pending=structuredClone(draft),previous=preview;
      const answer=await requests.run('compile',signal=>apply?confirmDraft(client,pending,context,previous,signal):previewDraft(client,pending,context,signal));
      if(!answer.current||token!==generation||panel.hidden)return;
      preview=answer.value;
      if(apply){applying=true;try{scene.port.setGraph(preview);basePacket=scene.port.packet;applied=true;}finally{applying=false;}}
    }catch(error){if(token===generation){failure=error.message||'Не удалось собрать линзу.';preview=null;}}
    finally{if(token===generation){busy=false;renderResult();body.scrollTop+=result.getBoundingClientRect().bottom-body.getBoundingClientRect().bottom;scene.invalidate();}}
  }
  function selectionChanged(){
    if(applying||panel.hidden||scene.port.packet===basePacket)return;
    stop();preview=null;stale=true;renderResult();
  }
  addEventListener('pagehide',stop);refreshIcons();return {selectionChanged};
}
