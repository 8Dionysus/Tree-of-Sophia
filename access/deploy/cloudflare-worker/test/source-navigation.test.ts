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

test("sourceDossier accepts every bibliographic carrier and preserves the selected route", () => {
  for (const [objectId, kind] of [["work", "work"], ["expression", "expression"], ["link", "link"]] as const) {
    const packet = sourceDossier(navigation, objectId, 300);
    assert.equal((packet.object as Record<string, unknown>).node_kind, kind);
    assert.equal((packet.tree_paths as Array<Record<string, unknown>>)[0]!.node_ids instanceof Array, true);
    assert.equal((packet.tree_paths as Array<Record<string, unknown>>)[0]!.node_ids.slice(-1)[0], objectId);
  }
});

function sharedFileRightsNavigation(rightsBPositive = false): Item {
  const fileId = "tos.file.sha256.shared";
  const itemA = "tos.item.copy.a";
  const itemB = "tos.item.copy.b";
  const manifestA = "ToS/source-witnesses/fixture/copy-a/item.manifest.json";
  const manifestB = "ToS/source-witnesses/fixture/copy-b/item.manifest.json";
  const rightsA = "ToS/source-witnesses/fixture/copy-a/rights.json";
  const rightsB = "ToS/source-witnesses/fixture/copy-b/rights.json";
  const membership = (itemId: string, manifestRef: string, rightsRef: string, basename: string) => ({
    edge_id: `${itemId}:${fileId}`,
    from_id: itemId,
    predicate_id: "has_file",
    to_id: fileId,
    edge_kind: "authored_item_manifest",
    source_refs: [manifestRef],
    properties: { item_file_contexts: [{
      manifest_ref: manifestRef,
      acquisition_event_ref: `tos.event.acquisition.${itemId}`,
      rights_ref: rightsRef,
      payload_entries: [{ relative_path: `payload/${basename}`, original_basename: basename, fixity_verified_at: "2026-09-22T10:00:00Z", container_member: false }],
    }] },
  });
  return {
    authority_boundary: "source-owned fixture navigation",
    nodes: [
      { node_id: fileId, node_kind: "file", source_ref: manifestA },
      { node_id: itemA, node_kind: "item", source_ref: manifestA },
      { node_id: itemB, node_kind: "item", source_ref: manifestB },
    ],
    edges: [membership(itemA, manifestA, rightsA, "a.bin"), membership(itemB, manifestB, rightsB, "b.bin")],
    rights: [
      { rights_id: "rights-a", source_ref: rightsA, scope_refs: [itemA, fileId], assessment_status: "public_domain_reviewed", redistribution_posture: "authorized", review_status: "accepted" },
      { rights_id: "rights-b", source_ref: rightsB, scope_refs: [itemB, fileId], assessment_status: rightsBPositive ? "licensed" : "copyright_undetermined", redistribution_posture: rightsBPositive ? "authorized" : "not_authorized", review_status: rightsBPositive ? "accepted" : "not_reviewed" },
    ],
  };
}

test("shared File rights remain attached to exact Item memberships", () => {
  const navigation = sharedFileRightsNavigation();
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  const fileSummary = file.agent_summary as Item;
  assert.equal(fileSummary.can_conclude_legal_openness, false);
  assert.equal(fileSummary.rights_posture, "membership_scoped_review_required");
  assert.deepEqual(fileSummary.rights_scope_refs, ["tos.file.sha256.shared", "tos.item.copy.a", "tos.item.copy.b"]);
  assert.deepEqual((file.rights as Item[]).map((record) => record.rights_id), ["rights-a", "rights-b"]);

  const itemA = sourceDossier(navigation, "tos.item.copy.a", 20);
  assert.equal((itemA.agent_summary as Item).can_conclude_legal_openness, true);
  assert.deepEqual((itemA.rights as Item[]).map((record) => record.rights_id), ["rights-a"]);
  const itemB = sourceDossier(navigation, "tos.item.copy.b", 20);
  assert.equal((itemB.agent_summary as Item).can_conclude_legal_openness, false);
  assert.deepEqual((itemB.rights as Item[]).map((record) => record.rights_id), ["rights-b"]);

  const allPositive = sourceDossier(sharedFileRightsNavigation(true), "tos.file.sha256.shared", 20);
  assert.equal((allPositive.agent_summary as Item).can_conclude_legal_openness, true);
  const truncated = sourceDossier(sharedFileRightsNavigation(true), "tos.file.sha256.shared", 2);
  assert.equal(truncated.truncated, true);
  assert.equal((truncated.agent_summary as Item).can_conclude_legal_openness, false);
});

test("legacy single-Item File dossiers remain compatible while shared unbound Files fail closed", () => {
  const single = sharedFileRightsNavigation();
  single.nodes = (single.nodes as Item[]).filter((node) => node.node_id !== "tos.item.copy.b");
  const withoutRightsRef = (edge: Item): Item => {
    const properties = edge.properties as Item;
    const contexts = properties.item_file_contexts as Item[];
    return {
      ...edge,
      properties: { item_file_contexts: contexts.map(({ rights_ref: _rightsRef, ...context }) => context) },
    };
  };
  single.edges = (single.edges as Item[]).slice(0, 1).map(withoutRightsRef);
  single.rights = (single.rights as Item[]).slice(0, 1);
  assert.equal((sourceDossier(single, "tos.file.sha256.shared", 20).agent_summary as Item).can_conclude_legal_openness, true);

  const sharedLegacy = sharedFileRightsNavigation();
  sharedLegacy.edges = (sharedLegacy.edges as Item[]).map(withoutRightsRef);
  const file = sourceDossier(sharedLegacy, "tos.file.sha256.shared", 20);
  assert.equal((file.agent_summary as Item).can_conclude_legal_openness, false);
  assert.deepEqual(file.rights, []);
});
