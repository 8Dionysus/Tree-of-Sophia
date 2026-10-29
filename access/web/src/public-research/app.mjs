import {mountCorpusEntry} from '../corpus-reader/host.mjs';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {createPublicCorpusProvider} from '../corpus-reader/public-provider.mjs';
import {decodeReadingRoute, encodeReadingRoute} from '../corpus-reader/route.mjs';
import {mountPublicAgentBridge} from './agent-bridge.mjs';
import {createPublicResearchOwners, mountPublicWorkspaceView} from './workspace-view.mjs';
import {initializePublicReaderRules} from './bootstrap.mjs';
import {createPublicSpace} from './space-library.mjs';
import {mountPublicResearchSpace} from './public-space.mjs';
import './public-research.css';

const root = document.querySelector('#public-research') ?? document.querySelector('#public-reader-host');
const profile = root?.dataset.profile === 'antiquity' ? 'antiquity' : 'zarathustra';
const requestedLocale=new URLSearchParams(location.search).get('lang');
let locale=requestedLocale==='en'||requestedLocale==='ru'?requestedLocale:(navigator.language.toLowerCase().startsWith('en')?'en':'ru');
let english=locale==='en';
const copy = {
  zarathustra: {
    title: 'Заратустра', eyebrow: 'Читать и исследовать',
    intro: 'Одно произведение. Много возвращений к его словам.',
    description: 'Читайте доступные издания, отмечайте точные места и сохраняйте собственные мысли в общем блокноте.',
    question: 'С каким вопросом вы пришли к Заратустре?', placeholder: 'Например: что меняется, когда человек становится творцом собственных ценностей?',
    other: 'Античность', status: 'Данные о корпусе загружаются по запросу.',
  },
  antiquity: {
    title: 'Античность', eyebrow: 'Читать и исследовать',
    intro: 'Один вопрос проходит сквозь разные тексты и голоса.',
    description: 'Сопоставляйте доступные источники, сохраняйте точные места и возвращайтесь к своей линии исследования.',
    question: 'Какой вопрос вы хотите пронести через античные тексты?', placeholder: 'Например: что делает жизнь хорошей?',
    other: 'Заратустра', status: 'Данные о корпусе загружаются по запросу.',
  },
};
const copyEn={
  zarathustra:{title:'Zarathustra',eyebrow:'Read and explore',intro:'One work. Many returns to its words.',description:'Read available editions, mark exact passages, and keep your own thoughts in a shared notebook.',question:'What question brings you to Zarathustra?',placeholder:'For example: what changes when a person creates their own values?',other:'Antiquity',status:'Corpus details load when requested.'},
  antiquity:{title:'Antiquity',eyebrow:'Read and explore',intro:'One question moves across texts and voices.',description:'Compare available sources, save exact places, and return to your own line of inquiry.',question:'What question will you carry through ancient texts?',placeholder:'For example: what makes a life good?',other:'Zarathustra',status:'Corpus details load when requested.'},
};
const stringsFor=()=>english?copyEn[profile]:copy[profile];
let strings=stringsFor();
document.title=`${strings.title} · ${english?'Tree of Sophia':'Древо Софии'}`;

