#!/usr/bin/env node
// The installed browser validator, measured against the retained direct oracle.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {installClaimReferenceRules} from '../../../../access/web/src/observatory/claim-reference-rust.mjs';
import {validateClaimReference} from '../../../../access/web/src/observatory/knowledge-client.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node --experimental-strip-types claim-reference-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const valid={claimId:'claim\ud800',pathId:'path',relationType:'supports',nodeIds:['subject','claim\ud800','object'],
  relationIds:['left','right'],detailRelationIds:['detail'],closureNodeIds:['subject','claim\ud800','object'],wording:'discard'};
const oracle=validateClaimReference(valid,'claim\ud800');
const binding=await import(pathToFileURL(bindingPath).href);
await binding.default({module_or_path:await readFile(wasmPath)});
installClaimReferenceRules(binding);
assert.deepEqual(validateClaimReference(valid,'claim\ud800'),oracle);
assert.equal(JSON.stringify(oracle).includes('wording'),false);
for(const bad of [
  [{...valid,relationIds:['left','left']},'claim\ud800'],
  [{...valid,closureNodeIds:['subject','object']},'claim\ud800'],
  [valid,'other'],
])assert.throws(()=>validateClaimReference(...bad));
console.log(JSON.stringify({status:'pass',claim_reference_cases:4,actual_wasm:true,legacy_oracle:true}));
