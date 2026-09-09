import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFileSync, mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {build} from 'esbuild';
import {Miniflare, convertV4MiniflareOptions} from 'miniflare';
import {exploreD1, explorationCapabilitiesD1, normalizeExploration} from '../src/exploration.ts';
import {bindOriginD1, REQUEST_V2, RESULT_V2, type Origin, type ResolvedOrigin} from '../src/exploration-origin.ts';
import {HttpError, type Item} from '../src/common.ts';
import {knowledgeScene} from '../src/knowledge.ts';

type Graph = {source_revision: string; nodes: Item[]; relations: Item[]; authority_boundary: Item};
type Query = {schema_version: string; source_revision: string; origin: Origin} & Item;
type Packet = Item & {origin: ResolvedOrigin; nodes: Item[]; relations: Item[];
  query: {page_nodes: number; page_relations: number}; scene: ReturnType<typeof knowledgeScene>;
  page: {primary_node_ids: string[]; context_node_ids: string[]; primary_relation_ids: string[];
    context_relation_ids: string[]; next_cursor: string | null; work_units?: number};
  counts: {discovered_nodes: number; emitted_relations: number}};
const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const migration = readFileSync(new URL('../migrations/0001-exploration.sql', import.meta.url), 'utf8').replace(/^--.*$/gm, '').trim();
const python = (code: string, input: unknown) => JSON.parse(execFileSync('python3', ['-c',
  "import sys,json;sys.path[:0]=['access/src','access/tests'];" + code],
  {cwd: repo, input: JSON.stringify(input), encoding: 'utf8', maxBuffer: 32 * 1024 * 1024}));
function graph(seed = 0, identities = false): Graph {
  return python("from test_exploration_origin import origin_graph;p=json.load(sys.stdin);print(json.dumps(origin_graph(seed=p['seed'],identities=p['identities'])))", {seed, identities});
}
function query(g: Graph, kind: Origin['kind'] = 'relation', options: Item = {}): Query {
  const item = g[kind === 'node' ? 'nodes' : 'relations'][0]!;
  return {schema_version: REQUEST_V2, source_revision: g.source_revision,
    origin: {kind, id: String(item.id), content_revision: String(item.content_revision)}, ...options};
}
async function init(db: D1Database, g: Graph, duplicateTables = false) {
  await db.batch([
    db.prepare('CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part))'),
    db.prepare(`CREATE TABLE knowledge_nodes(id TEXT ${duplicateTables ? '' : 'PRIMARY KEY'},entity_id TEXT,native_id TEXT,source_graph TEXT,json TEXT)`),
    db.prepare(`CREATE TABLE knowledge_relations(id TEXT ${duplicateTables ? '' : 'PRIMARY KEY'},from_id TEXT,to_id TEXT,source_graph TEXT,predicate_id TEXT,json TEXT)`),
    db.prepare("INSERT INTO edge_meta VALUES ('data_revision',0,?)").bind(JSON.stringify({sha256: g.source_revision})),
    db.prepare("INSERT INTO edge_meta VALUES ('knowledge_exploration_top',0,?)")
      .bind(JSON.stringify({source_revision: g.source_revision, authority_boundary: g.authority_boundary})),
    ...g.nodes.map(n => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?)')
      .bind(n.id, n.entity_id, n.native_id, n.source_graph, JSON.stringify(n))),
    ...g.relations.map(r => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?)')
      .bind(r.id, r.from_id, r.to_id, r.source_graph, r.predicate_id, JSON.stringify(r))),
  ]);
  await db.batch(migration.split(/\n(?=CREATE |INSERT )/).map(s => db.prepare(s)));
}
function assertClosure(packet: Packet) {
  const {origin, page, query: q} = packet;
  assert.equal(packet.schema, RESULT_V2);
  assert.equal(Object.hasOwn(packet, 'focus'), false);
  const roots = origin.kind === 'node' ? [origin.id] : [...new Set(Object.values(origin.endpoints!).map(n => n.node_id))];
  const nodes = packet.nodes.map(n => String(n.id)), relations = packet.relations.map(r => String(r.id));
  assert.equal(nodes.length, new Set(nodes).size);
  assert.equal(relations.length, new Set(relations).size);
  assert.deepEqual([...page.primary_node_ids, ...page.context_node_ids].sort(), [...nodes].sort());
  assert.deepEqual([...page.primary_relation_ids, ...page.context_relation_ids].sort(), [...relations].sort());
  assert.ok(roots.every(id => page.context_node_ids.includes(id)));
  assert.ok(page.primary_node_ids.length <= q.page_nodes);
  assert.ok(page.primary_relation_ids.length <= q.page_relations);
  assert.ok(nodes.length <= q.page_nodes + 2 * q.page_relations + 2);
  assert.ok(relations.length <= q.page_relations + 1);
  assert.equal(packet.scene.focus_vertex_id === null, origin.kind === 'relation');
  assert.deepEqual(page.context_relation_ids, origin.kind === 'relation' ? [origin.id] : []);
  if (origin.kind === 'relation') assert.ok(packet.scene.compact.relation_ids.includes(origin.id));
  for (const edge of packet.relations) assert.ok(nodes.includes(String(edge.from_id)) && nodes.includes(String(edge.to_id)));
  assert.deepEqual(packet.scene, knowledgeScene(packet.nodes, packet.relations,
    origin.kind === 'node' ? origin.id : null, origin.kind === 'relation' ? origin.id : null));
}
async function collect(db: D1Database, request: Query): Promise<Packet[]> {
  let packet = await exploreD1(db, request) as Packet;
  const pages: Packet[] = [];
  for (let count = 0; count < 500; count++) {
    assertClosure(packet); pages.push(packet);
    const cursor = packet.page.next_cursor;
    if (!cursor) return pages;
    packet = await exploreD1(db, {cursor}) as Packet;
    assert.deepEqual(await exploreD1(db, {cursor}), packet);
  }
  throw new Error('typed-origin exploration did not terminate');
}
function semanticPages(pages: Packet[]) {
  return pages.map(packet => {
    const clean = structuredClone(packet);
    delete clean.execution_version; delete clean.snapshot_revision; delete clean.page.work_units;
    clean.page.next_cursor = clean.page.next_cursor ? 'continued' : null;
    return clean;
  });
}
const inlineWorker = {modules: true, script: 'export default {fetch(){return new Response()}}'};

