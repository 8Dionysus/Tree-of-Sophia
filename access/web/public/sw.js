// The site's service worker (E3): the stands open without a network once they were read or saved for it.
// Files whose names carry their content's hash (the build's assets, the layer store by SHA-256) never change and are
// served from the cache first; a stand's page is asked of the network first and falls back to the copy last kept;
// the site's other files (its root files and the files under /__research/ whose names stay) are served from the cache
// while a fresh copy is fetched. The page may ask to keep a list
// of files at once (save a stand for working offline), and is told how far it got. Nothing here is sent anywhere.
const VERSION='tos-offline-v1';
const IMMUTABLE=`${VERSION}-immutable`,PAGES=`${VERSION}-pages`,ROOT=`${VERSION}-root`;
const STANDS=new Set(['/','/zarathustra/','/antiquity/','/aesthetics/']);

/** How a request is served: 'immutable', 'page', 'root' or null (left to the network). */
function routeOf(url,origin){
  if(url.origin!==origin)return null;
  const path=url.pathname;
  if(/^\/__research\/(assets\/|layers\/sha256\/[0-9a-f]{64}$)/.test(path))return 'immutable';
  if(STANDS.has(path)||STANDS.has(`${path}/`))return 'page';
  if(path==='/theme.js'||path.startsWith('/__research/'))return 'root';
  return null;
}

async function immutable(request){
  const cache=await caches.open(IMMUTABLE),kept=await cache.match(request);if(kept)return kept;
  const response=await fetch(request);if(response.ok)await cache.put(request,response.clone());return response;
}
async function page(request){
  const cache=await caches.open(PAGES);
  try{const response=await fetch(request);if(response.ok)await cache.put(new URL(request.url).pathname,response.clone());return response;}
  catch(error){const kept=await cache.match(new URL(request.url).pathname,{ignoreSearch:true});if(kept)return kept;throw error;}
}
async function root(request){
  const cache=await caches.open(ROOT),kept=await cache.match(request,{ignoreSearch:true});
  const fresh=fetch(request).then(async response=>{if(response.ok)await cache.put(request,response.clone());return response;});
  if(kept){fresh.catch(()=>{});return kept;}
  return fresh;
}

self.addEventListener('install',event=>{event.waitUntil(self.skipWaiting());});
self.addEventListener('activate',event=>{event.waitUntil((async()=>{
  for(const name of await caches.keys())if(name.startsWith('tos-offline-')&&!name.startsWith(VERSION))await caches.delete(name);
  await self.clients.claim();
})());});
self.addEventListener('fetch',event=>{
  const request=event.request;if(request.method!=='GET')return;
  const route=routeOf(new URL(request.url),self.location.origin);
  if(route==='immutable')event.respondWith(immutable(request));
  else if(route==='page'&&request.mode==='navigate')event.respondWith(page(request));
  else if(route==='root')event.respondWith(root(request));
});
// {type:'keep', urls:[...]} keeps each file by its own route; the port hears {done, total, failed} as it goes.
self.addEventListener('message',event=>{
  const message=event.data,port=event.ports?.[0];if(message?.type!=='keep'||!Array.isArray(message.urls))return;
  event.waitUntil((async()=>{
    const urls=[...new Set(message.urls.map(String))],report={done:0,total:urls.length,failed:0};
    for(const address of urls){
      const url=new URL(address,self.location.origin),route=routeOf(url,self.location.origin);
      try{
        const name=route==='immutable'?IMMUTABLE:route==='page'?PAGES:route==='root'?ROOT:null;if(!name)throw new Error('not kept');
        const cache=await caches.open(name),key=route==='page'?url.pathname:url.href;
        if(!(await cache.match(key))){const response=await fetch(url.href,{credentials:'same-origin'});if(!response.ok)throw new Error(String(response.status));await cache.put(key,response);}
        report.done++;
      }catch{report.failed++;}
      if((report.done+report.failed)%25===0)port?.postMessage({...report,running:true});
    }
    port?.postMessage({...report,running:false});
  })());
});
self.tosOfflineRoute=routeOf;
