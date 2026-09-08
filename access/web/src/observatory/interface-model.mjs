export const INTERFACE_KEY='tos-observatory-interface-v1';
export const TOOL_IDS=['search','lenses','workspace','navigation','builder','evidence','sources','reader'];
export const DEFAULT_INTERFACE={v:1,pinned:['search','lenses','workspace','navigation'],dock:'auto',text:'comfortable',labels:'normal',sizes:{}};
const panelIds=['inspector','workspace','evidence','navigation','builder','studio','reader','history','copy'];
export function validateInterface(value){
  if(value?.v!==1||!Array.isArray(value.pinned)||value.pinned.length>TOOL_IDS.length||new Set(value.pinned).size!==value.pinned.length||value.pinned.some(id=>!TOOL_IDS.includes(id))
    ||!['auto','left','right'].includes(value.dock)||!['comfortable','large'].includes(value.text)||!['normal','large'].includes(value.labels)||!value.sizes||typeof value.sizes!=='object')throw new Error('Настройки интерфейса не удалось прочитать.');
  const sizes={};for(const id of panelIds){const size=value.sizes[id];if(!size)continue;if(!Number.isFinite(size.width)||size.width<280||size.width>760||!Number.isFinite(size.height)||size.height<240||size.height>800)throw new Error('Сохранённый размер окна повреждён.');sizes[id]={width:size.width,height:size.height};}
  return {v:1,pinned:[...value.pinned],dock:value.dock,text:value.text,labels:value.labels,sizes};
}
export function readInterface(storage){const text=storage?.getItem(INTERFACE_KEY);if(!text)return structuredClone(DEFAULT_INTERFACE);if(text.length>5000)throw new Error('Настройки интерфейса слишком велики.');return validateInterface(JSON.parse(text));}
