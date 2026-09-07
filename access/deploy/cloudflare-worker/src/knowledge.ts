import { normalizePagination, paginateLens, type Pagination } from './lens-pagination.ts';
import { selectHumanForms } from './human-forms.ts';

export type Item = Record<string, unknown>;

const CARRIER_SOURCE_PRIORITY: Record<string, number> = {
  'source-navigation': 0, canon: 1, 'source-claims': 2, philosophy: 3,
  'candidate-intake': 4, repository: 5, 'semantic-interchange': 6,
};

function compareIds(a: string, b: string) {
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
// Unknown incident edges keep a Claim explicit; grounds fold into inspectable
// path details only when they do not carry a retained or focused neighborhood.
function compactClaimScene(nodes: Item[], relations: Item[], vertices: SceneVertex[], arcs: SceneArc[],
                           byNode: Map<string, string>, focusNodeId: string | null) {
  const byRelation = new Map(relations.map(r => [String(r.id), r]));
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
  for (const [id, node] of [...claims].sort(([a], [b]) => compareIds(a, b))) {
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
    else if (ids.some(id => !candidates.has(id))) reason = 'mixed-or-incomplete-claim-carriers';
    else {
      const legs = new Set(ids.flatMap(id => candidates.get(id)!.legs));
      for (const arc of incident.get(vertex.id) ?? []) {
        if (legs.has(arc.relation_id)) continue;
        const relation = byRelation.get(arc.relation_id)!;
        if (!idSet.has(String(relation.from_id)) || relation.relation_type_id !== 'tos.relation.claim-supported-by'
            || arc.to_id === vertex.id) { reason = 'nonfoldable-incident-relation'; break; }
        if (arc.to_id === focusVertex) { reason = 'focus-detail'; break; }
      }
    }
    if (reason) {
      for (const id of localClaims) if (!reasons.has(id)) reasons.set(id, reason);
      continue;
    }
    folded.add(vertex.id);
    for (const id of ids) {
      const {node, claim, legs} = candidates.get(id)!;
      const details = (outgoing.get(id) ?? []).filter(r => r.relation_type_id === 'tos.relation.claim-supported-by')
        .map(r => String(r.id)).sort(compareIds);
      for (const relationId of [...legs, ...details]) removed.add(relationId);
      for (const relationId of details) detailVertices.add(byNode.get(String(byRelation.get(relationId)!.to_id))!);
      let wording: string | null = null;
      const roles = record(record(node.human_form_selection).roles);
      for (const role of ['caption', 'statement', 'hover']) if (record(roles[role]).state === 'ready') {
        wording = `/human_form_selection/roles/${role}/packet`; break;
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
        reading: {mode: 'claim-with-mandatory-context', node_id: id, content_revision: node.content_revision,
          wording_pointer: wording, wording_state: wording ? 'available' : 'missing',
          context_pointers: ['/semantics', '/epistemic'], relation_context_ids: [...legs, ...details], standalone: false}});
    }
  }
  const retainedArcs = arcs.filter(arc => !removed.has(arc.relation_id));
  const endpoints = new Set([...retainedArcs, ...paths].flatMap(arc => [arc.from_id, arc.to_id]));
  const claimVertices = new Set([...claims.keys()].map(id => byNode.get(id)!));
  for (const id of detailVertices) if (!endpoints.has(id) && id !== focusVertex && !claimVertices.has(id)) folded.add(id);
  return {rule: 'explicit-claim-paths-v1', vertex_ids: vertices.filter(v => !folded.has(v.id)).map(v => v.id),
    relation_ids: retainedArcs.map(arc => arc.relation_id), claim_paths: paths.sort((a, b) => compareIds(a.id, b.id)),
    folded_vertex_ids: [...folded].sort(compareIds),
    retained_claims: [...reasons].sort(([a], [b]) => compareIds(a, b)).map(([node_id, reason]) => ({node_id, reason})),
    authority: 'presentation-only-no-new-assertion'};
}

// Presentation identity only. The enclosing packet owns exact records,
// revisions, wording and inspection; this does not adjudicate same_as claims.
export function knowledgeScene(nodes: Item[], relations: Item[], focusNodeId: string | null = null) {
  const groups = new Map<string, {entity_id: string | null; nodes: Item[]}>();
  const byNode = new Map<string, string>();
  for (const node of nodes) {
    const entity = typeof node.entity_id === 'string' && node.entity_id.startsWith('tos.') ? node.entity_id : null;
    const id = entity ? 'tos-scene:entity:' + entity : 'tos-scene:carrier:' + String(node.id);
    byNode.set(String(node.id), id);
    if (!groups.has(id)) groups.set(id, {entity_id: entity, nodes: []});
    groups.get(id)!.nodes.push(node);
  }
  const vertices = [...groups].sort(([a], [b]) => compareIds(a, b)).map(([id, group]) => {
    const ordered = group.nodes.slice().sort((a, b) =>
      (CARRIER_SOURCE_PRIORITY[String(a.source_graph)] ?? 99) - (CARRIER_SOURCE_PRIORITY[String(b.source_graph)] ?? 99)
      || compareIds(String(a.id), String(b.id)));
    return {id, entity_id: group.entity_id, node_ids: group.nodes.map(n => String(n.id)).sort(compareIds),
      representative_node_id: String(ordered[0]!.id)};
  });
  const arcs: {relation_id: string; from_id: string; to_id: string}[] = [], collapsed: string[] = [];
  for (const relation of relations.slice().sort((a, b) => compareIds(String(a.id), String(b.id)))) {
    const left = byNode.get(String(relation.from_id)), right = byNode.get(String(relation.to_id));
    if (!left || !right) throw new Error('scene relation endpoint missing from returned packet');
    if (left === right && relation.relation_type_id === 'tos.relation.projects') collapsed.push(String(relation.id));
    else arcs.push({relation_id: String(relation.id), from_id: left, to_id: right});
  }
  return {schema_version: 'tos_knowledge_scene_v1', vertices, arcs, collapsed_relation_ids: collapsed,
    compact: compactClaimScene(nodes, relations, vertices, arcs, byNode, focusNodeId),
    focus_vertex_id: focusNodeId === null ? null : byNode.get(focusNodeId) ?? null,
    scope: 'returned-packet-only', identity_rule: 'declared-tos-entity-id',
    authority: 'presentation-mapping-not-semantic-admission'};
}

export type LocalizedText = {
  [language: string]: string | null | undefined;
  default: string;
  ru: string | null;
  en: string | null;
  original?: string | null;
};

export type KnowledgeNode = {
  human_form_selection?: ReturnType<typeof selectHumanForms>;
  source_record?: Item;
  id: string;
  entity_id: string;
  native_id: string;
  source_graph: string;
  kind_id: string;
  type_id: string;
  type_mapping: Item;
  semantics: Item;
  content_revision: string;
  display: {
    title: LocalizedText;
    kind_label: LocalizedText;
    summary: LocalizedText;
    summary_state: string;
    provenance: Item;
  };
  epistemic: Item;
  graph_layers: string[];
  view_ids: string[];
  source_refs: string[];
  attributes: Item;
};

