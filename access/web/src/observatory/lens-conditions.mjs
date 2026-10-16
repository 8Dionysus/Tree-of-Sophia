import {t} from './ui-i18n.mjs';
import {ContractError,localized} from './knowledge-client.mjs';
import {conditionCatalogRules,compileConditionRules,normalizeConditions} from './lens-conditions-rust.mjs';

export const MAX_CONDITIONS=12;
const fail=message=>{throw new ContractError(message);};
const scalar=value=>value===null||typeof value==='boolean'||typeof value==='string'&&value.length<=1024||typeof value==='number'&&Number.isFinite(value);
const operatorSources={eq:"равно",neq:"не равно",in:"одно из",contains:"содержит",prefix:"начинается с",exists:"значение указано",gt:"больше",gte:"не меньше",lt:"меньше",lte:"не больше"};
export const operatorLabels=Object.defineProperties({},Object.fromEntries(Object.entries(operatorSources).map(([key,source])=>[key,{enumerable:true,get:()=>t(source)}])));
// Presentation of named access fields. Semantic property definitions always
// come from the snapshot catalog; their binding is never translated to a path.
const fields={
  kind_id:"Тип узла",type_id:"Тип сущности",predicate_id:"Тип связи",relation_type_id:"Тип отношения",
  'display.title.default':"Название",'display.title.ru':"Название · русский",'display.title.en':"Название · английский",
  'display.summary.default':"Описание",'display.summary_state':"Наличие описания",
  'display.label.default':"Название связи",'display.statement.default':"Формулировка связи",'display.explanation_state':"Наличие объяснения",
  'epistemic.authority_layer':"Слой знания",'epistemic.review_posture':"Статус проверки",'epistemic.canon_status':"Статус канона",
  graph_layers:"Слои графа",view_ids:"Представления",source_refs:"Ссылки на источники",source_dossier_ref:"Досье источника",
};
export function validateConditions(value,{maxConditions=MAX_CONDITIONS}={}){
  try{return normalizeConditions(value,maxConditions);}
  catch(error){const code=String(error);
    if(code.includes('invalid_conditions_limit'))fail(t("Можно добавить до {0} условий в каждый раздел.",[maxConditions]));
    if(code.includes('invalid_conditions_rule'))fail(t("Условие неполно или имеет неподдерживаемое значение."));
    fail(t("Неверный набор условий."));}
}
export function conditionCatalog({catalog,schema},kind){
  const caps=catalog?.capabilities,wireEntries=conditionCatalogRules({catalog,schema},kind);
  return wireEntries.map(entry=>{
    const valueType=entry.value_type,operators=entry.operators;
    if(entry.selector==='field')return {selector:'field',id:entry.id,title:t(fields[entry.id]||entry.id),valueType,operators,
      suggestions:(caps?.facets?.[kind]?.[entry.id]||[]).map(value=>value?.value).filter(scalar),definition:'',appliesTo:[]};
    const property=catalog?.semantic_registries?.properties?.find(item=>item?.property_id===entry.id)||{};
    return {selector:'property_id',id:entry.id,title:localized(property.labels,entry.id),valueType,operators,
      definition:typeof property.definition==='string'?property.definition:'',unit:property.unit,language:property.language,
      appliesTo:property.applies_to,inherited:property.inherited===true,suggestions:[]};
  });
}
export const conditionKey=rule=>rule.selector+':'+rule.id;
export function defaultCondition(entry){return {selector:entry.selector,id:entry.id,op:entry.operators[0],value:entry.valueType==='boolean'||entry.operators[0]==='exists'?true:''};}
export function compileConditions(rules,context,kind,{reserveTypeSlot=true}={}){
  let compiled;
  try{compiled=compileConditionRules(rules,context,kind,{reserveTypeSlot});}
  catch(error){
    const code=String(error);
    if(code.includes('condition_limit'))fail(t("Сервер допускает меньше условий в этом разделе."));
    if(code.includes('condition_not_supported')){
      const entries=conditionCatalog(context,kind),rule=rules.find((item,index)=>{
        if(!(index in rules))return false;
        const entry=entries.find(candidate=>conditionKey(candidate)===conditionKey(item));
        return !entry||!entry.operators.includes(item.op);
      });
      const entry=rule&&entries.find(candidate=>conditionKey(candidate)===conditionKey(rule));
      fail(t("Условие больше не поддерживается: {0}. Измените или удалите его.", [entry?.title||rule?.id||'']));
    }
    if(code.includes('condition_exists_value'))fail(t("Для наличия значения выберите «да» или «нет»."));
    if(code.includes('condition_single_value'))fail(t("Эта операция принимает одно значение."));
    if(code.includes('condition_contains_value'))fail(t("Для поиска внутри текста нужно одно значение."));
    if(code.includes('condition_value_type'))fail(t("Проверьте тип значения."));
    fail(t("Условие неполно или имеет неподдерживаемое значение."));
  }
  const output=new Array(compiled.length);
  for(let index=0;index<compiled.length;index++){
    if(!(index in rules))continue;
    const filter=compiled[index];
    if(filter===null){output[index]=undefined;continue;}
    const {value_index:valueIndex,...wire}=filter;
    output[index]={...wire,value:structuredClone(rules[valueIndex].value)};
  }
  return output;
}
