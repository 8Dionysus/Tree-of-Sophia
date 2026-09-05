import { HttpError, parseItem, type Item } from "./common.ts";
import {
  finalizeKnowledgeLens,
  focusLensSpec,
  normalizeLensSpec,
  OVERVIEW_EXCLUDED_PREDICATES,
  type FocusKnowledgeOptions,
  type KnowledgeNode,
  type KnowledgeRelation,
  type LensFilter,
  type LensSpec,
  type SortRule,
  type PathCondition,
  type Inclusion,
} from "./knowledge.ts";
import { jsonRows, meta, rows } from "./store.ts";

const KNOWLEDGE_SOURCES = new Set(["philosophy", "canon", "candidate-intake", "source-navigation", "source-claims", "semantic-interchange", "repository"]);
const PAGE_SIZE = 2000;

async function consistentRead(db: D1Database, read: () => Promise<Item>): Promise<Item> {
  const before = await meta<Item>(db, "data_revision");
  const result = await read();
  const after = await meta<Item>(db, "data_revision");
  if (before.sha256 !== after.sha256) throw new HttpError(409, "knowledge snapshot changed during query; retry against the current revision");
  return result;
}

export async function executeKnowledgeLensD1(db: D1Database, specValue: unknown): Promise<Item> {
  return consistentRead(db, () => executeKnowledgeLensD1Unchecked(db, specValue));
}

export async function knowledgeSearchD1(db: D1Database, options: Parameters<typeof knowledgeSearchD1Unchecked>[1]): Promise<Item> {
  return consistentRead(db, () => knowledgeSearchD1Unchecked(db, options));
}

export async function knowledgeNodeD1(db: D1Database, id: string, relationLimit: number): Promise<Item> {
  return consistentRead(db, () => knowledgeNodeD1Unchecked(db, id, relationLimit));
}

export async function knowledgeRelationD1(db: D1Database, id: string): Promise<Item> {
  return consistentRead(db, () => knowledgeRelationD1Unchecked(db, id));
}

type ItemKind = "node" | "relation";
type SqlFragment = { sql: string; bindings: unknown[] };
type RelationHeader = { id: string; from_id: string; to_id: string; from_source: string; to_source: string };
type JsonRow = { json: string };
type CountRow = { count: number };

function bound(value: unknown): string | number | null {
  if (typeof value === "boolean") return value ? 1 : 0;
  if (typeof value === "string" || typeof value === "number" || value === null) return value;
  throw new Error("knowledge filter values must be scalar");
}

function jsonPath(field: string): string {
  return `$.${field.split(".").map((segment) => `"${segment}"`).join(".")}`;
}

function fieldExpression(alias: string, field: string, kind: ItemKind): string {
  const columns: Record<string, string> = kind === "node"
    ? { id: "id", entity_id: "entity_id", native_id: "native_id", source_graph: "source_graph", kind_id: "kind_id", type_id: "type_id" }
    : {
      id: "id", native_id: "native_id", source_graph: "source_graph", from_id: "from_id",
      to_id: "to_id", predicate_id: "predicate_id", relation_type_id: "relation_type_id",
    };
  const column = columns[field];
  if (column) return `${alias}.${column}`;
  return `json_extract(${alias}.json, '${jsonPath(field)}')`;
}

function jsonTypeExpression(alias: string, field: string): string {
  return `json_type(${alias}.json, '${jsonPath(field)}')`;
}

function equalityFilter(alias: string, field: string, kind: ItemKind, expected: unknown): SqlFragment {
  const expression = fieldExpression(alias, field, kind);
  const value = bound(expected);
  const type = jsonTypeExpression(alias, field);
  return {
    sql: `(((${type} != 'array' OR ${type} IS NULL) AND ${expression} IS ?) OR (${type} = 'array' AND EXISTS (SELECT 1 FROM json_each(${expression}) actual WHERE actual.value IS ?)))`,
    bindings: [value, value],
  };
}

