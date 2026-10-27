import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {referenceKey, validateReference} from '../corpus-reader/model.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {describeSourceLocator, ensureSourceRecord, resolveSourceLocator} from './source-address.mjs';
import {createCompareAndSwapPersistence} from './workspace-persistence.mjs';
import {applyGraphSpace, exportGraphSpace, graphKeyInfo, inspectGraphSpace, shouldReloadImportedGraph, validateGraphSpace} from './graph-transfer.mjs';
import './workspace-view.css';
import {isEnglish, kindLabel, actionLabel, proposalStatement, researchQuestionLine, displayPublicError} from './presentation-i18n.mjs';

export const PUBLIC_WORKSPACE_SCHEMA = 'tos.public_research.personal_workspace.v1';
export const PUBLIC_WORKSPACE_VERSION = 1;
export const WORKSPACE_KEY = 'tos-research-workspace-v1';
const MAX_EXPORT_BYTES = 12_000_000;
const MAX_NOTE_TEXT = 4_000;
const OBJECTION_PREFIX = 'Objection to thesis: ';
const REVISION_PREFIX = 'Revises earlier thesis: ';
const MAX_OBJECTION_TEXT = MAX_NOTE_TEXT - OBJECTION_PREFIX.length;
const MAX_REVISION_TEXT = MAX_NOTE_TEXT - REVISION_PREFIX.length;
const thoughtTitle = value => value.slice(0, 512).replace(/[\uD800-\uDBFF]$/, '');
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const only = (value, fields) => object(value) && Object.keys(value).length === fields.length && fields.every(field => Object.hasOwn(value, field));
const makeId = prefix => `${prefix}-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`;
const el = (tag, className = '', value) => { const node = document.createElement(tag); if (className) node.className = className; if (value !== undefined) node.textContent = value; return node; };
const btn = (label, className = '') => { const node = el('button', className, label); node.type = 'button'; return node; };
const json = value => JSON.stringify(value);
const equal = (left, right) => json(left) === json(right);

export function createPublicResearchOwners() {
  const workspace = createBrowserResearchWorkspace({persistence: createCompareAndSwapPersistence(globalThis.localStorage, WORKSPACE_KEY)});
  const shelf = createResearchShelfStore();
  return {workspace, shelf};
}

function labels(locale) {
  return String(locale() || 'ru').toLowerCase().startsWith('en') ? {
    heading: 'My research', intro: 'Your questions and interpretations stay personal. Every source link reopens its exact text version. A manual-entry label does not verify that the author is human.',
    thesis: 'Thesis', objection: 'Objection', revision: 'Reply or revised thesis', grounds: 'Exact grounds', save: 'Save this thought',
    empty: 'No personal hypotheses yet.', proposals: 'Source-linked proposals', history: 'Recent workspace history', graph: 'My research links',
    source: 'Exact source', open: 'Open exact text', review: 'Local proposal · pending review · not canon',
    export: 'Export complete research', exportDraft: 'Export unsaved draft', import: 'Import complete research', preview: 'Review import', apply: 'Apply available imports',
    add: 'Add selected passage', question: 'Question', pick: 'Open the reader, select a passage, and add it here. A thesis needs at least two exact grounds.', graphSpace: 'Tree graphs and layout',
    noProposal: 'No proposals yet.', emptyGrounds: 'Select exact passages above before saving.', undo: 'Undo research action', redo: 'Redo research action',
  } : {
    heading: 'Моё исследование', intro: 'Ваши вопросы и выводы остаются личными. Каждая ссылка открывает точную версию текста. Метка ручного ввода не удостоверяет, что автор — человек.',
    thesis: 'Тезис', objection: 'Возражение', revision: 'Ответ или пересмотренный тезис', grounds: 'Точные основания', save: 'Сохранить эту мысль',
    empty: 'Личных гипотез пока нет.', proposals: 'Предложения с источниками', history: 'Последние действия', graph: 'Связи моего исследования',
    source: 'Точный источник', open: 'Открыть точный текст', review: 'Личное предложение · ожидает проверки · не канон',
    export: 'Экспорт полного исследования', exportDraft: 'Экспорт несохранённого черновика', import: 'Импорт полного исследования', preview: 'Проверить импорт', apply: 'Применить доступный импорт',
    add: 'Добавить выбранный фрагмент', question: 'Вопрос', pick: 'Откройте чтение, выберите место и добавьте его здесь. Для тезиса нужны два точных основания.', graphSpace: 'Графы Древа и раскладка',
    noProposal: 'Предложений пока нет.', emptyGrounds: 'Перед сохранением выберите точные фрагменты.', undo: 'Отменить действие исследования', redo: 'Вернуть действие исследования',
  };
}

function workspaceEmpty(packet) {
  return !packet.selected_lens && !packet.excluded_edge_ids?.length && !packet.route_snapshots?.length
    && !packet.hypotheses?.length && !packet.proposals?.length && !packet.notes?.length;
}

function ownerConflicts(current, incoming, owner) {
  const conflicts = [];
  const by = (rows, key) => new Map((rows || []).map(row => [row[key], row]));
  if (owner === 'workspace') {
    const currentRows = current;
    if (equal(currentRows, incoming)) return conflicts;
    if (workspaceEmpty(incoming)) return conflicts;
    if (workspaceEmpty(currentRows)) return conflicts;
    conflicts.push('The current Rust workspace is non-empty and differs from this packet.');
    return conflicts;
  }
  const records = by(current.records, 'id');
  for (const item of incoming.records || []) if (records.has(item.id) && !equal(records.get(item.id), item)) conflicts.push(`shelf record: ${item.id}`);
  const collections = by(current.collections, 'id');
  for (const item of incoming.collections || []) if (collections.has(item.id) && !equal(collections.get(item.id), item)) conflicts.push(`shelf collection: ${item.id}`);
  return conflicts;
}

/** Keep each existing notebook record and select only equal or new incoming records. */
export function planNotebookImport(current, incoming) {
  const packet = {...incoming}, counts = {}, conflicts = [];
  for (const [name, key] of [['notes', 'id'], ['readings', 'slot'], ['preferences', 'key']]) {
    const existing = new Map((current[name] || []).map(item => [item[key], item]));
    const tally = {added: 0, equal: 0, skipped: 0};
    packet[name] = (incoming[name] || []).filter(item => {
      if (!existing.has(item[key])) { tally.added++; return true; }
      if (equal(existing.get(item[key]), item)) { tally.equal++; return true; }
      tally.skipped++; conflicts.push(`${name}: ${item[key]}`); return false;
    });
    counts[name] = tally;
  }
  return {packet, counts, conflicts};
}

