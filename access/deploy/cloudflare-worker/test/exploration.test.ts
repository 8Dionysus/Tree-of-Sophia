import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFileSync, mkdtempSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {build} from 'esbuild';
import {Miniflare, convertV4MiniflareOptions} from 'miniflare';
import {ADJACENCY_SQL, IDENTITY_SQL, exploreD1, explorationCapabilitiesD1, normalizeExploration} from '../src/exploration.ts';
import {knowledgeScene, type Item} from '../src/knowledge.ts';

const migration = readFileSync(new URL('../migrations/0001-exploration.sql', import.meta.url), 'utf8').replace(/^--.*$/gm, '').trim();
const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const python = (code: string, input: unknown) => JSON.parse(execFileSync('python3', ['-c',
  "import sys,json;sys.path[:0]=['access/src','access/tests'];" + code], {cwd: repo, input: JSON.stringify(input), encoding: 'utf8'}));
function graph(size = 12, seed = 0) {
  return python("from test_exploration import graph_for;p=json.load(sys.stdin);print(json.dumps(graph_for(p['size'],p['seed'])))", {size, seed});
}
async function init(db: D1Database, g: ReturnType<typeof graph>, migrated = true) {
  await db.batch([
    db.prepare('CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part))'),
    db.prepare('CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,entity_id TEXT,native_id TEXT,source_graph TEXT,json TEXT)'),
    db.prepare('CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,from_id TEXT,to_id TEXT,source_graph TEXT,predicate_id TEXT,json TEXT)'),
    db.prepare("INSERT INTO edge_meta VALUES ('data_revision',0,?)").bind(JSON.stringify({sha256: g.source_revision})),
    db.prepare("INSERT INTO edge_meta VALUES ('knowledge_exploration_top',0,?)").bind(JSON.stringify({source_revision:g.source_revision, authority_boundary:{writes_to_tree:false}})),
    ...g.nodes.map((n: {id:string;source_graph:string;entity_id?:string}) => db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?)').bind(n.id,n.entity_id??n.id,n.id,n.source_graph,JSON.stringify(n))),
    ...g.relations.map((r: {id:string;from_id:string;to_id:string;source_graph:string;predicate_id:string}) => db.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?)').bind(r.id,r.from_id,r.to_id,r.source_graph,r.predicate_id,JSON.stringify(r))),
  ]);
  if (migrated) await db.batch(migration.split(/\n(?=CREATE |INSERT )/).map(s => db.prepare(s)));
}
async function collect(db: D1Database, query: unknown) {
  const pages = []; let page = await exploreD1(db, query);
  for (let count = 0; count < 2000; count++) {
    pages.push(page);
    const focus = page.focus as {node_id: string};
    assert.deepEqual(page.scene, knowledgeScene(page.nodes as Item[], page.relations as Item[], focus.node_id));
    const cursor = (page.page as {next_cursor:string|null}).next_cursor;
    if (!cursor) return pages;
    page = await exploreD1(db, {cursor});
    assert.deepEqual(await exploreD1(db, {cursor}), page);
  }
  throw Error('exploration did not terminate');
}
function membership(pages: Awaited<ReturnType<typeof collect>>) {
  return {
    nodes: pages.flatMap(p => (p.page as {primary_node_ids:string[]}).primary_node_ids),
    edges: pages.flatMap(p => (p.relations as {id:string}[]).map(e => e.id)),
  };
}

