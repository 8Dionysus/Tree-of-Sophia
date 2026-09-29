// Installed once by the browser entry after its generated WASM runtime loads.
let validatePacket;
const encoder = new TextEncoder();

export function installWorkspaceCopyRules(runtime) {
  if (typeof runtime?.validate_workspace_copy_wasm_v1 !== 'function')
    throw new TypeError('Workspace-copy WASM rule is unavailable');
  validatePacket = runtime.validate_workspace_copy_wasm_v1;
}

export function validateWorkspaceCopyPacket(value) {
  // Direct source imports retain the component validators as their oracle.
  // The serving entry installs the Rust rule before importing the app.
  if (validatePacket) validatePacket(encoder.encode(JSON.stringify(value)));
}
