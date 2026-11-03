// The learning mode as exploration (metroidbrainia): the core's constellation is
// known only as far as the person has opened it. A star at the threshold carries
// the book's own question; it opens when the person makes the book's link at its
// exact place. Help levels change the trial, never the material. A connected agent
// receives every trial and reading through the existing cognitive session.
import Graph from 'graphology';
import forceAtlas2 from 'graphology-layout-forceatlas2';
import {copyLocale,localeText,localeValue,resolveLocale} from './locale.mjs';
import {chapterTitle} from './fragments.mjs';
import './knowing.css';

const SVG='http://www.w3.org/2000/svg';
const NODE_KINDS=new Set(['figure','tos.entity.conception','tos.entity.thought-image','conception']);
const LEVELS=['light','medium','heavy'];
const STARTS=[
  {id:'bestowing-virtue',ru:'Дар и благословение',en:'Gift and blessing'},
  {id:'gateway',ru:'Ворота «Мгновение»',en:'The gateway “Moment”'},
  {id:'lion',ru:'Три превращения',en:'The three metamorphoses'},
  {id:'soothsayer',ru:'Голоса против',en:'Voices against'},
];
const hash=text=>{let h=2166136261;for(const c of String(text)){h^=c.codePointAt(0);h=Math.imul(h,16777619);}return h>>>0;};
const shuffled=(items,seed)=>items.map((item,i)=>[hash(`${seed}:${i}:${JSON.stringify(item)}`),item]).sort((a,b)=>a[0]-b[0]).map(pair=>pair[1]);

