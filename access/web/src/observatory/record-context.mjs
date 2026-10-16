let installedRuntime;
export function installRecordContextRules(runtime){
  if(typeof runtime?.essential_context_v1!=='function')throw new TypeError('Generated record-context Rust rule export is incomplete');
  installedRuntime=runtime;
}
export function essentialContext(raw){
  if(!installedRuntime)throw new Error('Record-context Rust rules are not installed');
  return installedRuntime.essential_context_v1(raw);
}
