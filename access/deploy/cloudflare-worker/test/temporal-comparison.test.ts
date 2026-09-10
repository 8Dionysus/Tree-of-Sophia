import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';
import { HttpError, type Item } from '../src/common.ts';
import { KnowledgeRevisionConflict } from '../src/lens-pagination.ts';
import { compareTemporalOperands, normalizeTemporalComparisonRequest, temporalNodeFromJson, type TemporalRequest } from '../src/temporal-comparison.ts';
import { knowledgeTemporalCompareD1 } from '../src/knowledge-store.ts';
import { lensCarrier, type KnowledgeNode } from '../src/knowledge.ts';

type Fixture = { name: string; graph: { source_revision: string; nodes: Item[] };
  raw_nodes: string[];
  delivery?: {node: KnowledgeNode; full: KnowledgeNode; compact: KnowledgeNode};
  request: TemporalRequest; expected?: Item; error_status?: number; error?: string };

let cachedFixtures: Fixture[] | undefined;
function fixtures(): Fixture[] {
  // Reuse the actual Python normalizer and its source-binding controls.
  // These are disposable arithmetic fixtures plus the unchanged Basel pair,
  // not admitted historical Claims or a full corpus readiness check.
  return cachedFixtures ??= JSON.parse(execFileSync('python3', ['-c', [
    "import sys,json;sys.path[:0]=['access/tests','access/src']",
    'from test_temporal_comparison import TemporalComparisonTests',
    'TemporalComparisonTests.setUpClass()',
    'print(json.dumps(TemporalComparisonTests().transport_cases()))',
  ].join(';')], { cwd: fileURLToPath(new URL('../../../../', import.meta.url)),
    encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }));
}

function rowKeyVariants(fixture: Fixture): {name: string; nodes: string[]; ambiguous: boolean}[] {
  const claim = fixture.graph.nodes.find(node => node.id === fixture.request.left.node_id)!;
  const valueId = ((claim.semantics as Item).claim as Item).object_node_id;
  return [
    {name: 'escaped-keys-and-whitespace', ambiguous: false, nodes: fixture.raw_nodes.map(raw =>
      ' \n' + raw.replace(/"(attributes|source_claim|semantics|time|raw|value)"(?=\s*:)/g,
        (_, key: string) => '"\\u' + key.charCodeAt(0).toString(16).padStart(4, '0') + key.slice(1) + '"') + '\n ')},
    {name: 'duplicate-escaped-selected-key', ambiguous: true, nodes: fixture.raw_nodes.map(raw => {
      const node = JSON.parse(raw) as Item;
      // The second decoded attributes key wins in JSON.parse. Its JSON
      // round-trip loses 1.0/unsafe-integer number spelling, while the first
      // literal key still carries the original source bytes. Never mix them.
      return node.id === valueId ? raw.slice(0, -1) + ',"\\u0061ttributes":' + JSON.stringify(node.attributes) + '}' : raw;
    })},
  ];
}

function assertRowKeyResult(result: Item, fixture: Fixture, ambiguous: boolean): void {
  if (!ambiguous) assert.deepEqual(result, fixture.expected);
  else {
    const comparison = result.comparison as Item;
    assert.equal(comparison.status, 'undetermined');
    assert.equal(comparison.relation, null);
    assert.ok((comparison.reasons as Item[]).some(reason => reason.side === 'left'
      && reason.code === 'document-catalogue-exact-source-binding-inconsistent'));
  }
}

