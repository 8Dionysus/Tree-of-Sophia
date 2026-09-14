import './style.css';
import {mountConstructorSky} from './sky.mjs';
import {createConstructorModel} from './model.mjs';
import {createAtlasLibrary} from './atlas-data.mjs';
import {LENSES,RELATIONS,makeLensView,shortestPath} from './atlas-view.mjs';
import {bindResearchRoutes,routeGraphInput,routePath,createJourneyNavigator} from './journey-state.mjs';
import {bindFragmentCatalog,textDigest} from './fragment-catalog.mjs';
import {createResearchReader} from '../src/reader/reader.mjs';
import {createReaderNotebook} from '../src/reader/notebook.mjs';
import {readerVersionCodes} from '../src/reader/version-options.mjs';
import {INQUIRY_FOUNDATION,LENS_INQUIRY} from './inquiry-foundation.mjs';
import {bindCloseReadingGuides} from './close-reading-guides.mjs';
import {checkInquiryTextReferences,inquiryReadingContexts,routeGroundContext} from './inquiry-layer.mjs';
import {sourceReference} from './source-references.mjs';
import {carrySemanticWorkspace} from './semantic-workspace.mjs';

const root=document.querySelector('#tree');
let lang=new URL(location.href).searchParams.get('lang')==='en'?'en':'ru';
const words={ru:{brand:'ДРЕВО СОФИИ',subtitle:'Живая архитектура мысли',search:'Найти мысль, образ, источник',materials:'Материалы',collections:'Сборки',grow:'Прорастить ветвь',new:'Новая мысль',lens:'ЛИНЗЫ',relations:'ЯЗЫК СВЯЗЕЙ',all:'Всё Древо',meaning:'Смысл',connections:'Связи',source:'Истоки',close:'Закрыть',add:'Добавить в Древо',connect:'Провести связь',path:'Найти путь',compare:'Сопоставить',near:'Окружение',develop:'Развить мысль',back:'Вернуться к Древу',stars:'узла',links:'связей',visible:'в поле зрения',prepared:'СОБРАННЫЕ ИССЛЕДОВАНИЯ',open:'Исследовать',create:'Создать',save:'Сохранить',cancel:'Отмена',title:'Название',body:'Мысль',kind:'Тип',relationKind:'Характер связи',reason:'Почему они связаны?',pick:'Выберите вторую звезду',noPath:'В выбранных типах связей путь пока не найден.',local:'Исследовательский макет',demo:'Интерпретация для макета',witness:'Материал источника',own:'Ваша мысль',empty:'Раскройте готовое Древо',emptyText:'Здесь уже подготовлены источники, идеи и несколько способов исследовать их связи.',restore:'Раскрыть Древо',saved:'Изменения сохранены',undo:'Отменить',redo:'Повторить',overview:'Обзор',fullscreen:'Полный экран',cinema:'Скрыть интерфейс',show:'Показать интерфейс',motion:'Движение пространства',menu:'Пространство',export:'Скачать Древо',import:'Открыть Древо из файла',clear:'Очистить пространство',remove:'Убрать',edit:'Изменить',sourceText:'Текст и происхождение',exact:'Точный текст',generated:'Этот узел написан для исследовательского макета. Это предлагаемая мысль, а не цитата или принятое положение Древа.',relationDemo:'Связь предложена в макете. Её смысл можно исследовать, оспорить или изменить в своей сборке.',created:'Ветвь добавлена. Её можно исследовать и изменять.',pathLabel:'Путь между мыслями',comparison:'Две мысли рядом',hint:'Звезда → мысль · линия → отношение · линза → другой взгляд',archive:'Подготовленные материалы',hypothesis:'Моя версия',book:'Книга',work:'Книга',part:'Часть',chapter:'Глава',fragment:'Фрагмент',dossier:'Досье',concept:'Понятие',interpretation:'Толкование',question:'Вопрос',symbol:'Образ',figure:'Мыслитель',character:'Персонаж',tradition:'Традиция',note:'Заметка',excerpt:'Фрагмент',shared:'Общие соседи',extend:'Продолжить исследование',filter:'Показать связи',about:'Что здесь устроено',error:'Не удалось выполнить действие',on:'Показаны',off:'Скрыты'},en:{brand:'TREE OF SOPHIA',subtitle:'A living architecture of thought',search:'Find a thought, image or source',materials:'Materials',collections:'Collections',grow:'Grow a branch',new:'New thought',lens:'LENSES',relations:'LANGUAGE OF RELATIONS',all:'The whole tree',meaning:'Meaning',connections:'Relations',source:'Origins',close:'Close',add:'Add to the tree',connect:'Draw a relation',path:'Find a path',compare:'Compare',near:'Neighborhood',develop:'Develop this thought',back:'Return to the tree',stars:'nodes',links:'relations',visible:'in view',prepared:'PREPARED INVESTIGATIONS',open:'Explore',create:'Create',save:'Save',cancel:'Cancel',title:'Title',body:'Thought',kind:'Type',relationKind:'Kind of relation',reason:'Why are they connected?',pick:'Choose a second star',noPath:'No path was found among the selected relation types.',local:'Research mockup',demo:'Interpretation for the mockup',witness:'Source material',own:'Your thought',empty:'Unfold the prepared tree',emptyText:'Sources, ideas and several ways to investigate their connections are already here.',restore:'Unfold the tree',saved:'Changes saved',undo:'Undo',redo:'Redo',overview:'Overview',fullscreen:'Full screen',cinema:'Hide interface',show:'Show interface',motion:'Space motion',menu:'Workspace',export:'Download tree',import:'Open tree from file',clear:'Clear space',remove:'Remove',edit:'Edit',sourceText:'Text and provenance',exact:'Exact text',generated:'This node was written for the research mockup. It is a proposed thought, not a quotation or an accepted position of the Tree.',relationDemo:'This relation is proposed in the mockup. Explore it, challenge it or develop it in your own collection.',created:'The branch has grown. You can explore and develop it.',pathLabel:'A path between thoughts',comparison:'Two thoughts side by side',hint:'Star → thought · line → relation · lens → another view',archive:'Prepared materials',hypothesis:'My reading',book:'Book',work:'Book',part:'Part',chapter:'Chapter',fragment:'Passage',dossier:'Dossier',concept:'Concept',interpretation:'Interpretation',question:'Question',symbol:'Image',figure:'Thinker',character:'Character',tradition:'Tradition',note:'Note',excerpt:'Passage',shared:'Shared neighbors',extend:'Continue investigating',filter:'Show relations',about:'How this tree works',error:'Could not complete the action',on:'Shown',off:'Hidden'}};
Object.assign(words.ru,{routes:'Маршруты',allRoutes:'Все маршруты',routeStart:'Начать путь',resume:'Продолжить маршрут',step:'Шаг',of:'из',previous:'Назад',next:'Дальше',gather:'Собрать мысль',routeEnd:'К чему пришли',routeExit:'Вернуться к Древу',openStep:'Вернуться к шагу',perspectives:'Детали чтения',example:'Проверить на примере',consider:'Вопрос с собой',walks:'ВЫБЕРИТЕ ВОПРОС',freeCollections:'Свободные сборки',routeSaved:'Место в маршруте сохранено',routeLost:'Маршрут прерван: один из его узлов или переходов убран.',nextRelation:'СЛЕДУЮЩИЙ ПЕРЕХОД',lastStep:'Последний шаг',routeOverview:'Весь маршрут'});
Object.assign(words.en,{routes:'Routes',allRoutes:'All routes',routeStart:'Begin the walk',resume:'Continue the walk',step:'Step',of:'of',previous:'Back',next:'Next',gather:'Gather the thought',routeEnd:'What we have reached',routeExit:'Return to the tree',openStep:'Return to this step',perspectives:'Details to follow',example:'Try an example',consider:'A question to carry',walks:'CHOOSE A QUESTION',freeCollections:'Open collections',routeSaved:'Your place is saved',routeLost:'The route was interrupted: one of its nodes or transitions was removed.',nextRelation:'THE NEXT TRANSITION',lastStep:'Last step',routeOverview:'The whole route'});
Object.assign(words.ru,{reader:'Чтение'});Object.assign(words.en,{reader:'Reading'});
Object.assign(words.ru,{inquiry:'Предлагаемое прочтение',argument:'Ход мысли',counterReading:'Другое прочтение',grounds:'Текстовые основания',readingQuestion:'Вопрос к этому чтению',experiment:'Попробовать при чтении',readGround:'Открыть этот текст',externalGround:'Текст в издании',editionGround:'Найти издание',relationLimit:'Предел этого сопоставления',returnToText:'Вернуться к тексту',lensInquiry:'Как исследовать этот вопрос',lensMethod:'Способ чтения',lensLimit:'Что остаётся вне линзы',routeStarting:'С чем начинаем',routeStakes:'Почему это меняет ответ',routeCarry:'Что проверить дальше',relationWarrant:'На чём держится переход'});
Object.assign(words.en,{inquiry:'A proposed reading',argument:'The argument',counterReading:'Another reading',grounds:'Textual grounds',readingQuestion:'A question for this reading',experiment:'Try while reading',readGround:'Open this text',externalGround:'Text in the edition',editionGround:'Find the edition',relationLimit:'Limits of this comparison',returnToText:'Return to the text',lensInquiry:'How to investigate this question',lensMethod:'A way of reading',lensLimit:'What the lens leaves outside',routeStarting:'Where we begin',routeStakes:'Why this changes the answer',routeCarry:'What to examine next',relationWarrant:'What grounds this transition'});
const t=k=>words[lang][k]??RELATIONS[k]?.[lang]??k;
const count=(n,kind)=>{if(lang==='en')return `${n} ${kind==='stars'?(n===1?'node':'nodes'):(n===1?'relation':'relations')}`;const forms=kind==='stars'?['узел','узла','узлов']:['связь','связи','связей'],d=n%100;return `${n} ${forms[d>10&&d<20?2:n%10===1?0:n%10>=2&&n%10<=4?1:2]}`;};
Object.assign(words.ru,{readFragment:'Открыть фрагмент',sourceAccess:'Источник и доступ',sourceLink:'Ссылка',relatedText:'Связанный текст',fullFragment:'Полный фрагмент',parallelText:'Рядом',fragmentEnd:'Конец фрагмента',fragmentBoundary:'Границы фрагмента',fragmentRights:'Источник, перевод и права',fragmentUnavailable:'Полный текст пока недоступен',fragmentLoadError:'Не удалось загрузить полные фрагменты',originalSource:'Открыть источник',videoCredit:'Атрибуция для видео',creditCopied:'Атрибуция скопирована',paragraphs:'абзацев',translation:'Перевод',parallelNote:'Две версии одной единицы произведения. Разбиение на абзацы у переводчиков может различаться.'});
Object.assign(words.en,{readFragment:'Read the passage',sourceAccess:'Source and access',sourceLink:'Link',relatedText:'Related text',fullFragment:'Complete passage',parallelText:'Side by side',fragmentEnd:'End of passage',fragmentBoundary:'Passage boundaries',fragmentRights:'Source, translation and rights',fragmentUnavailable:'Full text is not yet available',fragmentLoadError:'Could not load the complete passages',originalSource:'Open the source',videoCredit:'Attribution for a video',creditCopied:'Attribution copied',paragraphs:'paragraphs',translation:'Translation',parallelNote:'Two versions of the same source unit. Translators may divide their paragraphs differently.'});
const tr=v=>typeof v==='string'?v:v?.[lang]??v?.ru??v?.en??'';
const el=(tag,cls,text)=>{const n=document.createElement(tag);if(cls)n.className=cls;if(text!==undefined)n.textContent=text;return n;};
const btn=(text,fn,cls='text-button')=>{const b=el('button',cls,text);b.type='button';b.onclick=fn;return b;};
const p=(text,cls='body')=>el('p',cls,text);
let noticeTimer;
function notice(text){const n=root.querySelector('.notice');n.textContent=text;n.hidden=false;clearTimeout(noticeTimer);noticeTimer=setTimeout(()=>n.hidden=true,4200);}
function attempt(fn){try{return fn()}catch(error){notice(`${t('error')}: ${error.message}`);return null;}}
try{
 if(new URL(location.href).searchParams.get('live')==='1'){
   await (await import('./live-controller.mjs')).mountLiveResearch(root);
 }else{
 const response=await fetch('./library.json');if(!response.ok)throw Error(`HTTP ${response.status}`);
 const sourceLibrary=await response.json();
 const library=await createAtlasLibrary(sourceLibrary),atlas=library.atlas,materials=new Map(library.nodes.map(m=>[m.id,m])),atlasEdges=new Map(atlas.edges.map(e=>['atlas:'+e.id,e]));
 let fragments=null,fragmentError=null;
 if(sourceLibrary.fragmentCatalog){
  try{
   const reference=sourceLibrary.fragmentCatalog;
   if(!/^assets\/[a-z0-9.-]+\.json$/.test(reference.path))throw Error('Invalid fragment asset path');
   const reply=await fetch('./'+reference.path);if(!reply.ok)throw Error(`HTTP ${reply.status}`);
   const raw=await reply.text();if(raw.length>5_000_000||await textDigest(raw)!==reference.sha256)throw Error('Fragment catalog digest mismatch');
   const verifiedFragments=await bindFragmentCatalog(JSON.parse(raw),library.nodes.map(n=>n.id));
   checkInquiryTextReferences(library.nodes,verifiedFragments.data);fragments=verifiedFragments;
  }catch(error){fragmentError=error.message;}
 }
 const readingGuides=fragments?bindCloseReadingGuides(fragments.data,sourceLibrary.fragmentCatalog.sha256):new Map();
 let storage;try{storage=localStorage;}catch{}
 const routes=bindResearchRoutes(atlas.routes,library),readingContexts=inquiryReadingContexts(library.nodes,routes),carried=await carrySemanticWorkspace(storage,library,routes);
 const key='tos-living-tree-v1:'+library.fingerprint,model=createConstructorModel(library,{storage,key});
 const journey=createJourneyNavigator(routes,{storage,key:'tos-reading-route-v1:'+library.fingerprint});
 let first=!model.getState().nodes.length;try{first=!storage?.getItem(key);}catch{}
 if(first&&!model.persistenceError()&&!carried.errors.some(item=>item.kind==='tree'))model.seedAtlas();
 let lens='tree',collection=null,selected=null,selectedEdge=null,readingTab='meaning',pair=null,neighborhood=null,path=null,cinema=false,moving=!matchMedia('(prefers-reduced-motion: reduce)').matches,deck=true;
 let enabled=new Set(Object.keys(RELATIONS)),view;
 root.innerHTML=`<canvas class="tree-sky" aria-hidden="true"></canvas><div class="tree-clusters"></div><div class="edge-labels"></div><div class="tree-stars"></div>
 <header class="header"><div class="brand"><span class="brand-mark">✧</span><div><b data-copy="brand"></b><small data-copy="subtitle"></small></div></div><button class="search-trigger" data-action="materials"><span>⌕</span><span data-copy="search"></span><kbd>/</kbd></button><nav class="main-tools"><button data-action="reader" data-copy="reader"></button><button data-action="routes" data-copy="routes" class="routes-trigger"></button><button data-action="grow" class="grow-button"><span>✣</span> <span data-copy="grow"></span></button></nav><div class="language"><button data-lang="ru">RU</button><button data-lang="en">EN</button></div><button class="icon" data-action="menu" data-title="menu">≡</button></header>
 <aside class="lens-rail"><span class="eyebrow" data-copy="lens"></span><nav class="lens-list"></nav><button class="new-thought" data-action="create"><span>＋</span><span data-copy="new"></span></button></aside>
 <div class="view-heading"><div class="view-kicker"></div><h1></h1><p></p><button class="view-guide text-button" data-action="inquiry"><span data-copy="lensInquiry"></span> ↗</button><button class="return-tree" data-action="return" hidden>← <span data-copy="back"></span></button></div>
 <aside class="reading" hidden></aside><aside class="materials-panel panel" hidden><div class="panel-top"><h2 data-copy="archive"></h2><button class="icon" data-action="close-materials" data-title="close">×</button></div><input class="search" type="search"><div class="material-filters"></div><div class="material-list"></div></aside>
 <div class="empty-space" hidden><h1 data-copy="empty"></h1><p data-copy="emptyText"></p><button class="primary" data-action="restore" data-copy="restore"></button></div>
 <section class="collection-deck"><div class="deck-caption"><span class="eyebrow" data-copy="walks"></span><button class="text-button" data-action="routes">↗ <span data-copy="allRoutes"></span></button><button class="text-button resume-walk" data-action="resume" hidden></button></div><div class="collection-cards"></div></section>
 <aside class="journey-dock" hidden aria-label="Маршрут"></aside><div class="relation-bar"><button data-action="filters" class="relation-toggle"><span>⎯</span><span data-copy="relations"></span></button><div class="relation-chips"></div></div>
 <footer class="footer"><button class="posture-button" data-action="about"><i></i><span data-copy="local"></span></button><span class="space-count"></span><div class="footer-tools"><button class="icon" data-action="undo" data-title="undo">↶</button><button class="icon" data-action="redo" data-title="redo">↷</button><i></i><button class="icon" data-action="overview" data-title="overview">⤢</button><button class="icon" data-action="motion" data-title="motion">✧</button><button class="icon" data-action="fullscreen" data-title="fullscreen">⛶</button><button class="icon" data-action="cinema" data-title="cinema">◌</button></div><span class="save-state"></span></footer>
 <div class="pair-prompt" hidden><span></span><button data-action="cancel-pair" data-copy="cancel"></button></div><button class="show-panels" data-action="cinema" data-copy="show" hidden></button><p class="notice" role="status" hidden></p><dialog class="dialog"></dialog><input class="import-file" type="file" accept="application/json,.json" hidden>`;
 let researchReader=null;
 const reading=root.querySelector('.reading'),dialog=root.querySelector('dialog'),drawer=root.querySelector('.materials-panel');
 const state=()=>model.getState(),node=id=>state().nodes.find(n=>n.id===id),material=n=>materials.get(n?.materialId),label=n=>n?(n.materialId?tr(material(n)?.title):n.title):'',body=n=>n?(n.materialId?tr(material(n)?.atlasBody??material(n)?.body):n.body):'';
 const edgeLabel=e=>tr(atlasEdges.get(e.id)?.label)||e.label||t(e.kind),edgeBody=e=>tr(atlasEdges.get(e.id)?.body)||e.label||t('relationDemo');
 const sky=mountConstructorSky(root,{onSelect:choose,onEdgeSelect:chooseEdge,onMove:(id,pos)=>attempt(()=>{const original=node(id),shown=view.nodes.find(n=>n.id===id);if(original&&shown)model.moveNode(id,original.position.map((v,i)=>v+pos[i]-shown.position[i]));})});
 function reflect(){
  root.querySelectorAll('[data-copy]').forEach(n=>n.textContent=t(n.dataset.copy));
  root.querySelectorAll('[data-title]').forEach(n=>{n.title=t(n.dataset.title);n.setAttribute('aria-label',t(n.dataset.title));});
  root.querySelectorAll('[data-lang]').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.lang===lang)));
  document.documentElement.lang=lang;document.title=`${t('brand')} · ${t('subtitle')}`;
  root.querySelector('.search').placeholder=t('search');root.querySelector('.search').setAttribute('aria-label',t('search'));
  const s=state();let currentRoute=journey.currentRoute();
  if(currentRoute&&(currentRoute.steps.some(step=>!s.nodes.some(n=>n.id===step.graphNodeId))||currentRoute.transitions.some(step=>!s.edges.some(e=>e.id===step.graphEdgeId)))){journey.leave();currentRoute=null;path=null;notice(t('routeLost'));}
  if(currentRoute)path=routePath(currentRoute);
  if(selected&&!node(selected))selected=null;if(selectedEdge&&!s.edges.some(e=>e.id===selectedEdge))selectedEdge=null;if(pair&&!node(pair.from))pair=null;
  if(path&&(path.nodeIds.some(id=>!node(id))||path.edgeIds.some(id=>!s.edges.some(e=>e.id===id))))path=null;
  root.dataset.journey=String(!!currentRoute);root.dataset.routeId=currentRoute?.id??'';root.dataset.routeStep=journey.getState()?.index??'';
  root.dataset.reading=String(!!(selected||selectedEdge));reading.hidden=!(selected||selectedEdge);
  root.dataset.deck=String(deck&&!selected&&!selectedEdge&&!cinema);root.querySelector('.collection-deck').hidden=!deck||!!selected||!!selectedEdge;
  view=makeLensView(s,library,{lens,assembly:collection,enabled:currentRoute?null:enabled,neighborhood,path});
  sky.update({...view,selectedEdgeId:selectedEdge,nodes:view.nodes.map(n=>({...n,accessibilityLabel:`${label(n)} · ${t(n.kind)}`})),edges:view.edges.map(e=>({...e,label:edgeLabel(e)})),clusters:view.clusters.map(c=>({...c,title:tr(c.title),subtitle:''}))},Object.fromEntries(s.nodes.map(n=>{const i=currentRoute?.steps.findIndex(step=>step.graphNodeId===n.id)??-1;return [n.id,(i>=0?`${i+1} · `:'')+label(n)];})));
  sky.select(selected);sky.link(pair?.from??null);
  const current=LENSES.find(l=>l.id===lens);root.querySelector('.view-kicker').textContent=currentRoute?`${t('routes')} · ${journey.getState().index+1} ${t('of')} ${currentRoute.steps.length}`:path?t('pathLabel'):collection?tr(collection.subtitle):lang==='ru'?'ФИЛОСОФИЯ · НИЦШЕ И СОЗВЕЗДИЕ МЫСЛИ':'PHILOSOPHY · NIETZSCHE AND A CONSTELLATION OF THOUGHT';
  root.querySelector('.view-heading h1').textContent=currentRoute?tr(currentRoute.title):path?`${label(node(path.nodeIds[0]))} → ${label(node(path.nodeIds.at(-1)))}`:collection?tr(collection.title):lens==='tree'?(lang==='ru'?'Вечное возвращение':'Eternal recurrence'):tr(current.title);
  root.querySelector('.view-heading p').textContent=currentRoute?tr(currentRoute.question):collection?tr(collection.question):tr(LENS_INQUIRY[lens]?.question??current.description);
  root.querySelector('.return-tree').hidden=lens==='tree'&&!collection&&!neighborhood&&!path;
  const rail=root.querySelector('.lens-list');rail.replaceChildren();for(const l of LENSES){const b=btn('',()=>setLens(l.id),'lens');b.dataset.lens=l.id;b.setAttribute('aria-pressed',String(lens===l.id));b.append(el('span','lens-icon',l.icon),el('span','',tr(l.title)));b.title=tr(l.description);rail.append(b);}
  const cards=root.querySelector('.collection-cards');cards.replaceChildren();routes.slice(0,4).forEach((route,i)=>{const b=btn('',()=>startRoute(route.id),'collection-card route-card');b.append(el('span','collection-number',`${String(i+1).padStart(2,'0')} · ${route.steps.length} ${lang==='ru'?'шагов':'steps'}`),el('strong','',tr(route.title)),el('small','',tr(route.question)),el('span','collection-arrow','↗'));cards.append(b);});
  const resume=root.querySelector('.resume-walk'),saved=journey.savedPoint();resume.hidden=!saved;resume.textContent=saved?`${t('resume')} · ${saved.index+1}/${routes.find(r=>r.id===saved.routeId).steps.length} →`:'';
  const chips=root.querySelector('.relation-chips');chips.replaceChildren();for(const k of ['supports','interprets','contrasts','compares','develops']){const b=btn(t(k),()=>toggleRelation(k),'relation-chip');b.style.setProperty('--relation-color',RELATIONS[k].color);b.setAttribute('aria-pressed',String(enabled.has(k)));b.title=`${t(enabled.has(k)?'on':'off')}: ${t(k)}`;chips.append(b);}
  root.querySelector('.space-count').textContent=`${count(view.nodes.length,'stars')} · ${count(view.edges.length,'links')} ${t('visible')}`;
  root.querySelector('.save-state').textContent=model.persistenceError()||(currentRoute&&journey.error())||(carried.errors.length?(lang==='ru'?'Предыдущее поле сохранено; перенос не удался':'The previous workspace is preserved; transfer failed'):t('saved'));
  root.querySelector('[data-action="undo"]').disabled=!model.canUndo();root.querySelector('[data-action="redo"]').disabled=!model.canRedo();
  root.querySelector('.empty-space').hidden=s.nodes.length>0;root.querySelector('.pair-prompt').hidden=!pair;root.querySelector('.pair-prompt span').textContent=pair?`${t(pair.mode)}: ${label(node(pair.from))} → ${t('pick')}`:'';
  root.dataset.nodes=s.nodes.length;root.dataset.edges=s.edges.length;root.dataset.visibleNodes=view.nodes.length;root.dataset.visibleEdges=view.edges.length;root.dataset.lens=lens;
  renderReading();renderJourneyDock();if(!drawer.hidden)renderMaterials();sky.refresh();
 }

 function startRoute(id,index=0){return attempt(()=>{
  const route=routes.find(item=>item.id===id);if(!route)throw Error('Unknown route');
  model.growAtlas(routeGraphInput(route));journey.start(id,index);collection=neighborhood=null;lens='tree';path=routePath(route);selected=route.steps[index].graphNodeId;selectedEdge=null;readingTab='meaning';pair=null;deck=false;drawer.hidden=true;dialog.close();reflect();reading.scrollTop=0;sky.frame();
  return true;
 });}
 function resumeRoute(){const saved=journey.savedPoint();if(saved&&startRoute(saved.routeId,saved.index)&&saved.finished)finishRoute();}
 function moveRoute(index){const route=journey.currentRoute(),focus=document.activeElement?.dataset.routeControl;if(!route||!journey.go(index))return;selected=route.steps[index].graphNodeId;selectedEdge=null;readingTab='meaning';reflect();reading.scrollTop=0;restoreRouteFocus(focus);}
 function finishRoute(){const focus=document.activeElement?.dataset.routeControl;if(!journey.finish())return;selected=journey.currentRoute().steps.at(-1).graphNodeId;selectedEdge=null;readingTab='meaning';reflect();reading.scrollTop=0;restoreRouteFocus(focus);}
 function restoreRouteFocus(control){if(!control)return;const target=[...root.querySelectorAll('[data-route-control]')].find(button=>button.dataset.routeControl===control);(target&&!target.disabled?target:root.querySelector('.journey-stop[aria-current=step]'))?.focus({preventScroll:true});}
 function showRoutes(){
  const content=modal(t('routes'),true),saved=journey.savedPoint();
  content.append(p(lang==='ru'?'Каждый путь начинается с вопроса. Мысль меняется от шага к шагу; переходы показывают, почему стоит двигаться именно к соседнему узлу.':'Each walk begins with a question. The thought changes from step to step; transitions explain why the next node matters.'));
  if(saved){const route=routes.find(r=>r.id===saved.routeId);content.append(btn(`${t('resume')}: ${tr(route.title)} · ${saved.index+1}/${route.steps.length} →`,resumeRoute,'primary route-resume'));}
  const grid=el('div','collection-grid route-options');
  routes.forEach(route=>{const b=btn('',()=>startRoute(route.id),'collection-option route-option');b.append(el('small','',`${route.steps.length} ${lang==='ru'?'шагов':'steps'}`),el('h3','',tr(route.title)),p(tr(route.description)),el('span','',t('routeStart')+' →'));grid.append(b);});content.append(grid,btn(t('freeCollections')+' ↗',collections,'text-button free-collections-link'));
 }
 function renderJourneyDock(){
  const dock=root.querySelector('.journey-dock'),route=journey.currentRoute(),point=journey.getState();dock.replaceChildren();dock.hidden=!route||cinema;dock.setAttribute('aria-label',t('routes'));if(!route)return;
  const progress=el('nav','journey-progress');progress.setAttribute('aria-label',t('routeOverview'));
  route.steps.forEach((step,index)=>{const b=btn(String(index+1),()=>moveRoute(index),'journey-stop');b.setAttribute('aria-label',`${t('step')} ${index+1}: ${label(node(step.graphNodeId))}`);b.setAttribute('aria-current',index===point.index?'step':'false');b.dataset.routeControl='step-'+index;b.title=tr(step.title);progress.append(b);});
  const next=route.transitions[point.index],bridge=el('div','journey-bridge');
  bridge.append(el('span','eyebrow',point.finished?t('routeEnd'):next?t('nextRelation'):t('lastStep')));
  if(next){bridge.append(p(tr(next.body)));bridge.append(btn(t('connections')+' ↗',()=>chooseEdge(next.graphEdgeId),'journey-edge-link'));}
  else bridge.append(p(tr(point.finished?route.conclusion:route.steps[point.index].question??route.question)));
  const navigation=el('div','journey-navigation'),prev=btn('← '+t('previous'),()=>moveRoute(point.index-1),'text-button');prev.disabled=point.index===0;
  navigation.append(prev,btn(t('openStep'),()=>moveRoute(point.index),'journey-current'),btn(point.finished?t('allRoutes')+' →':next?t('next')+' →':t('gather')+' ↗',point.finished?showRoutes:next?()=>moveRoute(point.index+1):finishRoute,'primary'));
  [...navigation.children].forEach((button,index)=>button.dataset.routeControl=['previous','current','next'][index]);dock.append(progress,bridge,navigation);
 }

 function resetView(){journey.leave();lens='tree';collection=null;neighborhood=null;path=null;selected=selectedEdge=null;pair=null;deck=true;reflect();sky.frame();}
 function setLens(id){journey.leave();lens=id;neighborhood=path=null;selected=selectedEdge=null;pair=null;reflect();sky.frame();}
 function openCollection(a){return attempt(()=>{model.growAtlas({nodeIds:a.nodeIds,edgeIds:a.edgeIds});journey.leave();collection=a;lens='tree';selected=selectedEdge=null;neighborhood=path=null;deck=false;drawer.hidden=true;reflect();sky.frame();return true;});}
 function choose(id){
  const active=journey.currentRoute();if(active&&!pair){const index=active.steps.findIndex(step=>step.graphNodeId===id);if(index>=0)journey.go(index);else{journey.leave();path=null;}}
  if(pair&&pair.from!==id){const current=pair;pair=null;if(current.mode==='connect')relationDialog(current.from,id);else if(current.mode==='compare')compare(current.from,id);else{path=shortestPath(state(),current.from,id,enabled);if(!path)notice(t('noPath'));else{collection=neighborhood=null;selected=selectedEdge=null;lens='tree';deck=false;}}reflect();if(path)sky.frame();return;}
  const outside=!view.nodes.some(n=>n.id===id);if(outside){lens='tree';collection=neighborhood=path=null;}selected=id;selectedEdge=null;readingTab='meaning';drawer.hidden=true;reading.scrollTop=0;reflect();if(outside)sky.frame();
 }
 function chooseEdge(id){if(!state().edges.some(e=>e.id===id))return;selectedEdge=id;selected=null;readingTab='meaning';reading.scrollTop=0;reflect();}
 function closeReading(){selected=selectedEdge=null;reflect();}
 function startPair(mode){if(!selected)return;journey.leave();path=collection=neighborhood=null;lens='tree';pair={mode,from:selected};notice(`${t(mode)} · ${t('pick')}`);reflect();}
 function renderReading(){
  reading.replaceChildren();if(!selected&&!selectedEdge)return;
  if(selectedEdge){const edge=state().edges.find(e=>e.id===selectedEdge);if(!edge)return;const top=el('div','reading-top');top.append(el('span','eyebrow',t(edge.kind)),btn('×',closeReading,'icon'));top.lastChild.setAttribute('aria-label',t('close'));reading.append(top,el('h1','',edgeLabel(edge)),p(edgeBody(edge)));
    const inquiry=atlasEdges.get(edge.id)?.inquiry;if(inquiry){reading.append(inquiryDetail(t('relationWarrant'),inquiry.warrant,true));appendGrounds(reading,inquiry.grounds);if(inquiry.limit)reading.append(inquiryDetail(t('relationLimit'),inquiry.limit));appendInquiryQuestion(reading,inquiry.question);}
    const endpoints=el('div','edge-endpoints');endpoints.append(btn(label(node(edge.from)),()=>choose(edge.from)),el('span','','↓ '+t(edge.kind)),btn(label(node(edge.to)),()=>choose(edge.to)));reading.append(endpoints,p(t('relationDemo'),'posture'),btn(t('remove'),()=>attempt(()=>{model.removeEdge(edge.id);selectedEdge=null;reflect();}),'remove-button'));return;}
  const n=node(selected);if(!n)return;const m=material(n),top=el('div','reading-top');top.append(el('span','eyebrow',t(n.kind)),btn('×',closeReading,'icon'));top.lastChild.setAttribute('aria-label',t('close'));reading.append(top,el('h1','',label(n)));
  const fragmentBinding=fragments?.forMaterial(m?.id);
  if(fragmentBinding){const available=fragmentBinding.passages.filter(passage=>passage.status==='available'),count=available.length,codes=readerVersionCodes({versions:Object.assign({},...available.map(passage=>passage.versions))}).map(code=>code.toUpperCase()).join(' / '),entry=btn('▤ '+t(count?'readFragment':'sourceAccess'),()=>fragmentDialog(m),'fragment-entry');entry.dataset.fragmentEntry=m.id;entry.append(el('span','',count>1?`${count} · ${codes}`:count?codes:t('sourceLink')));reading.append(entry);}
  else if(fragmentError&&m)reading.append(p(t('fragmentLoadError'),'muted'));
  const active=journey.currentRoute(),point=journey.getState();if(active&&active.steps[point.index].graphNodeId===n.id){const step=active.steps[point.index],lead=el('section','step-lead');lead.append(el('span','eyebrow',`${t('step')} ${point.index+1} ${t('of')} ${active.steps.length}`),el('h2','',tr(step.title)),p(tr(step.body),'step-guidance'));if(point.finished)lead.append(el('h2','',t('routeEnd')),p(tr(active.conclusion),'route-conclusion'),btn(t('routes')+' →',showRoutes,'text-button'));else if(step.question)lead.append(p(tr(step.question),'step-question'));appendGrounds(lead,point.finished?active.grounds:step.grounds,m?.id);reading.append(lead);}
  const tabs=el('nav','reading-tabs');for(const k of ['meaning','connections','source']){const b=btn(t(k),()=>{readingTab=k;renderReading();});b.setAttribute('aria-pressed',String(readingTab===k));tabs.append(b);}reading.append(tabs);
  const connections=state().edges.filter(e=>e.from===n.id||e.to===n.id);
  if(readingTab==='meaning'){
   if(m?.quote){const q=el('blockquote','quote',tr(m.quote));reading.append(q);if(m.quoteNote)reading.append(p(tr(m.quoteNote),'muted'));}reading.append(p(body(n)));
   if(m?.speaker)reading.append(p(tr(m.speaker),'muted'));
   if(m?.inquiry){const inquiry=m.inquiry,group=el('section','node-inquiry');group.append(el('span','eyebrow',t('inquiry')),inquiryDetail(t('argument'),inquiry.argument,true));
    appendGrounds(group,inquiry.grounds,m.id);
    if(inquiry.counterReading){const alternative=inquiryDetail(t('counterReading'),inquiry.counterReading.text);appendGrounds(alternative,inquiry.counterReading.grounds,m.id);group.append(alternative);}
    appendInquiryQuestion(group,inquiry.question);
    if(inquiry.experiment){const experiment=inquiryDetail(t('experiment'),inquiry.experiment.setup);experiment.append(p(tr(inquiry.experiment.question),'inquiry-prompt'));group.append(experiment);}reading.append(group);}
   if(m?.perspectives?.length){const group=el('section','perspectives');group.append(el('h3','minor-title',t('perspectives')));for(const perspective of m.perspectives){const detail=el('details','perspective'),summary=el('summary','',tr(perspective.title));detail.append(summary,p(tr(perspective.body)));group.append(detail);}reading.append(group);}
   if(m?.example){const detail=el('details','thought-example');detail.append(el('summary','',t('example')),p(tr(m.example)));reading.append(detail);}
   if(m?.question&&!m?.inquiry?.question){const question=el('section','thought-question');question.append(el('span','eyebrow',t('consider')),p(tr(m.question)));reading.append(question);}
   const actions=el('div','reading-actions');actions.append(btn('✣ '+t('develop'),()=>draftDialog(null,n),'primary'),btn('⌁ '+t('near'),()=>{journey.leave();neighborhood=n.id;path=null;collection=null;lens='tree';reflect();sky.frame();}));reading.append(actions);
   reading.append(el('h3','minor-title',t('connections')));for(const edge of connections.slice(0,4))reading.append(relationRow(edge,n.id));
   if(connections.length>4)reading.append(btn(`${t('connections')} · ${connections.length} →`,()=>{readingTab='connections';renderReading();}));
  }else if(readingTab==='connections'){
   const modes=el('div','connect-modes');for(const k of ['connect','path','compare'])modes.append(btn(t(k),()=>startPair(k),'mode-button'));reading.append(modes);
   for(const edge of connections)reading.append(relationRow(edge,n.id));
  }else{
   reading.append(p(m?.demo?t('generated'):m?t('witness'):t('own'),'source-intro'));
   if(m&&!m.demo){reading.append(p(tr(m.sourceNote),'muted'));reading.append(btn(t('sourceText')+' ↗',()=>fragmentBinding?fragmentDialog(m):sourceDialog(m),'primary'));}
   if(m?.inquiry)appendGrounds(reading,m.inquiry.grounds,m.id);
   else if(m?.demo&&m.sourceRefs?.length)for(const ref of m.sourceRefs){const a=el('a','source-link',ref.label);if(/^https?:\/\//.test(ref.ref)){a.href=ref.ref;a.target='_blank';a.rel='noopener noreferrer';reading.append(a);}else reading.append(p(ref.label+' · '+ref.ref,'muted'));}
   const origins=connections.filter(e=>['supports','contains','interprets'].includes(e.kind));for(const edge of origins)reading.append(relationRow(edge,n.id));
  }
  const tools=el('div','reading-bottom');tools.append(btn('↔ '+t('connect'),()=>startPair('connect')),btn(t('path'),()=>startPair('path')),btn(t('compare'),()=>startPair('compare')));reading.append(tools);
  if(!m)reading.append(btn(t('edit'),()=>draftDialog(n),'text-button'));
  reading.append(p(m?.demo?t('demo'):m?t('witness'):t('own'),'posture'),btn(t('remove'),()=>attempt(()=>{model.removeNode(n.id);selected=null;reflect();}),'remove-button'));
 }
 function appendInquiryQuestion(parent,text){if(!text)return;const question=el('section','thought-question');question.append(el('span','eyebrow',t('readingQuestion')),p(tr(text)));parent.append(question);}
 function appendGrounds(parent,grounds,contextId){
  if(!grounds?.length)return;
  const section=el('section','textual-grounds');section.append(el('span','eyebrow',t('grounds')));
  for(const ground of grounds){
   const source=sourceReference(ground.ref),detail=el('details','textual-ground');detail.dataset.sourceReference=ground.ref;
   const summary=el('summary');summary.append(el('strong','',tr(source.author)+' · '+tr(source.work)),el('span','',tr(source.locator)));detail.append(summary,p(tr(ground.focus),'ground-focus'));
   const available=source.passageId&&fragments?.data.passages.some(doc=>doc.id===source.passageId&&doc.status==='available');
   if(available){const open=btn(t('readGround')+' ↗',()=>{if(dialog.open)dialog.close();researchReader?.open({documentId:source.passageId,contextId:typeof contextId==='function'?contextId(ground.ref):contextId});},'text-button');open.dataset.inquiryPassage=source.passageId;detail.append(open);}
   else{const link=el('a','source-link',(source.access==='edition'?t('editionGround'):t('externalGround'))+(source.language?' · '+source.language.toUpperCase():'')+' ↗');link.href=source.url;link.target='_blank';link.rel='noopener noreferrer';detail.append(link);}
   section.append(detail);
  }
  parent.append(section);
 }
 function inquiryDetail(title,text,open=false){const detail=el('details','inquiry-detail');detail.open=open;detail.append(el('summary','',title),p(tr(text)));return detail;}
 function showInquiry(){const route=journey.currentRoute(),inquiry=route?.investigation??LENS_INQUIRY[lens],content=modal(route?tr(route.title):tr(LENSES.find(item=>item.id===lens).title));
  content.append(p(tr(route?.question??inquiry.question),'inquiry-leading'));
  for(const [field,label]of route?[['startingPoint','routeStarting'],['stakes','routeStakes'],['carryForward','routeCarry']]:[['method','lensMethod'],['blindSpot','lensLimit']])if(inquiry[field])content.append(el('h3','minor-title',t(label)),p(tr(inquiry[field])));
  appendGrounds(content,route?.grounds??inquiry.grounds,route?ref=>routeGroundContext(route,ref):undefined);
 }
 function relationRow(edge,id){const row=el('div','connection-row'),other=node(edge.from===id?edge.to:edge.from);const b=btn('',()=>chooseEdge(edge.id),'connection-target');b.append(el('small','',`${edge.from===id?'→':'←'} ${edgeLabel(edge)}`),el('span','',label(other)));b.style.setProperty('--relation-color',RELATIONS[edge.kind]?.color);row.append(b,btn('↗',()=>choose(other.id),'icon'));row.lastChild.setAttribute('aria-label',`${t('open')}: ${label(other)}`);return row;}
 function modal(title,wide=false){dialog.replaceChildren();dialog.classList.remove('fragment-dialog');delete dialog.dataset.passage;dialog.classList.toggle('wide',wide);const head=el('div','dialog-top');head.append(el('h2','',title),btn('×',()=>dialog.close(),'icon'));head.lastChild.setAttribute('aria-label',t('close'));dialog.append(head);const content=el('div','dialog-content');dialog.append(content);if(!dialog.open)dialog.showModal();return content;}
 function field(form,label,input){const wrapper=el('label','field');wrapper.append(el('span','',label),input);form.append(wrapper);return input;}
 function selectOptions(keys){const input=el('select');for(const k of keys){const option=el('option','',t(k));option.value=k;input.append(option);}return input;}
 function draftDialog(editing=null,context=null){
  const content=modal(editing?t('edit'):t('new')),form=el('form');content.append(form);
  const kind=field(form,t('kind'),selectOptions(['concept','interpretation','question','note','excerpt']));kind.value=editing?.kind??(context?'interpretation':'concept');kind.disabled=!!editing;
  const title=field(form,t('title'),el('input'));title.required=true;title.maxLength=250;title.value=editing?.title??(context?`${t('hypothesis')}: ${label(context)}`:'');
  const text=field(form,t('body'),el('textarea'));text.rows=5;text.maxLength=4000;text.value=editing?.body??(context?body(context):'');
  if(context)form.append(p(`${t('connections')}: ${label(context)}`,'muted'));
  const actions=el('div','dialog-actions');actions.append(btn(t('cancel'),()=>dialog.close()));const save=el('button','primary',t('save'));save.type='submit';actions.append(save);form.append(actions);
  form.onsubmit=e=>{e.preventDefault();attempt(()=>{if(editing)model.editDraft(editing.id,{title:title.value.trim(),body:text.value.trim()});else selected=model.addDraftWithContext({kind:kind.value,title:title.value.trim(),body:text.value.trim()},{...(context?{sourceId:context.id}:{}),relationKind:kind.value==='question'?'questions':'develops',reverse:kind.value==='question'});dialog.close();journey.leave();collection=neighborhood=path=null;lens='tree';reflect();sky.frame();});};title.focus();
 }
 function relationDialog(from,to){const content=modal(t('connect')),form=el('form');content.append(p(`${label(node(from))} → ${label(node(to))}`),form);const kind=field(form,t('relationKind'),selectOptions(Object.keys(RELATIONS).filter(k=>k!=='contains')));const reason=field(form,t('reason'),el('textarea'));reason.rows=3;reason.maxLength=500;reason.required=true;const save=el('button','primary',t('save'));save.type='submit';form.append(save);form.onsubmit=e=>{e.preventDefault();attempt(()=>{const id=model.connect(from,to,kind.value,reason.value.trim());dialog.close();chooseEdge(id);});};}
 function compare(from,to){const content=modal(t('comparison'),true),cols=el('div','comparison-grid');
  content.append(p(lang==='ru'?'Выберите общий вопрос для двух мыслей. Сравните их основания и условия; одинаковый вывод ещё не означает одинакового довода.':'Choose a shared question for the two thoughts. Compare their grounds and conditions; the same conclusion does not yet mean the same argument.','muted'));
  for(const id of [from,to]){const n=node(id),m=material(n),col=el('section');col.append(el('span','eyebrow',t(n.kind)),el('h3','',label(n)));if(m?.quote)col.append(el('blockquote','quote',tr(m.quote)));col.append(p(body(n)));if(m?.inquiry){col.append(inquiryDetail(t('argument'),m.inquiry.argument));appendGrounds(col,m.inquiry.grounds,m.id);if(m.inquiry.counterReading){const alternative=inquiryDetail(t('counterReading'),m.inquiry.counterReading.text);appendGrounds(alternative,m.inquiry.counterReading.grounds,m.id);col.append(alternative);}}col.append(btn(t('open')+' ↗',()=>{dialog.close();choose(id);}));cols.append(col);}content.append(cols);const neighbors=id=>new Set(state().edges.flatMap(e=>e.from===id?[e.to]:e.to===id?[e.from]:[])),a=neighbors(from),shared=[...neighbors(to)].filter(id=>a.has(id));if(shared.length){content.append(el('h3','',t('shared')));for(const id of shared)content.append(btn(label(node(id)),()=>{dialog.close();choose(id);},'shared-node'));}content.append(btn(t('connect')+' →',()=>relationDialog(from,to),'primary'));}
 function fragmentDialog(m){
  const binding=fragments?.forMaterial(m.id);if(!binding)return;
  researchReader?.open({documentId:binding.passages.find(item=>item.status==='available')?.id??binding.passages[0].id,contextId:m.id});
 }
 function sourceDialog(m){const content=modal(t('sourceText'));content.append(el('h3','',tr(m.title)));if(m.sourceNote)content.append(p(tr(m.sourceNote)));if(m.quoteNote)content.append(p(tr(m.quoteNote)));for(const code of ['de','ru'])if(m.exact?.[code])content.append(el('h4','',`${t('exact')} · ${code.toUpperCase()}`),el('blockquote','exact',m.exact[code]));for(const ref of m.sourceRefs??[])content.append(el('h4','',ref.label),el('code','ref',ref.ref));}
 function showMaterials(){drawer.hidden=!drawer.hidden;if(!drawer.hidden){renderMaterials();root.querySelector('.search').focus();}}
 let materialKind='all';function renderMaterials(){const q=root.querySelector('.search').value.trim().toLocaleLowerCase(),filters=root.querySelector('.material-filters');filters.replaceChildren();for(const k of ['all','concept','symbol','fragment','question','figure']){const b=btn(t(k),()=>{materialKind=k;renderMaterials();});b.setAttribute('aria-pressed',String(materialKind===k));filters.append(b);}const list=root.querySelector('.material-list');list.replaceChildren();const curated=new Set([...atlas.defaultIds,...(atlas.extensions??[]).flatMap(a=>a.nodeIds)]);const searchTexts=m=>[m.title,m.atlasBody??m.body,m.inquiry?.argument,m.inquiry?.question,m.inquiry?.counterReading?.text,...(m.inquiry?.grounds??[]).map(item=>item.focus),m.inquiry?.experiment?.setup,m.inquiry?.experiment?.question,...(m.perspectives??[]).flatMap(item=>[item.title,item.body])].map(tr),pool=library.nodes.filter(m=>curated.has(m.id)&&(materialKind==='all'||m.kind===materialKind)&&(!q||searchTexts(m).some(text=>text.toLocaleLowerCase().includes(q))));for(const m of pool){const b=btn('',()=>attempt(()=>{model.growAtlas({nodeIds:[m.id]});drawer.hidden=true;choose('material:'+m.id);}),'material-row');let summary=tr(m.atlasBody??m.body);if(q){const match=searchTexts(m).slice(1).find(text=>text.toLocaleLowerCase().includes(q));if(match){const start=Math.max(0,match.toLocaleLowerCase().indexOf(q)-65);summary=(start?'…':'')+match.slice(start,start+220)+(match.length>start+220?'…':'');}}b.append(el('small','',t(m.kind)),el('strong','',tr(m.title)),p(summary,'material-summary'));list.append(b);} }
 function collections(){const content=modal(t('collections'),true);content.append(p(lang==='ru'?'Готовые способы войти в Древо. В каждой сборке уже связаны тексты, вопросы и несколько возможных ответов.':'Ready ways into the tree. Each collection already connects texts, questions and several possible answers.'));const grid=el('div','collection-grid');atlas.assemblies.forEach(a=>{const b=btn('',()=>{if(openCollection(a))dialog.close();},'collection-option');b.append(el('h3','',tr(a.title)),p(tr(a.question)),el('small','',`${count(a.nodeIds.length,'stars')} · ${tr(a.subtitle)}`),el('span','',t('open')+' ↗'));grid.append(b);});content.append(grid);}
 function grow(){const content=modal(t('grow'));content.append(p(lang==='ru'?'Продолжите готовую мысль. Каждая ветвь добавит новые понятия и объяснённые связи с уже видимым Древом.':'Continue a prepared thought. Each branch adds new concepts and explained connections to the existing tree.'));for(const extension of atlas.extensions??[]){const b=btn('',()=>attempt(()=>{model.growAtlas({nodeIds:extension.nodeIds});dialog.close();journey.leave();collection=neighborhood=path=null;selected=selectedEdge=null;lens='tree';deck=false;reflect();sky.frame();notice(t('created'));}),'extension-option');const present=extension.nodeIds.every(id=>state().nodes.some(n=>n.materialId===id));b.append(el('h3','',tr(extension.title)),p(tr(extension.subtitle)),el('span','',present?`${t('open')} ↗`:`＋ ${count(extension.nodeIds.length,'stars')}`));content.append(b);}content.append(btn('＋ '+t('new'),()=>{dialog.close();draftDialog();},'primary'));}
 function toggleRelation(k){journey.leave();if(enabled.has(k))enabled.delete(k);else enabled.add(k);path=null;reflect();}
 function filters(){const content=modal(t('filter'));for(const [k,v]of Object.entries(RELATIONS)){const row=el('label','filter-row'),input=el('input');input.type='checkbox';input.checked=enabled.has(k);input.onchange=()=>toggleRelation(k);const dot=el('i');dot.style.background=v.color;row.append(input,dot,el('span','',t(k)));content.append(row);}content.append(p(lang==='ru'?'Фильтр меняет видимые отношения. Узлы и сохранённые связи остаются в Древе.':'The filter changes which relations are visible. Nodes and saved relations stay in the tree.','muted'));}
 function download(name,text){const a=el('a');a.href=URL.createObjectURL(new Blob([text],{type:'application/json'}));a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(a.href),1000);}
 function menu(){const content=modal(t('menu'));content.append(btn(t('routes'),showRoutes,'menu-action'),btn(t('freeCollections'),collections,'menu-action'),btn(t('export'),()=>download('tree-of-sophia.workspace.json',model.exportPacket()),'menu-action'),btn(t('import'),()=>{dialog.close();root.querySelector('.import-file').click();},'menu-action'),btn(t('restore'),()=>attempt(()=>{model.seedAtlas();dialog.close();resetView();}),'menu-action'),btn(t('clear'),()=>attempt(()=>{model.clear();dialog.close();resetView();}),'menu-action'),p('H · '+t('cinema')+' / F · '+t('fullscreen')+' / O · '+t('overview')+' / Ctrl+Z · '+t('undo'),'muted'));}
 function about(){const content=modal(t('about'));content.append(p(tr(INQUIRY_FOUNDATION.question),'inquiry-leading'),p(tr(INQUIRY_FOUNDATION.orientation)));
  for(const distinction of INQUIRY_FOUNDATION.distinctions)content.append(inquiryDetail(tr(distinction.title),distinction.body));
  appendGrounds(content,INQUIRY_FOUNDATION.grounds);
  content.append(p(tr(INQUIRY_FOUNDATION.practice)),p(lang==='ru'?'Прочтение открывает путь к конкретному месту книги. Его основание можно раскрыть рядом с мыслью, а собственную версию — сохранить и связать с текстом.':'A reading leads back to a specific passage. Open its grounds beside the thought, then save and connect your own reading to the text.','muted'),p(t('hint'),'muted'));}
 function toggleCinema(){cinema=!cinema;root.dataset.cinema=String(cinema);for(const selector of ['.header','.lens-rail','.view-heading','.collection-deck','.relation-bar','.footer','.materials-panel','.reading','.journey-dock'])root.querySelector(selector).inert=cinema;root.querySelector('.show-panels').hidden=!cinema;reflect();}
 async function fullscreen(){try{if(document.fullscreenElement)await document.exitFullscreen();else await root.requestFullscreen();}catch(error){notice(error.message);}}
 if(fragments)researchReader=createResearchReader({host:root,documents:fragments.data.passages,notebook:createReaderNotebook({storage}),locale:()=>lang,
  related:documentId=>{const result=new Map([...fragments.bindings.values()].filter(binding=>binding.passageIds.includes(documentId)).map(binding=>[binding.nodeId,{id:binding.nodeId,title:materials.get(binding.nodeId).title,context:binding.context}]));for(const m of materials.values()){const focus=readingContexts.get(m.id)?.get(documentId);if(focus)result.set(m.id,{id:m.id,title:m.title,context:focus});}return [...result.values()];},
  context:(id,documentId)=>{const binding=fragments.bindings.get(id),focus=readingContexts.get(id)?.get(documentId);return focus??(binding?.passageIds.includes(documentId)?binding.context:'');},
  guide:documentId=>readingGuides.get(documentId),
  onReveal:id=>{model.growAtlas({nodeIds:[id]});journey.leave();collection=neighborhood=path=pair=null;lens='tree';choose('material:'+id);sky.frame();},
  onDevelop:({document,text,citation,contextId})=>{
   const candidates=[...fragments.bindings.values()].filter(binding=>binding.passageIds.includes(document.id)),inquiryContext=readingContexts.get(contextId)?.has(document.id),materialId=inquiryContext?contextId:candidates.find(binding=>binding.nodeId===contextId)?.nodeId??candidates[0]?.nodeId;
   const body=text+'\n\n'+citation;if(body.length>4000)throw Error(lang==='ru'?'Заметка вместе с источником превышает 4000 знаков. Сократите мысль перед добавлением в Древо.':'The note and citation exceed 4000 characters. Shorten the thought before adding it to the tree.');
   selected=model.addDraftWithContext({kind:'note',title:(text.split('\n')[0].slice(0,180)+' · '+tr(document.title)).slice(0,250),body},{...(materialId?{materialId}:{}),relationKind:'develops'});
   selectedEdge=null;journey.leave();collection=neighborhood=path=pair=null;lens='tree';reflect();sky.frame();
  }
 });
 const actions={reader:()=>{if(researchReader)researchReader.open();else notice(t('fragmentLoadError')+(fragmentError?': '+fragmentError:''));},materials:showMaterials,routes:showRoutes,resume:resumeRoute,collections,grow,create:()=>draftDialog(),menu,about,inquiry:showInquiry,filters,return:resetView,restore:()=>{model.seedAtlas();resetView();},'close-materials':()=>drawer.hidden=true,'cancel-pair':()=>{pair=null;reflect();},undo:()=>model.undo(),redo:()=>model.redo(),overview:()=>sky.frame(),motion:()=>{moving=!moving;sky.motion(moving);root.querySelector('[data-action="motion"]').setAttribute('aria-pressed',String(moving));},fullscreen,cinema:toggleCinema};
 root.querySelectorAll('[data-action]').forEach(b=>b.addEventListener('click',()=>attempt(actions[b.dataset.action])));
 root.querySelectorAll('[data-lang]').forEach(b=>b.onclick=()=>{lang=b.dataset.lang;const url=new URL(location.href);url.searchParams.set('lang',lang);history.replaceState(null,'',url);reflect();});
 root.querySelector('.search').oninput=renderMaterials;
 root.querySelector('.import-file').onchange=async e=>{const file=e.target.files[0];if(!file)return;if(file.size>1_000_000)notice('File exceeds 1 MB');else{const text=await file.text();attempt(()=>{model.importPacket(text);resetView();});}e.target.value='';};
 document.addEventListener('keydown',e=>{if(dialog.open||researchReader?.isOpen()||e.target.closest('input,textarea,select,[contenteditable="true"]'))return;const k=e.key.toLowerCase();if((e.ctrlKey||e.metaKey)&&k==='z'){e.preventDefault();attempt(()=>e.shiftKey?model.redo():model.undo());}else if(!e.ctrlKey&&!e.metaKey){if(k==='h')toggleCinema();if(k==='f')fullscreen();if(k==='o')sky.frame();if(k==='/'){e.preventDefault();showMaterials();}if(k==='escape'){pair=null;drawer.hidden=true;closeReading();}}});
 dialog.addEventListener('click',e=>{if(e.target===dialog){const r=dialog.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)dialog.close();}});
 model.subscribe(reflect);reflect();sky.frame();root.dataset.ready='true';
 if(carried.copied.includes('tree'))notice(lang==='ru'?'Ваше личное поле перенесено в новую смысловую редакцию. Прежняя копия сохранена.':'Your personal workspace has moved to the new semantic edition. The earlier copy is preserved.');
 }
}catch(error){root.replaceChildren(el('h1','loading',t('error')),p(error.message,'loading'));}
