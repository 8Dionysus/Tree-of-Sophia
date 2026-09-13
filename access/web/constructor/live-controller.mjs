import './live.css';
import '../src/observatory/human-forms.css';
import {mountConstructorSky} from './sky.mjs';
import {createLiveResearch} from './live-research.mjs';
import {ExplorationSession} from '../src/observatory/exploration-session.mjs';
import {liveLabel,liveEdgeLabel} from './live-model.mjs';
import {readingDocument} from '../src/observatory/reader-model.mjs';
import {renderHumanForms,renderClaimContext,renderEssentialContext} from '../src/observatory/human-forms-view.mjs';
import {setUiLanguage} from '../src/observatory/ui-i18n.mjs';

const copy={ru:{brand:'ДРЕВО СОФИИ',subtitle:'Исследование исходного знания',search:'Найти предмет или связь',close:'Закрыть',
  view:'ПРЕДСТАВЛЕНИЕ',compact:'Смысловые связи',grouped:'Предметы и записи',raw:'Все носители',conditions:'Условия раскрытия',
  compare:'Сопоставить',continue:'Продолжить раскрытие',overview:'Вместить в поле',motion:'Движение пространства',cinema:'Скрыть интерфейс',
  show:'Показать интерфейс',empty:'С чего начнём исследование?',emptyText:'Найдите слово, мысль, человека, произведение, событие или связь.',
  loading:'Загружаю…',readonly:'Чтение · без изменения источников',searchGo:'Найти',more:'Следующая страница',
  none:'Совпадений на этой странице нет.',node:'Предмет',relation:'Связь',read:'Читать',expand:'Раскрыть окружение',
  newSpace:'Открыть в новом поле',pin:'Закрепить для сравнения',sources:'Источники',technical:'Точные сведения',
  reasons:'Почему включено',noReading:'Выберите звезду или линию для чтения.',cancel:'Отменить запрос',
  profile:'Правило близости',direction:'Направление',either:'В обе стороны',incoming:'Входящие',outgoing:'Исходящие',
  depth:'Глубина',predicates:'Отношения (пусто — все)',sourceGraphs:'Источники графа',apply:'Раскрыть выбранное',
  conditionNote:'Условия относятся к следующему раскрытию. Уже прочитанные области остаются в поле.',
  noTitle:'Название не предоставлено',noDescription:'Описание не предоставлено.',clear:'Убрать закреплённые карточки',
  noPins:'Закрепите до двух материалов из карточки выбранного предмета или связи.',
  discoveryFailed:'Этот backend не предоставил совместимый контракт исследования. Исходные данные не изменены.',
  retry:'Повторить подключение',retryReading:'Повторить чтение',selected:'Выбранный материал',complete:'Раскрытие завершено',space:'Локальное поле чтения'},
en:{brand:'TREE OF SOPHIA',subtitle:'Explore source-owned knowledge',search:'Find an object or relation',close:'Close',
  view:'PRESENTATION',compact:'Meaningful relations',grouped:'Objects and records',raw:'All carriers',conditions:'Expansion conditions',
  compare:'Compare',continue:'Continue expansion',overview:'Fit the field',motion:'Space motion',cinema:'Hide interface',
  show:'Show interface',empty:'Where shall we begin?',emptyText:'Find a word, thought, person, work, event or relation.',
  loading:'Loading…',readonly:'Reading · no source changes',searchGo:'Search',more:'Next page',none:'No matches on this page.',
  node:'Object',relation:'Relation',read:'Read',expand:'Expand neighborhood',newSpace:'Open in a new field',pin:'Pin for comparison',
  sources:'Sources',technical:'Exact details',reasons:'Why included',noReading:'Select a star or line to read.',cancel:'Cancel request',
  profile:'Proximity rule',direction:'Direction',either:'Both directions',incoming:'Incoming',outgoing:'Outgoing',depth:'Depth',
  predicates:'Relations (empty means all)',sourceGraphs:'Graph sources',apply:'Expand selection',
  conditionNote:'Conditions apply to the next expansion. Previously read areas remain in the field.',
  noTitle:'Name not supplied',noDescription:'Description not supplied.',clear:'Remove pinned cards',
  noPins:'Pin up to two materials from the selected object or relation card.',
  discoveryFailed:'This backend did not provide a compatible exploration contract. Source data is unchanged.',
  retry:'Reconnect',retryReading:'Retry reading',selected:'Selected material',complete:'Expansion complete',space:'Local reading field'}};
