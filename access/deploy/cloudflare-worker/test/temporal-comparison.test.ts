import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { HttpError, type Item } from '../src/common.ts';
import { KnowledgeRevisionConflict } from '../src/lens-pagination.ts';
import { compareTemporalOperands, normalizeTemporalComparisonRequest, temporalNodeFromJson, type TemporalRequest } from '../src/temporal-comparison.ts';
import { lensCarrier, type KnowledgeNode } from '../src/knowledge.ts';

import {nativePacketJson, type NativePacket} from '../src/native-lens.ts';
const decoded = (packet: NativePacket): Item => JSON.parse(nativePacketJson(packet));

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


test('exact temporal envelopes and all retained context match Python', async t => {
  for (const fixture of fixtures()) {
    await t.test(fixture.name, async () => {
      const original = structuredClone(fixture.graph);
      const nodes = fixture.raw_nodes.map(temporalNodeFromJson);
      const reads: string[] = [];
      const read = () => compareTemporalOperands(fixture.graph.source_revision, fixture.request, async id => {
        reads.push(id);
        return nodes.filter(node => (node.value as Item).id === id);
      });
      if (fixture.error_status) await assert.rejects(read(), (error: unknown) =>
        error instanceof HttpError && error.status === fixture.error_status && error.message === fixture.error);
      else assert.deepEqual(decoded(await read()), fixture.expected);
      assert.deepEqual(fixture.graph, original);
      assert.ok(reads.length <= (fixture.name.startsWith('document-') ? 6 : 4));
    });
  }
  const fixture = fixtures()[0]!;
  const lookup = async (id: string) => fixture.raw_nodes.map(temporalNodeFromJson).filter(node => (node.value as Item).id === id);
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
    if (variant.ambiguous) {
      assert.throws(() => variant.nodes.map(temporalNodeFromJson), (error: unknown) => error instanceof HttpError && error.status === 503);
      continue;
    }
    const nodes = variant.nodes.map(temporalNodeFromJson);
    assert.deepEqual(decoded(await compareTemporalOperands(fixture.graph.source_revision, fixture.request,
      async id => nodes.filter(node => (node.value as Item).id === id))), fixture.expected);
  }
});
