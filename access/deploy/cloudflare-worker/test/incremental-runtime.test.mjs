import assert from "node:assert/strict";
import test from "node:test";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";

test("D1 publishes a staged delta atomically and rejects an incomplete stage", async () => {
  const fixture = JSON.parse(execFileSync("python", ["-c", `
import json, tempfile
from pathlib import Path
from access.tests.test_incremental_runtime import IncrementalRuntimeTests
t = IncrementalRuntimeTests()
db = t.database()
initial = [s for s in db.iterdump() if s not in ('BEGIN TRANSACTION;', 'COMMIT;')]
db.close()
with tempfile.TemporaryDirectory() as root:
    p = Path(root)/'delta.sql'
    base, _ = t.build(p, 'a'*64, [('one','old'), ('two','stable'), ('three','removed')])
    _, counts = t.build(p, 'b'*64, [('one','new'), ('two','stable')], base)
    print(json.dumps({'initial': initial, 'delta': p.read_text().splitlines(), 'counts': counts}))
`], { cwd: fileURLToPath(new URL("../../../../", import.meta.url)), encoding: "utf8" }));
  const mf = new Miniflare(convertV4MiniflareOptions({ modules: true, script: "export default {fetch(){return new Response('fixture')}}", d1Databases: ["DB"] }));
  try {
    const db = await mf.getD1Database("DB");
    await db.batch(fixture.initial.map((sql) => db.prepare(sql)));
    const publication = fixture.delta.findIndex((sql) => sql.startsWith("INSERT OR REPLACE INTO tos_delta_publications"));
    await db.batch(fixture.delta.slice(0, publication).map((sql) => db.prepare(sql)));
    assert.equal(await db.prepare("SELECT value FROM knowledge_nodes WHERE id='one'").first("value"), "old");
    const stage = "tos_delta_" + "b".repeat(16) + "_knowledge_nodes";
    await db.prepare(`DELETE FROM ${stage}`).run();
    await assert.rejects(db.prepare(fixture.delta[publication]).run(), /incomplete delta staging/);
    assert.equal(await db.prepare("SELECT value FROM knowledge_nodes WHERE id='one'").first("value"), "old");
    // Full replay restores staging, publishes once, and removes staging tables.
    await db.batch(fixture.delta.map((sql) => db.prepare(sql)));
    assert.equal(await db.prepare("SELECT value FROM knowledge_nodes WHERE id='one'").first("value"), "new");
    await db.batch(fixture.delta.map((sql) => db.prepare(sql)));
    assert.equal(await db.prepare("SELECT count(*) AS n FROM knowledge_nodes").first("n"), 2);
  } finally {
    await mf.dispose();
  }
});
