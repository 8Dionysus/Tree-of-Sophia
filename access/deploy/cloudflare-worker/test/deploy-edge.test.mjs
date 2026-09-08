import assert from "node:assert/strict";
import test from "node:test";
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
    for (const maximum of [0, -1, 1.5, Infinity]) {
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
