import { publicPrototypeAsset, withReaderSecurity } from "./public-prototypes.ts";
import { localizeHead, HEADS } from "./public-head.ts";
import { resolveLocale } from "../../../../bonsai/lib/locale-tag.mjs";


interface Env {
  ASSETS: Fetcher;
}

function textResponse(message: string, status: number, method: string): Response {
  return withReaderSecurity(new Response(method === "HEAD" ? null : message, {
    status,
    headers: { "Content-Type": "text/plain; charset=utf-8", "Cache-Control": "no-store" },
  }));
}

// Icons, the crawler files and the link cards live at the root of the research build.
// The theme script sets the reader's theme before a page paints (web/public/theme.js).
const ROOT_FILES = new Set(["/favicon.svg", "/favicon.ico", "/apple-touch-icon.png", "/robots.txt", "/sitemap.xml", "/theme.js"]);
const rootFile = (pathname: string) => ROOT_FILES.has(pathname) || /^\/og\/[a-z-]+\.jpg$/.test(pathname);

async function standPage(env: Env, request: Request, page: string): Promise<Response> {
  const response = await env.ASSETS.fetch(new Request(new URL(page, request.url), request));
  const stand = page.replace(/\//g, "");
  const lang = new URL(request.url).searchParams.get("lang");
  if (!response.ok || request.method === "HEAD" || !HEADS[stand] || !resolveLocale(lang, null)) return withReaderSecurity(response);
  const headers = new Headers(response.headers);
  headers.delete("Content-Length");
  headers.delete("ETag");
  return withReaderSecurity(new Response(localizeHead(await response.text(), stand, lang), { status: response.status, headers }));
}

/** The page for an address the site does not hold, with the status that says so. */
async function notFound(env: Env, request: Request): Promise<Response> {
  const page = await env.ASSETS.fetch(new Request(new URL("/not-found/", request.url)));
  if (!page.ok) return textResponse("not found", 404, request.method);
  const headers = new Headers(page.headers);
  headers.set("Cache-Control", "no-store");
  return withReaderSecurity(new Response(request.method === "HEAD" ? null : page.body, { status: 404, headers }));
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/health" || ["/api/", "/__edge/", "/static/"].some(prefix => url.pathname.startsWith(prefix))) {
      return notFound(env, request);
    }
    if (url.hostname.toLowerCase() === "www.treeofsophia.com") {
      url.hostname = "treeofsophia.com";
      return withReaderSecurity(Response.redirect(url.toString(), 308));
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return textResponse("method not allowed", 405, request.method);
    }

    // The public site's main address greets the reader with its bonsai to choose from.
    // Fetch the directory directly to avoid ASSETS index canonicalization.
    const page = url.pathname === "/" ? "/home/" : publicPrototypeAsset(url.pathname);
    if (page) return standPage(env, request, page);

    if (url.pathname.startsWith("/__research/")) {
      url.pathname = `/${url.pathname.slice("/__research/".length)}`;
      return withReaderSecurity(await env.ASSETS.fetch(new Request(url, request)));
    }

    if (rootFile(url.pathname)) return withReaderSecurity(await env.ASSETS.fetch(new Request(url, request)));

    return notFound(env, request);
  },
} satisfies ExportedHandler<Env>;
