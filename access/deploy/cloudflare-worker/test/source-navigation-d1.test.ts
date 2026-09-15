import assert from "node:assert/strict";
import test from "node:test";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { sourceDossier, sourceDescend } from "../src/source-navigation.ts";
import { sourceDossierD1, sourceDescendD1, SOURCE_NAVIGATION_PAGE_SIZE } from "../src/source-navigation-store.ts";
import type { Item } from "../src/common.ts";
import {createHash} from 'node:crypto';

type Navigation = {
  schema_version: string;
  authority_boundary: string;
  counts: { nodes: number; edges: number; rights: number };
  nodes: Item[];
  edges: Item[];
  rights: Item[];
};

const SCHEMA = `
CREATE TABLE edge_meta(key TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY(key, part));
CREATE TABLE knowledge_exploration_clock(singleton INTEGER PRIMARY KEY, epoch INTEGER NOT NULL);
INSERT INTO knowledge_exploration_clock VALUES(1,0);
CREATE TABLE source_navigation_nodes(node_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, node_kind TEXT NOT NULL, source_ref TEXT NOT NULL, label TEXT NOT NULL, identity_status TEXT NOT NULL, properties_json TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE source_navigation_node_payload(id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY(id, part));
CREATE TABLE source_navigation_edges(edge_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, from_id TEXT NOT NULL, to_id TEXT NOT NULL, edge_kind TEXT NOT NULL, predicate_id TEXT NOT NULL, review_status TEXT NOT NULL, source_refs_json TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE source_navigation_edge_payload(id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY(id, part));
CREATE TABLE source_navigation_rights(rights_id TEXT PRIMARY KEY, ord INTEGER NOT NULL, scope_refs_json TEXT NOT NULL, json TEXT NOT NULL);
CREATE TABLE source_navigation_rights_payload(id TEXT NOT NULL, part INTEGER NOT NULL, json_chunk TEXT NOT NULL, PRIMARY KEY(id, part));
CREATE INDEX source_navigation_edges_from_idx ON source_navigation_edges(from_id, edge_id);
CREATE INDEX source_navigation_edges_to_idx ON source_navigation_edges(to_id, edge_id);
CREATE INDEX source_navigation_rights_scope_idx ON source_navigation_rights(scope_refs_json);
`;

