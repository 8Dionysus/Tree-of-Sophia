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

// The access Worker owns the Custom Domain and keeps the public APIs and their data; the
// site itself is the two stands. Only these reach the access Worker.
function originPath(pathname: string): boolean {
  if (pathname === "/health" || pathname.startsWith("/api/") || pathname.startsWith("/__edge/")) return true;
  return pathname.startsWith("/static/") && !/\.html?$/i.test(pathname);
}

// Icons, the crawler files and the link cards live at the root of the research build.
const ROOT_FILES = new Set(["/favicon.svg", "/favicon.ico", "/apple-touch-icon.png", "/robots.txt", "/sitemap.xml"]);
const rootFile = (pathname: string) => ROOT_FILES.has(pathname) || /^\/og\/[a-z-]+\.jpg$/.test(pathname);

// A stand's page names itself in English, the site's default. A link that asks for Russian gets
// its title, description and card in Russian; a link in another language of the site keeps the
// English head under its own language tag. A shared link reads in the reader's language.
type Head = { title: string; og: string; description: string };
const HEADS: Record<string, Record<"en" | "ru", Head>> = {
  zarathustra: {
    en: {
      title: "Zarathustra · Tree of Sophia",
      og: "Thus Spoke Zarathustra · Tree of Sophia",
      description: "Thus Spoke Zarathustra in the Tree of Sophia: the German original, Common’s and Antonovsky’s translations, and a map of the book — its concepts, figures, links and reading routes.",
    },
    ru: {
      title: "Заратустра · Древо Софии",
      og: "Так говорил Заратустра · Древо Софии",
      description: "«Так говорил Заратустра» в Древе Софии: немецкий оригинал, переводы Антоновского и Коммона и карта книги — её понятия, фигуры, связи и маршруты чтения.",
    },
  },
  antiquity: {
    en: {
      title: "Antiquity · Tree of Sophia",
      og: "Antiquity · Tree of Sophia",
      description: "Antiquity in the Tree of Sophia: Plato, Aristotle, the Stoics, Epicurus, Cicero, Seneca and Sextus — questions, places in the original and their readings.",
    },
    ru: {
      title: "Античность · Древо Софии",
      og: "Античность · Древо Софии",
      description: "Античность в Древе Софии: Платон, Аристотель, стоики, Эпикур, Цицерон, Сенека и Секст — вопросы, места в оригинале и их прочтения.",
    },
  },
};
const SITE = { en: "Tree of Sophia", ru: "Древо Софии" } as const;
const OG_LOCALE = { en: "en_US", ru: "ru_RU" } as const;
const LANGUAGES = new Set(["en", "ru", "es", "de", "zh-Hans"]);
const escapeHtml = (value: string) => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/** The stand page's head in the language its link asks for: Russian copy for Russian, English otherwise. */
export function localizeHead(html: string, stand: string, lang: string | null): string {
  const heads = HEADS[stand];
  if (!heads || !lang || !LANGUAGES.has(lang)) return html;
  const copy = lang === "ru" ? "ru" : "en", head = heads[copy];
  const content = (pattern: RegExp, value: string) => (text: string) => text.replace(pattern, `$1${escapeHtml(value)}$2`);
  return [
    (text: string) => text.replace(/<html lang="[^"]*"/, `<html lang="${lang === "zh-Hans" ? "zh" : lang}"`),
    (text: string) => text.replace(/<title>[^<]*<\/title>/, `<title>${escapeHtml(head.title)}</title>`),
    content(/(<meta name="description" content=")[^"]*(")/, head.description),
    content(/(<meta property="og:description" content=")[^"]*(")/, head.description),
    content(/(<meta property="og:title" content=")[^"]*(")/, head.og),
    content(/(<meta property="og:site_name" content=")[^"]*(")/, SITE[copy]),
    content(/(<meta property="og:locale" content=")[^"]*(")/, OG_LOCALE[copy]),
    (text: string) => text.replace(/(<meta property="og:image" content="[^"]*\/og\/[a-z]+)-(?:ru|en)(\.jpg")/, `$1-${copy}$2`),
  ].reduce((text, step) => step(text), html);
}

async function standPage(env: Env, request: Request, page: string): Promise<Response> {
  const response = await env.ASSETS.fetch(new Request(new URL(page, request.url), request));
  const stand = page.replace(/\//g, "");
  const lang = new URL(request.url).searchParams.get("lang");
  if (!response.ok || request.method === "HEAD" || !HEADS[stand] || !lang || !LANGUAGES.has(lang)) return withReaderSecurity(response);
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
    // Route fetch() reaches the access Worker's Custom Domain without re-entering this route.
    if (originPath(url.pathname)) {
      if (["treeofsophia.com", "www.treeofsophia.com"].includes(url.hostname.toLowerCase())) return fetch(request);
      return textResponse("not found", 404, request.method);
    }
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
    if (page) return standPage(env, request, page);

    if (url.pathname.startsWith("/__research/")) {
      url.pathname = `/${url.pathname.slice("/__research/".length)}`;
      return withReaderSecurity(await env.ASSETS.fetch(new Request(url, request)));
    }

    if (rootFile(url.pathname)) return withReaderSecurity(await env.ASSETS.fetch(new Request(url, request)));

    return notFound(env, request);
  },
} satisfies ExportedHandler<Env>;