const h = (tag, cls, value) => { const node = document.createElement(tag); if (cls) node.className = cls; if (value !== undefined) node.textContent = value; return node; };
const button = (label, cls = '') => { const node = h('button', cls, label); node.type = 'button'; return node; };
const link = (label, href, cls = '') => { const node = h('a', cls, label); node.href = href; return node; };
const routeBase = location.pathname.startsWith(import.meta.env.BASE_URL) ? import.meta.env.BASE_URL : '/';
const profileUrl = name => `${routeBase}${name}/`;
const alternate = profileUrl(profile === 'antiquity' ? 'zarathustra' : 'antiquity');
const targetProfile = profile === 'antiquity' ? 'zarathustra' : 'antiquity';
const toolbar = h('nav', 'main-tools pr-toolbar');
const readerRoot = h('main', 'pr-page pr-space-page');
readerRoot.id = 'public-reader-host';
const header = h('header', 'header pr-header');
header.append(link(english ? 'Tree of Sophia' : 'Древо Софии', profileUrl(profile), 'pr-wordmark'), toolbar);
const hero = h('section', 'pr-hero');
const eyebrow = h('p', 'pr-eyebrow', strings.eyebrow);
const title = h('h1', '', strings.title);
const intro = h('p', 'pr-intro', strings.intro);
const description = h('p', 'pr-description', strings.description);
const actions = h('div', 'pr-actions');
// The two public spaces sit side by side: the current one is marked, the other is a link.
const spacesNav = h('nav', 'pr-spaces');
const currentSpace = h('span', 'pr-space-current', strings.title);
currentSpace.setAttribute('aria-current', 'page');
const switchProfile = link(strings.other, alternate, 'pr-switch');
switchProfile.href = `${alternate}?lang=${locale}`;
const labelSpaces = () => { spacesNav.setAttribute('aria-label', english ? 'Spaces' : 'Пространства'); switchProfile.setAttribute('aria-label', `${english ? 'Explore' : 'Перейти'}: ${strings.other}`); };
labelSpaces();
if (profile === 'zarathustra') spacesNav.append(currentSpace, switchProfile); else spacesNav.append(switchProfile, currentSpace);
toolbar.append(spacesNav);
const questionLabel = h('label', 'pr-question-label', strings.question);
const question = document.createElement('textarea');
question.className = 'pr-question';
question.rows = 2;
question.maxLength = 4000;
question.placeholder = strings.placeholder;
question.autocomplete = 'off';
questionLabel.append(question);
const saveState = h('p', 'pr-save-state', english ? 'Your question is saved on this device.' : 'Вопрос сохраняется в блокноте на этом устройстве.');
saveState.setAttribute('role', 'status');
saveState.setAttribute('aria-live', 'polite');
const resolveQuestionConflict = button(english ? 'Keep this draft and load the saved question' : 'Сохранить этот черновик и открыть сохранённый вопрос', 'pr-question-recover');
resolveQuestionConflict.hidden = true;
const questionHistory = document.createElement('details');
questionHistory.className = 'pr-question-history';
const questionHistoryTitle = h('summary', '', english ? 'Previous questions' : 'Предыдущие вопросы');
const questionHistoryList = h('div', 'pr-question-history-list');
questionHistory.append(questionHistoryTitle, questionHistoryList);
const status = h('p', 'pr-provider-state', strings.status);
status.setAttribute('role', 'status');
const journeys = h('section', 'pr-journeys');
journeys.hidden = true;
hero.append(eyebrow, title, intro, description, actions, questionLabel, saveState, resolveQuestionConflict, questionHistory, status, journeys);
root.replaceWith(readerRoot);

const DB_NAME = 'tos-public-research-v1';
const QUESTION_KEY = 'tos.public-research.question.v1';
const QUESTION_HISTORY_PREFIX = 'tos.public-research.question-history.v1:';
const routeKey = name => `tos.public-research.last-route.${name}.v1`;
const notebook = createCorpusNotebook({dbName: DB_NAME});
let entry;
let agentBridge;
let workspaceView;
let publicOwners;
let spaceView;
let appDisposed=false;
let saveTimer = null;
let questionTouched = false;
let questionSequence = 0;
let questionWrite = Promise.resolve();
let questionExpected;
let allowProfileLeave = false;
let resolveQuestionReady;
const questionReady = new Promise(resolve => { resolveQuestionReady = resolve; });
const questionListeners = new Set();
const notifyQuestion = () => { for (const listener of questionListeners) listener(question.value); };
const historyId = () => globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
let corpusWorkCount;
function refreshProviderStatus(){status.textContent=corpusWorkCount===false?(english?'The public corpus could not be reached. Open the reader to check source availability.':'Не удалось связаться с публичным корпусом. Откройте чтение, чтобы проверить доступность источника.'):Number.isInteger(corpusWorkCount)?(english?`${corpusWorkCount} works are listed. Text is loaded in bounded passages.`:`Произведений в списке: ${corpusWorkCount}. Текст загружается небольшими фрагментами.`):strings.status;}

