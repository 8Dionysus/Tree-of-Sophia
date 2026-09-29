#!/usr/bin/env node
// Exact generated-WASM browser consumer against the retained local JS oracle.
import assert from 'node:assert/strict';
import {readFile,stat} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {join} from 'node:path';

const [bindingPath,wasmPath]=process.argv.slice(2);
if(!bindingPath||!wasmPath)throw new Error('usage: node research-shelf-wasm-host.mjs BINDING.mjs MODULE_bg.wasm');
const binding=await import(pathToFileURL(bindingPath).href);
const wasm=await readFile(wasmPath);
const started=performance.now();
await binding.default({module_or_path:wasm});
const startupMs=performance.now()-started;
// The maintained browser module graph includes extensionless TS imports. Use
// its existing esbuild dependency to load one in-memory consumer module.
const root=fileURLToPath(new URL('../../../../',import.meta.url));
const require=createRequire(pathToFileURL(join(root,'access/web/package.json')).href);
const {build}=require('esbuild');
const bundled=await build({stdin:{contents:`
  export {installResearchShelfRules} from './access/web/src/research-shelf/rules.mjs';
  export {createMemoryResearchShelfStore} from './access/web/src/research-shelf/storage.mjs';
  export {importSuppliedResearchPacket,validateShelfExport} from './access/web/src/research-shelf/model.mjs';
  export {readingKey} from './access/web/src/observatory/reader-model.mjs';
`,resolveDir:root,sourcefile:'research-shelf-consumer.mjs'},bundle:true,platform:'node',format:'esm',write:false});
const consumer=await import(`data:text/javascript;base64,${Buffer.from(bundled.outputFiles[0].contents).toString('base64')}`);
const {installResearchShelfRules,createMemoryResearchShelfStore,importSuppliedResearchPacket,
  validateShelfExport,readingKey}=consumer;
const revision='a'.repeat(64),content='b'.repeat(64),at='2026-09-28T12:00:00.000Z';
const target={kind:'node',id:'tos.node.shelf-wasm',sourceRevision:revision,contentRevision:content};
const input=id=>({id,title:`Shelf ${id}`,type:'material',target,collectionIds:[]});
async function exercise(){
  const store=createMemoryResearchShelfStore({now:()=>at});
  const created=await store.save(input('one'),{expectedRevision:null});
  await store.save(input('two'),{expectedRevision:null});
  const collection=await store.saveCollection({id:'group',title:'Group'});
  await assert.rejects(store.save({...created.item,target:{...target,id:'other-node'}},
    {expectedRevision:created.item.revision}),error=>error.code==='invalid-input','exact target is immutable');
  const renamed=await store.saveCollection({id:'group',title:'Renamed'},
    {expectedRevision:collection.item.revision});
  await store.save({...created.item,collectionIds:['group']},{expectedRevision:created.item.revision});
  const page=await store.list({limit:1});
  assert.ok(page.nextCursor);
  const packet=await store.export();
  const identical=await store.import(packet);
  assert.deepEqual(identical.counts,{records:0,collections:0});
  await store.removeCollection('group',renamed.item.revision);
  const detached=await store.get('one');
  const after=await store.export();
  await store.close();
  const withoutExportTime=({exportedAt,...body})=>body;
  return {packet:withoutExportTime(packet),after:withoutExportTime(after),detached,page,identical};
}
const oracle=await exercise();
const reading={v:1,activeKey:readingKey('node',target.id),entries:[{...target,preferred:'default',positions:[]}]};
const migrationOptions={source:'reading-resume',now:()=>at};
const oracleMigration=importSuppliedResearchPacket(reading,migrationOptions);
installResearchShelfRules(binding);
const actual=await exercise();
assert.deepEqual(actual,oracle,'shelf records, CAS, collection detach, export and cursor');
const collision={...actual.packet,records:[actual.packet.records[0],actual.packet.records[0]]};
assert.throws(()=>validateShelfExport(collision),error=>error.code==='invalid-packet','duplicate exact ID');
// Migration in the active path must retain the old FNV identity and skip rules.
const actualMigration=importSuppliedResearchPacket(reading,migrationOptions);
assert.deepEqual(actualMigration.packet.records,oracleMigration.packet.records);
assert.deepEqual(actualMigration.retained,oracleMigration.retained);
assert.deepEqual(actualMigration.skipped,oracleMigration.skipped);
assert.match(actualMigration.packet.records[0].id,/^import:material:[a-f0-9]{16}$/u);
const encode=new TextEncoder();
const header=count=>encode.encode(JSON.stringify({schema:'tos.research_shelf.export.v1',version:1,
  generation:0,recordCount:count,collectionCount:0}));
const ordered=new binding.ResearchShelfPacketIndex(header(2));
try{ordered.accept_records(encode.encode(JSON.stringify(['\uE000','😀'].sort())));ordered.finish();}
finally{ordered.free();}
const duplicate=new binding.ResearchShelfPacketIndex(header(2));
try{duplicate.accept_records(encode.encode(JSON.stringify(['same'])));
  assert.throws(()=>duplicate.accept_records(encode.encode(JSON.stringify(['same']))),/invalid-packet/u,'cross-chunk duplicate');}
finally{duplicate.free();}
assert.throws(()=>new binding.ResearchShelfPacketIndex(encode.encode(JSON.stringify({
  schema:'tos.research_shelf.export.v1',version:1,generation:0,recordCount:200_001,collectionCount:0,
}))),/limit/u,'record bound without whole-record allocation');
console.log(JSON.stringify({status:'pass',cases:11,host:`Node ${process.version} WebAssembly`,
  wasm_bytes:wasm.byteLength,glue_bytes:(await stat(bindingPath)).size,startup_ms:Number(startupMs.toFixed(3)),
  packet_index:'chunked ids',whole_packet_wasm_copy:false}));
