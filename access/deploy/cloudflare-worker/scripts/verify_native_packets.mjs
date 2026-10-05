// Maintained local verification request matrix from verify_local_runtime.py.
// Product semantics come from the selected native reader and Worker.
import assert from 'node:assert/strict';
const quote = value => encodeURIComponent(value).replace(/[!'()*]/g, c => '%' + c.charCodeAt(0).toString(16).toUpperCase());
export function nativeVerificationCases(profile, knowledge_relation_id) {
  const relation_id = knowledge_relation_id;
  const node_id = 'candidate-node:table-i-a01-node-016', target_id = 'candidate-node:table-i-a01-node-014';
  const source_work_id = 'tos.work.egyptian-scholarship.on-four-songs-contained-in-an-egyptian-papyrus-in-the-british-museum';
  const knowledge_node_id = profile === 'representative' ? 'philosophy:a' : 'philosophy:philosophy.atlas';
  const knowledge_author_id = 'source-navigation:tos.agent.friedrich-nietzsche';
  const knowledge_work_id = 'tos.work.friedrich-nietzsche.also-sprach-zarathustra';
  const large_knowledge_node_id = 'canon:tos.source.thus-spoke-zarathustra.prologue';
  if (profile === "representative") return [
    ["corpus status", "/api/corpus/status"],
    ["corpus summary", "/api/corpus/summary"],
    ["philosophy status", "/api/philosophy/status"],
    ["philosophy views", "/api/philosophy/views"],
    ["knowledge catalog", "/api/knowledge/catalog"],
    ["knowledge contracts", "/api/knowledge/contracts"],
    ["knowledge search", "/api/knowledge/search?query=Alpha&sources=philosophy&limit=5"],
    ["knowledge node", ("/api/knowledge/nodes/" + quote(knowledge_node_id) + "?relation_limit=20")],
    ["knowledge relation", ("/api/knowledge/relations/" + quote(relation_id))],
    ["knowledge focus", ("/api/knowledge/focus/" + quote(knowledge_node_id) + "?depth=1")],
    ["stored knowledge lens", "/api/knowledge/lenses/chronology"],
    ["philosophy view", "/api/philosophy/views/chronology?limit=100"],
    ["corpus view", "/api/corpus/graph-views/corpus-topology?limit=37"],
    ["corpus search", "/api/corpus/search?query=Alpha&limit=5"],
    ["philosophy search", "/api/philosophy/search?query=Alpha&limit=5"],
    ["source descent", "/api/source/navigation/philosophy.eras.fixture?max_depth=8&limit=20"],
    ["source dossier", "/api/source/dossiers/tos.link.fixture.download?limit=20"],
    ["philosophy node", "/api/philosophy/nodes/a"],
    ["philosophy neighborhood", "/api/philosophy/neighborhood/a?depth=1&limit=10"],
    ["philosophy path", "/api/philosophy/paths?from=a&to=b&max_depth=2&direction=outgoing&alternatives=2"],
    ["philosophy evidence", "/api/philosophy/query/epistemic/a?view_id=direct-only&limit=10"]
  ];
  if (profile === "production") return [
    ["corpus status", "/api/corpus/status"],
    ["corpus summary", "/api/corpus/summary"],
    ["philosophy status", "/api/philosophy/status"],
    ["philosophy views", "/api/philosophy/views"],
    ["knowledge catalog", "/api/knowledge/catalog"],
    ["knowledge contracts", "/api/knowledge/contracts"],
    ["knowledge search", "/api/knowledge/search?query=Zarathustra&sources=philosophy&limit=5"],
    ["Unicode knowledge search", ("/api/knowledge/search?query=" + quote("Заратустра") + "&sources=philosophy&limit=5")],
    ["knowledge node", ("/api/knowledge/nodes/" + quote(knowledge_node_id) + "?relation_limit=20")],
    ["lossless large knowledge node", ("/api/knowledge/nodes/" + quote(large_knowledge_node_id) + "?relation_limit=20")],
    ["knowledge relation", ("/api/knowledge/relations/" + quote(knowledge_relation_id))],
    ["focused knowledge neighborhood", ("/api/knowledge/focus/" + quote(knowledge_node_id) + "?sources=philosophy&depth=1&direction=either&node_limit=40&relation_limit=40")],
    ["author-to-works knowledge neighborhood", ("/api/knowledge/focus/" + quote(knowledge_author_id) + "?sources=source-navigation&depth=1&direction=either&node_limit=40&relation_limit=40")],
    ["cross-layer work knowledge neighborhood", ("/api/knowledge/focus/" + quote(knowledge_work_id) + "?sources=canon,source-navigation,source-claims,semantic-interchange&depth=5&direction=either&predicates=authored_by,has_expression,embodied_by,has_subject,has_object,has_normalized_place,projects,grounded_in,commentary-on&node_limit=400&relation_limit=800")],
    ["stored knowledge lens", "/api/knowledge/lenses/corpus-topology"],
    ["Zarathustra word-analysis capability", "/api/zarathustra/word-analysis?query=Geist&language=de&rank=2&include_semantic_neighbors=true"],
    ["chronology view", "/api/philosophy/views/chronology?limit=1000"],
    ["dynamic corpus view", "/api/corpus/graph-views/route-graph?limit=37"],
    ["corpus search", "/api/corpus/search?query=zarathustra&limit=5"],
    ["philosophy search", "/api/philosophy/search?query=Gilgamesh&limit=5"],
    ["source descent", ("/api/source/navigation/" + quote(source_work_id) + "?max_depth=3&limit=40")],
    ["source dossier", ("/api/source/dossiers/" + quote(source_work_id) + "?limit=300")],
    ["node packet", ("/api/philosophy/nodes/" + quote(node_id))],
    ["neighborhood", ("/api/philosophy/neighborhood/" + quote(node_id) + "?depth=1&limit=10")],
    ["path", ("/api/philosophy/paths?from=" + quote(node_id) + "&to=" + quote(target_id) + "&max_depth=2&direction=outgoing&alternatives=2")],
    ["philosophy evidence lens", ("/api/philosophy/query/epistemic/" + quote(node_id) + "?limit=10")],
    ["corpus evidence lens", "/api/corpus/query/epistemic/m113?view_id=route-graph&limit=10"]
  ];
  throw new Error('TOS_VERIFY_PROFILE must be production or representative');
}

// Transport bounds are supplied by the existing selected reader profile and
// whole verifier deadline. No request performs compilation or disclosure admission.
export async function compareNativePackets({nativeBase, workerBase, profile, deadline, maximumResponseBytes, dataRevision, sourceRevision, compareRawPackets}) {
  const remaining = () => {
    const value = deadline - Date.now();
    if (value <= 0) throw new Error('public D1 verification deadline exceeded');
    return value;
  };
  async function fetchPacket(base, path, body, expectedMedia = 'application/json') {
    const response = await fetch(base + path, {redirect: 'error', signal: AbortSignal.timeout(Math.min(30_000, remaining())),
      ...(body === undefined ? {} : {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)})});
    if (response.status !== 200) {
      // Preserve the same refused response before lifecycle cleanup. This is
      // diagnostic custody only: no extra request or response acceptance.
      const cap = Math.min(maximumResponseBytes, 8192);
      const chunks = []; let count = 0, complete = false;
      if (response.body) {
        const reader = response.body.getReader();
        try {
          for (;;) {
            remaining(); const {done, value} = await reader.read();
            if (done) {complete = true; break;}
            const kept = value.subarray(0, Math.max(0, cap - count));
            chunks.push(kept); count += kept.byteLength;
            if (value.byteLength > kept.byteLength || count === cap) break;
          }
        } finally {await reader.cancel();}
      }
      const diagnostic = {schema: 'tos_native_packet_refusal_v1', path,
        status: response.status, retained_bytes: count, complete,
        body_base64: Buffer.concat(chunks, count).toString('base64')};
      throw new Error(`${path} returned HTTP ${response.status} ${JSON.stringify(diagnostic)}`);
    }
    const media = response.headers.get('content-type')?.split(';')[0].trim().toLowerCase();
    if (media !== expectedMedia) throw new Error(`${path} returned wrong content type: ${media}`);
    if (!response.body) throw new Error(`${path} response body absent`);
    const reader = response.body.getReader();
    const chunks = []; let count = 0;
    try {
      for (;;) {
        remaining(); const {done, value} = await reader.read(); if (done) break;
        count += value.byteLength;
        if (count > maximumResponseBytes) throw new Error(`${path} exceeds selected reader response bound`);
        chunks.push(value);
      }
    } finally { await reader.cancel(); }
    return Buffer.concat(chunks, count);
  }
  async function request(base, path, body, jsonl = false) {
    return fetchPacket(base, path, body, jsonl ? 'application/x-ndjson' : 'application/json');
  }
  const observed = raw => {
    const value = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(raw));
    if (!value || Array.isArray(value) || typeof value !== 'object') throw new Error('transport observation requires an object');
    return value;
  };
  // Transport only: all domain comparison, shape/UTF8/duplicates/root strings,
  // ordered keys and numeric kinds/lexemes belong to the native FND owner.
  async function compare(label, actual, expected, shape = 'json') {
    await compareRawPackets({label, actual, expected, shape});
  }
  const node = profile === 'representative' ? 'philosophy:a' : 'philosophy:philosophy.atlas';
  const packet = await request(nativeBase, `/api/knowledge/nodes/${quote(node)}?relation_limit=20`);
  const catalog = await request(nativeBase, '/api/knowledge/catalog');
  const nodeObservation = observed(packet), catalogObservation = observed(catalog);
  assert.ok(typeof nodeObservation.source_revision === 'string' && nodeObservation.source_revision, 'native oracle knowledge revision is absent');
  assert.equal(catalogObservation.source_revision, sourceRevision, 'native oracle differs from held source capture');
  assert.equal(nodeObservation.source_revision, catalogObservation.source_revision, 'native oracle knowledge revision changed');
  assert.ok(nodeObservation.related_relations?.length, `knowledge parity node has no relation: ${node}`);
  const relation = String(nodeObservation.related_relations[0].id);
  assert.equal(observed(await request(workerBase, '/health')).data_revision, dataRevision, 'local D1 imported revision differs');
  for (const [label, path] of nativeVerificationCases(profile, relation)) {
    await compare(`Cloudflare contract drift for ${label}`, await request(workerBase, path), await request(nativeBase, path));
    console.log(`ok ${profile}: ${label}`);
  }
  const representative = {schema_version: 'tos_lens_spec_v1', lens_id: 'relations-first', sources: ['philosophy'],
    node_query: {enabled: false, match: 'all', filters: []}, relation_query: {match: 'all', filters: [{field: 'predicate_id', op: 'eq', value: 'relates'}]},
    composition: {endpoint_policy: 'independent'}, limits: {nodes: 10, relations: 10, groups: 10}};
  const arbitrary = {schema_version: 'tos_lens_spec_v1', lens_id: 'edge-contract-smoke', sources: ['philosophy'],
    node_query: {enabled: false}, relation_query: {filters: [{field: 'predicate_id', op: 'eq', value: 'uses_script'}]},
    composition: {endpoint_policy: 'independent'}, limits: {nodes: 20, relations: 10, groups: 10}};
  const lenses = profile === 'representative' ? [representative] : [arbitrary, {...arbitrary, detail: 'compact'},
    {schema_version: 'tos_lens_spec_v1', lens_id: 'source-scope-parity', sources: ['source-claims', 'semantic-interchange'],
      seed: {focus_node_id: 'tos.work.friedrich-nietzsche.also-sprach-zarathustra'}, node_query: {enabled: false}, traversal: {depth: 2, profile: 'all'}},
    {schema_version: 'tos_lens_spec_v1', lens_id: 'edge-null-filter-smoke', sources: ['philosophy'],
      node_query: {filters: [{field: 'attributes.missing_contract_probe', op: 'in', value: [null]}]},
      relation_query: {enabled: false}, limits: {nodes: 3, relations: 0, groups: 3}}];
  for (const spec of lenses) {
    await compare(`Cloudflare lens drift: ${spec.lens_id}/${spec.detail ?? 'full'}`, await request(workerBase, '/api/knowledge/lenses/compile', spec),
      await request(nativeBase, '/api/knowledge/lenses/compile', spec));
    console.log(`ok: compiled lens ${spec.lens_id}/${spec.detail ?? 'full'}`);
  }
  if (profile === 'representative') {
    assert.equal(observed(await request(nativeBase, '/api/knowledge/catalog')).source_revision, sourceRevision, 'native oracle source changed during verification');
    console.log('representative software profile only; production and scale coverage remains outstanding'); return;
  }
  const scale = 'view_id=chronology&layers=evidence-relation%2Chistorical-relation';
  const manifestPath = `/api/philosophy/scale-export/manifest?${scale}`;
  await compare('scale manifest', await request(workerBase, manifestPath), await request(nativeBase, manifestPath));
  for (const table of ['nodes', 'edges', 'clusters', 'cluster-node-memberships', 'cluster-edge-memberships']) {
    const path = `/api/philosophy/scale-export/${table}.jsonl?${scale}`;
    await compare(`scale export ${table}`, await request(workerBase, path, undefined, true), await request(nativeBase, path, undefined, true), 'jsonl');
    console.log(`ok: scale export ${table}`);
  }
  const emptyPath = '/api/philosophy/scale-export/manifest?view_id=chronology&layers=__tos_none__';
  const empty = await request(workerBase, emptyPath);
  await compare('empty scale filter', empty, await request(nativeBase, emptyPath));
  assert.ok(Object.values(observed(empty).tables).every(descriptor => descriptor.row_count === 0), 'explicit empty layer filter');
  const csvPath = `/api/philosophy/scale-export/nodes.csv?${scale}`;
  const actualCsv = await fetchPacket(workerBase, csvPath, undefined, 'text/csv');
  const expectedCsv = await fetchPacket(nativeBase, csvPath, undefined, 'text/csv');
  assert.ok(actualCsv.subarray(0, actualCsv.indexOf(10) < 0 ? actualCsv.length : actualCsv.indexOf(10)).toString('utf8').trim(), 'CSV download header');
  await compare('scale CSV full packet drift', actualCsv, expectedCsv, 'csv');
  assert.equal(observed(await request(nativeBase, '/api/knowledge/catalog')).source_revision, sourceRevision, 'native oracle source changed during verification');
  console.log('ok: scale export CSV and empty filter');
}