function validRoute(value) {
  if (typeof value !== 'string' || !value.startsWith('#reading=')) return null;
  try { decodeReadingRoute(value); return value; } catch { return null; }
}

// Profiles choose a catalog and keep their own last route. Explicit source
// addresses, including an unavailable one, must reach the provider unchanged.

function persistQuestion() {
  clearTimeout(saveTimer);
  saveTimer = null;
  if (!questionTouched) return questionWrite.then(() => true, () => true);
  const sequence = questionSequence;
  const value = question.value;
  questionWrite = questionWrite.catch(() => {}).then(async () => {
    if (!(await questionReady)) throw new Error('Notebook storage is unavailable.');
    await notebook.setPreference(QUESTION_KEY, value, {expectedValue: questionExpected,
      archivePreviousKey: `${QUESTION_HISTORY_PREFIX}${historyId()}`});
    questionExpected = value;
  });
  return questionWrite.then(() => {
    if (sequence === questionSequence) {
      questionTouched = false;
      resolveQuestionConflict.hidden = true;
      saveState.textContent = english ? 'Saved in the shared notebook on this device.' : 'Сохранено в общем блокноте на этом устройстве.';
      if (questionHistory.open) void renderQuestionHistory();
    }
    return true;
  }).catch(error => {
    if (sequence === questionSequence) {
      resolveQuestionConflict.hidden = error?.code !== 'conflict';
      saveState.textContent = error?.code === 'conflict'
        ? (english ? 'This question changed in another tab. Keep this draft before loading the saved question.' : 'Вопрос изменился в другой вкладке. Сохраните этот черновик перед открытием общего вопроса.')
        : (english ? 'Could not save. Your text remains here until you leave.' : 'Не удалось сохранить. Текст останется здесь, пока вы не уйдёте со страницы.');
    }
    return false;
  });
}

function scheduleQuestionSave() {
  questionTouched = true;
  questionSequence += 1;
  notifyQuestion();
  clearTimeout(saveTimer);
  saveState.textContent = english ? 'Saving…' : 'Сохраняется…';
  saveTimer = setTimeout(() => void persistQuestion(), 250);
}

async function refreshQuestion() {
  if (!(await questionReady)) return false;
  try {
    await questionWrite.catch(() => {});
    if (questionTouched) return false;
    const sequence = questionSequence;
    const value = await notebook.getPreference(QUESTION_KEY);
    if (questionTouched || sequence !== questionSequence) return false;
    questionExpected = value;
    question.value = typeof value === 'string' ? value : '';
    notifyQuestion();
    return true;
  } catch { return false; }
}

async function ensureCurrentQuestion() {
  return (await persistQuestion()) && (await refreshQuestion());
}

