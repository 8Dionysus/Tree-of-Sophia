#!/usr/bin/env node
// Exercise the installed saved-condition rule through both lens and path reads.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {validateConditions} from '../../../../access/web/src/observatory/lens-conditions.mjs';
import {installConditionRules} from '../../../../access/web/src/observatory/lens-conditions-rust.mjs';
import {validateDraft,readSaved,SAVED_LENSES_KEY} from '../../../../access/web/src/observatory/lens-model.mjs';
import {validatePathDraft} from '../../../../access/web/src/observatory/lens-path-editor.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node conditions-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const node={selector:'property_id',id:'tos.property.opaque',op:'in',value:[-0,false,'\ud800']};
const relation={selector:'field',id:'display.label.default',op:'contains',value:'А\u0301'};
const sparse=[,'kept'];
const valid=[
  {nodes:[node,{...node,id:'same',value:sparse},{...node,id:'same',value:null}],relations:[relation]},
  {nodes:[],relations:[]},
  {nodes:[{selector:'field',id:'😀'.repeat(128),op:'eq',value:'😀'.repeat(512)}],relations:[]},
];
const invalid=[
  {nodes:Array(13).fill(node),relations:[]},
  {nodes:[{...node,value:Infinity}],relations:[]},
  {nodes:[{...node,value:[undefined]}],relations:[]},
  {nodes:[],relations:[node]},
  {nodes:[],relations:[],unknown:'must reject'},
];
const oracle=valid.map(value=>validateConditions(structuredClone(value)));
for(const value of invalid)assert.throws(()=>validateConditions(structuredClone(value)));
const path32={nodes:Array(32).fill(relation),relations:[]};
const pathOracle=validateConditions(path32,{maxConditions:32});
const draft={v:2,name:'Моя линза',scope:'all',sources:['knowledge'],nodeIds:[],focusId:null,query:'мысль',kinds:[],predicates:[],
  depth:0,direction:'either',profile:'all',limit:20,relations:true,conditions:valid[0]};
const draftOracle=validateDraft(structuredClone(draft));
const saved=JSON.stringify([draft]),savedOracle=validateDraft(JSON.parse(saved)[0]);
const path=[{pathId:'path-1',steps:[{nodeQuery:{conditions:[node]},relationQuery:{conditions:[relation]}}]}];
const pathExpected=validatePathDraft(structuredClone(path));

const rules=await import(pathToFileURL(bindingPath).href);
await rules.default({module_or_path:await readFile(wasmPath)});
installConditionRules(rules);
for(let index=0;index<valid.length;index++)assert.deepEqual(validateConditions(structuredClone(valid[index])),oracle[index],`conditions ${index}`);
for(const value of invalid)assert.throws(()=>validateConditions(structuredClone(value)));
assert.deepEqual(validateConditions(path32,{maxConditions:32}),pathOracle,'path limit 32');
assert.deepEqual(validateDraft(structuredClone(draft)),draftOracle,'actual lens draft');
assert.deepEqual(validatePathDraft(structuredClone(path)),pathExpected,'actual path draft');
const storage={getItem:key=>key===SAVED_LENSES_KEY?saved:null};
assert.deepEqual(readSaved(storage),[savedOracle],'actual saved lens reader');
assert.equal(storage.getItem(SAVED_LENSES_KEY),saved,'reader leaves storage unchanged');
console.log(JSON.stringify({status:'pass',condition_cases:valid.length+invalid.length+1,
  actual_consumers:['validateDraft','validatePathDraft','readSaved'],storage_write:false}));
