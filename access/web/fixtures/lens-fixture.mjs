import {mountObservatory} from '../src/observatory/app.mjs';
import {createObservatoryData} from '../src/observatory/data-services.mjs';
import {mountPerformanceProbe} from './performance-probe.mjs';
import {lensContext,boundary} from './lens-scenarios.mjs';

let generation=1,compiles=0;
const revision=()=>generation.toString(16).padStart(64,'0');
const form=text=>({default:text,ru:text});
const names=['Альфа','Бета','Гамма','Дельта','Эпсилон','Дзета'];
const allNodes=Array.from({length:40},(_,index)=>{const name=names[index]||'Материал '+(index+1);return ({id:'fixture:node:'+index,kind_id:'fixture-material',source_graph:'philosophy',native_id:'fixture-'+index,source_refs:['Искусственные данные UI; не источник Древа'],content_revision:(index+10).toString(16).padStart(64,'0'),
  attributes:{fixture_title:name,fixture_number:index,fixture_flag:index%2===0,fixture_tags:index%2?['Б','В']:['А','Б']},
  display:{title:form(name+' · проверка линзы'),kind_label:form('Проверочный материал'),summary:form(Array.from({length:20},(_,i)=>`Абзац ${i+1}. Это искусственный материал ${name} для проверки непрерывности чтения при смене линзы. Он не является историческим утверждением.`).join('\n\n')),summary_state:'authored'},
  epistemic:{review_posture:index===5?'fixture-open':'fixture-reviewed'}});});
let nodes=allNodes.slice(0,6);
const allRelations=Array.from({length:80},(_,index)=>({id:'fixture:relation:'+index,from_id:allNodes[index%40].id,to_id:allNodes[(index%40+1+Math.floor(index/40))%40].id,predicate_id:'fixture-related',source_graph:'philosophy',native_id:'fixture-relation-'+index,content_revision:'f'.repeat(64),source_refs:['Искусственная связь UI'],
  display:{label:form('Проверочная связь '+(index+1)),statement:form('Это связь между двумя проверочными материалами.'),explanation:form('Только сценарий интерфейса.'),explanation_state:'authored'},epistemic:{review_posture:index%2?'fixture-open':'fixture-reviewed'}}));
