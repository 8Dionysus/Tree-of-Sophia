import {kindLabel, displayPublicError} from './presentation-i18n.mjs';
import {referenceKey, validateReference} from '../corpus-reader/model.mjs';
import {editorialLabel} from '../corpus-reader/editorial.mjs';
import {describeSourceLocator, ensureSourceRecord} from './source-address.mjs';
import {WORKSPACE_KEY} from './workspace-view.mjs';
import './agent-bridge.css';

export const AGENT_TASK_SCHEMA = 'tos.public_research.agent_task.v1';
export const AGENT_TASK_SCHEMA_V2 = 'tos.public_research.agent_task.v2';
export const AGENT_RESPONSE_SCHEMA = 'tos.public_research.agent_response.v1';
const TASK_VERSION = 2;
const RESPONSE_VERSION = 1;
const TASK_LIMIT = 64_000;
const RESPONSE_LIMIT = 64_000;
const MAX_SOURCES = 8;
const MAX_PROPOSALS = 12;
const MAX_QUOTE = 2_000;
const TASK_INSTRUCTIONS = 'Read the exact excerpts and references. Return only the response JSON contract below. Keep claims and objections separate, cite only the supplied source ids, and state uncertainty. Do not claim canon or review.';
const TASK_INSTRUCTIONS_V2 = `${TASK_INSTRUCTIONS} Context and editorial annotation ranges use Unicode code-point offsets in the referenced unit; annotations are not a complete apparatus.`;
const TASKS_KEY = 'tos.public-research.agent-tasks.v1';
const TASK_KEY_PREFIX = 'tos.public-research.agent-task.v1:';
const RESPONSE_KEY_PREFIX = 'tos.public-research.agent-response.v1:';
const BASKET_KEY = 'tos.public-research.source-basket.v1';
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

export function taskPacket(value) {
  if (!only(value, ['schema', 'version', 'taskId', 'createdAt', 'profile', 'question', 'workspaceRevision', 'sources', 'instructions', 'responseContract'])) throw new TypeError('The agent task packet has unexpected fields.');
  const v2 = value.schema === AGENT_TASK_SCHEMA_V2 && value.version === 2;
  if (!v2 && !(value.schema === AGENT_TASK_SCHEMA && value.version === 1)) throw new TypeError('Unsupported agent task version.');
  bounded(value.taskId, 128, 'Task id');
  bounded(value.createdAt, 64, 'Created time');
  bounded(value.profile, 32, 'Profile');
  bounded(value.question, 4_000, 'Question');
  if (!Number.isSafeInteger(value.workspaceRevision) || value.workspaceRevision < 0) throw new TypeError('The workspace revision is invalid.');
  if (!Array.isArray(value.sources) || value.sources.length < 1 || value.sources.length > MAX_SOURCES) throw new TypeError(`Choose between 1 and ${MAX_SOURCES} exact sources.`);
  const seen = new Set();
  const sources = value.sources.map((source, index) => {
    const required = ['id', 'reference', 'title', 'quote', 'language', 'versionLabel'];
    const optional = ['context', 'contextTruncated', ...(v2 ? ['contextRange', 'editorialAnnotations', 'sourceLayerKind'] : [])];
    if (!only(source, required, optional) || (v2 && (!own(source, 'contextRange') || !own(source, 'editorialAnnotations')))) throw new TypeError(`Source ${index + 1} has an invalid shape.`);
    const sourceId = bounded(source.id, 32, `Source ${index + 1} id`);
    if (seen.has(sourceId)) throw new TypeError('Source ids must be unique.');
    seen.add(sourceId);
    const checked = checkReference(source.reference);
    if (source.contextTruncated !== undefined && typeof source.contextTruncated !== 'boolean') throw new TypeError(`Source ${index + 1} contextTruncated must be a boolean.`);
    const result = {id: sourceId, reference: checked.reference, title: bounded(source.title, 512, 'Source title'),
      quote: boundedText(source.quote, MAX_QUOTE, 'Selected text'),
      context: boundedText(source.context ?? '', MAX_QUOTE * 2, 'Bounded context excerpt'), contextTruncated: source.contextTruncated === true,
      language: bounded(source.language, 64, 'Source language', {empty: true}),
      versionLabel: bounded(source.versionLabel, 512, 'Edition label', {empty: true})};
    if (v2) {
      const range = source.contextRange, points = Array.from(result.context), selector = result.reference.selector;
      if (!only(range, ['start','end','positionUnit','interval']) || !Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.end)
        || range.start < 0 || range.end < range.start || range.end - range.start !== points.length
        || range.positionUnit !== 'unicode_code_point' || range.interval !== 'half_open') throw new TypeError('The source context range is invalid.');
      if (selector.end > selector.start && (selector.start < range.start || selector.end > range.end
        || points.slice(selector.start-range.start, selector.end-range.start).join('') !== result.quote)) throw new TypeError('The selected quote does not match its bounded context.');
      if (!Array.isArray(source.editorialAnnotations) || source.editorialAnnotations.length > 512) throw new TypeError('The editorial annotation list is invalid.');
      const kinds = new Set(['note','addition','deletion','gap','notes','adjustments','publication','fragments','laterWitness']);
      result.contextRange = {start:range.start,end:range.end,positionUnit:range.positionUnit,interval:range.interval};
      result.editorialAnnotations = source.editorialAnnotations.map(item => {
        if (!only(item, ['kind','start','end']) || !kinds.has(item.kind) || !Number.isSafeInteger(item.start) || !Number.isSafeInteger(item.end)
          || item.start < range.start || item.end > range.end || item.end <= item.start) throw new TypeError('An editorial annotation is outside the context.');
        return {kind:item.kind,start:item.start,end:item.end};
      });
      if (own(source, 'sourceLayerKind')) {
        if (source.sourceLayerKind !== null && !kinds.has(source.sourceLayerKind)) throw new TypeError('The source layer role is invalid.');
        result.sourceLayerKind = source.sourceLayerKind;
      }
    }
    return result;
  });
  if (value.instructions !== (v2 ? TASK_INSTRUCTIONS_V2 : TASK_INSTRUCTIONS)) throw new TypeError('The task instructions are invalid.');
  if (!only(value.responseContract, ['schema', 'version', 'fields']) || value.responseContract.schema !== AGENT_RESPONSE_SCHEMA || value.responseContract.version !== RESPONSE_VERSION || !Array.isArray(value.responseContract.fields)) throw new TypeError('The response contract is invalid.');
  return {...value, sources};
}

