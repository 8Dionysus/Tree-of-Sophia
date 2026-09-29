// Fixed copy-admission phases carry only metadata. Native JSON/Date and all
// component values remain in the host; no complete copy is reparsed in WASM.
let validatePacket;
const encoder = new TextEncoder();

export function installWorkspaceCopyRules(runtime) {
  if (typeof runtime?.validate_workspace_copy_wasm_v1 !== 'function')
    throw new TypeError('Workspace-copy WASM rule is unavailable');
  validatePacket = runtime.validate_workspace_copy_wasm_v1;
}

function admit(value) {
  if (!validatePacket) throw new TypeError('Workspace-copy WASM rule is unavailable');
  validatePacket(encoder.encode(JSON.stringify(value)));
}
export function validateWorkspaceCopyEnvelope(value) {
  // The existing schema is 28 ASCII units; retain one extra unit so a longer
  // prefix cannot become an admitted schema after this bounded projection.
  admit({phase:'envelope',schema:typeof value?.schema==='string'?value.schema.slice(0,29):null,
    version:typeof value?.v==='number'?value.v:null,
    valid_date:typeof value?.exportedAt==='string'&&Number.isFinite(Date.parse(value.exportedAt)),
    place_count:Array.isArray(value?.places)?value.places.length:null});
}
export function validateWorkspaceCopyPlacesSize(characters) {
  admit({phase:'places-size',characters});
}
export function validateWorkspaceCopyPlaces(places,lenses) {
  admit({phase:'places',ids:places.map(place=>place.id),lenses_array:Array.isArray(lenses)});
}
export function validateWorkspaceCopyLenses(lenses) {
  admit({phase:'lenses',names:lenses.map(lens=>lens.name)});
}
export function validateWorkspaceCopyPacket(value) {
  admit({phase:'normalized',history:{version:value.history.v,ids:value.history.entries.map(entry=>entry.id),cursor:value.history.cursor},
    resume:value.resume===null?null:{id:value.resume?.id},preferences_version:value.preferences.v,
    reading_version:value.reading.v,research_object:value.research!==null&&typeof value.research==='object'&&!Array.isArray(value.research)});
}
export function validateWorkspaceCopyCustody(flags) {
  admit({phase:'storage',flags});
}