function baseNavigation(): Navigation {
  return {
    schema_version: "tos_source_navigation_v1",
    authority_boundary: "source-owned navigation only",
    counts: { nodes: 5, edges: 4, rights: 1 },
    nodes: [
      { node_id: "era", node_kind: "era", label: "Fixture era", source_ref: "era.json", identity_status: "not_applicable", properties: {} },
      { node_id: "planting", node_kind: "source_planting", label: "Fixture planting", source_ref: "planting.json", identity_status: "not_applicable", properties: {} },
      { node_id: "work", node_kind: "work", label: "Fixture work", source_ref: "work.json", identity_status: "verified", properties: {} },
      { node_id: "expression", node_kind: "expression", label: "Fixture expression", source_ref: "expression.json", identity_status: "verified", properties: {} },
      { node_id: "link", node_kind: "link", label: "Fixture link", source_ref: "link.json", identity_status: "verified", properties: { uri: "https://example.test/work", access_status: "open_download" } },
    ],
    edges: [
      { edge_id: "e1", from_id: "era", to_id: "planting", predicate_id: "contains", edge_kind: "authored_branch_hierarchy", review_status: "not_applicable", source_refs: ["era.json"] },
      { edge_id: "e2", from_id: "planting", to_id: "work", predicate_id: "references_source_witness", edge_kind: "authored_source_planting", review_status: "not_applicable", source_refs: ["planting.json"] },
      { edge_id: "e3", from_id: "work", to_id: "expression", predicate_id: "has_expression", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim.jsonl"] },
      { edge_id: "e4", from_id: "work", to_id: "link", predicate_id: "downloadable_at", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["link.jsonl"] },
    ],
    rights: [
      { rights_id: "r1", scope_refs: ["work"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", source_ref: "rights.json" },
    ],
  };
}

async function populate(db: D1Database, navigation: Navigation, payloadIds: Set<string> = new Set()): Promise<void> {
  await db.exec(SCHEMA);
  await db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind('data_revision',JSON.stringify({sha256:'a'.repeat(64)})).run();
  await db.prepare("INSERT INTO edge_meta VALUES (?, 0, ?)").bind("source_navigation_top", JSON.stringify({
    schema_version: navigation.schema_version,
    authority_boundary: navigation.authority_boundary,
    counts: navigation.counts,
  })).run();
  async function checksum(kind:string,id:string,raw:string) {
    const hash=(text:string)=>createHash('sha256').update(text,'utf8').digest('hex');
    await db.prepare('INSERT INTO edge_meta VALUES (?,0,?)')
      .bind(`source_navigation_row_digest:${kind}:${hash(id)}`,JSON.stringify({sha256:hash(raw)})).run();
  }
  for (const [ord, node] of navigation.nodes.entries()) {
    const id = String(node.node_id);
    const payload = JSON.stringify(node);
    await checksum('nodes',id,payload);
    await db.prepare("INSERT INTO source_navigation_nodes VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
      .bind(id, ord, node.node_kind, node.source_ref, node.label, node.identity_status,
        JSON.stringify(node.properties ?? {}), payloadIds.has(id) ? "" : payload).run();
    if (payloadIds.has(id)) await db.prepare("INSERT INTO source_navigation_node_payload VALUES (?, 0, ?)").bind(id, payload).run();
  }
  for (const [ord, edge] of navigation.edges.entries()) {
    const id = String(edge.edge_id);
    const payload = JSON.stringify(edge);
    await checksum('edges',id,payload);
    await db.prepare("INSERT INTO source_navigation_edges VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
      .bind(id, ord, edge.from_id, edge.to_id, edge.edge_kind, edge.predicate_id, edge.review_status,
        JSON.stringify(edge.source_refs ?? []), payloadIds.has(id) ? "" : payload).run();
    if (payloadIds.has(id)) await db.prepare("INSERT INTO source_navigation_edge_payload VALUES (?, 0, ?)").bind(id, payload).run();
  }
  for (const [ord, right] of navigation.rights.entries()) {
    const id = String(right.rights_id);
    const payload = JSON.stringify(right);
    await checksum('rights',id,payload);
    await db.prepare("INSERT INTO source_navigation_rights VALUES (?, ?, ?, ?)")
      .bind(id, ord, JSON.stringify(right.scope_refs ?? []), payloadIds.has(id) ? "" : payload).run();
    if (payloadIds.has(id)) await db.prepare("INSERT INTO source_navigation_rights_payload VALUES (?, 0, ?)").bind(id, payload).run();
  }
}

async function database(): Promise<{ mf: Miniflare; db: D1Database }> {
  const mf = new Miniflare(convertV4MiniflareOptions({
    modules: true,
    script: "export default {fetch(){return new Response()}}",
    d1Databases: ["DB"],
  }));
  return { mf, db: await mf.getD1Database("DB") };
}

test('native source routes refuse an ABA publication during an otherwise valid row read', async () => {
  const {mf,db}=await database();
  try {
    await populate(db,baseNavigation());
    for (const route of ['descent','dossier']) {
      let changed=false;
      const interleaved=new Proxy(db, {get(target,property) {
        if (property!=='prepare') return Reflect.get(target,property);
        return (sql:string) => {
          const wrap=(statement:D1PreparedStatement):D1PreparedStatement => new Proxy(statement,{get(inner,key) {
            if (key==='bind') return (...values:unknown[])=>wrap(inner.bind(...values));
            if (key==='all') return async () => {
              const result=await inner.all();
              if (!changed && sql.includes('source_navigation_nodes')) {
                changed=true;
                // A -> B -> A keeps revision bytes but invalidates the epoch.
                await db.batch([
                  db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'b'.repeat(64)})),
                  db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'a'.repeat(64)})),
                  db.prepare('UPDATE knowledge_exploration_clock SET epoch=epoch+2 WHERE singleton=1'),
                ]);
              }
              return result;
            };
            const value=Reflect.get(inner,key);
            return typeof value==='function' ? value.bind(inner) : value;
          }});
          return wrap(target.prepare(sql));
        };
      }});
      await assert.rejects(route==='descent' ? sourceDescendD1(interleaved,'era',8,300)
        : sourceDossierD1(interleaved,'work',300), /snapshot changed during query/);
      assert.equal(changed,true);
    }
  } finally { await mf.dispose(); }
});

