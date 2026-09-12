import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import test from "node:test";
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { knowledgeSearchD1, knowledgeSearchD1Indexed, knowledgeNodeD1, knowledgeRelationD1 } from "../src/knowledge-store.ts";
import {nativePacketJson} from '../src/native-lens.ts';

import {executePublishedFixtureLens, publishNativeLensFixture} from './native-lens-fixture.ts';
import { executeKnowledgeLens, focusKnowledgeNode, knowledgeScene, normalizeLensSpec, selectDisplayForm, type KnowledgeGraph } from "../src/knowledge.ts";
import { selectHumanForms, formDeliveryCost, HUMAN_FORM_SELECTION_BUDGET } from '../src/human-forms.ts';
import { decodeHumanFormSelection } from '../../../shared/human-form-selection-codec.ts';

const knowledgeExplorationMigration = readFileSync(
  new URL('../migrations/0001-exploration.sql', import.meta.url),
  'utf8',
).replace(/^--.*$/gm, '').trim();

async function applyKnowledgeExplorationMigration(db: D1Database): Promise<void> {
  await db.batch(knowledgeExplorationMigration.split(/\n(?=CREATE |INSERT )/).map((statement) => db.prepare(statement)));
}

function decodeIndexedCursor(value: string): Record<string, unknown> {
  return JSON.parse(Buffer.from(value, "base64url").toString("utf8")) as Record<string, unknown>;
}

function encodeIndexedCursor(value: Record<string, unknown>): string {
  return Buffer.from(JSON.stringify(value), "utf8").toString("base64url");
}

function decodeDeliveredHumanForms(value: unknown): ReturnType<typeof selectHumanForms> {
  return decodeHumanFormSelection(value) as ReturnType<typeof selectHumanForms>;
}

function hasHttpStatus(status: number) {
  return (error: unknown): boolean => Boolean(
    error && typeof error === "object" && "status" in error && (error as { status?: unknown }).status === status,
  );
}

