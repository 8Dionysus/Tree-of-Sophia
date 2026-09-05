import shell from './shell.html?raw';
import './scene.css';
import './connected.css';
import './workspace.css';
import './evidence.css';
import {mountScene} from './scene.js';
import {createTools} from './workspace.mjs';
import {createPanelHost} from './panels.mjs';
import {createEvidencePanel} from './evidence-panel.mjs';
import {evidenceRoute} from './evidence-model.mjs';
import {createPageCommandRegistry} from '../page-commands';
import {createWebMCPAdapter} from '../webmcp';
import {KnowledgeClient,focusSpec,relationSpec,localized,DEFAULT_FOCUS} from './knowledge-client.mjs';

const host=document.getElementById('app');
host.innerHTML=shell;
const root=host.firstElementChild;
let scene,tools,evidence,registry,syncing=false,lastContext='';
const client=new KnowledgeClient();
function selected(){
  if(tools?.auxiliarySelection)return tools.auxiliarySelection;
  const selection=scene?.port.selection;if(!selection)return null;
  const raw=selection.relationId?scene.port.relation(selection.relationId):scene.port.node(selection.nodeId);
  if(!raw)return null;
  return {id:raw.id,kind:selection.relationId?'edge':'node',semantic_kind:raw.kind_id||'relation',
    label:localized(raw.display.title||raw.display.label),subtitle:localized(raw.display.statement),
    from_id:raw.from_id,to_id:raw.to_id,predicate_id:raw.predicate_id,source_refs:raw.source_refs,
    authority_posture:raw.epistemic?.authority_layer,review_posture:raw.epistemic?.review_posture,
    canon_status:raw.epistemic?.canon_status,reroutable:false,evidence_available:Boolean(evidenceRoute(raw))};
}
function sync(){
  if(!scene)return;
  const selection=selected(),packet=scene.port.packet;
  const url=new URL(location.href);
  if(packet?.focus?.node_id)url.searchParams.set('focus',packet.focus.node_id);
  if(selection&&!tools?.auxiliarySelection)url.searchParams.set('selection',selection.id);else url.searchParams.delete('selection');
  history.replaceState(null,'',url);
  const key=JSON.stringify([packet?.source_revision,packet?.focus,selection,root.dataset.lens]);
  if(key!==lastContext){lastContext=key;if(!syncing)registry?.notifyStateChange();}
  tools?.selectionChanged();
  evidence?.selectionChanged();
}
const commit=action=>{syncing=true;try{return action();}finally{syncing=false;sync();}};
scene=mountScene(root,{initialFocus:new URLSearchParams(location.search).get('focus')||DEFAULT_FOCUS,onChange:()=>{tools?.clearAuxiliarySelection();sync();}});
const panels=createPanelHost(root,scene);
tools=createTools(root,scene,{selected,panels,onChange:()=>{if(!syncing)registry?.notifyStateChange();sync();}});
evidence=createEvidencePanel(root,scene,panels,{selected,onUserAction:()=>registry?.notifyStateChange()});
const handlers={
  ...tools.handlers,
  ...evidence.handlers,
  'tos.page.inspect-selection':()=>selected(),
  'tos.page.open-view':async(input,{signal})=>{
    scene.ui.cancelPending();
    if(input.mode!=='philosophy'||(input.graph_mode&&input.graph_mode!=='nodes')||!['constellations','observatory'].includes(String(input.view_id)))throw new Error('Эта линза открывается в расширенном исследовательском режиме.');
    const packet=await client.compile(focusSpec(String(input.focus_id||DEFAULT_FOCUS)),signal);
    signal.throwIfAborted();commit(()=>scene.port.setGraph(packet,{selectFocus:Boolean(input.focus_id)}));return {view_id:'observatory'};
  },
  'tos.page.select':async(input,{signal})=>{
    scene.ui.cancelPending();
    const id=String(input.item_id||'');
    if(commit(()=>tools.chooseGap(id)))return selected();
    if(scene.port.node(id)){commit(()=>scene.port.selectNode(id));return selected();}
    if(scene.port.relation(id)){commit(()=>scene.port.selectRelation(id));return selected();}
    const hit=searchHits.get(id);if(!hit)throw new Error('Выберите объект из текущей области или результатов поиска.');
    const packet=await client.compile(hit.from_id?relationSpec(hit):focusSpec(id),signal,searchRevision);
    signal.throwIfAborted();commit(()=>{scene.port.setGraph(packet,{selectFocus:!hit.from_id});if(hit.from_id)scene.port.selectRelation(id,{rememberView:false});});return selected();
  },
  'tos.page.search':async(input,{signal})=>{
    scene.ui.cancelPending();
    const query=String(input.query||'').trim().slice(0,256);
    const packet=await client.search(query,signal);signal.throwIfAborted();
    scene.openSearch();scene.ui.cancelSearch();root.querySelector('#sc-query').value=query;
    const results=root.querySelector('.sc-search-results');results.replaceChildren();searchHits.clear();searchRevision=packet.source_revision;
    for(const [kind,list]of [['node',packet.nodes],['relation',packet.relations]])for(const raw of list){searchHits.set(raw.id,raw);results.append(scene.ui.searchRow(raw,kind,packet.source_revision));}
    scene.invalidate();return {query,result_count:packet.counts.matching_nodes+packet.counts.matching_relations,
      results:[...searchHits.values()].map(raw=>({id:raw.id,label:localized(raw.display.title||raw.display.label),kind:raw.kind_id||'relation',summary:localized(raw.display.summary||raw.display.statement)}))};
  },
  'tos.page.show-neighborhood':async(input,{signal})=>{
    scene.ui.cancelPending();
    const id=selected()?.id;if(!id||selected().kind!=='node')throw new Error('Сначала выберите звезду.');
    const packet=await client.compile(focusSpec(id,{depth:Math.max(1,Math.min(3,Number(input.depth)||1))}),signal,scene.port.packet.source_revision);signal.throwIfAborted();commit(()=>scene.port.setGraph(packet,{selectFocus:true}));
    return {node:{node_id:id},neighbors:packet.nodes.map(n=>({node_id:n.id,label:localized(n.display.title)})),edges:packet.relations.map(r=>({edge_id:r.id}))};
  },
  'tos.page.clear-focus':()=>commit(()=>{scene.closeInspector(false,true);scene.overview();return {cleared:true};}),
};
const searchHits=new Map();let searchRevision=null;
// Local edits use the same owner module as the research workbench. The shell owns no ToS write API.
for(const id of Object.keys(tools.handlers)){
  const handler=handlers[id];handlers[id]=(input,execution)=>commit(()=>handler(input,execution));
}
registry=createPageCommandRegistry(()=>({mode:'philosophy',view_id:'observatory',graph_mode:'nodes',selected:selected(),
  path_start_node_id:null,active_layers:['knowledge'],active_predicates:['overview'],deep_link:location.href,research_workspace:tools.workspace.summary()}),handlers);
const allowed=new Set(['tos.page.context','tos.page.cancel',...Object.keys(handlers)]);
root.querySelector('#sc-query').addEventListener('input',()=>registry.notifyStateChange());
const webmcp=createWebMCPAdapter(registry,document,allowed);
webmcp.subscribeStatus(status=>tools.agentStatus(status));
void webmcp.start();
window.addEventListener('pagehide',()=>webmcp.stop());
window.addEventListener('pageshow',event=>{if(event.persisted)void webmcp.start();});
// Restore a selected object only after its containing snapshot is ready.
const restoreId=new URLSearchParams(location.search).get('selection');
if(restoreId){const observer=new MutationObserver(()=>{if(!scene.port.packet)return;observer.disconnect();commit(()=>{if(scene.port.node(restoreId))scene.port.selectNode(restoreId);else if(scene.port.relation(restoreId))scene.port.selectRelation(restoreId);});});observer.observe(root,{attributes:true,attributeFilter:['data-graph-revision']});}