function tracedDatabase(db: D1Database, limits: number[]): D1Database {
  return {
    prepare(sql: string): D1PreparedStatement {
      const statement = db.prepare(sql);
      return {
        bind(...values: unknown[]): D1PreparedStatement {
          const bound = statement.bind(...values);
          return {
            all<T = Record<string, unknown>>(...args: []): Promise<D1Result<T>> {
              if (sql.includes("source_navigation_edges") || sql.includes("source_navigation_rights")) {
                const candidate = values.at(-1);
                if (typeof candidate === "number") limits.push(candidate);
              }
              return bound.all<T>(...args);
            },
            first<T = Record<string, unknown>>(...args: []): Promise<T | null> { return bound.first<T>(...args); },
            run<T = Record<string, unknown>>(...args: []): Promise<D1Result<T>> { return bound.run<T>(...args); },
            raw<T = unknown[]>(...args: []): Promise<T[]> { return bound.raw<T>(...args); },
            bind: statement.bind.bind(statement),
          } as D1PreparedStatement;
        },
        all: statement.all.bind(statement),
        first: statement.first.bind(statement),
        run: statement.run.bind(statement),
        raw: statement.raw.bind(statement),
      } as D1PreparedStatement;
    },
    batch: db.batch.bind(db),
    exec: db.exec.bind(db),
    dump: db.dump.bind(db),
    withSession: db.withSession.bind(db),
  } as D1Database;
}

test("D1 source descent matches the pure packet and pages high-degree adjacency", async () => {
  const { mf, db } = await database();
  try {
    const navigation = baseNavigation();
    for (let i = 0; i < SOURCE_NAVIGATION_PAGE_SIZE * 2 + 7; i += 1) {
      const id = `target-${String(i).padStart(3, "0")}`;
      navigation.nodes.push({ node_id: id, node_kind: "item", label: id, source_ref: `${id}.json`, identity_status: "verified", properties: {} });
      navigation.edges.push({ edge_id: `high-${String(i).padStart(3, "0")}`, from_id: "work", to_id: id, predicate_id: "exemplified_by", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: [`${id}.jsonl`] });
    }
    navigation.counts = { nodes: navigation.nodes.length, edges: navigation.edges.length, rights: 1 };
    await populate(db, navigation, new Set(["target-007"]));
    const limits: number[] = [];
    const traced = tracedDatabase(db, limits);
    const expected = sourceDescend(navigation, "work", 1, 300);
    const actual = await sourceDescendD1(traced, "work", 1, 300);
    assert.deepEqual(actual, expected);
    assert.ok(limits.length >= 3, "the high-degree adjacency should require several pages");
    assert.ok(limits.every((value) => value <= SOURCE_NAVIGATION_PAGE_SIZE));
    assert.equal((actual.nodes as Item[]).length, 210);
  } finally {
    await mf.dispose();
  }
});

