import {ui,uiAttribute,uiChildren,uiText,uiComputed,uiLanguage} from './ui-i18n.mjs';
import {BUDGET,ContractError,RequestSlots,localized,displayTitle,sameJson} from './knowledge-client.mjs';
import {createConstructorCatalogLoader,initialDraft,validateDraft,compileDraft,previewDraft,draftForPacket,lensDelta} from './lens-model.mjs';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {conditionCatalog} from './lens-conditions.mjs';
import {createConditionEditor,humanConditionText} from './lens-condition-editor.mjs';
import {createLensPathEditor} from './lens-path-editor.mjs';
import {sourceLabel,relationLabel} from './human-presentation.mjs';
import './lens-builder.css';

export const LENS_BUILDER_LIMITS=Object.freeze({nodes:BUDGET.nodes,relations:BUDGET.relations,catalogPage:40});
const REVISION=/^[a-f0-9]{64}$/;
const AREA_SCHEMAS=new Set(['tos_lens_result_v1','tos_browser_exploration_view_v1']);
const SELECTION_KINDS=new Set(['node','relation','claim-path']);
let builderId=0;

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node,text);if(className)node.className=className;return node;};
const option=(value,text)=>{const node=el('option',text);node.value=String(value);return node;};
const button=(text,action,className='lens-builder-button')=>{const node=el('button',text,className);node.type='button';node.addEventListener('click',()=>void Promise.resolve().then(action));return node;};
const field=(text,input)=>{const label=el('label','','lens-builder-field');uiChildren(label,'append',el('span',text),input);return label;};
const copy=value=>typeof structuredClone==='function'?structuredClone(value):JSON.parse(JSON.stringify(value));
const record=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const revision=value=>typeof value==='string'&&REVISION.test(value);
const language=locale=>{const value=typeof locale==='function'?locale():locale;return value==='en'||value==='es'?value:'ru';};
const count=(number,one,few,many)=>uiComputed(()=>`${number} ${{one,few,many}[new Intl.PluralRules(uiLanguage()).select(number)]||many}`);

function fail(message){throw new ContractError(message);}

/**
 * Validate only the carrier boundary owned by this UI. The caller owns the
 * full LensResult or exploration-view admission; this helper never relabels
 * one schema as the other and never manufactures a LensResult focus.
 */
export function normalizeLensBuilderArea(value){
  if(!record(value)||!record(value.packet)||!AREA_SCHEMAS.has(value.packet.schema)
    ||!revision(value.packet.source_revision)||!Array.isArray(value.packet.nodes)||!Array.isArray(value.packet.relations))
    fail(ui("Область не соответствует контракту конструктора линз."));
  const nodeIds=new Set(),relationIds=new Set();
  for(const item of value.packet.nodes){if(!record(item)||typeof item.id!=='string'||!item.id||nodeIds.has(item.id))fail(ui("Область не соответствует контракту конструктора линз."));nodeIds.add(item.id);}
  for(const item of value.packet.relations){if(!record(item)||typeof item.id!=='string'||!item.id||relationIds.has(item.id))fail(ui("Область не соответствует контракту конструктора линз."));relationIds.add(item.id);}
  const selection=value.selection??value.packet.selection??(value.packet.focus?.node_id?{kind:'node',id:value.packet.focus.node_id}:null);
  if(!record(selection)||!SELECTION_KINDS.has(selection.kind)||typeof selection.id!=='string'||!selection.id
    ||selection.claimId!==undefined&&(typeof selection.claimId!=='string'||!selection.claimId))
    fail(ui("Выбор области не соответствует контракту конструктора линз."));
  const allowedSelectionKeys=selection.kind==='claim-path'?['kind','id','claimId']:['kind','id'];
  if(Object.keys(selection).some(key=>!allowedSelectionKeys.includes(key)))fail(ui("Выбор области не соответствует контракту конструктора линз."));
  if(selection.kind==='claim-path'&&!selection.claimId)fail(ui("Для пути области не указан Claim."));
  if(selection.kind==='node'&&!nodeIds.has(selection.id)||selection.kind==='relation'&&!relationIds.has(selection.id)
    ||selection.kind==='claim-path'&&!nodeIds.has(selection.claimId))fail(ui("Выбор области не соответствует контракту конструктора линз."));
  return {packet:value.packet,selection};
}

/** Keep the source packet intact while exposing its bounded display budget. */
export function lensBuilderAreaBudget(area){
  const normalized=normalizeLensBuilderArea(area),packet=normalized.packet;
  return {nodes:packet.nodes.length,relations:packet.relations.length,
    nodeOverflow:Math.max(0,packet.nodes.length-LENS_BUILDER_LIMITS.nodes),
    relationOverflow:Math.max(0,packet.relations.length-LENS_BUILDER_LIMITS.relations),
    overBudget:packet.nodes.length>LENS_BUILDER_LIMITS.nodes||packet.relations.length>LENS_BUILDER_LIMITS.relations,
    selection:copy(normalized.selection),sourceRevision:packet.source_revision};
}