function filterFragment(alias: string, rule: LensFilter, kind: ItemKind): SqlFragment {
  const expression = fieldExpression(alias, rule.field, kind);
  const type = jsonTypeExpression(alias, rule.field);
  if (rule.op === "exists") {
    return { sql: `${expression} IS ${rule.value ? "NOT " : ""}NULL`, bindings: [] };
  }
  if (rule.op === "eq" || rule.op === "neq") {
    const equal = equalityFilter(alias, rule.field, kind, rule.value);
    return rule.op === "eq" ? equal : { sql: `NOT ${equal.sql}`, bindings: equal.bindings };
  }
  if (rule.op === "in") {
    const values = Array.isArray(rule.value) ? rule.value : [rule.value];
    const serialized = JSON.stringify(values.map(bound));
    return {
      sql: `(((${type} != 'array' OR ${type} IS NULL) AND EXISTS (SELECT 1 FROM json_each(?) expected WHERE ${expression} IS expected.value)) OR (${type} = 'array' AND EXISTS (SELECT 1 FROM json_each(${expression}) actual WHERE EXISTS (SELECT 1 FROM json_each(?) expected WHERE actual.value IS expected.value))))`,
      bindings: [serialized, serialized],
    };
  }
  if (rule.op === "contains") {
    const values = Array.isArray(rule.value) ? rule.value : [rule.value];
    const serialized = JSON.stringify(values.map(bound));
    const arraySql = `(${type} = 'array' AND NOT EXISTS (SELECT 1 FROM json_each(?) expected WHERE NOT EXISTS (SELECT 1 FROM json_each(${expression}) actual WHERE actual.value IS expected.value)))`;
    if (Array.isArray(rule.value)) return { sql: arraySql, bindings: [serialized] };
    return {
      sql: `(${arraySql} OR (${type} != 'array' AND instr(lower(CAST(${expression} AS TEXT)), ?) > 0))`,
      bindings: [serialized, String(rule.value ?? "").toLocaleLowerCase()],
    };
  }
  if (rule.op === "prefix") {
    const expected = String(rule.value ?? "").toLocaleLowerCase();
    return {
      sql: `(${type} != 'array' AND substr(lower(CAST(${expression} AS TEXT)), 1, ?) = ?)`,
      bindings: [expected.length, expected],
    };
  }
  const comparator = { gt: ">", gte: ">=", lt: "<", lte: "<=" }[rule.op];
  if (!comparator) throw new Error(`unsupported knowledge filter operator: ${rule.op}`);
  return {
    sql: `(${type} IN ('integer', 'real') AND CAST(${expression} AS REAL) ${comparator} ?)`,
    bindings: [bound(rule.value)],
  };
}

function groupFragment(alias: string, group: LensSpec["node_query"] | LensSpec["relation_query"], kind: ItemKind): SqlFragment {
  if (!group.enabled) return { sql: "0 = 1", bindings: [] };
  if (group.filters.length === 0) return { sql: "1 = 1", bindings: [] };
  const fragments = group.filters.map((rule) => filterFragment(alias, rule, kind));
  return {
    sql: `(${fragments.map((item) => item.sql).join(group.match === "all" ? " AND " : " OR ")})`,
    bindings: fragments.flatMap((item) => item.bindings),
  };
}

function sourceFragment(alias: string, sources: string[]): SqlFragment {
  return {
    sql: `${alias}.source_graph IN (SELECT value FROM json_each(?))`,
    bindings: [JSON.stringify(sources)],
  };
}

function orderClause(alias: string, rules: SortRule[], kind: ItemKind): string {
  const fields = rules.map((rule) => {
    const expression = fieldExpression(alias, rule.field, kind);
    return `lower(CAST(${expression} AS TEXT)) ${rule.direction.toUpperCase()}`;
  });
  fields.push(`${alias}.id ASC`);
  return fields.join(", ");
}

function joinFragments(parts: SqlFragment[]): SqlFragment {
  return {
    sql: parts.map((item) => `(${item.sql})`).join(" AND "),
    bindings: parts.flatMap((item) => item.bindings),
  };
}

