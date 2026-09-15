import {mountCorpusReader} from './reader.mjs';
import {createCorpusNotebook} from './notebook.mjs';
import {bindCorpusProvider,createUnavailableProvider} from './provider.mjs';
import {decodeReadingRoute,encodeReadingRoute} from './route.mjs';
import {mountCorpusContents} from './contents.mjs';
import {mountNativeReader} from './native-reader.mjs';
import './host.css';

// Both real UI hosts use this entry. A backend binds a supplied provider; neither
// this entry nor an unavailable provider fetches demo texts as a fallback.
export function mountCorpusEntry({root,provider=createUnavailableProvider(),notebook,client,
  toolbar=root.querySelector('.main-tools, .sc-header-actions'),locale=()=> 'ru',
  graphNavigate,onError,onReadingChange,route=true,dbName='tos-corpus-reader-v1'}={}) {
  if(!root || !toolbar)throw new TypeError('Reader host and toolbar are required.');
  provider=bindCorpusProvider(provider);
  const ownsNotebook=!notebook;
  notebook??=createCorpusNotebook({dbName});
  const opener=document.createElement('button');opener.type='button';opener.className='corpus-open sc-control';
  opener.dataset.corpusOpen='true';
  const report=error=>{
    if(onError){onError(error);return;}
    let notice=root.querySelector('.corpus-entry-notice');
    if(!notice){notice=document.createElement('p');notice.className='corpus-entry-notice';notice.setAttribute('role','status');root.append(notice);}
    notice.textContent=error?.message??String(error);
  };
  const updateLabel=()=>{opener.textContent=locale()==='en'?'Library':'Библиотека';opener.setAttribute('aria-label',locale()==='en'?'Read works and editions':'Читать произведения и издания');};
  const reader=mountCorpusReader({host:document.body,provider,notebook,locale,
    onNativeReference:(reference,note)=>native?.open({reference,note}),
    onClose(){contents?.close();root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);(opener.offsetParent===null&&narrow?narrow:opener).focus({preventScroll:true});},
    onLocation(address){if(route){try{const url=new URL(location.href);url.hash=encodeReadingRoute(address);history.replaceState(history.state,'',url);}catch(error){report(error);}}},
    async onGraphRequest(value){
      const target=value?.context?.graphTarget;
      if(!graphNavigate || !target || !['node','relation'].includes(target.kind) || typeof target.id!=='string' || !target.id || target.id.length>2048) {
        throw new Error(locale()==='en'?'This passage has no available graph address.':'Для этого фрагмента нет доступного адреса в графе.');
      }
      await graphNavigate(target,value.reference);
    },
  });
  const native=client?mountNativeReader({client,notebook,locale,onGraphRequest:graphNavigate,
    onLegacyReference:reference=>void open({documentId:reference.target.workId,versionId:reference.versionId,reference}),
    onOpen(){root.inert=true;root.dataset.corpusReading='true';onReadingChange?.(true);},
    onClose(){root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);}}):null;
  const contents=mountCorpusContents({reader,provider,locale});
  const open=async(address={})=>{
    if(native&&!native.root.hidden&&!(await native.close()))return;
    root.dataset.corpusReading='true';root.inert=true;onReadingChange?.(true);
    try{return await reader.open(address);}catch(error){root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);report(error);}
  };
  opener.addEventListener('click',()=>void open());toolbar.append(opener);updateLabel();
  // Constructor hides its wide toolbar on a narrow screen. Reading must stay
  // reachable there even before any graph selection has been made.
  const narrow=root.querySelector('.header')?document.createElement('button'):null;
  if(narrow){narrow.type='button';narrow.className='corpus-open corpus-open-narrow';narrow.textContent='▤';narrow.setAttribute('aria-label',locale()==='en'?'Read works and editions':'Читать произведения и издания');narrow.onclick=()=>void open();root.querySelector('.header').append(narrow);}
  root.addEventListener('click',updateLabel);
  const readEvent=event=>void open(event.detail??{});
  root.addEventListener('sophia-read-text',readEvent);
  const nativeEvent=async event=>{if(!native)return;await reader.close();if(reader.isOpen())return;root.dataset.corpusReading='true';await native.open(event.detail??{});};
  root.addEventListener('sophia-read-native',nativeEvent);
  const notesOpen=native?document.createElement('button'):null;
  if(notesOpen){notesOpen.type='button';notesOpen.className='corpus-open sc-control';notesOpen.dataset.nativeNotes='true';notesOpen.textContent=locale()==='en'?'My notes':'Мои записи';
    notesOpen.onclick=async()=>{await reader.close();if(reader.isOpen())return;await native.openNotes();};toolbar.append(notesOpen);}
  const restore=()=>{if(!route)return;try{const address=decodeReadingRoute(location.hash);if(address)void open(address);}catch(error){report(error);}};
  window.addEventListener('hashchange',restore);
  let destroyed=false,destroyWork=null;
  const destroy=()=>{
    if(destroyed)return destroyWork;destroyed=true;contents.destroy();
    const saving=[reader.destroy(),native?.flush()];
    opener.remove();notesOpen?.remove();narrow?.remove();root.removeEventListener('click',updateLabel);root.removeEventListener('sophia-read-text',readEvent);root.removeEventListener('sophia-read-native',nativeEvent);window.removeEventListener('hashchange',restore);window.removeEventListener('pagehide',pageHide);
    destroyWork=Promise.allSettled(saving).then(async()=>{native?.destroy();if(ownsNotebook)await notebook.close();});return destroyWork;
  };
  const pageHide=event=>{if(!event.persisted)destroy();};
  window.addEventListener('pagehide',pageHide);restore();
  return {reader,native,notebook,opener,open,destroy};
}