function realFormNode(): KnowledgeGraph['nodes'][number] {
  return JSON.parse(execFileSync('python3', ['-c',
    "import sys,json,pathlib;sys.path.insert(0,'access/src');from tos_access.knowledge import _normalize_node;g=json.loads(pathlib.Path('ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json').read_text());n=next(n for n in g['nodes'] if n['properties'].get('identity_ref')=='tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese');print(json.dumps(_normalize_node(n,'source-claims')))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding:'utf8'}));
}

function assessedFormNode(): KnowledgeGraph['nodes'][number] {
  // Synthetic admission of existing source-copy wording, not a real review.
  const node = realFormNode();
  const packet = structuredClone((node.attributes.human_forms as Record<string, unknown>[]).find(p => p.role === 'hover')!);
  Object.assign(packet, {derivation: 'freeform', assessment_snapshot: {
    owner_snapshot: 'sha256:' + 'd'.repeat(64), journal_revision: 'e'.repeat(64), journal_batches: 1,
    publication_authorized: false, current_runtime_grant: false},
    admission: {schema_version: 'tos_knowledge_admission_v1', subject: structuredClone(packet.form),
      policy: {id: 'tos.policy.fixture', version: 1, digest: 'sha256:' + 'f'.repeat(64)},
      status: 'admitted', can_use: true, is_semantic_evaluation: false, use: 'research'}});
  node.attributes.human_forms = [packet];
  return node;
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

function claimNavigationFixture(): {graph: KnowledgeGraph; fullGraph: KnowledgeGraph; fullScene: unknown;
  claims: string[]; versions: string[]; cases: {spec: unknown; expected: unknown}[]} {
  // Build disposable navigation carriers from three real legacy Claims and
  // their exact public identity records. No source/form files are changed;
  // this transport fixture does not assert graph-wide source closure.
  return JSON.parse(execFileSync('python3', ['-c', `
import copy, json, pathlib, sys
sys.path[:0] = ['access/src', 'scripts']
from source_witness_bibliographic_graph_common import build_claim_navigation_descriptor
from tos_access.knowledge import (_entity_registry_indexes, _relation_registry_indexes,
    _validate_claim_navigation_carriers, _normalize_node, _normalize_relation,
    _source_claim_kind, _finalize_knowledge_node, _stable_digest, _exact_record_digest,
    execute_knowledge_lens, knowledge_scene)
root = pathlib.Path('.')
raw = json.loads((root / 'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json').read_text())
entities = json.loads((root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
relations = json.loads((root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())
claim_ids = {
    'tos.claim.expression.also-sprach-zarathustra.ru-nani-1899-nine-fragments.translated-by-s-p-nani',
    'tos.claim.topology.expression-edition.friedrich-nietzsche.also-sprach-zarathustra.ru-nani-1899-nine-fragments.embodied-by.saint-petersburg-stasyulevich-1899-nine-fragments',
    'tos.claim.topology.work-expression.friedrich-nietzsche.also-sprach-zarathustra.has-expression.ru-nani-1899-nine-fragments',
}
traces = [trace for trace in raw['claim_traces'] if trace['claim_ref'] in claim_ids]
assert len(traces) == len(claim_ids)
selected_ids = {trace[key] for trace in traces for key in ('claim_node_id', 'subject_node_id', 'object_node_id')}
incident_edges = [edge for edge in raw['edges'] if edge['claim_ref'] in claim_ids]
full_ids = selected_ids | {edge[key] for edge in incident_edges for key in ('from_id', 'to_id')}
selected = [copy.deepcopy(node) for node in raw['nodes'] if node['node_id'] in full_ids]
by_native = {node['node_id']: node for node in selected}
entity_entries, entity_mappings, entity_fallback = _entity_registry_indexes(entities)
relation_entries, relation_mappings, relation_fallback = _relation_registry_indexes(relations)
for trace in traces:
    carrier = by_native[trace['claim_node_id']]
    carrier['properties']['navigation_descriptor'] = build_claim_navigation_descriptor(
        carrier['properties']['source_claim'], by_native[trace['subject_node_id']],
        by_native[trace['object_node_id']], relations, entities)
_validate_claim_navigation_carriers(selected, relations, entity_entries, entity_mappings)
nodes = [_normalize_node(node, 'source-claims', source_kind_id=_source_claim_kind(node),
    entity_type_entries=entity_entries, entity_type_mappings=entity_mappings,
    fallback_type_id=entity_fallback) for node in selected]
by_id = {node['id']: node for node in nodes}
for trace in traces:
    identifier = 'source-claims:' + trace['claim_node_id']
    node = by_id[identifier]
    claim = {**node['semantics']['claim'],
        'subject_node_id': 'source-claims:' + trace['subject_node_id'],
        'object_node_id': 'source-claims:' + trace['object_node_id'],
        'relation_type_id': relation_mappings[('source-claims', trace['predicate'], 'claim-predicate')],
        'predicate_mapping_status': 'mapped'}
    by_id[identifier] = _finalize_knowledge_node(node, (claim, trace), [])
# Synthetic exact-version transport controls, not a real historical judgment
# or archive verification. The source-owner end-to-end tests cover that chain.
record = copy.deepcopy(by_native[traces[0]['claim_node_id']]['properties']['source_claim'])
record.update(claim_id='tos.claim.synthetic-exact-version-transport', claim_version=1)
record['qualifiers'] = {'statement': 'Keine gesicherte Zuschreibung; synthetischer Transporttest.',
    'statement_language': 'de', 'statement_script': 'Latn', 'polarity': 'negative',
    'unknown_extension': {'false': False, 'zero': 0, 'null': None, 'empty': []}}
derived_versions = []
version_cases = [('claim', record)]
for language in ('de', None):
    metadata = {'record_id': 'tos.agent.synthetic-version-transport-' + str(language).lower(),
        'record_type': 'agent', 'record_version': 4, 'preferred_label': 'Synthetic historical description',
        'notes': 'Keine gesicherte Gleichsetzung; synthetischer Metadatentest.',
        'field_languages': {'notes': {'language': language, 'script': 'Latn'}},
        'unknown_extension': {'polarity': 'negative', 'false': False, 'zero': 0, 'null': None, 'empty': []}}
    version_cases.append(('metadata', metadata))
for record_kind, record, available in [(kind, value, state) for kind, value in version_cases for state in (True, False)]:
    reference = {'id': record['claim_id'] if record_kind == 'claim' else record['record_id'],
        'version': record['claim_version'] if record_kind == 'claim' else record['record_version'],
        'digest': 'sha256:' + _exact_record_digest(record)}
    ref = reference if available else {**reference, 'version': 2, 'digest': 'sha256:' + '0' * 64}
    view = {'schema_version': 'tos_record_version_view_v1', 'record_ref': ref, 'record_kind': record_kind,
        'status': 'available' if available else 'missing', 'reason': 'synthetic-transport-only',
        'version_status': 'historical' if available else None, 'record': record if available else None,
        'provenance': {'fixture': 'not-an-archive-verification'} if available else {},
        'grants_current_use': False, 'performs_assessment': False}
    carrier = {'node_id': 'record-version:' + _exact_record_digest(ref), 'node_kind': 'record-version',
        'source_ref': 'test:synthetic-exact-version-transport', 'properties': {'record_version_view': view}}
    node = _normalize_node(carrier, 'source-navigation', entity_type_entries=entity_entries,
        entity_type_mappings=entity_mappings, fallback_type_id=entity_fallback)
    derived_versions.append(node)
    by_id[node['id']] = node
full_edges = [_normalize_relation({**edge, 'predicate_id': edge['edge_kind'],
    'source_ref': edge['source_claim_file_ref'], 'graph_layers': ['bibliographic-claim']}, 'source-claims', by_id,
    relation_type_entries=relation_entries, relation_type_mappings=relation_mappings,
    fallback_relation_type_id=relation_fallback) for edge in incident_edges]
full_graph = {'schema': 'tos_knowledge_graph_v1', 'source_revision': _stable_digest([selected, relations, entities, derived_versions]),
    'nodes': list(by_id.values()), 'relations': full_edges,
    'counts': {'nodes': len(by_id), 'relations': len(full_edges)},
    'authority_boundary': {'source_owner': 'Tree-of-Sophia', 'is_source': False, 'is_canon': False, 'writes_to_tree': False}}
graph = {**full_graph, 'nodes': [node for node in by_id.values()
    if node['native_id'] in selected_ids or node['type_id'] == 'tos.entity.record-version'],
    'relations': [edge for edge in full_edges if edge['predicate_id'] in {'has_subject', 'has_object'}]}
graph['counts'] = {'nodes': len(graph['nodes']), 'relations': len(graph['relations'])}
specs = [{'schema_version': 'tos_lens_spec_v1', 'lens_id': 'claim-navigation-transport',
    'sources': ['source-claims', 'source-navigation'], 'language': language, 'detail': detail}
    for language in ('ru', 'en') for detail in ('compact', 'full')]
print(json.dumps({'graph': graph, 'fullGraph': full_graph,
    'fullScene': knowledge_scene(full_graph['nodes'], full_graph['relations']),
    'claims': sorted('source-claims:claim:' + identity for identity in claim_ids),
    'versions': sorted(node['id'] for node in derived_versions),
    'cases': [{'spec': spec, 'expected': execute_knowledge_lens(graph, spec)} for spec in specs]}))
`], {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding: 'utf8', maxBuffer: 4 * 1024 * 1024}));
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
  authority_boundary: { source_owner: 'Tree-of-Sophia', is_source: false, is_canon: false, writes_to_tree: false },
};

test("knowledge reads require the publication clock and reject invalid or ABA snapshots", async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules: true,
    script: 'export default {fetch(){return new Response()}}', d1Databases: ['DB']}));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare('CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision', 0, ?)").bind(JSON.stringify({sha256: graph.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top', 0, ?)").bind(JSON.stringify({source_revision: graph.source_revision, authority_boundary: graph.authority_boundary})),
      ...graph.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(
        n.id, n.entity_id, n.native_id, n.source_graph, n.kind_id, n.type_id,
        n.display.title.default.toLowerCase(), JSON.stringify(n).toLowerCase(), JSON.stringify(n))),
      ...graph.relations.map(r => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)').bind(
        r.id, r.native_id, r.source_graph, r.from_id, r.to_id, r.predicate_id, r.relation_type_id,
        r.display.label.default.toLowerCase(), JSON.stringify(r).toLowerCase(), JSON.stringify(r))),
    ]);
    const request = {query: '', sources: null, kindIds: [], predicateIds: [], offset: 0, limit: 2};
    await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503),
      'a database without the additive publication-clock migration is not ready');
    await applyKnowledgeExplorationMigration(db);

    await db.prepare('UPDATE knowledge_exploration_clock SET epoch=? WHERE singleton=1').bind(-1).run();
    await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
    await db.prepare('UPDATE knowledge_exploration_clock SET epoch=? WHERE singleton=1').bind(0).run();

    for (const epoch of [1.5, Number.MAX_SAFE_INTEGER + 1]) {
      await db.prepare('UPDATE knowledge_exploration_clock SET epoch=? WHERE singleton=1').bind(epoch).run();
      await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
    }
    await db.prepare('UPDATE knowledge_exploration_clock SET epoch=? WHERE singleton=1').bind(0).run();

    for (const value of [null, [], {sha256: ''}]) {
      await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
        .bind(JSON.stringify(value)).run();
      await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
      await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
        .bind(JSON.stringify({sha256: graph.source_revision})).run();
    }

    // The production schema has a singleton CHECK/PRIMARY KEY, but readiness
    // must still fail closed if a legacy or hand-built table violates it.
    await db.prepare('DROP TABLE knowledge_exploration_clock').run();
    await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
    await db.batch([
      db.prepare('CREATE TABLE knowledge_exploration_clock (singleton INTEGER PRIMARY KEY CHECK(singleton=1), epoch INTEGER NOT NULL)'),
      db.prepare('INSERT INTO knowledge_exploration_clock VALUES (1,0)'),
    ]);
    await db.prepare('DROP TABLE knowledge_exploration_clock').run();
    await db.batch([
      db.prepare('CREATE TABLE knowledge_exploration_clock (singleton INTEGER, epoch INTEGER)'),
      db.prepare('INSERT INTO knowledge_exploration_clock VALUES (1,0)'),
      db.prepare('INSERT INTO knowledge_exploration_clock VALUES (1,0)'),
    ]);
    await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
    await db.prepare('DROP TABLE knowledge_exploration_clock').run();
    await db.batch([
      db.prepare('CREATE TABLE knowledge_exploration_clock (singleton INTEGER PRIMARY KEY CHECK(singleton=1), epoch INTEGER NOT NULL)'),
      db.prepare('INSERT INTO knowledge_exploration_clock VALUES (1,0)'),
    ]);

    // A revision split with no part zero must not be treated as a valid
    // one-part revision by the guard.
    await db.prepare("UPDATE edge_meta SET part=1 WHERE key='data_revision'").run();
    await assert.rejects(() => knowledgeSearchD1(db, request), hasHttpStatus(503));
    await db.prepare("UPDATE edge_meta SET part=0 WHERE key='data_revision'").run();

    const originalPrepare = db.prepare.bind(db);
    let snapshotReads = 0;
    const intercepted = new Proxy(db, {get(target, property) {
      if (property === 'prepare') return (sql: string) => {
        const statement = originalPrepare(sql);
        if (!sql.includes('knowledge_exploration_clock')) return statement;
        snapshotReads += 1;
        if (snapshotReads !== 2) return statement;
        const originalFirst = statement.first.bind(statement) as <T>(columnName?: string) => Promise<T | null>;
        return new Proxy(statement, {get(inner, method, receiver) {
          if (method !== 'first') return Reflect.get(inner, method, receiver);
          return async <T>(columnName?: string) => {
            await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
              .bind(JSON.stringify({sha256: 'b'.repeat(64)})).run();
            await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
              .bind(JSON.stringify({sha256: graph.source_revision})).run();
            return originalFirst<T>(columnName);
          };
        }});
      };
      const value = Reflect.get(target, property);
      return typeof value === 'function' ? value.bind(target) : value;
    }});
    await assert.rejects(() => knowledgeSearchD1(intercepted, request), hasHttpStatus(409),
      'the epoch rejects A->B->A even when the digest returns to A');
    assert.equal(snapshotReads, 2);
  } finally {
    await mf.dispose();
  }
});

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
    await applyKnowledgeExplorationMigration(db);
    const base = { schema_version: 'tos_lens_spec_v1', lens_id: 'path-parity', sources: ['philosophy'], explain: true };
    for (const direction of ['outgoing', 'incoming', 'either']) {
      for (const quantifier of ['exists', 'not_exists']) {
        for (const length of [1, 2, 3, 4]) {
          const spec = {...base, path_query: [{path_id: 'p', quantifier, steps: Array.from({length}, () => ({direction}))}]};
          assert.deepEqual(await executePublishedFixtureLens(db, spec), await executeKnowledgeLens(graph, spec));
        }
      }
    }
    const joined = {...base, path_query: [
      {path_id: 'agent', steps: [{}, {node_query: {filters: [{field: 'type_id', op: 'eq', value: 'tos.entity.agent'}]}}]},
      {path_id: 'work', steps: [{node_query: {filters: [{field: 'type_id', op: 'eq', value: 'tos.entity.work'}]}}]}
    ]};
    const joinedResult = await executePublishedFixtureLens(db, joined);
    assert.deepEqual(joinedResult, await executeKnowledgeLens(graph, joined));
    assert.deepEqual((joinedResult.nodes as {id:string}[]).map(n=>n.id), ['philosophy:a']);
    const focused = {...base, seed: {focus_node_id: 'philosophy:a'}, node_query: {enabled: false}, traversal: {depth: 2}};
    assert.deepEqual(await executePublishedFixtureLens(db, focused), await executeKnowledgeLens(graph, focused));
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
        assert.deepEqual(await executePublishedFixtureLens(db, spec), pure);
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
        assert.deepEqual(await executePublishedFixtureLens(db, spec), pure);
        assert.equal(JSON.stringify(pure.lens).includes('_property_binding'), false);
      }
    }
    for (const change of [{property_id: 'tos.property.unknown'}, {op: 'contains'}, {value: true},
                         {value: '3'}, {field: 'attributes.score'}, {property_id: 'tos.property.bad\r'},
                         {property_id: 'tos.property.bad\n'}, {property_id: null}]) {
      const spec = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-score', op: 'eq', value: 3, ...change}]}};
      await assert.rejects(executeKnowledgeLens(propertyGraph, spec));
      await assert.rejects(executePublishedFixtureLens(db, spec));
    }
    for (const [name, op, value] of [['word', 'eq', 'Свобода Ω 🦉\u0000fin'], ['word', 'contains', 'Ω 🦉'],
        ['word', 'prefix', 'Свобода Ω 🦉\u0000fi'], ['word', 'prefix', ''],
        ['word', 'prefix', 'Свобода'], ['word', 'prefix', 'свобода'], ['word', 'contains', ['Свобода']],
        ['flag', 'eq', false], ['flag', 'neq', true], ['tags', 'eq', 'Мысль'],
        ['tags', 'in', ['Freiheit']], ['tags', 'contains', ['Мысль', 'Freiheit']]] as const) {
      const spec = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-' + name, op, value}]}};
      const pure = await executeKnowledgeLens(propertyGraph, spec);
      assert.deepEqual(pure, python(spec, propertyGraph));
      assert.deepEqual(await executePublishedFixtureLens(db, spec), pure);
    }
    // Returning to the old snapshot removes the binding as well as the synthetic values.
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'").bind(JSON.stringify({
      source_revision: graph.source_revision, authority_boundary: graph.authority_boundary})).run();
    for (const n of graph.nodes) await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(n), n.id).run();
    const absentProperty = {...base, node_query: {filters: [{property_id: 'tos.property.fixture-score', op: 'eq', value: 3}]}};
    await assert.rejects(executeKnowledgeLens(graph, absentProperty));
    await assert.rejects(executePublishedFixtureLens(db, absentProperty));
    const carriers = structuredClone(graph);
    carriers.nodes[1]!.entity_id = carriers.nodes[0]!.entity_id;
    await db.prepare('UPDATE knowledge_nodes SET entity_id=?, json=? WHERE id=?')
      .bind(carriers.nodes[1]!.entity_id, JSON.stringify(carriers.nodes[1]), carriers.nodes[1]!.id).run();
    for (const paging of [null, {nodes: 1, relations: 1}]) {
      const spec = {...focused, pagination: paging};
      const pure = await executeKnowledgeLens(carriers, spec);
      assert.deepEqual(await executePublishedFixtureLens(db, spec), pure);
      assert.deepEqual(pure, python(spec, carriers));
      const scene = pure.scene as {vertices: {node_ids: string[]}[]};
      assert.equal(scene.vertices.filter(v => v.node_ids.includes('philosophy:a'))[0]!.node_ids.length, 2);
    }
    for (const profile of ['overview', 'all']) for (const size of [1, 3]) {
      const spec = {...focused, traversal: {depth: 1, profile}, limits: {nodes: size}};
      const pure = await executeKnowledgeLens(carriers, spec);
      assert.deepEqual(await executePublishedFixtureLens(db, spec), pure);
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
      const page = await executePublishedFixtureLens(db, spec);
      assert.deepEqual(page, await executeKnowledgeLens(graph, spec));
      assert.deepEqual(page, python(spec));
      assert.equal(page.fingerprint, whole.fingerprint);
      const info = page.page as {primary_node_ids:string[]; next_cursor:string|null};
      nodeIds.push(...info.primary_node_ids);
      relationIds.push(...(page.relations as {id:string}[]).map(r=>r.id));
      cursor = info.next_cursor;
      if (!cursor) break;
      await assert.rejects(executePublishedFixtureLens(db,{...spec,lens_id:'different',pagination:{...spec.pagination,cursor}}), /query or snapshot changed/);
    }
    assert.deepEqual(nodeIds, whole.nodes.map(n=>n.id));
    assert.deepEqual(relationIds, whole.relations.map(r=>r.id));
    assert.deepEqual(await executePublishedFixtureLens(db, joined), python(joined));
    const httpSpec = {...focused, pagination: {nodes: 1, relations: 1, cursor: null as string|null}};
    const requestPage = async (spec: unknown) => {
      await publishNativeLensFixture(db);
      return mf.dispatchFetch('http://tos.test/api/knowledge/lenses/compile',
        {method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify(spec)});
    };
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
    await publishNativeLensFixture(db);
    for (const packet of [JSON.parse(nativePacketJson(await knowledgeNodeD1(db,'philosophy:a',0))), JSON.parse(nativePacketJson(await knowledgeRelationD1(db,'philosophy:e'))),
      await knowledgeSearchD1(db,{query:'',sources:null,kindIds:[],predicateIds:[],offset:0,limit:2})]) {
      assert.equal(packet.source_revision, graph.source_revision);
    }
    // Search ordering must agree with Python when a reader-visible form is
    // mixed with a digest-only match.  The substring filter remains broad;
    // only the deterministic display-field ordering changes.
    const rankingGraph = structuredClone(graph);
    rankingGraph.nodes[0]!.display.title.default = 'Unrelated node';
    rankingGraph.nodes[0]!.display.summary.default = 'A reader-visible note names 4363.';
    rankingGraph.nodes[1]!.display.title.default = 'Anchor · 4363';
    rankingGraph.nodes[2]!.attributes.content_revision = 'sha256:4363abc';
    rankingGraph.relations[0]!.display.statement.default = 'Anchor · 4363 — relates Beta.';
    rankingGraph.relations[1]!.attributes.content_revision = 'sha256:4363def';
    const restoreRows = async () => {
      for (const n of graph.nodes) await db.prepare('UPDATE knowledge_nodes SET json=?,search_text=? WHERE id=?')
        .bind(JSON.stringify(n), JSON.stringify(n).toLowerCase(), n.id).run();
      for (const r of graph.relations) await db.prepare('UPDATE knowledge_relations SET json=?,search_text=? WHERE id=?')
        .bind(JSON.stringify(r), JSON.stringify(r).toLowerCase(), r.id).run();
    };
    try {
      for (const n of rankingGraph.nodes) await db.prepare('UPDATE knowledge_nodes SET json=?,search_text=? WHERE id=?')
        .bind(JSON.stringify(n), JSON.stringify(n).toLowerCase(), n.id).run();
      for (const r of rankingGraph.relations) await db.prepare('UPDATE knowledge_relations SET json=?,search_text=? WHERE id=?')
        .bind(JSON.stringify(r), JSON.stringify(r).toLowerCase(), r.id).run();
      const expectedSearch = JSON.parse(execFileSync('python3', ['-c',
        "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import search_knowledge_graph;p=json.load(sys.stdin);print(json.dumps(search_knowledge_graph(p['graph'],p['query'],limit=p['limit'])))"],
        {cwd: fileURLToPath(new URL('../../../../', import.meta.url)),
          input: JSON.stringify({graph: rankingGraph, query: '4363', limit: 3}), encoding: 'utf8'}));
      const actualSearch = await knowledgeSearchD1(db,{query:'4363',sources:null,kindIds:[],predicateIds:[],offset:0,limit:3});
      assert.deepEqual(actualSearch, expectedSearch);
      assert.deepEqual((actualSearch.nodes as {id:string}[]).map(n => n.id), ['philosophy:a','philosophy:b','philosophy:c']);
      assert.deepEqual((actualSearch.relations as {id:string}[]).map(r => r.id), ['philosophy:e','philosophy:f']);
    } finally {
      await restoreRows();
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
    const languageResult = await executePublishedFixtureLens(db, languageSpec);
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
    const formResult = await executePublishedFixtureLens(db, formSpec);
    assert.deepEqual(formResult, await executeKnowledgeLens(scoped, formSpec));
    const pythonForms = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: formSpec}), encoding:'utf8'}));
    assert.deepEqual(formResult, pythonForms);
    const delivered = (formResult.nodes as KnowledgeGraph['nodes'])[0]!;
    assert.deepEqual(delivered.attributes, {});
    const selectedForms = decodeDeliveredHumanForms(delivered.human_form_selection);
    assert.equal(selectedForms.roles.name!.state, 'ready');
    assert.equal(selectedForms.roles.hover!.state, 'ready');
    assert.equal(selectedForms.roles.name!.packet!.display_text, 'По ту сторону добра и зла');
    assert.equal(selectedForms.roles.name!.packet!.standalone_reading, false);
    const assessedNode = assessedFormNode();
    const sourceFormIndex = scoped.nodes.findIndex(node => node.id === sourceFormNode.id);
    for (const state of ['ready', 'needs-assessment', 'invalid']) {
      const candidate = structuredClone(assessedNode);
      const packet = (candidate.attributes.human_forms as Record<string, unknown>[])[0]!;
      if (state === 'needs-assessment') Object.assign(packet, {state, display_text: null, context: [], admission: null});
      if (state === 'invalid') (packet.assessment_snapshot as Record<string, unknown>).publication_authorized = true;
      scoped.nodes[sourceFormIndex] = candidate;
      await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(candidate), candidate.id).run();
      const edge = await executePublishedFixtureLens(db, formSpec);
      assert.deepEqual(edge, await executeKnowledgeLens(scoped, formSpec));
      const pythonAssessed = JSON.parse(execFileSync('python3', ['-c',
        "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
        {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: formSpec}), encoding:'utf8'}));
      assert.deepEqual(edge, pythonAssessed);
      const forms = decodeDeliveredHumanForms((edge.nodes as KnowledgeGraph['nodes'])[0]!.human_form_selection);
      assert.equal(forms.roles.hover!.state === 'ready', state === 'ready');
      if (state === 'ready') assert.deepEqual(forms.roles.hover!.packet, packet);
      if (state === 'invalid') assert.equal(forms.state, 'invalid');
    }
    scoped.nodes[sourceFormIndex] = sourceFormNode;
    await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(sourceFormNode), sourceFormNode.id).run();
    const claimNode = claimFormNode();
    scoped.nodes.push(claimNode);
    await db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(claimNode.id, claimNode.entity_id,
      claimNode.native_id, claimNode.source_graph, claimNode.kind_id, claimNode.type_id,
      claimNode.display.title.default.toLowerCase(), JSON.stringify(claimNode).toLowerCase(), JSON.stringify(claimNode)).run();
    const claimSpec = {...formSpec, seed: {focus_node_id: claimNode.id}};
    const claimResult = await executePublishedFixtureLens(db, claimSpec);
    assert.deepEqual(claimResult, await executeKnowledgeLens(scoped, claimSpec));
    const pythonClaim = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import execute_knowledge_lens;p=json.load(sys.stdin);print(json.dumps(execute_knowledge_lens(p['graph'],p['spec'])))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify({graph: scoped, spec: claimSpec}), encoding:'utf8'}));
    assert.deepEqual(claimResult, pythonClaim);
    const claimPacket = decodeDeliveredHumanForms((claimResult.nodes as KnowledgeGraph['nodes'])[0]!.human_form_selection).roles.statement!.packet!;
    assert.equal(claimPacket.standalone_reading, false);
    assert.equal(claimPacket.admission, null);
    assert.deepEqual((claimPacket.context as {value: unknown}[])[0]!.value, claimNode.attributes.source_claim);
    scoped.nodes[1]!.source_graph = 'repository';
    await db.prepare("UPDATE knowledge_nodes SET source_graph='repository', json=? WHERE id=?").bind(JSON.stringify(scoped.nodes[1]), 'philosophy:b').run();
    assert.deepEqual(await executePublishedFixtureLens(db,joined), await executeKnowledgeLens(scoped,joined));
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
      assert.deepEqual(await executePublishedFixtureLens(db,spec),result);
      assert.deepEqual(result,python(spec,pathGraph));
      const view = (result.scene as {compact:{claim_paths:{claim_node_id:string;reading:{standalone:boolean}}[]}}).compact;
      assert.equal(view.claim_paths.length,
        (focus === 'philosophy:a' || focus === 'philosophy:c') && paging === null ? 1 : 0);
      if (view.claim_paths.length) {
        assert.equal(view.claim_paths[0]!.claim_node_id,pathClaim.id);
        assert.equal(view.claim_paths[0]!.reading.standalone,false);
      }
    }
  } finally { await mf.dispose(); }
});

