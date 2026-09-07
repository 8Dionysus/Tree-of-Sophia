// Explicit, development-only scenarios. None is ToS knowledge, a historical
// assertion, an API fixture installed into the backend, or a production entry.
import {mountObservatory} from '../src/observatory/app.mjs';
import {createObservatoryData} from '../src/observatory/data-services.mjs';
import {focusSpec} from '../src/observatory/knowledge-client.mjs';

let generation=1,mode='long';
const revision=()=>generation.toString(16).padStart(64,'0');
const localized=(ru,extra={})=>({ru,default:ru,en:null,original:null,...extra});
const paragraph='Это искусственный материал для проверки чтения. Формулировка содержит оговорку: возможно, два прочтения различаются; отсутствие свидетельства не означает отрицание. Текст не является историческим утверждением.';
function record(index){
  const summary=mode==='languages'?{default:'Texte de vérification sans langue déclarée.',ru:null,en:'This is a test passage, not a historical assertion.',original:'λόγος — δοκιμή','ar':'هذا نص اختبار للقراءة، وليس ادعاءً تاريخياً.','zh-Hant':'這是一段用於閱讀測試的文字。'}:
    localized(Array.from({length:36},(_,i)=>`Абзац ${i+1}. ${paragraph}`).join('\n\n'));
  return {id:'fixture:subject:'+index,kind_id:'work',content_revision:(generation+index+10).toString(16).padStart(64,'0'),
    native_id:'fixture-'+index,source_graph:'fixture',source_refs:['Проверочный сценарий интерфейса; не источник Древа'],
    display:{title:localized(index<2?`Материал ${index?'Б':'А'} · проверка чтения`:`Проверочный узел ${index+1}`),kind_label:localized('Проверочный материал'),
      summary,summary_state:mode==='missing'?'missing':'authored',provenance:{}},
    epistemic:{authority_layer:null,review_posture:index%2?'disputed':'unreviewed',canon_status:null,confidence:null}};
}
function graph(){
  const nodes=Array.from({length:mode==='dense'?40:6},(_,index)=>record(index));
  const relations=Array.from({length:mode==='dense'?80:5},(_,index)=>({id:'fixture:relation:'+index,from_id:nodes[index%nodes.length].id,to_id:nodes[(index+1+Math.floor(index/nodes.length))%nodes.length].id,
    content_revision:revision(),source_refs:['Проверочная связь; не историческое утверждение'],predicate_id:'fixture-related',source_graph:'fixture',native_id:'fixture-relation-'+index,
    display:{label:localized('Проверочная связь'),inverse_label:localized('Обратное чтение проверочной связи'),statement:localized('Возможно, эти материалы связаны в пределах проверочного сценария.'),
      explanation:localized(paragraph+'\n\n'+paragraph),explanation_state:'authored',provenance:{}},epistemic:{review_posture:'disputed',canon_status:null,confidence:null}}));
  return {schema:'tos_lens_result_v1',source_revision:revision(),fingerprint:revision(),authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},
    nodes,relations,focus:{node_id:nodes[0].id},counts:{nodes:nodes.length,relations:relations.length,matched_nodes:nodes.length,eligible_relations:relations.length,truncated_nodes:0,truncated_relations:0},
    inclusion:{authority:'query-execution-not-semantic-proof',nodes:Object.fromEntries(nodes.map(node=>[node.id,{kind:'query'}]))}};
}
const data=createObservatoryData({fetcher:async(url)=>{
  const connection=document.querySelector('#fixture-connection').value;
  if(connection==='slow')await new Promise(resolve=>setTimeout(resolve,2000));
  if(connection==='offline')throw new TypeError('Fixture offline');
  if(connection==='restricted')return {ok:false,status:403};
  const packet=graph();let body;
  if(url.includes('/lenses/compile'))body=packet;
  else if(url.includes('/explore/capabilities'))body={available:false};
  else if(url.includes('/nodes/')){
    const id=decodeURIComponent(url.split('/nodes/')[1].split('?')[0]),raw=packet.nodes.find(node=>node.id===id);
    body={schema:'tos_knowledge_node_packet_v1',source_revision:revision(),matches:raw?[raw]:[]};
  }else if(url.includes('/relations/')){
    const id=decodeURIComponent(url.split('/relations/')[1].split('?')[0]),raw=packet.relations.find(relation=>relation.id===id);
    body={schema:'tos_knowledge_relation_packet_v1',source_revision:revision(),matches:raw?[raw]:[],endpoints:packet.nodes.filter(node=>[raw?.from_id,raw?.to_id].includes(node.id))};
  }else if(url.includes('/search?')){
    const query=new URL('http://fixture'+url).searchParams.get('query')||'';
    const nodes=packet.nodes.filter(node=>(node.display.title.ru+' '+node.id).toLowerCase().includes(query.toLowerCase())).slice(0,6);
    body={schema:'tos_knowledge_search_v1',source_revision:revision(),nodes,relations:[],counts:{matching_nodes:nodes.length,matching_relations:0}};
  }else return {ok:false,status:404};
  return {ok:true,json:async()=>body};
}});
const {root,scene}=mountObservatory({data,initialRoute:'?focus=fixture%3Asubject%3A0'});
root.dataset.fixture='true';
new ResizeObserver(entries=>document.documentElement.style.setProperty('--fixture-bar-height',entries[0].target.getBoundingClientRect().height+'px')).observe(document.querySelector('.fixture-controls'));
async function load(){
  scene.ui.cancelPending();
  try{const packet=await data.client.compile(focusSpec('fixture:subject:0'));scene.port.setGraph(packet);root.querySelector('.sc-context h2').textContent='Проверка чтения';}
  catch(error){document.querySelector('#fixture-metrics').textContent=error.message;}
}
document.querySelector('#fixture-reload').addEventListener('click',()=>{mode=document.querySelector('#fixture-case').value;generation++;void load();});
document.querySelector('#fixture-revision').addEventListener('click',()=>{generation++;void load();});
document.querySelector('#fixture-measure').addEventListener('click',()=>{
  const output=document.querySelector('#fixture-metrics'),times=[],start=performance.now();let previous=start;
  output.textContent='Измеряю…';
  const frame=now=>{times.push(now-previous);previous=now;if(now-start<5000){requestAnimationFrame(frame);return;}
    times.shift();times.sort((a,b)=>a-b);const p95=times[Math.floor(times.length*.95)]||0;
    output.textContent=JSON.stringify({frames:times.length,p95_ms:+p95.toFixed(2),draw_ms:root.dataset.drawMs,heap_bytes:performance.memory?.usedJSHeapSize||null});};
  requestAnimationFrame(frame);
});
