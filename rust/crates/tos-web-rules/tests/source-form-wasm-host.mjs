#!/usr/bin/env node
// New browser source-form path only; the owner command is a mocked I/O seam.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {installSourceFormRules,createSourceFormSession} from '../../../../access/web/constructor/source-form-session-rust.mjs';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node source-form-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const rules=await import(pathToFileURL(bindingPath).href);
await rules.default({module_or_path:await readFile(wasmPath)});
installSourceFormRules(rules);

const context={schema_version:'tos_local_source_command_result_v1',command_operations:['describe','prepare','apply'],
  source_fields:[{field_id:'metadata.preferred-name'}],allowed_form_ids:['tos.form.example'],
  source:{id:'tos.work.example',version:2,digest:'sha256:'+'a'.repeat(64)},revision:'sha256:before',owner_configuration:'sha256:grant'};
const prepared={...context,revision:'sha256:prepared',prepared_change:{operation:'form.revise',
  form:{form_id:'tos.form.example',content:{kind:'source-copy',slot:'name'}}}};
let fail=true,ids=0,describes=0;const applies=[];
const client={execute:async request=>{
  if(request.operation==='describe'){describes++;return context;}
  if(request.operation==='prepare')return prepared;
  applies.push(request);
  if(fail)throw new Error('delivery outcome unknown');
  return {schema_version:context.schema_version,receipt:{command_id:request.command_id},grants_admission:false};
}};
const session=createSourceFormSession(client,{commandId:()=>`command-${++ids}`});
await session.describe();
await assert.rejects(session.prepare({formId:'guessed',fieldId:'metadata.preferred-name'}));
await session.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'});
await assert.rejects(session.commit());
assert.equal(session.state().uncertain,true);
assert.equal(session.retainedCommand().expected_revision,'sha256:prepared');
assert.deepEqual(session.retainedCommand().expected_source,context.source);
await assert.rejects(session.describe());assert.equal(describes,1,'pending describe refuses before owner I/O');
await assert.rejects(session.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'}));
fail=false;const result=await session.commit();
assert.deepEqual(applies[0],applies[1]);
assert.equal(ids,1);
assert.equal(result.result.grants_admission,false);
assert.equal(result.uncertain,false);
const retained=session.retainedCommand();retained.command_id='tampered';
assert.equal(session.retainedCommand().command_id,'command-1');
let oversizedSends=0;
const oversized=createSourceFormSession({execute:async request=>{
  if(request.operation==='describe')return context;
  if(request.operation==='prepare')return {...prepared,prepared_change:{...prepared.prepared_change,
    form:{...prepared.prepared_change.form,content:{kind:'source-copy',slot:'x'.repeat(1_100_000)}}}};
  oversizedSends++;throw new Error('must not dispatch');
}},{commandId:()=> 'oversized-command'});
await oversized.describe();await oversized.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'});
await assert.rejects(oversized.commit());
assert.equal(oversizedSends,0,'unencodable command never dispatched');
assert.equal(oversized.state().uncertain,false);
assert.equal(oversized.retainedCommand(),null);
oversized.dispose();
session.dispose();
console.log(JSON.stringify({status:'pass',source_form_browser_cases:9,owner_write:false,old_cases_repeated:false}));
