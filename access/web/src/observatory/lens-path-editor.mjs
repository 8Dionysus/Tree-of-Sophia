import {ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
import {ContractError} from './knowledge-client.mjs';
import {compileConditions,validateConditions} from './lens-conditions.mjs';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {createConditionEditor} from './lens-condition-editor.mjs';

export const MAX_PATH_CONDITIONS=4;
export const MAX_PATH_STEPS=4;
export const MAX_PATH_FILTERS=32;

const PATH_ID=/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$(?![\s\S])/;
const PROPERTY_ID=/^tos\.property\.[a-z0-9-]+$(?![\s\S])/;
const OPERATORS=new Set(['eq','neq','in','contains','prefix','exists','gt','gte','lt','lte']);
const DIRECTIONS=['outgoing','incoming','either'];
const MATCHES=['all','any'];
const QUANTIFIERS=['exists','not_exists'];
const fail=message=>{throw new ContractError(message);};
const record=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const ownKeys=(value,allowed)=>Object.keys(value).some(key=>!allowed.includes(key));
const strings=(value,max,length=1024)=>Array.isArray(value)&&value.length<=max&&new Set(value).size===value.length
  &&value.every(item=>typeof item==='string'&&item.length>0&&item.length<=length);
const finiteScalar=value=>value===null||typeof value==='boolean'||typeof value==='string'&&value.length<=1024
  ||typeof value==='number'&&Number.isFinite(value);
const defaultQuery=()=>({enabled:true,match:'all',conditions:[]});
const clone=value=>structuredClone(value);

export function defaultPathStep(){return {direction:'outgoing',kinds:[],predicates:[],nodeQuery:defaultQuery(),relationQuery:defaultQuery()};}
export function defaultPath(pathId='path-1'){return {pathId,quantifier:'exists',steps:[defaultPathStep()]};}

function normalizeQuery(value,kind,maxConditions=MAX_PATH_FILTERS){
  if(value===undefined)return defaultQuery();
  if(!record(value)||ownKeys(value,['enabled','match','conditions']))fail(ui("Условие шага имеет неподдерживаемую форму."));
  const enabled=value.enabled===undefined?true:value.enabled;
  const match=value.match===undefined?'all':value.match;
  if(typeof enabled!=='boolean'||!MATCHES.includes(match))fail(ui("Условие шага имеет неподдерживаемые параметры."));
  const conditions=value.conditions===undefined?[]:value.conditions;
  if(!Array.isArray(conditions)||conditions.length>maxConditions)fail(ui("В одной группе шага можно сохранить до {0} условий.",[maxConditions]));
  const checked=validateConditions(kind==='nodes'?{nodes:conditions,relations:[]}:{nodes:[],relations:conditions},{maxConditions})[kind];
  return {enabled,match,conditions:checked};
}

function normalizeStep(value){
  if(!record(value)||ownKeys(value,['direction','kinds','predicates','nodeQuery','relationQuery']))fail(ui("Шаг пути имеет неподдерживаемую форму."));
  const direction=value.direction===undefined?'outgoing':value.direction;
  if(!DIRECTIONS.includes(direction))fail(ui("Направление шага больше не поддерживается."));
  const kinds=value.kinds===undefined?[]:value.kinds;
  const predicates=value.predicates===undefined?[]:value.predicates;
  if(!strings(kinds,100)||!strings(predicates,100))fail(ui("Типы шага неполны или превышают допустимый размер."));
  return {direction,kinds:[...kinds],predicates:[...predicates],nodeQuery:normalizeQuery(value.nodeQuery,'nodes'),relationQuery:normalizeQuery(value.relationQuery,'relations')};
}

export function validatePathDraft(value){
  if(value===undefined)return [];
  if(!Array.isArray(value)||value.length>MAX_PATH_CONDITIONS)fail(ui("Можно добавить до {0} условий пути.",[MAX_PATH_CONDITIONS]));
  const ids=new Set();
  return value.map(path=>{
    if(!record(path)||ownKeys(path,['pathId','quantifier','steps']))fail(ui("Условие пути имеет неподдерживаемую форму."));
    const pathId=path.pathId;
    if(typeof pathId!=='string'||!PATH_ID.test(pathId)||ids.has(pathId))fail(ui("Идентификаторы условий пути должны быть уникальными и безопасными."));
    ids.add(pathId);
    const quantifier=path.quantifier===undefined?'exists':path.quantifier;
    if(!QUANTIFIERS.includes(quantifier))fail(ui("Квантификатор пути больше не поддерживается."));
    if(!Array.isArray(path.steps)||path.steps.length<1||path.steps.length>MAX_PATH_STEPS)fail(ui("Путь должен содержать от 1 до {0} шагов.",[MAX_PATH_STEPS]));
    return {pathId,quantifier,steps:path.steps.map(normalizeStep)};
  });
}

function integer(value){return Number.isInteger(value)&&value>=1;}
function minimum(values){return Math.min(...values.filter(integer));}

export function pathLimits(context){
  const schema=context?.schema,properties=schema?.properties?.path_query,path=schema?.$defs?.pathCondition;
  const stepItems=path?.properties?.steps?.items,nodeQuery=schema?.$defs?.nodeQuery,relationQuery=schema?.$defs?.relationQuery;
  const pathMax=properties?.maxItems,stepMax=path?.properties?.steps?.maxItems;
  const nodeFilterMax=nodeQuery?.properties?.filters?.maxItems,relationFilterMax=relationQuery?.properties?.filters?.maxItems;
  const nodeMatches=nodeQuery?.properties?.match?.enum,relationMatches=relationQuery?.properties?.match?.enum;
  if(!integer(pathMax)||!integer(stepMax)||!integer(nodeFilterMax)||!integer(relationFilterMax)
    ||!Array.isArray(path?.properties?.quantifier?.enum)||!Array.isArray(stepItems?.properties?.direction?.enum)
    ||!Array.isArray(nodeMatches)||!Array.isArray(relationMatches))
    fail(ui("Схема линз не объявляет границы условий пути."));
  const advertised=context?.catalog?.capabilities?.path_query;
  if(advertised!==undefined&&(!record(advertised)
    ||!integer(advertised.conditions)||!integer(advertised.steps_per_condition)
    ||!Array.isArray(advertised.quantifiers)||!advertised.quantifiers.every(value=>typeof value==='string')
    ||advertised.combination!=='all'||advertised.scope!=='node-selector-roots-and-selected-sources'))
    fail(ui("Каталог объявляет неподдерживаемые границы условий пути."));
  const conditions=minimum([MAX_PATH_CONDITIONS,pathMax,advertised?.conditions]);
  const steps=minimum([MAX_PATH_STEPS,stepMax,advertised?.steps_per_condition]);
  const quantifiers=path.properties.quantifier.enum.filter(value=>QUANTIFIERS.includes(value)
    &&(!advertised||advertised.quantifiers.includes(value)));
  const directions=stepItems.properties.direction.enum.filter(value=>DIRECTIONS.includes(value));
  const allowedMatches=matches=>matches.filter(value=>MATCHES.includes(value));
  const nodeMatchValues=allowedMatches(nodeMatches),relationMatchValues=allowedMatches(relationMatches);
  if(!integer(conditions)||!integer(steps)||!quantifiers.length||!directions.length||!nodeMatchValues.length||!relationMatchValues.length)
    fail(ui("Схема линз не объявляет совместимые условия пути."));
  return {conditions,steps,nodeFilters:nodeFilterMax,relationFilters:relationFilterMax,quantifiers,directions,
    nodeMatches:nodeMatchValues,relationMatches:relationMatchValues};
}

function catalogIds(catalog,key){
  const values=key==='kinds'?catalog?.node_kinds:catalog?.predicates;
  return new Set((Array.isArray(values)?values:[]).map(item=>item?.[key==='kinds'?'kind_id':'predicate_id']).filter(item=>typeof item==='string'));
}

export function compilePathQuery(value,context){
  const paths=validatePathDraft(value);
  // An empty path list is the legacy request: it must remain executable even
  // when an older catalog/schema has no path extension yet.
  if(!paths.length)return [];
  const limits=pathLimits(context),catalog=context?.catalog;
  const supportsNodeIn=catalog?.capabilities?.filter_operators?.includes('in')
    &&context?.schema?.$defs?.nodeFilter?.properties?.op?.enum?.includes('in');
  const supportsRelationIn=catalog?.capabilities?.filter_operators?.includes('in')
    &&context?.schema?.$defs?.relationFilter?.properties?.op?.enum?.includes('in');
  if(paths.length>limits.conditions)fail(ui("Сервер допускает меньше условий пути."));
  const kinds=catalogIds(catalog,'kinds'),predicates=catalogIds(catalog,'predicates');
  for(const path of paths){
    if(!limits.quantifiers.includes(path.quantifier))fail(ui("Сервер не поддерживает этот квантификатор пути."));
    if(path.steps.length>limits.steps)fail(ui("Сервер допускает меньше шагов в условии пути."));
    for(const step of path.steps){
      if(!limits.directions.includes(step.direction))fail(ui("Сервер не поддерживает это направление шага."));
      if(!limits.nodeMatches.includes(step.nodeQuery.match)||!limits.relationMatches.includes(step.relationQuery.match))fail(ui("Сервер не поддерживает это сочетание условий шага."));
      if((step.kinds.length&&!supportsNodeIn)||(step.predicates.length&&!supportsRelationIn))fail(ui("Сервер не объявляет операцию выбора типов для условий пути."));
      if(step.kinds.some(id=>!kinds.has(id))||step.predicates.some(id=>!predicates.has(id)))fail(ui("Словарь типов шага изменился. Обновите каталог."));
    }
  }
  return paths.map(path=>({path_id:path.pathId,quantifier:path.quantifier,steps:path.steps.map(step=>{
    const nodeFilters=step.kinds.length?[{field:'kind_id',op:'in',value:[...step.kinds]}]:[];
    nodeFilters.push(...compileConditions(step.nodeQuery.conditions,context,'nodes',{reserveTypeSlot:step.kinds.length>0}));
    const relationFilters=step.predicates.length?[{field:'predicate_id',op:'in',value:[...step.predicates]}]:[];
    relationFilters.push(...compileConditions(step.relationQuery.conditions,context,'relations',{reserveTypeSlot:step.predicates.length>0}));
    if(nodeFilters.length>limits.nodeFilters||relationFilters.length>limits.relationFilters)fail(ui("Условия шага превышают границу схемы."));
    return {direction:step.direction,
      node_query:{enabled:step.nodeQuery.enabled,match:step.nodeQuery.match,filters:nodeFilters},
      relation_query:{enabled:step.relationQuery.enabled,match:step.relationQuery.match,filters:relationFilters}};
  })}));
}

function filterDraft(raw,kind){
  if(!record(raw)||ownKeys(raw,['field','property_id','op','value'])
    ||(('field' in raw)===('property_id' in raw))||typeof raw.op!=='string'||!OPERATORS.has(raw.op)
    ||!Object.hasOwn(raw,'value')||(kind==='relations'&&'property_id' in raw)
    ||('property_id' in raw&& (typeof raw.property_id!=='string'||!PROPERTY_ID.test(raw.property_id)))
    ||('field' in raw&&typeof raw.field!=='string')
    ||(Array.isArray(raw.value)?raw.value.length>100||raw.value.some(item=>!finiteScalar(item)):!finiteScalar(raw.value)))
    fail(ui("Фильтр шага не соответствует схеме."));
  return {selector:'property_id' in raw?'property_id':'field',id:'property_id' in raw?raw.property_id:raw.field,op:raw.op,value:clone(raw.value)};
}

function queryFromSpec(value,kind){
  if(value===undefined)return {query:defaultQuery(),kinds:[],predicates:[]};
  if(!record(value)||ownKeys(value,['enabled','match','filters']))fail(ui("Запрос шага не соответствует схеме."));
  const enabled=value.enabled===undefined?true:value.enabled,match=value.match===undefined?'all':value.match;
  if(typeof enabled!=='boolean'||!MATCHES.includes(match))fail(ui("Запрос шага имеет неподдерживаемые параметры."));
  const filters=value.filters===undefined?[]:value.filters;
  if(!Array.isArray(filters)||filters.length>32)fail(ui("Условия шага превышают границу схемы."));
  const conditions=[];let selectedFilter=null;
  for(const raw of filters){
    const rule=filterDraft(raw,kind);
    const typeField=kind==='nodes'?'kind_id':'predicate_id';
    const selectable=rule.selector==='field'&&rule.id===typeField&&rule.op==='in'
      &&Array.isArray(rule.value)&&rule.value.length>0&&strings(rule.value,100)&&new Set(rule.value).size===rule.value.length;
    if(selectable&&!selectedFilter)selectedFilter=rule;
    else conditions.push(rule);
  }
  const draftQuery=normalizeQuery({enabled,match,conditions},kind);
  const selected=selectedFilter?.value||[];
  return {query:draftQuery,kinds:kind==='nodes'?selected:[],predicates:kind==='relations'?selected:[]};
}

export function pathDraftFromSpec(value){
  if(value===undefined)return [];
  if(!Array.isArray(value)||value.length>MAX_PATH_CONDITIONS)fail(ui("Условия пути превышают границу схемы."));
  const paths=value.map(path=>{
    if(!record(path)||ownKeys(path,['path_id','quantifier','steps'])||typeof path.path_id!=='string'||!PATH_ID.test(path.path_id)
      ||path.quantifier!==undefined&&!QUANTIFIERS.includes(path.quantifier)||!Array.isArray(path.steps)||path.steps.length<1||path.steps.length>MAX_PATH_STEPS)
      fail(ui("Условие пути не соответствует схеме."));
    return {pathId:path.path_id,quantifier:path.quantifier===undefined?'exists':path.quantifier,steps:path.steps.map(step=>{
      if(!record(step)||ownKeys(step,['direction','node_query','relation_query'])
        ||step.direction!==undefined&&!DIRECTIONS.includes(step.direction))fail(ui("Шаг пути не соответствует схеме."));
      const node=queryFromSpec(step.node_query,'nodes'),relation=queryFromSpec(step.relation_query,'relations');
      return {direction:step.direction===undefined?'outgoing':step.direction,kinds:node.kinds,predicates:relation.predicates,
        nodeQuery:node.query,relationQuery:relation.query};
    })};
  });
  return validatePathDraft(paths);
}

export const validatePaths=validatePathDraft;
export const draftPathsFromSpec=pathDraftFromSpec;

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node,text);node.className=className;return node;};
const option=(value,text)=>{const node=el('option',text);node.value=value;return node;};
const button=(text,action,className='sc-builder-link')=>{const node=el('button',text,className);node.type='button';node.addEventListener('click',action);return node;};
const field=(text,input)=>{const label=el('label','','sc-builder-field');uiChildren(label,'append',el('span',text),input);return label;};
const technical=(summary,text)=>{const details=el('details','','sc-technical-disclosure');uiChildren(details,'append',el('summary',summary),el('p',text));return details;};

