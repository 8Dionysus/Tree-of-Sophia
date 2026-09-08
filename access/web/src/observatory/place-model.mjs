import {BUDGET,specForPacket,validateLens,validateExploration} from './knowledge-client.mjs';
import {draftForPacket,validateDraft,constructorCatalog,previewDraft} from './lens-model.mjs';
import {validatePose} from './view-state.mjs';
export const PLACES_KEY='tos-observatory-places-v1',RESUME_KEY='tos-observatory-resume-v1';
const bad=()=>{throw new Error('Сохранённое место не удалось прочитать. Запись осталась в браузере.');};
function spec(value){
  if(!value||JSON.stringify(value).length>24000||value.schema_version!=='tos_lens_spec_v1'||!Number.isInteger(value.limits?.nodes)||value.limits.nodes<1||value.limits.nodes>BUDGET.nodes
    ||!Number.isInteger(value.limits?.relations)||value.limits.relations<0||value.limits.relations>BUDGET.relations||!Number.isInteger(value.limits?.groups)||value.limits.groups<0||value.limits.groups>8
    ||(value.traversal?.depth!==undefined&&(!Number.isInteger(value.traversal.depth)||value.traversal.depth<0||value.traversal.depth>3)))bad();
  const allowed=['schema_version','lens_id','title','language','detail','explain','sources','seed','node_query','relation_query','traversal','composition','limits'];
  return Object.fromEntries(allowed.filter(key=>value[key]!==undefined).map(key=>[key,structuredClone(value[key])]));
}
export function validatePlace(value){
  if(value?.v!==1||typeof value.name!=='string'||!value.name.trim()||value.name.length>64||typeof value.id!=='string'||!value.id||value.id.length>80
    ||!Number.isFinite(new Date(value.savedAt).getTime())||typeof value.savedAt!=='number'||!/^[a-f0-9]{64}$/.test(value.sourceRevision||'')||typeof value.route!=='string'||value.route.length>50000)bad();
  const valid={v:1,id:value.id,name:value.name.trim(),savedAt:value.savedAt,sourceRevision:value.sourceRevision,route:value.route,spec:spec(value.spec),draft:value.draft?validateDraft(value.draft):null,pose:validatePose(value.pose)};
  if(JSON.stringify(valid).length>65000)bad();return valid;
}
export function capturePlace(packet,pose,{name,id,route,savedAt=Date.now()}){
  if(packet.schema==='tos_exploration_result_v1')validateExploration(packet);else validateLens(packet);
  // A saved exploration page is a view of its visible IDs, never a durable cursor.
  const query=specForPacket(packet)||{schema_version:'tos_lens_spec_v1',lens_id:'observatory-saved-area',language:'ru',detail:'compact',explain:true,
    seed:{node_ids:packet.nodes.map(n=>n.id)},node_query:{enabled:true},
    relation_query:{enabled:Boolean(packet.relations.length),filters:packet.relations.length?[{field:'id',op:'in',value:packet.relations.map(r=>r.id)}]:[]},
    traversal:{depth:0,profile:'all'},limits:{...BUDGET,groups:8}};
  return validatePlace({v:1,id,name,savedAt,route,sourceRevision:packet.source_revision,spec:query,draft:draftForPacket(packet),pose});
}
export function readPlaces(storage){const text=storage.getItem(PLACES_KEY);if(!text)return [];if(text.length>800000)bad();const values=JSON.parse(text);if(!Array.isArray(values)||values.length>12)bad();const entries=values.map(validatePlace);if(new Set(entries.map(e=>e.id)).size!==entries.length)bad();return entries;}
export function savePlace(storage,place){const value=validatePlace(place),entries=readPlaces(storage),index=entries.findIndex(e=>e.id===value.id);if(index<0){if(entries.length>=12)throw new Error('Сохранено 12 мест. Удалите ненужное место, чтобы добавить новое.');entries.unshift(value);}else entries[index]=value;storage.setItem(PLACES_KEY,JSON.stringify(entries));return entries;}
export function pinHistoryPlace(storage,entry){
  const place=validatePlace(entry),id=place.id;
  const existing=readPlaces(storage).find(value=>value.id===id);
  if(existing)return existing;
  const pinned=validatePlace({...place,id,savedAt:Date.now()});savePlace(storage,pinned);return pinned;
}
export function readResume(storage,route){const text=storage.getItem(RESUME_KEY);if(!text)return null;if(text.length>65000)bad();const value=validatePlace(JSON.parse(text));return !route||route===value.route?value:null;}
export async function reopenPlace(client,value,signal){
  const place=validatePlace(value);
  const packet=place.draft?await previewDraft(client,place.draft,await constructorCatalog(client,signal),signal):await client.compile({...place.spec,explain:true},signal);
  if(!packet.nodes.length)throw new Error('В этом месте больше нет доступных звёзд. Предыдущий вид сохранён.');
  return {packet,pose:place.pose,changed:packet.source_revision!==place.sourceRevision};
}
