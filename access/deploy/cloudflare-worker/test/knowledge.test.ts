import assert from "node:assert/strict";
import test from "node:test";
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { executeKnowledgeLensD1, knowledgeSearchD1, knowledgeNodeD1, knowledgeRelationD1 } from "../src/knowledge-store.ts";

import { executeKnowledgeLens, focusKnowledgeNode, normalizeLensSpec, type KnowledgeGraph } from "../src/knowledge.ts";

const graph: KnowledgeGraph = {
  schema: "tos_knowledge_graph_v1",
  source_revision: "a".repeat(64),
  nodes: [
    {
      id: "philosophy:a", entity_id: "tos.concept.a", native_id: "a", source_graph: "philosophy", kind_id: "concept", type_id: "tos.entity.concept", type_mapping: { status: "mapped", source_kind_id: "concept" }, semantics: {},
      display: { title: { default: "Alpha", ru: "Альфа", en: "Alpha" }, kind_label: { default: "concept", ru: null, en: "concept" }, summary: { default: "Concept Alpha", ru: null, en: "Concept Alpha" }, summary_state: "metadata-synthesis", provenance: {} },
      epistemic: { authority_layer: "candidate", canon_status: "pre-canon", review_posture: null, confidence: null },
      graph_layers: ["conceptual-relation"], view_ids: [], source_refs: ["ToS/a"], attributes: {}, content_revision: "b".repeat(64),
    },
    {
      id: "philosophy:b", entity_id: "tos.work.b", native_id: "b", source_graph: "philosophy", kind_id: "work", type_id: "tos.entity.work", type_mapping: { status: "mapped", source_kind_id: "work" }, semantics: {},
      display: { title: { default: "Beta", ru: null, en: "Beta" }, kind_label: { default: "work", ru: null, en: "work" }, summary: { default: "Work Beta", ru: null, en: "Work Beta" }, summary_state: "metadata-synthesis", provenance: {} },
      epistemic: { authority_layer: null, canon_status: null, review_posture: null, confidence: null },
      graph_layers: ["source-relation"], view_ids: [], source_refs: ["ToS/b"], attributes: {}, content_revision: "c".repeat(64),
    },
    {
      id: "philosophy:c", entity_id: "tos.agent.c", native_id: "c", source_graph: "philosophy", kind_id: "person", type_id: "tos.entity.agent", type_mapping: { status: "mapped", source_kind_id: "person" }, semantics: {},
      display: { title: { default: "Gamma", ru: "Гамма", en: "Gamma" }, kind_label: { default: "person", ru: null, en: "person" }, summary: { default: "Person Gamma", ru: null, en: "Person Gamma" }, summary_state: "metadata-synthesis", provenance: {} },
      epistemic: { authority_layer: null, canon_status: null, review_posture: null, confidence: null },
      graph_layers: ["source-relation"], view_ids: [], source_refs: ["ToS/c"], attributes: {}, content_revision: "e".repeat(64),
    },
  ],
  relations: [
    {
      id: "philosophy:e", native_id: "e", source_graph: "philosophy", from_id: "philosophy:a", to_id: "philosophy:b", predicate_id: "relates", relation_type_id: "tos.relation.related", predicate_mapping: { status: "mapped", source_predicate_id: "relates" }, semantics: {},
      display: { label: { default: "relates", ru: null, en: "relates" }, inverse_label: null, statement: { default: "Alpha relates Beta.", ru: null, en: "Alpha relates Beta." }, explanation: { default: "No explanation supplied.", ru: null, en: "No explanation supplied." }, explanation_state: "missing", provenance: {} },
      epistemic: { authority_layer: null, canon_status: null, review_posture: null, confidence: null },
      graph_layers: ["conceptual-relation"], view_ids: [], source_refs: ["ToS/e"], attributes: {}, content_revision: "d".repeat(64),
    },
    {
      id: "philosophy:f", native_id: "f", source_graph: "philosophy", from_id: "philosophy:b", to_id: "philosophy:c", predicate_id: "extends", relation_type_id: "tos.relation.related", predicate_mapping: { status: "mapped", source_predicate_id: "extends" }, semantics: {},
      display: { label: { default: "extends", ru: null, en: "extends" }, inverse_label: null, statement: { default: "Beta extends Gamma.", ru: null, en: "Beta extends Gamma." }, explanation: { default: "No explanation supplied.", ru: null, en: "No explanation supplied." }, explanation_state: "missing", provenance: {} },
      epistemic: { authority_layer: null, canon_status: null, review_posture: null, confidence: null },
      graph_layers: ["conceptual-relation"], view_ids: [], source_refs: ["ToS/f"], attributes: {}, content_revision: "f".repeat(64),
    },
  ],
  counts: { nodes: 3, relations: 2 },
  authority_boundary: { is_source: false, is_canon: false },
};

