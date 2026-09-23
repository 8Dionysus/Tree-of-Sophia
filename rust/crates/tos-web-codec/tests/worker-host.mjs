#!/usr/bin/env node
// Bounded local workerd/Miniflare import check. No D1, remote API or deployment.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [miniflarePackage, bindingPath, wasmPath, wranglerPath] = process.argv.slice(2);
if (!miniflarePackage || !bindingPath || !wasmPath || !wranglerPath) {
  throw new Error('usage: node worker-host.mjs MINIFLARE_PACKAGE.json BINDING.js MODULE_bg.wasm WRANGLER.jsonc');
}
const wrangler = await readFile(wranglerPath, 'utf8');
const compatibilityDate = /^\s*"compatibility_date"\s*:\s*"(\d{4}-\d{2}-\d{2})"/m.exec(wrangler)?.[1];
if (!compatibilityDate) throw new Error('Wrangler compatibility_date is missing');
const originalCwd = process.cwd();
const workerCwd = await mkdtemp(join(tmpdir(), 'tos-web-worker-host-'));
process.chdir(workerCwd);
let mf;
try {
  const require = createRequire(pathToFileURL(miniflarePackage).href);
  const { Miniflare, convertV4MiniflareOptions } = require('miniflare');
  const generatedGlue = await readFile(bindingPath, 'utf8');
  const generatedWasm = await readFile(wasmPath);
  const moduleRoot = dirname(bindingPath);
  const entry = `
import { initSync, codec_v1, codec_capabilities_v1 } from './tos_web_codec.mjs';
import codecModule from './tos_web_codec_bg.wasm';
let ready = false;
export default {
  async fetch(request) {
    try {
      if (!ready) { initSync({ module: codecModule }); ready = true; }
      const capabilities = JSON.parse(codec_capabilities_v1());
      const input = new Uint8Array(await request.arrayBuffer());
      const result = codec_v1(input, 'canonical', capabilities.canonical_profiles[1]);
      try {
        if (!result.ok()) return new Response('', {
          status: 422, headers: { 'x-tos-error': result.error_code() },
        });
        return new Response(result.bytes(), { status: 200 });
      } finally { result.free(); }
    } catch (error) { return new Response(String(error.stack ?? error), { status: 500 }); }
  },
};`;
  mf = new Miniflare(convertV4MiniflareOptions({
    modulesRoot: moduleRoot,
    modules: [
      { type: 'ESModule', path: join(moduleRoot, 'worker-host.mjs'), contents: entry },
      { type: 'ESModule', path: join(moduleRoot, 'tos_web_codec.mjs'), contents: generatedGlue },
      { type: 'CompiledWasm', path: join(moduleRoot, 'tos_web_codec_bg.wasm'), contents: generatedWasm },
    ],
    compatibilityDate,
  }));
  const good = await mf.dispatchFetch('http://local.test/', {
    method: 'POST', body: '{"f":1e-6}',
  });
  assert.equal(good.status, 200, await good.clone().text());
  assert.equal(await good.text(), '{"f":1e-06}');
  const duplicate = await mf.dispatchFetch('http://local.test/', {
    method: 'POST', body: '{"id":1,"\\u0069d":2}',
  });
  assert.equal(duplicate.status, 422);
  assert.equal(duplicate.headers.get('x-tos-error'), 'duplicate_member');
  console.log(JSON.stringify({ status: 'pass', host: 'local Miniflare/workerd', compatibility_date: compatibilityDate, cases: 2 }));
} finally {
  if (mf) await mf.dispose();
  process.chdir(originalCwd);
  await rm(workerCwd, { recursive: true });
}
