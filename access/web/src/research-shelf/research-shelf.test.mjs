import {expect,test} from 'vitest';
import {createReference} from '../corpus-reader/model.mjs';
import {NATIVE_REFERENCE_SCHEMA} from '../corpus-reader/native-reference.mjs';
import {createResearchWorkspace} from '../research-workspace.ts';
import {readingKey} from '../observatory/reader-model.mjs';
import {COPY_SCHEMA} from '../observatory/workspace-copy.mjs';
import {DEFAULT_INTERFACE} from '../observatory/interface-model.mjs';
import {emptyReading} from '../observatory/reading-resume.mjs';
import {
  EXPLORATION_DIRECTIONS,EXPLORATION_PROFILES,SUPPLIED_PACKET_SOURCES,
  importSuppliedResearchPacket,migrateResearchShelfExport,validateMaterialTarget,validateShelfExport,validateTarget,
} from './model.mjs';
import {createMemoryResearchShelfState,createMemoryResearchShelfStore,createResearchShelfStore} from './storage.mjs';

const revision='a'.repeat(64),content='b'.repeat(64);
const reference=createReference({source:{workId:'tos.work.shelf-fixture',expressionId:'tos.expression.shelf-fixture',editionId:'tos.edition.shelf-fixture',itemId:'tos.item.shelf-fixture',fileId:'tos.file.shelf-fixture',fileSha256:revision,textLayerRef:'fixture/shelf.txt',textLayerSha256:content},versionId:'tos.expression.shelf-fixture',unitId:'tos.text-unit.shelf-fixture',start:0,end:4});
const material=(patch={})=>({kind:'node',id:'tos.node.shelf-fixture',sourceRevision:revision,contentRevision:content,...patch});
const claimReference={claimId:'tos.node.shelf-fixture',pathId:'tos.path.shelf-fixture',relationType:'tos.relation.supports',nodeIds:['tos.node.subject','tos.node.shelf-fixture','tos.node.object'],relationIds:['tos.relation.subject','tos.relation.object'],detailRelationIds:[],closureNodeIds:['tos.node.subject','tos.node.shelf-fixture','tos.node.object']};
const lens={v:1,name:'Shelf lens',scope:'all',sources:['knowledge'],nodeIds:[],focusId:null,query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'overview',limit:1,relations:false};
const target=(type='material')=>({
  material:material({claimReference}),
  form:{material:material(),role:'statement',form:{id:'tos.form.shelf-fixture',version:1,digest:'sha256:'+revision}},
  text:{reference},
  lens:{draft:lens},
  route:{origin:material(),options:{profile:'overview',direction:'either',max_depth:2,sources:['knowledge'],predicate_ids:[]}},
}[type]);
const input=(id='record',type='material')=>({id,title:`Record ${id}`,type,target:type==='material'?target(type):target(type),collectionIds:[]});

test('all five target types retain exact owner fields and reject unknown types or fields',()=>{
  for(const type of ['material','form','text','lens','route']){
    const checked=validateTarget(type,target(type));
    expect(checked).toBeTruthy();
    if(type==='lens')expect(checked.draft).toMatchObject({v:2,name:'Shelf lens',scope:'all'});
  }
  const withClaim=validateMaterialTarget(material({claimReference}));expect(withClaim.claimReference).toEqual(claimReference);
  expect(()=>validateTarget('unknown',{})).toThrow();
  expect(()=>validateTarget('material',{...material(),extra:'must reject'})).toThrow();
  expect(()=>validateTarget('material',{...material(),claimReference:{...claimReference,extra:'must reject'}})).toThrow();
  expect(()=>validateTarget('route',{...target('route'),options:{...target('route').options,unexpected:true}})).toThrow();
  expect(EXPLORATION_DIRECTIONS).toContain('either');expect(EXPLORATION_PROFILES).toContain('overview');
});

test('Claim validation is delegated to one exact reading entry',()=>{
  const bad={...claimReference,nodeIds:[claimReference.nodeIds[0],claimReference.nodeIds[1],claimReference.nodeIds[2]],relationIds:['tos.relation.subject']};
  expect(()=>validateMaterialTarget(material({claimReference:bad}))).toThrow();
  expect(()=>validateMaterialTarget({...material({claimReference}),kind:'relation'})).toThrow();
});

