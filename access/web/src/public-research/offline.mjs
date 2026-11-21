// Working without a network (E3), the page's side: the built site registers its service worker (public/sw.js) at the
// root, and the sheet "Working offline" keeps a whole stand at once: the stand's page, every file its page may load as
// the build lists them (asset-manifest.json), and, if the person wants, the stand's own copies of its images. The act
// journal, the workplaces and the workbench live in this browser and need no network in the first place.
import './act-desk.css';
import {localeText,resolveLocale} from './locale.mjs';
import {loadMedia} from './bonsai.mjs';

const BASE='/__research/';

/**
 * Every file the build lists for a page, from the files the page itself names (its scripts and styles, as
 * `assets/...` paths): their chunks, the chunks they import or load later, their styles and assets. The stands share
 * one application chunk, which the build lists by its file rather than as an entry of each page.
 */
export function standFiles(manifest,named,{base=BASE,stand=null,versions=true}={}){
  // A stand keeps its own data, not another stand's; the retained versions of its core only when asked for.
  const skip=key=>(stand&&new RegExp(`(^|/)bonsai/(?!${stand}/)(zarathustra|antiquity|aesthetics)/`).test(key))||(!versions&&/(^|\/)bonsai\/[^/]+\/history\//.test(key));
  const byFile=new Map(Object.entries(manifest??{}).map(([key,chunk])=>[chunk.file,key]));
  const seen=new Set(),files=new Set(named.map(file=>`${base}${file}`)),queue=named.map(file=>byFile.get(file)??file);
  while(queue.length){
    const key=queue.shift();if(seen.has(key)||!manifest?.[key]||skip(key))continue;seen.add(key);
    const chunk=manifest[key];if(chunk.file)files.add(`${base}${chunk.file}`);
    for(const css of chunk.css??[])files.add(`${base}${css}`);
    for(const asset of chunk.assets??[])files.add(`${base}${asset}`);
    queue.push(...(chunk.imports??[]),...(chunk.dynamicImports??[]));
  }
  return [...files].sort();
}

/** The stand's own copies of its images (the layer store by SHA-256) and how many bytes they take. */
export function mediaFiles(media,{base=BASE}={}){
  const files=new Map();
  for(const image of Object.values(media?.module?.images??{}))if(image.api==='file')for(const copy of [image.preview,image.file])if(copy?.sha256)files.set(`${base}layers/sha256/${copy.sha256}`,copy.bytes??0);
  return {urls:[...files.keys()].sort(),bytes:[...files.values()].reduce((sum,value)=>sum+value,0)};
}

const labels={
  title:['Работа без сети','Working offline'],
  lead:['Стенд, прочитанный однажды, открывается без сети. Здесь его можно сохранить целиком, чтобы без сети открывались и главы, и линзы, которых вы ещё не открывали. Журнал действий, рабочие места и верстак живут в этом браузере и сети не требуют.','A stand read once opens without a network. Here it can be kept whole, so that chapters and lenses you have not opened yet open offline too. The act journal, the workplaces and the workbench live in this browser and need no network.'],
  ready:['Работа без сети включена','Working offline is on'],
  devOnly:['Работа без сети действует на собранном сайте, не на сервере разработки','Working offline works on the built site, not on the development server'],
  unsupported:['Этот браузер не держит страницы без сети','This browser does not keep pages offline'],
  keep:['Сохранить стенд целиком','Keep the whole stand'],
  images:['с изображениями стенда','with the stand\'s images'],
  versions:['с прежними версиями ядра (для «Что выросло» и «Как было»)','with the former versions of the core (for “What grew” and “As it was”)'],
  progress:['Сохранено {0} из {1}','Kept {0} of {1}'],
  failed:['не удалось: {0}','failed: {0}'],
  storage:['Занято в этом браузере: {0} МБ из {1} МБ','Used in this browser: {0} MB of {1} MB'],
  persisted:['Браузер обещал не стирать сохранённое','The browser promised not to clear what is kept'],
  megabytes:['МБ','MB'],
  close:['Закрыть','Close'],
};

export function registerOffline({view=globalThis,production=Boolean(import.meta.env?.PROD)}={}){
  if(!production||!view.navigator?.serviceWorker)return null;
  return view.navigator.serviceWorker.register('/sw.js',{scope:'/'}).catch(()=>null);
}

export function mountOffline({shell,actionLayer,locale=()=> 'ru',profile='zarathustra',view=globalThis,production=Boolean(import.meta.env?.PROD),loadManifest=null,media=loadMedia}){
  const ru=()=>resolveLocale(locale())==='ru';
  const t=key=>ru()?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='')=>{const node=el('button',cls,text);node.type='button';return node;};
  const registration=registerOffline({view,production});
  const manifest=loadManifest??(()=>view.fetch(`${BASE}asset-manifest.json`).then(response=>{if(!response.ok)throw new Error(String(response.status));return response.json();}));
  const dialogs=[];

  /** Ask the service worker to keep `urls`; `onProgress` hears {done,total,failed,running}. */
  async function keep(urls,onProgress=()=>{}){
    const worker=(await view.navigator.serviceWorker.ready)?.active;if(!worker)throw new Error('no service worker');
    return new Promise(resolve=>{const channel=new view.MessageChannel();channel.port1.onmessage=event=>{onProgress(event.data);if(!event.data.running)resolve(event.data);};worker.postMessage({type:'keep',urls},[channel.port2]);});
  }
  async function urlsToKeep({images,versions=false}){
    const pages=[new URL(view.location.href).pathname];
    const root=['/theme.js',`${BASE}theme.js`,`${BASE}cosmos.js`,`${BASE}cosmos.css`,`${BASE}favicon.svg`,`${BASE}asset-manifest.json`];
    // The page names its own scripts and styles; the build's list leads from them to everything they may load.
    const named=[...doc.querySelectorAll('script[src],link[rel="stylesheet"][href],link[rel="modulepreload"][href]')].map(node=>new URL(node.getAttribute('src')??node.getAttribute('href'),view.location.href).pathname)
      .filter(path=>path.startsWith(`${BASE}assets/`)).map(path=>path.slice(BASE.length));
    const files=standFiles(await manifest(),named,{stand:profile,versions});
    const pictures=images?mediaFiles(await media(profile).catch(()=>null)).urls:[];
    // What the page has already loaded from the site (its icons, the sky's pictures, the corpus manifest) stays too.
    const loaded=(view.performance?.getEntriesByType?.('resource')??[]).map(entry=>new URL(entry.name)).filter(url=>url.origin===view.location.origin&&url.pathname.startsWith(BASE)).map(url=>url.pathname+url.search);
    return [...pages,...root,...files,...pictures,...loaded];
  }

  async function open(){
    const dialog=el('dialog','ad-sheet of-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',t('title')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal?.();
    body.append(el('p','ad-muted',t('lead')));
    if(!view.navigator?.serviceWorker){body.append(el('p','',t('unsupported')));return dialog;}
    if(!production){body.append(el('p','',t('devOnly')));return dialog;}
    const state=el('p','',registration?t('ready'):t('unsupported')),line=el('p','ad-muted');line.setAttribute('aria-live','polite');
    const pictures=el('input');pictures.type='checkbox';pictures.id='of-images';
    const size=mediaFiles(await media(profile).catch(()=>null));
    const label=el('label','ad-check-row');label.append(pictures,el('span','',`${t('images')}${size.bytes?` · ${(size.bytes/1048576).toFixed(0)} ${t('megabytes')}`:''}`));
    const former=el('input');former.type='checkbox';former.id='of-versions';
    const formerLabel=el('label','ad-check-row');formerLabel.append(former,el('span','',t('versions')));
    const go=button(t('keep'),'primary');
    go.addEventListener('click',async()=>{
      go.disabled=true;
      try{
        await view.navigator.storage?.persist?.().then(granted=>{if(granted)body.append(el('p','ad-muted',t('persisted')));}).catch(()=>{});
        const urls=await urlsToKeep({images:pictures.checked,versions:former.checked});
        const result=await keep(urls,report=>{line.textContent=`${t('progress').replace('{0}',report.done).replace('{1}',report.total)}${report.failed?` · ${t('failed').replace('{0}',report.failed)}`:''}`;});
        await paintStorage();return result;
      }catch(error){line.textContent=String(error?.message??error);}finally{go.disabled=false;}
    });
    const storage=el('p','ad-muted');
    async function paintStorage(){const estimate=await view.navigator.storage?.estimate?.().catch(()=>null);if(estimate)storage.textContent=t('storage').replace('{0}',(estimate.usage/1048576).toFixed(1)).replace('{1}',(estimate.quota/1048576).toFixed(0));}
    body.append(state);if(size.urls.length)body.append(label);if(profile==='zarathustra')body.append(formerLabel);body.append(go,line,storage);
    await paintStorage();return dialog;
  }
  const remove=actionLayer?.addActions?.([{id:'offline',text:()=>t('title'),for:['any'],run:()=>open()}]);
  return {open,keep,urlsToKeep,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
