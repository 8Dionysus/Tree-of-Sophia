import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import shell from './shell.html?raw';
import './scene.css';
import './connected.css';
import './workspace.css';
import './evidence.css';
import './navigation.css';
import './lens.css';
import './reading.css';
import './reader.css';
import './themes.css';
import {createReaderPanel} from './reader-panel.mjs';
import {createStudio} from './studio.mjs';
import {createTravelPanel} from './travel-panel.mjs';
import {createWorkspaceCopyPanel} from './workspace-copy-panel.mjs';
import {createSettingsPanel} from './settings-panel.mjs';
import {createContextHints,describeControls} from './context-hints.mjs';
import {createSceneFeedback} from './scene-feedback.mjs';
import {mountScene} from './scene.js';
import {createTools} from './workspace.mjs';
import {createPanelHost} from './panels.mjs';
import {createEvidencePanel} from './evidence-panel.mjs';
import {evidenceRoute} from './evidence-model.mjs';
import {createNavigationPanel} from './navigation-panel.mjs';
import {createLensPanel} from './lens-panel.mjs';
import {draftForPacket,encodeDraft} from './lens-model.mjs';
import {pathAvailable} from './navigation-model.mjs';
import {createPageCommandRegistry} from '../page-commands';
import {createWebMCPAdapter} from '../webmcp';
import {focusSpec,relationSpec,localized,DEFAULT_FOCUS} from './knowledge-client.mjs';
import {createObservatoryData} from './data-services.mjs';

