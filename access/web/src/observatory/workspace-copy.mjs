import {createResearchWorkspace} from '../research-workspace';
import {validatePlace,PLACES_KEY,RESUME_KEY} from './place-model.mjs';
import {validateHistory,HISTORY_KEY} from './travel-model.mjs';
import {validateInterface,INTERFACE_KEY} from './interface-model.mjs';
import {validateReading,READING_KEY} from './reading-resume.mjs';
import {readSaved,SAVED_LENSES_KEY} from './lens-model.mjs';

export const COPY_SCHEMA='tos_observatory_workspace_v1',COPY_FILE_LIMIT=12000000;
const bad=()=>{throw new Error('Файл не является полной копией исследования или содержит повреждённые данные.');};
export function validateWorkspaceCopy(input){
  const text=typeof input==='string'?input:JSON.stringify(input);if(typeof text!=='string'||text.length>4000000)bad();
  const value=JSON.parse(text);
  if(value?.schema!==COPY_SCHEMA||value.v!==1||typeof value.exportedAt!=='string'||!Number.isFinite(Date.parse(value.exportedAt))
    ||!Array.isArray(value.places)||value.places.length>12||JSON.stringify(value.places).length>800000)bad();
  const places=value.places.map(validatePlace);if(new Set(places.map(place=>place.id)).size!==places.length)bad();
  if(!Array.isArray(value.lenses))bad();const lenses=readSaved({getItem:()=>JSON.stringify(value.lenses)});
  if(new Set(lenses.map(lens=>lens.name)).size!==lenses.length)bad();
  const research=createResearchWorkspace({persistence:false});research.importPacket(JSON.stringify(value.research));
  return {schema:COPY_SCHEMA,v:1,exportedAt:new Date(value.exportedAt).toISOString(),history:validateHistory(value.history),places,lenses,
    resume:value.resume===null?null:validatePlace(value.resume),preferences:validateInterface(value.preferences),reading:validateReading(value.reading),research:JSON.parse(research.exportPacket())};
}
export function copyStorageKeys(pathname){return [HISTORY_KEY+':'+pathname,PLACES_KEY,RESUME_KEY,INTERFACE_KEY,READING_KEY+':'+pathname,'tos-research-workspace-v1',SAVED_LENSES_KEY];}
export function snapshotCopyStorage(storage,pathname){
  if(!storage)throw new Error('Локальное хранилище недоступно. Скопируйте исследование в файл.');
  return new Map(copyStorageKeys(pathname).map(key=>[key,storage.getItem(key)]));
}
export function commitWorkspaceCopy(storage,pathname,input,before){
  const copy=validateWorkspaceCopy(input),keys=copyStorageKeys(pathname);
  if(keys.some(key=>!before.has(key)||storage.getItem(key)!==before.get(key)))
    throw new Error('Исследование изменилось после проверки файла. Выберите файл заново, чтобы сравнить с текущими данными.');
  const values=[copy.history,copy.places,copy.resume,copy.preferences,copy.reading,copy.research,copy.lenses],written=[];
  try{
    keys.forEach((key,index)=>{const text=values[index]===null?null:JSON.stringify(values[index]);
      if(text===null)storage.removeItem(key);else storage.setItem(key,text);written.push(key);});
  }catch(cause){
    let restored=true;
    for(const key of written.reverse())try{const original=before.get(key);if(original===null)storage.removeItem(key);else storage.setItem(key,original);}catch{restored=false;}
    throw new Error(restored?'Браузер не смог сохранить копию. Прежнее исследование восстановлено.':
      'Браузер не смог восстановить все прежние записи. Скачайте прежнюю копию перед закрытием страницы.',{cause});
  }
  return copy;
}
