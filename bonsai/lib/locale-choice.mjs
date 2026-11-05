// The interface languages and how the site picks one, with nothing else: a page that only greets the
// reader (the choice of a bonsai) reads its language here without loading the catalogs.

// English first: the site opens in English unless the address, the reader's saved choice or
// the system names another language it has.
export const BONSAI_LOCALES=Object.freeze(['en','ru','es','de','zh-Hans']);
export const DEFAULT_LOCALE='en';

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