function selectedNodeId(selection){
  if(selection?.kind==='node')return selection.id;
  if(selection?.kind==='claim-path')return selection.claimId;
  return null;
}

/** Resolve a saved focus label from the bounded area packet only. */
export function lensBuilderFocusLabel(area,draft,preferred=uiLanguage()){
  const center=Array.isArray(area?.packet?.nodes)
    ?area.packet.nodes.find(item=>item?.id===draft?.focusId)
    :null;
  return center?displayTitle(center,ui("Звезда"),preferred):ui("Звезда недоступна");
}

/**
 * Build a valid working draft for a retained exploration view. If the view
 * exceeds LensSpec's node selector bound, all node IDs stay in the source
 * packet and the working draft explicitly offers focus/all; no IDs are
 * silently sliced.
 */
export function prepareLensBuilderDraft({packet,context,selection,draft:inputDraft}={}){
  if(!context)fail(ui("Для конструктора нужен каталог."));
  if(packet==null){
    const draft=inputDraft===undefined?copy(initialDraft(null,context)):copy(inputDraft);
    return {draft,areaChoice:null,sourceNodeIds:Array.isArray(draft?.nodeIds)?draft.nodeIds.length:0,
      sourceNodeCount:0,sourceRelationCount:0};
  }
  if(!record(packet))fail(ui("Область не соответствует контракту конструктора линз."));
  const area=normalizeLensBuilderArea({packet,selection});
  const provided=inputDraft!==undefined;
  let draft=provided?copy(inputDraft):copy(draftForPacket(packet)||initialDraft(packet,context));
  const sourceNodeCount=packet.nodes.length;
  const candidateFocus=selectedNodeId(area.selection)||packet.focus?.node_id||null;
  const sourceNodeIds=Array.isArray(draft.nodeIds)?draft.nodeIds.length:0;
  const tooManyNodeIds=sourceNodeIds>LENS_BUILDER_LIMITS.nodes;
  const largeArea=sourceNodeCount>LENS_BUILDER_LIMITS.nodes;
  let areaChoice=null;
  if(!provided&&largeArea){
    // The original IDs remain available through packet. The draft itself is
    // intentionally moved to a representable scope and the UI explains why.
    draft={...draft,scope:candidateFocus?'focus':'all',nodeIds:[],focusId:candidateFocus||null};
    areaChoice={required:true,nodeCount:sourceNodeCount,relationCount:packet.relations.length,
      choices:candidateFocus?['focus','all']:['all'],defaultScope:candidateFocus?'focus':'all',sourceNodeIds:sourceNodeCount};
  }else if(tooManyNodeIds){
    // An imported/host-supplied oversized draft is an explicit invalid state.
    // Do not repair it by truncating its selector.
    areaChoice={required:true,invalid:true,nodeCount:sourceNodeCount,relationCount:packet.relations.length,
      choices:candidateFocus?['focus','all']:['all'],defaultScope:null,sourceNodeIds};
  }else if(largeArea){
    const choices=candidateFocus?['focus','all']:['all'];
    areaChoice={required:true,nodeCount:sourceNodeCount,relationCount:packet.relations.length,
      choices,defaultScope:choices.includes(draft.scope)?draft.scope:choices[0],sourceNodeIds:sourceNodeCount};
  }
  return {draft,areaChoice,sourceNodeIds:largeArea?sourceNodeCount:sourceNodeIds,
    sourceNodeCount,sourceRelationCount:packet.relations.length};
}

/** A preview from an older draft is never eligible for the apply seam. */
export function lensBuilderCanApply({preview,previewRevision,draftRevision,areaRevision,sourceRevision}={}){
  return Boolean(preview&&Array.isArray(preview.nodes)&&preview.nodes.length>0
    &&preview.source_revision===sourceRevision&&previewRevision===draftRevision
    &&(areaRevision==null||areaRevision===sourceRevision));
}

function areaSignature(area){
  const packet=area.packet;
  return {schema:packet.schema,source_revision:packet.source_revision,
    snapshot_revision:packet.snapshot_revision??null,fingerprint:packet.fingerprint??null,
    nodes:packet.nodes.length,relations:packet.relations.length,selection:copy(area.selection)};
}

