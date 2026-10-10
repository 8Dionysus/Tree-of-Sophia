import assert from "node:assert/strict";
import test from "node:test";

import {listParam} from "../src/common.ts";
import {classicBoundedClusters as boundedClusters,classicBoundedGraph as boundedGraph,classicSourceRefs as sourceRefs} from "../src/worker-classic.ts";

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


import {workerClassicRules,PhysicalKeys,classicBoundedGraph,classicBoundedClusters,classicNeedle} from "../src/worker-classic.ts";

test("classic Rust profile keeps opaque source objects and UTF16 identity",()=>{
  const id="\ud800",nodes=[{node_id:id},{node_id:"b"},{node_id:"outside"}],edges=[{edge_id:"e",from_id:id,to_id:"b"}];
  const bounded=classicBoundedGraph(nodes,edges,2);
  assert.equal(bounded.nodes[0],nodes[0]);assert.equal(bounded.edges[0],edges[0]);
  const clusters=[{member_node_ids:[id,"outside"],member_edge_ids:["e"],properties:{member_count:2,edge_count:1}}];
  assert.deepEqual(classicBoundedClusters(clusters,bounded.nodes,bounded.edges)[0]?.member_node_ids,[id]);
  const keys=new PhysicalKeys();assert.notEqual(keys.key(id),keys.key("\ufffd"));
});

test("classic Rust path keeps breadth-level frontier bound and direction semantics",()=>{
  const rules=new (workerClassicRules().WorkerPath)(0,7000,2,2,1,new Uint32Array());
  try {
    for(let id=1;id<=6000;id+=1)rules.observe_edge(id,0,id);
    rules.finish_level();
    assert.equal(rules.enqueued(),5001);assert.equal(rules.max_frontier(),5000);assert.equal(rules.truncated(),true);
    assert.equal(rules.current_ids().length,5000);
    // This destination can be traversed without a separate node-row admission.
    rules.observe_edge(9000,1,7000);rules.finish_level();
    assert.equal(rules.current_ids().length,1);assert.equal(rules.active(),true);
    rules.finish_level();assert.equal(rules.path_count(),1);
    assert.deepEqual(Array.from(rules.path_nodes(0)),[0,1,7000]);
    assert.deepEqual(Array.from(rules.path_directions(0)),[0,0]);
  } finally{rules.free();}
});

test("classic Rust neighborhood follows host ord tie order and first incident edge",()=>{
  const rules=new (workerClassicRules().WorkerNeighborhood)(0,1,2);
  try{
    rules.observe_edge(4,0,2,0);rules.observe_edge(5,0,1,1);rules.observe_edge(6,0,1,2);
    rules.finish_level(Uint32Array.from([1,2]));
    assert.deepEqual(Array.from(rules.neighbors()),[1,2]);
    assert.deepEqual(Array.from(rules.edges()),[1,0]);
    assert.equal(rules.active(),false);
  }finally{rules.free();}
});


test("classic query needle requests ECMAScript Unicode intrinsics",()=>{
  assert.equal(classicNeedle("\ufeff I \ufeff"),"i");
  assert.equal(classicNeedle("\u001cI"),"\u001ci");
});

test("classic Evidence defaults and challenge reserve remain projection-only",()=>{
  const runtime=workerClassicRules();
  const fallback=JSON.parse(runtime.WorkerEvidence.fallback(runtime.WorkerEvidence.canon_layer("canon")));
  assert.equal(fallback.conclusion.canon_membership,true);assert.equal(fallback.conclusion.can_conclude,false);
  const rules=new runtime.WorkerEpistemic(2,true);
  try{
    rules.observe_relation(0,0,false);rules.observe_relation(1,1,true);rules.observe_relation(2,2,true);rules.finish_relations();
    assert.deepEqual(Array.from(rules.challenge_rows()),[1]);assert.deepEqual(Array.from(rules.context_rows()),[0]);
    assert.equal(rules.available(),2);assert.equal(rules.challenge_state(),"projected_signals_truncated");
  }finally{rules.free();}
});
