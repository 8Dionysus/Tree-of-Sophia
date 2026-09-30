import { HttpError, stringArray, stringValue, type Item } from "./common.ts";
import {NativeD1Read, nativeD1Limits, nativeSha256} from './native-d1-read.ts';
import {consistentRead} from './knowledge-store.ts';
import {sourceNavigationRules,observeRights,observeMemberships,type SourceRights} from "./source-navigation-rules.ts";
import {PhysicalKeys} from "./worker-classic.ts";

export class SourceNavigationError extends Error {
  readonly status:number;
  constructor(status:number,message:string){super(message);this.status=status;}
}

/*
 * Source navigation is stored as a row projection in D1.  The JSON payload
 * remains the owned navigation record; the scalar columns are only indexes
 * for bounded selection. Rust owns the maintained query policy; the host
 * retains hydration, checksums, current fencing and original row objects.
 */

type SourceJsonRow = { json: string };
type SourceNodeRow = SourceJsonRow & {
  node_id: string;
  node_kind: string;
  source_ref: string;
  label: string;
  identity_status: string;
  properties_json: string;
};
type SourceEdgeRow = SourceJsonRow & {
  edge_id: string;
  from_id: string;
  to_id: string;
  edge_kind: string;
  predicate_id: string;
  review_status: string;
  source_refs_json: string;
};
type SourceRightRow = SourceJsonRow & { rights_id: string; scope_refs_json: string };

const SOURCE_NODE_TABLE = "source_navigation_nodes";
const SOURCE_NODE_PAYLOAD_TABLE = "source_navigation_node_payload";
const SOURCE_EDGE_TABLE = "source_navigation_edges";
const SOURCE_EDGE_PAYLOAD_TABLE = "source_navigation_edge_payload";
const SOURCE_RIGHT_TABLE = "source_navigation_rights";
const SOURCE_RIGHT_PAYLOAD_TABLE = "source_navigation_rights_payload";

// One response is bounded to 300 source nodes.  A page is deliberately
// smaller than that bound so a high-degree node can never make one D1 `.all()`
// call unbounded.  We still walk every page: the pure contract returns every
// edge whose endpoints are admitted, and silently dropping a later page would
// make both the edge packet and dossier closure misleading.
export const SOURCE_NAVIGATION_PAGE_SIZE = 100;

function parseRow<T extends SourceJsonRow>(row: T, payload?: string): Item {
  const value: unknown = JSON.parse(payload ?? row.json);
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("source-navigation D1 row is not a JSON object");
  }
  return value as Item;
}

async function payloadFor(
  db: D1Database,
  table: string,
  id: string,
): Promise<string> {
  const rows = await db
    .prepare(`SELECT part, json_chunk FROM ${table} WHERE id = ? ORDER BY part`)
    .bind(id)
    .all<{ part: number; json_chunk: string }>();
  if (rows.results.length === 0) throw new Error(`source-navigation row ${id} has no payload`);
  for (const [index, row] of rows.results.entries()) {
    if (row.part !== index || typeof row.json_chunk !== "string") {
      throw new Error(`source-navigation row ${id} has incomplete payload chunks`);
    }
  }
  return rows.results.map((row) => row.json_chunk).join("");
}

async function parseHydrated<T extends SourceJsonRow>(db: D1Database, row: T, table: string, id: string): Promise<Item> {
  const raw = row.json === "" ? await payloadFor(db, table, id) : row.json;
  const kinds: Record<string,string> = {
    [SOURCE_NODE_PAYLOAD_TABLE]:'nodes', [SOURCE_EDGE_PAYLOAD_TABLE]:'edges', [SOURCE_RIGHT_PAYLOAD_TABLE]:'rights',
  };
  const kind = kinds[table];
  if (!kind || !id) throw new HttpError(503,'invalid source-navigation checksum identity');
  const key = `source_navigation_row_digest:${kind}:${await nativeSha256(id)}`;
  await requireSourceDigest(db,key,raw);
  return parseRow(row, raw);
}