let questionHistoryRender = 0;
let restoringHistoryQuestion = false;
async function renderQuestionHistory() {
  const request = ++questionHistoryRender;
  const openGroups = new Set([...questionHistoryList.querySelectorAll('details[open]')].map(node => node.dataset.historyGroup));
  try {
    const packet = await notebook.exportPacket();
    if (appDisposed || request !== questionHistoryRender) return;
    const entries = packet.preferences.filter(item => item.key.startsWith(QUESTION_HISTORY_PREFIX)
      && typeof item.value?.value === 'string').sort((a, b) => String(b.value.changedAt).localeCompare(String(a.value.changedAt)));
    const groups = [];
    for (const item of entries) {
      const previous = groups.at(-1)?.at(-1), at = Date.parse(item.value.changedAt), previousAt = Date.parse(previous?.value.changedAt);
      // Group only presentation by editing time. Every original snapshot remains
      // independently visible and restorable; no saved preference is rewritten.
      if (previous && !item.value.unsaved && !previous.value.unsaved && Number.isFinite(at)
        && Number.isFinite(previousAt) && previousAt - at >= 0 && previousAt - at < 120_000) groups.at(-1).push(item);
      else groups.push([item]);
    }
    const renderEntry = item => {
      const row = h('div', 'pr-question-history-entry');
      const kind = item.value.unsaved ? (english ? 'Draft preserved after a conflict' : 'Черновик, сохранённый после конфликта')
        : (english ? 'Saved editing snapshot' : 'Сохранённый снимок редактирования');
      const date = new Date(item.value.changedAt);
      const timestamp = Number.isFinite(date.getTime()) ? date.toLocaleString(english ? 'en' : 'ru') : item.value.changedAt;
      row.append(h('small', '', [kind, timestamp].filter(Boolean).join(' · ')), h('p', '', item.value.value));
      const restore = button(english ? 'Use this question' : 'Сделать текущим вопросом', 'pr-question-restore');
      const feedback = h('p', 'pr-save-state'); feedback.setAttribute('role', 'status');
      restore.disabled = item.value.value.length > 4000;
      restore.addEventListener('click', async () => {
        if (restoringHistoryQuestion) return;
        if (workspaceView?.hasUnsavedDraft?.()) {
          feedback.textContent = english ? 'Save or export the current thought draft before changing its question.'
            : 'Сохраните или экспортируйте текущий черновик мысли перед сменой его вопроса.';
          return;
        }
        restoringHistoryQuestion = true; restore.disabled = true;
        try {
          const sequence = questionSequence, currentText = question.value;
          if (!(await persistQuestion())) {
            feedback.textContent = english ? 'The saved question could not be changed. Your current text remains here.'
              : 'Не удалось сменить сохранённый вопрос. Текущий текст остаётся здесь.';
            return;
          }
          if (sequence !== questionSequence || currentText !== question.value) {
            feedback.textContent = english ? 'Your question changed while saving. The current edits remain here; choose the snapshot again when ready.'
              : 'Во время сохранения вопрос изменился. Новые правки остаются здесь; выберите снимок снова, когда будете готовы.';
            return;
          }
          if (!(await setQuestion(item.value.value))) {
            feedback.textContent = english ? 'The saved question could not be changed. Your current text remains here.'
              : 'Не удалось сменить сохранённый вопрос. Текущий текст остаётся здесь.';
            return;
          }
          question.focus();
        } catch {
          feedback.textContent = english ? 'This snapshot could not be restored. Both saved versions remain in history.'
            : 'Не удалось восстановить снимок. Обе сохранённые версии остаются в истории.';
        } finally { restoringHistoryQuestion = false; restore.disabled = item.value.value.length > 4000; }
      });
      row.append(restore, feedback); return row;
    };
    const rows = groups.map(items => {
      if (items.length === 1) return renderEntry(items[0]);
      const group = h('details', 'pr-question-history-group'); group.dataset.historyGroup = items.at(-1).key;
      group.open = openGroups.has(group.dataset.historyGroup);
      const snippet = items[0].value.value.slice(0, 100);
      group.append(h('summary', '', `${items.length} ${english ? 'editing snapshots' : 'снимков редактирования'} · ${snippet}${items[0].value.value.length > 100 ? '…' : ''}`), ...items.map(renderEntry));
      return group;
    });
    questionHistoryList.replaceChildren(...(rows.length ? rows : [h('p', '', english ? 'No earlier saved questions.' : 'Ранее сохранённых вопросов нет.')]));
  } catch { if (request === questionHistoryRender) questionHistoryList.textContent = english ? 'Could not load previous questions.' : 'Не удалось загрузить предыдущие вопросы.'; }
}

