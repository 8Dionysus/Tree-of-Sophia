import assert from "node:assert/strict";
import test from "node:test";
import worker from "../src/public-research-entry.ts";

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
