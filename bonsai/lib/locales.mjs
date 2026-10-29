import es from '../locales/es.json' with {type:'json'};
import de from '../locales/de.json' with {type:'json'};
import zh from '../locales/zh-Hans.json' with {type:'json'};

export const BONSAI_LOCALES=Object.freeze(['ru','en','es','de','zh-Hans']);
const catalogs=Object.freeze({es,de,'zh-Hans':zh});

/** Interface language only. Source edition languages and identities stay literal. */
export function resolveLocale(value,fallback='ru'){
  const tag=String(typeof value==='function'?value():value??'').trim().replaceAll('_','-').toLowerCase();
  if(tag==='zh'||tag==='zh-hans'||/^zh-(?:cn|sg)(?:-|$)/.test(tag)||tag.startsWith('zh-hans-'))return 'zh-Hans';
  const base=tag.split('-')[0];
  return ['ru','en','es','de'].includes(base)?base:fallback;
}

export const copyLocale=value=>resolveLocale(value)==='ru'?'ru':'en';
export const bonsaiTranslation=(source,locale)=>{const catalog=catalogs[resolveLocale(locale)];return catalog&&Object.hasOwn(catalog,source)?catalog[source]:undefined;};
export function bonsaiText(source,locale,values=[]){
  const template=bonsaiTranslation(source,locale)??source;
  return template.replace(/\{(\d+)\}/g,(match,index)=>index<values.length?String(values[index]):match);
}

/** Select an authored display form; a plain source/user string is never translated. */
export function bonsaiValue(value,locale){
  if(typeof value==='string')return value;
  if(!value||typeof value!=='object')return '';
  const language=resolveLocale(locale);
  return value[language]??(value.en?bonsaiText(value.en,language):value.ru)??'';
}
