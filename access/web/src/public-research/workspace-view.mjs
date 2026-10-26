import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {referenceKey, validateReference} from '../corpus-reader/model.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {ensureSourceRecord, resolveSourceLocator} from './source-address.mjs';
import {createCompareAndSwapPersistence} from './workspace-persistence.mjs';
import './workspace-view.css';

export const PUBLIC_WORKSPACE_SCHEMA = 'tos.public_research.personal_workspace.v1';
export const PUBLIC_WORKSPACE_VERSION = 1;
const WORKSPACE_KEY = 'tos-research-workspace-v1';
const MAX_EXPORT_BYTES = 12_000_000;
const MAX_NOTE_TEXT = 4_000;
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
    heading: 'My research', intro: 'Your questions and interpretations stay personal. Every source link reopens its exact text version.',
    thesis: 'Thesis', objection: 'Objection', revision: 'Reply or revised thesis', grounds: 'Exact grounds', save: 'Save this thought',
    empty: 'No personal hypotheses yet.', proposals: 'Source-linked proposals', history: 'Recent workspace history', graph: 'My research links',
    source: 'Exact source', open: 'Open exact text', review: 'Local proposal · pending review · not canon',
    export: 'Export complete research', exportDraft: 'Export unsaved draft', import: 'Import complete research', preview: 'Review import', apply: 'Apply owner imports',
    add: 'Add selected passage', question: 'Question', pick: 'Open the reader, select a passage, and add it here. A thesis needs at least two exact grounds.',
    noProposal: 'No proposals yet.', emptyGrounds: 'Select exact passages above before saving.', undo: 'Undo', redo: 'Redo',
  } : {
    heading: 'Моё исследование', intro: 'Ваши вопросы и выводы остаются личными. Каждая ссылка открывает точную версию текста.',
    thesis: 'Тезис', objection: 'Возражение', revision: 'Ответ или пересмотренный тезис', grounds: 'Точные основания', save: 'Сохранить эту мысль',
    empty: 'Личных гипотез пока нет.', proposals: 'Предложения с источниками', history: 'Последние действия', graph: 'Связи моего исследования',
    source: 'Точный источник', open: 'Открыть точный текст', review: 'Личное предложение · ожидает review · не канон',
    export: 'Экспорт полного исследования', exportDraft: 'Экспорт несохранённого черновика', import: 'Импорт полного исследования', preview: 'Проверить импорт', apply: 'Применить импорт владельцев',
    add: 'Добавить выбранный фрагмент', question: 'Вопрос', pick: 'Откройте чтение, выберите место и добавьте его здесь. Для тезиса нужны два точных основания.',
    noProposal: 'Предложений пока нет.', emptyGrounds: 'Перед сохранением выберите точные фрагменты.', undo: 'Отменить', redo: 'Вернуть',
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
  if (owner === 'notebook') {
    for (const [key, rows] of [['notes', 'id'], ['readings', 'slot'], ['preferences', 'key']]) {
      const existing = by(current[key], rows);
      for (const item of incoming[key] || []) if (existing.has(item[rows]) && !equal(existing.get(item[rows]), item)) conflicts.push(`${key}: ${item[rows]}`);
    }
    return conflicts;
  }
  const records = by(current.records, 'id');
  for (const item of incoming.records || []) if (records.has(item.id) && !equal(records.get(item.id), item)) conflicts.push(`shelf record: ${item.id}`);
  const collections = by(current.collections, 'id');
  for (const item of incoming.collections || []) if (collections.has(item.id) && !equal(collections.get(item.id), item)) conflicts.push(`shelf collection: ${item.id}`);
  return conflicts;
}

async function validateAggregate(value) {
  if (!only(value, ['schema', 'version', 'exportedAt', 'notebook', 'workspace', 'shelf'])
    || value.schema !== PUBLIC_WORKSPACE_SCHEMA || value.version !== PUBLIC_WORKSPACE_VERSION
    || typeof value.exportedAt !== 'string' || !Number.isFinite(Date.parse(value.exportedAt))) throw new TypeError('Unsupported complete research packet.');
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
    exportedAt: new Date(value.exportedAt).toISOString(), notebook: checkedNotebook, workspace: checkedWorkspace, shelf: checkedShelf};
  if (new TextEncoder().encode(json(result)).byteLength > MAX_EXPORT_BYTES) throw new TypeError('The complete research packet exceeds 12 MB.');
  return result;
}