/** Validate one source with exactly the same checks as an exported v2 task. */
function checkedBasketSource(source) {
  return taskPacket({schema:AGENT_TASK_SCHEMA_V2,version:2,taskId:'source-check',createdAt:'source-check',profile:'source-check',
    question:'source-check',workspaceRevision:0,sources:[source],instructions:TASK_INSTRUCTIONS_V2,
    responseContract:{schema:AGENT_RESPONSE_SCHEMA,version:RESPONSE_VERSION,fields:[]}}).sources[0];
}

export function parseSavedSourceBasket(raw) {
  if (typeof raw !== 'string' || new TextEncoder().encode(raw).byteLength > 512_000) throw new TypeError('Oversized source selection');
  const saved = JSON.parse(raw);
  if (!Array.isArray(saved) || saved.length > MAX_SOURCES) throw new TypeError('Invalid source selection');
  const seenIds = new Set(), seenKeys = new Set(); let sequence = 0;
  const sources = saved.map(item => {
    if (!object(item)) throw new TypeError('Invalid source selection');
    const {key:ignored, ...value} = item, source = checkedBasketSource(value), key = referenceKey(source.reference);
    if (seenIds.has(source.id) || seenKeys.has(key)) throw new TypeError('Duplicate source selection');
    seenIds.add(source.id); seenKeys.add(key);
    const number = /^s([1-9]\d{0,14})$/.exec(source.id);
    if (!number || !Number.isSafeInteger(Number(number[1]))) throw new TypeError('Invalid selection id');
    sequence = Math.max(sequence, Number(number[1]));
    return {...source,key};
  });
  return {sources,sequence};
}

export function mergeSourceBasket(current, incoming, initialSequence) {
  if (!Array.isArray(incoming) || incoming.length > MAX_SOURCES) throw new TypeError('The source selection is invalid.');
  const candidate = current.map(source => copy(source)), mapping = new Map(), seenIds = new Set(); let sequence = initialSequence;
  for (const raw of incoming) {
    if (!object(raw)) throw new TypeError('A source selection is invalid.');
    const {key:ignored, ...value} = raw, source = checkedBasketSource(value), key = referenceKey(source.reference);
    if (seenIds.has(source.id)) throw new TypeError('A source selection has duplicate ids.');
    seenIds.add(source.id);
    const existing = candidate.find(item => item.key === key);
    if (existing) {
      const {id:existingId,key:existingKey,...existingData} = existing, {id:incomingId,...incomingData} = source;
      if (JSON.stringify(existingData) !== JSON.stringify(incomingData)) throw new Error('A selected source has different context; both selections are preserved.');
      mapping.set(source.id, existingId); continue;
    }
    if (candidate.length >= MAX_SOURCES) throw new TypeError('The source selection exceeds eight passages.');
    const id = `s${++sequence}`; candidate.push({...source,id,key}); mapping.set(source.id, id);
  }
  return {sources:candidate,sequence,mapping};
}

