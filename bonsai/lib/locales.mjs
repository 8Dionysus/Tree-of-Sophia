import {BONSAI_LOCALES,DEFAULT_LOCALE,resolveLocale,copyLocale,initialLocale} from './locale-choice.mjs';
export {BONSAI_LOCALES,DEFAULT_LOCALE,resolveLocale,copyLocale,initialLocale};
// A language with a catalog of its own comes when a reader first chooses it (loadBonsaiCatalog), so a page carries the
// words of its reader's language alone. English and Russian are the bonsais' own languages and have no catalog.
const sources={
  es:()=>import('../locales/es.json',{with:{type:'json'}}),
  de:()=>import('../locales/de.json',{with:{type:'json'}}),
  'zh-Hans':()=>import('../locales/zh-Hans.json',{with:{type:'json'}}),
};
const catalogs={},loading={};
export function loadBonsaiCatalog(locale){
  const language=resolveLocale(locale);
  if(catalogs[language]||!sources[language])return Promise.resolve();
  return loading[language]??=sources[language]().then(module=>{catalogs[language]=Object.freeze(module.default);},error=>{delete loading[language];throw error;});
}

/**
 * The one translation a reader of `locale` sees: one in a language the interface itself speaks to
 * them in, the interface language or else the language of its copy (copyLocale: English for the
 * languages without their own copy). A Russian reader is never given English, nor an English reader
 * Russian; where neither exists the reader has the original alone.
 */
export function translationLanguage(locale,available=[]){
  const offered=[].concat(available??[]);
  return [resolveLocale(locale),copyLocale(locale)].find(language=>offered.includes(language))??null;
}

/**
 * Every language a reader of `locale` may see of a text written in `original`: the original and, unless
 * the interface already reads the original, that one translation. Cards, term origins, the parallel
 * reader, its editions and proposed correspondences all choose their languages here.
 */
export function shownLanguages(locale,original,available=[]){
  if(resolveLocale(locale)===original)return [original];
  const translation=translationLanguage(locale,[].concat(available??[]).filter(language=>language!==original));
  return translation?[original,translation]:[original];
}
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
