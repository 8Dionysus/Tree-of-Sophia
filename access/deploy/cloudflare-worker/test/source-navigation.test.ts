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
    { rights_id: "r1", assessment_kind: "aggregate", scope_refs: ["work"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", source_ref: "rights.json" },
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

function sharedFileRightsNavigation(rightsBPositive = false, legacyRights = false): Item {
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
      ...(legacyRights ? {} : { rights_ref: rightsRef }),
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
      { rights_id: "rights-a", source_ref: rightsA, scope_refs: [itemA, fileId], assessment_status: "public_domain_reviewed", redistribution_posture: "authorized", review_status: "accepted", ...(legacyRights ? {} : { assessment_kind: "aggregate" }) },
      { rights_id: "rights-b", source_ref: rightsB, scope_refs: [itemB, fileId], assessment_status: rightsBPositive ? "licensed" : "copyright_undetermined", redistribution_posture: rightsBPositive ? "authorized" : "not_authorized", review_status: rightsBPositive ? "accepted" : "not_reviewed", ...(legacyRights ? {} : { assessment_kind: "aggregate" }) },
    ],
  };
}

function sharedWorkRightsNavigation(legacy = false): Item {
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
      ...(legacy ? {} : { rights_ref: rightsRef }),
      payload_entries: [{ relative_path: `payload/${basename}`, original_basename: basename }],
    }] },
  });
  return {
    authority_boundary: "source-owned fixture navigation",
    nodes: [
      { node_id: "work-a", node_kind: "work", source_ref: "work-a.json" },
      { node_id: "expression-a", node_kind: "expression", source_ref: "expression-a.json" },
      { node_id: "edition-a", node_kind: "edition", source_ref: "edition-a.json" },
      { node_id: "work-b", node_kind: "work", source_ref: "work-b.json" },
      { node_id: "expression-b", node_kind: "expression", source_ref: "expression-b.json" },
      { node_id: "edition-b", node_kind: "edition", source_ref: "edition-b.json" },
      { node_id: itemA, node_kind: "item", source_ref: manifestA },
      { node_id: itemB, node_kind: "item", source_ref: manifestB },
      { node_id: fileId, node_kind: "file", source_ref: manifestA },
    ],
    edges: [
      { edge_id: "a-work-expression", from_id: "work-a", to_id: "expression-a", predicate_id: "has_expression", edge_kind: "evidence_claim", source_refs: ["claim-a.jsonl"] },
      { edge_id: "a-expression-edition", from_id: "expression-a", to_id: "edition-a", predicate_id: "embodied_by", edge_kind: "evidence_claim", source_refs: ["claim-a.jsonl"] },
      { edge_id: "a-edition-item", from_id: "edition-a", to_id: itemA, predicate_id: "exemplified_by", edge_kind: "evidence_claim", source_refs: ["claim-a.jsonl"] },
      membership(itemA, manifestA, rightsA, "a.bin"),
      { edge_id: "b-work-expression", from_id: "work-b", to_id: "expression-b", predicate_id: "has_expression", edge_kind: "evidence_claim", source_refs: ["claim-b.jsonl"] },
      { edge_id: "b-expression-edition", from_id: "expression-b", to_id: "edition-b", predicate_id: "embodied_by", edge_kind: "evidence_claim", source_refs: ["claim-b.jsonl"] },
      { edge_id: "b-edition-item", from_id: "edition-b", to_id: itemB, predicate_id: "exemplified_by", edge_kind: "evidence_claim", source_refs: ["claim-b.jsonl"] },
      membership(itemB, manifestB, rightsB, "b.bin"),
    ],
    rights: [
      { rights_id: legacy ? "tos.rights.fixture.work-a" : "rights-work-a", source_ref: "ToS/source-witnesses/fixture/work-a/rights.json", scope_refs: ["work-a"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", ...(legacy ? {} : { assessment_kind: "aggregate" }) },
      { rights_id: legacy ? "tos.rights.fixture.copy-a" : "rights-a", source_ref: rightsA, scope_refs: [itemA, fileId], assessment_status: "public_domain_reviewed", redistribution_posture: "authorized", review_status: "accepted", ...(legacy ? {} : { assessment_kind: "aggregate" }) },
      { rights_id: legacy ? "tos.rights.fixture.copy-b" : "rights-b", source_ref: rightsB, scope_refs: [itemB, fileId], assessment_status: "copyright_undetermined", redistribution_posture: "not_authorized", review_status: "not_reviewed", ...(legacy ? {} : { assessment_kind: "aggregate" }) },
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

test("positive Item layer remains evidence but cannot lift a restrictive aggregate", () => {
  const navigation = sharedFileRightsNavigation();
  const rights = navigation.rights as Item[];
  rights[0]!.assessment_status = "copyright_undetermined";
  rights[0]!.redistribution_posture = "not_authorized";
  rights.push({
    rights_id: "tos.rights.fixture.copy-a.layer.ocr",
    assessment_kind: "layer",
    source_ref: "ToS/source-witnesses/fixture/copy-a/rights.json",
    scope_refs: ["tos.file.sha256.shared"],
    assessment_status: "licensed",
    redistribution_posture: "authorized",
    review_status: "accepted",
  });

  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  const summary = file.agent_summary as Item;
  assert.equal(summary.can_conclude_legal_openness, false);
  assert.equal(summary.rights_posture, "not_cleared");
  assert.deepEqual((file.rights as Item[]).map((record) => record.rights_id), [
    "rights-a", "rights-b", "tos.rights.fixture.copy-a.layer.ocr",
  ]);
});

test("legacy rights rows classify only one unambiguous aggregate per source", () => {
  const navigation = sharedFileRightsNavigation(false, true);
  navigation.nodes = (navigation.nodes as Item[]).filter((node) => node.node_id !== "tos.item.copy.b");
  navigation.edges = (navigation.edges as Item[]).slice(0, 1);
  const rights = navigation.rights as Item[];
  navigation.rights = [{
    ...rights[0]!,
    rights_id: "tos.rights.fixture.copy-a",
    assessment_status: "copyright_undetermined",
    redistribution_posture: "not_authorized",
  }, {
    rights_id: "tos.rights.fixture.copy-a.layer.ocr",
    source_ref: "ToS/source-witnesses/fixture/copy-a/rights.json",
    scope_refs: ["tos.item.copy.a", "tos.file.sha256.shared"],
    assessment_status: "licensed",
    redistribution_posture: "authorized",
    review_status: "accepted",
  }];
  const grouped = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((grouped.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((grouped.agent_summary as Item).rights_posture, "not_cleared");
  assert.equal((grouped.rights as Item[]).length, 2);

  navigation.rights = [{
    ...rights[0]!,
    rights_id: "tos.rights.fixture.layer.aggregate",
  }];
  const overlappingRootId = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((overlappingRootId.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((overlappingRootId.agent_summary as Item).rights_posture, "not_cleared");
  assert.ok(((overlappingRootId.agent_summary as Item).gaps as string[]).includes("no unambiguous aggregate rights assessment"));

  navigation.rights = [{
    ...rights[0]!,
    rights_id: "tos.rights.fixture.copy-a",
    assessment_status: "licensed",
    redistribution_posture: "authorized",
  }, {
    ...rights[0]!,
    rights_id: "tos.rights.fixture.copy-a.extra",
    assessment_status: "copyright_undetermined",
    redistribution_posture: "not_authorized",
  }];
  const multipleRoots = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((multipleRoots.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((multipleRoots.agent_summary as Item).rights_posture, "not_cleared");
  assert.ok(((multipleRoots.agent_summary as Item).gaps as string[]).includes("no unambiguous aggregate rights assessment"));
});

test("File-only rights rows follow an exact rights_ref and stay out of sibling Item dossiers", () => {
  const navigation = sharedFileRightsNavigation();
  const rights = navigation.rights as Item[];
  rights.push({
    rights_id: "rights-a-file-layer",
    assessment_kind: "layer",
    source_ref: "ToS/source-witnesses/fixture/copy-a/rights.json",
    scope_refs: ["tos.file.sha256.shared"],
    assessment_status: "copyright_undetermined",
    redistribution_posture: "not_authorized",
    review_status: "unreviewed",
  });

  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.deepEqual((file.rights as Item[]).map((record) => record.rights_id), [
    "rights-a", "rights-a-file-layer", "rights-b",
  ]);
  const itemB = sourceDossier(navigation, "tos.item.copy.b", 20);
  assert.deepEqual((itemB.rights as Item[]).map((record) => record.rights_id), ["rights-b"]);
  assert.equal((itemB.agent_summary as Item).can_conclude_legal_openness, false);
});

test("File-only candidate rights remain review-required when bound by exact Item context", () => {
  const navigation = sharedFileRightsNavigation();
  navigation.nodes = (navigation.nodes as Item[]).filter((node) => node.node_id !== "tos.item.copy.b");
  navigation.edges = (navigation.edges as Item[]).slice(0, 1);
  navigation.rights = [{
    rights_id: "rights-a-file-candidate",
    assessment_kind: "aggregate",
    source_ref: "ToS/source-witnesses/fixture/copy-a/rights.json",
    scope_refs: ["tos.file.sha256.shared"],
    assessment_status: "licensed",
    redistribution_posture: "authorized",
    review_status: "not_reviewed",
  }];
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  const summary = file.agent_summary as Item;
  assert.deepEqual((file.rights as Item[]).map((record) => record.rights_id), ["rights-a-file-candidate"]);
  assert.equal(summary.rights_posture, "candidate_requires_human_review");
  assert.equal(summary.can_conclude_legal_openness, false);
});

test("File aggregate requires a context for every Item-manifest edge source", () => {
  const navigation = sharedFileRightsNavigation(true);
  const edge = (navigation.edges as Item[])[0]!;
  edge.source_refs = [
    "ToS/source-witnesses/fixture/copy-a/item.manifest.json",
    "ToS/source-witnesses/fixture/copy-a/superseded-item.manifest.json",
  ];
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  const summary = file.agent_summary as Item;
  assert.equal(summary.can_conclude_legal_openness, false);
  assert.equal(summary.rights_posture, "membership_scoped_review_required");
});

test("File aggregate rejects duplicate manifest contexts", () => {
  const navigation = sharedFileRightsNavigation(true);
  const edge = (navigation.edges as Item[])[0]!;
  const properties = edge.properties as Item;
  const contexts = properties.item_file_contexts as Item[];
  edge.properties = { item_file_contexts: [...contexts, { ...contexts[0]! }] };
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((file.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((file.agent_summary as Item).rights_posture, "membership_scoped_review_required");
});

test("legacy single-Item File dossiers remain compatible while shared unbound Files fail closed", () => {
  const single = sharedFileRightsNavigation(false, true);
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
  single.rights = [{
    ...(single.rights as Item[])[0]!,
    rights_id: "tos.rights.fixture.copy-a",
  }];
  assert.equal((sourceDossier(single, "tos.file.sha256.shared", 20).agent_summary as Item).can_conclude_legal_openness, true);
  const singleRight = (single.rights as Item[])[0]!;
  single.rights = [singleRight, {
    ...singleRight,
    rights_id: "tos.rights.fixture.alternate",
    source_ref: "ToS/source-witnesses/fixture/alternate-rights.json",
    redistribution_posture: "not_authorized",
  }];
  const conflictingLegacy = sourceDossier(single, "tos.file.sha256.shared", 20);
  assert.equal((conflictingLegacy.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((conflictingLegacy.agent_summary as Item).rights_posture, "membership_scoped_review_required");
  assert.deepEqual(conflictingLegacy.rights, []);

  const sharedLegacy = sharedFileRightsNavigation();
  sharedLegacy.edges = (sharedLegacy.edges as Item[]).map(withoutRightsRef);
  const file = sourceDossier(sharedLegacy, "tos.file.sha256.shared", 20);
  assert.equal((file.agent_summary as Item).can_conclude_legal_openness, false);
  assert.deepEqual(file.rights, []);
});

test("legacy single-Item File fallback rejects File-only assessment scopes", () => {
  const navigation = sharedFileRightsNavigation();
  navigation.nodes = (navigation.nodes as Item[]).filter((node) => node.node_id !== "tos.item.copy.b");
  const edge = (navigation.edges as Item[])[0]!;
  const properties = edge.properties as Item;
  const contexts = properties.item_file_contexts as Item[];
  navigation.edges = [{
    ...edge,
    properties: { item_file_contexts: contexts.map(({ rights_ref: _rightsRef, ...context }) => context) },
  }];
  navigation.rights = [{
    rights_id: "legacy-file-only",
    source_ref: "ToS/source-witnesses/fixture/copy-a/rights.json",
    scope_refs: ["tos.file.sha256.shared"],
    assessment_status: "licensed",
    redistribution_posture: "authorized",
    review_status: "accepted",
  }];
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((file.agent_summary as Item).can_conclude_legal_openness, false);
  assert.equal((file.agent_summary as Item).rights_posture, "membership_scoped_review_required");
  assert.deepEqual(file.rights, []);
});

test("legacy single-manifest edge without context preserves unique Item-scoped rights", () => {
  const navigation = sharedFileRightsNavigation();
  navigation.nodes = (navigation.nodes as Item[]).filter((node) => node.node_id !== "tos.item.copy.b");
  const edge = (navigation.edges as Item[])[0]!;
  navigation.edges = [{ ...edge, properties: undefined }];
  navigation.rights = (navigation.rights as Item[]).slice(0, 1);
  const file = sourceDossier(navigation, "tos.file.sha256.shared", 20);
  assert.equal((file.agent_summary as Item).can_conclude_legal_openness, true);
  assert.deepEqual((file.rights as Item[]).map((record) => record.rights_id), ["rights-a"]);
});

test("Work, Expression, and Edition dossiers expose File rights only through their reachable Item memberships", () => {
  const navigation = sharedWorkRightsNavigation();
  for (const ancestorId of ["work-a", "expression-a", "edition-a"]) {
    const dossier = sourceDossier(navigation, ancestorId, 20);
    assert.deepEqual((dossier.rights as Item[]).map((record) => record.rights_id), ["rights-a", "rights-work-a"]);
    assert.equal((dossier.source_refs as string[]).includes("ToS/source-witnesses/fixture/copy-b/rights.json"), false);
  }
});

test("ancestor legacy File rights remain only for a unique single-owner source", () => {
  const navigation = sharedWorkRightsNavigation(true);
  const edges = navigation.edges as Item[];
  navigation.edges = edges.filter((edge) => edge.edge_id !== "tos.item.copy.b:tos.file.sha256.shared");
  navigation.rights = (navigation.rights as Item[]).slice(0, 2);
  const singleOwner = sourceDossier(navigation, "work-a", 20);
  assert.deepEqual((singleOwner.rights as Item[]).map((record) => record.rights_id), ["tos.rights.fixture.copy-a", "tos.rights.fixture.work-a"]);

  navigation.rights = [
    ...(navigation.rights as Item[]),
    {
      ...((navigation.rights as Item[])[1] as Item),
      rights_id: "tos.rights.fixture.copy-a.conflict",
      source_ref: "ToS/source-witnesses/fixture/copy-a/alternate-rights.json",
      redistribution_posture: "not_authorized",
    },
  ];
  const ambiguous = sourceDossier(navigation, "work-a", 20);
  assert.deepEqual((ambiguous.rights as Item[]).map((record) => record.rights_id), ["tos.rights.fixture.work-a"]);

  const sharedLegacy = sharedWorkRightsNavigation(true);
  const shared = sourceDossier(sharedLegacy, "work-a", 20);
  assert.deepEqual((shared.rights as Item[]).map((record) => record.rights_id), ["tos.rights.fixture.work-a"]);
  assert.equal((shared.source_refs as string[]).includes("ToS/source-witnesses/fixture/copy-b/rights.json"), false);
});

test("truncated ancestor dossiers do not import rights from an untraversed shared File", () => {
  const dossier = sourceDossier(sharedWorkRightsNavigation(), "work-a", 4);
  assert.equal(dossier.truncated, true);
  assert.deepEqual((dossier.rights as Item[]).map((record) => record.rights_id), ["rights-a", "rights-work-a"]);
  assert.equal((dossier.source_refs as string[]).includes("ToS/source-witnesses/fixture/copy-b/rights.json"), false);
});
