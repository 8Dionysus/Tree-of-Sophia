// The action layer of a stand: one menu of actions for whatever is chosen (a star, a link, a passage or nothing),
// the same list as a command palette (Ctrl/⌘+K), the presence layer (reading, research, with agents), links to
// any subject, the tray of long tasks and the table of what agents can do. Every action calls an owner the page
// already has: a page command, the act journal, the space or the reader. Nothing here writes a source.
import './action-layer.css';
import {localeText,resolveLocale} from './locale.mjs';
import {encodeReadingRoute} from '../corpus-reader/route.mjs';
import {PRESETS,keyLabel} from './control-prefs.mjs';
import pageCommands from '../../../contracts/page-commands.v1.json' with {type:'json'};
import actCommands from '../../../contracts/act-commands.v1.json' with {type:'json'};
import knowledgeApi from '../../../contracts/knowledge-api.v1.json' with {type:'json'};

export const LAYERS=Object.freeze(['reading','research','agents']);
export const LAYER_PARAM='treeLayer';
const LAYER_GLYPH={reading:'◌',research:'◎',agents:'✦'};

const labels={
  reading:['Чтение','Reading'],
  research:['Исследование','Research'],
  agents:['С агентами','With agents'],
  layer:['Слой','Layer'],
  layerHint:['Слой присутствия: что видно поверх Древа. Клавиша L','Presence layer: what shows over the Tree. Key L'],
  palette:['Команды','Commands'],
  paletteHint:['Все действия для выбранного. Ctrl или ⌘ и K','Every action for what is chosen. Ctrl or ⌘ and K'],
  search:['Найти действие','Find an action'],
  nothing:['Ничего не найдено','Nothing found'],
  chosen:['Выбрано','Chosen'],
  noChoice:['Ничего не выбрано','Nothing chosen'],
  actions:['Действия','Actions'],
  openCard:['Открыть карточку','Open the card'],
  openArticle:['Статья в справочнике','Article in the reference'],
  compare:['Сравнить','Compare'],
  bench:['На верстак','To the workbench'],
  copyLink:['Копировать ссылку','Copy link'],
  copyViewLink:['Копировать ссылку на этот вид','Copy a link to this view'],
  copyQuote:['Копировать цитату с местом','Copy the quotation with its place'],
  copyJson:['Копировать как JSON','Copy as JSON'],
  copied:['Скопировано','Copied'],
  copyFailed:['Не удалось скопировать: выделите текст и скопируйте вручную','Could not copy: select the text and copy it by hand'],
  routes:['Маршруты','Routes'],
  route:['Маршрут','Route'],
  find:['Искать в пространстве','Search the space'],
  lens:['Линза','Lens'],
  tasks:['Задачи','Tasks'],
  tasksEmpty:['Долгих задач нет','No long tasks'],
  running:['идёт','running'],
  done:['готово','done'],
  failed:['не удалось','failed'],
  capabilities:['Возможности агентов','What agents can do'],
  capabilitiesLead:['Каждая строка — одна возможность: как её вызывает человек, страница, агент на странице и внешний агент. Пустая клетка — то, чего этот путь пока не умеет.','Each row is one capability: how the person, the page, an agent on the page and an outside agent call it. An empty cell is what that route cannot do yet.'],
  colInterface:['Интерфейс','Interface'],
  colPage:['Команда страницы','Page command'],
  colAgent:['Инструмент агента','Agent tool'],
  colNative:['tos mcp','tos mcp'],
  colCli:['tos','tos'],
  manifest:['Скопировать манифест для агента','Copy the manifest for an agent'],
  journal:['Журнал действий','Act journal'],
  journalLoaded:['записей прочитано','records read'],
  settings:['Настройки управления','Control settings'],
  preset:['Набор','Preset'],
  'preset-mouse':['Мышь','Mouse'],
  'preset-trackpad':['Трекпад','Trackpad'],
  'preset-touch':['Касание','Touch'],
  'preset-precise':['Точное','Precise'],
  'preset-custom':['Свой','Custom'],
  turnSpeed:['Скорость поворота','Turning speed'],
  zoomSpeed:['Скорость приближения','Zoom speed'],
  inertia:['Поворот докатывается после отпускания','A turn coasts after letting go'],
  invertTurn:['Обратный поворот','Inverted turning'],
  invertZoom:['Обратное приближение','Inverted zoom'],
  keys:['Клавиши','Keys'],
  pressKey:['Нажмите клавишу…','Press a key…'],
  'key-palette':['Команды (с Ctrl или ⌘)','Commands (with Ctrl or ⌘)'],
  'key-layer':['Слой','Layer'],
  'key-search':['Поиск','Search'],
  'key-frame':['Вписать всё','Fit everything'],
  'key-bench':['На верстак','To the workbench'],
  'key-expand':['Развернуть карточку','Expand the card'],
  textScale:['Размер текста для чтения','Reading text size'],
  contrast:['Контраст знаков','Contrast of signs'],
  'contrast-normal':['Обычный','Normal'],
  'contrast-high':['Высокий','High'],
  reset:['Вернуть исходные','Restore defaults'],
  close:['Закрыть','Close'],
};