test('Unicode edge-whitespace schema uses the same policy as the runtime', () => {
  // Run the actual schema pattern in ECMAScript; Python-jsonschema alone
  // cannot catch differences between JavaScript and Python whitespace sets.
  const schema = JSON.parse(readFileSync(new URL('../../../contracts/exploration-request.v2.schema.json', import.meta.url), 'utf8'));
  const pattern = new RegExp(schema.$defs.id.pattern, 'u');
  const whitespace = [9, 10, 11, 12, 13, 28, 29, 30, 31, 32, 0x85, 0xa0, 0x1680,
    ...Array.from({length: 11}, (_, i) => 0x2000 + i), 0x2028, 0x2029, 0x202f, 0x205f, 0x3000];
  const cases: [string, boolean][] = whitespace.flatMap(point => [
    [String.fromCodePoint(point) + 'node', false], ['node' + String.fromCodePoint(point), false]] as [string, boolean][]);
  for (const id of ['\ufeffnode', 'node\ufeff', '\u200bnode', 'node\u200b', 'node\ninside', 'a\u0085b', '😀']) cases.push([id, true]);
  for (const [id, expected] of cases) {
    const request = {schema_version: REQUEST_V2, source_revision: 'a'.repeat(64),
      origin: {kind: 'node', id, content_revision: 'a'.repeat(64)}};
    assert.equal(pattern.test(id), expected, JSON.stringify(id));
    if (expected) assert.doesNotThrow(() => normalizeExploration(request));
    else assert.throws(() => normalizeExploration(request), (e: unknown) => e instanceof HttpError && e.status === 400);
  }
});