test("indexed D1 path conditions and inclusion agree with the pure engine", async () => {
  const bundle = await build({ entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
    bundle: true, write: false, format: 'esm', platform: 'browser', target: 'es2022' });
  const mf = new Miniflare(convertV4MiniflareOptions({ modules: true, script: bundle.outputFiles[0]!.text, d1Databases: ["DB"] }));
  try {
    const db = await mf.getD1Database("DB");
    await db.batch([
      db.prepare("CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)"),
      db.prepare("CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)"),
      db.prepare("CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)"),
      db.prepare("CREATE INDEX kn_from ON knowledge_relations(from_id)"),
      db.prepare("CREATE INDEX kn_to ON knowledge_relations(to_id)"),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision', 0, ?) ").bind(JSON.stringify({sha256: graph.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top', 0, ?) ").bind(JSON.stringify({source_revision: graph.source_revision, authority_boundary: graph.authority_boundary})),
      ...graph.nodes.map(n => db.prepare("INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)").bind(n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,n.display.title.default.toLowerCase(),JSON.stringify(n).toLowerCase(),JSON.stringify(n))),
      ...graph.relations.map(r => db.prepare("INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)").bind(r.id,r.native_id,r.source_graph,r.from_id,r.to_id,r.predicate_id,r.relation_type_id,r.display.label.default.toLowerCase(),JSON.stringify(r).toLowerCase(),JSON.stringify(r))),
    ]);
    const base = { schema_version: 'tos_lens_spec_v1', lens_id: 'path-parity', sources: ['philosophy'], explain: true };
    for (const direction of ['outgoing', 'incoming', 'either']) {
      for (const quantifier of ['exists', 'not_exists']) {
        for (const length of [1, 2, 3, 4]) {
          const spec = {...base, path_query: [{path_id: 'p', quantifier, steps: Array.from({length}, () => ({direction}))}]};
          assert.deepEqual(await executeKnowledgeLensD1(db, spec), await executeKnowledgeLens(graph, spec));
        }
      }
    }
    const joined = {...base, path_query: [
      {path_id: 'agent', steps: [{}, {node_query: {filters: [{field: 'type_id', op: 'eq', value: 'tos.entity.agent'}]}}]},
      {path_id: 'work', steps: [{node_query: {filters: [{field: 'type_id', op: 'eq', value: 'tos.entity.work'}]}}]}
    ]};
    const joinedResult = await executeKnowledgeLensD1(db, joined);
    assert.deepEqual(joinedResult, await executeKnowledgeLens(graph, joined));
    assert.deepEqual((joinedResult.nodes as {id:string}[]).map(n=>n.id), ['philosophy:a']);
    const focused = {...base, seed: {focus_node_id: 'philosophy:a'}, node_query: {enabled: false}, traversal: {depth: 2}};
    assert.deepEqual(await executeKnowledgeLensD1(db, focused), await executeKnowledgeLens(graph, focused));
    const python = (spec: unknown) => JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph,spec}), encoding:'utf8'}));
    const whole = await executeKnowledgeLens(graph, focused);
    const nodeIds: string[] = [], relationIds: string[] = [];
    let cursor: string | null = null;
    for (let iteration = 0; iteration < 5; iteration++) {
      const spec = {...focused, pagination: {nodes: 1, relations: 1, cursor}};
      const page = await executeKnowledgeLensD1(db, spec);
      assert.deepEqual(page, await executeKnowledgeLens(graph, spec));
      assert.deepEqual(page, python(spec));
      assert.equal(page.fingerprint, whole.fingerprint);
      const info = page.page as {primary_node_ids:string[]; next_cursor:string|null};
      nodeIds.push(...info.primary_node_ids);
      relationIds.push(...(page.relations as {id:string}[]).map(r=>r.id));
      cursor = info.next_cursor;
      if (!cursor) break;
      await assert.rejects(executeKnowledgeLensD1(db,{...spec,lens_id:'different',pagination:{...spec.pagination,cursor}}), /query or snapshot changed/);
    }
    assert.deepEqual(nodeIds, whole.nodes.map(n=>n.id));
    assert.deepEqual(relationIds, whole.relations.map(r=>r.id));
    assert.deepEqual(await executeKnowledgeLensD1(db, joined), python(joined));
    const httpSpec = {...focused, pagination: {nodes: 1, relations: 1, cursor: null as string|null}};
    const requestPage = (spec: unknown) => mf.dispatchFetch('http://tos.test/api/knowledge/lenses/compile',
      {method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify(spec)});
    const firstResponse = await requestPage(httpSpec);
    assert.equal(firstResponse.status, 200);
    const firstPage = await firstResponse.json() as {page:{next_cursor:string}};
    httpSpec.pagination.cursor = firstPage.page.next_cursor;
    assert.ok(httpSpec.pagination.cursor);
    assert.equal((await requestPage({...httpSpec,lens_id:'another-query'})).status, 409);
    assert.equal((await requestPage({...httpSpec,pagination:{...httpSpec.pagination,cursor:'malformed'}})).status, 400);
    assert.equal((await requestPage(httpSpec)).status, 200);
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'")
      .bind(JSON.stringify({source_revision:'f'.repeat(64),authority_boundary:graph.authority_boundary})).run();
    assert.equal((await requestPage(httpSpec)).status, 409);
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'")
      .bind(JSON.stringify({source_revision:graph.source_revision,authority_boundary:graph.authority_boundary})).run();
    for (const packet of [await knowledgeNodeD1(db,'philosophy:a',0), await knowledgeRelationD1(db,'philosophy:e'),
      await knowledgeSearchD1(db,{query:'',sources:null,kindIds:[],predicateIds:[],offset:0,limit:2})]) {
      assert.equal(packet.source_revision, graph.source_revision);
    }
    const scoped = structuredClone(graph);
    scoped.nodes[1]!.source_graph = 'repository';
    await db.prepare("UPDATE knowledge_nodes SET source_graph='repository', json=? WHERE id=?").bind(JSON.stringify(scoped.nodes[1]), 'philosophy:b').run();
    assert.deepEqual(await executeKnowledgeLensD1(db,joined), await executeKnowledgeLens(scoped,joined));
  } finally { await mf.dispose(); }
});

