import assert from "node:assert/strict";
import test from "node:test";

// Explicit fixture lifetime; production callers select their own whole budget.
process.env.TOS_D1_SQL_STREAM_MAX_SECONDS = '30';
import { mkdtempSync, readFileSync, writeFileSync, renameSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';

import {
  syncFile,
  sqlImportChunks,
  LEGACY_REVISION_QUERY,
  REVISION_QUERY,
  resultRows,
  revisionFromRows,
  revisionQueryForColumns,
  syncDecision,
} from "../scripts/deploy_edge.mjs";

test('streamed imports preserve complete SQL statements and clean owned scratch on abort', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'tos-sql-contract-'));
  const path = join(directory, 'input.sql');
  const sql = "INSERT INTO t VALUES ('София''s 🌳\r\n\n  \rnext;\n-- literal');\n"
    + "CREATE TRIGGER p AFTER INSERT ON t\nBEGIN\nSELECT 1;\nSELECT 2;\nEND;\nSELECT 3;";
  try {
    writeFileSync(path, sql);
    const chunks = [];
    for await (const file of sqlImportChunks(path, 40)) {
      chunks.push(readFileSync(file, 'utf8'));
      // A suspended consumer must have just one file, not a prefetched corpus.
      assert.deepEqual(readdirSync(dirname(file)), [basename(file)]);
    }
    assert.equal(chunks.join(''), sql);
    assert.equal(chunks.length, 3);
    assert.deepEqual(readdirSync(directory), ['input.sql']);
    for await (const file of sqlImportChunks(path, 40)) {
      assert.ok(readFileSync(file, 'utf8').endsWith('\n'));
      break;
    }
    assert.deepEqual(readdirSync(directory), ['input.sql']);
  } finally {
    rmSync(directory, { recursive: true });
  }
});

test('SQL framing rejects incomplete and over-budget records and cleans only owned scratch', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'tos-sql-framing-'));
  const path = join(directory, 'input.sql');
  const marker = join(directory, 'unrelated');
  writeFileSync(marker, 'keep');
  try {
    for (const invalid of ["SELECT 'unterminated\n\n", "SELECT 1", "SELECT '" + '🌳'.repeat(25_000) + "';\n"]) {
      writeFileSync(path, "SELECT 1;\nSELECT 2;\n" + invalid);
      await assert.rejects(async () => {
        for await (const file of sqlImportChunks(path, 10)) readFileSync(file);
      }, /SQL import framing failed/);
      assert.deepEqual(readdirSync(directory).sort(), ['input.sql', 'unrelated']);
      assert.equal(readFileSync(marker, 'utf8'), 'keep');
    }
    for (const maximum of [0, -1, 1.5, Infinity, 9007199254740992, '40', 40n, null]) {
      await assert.rejects(async () => {
        for await (const file of sqlImportChunks(path, maximum)) readFileSync(file);
      }, /positive safe integer/);
    }
    writeFileSync(path, "SELECT '" + 'x'.repeat(100_000 - "SELECT '';".length) + "';\n");
    const chunks = [];
    for await (const file of sqlImportChunks(path, 100_000)) chunks.push(readFileSync(file));
    assert.deepEqual(Buffer.concat(chunks), readFileSync(path));
    assert.equal(chunks.length, 1);
    assert.equal(chunks[0].length, 100_001); // SQL limit plus producer LF.
  } finally {
    rmSync(directory, { recursive: true });
  }
});

test('held native deadline preserves the in-use chunk until the platform reaps and cleans its district', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'tos-sql-held-deadline-'));
  const path = join(directory, 'input.sql');
  try {
    writeFileSync(path, 'SELECT 1;\nSELECT 2;\n');
    const chunks = sqlImportChunks(path, 10, 1);
    const first = await chunks.next();
    assert.equal(readFileSync(first.value, 'utf8'), 'SELECT 1;\n');
    await new Promise(done => setTimeout(done, 1500));
    // Timeout cannot unlink bytes while an external upload is still opening
    // them. Native custody ends; the platform owns this exact chunk district.
    assert.equal(readFileSync(first.value, 'utf8'), 'SELECT 1;\n');
    assert.deepEqual(readdirSync(dirname(first.value)), [basename(first.value)]);
    await assert.rejects(chunks.next(), /deadline exceeded/);
    assert.deepEqual(readdirSync(directory), ['input.sql']);
  } finally {
    rmSync(directory, { recursive: true });
  }
});

test('host cancellation escalates the exact TERM-resistant child and closes within one cleanup deadline', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'tos-sql-cancel-'));
  const path = join(directory, 'input.sql');
  const standin = join(directory, 'term-resistant.mjs');
  const previousBinary = process.env.TOS_ACCESS_BIN;
  try {
    writeFileSync(path, 'SELECT 1;\n');
    writeFileSync(standin, `#!/usr/bin/env node
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
const args = process.argv.slice(2);
const directory = args[args.indexOf('--directory') + 1];
process.on('SIGTERM', () => {});
process.stdin.resume();
process.stdin.once('data', () => {
  writeFileSync(join(directory, 'part-0.sql'), 'SELECT 1;\\n');
  process.stdout.write(JSON.stringify({kind:'chunk',file:'part-0.sql'}) + '\\n');
});
setInterval(() => {}, 1000);
`, {mode: 0o700});
    process.env.TOS_ACCESS_BIN = standin;
    const chunks = sqlImportChunks(path, 10, 30);
    const first = await chunks.next();
    assert.equal(readFileSync(first.value, 'utf8'), 'SELECT 1;\n');
    const started = performance.now();
    await chunks.return();
    assert.ok(performance.now() - started < 6000, 'cleanup exceeded its bounded deadline');
    assert.deepEqual(readdirSync(directory).sort(), ['input.sql', 'term-resistant.mjs']);
  } finally {
    if (previousBinary === undefined) delete process.env.TOS_ACCESS_BIN;
    else process.env.TOS_ACCESS_BIN = previousBinary;
    rmSync(directory, {recursive:true});
  }
});