async function retainQuestionDraftAndRefresh() {
  const value = question.value, sequence = questionSequence;
  try {
    await notebook.setPreference(`${QUESTION_HISTORY_PREFIX}${historyId()}`,
      {value, changedAt:new Date().toISOString(), unsaved:true}, {expectedValue:undefined});
    if (sequence !== questionSequence || value !== question.value) {
      saveState.textContent = english ? 'A copy of the earlier draft was saved. The current edits remain here.' : 'Копия прежнего черновика сохранена. Текущие изменения остаются здесь.';
      return;
    }
    questionTouched = false;
    if (!(await refreshQuestion())) { questionTouched = true; throw new Error('Could not load the saved question.'); }
    resolveQuestionConflict.hidden = true;
    saveState.textContent = english ? 'Your draft is in Previous questions; the shared saved question is shown.' : 'Черновик сохранён в предыдущих вопросах; показан общий сохранённый вопрос.';
    if (questionHistory.open) await renderQuestionHistory();
  } catch {
    saveState.textContent = english ? 'Could not preserve the draft. Your text remains here.' : 'Не удалось сохранить черновик. Ваш текст остаётся здесь.';
  }
}

function confirmDraftNavigation() {
  return new Promise(resolve => {
    const dialog = document.createElement('dialog'); dialog.className = 'pr-draft-dialog';
    dialog.setAttribute('aria-labelledby', 'pr-draft-dialog-title');
    const heading = h('h2', '', english ? 'Unsaved research draft' : 'Несохранённый черновик исследования'); heading.id = 'pr-draft-dialog-title';
    const description = h('p', '', english
      ? 'Your research draft and selected sources are not saved. Stay to save or copy them, or leave and discard this draft.'
      : 'Черновик исследования и выбранные источники не сохранены. Останьтесь, чтобы сохранить или скопировать их, либо перейдите и потеряете этот черновик.');
    const stay = button(english ? 'Keep editing' : 'Продолжить работу');
    const leave = button(english ? 'Leave and discard draft' : 'Перейти и потерять черновик');
    let finished = false;
    const finish = result => { if (finished) return; finished = true; dialog.close(); dialog.remove(); resolve(result); };
    stay.addEventListener('click', () => finish(false)); leave.addEventListener('click', () => finish(true));
    dialog.addEventListener('cancel', event => { event.preventDefault(); finish(false); });
    dialog.append(heading, description, stay, leave); document.body.append(dialog);
    try { dialog.showModal(); stay.focus(); } catch { dialog.remove(); resolve(false); }
  });
}

async function switchProfiles(event) {
  event.preventDefault();
  try {
    if (workspaceView?.hasUnsavedDraft?.() && !(await confirmDraftNavigation())) return;
    if (!(await persistQuestion())) return;
    const currentRoute = validRoute(location.hash);
    if (currentRoute) await notebook.setPreference(routeKey(profile), currentRoute);
    const next=new URL(alternate,location.href);next.searchParams.set('lang',locale);
    allowProfileLeave = true;
    location.assign(next.toString());
  } catch {
    allowProfileLeave = false;
    status.textContent = english ? 'Could not save this profile before switching.' : 'Не удалось сохранить состояние профиля перед переключением.';
  }
}

