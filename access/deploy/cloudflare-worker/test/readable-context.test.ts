import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {Miniflare, convertV4MiniflareOptions} from 'miniflare';
import {executeKnowledgeLens, type KnowledgeGraph} from '../src/knowledge.ts';
import {knowledgeNodeD1} from '../src/knowledge-store.ts';
import {nativePacketJson} from '../src/native-lens.ts';
import {executePublishedFixtureLens} from './native-lens-fixture.ts';

const knowledgeExplorationMigration = readFileSync(
  new URL('../migrations/0001-exploration.sql', import.meta.url),
  'utf8',
).replace(/^--.*$/gm, '').trim();

test('native canonical contexts survive Python, Worker and addressed D1 delivery without entering compact packets', async () => {
  const fixture = JSON.parse(execFileSync('python3', ['-c', `
import json,sys
sys.path.insert(0,'access/tests')
from test_readable_context import real_canonical_graph
from tos_access.knowledge import execute_knowledge_lens
graph,catalog=real_canonical_graph()
cases=[]
for language in ('ru','en'):
    for detail in ('compact','full'):
        spec={'schema_version':'tos_lens_spec_v1','lens_id':'native-canonical-context',
              'sources':['canon'],'language':language,'detail':detail}
        cases.append({'spec':spec,'expected':execute_knowledge_lens(graph,spec)})
print(json.dumps({'graph':graph,'cases':cases}))
`], {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding: 'utf8'}));
  const graph = fixture.graph as KnowledgeGraph, before = structuredClone(graph);
  const canonical = graph.nodes.filter(n => n.attributes.schema_version === 'tos_canonical_node_v1');
  assert.equal(canonical.length, 2, 'real support and departure source nodes');
  for (const row of canonical) {
    assert.equal(row.readable_context!.state, 'requires-exact-context');
    assert.equal(row.readable_context!.reason, 'context-presentation-budget');
    assert.deepEqual(row.readable_context!.exact_context_pointers,
      ['/attributes/human_forms', '/attributes/source_record']);
    assert.deepEqual(row.readable_context!.contexts, []);
  }
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,
    script:'export default {fetch(){return new Response()}}', d1Databases:['DB']}));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare('CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision',0,?)").bind(JSON.stringify({sha256:graph.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top',0,?)").bind(JSON.stringify({source_revision:graph.source_revision,authority_boundary:graph.authority_boundary})),
      ...graph.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(
        n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,
        n.display.title.default.toLowerCase(),JSON.stringify(n).toLowerCase(),JSON.stringify(n))),
      ...graph.relations.map(r => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)').bind(
        r.id,r.native_id,r.source_graph,r.from_id,r.to_id,r.predicate_id,r.relation_type_id,
        r.display.label.default.toLowerCase(),JSON.stringify(r).toLowerCase(),JSON.stringify(r))),
    ]);
    await applyKnowledgeExplorationMigration(db);
    for (const {spec,expected} of fixture.cases) {
      assert.deepEqual(await executeKnowledgeLens(graph,spec), expected);
      const delivered = await executePublishedFixtureLens(db,spec);
      assert.deepEqual(delivered,expected);
      for (const row of delivered.nodes as KnowledgeGraph['nodes']) {
        const source = canonical.find(n => n.id === row.id);
        if (!source) continue; // Preserve real unresolved relation endpoints too.
        if (spec.detail === 'full') {
          assert.deepEqual(row.readable_context,source.readable_context);
          assert.deepEqual(row.attributes.source_record,source.attributes.source_record);
          assert.deepEqual(row.attributes.human_forms,source.attributes.human_forms);
          assert.equal(row.readable_context!.performs_semantic_assessment,false);
        } else {
          assert.equal(Object.hasOwn(row,'readable_context'),false);
          assert.equal(Object.hasOwn(row,'source_record'),false);
          assert.deepEqual(row.attributes,{});
        }
      }
    }
    for (const row of canonical) {
      const inspected = JSON.parse(nativePacketJson(await knowledgeNodeD1(db,row.id,0)));
      assert.deepEqual(inspected.matches[0].readable_context,row.readable_context);
    }
    assert.deepEqual(graph,before,'delivery preserves source rows, versions and form context');
  } finally {
    await mf.dispose();
  }
});

async function applyKnowledgeExplorationMigration(db: D1Database): Promise<void> {
  await db.batch(knowledgeExplorationMigration.split(/\n(?=CREATE |INSERT )/).map((statement) => db.prepare(statement)));
}

