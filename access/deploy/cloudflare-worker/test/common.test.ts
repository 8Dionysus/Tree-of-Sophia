import assert from "node:assert/strict";
import test from "node:test";

import { boundedClusters, boundedGraph, listParam, sourceRefs } from "../src/common.ts";

test("boundedGraph preserves projection order and keeps endpoint-closed edges", () => {
  const nodes = [
    { node_id: "a" },
    { node_id: "b" },
    { node_id: "c" },
    { node_id: "d" },
  ];
  const edges = [
    { edge_id: "e1", from_id: "a", to_id: "b" },
    { edge_id: "e2", from_id: "c", to_id: "d" },
    { edge_id: "e3", from_id: "b", to_id: "a" },
  ];

  assert.deepEqual(boundedGraph(nodes, edges, 2), {
    nodes: [{ node_id: "a" }, { node_id: "b" }],
    edges: [
      { edge_id: "e1", from_id: "a", to_id: "b" },
      { edge_id: "e3", from_id: "b", to_id: "a" },
    ],
  });
});

test("boundedClusters reports original membership while returning only visible members", () => {
  const clusters = [
    {
      cluster_id: "c1",
      member_node_ids: ["a", "missing"],
      member_edge_ids: ["e1", "missing"],
      properties: { member_count: 2, edge_count: 2, stable: true },
    },
  ];
  const result = boundedClusters(
    clusters,
    [{ node_id: "a" }],
    [{ edge_id: "e1", from_id: "a", to_id: "a" }],
  );

  assert.deepEqual(result[0], {
    cluster_id: "c1",
    member_node_ids: ["a"],
    member_edge_ids: ["e1"],
    available_member_node_count: 2,
    available_member_edge_count: 2,
    properties: { member_count: 1, edge_count: 1, stable: true },
  });
});

test("query lists and source references are deterministic", () => {
  const search = new URLSearchParams("layers=b,a,,b");
  assert.deepEqual(listParam(search, "layers"), ["b", "a", "b"]);
  assert.deepEqual(
    sourceRefs([
      { source_ref: "z" },
      { source_refs: ["a", "z"] },
      { source_ref: "b" },
    ]),
    ["a", "b", "z"],
  );
});
