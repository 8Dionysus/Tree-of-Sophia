import './philology.css';
import {referenceKey, validateReference} from './model.mjs';

const COPY = {
  ru: {
    title: 'Выбранное слово', language: 'Язык источника', address: 'Точный адрес',
    unavailable: 'Разбор для этого слова недоступен.', choose: 'Выделите слово в тексте, чтобы запросить разбор.',
    loading: 'Загружаем предоставленный разбор…', candidates: 'Варианты анализа', analysisSource: 'Источник анализа',
    version: 'Версия данных', lemma: 'Лемма', morphology: 'Морфология', gloss: 'Значение',
    source: 'Источник кандидата', alternatives: 'Альтернативы', dictionary: 'Словарная статья',
    concordance: 'Найти буквальные употребления', more: 'Ещё совпадения', noConcordance: 'Употреблений не найдено.',
    noSearch: 'Поиск по версии недоступен.', noExact: 'Совпадение не содержит точного адреса; перейти нельзя.',
    returnContext: 'Открыть это употребление в тексте',
    reasons: {
      unrecognized:'Анализатор не распознал обработанную форму.',
      not_processed:'Эта форма ещё не включена в подготовленный набор.',
      provider_error:'Сервис анализа вернул ошибку.',
      unsupported_language:'Для этого языка нет набора анализа.',
      source_changed:'Источник изменился; анализ не привязан к текущему тексту.',
      form_mismatch:'Форма анализа не совпала с выбранным вхождением.',
    },
  },
  en: {
    title: 'Selected word', language: 'Source language', address: 'Exact address',
    unavailable: 'Analysis is unavailable for this word.', choose: 'Select a word in the text to request analysis.',
    loading: 'Loading supplied analysis…', candidates: 'Analysis candidates', analysisSource: 'Analysis source',
    version: 'Data version', lemma: 'Lemma', morphology: 'Morphology', gloss: 'Gloss',
    source: 'Candidate source', alternatives: 'Alternatives', dictionary: 'Dictionary entry',
    concordance: 'Find literal occurrences', more: 'More matches', noConcordance: 'No occurrences found.',
    noSearch: 'Version search is unavailable.', noExact: 'This match has no exact address and cannot be opened.',
    returnContext: 'Open this occurrence in the text',
    reasons: {
      unrecognized:'The analyzer did not recognize this processed form.',
      not_processed:'This form is not yet included in the prepared analysis set.',
      provider_error:'The analysis service returned an error.',
      unsupported_language:'No analysis set is available for this language.',
      source_changed:'The source changed; analysis is not bound to the current text.',
      form_mismatch:'The analyzed form does not match the selected occurrence.',
    },
  },
};

const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const localized = (value, locale) => {
  if (typeof value === 'string') return value;
  if (!object(value)) return '';
  const base = String(locale || 'ru').toLowerCase().split('-')[0];
  return value[locale] ?? value[base] ?? value.ru ?? value.en ?? Object.values(value).find(item => typeof item === 'string') ?? '';
};

function element(tag, className = '', value) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (value !== undefined) node.textContent = value;
  return node;
}

function safeUrl(value) {
  if (typeof value !== 'string' || !value.trim()) return '';
  try {
    const url = new URL(value, document.baseURI);
    return ['http:', 'https:'].includes(url.protocol) ? url.href : '';
  } catch { return ''; }
}

function metadataLines(value, locale) {
  if (!object(value)) return [];
  return Object.entries(value)
    .filter(([key, item]) => typeof item === 'string' && item && !/token|secret|payload|text/i.test(key))
    .map(([key, item]) => [key, localized(item, locale)]);
}

/** Exact source form, sliced by the reader's Unicode-code-point selector. */
export function selectedPhilologyForm(text, reference) {
  try {
    const ref = validateReference(reference);
    const {start, end} = ref.selector;
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || end <= start) return '';
    return Array.from(String(text ?? '')).slice(start, end).join('');
  } catch { return ''; }
}

