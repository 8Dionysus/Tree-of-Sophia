import {test} from 'vitest';
import assert from 'node:assert/strict';
import {createSourceFormSession} from './source-form-session.mjs';

const context={schema_version:'tos_local_source_command_result_v1',command_operations:['describe','prepare','apply'],
  source_fields:[{field_id:'metadata.preferred-name'}],allowed_form_ids:['tos.form.example'],
  source:{id:'tos.work.example',version:2,digest:'sha256:'+'a'.repeat(64)},revision:'sha256:before',owner_configuration:'sha256:grant'};
const prepared={...context,revision:'sha256:prepared',prepared_change:{operation:'form.revise',
  form:{form_id:'tos.form.example',content:{kind:'source-copy',slot:'name'}}}};

test('owner preparation supplies exact expectations and apply preserves its receipt without admission inference',async()=>{
  const calls=[],client={execute:async request=>{calls.push(request);
    return request.operation==='describe'?context:request.operation==='prepare'?prepared:
      {schema_version:context.schema_version,receipt:{command_id:request.command_id},grants_admission:false};}};
  const session=createSourceFormSession(client,{commandId:()=> 'stable-command'});
  await session.describe();await session.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'});
  const result=await session.commit();assert.equal(result.result.grants_admission,false);
  assert.equal(calls[2].expected_revision,'sha256:prepared');assert.deepEqual(calls[2].expected_source,context.source);
  assert.equal(calls[2].command_id,'stable-command');assert.deepEqual(calls[2].changes,[prepared.prepared_change]);
});

test('unknown delivery retains exact ID and bytes for explicit replay, never creates another command',async()=>{
  let fail=true,ids=0;const calls=[],client={execute:async request=>{
    if(request.operation==='describe')return context;if(request.operation==='prepare')return prepared;
    calls.push(request);if(fail)throw new Error('response lost after possible commit');
    return {schema_version:context.schema_version,receipt:{command_id:request.command_id},replayed:true};}};
  const session=createSourceFormSession(client,{commandId:()=> `command-${++ids}`});
  await session.describe();await session.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'});
  await assert.rejects(session.commit());assert.equal(session.state().uncertain,true);
  assert.equal(calls.length,1);await assert.rejects(session.prepare({formId:'tos.form.example',fieldId:'metadata.preferred-name'}));
  fail=false;await session.commit();assert.deepEqual(calls[0],calls[1]);assert.equal(ids,1);
  const retained=session.retainedCommand();retained.command_id='tampered';assert.equal(session.retainedCommand().command_id,'command-1');
});

test('unsupported owner and guessed field/form cannot create a write',async()=>{
  const unsupported=createSourceFormSession({execute:async()=>({})});await assert.rejects(unsupported.describe());
  let calls=0;const session=createSourceFormSession({execute:async()=>{calls++;return context;}});
  await session.describe();await assert.rejects(session.prepare({formId:'guessed',fieldId:'metadata.preferred-name'}));
  await assert.rejects(session.commit());assert.equal(calls,1);
});
