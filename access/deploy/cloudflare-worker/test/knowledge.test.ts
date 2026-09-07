import assert from "node:assert/strict";
import test from "node:test";
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { executeKnowledgeLensD1, knowledgeSearchD1, knowledgeNodeD1, knowledgeRelationD1 } from "../src/knowledge-store.ts";

import { executeKnowledgeLens, focusKnowledgeNode, normalizeLensSpec, selectDisplayForm, type KnowledgeGraph } from "../src/knowledge.ts";
import { selectHumanForms, formDeliveryCost, HUMAN_FORM_SELECTION_BUDGET } from '../src/human-forms.ts';

function realFormNode(): KnowledgeGraph['nodes'][number] {
  return JSON.parse(execFileSync('python3', ['-c',
    "import sys,json,pathlib;sys.path.insert(0,'access/src');from tos_access.knowledge import _normalize_node;g=json.loads(pathlib.Path('ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json').read_text());n=next(n for n in g['nodes'] if n['properties'].get('identity_ref')=='tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese');print(json.dumps(_normalize_node(n,'source-claims')))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding:'utf8'}));
}

function claimFormNode(): KnowledgeGraph['nodes'][number] {
  // A disposable source-copy form of the real, unreviewed letter Claim;
  // not a new historical assertion or an assessed source form.
  return JSON.parse(execFileSync('python3', ['-c', [
    "import sys,json,pathlib;sys.path[:0]=['access/src','mechanics/growth-cycle/parts/branch-growth-cycle/scripts']",
    'from source_commands import prepare_claim_change, materialize_claim_forms',
    'from tos_access.knowledge import _normalize_node',
    "g=json.loads(pathlib.Path('ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json').read_text())",
    "n=next(n for n in g['nodes'] if n['properties'].get('claim_ref')=='tos.claim.nietzsche-letter-705.sender')",
    "c=n['properties']['source_claim']",
    "f=prepare_claim_change(c,None,'software:test-only','tos.form.test.claim','claim.statement')['form']",
    "s={'schema_version':'tos_human_form_set_v1','subject':f['subject'],'forms':[f],'prior_forms':[]}",
    "n['properties'].update(human_forms=materialize_claim_forms(c,s,access_allowed=True),human_forms_source_ref='test-only:source-copy')",
    "print(json.dumps(_normalize_node(n,'source-claims')))"
  ].join(';')], {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding:'utf8'}));
}

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
    // A shared record maker/provenance event is not semantic proximity.
    // Exact inspection and the full technical profile still expose the edge.
    for (const relationType of ['tos.relation.made-by', 'tos.relation.generated-by']) {
      const technical = structuredClone(graph);
      technical.relations[0]!.relation_type_id = relationType;
      await db.prepare('UPDATE knowledge_relations SET relation_type_id=?, json=? WHERE id=?')
        .bind(relationType, JSON.stringify(technical.relations[0]), technical.relations[0]!.id).run();
      for (const profile of ['overview', 'all']) {
        const spec = {...focused, traversal: {depth: 2, profile}};
        const pure = await executeKnowledgeLens(technical, spec);
        assert.deepEqual(pure.nodes.map(n => n.id), profile === 'overview' ? ['philosophy:a'] : graph.nodes.map(n => n.id));
        assert.deepEqual(await executeKnowledgeLensD1(db, spec), pure);
      }
    }
    await db.prepare('UPDATE knowledge_relations SET relation_type_id=?, json=? WHERE id=?')
      .bind(graph.relations[0]!.relation_type_id, JSON.stringify(graph.relations[0]), graph.relations[0]!.id).run();
    const python = (spec: unknown, source = graph) => JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph:source,spec}), encoding:'utf8'}));
    const propertyGraph = structuredClone(graph);
    propertyGraph.query_properties = [{property_id: 'tos.property.fixture-score', field: 'attributes.score',
      value_type: 'number', applies_to: ['tos.entity.concept'], inherited: true, operators: ['eq', 'neq', 'gt', 'exists']}];
    propertyGraph.nodes[0]!.attributes.score = 3;
    propertyGraph.nodes[1]!.semantics.type_ancestors = ['tos.entity.concept'];
    // The third node has the same physical field but is outside the property's declared type.
    propertyGraph.nodes[2]!.attributes.score = 9;
    propertyGraph.query_properties.push(
      {property_id: 'tos.property.fixture-word', field: 'attributes.word', value_type: 'string',
        applies_to: ['tos.entity.concept'], inherited: false, operators: ['eq', 'contains', 'prefix']},
      {property_id: 'tos.property.fixture-flag', field: 'attributes.flag', value_type: 'boolean',
        applies_to: ['tos.entity.concept'], inherited: false, operators: ['eq', 'neq']},
      {property_id: 'tos.property.fixture-tags', field: 'attributes.tags', value_type: 'string-array',
        applies_to: ['tos.entity.concept'], inherited: false, operators: ['eq', 'in', 'contains']});
    Object.assign(propertyGraph.nodes[0]!.attributes, {word: 'Свобода Ω 🦉\u0000fin', flag: false, tags: ['Мысль', 'Freiheit']});
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'").bind(JSON.stringify({
      source_revision: graph.source_revision, authority_boundary: graph.authority_boundary,
      query_properties: propertyGraph.query_properties})).run();
    for (const n of propertyGraph.nodes) await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(n), n.id).run();
    for (const [op, value] of [['eq', 3], ['neq', 9], ['gt', 2], ['exists', false]] as const) {
      const node_query = {filters: [{property_id: 'tos.property.fixture-score', op, value}]};
      for (const path of [false, true]) for (const detail of ['compact', 'full']) {
        const spec = {...base, detail, ...(path ? {path_query: [{path_id: 'property-target', steps: [{node_query}]}]} : {node_query})};
        const pure = await executeKnowledgeLens(propertyGraph, spec);
        assert.deepEqual(pure, python(spec, propertyGraph));
        assert.deepEqual(await executeKnowledgeLensD1(db, spec), pure);
        assert.equal(JSON.stringify(pure.lens).includes('_property_binding'), false);
      }
    }
    for (const change of [{property_id: 'tos.property.unknown'}, {op: 'contains'}, {value: true},
                         {value: '3'}, {field: 'attributes.score'}, {property_id: 'tos.property.bad\r'},
                         {property_id: 'tos.property.bad\n'}, {property_id: null}]) {
      const spec = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-score', op: 'eq', value: 3, ...change}]}};
      await assert.rejects(executeKnowledgeLens(propertyGraph, spec));
      await assert.rejects(executeKnowledgeLensD1(db, spec));
    }
    for (const [name, op, value] of [['word', 'eq', 'Свобода Ω 🦉\u0000fin'], ['word', 'contains', 'Ω 🦉'],
        ['word', 'prefix', 'Свобода Ω 🦉\u0000fi'], ['word', 'prefix', ''],
        ['word', 'prefix', 'Свобода'], ['word', 'prefix', 'свобода'], ['word', 'contains', ['Свобода']],
        ['flag', 'eq', false], ['flag', 'neq', true], ['tags', 'eq', 'Мысль'],
        ['tags', 'in', ['Freiheit']], ['tags', 'contains', ['Мысль', 'Freiheit']]] as const) {
      const spec = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-' + name, op, value}]}};
      const pure = await executeKnowledgeLens(propertyGraph, spec);
      assert.deepEqual(pure, python(spec, propertyGraph));
      assert.deepEqual(await executeKnowledgeLensD1(db, spec), pure);
    }
    // Returning to the old snapshot removes the binding as well as the synthetic values.
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'").bind(JSON.stringify({
      source_revision: graph.source_revision, authority_boundary: graph.authority_boundary})).run();
    for (const n of graph.nodes) await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(n), n.id).run();
    const absentProperty = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-score', op: 'eq', value: 3}]}};
    await assert.rejects(executeKnowledgeLens(graph, absentProperty));
    await assert.rejects(executeKnowledgeLensD1(db, absentProperty));
    const carriers = structuredClone(graph);
    carriers.nodes[1]!.entity_id = carriers.nodes[0]!.entity_id;
    await db.prepare('UPDATE knowledge_nodes SET entity_id=?, json=? WHERE id=?')
      .bind(carriers.nodes[1]!.entity_id, JSON.stringify(carriers.nodes[1]), carriers.nodes[1]!.id).run();
    for (const paging of [null, {nodes: 1, relations: 1}]) {
      const spec = {...focused, pagination: paging};
      const pure = await executeKnowledgeLens(carriers, spec);
      assert.deepEqual(await executeKnowledgeLensD1(db, spec), pure);
      assert.deepEqual(pure, python(spec, carriers));
      const scene = pure.scene as {vertices: {node_ids: string[]}[]};
      assert.equal(scene.vertices.filter(v => v.node_ids.includes('philosophy:a'))[0]!.node_ids.length, 2);
    }
    for (const profile of ['overview', 'all']) for (const size of [1, 3]) {
      const spec = {...focused, traversal: {depth: 1, profile}, limits: {nodes: size}};
      const pure = await executeKnowledgeLens(carriers, spec);
      assert.deepEqual(await executeKnowledgeLensD1(db, spec), pure);
      assert.deepEqual(pure, python(spec, carriers));
      assert.equal(pure.nodes.some(n => n.id === 'philosophy:c'), profile === 'overview' && size > 1);
      assert.equal((pure.counts as {identity_expansion_limited:boolean}).identity_expansion_limited, profile === 'overview' && size === 1);
    }
    await db.prepare('UPDATE knowledge_nodes SET entity_id=?, json=? WHERE id=?')
      .bind(graph.nodes[1]!.entity_id, JSON.stringify(graph.nodes[1]), graph.nodes[1]!.id).run();
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
    // New language/script keys pass through all three execution backends.
    scoped.nodes[0]!.display.title['grc-Grek'] = 'λόγος';
    scoped.relations[0]!.display.statement.fr = 'Attribution non établie.';
    scoped.relations[0]!.semantics.assertion_contexts = [{schema_version: 'tos_assertion_context_v1',
      binding_role: 'carrier', source_record_digest: '1'.repeat(64), source_refs: ['ToS/e'],
      fields: {polarity: {value: 'negative', source_pointer: '/polarity'},
        qualifiers: {value: {'x-unknown': false}, source_pointer: '/qualifiers'}},
      conflicts: [], interpretation: 'source-declared-not-semantic-assessment'}];
    await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(scoped.nodes[0]), 'philosophy:a').run();
    await db.prepare('UPDATE knowledge_relations SET json=? WHERE id=?').bind(JSON.stringify(scoped.relations[0]), 'philosophy:e').run();
    const languageSpec = {...base, language: 'fr-CA', title: {'fr-CA': 'Lecture'}, detail: 'compact',
      node_query: {filters: [{field: 'display.title.grc-Grek', op: 'eq', value: 'λόγος'}]},
      relation_query: {filters: [{field: 'display.statement.fr', op: 'contains', value: 'non'}]},
      composition: {endpoint_policy: 'either'}};
    const languageResult = await executeKnowledgeLensD1(db, languageSpec);
    assert.deepEqual(languageResult, await executeKnowledgeLens(scoped, languageSpec));
    const pythonLanguage = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: languageSpec}), encoding:'utf8'}));
    assert.deepEqual(languageResult, pythonLanguage);
    assert.equal((languageResult.relations as KnowledgeGraph['relations'])[0]!.display.statement.fr, 'Attribution non établie.');
    const selected = (languageResult.relations as KnowledgeGraph['relations'])[0]!.display_selection as {
      fields: {statement: {selected_key: string; reason: string}};
      content_revision: string; essential_context_pointers: string[];
    };
    assert.equal(selected.fields.statement.selected_key, 'fr');
    assert.equal(selected.fields.statement.reason, 'less-specific-language');
    assert.equal(selected.content_revision, scoped.relations[0]!.content_revision);
    assert.deepEqual(selected.essential_context_pointers, ['/semantics/assertion_contexts/0']);
    const sourceFormNode = realFormNode();
    scoped.nodes.push(sourceFormNode);
    await db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(sourceFormNode.id, sourceFormNode.entity_id,
      sourceFormNode.native_id, sourceFormNode.source_graph, sourceFormNode.kind_id, sourceFormNode.type_id,
      sourceFormNode.display.title.default.toLowerCase(), JSON.stringify(sourceFormNode).toLowerCase(), JSON.stringify(sourceFormNode)).run();
    const formSpec = {...base, sources: ['source-claims'], language: 'ru', detail: 'compact',
      seed: {focus_node_id: sourceFormNode.id}, node_query: {enabled: false}, relation_query: {enabled: false}};
    const formResult = await executeKnowledgeLensD1(db, formSpec);
    assert.deepEqual(formResult, await executeKnowledgeLens(scoped, formSpec));
    const pythonForms = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: formSpec}), encoding:'utf8'}));
    assert.deepEqual(formResult, pythonForms);
    const delivered = (formResult.nodes as KnowledgeGraph['nodes'])[0]!;
    assert.deepEqual(delivered.attributes, {});
    const selectedForms = delivered.human_form_selection as ReturnType<typeof selectHumanForms>;
    assert.equal(selectedForms.roles.name!.state, 'ready');
    assert.equal(selectedForms.roles.hover!.state, 'ready');
    assert.equal(selectedForms.roles.name!.packet!.display_text, 'По ту сторону добра и зла');
    assert.equal(selectedForms.roles.name!.packet!.standalone_reading, false);
    const claimNode = claimFormNode();
    scoped.nodes.push(claimNode);
    await db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(claimNode.id, claimNode.entity_id,
      claimNode.native_id, claimNode.source_graph, claimNode.kind_id, claimNode.type_id,
      claimNode.display.title.default.toLowerCase(), JSON.stringify(claimNode).toLowerCase(), JSON.stringify(claimNode)).run();
    const claimSpec = {...formSpec, seed: {focus_node_id: claimNode.id}};
    const claimResult = await executeKnowledgeLensD1(db, claimSpec);
    assert.deepEqual(claimResult, await executeKnowledgeLens(scoped, claimSpec));
    const pythonClaim = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: claimSpec}), encoding:'utf8'}));
    assert.deepEqual(claimResult, pythonClaim);
    const claimPacket = ((claimResult.nodes as KnowledgeGraph['nodes'])[0]!.human_form_selection as ReturnType<typeof selectHumanForms>).roles.statement!.packet!;
    assert.equal(claimPacket.standalone_reading, false);
    assert.equal(claimPacket.admission, null);
    assert.deepEqual((claimPacket.context as {value: unknown}[])[0]!.value, claimNode.attributes.source_claim);
    scoped.nodes[1]!.source_graph = 'repository';
    await db.prepare("UPDATE knowledge_nodes SET source_graph='repository', json=? WHERE id=?").bind(JSON.stringify(scoped.nodes[1]), 'philosophy:b').run();
    assert.deepEqual(await executeKnowledgeLensD1(db,joined), await executeKnowledgeLens(scoped,joined));
    // Synthetic topology with an explicit disputed Claim; no historical fact
    // follows from the test's normalized endpoint declarations.
    const pathGraph = structuredClone(graph);
    const pathClaim = pathGraph.nodes[2]!;
    pathClaim.type_id = 'tos.entity.claim';
    pathClaim.kind_id = 'claim';
    pathClaim.semantics = {claim: {subject_node_id: 'philosophy:a', object_node_id: 'philosophy:b',
      relation_type_id: 'tos.relation.correspondence-addressee', predicate_mapping_status: 'mapped', review_status: 'contested'},
      assertion_contexts: [{fields: {polarity: {value: 'negative'}, qualifiers: {value: {'unknown-extension': false}}}}]};
    pathClaim.display.summary = {default: 'Disputed attribution.', ru: null, en: 'Disputed attribution.'};
    pathClaim.display.provenance.source_summary_available = true;
    pathGraph.relations.forEach((r, i) => {
      r.from_id = pathClaim.id; r.to_id = pathGraph.nodes[i]!.id;
      r.relation_type_id = i ? 'tos.relation.has-object' : 'tos.relation.has-subject';
    });
    for (const n of pathGraph.nodes) await db.prepare('UPDATE knowledge_nodes SET source_graph=?,kind_id=?,type_id=?,json=? WHERE id=?')
      .bind(n.source_graph,n.kind_id,n.type_id,JSON.stringify(n),n.id).run();
    for (const r of pathGraph.relations) await db.prepare('UPDATE knowledge_relations SET from_id=?,to_id=?,relation_type_id=?,json=? WHERE id=?')
      .bind(r.from_id,r.to_id,r.relation_type_id,JSON.stringify(r),r.id).run();
    for (const focus of ['philosophy:a', 'philosophy:c']) for (const paging of [null, {nodes: 1, relations: 1}]) {
      const spec = {...base, detail: 'compact', language: 'en', seed: {focus_node_id: focus}, pagination: paging};
      const result = await executeKnowledgeLens(pathGraph,spec);
      assert.deepEqual(await executeKnowledgeLensD1(db,spec),result);
      assert.deepEqual(result,python(spec,pathGraph));
      const view = (result.scene as {compact:{claim_paths:{claim_node_id:string;reading:{standalone:boolean}}[]}}).compact;
      assert.equal(view.claim_paths.length,focus === 'philosophy:a' && paging === null ? 1 : 0);
      if (view.claim_paths.length) {
        assert.equal(view.claim_paths[0]!.claim_node_id,pathClaim.id);
        assert.equal(view.claim_paths[0]!.reading.standalone,false);
      }
    }
  } finally { await mf.dispose(); }
});