/** Build non-overlapping marks, reserving the exact selected occurrence first. */
export function philologyMarkIntervals(length, searchRanges = [], selectedRange = null, searchEnabled = false) {
  const boundedLength = Number.isSafeInteger(length) && length >= 0 ? length : 0;
  const selected = selectedRange && Number.isSafeInteger(selectedRange.start) && Number.isSafeInteger(selectedRange.end)
    && selectedRange.start >= 0 && selectedRange.end > selectedRange.start && selectedRange.end <= boundedLength
    ? {start:selectedRange.start,end:selectedRange.end}
    : null;
  const intervals = [];
  if (searchEnabled) for (const range of Array.isArray(searchRanges) ? searchRanges : []) {
    if (!Number.isSafeInteger(range?.start) || !Number.isSafeInteger(range?.end)) continue;
    const start = Math.max(0, Math.min(boundedLength, range.start));
    const end = Math.max(start, Math.min(boundedLength, range.end));
    if (end <= start) continue;
    if (selected && start < selected.end && end > selected.start) {
      if (start < selected.start) intervals.push({start,end:Math.min(end,selected.start),selected:false});
      if (end > selected.end) intervals.push({start:Math.max(start,selected.end),end,selected:false});
    } else intervals.push({start,end,selected:false});
  }
  if (selected) intervals.push({...selected,selected:true});
  return intervals.sort((left,right)=>left.start-right.start || Number(right.selected)-Number(left.selected) || left.end-right.end);
}

/** Keep only analysis packets bound to this exact occurrence. */
export function bindPhilologyResult(value, reference) {
  if (!object(value) || !object(value.reference)) return {status: 'unavailable', candidates: []};
  try {
    const expected = referenceKey(validateReference(reference));
    const returned = referenceKey(validateReference(value.reference));
    if (!expected || expected !== returned) return {status: 'mismatch', candidates: []};
  } catch { return {status: 'mismatch', candidates: []}; }
  const candidates = Array.isArray(value.candidates) ? value.candidates : [];
  const version = value.analysisVersion ?? value.version;
  if (value.status !== 'available' || typeof version !== 'string' || !version || !candidates.length) {
    return {...value, status: 'unavailable', candidates: []};
  }
  return {...value, analysisVersion: version, candidates};
}

function appendMetadata(parent, heading, value, locale, labels) {
  if (typeof value === 'string' && value) {
    const section = element('section', 'cr-philology-source');
    section.append(element('h3', '', heading), element('p', 'cr-philology-meta', value));
    parent.append(section);
    return;
  }
  const rows = metadataLines(value, locale);
  if (!rows.length) return;
  const section = element('section', 'cr-philology-source');
  section.append(element('h3', '', heading));
  for (const [key, item] of rows) {
    const row = element('p', 'cr-philology-meta');
    row.append(element('strong', '', labels[key] || key));
    const url = /url|uri|href/i.test(key) ? safeUrl(item) : '';
    if (url) {
      const link = element('a', '', item); link.href = url; link.target = '_blank'; link.rel = 'noopener noreferrer'; row.append(link);
    } else row.append(document.createTextNode(item));
    section.append(row);
  }
  parent.append(section);
}

function renderCandidate(parent, candidate, index, locale, words) {
  if (!object(candidate)) return;
  const card = element('article', 'cr-philology-candidate');
  card.append(element('h3', '', localized(candidate.lemma, locale) || `${words.candidates} ${index + 1}`));
  const fields = [
    [words.morphology, candidate.morphology],
    [words.gloss, candidate.gloss ?? candidate.meaning],
  ];
  for (const [label, value] of fields) {
    const text = localized(value, locale);
    if (text) { const row = element('p', ''); row.append(element('strong', '', `${label}: `), document.createTextNode(text)); card.append(row); }
  }
  appendMetadata(card, words.source, candidate.source ?? candidate.provenance ?? candidate.sourceRef, locale, {});
  const alternatives = Array.isArray(candidate.alternatives) ? candidate.alternatives : [];
  if (alternatives.length) {
    const list = element('ul', 'cr-philology-alternatives');
    for (const alternative of alternatives) {
      const text = typeof alternative === 'string' ? alternative : localized(alternative?.label ?? alternative?.lemma, locale);
      if (text) list.append(element('li', '', text));
    }
    if (list.childElementCount) { card.append(element('h4', '', words.alternatives), list); }
  }
  const dictionaryUrl = safeUrl(candidate.dictionaryUrl ?? candidate.dictionary?.url ?? candidate.articleUrl);
  if (dictionaryUrl) {
    const link = element('a', 'cr-philology-dictionary', words.dictionary);
    link.href = dictionaryUrl; link.target = '_blank'; link.rel = 'noopener noreferrer'; card.append(link);
  }
  parent.append(card);
}