test("indexed D1 search keeps exhausted kinds exhausted and matches bounded Python packet order", async () => {
  const bundle = await build({ entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
    bundle: true, write: false, format: 'esm', platform: 'browser', target: 'es2022' });
  const mf = new Miniflare(convertV4MiniflareOptions({ modules: true, script: bundle.outputFiles[0]!.text, d1Databases: ["DB"] }));
  try {
    const db = await mf.getD1Database("DB");
    const indexedGraph = structuredClone(graph);
    indexedGraph.nodes.push(
      {...indexedGraph.nodes[1]!, id: "philosophy:alpha-extra-1", native_id: "alpha-extra-1",
        display: {...indexedGraph.nodes[1]!.display, title: {default: "Alpha extra one"}, summary: {default: "Alpha extra one"}}},
      {...indexedGraph.nodes[2]!, id: "philosophy:alpha-extra-2", native_id: "alpha-extra-2",
        display: {...indexedGraph.nodes[2]!.display, title: {default: "Alpha extra two"}, summary: {default: "Alpha extra two"}}},
    );
    const searchable = (item: Record<string, unknown>) => JSON.stringify(item).toLowerCase();
    const grams = (value: string) => {
      const result = new Set<string>();
      for (let index = 0; index <= value.length - 3; index += 1) result.add(value.slice(index, index + 3));
      return [...result];
    };
    const documentRows: D1PreparedStatement[] = [];
    const gramRows: D1PreparedStatement[] = [];
    const stats = new Map<string, number>();
    for (const [kind, items] of [["nodes", indexedGraph.nodes], ["relations", indexedGraph.relations]] as const) {
      items.forEach((item, position) => {
        const json = JSON.stringify(item);
        const lower = searchable(item as unknown as Record<string, unknown>);
        const display = item.display as Record<string, unknown>;
        const primary = kind === "nodes"
          ? JSON.stringify([((display.title as Record<string, unknown>)?.default ?? "").toString().toLowerCase()])
          : JSON.stringify([((display.label as Record<string, unknown>)?.default ?? "").toString().toLowerCase()]);
        const visible = kind === "nodes"
          ? JSON.stringify([((display.title as Record<string, unknown>)?.default ?? "").toString().toLowerCase()])
          : JSON.stringify([((display.label as Record<string, unknown>)?.default ?? "").toString().toLowerCase()]);
        documentRows.push(db.prepare("INSERT INTO knowledge_search_documents VALUES (?,?,?,?,?,?,?,?,?,?,?,?)")
          .bind(kind, position, item.id, item.source_graph, item.kind_id ?? "", item.predicate_id ?? "",
            String(item.id).toLowerCase(), String(item.native_id).toLowerCase(), primary, visible, lower.length, ""));
        for (const gram of grams(lower)) {
          gramRows.push(db.prepare("INSERT INTO knowledge_search_grams VALUES (?,?,?,?)").bind(kind, 3, gram, position));
          const key = `${kind}\0${gram}`;
          stats.set(key, (stats.get(key) ?? 0) + 1);
        }
      });
    }
    await db.batch([
      db.prepare("CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)"),
      db.prepare("CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)"),
      db.prepare("CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)"),
      db.prepare("CREATE TABLE knowledge_search_documents (kind TEXT, position INTEGER, id TEXT, source_graph TEXT, kind_id TEXT, predicate_id TEXT, id_lower TEXT, native_id_lower TEXT, identity_values TEXT, visible_values TEXT, document_chars INTEGER, document_digest TEXT, PRIMARY KEY(kind,position))"),
      db.prepare("CREATE TABLE knowledge_search_grams (kind TEXT, n INTEGER, gram TEXT, position INTEGER, PRIMARY KEY(kind,n,gram,position))"),
      db.prepare("CREATE TABLE knowledge_search_gram_stats (kind TEXT, n INTEGER, gram TEXT, postings INTEGER, PRIMARY KEY(kind,n,gram))"),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision',0,?)").bind(JSON.stringify({sha256: indexedGraph.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top',0,?)").bind(JSON.stringify({source_revision: indexedGraph.source_revision, authority_boundary: indexedGraph.authority_boundary})),
      ...indexedGraph.nodes.map(n => db.prepare("INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)").bind(n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,n.display.title.default.toLowerCase(),searchable(n),JSON.stringify(n))),
      ...indexedGraph.relations.map(r => db.prepare("INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)").bind(r.id,r.native_id,r.source_graph,r.from_id,r.to_id,r.predicate_id,r.relation_type_id,r.display.label.default.toLowerCase(),searchable(r),JSON.stringify(r))),
      ...documentRows,
      ...gramRows,
      ...[...stats.entries()].map(([key, postings]) => {
        const [kind, gram] = key.split("\0");
        return db.prepare("INSERT INTO knowledge_search_gram_stats VALUES (?,?,?,?)").bind(kind, 3, gram, postings);
      }),
    ]);
    await applyKnowledgeExplorationMigration(db);

    const first = await knowledgeSearchD1Indexed(db, {query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1});
    assert.deepEqual((first.nodes as {id:string}[]).map(item => item.id), ["philosophy:a"]);
    assert.deepEqual((first.relations as {id:string}[]).map(item => item.id), ["philosophy:e"]);
    assert.equal((first.counts as {matching_nodes:number|null}).matching_nodes, null);
    assert.equal((first.counts as {matching_relations:number|null}).matching_relations, 1);
    assert.ok((first.page as {next_cursor:string|null}).next_cursor);

    const second = await knowledgeSearchD1Indexed(db, {
      query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1,
      cursor: (first.page as {next_cursor:string}).next_cursor,
    });
    assert.deepEqual((second.nodes as {id:string}[]).map(item => item.id), ["philosophy:alpha-extra-1"]);
    assert.deepEqual(second.relations, []);
    assert.equal((second.counts as {matching_nodes:number|null}).matching_nodes, null);
    assert.equal((second.counts as {matching_relations:number|null}).matching_relations, null);
    const third = await knowledgeSearchD1Indexed(db, {
      query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1,
      cursor: (second.page as {next_cursor:string}).next_cursor,
    });
    assert.deepEqual((third.nodes as {id:string}[]).map(item => item.id), ["philosophy:alpha-extra-2"]);
    assert.equal((third.page as {next_cursor:string|null}).next_cursor, null);

    // Compare the bounded packet semantics against the pure Python engine on
    // this same synthetic graph. Indexed page/work counters and opaque cursor
    // carriers are backend mechanics, so the comparison intentionally omits
    // them while retaining source, filters, authority, and complete packets.
    const python = JSON.parse(execFileSync('python3', ['-c', [
      "import json,sys;sys.path.insert(0,'access/src');from tos_access.knowledge import search_knowledge_graph",
      "p=json.load(sys.stdin);print(json.dumps(search_knowledge_graph(p['graph'],p['query'],sources=p['sources'],kind_ids=p['kind_ids'],predicate_ids=p['predicate_ids'],limit=p['limit'])))",
    ].join(';')], {
      cwd: fileURLToPath(new URL('../../../../', import.meta.url)),
      input: JSON.stringify({graph: indexedGraph, query: "alpha", sources: null, kind_ids: [], predicate_ids: [], limit: 100}),
      encoding: 'utf8',
    }));
    const packet = (value: Record<string, unknown>) => ({
      source_revision: value.source_revision,
      query: value.query,
      filters: value.filters,
      nodes: value.nodes,
      relations: value.relations,
      authority_boundary: value.authority_boundary,
    });
    assert.deepEqual(packet({
      source_revision: first.source_revision,
      query: first.query,
      filters: first.filters,
      nodes: [...(first.nodes as unknown[]), ...(second.nodes as unknown[]), ...(third.nodes as unknown[])],
      relations: [...(first.relations as unknown[]), ...(second.relations as unknown[]), ...(third.relations as unknown[])],
      authority_boundary: first.authority_boundary,
    }), packet(python));

    const firstCursor = (first.page as {next_cursor:string}).next_cursor;
    const decodedOuter = decodeIndexedCursor(firstCursor);
    assert.equal(decodedOuter.relations_exhausted, true);
    assert.equal(typeof decodedOuter.nodes, "string");
    const oldSchema = {...decodedOuter, schema: "tos_knowledge_search_indexed_cursor_v1"};
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1, cursor: encodeIndexedCursor(oldSchema),
      }),
      hasHttpStatus(400),
      'the pre-epoch cursor schema is not silently accepted',
    );
    const malformedOuter = {...decodedOuter};
    delete malformedOuter.nodes_exhausted;
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1, cursor: encodeIndexedCursor(malformedOuter),
      }),
      hasHttpStatus(400),
    );

    const malformedInner = decodeIndexedCursor(decodedOuter.nodes as string);
    malformedInner.rank = 4;
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1,
        cursor: encodeIndexedCursor({...decodedOuter, nodes: encodeIndexedCursor(malformedInner)}),
      }),
      hasHttpStatus(400),
    );

    // The relation kind is already exhausted in this cursor. Filter changes
    // must still invalidate the whole envelope before skipping that kind.
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: ["concept"], predicateIds: [], limit: 1, cursor: firstCursor,
      }),
      hasHttpStatus(409),
    );

    // A valid continuation can have no rows left after its key. The outer
    // cursor must close both kinds rather than restarting the node scan.
    const zeroCandidateInner = decodeIndexedCursor(decodedOuter.nodes as string);
    Object.assign(zeroCandidateInner, {rank: 3, id: "zzzzzz", position: Number.MAX_SAFE_INTEGER - 1});
    const zeroCandidate = await knowledgeSearchD1Indexed(db, {
      query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1,
      cursor: encodeIndexedCursor({...decodedOuter, nodes: encodeIndexedCursor(zeroCandidateInner)}),
    });
    assert.deepEqual(zeroCandidate.nodes, []);
    assert.deepEqual(zeroCandidate.relations, []);
    assert.equal((zeroCandidate.page as {next_cursor:string|null}).next_cursor, null);
    const zeroWork = zeroCandidate.work as {nodes:{candidate_rows:number; verified_chars:number}};
    assert.ok(zeroWork.nodes.candidate_rows > 0);
    assert.ok(zeroWork.nodes.verified_chars > 0);

    const firstWork = first.work as {
      nodes: {candidate_rows:number; verified_chars:number; selection_rows_read?:number};
      relations: {candidate_rows:number; verified_chars:number; selection_rows_read?:number};
    };
    assert.ok(firstWork.nodes.candidate_rows > (first.nodes as unknown[]).length);
    assert.ok(firstWork.nodes.verified_chars >= String(JSON.stringify((first.nodes as unknown[])[0])).length);
    for (const work of [firstWork.nodes, firstWork.relations]) {
      if (work.selection_rows_read !== undefined) assert.ok(Number.isSafeInteger(work.selection_rows_read));
    }

    // Guard the indexed path at the same transaction boundary as the legacy
    // path: an A->B->A publication during the read must fail by epoch even
    // though the data_revision digest ends at its original value.
    const originalPrepare = db.prepare.bind(db);
    let snapshotReads = 0;
    const intercepted = new Proxy(db, {get(target, property) {
      if (property === 'prepare') return (sql: string) => {
        const statement = originalPrepare(sql);
        if (!sql.includes('knowledge_exploration_clock')) return statement;
        snapshotReads += 1;
        if (snapshotReads !== 2) return statement;
        const originalFirst = statement.first.bind(statement) as <T>(columnName?: string) => Promise<T | null>;
        return new Proxy(statement, {get(inner, method, receiver) {
          if (method !== 'first') return Reflect.get(inner, method, receiver);
          return async <T>(columnName?: string) => {
            await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
              .bind(JSON.stringify({sha256: 'b'.repeat(64)})).run();
            await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'")
              .bind(JSON.stringify({sha256: indexedGraph.source_revision})).run();
            return originalFirst<T>(columnName);
          };
        }});
      };
      const value = Reflect.get(target, property);
      return typeof value === 'function' ? value.bind(target) : value;
    }});
    await assert.rejects(
      () => knowledgeSearchD1Indexed(intercepted, {query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1}),
      hasHttpStatus(409),
      'indexed A->B->A reads are rejected by the publication epoch',
    );
    assert.equal(snapshotReads, 2);
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1, cursor: firstCursor,
      }),
      hasHttpStatus(409),
      'a continuation cursor remains bound to its publication epoch',
    );

    // The budget preflight must reject from carrier metadata before rank JSON
    // fields or source search_text are evaluated. An invalid rank carrier is
    // intentional: reaching rank evaluation would be a different failure.
    await db.prepare(
      "UPDATE knowledge_search_documents SET document_chars=?, identity_values=? WHERE kind='nodes' AND position=0",
    ).bind(16_000_001, "not-json").run();
    await assert.rejects(
      () => knowledgeSearchD1Indexed(db, {
        query: "alpha", sources: null, kindIds: [], predicateIds: [], limit: 1,
      }),
      hasHttpStatus(413),
    );
  } finally {
    await mf.dispose();
  }
});

