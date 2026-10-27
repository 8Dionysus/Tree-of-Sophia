import {referenceKey, validateReference} from '../corpus-reader/model.mjs';
import {ensureSourceRecord} from './source-address.mjs';
import './agent-bridge.css';

export const AGENT_TASK_SCHEMA = 'tos.public_research.agent_task.v1';
export const AGENT_RESPONSE_SCHEMA = 'tos.public_research.agent_response.v1';
const TASK_VERSION = 1;
const RESPONSE_VERSION = 1;
const TASK_LIMIT = 64_000;
const RESPONSE_LIMIT = 64_000;
const MAX_SOURCES = 8;
const MAX_PROPOSALS = 12;
const MAX_QUOTE = 2_000;
const TASKS_KEY = 'tos.public-research.agent-tasks.v1';
const TASK_KEY_PREFIX = 'tos.public-research.agent-task.v1:';
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const own = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
const only = (value, required, optional = []) => object(value)
  && required.every(key => own(value, key))
  && Object.keys(value).every(key => required.includes(key) || optional.includes(key));
const bounded = (value, max, label, {empty = false} = {}) => {
  if (typeof value !== 'string' || value.length > max || (!empty && !value.trim())) throw new TypeError(`${label} must be ${empty ? 'a' : 'a non-empty'} string of at most ${max} characters.`);
  return value.trim();
};
const boundedText = (value, max, label) => {
  if (typeof value !== 'string' || value.length > max) throw new TypeError(`${label} must be a string of at most ${max} characters.`);
  return value;
};
const id = prefix => `${prefix}-${globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`}`;
const copy = value => structuredClone(value);
const text = (tag, className, value) => { const node = document.createElement(tag); if (className) node.className = className; if (value !== undefined) node.textContent = value; return node; };
const button = (label, className = '') => { const node = text('button', className, label); node.type = 'button'; return node; };

function checkReference(value) {
  const reference = validateReference(value);
  if (reference.schemaVersion !== 'tos.corpus.reader.reference.v1') throw new TypeError('Only exact corpus text references can enter an agent task.');
  const key = referenceKey(reference);
  if (typeof key !== 'string' || key.length > 1_024) throw new TypeError('The exact source reference is too large.');
  return {reference, key};
}

function taskPacket(value) {
  if (!only(value, ['schema', 'version', 'taskId', 'createdAt', 'profile', 'question', 'workspaceRevision', 'sources', 'instructions', 'responseContract'])) throw new TypeError('The agent task packet has unexpected fields.');
  if (value.schema !== AGENT_TASK_SCHEMA || value.version !== TASK_VERSION) throw new TypeError('Unsupported agent task version.');
  bounded(value.taskId, 128, 'Task id');
  bounded(value.createdAt, 64, 'Created time');
  bounded(value.profile, 32, 'Profile');
  bounded(value.question, 4_000, 'Question');
  if (!Number.isSafeInteger(value.workspaceRevision) || value.workspaceRevision < 0) throw new TypeError('The workspace revision is invalid.');
  if (!Array.isArray(value.sources) || value.sources.length < 1 || value.sources.length > MAX_SOURCES) throw new TypeError(`Choose between 1 and ${MAX_SOURCES} exact sources.`);
  const seen = new Set();
  const sources = value.sources.map((source, index) => {
    if (!object(source) || ['id', 'reference', 'title', 'quote', 'language', 'versionLabel'].some(key => !Object.hasOwn(source, key))
      || Object.keys(source).some(key => !['id', 'reference', 'title', 'quote', 'context', 'contextTruncated', 'language', 'versionLabel'].includes(key))) throw new TypeError(`Source ${index + 1} has an invalid shape.`);
    const sourceId = bounded(source.id, 32, `Source ${index + 1} id`);
    if (seen.has(sourceId)) throw new TypeError('Source ids must be unique.');
    seen.add(sourceId);
    const checked = checkReference(source.reference);
    if (source.contextTruncated !== undefined && typeof source.contextTruncated !== 'boolean') throw new TypeError(`Source ${index + 1} contextTruncated must be a boolean.`);
    return {id: sourceId, reference: checked.reference, title: bounded(source.title, 512, 'Source title'),
      quote: boundedText(source.quote, MAX_QUOTE, 'Selected text'),
      context: boundedText(source.context ?? '', MAX_QUOTE * 2, 'Bounded context excerpt'), contextTruncated: source.contextTruncated === true,
      language: bounded(source.language, 64, 'Source language', {empty: true}),
      versionLabel: bounded(source.versionLabel, 512, 'Edition label', {empty: true})};
  });
  if (value.instructions !== 'Read the exact excerpts and references. Return only the response JSON contract below. Keep claims and objections separate, cite only the supplied source ids, and state uncertainty. Do not claim canon or review.') throw new TypeError('The task instructions are invalid.');
  if (!only(value.responseContract, ['schema', 'version', 'fields']) || value.responseContract.schema !== AGENT_RESPONSE_SCHEMA || value.responseContract.version !== RESPONSE_VERSION || !Array.isArray(value.responseContract.fields)) throw new TypeError('The response contract is invalid.');
  return {...value, sources};
}

function responseHypothesis(task, response) {
  let questionExcerpt = task.question.slice(0, 220);
  const lastCodeUnit = questionExcerpt.charCodeAt(questionExcerpt.length - 1);
  if (lastCodeUnit >= 0xD800 && lastCodeUnit <= 0xDBFF) questionExcerpt = questionExcerpt.slice(0, -1);
  const title = `Agent response: ${questionExcerpt}`.trim();
  const body = `${task.question}\n\nExternal agent response for task ${task.taskId}. ${response.uncertainty ? `Uncertainty: ${response.uncertainty}` : ''}`.trim();
  if (body.length > 4_000) throw new TypeError(`The complete question and uncertainty need ${body.length} characters; the workspace allows 4,000. Shorten them before applying this response.`);
  return {id: `agent-${task.taskId}`, title, body};
}

function generatedProposalIds(task, proposals) {
  const prefix = `agent-${task.taskId}-`;
  const ids = [];
  for (const proposal of proposals) {
    ids.push(`${prefix}${proposal.id}`);
    if (proposal.objection) ids.push(`${prefix}${proposal.id}-objection`);
  }
  if (new Set(ids).size !== ids.length) throw new TypeError('This response generates colliding proposal ids. Rename the proposal ids or remove the collision before applying.');
  return {hypothesisId: `agent-${task.taskId}`, proposalIds: ids};
}

export function responsePacket(value, task) {
  if (!only(value, ['schema', 'version', 'taskId', 'uncertainty', 'proposals'])) throw new TypeError('The agent response has unexpected fields.');
  if (value.schema !== AGENT_RESPONSE_SCHEMA || value.version !== RESPONSE_VERSION) throw new TypeError('Unsupported agent response version.');
  if (value.taskId !== task.taskId) throw new TypeError('This response belongs to a different task.');
  const uncertainty = bounded(value.uncertainty, 2_000, 'Uncertainty', {empty: true});
  if (!Array.isArray(value.proposals) || !value.proposals.length || value.proposals.length > MAX_PROPOSALS) throw new TypeError(`The response must contain 1–${MAX_PROPOSALS} proposals.`);
  const ids = new Set();
  const sourceIds = new Set(task.sources.map(source => source.id));
  const refs = list => {
    if (!Array.isArray(list) || list.length < 1 || list.length > MAX_SOURCES) throw new TypeError('Every claim and objection must cite at least one supplied source id.');
    const result = list.map(value => bounded(value, 32, 'Source id'));
    if (new Set(result).size !== result.length || result.some(value => !sourceIds.has(value))) throw new TypeError('A claim cites a source that was not in this task.');
    return result;
  };
  const proposals = value.proposals.map((proposal, index) => {
    if (!only(proposal, ['id', 'kind', 'statement', 'sourceIds', 'evidenceIds'], ['objection'])) throw new TypeError(`Proposal ${index + 1} has an invalid shape.`);
    const proposalId = bounded(proposal.id, 80, `Proposal ${index + 1} id`);
    if (ids.has(proposalId)) throw new TypeError('Proposal ids must be unique.');
    ids.add(proposalId);
    if (!['interpretation', 'relation'].includes(proposal.kind)) throw new TypeError(`Proposal ${index + 1} kind must be interpretation or relation.`);
    const statement = bounded(proposal.statement, 2_000, 'Proposal statement');
    const sourceRefs = refs(proposal.sourceIds);
    const evidenceRefs = refs(proposal.evidenceIds);
    let objection;
    if (proposal.objection !== undefined) {
      if (!only(proposal.objection, ['statement', 'sourceIds'])) throw new TypeError(`Proposal ${index + 1} objection has an invalid shape.`);
      objection = {statement: bounded(proposal.objection.statement, 2_000, 'Objection'), sourceRefs: refs(proposal.objection.sourceIds)};
    }
    if (proposal.kind === 'relation' && new Set([...sourceRefs, ...evidenceRefs]).size < 2) throw new TypeError('A relation proposal must cite at least two distinct exact sources.');
    return {id: proposalId, kind: proposal.kind, statement, sourceIds: sourceRefs, evidenceIds: evidenceRefs, objection};
  });
  const response = {schema: AGENT_RESPONSE_SCHEMA, version: RESPONSE_VERSION, taskId: task.taskId, uncertainty, proposals};
  responseHypothesis(task, response);
  generatedProposalIds(task, proposals);
  return response;
}

export function planAgentApplication(task, response, selectedProposals, state) {
  const hypothesis = responseHypothesis(task, response);
  const {hypothesisId, proposalIds} = generatedProposalIds(task, selectedProposals);
  const existingProposalIds = new Set(state.proposals.map(item => item.id));
  const existingHypothesis = state.hypotheses.find(item => item.id === hypothesisId);
  const presentCount = proposalIds.filter(id => existingProposalIds.has(id)).length;
  if (presentCount === proposalIds.length && existingHypothesis?.title === hypothesis.title && existingHypothesis?.body === hypothesis.body) {
    return {already: true, hypothesis, hypothesisId, proposalIds};
  }
  if (presentCount || existingHypothesis) throw new Error('This agent task already occupies one or more workspace ids. Review the existing response before applying again.');
  return {already: false, hypothesis, hypothesisId, proposalIds};
}

async function fingerprint(sources) {
  const input = new TextEncoder().encode(JSON.stringify(sources.map(source => ({id: source.id, reference: source.reference}))));
  const digest = await globalThis.crypto.subtle.digest('SHA-256', input);
  return `sha256:${[...new Uint8Array(digest)].map(value => value.toString(16).padStart(2, '0')).join('')}`;
}

export function mountPublicAgentBridge({root, api, notebook, workspace, shelf, locale = 'ru'} = {}) {
  if (!root || !api || !notebook || !workspace || !shelf) throw new TypeError('The public agent bridge requires its shell, reader API, notebook, shared workspace, and research shelf.');
  const english = () => String(typeof locale === 'function' ? locale() : locale).toLowerCase().startsWith('en');
  const words = () => english() ? {
    heading: 'Work with an external agent', intro: 'Choose exact passages, export a bounded task, then review the returned proposals before adding them to your personal workspace.',
    add: 'Add selected passage', remove: 'Remove', noSelection: 'Select text in the reader first.', sourceCount: 'Selected exact sources',
    export: 'Export task JSON', copy: 'Copy task JSON', copyFail: 'Clipboard is unavailable. Select and copy the task JSON below.',
    import: 'Paste agent response JSON', preview: 'Preview response', file: 'Load JSON file', apply: 'Apply selected proposals to my workspace',
    reopen: 'Open exact source', source: 'Source', objection: 'Objection', noTask: 'Export a task before importing a response.',
    stale: 'This exact source no longer resolves. It cannot be retargeted.', exact: 'Exact source verified.', applied: 'Selected proposals were added to your personal workspace as pending review.',
    appliedAlready: 'These proposals are already in the personal workspace.', invalid: 'Could not read this task or response.', proposal: 'Proposal', status: 'Agent task and response stay local until you export or copy them.',
  } : {
    heading: 'Исследование с внешним агентом', intro: 'Выберите точные фрагменты, экспортируйте ограниченное задание и проверьте возвращённые предложения перед добавлением в личную рабочую область.',
    add: 'Добавить выбранный фрагмент', remove: 'Убрать', noSelection: 'Сначала выделите текст в читателе.', sourceCount: 'Выбранные точные источники',
    export: 'Экспорт задания JSON', copy: 'Копировать задание JSON', copyFail: 'Буфер обмена недоступен. Выделите JSON задания и скопируйте его вручную.',
    import: 'Вставьте JSON ответа агента', preview: 'Проверить ответ', file: 'Загрузить JSON-файл', apply: 'Добавить выбранные предложения в мою рабочую область',
    reopen: 'Открыть точный источник', source: 'Источник', objection: 'Возражение', noTask: 'Сначала экспортируйте задание.',
    stale: 'Этот точный источник больше не разрешается. Автоматическая перепривязка запрещена.', exact: 'Точный источник проверен.', applied: 'Выбранные предложения добавлены в личную рабочую область со статусом ожидания review.',
    appliedAlready: 'Эти предложения уже есть в личной рабочей области.', invalid: 'Не удалось прочитать это задание или ответ.', proposal: 'Предложение', status: 'Задание и ответ агента остаются локальными, пока вы не экспортируете или не скопируете их.',
  };
  const t = words;
  const section = text('details', 'pr-agent-bridge');
  section.setAttribute('aria-labelledby', 'pr-agent-title');
  const heading = text('summary', 'pr-agent-title', t().heading); heading.id = 'pr-agent-title';
  const intro = text('p', 'pr-agent-intro', t().intro);
  const sourceHeading = text('h3', '', t().sourceCount);
  const sourceList = text('div', 'pr-agent-sources');
  const addSource = button(t().add, 'pr-agent-button');
  const taskButtons = text('div', 'pr-agent-buttons');
  const exportButton = button(t().export, 'pr-agent-button pr-agent-primary');
  const copyButton = button(t().copy, 'pr-agent-button');
  const taskText = document.createElement('textarea'); taskText.className = 'pr-agent-json'; taskText.readOnly = true; taskText.rows = 8; taskText.setAttribute('aria-label', 'Agent task JSON');
  const importLabel = text('label', 'pr-agent-label', t().import);
  const responseText = document.createElement('textarea'); responseText.className = 'pr-agent-json'; responseText.rows = 8; responseText.maxLength = RESPONSE_LIMIT; responseText.setAttribute('aria-label', 'Agent response JSON'); importLabel.append(responseText);
  const fileInput = document.createElement('input'); fileInput.type = 'file'; fileInput.accept = 'application/json,.json'; fileInput.className = 'pr-agent-file';
  const previewButton = button(t().preview, 'pr-agent-button');
  const applyButton = button(t().apply, 'pr-agent-button pr-agent-primary'); applyButton.disabled = true;
  const review = text('div', 'pr-agent-review'); review.setAttribute('aria-live', 'polite');
  const status = text('p', 'pr-agent-status', t().status); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const controls = text('div', 'pr-agent-buttons'); controls.append(addSource);
  taskButtons.append(exportButton, copyButton);
  section.append(heading, intro, sourceHeading, sourceList, controls, taskButtons, taskText, text('h3', '', t().import), importLabel, fileInput, previewButton, review, applyButton, status);
  root.append(section);

  let chosen = [];
  let sourceSequence = 0;
  let tasks = [];
  const sourceListeners = new Set();
  let activeTask = null;
  let activeResponse = null;
  let selectedProposalIds = new Set();
  const message = value => { status.textContent = value; };
  const notifySources = () => { const current = chosen.map(source => copy(source)); for (const listener of sourceListeners) listener(current); };
  const sourceTitle = source => [source.title, source.versionLabel].filter(Boolean).join(' · ') || source.id;
  const renderSources = () => {
    sourceList.replaceChildren();
    if (!chosen.length) sourceList.append(text('p', 'pr-agent-empty', english() ? 'No exact passage selected yet.' : 'Точные фрагменты ещё не выбраны.'));
    chosen.forEach((source, index) => {
      const row = text('article', 'pr-agent-source');
      const titleNode = text('strong', '', sourceTitle(source));
      const quoteNode = text('p', 'pr-agent-selection', source.quote || (english() ? 'Exact passage reference' : 'Точная ссылка на фрагмент'));
      const contextNode = text('p', 'pr-agent-context', source.context || '');
      if (source.contextTruncated) contextNode.append(text('span', 'pr-agent-truncated', english() ? ' · excerpt truncated' : ' · фрагмент сокращён'));
      const remove = button(t().remove, 'pr-agent-small'); remove.addEventListener('click', () => { chosen.splice(index, 1); renderSources(); notifySources(); });
      const reopen = button(t().reopen, 'pr-agent-small'); reopen.addEventListener('click', () => void reopenSource(source));
      row.append(titleNode, quoteNode, contextNode, text('code', '', source.id), remove, reopen); sourceList.append(row);
    });
    addSource.disabled = chosen.length >= MAX_SOURCES;
  };
  const reopenSource = async source => {
    try {
      const result = await api.provider.resolve({reference: source.reference});
      if (result.status !== 'exact') { message(t().stale); return false; }
      const reference = result.reference;
      await api.open({documentId: reference.target.workId, versionId: reference.versionId, unitId: reference.unitId,
        revision: reference.target.textLayerSha256, reference});
      return true;
    } catch { message(t().stale); return false; }
  };
  const addSelectedSource = () => {
    const selected = api.reader?.selectedSource?.();
    if (!selected || !selected.reference) { message(t().noSelection); return false; }
    try {
      const checked = checkReference(selected.reference);
      if (chosen.some(source => source.key === checked.key)) { message(english() ? 'That exact source is already selected.' : 'Этот точный источник уже выбран.'); return false; }
      if (chosen.length >= MAX_SOURCES) return false;
      const title = bounded(selected.title || selected.versionLabel || selected.reference.unitId, 512, 'Source title');
      const quote = boundedText(selected.quote || '', MAX_QUOTE, 'Selected text');
      const rawContext = Array.from(String(selected.text || ''));
      const context = rawContext.slice(0, MAX_QUOTE).join('');
      const source = {id: `s${++sourceSequence}`, key: checked.key, reference: checked.reference, title, quote, context,
        contextTruncated: rawContext.length > MAX_QUOTE,
        language: bounded(selected.language || '', 64, 'Language', {empty: true}), versionLabel: bounded(selected.versionLabel || '', 512, 'Edition label', {empty: true})};
      chosen.push(source); renderSources(); notifySources(); message(t().exact); return true;
    } catch (error) { message(error?.message ?? t().invalid); return false; }
  };

  const restoreTasks = async () => {
    try {
      const savedIds = await notebook.getPreference(TASKS_KEY);
      if (Array.isArray(savedIds) && savedIds.length <= 5_000) for (const taskId of savedIds) {
        if (typeof taskId !== 'string' || taskId.length > 128) continue;
        try {
          const value = await notebook.getPreference(`${TASK_KEY_PREFIX}${taskId}`);
          if (value) { const task = taskPacket(value); if (task.taskId === taskId && new TextEncoder().encode(JSON.stringify(task)).byteLength <= TASK_LIMIT) tasks.push(task); }
        } catch { /* Keep other saved tasks available when one old packet is invalid. */ }
      }
    } catch { /* task exchange remains available in this page session */ }
  };
  const saveTask = async task => {
    const serialized = JSON.stringify(task);
    if (new TextEncoder().encode(serialized).byteLength > TASK_LIMIT) throw new TypeError('The task exceeds the 64 KB export limit. Remove sources or shorten the question.');
    try {
      await notebook.setPreference(`${TASK_KEY_PREFIX}${task.taskId}`, task);
      const ids = await notebook.getPreference(TASKS_KEY);
      const nextIds = Array.isArray(ids) ? ids.filter(value => value !== task.taskId) : [];
      nextIds.push(task.taskId);
      await notebook.setPreference(TASKS_KEY, nextIds);
      tasks = [...tasks.filter(value => value.taskId !== task.taskId), task];
    } catch { throw new Error(english() ? 'The task could not be saved in the notebook.' : 'Не удалось сохранить задание в блокноте.'); }
  };
  const createTask = async () => {
    const question = bounded(await api.getQuestion(), 4_000, 'Question');
    if (!chosen.length) throw new TypeError(english() ? 'Add at least one exact selected source.' : 'Добавьте хотя бы один точный выбранный источник.');
    const sources = chosen.map(source => ({id: source.id, reference: source.reference, title: source.title,
      quote: source.quote, context: source.context, contextTruncated: source.contextTruncated,
      language: source.language, versionLabel: source.versionLabel}));
    const task = taskPacket({schema: AGENT_TASK_SCHEMA, version: TASK_VERSION, taskId: id('task'), createdAt: new Date().toISOString(),
      profile: api.profile, question, workspaceRevision: workspace?.summary().revision ?? 0, sources,
      instructions: 'Read the exact excerpts and references. Return only the response JSON contract below. Keep claims and objections separate, cite only the supplied source ids, and state uncertainty. Do not claim canon or review.',
      responseContract: {schema: AGENT_RESPONSE_SCHEMA, version: RESPONSE_VERSION,
        fields: ['taskId', 'uncertainty', 'proposals[{id,kind:interpretation|relation,statement,sourceIds,evidenceIds,objection?{statement,sourceIds}}]']}});
    await saveTask(task);
    activeTask = task; activeResponse = null; selectedProposalIds.clear(); applyButton.disabled = true; review.replaceChildren();
    taskText.value = JSON.stringify(task, null, 2);
    message(english() ? 'Task saved in this device notebook. Export or copy it to any external agent.' : 'Задание сохранено в блокноте этого устройства. Экспортируйте или скопируйте его для любого внешнего агента.');
    return task;
  };
  const readImport = async () => {
    const raw = responseText.value;
    if (new TextEncoder().encode(raw).byteLength > RESPONSE_LIMIT) throw new TypeError('The response exceeds 64 KB.');
    let value; try { value = JSON.parse(raw); } catch { throw new TypeError('The response is not valid JSON.'); }
    const matchingTask = tasks.find(task => task.taskId === value?.taskId) || activeTask;
    if (!matchingTask) throw new TypeError(t().noTask);
    const task = taskPacket(matchingTask);
    const response = responsePacket(value, task);
    activeTask = task; activeResponse = response;
    selectedProposalIds = new Set(response.proposals.map(proposal => proposal.id));
    await renderReview();
    return response;
  };
  const resolveResponse = async () => {
    if (!activeTask || !activeResponse) return new Map();
    const sourceIds = new Set();
    activeResponse.proposals.forEach(proposal => {
      proposal.sourceIds.concat(proposal.evidenceIds).forEach(sourceId => sourceIds.add(sourceId));
      proposal.objection?.sourceRefs.forEach(sourceId => sourceIds.add(sourceId));
    });
    const byId = new Map(activeTask.sources.map(source => [source.id, source]));
    const results = new Map();
    for (const sourceId of sourceIds) {
      const source = byId.get(sourceId);
      if (!source) { results.set(sourceId, {status: 'unavailable'}); continue; }
      try { results.set(sourceId, await api.provider.resolve({reference: source.reference})); }
      catch { results.set(sourceId, {status: 'unavailable'}); }
    }
    return results;
  };
  const renderReview = async () => {
    review.replaceChildren(); applyButton.disabled = true;
    if (!activeTask || !activeResponse) return;
    const results = await resolveResponse();
    const sources = new Map(activeTask.sources.map(source => [source.id, source]));
    let allExact = true;
    const cards = [];
    const citationNode = sourceId => {
      const source = sources.get(sourceId); const result = results.get(sourceId);
      if (!source) { allExact = false; return text('span', 'pr-agent-stale', `${sourceId}: ${t().stale}`); }
      const row = text('div', `pr-agent-citation ${result?.status === 'exact' ? 'is-exact' : 'is-stale'}`);
      if (result?.status !== 'exact') { allExact = false; row.append(text('span', 'pr-agent-stale', `${sourceTitle(source)} — ${t().stale}`)); }
      else {
        const open = button(sourceTitle(source), 'pr-agent-cite'); open.addEventListener('click', () => void reopenSource(source));
        row.append(open, text('small', '', t().exact));
      }
      return row;
    };
    for (const proposal of activeResponse.proposals) {
      const card = text('article', 'pr-agent-proposal');
      const label = document.createElement('label'); label.className = 'pr-agent-select';
      const check = document.createElement('input'); check.type = 'checkbox'; check.checked = selectedProposalIds.has(proposal.id); check.addEventListener('change', () => { check.checked ? selectedProposalIds.add(proposal.id) : selectedProposalIds.delete(proposal.id); applyButton.disabled = !allExact || !selectedProposalIds.size; });
      label.append(check, text('strong', '', `${t().proposal} · ${proposal.kind}`));
      card.append(label, text('p', '', proposal.statement));
      const citations = text('div', 'pr-agent-citations'); proposal.sourceIds.forEach(sourceId => citations.append(citationNode(sourceId)));
      card.append(text('h4', '', t().source), citations);
      const evidence = text('div', 'pr-agent-citations'); proposal.evidenceIds.forEach(sourceId => evidence.append(citationNode(sourceId)));
      card.append(text('h4', '', english() ? 'Evidence' : 'Основание'), evidence);
      if (proposal.objection) {
        card.append(text('h4', 'pr-agent-objection-heading', t().objection), text('p', 'pr-agent-objection', proposal.objection.statement));
        const objections = text('div', 'pr-agent-citations'); proposal.objection.sourceRefs.forEach(sourceId => objections.append(citationNode(sourceId)));
        card.append(objections);
      }
      cards.push(card);
    }
    if (activeResponse.uncertainty) review.append(text('p', 'pr-agent-uncertainty', `${english() ? 'Uncertainty' : 'Неопределённость'}: ${activeResponse.uncertainty}`));
    cards.forEach(card => review.append(card));
    if (allExact) {
      applyButton.disabled = selectedProposalIds.size === 0;
      message(t().exact);
    } else {
      applyButton.disabled = true; message(t().stale);
    }
  };
  const applyResponse = async () => {
    if (!workspace || !activeTask || !activeResponse || !selectedProposalIds.size) throw new Error(english() ? 'The personal workspace is unavailable or no proposal is selected.' : 'Личная рабочая область недоступна или предложения не выбраны.');
    const results = await resolveResponse();
    const referenced = new Set();
    const selected = activeResponse.proposals.filter(proposal => selectedProposalIds.has(proposal.id));
    selected.forEach(proposal => {
      proposal.sourceIds.concat(proposal.evidenceIds).forEach(sourceId => referenced.add(sourceId));
      proposal.objection?.sourceRefs.forEach(sourceId => referenced.add(sourceId));
    });
    if ([...referenced].some(sourceId => results.get(sourceId)?.status !== 'exact')) { await renderReview(); throw new Error(t().stale); }
    const sourceMap = new Map(activeTask.sources.map(source => [source.id, source]));
    const currentQuestion = bounded(await api.getQuestion(), 4_000, 'Question');
    if (currentQuestion !== activeTask.question) throw new Error(english() ? 'The question changed after task export. Export a new task before applying this response.' : 'После экспорта задания вопрос изменился. Перед применением ответа экспортируйте новое задание.');
    const work = workspace;
    let plan = planAgentApplication(activeTask, activeResponse, selected, work.getState());
    if (plan.already) { message(t().appliedAlready); return; }
    if (activeTask.workspaceRevision !== work.summary().revision) throw new Error(english()
      ? 'The workspace changed after this task was exported. Keep the response for review, then export a new task before applying it.'
      : 'После экспорта задания рабочая область изменилась. Ответ сохранён для просмотра; перед применением экспортируйте новое задание.');
    const dataFingerprint = await fingerprint(activeTask.sources);
    const sourceLocators = new Map();
    for (const sourceId of referenced) {
      const source = sourceMap.get(sourceId);
      sourceLocators.set(sourceId, await ensureSourceRecord(shelf, source.reference, source.title));
    }

    plan = planAgentApplication(activeTask, activeResponse, selected, work.getState());
    if (plan.already) { message(t().appliedAlready); return; }
    if (activeTask.workspaceRevision !== work.summary().revision) throw new Error(english()
      ? 'The workspace changed after this task was exported. Keep the response for review, then export a new task before applying it.'
      : 'После экспорта задания рабочая область изменилась. Ответ сохранён для просмотра; перед применением экспортируйте новое задание.');
    work.addHypothesis(plan.hypothesis);
    const stage = (proposalId, kind, statement, sourceIds, evidenceIds, targetIds) => {
      const proposalKey = `agent-${activeTask.taskId}-${proposalId}`;
      const sourceRefs = [...new Set(sourceIds.map(sourceId => sourceLocators.get(sourceId)))];
      const evidenceRefs = [...new Set(evidenceIds.map(sourceId => sourceLocators.get(sourceId)))];
      const target = targetIds.map(sourceId => sourceMap.get(sourceId).reference.unitId);
      const base = {id: proposalKey, kind, parentHypothesisId: plan.hypothesisId, statement, sourceRefs, evidenceRefs,
        confidencePosture: {value: 'unknown', meaning: 'maker_declared_uncertainty_not_truth_probability'},
        actorOrigin: 'agent', basePageRevision: 0, baseWorkspaceRevision: work.summary().revision, dataFingerprint};
      if (kind === 'relation') { base.fromId = target[0]; base.toId = target[1]; }
      else base.targetId = target[0];
      work.stageProposal(base);
    };
    selected.forEach(proposal => {
      const citationIds = [...new Set([...proposal.sourceIds, ...proposal.evidenceIds])];
      stage(proposal.id, proposal.kind, proposal.statement, proposal.sourceIds, proposal.evidenceIds,
        proposal.kind === 'relation' ? citationIds.slice(0, 2) : [proposal.sourceIds[0]]);
      if (proposal.objection) stage(`${proposal.id}-objection`, 'interpretation', `Objection: ${proposal.objection.statement}`,
        proposal.objection.sourceRefs, proposal.objection.sourceRefs, [proposal.objection.sourceRefs[0]]);
    });
    await renderReview();
    message(t().applied);
  };

  addSource.addEventListener('click', addSelectedSource);
  exportButton.addEventListener('click', async () => {
    try {
      const task = await createTask();
      const blob = new Blob([JSON.stringify(task, null, 2)], {type: 'application/json'});
      const url = URL.createObjectURL(blob); const anchor = document.createElement('a');
      anchor.href = url; anchor.download = `tos-agent-task-${task.taskId}.json`; anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 0);
    } catch (error) { message(error?.message ?? t().invalid); }
  });
  copyButton.addEventListener('click', async () => {
    try {
      const task = await createTask(); const payload = JSON.stringify(task, null, 2);
      taskText.value = payload;
      if (!navigator.clipboard?.writeText) throw new Error(t().copyFail);
      await navigator.clipboard.writeText(payload); message(english() ? 'Task JSON copied.' : 'JSON задания скопирован.');
    } catch (error) { message(error?.message ?? t().copyFail); }
  });
  previewButton.addEventListener('click', () => void readImport().catch(error => { activeResponse = null; applyButton.disabled = true; review.replaceChildren(text('p', 'pr-agent-stale', error?.message ?? t().invalid)); message(error?.message ?? t().invalid); }));
  applyButton.addEventListener('click', () => void applyResponse().catch(error => message(error?.message ?? t().invalid)));
  fileInput.addEventListener('change', async () => {
    const file = fileInput.files?.[0]; if (!file) return;
    if (file.size > RESPONSE_LIMIT) { message('The response exceeds 64 KB.'); fileInput.value = ''; return; }
    responseText.value = await file.text(); fileInput.value = '';
  });
  void restoreTasks();
  renderSources();
  let destroyed = false;
  const destroy = () => { if (destroyed) return; destroyed = true; window.removeEventListener('pagehide', pageHide); sourceListeners.clear(); section.remove(); };
  const pageHide = event => { if (!event.persisted) destroy(); };
  window.addEventListener('pagehide', pageHide);
  return {destroy, addSelectedSource, exportTask: createTask,
    selectedSources: () => chosen.map(source => copy(source)),
    subscribeSources(listener) { if (typeof listener !== 'function') throw new TypeError('A source listener is required.'); sourceListeners.add(listener); listener(chosen.map(source => copy(source))); return () => sourceListeners.delete(listener); }};
}

