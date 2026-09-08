import {mountObservatory} from '../src/observatory/app.mjs';
import {createObservatoryData} from '../src/observatory/data-services.mjs';
import {formNode,formLens} from './human-form-data.mjs';

const data=createObservatoryData({fetcher:async(url,options={})=>{
  const spec=JSON.parse(options.body||'{}'),mode=document.querySelector('#forms-mode').value,material=spec.lens_id==='sophia-observatory-material';
  if(material&&mode==='slow')await new Promise(resolve=>setTimeout(resolve,spec.language==='ru'?2000:100));
  if(material&&mode==='restricted')return {ok:false,status:403};
  const requested=spec.language||'ru',actual=['auto','original'].includes(requested)?'ru':requested;
  const nodes=[formNode('fixture:forms:a',actual),formNode('fixture:forms:b',actual)];nodes[1].display.title.ru='Второй материал';
  for(const raw of nodes){
    const selection=raw.human_form_selection;selection.requested_language=requested;
    if(requested==='auto')for(const selected of Object.values(selection.roles))selected.reason='automatic';
    if(requested==='original')for(const role of Object.keys(selection.roles))selection.roles[role]={state:'unavailable',reason:'original-role-not-declared',form:null,packet:null};
    if(mode==='states'){
      selection.roles.hover={state:'unavailable',reason:'no-ready-form',form:null,packet:null};selection.candidates.find(c=>c.role==='hover').state='stale';
      selection.roles.history={state:'ambiguous',reason:'multiple-forms',form:null,packet:null};
      selection.roles.technical={state:'over-budget',reason:'inspect-exact-form',form:selection.roles.technical.form,packet:null};
    }
    if(material&&mode==='invalid')delete selection.roles.statement.packet.context;
  }
  if(url.includes('/explore/capabilities'))return {ok:true,json:async()=>({available:false})};
  if(!url.includes('/lenses/compile'))return {ok:false,status:404};
  const selected=material?nodes.filter(node=>node.id===spec.seed?.focus_node_id):nodes;
  const packet=formLens(selected);packet.counts={nodes:selected.length,relations:0,matched_nodes:selected.length,truncated_nodes:0,truncated_relations:0};
  return {ok:true,json:async()=>packet};
}});
const {root,scene}=mountObservatory({data,initialRoute:'?focus=fixture:forms:a'});root.dataset.fixture='true';
new ResizeObserver(entries=>document.documentElement.style.setProperty('--fixture-bar-height',entries[0].target.getBoundingClientRect().height+'px')).observe(document.querySelector('.fixture-controls'));
document.querySelector('#forms-check').addEventListener('click',()=>{
  const output=document.querySelector('#forms-proof');if(!output.hidden){output.hidden=true;return;}output.hidden=false;
  const savedText=localStorage.getItem('tos-observatory-reading-v1:'+location.pathname)||'null',saved=JSON.parse(savedText);
  const shown=[...root.querySelectorAll('.sc-reader-article')].map(article=>({id:article.dataset.readingId,visible:!article.hidden,
    top:Math.round(article.querySelector('.sc-reader-body').scrollTop),language:article.querySelector('select').value}));
  output.textContent=JSON.stringify({shown,saved:saved?.entries.map(entry=>({id:entry.id,language:entry.preferred,
    positions:entry.positions.map(([key,position])=>({formBound:JSON.parse(key).length===5,top:Math.round(position.top),anchor:position.anchor?.key}))})),
    includesSourceText:savedText.includes('Текст формы')||savedText.includes('НЕ доказано')});
});
