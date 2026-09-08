export const displayLanguageKey=key=>!['default','original'].includes(key)&&/^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(key);
const present=value=>typeof value==='string'&&Boolean(value.trim());

// Shared by navigation and reading. Select supplied wording, never translate
// it or treat the owner fallback keys `default` / `original` as language tags.
export function displayForm(value,preferred='ru'){
  const keys=[preferred,'default','original','ru','en',...Object.keys(value||{}).filter(displayLanguageKey)];
  const key=keys.find(key=>present(value?.[key]));
  return key?{text:value[key],key,lang:displayLanguageKey(key)?key:null,fallback:key!==preferred}:null;
}
