// The browser Vite graph has two existing extensionless TypeScript imports.
// Keep the Node actual-binding harness on the same source graph.
export async function resolve(specifier,context,nextResolve){
  if(specifier.endsWith('/research-workspace-rust'))
    return nextResolve(`${specifier}.ts`,context);
  return nextResolve(specifier,context);
}