function responseHypothesis(task, response) {
  let questionExcerpt = task.question.slice(0, 220);
  const lastCodeUnit = questionExcerpt.charCodeAt(questionExcerpt.length - 1);
  if (lastCodeUnit >= 0xD800 && lastCodeUnit <= 0xDBFF) questionExcerpt = questionExcerpt.slice(0, -1);
  const title = `Agent response: ${questionExcerpt}`.trim();
  const body = `${task.question}\n\nExternal agent response for task ${task.taskId}. ${response.uncertainty ? `Uncertainty: ${response.uncertainty}` : ''}`.trim();
  if (body.length > 4_000) throw new TypeError(`The complete question and uncertainty need ${body.length} characters; the workspace allows 4,000. Shorten them before applying this response.`);
  return {id: `agent-${task.taskId}`, kind:'note', title, body};
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

/** Select an objection independently without mutating the received response. */
export function selectAgentProposals(response, proposalIds, objectionIds) {
  return (response?.proposals || []).filter(proposal => proposalIds.has(proposal.id))
    .map(proposal => proposal.objection && !objectionIds.has(proposal.id) ? {...proposal, objection:undefined} : proposal);
}

/** Stage the selected response while preserving an objection's parent proposal. */
export function stageAgentProposals({task, selected, workspace, sourceLocators, dataFingerprint, plan}) {
  const sourceMap = new Map(task.sources.map(source => [source.id, source]));
  for (const proposal of selected) {
    for (const sourceId of [...proposal.sourceIds, ...proposal.evidenceIds, ...(proposal.objection?.sourceRefs || [])]) {
      if (!sourceMap.has(sourceId) || typeof sourceLocators.get(sourceId) !== 'string') throw new TypeError('A selected agent source is unavailable.');
    }
  }
  workspace.addHypothesis(plan.hypothesis);
  if (workspace.persistenceError?.()) throw new Error('The workspace changed in another tab. Export your unsaved workspace draft before reloading.');
  const stage = (proposalId, kind, statement, sourceIds, evidenceIds, targetIds, targetOverride = null) => {
    const proposalKey = `agent-${task.taskId}-${proposalId}`;
    const sourceRefs = [...new Set(sourceIds.map(sourceId => sourceLocators.get(sourceId)))];
    const evidenceRefs = [...new Set(evidenceIds.map(sourceId => sourceLocators.get(sourceId)))];
    const target = targetIds.map(sourceId => sourceMap.get(sourceId).reference.unitId);
    const base = {id: proposalKey, kind, parentHypothesisId: plan.hypothesisId, statement, sourceRefs, evidenceRefs,
      confidencePosture: {value: 'unknown', meaning: 'maker_declared_uncertainty_not_truth_probability'},
      actorOrigin: 'agent', basePageRevision: 0, baseWorkspaceRevision: workspace.summary().revision, dataFingerprint};
    if (kind === 'relation') { base.fromId = target[0]; base.toId = target[1]; }
    else base.targetId = targetOverride || target[0];
    workspace.stageProposal(base);
    if (workspace.persistenceError?.()) throw new Error('The workspace changed in another tab. Export your unsaved workspace draft before reloading.');
  };
  selected.forEach(proposal => {
    const citationIds = [...new Set([...proposal.sourceIds, ...proposal.evidenceIds])];
    stage(proposal.id, proposal.kind, proposal.statement, proposal.sourceIds, proposal.evidenceIds,
      proposal.kind === 'relation' ? citationIds.slice(0, 2) : [proposal.sourceIds[0]]);
    if (proposal.objection) stage(`${proposal.id}-objection`, 'interpretation', `Objection: ${proposal.objection.statement}`,
      proposal.objection.sourceRefs, proposal.objection.sourceRefs, [proposal.objection.sourceRefs[0]], `agent-${task.taskId}-${proposal.id}`);
  });
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
    stale: 'This exact source no longer resolves. It cannot be retargeted.', exact: 'Exact source address is available.', applied: 'Selected proposals were added to your personal workspace as pending review.',
    appliedAlready: 'These proposals are already in the personal workspace.', invalid: 'Could not read this task or response.', proposal: 'Proposal', status: 'Agent task and response stay local until you export or copy them.',
  } : {
    heading: 'Исследование с внешним агентом', intro: 'Выберите точные фрагменты, экспортируйте ограниченное задание и проверьте возвращённые предложения перед добавлением в личную рабочую область.',
    add: 'Добавить выбранный фрагмент', remove: 'Убрать', noSelection: 'Сначала выделите текст в читателе.', sourceCount: 'Выбранные точные источники',
    export: 'Экспорт задания JSON', copy: 'Копировать задание JSON', copyFail: 'Буфер обмена недоступен. Выделите JSON задания и скопируйте его вручную.',
    import: 'Вставьте JSON ответа агента', preview: 'Проверить ответ', file: 'Загрузить JSON-файл', apply: 'Добавить выбранные предложения в мою рабочую область',
    reopen: 'Открыть точный источник', source: 'Источник', objection: 'Возражение', noTask: 'Сначала экспортируйте задание.',
    stale: 'Этот точный источник больше не разрешается. Автоматическая перепривязка запрещена.', exact: 'Точный адрес источника доступен.', applied: 'Выбранные предложения добавлены в личную рабочую область со статусом ожидания проверки.',
    appliedAlready: 'Эти предложения уже есть в личной рабочей области.', invalid: 'Не удалось прочитать это задание или ответ.', proposal: 'Предложение', status: 'Задание и ответ агента остаются локальными, пока вы не экспортируете или не скопируете их.',
  };
  const t = words;
  const section = text('details', 'pr-agent-bridge');
  section.setAttribute('aria-labelledby', 'pr-agent-title');
  const heading = text('summary', 'pr-agent-title', t().heading); heading.id = 'pr-agent-title';
  const intro = text('p', 'pr-agent-intro', t().intro);
  const sourceHeading = text('h3', '', t().sourceCount);
  const basketHint = text('p', 'pr-agent-empty', english() ? 'This temporary selection stays in this tab across profile changes; it is not part of the complete research export until used in a saved thought or task.' : 'Временная подборка остаётся в этой вкладке при смене профиля; в полный экспорт она попадёт после сохранения мысли или задания.');
  const sourceList = text('div', 'pr-agent-sources');
  const addSource = button(t().add, 'pr-agent-button');
  const taskButtons = text('div', 'pr-agent-buttons');
  const exportButton = button(t().export, 'pr-agent-button pr-agent-primary');
  const copyButton = button(t().copy, 'pr-agent-button');
  const copySnapshotButton = button(english() ? 'Copy displayed saved task' : 'Копировать показанное задание', 'pr-agent-button'); copySnapshotButton.disabled = true;
  const taskText = document.createElement('textarea'); taskText.className = 'pr-agent-json'; taskText.readOnly = true; taskText.rows = 8; taskText.setAttribute('aria-label', 'Agent task JSON');
  const taskNotice = text('p', 'pr-agent-status'); taskNotice.hidden = true;
  const taskHistory = text('details', 'pr-agent-history');
  const taskHistoryTitle = text('summary', '', english() ? 'Saved tasks and responses' : 'Сохранённые задания и ответы');
  const taskHistoryList = text('div', 'pr-agent-saved-list'); taskHistory.append(taskHistoryTitle, taskHistoryList);
  const importLabel = text('label', 'pr-agent-label'); const importLabelText = text('span', '', t().import); importLabel.append(importLabelText);
  const responseText = document.createElement('textarea'); responseText.className = 'pr-agent-json'; responseText.rows = 8; responseText.maxLength = RESPONSE_LIMIT; responseText.setAttribute('aria-label', 'Agent response JSON'); importLabel.append(responseText);
  const fileInput = document.createElement('input'); fileInput.type = 'file'; fileInput.accept = 'application/json,.json'; fileInput.className = 'pr-agent-file';
  const previewButton = button(t().preview, 'pr-agent-button');
  const applyButton = button(t().apply, 'pr-agent-button pr-agent-primary'); applyButton.disabled = true;
  const review = text('div', 'pr-agent-review'); review.setAttribute('aria-live', 'polite');
  const responseStatus = text('p', 'pr-agent-status');
  const status = text('p', 'pr-agent-status', t().status); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  const controls = text('div', 'pr-agent-buttons'); controls.append(addSource);
  taskButtons.append(exportButton, copyButton, copySnapshotButton);
  const importHeading = text('h3', '', t().import);
  const responseNotice = text('p', 'pr-agent-status');
  section.append(heading, intro, sourceHeading, basketHint, sourceList, controls, status, taskButtons, taskText, taskNotice, taskHistory, importHeading, importLabel, fileInput, previewButton, responseNotice, review, applyButton, responseStatus);
  root.append(section);

  let chosen = [];
  let sourceSequence = 0;
  let basketSaved = true, basketWritable = true;
  let tasks = [];
  let savedResponses = [];
  const sourceListeners = new Set();
  let activeTask = null;
  let displayedTask = null;
  let activeResponse = null;
  let selectedProposalIds = new Set();
  let selectedObjectionIds = new Set();
  let applyingResponse = false;
  let creatingTask = false;
  const message = value => { status.textContent = value; };
  const updateBasketHint = () => {
    basketHint.textContent = !basketWritable
      ? (english() ? 'The previous selection could not be restored; its saved data was not overwritten.' : 'Не удалось восстановить прежнюю подборку; сохранённые данные не перезаписаны.')
      : basketSaved
      ? (english() ? 'This temporary selection stays in this tab across profile changes; it is not part of the complete research export until used in a saved thought or task.' : 'Временная подборка остаётся в этой вкладке при смене профиля; в полный экспорт она попадёт после сохранения мысли или задания.')
      : (english() ? 'This selection cannot be retained in this tab. Export the thought form or task before leaving.' : 'Подборку не удалось сохранить во вкладке. Перед уходом экспортируйте форму мысли или задание.');
  };
  const selectedProposals = () => selectAgentProposals(activeResponse, selectedProposalIds, selectedObjectionIds);
  const preflight = () => {
    if (!activeTask) return '';
    if (String(api.getQuestion() || '').trim() !== activeTask.question) return english()
      ? 'The question changed after this task was saved; review the response and export a new task before applying.'
      : 'После сохранения задания вопрос изменился; ответ можно просмотреть, но для применения нужно новое задание.';
    const saved = globalThis.localStorage?.getItem(WORKSPACE_KEY);
    if (workspace.persistenceError?.() || (saved !== null && saved !== undefined && saved !== workspace.exportPacket())) return english()
      ? 'The workspace changed after this task was saved; review the response and export a new task before applying.'
      : 'После сохранения задания исследование изменилось; ответ можно просмотреть, но для применения нужно новое задание.';
    if (activeTask.workspaceRevision !== workspace.summary().revision) {
      try { if (planAgentApplication(activeTask, activeResponse, selectedProposals(), workspace.getState()).already) return ''; } catch { /* defer collision detail to Apply */ }
      return english() ? 'The workspace changed after this task was saved; review the response and export a new task before applying.'
        : 'После сохранения задания исследование изменилось; ответ можно просмотреть, но для применения нужно новое задание.';
    }
    return '';
  };
  const renderTaskHistory = () => {
    taskHistoryTitle.textContent = english() ? 'Saved tasks and responses' : 'Сохранённые задания и ответы';
    taskHistoryList.replaceChildren();
    if (!tasks.length) taskHistoryList.append(text('p', 'pr-agent-empty', english() ? 'No saved tasks yet.' : 'Сохранённых заданий пока нет.'));
    for (const task of [...tasks].sort((a,b) => b.createdAt.localeCompare(a.createdAt))) {
      const row = text('div', 'pr-agent-saved-task');
      const chars = Array.from(task.question), excerpt = chars.slice(0,120).join('') + (chars.length > 120 ? '…' : '');
      const open = button(`${task.createdAt} · ${task.profile} · ${excerpt}`, 'pr-agent-small'); open.title = task.question;
      open.addEventListener('click', () => { displayedTask = task; activeTask = task; clearResponsePreview(); taskText.value = JSON.stringify(task, null, 2); copySnapshotButton.disabled = false; updateTaskNotice(); });
      row.append(open);
      for (const response of savedResponses.filter(item => item.taskId === task.taskId)) {
        const show = button(`${english() ? 'View saved response' : 'Показать сохранённый ответ'}${response.hash ? ` · ${response.hash.slice(0,8)}` : ''}`, 'pr-agent-small');
        show.addEventListener('click', () => { responseText.value = response.raw; clearResponsePreview(); responseText.focus(); }); row.append(show);
      }
      taskHistoryList.append(row);
    }
  };
  const persistBasket = () => {
    if (!basketWritable) { basketSaved = !chosen.length; updateBasketHint(); return; }
    try {
      const storage = globalThis.sessionStorage;
      if (!storage) throw new Error('Session storage unavailable');
      if (chosen.length) storage.setItem(BASKET_KEY, JSON.stringify(chosen));
      else storage.removeItem(BASKET_KEY);
      basketSaved = true;
    } catch { basketSaved = !chosen.length; }
    updateBasketHint();
  };
  const restoreBasket = () => {
    try {
      const storage = globalThis.sessionStorage;
      if (!storage) { basketSaved = false; updateBasketHint(); return; }
      const raw = storage.getItem(BASKET_KEY); if (raw === null) return;
      const restored = parseSavedSourceBasket(raw); chosen = restored.sources; sourceSequence = restored.sequence;
      basketSaved = true; updateBasketHint();
    } catch {
      chosen = []; basketSaved = false; basketWritable = false; updateBasketHint();
      message(english() ? 'The earlier source selection could not be restored; its saved data was not overwritten.' : 'Не удалось восстановить прежнюю подборку источников; сохранённые данные не перезаписаны.');
    }
  };
  const updateTaskNotice = () => {
    if (!displayedTask) { taskNotice.hidden = true; return; }
    const sameSources = JSON.stringify(displayedTask.sources.map(source => referenceKey(source.reference))) === JSON.stringify(chosen.map(source => source.key));
    const stale = !sameSources || displayedTask.question !== api.getQuestion() || displayedTask.workspaceRevision !== workspace.summary().revision;
    taskNotice.hidden = false;
    taskNotice.textContent = stale
      ? (english() ? 'This JSON is an earlier saved task. Export a new task to include the current question and sources.' : 'Этот JSON — ранее сохранённое задание. Экспортируйте новое, чтобы включить текущий вопрос и источники.')
      : (english() ? 'This JSON is a saved task snapshot.' : 'Этот JSON — снимок сохранённого задания.');
  };
  const notifySources = () => { const current = chosen.map(source => copy(source)); for (const listener of sourceListeners) listener(current); updateTaskNotice(); };
  const onContextChanged = () => {
    updateTaskNotice();
    if (!activeResponse || applyingResponse) return;
    const stale = preflight();
    if (stale) { applyButton.disabled = true; responseStatus.textContent = stale; message(stale); }
    else void renderReview();
  };
  const unsubscribeQuestion = api.subscribeQuestion?.(onContextChanged);
  const unsubscribeWorkspace = workspace.subscribe(onContextChanged);
  const sourceTitle = source => {
    const selector = source.reference?.selector;
    const range = Number.isSafeInteger(selector?.start) && Number.isSafeInteger(selector?.end) && selector.end > selector.start
      ? `${selector.start}–${selector.end} ${english() ? 'characters' : 'знаков'}` : '';
    return [source.title, source.versionLabel, range].filter(Boolean).join(' · ') || source.id;
  };
  // Task snapshots retain their original labels. Visible source addresses use
  // current corpus metadata so a profile/language change does not mix UI copy.
  const sourceLabels = new Map();
  const localizeSourceNode = (node, source) => {
    if (!api.provider) return;
    const language = english() ? 'en' : 'ru', key = `${language}:${referenceKey(source.reference)}`;
    if (!sourceLabels.has(key)) sourceLabels.set(key,
      describeSourceLocator(JSON.stringify(source.reference), shelf, api.provider, language).catch(() => null));
    void sourceLabels.get(key).then(result => {
      if (node.isConnected && language === (english() ? 'en' : 'ru') && result?.status === 'exact' && result.label) node.textContent = result.label;
    });
  };
  const renderSources = () => {
    sourceList.replaceChildren();
    if (!chosen.length) sourceList.append(text('p', 'pr-agent-empty', english() ? 'No exact passage selected yet.' : 'Точные фрагменты ещё не выбраны.'));
    chosen.forEach((source, index) => {
      const row = text('article', 'pr-agent-source');
      const titleNode = text('strong', '', sourceTitle(source));
      localizeSourceNode(titleNode, source);
      const quoteNode = text('p', 'pr-agent-selection', source.quote || (english() ? 'Exact passage reference' : 'Точная ссылка на фрагмент'));
      const contextNode = text('p', 'pr-agent-context', source.context || '');
      row.append(titleNode, quoteNode);
      if (source.sourceLayerKind) row.append(text('p', 'pr-agent-editorial', editorialLabel(source.sourceLayerKind, english() ? 'en' : 'ru') || source.sourceLayerKind));
      const range = source.contextRange;
      const rangeLabel = range ? `${english() ? 'Context in unit' : 'Контекст в единице'}: ${range.start}–${range.end}${source.contextTruncated ? (english() ? ' · excerpt truncated' : ' · фрагмент сокращён') : ''}` : '';
      if (rangeLabel) row.append(text('p', 'pr-agent-truncated', rangeLabel));
      for (const annotation of source.editorialAnnotations || []) {
        const fragment = Array.from(source.context || '').slice(annotation.start-range.start, annotation.end-range.start).join('');
        row.append(text('p', 'pr-agent-editorial', `${editorialLabel(annotation.kind, english() ? 'en' : 'ru') || annotation.kind} · ${annotation.start}–${annotation.end}: ${fragment}`));
      }
      const remove = button(t().remove, 'pr-agent-small'); remove.addEventListener('click', () => { chosen.splice(index, 1); persistBasket(); renderSources(); notifySources(); });
      const reopen = button(t().reopen, 'pr-agent-small'); reopen.addEventListener('click', () => void reopenSource(source));
      row.append(contextNode, text('code', '', source.id), remove, reopen); sourceList.append(row);
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
      const chars = Array.from(String(selected.text || '')), selector = checked.reference.selector;
      const start = selector.end > selector.start ? Math.max(0, Math.min(selector.start - Math.floor((MAX_QUOTE - (selector.end-selector.start))/2), chars.length-MAX_QUOTE)) : 0;
      const end = Math.min(chars.length, start + MAX_QUOTE);
      const contextRange = selected.contextRange || {start,end,positionUnit:'unicode_code_point',interval:'half_open'};
      const source = checkedBasketSource({id:`s${++sourceSequence}`, reference:checked.reference, title, quote,
        context:selected.context ?? chars.slice(start,end).join(''), contextRange,
        contextTruncated:selected.contextTruncated ?? (start > 0 || end < chars.length),
        editorialAnnotations:selected.editorialAnnotations ?? [], ...(selected.sourceLayerKind ? {sourceLayerKind:selected.sourceLayerKind} : {}),
        language:bounded(selected.language || '', 64, 'Language', {empty:true}), versionLabel:bounded(selected.versionLabel || '', 512, 'Edition label', {empty:true})});
      chosen.push({...source,key:checked.key}); persistBasket(); renderSources(); notifySources();
      message((english() ? 'Exact passage selected.' : 'Точный фрагмент выбран.') + (!basketSaved
        ? (english() ? ' This selection cannot be retained in this tab; save or copy it before leaving.' : ' Подборку не удалось сохранить во вкладке; сохраните или скопируйте её перед уходом.') : '')); return true;
    } catch (error) { message(displayPublicError(error, locale)); return false; }
  };
  const importSources = incoming => {
    if (!basketWritable) throw new Error('The earlier source selection could not be restored; it has not been overwritten.');
    const merged = mergeSourceBasket(chosen, incoming, sourceSequence);
    chosen = merged.sources; sourceSequence = merged.sequence; persistBasket(); renderSources(); notifySources();
    return merged.mapping;
  };

  const restoreTasks = async () => {
    try {
      const packet = await notebook.exportPacket();
      const saved = packet.preferences.filter(item => item.key.startsWith(TASK_KEY_PREFIX)).slice(0, 5_000);
      for (const item of saved) {
        const taskId = item.key.slice(TASK_KEY_PREFIX.length);
        if (!taskId || taskId.length > 128) continue;
        try {
          const value = item.value;
          if (value) { const task = taskPacket(value); if (task.taskId === taskId && new TextEncoder().encode(JSON.stringify(task)).byteLength <= TASK_LIMIT) tasks.push(task); }
        } catch { /* Keep other saved tasks available when one old packet is invalid. */ }
      }
      savedResponses = packet.preferences.filter(item => item.key.startsWith(RESPONSE_KEY_PREFIX) && object(item.value))
        .map(item => ({...item.value,hash:item.key.split(':').at(-1)})).filter(item => typeof item.taskId === 'string' && typeof item.raw === 'string');
      renderTaskHistory();
    } catch { /* task exchange remains available in this page session */ }
  };
  const saveTask = async task => {
    const serialized = JSON.stringify(task);
    if (new TextEncoder().encode(serialized).byteLength > TASK_LIMIT) throw new TypeError('The task exceeds the 64 KB export limit. Remove sources or shorten the question.');
    try {
      await notebook.setPreference(`${TASK_KEY_PREFIX}${task.taskId}`, task, {expectedValue: undefined});
      for (let attempt = 0; attempt < 3; attempt++) {
        const ids = await notebook.getPreference(TASKS_KEY);
        if (ids !== undefined && !Array.isArray(ids)) throw new Error('The saved task index is invalid.');
        const nextIds = Array.isArray(ids) ? ids.filter(value => value !== task.taskId) : [];
        nextIds.push(task.taskId);
        try { await notebook.setPreference(TASKS_KEY, nextIds, {expectedValue: ids}); break; }
        catch (error) { if (error?.code !== 'conflict' || attempt === 2) throw error; }
      }
      tasks = [...tasks.filter(value => value.taskId !== task.taskId), task];
      renderTaskHistory();
    } catch { throw new Error(english() ? 'The task could not be saved in the notebook.' : 'Не удалось сохранить задание в блокноте.'); }
  };
  const createTask = async () => {
    if (creatingTask) throw new Error('A task is already being created.');
    creatingTask = true; exportButton.disabled = true; copyButton.disabled = true;
    try {
    await tasksReady;
    if (api.ensureCurrentQuestion && !(await api.ensureCurrentQuestion())) throw new Error('The research question has unsaved changes or changed in another tab.');
    const question = bounded(await api.getQuestion(), 4_000, 'Question');
    if (!chosen.length) throw new TypeError(english() ? 'Add at least one exact selected source.' : 'Добавьте хотя бы один точный выбранный источник.');
    const sources = chosen.map(({key, ...source}) => source);
    const task = taskPacket({schema: AGENT_TASK_SCHEMA_V2, version: TASK_VERSION, taskId: id('task'), createdAt: new Date().toISOString(),
      profile: api.profile, question, workspaceRevision: workspace?.summary().revision ?? 0, sources,
      instructions: TASK_INSTRUCTIONS_V2,
      responseContract: {schema: AGENT_RESPONSE_SCHEMA, version: RESPONSE_VERSION,
        fields: ['taskId', 'uncertainty', 'proposals[{id,kind:interpretation|relation,statement,sourceIds,evidenceIds,objection?{statement,sourceIds}}]']}});
    await saveTask(task);
    activeTask = task; activeResponse = null; selectedProposalIds.clear(); selectedObjectionIds.clear(); applyButton.disabled = true; review.replaceChildren();
    taskText.value = JSON.stringify(task, null, 2);
    displayedTask = task; copySnapshotButton.disabled = false; updateTaskNotice();
    message(english() ? 'Task saved in this device notebook. Export or copy it to any external agent.' : 'Задание сохранено в блокноте этого устройства. Экспортируйте или скопируйте его для любого внешнего агента.');
    return task;
    } finally { creatingTask = false; exportButton.disabled = false; copyButton.disabled = false; }
  };
  const readImport = async () => {
    await tasksReady;
    const raw = responseText.value;
    if (new TextEncoder().encode(raw).byteLength > RESPONSE_LIMIT) throw new TypeError('The response exceeds 64 KB.');
    let value; try { value = JSON.parse(raw); } catch { throw new TypeError('The response is not valid JSON.'); }
    const matchingTask = tasks.find(task => task.taskId === value?.taskId) || activeTask;
    if (!matchingTask) throw new TypeError(t().noTask);
    const task = taskPacket(matchingTask);
    const response = responsePacket(value, task);
    activeTask = task; activeResponse = response;
    selectedProposalIds = new Set(response.proposals.map(proposal => proposal.id));
    selectedObjectionIds = new Set(response.proposals.filter(proposal => proposal.objection).map(proposal => proposal.id));
    try {
      const digest = await globalThis.crypto.subtle.digest('SHA-256', new TextEncoder().encode(raw));
      const hash = [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('');
      const key = `${RESPONSE_KEY_PREFIX}${task.taskId}:${hash}`, previous = await notebook.getPreference(key);
      if (previous === undefined) await notebook.setPreference(key, {taskId:task.taskId,raw}, {expectedValue:undefined});
      else if (previous?.raw !== raw || previous?.taskId !== task.taskId) throw new Error('Response storage conflict');
      if (!savedResponses.some(item => item.taskId === task.taskId && item.raw === raw)) savedResponses.push({taskId:task.taskId,raw,hash});
      renderTaskHistory();
      if (activeTask === task && activeResponse === response && responseText.value === raw) responseNotice.textContent = english() ? 'Original response saved on this device.' : 'Исходный ответ сохранён на этом устройстве.';
    } catch { if (activeTask === task && activeResponse === response && responseText.value === raw) responseNotice.textContent = english() ? 'The response could not be saved; copy it before leaving.' : 'Не удалось сохранить ответ; скопируйте его перед уходом.'; }
    if (activeTask !== task || activeResponse !== response || responseText.value !== raw) return response;
    return response;
  };
  const clearResponsePreview = () => {
    activeResponse = null; selectedProposalIds.clear(); selectedObjectionIds.clear(); review.replaceChildren(); applyButton.disabled = true; responseStatus.textContent = '';
    responseNotice.textContent = english() ? 'Preview the response before applying.' : 'Проверьте ответ перед применением.';
  };
  const resolveResponse = async (task = activeTask, response = activeResponse) => {
    if (!task || !response) return new Map();
    const sourceIds = new Set();
    response.proposals.forEach(proposal => {
      proposal.sourceIds.concat(proposal.evidenceIds).forEach(sourceId => sourceIds.add(sourceId));
      proposal.objection?.sourceRefs.forEach(sourceId => sourceIds.add(sourceId));
    });
    const byId = new Map(task.sources.map(source => [source.id, source]));
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
    const task = activeTask, response = activeResponse;
    const results = await resolveResponse(task, response);
    if (activeTask !== task || activeResponse !== response) return;
    const sources = new Map(task.sources.map(source => [source.id, source]));
    const selectedExact = () => selectedProposals().every(proposal =>
      [...proposal.sourceIds, ...proposal.evidenceIds, ...(proposal.objection?.sourceRefs || [])].every(sourceId => results.get(sourceId)?.status === 'exact'));
    const cards = [];
    const citationNode = sourceId => {
      const source = sources.get(sourceId); const result = results.get(sourceId);
      if (!source) return text('span', 'pr-agent-stale', `${sourceId}: ${t().stale}`);
      const row = text('div', `pr-agent-citation ${result?.status === 'exact' ? 'is-exact' : 'is-stale'}`);
      if (result?.status !== 'exact') row.append(text('span', 'pr-agent-stale', `${sourceTitle(source)} — ${t().stale}`));
      else {
        const open = button(sourceTitle(source), 'pr-agent-cite'); localizeSourceNode(open, source); open.addEventListener('click', () => void reopenSource(source));
        row.append(open, text('small', '', t().exact));
      }
      return row;
    };
    for (const proposal of activeResponse.proposals) {
      const card = text('article', 'pr-agent-proposal');
      const label = document.createElement('label'); label.className = 'pr-agent-select';
      const check = document.createElement('input'); check.type = 'checkbox'; check.checked = selectedProposalIds.has(proposal.id); check.addEventListener('change', () => { check.checked ? selectedProposalIds.add(proposal.id) : selectedProposalIds.delete(proposal.id); if (objectionCheck) objectionCheck.disabled = !check.checked; applyButton.disabled = applyingResponse || Boolean(preflight()) || !selectedExact() || !selectedProposalIds.size; });
      label.append(check, text('strong', '', `${t().proposal} · ${kindLabel(proposal.kind, locale)}`));
      card.append(label, text('p', '', proposal.statement));
      const citations = text('div', 'pr-agent-citations'); proposal.sourceIds.forEach(sourceId => citations.append(citationNode(sourceId)));
      card.append(text('h4', '', t().source), citations);
      const evidence = text('div', 'pr-agent-citations'); proposal.evidenceIds.forEach(sourceId => evidence.append(citationNode(sourceId)));
      card.append(text('h4', '', english() ? 'Evidence' : 'Основание'), evidence);
      let objectionCheck = null;
      if (proposal.objection) {
        const objectionLabel = document.createElement('label'); objectionLabel.className = 'pr-agent-select';
        objectionCheck = document.createElement('input'); objectionCheck.type = 'checkbox'; objectionCheck.checked = selectedObjectionIds.has(proposal.id); objectionCheck.disabled = !check.checked;
        objectionCheck.addEventListener('change', () => { objectionCheck.checked ? selectedObjectionIds.add(proposal.id) : selectedObjectionIds.delete(proposal.id); applyButton.disabled = applyingResponse || Boolean(preflight()) || !selectedExact() || !selectedProposalIds.size; });
        objectionLabel.append(objectionCheck, text('strong', '', english() ? 'Include this objection' : 'Добавить это возражение'));
        card.append(objectionLabel, text('p', 'pr-agent-objection', proposal.objection.statement), text('small', 'pr-agent-empty', english()
          ? 'An unchecked objection stays in the original response and is not applied.'
          : 'Неотмеченное возражение остаётся в исходном ответе и не добавляется в исследование.'));
        const objections = text('div', 'pr-agent-citations'); proposal.objection.sourceRefs.forEach(sourceId => objections.append(citationNode(sourceId)));
        card.append(objections);
      }
      cards.push(card);
    }
    if (activeResponse.uncertainty) review.append(text('p', 'pr-agent-uncertainty', `${english() ? 'Uncertainty' : 'Неопределённость'}: ${activeResponse.uncertainty}`));
    cards.forEach(card => review.append(card));
    if (selectedExact()) {
      const stale = preflight();
      applyButton.disabled = applyingResponse || Boolean(stale) || selectedProposalIds.size === 0;
      responseStatus.textContent = stale || (english() ? 'Exact source addresses are available; the argument still needs review.' : 'Точные адреса доступны; содержание довода требует проверки.');
      message(responseStatus.textContent);
    } else {
      applyButton.disabled = true; responseStatus.textContent = t().stale; message(t().stale);
    }
  };
  const applyResponse = async () => {
    if (!workspace || !activeTask || !activeResponse || !selectedProposalIds.size) throw new Error(english() ? 'The personal workspace is unavailable or no proposal is selected.' : 'Личная рабочая область недоступна или предложения не выбраны.');
    const task = activeTask, response = activeResponse, raw = responseText.value;
    const checkResponse = () => { if (activeTask !== task || activeResponse !== response || responseText.value !== raw) throw new Error('The response changed; preview it again before applying.'); };
    const results = await resolveResponse(task, response); checkResponse();
    const referenced = new Set();
    const selected = selectedProposals();
    selected.forEach(proposal => {
      proposal.sourceIds.concat(proposal.evidenceIds).forEach(sourceId => referenced.add(sourceId));
      proposal.objection?.sourceRefs.forEach(sourceId => referenced.add(sourceId));
    });
    if ([...referenced].some(sourceId => results.get(sourceId)?.status !== 'exact')) { await renderReview(); throw new Error(t().stale); }
    const sourceMap = new Map(task.sources.map(source => [source.id, source]));
    const checkWorkspace = () => {
      const saved = globalThis.localStorage.getItem(WORKSPACE_KEY);
      if (workspace.persistenceError?.() || (saved !== null && saved !== workspace.exportPacket())) {
        throw new Error('The workspace changed in another tab. Review the saved state before applying this response.');
      }
    };
    const checkQuestion = async () => {
      if ((api.ensureCurrentQuestion && !(await api.ensureCurrentQuestion())) || bounded(await api.getQuestion(), 4_000, 'Question') !== task.question) {
        throw Object.assign(new Error('The question changed after task export.'), {code:'agent-question-changed'});
      }
    };
    await checkQuestion();
    checkResponse();
    checkWorkspace();
    const work = workspace;
    let plan = planAgentApplication(task, response, selected, work.getState());
    if (plan.already) return t().appliedAlready;
    if (task.workspaceRevision !== work.summary().revision) throw new Error(english()
      ? 'The workspace changed after this task was exported. Keep the response for review, then export a new task before applying it.'
      : 'После экспорта задания рабочая область изменилась. Ответ сохранён для просмотра; перед применением экспортируйте новое задание.');
    const dataFingerprint = await fingerprint(task.sources); checkResponse();
    const sourceLocators = new Map();
    for (const sourceId of referenced) {
      const source = sourceMap.get(sourceId);
      sourceLocators.set(sourceId, await ensureSourceRecord(shelf, source.reference, source.title)); checkResponse();
    }

    plan = planAgentApplication(task, response, selected, work.getState());
    if (plan.already) return t().appliedAlready;
    if (task.workspaceRevision !== work.summary().revision) throw new Error(english()
      ? 'The workspace changed after this task was exported. Keep the response for review, then export a new task before applying it.'
      : 'После экспорта задания рабочая область изменилась. Ответ сохранён для просмотра; перед применением экспортируйте новое задание.');
    await checkQuestion();
    checkResponse();
    checkWorkspace();
    stageAgentProposals({task, selected, workspace:work, sourceLocators, dataFingerprint, plan});
    return t().applied;
  };

  addSource.addEventListener('click', addSelectedSource);
  exportButton.addEventListener('click', async () => {
    try {
      const task = await createTask();
      const blob = new Blob([JSON.stringify(task, null, 2)], {type: 'application/json'});
      const url = URL.createObjectURL(blob); const anchor = document.createElement('a');
      anchor.href = url; anchor.download = `tos-agent-task-${task.taskId}.json`; anchor.click(); setTimeout(() => URL.revokeObjectURL(url), 0);
    } catch (error) { message(displayPublicError(error, locale)); }
  });
  copyButton.addEventListener('click', async () => {
    try {
      const task = await createTask(); const payload = JSON.stringify(task, null, 2);
      taskText.value = payload;
      if (!navigator.clipboard?.writeText) throw new Error(t().copyFail);
      await navigator.clipboard.writeText(payload); message(english() ? 'Task JSON copied.' : 'JSON задания скопирован.');
    } catch (error) { message(error?.message === t().copyFail ? t().copyFail : displayPublicError(error, locale)); }
  });
  copySnapshotButton.addEventListener('click', async () => {
    if (!displayedTask) return;
    try { if (!navigator.clipboard?.writeText) throw new Error(t().copyFail); await navigator.clipboard.writeText(taskText.value); message(english() ? 'Displayed saved task copied.' : 'Показанное сохранённое задание скопировано.'); }
    catch { message(t().copyFail); }
  });
  previewButton.addEventListener('click', async () => {
    if (previewButton.disabled || applyingResponse) return;
    previewButton.disabled = true; responseText.readOnly = true; fileInput.disabled = true; taskHistory.inert = true; applyButton.disabled = true;
    try { await readImport(); }
    catch (error) { activeResponse = null; responseStatus.textContent = displayPublicError(error, locale); review.replaceChildren(text('p', 'pr-agent-stale', responseStatus.textContent)); message(responseStatus.textContent); }
    finally { previewButton.disabled = false; responseText.readOnly = false; fileInput.disabled = false; taskHistory.inert = false; if (activeResponse) await renderReview(); }
  });
  responseText.addEventListener('input', clearResponsePreview);
  applyButton.addEventListener('click', async () => {
    if (applyingResponse) return;
    applyingResponse = true; applyButton.disabled = true; previewButton.disabled = true; responseText.readOnly = true; fileInput.disabled = true; taskHistory.inert = true; review.inert = true;
    let outcome;
    try { outcome = await applyResponse(); }
    catch (error) { outcome = displayPublicError(error, locale); }
    finally {
      applyingResponse = false; previewButton.disabled = false; responseText.readOnly = false; fileInput.disabled = false; taskHistory.inert = false; review.inert = false;
      if (activeResponse) await renderReview();
      if (outcome) { responseStatus.textContent = outcome; message(outcome); }
    }
  });
  fileInput.addEventListener('change', async () => {
    const file = fileInput.files?.[0]; if (!file) return;
    if (file.size > RESPONSE_LIMIT) { message(english() ? 'The response exceeds 64 KB.' : 'Ответ превышает 64 КБ.'); fileInput.value = ''; return; }
    responseText.value = await file.text(); clearResponsePreview(); fileInput.value = '';
  });
  restoreBasket();
  const tasksReady = restoreTasks();
  renderSources();
  let destroyed = false;
  const destroy = () => { if (destroyed) return; destroyed = true; window.removeEventListener('pagehide', pageHide); unsubscribeQuestion?.(); unsubscribeWorkspace?.(); sourceListeners.clear(); section.remove(); };
  const pageHide = event => { if (!event.persisted) destroy(); };
  window.addEventListener('pagehide', pageHide);
  const refreshLocale = () => { heading.textContent=t().heading; intro.textContent=t().intro; sourceHeading.textContent=t().sourceCount; addSource.textContent=t().add; exportButton.textContent=t().export; copyButton.textContent=t().copy; copySnapshotButton.textContent=english()?'Copy displayed saved task':'Копировать показанное задание'; importHeading.textContent=t().import; importLabelText.textContent=t().import; previewButton.textContent=t().preview; applyButton.textContent=t().apply; taskText.setAttribute('aria-label', english() ? 'Agent task JSON' : 'JSON задания агенту'); responseText.setAttribute('aria-label', english() ? 'Agent response JSON' : 'JSON ответа агента'); fileInput.setAttribute('aria-label', t().file); status.textContent=t().status; updateBasketHint(); renderSources(); renderTaskHistory(); updateTaskNotice(); if (activeResponse) void renderReview(); };
  refreshLocale();
  return {destroy, refreshLocale, addSelectedSource, importSources, exportTask: createTask,
    selectedSources: () => chosen.map(source => copy(source)),
    hasUnsavedSources: () => chosen.length > 0 && !basketSaved,
    subscribeSources(listener) { if (typeof listener !== 'function') throw new TypeError('A source listener is required.'); sourceListeners.add(listener); listener(chosen.map(source => copy(source))); return () => sourceListeners.delete(listener); }};
}

