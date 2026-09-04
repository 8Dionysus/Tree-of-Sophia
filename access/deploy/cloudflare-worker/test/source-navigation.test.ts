import assert from "node:assert/strict";
import test from "node:test";

import { sourceDescend, sourceDossier } from "../src/source-navigation.ts";

const navigation = {
  authority_boundary: "source-owned navigation only",
  nodes: [
    { node_id: "era", node_kind: "era", source_ref: "era.json" },
    { node_id: "planting", node_kind: "source_planting", source_ref: "planting.json" },
    { node_id: "work", node_kind: "work", source_ref: "work.json" },
    { node_id: "expression", node_kind: "expression", source_ref: "expression.json" },
    { node_id: "link", node_kind: "link", source_ref: "link.json", properties: { access_status: "open_download" } },
  ],
  edges: [
    { edge_id: "e1", from_id: "era", to_id: "planting", predicate_id: "contains", edge_kind: "authored_branch_hierarchy", source_refs: ["era.json"] },
    { edge_id: "e2", from_id: "planting", to_id: "work", predicate_id: "references_source_witness", edge_kind: "authored_source_planting", source_refs: ["planting.json"] },
    { edge_id: "e3", from_id: "work", to_id: "expression", predicate_id: "has_expression", edge_kind: "evidence_claim", source_refs: ["claims.jsonl"] },
    { edge_id: "e4", from_id: "work", to_id: "link", predicate_id: "downloadable_at", edge_kind: "evidence_claim", source_refs: ["links.jsonl"] },
  ],
  rights: [
    { rights_id: "r1", scope_refs: ["work"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", source_ref: "rights.json" },
  ],
};

test("sourceDescend preserves bounded deterministic traversal", () => {
  const packet = sourceDescend(navigation, "era", 8, 3);
  assert.deepEqual(packet.counts, { nodes: 3, edges: 2 });
  assert.equal(packet.truncated, true);
  assert.deepEqual((packet.nodes as Array<Record<string, unknown>>).map((node) => node.node_id), ["era", "planting", "work"]);
});

test("sourceDossier joins tree route, evidence links, and reviewed rights", () => {
  const packet = sourceDossier(navigation, "work", 300);
  assert.deepEqual(packet.agent_summary, {
    technical_access: "downloadable",
    rights_posture: "reviewed_reuse_route",
    human_review_required: false,
    can_conclude_legal_openness: true,
    availability_is_license: false,
    rights_scope_refs: ["work"],
    gaps: [],
  });
  assert.deepEqual((packet.tree_paths as Array<Record<string, unknown>>)[0].node_ids, ["era", "planting", "work"]);
  assert.deepEqual((packet.relations as Array<Record<string, unknown>>).map((edge) => edge.edge_id), ["e1", "e2", "e3", "e4"]);
});
