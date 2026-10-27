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
const switchProfile = link(`${english ? 'Explore' : 'Перейти'} ${strings.other}`, alternate, 'pr-switch');
switchProfile.href = `${alternate}?lang=${locale}`;
toolbar.append(switchProfile);
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
const status = h('p', 'pr-provider-state', strings.status);
status.setAttribute('role', 'status');
const journeys = h('section', 'pr-journeys');
journeys.hidden = true;
hero.append(eyebrow, title, intro, description, actions, questionLabel, saveState, status, journeys);
root.replaceWith(readerRoot);

const DB_NAME = 'tos-public-research-v1';
const QUESTION_KEY = 'tos.public-research.question.v1';
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
let questionRestored = false;
let questionSequence = 0;
let questionWrite = Promise.resolve();
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
  if (!questionRestored && !questionTouched) return Promise.resolve(true);
  const sequence = ++questionSequence;
  const value = question.value;
  questionWrite = questionWrite.catch(() => {}).then(() => notebook.setPreference(QUESTION_KEY, value));
  return questionWrite.then(() => {
    if (sequence === questionSequence) saveState.textContent = english ? 'Saved in the shared notebook on this device.' : 'Сохранено в общем блокноте на этом устройстве.';
    return true;
  }).catch(() => {
    if (sequence === questionSequence) saveState.textContent = english ? 'Could not save. Your text remains here until you leave.' : 'Не удалось сохранить. Текст останется здесь, пока вы не уйдёте со страницы.';
    return false;
  });
}

function scheduleQuestionSave() {
  questionTouched = true;
  questionSequence += 1;
  clearTimeout(saveTimer);
  saveState.textContent = english ? 'Saving…' : 'Сохраняется…';
  saveTimer = setTimeout(() => void persistQuestion(), 250);
}

async function switchProfiles(event) {
  event.preventDefault();
  try {
    if (!(await persistQuestion())) return;
    const currentRoute = validRoute(location.hash);
    if (currentRoute) await notebook.setPreference(routeKey(profile), currentRoute);
    const next=new URL(alternate,location.href);next.searchParams.set('lang',locale);
    location.assign(next.toString());
  } catch {
    status.textContent = english ? 'Could not save this profile before switching.' : 'Не удалось сохранить состояние профиля перед переключением.';
  }
}

function setLanguage(value,{updateUrl=true}={}){
  if(!['ru','en'].includes(value)||value===locale)return;
  locale=value;english=locale==='en';strings=stringsFor();document.documentElement.lang=locale;document.title=`${strings.title} · ${english?'Tree of Sophia':'Древо Софии'}`;refreshProviderStatus();
  header.querySelector('.pr-wordmark').textContent=english?'Tree of Sophia':'Древо Софии';
  switchProfile.textContent=`${english?'Explore':'Перейти'} ${strings.other}`;
  switchProfile.href=alternate+`?lang=${locale}`;
  if(questionLabel.firstChild?.nodeType===Node.TEXT_NODE)questionLabel.firstChild.nodeValue=strings.question;
  question.placeholder=strings.placeholder;
  saveState.textContent=english?'Your question is saved on this device.':'Вопрос сохраняется в блокноте на этом устройстве.';
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
  question.value = value;
  questionTouched = true;
  workspaceView?.render();
  return persistQuestion();
}

async function openAddress(address) {
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
  return entry?.open(target) ?? null;
}

async function start() {
  if(appDisposed)return;
  try {
    const savedQuestion = await notebook.getPreference(QUESTION_KEY);
    if(appDisposed)return;
    if (!questionTouched && typeof savedQuestion === 'string') question.value = savedQuestion;
    questionRestored = true;
  } catch {
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
    onReadingLocation(address) {
      const saved = validRoute(encodeReadingRoute(address));
      if (saved) void notebook.setPreference(routeKey(profile), saved).catch(() => {
        saveState.textContent = english ? 'Could not save this reading position.' : 'Не удалось сохранить это место чтения.';
      });
    }});
  entry.opener.disabled = true;
  const narrowReader=readerRoot.querySelector('.corpus-open-narrow');if(narrowReader)narrowReader.disabled=true;
  window.tosPublicResearch = Object.freeze({profile, getQuestion: () => question.value, setQuestion, open: openAddress, reader: entry.reader, provider, notebook: entry.notebook});
  publicOwners = createPublicResearchOwners();
  const publicSpace = await createPublicSpace({profile,provider});
  if(appDisposed)return;
  spaceView = mountPublicResearchSpace({host:readerRoot,header,profile,locale:()=>locale,library:publicSpace.library,
    bindings:publicSpace.bindings,documentBindings:publicSpace.documentBindings,api:window.tosPublicResearch,
    workspace:publicOwners.workspace,question:questionLabel,saveState,status,journeys:profileInfo?.journeys??[]});
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
  agentBridge?.destroy();
  workspaceView?.destroy();
  spaceView?.destroy();
  delete window.tosPublicResearch;
  void persistQuestion().finally(() => Promise.resolve(entry?.destroy()).finally(() => notebook.close()));
}
function onPageHide(event) { if(!event.persisted)disposeApp(); }
switchProfile.addEventListener('click',onSwitchProfile);
window.addEventListener('pagehide',onPageHide);
question.addEventListener('input', scheduleQuestionSave);
if(import.meta.hot)import.meta.hot.dispose(disposeApp);

void start().catch(error => {
  const message = h('p', 'pr-error', english ? 'The reader could not start. Reload the page to try again.' : 'Не удалось запустить читатель. Перезагрузите страницу.');
  readerRoot.append(message);
});