function typePicker({draft,context,step,key,title,onChange}){
  const details=el('details','','sc-path-types'),summary=el('summary'),controls=el('div','','sc-path-type-controls'),search=el('input'),list=el('div','','sc-path-type-list');
  search.type='search';uiAttribute(search,'placeholder',ui("Найти в каталоге…"));uiAttribute(search,'aria-label',ui("Найти тип: {0}",[title]));
  const vocabulary=lensVocabulary(context.catalog,key);
  let visibleLimit=40;
  const updateSummary=()=>uiText(summary,`${String(title)} · ${step[key].length?ui("{0} выбрано",[step[key].length]):ui("любые")}`);
  function redraw(){
    const groups=vocabularyGroups(vocabulary,{sources:draft.sources,selected:step[key],query:search.value});uiChildren(list,'replaceChildren');
    for(const group of groups){
      const section=el('details','','sc-path-type-group');section.open=Boolean(search.value)||group.items.some(item=>item.selected);const heading=el('summary',`${String(group.title)} · ${group.items.length}`);uiChildren(section,'append',heading);
      for(const item of group.items.slice(0,visibleLimit)){
        const input=el('input');input.type='checkbox';input.value=item.id;input.checked=item.selected;
        input.addEventListener('change',()=>{step[key]=input.checked?[...step[key],item.id]:step[key].filter(id=>id!==item.id);updateSummary();onChange();});
        const name=el('span',item.title);if(item.title===item.id)uiAttribute(name,'title',item.id);const label=el('label','','sc-builder-choice');uiChildren(label,'append',input,name);uiChildren(section,'append',label);
      }
      if(group.items.length>visibleLimit)uiChildren(section,'append',button(ui("Ещё варианты · {0}",[group.items.length-visibleLimit]),()=>{visibleLimit+=40;redraw();},'sc-builder-link'));
      uiChildren(list,'append',section);
    }
    if(!groups.length)uiChildren(list,'append',el('p',ui("В каталоге нет подходящих типов."),'sc-builder-note'));
  }
  search.addEventListener('input',redraw);updateSummary();uiChildren(controls,'append',search);uiChildren(details,'append',summary,controls,list);redraw();return details;
}