function setLanguage(value,{updateUrl=true}={}){
  if(!['ru','en'].includes(value)||value===locale)return;
  locale=value;english=locale==='en';strings=stringsFor();document.documentElement.lang=locale;document.title=`${strings.title} · ${english?'Tree of Sophia':'Древо Софии'}`;refreshProviderStatus();
  header.querySelector('.pr-wordmark').textContent=english?'Tree of Sophia':'Древо Софии';
  switchProfile.textContent=strings.other;currentSpace.textContent=strings.title;labelSpaces();
  switchProfile.href=alternate+`?lang=${locale}`;
  if(questionLabel.firstChild?.nodeType===Node.TEXT_NODE)questionLabel.firstChild.nodeValue=strings.question;
  question.placeholder=strings.placeholder;
  saveState.textContent=questionTouched?(english?'Your question has unsaved changes.':'В вопросе есть несохранённые изменения.'):(english?'Your question is saved on this device.':'Вопрос сохраняется в блокноте на этом устройстве.');
  questionHistoryTitle.textContent=english?'Previous questions':'Предыдущие вопросы';
  resolveQuestionConflict.textContent=english?'Keep this draft and load the saved question':'Сохранить этот черновик и открыть сохранённый вопрос';
  if(questionHistory.open)void renderQuestionHistory();
  if(updateUrl){const url=new URL(location.href);url.searchParams.set('lang',locale);history.replaceState(history.state,'',url);}
  document.dispatchEvent(new CustomEvent('sophia-ui-language',{detail:{locale}}));
  spaceView?.refreshLocale();workspaceView?.refreshLocale?.();agentBridge?.refreshLocale?.();
}

function mountJourneys(items, documentIds) {
  if (!Array.isArray(items)) return;
  const usable = items.filter(item => item && typeof item === 'object'
    && ['id', 'title', 'question', 'documentId', 'versionId', 'unitId'].every(key => typeof item[key] === 'string' && item[key].trim().length > 0 && item[key].length <= 2048)
    && documentIds.includes(item.documentId));
  if (!usable.length) return;
  const heading = h('h2', 'pr-journeys-title', english ? 'Reading paths' : 'Показательные маршруты');
  journeys.setAttribute('aria-labelledby', 'pr-journeys-heading');
  heading.id = 'pr-journeys-heading';
  const list = h('div', 'pr-journey-list');
  for (const item of usable) {
    const card = h('article', 'pr-journey');
    const name = h('h3', '', item.title);
    const prompt = h('p', '', item.question);
    const details = h('details', 'pr-journey-details');
    details.append(h('summary', '', english ? 'Explore the question and voices' : 'Развернуть вопрос и голоса'));
    const openPlace = async target => {
      try {
        if (!window.tosPublicResearch?.getQuestion().trim() && !(await window.tosPublicResearch?.setQuestion(item.question))) return;
        await window.tosPublicResearch?.open({documentId: target.documentId, versionId: target.versionId, unitId: target.unitId});
      } catch {
        status.textContent = english ? 'This reading path could not be opened.' : 'Не удалось открыть этот маршрут чтения.';
      }
    };
    if (typeof item.readingHypothesis === 'string') {
      details.append(h('h4', '', english ? 'A reading to test' : 'Чтение для проверки'), h('p', '', item.readingHypothesis));
    }
    if (typeof item.strongAlternative === 'string') {
      details.append(h('h4', '', english ? 'A strong alternative' : 'Сильная альтернатива'), h('p', '', item.strongAlternative));
    }
    if (Array.isArray(item.steps)) {
      const steps = h('ol', 'pr-journey-steps');
      for (const step of item.steps) {
        const row = h('li', 'pr-journey-step');
        row.append(h('h4', '', step.title), h('small', 'pr-journey-voice', step.voice), h('p', '', step.prompt));
        const openStep = button(english ? 'Read the source' : 'Прочитать источник', 'pr-journey-open');
        openStep.addEventListener('click', () => void openPlace(step));
        row.append(openStep); steps.append(row);
      }
      details.append(steps);
    }
    details.append(h('p', '', english
      ? 'Return to your question, select two exact grounds, and save your thesis with an objection in My research.'
      : 'Вернитесь к своему вопросу: выберите два точных основания и сохраните тезис с возражением в «Моём исследовании».'));
    const open = button(english ? 'Start reading' : 'Начать чтение', 'pr-journey-open');
    open.addEventListener('click', () => { details.open = true; void openPlace(item); });
    card.append(name, prompt, open, details);
    list.append(card);
  }
  journeys.replaceChildren(heading, list);
  journeys.hidden = false;
}

