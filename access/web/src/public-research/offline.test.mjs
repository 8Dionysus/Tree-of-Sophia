import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {expect,test,vi} from 'vitest';
import {mediaFiles,standFiles} from './offline.mjs';
import {readMedia} from '../../../../bonsai/lib/media.mjs';
import zarathustraMedia from '../../../../bonsai/zarathustra/media.json' with {type:'json'};

const manifest={
  '_app-1.js':{file:'assets/zarathustra-index.js',imports:['_shared.js'],dynamicImports:['src/reader.mjs'],css:['assets/app.css']},
  '_shared.js':{file:'assets/shared-1.js',imports:['_app-1.js']},
  'src/reader.mjs':{file:'assets/reader-2.js',dynamicImports:['bonsai/zarathustra/witnesses/I.01.json'],assets:['assets/font-3.woff2']},
  'bonsai/zarathustra/witnesses/I.01.json':{file:'assets/zarathustra-witnesses-I-4.js'},
  'antiquity/index.html':{file:'assets/antiquity-index.js'},
};

test('a stand keeps every file its page may load: its chunks, the ones loaded later, their styles and assets',()=>{
  expect(standFiles(manifest,['assets/zarathustra-index.js'])).toEqual([
    '/__research/assets/app.css','/__research/assets/font-3.woff2','/__research/assets/reader-2.js',
    '/__research/assets/shared-1.js','/__research/assets/zarathustra-index.js','/__research/assets/zarathustra-witnesses-I-4.js',
  ]);
  expect(standFiles(manifest,['assets/elsewhere.js'])).toEqual(['/__research/assets/elsewhere.js']);
});

test('a stand keeps its own data, not another stand\'s, and its former versions only when asked for',()=>{
  const data={
    '_app-1.js':{file:'assets/app.js',dynamicImports:['../../bonsai/zarathustra/core/links.json','../../bonsai/antiquity/collection.json','../../bonsai/zarathustra/history/a.json','src/shared.mjs']},
    '../../bonsai/zarathustra/core/links.json':{file:'assets/links.js'},'../../bonsai/antiquity/collection.json':{file:'assets/collection.js'},
    '../../bonsai/zarathustra/history/a.json':{file:'assets/a.js'},'src/shared.mjs':{file:'assets/shared.js'},
  };
  expect(standFiles(data,['assets/app.js'],{stand:'zarathustra',versions:false})).toEqual(['/__research/assets/app.js','/__research/assets/links.js','/__research/assets/shared.js']);
  expect(standFiles(data,['assets/app.js'],{stand:'zarathustra'})).toContain('/__research/assets/a.js');
  expect(standFiles(data,['assets/app.js'],{stand:'antiquity'})).toEqual(['/__research/assets/app.js','/__research/assets/collection.js','/__research/assets/shared.js']);
});

test('the stand\'s own copies of its images are kept by their address in the layer store',()=>{
  const {urls,bytes}=mediaFiles(readMedia(zarathustraMedia));
  expect(urls.length).toBeGreaterThan(0);
  expect(urls.every(url=>/^\/__research\/layers\/sha256\/[0-9a-f]{64}$/.test(url))).toBe(true);
  expect(bytes).toBeGreaterThan(0);
});

// The service worker runs in a context of its own; it is loaded here with stand-ins for caches and the network.
function worker({online=true}={}){
  const stores=new Map(),listeners={},network=vi.fn(async request=>{if(!online)throw new TypeError('offline');return new Response(`net:${new URL(typeof request==='string'?request:request.url).pathname}`,{status:200});});
  const caches={open:async name=>{if(!stores.has(name))stores.set(name,new Map());const store=stores.get(name);const key=value=>typeof value==='string'?new URL(value,'https://treeofsophia.com').pathname+(value.includes('?')?'':''):new URL(value.url).pathname;
    return {match:async(request,options={})=>{const k=typeof request==='string'?request.startsWith('http')?new URL(request).pathname:request:new URL(request.url).pathname;const hit=store.get(k)??(options.ignoreSearch?store.get(k.split('?')[0]):undefined);return hit?.clone();},
      put:async(request,response)=>{store.set(typeof request==='string'?(request.startsWith('http')?new URL(request).pathname:request):new URL(request.url).pathname,response);},key};},
    keys:async()=>[...stores.keys()],delete:async name=>stores.delete(name)};
  const self={location:new URL('https://treeofsophia.com/sw.js'),addEventListener:(type,fn)=>{listeners[type]=fn;},skipWaiting:async()=>{},clients:{claim:async()=>{}}};
  vm.runInNewContext(readFileSync(new URL('../../public/sw.js',import.meta.url),'utf8'),{self,caches,fetch:network,URL,Response,Set,Map,Promise,String,Array,Error});
  const fetchEvent=async(url,mode='no-cors')=>{let answered=null;listeners.fetch({request:{url,method:'GET',mode},respondWith:value=>{answered=value;}});return answered?await answered:null;};
  return {self,stores,network,fetchEvent,set online(value){online=value;},listeners};
}

test('the service worker serves hashed files from its cache first and a stand\'s page from the network first',async()=>{
  const sw=worker();
  const route=sw.self.tosOfflineRoute,origin='https://treeofsophia.com';
  expect(route(new URL('https://treeofsophia.com/__research/assets/app-1.js'),origin)).toBe('immutable');
  expect(route(new URL(`https://treeofsophia.com/__research/layers/sha256/${'a'.repeat(64)}`),origin)).toBe('immutable');
  expect(route(new URL('https://treeofsophia.com/zarathustra/?lens=time'),origin)).toBe('page');
  expect(route(new URL('https://treeofsophia.com/__research/cosmos.js'),origin)).toBe('root');
  expect(route(new URL('https://treeofsophia.com/__research/public-corpus/manifest.json'),origin)).toBe('root');
  expect(route(new URL('https://images.metmuseum.org/x.jpg'),origin)).toBeNull();
  expect(route(new URL('https://treeofsophia.com/api/x'),origin)).toBeNull();
  expect(await (await sw.fetchEvent('https://treeofsophia.com/__research/assets/app-1.js')).text()).toBe('net:/__research/assets/app-1.js');
  expect(await (await sw.fetchEvent('https://treeofsophia.com/zarathustra/?lens=time','navigate')).text()).toBe('net:/zarathustra/');
  sw.online=false;
  // Without a network the hashed file and the stand's page come from what was kept; the page whatever its query.
  expect(await (await sw.fetchEvent('https://treeofsophia.com/__research/assets/app-1.js')).text()).toBe('net:/__research/assets/app-1.js');
  expect(await (await sw.fetchEvent('https://treeofsophia.com/zarathustra/?item=child','navigate')).text()).toBe('net:/zarathustra/');
  await expect(sw.fetchEvent('https://treeofsophia.com/antiquity/','navigate')).rejects.toThrow('offline');
});

test('the page may ask the service worker to keep a stand at once, and hears how far it got',async()=>{
  const sw=worker(),heard=[];let finished;
  const done=new Promise(resolve=>{finished=resolve;});
  sw.listeners.message({data:{type:'keep',urls:['/zarathustra/','/__research/assets/a.js','/__research/assets/a.js','/elsewhere']},ports:[{postMessage:value=>{heard.push(value);if(!value.running)finished();}}],waitUntil:promise=>promise});
  await done;
  expect(heard.at(-1)).toEqual({done:2,total:3,failed:1,running:false});
  sw.online=false;
  expect(await (await sw.fetchEvent('https://treeofsophia.com/__research/assets/a.js')).text()).toBe('net:/__research/assets/a.js');
});