function valueMemberGraph() {
  // Synthetic three-occurrence proposal, not historical evidence or accepted
  // membership. The focal occurrence is one member of the whole qualified set.
  const g = graph(6), members = ['0', '4', '5'];
  for (const id of members) g.nodes[Number(id)].type_id = 'tos.entity.occurrence';
  Object.assign(g.nodes[1], {type_id: 'tos.entity.claim', content_revision: 'b'.repeat(64),
    semantics: {claim: {subject_node_id: '0', object_node_id: '2', value_member_node_ids: members,
      relation_type_id: 'tos.relation.occurrence-motif-proposal', predicate_mapping_status: 'mapped',
      review_status: 'unreviewed'}},
    attributes: {source_claim: {object: {kind: 'motif-proposal', members: [...members]}}},
    display: {title: {default: 'Unassessed motif proposal'},
      summary: {default: 'A qualified proposal over occurrences 0, 4 and 5; not accepted membership.'},
      provenance: {source_summary_available: true}}});
  g.relations = [
    ['subject', '0', 'tos.relation.has-subject'],
    ['object', '2', 'tos.relation.has-object'],
    ['grounds', '3', 'tos.relation.claim-supported-by'],
    ...members.map((id, i) => ['member-' + i, id, 'tos.relation.claim-value-member']),
  ].map(([id, to_id, relation_type_id]) => ({...g.relations[0], id, from_id: '1', to_id, relation_type_id}));
  return g;
}

test('compact value-member context requires the full set and preserves focused or shared details', () => {
  const original = valueMemberGraph();
  const cases: {name: string; graph: ReturnType<typeof valueMemberGraph>; focus: string | null; reason: string | null}[] = [
    {name: 'complete', graph: original, focus: null, reason: null},
    {name: 'object focus', graph: original, focus: '2', reason: null},
    {name: 'claim focus', graph: original, focus: '1', reason: 'focus-claim'},
    ...['0', '3', '4', '5'].map(focus => ({name: 'detail focus ' + focus, graph: original, focus, reason: 'focus-detail'})),
  ];
  for (const name of ['missing third edge', 'missing third node', 'duplicate third target', 'extra member edge', 'incoming member edge']) {
    const g = structuredClone(original);
    if (name.startsWith('missing')) g.relations = g.relations.filter((r: Item) => r.id !== 'member-2');
    if (name === 'missing third node') g.nodes = g.nodes.filter((n: Item) => n.id !== '5');
    if (name === 'duplicate third target') g.relations.find((r: Item) => r.id === 'member-2').to_id = '4';
    if (name === 'extra member edge') g.relations.push({...g.relations.find((r: Item) => r.id === 'member-1'), id: 'member-extra'});
    if (name === 'incoming member edge') Object.assign(g.relations.find((r: Item) => r.id === 'member-2'), {from_id: '5', to_id: '1'});
    // Text and raw values keep all members even when the scene packet does not.
    assert.deepEqual(g.nodes[1].attributes.source_claim.object.members, ['0', '4', '5']);
    assert.equal(g.nodes[1].display.summary.default, original.nodes[1].display.summary.default);
    cases.push({name, graph: g, focus: null, reason: 'incomplete-value-member-context'});
  }
  for (const members of [null, '0,4,5', [], ['0', '4', '4'], ['0', '4', 5]]) {
    const g = structuredClone(original);
    g.nodes[1].semantics.claim.value_member_node_ids = members;
    cases.push({name: 'malformed member declaration ' + JSON.stringify(members), graph: g, focus: null,
      reason: 'incomplete-value-member-context'});
  }
  const undeclared = structuredClone(original);
  delete undeclared.nodes[1].semantics.claim.value_member_node_ids;
  cases.push({name: 'member edges without declaration', graph: undeclared, focus: null, reason: 'incomplete-value-member-context'});
  for (const members of [[], null]) {
    const empty = structuredClone(original);
    empty.nodes[1].semantics.claim.value_member_node_ids = members;
    empty.relations = empty.relations.filter((r: Item) => r.relation_type_id !== 'tos.relation.claim-value-member');
    cases.push({name: 'empty declaration without member edges ' + JSON.stringify(members), graph: empty, focus: null,
      reason: 'incomplete-value-member-context'});
  }
  const shared = structuredClone(original);
  shared.relations.push({...shared.relations[0], id: 'member-neighborhood', from_id: '4', to_id: '5',
    relation_type_id: 'tos.relation.related'});
  cases.push({name: 'shared member neighborhood', graph: shared, focus: null, reason: null});
  const expected = python("from tos_access.knowledge import knowledge_scene;p=json.load(sys.stdin);print(json.dumps([knowledge_scene(c['graph']['nodes'],c['graph']['relations'],c['focus']) for c in p]))", cases);
  for (const [index, c] of cases.entries()) {
    const before = structuredClone(c.graph);
    const scene = knowledgeScene(c.graph.nodes, c.graph.relations, c.focus), compact = scene.compact;
    assert.deepEqual(scene, expected[index], c.name + ': Python parity');
    assert.deepEqual(c.graph, before, c.name + ': input records remain unchanged');
    assert.equal(scene.vertices.flatMap(v => v.node_ids).length, c.graph.nodes.length, c.name + ': raw vertices');
    assert.equal(scene.arcs.length, c.graph.relations.length, c.name + ': raw arcs');
    assert.equal(compact.rule, 'explicit-claim-paths-v1');
    assert.equal(compact.authority, 'presentation-only-no-new-assertion');
    if (c.reason) {
      assert.deepEqual(compact.claim_paths, [], c.name);
      assert.deepEqual(compact.retained_claims, [{node_id: '1', reason: c.reason}], c.name);
      assert.equal(compact.vertex_ids.length, c.graph.nodes.length, c.name);
      assert.equal(compact.relation_ids.length, c.graph.relations.length, c.name);
    } else {
      assert.equal(compact.claim_paths.length, 1, c.name);
      const path = compact.claim_paths[0]!;
      assert.deepEqual(path.node_ids, ['0', '1', '2'], c.name + ': one qualified set, not pairwise assertions');
      assert.deepEqual(path.detail_relation_ids, ['grounds', 'member-0', 'member-1', 'member-2']);
      assert.deepEqual((path.reading as Item).relation_context_ids, ['subject', 'object', 'grounds', 'member-0', 'member-1', 'member-2']);
      assert.equal((path.reading as Item).standalone, false);
      assert.ok(compact.vertex_ids.includes('tos-scene:carrier:0'), 'the focal member remains a path endpoint');
      if (c.name === 'shared member neighborhood') {
        assert.deepEqual(compact.relation_ids, ['member-neighborhood']);
        for (const id of ['4', '5']) assert.ok(compact.vertex_ids.includes('tos-scene:carrier:' + id));
      } else {
        assert.deepEqual(compact.relation_ids, []);
        assert.deepEqual(compact.folded_vertex_ids, ['1', '3', '4', '5'].map(id => 'tos-scene:carrier:' + id));
      }
    }
  }
});

