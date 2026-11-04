// The site's interface languages and the one reading of a language tag, shared by the
// browser (lib/locales.mjs) and the Worker that serves the stand pages, so a link names
// the same language on both sides: zh-CN, zh_Hans and zh-SG are Simplified Chinese,
// ru-RU is Russian, an unknown tag falls back.

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
