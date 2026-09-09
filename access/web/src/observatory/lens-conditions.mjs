import {t} from './ui-i18n.mjs';
import {ContractError,localized} from './knowledge-client.mjs';

export const MAX_CONDITIONS=12;
const fail=message=>{throw new ContractError(message);};
const scalar=value=>value===null||typeof value==='boolean'||typeof value==='string'&&value.length<=1024||typeof value==='number'&&Number.isFinite(value);
const id=value=>typeof value==='string'&&value.length>0&&value.length<=256;
const operatorSources={eq:"равно",neq:"не равно",in:"одно из",contains:"содержит",prefix:"начинается с",exists:"значение указано",gt:"больше",gte:"не меньше",lt:"меньше",lte:"не больше"};
export const operatorLabels=Object.defineProperties({},Object.fromEntries(Object.entries(operatorSources).map(([key,source])=>[key,{enumerable:true,get:()=>t(source)}])));
const valueContracts={eq:'scalar',neq:'scalar',in:'scalar-or-scalar-array',contains:'scalar-or-scalar-array',prefix:'string',exists:'boolean',gt:'number',gte:'number',lt:'number',lte:'number'};
// Presentation of named access fields. Semantic property definitions always
// come from the snapshot catalog; their binding is never translated to a path.
const fields={
  kind_id:["Тип узла",'string'],type_id:["Тип сущности",'string'],predicate_id:["Тип связи",'string'],relation_type_id:["Тип отношения",'string'],
  'display.title.default':["Название",'string'],'display.title.ru':["Название · русский",'string'],'display.title.en':["Название · английский",'string'],
  'display.summary.default':["Описание",'string'],'display.summary_state':["Наличие описания",'string'],
  'display.label.default':["Название связи",'string'],'display.statement.default':["Формулировка связи",'string'],'display.explanation_state':["Наличие объяснения",'string'],
  'epistemic.authority_layer':["Слой знания",'string'],'epistemic.review_posture':["Статус проверки",'string'],'epistemic.canon_status':["Статус канона",'string'],
  graph_layers:["Слои графа",'string-array'],view_ids:["Представления",'string-array'],source_refs:["Ссылки на источники",'string-array'],source_dossier_ref:["Досье источника",'string'],
};
const typeOps={string:['eq','neq','in','contains','prefix','exists'],'string-array':['eq','neq','in','contains','exists'],number:['eq','neq','in','gt','gte','lt','lte','exists'],boolean:['eq','neq','exists']};
export function validateConditions(value){
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['nodes','relations'].includes(k)))fail(t("Неверный набор условий."));
  const result={};
  for(const kind of ['nodes','relations']){
    const entries=value[kind];
    if(!Array.isArray(entries)||entries.length>MAX_CONDITIONS)fail(t("Можно добавить до 12 условий в каждый раздел."));
    result[kind]=entries.map(rule=>{
      if(!rule||!['field','property_id'].includes(rule.selector)||!id(rule.id)||!Object.hasOwn(operatorLabels,rule.op)
        ||!(Array.isArray(rule.value)?rule.value.length<=100&&rule.value.every(scalar):scalar(rule.value))
        ||kind==='relations'&&rule.selector==='property_id')fail(t("Условие неполно или имеет неподдерживаемое значение."));
      return {selector:rule.selector,id:rule.id,op:rule.op,value:structuredClone(rule.value)};
    });
  }
  return result;
}
const fieldEnum=(schema,kind)=>(schema.$defs?.[kind+'Field']?.anyOf||[]).flatMap(part=>part.enum||[]);
export function conditionCatalog({catalog,schema},kind){
  const caps=catalog.capabilities,wireKind=kind==='nodes'?'node':'relation';
  const allowed=fieldEnum(schema,wireKind),filter=schema.$defs?.[wireKind+'Filter'];
  const operators=(list,type)=>(typeOps[type]||[]).filter(op=>list?.includes(op)&&caps.filter_operators?.includes(op)
    &&filter?.properties?.op?.enum?.includes(op)&&caps.operator_value_contracts?.[op]===valueContracts[op]);
  const entries=[];
  for(const field of caps[wireKind+'_fields']||[]){
    if(!Object.hasOwn(fields,field)||!allowed.includes(field))continue;
    const [title,valueType]=fields[field],ops=operators(Object.keys(operatorLabels),valueType);
    if(ops.length)entries.push({selector:'field',id:field,title:t(title),valueType,operators:ops,
      suggestions:(caps.facets?.[kind]?.[field]||[]).map(entry=>entry.value).filter(scalar),definition:'',appliesTo:[]});
  }
  const propertyContract=caps.property_filters;
  if(kind==='nodes'&&propertyContract?.selector==='property_id'&&propertyContract.scope==='node-query-and-path-node-query'
    &&propertyContract.binding==='same-graph-snapshot'&&propertyContract.operators==='declared-per-property'
    &&filter?.properties?.property_id&&filter.oneOf?.some(part=>part.required?.includes('property_id'))){
    const properties=catalog.semantic_registries?.properties;
    if(Array.isArray(properties)&&properties.length<=5000){
      const counts=new Map();for(const p of properties)counts.set(p?.property_id,(counts.get(p?.property_id)||0)+1);
      for(const p of properties){
        if(!/^tos\.property\.[a-z0-9-]+$/.test(p?.property_id||'')||counts.get(p.property_id)!==1
          ||!Object.hasOwn(typeOps,p.value_type)||!Array.isArray(p.applies_to)||!p.applies_to.every(id))continue;
        const ops=operators(p.operators,p.value_type);if(!ops.length)continue;
        entries.push({selector:'property_id',id:p.property_id,title:localized(p.labels,p.property_id),valueType:p.value_type,operators:ops,
          definition:typeof p.definition==='string'?p.definition:'',unit:p.unit,language:p.language,
          appliesTo:p.applies_to,inherited:p.inherited===true,suggestions:[]});
      }
    }
  }
  return entries;
}
export const conditionKey=rule=>rule.selector+':'+rule.id;
export function defaultCondition(entry){return {selector:entry.selector,id:entry.id,op:entry.operators[0],value:entry.valueType==='boolean'||entry.operators[0]==='exists'?true:''};}
export function compileConditions(rules,context,kind){
  const entries=conditionCatalog(context,kind),wireKind=kind==='nodes'?'node':'relation';
  const limit=context.schema.$defs?.[wireKind+'Query']?.properties?.filters?.maxItems;
  // Reserve one slot for the existing type selector.
  if(rules.length&&(!Number.isInteger(limit)||rules.length+1>limit))fail(t("Сервер допускает меньше условий в этом разделе."));
  return rules.map(rule=>{
    const entry=entries.find(item=>conditionKey(item)===conditionKey(rule));
    if(!entry||!entry.operators.includes(rule.op))fail(t("Условие больше не поддерживается: {0}. Измените или удалите его.", [(entry?.title||rule.id)]));
    const values=Array.isArray(rule.value)?rule.value:[rule.value],type=entry.valueType==='string-array'?'string':entry.valueType;
    if(rule.op==='exists'){
      if(typeof rule.value!=='boolean')fail(t("Для наличия значения выберите «да» или «нет»."));
    }else if(!values.length||values.some(value=>typeof value!==type||typeof value==='number'&&!Number.isFinite(value)))fail(t("Проверьте тип значения: {0}.", [entry.title]));
    if(['eq','neq','prefix','gt','gte','lt','lte'].includes(rule.op)&&Array.isArray(rule.value))fail(t("Эта операция принимает одно значение."));
    if(rule.op==='contains'&&Array.isArray(rule.value)&&entry.valueType!=='string-array')fail(t("Для поиска внутри текста нужно одно значение."));
    return {[rule.selector]:rule.id,op:rule.op,value:structuredClone(rule.value)};
  });
}
export function conditionText(rule,entries){
  const entry=entries.find(item=>conditionKey(item)===conditionKey(rule));
  const value=rule.op==='exists'?(rule.value?t("да"):t("нет")):Array.isArray(rule.value)?rule.value.map(v=>JSON.stringify(v)).join(', '):JSON.stringify(rule.value);
  return `${entry?.title||rule.id} ${operatorLabels[rule.op]||rule.op} ${value}`;
}
