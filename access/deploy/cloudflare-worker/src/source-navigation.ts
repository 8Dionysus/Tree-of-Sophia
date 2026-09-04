type Item = Record<string, unknown>;

export class SourceNavigationError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

function objectArray(value: unknown): Item[] {
  return Array.isArray(value)
    ? value.filter((item): item is Item => Boolean(item) && typeof item === "object" && !Array.isArray(item))
    : [];
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.length > 0)
    : [];
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

const BIBLIOGRAPHIC_PREDICATES = new Set(["has_expression", "embodied_by", "exemplified_by"]);
const LINK_PREDICATES = new Set(["described_by", "metadata_at", "downloadable_at", "rights_statement_at"]);
const CHAIN_KINDS = [
  "branch",
  "era",
  "region",
  "tradition",
  "source_planting",
  "work",
  "expression",
  "edition",
  "item",
  "file",
  "link",
] as const;

function itemObject(value: unknown): Item {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

function sortedById(items: Item[], key: string): Item[] {
  return [...items].sort((left, right) => stringValue(left[key]).localeCompare(stringValue(right[key])));
}

function intersects(values: unknown, selected: Set<string>): boolean {
  return stringArray(values).some((value) => selected.has(value));
}

export function sourceDescend(navigation: Item, nodeId: string, maxDepth: number, limit: number): Item {
  const nodes = objectArray(navigation.nodes);
  const nodesById = new Map(nodes.map((node) => [stringValue(node.node_id), node]));
  if (!nodesById.has(nodeId)) throw new SourceNavigationError(404, `unknown ToS source-navigation node: ${nodeId}`);

  const outgoing = new Map<string, Item[]>();
  for (const edge of objectArray(navigation.edges)) {
    const fromId = stringValue(edge.from_id);
    if (!fromId) continue;
    const bucket = outgoing.get(fromId) ?? [];
    bucket.push(edge);
    outgoing.set(fromId, bucket);
  }
  for (const bucket of outgoing.values()) bucket.sort((a, b) => stringValue(a.edge_id).localeCompare(stringValue(b.edge_id)));

  const queue: Array<[string, number]> = [[nodeId, 0]];
  const depths = new Map<string, number>([[nodeId, 0]]);
  const selectedEdges: Item[] = [];
  const selectedEdgeIds = new Set<string>();
  let truncated = false;
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const [current, depth] = queue[cursor]!;
    if (depth >= maxDepth) continue;
    for (const edge of outgoing.get(current) ?? []) {
      const target = stringValue(edge.to_id);
      if (!nodesById.has(target)) continue;
      if (!depths.has(target) && depths.size >= limit) {
        truncated = true;
        continue;
      }
      const edgeId = stringValue(edge.edge_id);
      if (!selectedEdgeIds.has(edgeId)) {
        selectedEdgeIds.add(edgeId);
        selectedEdges.push(edge);
      }
      if (!depths.has(target)) {
        depths.set(target, depth + 1);
        queue.push([target, depth + 1]);
      }
    }
  }
  const selectedNodes = [...depths.entries()]
    .sort(([leftId, leftDepth], [rightId, rightDepth]) => leftDepth - rightDepth || leftId.localeCompare(rightId))
    .map(([id, depth]) => ({ ...nodesById.get(id), depth }));
  return {
    schema: "tos_source_descent_v1",
    root_id: nodeId,
    max_depth: maxDepth,
    limit,
    truncated,
    counts: { nodes: selectedNodes.length, edges: selectedEdges.length },
    nodes: selectedNodes,
    edges: selectedEdges,
    authority_note: navigation.authority_boundary,
  };
}