/** Undo or redo a just-saved thought as one UI action while its group is current. */
export function changeResearchHistory(workspace, direction, group) {
  if (!['undo', 'redo'].includes(direction)) throw new TypeError('Unknown research history action.');
  const expectedState = direction === 'undo' ? 'applied' : 'undone';
  const grouped = group?.state === expectedState && group.revision === workspace.summary().revision;
  const steps = grouped ? group.actions : 1;
  let changed = 0;
  for (let index = 0; index < steps; index++) {
    if (!workspace[direction]()) break;
    changed++;
  }
  const complete = grouped && changed === steps;
  return {changed, grouped:complete, group:complete
    ? {...group, state:direction === 'undo' ? 'undone' : 'applied', revision:workspace.summary().revision} : null};
}

/** Read an explicit proposal target, or label the deterministic legacy agent pattern as inferred. */
export function proposalResponseTarget(proposal, proposals) {
  const explicit = proposals.find(item => item.id === proposal.targetId);
  if (explicit) return {proposal:explicit, inferred:false};
  if (proposal.actorOrigin !== 'agent' || !proposal.id.startsWith('agent-')
    || !proposal.id.endsWith('-objection') || !/^Objection:/i.test(proposal.statement)) return null;
  const legacy = proposals.find(item => item.id === proposal.id.slice(0, -'-objection'.length)
    && item.actorOrigin === 'agent' && item.parentHypothesisId === proposal.parentHypothesisId);
  return legacy ? {proposal:legacy, inferred:true} : null;
}

export async function validateAggregate(value) {
  const baseFields = ['schema', 'version', 'exportedAt', 'notebook', 'workspace', 'shelf'];
  const fields = object(value) && Object.hasOwn(value, 'graphSpace') ? [...baseFields, 'graphSpace'] : baseFields;
  if (!only(value, fields)
    || value.schema !== PUBLIC_WORKSPACE_SCHEMA || value.version !== PUBLIC_WORKSPACE_VERSION
    || typeof value.exportedAt !== 'string' || !Number.isFinite(Date.parse(value.exportedAt))) throw new TypeError('Unsupported complete research packet.');
  const graphSpace = validateGraphSpace(value.graphSpace);
  const workspace = createBrowserResearchWorkspace({persistence: false});
  let checkedWorkspace;
  try { workspace.importPacket(json(value.workspace)); checkedWorkspace = JSON.parse(workspace.exportPacket()); }
  finally { workspace.dispose(); }
  const notebook = createCorpusNotebook({indexedDB: null, dbName: 'tos-public-research-import-check-v1'});
  let checkedNotebook;
  try { await notebook.importPacket(value.notebook); checkedNotebook = await notebook.exportPacket(); }
  finally { await notebook.close(); }
  const shelf = createResearchShelfStore({adapter: 'memory'});
  let checkedShelf;
  try { await shelf.import(value.shelf); checkedShelf = await shelf.exportPacket(); }
  finally { await shelf.close(); }
  const result = {schema: PUBLIC_WORKSPACE_SCHEMA, version: PUBLIC_WORKSPACE_VERSION,
    exportedAt: new Date(value.exportedAt).toISOString(), notebook: checkedNotebook, workspace: checkedWorkspace, shelf: checkedShelf,
    ...(graphSpace ? {graphSpace} : {})};
  if (new TextEncoder().encode(json(result)).byteLength > MAX_EXPORT_BYTES) throw new TypeError('The complete research packet exceeds 12 MB.');
  return result;
}

