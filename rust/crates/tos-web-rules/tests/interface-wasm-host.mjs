#!/usr/bin/env node
// The real shared reader must return the same local preferences through WASM.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {DEFAULT_INTERFACE,INTERFACE_KEY,readInterface,validateInterface} from '../../../../access/web/src/observatory/interface-model.mjs';
import {installInterfaceRules} from '../../../../access/web/src/observatory/interface-model-rust.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node interface-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const valid=[
  DEFAULT_INTERFACE,
  {v:1,pinned:['reader','search'],dock:'left',text:'large',labels:'large',inputMode:'mouse',sizes:{reader:{width:640,height:620}}},
  {...DEFAULT_INTERFACE,scrollAction:'pan',dragAction:'pan',sensitivity:'fast',motion:'paused',uiLanguage:'es',theme:'light',
    sizes:{settings:{width:430,height:560},unknown:{width:1}},positions:{settings:{x:.3,y:.4}}},
  {...DEFAULT_INTERFACE,sizes:[],positions:{}},
  {...DEFAULT_INTERFACE,positions:{settings:{x:-0,y:0}}},
];
const invalid=[
  {...DEFAULT_INTERFACE,pinned:['reader','reader']},
  {...DEFAULT_INTERFACE,inputMode:'tablet'},
  {...DEFAULT_INTERFACE,motion:'sometimes'},
  {...DEFAULT_INTERFACE,motion:Infinity},
  {...DEFAULT_INTERFACE,sizes:{settings:{width:279,height:560}}},
  {...DEFAULT_INTERFACE,positions:{settings:{x:1.5,y:0}}},
];
const oracle=valid.map(value=>validateInterface(structuredClone(value)));
for(const value of invalid)assert.throws(()=>validateInterface(structuredClone(value)));

const rules=await import(pathToFileURL(bindingPath).href);
await rules.default({module_or_path:await readFile(wasmPath)});
installInterfaceRules(rules);
for(let index=0;index<valid.length;index++){
  const input=structuredClone(valid[index]);
  assert.deepEqual(validateInterface(input),oracle[index],`interface ${index}`);
  const saved=JSON.stringify(input);
  const storage={getItem:key=>key===INTERFACE_KEY?saved:null};
  assert.deepEqual(readInterface(storage),oracle[index],`readInterface ${index}`);
  assert.equal(storage.getItem(INTERFACE_KEY),saved,'read preserves source storage');
}
for(const value of invalid)assert.throws(()=>validateInterface(structuredClone(value)));
console.log(JSON.stringify({status:'pass',interface_cases:valid.length+invalid.length,real_reader:true,storage_write:false}));
