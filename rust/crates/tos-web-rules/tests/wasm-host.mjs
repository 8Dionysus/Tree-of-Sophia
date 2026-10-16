#!/usr/bin/env node
// Actual Node WebAssembly comparison with the page TS rule and Python Unicode oracle.
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { chooseKnowledgeSearchMode } from '../../../../access/web/src/knowledge-search.ts';

const [bindingPath, wasmPath, unicodePath] = process.argv.slice(2);
if (!bindingPath || !wasmPath || !unicodePath) {
  throw new Error('usage: node --experimental-strip-types wasm-host.mjs BINDING.js MODULE_bg.wasm PYTHON-UNICODE.json');
}
const binding = await import(pathToFileURL(bindingPath).href);
const wasmBytes = await readFile(wasmPath);
const unicode = JSON.parse(await readFile(unicodePath, 'utf8'));
assert.equal(unicode.profile, 'tos-python-native-unicode-v1');
assert.equal(unicode.cases.length, 25);

const before = process.memoryUsage();
const start = performance.now();
await binding.default({ module_or_path: wasmBytes });
const startupMs = performance.now() - start;
const encoder = new TextEncoder();
function invoke(raw) {
  const result = binding.select_knowledge_search_mode_wasm_v1(encoder.encode(raw));
  try {
    return { mode: result.mode() ?? null, error: result.error_code() ?? null, minimum: result.minimum() };
  } finally { result.free(); }
}
function request(capabilities, query, requested_mode) {
  return JSON.stringify({ capabilities, ...(query === undefined ? {} : { query }),
    ...(requested_mode === undefined ? {} : { requested_mode }) });
}
function expectMode(caseId, capabilities, query, requested_mode, mode) {
  assert.equal(chooseKnowledgeSearchMode(capabilities, requested_mode, query), mode, `${caseId}/ts`);
  assert.deepEqual(invoke(request(capabilities, query, requested_mode)),
    { mode, error: null, minimum: -1 }, `${caseId}/wasm`);
}
function expectError(caseId, capabilities, query, requested_mode, pattern, code, minimum = -1) {
  assert.throws(() => chooseKnowledgeSearchMode(capabilities, requested_mode, query), pattern, `${caseId}/ts`);
  assert.deepEqual(invoke(request(capabilities, query, requested_mode)),
    { mode: null, error: code, minimum }, `${caseId}/wasm`);
}

const both = { modes: { indexed: { available: true, min_normalized_query_code_points: 3 },
  compressed: { available: true } } };
const indexedTwo = { modes: { indexed: { available: true, min_normalized_query_code_points: 2 } } };
const noEngines = { modes: { indexed: { available: false }, compressed: { available: false } } };
const operationStart = performance.now();
expectMode('both-prefer-indexed', both, 'fate', undefined, 'indexed');
expectMode('short-falls-to-compressed', both, '道', undefined, 'compressed');
expectError('explicit-short', both, '道', 'indexed', /requires at least 3/, 'query_too_short', 3);
expectMode('dotted-i-expands', indexedTwo, '  İ  ', undefined, 'indexed');
expectError('emoji-one-codepoint', indexedTwo, '😀', undefined, /shorter than the minimum/, 'no_eligible_mode');
const malformed = { modes: { indexed: { available: true, min_normalized_query_code_points: true } } };
expectError('boolean-minimum', malformed, 'fate', undefined, /invalid knowledge search query capability/, 'invalid_capability');
const unavailableMalformed = { modes: { indexed: { available: false, min_normalized_query_code_points: true } } };
expectError('unavailable-before-minimum', unavailableMalformed, 'fate', 'indexed', /mode unavailable/, 'mode_unavailable');
expectError('no-engines', noEngines, 'fate', undefined, /indexed and compressed engines are unavailable/, 'engines_unavailable');
expectError('unsafe-minimum', { modes: { indexed: { available: true, min_normalized_query_code_points: 9007199254740992 } } },
  'fate', undefined, /invalid knowledge search query capability/, 'invalid_capability');
expectMode('decimal-one-minimum', { modes: { indexed: { available: true, min_normalized_query_code_points: 1.0 } } },
  'fate', undefined, 'indexed');
assert.deepEqual(invoke('{"capabilities":{"modes":{"indexed":{"available":false}}},"capabilities":{"modes":{"indexed":{"available":true}}},"query":"fate"}'),
  { mode: 'indexed', error: null, minimum: -1 }, 'request-last-wins');
assert.equal(invoke('{"capabilities":{"modes":{"indexed":{"available":true}}},"query":"\\ud800"}').error,
  'invalid_input', 'lone surrogate is an explicit Rust refusal');

let unicodeComparisons = 0;
for (const vector of unicode.cases) {
  const floor = Math.max(1, vector.lowered_code_points);
  const caps = { modes: { indexed: { available: true, min_normalized_query_code_points: floor } } };
  if (vector.lowered_code_points === 0) {
    expectError(`${vector.name}/empty`, caps, vector.input, undefined,
      /shorter than the minimum/, 'no_eligible_mode');
  } else {
    expectMode(`${vector.name}/floor`, caps, vector.input, undefined, 'indexed');
  }
  unicodeComparisons++;
  const higher = { modes: { indexed: { available: true,
    min_normalized_query_code_points: vector.lowered_code_points + 1 } } };
  expectError(`${vector.name}/above-floor`, higher, vector.input, undefined,
    /shorter than the minimum/, 'no_eligible_mode');
  unicodeComparisons++;
}
const after = process.memoryUsage();
console.log(JSON.stringify({
  status: 'pass', host: `Node ${process.version} WebAssembly`,
  ts_cases: 10, raw_only_cases: 2, python_unicode_cases: unicode.cases.length,
  unicode_comparisons: unicodeComparisons,
  js_bytes: (await stat(bindingPath)).size, wasm_bytes: (await stat(wasmPath)).size,
  startup_ms: Number(startupMs.toFixed(3)),
  operation_ms: Number((performance.now() - operationStart).toFixed(3)),
  rss_before_bytes: before.rss, rss_after_bytes: after.rss, heap_used_after_bytes: after.heapUsed,
}));