function areaStillCurrent(previous,current){
  if(!previous&&!current)return true;
  if(!previous||!current)return false;
  if(previous.packet===current.packet)return sameJson(previous.selection,current.selection);
  return sameJson(areaSignature(previous),areaSignature(current))&&sameJson(previous.packet,current.packet);
}

function sourceNames(ids){return ids.map(id=>sourceLabel(id)).join(', ');}

function queryLines(draft,context){
  const nodeEntries=conditionCatalog(context,'nodes'),relationEntries=conditionCatalog(context,'relations');
  const lines=[draft.scope==='area'?ui("Из исходной области · {0}",[draft.nodeIds.length]):draft.scope==='focus'?ui("От выбранной звезды"):ui("По всему древу")];
  lines.push(ui("Источники: {0}",[sourceNames(draft.sources)||ui("не выбраны")]));
  if(draft.scope!=='focus'){
    if(draft.query)lines.push(ui("Поиск: «{0}»",[draft.query]));
    if(draft.kinds.length)lines.push(ui("Типы узлов: {0}",[draft.kinds.map(id=>{const item=context.catalog.node_kinds.find(entry=>entry.kind_id===id),value=localized(item?.display,ui("Недоступно"));return String(value)===id?ui("Недоступно"):value;}).join(', ')]));
    for(const rule of draft.conditions.nodes)lines.push(humanConditionText(rule,nodeEntries));
  }
  if(draft.paths?.length)for(const path of draft.paths)
    lines.push(ui("Условие пути {0}: {1} · шагов: {2}",[draft.paths.indexOf(path)+1,path.quantifier==='not_exists'?ui("Такого пути нет"):ui("Есть такой путь"),path.steps.length]));
  if(draft.relations){
    if(draft.predicates.length)lines.push(ui("Типы связей: {0}",[draft.predicates.map(id=>{const item=context.catalog.predicates.find(entry=>entry.predicate_id===id);return item?relationLabel(item):ui("Недоступно");}).join(', ')]));
    for(const rule of draft.conditions.relations)lines.push(ui("Связь: {0}",[humanConditionText(rule,relationEntries)]));
    lines.push(ui("Окружение: {0} · {1} · {2}",[draft.depth,({either:ui("в обе стороны"),outgoing:ui("по связям"),incoming:ui("против связей")})[draft.direction],draft.profile==='all'?ui("все типы"):ui("обзор")]));
  }else lines.push(ui("Без связей"));
  lines.push(ui("До {0} звёзд",[draft.limit]));
  return lines;
}

export const lensBuilderQueryLines=queryLines;

function choicePicker({draft,context,key,title,onChange,stateStore}){
  const state=stateStore.get(key)||{query:'',sort:'alphabet',limits:new Map(),expanded:new Map()};stateStore.set(key,state);
  const details=el('details','','lens-builder-choices'),summary=el('summary'),controls=el('div','','lens-builder-choice-controls'),search=el('input'),sort=el('select'),list=el('div','','lens-builder-options');
  search.type='search';search.maxLength=256;uiAttribute(search,'placeholder',ui("Найти в списке…"));uiAttribute(search,'aria-label',ui("Найти: {0}",[title]));search.value=state.query;
  for(const [id,label] of [['alphabet',ui("По алфавиту")],['frequency',ui("Сначала частые")]])uiChildren(sort,'append',option(id,label));sort.value=state.sort;
  const vocabulary=lensVocabulary(context.catalog,key);
  const caption=()=>uiText(summary,`${String(title)} · ${draft[key].length?ui("{0} выбрано",[draft[key].length]):ui("любые")}`);caption();
  function redraw(){
    const scroll=list.scrollTop;uiChildren(list,'replaceChildren');
    for(const group of vocabularyGroups(vocabulary,{sources:draft.sources,selected:draft[key],query:state.query,sort:state.sort})){
      const section=el('details','','lens-builder-choice-group');section.dataset.group=group.key;section.open=Boolean(state.query)||group.items.some(item=>item.selected)||state.expanded.get(group.key)===true;
      const heading=el('summary',`${String(group.title)} · ${group.items.length}`);section.addEventListener('toggle',()=>state.expanded.set(group.key,section.open));uiChildren(section,'append',heading);
      if(group.key==='unavailable')uiChildren(section,'append',el('p',ui("Некоторые сохранённые варианты недоступны. Снимите выбор или обновите каталог."),'lens-builder-note'));
      const limit=state.limits.get(group.key)||LENS_BUILDER_LIMITS.catalogPage;
      for(const item of group.items.slice(0,limit)){
        const input=el('input');input.type='checkbox';input.value=item.id;input.checked=item.selected;
        input.addEventListener('change',()=>{draft[key]=input.checked?[...draft[key],item.id]:draft[key].filter(id=>id!==item.id);caption();onChange();});
        const name=el('span',item.title);
        const label=el('label','','lens-builder-choice');uiChildren(label,'append',input,name);uiChildren(section,'append',label);
      }
      if(group.items.length>limit)uiChildren(section,'append',button(ui("Ещё варианты · {0}",[group.items.length-limit]),()=>{state.limits.set(group.key,limit+LENS_BUILDER_LIMITS.catalogPage);redraw();},'lens-builder-link'));
      uiChildren(list,'append',section);
    }
    if(!list.children.length)uiChildren(list,'append',el('p',ui("Нет вариантов для этих источников и поиска."),'lens-builder-note'));
    list.scrollTop=scroll;
  }
  search.addEventListener('input',()=>{state.query=search.value;redraw();});sort.addEventListener('change',()=>{state.sort=sort.value;redraw();});
  uiChildren(controls,'append',search,sort);uiChildren(details,'append',summary,controls,list,el('p',ui("Дополнительные типы собраны отдельно."),'lens-builder-note'),button(ui("Сбросить выбор"),()=>{draft[key]=[];caption();redraw();onChange();},'lens-builder-link'));redraw();return details;
}