async function setQuestion(value) {
  if (typeof value !== 'string' || value.length > 4000) throw new TypeError('Question must be a string of at most 4000 characters.');
  if (value === question.value && !questionTouched) return true;
  question.value = value;
  questionTouched = true;
  questionSequence += 1;
  notifyQuestion();
  return persistQuestion();
}

async function openAddress(address) {
  const returnFocus = document.activeElement;
  if (!address || typeof address !== 'object' || Array.isArray(address)
    || Object.keys(address).some(key => !['documentId', 'versionId', 'unitId', 'revision', 'reference'].includes(key))
    || ['documentId', 'versionId'].some(key => typeof address[key] !== 'string' || !address[key].trim() || address[key].length > 2048)
    || ['unitId', 'revision'].some(key => address[key] !== undefined && (typeof address[key] !== 'string' || !address[key].trim() || address[key].length > 2048))
    || (address.reference !== undefined && (!address.reference || typeof address.reference !== 'object' || Array.isArray(address.reference)))) {
    throw new TypeError('Open requires documentId and versionId, with an optional exact unitId, revision, or reference.');
  }
  if (!(await persistQuestion())) return null;
  const target = {documentId: address.documentId, versionId: address.versionId};
  if (address.unitId !== undefined) target.unitId = address.unitId;
  if (address.revision !== undefined) target.revision = address.revision;
  if (address.reference !== undefined) target.reference = address.reference;
  return entry?.open(target, {returnFocus}) ?? null;
}

async function start() {
  if(appDisposed)return;
  try {
    const savedQuestion = await notebook.getPreference(QUESTION_KEY);
    questionExpected = savedQuestion;
    if (!appDisposed && !questionTouched && typeof savedQuestion === 'string') question.value = savedQuestion;
    resolveQuestionReady(true);
  } catch {
    resolveQuestionReady(false);
    saveState.textContent = english ? 'Notebook storage is unavailable. Your question may not persist.' : 'Блокнот недоступен. Вопрос может не сохраниться.';
  }
  if(appDisposed)return;
  await initializePublicReaderRules();
  if(appDisposed)return;
  const provider = createPublicCorpusProvider({baseUrl: `${import.meta.env.BASE_URL}public-corpus/`, profile, fetch: globalThis.fetch.bind(globalThis)});
  let manifest = null;
  try { manifest = await provider.describe(); } catch { /* the reader will report delivery failure */ }
  if(appDisposed)return;
  const profileInfo = manifest?.profiles?.[profile];
  const activeRoute = validRoute(location.hash);
  if(appDisposed)return;
  if (location.hash !== (activeRoute ?? '')) history.replaceState(history.state, '', `${location.pathname}${location.search}${activeRoute ?? ''}`);
  entry = mountCorpusEntry({root: readerRoot, provider, notebook, toolbar, locale: () => locale, route: true, dbName: DB_NAME,
    onLibraryRequest:()=>spaceView?.openCollection(),
    onReturnFocusFallback:()=>spaceView?.restoreFocus?.(),
    onReadingLocation(address) {
      const saved = validRoute(encodeReadingRoute(address));
      if (saved) void notebook.setPreference(routeKey(profile), saved).catch(() => {
        saveState.textContent = english ? 'Could not save this reading position.' : 'Не удалось сохранить это место чтения.';
      });
    }});
  entry.opener.disabled = true;
  const narrowReader=readerRoot.querySelector('.corpus-open-narrow');if(narrowReader)narrowReader.disabled=true;
  window.tosPublicResearch = Object.freeze({profile, getQuestion: () => question.value, setQuestion, ensureCurrentQuestion,
    refreshQuestion, subscribeQuestion(listener) { questionListeners.add(listener); return () => questionListeners.delete(listener); },
    open: openAddress, openLibrary:()=>entry.open(), reader: entry.reader, provider, notebook: entry.notebook});
  publicOwners = createPublicResearchOwners();
  const publicSpace = await createPublicSpace({profile,provider});
  if(appDisposed)return;
  spaceView = mountPublicResearchSpace({host:readerRoot,header,profile,locale:()=>locale,library:publicSpace.library,
    bindings:publicSpace.bindings,documentBindings:publicSpace.documentBindings,groundTargets:publicSpace.groundTargets,routeGroundTargets:publicSpace.routeGroundTargets,graphPredecessors:publicSpace.graphPredecessors,api:window.tosPublicResearch,
    workspace:publicOwners.workspace,shelf:publicOwners.shelf,question:questionLabel,saveState,status,journeys:profileInfo?.journeys??[]});
  saveState.after(resolveQuestionConflict, questionHistory);
  header.addEventListener('public-space-locale',onPublicSpaceLocale);
  agentBridge = mountPublicAgentBridge({root:spaceView.agentRoot, api:window.tosPublicResearch, notebook:entry.notebook,
    workspace:publicOwners.workspace, shelf:publicOwners.shelf, locale:()=>locale});
  workspaceView = mountPublicWorkspaceView({root:spaceView.workspaceRoot, api:window.tosPublicResearch, notebook:entry.notebook,
    workspace:publicOwners.workspace, shelf:publicOwners.shelf, locale:()=>locale,
    graphSpaceContext:{profile,libraryFingerprint:publicSpace.library.fingerprint,pristineGraphs:spaceView.pristineGraphs,reload:()=>window.location.reload()}});
  workspaceView.connectAgentBridge(agentBridge);
  workspaceView?.render();
  spaceView.refreshLocale();
  if (manifest && profileInfo) {
    const workCount = Array.isArray(profileInfo?.documentIds) ? profileInfo.documentIds.length : null;
    corpusWorkCount=workCount;refreshProviderStatus();
    entry.opener.disabled = false;
    readerRoot.querySelector('.corpus-open-narrow')?.removeAttribute('disabled');
  } else {
    corpusWorkCount=false;refreshProviderStatus();
    entry.opener.disabled = true;
    readerRoot.querySelector('.corpus-open-narrow')?.setAttribute('disabled','');
  }
}

