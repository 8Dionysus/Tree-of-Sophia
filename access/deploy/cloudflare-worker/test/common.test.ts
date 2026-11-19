import assert from "node:assert/strict";
import test from "node:test";
import {listParam} from "../src/common.ts";

test("query lists preserve order and repetitions", () => {
  assert.deepEqual(listParam(new URLSearchParams("layers=b,a,,b"), "layers"), ["b", "a", "b"]);
});
