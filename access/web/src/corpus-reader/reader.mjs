import './reader.css';
import {createNoteDraftJournal} from './note-draft.mjs';
import {
  READER_LIMITS,
  createCorpusReaderModel,
  exactReference,
  availabilityLabel,
  isTextAvailable,
  isReference,
  referenceKey,
  sameReference,
  unitReference,
} from './view-model.mjs';
import {createNotebookMetadataResolver,notebookFallbackLabel,notebookReferenceWorkId} from './notebook-labels.mjs';
import {languageName,sourceLinkLabel,corpusWorkTitle,corpusSectionLabel} from '../observatory/human-presentation.mjs';
import {bindPhilologyResult,philologyMarkIntervals,renderPhilology,selectedPhilologyForm,isSingleWordSelection} from './philology.mjs';
import {editionRepresentationKind,groupEditionRepresentations} from './edition-groups.mjs';
import {readableUnitLocator} from './location-label.mjs';
import {editorialRanges,editorialLabel,editorialKindForMatch,hasEditorialProjection,quotationWithEditorialRoles,sourceLayerKind} from './editorial.mjs';
import {selectedSourceContext} from './source-context.mjs';
import retained from '../../../public-corpus/retained-library-map.v1.json' with {type:'json'};
import variants from '../../../public-corpus/reader-textual-variants.v1.json' with {type:'json'};

const retainedApparatus=new Map(retained.books.flatMap(book=>book.versions.flatMap(version=>[
  ...(version.bookNotesUnitIds??[]).map(id=>[id,{key:'workNotes',versionId:version.versionId,revision:version.revision}]),
  ...(version.adjustmentUnitIds??[]).map(id=>[id,{key:'textualAdjustments',versionId:version.versionId,revision:version.revision}]),
])));
const apparatusLabel=(unit,locale)=>{const item=retainedApparatus.get(unit?.unitId??unit?.id),reference=unit?.reference;
  return item && reference?.versionId===item.versionId && reference?.target?.textLayerSha256===item.revision
    ? WORDING[String(locale).startsWith('en')?'en':'ru'][item.key] : '';};
// Structural section IDs can include an earlier scholarly marker. Show the
// range supplied by the retained navigation metadata without changing the
// unit's identity or claiming an offset for an individual marker.
const retainedEarlierMarkers=new Map(retained.books.flatMap(book=>book.versions.flatMap(version=>
  version.structure.flatMap(section=>{
    const first=section.markers?.[0],last=section.markers?.at(-1);
    const sectionNumber=/^\d+$/.test(section.sectionId??'')?Number(section.sectionId):null;
    const firstNumber=typeof first==='string'?Number(first.match(/^\d+/)?.[0]):NaN;
    return sectionNumber!==null && Number.isFinite(firstNumber) && firstNumber<sectionNumber && last
      ? [[`${version.versionId}\u0000${version.revision}\u0000${section.unitId}`,`${first}–${last}`]] : [];
  }))));
const earlierMarkerRange=(unit,versionId)=>retainedEarlierMarkers.get(`${versionId}\u0000${unit?.reference?.target?.textLayerSha256}\u0000${unit?.unitId??unit?.id}`) || '';
const withMarkerRange=(label,unit,versionId)=>{const range=earlierMarkerRange(unit,versionId);return range?`${label} (${range})`:label;};

const WORDING = {
  ru: {
    workNotes:'Примечания к произведению',textualAdjustments:'Текстологические сведения',
    reading: 'Чтение', close: 'Вернуться к Древу', library: 'Корпус', find: 'Найти произведение', loadMore: 'Ещё',
    previous: 'Предыдущий фрагмент', next: 'Следующий фрагмент', one: 'Один текст', parallel: 'Сравнить тексты', compare: 'Сравнить с',
    search: 'Поиск', searchPlaceholder: 'Искать в открытом тексте', loaded: 'Загруженный фрагмент', version: 'Всё произведение',
    corpus: 'Весь корпус', noSearch: 'Совпадений нет', searchUnavailable: 'Поиск недоступен для этого источника',
    notes: 'Блокнот', context: 'Контекст', source: 'Источник', word: 'Слово', settings: 'Вид текста', bookmark: 'Закладка', bookmarked: 'Закладка снята',
    addNote: 'Оставить заметку', notePlaceholder: 'Что открылось здесь? Вопрос, связь, возражение…', save: 'Сохранить', delete: 'Удалить',
    copy: 'Скопировать цитату', copied: 'Цитата и атрибуция скопированы', graph: 'Открыть в Древе', export: 'Скачать блокнот', import: 'Импортировать блокнот',
    noNotes: 'Выберите фрагмент, чтобы оставить заметку или закладку.', saved: 'Сохранено на этом устройстве', pending: 'Сохраняется…', memory: 'Только в памяти', session: 'Только на время сеанса', saveError: 'Не удалось сохранить', providerUnavailable: 'Источник полных текстов пока не подключён.',
    changed: 'Версия источника изменилась. Старая закладка сохранена.', unavailable: 'Полный текст недоступен',
    changedCurrent: 'Сохранённая привязка относится к другой версии текста.', unavailableCurrent: 'Этот фрагмент больше недоступен.', openCurrent: 'Открыть текущую версию',
    empty: 'В этой области пока нет текста.', loading: 'Загрузка…', error: 'Не удалось загрузить фрагмент',
    previousChunk: 'Назад', nextChunk: 'Дальше', edition: 'Издание', provenance: 'Происхождение текста', language: 'Язык', original: 'Оригинал', translation: 'Перевод',
    unknown: 'не указано', continuation: 'Есть продолжение', metadataOnly: 'Только метаданные', restricted: 'Доступ ограничен', unavailableVersion: 'Текст недоступен', availabilityUnknown: 'Доступность уточняется',
    untitled: 'Без названия', units: 'Единиц', passage: 'Фрагмент', characters: 'символов', location: 'Место',
    editionVersions: 'текстовых представления', sourceTextLayer: 'Текст из издания', retainedTextLayer: 'Сохранённый слой · старые привязки сохранены',
    selectedEdition: 'выбрано', currentLocation: 'Текущее место', loadedPassage: 'Загруженный фрагмент', selectedPassage: 'Выбранный фрагмент', independentColumns: 'Колонки читаются независимо; соответствие мест не задано.', mobileEditions: 'Выбрать текстовое представление', jumpToText:'К тексту', searchEdition:'Искать в издании',
    font: 'Размер', leading: 'Интервал', width: 'Ширина', theme: 'Тема', paper: 'Бумага', night: 'Ночь',
    select: 'Выбрать фрагмент в тексте', noCatalog: 'Произведения не найдены', works: 'Произведений',
  },
  en: {
    workNotes:'Notes on the work',textualAdjustments:'Textual adjustments',
    reading: 'Reading', close: 'Return to the tree', library: 'Corpus', find: 'Find a work', loadMore: 'More',
    previous: 'Previous passage', next: 'Next passage', one: 'One text', parallel: 'Compare texts', compare: 'Compare with',
    search: 'Search', searchPlaceholder: 'Search the open text', loaded: 'Loaded passage', version: 'This work',
    corpus: 'Entire corpus', noSearch: 'No matches', searchUnavailable: 'Search is unavailable for this source',
    notes: 'Notebook', context: 'Context', source: 'Source', word: 'Word', settings: 'Text appearance', bookmark: 'Bookmark', bookmarked: 'Bookmark removed',
    addNote: 'Leave a note', notePlaceholder: 'What opened here? A question, connection, objection…', save: 'Save', delete: 'Delete',
    copy: 'Copy quotation', copied: 'Quotation and attribution copied', graph: 'Open in the tree', export: 'Download notebook', import: 'Import notebook',
    noNotes: 'Choose a passage to leave a note or bookmark.', saved: 'Saved on this device', pending: 'Saving…', memory: 'Memory only', session: 'This session only', saveError: 'Could not save', providerUnavailable: 'The full-text source is not connected yet.', changed: 'The source version changed. The old bookmark is kept.',
    unavailable: 'Full text unavailable', empty: 'There is no text in this area yet.', loading: 'Loading…', error: 'Could not load this passage',
    changedCurrent: 'The saved anchor belongs to another text version.', unavailableCurrent: 'This passage is no longer available.', openCurrent: 'Open the current version',
    previousChunk: 'Previous', nextChunk: 'Next', edition: 'Edition', provenance: 'Text provenance', language: 'Language', original: 'Original', translation: 'Translation',
    unknown: 'unknown', continuation: 'More pages available', metadataOnly: 'Metadata only', restricted: 'Restricted', unavailableVersion: 'Text unavailable',
    font: 'Type size', leading: 'Line spacing', width: 'Line width', theme: 'Theme', paper: 'Paper', night: 'Night',
    select: 'Select a passage in the text', noCatalog: 'No works found', works: 'Works', availabilityUnknown: 'Availability is being checked',
    untitled: 'Untitled', units: 'Units', passage: 'Passage', characters: 'characters', location: 'Location',
    editionVersions: 'saved text versions', sourceTextLayer: 'Edition source text', retainedTextLayer: 'Retained layer · older links remain valid',
    selectedEdition: 'selected', currentLocation: 'Current location', loadedPassage: 'Loaded passage', selectedPassage: 'Selected passage', independentColumns: 'Each column keeps its own reading location; no alignment is implied.', mobileEditions: 'Choose a text representation', jumpToText:'Skip to text', searchEdition:'Search edition',
  },
};

const element = (tag, className = '', value) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (value !== undefined) node.textContent = value;
  return node;
};
const button = (label, action, className = '') => {
  const node = element('button', className, label);
  node.type = 'button';
  node.addEventListener('click', action);
  return node;
};
const localized = (value, locale) => {
  if (typeof value === 'string') return value;
  if (!value || typeof value !== 'object') return '';
  const base = String(locale || 'ru').toLowerCase().split('-')[0];
  return value[locale] ?? value[base] ?? value.ru ?? value.en ?? Object.values(value).find(item => typeof item === 'string') ?? '';
};
const attr = (value, fallback = '') => typeof value === 'string' ? value : fallback;
const readableLanguage = (value, locale) => {
  if (typeof value !== 'string' || !value.trim()) return '';
  return String(languageName(value, locale));
};
const wait = (ms, callback) => { const id = setTimeout(callback, ms); return () => clearTimeout(id); };
const codePointOffset = (value, utf16Offset) => Array.from(String(value).slice(0, Math.max(0, utf16Offset))).length;
const remember=(map,key,value)=>{map.delete(key);map.set(key,value);while(map.size>64)map.delete(map.keys().next().value);};
const codePointSlice = (value, start = 0, end = undefined) => Array.from(String(value)).slice(start, end).join('');

function safeExternalUrl(value) {
  if (typeof value !== 'string' || !value.trim()) return '';
  try {
    const url = new URL(value, document.baseURI);
    return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : '';
  } catch {
    return '';
  }
}

function download(name, content, type) {
  const link = document.createElement('a');
  link.href = URL.createObjectURL(new Blob([content], {type}));
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(link.href), 1000);
}

function selectedOffsets(row, node, offset) {
  const textRoot = row.querySelector('.cr-unit-text') || row;
  if (node !== textRoot && !textRoot.contains(node)) return null;
  try {
    const range = document.createRange();
    range.selectNodeContents(textRoot);
    range.setEnd(node, offset);
    return range.toString().length;
  } catch {
    return null;
  }
}

function quoteAttribution({document, version, unit, quote, locale}) {
  const title = corpusWorkTitle(document, locale) || document?.id || '';
  const author = localized(document?.author, locale);
  const work = localized(document?.work, locale);
  const originalTitle = localized(document?.title, locale);
  const edition = localized(version?.edition, locale) || version?.label || version?.id || '';
  const locator = withMarkerRange(apparatusLabel(unit,locale) || corpusSectionLabel(readableUnitLocator(unit,locale),locale,version?.id) || localized(version?.locator || document?.locator, locale) || unit?.unitId || '',unit,version?.id);
  const source = safeExternalUrl(attr(version?.sourceUrl || version?.source_url || document?.sourceUrl));
  const licenseUrl = safeExternalUrl(attr(version?.license?.url));
  const license = licenseUrl ? `${localized(version?.license?.label, locale) || (String(locale).startsWith('en') ? 'License' : 'Лицензия')}: ${licenseUrl}` : '';
  return [quote ? `«${quote}»` : '', [author,title,work && work !== title && work !== originalTitle ? work : ''].filter(Boolean).join(' · '), edition, locator, source, license].filter(Boolean).join('\n');
}

/**
 * Mounts a corpus reader over an existing host. The host is never replaced;
 * opening and closing only toggles this one persistent reading surface.
 */
