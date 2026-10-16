import {t} from './ui-i18n.mjs';
import {normalizeReadingResume} from './reading-resume-rust.mjs';

export const READING_KEY='tos-observatory-reading-v1';
export const emptyReading=()=>({v:1,activeKey:null,entries:[]});
const bad=()=>{throw new Error(t("Сохранённое чтение повреждено. Исходная запись оставлена в браузере."));};
export function validateReading(value){
  try{return normalizeReadingResume(value);}catch(error){if(error?.code==='rules-unavailable')throw error;bad();}
}
export function readReading(storage,key=READING_KEY){
  const text=storage?.getItem(key);if(!text)return emptyReading();if(text.length>200000)bad();return validateReading(JSON.parse(text));
}
