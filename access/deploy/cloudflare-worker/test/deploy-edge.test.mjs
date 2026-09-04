import assert from "node:assert/strict";
import test from "node:test";

import { resultRows, revisionFromRows, syncDecision } from "../scripts/deploy_edge.mjs";

test("D1 deploy skips a source revision that is already current", () => {
  assert.deepEqual(syncDecision("same", "same", 64_197), {
    required: false,
    reason: "revision-match",
  });
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
});