test('overview identity steps are resumable, zero distance, filtered and promote shorter queued paths', async () => {
  for (const shorter of [false, true]) {
    const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
    try {
      const db = await mf.getD1Database('DB'), g = graph(5);
      const entities = shorter ? ['focus','bridge','shared','shared','target'] : ['person','person','claim','work','work'];
      g.nodes.forEach((n: Item, i: number) => {n.entity_id = 'tos.test.' + entities[i];});
      const pairs = shorter ? [['0','1'],['0','2'],['1','3'],['3','4']] : [['2','1'],['2','3']];
      g.relations = pairs.map(([from_id,to_id],i) => ({...g.relations[0],id:String(i),from_id,to_id}));
      await init(db,g);
      for (const profile of ['overview','all']) for (const size of [1,3]) {
        const query = {focus_node_id:'0',max_depth:2,profile,page_nodes:size,page_relations:1};
        const expected = python("from tos_access.exploration import ExplorationService;p=json.load(sys.stdin);s=ExplorationService(lambda:p['graph'],work_limit=2);r=s.explore(p['query']);out=[r]\nwhile r['page']['next_cursor']:\n r=s.explore({'cursor':r['page']['next_cursor']});out.append(r)\nprint(json.dumps(out))",{graph:g,query});
        const pages = await collect(db,query);
        assert.deepEqual(membership(pages),membership(expected));
        const found = new Set(pages.flatMap(p => (p.nodes as {id:string}[]).map(n => n.id)));
        assert.equal(found.has(shorter?'4':'3'),profile==='overview');
        for (const p of pages) {
          assert.ok((p.nodes as unknown[]).length <= 1 + size + 2);
          assert.ok((p.page as {work_units:number}).work_units <= 512);
        }
      }
      g.nodes[1].source_graph = 'source-claims';
      await db.prepare('UPDATE knowledge_nodes SET source_graph=?,json=? WHERE id=?').bind('source-claims',JSON.stringify(g.nodes[1]),'1').run();
      const filtered = await collect(db,{focus_node_id:'0',sources:['philosophy'],max_depth:2,page_nodes:1});
      assert.ok(filtered.every(p => (p.nodes as {source_graph:string}[]).every(n => n.source_graph==='philosophy')));
      const plan = await db.prepare('EXPLAIN QUERY PLAN '+IDENTITY_SQL).bind('0','[]','',JSON.stringify(['philosophy'])).all<{detail:string}>();
      assert.match(plan.results.map(r=>r.detail).join('\n'),/knowledge_nodes_identity_seek.*entity_id=\? AND id>\?/);
      await db.exec('DROP INDEX knowledge_nodes_identity_seek');
      assert.equal((await explorationCapabilitiesD1(db)).available, false);
    } finally {await mf.dispose();}
  }
});

