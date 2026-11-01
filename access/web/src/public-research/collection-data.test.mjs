import {expect,it} from 'vitest';
import {extendPublicCollection} from './collection-data.mjs';
import {createPublicSpace} from './space-library.mjs';
import {upgradePublicGraph} from './graph-transfer.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';
import retained from '../../../public-corpus/retained-library-map.v1.json' with {type:'json'};
import collection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};

it('binds navigation through the separate public structure without changing the Zarathustra graph identity',async()=>{
  const id='tos.work.plato.meno',version={id:'published-en',language:'en',revision:'a'.repeat(64)};
  const fingerprint='b'.repeat(64),prepared={library:{fingerprint,nodes:[],space:{}},bindings:new Map(),documentBindings:new Map()};
  await extendPublicCollection(prepared,{profile:'zarathustra',sources:{documents:new Map([[id,{document:{id,versions:[version]}}]]),manifest:{documents:[{id}]}},
    structures:new Map([[id,[{label:'80',unitId:'published-section-80'}]]])});
  // The Zarathustra stand's first view holds its own entries; the shared Antiquity entries arrive when asked for.
  expect(prepared.library.space.collection.some(item=>item.id==='meno-search')).toBe(false);
  await prepared.library.space.loadCollection();
  const passage=prepared.library.space.collection.find(item=>item.id==='meno-search');
  expect(passage.publicAddress).toMatchObject({documentId:id,versionId:version.id,unitId:'published-section-80',precision:'section-entry'});
  expect(prepared.library.fingerprint).toBe(fingerprint);
  expect(prepared.library.space.collection.find(item=>item.id==='theaetetus-account').publicAddress).toBeUndefined();
});

it('connects every retained book lazily while preserving existing collection material IDs',async()=>{
  const ids=['tos.work.plato.meno','tos.work.plato.apology','tos.work.aristotle.nicomachean-ethics'];
  const documents=new Map(ids.map(id=>[id,{document:{id,versions:[{id:'published-en',language:'en',revision:'a'.repeat(64)}]}}]));
  const prepared={library:{fingerprint:'b'.repeat(64),nodes:[],space:{}},bindings:new Map(),documentBindings:new Map()};
  await extendPublicCollection(prepared,{profile:'zarathustra',sources:{documents,manifest:{documents:retained.books.map(book=>({id:book.documentId}))}},structures:new Map(ids.map(id=>[id,[]]))});
  await prepared.library.space.loadCollection();
  const works=prepared.library.space.collection.filter(item=>item.kind==='work');
  for(const book of retained.books){const work=works.find(item=>item.legacyId===book.legacyId);expect(work?.publicDocumentId).toBe(book.documentId);expect(work?.publicVersionId).toBeTruthy();}
  expect(retained.books).toHaveLength(49);
  const doctrines=works.find(item=>item.legacyId==='doctrines');
  expect(doctrines.publicDocumentId).toBeUndefined(); // The containing witness is not a new standalone edition.
  expect(prepared.bindings.get(doctrines.materialId)).toMatchObject({documentId:'tos.work.retained-antiquity.lives',versionId:'retained-lives-eng2-102e405ff1f2',reference:{selector:{start:50139,end:50255}}});
  expect(works.find(item=>item.legacyId==='theaetetus')?.materialId).toBe('antiquity:catalog:theaetetus');
});

it('distinguishes original source links from translations without changing the legacy link identity',()=>{
  const ref=id=>collection.entities.find(item=>item.id===id).sourceRefs[0];
  expect(ref('athenian-constitution')).toMatchObject({language:'grc',versionId:'demo-ru',label:{en:'Greek text (TEI)'}});
  expect(ref('enneads')).toMatchObject({language:'grc',versionId:'demo-ru',label:{en:'Greek text (TEI)'}});
  expect(ref('de-fato')).toMatchObject({language:'la',versionId:'demo-ru',label:{en:'Latin text (TEI)'}});
  expect(ref('isagoge')).toMatchObject({language:'la',versionId:'demo-ru',label:{en:'Latin translation: Boethius'}});
  expect(ref('phaedrus')).toMatchObject({language:'en',versionId:'demo-ru',label:{en:'English translation (TEI)'}});
});