test('assessed forms preserve snapshot limits and reject malformed authority across Python and Worker', () => {
  const node = assessedFormNode();
  const cases = [node];
  for (const [key, value] of [['publication_authorized', true], ['current_runtime_grant', true], ['owner_snapshot', 'unknown'],
    ['journal_revision', null], ['journal_batches', false], ['journal_batches', 9007199254740992]]) {
    const broken = structuredClone(node);
    ((broken.attributes.human_forms as Record<string, unknown>[])[0]!.assessment_snapshot as Record<string, unknown>)[String(key)] = value;
    cases.push(broken);
  }
  for (const change of ['missing-admission', 'rejected', 'malformed-status', 'wrong-subject', 'boolean-version', 'empty-journal']) {
    const broken = structuredClone(node);
    const packet = (broken.attributes.human_forms as Record<string, unknown>[])[0]!;
    const admission = packet.admission as Record<string, unknown>;
    if (change === 'missing-admission') delete packet.admission;
    else if (change === 'empty-journal') Object.assign(packet.assessment_snapshot as object, {journal_batches: 0, journal_revision: null});
    else if (change === 'rejected' || change === 'malformed-status') admission.status = change === 'rejected' ? 'rejected' : ['admitted'];
    else (admission.subject as Record<string, unknown>)[change === 'boolean-version' ? 'version' : 'id'] = change === 'boolean-version' ? true : 'tos.form.other';
    cases.push(broken);
  }
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(n,'ru') for n in json.load(sys.stdin)]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8'}));
  const results = cases.map(item => selectHumanForms(item, 'ru'));
  assert.deepEqual(results, python);
  assert.equal(results[0]!.roles.hover!.state, 'ready');
  assert.ok(results.slice(1).every(result => result.state === 'invalid'));
});

