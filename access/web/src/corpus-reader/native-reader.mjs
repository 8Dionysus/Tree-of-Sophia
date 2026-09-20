import {createNativeReference,validateNativeReference,NATIVE_REFERENCE_SCHEMA,nativeSelectedText} from './native-reference.mjs';
import {referenceDocumentId,referenceVersionId} from './model.mjs';
import {readingSlot} from './notebook.mjs';
import {readNativeSelection,readNativeReference} from './native-source.mjs';
import {scheduleExpiry} from './expiry.mjs';
import {rawDataDownload} from '../observatory/human-presentation.mjs';
import {sourceReadExport} from '../observatory/exact-source-read.mjs';
import './native-reader.css';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const action=(label,run)=>{const button=el('button',label);button.type='button';button.onclick=()=>void Promise.resolve().then(run).catch(error=>button.dispatchEvent(new CustomEvent('native-reader-error',{detail:error,bubbles:true})));return button;};
const HASH='#native-reading=';

// A DOM range may only address one delivered span. Separate source spans do
// not become a continuous selection through DOM layout or copied separators.
export function nativeRangeSelection(range,article,result){
  if(!range||range.collapsed)return null;
  const spanOf=node=>(node.nodeType===Node.ELEMENT_NODE?node:node.parentElement)?.closest('[data-native-span]');
  const first=spanOf(range.startContainer),last=spanOf(range.endContainer);
  if(!first||first!==last||!article.contains(first))return {status:'cross-span'};
  const before=range.cloneRange();before.selectNodeContents(first);before.setEnd(range.startContainer,range.startOffset);
  const index=Number(first.dataset.nativeSpan),base=result.native_unit.spans[index].selector.start;
  const start=base+Array.from(before.toString()).length,end=start+Array.from(range.toString()).length;
  const reference=createNativeReference(result,index,{start,end});
  return {status:'exact',reference,spanIndex:index,quote:nativeSelectedText(reference,result.native_unit.spans[index].text)};
}

