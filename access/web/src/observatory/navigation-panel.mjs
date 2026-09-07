import {createReadingMemory} from './reading-state.mjs';
import {KnowledgeClient,RequestSlots,RevisionError,localized} from './knowledge-client.mjs';
import {createToSQueryOperations} from '../query-operations';
import {pathAvailable,pathSearchSpec,explorationQuery,loadPaths} from './navigation-model.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const n=document.createElement(tag);n.textContent=text;n.className=className;return n;};
const button=(label,action,className='sc-nav-button')=>{const n=el('button',label,className);n.type='button';n.addEventListener('click',action);return n;};
const name=raw=>localized(raw?.display?.title||raw?.display?.label,'Выбрать звезду');

export function createNavigationPanel(root,scene,panels,{selected,commit,onUserAction}){
  const client=new KnowledgeClient(),transport=new KnowledgeClient({base:''}),requests=new RequestSlots();
  const queries=createToSQueryOperations((url,options)=>transport.request(url,options));
  let mode='neighbors',focus=null,start=null,end=null,revision=null,bookmark=null,page=null,result=null,index=0;
  let depth=2,direction='either',profile='overview',maxDepth=6,alternativeLimit=3,excluded=[],busy=false,error=null,searchTarget='end';
  let ticket=0,searchTimer=0,applying=false,focusReturn=null;
  const panel=el('section','','sc-panel sc-navigation-panel');panel.hidden=true;panel.setAttribute('aria-label','Связи и маршруты');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">АТЛАС ПЕРЕХОДОВ</span><button type="button" class="sc-icon sc-nav-close" aria-label="Закрыть маршруты"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-nav-heading"><span aria-hidden="true">⟡</span><h3>Продолжить мысль</h3></div><div class="sc-nav-tabs" role="tablist" aria-label="Способ исследования"></div><div class="sc-nav-body" id="sc-nav-body" role="tabpanel" tabindex="0"></div><div class="sc-nav-status" role="status"></div><div class="sc-nav-footer"></div>';
  root.append(panel);
  const body=panel.querySelector('.sc-nav-body'),status=panel.querySelector('.sc-nav-status'),footer=panel.querySelector('.sc-nav-footer');
  const reading=createReadingMemory(body);
  function cancel(){reading.capture();ticket++;requests.cancelAll();clearTimeout(searchTimer);busy=false;}
  panels.register('navigation',panel,cancel);
  const opener=button('',()=>{onUserAction();open();},'sc-control sc-navigation-open');opener.setAttribute('aria-label','Связи и маршруты');
  opener.innerHTML='<i data-lucide="route" aria-hidden="true"></i><span>Маршруты</span>';
  root.querySelector('.sc-header-actions').append(opener);
  function close(){panels.close('navigation');(focusReturn?.isConnected&&!focusReturn.closest('[hidden]')?focusReturn:opener).focus();}
  panel.querySelector('.sc-nav-close').addEventListener('click',()=>{onUserAction();close();});
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();onUserAction();close();}});
  const tabs=['neighbors','paths'].map((id,i)=>{
    const tab=button(i?'Маршрут':'Связи',()=>{onUserAction();cancel();mode=id;error=null;if(i&&!start&&pathAvailable(current()))start=current();render();},'');
    tab.id='sc-nav-tab-'+id;tab.setAttribute('role','tab');tab.setAttribute('aria-controls','sc-nav-body');
    tab.addEventListener('keydown',event=>{if(['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){event.preventDefault();tabs[event.key==='Home'?0:event.key==='End'?1:1-i].click();tabs[mode==='paths'?1:0].focus();}});
    panel.querySelector('.sc-nav-tabs').append(tab);return tab;
  });
  const current=()=>{const s=selected();return s?.kind==='node'?scene.port.node(s.id):null;};
  function fresh(){
    const next=scene.port.packet?.source_revision;
    if(next!==revision){cancel();revision=next;start=null;end=null;focus=null;result=null;page=null;excluded=[];bookmark=null;return true;}
    return false;
  }
  function open(tab=mode){
    fresh();if(!bookmark&&scene.port.packet)bookmark=scene.port.captureView();
    focusReturn=document.activeElement;panels.open('navigation');mode=tab;focus=current()||focus;
    if(page&&page.focus.node_id!==focus?.id)page=null;
    if(mode==='paths'&&!start&&pathAvailable(current()))start=current();
    render();tabs[mode==='paths'?1:0].focus();
  }
  function apply(packet,{command=false}={}){
    if(!bookmark)bookmark=scene.port.captureView();
    applying=true;try{if(command)commit(()=>scene.port.setGraph(packet));else scene.port.setGraph(packet);}finally{applying=false;}
  }
  async function run(work,accept,signal){
    scene.ui.cancelPending();error=null;busy=true;const id=++ticket;render();
    try{
      const reply=await requests.run('navigation',own=>work(signal?AbortSignal.any([own,signal]):own));
      signal?.throwIfAborted();if(!reply.current||id!==ticket||panel.hidden)throw new DOMException('Navigation cancelled','AbortError');
      accept(reply.value);return reply.value;
    }catch(e){if(id===ticket&&!panel.hidden){error=e.name==='AbortError'?new Error('Поиск прерван. Можно повторить.'):e;}throw e;}
    finally{if(id===ticket){busy=false;render();}}
  }
  const user=action=>()=>{onUserAction();void action().catch(()=>{});};
  function reveal(selector){const target=body.querySelector(selector);if(target&&!panel.hidden){body.scrollTop+=target.getBoundingClientRect().top-body.getBoundingClientRect().top;scene.invalidate();}}
  async function expand(continuation=false,signal){
    if(!focus)throw new Error('Сначала выберите звезду.');
    const previous=continuation?page:null,origin=focus;
    const next=await run(async own=>{
      await client.inspect('node',origin.id,own,revision,origin.content_revision);
      const next=await client.explore(previous?{cursor:previous.page.next_cursor}:explorationQuery(origin.id,{depth,direction,profile}),own,revision,previous);
      if(next.focus.node_id!==origin.id||next.nodes.find(n=>n.id===origin.id)?.content_revision!==origin.content_revision)throw new RevisionError();
      return next;
    },next=>{page=next;apply(next,{command:Boolean(signal)});},signal);
    reveal('.sc-nav-page');return next;
  }
  async function find(signal){
    if(!start||!end)throw new Error('Выберите начало и конец маршрута.');
    const found=await run(own=>loadPaths(start,end,revision,{client,queries,signal:own,direction,maxDepth,alternativeLimit,excluded}),next=>{
      result=next;index=0;if(next.paths.length)apply(next.paths[0].packet,{command:Boolean(signal)});
    },signal);
    reveal(found.found?'.sc-nav-variants':'.sc-nav-empty');return found;
  }
  function selectControl(label,value,choices,change){
    const wrap=el('label','','sc-nav-field');wrap.append(el('span',label));const select=el('select');
    for(const [id,title]of choices){const opt=el('option',title);opt.value=id;select.append(opt);}select.value=value;select.disabled=busy;
    select.addEventListener('change',()=>{onUserAction();cancel();error=null;change(select.value);render();});wrap.append(select);return wrap;
  }
  function options(isPath){
    const wrap=el('div','','sc-nav-options');
    wrap.append(selectControl('Направление',direction,[['either','В обе стороны'],['outgoing','По связям →'],['incoming','Против связей ←']],value=>{direction=value;result=null;page=null;}));
    wrap.append(selectControl('Глубина',String(isPath?maxDepth:depth),Array.from({length:isPath?8:3},(_,i)=>[String(i+1),String(i+1)+' '+(i===0?'шаг':i<4?'шага':'шагов')]),value=>{if(isPath){maxDepth=Number(value);result=null;}else{depth=Number(value);page=null;}}));
    if(isPath)wrap.append(selectControl('Варианты',String(alternativeLimit),[1,2,3,4,5].map(i=>[String(i),'До '+i]),value=>{alternativeLimit=Number(value);result=null;}));
    else wrap.append(selectControl('Содержание',profile,[['overview','Обзор связей'],['all','Все типы связей']],value=>{profile=value;page=null;}));
    return wrap;
  }
  function paragraph(text){return el('p',text,'sc-nav-note');}
  function inspect(raw,kind='node'){
    onUserAction();
    if(!(kind==='node'?scene.port.node(raw.id):scene.port.relation(raw.id))&&result?.paths[index])apply(result.paths[index].packet);
    panels.close('navigation');if(kind==='node')scene.port.selectNode(raw.id);else scene.port.selectRelation(raw.id);
  }
  function renderNeighbors(){
    body.append(el('div','ОТ ВЫБРАННОЙ ЗВЕЗДЫ','sc-nav-caption'),el('h4',focus?name(focus):'Выберите звезду в пространстве','sc-nav-origin'));
    if(!focus){body.append(paragraph('Откройте её карточку, затем «Окрестность».'));return;}
    body.append(options(false));
    const action=button(busy?'Раскрываю…':'Раскрыть связи',user(()=>expand()),'sc-nav-primary');action.disabled=busy;body.append(action);
    if(page){
      const summary=el('div','','sc-nav-page');summary.append(el('span','ОБЛАСТЬ '+String(page.page.number).padStart(2,'0'),'sc-nav-caption'),el('p',`Звёзд: ${page.nodes.length} · связей: ${page.relations.length}`));body.append(summary);
      body.append(paragraph(`За этот обход обнаружено узлов: ${page.counts.discovered_nodes}; показано связей: ${page.counts.emitted_relations}. На экране — текущая порция.`));
      if(page.status==='paused'){const more=button('Продолжить раскрытие →',user(()=>expand(true)));more.disabled=busy;body.append(more);}
      else body.append(paragraph(page.status==='complete'?'Обход завершён в выбранных пределах.':'Достигнут предел обхода. Выберите более близкий центр.'));
      const list=el('div','','sc-nav-discoveries');
      for(const id of page.page.primary_node_ids){const raw=page.nodes.find(n=>n.id===id);if(id!==focus.id)list.append(button(name(raw),()=>inspect(raw),'sc-nav-discovery'));}
      body.append(list);
    }else body.append(paragraph('Связи раскрываются небольшими областями. Сохраняются положение камеры и места уже знакомых звёзд.'));
  }
  function pick(which){searchTarget=which;result=null;error=null;render();body.querySelector('input')?.focus();}
  function endpoint(label,raw,which){
    const b=button('',()=>{onUserAction();pick(which);},'sc-nav-endpoint');b.disabled=busy;
    b.append(el('small',label),el('span',name(raw)));b.dataset.empty=String(!raw);return b;
  }
  function renderSearch(){
    const wrap=el('div','','sc-nav-search'),label=el('label',searchTarget==='start'?'Найти начало':'Найти конечную звезду');
    label.htmlFor='sc-nav-query';const input=el('input');input.id='sc-nav-query';input.type='search';input.placeholder='Имя или понятие…';input.autocomplete='off';input.disabled=busy;
    const list=el('div','','sc-nav-search-results');list.setAttribute('aria-live','polite');wrap.append(label,input,list);body.append(wrap);
    input.addEventListener('input',()=>{
      onUserAction();clearTimeout(searchTimer);requests.cancel('search');list.replaceChildren();const query=input.value.trim().slice(0,256);if(!query)return;
      const which=searchTarget;list.append(paragraph('Ищу…'));
      searchTimer=setTimeout(async()=>{
        try{
          const reply=await requests.run('search',signal=>client.compile(pathSearchSpec(query),signal,revision));
          if(!reply.current||!input.isConnected||panel.hidden)return;list.replaceChildren();
          const hits=reply.value.nodes.filter(pathAvailable);
          for(const raw of hits){const b=button('',()=>{onUserAction();cancel();if(which==='start'){start=raw;if(!end)searchTarget='end';}else end=raw;result=null;excluded=[];error=null;render();},'sc-nav-search-result');
            b.dataset.itemId=raw.id;
            b.append(el('span',name(raw)),el('small',localized(raw.display.kind_label)),el('span',localized(raw.display.summary),'sc-nav-search-context'),el('small',raw.native_id,'sc-nav-search-identity'));list.append(b);}
          list.append(paragraph(hits.length===6?'Первые 6 совпадений. Уточните название для более точного поиска.':hits.length?'Объекты философского графа':'Совпадений в философском графе нет.'));scene.invalidate();
        }catch(e){if(input.isConnected&&!panel.hidden){list.replaceChildren(paragraph(e.message));scene.invalidate();}}
      },200);
    });
  }
  function renderPaths(){
    const pair=el('div','','sc-nav-endpoints');pair.append(endpoint('01 / НАЧАЛО',start,'start'),el('span','↓','sc-nav-connector'),endpoint('02 / НАЗНАЧЕНИЕ',end,'end'));body.append(pair);
    body.append(paragraph('Поиск между объектами философского графа. Связность сама по себе не означает согласия.'));
    if(!result)renderSearch();
    body.append(options(true));
    if(excluded.length){const group=el('div','','sc-nav-exclusions');group.append(el('span','Обходим связи:','sc-nav-caption'));
      for(const raw of excluded){const b=button(name(raw)+' · вернуть',user(async()=>{excluded=excluded.filter(r=>r.id!==raw.id);await find();}));b.disabled=busy;group.append(b);}body.append(group);}
    const action=button(busy?'Ищу пути…':result?'Найти заново':'Найти пути',user(()=>find()),'sc-nav-primary');action.disabled=busy||!start||!end||start.id===end.id;body.append(action);
    if(!result)return;
    if(!result.found){body.append(el('h4','Здесь путь не найден','sc-nav-origin sc-nav-empty'),paragraph('В пределах выбранного направления, глубины и исключений. Можно изменить условия и повторить поиск.'));}
    if(result.exploration_truncated)body.append(paragraph('Поиск достиг вычислительного предела; другие пути могли остаться за ним.'));
    if(!result.found)return;
    const choices=el('div','','sc-nav-variants');choices.setAttribute('role','group');choices.setAttribute('aria-label','Варианты маршрута');
    result.paths.forEach((path,i)=>{const b=button(String(i+1).padStart(2,'0')+' · '+path.edges.length+' св.',()=>{onUserAction();index=i;apply(path.packet);render();},'sc-nav-variant');b.setAttribute('aria-pressed',String(i===index));b.disabled=busy;choices.append(b);});body.append(choices);
    body.append(paragraph(`Путь ${index+1} из ${result.path_count} · поиск до ${result.alternative_limit} вариантов.`));
    const path=result.paths[index],list=el('ol','','sc-nav-itinerary');
    path.nodes.forEach((raw,i)=>{
      const item=el('li');item.append(button(name(raw),()=>inspect(raw),'sc-nav-stop'));
      if(path.edges[i]){const edge=path.edges[i],hop=el('div','','sc-nav-hop');
        hop.append(button(name(edge),()=>inspect(edge,'relation'),'sc-nav-relation'));
        if(path.traversal[i].edge_direction==='reverse')hop.append(el('small','← против направления связи'));
        const skip=button('Обойти',user(async()=>{if(!excluded.some(r=>r.id===edge.id))excluded=[...excluded,edge];await find();}),'sc-nav-skip');skip.setAttribute('aria-label','Обойти связь: '+name(edge));skip.disabled=busy;hop.append(skip);item.append(hop);}
      list.append(item);
    });body.append(list);
  }
  function render(){
    reading.capture();reading.enter(JSON.stringify([revision,focus?.id,mode,index]));
    tabs.forEach((tab,i)=>{const active=mode===(i?'paths':'neighbors');tab.setAttribute('aria-selected',String(active));tab.tabIndex=active?0:-1;});
    body.setAttribute('aria-labelledby','sc-nav-tab-'+mode);body.setAttribute('aria-busy',String(busy));body.replaceChildren();
    if(mode==='paths')renderPaths();else renderNeighbors();
    status.textContent=error?.message||'';panel.dataset.state=busy?'loading':error?'error':'ready';
    footer.replaceChildren();
    if(bookmark){footer.append(button('↶ К исходному виду',()=>{onUserAction();const saved=bookmark;bookmark=null;panels.close('navigation');applying=true;try{scene.port.restoreView(saved);}finally{applying=false;}},'sc-nav-return'));}
    else footer.append(el('span','От звезды — к созвездию'));
    reading.restore();scene.invalidate();
  }
  async function fromCard({raw,kind,tab}){
    fresh();if(!bookmark)bookmark=scene.port.captureView();open(tab||'paths');
    if(kind==='node'){
      focus=raw;
      if(mode==='paths'&&pathAvailable(raw)){if(start&&start.id!==raw.id)end=raw;else start=raw;result=null;excluded=[];}
      else if(mode==='paths'){start=null;end=null;result=null;excluded=[];error=new Error('Для этой звезды поиск путей пока не подключён. Можно раскрыть её связи или выбрать объекты философского графа.');}
      render();return;
    }
    if(!pathAvailable(raw)){error=new Error('Поиск обходного пути пока доступен для связей философского графа.');render();return;}
    await run(async signal=>{
      const both=await Promise.all([raw.from_id,raw.to_id].map(id=>client.inspect('node',id,signal,revision)));
      return both.map(x=>x.match);
    },pair=>{[start,end]=pair;excluded=[raw];result=null;});
  }
  root.addEventListener('sophia-navigate',event=>{onUserAction();void fromCard(event.detail).catch(()=>{});});
  function selectionChanged(){
    if(applying)return;
    if(fresh()&&!panel.hidden){error=new Error('Область обновилась. Выберите звезду для нового исследования.');render();}
    // Manual navigation owns the camera. Pending reads may not replace its scene.
    if(busy){cancel();if(!panel.hidden){error=new Error('Выбор изменился. Повторите поиск из нужной звезды.');render();}}
  }
  async function pathCommand(input,{signal},reroute=false){
    if(input.constrain_to_view)throw new Error('Ограничение маршрута текущим видом пока не подключено. Доступен философский граф целиком.');
    const s=selected(),raw=reroute?scene.port.relation(s?.id):current();
    if(!raw||!pathAvailable(raw))throw new Error('Выберите объект философского графа.');
    fresh();if(!bookmark)bookmark=scene.port.captureView();open('paths');
    if(reroute){
      const pair=await Promise.all([raw.from_id,raw.to_id].map(id=>client.inspect('node',id,signal,revision)));
      signal.throwIfAborted();[start,end]=pair.map(x=>x.match);excluded=[raw];
    }else{if(!start)throw new Error('Сначала задайте начало маршрута.');end=raw;excluded=[];}
    if(input.excluded_edge_ids?.length){excluded=(await Promise.all(input.excluded_edge_ids.map(id=>client.inspect('relation',id,signal,revision)))).map(x=>x.match);}
    direction=input.direction||'outgoing';maxDepth=Number(input.max_depth)||6;alternativeLimit=Number(input.alternative_limit)||3;
    return find(signal);
  }
  panels.configure('navigation',{onResume:()=>{reading.restore();render();}});
  refreshIcons();window.addEventListener('pagehide',cancel);
  return {selectionChanged,get startId(){return start?.id||null;},handlers:{
    'tos.page.start-path':()=>{const raw=current();if(!pathAvailable(raw))throw new Error('Выберите звезду философского графа.');fresh();if(!bookmark)bookmark=scene.port.captureView();start=raw;end=null;result=null;excluded=[];open('paths');return {path_start_node_id:start.id};},
    'tos.page.find-path':(input,execution)=>pathCommand(input,execution),
    'tos.page.reroute-without-selection':(input,execution)=>pathCommand(input,execution,true),
    'tos.page.show-neighborhood':async(input,{signal})=>{fresh();focus=current();if(!focus)throw new Error('Сначала выберите звезду.');if(!bookmark)bookmark=scene.port.captureView();open('neighbors');depth=Math.max(1,Math.min(3,Number(input.depth)||1));
      const next=await expand(false,signal);return {node:{node_id:focus.id,label:name(focus)},neighbors:next.nodes.filter(n=>n.id!==focus.id).map(n=>({node_id:n.id,label:name(n)})),edges:next.relations.map(r=>({edge_id:r.id})),page:next.page,status:next.status};},
  }};
}
