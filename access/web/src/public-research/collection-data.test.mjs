import {expect,it} from 'vitest';
import {extendPublicCollection} from './collection-data.mjs';
import retained from '../../../public-corpus/retained-library-map.v1.json' with {type:'json'};
import collection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};

it('binds navigation through the separate public structure without changing the Zarathustra graph identity',async()=>{
  const id='tos.work.plato.meno',version={id:'published-en',language:'en',revision:'a'.repeat(64)};
  const fingerprint='b'.repeat(64),prepared={library:{fingerprint,nodes:[],space:{}},bindings:new Map(),documentBindings:new Map()};
  await extendPublicCollection(prepared,{profile:'zarathustra',sources:{documents:new Map([[id,{document:{id,versions:[version]}}]]),manifest:{documents:[{id}]}},
    structures:new Map([[id,[{label:'80',unitId:'published-section-80'}]]])});
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