test('source human forms preserve ambiguity, exact context and bounded delivery across Python and Worker', () => {
  const node = realFormNode();
  const cases: {item: Record<string, unknown>; language: string}[] = ['ru', 'ru-RU', 'auto', 'original', 'fr'].map(language => ({item: node, language}));
  const changed = structuredClone(node);
  changed.attributes.source_sha256 = '0'.repeat(64);
  cases.push({item: changed, language: 'ru'});
  const large = structuredClone(node);
  const largeForms = large.attributes.human_forms as Record<string, unknown>[];
  largeForms[1]!.display_text = '界'.repeat(16000);
  cases.push({item: large, language: 'ru'});
  const restricted = structuredClone(node);
  for (const packet of restricted.attributes.human_forms as Record<string, unknown>[]) {
    Object.assign(packet, {state: 'restricted', display_text: null, context: []});
  }
  cases.push({item: restricted, language: 'ru'});
  const leaked = structuredClone(restricted);
  (leaked.attributes.human_forms as Record<string, unknown>[])[0]!.display_text = 'must not be emitted';
  cases.push({item: leaked, language: 'ru'});
  const invalidVersion = structuredClone(node);
  ((invalidVersion.attributes.human_forms as Record<string, unknown>[])[0]!.subject as Record<string, unknown>).version = true;
  cases.push({item: invalidVersion, language: 'ru'});
  const nullForms = structuredClone(node);
  nullForms.attributes.human_forms = null;
  cases.push({item: nullForms, language: 'ru'});
  const missingLanguage = structuredClone(node);
  delete (missingLanguage.attributes.human_forms as Record<string, unknown>[])[0]!.language;
  cases.push({item: missingLanguage, language: 'ru'});
  const missingNull = structuredClone(restricted);
  delete (missingNull.attributes.human_forms as Record<string, unknown>[])[0]!.display_text;
  cases.push({item: missingNull, language: 'ru'});
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;p=json.load(sys.stdin);print(json.dumps([select_human_forms(c['item'],c['language']) for c in p]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8'}));
  const results = cases.map(({item, language}) => selectHumanForms(item, language));
  assert.deepEqual(results, python);
  assert.equal(results[0]!.roles.name!.state, 'ready');
  assert.equal(results[1]!.roles.name!.reason, 'less-specific-language');
  assert.equal(results[2]!.roles.name!.state, 'ambiguous');
  assert.equal(results[3]!.roles.name!.reason, 'original-role-not-declared');
  assert.equal(results[5]!.state, 'invalid');
  assert.equal(results[6]!.roles.name!.state, 'over-budget');
  assert.ok(results[6]!.roles.name!.form);
  assert.equal(results[6]!.roles.name!.packet, null);
  assert.equal(results[7]!.roles.name!.state, 'unavailable');
  assert.equal(results[8]!.state, 'invalid');
  assert.equal(results[9]!.state, 'invalid');
  for (const result of results.slice(10)) assert.equal(result.state, 'invalid');
  for (const result of results) {
    assert.ok(formDeliveryCost(result) <= HUMAN_FORM_SELECTION_BUDGET);
    assert.ok(new TextEncoder().encode(JSON.stringify(result)).length <= HUMAN_FORM_SELECTION_BUDGET);
  }
  // Returned delivery objects cannot mutate the cached source graph.
  results[0]!.roles.name!.packet!.display_text = 'modified only in the result';
  assert.equal(selectHumanForms(node, 'ru').roles.name!.packet!.display_text, 'По ту сторону добра и зла');
});

test('Claim forms bind the assertion rather than its object in Python and Worker', () => {
  const node = claimFormNode();
  const conflicting = structuredClone(node);
  conflicting.attributes.source_record = {record_id: node.entity_id, record_version: 1};
  const wrongIdentity = structuredClone(node);
  wrongIdentity.entity_id = 'tos.letter.not-the-claim';
  const changed = structuredClone(node);
  (changed.attributes.source_claim as Record<string, unknown>).claim_version = 2;
  const missing = structuredClone(node);
  delete missing.attributes.source_claim;
  const cases = [node, conflicting, wrongIdentity, changed, missing];
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(n,'ru') for n in json.load(sys.stdin)]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8'}));
  const results = cases.map(item => selectHumanForms(item, 'ru'));
  assert.deepEqual(results, python);
  assert.equal(results[0]!.roles.statement!.state, 'ready');
  for (const result of results.slice(1)) assert.equal(result.state, 'invalid');
});

test('display selection keeps fallback, original language and ambiguity observable', () => {
  const forms = {default: 'Unspecified language', original: 'λόγος', fr: 'mot',
    'zh-Hant': '詞', de: 'Wort', 'x-research': 'Unassessed wording'};
  for (const [requested, key, reason] of [
    ['FR', 'fr', 'exact-language'], ['fr-CA', 'fr', 'less-specific-language'],
    ['zh-Hant-TW', 'zh-Hant', 'less-specific-language'], ['de-DE-u-co-phonebk', 'de', 'less-specific-language'],
    ['x-research', 'x-research', 'exact-language'], ['es', 'default', 'fallback'],
    ['auto', 'default', 'automatic'], ['original', 'original', 'original-role'],
  ]) {
    const result = selectDisplayForm(forms, requested, 'grc-Grek');
    assert.equal(result.selected_key, key);
    assert.equal(result.reason, reason);
    assert.equal(result.actual_language, key === 'original' ? 'grc-Grek' : key === 'default' ? null : key);
  }
  assert.equal(selectDisplayForm(forms, 'original').actual_language, null);
  assert.equal(selectDisplayForm({fr: null}, 'fr').reason, 'missing');
  const ambiguous = selectDisplayForm({fr: 'oui', FR: 'non', default: 'fallback'}, 'fr-CA');
  assert.equal(ambiguous.reason, 'ambiguous-language-key');
  assert.equal(ambiguous.text, null);
  assert.deepEqual(ambiguous.available_keys, ['FR', 'default', 'fr']);
});

test('source linguistic context controls original selection without an inferred historical claim', () => {
  // Synthetic context added to the transport fixture is not a Jenseits source judgment.
  const node = realFormNode();
  const packet = (node.attributes.human_forms as Record<string, unknown>[])[0]!;
  const metadata = {binding: {record: {id: 'tos.record.language-contract', version: 1, digest: 'sha256:' + 'd'.repeat(64)}, pointer: ''},
    value: {language: packet.language, script: packet.script, relation: 'original', source: null, 'x-unknown': false}};
  packet.language_context = metadata;
  (packet.dependencies as Record<string, unknown>[]).push(metadata.binding.record);
  (packet.context as Record<string, unknown>[]).push({slot: 'language_context', ...structuredClone(metadata)});
  const absent = structuredClone(node);
  (absent.attributes.human_forms as Record<string, unknown>[])[0]!.context = [];
  const changed = structuredClone(node);
  const context = (changed.attributes.human_forms as Record<string, unknown>[])[0]!.context as Record<string, unknown>[];
  (context.at(-1)!.value as Record<string, unknown>)['x-unknown'] = 0;
  const multiple = structuredClone(node);
  const another = structuredClone(packet);
  another.form = {...another.form as Record<string, unknown>, id: 'tos.form.competing-original'};
  (multiple.attributes.human_forms as Record<string, unknown>[]).push(another);
  const missingDependency = structuredClone(node);
  (missingDependency.attributes.human_forms as Record<string, unknown>[])[0]!.dependencies = [];
  const cases = [node, absent, changed, multiple, missingDependency];
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(n,'original') for n in json.load(sys.stdin)]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8'}));
  const results = cases.map(item => selectHumanForms(item, 'original'));
  assert.deepEqual(results, python);
  assert.equal(results[0]!.roles.name!.reason, 'original');
  assert.deepEqual(results[0]!.roles.name!.packet, packet);
  assert.equal(results[1]!.state, 'invalid');
  assert.equal(results[2]!.state, 'invalid');
  assert.equal(results[3]!.roles.name!.state, 'ambiguous');
  assert.equal(results[4]!.state, 'invalid');
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

test('language fallback does not manufacture a translation or discard private-use forms', () => {
  const languageLimit = 'fr' + '-abcdefgh'.repeat(14);
  assert.equal(languageLimit.length, 128);
  const bounded = {schema_version: 'tos_lens_spec_v1', lens_id: 'bounded-language', language: languageLimit};
  assert.equal(normalizeLensSpec(bounded).language, languageLimit);
  assert.throws(() => normalizeLensSpec({...bounded, language: 'fra' + languageLimit.slice(2)}), /128/);
  for (const key of ['fr', 'zh-Hant', 'x-research', 'original']) {
    const spec = normalizeLensSpec({schema_version: 'tos_lens_spec_v1', lens_id: 'languages',
      language: key, title: {[key]: 'Exact source wording.'}});
    assert.equal(spec.title.default, 'Exact source wording.');
    assert.equal(spec.title[key], 'Exact source wording.');
    assert.equal(spec.title.ru, null);
    assert.equal(spec.title.en, null);
  }
  for (const key of ['fr\n', 'fr_CA', '__proto__', 'script.js']) {
    assert.throws(() => normalizeLensSpec({schema_version: 'tos_lens_spec_v1', lens_id: 'invalid-language',
      title: {[key]: 'not a declared form key'}}), /unknown title fields/);
  }
  for (const field of ['display.title.__proto__', 'display.title.fr.name', 'display.summary_state.fr']) {
    assert.throws(() => normalizeLensSpec({schema_version: 'tos_lens_spec_v1', lens_id: 'unsafe-language',
      node_query: {filters: [{field, op: 'eq', value: 'no'}]}}), /unsupported node filter field/);
  }
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
    () => normalizeLensSpec({ schema_version: "tos_lens_spec_v1", lens_id: "bad-title-field", title: { default: "valid", html_markup: "no" } }),
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
