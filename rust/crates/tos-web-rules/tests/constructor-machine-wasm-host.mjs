#!/usr/bin/env node
// Narrow actual-binding controls for constructor authoring and its live host.
import assert from 'node:assert/strict';
import {readFile,stat} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {createConstructorModel,installConstructorRules} from '../../../../access/web/constructor/model.mjs';
import {createConstructorModel as legacy} from '../../../../access/web/constructor/model-legacy.mjs';
import {installBrowserWorkspaceMachine} from '../../../../access/web/src/research-workspace-rust.ts';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw Error('usage: node --experimental-strip-types constructor-machine-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const binding=await import(pathToFileURL(bindingPath).href);
const wasm=await readFile(wasmPath),startup=performance.now();
await binding.default({module_or_path:wasm});
installConstructorRules(binding);
installBrowserWorkspaceMachine(binding.BrowserWorkspaceSession);
const fingerprint='a'.repeat(64);
const material=(id,kind,parentId)=>({id,kind,parentId,title:{ru:`Материал ${id}`,en:`Material ${id}`},body:{ru:'',en:''},sourceRefs:[{ref:`ToS/source-witnesses/${id}.md`}]});
const library={schema:'tos_constructor_library_v1',fingerprint,rootId:'work',nodes:[material('work','work',null),material('part','part','work'),material('fragment','fragment','part'),
  {id:'demo',kind:'concept',demo:true,parentId:null,title:{ru:'Макет',en:'Mockup'},body:{ru:'Мысль',en:'Thought'},position:[1,2,3],sourceRefs:[{ref:'forged:demo'}]}],
  atlas:{defaultIds:['demo','fragment','part'],edges:[{id:'relation',from:'demo',to:'fragment',kind:'interprets',label:{ru:'Связь',en:'Relation'}}]}};
if(process.argv.includes('--library-order-only')){
  const ordered={schema:library.schema,fingerprint,rootId:'work',nodes:[material('work','work',null),
    ...['zeta','alpha','mu','beta','extra'].map(id=>material(id,'part','work')),
    material('chapter-z','chapter','zeta'),material('chapter-a','chapter','zeta')]};
  const actual=createConstructorModel(ordered,{storage:null}),previous=legacy(ordered,{storage:null});
  actual.seed();previous.seed();
  assert.deepEqual(actual.getState().nodes.map(n=>n.materialId),['work','zeta','alpha','mu','beta']);
  assert.deepEqual(actual.getState().nodes.map(n=>n.materialId),previous.getState().nodes.map(n=>n.materialId));
  assert.deepEqual(actual.expand('material:zeta'),previous.expand('material:zeta'));
  assert.deepEqual(actual.getState().nodes.slice(-2).map(n=>n.materialId),['chapter-z','chapter-a']);
  const cycle=structuredClone(ordered);cycle.nodes.find(n=>n.id==='zeta').parentId='alpha';
  cycle.nodes.find(n=>n.id==='alpha').parentId='zeta';
  assert.throws(()=>createConstructorModel(cycle,{storage:null}),/cycle/);
  const missing=structuredClone(ordered);missing.nodes.find(n=>n.id==='zeta').parentId='absent';
  assert.throws(()=>createConstructorModel(missing,{storage:null}),/parent does not exist/);
  console.log(JSON.stringify({status:'pass',scope:'constructor-library-order',cases:5,actual_wasm:true}));
  process.exit(0);
}
const store=new Map(),storage={getItem:key=>store.get(key)??null,setItem:(key,value)=>store.set(key,value)};
const tree=createConstructorModel(library,{storage,key:'tree'}),oracle=legacy(library,{storage:null});
let cases=0,events=0;tree.subscribe(()=>events++);
const compare=label=>{
  const actual=tree.getState(),expected=oracle.getState();
  for(const node of actual.nodes){const other=expected.nodes.find(item=>item.id===node.id);
    assert.ok(other,`${label}/node ${node.id}`);
    node.position.forEach((value,index)=>assert.ok(Math.abs(value-other.position[index])<1e-9,`${label}/position ${node.id}/${index}`));
    node.position=other.position;
  }
  assert.deepEqual(actual,expected,label);cases++;
};
compare('initial');
const read=tree.getState();read.title='caller mutation';
assert.equal(tree.getState().title,oracle.getState().title,'cached Rust projection is copy-on-read');cases++;
const apply=(label,fn)=>{assert.deepEqual(fn(tree),fn(oracle),`${label}/return`);compare(label);};
apply('atlas',m=>m.seedAtlas());
apply('material',m=>m.addMaterial('work'));
apply('context',m=>m.addDraftWithContext({kind:'question',title:'Что значит?',sourceIds:['material:fragment']},{sourceId:'material:fragment',relationKind:'questions',reverse:true}));
const draft=tree.getState().nodes.find(n=>n.id.startsWith('draft:')).id;
apply('rename',m=>m.rename('Моё дерево'));
apply('move',m=>m.moveNode(draft,[11,-2,3]));
apply('undo',m=>m.undo());apply('redo',m=>m.redo());
const before=tree.exportPacket(),priorEvents=events;
const reordered=JSON.parse(before),reverseFields=Object.fromEntries(Object.entries(reordered).reverse());
assert.equal(tree.importPacket(JSON.stringify(reverseFields)),false);
assert.equal(tree.exportPacket(),before);assert.equal(events,priorEvents);cases++;
const alternateNumber=before.replace('"position":[1,2,3]','"position":[1e0,2e0,3e0]');
assert.notEqual(alternateNumber,before);
assert.equal(tree.importPacket(alternateNumber),false);assert.equal(tree.exportPacket(),before);cases++;
const forged=JSON.parse(before);forged.nodes.find(n=>n.id===draft).canon=true;
assert.throws(()=>tree.importPacket(JSON.stringify(forged)));assert.equal(tree.exportPacket(),before);assert.equal(events,priorEvents);cases++;
forged.nodes.find(n=>n.id===draft).canon=false;forged.libraryFingerprint='b'.repeat(64);
assert.throws(()=>tree.importPacket(JSON.stringify(forged)));assert.equal(tree.exportPacket(),before);cases++;
const research=JSON.parse(tree.exportResearch());
assert.deepEqual(research.proposals,[]);
assert.ok(research.hypotheses.every(h=>h.posture?.session_hypothesis===true&&h.posture?.canon===false));
assert.ok(research.notes.every(n=>!n.body.includes('forged:demo')));cases++;
const badLibrary=structuredClone(library);badLibrary.nodes.find(n=>n.id==='fragment').sourceRefs={ref:'invalid'};
const badTree=createConstructorModel(badLibrary,{storage:null});badTree.addMaterial('fragment');
assert.throws(()=>badTree.exportResearch());cases++;
const fresh=createConstructorModel(library,{storage,key:'tree'});
assert.deepEqual(fresh.getState(),tree.getState());assert.equal(fresh.canUndo(),false);cases++;
const badStorage={getItem:()=>'{"schema":"bad"}',setItem:()=>{throw Error('must not overwrite');}};
const protectedTree=createConstructorModel(library,{storage:badStorage});
assert.match(protectedTree.persistenceError(),/protected/);protectedTree.addMaterial('work');assert.match(protectedTree.persistenceError(),/protected/);cases++;
assert.ok(events>=7);cases++;
tree.dispose();tree.dispose();fresh.dispose();protectedTree.dispose();
console.log(JSON.stringify({status:'pass',constructor_cases:cases,actual_wasm:true,legacy_oracle:true,
  startup_ms:Number((performance.now()-startup).toFixed(2)),wasm_bytes:(await stat(wasmPath)).size}));