function localizedPacketCounts(packet){
  const nodes=packet?.nodes?.length??0,relations=packet?.relations?.length??0,counts=packet?.counts??{};
  return {nodes,relations,matched:Number.isInteger(counts.matched_nodes)?counts.matched_nodes:null,
    context:Number.isInteger(counts.context_nodes)?counts.context_nodes:null,
    truncatedNodes:Number.isInteger(counts.truncated_nodes)?counts.truncated_nodes:0,
    truncatedRelations:Number.isInteger(counts.truncated_relations)?counts.truncated_relations:0};
}

export function mountLensBuilder({host,client,locale='ru',getArea,getCatalog,onApply,onSave,onOpen,onClose,onError}={}){
  if(!host||typeof host.append!=='function')throw new TypeError('A lens builder host element is required.');
  if(!client||typeof client.request!=='function'||typeof client.compile!=='function')throw new TypeError('A knowledge client is required.');
  if(typeof getArea!=='function')throw new TypeError('getArea must return the current reading area.');
  const requests=new RequestSlots();
  const catalogLoader=createConstructorCatalogLoader(client,getCatalog);
  const root=el('section','','lens-builder');root.hidden=true;root.tabIndex=-1;root.setAttribute('role','dialog');root.setAttribute('aria-modal','true');root.lang=language(locale);uiAttribute(root,'aria-label',ui("Конструктор линз"));
  const header=el('header','','lens-builder-header'),heading=el('h2',ui("Собрать линзу")),closeButton=button(ui("Закрыть конструктор"),()=>close(),'lens-builder-close');
  heading.id=`lens-builder-title-${++builderId}`;root.setAttribute('aria-labelledby',heading.id);
  uiChildren(header,'append',el('div','','lens-builder-heading-copy'),closeButton);header.querySelector('.lens-builder-heading-copy').append(heading);
  const areaInfo=el('section','','lens-builder-area'),body=el('div','','lens-builder-body'),previewRegion=el('section','','lens-builder-preview');previewRegion.setAttribute('aria-live','polite');
  const status=el('p','','lens-builder-status');status.setAttribute('role','status');const footer=el('footer','','lens-builder-footer');
  uiChildren(root,'append',header,areaInfo,body,previewRegion,status,footer);host.append(root);

  let opened=false,destroyed=false,loading=false,previewing=false,area=null,context=null,draft=null,preview=null;
  let error='',previewError='',areaChoice=null,invalidDraft=null,previewRevision=-1,draftRevision=0,areaRevision=null,returnFocus=null;
  const choiceStates=new Map();
  const report=problem=>{if(destroyed)return;error=ui("Не удалось выполнить действие. Повторите запрос.");status.dataset.state='error';try{onError?.(problem);}catch{}};
  const areaProblem=message=>{const problem=message instanceof Error?message:new ContractError(message);if(message instanceof Error)report(problem);else{error=String(message);status.dataset.state='error';try{onError?.(problem);}catch{}}return problem;};
  // A top-level opening may have no current area. Keep that as an explicit
  // null state so a saved all/focus definition can still be edited without
  // manufacturing a packet or borrowing a stale scene.
  const currentArea=()=>{
    const value=getArea();
    if(value==null)return null;
    if(record(value)&&value.packet==null&&value.selection==null)return null;
    return normalizeLensBuilderArea(value);
  };
  const sourceMismatch=()=>Boolean(area&&context&&context.catalog.source_revision!==area.packet.source_revision);
  const draftValidationError=()=>{try{validateDraft(draft);return '';}catch{return ui("Проверьте настройки линзы.");}};
  const validDraft=()=>!draftValidationError();
  const areaScopeNode=()=>selectedNodeId(area?.selection)||area?.packet?.focus?.node_id||null;
  const previewApplicable=()=>lensBuilderCanApply({preview,previewRevision,draftRevision,areaRevision,sourceRevision:area?.packet?.source_revision??context?.catalog?.source_revision})&&!sourceMismatch()&&!areaChoice?.required;

  function focusables(){return [...root.querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),summary,a[href],[tabindex="0"]')].filter(node=>!node.closest('[hidden]'));}
  function handleKey(event){
    if(!opened)return;
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();return;}
    if(event.key!=='Tab')return;
    const items=focusables(),first=items[0],last=items.at(-1);if(!first||!last)return;
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}
    else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}
  }
  root.addEventListener('keydown',handleKey);

  function renderArea(){
    uiChildren(areaInfo,'replaceChildren');
    if(!area){uiChildren(areaInfo,'append',el('p',loading?ui("Загружаю область…"):ui("Текущая область не выбрана. Настройки можно собрать по выбранным источникам."),'lens-builder-note'));return;}
    const budget=lensBuilderAreaBudget(area),scope=area.packet.schema==='tos_lens_result_v1'?ui("Линза"):ui("Исследовательская область");
    const title=el('p',`${String(scope)} · ${ui("Исходных записей: {0}; связей: {1}",[budget.nodes,budget.relations])}`,'lens-builder-area-title');
    const areaSources=Array.isArray(area.packet.sources)?area.packet.sources:Array.isArray(draft?.sources)?draft.sources:[];
    const sources=el('p',ui("Источники области: {0}",[sourceNames(areaSources)]),'lens-builder-note');
    uiChildren(areaInfo,'append',title,sources);
    if(budget.overBudget)uiChildren(areaInfo,'append',el('p',ui("Область больше обычного размера. Выберите центр или весь граф.") ,'lens-builder-warning'));
    if(sourceMismatch())uiChildren(areaInfo,'append',el('p',ui("Каталог и область относятся к разным версиям данных. Обновите область перед просмотром или сохранением."),'lens-builder-warning'));
    if(areaChoice?.required){
      const choice=el('fieldset','','lens-builder-area-choice');uiChildren(choice,'append',el('legend',ui("Большая область: выберите область действия")));
      const help=areaChoice.invalid?ui("Выберите «От выбранной звезды» или «По всему древу»."):ui("Исходная область содержит {0} узлов. Выберите способ отбора.",[areaChoice.nodeCount]);
      uiChildren(choice,'append',el('p',help,'lens-builder-warning'));
      const select=el('select');uiAttribute(select,'aria-label',ui("Область действия большой области"));
      for(const value of areaChoice.choices)uiChildren(select,'append',option(value,value==='focus'?ui("От выбранной звезды"):ui("По всему древу")));
      if(areaChoice.defaultScope)select.value=areaChoice.defaultScope;
      select.addEventListener('change',()=>chooseLargeArea(select.value));
      const confirm=button(ui("Открыть область"),()=>chooseLargeArea(select.value),'lens-builder-primary');
      uiChildren(choice,'append',field(ui("Режим"),select),confirm);uiChildren(areaInfo,'append',choice);
    }
  }

  function chooseLargeArea(value){
    if(!areaChoice?.choices.includes(value))return;
    if(value==='focus'&&!areaScopeNode()){error=ui("Для режима «От выбранной звезды» нужна выбранная звезда в текущей области.");render();return;}
    if(!draft&&areaChoice.invalid){
      // This is the explicit repair action for an imported oversized draft.
      // Build a fresh bounded draft only after the user chooses focus/all;
      // never mutate or slice the invalid input behind their back.
      draft=initialDraft(area.packet,context);invalidDraft=null;
    }
    if(!draft)return;
    draft.scope=value;draft.focusId=value==='focus'?areaScopeNode():null;draft.nodeIds=[];areaChoice={...areaChoice,required:false,selected:value};touch({rerender:true});
  }

  function renderBody(){
    uiChildren(body,'replaceChildren');
    if(!context||!draft){if(!loading)uiChildren(body,'append',button(ui("Повторить загрузку"),()=>void load({refresh:true})));return;}
    const name=el('input');name.type='text';name.maxLength=64;name.value=draft.name;name.addEventListener('input',()=>{draft.name=name.value;touch();});uiChildren(body,'append',field(ui("Название линзы"),name));
    const scopeChoices=[];if(!areaChoice?.required||areaChoice.choices.includes('area'))scopeChoices.push(['area',ui("Из исходной области")]);scopeChoices.push(['focus',ui("От выбранной звезды")],['all',ui("По всему древу")]);
    const scope=el('select');for(const [value,label] of scopeChoices)uiChildren(scope,'append',option(value,label));scope.value=draft.scope;scope.addEventListener('change',()=>{draft.scope=scope.value;touch({rerender:true});});uiChildren(body,'append',field(ui("Отправная точка"),scope));
    if(draft.scope==='area')uiChildren(body,'append',el('p',ui("Исходная область: {0}. Фильтры выбирают начало; глубина добавляет окружение.",[count(draft.nodeIds.length,ui("звезда"),ui("звезды"),ui("звёзд"))]),'lens-builder-note'));
    if(draft.scope==='focus'){
      uiChildren(body,'append',el('p',ui("Центр: {0}",[lensBuilderFocusLabel(area,draft,language(locale))]),'lens-builder-focus'));
      if(!draft.focusId&&areaScopeNode())uiChildren(body,'append',button(ui("Взять выбранную звезду"),()=>{draft.focusId=areaScopeNode();touch({rerender:true});},'lens-builder-link'));
      uiChildren(body,'append',el('p',ui("Условия ниже выбирают связи вокруг явного центра."),'lens-builder-note'));
    }
    const sources=el('fieldset','','lens-builder-sources');uiChildren(sources,'append',el('legend',ui("Источники")));
    for(const id of context.catalog.capabilities.sources){const input=el('input');input.type='checkbox';input.checked=draft.sources.includes(id);input.addEventListener('change',()=>{draft.sources=input.checked?[...draft.sources,id]:draft.sources.filter(source=>source!==id);touch({rerender:true});});const label=el('label');uiChildren(label,'append',input,el('span',sourceLabel(id)));uiChildren(sources,'append',label);}uiChildren(body,'append',sources);
    if(draft.scope!=='focus'){
      const query=el('input');query.type='search';query.maxLength=256;query.value=draft.query;uiAttribute(query,'placeholder',ui("Имя, произведение, понятие…"));query.addEventListener('input',()=>{draft.query=query.value;touch();});uiChildren(body,'append',field(ui("Слова в исходных узлах"),query));
      uiChildren(body,'append',choicePicker({draft,context,key:'kinds',title:ui("Типы узлов"),onChange:touch,stateStore:choiceStates}));
    }
    if(draft.scope!=='focus'||draft.conditions.nodes.length)uiChildren(body,'append',createConditionEditor({draft,context,kind:'nodes',onChange:touch}));
    uiChildren(body,'append',createLensPathEditor({draft,context,onChange:touch}));
    const relationToggle=el('input');relationToggle.type='checkbox';relationToggle.checked=draft.relations;relationToggle.addEventListener('change',()=>{draft.relations=relationToggle.checked;touch({rerender:true});});const relationLabel=el('label','','lens-builder-toggle');uiChildren(relationLabel,'append',relationToggle,el('span',ui("Показывать связи и окружение")));uiChildren(body,'append',relationLabel);
    if(draft.relations)uiChildren(body,'append',choicePicker({draft,context,key:'predicates',title:ui("Типы связей"),onChange:touch,stateStore:choiceStates}));
    if(draft.relations||draft.conditions.relations.length)uiChildren(body,'append',createConditionEditor({draft,context,kind:'relations',onChange:touch}));
    const grid=el('div','','lens-builder-grid');
    if(draft.relations){
      for(const [label,key,choices] of [[ui("Глубина"),'depth',[[0,ui("Только исходные")],[1,ui("1 шаг")],[2,ui("2 шага")],[3,ui("3 шага")]]],[ui("Направление"),'direction',[['either',ui("В обе стороны")],['outgoing',ui("По связям →")],['incoming',ui("Против связей ←")]]],[ui("Подробность связей"),'profile',[['all',ui("Все типы, включая текст")],['overview',ui("Обзор без структуры текста")]]]]){
        const select=el('select');for(const [value,title] of choices)uiChildren(select,'append',option(value,title));select.value=String(draft[key]);select.addEventListener('change',()=>{draft[key]=key==='depth'?Number(select.value):select.value;touch();});uiChildren(grid,'append',field(label,select));
      }
    }
    const limit=el('select');for(const value of [10,20,40])uiChildren(limit,'append',option(value,ui("До {0}",[value])));limit.value=String(draft.limit);limit.addEventListener('change',()=>{draft.limit=Number(limit.value);touch();});uiChildren(grid,'append',field(ui("Звёзд в области"),limit));uiChildren(body,'append',grid);
  }

  function renderPreview(){
    uiChildren(previewRegion,'replaceChildren');
    if(!draft||!context)return;
    const summary=el('details','','lens-builder-query');uiChildren(summary,'append',el('summary',ui("Настройки линзы")));const lines=el('ul');for(const line of queryLines(draft,context))uiChildren(lines,'append',el('li',line));uiChildren(summary,'append',lines,el('p',ui("Источники настроек: {0}",[sourceNames(draft.sources)]),'lens-builder-note'));uiChildren(previewRegion,'append',summary);
    if(!preview)return;
    const counts=localizedPacketCounts(preview);uiChildren(previewRegion,'append',el('strong',ui("Исходных записей: {0}; связей: {1}",[counts.nodes,counts.relations])));
    if(counts.matched!==null)uiChildren(previewRegion,'append',el('p',ui("Условиями выбрано: {0}.",[counts.matched])));
    if(counts.truncatedNodes||counts.truncatedRelations)uiChildren(previewRegion,'append',el('p',ui("Результат ограничен: узлы {0}, связи {1}.",[counts.truncatedNodes,counts.truncatedRelations]),'lens-builder-warning'));
    if(area){const delta=lensDelta(area.packet,preview);uiChildren(previewRegion,'append',el('p',ui("Изменение исходных записей: +{0} / −{1}; связей: +{2} / −{3}.",[delta.nodes.added,delta.nodes.removed,delta.relations.added,delta.relations.removed])));}
    if(!counts.nodes)uiChildren(previewRegion,'append',el('p',ui("Совпадений нет. Измените источники или условие."),'lens-builder-warning'));
    if(previewRevision!==draftRevision)uiChildren(previewRegion,'append',el('p',ui("Результат относится к прежним настройкам. Запросите просмотр снова после правки."),'lens-builder-warning'));
    if(previewError)uiChildren(previewRegion,'append',el('p',previewError,'lens-builder-warning'));
  }

  function renderFooter(){
    uiChildren(footer,'replaceChildren');
    const previewButton=button(previewing?ui("Считаю…"):ui("Предпросмотр"),()=>void runPreview(),'lens-builder-primary');previewButton.disabled=loading||previewing||!draft||!validDraft()||sourceMismatch()||Boolean(areaChoice?.required);uiChildren(footer,'append',previewButton);
    const apply=button(ui("Открыть область"),()=>void applyPreview(),'lens-builder-primary');apply.disabled=loading||previewing||!previewApplicable();uiChildren(footer,'append',apply);
    const save=button(ui("Сохранить линзу"),()=>void saveDraft(),'lens-builder-button');save.disabled=loading||previewing||!draft||!validDraft()||sourceMismatch()||Boolean(areaChoice?.required);uiChildren(footer,'append',save);
    uiChildren(footer,'append',closeButton.cloneNode(false));
    const last=footer.lastElementChild;uiText(last,ui("Закрыть"));last.type='button';uiAttribute(last,'aria-label',ui("Закрыть конструктор"));last.addEventListener('click',()=>close());
  }

  function render(){
    const validation=draft?draftValidationError():'';
    try{
      root.dataset.state=error||validation?'error':previewError?'preview-error':preview&&previewRevision!==draftRevision?'preview-stale':preview?'preview':loading?'loading':'ready';
      renderArea();renderBody();renderPreview();renderFooter();uiText(status,error||validation||previewError||sourceMismatch()?error||validation||previewError||ui("Каталог и область относятся к разным версиям данных. Обновите область."):previewing?ui("Получаю результат…"):preview&&previewRevision!==draftRevision?ui("Условия изменены. Обновите предпросмотр."):'');
    }catch(problem){
      error=ui("Не удалось отобразить конструктор. Повторите загрузку.");root.dataset.state='error';
      try{onError?.(problem);}catch{}
      uiChildren(body,'replaceChildren',el('p',error,'lens-builder-warning'));
      uiChildren(previewRegion,'replaceChildren');uiChildren(footer,'replaceChildren');
      const retry=button(ui("Повторить загрузку"),()=>void load({refresh:true}),'lens-builder-link');uiChildren(footer,'append',retry);
      const closeAction=button(ui("Закрыть"),()=>close(),'lens-builder-button');uiChildren(footer,'append',closeAction);
      uiText(status,error);
    }
  }

  function touch({rerender=false}={}){if(!draft)return;requests.cancel('preview');previewing=false;draftRevision++;previewError='';error='';if(rerender)render();else{renderPreview();renderFooter();}}

  async function load({refresh=false}={}){
    if(destroyed)return false;
    const requestedDraft=pendingDraft;pendingDraft=undefined;
    requests.cancelAll();previewing=false;loading=true;error='';previewError='';context=null;area=null;draft=null;preview=null;areaChoice=null;invalidDraft=null;previewRevision=-1;areaRevision=null;render();const token=++draftRevision;
    try{
      const loaded=currentArea();area=loaded;areaRevision=loaded?.packet?.source_revision??null;const answer=await requests.run('catalog',signal=>catalogLoader.load(signal,{refresh}));
      if(!answer.current||token!==draftRevision||!opened)return false;
      if(!answer.value){areaProblem(ui("Словарь данных изменился. Обновите каталог и проверьте выбранные условия."));return false;}
      context=answer.value;
      if(area&&context.catalog.source_revision!==area.packet.source_revision){areaProblem(ui("Каталог и область относятся к разным версиям данных. Обновите область и загрузите каталог снова."));return false;}
      const prepared=prepareLensBuilderDraft({packet:area?.packet??null,context,selection:area?.selection??null,draft:requestedDraft});areaChoice=prepared.areaChoice;invalidDraft=null;
      try{draft=validateDraft(prepared.draft);}catch(problem){invalidDraft=prepared.draft;draft=null;areaProblem(problem);}
      preview=null;previewRevision=-1;draftRevision=0;
      return Boolean(draft);
    }catch(problem){if(opened)areaProblem(problem);return false;}
    finally{if(opened){loading=false;render();}}
  }

  let pendingDraft;
  async function open({draft:nextDraft}={}){
    if(destroyed)return false;
    if(!opened){opened=true;returnFocus=document.activeElement;root.hidden=false;try{onOpen?.();}catch(problem){report(problem);} }
    pendingDraft=nextDraft===undefined?undefined:copy(nextDraft);await load();if(!destroyed)root.querySelector('.lens-builder-close')?.focus({preventScroll:true});return Boolean(opened&&draft&&!error);
  }

  async function runPreview(){
    if(!draft||!context||!validDraft()||sourceMismatch()||areaChoice?.required)return false;
    const revisionAtStart=draftRevision;let current;
    try{current=currentArea();if(!areaStillCurrent(area,current))throw new ContractError(ui("Область изменилась. Откройте конструктор заново перед просмотром."));}catch(problem){areaProblem(problem);render();return false;}
    previewing=true;previewError='';error='';render();
    try{
      const pending=copy(draft),answer=await requests.run('preview',signal=>previewDraft(client,pending,context,signal));
      if(!answer.current||revisionAtStart!==draftRevision||!opened)return false;
      const sourceRevision=area?.packet?.source_revision??context.catalog.source_revision;
      if(answer.value.source_revision!==sourceRevision)throw new ContractError(ui("Результат относится к другой версии данных."));
      preview=answer.value;previewRevision=revisionAtStart;previewError='';return true;
    }catch(problem){if(revisionAtStart===draftRevision&&opened){previewError=ui("Не удалось выполнить просмотр. Проверьте условия и повторите.");try{onError?.(problem);}catch{}}return false;}
    finally{if(revisionAtStart===draftRevision){previewing=false;render();}}
  }

  async function applyPreview(){
    if(!previewApplicable())return false;
    try{
      const current=currentArea();if(!areaStillCurrent(area,current))throw new ContractError(ui("Область изменилась. Откройте конструктор заново перед открытием."));
      await onApply?.({packet:preview,draft:copy(draft)});close();return true;
    }catch(problem){report(problem);render();return false;}
  }

  async function saveDraft(){
    if(!draft||!validDraft()||sourceMismatch()||areaChoice?.required)return false;
    try{const current=currentArea();if(!areaStillCurrent(area,current))throw new ContractError(ui("Область изменилась. Откройте конструктор заново перед сохранением."));compileDraft(draft,context);await onSave?.({draft:copy(draft)});error='';status.dataset.state='saved';uiText(status,ui("Настройки линзы переданы на сохранение."));return true;}
    catch(problem){report(problem);render();return false;}
  }

  function close({restore=true}={}){
    if(!opened)return false;opened=false;requests.cancelAll();loading=false;previewing=false;root.hidden=true;try{onClose?.();}catch(problem){report(problem);}if(restore){const target=returnFocus?.isConnected&&!returnFocus.closest('[hidden]')?returnFocus:null;target?.focus({preventScroll:true});}return true;
  }

  function destroy(){if(destroyed)return;destroyed=true;if(opened)close({restore:false});requests.cancelAll();catalogLoader.clear();root.remove();}
  render();
  return {open,close,destroy,element:root,state:()=>({opened,loading,hasDraft:Boolean(draft),hasInvalidDraft:Boolean(invalidDraft),hasPreview:Boolean(preview),previewRevision,draftRevision,areaRevision,error,previewError,areaChoice})};
}

export default mountLensBuilder;
