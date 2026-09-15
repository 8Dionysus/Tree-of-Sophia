import '../constructor/style.css';
import {mountConstructorSky} from '../constructor/sky.mjs';
import {createFixtureProvider} from '../src/corpus-reader/fixture-provider.mjs';
import {createReference} from '../src/corpus-reader/model.mjs';
import {mountCorpusEntry} from '../src/corpus-reader/host.mjs';
import {mountExplorationScope} from '../src/corpus-reader/scope.mjs';
import {checkCorpusIndexedDB} from './corpus-indexeddb-check.mjs';
import './corpus-reader-fixture.css';

const root=document.getElementById('tree'),output=document.getElementById('corpus-metrics');
root.innerHTML=`<canvas class="tree-sky" aria-hidden="true"></canvas><div class="tree-clusters"></div><div class="edge-labels"></div><div class="tree-stars"></div>
  <header class="header"><div class="brand"><span class="brand-mark">✧</span><div><b>ДРЕВО СОФИИ</b><small>Проверка чтения и пространства</small></div></div><nav class="main-tools"></nav></header>
  <div class="view-heading"><div class="view-kicker">ИСКУССТВЕННЫЕ МАТЕРИАЛЫ</div><h1>Текст и пространство</h1><p>Выберите звезду или откройте библиотеку.</p></div>
  <footer class="footer"><span>Проверочный источник</span><div class="footer-tools"><button class="text-button" id="corpus-space-reset">Обзор</button></div></footer>`;
const conceptId='fixture:graph:concept',workId='fixture:graph:work';
const nodes=[{id:workId,title:'Проверочное произведение',kind:'work',position:[-170,0,0]},
  {id:conceptId,title:'Связанный материал',kind:'concept',position:[210,80,40]}];
const edges=[{id:'fixture:graph:edge',from:workId,to:conceptId,kind:'relates',label:'Проверочный переход'}];
const provider=createFixtureProvider({longDocumentUnits:100000,graphTarget:()=>({kind:'node',id:conceptId}),languages:['de','en','ru','grc','ar']});
let entry,firstDocument;
const sky=mountConstructorSky(root,{onSelect:()=>void entry.open({documentId:firstDocument}),onMove:()=>{},onEdgeSelect:()=>void entry.open({documentId:firstDocument})});
sky.update({nodes,edges,clusters:[]},Object.fromEntries(nodes.map(node=>[node.id,node.title])));
sky.motion(false);sky.frame();
document.getElementById('corpus-space-reset').onclick=()=>sky.frame();
entry=mountCorpusEntry({root,provider,dbName:'tos-corpus-reader-fixture-v1',graphNavigate:async target=>{
  if(!nodes.some(node=>node.id===target.id))throw new Error('Проверочный адрес отсутствует в графе.');
  sky.select(target.id);root.dataset.selection=target.id;
  root.querySelector('.view-heading p').textContent='Фрагмент привёл к этому материалу. Библиотека вернёт к чтению.';
}});
const removeScope=mountExplorationScope(root,{state:()=>({view:{nodes,relations:edges,continuation:{}},model:{vertices:nodes,edges},mode:'fixture'})});
firstDocument=(await provider.catalog({limit:1})).items[0].id;
root.dataset.fixtureReady='true';
const stats=()=>({provider:provider.metrics(),renderedUnits:document.querySelectorAll('.cr-unit').length,
  retainedWindows:Object.keys(entry.reader.state().windows??{}).length,
  retainedUnits:Object.values(entry.reader.state().windows??{}).reduce((sum,page)=>sum+(page.units?.length??0),0),
  notebook:entry.notebook.status(),camera:root.dataset.camera});
document.getElementById('corpus-measure').onclick=()=>{output.textContent=JSON.stringify(stats());};
document.getElementById('corpus-network').onchange=event=>{
  provider.controls.setLatency(event.target.value==='offline'?0:Number(event.target.value));
  for(const operation of ['catalog','document','window','search','resolve','structure'])provider.controls.setFailure(operation,event.target.value==='offline');
};
document.getElementById('corpus-revision').onclick=()=>{provider.controls.bumpRevision();output.textContent='Редакция изменена. Проверьте сохранённую ссылку и следующую страницу.';};
async function runCheck(button,operation){
  button.disabled=true;output.dataset.state='running';output.textContent='Проверка выполняется…';
  try{const result=await operation();output.dataset.state=result.ok?'passed':'failed';output.textContent=JSON.stringify(result);}
  catch(error){output.dataset.state='failed';output.textContent=JSON.stringify({ok:false,error:error.message});}
  finally{button.disabled=false;}
}
document.getElementById('corpus-check-storage').onclick=event=>void runCheck(event.currentTarget,checkCorpusIndexedDB);
const until=async predicate=>{const end=performance.now()+6000;while(!predicate()){if(performance.now()>end)throw new Error('Reading transition timed out.');await new Promise(resolve=>setTimeout(resolve,20));}};
document.getElementById('corpus-check-scale').onclick=event=>void runCheck(event.currentTarget,async()=>{
  provider.controls.setLatency(0);for(const op of ['catalog','document','window','search','resolve','structure'])provider.controls.clearFailure(op);
  const manifest=await provider.document({documentId:firstDocument}),versionId=manifest.versions[0].id;
  let cursor,maxRendered=0,maxRetained=0,maxWindows=0;
  const transitionsMs=[],framesMs=[];let frameId=0,lastFrame=null;
  const trackFrame=time=>{if(lastFrame!==null&&framesMs.length<4096)framesMs.push(time-lastFrame);lastFrame=time;frameId=requestAnimationFrame(trackFrame);};
  frameId=requestAnimationFrame(trackFrame);
  const start=performance.now();
  try{
  for(let index=0;index<100;index++){
    const step=performance.now();
    const page=await provider.window({documentId:firstDocument,versionId,cursor,limit:40});cursor=page.nextCursor;
    await entry.open({documentId:firstDocument,versionId,reference:createReference({source:manifest.versions[0].source,versionId,unitId:page.units[0].id})});
    await until(()=>document.querySelector('.cr-unit')?.dataset.unitId===page.units[0].id);
    transitionsMs.push(performance.now()-step);
    const current=stats();maxRendered=Math.max(maxRendered,current.renderedUnits);maxRetained=Math.max(maxRetained,current.retainedUnits);maxWindows=Math.max(maxWindows,current.retainedWindows);
    if(current.renderedUnits>80||current.retainedUnits>80||current.retainedWindows>2)throw new Error('The two-window text bound was exceeded.');
  }
  }finally{cancelAnimationFrame(frameId);}
  const distribution=values=>{const sorted=[...values].sort((a,b)=>a-b);return {samples:sorted.length,p50:sorted[Math.floor(sorted.length*.5)]??null,p95:sorted[Math.min(sorted.length-1,Math.floor(sorted.length*.95))]??null,max:sorted.at(-1)??null};};
  return {ok:true,transitions:100,elapsedMs:Math.round(performance.now()-start),transitionMs:distribution(transitionsMs),frameMs:distribution(framesMs),maxRendered,maxRetained,maxWindows,...stats()};
});
window.addEventListener('pagehide',event=>{if(!event.persisted){removeScope();entry.destroy();sky.dispose();}});
