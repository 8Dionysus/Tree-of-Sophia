#!/usr/bin/env node
// The real constructor and IndexedDB-facing module with the matched WASM rule.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {register} from 'node:module';
import {pathToFileURL} from 'node:url';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(bindingPath!=='--oracle-only'&&(!bindingPath||!wasmPath))
  throw Error('usage: node --experimental-strip-types live-resume-wasm-host.mjs [--oracle-only | BINDING.mjs MODULE_bg.wasm]');
register(new URL('./live-resume-ts-loader.mjs',import.meta.url));
const [{installLiveResumeRules,makeLiveResume,validateLiveResume,resolveLiveResumeSelection,createLiveResumeStore},
  {StableExplorationLayout},{compactFormLens},{pageFixture}]=await Promise.all([
  import('../../../../access/web/constructor/live-resume.mjs'),
  import('../../../../access/web/constructor/live-model.mjs'),
  import('../../../../access/web/fixtures/human-form-data.mjs'),
  import('../../../../access/web/src/observatory/exploration-test-fixtures.mjs'),
]);
const view=compactFormLens();
const path=view.scene.compact.claim_paths[0],raw=view.nodes.find(item=>item.id===path.claim_node_id);
const query=pageFixture().query;query.origin={kind:'node',id:raw.id,content_revision:raw.content_revision};
view.contexts=[{query}];
const selection={kind:'claim-path',id:path.id,claimId:path.claim_node_id};
const presentation={layout:new StableExplorationLayout().capture(),
  pose:{v:1,yaw:0,pitch:0,zoom:1,pan:[0,0],center:[0,0,0],fit:1},mode:'compact'};
const state={view,selection,areaKind:'exploration'};
const oracle=makeLiveResume(state,presentation);
if(bindingPath==='--oracle-only'){
  assert.deepEqual(resolveLiveResumeSelection(oracle.selection,view),selection);
  console.log(JSON.stringify({status:'pass',direct_source_oracle:true}));
  process.exit(0);
}
const binding=await import(pathToFileURL(bindingPath).href);
await binding.default({module_or_path:await readFile(wasmPath)});
installLiveResumeRules(binding);
const saved=makeLiveResume(state,presentation);
assert.deepEqual(saved,oracle);
assert.deepEqual(validateLiveResume(JSON.parse(JSON.stringify(saved))),oracle);
assert.deepEqual(resolveLiveResumeSelection(saved.selection,view),selection);
const rows=new Map(),indexedDB={open(){
  const db={createObjectStore(){},close(){},transaction(){
    const tx={};tx.objectStore=()=>({
      get(key){const request={result:rows.get(key)};queueMicrotask(()=>tx.oncomplete());return request;},
      put(value,key){rows.set(key,value);const request={result:key};queueMicrotask(()=>tx.oncomplete());return request;},
    });return tx;
  }};
  const request={result:db};queueMicrotask(()=>{request.onupgradeneeded();request.onsuccess();});return request;
}};
const store=createLiveResumeStore({indexedDB});
assert.deepEqual(await store.save(saved),saved);
assert.deepEqual(await store.load(),saved);store.close();
assert.equal(JSON.stringify(saved).includes('source_refs'),false);
assert.throws(()=>validateLiveResume({...saved,cursor:'opaque'}));
assert.throws(()=>validateLiveResume({...saved,sourceRevision:'f'.repeat(64)}));
assert.throws(()=>validateLiveResume({...saved,presentation:{...saved.presentation,mode:'future'}}));
const altered=structuredClone(view);
altered.scene.compact.claim_paths=[];
assert.equal(resolveLiveResumeSelection(saved.selection,altered),null);
assert.equal(resolveLiveResumeSelection(saved.selection,{...view,source_revision:'e'.repeat(64)}),null);
const changed=structuredClone(view);changed.nodes.find(item=>item.id===saved.selection.id).content_revision='f'.repeat(64);
assert.equal(resolveLiveResumeSelection(saved.selection,changed),null);
const reordered=structuredClone(saved.selection);reordered.claimReference.nodeIds.reverse();
assert.equal(resolveLiveResumeSelection(reordered,view),null);
const closure=structuredClone(saved.selection);closure.claimReference.closureNodeIds.reverse();
assert.deepEqual(resolveLiveResumeSelection(closure,view),selection);
const plain=structuredClone(saved.selection);delete plain.claimReference;
assert.deepEqual(resolveLiveResumeSelection(plain,view),{kind:'node',id:plain.id});
console.log(JSON.stringify({status:'pass',actual_binding:true,cases:14,
  boundary:'bounded resume and Claim identity only; source requery, geometry and storage remain in host'}));
