// How the site picks one of its interface languages, with nothing else: a page that only greets the reader
// (the choice of a bonsai) reads its language here without loading the catalogs. The languages and the
// reading of a tag are the ones the Worker shares (locale-tag.mjs).
import {BONSAI_LOCALES,DEFAULT_LOCALE,resolveLocale} from './locale-tag.mjs';

export {BONSAI_LOCALES,DEFAULT_LOCALE,resolveLocale};

export const copyLocale=value=>resolveLocale(value)==='ru'?'ru':'en';

/** The interface language at start: the address's, the reader's saved choice, the first system language the site has, else English. */
export function initialLocale({requested=null,saved=null,system=[]}={}){
  return resolveLocale(requested,null)??resolveLocale(saved,null)??[].concat(system??[]).map(value=>resolveLocale(value,null)).find(Boolean)??DEFAULT_LOCALE;
}