test('D1 typed origins conserve Python pages and independent zero/one-distance reachability', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({...inlineWorker, d1Databases: ['Plain', 'Identity']}));
  try {
    for (const identities of [false, true]) {
      const g = graph(identities ? 1 : 0, identities), db = await mf.getD1Database(identities ? 'Identity' : 'Plain');
      await init(db, g);
      const requests: Query[] = [];
      // Python owns the full combinatorial oracle. D1 keeps the new two-root
      // paths plus small exact-node canaries; legacy tests cover node BFS.
      for (const direction of ['either', 'incoming', 'outgoing']) {
        for (const profile of ['overview', 'all']) for (const size of [1, 4]) {
          requests.push(query(g, 'relation', {direction, max_depth: 2, profile, page_nodes: size, page_relations: size}));
        }
      }
      for (const max_depth of [0, 2]) requests.push(query(g, 'node', {max_depth, page_nodes: 1, page_relations: 1}));
      const expected = python("from test_exploration_origin import origin_reference;from tos_access.exploration import ExplorationService;p=json.load(sys.stdin);out=[]\nfor q in p['queries']:\n s=ExplorationService(lambda:p['graph']);r=s.explore(q);pages=[r]\n while r['page']['next_cursor']:\n  r=s.explore({'cursor':r['page']['next_cursor']});pages.append(r)\n nodes,relations,roots=origin_reference(p['graph'],q);out.append({'pages':pages,'nodes':sorted(nodes),'relations':sorted(relations),'roots':roots})\nprint(json.dumps(out))",
        {graph: g, queries: requests}) as {pages: Packet[]; nodes: string[]; relations: string[]; roots: string[]}[];
      for (const [index, request] of requests.entries()) {
        const pages = await collect(db, request), oracle = expected[index]!;
        const primary = pages.flatMap(p => p.page.primary_node_ids), emitted = pages.flatMap(p => p.page.primary_relation_ids);
        assert.equal(primary.length, new Set(primary).size);
        assert.equal(emitted.length, new Set(emitted).size);
        assert.deepEqual([...new Set([...primary, ...oracle.roots])].sort(), oracle.nodes);
        assert.deepEqual([...emitted].sort(), oracle.relations);
        assert.equal(pages.at(-1)!.counts.discovered_nodes, oracle.nodes.length);
        assert.equal(pages.at(-1)!.counts.emitted_relations, oracle.relations.length);
        assert.deepEqual(semanticPages(pages), semanticPages(oracle.pages), JSON.stringify(request));
        if (request.max_depth !== 0) assert.ok(pages[0]!.page.primary_node_ids.length + pages[0]!.page.primary_relation_ids.length > 0);
      }
      python("from test_exploration_origin import ExplorationOriginTests;ExplorationOriginTests.setUpClass();p=json.load(sys.stdin)\nfor packet in p: ExplorationOriginTests.validator.validate(packet)\nprint(json.dumps(True))",
        await collect(db, query(g, 'relation', {page_nodes: 1, page_relations: 1})));
    }
  } finally {await mf.dispose();}
});

test('D1 explicit relation survives filters, projects self-loop collapse and selected Claim folding', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({...inlineWorker, d1Databases: ['Pair', 'Loop', 'Claim']}));
  try {
    for (const selfLoop of [false, true]) {
      const g = graph(), selected = g.relations[0]!;
      if (selfLoop) selected.to_id = selected.from_id;
      selected.relation_type_id = 'tos.relation.projects'; selected.predicate_id = 'has_text_unit';
      const db = await mf.getD1Database(selfLoop ? 'Loop' : 'Pair'); await init(db, g);
      const [packet] = await collect(db, query(g, 'relation', {max_depth: 0, predicate_ids: ['not-the-origin'], page_nodes: 1, page_relations: 1}));
      assert.equal(packet!.nodes.length, selfLoop ? 1 : 2);
      assert.equal(packet!.counts.emitted_relations, 0);
      assert.deepEqual(packet!.page.primary_node_ids, []);
      assert.deepEqual(packet!.page.primary_relation_ids, []);
      assert.deepEqual(packet!.scene.collapsed_relation_ids, []);
    }
    const g = graph(), claim = g.nodes[0]!;
    claim.type_id = 'tos.entity.claim';
    (claim.semantics as Item).claim = {subject_node_id: g.nodes[1]!.id, object_node_id: g.nodes[2]!.id,
      predicate_mapping_status: 'mapped', relation_type_id: 'tos.relation.related-to'};
    g.relations = g.relations.slice(0, 2);
    g.relations.forEach((r, i) => Object.assign(r, {from_id: claim.id, to_id: g.nodes[i + 1]!.id,
      relation_type_id: i === 0 ? 'tos.relation.has-subject' : 'tos.relation.has-object'}));
    assert.equal(knowledgeScene(g.nodes, g.relations).compact.claim_paths.length, 1);
    const db = await mf.getD1Database('Claim'); await init(db, g);
    const [packet] = await collect(db, query(g, 'relation', {max_depth: 1}));
    assert.deepEqual(packet!.scene.compact.claim_paths, []);
    assert.deepEqual(packet!.scene.compact.retained_claims, [{node_id: claim.id, reason: 'focus-relation'}]);
  } finally {await mf.dispose();}
});