function queryEditor({draft,context,step,kind,limits,onChange}){
  const isNode=kind==='nodes',query=isNode?step.nodeQuery:step.relationQuery,section=el('section','','sc-path-query');
  uiAttribute(section,'aria-label',isNode?ui("Условия узлов шага"):ui("Условия связей шага"));
  const title=isNode?ui("Конечный узел"):ui("Связь шага"),heading=el('h5',title),enabled=el('input');enabled.type='checkbox';enabled.checked=query.enabled;
  const enabledText=el('span',query.enabled?ui("Условие включено"):ui("Условие сохранено, но выключено"));
  enabled.addEventListener('change',()=>{query.enabled=enabled.checked;uiText(enabledText,query.enabled?ui("Условие включено"):ui("Условие сохранено, но выключено"));onChange();});
  const enabledLabel=el('label','','sc-path-enabled');uiChildren(enabledLabel,'append',enabled,enabledText);
  const match=el('select'),matchValues=isNode?limits.nodeMatches:limits.relationMatches;
  for(const value of matchValues)uiChildren(match,'append',option(value,value==='all'?ui("Все условия"):ui("Любое условие")));match.value=query.match;
  match.addEventListener('change',()=>{query.match=match.value;onChange();});
  uiChildren(section,'append',heading,enabledLabel,field(ui("Сочетание условий"),match));
  if(isNode)uiChildren(section,'append',typePicker({draft,context,step,key:'kinds',title:ui("Типы конечных узлов"),onChange}));
  else uiChildren(section,'append',typePicker({draft,context,step,key:'predicates',title:ui("Типы отношений"),onChange}));
  const stepDraft={scope:'all',relations:true,conditions:{nodes:isNode?query.conditions:[],relations:isNode?[]:query.conditions}};
  uiChildren(section,'append',conditionEditor({draft:stepDraft,context,kind,maxConditions:isNode?limits.nodeFilters:limits.relationFilters,
    note:ui("Сочетание условий для шага задаётся настройкой выше."),onChange}));
  return section;
}