test('D1 continuation delivers the same qualified Claim scene as Python without inventing missing page legs', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
  try {
    const db = await mf.getD1Database('DB'), g = graph(4);
    g.nodes[1].type_id = 'tos.entity.claim';
    g.nodes[1].content_revision = 'b'.repeat(64);
    g.nodes[1].semantics = {claim:{subject_node_id:'0',object_node_id:'2',
      relation_type_id:'tos.relation.correspondence-addressee',predicate_mapping_status:'mapped',review_status:'contested'},
      assertion_contexts:[{fields:{polarity:{value:'negative'},qualifiers:{value:{unknown:false}}}}]};
    g.nodes[1].display = {title:{default:'A disputed attribution'},summary:{default:'Not established by this evidence.'},
      provenance:{source_summary_available:true}};
    g.relations = ['tos.relation.has-subject','tos.relation.has-object','tos.relation.claim-supported-by']
      .map((type,i)=>({...g.relations[0],id:'edge'+i,from_id:'1',to_id:['0','2','3'][i],relation_type_id:type}));
    await init(db,g);
    for (const size of [1,6]) {
      const query = {focus_node_id:'0',max_depth:2,page_nodes:size,page_relations:size};
      const pages = await collect(db,query);
      let count = 0;
      for (const page of pages) {
        const focus = (page.focus as {node_id:string}).node_id;
        const expected = python("from tos_access.knowledge import knowledge_scene;p=json.load(sys.stdin);print(json.dumps(knowledge_scene(p['nodes'],p['relations'],p['focus'])))",
          {nodes:page.nodes,relations:page.relations,focus});
        assert.deepEqual(page.scene,expected);
        const paths = (page.scene as {compact:{claim_paths:{reading:{wording_pointer:string;standalone:boolean}}[]}}).compact.claim_paths;
        count += paths.length;
        for (const path of paths) {
          assert.equal(path.reading.wording_pointer,'/display_selection/fields/summary');
          assert.equal(path.reading.standalone,false);
        }
      }
      assert.equal(count,size===1?0:1);
    }
  } finally {await mf.dispose();}
});

