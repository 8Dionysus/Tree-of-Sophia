#!/usr/bin/env node
// Portable finite recipe observation/binding only. Native products own all rules.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {constants} from 'node:fs';
import {open,realpath,lstat} from 'node:fs/promises';
import {isAbsolute,join,relative} from 'node:path';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const positive = n => Number.isSafeInteger(n) && n > 0;
const read = async (path, cap) => {
  assert.ok(isAbsolute(path) && positive(cap));
  const fd = await open(path, constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);
  try {
    const before = await fd.stat({bigint:true});
    assert.ok(before.isFile() && before.size > 0n && before.size <= BigInt(cap));
    const bytes = Buffer.alloc(Number(before.size));let offset=0;
    while(offset<bytes.length){const {bytesRead}=await fd.read(bytes,offset,bytes.length-offset,null);assert.ok(bytesRead>0);offset+=bytesRead;}
    const after=await fd.stat({bigint:true}),named=await lstat(path,{bigint:true});
    for(const field of ['dev','ino','size','mtimeNs','ctimeNs'])assert.ok(before[field]===after[field]&&before[field]===named[field]);
    return bytes;
  }finally{await fd.close();}
};
const observe = raw => JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(raw),(key,value,context)=>key==='work_deadline_ns'&&typeof value==='number'?context.source:value);
assert.equal(process.argv.length,3,'usage: bind_native_finite_verifier.mjs ABS_BOUND_SELECTION_JSON');
const selected=observe(await read(process.argv[2],65536));
assert.equal(selected.schema,'tos_host_finite_verifier_binding_v1');
assert.ok(['representative','production'].includes(selected.profile));
assert.ok(/^[0-9]+$/.test(selected.work_deadline_ns));
const cutoff=BigInt(selected.work_deadline_ns), began=process.hrtime.bigint();
assert.ok(cutoff>began&&cutoff-began<=BigInt(selected.maximum_seconds)*1000000000n,'same original finite cutoff required');
const current=()=>assert.ok(process.hrtime.bigint()<cutoff,'original cutoff expired during binding');
for(const key of ['native_binary','native_request_path','worker_root','failure_directory','spool_directory','scratch_root','node_exe','node_script','output']){
  assert.ok(isAbsolute(selected[key]),`actual ${key} required`);
  if(key!=='output')assert.equal(await realpath(selected[key]),selected[key]);
}
for(const key of ['maximum_seconds','maximum_response_bytes','maximum_marker_bytes','maximum_source_bytes','maximum_native_state_bytes','maximum_startup_receipt_bytes','maximum_native_log_bytes','maximum_readiness_milliseconds','maximum_shutdown_milliseconds','maximum_inherited_fds'])assert.ok(positive(selected[key]));
assert.ok(selected.stage_ticket_fd>=3&&selected.stage_ticket_fd<selected.maximum_inherited_fds);
assert.equal(process.env.ABYSS_STAGE_TICKET_FD,String(selected.stage_ticket_fd));
const rawManifest=await read(selected.source_input_manifest,262144);
assert.equal(digest(rawManifest),selected.source_input_manifest_sha256);
const manifest=observe(rawManifest);assert.equal(await realpath(manifest.root),manifest.root);
const files=new Map(manifest.files.map(file=>[file.path,file]));const bindings={};
for(const path of Object.values(manifest.source_paths)){
  const label=relative(manifest.root,path);assert.ok(label&&!label.startsWith('../')&&!isAbsolute(label));
  const pin=files.get(label);assert.ok(pin&&positive(pin.bytes));
  bindings[label]={bytes:pin.bytes,sha256:pin.sha256};
}
assert.equal(Object.keys(bindings).length,7);
const rawNative=await read(selected.native_request_path,16777216);assert.equal(digest(rawNative),selected.native_request_sha256);
const native=observe(rawNative);assert.deepStrictEqual(native.source_paths,manifest.source_paths);assert.equal(native.admission.work_deadline_ns,selected.work_deadline_ns);
assert.equal(native.admission.stage_ticket_fd,selected.stage_ticket_fd);
assert.equal(native.http.max_startup_receipt_bytes,selected.maximum_startup_receipt_bytes);
assert.equal(native.arguments.max_connections,selected.profile==='representative'?26:43);
assert.equal(native.arguments.listen,new URL(selected.native_base).host);
const marker=await read(join(selected.worker_root,'runtime/manifest.json'),selected.maximum_marker_bytes);
const paired=await read(join(selected.worker_root,'dist/__edge/build-manifest.json'),selected.maximum_marker_bytes);
assert.deepStrictEqual(marker,paired);const publication=observe(marker);
assert.equal(publication.schema,'tos_cloudflare_edge_build_v1');assert.equal(publication.read_model_schema,'tos_cloudflare_edge_read_model_v9');
assert.ok(/^[0-9a-f]{64}$/.test(publication.data_revision));
const request={schema:'tos_native_worker_full_verification_v1',profile:selected.profile,maximum_seconds:selected.maximum_seconds,
 maximum_response_bytes:selected.maximum_response_bytes,maximum_marker_bytes:selected.maximum_marker_bytes,maximum_source_bytes:selected.maximum_source_bytes,
 json_limits:selected.json_limits,maximum_native_state_bytes:selected.maximum_native_state_bytes,
 data_revision:publication.data_revision,completion_marker_sha256:digest(marker),source_root:manifest.root,worker_root:selected.worker_root,
 native_binary:selected.native_binary,native_base:selected.native_base,worker_base:selected.worker_base,failure_directory:selected.failure_directory,spool_directory:selected.spool_directory,
 source_bindings_scope:'producer_supplied_observations_only',source_bindings:bindings,
 native_startup:{request_path:selected.native_request_path,request_sha256:selected.native_request_sha256,maximum_request_bytes:16777216,
 maximum_startup_receipt_bytes:selected.maximum_startup_receipt_bytes,maximum_log_bytes:selected.maximum_native_log_bytes,
 maximum_readiness_milliseconds:selected.maximum_readiness_milliseconds,maximum_shutdown_milliseconds:selected.maximum_shutdown_milliseconds,
 maximum_inherited_fds:selected.maximum_inherited_fds,stage_ticket_fd:selected.stage_ticket_fd,work_deadline_ns:selected.work_deadline_ns}};