export function mountNativeReader({host=document.body,client,notebook,locale=()=> 'ru',onGraphRequest,onLegacyReference,onOpen,onClose}={}){
  const ru=()=>!String(locale()||'ru').toLowerCase().startsWith('en'),word=(a,b)=>ru()?a:b;
  const root=el('section','','native-reader');root.hidden=true;root.tabIndex=-1;root.setAttribute('role','dialog');root.setAttribute('aria-modal','true');
  root.setAttribute('aria-label',word('Чтение источника','Source reading'));
  const header=el('header','','nr-header'),title=el('h2',word('Текст источника','Source text'));
  const back=action(word('К Древу','Back to the tree'),()=>close());
  const notesButton=action(word('Мои записи','My notes'),()=>showNotes());
  header.append(back,title,notesButton);
  const status=el('p','','nr-status');status.setAttribute('role','status');
  const layout=el('div','','nr-layout'),article=el('article','','nr-article');article.tabIndex=0;
  const sidebar=el('aside','','nr-sidebar'),selectedLabel=el('p','','nr-selected'),quote=el('blockquote','','nr-quote');
  const input=el('textarea');input.maxLength=64000;input.setAttribute('aria-label',word('Заметка к фрагменту','Passage note'));
  const save=action(word('Сохранить заметку','Save note'),()=>saveDraft());
  const bookmark=action(word('Закладка','Bookmark'),()=>saveBookmark());
  const graph=action(word('В пространстве','In the graph'),()=>goToGraph());
  const noteActions=el('div','','nr-note-actions');noteActions.append(save,bookmark,graph);
  const notes=el('section','','nr-notes');notes.hidden=true;
  const updateLabels=()=>{root.setAttribute('aria-label',word('Чтение источника','Source reading'));
    for(const [button,a,b]of [[back,'К Древу','Back to the tree'],[notesButton,'Мои записи','My notes'],[save,'Сохранить заметку','Save note'],[bookmark,'Закладка','Bookmark'],[graph,'В пространстве','In the graph']])button.textContent=word(a,b);
    input.setAttribute('aria-label',word('Заметка к фрагменту','Passage note'));};
  sidebar.append(selectedLabel,quote,input,noteActions,notes);layout.append(article,sidebar);root.append(header,status,layout);host.append(root);
  let result=null,reference=null,selectedQuote='',note=null,dirty=false,saving=null,opened=false,destroyed=false;
  let request=null,generation=0,notesGeneration=0,restoreFocus=null,cancelExpiry=()=>{},positionTimer=null,positionWork=Promise.resolve();
  let expired=false,lastAddress=null;
  const announce=text=>{status.textContent=text;};
  const unavailable=error=>{
    const key=String(error?.code??error?.status??'').toLowerCase();
    if(key==='stale')return word('Текст изменился. Старая привязка сохранена.','The text changed. The old reference was kept.');
    if(key==='access-restricted'||key==='restricted'||key==='403')return word('Доступ к исходному тексту ограничен.','Access to the source text is restricted.');
    if(key==='missing'||key==='404'||key==='unavailable'||key==='unavailable_reference')return word('Исходный текст недоступен.','The source text is unavailable.');
    if(key==='unsupported'||key==='source-owner-reader-not-configured')return word('Этот источник пока не подключён к чтению.','This source is not connected to reading yet.');
    if(key==='corrupt')return word('Исходный текст не прошёл проверку.','The source text did not pass validation.');
    if(key==='over-budget'||key==='limit')return word('Фрагмент слишком велик для чтения здесь.','This passage is too large to read here.');
    if(key==='conflict')return word('Запись изменилась в другой вкладке.','The note changed in another tab.');
    if(key==='storage')return word('Положение чтения пока не сохранено.','The reading position was not saved.');
    if(key==='504'||key==='timeout')return word('Чтение источника заняло слишком много времени. Повторите попытку.','Source reading took too long. Try again.');
    return word('Не удалось открыть исходный текст. Повторите попытку.','The source text could not be opened. Try again.');
  };
  root.addEventListener('native-reader-error',event=>announce(unavailable(event.detail)));
  const scroller=()=>getComputedStyle(article).overflowY==='visible'?layout:article;
  function controls(){
    const usable=Boolean(reference&&result&&!expired);
    sidebar.hidden=!reference&&!result&&notes.hidden;layout.classList.toggle('nr-empty',sidebar.hidden);
    bookmark.disabled=!usable;input.disabled=!reference;save.disabled=!reference||!dirty||Boolean(saving);graph.disabled=!reference;
    selectedLabel.textContent=reference?word('Выделенный фрагмент · личная запись','Selected passage · personal note'):word('Выделите текст для заметки.','Select text to annotate.');
    quote.textContent=selectedQuote;quote.dir='auto';
    root.dataset.nativeState=expired?'expired':result?'available':reference?'unavailable':'empty';
  }
  function setReference(value,{text='',saved=null}={}){
    reference=validateNativeReference(value);selectedQuote=text;note=saved;input.value=saved?.text??'';dirty=false;controls();
  }
  async function saveDraft(){
    if(saving)return saving;
    if(!dirty||!reference)return true;
    const text=input.value;if(!text.trim()){announce(word('Напишите текст заметки или оставьте закладку.','Write a note or keep a bookmark.'));return false;}
    const selected=structuredClone(reference),original=note,currentText=text;
    saving=(async()=>{
      try{
        const saved=await notebook.putNote({kind:'note',reference:selected,text:currentText,...(selectedQuote?{quote:selectedQuote}:{}),
          ...(original?{id:original.id,expectedRecordRevision:original.revision}:{expectedRecordRevision:null})});
        note=saved.item;dirty=input.value!==currentText;
        announce(notebook.status().persistent?word('Заметка сохранена.','Note saved.'):word('Запись пока хранится только в памяти. Экспортируйте её перед закрытием.','This note is only in memory. Export it before closing.'));
        if(!notes.hidden)void showNotes();return !dirty;
      }catch(error){announce(error.code==='conflict'?word('Заметку изменила другая вкладка. Ваш текст остался здесь; сохраните его новой записью.','Another tab changed this note. Your text is still here; save it as a new note.'):unavailable(error));
        if(error.code==='conflict'&&!sidebar.querySelector('[data-save-copy]')){const copy=action(word('Сохранить новой записью','Save as a new note'),async()=>{note=null;copy.remove();await saveDraft();});copy.dataset.saveCopy='true';sidebar.append(copy);}return false;
      }
    })();controls();
    try{return await saving;}finally{saving=null;controls();}
  }
  async function saveBookmark(){
    if(!reference||expired)return;
    try{await notebook.putNote({kind:'bookmark',reference,expectedRecordRevision:null});announce(word('Закладка сохранена.','Bookmark saved.'));if(!notes.hidden)await showNotes();}catch(error){announce(unavailable(error));}
  }
  function updateLocation(){
    if(!reference)return;
    const url=new URL(location.href);url.hash=HASH+encodeURIComponent(JSON.stringify(reference));history.replaceState(history.state,'',url);
  }
  function persistPosition(){
    clearTimeout(positionTimer);if(!result||!reference||expired)return positionWork;
    const selected=structuredClone(reference),offset=Math.max(0,Math.round(scroller().scrollTop));
    positionWork=positionWork.catch(()=>{}).then(()=>notebook.saveReading({slot:readingSlot(selected),documentId:referenceDocumentId(selected),
      versionId:referenceVersionId(selected),reference:selected,offset})).catch(error=>announce(unavailable({...error,code:error?.code||'storage'})));
    return positionWork;
  }
  function selectedDOM(ref){
    const span=result.native_unit.spans.findIndex(value=>value.anchor_ref===ref.target.span.anchorRef),node=article.querySelector(`[data-native-span="${span}"]`);
    if(!node?.firstChild)return;
    const points=Array.from(node.textContent),base=ref.target.span.start;
    const range=document.createRange();range.setStart(node.firstChild,points.slice(0,ref.selector.start-base).join('').length);
    range.setEnd(node.firstChild,points.slice(0,ref.selector.end-base).join('').length);
    const selection=getSelection();selection.removeAllRanges();selection.addRange(range);
  }
  function renderText(){
    cancelExpiry();expired=false;article.replaceChildren();
    const unit=result.native_unit;
    if(unit.spans.length>128)throw Object.assign(new Error('native-span-limit'),{code:'limit'});
    title.textContent=word('Текст источника','Source text');
    const language=unit.summary.language;
    unit.spans.forEach((span,index)=>{
      if(index){const gap=el('p',word('Отдельный фрагмент источника','Separate source span'),'nr-span-gap');article.append(gap);}
      const text=el('pre',span.text,'nr-span-text');text.dataset.nativeSpan=String(index);text.dir='auto';if(language)text.lang=language;article.append(text);
    });
    if(unit.local_conditions){
      const conditions=el('section','','nr-conditions');conditions.append(el('h3',word('Условия локального чтения','Local reading conditions')),
        el('p',unit.local_conditions.condition_review));
      const expiryText=el('p');expiryText.textContent=word('Срок: ','Expires: ');
      try{expiryText.textContent+=new Intl.DateTimeFormat(ru()?'ru':'en',{dateStyle:'medium',timeStyle:'short'}).format(new Date(unit.local_conditions.expires_at));}
      catch{expiryText.textContent+=word('срок указан источником','deadline supplied by the source');}
      conditions.append(expiryText);
      const noticeRoles={license:word('Лицензия','License'),attribution:word('Атрибуция','Attribution'),notice:word('Уведомление','Notice')};
      for(const notice of unit.local_conditions.notices){const text=el('pre',notice.text);text.dir='auto';conditions.append(el('h4',noticeRoles[notice.role]||word('Уведомление','Notice')),text);}
      article.append(conditions);
      const expire=()=>{expired=true;article.replaceChildren(el('p',word('Срок условий чтения истёк. Откройте источник повторно для проверки.','The reading conditions expired. Reopen the source to recheck access.')));result=null;controls();};
      cancelExpiry=scheduleExpiry(unit.local_conditions.expires_at,expire);
    }
    const exact=el('details','','nr-exact');exact.append(el('summary',word('Об источнике','About the source')));
    const facts=el('div','','nr-exact-facts');
    const access=result?.text_access;
    if(access?.scope==='public-native-unit')facts.append(el('p',word('Открытый текст источника.','Public source text.')));
    if(access?.scope==='local-native-unit')facts.append(el('p',word('Локальный текст источника.','Local source text.')));
    if(unit.summary?.content_verified===true)facts.append(el('p',word('Текст проверен по исходному слою.','The text was checked against the source layer.')));
    if(access?.recorded_rights_verified===true)facts.append(el('p',word('Условия чтения указаны источником.','Reading conditions are supplied by the source.')));
    exact.append(facts);
    const deliveryGeneration=generation;
    // Resolve at click time: detached controls cannot retain text after
    // close, replacement or expiry, including a delayed expiry timer.
    const exportButton=rawDataDownload(()=>{
      if(deliveryGeneration!==generation||!opened||!result||expired)return;
      const deadline=result.native_unit.local_conditions?.expires_at;
      if(deadline&&Date.parse(deadline)<=Date.now())return;
      return sourceReadExport(result);
    },word('Скачать данные','Download data'),'tos-native-source.json');
    exact.append(exportButton);article.append(exact);
  }
  function reveal(){if(!opened)restoreFocus=document.activeElement;opened=true;root.hidden=false;onOpen?.();root.focus({preventScroll:true});}
  async function open(address={}){
    updateLabels();
    if(destroyed||!(await saveDraft()))return false;
    await persistPosition();request?.abort();const ticket=++generation;request=new AbortController();reveal();lastAddress=address;
    cancelExpiry();result=null;expired=false;article.replaceChildren();announce(word('Читаю источник…','Reading the source…'));
    if(address.reference){setReference(address.reference,{text:address.note?.quote??'',saved:address.note??null});}else{reference=null;note=null;selectedQuote='';input.value='';dirty=false;controls();}
    try{
      if(address.reference){const resolved=await readNativeReference(client,address.reference,{signal:request.signal});if(ticket!==generation)return false;result=resolved.result;}
      else{const read=await readNativeSelection(client,address.selection,{signal:request.signal,representation:address.representation});if(ticket!==generation)return false;
        if(read.status!=='available')throw Object.assign(new Error('native-text-unavailable'),{code:read.status||'unavailable'});result=read;}
      if(!reference)setReference(createNativeReference(result,0,{end:result.native_unit.spans[0].selector.start}));
      const selectedSpan=result.native_unit.spans.find(span=>span.anchor_ref===reference.target.span.anchorRef);
      selectedQuote=nativeSelectedText(reference,selectedSpan.text);
      renderText();controls();updateLocation();
      if(expired||!result)return false;
      const remembered=await notebook.loadReading(readingSlot(reference));if(ticket!==generation||expired||!result)return false;
      scroller().scrollTop=address.note?0:remembered?.offset??0;article.focus({preventScroll:true});selectedDOM(reference);
      if(address.note||!remembered)article.querySelector(`[data-native-span="${result.native_unit.spans.indexOf(selectedSpan)}"]`)?.scrollIntoView({block:'center'});
      announce('');
      return true;
    }catch(error){if(ticket!==generation||request?.signal.aborted)return false;result=null;controls();announce(unavailable(error));
      article.replaceChildren();
      if(!['unsupported','source-owner-reader-not-configured'].includes(error?.code??error?.status))article.append(action(word('Повторить чтение','Retry reading'),()=>open(lastAddress)));
      return false;}
  }
  async function showNotes(cursor=null){
    notes.hidden=false;sidebar.hidden=false;layout.classList.remove('nr-empty');const ticket=++notesGeneration;
    try{const page=await notebook.listNotes({limit:24,cursor});if(destroyed||ticket!==notesGeneration)return;
      notes.replaceChildren(el('h3',word('Мои записи','My notes')));
      for(const item of page.items){
        const native=item.reference.schemaVersion===NATIVE_REFERENCE_SCHEMA;
        const entry=action(item.text||item.quote||word('Закладка','Bookmark'),async()=>{
          if(native)await open({reference:item.reference,note:item});else if(await saveDraft()){await close();onLegacyReference?.(item.reference);}});
        entry.dataset.noteId=item.id;notes.append(entry);
      }
      if(!page.items.length)notes.append(el('p',word('Пока нет записей.','No saved notes yet.')));
      if(page.nextCursor)notes.append(action(word('Следующие записи','Next notes'),()=>showNotes(page.nextCursor)));
      if(cursor)notes.append(action(word('К первым записям','First notes'),()=>showNotes()));
      notes.append(action(word('Экспорт записей','Export notes'),async()=>{if(!(await saveDraft()))return;const packet=await notebook.exportPacket();
        const url=URL.createObjectURL(new Blob([JSON.stringify(packet,null,2)],{type:'application/json'}));const link=el('a');link.href=url;link.download='tos-notebook.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}));
    }catch(error){announce(unavailable(error));}
  }
  async function close(){
    if(destroyed)return true;
    if(!(await saveDraft())||destroyed)return false;await persistPosition();
    if(destroyed)return true;
    generation++;request?.abort();opened=false;root.hidden=true;cancelExpiry();
    if(location.hash.startsWith(HASH)){const url=new URL(location.href);url.hash='';history.replaceState(history.state,'',url);}
    result=null;reference=null;selectedQuote='';note=null;input.value='';article.replaceChildren();controls();
    onClose?.();restoreFocus?.isConnected&&restoreFocus.focus({preventScroll:true});return true;
  }
  async function goToGraph(){
    if(!reference||!(await saveDraft()))return;
    try{await onGraphRequest?.(reference.origin,reference);await close();}catch(error){announce(unavailable(error));}
  }
  const selectionChanged=async()=>{
    if(!opened||!result||expired)return;const selection=getSelection();if(!selection?.rangeCount||selection.isCollapsed)return;
    const range=selection.getRangeAt(0);if(!article.contains(range.commonAncestorContainer))return;
    const value=nativeRangeSelection(range,article,result);if(!value)return;
    if(value.status!=='exact'){announce(word('Для одной заметки выделите текст внутри одного исходного фрагмента.','Select within one source span for a single note.'));return;}
    const ticket=generation;if(!(await saveDraft())||ticket!==generation)return;setReference(value.reference,{text:value.quote});updateLocation();void persistPosition();
  };
  article.addEventListener('pointerup',selectionChanged);article.addEventListener('keyup',selectionChanged);
  const onScroll=()=>{clearTimeout(positionTimer);positionTimer=setTimeout(persistPosition,350);};
  article.addEventListener('scroll',onScroll,{passive:true});layout.addEventListener('scroll',onScroll,{passive:true});
  input.addEventListener('input',()=>{dirty=true;controls();});
  root.addEventListener('keydown',event=>{
    event.stopPropagation();
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();void close();}
    if(event.key==='Tab'){const focusable=[...root.querySelectorAll('button,textarea,summary,a[href],input,select,[tabindex="0"]')].filter(node=>!node.disabled&&!node.closest('[hidden]'));
      const first=focusable[0],last=focusable.at(-1);if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}
      else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}}
  });
  const beforeUnload=event=>{if(dirty||saving){event.preventDefault();event.returnValue='';}};
  window.addEventListener('beforeunload',beforeUnload);
  const restore=()=>{if(!location.hash.startsWith(HASH))return;try{if(location.hash.length>32768)throw new Error('Native address is too large.');
    void open({reference:validateNativeReference(JSON.parse(decodeURIComponent(location.hash.slice(HASH.length))))});}catch(error){reveal();announce(unavailable(error));}};
  window.addEventListener('hashchange',restore);restore();controls();
  return {root,open,close,async openNotes(){if(!(await saveDraft()))return;updateLabels();reveal();await showNotes();},
    destroy(){destroyed=true;generation++;notesGeneration++;request?.abort();cancelExpiry();clearTimeout(positionTimer);root.remove();window.removeEventListener('hashchange',restore);window.removeEventListener('beforeunload',beforeUnload);},
    async flush(){
      // Host pagehide waits for this before native.destroy() removes the
      // scroller. A note failure must not discard an independently captured
      // reading position.
      const position=persistPosition();
      let notes=true;
      try{notes=await saveDraft();}finally{await position;}
      return notes;
    }};
}
