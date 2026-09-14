// Passive, bounded diagnostics for explicit development fixtures only.
// Event Timing reports slow events (>=16 ms), not an INP score. Heap is the
// optional, coarse Chromium estimate, not retained size or leak detection.
const round=value=>Math.round(value*100)/100;
function distribution(){
  const bins=new Uint32Array(2001);let count=0,sum=0,max=0;
  return {add(value){if(!Number.isFinite(value)||value<0)return;bins[Math.min(2000,Math.ceil(value*2))]++;count++;sum+=value;max=Math.max(max,value);},
    snapshot(){let total=0,p95=null;for(let i=0;i<bins.length;i++){total+=bins[i];if(count&&total>=Math.ceil(count*.95)){p95=i===2000?'>=1000':i/2;break;}}return {count,mean_ms:count?round(sum/count):null,p95_ms:p95,max_ms:round(max)};}};
}
export function mountPerformanceProbe(root){
  const controls=document.createElement('div');controls.className='fixture-probe';
  controls.innerHTML='<button type="button" id="fixture-probe-start">Измерять 10 минут</button><button type="button" id="fixture-probe-stop" disabled>Остановить измерение</button><output id="fixture-probe-status">Измерение выключено</output><details><summary>Измерения</summary><pre id="fixture-probe-report"></pre></details>';
  document.querySelector('.fixture-controls').append(controls);
  const startButton=controls.querySelector('#fixture-probe-start'),stopButton=controls.querySelector('#fixture-probe-stop'),status=controls.querySelector('output'),output=controls.querySelector('pre');
  let run=null,raf=0,timer=0,observers=[],pendingFrames=new Set();
  function state(){return {camera:root.dataset.camera,selected:root.dataset.selected,history:Number(root.dataset.history||0),nodes:root.querySelectorAll('.sc-node').length,
    dom_elements:root.querySelectorAll('*').length,readers:[...root.querySelectorAll('.sc-reader-article')].map(article=>({id:article.dataset.readingId,snapshot:article.dataset.snapshot,top:round(article.querySelector('.sc-reader-body').scrollTop)}))};}
  function sample(){
    const heap=performance.memory?.usedJSHeapSize;
    run.samples.push({seconds:round((performance.now()-run.started)/1000),heap_bytes:Number.isFinite(heap)?heap:null,actions:run.actions,...state()});
    if(run.samples.length>121)run.samples.shift();
  }
  function publish(){
    const report={schema:'tos_ui_fixture_measurement_v1',running:!run.stopped,elapsed_seconds:round(((run.stopped||performance.now())-run.started)/1000),
      probe_version:2,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},
      visible_frames:run.frames.snapshot(),event_to_raf:run.response.snapshot(),slow_event_duration:run.events.snapshot(),slow_event_queue:run.queue.snapshot(),long_tasks:run.longTasks.snapshot(),
      supported:run.supported,actions:run.actions,trusted_events:run.trustedEvents,panel_open_to_raf:run.panelOpen.snapshot(),hidden_transitions:run.hiddenTransitions,draw_mean_ms:root.dataset.drawMs,
      state:state(),samples:run.samples};
    output.textContent=JSON.stringify(report,null,2);
    status.textContent=(run.stopped?'Завершено · ':'Измеряю · ')+Math.round(report.elapsed_seconds)+' с · '+run.actions+' событий · кадр p95 '+report.visible_frames.p95_ms+' мс';
  }
  function stop(){
    if(!run||run.stopped)return;
    run.stopped=performance.now();cancelAnimationFrame(raf);for(const id of pendingFrames)cancelAnimationFrame(id);pendingFrames.clear();clearInterval(timer);
    for(const observer of observers)observer.disconnect();observers=[];sample();publish();startButton.disabled=false;stopButton.disabled=true;
  }
  function start(){
    stop();run={started:performance.now(),stopped:0,previous:0,frames:distribution(),response:distribution(),events:distribution(),queue:distribution(),longTasks:distribution(),panelOpen:distribution(),actions:0,trustedEvents:0,hiddenTransitions:0,samples:[],supported:[]};
    const supported=PerformanceObserver.supportedEntryTypes||[];
    for(const type of ['event','longtask'])if(supported.includes(type)){
      const observer=new PerformanceObserver(list=>{for(const entry of list.getEntries()){
        if(type==='event'){if(!root.contains(entry.target))continue;run.events.add(entry.duration);run.queue.add(entry.processingStart-entry.startTime);}
        else run.longTasks.add(entry.duration);
      }});
      observer.observe(type==='event'?{type,durationThreshold:16}:{type});observers.push(observer);run.supported.push(type);
    }
    const frame=now=>{if(!document.hidden){if(run.previous)run.frames.add(now-run.previous);run.previous=now;}else run.previous=0;raf=requestAnimationFrame(frame);};
    raf=requestAnimationFrame(frame);sample();publish();startButton.disabled=true;stopButton.disabled=false;
    timer=setInterval(()=>{sample();if(performance.now()-run.started>=600000)stop();else publish();},5000);
  }
  function action(event){
    if(!run||run.stopped)return;
    run.actions++;if(event.isTrusted)run.trustedEvents++;const at=performance.now();
    const opens=event.type==='click'&&event.target.closest('.sc-reader-resume,.sc-reader-open,.sc-builder-open,.sc-active-query,.sc-search-open,.sc-lenses-open');
    // rAF's supplied timestamp precedes this callback and can even precede the
    // input listener in the same frame. Measure callback arrival on one clock.
    const id=requestAnimationFrame(()=>{pendingFrames.delete(id);if(!document.hidden){const elapsed=performance.now()-at;run.response.add(elapsed);if(opens)run.panelOpen.add(elapsed);}});pendingFrames.add(id);
  }
  for(const type of ['click','input','keydown','wheel'])root.addEventListener(type,action,{capture:true,passive:true});
  document.addEventListener('visibilitychange',()=>{if(run&&!run.stopped){run.previous=0;if(document.hidden)run.hiddenTransitions++;}});
  window.addEventListener('pagehide',stop);
  startButton.addEventListener('click',start);stopButton.addEventListener('click',stop);
}