export function mountObservatory({host=document.getElementById('app'),data=createObservatoryData(),initialRoute=location.search}={}){
if(!host)throw new Error('Observatory mount point is missing.');
uiHTML(host, shell);
const root=host.firstElementChild;
let scene,tools,evidence,navigation,builder,studio,reader,travel,registry,syncing=false,lastContext='';
const {client}=data;
function selected(){
  if(tools?.auxiliarySelection)return tools.auxiliarySelection;
  const selection=scene?.port.selection;if(!selection)return null;
  const raw=selection.relationId?scene.port.relation(selection.relationId):scene.port.node(selection.nodeId);
  if(!raw)return null;
  return {id:raw.id,kind:selection.relationId?'edge':'node',semantic_kind:raw.kind_id||'relation',
    label:localized(raw.display.title||raw.display.label),subtitle:localized(raw.display.statement),
    from_id:raw.from_id,to_id:raw.to_id,predicate_id:raw.predicate_id,source_refs:raw.source_refs,
    authority_posture:raw.epistemic?.authority_layer,review_posture:raw.epistemic?.review_posture,
    canon_status:raw.epistemic?.canon_status,reroutable:pathAvailable(raw),path_available:pathAvailable(raw),evidence_available:Boolean(evidenceRoute(raw))};
}
function sync(){
  if(!scene)return;
  const selection=selected(),packet=scene.port.packet;
  const url=new URL(location.href);
  if(packet?.focus?.node_id)url.searchParams.set('focus',packet.focus.node_id);else if(packet)url.searchParams.delete('focus');
  if(packet){const draft=draftForPacket(packet);if(draft)url.searchParams.set('lens',encodeDraft(draft));else url.searchParams.delete('lens');}
  if(selection&&!tools?.auxiliarySelection)url.searchParams.set('selection',selection.id);else url.searchParams.delete('selection');
  history.replaceState(null,'',url);
  const key=JSON.stringify([packet?.source_revision,packet?.fingerprint,packet?.focus,selection,root.dataset.lens,navigation?.startId]);
  if(key!==lastContext){lastContext=key;if(!syncing)registry?.notifyStateChange();}
  tools?.selectionChanged();
  evidence?.selectionChanged();
  navigation?.selectionChanged();
  builder?.selectionChanged();
  studio?.selectionChanged();
  reader?.selectionChanged();
  travel?.observe();
}
const commit=action=>{syncing=true;try{return action();}finally{syncing=false;sync();}};
scene=mountScene(root,{client,autoStart:false,initialFocus:new URLSearchParams(initialRoute).get('focus')||DEFAULT_FOCUS,initialLens:new URLSearchParams(initialRoute).get('lens'),onChange:()=>{tools?.clearAuxiliarySelection();sync();}});
const panels=createPanelHost(root,scene,{onUserAction:()=>registry?.notifyStateChange()});
tools=createTools(root,scene,{data,selected,panels,onChange:()=>{if(!syncing)registry?.notifyStateChange();sync();}});
evidence=createEvidencePanel(root,scene,panels,{data,selected,onUserAction:()=>registry?.notifyStateChange()});
navigation=createNavigationPanel(root,scene,panels,{data,selected,commit,onUserAction:()=>registry?.notifyStateChange()});
builder=createLensPanel(root,scene,panels,{data,onUserAction:()=>registry?.notifyStateChange()});
const userAction=()=>registry?.notifyStateChange();
for(const [id,title,selector]of [['search',ui("Поиск"),'.sc-search-open'],['lenses',ui("Линзы"),'.sc-lenses-open'],['workspace',ui("Исследование"),'.sc-workspace-open'],['navigation',ui("Маршруты"),'.sc-navigation-open']]){const opener=root.querySelector(selector);panels.addTool(id,{title,opener,launch:()=>opener.click()});}
function selectedSource(){const selection=scene.port.selection,kind=selection.relationId?'relation':'node',raw=kind==='relation'?scene.port.relation(selection.relationId):scene.port.node(selection.nodeId);return raw?{raw,kind}:null;}
for(const [id,title,icon,event]of [['builder',ui("Конструктор линз"),'◈',null],['evidence',ui("Основания"),'✧','sophia-evidence'],['sources',ui("Источники"),'◇','sophia-sources']]){
  const opener=document.createElement('button');opener.type='button';opener.className='sc-control sc-tool-shortcut';uiAttribute(opener, 'aria-label', title);const symbol=document.createElement('b');uiText(symbol, icon);const label=document.createElement('span');uiText(label, title);uiChildren(opener, "append", symbol, label);uiChildren(root.querySelector('.sc-header-actions'), "append", opener);
  const launch=()=>{userAction();if(id==='builder')root.querySelector('.sc-builder-open').click();else{const source=selectedSource();if(source)root.dispatchEvent(new CustomEvent(event,{detail:source}));else scene.port.announce(ui("Сначала выберите звезду или связь."));}};
  uiAttribute(opener, "data-tooltip", id==='builder'?ui("Собрать собственную область по условиям."):ui("Открыть {0} выбранной звезды или связи.", [title.toLowerCase()]));
  opener.addEventListener('click',launch);panels.addTool(id,{title,opener,launch,available:()=>id==='builder'||Boolean(selectedSource())});
}
reader=createReaderPanel(root,scene,panels,{data,onUserAction:userAction});
studio=createStudio(root,scene,panels,{data,initialRoute,onUserAction:userAction});
travel=createTravelPanel(root,scene,panels,{client,onUserAction:userAction});
createWorkspaceCopyPanel(root,scene,panels,{workspace:tools.workspace,reader,travel,studio,onUserAction:userAction});
createSettingsPanel(root,scene,panels,{studio,onUserAction:userAction});
createSceneFeedback(root,scene);
describeControls(root);createContextHints(root);
const handlers={
  ...tools.handlers,
  ...evidence.handlers,
  'tos.page.inspect-selection':()=>selected(),
  'tos.page.open-view':async(input,{signal})=>{
    scene.ui.cancelPending();
    if(input.mode!=='philosophy'||(input.graph_mode&&input.graph_mode!=='nodes')||!['constellations','observatory'].includes(String(input.view_id)))throw new Error(ui("Эта линза открывается в расширенном исследовательском режиме."));
    const packet=await client.compile(focusSpec(String(input.focus_id||DEFAULT_FOCUS)),signal);
    signal.throwIfAborted();commit(()=>scene.port.setGraph(packet,{selectFocus:Boolean(input.focus_id)}));return {view_id:'observatory'};
  },
  'tos.page.select':async(input,{signal})=>{
    scene.ui.cancelPending();
    const id=String(input.item_id||'');
    if(commit(()=>tools.chooseGap(id)))return selected();
    if(scene.port.node(id)){commit(()=>scene.port.selectNode(id));return selected();}
    if(scene.port.relation(id)){commit(()=>scene.port.selectRelation(id));return selected();}
    const hit=searchHits.get(id);if(!hit)throw new Error(ui("Выберите объект из текущей области или результатов поиска."));
    const packet=await client.compile(hit.from_id?relationSpec(hit):focusSpec(id),signal,searchRevision);
    signal.throwIfAborted();commit(()=>{scene.port.setGraph(packet,{selectFocus:!hit.from_id});if(hit.from_id)scene.port.selectRelation(id,{rememberView:false});});return selected();
  },
  'tos.page.search':async(input,{signal})=>{
    scene.ui.cancelPending();
    const query=String(input.query||'').trim().slice(0,256);
    const packet=await client.search(query,signal);signal.throwIfAborted();
    scene.ensureSearch();scene.ui.cancelSearch();root.querySelector('#sc-query').value=query;
    const results=root.querySelector('.sc-search-results');uiChildren(results, "replaceChildren");searchHits.clear();searchRevision=packet.source_revision;
    for(const [kind,list]of [['node',packet.nodes],['relation',packet.relations]])for(const raw of list){searchHits.set(raw.id,raw);uiChildren(results, "append", scene.ui.searchRow(raw,kind,packet.source_revision));}
    scene.invalidate();return {query,result_count:packet.counts.matching_nodes+packet.counts.matching_relations,
      results:[...searchHits.values()].map(raw=>({id:raw.id,label:localized(raw.display.title||raw.display.label),kind:raw.kind_id||'relation',summary:localized(raw.display.summary||raw.display.statement)}))};
  },
  ...navigation.handlers,
  'tos.page.clear-focus':()=>commit(()=>{scene.closeInspector(false,true);scene.overview();return {cleared:true};}),
};
const searchHits=new Map();let searchRevision=null;
// Local edits use the same owner module as the research workbench. The shell owns no ToS write API.
for(const id of Object.keys(tools.handlers)){
  const handler=handlers[id];handlers[id]=(input,execution)=>commit(()=>handler(input,execution));
}
registry=createPageCommandRegistry(()=>{
  const draft=draftForPacket(scene.port.packet);
  return {mode:'philosophy',view_id:'observatory',graph_mode:'nodes',selected:selected(),path_start_node_id:navigation.startId,
    active_layers:draft?[...draft.sources]:['knowledge'],
    active_predicates:draft?(draft.relations?(draft.predicates.length?[...draft.predicates]:[...new Set(scene.port.packet.relations.map(r=>r.predicate_id))]):[]):['overview'],
    deep_link:location.href,research_workspace:tools.workspace.summary()};
},handlers);
const allowed=new Set(['tos.page.context','tos.page.cancel',...Object.keys(handlers)]);
root.querySelector('#sc-query').addEventListener('input',()=>registry.notifyStateChange());
const webmcp=createWebMCPAdapter(registry,document,allowed);
webmcp.subscribeStatus(status=>tools.agentStatus(status));
void webmcp.start();
window.addEventListener('pagehide',()=>webmcp.stop());
window.addEventListener('pageshow',event=>{if(event.persisted)void webmcp.start();});
// A saved local pose takes precedence only for its matching route (or home).
void studio.start().then(restored=>{
  travel.start({restored:restored&&Boolean(travel.resume(initialRoute))});
  if(restored)return;const restoreId=new URLSearchParams(initialRoute).get('selection');if(!restoreId)return;
  const restore=()=>{if(!scene.port.packet)return;observer.disconnect();commit(()=>{if(scene.port.node(restoreId))scene.port.selectNode(restoreId);else if(scene.port.relation(restoreId))scene.port.selectRelation(restoreId);});};
  const observer=new MutationObserver(restore);observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision']});restore();
});
return {root,scene};
}
