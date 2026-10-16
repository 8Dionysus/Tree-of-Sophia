#!/usr/bin/env node
// The real durable reader and storage path, compared with its direct oracle.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {installReadingRules} from '../../../../access/web/src/observatory/reading-resume-rust.mjs';
import {installHumanFormRules} from '../../../../access/web/src/observatory/human-form-rules.mjs';
import {readReading,READING_KEY,validateReading} from '../../../../access/web/src/observatory/reading-resume.mjs';
import {readingKey} from '../../../../access/web/src/observatory/reader-model.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if((bindingPath&&!wasmPath)||(!bindingPath&&wasmPath))throw new Error('usage: node --experimental-strip-types reading-resume-wasm-host.mjs [BINDING.mjs MODULE_bg.wasm]');
const hash='a'.repeat(64),other='b'.repeat(64),id='claim\ud800',key=readingKey('node',id);
const claim={claimId:id,pathId:'path',relationType:'supports',nodeIds:['subject',id,'object'],
  relationIds:['left','right'],detailRelationIds:['detail'],closureNodeIds:['subject',id,'object'],ignored:'discard'};
const packet={v:1,activeKey:key,entries:[
  {kind:'node',id,sourceRevision:hash,contentRevision:other,preferred:'default',claimReference:claim,
    positions:[[JSON.stringify([key,hash,other,'default']),{top:-0,details:[['sources',true,'discard'],['identity',false]],
      anchor:{key:'form:statement:context:12:part:3',offset:-0,ignored:'discard'}}]],ignored:'discard'},
  {kind:'relation',id:'relation:1',sourceRevision:hash,contentRevision:other,preferred:'default',positions:[]},
]};
const oracle=validateReading(structuredClone(packet));
assert.equal(JSON.stringify(oracle).includes('discard'),false);
assert.equal(Object.is(oracle.entries[0].positions[0][1].top,-0),true);
const invalid=[
  {...packet,activeKey:'foreign'},
  {...packet,entries:[packet.entries[0],packet.entries[0]]},
  {...packet,entries:[{...packet.entries[0],positions:[[JSON.stringify([key,hash,other,'default']),
    {...packet.entries[0].positions[0][1],anchor:{key:'description:1:part:2',offset:0}}]]},packet.entries[1]]},
  {...packet,entries:[{...packet.entries[0],claimReference:{...claim,relationIds:['left','left']}},packet.entries[1]]},
];
for(const value of invalid)assert.throws(()=>validateReading(structuredClone(value)));

if(bindingPath){
  const rules=await import(pathToFileURL(bindingPath).href);
  await rules.default({module_or_path:await readFile(wasmPath)});
  installHumanFormRules(rules);
  const originalLanguage={...packet,entries:[{...packet.entries[0],preferred:'original'},packet.entries[1]]};
  const originalOracle=validateReading(structuredClone(originalLanguage));
  installReadingRules(rules);
  assert.deepEqual(validateReading(structuredClone(packet)),oracle);
  assert.deepEqual(validateReading(structuredClone(originalLanguage)),originalOracle);
  for(const value of invalid)assert.throws(()=>validateReading(structuredClone(value)));
  const stored=new Map([[READING_KEY,JSON.stringify(packet)]]);
  const storage={getItem:name=>stored.get(name)??null};
  assert.deepEqual(readReading(storage),JSON.parse(JSON.stringify(oracle)));
  assert.deepEqual(stored.get(READING_KEY),JSON.stringify(packet));
}
console.log(JSON.stringify({status:'pass',reading_cases:invalid.length+1,actual_wasm:!!bindingPath,legacy_oracle:true}));