test('D1 motif value-member pages preserve the complete context and fail closed on a missing third edge', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules: true,
    script: 'export default {fetch(){return new Response()}}', d1Databases: ['Complete', 'Missing']}));
  try {
    for (const missing of [false, true]) {
      const db = await mf.getD1Database(missing ? 'Missing' : 'Complete'), g = valueMemberGraph();
      if (missing) {
        g.relations = g.relations.filter((r: Item) => r.id !== 'member-2');
        // Keep the third occurrence in the returned raw packet via a different
        // neighborhood. Neither its presence nor source wording supplies the
        // missing typed member edge.
        g.relations.push({...g.relations[0], id: 'other-neighborhood', from_id: '0', to_id: '5',
          relation_type_id: 'tos.relation.related'});
      }
      await init(db, g);
      for (const focus of ['2', '0', '5']) for (const size of [1, 8]) {
        const pages = await collect(db, {focus_node_id: focus, max_depth: 3, page_nodes: size, page_relations: size});
        const expected = python("from tos_access.knowledge import knowledge_scene;p=json.load(sys.stdin);print(json.dumps([knowledge_scene(c['nodes'],c['relations'],c['focus']['node_id']) for c in p]))", pages);
        let folded = 0;
        for (const [index, page] of pages.entries()) {
          assert.deepEqual(page.scene, expected[index]);
          const scene = page.scene as ReturnType<typeof knowledgeScene>;
          folded += scene.compact.claim_paths.length;
          for (const path of scene.compact.claim_paths) {
            assert.deepEqual(path.detail_relation_ids, ['grounds', 'member-0', 'member-1', 'member-2']);
            assert.equal((path.reading as Item).standalone, false);
          }
          if (size === 8) {
            assert.equal((page.nodes as Item[]).length, 6, 'all raw member nodes remain available');
            const claim = (page.nodes as {id: string; semantics: {claim: {value_member_node_ids: string[]}};
              display: {summary: {default: string}}}[]).find(n => n.id === '1')!;
            assert.deepEqual(claim.semantics.claim.value_member_node_ids, ['0', '4', '5']);
            assert.equal(claim.display.summary.default, g.nodes[1].display.summary.default);
            // Exploration is a compact carrier; full raw attributes stay in
            // the indexed inspection record, not duplicated into every page.
            const stored = JSON.parse((await db.prepare('SELECT json FROM knowledge_nodes WHERE id=?').bind('1').first<string>('json'))!);
            assert.deepEqual(stored.attributes.source_claim.object.members, ['0', '4', '5']);
            if (missing) assert.deepEqual(scene.compact.retained_claims,
              [{node_id: '1', reason: 'incomplete-value-member-context'}]);
            else if (focus !== '2') assert.deepEqual(scene.compact.retained_claims,
              [{node_id: '1', reason: 'focus-detail'}]);
          }
        }
        assert.equal(folded, !missing && focus === '2' && size === 8 ? 1 : 0);
      }
    }
  } finally {await mf.dispose();}
});

test('D1 exploration conserves Python BFS order across direction, depth, size and cycles', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
  try {
    const db = await mf.getD1Database('DB'), g = graph(6);
    await init(db,g);
    for (const direction of ['either','incoming','outgoing']) for (const max_depth of [1,3]) for (const size of [1,7]) {
      const query = {focus_node_id:'0',direction,max_depth,page_nodes:size,page_relations:size};
      const expected = python("from tos_access.exploration import ExplorationService;p=json.load(sys.stdin);s=ExplorationService(lambda:p['graph']);r=s.explore(p['query']);out=[r]\nwhile r['page']['next_cursor']:\n r=s.explore({'cursor':r['page']['next_cursor']});out.append(r)\nprint(json.dumps(out))",{graph:g,query});
      const pages = await collect(db,query);
      assert.deepEqual(membership(pages),membership(expected));
      assert.equal(pages.at(-1)!.status,'complete');
      for (const page of pages) {
        const ids = new Set((page.nodes as {id:string}[]).map(n => n.id));
        for (const e of page.relations as {id:string;from_id:string;to_id:string}[]) assert.ok(ids.has(e.from_id)&&ids.has(e.to_id));
      }
    }
    const plan = await db.prepare('EXPLAIN QUERY PLAN '+ADJACENCY_SQL).bind('0','','0','').all<{detail:string}>();
    const details = plan.results.map(r=>r.detail).join('\n');
    assert.match(details,/knowledge_relations_from_seek.*from_id=\? AND id>\?/);
    assert.match(details,/knowledge_relations_to_seek.*to_id=\? AND id>\?/);
  } finally {await mf.dispose();}
});

