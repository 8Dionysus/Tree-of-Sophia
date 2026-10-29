import es from './locales/es.json' with {type:'json'};
import de from './locales/de.json' with {type:'json'};
import zh from './locales/zh-Hans.json' with {type:'json'};
import {BONSAI_LOCALES,resolveLocale,copyLocale,bonsaiTranslation} from '../../../../bonsai/lib/locales.mjs';

export {resolveLocale,copyLocale};
export const PUBLIC_LOCALES=BONSAI_LOCALES;
export const LOCALE_NAMES=Object.freeze({ru:'Русский',en:'English',es:'Español',de:'Deutsch','zh-Hans':'简体中文'});
const catalogs=Object.freeze({es,de,'zh-Hans':zh});
const translation=(source,language)=>{const catalog=catalogs[language];return catalog&&Object.hasOwn(catalog,source)?catalog[source]:bonsaiTranslation(source,language);};

/** Only call for authored presentation messages, never raw user/provider text. */
export function localeText(source,locale,values=[],context=null){
  const language=resolveLocale(locale);
  let template=(context?translation(`${context}:${source}`,language):undefined)??translation(source,language);
  // Bibliographic display lines are composed from authored forms. Translate
  // their components after identity construction, retaining the exact locator.
  if(template===undefined&&language!=='ru'&&language!=='en'&&source.includes(' · ')){
    template=source.split(' · ').map(part=>{
      const direct=translation(part,language);if(direct!==undefined)return direct;
      const section=/^(Book|Chapter|Section) (\d+)$/.exec(part);
      return section?localeText(`${section[1]} {0}`,language,[section[2]]):part;
    }).join(' · ');
  }
  template??=source;
  return template.replace(/\{(\d+)\}/g,(match,index)=>index<values.length?String(values[index]):match);
}

/** Localize explicit authored forms without changing their stored representation. */
export function localeValue(value,locale){
  if(typeof value==='string')return value;
  if(!value||typeof value!=='object')return '';
  const language=resolveLocale(locale);
  return value[language]??(value.en?localeText(value.en,language):value.ru)??'';
}

/** A dictionary of static UI labels. Do not pass workspace/source packets here. */
export function localeLabels(record,locale){
  if(typeof record==='string')return localeText(record,locale);
  if(Array.isArray(record))return record.map(value=>localeLabels(value,locale));
  if(record&&typeof record==='object')return Object.fromEntries(Object.entries(record).map(([key,value])=>[key,localeLabels(value,locale)]));
  return record;
}
