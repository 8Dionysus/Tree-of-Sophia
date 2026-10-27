import {expect,it} from 'vitest';
import {extendPublicCollection} from './collection-data.mjs';

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