test('SQL imports reject producer file replacement while the consumer is suspended', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'tos-sql-replacement-'));
  const path = join(directory, 'input.sql');
  try {
    writeFileSync(path, 'SELECT 1;\nSELECT 2;\n');
    const chunks = sqlImportChunks(path, 10);
    const first = await chunks.next();
    assert.equal(readFileSync(first.value, 'utf8'), 'SELECT 1;\n');
    const replacement = join(directory, 'replacement.sql');
    writeFileSync(replacement, 'SELECT 3;\nSELECT 4;\n');
    renameSync(replacement, path);
    await assert.rejects(chunks.next(), /source changed between chunks/);
    assert.deepEqual(readdirSync(directory), ['input.sql']);
  } finally {
    rmSync(directory, { recursive: true });
  }
});

test("D1 deploy skips a source revision that is already current", () => {
  assert.deepEqual(syncDecision("same", "same", 64_197), {
    required: false,
    reason: "revision-match",
  });
});

test("delta import is selected only for its exact serving baseline", () => {
  const manifest = { data_revision: "new", counts: { sql_statements: 100000,
    delta: { available: true, base_revision: "old", target_revision: "new", sql_statements: 85 } } };
  assert.equal(syncFile(manifest, "old").mode, "delta");
  assert.equal(syncFile(manifest, "another").mode, "full");
  assert.equal(syncFile(manifest, null).mode, "full");
});

test("D1 deploy admits a changed revision and supports an optional safety ceiling", () => {
  assert.deepEqual(syncDecision("new", "old", 64_197), {
    required: true,
    reason: "revision-changed",
  });
  assert.throws(() => syncDecision("new", "old", 75_001, 75_000), /safety ceiling/);
});

test("D1 revision parser accepts Wrangler JSON without using deployment metadata", () => {
  const rows = resultRows(JSON.stringify([{ success: true, results: [{ json: "{\"sha256\":\"abc\"}" }] }]));
  assert.equal(revisionFromRows(rows), "abc");
  assert.match(REVISION_QUERY, /GROUP_CONCAT\(json_chunk/);
  assert.match(REVISION_QUERY, /ORDER BY part/);
  assert.equal(revisionQueryForColumns(["key", "part", "json_chunk"]), REVISION_QUERY);
  assert.equal(revisionQueryForColumns(["key", "json"]), LEGACY_REVISION_QUERY);
  assert.equal(revisionQueryForColumns(["key"]), null);
});

test('sync policy preserves lazy full fallback, opaque revision identity and refusal coercion', () => {
  const reads = [];
  let calls = 0;
  const manifest = {get counts() {
    reads.push('counts');
    return ++calls === 1 ? {delta: {
      get available() {reads.push('available'); return true;},
      get base_revision() {reads.push('base'); return 'other';},
      get target_revision() {throw new Error('short-circuited target');},
    }} : {sql_statements: 42};
  }};
  assert.deepEqual(syncFile(manifest, 'current'), {file: 'runtime/read-model.sql', statements: 42, mode: 'full'});
  assert.deepEqual(reads, ['counts', 'available', 'base', 'counts']);
  const revision = {};
  assert.deepEqual(syncDecision(revision, revision, 1), {required: false, reason: 'revision-match'});
  assert.deepEqual(syncDecision('new', Symbol('old'), 1), {required: true, reason: 'revision-changed'});
  assert.throws(() => syncDecision('same', 'same', 0), /statement count/);
  assert.throws(() => syncDecision('same', 'same', 1, Symbol('ceiling')), TypeError);
  assert.throws(() => syncDecision('same', 'same', Number.MAX_SAFE_INTEGER + 1), /statement count/);
});

test('D1 revision policy preserves refusal order and retained native callbacks', () => {
  assert.throws(() => resultRows('{}'), /revision query did not succeed/);
  assert.throws(() => resultRows('[{"success":false,"results":null}]'), /revision query did not succeed/);
  assert.equal(revisionFromRows([{json: 42}]), null);
  assert.equal(revisionFromRows([{json: '{"sha256":""}'}]), null);
  assert.throws(() => revisionFromRows([{json: '{'}]), SyntaxError);
  assert.equal(revisionQueryForColumns(['json_chunk', 'json']), LEGACY_REVISION_QUERY);
  let successCallback, resultsCallback;
  const some = Array.prototype.some, flatMap = Array.prototype.flatMap;
  try {
    Array.prototype.some = function(callback) { successCallback = callback; return 0; };
    Array.prototype.flatMap = function(callback) { resultsCallback = callback; return ['opaque']; };
    assert.deepEqual(resultRows('[{"success":true}]'), ['opaque']);
  } finally { Array.prototype.some = some; Array.prototype.flatMap = flatMap; }
  assert.equal(successCallback({success: true}), false);
  assert.equal(successCallback(null), true);
  let reads = 0;
  const opaque = {};
  assert.equal(resultsCallback({get results() { return ++reads === 1 ? [] : opaque; }}), opaque);
  assert.equal(reads, 2);
  assert.deepEqual(resultsCallback({results: 'not an array'}), []);
  const originalParse = JSON.parse;
  let digestReads = 0;
  try {
    JSON.parse = () => ({get sha256() { return ++digestReads < 3 ? 'admitted' : opaque; }});
    assert.equal(revisionFromRows([{json: 'source'}]), opaque);
    assert.equal(digestReads, 3);
  } finally { JSON.parse = originalParse; }
});