current();const encoded=Buffer.from(JSON.stringify(request)+'\n');assert.ok(encoded.length<=selected.wrapper_limits.maximum_request_bytes);
const output=await open(selected.output,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);
try{let offset=0;while(offset<encoded.length){current();const {bytesWritten}=await output.write(encoded,offset,encoded.length-offset,null);assert.ok(bytesWritten>0);offset+=bytesWritten;}await output.sync();}finally{await output.close();}
current();
const argv=['verify-edge-local','--worker-root',selected.worker_root,'--node-exe',selected.node_exe,'--node-script',selected.node_script,'--request',selected.output,
 '--worker-port',new URL(selected.worker_base).port,'--scratch-root',selected.scratch_root,'--work-deadline-ns',selected.work_deadline_ns,'--max-seconds',String(selected.maximum_seconds)];
for(const [key,value]of Object.entries(selected.wrapper_limits)){assert.ok(positive(value));argv.push('--'+key.replaceAll('_','-'),String(value));}
console.log(JSON.stringify({schema:'tos_host_finite_verifier_bound_recipe_v1',path:selected.output,bytes:encoded.length,sha256:digest(encoded),native_binary:selected.native_binary,argv,
 source_commit:selected.source_commit,scope:'portable transport selection, no capture authority or runtime verdict',source_input_manifest_sha256:selected.source_input_manifest_sha256,data_revision:request.data_revision,completion_marker_sha256:request.completion_marker_sha256}));
