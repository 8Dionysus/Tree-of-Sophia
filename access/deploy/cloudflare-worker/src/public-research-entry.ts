import { publicPrototypeAsset, withReaderSecurity } from "./public-prototypes.ts";
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

// The access Worker owns the Custom Domain and keeps the public APIs and their data; the
// site itself is the stands. Only these reach the access Worker.
function originPath(pathname: string): boolean {
  if (pathname === "/health" || pathname.startsWith("/api/") || pathname.startsWith("/__edge/")) return true;
  return pathname.startsWith("/static/") && !/\.html?$/i.test(pathname);
}

// Icons, the crawler files and the link cards live at the root of the research build.
// The theme script sets the reader's theme before a page paints (web/public/theme.js).
const ROOT_FILES = new Set(["/favicon.svg", "/favicon.ico", "/apple-touch-icon.png", "/robots.txt", "/sitemap.xml", "/theme.js"]);
const rootFile = (pathname: string) => ROOT_FILES.has(pathname) || /^\/og\/[a-z-]+\.jpg$/.test(pathname);

// A stand's page names itself in English, the site's default. A link that asks for Russian gets
// its title, description and card in Russian; a link in another language of the site keeps the
// English head under its own language tag. A shared link reads in the reader's language.
type Head = { title: string; og: string; description: string };
const HEADS: Record<string, Record<"en" | "ru", Head>> = {
  home: {
    en: {
      title: "Tree of Sophia",
      og: "Tree of Sophia",
      description: "Thought grown from the sources. Each bonsai cultivates its own direction.",
    },
    ru: {
      title: "Древо Софии",
      og: "Древо Софии",
      description: "Мысль, выращенная из источников. Каждый бонсай культивирует своё направление.",
    },
  },
  zarathustra: {
    en: {
      title: "Zarathustra · Tree of Sophia",
      og: "Thus Spoke Zarathustra · Tree of Sophia",
      description: "Thus Spoke Zarathustra by Friedrich Nietzsche in the Tree of Sophia.",
    },
    ru: {
      title: "Заратустра · Древо Софии",
      og: "Так говорил Заратустра · Древо Софии",
      description: "«Так говорил Заратустра» Фридриха Ницше в Древе Софии.",
    },
  },
  antiquity: {
    en: {
      title: "Antiquity · Tree of Sophia",
      og: "Antiquity · Tree of Sophia",
      description: "The philosophy of ancient Greece and Rome in the Tree of Sophia.",
    },
    ru: {
      title: "Античность · Древо Софии",
      og: "Античность · Древо Софии",
      description: "Философия Древней Греции и Рима в Древе Софии.",
    },
  },
  aesthetics: {
    en: {
      title: "Aesthetics · Tree of Sophia",
      og: "Aesthetics · Tree of Sophia",
      description: "The philosophy of beauty, art and sensory experience in the Tree of Sophia.",
    },
    ru: {
      title: "Эстетика · Древо Софии",
      og: "Эстетика · Древо Софии",
      description: "Философия прекрасного, искусства и чувственного опыта в Древе Софии.",
    },
  },
};
const SITE = { en: "Tree of Sophia", ru: "Древо Софии" } as const;
const OG_LOCALE = { en: "en_US", ru: "ru_RU" } as const;
const escapeHtml = (value: string) => value.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/**
 * The stand page's head in the language its link asks for: Russian copy for Russian, English otherwise.
 * The tag is read as the browser reads it (bonsai/lib/locale-tag.mjs), so zh-CN or ru-RU name the same
 * language here as in the page; a tag the site does not have leaves the English page as it is.
 */
export function localizeHead(html: string, stand: string, requested: string | null): string {
  const heads = HEADS[stand], lang = resolveLocale(requested, null);
  if (!heads || !lang) return html;
  const copy = lang === "ru" ? "ru" : "en", head = heads[copy];
  const content = (pattern: RegExp, value: string) => (text: string) => text.replace(pattern, `$1${escapeHtml(value)}$2`);
  return [
    (text: string) => text.replace(/<html lang="[^"]*"/, `<html lang="${lang}"`),
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
