import {t} from './ui-i18n.mjs';
import {readingKey} from './reader-model.mjs';
import {contentLanguage,validFormIdentity} from './human-forms.mjs';
import {persistentReadingAnchorKey} from './reading-anchor.mjs';

export const READING_KEY='tos-observatory-reading-v1';
export const emptyReading=()=>({v:1,activeKey:null,entries:[]});
const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const language=value=>value==='default'||contentLanguage(value);
const bounded=(n,min,max)=>Number.isFinite(n)&&n>=min&&n<=max;
const bad=()=>{throw new Error(t("Сохранённое чтение повреждено. Исходная запись оставлена в браузере."));};
export function validateReading(value){
  if(value?.v!==1||!Array.isArray(value.entries)||value.entries.length>2)bad();
  const entries=value.entries.map(entry=>{
    if(!['node','relation'].includes(entry.kind)||typeof entry.id!=='string'||!entry.id||entry.id.length>1024
      ||!hash(entry.sourceRevision)||!hash(entry.contentRevision)||!language(entry.preferred)||!Array.isArray(entry.positions)||entry.positions.length>8)bad();
    const key=readingKey(entry.kind,entry.id);
    const positions=entry.positions.map(([positionKey,position])=>{
      if(typeof positionKey!=='string'||positionKey.length>32768)bad();
      let parts;try{parts=JSON.parse(positionKey);}catch{bad();}
      if(!Array.isArray(parts)||![4,5].includes(parts.length)||(parts.length===5&&!validFormIdentity(parts[4],parts[3]))||parts[0]!==key||parts[1]!==entry.sourceRevision||parts[2]!==entry.contentRevision||!language(parts[3])
        ||!bounded(position?.top,0,10000000)||!Array.isArray(position.details)||position.details.length>2)bad();
      const details=position.details.map(([id,open])=>{if(!['sources','identity'].includes(id)||typeof open!=='boolean')bad();return [id,open];});
      if(new Set(details.map(d=>d[0])).size!==details.length)bad();
      const anchor=position.anchor;
      if(anchor!==null&&(!anchor||!persistentReadingAnchorKey(anchor.key)||!bounded(anchor.offset,-10000000,10000000)))bad();
      return [positionKey,{top:position.top,details,anchor:anchor?{key:anchor.key,offset:anchor.offset}:null}];
    });
    if(new Set(positions.map(p=>p[0])).size!==positions.length)bad();
    return {kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,contentRevision:entry.contentRevision,preferred:entry.preferred,positions};
  });
  const keys=entries.map(entry=>readingKey(entry.kind,entry.id));
  if(new Set(keys).size!==keys.length||(entries.length?!keys.includes(value.activeKey):value.activeKey!==null))bad();
  const result={v:1,activeKey:value.activeKey,entries};if(JSON.stringify(result).length>200000)bad();return result;
}
export function readReading(storage,key=READING_KEY){
  const text=storage?.getItem(key);if(!text)return emptyReading();if(text.length>200000)bad();return validateReading(JSON.parse(text));
}