function conditionEditor({draft,context,kind,maxConditions,note,onChange}){
  return createConditionEditor({draft,context,kind,maxConditions,note,onChange});
}

export function createLensPathEditor({draft,context,onChange=()=>{}}){
  if(draft?.paths===undefined)draft.paths=[];
  if(!draft||!Array.isArray(draft.paths))fail(ui("Условия пути имеют неподдерживаемую форму."));
  draft.paths=validatePathDraft(draft.paths);
  let limits,legacyNoPath=false;
  try{limits=pathLimits(context);}catch(error){
    const schemaHasPath=Boolean(context?.schema?.properties?.path_query||context?.schema?.$defs?.pathCondition);
    if(draft.paths.length||schemaHasPath||!(error instanceof ContractError))throw error;
    legacyNoPath=true;limits={conditions:0,steps:0,nodeFilters:0,relationFilters:0,quantifiers:[],directions:[],nodeMatches:[],relationMatches:[]};
  }
  const section=el('section','','sc-path-editor');
  uiAttribute(section,'aria-label',ui("Условия пути"));
  function render(){
    uiChildren(section,'replaceChildren');
    uiChildren(section,'append',el('h4',ui("Пути к условиям")),el('p',ui("Каждое условие начинается от узлов, найденных основной линзой."),'sc-builder-note'),technical(ui("Технические сведения"),ui("Шаги остаются в выбранных источниках; not_exists означает отсутствие только в этой области источников.")));
    if(legacyNoPath)uiChildren(section,'append',el('p',ui("Проверка пути недоступна в текущем каталоге; основной запрос работает как прежде."),'sc-builder-warning'),technical(ui("Технические сведения"),ui("Схема линз пока не объявила path_query. Основной запрос сохраняет прежнюю семантику.")));
    if(!draft.paths.length)uiChildren(section,'append',el('p',ui("Пути не добавлены. Основной запрос сохраняет прежнюю семантику."),'sc-builder-note'));
    draft.paths.forEach((path,pathIndex)=>{
      const box=el('fieldset','','sc-path-condition'),legend=el('legend',ui("Условие пути {0}",[pathIndex+1]));uiChildren(box,'append',legend);
      const idInput=el('input');idInput.type='text';idInput.maxLength=128;idInput.value=path.pathId;uiAttribute(idInput,'aria-label',ui("Идентификатор условия пути {0}",[pathIndex+1]));idInput.addEventListener('input',()=>{path.pathId=idInput.value;onChange();});
      const quantifier=el('select');for(const value of limits.quantifiers)uiChildren(quantifier,'append',option(value,value==='exists'?ui("Есть такой путь"):ui("Такого пути нет")));quantifier.value=path.quantifier;quantifier.addEventListener('change',()=>{path.quantifier=quantifier.value;onChange();});
      const top=el('div','','sc-path-top');uiChildren(top,'append',field(ui("Имя условия"),idInput),field(ui("Проверка"),quantifier),button(ui("Удалить условие"),()=>{draft.paths.splice(pathIndex,1);render();onChange();},'sc-builder-link'));uiChildren(box,'append',top);
      path.steps.forEach((step,stepIndex)=>{
        const item=el('fieldset','','sc-path-step'),stepLegend=el('legend',ui("Шаг {0} из {1}",[stepIndex+1,path.steps.length]));uiChildren(item,'append',stepLegend);
        const direction=el('select');for(const value of limits.directions)uiChildren(direction,'append',option(value,{outgoing:ui("По направлению →"),incoming:ui("Против направления ←"),either:ui("В любую сторону")}[value]));direction.value=step.direction;direction.addEventListener('change',()=>{step.direction=direction.value;onChange();});
        const removeStep=button(ui("Удалить шаг"),()=>{path.steps.splice(stepIndex,1);render();onChange();},'sc-builder-link');removeStep.disabled=path.steps.length<=1;
        const stepTop=el('div','','sc-path-step-top');uiChildren(stepTop,'append',field(ui("Направление"),direction),removeStep);uiChildren(item,'append',stepTop,
          queryEditor({draft,context,step,kind:'nodes',limits,onChange}),queryEditor({draft,context,step,kind:'relations',limits,onChange}));uiChildren(box,'append',item);
      });
      const addStep=button(ui("＋ Добавить шаг"),()=>{path.steps.push(defaultPathStep());render();onChange();});addStep.disabled=path.steps.length>=limits.steps;uiChildren(box,'append',addStep);uiChildren(section,'append',box);
    });
    const addPath=button(ui("＋ Добавить условие пути"),()=>{const used=new Set(draft.paths.map(path=>path.pathId));let index=1;while(used.has(`path-${index}`))index++;draft.paths.push(defaultPath(`path-${index}`));render();onChange();});addPath.disabled=legacyNoPath||draft.paths.length>=limits.conditions;uiChildren(section,'append',addPath);
  }
  render();return section;
}

export const createPathEditor=createLensPathEditor;