const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=String(text??'');node.className=className;return node;};
const button=(text,callback,className='text-button')=>{const node=el('button',text,className);node.type='button';node.onclick=callback;return node;};
const detail=(title,value)=>{const node=el('details');node.append(el('summary',title),el('pre',JSON.stringify(value,null,2),'live-json'));return node;};

// Explicit mode only. No demo fallback, localStorage migration, source writer,
// external endpoint selection, or automatic consumer activation lives here.
export async function mountLiveResearch(root,{session,skyFactory=mountConstructorSky,url=new URL(location.href)}={}){
  const activeSession=session??new ExplorationSession();
  let language=url.searchParams.get('lang')==='en'?'en':'ru',controller,readingOpen=false,disposed=false;
  let options={},searchPage=null,searchQuery='',searchGeneration=0,renderedReading=null,renderedReadingError=null,renderedComparison=null;
  const t=key=>copy[language][key];setUiLanguage(language);
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
    <footer class="footer"><span class="posture-button" data-live-copy="readonly"></span><span class="space-count"></span><div class="footer-tools">
    <button class="icon" data-live-action="overview" data-live-title="overview">⤢</button><button class="icon" data-live-action="motion" data-live-title="motion">✧</button><button class="icon" data-live-action="cinema" data-live-title="cinema">◌</button></div></footer>
    <button class="show-panels" data-live-action="cinema" data-live-copy="show" hidden></button><p class="notice" role="status" hidden></p><dialog class="dialog"></dialog>`;
  const reading=root.querySelector('.reading'),drawer=root.querySelector('.materials-panel'),dialog=root.querySelector('dialog'),notice=root.querySelector('.notice');
  const report=error=>{notice.textContent=error?.message??String(error);notice.hidden=false;};
  const attempt=action=>{try{return action();}catch(error){report(error);return null;}};
  const sky=skyFactory(root,{onSelect:id=>{readingOpen=true;attempt(()=>controller.selectNode(id));},
    onEdgeSelect:id=>{readingOpen=true;attempt(()=>controller.selectEdge(id));},onMove:(id,position)=>attempt(()=>controller.move(id,position))});
  function selectedLabel(state){
    if(!state.view)return t('empty');const target=state.view.selection;
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
    const more=root.querySelector('[data-live-action="continue"]');more.disabled=state.loading||!state.view?.continuation.next_cursor;
    more.textContent=state.view&&!state.view.continuation.next_cursor?t('complete'):t('continue');
    root.querySelector('[data-live-action="cancel"]').hidden=!state.loading;
    root.querySelector('.space-count').textContent=state.view?`${state.view.nodes.length} / 200 · ${state.view.relations.length} / 600`:'';
    const rail=root.querySelector('.lens-list');rail.replaceChildren();
    for(const mode of ['compact','grouped','raw']){const item=button(t(mode),()=>attempt(()=>controller.mode(mode)),'lens');
      item.dataset.liveProjection=mode;item.setAttribute('aria-pressed',String(state.mode===mode));rail.append(item);}
    root.dataset.visibleNodes=String(state.model?.vertices.length??0);root.dataset.visibleEdges=String(state.model?.edges.length??0);
    root.dataset.sourceRevision=state.view?.source_revision??'';root.dataset.snapshotRevision=state.view?.snapshot_revision??'';
    root.dataset.selection=state.view?.selection.id??'';root.dataset.loading=String(state.loading);
    reading.hidden=!readingOpen;root.dataset.reading=String(readingOpen);
    if(readingOpen&&(renderedReading!==state.reading||renderedReadingError!==state.readingError)){
      renderedReading=state.reading;renderedReadingError=state.readingError;renderReading(state);
    }
    if(state.error)report(state.error);
    if(dialog.open&&dialog.dataset.kind==='comparison'&&renderedComparison!==state.comparison)renderComparison(state);
    sky.refresh();
  }
  controller=createLiveResearch({session:activeSession,sky,language,onChange:reflect});
  const transport=()=>controller.state().discovery;
  function modal(title,kind){
    dialog.replaceChildren();dialog.dataset.kind=kind;dialog.classList.toggle('wide',kind==='comparison');
    const top=el('div','','dialog-top'),close=button('×',()=>dialog.close(),'icon');close.setAttribute('aria-label',t('close'));
    top.append(el('h2',title),close);const body=el('div','','dialog-content');dialog.append(top,body);if(!dialog.open)dialog.showModal();return body;
  }
  function appendReading(container,snapshot){
    const doc=readingDocument(snapshot,language);container.append(el('h1',doc.title?.text??t('noTitle')));
    if(doc.humanForms)container.append(renderHumanForms(snapshot.raw,{exactForms:snapshot.exactForms}));
    else for(const block of doc.blocks){container.append(el('h3',block.title,'minor-title'));const text=el('p',block.form?.text??t('noDescription'),'body');
      if(block.form?.lang)text.lang=block.form.lang;container.append(text);}
    container.append(renderEssentialContext(doc.essentialContext));
    if(snapshot.claimReading)container.append(renderClaimContext(snapshot.claimReading));
    if(snapshot.claimContextUnavailable)container.append(el('p',language==='ru'?'Контекст утверждения не вошёл в эту карточку. Раскройте его связи.':'Claim context is not included in this card. Expand its relations.','muted'));
    const sources=el('details');sources.append(el('summary',t('sources')));
    for(const ref of doc.sourceRefs){
      if(/^https?:\/\//i.test(ref)){const link=el('a',ref,'source-link');link.href=ref;link.target='_blank';link.rel='noopener noreferrer';sources.append(link);}
      else sources.append(el('p',ref,'source-link'));
    }
    container.append(sources,detail(t('technical'),{source_revision:snapshot.sourceRevision,record:snapshot.raw}));
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
      button(t('newSpace'),()=>open(controller.selectedTarget(),true)),button(t('pin'),()=>void controller.pin()));reading.append(actions);
    appendReading(reading,state.reading);
    const target=state.view.selection,ids=target.kind==='claim-path'?[target.claimId]:[target.id],kind=target.kind==='relation'?'relations':'nodes';
    const reasons=state.view.contexts.flatMap(context=>ids.flatMap(id=>(context.inclusion[kind][id]??[]).map(reason=>({origin:context.origin,query:context.query,...reason}))));
    reading.append(detail(t('reasons'),reasons));
    const links=el('details');links.append(el('summary',t('conditions')));
    for(const edge of state.model.edges){if(![edge.fromId,edge.toId].includes(state.model.carrierToVertex.get(ids[0])))continue;
      links.append(button(liveEdgeLabel(state.view,edge,language).text,()=>controller.selectEdge(edge.id),'material-row'));}
    reading.append(links);
  }
  async function open(target,replace=false){
    if(!target)return;notice.hidden=true;
    const result=await controller.open(target,{replace,options});
    if(result&&!disposed){drawer.hidden=true;dialog.close();readingOpen=true;renderedReading=undefined;reflect(controller.state());await controller.read();}
  }
  async function search(cursor=null){
    const query=root.querySelector('.search').value,token=++searchGeneration;
    root.querySelector('.live-search-status').textContent=t('loading');root.querySelector('.live-more').disabled=true;
    try{
      // Search uses the same session as the scene and the bound discovery.
      const result=await activeSession.search(query,{cursor});if(disposed||token!==searchGeneration||!result)return;
      searchPage=result;searchQuery=query;renderSearch();
    }catch(error){if(!disposed&&token===searchGeneration){root.querySelector('.live-search-status').textContent=error.message;report(error);}}
  }
  function renderSearch(){
    const list=root.querySelector('.material-list');list.replaceChildren();const rows=searchPage?[...searchPage.nodes.map(raw=>({kind:'node',raw})),...searchPage.relations.map(raw=>({kind:'relation',raw}))]:[];
    for(const {kind,raw} of rows){const item=button('',()=>open({kind,id:raw.id,content_revision:raw.content_revision}),'material-row');
      item.dataset.resultKind=kind;item.dataset.resultId=raw.id;item.append(el('small',t(kind)),el('strong',liveLabel(raw,language).text));
      const identity=el('details');identity.append(el('summary',t('technical')),el('p',raw.id,'muted'));list.append(item,identity);}
    root.querySelector('.live-search-status').textContent=rows.length?'':searchPage?t('none'):'';
    const more=root.querySelector('.live-more');more.hidden=!searchPage?.page.has_more;more.disabled=false;
  }
  function showSearch(){drawer.hidden=false;root.querySelector('.search').focus();sky.refresh();}
  function showConditions(){
    const discovery=transport();if(!discovery)return;const body=modal(t('conditions'),'conditions');body.append(el('p',t('conditionNote'),'body'));
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
    const predicates=field(t('predicates'),choose('predicates',discovery.catalog.predicates.map(item=>[item.predicate_id,liveLabel(item,language).role==='missing'?item.predicate_id:liveLabel(item,language).text]),true));
    for(const option of predicates.options)option.selected=(options.predicate_ids??[]).includes(option.value);
    const submit=el('button',t('apply'),'primary');submit.type='submit';submit.disabled=!controller.state().view;form.append(submit);
    form.onsubmit=event=>{event.preventDefault();options={profile:profile.value,direction:direction.value,max_depth:Number(depth.value),
      sources:[...sources.selectedOptions].map(item=>item.value),predicate_ids:[...predicates.selectedOptions].map(item=>item.value)};void open(controller.selectedTarget());};
  }
  function renderComparison(state){
    renderedComparison=state.comparison;const body=modal(t('compare'),'comparison'),grid=el('div','','comparison-grid');body.append(grid);
    if(!state.comparison.length)body.append(el('p',t('noPins'),'body'));
    for(const item of state.comparison){const card=el('section');grid.append(card);if(item.reading)appendReading(card,item.reading);else card.append(el('p',item.error?.message??t('loading'),'body'));}
    body.append(button(t('clear'),()=>controller.clearComparison()));
  }
  let moving=!matchMedia('(prefers-reduced-motion: reduce)').matches;
  const actions={search:showSearch,'close-search':()=>{searchGeneration++;activeSession.cancelSearch();drawer.hidden=true;sky.refresh();},
    conditions:showConditions,compare:()=>renderComparison(controller.state()),continue:()=>void controller.continue(),cancel:()=>controller.cancel(),
    overview:()=>sky.frame(),motion:()=>{moving=!moving;sky.motion(moving);root.querySelector('[data-live-action="motion"]').setAttribute('aria-pressed',String(moving));},
    cinema:()=>{const hidden=root.dataset.cinema!=='true';root.dataset.cinema=String(hidden);root.querySelector('.show-panels').hidden=!hidden;sky.refresh();}};
  const click=event=>{const action=event.target.closest('[data-live-action]')?.dataset.liveAction;if(action)attempt(actions[action]);
    const lang=event.target.closest('[data-live-lang]')?.dataset.liveLang;if(lang){language=lang;setUiLanguage(lang);renderedReading=undefined;controller.language(lang);renderSearch();}};
  root.addEventListener('click',click);
  root.querySelector('.live-search-form').onsubmit=event=>{event.preventDefault();void search();};
  root.querySelector('.search').oninput=()=>{searchGeneration++;activeSession.cancelSearch();searchPage=null;renderSearch();};
  root.querySelector('.live-more').onclick=()=>{if(searchQuery===root.querySelector('.search').value)void search(searchPage.page.next_cursor);};
  const keydown=event=>{if(event.target.closest('input,textarea,select,[contenteditable]'))return;
    if(event.key==='/'){event.preventDefault();showSearch();}if(event.key==='Escape'){actions['close-search']();readingOpen=false;controller.closeReading();}
    if(event.key==='o'&&!dialog.open)sky.frame();};
  document.addEventListener('keydown',keydown);
  const dispose=()=>{disposed=true;searchGeneration++;controller.dispose();root.removeEventListener('click',click);document.removeEventListener('keydown',keydown);};
  window.addEventListener('pagehide',dispose,{once:true});
  reflect(controller.state());
  const discovery=await controller.start();if(discovery){root.dataset.ready='true';
    const id=url.searchParams.get('focus'),kind=url.searchParams.get('kind')??'node';
    if(id&&['node','relation'].includes(kind))await open({kind,id});else showSearch();
  }else if(!disposed){const body=modal(t('discoveryFailed'),'connection');body.append(button(t('retry'),async()=>{
    if(await controller.start()){root.dataset.ready='true';dialog.close();showSearch();}}));}
  return {controller,dispose};
}
