#!/usr/bin/env node
// Local workerd execution of the same generated WASM domain-rule binding.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [miniflarePackage, bindingPath, wasmPath, wranglerPath, temporalCapturePath] = process.argv.slice(2);
if (!miniflarePackage || !bindingPath || !wasmPath || !wranglerPath) throw new Error('usage: node domain-worker-host.mjs WORKER_PACKAGE.json BINDING.mjs MODULE_bg.wasm WRANGLER.jsonc');
const require = createRequire(pathToFileURL(miniflarePackage).href);
const { Miniflare, convertV4MiniflareOptions } = require('miniflare');
const { transform } = require('esbuild');
const wrangler = await readFile(wranglerPath, 'utf8');
const compatibilityDate = /^\s*"compatibility_date"\s*:\s*"(\d{4}-\d{2}-\d{2})"/m.exec(wrangler)?.[1];
assert.ok(compatibilityDate);
const glue = await readFile(bindingPath, 'utf8');
const wasm = await readFile(wasmPath);
const moduleRoot = dirname(bindingPath);
const temporalDriver = (await transform(await readFile(new URL('../../../../access/deploy/cloudflare-worker/src/selected-temporal-runtime.ts',import.meta.url),'utf8'),
  {loader:'ts',format:'esm',target:'es2022'})).code;
const entry = `
import { initSync, compact_knowledge_search_page_wasm_v1, workspace_proposal_digest_wasm_v1, workspace_transition_wasm_v1, TemporalReplaySession } from './tos_web_rules.mjs';
import { deliverSelectedTemporal } from './selected-temporal-runtime.mjs';
import rulesModule from './tos_web_rules_bg.wasm';
let ready = false;
export default { async fetch(request) {
  if (!ready) { initSync({ module: rulesModule }); ready = true; }
  const raw = new Uint8Array(await request.arrayBuffer());
  if(new URL(request.url).pathname==='/temporal'){
    const fixture=JSON.parse(new TextDecoder().decode(raw)),encoder=new TextEncoder(),abort=new AbortController();let held=false;
    const selected={sourceRevision:fixture.revision,claimSourceGraph:fixture.profile,admission:encoder.encode(fixture.admission),
      async checkSelected(){},async readExactNode(id){
        if(fixture.carriers){const row=fixture.carriers.find(row=>row.id===id);return row?encoder.encode(row.raw):null;}
        if(id!==fixture.id)throw Error('fixture exact identity differs');
        if(fixture.cancel)abort.abort();
        return fixture.absent?null:encoder.encode(fixture.carrier);
      },async withCurrentDisclosure(deliver){if(fixture.withdrawn)throw Error('withdrawn');
        held=true;try{return await deliver();}finally{held=false;}}
    };
    try{return await deliverSelectedTemporal({TemporalReplaySession},selected,encoder.encode(fixture.request),async bytes=>{
      if(!held)throw Error('disclosure lease absent');return new Response(bytes,{headers:{'content-type':'application/json'}});
    },abort.signal);}catch(error){return Response.json({error:error.code??error.name,message:error.message});}
  }
  if (new URL(request.url).pathname === '/envelope') {
    const result = compact_knowledge_search_page_wasm_v1(raw);
    try { return result.ok() ? new Response(result.bytes(), { headers: { 'content-type': 'application/json' } }) : Response.json({ error: result.error_code() }); }
    finally { result.free(); }
  }
  if (new URL(request.url).pathname === '/workspace') {
    const result = workspace_transition_wasm_v1(raw);
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
      { type: 'ESModule', path: join(moduleRoot, 'selected-temporal-runtime.mjs'), contents: temporalDriver },
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
  const schema = 'tos_research_workspace_transition_v1';
  const created = await call('/workspace', { schema, operation: 'create', session_id: 'workerd', history_limit: 2 });
  assert.equal(created.machine.state.revision, 0);
  const excluded = await call('/workspace', { schema, operation: 'apply', machine: created.machine,
    command: { kind: 'edge.exclude', edge_id: 'edge:a' } });
  assert.equal(excluded.machine.state.revision, 1);
  assert.deepEqual(excluded.machine.state.excluded_edge_ids, ['edge:a']);
  const undone = await call('/workspace', { schema, operation: 'undo', machine: excluded.machine });
  assert.equal(undone.value, true);
  assert.deepEqual(undone.machine.state.excluded_edge_ids, []);
  assert.deepEqual(await call('/workspace', { schema, operation: 'import', machine: undone.machine, packet: '{"schema":"bad"}' }), { error: 'invalid_packet' });
  const oracle=JSON.parse(await readFile(new URL('../../tos-query/tests/fixtures/cmp_knowledge_inspect_python_oracle.json',import.meta.url),'utf8'));
  const owned=oracle.cases.find(row=>row.kind==='nodes'&&row.packet.matches.length===1);assert.ok(owned);
  const node=owned.packet.matches[0],temporalFixture={revision:owned.packet.source_revision,profile:node.source_graph,id:node.id,
    carrier:JSON.stringify(node),request:JSON.stringify({schema_version:'tos_temporal_comparison_request_v1',source_revision:owned.packet.source_revision,
      left:{node_id:node.id,content_revision:node.content_revision},right:{node_id:node.id,content_revision:node.content_revision}}),
    admission:JSON.stringify({max_json_bytes:1048576,max_json_depth:64,max_json_visits:300000,max_integer_digits:4300,
      max_source_bytes:1048576,max_replay_bytes:8388608,max_output_bytes:1048576})};
  const temporal=await call('/temporal',temporalFixture);
  assert.equal(temporal.comparison.status,'unsupported');assert.deepEqual(temporal.left.claim,node);assert.deepEqual(temporal.right.claim,node);
  assert.equal((await call('/temporal',{...temporalFixture,cancel:true})).error,'AbortError');
  assert.equal((await call('/temporal',{...temporalFixture,withdrawn:true})).message,'withdrawn');
  assert.equal((await call('/temporal',{...temporalFixture,absent:true})).error,'UnknownIdentifier');
  let genuineTemporalCases=0;
  if(temporalCapturePath){
    const capture=JSON.parse(await readFile(temporalCapturePath,'utf8'));
    assert.equal(new Set(capture.carriers.map(row=>row.id)).size,capture.carriers.length);
    for(const testCase of capture.temporal){
      const response=await mf.dispatchFetch('http://local.test/temporal',{method:'POST',body:JSON.stringify({
        revision:capture.source_revision,profile:capture.claim_source_graph,admission:temporalFixture.admission,
        request:testCase.request,carriers:capture.carriers})});
      assert.equal(response.status,200);
      assert.equal(await response.text(),testCase.packet,'actual native selected and workerd/WASM packet bytes differ');
      genuineTemporalCases++;
    }
    assert.ok(genuineTemporalCases>0,'native capture contains no temporal cases');
  }
  console.log(JSON.stringify({ status: 'pass', host: 'local Miniflare/workerd WebAssembly', compatibility_date: compatibilityDate,
    cases: 9, temporal_bridge_cases:4,genuine_temporal_cases:genuineTemporalCases }));
} finally {
  if (mf) await mf.dispose();
  process.chdir(originalCwd);
  await rm(temp, { recursive: true });
}