export function mountKnowing({root,index,locale,notebook,profile,fragments,onOpenReference,onOpenSpace,getSession}){
  const key=`tos.public-research.knowing.v1.${profile}`;
  const t=(ru,en,values=[])=>copyLocale(locale)==='en'?localeText(en,locale(),values):ru.replace(/\{(\d+)\}/g,(m,i)=>values[i]??m);
  const tr=value=>localeValue(value,locale());
  const el=(tag,cls='',text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const svg=(tag,attrs={})=>{const node=document.createElementNS(SVG,tag);for(const [k,v]of Object.entries(attrs))node.setAttribute(k,v);return node;};
  const btn=(text,run,cls='k-button')=>{const node=el('button',cls,text);node.type='button';node.addEventListener('click',run);return node;};

  // ── Material: core subjects and the links the stand admitted ──
  const nodes=new Map(),links=[],byNode=new Map();
  for(const item of index.items.values())if(item.owner==='bonsai/core'&&NODE_KINDS.has(item.kind)&&item.record?.title)nodes.set(item.id,item);
  for(const item of index.items.values()){
    const r=item.record;if(item.kind!=='relation'||r?.admission!=='map'||!nodes.has(r.from)||!nodes.has(r.to)||r.from===r.to)continue;
    links.push(item);for(const end of [r.from,r.to]){if(!byNode.has(end))byNode.set(end,[]);byNode.get(end).push(item);}
  }
  for(const id of [...nodes.keys()])if(!byNode.has(id))nodes.delete(id);
  const title=id=>tr(nodes.get(id)?.title)||id;
  const kindWord=id=>{const k=nodes.get(id)?.kind;return k==='figure'?t('Фигура','Figure'):k==='tos.entity.thought-image'?t('Образ','Image'):t('Трактовка','Interpretation');};
  const other=(link,id)=>link.record.from===id?link.record.to:link.record.from;
  const placeOf=link=>{const r=link.record,cite=r.grounds?.[0]??r.cite;return Array.isArray(cite)?cite[0]:cite;};
  let chapters=new Map();
  void import('../../../../bonsai/zarathustra/witnesses.json').then(module=>{chapters=new Map((module.default??module).chapters.map(item=>[item.key,item]));if(!disposed)paint();}).catch(()=>{});
  const chapterName=key=>{const chapter=chapters.get(key),language=resolveLocale(locale());return chapter?chapterTitle(chapter,['ru','de'].includes(language)?language:'en')||key:key;};
  // A rumor names where it is heard; the link's own question joins it when it differs from its star's question.
  const rumorOf=(link,from)=>{const own=tr(link.record.question),general=tr(nodes.get(from)?.record?.question),place=placeOf(link);const target=other(link,from);return {head:`→ ${kindWord(target).toLocaleLowerCase()} ?${place?.c?` · ${chapterName(place.c)}`:''}`,question:own&&own!==general?own:''};};

  // ── Layout: one deterministic constellation of the whole core ──
  const graph=new Graph({type:'undirected',multi:false});
  [...nodes.keys()].sort().forEach((id,i)=>{const a=i*2.399963229728653,r=10*Math.sqrt(i+1);graph.addNode(id,{x:Math.cos(a)*r,y:Math.sin(a)*r,size:1});});
  for(const link of links){const {from,to}=link.record;if(!graph.hasEdge(from,to))graph.addEdge(from,to);}
  forceAtlas2.assign(graph,{iterations:500,settings:{gravity:1.2,scalingRatio:12,strongGravityMode:true,slowDown:2,barnesHutOptimize:false,linLogMode:false}});
  const W=1000,H=720,pad=60,pos=new Map();
  {const xs=graph.mapNodes((n,a)=>a.x),ys=graph.mapNodes((n,a)=>a.y),[x0,x1,y0,y1]=[Math.min(...xs),Math.max(...xs),Math.min(...ys),Math.max(...ys)];
    const s=Math.min((W-2*pad)/((x1-x0)||1),(H-2*pad)/((y1-y0)||1));graph.forEachNode((n,a)=>pos.set(n,[pad+(a.x-x0)*s+((W-2*pad)-(x1-x0)*s)/2,pad+(a.y-y0)*s+((H-2*pad)-(y1-y0)*s)/2]));}

  // ── State: what this person has opened ──
  let state={level:'light',start:null,known:[],found:[],hypotheses:[]},expected,anchor=null,trial=null,focus=null,view={x:0,y:0,w:W,h:H},disposed=false,saving=Promise.resolve();
  const known=()=>new Set(state.known);
  const frontierLinks=()=>{const k=known();return links.filter(link=>k.has(link.record.from)!==k.has(link.record.to));};
  const frontier=()=>{const k=known(),out=new Set();for(const link of frontierLinks())out.add(k.has(link.record.from)?link.record.to:link.record.from);return out;};
  const foundLinks=()=>new Set(state.found.map(item=>item.link));
  const clean=value=>({level:LEVELS.includes(value?.level)?value.level:'light',start:nodes.has(value?.start)?value.start:null,known:Array.isArray(value?.known)?value.known.filter(id=>nodes.has(id)):[],found:Array.isArray(value?.found)?value.found.filter(item=>links.some(link=>link.id===item?.link)):[],hypotheses:Array.isArray(value?.hypotheses)?value.hypotheses.slice(-200):[]});
  function save(){
    const value=structuredClone(state),base=clean(expected);
    const mergeList=(before,local,remote,key)=>{
      const localKeys=new Set(local.map(key)),removed=new Set(before.map(key).filter(id=>!localKeys.has(id)));
      const result=remote.filter(item=>!removed.has(key(item))),seen=new Set(result.map(key));
      for(const item of local)if(!seen.has(key(item))){result.push(item);seen.add(key(item));}
      return result;
    };
    saving=saving.catch(()=>{}).then(async()=>{
      for(let attempt=0;attempt<3;attempt++){
        const current=await notebook.getPreference(key),remote=clean(current);
        const merged={level:value.level!==base.level?value.level:remote.level,start:value.start!==base.start?value.start:remote.start,
          known:mergeList(base.known,value.known,remote.known,x=>x),found:mergeList(base.found,value.found,remote.found,x=>x.link),hypotheses:mergeList(base.hypotheses,value.hypotheses,remote.hypotheses,x=>JSON.stringify(x))};
        try{await notebook.setPreference(key,merged,{expectedValue:current});expected=merged;if(JSON.stringify(state)===JSON.stringify(value)){state=merged;paint();}return;}
        catch(error){if(error?.code!=='conflict'||attempt===2)throw error;}
      }
    }).catch(error=>{hint.textContent=t('Не удалось сохранить познание: ','Could not save learning: ')+error.message;});
  }
  // The interlocutor records an event or answers null when it is paused or closed; trace events go without waiting.
  const record=(type,detail)=>{try{return Promise.resolve(getSession?.()?.record(type,detail)??null);}catch(error){return Promise.reject(error);}};
  const send=(type,detail)=>{void record(type,detail).catch(()=>{});};

  // ── DOM ──
  const stage=el('div','k-stage'),sky=svg('svg',{class:'k-sky',role:'group'}),dust=svg('g',{class:'k-dust'}),edgeLayer=svg('g',{class:'k-edges'}),starLayer=svg('g',{class:'k-stars'});
  const defs=svg('defs');defs.innerHTML='<radialGradient id="k-glow"><stop offset="0" stop-color="#f3dca4" stop-opacity=".55"/><stop offset="1" stop-color="#f3dca4" stop-opacity="0"/></radialGradient><radialGradient id="k-rumor"><stop offset="0" stop-color="#8fb4dc" stop-opacity=".35"/><stop offset="1" stop-color="#8fb4dc" stop-opacity="0"/></radialGradient>';
  sky.append(defs,dust,edgeLayer,starLayer);
  for(let i=0;i<260;i++){const h=hash(`dust${i}`);dust.append(svg('circle',{cx:(h%10007)/10007*W*1.6-W*.3,cy:((h>>>8)%9973)/9973*H*1.6-H*.3,r:(h%3)*.3+.25,opacity:((h>>>4)%40)/100+.08}));}
  const tools=el('div','k-tools'),trialBox=el('section','k-trial'),hint=el('p','k-hint');trialBox.hidden=true;trialBox.setAttribute('aria-live','polite');
  tools.append(btn('+',()=>zoom(.8),'k-tool'),btn('−',()=>zoom(1.25),'k-tool'),btn('⌖',()=>frame(),'k-tool'));
  stage.append(sky,tools,hint,trialBox);
  const log=el('aside','k-log'),agentBox=el('details','k-agent'),agentTitle=el('summary'),agentRoot=el('div','k-agent-body');agentBox.append(agentTitle,agentRoot);
  root.classList.add('k-root');root.append(stage,log);

  // ── Drawing ──
  const stars=new Map(),edges=new Map();
  for(const [id,[x,y]] of pos){
    const g=svg('g',{class:'k-star','data-id':id,transform:`translate(${x} ${y})`,tabindex:'0',role:'button'});
    g.append(svg('circle',{class:'k-halo',r:'18'}),svg('circle',{class:'k-core',r:'3.4'}),svg('text',{class:'k-label','text-anchor':'middle'}));
    g.addEventListener('click',event=>{event.stopPropagation();choose(id);});
    g.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();choose(id);}});
    starLayer.append(g);stars.set(id,g);
  }
  for(const link of links){
    const [x1,y1]=pos.get(link.record.from),[x2,y2]=pos.get(link.record.to),line=svg('line',{class:'k-edge',x1,y1,x2,y2});
    const label=svg('title');line.append(label);line.addEventListener('click',event=>{event.stopPropagation();if(foundLinks().has(link.id))show(link);});
    edgeLayer.append(line);edges.set(link.id,{line,label,link});
  }
  // A known star's name moves under it when a star to be chosen stands where its name would be; names keep their size
  // on screen, so this is weighed again whenever the view zooms.
  let marks={known:new Set(),marked:[]};
  function placeNames(){
    const cw=sky.clientWidth,ch=sky.clientHeight,kz=cw&&ch?Math.max(view.w/cw,view.h/ch):1;
    for(const [id,g] of stars){
      const at=pos.get(id),half=(title(id).length*3.8+12)*kz;
      g.classList.toggle('is-label-below',marks.known.has(id)&&marks.marked.some(([x,y])=>Math.abs(x-at[0])<half&&y<at[1]+2*kz&&y>at[1]-34*kz));
    }
  }
  function paint(journal=true){
    const k=known(),f=frontier(),found=foundLinks(),heavy=state.level==='heavy',candidates=new Set(trial?.kind==='verb'?[trial.target]:trial?.candidates??[]);
    for(const [id,g] of stars){
      const isKnown=k.has(id),isFrontier=!isKnown&&f.has(id);
      g.classList.toggle('is-known',isKnown);g.classList.toggle('is-frontier',isFrontier&&!heavy);g.classList.toggle('is-dark',!isKnown&&(!isFrontier||heavy));
      g.classList.toggle('is-anchor',anchor===id);g.classList.toggle('is-focus',focus===id);g.classList.toggle('is-candidate',candidates.has(id));
      const label=g.querySelector('.k-label'),number=trial?.kind==='target'?trial.candidates.indexOf(id):-1;label.textContent=isKnown?title(id):number>=0?String(number+1):isFrontier&&!heavy?'?':'';
      g.setAttribute('aria-label',isKnown?`${kindWord(id)}: ${title(id)}`:isFrontier&&!heavy?t('Звезда на пороге: откройте её связью','A star at the threshold: open it with a link'):t('Неоткрытая звезда','Unopened star'));
    }
    marks={known:k,marked:[...candidates,...(heavy?[]:f)].filter(id=>!k.has(id)).map(id=>pos.get(id)).filter(Boolean)};placeNames();
    for(const {line,label,link} of edges.values()){
      const {from,to}=link.record,a=k.has(from),b=k.has(to),isFound=found.has(link.id)||(a&&b);
      line.classList.toggle('is-known',isFound);line.classList.toggle('is-rumor',!isFound&&a!==b&&!heavy);line.classList.toggle('is-hidden',!isFound&&!(a!==b&&!heavy));
      line.classList.toggle('is-current',trial?.link?.id===link.id&&trial.kind!=='target');
      label.textContent=isFound?`${title(from)} — ${tr(link.record.verb)} → ${title(to)}`:'';
    }
    if(!state.known.length&&!trial&&!trialBox.classList.contains('is-start'))startPanel();
    hint.textContent=!state.known.length?'':state.level==='heavy'?t('Выберите открытую звезду, затем любую тёмную — и скажите, что их связывает.','Choose an opened star, then any dark one, and say what joins them.'):state.level==='medium'?t('Звёзды на пороге отмечены «?». Откройте их местом книги.','Stars at the threshold are marked “?”. Open them through a place in the book.'):t('Нажмите «?» рядом с открытой звездой.','Click a “?” next to an opened star.');
    if(journal)renderLog();
  }

  // ── Camera ──
  const applyView=()=>{sky.setAttribute('viewBox',`${view.x} ${view.y} ${view.w} ${view.h}`);const cw=sky.clientWidth,ch=sky.clientHeight;sky.style.setProperty('--kz',(cw&&ch?Math.max(view.w/cw,view.h/ch):view.w/W).toFixed(4));placeNames();};
  function zoom(f,cx=view.x+view.w/2,cy=view.y+view.h/2){const w=Math.min(W*1.6,Math.max(160,view.w*f)),h=w*H/W;view={x:cx-(cx-view.x)*w/view.w,y:cy-(cy-view.y)*h/view.h,w,h};applyView();}
  function glide(target){const from={...view},start=performance.now();const step=now=>{const p=Math.min(1,(now-start)/450),e=1-(1-p)**3;view={x:from.x+(target.x-from.x)*e,y:from.y+(target.y-from.y)*e,w:from.w+(target.w-from.w)*e,h:from.h+(target.h-from.h)*e};applyView();if(p<1&&!disposed)requestAnimationFrame(step);};requestAnimationFrame(step);}
  function frame(ids=state.known.length?[...known(),...frontier()]:[...pos.keys()]){
    const pts=ids.map(id=>pos.get(id)).filter(Boolean);if(!pts.length)return;
    let [x0,x1,y0,y1]=[Math.min(...pts.map(p=>p[0])),Math.max(...pts.map(p=>p[0])),Math.min(...pts.map(p=>p[1])),Math.max(...pts.map(p=>p[1]))];
    const w=Math.max(260,(x1-x0)*1.5,(y1-y0)*1.5*W/H),h=w*H/W;glide({x:(x0+x1)/2-w/2,y:(y0+y1)/2-h/2,w,h});
  }
  // While a trial is open the stars it concerns sit above the panel.
  function frameAbove(ids){
    const pts=ids.map(id=>pos.get(id)).filter(Boolean);if(!pts.length)return;
    const [x0,x1,y0,y1]=[Math.min(...pts.map(p=>p[0])),Math.max(...pts.map(p=>p[0])),Math.min(...pts.map(p=>p[1])),Math.max(...pts.map(p=>p[1]))];
    const w=Math.max(view.w*.85,320,(x1-x0)*1.6,(y1-y0)*3.8*W/H),h=w*H/W;glide({x:(x0+x1)/2-w/2,y:Math.min(y0-h*.1,y1-h*.4),w,h});
  }
  applyView();new ResizeObserver(()=>applyView()).observe(stage);
  sky.addEventListener('wheel',event=>{event.preventDefault();const r=sky.getBoundingClientRect();zoom(event.deltaY>0?1.12:.89,view.x+(event.clientX-r.left)/r.width*view.w,view.y+(event.clientY-r.top)/r.height*view.h);},{passive:false});
  let drag=null;
  sky.addEventListener('pointerdown',event=>{if(event.target.closest('.k-star'))return;drag={x:event.clientX,y:event.clientY,view:{...view},moved:false};sky.setPointerCapture(event.pointerId);});
  sky.addEventListener('pointermove',event=>{if(!drag)return;const r=sky.getBoundingClientRect(),dx=(event.clientX-drag.x)/r.width*view.w,dy=(event.clientY-drag.y)/r.height*view.h;if(Math.abs(dx)+Math.abs(dy)>2)drag.moved=true;view={...view,x:drag.view.x-dx,y:drag.view.y-dy};applyView();});
  sky.addEventListener('pointerup',()=>{if(drag&&!drag.moved){anchor=null;if(trial?.kind!=='target')closeTrial();paint();}drag=null;});

  // ── Choosing a star ──
  function choose(id){
    const k=known();
    if(trial?.kind==='target'&&trial.candidates.includes(id)){answerTarget(id);return;}
    if(!state.known.length){begin(id);return;}
    if(k.has(id)){focus=id;anchor=id;closeTrial();show(id);paint();return;}
    if(state.level==='heavy'){if(anchor&&k.has(anchor))openReading(anchor,id);else{hint.textContent=t('Сначала выберите открытую звезду — от неё вы протянете связь.','First choose an opened star; your link will start there.');}return;}
    const candidates=frontierLinks().filter(link=>(link.record.from===id||link.record.to===id)&&k.has(other(link,id)));
    if(!candidates.length){hint.textContent=t('Эта звезда пока далеко: откройте сначала её соседей.','This star is still far away: open its neighbours first.');return;}
    const link=anchor?candidates.find(item=>other(item,id)===anchor)??candidates[0]:candidates[0];
    openTrial(link,id);
  }
  function begin(id){closeTrial();state={...state,start:id,known:[id]};focus=id;anchor=id;save();send('trial',{id,text:t('Начало исследования: {0}.','Beginning of the inquiry: {0}.',[title(id)])});paint();frame();show(id);}

  // ── Trials ──
  // The two other verbs come from the same stars first, so the choice is about this place, never about grammar.
  const verbOptions=link=>{
    const verb=tr(link.record.verb),near=[...(byNode.get(link.record.from)??[]),...(byNode.get(link.record.to)??[])].map(item=>tr(item.record.verb));
    const pool=[...new Set([...near,...links.map(item=>tr(item.record.verb))])].filter(v=>v&&v!==verb);
    const pick=[...new Set(near)].filter(v=>v&&v!==verb).sort((a,b)=>hash(link.id+a)-hash(link.id+b)).slice(0,2);
    for(const v of pool.sort((a,b)=>hash(link.id+a)-hash(link.id+b)))if(pick.length<2&&!pick.includes(v))pick.push(v);
    return shuffled([verb,...pick],link.id);
  };
  function openTrial(link,target){
    const origin=other(link,target),medium=state.level==='medium';
    const nearby=[...new Set((byNode.get(origin)??[]).map(item=>other(item,origin)).filter(id=>id!==target&&!known().has(id)))];
    trial={link,target,origin,kind:medium?'target':'verb',candidates:medium?shuffled([target,...nearby.sort((a,b)=>hash(a+link.id)-hash(b+link.id)).slice(0,2)],link.id):[],wrong:new Set(),hinted:false};
    renderTrial();paint();frameAbove([origin,...(medium?trial.candidates:[target])]);
  }
  function closeTrial(){trial=null;trialBox.hidden=true;trialBox.classList.remove('is-start');trialBox.replaceChildren();}
  // The dark sky opens with its entrances: each lights its star while chosen.
  function startPanel(){
    trialBox.replaceChildren();trialBox.hidden=false;trialBox.classList.add('is-start');
    trialBox.append(el('p','k-found',t('С чего начать','Where to begin')),el('p','k-ask',t('Небо ещё тёмное. Выберите вход — остальное откроется вашими связями.','The sky is still dark. Choose an entrance; the rest opens through your links.')));
    const choices=el('div','k-choices');
    for(const item of STARTS.filter(item=>nodes.has(item.id))){const b=btn(t(item.ru,item.en),()=>begin(item.id),'k-choice');b.title=title(item.id);b.addEventListener('mouseenter',()=>{focus=item.id;paint();});b.addEventListener('focus',()=>{focus=item.id;paint();});choices.append(b);}
    trialBox.append(choices);
  }
  function trialHead(){
    const {link,origin,target,kind}=trial,head=el('div','k-trial-head');
    const label=id=>kind==='target'&&id===target?'?':title(id);
    head.append(el('span','k-chip',kindWord(origin)),el('strong','',label(link.record.from)),el('span','k-arrow','→'),el('strong','',label(link.record.to)));
    const q=tr(link.record.question);trialBox.append(head);if(q)trialBox.append(el('p','k-question',q));
    const cite=placeOf(link);if(cite)trialBox.append(fragments.fragment(cite));
  }
  function renderTrial(){
    trialBox.replaceChildren();trialBox.hidden=false;trialHead();
    const {kind,link}=trial,choices=el('div','k-choices');
    if(kind==='target'){
      trialBox.append(el('p','k-ask',t('Куда ведёт это место? Выберите звезду на карте.','Where does this place lead? Choose a star on the map.')));
      for(const id of trial.candidates){const b=btn(state.known.includes(id)?title(id):t('Звезда {0}','Star {0}',[trial.candidates.indexOf(id)+1]),()=>answerTarget(id),'k-choice');b.disabled=trial.wrong.has(id);b.addEventListener('mouseenter',()=>{focus=id;paint();});choices.append(b);}
    }else{
      trialBox.append(el('p','k-ask',t('Каким глаголом книга связывает их?','With what verb does the book join them?')));
      for(const verb of verbOptions(link)){const b=btn(verb,()=>answerVerb(verb),'k-choice');b.disabled=trial.wrong.has(verb);choices.append(b);}
    }
    trialBox.append(choices);
    const row=el('div','k-trial-row');row.append(btn(t('Показать ответ книги','Show the book’s answer'),()=>{trial.hinted=true;discover(trial.link,{hinted:true});}),btn(t('Отложить','Later'),()=>{closeTrial();paint();}));trialBox.append(row);
  }
  function answerTarget(id){
    if(id!==trial.target){trial.wrong.add(id);send('trial',{id:trial.link.id,text:t('Неверный адресат: {0} вместо скрытой звезды; место {1}.','Wrong destination: {0} instead of the hidden star; place {1}.',[title(id),placeOf(trial.link)?.c??''])});flash(t('Это место ведёт не туда. Перечитайте его.','This place does not lead there. Read it again.'));renderTrial();return;}
    trial.kind='verb';renderTrial();paint();
  }
  function answerVerb(verb){
    if(verb!==tr(trial.link.record.verb)){trial.wrong.add(verb);flash(t('Книга говорит иначе. Перечитайте место.','The book says otherwise. Read the place again.'));renderTrial();return;}
    discover(trial.link,{hinted:trial.hinted||trial.wrong.size>0});
  }
  function discover(link,{hinted=false,reading=''}={}){
    const {from,to}=link.record,target=known().has(from)?to:from;
    if(!state.known.includes(target))state.known=[...state.known,target];
    if(!state.found.some(item=>item.link===link.id))state.found=[...state.found,{link:link.id,level:state.level,hinted,at:new Date().toISOString(),...(reading?{reading}:{})}];
    save();focus=target;anchor=target;
    // The reader's own reading goes to the interlocutor with the opened link.
    send('trial',{id:link.id,text:t('Открыта связь: {0} — {1} → {2} ({3}). Уровень: {4}{5}.','Link opened: {0} — {1} → {2} ({3}). Level: {4}{5}.',[title(from),tr(link.record.verb),title(to),placeOf(link)?.c??'',state.level,hinted?t(', с подсказкой',', with a hint'):''])+(reading?` ${t('Прочтение читателя: {0}','The reader’s reading: {0}',[reading])}`:'')});
    revealed(link,reading);paint();shown=target;renderLog();const [x,y]=pos.get(target);glide({x:x-view.w/2,y:y-view.h*.36,w:view.w,h:view.h});
  }
  function revealed(link,reading){
    trial={link,kind:'revealed',candidates:[]};trialBox.replaceChildren();trialBox.hidden=false;
    const head=el('div','k-trial-head');head.append(el('strong','',title(link.record.from)),el('span','k-verb',tr(link.record.verb)),el('strong','',title(link.record.to)));trialBox.append(el('p','k-found',t('Открыто','Opened')),head);
    if(reading){const mine=el('div','k-mine');mine.append(el('small','',t('Ваше прочтение','Your reading')),el('p','',reading));trialBox.append(mine);}
    const sense=tr(link.record.sense);if(sense)trialBox.append(el('p','k-sense',sense));
    const counter=link.record.assessment?.counter_reading;if(tr(counter)){const c=el('details','k-counter');c.append(el('summary','',t('Сильное другое прочтение','A strong other reading')),el('p','',tr(counter)));for(const cite of counter.grounds??[])c.append(fragments.fragment(cite));trialBox.append(c);}
    const own=el('form','k-own'),area=el('textarea');area.rows=2;area.maxLength=2000;area.placeholder=t('Как вы сами скажете эту связь? Собеседник ответит.','How would you say this link yourself? Your interlocutor will reply.');
    // The reader's words leave the field only once the interlocutor has kept them.
    const kept=t('Собеседник сейчас не принимает мысли: он на паузе или не подключён. Текст остался в поле.','The interlocutor is not taking thoughts now: it is paused or not connected. Your text remains in the field.');
    const submit=btn(t('Отправить собеседнику','Send to the interlocutor'),()=>{
      const text=area.value.trim();if(!text||submit.disabled)return;submit.disabled=true;
      void record('thought',{id:link.id,text}).then(event=>{
        if(!event){flash(kept);return;}
        if(area.value.trim()===text)area.value='';flash(t('Отправлено. Ответ появится в разделе «Собеседник».','Sent. The reply will appear under “Interlocutor”.'));
      },error=>flash(`${t('Не отправлено; текст остался в поле.','Not sent; your text remains in the field.')} ${error?.message??''}`.trim())).finally(()=>{submit.disabled=false;});
    });
    own.append(area,submit);
    own.addEventListener('submit',event=>event.preventDefault());trialBox.append(own);
    const row=el('div','k-trial-row');row.append(btn(t('Дальше','Continue'),()=>{closeTrial();paint();}),btn(t('Открыть связь целиком','Open the whole link'),()=>onOpenReference?.(link.id)));trialBox.append(row);
  }
  function openReading(from,to){
    trial={kind:'reading',origin:from,target:to,candidates:[]};trialBox.replaceChildren();trialBox.hidden=false;
    const head=el('div','k-trial-head');head.append(el('strong','',title(from)),el('span','k-arrow','→'),el('strong','',t('тёмная звезда','a dark star')));trialBox.append(head,el('p','k-ask',t('Что, по-вашему, их связывает? Назовите глагол и место, если помните.','What do you think joins them? Name the verb and the place if you remember it.')));
    const form=el('form','k-own'),area=el('textarea');area.rows=3;area.maxLength=2000;form.append(area,btn(t('Сверить с книгой','Check against the book'),()=>check()));form.addEventListener('submit',event=>{event.preventDefault();check();});trialBox.append(form);
    const row=el('div','k-trial-row');row.append(btn(t('Отложить','Later'),()=>{closeTrial();paint();}));trialBox.append(row);area.focus();
    function check(){
      const text=area.value.trim();if(!text){area.focus();return;}
      const match=(byNode.get(from)??[]).find(link=>other(link,from)===to);
      if(match){discover(match,{reading:text});return;}
      state.hypotheses=[...state.hypotheses,{from,to,text,at:new Date().toISOString()}];save();renderLog();
      send('thought',{id:from,text:t('Гипотеза связи: {0} → {1}: {2}','Link hypothesis: {0} → {1}: {2}',[title(from),nodes.get(to)?title(to):to,text])});
      trialBox.replaceChildren(el('p','k-ask',t('Прямой связи этих звёзд в ядре нет. Ваша гипотеза сохранена и передана собеседнику.','The core has no direct link between these stars. Your hypothesis is saved and passed to your interlocutor.')),btn(t('Дальше','Continue'),()=>{closeTrial();paint();}));
    }
  }
  let flashTimer;function flash(text){hint.textContent=text;hint.classList.add('is-flash');clearTimeout(flashTimer);flashTimer=setTimeout(()=>hint.classList.remove('is-flash'),1600);}

  // ── Journal ──
  let shown=null;
  function show(target){shown=target;renderLog();}
  function renderLog(){
    log.replaceChildren();
    const head=el('header','k-log-head');head.append(el('h2','',t('Познание','Knowing')));
    const levels=el('div','k-levels');levels.setAttribute('role','radiogroup');levels.setAttribute('aria-label',t('Уровень помощи','Level of help'));
    for(const [value,ru,en] of [['light','Лёгкий','Light'],['medium','Средний','Medium'],['heavy','Тяжёлый','Heavy']]){const b=btn(t(ru,en),()=>{state.level=value;save();closeTrial();const select=agentRoot.querySelector('.cognitive-controls select');if(select&&select.value!==value){select.value=value;select.dispatchEvent(new Event('change'));}paint();},'k-level');b.setAttribute('role','radio');b.setAttribute('aria-checked',String(state.level===value));levels.append(b);}
    const levelNote={light:t('Подсказки ведут к ближайшим открытиям.','Hints lead to the nearest discoveries.'),medium:t('Испытания внимания: куда ведёт место и каким словом.','Attention trials: where a place leads and with what word.'),heavy:t('Без подсказок: связи вы протягиваете сами.','No hints: you draw the links yourself.')}[state.level];
    head.append(levels,el('p','k-muted',levelNote));
    const total=nodes.size,opened=state.known.length,bar=el('div','k-progress');bar.append(el('span'));bar.firstChild.style.width=`${Math.round(opened/total*100)}%`;
    head.append(bar,el('p','k-muted',t('Открыто {0} из {1} · связей {2} из {3}','Opened {0} of {1} · links {2} of {3}',[opened,total,state.found.length,links.length])));log.append(head);
    if(shown){
      const link=links.find(item=>item.id===shown?.id),card=el('section','k-card');
      if(link){card.append(el('p','k-eyebrow',t('Связь','Link')));const h=el('div','k-trial-head');h.append(el('strong','',title(link.record.from)),el('span','k-verb',tr(link.record.verb)),el('strong','',title(link.record.to)));card.append(h);const sense=tr(link.record.sense);if(sense)card.append(el('p','',sense));const cite=placeOf(link);if(cite)card.append(fragments.fragment(cite));card.append(btn(t('Открыть в справочнике','Open in reference'),()=>onOpenReference?.(link.id)));}
      else if(nodes.has(shown)){
        const record=nodes.get(shown).record,essence=record.conception??record.essence??{};
        card.append(el('p','k-eyebrow',kindWord(shown)),el('h3','',title(shown)));
        if(tr(record.question))card.append(el('p','k-question',tr(record.question)));
        if(essence.formula?.cite)card.append(fragments.fragment(essence.formula.cite));
        const thesis=tr(essence.thesis)||tr(essence.position)||tr(essence.who);if(thesis)card.append(el('p','',thesis));
        const row=el('div','k-trial-row');row.append(btn(t('Статья','Article'),()=>onOpenReference?.(shown)),btn(t('В пространстве','In the space'),()=>onOpenSpace?.(shown)));card.append(row);
      }
      if(card.childElementCount)log.append(card);
    }
    const rumors=state.level==='heavy'?[]:frontierLinks();
    if(rumors.length){
      const box=el('details','k-list');box.open=true;box.append(el('summary','',`${t('Вопросы на пороге','Questions at the threshold')} · ${rumors.length}`));
      const k=known(),groups=new Map();
      for(const link of rumors){const from=k.has(link.record.from)?link.record.from:link.record.to;if(!groups.has(from))groups.set(from,[]);groups.get(from).push(link);}
      for(const [from,items] of groups){
        box.append(el('h4','k-group',title(from)));
        for(const link of items.slice(0,12)){const target=other(link,from),rumor=rumorOf(link,from),row=btn('',()=>{anchor=from;openTrial(link,target);},'k-row');row.append(el('span','k-row-head',rumor.head));if(rumor.question)row.append(el('span','k-row-question',rumor.question));row.addEventListener('mouseenter',()=>{focus=target;paint(false);});box.append(row);}
      }
      log.append(box);
    }
    if(state.found.length){
      const box=el('details','k-list');box.append(el('summary','',`${t('Открытия','Discoveries')} · ${state.found.length}`));
      for(const item of [...state.found].reverse()){const link=links.find(value=>value.id===item.link);if(!link)continue;box.append(btn(`${title(link.record.from)} — ${tr(link.record.verb)} → ${title(link.record.to)}${item.hinted?' ·'+t(' с подсказкой',' with a hint'):''}`,()=>show(link),'k-row'));}
      log.append(box);
    }
    if(state.hypotheses.length){const box=el('details','k-list');box.append(el('summary','',`${t('Мои гипотезы','My hypotheses')} · ${state.hypotheses.length}`));for(const item of [...state.hypotheses].reverse())box.append(el('p','k-muted',`${title(item.from)} → ${title(item.to)}: ${item.text}`));log.append(box);}
    agentTitle.textContent=t('Собеседник','Interlocutor');log.append(agentBox);
    if(state.known.length){const reset=btn(t('Начать заново','Start over'),()=>{if(reset.dataset.armed){state={...state,start:null,known:[],found:[],hypotheses:state.hypotheses};anchor=null;focus=null;shown=null;closeTrial();save();paint();frame();}else{reset.dataset.armed='1';reset.textContent=t('Нажмите ещё раз, чтобы стереть открытия','Click again to erase discoveries');}},'k-reset');log.append(reset);}
  }

  const ready=(async()=>{try{expected=await notebook.getPreference(key);state=clean(expected);}catch{}if(disposed)return;anchor=state.known.at(-1)??null;paint();frame();})();
  return {agentRoot,ready,setHelpLevel(value){if(LEVELS.includes(value)&&state.level!==value){state.level=value;save();closeTrial();paint();}},focusItem(id){if(nodes.has(id)&&state.known.includes(id)){focus=id;anchor=id;show(id);paint();}},refreshLocale(){paint();if(trialBox.classList.contains('is-start'))startPanel();else if(trial?.kind==='revealed')revealed(trial.link);else if(trial&&trial.kind!=='reading')renderTrial();},destroy(){disposed=true;clearTimeout(flashTimer);root.classList.remove('k-root');root.replaceChildren();}};
}
