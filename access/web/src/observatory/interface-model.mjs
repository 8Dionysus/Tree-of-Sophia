import {t} from './ui-i18n.mjs';
import {normalizeInterfacePreferences} from './interface-model-rust.mjs';
export const INTERFACE_KEY='tos-observatory-interface-v1';
export const DEFAULT_INTERFACE={v:1,pinned:['search','lenses','workspace','navigation'],dock:'auto',text:'comfortable',labels:'normal',scrollAction:'auto',dragAction:'rotate',sensitivity:'normal',motion:'system',uiLanguage:'ru',theme:'dark',sizes:{},positions:{}};
export function validateInterface(value){
  return normalizeInterfacePreferences(value);
}
export function readInterface(storage){const text=storage?.getItem(INTERFACE_KEY);if(!text)return structuredClone(DEFAULT_INTERFACE);if(text.length>5000)throw new Error(t("Настройки интерфейса слишком велики."));return validateInterface(JSON.parse(text));}