test('exact temporal envelopes and all retained context match Python', async t => {
  for (const fixture of fixtures()) {
    await t.test(fixture.name, async () => {
      const original = structuredClone(fixture.graph);
      const nodes = fixture.raw_nodes.map(temporalNodeFromJson);
      const reads: string[] = [];
      const read = () => compareTemporalOperands(fixture.graph.source_revision, fixture.request, async id => {
        reads.push(id);
        return nodes.filter(node => node.id === id);
      });
      if (fixture.error_status) await assert.rejects(read(), (error: unknown) =>
        error instanceof HttpError && error.status === fixture.error_status && error.message === fixture.error);
      else assert.deepEqual(await read(), fixture.expected);
      assert.deepEqual(fixture.graph, original);
      assert.ok(reads.length <= (fixture.name.startsWith('document-') ? 6 : 4));
    });
  }
  const fixture = fixtures()[0]!;
  const lookup = async (id: string) => fixture.graph.nodes.filter(node => node.id === id);
  await assert.rejects(compareTemporalOperands('0'.repeat(64), fixture.request, lookup), KnowledgeRevisionConflict);
  await assert.rejects(compareTemporalOperands(fixture.graph.source_revision,
    { ...fixture.request, left: { ...fixture.request.left, content_revision: '0'.repeat(64) } }, lookup), KnowledgeRevisionConflict);
  await assert.rejects(compareTemporalOperands(fixture.graph.source_revision,
    { ...fixture.request, left: { ...fixture.request.left, node_id: 'not-an-alias' } }, lookup),
  (error: unknown) => error instanceof HttpError && error.status === 404);
  for (const invalid of [null, [], {}, { ...fixture.request, calendar: 'gregorian' },
    { ...fixture.request, left: { ...fixture.request.left, node_id: ' x' } },
    { ...fixture.request, left: { ...fixture.request.left, node_id: '😀'.repeat(1025) } }]) {
    assert.throws(() => normalizeTemporalComparisonRequest(invalid), HttpError);
  }
  assert.equal(normalizeTemporalComparisonRequest({ ...fixture.request,
    left: { ...fixture.request.left, node_id: '😀'.repeat(1024) } }).left.node_id.length, 2048);
});

test('native document compact delivery and exact row key identity remain separate', async () => {
  const fixture = fixtures().find(value => value.name === 'document-native-numbers')!;
  const delivery = fixture.delivery!, original = structuredClone(delivery.node);
  assert.deepEqual(lensCarrier(delivery.node, 'full', 'en'), delivery.full);
  assert.deepEqual(lensCarrier(delivery.node, 'compact', 'en'), delivery.compact);
  assert.equal(Object.hasOwn(delivery.compact.semantics.claim as Item, 'source_canonical_json'), false);
  assert.deepEqual(delivery.node, original);
  for (const variant of rowKeyVariants(fixture)) {
    const nodes = variant.nodes.map(temporalNodeFromJson);
    assertRowKeyResult(await compareTemporalOperands(fixture.graph.source_revision, fixture.request,
      async id => nodes.filter(node => node.id === id)), fixture, variant.ambiguous);
  }
});