test("edge lens engine composes an unknown declarative lens", async () => {
  const spec = normalizeLensSpec({
    schema_version: "tos_lens_spec_v1",
    lens_id: "edge.concepts",
    sources: ["philosophy"],
    node_query: { match: "all", filters: [{ field: "kind_id", op: "eq", value: "concept" }] },
    relation_query: { match: "all", filters: [] },
    traversal: { depth: 1, direction: "either", predicate_ids: [] },
    composition: { endpoint_policy: "both", group_by: ["kind_id"], sort_nodes: [], sort_relations: [] },
    presentation: { layout: "semantic", color_by: "kind_id", lane_by: null, size_by: null, inspector_fields: ["display.summary"] },
    limits: { nodes: 20, relations: 20, groups: 20 },
  });
  const result = await executeKnowledgeLens(graph, spec);

  assert.deepEqual(result.nodes.map((node) => node.id), ["philosophy:a", "philosophy:b"]);
  assert.deepEqual(result.relations.map((relation) => relation.id), ["philosophy:e"]);
  assert.equal(result.presentation.layout, "semantic");
  assert.equal(result.fingerprint.length, 64);
  assert.equal(result.authority_boundary.is_source, false);
});

test("edge lens validator rejects unsafe paths and unbounded requests", () => {
  assert.equal(
    normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "depth-five", traversal: { depth: 5 } }).traversal.depth,
    5,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "too-deep", traversal: { depth: 6 } }),
    /traversal.depth must be between 0 and 5/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "unsafe", node_query: { match: "all", filters: [{ field: "__proto__.x", op: "eq", value: "x" }] } }),
    /unsupported node filter field/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "too-large", limits: { nodes: 1001, relations: 20, groups: 20 } }),
    /nodes must be between 1 and 1000/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "unsafe-nested", node_query: { filters: [{ field: "attributes.safe.constructor.name", op: "eq", value: "x" }] } }),
    /unsupported node filter field/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "unknown", executable_code: "no" }),
    /unknown lens spec fields/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "bad-title", title: 42 }),
    /title must be a non-empty string or localized object/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "bad-title-field", title: { default: "valid", html: "no" } }),
    /unknown title fields/,
  );
  assert.throws(
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "bad-focus", seed: { focus_node_id: "" } }),
    /seed.focus_node_id must be a non-empty string or null/,
  );
});