async function sourceFingerprint(locators) {
  const bytes = new TextEncoder().encode(JSON.stringify([...new Set(locators)].sort()));
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return `sha256:${[...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

/** Check generated proposal statements before saving the thought to its existing owners. */
export async function savePublicThought({thesis, grounds, objection, objectionSources, revision, api, workspace, shelf, notebook, labels: t}) {
  const inputText = (value, limit, field) => {
    if (typeof value !== 'string') throw new TypeError(`${field} must be text.`);
    const checked = value.trim();
    if (checked.length > limit) throw new TypeError(`${field} must contain at most ${limit} characters.`);
    return checked;
  };
  const claim = inputText(thesis, MAX_NOTE_TEXT, 'The thesis');
  const objectionText = inputText(objection, MAX_OBJECTION_TEXT, 'The objection');
  const revisionText = inputText(revision, MAX_REVISION_TEXT, 'The revision');
  if (!claim) throw new TypeError('The thesis must not be empty.');
  if (!Array.isArray(grounds) || grounds.length < 2) throw new TypeError(t().pick);
  if (objectionText && (!Array.isArray(objectionSources) || objectionSources.length < 1)) throw new TypeError('Choose an exact source for the objection.');

  const objectionStatement = objectionText ? `${OBJECTION_PREFIX}${objectionText}` : '';
  const revisionStatement = revisionText ? `${REVISION_PREFIX}${revisionText}` : '';
  for (const [field, value] of [['the thesis proposal', claim], ['the objection proposal', objectionStatement], ['the revision proposal', revisionStatement]]) {
    if (value.length > MAX_NOTE_TEXT) throw new TypeError(`${field} exceeds the 4,000-character workspace statement limit.`);
  }
  const noteText = `${t().thesis}: ${claim}${objectionText ? `\n\n${t().objection}: ${objectionText}` : ''}${revisionText ? `\n\n${t().revision}: ${revisionText}` : ''}`;

  const toRefs = sources => sources.map(source => {
    if (!source || typeof source !== 'object') throw new TypeError('An exact source selection is invalid.');
    const reference = validateReference(source.reference);
    return {reference, locator: JSON.stringify(reference), key: referenceKey(reference), source};
  });
  const groundRefs = toRefs(grounds), objectionRefs = objectionText ? toRefs(objectionSources) : [];
  for (const item of [...groundRefs, ...objectionRefs]) {
    const result = await api.provider.resolve({reference: item.reference});
    if (result.status !== 'exact') throw new Error(t().open + ': stale source references cannot be retargeted.');
  }

  const thesisId = makeId('thesis');
  const objectionId = objectionText ? makeId('objection') : null;
  const revisionId = revisionText ? makeId('revision') : null;
  const title = thoughtTitle(`${api.getQuestion().trim() || t().thesis}: ${claim}`);

  const sourceLocators = new Map();
  for (const item of [...groundRefs, ...objectionRefs]) {
    if (!sourceLocators.has(item.key)) sourceLocators.set(item.key, await ensureSourceRecord(shelf, item.reference, item.source.title));
  }
  const compactRefs = sources => [...new Set(sources.map(item => sourceLocators.get(item.key)))];
  const fingerprints = {
    grounds: await sourceFingerprint(compactRefs(groundRefs)),
    ...(objectionText ? {objection: await sourceFingerprint(compactRefs(objectionRefs))} : {}),
    ...(revisionText ? {revision: await sourceFingerprint(compactRefs(objectionRefs.length ? objectionRefs : groundRefs))} : {}),
  };
  const stage = ({id, kind, targetId, fromId, toId, statement, refs, dataFingerprint}) => {
    const sourceRefs = compactRefs(refs);
    workspace.stageProposal({id, kind, parentHypothesisId: thesisId, ...(targetId ? {targetId} : {}), ...(fromId ? {fromId} : {}), ...(toId ? {toId} : {}),
      statement, sourceRefs, evidenceRefs: [...sourceRefs], confidencePosture: {value: 'unknown', meaning: 'maker_declared_uncertainty_not_truth_probability'},
      actorOrigin: 'human', basePageRevision: 0, baseWorkspaceRevision: workspace.summary().revision, dataFingerprint});
  };
  workspace.addHypothesis({id: thesisId, title, body: claim});
  stage({id: makeId('ground'), kind: 'interpretation', targetId: thesisId, statement: claim, refs: groundRefs, dataFingerprint: fingerprints.grounds});
  if (objectionId) {
    workspace.addHypothesis({id: objectionId, title: thoughtTitle(`${t().objection}: ${objectionText}`), body: objectionText});
    stage({id: makeId('objection-link'), kind: 'relation', fromId: thesisId, toId: objectionId, statement: objectionStatement, refs: objectionRefs, dataFingerprint: fingerprints.objection});
  }
  if (revisionId) {
    workspace.addHypothesis({id: revisionId, title: thoughtTitle(`${t().revision}: ${revisionText}`), body: revisionText});
    const refs = objectionRefs.length ? objectionRefs : groundRefs;
    stage({id: makeId('revision-link'), kind: 'relation', fromId: objectionId || thesisId, toId: revisionId, statement: revisionStatement, refs, dataFingerprint: fingerprints.revision});
  }
  for (const item of groundRefs) await notebook.putNote({id: makeId('research-ground'), kind: 'note', reference: item.reference,
    quote: item.source.quote || undefined, text: noteText});
}

export function mountPublicWorkspaceView({root, api, notebook, workspace, shelf, locale = () => 'ru', graphSpaceContext = null} = {}) {
  if (!root || !api || !notebook || !workspace || !shelf) throw new TypeError('The personal workspace needs the shared Rust workspace, notebook, and shelf owners.');
  const t = () => labels(locale);
  const section = el('details', 'pr-workspace'); section.setAttribute('aria-labelledby', 'pr-workspace-title');
  const title = el('summary', 'pr-workspace-title', t().heading); title.id = 'pr-workspace-title';
  const intro = el('p', 'pr-workspace-intro', t().intro);
  const toolbar = el('div', 'pr-workspace-toolbar');
  const undo = btn(t().undo, 'pr-workspace-button'); const redo = btn(t().redo, 'pr-workspace-button');
  const addSource = btn(t().add, 'pr-workspace-button'); addSource.disabled = true;
  const exportButton = btn(t().export, 'pr-workspace-button pr-workspace-primary');
  const draftExportButton = btn(t().exportDraft, 'pr-workspace-button'); draftExportButton.hidden = true;
  const importButton = btn(t().import, 'pr-workspace-button');
  const importFile = document.createElement('input'); importFile.type = 'file'; importFile.accept = 'application/json,.json'; importFile.hidden = true;
  const content = el('div', 'pr-workspace-content');
  const status = el('p', 'pr-workspace-status'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const preview = el('div', 'pr-workspace-import-preview');
  toolbar.append(undo, redo, addSource, exportButton, draftExportButton, importButton, importFile);
  section.append(title, intro, toolbar, content, preview, status);
  root.append(section);

  let sourceItems = [];
  let sourcePicker = null;
  let proposalForm = null;
  let pendingImport = null;
  let importRevision = null;
  let unsubSources = null;
  let savingThought = false;
  let lastThoughtGroup = null;
  const message = value => {
    const error = workspace.persistenceError?.();
    draftExportButton.hidden = !error;
    if (!error) { status.textContent = value; return; }
    const english = String(locale() || 'ru').toLowerCase().startsWith('en');
    const warning = english
      ? `Another tab changed this research workspace. Your current draft is not saved. Export it before reloading.`
      : `В другой вкладке изменилась рабочая область. Текущий черновик не сохранён. Экспортируйте его перед перезагрузкой.`;
    status.textContent = value && !String(value).includes(warning) ? `${value} ${warning}` : (value || warning);
  };
  const updateQuestionLine = () => {
    const line = proposalForm?.querySelector('.pr-workspace-question');
    if (line) line.textContent = researchQuestionLine(api.getQuestion(), locale);
  };
  const unsubscribeQuestion = api.subscribeQuestion?.(updateQuestionLine);
  const resolveAndOpen = async reference => {
    const checked = validateReference(reference);
    const result = await api.provider.resolve({reference: checked});
    if (result.status !== 'exact') throw new Error(t().open + ': source is stale or unavailable.');
    const exact = result.reference;
    await api.open({documentId: exact.target.workId, versionId: exact.versionId, unitId: exact.unitId,
      revision: exact.target.textLayerSha256, reference: exact});
  };
  const sourceLabel = source => {
    const selector = source.reference?.selector;
    const range = Number.isSafeInteger(selector?.start) && Number.isSafeInteger(selector?.end) && selector.end > selector.start
      ? `${selector.start}–${selector.end} ${isEnglish(locale) ? 'characters' : 'знаков'}` : '';
    return [source.title, source.versionLabel, range].filter(Boolean).join(' · ');
  };
  const sourceButton = source => {
    const action = btn(sourceLabel(source), 'pr-workspace-source-link');
    action.addEventListener('click', () => void resolveAndOpen(source.reference).catch(error => message(displayPublicError(error, locale))));
    return action;
  };
  const subscribeSourceList = bridge => {
    sourcePicker = bridge; addSource.disabled = !bridge?.addSelectedSource;
    unsubSources?.();
    unsubSources = bridge?.subscribeSources?.(sources => {
      sourceItems = sources;
      if (proposalForm) { const current = proposalForm; current.replaceWith(renderForm()); }
    }) || null;
  };
  const selectedRefs = (group) => [...group.querySelectorAll('input[type="checkbox"]:checked')].map(input => sourceItems.find(source => source.id === input.value)).filter(Boolean);
  const renderForm = () => {
    const previous = proposalForm;
    const previousFields = previous ? [...previous.querySelectorAll('.pr-workspace-field textarea')].map(node => node.value) : [];
    const previousGroups = previous ? [...previous.querySelectorAll('.pr-workspace-source-picker')].map(group => [...group.querySelectorAll('input:checked')].map(node => node.value)) : [];
    const form = el('form', 'pr-workspace-editor'); proposalForm = form;
    const question = el('p', 'pr-workspace-question', researchQuestionLine(api.getQuestion(), locale));
    const thesisLabel = el('label', 'pr-workspace-field', t().thesis); const thesis = document.createElement('textarea'); thesis.maxLength = MAX_NOTE_TEXT; thesis.rows = 3; thesis.required = true; thesis.value = previousFields[0] ?? ''; thesisLabel.append(thesis);
    const sourceGroup = el('fieldset', 'pr-workspace-source-picker'); sourceGroup.append(el('legend', '', t().grounds));
    if (!sourceItems.length) sourceGroup.append(el('p', 'pr-workspace-muted', t().pick));
    for (const source of sourceItems) { const label = el('label', 'pr-workspace-checkbox'); const input = document.createElement('input'); input.type = 'checkbox'; input.value = source.id; label.append(input, el('span', '', sourceLabel(source))); label.append(sourceButton(source)); sourceGroup.append(label); }
    const objectionLabel = el('label', 'pr-workspace-field', t().objection); const objection = document.createElement('textarea'); objection.maxLength = MAX_OBJECTION_TEXT; objection.rows = 2; objection.value = previousFields[1] ?? ''; objectionLabel.append(objection,
      el('small', 'pr-workspace-muted', String(locale() || 'ru').toLowerCase().startsWith('en') ? `Up to ${MAX_OBJECTION_TEXT} characters.` : `До ${MAX_OBJECTION_TEXT} знаков.`));
    const objectionGroup = el('fieldset', 'pr-workspace-source-picker'); objectionGroup.append(el('legend', '', `${t().objection} · ${t().grounds}`));
    for (const source of sourceItems) { const label = el('label', 'pr-workspace-checkbox'); const input = document.createElement('input'); input.type = 'checkbox'; input.value = source.id; label.append(input, el('span', '', sourceLabel(source))); objectionGroup.append(label); }
    const revisionLabel = el('label', 'pr-workspace-field', t().revision); const revision = document.createElement('textarea'); revision.maxLength = MAX_REVISION_TEXT; revision.rows = 2; revision.value = previousFields[2] ?? ''; revisionLabel.append(revision,
      el('small', 'pr-workspace-muted', String(locale() || 'ru').toLowerCase().startsWith('en') ? `Up to ${MAX_REVISION_TEXT} characters.` : `До ${MAX_REVISION_TEXT} знаков.`));
    for (const group of [sourceGroup, objectionGroup]) {
      const priorIds = previousGroups.shift() || [];
      for (const input of group.querySelectorAll('input[type="checkbox"]')) input.checked = priorIds.includes(input.value);
    }
    const save = btn(t().save, 'pr-workspace-button pr-workspace-primary');
    save.type = 'submit'; save.disabled = savingThought;
    form.append(question, thesisLabel, sourceGroup, objectionLabel, objectionGroup, revisionLabel, save);
    form.addEventListener('submit', event => {
      event.preventDefault();
      if (savingThought) return;
      const submitted = {thesis: thesis.value, grounds: selectedRefs(sourceGroup), objection: objection.value, objectionSources: selectedRefs(objectionGroup), revision: revision.value};
      const signature = draftSignature(); savingThought = true; save.disabled = true; undo.disabled = true; redo.disabled = true;
      void saveThought(submitted).then(() => {
        if (draftSignature() === signature) { proposalForm = null; render(); }
      }).catch(error => message(displayPublicError(error, locale))).finally(() => {
        savingThought = false;
        const currentSave = proposalForm?.querySelector('button[type="submit"]'); if (currentSave) currentSave.disabled = false;
        const summary = workspace.summary(); undo.disabled = !summary.can_undo; redo.disabled = !summary.can_redo;
      });
    });
    return form;
  };

  const draftSignature = () => json({fields: [...(proposalForm?.querySelectorAll('.pr-workspace-field textarea') || [])].map(node => node.value),
    groups: [...(proposalForm?.querySelectorAll('.pr-workspace-source-picker') || [])].map(group => [...group.querySelectorAll('input:checked')].map(node => node.value))});
  const hasUnsavedDraft = () => Boolean(proposalForm && ([...proposalForm.querySelectorAll('.pr-workspace-field textarea')].some(node => node.value.trim())
    || proposalForm.querySelector('.pr-workspace-source-picker input:checked')));

  async function saveThought({thesis, grounds, objection, objectionSources, revision}) {
    if (api.ensureCurrentQuestion && !(await api.ensureCurrentQuestion())) throw new Error('The research question has unsaved changes or changed in another tab.');
    const storedWorkspace = globalThis.localStorage.getItem(WORKSPACE_KEY);
    if (workspace.persistenceError?.() || (storedWorkspace !== null && storedWorkspace !== workspace.exportPacket())) {
      throw new Error('The research workspace changed in another tab. Review the saved state before continuing.');
    }
    await savePublicThought({thesis, grounds, objection, objectionSources, revision, api, workspace, shelf, notebook, labels: t});
    await api.reader?.refreshNotebook?.();
    if (workspace.persistenceError?.()) throw new Error('The research workspace changed in another tab. Export your unsaved workspace draft before reloading.');
    lastThoughtGroup = {state:'applied', revision:workspace.summary().revision,
      actions:2 + (objection.trim() ? 2 : 0) + (revision.trim() ? 2 : 0)};
    render(); message(isEnglish(locale) ? 'Saved locally. Earlier hypotheses remain in history; proposals are pending review.' : 'Сохранено на этом устройстве. Прежние гипотезы остаются в журнале; предложения ожидают проверки.');
  }

  const renderGraph = state => {
    const graph = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); graph.setAttribute('class', 'pr-workspace-graph'); graph.setAttribute('role', 'img'); graph.setAttribute('aria-label', t().graph);
    const nodes = state.hypotheses; const width = Math.max(720, 172 + Math.max(0, nodes.length - 1) * 137);
    graph.setAttribute('viewBox', `0 0 ${width} 210`); graph.style.width = `${width}px`;
    const index = new Map(nodes.map((node, i) => [node.id, {x: 86 + i * 137, y: 94, node, ordinal: i + 1}]));
    const edges = state.proposals.filter(proposal => proposal.kind === 'relation' && index.has(proposal.fromId) && index.has(proposal.toId));
    for (const edge of edges) {
      const from = index.get(edge.fromId), to = index.get(edge.toId); const line = document.createElementNS('http://www.w3.org/2000/svg', 'line');
      line.setAttribute('x1', String(from.x)); line.setAttribute('y1', '94'); line.setAttribute('x2', String(to.x)); line.setAttribute('y2', '94'); line.setAttribute('class', 'pr-workspace-edge'); graph.append(line);
    }
    for (const [i, entry] of [...index.values()].entries()) {
      const group = document.createElementNS('http://www.w3.org/2000/svg', 'g'); group.setAttribute('class', 'pr-workspace-node'); group.setAttribute('transform', `translate(${entry.x} 94)`);
      const circle = document.createElementNS('http://www.w3.org/2000/svg', 'circle'); circle.setAttribute('r', '32'); group.append(circle);
      const caption = document.createElementNS('http://www.w3.org/2000/svg', 'text'); caption.setAttribute('y', '53'); caption.setAttribute('text-anchor', 'middle'); caption.textContent = `${entry.ordinal}`; const nodeTitle = document.createElementNS('http://www.w3.org/2000/svg', 'title'); nodeTitle.textContent = entry.node.title; group.append(nodeTitle); group.append(caption); graph.append(group);
    }
    if (!nodes.length) { const empty = document.createElementNS('http://www.w3.org/2000/svg', 'text'); empty.setAttribute('class', 'pr-workspace-graph-empty'); empty.setAttribute('x', '18'); empty.setAttribute('y', '28'); empty.textContent = t().empty; graph.append(empty); }
    return graph;
  };
  const referenceButton = (locator, index) => {
    const label = `${t().open} ${index + 1}`;
    const button = btn(label, 'pr-workspace-source-link');
    void describeSourceLocator(locator, shelf, api.provider, locale()).then(result => {
      if (!button.isConnected) return;
      button.textContent = `${label} · ${result.label}`;
      button.dataset.state = result.status;
    }).catch(() => {});
    button.addEventListener('click', () => void resolveSourceLocator(locator, shelf, api.provider).then(result => {
      if (result.status !== 'exact') throw new Error('This saved source is stale or unavailable. It was not retargeted.');
      return resolveAndOpen(result.reference);
    }).then(() => { button.dataset.state = 'exact'; }).catch(() => {
      button.dataset.state = 'stale'; button.textContent = `${label} · ${String(locale()).startsWith('en') ? 'changed or unavailable' : 'изменился или недоступен'}`;
      message(String(locale()).startsWith('en') ? 'This saved source is stale or unavailable. It was not retargeted.' : 'Сохранённый источник изменился или недоступен. Адрес не переназначен.');
    }));
    return button;
  };
  function render() {
    const state = workspace.getState(); const summary = workspace.summary();
    draftExportButton.hidden = !workspace.persistenceError?.();
    if (workspace.persistenceError?.()) message(status.textContent);
    undo.disabled = savingThought || !summary.can_undo; redo.disabled = savingThought || !summary.can_redo;
    content.replaceChildren();
    const form = renderForm(); content.append(form);
    const graphSection = el('section', 'pr-workspace-panel'); const graphScroll = el('div', 'pr-workspace-graph-scroll'); graphScroll.append(renderGraph(state));
    graphSection.append(el('h3', '', t().graph), graphScroll); content.append(graphSection);
    const hypotheses = el('section', 'pr-workspace-panel'); hypotheses.append(el('h3', '', t().heading));
    if (!state.hypotheses.length) hypotheses.append(el('p', 'pr-workspace-muted', t().empty));
    for (const [hypothesisIndex, hypothesis] of state.hypotheses.entries()) {
      const card = el('article', 'pr-workspace-card'); card.dataset.hypothesisId = hypothesis.id;
      card.append(el('h4', '', `${hypothesisIndex + 1}. ${hypothesis.title}`), el('p', '', hypothesis.body), el('small', 'pr-workspace-muted', hypothesis.posture?.canon === false ? (isEnglish(locale) ? 'personal · unreviewed · not canon' : 'личное · не проверено · не канон') : (isEnglish(locale) ? 'personal' : 'личное')));
      hypotheses.append(card);
    }
    content.append(hypotheses);
    const proposals = el('section', 'pr-workspace-panel'); proposals.append(el('h3', '', t().proposals));
    if (!state.proposals.length) proposals.append(el('p', 'pr-workspace-muted', t().noProposal));
    for (const proposal of state.proposals) {
      const card = el('article', 'pr-workspace-card pr-workspace-proposal');
      card.dataset.proposalId = proposal.id; card.tabIndex = -1;
      card.append(el('h4', '', kindLabel(proposal.kind, locale)), el('p', '', proposalStatement(proposal.statement, locale)), el('small', 'pr-workspace-muted', t().review));
      const targetMatch = proposalResponseTarget(proposal, state.proposals);
      if (targetMatch) {
        const targetProposal = targetMatch.proposal;
        const prefix = targetMatch.inferred
          ? (isEnglish(locale) ? 'Possible objection to (legacy record)' : 'Возможное возражение к (старая запись)')
          : /^Objection:/i.test(proposal.statement)
          ? (isEnglish(locale) ? 'Objection to' : 'Возражение к')
          : (isEnglish(locale) ? 'Related proposal' : 'Связанное предложение');
        const statement = Array.from(targetProposal.statement);
        const excerpt = statement.slice(0, 180).join('') + (statement.length > 180 ? '…' : '');
        const target = btn(`${prefix}: ${excerpt}`, 'pr-workspace-source-link');
        target.addEventListener('click', () => {
          const card = [...proposals.querySelectorAll('[data-proposal-id]')].find(item => item.dataset.proposalId === targetProposal.id);
          card?.scrollIntoView({block:'nearest'}); card?.focus({preventScroll:true});
        });
        card.append(target);
      }
      const refs = [...new Set([...proposal.sourceRefs, ...proposal.evidenceRefs])];
      const links = el('div', 'pr-workspace-reference-list'); refs.forEach((ref, index) => links.append(referenceButton(ref, index)));
      if (!refs.length) links.append(el('span', 'pr-workspace-muted', isEnglish(locale) ? 'No source references.' : 'Ссылок на источники нет.'));
      card.append(el('h5', '', t().source), links); proposals.append(card);
    }
    content.append(proposals);
    const history = el('section', 'pr-workspace-panel'); history.append(el('h3', '', t().history));
    for (const entry of state.journal.slice(-12).reverse()) {
      const target = entry.targetId ? state.hypotheses.find(item => item.id === entry.targetId)?.title || entry.targetId : '';
      history.append(el('p', 'pr-workspace-history-row', `${entry.sequence}. ${actionLabel(entry.action, locale)}${target ? ` · ${target}` : ''}`));
    }
    content.append(history);
  }

  const exportPacket = async ({draft = false} = {}) => {
    if (api.ensureCurrentQuestion && !(await api.ensureCurrentQuestion())) throw new Error('The question changed in another tab; copy your unsaved text before exporting.');
    const localWorkspace = workspace.exportPacket();
    const savedWorkspace = globalThis.localStorage.getItem(WORKSPACE_KEY);
    const staleWorkspace = savedWorkspace !== null && savedWorkspace !== localWorkspace;
    const packet = {schema: PUBLIC_WORKSPACE_SCHEMA, version: PUBLIC_WORKSPACE_VERSION, exportedAt: new Date().toISOString(),
      notebook: await notebook.exportPacket(), workspace: JSON.parse(!draft && staleWorkspace ? savedWorkspace : localWorkspace), shelf: await shelf.exportPacket(),
      graphSpace: exportGraphSpace(globalThis.localStorage)};
    const serialized = JSON.stringify(packet, null, 2);
    if (new TextEncoder().encode(serialized).byteLength > MAX_EXPORT_BYTES) throw new TypeError('The complete research export exceeds 12 MB.');
    const link = el('a'); const url = URL.createObjectURL(new Blob([serialized], {type: 'application/json'}));
    link.href = url; link.download = `tos-public-research${draft ? '-unsaved-draft' : ''}-${new Date().toISOString().slice(0, 10)}.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    message(isEnglish(locale)
      ? (draft ? 'Unsaved workspace draft, source shelf and tree graphs were exported.' : `Complete notebook, workspace, source shelf and tree graphs/layout downloaded.${staleWorkspace ? ' The workspace was read from the latest saved state because this tab is older.' : ''}`)
      : (draft ? 'Несохранённый черновик, полка источников и графы Древа экспортированы.' : `Полный экспорт блокнота, исследования, полки источников и графов Древа скачан.${staleWorkspace ? ' Исследование взято из последнего сохранённого состояния: эта вкладка устарела.' : ''}`));
  };

  const packetCounts = packet => ({notes: packet.notebook.notes.length, readings: packet.notebook.readings.length, preferences: packet.notebook.preferences.length,
    hypotheses: packet.workspace.hypotheses.length, proposals: packet.workspace.proposals.length,
    shelfRecords: packet.shelf.records.length, collections: packet.shelf.collections.length, graphs: packet.graphSpace?.entries?.filter(item => graphKeyInfo(item.key)?.kind === 'graph').length ?? 0});
  const currentPackets = async () => ({notebook: await notebook.exportPacket(), workspace: JSON.parse(workspace.exportPacket()), shelf: await shelf.exportPacket(), graphSpace: exportGraphSpace(globalThis.localStorage)});
  const assessImport = async input => {
    const incoming = await validateAggregate(input); const current = await currentPackets();
    const currentStorage = globalThis.localStorage.getItem(WORKSPACE_KEY);
    const workspaceMatchesStorage = currentStorage === null
      ? workspaceEmpty(current.workspace)
      : currentStorage === workspace.exportPacket();
    const notebookPlan = planNotebookImport(current.notebook, incoming.notebook);
    const conflicts = {
      workspace: ownerConflicts(current.workspace, incoming.workspace, 'workspace'),
      notebook: notebookPlan.conflicts,
      shelf: ownerConflicts(current.shelf, incoming.shelf, 'shelf'),
    };
    const graph = inspectGraphSpace(globalThis.localStorage, incoming.graphSpace, graphSpaceContext);
    const currentShelfRecords = new Map(current.shelf.records.map(record => [record.id, record]));
    const incomingShelfRecords = new Map(incoming.shelf.records.map(record => [record.id, record]));
    const requiredShelfIds = new Set(incoming.workspace.proposals.flatMap(proposal => [...proposal.source_refs, ...proposal.evidence_refs])
      .filter(locator => locator.startsWith('research-shelf:')).map(locator => locator.slice('research-shelf:'.length)));
    for (const id of requiredShelfIds) {
      const existing = currentShelfRecords.get(id), incomingRecord = incomingShelfRecords.get(id);
      const exactTextRecord = record => {
        if (!record || record.type !== 'text') return false;
        try { validateReference(record.target?.reference); return true; } catch { return false; }
      };
      if (exactTextRecord(existing) && (!incomingRecord || JSON.stringify(validateReference(existing.target.reference)) === JSON.stringify(validateReference(incomingRecord.target.reference)))) continue;
      if (!incomingRecord || conflicts.shelf.length) conflicts.workspace.push(`workspace source address ${id} cannot be imported because its shelf owner packet is unavailable or conflicted.`);
    }
    if (!workspaceMatchesStorage) conflicts.workspace.push('The workspace changed in another tab; reopen and review the file again.');
    const expected = {notebookRevision: current.notebook.revision, shelfGeneration: current.shelf.generation, workspaceStorage: currentStorage, graphStorage: graph.snapshot};
    return {incoming, current, conflicts, expected, graph, notebookPlan};
  };
  const renderImportPreview = assessment => {
    preview.replaceChildren(); pendingImport = assessment; importRevision = assessment.expected;
    const summary = packet => { const c = packetCounts(packet); return isEnglish(locale) ? `notes ${c.notes}, readings ${c.readings}, hypotheses ${c.hypotheses}, proposals ${c.proposals}, shelf addresses ${c.shelfRecords}, collections ${c.collections}, graph packets ${c.graphs}` : `заметки: ${c.notes}, чтения: ${c.readings}, гипотезы: ${c.hypotheses}, предложения: ${c.proposals}, адреса на полке: ${c.shelfRecords}, коллекции: ${c.collections}, графы: ${c.graphs}`; };
    const ownerName = owner => ({workspace:isEnglish(locale)?'Research workspace':'Исследование',notebook:isEnglish(locale)?'Notebook':'Блокнот',shelf:isEnglish(locale)?'Source shelf':'Полка источников'})[owner];
    const conflictText = conflict => { if (isEnglish(locale)) return conflict; if (conflict.startsWith('The current Rust workspace')) return 'Текущее исследование не пусто и отличается от импортируемого.'; if (conflict.startsWith('The workspace changed')) return 'Исследование изменилось в другой вкладке; заново откройте и проверьте файл.'; if (conflict.startsWith('workspace source address')) return 'Адрес источника нельзя импортировать: данные полки отсутствуют или конфликтуют.'; const [key,...rest]=conflict.split(': '); const label=({notes:'Заметка',readings:'Чтение',preferences:'Настройка','shelf record':'Запись полки','shelf collection':'Коллекция полки'})[key] || 'Конфликт'; return `${label}: ${rest.join(': ')}`; };
    preview.append(el('p', '', isEnglish(locale) ? `Incoming: ${summary(assessment.incoming)}. Current: ${summary(assessment.current)}.` : `В файле: ${summary(assessment.incoming)}. Сейчас: ${summary(assessment.current)}.`));
    const list = el('ul', 'pr-workspace-conflicts');
    let conflictCount = 0, readyOwners = 0;
    for (const owner of ['workspace', 'notebook', 'shelf']) {
      if (owner === 'notebook') {
        const c = assessment.notebookPlan.counts;
        const added = c.notes.added + c.readings.added + c.preferences.added;
        const skipped = c.notes.skipped + c.readings.skipped + c.preferences.skipped;
        if (added || !skipped) readyOwners++;
        conflictCount += skipped;
        list.append(el('li', skipped ? 'pr-workspace-stale' : 'pr-workspace-owner-ok', isEnglish(locale)
          ? `Notebook: add ${c.notes.added} notes, ${c.readings.added} reading positions, ${c.preferences.added} settings; keep current records and skip ${skipped} conflicts. Original file remains unchanged.`
          : `Блокнот: добавить заметок ${c.notes.added}, позиций чтения ${c.readings.added}, настроек ${c.preferences.added}; сохранить текущие записи и пропустить конфликтов: ${skipped}. Исходный файл не изменится.`));
        continue;
      }
      const items = assessment.conflicts[owner];
      if (!items.length) { readyOwners++; list.append(el('li', 'pr-workspace-owner-ok', `${ownerName(owner)}: ${isEnglish(locale) ? 'ready for additive import.' : 'готово к добавлению данных.'}`)); }
      for (const conflict of items) { conflictCount++; list.append(el('li', 'pr-workspace-stale', `${ownerName(owner)}: ${conflictText(conflict)}`)); }
    }
    if (assessment.incoming.graphSpace) {
      const graphName = info => info.kind === 'graph'
        ? (isEnglish(locale) ? `Graph ${info.profile} (${info.fingerprint.slice(0, 8)})` : `Граф ${info.profile} (${info.fingerprint.slice(0, 8)})`)
        : info.kind === 'owners'
          ? (isEnglish(locale) ? `Graph bindings ${info.profile} (${info.fingerprint.slice(0, 8)})` : `Связи графа ${info.profile} (${info.fingerprint.slice(0, 8)})`)
          : (isEnglish(locale) ? `Graph positions ${info.profile}` : `Позиции графа ${info.profile}`);
      const graphAvailable = assessment.graph.addable > 0;
      if (graphAvailable) { readyOwners++; list.append(el('li', 'pr-workspace-owner-ok', `${t().graphSpace}: ${isEnglish(locale) ? `${assessment.graph.addable} records ready for additive import` : `к добавлению: ${assessment.graph.addable} записей`} `)); }
      else if (assessment.graph.equal) list.append(el('li', 'pr-workspace-owner-ok', `${t().graphSpace}: ${isEnglish(locale) ? 'already equal; current state kept' : 'данные совпадают; сохранено текущее состояние'}`));
      else if (!assessment.graph.conflicts.length) list.append(el('li', 'pr-workspace-owner-ok', `${t().graphSpace}: ${isEnglish(locale) ? 'no records in this file' : 'в файле нет записей'}`));
      for (const entry of assessment.graph.conflicts) {
        conflictCount++;
        const detail = entry.status === 'blocked'
          ? (isEnglish(locale) ? 'skipped because its graph record conflicts; current data is kept' : 'пропущено: исходный граф конфликтует; текущие данные сохранены')
          : (isEnglish(locale) ? 'differs from current browser data; original will be kept' : 'отличается от данных браузера; исходная запись будет сохранена');
        list.append(el('li', 'pr-workspace-stale', `${t().graphSpace} · ${graphName(entry)}: ${detail}`));
      }
    } else {
      list.append(el('li', 'pr-workspace-stale', t().graphSpace + ': ' + (isEnglish(locale) ? 'not included in this older v1 file' : 'не включены в этот старый файл v1')));
    }
    preview.append(list);
    const apply = btn(t().apply, 'pr-workspace-button pr-workspace-primary'); apply.disabled = readyOwners === 0;
    apply.addEventListener('click', () => void applyImport().catch(error => message(displayPublicError(error, locale))));
    preview.append(apply);
    message(isEnglish(locale) ? (conflictCount ? 'Conflicting records will be kept; all available sections can still be imported.' : 'All included sections are valid for additive import.') : (conflictCount ? 'Конфликтующие записи останутся без изменений; доступные разделы можно добавить.' : 'Все включённые разделы проверены для добавления.'));
  };
  const applyImport = async () => {
    if (!pendingImport || !importRevision) return;
    if (api.ensureCurrentQuestion && !(await api.ensureCurrentQuestion())) throw new Error('The research question has unsaved changes or changed in another tab.');
    const fresh = await assessImport(pendingImport.incoming);
    if (fresh.expected.workspaceStorage !== importRevision.workspaceStorage || fresh.expected.notebookRevision !== importRevision.notebookRevision || fresh.expected.shelfGeneration !== importRevision.shelfGeneration || JSON.stringify(fresh.expected.graphStorage) !== JSON.stringify(importRevision.graphStorage)) {
      renderImportPreview(fresh); message(isEnglish(locale) ? 'Research changed after preview. Review the refreshed conflicts before importing.' : 'Исследование изменилось после просмотра. Перед импортом проверьте обновлённые конфликты.'); return;
    }
    const currentProfile = graphSpaceContext?.profile;
    const incomingGraphKeys = fresh.graph.entries.filter(item => item.status === 'missing' || item.status === 'pristine').map(item => item.key);
    if (hasUnsavedDraft() && shouldReloadImportedGraph(incomingGraphKeys, currentProfile)) {
      message(isEnglish(locale) ? 'Save or copy your research draft before importing: applying this graph reloads the page.' : 'Сохраните или скопируйте черновик исследования перед импортом: применение графа перезагрузит страницу.');
      return;
    }
    const outcomes = [];
    if (fresh.conflicts.shelf.length) outcomes.push(isEnglish(locale) ? 'Source shelf: skipped due to conflicts' : 'Полка источников: пропущена из-за конфликтов');
    else {
      const shelfResult = await shelf.import(fresh.incoming.shelf, {expectedGeneration: fresh.expected.shelfGeneration});
      outcomes.push(isEnglish(locale) ? `Source shelf: added ${shelfResult.counts.records} addresses and ${shelfResult.counts.collections} collections` : `Полка источников: добавлено адресов ${shelfResult.counts.records}, коллекций ${shelfResult.counts.collections}`);
    }
    const notebookResult = await notebook.importPacket(fresh.notebookPlan.packet, {expectedRevision: fresh.expected.notebookRevision});
    await api.reader?.refreshNotebook?.();
    const nc = fresh.notebookPlan.counts, skippedNotebook = nc.notes.skipped + nc.readings.skipped + nc.preferences.skipped;
    outcomes.push(isEnglish(locale)
      ? `Notebook: added ${nc.notes.added} notes, ${nc.readings.added} reading positions and ${nc.preferences.added} settings; ${skippedNotebook} conflicts kept current (revision ${notebookResult.revision})`
      : `Блокнот: добавлено заметок ${nc.notes.added}, позиций ${nc.readings.added}, настроек ${nc.preferences.added}; текущие записи сохранены при конфликтах: ${skippedNotebook} (версия ${notebookResult.revision})`);
    if (fresh.conflicts.workspace.length) outcomes.push(isEnglish(locale) ? 'Research workspace: skipped due to conflicts' : 'Исследование: пропущено из-за конфликтов');
    else if (equal(fresh.current.workspace, fresh.incoming.workspace) || workspaceEmpty(fresh.incoming.workspace)) outcomes.push(isEnglish(locale) ? 'Research workspace: already equal or empty; current state kept' : 'Исследование: данные совпадают или пусты; сохранено текущее состояние');
    else { workspace.importPacket(JSON.stringify(fresh.incoming.workspace)); outcomes.push(isEnglish(locale) ? 'Research workspace: imported atomically' : 'Исследование: импортировано целиком'); }
    const graphResult = applyGraphSpace(globalThis.localStorage, fresh.incoming.graphSpace, graphSpaceContext);
    if (fresh.incoming.graphSpace) outcomes.push(isEnglish(locale)
      ? `Tree graphs/layout: added ${graphResult.added.length}, already equal ${graphResult.equal.length}, conflicts kept ${graphResult.conflicts.length}`
      : `Графы Древа и раскладка: добавлено ${graphResult.added.length}, совпадало ${graphResult.equal.length}, конфликтов сохранено ${graphResult.conflicts.length}`);
    pendingImport = null; preview.replaceChildren(); render(); message(outcomes.join(' · '));
    const questionValue = fresh.incoming.notebook.preferences.find(item => item.key === 'tos.public-research.question.v1')?.value;
    if (typeof questionValue === 'string') await api.refreshQuestion?.();
    if (shouldReloadImportedGraph(graphResult.added, currentProfile)) {
      try { globalThis.sessionStorage?.setItem('tos.public-research.import-result.v1', outcomes.join(' · ')); } catch { /* import already completed */ }
      if (typeof graphSpaceContext?.reload === 'function') graphSpaceContext.reload();
      else globalThis.location?.reload?.();
    }
  };

  addSource.addEventListener('click', () => {
    const added = sourcePicker?.addSelectedSource?.();
    const english = String(locale()).startsWith('en');
    message(added ? (english ? 'Exact passage added to your grounds.' : 'Точный фрагмент добавлен к основаниям.') : (english ? 'Select a new passage in the reader before adding it.' : 'Чтобы добавить новое основание, выберите другой фрагмент в читателе.'));
  });
  const changeHistory = direction => {
    if (savingThought) return;
    const result = changeResearchHistory(workspace, direction, lastThoughtGroup);
    if (!result.changed) return;
    lastThoughtGroup = result.group;
    if (result.grouped) {
      message(isEnglish(locale)
        ? 'The saved thought’s research actions changed together. Source notebook notes remain in place.'
        : 'Действия сохранённой мысли изменены вместе. Заметки у источников остаются на месте.');
    }
    render();
  };
  undo.addEventListener('click', () => changeHistory('undo'));
  redo.addEventListener('click', () => changeHistory('redo'));
  section.addEventListener('keydown', event => {
    if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'z'
      || event.target?.closest?.('input,textarea,select,[contenteditable="true"]')) return;
    event.preventDefault(); event.stopPropagation(); changeHistory(event.shiftKey ? 'redo' : 'undo');
  });
  exportButton.addEventListener('click', () => void exportPacket().catch(error => message(displayPublicError(error, locale))));
  draftExportButton.addEventListener('click', () => void exportPacket({draft: true}).catch(error => message(displayPublicError(error, locale))));
  importButton.addEventListener('click', () => { importFile.value = ''; importFile.click(); });
  importFile.addEventListener('change', async () => {
    const file = importFile.files?.[0]; importFile.value = ''; if (!file) return;
    if (file.size > MAX_EXPORT_BYTES) { message(isEnglish(locale) ? 'The complete research file exceeds 12 MB.' : 'Файл исследования превышает 12 МБ.'); return; }
    try { const assessment = await assessImport(JSON.parse(await file.text())); renderImportPreview(assessment); }
    catch (error) { pendingImport = null; preview.replaceChildren(); message(displayPublicError(error, locale)); }
  });
  render();
  try {
    const previousResult = globalThis.sessionStorage?.getItem('tos.public-research.import-result.v1');
    if (previousResult) {
      section.open = true; message(previousResult); globalThis.sessionStorage.removeItem('tos.public-research.import-result.v1');
      root.dispatchEvent(new CustomEvent('public-research-import-result', {bubbles:true}));
    }
  } catch { /* the result remains in the current session when storage is unavailable */ }
  const unsubscribeWorkspace = workspace.subscribe(() => { render(); section.open=true; });
  let destroyed = false;
  const refreshLocale = () => { title.textContent=t().heading; intro.textContent=t().intro; undo.textContent=t().undo; redo.textContent=t().redo; addSource.textContent=t().add; exportButton.textContent=t().export; draftExportButton.textContent=t().exportDraft; importButton.textContent=t().import; render(); if (pendingImport) renderImportPreview(pendingImport); else status.textContent=''; };
  return {workspace, shelf, connectAgentBridge: subscribeSourceList, render, refreshLocale, hasUnsavedDraft, destroy() {
    if (destroyed) return;
    destroyed = true; unsubscribeQuestion?.(); unsubSources?.(); unsubscribeWorkspace(); section.remove(); workspace.dispose(); void shelf.close();
  }};
}