test('assessed source-copy and separate parent context have lossless bounded Python/Worker delivery', () => {
  const node = assessedFormNode();
  const packet = (node.attributes.human_forms as Record<string, unknown>[])[0]!;
  const formAdmission = packet.admission as Record<string, unknown>;
  packet.derivation = 'source-copy';
  packet.standalone_reading = false;
  (packet.assessment_snapshot as Record<string, unknown>).subject_assessment_required = true;
  packet.subject_assessment = {schema_version: 'tos_human_form_subject_assessment_v1',
    subject: structuredClone(packet.subject), journal_revision: 'a'.repeat(64), journal_batches: 2,
    historical_withdrawals: [{id: 'tos.assessment.synthetic-withdrawal', version: 1, digest: 'sha256:' + 'b'.repeat(64)}],
    form_admission_is_parent_endorsement: false,
    admission: {schema_version: 'tos_knowledge_admission_v1', subject: structuredClone(packet.subject),
      policy: structuredClone(formAdmission.policy), use: 'research', status: 'rejected', can_use: false,
      limits: ['Synthetic rejected parent remains visible; no historical assessment.'], is_semantic_evaluation: false,
      retained_unknown_member: {counterevidence: false}}};
  const cases = [node];
  for (const derivation of ['freeform', 'source-copy']) for (const status of ['admitted', 'admitted-with-limits', 'disputed', 'rejected', 'deferred', 'unreviewed']) {
    const candidate = structuredClone(node);
    const body = (candidate.attributes.human_forms as Record<string, unknown>[])[0]!;
    body.derivation = derivation;
    Object.assign((body.subject_assessment as Record<string, unknown>).admission as object,
      {status, can_use: status.startsWith('admitted')});
    cases.push(candidate);
  }
  const validCount = cases.length;
  for (const change of ['missing-parent', 'missing-marker', 'false-marker', 'endorsement', 'standalone', 'template',
    'bad-derivation', 'wrong-subject', 'wrong-admission-subject', 'wrong-use', 'wrong-policy', 'missing-limits',
    'bad-limits', 'bad-status', 'boolean-count', 'unsafe-count', 'missing-head', 'empty-journal-with-withdrawal', 'bad-withdrawal', 'bad-can-use']) {
    const candidate = structuredClone(node);
    const body = (candidate.attributes.human_forms as Record<string, unknown>[])[0]!;
    const parent = body.subject_assessment as Record<string, unknown>, admission = parent.admission as Record<string, unknown>;
    if (change === 'missing-parent') delete body.subject_assessment;
    else if (change === 'missing-marker') delete (body.assessment_snapshot as Record<string, unknown>).subject_assessment_required;
    else if (change === 'false-marker') (body.assessment_snapshot as Record<string, unknown>).subject_assessment_required = false;
    else if (change === 'endorsement') parent.form_admission_is_parent_endorsement = true;
    else if (change === 'standalone') Object.assign(body, {standalone_reading: true, context: []});
    else if (change === 'template' || change === 'bad-derivation') body.derivation = change === 'template' ? 'template' : ['source-copy'];
    else if (change === 'wrong-subject') (parent.subject as Record<string, unknown>).id = 'tos.claim.other';
    else if (change === 'wrong-admission-subject') (admission.subject as Record<string, unknown>).id = 'tos.claim.other';
    else if (change === 'wrong-use') admission.use = 'publication';
    else if (change === 'wrong-policy') (admission.policy as Record<string, unknown>).version = 2;
    else if (change === 'missing-limits') delete admission.limits;
    else if (change === 'bad-limits') admission.limits = [false];
    else if (change === 'bad-status') admission.status = ['admitted'];
    else if (change === 'boolean-count') parent.journal_batches = true;
    else if (change === 'unsafe-count') parent.journal_batches = 9007199254740992;
    else if (change === 'missing-head') parent.journal_revision = null;
    else if (change === 'empty-journal-with-withdrawal') Object.assign(parent, {journal_batches: 0, journal_revision: null});
    else if (change === 'bad-withdrawal') (parent.historical_withdrawals as Record<string, unknown>[])[0]!.version = true;
    else admission.can_use = 0;
    cases.push(candidate);
  }
  const large = structuredClone(node);
  (((large.attributes.human_forms as Record<string, unknown>[])[0]!.subject_assessment as Record<string, unknown>).admission as Record<string, unknown>).limits = ['x'.repeat(20000)];
  cases.push(large);
  const before = structuredClone(cases);
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(n,'ru') for n in json.load(sys.stdin)]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8', maxBuffer: 4194304}));
  const results = cases.map(item => selectHumanForms(item, 'ru'));
  assert.deepEqual(results, python);
  assert.deepEqual(cases, before);
  for (let index = 0; index < validCount; index++) assert.deepEqual(results[index]!.roles.hover!.packet,
    (cases[index]!.attributes.human_forms as Record<string, unknown>[])[0]);
  assert.ok(results.slice(validCount, -1).every(result => result.state === 'invalid'));
  assert.equal(results.at(-1)!.roles.hover!.state, 'over-budget');
  assert.equal(results.at(-1)!.roles.hover!.packet, null);
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

test('form budget prioritizes requested language across roles in Python and Worker', () => {
  // Synthetic packets test allocation and preservation, not language quality.
  const subject = {id: 'tos.record.form-fixture', version: 1, digest: 'sha256:' + 'a'.repeat(64)};
  const statement = {schema_version: 'tos_human_form_materialization_v1',
    form: {id: 'tos.form.fixture-fr', version: 1, digest: 'sha256:' + 'b'.repeat(64)},
    subject, state: 'ready', role: 'statement', language: 'fr' as string | null, script: 'Latn',
    display_text: 'Cette attribution n’est pas établie.',
    context: [{slot: 'qualifiers', binding: {record: subject, pointer: '/qualifiers'},
      value: {negated: true, unknown: false, confidence: 0, condition: null, long_qualification: 'x'.repeat(11000)}}],
    issues: [], admission: null, performs_semantic_assessment: false, standalone_reading: false,
    derivation: 'source-copy', dependencies: [subject]};
  const name = structuredClone(statement);
  Object.assign(name, {role: 'name', language: null, display_text: 'Fallback name'});
  name.form.id = 'tos.form.fixture-fallback-name';
  name.context[0]!.value.long_qualification = 'y'.repeat(3500);
  const node = {content_revision: 'c'.repeat(64), attributes: {
    source_record: {record_id: subject.id, record_version: 1}, source_sha256: 'a'.repeat(64),
    human_forms_source_ref: 'test-only:allocation-fixture', human_forms: [statement, name]}};
  const cases = ['FR', 'fr-CA', 'auto', 'de'].map(language => ({item: structuredClone(node), language}));
  name.language = 'fr';
  cases.push({item: structuredClone(node), language: 'fr'});
  statement.language = 'fr-CA';
  cases.push({item: structuredClone(node), language: 'fr-CA'});
  const before = structuredClone(cases);
  const python = JSON.parse(execFileSync('python3', ['-c',
    "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(c['item'],c['language']) for c in json.load(sys.stdin)]))"],
    {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding: 'utf8'}));
  const results = cases.map(({item, language}) => selectHumanForms(item, language));
  assert.deepEqual(results, python);
  assert.deepEqual(cases, before);
  for (const [index, result] of results.entries()) {
    const statementFirst = [0, 1, 5].includes(index);
    const winner = statementFirst ? 'statement' : 'name', omitted = statementFirst ? 'name' : 'statement';
    assert.deepEqual(result.roles[winner]!.packet, cases[index]!.item.attributes.human_forms.find(p => p.role === winner));
    assert.equal(result.roles[omitted]!.state, 'over-budget');
    assert.deepEqual(result.roles[omitted]!.form, cases[index]!.item.attributes.human_forms.find(p => p.role === omitted)!.form);
    assert.equal(result.roles[omitted]!.packet, null);
    assert.ok(formDeliveryCost(result) <= HUMAN_FORM_SELECTION_BUDGET);
    assert.ok(new TextEncoder().encode(JSON.stringify(result)).length <= HUMAN_FORM_SELECTION_BUDGET);
  }
  assert.equal(results[0]!.roles.statement!.reason, 'exact-language');
  assert.equal(results[1]!.roles.statement!.reason, 'less-specific-language');
  results[0]!.roles.statement!.packet!.display_text = 'result-only mutation';
  assert.deepEqual(cases, before);
});

test('native witness forms bind unchanged identities in Python and Worker', () => {
  for (const [schema, field, prefix] of [
    ['tos_scholarly_composite_witness_v1', 'composite_id', 'tos.composite.'],
    ['tos_artifact_source_witness_v1', 'artifact_id', 'tos.artifact.'],
    ['tos_artifact_source_witness_v2', 'artifact_id', 'tos.artifact.'],
  ] as const) {
    // Synthetic envelopes test the consumer binding, not historical metadata.
    const node = realFormNode();
    const source = node.attributes.source_record as Record<string, unknown>;
    const old = source.record_id;
    const identifier = prefix + 'synthetic-form';
    const replaced = JSON.parse(JSON.stringify(node).replaceAll(String(old), identifier)) as typeof node;
    const nativeSource = replaced.attributes.source_record as Record<string, unknown>;
    delete nativeSource.record_id;
    nativeSource.schema_version = schema;
    nativeSource[field] = identifier;
    replaced.entity_id = identifier;
    const badCarrier = structuredClone(replaced);
    badCarrier.entity_id = prefix + 'other';
    const shadow = structuredClone(replaced);
    (shadow.attributes.source_record as Record<string, unknown>).record_id = identifier;
    const cases = [replaced, badCarrier, shadow, ...['unknown', '__proto__', 'constructor', [], {}].map(schemaVersion => {
      const changed = structuredClone(replaced);
      (changed.attributes.source_record as Record<string, unknown>).schema_version = schemaVersion;
      return changed;
    })];
    const python = JSON.parse(execFileSync('python3', ['-c',
      "import sys,json;sys.path.insert(0,'access/src');from tos_access.knowledge import select_human_forms;print(json.dumps([select_human_forms(n,'ru') for n in json.load(sys.stdin)]))"],
      {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), input: JSON.stringify(cases), encoding:'utf8'}));
    const results = cases.map(item => selectHumanForms(item, 'ru'));
    assert.deepEqual(results, python);
    assert.equal(results[0]!.roles.name!.state, 'ready');
    for (const result of results.slice(1)) assert.equal(result.state, 'invalid');
  }
});

