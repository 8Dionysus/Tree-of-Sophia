import './live.css';
import '../src/observatory/human-forms.css';
import {mountConstructorSky} from './sky.mjs';
import {createLiveResearch,seekSearchPage,SEARCH_SEEK_WINDOW} from './live-research.mjs';
import {ExplorationSession} from '../src/observatory/exploration-session.mjs';
import {isSourceDossierRef,SOURCE_DOSSIER_LIMIT,specForPacket,localized} from '../src/observatory/knowledge-client.mjs';
import {liveLabel,liveEdgeLabel,livePredicateLabel,liveTypeColor} from './live-model.mjs';
import {readingDocument} from '../src/observatory/reader-model.mjs';
import {renderHumanForms,renderClaimContext,renderEssentialContext} from '../src/observatory/human-forms-view.mjs';
import {setUiLanguage} from '../src/observatory/ui-i18n.mjs';
import {mountSourceCommandPanel} from './source-command-panel.mjs';
import {inclusionSummary} from './inclusion-summary.mjs';
import {mountCorpusEntry} from '../src/corpus-reader/host.mjs';
import {exactSourceRepresentations} from '../src/observatory/exact-source-read.mjs';
import {mountTemporalComparison} from '../src/observatory/temporal-compare.mjs';
import {createLiveResumeStore,makeLiveResume,resolveLiveResumeSelection} from './live-resume.mjs';
import {mountResearchShelfEntry} from '../src/research-shelf/host.mjs';
import {mountLensBuilder} from '../src/observatory/lens-builder.mjs';
import {constructorCatalog,previewDraft} from '../src/observatory/lens-model.mjs';
import {validateRouteTarget} from '../src/research-shelf/model.mjs';
import {RevisionError} from '../src/observatory/knowledge-client.mjs';

const copy={ru:{brand:'ДРЕВО СОФИИ',subtitle:'Исследование исходного знания',search:'Найти предмет или связь',close:'Закрыть',
  view:'ПРЕДСТАВЛЕНИЕ',compact:'Смысловые связи',grouped:'Предметы и записи',raw:'Все носители',conditions:'Условия раскрытия',
  compare:'Сопоставить',continue:'Продолжить раскрытие',overview:'Вместить в поле',motion:'Движение пространства',cinema:'Скрыть интерфейс',
  show:'Показать интерфейс',empty:'С чего начнём исследование?',emptyText:'Найдите слово, мысль, человека, произведение, событие или связь.',
  loading:'Загружаю…',readonly:'Чтение · без изменения источников',searchGo:'Найти',more:'Следующая страница',continueSearch:'Продолжить поиск',
  searchPaused:'Поиск приостановлен после окна подзапроса: {0}/{1} запросов, {2}/{3} байт, {4}/{5} мс. Продолжите поиск.',
  none:'Совпадений на этой странице нет.',node:'Предмет',relation:'Связь',read:'Читать',expand:'Раскрыть окружение',
  newSpace:'Открыть в новом поле',pin:'Закрепить для сравнения',sources:'Источники',technical:'Точные сведения',
  reasons:'Почему включено',noReading:'Выберите звезду или линию для чтения.',cancel:'Отменить запрос',
  profile:'Правило близости',direction:'Направление',either:'В обе стороны',incoming:'Входящие',outgoing:'Исходящие',
  depth:'Глубина',predicates:'Отношения (пусто — все)',sourceGraphs:'Источники графа',apply:'Раскрыть выбранное',
  conditionNote:'Условия относятся к следующему раскрытию. Уже прочитанные области остаются в поле.',
  noTitle:'Название не предоставлено',noDescription:'Описание не предоставлено.',clear:'Убрать закреплённые карточки',
  noPins:'Закрепите до двух материалов из карточки выбранного предмета или связи.',
  discoveryFailed:'Этот backend не предоставил совместимый контракт исследования. Исходные данные не изменены.',
  retry:'Повторить подключение',retryReading:'Повторить чтение',selected:'Выбранный материал',complete:'Раскрытие завершено',space:'Локальное поле чтения',
  sourceRecord:'Открыть исходную запись',sourceRecordNote:'Это точная публичная запись, связанная с выбранной карточкой. Она не является текстом носителя, оценкой достоверности или разрешением использования.',sourceRecordWords:'Исходные примечания',sourceRecordNoWords:'У записи нет отдельного текстового примечания; её поля доступны в точных сведениях.',sourceRecordUnavailable:'Для этой карточки точное чтение источника пока не доступно.',sourceRecordMissing:'Точная исходная запись не найдена.',sourceRecordRestricted:'Доступ к исходной записи ограничен.',sourceRecordCorrupt:'Целостность исходной записи не подтверждена.',sourceRecordBudget:'Исходная запись превысила границы этого чтения.',
  sourceDossier:'Открыть точное досье источника',sourceChanges:'Изменение источника',sourceNoDossier:'Для этой локальной ссылки безопасный маршрут к досье не заявлен; путь не открывается автоматически.',
  sourceDossierLoading:'Загружаю досье источника…',sourceDossierUnavailable:'Досье источника сейчас недоступно.',sourceNoLinks:'В возвращённой цепочке нет HTTP-ссылки на носитель.',sourceIdentity:'Точная идентичность',sourceRights:'Права и границы',sourceLinks:'Наблюдаемые ссылки',sourceStructure:'Структура досье',sourceRefs:'Ссылки владельца',sourceDossierNote:'Это досье содержит ограниченный набор библиографических сведений и технических ссылок. Оно не выдаёт исходные байты и не подтверждает разрешение на их использование.',sourceDossierVersionNote:'Досье не привязано к версии данных или записи. Версии карточки показаны отдельно и не являются версией досье.',sourceCommandUnconfirmedClose:'В этой операции есть неподтверждённая команда. Перед закрытием сохраните её; иначе повтор станет невозможен.',sourceCommandCopy:'Скопировать сохранённую команду',sourceCommandCopied:'Точная команда скопирована. Теперь можно закрыть окно.',sourceCommandClose:'Закрыть после сохранения',sourceCommandCopyUnavailable:'Не удалось скопировать команду; оставьте окно открытым и повторите попытку.'},
en:{brand:'TREE OF SOPHIA',subtitle:'Explore source-owned knowledge',search:'Find an object or relation',close:'Close',
  view:'PRESENTATION',compact:'Meaningful relations',grouped:'Objects and records',raw:'All carriers',conditions:'Expansion conditions',
  compare:'Compare',continue:'Continue expansion',overview:'Fit the field',motion:'Space motion',cinema:'Hide interface',
  show:'Show interface',empty:'Where shall we begin?',emptyText:'Find a word, thought, person, work, event or relation.',
  loading:'Loading…',readonly:'Reading · no source changes',searchGo:'Search',more:'Next page',continueSearch:'Continue search',
  searchPaused:'Search paused after the seek window: {0}/{1} requests, {2}/{3} bytes, {4}/{5} ms. Continue when ready.',none:'No matches on this page.',
  node:'Object',relation:'Relation',read:'Read',expand:'Expand neighborhood',newSpace:'Open in a new field',pin:'Pin for comparison',
  sources:'Sources',technical:'Exact details',reasons:'Why included',noReading:'Select a star or line to read.',cancel:'Cancel request',
  profile:'Proximity rule',direction:'Direction',either:'Both directions',incoming:'Incoming',outgoing:'Outgoing',depth:'Depth',
  predicates:'Relations (empty means all)',sourceGraphs:'Graph sources',apply:'Expand selection',
  conditionNote:'Conditions apply to the next expansion. Previously read areas remain in the field.',
  noTitle:'Name not supplied',noDescription:'Description not supplied.',clear:'Remove pinned cards',
  noPins:'Pin up to two materials from the selected object or relation card.',
  discoveryFailed:'This backend did not provide a compatible exploration contract. Source data is unchanged.',
  retry:'Reconnect',retryReading:'Retry reading',selected:'Selected material',complete:'Expansion complete',space:'Local reading field',
  sourceRecord:'Open source record',sourceRecordNote:'This is the exact public record bound to the selected card. It is not carrier text, an assessment of truth, or permission to use the material.',sourceRecordWords:'Source notes',sourceRecordNoWords:'This record has no separate textual note; its fields are available in exact details.',sourceRecordUnavailable:'Exact source reading is not available for this card yet.',sourceRecordMissing:'The exact source record was not found.',sourceRecordRestricted:'Access to the source record is restricted.',sourceRecordCorrupt:'Source record integrity could not be confirmed.',sourceRecordBudget:'The source record exceeds this reading budget.',
  sourceDossier:'Open exact source dossier',sourceChanges:'Source changes',sourceNoDossier:'No safe dossier route is advertised for this local reference; the path is not opened automatically.',
  sourceDossierLoading:'Loading bounded source dossier…',sourceDossierUnavailable:'The source dossier is currently unavailable.',sourceNoLinks:'No carrier HTTP link is present in the returned link chain.',sourceIdentity:'Exact identity',sourceRights:'Rights and boundaries',sourceLinks:'Observed links',sourceStructure:'Dossier structure',sourceRefs:'Owner references',sourceDossierNote:'This dossier contains bounded bibliographic context and technical links. It does not deliver source bytes or conclude legal openness.',sourceDossierVersionNote:'The dossier contract carries no source/content revision; card revisions are shown separately and are not a dossier version.',sourceCommandUnconfirmedClose:'This operation has an unconfirmed command. Save it before closing or replay will be impossible.',sourceCommandCopy:'Copy retained command',sourceCommandCopied:'The exact command was copied. You can now close the window.',sourceCommandClose:'Close after saving',sourceCommandCopyUnavailable:'The command could not be copied; keep this window open and try again.'}};
