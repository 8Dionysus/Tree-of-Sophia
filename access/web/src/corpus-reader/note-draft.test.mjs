import {expect, test} from 'vitest';
import {createNoteDraftJournal} from './note-draft.mjs';
import {createCorpusNotebook} from './notebook.mjs';
import {createFixtureProvider} from './fixture-provider.mjs';
import {createCorpusReaderModel} from './view-model.mjs';

async function fixture(){
  const values=new Map();
  const storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value),removeItem:key=>values.delete(key)};
  const memory=createCorpusNotebook({adapter:'memory'});
  // Unit fixture models a successful durable commit; real IndexedDB and
  // document exit are covered by the browser regression.
  const notebook={...memory,status:()=>({...memory.status(),persistent:true})};
  const model=createCorpusReaderModel({provider:createFixtureProvider({documentCount:1,unitCount:80}),notebook});
  await model.ready();const page=await model.open();
  const reference=model.anchorFor(page.units[0]);
  const journal=()=>createNoteDraftJournal({notebook,storage,key:'fixture-draft',makeId:()=> 'fixture-id'});
  return {values,storage,notebook,model,reference,journal};
}

test('an interrupted note is recovered once without overwriting a parallel edit',async()=>{
  const f=await fixture();
  const old=await f.notebook.putNote({kind:'note',reference:f.reference,text:'original',expectedRecordRevision:null});
  f.journal().capture({reference:f.reference,text:'unfinished 😀 α',originalNoteId:old.item.id});
  await f.notebook.putNote({id:old.item.id,kind:'note',reference:f.reference,text:'other tab',expectedRecordRevision:old.item.revision});
  expect(await f.journal().recover()).toBe(true);
  expect((await f.notebook.getNote(old.item.id)).text).toBe('other tab');
  expect(await f.notebook.getNote('recovered-fixture-id')).toMatchObject({text:'unfinished 😀 α',reference:f.reference});
  expect(await f.journal().recover()).toBe(false);
  expect((await f.notebook.listNotes()).items).toHaveLength(2);
});

test('an older completed write cannot clear a newer draft',async()=>{
  const f=await fixture(),journal=f.journal();
  journal.capture({reference:f.reference,text:'first'});
  journal.capture({reference:f.reference,text:'latest'});
  journal.acknowledge({reference:f.reference,text:'first'});
  expect(await f.journal().recover()).toBe(true);
  expect((await f.notebook.getNote('recovered-fixture-id')).text).toBe('latest');
});

test('lost recovery acknowledgement cannot create a duplicate or overwrite a note',async()=>{
  const f=await fixture();f.journal().capture({reference:f.reference,text:'recover me'});
  const remove=f.storage.removeItem;f.storage.removeItem=()=>{throw new Error('interrupted cleanup');};
  await expect(f.journal().recover()).rejects.toThrow('interrupted cleanup');
  f.storage.removeItem=remove;
  await f.journal().recover();
  expect((await f.notebook.listNotes()).items).toHaveLength(1);
});

test('already saved text clears recovery without making a copy',async()=>{
  const f=await fixture();
  const saved=await f.notebook.putNote({kind:'note',reference:f.reference,text:'saved',expectedRecordRevision:null});
  f.journal().capture({reference:f.reference,text:'saved',originalNoteId:saved.item.id});
  expect(await f.journal().recover()).toBe(false);
  expect(f.values.size).toBe(0);
  expect((await f.notebook.listNotes()).items).toHaveLength(1);
});

test('unrecovered or malformed data is preserved instead of being replaced',async()=>{
  const f=await fixture();f.journal().capture({reference:f.reference,text:'pending'});
  expect(()=>f.journal().capture({reference:f.reference,text:'new edit'})).toThrow('still needs recovery');
  f.values.set('fixture-draft','invalid');
  await expect(f.journal().recover()).rejects.toThrow();
  expect(f.values.get('fixture-draft')).toBe('invalid');
});
