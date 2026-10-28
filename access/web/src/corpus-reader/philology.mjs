import './philology.css';
import {referenceKey, validateReference} from './model.mjs';
import {languageName} from '../observatory/human-presentation.mjs';

const COPY = {
  ru: {
    title: 'Выбранное слово', passage:'Выбранный фрагмент', selectWord:'Для разбора и словаря выделите одно слово.', passageConcordance:'Найти буквальные употребления фрагмента', language: 'Язык источника', address: 'Точный адрес',
    unavailable: 'Разбор для этого слова недоступен.', choose: 'Выделите слово в тексте, чтобы запросить разбор.',
    loading: 'Загружаем предоставленный разбор…', candidates: 'Варианты анализа', analysisSource: 'Источник анализа',
    version: 'Версия данных', lemma: 'Лемма', morphology: 'Морфология', gloss: 'Значение',
    source: 'Источник кандидата', alternatives: 'Альтернативы', dictionary: 'Словарная статья', rawAnalysis:'Исходный разбор анализатора',
    concordance: 'Найти буквальные употребления', more: 'Ещё совпадения', noConcordance: 'Употреблений не найдено.',
    noSearch: 'Поиск по версии недоступен.', noExact: 'Совпадение не содержит точного адреса; перейти нельзя.',
    returnContext: 'Открыть это употребление в тексте',
    mappings: 'Предложенные соответствия', noExactMapping: 'Для этого точного вхождения соответствие пока не подготовлено.',
    proposal: 'Предложенное сопоставление · не канон', target: 'Точный фрагмент в другом издании', openTarget: 'Открыть точный фрагмент', reviewBasis: 'Основание предложения',
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
    title: 'Selected word', passage:'Selected passage', selectWord:'Select a single word for analysis and dictionary lookup.', passageConcordance:'Find literal occurrences of this passage', language: 'Source language', address: 'Exact address',
    unavailable: 'Analysis is unavailable for this word.', choose: 'Select a word in the text to request analysis.',
    loading: 'Loading supplied analysis…', candidates: 'Analysis candidates', analysisSource: 'Analysis source',
    version: 'Data version', lemma: 'Lemma', morphology: 'Morphology', gloss: 'Gloss',
    source: 'Candidate source', alternatives: 'Alternatives', dictionary: 'Dictionary entry', rawAnalysis:'Original analyzer output',
    concordance: 'Find literal occurrences', more: 'More matches', noConcordance: 'No occurrences found.',
    noSearch: 'Version search is unavailable.', noExact: 'This match has no exact address and cannot be opened.',
    returnContext: 'Open this occurrence in the text',
    mappings: 'Proposed correspondences', noExactMapping: 'No correspondence is prepared for this exact occurrence.',
    proposal: 'Proposed correspondence · not canon', target: 'Exact passage in another edition', openTarget: 'Open exact passage', reviewBasis: 'Proposal basis',
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

// A phrase must not be presented to a dictionary or analyzer as one word.
// Keep apostrophes and hyphens within an otherwise continuous word.
export const isSingleWordSelection = value => typeof value==='string'
  && /^[\p{L}\p{M}\p{N}]+(?:['’ʼ-][\p{L}\p{M}\p{N}]+)*$/u.test(value);

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
export function bindPhilologyResult(value, reference, form = null) {
  if (!object(value) || !object(value.reference)) return {status: 'unavailable', candidates: []};
  let expected;
  try {
    expected = referenceKey(validateReference(reference));
    const returned = referenceKey(validateReference(value.reference));
    if (!expected || expected !== returned) return {status: 'mismatch', candidates: []};
  } catch { return {status: 'mismatch', candidates: []}; }
  const mappings = (Array.isArray(value.mappings) ? value.mappings : []).filter(mapping => {
    if (!object(mapping) || mapping.status !== 'proposed' || mapping.reviewStatus !== 'model_proposal_owner_review_pending'
      || (form !== null && mapping.sourceQuote !== form)) return false;
    try {
      return referenceKey(validateReference(mapping.sourceReference)) === expected
        && validateReference(mapping.targetReference).target.workId === validateReference(reference).target.workId
        && typeof mapping.targetQuote === 'string' && mapping.targetQuote.length > 0;
    } catch { return false; }
  }).slice(0, 8);
  const mappingFields = {mappings,mappingStatus:mappings.length?'available':'none'};
  const candidates = Array.isArray(value.candidates) ? value.candidates : [];
  const version = value.analysisVersion ?? value.version;
  if (value.status !== 'available' || typeof version !== 'string' || !version || !candidates.length) {
    return {...value, ...mappingFields, status: 'unavailable', candidates: []};
  }
  return {...value, ...mappingFields, analysisVersion: version, candidates};
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
    row.append(element('strong', '', labels[key] || ({id: locale === 'en' ? 'Identifier: ' : 'Идентификатор: ', name: locale === 'en' ? 'Name: ' : 'Название: ', url: locale === 'en' ? 'Link: ' : 'Ссылка: ', version: locale === 'en' ? 'Version: ' : 'Версия: ', license: locale === 'en' ? 'License: ' : 'Лицензия: '})[key] || (locale === 'en' ? 'Details: ' : 'Сведения: ')));
    const url = /url|uri|href/i.test(key) ? safeUrl(item) : '';
    if (url) {
      const link = element('a', '', item); link.href = url; link.target = '_blank'; link.rel = 'noopener noreferrer'; row.append(link);
    } else row.append(document.createTextNode(item));
    section.append(row);
  }
  parent.append(section);
}

const MORPH = {
 noun:['существительное','noun'], verb:['глагол','verb'], adjective:['прилагательное','adjective'], adverb:['наречие','adverb'], article:['артикль','article'], pronoun:['местоимение','pronoun'], preposition:['предлог','preposition'], conjunction:['союз','conjunction'], particle:['частица','particle'], exclamation:['междометие','exclamation'], irregular:['неправильная форма','irregular form'],
 nominative:['именительный падеж','nominative'], accusative:['винительный падеж','accusative'], genitive:['родительный падеж','genitive'], dative:['дательный падеж','dative'], vocative:['звательный падеж','vocative'], masculine:['мужской род','masculine'], feminine:['женский род','feminine'], neuter:['средний род','neuter'], singular:['единственное число','singular'], plural:['множественное число','plural'], dual:['двойственное число','dual'], present:['настоящее время','present'], past:['прошедшее время','past'], future:['будущее время','future'], imperfect:['имперфект','imperfect'], aorist:['аорист','aorist'], perfect:['перфект','perfect'], indicative:['изъявительное наклонение','indicative'], subjunctive:['сослагательное наклонение','subjunctive'], optative:['оптатив','optative'], imperative:['повелительное наклонение','imperative'], infinitive:['инфинитив','infinitive'], participle:['причастие','participle'], active:['действительный залог','active'], passive:['страдательный залог','passive'], middle:['средний залог','middle'], mediopassive:['средне-страдательный залог','mediopassive'], '1st':['первое лицо','first person'], '2nd':['второе лицо','second person'], '3rd':['третье лицо','third person'], Attic:['аттический','Attic'], Ionic:['ионийский','Ionic'], Doric:['дорийский','Doric'], Aeolic:['эолийский','Aeolic'], epic:['эпический','epic']
};
const TAGS={NN:'noun',V:'verb',ADJ:'adjective',ADV:'adverb',ART:'article',PRO:'pronoun',PREP:'preposition',CONJ:'conjunction',Masc:'masculine',Fem:'feminine',Neut:'neuter',Nom:'nominative',Acc:'accusative',Gen:'genitive',Dat:'dative',Sg:'singular',Pl:'plural',Pres:'present',Past:'past',Perf:'perfect',Ind:'indicative',Subj:'subjunctive',Imp:'imperative',Inf:'infinitive',Part:'participle','1':'1st','2':'2nd','3':'3rd'};
export function morphologyLabel(value, locale) {
 const en=String(locale||'ru').toLowerCase().startsWith('en');
 if (typeof value !== 'string') return localized(value,locale);
 if (value.includes('<')) return value.split(/(\s*\/\s*)/).map((analysis,index)=>{
   if(index%2)return analysis;
   const tags=[...analysis.matchAll(/<([^<>]+)>/g)];
   return tags.map(match=>MORPH[TAGS[match[1]]]?.[en?1:0]??match[0]).join(' · ')||analysis;
 }).join('');
 return value.split(/(\s*[·/]\s*|\s+)/).map(part=>MORPH[part]?.[en?1:0] ?? part).join('');
}

function renderCandidate(parent, candidate, index, locale, words) {
  if (!object(candidate)) return;
  const card = element('article', 'cr-philology-candidate');
  card.append(element('h3', '', localized(candidate.lemma, locale) || `${words.candidates} ${index + 1}`));
  const alternatives = Array.isArray(candidate.alternatives) ? candidate.alternatives : [];
  const fields = [
    [words.morphology, candidate.morphology],
    [words.gloss, candidate.gloss ?? candidate.meaning],
  ];
  for (const [label, value] of fields) {
    const text = label === words.morphology ? morphologyLabel(value, locale) : localized(value, locale);
    if (text) { const row = element('p', ''); row.append(element('strong', '', `${label}: `), document.createTextNode(text)); card.append(row); }
  }
  if(typeof candidate.morphology==='string' && candidate.morphology.includes('<')){
    const original=element('details','cr-philology-source');
    const raw=[candidate.morphology,...alternatives.filter(value=>typeof value==='string'&&value.includes('<'))].join('\n');
    const text=element('p','cr-philology-meta',raw);text.style.whiteSpace='pre-wrap';
    original.append(element('summary','',words.rawAnalysis),text);card.append(original);
  }
  appendMetadata(card, words.source, candidate.source ?? candidate.provenance ?? candidate.sourceRef, locale, {});
  if (alternatives.length) {
    const list = element('ul', 'cr-philology-alternatives');
    for (const alternative of alternatives) {
      const text = typeof alternative === 'string' ? alternative : localized(alternative?.label ?? alternative?.lemma, locale);
      if (text) list.append(element('li', '', typeof alternative==='string'&&alternative.includes('<')?morphologyLabel(alternative,locale):text));
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
  onConcordance, onMore, onOccurrence, onMappingTarget,
} = {}) {
  const words = COPY[String(locale).toLowerCase().startsWith('en') ? 'en' : 'ru'];
  const multiword=Boolean(form) && !isSingleWordSelection(form);
  body.append(element('h2', 'cr-inspector-title', multiword ? words.passage : words.title));
  const selected = element('blockquote', 'cr-philology-form', form || words.choose);
  body.append(selected);
  const source = reference?.target;
  const exact = element('dl', 'cr-philology-address');
  const address = source ? [source.workId, source.expressionId, source.editionId, source.itemId, source.fileId, source.textLayerRef, source.fileSha256, source.textLayerSha256].filter(Boolean).join(' › ') : '';
  for (const [label, value] of [[words.language, language ? languageName(language, locale) : ''], [words.address, address], [String(locale).startsWith('en') ? 'Version / unit' : 'Версия / фрагмент', reference ? `${reference.versionId} / ${reference.unitId}` : ''], [String(locale).startsWith('en') ? 'Code-point offsets' : 'Позиции символов Unicode', reference?.selector ? `${reference.selector.start}–${reference.selector.end}` : '']]) {
    if (!value) continue;
    exact.append(element('dt', '', label), element('dd', '', value));
  }
  if (exact.childElementCount) {
    const details=element('details','cr-philology-exact');details.append(element('summary','',words.address),exact);body.append(details);
  }
  if(language)body.append(element('p','cr-philology-muted',`${words.language}: ${languageName(language, locale)}`));

  if (multiword) body.append(element('p','cr-philology-muted',words.selectWord));
  else if (busy) body.append(element('p', 'cr-philology-muted', words.loading));
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

  const dictionaryLinks = !multiword && Array.isArray(result?.dictionaryLinks) ? result.dictionaryLinks : [];
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

  const mappings = !multiword && Array.isArray(result?.mappings) ? result.mappings : [];
  if (mappings.length) {
    const section=element('section','cr-philology-mappings');
    section.append(element('h3','',words.mappings));
    for(const mapping of mappings){
      const card=element('article','cr-philology-mapping');
      card.append(element('p','cr-philology-muted',words.proposal));
      card.append(element('p','cr-philology-mapping-target',`${words.target}: ${localized(mapping.targetVersionLabel,locale)||mapping.targetVersionId||''} · ${mapping.targetLanguage||''}`));
      card.append(element('blockquote','',mapping.targetQuote));
      const targetReference=mapping.targetReference,targetScope=targetReference?.target;
      if(targetScope&&targetReference.selector){
        const address=[targetScope.workId,targetScope.expressionId,targetScope.editionId,targetScope.itemId,targetScope.fileId,targetScope.textLayerRef,targetScope.fileSha256,targetScope.textLayerSha256,
          `${targetReference.selector.start}–${targetReference.selector.end} ${locale === 'en' ? 'code points' : 'символов'}`].filter(Boolean).join(' › ');
        const details=element('details','cr-philology-exact');details.append(element('summary','',words.address),element('p','cr-philology-context-address',address));card.append(details);
      }
      if(mapping.reviewBasis)card.append(element('p','cr-philology-muted',`${words.reviewBasis}: ${mapping.reviewBasis}`));
      for(const evidence of Array.isArray(mapping.evidence)?mapping.evidence:[]){if(typeof evidence?.description==='string')card.append(element('p','cr-philology-muted',evidence.description));}
      const open=element('button','','');open.type='button';open.textContent=words.openTarget;
      open.disabled=typeof onMappingTarget!=='function';open.addEventListener('click',()=>onMappingTarget?.(mapping));card.append(open);
      section.append(card);
    }
    body.append(section);
  } else if(!multiword && form && !busy && result) body.append(element('p','cr-philology-muted',words.noExactMapping));

  const actions = element('div', 'cr-philology-actions');
  const search = element('button', '', multiword ? words.passageConcordance : words.concordance); search.type = 'button'; search.disabled = !form || !searchAvailable;
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