/** Presentation-only panel; callbacks keep fetching/navigation with the reader. */
export function renderPhilology(body, {
  locale = 'ru', form = '', language = '', reference = null, version = null,
  result = null, busy = false, concordance = null, searchAvailable = false,
  onConcordance, onMore, onOccurrence,
} = {}) {
  const words = COPY[String(locale).toLowerCase().startsWith('en') ? 'en' : 'ru'];
  body.append(element('h2', 'cr-inspector-title', words.title));
  const selected = element('blockquote', 'cr-philology-form', form || words.choose);
  body.append(selected);
  const source = reference?.target;
  const exact = element('dl', 'cr-philology-address');
  const address = source ? [source.workId, source.expressionId, source.editionId, source.itemId, source.fileId, source.textLayerRef, source.fileSha256, source.textLayerSha256].filter(Boolean).join(' › ') : '';
  for (const [label, value] of [[words.language, language || ''], [words.address, address], ['version / unit', reference ? `${reference.versionId} / ${reference.unitId}` : ''], ['code-point', reference?.selector ? `${reference.selector.start}–${reference.selector.end}` : '']]) {
    if (!value) continue;
    exact.append(element('dt', '', label), element('dd', '', value));
  }
  if (exact.childElementCount) {
    const details=element('details','cr-philology-exact');details.append(element('summary','',words.address),exact);body.append(details);
  }
  if(language)body.append(element('p','cr-philology-muted',`${words.language}: ${language}`));

  if (busy) body.append(element('p', 'cr-philology-muted', words.loading));
  else if (!form) body.append(element('p', 'cr-philology-muted', words.choose));
  else if (!result || result.status === 'unavailable') {
    body.append(element('p', 'cr-philology-muted', words.reasons?.[result?.reason] || words.unavailable));
    if (result?.source) appendMetadata(body, words.analysisSource, result.source, locale, {});
  }
  else if (result.status === 'mismatch') body.append(element('p', 'cr-philology-muted', words.unavailable));
  else {
    body.append(element('p', 'cr-philology-version', `${words.version}: ${result.analysisVersion}`));
    appendMetadata(body, words.analysisSource, result.source, locale, {});
    body.append(element('h3', 'cr-philology-heading', words.candidates));
    result.candidates.forEach((candidate, index) => renderCandidate(body, candidate, index, locale, words));
    if (Array.isArray(result.alternatives) && result.alternatives.length) {
      const section = element('section', 'cr-philology-source');
      section.append(element('h3', '', words.alternatives));
      for (const item of result.alternatives) section.append(element('p', '', typeof item === 'string' ? item : localized(item?.label ?? item?.lemma, locale)));
      body.append(section);
    }
  }

  const dictionaryLinks = Array.isArray(result?.dictionaryLinks) ? result.dictionaryLinks : [];
  const usableDictionaryLinks = dictionaryLinks.flatMap(item => {
    const url = safeUrl(item?.url);
    const label = localized(item?.label, locale);
    return url && label ? [{url,label}] : [];
  });
  if (usableDictionaryLinks.length) {
    const section = element('section', 'cr-philology-source');
    section.append(element('h3', '', words.dictionary));
    for (const item of usableDictionaryLinks) {
      const link = element('a', 'cr-philology-dictionary', item.label);
      link.href = item.url; link.target = '_blank'; link.rel = 'noopener noreferrer'; section.append(link);
    }
    body.append(section);
  }

  const actions = element('div', 'cr-philology-actions');
  const search = element('button', '', words.concordance); search.type = 'button'; search.disabled = !form || !searchAvailable;
  search.addEventListener('click', () => onConcordance?.(form)); actions.append(search);
  if (!searchAvailable) body.append(element('p', 'cr-philology-muted', words.noSearch));
  body.append(actions);
  if (concordance) {
    for (const hit of concordance.items || []) {
      const button = element('button', 'cr-philology-hit', ''); button.type = 'button';
      button.append(element('strong', '', hit.label || hit.unitId || words.returnContext), element('span', '', hit.snippet || ''));
      button.disabled = !hit.reference;
      button.addEventListener('click', () => hit.reference && onOccurrence?.(hit));
      body.append(button);
    }
    if (concordance.inexactCount) body.append(element('p', 'cr-philology-muted', words.noExact));
    if (concordance.items?.length === 0 && !concordance.nextCursor && !concordance.inexactCount) body.append(element('p', 'cr-philology-muted', words.noConcordance));
    if (concordance.nextCursor) {
      const more = element('button', 'cr-more', words.more); more.type = 'button'; more.addEventListener('click', () => onMore?.()); body.append(more);
    }
  }
  // Keep source metadata visible next to the word even when the analysis source is unavailable.
  if (version?.edition) body.append(element('p', 'cr-philology-muted', localized(version.edition, locale)));
}
