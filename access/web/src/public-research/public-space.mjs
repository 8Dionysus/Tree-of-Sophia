import {PUBLIC_LOCALES,LOCALE_NAMES,resolveLocale,copyLocale,localeText,localeValue} from './locale.mjs';
import {mountConstructorSky} from '../../constructor/sky.mjs';
import {LENSES, RELATIONS, makeLensView, shortestPath} from '../../constructor/atlas-view.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {INQUIRY_FOUNDATION, LENS_INQUIRY} from '../../constructor/inquiry-foundation.mjs';
import {mountCollectionView} from './collection-view.mjs';
import {upgradePublicGraph} from './graph-transfer.mjs';
import {localizePublicJourney} from './journey-i18n.mjs';
import {displayPublicError,hypothesisTitle,proposalDisplay} from './presentation-i18n.mjs';
import {describeSourceLocator} from './source-address.mjs';
import {personalKind,readPersonalKinds} from './personal-kind.mjs';
import {isAuthoredTransition,ladderRung,nodeForm,partName,relationStyle,routeVoices,searchScore,sourceLine,stepWitness} from './space-legibility.mjs';
import {antiquityTimeline,formatDating,formatYears,stepTimeline,trajectory,zarathustraTimeline} from './space-timeline.mjs';
import {bonsaiFor,depthOf,lensPositions} from './bonsai.mjs';
import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {readPlaces} from '../../../../bonsai/lib/places.mjs';
import {clipLinesRight,planeOfPlaces} from './space-geography.mjs';
import {proximityCriteria,proximityLayout} from './space-proximity.mjs';
import {datingKind,measureAntiquity,measureZarathustra} from './space-measure.mjs';
import {depthLayout} from './space-depth.mjs';
import {mountHints,setHint} from './space-hints.mjs';
import {controlsDismissed,controlsShouldOpen,mountControls} from './space-controls.mjs';
import {besidePosition,disputes,ownTitle,readingsAround,restsOn,routeStars,routeWithRecords,threadsIntoDark} from './space-play.mjs';
import {citesOf,createFragments} from './fragments.mjs';
import '../../constructor/style.css';
import './public-space.css';

const el=(tag,cls='',value)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
const button=(value,cls='')=>{const node=el('button',cls,value);node.type='button';return node;};
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const translations={
  ru:{space:'Пространство исследования',profiles:'Профили',kickerZarathustra:'ЗАРАТУСТРА · ПУБЛИЧНЫЕ ИЗДАНИЯ',kickerAntiquity:'АНТИЧНЫЙ МИР · ПУБЛИЧНЫЕ ИЗДАНИЯ',search:'Искать тексты, понятия и вопросы…',lenses:'ЛИНЗЫ',tree:'Древо',read:'Читать',library:'Библиотека',routes:'Маршруты',grow:'Вырастить ветвь',research:'Моё исследование',new:'Новая мысль',connect:'Связать мысли',compare:'Сравнить',fit:'Вписать всё',undo:'Отменить',redo:'Вернуть',fullscreen:'На весь экран',filter:'Связи',all:'Все связи',sources:'Источники',readings:'Прочтения',question:'Вопрос исследования',searchResults:'Найдено',emptySearch:'Совпадений нет.',select:'Выберите звезду или линию, чтобы открыть её основание и точный источник.',selected:'Выбрано',kind:'Тип',posture:'Статус',demo:'Предложенное прочтение для исследования · не канон',personal:'Личная гипотеза · сохранена в общем исследовании',sourceExact:'Точная версия и место доступны в публичном чтении.',sourceWork:'Публичная запись произведения; точный фрагмент не закреплён.',sourceMissing:'Для этой ссылки полный текст здесь не представлен.',openExact:'Открыть точное место',openWork:'Открыть произведение',sourceRefs:'Указатели на источник',body:'Прочтение',questionLabel:'Вопрос',connections:'Связи',related:'Перейти к узлу',relation:'Тип связи',edgeBody:'Обоснование связи',readRoute:'Начать чтение',routeStart:'Маршрут исследования',routeNext:'Следующий шаг',routePrevious:'Предыдущий шаг',step:'Шаг',noRoutes:'Для этого профиля ещё нет маршрутов.',growTitle:'Ветви исследования',growIntro:'Добавьте подготовленную линию в личное пространство или начните собственную.',createBranch:'Создать свою мысль',save:'Сохранить',cancel:'Отмена',title:'Название',text:'Текст мысли',newNode:'Новая мысль',newEdge:'Связать две мысли',chooseFirst:'Выберите первую звезду, затем вторую.',chooseSecond:'Теперь выберите вторую звезду.',edgeLabel:'Коротко объясните, что связывает мысли',chooseRelation:'Вид связи',connected:'Связь добавлена.',saved:'Сохранено в личном исследовании и графе.',saveError:'Не удалось сохранить мысль в общем исследовании.',comparePick:'Выберите две звезды для сопоставления.',comparison:'Сопоставление',start:'Открыть',empty:'В этом пространстве пока нет узлов.',countNodes:'узлов',countEdges:'связей',record:'Записано в этом браузере',close:'Закрыть',linkOnly:'Только библиографическая ссылка',resetRoute:'Закрыть маршрут',imagine:'Линза задаёт способ чтения; она не меняет исходное Древо.',unavailable:'Источник не удалось открыть.',export:'Экспорт пространства',import:'Импорт пространства',source:'Источник',work:'Произведение',part:'Часть',fragment:'Фрагмент',chapter:'Глава',concept:'Понятие',interpretation:'Прочтение',question:'Вопрос',note:'Заметка',dossier:'Досье',figure:'Мыслитель',tradition:'Традиция',symbol:'Образ'},
  en:{space:'Research space',profiles:'Profiles',kickerZarathustra:'ZARATHUSTRA · PUBLIC EDITIONS',kickerAntiquity:'ANCIENT WORLDS · PUBLIC EDITIONS',search:'Search texts, concepts and questions…',lenses:'LENSES',tree:'The tree',read:'Read',library:'Library',routes:'Routes',grow:'Grow a branch',research:'My research',new:'New thought',connect:'Connect thoughts',compare:'Compare',fit:'Fit all',undo:'Undo',redo:'Redo',fullscreen:'Fullscreen',filter:'Relations',all:'All relations',sources:'Sources',readings:'Readings',question:'Research question',searchResults:'Results',emptySearch:'No matches.',select:'Select a star or line to open its reading and source.',selected:'Selected',kind:'Type',posture:'Status',demo:'Proposed reading for exploration · not canon',personal:'Personal hypothesis · saved in shared research',sourceExact:'The exact version and place are available in the public reader.',sourceWork:'Public work record; no exact passage is bound.',sourceMissing:'Full text for this reference is not supplied here.',openExact:'Open exact passage',openWork:'Open work',sourceRefs:'Source references',body:'Reading',questionLabel:'Question',connections:'Connections',related:'Open node',relation:'Relation type',edgeBody:'Why these thoughts are connected',readRoute:'Start reading',routeStart:'Research paths',routeNext:'Next step',routePrevious:'Previous step',step:'Step',noRoutes:'No routes are available for this profile yet.',growTitle:'Research branches',growIntro:'Add a prepared line to your space or begin your own.',createBranch:'Create your own thought',save:'Save',cancel:'Cancel',title:'Title',text:'Thought',newNode:'New thought',newEdge:'Connect two thoughts',chooseFirst:'Choose the first star, then the second.',chooseSecond:'Now choose the second star.',edgeLabel:'Briefly explain what connects these thoughts',chooseRelation:'Relation type',connected:'Connection added.',saved:'Saved in personal research and the graph.',saveError:'Could not save this thought in shared research.',comparePick:'Choose two stars to compare.',comparison:'Comparison',start:'Open',empty:'This space has no nodes yet.',countNodes:'nodes',countEdges:'relations',record:'Saved in this browser',close:'Close',linkOnly:'Bibliographic link only',resetRoute:'Close route',imagine:'A lens offers a way to read; it does not change the source tree.',unavailable:'The source could not be opened.',export:'Export space',import:'Import space',source:'Source',work:'Work',part:'Part',fragment:'Passage',chapter:'Chapter',concept:'Concept',interpretation:'Interpretation',question:'Question',note:'Note',dossier:'Dossier',figure:'Thinker',tradition:'Tradition',symbol:'Image'},
};
const tr=localeValue;
// A title's middle dot stays at the end of its line, never at the start of the next.
const keepDots=text=>String(text??'').replace(/ · /g,'\u00a0· ');
const extraTranslations={
  ru:{addToSpace:'Добавить в пространство',removeFromSpace:'Убрать из пространства',removedFromSpace:'Узел убран из пространства. Отмена восстановит его связи; текст остаётся в библиотеке.',previewOnly:'Просмотр материала · пространство не изменено',authorWorks:'Произведения автора',visibleCount:'В текущей выборке',recordScope:'Древо и личные мысли сохранены в этом браузере',routeScope:'Маршрут и шаг сохранены в адресе страницы',lensInquiry:'Как исследовать этот вопрос',lensMethod:'Способ чтения',lensLimit:'Что остаётся вне линзы',argument:'Ход мысли',counterReading:'Другое прочтение',readingQuestion:'Вопрос к этому чтению',experiment:'Попробовать при чтении',perspectives:'Детали чтения',example:'Проверить на примере',relationWarrant:'На чём держится переход',relationLimit:'Предел этого сопоставления',nextRelation:'Следующий переход',routeEnd:'К чему пришли',routeStarting:'С чем начинаем',routeStakes:'Почему это меняет ответ',routeCarry:'Что проверить дальше',practice:'Практика',chooseQuestion:'Выберите вопрос',graphSaveError:'Не удалось сохранить граф в этом браузере. Он пока доступен в открытой вкладке.',steps:'шагов',stepsOne:'шаг',stepsFew:'шага',graphUndo:'Отменить действие в пространстве',graphRedo:'Вернуть действие в пространстве',bibliographicOnly:'Только библиографическое указание',externalBibliographic:'Внешняя библиографическая ссылка · доступ к полному тексту не заявлен',openExternal:'Открыть внешнюю библиографическую ссылку',basedOn:'Основано на',groundsFor:'Основание для',endpointSource:'Источник узла',endpointRelatedSources:'Ссылки связанных предложений',personalGrounds:'Основания личного исследования',personalRelations:'Предложения и связи',personalObjection:'Возражение к предложению',openGround:'Открыть основание',readWithin:'Читать в составе',containedSource:'Текст доступен внутри существующего издания',sourceLoading:'Сведения об источнике…'},
  en:{addToSpace:'Add to space',removeFromSpace:'Remove from space',removedFromSpace:'Node removed from the space. Undo restores its relations; the text remains in the library.',previewOnly:'Material preview · your space has not changed',authorWorks:'Works by this author',visibleCount:'In current selection',recordScope:'Tree and personal thoughts saved in this browser',routeScope:'Route and step are kept in the page address',lensInquiry:'How to investigate this question',lensMethod:'A way of reading',lensLimit:'What the lens leaves outside',argument:'The argument',counterReading:'Another reading',readingQuestion:'A question for this reading',experiment:'Try while reading',perspectives:'Details to follow',example:'Try an example',relationWarrant:'What grounds this transition',relationLimit:'Limits of this comparison',nextRelation:'The next transition',routeEnd:'What we have reached',routeStarting:'Where we begin',routeStakes:'Why this changes the answer',routeCarry:'What to examine next',practice:'Practice',chooseQuestion:'Choose a question',graphSaveError:'Could not save the graph in this browser. It remains available in this tab.',steps:'steps',stepsOne:'step',stepsFew:'steps',graphUndo:'Undo a space action',graphRedo:'Redo a space action',bibliographicOnly:'Bibliographic reference only',externalBibliographic:'External bibliographic link · full-text access is not asserted',openExternal:'Open external bibliographic link',basedOn:'Based on',groundsFor:'Grounds',endpointSource:'Source of the endpoint',endpointRelatedSources:'Sources from related proposals',personalGrounds:'Grounds of this personal inquiry',personalRelations:'Proposals and relations',personalObjection:'Objection to proposal',openGround:'Open ground',readWithin:'Read within',containedSource:'This text is available within an existing edition',sourceLoading:'Source details…'},
};
Object.assign(extraTranslations.ru,{zoomIn:'Увеличить Древо',zoomOut:'Уменьшить Древо',mapSearch:'Искать в пространстве…',mapSearchHint:'Поиск по названиям и описаниям пространства; полный текст ищите в Чтении.',personal:'Личная запись',personalUnknown:'Вид записи не указан',pendingProposal:'Личное предложение · ожидает проверки · не канон',growEmpty:'Создайте собственную мысль в этом пространстве.',routeQuestion:'Вопрос маршрута',generalQuestion:'Общий вопрос',lensQuestion:'Вопрос этого взгляда',modeCancel:'Отменить выбор',modeFirst:'Выберите первую звезду',modeSecond:'Выберите вторую звезду',modeCompare:'Сопоставление',modeConnect:'Связь',stepName:'Шаг',voice:'Голос'});
Object.assign(extraTranslations.en,{zoomIn:'Zoom in on the tree',zoomOut:'Zoom out on the tree',mapSearch:'Search this space…',mapSearchHint:'Searches space titles and descriptions; search full text in the reader.',personal:'Personal record',personalUnknown:'Record type not specified',pendingProposal:'Personal proposal · pending review · not canon',growEmpty:'Create your own thought in this space.',routeQuestion:'Route question',generalQuestion:'General question',lensQuestion:'Question for this view',modeCancel:'Cancel selection',modeFirst:'Choose the first star',modeSecond:'Choose the second star',modeCompare:'Compare',modeConnect:'Connect',stepName:'Step',voice:'Voice'});
Object.assign(extraTranslations.ru,{excerpt:'Выдержка',proposalGrounds:'Основания этого предложения',manualOrigin:'Ручной ввод',agentOrigin:'Ответ внешнего агента',personalOrigin:'Личное предложение',pending:'Ожидает проверки',notCanon:'Не канон',postureShort:'Прочтения · не канон'});
Object.assign(extraTranslations.ru,{turn:'Повернуть пространство',turnRest:'Вернуть пространство к исходному повороту',turnRestNote:'Нажатие возвращает исходный поворот',controls:'Управление',transition:'Переход',branch:'Ветвь',journey:'Маршрут',chooseBranch:'Выберите ветвь или маршрут',walkBranch:'Пройти ветвь',openSection:'Открыть раздел издания',sectionSource:'Открывается раздел издания, где стоит это место; точная фраза не размечена.',moveOne:'ход',moveFew:'хода',moveMany:'ходов'});
Object.assign(extraTranslations.en,{turn:'Turn the space',turnRest:'Bring the space back to its resting turn',turnRestNote:'Pressing it brings back the resting turn',controls:'Controls',transition:'Link',branch:'Branch',journey:'Route',chooseBranch:'Choose a branch or a route',walkBranch:'Walk this branch',openSection:'Open the edition section',sectionSource:'Opens the edition section that contains this passage; the exact sentence is not marked.',moveOne:'move',moveFew:'moves',moveMany:'moves'});
Object.assign(extraTranslations.ru,{dating_medium:'опора — обзор исследований',dating_low:'опора — источник без ссылок на исследования',lifeLine:'Годы автора',journeys:'Маршруты чтения',researchActions:'Исследование',orientation:'Где вы в Древе',mapTools:'Управление картой',openColumn:'Панель',collapseColumn:'Свернуть панель',toOverview:'Ко всему пространству',backToRoute:'назад к маршруту',onMap:'На карте',selectedLink:'Выбрана связь',lensWord:'линза',statusTitle:'Статус',formSource:'Источник',formSourceHint:'Произведение, место, автор или традиция — то, на что опираются прочтения',formProposal:'Прочтение',formProposalHint:'Авторское прочтение, понятие, вопрос или ветвь — предложено для исследования, не канон',formPersonal:'Личное',formPersonalHint:'Ваша мысль или связь, сохранённая в этом браузере',
  ladderLabel:'Ступень толкования',ladderHint:'Древо различает источник, его прочтение и сопоставление прочтений; каждая ступень возвращается к источнику.',rung_source:'Источник',rung_reading:'Прочтение',rung_comparison:'Сопоставление',rungPersonal:'Личная запись',ownPlaceholder:'Как вы это читаете? Одной строкой',ownWrite:'Записать',ownMine:'Ваше прочтение',ownAround:'Прочтения рядом',ownNearest:'Ближе всего к вашему',ownNone:'Ни к одному',ownRestsOn:'Опирается на',ownDisputed:'С этим спорит',ownWritten:'Ваше прочтение встало в пространство',threadNote:'ещё не в пространстве',threadAdded:'Звезда вошла в пространство',
  basisContains:'Структура: часть и целое.',basisGround:'Прочтение опирается на это место источника.',basisEditorial:'Редакторское сопоставление двух прочтений · предложено, не канон · не утверждает исторического влияния.',basisProposed:'Связь предложена для исследования · не канон · сходство не означает влияния.',basisPersonal:'Личная связь из вашего исследования.'});
Object.assign(extraTranslations.en,{dating_medium:'rests on a summary of scholarship',dating_low:'rests on a source that cites no study',lifeLine:'Author’s years',journeys:'Reading routes',researchActions:'Research',orientation:'Where you are in the tree',mapTools:'Map controls',openColumn:'Panel',collapseColumn:'Collapse panel',toOverview:'To the whole space',backToRoute:'back to the route',onMap:'On the map',selectedLink:'Selected link',lensWord:'lens',statusTitle:'Status',formSource:'Source',formSourceHint:'A work, passage, author or tradition: what readings rest on',formProposal:'Reading',formProposalHint:'An authored reading, concept, question or branch: proposed for inquiry, not canon',formPersonal:'Personal',formPersonalHint:'Your own thought or link, saved in this browser',
  ladderLabel:'Step of interpretation',ladderHint:'The tree tells a source, its reading and a comparison of readings apart; each step returns to the source.',rung_source:'Source',rung_reading:'Reading',rung_comparison:'Comparison',rungPersonal:'Personal record',ownPlaceholder:'How do you read it? One line',ownWrite:'Write it down',ownMine:'Your reading',ownAround:'Readings nearby',ownNearest:'Nearest to yours',ownNone:'None of them',ownRestsOn:'Rests on',ownDisputed:'Disputed by',ownWritten:'Your reading has taken its place in the space',threadNote:'not yet in the space',threadAdded:'The star has entered the space',
  basisContains:'Structure: a part and its whole.',basisGround:'The reading rests on this place in the source.',basisEditorial:'An editorial comparison of two readings · proposed, not canon · not a claim of historical influence.',basisProposed:'A relation proposed for inquiry · not canon · similarity is not influence.',basisPersonal:'A personal link from your research.'});
Object.assign(extraTranslations.ru,{fragmentOriginal:'Оригинал',fragmentTranslation:'Перевод',fragmentWhole:'Читать в целом произведении',refBook:'Книга {n}',refChapter:'Глава {n}',refSection:'Раздел {n}',refFragment:'Фрагмент {n}',refLines:'Строки от {n}',refArrian:'Предисловие Арриана',fragmentQuoted:'Откуда взята цитата',fragmentBook:'Об издании книги',fragmentEdition:'Открыть в читателе',fragmentOmitted:'В этом издании перевода места нет',fragmentUnavailable:'Текст этого места сейчас недоступен',parallelPrevious:'Предыдущая глава',parallelNext:'Следующая глава'});
Object.assign(extraTranslations.en,{fragmentOriginal:'Original',fragmentTranslation:'Translation',fragmentWhole:'Read in the whole work',refBook:'Book {n}',refChapter:'Chapter {n}',refSection:'Section {n}',refFragment:'Fragment {n}',refLines:'Lines from {n}',refArrian:'Arrian’s preface',fragmentQuoted:'Where the quotation is taken from',fragmentBook:'About the book’s edition',fragmentEdition:'Open in the reader',fragmentOmitted:'This edition of the translation lacks the place',fragmentUnavailable:'The text of this place is unavailable now',parallelPrevious:'Previous chapter',parallelNext:'Next chapter'});
Object.assign(extraTranslations.en,{excerpt:'Excerpt',proposalGrounds:'Grounds of this proposal',manualOrigin:'Manual entry',agentOrigin:'External agent response',personalOrigin:'Personal proposal',pending:'Pending review',notCanon:'Not canon',postureShort:'Readings · not canon'});
// The measuring lenses and the measure panel speak in few words; the map shows the rest.
Object.assign(extraTranslations.ru,{"traceTitle": "Понятие во времени", "traceLink": "след понятия", "traceLabel": "Понятие", "nearTitle": "Близость по", "nearIsCenter": "центр", "nearClosest": "ближе всех", "nearFrom": "от", "nearNotMeasured": "не измеряется", "nearSharedConcepts": "общих понятий", "nearSharedGrounds": "общих оснований", "dateUnknown": "дата неизвестна", "duringLifeShort": "в пределах жизни автора", "duringActivityShort": "в годы деятельности автора", "placeUnknown": "место не установлено", "placeActivityShort": "здесь жил автор", "certainty_exact": "точно", "certainty_approximate": "приблизительно", "certainty_uncertain": "под вопросом", "certainty_unknown": "неизвестно", "certainty_none": "без датировки", "measureKind_date-assertion": "своя дата", "measureKind_interval-assertion": "свой интервал", "measureKind_relative-phase": "внутри периода", "measureKind_relative-life": "в пределах жизни автора", "measureKind_relative-activity": "в годы деятельности автора", "measureKind_relative-order": "относительно другого события", "measureKind_unknown-date": "дата неизвестна", "measureKind_none": "без датировки", "footing_high": "исследование", "footing_medium": "обзор исследований", "footing_low": "без ссылок на исследования", "placeBasis_composition": "место написания", "placeBasis_event": "место события", "placeBasis_activity": "место жизни автора", "placeBasis_unknown": "не установлено", "measureAction": "Мера и полнота", "measureDating": "Датировки", "measureCertainty": "Точность", "measureFooting": "Опора", "measurePlaces": "Места", "measureEras": "Эпохи", "measureConcepts": "Понятия", "measureBranches": "Ветви во времени", "measureLifeOnly": "Без собственной датировки", "measurePlaceless": "Без места", "measureGrounding": "Опора звёзд", "measureInBook": "главы книги", "measureOutside": "другие тексты", "measureChapters": "Главы", "measureOutsideTexts": "Тексты вне книги", "measureRoutes": "Маршруты по книге"});
Object.assign(extraTranslations.en,{"traceTitle": "A concept through time", "traceLink": "concept trace", "traceLabel": "Concept", "nearTitle": "Proximity by", "nearIsCenter": "centre", "nearClosest": "nearest", "nearFrom": "from", "nearNotMeasured": "not measured", "nearSharedConcepts": "shared concepts", "nearSharedGrounds": "shared grounds", "dateUnknown": "date unknown", "duringLifeShort": "within the author’s life", "duringActivityShort": "within the author’s years of activity", "placeUnknown": "place unknown", "placeActivityShort": "where the author lived", "certainty_exact": "exact", "certainty_approximate": "approximate", "certainty_uncertain": "uncertain", "certainty_unknown": "unknown", "certainty_none": "no dating", "measureKind_date-assertion": "a date of its own", "measureKind_interval-assertion": "an interval of its own", "measureKind_relative-phase": "within a phase", "measureKind_relative-life": "within the author’s life", "measureKind_relative-activity": "within the author’s years of activity", "measureKind_relative-order": "relative to another event", "measureKind_unknown-date": "date unknown", "measureKind_none": "no dating", "footing_high": "a study", "footing_medium": "a summary of scholarship", "footing_low": "no study cited", "placeBasis_composition": "place of writing", "placeBasis_event": "place of the event", "placeBasis_activity": "where the author lived", "placeBasis_unknown": "unknown", "measureAction": "Measure and completeness", "measureDating": "Datings", "measureCertainty": "Precision", "measureFooting": "Grounds", "measurePlaces": "Places", "measureEras": "Eras", "measureConcepts": "Concepts", "measureBranches": "Branches in time", "measureLifeOnly": "No dating of their own", "measurePlaceless": "No place", "measureGrounding": "What the stars rest on", "measureInBook": "chapters of the book", "measureOutside": "other texts", "measureChapters": "Chapters", "measureOutsideTexts": "Texts outside the book", "measureRoutes": "Routes through the book"});
const materialMap=library=>new Map((library?.nodes??[]).map(item=>[item.id,item]));
const graphIdForMaterial=id=>`material:${id}`;
const sourceKey=binding=>binding?[binding.documentId,binding.versionId,binding.unitId].join('\u0000'):'';
const copyPosition=value=>Array.isArray(value)&&value.length===3&&value.every(Number.isFinite)?value:null;
function downloadGraph(packet,name){const blob=new Blob([packet],{type:'application/json'}),url=URL.createObjectURL(blob),anchor=document.createElement('a');anchor.href=url;anchor.download=name;anchor.hidden=true;document.body.append(anchor);anchor.click();anchor.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);}

