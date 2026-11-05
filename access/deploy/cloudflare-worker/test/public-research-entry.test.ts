import assert from "node:assert/strict";
import test from "node:test";
import worker, { localizeHead } from "../src/public-research-entry.ts";

function fixture(fetchAsset: (request: Request) => Promise<Response> | Response) {
  const paths: string[] = [];
  const env = { ASSETS: { async fetch(request: Request) {
    paths.push(new URL(request.url).pathname);
    return fetchAsset(request);
  } } };
  return { paths, fetch: (request: Request) => worker.fetch(request, env as never, {} as never) };
}

test("public page paths map to their MPA HTML and receive security headers", async () => {
  const app = fixture(request => new Response(new URL(request.url).pathname, {
    headers: { "Content-Type": "text/html; charset=utf-8" },
  }));
  for (const [path, expected] of [
    ["/", "/zarathustra/"],
    ["/?lang=en", "/zarathustra/"],
    ["/zarathustra", "/zarathustra/"],
    ["/zarathustra/", "/zarathustra/"],
    ["/antiquity", "/antiquity/"],
    ["/antiquity/", "/antiquity/"],
  ]) {
    const response = await app.fetch(new Request(`https://treeofsophia.com${path}`));
    assert.equal(app.paths.at(-1), expected);
    assert.equal(await response.text(), expected);
    assert.match(response.headers.get("Content-Security-Policy") ?? "", /default-src 'self'/);
    assert.match(response.headers.get("Content-Security-Policy") ?? "", /'wasm-unsafe-eval'/);
    assert.doesNotMatch(response.headers.get("Content-Security-Policy") ?? "", /(?:^|\s)'unsafe-eval'(?:\s|;|$)/);
  }
});

test("research bundle and public corpus URLs lose only the public prefix", async () => {
  const app = fixture(request => new Response(new URL(request.url).pathname));
  for (const [path, expected] of [
    ["/__research/assets/reader.js", "/assets/reader.js"],
    ["/__research/wasm/tos_web_rules_bg.wasm", "/wasm/tos_web_rules_bg.wasm"],
    ["/__research/public-corpus/manifest.json", "/public-corpus/manifest.json"],
  ]) {
    const response = await app.fetch(new Request(`https://treeofsophia.com${path}`));
    assert.equal(app.paths.at(-1), expected);
    assert.equal(await response.text(), expected);
  }
});

test("unmatched paths reach the origin and www research redirects preserve path and query", async (t) => {
  const originRequests: Request[] = [];
  t.mock.method(globalThis, "fetch", async (request: Request) => { originRequests.push(request); return new Response("origin"); });
  const app = fixture(request => new URL(request.url).pathname === "/missing.json"
    ? new Response("not found", { status: 404 }) : new Response("unexpected"));
  const response = await app.fetch(new Request("https://www.treeofsophia.com/antiquity/?lang=en"));
  assert.equal(response.status, 308);
  assert.equal(response.headers.get("Location"), "https://treeofsophia.com/antiquity/?lang=en");
  const home = await app.fetch(new Request("https://www.treeofsophia.com/?lang=ru"));
  assert.equal(home.status, 308);
  assert.equal(home.headers.get("Location"), "https://treeofsophia.com/?lang=ru");
  for (const path of ["/api/health", "/static/reader.js"]) {
    const missing = await app.fetch(new Request(`https://treeofsophia.com${path}`));
    assert.equal(await missing.text(), "origin");
  }
  const post = new Request("https://treeofsophia.com/api/query?x=1", {method: "POST", body: "preserved"});
  assert.equal(await (await app.fetch(post)).text(), "origin");
  assert.equal(originRequests.at(-1), post);
  assert.equal(await originRequests.at(-1)!.text(), "preserved");
  assert.equal(app.paths.length, 0);
  assert.equal((await app.fetch(new Request("https://treeofsophia.com/__research/missing.json"))).status, 404);
  assert.deepEqual(app.paths, ["/missing.json"]);
});

test("HEAD is preserved and writes are rejected", async () => {
  let seenMethod = "";
  const app = fixture(request => { seenMethod = request.method; return new Response("ok"); });
  const head = await app.fetch(new Request("https://treeofsophia.com/__research/public-corpus/manifest.json", { method: "HEAD" }));
  assert.equal(seenMethod, "HEAD");
  assert.equal(head.status, 200);
  assert.equal((await app.fetch(new Request("https://treeofsophia.com/zarathustra", { method: "POST" }))).status, 405);
});

test("an address the site does not hold gets the Tree's own page with status 404, never the older atlas", async (t) => {
  const originRequests: Request[] = [];
  t.mock.method(globalThis, "fetch", async (request: Request) => { originRequests.push(request); return new Response("origin"); });
  const app = fixture(request => new URL(request.url).pathname === "/not-found/"
    ? new Response("<html>not in the Tree</html>", { headers: { "Content-Type": "text/html; charset=utf-8" } })
    : new Response("unexpected"));
  for (const path of ["/about", "/en/", "/research.html", "/static/index.html", "/static/research.html", "/zarathustra-old", "/index.html"]) {
    const response = await app.fetch(new Request(`https://treeofsophia.com${path}`));
    assert.equal(response.status, 404, path);
    assert.equal(await response.text(), "<html>not in the Tree</html>", path);
    assert.match(response.headers.get("Content-Security-Policy") ?? "", /default-src 'self'/);
  }
  const head = await app.fetch(new Request("https://treeofsophia.com/about", { method: "HEAD" }));
  assert.equal(head.status, 404);
  assert.equal(await head.text(), "");
  assert.equal(originRequests.length, 0);
});

