import { withSecurity } from "./common.ts";

/** The public prototype HTML is emitted by the Vite multi-page build. */
export function publicPrototypeAsset(pathname: string): string | null {
  // ASSETS canonicalizes /index.html to its directory. Fetch the directory
  // directly so a rewrite cannot redirect the public URL back to itself.
  if (pathname === '/zarathustra' || pathname === '/zarathustra/') return '/zarathustra/';
  if (pathname === '/antiquity' || pathname === '/antiquity/') return '/antiquity/';
  return null;
}

export function withReaderSecurity(response: Response): Response {
  const secured = withSecurity(response);
  // The reader compiles its bundled Rust WASM. JavaScript eval and inline
  // scripts stay disallowed; only WASM compilation is admitted on this Worker.
  secured.headers.set("Content-Security-Policy", secured.headers.get("Content-Security-Policy")!
    .replace("script-src 'self'", "script-src 'self' 'wasm-unsafe-eval'"));
  return secured;
}

