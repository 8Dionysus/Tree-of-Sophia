// Independent pre-migration oracle for matched-binding parity checks.
// Serving code does not import this validator.
import {t} from './ui-i18n.mjs';
import {ContractError} from './knowledge-client.mjs';
import {MAX_CONDITIONS,operatorLabels} from './lens-conditions.mjs';
const scalar=value=>value===null||typeof value==='boolean'||typeof value==='string'&&value.length<=1024||typeof value==='number'&&Number.isFinite(value);
const id=value=>typeof value==='string'&&value.length>0&&value.length<=256;
const fail=message=>{throw new ContractError(message);};
export function validateConditionsLegacyOracle(value,{maxConditions=MAX_CONDITIONS}={}){
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['nodes','relations'].includes(k)))fail(t("Неверный набор условий."));
  const result={};
  for(const kind of ['nodes','relations']){
    const entries=value[kind];
    if(!Number.isInteger(maxConditions)||maxConditions<0||!Array.isArray(entries)||entries.length>maxConditions)
      fail(t("Можно добавить до {0} условий в каждый раздел.",[maxConditions]));
    result[kind]=entries.map(rule=>{
      if(!rule||!['field','property_id'].includes(rule.selector)||!id(rule.id)||!Object.hasOwn(operatorLabels,rule.op)
        ||!(Array.isArray(rule.value)?rule.value.length<=100&&rule.value.every(scalar):scalar(rule.value))
        ||kind==='relations'&&rule.selector==='property_id')fail(t("Условие неполно или имеет неподдерживаемое значение."));
      return {selector:rule.selector,id:rule.id,op:rule.op,value:structuredClone(rule.value)};
    });
  }
  return result;
}