test('Worker HTTP and indexed D1 agree; stale or malformed selections fail closed', async t => {
  const bundle = await build({ entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
    bundle: true, write: false, format: 'esm', platform: 'browser', target: 'es2022' });
  const mf = new Miniflare(convertV4MiniflareOptions({ modules: true,
    script: bundle.outputFiles[0]!.text, d1Databases: ['DB'] }));
  try {
    const db = await mf.getD1Database('DB');
    await db.batch([
      db.prepare('CREATE TABLE edge_meta (key TEXT, part INTEGER, json_chunk TEXT)'),
      // Permit duplicate rows only for the explicit corrupt-projection case;
      // the production store has unique identities and its index is stronger.
      db.prepare('CREATE TABLE knowledge_nodes (id TEXT, json TEXT)'),
      db.prepare('CREATE INDEX temporal_exact_node ON knowledge_nodes(id)'),
    ]);
    const all = fixtures();
    for (const fixture of all) {
      await t.test(fixture.name, async () => {
        await db.batch([
          db.prepare('DELETE FROM edge_meta'), db.prepare('DELETE FROM knowledge_nodes'),
          db.prepare("INSERT INTO edge_meta VALUES ('data_revision', 0, ?)").bind(JSON.stringify({ sha256: fixture.graph.source_revision })),
          db.prepare("INSERT INTO edge_meta VALUES ('knowledge_top', 0, ?)").bind(JSON.stringify({ source_revision: fixture.graph.source_revision })),
          ...fixture.raw_nodes.map(raw => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?)').bind(JSON.parse(raw).id, raw)),
        ]);
        const reads: string[] = [];
        const observed = new Proxy(db, { get(target, key) {
          if (key === 'prepare') return (sql: string) => { reads.push(sql); return target.prepare(sql); };
          const value = Reflect.get(target, key, target);
          return typeof value === 'function' ? value.bind(target) : value;
        } });
        if (fixture.error_status) await assert.rejects(knowledgeTemporalCompareD1(observed, fixture.request), (error: unknown) =>
          error instanceof HttpError && error.status === fixture.error_status && error.message === fixture.error);
        else assert.deepEqual(await knowledgeTemporalCompareD1(observed, fixture.request), fixture.expected);
        assert.ok(reads.length <= (fixture.name.startsWith('document-') ? 9 : 7));
        assert.ok(reads.filter(sql => sql.includes('knowledge_nodes')).every(sql => sql === 'SELECT json FROM knowledge_nodes WHERE id = ? LIMIT 2'));
        assert.ok(reads.every(sql => !sql.includes('knowledge_relations')));
        const response = await mf.dispatchFetch('http://localhost/api/knowledge/temporal/compare', {
          method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(fixture.request),
        });
        assert.equal(response.status, fixture.error_status ?? 200);
        const packet = await response.json() as Item;
        if (fixture.error_status) {
          assert.equal(packet.error, fixture.error);
          assert.equal(packet.comparison, undefined);
        } else {
          // The public JSON framing has IEEE-754 numbers (including -0 -> 0).
          // Exact source number spelling remains in the unchanged canonical
          // string; compare that as part of the full packet, never rehash it.
          assert.deepEqual(packet, JSON.parse(JSON.stringify(fixture.expected)));
        }
        if (fixture.name === 'document-native-numbers') for (const variant of rowKeyVariants(fixture)) {
          await db.batch([db.prepare('DELETE FROM knowledge_nodes'), ...variant.nodes.map(raw =>
            db.prepare('INSERT INTO knowledge_nodes VALUES (?,?)').bind(JSON.parse(raw).id, raw))]);
          assertRowKeyResult(await knowledgeTemporalCompareD1(db, fixture.request), fixture, variant.ambiguous);
          const response = await mf.dispatchFetch('http://localhost/api/knowledge/temporal/compare', {
            method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(fixture.request),
          });
          assert.equal(response.status, 200, variant.name);
          assertRowKeyResult(await response.json() as Item,
            {...fixture, expected: JSON.parse(JSON.stringify(fixture.expected))}, variant.ambiguous);
        }
      });
    }
    const fixture = all.at(-1)!;
    for (const [request, status] of [
      [{ ...fixture.request, source_revision: '0'.repeat(64) }, 409],
      [{ ...fixture.request, left: { ...fixture.request.left, content_revision: '0'.repeat(64) } }, 409],
      [{ ...fixture.request, left: { ...fixture.request.left, node_id: 'not-an-alias' } }, 404],
      [{ ...fixture.request, calendar: 'gregorian' }, 400], [[], 400],
    ] as const) {
      const response = await mf.dispatchFetch('http://localhost/api/knowledge/temporal/compare', {
        method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request),
      });
      assert.equal(response.status, status);
      assert.equal(typeof (await response.json() as Item).error, 'string');
    }
    for (const [body, contentType, status] of [['{}', 'text/plain', 415], ['{', 'application/json', 400],
      [' '.repeat(65537), 'application/json', 413]] as const) {
      const response = await mf.dispatchFetch('http://localhost/api/knowledge/temporal/compare', {
        method: 'POST', headers: { 'Content-Type': contentType }, body,
      });
      assert.equal(response.status, status);
      await response.arrayBuffer();
    }
    // Inject a legitimate publication boundary during the exact-node read.
    // Only this disposable database is changed; a mixed snapshot is not returned.
    let changed = false;
    const changing = new Proxy(db, { get(target, key) {
      if (key === 'prepare') return (sql: string) => {
        const statement = target.prepare(sql);
        if (!sql.includes('knowledge_nodes')) return statement;
        return new Proxy(statement, { get(prepared, operation) {
          if (operation === 'bind') return (...values: unknown[]) => {
            const bound = prepared.bind(...values);
            return new Proxy(bound, { get(boundStatement, method) {
              if (method === 'all') return async () => {
                const result = await boundStatement.all();
                if (!changed) {
                  changed = true;
                  await db.prepare("UPDATE edge_meta SET json_chunk = ? WHERE key = 'data_revision'")
                    .bind(JSON.stringify({ sha256: 'f'.repeat(64) })).run();
                }
                return result;
              };
              const value = Reflect.get(boundStatement, method, boundStatement);
              return typeof value === 'function' ? value.bind(boundStatement) : value;
            } });
          };
          const value = Reflect.get(prepared, operation, prepared);
          return typeof value === 'function' ? value.bind(prepared) : value;
        } });
      };
      const value = Reflect.get(target, key, target);
      return typeof value === 'function' ? value.bind(target) : value;
    } });
    await assert.rejects(knowledgeTemporalCompareD1(changing, fixture.request),
      (error: unknown) => error instanceof HttpError && error.status === 409);
    assert.equal(changed, true);
  } finally {
    await mf.dispose();
  }
});