test("D1 Work and Link dossiers preserve closure, rights, truncation, and chunk hydration", async () => {
  const { mf, db } = await database();
  try {
    const navigation = baseNavigation();
    for (let i = 0; i < SOURCE_NAVIGATION_PAGE_SIZE + 3; i += 1) {
      navigation.rights.push({ rights_id: `r-${String(i).padStart(3, "0")}`, scope_refs: ["work"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", source_ref: `rights-${i}.json` });
    }
    navigation.counts.rights = navigation.rights.length;
    await populate(db, navigation, new Set(["link", "r-101"]));

    const expectedWork = sourceDossier(navigation, "work", 300);
    const actualWork = await sourceDossierD1(db, "work", 300);
    assert.deepEqual(actualWork, expectedWork);
    assert.equal((actualWork.rights as Item[]).length, navigation.rights.length);

    const expectedLink = sourceDossier(navigation, "link", 3);
    const actualLink = await sourceDossierD1(db, "link", 3);
    assert.deepEqual(actualLink, expectedLink);
    assert.equal(actualLink.truncated, true);
    assert.equal((actualLink.object as Item).properties && ((actualLink.object as Item).properties as Item).access_status, "open_download");
  } finally {
    await mf.dispose();
  }
});

test("D1 source routes fail closed for packet members and unknown dossier kinds", async () => {
  const { mf, db } = await database();
  try {
    const navigation = baseNavigation();
    navigation.nodes.push({ node_id: "packet-member", node_kind: "item", label: "packet", source_ref: "packet.json", identity_status: "verified", properties: { packet_id: "dense" } });
    navigation.counts.nodes += 1;
    await populate(db, navigation);
    await assert.rejects(sourceDescendD1(db, "packet-member", 1, 3), /unknown ToS source-navigation node/);
    await assert.rejects(sourceDossierD1(db, "era", 3), /dossiers are currently available/);
  } finally {
    await mf.dispose();
  }
});

test("D1 hydrates empty selection sentinels for nodes, edges and rights", async () => {
  const { mf, db } = await database();
  try {
    const navigation = baseNavigation();
    navigation.nodes.push({ node_id: "packet-member", node_kind: "item", label: "packet", source_ref: "packet.json", identity_status: "provisional", properties: { packet_id: "dense" } });
    navigation.counts.nodes += 1;
    const payloads = new Set([
      ...navigation.nodes.map(row => String(row.node_id)),
      ...navigation.edges.map(row => String(row.edge_id)),
      ...navigation.rights.map(row => String(row.rights_id)),
    ]);
    await populate(db, navigation, payloads);
    // Exact compact-row framing emitted when a selection hint also overflows.
    await db.prepare("UPDATE source_navigation_nodes SET properties_json = ''").run();
    await db.prepare("UPDATE source_navigation_edges SET source_refs_json = ''").run();
    await db.prepare("UPDATE source_navigation_rights SET scope_refs_json = ''").run();
    assert.deepEqual(await sourceDossierD1(db, "work", 30), sourceDossier(navigation, "work", 30));
    assert.deepEqual(await sourceDossierD1(db, "link", 30), sourceDossier(navigation, "link", 30));
    await assert.rejects(sourceDescendD1(db, "packet-member", 1, 3), /unknown ToS source-navigation node/);
  } finally {
    await mf.dispose();
  }
});

test('D1 rejects full-row drift and missing checksums before deriving rights posture', async () => {
  for (const chunked of [false,true]) {
    const {mf,db}=await database();
    try {
      const navigation=baseNavigation();
      navigation.rights[0].review_status='unreviewed';
      await populate(db,navigation,chunked?new Set(['work','e3','r1']):new Set());
      const expected=sourceDossier(navigation,'work',300);
      assert.deepEqual(await sourceDossierD1(db,'work',300),expected);
      for (const [kind,id,delta] of [
        ['nodes','work',{research_note:'altered'}],
        ['edges','e3',{research_note:'altered'}],
        ['rights','r1',{review_status:'accepted'}],
      ] as const) {
        const spec={nodes:['node_id','node'],edges:['edge_id','edge'],rights:['rights_id','rights']}[kind];
        const table=chunked?`source_navigation_${spec[1]}_payload`:`source_navigation_${kind}`;
        const column=chunked?'json_chunk':'json', key=chunked?'id':spec[0];
        const original=await db.prepare(`SELECT ${column} AS raw FROM ${table} WHERE ${key}=?`).bind(id).first<{raw:string}>();
        assert.ok(original);
        await db.prepare(`UPDATE ${table} SET ${column}=? WHERE ${key}=?`)
          .bind(JSON.stringify({...JSON.parse(original.raw),...delta}),id).run();
        await assert.rejects(sourceDossierD1(db,'work',300),/checksum differs/);
        await db.prepare(`UPDATE ${table} SET ${column}=? WHERE ${key}=?`).bind(original.raw,id).run();
      }
      const key=`source_navigation_row_digest:rights:${createHash('sha256').update('r1').digest('hex')}`;
      await db.prepare('DELETE FROM edge_meta WHERE key=?').bind(key).run();
      await assert.rejects(sourceDossierD1(db,'work',300),/explicit product migration required/);
    } finally { await mf.dispose(); }
  }
});
