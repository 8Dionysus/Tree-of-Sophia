// Test transport for the existing Rust diagnostics-v2 schema worker. Domain
// constraints remain in the declared JSON schemas and the Rust validator.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync,readdirSync,statSync} from 'node:fs';
import {isAbsolute,join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const bytes = value => Buffer.isBuffer(value) ? value : Buffer.from(value);
const hash = (...parts) => {const h=createHash('sha256');for(const p of parts)h.update(bytes(p));return h.digest();};
const concat = (...parts) => Buffer.concat(parts.map(bytes));
const u16=n=>{const b=Buffer.alloc(2);b.writeUInt16BE(n);return b;};
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
const u64=n=>{const b=Buffer.alloc(8);b.writeBigUInt64BE(BigInt(n));return b;};
const field=value=>{const b=bytes(value);return concat(u32(b.length),b);};
const version=u16(2),caps=concat(u32(8),u32(32768),u16(128),u32(4096));
const capsHash=hash('tos-schema-diagnostics-caps-v2\0',version,caps);
const emptyIssues=hash('tos-schema-diagnostics-issues-v2\0',u64(0));

export function validateNativePackets(packets,schemaName,contracts=fileURLToPath(new URL('../../../contracts/',import.meta.url))) {
  assert(Array.isArray(packets) && packets.length>0 && packets.length<=512);
  const worker=process.env.TOS_SCHEMA_WORKER_PATH;
  assert(worker && isAbsolute(worker),'select TOS_SCHEMA_WORKER_PATH for actual Rust schema validation');
  assert(statSync(worker).size<=128*1024*1024);
  const workerHash=hash(readFileSync(worker));
  const available=new Map();let resourceBytes=0;
  for(const name of readdirSync(contracts).sort()) {
    if(!name.endsWith('.schema.json'))continue;
    const path=join(contracts,name);assert(statSync(path).size<=4*1024*1024);
    const raw=readFileSync(path);resourceBytes+=raw.length;assert(resourceBytes<=32*1024*1024);
    const value=JSON.parse(raw);assert(value.$schema==='https://json-schema.org/draft/2020-12/schema');
    assert(typeof value.$id==='string' && value.$id.startsWith('https://') && !available.has(value.$id));
    available.set(value.$id,{raw,value,name});
  }
  const root=[...available].find(([,r])=>r.name===schemaName)?.[0];assert(root,'selected result schema absent');
  const selected=new Map();
  function add(uri) {
    if(selected.has(uri))return;
    const row=available.get(uri);assert(row,`local schema dependency absent: ${uri}`);selected.set(uri,row.raw);
    function visit(v) {if(!v || typeof v!=='object')return;
      if(typeof v.$ref==='string'){const ref=new URL(v.$ref,uri);ref.hash='';add(ref.href);}
      for(const child of Object.values(v))visit(child);
    }
    visit(row.value);
  }
  add(root);assert(selected.size<=512);
  const resources=[...selected].sort(([a],[b])=>a<b?-1:a>b?1:0);
  const schemaHash=hash('tos-schema-set-v1\0',...resources.flatMap(([uri,raw])=>[u64(Buffer.byteLength(uri)),uri,hash(raw)]));
  const encodedResources=concat(u32(resources.length),...resources.flatMap(([uri,raw])=>[field(uri),field(raw)]));
  for(let start=0;start<packets.length;start+=64) {
    const units=packets.slice(start,start+64).map((packet,index)=>{
      const raw=Buffer.from(JSON.stringify(packet));assert(raw.length<=1024*1024);
      const body=concat(u64(index),field(`packet-${start+index}`),field(`packets/${start+index}.json`),field(root),field(raw));
      return {body,digest:hash('tos-val2-batch-unit-v1\0',body)};
    });
    const request=concat('TOSV2SD2',version,caps,workerHash,schemaHash,Buffer.from([1]),encodedResources,u32(units.length),...units.map(u=>u.body));
    assert(request.length<=36*1024*1024);
    const requestHash=hash(request),stream=[];
    const responses=units.map((u,index)=>{
      const report=hash('tos-schema-diagnostics-report-v2\0',version,workerHash,requestHash,u.digest,schemaHash,capsHash,Buffer.alloc(3),u64(0),emptyIssues);
      stream.push(u.digest,Buffer.alloc(3),u64(0),u32(0),report);
      return concat('TOSV2DU2',version,u64(index),u.digest,Buffer.alloc(2),u64(0),Buffer.alloc(1),u32(0),u32(0),emptyIssues);
    });
    const common=concat(version,requestHash,workerHash,schemaHash,capsHash,u32(units.length));
    const expected=concat('TOSV2DA2',common,...responses,'TOSV2DF2',common,hash('tos-schema-diagnostics-results-v2\0',...stream));
    const result=spawnSync('/usr/bin/prlimit',['--as=1073741824','--cpu=15','--',worker],{input:request,timeout:20000,killSignal:'SIGKILL',maxBuffer:8*1024*1024});
    assert.ifError(result.error);assert.equal(result.status,0,`native schema worker failed: ${result.stderr}`);
    assert(result.stdout.equals(expected),`native schema validation refused or returned incomplete diagnostics for packet batch ${start}`);
  }
  assert(hash(readFileSync(worker)).equals(workerHash),'selected schema worker changed');
}