test('D1 binding rejects drift, aliases, excluded endpoints and corrupt carriers without admitting checkpoints', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({...inlineWorker, d1Databases: ['DB', 'Ambiguous']}));
  try {
    const g = graph(0, true), db = await mf.getD1Database('DB'), request = query(g);
    await init(db, g);
    const rejects = async (body: unknown, status: number) => {
      await assert.rejects(exploreD1(db, body), (error: unknown) => error instanceof HttpError && error.status === status);
      assert.equal(await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first<number>('n'), 0);
    };
    for (const changed of [{focus_node_id: '0'}, {cursor: 'a'.repeat(64)}, {schema_version: 'future'}, {source_revision: null},
      {origin: []}, {origin: {...request.origin, kind: []}}, {origin: {...request.origin, extra: true}},
      {source_revision: g.source_revision + '\n'}, {origin: {...request.origin, content_revision: request.origin.content_revision + '\n'}},
      {origin: {...request.origin, id: '\u0085prefixed'}}, {origin: {...request.origin, id: ' spaced '}}]) await rejects({...request, ...changed}, 400);
    await rejects({...request, source_revision: 'b'.repeat(64)}, 409);
    await rejects({...request, origin: {...request.origin, content_revision: 'b'.repeat(64)}}, 409);
    await rejects({...request, origin: {...request.origin, id: g.relations[0]!.native_id}}, 404);
    await rejects({...query(g, 'node'), origin: {...query(g, 'node').origin, id: g.nodes[0]!.entity_id}}, 404);
    await rejects({...request, sources: ['canon']}, 400);
    const node = g.nodes[1]!, changedNode = {...node, source_graph: 'canon'};
    await db.prepare('UPDATE knowledge_nodes SET source_graph=?,json=? WHERE id=?').bind('canon', JSON.stringify(changedNode), node.id).run();
    await rejects({...request, sources: ['philosophy']}, 400);
    await db.prepare('UPDATE knowledge_nodes SET source_graph=?,json=? WHERE id=?').bind(node.source_graph, JSON.stringify(node), node.id).run();
    for (const key of ['attributes', 'semantics', 'display', 'epistemic', 'predicate_mapping']) {
      await db.prepare('UPDATE knowledge_relations SET json=? WHERE id=?').bind(JSON.stringify({...g.relations[0], [key]: null}), request.origin.id).run();
      await rejects(request, 503);
    }
    await db.prepare('UPDATE knowledge_relations SET json=? WHERE id=?').bind(JSON.stringify(g.relations[0]), request.origin.id).run();
    for (const value of [null, [], {...node, semantics: {time: []}}, {...node, id: 'different'}, {...node, source_refs: []}]) {
      await db.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').bind(JSON.stringify(value), node.id).run();
      await rejects(request, 503);
    }
    await db.prepare('DELETE FROM knowledge_nodes WHERE id=?').bind(node.id).run();
    await rejects(request, 503);
    const ambiguous = await mf.getD1Database('Ambiguous'); await init(ambiguous, g, true);
    await ambiguous.prepare('INSERT INTO knowledge_relations SELECT * FROM knowledge_relations WHERE id=?').bind(request.origin.id).run();
    await assert.rejects(exploreD1(ambiguous, request), (e: unknown) => e instanceof HttpError && e.status === 404);
    await ambiguous.prepare('DELETE FROM knowledge_relations WHERE rowid=(SELECT max(rowid) FROM knowledge_relations)').run();
    await ambiguous.prepare('INSERT INTO knowledge_nodes SELECT * FROM knowledge_nodes WHERE id=?').bind(node.id).run();
    await assert.rejects(exploreD1(ambiguous, request), (e: unknown) => e instanceof HttpError && e.status === 503);
  } finally {await mf.dispose();}
});

