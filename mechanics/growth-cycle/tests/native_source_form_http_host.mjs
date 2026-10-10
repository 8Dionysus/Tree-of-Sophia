/** Real maintained browser form session against the installed native HTTP owner.
 * Synthetic fixture selection is private; no credential or source value is logged.
 */
import assert from 'node:assert/strict';
import {readFile,stat} from 'node:fs/promises';
import {createHash,webcrypto} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {createSourceCommandClient} from '../../../access/web/constructor/source-command-client.mjs';
import {installSourceFormRules,createSourceFormSession} from '../../../access/web/constructor/source-form-session-rust.mjs';

globalThis.crypto??=webcrypto;
const [packetPath]=process.argv.slice(2);
assert.equal(process.argv.length,3);
const info=await stat(packetPath);assert.ok(info.isFile()&&(info.mode&0o077)===0&&info.size<16384);
const packet=JSON.parse(await readFile(packetPath,'utf8'));
assert.equal(packet.schema_version,'tos_native_http_form_host_fixture_v1');
const binding=await readFile(packet.binding),wasm=await readFile(packet.wasm);
assert.equal(createHash('sha256').update(binding).digest('hex'),packet.binding_sha256);
assert.equal(createHash('sha256').update(wasm).digest('hex'),packet.wasm_sha256);
const rules=await import(pathToFileURL(packet.binding).href);
await rules.default({module_or_path:wasm});installSourceFormRules(rules);
let loseResponse=true,applies=0;const requests=[];
const client=createSourceCommandClient({origin:packet.origin,token:packet.token,fetchImpl:async(url,options)=>{
  const request=options.body?JSON.parse(options.body):null;
  // A browser supplies this page origin automatically; Node's fetch does not.
  const response=await fetch(url,{...options,headers:{...options.headers,Origin:packet.browser_origin}});
  assert.equal(response.headers.get('access-control-allow-origin'),packet.browser_origin);
  if(request?.operation==='apply'){
    applies++;requests.push(options.body);
    if(loseResponse){loseResponse=false;await response.arrayBuffer();throw new Error('synthetic response loss after owner completion');}
  }
  return response;
}});
const session=createSourceFormSession(client,{commandId:()=> 'synthetic-native-http-browser-form'});
try{
  await session.describe();
  const prepared=await session.prepare({formId:packet.form_id,fieldId:'metadata.source-note'});
  const preview=prepared.current.prepared_materialization;assert.equal(preview.state,'ready');assert.equal(typeof preview.display_text,'string');
  assert.equal(preview.form.id,prepared.prepared.form.form_id);assert.equal(preview.form.version,prepared.prepared.form.form_version);
  await assert.rejects(session.commit());assert.equal(session.state().uncertain,true);
  const retained=session.retainedCommand();assert.equal(typeof retained.expected_dependencies,'string');assert.match(retained.expected_dependencies,/^sha256:[a-f0-9]{64}$/);
  const result=await session.commit();assert.equal(result.uncertain,false);assert.equal(result.result.replayed,true);
  assert.equal(result.result.receipt.command_id,retained.command_id);assert.equal(result.result.grants_admission,false);
  assert.equal(applies,2);assert.equal(requests[0],requests[1]);
  console.log(JSON.stringify({schema_version:'tos_native_http_browser_form_observation_v1',success:true,real_owner_http:true,response_loss_after_completion:true,exact_pending_replay:true,owner_dependencies_retained:true,readable_owner_preview:true,exact_browser_origin:true,grants_admission:false}));
}finally{session.dispose();client.close();}