async function requireSourceDigest(db:D1Database,key:string,raw:string):Promise<void> {
  const checksum = await db.prepare(`SELECT part,
    CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=128 THEN json_chunk ELSE NULL END AS json_chunk
    FROM edge_meta WHERE key=? ORDER BY part LIMIT 2`).bind(key).all<{part:number;json_chunk:string|null}>();
  const first=checksum.results[0];
  if (checksum.results.length!==1 || !first || first.part!==0 || first.json_chunk===null)
    throw new HttpError(503,'source-navigation checksum unavailable; explicit product migration required');
  let expected: unknown;
  try { expected=JSON.parse(first.json_chunk); }
  catch { throw new HttpError(503,'invalid source-navigation checksum'); }
  if (!expected || typeof expected!=='object' || Array.isArray(expected)
    || Object.keys(expected).join(',')!=='sha256' || (expected as {sha256:unknown}).sha256!==await nativeSha256(raw))
    throw new HttpError(503,'emitted source-navigation row checksum differs');
}

function sortedById(items: Item[], key: string): Item[] {
  return [...items].sort((left, right) => stringValue(left[key]).localeCompare(stringValue(right[key])));
}

function pageSize(limit: number): number {
  return Math.max(1, Math.min(SOURCE_NAVIGATION_PAGE_SIZE, limit));
}

async function sourceNavigationHeader(db: D1Database): Promise<Item> {
  const {raw} = await new NativeD1Read(db,nativeD1Limits).metadata('source_navigation_top');
  await requireSourceDigest(db,'source_navigation_header_digest',raw);
  const header = parseRow({json:raw});
  if (header.schema_version !== "tos_source_navigation_v1") {
    throw new Error("ToS source-navigation metadata has an unsupported schema_version");
  }
  return header;
}

async function sourceNode(db: D1Database, id: string): Promise<Item | null> {
  const row = await db
    .prepare(`SELECT node_id, node_kind, source_ref, label, identity_status, properties_json, json
                FROM ${SOURCE_NODE_TABLE} WHERE node_id = ?`)
    .bind(id)
    .first<SourceNodeRow>();
  if (!row) return null;
  const node = await parseHydrated(db, row, SOURCE_NODE_PAYLOAD_TABLE, row.node_id);
  for (const field of ["node_kind", "source_ref", "label", "identity_status"] as const) {
    if (stringValue(node[field]) !== row[field]) throw new Error(`source-navigation node selection mismatch: ${row.node_id}`);
  }
  // `properties_json` is an indexed selection hint and is intentionally empty
  // for rows whose complete JSON would breach the D1 row budget.  In that
  // case the hydrated record is the authority for packet filtering.
  let properties: Item;
  if (row.properties_json === "") {
    properties = node.properties && typeof node.properties === "object" && !Array.isArray(node.properties)
      ? node.properties as Item
      : {};
  } else {
    const parsed: unknown = JSON.parse(row.properties_json);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error(`source-navigation node selection is invalid: ${row.node_id}`);
    }
    const hint = parsedObject(parsed);
    properties = hint;
    const fullProperties = node.properties && typeof node.properties === "object" && !Array.isArray(node.properties)
      ? node.properties as Item
      : {};
    for (const key of ["packet_id", "access_status"] as const) {
      const hasHint = Object.prototype.hasOwnProperty.call(hint, key);
      const hasFull = Object.prototype.hasOwnProperty.call(fullProperties, key);
      if (hasHint !== hasFull || (hasHint && JSON.stringify(hint[key]) !== JSON.stringify(fullProperties[key]))) {
        throw new Error(`source-navigation node selection mismatch: ${row.node_id}`);
      }
    }
  }
  return sourceNavigationRules().WorkerSourceWalk.packet_filtered(stringValue(properties.packet_id)) ? null : node;
}

function parsedObject(value: unknown): Item {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Item : {};
}

type EdgeDirection = "incoming" | "outgoing";

function semanticClause(): { sql: string; values: string[] } {
  const predicates: string[] = JSON.parse(sourceNavigationRules().WorkerSourceWalk.semantic_predicates());
  return {
    sql: `(e.edge_kind = 'authored_item_manifest'
            OR (e.edge_kind = 'evidence_claim' AND e.predicate_id IN (${predicates.map(() => "?").join(", ")})))`,
    values: predicates,
  };
}

type EdgePageRow = { id: string; item: Item };