test('Claim forms bind the assertion rather than its object in Python and Worker', () => {
  const node = claimFormNode();
  const conflicting = structuredClone(node);
  conflicting.attributes.source_record = {record_id: node.entity_id, record_version: 1};
  const wrongIdentity = structuredClone(node);
  wrongIdentity.entity_id = 'tos.letter.not-the-claim';
  const changed = structuredClone(node);
  const changedClaim = changed.attributes.source_claim as Record<string, unknown>;
  changedClaim.claim_version = (changedClaim.claim_version as number) + 1;
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

test('Claim navigation and exact Claim/metadata versions survive RU/EN compact/full D1 reads without new authority', async () => {
  const fixture = claimNavigationFixture(), source = structuredClone(fixture.graph);
  const fullScene = knowledgeScene(fixture.fullGraph.nodes, fixture.fullGraph.relations, null);
  assert.deepEqual(fullScene, fixture.fullScene, 'full real incident context agrees with Python');
  const fullCompact = fullScene.compact;
  assert.deepEqual(fullCompact.claim_paths, []);
  assert.deepEqual(fullCompact.retained_claims.map(claim => claim.node_id).sort(), fixture.claims);
  assert.deepEqual(fullCompact.retained_claims, fixture.claims.map(node_id => ({
    node_id, reason: 'nonfoldable-incident-relation'})));
  const mf = new Miniflare(convertV4MiniflareOptions({modules: true,
    script: 'export default {fetch(){return new Response()}}', d1Databases: ['DB']}));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare('CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision', 0, ?)").bind(JSON.stringify({sha256: source.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top', 0, ?)").bind(JSON.stringify({source_revision: source.source_revision, authority_boundary: source.authority_boundary})),
      ...source.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(
        n.id, n.entity_id, n.native_id, n.source_graph, n.kind_id, n.type_id,
        n.display.title.default.toLowerCase(), JSON.stringify(n).toLowerCase(), JSON.stringify(n))),
      ...source.relations.map(r => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)').bind(
        r.id, r.native_id, r.source_graph, r.from_id, r.to_id, r.predicate_id, r.relation_type_id,
        r.display.label.default.toLowerCase(), JSON.stringify(r).toLowerCase(), JSON.stringify(r))),
    ]);
    await applyKnowledgeExplorationMigration(db);
    for (const {spec, expected} of fixture.cases) {
      const {language, detail} = spec as {language: 'ru' | 'en'; detail: 'compact' | 'full'};
      const pure = await executeKnowledgeLens(source, spec);
      const result = await executePublishedFixtureLens(db, spec);
      assert.deepEqual(pure, expected, language + '/' + detail + ': Python/Worker parity');
      assert.deepEqual(result, pure, language + '/' + detail + ': D1 transport');
      assert.deepEqual(await executePublishedFixtureLens(db, spec), result, 'repeated reads preserve the same snapshot');
      for (const id of fixture.claims) {
        const original = source.nodes.find(n => n.id === id)!;
        const node = result.nodes.find(n => n.id === id)!;
        assert.deepEqual(node.display, original.display);
        assert.equal(node.display.provenance.title, 'navigation-template');
        assert.equal(node.display.provenance.source_title_available, false);
        assert.equal(node.display.provenance.source_summary_available, false);
        const navigation = node.display.provenance.navigation_descriptor as Record<string, unknown>;
        assert.equal(navigation.state, 'ready', 'navigation is usable, not semantic reading');
        assert.equal(navigation.purpose, 'claim-navigation-only');
        assert.equal(navigation.standalone, false);
        assert.deepEqual(navigation, original.display.provenance.navigation_descriptor);
        const selection = (node as unknown as {display_selection: {content_revision: string;
          fields: Record<string, {text: string; content_available: boolean; actual_language: string}>;
          essential_context_pointers: string[]}}).display_selection;
        assert.equal(selection.content_revision, original.content_revision);
        assert.equal(selection.fields.title!.text, original.display.title[language]);
        assert.equal(selection.fields.title!.actual_language, language);
        assert.equal(selection.fields.title!.content_available, false);
        assert.equal(selection.fields.summary!.content_available, false);
        assert.ok(selection.essential_context_pointers.length > 0);
        assert.deepEqual(node.semantics, original.semantics, 'the whole assertion context remains intact');
        assert.deepEqual(node.epistemic, original.epistemic);
        assert.equal(Object.hasOwn(node, 'human_form_selection'), false, 'no HumanForm is manufactured');
        if (detail === 'compact') {
          assert.deepEqual(node.attributes, {});
          assert.equal(Object.hasOwn(node, 'source_record'), false);
        } else {
          assert.deepEqual(node.attributes.source_claim, original.attributes.source_claim);
          assert.deepEqual(node.attributes.navigation_descriptor, original.attributes.navigation_descriptor);
          assert.deepEqual(node.source_record, original.source_record);
        }
      }
      for (const id of fixture.versions) {
        const original = source.nodes.find(n => n.id === id)!;
        const node = result.nodes.find(n => n.id === id)!;
        assert.deepEqual(node.semantics, original.semantics, 'exact pointer and whole historical context survive');
        assert.deepEqual(node.display, original.display);
        assert.deepEqual(node.epistemic, {authority_layer: 'derived-export', canon_status: null,
          review_posture: 'not-recorded', confidence: null});
        const version = node.semantics.record_version as {status: string; record_kind: string; record_ref: {id: string};
          grants_current_use: boolean; performs_assessment: boolean};
        assert.notEqual(node.entity_id, version.record_ref.id, 'version is not the current Claim identity');
        assert.equal(version.grants_current_use, false);
        assert.equal(version.performs_assessment, false);
        assert.equal(Object.hasOwn(node, 'human_form_selection'), false);
        if (version.status === 'missing') {
          assert.equal(Object.hasOwn(node.semantics, 'assertion_contexts'), false);
          assert.equal(node.display.summary_state, 'missing');
          assert.equal(node.display.provenance.source_summary_available, false);
        } else {
          assert.deepEqual(node.semantics.assertion_contexts, original.semantics.assertion_contexts);
          const declaredLanguage = original.display.provenance.summary_source_language;
          const selection = node.display_selection as {fields: {summary: {actual_language: string | null; content_available: boolean}}};
          assert.equal(selection.fields.summary.actual_language, declaredLanguage);
          assert.equal(selection.fields.summary.content_available, true);
          assert.equal(node.display.summary.original, version.record_kind === 'claim'
            ? 'Keine gesicherte Zuschreibung; synthetischer Transporttest.'
            : 'Keine gesicherte Gleichsetzung; synthetischer Metadatentest.');
          if (declaredLanguage === null) assert.equal(node.display.summary.de, undefined);
          else assert.equal(node.display.summary.de, node.display.summary.original);
          assert.equal(node.display.provenance.summary, 'exact-record-quotation');
        }
        assert.deepEqual(node.attributes, detail === 'full' ? original.attributes : {});
        if (detail === 'full') assert.deepEqual(node.source_record, original.source_record);
        else assert.equal(Object.hasOwn(node, 'source_record'), false);
      }
      const compact = (result.scene as {compact: {claim_paths: {claim_node_id: string; reading: {
        wording_pointer: string | null; wording_state: string; standalone: boolean}}[]}}).compact;
      assert.deepEqual(compact.claim_paths.map(path => path.claim_node_id).sort(), fixture.claims);
      for (const path of compact.claim_paths) {
        assert.equal(path.reading.wording_state, 'missing');
        assert.equal(path.reading.wording_pointer, null);
        assert.equal(path.reading.standalone, false);
      }
    }
    assert.deepEqual(fixture.graph, source, 'transport does not rewrite source carriers');
  } finally {
    await mf.dispose();
  }
});

