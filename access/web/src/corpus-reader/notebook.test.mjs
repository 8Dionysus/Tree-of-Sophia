import {test,expect} from 'vitest';
import {createReference} from './model.mjs';
import {
  CorpusNotebookError,
  NOTEBOOK_SCHEMA,
  NOTEBOOK_VERSION,
  READING_LIMIT,
  createCorpusNotebook,
  createMemoryCorpusNotebookState,
  readingSlot,
} from './notebook.mjs';

const source=(patch={})=>({
  workId:'tos.work.fixture-000001',
  expressionId:'tos.expression.fixture-000001-ru',
  editionId:'tos.edition.fixture-000001-ru',
  itemId:'tos.item.fixture-000001-ru',
  fileId:'tos.file.fixture-000001-ru',
  fileSha256:'a'.repeat(64),
  textLayerRef:'fixture/text/000001/ru/source.txt',
  textLayerSha256:'b'.repeat(64),
  ...patch,
});
const reference=(patch={})=>createReference({source:source(patch.source),versionId:patch.versionId??'tos.expression.fixture-000001-ru',unitId:patch.unitId??'tos.text-unit.sid-00000001aaaaaaaaaaaaaaaaaaaaaaaa',start:patch.start??0,end:patch.end??12});
const open=(options={})=>createCorpusNotebook({adapter:'memory',...options});

test('memory mode is explicit and advertises its non-persistent boundary',async()=>{
  const notebook=open();
  expect(notebook.status()).toMatchObject({adapter:'pending',persistent:false,warning:null,closed:false});
  await notebook.getPreference('missing');
  expect(notebook.status()).toMatchObject({adapter:'memory',persistent:false,warning:'memory-only'});
  await notebook.close();
  expect(notebook.status().closed).toBe(true);
  await expect(notebook.listNotes()).rejects.toMatchObject({code:'closed'});
});

test('missing or failed IndexedDB is an explicit memory session with a visible warning',async()=>{
  const absent=createCorpusNotebook({indexedDB:undefined});
  await absent.putNote({reference:reference(),kind:'bookmark'});
  expect(absent.status()).toMatchObject({adapter:'memory',persistent:false,warning:'storage-unavailable'});
  const failed=createCorpusNotebook({indexedDB:{open(){throw Object.assign(new Error('quota'),{name:'QuotaExceededError'});}}});
  await failed.getPreference('theme');
  expect(failed.status()).toMatchObject({adapter:'memory',persistent:false,warning:'quota'});
});

test('notes keep exact references and page through an indexed-shaped record store',async()=>{
  const notebook=open();
  const first=await notebook.putNote({reference:reference(),quote:'Exact quote',text:'A thought',kind:'note'});
  const second=await notebook.putNote({reference:reference({unitId:'tos.text-unit.sid-00000002bbbbbbbbbbbbbbbbbbbbbbbb'}),kind:'bookmark'});
  const other=await notebook.putNote({reference:reference({source:{workId:'tos.work.fixture-000002'}}),kind:'bookmark'});
  expect(first).toMatchObject({item:{kind:'note',documentId:'tos.work.fixture-000001',quote:'Exact quote',text:'A thought'},revision:1});
  expect(second.item).toMatchObject({kind:'bookmark',documentId:'tos.work.fixture-000001'});
  expect(other.item.documentId).toBe('tos.work.fixture-000002');
  const page=await notebook.listNotes({documentId:'tos.work.fixture-000001',limit:1});
  expect(page.items).toHaveLength(1);expect(page.nextCursor).toEqual(expect.any(String));
  const next=await notebook.listNotes({documentId:'tos.work.fixture-000001',limit:1,cursor:page.nextCursor});
  expect(next.items).toHaveLength(1);expect(next.nextCursor).toBeNull();
  expect(next.items[0].reference.target.workId).toBe('tos.work.fixture-000001');
  const globalPage=await notebook.listNotes({documentId:null,limit:2});
  expect(globalPage.items).toHaveLength(2);expect(globalPage.nextCursor).toEqual(expect.any(String));
  const globalNext=await notebook.listNotes({documentId:null,limit:2,cursor:globalPage.nextCursor});
  expect(globalNext.items).toHaveLength(1);expect(globalNext.nextCursor).toBeNull();
  const globalItems=[...globalPage.items,...globalNext.items];
  expect(new Set(globalItems.map(item=>item.id)).size).toBe(3);
  expect(new Set(globalItems.map(item=>item.documentId)).size).toBe(2);
  expect((await notebook.listNotes({documentId:'tos.work.fixture-000003'})).items).toEqual([]);
});

test('expected revisions provide atomic cross-context conflict detection',async()=>{
  const shared=createMemoryCorpusNotebookState();
  const first=open({memoryStore:shared});
  const second=open({memoryStore:shared});
  const created=await first.putNote({reference:reference(),kind:'bookmark'});
  await expect(second.putNote({reference:reference({unitId:'tos.text-unit.sid-00000003cccccccccccccccccccccccc'}),kind:'note',expectedRevision:0}))
    .rejects.toMatchObject({code:'conflict',expectedRevision:0,actualRevision:created.revision});
  const updated=await second.putNote({id:created.item.id,reference:reference(),kind:'note',text:'Updated',expectedRevision:created.revision});
  expect(updated.item.text).toBe('Updated');
  expect(updated.revision).toBe(created.revision+1);
});