const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=String(text??'');node.className=className;return node;};
const button=(text,callback,className='text-button')=>{const node=el('button',text,className);node.type='button';node.onclick=callback;return node;};
const detail=(title,value)=>{const node=el('details');node.append(el('summary',title),el('pre',JSON.stringify(value,null,2),'live-json'));return node;};

// Explicit mode only. No demo fallback, localStorage migration, source writer,
// external endpoint selection, or automatic consumer activation lives here.
export async function mountLiveResearch(root,{session,skyFactory=mountConstructorSky,url=new URL(location.href)}={}){
  const activeSession=session??new ExplorationSession();
  let language=url.searchParams.get('lang')==='en'?'en':'ru',controller,readingOpen=false,disposed=false;
  let research=null,moving=!matchMedia('(prefers-reduced-motion: reduce)').matches;
  let routedView=null;
  let options={},searchPage=null,searchQuery='',searchSeek=null,searchGeneration=0,surfaceGeneration=0,renderedReading=null,renderedReadingError=null,renderedComparison=null;
  const t=key=>copy[language][key];setUiLanguage(language);
  const word=(ru,en)=>language==='ru'?ru:en;
  const viewStore=createLiveResumeStore();let resumeTimer=null,resumeWriting=Promise.resolve(),resumeEnabled=false,lastResumeKey='',unresolvedResume=null;
  root.dataset.mode='live';root.dataset.ready='false';
  root.innerHTML=`<canvas class="tree-sky" aria-hidden="true"></canvas><div class="tree-clusters"></div><div class="edge-labels"></div><div class="tree-stars"></div>
    <header class="header"><div class="brand"><span class="brand-mark">✧</span><div><b data-live-copy="brand"></b><small data-live-copy="subtitle"></small></div></div>
    <button class="search-trigger" data-live-action="search"><span>⌕</span><span data-live-copy="search"></span><kbd>/</kbd></button>
    <nav class="main-tools"><button data-live-action="conditions" data-live-copy="conditions"></button><button data-live-action="compare" data-live-copy="compare"></button></nav>
    <div class="language"><button data-live-lang="ru">RU</button><button data-live-lang="en">EN</button></div></header>
    <aside class="lens-rail"><span class="eyebrow" data-live-copy="view"></span><nav class="lens-list"></nav></aside>
    <div class="view-heading"><div class="view-kicker" data-live-copy="space"></div><h1></h1><p></p></div>
    <aside class="reading" hidden></aside>
    <aside class="materials-panel panel" hidden><div class="panel-top"><h2 data-live-copy="search"></h2><button class="icon" data-live-action="close-search">×</button></div>
      <form class="live-search-form"><input class="search" type="search" maxlength="256"><button class="primary" type="submit" data-live-copy="searchGo"></button></form>
      <p class="muted live-search-status" role="status"></p><div class="material-list"></div><button class="text-button live-more" data-live-copy="more" hidden></button></aside>
    <div class="empty-space"><h1 data-live-copy="empty"></h1><p data-live-copy="emptyText"></p><button class="primary" data-live-action="search" data-live-copy="searchGo"></button></div>
    <div class="relation-bar"><button class="primary" data-live-action="continue" data-live-copy="continue" disabled></button><button class="text-button" data-live-action="cancel" data-live-copy="cancel" hidden></button></div>
    <footer class="footer"><button class="text-button live-back" data-live-action="back" hidden></button><button class="posture-button" data-live-action="scope" data-live-copy="readonly"></button><span class="space-count"></span><div class="footer-tools">
    <button class="icon" data-live-action="overview" data-live-title="overview">⤢</button><button class="icon" data-live-action="motion" data-live-title="motion">✧</button><button class="icon" data-live-action="cinema" data-live-title="cinema">◌</button></div></footer>
    <button class="show-panels" data-live-action="cinema" data-live-copy="show" hidden></button><p class="notice" role="status" hidden></p><dialog class="dialog"></dialog>`;
  const reading=root.querySelector('.reading'),drawer=root.querySelector('.materials-panel'),dialog=root.querySelector('dialog'),notice=root.querySelector('.notice');
  let sourceCommandCleanup=null,temporalComparison=null;
  let sourceCommandBeforeUnload=false;
  const sourceCommandBeforeUnloadHandler=event=>{
    if(!sourceCommandPending())return;
    event.preventDefault();event.returnValue='';
  };
  function syncSourceCommandBeforeUnload(){
    const pending=sourceCommandPending();
    if(!pending)dialog.querySelector('[data-source-command-close-warning]')?.remove();
    if(pending&&!sourceCommandBeforeUnload){
      window.addEventListener('beforeunload',sourceCommandBeforeUnloadHandler);sourceCommandBeforeUnload=true;
    }else if(!pending&&sourceCommandBeforeUnload){
      window.removeEventListener('beforeunload',sourceCommandBeforeUnloadHandler);sourceCommandBeforeUnload=false;
    }
  }
  function disposeSourceCommandPanel(){
    const mounted=sourceCommandCleanup;sourceCommandCleanup=null;
    if(typeof mounted==='function')mounted();
    else if(mounted&&typeof mounted.dispose==='function')mounted.dispose();
    if(sourceCommandBeforeUnload){
      window.removeEventListener('beforeunload',sourceCommandBeforeUnloadHandler);sourceCommandBeforeUnload=false;
    }
  }
  const sourceCommandPending=()=>Boolean(sourceCommandCleanup?.hasUnconfirmedCommand?.());
  function sourceCommandCloseWarning(){
    const body=dialog.querySelector('.dialog-content');
    if(!body||body.querySelector('[data-source-command-close-warning]'))return;
    const warning=el('section','','source-command-close-warning');
    warning.dataset.sourceCommandCloseWarning='true';
    warning.append(el('p',t('sourceCommandUnconfirmedClose'),'body'));
    const actions=el('div','','dialog-actions');
    const copy=button(t('sourceCommandCopy'),async()=>{
      copy.disabled=true;
      try{
        const retained=sourceCommandCleanup?.retainedCommand?.();
        const write=globalThis.navigator?.clipboard?.writeText;
        if(!retained||typeof write!=='function')throw new Error(t('sourceCommandCopyUnavailable'));
        await write.call(globalThis.navigator.clipboard,JSON.stringify(retained,null,2));
        status.textContent=t('sourceCommandCopied');
        close.disabled=false;
      }catch(error){
        status.textContent=error?.message??t('sourceCommandCopyUnavailable');
        copy.disabled=false;
      }
    },'text-button');
    const close=button(t('sourceCommandClose'),()=>{
      dialog.close();
    },'text-button');
    close.disabled=true;
    const status=el('p','','muted');status.setAttribute('role','status');
    actions.append(copy,close);warning.append(actions,status);body.prepend(warning);
  }
  function closeDialog(){
    if(sourceCommandPending()){sourceCommandCloseWarning();return false;}
    dialog.close();return true;
  }
  const report=error=>{notice.textContent=error?.message??String(error);notice.hidden=false;};
  const attempt=action=>{try{return action();}catch(error){report(error);return null;}};
  const sky=skyFactory(root,{onSelect:id=>{readingOpen=true;attempt(()=>controller.selectNode(id));},
    onEdgeSelect:id=>{readingOpen=true;attempt(()=>controller.selectEdge(id));},onMove:(id,position)=>attempt(()=>controller.move(id,position))});
  function selectedLabel(state){
    if(!state.view)return t('empty');const target=state.selection;
    if(target.kind==='claim-path'){const edge=state.model.edges.find(item=>item.id===target.id);return edge?liveEdgeLabel(state.view,edge,language).text:t('selected');}
    const raw=state.view[target.kind==='node'?'nodes':'relations'].find(item=>item.id===target.id);return liveLabel(raw,language).text;
  }
  function reflect(state){
    root.querySelectorAll('[data-live-copy]').forEach(node=>node.textContent=t(node.dataset.liveCopy));
    root.querySelectorAll('[data-live-title]').forEach(node=>{node.title=t(node.dataset.liveTitle);node.setAttribute('aria-label',node.title);});
    root.querySelectorAll('[data-live-lang]').forEach(node=>node.setAttribute('aria-pressed',String(node.dataset.liveLang===language)));
    root.querySelector('.search').placeholder=t('search');root.querySelector('.search').setAttribute('aria-label',t('search'));
    root.querySelector('[data-live-action="close-search"]').setAttribute('aria-label',t('close'));
    root.querySelector('.empty-space').hidden=Boolean(state.view);
    root.querySelector('.view-heading h1').textContent=state.view?selectedLabel(state):'';
    root.querySelector('.view-heading p').textContent=state.loading?t('loading'):state.view?t('readonly'):'';
    const more=root.querySelector('[data-live-action="continue"]');more.disabled=state.loading||!state.view?.continuation?.next_cursor;
    more.hidden=state.areaKind==='lens';more.textContent=state.view&&!state.view.continuation?.next_cursor?t('complete'):t('continue');
    root.querySelector('[data-live-action="cancel"]').hidden=!state.loading;
    root.querySelector('.space-count').textContent=state.view?word(`${state.model.vertices.length} объектов · ${state.model.edges.length} связей`,`${state.model.vertices.length} objects · ${state.model.edges.length} relations`):'';
    const back=root.querySelector('.live-back');back.hidden=!state.historyDepth;back.textContent=word('← Предыдущая область','← Previous area');back.disabled=state.loading;
    const rail=root.querySelector('.lens-list');rail.replaceChildren();
    for(const mode of ['compact','grouped','raw']){const item=button(t(mode),()=>attempt(()=>controller.mode(mode)),'lens');
      item.dataset.liveProjection=mode;item.setAttribute('aria-pressed',String(state.mode===mode));rail.append(item);}
    root.dataset.visibleNodes=String(state.model?.vertices.length??0);root.dataset.visibleEdges=String(state.model?.edges.length??0);
    root.dataset.sourceRevision=state.view?.source_revision??'';root.dataset.snapshotRevision=state.view?.snapshot_revision??'';
    root.dataset.selection=state.selection?.id??'';root.dataset.loading=String(state.loading);root.dataset.history=String(state.historyDepth);
    if(state.view&&state.view!==routedView){
      const route=new URL(location.href);route.searchParams.delete('shelfRoute');
      const origin=state.areaKind==='exploration'?state.view.contexts.at(-1)?.query?.origin:null;
      if(origin){route.searchParams.set('focus',origin.id);route.searchParams.set('kind',origin.kind);}
      else{route.searchParams.delete('focus');route.searchParams.delete('kind');}
      history.replaceState(history.state,'',route);routedView=state.view;
    }
    reading.hidden=!readingOpen;root.dataset.reading=String(readingOpen);
    if(readingOpen&&(renderedReading!==state.reading||renderedReadingError!==state.readingError)){
      renderedReading=state.reading;renderedReadingError=state.readingError;renderReading(state);
    }
    if(state.error)report(state.error);
    if(dialog.open&&dialog.dataset.kind==='comparison'&&renderedComparison!==state.comparison)renderComparison(state);
    sky.refresh();
    scheduleResume();
  }
  controller=createLiveResearch({session:activeSession,sky,language,onChange:reflect});
  const corpus=mountCorpusEntry({root,client:activeSession.client,locale:()=>language,onError:report,
    onReadingChange:active=>sky.motion(active?false:moving),
    graphNavigate:async target=>{
      const state=controller.state();
      if(state.view?.[target.kind==='node'?'nodes':'relations'].some(item=>item.id===target.id)){
        readingOpen=true;controller.selectRaw(target);return;
      }
      const result=await open(target);if(!result&&controller.state().error)throw controller.state().error;
    }});
  const builder=mountLensBuilder({host:document.body,client:activeSession.client,locale:()=>language,
    getArea:()=>{const state=controller.state();return state.view?{packet:state.view,selection:state.selection}:null;},
    getCatalog:()=>controller.state().discovery?.catalog??null,
    onOpen:()=>{root.inert=true;sky.motion(false);},onClose:()=>{root.inert=false;sky.motion(moving);},onError:report,
    onApply:async({packet})=>{if(!controller.showLens(packet))throw new Error(word('В этой области пока нет предметов для отображения.','This area has no objects to show.'));readingOpen=true;await controller.read();},
    onSave:({draft})=>research.save({type:'lens',title:draft.name,target:{draft}}),
  });
  research=mountResearchShelfEntry({root,client:activeSession.client,corpus,locale:()=>language,onError:report,
    onViewChange:active=>sky.motion(active?false:moving),onLens:draft=>builder.open({draft}),onRoute:openSavedRoute,
    onMaterial:({snapshot,target,form})=>{
      const body=modal(word('Материал с вашей полки','Material from your shelf'),'shelf-material');if(!body)return;
      appendReading(body,snapshot);
      body.append(button(word('Открыть в пространстве','Open in space'),()=>open({kind:target.kind,id:target.id,content_revision:target.contentRevision})));
      const section=form?body.querySelector(`[data-form-role="${form.role}"]`):null;
      if(section){for(let ancestor=section;ancestor;ancestor=ancestor.parentElement)if(ancestor.tagName==='DETAILS')ancestor.open=true;section.scrollIntoView({block:'nearest'});}
    },
  });
  // The shared shelf also pages notebook notes; one primary personal entry is
  // enough here. The native reader retains its own contextual notes action.
  root.querySelector('[data-native-notes]')?.remove();
  const lensOpen=button(word('Собрать линзу','Build a lens'),()=>void builder.open(),'live-lens-open');root.querySelector('.main-tools').prepend(lensOpen);
  const narrowLens=button('◈',()=>void builder.open(),'live-lens-narrow');narrowLens.setAttribute('aria-label',word('Собрать линзу','Build a lens'));root.querySelector('.header').append(narrowLens);
  const transport=()=>controller.state().discovery;
  function modal(title,kind){
    if(sourceCommandPending()){sourceCommandCloseWarning();return null;}
    controller.cancelSourceRecord?.();controller.cancelSourceDossier?.();
    disposeSourceCommandPanel();temporalComparison?.destroy();temporalComparison=null;surfaceGeneration++;
    dialog.replaceChildren();dialog.dataset.kind=kind;dialog.classList.toggle('wide',['comparison','source-dossier'].includes(kind));
    const top=el('div','','dialog-top'),close=button('×',closeDialog,'icon');close.setAttribute('aria-label',t('close'));
    top.append(el('h2',title),close);const body=el('div','','dialog-content');dialog.append(top,body);if(!dialog.open)dialog.showModal();return body;
  }
  dialog.addEventListener('close',()=>{surfaceGeneration++;controller.cancelSourceDossier?.();controller.cancelSourceRecord?.();temporalComparison?.destroy();temporalComparison=null;disposeSourceCommandPanel();});
  dialog.addEventListener('cancel',event=>{if(sourceCommandPending()){event.preventDefault();sourceCommandCloseWarning();}});
  const sourceDossierRefs=snapshot=>[...new Set([snapshot?.raw?.source_dossier_ref,...(snapshot?.endpoints??[]).map(item=>item?.source_dossier_ref)])]
    .filter(isSourceDossierRef);
  function appendSourceDossier(container,dossier,snapshot,requestedRef){
    const object=dossier.object,summary=dossier.agent_summary;
    container.append(el('p',t('sourceDossierNote'),'body'),el('p',t('sourceDossierVersionNote'),'muted'));
    if(typeof object?.label==='string'&&object.label)container.append(el('h3',object.label,'minor-title'));
    container.append(detail(t('sourceIdentity'),{object_id:dossier.object_id,node_kind:object.node_kind,
      requested_source_dossier_ref:requestedRef,material_source_dossier_refs:sourceDossierRefs(snapshot),
      dossier_source_revision:null,dossier_content_revision:null,
      material_card_source_revision:snapshot.sourceRevision,material_card_content_revision:snapshot.raw.content_revision}));
    container.append(detail(t('sourceRights'),summary));
    const observed=[];
    for(const link of dossier.chain?.link??[]){
      const uri=link?.properties?.uri,status=link?.properties?.access_status;
      if(typeof uri==='string'&&/^https?:\/\//i.test(uri))observed.push({uri,
        label:typeof link.label==='string'&&link.label?link.label:uri,
        access_status:typeof status==='string'&&status?status:'unknown'});
    }
    const seen=new Set();
    const links=el('details');links.append(el('summary',t('sourceLinks')));
    for(const item of observed){if(seen.has(item.uri))continue;seen.add(item.uri);
      const row=el('p','','source-link');const anchor=el('a',item.label);anchor.href=item.uri;anchor.target='_blank';anchor.rel='noopener noreferrer';
      row.append(anchor,el('small',` · ${item.access_status}`,'muted'));links.append(row);}
    if(seen.size)container.append(links);else container.append(el('p',t('sourceNoLinks'),'muted'));
    container.append(detail(t('sourceStructure'),{chain:dossier.chain,tree_paths:dossier.tree_paths,relations:dossier.relations,
      rights:dossier.rights,truncated:dossier.truncated}));
    container.append(detail(t('sourceRefs'),dossier.source_refs));
  }
  async function openSourceDossier(requestedRef,snapshot){
    if(!isSourceDossierRef(requestedRef))return;
    const body=modal(t('sourceDossier'),'source-dossier');if(!body)return;
    const surface=surfaceGeneration;body.append(el('p',t('sourceDossierLoading'),'body'));
    try{
      const dossier=await controller.sourceDossier(requestedRef,{limit:SOURCE_DOSSIER_LIMIT});
      if(disposed||surface!==surfaceGeneration)return;
      body.replaceChildren();
      if(dossier)appendSourceDossier(body,dossier,snapshot,requestedRef);else body.append(el('p',t('sourceDossierUnavailable'),'body'));
    }catch(error){if(!disposed&&surface===surfaceGeneration){body.replaceChildren(el('p',error?.message??String(error),'body'));}}
  }
  async function openSourceRecord(snapshot,representation='record'){
    if(representation!=='record'){
      closeDialog();root.dataset.corpusReading='true';
      return corpus.native.open({selection:{kind:snapshot.kind,id:snapshot.raw.id,source_revision:snapshot.sourceRevision,
        content_revision:snapshot.raw.content_revision},representation});
    }
    const body=modal(t('sourceRecord'),'source-record');if(!body)return;
    const surface=surfaceGeneration;body.append(el('p',t('loading'),'body'));
    try{
      const result=await controller.sourceRecord(snapshot,{representation});
      if(disposed||surface!==surfaceGeneration)return;
      body.replaceChildren();
      if(!result){body.append(el('p',t('sourceRecordUnavailable'),'body'));return;}
      body.dataset.sourceReadStatus=result.status;
      if(result.status!=='available'){
        const label={missing:'sourceRecordMissing','access-restricted':'sourceRecordRestricted',corrupt:'sourceRecordCorrupt','over-budget':'sourceRecordBudget'}[result.status]??'sourceRecordUnavailable';
        body.append(el('p',t(label),'body'));return;
      }
      body.append(el('p',t('sourceRecordNote'),'body'));
      const nativeActions=result.record.native_text_binding?el('div','','source-native-actions'):null;
      if(nativeActions)body.append(nativeActions);
      if(typeof result.record.preferred_label==='string')body.append(el('h3',result.record.preferred_label,'minor-title'));
      const sourceNote=result.layer==='authored_csv_record'?result.record.note:result.record.notes;
      if(typeof sourceNote==='string'&&sourceNote.trim()){
        body.append(el('h3',t('sourceRecordWords'),'minor-title'));
        const notes=el('p',sourceNote,'body');
        if(typeof result.record.language==='string')notes.lang=result.record.language;
        body.append(notes);
      }else body.append(el('p',t('sourceRecordNoWords'),'muted'));
      body.append(detail(t('sourceIdentity'),{source_revision:result.source_revision,content_revision:result.content_revision,
        ...(result.layer==='authored_csv_record'?{target:result.handle.target}:{record_ref:result.record_ref})}),
        detail(t('sourceRights'),result.access),detail(t('technical'),result.record),detail(t('sourceRefs'),result.provenance));
      if(nativeActions){
        // The exact record is already available. Discovering optional text
        // actions must neither delay its display nor replace it on failure.
        try{
          const choices=await exactSourceRepresentations(activeSession.client,snapshot.sourceRevision);
          if(disposed||surface!==surfaceGeneration)return;
          for(const mode of choices)nativeActions.append(button(mode==='native_local_unit'
            ?word('Читать по локальным условиям','Read under local conditions')
            :word('Открыть точный текст фрагмента','Open exact fragment text'),()=>void openSourceRecord(snapshot,mode),'source-link'));
        }catch{
          if(disposed||surface!==surfaceGeneration)return;
          nativeActions.append(el('p',word('Способы чтения текста сейчас недоступны.','Text reading options are currently unavailable.'),'muted'));
        }
      }
    }catch(error){if(!disposed&&surface===surfaceGeneration)body.replaceChildren(el('p',error?.message??t('sourceRecordUnavailable'),'body'));}
  }
  function openSourceCommands(){
    try{
      const body=modal(t('sourceChanges'),'source-command');
      if(!body)return;
      sourceCommandCleanup=mountSourceCommandPanel(body,{language,onChanged:syncSourceCommandBeforeUnload});
      syncSourceCommandBeforeUnload();
    }catch(error){report(error);}
  }
  function appendReading(container,snapshot){
    const doc=readingDocument(snapshot,language);container.append(el('h1',doc.title?.text??t('noTitle')));
    if(doc.humanForms)container.append(renderHumanForms(snapshot.raw,{exactForms:snapshot.exactForms,readableContext:snapshot.readableContext}));
    else for(const block of doc.blocks){container.append(el('h3',block.title,'minor-title'));const text=el('p',block.form?.text??t('noDescription'),'body');
      if(block.form?.lang)text.lang=block.form.lang;container.append(text);}
    container.append(renderEssentialContext(doc.essentialContext,snapshot.readableContext));
    if(snapshot.claimReading)container.append(renderClaimContext(snapshot.claimReading,snapshot.readableContext));
    if(snapshot.claimContextUnavailable)container.append(el('p',language==='ru'?'Контекст утверждения не вошёл в эту карточку. Раскройте его связи.':'Claim context is not included in this card. Expand its relations.','muted'));
    const sources=el('details');sources.append(el('summary',t('sources')));
    const exactSource=button(t('sourceRecord'),()=>void openSourceRecord(snapshot),'source-link');
    exactSource.dataset.sourceRecordId=snapshot.raw.id;sources.append(exactSource);
    for(const ref of doc.sourceRefs){
      if(/^https?:\/\//i.test(ref)){const link=el('a',ref,'source-link');link.href=ref;link.target='_blank';link.rel='noopener noreferrer';sources.append(link);}
      else sources.append(el('p',ref,'source-link'));
    }
    const dossierRefs=sourceDossierRefs(snapshot);
    for(const ref of dossierRefs){
      const action=button(`${t('sourceDossier')} · ${ref}`,()=>void openSourceDossier(ref,snapshot),'source-link');
      action.dataset.sourceDossierRef=ref;sources.append(action);
    }
    if(!dossierRefs.length&&doc.sourceRefs.some(ref=>!/^https?:\/\//i.test(ref)))sources.append(el('p',t('sourceNoDossier'),'muted'));
    container.append(sources,detail(t('technical'),{source_revision:snapshot.sourceRevision,record:snapshot.raw}));
    research?.addReadingActions(container,snapshot);
  }
  function renderReading(state){
    reading.replaceChildren();const top=el('div','','reading-top');top.append(el('span',t('selected'),'eyebrow'),button('×',()=>{
      readingOpen=false;controller.closeReading();reflect(controller.state());},'icon'));top.lastChild.setAttribute('aria-label',t('close'));reading.append(top);
    if(!state.reading){
      reading.append(el('p',state.readingError?.message??t('loading'),'body'));
      if(state.readingError)reading.append(button(t('retryReading'),()=>void controller.read()));
      return;
    }
    const actions=el('div','','reading-actions');actions.append(button(t('expand'),()=>open(controller.selectedTarget())),
      button(t('newSpace'),()=>open(controller.selectedTarget(),true)),button(t('pin'),()=>void controller.pin()),
      button(t('sourceChanges'),openSourceCommands,'source-link'));reading.append(actions);
    appendReading(reading,state.reading);
    const target=state.selection,ids=target.kind==='claim-path'?[target.claimId]:[target.id],kind=target.kind==='relation'?'relations':'nodes';
    const reasons=state.areaKind==='lens'?ids.filter(id=>state.view.inclusion?.[kind]?.[id]).map(id=>({query:specForPacket(state.view),reason:state.view.inclusion[kind][id]})):
      state.view.contexts.flatMap(context=>ids.flatMap(id=>(context.inclusion[kind][id]??[]).map(reason=>({origin:context.origin,query:context.query,...reason}))));
    const explanation=el('details');explanation.append(el('summary',t('reasons')));
    explanation.append(el('p',language==='ru'?'Это объяснение выполнения запроса, а не доказательство смысловой связи или истинности.':'This explains query execution, not proof of a semantic relationship or truth.','body'));
    for(const reason of reasons){
      const paragraph=el('p','','body');
      paragraph.textContent=inclusionSummary({...reason.reason,query:reason.query},{language,
        nodeLabel:id=>{const raw=state.view.nodes.find(row=>row.id===id);return raw?liveLabel(raw,language).text:null;},
        relationLabel:id=>{const raw=state.view.relations.find(row=>row.id===id);return raw?liveLabel(raw,language).text:null;},
        predicateLabel:id=>{const predicate=state.discovery?.catalog?.predicates?.find(row=>row.predicate_id===id);return predicate?livePredicateLabel(predicate,language):id;},
      }).join(' ');
      explanation.append(paragraph);
    }
    explanation.append(detail(language==='ru'?'Точные данные исполнения':'Exact execution data',reasons));reading.append(explanation);
    const links=el('details');links.append(el('summary',t('conditions')));
    for(const edge of state.model.edges){if(![edge.fromId,edge.toId].includes(state.model.carrierToVertex.get(ids[0])))continue;
      links.append(button(liveEdgeLabel(state.view,edge,language).text,()=>controller.selectEdge(edge.id),'material-row'));}
    reading.append(links);
  }
  async function open(target,replace=false){
    if(!target)return;notice.hidden=true;
    const surface=surfaceGeneration;
    const result=await controller.open(target,{replace,options});
    if(result&&!disposed){
      // A late graph request must not dismiss a tool the reader opened while
      // waiting. Only the surface that initiated this request may be closed.
      if(surface===surfaceGeneration){drawer.hidden=true;closeDialog();}
      readingOpen=true;renderedReading=undefined;reflect(controller.state());await controller.read();
    }
    return result;
  }
  async function openSavedRoute(value){
    const target=validateRouteTarget(value);
    if(target.origin.sourceRevision!==controller.state().discovery?.catalog.source_revision)throw new RevisionError();
    options=target.options;
    const result=await open({kind:target.origin.kind,id:target.origin.id,content_revision:target.origin.contentRevision},true);
    if(!result)throw controller.state().error??new Error(word('Не удалось открыть сохранённый маршрут.','The saved route could not be opened.'));
    return result;
  }
  function scheduleResume(){
    if(!resumeEnabled||disposed)return;clearTimeout(resumeTimer);resumeTimer=setTimeout(()=>void saveResume(),500);
  }
  function saveResume(){
    clearTimeout(resumeTimer);
    try{
      const value=makeLiveResume(controller.state(),controller.capturePresentation());if(!value)return resumeWriting;
      if(unresolvedResume){
        if(unresolvedResume.sourceRevision===value.sourceRevision&&unresolvedResume.area===JSON.stringify(value.area)
          &&unresolvedResume.selection===JSON.stringify(controller.state().selection))return resumeWriting;
        unresolvedResume=null;
      }
      const key=JSON.stringify(value);if(key===lastResumeKey)return resumeWriting;
      resumeWriting=resumeWriting.catch(()=>{}).then(()=>viewStore.save(value)).then(()=>{lastResumeKey=key;}).catch(report);return resumeWriting;
    }catch(error){report(error);return Promise.resolve();}
  }
  function restoreResumeSelection(saved){
    const target=resolveLiveResumeSelection(saved.selection,controller.state().view);
    if(target){unresolvedResume=null;controller.selectRaw(target);return true;}
    unresolvedResume={sourceRevision:saved.sourceRevision,area:JSON.stringify(saved.area),selection:JSON.stringify(controller.state().selection)};
    readingOpen=false;controller.closeReading();
    report(new Error(word('Сохранённый точный выбор отсутствует в этой области или изменился. Прежняя запись сохранена; выберите доступный материал, чтобы продолжить.',
      'The saved exact selection is absent from this area or has changed. Its record is preserved; select an available material to continue.')));
    return false;
  }
  function showScope(){
    const state=controller.state(),body=modal(word('Область исследования','Research area'),'scope');if(!body)return;
    if(!state.view){body.append(el('p',t('emptyText')));return;}
    body.append(el('p',word(`Видно ${state.model.vertices.length} объектов и ${state.model.edges.length} связей.`,
      `${state.model.vertices.length} objects and ${state.model.edges.length} relations are visible.`),'body'));
    body.append(el('p',word(`Удержано исходных записей: ${state.view.nodes.length}; связей: ${state.view.relations.length}. Это ограниченная область исследования.`,
      `Retained source records: ${state.view.nodes.length}; relations: ${state.view.relations.length}. This is a bounded research area.`),'body'));
    if(state.areaKind==='lens')body.append(detail(word('Охват запроса','Query coverage'),state.view.counts));
    else body.append(el('p',state.view.continuation.next_cursor?word('Есть продолжение раскрытия.','More expansion is available.'):word('Этот запрос завершён в своих границах.','This query has finished within its scope.'),'body'));
    const types=new Map();
    for(const vertex of state.model.vertices){const raw=state.model.rawNodesById.get(vertex.representativeId),id=raw.type_id??raw.kind_id??null;
      if(types.has(id)){types.get(id).count++;continue;}
      const entry=state.discovery?.catalog.node_kinds?.find(item=>item.kind_id===raw.kind_id||item.type_id===raw.type_id);
      const label=localized(entry?.display,word('Тип не назван','Type unnamed'),language);
      types.set(id,{label,count:1,color:liveTypeColor(raw)});
    }
    body.append(el('h3',word('Типы в поле','Types in this field'),'minor-title'));
    for(const {label,count,color} of types.values()){const row=el('p',`${label} · ${count}`,'live-legend-item');row.style.setProperty('--type-color',color);body.append(row);}
    body.append(el('p',word('Цвет различает объявленные типы. Расположение — ваше представление области.','Color distinguishes declared types. Position is your view of the area.'),'muted'));
    const saved=makeLiveResume(state,controller.capturePresentation());
    if(saved)body.append(button(word('Сохранить эту область','Save this area'),async()=>{
      try{await research.save({type:saved.area.type,title:selectedLabel(state),target:saved.area.target});report(new Error(word('Область сохранена на вашей полке.','The area is saved on your shelf.')));}catch(error){report(error);}
    }));
  }
  const format=(key,values)=>String(t(key)).replace(/\{(\d+)\}/g,(_,index)=>String(values[Number(index)]??''));
  async function search(cursor=null){
    const query=root.querySelector('.search').value,token=++searchGeneration;
    root.querySelector('.live-search-status').textContent=t('loading');root.querySelector('.live-more').disabled=true;
    try{
      // Search uses the same session as the scene and the bound discovery. An
      // empty prefix is sought only within the explicit browser-side window;
      // the returned cursor remains the sole continuation authority.
      const seek=await seekSearchPage(nextCursor=>activeSession.search(query,{cursor:nextCursor}),{
        cursor,isCurrent:()=>!disposed&&token===searchGeneration&&root.querySelector('.search').value===query,window:SEARCH_SEEK_WINDOW});
      if(disposed||token!==searchGeneration||!seek.page)return;
      searchPage=seek.page;searchQuery=query;searchSeek=seek.paused?seek:null;renderSearch();
    }catch(error){if(!disposed&&token===searchGeneration){root.querySelector('.live-search-status').textContent=error.message;report(error);}}
  }
  function renderSearch(){
    const list=root.querySelector('.material-list');list.replaceChildren();const rows=searchPage?[...searchPage.nodes.map(raw=>({kind:'node',raw})),...searchPage.relations.map(raw=>({kind:'relation',raw}))]:[];
    for(const {kind,raw} of rows){const item=button('',()=>open({kind,id:raw.id,content_revision:raw.content_revision}),'material-row');
      item.dataset.resultKind=kind;item.dataset.resultId=raw.id;item.append(el('small',t(kind)),el('strong',liveLabel(raw,language).text));
      const identity=el('details');identity.append(el('summary',t('technical')),el('p',raw.id,'muted'));list.append(item,identity);}
    root.querySelector('.live-search-status').textContent=rows.length?'':searchSeek?.paused
      ?format('searchPaused',[searchSeek.requests,searchSeek.limits.maxRequests,searchSeek.bytes,searchSeek.limits.maxBytes,
        searchSeek.elapsedMs,searchSeek.limits.maxTimeMs]):searchPage?t('none'):'';
    const more=root.querySelector('.live-more');more.hidden=!searchPage?.page.has_more;more.disabled=false;
    more.textContent=searchSeek?.paused?t('continueSearch'):t('more');
  }
  function showSearch(){surfaceGeneration++;drawer.hidden=false;root.querySelector('.search').focus();sky.refresh();}
  function showConditions(){
    const discovery=transport();if(!discovery)return;const body=modal(t('conditions'),'conditions');if(!body)return;body.append(el('p',t('conditionNote'),'body'));
    const form=el('form');body.append(form);
    const field=(label,node)=>{const row=el('label',label,'field');row.append(node);form.append(row);return node;};
    const choose=(name,items,multiple=false)=>{const select=el('select');select.name=name;select.multiple=multiple;if(multiple)select.size=5;
      for(const [id,title] of items){const option=el('option',title);option.value=id;select.append(option);}return select;};
    const profile=field(t('profile'),choose('profile',discovery.catalog.capabilities.neighborhood_profiles.map(item=>[item.profile,item.definition])));
    profile.value=options.profile??'overview';
    const direction=field(t('direction'),choose('direction',['either','incoming','outgoing'].map(id=>[id,t(id)])));direction.value=options.direction??'either';
    const depth=field(t('depth'),el('input'));depth.type='number';depth.min='0';depth.max=String(Math.min(10,discovery.exploration.limits.depth));depth.value=String(options.max_depth??2);
    const sources=field(t('sourceGraphs'),choose('sources',discovery.catalog.capabilities.sources.map(id=>[id,id]),true));
    for(const option of sources.options)option.selected=(options.sources??discovery.catalog.capabilities.sources).includes(option.value);
    const predicates=field(t('predicates'),choose('predicates',discovery.catalog.predicates.map(item=>[item.predicate_id,livePredicateLabel(item,language)]),true));
    for(const option of predicates.options)option.selected=(options.predicate_ids??[]).includes(option.value);
    const submit=el('button',t('apply'),'primary');submit.type='submit';submit.disabled=!controller.state().view;form.append(submit);
    form.onsubmit=event=>{event.preventDefault();options={profile:profile.value,direction:direction.value,max_depth:Number(depth.value),
      sources:[...sources.selectedOptions].map(item=>item.value),predicate_ids:[...predicates.selectedOptions].map(item=>item.value)};void open(controller.selectedTarget());};
  }
  function renderComparison(state){
    renderedComparison=state.comparison;const body=modal(t('compare'),'comparison');if(!body)return;const grid=el('div','','comparison-grid');body.append(grid);
    if(!state.comparison.length)body.append(el('p',t('noPins'),'body'));
    for(const item of state.comparison){const card=el('section');grid.append(card);if(item.reading)appendReading(card,item.reading);else card.append(el('p',item.error?.message??t('loading'),'body'));}
    temporalComparison=mountTemporalComparison({client:activeSession.client,locale:()=>language,getReadings:()=>controller.state().comparison.map(item=>item.reading)});
    body.append(temporalComparison.element);
    body.append(button(t('clear'),()=>controller.clearComparison()));
  }
  const actions={search:showSearch,'close-search':()=>{searchGeneration++;activeSession.cancelSearch();searchSeek=null;drawer.hidden=true;sky.refresh();},
    conditions:showConditions,compare:()=>renderComparison(controller.state()),continue:()=>void controller.continue(),cancel:()=>controller.cancel(),scope:showScope,
    back:()=>{readingOpen=true;controller.back();reflect(controller.state());},
    overview:()=>sky.frame(),motion:()=>{moving=!moving;sky.motion(moving);root.querySelector('[data-live-action="motion"]').setAttribute('aria-pressed',String(moving));},
    cinema:()=>{const hidden=root.dataset.cinema!=='true';root.dataset.cinema=String(hidden);root.querySelector('.show-panels').hidden=!hidden;sky.refresh();}};
  const click=event=>{const action=event.target.closest('[data-live-action]')?.dataset.liveAction;if(action)attempt(actions[action]);
    const lang=event.target.closest('[data-live-lang]')?.dataset.liveLang;if(lang){language=lang;setUiLanguage(lang);renderedReading=undefined;controller.language(lang);renderSearch();}};
  root.addEventListener('click',click);
  root.querySelector('.live-search-form').onsubmit=event=>{event.preventDefault();void search();};
  root.querySelector('.search').oninput=()=>{searchGeneration++;activeSession.cancelSearch();searchPage=null;searchSeek=null;renderSearch();};
  root.querySelector('.live-more').onclick=()=>{if(searchPage&&searchQuery===root.querySelector('.search').value)void search(searchPage.page.next_cursor);};
  const keydown=event=>{if(root.inert||event.defaultPrevented||event.target.closest('input,textarea,select,[contenteditable]'))return;
    if(event.key==='/'){event.preventDefault();showSearch();}if(event.key==='Escape'){actions['close-search']();readingOpen=false;controller.closeReading();}
    if(event.key==='o'&&!dialog.open)sky.frame();};
  document.addEventListener('keydown',keydown);
  root.addEventListener('pointerup',scheduleResume);root.addEventListener('wheel',scheduleResume,{passive:true});
  const visibility=()=>{if(document.visibilityState==='hidden'&&resumeEnabled)void saveResume();};document.addEventListener('visibilitychange',visibility);
  const dispose=()=>{if(disposed)return;if(resumeEnabled)void saveResume().finally(()=>viewStore.close());else viewStore.close();disposed=true;clearTimeout(resumeTimer);searchGeneration++;disposeSourceCommandPanel();research.destroy();builder.destroy();corpus.destroy();controller.dispose();root.removeEventListener('click',click);root.removeEventListener('pointerup',scheduleResume);root.removeEventListener('wheel',scheduleResume);document.removeEventListener('keydown',keydown);document.removeEventListener('visibilitychange',visibility);};
  window.addEventListener('pagehide',event=>{if(!event.persisted)dispose();});
  reflect(controller.state());
  const discovery=await controller.start();if(discovery){root.dataset.ready='true';
    const id=url.searchParams.get('focus'),kind=url.searchParams.get('kind')??'node',shelfRoute=url.searchParams.get('shelfRoute');
    if(shelfRoute){try{const record=await research.shelf.store.get(shelfRoute);if(record?.type!=='route')throw new Error(word('Сохранённый маршрут не найден.','The saved route was not found.'));await openSavedRoute(record.target);}catch(error){report(error);showSearch();}}
    else if(id&&['node','relation'].includes(kind)){
      let saved=null;try{saved=await viewStore.load();}catch(error){report(error);}
      const matching=saved?.sourceRevision===discovery.catalog.source_revision&&saved.area.type==='route'
        &&saved.area.target.origin.kind===kind&&saved.area.target.origin.id===id;
      if(matching)options=saved.area.target.options;
      const opened=await open({kind,id});
      if(opened&&matching){controller.restorePresentation(saved.presentation);
        restoreResumeSelection(saved);}
    }else {
      let saved=null;try{saved=await viewStore.load();}catch(error){report(error);}
      if(saved&&saved.area.type==='route'&&saved.sourceRevision===discovery.catalog.source_revision){
        options=saved.area.target.options;
        const origin=saved.area.target.origin;
        const opened=await open({kind:origin.kind,id:origin.id,content_revision:origin.contentRevision},true);
        if(opened){controller.restorePresentation(saved.presentation);
          if(restoreResumeSelection(saved))report(new Error(word('Область восстановлена по сохранённому запросу. Продолжение можно раскрыть снова.','The area was restored from its saved query. Further pages can be expanded again.')));
        }
      }else if(saved?.area.type==='lens'&&saved.sourceRevision===discovery.catalog.source_revision){
        try{const context=await constructorCatalog(activeSession.client);const packet=await previewDraft(activeSession.client,saved.area.target.draft,context);
          if(packet.source_revision!==saved.sourceRevision)throw new RevisionError();
          if(!controller.showLens(packet))throw new Error(word('В сохранённой линзе сейчас нет предметов.','The saved lens has no objects now.'));
          controller.restorePresentation(saved.presentation);readingOpen=true;
          if(restoreResumeSelection(saved))await controller.read();
        }catch(error){report(error);showSearch();}
      }else {showSearch();if(saved)report(new Error(word('Сохранённая область относится к другому снимку. Она остаётся в хранилище.','The saved area belongs to another snapshot. It remains in storage.')));}
    }
    resumeEnabled=true;
  }else if(!disposed){
    const body=modal(t('discoveryFailed'),'connection');
    if(body){
      const problem=el('p',controller.state().error?.message??'','body');
      body.append(problem,button(t('retry'),async()=>{
        if(await controller.start()){root.dataset.ready='true';closeDialog();showSearch();}
        else problem.textContent=controller.state().error?.message??'';
      }));
    }
  }
  return {controller,corpus,research,builder,dispose};
}
