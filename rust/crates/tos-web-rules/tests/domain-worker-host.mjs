#!/usr/bin/env node
// Local workerd execution of the same generated WASM domain-rule binding.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [miniflarePackage, bindingPath, wasmPath, wranglerPath] = process.argv.slice(2);
if (!miniflarePackage || !bindingPath || !wasmPath || !wranglerPath) throw new Error('usage: node domain-worker-host.mjs WORKER_PACKAGE.json BINDING.mjs MODULE_bg.wasm WRANGLER.jsonc');
const require = createRequire(pathToFileURL(miniflarePackage).href);
const { Miniflare, convertV4MiniflareOptions } = require('miniflare');
const wrangler = await readFile(wranglerPath, 'utf8');
const compatibilityDate = /^\s*"compatibility_date"\s*:\s*"(\d{4}-\d{2}-\d{2})"/m.exec(wrangler)?.[1];
assert.ok(compatibilityDate);
const glue = await readFile(bindingPath, 'utf8');
const wasm = await readFile(wasmPath);
const moduleRoot = dirname(bindingPath);
const entry = `
import { initSync, compact_knowledge_search_page_wasm_v1, workspace_proposal_digest_wasm_v1 } from './tos_web_rules.mjs';
import rulesModule from './tos_web_rules_bg.wasm';
let ready = false;
export default { async fetch(request) {
  if (!ready) { initSync({ module: rulesModule }); ready = true; }
  const raw = new Uint8Array(await request.arrayBuffer());
  if (new URL(request.url).pathname === '/envelope') {
    const result = compact_knowledge_search_page_wasm_v1(raw);
    try { return result.ok() ? new Response(result.bytes(), { headers: { 'content-type': 'application/json' } }) : Response.json({ error: result.error_code() }); }
    finally { result.free(); }
  }
  const result = workspace_proposal_digest_wasm_v1(raw);
  try { return Response.json({ digest: result.digest() ?? null, error: result.error_code() ?? null }); }
  finally { result.free(); }
} };`;
const originalCwd = process.cwd();
const temp = await mkdtemp(join(tmpdir(), 'tos-rust-web-domain-worker-'));
process.chdir(temp);
let mf;
try {
  mf = new Miniflare(convertV4MiniflareOptions({
    modulesRoot: moduleRoot,
    modules: [
      { type: 'ESModule', path: join(moduleRoot, 'worker-domain-host.mjs'), contents: entry },
      { type: 'ESModule', path: join(moduleRoot, 'tos_web_rules.mjs'), contents: glue },
      { type: 'CompiledWasm', path: join(moduleRoot, 'tos_web_rules_bg.wasm'), contents: wasm },
    ],
    compatibilityDate,
  }));
  const call = async (path, body) => {
    const response = await mf.dispatchFetch(`http://local.test${path}`, { method: 'POST', body: JSON.stringify(body) });
    assert.equal(response.status, 200, await response.clone().text());
    return response.json();
  };
  const proposal = {
    id: 'proposal:one', kind: 'interpretation', parent_hypothesis_id: 'hyp:one', target_id: 'edge:one',
    statement: 'Another reading.', source_refs: ['source:one'], evidence_refs: ['evidence:one'],
    confidence_posture: { value: 'low', meaning: 'maker_declared_uncertainty_not_truth_probability' },
    actor_origin: 'agent', base_page_revision: 7, base_workspace_revision: 1,
    data_fingerprint: 'sha256:fixture', created_at: '2026-09-23T12:00:00.000Z',
    local_only: true, review_status: 'pending_human_review', canon: false,
  };
  const digest = 'fnv1a64:8199d8cd9ebba5ad';
  assert.deepEqual(await call('/proposal', { operation: 'stage', current_revision: 1, hypothesis_ids: ['hyp:one'], existing_proposal_ids: [], proposal }),
    { digest, error: null });
  assert.deepEqual(await call('/proposal', { operation: 'verify', hypothesis_ids: ['hyp:one'], proposal: { ...proposal, digest } }),
    { digest, error: null });
  assert.deepEqual(await call('/proposal', { operation: 'stage', current_revision: 2, hypothesis_ids: ['hyp:one'], existing_proposal_ids: [], proposal }),
    { digest: null, error: 'stale_revision' });
  const result = {
    schema: 'tos_page_command_result_v1', command_id: 'tos.page.knowledge-search', context_revision: 2,
    context: { deep_link: 'http://tos.local/' }, value: {
      schema: 'tos_knowledge_search_indexed_v2', search_mode: 'indexed', query: 'fate', result_count: 1,
      nodes: [{ id: 'node:one', semantic_kind: 'concept', label: 'First', source_refs: ['source:a'] }], relations: [],
      counts: { matching_nodes: null }, page: { has_more: true, next_cursor: 'opaque/+' },
      source_revision: 'revision:test',
    },
  };
  const page = await call('/envelope', { limit: 6, expected_mode: 'indexed', result });
  assert.equal(page.next_cursor, 'opaque/+');
  assert.equal(page.nodes[0].id, 'node:one');
  assert.equal(page.source_revision, 'revision:test');
  const refused = await call('/envelope', { limit: 6, expected_mode: 'compressed', result });
  assert.deepEqual(refused, { error: 'invalid_mode_schema' });
  console.log(JSON.stringify({ status: 'pass', host: 'local Miniflare/workerd WebAssembly', compatibility_date: compatibilityDate, cases: 5 }));
} finally {
  if (mf) await mf.dispose();
  process.chdir(originalCwd);
  await rm(temp, { recursive: true });
}