export type KnowledgeRelation = {
  human_form_selection?: ReturnType<typeof selectHumanForms>;
  source_record?: Item;
  id: string;
  native_id: string;
  source_graph: string;
  from_id: string;
  to_id: string;
  predicate_id: string;
  relation_type_id: string;
  predicate_mapping: Item;
  semantics: Item;
  content_revision: string;
  display: {
    label: LocalizedText;
    inverse_label: LocalizedText | null;
    statement: LocalizedText;
    explanation: LocalizedText;
    explanation_state: string;
    provenance: Item;
  };
  epistemic: Item;
  graph_layers: string[];
  view_ids: string[];
  source_refs: string[];
  attributes: Item;
};

export type KnowledgeGraph = {
  schema: "tos_knowledge_graph_v1";
  source_revision: string;
  query_properties?: QueryProperty[];
  nodes: KnowledgeNode[];
  relations: KnowledgeRelation[];
  counts: Item;
  authority_boundary: Item;
};

export type QueryProperty = {property_id: string; field: string; value_type: 'string' | 'number' | 'boolean' | 'string-array';
  applies_to: string[]; inherited: boolean; operators: string[]};
export type LensFilter = { field?: string; property_id?: string; op: string; value: unknown; _property_binding?: QueryProperty };
export type SortRule = { field: string; direction: "asc" | "desc" };
export type PathCondition = {
  path_id: string;
  quantifier: "exists" | "not_exists";
  steps: { direction: "incoming" | "outgoing" | "either";
    node_query: LensSpec["node_query"]; relation_query: LensSpec["relation_query"] }[];
};
export type Inclusion = { nodes: Record<string, Item>; relations: Record<string, Item>; authority: "query-execution-not-semantic-proof" };
export type LensSpec = {
  pagination: Pagination | null;
  path_query: PathCondition[];
  explain: boolean;
  detail: "full" | "compact";
  schema_version: "tos_lens_spec_v1";
  lens_id: string;
  title: LocalizedText;
  description: LocalizedText;
  language: string;
  sources: string[];
  seed: { focus_node_id: string | null; node_ids: string[]; text_query: string };
  node_query: { enabled: boolean; match: "all" | "any"; filters: LensFilter[] };
  relation_query: { enabled: boolean; match: "all" | "any"; filters: LensFilter[] };
  traversal: { depth: number; direction: "outgoing" | "incoming" | "either"; predicate_ids: string[]; profile: "all" | "overview" };
  composition: {
    endpoint_policy: "both" | "either" | "independent";
    group_by: string[];
    sort_nodes: SortRule[];
    sort_relations: SortRule[];
  };
  presentation: {
    layout: string;
    color_by: string | null;
    lane_by: string | null;
    size_by: string | null;
    inspector_fields: string[];
  };
  limits: { nodes: number; relations: number; groups: number };
};