async function sourceFingerprint(locators) {
  const bytes = new TextEncoder().encode(JSON.stringify([...new Set(locators)].sort()));
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return `sha256:${[...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

export function mountPublicWorkspaceView({root, api, notebook, workspace, shelf, locale = () => 'ru'} = {}) {
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
  const message = value => {
    const error = workspace.persistenceError?.();
    draftExportButton.hidden = !error;
    if (!error) { status.textContent = value; return; }
    const english = String(locale() || 'ru').toLowerCase().startsWith('en');
    status.textContent = english
      ? `Another tab changed this research workspace. Your current draft is not saved. Export it before reloading. (${error})`
      : `В другой вкладке изменилась рабочая область. Текущий черновик не сохранён. Экспортируйте его перед перезагрузкой. (${error})`;
  };
  const updateQuestionLine = () => {
    const line = proposalForm?.querySelector('.pr-workspace-question');
    if (line) line.textContent = `${t().question}: ${api.getQuestion()}`;
  };
  const questionInput = event => { if (event.target?.matches?.('.pr-question')) updateQuestionLine(); };
  root.addEventListener('input', questionInput);
  const resolveAndOpen = async reference => {
    const checked = validateReference(reference);
    const result = await api.provider.resolve({reference: checked});
    if (result.status !== 'exact') throw new Error(t().open + ': source is stale or unavailable.');
    const exact = result.reference;
    await api.open({documentId: exact.target.workId, versionId: exact.versionId, unitId: exact.unitId,
      revision: exact.target.textLayerSha256, reference: exact});
  };
  const sourceButton = source => {
    const action = btn(`${source.title} · ${source.versionLabel}`, 'pr-workspace-source-link');
    action.addEventListener('click', () => void resolveAndOpen(source.reference).catch(error => message(error?.message ?? t().open)));
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
    const question = el('p', 'pr-workspace-question', `${t().question}: ${api.getQuestion()}`);
    const thesisLabel = el('label', 'pr-workspace-field', t().thesis); const thesis = document.createElement('textarea'); thesis.maxLength = MAX_NOTE_TEXT; thesis.rows = 3; thesis.required = true; thesis.value = previousFields[0] ?? ''; thesisLabel.append(thesis);
    const sourceGroup = el('fieldset', 'pr-workspace-source-picker'); sourceGroup.append(el('legend', '', t().grounds));
    if (!sourceItems.length) sourceGroup.append(el('p', 'pr-workspace-muted', t().pick));
    for (const source of sourceItems) { const label = el('label', 'pr-workspace-checkbox'); const input = document.createElement('input'); input.type = 'checkbox'; input.value = source.id; label.append(input, el('span', '', `${source.title} · ${source.versionLabel}`)); label.append(sourceButton(source)); sourceGroup.append(label); }
    const objectionLabel = el('label', 'pr-workspace-field', t().objection); const objection = document.createElement('textarea'); objection.maxLength = MAX_NOTE_TEXT; objection.rows = 2; objection.value = previousFields[1] ?? ''; objectionLabel.append(objection);
    const objectionGroup = el('fieldset', 'pr-workspace-source-picker'); objectionGroup.append(el('legend', '', `${t().objection} · ${t().grounds}`));
    for (const source of sourceItems) { const label = el('label', 'pr-workspace-checkbox'); const input = document.createElement('input'); input.type = 'checkbox'; input.value = source.id; label.append(input, el('span', '', `${source.title} · ${source.versionLabel}`)); objectionGroup.append(label); }
    const revisionLabel = el('label', 'pr-workspace-field', t().revision); const revision = document.createElement('textarea'); revision.maxLength = MAX_NOTE_TEXT; revision.rows = 2; revision.value = previousFields[2] ?? ''; revisionLabel.append(revision);
    for (const group of [sourceGroup, objectionGroup]) {
      const priorIds = previousGroups.shift() || [];
      for (const input of group.querySelectorAll('input[type="checkbox"]')) input.checked = priorIds.includes(input.value);
    }
    const save = btn(t().save, 'pr-workspace-button pr-workspace-primary');
    save.type = 'submit';
    form.append(question, thesisLabel, sourceGroup, objectionLabel, objectionGroup, revisionLabel, save);
    form.addEventListener('submit', event => { event.preventDefault(); void saveThought({thesis: thesis.value, grounds: selectedRefs(sourceGroup), objection: objection.value, objectionSources: selectedRefs(objectionGroup), revision: revision.value}).catch(error => message(error?.message ?? 'Could not save this thought.')); });
    return form;
  };

  async function saveThought({thesis, grounds, objection, objectionSources, revision}) {
    const claim = thesis.trim();
    if (!claim || claim.length > MAX_NOTE_TEXT) throw new TypeError('The thesis must contain up to 4,000 characters.');
    if (grounds.length < 2) throw new TypeError(t().pick);
    if (objection.trim() && objectionSources.length < 1) throw new TypeError('Choose an exact source for the objection.');
    const toRefs = sources => sources.map(source => {
      const reference = validateReference(source.reference);
      const locator = JSON.stringify(reference);
      return {reference, locator, key: referenceKey(reference), source};
    });
    const groundRefs = toRefs(grounds), objectionRefs = objection.trim() ? toRefs(objectionSources) : [];
    for (const item of [...groundRefs, ...objectionRefs]) {
      const result = await api.provider.resolve({reference: item.reference});
      if (result.status !== 'exact') throw new Error(t().open + ': stale source references cannot be retargeted.');
    }
    for (const item of [...groundRefs, ...objectionRefs]) {
      item.locator = await ensureSourceRecord(shelf, item.reference, item.source.title);
    }
    const dataFingerprint = await sourceFingerprint([...groundRefs, ...objectionRefs].map(item => item.locator));
    const thesisId = makeId('thesis');
    const currentQuestion = api.getQuestion().trim();
    const titleText = `${currentQuestion || t().thesis}: ${claim}`.slice(0, 512);
    workspace.addHypothesis({id: thesisId, title: titleText, body: claim});
    await stage({id: makeId('ground'), kind: 'interpretation', parentHypothesisId: thesisId, targetId: thesisId,
      statement: claim, refs: groundRefs, baseWorkspaceRevision: workspace.summary().revision});
    let objectionId = null;
    if (objection.trim()) {
      objectionId = makeId('objection');
      workspace.addHypothesis({id: objectionId, title: `${t().objection}: ${objection.trim()}`.slice(0, 512), body: objection.trim()});
      await stage({id: makeId('objection-link'), kind: 'relation', parentHypothesisId: thesisId, fromId: thesisId, toId: objectionId,
        statement: `Objection to thesis: ${objection.trim()}`, refs: objectionRefs, baseWorkspaceRevision: workspace.summary().revision});
    }
    if (revision.trim()) {
      const revisionId = makeId('revision');
      workspace.addHypothesis({id: revisionId, title: `${t().revision}: ${revision.trim()}`.slice(0, 512), body: revision.trim()});
      const refs = objectionRefs.length ? objectionRefs : groundRefs;
      await stage({id: makeId('revision-link'), kind: 'relation', parentHypothesisId: thesisId, fromId: objectionId || thesisId, toId: revisionId,
        statement: `Revises earlier thesis: ${revision.trim()}`, refs, baseWorkspaceRevision: workspace.summary().revision});
    }
    for (const item of groundRefs) {
      const noteId = makeId('research-ground');
      await notebook.putNote({id: noteId, kind: 'note', reference: item.reference,
        quote: item.source.quote || undefined,
        text: `${t().thesis}: ${claim}${objection.trim() ? `\n\n${t().objection}: ${objection.trim()}` : ''}${revision.trim() ? `\n\n${t().revision}: ${revision.trim()}` : ''}`.slice(0, 16_000)});
    }
    render(); message('Saved locally. Earlier hypotheses remain in history; proposals are pending review.');
  }

  async function stage({id, kind, parentHypothesisId, targetId, fromId, toId, statement, refs, baseWorkspaceRevision}) {
    const sourceRefs = refs.map(item => item.locator);
    const dataFingerprint = await sourceFingerprint(sourceRefs);
    workspace.stageProposal({id, kind, parentHypothesisId, ...(targetId ? {targetId} : {}), ...(fromId ? {fromId} : {}), ...(toId ? {toId} : {}),
      statement, sourceRefs, evidenceRefs: [...sourceRefs], confidencePosture: {value: 'unknown', meaning: 'maker_declared_uncertainty_not_truth_probability'},
      actorOrigin: 'human', basePageRevision: 0, baseWorkspaceRevision, dataFingerprint});
  }

  const renderGraph = state => {
    const graph = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); graph.setAttribute('class', 'pr-workspace-graph'); graph.setAttribute('viewBox', '0 0 720 210'); graph.setAttribute('role', 'img'); graph.setAttribute('aria-label', t().graph);
    const nodes = state.hypotheses.slice(-5); const index = new Map(nodes.map((node, i) => [node.id, {x: 86 + i * 137, y: 94, node, ordinal: state.hypotheses.length - nodes.length + i + 1}]));
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
    button.addEventListener('click', () => void resolveSourceLocator(locator, shelf, api.provider).then(result => {
      if (result.status !== 'exact') throw new Error('This saved source is stale or unavailable. It was not retargeted.');
      button.textContent = label;
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
    undo.disabled = !summary.can_undo; redo.disabled = !summary.can_redo;
    content.replaceChildren();
    const form = renderForm(); content.append(form);
    const graphSection = el('section', 'pr-workspace-panel'); graphSection.append(el('h3', '', t().graph), renderGraph(state)); content.append(graphSection);
    const hypotheses = el('section', 'pr-workspace-panel'); hypotheses.append(el('h3', '', t().heading));
    if (!state.hypotheses.length) hypotheses.append(el('p', 'pr-workspace-muted', t().empty));
    for (const [hypothesisIndex, hypothesis] of state.hypotheses.entries()) {
      const card = el('article', 'pr-workspace-card'); card.dataset.hypothesisId = hypothesis.id;
      card.append(el('h4', '', `${hypothesisIndex + 1}. ${hypothesis.title}`), el('p', '', hypothesis.body), el('small', 'pr-workspace-muted', hypothesis.posture?.canon === false ? 'personal · unreviewed · not canon' : 'personal'));
      hypotheses.append(card);
    }
    content.append(hypotheses);
    const proposals = el('section', 'pr-workspace-panel'); proposals.append(el('h3', '', t().proposals));
    if (!state.proposals.length) proposals.append(el('p', 'pr-workspace-muted', t().noProposal));
    for (const proposal of state.proposals) {
      const card = el('article', 'pr-workspace-card pr-workspace-proposal');
      card.append(el('h4', '', proposal.kind), el('p', '', proposal.statement), el('small', 'pr-workspace-muted', t().review));
      const refs = [...new Set([...proposal.sourceRefs, ...proposal.evidenceRefs])];
      const links = el('div', 'pr-workspace-reference-list'); refs.forEach((ref, index) => links.append(referenceButton(ref, index)));
      if (!refs.length) links.append(el('span', 'pr-workspace-muted', 'No source references.'));
      card.append(el('h5', '', t().source), links); proposals.append(card);
    }
    content.append(proposals);
    const history = el('section', 'pr-workspace-panel'); history.append(el('h3', '', t().history));
    for (const entry of state.journal.slice(-12).reverse()) {
      const target = entry.targetId ? state.hypotheses.find(item => item.id === entry.targetId)?.title || entry.targetId : '';
      history.append(el('p', 'pr-workspace-history-row', `${entry.sequence}. ${entry.action}${target ? ` · ${target}` : ''}`));
    }
    content.append(history);
  }

  const exportPacket = async ({draft = false} = {}) => {
    await api.setQuestion(api.getQuestion());
    const packet = {schema: PUBLIC_WORKSPACE_SCHEMA, version: PUBLIC_WORKSPACE_VERSION, exportedAt: new Date().toISOString(),
      notebook: await notebook.exportPacket(), workspace: JSON.parse(workspace.exportPacket()), shelf: await shelf.exportPacket()};
    const serialized = JSON.stringify(packet, null, 2);
    if (new TextEncoder().encode(serialized).byteLength > MAX_EXPORT_BYTES) throw new TypeError('The complete research export exceeds 12 MB.');
    const link = el('a'); const url = URL.createObjectURL(new Blob([serialized], {type: 'application/json'}));
    link.href = url; link.download = `tos-public-research${draft ? '-unsaved-draft' : ''}-${new Date().toISOString().slice(0, 10)}.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    message(draft ? 'Unsaved workspace draft and its source shelf were exported.' : 'Complete notebook, workspace and shelf export downloaded.');
  };

  const packetCounts = packet => ({notes: packet.notebook.notes.length, readings: packet.notebook.readings.length, preferences: packet.notebook.preferences.length,
    hypotheses: packet.workspace.hypotheses.length, proposals: packet.workspace.proposals.length,
    shelfRecords: packet.shelf.records.length, collections: packet.shelf.collections.length});
  const currentPackets = async () => ({notebook: await notebook.exportPacket(), workspace: JSON.parse(workspace.exportPacket()), shelf: await shelf.exportPacket()});
  const assessImport = async input => {
    const incoming = await validateAggregate(input); const current = await currentPackets();
    const currentStorage = globalThis.localStorage.getItem(WORKSPACE_KEY);
    const workspaceMatchesStorage = currentStorage === null
      ? workspaceEmpty(current.workspace)
      : currentStorage === workspace.exportPacket();
    const conflicts = {
      workspace: ownerConflicts(current.workspace, incoming.workspace, 'workspace'),
      notebook: ownerConflicts(current.notebook, incoming.notebook, 'notebook'),
      shelf: ownerConflicts(current.shelf, incoming.shelf, 'shelf'),
    };
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
    const expected = {notebookRevision: current.notebook.revision, shelfGeneration: current.shelf.generation, workspaceStorage: currentStorage};
    return {incoming, current, conflicts, expected};
  };
  const renderImportPreview = assessment => {
    preview.replaceChildren(); pendingImport = assessment; importRevision = assessment.expected;
    const summary = packet => { const count = packetCounts(packet); return `notes ${count.notes}, readings ${count.readings}, hypotheses ${count.hypotheses}, proposals ${count.proposals}, shelf addresses ${count.shelfRecords}, collections ${count.collections}`; };
    preview.append(el('p', '', `Incoming: ${summary(assessment.incoming)}. Current: ${summary(assessment.current)}.`));
    const list = el('ul', 'pr-workspace-conflicts');
    let conflictCount = 0, readyOwners = 0;
    for (const owner of ['workspace', 'notebook', 'shelf']) {
      const items = assessment.conflicts[owner];
      if (!items.length) { readyOwners++; list.append(el('li', 'pr-workspace-owner-ok', `${owner}: ready for additive import.`)); }
      for (const conflict of items) { conflictCount++; list.append(el('li', 'pr-workspace-stale', `${owner}: ${conflict}`)); }
    }
    preview.append(list);
    const apply = btn(t().apply, 'pr-workspace-button pr-workspace-primary'); apply.disabled = readyOwners === 0;
    apply.addEventListener('click', () => void applyImport().catch(error => message(error?.message ?? 'Import failed.')));
    preview.append(apply);
    message(conflictCount ? 'Conflicting owners will be skipped; conflict-free owners can still import atomically.' : 'All three owner packets are valid. Each import will commit atomically through that owner.');
  };
  const applyImport = async () => {
    if (!pendingImport || !importRevision) return;
    const fresh = await assessImport(pendingImport.incoming);
    if (fresh.expected.workspaceStorage !== importRevision.workspaceStorage || fresh.expected.notebookRevision !== importRevision.notebookRevision || fresh.expected.shelfGeneration !== importRevision.shelfGeneration) {
      renderImportPreview(fresh); message('Research changed after preview. Review the refreshed owner conflicts before importing.'); return;
    }
    const outcomes = [];
    if (fresh.conflicts.shelf.length) outcomes.push(`shelf: skipped (${fresh.conflicts.shelf.join('; ')})`);
    else {
      const shelfResult = await shelf.import(fresh.incoming.shelf, {expectedGeneration: fresh.expected.shelfGeneration});
      outcomes.push(`shelf: added ${shelfResult.counts.records} addresses and ${shelfResult.counts.collections} collections`);
    }
    if (fresh.conflicts.notebook.length) outcomes.push(`notebook: skipped (${fresh.conflicts.notebook.join('; ')})`);
    else {
      const notebookResult = await notebook.importPacket(fresh.incoming.notebook, {expectedRevision: fresh.expected.notebookRevision});
      await api.reader?.refreshNotebook?.();
      outcomes.push(`notebook: merged exact notes and reading records (revision ${notebookResult.revision})`);
    }
    if (fresh.conflicts.workspace.length) outcomes.push(`workspace: skipped (${fresh.conflicts.workspace.join('; ')})`);
    else if (equal(fresh.current.workspace, fresh.incoming.workspace) || workspaceEmpty(fresh.incoming.workspace)) outcomes.push('workspace: already equal or empty; kept current state');
    else { workspace.importPacket(JSON.stringify(fresh.incoming.workspace)); outcomes.push('workspace: imported atomically by Rust'); }
    pendingImport = null; preview.replaceChildren(); render(); message(outcomes.join(' · '));
    const questionValue = fresh.incoming.notebook.preferences.find(item => item.key === 'tos.public-research.question.v1')?.value;
    if (typeof questionValue === 'string' && !fresh.conflicts.notebook.length) await api.setQuestion(questionValue);
  };

  addSource.addEventListener('click', () => {
    const added = sourcePicker?.addSelectedSource?.();
    const english = String(locale()).startsWith('en');
    message(added ? (english ? 'Exact passage added to your grounds.' : 'Точный фрагмент добавлен к основаниям.') : (english ? 'Select a new passage in the reader before adding it.' : 'Чтобы добавить новое основание, выберите другой фрагмент в читателе.'));
  });
  undo.addEventListener('click', () => { if (workspace.undo()) render(); });
  redo.addEventListener('click', () => { if (workspace.redo()) render(); });
  exportButton.addEventListener('click', () => void exportPacket().catch(error => message(error?.message ?? 'Export failed.')));
  draftExportButton.addEventListener('click', () => void exportPacket({draft: true}).catch(error => message(error?.message ?? 'Draft export failed.')));
  importButton.addEventListener('click', () => { importFile.value = ''; importFile.click(); });
  importFile.addEventListener('change', async () => {
    const file = importFile.files?.[0]; importFile.value = ''; if (!file) return;
    if (file.size > MAX_EXPORT_BYTES) { message('The complete research file exceeds 12 MB.'); return; }
    try { const assessment = await assessImport(JSON.parse(await file.text())); renderImportPreview(assessment); }
    catch (error) { pendingImport = null; preview.replaceChildren(); message(error?.message ?? 'The complete research packet is invalid.'); }
  });
  render();
  const unsubscribeWorkspace = workspace.subscribe(() => { render(); section.open=true; });
  let destroyed = false;
  return {workspace, shelf, connectAgentBridge: subscribeSourceList, render, destroy() {
    if (destroyed) return;
    destroyed = true; root.removeEventListener('input', questionInput); unsubSources?.(); unsubscribeWorkspace(); section.remove(); workspace.dispose(); void shelf.close();
  }};
}