/** A public adapter over the existing constructor scene and its Rust graph owner. */
export function mountPublicResearchSpace({host,header,profile,locale=()=> 'ru',library,bindings=new Map(),documentBindings=new Map(),groundTargets=()=>[],routeGroundTargets=()=>null,graphPredecessors=[],api,workspace,shelf,question,saveState,status,personalOwnerRoot,agentOwnerRoot,journeys=[]}={}){
  if(!host||!library||!api)throw new TypeError('The public space needs a host, library, and reader API.');
  // Antiquity calls its authored routes branches, as the earlier tree did.
  const profileWords={antiquity:{ru:{routes:'Ветви',stepName:'Ход',backToRoute:'назад к ветви',routeNext:'Следующий ход',routePrevious:'Предыдущий ход'},en:{routes:'Branches',stepName:'Move',backToRoute:'back to the branch',routeNext:'Next move',routePrevious:'Previous move'}}}[profile]??{};
  const t=key=>{const lang=copyLocale(locale);return localeText(profileWords[lang]?.[key]??extraTranslations[lang][key]??translations[lang][key]??key,locale);};
  const quote=text=>{const language=resolveLocale(locale);return language==='de'?`„${text}“`:['ru','es'].includes(language)?`«${text}»`:`“${text}”`;};
  const openPublicAddress=address=>api.open(Object.fromEntries(Object.entries(address).filter(([key])=>['documentId','versionId','unitId','revision','reference'].includes(key))));
  // The work's own text in place of a retelling: cited places, opened in the whole work.
  const fragments=createFragments({profile,locale,t:key=>t(key),provider:api.provider,openAddress:openPublicAddress,hint:(element,text)=>setHint(element,text),onError:()=>notify(t('unavailable')),host:()=>shell});
  const materials=materialMap(library),atlas=library.atlas??{},routes=library.space?.routes??atlas.routes??[];
  const collectionMaterials=new Map((library.space?.collection??[]).map(item=>[item.materialId??item.id,{...item,id:item.materialId??item.id,kind:({author:'figure',school:'tradition'})[item.kind]??item.kind}]));
  const previewMaterials=new Map([...collectionMaterials,...materials]);
  const entryByLegacyId=new Map([...previewMaterials.values()].map(item=>[item.legacyId??item.id,item]));
  // Authors and works by their collection ids, for the witness line of a move.
  const collectionByLegacyId=new Map((library.space?.collection??[]).map(item=>[item.legacyId??item.id,item]));
  // The shared library's entries from another stand come after this stand's first view; the library and the search
  // take them in when they arrive, and the library waits for them when it is opened first.
  const moreCollection=typeof library.space?.loadCollection==='function'?library.space.loadCollection:null;
  let collectionReady=!moreCollection;
  function absorbCollection(){
    for(const [id,value]of bindings instanceof Map?bindings:Object.entries(bindings??{}))if(!sourceMap.has(id))sourceMap.set(id,value);
    for(const [id,value]of documentBindings instanceof Map?documentBindings:Object.entries(documentBindings??{}))if(!workMap.has(id))workMap.set(id,value);
    for(const item of library.space?.collection??[]){
      const id=item.materialId??item.id,legacy=item.legacyId??item.id;
      if(!collectionMaterials.has(id)){const value={...item,id,kind:({author:'figure',school:'tradition'})[item.kind]??item.kind};collectionMaterials.set(id,value);if(!previewMaterials.has(id))previewMaterials.set(id,value);}
      if(!entryByLegacyId.has(legacy))entryByLegacyId.set(legacy,previewMaterials.get(id));
      if(!collectionByLegacyId.has(legacy))collectionByLegacyId.set(legacy,item);
    }
  }
  const loadMoreCollection=()=>!moreCollection||collectionReady?Promise.resolve():moreCollection().then(()=>{if(destroyed)return;absorbCollection();collectionReady=true;if(searchValue)renderSearch();});
  // The stand's bonsai: its chronology, places, and how each of its lenses reads.
  const bonsai=bonsaiFor(profile);
  // The lens "Time" exists where the data dates or orders its material: Antiquity by the years its bonsai gives the writings, Zarathustra by the order of the book.
  const timeline=profile==='antiquity'?antiquityTimeline({routes,entries:library.space?.collection??[],chronology:bonsai?.chronology}):profile==='zarathustra'?zarathustraTimeline({materials:[...materials.values()],references:atlas.references??{},clusters:atlas.clusters??[],chronology:bonsai?.chronology}):null;
  // What a star's dating and place are about: the writing of its passage or work (Antiquity), of the text it first rests on (Zarathustra).
  const situationOf=materialId=>profile==='antiquity'?timeline?.timeOf?.(materialId)?.composition??null:bonsai?.chronology?.compositions?.references?.[timeline?.firstGround?.(materialId)]??null;
  const placesReader=bonsai?.places?readPlaces(bonsai.places):null;
  const placeOf=materialId=>{const situation=situationOf(materialId),own=situation?placesReader?.at(situation):null;return own?{...own,situation}:null;};
  const placeEntryOf=materialId=>{const situation=situationOf(materialId);return situation?placesReader?.entry(situation)??null:null;};
  const starIds=timeline?[...timeline.positions.keys()]:[];
  // The lens "Space" exists where the bonsai places the material; a star without a sourced place stands apart from the map.
  const geography=bonsai?.places?planeOfPlaces({stars:starIds,placeOf,places:bonsai.places.places}):null;
  // The lens "Proximity" measures from one star by one criterion its bonsai names; its direction groups by branch or circle.
  const nearGroups=profile==='antiquity'?routes.map(item=>({id:item.id,title:item.title})):(atlas.clusters??[]).map(cluster=>({id:cluster.id,title:cluster.title}));
  const nearGroupOf=new Map();
  if(profile==='antiquity')for(const item of routes)for(const step of item.steps??[]){const id=step.nodeId??step.graphNodeId;if(id&&!nearGroupOf.has(id))nearGroupOf.set(id,item.id);}
  else for(const material of materials.values())if(material.cluster)nearGroupOf.set(material.id,material.cluster);
  const starSet=new Set(starIds),nearEdges=(atlas.edges??[]).filter(edge=>starSet.has(edge.from)&&starSet.has(edge.to)&&edge.kind!=='contains').map(edge=>[edge.from,edge.to]);
  const passageOf=materialId=>collectionByLegacyId.get(String(materialId).replace(/^antiquity:reading:/,''));
  const distances=proximityCriteria({conceptsOf:id=>passageOf(id)?.concepts??null,groundsOf:id=>timeline?.groundsOf?.(id)??null,edges:nearEdges,spanOf:id=>timeline?.spanOf?.(id)??null,pointOf:id=>placeOf(id)?.point??null,chapterOf:id=>timeline?.placeOf?.(id)?.chapter??null});
  const criteria=(bonsai?.manifest?.lenses?.proximity?.criteria??[]).filter(item=>distances[item.id]&&(!item.requires||(item.requires==='places'?!!geography:item.requires==='chronology'?timeline?.kind==='years':true)));
  const timeLens=timeline?{id:'time',icon:'◷',title:{ru:'Время',en:'Time'},description:timeline.kind==='years'?{ru:'Годы написания; строки — ветви',en:'Years of writing; rows are branches'}:{ru:'Порядок книги, от Пролога до четвёртой части; строки — круги карты',en:'The order of the book, from the Prologue to Part Four; rows are the map’s circles'}}:null;
  const spaceLens=geography?{id:'space',icon:'⌖',title:{ru:'Пространство',en:'Space'},description:{ru:'Где написан текст или где жил его автор',en:'Where the text was written, or where its author lived'}}:null;
  const proximityLens=starIds.length&&criteria.length?{id:'proximity',icon:'⊙',title:{ru:'Близость',en:'Proximity'},description:{ru:'Расстояние от выбранной звезды',en:'Distance from the chosen star'}}:null;
  const lenses=[...(Array.isArray(library.space?.lenses)?library.space.lenses:LENSES),...[timeLens,spaceLens,proximityLens].filter(Boolean)].map(item=>({...item,positions:lensPositions(profile,item.id)}));
  // A measuring lens lays the stars out itself; the others keep the map's own arrangement.
  const measuredLens=id=>(id==='time'&&!!timeline)||(id==='space'&&!!geography)||(id==='proximity'&&!!proximityLens);
  const relationKinds=[...new Set([...(Array.isArray(library.space?.relationKinds)?library.space.relationKinds:Object.keys(RELATIONS)),'relates'])].filter(key=>RELATIONS[key]);
  // Relation words, colours and line patterns: the profile's own vocabulary where it has one.
  const styleOf=key=>relationStyle(profile,key,RELATIONS[key]??{});
  // Atlas edge ids may carry the 'atlas:' prefix once in the library and again in the graph copy.
  const bareEdgeId=id=>String(id??'').replace(/^(?:atlas:)+/,'');
  const atlasEdgeById=new Map((atlas.edges??[]).map(edge=>[bareEdgeId(edge.id),edge]));
  const atlasEdgeFor=id=>atlasEdgeById.get(bareEdgeId(id));
  const moveIds=new Set(routes.flatMap(item=>(item.steps??[]).map(step=>step.nodeId??step.graphNodeId)).filter(Boolean));
  const relationWord=key=>tr(styleOf(key),locale());
  // One word per kind of path: Antiquity walks branches by moves, every other space walks routes by steps.
  const routeWord=item=>t(profile==='antiquity'&&item?.source==='atlas'?'branch':'journey');
  const routeSize=(item,count)=>profile==='antiquity'&&item?.source==='atlas'?countMoves(count):countSteps(count);
  const kindName=kind=>t(({work:'work',part:'part',chapter:'chapter',fragment:'fragment',concept:'concept',interpretation:'interpretation',question:'question',note:'note',excerpt:'excerpt',thought:'personalUnknown',dossier:'dossier',figure:'figure',tradition:'tradition',symbol:'symbol'})[kind]??'source');
  const publicTitle=()=>profile==='zarathustra'?(tr(materials.get('demo:recurrence')?.title,locale())||tr(library.space?.title,locale())||t('space')):(tr(library.space?.title,locale())||t('space'));
  const countUnit=(value,kind)=>{const n=Math.abs(Number(value)||0);if((copyLocale(locale())==='en'))return localeText(kind==='nodes'?(n===1?'node':'nodes'):(n===1?'relation':'relations'),locale);const mod100=n%100,mod10=n%10;if(kind==='nodes')return mod100>=11&&mod100<=14?'узлов':mod10===1?'узел':mod10>=2&&mod10<=4?'узла':'узлов';return mod100>=11&&mod100<=14?'связей':mod10===1?'связь':mod10>=2&&mod10<=4?'связи':'связей';};
  const countText=(value,kind)=>`${value} ${countUnit(value,kind)}`;
  const countMoves=value=>{const n=Math.abs(Number(value)||0),mod100=n%100,mod10=n%10;return `${n} ${(copyLocale(locale())==='en')?(n===1?t('moveOne'):t('moveMany')):mod100>=11&&mod100<=14?t('moveMany'):mod10===1?t('moveOne'):mod10>=2&&mod10<=4?t('moveFew'):t('moveMany')}`;};
  const countSteps=value=>{const n=Number(value)||0;if((copyLocale(locale())==='en'))return `${n} ${n===1?t('stepsOne'):t('steps')}`;const mod100=Math.abs(n)%100,mod10=Math.abs(n)%10;return `${n} ${mod100>=11&&mod100<=14?t('steps'):mod10===1?t('stepsOne'):mod10>=2&&mod10<=4?t('stepsFew'):t('steps')}`;};
  const spaceKey=`tos.public-research.graph.v1.${profile}.${library.fingerprint}`;
  const baselineModel=createConstructorModel(library,{storage:null});
  let pristineGraphs;try{baselineModel.seedAtlas();pristineGraphs={[spaceKey]:baselineModel.exportPacket()};}finally{baselineModel.dispose();}
  let upgradeError=null;
  try{upgradePublicGraph(localStorage,{profile,fingerprint:library.fingerprint,predecessors:graphPredecessors,pristineGraph:pristineGraphs[spaceKey],validate:packet=>{const verifier=createConstructorModel(library,{storage:null});try{verifier.importPacket(packet);return verifier.exportPacket();}finally{verifier.dispose();}}});}catch(error){upgradeError=error;}
  const model=createConstructorModel(library,{key:spaceKey});
  // A link may name its lens, so a view on the scale of time can be shared as it is.
  const linkedLens=new URLSearchParams(location.search).get('treeLens'),defaultLens=lenses.some(item=>item.id==='tree')?'tree':lenses[0]?.id??'tree';
  let state=model.getState(),lens=lenses.some(item=>item.id===linkedLens)?linkedLens:defaultLens,enabled=new Set(relationKinds),selected=null,selectedEdge=null,searchValue='',route=null,routeIndex=0,compareIds=[],connectIds=[],interaction=null,panelMode='detail',personalState=workspace?.getState?.()??null,destroyed=false,routeDockTouched=false;
  // A measuring lens can be linked with what it measures by: the proximity criterion, the concept traced through time.
  const linkedCriterion=new URLSearchParams(location.search).get('treeCriterion'),linkedConcept=new URLSearchParams(location.search).get('treeConcept');
  let criterion=criteria.some(item=>item.id===linkedCriterion)?linkedCriterion:criteria[0]?.id??null,traceConcept=linkedConcept||null,measureOpen=false,nearCenter=null;
  const desktopMode=window.matchMedia('(min-width: 761px)');
  let browsingRoutes=false,columnOpen=true,routeStepNode=null;
  const routeDetailState=new Map();
  const personalPositions=new Map();
  const materializedHypotheses=new Map(),managedDrafts=new Map(),researchUndoneOwners=new Set();let syncingOwner=false;
  const positionsKey=`tos.public-research.graph-positions.v1.${profile}`;
  const ownersKey=`${spaceKey}.owners`;
  try{const saved=JSON.parse(localStorage.getItem(ownersKey)||'{}');if(object(saved))for(const [ownerId,graphId]of Object.entries(saved))if(typeof graphId==='string'&&state.nodes.some(node=>node.id===graphId))materializedHypotheses.set(ownerId,graphId);}catch{}
  let ownersPersistenceError=false,positionsPersistenceError=false;
  function bindHypothesis(ownerId,graphId){for(const [previousOwner,previousGraph]of materializedHypotheses)if(previousOwner!==ownerId&&previousGraph===graphId)materializedHypotheses.delete(previousOwner);materializedHypotheses.set(ownerId,graphId);}
  function saveOwnerBindings(){try{localStorage.setItem(ownersKey,JSON.stringify(Object.fromEntries(materializedHypotheses)));ownersPersistenceError=false;}catch{ownersPersistenceError=true;notify(t('graphSaveError'));}}
  try{const saved=JSON.parse(localStorage.getItem(positionsKey)||'{}');if(object(saved))for(const [key,value]of Object.entries(saved)){const p=copyPosition(value);if(p)personalPositions.set(key,p);}}catch{}
  const shell=el('section','pr-public-space');shell.id='tree';shell.dataset.profile=profile;shell.dataset.ready='false';shell.dataset.reading='false';shell.dataset.column='overview';shell.dataset.columnOpen='true';
  const canvas=document.createElement('canvas');canvas.className='tree-sky';canvas.setAttribute('aria-label',t('space'));
  if(header){header.classList.add('header','ps-header');shell.append(header);}
  shell.append(canvas);
  // One search for the whole space sits in the header row; '/' opens it.
  const searchBox=el('div','ps-search-box');
  const search=document.createElement('input');search.type='search';search.className='ps-search';search.autocomplete='off';search.placeholder=t('mapSearch');search.setAttribute('aria-label',t('mapSearch'));
  const searchKey=el('kbd','ps-search-key','/');searchKey.setAttribute('aria-hidden','true');
  const searchResults=el('div','ps-search-results');searchResults.hidden=true;
  searchBox.append(search,searchKey,searchResults);
  // The left rail holds the two ways into the tree: lenses say how to look, research actions what to do.
  const lensRail=el('nav','lens-rail ps-lens-rail');lensRail.setAttribute('aria-label',t('lenses'));
  const lensTitle=el('p','eyebrow ps-rail-title',t('lenses')),lensList=el('div','lens-list');
  const actionsTitle=el('p','eyebrow ps-rail-title',t('researchActions')),mainActions=el('div','ps-main-actions');mainActions.setAttribute('role','group');mainActions.setAttribute('aria-label',t('researchActions'));
  lensRail.append(lensTitle,lensList,actionsTitle,mainActions);
  for(const [key,icon,action] of [['routes','⌁',()=>toggleRoutes()],['lensInquiry','?',()=>showInquiry()],['grow','✣',()=>openGrow()],['research','◎',()=>togglePersonal()],['new','＋',()=>openNewThought()],['connect','↗',()=>beginConnect()],['compare','⇄',()=>beginCompare()],['measureAction','∑',()=>openMeasure()]]){const b=button('','ps-top-action');b.dataset.spaceAction=key;setHint(b,t(key));b.append(el('span','ps-action-icon',icon),el('span','ps-action-label',t(key)));b.addEventListener('click',action);mainActions.append(b);}
  // Orientation: where in the tree the reader stands, from the whole space down to the chosen star.
  const heading=el('header','view-heading ps-orientation');
  const kicker=el('p','view-kicker');
  const crumbRow=el('div','ps-crumb-row'),titleHeading=el('h1','ps-space-title'),titleButton=button('','ps-crumb ps-crumb-root');titleHeading.append(titleButton);
  const crumbNav=el('nav','ps-crumbs');crumbNav.setAttribute('aria-label',t('orientation'));const crumbList=el('ol','ps-crumb-list');crumbNav.append(crumbList);
  crumbRow.append(titleHeading,crumbNav);heading.append(kicker,crumbRow);
  const mapTools=el('div','ps-map-tools');mapTools.setAttribute('role','toolbar');mapTools.setAttribute('aria-label',t('mapTools'));
  const deck=el('section','ps-route-dock');deck.hidden=true;
  const relationBar=el('section','relation-bar ps-relations');
  const relationToggle=button(t('filter'),'relation-toggle');
  const relationChips=el('div','relation-chips'),statusLegend=el('div','ps-status-legend');relationBar.append(relationToggle,relationChips,statusLegend);
  const footer=el('footer','footer ps-footer');
  const footerTools=el('div','footer-tools');
  // The footer speaks only when something happened: a saved change, an error.
  const statusLabel=el('span','save-state');statusLabel.setAttribute('role','status');
  // The reading column holds the space's overview, an open branch or route, or the chosen star.
  // On a narrow screen it becomes a sheet whose top bar always stays in reach.
  const reading=el('aside','reading ps-panel ps-column');reading.hidden=true;
  const panelTop=el('div','reading-top'),panelTitle=button('','eyebrow ps-column-title'),panelClose=button('×','icon');panelClose.setAttribute('aria-label',t('close'));panelTop.append(panelTitle,panelClose);
  const panelContent=el('div','ps-panel-content');
  // A measuring lens keeps its own controls at the top of the column: the concept traced through time, the criterion of proximity.
  const lensControls=el('div','ps-lens-controls');lensControls.hidden=true;
  reading.append(panelTop,lensControls,deck,panelContent);
  const columnTab=button('','ps-column-tab');columnTab.hidden=true;columnTab.append(el('span','',`‹ ${t('openColumn')}`));
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
  const modeBanner=el('section','ps-pick-mode');modeBanner.hidden=true;modeBanner.setAttribute('aria-live','polite');
  const modeText=el('span'),modeCancel=button(t('modeCancel'),'text-button');modeCancel.addEventListener('click',()=>cancelInteraction());modeBanner.append(modeText,modeCancel);
  const dialog=document.createElement('dialog');dialog.className='ps-dialog';
  const sourceMap=new Map((bindings instanceof Map?bindings.entries():Object.entries(bindings??{})).map(([id,value])=>[id,value]));
  const workMap=new Map((documentBindings instanceof Map?documentBindings.entries():Object.entries(documentBindings??{})).map(([id,value])=>[id,value]));
  let journeyList=Array.isArray(journeys)?journeys:[];
  const importInput=document.createElement('input');importInput.type='file';importInput.accept='application/json,.json';importInput.hidden=true;importInput.setAttribute('aria-label',t('import'));
  shell.append(lensRail,heading,mapTools,relationBar,footer,reading,columnTab,personal,compareBar,modeBanner,dialog,importInput);
  if(header){const tools=header.querySelector('.main-tools, .pr-toolbar');header.insertBefore(searchBox,tools??null);}else shell.append(searchBox);
  host.append(shell);

  const langButton=localeButtonContainer();
  if(header){const toolbar=header.querySelector('.main-tools, .pr-toolbar');if(toolbar)toolbar.append(langButton);}
  function localeButtonContainer(){
    const group=el('div','language ps-language');
    const select=document.createElement('select');select.className='ps-locale-select';select.setAttribute('aria-label',localeText('Interface language',locale));for(const value of PUBLIC_LOCALES){const option=document.createElement('option');option.value=value;option.lang=value;option.textContent=LOCALE_NAMES[value];select.append(option);}select.value=resolveLocale(locale);select.addEventListener('change',()=>header?.dispatchEvent(new CustomEvent('public-space-locale',{detail:{locale:select.value}})));group.append(select);
    return group;
  }
  function setLocaleState(){
    const value=resolveLocale(locale);
    shell.dataset.locale=value;document.documentElement.lang=value;
    for(const select of document.querySelectorAll('.ps-locale-select')){select.value=value;select.setAttribute('aria-label',localeText('Interface language',locale));}
    search.placeholder=t('mapSearch');search.setAttribute('aria-label',t('mapSearch'));searchResults.setAttribute('aria-label',t('searchResults'));
    lensTitle.textContent=t('lenses');actionsTitle.textContent=t('researchActions');lensRail.setAttribute('aria-label',t('lenses'));mainActions.setAttribute('aria-label',t('researchActions'));importInput.setAttribute('aria-label',t('import'));
    crumbNav.setAttribute('aria-label',t('orientation'));mapTools.setAttribute('aria-label',t('mapTools'));columnTab.firstChild.textContent=`‹ ${t('openColumn')}`;
    for(const b of mainActions.querySelectorAll('[data-space-action]')){b.querySelector('.ps-action-label').textContent=t(b.dataset.spaceAction);setHint(b,t(b.dataset.spaceAction));}
    relationToggle.textContent=t('filter');relationToggle.setAttribute('aria-label',t('filter'));canvas.setAttribute('aria-label',t('space'));
    for(const item of uiActions)paintTool(item);
    clearTimeout(notify.timer);notify.timer=null;statusLabel.textContent=savedStatus();
    personalTop.querySelector('.eyebrow').textContent=t('research');
    panelClose.setAttribute('aria-label',t('close'));personalClose.setAttribute('aria-label',t('close'));
    controls?.refresh();hints?.hide();
    modeCancel.textContent=t('modeCancel');voiceCache.clear();paintLenses();paintRelations();if(searchValue)renderSearch();if(!compareBar.hidden)renderComparison();reflect();
  }
  const uiActions=[];
  let turnGlobe=null,lastPose=null;
  function globe(){
    const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg');svg.setAttribute('viewBox','-12 -12 24 24');svg.setAttribute('aria-hidden','true');
    const shape=(tag,attrs)=>{const node=document.createElementNS(ns,tag);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);svg.append(node);return node;};
    shape('circle',{r:'9',fill:'none',stroke:'currentColor','stroke-width':'1.2'});
    turnGlobe={meridian:shape('ellipse',{rx:'3',ry:'9',fill:'none',stroke:'currentColor','stroke-width':'.9',opacity:'.75'}),equator:shape('ellipse',{rx:'9',ry:'2',fill:'none',stroke:'currentColor','stroke-width':'.9',opacity:'.75'}),front:shape('circle',{r:'1.9',fill:'currentColor'})};
    paintTurn(lastPose);
    return svg;
  }
  // The controls tool is drawn as a mouse beside a finger: how the space is held.
  function controlsSign(){
    const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg');svg.setAttribute('viewBox','0 0 24 24');svg.setAttribute('aria-hidden','true');
    const add=(tag,attrs)=>{const node=document.createElementNS(ns,tag);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);svg.append(node);};
    const line={fill:'none',stroke:'currentColor','stroke-width':'1.3','stroke-linecap':'round','stroke-linejoin':'round'};
    add('rect',{x:'3.5',y:'5',width:'9',height:'14',rx:'4.5',...line});add('path',{d:'M8 5v5',...line});
    add('path',{d:'M16.5 19.5v-6.6a1.4 1.4 0 0 1 2.8 0v3.4l1.2.3a1.6 1.6 0 0 1 1.2 1.8l-.4 2.6',...line,opacity:'.8'});
    return svg;
  }
  // The globe shows the space's turn: its meridian and equator open as the space turns, its dot is the space's front.
  function paintTurn(pose){
    lastPose=pose;if(!turnGlobe||!pose)return;
    const yaw=pose.yaw,pitch=pose.pitch;
    turnGlobe.meridian.setAttribute('rx',Math.max(.7,Math.abs(Math.sin(yaw))*9).toFixed(2));
    turnGlobe.equator.setAttribute('ry',Math.max(.7,Math.abs(Math.sin(pitch))*9).toFixed(2));
    turnGlobe.front.setAttribute('cx',(Math.sin(yaw)*7.6).toFixed(2));turnGlobe.front.setAttribute('cy',(-Math.sin(pitch)*7.6).toFixed(2));
  }
  function paintTool({button:b,key,label}){
    // The globe is named by what pressing it does; its title says how the space turns.
    if(key==='turn'){b.replaceChildren(globe());setHint(b,t(key),['←','↑','→','↓'],{note:t('turnRestNote')});b.setAttribute('aria-label',t('turnRest'));return;}
    if(key==='controls'){b.replaceChildren(controlsSign());setHint(b,t(key));b.setAttribute('aria-label',t(key));return;}
    b.replaceChildren(el('span','ps-tool-symbol',label));
    if(key==='routes'||key==='research')b.append(el('span','ps-tool-label',t(key)));
    // A tool names its key; the overview also says how the map is moved.
    const keyHint={zoomIn:'+',zoomOut:'−',fit:'O'}[key];
    setHint(b,t(key),keyHint);b.setAttribute('aria-label',t(key));
  }
  function addTool(key,label,action,cls='',target=footerTools){
    const b=button('',cls),item={button:b,key,label};b.dataset.tool=key;paintTool(item);b.addEventListener('click',action);target.append(b);uiActions.push(item);return b;
  }
  // The footer carries the narrow screen's research actions; a wide screen keeps them, labelled, in the rail.
  addTool('lensInquiry','?',()=>showInquiry(),'icon');
  addTool('routes','⌁',()=>toggleRoutes(),'icon');
  addTool('grow','✣',()=>openGrow(),'icon');
  addTool('research','◎',()=>togglePersonal(),'icon');
  addTool('new','＋',()=>openNewThought(),'icon');
  addTool('connect','↗',()=>beginConnect(),'icon');
  addTool('compare','⇄',()=>beginCompare(),'icon');
  // Moving the map is its own small group at the map's corner; the globe turns the space and brings it back to rest.
  // The click that ends a drag of the globe belongs to the drag; a new press starts clean, and the keyboard always brings the space back.
  let turnDragged=false;
  const turnButton=addTool('turn','',event=>{if(event.detail!==0&&turnDragged){turnDragged=false;return;}sky?.turnToRest?.();},'icon ps-globe',mapTools);
  {
    let turning=null;
    turnButton.addEventListener('pointerdown',event=>{turnDragged=false;if(event.button!==0)return;turning={id:event.pointerId,x:event.clientX,y:event.clientY,last:[event.clientX,event.clientY],moved:false};turnButton.setPointerCapture(event.pointerId);});
    turnButton.addEventListener('pointermove',event=>{
      if(!turning||event.pointerId!==turning.id)return;
      if(!turning.moved&&Math.hypot(event.clientX-turning.x,event.clientY-turning.y)<3)return;
      turning.moved=true;const [x,y]=turning.last;turning.last=[event.clientX,event.clientY];sky?.turnBy?.((event.clientX-x)*.008,(event.clientY-y)*.008);
    });
    const done=event=>{if(!turning||event.pointerId!==turning.id)return;turnDragged=turning.moved&&event.type==='pointerup';turning=null;};
    turnButton.addEventListener('pointerup',done);turnButton.addEventListener('pointercancel',done);
  }
  addTool('zoomOut','−',()=>sky?.zoomBy(1/1.25),'icon',mapTools);
  addTool('zoomIn','+',()=>sky?.zoomBy(1.25),'icon',mapTools);
  addTool('fit','⌖',()=>frameSky(),'icon',mapTools);
  addTool('fullscreen','⛶',()=>void toggleFullscreen(),'icon',mapTools);
  // The controls window comes back from here, after it has closed for good.
  let controls=null,hints=null;
  const controlsButton=addTool('controls','',()=>{if(controls?.open)controls.close();else{controls?.show({focus:true});placeControls();}},'icon ps-controls-tool',mapTools);
  footerTools.append(el('i'));
  const undoButton=addTool('graphUndo','↶',()=>{model.undo();checkGraphPersistence();},'icon');
  const redoButton=addTool('graphRedo','↷',()=>{model.redo();checkGraphPersistence();},'icon');
  addTool('export','⇩',()=>{try{downloadGraph(model.exportPacket(),`tos-${profile}-research-tree.json`);}catch(error){notify(displayPublicError(error,locale));}},'icon');
  addTool('import','⇧',()=>importInput.click(),'icon');
  importInput.addEventListener('change',async()=>{const file=importInput.files?.[0];if(!file)return;try{if(file.size>1_000_000)throw new Error('constructor packet exceeds the 1 MB storage limit');model.importPacket(await file.text());selected=null;selectedEdge=null;notifyGraphResult(t('saved'));frameSky();}catch(error){notify(displayPublicError(error,locale));}finally{importInput.value='';}});
  relationToggle.addEventListener('click',()=>toggleRelations());
  footer.append(statusLabel,footerTools);
  titleButton.addEventListener('click',()=>goOverview());
  panelClose.addEventListener('click',()=>closeColumnLayer());personalClose.addEventListener('click',closePersonal);
  panelTitle.addEventListener('click',()=>toggleSheet());columnTab.addEventListener('click',()=>openColumn());
  search.addEventListener('input',()=>{searchValue=search.value.trim();renderSearch();});
  search.addEventListener('keydown',event=>{if(event.key==='Escape'){search.value='';searchValue='';searchResults.hidden=true;}if(event.key==='Enter')searchResults.querySelector('button')?.click();});
  document.addEventListener('keydown',keyboard);
  // With nothing focused, arrows turn the space while the map is what was last pressed; after a press elsewhere they are the page's again.
  let mapActive=true;
  const notePress=event=>{mapActive=!!event.target?.closest?.('#tree canvas.tree-sky,#tree .tree-stars,#tree .edge-labels,#tree .tree-clusters,#tree .ps-map-tools');};
  document.addEventListener('pointerdown',notePress,{capture:true});

  function savedStatus(){return model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError?t('graphSaveError'):'';}
  function notify(message){statusLabel.textContent=message;clearTimeout(notify.timer);notify.timer=setTimeout(()=>{notify.timer=null;statusLabel.textContent=savedStatus();},3800);}
  function persistView(){
    const url=new URL(location.href);
    if(route){url.searchParams.set('treeRoute',`${route.source}:${route.id}`);url.searchParams.set('treeStep',String(routeIndex+1));}
    else{url.searchParams.delete('treeRoute');url.searchParams.delete('treeStep');}
    if(lens!==defaultLens)url.searchParams.set('treeLens',lens);else url.searchParams.delete('treeLens');
    if(lens==='proximity'&&criterion&&criterion!==criteria[0]?.id)url.searchParams.set('treeCriterion',criterion);else url.searchParams.delete('treeCriterion');
    if(lens==='time'&&traceConcept)url.searchParams.set('treeConcept',traceConcept);else url.searchParams.delete('treeConcept');
    history.replaceState(history.state,'',url);
    if(!notify.timer)statusLabel.textContent=savedStatus();
  }
  function checkGraphPersistence(){if(model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError)notify(t('graphSaveError'));}
  function notifyGraphResult(successMessage){if(model.persistenceError?.()||ownersPersistenceError||positionsPersistenceError)notify(t('graphSaveError'));else notify(successMessage);}
  function humanEdgeLabel(edge){const value=tr(edge?.label,locale()).trim();if(!value||/^[a-z0-9][a-z0-9_.-]*(?::[a-z0-9][a-z0-9_.-]*)+$/i.test(value))return '';return value;}
  function label(item){return tr(item?.title,locale())||item?.title||item?.id||'';}
  function preferredGroundTarget(ref,contextId){
    const targets=groundTargets(ref,contextId),language=['ru','de'].includes(resolveLocale(locale))?'de':'en';
    return targets.find(target=>target.language===language&&target.scope==='passage')??targets.find(target=>target.language===language)??targets[0]??null;
  }
  function appendGrounds(parent,grounds,contextId,heading=null){
    if(!grounds?.length)return false;
    const section=el('section','ps-grounds');section.append(heading??el('h2','minor-title',t('sourceRefs')));
    for(const ground of grounds){
      const source=atlas.references?.[ground.ref],target=preferredGroundTarget(ground.ref,contextId),binding=sourceForRef(ground.ref)??sourceMap.get(ground.ref);
      const title=tr(target?.title,locale())||[tr(source?.author,locale()),tr(source?.work,locale()),tr(source?.locator,locale())].filter(Boolean).join(' · ')||ground.ref;
      const detail=el('details','textual-ground');detail.append(el('summary','',title));
      for(const cite of citesOf(ground))detail.append(fragments.fragment(cite));
      if(ground.focus)detail.append(el('p','ground-focus',tr(ground.focus,locale())));
      const address=citesOf(ground).length?null:target?.address??binding;
      if(address){const open=button(`↗ ${t(address.unitId?'openExact':'openWork')}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(address).catch(()=>notify(t('unavailable'))));detail.append(open);}
      else if(!citesOf(ground).length&&typeof source?.url==='string'&&/^https?:\/\//.test(source.url)){const link=el('a','source-link',`${title} ↗`);link.href=source.url;link.target='_blank';link.rel='noopener noreferrer';detail.append(link);}
      section.append(detail);
    }
    parent.append(section);return true;
  }
  function appendSourcePointers(parent,material){return appendGrounds(parent,material?.inquiry?.grounds,material?.id);}
  function inquiryDetail(parent,title,value,open=false){if(!value||!fragments.hasAuthored(value))return null;const detail=el('details','inquiry-detail');detail.open=open;detail.append(el('summary','',title));fragments.render(detail,value,{className:''});parent.append(detail);return detail;}
  function inquiryQuestion(parent,value){
    if(!value||!fragments.hasAuthored(value))return;
    const card=el('section','ps-question-card'),own=tr(value,locale()).trim();
    // A question asked by the work is a quotation, not our own research prompt.
    if(own)card.append(el('span','eyebrow',t('readingQuestion')),el('p','',own));
    for(const cite of citesOf(value))card.append(fragments.fragment(cite));
    parent.append(card);
  }
  function showInquiry(){
    const current=route?.source==='atlas'?routes.find(item=>item.id===route.id):null;
    const journey=route?.source==='manifest'?localizePublicJourney(journeyList.find(item=>item.id===route.id),locale()):null;
    const foundation=library.space?.inquiryFoundation??library.inquiryFoundation??(profile==='zarathustra'?INQUIRY_FOUNDATION:null);
    const inquiry=current?.investigation??library.space?.lensInquiry?.[lens]??library.lensInquiry?.[lens]??(profile==='zarathustra'?LENS_INQUIRY[lens]:null);
    dialog.replaceChildren();const content=makeDialog(t('lensInquiry'));
    const routeQuestion=tr(current?.question??journey?.question,locale());
    const lensQuestion=tr(inquiry?.question,locale());const generalQuestion=tr(foundation?.question,locale());
    if(routeQuestion)content.append(el('h3','minor-title',t('routeQuestion')),el('p','inquiry-leading',routeQuestion));
    if(lensQuestion&&lensQuestion!==routeQuestion)content.append(el('h3','minor-title',t('lensQuestion')),el('p','inquiry-leading',lensQuestion));
    if(generalQuestion&&generalQuestion!==routeQuestion&&generalQuestion!==lensQuestion)content.append(el('h3','minor-title',t('generalQuestion')),el('p','inquiry-leading',generalQuestion));
    if(!routeQuestion&&!lensQuestion&&!generalQuestion){const description=tr(lenses.find(item=>item.id===lens)?.description,locale());if(description)content.append(el('p','inquiry-leading',description));}
    if(foundation?.orientation)fragments.render(content,foundation.orientation,{className:''});
    for(const [field,key] of [['method','lensMethod'],['blindSpot','lensLimit'],['startingPoint','routeStarting'],['stakes','routeStakes'],['carryForward','routeCarry']])if(inquiry?.[field]&&fragments.hasAuthored(inquiry[field])){content.append(el('h3','minor-title',t(key)));fragments.render(content,inquiry[field],{className:''});}
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
    const target=profile==='zarathustra'?routeGroundTargets(route?.id,routeIndex,['ru','de'].includes(resolveLocale(locale))?'de':'en'):null;
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
  // A section entry opens the edition section that contains the passage; say so instead of 'open the work'.
  const sectionEntry=source=>!source?.exact&&!!source?.binding?.unitId;
  function sourceActionLabel(source){return source.containerTitle?`${t('readWithin')} ${tr(source.containerTitle,locale())} · ${tr(source.containerLocator,locale())}`:t(source.exact?'openExact':sectionEntry(source)?'openSection':'openWork');}
  function isBasedOn(edge){return String(edge.id).startsWith('atlas:antiquity:reading-source:')&&edge.kind==='supports'&&String(edge.from).includes('antiquity:reading:')&&String(edge.to).includes('antiquity:source:');}
  function relationName(edge,inverse=false){if(edge.proposalId)return tr(edge.label,locale());return isBasedOn(edge)?t(inverse?'groundsFor':'basedOn'):relationWord(edge.kind)||t('relation');}
  function personalProposals(ownerId){
    if(!ownerId)return [];
    return (workspace?.getState?.()?.proposals??personalState?.proposals??[]).filter(item=>item.parentHypothesisId===ownerId||item.targetId===ownerId||item.fromId===ownerId||item.toId===ownerId);
  }
  function appendPersonalGrounds(parent,proposals,heading='personalGrounds'){
    const refs=[...new Set(proposals.flatMap(item=>[...(item.sourceRefs??[]),...(item.evidenceRefs??[])]))];
    if(!refs.length)return;
    const section=el('section','ps-personal-grounds');section.append(el('h2','minor-title',t(heading)));parent.append(section);
    for(const locator of refs){
      const open=button(t('sourceLoading'),'text-button');open.disabled=true;section.append(open);
      void describeSourceLocator(locator,shelf,api.provider,locale()).then(result=>{
        if(!section.isConnected)return;
        open.textContent=`↗ ${t('openGround')} · ${result.label}`;open.disabled=result.status!=='exact';
        if(result.reference)open.addEventListener('click',()=>void openPublicAddress({documentId:result.reference.target.workId,versionId:result.reference.versionId,unitId:result.reference.unitId,revision:result.reference.target.textLayerSha256,reference:result.reference}).catch(()=>notify(t('unavailable'))));
      }).catch(()=>{if(section.isConnected)open.textContent=t('sourceMissing');});
    }
  }
  function proposalStatus(proposal){
    const parts=[t(proposal.actorOrigin==='agent'?'agentOrigin':proposal.actorOrigin==='human'?'manualOrigin':'personalOrigin')];
    if(proposal.reviewStatus==='pending_review')parts.push(t('pending'));
    return parts.join(' · ');
  }
  function appendPersonalProposals(parent,proposals){
    if(!proposals.length)return;
    const list=el('section','ps-personal-proposals'),articles=new Map();list.append(el('h2','minor-title',t('personalRelations')));
    for(const proposal of proposals){
      const article=el('article','ps-personal-proposal');articles.set(proposal.id,article);
      article.append(el('p','posture',proposalStatus(proposal)),el('p','body',proposalDisplay(proposal,locale())));
      const target=proposals.find(item=>item.id===proposal.targetId);
      if(target){const link=button(`${t('personalObjection')}: ${proposalDisplay(target,locale()).slice(0,110)}`,'text-button');link.addEventListener('click',()=>articles.get(target.id)?.scrollIntoView({block:'nearest'}));article.append(link);}
      appendPersonalGrounds(article,[proposal],'proposalGrounds');list.append(article);
    }
    parent.append(list);
  }
  function matForNode(node){return node?.materialId?previewMaterials.get(node.materialId):null;}
  function liveWorkspaceNodes(){
    const ownerState=workspace?.getState?.()??personalState;
    personalState=ownerState;
    const stored=new Map(state.nodes.map(node=>[node.id,node]));
    const boundKinds=readPersonalKinds(localStorage);
    const byOwnerId=new Map((ownerState?.hypotheses??[]).map(item=>[item.id,item]));
    const graphId=ownerId=>{
      const mapped=materializedHypotheses.get(ownerId);
      return mapped&&stored.has(mapped)?mapped:stored.has(ownerId)?ownerId:`personal:${ownerId}`;
    };
    const materialNodes=[];
    for(const hypothesis of byOwnerId.values()){
      const id=graphId(hypothesis.id),saved=stored.get(id);
      const position=personalPositions.get(id)??personalPositions.get(`personal:${hypothesis.id}`)??saved?.position??[((materialNodes.length%5)-2)*130,240+Math.floor(materialNodes.length/5)*115,25];
      materialNodes.push({...saved,id,kind:personalKind(hypothesis,boundKinds,saved?.kind),title:hypothesisTitle(hypothesis,locale()),rawTitle:hypothesis.title,body:hypothesis.body,sourceIds:saved?.sourceIds??[],position,localOnly:true,workspaceHypothesisId:hypothesis.id,ownerType:'personal-hypothesis',major:true,color:'#b3d9c4'});
    }
    const extraEdges=[];
    const resolve=id=>!id?null:byOwnerId.has(id)?graphId(id):state.nodes.find(node=>node.id===id||node.materialId===id)?.id;
    for(const hypothesis of byOwnerId.values()){
      const id=graphId(hypothesis.id),target=resolve(hypothesis.targetId);
      if(target&&target!==id)extraEdges.push({id:`personal-edge:${hypothesis.id}`,from:id,to:target,kind:'relates',label:copyLocale(locale)==='en'?localeText('Personal link',locale):'Личная связь',body:hypothesis.body,color:RELATIONS.relates.color,ownerType:'personal-hypothesis'});
      const from=resolve(hypothesis.fromId),to=resolve(hypothesis.toId);
      if(from&&to&&from!==to)extraEdges.push({id:`personal-relation:${hypothesis.id}`,from,to,kind:'relates',label:hypothesis.predicateLabel||(copyLocale(locale)==='en'?localeText('Personal relation',locale):'Личная связь'),body:hypothesis.body,color:RELATIONS.relates.color,ownerType:'personal-hypothesis'});
    }
    // An own reading set beside a reading stands linked to it, in the reader's own view only.
    for(const record of ownReadings.values()){
      if(!record.near||!byOwnerId.has(record.id))continue;
      const from=graphId(record.id),to=resolve(record.near.startsWith('material:')?record.near.slice(9):record.near);
      if(to&&to!==from)extraEdges.push({id:`personal-near:${record.id}`,from,to,kind:'relates',label:t('ownNearest'),color:RELATIONS.relates.color,ownerType:'personal-hypothesis'});
    }
    // These are projections of existing proposal relations, never new claims
    // written into the user's graph or admitted to the prepared library.
    for(const proposal of ownerState?.proposals??[]){
      if(proposal.kind!=='relation')continue;
      const from=resolve(proposal.fromId),to=resolve(proposal.toId);
      if(!from||!to||from===to)continue;
      const revision=proposal.id.startsWith('revision-link');
      const objection=proposal.id.startsWith('objection-link'),kind='relates';
      extraEdges.push({id:`personal-proposal:${proposal.id}`,from,to,kind,label:copyLocale(locale)==='en'?(revision?'Receives a reply or revision':objection?'Has an objection':'Personal relation'):(revision?'Получает ответ или пересмотр':objection?'Имеет возражение':'Личная связь'),body:proposalDisplay(proposal,locale()),proposalId:proposal.id,color:RELATIONS[kind].color,ownerType:'personal-hypothesis'});
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
        const graphId=candidate.addDraft({kind:node.kind,title:node.rawTitle??node.title,body:node.body,position:node.position});
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
    // An open branch and the scale of time both read prepared material the saved graph may not hold yet.
    const context=[];
    if(route?.source==='atlas')for(const step of routes.find(item=>item.id===route.id)?.steps??[])context.push(step.nodeId??step.graphNodeId);
    if(measuredLens(lens))context.push(...starIds);
    if(context.length){
      for(const materialId of context){const material=materials.get(materialId),id=graphIdForMaterial(materialId);if(!material||byId.has(id))continue;
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
    // A trace's lines exist only in the view that draws them.
    traceLinks.clear();
    const current=graphState();
    // A route keeps showing the reader's own records about its stars, including one just written on a step.
    const measuredView=measuredLens(lens),routePath=interaction||!compareBar.hidden?null:routeWithRecords(route?.path??null,current);
    const projected=makeLensView(current,library,{lens:measuredView?'tree':lens,enabled:enabled.size===relationKinds.length?null:enabled,path:routePath});
    const clusterNames=profile==='antiquity'?{routes:{title:{ru:'Ветви',en:'Branches'},subtitle:{ru:'Вопросы, с которых начинается исследование',en:'Questions where an inquiry begins'}}}:{};
    projected.clusters=(projected.clusters??[]).map(cluster=>({...cluster,title:tr(clusterNames[cluster.id]?.title??cluster.title,locale()),subtitle:tr(clusterNames[cluster.id]?.subtitle??cluster.subtitle,locale())}));
    projected.edges=projected.edges.map(edge=>{
      const next={...edge,label:isBasedOn(edge)?relationName(edge):humanEdgeLabel(edge)||humanEdgeLabel(atlasEdgeFor(edge.id))||relationName(edge)};
      const style=styleOf(edge.kind),word=relationWord(edge.kind);
      if(profile!=='antiquity'){
        // Every authored link names its kind on the map; its line and weight follow the kind's family.
        const tagged=!edge.ownerType&&edge.kind!=='contains'&&!!word&&next.label!==word;
        return {...next,dash:style.dash,weight:style.weight??1,...(tagged?{tag:word}:{})};
      }
      // Antiquity's authored transitions carry their kind as a word, a line pattern and a weight.
      const authored=isAuthoredTransition(edge);
      return {...next,color:edge.ownerType?edge.color:style.color??edge.color,dash:style.dash,weight:authored?style.weight:1,...(authored?{tag:word,baseAlpha:.42}:{})};
    });
    const routeIds=new Set(route?.path?.nodeIds??[]),selectedIds=new Set([selected].filter(Boolean));
    projected.nodes=projected.nodes.map(node=>{
      const material=matForNode(node);
      if(node.ownerType==='personal-hypothesis')return {...node,title:node.title,body:node.body,form:'personal'};
      const next={...node,title:label(material)??node.title,color:node.color??material?.color,form:nodeForm({material,node})};
      // A move names its witness; in the crowded overview only the chosen move does.
      const line=profile==='antiquity'&&partName(profile,material,locale(),{moveIds})&&(routeIds.has(node.id)||selectedIds.has(node.id))?sourceLine(material,{materials:previewMaterials,entryByLegacyId:collectionByLegacyId,locale:locale()}):'';
      return line?{...next,subtitle:line,labelLines:2,labelMaxWidth:210,accessibilityLabel:`${label(material)} · ${line}`}:next;
    });
    if(lens==='time'&&timeline)applyTimeline(projected,routePath);
    else if(lens==='space'&&geography)applyPlane(projected);
    else if(lens==='proximity'&&proximityLens)applyProximity(projected);
    else applyDepth(projected);
    // A new record rises out of what it is about, and a star a thread brings in out of the star it was followed from.
    if(rising.size)projected.nodes=projected.nodes.map(node=>rising.has(node.id)?{...node,from:rising.get(node.id)}:node);
    if(selectedEdge)projected.selectedEdgeId=selectedEdge;
    return projected;
  }
  // A space that groups is read in depth as its bonsai declares, by the step of interpretation; see space-depth.mjs.
  const depth=depthOf(profile);
  function applyDepth(projected){
    const laid=depthLayout(projected.nodes,{stepOf:node=>ladderRung({material:matForNode(node),node}),depth,locale:locale()});
    projected.nodes=laid.nodes;projected.planes=laid.planes;
  }
  // ── The play of knowing: nothing is scored and nothing hidden; what grows is the reader's own understanding. ──
  const rising=new Map();
  const playEdges=()=>{
    const out=(atlas.edges??[]).map(edge=>({from:graphIdForMaterial(edge.from),to:graphIdForMaterial(edge.to),kind:edge.kind}));
    for(const edge of state.edges??[])if(!edge.ownerType)out.push({from:edge.from,to:edge.to,kind:edge.kind});
    return out;
  };
  const materialOfGraph=id=>typeof id==='string'&&id.startsWith('material:')?materials.get(id.slice(9))??previewMaterials.get(id.slice(9))??null:null;
  const rungOfGraph=id=>{const material=materialOfGraph(id);return material?ladderRung({material,node:{kind:material.kind}}):null;};
  // From the chosen star on the whole tree, threads lead into the dark toward the stars the space does not hold yet.
  const threadKinds=new Map();
  function darkThreads(id){
    if(lens!=='tree'||measuredLens(lens)||route?.path||interaction||!compareBar.hidden||!id||!materialOfGraph(id))return [];
    const current=graphState();
    const held=new Set(current.nodes.map(node=>node.id));
    const positionOf=graphId=>{const node=current.nodes.find(item=>item.id===graphId);if(Array.isArray(node?.position))return node.position;const position=materialOfGraph(graphId)?.position;return Array.isArray(position)&&position.length>=2?position:null;};
    return threadsIntoDark(id,{edges:playEdges(),shown:held,positionOf}).filter(thread=>materials.has(thread.id.slice(9))).map(thread=>{threadKinds.set(`thread:${thread.id}`,thread.kind);return {id:`thread:${thread.id}`,from:id,direction:thread.direction,color:styleOf(thread.kind)?.color??RELATIONS[thread.kind]?.color,label:`${label(materialOfGraph(thread.id))} · ${t('threadNote')}`};});
  }
  function threadHint(threadId){
    const graphId=String(threadId).replace(/^thread:/,''),material=materialOfGraph(graphId);if(!material)return null;
    const kind=threadKinds.get(threadId);
    return {eyebrow:[kind?relationWord(kind):'',kindName(material.kind)].filter(Boolean).join(' · '),title:keepDots(label(material)),lines:[t('threadNote')]};
  }
  // A star brought into the space comes with its prepared links to the stars the space already holds,
  // as growing a branch brings them, so the thread it was followed by stays drawn.
  function growWith(materialId){
    const held=new Set(state.nodes.map(node=>node.materialId).filter(Boolean));held.add(materialId);
    const edgeIds=(atlas.edges??[]).filter(edge=>edge.id&&((edge.from===materialId&&held.has(edge.to))||(edge.to===materialId&&held.has(edge.from)))).map(edge=>edge.id);
    model.growAtlas({nodeIds:[materialId],...(edgeIds.length?{edgeIds}:{})});state=model.getState();
    return graphIdForMaterial(materialId);
  }
  // Following a thread brings its star into the space, out of the star it was followed from.
  function followThread(threadId){
    const graphId=String(threadId).replace(/^thread:/,''),materialId=graphId.slice(9),from=selected;
    // The rising is set before the star exists: adding it draws the space at once.
    if(from)rising.set(graphId,from);
    try{const added=growWith(materialId);selectActual(added);notifyGraphResult(t('threadAdded'));reflect();sky?.reveal(added);}
    catch(error){rising.delete(graphId);notify(displayPublicError(error,locale));}
  }
  // Your own reading first. Where a step or a riddle asks, one line of your own is written before the readings open;
  // it becomes your star in front of what it is about, and can then stand beside the reading you find nearest.
  // The record keeps your words and what they are about; where it stands, and beside which reading, is your view of it.
  const ownKey=`tos.public-research.own-readings.v1.${profile}`,ownReadings=new Map(),ownDrafts=new Map();
  try{const saved=JSON.parse(localStorage.getItem(ownKey)||'{}');if(object(saved))for(const [key,value]of Object.entries(saved)){const record=typeof value==='string'?{id:value}:object(value)&&typeof value.id==='string'?{id:value.id,...(typeof value.near==='string'?{near:value.near}:{})}:null;if(record)ownReadings.set(key,record);}}catch{}
  const saveOwnReadings=()=>{try{localStorage.setItem(ownKey,JSON.stringify(Object.fromEntries(ownReadings)));}catch{}};
  const savePersonalPositions=()=>{try{localStorage.setItem(positionsKey,JSON.stringify(Object.fromEntries(personalPositions)));positionsPersistenceError=false;}catch{positionsPersistenceError=true;}};
  const hypothesesNow=()=>workspace?.getState?.()?.hypotheses??personalState?.hypotheses??[];
  const ownReadingOf=targetId=>{const record=ownReadings.get(targetId);return record?hypothesesNow().find(item=>item.id===record.id)??null:null;};
  // The star of a record: its graph node once it was joined to the graph, otherwise its own.
  const nodeOfOwn=id=>{const mapped=materializedHypotheses.get(id);return mapped&&state.nodes.some(node=>node.id===mapped)?mapped:state.nodes.some(node=>node.id===id)?id:`personal:${id}`;};
  const placeOfGraph=graphId=>graphState().nodes.find(node=>node.id===graphId)?.position??materialOfGraph(graphId)?.position??null;
  // After the block is drawn again, the keyboard is where it acted.
  function focusOwn(targetId,selector){requestAnimationFrame(()=>{const box=[...shell.querySelectorAll('.ps-own')].find(item=>item.dataset.target===targetId);box?.querySelector(selector)?.focus({preventScroll:true});});}
  function writeOwnReading(targetId,text){
    if(!workspace?.addHypothesis)return;
    const id=`space:${profile}:own:${crypto.randomUUID()}`,nodeId=`personal:${id}`,targetGraph=graphIdForMaterial(targetId),at=placeOfGraph(targetGraph);
    // Its place and its rising are set before the record exists, so the first view of it already stands in front of its question.
    if(at){personalPositions.set(nodeId,besidePosition(at,0));savePersonalPositions();}
    rising.set(nodeId,targetGraph);ownReadings.set(targetId,{id});saveOwnReadings();
    try{workspace.addHypothesis({id,kind:'interpretation',title:ownTitle(text),body:text,targetId});personalState=workspace.getState?.()??personalState;notifyGraphResult(t('ownWritten'));}
    catch(error){ownReadings.delete(targetId);saveOwnReadings();rising.delete(nodeId);notify(displayPublicError(error,locale));}
    reflect();focusOwn(targetId,'.ps-own-line');
  }
  // Setting your star beside a reading moves it there and links it to that reading in your view; the record itself is untouched,
  // so the workspace's own undo never splits it.
  function standNearest(targetId,readingGraph){
    const record=ownReadings.get(targetId),mine=ownReadingOf(targetId);if(!record||!mine)return;
    if((record.near??null)===(readingGraph??null))return;
    // A reading the space does not hold comes into it first, so the record can stand beside it and be linked to it.
    if(readingGraph&&!state.nodes.some(node=>node.id===readingGraph)){try{growWith(readingGraph.slice(9));}catch(error){notify(displayPublicError(error,locale));return;}}
    const nodeId=nodeOfOwn(mine.id),index=readingGraph?[...ownReadings.values()].filter(item=>item!==record&&item.near===readingGraph).length:0,at=placeOfGraph(readingGraph??graphIdForMaterial(targetId));
    if(at){personalPositions.set(nodeId,besidePosition(at,index));savePersonalPositions();}
    if(readingGraph)record.near=readingGraph;else delete record.near;saveOwnReadings();
    reflect();focusOwn(targetId,'.ps-own-choice[aria-pressed="true"]');
  }
  function ownReadingBlock(parent,targetId){
    if(!workspace?.addHypothesis||!targetId)return null;
    const targetGraph=graphIdForMaterial(targetId),mine=ownReadingOf(targetId),edges=playEdges();
    const around=readingsAround(targetGraph,{edges,rungOf:rungOfGraph}).filter(item=>materialOfGraph(item.id));
    const box=el('section','ps-own');box.dataset.state=mine?'written':'open';box.dataset.target=targetId;
    const nameOf=graphId=>keepDots(label(materialOfGraph(graphId)));
    const row=(graphId,small)=>{const b=button('','connection-target');b.append(el('small','',small),el('span','',nameOf(graphId)));b.addEventListener('click',()=>selectNode(graphId));return b;};
    if(!mine){
      const form=document.createElement('form');form.className='ps-own-form';
      const input=document.createElement('textarea');input.rows=2;input.maxLength=600;input.placeholder=t('ownPlaceholder');input.setAttribute('aria-label',t('ownPlaceholder'));
      // A line being written survives the column being drawn again, with its caret.
      const draft=ownDrafts.get(targetId);if(draft){input.value=draft.text;if(draft.focused)requestAnimationFrame(()=>{if(input.isConnected){input.focus({preventScroll:true});input.setSelectionRange(draft.start,draft.end);}});}
      const keep=()=>{ownDrafts.set(targetId,{text:input.value,start:input.selectionStart,end:input.selectionEnd,focused:document.activeElement===input});};
      input.addEventListener('input',keep);input.addEventListener('select',keep);input.addEventListener('keyup',keep);input.addEventListener('blur',()=>{const value=ownDrafts.get(targetId);if(value)value.focused=false;});
      const write=document.createElement('button');write.type='submit';write.className='primary ps-own-write';write.textContent=t('ownWrite');
      form.append(input,write);
      form.addEventListener('submit',event=>{event.preventDefault();const text=input.value.trim();if(!text){input.focus();return;}ownDrafts.delete(targetId);writeOwnReading(targetId,text);});
      // Enter writes the line; Shift+Enter breaks it.
      input.addEventListener('keydown',event=>{if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();form.requestSubmit();}});
      box.append(form);
      // The readings wait folded beneath, a click away, until you have written your own.
      if(around.length){const later=document.createElement('details');later.className='ps-own-around';later.append(el('summary','',`${t('ownAround')} · ${around.length}`));for(const item of around)later.append(row(item.id,relationWord(item.kind)||kindName(materialOfGraph(item.id)?.kind)));box.append(later);}
      parent.append(box);return box;
    }
    const own=button('','ps-own-line');own.append(el('span','eyebrow',t('ownMine')),el('span','ps-own-text',mine.body));
    own.addEventListener('click',()=>selectNode(nodeOfOwn(mine.id)));box.append(own);
    if(around.length){
      const near=el('div','ps-own-near');near.setAttribute('role','group');near.append(el('p','eyebrow',t('ownNearest')));
      const chosen=ownReadings.get(targetId)?.near??null;
      for(const item of around){const b=button('','ps-own-choice');b.append(el('span','',nameOf(item.id)),el('small','',relationWord(item.kind)||kindName(materialOfGraph(item.id)?.kind)));b.setAttribute('aria-pressed',String(chosen===item.id));b.addEventListener('click',()=>standNearest(targetId,chosen===item.id?null:item.id));near.append(b);}
      const none=button(t('ownNone'),'ps-own-choice ps-own-none');none.setAttribute('aria-pressed',String(!chosen));none.addEventListener('click',()=>standNearest(targetId,null));near.append(none);
      box.append(near);
      // What follows from standing there: what that reading rests on, and what disputes it.
      if(chosen&&materialOfGraph(chosen)){
        // A reading rests first on its grounds in the source, else on the sources it is linked to; it is disputed
        // by what answers it otherwise or puts it in question, and by the other reading authored against it.
        const reading=materialOfGraph(chosen),rests=el('div','ps-own-follows'),against=el('div','ps-own-follows');
        if(!appendGrounds(rests,reading?.inquiry?.grounds,reading?.id,el('p','eyebrow',t('ownRestsOn')))){const linked=restsOn(chosen,{edges,rungOf:rungOfGraph}).filter(materialOfGraph);if(linked.length){rests.append(el('p','eyebrow',t('ownRestsOn')));for(const id of linked)rests.append(row(id,t(`rung_${rungOfGraph(id)??'source'}`)));}}
        const others=disputes(chosen,{edges}).filter(id=>materialOfGraph(id)&&id!==targetGraph),counter=reading?.inquiry?.counterReading?.text;
        if(others.length||counter){against.append(el('p','eyebrow',t('ownDisputed')));for(const id of others)against.append(row(id,relationWord(edges.find(edge=>(edge.from===chosen&&edge.to===id)||(edge.to===chosen&&edge.from===id))?.kind)||''));if(counter)inquiryDetail(against,t('counterReading'),counter);}
        for(const part of [rests,against])if(part.childElementCount)box.append(part);
      }
    }
    parent.append(box);return box;
  }
  // Time is a view: stars take places the scale gives them, the saved graph keeps its own.
  function applyTimeline(projected,routePath){
    const current=stepView(routePath)?routes.find(item=>item.id===route.id):null;
    const stepIds=current?(current.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(Boolean):null;
    const steps=stepIds?stepTimeline(stepIds,id=>timeline.xOf(id)):null;
    const place=node=>node.materialId?(steps?steps.get(node.materialId):timeline.positions.get(node.materialId)):null;
    projected.nodes=projected.nodes.filter(node=>place(node)).map(node=>({...node,position:[...place(node)]}));
    const ids=new Set(projected.nodes.map(node=>node.id));projected.edges=projected.edges.filter(edge=>ids.has(edge.from)&&ids.has(edge.to));
    // An open branch reads top to bottom in its own order, so the rows' names and threads step aside.
    const labels=timeline.labels(locale(),{laneWidth:laneStrip()-24}).filter(label=>!steps||(label.variant!=='lane'&&label.variant!=='thread'));
    // The chosen move shows on the scale the span its writing is dated to, in the bonsai's own kind of dating.
    scaleChoice=selected;
    const chosen=projected.nodes.find(node=>node.id===selected),span=chosen&&timeline.kind==='years'?timeline.spanOf(chosen.materialId):null;
    if(span){
      const time=timeline.timeOf(chosen.materialId),work=time?.workId?collectionByLegacyId.get(time.workId):null,own=datingWords(time.composition);
      const words=`${own?.when||formatYears(span,locale())}${own?.kind==='relative-life'||own?.kind==='relative-activity'?` · ${own.context}`:''}`;
      const xs=[timeline.xOfYear(span.from),timeline.xOfYear(span.to)];
      labels.push({id:'time:span',variant:'span',strip:timeline.rows.side,span:xs,title:`${work?`${quote(tr(work.title,locale()))} · `:''}${words}`,subtitle:'',maxWidth:360,center:[(xs[0]+xs[1])/2,0,0]});
    }
    if(traceConcept)traceThrough(projected,labels,span);
    projected.clusters=labels;
  }
  // A traced concept joins the stars that carry it in the order of the scale; the rest step back.
  function conceptsOfStar(materialId){
    if(profile==='antiquity')return passageOf(materialId)?.concepts??[];
    const own=materials.get(materialId)?.kind==='concept'?[materialId]:[];
    return [...own,...(atlas.edges??[]).filter(edge=>edge.kind!=='contains'&&(edge.from===materialId||edge.to===materialId)).map(edge=>edge.from===materialId?edge.to:edge.from).filter(id=>materials.get(id)?.kind==='concept')];
  }
  function traceableConcepts(){
    const counts=new Map();for(const id of starIds)for(const concept of new Set(conceptsOfStar(id)))counts.set(concept,(counts.get(concept)??0)+1);
    const title=id=>profile==='antiquity'?collectionByLegacyId.get(id)?.title:materials.get(id)?.title;
    return [...counts.entries()].filter(([,n])=>n>=2).map(([id,n])=>({id,n,title:title(id)??id})).sort((a,b)=>b.n-a.n||tr(a.title,locale()).localeCompare(tr(b.title,locale())));
  }
  // The trace's lines by id: a chosen one names its concept and its two ends, whatever colons their ids hold.
  const traceLinks=new Map();
  const traceName=concept=>tr(traceableConcepts().find(item=>item.id===concept)?.title,locale())||concept;
  function traceThrough(projected,labels,chosenSpan){
    const members=new Set(projected.nodes.filter(node=>conceptsOfStar(node.materialId).includes(traceConcept)).map(node=>node.id));
    if(!members.size)return;
    // A star off the scale carries the concept too, but has no place in its order: it joins no line.
    const onScale=new Set(projected.nodes.filter(node=>members.has(node.id)&&!timeline.isOutside(node.materialId)).map(node=>node.id));
    const at=new Map(projected.nodes.map(node=>[node.id,node.position[0]])),line=trajectory([...onScale],id=>at.get(id));
    projected.nodes=projected.nodes.map(node=>members.has(node.id)?{...node,prominent:true}:{...node,dim:true});
    for(const [from,to] of line.links){const id=`trace:${traceConcept}:${from}:${to}`;traceLinks.set(id,{concept:traceConcept,from,to});projected.edges.push({id,from,to,kind:'relates',label:t('traceLink'),color:'#efd28a',dash:'dotted',weight:1.5,highlight:true});}
    if(!chosenSpan&&line.ids.length>1){
      const xs=[at.get(line.ids[0]),at.get(line.ids.at(-1))];
      labels.push({id:'time:trace',variant:'span',strip:timeline.rows.side,span:xs,title:`${t('traceLabel')} ${quote(traceName(traceConcept))} · ${members.size}`,subtitle:'',maxWidth:360,center:[(xs[0]+xs[1])/2,0,0]});
    }
  }
  // Space is a view as well: stars stand at the place of their writing, the saved graph keeps its own positions.
  function applyPlane(projected){
    const place=node=>node.materialId?geography.positions.get(node.materialId):null;
    projected.nodes=projected.nodes.filter(node=>place(node)).map(node=>({...node,position:[...place(node)]}));
    // A line joins two places on the map; a star without a place stands apart and draws none across it.
    const ids=new Set(projected.nodes.filter(node=>!geography.isOutside(node.materialId)).map(node=>node.id));projected.edges=projected.edges.filter(edge=>ids.has(edge.from)&&ids.has(edge.to));
    scaleChoice=selected;
    projected.clusters=geography.labels(locale());
    projected.backdrop=geographyBackdrop();
  }
  // The coastlines lie under the map of places in its own projection; computed once.
  let backdropLines=null;
  function geographyBackdrop(){return backdropLines??=clipLinesRight((bonsai?.basemap?.lines??[]).map(line=>line.map(point=>geography.toXY(point))),geography.map.x[1]);}
  // Proximity is measured from one star: the chosen one, else the open step, else the last centre.
  function nearCenterId(){
    const own=String(selected??'').replace(/^material:/,''),step=String(routeStepNode??'').replace(/^material:/,'');
    if(starSet.has(own))nearCenter=own;else if(!nearCenter&&starSet.has(step))nearCenter=step;
    return nearCenter??=starIds[0]??null;
  }
  const nearCriterion=()=>criteria.find(item=>item.id===criterion)??criteria[0];
  // A distance in words: the share in common for concepts and grounds, else the number with its unit in the right form.
  const UNIT_FORMS={relations:{ru:['шаг','шага','шагов'],en:['step','steps']},time:{ru:['год','года','лет'],en:['year','years']}};
  function nearValue(item,value){
    if(!Number.isFinite(value))return '';
    if(item.id==='concepts'||item.id==='grounds')return `${Math.round((1-value)*100)}\u00a0%`;
    const n=Math.round(value),forms=UNIT_FORMS[item.id],en=(copyLocale(locale())==='en');
    const word=forms?(en?localeText(forms.en[n===1?0:1],locale):forms.ru[n%100>=11&&n%100<=14?2:n%10===1?0:n%10>=2&&n%10<=4?1:2]):tr(item.unit,locale());
    return word?`${n}\u00a0${word}`:String(n);
  }
  function applyProximity(projected){
    const center=nearCenterId(),item=nearCriterion();
    if(!center||!item){projected.nodes=[];projected.edges=[];projected.clusters=[];return;}
    const nearLayout=proximityLayout({center,stars:starIds,distance:distances[item.id](center),groupOf:id=>nearGroupOf.get(id)??null,groups:nearGroups,integer:item.id==='relations'||item.id==='chapters'});
    const place=node=>node.materialId?nearLayout.positions.get(node.materialId):null;
    projected.nodes=projected.nodes.filter(node=>place(node)).map(node=>({...node,position:[...place(node)],...(node.materialId===center?{prominent:true}:{})}));
    const ids=new Set(projected.nodes.map(node=>node.id));projected.edges=projected.edges.filter(edge=>ids.has(edge.from)&&ids.has(edge.to)&&(edge.from===graphIdForMaterial(center)||edge.to===graphIdForMaterial(center)));
    scaleChoice=selected;
    projected.clusters=nearLayout.labels(locale(),{format:value=>nearValue(item,value),unmeasuredLabel:{ru:t('nearNotMeasured'),en:t('nearNotMeasured')}});
  }
  // A measuring lens fills its free area and shows its whole extent; other lenses frame the sky as the constructor does.
  function frameSky(){
    if(!sky)return;
    if(lens==='time'&&timeline)sky.frame({fill:true,x:timeline.bounds});
    // A plane keeps one unit on both axes: the map of places and the rings of proximity are never stretched.
    else if(lens==='space'&&geography)sky.frame({fill:true,plane:true,x:geography.bounds.x,y:geography.bounds.y});
    else if(lens==='proximity'&&proximityLens)sky.frame({fill:true,plane:true,x:[-NEAR_EXTENT,NEAR_EXTENT],y:[-NEAR_EXTENT,NEAR_EXTENT]});
    else sky.frame();
  }
  // An open branch or route on the scale of time reads in its own rows, top to bottom.
  function stepView(routePath=interaction||!compareBar.hidden?null:route?.path??null){return !!(routePath&&route?.source==='atlas');}
  // The rows' names keep a strip at the map's left, as wide as the longest name needs; an open branch has no such rows.
  // Its type follows the page's widths: 13px on a wide screen, 12px on a middle one, 11px on a phone.
  let laneMeasure=null;
  function laneStrip(){
    if(!(lens==='time'&&timeline)||stepView())return 0;
    const width=shell.clientWidth||innerWidth,[size,cap]=width>1180?[13,248]:width>760?[12,176]:[11,162];
    laneMeasure??=document.createElement('canvas').getContext('2d');if(!laneMeasure)return cap;
    laneMeasure.font=`${size}px ${getComputedStyle(shell).getPropertyValue('--ps-serif').trim()||'Georgia,serif'}`;
    const widest=Math.max(0,...timeline.lanes.map(lane=>laneMeasure.measureText(tr(lane.title,locale())).width));
    return Math.round(Math.min(cap,Math.max(72,widest+34)));
  }
  let sky,scaleChoice;
  const NEAR_EXTENT=530;
  // The layout a measuring lens shows, and the strip its scale keeps above the map.
  const layoutKind=()=>lens==='time'&&timeline?timeline.kind:lens==='space'&&geography?'plane':lens==='proximity'&&proximityLens?'proximity':null;
  const layoutStrip=()=>lens==='time'&&timeline?timeline.strip:lens==='space'&&geography?geography.strip:0;
  let shownView=null;
  function refreshSky(){if(!sky)return;shownView=visibleState();sky.update(shownView,Object.fromEntries(graphState().nodes.map(node=>[node.id,label(matForNode(node)??node)])));rising.clear();sky.select(selected);sky.selectEdge(selectedEdge);sky.refresh();}
  function reflect(){
    if(destroyed)return;state=model.getState();
    // The scale of time rearranges the map's frame: its tools join the heading and its strips take their room.
    const kind=layoutKind();if(kind)shell.dataset.scale=kind;else delete shell.dataset.scale;
    turnButton.hidden=measuredLens(lens);controlsButton.hidden=measuredLens(lens);
    if(measuredLens(lens)&&controls?.open)controls.close();
    else if(!measuredLens(lens)&&controls&&!controlsDone&&!controlsTimer&&!controls.open)controlsTimer=setTimeout(()=>openControlsOnce(),900);
    refreshSky();
    undoButton.disabled=!model.canUndo();redoButton.disabled=!model.canRedo();
    if(!notify.timer)statusLabel.textContent=savedStatus();
    if(personal.hidden)renderColumn();
    renderOrientation();renderSearch();renderInteraction();
  }
  function paintLenses(){
    lensList.replaceChildren();
    // Lenses that only group come first; those that measure follow apart, and show their scale on the map.
    let previous=null;
    for(const item of lenses){
      const kind=item.positions?.positions==='measure'?'measure':'group';
      if(previous==='group'&&kind==='measure')lensList.append(el('span','ps-lens-divider'));previous=kind;
      const b=button('', 'lens');b.setAttribute('aria-pressed',String(lens===item.id));b.dataset.positions=kind;
      b.append(el('span','lens-icon',item.icon),el('span','lens-name',tr(item.title,locale())));
      setHint(b,tr(item.description,locale()));
      b.addEventListener('click',event=>{lens=item.id;const refocus=event.detail===0||lensList.contains(document.activeElement);
      // An open branch reads on the tree, on the scale of time and on the map of places; other lenses look at the whole space.
      if(route&&!['tree','time','space'].includes(lens)){route=null;routeStepNode=null;shell.dataset.journey='false';}browsingRoutes=false;persistView();paintLenses();reflect();frameSky();
      // The lenses are drawn again; the keyboard stays on the one it chose.
      if(refocus)lensList.querySelector('[aria-pressed="true"]')?.focus({preventScroll:true});});lensList.append(b);}
    // On a phone the lenses scroll in one row; the chosen one stays in sight, also when a link opened it.
    const chosen=lensList.querySelector('[aria-pressed="true"]');
    if(chosen&&lensList.scrollWidth>lensList.clientWidth&&(chosen.offsetLeft<lensList.scrollLeft||chosen.offsetLeft+chosen.offsetWidth>lensList.scrollLeft+lensList.clientWidth))lensList.scrollLeft=chosen.offsetLeft-(lensList.clientWidth-chosen.offsetWidth)/2;
  }
  function paintRelations(){
    relationChips.replaceChildren();
    const all=button(t('all'),'relation-chip');all.setAttribute('aria-pressed',String(enabled.size===relationKinds.length));all.addEventListener('click',()=>{enabled=new Set(relationKinds);reflect();paintRelations();});relationChips.append(all);
    for(const key of relationKinds){const value=styleOf(key),b=button(tr(value,locale()),'relation-chip');b.style.setProperty('--relation-color',value.color);b.dataset.dash=value.dash??'solid';b.style.setProperty('--relation-weight',String(value.weight??1));if(value.hint)setHint(b,tr(value.hint,locale()));b.setAttribute('aria-pressed',String(enabled.has(key)));b.addEventListener('click',()=>{if(enabled.has(key))enabled.delete(key);else enabled.add(key);reflect();paintRelations();});relationChips.append(b);}
    // The legend also says what a star's form means: what the tree stands on, what is proposed, what is yours.
    const forms=el('div','ps-status-items');statusLegend.replaceChildren(el('span','ps-legend-title',t('statusTitle')),forms);
    for(const [form,key] of [['source','formSource'],['proposal','formProposal'],['personal','formPersonal']]){const chip=el('span','ps-form-chip');chip.dataset.form=form;setHint(chip,t(`${key}Hint`));chip.append(el('i'),el('span','',t(key)));forms.append(chip);}
  }
  // ------------------------------------------------------------ measure and completeness
  function openMeasure(){measureOpen=true;selectedEdge=null;browsingRoutes=false;personal.hidden=true;columnOpen=true;sky?.selectEdge(null);reflect();keepInColumn(panelContent.firstElementChild);}
  function bars(parent,title,rows,{total=null}={}){
    const box=el('section','ps-measure-block');box.append(el('h2','minor-title',title));
    const max=Math.max(1,total??Math.max(0,...rows.map(row=>row.n)));
    for(const row of rows){
      const line=el(row.action?'button':'div','ps-bar');if(row.action){line.type='button';line.addEventListener('click',row.action);}
      const whole=row.total??total,track=el('span','ps-bar-track'),fill=el('i');fill.style.width=`${Math.max(row.n?2:0,Math.round(row.n/(whole??max)*100))}%`;track.append(fill);
      line.append(el('span','ps-bar-label',row.label),track,el('span','ps-bar-n',row.total?`${row.n} / ${row.total}`:String(row.n)));
      if(row.detail)setHint(line,row.detail);
      box.append(line);
    }
    parent.append(box);
  }
  // A path drawn on a shared scale: each step at its value, joined in its own order; a step back is drawn apart.
  function sparkline(values,min,max){
    const ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),w=200,h=18,x=value=>4+(value-min)/Math.max(1e-9,max-min)*(w-8);
    svg.setAttribute('viewBox',`0 0 ${w} ${h}`);svg.setAttribute('class','ps-spark');svg.setAttribute('aria-hidden','true');
    const line=(x1,x2,cls)=>{const l=document.createElementNS(ns,'line');l.setAttribute('x1',x1.toFixed(1));l.setAttribute('x2',x2.toFixed(1));l.setAttribute('y1',String(h/2));l.setAttribute('y2',String(h/2));l.setAttribute('class',cls);svg.append(l);};
    line(4,w-4,'ps-spark-base');
    const known=values.filter(Number.isFinite);
    for(let i=1;i<known.length;i++){const a=x(known[i-1]),b=x(known[i]);if(Math.abs(b-a)<.5)continue;const arc=document.createElementNS(ns,'path'),mid=(a+b)/2,lift=b<a?-6:6;arc.setAttribute('d',`M${a.toFixed(1)} ${h/2} Q${mid.toFixed(1)} ${h/2+lift} ${b.toFixed(1)} ${h/2}`);arc.setAttribute('class',b<a?'ps-spark-back':'ps-spark-step');svg.append(arc);}
    for(const value of known){const dot=document.createElementNS(ns,'circle');dot.setAttribute('cx',x(value).toFixed(1));dot.setAttribute('cy',String(h/2));dot.setAttribute('r','2');dot.setAttribute('class','ps-spark-dot');svg.append(dot);}
    return svg;
  }
  function pathRows(parent,title,rows,{min,max,ends}){
    const box=el('section','ps-measure-block');box.append(el('h2','minor-title',title));
    for(const row of rows){
      const item=routeItems().find(entry=>entry.source==='atlas'&&entry.id===row.id),b=button('','ps-path-row');
      const head=el('span','ps-path-head');head.append(el('span','ps-path-title',tr(row.title,locale())),el('span','ps-path-count',`${row.size}${row.returns?` · ↺ ${row.returns}`:''}`));
      b.append(head,sparkline(row.values,min,max));if(item)b.addEventListener('click',()=>{measureOpen=false;startRoute(item);});box.append(b);
    }
    const scale=el('div','ps-path-scale');scale.append(el('span','',ends[0]),el('span','',ends[1]));box.append(scale);
    parent.append(box);
  }
  function renderMeasure(){
    panelContent.replaceChildren(el('h1','',publicTitle()));
    const words=(prefix,map,order)=>order.filter(key=>map[key]).map(key=>({label:t(`${prefix}${key}`),n:map[key]}));
    if(profile==='antiquity'){
      const m=measureAntiquity({routes,entries:library.space?.collection??[],timeline,placeOf});if(!m)return;
      bars(panelContent,`${t('measureDating')} · ${countMoves(m.total)}`,words('measureKind_',m.dating.kinds,['date-assertion','interval-assertion','relative-phase','relative-life','relative-activity','unknown-date','none']),{total:m.total});
      bars(panelContent,t('measureCertainty'),words('certainty_',m.dating.certainty,['exact','approximate','uncertain','unknown']),{total:m.total});
      bars(panelContent,t('measureFooting'),words('footing_',m.dating.footing,['high','medium','low']),{total:m.total});
      if(geography)bars(panelContent,t('measurePlaces'),words('placeBasis_',m.places.bases,['composition','event','activity','unknown']),{total:m.total});
      bars(panelContent,t('measureEras'),m.eras.map(row=>({label:tr(row.title,locale()),n:row.n})),{total:m.total});
      // A concept opens its trace on the scale of time.
      bars(panelContent,t('measureConcepts'),m.concepts.map(row=>({label:tr(row.title,locale()),n:row.n,detail:t('traceTitle'),action:()=>{traceConcept=row.id;lens='time';measureOpen=false;persistView();paintLenses();reflect();frameSky();}})));
      const middle=id=>{const span=timeline.spanOf(id);return span?(span.from+span.to)/2:NaN;};
      pathRows(panelContent,t('measureBranches'),routes.map(item=>{const ids=(item.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(id=>timeline.positions.has(id)),row=m.branches.find(value=>value.id===item.id);return {id:item.id,title:item.title,size:countMoves(ids.length),returns:row?.returns??0,values:ids.map(middle)};}),
        {min:timeline.domain.min,max:timeline.domain.max,ends:timeline.scaleEnds(locale())});
      const gaps=el('section','ps-measure-block');
      const list=(title,rows)=>{if(!rows.length)return;gaps.append(el('h2','minor-title',title),el('p','ps-measure-list',rows.map(row=>`${quote(tr(row.title,locale()))} · ${row.n}`).join(' · ')));};
      list(t('measureLifeOnly'),m.gaps.lifeOnly);if(geography)list(t('measurePlaceless'),m.gaps.placeless);
      if(gaps.childElementCount)panelContent.append(gaps);
      return;
    }
    const m=measureZarathustra({routes,timeline,book:bonsai?.chronology?.book,placeOf,materials:[...materials.values()]});if(!m)return;
    bars(panelContent,`${t('measureGrounding')} · ${m.total}`,[{label:t('measureInBook'),n:m.inBook},{label:t('measureOutside'),n:m.outside}],{total:m.total});
    // Each part says when and where it was written, as its bonsai dates and places it.
    const partWords=row=>{const span=row.writing?chronologyReader?.span(row.writing):null,place=row.writing?placesReader?.at(row.writing):null;return [span?formatYears({...span,certainty:'exact'},locale()):'',place?tr(placesReader.place(place.place)?.label,locale()):''].filter(Boolean);};
    if(m.chapters.total)bars(panelContent,`${t('measureChapters')} · ${m.chapters.cited} / ${m.chapters.total}`,m.parts.map(row=>({label:[`${copyLocale(locale)==='en'?localeText('Part',locale):'Часть'} ${['I','II','III','IV'][row.part-1]??row.part}`,...partWords(row)].join(' · '),n:row.cited,total:row.total})));
    if(m.outsideTexts.length)bars(panelContent,t('measureOutsideTexts'),m.outsideTexts.map(row=>({label:tr(atlas.references?.[row.ref]?.work,locale())||row.work||'—',n:row.n})));
    if(geography)bars(panelContent,t('measurePlaces'),words('placeBasis_',m.places.bases,['composition','event','activity','unknown']),{total:m.total});
    const chapterOf=id=>timeline.placeOf(id)?.chapter??NaN;
    pathRows(panelContent,t('measureRoutes'),routes.map(item=>{const ids=(item.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(id=>timeline.positions.has(id)),row=m.routes.find(value=>value.id===item.id);return {id:item.id,title:item.title,size:countSteps(ids.length),returns:row?.returns??0,values:ids.map(chapterOf)};}),
      {min:0,max:timeline.domain.max,ends:timeline.scaleEnds(locale())});
  }
  function renderWelcome(){panelContent.replaceChildren();}
  function renderLensControls(mode){
    lensControls.replaceChildren();
    if(mode!=='measure'){if(lens==='time'&&timeline)appendTraceControls(lensControls);if(lens==='proximity'&&proximityLens)appendProximityControls(lensControls);}
    lensControls.hidden=!lensControls.childElementCount;
  }
  let traceOpen=false;
  function appendTraceControls(parent){
    const concepts=traceableConcepts();if(!concepts.length)return;
    const box=el('section','ps-trace');box.append(el('h2','minor-title',t('traceTitle')));
    // The most frequent concepts and the traced one show at once; the rest open on demand, or scroll in one row on a phone.
    const chips=el('div','ps-chip-row');chips.dataset.open=String(traceOpen);
    const extra=concepts.filter((concept,index)=>index>=6&&concept.id!==traceConcept).length;
    concepts.forEach((concept,index)=>{const chip=button('','ps-trace-chip');if(index>=6&&concept.id!==traceConcept)chip.classList.add('ps-chip-extra');chip.append(el('span','',tr(concept.title,locale())),el('small','',String(concept.n)));chip.setAttribute('aria-pressed',String(traceConcept===concept.id));chip.addEventListener('click',()=>{traceConcept=traceConcept===concept.id?null:concept.id;persistView();reflect();});chips.append(chip);});
    if(extra){const more=button(traceOpen?'−':`+${extra}`,'ps-trace-chip ps-chip-more');more.setAttribute('aria-expanded',String(traceOpen));more.setAttribute('aria-label',t('traceTitle'));more.addEventListener('click',()=>{traceOpen=!traceOpen;renderLensControls(columnMode());});chips.append(more);}
    box.append(chips);parent.append(box);
  }
  function appendProximityControls(parent){
    const item=nearCriterion(),box=el('section','ps-near');box.append(el('h2','minor-title',t('nearTitle')));
    const chips=el('div','ps-chip-row');chips.setAttribute('role','radiogroup');chips.setAttribute('aria-label',t('nearTitle'));
    for(const value of criteria){const chip=button(tr(value.label,locale()),'ps-near-chip');chip.setAttribute('role','radio');chip.setAttribute('aria-checked',String(value.id===item?.id));chip.setAttribute('aria-pressed',String(value.id===item?.id));setHint(chip,tr(value.measure,locale()));chip.addEventListener('click',()=>{criterion=value.id;persistView();reflect();});chips.append(chip);}
    box.append(chips);parent.append(box);
  }
  function ladderLine(rung){
    const wrap=el('p','ps-ladder');setHint(wrap,t('ladderHint'));wrap.setAttribute('aria-label',t('ladderLabel'));
    if(rung==='personal'){const item=el('span','ps-rung',t('rungPersonal'));item.setAttribute('aria-current','true');wrap.append(item);return wrap;}
    ['source','reading','comparison'].forEach((key,index)=>{if(index)wrap.append(el('span','ps-rung-sep','›'));const item=el('span','ps-rung',t(`rung_${key}`));if(key===rung)item.setAttribute('aria-current','true');wrap.append(item);});
    return wrap;
  }
  const chronologyReader=bonsai?.chronology?readChronology(bonsai.chronology):null;
  // A star's facts sit under its title as short lines, each marked with its lens's sign:
  // ◷ when its text was written, ⌖ where, ⊙ how near it stands. Each opens to what it rests on.
  function datingWords(situationId){
    const situation=chronologyReader?.situation(situationId);if(!situation)return null;
    const span=chronologyReader.span(situationId),kind=datingKind(situationId,chronologyReader),dating=situation.dating;
    const anchor=dating?.kind==='relative-order'?chronologyReader.situation(dating.relative?.anchor_ref):null;
    const when=tr(situation.display,locale())||formatDating(dating,locale(),{span})||t('dateUnknown');
    // Antiquity's star already names its work; Zarathustra's star rests on a text it names here.
    const context=kind==='relative-phase'?tr(anchor?.label,locale()):kind==='relative-life'?t('duringLifeShort'):kind==='relative-activity'?t('duringActivityShort'):profile==='antiquity'?'':tr(situation.label,locale());
    return {situation,span,kind,when,context,certainty:span?.certainty??dating?.certainty};
  }
  function fact(icon,value,context,{kind=''}={}){
    const fold=document.createElement('details');fold.className='ps-fact';if(kind)fold.dataset.kind=kind;
    const summary=el('summary');summary.append(el('span','ps-fact-icon',icon),el('span','ps-fact-value',value));if(context)summary.append(el('span','ps-fact-context',context));
    const body=el('div','ps-fact-body');fold.append(summary,body);return {fold,body};
  }
  function sourceRow(parent,sources,extra=[]){
    const row=el('p','ps-fact-sources');
    for(const source of [...sources,...extra]){const link=document.createElement('a');link.href=source.url;link.target='_blank';link.rel='noopener noreferrer';link.textContent=`${source.short} ↗`;setHint(link,source.title??'');row.append(link);}
    if(row.childElementCount)parent.append(row);
  }
  function appendDating(parent,materialId){
    if(!materialId||!chronologyReader)return;
    const situationId=situationOf(materialId),words=situationId?datingWords(situationId):null;if(!words)return;
    const {fold,body}=fact('◷',words.when,words.context,{kind:words.kind});
    const said=[tr(words.situation.statement,locale()),tr(words.situation.note,locale())].filter(Boolean).join(' ');if(said)body.append(el('p','',said));
    if(words.situation.footing!=='high')body.append(el('p','ps-fact-footing',t(`dating_${words.situation.footing}`)));
    sourceRow(body,chronologyReader.sources(situationId));
    // A dating within a life or a phase shows that span too, with its own source.
    const authorId=profile==='antiquity'?timeline?.timeOf?.(materialId)?.authorId:null,lifeId=authorId?chronologyReader.lifeOf(authorId):null,life=lifeId?chronologyReader.span(lifeId):null;
    if(life){const line=el('p','ps-fact-life',`${tr(chronologyReader.situation(lifeId)?.label,locale())} · ${formatYears(life,locale())}`);body.append(line);sourceRow(body,chronologyReader.sources(lifeId));}
    parent.append(fold);
  }
  function appendPlace(parent,materialId){
    if(!materialId||!geography||!starSet.has(materialId))return;
    // An entry may name a place, or say why the sources leave it open.
    const entry=placeEntryOf(materialId),place=entry?.place?placesReader.place(entry.place):null;
    const {fold,body}=fact('⌖',place?tr(place.label,locale()):t('placeUnknown'),entry?.basis==='activity'&&place?t('placeActivityShort'):'',{kind:place?entry.basis:'unknown'});
    if(entry?.statement)body.append(el('p','',tr(entry.statement,locale())));
    if(entry?.footing&&entry.footing!=='high')body.append(el('p','ps-fact-footing',t(`dating_${entry.footing}`)));
    const situation=situationOf(materialId);
    sourceRow(body,situation?placesReader.sources(situation):[],(place?.gazetteer??[]).map(item=>({short:item.scheme==='pleiades'?'Pleiades':item.scheme==='wikidata'?'Wikidata':item.scheme,url:item.url,title:item.url})));
    if(!body.childElementCount)fold.classList.add('ps-fact-plain');
    parent.append(fold);
  }
  function appendDistance(parent,materialId){
    const center=nearCenterId(),item=nearCriterion();if(!materialId||!center||!item||!starSet.has(materialId))return;
    // Distances are measured here from the current centre, never read back from the map last drawn.
    const measure=distances[item.id](center),distanceOf=id=>{const value=measure(id);return Number.isFinite(value)?value:null;};
    // The centre lists the stars nearest to it by the criterion; choosing one moves the centre there.
    if(materialId===center){
      const nearest=starIds.filter(id=>id!==center&&distanceOf(id)!==null).sort((a,b)=>distanceOf(a)-distanceOf(b)).slice(0,6);
      const {fold,body}=fact('⊙',tr(item.label,locale()),nearest.length?t('nearClosest'):t('nearIsCenter'));
      for(const id of nearest){const row=button('','connection-target');row.append(el('small','',nearValue(item,distanceOf(id))),el('span','',label(previewMaterials.get(id))));row.addEventListener('click',()=>selectNode(graphIdForMaterial(id)));body.append(row);}
      if(!body.childElementCount)fold.classList.add('ps-fact-plain');
      parent.append(fold);return;
    }
    // By shared concepts or grounds the fact counts what the two stars share, and names it.
    const own=item.id==='concepts'?id=>conceptsOfStar(id):item.id==='grounds'?id=>timeline?.groundsOf?.(id)??[]:null;
    const shared=own?[...new Set(own(materialId))].filter(id=>new Set(own(center)).has(id)):[];
    const value=distanceOf(materialId);
    const words=value===null?t('nearNotMeasured'):own?`${t(item.id==='concepts'?'nearSharedConcepts':'nearSharedGrounds')}: ${shared.length}`:nearValue(item,value);
    const {fold,body}=fact('⊙',words,`${t('nearFrom')} ${quote(label(previewMaterials.get(center)))}`);
    const name=id=>item.id==='concepts'?tr(collectionByLegacyId.get(id)?.title,locale())||tr(materials.get(id)?.title,locale())||id:[tr(atlas.references?.[id]?.work,locale()),tr(atlas.references?.[id]?.locator,locale())].filter(Boolean).join(' · ')||id;
    if(shared.length)body.append(el('p','',shared.map(name).join(' · ')));
    if(!body.childElementCount)fold.classList.add('ps-fact-plain');
    parent.append(fold);
  }
  function renderNode(id,{own=true}={}){
    const materialId=id.startsWith('material:')?id.slice(9):null;
    const preview=materialId&&previewMaterials.get(materialId);
    const node=graphState().nodes.find(item=>item.id===id)??(preview?{id,materialId,kind:preview.kind,sourceIds:[]}:null);if(!node){selected=null;return renderWelcome();}
    const inSavedGraph=state.nodes.some(item=>item.id===id),inView=graphState().nodes.some(item=>item.id===id);
    const material=matForNode(node),ownerHyp=(personalState?.hypotheses??[]).find(item=>item.id===node.workspaceHypothesisId||item.id===node.id||materializedHypotheses.get(item.id)===node.id);
    panelContent.replaceChildren();
    const nodeTitle=label(node.ownerType==='personal-hypothesis'?node:material??node),part=partName(profile,material,locale(),{moveIds});panelContent.append(el('p','eyebrow',`${t('kind')} · ${part||kindName(material?.kind??node.kind)}`),el('h1','',keepDots(node.ownerType==='personal-hypothesis'&&nodeTitle.length>120?kindName(node.kind):nodeTitle)));
    // A move names its author, work and place right under its title.
    const witness=part&&!material?.route?sourceLine(material,{materials:previewMaterials,entryByLegacyId:collectionByLegacyId,locale:locale()}):'';if(witness)panelContent.append(el('p','ps-source-line',witness));
    // A star names when and where its text was written, in the bonsai's own kinds of dating and with their sources;
    // on the lens of proximity, how far it stands from the centre and by what.
    appendDating(panelContent,material?.id);appendPlace(panelContent,material?.id);
    if(lens==='proximity'&&proximityLens)appendDistance(panelContent,material?.id);
    panelContent.append(ladderLine(ladderRung({material,node})));
    const branchRoute=profile==='antiquity'?routeItems().find(item=>item.source==='atlas'&&(item.id===material?.route?.id||`antiquity:route:${item.id}`===material?.id)):null;
    if(branchRoute){const walk=button(`${t('walkBranch')} · ${countMoves(branchRoute.steps?.length??0)} →`,'primary ps-walk-branch');walk.addEventListener('click',()=>{closePanel();startRoute(branchRoute);});panelContent.append(walk);}if(node.ownerType==='personal-hypothesis'&&nodeTitle.length>120)panelContent.append(el('p','ps-personal-title',nodeTitle));
    if(node.ownerType==='personal-hypothesis'){
      if(node.body)panelContent.append(el('p','body',node.body));
      if(ownerHyp?.targetId)panelContent.append(el('p','muted',`${t('related')}: ${ownerHyp.targetId}`));
      const proposals=personalProposals(ownerHyp?.id??node.workspaceHypothesisId);
      appendPersonalProposals(panelContent,proposals);
    }else if(material){
      const source=sourceForMaterial(material,node);
      if(source?.exact){const open=button(`↗ ${sourceActionLabel(source)}`,'primary');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));panelContent.append(open);}
      else if(source?.binding){const open=button(`↗ ${sourceActionLabel(source)}`,'primary');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));panelContent.append(open);}
      const body=material.atlasBody??material.body;
      if(body)fragments.render(panelContent,body,{className:'body'});
      // A riddle asks the reader first: its question, the reader's own line, and only then its readings, folded until written.
      const riddle=own&&material.kind==='question'&&materials.has(material.id),answered=!riddle||!!ownReadingOf(material.id);
      if(riddle){const asks=material.inquiry?.question??material.question;if(tr(asks,locale()).trim())inquiryQuestion(panelContent,asks);ownReadingBlock(panelContent,material.id);}
      else if(tr(material.question,locale()).trim()&&!material.inquiry?.question){const questionCard=el('section','ps-question-card');questionCard.append(el('span','eyebrow',t('questionLabel')),el('p','',tr(material.question,locale())));panelContent.append(questionCard);}
      const inquiry=material.inquiry;
      if(inquiry){
        inquiryDetail(panelContent,t('argument'),inquiry.thesis??inquiry.argument,answered);
        appendGrounds(panelContent,inquiry.grounds,material.id);
        if(inquiry.counterReading){const alternative=inquiryDetail(panelContent,t('counterReading'),inquiry.counterReading.text,answered);if(alternative)appendGrounds(alternative,inquiry.counterReading.grounds,material.id);}
        if(!riddle)inquiryQuestion(panelContent,inquiry.question);
        if(inquiry.experiment){const experiment=inquiryDetail(panelContent,t('experiment'),inquiry.experiment.setup);if(experiment&&inquiry.experiment.question)experiment.append(el('p','inquiry-prompt',tr(inquiry.experiment.question,locale())));}
      }
      if(material.perspectives?.length){panelContent.append(el('h2','minor-title',t('perspectives')));for(const item of material.perspectives)inquiryDetail(panelContent,tr(item.title,locale()),item.body);}
      if(material.example)inquiryDetail(panelContent,t('example'),material.example);
      if(!material.inquiry?.grounds?.length&&material.sourceRefs?.length){panelContent.append(el('h2','minor-title',t('sourceRefs')));for(const ref of material.sourceRefs){const address=typeof ref==='string'?ref:ref.url??ref.ref;const labelText=typeof ref==='string'?ref:tr(ref.label,locale())||ref.title||address;if(typeof address==='string'&&/^https?:\/\//.test(address)){const a=el('a','source-link',`${labelText} ↗`);a.href=address;a.target='_blank';a.rel='noopener noreferrer';panelContent.append(a);}else if(labelText)panelContent.append(el('p','muted',labelText));}}
    }else if(node.body)panelContent.append(el('p','body',node.body));
    const authorId=material?.legacyId??material?.id,works=material?.kind==='figure'?[...previewMaterials.values()].filter(item=>item.kind==='work'&&((material.works??[]).includes(item.legacyId??item.id)||(authorId&&item.author===authorId))):[];
    if(works.length){panelContent.append(el('h2','minor-title',t('authorWorks')));for(const work of works){const open=button(label(work),'connection-target');open.addEventListener('click',()=>previewMaterial(work.id));panelContent.append(open);}}
    const connections=graphState().edges.filter(edge=>edge.from===id||edge.to===id);
    if(connections.length){panelContent.append(el('h2','minor-title',t('connections')));for(const edge of connections){const otherId=edge.from===id?edge.to:edge.from;const other=graphState().nodes.find(item=>item.id===otherId);const row=button('', 'connection-target'),type=relationName(edge,edge.to===id),extra=humanEdgeLabel(edge);row.append(el('small','',`${isBasedOn(edge)||edge.from===id?'→':'←'} ${type}${extra&&extra!==type?` · ${extra}`:''}`),el('span','',label(other?.ownerType==='personal-hypothesis'?other:matForNode(other)??other)));row.style.setProperty('--relation-color',styleOf(edge.kind).color??'#a5b7cf');row.addEventListener('click',()=>selectNode(otherId));panelContent.append(row);}}
    if(material&&materials.has(material.id)){
      const edit=button(t(inSavedGraph?'removeFromSpace':'addToSpace'),'text-button');
      edit.addEventListener('click',()=>{try{if(inSavedGraph){model.removeNode(id);selected=id;notifyGraphResult(t('removedFromSpace'));}else{const added=model.addMaterial(material.id);state=model.getState();selectActual(added);notifyGraphResult(t('saved'));}reflect();}catch(error){notify(displayPublicError(error,locale));}});panelContent.append(edit);
    }
    if(!inView)return;
    const actions=el('div','reading-bottom');
    actions.append(button(`↔ ${t('compare')}`,'text-button'),button(`↗ ${t('connect')}`,'text-button'));
    actions.children[0].addEventListener('click',()=>{closePanel();beginCompare(id);});actions.children[1].addEventListener('click',()=>{closePanel();beginConnect(id);});panelContent.append(actions);
  }
  function findEdge(id){return graphState().edges.find(edge=>edge.id===id);}
  // A chosen line the view no longer draws (a trace turned off, a lens changed) is let go, and the column is drawn anew without it.
  function dropEdge(){selectedEdge=null;sky?.selectEdge(null);renderColumn();renderOrientation();}
  function renderEdge(id){
    // A traced concept's line is no relation of the graph: it says only where the concept recurs.
    if(String(id).startsWith('trace:')){
      const link=traceLinks.get(id);if(!link)return dropEdge();
      const {concept,from,to}=link,name=traceName(concept),ends=[from,to].map(value=>label(previewMaterials.get(String(value).replace(/^material:/,''))));
      panelContent.replaceChildren(el('p','eyebrow',t('traceTitle')),el('h1','',quote(name)));
      const endpoints=el('div','edge-endpoints');[from,to].forEach((id,index)=>{const b=button(ends[index],'text-button');b.addEventListener('click',()=>selectNode(id));endpoints.append(b);if(!index)endpoints.append(el('span','','→'));});panelContent.append(endpoints);
      return;
    }
    const edge=findEdge(id);if(!edge)return dropEdge();
    panelContent.replaceChildren();
    const from=graphState().nodes.find(node=>node.id===edge.from),to=graphState().nodes.find(node=>node.id===edge.to),atlasEdge=(atlas.edges??[]).find(item=>`atlas:${item.id}`===id||item.id===id);
    // The graph copy of an atlas edge may carry only an id; the authored label lives on the atlas edge.
    const relationLabel=relationName(edge),edgeLabel=humanEdgeLabel(edge)||humanEdgeLabel(atlasEdge);
    // A transition is a link a branch or route actually walks; elsewhere it is a relation.
    const openRoute=route?.source==='atlas'?routes.find(item=>item.id===route.id):null,plainId=bareEdgeId(edge.id);
    const authored=profile==='antiquity'?isAuthoredTransition(edge):(openRoute?.transitions??[]).some(item=>bareEdgeId(item.edgeId??item.id)===plainId);
    panelContent.append(el('p','eyebrow',(authored?t('transition'):t('relation'))+' · '+relationLabel),el('h1','',edgeLabel&&edgeLabel.length<=120?edgeLabel:relationLabel));if(edgeLabel.length>120)panelContent.append(el('p','body',edgeLabel));
    if(!edge.ownerType&&edge.kind!=='contains'&&!isBasedOn(edge))panelContent.append(ladderLine('comparison'));
    if(edge.body||atlasEdge?.body)fragments.render(panelContent,edge.body??atlasEdge.body,{className:'body'});
    const inquiry=atlasEdge?.inquiry??edge.inquiry;if(inquiry){inquiryDetail(panelContent,t('relationWarrant'),inquiry.warrant,true);appendGrounds(panelContent,inquiry.grounds);inquiryDetail(panelContent,t('relationLimit'),inquiry.limit);inquiryQuestion(panelContent,inquiry.question);}
    const endpoints=el('div','edge-endpoints');const a=button(label(from?.ownerType==='personal-hypothesis'?from:matForNode(from)??from),'text-button'),b=button(label(to?.ownerType==='personal-hypothesis'?to:matForNode(to)??to),'text-button');a.addEventListener('click',()=>selectNode(from.id));b.addEventListener('click',()=>selectNode(to.id));endpoints.append(a,el('span','',relationLabel),b);panelContent.append(endpoints);
    const proposal=(workspace?.getState?.()?.proposals??[]).find(item=>item.id===edge.proposalId);
    if(proposal){panelContent.append(el('p','posture',proposalStatus(proposal)));appendPersonalGrounds(panelContent,[proposal],'proposalGrounds');}
    for(const endpoint of [from,to]){
      if(endpoint?.ownerType==='personal-hypothesis'){
        const section=el('section','ps-endpoint-source');section.append(el('h2','minor-title',`${t('endpointRelatedSources')}: ${label(endpoint)}`));
        appendPersonalGrounds(section,personalProposals(endpoint.workspaceHypothesisId));panelContent.append(section);continue;
      }
      const source=sourceForMaterial(matForNode(endpoint),endpoint);
      if(!source)continue;
      const section=el('section','ps-endpoint-source');section.append(el('h2','minor-title',`${t('endpointSource')}: ${label(matForNode(endpoint)??endpoint)}`));
      const open=button(`↗ ${sourceActionLabel(source)}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));section.append(open);panelContent.append(section);
    }
    const actions=el('div','reading-actions'),compare=button(t('compare'),'text-button');compare.addEventListener('click',()=>{closePanel();beginCompare(from?.id,to?.id);});actions.append(compare);panelContent.append(actions);
  }
  function selectNode(id,{fromMap=false}={}){
    if(interaction==='connect'){const nextId=id;if(!connectIds.length){connectIds=[nextId];renderInteraction();return;}if(nextId===connectIds[0])return;connectIds=[connectIds[0],nextId];interaction=null;renderInteraction();openRelationDialog();return;}
    if(interaction==='compare'){if(!compareIds.includes(id))compareIds.push(id);if(compareIds.length===2){interaction=null;renderInteraction();renderComparison();}else renderInteraction();return;}
    selectActual(id,{reveal:!fromMap});
  }
  function renderInteraction(){
    modeBanner.hidden=!interaction;const first=(interaction==='connect'?connectIds:compareIds)[0];
    if(!interaction){sky?.link(null);return;}
    const node=first&&graphState().nodes.find(item=>item.id===first);
    modeText.textContent=`${t(interaction==='connect'?'modeConnect':'modeCompare')}: ${first?`${label(node?.ownerType==='personal-hypothesis'?node:matForNode(node)??node)} → ${t('modeSecond')}`:t('modeFirst')}`;
    sky?.link(first??null);
  }
  function cancelInteraction(){
    const wasCompare=interaction==='compare';interaction=null;connectIds=[];compareIds=[];renderInteraction();
    refreshSky();renderColumn();
  }
  function previewMaterial(materialId){measureOpen=false;const id=graphIdForMaterial(materialId);selected=id;selectedEdge=null;browsingRoutes=false;searchValue='';search.value='';searchResults.hidden=true;search.blur();personal.hidden=true;columnOpen=true;sky?.selectEdge(null);if(graphState().nodes.some(item=>item.id===id)){sky?.select(id);sky?.reveal(id);}reflect();sky?.refresh();}
  // Choosing a star on the map leaves the map where it is. Choosing it from a list,
  // a link or a step brings it into view only if it would otherwise stay hidden.
  function selectActual(id,{reveal=false}={}){measureOpen=false;selected=id;selectedEdge=null;browsingRoutes=false;personal.hidden=true;columnOpen=true;sky?.select(id);sky?.selectEdge(null);renderColumn();renderOrientation();sky?.refresh();if(reveal||!desktop())sky?.reveal(id);}
  function selectEdge(id){measureOpen=false;selectedEdge=id;selected=null;browsingRoutes=false;personal.hidden=true;columnOpen=true;sky?.select(null);sky?.selectEdge(id);renderColumn();renderOrientation();sky?.refresh();}
  function openPanel(){columnOpen=true;renderColumn();sky?.refresh();}
  function closePanel(){selected=route?routeStepNode:null;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);if(!desktop())columnOpen=false;reflect();sky?.refresh();}
  function togglePersonal(){
    if(!personal.hidden){closePersonal();return;}
    personal.hidden=false;reading.hidden=true;columnTab.hidden=true;shell.dataset.reading='true';shell.dataset.column='personal';shell.dataset.columnOpen='true';
    personalState=workspace?.getState?.()??personalState;renderPersonalPanel();sky?.refresh();
  }
  function restoreRouteDock(){renderColumn();sky?.refresh();}
  function closePersonal(){personal.hidden=true;renderColumn();sky?.refresh();}
  function closeRoute(){routeDockTouched=true;route=null;routeStepNode=null;browsingRoutes=false;selected=null;selectedEdge=null;sky?.select(null);sky?.selectEdge(null);persistView();shell.dataset.journey='false';if(!desktop())columnOpen=false;reflect();frameSky();}
  function toggleRoutes(){
    routeDockTouched=true;personal.hidden=true;
    // From the measure panel the routes open in its place.
    if(measureOpen){measureOpen=false;browsingRoutes=!!(route||selected||selectedEdge);columnOpen=true;}
    else if(columnMode()==='overview'&&columnOpen){
      if(browsingRoutes)browsingRoutes=false;
      else if(!desktop())columnOpen=false;
      else{renderColumn();deck.querySelector('.ps-route-card')?.focus();return;}
    }else{browsingRoutes=!!(route||selected||selectedEdge);columnOpen=true;}
    renderColumn();renderOrientation();sky?.refresh();
  }
  function routeItems(){return [...routes.map(item=>({...item,source:'atlas'})),...journeyList.map(item=>({...item,source:'manifest'}))];}

  // ------------------------------------------------------------ the reading column
  const desktop=()=>desktopMode.matches;
  function currentRouteItem(){return route?routeItems().find(item=>item.source===route.source&&item.id===route.id)??null:null;}
  function shownRoute(item){return item?.source==='manifest'?localizePublicJourney(item,locale()):item;}
  function columnMode(){
    if(!personal.hidden)return 'personal';
    if(measureOpen)return 'measure';
    if(browsingRoutes)return 'overview';
    if(selectedEdge)return 'edge';
    if(selected&&!(route&&selected===routeStepNode))return 'node';
    return route?'route':'overview';
  }
  function columnCaption(mode){
    const item=currentRouteItem();
    if(mode==='route'&&item)return `${routeWord(item)} · ${t('stepName')} ${routeIndex+1} / ${shownRoute(item)?.steps?.length??0}`;
    if(mode==='node')return t('selected');
    if(mode==='edge')return t('selectedLink');
    if(mode==='measure')return t('measureAction');
    return `${t('routes')} · ${routes.length}`;
  }
  function renderColumn(){
    if(destroyed)return;
    const mode=columnMode(),wide=desktop();shell.dataset.column=mode;reading.dataset.mode=mode;
    if(mode==='personal'){reading.hidden=true;columnTab.hidden=true;shell.dataset.reading='true';shell.dataset.columnOpen='true';return;}
    // A wide screen shows the column or a tab to reopen it; a narrow screen always keeps the sheet's top bar.
    reading.hidden=wide&&!columnOpen;columnTab.hidden=!wide||columnOpen;
    shell.dataset.columnOpen=String(columnOpen);shell.dataset.sheet=wide?'':columnOpen?'open':'peek';shell.dataset.reading=String(columnOpen);
    panelTitle.textContent=columnCaption(mode);panelTitle.setAttribute('aria-expanded',String(columnOpen));
    panelClose.hidden=!wide&&!columnOpen;panelClose.setAttribute('aria-label',t(mode==='overview'&&!browsingRoutes?'collapseColumn':mode==='route'?'resetRoute':'close'));
    renderRoutes();renderLensControls(mode);
    if(mode==='overview')renderWelcome();else if(mode==='route')renderStepNode();else if(mode==='node')renderNode(selected);else if(mode==='edge')renderEdge(selectedEdge);else if(mode==='measure')renderMeasure();
    // The scale of time shows the chosen move's author's life, so a new choice redraws its strip.
    if(measuredLens(lens)&&scaleChoice!==selected)refreshSky();
  }
  // The close button climbs one level: a chosen star or link, then the open branch, then the column itself.
  function closeColumnLayer({collapse=true}={}){
    const mode=columnMode(),wasOpen=columnOpen;
    if(mode==='measure'){measureOpen=false;reflect();return;}
    if(mode==='node'||mode==='edge'){selected=route?routeStepNode:null;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);if(!route&&!desktop())columnOpen=false;}
    else if(mode==='overview'&&browsingRoutes)browsingRoutes=false;
    else if(mode==='route'){closeRoute();return;}
    else if(collapse)columnOpen=false;
    else return;
    reflect();refitAfter(wasOpen);
  }
  function toggleSheet(){if(desktop()){reading.scrollTo?.({top:0,behavior:'smooth'});return;}const wasOpen=columnOpen;columnOpen=!columnOpen;renderColumn();refitAfter(wasOpen);}
  function openColumn(){const wasOpen=columnOpen;columnOpen=true;renderColumn();refitAfter(wasOpen);}
  // A column or sheet opened or closed beside the scale of time changes its free area, so the scale refits; elsewhere the camera stays.
  function refitAfter(wasOpen){if(wasOpen!==columnOpen&&measuredLens(lens))frameSky();else sky?.refresh();}
  function keepInColumn(node){
    if(!node||reading.hidden)return;const box=reading.getBoundingClientRect(),r=node.getBoundingClientRect(),top=box.top+(panelTop.getBoundingClientRect().height||48);
    if(r.top<top)reading.scrollTop-=top-r.top;else if(r.bottom>box.bottom-12)reading.scrollTop+=Math.min(r.top-top,r.bottom-box.bottom+12);
  }
  // ------------------------------------------------------------ orientation
  function goOverview(){measureOpen=false;
    browsingRoutes=false;selected=null;selectedEdge=null;sky?.select(null);sky?.selectEdge(null);personal.hidden=true;
    if(route){route=null;routeStepNode=null;shell.dataset.journey='false';persistView();}
    columnOpen=desktop();reflect();frameSky();
  }
  function showRouteLevel(){browsingRoutes=false;selected=routeStepNode;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);personal.hidden=true;columnOpen=true;reflect();frameSky();}
  function returnToStep(){browsingRoutes=false;selected=routeStepNode;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);personal.hidden=true;columnOpen=true;reflect();if(selected)sky?.reveal(selected);keepInColumn(deck.querySelector('.ps-step[data-current="true"]'));}
  function renderOrientation(){
    const lensTitleText=lens!=='tree'?tr(lenses.find(item=>item.id===lens)?.title,locale()):'';
    kicker.textContent=lensTitleText;kicker.hidden=!lensTitleText;
    titleButton.textContent=publicTitle();setHint(titleButton,t('toOverview'));
    crumbList.replaceChildren();
    const add=(text,action)=>{const item=el('li','ps-crumb-item');if(action){const b=button(text,'ps-crumb');b.addEventListener('click',action);item.append(b);}else{const current=el('span','ps-crumb ps-crumb-current',text);current.setAttribute('aria-current','location');item.append(current);}crumbList.append(item);};
    const mode=columnMode(),item=currentRouteItem();
    if(item){
      add(`${routeWord(item)} ${quote(tr(shownRoute(item)?.title,locale()))}`,showRouteLevel);
      add(`${t('stepName')} ${routeIndex+1}`,mode==='route'?null:returnToStep);
    }
    if(mode==='node'){const node=graphState().nodes.find(value=>value.id===selected),material=matForNode(node)??previewMaterials.get(String(selected??'').replace(/^material:/,''));add(label(node?.ownerType==='personal-hypothesis'?node:material??node)||t('selected'),null);}
    // A trace line is named by its concept; a link of the graph by its kind.
    if(mode==='edge'){const link=traceLinks.get(selectedEdge),edge=link?null:findEdge(selectedEdge);add(link?`${t('traceLabel')} ${quote(traceName(link.concept))}`:edge?`${t('transition')} · ${relationName(edge)}`:t('transition'),null);}
    titleButton.toggleAttribute('aria-current',!item&&mode==='overview');if(!item&&mode==='overview')titleButton.setAttribute('aria-current','location');
  }
  // ------------------------------------------------------------ routes inside the column
  const voiceCache=new Map();
  function routeVoicesFor(item){
    const key=`${item.source}:${item.id}:${locale()}`;if(voiceCache.has(key))return voiceCache.get(key);
    const voices=routeVoices(shownRoute(item),{references:atlas.references,materials,previewMaterials,entryByLegacyId:collectionByLegacyId,locale:locale()});voiceCache.set(key,voices);return voices;
  }
  function stepWitnesses(step){
    const stepMaterial=materials.get(step?.nodeId)??previewMaterials.get(step?.nodeId);
    // A step names its author, work and place: an Antiquity move by its passage, any other step by its own grounds.
    return profile==='antiquity'?[sourceLine(stepMaterial,{materials:previewMaterials,entryByLegacyId:collectionByLegacyId,locale:locale()})].filter(Boolean):stepWitness(step,{references:atlas.references,locale:locale()});
  }
  function transitionAfter(current,index){
    const steps=current?.steps??[],step=steps[index],next=steps[index+1];if(!step||!next)return null;
    const transition=(current.transitions??[]).find(item=>(item.fromNodeId===step.nodeId&&item.toNodeId===next.nodeId)||(item.from===step.nodeId&&item.to===next.nodeId))??(profile==='zarathustra'?(current.transitions??[])[index]:null);
    if(!transition)return null;
    // A route may walk an atlas link against its direction, so the line states the link as authored.
    const link=transition.kind?null:atlasEdgeFor(transition.edgeId),kind=transition.kind??link?.kind;
    const course=link?`${label(materials.get(link.from))||link.from} → ${label(materials.get(link.to))||link.to}`:[tr(transition.label,locale()),tr(next.title,locale())].filter(Boolean).join(' → ');
    return {kind,course,body:transition.body,link};
  }
  function turnRow(turn,expanded){
    const row=el('div','ps-turn');
    if(turn.kind&&relationWord(turn.kind)){const style=styleOf(turn.kind),word=relationWord(turn.kind),line=el('p','ps-turn-line');row.style.setProperty('--relation-color',style.color);row.style.setProperty('--relation-weight',String((style.weight??1)*1.4));line.dataset.dash=style.dash;line.append(el('i'),el('b','',word),el('span','',turn.course));setHint(line,[style.hint?tr(style.hint,locale()):'',humanEdgeLabel(turn.link)].filter(Boolean).join(' · '));row.append(line);}
    else if(turn.course)row.append(el('p','ps-turn-line ps-turn-plain',turn.course));
    if(expanded&&turn.body)fragments.render(row,turn.body,{className:'ps-turn-body'});
    return row;
  }
  function stepSourceAction(step,witnessCount){
    // A step that shows its places carries the way into the whole work with them.
    if(citesOf(step?.body).length)return null;
    const groundCites=(step?.grounds??[]).flatMap(ground=>citesOf(ground));
    if(groundCites.length){const box=el('div','ps-step-places');for(const cite of groundCites)box.append(fragments.fragment(cite));return box;}
    const source=routeSource(step);if(!source)return null;
    if(source.binding){const open=button(`↗ ${sourceActionLabel(source)}`,'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));return open;}
    const language={de:{ru:'немецкий',en:'German'},en:{ru:'английский',en:'English'},fr:{ru:'французский',en:'French'},grc:{ru:'древнегреческий',en:'Ancient Greek'},la:{ru:'латинский',en:'Latin'}}[source.reference?.language];
    const title=[tr(source.reference?.author,locale()),tr(source.reference?.work,locale()),tr(source.reference?.locator,locale()),language&&tr(language,locale())].filter(Boolean).join(' · ');
    if(source.externalUrl){const link=el('a','text-button',witnessCount===1?`↗ ${t('openExternal')}`:`↗ ${t('openExternal')} · ${title}`);setHint(link,title);link.href=source.externalUrl;link.target='_blank';link.rel='noopener noreferrer';return link;}
    return el('p','muted',`${t('bibliographicOnly')}${title?` · ${title}`:''}`);
  }
  function stepNav(index,total,go){
    const nav=el('div','ps-step-nav');
    if(index>0){const previous=button(`← ${t('routePrevious')}`,'text-button');previous.addEventListener('click',()=>go(index-1));nav.append(previous);}
    if(index<total-1){const next=button(`${t('routeNext')} →`,'primary');next.addEventListener('click',()=>go(index+1));nav.append(next);}
    return nav;
  }
  function stepHead(index,title,witness,current,go){
    const head=button('','ps-step-head');head.setAttribute('aria-current',String(current));head.setAttribute('aria-label',`${t('stepName')} ${index+1}: ${title}`);
    head.append(el('span','ps-step-number',String(index+1)),el('span','ps-step-title',title));if(witness)head.append(el('span','ps-step-witness',witness));
    head.addEventListener('click',()=>go(index));return head;
  }
  function renderRoutes(){
    if(!deck)return;for(const detail of deck.querySelectorAll('details[data-route-detail]'))routeDetailState.set(detail.dataset.routeDetail,detail.open);
    // A card that is drawn again no longer hears the pointer leave; its lit way goes out with it.
    const mode=columnMode();deck.replaceChildren();sky?.light(null);deck.dataset.active=String(!!route);deck.dataset.mode=mode;
    if(mode==='overview'){deck.hidden=false;renderRouteList();return;}
    if(!route){deck.hidden=true;return;}
    deck.hidden=false;
    if(mode==='route'){if(route.source==='atlas')renderRouteWalk();else renderJourneyWalk();return;}
    renderRouteStrip();
  }
  function renderRouteList(){
    // Authored branches or routes first; reading routes from the corpus manifest follow as their own group.
    const groups=[[t('routes'),routeItems().filter(item=>item.source==='atlas')],[t('journeys'),routeItems().filter(item=>item.source==='manifest')]];
    groups.forEach(([title,items],index)=>{if(!items.length&&index)return;if(index)deck.append(el('h2','minor-title ps-list-title',`${title} · ${items.length}`));renderRouteCards(items);});
  }
  function renderRouteCards(items){
    const list=el('div','ps-route-list');
    for(const item of items){
      const shown=shownRoute(item);if(!shown)continue;
      const card=button('','ps-route-card');if(route&&route.source===item.source&&route.id===item.id)card.setAttribute('aria-current','true');
      card.append(el('span','ps-card-kicker',`${routeWord(item)} · ${routeSize(item,shown.steps?.length??0)}`),el('strong','',tr(shown.title,locale())));
      const question=tr(shown.question??shown.description,locale());if(question)card.append(el('small','',question));
      const voices=routeVoicesFor(item);if(voices.length)card.append(el('span','ps-card-voices',voices.length>4?`${voices.slice(0,4).join(' · ')} …`:voices.join(' · ')));
      card.addEventListener('click',()=>{sky?.light(null);startRoute(item);});list.append(card);
      // Pointed at, a branch or route lights its way through the stars it walks.
      if(item.source==='atlas'){
        const light=()=>{const view=new Set((shownView?.nodes??[]).map(node=>node.id));sky?.light(routeStars(shown.steps??[],{graphIdOf:id=>graphIdForMaterial(materials.has(id)?id:String(id).replace(/^material:/,'')),shown:view}));};
        const dark=()=>sky?.light(null);
        card.addEventListener('pointerenter',light);card.addEventListener('focus',light);card.addEventListener('pointerleave',dark);card.addEventListener('blur',dark);
      }
    }
    if(!list.childElementCount)list.append(el('p','ps-route-empty',t('noRoutes')));
    deck.append(list);
  }
  // A branch reads top to bottom: every move with its witness, and between moves the kind of transition.
  function renderRouteWalk(){
    const current=routes.find(item=>item.id===route.id),steps=current?.steps??[],item={...current,source:'atlas'};
    const head=el('div','ps-walk-head');head.append(el('span','ps-card-kicker',`${routeWord(item)} · ${routeSize(item,steps.length)}`),el('h2','ps-route-title',tr(current?.title,locale())));
    if(current?.question)head.append(el('p','ps-route-question',tr(current.question,locale())));
    const voices=routeVoicesFor(item);if(voices.length)head.append(el('p','ps-card-voices',voices.join(' · ')));
    deck.append(head);
    const list=el('ol','ps-steps');
    steps.forEach((step,index)=>{
      const isCurrent=index===routeIndex,witnesses=stepWitnesses(step),entry=el('li','ps-step');entry.dataset.current=String(isCurrent);
      entry.append(stepHead(index,tr(step.title,locale()),witnesses[0],isCurrent,goRouteStep));
      if(isCurrent){
        const body=el('div','ps-step-body');for(const witness of witnesses.slice(1))body.append(el('p','ps-source-line',witness));
        fragments.render(body,step.body,{className:'ps-step-text'});if(step.question)inquiryQuestion(body,step.question);
        if(step.question){const raw=step.nodeId??step.graphNodeId,stepMaterial=raw&&(materials.has(raw)?raw:raw.replace(/^material:/,''));if(stepMaterial&&materials.has(stepMaterial))ownReadingBlock(body,stepMaterial);}
        const action=stepSourceAction(step,witnesses.length);if(action?.classList.contains('ps-step-places'))body.append(action);else if(action){const row=el('div','ps-route-actions');row.append(action);body.append(row);}
        entry.append(body);
      }
      const turn=transitionAfter(current,index);if(turn)entry.append(turnRow(turn,isCurrent));
      if(isCurrent)entry.append(stepNav(index,steps.length,goRouteStep));
      list.append(entry);
    });
    deck.append(list);
    if(current?.conclusion){const key=`atlas:${route.id}:conclusion`,end=inquiryDetail(deck,t('routeEnd'),current.conclusion,routeDetailState.get(key)??routeIndex===steps.length-1);if(end)end.dataset.routeDetail=key;}
  }
  function renderJourneyWalk(){
    const current=localizePublicJourney(journeyList.find(item=>item.id===route.id),locale()),steps=current?.steps??[],item={...current,source:'manifest'};
    const head=el('div','ps-walk-head');head.append(el('span','ps-card-kicker',`${routeWord(item)} · ${routeSize(item,steps.length)}`),el('h2','ps-route-title',current?.title??''));
    if(current?.question)head.append(el('p','ps-route-question',current.question));deck.append(head);
    const list=el('ol','ps-steps');
    steps.forEach((step,index)=>{
      const isCurrent=index===routeIndex,entry=el('li','ps-step');entry.dataset.current=String(isCurrent);
      entry.append(stepHead(index,step.title??'',step.voice?`${t('voice')}: ${step.voice}`:'',isCurrent,goJourneyStep));
      if(isCurrent){
        const body=el('div','ps-step-body');body.append(el('p','ps-step-text',step.prompt??''));
        const row=el('div','ps-route-actions'),open=button(`↗ ${t('openExact')}`,'text-button');open.addEventListener('click',()=>goJourneyStep(index,true));row.append(open);body.append(row);
        entry.append(body,stepNav(index,steps.length,goJourneyStep));
      }
      list.append(entry);
    });
    deck.append(list);
    for(const [field,key] of [['readingHypothesis','argument'],['strongAlternative','counterReading']])if(current?.[field]){const stateKey=`${route.source}:${route.id}:${field}`;const detail=inquiryDetail(deck,t(key),current[field],routeDetailState.get(stateKey)??false);if(detail)detail.dataset.routeDetail=stateKey;}
  }
  // Away from the step, the branch stays one line above the chosen star, with the way back.
  function renderRouteStrip(){
    const item=currentRouteItem();if(!item){deck.hidden=true;return;}
    const shown=shownRoute(item),back=button('','ps-route-strip');
    back.append(el('span','ps-card-kicker',`${routeWord(item)} · ${t('stepName')} ${routeIndex+1} / ${shown?.steps?.length??0}`),el('strong','',tr(shown?.title,locale())),el('span','ps-strip-return',`← ${t('backToRoute')}`));
    back.addEventListener('click',()=>returnToStep());deck.append(back);
  }
  // The star behind a step keeps its own reading, links and sources, folded beneath the branch.
  let stepNodeOpen=false;
  function renderStepNode(){
    panelContent.replaceChildren();if(!routeStepNode)return;
    const node=graphState().nodes.find(item=>item.id===routeStepNode);if(!node)return;
    // A step that asks already holds the reader's own line; its star does not ask again beneath it.
    const step=route?.source==='atlas'?routes.find(item=>item.id===route.id)?.steps?.[routeIndex]:null;
    renderNode(routeStepNode,{own:!step?.question});
    const parts=[...panelContent.childNodes],fold=document.createElement('details');fold.className='ps-step-node';fold.open=stepNodeOpen;
    fold.addEventListener('toggle',()=>{stepNodeOpen=fold.open;});
    fold.append(el('summary','',`${t('onMap')} · ${label(node.ownerType==='personal-hypothesis'?node:matForNode(node)??node)}`),...parts);
    const title=fold.querySelector('h1');if(title){const smaller=el('h3','ps-step-node-title',title.textContent);title.replaceWith(smaller);}
    panelContent.replaceChildren(fold);
  }
  function syncInitialRouteDock(){columnOpen=desktop();renderColumn();sky?.refresh();}
  function startRoute(item){measureOpen=false;
    routeDockTouched=true;browsingRoutes=false;personal.hidden=true;columnOpen=true;
    route={id:item.id,source:item.source,path:null};routeIndex=0;routeStepNode=null;selectedEdge=null;sky?.selectEdge(null);
    if(item.source==='atlas'){
      const ids=(item.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(Boolean);
      const materialIds=ids.map(id=>materials.has(id)?id:id.replace(/^material:/,''));
      const current=graphState(),nodeIds=materialIds.map(graphIdForMaterial).filter(id=>current.nodes.some(node=>node.id===id));
      route.path=pathThrough(nodeIds,current);if(nodeIds[0]){selected=nodeIds[0];routeStepNode=nodeIds[0];sky?.select(nodeIds[0]);}
    }
    shell.dataset.journey='true';refreshSky();
    if(item.source==='manifest')goJourneyStep(0,false);else if(item.steps?.[0])goRouteStep(0);else reflect();
    reading.scrollTop=0;frameSky();
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
    // The whole branch is already framed; a step moves the highlight, and the map pans only if the step is out of sight.
    if(materialId){selected=graphIdForMaterial(materialId);routeStepNode=selected;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);}
    columnOpen=true;reflect();if(selected)sky?.reveal(selected);
    if(moveFocus)deck.querySelector('.ps-step-head[aria-current="true"]')?.focus();
    keepInColumn(deck.querySelector('.ps-step[data-current="true"]'));
  }
  function goJourneyStep(index,open=false){
    const moveFocus=!!document.activeElement?.closest('.ps-route-dock');browsingRoutes=false;
    const item=localizePublicJourney(journeyList.find(value=>value.id===route?.id),locale()),step=item?.steps?.[index];if(!step)return;routeIndex=index;persistView();
    const address={documentId:step.documentId,versionId:step.versionId,unitId:step.unitId};
    const binding=[...sourceMap.values()].find(value=>value.documentId===address.documentId&&value.versionId===address.versionId&&value.unitId===address.unitId);
    const node=binding&&[...sourceMap.entries()].find(([,value])=>value===binding)?.[0];
    routeStepNode=node?graphIdForMaterial(node):null;selected=routeStepNode;selectedEdge=null;sky?.select(selected);sky?.selectEdge(null);
    columnOpen=true;reflect();if(selected)sky?.reveal(selected);
    if(open)void openPublicAddress(address).catch(()=>notify(t('unavailable')));else if(moveFocus)deck.querySelector('.ps-step-head[aria-current="true"]')?.focus();
    keepInColumn(deck.querySelector('.ps-step[data-current="true"]'));
  }
  function sourceForRef(ref){
    for(const [id,value] of sourceMap)if(value.referenceId===ref||value.unitId===ref)return value;
    return null;
  }
  function openCollection(){
    if(!collectionReady)void loadMoreCollection().then(()=>{if(!destroyed&&dialog.open&&collectionRoot?.isConnected)collectionView?.refresh();}).catch(()=>{});
    if(dialog.open)dialog.close();dialog.replaceChildren();collectionRoot=makeDialog(t('library'));collectionRoot.classList.add('ps-collection');
    const collectionHeader=dialog.querySelector('.dialog-top');collectionHeader.insertBefore(localeButtonContainer(),collectionHeader.lastChild);
    const entries=Array.isArray(library.space?.collection)&&library.space.collection.length?library.space.collection:(library.nodes??[]);
    collectionView=mountCollectionView({root:collectionRoot,entries,locale,
      onReadCatalog:()=>{dialog.close();void api.openLibrary().catch(()=>notify(t('unavailable')));},
      hasMaterial:id=>materials.has(id),materialInGraph:id=>state.nodes.some(node=>node.materialId===id),
      renderAuthored:(parent,value,options)=>fragments.render(parent,value,options),
      onOpenMaterial:id=>{try{const graphId=model.addMaterial(id);checkGraphPersistence();state=model.getState();dialog.close();selectActual(graphId,{reveal:true});reflect();}catch(error){notify(displayPublicError(error,locale));}},
      onOpenSource:item=>{const material=materials.get(item.materialId??item.id)??item;const source=sourceForMaterial(material,{sourceNodeId:item.sourceNodeId??material.sourceNodeId});if(!source)return null;
        const wrap=el('div','ps-collection-source');const open=button(`↗ ${sourceActionLabel(source)}`,'primary');open.addEventListener('click',()=>{dialog.close();void openPublicAddress(source.binding).catch(()=>notify(t('unavailable')));});wrap.append(open);return wrap;}
    });dialog.showModal();
  }
  function openGrow(){
    dialog.replaceChildren();const content=makeDialog(t('growTitle'));if(!(atlas.extensions??[]).length)content.append(el('p','ps-dialog-intro',t('growEmpty')));
    const list=el('div','ps-branch-list');for(const branch of atlas.extensions??[]){const b=button('','ps-branch');b.append(el('strong','',tr(branch.title,locale())),el('span','',tr(branch.subtitle,locale())),el('small','',countText(branch.nodeIds?.length??0,'nodes')));b.addEventListener('click',()=>{model.growAtlas({nodeIds:branch.nodeIds??[],...(branch.edgeIds?{edgeIds:branch.edgeIds}:{})});dialog.close();notifyGraphResult(t('saved'));reflect();frameSky();});list.append(b);}content.append(list,button(`＋ ${t('createBranch')}`,'primary'));content.lastChild.addEventListener('click',openNewThought);dialog.showModal();
  }
  function openNewThought(){
    if(dialog.open)dialog.close();dialog.replaceChildren();const content=makeDialog(t('newNode'));const form=document.createElement('form');const kind=document.createElement('select');for(const value of ['concept','interpretation','question','note'])kind.append(new Option(tr({concept:{ru:'Понятие',en:'Concept'},interpretation:{ru:'Прочтение',en:'Reading'},question:{ru:'Вопрос',en:'Question'},note:{ru:'Заметка',en:'Note'}}[value],locale()),value));
    const title=document.createElement('input');title.maxLength=250;title.required=true;const body=document.createElement('textarea');body.rows=5;body.maxLength=4000;body.required=true;
    form.append(field(t('kind'),kind),field(t('title'),title),field(t('text'),body));const actions=el('div','dialog-actions');actions.append(button(t('cancel')));actions.firstChild.addEventListener('click',()=>dialog.close());const save=document.createElement('button');save.type='submit';save.className='primary';save.textContent=t('save');actions.append(save);form.append(actions);content.append(form);
    form.addEventListener('submit',event=>{event.preventDefault();try{const input={id:'',kind:kind.value,title:title.value.trim(),body:body.value.trim()},id=model.addDraft({kind:kind.value,title:input.title,body:input.body});input.id=`space:${profile}:${crypto.randomUUID()}`;bindHypothesis(input.id,id);saveOwnerBindings();const managed={input,ownerSynced:false};managedDrafts.set(id,managed);try{workspace?.addHypothesis?.(input);managed.ownerSynced=true;notifyGraphResult(t('saved'));}catch{notify(model.persistenceError?.()?t('graphSaveError'):t('saveError'));}personalState=workspace?.getState?.()??personalState;selected=id;selectedEdge=null;dialog.close();reflect();frameSky();}catch(error){notify(displayPublicError(error,locale));}});dialog.showModal();title.focus();
  }
  function field(label,input){const wrapper=el('label','ps-field');wrapper.append(el('span','',label),input);return wrapper;}
  function beginConnect(first){compareBar.hidden=true;interaction='connect';connectIds=first?[first]:[];compareIds=[];closePanel();renderInteraction();}
  function openRelationDialog(){
    dialog.replaceChildren();const from=graphState().nodes.find(item=>item.id===connectIds[0]),to=graphState().nodes.find(item=>item.id===connectIds[1]);const content=makeDialog(t('newEdge'));content.append(el('p','ps-dialog-intro',`${label(from?.ownerType==='personal-hypothesis'?from:matForNode(from)??from)} → ${label(to?.ownerType==='personal-hypothesis'?to:matForNode(to)??to)}`));const form=document.createElement('form'),kind=document.createElement('select');
    for(const key of relationKinds.filter(key=>key!=='contains'))kind.append(new Option(relationWord(key),key));kind.value='relates';const reason=document.createElement('textarea');reason.rows=3;reason.maxLength=500;reason.required=true;form.append(field(t('chooseRelation'),kind),field(t('edgeLabel'),reason));const actions=el('div','dialog-actions');const cancel=button(t('cancel'));cancel.addEventListener('click',()=>{connectIds=[];dialog.close();});const save=document.createElement('button');save.type='submit';save.className='primary';save.textContent=t('save');actions.append(cancel,save);form.append(actions);content.append(form);
    form.addEventListener('submit',event=>{event.preventDefault();try{const edgeId=connectPersonalNodes(connectIds[0],connectIds[1],kind.value,reason.value.trim());connectIds=[];dialog.close();selectEdge(edgeId);notifyGraphResult(t('connected'));}catch(error){notify(displayPublicError(error,locale));}});dialog.showModal();reason.focus();
  }
  function beginCompare(first,second){compareBar.hidden=true;interaction='compare';connectIds=[];compareIds=[first,second].filter(Boolean);if(compareIds.length>=2){interaction=null;renderInteraction();renderComparison();return;}closePanel();refreshSky();frameSky();renderInteraction();}
  function renderComparison(){
    if(compareIds.length<2)return;const values=compareIds.map(id=>graphState().nodes.find(node=>node.id===id));compareBar.replaceChildren();compareBar.hidden=false;
    const close=button('×','icon');close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>{compareIds=[];compareBar.hidden=true;refreshSky();renderColumn();});compareBar.append(close,el('h2','',t('comparison')));const columns=el('div','ps-compare-columns');
    for(const node of values){const material=matForNode(node),card=el('article','ps-compare-card');card.append(el('h3','',label(node.ownerType==='personal-hypothesis'?node:material??node)),el('p','body',tr(node.body??material?.atlasBody??material?.body,locale())));const source=sourceForMaterial(material,node);if(source){const open=button(sourceActionLabel(source),'text-button');open.addEventListener('click',()=>void openPublicAddress(source.binding).catch(()=>notify(t('unavailable'))));card.append(open);}else if(material?.sourceAccess==='metadata-only')card.append(el('p','ps-source-status',t('bibliographicOnly')));if(node.ownerType==='personal-hypothesis')appendPersonalProposals(card,personalProposals(node.workspaceHypothesisId));columns.append(card);}
    compareBar.append(columns);personal.hidden=true;if(!desktop())columnOpen=false;renderColumn();sky?.refresh();
  }
  function toggleRelations(){
    dialog.replaceChildren();const content=makeDialog(t('filter'));for(const key of relationKinds){const value=styleOf(key),row=el('label','ps-relation-filter'),input=document.createElement('input');input.type='checkbox';input.checked=enabled.has(key);input.addEventListener('change',()=>{if(input.checked)enabled.add(key);else enabled.delete(key);reflect();});row.append(input,el('i',''));row.lastChild.style.background=value.color;row.lastChild.style.setProperty('--relation-color',value.color);row.lastChild.dataset.dash=value.dash??'solid';row.append(el('span','',value.hint?`${tr(value,locale())} — ${tr(value.hint,locale())}`:tr(value,locale())));content.append(row);}content.append(el('p','muted',t('imagine')));dialog.showModal();
  }
  function makeDialog(title){const head=el('div','dialog-top'),close=button('×','icon');close.addEventListener('click',()=>dialog.close());close.setAttribute('aria-label',t('close'));head.append(el('h2','',title),close);dialog.append(head);const content=el('div','dialog-content');dialog.append(content);return content;}
  function renderSearch(){
    if(!searchResults)return;searchResults.replaceChildren();const query=searchValue.toLocaleLowerCase();if(!query){searchResults.hidden=true;return;}
    // Names first: a branch called by its own name leads the results.
    const current=graphState(),options={locale:locale()};
    const found=[...previewMaterials.values()].map(item=>({score:searchScore(item,searchValue,{...options,authorTitle:tr(entryByLegacyId.get(item.author)?.title,locale())}),kind:partName(profile,item,locale(),{moveIds})||kindName(item.kind),title:label(item),body:tr(item.atlasBody??item.body,locale()),open:()=>previewMaterial(item.id)}));
    // Routes and their steps are found by their own names too, wherever the library does not already hold them as materials.
    const held=new Set([...previewMaterials.values()].map(item=>item.route?.id).filter(Boolean));
    for(const item of routeItems()){
      const shown=item.source==='manifest'?localizePublicJourney(item,locale()):item;if(!shown||item.source==='atlas'&&held.has(item.id))continue;
      const name=tr(shown.title,locale());
      found.push({score:searchScore({title:shown.title,question:shown.question,body:shown.description,route:shown},searchValue,options),kind:`${routeWord(item)} · ${routeSize(item,shown.steps?.length??0)}`,title:name,body:tr(shown.question??shown.description,locale()),open:()=>{searchResults.hidden=true;search.blur();startRoute(item);}});
      if(item.source!=='atlas')continue;
      (shown.steps??[]).forEach((step,index)=>{const material=materials.get(step.nodeId);if(material&&label(material)===tr(step.title,locale()))return;
        found.push({score:searchScore({title:step.title,question:step.question,body:step.body},searchValue,options),kind:`${routeWord(item)} · ${t('stepName')} ${index+1}`,title:tr(step.title,locale()),body:name,open:()=>{searchResults.hidden=true;search.blur();startRoute(item);goRouteStep(index);}});});
    }
    const available=found.map((entry,index)=>({...entry,index})).filter(entry=>entry.score>0).sort((a,b)=>b.score-a.score||a.index-b.index).slice(0,24);
    for(const item of available){const b=button('','ps-result');b.append(el('small','',item.kind),el('strong','',item.title),el('span','',String(item.body??'').slice(0,140)));b.addEventListener('click',item.open);searchResults.append(b);}
    for(const item of current.nodes.filter(node=>node.ownerType==='personal-hypothesis'&&`${node.title} ${node.body}`.toLocaleLowerCase().includes(query))){const b=button('','ps-result');b.append(el('small','',t('personal')),el('strong','',item.title),el('span','',item.body.slice(0,140)));b.addEventListener('click',()=>{searchResults.hidden=true;search.blur();selectActual(item.id);});searchResults.append(b);}
    if(!searchResults.childElementCount)searchResults.append(el('p','ps-no-results',t('emptySearch')));searchResults.hidden=false;
  }
  function renderPersonalPanel(){
    if(personal.hidden)return;personalContent.querySelector('.ps-personal-summary')?.remove();const summary=el('p','ps-personal-summary',(copyLocale(locale())==='en')?localeText("Personal thoughts: {0} · proposals: {1}",locale,[personalState?.hypotheses?.length??0,personalState?.proposals?.length??0]):`Личных мыслей: ${personalState?.hypotheses?.length??0} · предложений: ${personalState?.proposals?.length??0}`);personalContent.prepend(summary);
  }
  function toggleFullscreen(){if(document.fullscreenElement)return document.exitFullscreen().catch(()=>{});return shell.requestFullscreen?.().catch(()=>{});}
  function keyboard(event){
    if(dialog.open||event.target.closest('input,textarea,select,[contenteditable="true"],.ps-personal'))return;
    const key=event.key.toLowerCase();if((event.ctrlKey||event.metaKey)&&key==='z'){event.preventDefault();event.shiftKey?model.redo():model.undo();checkGraphPersistence();}
    else if(!event.ctrlKey&&!event.metaKey&&!event.altKey){
      if(key==='o')frameSky();
      if(key==='/'){event.preventDefault();search.focus();}
      if(key==='+'||key==='='){event.preventDefault();sky?.zoomBy(1.25);}
      if(key==='-'||key==='_'){event.preventDefault();sky?.zoomBy(1/1.25);}
      // Arrows walk the open branch or route a step at a time.
      const arrow={arrowleft:[-1,0],arrowright:[1,0],arrowup:[0,-1],arrowdown:[0,1]}[key];
      if(arrow&&!measuredLens(lens)&&(!route||event.shiftKey)&&(event.target===document.body?mapActive:event.target.closest('#tree .tree-stars,#tree canvas,#tree .ps-map-tools'))){event.preventDefault();sky?.turnBy?.(arrow[0]*.12,arrow[1]*.09);return;}
      if(route&&!event.shiftKey&&(key==='arrowleft'||key==='arrowright')&&!event.target.closest('.ps-route-progress')){const next=routeIndex+(key==='arrowright'?1:-1),total=route.source==='atlas'?routes.find(item=>item.id===route.id)?.steps?.length??0:journeyList.find(item=>item.id===route.id)?.steps?.length??0;if(next>=0&&next<total){event.preventDefault();route.source==='atlas'?goRouteStep(next):goJourneyStep(next);}}
      // Escape climbs one level at a time: a pending pick, a comparison, an open panel, then the route.
      if(key==='escape'){if(!searchResults.hidden){searchResults.hidden=true;return;}if(interaction){cancelInteraction();return;}if(!compareBar.hidden){compareIds=[];compareBar.hidden=true;refreshSky();renderColumn();return;}if(!personal.hidden){closePersonal();return;}closeColumnLayer({collapse:false});}
    }
  }
  function restoreFocus(){
    const target=!reading.hidden?(deck.querySelector('.ps-step-head[aria-current="true"]')??panelContent.querySelector('button,a[href]')??reading.querySelector('button,a[href]')):mainActions.querySelector('[data-space-action="routes"]');
    if(!target?.isConnected)return false;target.focus({preventScroll:true});return document.activeElement===target;
  }
  let collectionView=null,collectionRoot=null;
  function refreshLocale(){setLocaleState();if(!personal.hidden)renderPersonalPanel();if(dialog.open){dialog.querySelector('.dialog-top .icon')?.setAttribute('aria-label',t('close'));}if(dialog.open&&collectionRoot?.isConnected){dialog.querySelector('.dialog-top h2').textContent=t('library');for(const b of dialog.querySelectorAll('[data-public-locale]'))b.setAttribute('aria-pressed',String(b.dataset.publicLocale===locale()));collectionView?.refresh();}}
  function updateWorkspace(){personalState=workspace?.getState?.()??personalState;if(syncingOwner)return;reflect();if(!personal.hidden)renderPersonalPanel();}
  function syncManagedDrafts(next){
    const graphIds=new Set(next.nodes.map(node=>node.id)),ownerIds=new Set((workspace?.getState?.()?.hypotheses??[]).map(item=>item.id));let changed=false;syncingOwner=true;
    try{for(const [id,managed]of managedDrafts){if(!managed.ownerSynced||researchUndoneOwners.has(managed.input.id))continue;if(graphIds.has(id)&&!ownerIds.has(managed.input.id)){workspace?.addHypothesis?.(managed.input);changed=true;}else if(!graphIds.has(id)&&ownerIds.has(managed.input.id)){workspace?.removeHypothesis?.(managed.input.id);changed=true;}}}
    catch{}finally{syncingOwner=false;}
    if(changed)personalState=workspace?.getState?.()??personalState;
  }
  const unsubscribeModel=model.subscribe(next=>{state=next;syncManagedDrafts(next);reflect();});
  const unsubscribeWorkspace=workspace?.subscribe?.(updateWorkspace);
  const showImportResult=()=>{if(personal.hidden)togglePersonal();};
  workspaceRoot.addEventListener('public-research-import-result',showImportResult);
  const researchHistory=event=>{for(const id of event.detail?.removedHypothesisIds??[])researchUndoneOwners.add(id);for(const id of event.detail?.restoredHypothesisIds??[])researchUndoneOwners.delete(id);};
  workspaceRoot.addEventListener('public-research-workspace-history',researchHistory);
  if(question)question.classList.add('ps-question-input');
  searchResults.hidden=true;deck.hidden=true;personal.hidden=true;reading.hidden=true;
  paintLenses();paintRelations();desktopMode.addEventListener('change',syncInitialRouteDock);syncInitialRouteDock();
  try{if(!model.getState().nodes.length)model.seedAtlas();}catch{}
  checkGraphPersistence();
  state=model.getState();
  // The free area is what the panel, route dock, heading and toolbars leave visible.
  // On a wide screen the panel column is always reserved, so opening it never moves the map.
  function freeArea({width,height}){
    const box=shell.getBoundingClientRect(),visible=node=>node&&!node.hidden&&getComputedStyle(node).display!=='none';
    const rect=node=>{const r=node.getBoundingClientRect();return {left:r.left-box.left,right:r.right-box.left,top:r.top-box.top,bottom:r.bottom-box.top};};
    const wide=desktopMode.matches,rail=wide&&visible(lensRail)?rect(lensRail):null;
    // The Time lens keeps a strip above the map for its scale, and one at its left for the rows' names.
    const scale=measuredLens(lens);
    // A map of places names its parallels in a narrow strip at its left.
    const left=(rail?rail.right+24:16)+laneStrip()+(lens==='space'&&geography?58:0);
    // On a wide screen the open column is the map's right edge; a collapsed column gives the map its width back.
    const column=wide?[reading,personal].find(visible):null,tab=wide&&visible(columnTab)?rect(columnTab):null;
    // The map tools stand over the map, and the stars keep clear of them: on a wide screen at its right, on a narrow one below it.
    const tools=visible(mapTools)?rect(mapTools):null;
    const right=Math.min(column?rect(column).left-20:tab?tab.left-16:width-16,wide&&tools&&!scale?tools.left-14:Infinity);
    const below=(visible(heading)?rect(heading).bottom:0)+14;let top=below+(scale&&layoutStrip()?layoutStrip()+4:0);
    const floors=[height-16];
    for(const node of [footer,relationBar])if(visible(node))floors.push(rect(node).top-12);
    // On a narrow screen the scale of time ends above the map's tools, so no row runs under them.
    if(!wide&&tools)floors.push(tools.top-10);
    if(!wide)for(const node of [reading,personal,compareBar])if(visible(node))floors.push(rect(node).top-14);
    const bottom=Math.min(...floors);
    // Under an open sheet the scale's strip has no room: its stars keep below the heading and its labels wait.
    if(scale&&bottom-top<120)top=below;
    return bottom-top>=120&&right-left>=120?{left,right,top,bottom}:null;
  }
  // The space turns under a drag and rests a little turned, so its depth shows; Shift, the right button or two fingers move it,
  // a pinch or the wheel zooms, and a star held still is picked up. A measuring lens lays its scale flat and only moves.
  sky=mountConstructorSky(shell,{rest:depth?.rest??null,intro:true,holdToMove:380,turnFromStars:true,touch:true,secondaryPan:true,inertia:true,depthCue:true,planarMoves:true,nativeTitles:false,onThreadSelect:id=>followThread(id),threadsFor:id=>darkThreads(id),onEdgeHover:id=>{const label=id?shell.querySelector(`.edge-label[data-edge="${CSS.escape(id)}"]`):null;if(label)hints?.showFor(label);else hints?.hide();},onPose:pose=>paintTurn(pose),dragNodes:()=>!measuredLens(lens),getViewport:freeArea,onSelect:id=>selectNode(id,{fromMap:true}),onMove:(id,moved)=>{const node=graphState().nodes.find(item=>item.id===id),own=node?.position?.[2],position=[moved[0],moved[1],Number.isFinite(own)?own:moved[2]];if(node?.ownerType==='personal-hypothesis'){personalPositions.set(id,position);try{localStorage.setItem(positionsKey,JSON.stringify(Object.fromEntries(personalPositions)));positionsPersistenceError=false;}catch{positionsPersistenceError=true;notify(t('graphSaveError'));};}else try{model.moveNode(id,position);checkGraphPersistence();}catch{}},onEdgeSelect:selectEdge});
  // One hint answers a pointer that rests on something, or a keyboard that reaches it: a tool says what it does and its key,
  // a star its kind, when and where, a link its kind and what that kind means. Nothing of it stays on the screen by itself.
  function starHint(id){
    const node=graphState().nodes.find(item=>item.id===id);if(!node)return null;
    if(node.ownerType==='personal-hypothesis')return {eyebrow:t('rungPersonal'),title:label(node),lines:[]};
    const material=matForNode(node),part=partName(profile,material,locale(),{moveIds}),rung=ladderRung({material,node});
    const eyebrow=[part||kindName(material?.kind??node.kind),rung==='personal'?t('rungPersonal'):t(`rung_${rung}`)].filter(Boolean).join(' · ');
    const lines=[],situationId=material?.id&&chronologyReader?situationOf(material.id):null,words=situationId?datingWords(situationId):null;
    if(words)lines.push(`◷ ${words.when}${words.context?` · ${words.context}`:''}`);
    if(material?.id&&geography&&starSet.has(material.id)){const entry=placeEntryOf(material.id),place=entry?.place?placesReader.place(entry.place):null;if(place)lines.push(`⌖ ${tr(place.label,locale())}`);}
    return {eyebrow,title:keepDots(label(material??node)),lines};
  }
  function edgeHint(id){
    const edge=shownView?.edges?.find(item=>item.id===id);if(!edge||edge.proposalId)return null;
    const meaning=styleOf(edge.kind)?.hint?tr(styleOf(edge.kind).hint,locale()):'';if(!meaning)return null;
    const word=isBasedOn(edge)?relationName(edge):relationWord(edge.kind);
    return {text:word?`${word}: ${meaning}`:meaning,keys:[],note:''};
  }
  hints=mountHints(shell,{
    selector:'[data-hint],.tree-star,.edge-label,.tree-thread',
    describe:element=>element.matches('.tree-star')?starHint(element.dataset.node):element.matches('.edge-label')?edgeHint(element.dataset.edge):element.matches('.tree-thread')?threadHint(element.dataset.thread):undefined,
    side:element=>element.matches('.tree-star,.tree-thread')?'right':element.closest('.ps-map-tools')?(desktopMode.matches&&!shell.dataset.scale?'left':'bottom'):element.closest('.ps-lens-rail')&&desktopMode.matches?'right':'top',
    moving:element=>element.matches('.tree-star,.edge-label,.tree-thread'),
  });
  // The controls window opens by itself once, after the space's first turn, on a space that turns; it closes for good
  // only when asked, and comes back from the map tools.
  // controlsDone: this visit has settled whether the window opens by itself (it opened, or it is not to).
  let lastPointer=null,controlsDone=false,controlsTimer=0;
  const notePointer=event=>{lastPointer=event.pointerType==='mouse'?'mouse':'touch';};
  shell.addEventListener('pointerdown',notePointer,{capture:true});
  const storageOf=()=>{try{return window.localStorage;}catch{return null;}};
  controls=mountControls(shell,{storage:storageOf(),anchor:mapTools,locale:()=>locale(),device:()=>lastPointer??(window.matchMedia?.('(hover: none) and (pointer: coarse)').matches?'touch':'mouse')});
  // The window stands over the free part of the map, no wider or taller than it, so it covers none of the tools around.
  function placeControls(){
    const panel=controls?.element;if(!panel||panel.hidden)return;
    const area=freeArea({width:shell.clientWidth,height:shell.clientHeight})??{left:8,right:shell.clientWidth-8,top:8,bottom:shell.clientHeight-8};
    const room={width:Math.max(220,area.right-area.left),height:Math.max(160,area.bottom-area.top)};
    panel.style.width=`${Math.round(Math.min(420,room.width))}px`;panel.style.maxHeight=`${Math.round(room.height)}px`;
    const width=panel.offsetWidth,height=Math.min(panel.offsetHeight,room.height);
    panel.style.left=`${Math.round(Math.max(8,Math.min(shell.clientWidth-width-8,(area.left+area.right)/2-width/2)))}px`;
    panel.style.top=`${Math.round(Math.max(8,Math.min(shell.clientHeight-height-8,(area.top+area.bottom)/2-height/2)))}px`;
  }
  function openControlsOnce(){
    clearTimeout(controlsTimer);controlsTimer=0;
    if(destroyed||controlsDone||controls.open)return;
    const storage=storageOf();
    // A reader who asked not to see it again is settled at once, without touching storage again this visit.
    if(controlsDismissed(storage)){controlsDone=true;return;}
    // On a flat lens it waits for the first space that turns.
    if(measuredLens(lens))return;
    // Not over a press, the library, the reader or a search that has the reader's attention; it waits for them to end.
    const busy=dialog.open||!searchResults.hidden||shell.querySelector('.tree-sky.dragging')||shell.closest('[inert]')||document.querySelector('[data-corpus-reading]');
    if(busy){controlsTimer=setTimeout(openControlsOnce,1200);return;}
    controlsDone=true;
    // Where the browser keeps nothing, the window does not insist.
    if(!controlsShouldOpen({storage}))return;
    controls.show();placeControls();
  }
  const firstTurn=window.matchMedia?.('(prefers-reduced-motion: reduce)').matches?400:2700;
  controlsTimer=setTimeout(openControlsOnce,firstTurn);
  // On the scale of time a row's name opens its branch.
  const openLane=event=>{const lane=event.target.closest?.('.tree-cluster[data-ref]');if(!lane||!(lens==='time'&&timeline))return;const item=routeItems().find(entry=>entry.source==='atlas'&&entry.id===lane.dataset.ref);if(!item)return;event.stopPropagation();startRoute(item);};
  shell.addEventListener('click',openLane);
  let refitFrame=0;const refitScale=()=>{placeControls();if(!measuredLens(lens))return;cancelAnimationFrame(refitFrame);refitFrame=requestAnimationFrame(frameSky);};
  window.addEventListener('resize',refitScale);
  const viewParams=new URLSearchParams(location.search),savedRoute=viewParams.get('treeRoute'),savedStep=Number(viewParams.get('treeStep'));
  const restoredRoute=routeItems().find(item=>`${item.source}:${item.id}`===savedRoute);
  if(restoredRoute){startRoute(restoredRoute);const step=Math.max(0,Math.min((restoredRoute.steps?.length??1)-1,Number.isSafeInteger(savedStep)?savedStep-1:0));if(restoredRoute.source==='atlas')goRouteStep(step);else goJourneyStep(step);}
  else if(savedRoute)persistView();
  reflect();frameSky();shell.dataset.ready='true';
  // Once the stand has drawn, the rest of the shared library arrives in a quiet moment.
  if(moreCollection){const idle=window.requestIdleCallback?callback=>window.requestIdleCallback(callback,{timeout:5000}):callback=>setTimeout(callback,1800);idle(()=>{if(!destroyed)void loadMoreCollection().catch(()=>{});});}
  if(upgradeError)notify(copyLocale(locale)==='en'?localeText('The saved graph could not be upgraded. Its original copy is preserved.',locale):'Не удалось перенести сохранённый граф. Исходная копия сохранена.');
  const localeEvent=event=>{if(event.detail?.locale&&event.detail.locale!==locale()){};refreshLocale();};header?.addEventListener('public-space-locale',localeEvent);
  return {root:shell,pristineGraphs,openCollection,personalRoot:personalContent,workspaceRoot,agentRoot,refreshLocale,restoreFocus,refreshJourneys(items){journeyList=Array.isArray(items)?items:[];if(route)renderRoutes();},renderJourneys:renderRoutes,model,
    destroy(){if(destroyed)return;destroyed=true;window.removeEventListener('resize',refitScale);cancelAnimationFrame(refitFrame);shell.removeEventListener('click',openLane);unsubscribeModel?.();unsubscribeWorkspace?.();workspaceRoot.removeEventListener('public-research-import-result',showImportResult);workspaceRoot.removeEventListener('public-research-workspace-history',researchHistory);document.removeEventListener('keydown',keyboard);document.removeEventListener('pointerdown',notePress,{capture:true});clearTimeout(controlsTimer);shell.removeEventListener('pointerdown',notePointer,{capture:true});hints?.dispose();controls?.dispose();fragments.dispose();desktopMode.removeEventListener('change',syncInitialRouteDock);header?.removeEventListener('public-space-locale',localeEvent);sky?.dispose();model.dispose();shell.remove();}};
}