const onPublicSpaceLocale=event=>setLanguage(event.detail?.locale);
const onSwitchProfile=event=>void switchProfiles(event);
function disposeApp() {
  if(appDisposed)return;
  appDisposed=true;clearTimeout(saveTimer);
  switchProfile.removeEventListener('click',onSwitchProfile);
  header.removeEventListener('public-space-locale',onPublicSpaceLocale);
  question.removeEventListener('input',scheduleQuestionSave);
  window.removeEventListener('pagehide',onPageHide);
  window.removeEventListener('beforeunload',onBeforeUnload);
  agentBridge?.destroy();
  workspaceView?.destroy();
  spaceView?.destroy();
  delete window.tosPublicResearch;
  void persistQuestion().finally(() => Promise.resolve(entry?.destroy()).finally(() => notebook.close()));
}
function onPageHide(event) { if(!event.persisted)disposeApp(); }
function onBeforeUnload(event) {
  if (!allowProfileLeave && (questionTouched || workspaceView?.hasUnsavedDraft?.())) { event.preventDefault(); event.returnValue = ''; }
}
switchProfile.addEventListener('click',onSwitchProfile);
window.addEventListener('pagehide',onPageHide);
window.addEventListener('beforeunload',onBeforeUnload);
question.addEventListener('input', scheduleQuestionSave);
resolveQuestionConflict.addEventListener('click', () => void retainQuestionDraftAndRefresh());
questionHistory.addEventListener('toggle', () => { if(questionHistory.open)void renderQuestionHistory(); });
if(import.meta.hot)import.meta.hot.dispose(disposeApp);

void start().catch(error => {
  const message = h('p', 'pr-error', english ? 'The reader could not start. Reload the page to try again.' : 'Не удалось запустить читатель. Перезагрузите страницу.');
  readerRoot.append(message);
});
