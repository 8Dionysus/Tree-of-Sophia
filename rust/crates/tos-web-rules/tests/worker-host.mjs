#!/usr/bin/env node
// Local workerd import and execution of the generated browser rule binding.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [miniflarePackage, bindingPath, wasmPath, wranglerPath] = process.argv.slice(2);
if (!miniflarePackage || !bindingPath || !wasmPath || !wranglerPath) {
  throw new Error('usage: node worker-host.mjs EXISTING_WORKER_PACKAGE.json BINDING.js MODULE_bg.wasm WRANGLER.jsonc');
}
const require = createRequire(pathToFileURL(miniflarePackage).href);
const { Miniflare, convertV4MiniflareOptions } = require('miniflare');
const wrangler = await readFile(wranglerPath, 'utf8');
const compatibilityDate = /^\s*"compatibility_date"\s*:\s*"(\d{4}-\d{2}-\d{2})"/m.exec(wrangler)?.[1];
assert.ok(compatibilityDate);
const glue = await readFile(bindingPath, 'utf8');
const wasm = await readFile(wasmPath);
const moduleRoot = dirname(bindingPath);
const entry = `
import {initSync,select_knowledge_search_mode_wasm_v1} from './tos_web_rules.mjs';
import rulesModule from './tos_web_rules_bg.wasm';
let ready=false;
export default {async fetch(request){
  if(!ready){initSync({module:rulesModule});ready=true;}
  const result=select_knowledge_search_mode_wasm_v1(new Uint8Array(await request.arrayBuffer()));
  try{return Response.json({mode:result.mode()??null,error:result.error_code()??null,minimum:result.minimum()});}
  finally{result.free();}
}};`;
const originalCwd = process.cwd();
const temp = await mkdtemp(join(tmpdir(), 'tos-rust-web-rules-worker-'));
process.chdir(temp);
let mf;
try {
  mf = new Miniflare(convertV4MiniflareOptions({
    modulesRoot: moduleRoot,
    modules: [
      { type: 'ESModule', path: join(moduleRoot, 'worker-host.mjs'), contents: entry },
      { type: 'ESModule', path: join(moduleRoot, 'tos_web_rules.mjs'), contents: glue },
      { type: 'CompiledWasm', path: join(moduleRoot, 'tos_web_rules_bg.wasm'), contents: wasm },
    ],
    compatibilityDate,
  }));
  const call = async (body) => {
    const response = await mf.dispatchFetch('http://local.test/', { method: 'POST', body });
    assert.equal(response.status, 200, await response.clone().text());
    return response.json();
  };
  const caps = { modes: { indexed: { available: true, min_normalized_query_code_points: 2 },
    compressed: { available: true } } };
  assert.deepEqual(await call(JSON.stringify({ capabilities: caps, query: '  İ  ' })),
    { mode: 'indexed', error: null, minimum: -1 });
  assert.deepEqual(await call(JSON.stringify({ capabilities: caps, query: '😀' })),
    { mode: 'compressed', error: null, minimum: -1 });
  assert.deepEqual(await call(JSON.stringify({ capabilities: caps, query: '😀', requested_mode: 'indexed' })),
    { mode: null, error: 'query_too_short', minimum: 2 });
  console.log(JSON.stringify({ status: 'pass', host: 'local Miniflare/workerd WebAssembly', compatibility_date: compatibilityDate, cases: 3 }));
} finally {
  if (mf) await mf.dispose();
  process.chdir(originalCwd);
  await rm(temp, { recursive: true });
}