test("edge lens engine can start from matching relations", async () => {
  const result = await executeKnowledgeLens(graph, {
    schema_version: "tos_lens_spec_v1",
    lens_id: "relations-first",
    sources: ["philosophy"],
    node_query: { enabled: false },
    relation_query: { filters: [{ field: "predicate_id", op: "eq", value: "relates" }] },
    composition: { endpoint_policy: "independent" },
    limits: { nodes: 10, relations: 10, groups: 10 },
  });
  assert.deepEqual(result.nodes.map((item) => item.id), ["philosophy:a", "philosophy:b"]);
  assert.deepEqual(result.relations.map((item) => item.id), ["philosophy:e"]);
});

test("focus is explicit and closure cannot escape the requested neighborhood", async () => {
  const focused = await focusKnowledgeNode(graph, "a", {
    sources: ["philosophy"], depth: 1, direction: "either", nodeLimit: 20, relationLimit: 20,
  });
  assert.equal(focused.lens.seed.focus_node_id, "a");
  assert.equal((focused.focus as { node_id: string }).node_id, "philosophy:a");
  assert.equal((focused.focus as { entity_id: string }).entity_id, "tos.concept.a");
  assert.equal((focused.agent_summary as { focus_node_id: string }).focus_node_id, "philosophy:a");
  assert.deepEqual(focused.nodes.map((item) => item.id), ["philosophy:a", "philosophy:b"]);
  assert.deepEqual(focused.relations.map((item) => item.id), ["philosophy:e"]);
  assert.equal((focused.counts as { eligible_relations: number }).eligible_relations, 1);
  assert.equal((focused.counts as { truncated_relations: number }).truncated_relations, 0);

  const closure = await executeKnowledgeLens(graph, {
    schema_version: "tos_lens_spec_v1",
    lens_id: "bounded-either-closure",
    sources: ["philosophy"],
    seed: { node_ids: ["a"] },
    traversal: { depth: 0 },
    composition: { endpoint_policy: "either" },
    limits: { nodes: 20, relations: 20, groups: 20 },
  });
  assert.deepEqual(closure.nodes.map((item) => item.id), ["philosophy:a", "philosophy:b"]);
  assert.deepEqual(closure.relations.map((item) => item.id), ["philosophy:e"]);

  await assert.rejects(
    () => focusKnowledgeNode(graph, "missing", { sources: ["philosophy"] }),
    /unknown ToS knowledge focus/,
  );
  const ambiguous: KnowledgeGraph = {
    ...graph,
    nodes: [...graph.nodes, { ...graph.nodes[0]!, id: "canon:duplicate-a", entity_id: "tos.concept.other-a", source_graph: "canon" }],
  };
  await assert.rejects(
    () => focusKnowledgeNode(ambiguous, "a", { sources: ["philosophy", "canon"] }),
    /ambiguous ToS knowledge focus/,
  );

  const sharedIdentity: KnowledgeGraph = {
    ...graph,
    nodes: [...graph.nodes, { ...graph.nodes[0]!, id: "source-navigation:tos.concept.a", native_id: "tos.concept.a", source_graph: "source-navigation" }],
  };
  const byEntity = await focusKnowledgeNode(sharedIdentity, "tos.concept.a", {
    sources: ["philosophy", "source-navigation"], depth: 0,
  });
  assert.equal((byEntity.focus as { node_id: string }).node_id, "source-navigation:tos.concept.a");
  assert.equal((byEntity.focus as { resolved_by: string }).resolved_by, "entity_id");

  const sortedByEntity = await executeKnowledgeLens(sharedIdentity, {
    schema_version: "tos_lens_spec_v1",
    lens_id: "stable-focus-through-finalization",
    sources: ["philosophy", "source-navigation"],
    seed: { focus_node_id: "tos.concept.a" },
    traversal: { depth: 0 },
    composition: { sort_nodes: [{ field: "id", direction: "asc" }] },
  });
  assert.equal((sortedByEntity.focus as { node_id: string }).node_id, "source-navigation:tos.concept.a");
});