async function sourceEdgePage(
  db: D1Database,
  direction: EdgeDirection,
  nodeId: string,
  semantic: boolean,
  cursor: string | null,
  limit: number,
): Promise<EdgePageRow[]> {
  const endpoint = direction === "outgoing" ? "e.from_id" : "e.to_id";
  const other = direction === "outgoing" ? "e.to_id" : "e.from_id";
  const keyset = cursor === null ? "" : " AND e.edge_id > ?";
  const semanticFilter = semantic ? ` AND ${semanticClause().sql}` : "";
  const values: unknown[] = [nodeId];
  if (cursor !== null) values.push(cursor);
  if (semantic) values.push(...semanticClause().values);
  values.push(limit);
  const rows = await db
    .prepare(`SELECT e.edge_id, e.from_id, e.to_id, e.edge_kind, e.predicate_id,
                     e.review_status, e.source_refs_json, e.json
                FROM ${SOURCE_EDGE_TABLE} e
                JOIN ${SOURCE_NODE_TABLE} other_node
                  ON other_node.node_id = ${other}
               WHERE ${endpoint} = ?${keyset}${semanticFilter}
               ORDER BY e.edge_id
               LIMIT ?`)
    .bind(...values)
    .all<SourceEdgeRow>();
  const result: EdgePageRow[] = [];
  for (const row of rows.results) {
    const item = await parseHydrated(db, row, SOURCE_EDGE_PAYLOAD_TABLE, row.edge_id);
    for (const field of ["edge_id", "from_id", "to_id", "edge_kind", "predicate_id", "review_status"] as const) {
      if (stringValue(item[field]) !== row[field]) throw new Error(`source-navigation edge selection mismatch: ${row.edge_id}`);
    }
    if (row.source_refs_json !== "" && JSON.stringify(item.source_refs ?? []) !== row.source_refs_json) {
      throw new Error(`source-navigation edge selection mismatch: ${row.edge_id}`);
    }
    result.push({ id: row.edge_id, item });
  }
  return result;
}

async function sourceEdges(
  db: D1Database,
  direction: EdgeDirection,
  nodeId: string,
  semantic = false,
  limit = SOURCE_NAVIGATION_PAGE_SIZE,
): Promise<Item[]> {
  const result: Item[] = [];
  const size = pageSize(limit);
  let cursor: string | null = null;
  while (true) {
    const page = await sourceEdgePage(db, direction, nodeId, semantic, cursor, size);
    result.push(...page.map((row) => row.item));
    if (page.length < size) break;
    const next = page[page.length - 1]?.id ?? "";
    // The authored projection requires a stable nonempty edge_id.  Failing
    // closed here prevents an invalid row from causing an endless keyset loop
    // while still keeping every valid page bounded.
    if (!next || next === cursor) throw new Error("source-navigation edge has no stable edge_id");
    cursor = next;
  }
  // D1's binary text collation and JS localeCompare are not interchangeable
  // for arbitrary Unicode IDs.  Pages are bounded, while this final ordering
  // preserves the source-navigation engine's established deterministic order.
  return sortedById(result, "edge_id");
}

async function sourceRights(db: D1Database, ids: Iterable<string>, limit: number): Promise<Item[]> {
  const values = [...new Set(ids)].filter(Boolean);
  if (values.length === 0) return [];
  const size = pageSize(limit);
  const result: Item[] = [];
  let cursor: string | null = null;
  const encodedIds = JSON.stringify(values);
  while (true) {
    const keyset: string = cursor === null ? "" : " AND r.rights_id > ?";
    const bindings: unknown[] = [encodedIds];
    if (cursor !== null) bindings.push(cursor);
    bindings.push(size);
    const rows: D1Result<SourceRightRow> = await db
      .prepare(`SELECT r.rights_id, r.scope_refs_json, r.json
                  FROM ${SOURCE_RIGHT_TABLE} r
                 WHERE (r.scope_refs_json = '' OR EXISTS (
                   SELECT 1 FROM json_each(r.scope_refs_json) scope
                    WHERE scope.value IN (SELECT value FROM json_each(?))
                 ))${keyset}
                 ORDER BY r.rights_id
                 LIMIT ?`)
      .bind(...bindings)
      .all<SourceRightRow>();
    for (const row of rows.results) {
      const item = await parseHydrated(db, row, SOURCE_RIGHT_PAYLOAD_TABLE, row.rights_id);
      if (stringValue(item.rights_id) !== row.rights_id) throw new Error(`source-navigation rights selection mismatch: ${row.rights_id}`);
      if (row.scope_refs_json !== "" && JSON.stringify(item.scope_refs ?? []) !== row.scope_refs_json) {
        throw new Error(`source-navigation rights selection mismatch: ${row.rights_id}`);
      }
      result.push(item);
    }
    if (rows.results.length < size) break;
    const next: string = rows.results[rows.results.length - 1]?.rights_id ?? "";
    if (!next || next === cursor) throw new Error("source-navigation rights row has no stable rights_id");
    cursor = next;
  }
  return sortedById(result, "rights_id");
}