let relations=allRelations.slice(0,5);
function catalog(){
  const mode=document.querySelector('#fixture-contract').value,context=lensContext({properties:mode!=='legacy',revision:revision()});
  if(mode==='removed')context.catalog.semantic_registries.properties=context.catalog.semantic_registries.properties.filter(p=>p.property_id!=='tos.property.fixture-number');
  return context;
}
// This small evaluator exists only to exercise UI states. Real matching remains
// owned and validated by tos_access, not by this development fixture.
function matches(item,rule,context){
  const property=rule.property_id&&context.catalog.semantic_registries.properties.find(p=>p.property_id===rule.property_id);
  if(rule.property_id&&!property)throw new Error('Unknown fixture property');
  const actual=(property?.field||rule.field).split('.').reduce((v,key)=>v?.[key],item),expected=rule.value;
  if(rule.op==='exists')return (actual!==null&&actual!==undefined)===expected;
  if(actual==null)return false;
  if(rule.op==='eq')return actual===expected||Array.isArray(actual)&&actual.includes(expected);
  if(rule.op==='neq')return !matches(item,{...rule,op:'eq'},context);
  if(rule.op==='in'){const values=Array.isArray(expected)?expected:[expected];return Array.isArray(actual)?actual.some(v=>values.includes(v)):values.includes(actual);}
  if(rule.op==='contains')return Array.isArray(actual)?(Array.isArray(expected)?expected:[expected]).every(v=>actual.includes(v)):String(actual).includes(expected);
  if(rule.op==='prefix')return String(actual).startsWith(expected);
  return ({gt:actual>expected,gte:actual>=expected,lt:actual<expected,lte:actual<=expected})[rule.op]===true;
}
function compile(spec,context){
  const accepted=relations.filter(r=>(spec.relation_query?.filters||[]).every(rule=>matches(r,rule,context)));
  const roots=spec.node_query?.enabled===false?nodes.filter(n=>n.id===spec.seed?.focus_node_id):nodes.filter(n=>
    (!spec.seed?.node_ids||spec.seed.node_ids.includes(n.id))&&(!spec.seed?.text_query||n.display.title.default.toLowerCase().includes(spec.seed.text_query.toLowerCase()))
    &&(spec.node_query?.filters||[]).every(rule=>matches(n,rule,context)));
  const selected=roots.slice(0,spec.limits?.nodes||40),causes=Object.fromEntries(selected.map(n=>[n.id,{kind:spec.node_query?.enabled===false?'focus':'selector'}]));
  let frontier=[...selected];
  if(spec.relation_query?.enabled!==false)for(let depth=0;depth<(spec.traversal?.depth||0);depth++){
    const next=[];
    for(const via of frontier)for(const relation of accepted){
      const direction=spec.traversal?.direction||'either',id=relation.from_id===via.id&&direction!=='incoming'?relation.to_id:relation.to_id===via.id&&direction!=='outgoing'?relation.from_id:null;
      const neighbor=nodes.find(n=>n.id===id);if(neighbor&&!causes[id]&&selected.length<(spec.limits?.nodes||40)){selected.push(neighbor);next.push(neighbor);causes[id]={kind:'traversal',via_node_id:via.id,via_relation_id:relation.id,depth:depth+1};}
    }
    frontier=next;
  }
  const edges=spec.relation_query?.enabled===false?[]:accepted.filter(r=>causes[r.from_id]&&causes[r.to_id]).slice(0,spec.limits?.relations??80);
  const fingerprint=(compiles+100).toString(16).padStart(64,'0');
  return {schema:'tos_lens_result_v1',source_revision:context.catalog.source_revision,fingerprint,authority_boundary:boundary,nodes:selected,relations:edges,
    focus:causes[spec.seed?.focus_node_id]?{node_id:spec.seed.focus_node_id}:null,
    counts:{nodes:selected.length,relations:edges.length,matched_nodes:roots.length,eligible_relations:edges.length,truncated_nodes:Math.max(0,roots.length-selected.length),truncated_relations:0},inclusion:{authority:'query-execution-not-semantic-proof',nodes:causes}};
}
const data=createObservatoryData({fetcher:async(url,options={})=>{
  const connection=document.querySelector('#fixture-connection').value,context=catalog();let body;
  if(url.includes('/lenses/compile')){
    compiles++;const spec=JSON.parse(options.body);document.querySelector('#fixture-metrics').textContent='Запросов: '+compiles;document.querySelector('#fixture-metrics').dataset.lastSpec=options.body;
    try{body=compile(spec,context);}catch{return {ok:false,status:400};}
  }else if(url.endsWith('/catalog'))body=context.catalog;
  else if(url.endsWith('/contracts'))body={schema:'tos_knowledge_contract_bundle_v1',authority_boundary:boundary,contracts:{lens_spec:context.schema}};
  else if(url.includes('/nodes/')){const raw=nodes.find(n=>n.id===decodeURIComponent(url.split('/nodes/')[1].split('?')[0]));body={schema:'tos_knowledge_node_packet_v1',source_revision:revision(),matches:raw?[raw]:[]};}
  else if(url.includes('/relations/')){const raw=relations.find(n=>n.id===decodeURIComponent(url.split('/relations/')[1].split('?')[0]));body={schema:'tos_knowledge_relation_packet_v1',source_revision:revision(),matches:raw?[raw]:[],endpoints:nodes.filter(n=>[raw?.from_id,raw?.to_id].includes(n.id))};}
  else if(url.includes('/explore/capabilities'))body={available:false};
  else if(url.includes('/search?')){const params=new URL('http://fixture'+url).searchParams,query=(params.get('query')||'').toLowerCase(),offset=Number(params.get('offset')||0),found=nodes.filter(n=>n.display.title.default.toLowerCase().includes(query));body={schema:'tos_knowledge_search_v1',source_revision:revision(),nodes:found.slice(offset,offset+6),relations:[],counts:{matching_nodes:found.length,matching_relations:0}};}
  else return {ok:false,status:404};
  if(connection==='slow')await new Promise(resolve=>setTimeout(resolve,10000));
  if(connection==='offline')throw new TypeError('Fixture offline');
  if(connection==='restricted')return {ok:false,status:403};
  return {ok:true,json:async()=>structuredClone(body)};
}});
const {root}=mountObservatory({data,initialRoute:location.search||'?focus=fixture%3Anode%3A0'});root.dataset.fixture='true';
new ResizeObserver(entries=>document.documentElement.style.setProperty('--fixture-bar-height',entries[0].target.getBoundingClientRect().height+'px')).observe(document.querySelector('.fixture-controls'));
document.querySelector('#fixture-contract').addEventListener('change',()=>{generation++;});
document.querySelector('#fixture-revision').addEventListener('click',()=>{generation++;document.querySelector('#fixture-metrics').textContent='Новый снимок: '+generation;});

document.querySelector('#fixture-size').addEventListener('change',event=>{const dense=event.target.value==='40';nodes=dense?allNodes:allNodes.slice(0,6);relations=dense?allRelations:allRelations.slice(0,5);generation++;});
mountPerformanceProbe(root);
