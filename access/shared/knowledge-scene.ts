/** Shared packet-local scene composition. No source writes or identity admission. */
type Item = Record<string, unknown>;

const CARRIER_SOURCE_PRIORITY: Record<string, number> = {
  'source-navigation': 0, canon: 1, 'source-claims': 2, philosophy: 3,
  'candidate-intake': 4, repository: 5, 'semantic-interchange': 6,
};

export function compareSceneIds(a: string, b: string) {
    const left = Array.from(a), right = Array.from(b);
    for (let i = 0; i < Math.min(left.length, right.length); i++) {
      const delta = left[i]!.codePointAt(0)! - right[i]!.codePointAt(0)!;
      if (delta) return delta;
    }
    return left.length - right.length;
}

type SceneVertex = {id: string; node_ids: string[]};
type SceneArc = {relation_id: string; from_id: string; to_id: string};

// Optional view over the exact packet, not a new asserted relationship.
// Unknown incident edges keep a Claim explicit; grounds and typed value members
// fold into inspectable path details, retaining focused or shared neighborhoods.
function compactClaimScene(nodes: Item[], relations: Item[], vertices: SceneVertex[], arcs: SceneArc[],
                           byNode: Map<string, string>, focusNodeId: string | null, focusRelationId: string | null) {
  const byRelation = new Map(relations.map(r => [String(r.id), r]));
  const detailTypes = new Set(['tos.relation.claim-supported-by', 'tos.relation.claim-value-member']);
  const outgoing = new Map<string, Item[]>(), incident = new Map<string, SceneArc[]>();
  for (const relation of relations) {
    const id = String(relation.from_id);
    if (!outgoing.has(id)) outgoing.set(id, []);
    outgoing.get(id)!.push(relation);
  }
  for (const arc of arcs) for (const id of new Set([arc.from_id, arc.to_id])) {
    if (!incident.has(id)) incident.set(id, []);
    incident.get(id)!.push(arc);
  }
  const claims = new Map(nodes.filter(n => n.type_id === 'tos.entity.claim'
    || strings(record(n.semantics).type_ancestors).includes('tos.entity.claim')).map(n => [String(n.id), n]));
  const candidates = new Map<string, {node: Item; claim: Item; legs: string[]}>(), reasons = new Map<string, string>();
  for (const [id, node] of [...claims].sort(([a], [b]) => compareSceneIds(a, b))) {
    const claim = record(record(node.semantics).claim), subject = claim.subject_node_id, object = claim.object_node_id;
    if (typeof subject !== 'string' || typeof object !== 'string' || !byNode.has(subject) || !byNode.has(object)) {
      reasons.set(id, 'incomplete-claim-contract'); continue;
    }
    if (claim.predicate_mapping_status !== 'mapped' || !claim.relation_type_id) {
      reasons.set(id, 'unmapped-claim-predicate'); continue;
    }
    if ([byNode.get(subject), byNode.get(object)].includes(byNode.get(id))) {
      reasons.set(id, 'claim-endpoint-identity-collision'); continue;
    }
    const legs = ['tos.relation.has-subject', 'tos.relation.has-object']
      .map(kind => (outgoing.get(id) ?? []).filter(r => r.relation_type_id === kind));
    if (legs.some(leg => leg.length !== 1) || legs[0]![0]!.to_id !== subject || legs[1]![0]!.to_id !== object) {
      reasons.set(id, 'incomplete-or-ambiguous-path'); continue;
    }
    const members = claim.value_member_node_ids;
    const memberEdges = (outgoing.get(id) ?? []).filter(r => r.relation_type_id === 'tos.relation.claim-value-member');
    if (Object.hasOwn(claim, 'value_member_node_ids') || memberEdges.length) {
      const targets = new Set(memberEdges.map(r => String(r.to_id)));
      if (!Array.isArray(members) || !members.length || members.some(member => typeof member !== 'string')
          || new Set(members).size !== members.length
          || members.some(member => !byNode.has(member) || !targets.has(member))
          || memberEdges.length !== members.length || targets.size !== members.length) {
        reasons.set(id, 'incomplete-value-member-context'); continue;
      }
    }
    candidates.set(id, {node, claim, legs: [String(legs[0]![0]!.id), String(legs[1]![0]!.id)]});
  }
  const focusVertex = focusNodeId === null ? undefined : byNode.get(focusNodeId);
  const folded = new Set<string>(), removed = new Set<string>(), detailVertices = new Set<string>();
  const paths: (Item & {id: string; from_id: string; to_id: string})[] = [];
  for (const vertex of vertices) {
    const ids = vertex.node_ids, idSet = new Set(ids), localClaims = ids.filter(id => claims.has(id));
    if (!localClaims.length) continue;
    let reason: string | null = null;
    if (vertex.id === focusVertex) reason = 'focus-claim';
    else if ((incident.get(vertex.id) ?? []).some(arc => arc.relation_id === focusRelationId)) reason = 'focus-relation';
    else if (ids.some(id => !candidates.has(id))) reason = 'mixed-or-incomplete-claim-carriers';
    else {
      const legs = new Set(ids.flatMap(id => candidates.get(id)!.legs));
      for (const arc of incident.get(vertex.id) ?? []) {
        if (legs.has(arc.relation_id)) continue;
        const relation = byRelation.get(arc.relation_id)!;
        if (!idSet.has(String(relation.from_id)) || !detailTypes.has(String(relation.relation_type_id))
            || arc.to_id === vertex.id) { reason = 'nonfoldable-incident-relation'; break; }
        if (arc.to_id === focusVertex) { reason = 'focus-detail'; break; }
      }
    }
    // A focused Claim remains an explicit vertex, while a complete path is
    // still useful as the bounded reader's exact context unit. Do not fold
    // the focused vertex; consume only the path relations below so the scene
    // accounts for them once through `claim_paths`.
    if (reason && reason !== 'focus-claim') {
      for (const id of localClaims) if (!reasons.has(id)) reasons.set(id, reason);
      continue;
    }
    if (reason === 'focus-claim' && localClaims.some(id => !candidates.has(id))) {
      for (const id of localClaims) if (!reasons.has(id)) reasons.set(id, reason);
      continue;
    }
    if (reason === null) folded.add(vertex.id);
    for (const id of ids) {
      if (!candidates.has(id)) continue;
      const {node, claim, legs} = candidates.get(id)!;
      const details = (outgoing.get(id) ?? []).filter(r => detailTypes.has(String(r.relation_type_id)))
        .map(r => String(r.id)).sort(compareSceneIds);
      for (const relationId of [...legs, ...details]) removed.add(relationId);
      for (const relationId of details) detailVertices.add(byNode.get(String(byRelation.get(relationId)!.to_id))!);
      let wording: string | null = null;
      let wordingMode = 'claim-with-mandatory-context';
      const roles = record(record(node.human_form_selection).roles);
      for (const role of ['caption', 'statement', 'hover']) if (record(roles[role]).state === 'ready') {
        const shared = record(node.human_form_selection).schema_version === 'tos_human_form_selection_v2';
        wording = `/human_form_selection/roles/${role}` + (shared ? '' : '/packet');
        if (shared) wordingMode = 'claim-with-shared-form-context-v2';
        break;
      }
      if (wording === null) {
        const fields = record(record(node.display_selection).fields);
        for (const field of ['summary', 'title']) if (record(fields[field]).content_available === true) {
          wording = `/display_selection/fields/${field}`; break;
        }
      }
      paths.push({id: 'tos-scene:claim-path:' + id,
        from_id: byNode.get(String(claim.subject_node_id))!, to_id: byNode.get(String(claim.object_node_id))!,
        claim_node_id: id, relation_type_id: claim.relation_type_id,
        node_ids: [claim.subject_node_id, id, claim.object_node_id], relation_ids: legs, detail_relation_ids: details,
        reading: {mode: wordingMode, node_id: id, content_revision: node.content_revision,
          wording_pointer: wording, wording_state: wording ? 'available' : 'missing',
          context_pointers: ['/semantics', '/epistemic'], relation_context_ids: [...legs, ...details], standalone: false}});
    }
  }
  const retainedArcs = arcs.filter(arc => !removed.has(arc.relation_id));
  const endpoints = new Set([...retainedArcs, ...paths].flatMap(arc => [arc.from_id, arc.to_id]));
  const claimVertices = new Set([...claims.keys()].map(id => byNode.get(id)!));
  for (const id of detailVertices) if (!endpoints.has(id) && id !== focusVertex && !claimVertices.has(id)) folded.add(id);
  return {rule: 'explicit-claim-paths-v1', vertex_ids: vertices.filter(v => !folded.has(v.id)).map(v => v.id),
    relation_ids: retainedArcs.map(arc => arc.relation_id), claim_paths: paths.sort((a, b) => compareSceneIds(a.id, b.id)),
    folded_vertex_ids: [...folded].sort(compareSceneIds),
    retained_claims: [...reasons].sort(([a], [b]) => compareSceneIds(a, b)).map(([node_id, reason]) => ({node_id, reason})),
    authority: 'presentation-only-no-new-assertion'};
}

