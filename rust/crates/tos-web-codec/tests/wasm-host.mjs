#!/usr/bin/env node
// Real JavaScript WebAssembly host check of the versioned Rust codec binding.
// The independent fixture supplies expected bytes and errors; JS only carries
// bytes across the generated wasm-bindgen boundary.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const [bindingPath, wasmPath, foundationPath, profilesPath, floatPath] = process.argv.slice(2);
if (!bindingPath || !wasmPath || !foundationPath || !profilesPath) {
  throw new Error('usage: node wasm-host.mjs BINDING.js MODULE_bg.wasm FOUNDATION.jsonl CANONICAL-PROFILES.jsonl [FLOAT-DIFFERENTIAL.jsonl]');
}

const binding = await import(pathToFileURL(bindingPath).href);
const wasmBytes = await readFile(wasmPath);
const lines = async path => (await readFile(path, 'utf8')).trim().split('\n').map(JSON.parse);
const foundation = await lines(foundationPath);
const profiles = await lines(profilesPath);
const floats = floatPath ? await lines(floatPath) : [];
const before = process.memoryUsage();
const start = performance.now();
await binding.default({ module_or_path: wasmBytes });
const startupMs = performance.now() - start;

const capabilities = JSON.parse(binding.codec_capabilities_v1());
assert.equal(capabilities.abi, 'tos_web_codec_v1');
assert.equal(capabilities.json_format, 'tos_foundation_json_v1');
assert.equal(capabilities.canonical_profile, 'tos_corpus_snapshot_canonical_v1');
assert.deepEqual(capabilities.json_profiles, ['tos_published_json_v1', 'tos_request_last_wins_json_v1']);
assert.deepEqual(capabilities.canonical_profiles, [
  'tos_corpus_snapshot_canonical_v1', 'tos_source_record_digest_v1', 'tos_source_command_input_v1',
]);
assert.equal(capabilities.canonical_float_supported, true);
assert.equal(capabilities.strict_duplicate_rejection, true);
assert.equal(capabilities.request_last_wins, true);
assert.equal(capabilities.escaped_lone_surrogate_preserved, true);
assert.equal(capabilities.canonical_lone_surrogate_supported, false);

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
let foundationChecked = 0;
let profileChecked = 0;
let floatChecked = 0;
const operationStart = performance.now();
for (const vector of foundation) {
  if (!['parse', 'parse_preserve', 'canonical'].includes(vector.operation)) continue;
  const raw = vector.input_hex
    ? Uint8Array.from(Buffer.from(vector.input_hex, 'hex'))
    : encoder.encode(vector.input_utf8);
  const operation = vector.operation === 'canonical' ? 'canonical' : 'parse_preserve';
  if (vector.operation === 'canonical') assert.equal(vector.profile, 'CorpusSnapshotV1');
  const profile = vector.operation === 'canonical' ? capabilities.canonical_profile
    : vector.mode === 'PublishedStrict' ? capabilities.json_profiles[0]
    : vector.mode === 'RequestLastWins' ? capabilities.json_profiles[1]
    : (() => { throw new Error(`unknown fixture mode ${vector.mode}`); })();
  const result = binding.codec_v1(raw, operation, profile);
  try {
    if (vector.expected.reject) {
      assert.equal(result.ok(), false, vector.case_id);
      assert.equal(result.error_code(), vector.expected.reject, vector.case_id);
      assert.equal(result.bytes().length, 0, vector.case_id);
    } else {
      assert.equal(result.ok(), true, vector.case_id);
      assert.equal(result.error_code(), undefined, vector.case_id);
      const actual = decoder.decode(result.bytes());
      assert.equal(actual, vector.expected.preserved_utf8 ?? vector.expected.canonical_utf8, vector.case_id);
      if (vector.expected.digest_hex) {
        assert.equal(createHash('sha256').update(result.bytes()).digest('hex'), vector.expected.digest_hex, vector.case_id);
      }
    }
    foundationChecked++;
  } finally {
    result.free();
  }
}
assert.equal(foundation.length, 23, 'the independent foundation fixture changed');
assert.equal(foundationChecked, 18, 'the WEB-applicable foundation selection changed');

function expectCanonical(raw, profile, expectedBytes, expectedDigest, caseId) {
  const result = binding.codec_v1(raw, 'canonical', profile);
  try {
    assert.equal(result.ok(), true, caseId);
    const bytes = result.bytes();
    assert.equal(decoder.decode(bytes), expectedBytes, caseId);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), expectedDigest, caseId);
  } finally {
    result.free();
  }
}

for (const vector of profiles) {
  const raw = encoder.encode(vector.input_utf8);
  const expected = vector.expected;
  expectCanonical(raw, capabilities.canonical_profiles[0], expected.corpus_snapshot_v1_utf8,
    expected.corpus_snapshot_v1_sha256, `${vector.case_id}/corpus`);
  expectCanonical(raw, capabilities.canonical_profiles[1], expected.source_record_v1_utf8,
    expected.source_record_v1_sha256, `${vector.case_id}/record`);
  expectCanonical(raw, capabilities.canonical_profiles[2], expected.source_record_v1_utf8,
    expected.source_record_v1_sha256, `${vector.case_id}/command`);
  profileChecked += 3;
}
assert.equal(profiles.length, 17, 'the independent profile fixture changed');
assert.equal(profileChecked, 51);

for (const vector of floats) {
  const raw = encoder.encode(vector.input_utf8);
  expectCanonical(raw, capabilities.canonical_profiles[0], vector.corpus_snapshot_v1_utf8,
    vector.corpus_snapshot_v1_sha256, `${vector.bits_hex}/corpus`);
  expectCanonical(raw, capabilities.canonical_profiles[1], vector.source_record_v1_utf8,
    vector.source_record_v1_sha256, `${vector.bits_hex}/record`);
  floatChecked += 2;
}
if (floatPath) {
  assert.equal(floats.length, 4129, 'the independent finite-float sample changed');
  assert.equal(floatChecked, 8258);
}

const duplicate = binding.codec_v1(encoder.encode('{"id":1,"\\u0069d":2}'),
  'canonical', capabilities.canonical_profiles[2]);
try {
  assert.equal(duplicate.ok(), false);
  assert.equal(duplicate.error_code(), 'duplicate_member');
} finally {
  duplicate.free();
}

const unknown = binding.codec_v1(encoder.encode('{}'), 'canonical', 'FutureV2');
try {
  assert.equal(unknown.ok(), false);
  assert.equal(unknown.error_code(), 'unsupported_format');
} finally {
  unknown.free();
}
const after = process.memoryUsage();
console.log(JSON.stringify({
  status: 'pass',
  host: `Node ${process.version} WebAssembly`,
  abi: capabilities.abi,
  foundation_vectors: foundationChecked,
  profile_vectors: profileChecked,
  float_vectors: floatChecked,
  wasm_bytes: (await stat(wasmPath)).size,
  js_bytes: (await stat(bindingPath)).size,
  startup_ms: Number(startupMs.toFixed(3)),
  vector_ms: Number((performance.now() - operationStart).toFixed(3)),
  rss_before_bytes: before.rss,
  rss_after_bytes: after.rss,
  heap_used_after_bytes: after.heapUsed,
}));
