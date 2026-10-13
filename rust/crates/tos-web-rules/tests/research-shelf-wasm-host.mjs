#!/usr/bin/env node
// Exact generated-WASM browser consumer against retained behavior fixtures.
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
  export {installCorpusReferenceRules} from './access/web/src/corpus-reader/corpus-reference-rules.mjs';
  export {installResearchShelfRules} from './access/web/src/research-shelf/rules.mjs';
  export {installReadingRules} from './access/web/src/observatory/reading-resume-rust.mjs';
  export {installClaimReferenceRules} from './access/web/src/observatory/claim-reference-rust.mjs';
  export {createMemoryResearchShelfStore} from './access/web/src/research-shelf/storage.mjs';
  export {importSuppliedResearchPacket,validateShelfExport,validateShelfRecord} from './access/web/src/research-shelf/model.mjs';
  export {readingKey} from './access/web/src/observatory/reader-model.mjs';
`,resolveDir:root,sourcefile:'research-shelf-consumer.mjs'},bundle:true,platform:'node',format:'esm',write:false});
const consumer=await import(`data:text/javascript;base64,${Buffer.from(bundled.outputFiles[0].contents).toString('base64')}`);
const {installCorpusReferenceRules,installResearchShelfRules,installReadingRules,installClaimReferenceRules,
  createMemoryResearchShelfStore,importSuppliedResearchPacket,
  validateShelfExport,validateShelfRecord,readingKey}=consumer;
installCorpusReferenceRules(binding);
installResearchShelfRules(binding);
installReadingRules(binding);
installClaimReferenceRules(binding);
const encode=new TextEncoder();
const revision='a'.repeat(64),content='b'.repeat(64),at='2026-09-28T12:00:00.000Z';
const target={kind:'node',id:'tos.node.shelf-wasm',sourceRevision:revision,contentRevision:content};
const input=id=>({id,title:`  Shelf ${id} `,type:'material',target,collectionIds:[]});
async function exercise(){
  const store=createMemoryResearchShelfStore({now:()=>at});
  const supplied={...input('one'),target:{...target},collectionIds:[]};
  const created=await store.save(supplied,{expectedRevision:null});
  supplied.target.id='changed-input';supplied.collectionIds.push('changed-input');
  assert.deepEqual(await store.get('one'),created.item,'save detaches public input');
  const read=await store.get('one');read.target.id='changed-read';read.collectionIds.push('changed-read');
  const list=await store.list();list.items[0].target.id='changed-list';
  const exported=await store.export();exported.records[0].target.id='changed-export';
  assert.deepEqual(await store.get('one'),created.item,'get/list/export remain detached');
  await store.save(input('two'),{expectedRevision:null});
  const collection=await store.saveCollection({id:'group',title:' Group '});
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
const reading={v:1,activeKey:readingKey('node',target.id),entries:[{...target,preferred:'default',positions:[[JSON.stringify([readingKey('node',target.id),revision,content,'default']),{top:0,details:[],anchor:null}]]}]};
const migrationOptions={source:'reading-resume',now:()=>at};
const reportCases=[
  {source:'research-workspace',section:'notes',count:256,single:false},
  {source:'workspace-copy',section:'history',count:3,single:false},
  {source:'workspace-copy',section:'resume',count:1,single:true},
];
const actual=await exercise();
// Captured from the 787cbff24ba2 shelf consumer and its matched WASM owner.
// Keep concrete outputs after retiring the duplicate JS semantic engine.
const expectedRecord=(id,revision,collectionIds=[])=>({id,title:`Shelf ${id}`,type:'material',target,
  collectionIds,createdAt:at,updatedAt:at,revision});
const expectedPacket={schema:'tos.research_shelf.export.v1',version:1,generation:5,
  records:[expectedRecord('one',2,['group']),expectedRecord('two',1)],
  collections:[{id:'group',title:'Renamed',createdAt:at,updatedAt:at,revision:2}]};
assert.deepEqual(actual.packet,expectedPacket,'shelf records, CAS and export');
assert.deepEqual(actual.after,{...expectedPacket,generation:6,
  records:[expectedRecord('one',3),expectedRecord('two',1)],collections:[]},'collection detach');
assert.deepEqual(actual.detached,expectedRecord('one',3));
assert.deepEqual(actual.page.items,[expectedRecord('two',1)],'descending tie-break order');
assert.equal(actual.page.generation,5);
assert.deepEqual(JSON.parse(Buffer.from(actual.page.nextCursor,'base64url').toString('utf8')),
  {v:1,generation:5,filter:{type:null,collectionId:null},updatedAt:at,id:'two'},'bound cursor');
assert.deepEqual(actual.identical,{generation:5,counts:{records:0,collections:0},changed:false});
const targetClaim={claimId:target.id,pathId:'tos.path.shelf-wasm',relationType:'tos.relation.supports',
  nodeIds:['tos.node.subject',target.id,'tos.node.object'],relationIds:['tos.relation.subject','tos.relation.object'],
  detailRelationIds:[],closureNodeIds:['tos.node.subject',target.id,'tos.node.object']};
const immutable=createMemoryResearchShelfStore({now:()=>at});
const claimed=await immutable.save({...input('reordered-target'),target:{...target,claimReference:targetClaim}},
  {expectedRevision:null});
const reorderedTarget={claimReference:Object.fromEntries(Object.entries(targetClaim).reverse()),
  contentRevision:content,sourceRevision:revision,id:target.id,kind:target.kind};
const directUpdate=JSON.parse(new TextDecoder().decode(binding.research_shelf_rule_wasm_v1(encode.encode(JSON.stringify({
  operation:'update_record',value:{old:claimed.item,input:{...claimed.item,title:'Direct reordered target',target:reorderedTarget},now:at},
})))));
assert.equal(directUpdate.revision,2,'Rust compares reordered target fields semantically');
assert.deepEqual(directUpdate.target,claimed.item.target,'Rust preserves nested target identity');
const reordered=await immutable.save({...claimed.item,title:'Reordered target',target:reorderedTarget},
  {expectedRevision:claimed.item.revision});
assert.equal(reordered.item.revision,2,'reordered target fields preserve immutable semantic identity');
assert.deepEqual(reordered.item.target,claimed.item.target,'nested Claim keys are canonicalized');
assert.deepEqual(validateShelfRecord({...actual.packet.records[0],collectionIds:undefined}).collectionIds,[],
  'the historical undefined collectionIds default remains empty');
await assert.rejects(immutable.save({...input('undefined-claim'),target:{...target,claimReference:undefined}},
  {expectedRevision:null}),error=>error.code==='invalid-target','an own undefined Claim binding remains invalid');
await assert.rejects(immutable.save({...input('unknown-undefined'),foo:undefined},{expectedRevision:null}),
  error=>error.code==='invalid-input','own undefined unknown fields remain visible to Rust exact-key checks');
await assert.rejects(immutable.save({...input('unknown-empty-key'),'':undefined},{expectedRevision:null}),
  error=>error.code==='invalid-input','an empty-string own undefined key remains visible to Rust exact-key checks');
await immutable.close();
const collision={...actual.packet,records:[actual.packet.records[0],actual.packet.records[0]]};
assert.throws(()=>validateShelfExport(collision),error=>error.code==='invalid-packet','duplicate exact ID');
// Migration in the active path must retain the old FNV identity and skip rules.
const actualMigration=importSuppliedResearchPacket(reading,migrationOptions);
const importedId='import:material:c3fea27d2ba567ba';
assert.deepEqual(actualMigration.packet,{schema:'tos.research_shelf.export.v1',version:1,generation:0,
  records:[{...expectedRecord(importedId,1),title:'Материал'}],collections:[],exportedAt:at});
assert.deepEqual(actualMigration.retained,[{source:'reading-resume',section:'entries',index:0,
  type:'material',recordId:importedId}]);
assert.deepEqual(actualMigration.skipped,[{source:'reading-resume',section:'entries',index:0,
  reason:'non-portable-reading-position'}]);
const header=count=>encode.encode(JSON.stringify({keys:['schema','version','generation','records','collections'],
  schema:'tos.research_shelf.export.v1',version:1,generation:0,recordCount:count,collectionCount:0}));
assert.throws(()=>new binding.ResearchShelfPacketIndex(encode.encode(JSON.stringify({
  schema:'tos.research_shelf.export.v1',version:1,generation:0,recordCount:0,collectionCount:0,
}))),/invalid-packet/u,'the index requires the source packet key list');
const ordered=new binding.ResearchShelfPacketIndex(header(2));
try{ordered.accept_records(encode.encode(JSON.stringify(['\uE000','😀'].sort())));ordered.finish();}
finally{ordered.free();}
const duplicate=new binding.ResearchShelfPacketIndex(header(2));
try{duplicate.accept_records(encode.encode(JSON.stringify(['same'])));
  assert.throws(()=>duplicate.accept_records(encode.encode(JSON.stringify(['same']))),/invalid-packet/u,'cross-chunk duplicate');}
finally{duplicate.free();}
assert.throws(()=>new binding.ResearchShelfPacketIndex(encode.encode(JSON.stringify({
  keys:['schema','version','generation','records','collections'],
  schema:'tos.research_shelf.export.v1',version:1,generation:0,recordCount:200_001,collectionCount:0,
}))),/limit/u,'record bound without whole-record allocation');
const decode=new TextDecoder();
const shelfRule=(operation,value)=>JSON.parse(decode.decode(binding.research_shelf_rule_wasm_v1(
  encode.encode(JSON.stringify({operation,value})))));
for(const value of ['0000-01-01T00:00:00.000Z','2024-02-29T23:59:59.999Z',
  '-000001-01-01T00:00:00.000Z','+010000-01-01T00:00:00.000Z',
  '-271821-04-20T00:00:00.000Z','+275760-09-13T00:00:00.000Z'])
  assert.equal(shelfRule('timestamp',{value}),value,`canonical Date ISO ${value}`);
for(const value of ['-000000-01-01T00:00:00.000Z','+009999-01-01T00:00:00.000Z',
  '2023-02-29T00:00:00.000Z','-271821-04-19T23:59:59.999Z',
  '+275760-09-13T00:00:00.001Z','2026-01-01T00:00:00.00Z'])
  assert.throws(()=>shelfRule('timestamp',{value}),/invalid-record/u,`reject noncanonical Date ISO ${value}`);
const reverseTime={id:'time',title:'Time',type:'material',target,collectionIds:[],
  createdAt:'2026-09-28T12:00:00.001Z',updatedAt:'2026-09-28T12:00:00.000Z',revision:1};
assert.throws(()=>shelfRule('validate_record',reverseTime),/invalid-record/u,'same-year chronology');
for(const value of reportCases){
  const actual=JSON.parse(decode.decode(binding.research_shelf_rule_wasm_v1(encode.encode(JSON.stringify({operation:'migration_skips',value})))));
  const reason=value.section==='notes'?'non-portable-note':'legacy-graph-pose';
  assert.deepEqual(actual,Array.from({length:value.count},(_,index)=>({source:value.source,section:value.section,index:value.single?null:index,reason})));
}
const lateCollision={...actual.packet,records:[{...actual.packet.records[0],id:'new-first'}, {...actual.packet.records[1],title:'conflict'}]};
const atomic=createMemoryResearchShelfStore({now:()=>at});
await atomic.import(actual.packet);
await assert.rejects(atomic.import(lateCollision),error=>error.code==='conflict');
assert.equal(await atomic.get('new-first'),null,'late collision leaves no early import');
await atomic.close();
const members=createMemoryResearchShelfStore({now:()=>at});
await members.saveCollection({id:'group',title:'Group'});
await members.save({...input('a-first'),collectionIds:['group']});
await members.save({...input('z-last'),collectionIds:['group']});
const memberPacket=await members.export();memberPacket.records.find(item=>item.id==='z-last').revision=Number.MAX_SAFE_INTEGER;
const rollback=createMemoryResearchShelfStore({now:()=>at});await rollback.import(memberPacket);
const before=await rollback.export();
await assert.rejects(rollback.removeCollection('group',1),error=>error.code==='invalid-input');
const after=await rollback.export();
assert.deepEqual(after.records,before.records,'late detach failure preserves prior records');
assert.deepEqual(after.collections,before.collections);assert.equal(after.generation,before.generation);
await members.close();await rollback.close();
console.log(JSON.stringify({status:'pass',cases:18,host:`Node ${process.version} WebAssembly`,
  wasm_bytes:wasm.byteLength,glue_bytes:(await stat(bindingPath)).size,startup_ms:Number(startupMs.toFixed(3)),
  packet_index:'chunked ids',whole_packet_wasm_copy:false}));