test('readable context preserves exact full packets and is absent from compact Python/Worker/D1 delivery', async () => {
  const fixture = JSON.parse(execFileSync('python3', ['-c', `
import json,sys
sys.path.insert(0,'access/tests')
from test_readable_context import real_freedom_graph
from tos_access.knowledge import execute_knowledge_lens
g,catalog=real_freedom_graph()
numeric,_=real_freedom_graph(numeric_control=True)
cases=[]
for language in ('ru','en'):
    for detail in ('compact','full'):
        spec={'schema_version':'tos_lens_spec_v1','lens_id':'readable-context-parity',
              'sources':['source-navigation'],'language':language,'detail':detail}
        cases.append({'spec':spec,'expected':execute_knowledge_lens(g,spec)})
spec={'schema_version':'tos_lens_spec_v1','lens_id':'numeric-context-control','sources':['source-navigation'],'detail':'full'}
print(json.dumps({'graph':g,'catalog':{'context_presentation':catalog['context_presentation']},'cases':cases,
                 'numeric':{'graph':numeric,'spec':spec,'expected':execute_knowledge_lens(numeric,spec)}}))
`], {cwd: fileURLToPath(new URL('../../../../', import.meta.url)), encoding: 'utf8'})) as {
    graph: KnowledgeGraph; catalog: Record<string, unknown>;
    cases: {spec: {detail: 'compact' | 'full'; language: string}; expected: Record<string, unknown>}[];
    numeric: {graph: KnowledgeGraph; spec: Record<string, unknown>; expected: Record<string, unknown>};
  };
  const graph = fixture.graph, original = structuredClone(graph);
  const mf = new Miniflare(convertV4MiniflareOptions({modules: true,
    script: 'export default {fetch(){return new Response()}}', d1Databases: ['DB']}));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT PRIMARY KEY, entity_id TEXT, native_id TEXT, source_graph TEXT, kind_id TEXT, type_id TEXT, title_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare('CREATE TABLE knowledge_relations (id TEXT PRIMARY KEY, native_id TEXT, source_graph TEXT, from_id TEXT, to_id TEXT, predicate_id TEXT, relation_type_id TEXT, label_text TEXT, search_text TEXT, json TEXT)'),
      db.prepare("INSERT INTO edge_meta VALUES ('data_revision',0,?)").bind(JSON.stringify({sha256: graph.source_revision})),
      db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top',0,?)").bind(JSON.stringify({source_revision: graph.source_revision, authority_boundary: graph.authority_boundary})),
      ...graph.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(
        n.id, n.entity_id, n.native_id, n.source_graph, n.kind_id, n.type_id,
        n.display.title.default.toLowerCase(), JSON.stringify(n).toLowerCase(), JSON.stringify(n))),
    ]);
    await applyKnowledgeExplorationMigration(db);
    for (const {spec, expected} of fixture.cases) {
      const pure = await executeKnowledgeLens(graph, spec);
      const stored = await executePublishedFixtureLens(db, spec);
      assert.deepEqual(pure, expected, `${spec.language}/${spec.detail}: Python/Worker`);
      assert.deepEqual(stored, expected, `${spec.language}/${spec.detail}: local D1`);
      for (const node of stored.nodes as KnowledgeGraph['nodes']) {
        const source = graph.nodes.find(n => n.id === node.id)!;
        if (spec.detail === 'full') {
          assert.equal(node.readable_context!.state, 'complete');
          assert.deepEqual(node.readable_context, source.readable_context);
          assert.deepEqual(node.attributes.human_forms, source.attributes.human_forms);
          const vocabulary = fixture.catalog.context_presentation as Record<string, unknown>;
          assert.deepEqual(node.readable_context!.vocabulary, Object.fromEntries(
            ['id','version','source_ref','digest'].map(key => [key,vocabulary[key]])));
        } else {
          assert.equal(Object.hasOwn(node, 'readable_context'), false);
          assert.equal(Object.hasOwn(node, 'source_record'), false);
          assert.deepEqual(node.attributes, {});
        }
      }
    }
    const source = graph.nodes.find(n => n.source_graph === 'source-navigation')!;
    const inspected = JSON.parse(nativePacketJson(await knowledgeNodeD1(db, source.id, 10)));
    assert.deepEqual((inspected.matches as Record<string, unknown>[])[0]!.readable_context, source.readable_context);
    assert.deepEqual(graph, original, 'delivery leaves source material unchanged');
    // Synthetic numeric extension of this source-copy fixture: no source write.
    // Ordinary JSON fields lose precision, while the existing canonical text
    // retains the exact values and spelling used by the source-record digest.
    const numeric = fixture.numeric;
    await db.batch([
      db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256: numeric.graph.source_revision})),
      db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_top'").bind(JSON.stringify({source_revision: numeric.graph.source_revision, authority_boundary: numeric.graph.authority_boundary})),
      ...numeric.graph.nodes.map(n => db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(n), n.id)),
    ]);
    const delivered = await executePublishedFixtureLens(db, numeric.spec);
    // A JSON HTTP response normalizes -0 to 0. That ordinary carrier is not
    // the numeric-fidelity evidence; the canonical string below is unchanged.
    assert.deepEqual(JSON.parse(JSON.stringify(delivered)), JSON.parse(JSON.stringify(numeric.expected)));
    assert.deepEqual(await executeKnowledgeLens(numeric.graph, numeric.spec), numeric.expected);
    const numericNode = (delivered.nodes as KnowledgeGraph['nodes'])[0]!;
    const materials = numericNode.readable_context!.exact_materials as {digest: string; canonical_json: string; origin_pointers: string[]}[];
    assert.equal(materials.length, 1);
    assert.ok(materials[0]!.canonical_json.includes('[1,1.0,9007199254740993,-0.0,1e-07,1e+21,false]'));
    assert.equal(materials[0]!.digest, 'sha256:' + createHash('sha256').update(materials[0]!.canonical_json, 'utf8').digest('hex'));
    assert.deepEqual(materials[0]!.origin_pointers, ['/attributes/source_record']);
  } finally {
    await mf.dispose();
  }
});
