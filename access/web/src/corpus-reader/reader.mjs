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
  searchUnits,
  unitReference,
} from './view-model.mjs';
import {languageName,sourceLinkLabel} from '../observatory/human-presentation.mjs';

const WORDING = {
  ru: {
    reading: 'Чтение', close: 'Вернуться к Древу', library: 'Корпус', find: 'Найти произведение', loadMore: 'Ещё',
    previous: 'Предыдущий фрагмент', next: 'Следующий фрагмент', one: 'Один текст', parallel: 'Рядом', compare: 'Сравнить с',
    search: 'Поиск', searchPlaceholder: 'Искать в открытом тексте', loaded: 'Загруженный фрагмент', version: 'Всё произведение',
    corpus: 'Весь корпус', noSearch: 'Совпадений нет', searchUnavailable: 'Поиск недоступен для этого источника',
    notes: 'Блокнот', context: 'Контекст', source: 'Источник', settings: 'Вид текста', bookmark: 'Закладка', bookmarked: 'Закладка снята',
    addNote: 'Оставить заметку', notePlaceholder: 'Что открылось здесь? Вопрос, связь, возражение…', save: 'Сохранить', delete: 'Удалить',
    copy: 'Скопировать цитату', copied: 'Цитата и атрибуция скопированы', graph: 'Открыть в Древе', export: 'Скачать блокнот', import: 'Импортировать блокнот',
    noNotes: 'Выберите фрагмент, чтобы оставить заметку или закладку.', saved: 'Сохранено на этом устройстве', pending: 'Сохраняется…', memory: 'Только в памяти', session: 'Только на время сеанса', saveError: 'Не удалось сохранить', providerUnavailable: 'Источник полных текстов пока не подключён.',
    changed: 'Версия источника изменилась. Старая закладка сохранена.', unavailable: 'Полный текст недоступен',
    changedCurrent: 'Сохранённая привязка относится к другой версии текста.', unavailableCurrent: 'Этот фрагмент больше недоступен.', openCurrent: 'Открыть текущую версию',
    empty: 'В этой области пока нет текста.', loading: 'Загрузка…', error: 'Не удалось загрузить фрагмент',
    previousChunk: 'Назад', nextChunk: 'Дальше', edition: 'Издание', provenance: 'Происхождение текста', language: 'Язык', original: 'Оригинал', translation: 'Перевод',
    unknown: 'не указано', continuation: 'Есть продолжение', metadataOnly: 'Только метаданные', restricted: 'Доступ ограничен', unavailableVersion: 'Текст недоступен', availabilityUnknown: 'Доступность уточняется',
    untitled: 'Без названия', units: 'Единиц', passage: 'Фрагмент',
    font: 'Размер', leading: 'Интервал', width: 'Ширина', theme: 'Тема', paper: 'Бумага', night: 'Ночь',
    select: 'Выбрать фрагмент в тексте', noCatalog: 'Произведения не найдены', works: 'Произведений',
  },
  en: {
    reading: 'Reading', close: 'Return to the tree', library: 'Corpus', find: 'Find a work', loadMore: 'More',
    previous: 'Previous passage', next: 'Next passage', one: 'One text', parallel: 'Side by side', compare: 'Compare with',
    search: 'Search', searchPlaceholder: 'Search the open text', loaded: 'Loaded passage', version: 'This work',
    corpus: 'Entire corpus', noSearch: 'No matches', searchUnavailable: 'Search is unavailable for this source',
    notes: 'Notebook', context: 'Context', source: 'Source', settings: 'Text appearance', bookmark: 'Bookmark', bookmarked: 'Bookmark removed',
    addNote: 'Leave a note', notePlaceholder: 'What opened here? A question, connection, objection…', save: 'Save', delete: 'Delete',
    copy: 'Copy quotation', copied: 'Quotation and attribution copied', graph: 'Open in the tree', export: 'Download notebook', import: 'Import notebook',
    noNotes: 'Choose a passage to leave a note or bookmark.', saved: 'Saved on this device', pending: 'Saving…', memory: 'Memory only', session: 'This session only', saveError: 'Could not save', providerUnavailable: 'The full-text source is not connected yet.', changed: 'The source version changed. The old bookmark is kept.',
    unavailable: 'Full text unavailable', empty: 'There is no text in this area yet.', loading: 'Loading…', error: 'Could not load this passage',
    changedCurrent: 'The saved anchor belongs to another text version.', unavailableCurrent: 'This passage is no longer available.', openCurrent: 'Open the current version',
    previousChunk: 'Previous', nextChunk: 'Next', edition: 'Edition', provenance: 'Text provenance', language: 'Language', original: 'Original', translation: 'Translation',
    unknown: 'unknown', continuation: 'More pages available', metadataOnly: 'Metadata only', restricted: 'Restricted', unavailableVersion: 'Text unavailable',
    font: 'Type size', leading: 'Line spacing', width: 'Line width', theme: 'Theme', paper: 'Paper', night: 'Night',
    select: 'Select a passage in the text', noCatalog: 'No works found', works: 'Works', availabilityUnknown: 'Availability is being checked',
    untitled: 'Untitled', units: 'Units', passage: 'Passage',
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
  const title = localized(document?.title, locale) || document?.id || '';
  const work = localized(document?.work, locale);
  const edition = localized(version?.edition, locale) || version?.label || version?.id || '';
  const locator = localized(unit?.label || version?.locator || document?.locator, locale) || unit?.sourceReference || unit?.unitId || '';
  const source = safeExternalUrl(attr(version?.sourceUrl || version?.source_url || document?.sourceUrl));
  return [quote ? `«${quote}»` : '', `${title}${work && work !== title ? ` · ${work}` : ''}`, edition, locator, source].filter(Boolean).join('\n');
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
  let currentTab = 'context';
  let searchOpen = false;
  let searchScope = 'loaded';
  let searchQuery = '';
  let searchResult = {items: []};
  let comparison = new Map();
  let panePositions = new Map();
  let paneSaveTimers = new Map();
  let paneSaveWork = Promise.resolve();
  let selected = null;
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

  const t = key => WORDING[uiLocale()][key] || key;
  const modelState = () => renderSnapshot ?? model.snapshot();
  const activeDocument = () => modelState().document;
  const activeVersion = versionId => activeDocument()?.versions?.find(item => item.id === versionId) || null;
  const documentTitle = (document, aliases = []) => {
    const value = localized(document?.title, uiLocale());
    return value && ![document?.id, ...aliases].includes(value) ? value : t('untitled');
  };
  const versionLanguage = version => readableLanguage(version?.language, uiLocale());
  const unitLabel = unit => {
    if (typeof unit?.label === 'string' && unit.label.trim()) return unit.label;
    if (Number.isSafeInteger(unit?.ordinal) && unit.ordinal >= 0) return `${t('passage')} ${unit.ordinal + (unit.ordinal === 0 ? 1 : 0)}`;
    return t('passage');
  };
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
  const currentIdentity = versionId => {
    const state = modelState();
    const document = state.document;
    const version = document?.versions?.find(item => item.id === (versionId === undefined ? state.activeVersionId : versionId));
    return {documentId: document?.id || '', versionId: version?.id || '', sourceRevision: version?.sourceRevision ?? null, contentRevision: version?.contentRevision ?? null};
  };
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
    const exact = exactReference(reference, currentIdentity(reference.versionId));
    if (!exact) return;
    onLocation({
      documentId: exact.target.workId,
      versionId: exact.versionId,
      unitId: exact.unitId,
      revision: exact.target.textLayerSha256,
    });
  }

  function capturePanePositions() {
    if (disposed || !root) return paneSaveWork;
    const writes=[],document=modelState().document;
    for (const [index, pane] of [...root.querySelectorAll('.cr-pane')].entries()) {
      remember(panePositions,pane.dataset.versionId, pane.scrollTop);
      const visibleStart=pane.getBoundingClientRect().top + 18;
      const row = [...pane.querySelectorAll('.cr-unit')].find(item => item.getBoundingClientRect().bottom > visibleStart);
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
    return items.notes.find(item => sameNoteReference(item, selected.reference)) || null;
  }

  async function flushNote() {
    clearTimeout(noteTimer);
    while (noteFlushPromise) { const saved=await noteFlushPromise; if(!saved)return false; }
    if (!noteDirty || !draft || !selected?.reference) return true;
    const target=selected.reference, value=draft.text, key=referenceKey(target), quote=selected.quote || '';
    const task=(async()=>{
      try {
        if(value.trim())await model.saveNote(target,value,quote);else await model.deleteNote(target);
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
    selected = {reference, unit, versionId, quote:quote||unit.text.slice(0,READER_LIMITS.quote)};
    inspectorOpen = true;
    currentTab = 'notes';
    mode = mode || 'single';
    emitLocation(reference);
    const layout = root.querySelector('.cr-layout');
    if (layout && !layout.querySelector('.cr-inspector')) {
      layout.append(element('aside', 'cr-inspector'));
      root.dataset.inspector = 'true';
    }
    renderInspector();
    markSelection();
    if (focus) root.querySelector('.cr-note-editor')?.focus();
  }

  function markSelection() {
    for (const row of root.querySelectorAll('.cr-unit')) row.dataset.selected = String(selected && row.dataset.unitId===selected.reference.unitId && row.closest('.cr-pane')?.dataset.versionId===selected.versionId);
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
    const quote = selected.quote || item.text;
    const value = quoteAttribution({document, version, unit: item, quote, locale: uiLocale()});
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
    const menu = button('☰', () => { libraryOpen = !libraryOpen; render(); }, 'cr-icon cr-menu');
    menu.hidden=!state.document;
    menu.setAttribute('aria-label', t('library')); menu.setAttribute('aria-expanded', String(libraryOpen));
    const title = element('div', 'cr-title');
    title.append(element('span', 'cr-kicker', `✧ ${t('reading')}`));
    title.append(element('h1', '', state.document ? documentTitle(state.document) : t('reading')));
    const author = localized(state.document?.author, uiLocale());
    if (author) title.append(element('p', '', author));
    header.append(menu, title, button('×', () => close(), 'cr-close'));
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
    const input = element('input', 'cr-library-search'); input.type = 'search'; input.placeholder = t('find'); input.value = state.catalog.query || '';
    input.addEventListener('input', () => {
      catalogCancel?.(); catalogCancel = wait(240, () => model.catalog({query: input.value}).catch(error => announce(friendlyError(error))));
    });
    aside.append(input);
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
      const count = `${t('units')}: ${countLabel(document.unitCount)}`;
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
    selected = null;
    searchQuery = '';
    searchResult = {items: []};
    const result = await model.open({documentId});
    if(disposed)return null;
    await ensureVisiblePanes();
    if(disposed)return null;
    render();
    return result;
  }

  async function openVersion(documentId, versionId) {
    if(!(await flushNote()) || disposed)return null;
    if(window.innerWidth<=850){libraryOpen=false;inspectorOpen=false;}
    await capturePanePositions();
    if(disposed)return null;
    selected = null;
    searchQuery = '';
    searchResult = {items: []};
    const result = await model.open({documentId, versionId});
    if(disposed)return null;
    await ensureVisiblePanes();
    if(disposed)return null;
    render();
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

  function renderVersionBar(state, versions) {
    const toolbar = element('nav', 'cr-toolbar'); toolbar.setAttribute('aria-label', t('edition'));
    for (const version of state.document?.versions || []) {
      const b = button(versionLabel(version), () => {
        openVersion(state.document.id, version.id).catch(error => announce(friendlyError(error)));
      }, 'cr-version');
      b.setAttribute('aria-pressed', String(version.id === state.activeVersionId)); b.dataset.available = String(isTextAvailable(version));
      if (!isTextAvailable(version)) b.title = catalogAvailability(version);
      toolbar.append(b);
    }
    if ((state.document?.versions || []).length > 1) {
      toolbar.append(button(mode === 'parallel' ? t('one') : t('parallel'), async () => {
        if(!(await flushNote()))return;
        capturePanePositions();
        mode = mode === 'parallel' ? 'single' : 'parallel';
        render();
        void ensureVisiblePanes();
      }, 'cr-mode'));
    }
    if (mode === 'parallel' && versions.length > 1) {
      const label = element('label', 'cr-compare'); label.append(element('span', '', t('compare')));
      const select = element('select'); select.setAttribute('aria-label', t('compare'));
      for (const version of state.document.versions.filter(item => item.id !== state.activeVersionId && isTextAvailable(item))) {
        const option = element('option', '', versionLabel(version)); option.value = version.id; select.append(option);
      }
      select.value = versions[1]?.id || '';
      select.addEventListener('change', async () => { if(!(await flushNote()))return; capturePanePositions(); remember(comparison,state.document.id, select.value); render(); void ensureVisiblePanes(); }); label.append(select); toolbar.append(label);
    }
    toolbar.append(element('span', 'cr-grow'));
    toolbar.append(button('⌕', () => { searchOpen = !searchOpen; if (!searchOpen) { searchQuery = ''; searchResult = {items: []}; } render(); root.querySelector('.cr-search-input')?.focus(); }, 'cr-icon cr-search-open'));
    toolbar.lastChild.setAttribute('aria-label',t('search'));
    toolbar.append(button('Aa', () => { settingsOpen = !settingsOpen; render(); }, 'cr-settings-button'));
    toolbar.lastChild.setAttribute('aria-label',t('settings'));
    toolbar.append(button('✧', () => { inspectorOpen = !inspectorOpen; render(); }, 'cr-icon'));
    toolbar.lastChild.setAttribute('aria-label', t('notes')); toolbar.lastChild.setAttribute('aria-expanded', String(inspectorOpen));
    return toolbar;
  }

  function renderSearch(state) {
    const bar = element('section', 'cr-search');
    const stored = state.document ? state.searches[stateKey(state.document.id, state.activeVersionId)] : null;
    if (stored?.query === searchQuery && stored?.scope === searchScope) searchResult = stored;
    const input = element('input', 'cr-search-input'); input.type = 'search'; input.placeholder = t('searchPlaceholder'); input.value = searchQuery; input.maxLength = READER_LIMITS.query;
    const scope = element('select', 'cr-search-scope');scope.setAttribute('aria-label',uiLocale()==='en'?'Search scope':'Область поиска');
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
      searchCancel?.(); searchQuery = input.value; searchScope = scope.value;
      searchCancel = wait(180, () => model.search({query: searchQuery, scope: searchScope}).then(result => { searchResult = result || {items: []}; render(); }).catch(error => { searchResult = {items: [], nextCursor: null, total: null}; announce(friendlyError(error)); render(); }));
    };
    input.addEventListener('input', update); scope.addEventListener('change', update); input.addEventListener('keydown', event => { if (event.key === 'Escape') { event.stopPropagation();event.preventDefault();searchOpen = false; searchQuery = ''; searchResult = {items: []}; render(); } });
    bar.append(input, scope, count);
    const hasContinuation = Boolean(searchResult.nextCursor);
    if (searchResult.items?.length || hasContinuation) {
      const results = element('div', 'cr-search-results');
      for (const item of searchResult.items.slice(-READER_LIMITS.searchResults)) {
        const result = button('', () => {
          const version = activeVersion(state.activeVersionId);
          model.loadWindow({versionId: state.activeVersionId, unitId: item.unitId, revision: version?.revision}).then(window => {
            const unit = window?.units?.find(value => value.unitId === item.unitId || value.id === item.unitId) || window?.units?.[0];
            if (unit) { selected = null; emitLocation(unitReference(unit, version, state.document, {})); render(); }
          }).catch(() => announce(t('error')));
        }, 'cr-search-result');
        const unit = currentWindow(state.activeVersionId)?.units?.find(value => (value.unitId || value.id) === item.unitId);
        result.append(element('strong', '', item.label || unitLabel(unit || item)), element('span', '', item.snippet || item.excerpt || ''));
        results.append(result);
      }
      if (!searchResult.items?.length && hasContinuation) results.append(element('span', 'cr-muted', t('continuation')));
      bar.append(results);
      if (hasContinuation) {
        bar.append(button(t('loadMore'), async event => {
          const trigger = event.currentTarget;
          trigger.disabled = true;
          try {
            searchResult = await model.search({query: searchQuery, scope: searchScope, cursor: searchResult.nextCursor}) || searchResult;
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

  function unitMatches(unit) {
    return (searchResult.items || []).filter(item => item.unitId && item.unitId === (unit.unitId || unit.id));
  }

  function renderUnit(unit, version, sourceDocument) {
    const versionId=version.id;
    const row = element(unit.heading ? 'h3' : 'div', `cr-unit${unit.heading ? ' cr-heading' : ''}${unit.kind === 'verse_line' ? ' cr-verse-line' : ''}`);
    // All units in this render share its validated snapshot. Looking up the
    // active document per unit used to clone both text windows twice per row.
    const reference = unitReference(unit, version, sourceDocument, {});
    row.dataset.reference = referenceKey(reference); row.dataset.unitId = unit.unitId || ''; row.dataset.kind = unit.kind || ''; row.__readerUnit = unit;
    if (selected && sameReference(selected.reference, reference)) row.dataset.selected = 'true';
    const ordinal = Number.isSafeInteger(unit.ordinal) && unit.ordinal > 0 ? unit.ordinal : Number.isSafeInteger(unit.ordinal) ? unit.ordinal + 1 : null;
    const markerLabel = unit.label || (ordinal === null ? '·' : String(ordinal));
    const marker = button(markerLabel, () => selectUnit(unit, versionId, {focus: true}), 'cr-unit-marker');
    marker.setAttribute('aria-label', `${t('select')} ${markerLabel}`);
    const content = element(unit.heading?'span':'div', 'cr-unit-text');
    const matches = unitMatches(unit).filter(match => Number.isSafeInteger(match.start) && Number.isSafeInteger(match.end) && match.end >= match.start).sort((a, b) => a.start - b.start);
    let cursor = 0;
    if (matches.length && searchQuery) {
      for (const match of matches) {
        const start = Math.max(cursor, match.start);
        const end = Math.max(start, match.end);
        content.append(document.createTextNode(codePointSlice(unit.text, cursor, start)));
        const mark = element('mark', '', codePointSlice(unit.text, start, end)); content.append(mark); cursor = end;
      }
    }
    content.append(document.createTextNode(codePointSlice(unit.text, cursor)));
    row.append(marker, content);
    return row;
  }

  function renderPane(version,state) {
    const pane=element('article','cr-pane');pane.dataset.versionId=version.id;pane.lang=version.language||'';
    pane.dir=/^(ar|he|fa|ur)(-|$)/i.test(version.language||'')?'rtl':'ltr';
    pane.tabIndex=0;pane.setAttribute('aria-label',`${documentTitle(state.document)} · ${versionLabel(version)}`);
    const head=element('header','cr-pane-head');head.append(element('span','cr-kicker',versionLabel(version)),element('h2','',documentTitle({id:state.document.id,title:version.title||state.document.title},[version.id])));
    const meta=[localized(version.edition,uiLocale()),localized(version.translator,uiLocale()),localized(version.locator,uiLocale())].filter(Boolean).join(' · ');
    if(meta)head.append(element('p','cr-muted',meta));pane.append(head);
    const page=state.windows[stateKey(state.document.id,version.id)];pane.dataset.windowStart=page?.units?.[0]?.id||'';
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
    else if(page){const body=element('div','cr-text-page');for(const unit of page.units)body.append(renderUnit(unit,version,state.document));pane.append(body);}
    const windowError = state.windowErrors?.[stateKey(state.document.id, version.id)];
    if (page && windowError) {
      const notice = element('p', 'cr-notice cr-window-error', friendlyError(windowError));
      notice.setAttribute('role', 'status');
      pane.insertBefore(notice, pane.querySelector('.cr-pane-nav'));
    }
    const move=async direction=>{if(!(await flushNote()))return;capturePanePositions();try{
      const result=await model.loadWindow({versionId:version.id,direction});if(result){selected=null;panePositions.delete(version.id);const first=result.units[0];emitLocation(first?unitReference(first,activeVersion(version.id),activeDocument(),{}):null);render();}}
      catch(error){announce(t('error'));}};
    const nav=element('footer','cr-pane-nav');const previous=button(`← ${t('previousChunk')}`,()=>move('previous'),'cr-nav');const next=button(`${t('nextChunk')} →`,()=>move('next'),'cr-nav');
    previous.disabled=stale||!page?.hasPrevious;next.disabled=stale||!page?.hasNext;
    const first=page?.units?.[0]?.ordinal,last=page?.units?.at(-1)?.ordinal,total=Number.isSafeInteger(page?.total)?page.total:null;
    const range = first && last ? `${first}–${last} / ${total ?? t('unknown')}` : `${page?.units?.length ? page.units.length : '—'} / ${total ?? t('unknown')}`;
    nav.append(previous,element('span','cr-muted',range),next);pane.append(nav);
    pane.addEventListener('scroll',()=>{remember(panePositions,version.id,pane.scrollTop);clearTimeout(paneSaveTimers.get(version.id));paneSaveTimers.set(version.id,setTimeout(()=>{paneSaveTimers.delete(version.id);if(!disposed&&opened)capturePanePositions();},550));},{passive:true});
    return pane;
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
    const current = selected?.unit || currentWindow(state.activeVersionId)?.units?.[0];
    if (current && graphTarget(current)) body.append(button(t('graph'), () => openGraph(current), 'cr-graph-action'));
  }

  function renderNotes(body, state) {
    body.append(element('h2', 'cr-inspector-title', t('notes')));
    if (!selected?.reference) { body.append(element('p', 'cr-muted', t('noNotes'))); renderNotebookList(body); renderTransfer(body); return; }
    const selectedNote = selectedRecord();
    const quote=selected.quote||selectedNote?.quote;
    if(quote){const excerpt=element('blockquote','cr-quote',quote);excerpt.dataset.reference=JSON.stringify(selected.reference);body.append(excerpt);}
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
    noteEditor = element('textarea', 'cr-note-editor'); noteEditor.rows = 7; noteEditor.maxLength = READER_LIMITS.note; noteEditor.placeholder = t('notePlaceholder'); noteEditor.value = draft?.key===referenceKey(selected.reference)?draft.text:selectedNote?.text || '';
    noteEditor.addEventListener('input', () => {
      draft={key:referenceKey(selected.reference),text:noteEditor.value};noteDirty = true;
      try{draftJournal.capture({reference:selected.reference,text:draft.text,quote:selected.quote||'',originalNoteId:selectedRecord()?.id??null});}
      catch(error){announce(friendlyError(error));}
      clearTimeout(noteTimer);noteTimer=setTimeout(flushNote,450);updateFooter();
    }); label.append(noteEditor); body.append(label);
    const actions = element('div', 'cr-note-actions');
    actions.append(button(t('save'), async () => {
      if (await flushNote()) { announce(notebookSaveLabel()); renderInspector(); updateFooter(); }
    }), button(t('delete'), async () => {
      if (!(await flushNote())) return;
      try{await model.deleteNote(selected.reference);}catch(error){announce(error.code==='conflict'?(uiLocale()==='en'?'This note changed in another window.':'Заметка изменилась в другом окне.'):t('saveError'));return;}
      selected = {...selected, quote: ''};
      announce(notebookSaveLabel());
      renderInspector();
    }));
    body.append(actions);
    renderNotebookList(body);
    renderTransfer(body);
  }

  function renderNotebookList(body) {
    const data=notebookItems(),page=model.notebookState();
    const details=element('details','cr-notebook-list');details.open=true;
    details.append(element('summary','',`${t('notes')} (${data.notes.length+data.bookmarks.length})`));
    for(const item of [...data.notes,...data.bookmarks].reverse()){
      const ref=item.reference;
      const catalogDocument=modelState().catalog.items.find(document=>document.id===ref?.target?.workId);
      const catalogVersion=catalogDocument?.versions?.find(version=>version.id===ref?.versionId);
      const contextLabel=[catalogDocument?documentTitle(catalogDocument):'',catalogVersion?versionLanguage(catalogVersion):''].filter(Boolean).join(' · ');
      const entry=button('',async()=>{
        if(!(await flushNote()))return;capturePanePositions();
        if(ref.schemaVersion==='tos.corpus.reader.native-reference.v1'){await close();await onNativeReference?.(ref,item);return;}
        try{await model.open({documentId:ref.target.workId,versionId:ref.versionId,reference:ref});await ensureVisiblePanes();
          selected={reference:exactReference(ref),unit:currentWindow(ref.versionId)?.units?.find(unit=>unit.id===ref.unitId)||null,versionId:ref.versionId,quote:item.quote||''};render();}
        catch(error){announce(t('error'));}
      },'cr-note-entry');entry.append(element('strong','',`${item.kind==='bookmark'?'◆ ':''}${contextLabel||t('passage')}`),element('span','',item.text||item.quote||t('bookmark')));details.append(entry);
    }
    if(model.notebookPage){
      if(page.nextCursor)details.append(button(t('loadMore'),async()=>{if(!(await flushNote()))return;try{await model.notebookPage({cursor:page.nextCursor});renderInspector();}catch{announce(t('error'));}},'cr-more'));
      if(page.cursor)details.append(button(uiLocale()==='en'?'First notes':'К первым записям',async()=>{if(!(await flushNote()))return;await model.notebookPage();renderInspector();},'cr-more'));
    }
    body.append(details);
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
      const section = element('section', 'cr-source'); section.append(element('h3', '', [versionLanguage(version), localized(version.edition, uiLocale()), status].filter(Boolean).join(' · ') || t('edition')));
      const sourceUrl = safeExternalUrl(attr(version.sourceUrl || version.source_url || version.source?.url || version.source?.href));
      if (sourceUrl) { const link = element('a', '', sourceLinkLabel(sourceUrl, uiLocale())); link.href = sourceUrl; link.target = '_blank'; link.rel = 'noopener noreferrer'; section.append(link); }
      body.append(section);
    }
  }

  function renderInspector() {
    const state = modelState(); const inspector = root.querySelector('.cr-inspector'); if (!inspector) return;
    noteEditor = null; inspector.replaceChildren();
    const tabs = element('nav', 'cr-inspector-tabs');
    for (const name of ['context', 'notes', 'source']) { const b = button(t(name), async () => { if(!(await flushNote()))return; currentTab = name; renderInspector(); }, 'cr-tab'); b.setAttribute('aria-pressed', String(currentTab === name)); tabs.append(b); }
    inspector.append(tabs); const body = element('div', 'cr-inspector-body'); inspector.append(body);
    if (currentTab === 'notes') renderNotes(body, state); else if (currentTab === 'source') renderSource(body, state); else renderContext(body, state);
  }

  function renderSettings() {
    const data = model.notebookState(); const preferences = data.preferences || {};
    const settings = element('section', 'cr-settings');
    const adjust = (label, values, key) => { const row = element('div', 'cr-setting'); row.append(element('span', '', label)); for (const value of values) { const labelValue=({comfortable:uiLocale()==='en'?'Comfortable':'Обычная',wide:uiLocale()==='en'?'Wide':'Широкая',night:t('night'),paper:t('paper')})[value]||String(value);const b = button(labelValue, () => { model.setPreferences({[key]: value}); render(); }, 'cr-setting-button'); b.setAttribute('aria-pressed', String(preferences[key] === value)); row.append(b); } settings.append(row); };
    adjust(t('font'), [16, 18, 20, 22], 'fontSize'); adjust(t('leading'), [1.6, 1.9, 2.2], 'lineHeight'); adjust(t('width'), ['comfortable', 'wide'], 'width'); adjust(t('theme'), ['night', 'paper'], 'theme'); return settings;
  }

  function updateFooter() {
    const footer = root.querySelector('.cr-footer'); if (!footer) return;
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
    const focused=document.activeElement;const focusClass=['cr-library-search','cr-search-input','cr-search-scope','cr-note-editor'].find(name=>focused?.classList?.contains(name));
    const selectionStart=focused?.selectionStart,selectionEnd=focused?.selectionEnd;
    for(const pane of root.querySelectorAll('.cr-pane')){if(pane.dataset.windowStart===currentWindow(pane.dataset.versionId)?.units?.[0]?.id)remember(panePositions,pane.dataset.versionId,pane.scrollTop);else panePositions.delete(pane.dataset.versionId);}
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
    const main = element('main', 'cr-main'); const panes = element('div', 'cr-panes');
    if (versions.length > 1) panes.classList.add('cr-parallel');
    for (const version of versions) panes.append(renderPane(version, state));
    main.append(panes); layout.append(main);
    if (inspectorOpen) layout.append(element('aside', 'cr-inspector'));
    root.append(layout, renderFooter()); renderInspector(); restorePanePositions(); markSelection();
    const location = root.querySelector('.cr-location'); if (location) { location.textContent = locationLabel(currentRef()); }
    rendering=false;
    if(focusClass){const target=root.querySelector('.'+focusClass);if(target){target.focus({preventScroll:true});if(Number.isInteger(selectionStart)&&typeof target.setSelectionRange==='function')target.setSelectionRange(selectionStart,selectionEnd);}}
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
      const firstUnit = openedResult && !stale ? currentWindow(state.activeVersionId)?.units?.[0] : null;
      if (firstUnit) emitLocation(unitReference(firstUnit, activeVersion(state.activeVersionId), state.document, {}));
      render();
    } catch (error) { announce(friendlyError(error)); render(); }
  }

  async function close({notify = true} = {}) {
    if (!opened || disposed) return true;
    if(!(await flushNote()))return false;
    if(disposed || !root)return true;
    await capturePanePositions();
    if(disposed || !root)return true;
    opened=false; root.hidden=true; delete root.dataset.open;model.cancel();
    if(notify)onClose?.();
    const focus=opener;opener=null;
    if(focus?.isConnected&&!focus.closest('[hidden]')&&focus.offsetParent!==null)focus.focus?.({preventScroll:true});
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
      opener = document.activeElement;
      opened = true; root.hidden = false; root.dataset.open = 'true';
      if (args.mode === 'parallel' || args.mode === 'single') mode = args.mode;
      if (args.reference || args.unitId) selected = null;
      const hasTarget = Boolean(args.documentId || args.versionId || args.reference || args.unitId || args.revision);
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
    destroy() {
      if(destroyWork)return destroyWork;
      clearTimeout(noteTimer);
      for(const timer of paneSaveTimers.values())clearTimeout(timer);paneSaveTimers.clear();
      // pagehide is best effort: capture while the panes still exist, then
      // retain the DOM and notebook until both note and position writes settle.
      const positionSaving=opened?capturePanePositions():Promise.resolve();
      const saving=Promise.allSettled([flushNote(),positionSaving]);
      disposed=true;opened=false;catalogCancel?.();searchCancel?.();
      document.removeEventListener('selectionchange',onSelectionChange);document.removeEventListener('visibilitychange',visibilityChanged);
      window.removeEventListener('beforeunload',beforeUnload);
      destroyWork=saving.finally(()=>{root?.remove();root=null;model.destroy();});return destroyWork;
    },
    isOpen: () => opened,
    state: () => model.snapshot(),
  };
}