const pageCommandIds=new Set(pageCommands.commands.map(command=>command.command_id));

export function mountActionLayer({shell,header,space,experience,registry=null,acts=null,api,locale=()=> 'ru',tier=()=> 'propose',controls=null}){
  const keys=()=>controls?.get?.().keys??{palette:'KeyK',layer:'KeyL'};
  const t=key=>{const pair=labels[key];if(!pair)return key;return resolveLocale(locale())==='ru'?pair[0]:localeText(pair[1],locale());};
  const doc=shell.ownerDocument,view=doc.defaultView;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const disposers=[];
  const on=(target,type,listener,options)=>{target.addEventListener(type,listener,options);disposers.push(()=>target.removeEventListener(type,listener,options));};

  // ------------------------------------------------------------ presence layer (A1)
  const validLayer=value=>LAYERS.includes(value)?value:'reading';
  let layer=validLayer(new URLSearchParams(view.location.search).get(LAYER_PARAM));
  // The bar holds one button that walks the layers; the menu under "more" holds all three.
  const layerControl=button('','al-layer-current');
  on(layerControl,'click',()=>setLayer(LAYERS[(LAYERS.indexOf(layer)+1)%LAYERS.length]));
  const layerGroup=el('div','al-layers');layerGroup.setAttribute('role','radiogroup');
  const layerButtons=LAYERS.map(id=>{
    const b=button('','al-layer');b.dataset.layer=id;b.setAttribute('role','radio');b.append(el('span','al-layer-glyph',LAYER_GLYPH[id]),el('span','al-layer-name'));
    on(b,'click',()=>setLayer(id));layerGroup.append(b);return b;
  });
  function paintLayer(){
    shell.dataset.presenceLayer=layer;doc.documentElement.dataset.presenceLayer=layer;
    layerControl.textContent=LAYER_GLYPH[layer];layerControl.dataset.layer=layer;
    layerControl.setAttribute('aria-label',`${t('layer')}: ${t(layer)}`);layerControl.title=`${t('layer')}: ${t(layer)} · ${t('layerHint')}`;
    layerGroup.setAttribute('aria-label',t('layer'));
    for(const b of layerButtons){const id=b.dataset.layer;b.setAttribute('aria-checked',String(id===layer));b.querySelector('.al-layer-name').textContent=t(id);}
  }
  function setLayer(next,{write=true}={}){
    next=validLayer(next);if(next===layer){paintLayer();return layer;}
    layer=next;paintLayer();
    if(write){const url=new URL(view.location.href);if(layer==='reading')url.searchParams.delete(LAYER_PARAM);else url.searchParams.set(LAYER_PARAM,layer);view.history.replaceState(view.history.state,'',url);}
    shell.dispatchEvent(new view.CustomEvent('public-research-layer',{bubbles:true,detail:{layer}}));
    registry?.notifyStateChange?.();
    return layer;
  }
  const tools=header?.querySelector('.main-tools');
  if(tools)tools.insertBefore(layerControl,tools.querySelector('.corpus-open')??tools.firstChild);else header?.append(layerControl);
  const more=header?.querySelector('.rb-menu');
  if(more)more.prepend(layerGroup);

  // ------------------------------------------------------------ tasks tray (E1)
  const tray=el('aside','al-tray');tray.hidden=true;
  const trayToggle=button('','al-tray-toggle'),trayList=el('ol','al-tray-list');trayList.hidden=true;
  tray.append(trayToggle,trayList);shell.append(tray);
  const tasks=new Map();let taskSeq=0;
  function paintTray(){
    const items=[...tasks.values()];const running=items.filter(task=>task.state==='running').length;
    tray.hidden=items.length===0;tray.dataset.running=String(running>0);
    trayToggle.textContent=`${t('tasks')} · ${running?`${running} ${t('running')}`:items.length}`;
    trayToggle.setAttribute('aria-expanded',String(!trayList.hidden));
    trayList.replaceChildren(...items.slice(-12).reverse().map(task=>{
      const row=el('li','al-task');row.dataset.state=task.state;
      row.append(el('span','al-task-label',task.label),el('span','al-task-state',task.detail||t(task.state==='running'?'running':task.state==='done'?'done':'failed')));
      return row;
    }));
  }
  on(trayToggle,'click',()=>{trayList.hidden=!trayList.hidden;paintTray();});
  /** A long task with a visible state. It never blocks the interface. */
  function startTask(label){
    const id=++taskSeq,task={id,label,state:'running',detail:''};tasks.set(id,task);paintTray();
    const finish=(state,detail='')=>{task.state=state;task.detail=detail;paintTray();view.setTimeout(()=>{if(tasks.get(id)?.state!=='running'){tasks.delete(id);paintTray();}},state==='done'?8000:30000);};
    return {progress(detail){task.detail=String(detail??'');paintTray();},done:detail=>finish('done',detail),fail:error=>finish('failed',String(error?.message??error??''))};
  }
  /** Show a promise's work in the tray while it runs. */
  function track(promise,label){const task=startTask(label);Promise.resolve(promise).then(value=>{task.done();return value;},error=>{task.fail(error);});return promise;}
  if(acts){
    // Reading a long journal happens in the background; the tray shows it only while it lasts.
    const task=startTask(t('journal'));
    acts.ready.then(status=>status.damaged?task.fail(`${status.damaged.error} · ${status.damaged.seq}`):task.done(`${status.loaded} ${t('journalLoaded')}`),error=>task.fail(error));
  }

  // ------------------------------------------------------------ targets and links (D4)
  /** What the actions act on: the chosen star or link, a selected passage, or nothing. */
  function currentTarget(){
    const passage=api?.reader?.selectedSource?.();
    const chosen=space?.selection?.();
    if(chosen?.id){
      const item=space.index?.get?.(chosen.id)??null;
      return {type:chosen.kind==='relation'?'link':'star',id:chosen.id,label:titleOf(item)??chosen.id,item};
    }
    if(passage?.reference)return {type:'passage',id:null,label:passage.quote?.slice(0,80)??'',passage};
    return {type:'none',id:null,label:''};
  }
  function titleOf(item){
    const title=item?.title;if(!title)return null;
    if(typeof title==='string')return title;
    return title[resolveLocale(locale())==='ru'?'ru':'en']??title.en??title.ru??null;
  }
  /** A link that opens the target where it is: its subject, the experience mode, the lens, the layer and the route step. */
  function linkTo(target){
    const url=new URL(view.location.href);
    url.hash='';
    if(target?.id){url.searchParams.set('item',target.id);if(space?.index?.revision)url.searchParams.set('revision',space.index.revision);}
    const state=experience?.getState?.();if(state?.mode)url.searchParams.set('experience',state.mode);
    if(layer!=='reading')url.searchParams.set(LAYER_PARAM,layer);else url.searchParams.delete(LAYER_PARAM);
    if(target?.type==='passage'&&target.passage?.reference){
      // The reader's own address of a place: work, version, unit and the text layer it was read in.
      const reference=target.passage.reference;
      const route={documentId:reference.target?.workId,versionId:reference.versionId,unitId:reference.unitId,revision:reference.target?.textLayerSha256};
      if(Object.values(route).every(value=>typeof value==='string'&&value))url.hash=encodeReadingRoute(route).slice(1);
    }
    return url.href;
  }
  const toast=el('div','al-toast');toast.setAttribute('role','status');toast.hidden=true;shell.append(toast);
  let toastTimer=0;
  function say(text){toast.textContent=text;toast.hidden=false;view.clearTimeout(toastTimer);toastTimer=view.setTimeout(()=>{toast.hidden=true;},2400);}
  async function copy(text){
    try{await view.navigator.clipboard.writeText(text);say(t('copied'));return true;}
    catch{say(t('copyFailed'));return false;}
  }

  // ------------------------------------------------------------ the action catalog (B1, C2)
  // Each action names the owner it calls; `command` ties it to a page command when there is one (D5).
  const actions=[
    {id:'open-card',label:'openCard',for:['star','link'],command:'tos.page.select',run:target=>registry?registry.invoke('tos.page.select',{item_id:target.id}):experience?.open?.(target.id)},
    {id:'open-article',label:'openArticle',for:['star','link'],command:'tos.page.open-view',run:target=>registry?registry.invoke('tos.page.open-view',{view_id:'reference',focus_id:target.id}):(experience?.open?.(target.id),experience?.setMode?.('reference'))},
    {id:'compare',label:'compare',for:['star'],command:'tos.page.compare',run:target=>registry?registry.invoke('tos.page.compare',{item_id:target.id}):space?.compare?.(target.id)},
    {id:'bench',label:'bench',for:['star','link'],command:'tos.page.lay-on-bench',when:()=>space?.actions?.canLayOnBench?.(),run:()=>registry?registry.invoke('tos.page.lay-on-bench',{}):space?.actions?.layOnBench?.()},
    {id:'copy-link',label:'copyLink',for:['star','link','passage'],when:target=>target.type!=='passage'||Boolean(target.passage),run:target=>copy(linkTo(target))},
    {id:'copy-quote',label:'copyQuote',for:['passage'],run:target=>copy(target.passage
      ?`«${target.passage.quote}» — ${[target.passage.title,target.passage.versionLabel].filter(Boolean).join(', ')}\n${linkTo(target)}`
      :`«${target.quote.replace(/^[«"“]+|[»"”]+$/gu,'')}» — ${[target.workTitle,target.stand.chapter].filter(Boolean).join(', ')}`)},
    {id:'copy-place',label:'copyJson',for:['passage'],when:target=>Boolean(target.stand),run:target=>copy(JSON.stringify({quote:target.quote,...target.stand},null,2))},
    {id:'copy-json',label:'copyJson',for:['star','link'],command:'tos.page.research-item',run:async target=>copy(JSON.stringify(registry?await registry.invoke('tos.page.research-item',{id:target.id}):target.item,null,2))},
    {id:'copy-view-link',label:'copyViewLink',for:['none','star','link','passage'],run:()=>copy(linkTo({type:'none'}))},
    ...LAYERS.map(id=>({id:`layer-${id}`,label:id,prefix:'layer',for:['any'],key:()=>keyLabel(keys().layer),command:'tos.page.set-layer',active:()=>layer===id,run:()=>registry?registry.invoke('tos.page.set-layer',{layer:id}):setLayer(id)})),
    {id:'routes',label:'routes',for:['any'],run:()=>space?.actions?.toggleRoutes?.()},
    {id:'find',label:'find',for:['any'],key:()=>keyLabel(controls?.get?.().keys?.search??'Slash'),run:()=>space?.actions?.focusSearch?.()},
    {id:'settings',label:'settings',for:['any'],when:()=>Boolean(controls),run:()=>openSettings()},
    {id:'tasks',label:'tasks',for:['any'],run:()=>{tray.hidden=false;trayList.hidden=false;paintTray();}},
    {id:'capabilities',label:'capabilities',for:['any'],run:()=>openCapabilities()},
    {id:'manifest',label:'manifest',for:['any'],command:'tos.act.manifest',when:()=>Boolean(acts),run:async()=>{const {actManifest}=await import('../act-journal/agent-tools.mjs');return copy(JSON.stringify(await actManifest(acts,{tier:tier()}),null,2));}},
  ];
  // One action per lens, named by the lens.
  const lensActions=()=>[...(space?.actions?.lenses?.()??[]).map(item=>({id:`lens-${item.id}`,text:`${t('lens')}: ${titleFromForm(item.title)}`,for:['any'],command:'tos.page.set-lens',active:()=>space.actions.lens()===item.id,run:()=>registry?registry.invoke('tos.page.set-lens',{lens:item.id}):space.actions.setLens(item.id)})),
    // Every route is one action away.
    ...(space?.actions?.routes?.()??[]).map(item=>({id:`route-${item.id}`,text:`${t('route')}: ${titleFromForm(item.title)}`,for:['any'],command:'tos.page.start-route',active:()=>space.actions.routeState?.()?.id===item.id,run:()=>registry?registry.invoke('tos.page.start-route',{route_id:item.id}):space.actions.startRoute?.(item.id)}))];
  const titleFromForm=form=>typeof form==='string'?form:form?.[resolveLocale(locale())==='ru'?'ru':'en']??(form?.en?localeText(form.en,locale()):form?.ru)??'';
  const actionText=action=>typeof action.text==='function'?action.text():action.text??(action.prefix?`${t(action.prefix)}: ${t(action.label)}`:t(action.label));
  // Other owners add their actions (the act desk: tickets, review, transfer); each keeps its own label.
  const added=[];
  function addActions(list){added.push(...list);return ()=>{for(const action of list){const at=added.indexOf(action);if(at>=0)added.splice(at,1);}};}
  const applies=(action,target)=>(action.for.includes('any')||action.for.includes(target.type))&&(!action.when||action.when(target));

  // ------------------------------------------------------------ palette and menu
  const palette=el('dialog','al-palette');palette.setAttribute('aria-label',t('palette'));
  const paletteHead=el('div','al-palette-head'),chip=el('p','al-palette-target'),input=el('input','al-palette-input');
  input.type='search';input.autocomplete='off';input.spellcheck=false;
  const list=el('ul','al-palette-list');list.setAttribute('role','listbox');list.id='al-palette-list';
  input.setAttribute('role','combobox');input.setAttribute('aria-controls',list.id);input.setAttribute('aria-expanded','true');
  paletteHead.append(chip,input);palette.append(paletteHead,list);shell.append(palette);
  let shown=[],cursor=0,target={type:'none'},opener=null;
  function renderList(){
    const query=input.value.trim().toLocaleLowerCase();
    const pool=[...actions,...added,...lensActions()].filter(action=>applies(action,target));
    shown=query?pool.filter(action=>actionText(action).toLocaleLowerCase().includes(query)):pool;
    cursor=Math.min(cursor,Math.max(0,shown.length-1));
    list.replaceChildren(...(shown.length?shown.map((action,index)=>{
      const item=el('li','al-palette-item');item.setAttribute('role','option');item.id=`al-option-${index}`;
      item.setAttribute('aria-selected',String(index===cursor));if(action.active?.())item.dataset.active='true';
      item.append(el('span','al-palette-label',actionText(action)));
      if(action.key)item.append(el('kbd','al-palette-key',typeof action.key==='function'?action.key():action.key));
      item.addEventListener('pointerdown',event=>event.preventDefault());
      item.addEventListener('click',()=>{cursor=index;void runCurrent();});
      return item;
    }):[el('li','al-palette-empty',t('nothing'))]));
    input.setAttribute('aria-activedescendant',shown.length?`al-option-${cursor}`:'');
    list.querySelector('[aria-selected="true"]')?.scrollIntoView({block:'nearest'});
  }
  function openPalette({anchor=null,for:forTarget=null}={}){
    target=forTarget??currentTarget();opener=doc.activeElement;
    chip.textContent=target.type==='none'?t('noChoice'):`${t('chosen')}: ${target.label}`;
    input.placeholder=t('search');input.value='';cursor=0;
    palette.dataset.mode=anchor?'menu':'palette';
    if(anchor){const box=anchor.getBoundingClientRect();palette.style.top=`${Math.min(box.bottom+6,view.innerHeight-320)}px`;palette.style.left=`${Math.max(8,Math.min(box.left,view.innerWidth-360))}px`;}
    else{palette.style.top='';palette.style.left='';}
    renderList();
    if(!palette.open)palette.showModal();
    input.focus();
  }
  function closePalette(){if(palette.open)palette.close();}
  async function runCurrent(){
    const action=shown[cursor];if(!action)return;
    const chosen=target;closePalette();
    try{await action.run(chosen);}catch(error){say(String(error?.message??error));}
  }
  on(input,'input',()=>{cursor=0;renderList();});
  on(input,'keydown',event=>{
    if(event.key==='ArrowDown'){event.preventDefault();cursor=Math.min(shown.length-1,cursor+1);renderList();}
    else if(event.key==='ArrowUp'){event.preventDefault();cursor=Math.max(0,cursor-1);renderList();}
    else if(event.key==='Enter'){event.preventDefault();void runCurrent();}
  });
  on(palette,'click',event=>{if(event.target===palette)closePalette();});
  on(palette,'close',()=>{if(opener?.isConnected)opener.focus?.({preventScroll:true});});

  // The card's own tools get one more button: the menu of every action for this star or link.
  const cardMenu=button('⋯','icon al-card-menu');
  on(cardMenu,'click',()=>openPalette({anchor:cardMenu}));
  space?.actions?.columnTools?.append(cardMenu);
  // A selected passage in the reader offers the same menu.
  api?.reader?.setPassageActions?.([{get label(){return t('actions');},run:value=>{const passage=api.reader.selectedSource?.();
    if(!passage)return;openPalette({anchor:{getBoundingClientRect:()=>value.range.getBoundingClientRect()},for:{type:'passage',id:null,label:passage.quote?.slice(0,80)??'',passage}});}}]);
  // A passage of the stand's own witnesses (its quotations and parallel chapters) offers it too.
  space?.actions?.setPassageActions?.([{get label(){return t('actions');},run:(value,place)=>{if(!place)return;
    const quote=String(value.text??'').replace(/\s+/g,' ').trim();
    openPalette({anchor:{getBoundingClientRect:()=>value.range.getBoundingClientRect()},for:{type:'passage',id:null,label:quote.slice(0,80),quote,stand:place,workTitle:value.element.closest('dialog')?.querySelector('h1,h2')?.textContent?.trim()??''}});}}]);

  // Keys: Ctrl/⌘+K opens the palette anywhere; L walks the layers when no field has the keyboard.
  on(doc,'keydown',event=>{
    if((event.ctrlKey||event.metaKey)&&!event.altKey&&event.code===keys().palette){event.preventDefault();if(palette.open)closePalette();else openPalette();return;}
    if(event.ctrlKey||event.metaKey||event.altKey||event.defaultPrevented)return;
    if(event.target.closest?.('input,textarea,select,[contenteditable="true"],dialog[open]'))return;
    if(event.code===keys().layer){event.preventDefault();setLayer(LAYERS[(LAYERS.indexOf(layer)+1)%LAYERS.length]);}
  },{capture:true});

  // ------------------------------------------------------------ what agents can do (D5)
  const capabilities=el('dialog','al-capabilities');shell.append(capabilities);
  function capabilityRows(){
    const tools=new Set(['tos.page.context','tos.page.cancel',...pageCommandIds]);
    const rows=[];
    for(const action of [...actions,...added,...lensActions()]){
      const command=action.command??null;
      rows.push({interface:actionText(action),page:command&&pageCommandIds.has(command)?command:'',agent:command&&(tools.has(command)||command.startsWith('tos.act.'))?command:'',native:'',cli:''});
    }
    for(const command of pageCommands.commands)if(!actions.some(action=>action.command===command.command_id))rows.push({interface:'',page:command.command_id,agent:command.command_id,native:'',cli:''});
    for(const command of actCommands.commands)if(!actions.some(action=>action.command===command.command_id))rows.push({interface:'',page:'',agent:command.command_id,native:'',cli:''});
    for(const operation of knowledgeApi.operations)rows.push({interface:'',page:'',agent:'',native:operation.mcp_tool??'',cli:operation.cli??''});
    return rows;
  }
  function openCapabilities(){
    const head=el('div','al-capabilities-head'),title=el('h2','',t('capabilities')),close=button('×','icon');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>capabilities.close());
    head.append(title,close);
    const table=el('table','al-capabilities-table'),thead=el('thead'),headRow=el('tr');
    for(const key of ['colInterface','colPage','colAgent','colNative','colCli'])headRow.append(el('th','',t(key)));
    thead.append(headRow);const body=el('tbody');
    for(const row of capabilityRows()){const tr=el('tr');for(const key of ['interface','page','agent','native','cli']){const cell=el('td',row[key]?'':'al-gap',row[key]);if(key!=='interface'&&row[key])cell.classList.add('al-code');tr.append(cell);}body.append(tr);}
    table.append(thead,body);
    const scroller=el('div','al-capabilities-scroll');scroller.append(table);
    capabilities.replaceChildren(head,el('p','al-capabilities-lead',t('capabilitiesLead')),scroller);
    capabilities.showModal();
  }

  // ------------------------------------------------------------ control settings (F1) and reading size and contrast (F4)
  const settings=el('dialog','al-settings');shell.append(settings);
  function openSettings(){
    if(!controls)return;
    const prefs=controls.get(),form=el('div','al-settings-body');
    const head=el('div','al-capabilities-head'),close=button('×','icon');close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>settings.close());
    head.append(el('h2','',t('settings')),close);
    const row=(label,control)=>{const wrap=el('label','al-setting');wrap.append(el('span','',label),control);return wrap;};
    const preset=el('select','al-input');preset.id='al-preset';
    for(const name of [...Object.keys(PRESETS),'custom']){const option=el('option','',t(`preset-${name}`));option.value=name;option.selected=prefs.preset===name;preset.append(option);}
    preset.addEventListener('change',()=>{if(preset.value!=='custom')controls.set({preset:preset.value});openSettings();});
    const range=(name,min,max,step)=>{const input=el('input','al-range');input.type='range';input.id=`al-${name}`;input.min=String(min);input.max=String(max);input.step=String(step);input.value=String(prefs[name]);
      const out=el('output','al-range-value',`${prefs[name].toFixed(2)}×`);input.addEventListener('input',()=>{out.textContent=`${Number(input.value).toFixed(2)}×`;});input.addEventListener('change',()=>{controls.set({[name]:Number(input.value)});openSettings();});
      const wrap=el('span','al-range-wrap');wrap.append(input,out);return wrap;};
    const check=name=>{const input=el('input');input.type='checkbox';input.id=`al-${name}`;input.checked=Boolean(prefs[name]);input.addEventListener('change',()=>controls.set({[name]:input.checked}));return input;};
    const keyRows=el('div','al-keys');
    for(const [name,code] of Object.entries(prefs.keys)){
      const capture=button(keyLabel(code),'al-key');capture.id=`al-key-${name}`;
      capture.addEventListener('click',()=>{capture.textContent=t('pressKey');
        const onKey=event=>{event.preventDefault();event.stopPropagation();doc.removeEventListener('keydown',onKey,true);if(event.key!=='Escape')controls.set({keys:{...controls.get().keys,[name]:event.code}});openSettings();};
        doc.addEventListener('keydown',onKey,true);});
      keyRows.append(row(t(`key-${name}`),capture));
    }
    const contrast=el('select','al-input');contrast.id='al-contrast';
    for(const name of ['normal','high']){const option=el('option','',t(`contrast-${name}`));option.value=name;option.selected=prefs.contrast===name;contrast.append(option);}
    contrast.addEventListener('change',()=>controls.set({contrast:contrast.value}));
    const reset=button(t('reset'),'text-button');reset.addEventListener('click',()=>{controls.reset();openSettings();});
    form.append(row(t('preset'),preset),row(t('turnSpeed'),range('turnSpeed',.2,3,.1)),row(t('zoomSpeed'),range('zoomSpeed',.2,3,.1)),
      row(t('inertia'),check('inertia')),row(t('invertTurn'),check('invertTurn')),row(t('invertZoom'),check('invertZoom')),
      el('h3','al-settings-title',t('keys')),keyRows,row(t('textScale'),range('textScale',.85,1.5,.05)),row(t('contrast'),contrast),reset);
    settings.replaceChildren(head,form);
    if(!settings.open)settings.showModal();
  }

  function refreshLocale(){paintLayer();paintTray();cardMenu.setAttribute('aria-label',t('actions'));cardMenu.title=`${t('actions')} · Ctrl+K`;}
  refreshLocale();
  if(layer!=='reading')shell.dispatchEvent(new view.CustomEvent('public-research-layer',{bubbles:true,detail:{layer}}));

  return {
    layer:()=>layer,setLayer,openPalette,closePalette,linkTo,currentTarget,track,startTask,refreshLocale,addActions,say,copy,openSettings,
    /** For the page context: the presence layer, lens, route step and the reader's chosen passage. */
    context:()=>{const passage=api?.reader?.selectedSource?.();return {presence_layer:layer,lens:space?.actions?.lens?.()??null,route:space?.actions?.routeState?.()??null,
      bench_open:Boolean(space?.bench?.()?.isOpen?.()),reader_selection:passage?.reference?{reference:passage.reference,quote:String(passage.quote??'').slice(0,500)}:null};},
    destroy(){for(const dispose of disposers.splice(0))dispose();layerControl.remove();layerGroup.remove();settings.remove();tray.remove();toast.remove();palette.remove();capabilities.remove();cardMenu.remove();api?.reader?.setPassageActions?.([]);space?.actions?.setPassageActions?.([]);delete doc.documentElement.dataset.presenceLayer;},
  };
}