test('many carriers of one identity do not cause quadratic D1 expansion work', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
  try {
    const db = await mf.getD1Database('DB'), g = graph(64);
    g.relations = [];
    g.nodes.forEach((n: Item) => {n.entity_id = 'tos.test.one-subject';});
    await init(db,g);
    const pages = await collect(db,{focus_node_id:'0',max_depth:1,page_nodes:7});
    assert.equal(membership(pages).nodes.length,64);
    assert.ok(pages.reduce((sum,p) => sum + (p.page as {work_units:number}).work_units,0) <= 4 * 64 + 1);
  } finally {await mf.dispose();}
});

test('overview exploration excludes typed record provenance but not unknown predicate spellings', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
  try {
    const db = await mf.getD1Database('DB'), g = graph(3);
    g.relations = [
      {...g.relations[0], id:'maker-a',from_id:'0',to_id:'1',predicate_id:'made_by',relation_type_id:'tos.relation.made-by'},
      {...g.relations[0], id:'maker-b',from_id:'2',to_id:'1',predicate_id:'made_by',relation_type_id:'tos.relation.made-by'},
    ];
    await init(db,g);
    for (const relationType of ['tos.relation.made-by','tos.relation.generated-by','tos.relation.related']) {
      for (const edge of g.relations) {
        edge.relation_type_id=relationType;
        await db.prepare('UPDATE knowledge_relations SET json=? WHERE id=?').bind(JSON.stringify(edge),edge.id).run();
      }
      for (const profile of ['overview','all']) {
        const pages=await collect(db,{focus_node_id:'0',max_depth:2,profile,page_nodes:1,page_relations:1});
        assert.deepEqual(membership(pages).nodes, profile==='overview'&&relationType!=='tos.relation.related'?['0']:['0','1','2']);
      }
    }
  } finally {await mf.dispose();}
});