test('text targets retain native and v1 reference schemas without source wording',()=>{
  const native={schemaVersion:NATIVE_REFERENCE_SCHEMA,origin:{kind:'node',id:'opaque:native-node'},representation:'native_public_unit',target:{
    record:{id:'tos.record.shelf',version:1,digest:'sha256:'+revision},packet:{id:'tos.packet.shelf',version:1,sha256:revision},
    segmentation:{id:'tos.segmentation.shelf',version:1},unit:{id:'tos.unit.shelf',version:1},textLayer:{id:'tos.layer.shelf',version:1,recordSha256:content},
    representationSha256:content,span:{anchorRef:'tos.anchor.shelf',start:0,end:4,exactSha256:revision}},selector:{type:'text_position',start:1,end:3,positionUnit:'unicode_code_point',interval:'half_open'}};
  expect(validateTarget('text',{reference:native}).reference).toMatchObject({schemaVersion:NATIVE_REFERENCE_SCHEMA});
  expect(validateTarget('text',{reference}).reference.schemaVersion).toBe('tos.corpus.reader.reference.v1');
  expect(validateTarget('text',{reference:{...reference,text:'source wording must not enter the shelf'}}).reference.text).toBeUndefined();
});

test('per-record CAS detects stale updates and deletes while unrelated generation changes remain visible',async()=>{
  const shared=createMemoryResearchShelfState(),first=createMemoryResearchShelfStore({memoryStore:shared}),second=createMemoryResearchShelfStore({memoryStore:shared});
  const created=await first.save(input('one'),{expectedRevision:null});
  const other=await second.save(input('two'),{expectedRevision:null});
  const updated=await first.save({...created.item,title:'First edit'},{expectedRevision:created.item.revision});
  expect(updated.item.revision).toBe(2);expect(other.item.revision).toBe(1);
  await expect(second.save({...created.item,title:'Stale edit'},{expectedRevision:created.item.revision})).rejects.toMatchObject({code:'conflict',id:'one',actualRecordRevision:2});
  await expect(second.remove('one',created.item.revision)).rejects.toMatchObject({code:'conflict',id:'one'});
  await second.remove('one',updated.item.revision);expect(await first.get('one')).toBeNull();
});

test('pagination cursors bind both filter and generation',async()=>{
  const store=createMemoryResearchShelfStore();
  await store.save(input('a'),{expectedRevision:null});await store.save(input('b'),{expectedRevision:null});
  const first=await store.list({limit:1});expect(first.items).toHaveLength(1);expect(first.nextCursor).toEqual(expect.any(String));
  await expect(store.list({limit:1,cursor:first.nextCursor,type:'text'})).rejects.toMatchObject({code:'invalid-cursor'});
  await store.save(input('c'),{expectedRevision:null});
  await expect(store.list({limit:1,cursor:first.nextCursor})).rejects.toMatchObject({code:'invalid-cursor'});
});

test('additive import is atomic and accepts an identical re-import',async()=>{
  const local=createMemoryResearchShelfStore(),incoming=createMemoryResearchShelfStore();
  const kept=await local.save(input('kept'),{expectedRevision:null});await incoming.save(input('new'),{expectedRevision:null});
  const packet=await incoming.export();packet.records[0]={...packet.records[0],id:kept.item.id,title:'different'};
  await expect(local.import(packet)).rejects.toMatchObject({code:'conflict',id:kept.item.id});
  expect((await local.list()).items.map(item=>item.id)).toEqual(['kept']);
  const clean=await incoming.export();const added=await local.import(clean);expect(added.counts.records).toBe(1);
  const same=await local.import(clean);expect(same.counts.records).toBe(0);expect((await local.list()).items.map(item=>item.id)).toEqual(['new','kept']);
});