// Presentation identity only. The enclosing packet owns exact records,
// revisions, wording and inspection; this does not adjudicate same_as claims.
export function knowledgeScene(nodes: Item[], relations: Item[], focusNodeId: string | null = null, focusRelationId: string | null = null) {
  const groups = new Map<string, {entity_id: string | null; nodes: Item[]}>();
  const byNode = new Map<string, string>();
  for (const node of nodes) {
    const entity = typeof node.entity_id === 'string' && node.entity_id.startsWith('tos.') ? node.entity_id : null;
    const id = entity ? 'tos-scene:entity:' + entity : 'tos-scene:carrier:' + String(node.id);
    byNode.set(String(node.id), id);
    if (!groups.has(id)) groups.set(id, {entity_id: entity, nodes: []});
    groups.get(id)!.nodes.push(node);
  }
  const vertices = [...groups].sort(([a], [b]) => compareSceneIds(a, b)).map(([id, group]) => {
    const ordered = group.nodes.slice().sort((a, b) =>
      (CARRIER_SOURCE_PRIORITY[String(a.source_graph)] ?? 99) - (CARRIER_SOURCE_PRIORITY[String(b.source_graph)] ?? 99)
      || compareSceneIds(String(a.id), String(b.id)));
    return {id, entity_id: group.entity_id, node_ids: group.nodes.map(n => String(n.id)).sort(compareSceneIds),
      representative_node_id: String(ordered[0]!.id)};
  });
  const arcs: {relation_id: string; from_id: string; to_id: string}[] = [], collapsed: string[] = [];
  for (const relation of relations.slice().sort((a, b) => compareSceneIds(String(a.id), String(b.id)))) {
    const left = byNode.get(String(relation.from_id)), right = byNode.get(String(relation.to_id));
    if (!left || !right) throw new Error('scene relation endpoint missing from returned packet');
    if (left === right && relation.relation_type_id === 'tos.relation.projects' && relation.id !== focusRelationId) collapsed.push(String(relation.id));
    else arcs.push({relation_id: String(relation.id), from_id: left, to_id: right});
  }
  return {schema_version: 'tos_knowledge_scene_v1', vertices, arcs, collapsed_relation_ids: collapsed,
    compact: compactClaimScene(nodes, relations, vertices, arcs, byNode, focusNodeId, focusRelationId),
    focus_vertex_id: focusNodeId === null ? null : byNode.get(focusNodeId) ?? null,
    scope: 'returned-packet-only', identity_rule: 'declared-tos-entity-id',
    authority: 'presentation-mapping-not-semantic-admission'};
}

function record(value: unknown): Item {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string" && item.length > 0) : [];
}
