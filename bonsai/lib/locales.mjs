import es from '../locales/es.json' with {type:'json'};
import de from '../locales/de.json' with {type:'json'};
import zh from '../locales/zh-Hans.json' with {type:'json'};

// English first: the site opens in English unless the address, the reader's saved choice or
// the system names another language it has.
export const BONSAI_LOCALES=Object.freeze(['en','ru','es','de','zh-Hans']);
export const DEFAULT_LOCALE='en';
const catalogs=Object.freeze({es,de,'zh-Hans':zh});

/** Interface language only. Source edition languages and identities stay literal. */
export function resolveLocale(value,fallback=DEFAULT_LOCALE){
  const tag=String(typeof value==='function'?value():value??'').trim().replaceAll('_','-').toLowerCase();
  if(tag==='zh'||tag==='zh-hans'||/^zh-(?:cn|sg)(?:-|$)/.test(tag)||tag.startsWith('zh-hans-'))return 'zh-Hans';
  const base=tag.split('-')[0];
  return ['ru','en','es','de'].includes(base)?base:fallback;
}

export const copyLocale=value=>resolveLocale(value)==='ru'?'ru':'en';

/** The interface language at start: the address's, the reader's saved choice, the first system language the site has, else English. */
export function initialLocale({requested=null,saved=null,system=[]}={}){
  return resolveLocale(requested,null)??resolveLocale(saved,null)??[].concat(system??[]).map(value=>resolveLocale(value,null)).find(Boolean)??DEFAULT_LOCALE;
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
