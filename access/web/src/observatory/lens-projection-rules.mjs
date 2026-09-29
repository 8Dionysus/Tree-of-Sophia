let runtime,slots;
export function installLensProjectionRules(value){
  if(typeof value?.LensSortSession!=='function'||typeof value?.LensNodeSession!=='function')
    throw new TypeError('Generated lens projection rules are unavailable');
  runtime=value;slots=JSON.parse(value.LensNodeSession.slot_table());
}
export const createLensSortSession=()=>new runtime.LensSortSession();
export const createLensNodeSession=nextSlot=>new runtime.LensNodeSession(nextSlot);
export const lensDegreeNext=value=>runtime.LensNodeSession.degree_next(Boolean(value),typeof value==='number'?value:0);
export const lensSlotPosition=slot=>slots[slot]?.slice();
