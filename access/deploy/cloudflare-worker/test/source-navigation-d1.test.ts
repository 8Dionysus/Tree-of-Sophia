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
      { rights_id: "r1", assessment_kind: "aggregate", scope_refs: ["work"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted", source_ref: "rights.json" },
    ],
  };
}

function sharedWorkRightsNavigation(legacy = false): Navigation {
  const fileId = "tos.file.sha256.shared";
  const itemA = "tos.item.copy.a";
  const itemB = "tos.item.copy.b";
  const manifestA = "ToS/source-witnesses/fixture/copy-a/item.manifest.json";
  const manifestB = "ToS/source-witnesses/fixture/copy-b/item.manifest.json";
  const rightsA = "ToS/source-witnesses/fixture/copy-a/rights.json";
  const rightsB = "ToS/source-witnesses/fixture/copy-b/rights.json";
  const membership = (itemId: string, manifestRef: string, rightsRef: string, basename: string): Item => ({
    edge_id: `${itemId}:${fileId}`,
    from_id: itemId,
    to_id: fileId,
    predicate_id: "has_file",
    edge_kind: "authored_item_manifest",
    review_status: "not_applicable",
    source_refs: [manifestRef],
    properties: { item_file_contexts: [{
      manifest_ref: manifestRef,
      acquisition_event_ref: `tos.event.acquisition.${itemId}`,
      ...(legacy ? {} : { rights_ref: rightsRef }),
      payload_entries: [{ relative_path: `payload/${basename}`, original_basename: basename }],
    }] },
  });
  const nodes: Item[] = [
    { node_id: "work-a", node_kind: "work", label: "A", source_ref: "work-a.json", identity_status: "verified", properties: {} },
    { node_id: "expression-a", node_kind: "expression", label: "A expression", source_ref: "expression-a.json", identity_status: "verified", properties: {} },
    { node_id: "edition-a", node_kind: "edition", label: "A edition", source_ref: "edition-a.json", identity_status: "verified", properties: {} },
    { node_id: "work-b", node_kind: "work", label: "B", source_ref: "work-b.json", identity_status: "verified", properties: {} },
    { node_id: "expression-b", node_kind: "expression", label: "B expression", source_ref: "expression-b.json", identity_status: "verified", properties: {} },
    { node_id: "edition-b", node_kind: "edition", label: "B edition", source_ref: "edition-b.json", identity_status: "verified", properties: {} },
    { node_id: itemA, node_kind: "item", label: "A copy", source_ref: manifestA, identity_status: "verified", properties: {} },
    { node_id: itemB, node_kind: "item", label: "B copy", source_ref: manifestB, identity_status: "verified", properties: {} },
    { node_id: fileId, node_kind: "file", label: "shared", source_ref: manifestA, identity_status: "verified", properties: {} },
  ];
  const edges: Item[] = [
    { edge_id: "a-work-expression", from_id: "work-a", to_id: "expression-a", predicate_id: "has_expression", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-a.jsonl"] },
    { edge_id: "a-expression-edition", from_id: "expression-a", to_id: "edition-a", predicate_id: "embodied_by", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-a.jsonl"] },
    { edge_id: "a-edition-item", from_id: "edition-a", to_id: itemA, predicate_id: "exemplified_by", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-a.jsonl"] },
    membership(itemA, manifestA, rightsA, "a.bin"),
    { edge_id: "b-work-expression", from_id: "work-b", to_id: "expression-b", predicate_id: "has_expression", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-b.jsonl"] },
    { edge_id: "b-expression-edition", from_id: "expression-b", to_id: "edition-b", predicate_id: "embodied_by", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-b.jsonl"] },
    { edge_id: "b-edition-item", from_id: "edition-b", to_id: itemB, predicate_id: "exemplified_by", edge_kind: "evidence_claim", review_status: "unreviewed", source_refs: ["claim-b.jsonl"] },
    membership(itemB, manifestB, rightsB, "b.bin"),
  ];
  const rights: Item[] = [
    { rights_id: legacy ? "tos.rights.fixture.work-a" : "rights-work-a", ...(legacy ? {} : { assessment_kind: "aggregate" }), source_ref: "ToS/source-witnesses/fixture/work-a/rights.json", scope_refs: ["work-a"], assessment_status: "licensed", redistribution_posture: "authorized", review_status: "accepted" },
    { rights_id: legacy ? "tos.rights.fixture.copy-a" : "rights-a", ...(legacy ? {} : { assessment_kind: "aggregate" }), source_ref: rightsA, scope_refs: [itemA, fileId], assessment_status: "public_domain_reviewed", redistribution_posture: "authorized", review_status: "accepted" },
    { rights_id: legacy ? "tos.rights.fixture.copy-b" : "rights-b", ...(legacy ? {} : { assessment_kind: "aggregate" }), source_ref: rightsB, scope_refs: [itemB, fileId], assessment_status: "copyright_undetermined", redistribution_posture: "not_authorized", review_status: "not_reviewed" },
  ];
  return {
    schema_version: "tos_source_navigation_v1",
    authority_boundary: "source-owned fixture navigation",
    counts: { nodes: nodes.length, edges: edges.length, rights: rights.length },
    nodes,
    edges,
    rights,
  };
}

async function populate(db: D1Database, navigation: Navigation, payloadIds: Set<string> = new Set()): Promise<void> {
  await db.exec(SCHEMA);
  await db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind('data_revision',JSON.stringify({sha256:'a'.repeat(64)})).run();
  const header=JSON.stringify({
    schema_version: navigation.schema_version,
    authority_boundary: navigation.authority_boundary,
    counts: navigation.counts,
  });
  await db.prepare("INSERT INTO edge_meta VALUES (?,0,?)").bind('source_navigation_top',header).run();
  await db.prepare("INSERT INTO edge_meta VALUES (?,0,?)").bind('source_navigation_header_digest',
    JSON.stringify({sha256:createHash('sha256').update(header,'utf8').digest('hex')})).run();
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

test('native source routes distinguish concurrent row/checksum replacement from stable corruption', async () => {
  for (const route of ['descent','dossier']) {
    const {mf,db}=await database();
    try {
      await populate(db,baseNavigation());
      let changed=false;
      const interleaved=new Proxy(db,{get(target,property) {
        if (property!=='prepare') return Reflect.get(target,property);
        return (sql:string) => {
          const wrap=(statement:D1PreparedStatement):D1PreparedStatement => new Proxy(statement,{get(inner,key) {
            if (key==='bind') return (...values:unknown[])=>wrap(inner.bind(...values));
            if (key==='first') return async () => {
              const result=await inner.first< {json:string;node_id:string} >();
              if (!changed && sql.includes('source_navigation_nodes') && result) {
                changed=true;
                const raw=JSON.stringify({...JSON.parse(result.json),research_note:'new publication'});
                const hash=(value:string)=>createHash('sha256').update(value,'utf8').digest('hex');
                await db.batch([
                  db.prepare('UPDATE source_navigation_nodes SET json=? WHERE node_id=?').bind(raw,result.node_id),
                  db.prepare('UPDATE edge_meta SET json_chunk=? WHERE key=?')
                    .bind(JSON.stringify({sha256:hash(raw)}),`source_navigation_row_digest:nodes:${hash(result.node_id)}`),
                  db.prepare('UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1'),
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
        : sourceDossierD1(interleaved,'work',300),/snapshot changed during query/);
      assert.equal(changed,true);
    } finally { await mf.dispose(); }
  }
});

test('native source header authority drift and missing header digest refuse', async () => {
  const {mf,db}=await database();
  try {
    await populate(db,baseNavigation());
    const original=await db.prepare("SELECT json_chunk FROM edge_meta WHERE key='source_navigation_top'").first<{json_chunk:string}>();
    assert.ok(original);
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='source_navigation_top'")
      .bind(JSON.stringify({...JSON.parse(original.json_chunk),authority_boundary:'forged canon authority'})).run();
    await assert.rejects(sourceDossierD1(db,'work',300),/checksum differs/);
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='source_navigation_top'").bind(original.json_chunk).run();
    await db.prepare("DELETE FROM edge_meta WHERE key='source_navigation_header_digest'").run();
    await assert.rejects(sourceDescendD1(db,'era',8,300),/explicit product migration/);
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

test("D1 openness uses aggregate assessments while retaining positive layer evidence", async () => {
  const { mf, db } = await database();
  try {
    const navigation = baseNavigation();
    navigation.rights = [{
      rights_id: "tos.rights.fixture.work",
      assessment_kind: "aggregate",
      source_ref: "rights.json",
      scope_refs: ["work"],
      assessment_status: "copyright_undetermined",
      redistribution_posture: "not_authorized",
      review_status: "accepted",
    }, {
      rights_id: "tos.rights.fixture.work.layer.ocr",
      assessment_kind: "layer",
      source_ref: "rights.json",
      scope_refs: ["work"],
      assessment_status: "licensed",
      redistribution_posture: "authorized",
      review_status: "accepted",
    }];
    navigation.counts.rights = navigation.rights.length;
    await populate(db, navigation);

    const result = await sourceDossierD1(db, "work", 30);
    const summary = result.agent_summary as Item;
    assert.equal(summary.can_conclude_legal_openness, false);
    assert.equal(summary.rights_posture, "not_cleared");
    assert.deepEqual((result.rights as Item[]).map((record) => record.rights_id), [
      "tos.rights.fixture.work",
      "tos.rights.fixture.work.layer.ocr",
    ]);
  } finally {
    await mf.dispose();
  }
});

test("D1 Work dossiers bind File-scoped rights to reachable Item membership contexts", async () => {
  const exact = sharedWorkRightsNavigation();
  const legacySingle = sharedWorkRightsNavigation(true);
  legacySingle.edges = legacySingle.edges.filter((edge) => edge.edge_id !== "tos.item.copy.b:tos.file.sha256.shared");
  legacySingle.rights = legacySingle.rights.slice(0, 2);
  legacySingle.counts = { nodes: legacySingle.nodes.length, edges: legacySingle.edges.length, rights: legacySingle.rights.length };
  const legacyAmbiguous: Navigation = {
    ...legacySingle,
    rights: [
      ...legacySingle.rights,
      {
        ...(legacySingle.rights[1] as Item),
        rights_id: "tos.rights.fixture.copy-a.conflict",
        source_ref: "ToS/source-witnesses/fixture/copy-a/alternate-rights.json",
        redistribution_posture: "not_authorized",
      },
    ],
  };
  legacyAmbiguous.counts = { ...legacyAmbiguous.counts, rights: legacyAmbiguous.rights.length };

  for (const navigation of [exact, legacySingle, legacyAmbiguous, sharedWorkRightsNavigation(true)]) {
    const { mf, db } = await database();
    try {
      await populate(db, navigation);
      const expected = sourceDossier(navigation, "work-a", 20);
      const actual = await sourceDossierD1(db, "work-a", 20);
      assert.deepEqual(actual, expected);
      const rights = actual.rights as Item[];
      assert.equal(rights.some((record) => record.source_ref === "ToS/source-witnesses/fixture/copy-b/rights.json"), false);
      if (navigation === exact || navigation === legacySingle) {
        assert.deepEqual(rights.map((record) => record.rights_id), navigation === exact
          ? ["rights-a", "rights-work-a"]
          : ["tos.rights.fixture.copy-a", "tos.rights.fixture.work-a"]);
      } else {
        assert.deepEqual(rights.map((record) => record.rights_id), ["tos.rights.fixture.work-a"]);
      }
    } finally {
      await mf.dispose();
    }
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
