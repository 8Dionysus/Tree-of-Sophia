#!/usr/bin/env node
// Compare the generated Rust transition binding with the maintained TS workspace.
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { createResearchWorkspace } from '../../../../access/web/src/research-workspace.ts';

const [bindingPath, wasmPath] = process.argv.slice(2);
if (!bindingPath || !wasmPath) throw new Error('usage: node --experimental-strip-types workspace-machine-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const binding = await import(pathToFileURL(bindingPath).href);
const wasm = await readFile(wasmPath);
const before = process.memoryUsage();
const start = performance.now();
await binding.default({ module_or_path: wasm });
const startupMs = performance.now() - start;
const encode = new TextEncoder();
const decode = new TextDecoder();
if(process.argv.includes('--browser-session-only')){
  const {installBrowserWorkspaceMachine,createBrowserResearchWorkspace}=await import('../../../../access/web/src/research-workspace-rust.ts');
  installBrowserWorkspaceMachine(binding.BrowserWorkspaceSession);
  const saved=[];
  const browser=createBrowserResearchWorkspace({sessionId:'browser',historyLimit:3,
    persistence:{load:()=>null,save:value=>saved.push(value)}});
  const oracle=createResearchWorkspace({sessionId:'browser',historyLimit:3,persistence:false});
  const compare=label=>{assert.equal(browser.exportPacket(),oracle.exportPacket(),label);
    assert.deepEqual(browser.summary(),oracle.summary(),`${label}/summary`);};
  compare('create');
  const unsubscribe=browser.subscribe(state=>{state.notes[0].body='listener edit';});
  const returned=browser.addNote({id:'note:one',body:'  First  '});
  assert.equal(returned.notes[0].body,'First','returned state is independent of listener snapshot');
  unsubscribe();oracle.addNote({id:'note:one',body:'  First  '});compare('note');
  const read=browser.getState();read.notes[0].body='caller edit';
  assert.equal(browser.getState().notes[0].body,'First','cached Rust projection is copy-on-read');
  browser.addHypothesis({id:'hyp:one',title:'Reading',body:'Local reading.',targetId:'node:a'});
  oracle.addHypothesis({id:'hyp:one',title:'Reading',body:'Local reading.',targetId:'node:a'});compare('hypothesis');
  const proposal={id:'proposal:one',kind:'interpretation',parentHypothesisId:'hyp:one',targetId:'node:a',
    statement:'Another reading.',sourceRefs:['source:one'],evidenceRefs:['evidence:one'],
    confidencePosture:{value:'low',meaning:'maker_declared_uncertainty_not_truth_probability'},
    actorOrigin:'human',basePageRevision:3,baseWorkspaceRevision:browser.summary().revision,
    dataFingerprint:'sha256:fixture',createdAt:'2026-09-23T12:00:00.000Z'};
  assert.deepEqual(browser.stageProposal(proposal),oracle.stageProposal(proposal));compare('proposal');
  assert.equal(browser.undo(),oracle.undo());compare('undo');
  assert.equal(browser.redo(),oracle.redo());compare('redo');
  const before=browser.exportPacket();
  assert.throws(()=>browser.importPacket('{"schema":"bad"}'));
  assert.equal(browser.exportPacket(),before,'failed import atomic');
  assert.equal(saved.at(-1),before,'persisted packet');
  assert.equal(browser.persistenceError(),null);
  const padded=JSON.parse(before);
  padded.session_id='  padded  ';
  padded.hypotheses[0].id='  hyp:one  ';
  padded.proposals[0].parent_hypothesis_id='  hyp:one  ';
  padded.notes[0].id='  note:one  ';
  browser.importPacket(JSON.stringify(padded));oracle.importPacket(JSON.stringify(padded));compare('normalized-import-ids');
  if('dispose' in browser)browser.dispose();
  console.log(JSON.stringify({status:'pass',host:`Node ${process.version} WebAssembly`,
    browser_session_cases:7,independent_ts:true,old_host_cases_repeated:false,
    js_bytes:(await stat(bindingPath)).size,wasm_bytes:(await stat(wasmPath)).size,
    startup_ms:Number(startupMs.toFixed(3))}));
  process.exit(0);
}
const schema = 'tos_research_workspace_transition_v1';
function call(operation, machine, extra = {}) {
  const result = binding.workspace_transition_wasm_v1(encode.encode(JSON.stringify({ schema, operation, ...(machine && { machine }), ...extra })));
  try { return result.ok() ? { value: JSON.parse(decode.decode(result.bytes())) } : { error: result.error_code() }; }
  finally { result.free(); }
}
const ts = createResearchWorkspace({ sessionId: 'host-oracle', historyLimit: 3, persistence: false });
let machine = call('create', null, { session_id: 'host-oracle', history_limit: 3 }).value.machine;
let cases = 0;
function parity(label) {
  const result = call('export', machine);
  assert.ok(result.value, `${label}/export: ${result.error}`);
  assert.equal(result.value.value, ts.exportPacket(), `${label}/packet`);
  const summary = call('summary', machine);
  assert.deepEqual(summary.value.value, ts.summary(), `${label}/summary`);
  cases++;
}
function apply(label, command, tsAction, changed = true) {
  tsAction();
  const result = call('apply', machine, { command });
  assert.ok(result.value, `${label}/apply: ${result.error}`);
  assert.equal(result.value.value.changed, changed, `${label}/changed`);
  machine = result.value.machine;
  parity(label);
}
parity('create');
apply('select-lens', { kind: 'lens.select', selection: { id: 'node:1', kind: 'node', label: 'First' } }, () => ts.selectLens({ id: 'node:1', kind: 'node', label: 'First' }));
apply('edge-exclude', { kind: 'edge.exclude', edge_id: 'edge:a' }, () => ts.excludeEdge('edge:a'));
apply('edge-repeat', { kind: 'edge.exclude', edge_id: 'edge:a' }, () => ts.excludeEdge('edge:a'), false);
apply('edge-include', { kind: 'edge.include', edge_id: 'edge:a' }, () => ts.includeEdge('edge:a'));
apply('hypothesis-add', { kind: 'hypothesis.add', hypothesis: { id: 'hyp:1', title: 'Reading', body: 'Text', posture: { session_hypothesis: true, source: false, reviewed: false, canon: false } } }, () => ts.addHypothesis({ id: 'hyp:1', title: 'Reading', body: 'Text' }));
const route1 = { id: 'route:1', label: 'One', from_id: 'node:1', to_id: 'node:2', node_ids: ['node:1', 'node:2'], edge_ids: ['edge:a'] };
const route2 = { ...route1, id: 'route:2', label: 'Two' };
function tsRoute(route) { return { id: route.id, label: route.label, fromId: route.from_id, toId: route.to_id, nodeIds: route.node_ids, edgeIds: route.edge_ids }; }
apply('route-one', { kind: 'route.snapshot', route: route1 }, () => ts.saveRouteSnapshot(tsRoute(route1)));
apply('route-two', { kind: 'route.snapshot', route: route2 }, () => ts.saveRouteSnapshot(tsRoute(route2)));
assert.equal(call('comparable_routes_ready', machine).value.value, ts.comparableRoutesReady(), 'comparable routes');
apply('note-add', { kind: 'note.add', note: { id: 'note:1', body: 'First', target_id: 'node:1' } }, () => ts.addNote({ id: 'note:1', body: 'First', targetId: 'node:1' }));
apply('note-update', { kind: 'note.update', note: { id: 'note:1', body: 'Second' } }, () => ts.updateNote({ id: 'note:1', body: 'Second' }));
const undone = call('undo', machine);
assert.equal(undone.value.value, ts.undo(), 'undo');
machine = undone.value.machine;
parity('undo');
const redone = call('redo', machine);
assert.equal(redone.value.value, ts.redo(), 'redo');
machine = redone.value.machine;
parity('redo');
apply('note-remove', { kind: 'note.remove', id: 'note:1' }, () => ts.removeNote('note:1'));
apply('route-remove', { kind: 'route.remove', id: 'route:2' }, () => ts.removeRouteSnapshot('route:2'));
apply('hypothesis-remove', { kind: 'hypothesis.remove', id: 'hyp:1' }, () => ts.removeHypothesis('hyp:1'));
assert.equal(call('clear_history', machine).value.value, null);
machine = call('clear_history', machine).value.machine;
ts.clearHistory();
parity('clear-history');
const packet = ts.exportPacket();
const imported = call('import', machine, { packet });
assert.ok(imported.value, `import: ${imported.error}`);
machine = imported.value.machine;
ts.importPacket(packet);
parity('import');
assert.equal(call('import', machine, { packet: '{"schema":"bad"}' }).error, 'invalid_packet');
// Saved packet strings follow the maintained importer normalization in Rust.
const padded = JSON.parse(packet);
padded.session_id = '  padded  ';
const tsImport = createResearchWorkspace({ sessionId: 'padded', persistence: false });
assert.equal(tsImport.importPacket(JSON.stringify(padded)), true);
const normalizedImport = call('import', machine, { packet: JSON.stringify(padded) });
assert.ok(normalizedImport.value,`normalized import: ${normalizedImport.error}`);
assert.equal(call('export', normalizedImport.value.machine).value.value,tsImport.exportPacket());
const colliding = createResearchWorkspace({ sessionId: 'pair-collision', persistence: false });
colliding.saveRouteSnapshot({ id: 'r1', label: 'a', fromId: 'a\u0000b', toId: 'c', nodeIds: [], edgeIds: [] });
colliding.saveRouteSnapshot({ id: 'r2', label: 'b', fromId: 'a', toId: 'b\u0000c', nodeIds: [], edgeIds: [] });
assert.equal(colliding.comparableRoutesReady(), true);
const collidingPacket = colliding.exportPacket();
const collidingMachine = call('import', machine, { packet: collidingPacket });
assert.ok(collidingMachine.value, `collision import: ${collidingMachine.error}`);
assert.equal(call('comparable_routes_ready', collidingMachine.value.machine).value.value, false);
const stageTs = createResearchWorkspace({ sessionId: 'proposal-oracle', persistence: false });
let stageMachine = call('create', null, { session_id: 'proposal-oracle' }).value.machine;
stageTs.addHypothesis({ id: 'hyp:stage', title: 'Reading', body: 'Local reading.' });
stageMachine = call('apply', stageMachine, { command: { kind: 'hypothesis.add', hypothesis: JSON.parse(stageTs.exportPacket()).hypotheses[0] } }).value.machine;
assert.equal(call('export', stageMachine).value.value, stageTs.exportPacket(), 'proposal hypothesis packet');
stageTs.stageProposal({ id: 'proposal:stage', kind: 'interpretation', parentHypothesisId: 'hyp:stage', targetId: 'edge:stage',
  statement: 'Another reading.', sourceRefs: ['source:one'], evidenceRefs: ['evidence:one'],
  confidencePosture: { value: 'low', meaning: 'maker_declared_uncertainty_not_truth_probability' },
  actorOrigin: 'agent', basePageRevision: 7, baseWorkspaceRevision: 1, dataFingerprint: 'sha256:fixture',
  createdAt: '2026-09-23T12:00:00.000Z' });
const unsignedProposal = { ...JSON.parse(stageTs.exportPacket()).proposals[0] };
delete unsignedProposal.digest;
const staged = call('apply', stageMachine, { command: { kind: 'proposal.stage', proposal: unsignedProposal } });
assert.ok(staged.value, `proposal stage: ${staged.error}`);
assert.equal(call('export', staged.value.machine).value.value, stageTs.exportPacket(), 'proposal stage packet');
cases++;
const after = process.memoryUsage();
console.log(JSON.stringify({ status: 'pass', host: `Node ${process.version} WebAssembly`, packet_summary_cases: cases, independent_ts: true,
  measured_nonparity_cases: 1,
  js_bytes: (await stat(bindingPath)).size, wasm_bytes: (await stat(wasmPath)).size,
  startup_ms: Number(startupMs.toFixed(3)), rss_before_bytes: before.rss, rss_after_bytes: after.rss }));
