#!/usr/bin/env node
// Caller byte staging and process transport only. Rust owns issuance/admission/rules.
import assert from 'node:assert/strict';
import {constants} from 'node:fs';
import {open,lstat,realpath,mkdir,symlink,opendir} from 'node:fs/promises';
import {isAbsolute,join,dirname,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {spawn} from 'node:child_process';
const hash=b=>createHash('sha256').update(b).digest('hex');
const positive=n=>Number.isSafeInteger(n)&&n>0;
const stamp=(a,b)=>['dev','ino','size','mtimeNs','ctimeNs'].every(k=>a[k]===b[k]);
const argv=process.argv.slice(2);
assert.ok((argv.length===2&&argv[0]==='--selection')||(argv.length===3&&argv[0]==='--inner'),'usage: run_native_verifier.mjs --selection ABS_SELECTION_JSON');
const inner=argv[0]==='--inner';
const read=async(path,maximum)=>{
 assert.ok(isAbsolute(path)&&positive(maximum));assert.equal(await realpath(path),path);
 const fd=await open(path,constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);
 try{const before=await fd.stat({bigint:true});assert.ok(before.isFile()&&before.size>=0n&&before.size<=BigInt(maximum));
  const bytes=Buffer.alloc(Number(before.size));let offset=0;
  while(offset<bytes.length){const{bytesRead}=await fd.read(bytes,offset,bytes.length-offset,null);assert.ok(bytesRead>0);offset+=bytesRead;}
  assert.ok(stamp(before,await fd.stat({bigint:true}))&&stamp(before,await lstat(path,{bigint:true})));
  return bytes;
 }finally{await fd.close();}
};
const observe=b=>JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(b));
const configRaw=await read(argv[1],65536),c=observe(configRaw);
assert.ok(Buffer.byteLength(argv[1])<=4096);if(inner){assert.equal(hash(configRaw),argv[2],'outer/inner caller selection changed');}
assert.equal(c.schema,'tos_native_verifier_caller_v1');assert.ok(['representative','production'].includes(c.profile));
assert.ok(typeof c.work_deadline_ns==='string'&&/^[0-9]+$/.test(c.work_deadline_ns));
const cutoff=BigInt(c.work_deadline_ns),current=()=>assert.ok(process.hrtime.bigint()<cutoff,'original caller deadline expired');current();
assert.ok(positive(c.maximum_seconds)&&positive(c.maximum_shutdown_milliseconds)&&c.maximum_shutdown_milliseconds<c.maximum_seconds*1000);
assert.ok(cutoff-process.hrtime.bigint()<=BigInt(c.maximum_seconds)*1000000000n);
for(const key of ['native_binary','node_exe','native_request_template','source_input_manifest','worker_inventory','worker_root','worker_node_modules','worker_shared_root','worker_shared_inventory','node_script','failure_directory']){
 assert.ok(isAbsolute(c[key]),`explicit ${key} required`);assert.equal(await realpath(c[key]),c[key]);
}
const within=(path,parent)=>{const label=relative(parent,path);return label===''||(!label.startsWith('../')&&label!=='..'&&!isAbsolute(label));};
assert.ok(c.issuer&&isAbsolute(c.issuer.persistent_store));assert.equal(await realpath(c.issuer.persistent_store),c.issuer.persistent_store);
const privateFailure=await lstat(c.failure_directory);
assert.ok(privateFailure.isDirectory()&&!privateFailure.isSymbolicLink()&&(privateFailure.mode&0o077)===0&&privateFailure.uid===process.getuid());
assert.ok(c.failure_directory!==c.issuer.persistent_store&&within(c.failure_directory,c.issuer.persistent_store),'durable failure directory must be private child of selected store');
const writable=[c.issuer.persistent_store,'/tmp','/var/tmp','/usr/tmp'];
if(inner)writable.push(process.env.ABYSS_STAGE_ROOT);
for(const path of [c.worker_node_modules,c.worker_shared_root])for(const root of writable){assert.ok(isAbsolute(root)&&!within(path,root)&&!within(root,path),'readonly software tree overlaps write-allowed root');}
const own=fileURLToPath(import.meta.url);
const pinned=async(path,sha,maximum)=>{assert.ok(/^[0-9a-f]{64}$/.test(sha));const raw=await read(path,maximum);assert.equal(hash(raw),sha);current();return raw;};
const childTransport=async(command,args,{capture=0,ticket=null,cwd=undefined}={})=>{
 current();let stdio=['inherit',capture?'pipe':'inherit',capture?'pipe':'inherit'];
 if(ticket!==null){assert.ok(Number.isSafeInteger(ticket)&&ticket>=3&&ticket<64);stdio=Array(64).fill('ignore');stdio[0]='inherit';stdio[1]=capture?'pipe':'inherit';stdio[2]=capture?'pipe':'inherit';stdio[ticket]=ticket;}
 const remaining=cutoff-process.hrtime.bigint();assert.ok(remaining>0n&&remaining<=2147483647000000n);
 const child=spawn(command,args,{cwd,stdio,env:{...process.env,WRANGLER_SEND_METRICS:'false',NO_UPDATE_NOTIFIER:'1'}});
 let closed=false,cancelled=false,total=0,error=null;const out=[];
 const cancel=()=>{if(!closed&&!cancelled){cancelled=true;child.kill('SIGTERM');}};
 const signal=()=>{error??=new Error('caller cancelled');cancel();};
 process.once('SIGINT',signal);process.once('SIGTERM',signal);
 // Existing Rust controller/wrapper owns its work/cleanup reserve. This
 // bridge bounds terminal observation by that same original cutoff.
 let terminalTimer;
 const collect=(bytes,stdout)=>{total+=bytes.length;if(total>capture){error??=new Error('finite transport output allowance exceeded');cancel();}else if(stdout)out.push(bytes);};
 if(capture){child.stdout.on('data',b=>collect(b,true));child.stderr.on('data',b=>collect(b,false));}
 try{const code=await new Promise((resolve,reject)=>{const terminalRemaining=cutoff-process.hrtime.bigint();terminalTimer=setTimeout(()=>{error??=new Error('external containment required: consumer close not confirmed by original deadline');cancel();reject(error);},terminalRemaining>0n?Number((terminalRemaining+999999n)/1000000n):0);child.once('error',e=>{error??=e;});child.once('close',(code,signal)=>{closed=true;error??=signal?new Error(`consumer signal ${signal}`):null;resolve(code);});});
  if(error)throw error;assert.equal(code,0,'selected native/transport consumer refused');current();return Buffer.concat(out);
 }finally{clearTimeout(terminalTimer);process.removeListener('SIGINT',signal);process.removeListener('SIGTERM',signal);}
};
if(!inner){
 // Ticket does not exist yet: npm's preceding spawn cannot discard this issuer's FD.
 const i=c.issuer;assert.ok(i&&isAbsolute(i.scratch_parent)&&isAbsolute(i.persistent_store));
 assert.equal(await realpath(i.scratch_parent),i.scratch_parent);assert.equal(await realpath(i.persistent_store),i.persistent_store);
 for(const key of ['quota_bytes','inodes','working_ram_bytes'])assert.ok(positive(i[key]));
 const command=[c.node_exe,own,'--inner',argv[1],hash(configRaw)];
 if(i.kind==='existing-cgroup'){
  assert.ok(isAbsolute(i.consumer_cgroup)&&isAbsolute(i.unshare_exe));
  await childTransport(c.native_binary,['private-stage-run','--unshare-exe',i.unshare_exe,'--consumer-cgroup',i.consumer_cgroup,
   '--scratch-parent',i.scratch_parent,'--quota-bytes',String(i.quota_bytes),'--inodes',String(i.inodes),'--working-ram-bytes',String(i.working_ram_bytes),
   '--work-deadline-ns',c.work_deadline_ns,'--maximum-shutdown-ms',String(c.maximum_shutdown_milliseconds),'--persistent-store',i.persistent_store,'--',...command]);
 }else{
  assert.equal(i.kind,'ci-systemd');assert.ok(isAbsolute(i.delegate_script));assert.equal(await realpath(i.delegate_script),i.delegate_script);
  assert.equal(i.quota_bytes,536870912);assert.equal(i.inodes,65536);assert.equal(i.working_ram_bytes,2684354560);
  await childTransport('/usr/bin/bash',[i.delegate_script,c.native_binary,i.scratch_parent,i.persistent_store,c.work_deadline_ns,String(c.maximum_shutdown_milliseconds),'--',...command]);
 }
}else{
 const root=process.env.ABYSS_STAGE_ROOT,ticket=Number(process.env.ABYSS_STAGE_TICKET_FD);
 assert.ok(isAbsolute(root)&&Number.isSafeInteger(ticket)&&ticket>=3&&ticket<64);assert.equal(await realpath(root),root);
 const output=join(root,'output'),spool=join(output,'spool'),scratch=join(output,'worker-scratch');
 await mkdir(spool,{mode:0o700});await mkdir(scratch,{mode:0o700});
 const inventory=observe(await pinned(c.worker_inventory,c.worker_inventory_sha256,1048576));
 assert.equal(inventory.schema,'tos_private_worker_product_byte_inventory_v1');assert.equal(inventory.root,c.worker_root);
 assert.ok(positive(c.worker_copy_maximum_bytes)&&c.worker_copy_maximum_bytes<=67108864&&inventory.files.length>0&&inventory.files.length<=4096);
 const software=join(output,'worker-software'),worker=join(software,'access/deploy/cloudflare-worker');
 await mkdir(worker,{recursive:true,mode:0o700});let copied=0;const seen=new Set();
 for(const pin of inventory.files){
  current();const label=pin.path,parts=label.split('/');
  assert.ok(typeof label==='string'&&!isAbsolute(label)&&parts.length<=128&&parts.every(p=>p&&p!=='.'&&p!=='..'&&p!=='node_modules')&&!seen.has(label));seen.add(label);
  assert.ok(Number.isSafeInteger(pin.bytes)&&pin.bytes>=0&&copied+pin.bytes<=c.worker_copy_maximum_bytes&&/^[0-9a-f]{64}$/.test(pin.sha256));
  const source=join(c.worker_root,label);assert.equal(await realpath(source),source);const fd=await open(source,constants.O_RDONLY|constants.O_NOFOLLOW|constants.O_NONBLOCK);
  const target=join(worker,label);await mkdir(dirname(target),{recursive:true,mode:0o700});const writer=await open(target,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);
  try{const before=await fd.stat({bigint:true});assert.ok(before.isFile());assert.equal(before.size,BigInt(pin.bytes));const digest=createHash('sha256'),buffer=Buffer.alloc(65536);let used=0;
   for(;;){current();const{bytesRead}=await fd.read(buffer,0,buffer.length,null);if(!bytesRead)break;used+=bytesRead;assert.ok(used<=pin.bytes);digest.update(buffer.subarray(0,bytesRead));let offset=0;
    while(offset<bytesRead){current();const{bytesWritten}=await writer.write(buffer,offset,bytesRead-offset,null);assert.ok(bytesWritten>0);offset+=bytesWritten;}}
   assert.equal(used,pin.bytes);assert.equal(digest.digest('hex'),pin.sha256);assert.ok(stamp(before,await fd.stat({bigint:true}))&&stamp(before,await lstat(source,{bigint:true})));await writer.sync();
  }finally{await fd.close();await writer.close();}copied+=pin.bytes;
 }
 // Preserve the maintained src -> ../../../shared import closure. These trees
 // remain outside write-allowed Landlock roots; only .wrangler/D1 copies mutate.
 await symlink(c.worker_node_modules,join(worker,'node_modules'));await symlink(c.worker_shared_root,join(software,'access/shared'));
 const shared=observe(await pinned(c.worker_shared_inventory,c.worker_shared_inventory_sha256,1048576));
 assert.equal(shared.schema,'tos_private_worker_shared_byte_inventory_v1');assert.equal(shared.root,c.worker_shared_root);
 assert.ok(Array.isArray(shared.files)&&shared.files.length>0&&shared.files.length<=4096);
 const sharedPins=new Map();let sharedBytes=0;
 for(const pin of shared.files){assert.ok(typeof pin.path==='string'&&!isAbsolute(pin.path)&&pin.path.split('/').every(p=>p&&p!=='.'&&p!=='..')&&!sharedPins.has(pin.path));
  assert.ok(Number.isSafeInteger(pin.bytes)&&pin.bytes>=0&&pin.bytes<=16777216&&/^[0-9a-f]{64}$/.test(pin.sha256));sharedBytes+=pin.bytes;assert.ok(sharedBytes<=67108864);sharedPins.set(pin.path,pin);}
 const sharedIdentity=await lstat(c.worker_shared_root,{bigint:true});assert.ok(sharedIdentity.isDirectory()&&!sharedIdentity.isSymbolicLink());
 const verifyShared=async()=>{
  current();const named=await lstat(c.worker_shared_root,{bigint:true});assert.equal(named.dev,sharedIdentity.dev);assert.equal(named.ino,sharedIdentity.ino);
  let entries=0;const files=new Set();
  const scan=async(dir,label)=>{const iterator=await opendir(dir,{bufferSize:32});
   for await(const entry of iterator){current();assert.ok(++entries<=8192);assert.ok(Buffer.byteLength(entry.name)<=4096&&!entry.isSymbolicLink());const item=label?label+'/'+entry.name:entry.name;
    assert.ok(item.split('/').length<=128&&Buffer.byteLength(item)<=4096);const path=join(dir,entry.name),kind=await lstat(path);
    if(kind.isDirectory()){assert.ok(!kind.isSymbolicLink());await scan(path,item);}else{assert.ok(kind.isFile()&&!kind.isSymbolicLink()&&sharedPins.has(item));files.add(item);}}};
  await scan(c.worker_shared_root,'');assert.equal(files.size,sharedPins.size);
  for(const [label,pin]of sharedPins){current();const body=await read(join(c.worker_shared_root,label),Math.max(1,pin.bytes));assert.equal(body.length,pin.bytes);assert.equal(hash(body),pin.sha256);current();}
  const after=await lstat(c.worker_shared_root,{bigint:true});assert.equal(after.dev,sharedIdentity.dev);assert.equal(after.ino,sharedIdentity.ino);
 };
 await verifyShared();
 const rawDTO=await pinned(c.native_request_template,c.native_request_template_sha256,16777216);
 assert.equal(typeof JSON.rawJSON,'function','selected Node must support exact raw numeric transport');
 const dto=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(rawDTO),(key,value,context)=>typeof value==='number'?JSON.rawJSON(context.source):value);
 const sourceObservations=observe(await pinned(c.source_input_manifest,c.source_input_manifest_sha256,262144));
 if(dto.source_paths===null)dto.source_paths=sourceObservations.source_paths;
 if(dto.query_store.path===null)dto.query_store.path=sourceObservations.query_store.path;
 if(dto.arguments.listen===null)dto.arguments.listen=new URL(c.native_base).host;
 const dtoConnections=dto.arguments.max_connections?.rawJSON;assert.equal(dtoConnections,c.profile==='representative'?'26':'43');
 assert.equal(dto.admission.work_deadline_ns,null);assert.equal(dto.admission.stage_ticket_fd,null);
 dto.admission.work_deadline_ns=JSON.rawJSON(c.work_deadline_ns);dto.admission.stage_ticket_fd=ticket;
 const nativeRequest=join(output,'native-request.json'),nativeBytes=Buffer.from(JSON.stringify(dto)+'\n');assert.ok(nativeBytes.length<=16777216);
 const writeFresh=async(path,bytes)=>{current();const fd=await open(path,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);try{let n=0;while(n<bytes.length){current();const{bytesWritten}=await fd.write(bytes,n,bytes.length-n,null);assert.ok(bytesWritten>0);n+=bytesWritten;}await fd.sync();}finally{await fd.close();}};
 await writeFresh(nativeRequest,nativeBytes);
 const budget=observe(await read(fileURLToPath(new URL('./native_finite_verifier_budget.json',import.meta.url)),65536));
 Object.assign(budget,{profile:c.profile,source_commit:c.source_commit,source_input_manifest:c.source_input_manifest,source_input_manifest_sha256:c.source_input_manifest_sha256,
  native_binary:c.native_binary,native_request_path:nativeRequest,native_request_sha256:hash(nativeBytes),worker_root:worker,native_base:c.native_base,worker_base:c.worker_base,
  failure_directory:c.failure_directory,spool_directory:spool,scratch_root:scratch,node_exe:c.node_exe,node_script:c.node_script,stage_ticket_fd:ticket,
  work_deadline_ns:c.work_deadline_ns,output:join(output,'host-request.json')});
 assert.equal(budget.maximum_seconds,c.maximum_seconds);assert.equal(budget.maximum_shutdown_milliseconds,c.maximum_shutdown_milliseconds);
 const selected=join(output,'host-binding.json');await writeFresh(selected,Buffer.from(JSON.stringify(budget)+'\n'));
 const bound=observe(await childTransport(c.node_exe,[fileURLToPath(new URL('./bind_native_finite_verifier.mjs',import.meta.url)),selected],{capture:32768,ticket}));
 assert.equal(bound.native_binary,c.native_binary);
 await childTransport(c.native_binary,bound.argv,{ticket});
 await verifyShared();
 console.log(JSON.stringify({schema:'tos_native_verifier_caller_terminal_v1',result:'inner direct native exit0',native_request_sha256:hash(nativeBytes),host_request_sha256:bound.sha256,worker_copied_bytes:copied,worker_copied_files:seen.size,work_deadline_ns:c.work_deadline_ns,
  scope:'native domain verdict and outer controller terminal custody/resource fit remain distinct'}));
}
