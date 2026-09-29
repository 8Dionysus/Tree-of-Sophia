let runtime;
export function installSourceDossierRules(value){
  if(typeof value?.SourceDossierSession!=='function')throw new TypeError('Generated source dossier Rust rules are unavailable');
  runtime=value;
}
export function createSourceDossierSession(request=false){
  if(!runtime)throw new Error('Source dossier Rust rules are not installed');
  return new runtime.SourceDossierSession(request);
}
export const dossierTextUnits=text=>Uint16Array.from({length:text.length},(_,index)=>text.charCodeAt(index));