test('reading slots, preferences and unresolved source references stay separate',async()=>{
  const notebook=open();
  const oldReference=reference();
  const saved=await notebook.saveReading({slot:'primary',documentId:'tos.work.fixture-000001',versionId:'tos.expression.fixture-000001-ru',reference:oldReference,offset:42});
  expect(saved.reading).toMatchObject({slot:'primary',offset:42,documentId:'tos.work.fixture-000001'});
  expect(await notebook.loadReading('secondary')).toBeNull();
  expect(await notebook.loadReading('primary')).toMatchObject({reference:oldReference});
  await notebook.setPreference('fontSize',19);
  expect(await notebook.getPreference('fontSize')).toBe(19);
  const changed=reference({source:{textLayerSha256:'c'.repeat(64)}});
  await notebook.putNote({reference:oldReference,kind:'note',text:'Keeps the old source address'});
  await notebook.putNote({reference:changed,kind:'bookmark'});
  const exported=await notebook.exportPacket();
  expect(exported).toMatchObject({schema:NOTEBOOK_SCHEMA,version:NOTEBOOK_VERSION});
  expect(exported.notes.map(item=>item.reference.target.textLayerSha256)).toEqual(expect.arrayContaining(['b'.repeat(64),'c'.repeat(64)]));
  expect(JSON.stringify(exported)).not.toContain('Synthetic paragraph');
});

test('canonical per-version slots preserve a third language position while legacy slots remain valid',async()=>{
  const notebook=open();
  const russian=reference(),english=reference({versionId:'tos.expression.fixture-000001-en'}),greek=reference({versionId:'tos.expression.fixture-000001-grc'});
  const russianSlot=readingSlot(russian),englishSlot=readingSlot(english),greekSlot=readingSlot(greek);
  expect(russianSlot).toMatch(/^reading:/);expect(new Set([russianSlot,englishSlot,greekSlot]).size).toBe(3);
  for(const [slot,value,offset] of [[russianSlot,russian,10],[englishSlot,english,20],[greekSlot,greek,30]])
    await notebook.saveReading({slot,documentId:value.target.workId,versionId:value.versionId,reference:value,offset});
  expect(await notebook.loadReading(russianSlot)).toMatchObject({versionId:russian.versionId,offset:10});
  expect(await notebook.loadReading(englishSlot)).toMatchObject({versionId:english.versionId,offset:20});
  expect(await notebook.loadReading(greekSlot)).toMatchObject({versionId:greek.versionId,offset:30});
  await notebook.saveReading({slot:'primary',documentId:russian.target.workId,versionId:russian.versionId,reference:russian,offset:40});
  expect(await notebook.loadReading('primary')).toMatchObject({offset:40});
  const packet=await notebook.exportPacket();expect(packet.readings).toHaveLength(4);
  await expect(notebook.saveReading({slot:readingSlot(russian),documentId:russian.target.workId,versionId:english.versionId,reference:russian})).rejects.toMatchObject({code:'invalid-input'});
  const oversized={schema:NOTEBOOK_SCHEMA,version:NOTEBOOK_VERSION,revision:0,notes:[],readings:Array.from({length:READING_LIMIT+1},()=>null),preferences:[]};
  await expect(notebook.importPacket(oversized)).rejects.toMatchObject({code:'limit'});
});

test('import validates and merges records without destructive replacement',async()=>{
  const notebook=open();
  const kept=await notebook.putNote({reference:reference(),kind:'note',text:'Keep this'});
  const before=await notebook.exportPacket();
  const invalid=structuredClone(before);invalid.notes[0].reference.target.fileSha256='not-a-digest';
  await expect(notebook.importPacket(invalid)).rejects.toMatchObject({code:'invalid-packet'});
  expect(await notebook.exportPacket()).toMatchObject({notes:before.notes});
  const packet={schema:NOTEBOOK_SCHEMA,version:NOTEBOOK_VERSION,revision:0,notes:[],readings:[],preferences:[{key:'theme',value:'night'}]};
  const imported=await notebook.importPacket(JSON.stringify(packet));
  expect(imported.revision).toBe(2);
  expect((await notebook.listNotes()).items.map(item=>item.id)).toEqual([kept.item.id]);
  expect(await notebook.getPreference('theme')).toBe('night');
  const sameRecord=await notebook.importPacket(before);
  expect(sameRecord.revision).toBe(imported.revision);
  const noOp=await notebook.importPacket(packet);
  expect(noOp.revision).toBe(imported.revision);
});

