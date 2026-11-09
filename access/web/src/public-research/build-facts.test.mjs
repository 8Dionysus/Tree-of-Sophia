import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {canonicalJSON} from '../../../../bonsai/lib/history-format.mjs';
import {loadBonsai} from './bonsai.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';

// A page built from the same files carries an identity; the one served from source carries none.
const facts=vi.hoisted(()=>({buildIdentity:null}));
vi.mock('virtual:tos-build-facts',async original=>{
  const actual=await original();
  return {researchRevision:actual.researchRevision,get buildIdentity(){return facts.buildIdentity;}};
});
const {researchRevision}=await import('virtual:tos-build-facts');
const {createPublicSpace}=await import('./space-library.mjs');

const sha256=async text=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text))),byte=>byte.toString(16).padStart(2,'0')).join('');
const id='tos.work.friedrich-nietzsche.also-sprach-zarathustra';
const provider=revision=>({describe:async()=>({revision,profiles:{zarathustra:{documentIds:[id]}},documents:[{id}]}),
  catalog:async()=>({items:[{id,title:'Thus Spoke Zarathustra',author:'Friedrich Nietzsche'}]}),
  document:async()=>({id,versions:[{id:'de',language:'de',revision:'a'.repeat(64)}]}),structure:async()=>({items:[],nextCursor:null})});
let values;
beforeEach(()=>{values=new Map();vi.stubGlobal('localStorage',{getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,String(value))});facts.buildIdentity=null;});
afterEach(()=>vi.unstubAllGlobals());

it('knows the research core revision the page would otherwise compute',async()=>{
  expect(researchRevision).toBe(await sha256(canonicalJSON((await loadBonsai('zarathustra')).core)));
  const prepared=await createPublicSpace({profile:'zarathustra',provider:provider('b'.repeat(64))});
  expect(prepared.library.space.researchRevision).toBe(researchRevision);
});

it('remembers the library fingerprint for one build and one corpus snapshot, and hashes again for any other',async()=>{
  const fresh=(await createPublicSpace({profile:'zarathustra',provider:provider('b'.repeat(64))})).library.fingerprint;
  expect(values.size).toBe(0);
  facts.buildIdentity='1'.repeat(64);
  expect((await createPublicSpace({profile:'zarathustra',provider:provider('b'.repeat(64))})).library.fingerprint).toBe(fresh);
  const held=JSON.parse(values.get('tos.public-research.library-fingerprint.v1.zarathustra'));
  expect(held.fingerprint).toBe(fresh);
  // What is remembered is what the next visit of the same build and snapshot reads.
  values.set('tos.public-research.library-fingerprint.v1.zarathustra',JSON.stringify({...held,fingerprint:'c'.repeat(64)}));
  expect((await createPublicSpace({profile:'zarathustra',provider:provider('b'.repeat(64))})).library.fingerprint).toBe('c'.repeat(64));
  // Another snapshot or another build hashes the library again.
  expect((await createPublicSpace({profile:'zarathustra',provider:provider('d'.repeat(64))})).library.fingerprint).not.toBe('c'.repeat(64));
  values.set('tos.public-research.library-fingerprint.v1.zarathustra',JSON.stringify({...held,fingerprint:'c'.repeat(64)}));
  facts.buildIdentity='2'.repeat(64);
  expect((await createPublicSpace({profile:'zarathustra',provider:provider('b'.repeat(64))})).library.fingerprint).toBe(fresh);
});
