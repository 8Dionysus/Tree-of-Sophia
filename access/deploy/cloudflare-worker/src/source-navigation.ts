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
const DOSSIER_KINDS = new Set(["work", "expression", "edition", "item", "file", "link"]);

function itemObject(value: unknown): Item {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

function sortedById(items: Item[], key: string): Item[] {
  return [...items].sort((left, right) => stringValue(left[key]).localeCompare(stringValue(right[key])));
}

function intersects(values: unknown, selected: Set<string>): boolean {
  return stringArray(values).some((value) => selected.has(value));
}

const ROOT_RIGHTS_ID = /^tos\.rights\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/;
const LAYER_RIGHTS_ID = /^tos\.rights\.[a-z0-9]+(?:[.-][a-z0-9]+)*\.layer\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/;

/** Keep layer findings visible while deriving openness only from an unambiguous aggregate row. */
export function aggregateRightsRecords(records: Item[]): Item[] {
  const bySource = new Map<string, Item[]>();
  for (const record of records) {
    const sourceRef = stringValue(record.source_ref);
    if (!sourceRef) continue;
    const group = bySource.get(sourceRef) ?? [];
    group.push(record);
    bySource.set(sourceRef, group);
  }

  const aggregateRecords: Item[] = [];
  for (const sourceRecords of bySource.values()) {
    if (sourceRecords.some((record) => Object.prototype.hasOwnProperty.call(record, "assessment_kind"))) {
      if (sourceRecords.some((record) => !["aggregate", "layer"].includes(stringValue(record.assessment_kind)))) continue;
      const aggregates = sourceRecords.filter((record) => stringValue(record.assessment_kind) === "aggregate");
      if (aggregates.length === 1) aggregateRecords.push(aggregates[0]!);
      continue;
    }

    const aggregateCandidates = sourceRecords.filter((record) =>
      ROOT_RIGHTS_ID.test(stringValue(record.rights_id)) && !LAYER_RIGHTS_ID.test(stringValue(record.rights_id)));
    const layerCandidates = sourceRecords.filter((record) => LAYER_RIGHTS_ID.test(stringValue(record.rights_id)));
    if (aggregateCandidates.length === 1 && aggregateCandidates.length + layerCandidates.length === sourceRecords.length) {
      aggregateRecords.push(aggregateCandidates[0]!);
    }
  }
  return aggregateRecords;
}

/** Keep File-scoped rights only through Item→File memberships in this dossier. */
export function filterFileScopedRights(
  rights: Item[],
  componentIds: Set<string>,
  nodesById: Map<string, Item>,
  componentEdges: Iterable<Item>,
  incomingByFile: Map<string, Item[]>,
): Item[] {
  const fileIds = new Set([...componentIds].filter((id) => stringValue(nodesById.get(id)?.node_kind) === "file"));
  if (fileIds.size === 0) return rights;
  const targetBibliographicIds = new Set([...componentIds].filter((id) =>
    ["work", "expression", "edition"].includes(stringValue(nodesById.get(id)?.node_kind))));
  const membershipsByFile = new Map<string, Array<{
    itemId: string;
    rightsRefs: Set<string>;
    legacy: boolean;
    valid: boolean;
  }>>();
  for (const edge of componentEdges) {
    if (edge.edge_kind !== "authored_item_manifest" || edge.predicate_id !== "has_file") continue;
    const itemId = stringValue(edge.from_id);
    const fileId = stringValue(edge.to_id);
    if (!componentIds.has(itemId) || stringValue(nodesById.get(itemId)?.node_kind) !== "item" || !fileIds.has(fileId)) continue;

    const rawSourceRefs = edge.source_refs;
    const edgeSourceRefs = stringArray(rawSourceRefs);
    const sourceRefSet = new Set(edgeSourceRefs);
    let valid = Array.isArray(rawSourceRefs) && edgeSourceRefs.length === rawSourceRefs.length
      && sourceRefSet.size === edgeSourceRefs.length && edgeSourceRefs.length > 0;
    const properties = itemObject(edge.properties);
    const rightsRefs = new Set<string>();
    let legacy = false;
    if (!Object.prototype.hasOwnProperty.call(properties, "item_file_contexts")) {
      legacy = true;
      valid = valid && edgeSourceRefs.length === 1;
    } else {
      const rawContexts = properties.item_file_contexts;
      const contextManifestRefs = new Set<string>();
      if (!Array.isArray(rawContexts) || rawContexts.length === 0) {
        valid = false;
      } else {
        for (const rawContext of rawContexts) {
          if (!rawContext || typeof rawContext !== "object" || Array.isArray(rawContext)) {
            valid = false;
            continue;
          }
          const context = rawContext as Item;
          const manifestRef = stringValue(context.manifest_ref);
          if (!manifestRef || !sourceRefSet.has(manifestRef) || contextManifestRefs.has(manifestRef)) valid = false;
          else contextManifestRefs.add(manifestRef);
          const rawRightsRef = context.rights_ref;
          const rightsRef = stringValue(rawRightsRef);
          if (rightsRef) rightsRefs.add(rightsRef);
          else if (rawRightsRef === undefined || rawRightsRef === null || rawRightsRef === "") legacy = true;
          else valid = false;
        }
        if (contextManifestRefs.size !== sourceRefSet.size
          || [...sourceRefSet].some((sourceRef) => !contextManifestRefs.has(sourceRef))) valid = false;
      }
    }
    if (rightsRefs.size > 1 || (rightsRefs.size > 0 && legacy)) valid = false;
    const memberships = membershipsByFile.get(fileId) ?? [];
    memberships.push({ itemId, rightsRefs, legacy, valid });
    membershipsByFile.set(fileId, memberships);
  }
  for (const memberships of membershipsByFile.values()) {
    const itemCounts = new Map<string, number>();
    for (const membership of memberships) {
      itemCounts.set(membership.itemId, (itemCounts.get(membership.itemId) ?? 0) + 1);
    }
    for (const membership of memberships) {
      if ((itemCounts.get(membership.itemId) ?? 0) > 1) membership.valid = false;
    }
  }

  const scopesFor = (record: Item): Set<string> => new Set(stringArray(record.scope_refs));
  const legacySourceIsUnique = (itemId: string, fileId: string, sourceRef: unknown): boolean => {
    const ownerEdges = (incomingByFile.get(fileId) ?? []).filter((edge) =>
      edge.edge_kind === "authored_item_manifest" && edge.predicate_id === "has_file");
    if (ownerEdges.length !== 1 || stringValue(ownerEdges[0]?.from_id) !== itemId) return false;
    const matching = rights.filter((record) => {
      const scopes = scopesFor(record);
      return scopes.has(itemId) && scopes.has(fileId);
    });
    const sources = new Set(matching.map((record) => stringValue(record.source_ref)).filter(Boolean));
    return sources.size === 1 && matching.every((record) => stringValue(record.source_ref))
      && sources.has(stringValue(sourceRef));
  };

  return rights.filter((record) => {
    const scopes = scopesFor(record);
    const fileScopes = [...fileIds].filter((id) => scopes.has(id));
    if (fileScopes.length === 0 || [...targetBibliographicIds].some((id) => scopes.has(id))) return true;
    return fileScopes.every((fileId) => (membershipsByFile.get(fileId) ?? []).some((membership) => {
      if (!membership.valid) return false;
      if (membership.rightsRefs.size > 0) return membership.rightsRefs.has(stringValue(record.source_ref));
      return membership.legacy && scopes.has(membership.itemId)
        && legacySourceIsUnique(membership.itemId, fileId, record.source_ref);
    }));
  });
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
  if (!DOSSIER_KINDS.has(selectedKind)) {
    throw new SourceNavigationError(400, "dossiers are available for Work, Expression, Edition, Item, File, and Link objects");
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
  if (selectedKind !== "work") {
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
        const structuralFileParent = currentKind === "file" && edge.edge_kind === "authored_item_manifest";
        if (!structuralFileParent && (edge.edge_kind !== "evidence_claim" || !allowed.has(stringValue(edge.predicate_id)))) continue;
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
  if (selectedKind !== "file") {
    rights.splice(0, rights.length, ...filterFileScopedRights(
      rights, componentIds, nodesById, componentEdges.values(), incoming,
    ));
  }
  let decisionScopeIds = new Set([objectId]);
  let decisionRights: Item[];
  let fileMembershipComplete = false;
  let fileMemberReviewedPositive: Record<string, boolean> = {};
  let fileMembershipGap = "";
  if (selectedKind === "link") {
    decisionScopeIds = new Set(
      [...componentEdges.values()]
        .filter((edge) => edge.to_id === objectId && edge.edge_kind === "evidence_claim")
        .map((edge) => stringValue(edge.from_id)),
    );
  }
  if (selectedKind === "file") {
    const membershipEdges = (incoming.get(objectId) ?? []).filter((edge) =>
      edge.edge_kind === "authored_item_manifest" && edge.predicate_id === "has_file" && edge.to_id === objectId,
    );
    const memberships = new Map<string, { rightsRefs: Set<string>; legacy: boolean; duplicate: boolean }>();
    let membershipBindingsValid = membershipEdges.length > 0;
    const representedManifestRefs = new Set<string>();
    for (const edge of membershipEdges) {
      const itemId = stringValue(edge.from_id);
      if (!itemId || stringValue(nodesById.get(itemId)?.node_kind) !== "item") {
        membershipBindingsValid = false;
        continue;
      }
      const prior = memberships.get(itemId);
      if (prior) prior.duplicate = true;
      const membership = prior ?? { rightsRefs: new Set<string>(), legacy: false, duplicate: false };
      memberships.set(itemId, membership);
      const rawEdgeSourceRefs = edge.source_refs;
      const edgeSourceRefs = new Set(stringArray(rawEdgeSourceRefs));
      if (!Array.isArray(rawEdgeSourceRefs) || edgeSourceRefs.size !== rawEdgeSourceRefs.length) {
        membershipBindingsValid = false;
      }
      for (const sourceRef of edgeSourceRefs) representedManifestRefs.add(sourceRef);
      const properties = itemObject(edge.properties);
      const hasContexts = Object.prototype.hasOwnProperty.call(properties, "item_file_contexts");
      if (!hasContexts) {
        membership.legacy = true;
        if (edgeSourceRefs.size !== 1) membershipBindingsValid = false;
        continue;
      }
      const rawContexts = properties.item_file_contexts;
      if (!Array.isArray(rawContexts) || rawContexts.length === 0) {
        membershipBindingsValid = false;
        continue;
      }
      const contexts = objectArray(rawContexts);
      if (contexts.length !== rawContexts.length) membershipBindingsValid = false;
      const contextManifestRefs = new Set<string>();
      for (const context of contexts) {
        const manifestRef = stringValue(context.manifest_ref);
        const rightsRef = stringValue(context.rights_ref);
        if (!manifestRef || !edgeSourceRefs.has(manifestRef)) membershipBindingsValid = false;
        if (manifestRef && contextManifestRefs.has(manifestRef)) membershipBindingsValid = false;
        else if (manifestRef) contextManifestRefs.add(manifestRef);
        if (rightsRef) membership.rightsRefs.add(rightsRef);
        else membership.legacy = true;
      }
      if (contextManifestRefs.size !== edgeSourceRefs.size
        || [...edgeSourceRefs].some((sourceRef) => !contextManifestRefs.has(sourceRef))) {
        membershipBindingsValid = false;
      }
    }
    // New content-addressed File nodes list every Item manifest that
    // establishes a membership. Old snapshots without node.source_refs retain
    // the legacy membership fallback above.
    if (Object.prototype.hasOwnProperty.call(selected, "source_refs")) {
      const rawFileSourceRefs = selected.source_refs;
      const fileSourceRefs = stringArray(rawFileSourceRefs);
      if (!Array.isArray(rawFileSourceRefs) || fileSourceRefs.length === 0
        || fileSourceRefs.length !== rawFileSourceRefs.length
        || new Set(fileSourceRefs).size !== fileSourceRefs.length
        || fileSourceRefs.some((sourceRef) => !sourceRef)
        || fileSourceRefs.length !== representedManifestRefs.size
        || fileSourceRefs.some((sourceRef) => !representedManifestRefs.has(sourceRef))) {
        membershipBindingsValid = false;
      }
    }
    const memberIds = new Set(memberships.keys());
    decisionScopeIds = new Set([objectId, ...memberIds]);
    fileMembershipComplete = memberIds.size > 0 && [...memberIds].every((id) => componentIds.has(id));
    if (!fileMembershipComplete) membershipBindingsValid = false;
    const legacySingleOwner = memberIds.size === 1;
    const rightsByMember = new Map<string, Item[]>();
    const boundRights = new Map<string, Item>();
    for (const [itemId, membership] of memberships) {
      if (membership.duplicate || membership.rightsRefs.size > 1
        || (membership.rightsRefs.size > 0 && membership.legacy)
        || (membership.rightsRefs.size === 0 && !(legacySingleOwner && membership.legacy))) {
        membershipBindingsValid = false;
        continue;
      }
      const memberRights = rights.filter((record) => {
        const scopes = new Set(stringArray(record.scope_refs));
        if (membership.rightsRefs.size > 0) {
          // The exact manifest rights_ref supplies the Item context. Preserve
          // assessments scoped to that Item or this selected File even when a
          // layered row does not repeat both scope IDs.
          return membership.rightsRefs.has(stringValue(record.source_ref))
            && (scopes.has(itemId) || scopes.has(objectId));
        }
        // Legacy snapshots have no edge-level binding and stay strict.
        return scopes.has(itemId) && scopes.has(objectId);
      });
      if (membership.rightsRefs.size === 0) {
        // Keep the prior single-owner behavior only when Item+File scope
        // resolves to one legacy rights source. Multiple source files without
        // an edge-level ref are ambiguous even if one record is positive.
        const legacySources = new Set(memberRights.map((record) => stringValue(record.source_ref)).filter(Boolean));
        if (legacySources.size !== 1 || memberRights.some((record) => !stringValue(record.source_ref))) {
          membershipBindingsValid = false;
          continue;
        }
      }
      rightsByMember.set(itemId, memberRights);
      for (const record of memberRights) boundRights.set(stringValue(record.rights_id) || JSON.stringify(record), record);
      if (memberRights.length === 0) membershipBindingsValid = false;
    }
    decisionRights = [...boundRights.values()].sort((left, right) => stringValue(left.rights_id).localeCompare(stringValue(right.rights_id)));
    // A File packet only contains rights records that are bound to one of its
    // exact Item memberships, never records that merely mention the File ID.
    rights.splice(0, rights.length, ...decisionRights);
    for (const [itemId, memberRights] of rightsByMember) {
      fileMemberReviewedPositive[itemId] = aggregateRightsRecords(memberRights).some((record) =>
        ["licensed", "public_domain_reviewed"].includes(stringValue(record.assessment_status))
        && ["authorized", "authorized_with_conditions"].includes(stringValue(record.redistribution_posture))
        && ["accepted", "accepted_with_limits"].includes(stringValue(record.review_status))
      );
    }
    fileMembershipComplete = fileMembershipComplete && membershipBindingsValid;
    if (!fileMembershipComplete) fileMembershipGap = "File membership or its exact rights binding is incomplete";
  } else {
    decisionRights = rights.filter((record) => intersects(record.scope_refs, decisionScopeIds));
    if (selectedKind === "item") {
      // An Item packet is scoped to that acquisition, not a sibling that
      // happens to share one of its content-addressed Files.
      rights.splice(0, rights.length, ...decisionRights);
    }
  }
  const dossierLinks = selectedKind === "link" ? [selected] : (chain.link ?? []);
  const linkStatuses = new Set(dossierLinks.map((node) => stringValue(itemObject(node.properties).access_status) || "unknown"));
  let technicalAccess = "unknown";
  if (linkStatuses.has("open_download")) technicalAccess = "downloadable";
  else if (linkStatuses.has("open_view")) technicalAccess = "viewable";
  else if (linkStatuses.has("metadata_only")) technicalAccess = "metadata_only";
  else if (["restricted", "login_required", "unavailable"].some((status) => linkStatuses.has(status))) {
    technicalAccess = "restricted_or_unavailable";
  }

  const decisionAggregateRights = aggregateRightsRecords(decisionRights);
  const positiveRights = decisionAggregateRights.filter((record) =>
    ["licensed", "public_domain_reviewed"].includes(stringValue(record.assessment_status))
    && ["authorized", "authorized_with_conditions"].includes(stringValue(record.redistribution_posture))
  );
  const reviewedPositive = positiveRights.filter((record) =>
    ["accepted", "accepted_with_limits"].includes(stringValue(record.review_status))
  );
  const fileAllMembersReviewedPositive = selectedKind === "file"
    && fileMembershipComplete
    && Object.keys(fileMemberReviewedPositive).length > 0
    && Object.values(fileMemberReviewedPositive).every(Boolean);
  const rightsPosture = selectedKind === "file"
    ? fileAllMembersReviewedPositive
      ? "reviewed_reuse_route"
      : fileMembershipGap || Object.values(fileMemberReviewedPositive).some(Boolean)
        ? "membership_scoped_review_required"
        : positiveRights.length > 0
          ? "candidate_requires_human_review"
          : decisionRights.length > 0
            ? "not_cleared"
            : "unknown"
    : reviewedPositive.length > 0
      ? "reviewed_reuse_route"
      : positiveRights.length > 0
        ? "candidate_requires_human_review"
        : decisionRights.length > 0
          ? "not_cleared"
          : "unknown";
  const gaps: string[] = [];
  if (decisionRights.length === 0) gaps.push("no associated public rights record");
  else if (decisionAggregateRights.length === 0) gaps.push("no unambiguous aggregate rights assessment");
  if (positiveRights.length > 0 && reviewedPositive.length === 0) gaps.push("positive rights route exists but has no accepted human review");
  if (fileMembershipGap) gaps.push(fileMembershipGap);
  else if (selectedKind === "file" && !fileAllMembersReviewedPositive) gaps.push("not every exact Item membership has an accepted positive rights route");
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
      human_review_required: selectedKind === "file" ? !fileAllMembersReviewedPositive : reviewedPositive.length === 0,
      can_conclude_legal_openness: selectedKind === "file" ? fileAllMembersReviewedPositive : reviewedPositive.length > 0,
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
