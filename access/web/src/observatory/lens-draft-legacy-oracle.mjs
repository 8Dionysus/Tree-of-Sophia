// Independent saved-root oracle. Paths retain their existing owner; callers
// comparing roots before WASM installation use empty/absent paths.
import {t} from './ui-i18n.mjs';
import {BUDGET,ContractError} from './knowledge-client.mjs';
import {validateConditionsLegacyOracle} from './lens-conditions-legacy-oracle.mjs';
import {validatePathDraft} from './lens-path-editor.mjs';
const bad=message=>{throw new ContractError(message);};
const strings=(value,max,length=1024)=>Array.isArray(value)&&value.length<=max&&new Set(value).size===value.length&&value.every(v=>typeof v==='string'&&v.length>0&&v.length<=length);
export function validateDraftLegacyOracle(value){
  if(!value||![1,2].includes(value.v)||typeof value.name!=='string'||!value.name.trim()||value.name.length>64
    ||!['area','focus','all'].includes(value.scope)||!strings(value.sources,7)||!value.sources.length
    ||!strings(value.nodeIds,BUDGET.nodes)||!strings(value.kinds,100)||!strings(value.predicates,100)
    ||typeof value.query!=='string'||value.query.length>256
    ||!(value.focusId===null||typeof value.focusId==='string'&&value.focusId.length>0&&value.focusId.length<=1024)
    ||!Number.isInteger(value.depth)||value.depth<0||value.depth>3
    ||!['either','outgoing','incoming'].includes(value.direction)||!['overview','all'].includes(value.profile)
    ||!Number.isInteger(value.limit)||value.limit<1||value.limit>BUDGET.nodes||typeof value.relations!=='boolean')bad(t("Настройки линзы неполны или превышают допустимый размер."));
  if(value.scope==='area'&&!value.nodeIds.length)bad(t("Исходная область пуста. Выберите поиск по древу."));
  if(value.scope==='focus'&&!value.focusId)bad(t("Сначала выберите звезду."));
  if(value.v===1&&(value.conditions!==undefined||value.paths!==undefined&&(!Array.isArray(value.paths)||value.paths.length)))bad(t("Версия сохранённой линзы не соответствует её условиям."));
  const conditions=validateConditionsLegacyOracle(value.v===1?{nodes:[],relations:[]}:value.conditions);
  const paths=validatePathDraft(value.v===1?[]:value.paths);
  const result={v:2,name:value.name.trim(),scope:value.scope,sources:[...value.sources],nodeIds:[...value.nodeIds],focusId:value.focusId,
    query:value.query,kinds:[...value.kinds],predicates:[...value.predicates],depth:value.depth,direction:value.direction,
    profile:value.profile,limit:value.limit,relations:value.relations,conditions};
  if(Object.hasOwn(value,'paths'))result.paths=paths;
  return result;
}
