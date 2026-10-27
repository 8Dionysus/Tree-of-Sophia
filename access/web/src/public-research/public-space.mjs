import {mountConstructorSky} from '../../constructor/sky.mjs';
import {LENSES, RELATIONS, makeLensView, shortestPath} from '../../constructor/atlas-view.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {INQUIRY_FOUNDATION, LENS_INQUIRY} from '../../constructor/inquiry-foundation.mjs';
import {mountCollectionView} from './collection-view.mjs';
import {upgradePublicGraph} from './graph-transfer.mjs';
import {localizePublicJourney} from './journey-i18n.mjs';
import {displayPublicError} from './presentation-i18n.mjs';
import {describeSourceLocator} from './source-address.mjs';
import '../../constructor/style.css';
import './public-space.css';

const el=(tag,cls='',value)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
const button=(value,cls='')=>{const node=el('button',cls,value);node.type='button';return node;};
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const translations={
  ru:{space:'Пространство исследования',profiles:'Профили',kickerZarathustra:'ЗАРАТУСТРА · ПУБЛИЧНЫЕ ИЗДАНИЯ',kickerAntiquity:'АНТИЧНЫЙ МИР · ПУБЛИЧНЫЕ ИЗДАНИЯ',search:'Искать тексты, понятия и вопросы…',lenses:'ЛИНЗЫ',tree:'Древо',read:'Читать',library:'Библиотека',routes:'Маршруты',grow:'Вырастить ветвь',research:'Моё исследование',new:'Новая мысль',connect:'Связать мысли',compare:'Сравнить',fit:'Вписать всё',undo:'Отменить',redo:'Вернуть',fullscreen:'На весь экран',filter:'Связи',all:'Все связи',sources:'Источники',readings:'Прочтения',question:'Вопрос исследования',searchResults:'Найдено',emptySearch:'Совпадений нет.',select:'Выберите звезду или линию, чтобы открыть её основание и точный источник.',selected:'Выбрано',kind:'Тип',posture:'Статус',demo:'Предложенное прочтение для исследования · не канон',personal:'Личная гипотеза · сохранена в общем исследовании',sourceExact:'Точная версия и место доступны в публичном чтении.',sourceWork:'Публичная запись произведения; точный фрагмент не закреплён.',sourceMissing:'Для этой ссылки полный текст здесь не представлен.',openExact:'Открыть точное место',openWork:'Открыть произведение',sourceRefs:'Указатели на источник',body:'Прочтение',questionLabel:'Вопрос',connections:'Связи',related:'Перейти к узлу',relation:'Тип связи',edgeBody:'Обоснование связи',readRoute:'Начать чтение',routeStart:'Маршрут исследования',routeNext:'Следующий шаг',routePrevious:'Предыдущий шаг',step:'Шаг',noRoutes:'Для этого профиля ещё нет маршрутов.',growTitle:'Ветви исследования',growIntro:'Добавьте подготовленную линию в личное пространство или начните собственную.',createBranch:'Создать свою мысль',save:'Сохранить',cancel:'Отмена',title:'Название',text:'Текст мысли',newNode:'Новая мысль',newEdge:'Связать две мысли',chooseFirst:'Выберите первую звезду, затем вторую.',chooseSecond:'Теперь выберите вторую звезду.',edgeLabel:'Коротко объясните, что связывает мысли',chooseRelation:'Вид связи',connected:'Связь добавлена.',saved:'Сохранено в личном исследовании и графе.',saveError:'Не удалось сохранить мысль в общем исследовании.',comparePick:'Выберите две звезды для сопоставления.',comparison:'Сопоставление',comparePrompt:'Сравните основания, различия и условия, не превращая соседство в доказательство.',start:'Открыть',empty:'В этом пространстве пока нет узлов.',countNodes:'узлов',countEdges:'связей',record:'Записано в этом браузере',close:'Закрыть',linkOnly:'Только библиографическая ссылка',resetRoute:'Закрыть маршрут',routeEvidence:'Точные места маршрута открываются в Чтении.',imagine:'Линза задаёт способ чтения; она не меняет исходное Древо.',unavailable:'Источник не удалось открыть.',export:'Экспорт пространства',import:'Импорт пространства',source:'Источник',work:'Произведение',part:'Часть',fragment:'Фрагмент',chapter:'Глава',concept:'Понятие',interpretation:'Прочтение',question:'Вопрос',note:'Заметка',dossier:'Досье',figure:'Мыслитель',tradition:'Традиция',symbol:'Образ'},
  en:{space:'Research space',profiles:'Profiles',kickerZarathustra:'ZARATHUSTRA · PUBLIC EDITIONS',kickerAntiquity:'ANCIENT WORLDS · PUBLIC EDITIONS',search:'Search texts, concepts and questions…',lenses:'LENSES',tree:'The tree',read:'Read',library:'Library',routes:'Routes',grow:'Grow a branch',research:'My research',new:'New thought',connect:'Connect thoughts',compare:'Compare',fit:'Fit all',undo:'Undo',redo:'Redo',fullscreen:'Fullscreen',filter:'Relations',all:'All relations',sources:'Sources',readings:'Readings',question:'Research question',searchResults:'Results',emptySearch:'No matches.',select:'Select a star or line to open its reading and source.',selected:'Selected',kind:'Type',posture:'Status',demo:'Proposed reading for exploration · not canon',personal:'Personal hypothesis · saved in shared research',sourceExact:'The exact version and place are available in the public reader.',sourceWork:'Public work record; no exact passage is bound.',sourceMissing:'Full text for this reference is not supplied here.',openExact:'Open exact passage',openWork:'Open work',sourceRefs:'Source references',body:'Reading',questionLabel:'Question',connections:'Connections',related:'Open node',relation:'Relation type',edgeBody:'Why these thoughts are connected',readRoute:'Start reading',routeStart:'Research paths',routeNext:'Next step',routePrevious:'Previous step',step:'Step',noRoutes:'No routes are available for this profile yet.',growTitle:'Research branches',growIntro:'Add a prepared line to your space or begin your own.',createBranch:'Create your own thought',save:'Save',cancel:'Cancel',title:'Title',text:'Thought',newNode:'New thought',newEdge:'Connect two thoughts',chooseFirst:'Choose the first star, then the second.',chooseSecond:'Now choose the second star.',edgeLabel:'Briefly explain what connects these thoughts',chooseRelation:'Relation type',connected:'Connection added.',saved:'Saved in personal research and the graph.',saveError:'Could not save this thought in shared research.',comparePick:'Choose two stars to compare.',comparison:'Comparison',comparePrompt:'Compare grounds, differences and conditions without treating proximity as proof.',start:'Open',empty:'This space has no nodes yet.',countNodes:'nodes',countEdges:'relations',record:'Saved in this browser',close:'Close',linkOnly:'Bibliographic link only',resetRoute:'Close route',routeEvidence:'Exact route locations open in the reader.',imagine:'A lens offers a way to read; it does not change the source tree.',unavailable:'The source could not be opened.',export:'Export space',import:'Import space',source:'Source',work:'Work',part:'Part',fragment:'Passage',chapter:'Chapter',concept:'Concept',interpretation:'Interpretation',question:'Question',note:'Note',dossier:'Dossier',figure:'Thinker',tradition:'Tradition',symbol:'Image'},
};
const tr=(value,locale)=>{
  if(typeof value==='string')return value;
  if(!object(value))return '';
  return value[locale]??value.en??value.ru??'';
};
const extraTranslations={
  ru:{addToSpace:'Добавить в пространство',removeFromSpace:'Убрать из пространства',removedFromSpace:'Узел убран из пространства. Отмена восстановит его связи; текст остаётся в библиотеке.',previewOnly:'Просмотр материала · пространство не изменено',authorWorks:'Произведения автора',visibleCount:'В текущей выборке',recordScope:'Древо и личные мысли сохранены в этом браузере',routeScope:'Маршрут и шаг сохранены в адресе страницы',lensInquiry:'Как исследовать этот вопрос',lensMethod:'Способ чтения',lensLimit:'Что остаётся вне линзы',argument:'Ход мысли',counterReading:'Другое прочтение',readingQuestion:'Вопрос к этому чтению',experiment:'Попробовать при чтении',perspectives:'Детали чтения',example:'Проверить на примере',relationWarrant:'На чём держится переход',relationLimit:'Предел этого сопоставления',nextRelation:'Следующий переход',routeEnd:'К чему пришли',routeStarting:'С чем начинаем',routeStakes:'Почему это меняет ответ',routeCarry:'Что проверить дальше',practice:'Практика',chooseQuestion:'Выберите вопрос',graphSaveError:'Не удалось сохранить граф в этом браузере. Он пока доступен в открытой вкладке.',steps:'шагов',stepsOne:'шаг',stepsFew:'шага',graphUndo:'Отменить действие в пространстве',graphRedo:'Вернуть действие в пространстве',bibliographicOnly:'Только библиографическое указание',externalBibliographic:'Внешняя библиографическая ссылка · доступ к полному тексту не заявлен',openExternal:'Открыть внешнюю библиографическую ссылку',basedOn:'Основано на',groundsFor:'Основание для',endpointSource:'Источник узла',endpointRelatedSources:'Ссылки связанных предложений',personalGrounds:'Основания личного исследования',personalRelations:'Предложения и связи',personalObjection:'Возражение к предложению',openGround:'Открыть основание',readWithin:'Читать в составе',containedSource:'Текст доступен внутри существующего издания',sourceLoading:'Сведения об источнике…'},
  en:{addToSpace:'Add to space',removeFromSpace:'Remove from space',removedFromSpace:'Node removed from the space. Undo restores its relations; the text remains in the library.',previewOnly:'Material preview · your space has not changed',authorWorks:'Works by this author',visibleCount:'In current selection',recordScope:'Tree and personal thoughts saved in this browser',routeScope:'Route and step are kept in the page address',lensInquiry:'How to investigate this question',lensMethod:'A way of reading',lensLimit:'What the lens leaves outside',argument:'The argument',counterReading:'Another reading',readingQuestion:'A question for this reading',experiment:'Try while reading',perspectives:'Details to follow',example:'Try an example',relationWarrant:'What grounds this transition',relationLimit:'Limits of this comparison',nextRelation:'The next transition',routeEnd:'What we have reached',routeStarting:'Where we begin',routeStakes:'Why this changes the answer',routeCarry:'What to examine next',practice:'Practice',chooseQuestion:'Choose a question',graphSaveError:'Could not save the graph in this browser. It remains available in this tab.',steps:'steps',stepsOne:'step',stepsFew:'steps',graphUndo:'Undo a space action',graphRedo:'Redo a space action',bibliographicOnly:'Bibliographic reference only',externalBibliographic:'External bibliographic link · full-text access is not asserted',openExternal:'Open external bibliographic link',basedOn:'Based on',groundsFor:'Grounds',endpointSource:'Source of the endpoint',endpointRelatedSources:'Sources from related proposals',personalGrounds:'Grounds of this personal inquiry',personalRelations:'Proposals and relations',personalObjection:'Objection to proposal',openGround:'Open ground',readWithin:'Read within',containedSource:'This text is available within an existing edition',sourceLoading:'Source details…'},
};
const materialMap=library=>new Map((library?.nodes??[]).map(item=>[item.id,item]));
const graphIdForMaterial=id=>`material:${id}`;
const sourceKey=binding=>binding?[binding.documentId,binding.versionId,binding.unitId].join('\u0000'):'';
const copyPosition=value=>Array.isArray(value)&&value.length===3&&value.every(Number.isFinite)?value:null;
function downloadGraph(packet,name){const blob=new Blob([packet],{type:'application/json'}),url=URL.createObjectURL(blob),anchor=document.createElement('a');anchor.href=url;anchor.download=name;anchor.hidden=true;document.body.append(anchor);anchor.click();anchor.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);}