test("icons, crawler files and link cards come from the root of the research build", async () => {
  const app = fixture(request => new Response(new URL(request.url).pathname));
  for (const path of ["/favicon.svg", "/favicon.ico", "/apple-touch-icon.png", "/robots.txt", "/sitemap.xml", "/og/zarathustra-en.jpg"]) {
    const response = await app.fetch(new Request(`https://treeofsophia.com${path}`));
    assert.equal(app.paths.at(-1), path);
    assert.equal(await response.text(), path);
  }
});

const STAND = `<!doctype html>
<html lang="en">
  <head>
    <title>Zarathustra · Tree of Sophia</title>
    <meta name="description" content="Thus Spoke Zarathustra in the Tree of Sophia" />
    <meta property="og:site_name" content="Tree of Sophia" />
    <meta property="og:title" content="Thus Spoke Zarathustra · Tree of Sophia" />
    <meta property="og:description" content="Thus Spoke Zarathustra in the Tree of Sophia" />
    <meta property="og:image" content="https://treeofsophia.com/og/zarathustra-en.jpg" />
    <meta property="og:locale" content="en_US" />
  </head>
</html>`;

test("a stand's page names itself in English unless its link asks for Russian", async () => {
  const russian = localizeHead(STAND, "zarathustra", "ru");
  assert.match(russian, /<html lang="ru"/);
  assert.match(russian, /<title>Заратустра · Древо Софии<\/title>/);
  assert.match(russian, /<meta property="og:title" content="Так говорил Заратустра · Древо Софии"/);
  assert.match(russian, /<meta name="description" content="«Так говорил Заратустра» в Древе Софии: немецкий оригинал/);
  assert.match(russian, /<meta property="og:site_name" content="Древо Софии"/);
  assert.match(russian, /og\/zarathustra-ru\.jpg/);
  assert.match(russian, /content="ru_RU"/);
  assert.equal(localizeHead(STAND, "zarathustra", null), STAND);
  assert.equal(localizeHead(STAND, "zarathustra", "xx"), STAND);
  const english = localizeHead(STAND, "zarathustra", "en");
  assert.match(english, /<title>Zarathustra · Tree of Sophia<\/title>/);
  assert.match(english, /<meta name="description" content="Thus Spoke Zarathustra in the Tree of Sophia: the German original/);
  const chinese = localizeHead(STAND, "zarathustra", "zh-Hans");
  assert.match(chinese, /<html lang="zh-Hans"/);
  assert.match(chinese, /<title>Zarathustra · Tree of Sophia<\/title>/);
  assert.doesNotMatch(localizeHead(STAND, "antiquity", "es"), /[А-Яа-яЁё]/);
  const app = fixture(() => new Response(STAND, { headers: { "Content-Type": "text/html; charset=utf-8", "Content-Length": "999", ETag: "x" } }));
  const response = await app.fetch(new Request("https://treeofsophia.com/?lang=ru"));
  assert.equal(app.paths.at(-1), "/zarathustra/");
  assert.match(await response.text(), /Так говорил Заратустра · Древо Софии/);
  assert.equal(response.headers.get("ETag"), null);
  const plain = await app.fetch(new Request("https://treeofsophia.com/zarathustra/"));
  assert.equal(await plain.text(), STAND);
});

test("the server reads a language tag as the page does, aliases included", async () => {
  const app = fixture(() => new Response(STAND, { headers: { "Content-Type": "text/html; charset=utf-8" } }));
  const page = async (lang: string) => (await app.fetch(new Request(`https://treeofsophia.com/zarathustra/?lang=${encodeURIComponent(lang)}`))).text();
  for (const alias of ["zh-CN", "zh_Hans", "zh-SG", "zh-Hans-CN", "ZH-cn", "zh"]) {
    const html = await page(alias);
    assert.match(html, /<html lang="zh-Hans"/, alias);
    assert.match(html, /<title>Zarathustra · Tree of Sophia<\/title>/, alias);
  }
  for (const alias of ["ru-RU", "RU", "ru_RU"]) {
    const html = await page(alias);
    assert.match(html, /<html lang="ru"/, alias);
    assert.match(html, /<title>Заратустра · Древо Софии<\/title>/, alias);
    assert.match(html, /content="ru_RU"/, alias);
  }
  assert.match(await page("es-MX"), /<html lang="es"/);
  assert.match(await page("de-AT"), /<html lang="de"/);
  for (const unknown of ["fr", "zh-TW", "xx-YY"]) assert.equal(await page(unknown), STAND, unknown);
});