/** Execute the source descent route against indexed D1 rows. */
export async function sourceDescendD1(
  db: D1Database,
  nodeId: string,
  maxDepth: number,
  limit: number,
): Promise<Item> {
  return consistentRead(db, () => readSourceDescendD1(db, nodeId, maxDepth, limit));
}

async function readSourceDescendD1(db: D1Database, nodeId: string, maxDepth: number, limit: number): Promise<Item> {
  const navigation=await sourceNavigationHeader(db),root=await sourceNode(db,nodeId);
  if(!root)throw new SourceNavigationError(404,`unknown ToS source-navigation node: ${nodeId}`);
  const walked=await walkSource(db,nodeId,root,false,maxDepth,limit);
  try{
    const nodes=walked.keys.strings(walked.rules.node_ids()).sort((left,right)=>
      walked.rules.node_depth(walked.keys.key(left))-walked.rules.node_depth(walked.keys.key(right))||left.localeCompare(right))
      .map(id=>({...walked.nodes.get(id),depth:walked.rules.node_depth(walked.keys.key(id))}));
    const edges=Array.from(walked.rules.edge_rows(),row=>walked.edges[row]!);
    return {schema:'tos_source_descent_v1',root_id:nodeId,max_depth:maxDepth,limit,truncated:walked.rules.truncated(),
      counts:{nodes:nodes.length,edges:edges.length},nodes,edges,authority_note:navigation.authority_boundary};
  }finally{walked.rules.free();}
}

async function walkSource(db:D1Database,id:string,root:Item,dossier:boolean,depth:number,limit:number){
  const keys=new PhysicalKeys(),rules=new (sourceNavigationRules().WorkerSourceWalk)(keys.key(id),keys.key(''),stringValue(root.node_kind),dossier,depth,limit);
  try{
  const nodes=new Map<string,Item>([[id,root]]),cache=new Map<string,Item|null>([[id,root]]),edges:Item[]=[];
  const incomingCache=new Map<string,Item[]>(),outgoingCache=new Map<string,Item[]>(),handles=new Map<Item,number>();
  const incoming=async(id:string)=>{const prior=incomingCache.get(id);if(prior)return prior;
    const rows=await sourceEdges(db,'incoming',id,false,limit);incomingCache.set(id,rows);return rows;};
  const outgoing=async(id:string,semantic:boolean)=>{if(!dossier)return sourceEdges(db,'outgoing',id,semantic,limit);const prior=outgoingCache.get(id);if(prior)return prior;
    const rows=await sourceEdges(db,'outgoing',id,semantic,limit);outgoingCache.set(id,rows);return rows;};
  while(rules.need()!=='done'){
    if(rules.need()==='sort'){rules.sorted(keys.observe(keys.strings(rules.sorting_ids()).sort()));continue;}
    const current=keys.strings(Uint32Array.of(rules.current()))[0]!;
    for(const edge of await (rules.incoming()?incoming(current):outgoing(current,rules.semantic()))){
      const known=handles.get(edge),row=known??edges.length,edgeId=keys.key(stringValue(edge.edge_id));
      const target=stringValue(edge.to_id);
      if(rules.edge(edgeId,row,keys.key(stringValue(edge.from_id)),keys.key(target),stringValue(edge.edge_kind),stringValue(edge.predicate_id))){
        const wanted=keys.strings(Uint32Array.of(rules.target()))[0]!;
        if(!cache.has(wanted))cache.set(wanted,await sourceNode(db,wanted));
        const node=cache.get(wanted);rules.loaded(Boolean(node),stringValue(node?.node_kind));
        if(node&&rules.contains(keys.key(wanted)))nodes.set(wanted,node);
      }
      if(known===undefined&&rules.retained(edgeId,row)){handles.set(edge,row);edges.push(edge);}
    }
    rules.finish_edges();
  }
  return {keys,rules,nodes,edges,incoming};
  }catch(error){rules.free();throw error;}
}

/** Execute the Work/Link dossier route against indexed D1 rows. */
export async function sourceDossierD1(db: D1Database, objectId: string, limit: number): Promise<Item> {
  return consistentRead(db, () => readSourceDossierD1(db, objectId, limit));
}