it('restores the newest compatible graph when preserved predecessors coexist',async()=>{
  const id='tos.work.friedrich-nietzsche.also-sprach-zarathustra',version={id:'de',language:'de',revision:'a'.repeat(64)};
  const document={id,versions:[version]},item={id,title:'Thus Spoke Zarathustra',author:'Friedrich Nietzsche'};
  const provider={describe:async()=>({revision:'fixture',profiles:{zarathustra:{documentIds:[id]}},documents:[{id}]}),
    catalog:async()=>({items:[item]}),document:async()=>document,structure:async()=>({items:[],nextCursor:null})};
  const prepared=await createPublicSpace({profile:'zarathustra',provider}),predecessors=prepared.graphPredecessors;
  const current=predecessors[0],strict=predecessors.find(p=>p.fingerprint.startsWith('f7162d1b'));
  const older=predecessors.find(p=>p.fingerprint.startsWith('c2a23454'));
  const key=fp=>`tos.public-research.graph.v1.zarathustra.${fp}`;
  const packet=(fp,title)=>JSON.stringify({libraryFingerprint:fp,nodes:[],title});
  const values=new Map([[key(current.fingerprint),packet(current.fingerprint,'Latest')],[`${key(current.fingerprint)}.owners`,'{"latest":"node"}'],
    [key(strict.fingerprint),packet(strict.fingerprint,'Previous')],[key(older.fingerprint),packet(older.fingerprint,'Old backup')]]);
  const storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,String(value))};
  const options={profile:'zarathustra',fingerprint:prepared.library.fingerprint,predecessors,validate:value=>value};
  expect(upgradePublicGraph(storage,options).from).toBe(key(current.fingerprint));
  expect(JSON.parse(values.get(key(options.fingerprint))).title).toBe('Latest');
  expect(values.get(`${key(options.fingerprint)}.owners`)).toBe('{"latest":"node"}');
  expect(JSON.parse(values.get(key(older.fingerprint))).title).toBe('Old backup');
  values.delete(key(options.fingerprint));values.delete(`${key(options.fingerprint)}.owners`);values.delete(key(current.fingerprint));
  expect(upgradePublicGraph(storage,options).from).toBe(key(strict.fingerprint));
  expect(JSON.parse(values.get(key(options.fingerprint))).title).toBe('Previous');
});

it('prefers the latest released Antiquity graph over older retained backups',async()=>{
  const ids=['tos.work.plato.meno','tos.work.plato.apology','tos.work.aristotle.nicomachean-ethics'];
  const version={id:'en',language:'en',revision:'a'.repeat(64)};
  const documents=ids.map(id=>({id,versions:[version]})),items=ids.map(id=>({id,title:id,author:'Author'}));
  const provider={describe:async()=>({revision:'fixture',profiles:{antiquity:{documentIds:ids}},documents:items}),
    catalog:async()=>({items}),document:async({documentId})=>documents.find(item=>item.id===documentId),structure:async()=>({items:[],nextCursor:null})};
  const prepared=await createPublicSpace({profile:'antiquity',provider}),predecessors=prepared.graphPredecessors;
  const latest=predecessors.find(p=>p.fingerprint.startsWith('ed91986c')),older=predecessors.find(p=>p.fingerprint.startsWith('f5f06c55'));
  const key=fp=>`tos.public-research.graph.v1.antiquity.${fp}`;
  const packet=(fp,title)=>JSON.stringify({libraryFingerprint:fp,nodes:[],title});
  const values=new Map([[key(latest.fingerprint),packet(latest.fingerprint,'Latest')],[key(older.fingerprint),packet(older.fingerprint,'Old backup')]]);
  const storage={getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,String(value))};
  const options={profile:'antiquity',fingerprint:prepared.library.fingerprint,predecessors,validate:value=>value};
  expect(upgradePublicGraph(storage,options).from).toBe(key(latest.fingerprint));
  expect(JSON.parse(values.get(key(options.fingerprint))).title).toBe('Latest');
  expect(JSON.parse(values.get(key(older.fingerprint))).title).toBe('Old backup');
});