export function mountCorpusReader({
  host = document.body,
  provider,
  notebook,
  locale = () => 'ru',
  onGraphRequest,
  onClose,
  onReturnFocusFallback,
  onLocation,
  onNativeReference,
} = {}) {
  if (!host || typeof host.append !== 'function') throw new Error('reader-host-required');
  const uiLocale = () => String(typeof locale === 'function' ? locale() : locale || 'ru').toLowerCase().startsWith('en') ? 'en' : 'ru';
  let root = element('section', 'corpus-reader');
  root.hidden = true;
  root.setAttribute('role', 'dialog');
  root.setAttribute('aria-modal', 'true');
  root.setAttribute('aria-label', WORDING.ru.reading);
  root.tabIndex = -1;
  host.append(root);

  let opened = false;
  let opener = null;
  let renderSerial = 0;
  let catalogCancel = null;
  let searchCancel = null;
  let searchGeneration = 0;
  let currentTab = 'context';
  let searchOpen = false;
  let searchScope = 'loaded';
  let searchVersionId = null;
  let searchQuery = '';
  let searchResult = {items: []};
  let comparison = new Map();
  let panePositions = new Map();
  let paneSaveTimers = new Map();
  let paneSaveWork = Promise.resolve();
  let selected = null;
  let philology = {busy:false,result:null,concordance:null,error:null};
  let philologyController = null;
  let noteEditor = null;
  let noteDirty = false;
  let noteTimer = null;
  let noteFlushPromise = null;
  let draft = null;
  let selectionSequence = 0;
  let disposed = false;
  let lastDataStamp = '';
  let rendering = false;
  let lastMessage = '';
  let mode = 'single';
  let settingsOpen = false;
  let libraryOpen = window.matchMedia('(min-width: 851px)').matches;
  let inspectorOpen = false;
  let destroyWork = null;
  let renderSnapshot = null;
  let notebookListGeneration = 0;
  const draftJournal = createNoteDraftJournal({notebook});
  const recovery = Promise.resolve().then(()=>draftJournal.recover()).then(recovered=>{
    if(recovered)announce(uiLocale()==='en'?'An unfinished note was recovered as a separate note.':'Несохранённый черновик восстановлен отдельной заметкой.');
  }).catch(error=>announce(friendlyError(error)));

  const model = createCorpusReaderModel({
    provider,
    notebook,
    locale: uiLocale(),
    beforeNotebookReady: recovery,
    onChange: state => {
      if (!opened || disposed) return;
      const stamp = JSON.stringify([state.document?.id, state.activeVersionId, state.referenceStatus, state.referenceStates, state.error?.message,
        state.catalog.query, state.catalog.busy, state.catalog.nextCursor, state.catalog.items.map(item=>[item.id,item.title]),
        Object.entries(state.windows).map(([key,page])=>[key,page.contentRevision,page.revision,page.units.map(unit=>unit.id),page.next,page.previous]),
        Object.entries(state.windowErrors || {}).map(([key,error])=>[key,error?.code,error?.message]),
        Object.entries(state.searches).map(([key,page])=>[key,page.query,page.scope,page.nextCursor,page.items?.map(item=>[item.unitId,item.start,item.end])])]);
      if (stamp === lastDataStamp || noteDirty || noteFlushPromise) { updateFooter(); return; }
      lastDataStamp = stamp;
      render(state);
    },
  });
  const notebookMetadataResolver = createNotebookMetadataResolver({readDocument: documentId => model.readDocumentMetadata(documentId)});

  const t = key => WORDING[uiLocale()][key] || key;
  const modelState = () => renderSnapshot ?? model.snapshot();
  const activeDocument = () => modelState().document;
  const activeVersion = versionId => activeDocument()?.versions?.find(item => item.id === versionId) || null;
  const documentTitle = (document, aliases = []) => {
    const value = corpusWorkTitle(document, uiLocale());
    return value && ![document?.id, ...aliases].includes(value) ? value : t('untitled');
  };
  const versionLanguage = version => readableLanguage(version?.language, uiLocale());
  const unitLabel = unit => {
    const apparatus=apparatusLabel(unit,uiLocale());if(apparatus)return apparatus;
    if (typeof unit?.label === 'string' && unit.label.trim()) return withMarkerRange(corpusSectionLabel(unit.label,uiLocale(),selected?.versionId || modelState().activeVersionId),unit,selected?.versionId || modelState().activeVersionId);
    if (Number.isSafeInteger(unit?.ordinal) && unit.ordinal >= 0) return `${t('passage')} ${unit.ordinal + (unit.ordinal === 0 ? 1 : 0)}`;
    return t('passage');
  };
  const paneUnitLabel = (unit, version) => withMarkerRange(apparatusLabel(unit,uiLocale())
    || corpusSectionLabel(readableUnitLocator(unit,uiLocale()), uiLocale(), version?.id || '')
    || (Number.isSafeInteger(unit?.ordinal) ? `${t('passage')} ${unit.ordinal + (unit.ordinal === 0 ? 1 : 0)}` : ''),unit,version?.id);
  const locationLabel = reference => {
    if (!reference) return '';
    const state = modelState();
    const version = activeVersion(reference.versionId);
    const page = state.document ? state.windows[stateKey(state.document.id, reference.versionId)] : null;
    const unit = selected?.reference && sameReference(selected.reference, reference)
      ? selected.unit
      : page?.units?.find(item => (item.unitId || item.id) === reference.unitId);
    return [version ? versionLabel(version) : '', unitLabel(unit)].filter(Boolean).join(' · ');
  };
  const friendlyError = error => {
    switch (error?.code || error?.message) {
      case 'document-unavailable': return t('noCatalog');
      case 'version-unavailable': return t('unavailable');
      case 'provider_unavailable': return t('providerUnavailable');
      case 'corpus-search-unavailable':
      case 'version-search-unavailable': return t('searchUnavailable');
      case 'conflict': return t('saveError');
      default: return t('error');
    }
  };
  const stateKey = (documentId, versionId) => `${documentId}\u0000${versionId}`;
  const currentWindow = versionId => {
    const state = modelState();
    return state.windows[stateKey(state.document?.id, versionId)] || null;
  };
  const currentRef = () => {
    if (selected?.reference) return selected.reference;
    const state = modelState();
    const versionId = state.activeVersionId;
    const unit = state.windows[stateKey(state.document?.id, versionId)]?.units?.[0];
    const version = state.document?.versions?.find(item => item.id === versionId);
    return unit ? unitReference(unit, version, state.document, {}) : null;
  };
  const catalogItems = () => modelState().catalog.items || [];

  function announce(message) {
    lastMessage = message || '';
    if(disposed||!root)return;
    const status = root.querySelector('.cr-status');
    if (status) { status.textContent = lastMessage; status.hidden = !lastMessage; }
  }

  function notebookSaveLabel() {
    const status = model.notebookStatus?.() || {};
    if (noteDirty || status.pending > 0 || (notebook && status.adapter === 'pending')) return t('pending');
    if (status.error) return t('saveError');
    if (!notebook) return t('session');
    return status.persistent ? t('saved') : t('memory');
  }

  function notebookSaveTitle() {
    const status = model.notebookStatus?.() || {};
    if (status.warning || status.error) return t('saveError');
    return status.persistent ? '' : t('memory');
  }

  function emitLocation(reference = currentRef()) {
    if (!reference || typeof onLocation !== 'function') return;
    const exact = exactReference(reference);
    if (!exact) return;
    onLocation({
      documentId: exact.target.workId,
      versionId: exact.versionId,
      unitId: exact.unitId,
      revision: exact.target.textLayerSha256,
    });
  }

  function visiblePaneRow(pane) {
    const paneTop=pane.getBoundingClientRect().top;
    const stickyBottom=pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??paneTop;
    const boundary=Math.max(paneTop,stickyBottom)+2;
    return [...pane.querySelectorAll('.cr-unit')].find(item=>item.getBoundingClientRect().bottom>boundary);
  }

  function capturePanePositions() {
    if (disposed || !root) return paneSaveWork;
    const writes=[],document=modelState().document;
    for (const [index, pane] of [...root.querySelectorAll('.cr-pane')].entries()) {
      remember(panePositions,pane.dataset.versionId, pane.scrollTop);
      const row=visiblePaneRow(pane);
      if (row?.__readerUnit) {
        const version = document?.versions?.find(item=>item.id===pane.dataset.versionId);
        const reference = unitReference(row.__readerUnit, version, document, {});
        if (reference) writes.push(() => Promise.resolve(model.savePosition(reference,0,index===0?'primary':'secondary')).catch(()=>{if(!disposed)announce(t('saveError'));}));
      }
    }
    // Scroll callbacks can already have queued a write when pagehide arrives.
    // Keep captures in order so a late earlier callback cannot overwrite the
    // position observed by the final DOM snapshot.
    paneSaveWork = paneSaveWork.catch(()=>{}).then(() => Promise.all(writes.map(write => write())));
    return paneSaveWork;
  }

  function restorePanePositions() {
    for (const pane of root.querySelectorAll('.cr-pane')) {
      const top = panePositions.get(pane.dataset.versionId);
      if (Number.isFinite(top)) { pane.scrollTop = top; continue; }
      const saved = model.notebookState().positions?.find(item => {
        const reference = item.reference || item.anchor;
        return reference?.versionId === pane.dataset.versionId;
      });
      if (!saved) continue;
      const key = referenceKey(saved.reference || saved.anchor);
      const row = [...pane.querySelectorAll('.cr-unit')].find(item => item.dataset.reference === key);
      row?.scrollIntoView({block: 'start'});
    }
  }

  function notebookItems() {
    const value = model.notebookState();
    return {
      notes: Array.isArray(value.notes) ? value.notes : [],
      bookmarks: Array.isArray(value.bookmarks) ? value.bookmarks : [],
    };
  }

  function sameNoteReference(item, reference) {
    return sameReference(item?.reference || item?.anchor, reference);
  }

  function selectedRecord() {
    if (!selected?.reference) return null;
    const items = notebookItems();
    if (Object.hasOwn(selected, 'noteId')) {
      return items.notes.find(item => item.id === selected.noteId && sameNoteReference(item, selected.reference)) || null;
    }
    const item = items.notes.find(note => sameNoteReference(note, selected.reference)) || null;
    selected.noteId = item?.id ?? null;
    selected.noteRevision = item?.revision;
    return item;
  }

  async function flushNote() {
    clearTimeout(noteTimer);
    while (noteFlushPromise) { const saved=await noteFlushPromise; if(!saved)return false; }
    if (!noteDirty || !draft || !selected?.reference) return true;
    const target=selected.reference, value=draft.text, key=referenceKey(target), quote=selected.quote || '';
    const recordId=draft.recordId, recordRevision=draft.recordRevision;
    const task=(async()=>{
      try {
        const result=value.trim()
          ? await model.saveNote(target,value,quote,{id:recordId,revision:recordRevision})
          : recordId ? await model.deleteNote(target,{id:recordId,revision:recordRevision}) : null;
        const savedId=result?.item?.id ?? null, savedRevision=result?.item?.revision;
        if (selected?.reference === target && selected.noteId === recordId) {
          selected.noteId=savedId; selected.noteRevision=savedRevision;
        }
        if (draft?.key===key && draft.recordId===recordId) {
          draft.recordId=savedId; draft.recordRevision=savedRevision;
          if (draft.text!==value) {
            try{draftJournal.capture({reference:target,text:draft.text,quote,originalNoteId:savedId});}catch{}
          }
        }
        if(notebook?.status?.().persistent){
          // A failed cleanup retains a recovery copy; it does not undo the
          // notebook transaction that has already committed successfully.
          try{draftJournal.acknowledge({reference:target,text:value});}catch{}
        }
        if(draft?.key===key&&draft.text===value){draft=null;noteDirty=false;}
        if(!disposed){announce(notebookSaveLabel());updateFooter();}
        return true;
      } catch(error) {
        if(!disposed){announce(error.code==='conflict' ? (uiLocale()==='en'?'This note changed in another window. Your draft is kept.':'Заметка изменилась в другом окне. Ваш черновик сохранён здесь.') : t('saveError'));updateFooter();}
        return false;
      }
    })();
    noteFlushPromise=task;
    const result=await task;
    if(noteFlushPromise===task)noteFlushPromise=null;
    if(result && !disposed)refreshNotebookList();
    if(!disposed)updateFooter();
    return result;
  }

  async function selectUnit(unit, versionId, {quote = '', start = null, end = null, focus = false} = {}) {
    const sequence=++selectionSequence;
    if(!(await flushNote()) || sequence!==selectionSequence || disposed)return;
    const document = activeDocument(); const version = activeVersion(versionId);
    const startOffset = Number.isSafeInteger(start) ? codePointOffset(unit.text, start) : 0;
    const endOffset = Number.isSafeInteger(end) ? codePointOffset(unit.text, end) : startOffset;
    const reference = unitReference(unit, version, document, {start: startOffset, end: Math.max(startOffset, endOffset)});
    if (!reference) return;
    selected = {reference, unit, versionId, quote:quote||unit.text.slice(0,READER_LIMITS.quote)};
    philologyController?.abort();
    philologyController = null;
    philology = {busy:false,result:null,concordance:null,error:null};
    inspectorOpen = true;
    if (window.innerWidth <= 850) libraryOpen = false;
    currentTab = endOffset > startOffset ? 'word' : 'notes';
    mode = mode || 'single';
    emitLocation(reference);
    const layout = root.querySelector('.cr-layout');
    if (layout && !layout.querySelector('.cr-inspector')) {
      layout.append(element('aside', 'cr-inspector'));
      root.dataset.inspector = 'true';
    }
    renderInspector();
    markSelection();
    updateFooter();
    void loadPhilology();
    if (focus) root.querySelector('.cr-note-editor')?.focus();
  }

  async function loadPhilology() {
    const reference = selected?.reference;
    const unit = selected?.unit;
    const version = activeVersion(selected?.versionId);
    if (!reference || !unit || !version) return;
    const form = selectedPhilologyForm(unit.text, reference);
    if(form && !isSingleWordSelection(form)) {
      philology = {busy:false,result:null,concordance:null,error:null};
      renderInspector();
      return;
    }
    if (!form || typeof provider?.analyze !== 'function') {
      philology = {busy:false,result:{status:'unavailable',candidates:[]},concordance:null,error:null};
      renderInspector();
      return;
    }
    const controller = new AbortController();
    philologyController = controller;
    philology = {busy:true,result:null,concordance:null,error:null};
    renderInspector();
    try {
      const response = await provider.analyze({reference,form,language:version.language || unit.language || '',signal:controller.signal});
      if (controller.signal.aborted || selected?.reference !== reference) return;
      philology = {busy:false,result:bindPhilologyResult(response,reference,form),concordance:null,error:null};
    } catch (error) {
      if (controller.signal.aborted || error?.name === 'AbortError' || selected?.reference !== reference) return;
      philology = {busy:false,result:{status:'unavailable',candidates:[]},concordance:null,error};
    }
    renderInspector();
  }

  async function findPhilologyConcordance(form, cursor = null) {
    const reference = selected?.reference;
    const versionId = selected?.versionId;
    if (!reference || !form || !model.searchCapabilities().version) return;
    try {
      const page = await model.search({documentId:reference.target.workId,versionId,query:form,scope:'version',cursor});
      if (selected?.reference !== reference || !page) return;
      const exactItems = [];
      let inexactCount = 0;
      for (const item of page.items || []) {
        const hit = exactReference(item.reference);
        if (!hit || hit.target.workId !== reference.target.workId || hit.versionId !== versionId
          || hit.unitId !== item.unitId || !Number.isSafeInteger(hit.selector.start)
          || !Number.isSafeInteger(hit.selector.end) || hit.selector.end <= hit.selector.start) {
          inexactCount += 1;
          continue;
        }
        exactItems.push({...item,reference:hit});
      }
      philology = {...philology,concordance:{...page,items:exactItems,inexactCount},error:null};
      renderInspector();
    } catch (error) {
      if (selected?.reference !== reference) return;
      philology = {...philology,concordance:{items:[],nextCursor:null,total:null},error};
      announce(friendlyError(error));
      renderInspector();
    }
  }

  async function openPhilologyOccurrence(hit) {
    const reference = exactReference(hit?.reference);
    const origin = selected?.reference;
    const state = modelState();
    if (!reference || !origin || reference.target.workId !== state.document?.id || reference.versionId !== origin.versionId
      || !Number.isSafeInteger(reference.selector.start) || !Number.isSafeInteger(reference.selector.end)
      || reference.selector.end <= reference.selector.start) return;
    if (!(await flushNote()) || selected?.reference !== origin) return;
    selectionSequence++;
    capturePanePositions();
    const result = await model.loadWindow({documentId:state.document.id,versionId:reference.versionId,reference});
    if (selected?.reference !== origin) return;
    const unit = result?.units?.find(value => (value.unitId || value.id) === reference.unitId);
    if (!unit) {
      announce(t('error')); return;
    }
    selected = {reference,unit,versionId:reference.versionId,quote:selectedPhilologyForm(unit.text,reference)};
    philology = {busy:false,result:null,concordance:null,error:null};
    currentTab = 'context';
    emitLocation(reference);
    render(); revealSelectedSource();
  }

  async function openPhilologyTarget(mapping) {
    const target=exactReference(mapping?.targetReference),origin=selected?.reference,state=modelState();
    if(!target||target.target.workId!==state.document?.id||!Number.isSafeInteger(target.selector.start)
      ||!Number.isSafeInteger(target.selector.end)||target.selector.end<=target.selector.start)return;
    try{
      const resolved=await provider?.resolve?.({reference:target});
      if(selected?.reference!==origin||resolved?.status!=='exact'||!sameReference(resolved.reference,target)){
        if(selected?.reference===origin)announce(t('changedCurrent'));
        return;
      }
      if(!(await flushNote())||selected?.reference!==origin)return;
      selectionSequence++;
      capturePanePositions();
      const opened=await model.open({documentId:target.target.workId,versionId:target.versionId,reference:target});
      if(selected?.reference!==origin||!opened)return;
      const current=modelState(),unit=current.windows[stateKey(target.target.workId,target.versionId)]?.units?.find(value=>(value.unitId||value.id)===target.unitId);
      if(!unit||selectedPhilologyForm(unit.text,target)!==mapping.targetQuote){announce(t('changedCurrent'));return;}
      selected={reference:target,unit,versionId:target.versionId,quote:mapping.targetQuote};
      philology={busy:false,result:null,concordance:null,error:null};currentTab='context';emitLocation(target);render();revealSelectedSource();
    }catch(error){if(selected?.reference===origin)announce(friendlyError(error));}
  }

  function revealSelectedSource() {
    const reference = selected?.reference;
    if (!reference || !opened || disposed || !root) return;
    if (window.innerWidth <= 850 && (inspectorOpen || libraryOpen)) {
      inspectorOpen = false; libraryOpen = false; render();
    }
    const pane = [...root.querySelectorAll('.cr-pane')].find(node => node.dataset.versionId === reference.versionId);
    const row = pane && [...pane.querySelectorAll('.cr-unit')].find(node => node.dataset.unitId === reference.unitId);
    if (!row) return;
    const mark = row.querySelector('[data-selected-occurrence]');
    (mark || row).scrollIntoView({block:mark?'center':'start',inline:'nearest'});
    const boundary=pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??pane.getBoundingClientRect().top;
    if(row.getBoundingClientRect().top<boundary && !mark)pane.scrollTop-=boundary-row.getBoundingClientRect().top+2;
    refreshPaneLocation(pane,activeVersion(reference.versionId));
  }

  function markSelection() {
    for (const row of root.querySelectorAll('.cr-unit')) row.dataset.selected = String(selected?.reference && row.dataset.unitId===selected.reference.unitId && row.closest('.cr-pane')?.dataset.versionId===selected.versionId);
  }

  function unitFromSelection() {
    const selection = window.getSelection?.();
    if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return null;
    const range = selection.getRangeAt(0);
    const row = selection.anchorNode?.parentElement?.closest?.('.cr-unit') || selection.focusNode?.parentElement?.closest?.('.cr-unit');
    if (!row || !root.contains(row)) return null;
    const pane = row.closest('.cr-pane');
    const unit = row.__readerUnit;
    if (!pane || !unit) return null;
    const start = selectedOffsets(row, range.startContainer, range.startOffset);
    const end = selectedOffsets(row, range.endContainer, range.endOffset);
    const quote = range.toString().slice(0, READER_LIMITS.quote);
    if (start === null || end === null || !quote) return null;
    return {unit, versionId: pane.dataset.versionId, quote, start, end};
  }

  function onSelectionChange() {
    if (!opened || rendering || disposed) return;
    const value = unitFromSelection();
    if (!value) return;
    selectUnit(value.unit, value.versionId, value);
  }

  async function copySelected() {
    const item = selected?.unit;
    if (!item) return;
    const document = activeDocument();
    const version = activeVersion(selected.versionId);
    const reference = selected.reference;
    if (!version || reference?.target?.workId !== document?.id || reference.versionId !== version.id
      || reference.unitId !== (item.unitId || item.id)
      || (version.source?.textLayerSha256 && reference.target.textLayerSha256!==version.source.textLayerSha256)) { announce(t('changedCurrent')); return; }
    const quote = quotationWithEditorialRoles(version,item,reference.selector,uiLocale());
    let value = quoteAttribution({document, version, unit: item, quote, locale: uiLocale()});
    const difference=variants.variants.find(entry=>entry.versionId===version.id && entry.revision===version.revision && entry.unitId===reference.unitId
      && codePointSlice(item.text,entry.selector.start,entry.selector.end)===entry.digitalReading
      && (reference.selector.end===reference.selector.start || reference.selector.start<entry.selector.end && reference.selector.end>entry.selector.start));
    if(difference)value+=`\n${uiLocale()==='en'?'Reviewed witness difference':'Проверенное различие свидетелей'}: “${difference.digitalReading}” / “${difference.printReading}” · ${difference.printWitness} · ${difference.printUrl}`;
    try { await navigator.clipboard.writeText(value); }
    catch {
      const fallback = element('textarea', 'cr-copy-fallback', value);
      fallback.readOnly = true; root.append(fallback); fallback.focus(); fallback.select();
    }
    announce(t('copied'));
  }

  function graphTarget(unit) {
    const target = unit?.graphTarget || unit?.graph_target || unit?.context?.graphTarget;
    if (!target || typeof target !== 'object' || !target.kind || !target.id) return null;
    return {kind: target.kind, id: target.id};
  }

  async function openGraph(unit) {
    if (!(await flushNote())) return;
    await capturePanePositions();
    const versionId = selected?.versionId || modelState().activeVersionId;
    const reference = unitReference(unit, activeVersion(versionId), activeDocument(), {});
    if (!reference) return;
    const context = {graphTarget: graphTarget(unit), documentId: reference.target.workId, versionId: reference.versionId, unitId: reference.unitId};
    try {
      await onGraphRequest?.({reference, context});
      await close();
    } catch (error) {
      announce(friendlyError(error));
      render();
    }
  }

  function renderHeader(state) {
    const header = element('header', 'cr-header');
    const menu = button('☰', () => { libraryOpen = !libraryOpen; if (libraryOpen && window.innerWidth <= 850) inspectorOpen = false; render(); }, 'cr-icon cr-menu');
    menu.dataset.focusKey='library-toggle';
    menu.hidden=!state.document;
    menu.setAttribute('aria-label', t('library')); menu.setAttribute('aria-expanded', String(libraryOpen));
    const title = element('div', 'cr-title');
    title.append(element('span', 'cr-kicker', `✧ ${t('reading')}`));
    title.append(element('h1', '', state.document ? documentTitle(state.document) : t('reading')));
    const author = localized(state.document?.author, uiLocale());
    if (author) title.append(element('p', '', author));
    const jump=button(t('jumpToText'),()=>root.querySelector('.cr-pane')?.focus({preventScroll:true}),'cr-jump-text');
    jump.disabled=!state.document;
    header.append(menu, jump, title, button('×', () => close(), 'cr-close'));
    header.querySelector('.cr-close').setAttribute('aria-label', t('close'));
    return header;
  }

  function catalogAvailability(value) {
    const status = availabilityLabel(value);
    const normalized = String(status).toLowerCase().replace(/-/g, '_');
    if (normalized === 'metadata_only' || normalized === 'metadata') return t('metadataOnly');
    if (['restricted', 'public_metadata_only', 'local_only'].includes(normalized)) return t('restricted');
    if (['unavailable', 'missing', 'expired', 'text_unavailable', 'withheld', 'unknown', 'pending', 'not_available', 'not_indexed', 'rights_unknown'].includes(normalized)) return t('unavailableVersion');
    return status === 'available' ? (uiLocale() === 'en' ? 'Available' : 'Доступно') : t('availabilityUnknown');
  }

  function countLabel(value) {
    return Number.isSafeInteger(value) && value >= 0 ? String(value) : t('unknown');
  }

  function renderLibrary(state) {
    const aside = element('aside', 'cr-library');
    aside.setAttribute('aria-label', t('library'));
    aside.append(element('h2', 'cr-panel-title', t('library')));
    aside.append(element('p', 'cr-library-count', `${t('works')}: ${countLabel(state.catalog.total)}`));
    const input = element('input', 'cr-library-search'); input.type = 'search'; input.placeholder = t('find'); input.setAttribute('aria-label',t('find')); input.value = state.catalog.query || '';
    input.addEventListener('input', () => {
      catalogCancel?.(); catalogCancel = wait(240, () => model.catalog({query: input.value}).catch(error => announce(friendlyError(error))));
    });
    const inputLabel=element('label','cr-library-search-label');inputLabel.append(element('span','',t('find')),input);aside.append(inputLabel);
    const list = element('div', 'cr-library-list');
    const items = catalogItems();
    if (!items.length && !state.catalog.busy) {
      list.append(element('p', 'cr-muted', state.catalog.nextCursor ? t('continuation') : t('noCatalog')));
    }
    for (const document of items) {
      const item = button('', () => { openDocument(document.id).catch(error => announce(friendlyError(error))); }, 'cr-library-item');
      item.dataset.current = String(document.id === state.document?.id);
      item.dataset.available = String(isTextAvailable(document));
      item.append(element('strong', '', documentTitle(document)));
      const author = localized(document.author, uiLocale());
      const descriptor = localized(document.locator, uiLocale()) || author;
      const availability = catalogAvailability(document);
      const count = Number.isSafeInteger(document.unitCount) && document.unitCount >= 0
        ? `${t('units')}: ${document.unitCount}` : '';
      const meta = [descriptor, availability, count].filter(Boolean).join(' · ');
      if (meta) item.append(element('small', '', meta));
      list.append(item);
    }
    aside.append(list);
    if (state.catalog.nextCursor) aside.append(button(t('loadMore'), () => model.catalog({query: state.catalog.query, cursor: state.catalog.nextCursor}).catch(error => announce(friendlyError(error))), 'cr-more'));
    return aside;
  }


  const paneLoads = new Map();

  async function ensureVisiblePanes() {
    const state = modelState();
    if (!state.document) return;
    const versions = chooseVersions(state.document, state);
    model.setVisibleVersions?.(versions.map(version => version.id));
    const pending = [];
    for (const version of versions) {
      const key = stateKey(state.document.id, version.id);
      const stale = ['changed', 'unavailable'].includes(state.referenceStates[key]);
      if (currentWindow(version.id) || paneLoads.has(key) || stale) continue;
      const load = Promise.resolve(model.loadWindow({documentId: state.document.id, versionId: version.id}))
        .catch(error => { announce(friendlyError(error)); return null; })
        .finally(() => paneLoads.delete(key));
      paneLoads.set(key, load);
      pending.push(load);
    }
    await Promise.all(pending);
  }

  async function openDocument(documentId) {
    if(!(await flushNote()) || disposed)return null;
    if(window.innerWidth<=850)libraryOpen=false;
    await capturePanePositions();
    if(disposed)return null;
    selectionSequence++; selected = null;
    searchCancel?.(); searchGeneration++;
    searchQuery = ''; searchVersionId = null;
    searchResult = {items: []};
    const result = await model.open({documentId});
    if(disposed)return null;
    await ensureVisiblePanes();
    if(disposed)return null;
    render();
    emitLocation();
    return result;
  }

  async function openVersion(documentId, versionId) {
    if(!(await flushNote()) || disposed)return null;
    if(window.innerWidth<=850){libraryOpen=false;inspectorOpen=false;}
    await capturePanePositions();
    if(disposed)return null;
    selectionSequence++; selected = null;
    searchCancel?.(); searchGeneration++;
    searchQuery = ''; searchVersionId = null;
    searchResult = {items: []};
    const result = await model.open({documentId, versionId});
    if(disposed)return null;
    await ensureVisiblePanes();
    if(disposed)return null;
    render();
    emitLocation();
    return result;
  }

  function chooseVersions(document, state) {
    const allVersions = document?.versions || [];
    const versions = allVersions.filter(isTextAvailable);
    // Keep an explicitly selected unavailable version visible so its rights or
    // delivery state is inspectable; silently falling back would retarget the
    // reader to another text layer.
    const active = allVersions.find(version => version.id === state.activeVersionId) || versions[0] || allVersions[0];
    if (!active) return [];
    if (!isTextAvailable(active) || mode !== 'parallel' || versions.length < 2) return [active];
    const preferred = comparison.get(document.id);
    const other = versions.find(version => version.id === preferred && version.id !== active.id) || versions.find(version => version.id !== active.id);
    return other ? [active, other] : [active];
  }

  function versionLabel(version) {
    const language = versionLanguage(version);
    const role = version.role === 'original' ? t('original') : version.role === 'translation' ? t('translation') : '';
    const edition = localized(version.edition, uiLocale())
      || (version.label && !/^(original|translation)(?:\s|\(|$)/i.test(String(version.label)) ? localized(version.label, uiLocale()) : '');
    const status = isTextAvailable(version) ? '' : catalogAvailability(version);
    return [language, role, edition, status].filter(Boolean).join(' · ') || t('edition');
  }

  function versionChoice(version) {
    const choice = button(versionLabel(version), () => {
      const picker = root.querySelector('.cr-edition-picker');
      if (picker) picker.open = false;
      openVersion(modelState().document.id, version.id).catch(error => announce(friendlyError(error)));
    }, 'cr-version');
    choice.dataset.versionId = version.id;
    choice.dataset.focusKey = `version:${version.id}`;
    choice.setAttribute('aria-pressed', String(version.id === modelState().activeVersionId));
    choice.dataset.available = String(isTextAvailable(version));
    if (!isTextAvailable(version)) choice.title = catalogAvailability(version);
    return choice;
  }

  function editionRepresentationLabel(version) {
    const kind = editionRepresentationKind(version);
    return kind === 'retained' ? t('retainedTextLayer') : kind === 'source' ? t('sourceTextLayer') : '';
  }

  function renderVersionChoices(versions, className = 'cr-edition-options') {
    const nav = element('nav', className);
    nav.setAttribute('aria-label', t('edition'));
    for (const group of groupEditionRepresentations(versions)) {
      if (group.versions.length === 1) {
        const choice=versionChoice(group.versions[0]);
        if(className==='cr-edition-options')choice.dataset.focusKey='mobile-edition';
        nav.append(choice);
        continue;
      }
      const details = element('details', 'cr-edition-group');
      const sourceVersion = group.versions.find(item => editionRepresentationKind(item) === 'source') || group.versions[0];
      const selectedVersion = group.versions.find(item => item.id === modelState().activeVersionId);
      const selectedLayer = selectedVersion ? editionRepresentationLabel(selectedVersion) : '';
      const label = element('summary', '', `${selectedLayer ? `${t('selectedEdition')}: ${selectedLayer} · ` : ''}${versionLabel(sourceVersion)} · ${group.versions.length} ${t('editionVersions')}`);
      label.dataset.focusKey=`edition-group:${sourceVersion.id}`;
      details.append(label);
      const layers = element('div', 'cr-edition-layers');
      for (const version of group.versions) {
        const descriptor = editionRepresentationLabel(version);
        const choice = versionChoice(version);
        choice.dataset.focusKey=className==='cr-edition-options'?'mobile-edition':label.dataset.focusKey;
        if (descriptor) choice.prepend(element('small', 'cr-edition-layer-kind', descriptor));
        layers.append(choice);
      }
      details.append(layers);
      nav.append(details);
    }
    return nav;
  }

  function renderVersionBar(state, versions) {
    const toolbar = element('nav', 'cr-toolbar'); toolbar.setAttribute('aria-label', t('edition'));
    const allVersions = state.document?.versions || [];
    toolbar.append(renderVersionChoices(allVersions, 'cr-desktop-editions'));
    const picker = element('details', 'cr-edition-picker');
    picker.setAttribute('aria-label', t('mobileEditions'));
    const active = allVersions.find(version => version.id === state.activeVersionId);
    const activeLayer = active ? editionRepresentationLabel(active) : '';
    const mobileSummary=element('summary', '', `${t('edition')} · ${activeLayer ? `${activeLayer} · ` : ''}${active ? versionLabel(active) : t('edition')}`);
    mobileSummary.dataset.focusKey='mobile-edition';picker.append(mobileSummary);
    picker.append(renderVersionChoices(allVersions));
    toolbar.append(picker);
    if ((state.document?.versions || []).length > 1) {
      toolbar.append(button(mode === 'parallel' ? t('one') : t('parallel'), async () => {
        if(!(await flushNote()))return;
        capturePanePositions();
        mode = mode === 'parallel' ? 'single' : 'parallel';
        searchCancel?.(); searchGeneration++; searchVersionId=null; searchResult={items:[]};
        render();
        if(searchOpen && searchQuery)queueSearch();
        void ensureVisiblePanes();
      }, 'cr-mode'));
      toolbar.lastChild.dataset.focusKey='compare-toggle';
    }
    if (mode === 'parallel' && versions.length > 1) {
      const label = element('label', 'cr-compare'); label.append(element('span', '', t('compare')));
      const select = element('select'); select.setAttribute('aria-label', t('compare'));
      select.dataset.focusKey='compare-target';
      for (const version of state.document.versions.filter(item => item.id !== state.activeVersionId && isTextAvailable(item))) {
        const option = element('option', '', [editionRepresentationLabel(version),versionLabel(version)].filter(Boolean).join(' · ')); option.value = version.id; select.append(option);
      }
      select.value = versions[1]?.id || '';
      select.addEventListener('change', async () => { if(!(await flushNote()))return; capturePanePositions(); remember(comparison,state.document.id, select.value); searchCancel?.();searchGeneration++;searchVersionId=null;searchResult={items:[]}; render(); if(searchOpen && searchQuery)queueSearch();void ensureVisiblePanes(); }); label.append(select); toolbar.append(label);
    }
    toolbar.append(element('span', 'cr-grow'));
    toolbar.append(button('⌕', () => { searchOpen = !searchOpen; if (!searchOpen) { searchQuery = ''; searchResult = {items: []}; } render(); root.querySelector('.cr-search-input')?.focus(); }, 'cr-icon cr-search-open'));
    toolbar.lastChild.setAttribute('aria-label',t('search')); toolbar.lastChild.setAttribute('aria-expanded',String(searchOpen));toolbar.lastChild.dataset.focusKey='search-toggle';
    toolbar.append(button('Aa', () => { settingsOpen = !settingsOpen; render(); }, 'cr-settings-button'));
    toolbar.lastChild.setAttribute('aria-label',t('settings')); toolbar.lastChild.setAttribute('aria-expanded',String(settingsOpen));toolbar.lastChild.dataset.focusKey='settings-toggle';
    toolbar.append(button('✧', () => { inspectorOpen = !inspectorOpen; if (inspectorOpen && window.innerWidth <= 850) libraryOpen = false; render(); }, 'cr-icon cr-notes-toggle'));
    toolbar.lastChild.setAttribute('aria-label', t('notes')); toolbar.lastChild.setAttribute('aria-expanded', String(inspectorOpen));toolbar.lastChild.dataset.focusKey='notebook-toggle';
    return toolbar;
  }

  function queueSearch() {
    searchCancel?.();
    const generation=++searchGeneration, query=searchQuery, selectedScope=searchScope;
    const documentId=modelState().document?.id,versionId=searchVersionId;
    searchResult={items:[],total:null};
    const current=()=>generation===searchGeneration && modelState().document?.id===documentId
      && searchVersionId===versionId && chooseVersions(modelState().document,modelState()).some(version=>version.id===versionId) && searchQuery===query && searchScope===selectedScope;
    searchCancel = wait(180, () => model.search({documentId,versionId,query,scope:selectedScope}).then(result => {
      if(current()){searchResult = result || {items: []}; render();}
    }).catch(error => { if(current()){searchResult = {items: [], nextCursor: null, total: null}; announce(friendlyError(error)); render();} }));
  }

  function renderSearch(state) {
    const bar = element('section', 'cr-search');
    const visibleVersions=chooseVersions(state.document,state);
    if(!visibleVersions.some(version=>version.id===searchVersionId))searchVersionId=state.activeVersionId;
    const targetVersionId=searchVersionId;
    const stored = state.document ? state.searches[stateKey(state.document.id, targetVersionId)] : null;
    if (stored?.query === searchQuery && stored?.scope === searchScope) searchResult = stored;
    const input = element('input', 'cr-search-input'); input.type = 'search'; input.placeholder = t('searchPlaceholder'); input.setAttribute('aria-label',t('searchPlaceholder'));input.value = searchQuery; input.maxLength = READER_LIMITS.query;
    const scope = element('select', 'cr-search-scope');scope.setAttribute('aria-label',uiLocale()==='en'?'Search scope':'Область поиска');
    let edition=null;
    if(visibleVersions.length>1){
      const label=element('label','cr-search-edition');label.append(element('span','',t('searchEdition')));
      edition=element('select','cr-search-version');edition.setAttribute('aria-label',t('searchEdition'));
      for(const version of visibleVersions){const option=element('option','',[editionRepresentationLabel(version),versionLabel(version)].filter(Boolean).join(' · '));option.value=version.id;edition.append(option);}
      edition.value=targetVersionId;label.append(edition);bar.append(label);
    }
    const capabilities = model.searchCapabilities();
    for (const [value, label, enabled] of [['loaded', t('loaded'), capabilities.loaded], ['version', t('version'), capabilities.version], ['corpus', t('corpus'), capabilities.corpus]]) {
      const option = element('option', '', label); option.value = value; option.disabled = !enabled; scope.append(option);
    }
    if (!capabilities[searchScope]) searchScope = capabilities.version ? 'version' : 'loaded'; scope.value = searchScope;
    const resultCount = searchResult.items?.length || 0;
    const countValue = searchResult.total === null || searchResult.total === undefined
      ? `${resultCount} · ${t('unknown')}`
      : `${resultCount} / ${searchResult.total}`;
    const count = element('span', 'cr-search-count', searchQuery ? countValue : ''); count.setAttribute('role', 'status');
    const update = () => {
      searchQuery = input.value; searchScope = scope.value;queueSearch();
    };
    input.addEventListener('input', update); scope.addEventListener('change', update);
    edition?.addEventListener('change',()=>{searchVersionId=edition.value;update();});
    input.addEventListener('keydown', event => { if (event.key === 'Escape') { event.stopPropagation();event.preventDefault();searchOpen = false; searchQuery = ''; searchResult = {items: []}; render();root.querySelector('.cr-search-open')?.focus(); } });
    const inputLabel=element('label','cr-search-query');inputLabel.append(element('span','',t('searchPlaceholder')),input);
    bar.append(inputLabel, scope, count);
    const hasContinuation = Boolean(searchResult.nextCursor);
    if (searchResult.items?.length || hasContinuation) {
      const results = element('div', 'cr-search-results');
      for (const item of searchResult.items.slice(-READER_LIMITS.searchResults)) {
        const result = button('', async () => {
          const sequence=++selectionSequence;
          if(!(await flushNote()) || sequence!==selectionSequence || disposed)return;
          const version = activeVersion(targetVersionId);
          const expectedDocument=state.document.id, expectedVersion=version?.id;
          const isCurrent=()=>sequence===selectionSequence && !disposed && modelState().document?.id===expectedDocument && searchVersionId===expectedVersion
            && chooseVersions(modelState().document,modelState()).some(item=>item.id===expectedVersion);
          if(!expectedVersion || !isCurrent())return;
          const exact=exactReference(item.reference);
          try {
            const page=await model.loadWindow({documentId:expectedDocument,versionId:expectedVersion,
              commitGuard:isCurrent,
              ...(exact?{reference:exact}:{unitId:item.unitId,revision:version.revision})});
            if(!isCurrent())return;
            const unit=page?.units?.find(value=>value.unitId===item.unitId||value.id===item.unitId);
            if(!unit){announce(t('error'));return;}
            const reference=exact || unitReference(unit,version,state.document,{start:item.start||0,end:item.end||0});
            selected={reference,unit,versionId:expectedVersion,quote:selectedPhilologyForm(unit.text,reference).slice(0,READER_LIMITS.quote)};
            emitLocation(reference); render(); revealSelectedSource();
          } catch { announce(t('error')); }
        }, 'cr-search-result');
        const unit = currentWindow(targetVersionId)?.units?.find(value => (value.unitId || value.id) === item.unitId);
        const kind=item.sourceLayer || (unit && editorialKindForMatch(activeVersion(targetVersionId),unit,item.start,item.end))
          || sourceLayerKind(activeVersion(targetVersionId),item,item.start);
        result.append(element('strong', '', corpusSectionLabel(item.label,uiLocale(),targetVersionId) || paneUnitLabel(unit || item,activeVersion(targetVersionId))));
        if(kind)result.append(element('small','cr-search-layer',editorialLabel(kind,uiLocale())));
        result.append(element('span', '', item.snippet || item.excerpt || ''));
        results.append(result);
      }
      if (!searchResult.items?.length && hasContinuation) results.append(element('span', 'cr-muted', t('continuation')));
      bar.append(results);
      if (hasContinuation) {
        bar.append(button(t('loadMore'), async event => {
          const trigger = event.currentTarget;
          const generation=searchGeneration,query=searchQuery,selectedScope=searchScope,documentId=state.document?.id,versionId=targetVersionId;
          trigger.disabled = true;
          try {
            const result=await model.search({documentId,versionId,query,scope:selectedScope,cursor:searchResult.nextCursor});
            if(generation!==searchGeneration || modelState().document?.id!==documentId || searchVersionId!==versionId || searchQuery!==query || searchScope!==selectedScope)return;
            searchResult = result || searchResult;
            render();
          } catch (error) {
            trigger.disabled = false;
            announce(friendlyError(error));
          }
        }, 'cr-more cr-search-more'));
      }
    } else if (searchQuery && searchResult.total !== null && searchResult.total !== undefined) bar.append(element('span', 'cr-muted', t('noSearch')));
    return bar;
  }

  function unitMatches(unit,versionId) {
    return versionId===searchVersionId ? (searchResult.items || []).filter(item => item.unitId && item.unitId === (unit.unitId || unit.id)) : [];
  }

  function renderUnit(unit, version, sourceDocument, {showWholeLayer=true} = {}) {
    const versionId=version.id;
    const row = element(unit.heading ? 'h3' : 'div', `cr-unit${unit.heading ? ' cr-heading' : ''}${unit.kind === 'verse_line' ? ' cr-verse-line' : ''}`);
    // All units in this render share its validated snapshot. Looking up the
    // active document per unit used to clone both text windows twice per row.
    const reference = unitReference(unit, version, sourceDocument, {});
    row.dataset.reference = referenceKey(reference); row.dataset.unitId = unit.unitId || ''; row.dataset.kind = unit.kind || ''; row.__readerUnit = unit;
    if (selected && sameReference(selected.reference, reference)) row.dataset.selected = 'true';
    const ordinal = Number.isSafeInteger(unit.ordinal) && unit.ordinal > 0 ? unit.ordinal : Number.isSafeInteger(unit.ordinal) ? unit.ordinal + 1 : null;
    const markerLabel = withMarkerRange(apparatusLabel(unit,uiLocale()) || corpusSectionLabel(unit.label,uiLocale(),versionId) || (ordinal === null ? '·' : String(ordinal)),unit,versionId);
    const marker = button(markerLabel, () => selectUnit(unit, versionId, {focus: true}), 'cr-unit-marker');
    if(Array.from(markerLabel).length>7)marker.classList.add('cr-unit-marker-long');
    marker.setAttribute('aria-label', `${t('select')} ${markerLabel}`);
    const content = element(unit.heading?'span':'div', 'cr-unit-text');
    const matches = unitMatches(unit,versionId).filter(match => Number.isSafeInteger(match.start) && Number.isSafeInteger(match.end) && match.end >= match.start).sort((a, b) => a.start - b.start);
    const codePoints = Array.from(unit.text).length;
    const selectedReference = selected?.versionId === versionId && selected.reference?.unitId === reference?.unitId
      ? exactReference({...reference, selector:selected.reference.selector})
      : null;
    const selectedSpan = selectedReference && sameReference(selectedReference, selected.reference)
      ? {start:selected.reference.selector.start,end:selected.reference.selector.end}
      : null;
    const intervals = philologyMarkIntervals(codePoints,matches,selectedSpan,Boolean(searchQuery));
    const editorial=editorialRanges(version,unit);
    const boundaries=[...new Set([0,codePoints,...intervals.flatMap(value=>[value.start,value.end]),...editorial.flatMap(value=>[value.start,value.end])])].sort((a,b)=>a-b);
    for(let index=0;index<boundaries.length-1;index++){
      const start=boundaries[index],end=boundaries[index+1];if(end<=start)continue;
      const selectedMark=intervals.find(value=>value.start<=start&&value.end>=end);
      const annotation=editorial.find(value=>value.start<=start&&value.end>=end);
      let piece=document.createTextNode(codePointSlice(unit.text,start,end));
      if(selectedMark){const mark=element('mark',selectedMark.selected?'cr-selected-occurrence':'');
        if(selectedMark.selected)mark.dataset.selectedOccurrence='true';mark.append(piece);piece=mark;}
      if(annotation){const span=element('span',`cr-editorial-${annotation.kind}${start===annotation.start?' cr-editorial-start':''}`);
        const label=editorialLabel(annotation.kind,uiLocale());span.title=label;span.dataset.editorialLabel=label;
        span.setAttribute('role','note');span.setAttribute('aria-label',`${label}: ${codePointSlice(unit.text,start,end)}`);
        span.append(piece);piece=span;}
      content.append(piece);
    }
    row.append(marker, content);
    const kinds=[...new Set(editorial.map(value=>value.kind))];
    const wholeLayer=sourceLayerKind(version,unit);
    if(showWholeLayer && wholeLayer && !kinds.includes(wholeLayer))kinds.unshift(wholeLayer);
    if(kinds.length){const notice=element('small','cr-editorial-notice',kinds.map(kind=>editorialLabel(kind,uiLocale())).join(' · '));row.append(notice);}
    const difference=variants.variants.find(value=>value.versionId===version.id && value.revision===version.revision && value.unitId===(unit.unitId||unit.id)
      && codePointSlice(unit.text,value.selector.start,value.selector.end)===value.digitalReading);
    if(difference){
      const notice=element('p','cr-textual-variant');
      notice.append(document.createTextNode(uiLocale()==='en'
        ? `Reviewed witness difference: this digital text reads “${difference.digitalReading}”; the printed edition reads “${difference.printReading}”. `
        : `Проверенное различие свидетелей: в цифровом тексте «${difference.digitalReading}», в печатном издании «${difference.printReading}». `));
      const link=element('a','',uiLocale()==='en'?'Printed witness, p. 334':'Печатный свидетель, с. 334');
      link.href=difference.printUrl;link.target='_blank';link.rel='noopener noreferrer';notice.append(link);row.insertBefore(notice,content);
    }
    return row;
  }

  function renderPane(version,state) {
    const pane=element('article','cr-pane');pane.dataset.versionId=version.id;pane.lang=version.language||'';
    pane.dir=/^(ar|he|fa|ur)(-|$)/i.test(version.language||'')?'rtl':'ltr';
    pane.tabIndex=0;pane.setAttribute('aria-label',`${documentTitle(state.document)} · ${versionLabel(version)}`);
    const currentLocation=element('div','cr-current-location');
    currentLocation.append(element('span','cr-kicker',t('currentLocation')),element('strong','',t('loading')));
    pane.append(currentLocation);
    const head=element('header','cr-pane-head');
    const layer=editionRepresentationLabel(version);
    if(layer)head.append(element('span','cr-layer-badge',layer));
    head.append(element('span','cr-kicker',versionLabel(version)),element('h2','',documentTitle({id:state.document.id,title:version.title||state.document.title},[version.id])));
    const meta=[localized(version.edition,uiLocale()),localized(version.translator,uiLocale()),localized(version.locator,uiLocale())].filter(Boolean).join(' · ');
    if(meta)head.append(element('p','cr-muted',meta));pane.append(head);
    if(hasEditorialProjection(version))head.append(element('p','cr-editorial-notice',uiLocale()==='en'
      ?'Source markup is shown where matched; consult the edition for its complete apparatus.'
      :'Пометы источника показаны там, где их удалось сопоставить; полный аппарат проверяйте в издании.'));
    const page=state.windows[stateKey(state.document.id,version.id)];pane.dataset.windowStart=page?.units?.[0]?.id||'';
    const selectedHere = selected?.versionId === version.id ? selected.unit : null;
    const firstUnit = page?.units?.[0];
    const lastUnit = page?.units?.at(-1);
    const exactLocation = selectedHere ? paneUnitLabel(selectedHere, version) : '';
    const firstLocation = firstUnit ? paneUnitLabel(firstUnit, version) : '';
    const lastLocation = lastUnit ? paneUnitLabel(lastUnit, version) : '';
    const loadedLocation = firstLocation
      ? `${firstLocation}${lastLocation && lastLocation !== firstLocation ? `–${lastLocation}` : ''}`
      : firstUnit ? paneUnitLabel(firstUnit, version) : '';
    if (loadedLocation) head.append(element('p', 'cr-column-location', `${t('loadedPassage')}: ${loadedLocation}`));
    if(exactLocation)head.append(element('p','cr-column-location',`${t('selectedPassage')}: ${exactLocation}`));
    const referenceStatus=state.referenceStates[stateKey(state.document.id,version.id)]||'exact';
    const stale=['changed','unavailable'].includes(referenceStatus);
    if(stale){
      const notice=element('div','cr-loading cr-reference-state');notice.setAttribute('role','status');notice.append(element('p','',referenceStatus==='changed'?t('changed'):t('unavailableCurrent')));
      notice.append(button(t('openCurrent'),async event=>{
        if(!(await flushNote()))return;const trigger=event.currentTarget;trigger.disabled=true;
        try{const result=await model.loadWindow({documentId:state.document.id,versionId:version.id,acceptCurrent:true});
          if(result){selected=null;panePositions.delete(version.id);const current=activeVersion(version.id);emitLocation(result.units[0]?unitReference(result.units[0],current,activeDocument(),{}):null);render();}}
        catch(error){announce(t('error'));trigger.disabled=false;}
      },'cr-open-current'));pane.append(notice);
    }
    if(!page&&!stale)pane.append(element('div','cr-loading',state.error?t('error'):t('loading')));
    else if(page&&!page.units.length)pane.append(element('p','cr-empty',t('empty')));
    else if(page){const body=element('div','cr-text-page');let previousLayer=null;
      for(const unit of page.units){const layer=sourceLayerKind(version,unit);body.append(renderUnit(unit,version,state.document,{showWholeLayer:layer!==previousLayer}));previousLayer=layer;}
      pane.append(body);}
    const windowError = state.windowErrors?.[stateKey(state.document.id, version.id)];
    if (page && windowError) {
      const notice = element('p', 'cr-notice cr-window-error', friendlyError(windowError));
      notice.setAttribute('role', 'status');
      pane.insertBefore(notice, pane.querySelector('.cr-pane-nav'));
    }
    const move=async direction=>{if(!(await flushNote()))return;capturePanePositions();try{
      selectionSequence++;const result=await model.loadWindow({versionId:version.id,direction});if(result){selected=null;panePositions.delete(version.id);const first=result.units[0];emitLocation(first?unitReference(first,activeVersion(version.id),activeDocument(),{}):null);render();}}
      catch(error){announce(t('error'));}};
    const nav=element('footer','cr-pane-nav');const previous=button(`← ${t('previousChunk')}`,()=>move('previous'),'cr-nav');const next=button(`${t('nextChunk')} →`,()=>move('next'),'cr-nav');
    previous.disabled=stale||!page?.hasPrevious;next.disabled=stale||!page?.hasNext;
    const first=page?.units?.[0]?.ordinal,last=page?.units?.at(-1)?.ordinal,total=Number.isSafeInteger(page?.total)?page.total:null;
    const range = first && last ? `${first}–${last} / ${total ?? t('unknown')}` : `${page?.units?.length ? page.units.length : '—'} / ${total ?? t('unknown')}`;
    nav.append(previous,element('span','cr-muted',range),next);pane.append(nav);
    pane.addEventListener('scroll',()=>{refreshPaneLocation(pane,version);remember(panePositions,version.id,pane.scrollTop);clearTimeout(paneSaveTimers.get(version.id));paneSaveTimers.set(version.id,setTimeout(()=>{paneSaveTimers.delete(version.id);if(!disposed&&opened)capturePanePositions();},550));},{passive:true});
    return pane;
  }

  function refreshPaneLocation(pane,version) {
    const value=pane.querySelector('.cr-current-location strong');
    if(!value)return;
    const row=visiblePaneRow(pane);
    const unit=row?.__readerUnit,layer=unit&&sourceLayerKind(version,unit);
    const label=unit?[paneUnitLabel(unit,version),layer?editorialLabel(layer,uiLocale()):''].filter(Boolean).join(' · '):'';
    value.textContent=label||t('loading');
  }

  function renderContext(body, state) {
    body.append(element('h2', 'cr-inspector-title', t('context')));
    const version = activeVersion(state.activeVersionId);
    if (state.referenceStatus === 'changed') body.append(element('p', 'cr-notice cr-changed', t('changed')));
    if (state.referenceStatus === 'unavailable') body.append(element('p', 'cr-notice cr-changed', t('unavailable')));
    const text = localized(state.document?.description || state.document?.context, uiLocale());
    if (text) body.append(element('p', 'cr-context-copy', text));
    if (version) {
      const metadata = [localized(version.edition, uiLocale()), localized(version.translator, uiLocale()), localized(version.locator, uiLocale())].filter(Boolean);
      if (metadata.length) {
        const details = element('details', 'cr-source-details'); details.open = true; details.append(element('summary', '', t('provenance')));
        details.append(element('p', '', metadata.join(' · '))); body.append(details);
      }
    }
    if (selected?.reference && selected?.unit) {
      const quote = selectedPhilologyForm(selected.unit.text, selected.reference)
        ? quotationWithEditorialRoles(activeVersion(selected.versionId),selected.unit,selected.reference.selector,uiLocale()) : '';
      if (quote) {
        const excerpt = element('blockquote','cr-quote',quote);
        excerpt.dataset.reference = JSON.stringify(selected.reference);
        body.append(excerpt);
        const sourceVersion = activeVersion(selected.versionId);
        const address = [documentTitle(state.document), sourceVersion ? versionLabel(sourceVersion) : '', unitLabel(selected.unit),
          `${selected.reference.selector.start}–${selected.reference.selector.end} ${t('characters')}`].filter(Boolean).join(' · ');
        body.append(element('p','cr-philology-context-address',address));
      }
    }
    const current = selected?.unit || currentWindow(state.activeVersionId)?.units?.[0];
    if (current && graphTarget(current)) body.append(button(t('graph'), () => openGraph(current), 'cr-graph-action'));
  }

  function renderNotes(body, state) {
    body.append(element('h2', 'cr-inspector-title', t('notes')));
    if (!selected?.reference) { body.append(element('p', 'cr-muted', t('noNotes'))); renderNotebookList(body); renderTransfer(body); return; }
    const selectedNote = selectedRecord();
    const rawQuote=selected.quote||selectedNote?.quote;
    const full=selected.unit?quotationWithEditorialRoles(activeVersion(selected.versionId),selected.unit,selected.reference.selector,uiLocale()):rawQuote;
    const quote=full?.slice(0,READER_LIMITS.quote);
    if(quote){const excerpt=element('blockquote','cr-quote',quote);excerpt.dataset.reference=JSON.stringify(selected.reference);body.append(excerpt);
      if(full.length>quote.length)body.append(element('p','cr-muted',uiLocale()==='en'?'Preview shortened; Copy quotation includes the full selection.':'Предпросмотр сокращён; кнопка копирует выделенный фрагмент целиком.'));
    }
    const tools = element('div', 'cr-note-tools');
    const isBookmarked = notebookItems().bookmarks.some(item => sameNoteReference(item, selected.reference));
    tools.append(button(isBookmarked ? '◆' : '◇', async event => {
      const trigger = event.currentTarget;
      trigger.disabled = true;
      try {
        await model.toggleBookmark(selected.reference, selected.quote || '');
        announce(notebookSaveLabel());
        renderInspector();
      } catch (error) {
        announce(error.message === 'quote-limit' ? t('error') : t('error'));
        trigger.disabled = false;
      }
    }, 'cr-bookmark'));
    tools.lastChild.setAttribute('aria-label', t('bookmark'));
    tools.append(button(t('copy'), copySelected, 'cr-copy')); body.append(tools);
    const label = element('label', 'cr-note-label', `${t('addNote')} · ${unitLabel(selected.unit)}`);
    noteEditor = element('textarea', 'cr-note-editor'); noteEditor.rows = 7; noteEditor.maxLength = READER_LIMITS.note; noteEditor.placeholder = t('notePlaceholder'); noteEditor.value = draft?.key===referenceKey(selected.reference)&&draft.recordId===selected.noteId?draft.text:selectedNote?.text || '';
    noteEditor.addEventListener('input', () => {
      selectedRecord();
      draft={key:referenceKey(selected.reference),text:noteEditor.value,recordId:selected.noteId,recordRevision:selected.noteRevision};noteDirty = true;
      try{draftJournal.capture({reference:selected.reference,text:draft.text,quote:selected.quote||'',originalNoteId:draft.recordId});}
      catch(error){announce(friendlyError(error));}
      clearTimeout(noteTimer);noteTimer=setTimeout(flushNote,450);updateFooter();
    }); label.append(noteEditor); body.append(label);
    const actions = element('div', 'cr-note-actions');
    actions.append(button(t('save'), async () => {
      if (await flushNote()) { announce(notebookSaveLabel()); renderInspector(); updateFooter(); }
    }), button(t('delete'), async () => {
      if (!(await flushNote())) return;
      selectedRecord();
      try{if(selected.noteId)await model.deleteNote(selected.reference,{id:selected.noteId,revision:selected.noteRevision});}catch(error){announce(error.code==='conflict'?(uiLocale()==='en'?'This note changed in another window.':'Заметка изменилась в другом окне.'):t('saveError'));return;}
      selected = {...selected, noteId:null, noteRevision:undefined, quote: ''};
      announce(notebookSaveLabel());
      renderInspector();
    }));
    body.append(actions);
    renderNotebookList(body);
    renderTransfer(body);
  }

  function renderNotebookList(body) {
    const generation = ++notebookListGeneration;
    const data=notebookItems(),page=model.notebookState();
    const details=element('details','cr-notebook-list');details.open=true;
    details.append(element('summary','',`${t('notes')} (${data.notes.length+data.bookmarks.length})`));
    for(const [index,item] of [...data.notes,...data.bookmarks].reverse().entries()){
      const ref=item.reference;
      const documentId=notebookReferenceWorkId(ref);
      const catalogDocument=documentId ? modelState().catalog.items.find(document=>document.id===documentId) : null;
      const catalogVersion=catalogDocument?.versions?.find(version=>version.id===ref?.versionId);
      const selector=ref?.selector;
      const locator=Number.isSafeInteger(selector?.start)&&Number.isSafeInteger(selector?.end)&&selector.end>selector.start
        ? `${selector.start}–${selector.end} ${t('characters')}` : '';
      const contextLabel=catalogDocument ? [documentTitle(catalogDocument),catalogVersion?versionLanguage(catalogVersion):'',locator].filter(Boolean).join(' · ') : '';
      const prefix=item.kind==='bookmark'?'◆ ':'';
      const fallbackLabel=notebookFallbackLabel(item,{index,locale:uiLocale(),passage:t('passage'),rangeLabel:t('characters')});
      const label=element('strong','',`${prefix}${contextLabel||fallbackLabel}`);
      const entry=button('',async()=>{
        if(!(await flushNote()))return;selectionSequence++;capturePanePositions();
        if(ref?.schemaVersion==='tos.corpus.reader.native-reference.v1'){await close();await onNativeReference?.(ref,item);return;}
        try{if(!documentId)throw new Error('invalid-reference');const openedReference=exactReference(ref);if(!openedReference)throw new Error('invalid-reference');
          const opened=await model.open({documentId,versionId:ref.versionId,reference:openedReference});
          const represented=modelState().document?.id===documentId&&activeVersion(ref.versionId);
          if(represented)emitLocation(openedReference);
          const noteTarget=item.kind==='note'?{noteId:item.id,noteRevision:item.revision}:{};
          if(!opened){selected={reference:openedReference,unit:null,versionId:ref.versionId,quote:item.quote||'',...noteTarget};render();return;}
          await ensureVisiblePanes();
          selected={reference:openedReference,unit:currentWindow(ref.versionId)?.units?.find(unit=>unit.id===ref.unitId)||null,versionId:ref.versionId,quote:item.quote||'',...noteTarget};render();revealSelectedSource();}
        catch(error){announce(t('error'));}
      },'cr-note-entry');entry.append(label,element('span','',item.text||item.quote||t('bookmark')));details.append(entry);
      if(documentId && (!catalogDocument || !catalogDocument.versions?.length)){
        void notebookMetadataResolver.resolve(documentId).then(document=>{
          if(!document || disposed || !opened || generation!==notebookListGeneration || !entry.isConnected)return;
          const version=document.versions?.find(itemVersion=>itemVersion.id===ref?.versionId);
          const resolved=[documentTitle(document),version?versionLanguage(version):'',locator].filter(Boolean).join(' · ');
          if(resolved)label.textContent=`${prefix}${resolved}`;
        }).catch(()=>{});
      }
    }
    if(model.notebookPage){
      if(page.nextCursor)details.append(button(t('loadMore'),async()=>{if(!(await flushNote()))return;try{await model.notebookPage({cursor:page.nextCursor});renderInspector();}catch{announce(t('error'));}},'cr-more'));
      if(page.cursor)details.append(button(uiLocale()==='en'?'First notes':'К первым записям',async()=>{if(!(await flushNote()))return;await model.notebookPage();renderInspector();},'cr-more'));
    }
    body.append(details);
  }

  // Refresh the list after an autosave without replacing the editor or its
  // draft/record identity. If the list itself owns focus, defer replacement
  // until the user leaves it.
  function refreshNotebookList() {
    const previous=root?.querySelector('.cr-notebook-list');
    if(!previous || !opened || disposed || currentTab!=='notes')return;
    const items=notebookItems();
    const summary=previous.querySelector('summary');
    if(summary)summary.textContent=`${t('notes')} (${items.notes.length+items.bookmarks.length})`;
    if(previous.contains(document.activeElement)){
      previous.addEventListener('focusout',()=>queueMicrotask(refreshNotebookList),{once:true});
      return;
    }
    const holder=element('div');renderNotebookList(holder);
    const replacement=holder.firstElementChild;
    if(replacement){replacement.open=previous.open;previous.replaceWith(replacement);}
  }

  function renderTransfer(body) {
    const transfer = element('div', 'cr-transfer');
    transfer.append(button(t('export'), async () => {
      try { if(!(await flushNote()))return;const packet = await model.exportNotebook(); download('tos-reader-notebook.json', typeof packet === 'string' ? packet : JSON.stringify(packet), 'application/json'); }
      catch { announce(t('error')); }
    }, 'cr-transfer-button'));
    const file = element('input', 'cr-import-file'); file.type = 'file'; file.accept = '.json,application/json'; file.hidden = true;
    file.addEventListener('change', async () => {
      const picked = file.files?.[0]; if (!picked) return;
      if(picked.size>32*1024*1024){announce(uiLocale()==='en'?'The import file exceeds 32 MiB.':'Файл импорта превышает 32 МиБ.');return;}
      if(!(await flushNote()))return;
      try {
        await model.importNotebook(await picked.text());
        await model.ready?.();
        announce(notebookSaveLabel());
        render();
      } catch { announce(t('error')); }
    });
    transfer.append(button(t('import'), () => file.click(), 'cr-transfer-button'), file); body.append(transfer);
  }

  function renderSource(body, state) {
    body.append(element('h2', 'cr-inspector-title', t('source')));
    for (const version of state.document?.versions || []) {
      const status = isTextAvailable(version) ? '' : catalogAvailability(version);
      const section = element('section', 'cr-source'); section.append(element('h3', '', [editionRepresentationLabel(version),versionLanguage(version), localized(version.edition, uiLocale()), status].filter(Boolean).join(' · ') || t('edition')));
      const sourceUrl = safeExternalUrl(attr(version.sourceUrl || version.source_url || version.source?.url || version.source?.href));
      if (version.attribution) section.append(element('p', '', localized(version.attribution, uiLocale())));
      if (sourceUrl) { const link = element('a', '', sourceLinkLabel(sourceUrl, uiLocale())); link.href = sourceUrl; link.target = '_blank'; link.rel = 'noopener noreferrer'; section.append(link); }
      const licenseUrl = safeExternalUrl(attr(version.license?.url));
      if (licenseUrl) {
        const notice = element('p', '', uiLocale() === 'en' ? 'Source text projection · ' : 'Текстовая проекция источника · ');
        const link = element('a', '', localized(version.license?.label, uiLocale()) || (uiLocale() === 'en' ? 'License' : 'Лицензия'));
        link.href = licenseUrl; link.target = '_blank'; link.rel = 'noopener noreferrer'; notice.append(link); section.append(notice);
      }
      body.append(section);
    }
  }

  function renderInspector() {
    const state = modelState(); const inspector = root.querySelector('.cr-inspector'); if (!inspector) return;
    noteEditor = null; inspector.replaceChildren();
    const tabs = element('nav', 'cr-inspector-tabs');
    for (const name of ['context', 'word', 'notes', 'source']) { const b = button(t(name), async () => {
      if(!(await flushNote()))return;
      currentTab = name;
      if (name === 'word' && selected?.reference && !philology.busy && (!philology.result || philology.error)) void loadPhilology();
      else renderInspector();
    }, 'cr-tab'); b.setAttribute('aria-pressed', String(currentTab === name)); tabs.append(b); }
    inspector.append(tabs); const body = element('div', 'cr-inspector-body'); inspector.append(body);
    if (currentTab === 'notes') renderNotes(body, state);
    else if (currentTab === 'source') renderSource(body, state);
    else if (currentTab === 'word') {
      const version = activeVersion(selected?.versionId || state.activeVersionId);
      const reference = selected?.reference || null;
      const form = selected?.unit ? selectedPhilologyForm(selected.unit.text, reference) : '';
      renderPhilology(body, {
        locale:uiLocale(),form,language:version?.language || selected?.unit?.language || '',reference,version,
        result:philology.result,busy:philology.busy,concordance:philology.concordance,
        searchAvailable:model.searchCapabilities().version,
        onConcordance:value=>void findPhilologyConcordance(value),
        onMore:()=>void findPhilologyConcordance(form,philology.concordance?.nextCursor),
        onOccurrence:hit=>void openPhilologyOccurrence(hit),
        onMappingTarget:mapping=>void openPhilologyTarget(mapping),
      });
      if (philology.error) body.append(element('p','cr-philology-muted',t('error')));
    } else renderContext(body, state);
  }

  function renderSettings() {
    const data = model.notebookState(); const preferences = data.preferences || {};
    const settings = element('section', 'cr-settings');
    const adjust = (label, values, key) => { const row = element('div', 'cr-setting'); row.append(element('span', '', label)); for (const value of values) { const labelValue=({comfortable:uiLocale()==='en'?'Comfortable':'Обычная',wide:uiLocale()==='en'?'Wide':'Широкая',night:t('night'),paper:t('paper')})[value]||String(value);const b = button(labelValue, () => { model.setPreferences({[key]: value}); render(); }, 'cr-setting-button'); b.dataset.focusKey=`setting:${key}:${value}`;b.setAttribute('aria-pressed', String(preferences[key] === value)); row.append(b); } settings.append(row); };
    adjust(t('font'), [16, 18, 20, 22], 'fontSize'); adjust(t('leading'), [1.6, 1.9, 2.2], 'lineHeight'); adjust(t('width'), ['comfortable', 'wide'], 'width'); adjust(t('theme'), ['night', 'paper'], 'theme'); return settings;
  }

  function updateFooter() {
    const footer = root.querySelector('.cr-footer'); if (!footer) return;
    footer.querySelector('.cr-location').textContent = locationLabel(currentRef());
    const save = footer.querySelector('.cr-save');
    save.textContent = notebookSaveLabel();
    save.title = notebookSaveTitle();
    const status = footer.querySelector('.cr-status'); status.textContent = lastMessage; status.hidden = !lastMessage;
  }

  function renderFooter() {
    const footer = element('footer', 'cr-footer');
    const save = element('span', 'cr-save', notebookSaveLabel());
    save.title = notebookSaveTitle();
    footer.append(element('span', 'cr-location', ''), element('span', 'cr-status', lastMessage), save);
    footer.querySelector('.cr-status').hidden = !lastMessage;
    return footer;
  }

  function render(snapshot) {
    if (!opened || disposed) return;
    // Share one detached snapshot within this synchronous render. Event and
    // async callbacks still read fresh state after the render has returned.
    const previous = renderSnapshot;
    renderSnapshot = snapshot ?? model.snapshot();
    try { renderContents(); } finally { renderSnapshot = previous; }
  }

  function renderContents() {
    const focused=document.activeElement;const focusClass=['cr-library-search','cr-search-input','cr-search-scope','cr-search-version','cr-note-editor'].find(name=>focused?.classList?.contains(name));
    const focusKey=focused?.dataset?.focusKey;
    const selectionStart=focused?.selectionStart,selectionEnd=focused?.selectionEnd;
    const visualAnchors=[];
    for(const pane of root.querySelectorAll('.cr-pane')){
      if(pane.dataset.windowStart===currentWindow(pane.dataset.versionId)?.units?.[0]?.id){
        remember(panePositions,pane.dataset.versionId,pane.scrollTop);
        const row=visiblePaneRow(pane),rect=row?.getBoundingClientRect();
        const boundary=Math.max(pane.getBoundingClientRect().top,pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??0)+2;
        if(row && rect?.height){const offset=rect.top-boundary;visualAnchors.push({versionId:pane.dataset.versionId,unitId:row.dataset.unitId,
          offset:offset>=0?offset:null,fraction:offset<0?Math.min(1,Math.max(0,-offset/rect.height)):0});}
      }else panePositions.delete(pane.dataset.versionId);
    }
    rendering=true;
    const state = modelState(); renderSerial += 1; const serial = renderSerial;
    root.replaceChildren(); root.className = `corpus-reader cr-theme-${model.notebookState().preferences?.theme || 'night'}`; root.dataset.mode = mode; root.dataset.library = String(libraryOpen); root.dataset.inspector = String(inspectorOpen);
    root.setAttribute('aria-label', `${t('reading')}${state.document ? ` · ${documentTitle(state.document)}` : ''}`);
    root.style.setProperty('--cr-font-size', `${model.notebookState().preferences?.fontSize || 18}px`);
    root.style.setProperty('--cr-leading', `${model.notebookState().preferences?.lineHeight || 1.9}`);
    root.style.setProperty('--cr-width', model.notebookState().preferences?.width === 'wide' ? '980px' : '740px');
    root.append(renderHeader(state));
    if (!state.document) {
      const empty=element('main','cr-empty-state');const error=state.catalog.error||state.error;
      const message=state.catalog.busy?t('loading'):error?.code==='provider_unavailable'?t('providerUnavailable'):error?t('error'):t('noCatalog');
      const disconnected=error?.code==='provider_unavailable';
      if(disconnected)empty.append(element('h2','',t('library')),element('p','',message));
      else if(error)empty.append(element('p','',message));
      if(error&&error.code!=='provider_unavailable')empty.append(button(uiLocale()==='en'?'Retry':'Повторить',()=>loadInitial()));
      // An empty first page can still carry an opaque continuation. Keep the
      // library visible so the user can advance one bounded page explicitly.
      if(!disconnected)empty.append(renderLibrary(state));
      root.append(empty);rendering=false;return;
    }
    const versions = chooseVersions(state.document, state);
    root.append(renderVersionBar(state, versions));
    if (searchOpen) root.append(renderSearch(state));
    if (settingsOpen) root.append(renderSettings());
    const layout = element('div', 'cr-layout'); if (libraryOpen) layout.append(renderLibrary(state));
    const main = element('main', 'cr-main');
    if (versions.length > 1) {
      const independent = element('p', 'cr-comparison-note', t('independentColumns'));
      independent.setAttribute('role', 'note');
      main.append(independent);
    }
    const panes = element('div', 'cr-panes');
    if (versions.length > 1) panes.classList.add('cr-parallel');
    for (const version of versions) panes.append(renderPane(version, state));
    main.append(panes); layout.append(main);
    if (inspectorOpen) layout.append(element('aside', 'cr-inspector'));
    root.append(layout, renderFooter()); renderInspector(); restorePanePositions();
    for(const anchor of visualAnchors){
      const pane=[...root.querySelectorAll('.cr-pane')].find(item=>item.dataset.versionId===anchor.versionId);
      const row=pane&&[...pane.querySelectorAll('.cr-unit')].find(item=>item.dataset.unitId===anchor.unitId);
      if(!row)continue;
      const boundary=Math.max(pane.getBoundingClientRect().top,pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??0)+2;
      const rect=row.getBoundingClientRect();
      pane.scrollTop+=anchor.offset===null?rect.top+anchor.fraction*rect.height-boundary:rect.top-boundary-anchor.offset;
    }
    for(const pane of root.querySelectorAll('.cr-pane'))refreshPaneLocation(pane,activeVersion(pane.dataset.versionId));
    markSelection();
    const location = root.querySelector('.cr-location'); if (location) { location.textContent = locationLabel(currentRef()); }
    rendering=false;
    const focusTarget=focusClass?root.querySelector('.'+focusClass):focusKey
      ? [...root.querySelectorAll('[data-focus-key]')].find(item=>item.dataset.focusKey===focusKey && item.offsetParent!==null) : null;
    if(focusTarget){focusTarget.focus({preventScroll:true});if(Number.isInteger(selectionStart)&&typeof focusTarget.setSelectionRange==='function')focusTarget.setSelectionRange(selectionStart,selectionEnd);}
    else if (serial === renderSerial && !root.contains(document.activeElement)) root.querySelector('.cr-pane, .cr-library-search, .cr-close')?.focus({preventScroll:true});
  }

  async function loadInitial(args = {}) {
    try {
      await model.ready?.();
      if (!modelState().catalog.items.length) await model.catalog({query: ''});
      // Do not turn an empty intermediate catalog page into an implicit scan.
      // The rendered library exposes its next cursor for an explicit action.
      if (!args.documentId && !modelState().catalog.items.length) { render(); return; }
      const request = {documentId: args.documentId, versionId: args.versionId, unitId: args.unitId, revision: args.revision};
      if (args.reference) request.reference = args.reference;
      const openedResult = await model.open(request);
      const state = modelState();
      if (!state.document) { render(); return; }
      await ensureVisiblePanes();
      const stale = state.referenceStatus === 'changed' || state.referenceStatus === 'unavailable';
      if (args.reference && !stale) {
        const reference = exactReference(args.reference);
        const unit = reference && currentWindow(reference.versionId)?.units?.find(value => (value.unitId || value.id) === reference.unitId);
        if (reference && unit) { selected={reference,unit,versionId:reference.versionId,quote:selectedPhilologyForm(unit.text,reference)}; currentTab='context'; }
      }
      const firstUnit = openedResult && !stale ? currentWindow(state.activeVersionId)?.units?.[0] : null;
      const requestedUnit = args.unitId && !stale ? currentWindow(state.activeVersionId)?.units?.find(value => (value.unitId||value.id)===args.unitId) : null;
      const location = args.reference && selected?.reference || (requestedUnit||firstUnit) && unitReference(requestedUnit||firstUnit, activeVersion(state.activeVersionId), state.document, {});
      if (location) emitLocation(location);
      render();
      if (args.reference && !stale) revealSelectedSource();
      else if(requestedUnit){
        const pane=[...root.querySelectorAll('.cr-pane')].find(item=>item.dataset.versionId===state.activeVersionId);
        const row=pane&&[...pane.querySelectorAll('.cr-unit')].find(item=>item.dataset.unitId===args.unitId);
        row?.scrollIntoView({block:'start',inline:'nearest'});
        if(row){const boundary=pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??pane.getBoundingClientRect().top;
          if(row.getBoundingClientRect().top<boundary)pane.scrollTop-=boundary-row.getBoundingClientRect().top+2;
          refreshPaneLocation(pane,activeVersion(state.activeVersionId));}
      }
    } catch (error) { announce(friendlyError(error)); render(); }
  }

  // Navigate an explicitly chosen visible comparison pane without promoting it
  // to the primary edition or disturbing the other pane's reading position.
  async function openPanePassage({documentId,versionId,unitId,revision} = {}) {
    const state=modelState(),version=activeVersion(versionId);
    if(!opened || disposed || documentId!==state.document?.id || !version || !unitId
      || !chooseVersions(state.document,state).some(item=>item.id===versionId))return false;
    if(revision && version.revision!==revision){announce(t('changedCurrent'));return false;}
    const sequence=++selectionSequence;
    if(!(await flushNote()) || disposed || sequence!==selectionSequence)return false;
    await capturePanePositions();
    if(disposed || sequence!==selectionSequence || modelState().document?.id!==documentId
      || !chooseVersions(modelState().document,modelState()).some(item=>item.id===versionId))return false;
    try{
      const stillCurrent=()=>!disposed && sequence===selectionSequence && modelState().document?.id===documentId
        && chooseVersions(modelState().document,modelState()).some(item=>item.id===versionId);
      const page=await model.loadWindow({documentId,versionId,unitId,revision,commitGuard:stillCurrent});
      if(disposed || sequence!==selectionSequence || modelState().document?.id!==documentId
        || !chooseVersions(modelState().document,modelState()).some(item=>item.id===versionId))return false;
      const unit=page?.units?.find(item=>(item.unitId||item.id)===unitId);
      if(!unit){announce(modelState().referenceStates[stateKey(documentId,versionId)]==='changed'?t('changedCurrent'):t('error'));return false;}
      selected=null;panePositions.delete(versionId);
      const ref=unitReference(unit,version,modelState().document,{});if(ref)emitLocation(ref);
      render();
      const pane=[...root.querySelectorAll('.cr-pane')].find(item=>item.dataset.versionId===versionId);
      const row=pane&&[...pane.querySelectorAll('.cr-unit')].find(item=>item.dataset.unitId===unitId);
      row?.scrollIntoView({block:'start',inline:'nearest'});
      if(row){const boundary=pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??pane.getBoundingClientRect().top;
        if(row.getBoundingClientRect().top<boundary)pane.scrollTop-=boundary-row.getBoundingClientRect().top+2;
        refreshPaneLocation(pane,version);pane.focus({preventScroll:true});}
      return Boolean(row);
    }catch(error){announce(friendlyError(error));return false;}
  }

  async function close({notify = true} = {}) {
    if (!opened || disposed) return true;
    if(!(await flushNote()))return false;
    if(disposed || !root)return true;
    await capturePanePositions();
    if(disposed || !root)return true;
    opened=false; selectionSequence++; searchCancel?.(); searchGeneration++; root.hidden=true; delete root.dataset.open;philologyController?.abort();philologyController=null;model.cancel();
    if(notify)onClose?.();
    const focus=opener;opener=null;
    if(focus?.isConnected&&focus!==document.body&&!focus.closest('[hidden], [inert]')&&focus.offsetParent!==null&&!focus.disabled)focus.focus?.({preventScroll:true});
    if(!focus || document.activeElement!==focus)onReturnFocusFallback?.();
    return true;
  }

  function focusables() {
    return [...root.querySelectorAll('button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),a[href]')].filter(item => !item.closest('[hidden]'));
  }

  function keydown(event) {
    if (!opened) return;
    event.stopPropagation();
    if (event.key === 'Escape') { event.stopPropagation(); event.preventDefault(); close(); return; }
    if (event.key !== 'Tab') return;
    event.stopPropagation();
    const items = focusables(); if (!items.length) { event.preventDefault(); root.focus(); return; }
    const firstItem = items[0], lastItem = items.at(-1);
    if (event.shiftKey && document.activeElement === firstItem) { event.preventDefault(); lastItem.focus(); }
    else if (!event.shiftKey && document.activeElement === lastItem) { event.preventDefault(); firstItem.focus(); }
  }

  root.addEventListener('keydown', keydown);
  root.addEventListener('click', event => { if (event.target === root) close(); });
  document.addEventListener('selectionchange', onSelectionChange);
  const beforeUnload=event=>{if(noteDirty||noteFlushPromise){void flushNote();event.preventDefault();event.returnValue='';}};
  const visibilityChanged=()=>{if(document.visibilityState==='hidden')void flushNote();};
  window.addEventListener('beforeunload',beforeUnload);
  document.addEventListener('visibilitychange',visibilityChanged);

  return {
    get element() { return root; },
    async open(args = {}) {
      if(disposed)return this;
      await model.ready?.();
      if(disposed || !(await flushNote()) || disposed || !root)return this;
      if(opened)await capturePanePositions();
      if(disposed || !root)return this;
      const wasOpened=opened;
      if(!wasOpened)opener = args.returnFocus || document.activeElement;
      opened = true; root.hidden = false; root.dataset.open = 'true';
      if (args.mode === 'parallel' || args.mode === 'single') mode = args.mode;
      const hasTarget = Boolean(args.documentId || args.versionId || args.reference || args.unitId || args.revision);
      if (hasTarget) { selectionSequence++; selected=null; searchCancel?.(); searchGeneration++; searchVersionId=null;searchQuery=''; searchResult={items:[]}; }
      // Keep the last usable window during a new request. Rebuilding the same
      // old panes here forces layout before the incoming window is rendered.
      if(!wasOpened || !hasTarget)render();
      const current = modelState(); const remembered = model.notebookState().active;
      if (!hasTarget && current.document) return Promise.resolve(this);
      const rememberedReferenceValue = remembered
        ? model.notebookState().positions?.find(item => {
          const reference = item.reference || item.anchor;
          return isReference(reference) && reference.target.workId === remembered.documentId && reference.versionId === remembered.version;
        })?.reference
        : null;
      const initial = hasTarget
        ? args
        : remembered
          ? {documentId: remembered.documentId, versionId: remembered.version, ...(rememberedReferenceValue ? {reference: rememberedReferenceValue} : {})}
          : {};
      return loadInitial(initial).then(() => this);
    },
    close,
    openPane: openPanePassage,
    visibleVersions(state = modelState()) {
      return state.document ? chooseVersions(state.document,state).map(version=>({id:version.id,label:[editionRepresentationLabel(version),versionLabel(version)].filter(Boolean).join(' · ')})) : [];
    },
    destroy() {
      if(destroyWork)return destroyWork;
      clearTimeout(noteTimer);
      for(const timer of paneSaveTimers.values())clearTimeout(timer);paneSaveTimers.clear();
      // pagehide is best effort: capture while the panes still exist, then
      // retain the DOM and notebook until both note and position writes settle.
      const positionSaving=opened?capturePanePositions():Promise.resolve();
      const saving=Promise.allSettled([flushNote(),positionSaving]);
      disposed=true;opened=false;philologyController?.abort();philologyController=null;notebookListGeneration+=1;notebookMetadataResolver.dispose();catalogCancel?.();searchCancel?.();
      document.removeEventListener('selectionchange',onSelectionChange);document.removeEventListener('visibilitychange',visibilityChanged);
      window.removeEventListener('beforeunload',beforeUnload);
      destroyWork=saving.finally(()=>{root?.remove();root=null;model.destroy();});return destroyWork;
    },
    isOpen: () => opened,
    refreshLocale() {
      if (!opened || disposed) return;
      const selection=window.getSelection(); let saved=null;
      if (selection?.rangeCount && !selection.isCollapsed) {
        const range=selection.getRangeAt(0), elementOf=node=>node.nodeType===1?node:node.parentElement;
        const row=elementOf(range.startContainer)?.closest('.cr-unit');
        const endRow=elementOf(range.endContainer)?.closest('.cr-unit');
        const content=row?.querySelector('.cr-unit-text');
        if (row && row===endRow && content?.contains(range.startContainer) && content.contains(range.endContainer)) {
          const before=range.cloneRange(); before.selectNodeContents(content); before.setEnd(range.startContainer,range.startOffset);
          saved={key:row.dataset.reference,start:before.toString().length,end:before.toString().length+range.toString().length};
        }
      }
      lastMessage=''; render();
      if (saved) {
        const row=[...root.querySelectorAll('.cr-unit')].find(item=>item.dataset.reference===saved.key), content=row?.querySelector('.cr-unit-text');
        if (content) { const walker=document.createTreeWalker(content,NodeFilter.SHOW_TEXT); let node,offset=0,start=null,end=null;
          while ((node=walker.nextNode())) { const next=offset+node.data.length; if (!start && saved.start<=next) start=[node,saved.start-offset]; if (saved.end<=next) {end=[node,saved.end-offset]; break;} offset=next; }
          if (start && end) {const range=document.createRange(); range.setStart(...start); range.setEnd(...end); selection.removeAllRanges(); selection.addRange(range);}
        }
      }
    },
    async refreshNotebook() {
      if (disposed || !(await flushNote())) return false;
      await model.refreshNotebook();
      if (opened) render();
      return true;
    },
    selectedSource() {
      if (!selected?.reference || !selected.unit) return null;
      const state = modelState();
      const version = activeVersion(selected.versionId);
      const document = state.document;
      if (!version || !document || selected.reference.target?.workId !== document.id) return null;
      const context = selectedSourceContext({reference:selected.reference,version,unit:selected.unit});
      if (!context) return null;
      return {
        reference: exactReference(selected.reference),
        quote: selectedPhilologyForm(selected.unit.text, selected.reference),
        text: String(selected.unit.text || ''),
        ...context,
        language: version.language || selected.unit.language || '',
        documentId: document.id,
        versionId: version.id,
        title: [documentTitle(document), paneUnitLabel(selected.unit,version)].filter(Boolean).join(' · '),
        versionLabel: versionLabel(version),
        edition: localized(version.edition, uiLocale()),
        source: version.source ? structuredClone(version.source) : null,
      };
    },
    state: () => model.snapshot(),
  };
}