// Bounded, correlated joins over the existing adjacency indexes. All values
// are bound; aliases and SQL operators are generated only by this compiler.
function pathPlan(condition: PathCondition, sources: string[]) {
  const joins: string[] = [];
  const filters: SqlFragment[] = [];
  const columns: string[] = [];
  const order: string[] = [];
  let previous = "n.id";
  condition.steps.forEach((step, index) => {
    const r = `pr${index}`, node = `pn${index}`;
    const adjacency = step.direction === "outgoing" ? `${r}.from_id = ${previous}`
      : step.direction === "incoming" ? `${r}.to_id = ${previous}`
      : `(${r}.from_id = ${previous} OR ${r}.to_id = ${previous})`;
    const endpoint = step.direction === "outgoing" ? `${r}.to_id`
      : step.direction === "incoming" ? `${r}.from_id`
      : `CASE WHEN ${r}.from_id = ${previous} THEN ${r}.to_id ELSE ${r}.from_id END`;
    joins.push(`${index === 0 ? "" : "JOIN "}knowledge_relations ${r} ${index === 0 ? "" : `ON ${adjacency}`}`);
    if (index === 0) filters.push({ sql: adjacency, bindings: [] });
    joins.push(`JOIN knowledge_nodes ${node} ON ${node}.id = ${endpoint}`);
    filters.push(sourceFragment(r, sources), sourceFragment(node, sources),
      groupFragment(r, step.relation_query, "relation"), groupFragment(node, step.node_query, "node"));
    columns.push(`${r}.id AS r${index}`, `${node}.id AS n${index}`);
    order.push(`${r}.id`, `${node}.id`);
    previous = `${node}.id`;
  });
  return { from: joins.join(" "), where: joinFragments(filters), columns: columns.join(", "), order: order.join(", ") };
}

function pathConditionFragment(condition: PathCondition, sources: string[]): SqlFragment {
  const plan = pathPlan(condition, sources);
  return { sql: `${condition.quantifier === "not_exists" ? "NOT " : ""}EXISTS (SELECT 1 FROM ${plan.from} WHERE ${plan.where.sql})`, bindings: plan.where.bindings };
}

async function pathProofs(db: D1Database, nodeId: string, spec: LensSpec): Promise<Item[]> {
  const proofs: Item[] = [];
  for (const condition of spec.path_query) {
    if (condition.quantifier === "not_exists") { proofs.push({ path_id: condition.path_id, absence_in_scope: true }); continue; }
    const plan = pathPlan(condition, spec.sources);
    const found = await rows<Record<string, string>>(db,
      `SELECT ${plan.columns} FROM knowledge_nodes n CROSS JOIN ${plan.from} WHERE n.id = ? AND ${plan.where.sql} ORDER BY ${plan.order} LIMIT 1`,
      nodeId, ...plan.where.bindings);
    if (!found[0]) throw new HttpError(409, "path witness changed during query; retry against the current revision");
    proofs.push({ path_id: condition.path_id,
      node_ids: [nodeId, ...condition.steps.map((_, index) => found[0]![`n${index}`])],
      relation_ids: condition.steps.map((_, index) => found[0]![`r${index}`]) });
  }
  return proofs;
}

async function count(db: D1Database, table: string, where: SqlFragment): Promise<number> {
  const result = await rows<CountRow>(db, `SELECT COUNT(*) AS count FROM ${table} WHERE ${where.sql}`, ...where.bindings);
  return Number(result[0]?.count ?? 0);
}

function asNodes(items: Item[]): KnowledgeNode[] {
  return items as KnowledgeNode[];
}

function asRelations(items: Item[]): KnowledgeRelation[] {
  return items as KnowledgeRelation[];
}

export async function nodesByIds(db: D1Database, ids: Iterable<string>): Promise<KnowledgeNode[]> {
  const values = [...new Set(ids)];
  if (values.length === 0) return [];
  return asNodes(await jsonRows(
    db,
    "SELECT json FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id",
    JSON.stringify(values),
  ));
}

