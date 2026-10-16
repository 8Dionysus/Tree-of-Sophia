let runtime;
export function installSourceDossierRules(value){
  if(['source_dossier_reference_v1','validate_source_dossier_request_v1','validate_source_dossier_v1'].some(name=>typeof value?.[name]!=='function'))throw new TypeError('Generated source dossier Rust rules are unavailable');
  runtime=value;
}
function installed(){if(!runtime)throw new Error('Source dossier Rust rules are not installed');return runtime;}
export const sourceDossierReference=value=>installed().source_dossier_reference_v1(value);
export const sourceDossierRequest=(value,limit)=>installed().validate_source_dossier_request_v1(value,limit);
export const sourceDossierCheck=(packet,expected)=>installed().validate_source_dossier_v1(packet,expected);