export function sourceDossier(navigation: Item, objectId: string, limit: number): Item {
  const nodes = objectArray(navigation.nodes);
  const nodesById = new Map(nodes.map((node) => [stringValue(node.node_id), node]));
  const selected = nodesById.get(objectId);
  if (!selected) throw new SourceNavigationError(404, `unknown ToS dossier object: ${objectId}`);
  const selectedKind = stringValue(selected.node_kind);
  if (selectedKind !== "work" && selectedKind !== "link") {
    throw new SourceNavigationError(400, "dossiers are currently available for Work and Link objects");
  }

  const allEdges = sortedById(objectArray(navigation.edges), "edge_id");
  const incoming = new Map<string, Item[]>();
  const semanticOutgoing = new Map<string, Item[]>();
  for (const edge of allEdges) {
    const left = stringValue(edge.from_id);
    const right = stringValue(edge.to_id);
    if (!left || !right) continue;
    const incomingBucket = incoming.get(right) ?? [];
    incomingBucket.push(edge);
    incoming.set(right, incomingBucket);
    const predicate = stringValue(edge.predicate_id);
    if (edge.edge_kind === "authored_item_manifest" || (
      edge.edge_kind === "evidence_claim" && (BIBLIOGRAPHIC_PREDICATES.has(predicate) || LINK_PREDICATES.has(predicate))
    )) {
      const outgoingBucket = semanticOutgoing.get(left) ?? [];
      outgoingBucket.push(edge);
      semanticOutgoing.set(left, outgoingBucket);
    }
  }

  const componentIds = new Set([objectId]);
  const componentEdges = new Map<string, Item>();
  let truncated = false;
  const admit = (nodeId: string): boolean => {
    if (componentIds.has(nodeId)) return true;
    if (!nodesById.has(nodeId)) return false;
    if (componentIds.size >= limit) {
      truncated = true;
      return false;
    }
    componentIds.add(nodeId);
    return true;
  };

  let forwardRoots = new Set<string>(selectedKind === "work" ? [objectId] : []);
  if (selectedKind === "link") {
    const lineageQueue = [objectId];
    const visitedLineage = new Set<string>();
    for (let cursor = 0; cursor < lineageQueue.length; cursor += 1) {
      const current = lineageQueue[cursor]!;
      if (visitedLineage.has(current)) continue;
      visitedLineage.add(current);
      const currentKind = stringValue(nodesById.get(current)?.node_kind);
      if (currentKind === "work") {
        forwardRoots.add(current);
        continue;
      }
      const allowed = currentKind === "link" ? LINK_PREDICATES : BIBLIOGRAPHIC_PREDICATES;
      for (const edge of incoming.get(current) ?? []) {
        if (edge.edge_kind !== "evidence_claim" || !allowed.has(stringValue(edge.predicate_id))) continue;
        const parent = stringValue(edge.from_id);
        if (!admit(parent)) continue;
        componentEdges.set(stringValue(edge.edge_id), edge);
        lineageQueue.push(parent);
      }
    }
    if (forwardRoots.size === 0) {
      forwardRoots = new Set([...componentIds].filter((id) => stringValue(nodesById.get(id)?.node_kind) !== "link"));
    }
  }

  const forwardQueue = [...forwardRoots].sort();
  const visitedForward = new Set<string>();
  for (let cursor = 0; cursor < forwardQueue.length; cursor += 1) {
    const current = forwardQueue[cursor]!;
    if (visitedForward.has(current)) continue;
    visitedForward.add(current);
    for (const edge of semanticOutgoing.get(current) ?? []) {
      const target = stringValue(edge.to_id);
      if (!admit(target)) continue;
      componentEdges.set(stringValue(edge.edge_id), edge);
      forwardQueue.push(target);
    }
  }

  const ancestorQueue: string[] = [];
  const workIds = [...componentIds]
    .filter((id) => stringValue(nodesById.get(id)?.node_kind) === "work")
    .sort();
  for (const workId of workIds) {
    for (const edge of incoming.get(workId) ?? []) {
      if (edge.edge_kind !== "authored_source_planting") continue;
      const parent = stringValue(edge.from_id);
      if (!nodesById.has(parent)) continue;
      if (!componentIds.has(parent) && componentIds.size >= limit) {
        truncated = true;
        continue;
      }
      componentIds.add(parent);
      componentEdges.set(stringValue(edge.edge_id), edge);
      ancestorQueue.push(parent);
    }
  }

  const visitedAncestors = new Set<string>();
  for (let cursor = 0; cursor < ancestorQueue.length; cursor += 1) {
    const current = ancestorQueue[cursor]!;
    if (visitedAncestors.has(current)) continue;
    visitedAncestors.add(current);
    const currentKind = stringValue(nodesById.get(current)?.node_kind);
    for (const edge of incoming.get(current) ?? []) {
      const isBranchParent = edge.edge_kind === "authored_branch_hierarchy";
      const isPlantingParent = currentKind === "source_planting"
        && edge.edge_kind === "authored_source_planting"
        && edge.predicate_id === "has_source_planting";
      if (!isBranchParent && !isPlantingParent) continue;
      const parent = stringValue(edge.from_id);
      if (!nodesById.has(parent)) continue;
      if (!componentIds.has(parent) && componentIds.size >= limit) {
        truncated = true;
        continue;
      }
      componentIds.add(parent);
      componentEdges.set(stringValue(edge.edge_id), edge);
      ancestorQueue.push(parent);
    }
  }

  const componentNodes = [...componentIds].sort().map((id) => nodesById.get(id) as Item);
  const chain: Record<string, Item[]> = {};
  for (const kind of CHAIN_KINDS) chain[kind] = componentNodes.filter((node) => node.node_kind === kind);
  const componentOutgoing = new Map<string, Item[]>();
  for (const edge of componentEdges.values()) {
    const bucket = componentOutgoing.get(stringValue(edge.from_id)) ?? [];
    bucket.push(edge);
    componentOutgoing.set(stringValue(edge.from_id), bucket);
  }
  const treePaths: Item[] = [];
  for (const era of chain.era ?? []) {
    const eraId = stringValue(era.node_id);
    const frontier: Array<[string, string[], string[]]> = [[eraId, [eraId], []]];
    const seen = new Set([eraId]);
    for (let cursor = 0; cursor < frontier.length; cursor += 1) {
      const [current, nodePath, edgePath] = frontier[cursor]!;
      if (current === objectId) {
        treePaths.push({ node_ids: nodePath, edge_ids: edgePath });
        break;
      }
      for (const edge of sortedById(componentOutgoing.get(current) ?? [], "edge_id")) {
        const target = stringValue(edge.to_id);
        if (!target || seen.has(target)) continue;
        seen.add(target);
        frontier.push([target, [...nodePath, target], [...edgePath, stringValue(edge.edge_id)]]);
      }
    }
  }

  const rights = objectArray(navigation.rights).filter((record) => intersects(record.scope_refs, componentIds));
  let decisionScopeIds = new Set([objectId]);
  if (selectedKind === "link") {
    decisionScopeIds = new Set(
      [...componentEdges.values()]
        .filter((edge) => edge.to_id === objectId && edge.edge_kind === "evidence_claim")
        .map((edge) => stringValue(edge.from_id)),
    );
  }
  const decisionRights = rights.filter((record) => intersects(record.scope_refs, decisionScopeIds));
  const dossierLinks = selectedKind === "work" ? (chain.link ?? []) : [selected];
  const linkStatuses = new Set(dossierLinks.map((node) => stringValue(itemObject(node.properties).access_status) || "unknown"));
  let technicalAccess = "unknown";
  if (linkStatuses.has("open_download")) technicalAccess = "downloadable";
  else if (linkStatuses.has("open_view")) technicalAccess = "viewable";
  else if (linkStatuses.has("metadata_only")) technicalAccess = "metadata_only";
  else if (["restricted", "login_required", "unavailable"].some((status) => linkStatuses.has(status))) {
    technicalAccess = "restricted_or_unavailable";
  }

  const positiveRights = decisionRights.filter((record) =>
    ["licensed", "public_domain_reviewed"].includes(stringValue(record.assessment_status))
    && ["authorized", "authorized_with_conditions"].includes(stringValue(record.redistribution_posture))
  );
  const reviewedPositive = positiveRights.filter((record) =>
    ["accepted", "accepted_with_limits"].includes(stringValue(record.review_status))
  );
  const rightsPosture = reviewedPositive.length > 0
    ? "reviewed_reuse_route"
    : positiveRights.length > 0
      ? "candidate_requires_human_review"
      : decisionRights.length > 0
        ? "not_cleared"
        : "unknown";
  const gaps: string[] = [];
  if (decisionRights.length === 0) gaps.push("no associated public rights record");
  if (positiveRights.length > 0 && reviewedPositive.length === 0) gaps.push("positive rights route exists but has no accepted human review");
  if ((chain.link ?? []).length === 0) gaps.push("no first-class associated Link record");

  const sourceRefSet = new Set<string>();
  for (const node of componentNodes) {
    const sourceRef = stringValue(node.source_ref);
    if (sourceRef) sourceRefSet.add(sourceRef);
  }
  for (const edge of componentEdges.values()) {
    for (const sourceRef of stringArray(edge.source_refs)) sourceRefSet.add(sourceRef);
  }
  for (const record of rights) {
    const sourceRef = stringValue(record.source_ref);
    if (sourceRef) sourceRefSet.add(sourceRef);
  }

  return {
    schema: "tos_source_dossier_v1",
    object_id: objectId,
    object: selected,
    agent_summary: {
      technical_access: technicalAccess,
      rights_posture: rightsPosture,
      human_review_required: reviewedPositive.length === 0,
      can_conclude_legal_openness: reviewedPositive.length > 0,
      availability_is_license: false,
      rights_scope_refs: [...decisionScopeIds].sort(),
      gaps,
    },
    chain,
    tree_paths: treePaths,
    relations: [...componentEdges.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([, edge]) => edge),
    rights: sortedById(rights, "rights_id"),
    source_refs: [...sourceRefSet].sort(),
    truncated,
    authority_note: navigation.authority_boundary,
  };
}