export async function relationsByIds(db: D1Database, ids: Iterable<string>): Promise<KnowledgeRelation[]> {
  const values = [...new Set(ids)];
  if (values.length === 0) return [];
  return asRelations(await jsonRows(
    db,
    "SELECT json FROM knowledge_relations WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id",
    JSON.stringify(values),
  ));
}

export async function resolveFocusNodeD1(
  db: D1Database,
  requestedId: string | null,
  sources: string[],
): Promise<KnowledgeNode | null> {
  if (requestedId === null) return null;
  const source = sourceFragment("n", sources);
  const exact = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.id = ? ORDER BY n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (exact.length > 0) return exact[0]!;
  const entity = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.entity_id = ? ORDER BY CASE n.source_graph WHEN 'source-navigation' THEN 0 WHEN 'canon' THEN 1 WHEN 'source-claims' THEN 2 WHEN 'philosophy' THEN 3 WHEN 'candidate-intake' THEN 4 WHEN 'repository' THEN 5 WHEN 'semantic-interchange' THEN 6 ELSE 99 END, n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (entity.length > 0) return entity[0]!;
  const native = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.native_id = ? ORDER BY n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (native.length === 0) throw new HttpError(400, `unknown ToS knowledge focus: ${requestedId}`);
  if (native.length > 1) {
    throw new HttpError(
      400,
      `ambiguous ToS knowledge focus ${requestedId}: ${native.map((item) => item.id).join(", ")}; use a namespaced node id`,
    );
  }
  return native[0]!;
}

async function allRelationHeaders(
  db: D1Database,
  where: SqlFragment,
  orderBy: string,
): Promise<RelationHeader[]> {
  const result: RelationHeader[] = [];
  let offset = 0;
  while (true) {
    const page = await rows<RelationHeader>(
      db,
      `SELECT r.id, r.from_id, r.to_id,
        (SELECT source_graph FROM knowledge_nodes WHERE id = r.from_id) AS from_source,
        (SELECT source_graph FROM knowledge_nodes WHERE id = r.to_id) AS to_source
       FROM knowledge_relations r WHERE ${where.sql} ORDER BY ${orderBy} LIMIT ? OFFSET ?`,
      ...where.bindings,
      PAGE_SIZE,
      offset,
    );
    result.push(...page);
    if (page.length < PAGE_SIZE) return result;
    offset += page.length;
    if (offset > 100_000) throw new Error("knowledge relation selector exceeded the execution safety ceiling");
  }
}

// The adjacency condition is a separate indexed subquery: adding source/JSON
// filters must not make SQLite choose the global source index for a local hop.
function adjacency(ids: Iterable<string>, direction: "outgoing" | "incoming" | "either"): SqlFragment {
  const value = JSON.stringify([...ids]);
  const parts = [];
  const bindings: string[] = [];
  if (direction !== "incoming") {
    parts.push("SELECT id FROM knowledge_relations WHERE from_id IN (SELECT value FROM json_each(?))");
    bindings.push(value);
  }
  if (direction !== "outgoing") {
    parts.push("SELECT id FROM knowledge_relations WHERE to_id IN (SELECT value FROM json_each(?))");
    bindings.push(value);
  }
  return { sql: `r.id IN (${parts.join(" UNION ")})`, bindings };
}

async function localRelationHeaders(db: D1Database, where: SqlFragment, ids: Iterable<string>,
  direction: "outgoing" | "incoming" | "either", orderBy: string): Promise<RelationHeader[]> {
  const local = joinFragments([where, adjacency(ids, direction)]);
  return allRelationHeaders(db, local, orderBy);
}

function neighborIds(relation: RelationHeader, nodeId: string, direction: LensSpec["traversal"]["direction"]): string[] {
  const result: string[] = [];
  if ((direction === "outgoing" || direction === "either") && relation.from_id === nodeId) result.push(relation.to_id);
  if ((direction === "incoming" || direction === "either") && relation.to_id === nodeId) result.push(relation.from_id);
  return result;
}