test('explicit supplied reading-resume import retains exact material and never mutates the owner packet',async()=>{
  const reading={v:1,activeKey:readingKey('node',material().id),entries:[{kind:'node',id:material().id,sourceRevision:revision,contentRevision:content,
    preferred:'default',positions:[],claimReference}]};
  const before=structuredClone(reading);
  expect(()=>migrateResearchShelfExport(reading)).toThrow(/explicit|Choose/);
  const migrated=importSuppliedResearchPacket(reading,{source:SUPPLIED_PACKET_SOURCES.READING_RESUME,now:()=> '2026-09-15T12:00:00.000Z'});
  expect(reading).toEqual(before);expect(migrated.source).toBe('reading-resume');expect(migrated.retained).toHaveLength(1);expect(migrated.skipped).toEqual([]);
  expect(validateShelfExport(migrated.packet).records[0]).toMatchObject({type:'material',target:{kind:'node',id:material().id,claimReference}});
  const destination=createMemoryResearchShelfStore();const imported=await destination.migrate(reading,{source:'reading-resume',now:()=> '2026-09-15T12:00:00.000Z'});
  expect(imported.retained).toEqual(migrated.retained);expect((await destination.list()).items).toHaveLength(1);
});

test('research-workspace import retains only separately supplied exact lenses and reports non-portable notes and graph poses',()=>{
  const workspace=createResearchWorkspace({persistence:false});workspace.addNote({id:'note:old',body:'A session thought',targetId:'node:old'});
  workspace.saveRouteSnapshot({id:'route:old',label:'Old route',fromId:'node:a',toId:'node:b',nodeIds:['node:a','node:b'],edgeIds:['edge:old']});
  workspace.selectLens({id:'node:center',kind:'node',label:'Center'});
  const packet=JSON.parse(workspace.exportPacket()),before=structuredClone(packet);
  const migrated=importSuppliedResearchPacket(packet,{source:'research-workspace',lenses:[lens],now:()=> '2026-09-15T12:00:00.000Z'});
  expect(packet).toEqual(before);expect(migrated.retained).toHaveLength(1);expect(migrated.packet.records[0]).toMatchObject({type:'lens',title:'Shelf lens',target:{draft:{v:2,name:'Shelf lens'}}});
  expect(migrated.skipped).toEqual(expect.arrayContaining([
    expect.objectContaining({section:'notes',reason:'non-portable-note'}),
    expect.objectContaining({section:'route_snapshots',reason:'legacy-graph-pose'}),
    expect.objectContaining({section:'selected_lens',reason:'non-portable-lens-selection'}),
  ]));
  expect(()=>importSuppliedResearchPacket(packet,{source:'research-workspace',lenses:[{...lens,unexpected:'must reject'}]})).toThrow(/unknown|invalid/i);
  expect(()=>importSuppliedResearchPacket({schema:'tos.research_shelf.export.v0',version:0,records:[]},{source:'research-workspace'})).toThrow(/unsupported|invalid/i);
});

test('workspace-copy import uses the real copy decoder for saved lenses and skips session notes',()=>{
  const workspace=createResearchWorkspace({persistence:false});workspace.addNote({id:'note:copy',body:'private session note'});
  const packet={schema:COPY_SCHEMA,v:1,exportedAt:'2026-09-15T12:00:00.000Z',history:{v:1,entries:[],cursor:-1},places:[],lenses:[lens],resume:null,
    preferences:structuredClone(DEFAULT_INTERFACE),reading:emptyReading(),research:JSON.parse(workspace.exportPacket())};
  const before=structuredClone(packet),migrated=importSuppliedResearchPacket(packet,{source:'workspace-copy',now:()=> '2026-09-15T12:00:00.000Z'});
  expect(packet).toEqual(before);expect(migrated.packet.records).toHaveLength(1);expect(migrated.packet.records[0]).toMatchObject({type:'lens',title:'Shelf lens'});
  expect(migrated.skipped).toEqual(expect.arrayContaining([expect.objectContaining({section:'notes',reason:'non-portable-note'})]));
  expect(JSON.stringify(migrated)).not.toContain('private session note');
});

test('explicit memory mode and unavailable IndexedDB advertise their persistence boundary',async()=>{
  const memory=createMemoryResearchShelfStore();expect(memory.status()).toMatchObject({adapter:'pending',persistent:false,warning:null});
  await memory.list();expect(memory.status()).toMatchObject({adapter:'memory',persistent:false,warning:'memory-only'});await memory.close();
  const unavailable=createResearchShelfStore({indexedDB:undefined});await unavailable.list();expect(unavailable.status()).toMatchObject({adapter:'memory',persistent:false,warning:'storage-unavailable'});await unavailable.destroy();
});