async function readSourceDossierD1(db: D1Database, objectId: string, limit: number): Promise<Item> {
  const navigation=await sourceNavigationHeader(db),selected=await sourceNode(db,objectId);
  if(!selected)throw new SourceNavigationError(404,`unknown ToS dossier object: ${objectId}`);
  const selectedKind=stringValue(selected.node_kind),runtime=sourceNavigationRules();
  if(!runtime.WorkerSourceWalk.supported(selectedKind))throw new SourceNavigationError(400,'dossiers are currently available for Work and Link objects');
  const walked=await walkSource(db,objectId,selected,true,0,limit),{keys,rules,nodes}=walked;
  let rightsRules:SourceRights|undefined;
  try{
    rightsRules=new runtime.WorkerSourceRights();
    const componentIds=keys.strings(rules.node_ids()).sort(),componentNodes=componentIds.map(id=>nodes.get(id)!);
    const chain:Record<string,Item[]>={};for(const kind of JSON.parse(runtime.WorkerSourceWalk.chain_kinds()) as string[])
      chain[kind]=componentIds.filter(id=>rules.kind_matches(keys.key(id),kind)).map(id=>nodes.get(id)!);
    const edgeRows=Array.from(rules.edge_rows()),componentEdges=edgeRows.map(row=>walked.edges[row]!);
    const orderedRows=Uint32Array.from(edgeRows.sort((left,right)=>stringValue(walked.edges[left]!.edge_id).localeCompare(stringValue(walked.edges[right]!.edge_id))));
    const treePaths:Item[]=[];
    rules.prepare_paths(keys.observe((chain.era??[]).map(era=>stringValue(era.node_id))),orderedRows);
    for(let index=0;index<rules.path_count();index++)treePaths.push({node_ids:keys.strings(rules.path_nodes(index)),
      edge_ids:Array.from(rules.path_edges(index),row=>stringValue(walked.edges[row]!.edge_id))});
    for(const id of componentIds)rightsRules.component(keys.key(id),stringValue(nodes.get(id)?.node_kind));
    const rights=(await sourceRights(db,keys.strings(rules.node_ids()),limit)).filter(record=>rightsRules!.intersects_component(keys.observe(stringArray(record.scope_refs))));
    observeRights(rightsRules,keys,rights);observeMemberships(rightsRules,keys,componentEdges);
    for(const edge of componentEdges){const properties=parsedObject(edge.properties),contexts=properties.item_file_contexts;
      let valid=0;if(Array.isArray(contexts))for(const value of contexts){const record=Boolean(value)&&typeof value==='object'&&!Array.isArray(value);
        if(runtime.WorkerSourceRights.context_ref(record,Boolean(stringValue(record?(value as Item).rights_ref:undefined))))valid++;}
      rightsRules.observe_legacy_file(keys.key(stringValue(edge.to_id)),stringValue(edge.edge_kind),stringValue(edge.predicate_id),Array.isArray(contexts),Array.isArray(contexts)?contexts.length:0,valid);
    }
    for(const file of keys.strings(rightsRules.legacy_file_ids()).filter(Boolean).sort())for(const edge of await walked.incoming(file))
      rightsRules.incoming_owner(keys.key(file),keys.key(stringValue(edge.from_id)),stringValue(edge.edge_kind),stringValue(edge.predicate_id));
    const filtered=Array.from(rightsRules.filtered_rows(),row=>rights[row]!);
    for(const link of selectedKind==='work'?(chain.link??[]):[selected])rightsRules.status(stringValue((link.properties as Item|undefined)?.access_status)||'unknown');
    const decision=rules.decision_ids(),summary=JSON.parse(rightsRules.summary(decision,(chain.link??[]).length)) as Item;
    summary.rights_scope_refs=keys.strings(decision).sort();
    for(const node of componentNodes){const ref=stringValue(node.source_ref);rules.reference(keys.key(ref),Boolean(ref));}
    for(const edge of componentEdges)for(const ref of stringArray(edge.source_refs))rules.reference(keys.key(ref),true);
    for(const record of filtered){const ref=stringValue(record.source_ref);rules.reference(keys.key(ref),Boolean(ref));}
    return {schema:'tos_source_dossier_v1',object_id:objectId,object:selected,agent_summary:summary,chain,tree_paths:treePaths,
      relations:Array.from(orderedRows,row=>walked.edges[row]!),rights:sortedById(filtered,'rights_id'),source_refs:keys.strings(rules.references()).sort(),
      truncated:rules.truncated(),authority_note:navigation.authority_boundary};
  }finally{rightsRules?.free();rules.free();}
}
