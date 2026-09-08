export const displayLanguageKey=key=>!['default','original'].includes(key)&&/^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(key);
const present=value=>typeof value==='string'&&Boolean(value.trim());
const languageTag=value=>typeof value==='string'&&/^(?:[a-z]{2,8}(?:-[a-z0-9]{1,8})*|[ix](?:-[a-z0-9]{1,8})+)$(?![\s\S])/i.test(value);
const formKey=key=>['default','original'].includes(key)||languageTag(key);

function suppliedForm(value,preferred,selection){
  // The caller owns the envelope's content-revision binding. This field-level
  // check only accepts a selection consistent with this dictionary and request.
  if(!value||typeof value!=='object'||Array.isArray(value)||!selection||typeof selection!=='object'||Array.isArray(selection))return null;
  const requested=preferred==='default'?'auto':preferred;
  const {selected_key:key,actual_language:lang,text,reason,available_keys:available}=selection;
  const keys=Object.keys(value).filter(key=>formKey(key)&&present(value[key])).sort();
  // Navigation wording can be present with content_available=false; that flag
  // describes source authority and is not promoted by this presentation helper.
  if(selection.requested_language!==requested||typeof selection.content_available!=='boolean'
    ||typeof key!=='string'||!Object.hasOwn(value,key)||!present(text)||value[key]!==text
    ||!Array.isArray(available)||available.length!==keys.length
    ||!available.every(item=>typeof item==='string')||available.slice().sort().some((item,index)=>item!==keys[index])
    ||!keys.includes(key)||!(lang===null||languageTag(lang)&&!['default','original','auto'].includes(lang.toLowerCase()))
    ||!['exact-language','less-specific-language','original-role','automatic','fallback'].includes(reason))return null;
  return {text,key,lang,fallback:reason==='fallback'||reason==='less-specific-language'};
}

// Shared by navigation and reading. Select supplied wording, never translate
// it or treat the owner fallback keys `default` / `original` as language tags.
export function displayForm(value,preferred='ru',selection=null){
  // A rejected supplied selection must not silently become a legacy guess.
  if(selection!==null)return suppliedForm(value,preferred,selection);
  const keys=[preferred,'default','original','ru','en',...Object.keys(value||{}).filter(displayLanguageKey)];
  const key=keys.find(key=>present(value?.[key]));
  return key?{text:value[key],key,lang:displayLanguageKey(key)?key:null,fallback:key!==preferred}:null;
}