test('V2 semantic identity proposals retain the whole frozen plan through Python Worker and D1', async () => {
  // Use the real owner transaction and portable normalizer in a disposable
  // synthetic source fixture. No canonical subjects or deployed DB are touched.
  const fixture: {graph: KnowledgeGraph; claimId: string; plan: unknown; members: string[];
    cases: {spec: unknown; expected: unknown}[]} = JSON.parse(execFileSync('python3', ['-c', `
import json, sys
sys.path[:0] = ['mechanics/growth-cycle/tests', 'access/src']
from test_identity_proposals import SemanticIdentityProposalCommandTests
from tos_access.knowledge import execute_knowledge_lens
test = SemanticIdentityProposalCommandTests()
with test.identity_creation() as (root, owner, config, claim, request, rebuild, helper, paths):
    test.create(owner, request)
    graph, _, _ = helper.historical_knowledge(root, rebuild())
    specs = [{'schema_version': 'tos_lens_spec_v1', 'lens_id': 'semantic-proposal-transport',
        'sources': ['source-claims'], 'language': language, 'detail': detail}
        for language in ('ru', 'en') for detail in ('compact', 'full')]
    result = {'graph': graph, 'claimId': 'source-claims:claim:' + claim['claim_id'],
        'plan': claim['object'], 'members': ['source-claims:identity:' + key for key in claim['object']['members']],
        'cases': [{'spec': spec, 'expected': execute_knowledge_lens(graph, spec)} for spec in specs]}
print(json.dumps(result))
`], {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding: 'utf8', maxBuffer: 8 * 1024 * 1024}));
  const source = fixture.graph, original = structuredClone(source);
  const mf = new Miniflare(convertV4MiniflareOptions({modules: true,
    script: 'export default {fetch(){return new Response()}}', d1Databases: ['DB']}));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare('CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision', 0, ?)").bind(JSON.stringify({sha256: source.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top', 0, ?)").bind(JSON.stringify({source_revision: source.source_revision, authority_boundary: source.authority_boundary})),
      ...source.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(
        n.id, n.entity_id, n.native_id, n.source_graph, n.kind_id, n.type_id,
        n.display.title.default.toLowerCase(), JSON.stringify(n).toLowerCase(), JSON.stringify(n))),
      ...source.relations.map(r => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)').bind(
        r.id, r.native_id, r.source_graph, r.from_id, r.to_id, r.predicate_id, r.relation_type_id,
        r.display.label.default.toLowerCase(), JSON.stringify(r).toLowerCase(), JSON.stringify(r))),
    ]);
    await applyKnowledgeExplorationMigration(db);
    for (const {spec, expected} of fixture.cases) {
      const pure = await executeKnowledgeLens(source, spec), result = await executePublishedFixtureLens(db, spec);
      assert.deepEqual(pure, expected, 'Python/Worker parity');
      assert.deepEqual(result, pure, 'D1 preserves the normalized proposal and members');
      const proposal = result.nodes.find(n => n.id === fixture.claimId)!;
      assert.ok(proposal);
      const contexts = proposal.semantics.assertion_contexts as {fields: Record<string, {value: unknown}>}[];
      assert.deepEqual(contexts[0]!.fields.object!.value, fixture.plan);
      assert.deepEqual(proposal.semantics, source.nodes.find(n => n.id === fixture.claimId)!.semantics);
      assert.equal(proposal.epistemic.review_posture, 'unreviewed');
      assert.deepEqual(result.relations.filter(r => r.relation_type_id === 'tos.relation.claim-value-member')
        .map(r => r.to_id).sort(), fixture.members.toSorted());
      for (const id of fixture.members) assert.ok(result.nodes.some(n => n.id === id), 'each old subject stays independent');
    }
    assert.deepEqual(source, original, 'transport performs no subject transition');
  } finally {
    await mf.dispose();
  }
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
