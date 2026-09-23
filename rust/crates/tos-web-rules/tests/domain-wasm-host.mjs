#!/usr/bin/env node
// Execute the generated WASM against the maintained page and workspace rules.
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { createPageCommandRegistry } from '../../../../access/web/src/page-commands.ts';
import { createWebMCPAdapter } from '../../../../access/web/src/webmcp.ts';
import { createResearchWorkspace } from '../../../../access/web/src/research-workspace.ts';

const [bindingPath, wasmPath] = process.argv.slice(2);
if (!bindingPath || !wasmPath) throw new Error('usage: node --experimental-strip-types domain-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const binding = await import(pathToFileURL(bindingPath).href);
const wasmBytes = await readFile(wasmPath);
const before = process.memoryUsage();
const start = performance.now();
await binding.default({ module_or_path: wasmBytes });
const startupMs = performance.now() - start;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function envelope(request) {
  const result = binding.compact_knowledge_search_page_wasm_v1(encoder.encode(JSON.stringify(request)));
  try { return result.ok() ? { value: JSON.parse(decoder.decode(result.bytes())) } : { error: result.error_code() }; }
  finally { result.free(); }
}
function proposal(request) {
  const result = binding.workspace_proposal_digest_wasm_v1(encoder.encode(JSON.stringify(request)));
  try { return { digest: result.digest() ?? null, error: result.error_code() ?? null }; }
  finally { result.free(); }
}

const context = {
  mode: 'philosophy', view_id: 'chronology', graph_mode: 'nodes', selected: null,
  path_start_node_id: null, active_layers: [], active_predicates: [],
  deep_link: 'http://tos.local/?view=chronology', research_workspace: {},
};
let captured;
const registry = createPageCommandRegistry(() => context, {
  'tos.page.knowledge-search': (input) => ({
    schema: input.search_mode === 'compressed' ? 'tos_knowledge_search_compressed_v3' : 'tos_knowledge_search_indexed_v2',
    search_mode: input.search_mode || 'indexed', query: input.query,
    result_count: 2,
    nodes: [{ id: 'node:long/' + 'n'.repeat(256), semantic_kind: 'concept', label: 'A'.repeat(118) + '😀x', source_refs: ['source:a', 'source:b', 'source:c', 'source:d'] }],
    relations: [{ id: 'edge:one', kind: 'relation', label: 'Relation', from_id: 'node:a', to_id: 'node:b' }],
    counts: { matching_nodes: null, matching_relations: null },
    page: { has_more: true, next_cursor: 'opaque/+' + 'x'.repeat(65520) },
    source_revision: 'revision:host',
  }),
});
const originalInvoke = registry.invoke;
registry.invoke = async (...args) => { captured = await originalInvoke(...args); return captured; };
const tools = new Map();
const adapter = createWebMCPAdapter(registry, { modelContext: { registerTool: async (tool) => tools.set(tool.name, tool) } }, new Set(['tos.page.knowledge-search']));
await adapter.start();
const tool = tools.get('tos.page.knowledge-search');
assert.ok(tool);
let tsCases = 0;
for (const mode of ['indexed', 'compressed']) {
  const reply = await tool.execute({ query: '道', search_mode: mode }, { signal: new AbortController().signal });
  const expected = JSON.parse(reply.content[0].text);
  const observed = envelope({ limit: 6, expected_mode: mode, result: captured });
  assert.deepEqual(observed, { value: expected }, `${mode} page/result parity`);
  assert.equal(expected.next_cursor.length, 65528);
  tsCases++;
}
adapter.stop();
const badMode = structuredClone(captured);
badMode.value.schema = 'tos_knowledge_search_indexed_v2';
assert.equal(envelope({ limit: 6, expected_mode: 'compressed', result: badMode }).error, 'invalid_mode_schema');
const oversize = structuredClone(captured);
oversize.value.nodes.push({ id: 'node:extra' });
oversize.value.result_count++;
assert.equal(envelope({ limit: 1, expected_mode: 'compressed', result: oversize }).error, 'page_exceeds_limit');
const badCursor = structuredClone(captured);
badCursor.value.page.has_more = false;
assert.equal(envelope({ limit: 6, expected_mode: 'compressed', result: badCursor }).error, 'invalid_page');

const workspace = createResearchWorkspace({ sessionId: 'wasm-host', persistence: false });
workspace.addHypothesis({ id: 'hyp:one', title: 'Reading', body: 'Local reading.' });
const currentRevision = workspace.getState().revision;
const staged = workspace.stageProposal({
  id: 'proposal:one', kind: 'interpretation', parentHypothesisId: 'hyp:one', targetId: 'edge:one',
  statement: 'Another reading.', sourceRefs: ['source:one'], evidenceRefs: ['evidence:one'],
  confidencePosture: { value: 'low', meaning: 'maker_declared_uncertainty_not_truth_probability' },
  actorOrigin: 'agent', basePageRevision: 7, baseWorkspaceRevision: currentRevision,
  dataFingerprint: 'sha256:fixture', createdAt: '2026-09-23T12:00:00.000Z',
});
const wire = JSON.parse(workspace.exportPacket()).proposals[0];
assert.equal(wire.digest, staged.digest);
const stageRequest = { operation: 'stage', current_revision: currentRevision, hypothesis_ids: ['hyp:one'], existing_proposal_ids: [], proposal: { ...wire } };
delete stageRequest.proposal.digest;
assert.deepEqual(proposal(stageRequest), { digest: staged.digest, error: null }, 'TS stage digest parity');
assert.deepEqual(proposal({ operation: 'verify', hypothesis_ids: ['hyp:one'], proposal: wire }), { digest: staged.digest, error: null }, 'TS import digest parity');
assert.equal(proposal({ ...stageRequest, current_revision: currentRevision + 1 }).error, 'stale_revision');
const tampered = { ...wire, statement: 'Changed.' };
assert.equal(proposal({ operation: 'verify', hypothesis_ids: ['hyp:one'], proposal: tampered }).error, 'digest_mismatch');
let dateCases = 0;
const dateVectors = [
  ['+275760-09-13T00:00:00.000Z', true],
  ['+275760-09-13T00:00:00.001Z', false],
  ['+275760-12-31T00:00:00.000Z', false],
  ['-271821-04-20T00:00:00.000Z', true],
  ['-271821-04-19T23:59:59.999Z', false],
];
for (const year of [-271821, -100000, -10000, -9999, -400, -1, 0, 1, 4, 100, 400, 1970, 2000, 2024, 9999, 10000, 100000, 275760]) {
  const yearText = year < 0 ? `-${String(-year).padStart(6, '0')}`
    : year > 9999 ? `+${String(year).padStart(6, '0')}` : String(year).padStart(4, '0');
  for (const suffix of ['-02-28T23:59:59.999Z', '-02-29T00:00:00.000Z']) {
    const date = yearText + suffix;
    const parsed = new Date(date);
    dateVectors.push([date, Number.isFinite(parsed.getTime()) && parsed.toISOString() === date]);
  }
}
for (const [date, accepted] of dateVectors) {
  const tsWorkspace = createResearchWorkspace({ sessionId: `date-${date}`, persistence: false });
  tsWorkspace.addHypothesis({ id: 'hyp:one', title: 'Reading', body: 'Local reading.' });
  const tsInput = { id: 'proposal:one', kind: 'interpretation', parentHypothesisId: 'hyp:one', targetId: 'edge:one',
    statement: 'Another reading.', sourceRefs: ['source:one'], evidenceRefs: ['evidence:one'],
    confidencePosture: { value: 'low', meaning: 'maker_declared_uncertainty_not_truth_probability' },
    actorOrigin: 'agent', basePageRevision: 7, baseWorkspaceRevision: 1, dataFingerprint: 'sha256:fixture', createdAt: date };
  let tsDigest = null;
  try { tsDigest = tsWorkspace.stageProposal(tsInput).digest; }
  catch (error) { assert.match(String(error), /canonical UTC ISO timestamp/); }
  const observed = proposal({ ...stageRequest, proposal: { ...stageRequest.proposal, created_at: date } });
  assert.equal(Boolean(tsDigest), accepted, `${date}/ts`);
  assert.equal(observed.error, accepted ? null : 'invalid_proposal', `${date}/wasm`);
  if (accepted) assert.equal(observed.digest, tsDigest, `${date}/digest`);
  dateCases++;
}

const after = process.memoryUsage();
console.log(JSON.stringify({ status: 'pass', host: `Node ${process.version} WebAssembly`, ts_page_cases: tsCases,
  refusal_cases: 5, ts_workspace_cases: 2, date_edge_cases: dateCases, js_bytes: (await stat(bindingPath)).size,
  wasm_bytes: (await stat(wasmPath)).size, startup_ms: Number(startupMs.toFixed(3)),
  rss_before_bytes: before.rss, rss_after_bytes: after.rss, heap_used_after_bytes: after.heapUsed }));