/** A public adapter over the existing constructor scene and its Rust graph owner. */
export function mountPublicResearchSpace({host,header,profile,locale=()=> 'ru',library,bindings=new Map(),documentBindings=new Map(),groundTargets=()=>[],routeGroundTargets=()=>null,graphPredecessors=[],api,workspace,shelf,question,saveState,status,personalOwnerRoot,agentOwnerRoot,journeys=[]}={}){
  if(!host||!library||!api)throw new TypeError('The public space needs a host, library, and reader API.');
  const t=key=>{const lang=String(locale()||'ru').startsWith('en')?'en':'ru';return extraTranslations[lang][key]??translations[lang][key]??key;};
  const openPublicAddress=address=>api.open(Object.fromEntries(Object.entries(address).filter(([key])=>['documentId','versionId','unitId','revision','reference'].includes(key))));
  const materials=materialMap(library),atlas=library.atlas??{},routes=library.space?.routes??atlas.routes??[];
  const collectionMaterials=new Map((library.space?.collection??[]).map(item=>[item.materialId??item.id,{...item,id:item.materialId??item.id,kind:({author:'figure',school:'tradition'})[item.kind]??item.kind}]));
  const previewMaterials=new Map([...collectionMaterials,...materials]);
  const entryByLegacyId=new Map([...previewMaterials.values()].map(item=>[item.legacyId??item.id,item]));
  const lenses=Array.isArray(library.space?.lenses)?library.space.lenses:LENSES;
  const relationKinds=[...new Set([...(Array.isArray(library.space?.relationKinds)?library.space.relationKinds:Object.keys(RELATIONS)),'relates'])].filter(key=>RELATIONS[key]);
  const kindName=kind=>t(({work:'work',part:'part',chapter:'chapter',fragment:'fragment',concept:'concept',interpretation:'interpretation',question:'question',note:'note',dossier:'dossier',figure:'figure',tradition:'tradition',symbol:'symbol'})[kind]??'source');
  const publicTitle=()=>profile==='zarathustra'?(tr(materials.get('demo:recurrence')?.title,locale())||tr(library.space?.title,locale())||t('space')):(tr(library.space?.title,locale())||t('space'));
  const countUnit=(value,kind)=>{const n=Math.abs(Number(value)||0);if(String(locale()).startsWith('en'))return kind==='nodes'?(n===1?'node':'nodes'):(n===1?'relation':'relations');const mod100=n%100,mod10=n%10;if(kind==='nodes')return mod100>=11&&mod100<=14?'узлов':mod10===1?'узел':mod10>=2&&mod10<=4?'узла':'узлов';return mod100>=11&&mod100<=14?'связей':mod10===1?'связь':mod10>=2&&mod10<=4?'связи':'связей';};
  const countText=(value,kind)=>`${value} ${countUnit(value,kind)}`;
  const countSteps=value=>{const n=Number(value)||0;if(String(locale()).startsWith('en'))return `${n} ${n===1?t('stepsOne'):t('steps')}`;const mod100=Math.abs(n)%100,mod10=Math.abs(n)%10;return `${n} ${mod100>=11&&mod100<=14?t('steps'):mod10===1?t('stepsOne'):mod10>=2&&mod10<=4?t('stepsFew'):t('steps')}`;};
  const spaceKey=`tos.public-research.graph.v1.${profile}.${library.fingerprint}`;
  const baselineModel=createConstructorModel(library,{storage:null});
  let pristineGraphs;try{baselineModel.seedAtlas();pristineGraphs={[spaceKey]:baselineModel.exportPacket()};}finally{baselineModel.dispose();}
  let upgradeError=null;
  try{upgradePublicGraph(localStorage,{profile,fingerprint:library.fingerprint,predecessors:graphPredecessors,pristineGraph:pristineGraphs[spaceKey],validate:packet=>{const verifier=createConstructorModel(library,{storage:null});try{verifier.importPacket(packet);return verifier.exportPacket();}finally{verifier.dispose();}}});}catch(error){upgradeError=error;}
  const model=createConstructorModel(library,{key:spaceKey});
  let state=model.getState(),lens=lenses.some(item=>item.id==='tree')?'tree':lenses[0]?.id??'tree',enabled=new Set(relationKinds),selected=null,selectedEdge=null,searchValue='',route=null,routeIndex=0,compareIds=[],connectIds=[],interaction=null,panelMode='detail',personalState=workspace?.getState?.()??null,destroyed=false,routeDockTouched=false;
  const desktopMode=window.matchMedia('(min-width: 761px)');
  let browsingRoutes=false,personalRouteWasVisible=false,comparisonRouteWasVisible=false;
  const personalPositions=new Map();
  const materializedHypotheses=new Map(),managedDrafts=new Map();let syncingOwner=false;
  const positionsKey=`tos.public-research.graph-positions.v1.${profile}`;
  const ownersKey=`${spaceKey}.owners`;
  try{const saved=JSON.parse(localStorage.getItem(ownersKey)||'{}');if(object(saved))for(const [ownerId,graphId]of Object.entries(saved))if(typeof graphId==='string'&&state.nodes.some(node=>node.id===graphId))materializedHypotheses.set(ownerId,graphId);}catch{}
  let ownersPersistenceError=false,positionsPersistenceError=false;
  function bindHypothesis(ownerId,graphId){for(const [previousOwner,previousGraph]of materializedHypotheses)if(previousOwner!==ownerId&&previousGraph===graphId)materializedHypotheses.delete(previousOwner);materializedHypotheses.set(ownerId,graphId);}
  function saveOwnerBindings(){try{localStorage.setItem(ownersKey,JSON.stringify(Object.fromEntries(materializedHypotheses)));ownersPersistenceError=false;}catch{ownersPersistenceError=true;notify(t('graphSaveError'));}}
  try{const saved=JSON.parse(localStorage.getItem(positionsKey)||'{}');if(object(saved))for(const [key,value]of Object.entries(saved)){const p=copyPosition(value);if(p)personalPositions.set(key,p);}}catch{}
  const shell=el('section','pr-public-space');shell.id='tree';shell.dataset.profile=profile;shell.dataset.ready='false';shell.dataset.reading='false';
  const canvas=document.createElement('canvas');canvas.className='tree-sky';canvas.setAttribute('aria-label',t('space'));
  if(header){header.classList.add('header','ps-header');shell.append(header);}
  shell.append(canvas);
  const search=document.createElement('input');search.type='search';search.className='ps-search';search.autocomplete='off';search.placeholder=t('search');search.setAttribute('aria-label',t('search'));
  const searchResults=el('div','ps-search-results');searchResults.hidden=true;
  const lensRail=el('nav','lens-rail ps-lens-rail');lensRail.setAttribute('aria-label',t('lenses'));lensRail.append(el('p','eyebrow',t('lenses')));
  const lensList=el('div','lens-list');lensRail.append(lensList,button(`＋ ${t('new')}`,'new-thought'));
  const heading=el('header','view-heading');heading.append(el('p','view-kicker',t(profile==='antiquity'?'kickerAntiquity':'kickerZarathustra')),el('h1','',publicTitle()),el('p','',tr(library.space?.intro,locale())||t('imagine')));
  const mainActions=el('nav','ps-main-actions');mainActions.setAttribute('aria-label',t('research'));
  for(const [key,action] of [['lensInquiry',()=>showInquiry()],['routes',()=>toggleRoutes()],['grow',()=>openGrow()],['research',()=>togglePersonal()]]){const b=button(t(key),'ps-top-action');b.dataset.spaceAction=key;b.addEventListener('click',action);mainActions.append(b);}
  const deck=el('section','collection-deck ps-route-dock');deck.hidden=true;
  const relationBar=el('section','relation-bar ps-relations');
  const relationToggle=button(`⌘ ${t('filter')}`,'relation-toggle');
  const relationChips=el('div','relation-chips');relationBar.append(relationToggle,relationChips);
  const footer=el('footer','footer ps-footer');
  const posture=button('', 'posture-button');posture.append(el('i'),el('span','',t('posture')));
  const count=el('span','space-count');
  const footerTools=el('div','footer-tools');
  const statusLabel=el('span','save-state',t('record'));
  const reading=el('aside','reading ps-panel');reading.hidden=true;
  const panelTop=el('div','reading-top');const panelClose=button('×','icon');panelClose.setAttribute('aria-label',t('close'));panelTop.append(el('span','eyebrow',t('selected')),panelClose);
  const panelContent=el('div','ps-panel-content');reading.append(panelTop,panelContent);
  const personal=el('aside','reading ps-panel ps-personal');personal.hidden=true;
  const personalTop=el('div','reading-top');const personalClose=button('×','icon');personalClose.setAttribute('aria-label',t('close'));personalTop.append(el('span','eyebrow',t('research')),personalClose);
  const personalContent=el('div','ps-personal-content');
  if(question)personalContent.append(question);
  if(saveState)personalContent.append(saveState);
  if(status)personalContent.append(status);
  const workspaceRoot=el('div','ps-workspace-owner'),agentRoot=el('div','ps-agent-owner');
  if(personalOwnerRoot){personalOwnerRoot.classList.add('ps-workspace-owner');workspaceRoot.append(personalOwnerRoot);}
  if(agentOwnerRoot){agentOwnerRoot.classList.add('ps-agent-owner');agentRoot.append(agentOwnerRoot);}
  personalContent.append(workspaceRoot,agentRoot);
  if(personalContent.childNodes.length)personal.append(personalTop,personalContent);
  const compareBar=el('div','ps-compare-bar');compareBar.hidden=true;
  const dialog=document.createElement('dialog');dialog.className='ps-dialog';
  const sourceMap=new Map((bindings instanceof Map?bindings.entries():Object.entries(bindings??{})).map(([id,value])=>[id,value]));
  const workMap=new Map((documentBindings instanceof Map?documentBindings.entries():Object.entries(documentBindings??{})).map(([id,value])=>[id,value]));
  let journeyList=Array.isArray(journeys)?journeys:[];
  const importInput=document.createElement('input');importInput.type='file';importInput.accept='application/json,.json';importInput.hidden=true;importInput.setAttribute('aria-label',t('import'));
  shell.append(search,searchResults,lensRail,heading,mainActions,deck,relationBar,footer,reading,personal,compareBar,dialog,importInput);
  host.append(shell);

  const langButton=localeButtonContainer();
  if(header){const toolbar=header.querySelector('.main-tools, .pr-toolbar');if(toolbar)toolbar.append(langButton);}
  function localeButtonContainer(){
    const group=el('div','language ps-language');
    for(const value of ['ru','en']){const b=button(value.toUpperCase());b.dataset.publicLocale=value;b.setAttribute('aria-pressed',String(value===locale()));b.addEventListener('click',()=>{header?.dispatchEvent(new CustomEvent('public-space-locale',{detail:{locale:value}}));});group.append(b);}
    return group;
  }
  function setLocaleState(){
    const value=String(locale()||'ru').startsWith('en')?'en':'ru';
    shell.dataset.locale=value;document.documentElement.lang=value;
    for(const b of langButton.querySelectorAll('[data-public-locale]'))b.setAttribute('aria-pressed',String(b.dataset.publicLocale===value));
    search.placeholder=t('search');search.setAttribute('aria-label',t('search'));searchResults.setAttribute('aria-label',t('searchResults'));
    lensRail.querySelector('.eyebrow').textContent=t('lenses');lensRail.setAttribute('aria-label',t('lenses'));mainActions.setAttribute('aria-label',t('research'));importInput.setAttribute('aria-label',t('import'));
    heading.querySelector('.view-kicker').textContent=t(profile==='antiquity'?'kickerAntiquity':'kickerZarathustra');heading.querySelector('h1').textContent=publicTitle();heading.querySelector('p:last-child').textContent=tr(library.space?.intro,locale())||t('imagine');
    for(const b of mainActions.querySelectorAll('[data-space-action]'))b.textContent=t(b.dataset.spaceAction);
    const newThoughtButton=lensRail.querySelector('.new-thought');if(newThoughtButton)newThoughtButton.textContent=`＋ ${t('new')}`;
    relationToggle.textContent=t('filter');relationToggle.setAttribute('aria-label',t('filter'));posture.querySelector('span').textContent=t('posture');canvas.setAttribute('aria-label',t('space'));importInput.setAttribute('aria-label',t('import'));
    for(const item of uiActions){item.button.textContent=item.label;item.button.title=t(item.key);item.button.setAttribute('aria-label',t(item.key));}
    clearTimeout(notify.timer);notify.timer=null;statusLabel.textContent=savedStatus();
    panelTop.querySelector('.eyebrow').textContent=t('selected');personalTop.querySelector('.eyebrow').textContent=t('research');
    panelClose.setAttribute('aria-label',t('close'));personalClose.setAttribute('aria-label',t('close'));
    paintLenses();paintRelations();if(searchValue)renderSearch();if(route||!deck.hidden)renderRoutes();if(!compareBar.hidden)renderComparison();reflect();
  }
  const uiActions=[];
  function addTool(key,label,action,cls=''){
    const b=button(label,cls);b.title=t(key);b.setAttribute('aria-label',t(key));b.addEventListener('click',action);footerTools.append(b);uiActions.push({button:b,key,label});return b;
  }
  addTool('lensInquiry','?',()=>showInquiry(),'icon');
  addTool('routes','⌁',()=>toggleRoutes(),'icon');
  addTool('grow','＋',()=>openGrow(),'icon');
  addTool('research','◎',()=>togglePersonal(),'icon');
  addTool('connect','↗',()=>beginConnect(),'icon');
  addTool('compare','⇄',()=>beginCompare(),'icon');
  footerTools.append(el('i'));
  const undoButton=addTool('graphUndo','↶',()=>{model.undo();checkGraphPersistence();},'icon');
  const redoButton=addTool('graphRedo','↷',()=>{model.redo();checkGraphPersistence();},'icon');
  addTool('fit','⌖',()=>sky?.frame(),'icon');
  addTool('fullscreen','⛶',()=>void toggleFullscreen(),'icon');
  addTool('export','⇩',()=>{try{downloadGraph(model.exportPacket(),`tos-${profile}-research-tree.json`);}catch(error){notify(displayPublicError(error,locale));}},'icon');
  addTool('import','⇧',()=>importInput.click(),'icon');
  importInput.addEventListener('change',async()=>{const file=importInput.files?.[0];if(!file)return;try{if(file.size>1_000_000)throw new Error('constructor packet exceeds the 1 MB storage limit');model.importPacket(await file.text());selected=null;selectedEdge=null;notifyGraphResult(t('saved'));sky?.frame();}catch(error){notify(displayPublicError(error,locale));}finally{importInput.value='';}});
  posture.addEventListener('click',()=>toggleRelations());
  relationToggle.addEventListener('click',()=>toggleRelations());
  footer.append(posture,count,statusLabel,footerTools);
  lensRail.querySelector('.new-thought').addEventListener('click',()=>openNewThought());
  panelClose.addEventListener('click',closePanel);personalClose.addEventListener('click',closePersonal);
  search.addEventListener('input',()=>{searchValue=search.value.trim();renderSearch();});
  search.addEventListener('keydown',event=>{if(event.key==='Escape'){search.value='';searchValue='';searchResults.hidden=true;}if(event.key==='Enter')searchResults.querySelector('button')?.click();});
  document.addEventListener('keydown',keyboard);

  function savedStatus(){if(model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError)return t('graphSaveError');return `${t('recordScope')}${route?` · ${t('routeScope')}`:''}`;}
  function notify(message){statusLabel.textContent=message;clearTimeout(notify.timer);notify.timer=setTimeout(()=>{notify.timer=null;statusLabel.textContent=savedStatus();},3800);}
  function persistView(){
    const url=new URL(location.href);
    if(route){url.searchParams.set('treeRoute',`${route.source}:${route.id}`);url.searchParams.set('treeStep',String(routeIndex+1));}
    else{url.searchParams.delete('treeRoute');url.searchParams.delete('treeStep');}
    history.replaceState(history.state,'',url);
    if(!notify.timer)statusLabel.textContent=savedStatus();
  }
  function checkGraphPersistence(){if(model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError)notify(t('graphSaveError'));}
  function notifyGraphResult(successMessage){if(model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError)notify(t('graphSaveError'));else notify(successMessage);}
  function humanEdgeLabel(edge){const value=tr(edge?.label,locale()).trim();if(!value||/^[a-z0-9][a-z0-9_.-]*(?::[a-z0-9][a-z0-9_.-]*)+$/i.test(value))return '';return value;}
  function label(item){return tr(item?.title,locale())||item?.title||item?.id||'';}
  function preferredGroundTarget(ref,contextId){
    const targets=groundTargets(ref,contextId),language=String(locale()).startsWith('en')?'en':'de';
    return targets.find(target=>target.language===language&&target.scope==='passage')??targets.find(target=>target.language===language)??targets[0]??null;
  }
  function appendGrounds(parent,grounds,contextId){
    if(!grounds?.length)return false;
    const section=el('section','ps-grounds');section.append(el('h2','minor-title',t('sourceRefs')));
    for(const ground of grounds){
      const source=atlas.references?.[ground.ref],target=preferredGroundTarget(ground.ref,contextId),binding=sourceForRef(ground.ref)??sourceMap.get(ground.ref);
      const title=tr(target?.title,locale())||[tr(source?.author,locale()),tr(source?.work,locale()),tr(source?.locator,locale())].filter(Boolean).join(' · ')||ground.ref;
      const detail=el('details','textual-ground');detail.append(el('summary','',title));
      if(ground.focus)detail.append(el('p','ground-focus',tr(ground.focus,locale())));
      const address=target?.address??binding;
      if(address){const open=button(`↗ ${t(address.unitId?'openExact':'openWork')}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(address).catch(()=>notify(t('unavailable'))));detail.append(open);}
      else if(typeof source?.url==='string'&&/^https?:\/\//.test(source.url)){const link=el('a','source-link',`${title} ↗`);link.href=source.url;link.target='_blank';link.rel='noopener noreferrer';detail.append(link);}
      section.append(detail);
    }
    parent.append(section);return true;
  }
  function appendSourcePointers(parent,material){return appendGrounds(parent,material?.inquiry?.grounds,material?.id);}
  function inquiryDetail(parent,title,value,open=false){if(!value)return null;const detail=el('details','inquiry-detail');detail.open=open;detail.append(el('summary','',title),el('p','',tr(value,locale())));parent.append(detail);return detail;}
  function inquiryQuestion(parent,value){if(!value)return;const card=el('section','ps-question-card');card.append(el('span','eyebrow',t('readingQuestion')),el('p','',tr(value,locale())));parent.append(card);}
  function showInquiry(){
    const current=route?.source==='atlas'?routes.find(item=>item.id===route.id):null;
    const foundation=library.space?.inquiryFoundation??library.inquiryFoundation??(profile==='zarathustra'?INQUIRY_FOUNDATION:null);
    const inquiry=current?.investigation??library.space?.lensInquiry?.[lens]??library.lensInquiry?.[lens]??(profile==='zarathustra'?LENS_INQUIRY[lens]:null);
    dialog.replaceChildren();const content=makeDialog(t('lensInquiry'));
    const question=current?.question??inquiry?.question??foundation?.question??lenses.find(item=>item.id===lens)?.description;
    if(question)content.append(el('p','inquiry-leading',tr(question,locale())));
    if(foundation?.orientation)content.append(el('p','',tr(foundation.orientation,locale())));
    for(const [field,key] of [['method','lensMethod'],['blindSpot','lensLimit'],['startingPoint','routeStarting'],['stakes','routeStakes'],['carryForward','routeCarry']])if(inquiry?.[field]){content.append(el('h3','minor-title',t(key)),el('p','',tr(inquiry[field],locale())));}
    for(const item of foundation?.distinctions??[])inquiryDetail(content,tr(item.title,locale()),item.body);
    if(foundation?.practice)inquiryDetail(content,t('practice'),foundation.practice);
    appendGrounds(content,current?.grounds??inquiry?.grounds??foundation?.grounds);
    dialog.showModal();
  }
  function sourceForMaterial(material,node=null){
    const ids=[material?.materialId,material?.id,node?.sourceNodeId,material?.sourceNodeId].filter(Boolean);
    for(const id of ids){const binding=sourceMap.get(id);if(binding?.unitId)return {binding,exact:binding.precision!=='section-entry',containerTitle:binding.containerTitle,containerLocator:binding.containerLocator};}
    if(material?.publicAddress)return {binding:material.publicAddress,exact:material.publicAddress.precision!=='section-entry'};
    if(material?.kind!=='work')for(const ground of material?.inquiry?.grounds??[]){const target=preferredGroundTarget(ground.ref,material.id);if(target)return {binding:target.address,exact:true};}
    for(const id of ids){const binding=workMap.get(id);if(binding?.documentId&&binding?.versionId)return {binding,exact:false};}
    for(const id of ids){const source=materials.get(id);if(source?.publicDocumentId&&source?.publicVersionId)return {binding:{documentId:source.publicDocumentId,versionId:source.publicVersionId},exact:false};}
    if(material?.publicDocumentId&&material?.publicVersionId)return {binding:{documentId:material.publicDocumentId,versionId:material.publicVersionId},exact:false};
    return null;
  }
  function routeSource(step){
    const target=profile==='zarathustra'?routeGroundTargets(route?.id,routeIndex,String(locale()).startsWith('en')?'en':'de'):null;
    if(target){
      if(target.kind==='exact')return {binding:target.address,exact:true};
      const reference=atlas.references?.[target.ref];
      let externalUrl=target.kind==='external'&&/^https?:\/\//.test(reference?.url??'')?reference.url:null;
      if(externalUrl&&/^#page=\d+$/.test(target.fragment??'')){const url=new URL(externalUrl);url.hash=target.fragment;externalUrl=url.href;}
      return {reference,externalUrl};
    }
    const exact=(step?.grounds??[]).map(value=>sourceForRef(value.ref)).find(Boolean);
    if(exact)return {binding:exact,exact:true};
    const ids=[step?.sourceNodeId,step?.nodeId,step?.graphNodeId].filter(Boolean);
    for(const id of ids){const binding=sourceMap.get(id);if(binding?.unitId)return {binding,exact:binding.precision!=='section-entry',containerTitle:binding.containerTitle,containerLocator:binding.containerLocator};}
    for(const id of ids){const material=materials.get(id)??materials.get(id.replace(/^material:/,''));const source=sourceForMaterial(material,{sourceNodeId:step?.sourceNodeId});if(source)return source;}
    return null;
  }
  function sourceActionLabel(source){return source.containerTitle?`${t('readWithin')} ${tr(source.containerTitle,locale())} · ${tr(source.containerLocator,locale())}`:t(source.exact?'openExact':'openWork');}
  function isBasedOn(edge){return String(edge.id).startsWith('atlas:antiquity:reading-source:')&&edge.kind==='supports'&&String(edge.from).includes('antiquity:reading:')&&String(edge.to).includes('antiquity:source:');}
  function relationName(edge,inverse=false){if(edge.proposalId)return tr(edge.label,locale());return isBasedOn(edge)?t(inverse?'groundsFor':'basedOn'):tr(RELATIONS[edge.kind],locale())||t('relation');}
  function personalProposals(ownerId){
    if(!ownerId)return [];
    return (workspace?.getState?.()?.proposals??personalState?.proposals??[]).filter(item=>item.parentHypothesisId===ownerId||item.targetId===ownerId||item.fromId===ownerId||item.toId===ownerId);
  }
  function appendPersonalGrounds(parent,proposals){
    const refs=[...new Set(proposals.flatMap(item=>[...(item.sourceRefs??[]),...(item.evidenceRefs??[])]))];
    if(!refs.length)return;
    const section=el('section','ps-personal-grounds');section.append(el('h2','minor-title',t('personalGrounds')));parent.append(section);
    for(const locator of refs){
      const open=button(t('sourceLoading'),'text-button');open.disabled=true;section.append(open);
      void describeSourceLocator(locator,shelf,api.provider,locale()).then(result=>{
        if(!section.isConnected)return;
        open.textContent=`↗ ${t('openGround')} · ${result.label}`;open.disabled=result.status!=='exact';
        if(result.reference)open.addEventListener('click',()=>void openPublicAddress({documentId:result.reference.target.workId,versionId:result.reference.versionId,unitId:result.reference.unitId,revision:result.reference.target.textLayerSha256,reference:result.reference}).catch(()=>notify(t('unavailable'))));
      }).catch(()=>{if(section.isConnected)open.textContent=t('sourceMissing');});
    }
  }
  function matForNode(node){return node?.materialId?previewMaterials.get(node.materialId):null;}
  function liveWorkspaceNodes(){
    const ownerState=workspace?.getState?.()??personalState;
    personalState=ownerState;
    const stored=new Map(state.nodes.map(node=>[node.id,node]));
    const byOwnerId=new Map((ownerState?.hypotheses??[]).map(item=>[item.id,item]));
    const graphId=ownerId=>{
      const mapped=materializedHypotheses.get(ownerId);
      return mapped&&stored.has(mapped)?mapped:stored.has(ownerId)?ownerId:`personal:${ownerId}`;
    };
    const materialNodes=[];
    for(const hypothesis of byOwnerId.values()){
      const id=graphId(hypothesis.id),saved=stored.get(id);
      const position=personalPositions.get(id)??personalPositions.get(`personal:${hypothesis.id}`)??saved?.position??[((materialNodes.length%5)-2)*130,240+Math.floor(materialNodes.length/5)*115,25];
      materialNodes.push({...saved,id,kind:saved?.kind??'note',title:hypothesis.title,body:hypothesis.body,sourceIds:saved?.sourceIds??[],position,localOnly:true,workspaceHypothesisId:hypothesis.id,ownerType:'personal-hypothesis',major:true,color:'#b3d9c4'});
    }
    const extraEdges=[];
    const resolve=id=>!id?null:byOwnerId.has(id)?graphId(id):state.nodes.find(node=>node.id===id||node.materialId===id)?.id;
    for(const hypothesis of byOwnerId.values()){
      const id=graphId(hypothesis.id),target=resolve(hypothesis.targetId);
      if(target&&target!==id)extraEdges.push({id:`personal-edge:${hypothesis.id}`,from:id,to:target,kind:'relates',label:locale()==='en'?'Personal link':'Личная связь',body:hypothesis.body,color:RELATIONS.relates.color,ownerType:'personal-hypothesis'});
      const from=resolve(hypothesis.fromId),to=resolve(hypothesis.toId);
      if(from&&to&&from!==to)extraEdges.push({id:`personal-relation:${hypothesis.id}`,from,to,kind:'relates',label:hypothesis.predicateLabel||(locale()==='en'?'Personal relation':'Личная связь'),body:hypothesis.body,color:RELATIONS.relates.color,ownerType:'personal-hypothesis'});
    }
    // These are projections of existing proposal relations, never new claims
    // written into the user's graph or admitted to the prepared library.
    for(const proposal of ownerState?.proposals??[]){
      if(proposal.kind!=='relation')continue;
      const from=resolve(proposal.fromId),to=resolve(proposal.toId);
      if(!from||!to||from===to)continue;
      const revision=proposal.id.startsWith('revision-link');
      const objection=proposal.id.startsWith('objection-link'),kind='relates';
      extraEdges.push({id:`personal-proposal:${proposal.id}`,from,to,kind,label:locale()==='en'?(revision?'Receives a reply or revision':objection?'Has an objection':'Personal relation'):(revision?'Получает ответ или пересмотр':objection?'Имеет возражение':'Личная связь'),body:proposal.statement,proposalId:proposal.id,color:RELATIONS[kind].color,ownerType:'personal-hypothesis'});
    }
    return {nodes:materialNodes,edges:extraEdges,byOwnerId};
  }
  function connectPersonalNodes(fromId,toId,kind,reason){
    const before=graphState(),nextBindings=[];
    const candidate=createConstructorModel(library,{storage:null});
    try{
      candidate.importPacket(model.exportPacket());
      const materialize=id=>{
        const node=before.nodes.find(item=>item.id===id);
        if(!node)throw new Error('A selected thought is no longer available.');
        if(node.routeContext)return candidate.addMaterial(node.materialId);
        if(node.ownerType!=='personal-hypothesis'||state.nodes.some(item=>item.id===id))return id;
        const graphId=candidate.addDraft({kind:node.kind,title:node.title,body:node.body,position:node.position});
        nextBindings.push([node.workspaceHypothesisId,graphId]);return graphId;
      };
      const from=materialize(fromId),to=materialize(toId),edgeId=candidate.connect(from,to,kind,reason);
      // Commit the fully validated operation once: cancellation and a failed
      // connection never materialize nodes, and one graph Undo reverses Save.
      model.importPacket(candidate.exportPacket());
      for(const [ownerId,graphId]of nextBindings)bindHypothesis(ownerId,graphId);
      saveOwnerBindings();reflect();checkGraphPersistence();return edgeId;
    }finally{candidate.dispose();}
  }
  function graphState(){
    const base=structuredClone(state),owner=liveWorkspaceNodes(),byId=new Map(base.nodes.map(n=>[n.id,n]));
    if(route?.source==='atlas'){
      const current=routes.find(item=>item.id===route.id);
      for(const step of current?.steps??[]){const materialId=step.nodeId??step.graphNodeId,material=materials.get(materialId),id=graphIdForMaterial(materialId);if(!material||byId.has(id))continue;
        const node={id,materialId,kind:material.kind,title:tr(material.title,locale()),body:tr(material.body,locale()),position:material.position??[0,0,0],sourceIds:[],routeContext:true};base.nodes.push(node);byId.set(id,node);}
      const existing=new Set(base.edges.map(edge=>edge.id));
      for(const edge of atlas.edges??[]){const from=graphIdForMaterial(edge.from),to=graphIdForMaterial(edge.to),id=`atlas:${edge.id}`;if(existing.has(id)||!byId.has(from)||!byId.has(to))continue;base.edges.push({...edge,id,from,to});existing.add(id);}
    }
    for(const n of owner.nodes){if(byId.has(n.id))Object.assign(byId.get(n.id),n);else{base.nodes.push(n);byId.set(n.id,n);}}
    const existingEdges=new Set(base.edges.map(e=>e.id));
    for(const edge of owner.edges)if(!existingEdges.has(edge.id))base.edges.push(edge);
    return base;
  }
  function visibleState(){
    const current=graphState();
    const projected=makeLensView(current,library,{lens,enabled:enabled.size===relationKinds.length?null:enabled,path:interaction||!compareBar.hidden?null:route?.path??null});
    projected.clusters=(projected.clusters??[]).map(cluster=>({...cluster,title:tr(cluster.title,locale()),subtitle:tr(cluster.subtitle,locale())}));
    projected.edges=projected.edges.map(edge=>({...edge,label:isBasedOn(edge)?relationName(edge):tr(edge.label,locale())||relationName(edge)}));
    projected.nodes=projected.nodes.map(node=>{
      const material=matForNode(node);
      if(node.ownerType==='personal-hypothesis')return {...node,title:node.title,body:node.body};
      return {...node,title:label(material)??node.title,color:node.color??material?.color};
    });
    if(selectedEdge)projected.selectedEdgeId=selectedEdge;
    return projected;
  }
  let sky;
  function refreshSky(){if(!sky)return;sky.update(visibleState(),Object.fromEntries(graphState().nodes.map(node=>[node.id,label(matForNode(node)??node)])));sky.select(selected);sky.selectEdge(selectedEdge);sky.refresh();}
  function reflect(){
    if(destroyed)return;state=model.getState();refreshSky();
    undoButton.disabled=!model.canUndo();redoButton.disabled=!model.canRedo();
    const current=visibleState();count.textContent=`${t('visibleCount')}: ${countText(current.nodes.length,'nodes')} · ${countText(current.edges.length,'edges')}`;if(!notify.timer)statusLabel.textContent=savedStatus();
    posture.querySelector('span').textContent=tr(library.space?.posture,locale())||t('posture');posture.setAttribute('aria-label',posture.textContent);
    if(personal.hidden){if(selectedEdge)renderEdge(selectedEdge);else if(selected)renderNode(selected);else if(reading.hidden&&!state.nodes.length)renderWelcome();}
    renderSearch();
  }
  function paintLenses(){
    lensList.replaceChildren();
    for(const item of lenses){const b=button('', 'lens');b.setAttribute('aria-pressed',String(lens===item.id));b.append(el('span','lens-icon',item.icon),el('span','',tr(item.title,locale())));b.title=tr(item.description,locale());b.addEventListener('click',()=>{lens=item.id;route=null;persistView();renderRoutes();paintLenses();reflect();});lensList.append(b);}
  }
  function paintRelations(){
    relationChips.replaceChildren();
    const all=button(t('all'),'relation-chip');all.setAttribute('aria-pressed',String(enabled.size===relationKinds.length));all.addEventListener('click',()=>{enabled=new Set(relationKinds);reflect();paintRelations();});relationChips.append(all);
    for(const key of relationKinds){const value=RELATIONS[key],b=button(tr(value,locale()),'relation-chip');b.style.setProperty('--relation-color',value.color);b.setAttribute('aria-pressed',String(enabled.has(key)));b.addEventListener('click',()=>{if(enabled.has(key))enabled.delete(key);else enabled.add(key);reflect();paintRelations();});relationChips.append(b);}
  }
  function renderWelcome(){
    reading.hidden=false;shell.dataset.reading='true';panelContent.replaceChildren(el('p','ps-welcome-mark','✧'),el('p','eyebrow',tr(library.space?.posture,locale())||t('posture')),el('p','ps-reading-intro',t('select')));
    const summary=el('div','ps-profile-summary');summary.append(el('strong','',countText(state.nodes.length,'nodes')),el('span','',countText(state.edges.length,'edges')));panelContent.append(summary);
    const actions=el('div','reading-actions');actions.append(button(t('routes'),'primary'));actions.lastChild.addEventListener('click',toggleRoutes);panelContent.append(actions);
  }
  function renderNode(id){
    const materialId=id.startsWith('material:')?id.slice(9):null;
    const preview=materialId&&previewMaterials.get(materialId);
    const node=graphState().nodes.find(item=>item.id===id)??(preview?{id,materialId,kind:preview.kind,sourceIds:[]}:null);if(!node){selected=null;return renderWelcome();}
    const inSavedGraph=state.nodes.some(item=>item.id===id),inView=graphState().nodes.some(item=>item.id===id);
    const material=matForNode(node),ownerHyp=(personalState?.hypotheses??[]).find(item=>item.id===node.workspaceHypothesisId||item.id===node.id||materializedHypotheses.get(item.id)===node.id);
    reading.hidden=false;personal.hidden=true;shell.dataset.reading='true';panelContent.replaceChildren();
    const nodeTitle=label(node.ownerType==='personal-hypothesis'?node:material??node);panelContent.append(el('p','eyebrow',`${t('kind')} · ${kindName(material?.kind??node.kind)}`),el('h1','',node.ownerType==='personal-hypothesis'&&nodeTitle.length>120?kindName(node.kind):nodeTitle));if(node.ownerType==='personal-hypothesis'&&nodeTitle.length>120)panelContent.append(el('p','ps-personal-title',nodeTitle));
    if(node.ownerType==='personal-hypothesis'){
      panelContent.append(el('p','posture',t('personal')));
      if(node.body)panelContent.append(el('p','body',node.body));
      if(ownerHyp?.targetId)panelContent.append(el('p','muted',`${t('related')}: ${ownerHyp.targetId}`));
      const proposals=personalProposals(ownerHyp?.id??node.workspaceHypothesisId);
      appendPersonalGrounds(panelContent,proposals);
      if(proposals.length){
        const list=el('section','ps-personal-proposals');list.append(el('h2','minor-title',t('personalRelations')));
        for(const proposal of proposals){
          const article=el('article','ps-personal-proposal');article.id=`ps-proposal-${proposal.id}`;
          article.append(el('p','posture',t('personal')),el('p','body',proposal.statement));
          const target=proposals.find(item=>item.id===proposal.targetId);
          if(target){const link=button(`${t('personalObjection')}: ${target.statement.slice(0,110)}`,'text-button');link.addEventListener('click',()=>{const destination=document.getElementById(`ps-proposal-${target.id}`);destination?.scrollIntoView({block:'nearest'});});article.append(link);}
          list.append(article);
        }
        panelContent.append(list);
      }
    }else if(material){
      const source=sourceForMaterial(material,node);
      if(source?.exact){panelContent.append(el('p','ps-source-status',t(source.containerTitle?'containedSource':'sourceExact')));const open=button(`↗ ${sourceActionLabel(source)}`,'primary');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));panelContent.append(open);}
      else if(source?.binding){panelContent.append(el('p','ps-source-status',t('sourceWork')));const open=button(`↗ ${t('openWork')}`,'primary');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));panelContent.append(open);}
      else if(material.sourceAccess==='metadata-only'||material.sourceRefs?.length){const external=(material.sourceRefs??[]).some(ref=>/^https?:\/\//.test(typeof ref==='string'?ref:ref.url??''))||(material.inquiry?.grounds??[]).some(ground=>/^https?:\/\//.test(atlas.references?.[ground.ref]?.url??''));panelContent.append(el('p','ps-source-status',t(external?'externalBibliographic':'bibliographicOnly')));}
      const postureLabel=material.demo===true||material.posture==='proposed'?'demo':material.sourceAccess;
      if(postureLabel==='demo')panelContent.append(el('p','posture',t('demo')));
      const body=material.atlasBody??material.body;
      if(body)panelContent.append(el('p','body',tr(body,locale())));
      if(tr(material.question,locale()).trim()&&!material.inquiry?.question){const questionCard=el('section','ps-question-card');questionCard.append(el('span','eyebrow',t('questionLabel')),el('p','',tr(material.question,locale())));panelContent.append(questionCard);}
      const inquiry=material.inquiry;
      if(inquiry){
        inquiryDetail(panelContent,t('argument'),inquiry.thesis??inquiry.argument,true);
        appendGrounds(panelContent,inquiry.grounds,material.id);
        if(inquiry.counterReading){const alternative=inquiryDetail(panelContent,t('counterReading'),inquiry.counterReading.text,true);if(alternative)appendGrounds(alternative,inquiry.counterReading.grounds,material.id);}
        inquiryQuestion(panelContent,inquiry.question);
        if(inquiry.experiment){const experiment=inquiryDetail(panelContent,t('experiment'),inquiry.experiment.setup);if(experiment&&inquiry.experiment.question)experiment.append(el('p','inquiry-prompt',tr(inquiry.experiment.question,locale())));}
      }
      if(material.perspectives?.length){panelContent.append(el('h2','minor-title',t('perspectives')));for(const item of material.perspectives)inquiryDetail(panelContent,tr(item.title,locale()),item.body);}
      if(material.example)inquiryDetail(panelContent,t('example'),material.example);
      if(!material.inquiry?.grounds?.length&&material.sourceRefs?.length){panelContent.append(el('h2','minor-title',t('sourceRefs')));for(const ref of material.sourceRefs){const address=typeof ref==='string'?ref:ref.url??ref.ref;const labelText=typeof ref==='string'?ref:tr(ref.label,locale())||ref.title||address;if(typeof address==='string'&&/^https?:\/\//.test(address)){const a=el('a','source-link',`${labelText} ↗`);a.href=address;a.target='_blank';a.rel='noopener noreferrer';panelContent.append(a);}else if(labelText)panelContent.append(el('p','muted',labelText));}}
    }else if(node.body)panelContent.append(el('p','body',node.body));
    const authorId=material?.legacyId??material?.id,works=material?.kind==='figure'?[...previewMaterials.values()].filter(item=>item.kind==='work'&&((material.works??[]).includes(item.legacyId??item.id)||(authorId&&item.author===authorId))):[];
    if(works.length){panelContent.append(el('h2','minor-title',t('authorWorks')));for(const work of works){const open=button(label(work),'connection-target');open.addEventListener('click',()=>previewMaterial(work.id));panelContent.append(open);}}
    const connections=graphState().edges.filter(edge=>edge.from===id||edge.to===id);
    if(connections.length){panelContent.append(el('h2','minor-title',t('connections')));for(const edge of connections){const otherId=edge.from===id?edge.to:edge.from;const other=graphState().nodes.find(item=>item.id===otherId);const row=button('', 'connection-target'),type=relationName(edge,edge.to===id),extra=humanEdgeLabel(edge);row.append(el('small','',`${isBasedOn(edge)||edge.from===id?'→':'←'} ${type}${extra&&extra!==type?` · ${extra}`:''}`),el('span','',label(other?.ownerType==='personal-hypothesis'?other:matForNode(other)??other)));row.style.setProperty('--relation-color',RELATIONS[edge.kind]?.color??'#a5b7cf');row.addEventListener('click',()=>selectNode(otherId));panelContent.append(row);}}
    if(material&&materials.has(material.id)){
      const edit=button(t(inSavedGraph?'removeFromSpace':'addToSpace'),'text-button');
      edit.addEventListener('click',()=>{try{if(inSavedGraph){model.removeNode(id);selected=id;notifyGraphResult(t('removedFromSpace'));}else{const added=model.addMaterial(material.id);state=model.getState();selectActual(added);notifyGraphResult(t('saved'));}reflect();}catch(error){notify(displayPublicError(error,locale));}});panelContent.append(edit);
    }
    if(!inView){panelContent.append(el('p','muted',t('previewOnly')));return;}
    const actions=el('div','reading-bottom');
    actions.append(button(`↔ ${t('compare')}`,'text-button'),button(`↗ ${t('connect')}`,'text-button'));
    actions.children[0].addEventListener('click',()=>{closePanel();beginCompare(id);});actions.children[1].addEventListener('click',()=>{closePanel();beginConnect(id);});panelContent.append(actions);
  }
  function findEdge(id){return graphState().edges.find(edge=>edge.id===id);}
  function renderEdge(id){
    const edge=findEdge(id);if(!edge){selectedEdge=null;return renderWelcome();}
    reading.hidden=false;personal.hidden=true;shell.dataset.reading='true';panelContent.replaceChildren();
    const from=graphState().nodes.find(node=>node.id===edge.from),to=graphState().nodes.find(node=>node.id===edge.to),atlasEdge=(atlas.edges??[]).find(item=>`atlas:${item.id}`===id||item.id===id);
    const relationLabel=relationName(edge),edgeLabel=humanEdgeLabel(edge);
    panelContent.append(el('p','eyebrow',t('relation')+' · '+relationLabel),el('h1','',edgeLabel&&edgeLabel.length<=120?edgeLabel:relationLabel));if(edgeLabel.length>120)panelContent.append(el('p','body',edgeLabel));
    if(edge.body||atlasEdge?.body)panelContent.append(el('p','body',tr(edge.body??atlasEdge.body,locale())));
    const inquiry=atlasEdge?.inquiry??edge.inquiry;if(inquiry){inquiryDetail(panelContent,t('relationWarrant'),inquiry.warrant,true);appendGrounds(panelContent,inquiry.grounds);inquiryDetail(panelContent,t('relationLimit'),inquiry.limit);inquiryQuestion(panelContent,inquiry.question);}
    const endpoints=el('div','edge-endpoints');const a=button(label(from?.ownerType==='personal-hypothesis'?from:matForNode(from)??from),'text-button'),b=button(label(to?.ownerType==='personal-hypothesis'?to:matForNode(to)??to),'text-button');a.addEventListener('click',()=>selectNode(from.id));b.addEventListener('click',()=>selectNode(to.id));endpoints.append(a,el('span','',relationLabel),b);panelContent.append(endpoints);
    const proposal=(workspace?.getState?.()?.proposals??[]).find(item=>item.id===edge.proposalId);
    if(proposal)appendPersonalGrounds(panelContent,[proposal]);
    for(const endpoint of [from,to]){
      if(endpoint?.ownerType==='personal-hypothesis'){
        const section=el('section','ps-endpoint-source');section.append(el('h2','minor-title',`${t('endpointRelatedSources')}: ${label(endpoint)}`));
        appendPersonalGrounds(section,personalProposals(endpoint.workspaceHypothesisId));panelContent.append(section);continue;
      }
      const source=sourceForMaterial(matForNode(endpoint),endpoint);
      if(!source)continue;
      const section=el('section','ps-endpoint-source');section.append(el('h2','minor-title',`${t('endpointSource')}: ${label(matForNode(endpoint)??endpoint)}`));
      const open=button(`↗ ${t(source.exact?'openExact':'openWork')}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));section.append(open);panelContent.append(section);
    }
    const actions=el('div','reading-actions'),compare=button(t('compare'),'text-button');compare.addEventListener('click',()=>{closePanel();beginCompare(from?.id,to?.id);});actions.append(compare);panelContent.append(actions);
  }
  function selectNode(id){
    if(interaction==='connect'){const nextId=id;if(!connectIds.length){connectIds=[nextId];notify(t('chooseSecond'));return;}if(nextId===connectIds[0])return;connectIds=[connectIds[0],nextId];interaction=null;openRelationDialog();return;}
    if(interaction==='compare'){if(!compareIds.includes(id))compareIds.push(id);if(compareIds.length===2){interaction=null;renderComparison();}return;}
    selectActual(id);
  }
  function previewMaterial(materialId){const id=graphIdForMaterial(materialId);selected=id;selectedEdge=null;searchValue='';search.value='';searchResults.hidden=true;search.blur();if(graphState().nodes.some(item=>item.id===id)){sky?.select(id);sky?.focus(id);}renderNode(id);}
  function selectActual(id){selected=id;selectedEdge=null;reading.hidden=false;personal.hidden=true;sky?.select(id);sky?.focus(id);renderNode(id);}
  function selectEdge(id){selectedEdge=id;selected=null;reading.hidden=false;personal.hidden=true;sky?.selectEdge(id);renderEdge(id);}
  function openPanel(){reading.hidden=false;shell.dataset.reading='true';sky?.refresh();}
  function closePanel(){reading.hidden=true;shell.dataset.reading='false';selected=null;selectedEdge=null;sky?.select(null);sky?.selectEdge(null);reflect();}
  function togglePersonal(){
    const opening=personal.hidden;
    if(!opening){closePersonal();return;}
    personalRouteWasVisible=!!route&&!deck.hidden;
    personal.hidden=false;reading.hidden=true;deck.hidden=true;shell.dataset.journey='false';shell.dataset.reading='true';
    personalState=workspace?.getState?.()??personalState;renderPersonalPanel();sky?.refresh();
  }
  function restoreRouteDock(){if(route){browsingRoutes=false;renderRoutes();deck.hidden=false;shell.dataset.journey='true';}}
  function closePersonal(){personal.hidden=true;shell.dataset.reading='false';if(personalRouteWasVisible)restoreRouteDock();personalRouteWasVisible=false;sky?.refresh();}
  function closeRoute(){routeDockTouched=true;route=null;browsingRoutes=false;persistView();deck.hidden=true;shell.dataset.journey='false';reflect();sky?.frame();}
  function toggleRoutes(){
    routeDockTouched=true;
    if(!deck.hidden&&browsingRoutes){deck.hidden=true;browsingRoutes=false;shell.dataset.journey='false';sky?.refresh();return;}
    browsingRoutes=true;renderRoutes();deck.hidden=false;shell.dataset.journey='true';reading.hidden=true;personal.hidden=true;shell.dataset.reading='false';sky?.refresh();
  }
  function routeItems(){return [...routes.map(item=>({...item,source:'atlas'})),...journeyList.map(item=>({...item,source:'manifest'}))];}
  function renderRoutes(){
    if(!deck)return;deck.dataset.active=String(!!route);deck.replaceChildren();const title=el('div','deck-caption');title.append(el('span','eyebrow',t('chooseQuestion')),button(t(route?'resetRoute':'close'),'text-button'));title.lastChild.addEventListener('click',closeRoute);deck.append(title);
    if(route&&!browsingRoutes&&route.source==='atlas'){
      const current=routes.find(item=>item.id===route.id),steps=current?.steps??[];const progress=el('div','ps-route-progress');
      for(let i=0;i<steps.length;i++){const b=button(String(i+1),'journey-stop');b.setAttribute('aria-current',String(i===routeIndex));b.setAttribute('aria-label',`${t('step')} ${i+1}`);b.addEventListener('click',()=>goRouteStep(i));progress.append(b);}deck.append(el('h2','ps-route-title',tr(current?.title,locale())),el('p','ps-route-question',tr(current?.question,locale())),progress);
      const step=steps[routeIndex];if(step){deck.append(el('h3','',tr(step.title,locale())),el('p','',tr(step.body,locale())));if(step.question)inquiryQuestion(deck,step.question);const nextStep=steps[routeIndex+1],transition=(current.transitions??[]).find(item=>(item.fromNodeId===step.nodeId&&item.toNodeId===nextStep?.nodeId)||(item.from===step.nodeId&&item.to===nextStep?.nodeId))??(profile==='zarathustra'?(current.transitions??[])[routeIndex]:null);if(nextStep&&transition?.body){deck.append(el('h3','minor-title',t('nextRelation')),el('p','',tr(transition.body,locale())));}else if(!nextStep&&current.conclusion){deck.append(el('h3','minor-title',t('routeEnd')),el('p','',tr(current.conclusion,locale())));}const actionRow=el('div','ps-route-actions');if(routeIndex>0){const previous=button(`← ${t('routePrevious')}`,'text-button');previous.addEventListener('click',()=>goRouteStep(routeIndex-1));actionRow.append(previous);}const source=routeSource(step);if(source?.binding){const open=button(`↗ ${t(source.exact?'openExact':'openWork')}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));actionRow.append(open);}else if(source){const lang={de:{ru:'немецкий',en:'German'},en:{ru:'английский',en:'English'},fr:{ru:'французский',en:'French'},grc:{ru:'древнегреческий',en:'Ancient Greek'},la:{ru:'латинский',en:'Latin'}}[source.reference?.language];const title=[tr(source.reference?.author,locale()),tr(source.reference?.work,locale()),tr(source.reference?.locator,locale()),lang&&tr(lang,locale())].filter(Boolean).join(' · ');if(source.externalUrl){const link=el('a','text-button',`↗ ${t('openExternal')} · ${title}`);link.href=source.externalUrl;link.target='_blank';link.rel='noopener noreferrer';actionRow.append(link);}else actionRow.append(el('p','muted',`${t('bibliographicOnly')}${title?` · ${title}`:''}`));}if(routeIndex<steps.length-1){const next=button(`${t('routeNext')} →`,'primary');next.addEventListener('click',()=>goRouteStep(routeIndex+1));actionRow.append(next);}deck.append(actionRow);}
      return;
    }
    if(route&&!browsingRoutes&&route.source==='manifest'){
      const current=localizePublicJourney(journeyList.find(item=>item.id===route.id),locale());const steps=current?.steps??[];const progress=el('div','ps-route-progress');
      for(let i=0;i<steps.length;i++){const b=button(String(i+1),'journey-stop');b.setAttribute('aria-current',String(i===routeIndex));b.setAttribute('aria-label',`${t('step')} ${i+1}`);b.addEventListener('click',()=>goJourneyStep(i));progress.append(b);}deck.append(el('h2','ps-route-title',current?.title??''),el('p','ps-route-question',current?.question??''),progress,el('h3','',steps[routeIndex]?.title??''),el('p','',steps[routeIndex]?.prompt??''));
      if(current?.readingHypothesis)inquiryDetail(deck,t('argument'),current.readingHypothesis);
      if(current?.strongAlternative)inquiryDetail(deck,t('counterReading'),current.strongAlternative);
      const actions=el('div','ps-route-actions');if(routeIndex>0){const previous=button(`← ${t('routePrevious')}`,'text-button');previous.addEventListener('click',()=>goJourneyStep(routeIndex-1));actions.append(previous);}
      const open=button(`↗ ${t('openExact')}`,'text-button');open.addEventListener('click',()=>goJourneyStep(routeIndex,true));actions.append(open);
      if(routeIndex<steps.length-1){const next=button(`${t('routeNext')} →`,'primary');next.addEventListener('click',()=>goJourneyStep(routeIndex+1));actions.append(next);}deck.append(actions,el('small','ps-route-source',t('routeEvidence')));return;
    }
    const cards=el('div','collection-cards');
    for(const item of routeItems()){
      const shown=item.source==='manifest'?localizePublicJourney(item,locale()):item;
      const b=button('','collection-card');b.append(el('span','collection-number',countSteps(shown.steps?.length??0)),el('strong','',tr(shown.title,locale())),el('small','',tr(shown.question??shown.description,locale())),el('span','collection-arrow','↗'));
      b.addEventListener('click',()=>startRoute(item));cards.append(b);
    }
    cards.tabIndex=0;cards.setAttribute('aria-label',t('routes'));
    if(!cards.childElementCount)cards.append(el('p','ps-route-empty',t('noRoutes')));deck.append(cards);
  }
  function syncInitialRouteDock(){if(routeDockTouched||route)return;if(desktopMode.matches){renderRoutes();deck.hidden=false;shell.dataset.journey='true';}else{deck.hidden=true;shell.dataset.journey='false';}sky?.refresh();}
  function startRoute(item){
    routeDockTouched=true;browsingRoutes=false;
    route={id:item.id,source:item.source,path:null};routeIndex=0;
    if(item.source==='atlas'){
      const ids=(item.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(Boolean);
      const materialIds=ids.map(id=>materials.has(id)?id:id.replace(/^material:/,''));
      const current=graphState(),nodeIds=materialIds.map(graphIdForMaterial).filter(id=>current.nodes.some(node=>node.id===id));
      route.path=pathThrough(nodeIds,current);if(nodeIds[0]){selected=nodeIds[0];sky?.select(nodeIds[0]);}
    }
    deck.hidden=false;shell.dataset.journey='true';renderRoutes();refreshSky();
    if(item.source==='manifest')goJourneyStep(0,false);else if(item.steps?.[0])goRouteStep(0);
  }
  function pathThrough(nodeIds,value){
    if(nodeIds.length<2)return {nodeIds,edgeIds:[]};const out=[nodeIds[0]],edgeIds=[];
    for(let i=1;i<nodeIds.length;i++){const partial=shortestPath(value,nodeIds[i-1],nodeIds[i]);if(!partial)continue;edgeIds.push(...partial.edgeIds);out.push(...partial.nodeIds.slice(1));}
    return {nodeIds:out,edgeIds:[...new Set(edgeIds)],restrictEdges:true};
  }
  function goRouteStep(index){
    const moveFocus=!!document.activeElement?.closest('.ps-route-dock');browsingRoutes=false;
    const item=routes.find(value=>value.id===route?.id),step=item?.steps?.[index];if(!step)return;routeIndex=index;persistView();
    const raw=step.nodeId??step.graphNodeId,materialId=materials.has(raw)?raw:raw?.replace(/^material:/,'');
    if(materialId){selected=graphIdForMaterial(materialId);selectedEdge=null;sky?.select(selected);sky?.focus(selected);}
    renderRoutes();reflect();if(moveFocus)deck.querySelector('[aria-current="true"]')?.focus();
  }
  function goJourneyStep(index,open=false){
    const moveFocus=!!document.activeElement?.closest('.ps-route-dock');browsingRoutes=false;
    const item=localizePublicJourney(journeyList.find(value=>value.id===route?.id),locale()),step=item?.steps?.[index];if(!step)return;routeIndex=index;persistView();renderRoutes();
    const address={documentId:step.documentId,versionId:step.versionId,unitId:step.unitId};
    const binding=[...sourceMap.values()].find(value=>value.documentId===address.documentId&&value.versionId===address.versionId&&value.unitId===address.unitId);
    const node=binding&&[...sourceMap.entries()].find(([,value])=>value===binding)?.[0];
    if(node){selected=graphIdForMaterial(node);sky?.select(selected);sky?.focus(selected);}
    if(open)void openPublicAddress(address).catch(()=>notify(t('unavailable')));else if(moveFocus)deck.querySelector('[aria-current="true"]')?.focus();
  }
  function sourceForRef(ref){
    for(const [id,value] of sourceMap)if(value.referenceId===ref||value.unitId===ref)return value;
    return null;
  }
  function openCollection(){
    if(dialog.open)dialog.close();dialog.replaceChildren();collectionRoot=makeDialog(t('library'));collectionRoot.classList.add('ps-collection');
    const collectionHeader=dialog.querySelector('.dialog-top');collectionHeader.insertBefore(localeButtonContainer(),collectionHeader.lastChild);
    const entries=Array.isArray(library.space?.collection)&&library.space.collection.length?library.space.collection:(library.nodes??[]);
    collectionView=mountCollectionView({root:collectionRoot,entries,locale,
      onReadCatalog:()=>{dialog.close();void api.openLibrary().catch(()=>notify(t('unavailable')));},
      hasMaterial:id=>materials.has(id),materialInGraph:id=>state.nodes.some(node=>node.materialId===id),
      onOpenMaterial:id=>{try{const graphId=model.addMaterial(id);checkGraphPersistence();state=model.getState();dialog.close();selectActual(graphId);reflect();}catch(error){notify(displayPublicError(error,locale));}},
      onOpenSource:item=>{const material=materials.get(item.materialId??item.id)??item;const source=sourceForMaterial(material,{sourceNodeId:item.sourceNodeId??material.sourceNodeId});if(!source)return null;
        const wrap=el('div','ps-collection-source');wrap.append(el('p','ps-source-status',t(source.containerTitle?'containedSource':source.exact?'sourceExact':'sourceWork')));const open=button(`↗ ${sourceActionLabel(source)}`,'primary');open.addEventListener('click',()=>{dialog.close();void openPublicAddress(source.binding).catch(()=>notify(t('unavailable')));});wrap.append(open);return wrap;}
    });dialog.showModal();
  }
  function openGrow(){
    dialog.replaceChildren();const content=makeDialog(t('growTitle'));content.append(el('p','ps-dialog-intro',t('growIntro')));
    const list=el('div','ps-branch-list');for(const branch of atlas.extensions??[]){const b=button('','ps-branch');b.append(el('strong','',tr(branch.title,locale())),el('span','',tr(branch.subtitle,locale())),el('small','',countText(branch.nodeIds?.length??0,'nodes')));b.addEventListener('click',()=>{model.growAtlas({nodeIds:branch.nodeIds??[],...(branch.edgeIds?{edgeIds:branch.edgeIds}:{})});dialog.close();notifyGraphResult(t('saved'));reflect();sky?.frame();});list.append(b);}content.append(list,button(`＋ ${t('createBranch')}`,'primary'));content.lastChild.addEventListener('click',openNewThought);dialog.showModal();
  }
  function openNewThought(){
    if(dialog.open)dialog.close();dialog.replaceChildren();const content=makeDialog(t('newNode'));const form=document.createElement('form');const kind=document.createElement('select');for(const value of ['concept','interpretation','question','note'])kind.append(new Option(tr({concept:{ru:'Понятие',en:'Concept'},interpretation:{ru:'Прочтение',en:'Reading'},question:{ru:'Вопрос',en:'Question'},note:{ru:'Заметка',en:'Note'}}[value],locale()),value));
    const title=document.createElement('input');title.maxLength=250;title.required=true;const body=document.createElement('textarea');body.rows=5;body.maxLength=4000;body.required=true;
    form.append(field(t('kind'),kind),field(t('title'),title),field(t('text'),body));const actions=el('div','dialog-actions');actions.append(button(t('cancel')));actions.firstChild.addEventListener('click',()=>dialog.close());const save=document.createElement('button');save.type='submit';save.className='primary';save.textContent=t('save');actions.append(save);form.append(actions);content.append(form);
    form.addEventListener('submit',event=>{event.preventDefault();try{const input={id:'',title:title.value.trim(),body:body.value.trim()},id=model.addDraft({kind:kind.value,title:input.title,body:input.body});input.id=`space:${profile}:${crypto.randomUUID()}`;bindHypothesis(input.id,id);saveOwnerBindings();const managed={input,ownerSynced:false};managedDrafts.set(id,managed);try{workspace?.addHypothesis?.(input);managed.ownerSynced=true;notifyGraphResult(t('saved'));}catch{notify(model.persistenceError?.()?t('graphSaveError'):t('saveError'));}personalState=workspace?.getState?.()??personalState;selected=id;selectedEdge=null;dialog.close();reflect();sky?.frame();}catch(error){notify(displayPublicError(error,locale));}});dialog.showModal();title.focus();
  }
  function field(label,input){const wrapper=el('label','ps-field');wrapper.append(el('span','',label),input);return wrapper;}
  function beginConnect(first){interaction='connect';connectIds=first?[first]:[];compareIds=[];closePanel();notify(connectIds.length?t('chooseSecond'):t('chooseFirst'));}
  function openRelationDialog(){
    dialog.replaceChildren();const from=graphState().nodes.find(item=>item.id===connectIds[0]),to=graphState().nodes.find(item=>item.id===connectIds[1]);const content=makeDialog(t('newEdge'));content.append(el('p','ps-dialog-intro',`${label(from?.ownerType==='personal-hypothesis'?from:matForNode(from)??from)} → ${label(to?.ownerType==='personal-hypothesis'?to:matForNode(to)??to)}`));const form=document.createElement('form'),kind=document.createElement('select');
    for(const key of relationKinds.filter(key=>key!=='contains'))kind.append(new Option(tr(RELATIONS[key],locale()),key));const reason=document.createElement('textarea');reason.rows=3;reason.maxLength=500;reason.required=true;form.append(field(t('chooseRelation'),kind),field(t('edgeLabel'),reason));const actions=el('div','dialog-actions');const cancel=button(t('cancel'));cancel.addEventListener('click',()=>{connectIds=[];dialog.close();});const save=document.createElement('button');save.type='submit';save.className='primary';save.textContent=t('save');actions.append(cancel,save);form.append(actions);content.append(form);
    form.addEventListener('submit',event=>{event.preventDefault();try{const edgeId=connectPersonalNodes(connectIds[0],connectIds[1],kind.value,reason.value.trim());connectIds=[];dialog.close();selectEdge(edgeId);notifyGraphResult(t('connected'));}catch(error){notify(displayPublicError(error,locale));}});dialog.showModal();reason.focus();
  }
  function beginCompare(first,second){comparisonRouteWasVisible=!!route&&!deck.hidden;deck.hidden=true;shell.dataset.journey='false';interaction='compare';connectIds=[];compareIds=[first,second].filter(Boolean);if(compareIds.length>=2){interaction=null;renderComparison();return;}closePanel();refreshSky();sky?.frame();notify(t('comparePick'));}
  function renderComparison(){
    if(compareIds.length<2)return;const values=compareIds.map(id=>graphState().nodes.find(node=>node.id===id));compareBar.replaceChildren();compareBar.hidden=false;
    const close=button('×','icon');close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>{compareIds=[];compareBar.hidden=true;if(comparisonRouteWasVisible)restoreRouteDock();comparisonRouteWasVisible=false;refreshSky();});compareBar.append(close,el('h2','',t('comparison')),el('p','',t('comparePrompt')));const columns=el('div','ps-compare-columns');
    for(const node of values){const material=matForNode(node),card=el('article','ps-compare-card');card.append(el('h3','',label(node.ownerType==='personal-hypothesis'?node:material??node)),el('small','posture',material?.demo?t('demo'):node.ownerType==='personal-hypothesis'?t('personal'):(material?.sourceAccess==='metadata-only'?t('linkOnly'):'')),el('p','body',tr(node.body??material?.atlasBody??material?.body,locale())));const source=sourceForMaterial(material,node);if(source){const open=button(source.exact?t('openExact'):t('openWork'),'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));card.append(open);}if(node.ownerType==='personal-hypothesis')appendPersonalGrounds(card,personalProposals(node.workspaceHypothesisId));columns.append(card);}
    compareBar.append(columns);shell.dataset.reading='false';reading.hidden=true;personal.hidden=true;sky?.refresh();
  }
  function toggleRelations(){
    dialog.replaceChildren();const content=makeDialog(t('filter'));for(const key of relationKinds){const value=RELATIONS[key],row=el('label','ps-relation-filter'),input=document.createElement('input');input.type='checkbox';input.checked=enabled.has(key);input.addEventListener('change',()=>{if(input.checked)enabled.add(key);else enabled.delete(key);reflect();});row.append(input,el('i',''));row.lastChild.style.background=value.color;row.append(el('span','',tr(value,locale())));content.append(row);}content.append(el('p','muted',t('imagine')));dialog.showModal();
  }
  function makeDialog(title){const head=el('div','dialog-top'),close=button('×','icon');close.addEventListener('click',()=>dialog.close());close.setAttribute('aria-label',t('close'));head.append(el('h2','',title),close);dialog.append(head);const content=el('div','dialog-content');dialog.append(content);return content;}
  function renderSearch(){
    if(!searchResults)return;searchResults.replaceChildren();const query=searchValue.toLocaleLowerCase();if(!query){searchResults.hidden=true;return;}
    const current=graphState(),available=[...previewMaterials.values()].filter(item=>{const author=entryByLegacyId.get(item.author);return [item.title,item.body,item.atlasBody,item.question,author?.title].some(value=>tr(value,locale()).toLocaleLowerCase().includes(query));}).slice(0,24);
    for(const item of available){const b=button('','ps-result');b.append(el('small','',kindName(item.kind)),el('strong','',label(item)),el('span','',tr(item.atlasBody??item.body,locale()).slice(0,140)));b.addEventListener('click',()=>{previewMaterial(item.id);});searchResults.append(b);}
    for(const item of current.nodes.filter(node=>node.ownerType==='personal-hypothesis'&&`${node.title} ${node.body}`.toLocaleLowerCase().includes(query))){const b=button('','ps-result');b.append(el('small','',t('personal')),el('strong','',item.title),el('span','',item.body.slice(0,140)));b.addEventListener('click',()=>{searchResults.hidden=true;search.blur();selectActual(item.id);});searchResults.append(b);}
    if(!searchResults.childElementCount)searchResults.append(el('p','ps-no-results',t('emptySearch')));searchResults.hidden=false;
  }
  function renderPersonalPanel(){
    if(personal.hidden)return;personalContent.querySelector('.ps-personal-summary')?.remove();const summary=el('p','ps-personal-summary',String(locale()).startsWith('en')?`Personal thoughts: ${personalState?.hypotheses?.length??0} · proposals: ${personalState?.proposals?.length??0}`:`Личных мыслей: ${personalState?.hypotheses?.length??0} · предложений: ${personalState?.proposals?.length??0}`);personalContent.prepend(summary);
  }
  function toggleFullscreen(){if(document.fullscreenElement)return document.exitFullscreen().catch(()=>{});return shell.requestFullscreen?.().catch(()=>{});}
  function keyboard(event){
    if(dialog.open||event.target.closest('input,textarea,select,[contenteditable="true"],.ps-personal'))return;
    const key=event.key.toLowerCase();if((event.ctrlKey||event.metaKey)&&key==='z'){event.preventDefault();event.shiftKey?model.redo():model.undo();checkGraphPersistence();}
    else if(!event.ctrlKey&&!event.metaKey){if(key==='o')sky?.frame();if(key==='/'){event.preventDefault();search.focus();}if(key==='escape'){interaction=null;connectIds=[];compareIds=[];compareBar.hidden=true;closePanel();closePersonal();closeRoute();}}
  }
  function restoreFocus(){
    const target=!deck.hidden?(deck.querySelector('[aria-current="true"]')??deck.querySelector('button,a[href]')):!reading.hidden?reading.querySelector('button,a[href]'):mainActions.querySelector('[data-space-action="routes"]');
    if(!target?.isConnected)return false;target.focus({preventScroll:true});return document.activeElement===target;
  }
  let collectionView=null,collectionRoot=null;
  function refreshLocale(){setLocaleState();if(!personal.hidden)renderPersonalPanel();if(dialog.open&&collectionRoot?.isConnected){dialog.querySelector('.dialog-top h2').textContent=t('library');for(const b of dialog.querySelectorAll('[data-public-locale]'))b.setAttribute('aria-pressed',String(b.dataset.publicLocale===locale()));collectionView?.refresh();}}
  function updateWorkspace(){personalState=workspace?.getState?.()??personalState;if(syncingOwner)return;reflect();if(!personal.hidden)renderPersonalPanel();}
  function syncManagedDrafts(next){
    const graphIds=new Set(next.nodes.map(node=>node.id)),ownerIds=new Set((workspace?.getState?.()?.hypotheses??[]).map(item=>item.id));let changed=false;syncingOwner=true;
    try{for(const [id,managed]of managedDrafts){if(!managed.ownerSynced)continue;if(graphIds.has(id)&&!ownerIds.has(managed.input.id)){workspace?.addHypothesis?.(managed.input);changed=true;}else if(!graphIds.has(id)&&ownerIds.has(managed.input.id)){workspace?.removeHypothesis?.(managed.input.id);changed=true;}}}
    catch{}finally{syncingOwner=false;}
    if(changed)personalState=workspace?.getState?.()??personalState;
  }
  const unsubscribeModel=model.subscribe(next=>{state=next;syncManagedDrafts(next);reflect();});
  const unsubscribeWorkspace=workspace?.subscribe?.(updateWorkspace);
  const showImportResult=()=>{if(personal.hidden)togglePersonal();};
  workspaceRoot.addEventListener('public-research-import-result',showImportResult);
  if(question)question.classList.add('ps-question-input');
  searchResults.hidden=true;deck.hidden=true;personal.hidden=true;reading.hidden=true;
  paintLenses();paintRelations();desktopMode.addEventListener('change',syncInitialRouteDock);syncInitialRouteDock();
  try{if(!model.getState().nodes.length)model.seedAtlas();}catch{}
  checkGraphPersistence();
  state=model.getState();
  sky=mountConstructorSky(shell,{onSelect:selectNode,onMove:(id,position)=>{const node=graphState().nodes.find(item=>item.id===id);if(node?.ownerType==='personal-hypothesis'){personalPositions.set(id,position);try{localStorage.setItem(positionsKey,JSON.stringify(Object.fromEntries(personalPositions)));positionsPersistenceError=false;}catch{positionsPersistenceError=true;notify(t('graphSaveError'));};}else try{model.moveNode(id,position);checkGraphPersistence();}catch{}},onEdgeSelect:selectEdge});
  const viewParams=new URLSearchParams(location.search),savedRoute=viewParams.get('treeRoute'),savedStep=Number(viewParams.get('treeStep'));
  const restoredRoute=routeItems().find(item=>`${item.source}:${item.id}`===savedRoute);
  if(restoredRoute){startRoute(restoredRoute);const step=Math.max(0,Math.min((restoredRoute.steps?.length??1)-1,Number.isSafeInteger(savedStep)?savedStep-1:0));if(restoredRoute.source==='atlas')goRouteStep(step);else goJourneyStep(step);}
  else if(savedRoute)persistView();
  reflect();sky.frame();shell.dataset.ready='true';
  if(upgradeError)notify(locale()==='en'?'The saved graph could not be upgraded. Its original copy is preserved.':'Не удалось перенести сохранённый граф. Исходная копия сохранена.');
  const localeEvent=event=>{if(event.detail?.locale&&event.detail.locale!==locale()){};refreshLocale();};header?.addEventListener('public-space-locale',localeEvent);
  return {root:shell,pristineGraphs,openCollection,personalRoot:personalContent,workspaceRoot,agentRoot,refreshLocale,restoreFocus,refreshJourneys(items){journeyList=Array.isArray(items)?items:[];if(route)renderRoutes();},renderJourneys:renderRoutes,model,
    destroy(){if(destroyed)return;destroyed=true;unsubscribeModel?.();unsubscribeWorkspace?.();workspaceRoot.removeEventListener('public-research-import-result',showImportResult);document.removeEventListener('keydown',keyboard);desktopMode.removeEventListener('change',syncInitialRouteDock);header?.removeEventListener('public-space-locale',localeEvent);sky?.dispose();model.dispose();shell.remove();}};
}
