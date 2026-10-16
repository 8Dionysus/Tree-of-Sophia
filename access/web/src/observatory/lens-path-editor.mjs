import {ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
import {ContractError} from './knowledge-client.mjs';
import {compileConditions} from './lens-conditions.mjs';
import {attachConditionValues,compilePathRules,conditionRequest,importPathDraftRules,normalizePathRule,observatoryPathLimits} from './lens-conditions-rust.mjs';
import {lensVocabulary,vocabularyGroups} from './lens-vocabulary.mjs';
import {createConditionEditor} from './lens-condition-editor.mjs';

export const MAX_PATH_CONDITIONS=4;
export const MAX_PATH_STEPS=4;
export const MAX_PATH_FILTERS=32;

const fail=message=>{throw new ContractError(message);};
const record=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const keys=(value,allowed)=>Object.keys(value).slice(0,allowed.length+1);
const defaultQuery=()=>({enabled:true,match:'all',conditions:[]});
const clone=value=>structuredClone(value);

export function defaultPathStep(){return {direction:'outgoing',kinds:[],predicates:[],nodeQuery:defaultQuery(),relationQuery:defaultQuery()};}
export function defaultPath(pathId='path-1'){return {pathId,quantifier:'exists',steps:[defaultPathStep()]};}

const valueDescriptor=value=>{
  if(value===undefined)return {kind:'undefined'};
  if(typeof value==='string')return {kind:'string',value:value.slice(0,129)};
  if(typeof value==='boolean')return {kind:'boolean',value};
  return {kind:'invalid'};
};
const mappedItems=(value,mapper)=>Array.from({length:value.length},(_,index)=>index in value?mapper(value[index]):{hole:true});
function typeListDescriptor(value){
  if(value===undefined)return {kind:'array',length:0,items:[]};
  if(!Array.isArray(value))return {kind:'invalid'};
  const length=value.length;
  if(length>100)return {kind:'array',length,items:[]};
  return {kind:'array',length,items:mappedItems(value,item=>
    typeof item==='string'?{kind:'string',value:item}:{kind:'invalid'})};
}
function queryDescriptor(value,kind){
  if(value===undefined)return {kind:'undefined'};
  if(!record(value))return {kind:'invalid'};
  const keyList=keys(value,['enabled','match','conditions']);
  const enabled=value.enabled,match=value.match,conditions=value.conditions;
  const groups=kind==='nodes'?{nodes:Array.isArray(conditions)?conditions:[],relations:[]}:{nodes:[],relations:Array.isArray(conditions)?conditions:[]};
  const descriptor=conditionRequest(groups,MAX_PATH_FILTERS);
  return {kind:'object',keys:keyList,
    enabled:valueDescriptor(enabled),match:valueDescriptor(match),
    conditions_kind:conditions===undefined?'undefined':Array.isArray(conditions)?'array':'invalid',
    conditions_length:Array.isArray(conditions)?conditions.length:null,condition_kind:kind,condition_request:descriptor};
}
function stepDescriptor(value){
  if(!record(value))return {kind:'invalid'};
  const keyList=keys(value,['direction','kinds','predicates','nodeQuery','relationQuery']);
  const direction=value.direction,kinds=value.kinds,predicates=value.predicates,nodeQuery=value.nodeQuery,relationQuery=value.relationQuery;
  return {kind:'object',keys:keyList,
    direction:valueDescriptor(direction),kinds:typeListDescriptor(kinds),predicates:typeListDescriptor(predicates),
    node_query:queryDescriptor(nodeQuery,'nodes'),relation_query:queryDescriptor(relationQuery,'relations')};
}
function stepListDescriptor(value){
  if(!Array.isArray(value))return {kind:'invalid'};
  return {kind:'array',length:value.length,items:value.length>0&&value.length<=MAX_PATH_STEPS?mappedItems(value,stepDescriptor):[]};
}
function pathDescriptor(value){
  if(!record(value))return {kind:'invalid'};
  const keyList=keys(value,['pathId','quantifier','steps']);
  const pathId=value.pathId,quantifier=value.quantifier,steps=value.steps;
  return {kind:'object',keys:keyList,path_id:valueDescriptor(pathId),
    quantifier:valueDescriptor(quantifier),steps:stepListDescriptor(steps)};
}
function pathListDescriptor(value){
  if(value===undefined)return {kind:'undefined'};
  if(!Array.isArray(value))return {kind:'invalid'};
  return {kind:'array',length:value.length,items:value.length<=MAX_PATH_CONDITIONS?mappedItems(value,pathDescriptor):[]};
}
const isHole=value=>value?.hole===true;
function normalizedQuery(original,normalized,kind){
  const source=original?.conditions===undefined?[]:original.conditions;
  const value=kind==='nodes'?{nodes:source,relations:[]}:{nodes:[],relations:source};
  const metadata=kind==='nodes'?{nodes:normalized.conditions,relations:[]}:{nodes:[],relations:normalized.conditions};
  return {...normalized,conditions:attachConditionValues(value,metadata)[kind]};
}
function restoreSteps(original,normalized){
  const output=new Array(normalized.length);
  for(let index=0;index<normalized.length;index++){
    if(!(index in original)||isHole(normalized[index]))continue;
    const source=original[index],step=normalized[index];
    output[index]={direction:step.direction,kinds:source.kinds===undefined?[]:[...source.kinds],
      predicates:source.predicates===undefined?[]:[...source.predicates],
      nodeQuery:normalizedQuery(source.nodeQuery,step.nodeQuery,'nodes'),
      relationQuery:normalizedQuery(source.relationQuery,step.relationQuery,'relations')};
  }
  return output;
}
function restorePaths(original,normalized){
  const output=new Array(normalized.length);
  for(let index=0;index<normalized.length;index++){
    if(!(index in original)||isHole(normalized[index]))continue;
    const source=original[index],path=normalized[index];
    output[index]={pathId:path.pathId,quantifier:path.quantifier,steps:restoreSteps(source.steps,path.steps)};
  }
  return output;
}
const pathError=error=>{
  const code=String(error).match(/invalid_path_list|invalid_path_shape|invalid_path_id|invalid_path_quantifier|invalid_path_steps|invalid_path_step|invalid_path_direction|invalid_path_types|invalid_path_query_shape|invalid_path_query_values|invalid_path_conditions|invalid_conditions_shape|invalid_conditions_limit|invalid_conditions_rule|invalid_conditions_output/u)?.[0];
  if(code==='invalid_path_list')return ui("Можно добавить до {0} условий пути.",[MAX_PATH_CONDITIONS]);
  if(code==='invalid_path_shape')return ui("Условие пути имеет неподдерживаемую форму.");
  if(code==='invalid_path_id')return ui("Идентификаторы условий пути должны быть уникальными и безопасными.");
  if(code==='invalid_path_quantifier')return ui("Квантификатор пути больше не поддерживается.");
  if(code==='invalid_path_steps')return ui("Путь должен содержать от 1 до {0} шагов.",[MAX_PATH_STEPS]);
  if(code==='invalid_path_step')return ui("Шаг пути имеет неподдерживаемую форму.");
  if(code==='invalid_path_direction')return ui("Направление шага больше не поддерживается.");
  if(code==='invalid_path_types')return ui("Типы шага неполны или превышают допустимый размер.");
  if(code==='invalid_path_query_shape')return ui("Условие шага имеет неподдерживаемую форму.");
  if(code==='invalid_path_query_values')return ui("Условие шага имеет неподдерживаемые параметры.");
  if(code==='invalid_path_conditions')return ui("В одной группе шага можно сохранить до {0} условий.",[MAX_PATH_FILTERS]);
  if(code==='invalid_conditions_limit')return ui("Можно добавить до {0} условий в каждый раздел.",[MAX_PATH_FILTERS]);
  if(code==='invalid_conditions_rule')return ui("Условие неполно или имеет неподдерживаемое значение.");
  return ui("Неверный набор условий.");
};

export function validatePathDraft(value){
  let normalized;
  try{normalized=normalizePathRule(pathListDescriptor(value));}
  catch(error){fail(pathError(error));}
  return restorePaths(Array.isArray(value)?value:[],normalized);
}
export function pathLimits(context){
  try{return observatoryPathLimits(context);}
  catch(error){
    const code=String(error);
    if(code.includes('path_schema_limits'))fail(ui("Схема линз не объявляет границы условий пути."));
    if(code.includes('path_catalog_limits'))fail(ui("Каталог объявляет неподдерживаемые границы условий пути."));
    if(code.includes('path_schema_compat'))fail(ui("Схема линз не объявляет совместимые условия пути."));
    fail(ui("Неверная схема или каталог условий пути."));
  }
}

export function compilePathQuery(value,context){
  return compileValidatedPathQuery(validatePathDraft(value),context);
}

// `compileDraft` already validated this newly normalized path list in the
// same synchronous call. Direct callers enter through `compilePathQuery`.
export function compileValidatedPathQuery(paths,context){
  // An empty path list is the legacy request: it must remain executable even
  // when an older catalog/schema has no path extension yet.
  if(!paths.length)return [];
  let wire;
  try{wire=compilePathRules(pathListDescriptor(paths),context);}
  catch(error){
    const code=String(error);
    if(code.includes('condition_')){
      for(const path of paths){
        if(!path)continue;
        for(const step of path.steps||[]){
          if(!step)continue;
          for(const [kind,query,selected] of [
            ['nodes',step.nodeQuery,step.kinds],['relations',step.relationQuery,step.predicates],
          ]){
            try{compileConditions(query.conditions,context,kind,{reserveTypeSlot:selected.length>0});}
            catch(conditionError){if(conditionError instanceof ContractError)throw conditionError;}
          }
        }
      }
    }
    if(code.includes('path_server_condition_count'))fail(ui("Сервер допускает меньше условий пути."));
    if(code.includes('path_server_quantifier'))fail(ui("Сервер не поддерживает этот квантификатор пути."));
    if(code.includes('path_server_step_count'))fail(ui("Сервер допускает меньше шагов в условии пути."));
    if(code.includes('path_server_direction'))fail(ui("Сервер не поддерживает это направление шага."));
    if(code.includes('path_server_match'))fail(ui("Сервер не поддерживает это сочетание условий шага."));
    if(code.includes('path_server_type_operation'))fail(ui("Сервер не объявляет операцию выбора типов для условий пути."));
    if(code.includes('path_type_catalog_changed'))fail(ui("Словарь типов шага изменился. Обновите каталог."));
    if(code.includes('condition_limit'))fail(ui("Сервер допускает меньше условий в этом разделе."));
    if(code.includes('path_filter_limit'))fail(ui("Условия шага превышают границу схемы."));
    if(code.includes('condition_not_supported'))fail(ui("Одно из условий шага больше не поддерживается."));
    if(code.includes('condition_exists_value'))fail(ui("Для наличия значения выберите «да» или «нет»."));
    if(code.includes('condition_single_value'))fail(ui("Эта операция принимает одно значение."));
    if(code.includes('condition_contains_value'))fail(ui("Для поиска внутри текста нужно одно значение."));
    if(code.includes('condition_value_type'))fail(ui("Проверьте тип значения в условии шага."));
    fail(ui("Условия пути не соответствуют текущей схеме и каталогу."));
  }
  const attachFilters=(filters,conditions)=>{
    const output=new Array(filters.length);
    for(let index=0;index<filters.length;index++){
      const filter=filters[index];
      if(filter===null){output[index]=undefined;continue;}
      if(!Object.hasOwn(filter,'value_index')){output[index]=filter;continue;}
      const {value_index:valueIndex,...rest}=filter;
      output[index]={...rest,value:clone(conditions[valueIndex].value)};
    }
    return output;
  };
  const result=new Array(wire.length);
  for(let pathIndex=0;pathIndex<wire.length;pathIndex++){
    if(!(pathIndex in paths))continue;
    const path=wire[pathIndex];if(path===null)continue;
    const source=paths[pathIndex],steps=new Array(path.steps.length);
    for(let stepIndex=0;stepIndex<path.steps.length;stepIndex++){
      if(!(stepIndex in source.steps))continue;
      const step=path.steps[stepIndex];if(step===null)continue;
      const original=source.steps[stepIndex];
      steps[stepIndex]={direction:step.direction,
        node_query:{...step.node_query,filters:attachFilters(step.node_query.filters,original.nodeQuery.conditions)},
        relation_query:{...step.relation_query,filters:attachFilters(step.relation_query.filters,original.relationQuery.conditions)}};
    }
    result[pathIndex]={path_id:path.path_id,quantifier:path.quantifier,steps};
  }
  return result;
}

const importString=value=>typeof value==='string'?{kind:'string',value:value.slice(0,257)}:{kind:'invalid'};
const importScalar=value=>value===null?{kind:'null'}:typeof value==='boolean'?{kind:'boolean'}:
  typeof value==='string'?{kind:'string',units:value.length}:typeof value==='number'?{kind:'number',finite:Number.isFinite(value)}:{kind:'invalid'};
const importValueShape=value=>Array.isArray(value)?{kind:'array',length:value.length,
  items:value.length<=100?Array.from({length:value.length},(_,index)=>index in value?importScalar(value[index]):{kind:'hole'}):[]}:importScalar(value);
const importTypeValues=value=>Array.isArray(value)?{kind:'array',length:value.length,
  items:value.length<=100?Array.from({length:value.length},(_,index)=>index in value
    ?typeof value[index]==='string'?{kind:'string',value:value[index].slice(0,1025)}:{kind:'invalid'}:{kind:'hole'}):[]}:{kind:'invalid'};
function importFilterDescriptor(raw,kind){
  if(!record(raw))return {kind:'invalid'};
  const typeField=kind==='nodes'?'kind_id':'predicate_id',isTypeCandidate=raw.field===typeField&&raw.op==='in'&&Array.isArray(raw.value);
  return {kind:'object',keys:keys(raw,['field','property_id','op','value']),
    field_present:'field' in raw,field:importString(raw.field),property_id_present:'property_id' in raw,property_id:importString(raw.property_id),
    op:importString(raw.op),value_present:Object.hasOwn(raw,'value'),value_shape:importValueShape(raw.value),
    type_values:isTypeCandidate?importTypeValues(raw.value):{kind:'invalid'}};
}
function importQueryDescriptor(value,kind){
  if(value===undefined)return {kind:'undefined'};
  if(!record(value))return {kind:'invalid'};
  const filters=value.filters;
  return {kind:'object',keys:keys(value,['enabled','match','filters']),enabled:valueDescriptor(value.enabled),match:valueDescriptor(value.match),
    filters_kind:filters===undefined?'undefined':Array.isArray(filters)?'array':'invalid',
    filters_length:Array.isArray(filters)?filters.length:null,
    filters:Array.isArray(filters)&&filters.length<=MAX_PATH_FILTERS
      ?mappedItems(filters,item=>importFilterDescriptor(item,kind)):[]};
}
function importStepDescriptor(value){
  if(!record(value))return {kind:'invalid'};
  return {kind:'object',keys:keys(value,['direction','node_query','relation_query']),direction:valueDescriptor(value.direction),
    node_query:importQueryDescriptor(value.node_query,'nodes'),relation_query:importQueryDescriptor(value.relation_query,'relations')};
}
function importPathDescriptor(value){
  if(!record(value))return {kind:'invalid'};
  return {kind:'object',keys:keys(value,['path_id','quantifier','steps']),path_id:importString(value.path_id),
    quantifier:valueDescriptor(value.quantifier),steps:Array.isArray(value.steps)?{kind:'array',length:value.steps.length,
      items:value.steps.length<=MAX_PATH_STEPS?mappedItems(value.steps,importStepDescriptor):[]}:{kind:'invalid'}};
}
function importPathListDescriptor(value){
  if(value===undefined)return {kind:'undefined'};
  if(!Array.isArray(value))return {kind:'invalid'};
  return {kind:'array',length:value.length,items:value.length<=MAX_PATH_CONDITIONS?mappedItems(value,importPathDescriptor):[]};
}
function importQueryDraft(normalized,source,kind){
  const filters=source?.filters||[];
  const conditions=normalized.conditions.map(condition=>{
    const {value_index:valueIndex,...rule}=condition;
    return {...rule,value:clone(filters[valueIndex].value)};
  });
  const selectedIndex=kind==='nodes'?normalized.kinds_index:normalized.predicates_index;
  const selected=selectedIndex===null?[]:filters[selectedIndex].value;
  return {query:{enabled:normalized.enabled,match:normalized.match,conditions},selected};
}
const importPathError=error=>{
  const code=String(error);
  if(code.includes('invalid_path_spec_list'))return ui("Условия пути превышают границу схемы.");
  if(code.includes('invalid_path_spec_shape'))return ui("Условие пути не соответствует схеме.");
  if(code.includes('invalid_path_spec_step'))return ui("Шаг пути не соответствует схеме.");
  if(code.includes('invalid_path_spec_query_shape'))return ui("Запрос шага не соответствует схеме.");
  if(code.includes('invalid_path_spec_query_values'))return ui("Запрос шага имеет неподдерживаемые параметры.");
  if(code.includes('invalid_path_spec_filter_limit'))return ui("Условия шага превышают границу схемы.");
  if(code.includes('invalid_path_spec_filter'))return ui("Фильтр шага не соответствует схеме.");
  return pathError(error);
};
export function pathDraftFromSpec(value){
  let imported;
  try{imported=importPathDraftRules(importPathListDescriptor(value));}
  catch(error){fail(importPathError(error));}
  const paths=new Array(imported.length);
  for(let pathIndex=0;pathIndex<imported.length;pathIndex++){
    if(!(pathIndex in value)||imported[pathIndex]===null)continue;
    const source=value[pathIndex],path=imported[pathIndex],steps=new Array(path.steps.length);
    for(let stepIndex=0;stepIndex<path.steps.length;stepIndex++){
      if(!(stepIndex in source.steps)||path.steps[stepIndex]===null)continue;
      const sourceStep=source.steps[stepIndex],step=path.steps[stepIndex];
      const node=importQueryDraft(step.nodeQuery,sourceStep.node_query,'nodes');
      const relation=importQueryDraft(step.relationQuery,sourceStep.relation_query,'relations');
      steps[stepIndex]={direction:step.direction,kinds:node.selected,predicates:relation.selected,
        nodeQuery:node.query,relationQuery:relation.query};
    }
    paths[pathIndex]={pathId:path.pathId,quantifier:path.quantifier,steps};
  }
  return validatePathDraft(paths);
}

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node,text);node.className=className;return node;};
const option=(value,text)=>{const node=el('option',text);node.value=value;return node;};
const button=(text,action,className='sc-builder-link')=>{const node=el('button',text,className);node.type='button';node.addEventListener('click',action);return node;};
const field=(text,input)=>{const label=el('label','','sc-builder-field');uiChildren(label,'append',el('span',text),input);return label;};

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
        const name=el('span',item.title),label=el('label','','sc-builder-choice');uiChildren(label,'append',input,name);uiChildren(section,'append',label);
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
    if(draft.paths.length)uiChildren(section,'append',el('h4',ui("Пути к условиям")),el('p',ui("Каждое условие начинается от узлов, найденных основной линзой."),'sc-builder-note'));
    if(legacyNoPath)uiChildren(section,'append',el('p',ui("Проверка пути пока недоступна. Основной запрос работает как прежде."),'sc-builder-warning'));
    draft.paths.forEach((path,pathIndex)=>{
      const box=el('fieldset','','sc-path-condition'),legend=el('legend',ui("Условие пути {0}",[pathIndex+1]));uiChildren(box,'append',legend);
      const quantifier=el('select');for(const value of limits.quantifiers)uiChildren(quantifier,'append',option(value,value==='exists'?ui("Есть такой путь"):ui("Такого пути нет")));quantifier.value=path.quantifier;quantifier.addEventListener('change',()=>{path.quantifier=quantifier.value;onChange();});
      const top=el('div','','sc-path-top');uiChildren(top,'append',field(ui("Проверка"),quantifier),button(ui("Удалить условие"),()=>{draft.paths.splice(pathIndex,1);render();onChange();},'sc-builder-link'));uiChildren(box,'append',top);
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