test('actual Worker HTTP continuation survives isolate restart and concurrent retries', async () => {
  const bundle = await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'});
  const directory = mkdtempSync(join(tmpdir(),'tos-exploration-'));
  const options = () => convertV4MiniflareOptions({modules:true,script:bundle.outputFiles[0]!.text,d1Databases:['DB'],resourcePersistencePath:directory});
  let mf = new Miniflare(options());
  const post = (body: unknown) => mf.dispatchFetch('http://tos.test/api/knowledge/explore',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
  try {
    let db = await mf.getD1Database('DB');
    await init(db,graph(),false);
    assert.equal((await explorationCapabilitiesD1(db)).available,false);
    assert.equal((await post({focus_node_id:'0'})).status,503);
    await db.batch(migration.split(/\n(?=CREATE |INSERT )/).map(s=>db.prepare(s)));
    const capabilities = await (await mf.dispatchFetch('http://tos.test/api/knowledge/explore/capabilities')).json() as {available:boolean;restart_survival:boolean};
    assert.equal(capabilities.available,true);assert.equal(capabilities.restart_survival,true);
    const initial = await (await post({focus_node_id:'0',page_nodes:1})).json() as {page:{next_cursor:string}};
    const cursor = initial.page.next_cursor;
    await mf.dispose();mf = new Miniflare(options());db = await mf.getD1Database('DB');
    const results = await Promise.all(Array.from({length:8},async()=>{
      const response=await post({cursor});assert.equal(response.status,200);return response.json();
    }));
    for (const response of results) assert.deepEqual(response,results[0]);
    const next=(results[0] as {page:{next_cursor:string}}).page.next_cursor;
    assert.ok(next);assert.equal((await post({cursor:next})).status,200);
    const checkpointCount = await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first<number>('n');
    assert.equal(checkpointCount,3,'concurrent losers must not create orphan successors');
    for (const [body,status] of [[{cursor:'bad'},400],[{cursor:'0'.repeat(64)},410],[{cursor,profile:'all'},400]] as const) assert.equal((await post(body)).status,status);
    // ABA is rejected even though the content revision ends at its original value.
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'b'.repeat(64)})).run();
    await db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'a'.repeat(64)})).run();
    assert.equal((await post({cursor})).status,409);
    const fresh=await (await post({focus_node_id:'0',page_nodes:1})).json() as {page:{next_cursor:string}};
    await db.prepare('UPDATE knowledge_exploration_checkpoints SET expires=0 WHERE token=?').bind(fresh.page.next_cursor).run();
    assert.equal((await post({cursor:fresh.page.next_cursor})).status,410);
    for (const version of [1, 2, 3, 4]) {
      const oldExecution=await (await post({focus_node_id:'0',page_nodes:1})).json() as {page:{next_cursor:string}};
      await db.prepare('UPDATE knowledge_exploration_checkpoints SET version=? WHERE token=?')
        .bind(`tos-exploration-d1-execution-v${version}`,oldExecution.page.next_cursor).run();
      assert.equal((await post({cursor:oldExecution.page.next_cursor})).status,409);
    }
  } finally {await mf.dispose();rmSync(directory,{recursive:true,force:true});}
});

test('D1 rejects crossed publication before committing a page; cache admission is atomic', async () => {
  const mf = new Miniflare(convertV4MiniflareOptions({modules:true,script:'export default {fetch(){return new Response()}}',d1Databases:['DB']}));
  try {
    const db=await mf.getD1Database('DB');await init(db,graph());
    const first=await exploreD1(db,{focus_node_id:'0',page_nodes:1});const cursor=(first.page as {next_cursor:string}).next_cursor;
    const original=db.batch.bind(db);
    let crossed=false;
    // Interpose at the real transaction boundary, after all graph reads.
    const intercepted = new Proxy(db, {get(target,property) {
      if (property === 'batch') return async (statements: D1PreparedStatement[]) => {
        if (!crossed) {crossed=true;await db.prepare("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'").run();}
        return original(statements);
      };
      const value=Reflect.get(target,property);return typeof value==='function' ? value.bind(target) : value;
    }});
    await assert.rejects(exploreD1(intercepted,{cursor}),/snapshot changed/);
    assert.ok(crossed);
    const record=await db.prepare('SELECT response FROM knowledge_exploration_checkpoints WHERE token=?').bind(cursor).first<{response:string|null}>();
    assert.equal(record!.response,null);
    // An oversized page is rejected before any new checkpoint or replay is admitted.
    await db.prepare("UPDATE knowledge_nodes SET json=json_set(json,'$.display.summary',?) WHERE id='0'").bind('x'.repeat(1_050_000)).run();
    const before=await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first<number>('n');
    await assert.rejects(exploreD1(db,{focus_node_id:'0',page_nodes:1}),/checkpoint exceeds/);
    assert.equal(await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first<number>('n'),before);
  } finally {await mf.dispose();}
});

test('D1 request validation agrees with local schema boundaries', () => {
  for (const value of [{},{focus_node_id:' '},{focus_node_id:'0',sources:[]},{focus_node_id:'0',page_nodes:true},{focus_node_id:'0',max_depth:null},{focus_node_id:'0',profile:null},{focus_node_id:'0',direction:[]},{focus_node_id:'0',extra:1}]) assert.throws(()=>normalizeExploration(value));
});
