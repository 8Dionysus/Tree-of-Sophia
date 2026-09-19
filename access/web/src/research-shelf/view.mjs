import {ui,uiLanguage} from '../observatory/ui-i18n.mjs';
import {createResearchShelfStore} from './storage.mjs';
import './research-shelf.css';

const own=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const copy=value=>typeof structuredClone==='function'?structuredClone(value):JSON.parse(JSON.stringify(value));
const typeNames={
  ru:{material:'Материал',form:'Форма',text:'Текстовый фрагмент',lens:'Линза',route:'Маршрут'},
  en:{material:'Material',form:'Form',text:'Text passage',lens:'Lens',route:'Route'},
  es:{material:'Material',form:'Forma',text:'Fragmento de texto',lens:'Lente',route:'Ruta'},
};

function language(locale){
  const value=typeof locale==='function'?locale():locale;
  return value==='en'||value==='es'?value:'ru';
}
function authored(locale,ru,en,es){
  // `ui` remains the shared catalog seam for Russian authored labels. New
  // shelf-specific wording stays local until its strings are accepted into
  // the common catalog; source/user text never passes through this helper.
  const selected=language(locale);
  return selected==='ru'?(uiLanguage()==='ru'?ui(ru):ru):selected==='es'?es:en;
}
const text=(tag,value='',className='')=>{const node=document.createElement(tag);node.textContent=String(value??'');if(className)node.className=className;return node;};
const button=(label,run,className='')=>{const node=text('button',label,className);node.type='button';node.addEventListener('click',()=>void Promise.resolve().then(run));return node;};
const titleOf=(record,locale)=>record.title||`${typeNames[language(locale)][record.type]??record.type} · ${record.id}`;
function targetSummary(record,locale='ru'){
  // Exact ids and revisions stay in the disclosure below each card. The
  // compact line names only the user-facing type; the title carries the
  // personal label chosen for the record.
  return typeNames[language(locale)][record?.type]??typeNames.ru[record?.type]??record?.type;
}
function noteText(note,locale){return note?.text||note?.quote||authored(locale,'Заметка','Note','Nota');}
function errorLabel(error,locale){
  const code=error?.code,lang=language(locale);
  const labels={
    conflict:{ru:'Запись изменилась в другом контексте.',en:'The record changed in another context.',es:'El registro cambió en otro contexto.'},
    'invalid-cursor':{ru:'Эта страница устарела. Загрузите её снова.',en:'This page is stale. Load it again.',es:'Esta página está desactualizada. Cárgala de nuevo.'},
    quota:{ru:'Браузер исчерпал место для полки.',en:'The browser ran out of shelf storage.',es:'El navegador se quedó sin espacio para el estante.'},
    'storage-unavailable':{ru:'Постоянное хранилище недоступно; записи остаются в памяти.',en:'Persistent storage is unavailable; records remain in memory.',es:'El almacenamiento persistente no está disponible; los registros quedan en memoria.'},
  };
  return labels[code]?.[lang]??error?.message??String(error);
}

/**
 * Mount the contextual local research shelf. `onOpen` receives a cloned shelf
 * record, or `{type:'text',target:{reference},note}` for an existing notebook
 * note. No source text is copied into the shelf. The optional corpus notebook
 * remains owned by its caller.
 */