test('import conflicts and stale expected revisions leave every record untouched',async()=>{
  const notebook=open();
  const kept=await notebook.putNote({reference:reference(),kind:'note',text:'Keep this'});
  const incomingNotebook=open();
  await incomingNotebook.putNote({reference:reference({unitId:'tos.text-unit.sid-00000002bbbbbbbbbbbbbbbbbbbbbbbb'}),kind:'note',text:'Conflicting source'});
  const incoming=await incomingNotebook.putNote({reference:reference({unitId:'tos.text-unit.sid-00000003cccccccccccccccccccccccc'}),kind:'bookmark'});
  const conflictPacket=await incomingNotebook.exportPacket();
  conflictPacket.notes[0].id=kept.item.id;
  conflictPacket.notes[0].text='Different content';
  await expect(notebook.importPacket(conflictPacket)).rejects.toMatchObject({code:'conflict',id:kept.item.id});
  expect((await notebook.listNotes()).items).toHaveLength(1);
  expect((await notebook.listNotes()).items[0].text).toBe('Keep this');
  expect((await notebook.listNotes()).items.some(item=>item.id===incoming.item.id)).toBe(false);

  const stalePacket=await incomingNotebook.exportPacket();
  await notebook.setPreference('localOnly',true);
  await expect(notebook.importPacket(stalePacket,{expectedRevision:1})).rejects.toMatchObject({code:'conflict',expectedRevision:1,actualRevision:2});
  expect((await notebook.listNotes()).items).toHaveLength(1);
  expect(await notebook.getPreference('localOnly')).toBe(true);
});

test('import preserves local reading and preference collisions while adding absent slots',async()=>{
  const notebook=open();
  const localReference=reference();
  await notebook.saveReading({slot:'primary',documentId:'tos.work.fixture-000001',versionId:'tos.expression.fixture-000001-ru',reference:localReference,offset:3});
  await notebook.setPreference('theme','local');
  const incomingNotebook=open();
  const incomingReference=reference({source:{textLayerSha256:'c'.repeat(64)}});
  await incomingNotebook.saveReading({slot:'primary',documentId:'tos.work.fixture-000001',versionId:'tos.expression.fixture-000001-ru',reference:incomingReference,offset:8});
  await incomingNotebook.saveReading({slot:'secondary',documentId:'tos.work.fixture-000001',versionId:'tos.expression.fixture-000001-ru',reference:incomingReference,offset:9});
  await incomingNotebook.setPreference('theme','incoming');
  const packet=await incomingNotebook.exportPacket();
  await notebook.importPacket(packet);
  expect(await notebook.loadReading('primary')).toMatchObject({offset:3,reference:localReference});
  expect(await notebook.loadReading('secondary')).toMatchObject({offset:9,reference:incomingReference});
  expect(await notebook.getPreference('theme')).toBe('local');
});

test('targeted note revision CAS ignores unrelated notebook edits and detects stale note writers',async()=>{
  const shared=createMemoryCorpusNotebookState(),first=open({memoryStore:shared}),second=open({memoryStore:shared});
  const created=await first.putNote({reference:reference(),kind:'note',text:'first'});
  await second.saveReading({slot:'primary',documentId:created.item.documentId,versionId:created.item.reference.versionId,reference:created.item.reference,offset:7});
  const updated=await first.putNote({id:created.item.id,reference:created.item.reference,kind:'note',text:'second',expectedRecordRevision:created.item.revision});
  expect(updated.item.text).toBe('second');
  await expect(second.putNote({id:created.item.id,reference:created.item.reference,kind:'note',text:'stale',expectedRecordRevision:created.item.revision}))
    .rejects.toMatchObject({code:'conflict',id:created.item.id,expectedRecordRevision:created.item.revision,actualRecordRevision:updated.item.revision});
  await expect(second.deleteNote(created.item.id,undefined,created.item.revision))
    .rejects.toMatchObject({code:'conflict',expectedRecordRevision:created.item.revision,actualRecordRevision:updated.item.revision});
  await second.deleteNote(created.item.id,undefined,updated.item.revision);
  expect((await first.listNotes()).items).toEqual([]);
});

test('null targeted note revision asserts that a supplied id is absent',async()=>{
  const notebook=open(),fixed='fixed-note-id';
  await notebook.putNote({id:fixed,reference:reference(),kind:'note',text:'created',expectedRecordRevision:null});
  await expect(notebook.putNote({id:fixed,reference:reference(),kind:'note',text:'collision',expectedRecordRevision:null}))
    .rejects.toMatchObject({code:'conflict',id:fixed,expectedRecordRevision:null});
});

test('mutation errors are explicit and do not silently mutate state',async()=>{
  const notebook=open();
  await expect(notebook.putNote({reference:reference(),kind:'invalid'})).rejects.toMatchObject({code:'invalid-input'});
  await expect(notebook.deleteNote('missing')).rejects.toMatchObject({code:'not-found'});
  await expect(notebook.listNotes({limit:0})).rejects.toMatchObject({code:'invalid-input'});
  await expect(notebook.listNotes({cursor:'broken'})).rejects.toMatchObject({code:'invalid-cursor'});
  await expect(notebook.putNote({reference:{},kind:'bookmark'})).rejects.toMatchObject({code:'invalid-reference'});
  expect((await notebook.listNotes()).items).toEqual([]);
  expect(notebook).toBeTruthy();
  expect(CorpusNotebookError).toBeTruthy();
});
