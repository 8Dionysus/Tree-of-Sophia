import {t} from './ui-i18n.mjs';
export const INTERFACE_KEY='tos-observatory-interface-v1';
export const TOOL_IDS=['search','lenses','workspace','navigation','builder','evidence','sources','reader'];
export const DEFAULT_INTERFACE={v:1,pinned:['search','lenses','workspace','navigation'],dock:'auto',text:'comfortable',labels:'normal',scrollAction:'auto',dragAction:'rotate',sensitivity:'normal',motion:'system',uiLanguage:'ru',theme:'dark',sizes:{},positions:{}};
const panelIds=['inspector','workspace','evidence','navigation','builder','studio','reader','history','copy','settings','search','lenses'];
export function validateInterface(value){
  if(value?.v!==1||!Array.isArray(value.pinned)||value.pinned.length>TOOL_IDS.length||new Set(value.pinned).size!==value.pinned.length||value.pinned.some(id=>!TOOL_IDS.includes(id))
    ||!['auto','left','right'].includes(value.dock)||!['comfortable','large'].includes(value.text)||!['normal','large'].includes(value.labels)||!value.sizes||typeof value.sizes!=='object')throw new Error(t("Настройки интерфейса не удалось прочитать."));
  // Old mouse mode was an explicit zoom choice; trackpad migrates to Auto.
  if(value.inputMode!==undefined&&!['trackpad','mouse'].includes(value.inputMode))throw new Error(t("Настройки управления повреждены."));
  const scrollAction=value.scrollAction??(value.inputMode==='mouse'?'zoom':'auto'),dragAction=value.dragAction??'rotate',sensitivity=value.sensitivity??'normal',motion=value.motion??'system',uiLanguage=value.uiLanguage??'ru',theme=value.theme??'dark';
  if(!['auto','pan','zoom'].includes(scrollAction)||!['rotate','pan'].includes(dragAction)||!['gentle','normal','fast'].includes(sensitivity)||!['system','paused','running'].includes(motion)
    ||!['ru','en','es'].includes(uiLanguage)||!['dark','light'].includes(theme))throw new Error(t("Настройки управления или оформления повреждены."));
  const sizes={};for(const id of panelIds){const size=value.sizes[id];if(!size)continue;if(!Number.isFinite(size.width)||size.width<280||size.width>760||!Number.isFinite(size.height)||size.height<240||size.height>800)throw new Error(t("Сохранённый размер окна повреждён."));sizes[id]={width:size.width,height:size.height};}
  const positions={};if(value.positions!==undefined&&(!value.positions||typeof value.positions!=='object'||Array.isArray(value.positions)))throw new Error(t("Сохранённое положение окна повреждено."));
  for(const id of panelIds){const point=value.positions?.[id];if(!point)continue;if(!Number.isFinite(point.x)||point.x<0||point.x>1||!Number.isFinite(point.y)||point.y<0||point.y>1)throw new Error(t("Сохранённое положение окна повреждено."));positions[id]={x:point.x,y:point.y};}
  return {v:1,pinned:[...value.pinned],dock:value.dock,text:value.text,labels:value.labels,scrollAction,dragAction,sensitivity,motion,uiLanguage,theme,sizes,positions};
}
export function readInterface(storage){const text=storage?.getItem(INTERFACE_KEY);if(!text)return structuredClone(DEFAULT_INTERFACE);if(text.length>5000)throw new Error(t("Настройки интерфейса слишком велики."));return validateInterface(JSON.parse(text));}