test('exact D1 origin closure uses one indexed relation and at most two indexed node reads', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({...inlineWorker, d1Databases: ['DB']}));
  try {
    const g = graph(), db = await mf.getD1Database('DB'); await init(db, g);
    const sql: string[] = [];
    const counted = new Proxy(db, {get(target, property) {
      if (property === 'prepare') return (statement: string) => {sql.push(statement); return target.prepare(statement);};
      const value = Reflect.get(target, property); return typeof value === 'function' ? value.bind(target) : value;
    }});
    const requested = normalizeExploration(query(g)); assert.ok('origin' in requested);
    const result = await bindOriginD1(counted, requested, g.source_revision);
    assert.equal(sql.length, 3); assert.equal(result.roots.length, 2);
    for (const statement of sql) {
      assert.match(statement, /WHERE id=\? LIMIT 2$/);
      const plan = await db.prepare('EXPLAIN QUERY PLAN ' + statement).bind(requested.origin.id).all<{detail: string}>();
      assert.match(plan.results.map(r => r.detail).join('\n'), /SEARCH .* USING INDEX .* \(id=\?\)/);
    }
    await db.prepare('UPDATE knowledge_relations SET json=json_set(json,\'$.to_id\',?) WHERE id=?')
      .bind(g.relations[0]!.from_id, requested.origin.id).run();
    sql.length = 0;
    const loop = await bindOriginD1(counted, requested, g.source_revision);
    assert.equal(sql.length, 2); assert.equal(loop.roots.length, 1);
  } finally {await mf.dispose();}
});

test('actual Worker v2 HTTP replay survives isolate restart and concurrency, but rejects ABA and expiry', async () => {
  const bundle = await build({entryPoints: [fileURLToPath(new URL('../src/index.ts', import.meta.url))],
    bundle: true, write: false, format: 'esm', platform: 'browser', target: 'es2022'});
  const directory = mkdtempSync(join(tmpdir(), 'tos-origin-'));
  const options = () => convertV4MiniflareOptions({modules: true, script: bundle.outputFiles[0]!.text,
    d1Databases: ['DB'], resourcePersistencePath: directory});
  let mf = new Miniflare(options());
  const post = (body: unknown) => mf.dispatchFetch('http://tos.test/api/knowledge/explore',
    {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)});
  try {
    const g = graph(); let db = await mf.getD1Database('DB'); await init(db, g);
    const capabilities = await explorationCapabilitiesD1(db);
    assert.deepEqual(capabilities.v2_origin_kinds, ['node', 'relation']);
    const request = query(g, 'relation', {page_nodes: 1, page_relations: 1});
    const initialResponse = await post(request); assert.equal(initialResponse.status, 200);
    const initial = await initialResponse.json() as Packet; assertClosure(initial);
    const cursor = initial.page.next_cursor; assert.ok(cursor);
    await mf.dispose(); mf = new Miniflare(options()); db = await mf.getD1Database('DB');
    const retries = await Promise.all(Array.from({length: 8}, async () => {
      const response = await post({cursor}); assert.equal(response.status, 200); return await response.json() as Packet;
    }));
    for (const packet of retries) {assert.deepEqual(packet, retries[0]); assertClosure(packet); assert.deepEqual(packet.origin, initial.origin);}
    const next = retries[0]!.page.next_cursor; assert.ok(next);
    assert.equal(await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first<number>('n'), 2);
    assert.equal((await post({cursor: next})).status, 200);
    for (const [body, status] of [[{cursor: 'bad'}, 400], [{cursor: cursor + '\n'}, 400], [{cursor: '0'.repeat(64)}, 410], [{cursor, origin: initial.origin}, 400],
      [{...request, source_revision: 'b'.repeat(64)}, 409], [{...request, origin: {...request.origin, id: 'missing'}}, 404]] as const) {
      assert.equal((await post(body)).status, status);
    }
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256: 'b'.repeat(64)})).run();
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256: g.source_revision})).run();
    assert.equal((await post({cursor})).status, 409);
    const fresh = await (await post(request)).json() as Packet;
    await db.prepare('UPDATE knowledge_exploration_checkpoints SET expires=0 WHERE token=?').bind(fresh.page.next_cursor).run();
    assert.equal((await post({cursor: fresh.page.next_cursor})).status, 410);
    const old = await (await post(request)).json() as Packet;
    await db.prepare("UPDATE knowledge_exploration_checkpoints SET version='tos-exploration-d1-execution-v5' WHERE token=?").bind(old.page.next_cursor).run();
    assert.equal((await post({cursor: old.page.next_cursor})).status, 409);
    const legacy = await (await post({focus_node_id: '0', max_depth: 0})).json() as Item;
    assert.equal(legacy.schema, 'tos_exploration_result_v1'); assert.equal(Object.hasOwn(legacy, 'origin'), false);
  } finally {await mf.dispose(); rmSync(directory, {recursive: true, force: true});}
});
