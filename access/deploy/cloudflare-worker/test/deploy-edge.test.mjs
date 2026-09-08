import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

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
  const sql = "INSERT INTO t VALUES ('София');\nCREATE TRIGGER p AFTER INSERT ON t BEGIN SELECT 1; SELECT 2; END;\nSELECT 3;\n";
  try {
    writeFileSync(path, sql);
    const chunks = [];
    for await (const file of sqlImportChunks(path, 40)) chunks.push(readFileSync(file, 'utf8'));
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