const SOURCES = ["philosophy", "canon", "candidate-intake", "source-navigation", "source-claims", "semantic-interchange", "repository"] as const;
export const OVERVIEW_EXCLUDED_PREDICATES = ["has_text_unit", "has_anchor", "anchored_in", "annotation_member"];
export const OVERVIEW_EXCLUDED_RELATION_TYPES = ["tos.relation.made-by", "tos.relation.generated-by"];
const OPERATORS = ["eq", "neq", "in", "contains", "prefix", "exists", "gt", "gte", "lt", "lte"] as const;
const LAYOUTS = ["auto", "organic", "timeline", "flow", "evidence", "semantic", "infrastructure", "hierarchical", "radial", "matrix"] as const;
const IDENTIFIER = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const ATTRIBUTE_FIELD = /^(?:attributes|semantics)\.[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/;
const UNSAFE_PATH_SEGMENTS = new Set(["__proto__", "prototype", "constructor"]);
const NODE_FIELDS = new Set([
  "id", "entity_id", "native_id", "source_graph", "kind_id", "type_id", "type_mapping.status", "type_mapping.source_kind_id", "display.title.default", "display.title.ru", "display.title.en",
  "display.kind_label.default", "display.summary.default", "display.summary.ru", "display.summary.en", "display.summary_state",
  "epistemic.authority_layer", "epistemic.canon_status", "epistemic.review_posture", "epistemic.confidence",
  "graph_layers", "view_ids", "source_refs",
]);
const RELATION_FIELDS = new Set([
  "id", "native_id", "source_graph", "from_id", "to_id", "predicate_id", "relation_type_id", "predicate_mapping.status", "predicate_mapping.source_predicate_id", "display.label.default", "display.label.ru",
  "display.label.en", "display.statement.default", "display.explanation.default", "display.explanation_state",
  "epistemic.authority_layer", "epistemic.canon_status", "epistemic.review_posture", "epistemic.confidence",
  "graph_layers", "view_ids", "source_refs",
]);

function record(value: unknown): Item {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

function strings(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string" && item.length > 0) : [];
}

function strictStrings(value: unknown, name: string, maximum: number, unique = false): string[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(`${name} must be an array`);
  if (value.length > maximum) throw new Error(`${name} must contain at most ${maximum} values`);
  if (value.some((item) => typeof item !== "string" || item.length === 0)) {
    throw new Error(`${name} must contain only non-empty strings`);
  }
  const result = value as string[];
  if (unique && new Set(result).size !== result.length) throw new Error(`${name} must contain unique values`);
  return [...result];
}

function text(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

function humanize(value: string): string {
  return value.replace(/[-_.]+/g, " ").trim() || "unnamed";
}

// Transport syntax only, not registration or linguistic quality validation.
const LANGUAGE_KEY = /^(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\s\S])/;
function formKey(key: string): boolean {
  return key === 'default' || key === 'original' || LANGUAGE_KEY.test(key);
}

function localized(value: unknown, fallback: string): LocalizedText {
  const source = record(value);
  const forms = Object.fromEntries(Object.entries(source).filter(([key]) => formKey(key)).map(([key, item]) => [key, text(item)]));
  const keys = ['default', 'ru', 'en', 'original', ...Object.keys(forms).filter(key => !['default', 'ru', 'en', 'original'].includes(key)).sort()];
  return {
    ru: text(source.ru),
    en: text(source.en),
    original: text(source.original),
    ...forms,
    default: keys.map(key => forms[key]).find(value => typeof value === 'string') ?? text(value) ?? fallback,
  };
}

export function selectDisplayForm(value: unknown, requestedLanguage = 'auto', originalLanguage: unknown = null): Item {
  const forms = Object.fromEntries(Object.entries(record(value)).filter(([key, value]) => formKey(key) && text(value) !== null).map(([key, value]) => [key, text(value)]));
  const available = Object.keys(forms).sort();
  let selected: string | null = null, reason = 'missing';
  if (requestedLanguage !== 'auto' && requestedLanguage !== 'original') {
    let candidate = requestedLanguage;
    while (candidate) {
      const matches = available.filter(key => key.toLowerCase() === candidate.toLowerCase());
      if (matches.length > 1) return {requested_language: requestedLanguage, selected_key: null, actual_language: null,
        text: null, reason: 'ambiguous-language-key', available_keys: available};
      if (matches.length) { selected = matches[0]!; reason = candidate === requestedLanguage ? 'exact-language' : 'less-specific-language'; break; }
      candidate = candidate.includes('-') ? candidate.slice(0, candidate.lastIndexOf('-')) : '';
      if (candidate && candidate.split('-').at(-1)!.length === 1) candidate = candidate.includes('-') ? candidate.slice(0, candidate.lastIndexOf('-')) : '';
    }
  } else if (requestedLanguage === 'original' && Object.hasOwn(forms, 'original')) {
    selected = 'original'; reason = 'original-role';
  }
  if (selected === null) {
    selected = ['default', 'ru', 'en', 'original', ...available].find(key => Object.hasOwn(forms, key)) ?? null;
    if (selected !== null) reason = requestedLanguage === 'auto' ? 'automatic' : 'fallback';
  }
  return {requested_language: requestedLanguage, selected_key: selected,
    actual_language: selected === 'original' ? originalLanguage : selected === 'default' ? null : selected,
    text: selected === null ? null : forms[selected], reason, available_keys: available};
}

function displaySelection(item: KnowledgeNode | KnowledgeRelation, language: string): Item {
  const display = record(item.display), semantics = record(item.semantics);
  const declaredLanguage = text(record(record(semantics.language_context).language).original_language);
  const originalLanguage = declaredLanguage && LANGUAGE_KEY.test(declaredLanguage) ? declaredLanguage : null;
  const fieldNames = 'from_id' in item ? ['label', 'inverse_label', 'statement', 'explanation'] : ['title', 'kind_label', 'summary'];
  const fields = Object.fromEntries(fieldNames.map(field => {
    const selection = selectDisplayForm(display[field], language, field === 'title' ? originalLanguage : null);
    const provenance = record(display.provenance);
    const missing = (field === 'title' && provenance.source_title_available === false)
      || (field === 'summary' && provenance.source_summary_available === false)
      || (field === 'explanation' && provenance.source_explanation_available === false);
    return [field, {...selection, content_available: selection.text !== null && !missing,
      source_form_pointer: selection.selected_key ? `/display/${field}/${selection.selected_key}` : null}];
  }));
  const contexts = Array.isArray(semantics.assertion_contexts) ? semantics.assertion_contexts : [];
  return {schema_version: 'tos_display_selection_v1', content_revision: item.content_revision, fields,
    essential_context_pointers: contexts.map((_, index) => `/semantics/assertion_contexts/${index}`),
    performs_translation: false, is_semantic_assessment: false};
}

function lensLocalized(value: unknown, fallback: string, name: string): LocalizedText {
  if (value === undefined || value === null) return localized(undefined, fallback);
  if (typeof value === "string") {
    const normalized = text(value);
    if (!normalized) throw new Error(`${name} must be a non-empty string or localized object`);
    return localized(normalized, fallback);
  }
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${name} must be a non-empty string or localized object`);
  }
  const source = value as Item;
  const unknown = Object.keys(source).filter(key => !formKey(key)).sort();
  if (unknown.length) throw new Error(`unknown ${name} fields: ${unknown.join(', ')}`);
  for (const [key, item] of Object.entries(source)) {
    if (item !== null && typeof item !== "string") throw new Error(`${name}.${key} must be a string or null`);
  }
  if ("default" in source && !text(source.default)) throw new Error(`${name}.default must be a non-empty string`);
  return localized(source, fallback);
}

function allowedField(field: string, kind: "node" | "relation"): boolean {
  if ((kind === "node" ? NODE_FIELDS : RELATION_FIELDS).has(field)) return true;
  const parts = field.split('.');
  const displayFields = kind === 'node' ? ['title', 'kind_label', 'summary'] : ['label', 'inverse_label', 'statement', 'explanation'];
  if (parts.length === 3 && parts[0] === 'display' && displayFields.includes(parts[1]!) && formKey(parts[2]!)) return true;
  return ATTRIBUTE_FIELD.test(field) && !field.split(".").some((segment) => UNSAFE_PATH_SEGMENTS.has(segment));
}

function onlyKeys(source: Item, allowed: readonly string[], name: string): void {
  const unknown = Object.keys(source).filter((key) => !allowed.includes(key)).sort();
  if (unknown.length > 0) throw new Error(`unknown ${name} fields: ${unknown.join(", ")}`);
}

function strictRecord(value: unknown, name: string, allowed: readonly string[]): Item {
  if (value === undefined || value === null) return {};
  if (typeof value !== "object" || Array.isArray(value)) throw new Error(`${name} must be an object`);
  const result = value as Item;
  onlyKeys(result, allowed, name);
  return result;
}

function filterValue(value: unknown, name: string): unknown {
  const values = Array.isArray(value) ? value : [value];
  if (values.length > 100) throw new Error(`${name} must contain at most 100 scalar values`);
  for (const item of values) {
    if (item !== null && typeof item === "object") throw new Error(`${name} must contain only scalar values`);
    if (typeof item === "number" && !Number.isFinite(item)) throw new Error(`${name} numbers must be finite`);
    if (typeof item === "string" && item.length > 1024) throw new Error(`${name} strings must contain at most 1024 characters`);
  }
  return value;
}

function boundedInteger(value: unknown, name: string, fallback: number, minimum: number, maximum: number): number {
  if (value === undefined || value === null) return fallback;
  if (typeof value === "boolean") throw new Error(`${name} must be between ${minimum} and ${maximum}`);
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new Error(`${name} must be between ${minimum} and ${maximum}`);
  }
  return parsed;
}

function filterGroup(value: unknown, kind: "node" | "relation"): { enabled: boolean; match: "all" | "any"; filters: LensFilter[] } {
  const source = strictRecord(value, `${kind}_query`, ["enabled", "match", "filters"]);
  const enabled = source.enabled ?? true;
  if (typeof enabled !== "boolean") throw new Error(`${kind}_query.enabled must be a boolean`);
  const match = source.match ?? "all";
  if (match !== "all" && match !== "any") throw new Error(`${kind}_query.match must be all or any`);
  const rawFilters = source.filters ?? [];
  if (!Array.isArray(rawFilters) || rawFilters.length > 32) throw new Error(`${kind}_query.filters must contain at most 32 filters`);
  const filters = rawFilters.map((raw): LensFilter => {
    const item = strictRecord(raw, `${kind} filter`, ["field", "property_id", "op", "value"]);
    if (('field' in item) === ('property_id' in item)) throw new Error(`${kind} filter requires exactly one field or property_id`);
    const property = item.property_id;
    if ('property_id' in item && (kind !== 'node' || typeof property !== 'string'
        || !/^tos\.property\.[a-z0-9-]+$(?![\s\S])/.test(property))) {
      throw new Error('property_id must identify a registered node property');
    }
    const field = typeof property === 'string' ? property : text(item.field) ?? "";
    if (!property && !allowedField(field, kind)) throw new Error(`unsupported ${kind} filter field: ${field}`);
    const op = text(item.op) ?? "";
    if (!(OPERATORS as readonly string[]).includes(op)) throw new Error(`unsupported ${kind} filter operator: ${op}`);
    if (!("value" in item)) throw new Error(`${kind} filter ${field} is missing value`);
    const normalizedValue = filterValue(item.value, `${kind} filter ${field}`);
    if ((op === "eq" || op === "neq") && Array.isArray(normalizedValue)) {
      throw new Error(`${kind} ${op} filter ${field} requires a scalar value`);
    }
    if (op === "exists" && typeof normalizedValue !== "boolean") throw new Error(`${kind} exists filter ${field} requires a boolean value`);
    if (op === "prefix" && typeof normalizedValue !== "string") throw new Error(`${kind} prefix filter ${field} requires a string value`);
    if (["gt", "gte", "lt", "lte"].includes(op) && typeof normalizedValue !== "number") {
      throw new Error(`${kind} numeric filter ${field} requires a number value`);
    }
    return { ...(property ? {property_id: field} : {field}), op, value: normalizedValue };
  });
  return { enabled, match, filters };
}

export function bindQueryProperties(definitions: QueryProperty[], spec: LensSpec): LensSpec {
  const bindings = new Map(definitions.map(d => [d.property_id, d]));
  if (bindings.size !== definitions.length) throw new Error('ambiguous snapshot property identity');
  const compiled = structuredClone(spec);
  const groups = [compiled.node_query, ...compiled.path_query.flatMap(path => path.steps.map(step => step.node_query))];
  for (const group of groups) for (const rule of group.filters) {
    if (!rule.property_id) continue;
    const definition = bindings.get(rule.property_id);
    if (!definition) throw new Error(`unknown snapshot property_id: ${rule.property_id}`);
    if (!allowedField(definition.field, 'node') || !definition.operators.includes(rule.op)) {
      throw new Error(`unsupported property operation: ${rule.property_id} ${rule.op}`);
    }
    if (rule.op !== 'exists') {
      const values = Array.isArray(rule.value) ? rule.value : [rule.value];
      const type = definition.value_type === 'string-array' ? 'string' : definition.value_type;
      if (values.some(value => typeof value !== type)) throw new Error(`property ${rule.property_id} requires ${type} query values`);
    }
    rule.field = definition.field;
    rule._property_binding = definition;
  }
  return compiled;
}

function sortRules(value: unknown, kind: "node" | "relation"): SortRule[] {
  if (value === undefined || value === null) return [{ field: "id", direction: "asc" }];
  if (!Array.isArray(value) || value.length > 8) throw new Error(`sort_${kind}s must contain at most 8 fields`);
  const result = value.map((raw): SortRule => {
    const item = strictRecord(raw, `${kind} sort`, ["field", "direction"]);
    const field = text(item.field) ?? "";
    if (!allowedField(field, kind)) throw new Error(`unsupported ${kind} sort field: ${field}`);
    const direction = text(item.direction) ?? "asc";
    if (direction !== "asc" && direction !== "desc") throw new Error(`sort direction must be asc or desc: ${field}`);
    return { field, direction };
  });
  return result.length > 0 ? result : [{ field: "id", direction: "asc" }];
}

function normalizePaths(value: unknown): PathCondition[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value) || value.length > 4) throw new Error("path_query must contain at most 4 conditions");
  const ids = new Set<string>();
  return value.map((raw): PathCondition => {
    const condition = strictRecord(raw, "path condition", ["path_id", "quantifier", "steps"]);
    const pathId = text(condition.path_id) ?? "";
    if (!IDENTIFIER.test(pathId) || ids.has(pathId)) throw new Error("path_id must be a unique safe stable identifier");
    ids.add(pathId);
    const quantifier = condition.quantifier ?? "exists";
    if (quantifier !== "exists" && quantifier !== "not_exists") throw new Error("path quantifier must be exists or not_exists");
    if (!Array.isArray(condition.steps) || condition.steps.length < 1 || condition.steps.length > 4) throw new Error("path steps must contain between 1 and 4 steps");
    const steps = condition.steps.map((rawStep): PathCondition['steps'][number] => {
      const step = strictRecord(rawStep, "path step", ["direction", "node_query", "relation_query"]);
      const direction = step.direction ?? "outgoing";
      if (direction !== "incoming" && direction !== "outgoing" && direction !== "either") throw new Error("path direction must be outgoing, incoming, or either");
      return { direction, node_query: filterGroup(step.node_query, "node"), relation_query: filterGroup(step.relation_query, "relation") };
    });
    return { path_id: pathId, quantifier, steps };
  });
}

export function normalizeLensSpec(value: unknown): LensSpec {
  const source = record(value);
  onlyKeys(source, ["schema_version", "lens_id", "title", "description", "language", "sources", "seed", "node_query", "relation_query", "traversal", "composition", "presentation", "limits", "detail", "path_query", "explain", "pagination"], "lens spec");
  const explain = source.explain ?? false;
  if (typeof explain !== "boolean") throw new Error("explain must be a boolean");
  const detail = source.detail ?? "full";
  if (detail !== "full" && detail !== "compact") throw new Error("detail must be full or compact");
  if (source.schema_version !== "tos_lens_spec_v1") throw new Error("lens spec schema_version must be tos_lens_spec_v1");
  const lensId = text(source.lens_id) ?? "";
  if (!IDENTIFIER.test(lensId)) throw new Error("lens_id must be a safe stable identifier");
  const rawSources = source.sources ?? [...SOURCES];
  if (!Array.isArray(rawSources) || rawSources.length === 0) throw new Error("sources must be a non-empty array");
  if (rawSources.length > SOURCES.length) throw new Error(`sources must contain at most ${SOURCES.length} values`);
  if (rawSources.some((item) => typeof item !== "string" || item.length === 0)) throw new Error("sources must contain only non-empty strings");
  const requestedSources = rawSources as string[];
  if (new Set(requestedSources).size !== requestedSources.length) throw new Error("sources must contain unique values");
  const unknownSources = requestedSources.filter((item) => !(SOURCES as readonly string[]).includes(item));
  if (unknownSources.length > 0) throw new Error(`unsupported knowledge sources: ${unknownSources.sort().join(", ")}`);

  const seed = strictRecord(source.seed, "seed", ["focus_node_id", "node_ids", "text_query"]);
  let focusNodeId: string | null = null;
  if (seed.focus_node_id !== undefined && seed.focus_node_id !== null) {
    focusNodeId = text(seed.focus_node_id);
    if (!focusNodeId) throw new Error("seed.focus_node_id must be a non-empty string or null");
    if (focusNodeId.length > 1024) throw new Error("seed.focus_node_id exceeds 1024 characters");
  }
  const nodeIds = strictStrings(seed.node_ids, "seed.node_ids", 100);
  const textQuery = text(seed.text_query) ?? "";
  if (textQuery.length > 256) throw new Error("seed.text_query exceeds 256 characters");

  const traversal = strictRecord(source.traversal, "traversal", ["depth", "direction", "predicate_ids", "profile"]);
  const profile = traversal.profile ?? "all";
  if (profile !== "all" && profile !== "overview") throw new Error("traversal.profile must be all or overview");
  const depth = boundedInteger(traversal.depth, "traversal.depth", 0, 0, 5);
  const direction = text(traversal.direction) ?? "either";
  if (direction !== "outgoing" && direction !== "incoming" && direction !== "either") {
    throw new Error("traversal.direction must be outgoing, incoming, or either");
  }

  const predicateIds = strictStrings(traversal.predicate_ids, "traversal.predicate_ids", 100, true);

  const composition = strictRecord(source.composition, "composition", ["endpoint_policy", "group_by", "sort_nodes", "sort_relations"]);
  const endpointPolicy = text(composition.endpoint_policy) ?? "both";
  if (endpointPolicy !== "both" && endpointPolicy !== "either" && endpointPolicy !== "independent") {
    throw new Error("composition.endpoint_policy must be both, either, or independent");
  }
  const groupBy = strictStrings(composition.group_by, "composition.group_by", 4, true);
  for (const field of groupBy) {
    if (!allowedField(field, "node") && !allowedField(field, "relation")) throw new Error(`unsupported group field: ${field}`);
  }

  const presentation = strictRecord(source.presentation, "presentation", ["layout", "color_by", "lane_by", "size_by", "inspector_fields"]);
  const layout = text(presentation.layout) ?? "auto";
  if (!(LAYOUTS as readonly string[]).includes(layout)) throw new Error(`unsupported presentation layout: ${layout}`);
  const presentationFields: Record<"color_by" | "lane_by" | "size_by", string | null> = { color_by: null, lane_by: null, size_by: null };
  for (const key of Object.keys(presentationFields) as Array<keyof typeof presentationFields>) {
    const field = text(presentation[key]);
    if (field && !allowedField(field, "node") && !allowedField(field, "relation")) throw new Error(`unsupported presentation field: ${field}`);
    presentationFields[key] = field;
  }
  const inspectorFields = strictStrings(presentation.inspector_fields, "presentation.inspector_fields", 32, true);
  for (const field of inspectorFields) {
    if (!["display", "display.summary", "epistemic", "source_refs", "attributes"].includes(field)
      && !allowedField(field, "node") && !allowedField(field, "relation")) {
      throw new Error(`unsupported inspector field: ${field}`);
    }
  }

  const limits = strictRecord(source.limits, "limits", ["nodes", "relations", "groups"]);
  const language = text(source.language) ?? "auto";
  if (language.length > 128 || (language !== "auto" && language !== "original" && !LANGUAGE_KEY.test(language))) {
    throw new Error("language must be auto, original, or a language tag of at most 128 characters");
  }
  return {
    schema_version: "tos_lens_spec_v1",
    path_query: normalizePaths(source.path_query),
    explain,
    pagination: normalizePagination(source.pagination),
    detail,
    lens_id: lensId,
    title: lensLocalized(source.title, humanize(lensId), "title"),
    description: lensLocalized(source.description, `Declarative knowledge lens ${lensId}.`, "description"),
    language,
    sources: requestedSources,
    seed: { focus_node_id: focusNodeId, node_ids: nodeIds, text_query: textQuery },
    node_query: filterGroup(source.node_query, "node"),
    relation_query: filterGroup(source.relation_query, "relation"),
    traversal: { depth, direction, predicate_ids: predicateIds, profile },
    composition: {
      endpoint_policy: endpointPolicy,
      group_by: groupBy,
      sort_nodes: sortRules(composition.sort_nodes, "node"),
      sort_relations: sortRules(composition.sort_relations, "relation"),
    },
    presentation: {
      layout,
      ...presentationFields,
      inspector_fields: inspectorFields.length > 0 ? inspectorFields : ["display", "epistemic", "source_refs"],
    },
    limits: {
      nodes: boundedInteger(limits.nodes, "nodes", 200, 1, 1000),
      relations: boundedInteger(limits.relations, "relations", 400, 0, 2000),
      groups: boundedInteger(limits.groups, "groups", 100, 1, 200),
    },
  };
}

function fieldValue(item: Item, path: string): unknown {
  let current: unknown = item;
  for (const segment of path.split(".")) {
    if (!current || typeof current !== "object" || Array.isArray(current) || !(segment in current)) return null;
    current = (current as Item)[segment];
  }
  return current;
}

function numeric(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function matchesFilter(item: Item, rule: LensFilter): boolean {
  if (!rule.field) throw new Error('unbound property filter');
  const definition = rule._property_binding;
  if (definition) {
    const types = [item.type_id, ...(definition.inherited ? strings(record(item.semantics).type_ancestors) : [])];
    if (!definition.applies_to.some(type => types.includes(type))) return false;
  }
  const actual = fieldValue(item, rule.field);
  const expected = rule.value;
  if (rule.op === "exists") return (actual !== null && actual !== undefined) === Boolean(expected);
  if (definition && (actual === null || actual === undefined)) return false;
  if (definition && typeof actual === 'string' && ['contains', 'prefix'].includes(rule.op)) {
    return typeof expected === 'string' && (rule.op === 'contains' ? actual.includes(expected) : actual.startsWith(expected));
  }
  if (rule.op === "eq") return actual === expected || (Array.isArray(actual) && actual.includes(expected));
  if (rule.op === "neq") return !matchesFilter(item, { ...rule, op: "eq" });
  if (rule.op === "in") {
    const values = Array.isArray(expected) ? expected : [expected];
    return Array.isArray(actual) ? actual.some((item) => values.includes(item)) : values.includes(actual);
  }
  if (rule.op === "contains") {
    const values = Array.isArray(expected) ? expected : [expected];
    return Array.isArray(actual)
      ? values.every((item) => actual.includes(item))
      : !Array.isArray(expected) && String(actual ?? "").toLocaleLowerCase().includes(String(expected).toLocaleLowerCase());
  }
  if (rule.op === "prefix") return String(actual ?? "").toLocaleLowerCase().startsWith(String(expected).toLocaleLowerCase());
  const left = numeric(actual);
  const right = numeric(expected);
  if (left === null || right === null) return false;
  if (rule.op === "gt") return left > right;
  if (rule.op === "gte") return left >= right;
  if (rule.op === "lt") return left < right;
  if (rule.op === "lte") return left <= right;
  return false;
}

function matchesGroup(item: Item, group: { match: "all" | "any"; filters: LensFilter[] }): boolean {
  if (group.filters.length === 0) return true;
  const results = group.filters.map((rule) => matchesFilter(item, rule));
  return group.match === "all" ? results.every(Boolean) : results.some(Boolean);
}

function compareValue(value: unknown): string {
  return String(value ?? "").toLocaleLowerCase();
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function sortItems<T extends Item>(items: Iterable<T>, rules: SortRule[]): T[] {
  const result = [...items];
  result.sort((left, right) => {
    for (const rule of rules) {
      const a = compareValue(fieldValue(left, rule.field));
      const b = compareValue(fieldValue(right, rule.field));
      const compared = compareText(a, b);
      if (compared !== 0) return rule.direction === "desc" ? -compared : compared;
    }
    return compareText(String(left.id ?? ""), String(right.id ?? ""));
  });
  return result;
}

function neighbors(relation: KnowledgeRelation, nodeId: string, direction: LensSpec["traversal"]["direction"]): string[] {
  const result: string[] = [];
  if ((direction === "outgoing" || direction === "either") && relation.from_id === nodeId) result.push(relation.to_id);
  if ((direction === "incoming" || direction === "either") && relation.to_id === nodeId) result.push(relation.from_id);
  return result;
}

function buildGroups(nodes: KnowledgeNode[], relations: KnowledgeRelation[], fields: string[], limit: number): Item[] {
  const result: Item[] = [];
  for (const field of fields) {
    const groups = new Map<string, { field: string; value: unknown; node_ids: string[]; relation_ids: string[] }>();
    for (const [kind, items] of [["node", nodes], ["relation", relations]] as const) {
      for (const item of items) {
        const raw = fieldValue(item, field);
        const members = Array.isArray(raw) ? raw : [raw];
        for (const member of members) {
          if (member === null || member === undefined) continue;
          const key = String(member);
          const entry = groups.get(key) ?? { field, value: member, node_ids: [], relation_ids: [] };
          (kind === "node" ? entry.node_ids : entry.relation_ids).push(item.id);
          groups.set(key, entry);
        }
      }
    }
    for (const key of [...groups.keys()].sort(compareText)) {
      const entry = groups.get(key)!;
      result.push({ ...entry, node_count: entry.node_ids.length, relation_count: entry.relation_ids.length });
      if (result.length >= limit) return result;
    }
  }
  return result;
}

function canonicalFingerprintValue(value: unknown): string {
  if (value === null) return "n;";
  if (typeof value === "boolean") return value ? "b1;" : "b0;";
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("stable digest cannot encode a non-finite number");
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setFloat64(0, Object.is(value, -0) ? 0 : value, false);
    return `d${[...bytes].map((item) => item.toString(16).padStart(2, "0")).join("")};`;
  }
  if (typeof value === "string") {
    return `s${new TextEncoder().encode(value).byteLength}:${value}`;
  }
  if (Array.isArray(value)) {
    return `a${value.length}[${value.map(canonicalFingerprintValue).join("")}]`;
  }
  if (value && typeof value === "object") {
    const source = value as Item;
    const keys = Object.keys(source).sort();
    return `o${keys.length}{${keys.map((key) => canonicalFingerprintValue(key) + canonicalFingerprintValue(source[key])).join("")}}`;
  }
  throw new Error(`stable digest cannot encode ${typeof value}`);
}

async function digest(value: unknown): Promise<string> {
  const bytes = new TextEncoder().encode(canonicalFingerprintValue(value));
  const hash = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(hash)].map((item) => item.toString(16).padStart(2, "0")).join("");
}

export function lensCarrier<T extends KnowledgeNode | KnowledgeRelation>(item: T, detail: LensSpec['detail'], language: string): T {
    const result = {...item, display_selection: displaySelection(item, language)};
    if (Object.hasOwn(record(item.attributes), 'human_forms')) Object.assign(result, {human_form_selection: selectHumanForms(item, language)});
  if (detail !== 'full') { result.attributes = {}; delete result.source_record; }
  return result;
}

export type LensExecutionCounts = {
  available_nodes: number;
  available_relations: number;
  matched_nodes: number;
  matched_relations: number;
  eligible_relations: number;
  identity_expansion_limited: boolean;
};

export async function finalizeKnowledgeLens(
  authorityBoundary: Item,
  sourceRevision: string,
  spec: LensSpec,
  selectedNodes: Iterable<KnowledgeNode>,
  selectedRelations: Iterable<KnowledgeRelation>,
  executionCounts: LensExecutionCounts,
  resolvedFocusNodeId: string | null = null,
  inclusion?: Inclusion,
): Promise<Item & { nodes: KnowledgeNode[]; relations: KnowledgeRelation[]; presentation: LensSpec["presentation"]; fingerprint: string; authority_boundary: Item }> {
  const finalNodes = sortItems(selectedNodes, spec.composition.sort_nodes);
  const finalRelations = sortItems(selectedRelations, spec.composition.sort_relations);
  const requestedFocus = spec.seed.focus_node_id;
  let focus: Item | null = null;
  if (requestedFocus !== null) {
    let focusedNode = resolvedFocusNodeId === null
      ? finalNodes.find((item) => item.id === requestedFocus)
      : finalNodes.find((item) => item.id === resolvedFocusNodeId);
    if (resolvedFocusNodeId !== null && !focusedNode) {
      throw new Error("resolved knowledge focus is missing from the lens result");
    }
    let resolvedBy = focusedNode?.id === requestedFocus
      ? "id"
      : focusedNode?.entity_id === requestedFocus
        ? "entity_id"
        : "native_id";
    if (!focusedNode) {
      focusedNode = finalNodes.find((item) => item.entity_id === requestedFocus);
      resolvedBy = "entity_id";
    }
    if (!focusedNode) {
      focusedNode = finalNodes.find((item) => item.native_id === requestedFocus);
      resolvedBy = "native_id";
    }
    if (!focusedNode) throw new Error("resolved knowledge focus is missing from the lens result");
    focus = {
      requested_id: requestedFocus,
      resolved_by: resolvedBy,
      node_id: focusedNode.id,
      entity_id: focusedNode.entity_id,
      native_id: focusedNode.native_id,
      source_graph: focusedNode.source_graph,
      kind_id: focusedNode.kind_id,
      type_id: focusedNode.type_id,
      display: focusedNode.display,
    };
  }
  const groups = buildGroups(finalNodes, finalRelations, spec.composition.group_by, spec.limits.groups);
  const sourceRefs = [...new Set([...finalNodes, ...finalRelations].flatMap((item) => item.source_refs))].sort();
  const missingNodeSummaries = finalNodes.filter((item) => item.display.summary_state === "missing").length;
  const missingRelationExplanations = finalRelations.filter((item) => item.display.explanation_state === "missing").length;
  const nodesWithoutSourceSummary = finalNodes.filter((item) => item.display.provenance.source_summary_available === false).length;
  const relationsWithoutSourceExplanation = finalRelations.filter((item) => item.display.provenance.source_explanation_available === false).length;
  const truncatedNodes = Math.max(0, executionCounts.matched_nodes - spec.limits.nodes);
  const truncatedRelations = Math.max(0, executionCounts.eligible_relations - finalRelations.length);
  const fingerprint = await digest({
    execution_version: "tos-lens-execution-v6",
    source_revision: sourceRevision,
    lens: Object.fromEntries(Object.entries(spec).filter(([key]) => key !== 'pagination')),
    nodes: finalNodes.map((item) => [item.id, item.content_revision ?? ""]),
    relations: finalRelations.map((item) => [item.id, item.content_revision ?? ""]),
    groups,
  });
  const countBy = (values: string[]): Item => Object.fromEntries([...new Set(values)].sort().map((value) => [value, values.filter((item) => item === value).length]));
  const result = paginateLens({
    schema: "tos_lens_result_v1",
    source_revision: sourceRevision,
    lens: spec,
    fingerprint,
    presentation: spec.presentation,
    focus,
    ...(spec.explain ? { inclusion } : {}),
    nodes: finalNodes.map((item) => lensCarrier(item, spec.detail, spec.language)),
    relations: finalRelations.map((item) => lensCarrier(item, spec.detail, spec.language)),
    groups,
    facets: {
      node_kinds: countBy(finalNodes.map((item) => item.kind_id)),
      predicates: countBy(finalRelations.map((item) => item.predicate_id)),
      sources: countBy(finalNodes.map((item) => item.source_graph)),
    },
    counts: {
      ...executionCounts,
      nodes: finalNodes.length,
      relations: finalRelations.length,
      groups: groups.length,
      truncated_nodes: truncatedNodes,
      truncated_relations: truncatedRelations,
      missing_node_summaries: missingNodeSummaries,
      missing_relation_explanations: missingRelationExplanations,
      nodes_without_source_summary: nodesWithoutSourceSummary,
      relations_without_source_explanation: relationsWithoutSourceExplanation,
    },
    source_refs: sourceRefs,
    warnings: [
      ...(executionCounts.identity_expansion_limited ? ['identity carrier expansion reached the node budget; use resumable exploration or narrower sources'] : []),
      ...(missingNodeSummaries ? [`${missingNodeSummaries} nodes expose an explicit missing-summary state`] : []),
      ...(missingRelationExplanations ? [`${missingRelationExplanations} relations expose an explicit missing-explanation state`] : []),
      ...(nodesWithoutSourceSummary ? [`${nodesWithoutSourceSummary} nodes use transparent metadata synthesis because no source summary is projected`] : []),
      ...(relationsWithoutSourceExplanation ? [`${relationsWithoutSourceExplanation} relations use transparent metadata synthesis because no source explanation is projected`] : []),
      ...(truncatedNodes ? [`node selector exceeded its bounded result by ${truncatedNodes} nodes`] : []),
      ...(truncatedRelations ? [`relation selector exceeded its bounded result by ${truncatedRelations} relations`] : []),
    ],
    authority_boundary: authorityBoundary,
    agent_summary: {
      lens_id: spec.lens_id,
      focus_node_id: focus ? focus.node_id : null,
      node_count: finalNodes.length,
      relation_count: finalRelations.length,
      group_count: groups.length,
      source_ref_count: sourceRefs.length,
      is_source: false,
      writes_to_tree: false,
    },
  }, spec.pagination);
  return {...result, scene: knowledgeScene(result.nodes, result.relations,
    result.focus ? String((result.focus as Item).node_id) : null)};
}

export async function executeKnowledgeLens(graph: KnowledgeGraph, specValue: unknown): Promise<Item & { nodes: KnowledgeNode[]; relations: KnowledgeRelation[]; presentation: LensSpec["presentation"]; fingerprint: string; authority_boundary: Item }> {
  const publicSpec = normalizeLensSpec(specValue);
  const spec = bindQueryProperties(graph.query_properties ?? [], publicSpec);
  const sourceRevision = graph.source_revision || await digest({ nodes: graph.nodes, relations: graph.relations });
  const sourceSet = new Set(spec.sources);
  const nodes = graph.nodes.filter((item) => sourceSet.has(item.source_graph));
  const relations = graph.relations.filter((item) => sourceSet.has(item.source_graph));
  const allNodes = new Map(nodes.map((item) => [item.id, item]));
  const requestedFocus = spec.seed.focus_node_id;
  let focusNode: KnowledgeNode | null = null;
  if (requestedFocus !== null) {
    focusNode = nodes.find((item) => item.id === requestedFocus) ?? null;
    if (!focusNode) {
      const sourcePriority: Record<string, number> = {
        "source-navigation": 0,
        canon: 1,
        "source-claims": 2,
        philosophy: 3,
        "candidate-intake": 4,
        repository: 5,
        "semantic-interchange": 6,
      };
      const entityMatches = nodes.filter((item) => item.entity_id === requestedFocus);
      entityMatches.sort((left, right) =>
        (sourcePriority[left.source_graph] ?? 99) - (sourcePriority[right.source_graph] ?? 99)
        || compareText(left.id, right.id));
      focusNode = entityMatches[0] ?? null;
    }
    if (!focusNode) {
      const matches = nodes.filter((item) => item.native_id === requestedFocus);
      if (matches.length === 0) throw new Error(`unknown ToS knowledge focus: ${requestedFocus}`);
      if (matches.length > 1) {
        const ids = matches.map((item) => item.id).sort(compareText).join(", ");
        throw new Error(`ambiguous ToS knowledge focus ${requestedFocus}: ${ids}; use a namespaced node id`);
      }
      focusNode = matches[0]!;
    }
  }
  const seedIds = new Set(spec.seed.node_ids);
  const adjacency = new Map<string, KnowledgeRelation[]>();
  for (const relation of [...relations].sort((a, b) => compareText(a.id, b.id))) {
    for (const endpoint of new Set([relation.from_id, relation.to_id])) {
      const list = adjacency.get(endpoint) ?? [];
      list.push(relation); adjacency.set(endpoint, list);
    }
  }
  let pathBudget = 100_000;
  const pathWitness = (start: string, condition: PathCondition): Item | null => {
    const walk = (current: string, index: number, nodeIds: string[], relationIds: string[]): Item | null => {
      if (index === condition.steps.length) return { path_id: condition.path_id, node_ids: nodeIds, relation_ids: relationIds };
      const step = condition.steps[index]!;
      for (const relation of adjacency.get(current) ?? []) {
        if (--pathBudget < 0) throw new Error("path query exceeded the execution safety ceiling");
        if (!step.relation_query.enabled || !matchesGroup(relation, step.relation_query)) continue;
        for (const neighbor of neighbors(relation, current, step.direction)) {
          const node = allNodes.get(neighbor);
          if (!node || !step.node_query.enabled || !matchesGroup(node, step.node_query)) continue;
          const found = walk(neighbor, index + 1, [...nodeIds, neighbor], [...relationIds, relation.id]);
          if (found) return found;
        }
      }
      return null;
    };
    return walk(start, 0, [start], []);
  };
  const proofs = new Map<string, Item[]>();
  const needle = spec.seed.text_query.toLocaleLowerCase();
  const base = spec.node_query.enabled ? nodes.filter((item) => {
    if (seedIds.size > 0 && !seedIds.has(item.id) && !seedIds.has(item.entity_id) && !seedIds.has(item.native_id)) return false;
    if (needle && !JSON.stringify(item).toLocaleLowerCase().includes(needle)) return false;
    if (!matchesGroup(item, spec.node_query)) return false;
    const paths: Item[] = [];
    for (const condition of spec.path_query) {
      const witness = pathWitness(item.id, condition);
      if ((witness !== null) !== (condition.quantifier === "exists")) return false;
      paths.push(witness ?? { path_id: condition.path_id, absence_in_scope: true });
    }
    proofs.set(item.id, paths);
    return true;
  }) : [];
  const selectedNodes = new Map<string, KnowledgeNode>();
  const inclusion: Inclusion = { nodes: {}, relations: {}, authority: "query-execution-not-semantic-proof" };
  if (focusNode) { selectedNodes.set(focusNode.id, focusNode); inclusion.nodes[focusNode.id] = { kind: "focus" }; }
  for (const item of sortItems(base, spec.composition.sort_nodes)) {
    if (selectedNodes.size >= spec.limits.nodes) break;
    if (!selectedNodes.has(item.id)) { selectedNodes.set(item.id, item); inclusion.nodes[item.id] = { kind: "selector", path_witnesses: proofs.get(item.id) ?? [] }; }
  }
  const predicateSet = new Set(spec.traversal.predicate_ids);
  const relationCandidates = sortItems(
    spec.relation_query.enabled
      ? relations.filter((item) => matchesGroup(item, spec.relation_query) && (predicateSet.size === 0 || predicateSet.has(item.predicate_id))
        && (spec.traversal.profile !== "overview" || (!OVERVIEW_EXCLUDED_PREDICATES.includes(item.predicate_id)
          && !OVERVIEW_EXCLUDED_RELATION_TYPES.includes(item.relation_type_id))))
      : [],
    spec.composition.sort_relations,
  );

  let frontier = [...selectedNodes.keys()];
  let identityExpansionLimited = false;
  const carrierGroups = new Map<string, string[]>();
  if (spec.traversal.profile === 'overview') for (const node of nodes) {
    if (!node.entity_id?.startsWith('tos.')) continue;
    if (!carrierGroups.has(node.entity_id)) carrierGroups.set(node.entity_id, []);
    carrierGroups.get(node.entity_id)!.push(node.id);
  }
  const traversed = new Set<string>();
  for (let depth = 0; depth < spec.traversal.depth; depth += 1) {
    const origins = new Map<string, string>();
    for (const id of frontier.slice().sort(compareIds)) {
      const entity = allNodes.get(id)!.entity_id;
      if (carrierGroups.has(entity) && !origins.has(entity)) origins.set(entity, id);
    }
    const aliases = [...new Set([...origins.keys()].flatMap(entity => carrierGroups.get(entity)!))]
      .filter(id => !selectedNodes.has(id)).sort(compareIds);
    for (const id of aliases) {
      if (selectedNodes.size >= spec.limits.nodes) { identityExpansionLimited = true; break; }
      const node = allNodes.get(id)!;
      selectedNodes.set(id, node);
      inclusion.nodes[id] = {kind: 'identity-carrier', via_node_id: origins.get(node.entity_id), entity_id: node.entity_id, depth};
      frontier.push(id);
    }
    const current = new Set(frontier);
    const next: string[] = [];
    for (const relation of relationCandidates) {
      let touched = false;
      for (const nodeId of current) {
        for (const neighbor of neighbors(relation, nodeId, spec.traversal.direction)) {
          touched = true;
          const node = allNodes.get(neighbor);
          if (node && !selectedNodes.has(neighbor) && selectedNodes.size < spec.limits.nodes) {
            selectedNodes.set(neighbor, node);
            inclusion.nodes[neighbor] = { kind: "traversal", via_node_id: nodeId, via_relation_id: relation.id, depth: depth + 1 };
            next.push(neighbor);
          }
        }
      }
      if (touched && traversed.size < spec.limits.relations) traversed.add(relation.id);
    }
    frontier = [...new Set(next)];
    if (frontier.length === 0) break;
  }

  const selectionBasis = new Set(selectedNodes.keys());
  const selectedRelations: KnowledgeRelation[] = [];
  let eligibleRelations = 0;
  for (const relation of relationCandidates) {
    const left = selectionBasis.has(relation.from_id);
    const right = selectionBasis.has(relation.to_id);
    const allowed =
      (spec.composition.endpoint_policy === "both" && left && right)
      || (spec.composition.endpoint_policy === "either" && (left || right))
      || spec.composition.endpoint_policy === "independent"
      || traversed.has(relation.id);
    if (!allowed) continue;
    eligibleRelations += 1;
    if (selectedRelations.length >= spec.limits.relations) continue;
    const missing = [...new Set([relation.from_id, relation.to_id].filter((id) => !selectedNodes.has(id)))];
    if (selectedNodes.size + missing.length > spec.limits.nodes) continue;
    for (const id of missing) {
      const node = allNodes.get(id);
      if (node) { selectedNodes.set(id, node); inclusion.nodes[id] = { kind: "endpoint", via_relation_id: relation.id }; }
    }
    if (selectedNodes.has(relation.from_id) && selectedNodes.has(relation.to_id)) {
      selectedRelations.push(relation);
      inclusion.relations[relation.id] = { kind: traversed.has(relation.id) ? "traversal" : "endpoint-policy", endpoint_policy: spec.composition.endpoint_policy };
    }
  }

  const matchedNodeIds = new Set(base.map((item) => item.id));
  if (focusNode) matchedNodeIds.add(focusNode.id);
  return finalizeKnowledgeLens(graph.authority_boundary, sourceRevision, publicSpec, selectedNodes.values(), selectedRelations, {
    available_nodes: nodes.length,
    available_relations: relations.length,
    matched_nodes: matchedNodeIds.size,
    matched_relations: relationCandidates.length,
    eligible_relations: eligibleRelations,
    identity_expansion_limited: identityExpansionLimited,
  }, focusNode?.id ?? null, inclusion);
}

export type FocusKnowledgeOptions = {
  profile?: "all" | "overview";
  sources?: string[];
  depth?: number;
  direction?: "outgoing" | "incoming" | "either";
  predicateIds?: string[];
  nodeLimit?: number;
  relationLimit?: number;
};

export function focusLensSpec(nodeId: string, options: FocusKnowledgeOptions = {}): LensSpec {
  const identifier = text(nodeId);
  if (!identifier) throw new Error("knowledge focus node id is required");
  return normalizeLensSpec({
    schema_version: "tos_lens_spec_v1",
    lens_id: "focus-neighborhood",
    title: { default: `Focus: ${identifier}` },
    description: { default: `Bounded knowledge neighborhood centered on ${identifier}.` },
    sources: options.sources ?? [...SOURCES],
    seed: { focus_node_id: identifier },
    node_query: { enabled: false },
    relation_query: { enabled: true },
    traversal: {
      depth: options.depth ?? 1,
      direction: options.direction ?? "either",
      predicate_ids: options.predicateIds ?? [],
      profile: options.profile ?? "overview",
    },
    composition: {
      endpoint_policy: "both",
      group_by: [],
      sort_nodes: [{ field: "id", direction: "asc" }],
      sort_relations: [{ field: "id", direction: "asc" }],
    },
    presentation: {
      layout: "radial",
      color_by: "kind_id",
      lane_by: "epistemic.authority_layer",
      size_by: null,
      inspector_fields: ["display", "epistemic", "source_refs", "attributes"],
    },
    limits: {
      nodes: options.nodeLimit ?? 200,
      relations: options.relationLimit ?? 400,
      groups: 100,
    },
  });
}

export async function focusKnowledgeNode(
  graph: KnowledgeGraph,
  nodeId: string,
  options: FocusKnowledgeOptions = {},
): Promise<Item & { nodes: KnowledgeNode[]; relations: KnowledgeRelation[]; presentation: LensSpec["presentation"]; fingerprint: string; authority_boundary: Item }> {
  return executeKnowledgeLens(graph, focusLensSpec(nodeId, options));
}
