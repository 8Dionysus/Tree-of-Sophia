#!/usr/bin/env node
// Compare the installed Rust pose path through the real saved-place readers.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {validatePose} from '../../../../access/web/src/observatory/view-state.mjs';
import {installPoseRules} from '../../../../access/web/src/observatory/view-state-rust.mjs';
import {capturePlace,readPlaces,savePlace,readResume,RESUME_KEY} from '../../../../access/web/src/observatory/place-model.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node pose-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const pose=()=>({lens:'constellations',yaw:-0,pitch:-.1,zoom:1.3,pan:{x:-0,y:35},selectedId:'opaque:\ud800',relationId:null,
  panelOpen:true,cardTab:'relations',vertices:[{id:'opaque:\ud800',slot:-0,p:[-0,1,2],sourcePosition:[0,-0,2],target:[0,1,-0],volumeZ:-0,sceneText:'discard'}],raw:'discard'});
let unrelated={};for(let i=0;i<140;i++)unrelated={next:unrelated};
const valid=[pose(),{...pose(),lens:'plane',selectedId:'😀'.repeat(512),vertices:[]},{...pose(),lens:'orbits',vertices:[],unrelated}];
const invalid=[
  {...pose(),zoom:Infinity},
  {...pose(),pan:{x:NaN,y:0}},
  {...pose(),vertices:[pose().vertices[0],pose().vertices[0]]},
  {...pose(),vertices:[{...pose().vertices[0],target:[1,2,3,4]}]},
  {...pose(),selectedId:'😀'.repeat(513)},
];
const oracle=valid.map(value=>validatePose(structuredClone(value)));
for(const value of invalid)assert.throws(()=>validatePose(structuredClone(value)));

const rules=await import(pathToFileURL(bindingPath).href);
await rules.default({module_or_path:await readFile(wasmPath)});
installPoseRules(rules);
for(let index=0;index<valid.length;index++)assert.deepEqual(validatePose(structuredClone(valid[index])),oracle[index],`pose ${index}`);
for(const value of invalid)assert.throws(()=>validatePose(structuredClone(value)));

const boundary={is_source:false,is_canon:false,writes_to_tree:false};
const source={schema:'tos_lens_result_v1',source_revision:'a'.repeat(64),authority_boundary:boundary,
  nodes:[{id:'opaque:a',source_refs:['ToS/example'],content_revision:'b'.repeat(64),display:{title:{ru:'A'}}}],relations:[]};
const saved=capturePlace(source,pose(),{name:'Opaque view',id:'place:1',route:'?focus=opaque%3Aa',savedAt:1});
assert.deepEqual(saved.pose,oracle[0]);
const values=new Map(),storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
savePlace(storage,saved);const persisted=JSON.parse(JSON.stringify(saved));assert.deepEqual(readPlaces(storage),[persisted]);
storage.setItem(RESUME_KEY,JSON.stringify(saved));assert.deepEqual(readResume(storage,saved.route),persisted);
const before=new Map(values);
assert.throws(()=>savePlace(storage,{...saved,pose:{...pose(),zoom:Infinity}}));
assert.deepEqual(values,before,'damaged pose never writes storage');
console.log(JSON.stringify({status:'pass',pose_cases:valid.length+invalid.length,real_place_read_save_resume:true,storage_after_rejection:'unchanged'}));
