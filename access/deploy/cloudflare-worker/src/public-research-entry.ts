import { publicPrototypeAsset, withReaderSecurity } from "./public-prototypes.ts";


interface Env {
  ASSETS: Fetcher;
}

function textResponse(message: string, status: number, method: string): Response {
  return withReaderSecurity(new Response(method === "HEAD" ? null : message, {
    status,
    headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" },
  }));
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.hostname.toLowerCase() === "www.treeofsophia.com") {
      url.hostname = "treeofsophia.com";
      return withReaderSecurity(Response.redirect(url.toString(), 308));
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return textResponse("method not allowed", 405, request.method);
    }

    // The public site's main address opens the research space, too.
    // Fetch the directory directly to avoid ASSETS index canonicalization.
    const page = url.pathname === "/" ? "/zarathustra/" : publicPrototypeAsset(url.pathname);
    if (page) {
      return withReaderSecurity(await env.ASSETS.fetch(new Request(new URL(page, request.url), request)));
    }

    if (url.pathname.startsWith("/__research/")) {
      url.pathname = `/${url.pathname.slice("/__research/".length)}`;
      return withReaderSecurity(await env.ASSETS.fetch(new Request(url, request)));
    }

    return textResponse("not found", 404, request.method);
  },
} satisfies ExportedHandler<Env>;