async function executeKnowledgeLensD1Unchecked(db: D1Database, specValue: unknown): Promise<Item> {
  const spec = normalizeLensSpec(specValue);
  const focusNode = await resolveFocusNodeD1(db, spec.seed.focus_node_id, spec.sources);
  const source = sourceFragment("n", spec.sources);
  const nodeParts: SqlFragment[] = [source, groupFragment("n", spec.node_query, "node")];
  nodeParts.push(...spec.path_query.map((condition) => pathConditionFragment(condition, spec.sources)));
  if (spec.seed.node_ids.length > 0) {
    nodeParts.push({
      sql: "(n.id IN (SELECT value FROM json_each(?)) OR n.entity_id IN (SELECT value FROM json_each(?)) OR n.native_id IN (SELECT value FROM json_each(?)))",
      bindings: [JSON.stringify(spec.seed.node_ids), JSON.stringify(spec.seed.node_ids), JSON.stringify(spec.seed.node_ids)],
    });
  }
  if (spec.seed.text_query) {
    nodeParts.push({ sql: "instr(n.search_text, ?) > 0", bindings: [spec.seed.text_query.toLocaleLowerCase()] });
  }
  const nodeWhere = joinFragments(nodeParts);
  const relationParts: SqlFragment[] = [
    sourceFragment("r", spec.sources),
    groupFragment("r", spec.relation_query, "relation"),
  ];
  if (spec.traversal.predicate_ids.length > 0) {
    relationParts.push({
      sql: "r.predicate_id IN (SELECT value FROM json_each(?))",
      bindings: [JSON.stringify(spec.traversal.predicate_ids)],
    });
  }
  if (spec.traversal.profile === "overview") {
    relationParts.push({sql: "r.predicate_id NOT IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(OVERVIEW_EXCLUDED_PREDICATES)]});
  }
  const relationWhere = joinFragments(relationParts);

  const availableNodeWhere = sourceFragment("n", spec.sources);
  const availableRelationWhere = sourceFragment("r", spec.sources);
  const focusSelectorWhere = focusNode
    ? joinFragments([...nodeParts, { sql: "n.id = ?", bindings: [focusNode.id] }])
    : null;
  const [availableNodes, availableRelations, matchedNodes, focusSelectorMatches, matchedRelations, baseRows, knowledgeTop] = await Promise.all([
    count(db, "knowledge_nodes n", availableNodeWhere),
    count(db, "knowledge_relations r", availableRelationWhere),
    spec.node_query.enabled ? count(db, "knowledge_nodes n", nodeWhere) : Promise.resolve(0),
    spec.node_query.enabled && focusSelectorWhere
      ? count(db, "knowledge_nodes n", focusSelectorWhere)
      : Promise.resolve(0),
    spec.relation_query.enabled
      ? count(db, "knowledge_relations r", relationWhere)
      : Promise.resolve(0),
    spec.node_query.enabled
      ? jsonRows(
        db,
        `SELECT n.json FROM knowledge_nodes n WHERE ${nodeWhere.sql} ORDER BY ${orderClause("n", spec.composition.sort_nodes, "node")} LIMIT ?`,
        ...nodeWhere.bindings,
        spec.limits.nodes,
      )
      : Promise.resolve([]),
    meta<Item>(db, "knowledge_top"),
  ]);

  const baseNodes = asNodes(baseRows);
  const selectedNodeIds = new Set<string>();
  const inclusion: Inclusion = { nodes: {}, relations: {}, authority: "query-execution-not-semantic-proof" };
  if (focusNode) { selectedNodeIds.add(focusNode.id); inclusion.nodes[focusNode.id] = { kind: "focus" }; }
  for (const item of baseNodes) {
    if (selectedNodeIds.size >= spec.limits.nodes) break;
    if (!selectedNodeIds.has(item.id)) {
      selectedNodeIds.add(item.id);
      inclusion.nodes[item.id] = { kind: "selector", path_witnesses: spec.explain ? await pathProofs(db, item.id, spec) : [] };
    }
  }
  let frontier = [...selectedNodeIds];
  const traversedRelationIds = new Set<string>();
  for (let depth = 0; depth < spec.traversal.depth; depth += 1) {
    const relationHeaders = spec.relation_query.enabled ? await localRelationHeaders(db, relationWhere,
      frontier, spec.traversal.direction, orderClause("r", spec.composition.sort_relations, "relation")) : [];
    const current = new Set(frontier);
    const next: string[] = [];
    for (const relation of relationHeaders) {
      let touched = false;
      for (const nodeId of current) {
        for (const neighbor of neighborIds(relation, nodeId, spec.traversal.direction)) {
          touched = true;
          if (!selectedNodeIds.has(neighbor) && selectedNodeIds.size < spec.limits.nodes
              && spec.sources.includes(neighbor === relation.from_id ? relation.from_source : relation.to_source)) {
            selectedNodeIds.add(neighbor);
            inclusion.nodes[neighbor] = { kind: "traversal", via_node_id: nodeId, via_relation_id: relation.id, depth: depth + 1 };
            next.push(neighbor);
          }
        }
      }
      if (touched && traversedRelationIds.size < spec.limits.relations) traversedRelationIds.add(relation.id);
    }
    frontier = [...new Set(next)];
    if (frontier.length === 0) break;
  }

  const selectionBasis = new Set(selectedNodeIds);
  const relationHeaders = !spec.relation_query.enabled ? [] : spec.composition.endpoint_policy === "independent"
    ? await allRelationHeaders(db, relationWhere, orderClause("r", spec.composition.sort_relations, "relation"))
    : await localRelationHeaders(db, relationWhere, selectionBasis, "either", orderClause("r", spec.composition.sort_relations, "relation"));
  const selectedRelationIds: string[] = [];
  let eligibleRelations = 0;
  for (const relation of relationHeaders) {
    const left = selectionBasis.has(relation.from_id);
    const right = selectionBasis.has(relation.to_id);
    const allowed =
      (spec.composition.endpoint_policy === "both" && left && right)
      || (spec.composition.endpoint_policy === "either" && (left || right))
      || spec.composition.endpoint_policy === "independent"
      || traversedRelationIds.has(relation.id);
    if (!allowed) continue;
    eligibleRelations += 1;
    if (selectedRelationIds.length >= spec.limits.relations) continue;
    const missing = [...new Set([relation.from_id, relation.to_id].filter((id) => !selectedNodeIds.has(id)))];
    if (missing.some((id) => !spec.sources.includes(id === relation.from_id ? relation.from_source : relation.to_source))) continue;
    if (selectedNodeIds.size + missing.length > spec.limits.nodes) continue;
    missing.forEach((id) => { selectedNodeIds.add(id); inclusion.nodes[id] = { kind: "endpoint", via_relation_id: relation.id }; });
    selectedRelationIds.push(relation.id);
    inclusion.relations[relation.id] = { kind: traversedRelationIds.has(relation.id) ? "traversal" : "endpoint-policy", endpoint_policy: spec.composition.endpoint_policy };
  }

  const [selectedNodes, selectedRelations] = await Promise.all([
    nodesByIds(db, selectedNodeIds),
    relationsByIds(db, selectedRelationIds),
  ]);
  const authority = knowledgeTop.authority_boundary && typeof knowledgeTop.authority_boundary === "object"
    ? knowledgeTop.authority_boundary as Item
    : {};
  return finalizeKnowledgeLens(authority, String(knowledgeTop.source_revision ?? ""), spec, selectedNodes, selectedRelations, {
    available_nodes: availableNodes,
    available_relations: availableRelations,
    matched_nodes: matchedNodes + (focusNode && focusSelectorMatches === 0 ? 1 : 0),
    matched_relations: matchedRelations,
    eligible_relations: eligibleRelations,
  }, focusNode?.id ?? null, inclusion);
}

export async function focusKnowledgeNodeD1(
  db: D1Database,
  nodeId: string,
  options: FocusKnowledgeOptions = {},
): Promise<Item> {
  return executeKnowledgeLensD1(db, focusLensSpec(nodeId, options));
}

function normalizedSources(values: string[] | null): string[] {
  const result = values?.length ? [...new Set(values)] : [...KNOWLEDGE_SOURCES];
  const unknown = result.filter((value) => !KNOWLEDGE_SOURCES.has(value));
  if (unknown.length > 0) throw new HttpError(400, `unsupported knowledge sources: ${unknown.sort().join(", ")}`);
  return result;
}

function bounded(value: number, name: string, minimum: number, maximum: number): number {
  if (!Number.isInteger(value) || value < minimum || value > maximum) throw new HttpError(400, `${name} must be between ${minimum} and ${maximum}`);
  return value;
}

async function knowledgeSearchD1Unchecked(
  db: D1Database,
  options: { query: string; sources: string[] | null; kindIds: string[]; predicateIds: string[]; offset: number; limit: number },
): Promise<Item> {
  const query = options.query.trim();
  if (query.length > 256) throw new HttpError(400, "knowledge search query exceeds 256 characters");
  const sources = normalizedSources(options.sources);
  const offset = bounded(options.offset, "offset", 0, 100_000);
  const limit = bounded(options.limit, "limit", 1, 100);
  if (options.kindIds.length > 100 || options.predicateIds.length > 100) {
    throw new HttpError(400, "knowledge search kind and predicate filters must contain at most 100 values");
  }
  const needle = query.toLocaleLowerCase();
  const nodeWhere = joinFragments([
    sourceFragment("n", sources),
    ...(options.kindIds.length ? [{ sql: "n.kind_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.kindIds)] }] : []),
    ...(needle ? [{ sql: "instr(n.search_text, ?) > 0", bindings: [needle] }] : []),
  ]);
  const relationWhere = joinFragments([
    sourceFragment("r", sources),
    ...(options.predicateIds.length ? [{ sql: "r.predicate_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.predicateIds)] }] : []),
    ...(needle ? [{ sql: "instr(r.search_text, ?) > 0", bindings: [needle] }] : []),
  ]);
  const nodeRank = needle
    ? "CASE WHEN lower(n.id) = ? OR lower(n.native_id) = ? OR n.title_text = ? THEN 0 WHEN instr(lower(n.id), ?) = 1 OR instr(lower(n.native_id), ?) = 1 OR instr(n.title_text, ?) = 1 THEN 1 ELSE 2 END, n.id"
    : "n.id";
  const relationRank = needle
    ? "CASE WHEN lower(r.id) = ? OR lower(r.native_id) = ? OR r.label_text = ? THEN 0 WHEN instr(lower(r.id), ?) = 1 OR instr(lower(r.native_id), ?) = 1 OR instr(r.label_text, ?) = 1 THEN 1 ELSE 2 END, r.id"
    : "r.id";
  const rankBindings = needle ? [needle, needle, needle, needle, needle, needle] : [];
  const [nodeCount, relationCount, nodeRows, relationRows, knowledgeTop] = await Promise.all([
    count(db, "knowledge_nodes n", nodeWhere),
    count(db, "knowledge_relations r", relationWhere),
    jsonRows(db, `SELECT n.json FROM knowledge_nodes n WHERE ${nodeWhere.sql} ORDER BY ${nodeRank} LIMIT ? OFFSET ?`, ...nodeWhere.bindings, ...rankBindings, limit, offset),
    jsonRows(db, `SELECT r.json FROM knowledge_relations r WHERE ${relationWhere.sql} ORDER BY ${relationRank} LIMIT ? OFFSET ?`, ...relationWhere.bindings, ...rankBindings, limit, offset),
    meta<Item>(db, "knowledge_top"),
  ]);
  return {
    schema: "tos_knowledge_search_v1",
    source_revision: knowledgeTop.source_revision,
    query,
    filters: { sources: [...sources].sort(), kind_ids: [...new Set(options.kindIds)].sort(), predicate_ids: [...new Set(options.predicateIds)].sort() },
    page: { offset, limit_per_kind: limit },
    counts: { matching_nodes: nodeCount, matching_relations: relationCount, returned_nodes: nodeRows.length, returned_relations: relationRows.length },
    nodes: nodeRows,
    relations: relationRows,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}

async function knowledgeNodeD1Unchecked(db: D1Database, id: string, relationLimit: number): Promise<Item> {
  const identifier = id.trim();
  if (!identifier) throw new HttpError(400, "knowledge node id is required");
  const exact = await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE id = ? ORDER BY id", identifier);
  const entity = exact.length ? [] : await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE entity_id = ? ORDER BY id", identifier);
  const matchedRows = exact.length
    ? exact
    : entity.length
    ? entity
    : await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE native_id = ? ORDER BY id", identifier);
  if (matchedRows.length === 0) throw new HttpError(404, `unknown ToS knowledge node: ${identifier}`);
  const matches = matchedRows.map((row) => parseItem(row.json));
  const ids = matches.map((item) => String(item.id));
  const relationWhere: SqlFragment = {
    sql: "from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?))",
    bindings: [JSON.stringify(ids), JSON.stringify(ids)],
  };
  const [relatedCount, selected, knowledgeTop] = await Promise.all([
    count(db, "knowledge_relations", relationWhere),
    jsonRows(
      db,
      `SELECT json FROM knowledge_relations WHERE ${relationWhere.sql} ORDER BY id LIMIT ?`,
      ...relationWhere.bindings,
      relationLimit,
    ),
    meta<Item>(db, "knowledge_top"),
  ]);
  const refs = [...new Set([...matches, ...selected].flatMap((item) => Array.isArray(item.source_refs) ? item.source_refs.map(String) : []))].sort();
  return {
    schema: "tos_knowledge_node_packet_v1",
    source_revision: knowledgeTop.source_revision,
    requested_id: identifier,
    ambiguous_native_id: exact.length === 0 && entity.length === 0 && matches.length > 1,
    shared_entity_id: entity.length > 1,
    matches,
    related_relations: selected,
    counts: { matches: matches.length, related_relations: relatedCount, returned_relations: selected.length },
    source_refs: refs,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}

async function knowledgeRelationD1Unchecked(db: D1Database, id: string): Promise<Item> {
  const identifier = id.trim();
  if (!identifier) throw new HttpError(400, "knowledge relation id is required");
  const exact = await rows<JsonRow>(db, "SELECT json FROM knowledge_relations WHERE id = ? ORDER BY id", identifier);
  const matchedRows = exact.length ? exact : await rows<JsonRow>(db, "SELECT json FROM knowledge_relations WHERE native_id = ? ORDER BY id", identifier);
  if (matchedRows.length === 0) throw new HttpError(404, `unknown ToS knowledge relation: ${identifier}`);
  const matches = matchedRows.map((row) => parseItem(row.json));
  const endpointIds = [...new Set(matches.flatMap((item) => [String(item.from_id), String(item.to_id)]))];
  const endpoints = await nodesByIds(db, endpointIds);
  const knowledgeTop = await meta<Item>(db, "knowledge_top");
  const refs = [...new Set([...matches, ...endpoints].flatMap((item) => Array.isArray(item.source_refs) ? item.source_refs.map(String) : []))].sort();
  return {
    schema: "tos_knowledge_relation_packet_v1",
    source_revision: knowledgeTop.source_revision,
    requested_id: identifier,
    ambiguous_native_id: exact.length === 0 && matches.length > 1,
    matches,
    endpoints,
    counts: { matches: matches.length, endpoints: endpoints.length },
    source_refs: refs,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}