export function mountResearchShelf({host,locale='ru',onOpen,onError,onOpenView,onCloseView,notebook,store,storage,dbName,indexedDB}={}){
  if(!host||typeof host.append!=='function')throw new TypeError('A shelf host element is required.');
  const shelf=store??createResearchShelfStore({storage,dbName,...(indexedDB===undefined?{}:{indexedDB})});
  const ownsStore=!store;
  const root=text('section','','research-shelf');root.hidden=true;root.tabIndex=-1;root.setAttribute('role','dialog');root.setAttribute('aria-modal','true');
  const heading=text('h2',authored(locale,'Исследовательская полка','Research shelf','Estante de investigación'));heading.id='research-shelf-title';root.setAttribute('aria-labelledby',heading.id);
  const closeButton=button(authored(locale,'Закрыть полку','Close shelf','Cerrar estante'),()=>close());closeButton.className='research-shelf-close';
  const header=text('header','','research-shelf-header');header.append(heading,closeButton);
  const status=text('p','','research-shelf-status');status.setAttribute('role','status');
  const toolbar=text('div','','research-shelf-toolbar');
  const search=text('input');search.type='search';search.maxLength=256;search.placeholder=String(authored(locale,'Поиск на загруженной странице','Search this loaded page','Buscar en esta página cargada'));search.setAttribute('aria-label',search.placeholder);
  const typeFilter=document.createElement('select');typeFilter.setAttribute('aria-label',String(authored(locale,'Тип','Type','Tipo')));
  const collectionFilter=document.createElement('select');collectionFilter.setAttribute('aria-label',String(authored(locale,'Подборка','Collection','Colección')));
  const deleteCollectionButton=button(authored(locale,'Удалить подборку','Delete collection','Eliminar colección'),()=>deleteCollection());deleteCollectionButton.className='research-shelf-delete-collection';
  const previous=button(authored(locale,'Предыдущая страница','Previous page','Página anterior'),()=>goPrevious());
  const next=button(authored(locale,'Следующая страница','Next page','Página siguiente'),()=>loadPage(page?.nextCursor||null,true));
  const exportButton=button(authored(locale,'Экспорт записей','Export records','Exportar registros'),()=>exportShelf());
  const importButton=button(authored(locale,'Импорт записей','Import records','Importar registros'),()=>fileInput.click());
  const fileInput=document.createElement('input');fileInput.type='file';fileInput.accept='application/json,.json';fileInput.hidden=true;fileInput.setAttribute('aria-label',String(authored(locale,'Файл полки','Shelf file','Archivo del estante')));
  const newCollection=text('input');newCollection.type='text';newCollection.maxLength=128;newCollection.placeholder=String(authored(locale,'Новая подборка','New collection','Nueva colección'));newCollection.setAttribute('aria-label',newCollection.placeholder);
  const addCollection=button(authored(locale,'Добавить подборку','Add collection','Añadir colección'),()=>createCollection());
  for(const [value,label] of [['','Все типы'],['material','Материал'],['form','Форма'],['text','Текст'],['lens','Линза'],['route','Маршрут']]){const option=document.createElement('option');option.value=value;option.textContent=String(language(locale)==='en'?({ '':'All types',material:'Material',form:'Form',text:'Text',lens:'Lens',route:'Route'})[value]:language(locale)==='es'?({ '':'Todos los tipos',material:'Material',form:'Forma',text:'Texto',lens:'Lente',route:'Ruta'})[value]:label);typeFilter.append(option);}
  const collectionGroup=text('label','');collectionGroup.append(newCollection,addCollection);
  toolbar.append(search,typeFilter,collectionFilter,deleteCollectionButton,previous,next,exportButton,importButton,fileInput,collectionGroup);
  const content=text('div','','research-shelf-content');
  root.append(header,status,toolbar,content);host.append(root);

  let opened=false,destroyed=false,loading=false,page=null,collections=[],notesPage=null,notesLoading=false;
  let pageCursor=null,history=[],notesCursor=null,notesHistory=[],requestToken=0,notesRequestToken=0,query='',focusReturn=null;
  const currentFilter=()=>({type:typeFilter.value||null,collectionId:collectionFilter.value||null});
  const report=error=>{
    if(destroyed)return;
    status.textContent=errorLabel(error,locale);
    status.dataset.state='error';
    try{onError?.(error);}catch{}
  };
  const announce=value=>{status.textContent=String(value??'');status.dataset.state='ok';};
  const memoryNotice=()=>{
    const value=shelf.status();
    if(value.warning==='memory-only')return authored(locale,'Полка сейчас хранится только в памяти. Экспортируйте записи перед закрытием страницы.','The shelf is currently memory-only. Export records before closing the page.','El estante solo está en memoria. Exporte los registros antes de cerrar la página.');
    if(value.adapter==='memory')return authored(locale,'Постоянное хранилище недоступно; полка работает в памяти.','Persistent storage is unavailable; the shelf is running in memory.','El almacenamiento persistente no está disponible; el estante funciona en memoria.');
    return null;
  };
  function renderFilters(){
    const selected=collectionFilter.value;
    collectionFilter.replaceChildren();
    const all=document.createElement('option');all.value='';all.textContent=String(authored(locale,'Все подборки','All collections','Todas las colecciones'));collectionFilter.append(all);
    for(const collection of collections){const option=document.createElement('option');option.value=collection.id;option.textContent=collection.title;collectionFilter.append(option);}
    collectionFilter.value=collections.some(item=>item.id===selected)?selected:'';
    const selectedCollection=collections.find(item=>item.id===collectionFilter.value);
    deleteCollectionButton.disabled=!selectedCollection||loading;
    previous.disabled=!history.length||loading;next.disabled=!page?.nextCursor||loading;
  }
  function pageItems(){
    const needle=query.trim().toLocaleLowerCase(language(locale));
    if(!needle)return page?.items??[];
    return (page?.items??[]).filter(record=>`${record.title} ${targetSummary(record,locale)}`.toLocaleLowerCase(language(locale)).includes(needle));
  }
  function collectionChecks(record,card){
    if(!collections.length){const empty=text('small',authored(locale,'Подборок пока нет.','No collections yet.','Aún no hay colecciones.'));card.append(empty);return;}
    const group=text('fieldset','','research-shelf-collections');group.append(text('legend',authored(locale,'Личные подборки','Personal collections','Colecciones personales')));
    for(const collection of collections){const label=text('label');const checkbox=document.createElement('input');checkbox.type='checkbox';checkbox.value=collection.id;checkbox.checked=record.collectionIds.includes(collection.id);label.append(checkbox,text('span',collection.title));group.append(label);}
    card.append(group);
  }
  function showConflict(card,record,draft){
    card.querySelector('[data-conflict]')?.remove();
    const area=text('div','','research-shelf-conflict');area.dataset.conflict='true';area.append(text('p',authored(locale,'Запись изменилась в другой вкладке. Ваши правки сохранены в этой форме.','Another tab changed this record. Your edits remain in this form.','Otra pestaña cambió este registro. Tus ediciones siguen en este formulario.')),
      button(authored(locale,'Сохранить копию','Save a copy','Guardar una copia'),async()=>{try{const value=await shelf.save({title:draft.title,type:record.type,target:record.target,collectionIds:draft.collectionIds});announce(authored(locale,'Копия сохранена как личная запись.','A personal copy was saved.','Se guardó una copia personal.'));await loadPage(null,false);return value;}catch(error){report(error);}}));
    card.append(area);
  }
  function recordCard(record){
    const card=text('article','','research-shelf-card');card.dataset.recordId=record.id;
    const cardHeader=text('div','','research-shelf-card-header');const title=text('h3',titleOf(record,locale));title.dir='auto';cardHeader.append(title,text('span',String(authored(locale,'Личная запись','Personal record','Registro personal')),'research-shelf-personal'));card.append(cardHeader);
    const meta=text('p',targetSummary(record,locale),'research-shelf-meta');meta.dir='auto';card.append(meta);
    const titleInput=document.createElement('input');titleInput.type='text';titleInput.maxLength=256;titleInput.value=record.title;titleInput.setAttribute('aria-label',String(authored(locale,'Название записи','Record title','Título del registro')));titleInput.className='research-shelf-title-input';
    const actions=text('div','','research-shelf-card-actions');const open=button(authored(locale,'Открыть','Open','Abrir'),async()=>{try{await onOpen?.(copy(record));}catch(error){report(error);}});const save=button(authored(locale,'Сохранить запись','Save record','Guardar registro'),async()=>{
      const selected=[...card.querySelectorAll('input[type="checkbox"]:checked')].map(input=>input.value);const draft={title:titleInput.value,type:record.type,target:record.target,collectionIds:selected};
      try{await shelf.save({id:record.id,...draft},{expectedRevision:record.revision});announce(authored(locale,'Запись сохранена.','Record saved.','Registro guardado.'));await loadPage(null,false);}catch(error){if(error?.code==='conflict')showConflict(card,record,draft);else report(error);}
    });const remove=button(authored(locale,'Удалить запись','Delete record','Eliminar registro'),async()=>{
      try{await shelf.remove(record.id,record.revision);announce(authored(locale,'Запись удалена.','Record deleted.','Registro eliminado.'));await loadPage(null,false);}catch(error){if(error?.code==='conflict'){showConflict(card,record,{title:titleInput.value,collectionIds:[...card.querySelectorAll('input[type="checkbox"]:checked')].map(input=>input.value)});}else report(error);}
    });actions.append(open,save,remove);card.append(titleInput,actions);collectionChecks(record,card);
    const exact=text('details','','research-shelf-exact');exact.append(text('summary',authored(locale,'Точный адрес','Exact address','Dirección exacta')),text('pre',JSON.stringify(record.target,null,2)));card.append(exact);
    return card;
  }
  function renderNotes(){
    if(!notebook)return null;
    const section=text('section','','research-shelf-notebook');section.append(text('h3',authored(locale,'Сохранённые заметки','Saved notes','Notas guardadas')));
    if(notesLoading&&!notesPage){section.append(text('p',authored(locale,'Заметки загружаются…','Loading notes…','Cargando notas…')));return section;}
    if(!notesPage){section.append(text('p',authored(locale,'Заметки пока недоступны.','Notes are not available yet.','Las notas aún no están disponibles.')));return section;}
    if(!notesPage.items.length)section.append(text('p',authored(locale,'Сохранённых заметок пока нет.','No saved notes yet.','Aún no hay notas guardadas.')));
    else{
      const list=text('ul');
      for(const note of notesPage.items){
        const item=text('li');
        item.append(text('span',noteText(note,locale)));
        const actions=text('div','','research-shelf-note-actions');
        actions.append(button(authored(locale,'Открыть заметку','Open note','Abrir nota'),async()=>{
          try{await onOpen?.({type:'text',target:{reference:note.reference},note});}catch(error){report(error);}
        }));
        item.append(actions);
        const ref=text('details');ref.append(text('summary',authored(locale,'Точная ссылка','Exact reference','Referencia exacta')),text('pre',JSON.stringify(note.reference,null,2)));item.append(ref);list.append(item);
      }
      section.append(list);
    }
    const controls=text('div','','research-shelf-note-pagination');
    const previousNotes=button(authored(locale,'Предыдущие заметки','Previous notes','Notas anteriores'),()=>{if(!notesHistory.length)return;void loadNotes(notesHistory.pop()??null,false);});
    const nextNotes=button(authored(locale,'Следующие заметки','Next notes','Siguientes notas'),()=>loadNotes(notesPage?.nextCursor||null,true));
    previousNotes.disabled=!notesHistory.length||notesLoading;nextNotes.disabled=!notesPage.nextCursor||notesLoading;controls.append(previousNotes,nextNotes);section.append(controls);
    return section;
  }
  function renderPage(){
    content.replaceChildren();const notice=memoryNotice();if(notice)content.append(text('p',notice,'research-shelf-memory'));if(loading){content.append(text('p',authored(locale,'Загружается…','Loading…','Cargando…')));renderFilters();return;}
    const items=pageItems();if(!items.length)content.append(text('p',query?authored(locale,'На этой загруженной странице ничего не найдено.','Nothing matched on this loaded page.','Nada coincide en esta página cargada.'):authored(locale,'Полка пока пуста.','The shelf is empty.','El estante está vacío.')));
    else{const list=text('div','','research-shelf-list');for(const record of items)list.append(recordCard(record));content.append(list);}
    const notes=renderNotes();if(notes)content.append(notes);renderFilters();
  }
  function goPrevious(){if(!history.length)return;const cursor=history.pop()??null;void loadPage(cursor,false);}
  async function loadCollections(){const result=await shelf.listCollections();collections=result.items;renderFilters();}
  async function loadNotes(cursor=null,forward=false){
    if(!notebook||typeof notebook.listNotes!=='function'){notesPage=null;return null;}
    if(forward&&notesPage?.nextCursor)notesHistory.push(notesCursor);
    else if(!forward&&cursor===null)notesHistory=[];
    const token=++notesRequestToken;notesCursor=cursor;notesLoading=true;renderPage();
    try{
      const options={limit:24};if(cursor)options.cursor=cursor;
      const result=await notebook.listNotes(options);if(destroyed||!opened||token!==notesRequestToken)return null;
      notesPage=result;notesLoading=false;renderPage();return result;
    }catch(error){if(destroyed||token!==notesRequestToken)return null;notesLoading=false;renderPage();report(error);return null;}
  }
  async function loadNotebook(){
    if(!notebook||typeof notebook.listNotes!=='function'){notesPage=null;return;}
    await loadNotes(null,false);
  }
  async function loadPage(cursor=null,forward=false){
    if(destroyed)return null;
    if(forward&&page?.nextCursor)history.push(pageCursor);
    else if(!forward&&cursor===null)history=[];
    const token=++requestToken;loading=true;pageCursor=cursor;renderPage();
    try{const result=await shelf.list({limit:24,cursor,...currentFilter()});if(destroyed||!opened||token!==requestToken)return null;page=result;loading=false;announce(memoryNotice()??'');renderPage();return result;}
    catch(error){if(destroyed||!opened||token!==requestToken)return null;loading=false;renderPage();report(error);return null;}
  }
  async function createCollection(){
    const value=newCollection.value.trim();if(!value){announce(authored(locale,'Введите название подборки.','Enter a collection name.','Escribe un nombre de colección.'));newCollection.focus();return;}
    try{await shelf.saveCollection({title:value});newCollection.value='';await loadCollections();announce(authored(locale,'Подборка создана.','Collection created.','Colección creada.'));renderPage();}catch(error){report(error);}
  }
  async function deleteCollection(){
    const selected=collections.find(item=>item.id===collectionFilter.value);if(!selected)return;
    try{await shelf.removeCollection(selected.id,selected.revision);announce(authored(locale,'Подборка удалена.','Collection deleted.','Colección eliminada.'));await loadCollections();history=[];await loadPage(null,false);}
    catch(error){report(error);}
  }
  async function exportShelf(){
    try{const packet=await shelf.export(),body=JSON.stringify(packet,null,2);if(typeof Blob==='undefined'||typeof URL==='undefined'||typeof URL.createObjectURL!=='function'){announce(body);return;}const url=URL.createObjectURL(new Blob([body],{type:'application/json'}));const link=document.createElement('a');link.href=url;link.download='tos-research-shelf.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);announce(authored(locale,'Экспорт готов.','Export ready.','Exportación lista.'));}catch(error){report(error);}
  }
  fileInput.addEventListener('change',async event=>{const file=event.target.files?.[0];fileInput.value='';if(!file)return;if(file.size>4_000_000){announce(authored(locale,'Файл полки слишком велик.','The shelf file is too large.','El archivo del estante es demasiado grande.'));return;}try{const result=await shelf.import(await file.text());announce(`${String(authored(locale,'Импорт добавил записей','Import added records','La importación añadió registros'))}: ${result.counts.records}; ${String(authored(locale,'подборок','collections','colecciones'))}: ${result.counts.collections}.`);await loadCollections();await loadPage(null,false);}catch(error){report(error);}});
  search.addEventListener('input',()=>{query=search.value;renderPage();});
  typeFilter.addEventListener('change',()=>{history=[];void loadPage(null,false);});collectionFilter.addEventListener('change',()=>{history=[];void loadPage(null,false);});
  const focusable=()=>[...root.querySelectorAll('button:not([disabled]),input:not([disabled]):not([type="hidden"]),select:not([disabled]),textarea:not([disabled]),a[href],[tabindex]:not([tabindex="-1"])')].filter(node=>!node.hidden&&!node.closest('[hidden]'));
  const viewHook=(hook,payload)=>{try{const result=hook?.(payload);if(result&&typeof result.then==='function')result.catch(report);}catch(error){report(error);}};
  root.addEventListener('keydown',event=>{
    if(!opened)return;
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();return;}
    if(event.key!=='Tab')return;
    const items=focusable();if(!items.length){event.preventDefault();event.stopPropagation();root.focus({preventScroll:true});return;}
    const first=items[0],last=items.at(-1);
    if(event.shiftKey&&document.activeElement===first){event.preventDefault();event.stopPropagation();last.focus({preventScroll:true});}
    else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();event.stopPropagation();first.focus({preventScroll:true});}
    else event.stopPropagation();
  });
  async function open(){
    if(destroyed)return false;if(opened)return true;
    const active=document.activeElement;focusReturn=active&&active!==document.body&&!root.contains(active)?active:focusReturn;
    opened=true;root.hidden=false;viewHook(onOpenView,{element:root,opener:focusReturn});
    closeButton.focus({preventScroll:true});
    try{await loadCollections();}catch(error){report(error);}await loadNotebook();const result=await loadPage(null,false);return Boolean(result);
  }
  function close(){
    if(!opened)return;opened=false;requestToken+=1;notesRequestToken+=1;root.hidden=true;viewHook(onCloseView,{element:root,opener:focusReturn});
    const target=focusReturn?.isConnected&&!focusReturn.hidden&&!focusReturn.closest('[inert]')?focusReturn:host;
    try{target?.focus?.({preventScroll:true});}catch{}
  }
  async function list(options={}){return shelf.list(options);}
  async function save(input,options={}){const result=await shelf.save(input,options);if(opened)await loadPage(null,false);return result;}
  async function remove(id,expectedRevision){const result=await shelf.remove(id,expectedRevision);if(opened)await loadPage(null,false);return result;}
  async function saveCollection(input,options={}){const result=await shelf.saveCollection(input,options);if(opened){await loadCollections();await loadPage(null,false);}return result;}
  async function removeCollection(id,expectedRevision){const result=await shelf.removeCollection(id,expectedRevision);if(opened){await loadCollections();await loadPage(null,false);}return result;}
  async function destroy(){if(destroyed)return;if(opened)close();destroyed=true;requestToken+=1;notesRequestToken+=1;root.remove();if(ownsStore)await shelf.destroy();}
  renderFilters();
  return {open,close,save,list,remove,saveCollection,removeCollection,destroy,status:()=>shelf.status(),store:shelf,element:root};
}

export default mountResearchShelf;
