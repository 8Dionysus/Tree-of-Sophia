let runtime;
export function installClientInspectionRules(value){
  if(typeof value?.validate_client_inspection_v1!=='function')throw new TypeError('Generated client inspection Rust rule is unavailable');
  runtime=value;
}
function installed(){if(!runtime)throw new Error('Client inspection Rust rules are not installed');return runtime;}
export const checkClientInspection=(packet,kind,id,expected,contentRevision,localized)=>installed().validate_client_inspection_v1(packet,kind,id,expected,contentRevision,localized);
