let installedRuntime;
export function installClientInspectionRules(runtime){
  if(typeof runtime?.ClientInspectionSession!=='function')throw new TypeError('Generated client inspection Rust rule is unavailable');
  installedRuntime=runtime;
}
export function createClientInspectionSession(...observations){
  if(!installedRuntime)throw new Error('Client inspection Rust rules are not installed');
  return new installedRuntime.ClientInspectionSession(...observations);
}
// RegExp.test applies ToString with string hint and refuses symbols.
export function inspectionRevisionText(value){
  if(typeof value==='symbol')throw new TypeError('Cannot convert a Symbol value to a string');
  return String(value);
}

export const inspectionRevisionUnits=text=>Uint16Array.from({length:text.length},(_,index)=>text.charCodeAt(index));

export const inspectionRefInvalid=(string,truthy)=>installedRuntime.ClientInspectionSession.ref_invalid(string,truthy);

export const createClientItemsSession=node=>installedRuntime.ClientInspectionSession.items(node);
