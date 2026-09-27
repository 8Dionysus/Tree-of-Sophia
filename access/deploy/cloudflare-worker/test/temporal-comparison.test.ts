import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import test from 'node:test';
import {fileURLToPath} from 'node:url';
import {lensCarrier, type KnowledgeNode} from '../src/knowledge.ts';
import {type Item} from '../src/common.ts';

// Presentation remains a host responsibility. Temporal computation and exact
// row-key/Unicode/request controls live on the maintained Rust HTTP route in
// native-temporal.test.mjs; no predecessor TS temporal executor remains.
test('native document compact delivery retains the existing presentation contract', () => {
  const delivery = JSON.parse(execFileSync('python3', ['-B', '-c', [
    "import sys,json;sys.path[:0]=['access/tests','access/src']",
    'from test_temporal_comparison import TemporalComparisonTests',
    'TemporalComparisonTests.setUpClass()',
    "print(json.dumps(next(case['delivery'] for case in TemporalComparisonTests().transport_cases() if case['name']=='document-native-numbers')))",
  ].join(';')], {cwd:fileURLToPath(new URL('../../../../', import.meta.url)),
    encoding:'utf8',maxBuffer:32*1024*1024})) as {node:KnowledgeNode;full:KnowledgeNode;compact:KnowledgeNode};
  const original = structuredClone(delivery.node);
  assert.deepEqual(lensCarrier(delivery.node, 'full', 'en'), delivery.full);
  assert.deepEqual(lensCarrier(delivery.node, 'compact', 'en'), delivery.compact);
  assert.equal(Object.hasOwn(delivery.compact.semantics.claim as Item, 'source_canonical_json'), false);
  assert.deepEqual(delivery.node, original);
});
