import {localeText,copyLocale} from '../public-research/locale.mjs';
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
  graphNavigate,onError,onReadingChange,onReadingLocation,onLibraryRequest,onReturnFocusFallback,onUiLocaleChange,route=true,dbName='tos-corpus-reader-v1'}={}) {
  if(!root || !toolbar)throw new TypeError('Reader host and toolbar are required.');
  provider=bindCorpusProvider(provider);
  const ownsNotebook=!notebook;
  notebook??=createCorpusNotebook({dbName});
  const opener=document.createElement('button');opener.type='button';opener.className='corpus-open sc-control';
  opener.dataset.corpusOpen='true';
  const english=()=>(copyLocale(locale()||'ru')==='en');
  const displayError=error=>{
    const key=String(error?.code??error?.status??'').toLowerCase();
    if(key==='provider_unavailable')return english()?localeText('The full-text source is not connected yet.',locale):'Источник полных текстов пока не подключён.';
    if(key==='version-unavailable'||key==='404')return english()?localeText('This edition is unavailable.',locale):'Это издание недоступно.';
    if(key==='document-unavailable')return english()?localeText('The work is unavailable.',locale):'Произведение недоступно.';
    if(key==='conflict')return english()?localeText('The notebook changed in another tab.',locale):'Блокнот изменён в другой вкладке.';
    if(key==='stale_reference'||key==='changed')return english()?localeText('The saved passage belongs to an older text version.',locale):'Сохранённый фрагмент относится к старой версии текста.';
    return english()?localeText('The reading view could not be opened. Try again.',locale):'Не удалось открыть чтение. Повторите попытку.';
  };
  const report=error=>{
    if(onError){onError(error);return;}
    let notice=root.querySelector('.corpus-entry-notice');
    if(!notice){notice=document.createElement('p');notice.className='corpus-entry-notice';notice.setAttribute('role','status');root.append(notice);}
    notice.textContent=displayError(error);
  };
  const updateLabel=()=>{opener.textContent=english()?localeText('Library',locale):'Библиотека';opener.setAttribute('aria-label',english()?localeText('Read works and editions',locale):'Читать произведения и издания');
    narrow?.setAttribute('aria-label',opener.getAttribute('aria-label'));if(notesOpen)notesOpen.textContent=english()?localeText('My notes',locale):'Мои записи';};
  const reader=mountCorpusReader({host:document.body,provider,notebook,locale,onUiLocaleChange,
    onNativeReference:(reference,note)=>native?.open({reference,note}),
    onClose(){contents?.close();if(route&&location.hash.startsWith('#reading=')){const url=new URL(location.href);url.hash='';history.replaceState(history.state,'',url);}root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);},
    onReturnFocusFallback(){
      const handled=onReturnFocusFallback?.();
      if(handled===true)return;
      const target=handled?.focus ? handled : (opener.offsetParent===null&&narrow?narrow:opener);
      target?.focus?.({preventScroll:true});
    },
    onLocation(address){if(route){try{const url=new URL(location.href);url.hash=encodeReadingRoute(address);history.replaceState(history.state,'',url);}catch(error){report(error);}}
      try{onReadingLocation?.(address);}catch(error){report(error);}},
    async onGraphRequest(value){
      const target=value?.context?.graphTarget;
      if(!graphNavigate || !target || !['node','relation'].includes(target.kind) || typeof target.id!=='string' || !target.id || target.id.length>2048) {
        throw new Error(copyLocale(locale)==='en'?localeText('This passage has no available graph address.',locale):'Для этого фрагмента нет доступного адреса в графе.');
      }
      await graphNavigate(target,value.reference);
    },
  });
  const native=client?mountNativeReader({client,notebook,locale,onGraphRequest:graphNavigate,
    onLegacyReference:reference=>void open({documentId:reference.target.workId,versionId:reference.versionId,reference}),
    onOpen(){root.inert=true;root.dataset.corpusReading='true';onReadingChange?.(true);},
    onClose(){root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);}}):null;
  const contents=mountCorpusContents({reader,provider,locale});
  const open=async(address={},options={})=>{
    const returnFocus=options.returnFocus || document.activeElement;
    if(native&&!native.root.hidden&&!(await native.close()))return;
    root.dataset.corpusReading='true';root.inert=true;onReadingChange?.(true);
    try{return await reader.open({...address,returnFocus});}catch(error){root.inert=false;delete root.dataset.corpusReading;onReadingChange?.(false);report(error);}
  };
  const openLibrary=()=>{if(onLibraryRequest)onLibraryRequest();else void open();};
  opener.addEventListener('click',openLibrary);toolbar.append(opener);
  // Constructor hides its wide toolbar on a narrow screen. Reading must stay
  // reachable there even before any graph selection has been made.
  const narrow=root.querySelector('.header')?document.createElement('button'):null;
  if(narrow){narrow.type='button';narrow.className='corpus-open corpus-open-narrow';narrow.textContent='▤';narrow.setAttribute('aria-label',english()?localeText('Read works and editions',locale):'Читать произведения и издания');narrow.onclick=openLibrary;root.querySelector('.header').append(narrow);}
  const refreshLocale=()=>{updateLabel(); reader.refreshLocale?.(); native?.refreshLocale?.();};
  document.addEventListener('sophia-ui-language',refreshLocale);
  const readEvent=event=>void open(event.detail??{});
  root.addEventListener('sophia-read-text',readEvent);
  const nativeEvent=async event=>{if(!native)return;await reader.close();if(reader.isOpen())return;root.dataset.corpusReading='true';await native.open(event.detail??{});};
  root.addEventListener('sophia-read-native',nativeEvent);
  const notesOpen=native?document.createElement('button'):null;
  if(notesOpen){notesOpen.type='button';notesOpen.className='corpus-open sc-control';notesOpen.dataset.nativeNotes='true';notesOpen.textContent=english()?localeText('My notes',locale):'Мои записи';
    notesOpen.onclick=async()=>{await reader.close();if(reader.isOpen())return;await native.openNotes();};toolbar.append(notesOpen);}
  updateLabel();
  const restore=()=>{if(!route)return;try{const address=decodeReadingRoute(location.hash);if(address)void open(address);}catch(error){report(error);}};
  window.addEventListener('hashchange',restore);
  let destroyed=false,destroyWork=null;
  const destroy=()=>{
    if(destroyed)return destroyWork;destroyed=true;contents.destroy();
    const saving=[reader.destroy(),native?.flush()];
    opener.remove();notesOpen?.remove();narrow?.remove();document.removeEventListener('sophia-ui-language',refreshLocale);root.removeEventListener('sophia-read-text',readEvent);root.removeEventListener('sophia-read-native',nativeEvent);window.removeEventListener('hashchange',restore);window.removeEventListener('pagehide',pageHide);
    destroyWork=Promise.allSettled(saving).then(async()=>{native?.destroy();if(ownsNotebook)await notebook.close();});return destroyWork;
  };
  // pagehide cannot be awaited by the browser; destroy performs the best
  // effort synchronously and keeps owned resources until its flushes settle.
  const pageHide=event=>{if(!event.persisted)void destroy();};
  window.addEventListener('pagehide',pageHide);restore();
  return {reader,native,notebook,opener,open,refreshLocale,destroy};
}
