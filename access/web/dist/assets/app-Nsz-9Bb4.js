import{a as Gc,b as jd,e as eu,c as tu,d as nu}from"./webmcp-BcZzX6i6.js";const iu=`<div id="sophia-gestures" aria-label="Древо Софии — область исследования">
  <canvas class="sc-sky" role="img" aria-label="Трёхмерное звёздное пространство. Два пальца сдвигают пространство, щипок приближает и отдаляет. Перетаскивание вращает; Shift с перетаскиванием сдвигает."></canvas>
  <div class="sc-vignette" aria-hidden="true"></div>
  <header class="sc-header">
    <div class="sc-brand"><span class="sc-sigil" aria-hidden="true">✧</span><div>Древо Софии<span class="sc-brand-sub">АТЛАС МЫСЛИ</span></div></div>
    <div class="sc-header-actions">
      <button type="button" class="sc-control sc-search-open" aria-label="Найти узел" aria-expanded="false"><i data-lucide="search" aria-hidden="true"></i><span>Найти в древе</span><kbd>/</kbd></button>
      <button type="button" class="sc-control sc-lenses-open" aria-label="Линзы" aria-expanded="false"><i data-lucide="layers-2" aria-hidden="true"></i><span>Линзы</span></button>
    </div>
  </header>
  <div class="sc-context"><span class="sc-eyebrow">ПРОСТРАНСТВО 01</span><h2>Созвездия мысли</h2><div class="sc-context-route"><button type="button" class="sc-back" aria-label="Вернуться к предыдущему виду" hidden><i data-lucide="arrow-left" aria-hidden="true"></i><span>Назад</span></button><span class="sc-context-sub">Загружаем область…</span></div></div>
  <div class="sc-constellation-label sc-label-west" hidden aria-hidden="true"><span>01 / ИСТОКИ</span><small>Течение и единство</small></div>
  <div class="sc-constellation-label sc-label-east" hidden aria-hidden="true"><span>03 / ГОРИЗОНТЫ</span><small>Человек и возможность</small></div>
  <div class="sc-nodes" role="group" aria-label="Узлы созвездий"></div>
  <section class="sc-panel sc-inspector" aria-label="Выбранный узел" hidden>
    <div class="sc-panel-top"><button type="button" class="sc-window-handle" aria-label="Переместить карточку" data-tooltip="Перетаскивание или стрелки на клавиатуре"><i data-lucide="grip-horizontal" aria-hidden="true"></i><span class="sc-eyebrow sc-kind">КОНЦЕПТ</span></button><button type="button" class="sc-icon sc-close" aria-label="Закрыть карточку"><i data-lucide="x" aria-hidden="true"></i></button></div>
    <h3 class="sc-node-title"></h3><div class="sc-node-original"></div>
    <div class="sc-card-tabs" role="tablist" aria-label="Содержание карточки"><button type="button" class="sc-card-tab" id="so-about-tab" role="tab" aria-selected="true" aria-controls="so-about">Смысл</button><button type="button" class="sc-card-tab" id="so-relations-tab" role="tab" aria-selected="false" aria-controls="so-relations">Связи <span class="sc-neighbor-count"></span></button></div>
    <div id="so-about" role="tabpanel" aria-labelledby="so-about-tab"><p class="sc-description"></p><div class="sc-provenance"></div></div>
    <div id="so-relations" role="tabpanel" aria-labelledby="so-relations-tab" hidden><div class="sc-neighbors" aria-label="Отношения выбранного узла"></div></div>
    <div class="sc-panel-bottom"><button type="button" class="sc-open-neighborhood">Окрестность</button><button type="button" class="sc-read-selected" aria-label="Оставить для чтения">Читать</button><button type="button" class="sc-focus">Приблизиться <span aria-hidden="true">↗</span></button></div>
  </section>
  <section class="sc-panel sc-search" aria-label="Поиск по Древу Софии" hidden>
    <div class="sc-panel-top"><label class="sc-eyebrow" for="sc-query">НАЙТИ В ДРЕВЕ</label><button class="sc-icon sc-search-close" type="button" aria-label="Закрыть поиск"><i data-lucide="x" aria-hidden="true"></i></button></div>
    <input id="sc-query" type="search" placeholder="Имя, произведение, понятие…" autocomplete="off">
    <div class="sc-search-results" aria-label="Результаты поиска"></div>
  </section>
  <section class="sc-panel sc-lenses" aria-label="Линзы пространства" hidden>
    <div class="sc-panel-top"><span class="sc-eyebrow">ЛИНЗЫ ПРОСТРАНСТВА</span><button class="sc-icon sc-lenses-close" type="button" aria-label="Закрыть линзы"><i data-lucide="x" aria-hidden="true"></i></button></div>
    <button class="sc-lens" data-lens="constellations" type="button" aria-pressed="true"><span>Созвездия</span><small>Свободная глубина</small><span class="sc-lens-mark" aria-hidden="true">✧</span></button>
    <button class="sc-lens" data-lens="orbits" type="button" aria-pressed="false"><span>Орбиты</span><small>Окружение выбранного узла</small><span class="sc-lens-mark" aria-hidden="true">◎</span></button>
    <button class="sc-lens" data-lens="plane" type="button" aria-pressed="false"><span>Плоскость</span><small>Спокойное чтение связей</small><span class="sc-lens-mark" aria-hidden="true">⌗</span></button>
  </section>
  <footer class="sc-footer">
    <div class="sc-caption"><span class="sc-tiny-star" aria-hidden="true"></span><span>Данные ToS</span><span class="sc-caption-detail">· ограниченная область</span></div>
    <div class="sc-navigation" aria-label="Управление пространством">
      <button type="button" class="sc-icon sc-minus" aria-label="Отдалить"><i data-lucide="minus" aria-hidden="true"></i></button>
      <button type="button" class="sc-overview" aria-label="Вернуться к общему виду">Общий вид</button>
      <button type="button" class="sc-icon sc-plus" aria-label="Приблизить"><i data-lucide="plus" aria-hidden="true"></i></button>
      <span class="sc-separator" aria-hidden="true"></span>
      <button type="button" class="sc-icon sc-motion" aria-label="Приостановить движение" aria-pressed="false"><i data-lucide="pause" aria-hidden="true"></i></button>
      <button type="button" class="sc-icon sc-input-mode" aria-label="Управление: тачпад. Переключить на мышь" data-tooltip="Тачпад · два пальца — сдвиг, щипок — полёт"><i data-lucide="touchpad" aria-hidden="true"></i></button>
    </div>
    <span class="sc-gesture">Два пальца — сдвиг · щипок — полёт</span>
  </footer>
  <div class="sc-data-notice" role="status" hidden><span></span><button type="button" class="sc-retry">Повторить</button></div><div class="sc-announcement" role="status" aria-live="polite"></div>
</div>
`;function Br(n,{limit:e=48}={}){const t=new Map;let i=null,r=!1;const s=(l,c)=>l.dataset.readingKey||l.querySelector("summary")?.textContent?.trim()||String(c);function a(){if(!i||r||n.getAttribute("aria-busy")==="true"||!n.isConnected||!n.getClientRects().length)return;const l=[...n.querySelectorAll("details")].map((d,h)=>[s(d,h),d.open]),c=n.getBoundingClientRect(),u=[...n.querySelectorAll("h4,article,p")].find(d=>d.getBoundingClientRect().bottom>c.top+4);t.delete(i),t.set(i,{top:n.scrollTop,details:l,anchor:u?{key:u.dataset.readingAnchor||null,text:u.textContent.slice(0,180),offset:u.getBoundingClientRect().top-c.top}:null}),t.size>e&&t.delete(t.keys().next().value)}function o(){const l=t.get(i);if(!l||!n.getClientRects().length)return;r=!0;const c=new Map(l.details);if([...n.querySelectorAll("details")].forEach((f,u)=>{c.has(s(f,u))&&(f.open=c.get(s(f,u)))}),n.scrollTop=l.top,l.anchor){const f=[...n.querySelectorAll("h4,article,p")].find(u=>l.anchor.key?u.dataset.readingAnchor===l.anchor.key:u.textContent.slice(0,180)===l.anchor.text);f&&(n.scrollTop+=f.getBoundingClientRect().top-n.getBoundingClientRect().top-l.anchor.offset)}r=!1}return n.addEventListener("scroll",a,{passive:!0}),{capture:a,restore:o,enter(l){l!==i&&(a(),i=l)},get key(){return i}}}const to="tos.work.friedrich-nietzsche.also-sprach-zarathustra",Xn=Object.freeze({nodes:40,relations:80}),Hc=new WeakMap,ru=n=>Hc.get(n)||null;class Ft extends Error{}class Ei extends Error{constructor(){super("Данные изменились. Обновите область, чтобы продолжить.")}}class Dr extends Error{constructor(e,t){super(t),this.status=e}}function mt(n,e=""){return[n?.ru,n?.default,n?.original,n?.en].find(t=>typeof t=="string"&&t.trim())||e}function ta(n,{depth:e=1}={}){return{schema_version:"tos_lens_spec_v1",lens_id:"sophia-observatory-focus",language:"ru",detail:"compact",explain:!0,seed:{focus_node_id:n},node_query:{enabled:!1},traversal:{depth:e,direction:"either",profile:"overview"},limits:{...Xn,groups:8}}}function Vc(n){const e=ta(n.from_id,{depth:0});return e.node_query={enabled:!0,filters:[{field:"id",op:"in",value:[n.from_id,n.to_id]}]},e.traversal.profile="all",e.relation_query={filters:[{field:"id",op:"eq",value:n.id}]},e.limits={nodes:2,relations:1,groups:2},e}function os(n,e){if(!/^[a-f0-9]{64}$/.test(n?.source_revision||""))throw new Ft("Ответ не содержит версию данных.");if(e&&n.source_revision!==e)throw new Ei;return n}function Ir(n,e){if(!Array.isArray(n))throw new Ft("Неверный список объектов.");const t=new Set;for(const i of n){if(!i||typeof i.id!="string"||!i.id||t.has(i.id)||!i.display||!mt(e==="node"?i.display.title:i.display.label)||!/^[a-f0-9]{64}$/.test(i.content_revision||"")||!Array.isArray(i.source_refs)||!i.source_refs.length||i.source_refs.some(r=>typeof r!="string"||!r))throw new Ft("Неполный или повторяющийся объект.");t.add(i.id)}return t}function Wc(n,e=null){if(os(n,e),n.authority_boundary?.is_source!==!1||n.authority_boundary?.is_canon!==!1||n.authority_boundary?.writes_to_tree!==!1)throw new Ft("Неподдерживаемый контракт области.");if(!Array.isArray(n.nodes)||!Array.isArray(n.relations)||n.nodes.length>Xn.nodes||n.relations.length>Xn.relations)throw new Ft("Область превышает бюджет отображения.");const t=Ir(n.nodes,"node");if(Ir(n.relations,"relation"),n.relations.some(i=>!t.has(i.from_id)||!t.has(i.to_id)))throw new Ft("Связь не содержит оба конца в области.");if(n.focus&&!t.has(n.focus.node_id))throw new Ft("Центр отсутствует в области.");return n}function da(n,e=null){if(n?.schema!=="tos_lens_result_v1")throw new Ft("Неподдерживаемый контракт линзы.");return Wc(n,e)}function jo(n,e=null,t=null){Wc(n,e);const i=n.page,r=new Set(n.nodes.map(s=>s.id));if(n.schema!=="tos_exploration_result_v1"||n.writes_to_tree!==!1||!/^[a-f0-9]{64}$/.test(n.snapshot_revision||"")||!["paused","complete","limit_reached"].includes(n.status)||!n.focus||!Number.isInteger(i?.number)||i.number<1||i.scope!=="resumable-neighborhood"||i.returned_nodes!==n.nodes.length||i.returned_relations!==n.relations.length||!Array.isArray(i.primary_node_ids)||!Array.isArray(i.context_node_ids)||i.primary_node_ids.length+i.context_node_ids.length!==r.size||new Set([...i.primary_node_ids,...i.context_node_ids]).size!==r.size||[...i.primary_node_ids,...i.context_node_ids].some(s=>!r.has(s))||n.counts?.scope!=="cumulative-discovered-not-global-total"||n.inclusion?.authority!=="query-execution-not-semantic-proof"||(n.status==="paused"?!/^[a-f0-9]{64}$/.test(i.next_cursor||""):i.next_cursor!==null))throw new Ft("Неполная страница раскрытия связей.");if(t&&(n.snapshot_revision!==t.snapshot_revision||n.focus.node_id!==t.focus.node_id||i.number!==t.page.number+1||JSON.stringify(n.query)!==JSON.stringify(t.query)))throw new Ei;return n}class cr{constructor(){this.slots=new Map}cancel(e){this.slots.get(e)?.abort(),this.slots.delete(e)}cancelAll(){for(const e of this.slots.keys())this.cancel(e)}async run(e,t){this.cancel(e);const i=new AbortController;this.slots.set(e,i);try{const r=await t(i.signal);return this.slots.get(e)===i?{current:!0,value:r}:{current:!1}}catch(r){if(this.slots.get(e)!==i||i.signal.aborted)return{current:!1};throw r}finally{this.slots.get(e)===i&&this.slots.delete(e)}}}class Tl{constructor({fetcher:e=globalThis.fetch.bind(globalThis),base:t="/api/knowledge",timeoutMs:i=6e4}={}){this.fetcher=e,this.base=t,this.timeoutMs=i}async request(e,{signal:t,body:i}={}){const r=new AbortController;let s=!1;const a=()=>r.abort(t.reason);t?.aborted?a():t?.addEventListener("abort",a,{once:!0});const o=setTimeout(()=>{s=!0,r.abort()},this.timeoutMs);try{const l=await this.fetcher(this.base+e,{signal:r.signal,method:i?"POST":"GET",headers:i?{"Content-Type":"application/json"}:{},...i?{body:JSON.stringify(i)}:{}});if(!l.ok)throw l.status===409?new Ei:new Dr(l.status,{400:"Запрос не удалось исполнить.",403:"Доступ к материалу ограничен.",404:"Объект больше не доступен.",410:"Срок сохранённого обхода истёк.",413:"Область слишком велика. Выберите более узкий центр.",503:"Этот способ просмотра пока не доступен."}[l.status]||"Не удалось получить данные. Попробуйте ещё раз.");const c=await l.json();if(!c||typeof c!="object")throw new Ft("Неверный ответ сервера.");return c}catch(l){throw s?new Dr(504,"Сервер отвечает дольше обычного. Попробуйте ещё раз."):!r.signal.aborted&&(l instanceof TypeError||l?.name==="NetworkError")?new Dr(0,"Нет связи с данными. Проверьте соединение и повторите запрос."):l instanceof SyntaxError?new Ft("Сервер вернул нечитаемый ответ. Повторите запрос."):l}finally{clearTimeout(o),t?.removeEventListener("abort",a)}}async search(e,t,i=0){const r=os(await this.request("/search?"+new URLSearchParams({query:e,limit:6,offset:i}),{signal:t}));if(r.schema!=="tos_knowledge_search_v1"||r.nodes?.length>6||r.relations?.length>6)throw new Ft("Неподдерживаемый ответ поиска.");return Ir(r.nodes,"node"),Ir(r.relations,"relation"),r}async compile(e,t,i=null){const r=structuredClone(e),s=da(await this.request("/lenses/compile",{signal:t,body:r}),i);return Hc.set(s,r),s}async explore(e,t,i,r=null){const s=jo(await this.request("/explore",{signal:t,body:e}),i,r);if(!r&&Object.entries(e).some(([a,o])=>JSON.stringify(s.query?.[a])!==JSON.stringify(o)))throw new Ft("Сервер вернул другую область раскрытия.");return s}async inspect(e,t,i,r,s){const a=os(await this.request("/"+(e==="node"?"nodes/":"relations/")+encodeURIComponent(t)+(e==="node"?"?relation_limit=0":""),{signal:i}),r);if(a.schema!==(e==="node"?"tos_knowledge_node_packet_v1":"tos_knowledge_relation_packet_v1"))throw new Ft("Неверная карточка.");Ir(a.matches,e);const o=a.matches.find(l=>l.id===t);if(!o)throw new Ft("Не найден точный идентификатор карточки.");if(s&&o.content_revision!==s)throw new Ei;if(e==="relation"){const l=Ir(a.endpoints,"node");if(!l.has(o.from_id)||!l.has(o.to_id))throw new Ft("Неполные концы связи.")}return{packet:a,match:o}}capabilities(e){return this.request("/explore/capabilities",{signal:e})}}const su=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];function au(n){let e=2166136261;for(const t of n)e=Math.imul(e^t.codePointAt(0),16777619);return e>>>0}function ou(n,e=[]){n.schema==="tos_exploration_result_v1"?jo(n):da(n);const t=new Map(e.map(c=>[c.id,c])),i=new Map;for(const c of n.relations)i.set(c.from_id,(i.get(c.from_id)||0)+1),i.set(c.to_id,(i.get(c.to_id)||0)+1);const r=n.focus?.node_id,s=n.nodes.slice().sort((c,f)=>(f.id===r)-(c.id===r)||(i.get(f.id)||0)-(i.get(c.id)||0)||c.id.localeCompare(f.id,"en")),a=new Set(n.nodes.map(c=>c.id)),o=new Set(e.filter(c=>a.has(c.id)).map(c=>c.slot));let l=0;return s.map((c,f)=>{const u=t.get(c.id);for(;o.has(l);)l++;const d=u?.slot??l++;o.add(d);const h=mt(c.display.title,c.id),_=h.length>46?h.slice(0,43)+"…":h,x=au(c.id),m=d*2.399963229728653,p=su[d]?.slice()||[Math.cos(m)*(260+d%4*58),Math.sin(m)*(160+d%3*47),-480+x%740];return{id:c.id,raw:c,name:_,fullName:h,original:c.display.title.original||c.display.title.en||"",kind:mt(c.display.kind_label,c.kind_id),description:mt(c.display.summary),main:f<8,above:f%3===1,group:c.id===r?1:c.kind_id==="agent"?0:c.kind_id==="expression"?2:1,slot:d,p:u?.p?.slice()||p,sourcePosition:u?.sourcePosition?.slice()||p.slice(),volumeZ:u?.volumeZ??p[2],pos:u?.pos?.slice()||p.slice(),target:u?.target?.slice()||p.slice()}})}const el=n=>typeof n=="string"&&!!n.trim(),no=n=>!["default","original"].includes(n)&&/^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(n),Xc=(n,e)=>JSON.stringify([n,e]);function bs(n){return{ru:"Русский",en:"English",original:"Исходная форма",default:"Форма по умолчанию"}[n]||n}function Lr(n,e="ru"){const i=[e,"default","original","ru","en",...Object.keys(n||{}).filter(no)].find(r=>el(n?.[r]));return i?{text:n[i],key:i,lang:no(i)?i:null,fallback:i!==e}:null}function lu(n){const e=n.raw.display,t=["title","label","kind_label","statement","summary","explanation"];return[...new Set(t.flatMap(i=>Object.keys(e[i]||{})))].filter(i=>(no(i)||["original","default"].includes(i))&&t.some(r=>el(e[r]?.[i])))}function Al(n){return Object.fromEntries(["id","kind_id","from_id","to_id","predicate_id","source_graph","native_id","content_revision","display","epistemic","source_refs"].filter(t=>n[t]!==void 0).map(t=>[t,structuredClone(n[t])]))}function cu({packet:n,match:e},t){if(os(n),!["node","relation"].includes(t)||!el(e?.id)||!e.display||!/^[a-f0-9]{64}$/.test(e.content_revision||"")||!Array.isArray(e.source_refs)||!e.source_refs.length)throw new Ft("Материал не содержит точной версии и источника.");const i=t==="relation"?n.endpoints||[]:[];if(t==="relation"&&![e.from_id,e.to_id].every(a=>i.some(o=>o.id===a)))throw new Ft("Для чтения связи нужны оба её участника.");const r=t==="relation"?[...new Set([e.from_id,e.to_id])].map(a=>i.find(o=>o.id===a)):[],s={kind:t,sourceRevision:n.source_revision,raw:Al(e),endpoints:r.map(Al)};if(JSON.stringify(s).length>5e5)throw new Ft("Материал слишком велик для закреплённой карточки. Откройте его источник.");return s}function du(n,e="ru"){const{raw:t,kind:i}=n,r=t.display,s=i==="node"?r.summary_state:r.explanation_state,a=[];return i==="relation"&&a.push({id:"statement",title:"Формулировка связи",form:Lr(r.statement,e)}),a.push({id:"description",title:i==="node"?"Описание":"Пояснение",state:s,form:s==="missing"?null:Lr(i==="node"?r.summary:r.explanation,e)}),{title:Lr(i==="node"?r.title:r.label,e),kind:i==="node"?Lr(r.kind_label,e):null,blocks:a,participants:i==="relation"?[["От",t.from_id],["К",t.to_id]].map(([o,l])=>({id:l,role:o,form:Lr(n.endpoints.find(c=>c.id===l).display.title,e)})):[],sourceRefs:[...t.source_refs],posture:t.epistemic||{}}}function uu({client:n,onChange:e=()=>{}}){const t=new Map,i=new cr;let r=null;const s=(o,l)=>{t.has(o)&&(t.set(o,{...t.get(o),...l}),e())};async function a(o,l=!1){const c=t.get(o);if(c){s(o,{loading:!0,error:null});try{const f=await i.run(o,h=>n.inspect(c.kind,c.id,h,l?null:c.sourceRevision,l?void 0:c.contentRevision));if(!f.current||!t.has(o))return;const u=cu(f.value,c.kind);if(u.raw.id!==c.id)throw new Ft("Сервер вернул другой предмет для чтения.");const d=c.bookmark?.graph?.packet?.source_revision===u.sourceRevision?c.bookmark:null;s(o,{snapshot:u,bookmark:d,sourceRevision:u.sourceRevision,contentRevision:u.raw.content_revision,loading:!1,error:null})}catch(f){const u=f instanceof Dr&&[403,404,410].includes(f.status);s(o,{loading:!1,error:f.message||"Материал не удалось загрузить.",...u?{snapshot:null,bookmark:null}:{}})}}}return{get entries(){return[...t.values()]},get sceneRevision(){return r},observeRevision(o){r!==o&&(r=o,e())},pin({raw:o,kind:l,sourceRevision:c,bookmark:f}){const u=Xc(l,o.id);if(t.has(u))return{key:u,existing:!0};if(t.size>=2)throw new Error("Уже закреплены два материала. Уберите один из них, чтобы добавить другой.");if(!["node","relation"].includes(l)||!/^[a-f0-9]{64}$/.test(c||""))throw new Ft("Сначала дождитесь загрузки выбранного материала.");return t.set(u,{key:u,kind:l,id:o.id,title:Lr(o.display.title||o.display.label),sourceRevision:c,contentRevision:o.content_revision,bookmark:f,snapshot:null,loading:!0,error:null}),e(),a(u),{key:u,existing:!1}},refresh:o=>a(o,!0),remove(o){i.cancel(o),t.delete(o),e()},suspend(){i.cancelAll();for(const[o,l]of t)l.loading&&t.set(o,{...l,loading:!1,error:"Загрузка прервана. Обновите материал, чтобы продолжить."})},dispose(){i.cancelAll(),t.clear()}}}const qc={xmlns:"http://www.w3.org/2000/svg",width:24,height:24,viewBox:"0 0 24 24",fill:"none",stroke:"currentColor","stroke-width":2,"stroke-linecap":"round","stroke-linejoin":"round"};const Yc=([n,e,t])=>{const i=document.createElementNS("http://www.w3.org/2000/svg",n);return Object.keys(e).forEach(r=>{i.setAttribute(r,String(e[r]))}),t?.length&&t.forEach(r=>{const s=Yc(r);i.appendChild(s)}),i},fu=(n,e={})=>{const i={...qc,...e};return Yc(["svg",i,n])};const hu=n=>{for(const e in n)if(e.startsWith("aria-")||e==="role"||e==="title")return!0;return!1};const pu=(...n)=>n.filter((e,t,i)=>!!e&&e.trim()!==""&&i.indexOf(e)===t).join(" ").trim();const mu=n=>n.replace(/^([A-Z])|[\s-_]+(\w)/g,(e,t,i)=>i?i.toUpperCase():t.toLowerCase());const gu=n=>{const e=mu(n);return e.charAt(0).toUpperCase()+e.slice(1)};const _u=n=>Array.from(n.attributes).reduce((e,t)=>(e[t.name]=t.value,e),{}),Rl=n=>typeof n=="string"?n:!n||!n.class?"":n.class&&typeof n.class=="string"?n.class.split(" "):n.class&&Array.isArray(n.class)?n.class:"",Cl=(n,{nameAttr:e,icons:t,attrs:i})=>{const r=n.getAttribute(e);if(r==null)return;const s=gu(r),a=t[s];if(!a)return console.warn(`${n.outerHTML} icon name was not found in the provided icons object.`);const o=_u(n),l=hu(o)?{}:{"aria-hidden":"true"},c={...qc,"data-lucide":r,...l,...i,...o},f=Rl(o),u=Rl(i),d=pu("lucide",`lucide-${r}`,...f,...u);d&&Object.assign(c,{class:d});const h=fu(a,c);return n.parentNode?.replaceChild(h,n)};const vu=[["path",{d:"m12 19-7-7 7-7"}],["path",{d:"M19 12H5"}]];const xu=[["path",{d:"M12 7v14"}],["path",{d:"M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"}]];const yu=[["path",{d:"M17 3a2 2 0 0 1 2 2v15a1 1 0 0 1-1.496.868l-4.512-2.578a2 2 0 0 0-1.984 0l-4.512 2.578A1 1 0 0 1 5 20V5a2 2 0 0 1 2-2z"}]];const bu=[["circle",{cx:"12",cy:"9",r:"1"}],["circle",{cx:"19",cy:"9",r:"1"}],["circle",{cx:"5",cy:"9",r:"1"}],["circle",{cx:"12",cy:"15",r:"1"}],["circle",{cx:"19",cy:"15",r:"1"}],["circle",{cx:"5",cy:"15",r:"1"}]];const Su=[["path",{d:"M13 13.74a2 2 0 0 1-2 0L2.5 8.87a1 1 0 0 1 0-1.74L11 2.26a2 2 0 0 1 2 0l8.5 4.87a1 1 0 0 1 0 1.74z"}],["path",{d:"m20 14.285 1.5.845a1 1 0 0 1 0 1.74L13 21.74a2 2 0 0 1-2 0l-8.5-4.87a1 1 0 0 1 0-1.74l1.5-.845"}]];const Mu=[["path",{d:"M5 12h14"}]];const Eu=[["rect",{x:"5",y:"2",width:"14",height:"20",rx:"7"}],["path",{d:"M12 6v4"}]];const wu=[["path",{d:"M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4"}],["path",{d:"M2 6h4"}],["path",{d:"M2 10h4"}],["path",{d:"M2 14h4"}],["path",{d:"M2 18h4"}],["path",{d:"M21.378 5.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z"}]];const Tu=[["rect",{x:"14",y:"3",width:"5",height:"18",rx:"1"}],["rect",{x:"5",y:"3",width:"5",height:"18",rx:"1"}]];const Au=[["path",{d:"M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"}]];const Ru=[["path",{d:"M5 12h14"}],["path",{d:"M12 5v14"}]];const Cu=[["circle",{cx:"6",cy:"19",r:"3"}],["path",{d:"M9 19h8.5a3.5 3.5 0 0 0 0-7h-11a3.5 3.5 0 0 1 0-7H15"}],["circle",{cx:"18",cy:"5",r:"3"}]];const Pu=[["path",{d:"m21 21-4.34-4.34"}],["circle",{cx:"11",cy:"11",r:"8"}]];const Lu=[["rect",{width:"20",height:"16",x:"2",y:"4",rx:"2"}],["path",{d:"M2 14h20"}],["path",{d:"M12 20v-6"}]];const Iu=[["path",{d:"M18 6 6 18"}],["path",{d:"m6 6 12 12"}]];const $c=({icons:n={},nameAttr:e="data-lucide",attrs:t={},root:i=document,inTemplates:r}={})=>{if(!Object.values(n).length)throw new Error(`Please provide an icons object.
If you want to use all the icons you can import it like:
 \`import { createIcons, icons } from 'lucide';
lucide.createIcons({icons});\``);if(typeof i>"u")throw new Error("`createIcons()` only works in a browser environment.");if(Array.from(i.querySelectorAll(`[${e}]`)).forEach(a=>Cl(a,{nameAttr:e,icons:n,attrs:t})),r&&Array.from(i.querySelectorAll("template")).forEach(o=>$c({icons:n,nameAttr:e,attrs:t,root:o.content,inTemplates:r})),e==="data-lucide"){const a=i.querySelectorAll("[icon-name]");a.length>0&&(console.warn("[Lucide] Some icons were found with the now deprecated icon-name attribute. These will still be replaced for backwards compatibility, but will no longer be supported in v1.0 and you should switch to data-lucide"),Array.from(a).forEach(o=>Cl(o,{nameAttr:"icon-name",icons:n,attrs:t})))}};function yi(){$c({icons:{Search:Pu,Layers2:Su,ArrowLeft:vu,GripHorizontal:bu,X:Iu,Minus:Mu,Plus:Ru,Pause:Tu,Play:Au,Touchpad:Lu,Mouse:Eu,NotebookPen:wu,Route:Cu,Bookmark:yu,BookOpen:xu}})}const gt=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Un=(n,e,t="")=>{const i=gt("button",n,t);return i.type="button",i.addEventListener("click",e),i},Pl={authored:"Авторское описание","source-derived":"Описание из источника","metadata-synthesis":"Описание составлено из метаданных"},Ll={disputed:"Оспаривается",rejected:"Отклонено",accepted:"Принято в указанной источником области",contested_review_required:"Требует рассмотрения",unresolved:"Не разрешено",review_status_unresolved:"Статус рассмотрения не установлен",pending_human_review:"Ожидает рассмотрения",unreviewed:"Не рассмотрено","not-recorded":"Не указан"};function Du(n,e,t,{data:{client:i},onUserAction:r=()=>{}}){const s=new Map;let a=null,o=null,l="",c=!1;const f=Un("",()=>P(),"sc-control sc-reader-open");f.setAttribute("aria-label","Чтение и сопоставление"),f.setAttribute("aria-expanded","false"),f.innerHTML='<i data-lucide="book-open" aria-hidden="true"></i><span>Чтение</span>',n.querySelector(".sc-header-actions").append(f);const u=Un("",()=>P(),"sc-reader-resume");u.hidden=!0,n.querySelector(".sc-context").append(u);const d=gt("section","","sc-panel sc-reader");d.hidden=!0,d.setAttribute("aria-label","Чтение и сопоставление"),d.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ЧТЕНИЕ</span><button type="button" class="sc-icon sc-reader-close" aria-label="Закрыть чтение"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-reader-heading"><h3 tabindex="-1">Удержать мысль</h3></div><div class="sc-reader-toolbar"></div><div class="sc-reader-tabs" role="tablist" aria-label="Закреплённые материалы"></div><div class="sc-reader-columns"></div><p class="sc-reader-notice" role="status"></p>',n.append(d);const h=d.querySelector(".sc-reader-columns"),_=d.querySelector(".sc-reader-tabs"),x=d.querySelector(".sc-reader-notice"),m=Un("Добавить выбранное",()=>b(),"sc-reader-add");d.querySelector(".sc-reader-toolbar").append(m);const p=gt("p","Оставьте здесь предмет или связь, чтобы читать, переходить к основаниям и сопоставлять с другим материалом.","sc-reader-empty"),C=uu({client:i,onChange:B}),D=()=>{const k=e.port.selection,K=k.relationId?"relation":"node",Z=K==="relation"?e.port.relation(k.relationId):e.port.node(k.nodeId);return Z?{raw:Z,kind:K,sourceRevision:e.port.packet?.source_revision}:null},M=k=>C.entries.find(K=>K.key===k);function L(){for(const k of s.values())k.reading.capture()}function A(){for(const k of s.values())k.article.hidden||k.reading.restore()}t.register("reader",d,()=>{L(),f.setAttribute("aria-expanded","false")}),t.configure("reader",{onResume:()=>{f.setAttribute("aria-expanded","true"),B()}}),t.addTool("reader",{title:"Чтение и сопоставление",opener:f,launch:()=>P()});function P(){r(),o=document.activeElement,t.open("reader"),f.setAttribute("aria-expanded","true"),B(),(s.get(a)?.body||d.querySelector("h3")).focus()}function v(){r(),t.close("reader"),(o?.isConnected&&!o.closest("[hidden]")?o:u.hidden?n.querySelector(".sc-studio-open"):u)?.focus()}d.querySelector(".sc-reader-close").addEventListener("click",v),d.addEventListener("keydown",k=>{k.key==="Escape"&&(k.preventDefault(),k.stopPropagation(),v())});function b(k=D()){if(!k){l="Сначала выберите звезду или связь.",P();return}r(),e.ui.captureReading();try{const K=C.pin({...k,bookmark:e.port.captureView()});a=K.key,l=K.existing?"Этот материал уже оставлен для чтения.":"Материал оставлен для чтения в этой вкладке."}catch(K){l=K.message}P()}function w(k){r(),L(),C.remove(k),l="Материал убран из чтения.",B(),(s.get(a)?.tab||m).focus()}function E(k){r(),L(),a=k,J(),A()}function N(k){const K=M(k);if(K?.bookmark){if(K.bookmark.graph.packet.source_revision!==e.port.packet?.source_revision){l="Данные изменились. Откройте актуальный материал в пространстве.",B();return}r(),L(),e.ui.cancelPending(),e.port.restoreView(K.bookmark)}}function G(k){const K=M(k);K?.snapshot&&(r(),L(),K.kind==="relation"?e.ui.chooseRelation(K.snapshot.raw,K.sourceRevision):e.ui.chooseNode(K.id,K.sourceRevision))}function W(k,K){const Z=M(k);!Z?.snapshot||Z.sourceRevision!==e.port.packet?.source_revision||(r(),L(),n.dispatchEvent(new CustomEvent(K,{detail:{raw:Z.snapshot.raw,kind:Z.kind}})))}function I(k){const K=k.key,Z=gt("article","","sc-reader-article");Z.dataset.readingId=k.id;const Y=gt("div","","sc-reader-item-head"),ne=gt("h4"),H=gt("p","","sc-reader-kind"),te=Un("Убрать",()=>w(K),"sc-reader-remove");Y.append(H,ne,te);const ie=gt("div","","sc-reader-item-controls"),X=gt("select");X.setAttribute("aria-label","Язык или форма материала");const he=gt("label","Форма");he.append(X);const _e=Un("Обновить",()=>{r(),C.refresh(K)});ie.append(he,_e);const we=gt("p","","sc-reader-state");we.setAttribute("role","status");const ee=gt("div","","sc-reader-body");ee.tabIndex=0;const ce=gt("div","","sc-reader-actions"),$=Un("К месту",()=>N(K)),pe=Un("В пространстве",()=>G(K));$.setAttribute("aria-label","Вернуться к месту закрепления"),pe.setAttribute("aria-label","Открыть материал в пространстве");const ye=Un("Основания",()=>W(K,"sophia-evidence")),Te=Un("Источники",()=>W(K,"sophia-sources"));ce.append($,pe,ye,Te),Z.append(Y,ie,we,ee,ce);const Pe=Un("",()=>E(K));Pe.setAttribute("role","tab"),Pe.id="sc-reader-tab-"+crypto.randomUUID(),Z.id="sc-reader-item-"+crypto.randomUUID(),Pe.setAttribute("aria-controls",Z.id),Pe.addEventListener("keydown",Ee=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(Ee.key))return;Ee.preventDefault();const ve=C.entries,O=ve.findIndex(Ze=>Ze.key===K),Xe=ve[Ee.key==="Home"?0:Ee.key==="End"?ve.length-1:(O+1)%ve.length];E(Xe.key),s.get(Xe.key).tab.focus()});const Ge={article:Z,head:Y,title:ne,kind:H,language:X,refresh:_e,state:we,body:ee,back:$,open:pe,evidence:ye,sources:Te,tab:Pe,reading:Br(ee,{limit:8}),preferred:"ru",snapshot:null};return X.addEventListener("change",()=>{r(),Ge.reading.capture(),Ge.preferred=X.value,Ge.snapshot=null,B()}),h.append(Z),_.append(Pe),Ge}function R(k,K,Z){const Y=gt("div","","sc-reader-text");if(K.lang&&(Y.lang=K.lang),K.text.split(/(\r?\n[\t ]*\r?\n)/).forEach((ne,H)=>{if(H%2){Y.append(document.createTextNode(ne));return}const te=gt("p",ne);te.dir="auto",te.dataset.readingAnchor=Z+":"+H,Y.append(te)}),k.append(Y),K.fallback||!K.lang){const ne=(K.fallback?"Выбранная форма отсутствует. Показана: ":"Показана: ")+bs(K.key)+(K.lang?".":". Язык в этой форме не указан.");k.append(gt("p",ne,"sc-reader-language-note"))}}function F(k,K){const Z=K.snapshot,Y=du(Z,k.preferred);k.reading.capture(),k.reading.enter(JSON.stringify([K.key,Z.sourceRevision,Z.raw.content_revision,k.preferred])),k.body.replaceChildren(),k.body.scrollTop=0,k.title.textContent=Y.title?.text||"Материал",k.title.dir="auto",Y.title?.lang?k.title.lang=Y.title.lang:k.title.removeAttribute("lang"),k.kind.textContent=K.kind==="relation"?"Связь":Y.kind?.text||"Предмет",(Y.title?.fallback||!Y.title?.lang)&&k.body.append(gt("p","Название: "+bs(Y.title?.key||"не указана")+(Y.title?.lang?"":"; язык не указан"),"sc-reader-language-note"));for(const X of Y.blocks){const he=gt("section","","sc-reader-section");he.append(gt("h5",X.title)),X.form?R(he,X.form,X.id):he.append(gt("p",X.id==="statement"?"Формулировка пока не предоставлена.":"Описание пока отсутствует.","sc-reader-gap")),Pl[X.state]&&he.append(gt("p",Pl[X.state],"sc-reader-origin")),k.body.append(he)}if(Y.participants.length){const X=gt("section","","sc-reader-section");X.append(gt("h5","Участники связи"));for(const he of Y.participants){const _e=Un("",()=>{r(),L(),e.ui.chooseNode(he.id,Z.sourceRevision)},"sc-reader-participant");_e.append(gt("small",he.role),gt("span",he.form?.text||"Участник")),X.append(_e)}k.body.append(X)}const ne=Y.posture.review_posture;Ll[ne]&&ne!=="not-recorded"&&k.body.append(gt("p","Рассмотрение: "+Ll[ne]+".","sc-reader-assessment"));const H=gt("details","","sc-reader-section");H.dataset.readingKey="sources",H.append(gt("summary","Источники · "+Y.sourceRefs.length));for(const X of Y.sourceRefs){let he=gt("span",X,"sc-source-ref");try{const _e=new URL(X);["http:","https:"].includes(_e.protocol)&&(he=gt("a",X,"sc-source-ref"),he.href=_e.href,he.target="_blank",he.rel="noopener noreferrer")}catch{}H.append(he)}k.body.append(H);const te=gt("details","","sc-reader-technical");te.dataset.readingKey="identity",te.append(gt("summary","Точные сведения о материале"));const ie=gt("dl");for(const[X,he]of[["Идентификатор",K.id],["Снимок данных",Z.sourceRevision],["Версия материала",Z.raw.content_revision],["Слой",Y.posture.authority_layer],["Рассмотрение",ne],["Канон",Y.posture.canon_status],["Уверенность, как передана источником",Y.posture.confidence]]){const _e=gt("div");_e.append(gt("dt",X),gt("dd",he==null||he==="not-recorded"?"Не указан":String(he))),ie.append(_e)}te.append(ie),k.body.append(te),k.snapshot=Z}function J(){const k=C.entries,K=c&&k.length===2;d.dataset.columns=K?"2":"1",_.hidden=k.length<2||K;for(const Z of k){const Y=s.get(Z.key),ne=Z.key===a;Y.article.hidden=!K&&!ne,Y.tab.setAttribute("aria-selected",String(ne)),Y.tab.tabIndex=ne?0:-1,_.hidden?(Y.article.removeAttribute("role"),Y.article.removeAttribute("aria-labelledby")):(Y.article.setAttribute("role","tabpanel"),Y.article.setAttribute("aria-labelledby",Y.tab.id))}e.invalidate()}function B(){d.hidden||(c=d.clientWidth>=580);const k=C.entries;for(const[Y,ne]of s)k.some(H=>H.key===Y)||(ne.article.remove(),ne.tab.remove(),s.delete(Y));k.some(Y=>Y.key===a)||(a=k[0]?.key||null);for(const Y of k){let ne=s.get(Y.key);if(ne||(ne=I(Y),s.set(Y.key,ne)),Y.snapshot&&ne.snapshot!==Y.snapshot){const ie=lu(Y.snapshot);if(ne.language.replaceChildren(...ie.map(X=>{const he=gt("option",bs(X));return he.value=X,he})),!ie.includes(ne.preferred)){const X=gt("option",bs(ne.preferred)+" — недоступна");X.value=ne.preferred,ne.language.prepend(X)}ne.language.value=ne.preferred,F(ne,Y)}else Y.snapshot||(ne.snapshot=null,ne.title.textContent=Y.title?.text||"Материал",ne.body.replaceChildren(gt("p",Y.loading?"Получаю материал…":"Материал пока недоступен.","sc-reader-gap")));ne.tab.textContent=ne.title.textContent,ne.body.setAttribute("aria-label","Чтение: "+ne.title.textContent),ne.language.disabled=!Y.snapshot,ne.refresh.disabled=Y.loading;const H=!!(Y.snapshot&&C.sceneRevision&&Y.sourceRevision!==C.sceneRevision),te=[Y.loading?"Обновляю материал…":null,Y.error,H?"Материал и сцена относятся к разным снимкам.":null,Y.snapshot&&Y.error?"Показан ранее закреплённый материал.":null];ne.state.textContent=te.filter(Boolean).join(" "),ne.state.hidden=!ne.state.textContent,ne.back.hidden=!Y.bookmark,ne.back.disabled=Y.bookmark?.graph?.packet?.source_revision!==e.port.packet?.source_revision,ne.open.disabled=!Y.snapshot||H,ne.evidence.disabled=!Y.snapshot||H,ne.sources.disabled=!Y.snapshot||H,ne.article.dataset.snapshot=Y.snapshot?.sourceRevision||""}p.hidden=!!k.length,p.isConnected||h.append(p);const K=D(),Z=K&&k.some(Y=>Y.key===Xc(K.kind,K.raw.id));m.disabled=!K||!Z&&k.length>=2,m.textContent=Z?"Читать выбранное":"Добавить выбранное",d.querySelector(".sc-reader-toolbar").hidden=!!Z||k.length===2,u.hidden=!k.length,u.textContent="К чтению · "+k.length,d.dataset.count=String(k.length),x.textContent=l||"Материалы сохраняются для чтения до закрытия вкладки.",J(),A()}return new ResizeObserver(()=>{if(d.hidden)return;const k=d.clientWidth>=580;k!==c&&(c=k,J()),A()}).observe(d),n.addEventListener("sophia-read",k=>b({...k.detail,sourceRevision:e.port.packet?.source_revision})),window.addEventListener("pagehide",k=>{k.persisted?C.suspend():C.dispose()}),window.addEventListener("pageshow",k=>{k.persisted&&B()}),yi(),B(),{selectionChanged(){C.observeRevision(e.port.packet?.source_revision),d.hidden||B()}}}const Kc=12,mi=n=>{throw new Ft(n)},io=n=>n===null||typeof n=="boolean"||typeof n=="string"&&n.length<=1024||typeof n=="number"&&Number.isFinite(n),Zc=n=>typeof n=="string"&&n.length>0&&n.length<=256,ls={eq:"равно",neq:"не равно",in:"одно из",contains:"содержит",prefix:"начинается с",exists:"значение указано",gt:"больше",gte:"не меньше",lt:"меньше",lte:"не больше"},Nu={eq:"scalar",neq:"scalar",in:"scalar-or-scalar-array",contains:"scalar-or-scalar-array",prefix:"string",exists:"boolean",gt:"number",gte:"number",lt:"number",lte:"number"},Il={kind_id:["Тип узла","string"],type_id:["Тип сущности","string"],predicate_id:["Тип связи","string"],relation_type_id:["Тип отношения","string"],"display.title.default":["Название","string"],"display.title.ru":["Название · русский","string"],"display.title.en":["Название · английский","string"],"display.summary.default":["Описание","string"],"display.summary_state":["Наличие описания","string"],"display.label.default":["Название связи","string"],"display.statement.default":["Формулировка связи","string"],"display.explanation_state":["Наличие объяснения","string"],"epistemic.authority_layer":["Слой знания","string"],"epistemic.review_posture":["Статус проверки","string"],"epistemic.canon_status":["Статус канона","string"],graph_layers:["Слои графа","string-array"],view_ids:["Представления","string-array"],source_refs:["Ссылки на источники","string-array"]},Dl={string:["eq","neq","in","contains","prefix","exists"],"string-array":["eq","neq","in","contains","exists"],number:["eq","neq","in","gt","gte","lt","lte","exists"],boolean:["eq","neq","exists"]};function Uu(n){(!n||typeof n!="object"||Array.isArray(n)||Object.keys(n).some(t=>!["nodes","relations"].includes(t)))&&mi("Неверный набор условий.");const e={};for(const t of["nodes","relations"]){const i=n[t];(!Array.isArray(i)||i.length>Kc)&&mi("Можно добавить до 12 условий в каждый раздел."),e[t]=i.map(r=>((!r||!["field","property_id"].includes(r.selector)||!Zc(r.id)||!Object.hasOwn(ls,r.op)||!(Array.isArray(r.value)?r.value.length<=100&&r.value.every(io):io(r.value))||t==="relations"&&r.selector==="property_id")&&mi("Условие неполно или имеет неподдерживаемое значение."),{selector:r.selector,id:r.id,op:r.op,value:structuredClone(r.value)}))}return e}const Fu=(n,e)=>(n.$defs?.[e+"Field"]?.anyOf||[]).flatMap(t=>t.enum||[]);function na({catalog:n,schema:e},t){const i=n.capabilities,r=t==="nodes"?"node":"relation",s=Fu(e,r),a=e.$defs?.[r+"Filter"],o=(f,u)=>(Dl[u]||[]).filter(d=>f?.includes(d)&&i.filter_operators?.includes(d)&&a?.properties?.op?.enum?.includes(d)&&i.operator_value_contracts?.[d]===Nu[d]),l=[];for(const f of i[r+"_fields"]||[]){if(!Object.hasOwn(Il,f)||!s.includes(f))continue;const[u,d]=Il[f],h=o(Object.keys(ls),d);h.length&&l.push({selector:"field",id:f,title:u,valueType:d,operators:h,suggestions:(i.facets?.[t]?.[f]||[]).map(_=>_.value).filter(io),definition:"",appliesTo:[]})}const c=i.property_filters;if(t==="nodes"&&c?.selector==="property_id"&&c.scope==="node-query-and-path-node-query"&&c.binding==="same-graph-snapshot"&&c.operators==="declared-per-property"&&a?.properties?.property_id&&a.oneOf?.some(f=>f.required?.includes("property_id"))){const f=n.semantic_registries?.properties;if(Array.isArray(f)&&f.length<=5e3){const u=new Map;for(const d of f)u.set(d?.property_id,(u.get(d?.property_id)||0)+1);for(const d of f){if(!/^tos\.property\.[a-z0-9-]+$/.test(d?.property_id||"")||u.get(d.property_id)!==1||!Object.hasOwn(Dl,d.value_type)||!Array.isArray(d.applies_to)||!d.applies_to.every(Zc))continue;const h=o(d.operators,d.value_type);h.length&&l.push({selector:"property_id",id:d.property_id,title:mt(d.labels,d.property_id),valueType:d.value_type,operators:h,definition:typeof d.definition=="string"?d.definition:"",unit:d.unit,language:d.language,appliesTo:d.applies_to,inherited:d.inherited===!0,suggestions:[]})}}}return l}const Jn=n=>n.selector+":"+n.id;function Nl(n){return{selector:n.selector,id:n.id,op:n.operators[0],value:n.valueType==="boolean"||n.operators[0]==="exists"?!0:""}}function Ul(n,e,t){const i=na(e,t),r=t==="nodes"?"node":"relation",s=e.schema.$defs?.[r+"Query"]?.properties?.filters?.maxItems;return n.length&&(!Number.isInteger(s)||n.length+1>s)&&mi("Сервер допускает меньше условий в этом разделе."),n.map(a=>{const o=i.find(f=>Jn(f)===Jn(a));(!o||!o.operators.includes(a.op))&&mi("Условие больше не поддерживается: "+(o?.title||a.id)+". Измените или удалите его.");const l=Array.isArray(a.value)?a.value:[a.value],c=o.valueType==="string-array"?"string":o.valueType;return a.op==="exists"?typeof a.value!="boolean"&&mi("Для наличия значения выберите «да» или «нет»."):(!l.length||l.some(f=>typeof f!==c||typeof f=="number"&&!Number.isFinite(f)))&&mi("Проверьте тип значения: "+o.title+"."),["eq","neq","prefix","gt","gte","lt","lte"].includes(a.op)&&Array.isArray(a.value)&&mi("Эта операция принимает одно значение."),a.op==="contains"&&Array.isArray(a.value)&&o.valueType!=="string-array"&&mi("Для поиска внутри текста нужно одно значение."),{[a.selector]:a.id,op:a.op,value:structuredClone(a.value)}})}function ro(n,e){const t=e.find(r=>Jn(r)===Jn(n)),i=n.op==="exists"?n.value?"да":"нет":Array.isArray(n.value)?n.value.map(r=>JSON.stringify(r)).join(", "):JSON.stringify(n.value);return`${t?.title||n.id} ${ls[n.op]||n.op} ${i}`}const Ou="observatory-custom",Jc="tos-observatory-lenses-v1",_n=n=>{throw new Ft(n)},nr=(n,e,t=1024)=>Array.isArray(n)&&n.length<=e&&new Set(n).size===n.length&&n.every(i=>typeof i=="string"&&i.length>0&&i.length<=t),Qc=new WeakMap,Nr=n=>Qc.get(n)||null;function Vr(n){(!n||![1,2].includes(n.v)||typeof n.name!="string"||!n.name.trim()||n.name.length>64||!["area","focus","all"].includes(n.scope)||!nr(n.sources,7)||!n.sources.length||!nr(n.nodeIds,Xn.nodes)||!nr(n.kinds,100)||!nr(n.predicates,100)||typeof n.query!="string"||n.query.length>256||!(n.focusId===null||typeof n.focusId=="string"&&n.focusId.length>0&&n.focusId.length<=1024)||!Number.isInteger(n.depth)||n.depth<0||n.depth>3||!["either","outgoing","incoming"].includes(n.direction)||!["overview","all"].includes(n.profile)||!Number.isInteger(n.limit)||n.limit<1||n.limit>Xn.nodes||typeof n.relations!="boolean")&&_n("Настройки линзы неполны или превышают допустимый размер."),n.scope==="area"&&!n.nodeIds.length&&_n("Исходная область пуста. Выберите поиск по древу."),n.scope==="focus"&&!n.focusId&&_n("Сначала выберите звезду."),n.v===1&&n.conditions!==void 0&&_n("Версия сохранённой линзы не соответствует её условиям.");const e=Uu(n.v===1?{nodes:[],relations:[]}:n.conditions);return{v:2,name:n.name.trim(),scope:n.scope,sources:[...n.sources],nodeIds:[...n.nodeIds],focusId:n.focusId,query:n.query,kinds:[...n.kinds],predicates:[...n.predicates],depth:n.depth,direction:n.direction,profile:n.profile,limit:n.limit,relations:n.relations,conditions:e}}function tl(n){const e=JSON.stringify(Vr(n));return(e.length>12e3||new URLSearchParams({lens:e}).toString().length>4e4)&&_n("Описание линзы слишком велико для ссылки. Сузьте исходную область или сократите значения условий."),e}function jc(n){(typeof n!="string"||n.length>12e3)&&_n("Ссылка на линзу слишком велика.");try{return Vr(JSON.parse(n))}catch(e){if(e instanceof Ft)throw e;_n("Не удалось прочитать настройки линзы.")}}async function nl(n,e){const[t,i]=await Promise.all([n.request("/catalog",{signal:e}),n.request("/contracts",{signal:e})]);os(t);const r=t.capabilities,s=i?.contracts?.lens_spec;return(t.schema!=="tos_knowledge_catalog_v1"||t.authority_boundary?.is_source!==!1||t.authority_boundary?.is_canon!==!1||t.authority_boundary?.writes_to_tree!==!1||i?.schema!=="tos_knowledge_contract_bundle_v1"||i.authority_boundary?.writes_to_tree!==!1||s?.properties?.schema_version?.const!=="tos_lens_spec_v1"||!nr(r?.sources,7)||!r.sources.length||!["in"].every(a=>r.filter_operators?.includes(a))||!r.node_fields?.includes("kind_id")||!r.relation_fields?.includes("predicate_id")||!Array.isArray(t.node_kinds)||!Array.isArray(t.predicates)||t.node_kinds.length>5e3||t.predicates.length>5e3||!nr(t.node_kinds.map(a=>a?.kind_id),5e3)||!nr(t.predicates.map(a=>a?.predicate_id),5e3)||!Array.isArray(s.properties.sources?.items?.enum)||!r.sources.every(a=>s.properties.sources.items.enum.includes(a))||!["nodes","relations","groups","traversal_depth"].every(a=>Number.isInteger(r.maximums?.[a])&&r.maximums[a]>=1)||!r.neighborhood_profiles?.some(a=>a.profile==="all")||!r.neighborhood_profiles?.some(a=>a.profile==="overview")||!["nodes","relations","groups"].every(a=>Number.isInteger(s.properties.limits?.properties?.[a]?.maximum)&&s.properties.limits.properties[a].maximum>=1)||!Number.isInteger(s.properties.traversal?.properties?.depth?.maximum)||r.inclusion?.authority!=="query-execution-not-semantic-proof")&&_n("Сервер пока не предоставляет совместимый конструктор линз."),{catalog:t,schema:s}}function Bu(n,e){return{v:2,name:"Моя линза",scope:n?.nodes?.length?"area":"all",sources:[...e.catalog.capabilities.sources],nodeIds:(n?.nodes||[]).map(t=>t.id),focusId:n?.focus?.node_id||null,query:"",kinds:[],predicates:[],depth:0,direction:"either",profile:"all",limit:Xn.nodes,relations:!0,conditions:{nodes:[],relations:[]}}}function ed(n,{catalog:e,schema:t}){const i=Vr(n),r=e.capabilities,s=(l,c)=>l.every(f=>c.includes(f));(!s(i.sources,r.sources)||!s(i.kinds,e.node_kinds.map(l=>l?.kind_id))||!s(i.predicates,e.predicates.map(l=>l.predicate_id)))&&_n("Словарь данных изменился. Обновите каталог и проверьте выбранные условия.");const a=Math.min(Xn.nodes,r.maximums.nodes,t.properties.limits.properties.nodes.maximum);(i.limit>a||i.depth>Math.min(r.maximums.traversal_depth,t.properties.traversal.properties.depth.maximum))&&_n("Сервер не поддерживает выбранный размер области.");const o={schema_version:"tos_lens_spec_v1",lens_id:Ou,title:i.name,language:"ru",detail:"compact",explain:!0,sources:i.sources,seed:i.scope==="focus"?{focus_node_id:i.focusId}:{text_query:i.query,...i.scope==="area"?{node_ids:i.nodeIds}:{}},node_query:{enabled:i.scope!=="focus",filters:i.kinds.length?[{field:"kind_id",op:"in",value:i.kinds}]:[]},relation_query:{enabled:i.relations,filters:i.predicates.length?[{field:"predicate_id",op:"in",value:i.predicates}]:[]},traversal:{depth:i.depth,direction:i.direction,profile:i.profile},composition:{endpoint_policy:"both"},limits:{nodes:i.limit,relations:i.relations?Math.min(Xn.relations,r.maximums.relations,t.properties.limits.properties.relations.maximum):0,groups:Math.min(8,r.maximums.groups,t.properties.limits.properties.groups.maximum)}};return i.scope!=="focus"&&o.node_query.filters.push(...Ul(i.conditions.nodes,{catalog:e,schema:t},"nodes")),i.relations&&o.relation_query.filters.push(...Ul(i.conditions.relations,{catalog:e,schema:t},"relations")),i.scope==="focus"&&(o.node_query.filters=[]),o}function td(n){da(n);const e=n.counts;return(!/^[a-f0-9]{64}$/.test(n.fingerprint||"")||n.inclusion?.authority!=="query-execution-not-semantic-proof"||!["matched_nodes","eligible_relations","truncated_nodes","truncated_relations"].every(t=>Number.isInteger(e?.[t])&&e[t]>=0)||e.nodes!==n.nodes.length||e.relations!==n.relations.length)&&_n("Сервер не подтвердил состав линзы."),{nodes:n.nodes.length,relations:n.relations.length,matched:e.matched_nodes,context:n.nodes.filter(t=>["traversal","endpoint"].includes(n.inclusion.nodes?.[t.id]?.kind)).length,limited:e.truncated_nodes>0||e.truncated_relations>0}}async function il(n,e,t,i){const r=await n.compile(ed(e,t),i,t.catalog.source_revision);return td(r),Qc.set(r,Vr(e)),r}function nd(n){const e=n.getItem(Jc);if(!e)return[];e.length>15e4&&_n("Сохранённые линзы превышают размер локального хранилища.");try{const t=JSON.parse(e);if(!Array.isArray(t)||t.length>12)throw new Error;return t.map(Vr)}catch{_n("Сохранённые линзы не удалось прочитать. Они остались в хранилище без изменений.")}}function ku(n,e){const t=jc(tl(e)),i=nd(n),r=i.findIndex(s=>s.name===t.name);return r<0?(i.length>=12&&_n("Уже сохранено 12 линз. Дайте этой линзе имя одной из существующих, чтобы обновить её."),i.push(t)):i[r]=t,n.setItem(Jc,JSON.stringify(i)),i}function zu(n,e){const t=i=>{const r=new Set((n?.[i]||[]).map(a=>a.id)),s=new Set(e[i].map(a=>a.id));return{added:[...s].filter(a=>!r.has(a)).length,removed:[...r].filter(a=>!s.has(a)).length}};return{nodes:t("nodes"),relations:t("relations")}}const ji=(n,e,t)=>typeof n=="number"&&Number.isFinite(n)&&n>=e&&n<=t,xa=n=>n===null||typeof n=="string"&&n.length>0&&n.length<=1024,ya=n=>Array.isArray(n)&&n.length===3&&n.every(e=>ji(e,-1e4,1e4));function so(n){if(!n||!["constellations","plane","orbits"].includes(n.lens)||!ji(n.yaw,-1e5,1e5)||!ji(n.pitch,-2,2)||!ji(n.zoom,.1,5)||!ji(n.pan?.x,-1e5,1e5)||!ji(n.pan?.y,-1e5,1e5)||!xa(n.selectedId)||!xa(n.relationId)||typeof n.panelOpen!="boolean"||!["about","relations"].includes(n.cardTab)||!Array.isArray(n.vertices)||n.vertices.length>40)throw new Error("Не удалось прочитать положение сохранённого места.");const e=new Set,t=n.vertices.map(i=>{if(!xa(i.id)||!i.id||e.has(i.id)||!Number.isInteger(i.slot)||i.slot<0||i.slot>1e3||!ya(i.p)||!ya(i.sourcePosition)||!ya(i.target)||!ji(i.volumeZ,-1e4,1e4))throw new Error("Сохранённое расположение звёзд повреждено.");return e.add(i.id),{id:i.id,slot:i.slot,p:[...i.p],sourcePosition:[...i.sourcePosition],target:[...i.target],pos:[...i.target],volumeZ:i.volumeZ}});return{lens:n.lens,yaw:n.yaw,pitch:n.pitch,zoom:n.zoom,pan:{x:n.pan.x,y:n.pan.y},selectedId:n.selectedId,relationId:n.relationId,panelOpen:n.panelOpen,cardTab:n.cardTab,vertices:t}}const rl="tos-observatory-places-v1",$s="tos-observatory-resume-v1",sr=()=>{throw new Error("Сохранённое место не удалось прочитать. Запись осталась в браузере.")};function Gu(n){return(!n||JSON.stringify(n).length>24e3||n.schema_version!=="tos_lens_spec_v1"||!Number.isInteger(n.limits?.nodes)||n.limits.nodes<1||n.limits.nodes>Xn.nodes||!Number.isInteger(n.limits?.relations)||n.limits.relations<0||n.limits.relations>Xn.relations||!Number.isInteger(n.limits?.groups)||n.limits.groups<0||n.limits.groups>8||n.traversal?.depth!==void 0&&(!Number.isInteger(n.traversal.depth)||n.traversal.depth<0||n.traversal.depth>3))&&sr(),Object.fromEntries(["schema_version","lens_id","title","language","detail","explain","sources","seed","node_query","relation_query","traversal","composition","limits"].filter(t=>n[t]!==void 0).map(t=>[t,structuredClone(n[t])]))}function hs(n){(n?.v!==1||typeof n.name!="string"||!n.name.trim()||n.name.length>64||typeof n.id!="string"||!n.id||n.id.length>80||!Number.isFinite(n.savedAt)||!/^[a-f0-9]{64}$/.test(n.sourceRevision||"")||typeof n.route!="string"||n.route.length>5e4)&&sr();const e={v:1,id:n.id,name:n.name.trim(),savedAt:n.savedAt,sourceRevision:n.sourceRevision,route:n.route,spec:Gu(n.spec),draft:n.draft?Vr(n.draft):null,pose:so(n.pose)};return JSON.stringify(e).length>65e3&&sr(),e}function Hu(n,e,{name:t,id:i,route:r,savedAt:s=Date.now()}){n.schema==="tos_exploration_result_v1"?jo(n):da(n);const a=ru(n)||{schema_version:"tos_lens_spec_v1",lens_id:"observatory-saved-area",language:"ru",detail:"compact",explain:!0,seed:{node_ids:n.nodes.map(o=>o.id)},node_query:{enabled:!0},relation_query:{enabled:!!n.relations.length,filters:n.relations.length?[{field:"id",op:"in",value:n.relations.map(o=>o.id)}]:[]},traversal:{depth:0,profile:"all"},limits:{...Xn,groups:8}};return hs({v:1,id:i,name:t,savedAt:s,route:r,sourceRevision:n.source_revision,spec:a,draft:Nr(n),pose:e})}function id(n){const e=n.getItem(rl);if(!e)return[];e.length>8e5&&sr();const t=JSON.parse(e);(!Array.isArray(t)||t.length>12)&&sr();const i=t.map(hs);return new Set(i.map(r=>r.id)).size!==i.length&&sr(),i}function Fl(n,e){const t=hs(e),i=id(n),r=i.findIndex(s=>s.id===t.id);if(r<0){if(i.length>=12)throw new Error("Сохранено 12 мест. Удалите ненужное место, чтобы добавить новое.");i.unshift(t)}else i[r]=t;return n.setItem(rl,JSON.stringify(i)),i}function Vu(n,e){const t=n.getItem($s);if(!t)return null;t.length>65e3&&sr();const i=hs(JSON.parse(t));return!e||e===i.route?i:null}async function Wu(n,e,t){const i=hs(e),r=i.draft?await il(n,i.draft,await nl(n,t),t):await n.compile({...i.spec,explain:!0},t);if(!r.nodes.length)throw new Error("В этом месте больше нет доступных звёзд. Предыдущий вид сохранён.");return{packet:r,pose:i.pose,changed:r.source_revision!==i.sourceRevision}}const rd="tos-observatory-interface-v1",Ol=["search","lenses","workspace","navigation","builder","evidence","sources","reader"],sl={v:1,pinned:["search","lenses","workspace","navigation"],dock:"auto",text:"comfortable",labels:"normal",sizes:{}},Xu=["inspector","workspace","evidence","navigation","builder","studio","reader"];function sd(n){if(n?.v!==1||!Array.isArray(n.pinned)||n.pinned.length>Ol.length||new Set(n.pinned).size!==n.pinned.length||n.pinned.some(t=>!Ol.includes(t))||!["auto","left","right"].includes(n.dock)||!["comfortable","large"].includes(n.text)||!["normal","large"].includes(n.labels)||!n.sizes||typeof n.sizes!="object")throw new Error("Настройки интерфейса не удалось прочитать.");const e={};for(const t of Xu){const i=n.sizes[t];if(i){if(!Number.isFinite(i.width)||i.width<280||i.width>760||!Number.isFinite(i.height)||i.height<240||i.height>800)throw new Error("Сохранённый размер окна повреждён.");e[t]={width:i.width,height:i.height}}}return{v:1,pinned:[...n.pinned],dock:n.dock,text:n.text,labels:n.labels,sizes:e}}function qu(n){const e=n?.getItem(rd);if(!e)return structuredClone(sl);if(e.length>5e3)throw new Error("Настройки интерфейса слишком велики.");return sd(JSON.parse(e))}const rn=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Fn=(n,e)=>{const t=rn("button",n);return t.type="button",t.addEventListener("click",e),t};function Yu(n,e,t,{data:{client:i},initialRoute:r,onUserAction:s}){const a=new cr;let o=null,l=[],c="",f="",u="places",d=!1,h=!1,_=!1,x=!0,m=null,p=null,C=null,D=null,M=0;try{o=localStorage,l=id(o)}catch(X){c=X.message}const L=Fn("",()=>{s(),G()});L.className="sc-control sc-studio-open",L.setAttribute("aria-label","Места и инструменты"),L.setAttribute("aria-expanded","false"),L.innerHTML='<i data-lucide="bookmark" aria-hidden="true"></i><span>Моё пространство</span>',n.querySelector(".sc-header-actions").append(L);const A=rn("section","","sc-panel sc-studio");A.hidden=!0,A.setAttribute("aria-label","Места и инструменты"),A.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">МОЁ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-studio-close" aria-label="Закрыть места и инструменты"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Места мысли</h3><div class="sc-studio-tabs" role="tablist" aria-label="Рабочее окружение"></div><div class="sc-studio-body" role="tabpanel" tabindex="0"></div><p class="sc-studio-status" role="status"></p>',n.append(A);const P=A.querySelector(".sc-studio-body"),v=A.querySelector(".sc-studio-status");P.id="sc-studio-content";const b=Fn("Повторить открытие места",()=>{s(),k(D)});b.className="sc-studio-retry",b.hidden=!0,v.after(b);function w(){M++,a.cancelAll(),d=!1}t.register("studio",A,()=>{w(),L.setAttribute("aria-expanded","false")}),t.configure("studio",{onResume:ie});const E=["places","tools"].map((X,he)=>{const _e=Fn(he?"Инструменты":"Места",()=>{s(),w(),u=X,ie()});return _e.id="sc-studio-"+X,_e.setAttribute("role","tab"),_e.setAttribute("aria-controls",P.id),A.querySelector(".sc-studio-tabs").append(_e),_e.addEventListener("keydown",we=>{if(["ArrowLeft","ArrowRight","Home","End"].includes(we.key)){we.preventDefault();const ee=we.key==="Home"?0:we.key==="End"?1:1-he;E[ee].click(),E[ee].focus()}}),_e});function N(){t.close("studio"),L.focus()}A.querySelector(".sc-studio-close").addEventListener("click",N),A.addEventListener("keydown",X=>{X.key==="Escape"&&(X.preventDefault(),X.stopPropagation(),N())});function G(){t.open("studio"),L.setAttribute("aria-expanded","true"),ie(),E[u==="places"?0:1].focus()}function W(X){c=X.message||"Не удалось выполнить действие.",K()}function I(X){try{s(),X()}catch(he){W(he)}}const R=()=>mt(e.port.node(e.port.selection.nodeId||e.port.packet?.focus?.node_id)?.display.title,"Моё место");function F(X,he){if(!e.port.packet?.nodes.length)throw new Error("Сначала дождитесь загрузки области.");return Hu(e.port.packet,e.port.capturePlace(),{name:X.slice(0,64),id:he,route:location.search})}function J(X,he){if(!o)throw new Error("Локальное хранилище недоступно.");l=Fl(o,F(X,he)),c="",f="Место сохранено в этом браузере.",ie()}function B(){if(!(!_||!x||h||d||!o||!e.port.packet?.nodes.length))try{const X=JSON.stringify(F(R(),"resume"));o.getItem($s)!==X&&o.setItem($s,X)}catch(X){c="Не удалось сохранить последний вид. "+X.message,K()}}function oe(){clearTimeout(m),m=setTimeout(B,800)}async function k(X,{initial:he=!1}={}){w();const _e=M;e.ui.cancelPending(),d=!0,D=X,c="",f="Возвращаюсь к месту…",K();try{const we=await a.run("place",ee=>Wu(i,X,ee));if(!we.current||_e!==M)return!1;h=!0;try{e.port.setGraph(we.value.packet,{initial:he}),e.port.restorePlace(we.value.pose)}finally{h=!1}return f=we.value.changed?"Место открыто. Данные обновились с последнего посещения.":"Место открыто.",D=null,x=!0,d=!1,oe(),!he&&!A.hidden&&ie(),e.port.announce(f),!0}catch(we){return _e===M&&W(we),!1}finally{_e===M&&(d=!1,K())}}function K(){v.textContent=c||f||t.storageError||"",A.setAttribute("aria-busy",String(d)),P.querySelectorAll("[data-place-action]").forEach(X=>X.disabled=d),b.hidden=u!=="places"||!D||!c,b.disabled=d}function Z(X,he){const _e=rn("label",X,"sc-studio-field");return _e.append(he),_e}function Y(){P.append(rn("p","Сохраните область, линзу и ракурс. При возвращении данные проверяются заново.","sc-studio-note"));const X=rn("form"),he=rn("input");he.type="text",he.maxLength=64,he.value=R().slice(0,64),he.required=!0,he.setAttribute("aria-label","Название места");const _e=rn("button","Сохранить текущее место");_e.type="submit",_e.dataset.placeAction="save",_e.disabled=d||!e.port.packet,X.append(Z("Название места",he),_e),X.addEventListener("submit",ee=>{ee.preventDefault(),I(()=>J(he.value.trim(),crypto.randomUUID()))}),P.append(X);for(const ee of l){const ce=rn("article","","sc-place"),$=Fn(ee.name,()=>{s(),k(ee)});$.className="sc-place-open",$.dataset.placeAction="open",ce.append($,rn("small",new Date(ee.savedAt).toLocaleDateString("ru",{day:"numeric",month:"long"})+" · "+ee.pose.vertices.length+" звёзд при сохранении"));const pe=rn("div","","sc-place-actions");pe.append(Fn("Обновить этим видом",()=>I(()=>J(ee.name,ee.id))),Fn("Удалить",()=>I(()=>{C=ee,l=l.filter(ye=>ye.id!==ee.id),o.setItem(rl,JSON.stringify(l)),f="Место удалено.",ie()}))),ce.append(pe),P.append(ce)}C&&P.append(Fn("Вернуть удалённое место",()=>I(()=>{l=Fl(o,C),C=null,f="Место восстановлено.",ie()}))),l.length||P.append(rn("p","Здесь появятся места, к которым хочется вернуться.","sc-studio-empty"));const we=Fn("Забыть последний вид",()=>I(()=>{o?.removeItem($s),x=!1,f="Автовозврат отключён до следующего открытия страницы.",K()}));we.className="sc-studio-subtle",P.append(rn("p","Последний вид запоминается автоматически в этом браузере.","sc-studio-note"),we)}function ne(X,he,_e){const we=rn("select");for(const[ee,ce]of _e){const $=rn("option",ce);$.value=ee,we.append($)}return we.value=t.preferences[he],we.addEventListener("change",()=>I(()=>{t.setPreferences({...t.preferences,[he]:we.value}),K()})),Z(X,we)}function H(){P.append(rn("p","Закрепите нужное в верхней панели. Все инструменты доступны и отсюда.","sc-studio-note"));const X=t.preferences,he=t.toolList(),_e=[...X.pinned,...he.map(we=>we.id).filter(we=>!X.pinned.includes(we))];for(const we of _e){const ee=he.find(ye=>ye.id===we);if(!ee)continue;const ce=rn("div","","sc-tool-choice"),$=rn("input");$.type="checkbox",$.checked=X.pinned.includes(we),$.setAttribute("aria-label","Закрепить: "+ee.title),$.addEventListener("change",()=>I(()=>{const ye=t.preferences;t.setPreferences({...ye,pinned:$.checked?[...ye.pinned,we]:ye.pinned.filter(Te=>Te!==we)}),te(we)}));const pe=Fn(ee.title,()=>I(()=>t.launch(we)));if(pe.disabled=!ee.available,ce.append($,pe),$.checked){const ye=Fn("↑",()=>I(()=>{const Te=t.preferences,Pe=Te.pinned.indexOf(we);Pe>0&&([Te.pinned[Pe-1],Te.pinned[Pe]]=[Te.pinned[Pe],Te.pinned[Pe-1]],t.setPreferences(Te),te(we))}));ye.setAttribute("aria-label","Выше: "+ee.title),ye.disabled=X.pinned[0]===we,ce.append(ye)}P.append(ce)}P.append(ne("Сторона окна","dock",[["auto","По свободному месту"],["left","Слева"],["right","Справа"]]),ne("Текст для чтения","text",[["comfortable","Обычный"],["large","Крупнее"]]),ne("Подписи звёзд","labels",[["normal","Обычные"],["large","Крупнее"]])),P.append(Fn("Восстановить исходное расположение",()=>I(()=>{t.setPreferences(structuredClone(sl)),f="Исходные настройки восстановлены.",ie()}))),P.append(rn("p","Размер окна меняется кнопкой ↔ или за угол. На клавиатуре: стрелки на уголке, Home — сброс.","sc-studio-note"))}function te(X){ie(),[...P.querySelectorAll("input")].find(he=>he.getAttribute("aria-label")==="Закрепить: "+t.toolList().find(_e=>_e.id===X)?.title)?.focus()}function ie(){const X=P.scrollTop;P.replaceChildren(),A.querySelector("h3").textContent=u==="places"?"Места мысли":"Мои инструменты",E.forEach((he,_e)=>{const we=u===(_e?"tools":"places");he.setAttribute("aria-selected",String(we)),he.tabIndex=we?0:-1}),P.setAttribute("aria-labelledby","sc-studio-"+u),u==="places"?Y():H(),P.scrollTop=X,K(),e.invalidate()}return n.addEventListener("pointerdown",X=>{d&&!A.contains(X.target)&&(w(),f="Возвращение прервано вашим действием.",K())},{capture:!0}),n.addEventListener("wheel",X=>{X.target.closest(".sc-panel")||(d&&(w(),f="Возвращение прервано вашим действием.",K()),oe())},{passive:!0}),n.addEventListener("keydown",X=>{d&&!A.contains(X.target)&&!["Tab","Shift","Control","Meta","Alt"].includes(X.key)&&(w(),f="Возвращение прервано вашим действием.",K())},{capture:!0}),n.addEventListener("pointerup",oe,{passive:!0}),n.addEventListener("keyup",oe,{passive:!0}),window.addEventListener("pagehide",()=>{clearTimeout(m),B(),w()}),document.addEventListener("visibilitychange",()=>{document.hidden&&B()}),yi(),{selectionChanged(){!h&&e.port.packet!==p&&(d&&w(),p=e.port.packet),oe()},async start(){let X=null;try{o&&(X=Vu(o,r))}catch(he){x=!1,W(he)}if(X){const he=await k(X,{initial:!0});if(_=!0,he)return e.ui.start({skipScene:!0}),oe(),!0;x=!1,G()}return _=!0,e.ui.start(),!1}}}function $u(n,e){if(n?.inclusion?.authority!=="query-execution-not-semantic-proof")return"";const t=n.inclusion.nodes?.[e];if(t?.kind==="selector")return"Эта звезда соответствует условиям выбора исходных узлов.";if(t?.kind==="focus")return"Центр выбранной области.";if(!["traversal","endpoint"].includes(t?.kind))return"";const i=n.relations.find(a=>a.id===t.via_relation_id),r=n.nodes.find(a=>a.id===t.via_node_id),s=["Эта звезда добавлена окружением; условия исходных узлов не обязаны выполняться."];return i&&s.push("Через связь «"+mt(i.display?.label,i.id)+"»"+(r?" с «"+mt(r.display?.title,r.id)+"»":"")+"."),Number.isInteger(t.depth)&&t.depth>0&&s.push("Шаг от исходной области: "+t.depth+"."),s.join(" ")}function Ku(n){const e=new Map;if(n?.inclusion?.authority!=="query-execution-not-semantic-proof")return e;for(const t of n.nodes){const i=n.inclusion.nodes?.[t.id]?.kind,r=i==="selector"?"matched":i==="focus"?"focus":["traversal","endpoint"].includes(i)?"context":null;r&&e.set(t.id,r)}return e}function Zu(n,e){const t=document.createElement("div");t.className="sc-inclusion-legend",t.hidden=!0,t.title="Причина появления в области; не оценка истинности или значимости",n.querySelector(".sc-context").append(t);let i=null,r=null,s=null,a=0;function o(){s&&(s.querySelector(".sc-inclusion-hint").hidden=!0,delete s.dataset.hintOpen,s=null)}function l(u){if(!u||u.hidden||u===s)return;o();const d=u.querySelector(".sc-inclusion-hint");if(!d)return;s=u,d.hidden=!1,u.dataset.hintOpen="true";const h=n.getBoundingClientRect(),_=u.getBoundingClientRect(),x=d.getBoundingClientRect(),m=Math.max(h.left+12,Math.min(_.left+(_.width-x.width)/2,h.right-x.width-12)),p=_.bottom+22,C=p+x.height<h.bottom-90?p:_.top-x.height-22;d.style.left=m-_.left+"px",d.style.top=Math.max(h.top+90,C)-_.top+"px"}n.addEventListener("pointerover",u=>l(u.target.closest?.(".sc-node"))),n.addEventListener("pointerout",u=>{s?.contains(u.target)&&!s.contains(u.relatedTarget)&&!s.contains(document.activeElement)&&o()}),n.addEventListener("focusin",u=>l(u.target.closest?.(".sc-node"))),n.addEventListener("focusout",u=>{s?.contains(u.target)&&!s.contains(u.relatedTarget)&&o()}),window.addEventListener("keydown",u=>{u.key==="Escape"&&s&&(o(),u.preventDefault(),u.stopPropagation())},{capture:!0}),n.addEventListener("pointerdown",o,{capture:!0}),n.addEventListener("click",o),n.addEventListener("wheel",o,{passive:!0}),window.addEventListener("resize",o);function c(){const u=e.port.packet;if(!u||u===i)return;o(),clearTimeout(r);const d=new Set(i?.nodes.map(x=>x.id)||[]),h=Ku(u),_={matched:0,focus:0,context:0};for(const x of n.querySelectorAll(".sc-node")){const m=h.get(x.dataset.id);x.dataset.inclusion=m||"",x.dataset.entering=String(!!(i&&!d.has(x.dataset.id)));let p=x.querySelector(".sc-inclusion-hint");m?(_[m]++,p||(p=document.createElement("span"),p.className="sc-inclusion-hint",p.id="sc-inclusion-hint-"+ ++a,p.setAttribute("role","tooltip"),x.append(p)),p.hidden=!0,p.textContent=$u(u,x.dataset.id),x.setAttribute("aria-describedby",p.id)):(p?.remove(),x.removeAttribute("aria-describedby"))}t.replaceChildren();for(const[x,m]of[["matched","По условиям"],["focus","Центр"],["context","Окружение"]])if(_[x]){const p=document.createElement("span");p.dataset.role=x,p.textContent=m+" "+_[x],t.append(p)}t.hidden=!h.size,i=u,e.invalidate(),r=setTimeout(()=>{for(const x of n.querySelectorAll('.sc-node[data-entering="true"]'))x.dataset.entering="false"},1600)}return new MutationObserver(c).observe(n,{attributes:!0,attributeFilter:["data-graph-revision","data-node-count","data-relation-count"]}),c(),window.addEventListener("pagehide",()=>{o(),clearTimeout(r)}),{update:c}}function Ju(n,e,{client:t,initialFocus:i=to,initialLens:r}={}){const s=R=>n.querySelector(R),a=new cr,o=new Map;let l=0,c=null,f=0;const u=Br(s("#so-about")),d=Br(s("#so-relations")),h=(R,F,J)=>{const B=document.createElement(R);return B.className=F,B.textContent=J,B};function _(R,F,J="sc-neighbor"){const B=h("button",J,R);return B.type="button",B.addEventListener("click",F),B}function x(R,F=null){s(".sc-data-notice").hidden=!R,s(".sc-data-notice span").textContent=R,s(".sc-retry").hidden=!F,c=F}s(".sc-retry").addEventListener("click",()=>c?.());function m(){a.cancel("inspect")}function p(){clearTimeout(l),a.cancel("search")}function C(){clearTimeout(l),a.cancelAll(),x("")}function D(){a.cancel("scene"),m(),x(""),e.packet&&(n.dataset.dataState="ready")}function M(R,F){x(R.message||"Связь с данными прервалась.",F),e.announce(R.message)}function L(R,F){o.delete(R),o.set(R,F),o.size>48&&o.delete(o.keys().next().value)}async function A(R,{expected:F=null,initial:J=!1,depth:B=1,selectFocus:oe=!0}={}){const k=ta(R,{depth:B});x("Получаю окрестность…"),n.dataset.dataState="loading";try{const K=await a.run("scene",Z=>t.compile(k,Z,F));if(!K.current)return;e.setGraph(K.value,{initial:J,selectFocus:oe}),n.dataset.dataState="ready",x(""),oe&&s("#so-about-tab").focus(),e.announce("Область загружена. Узлов: "+K.value.nodes.length+". Связей: "+K.value.relations.length+".")}catch(K){n.dataset.dataState="error",M(K,()=>A(R,{initial:J,depth:B,selectFocus:oe}))}}function P(R,F){if(F===e.packet?.source_revision&&e.node(R)){e.selectNode(R),s("#so-about-tab").focus();return}A(R,{expected:F,selectFocus:!0})}async function v(R,F){if(F===e.packet?.source_revision&&e.relation(R.id)){e.selectRelation(R.id);return}x("Открываю отношение…"),n.dataset.dataState="loading";try{const J=await a.run("scene",async B=>{const{match:oe}=await t.inspect("relation",R.id,B,F,R.content_revision),k=Vc(oe),K=await t.compile(k,B,F);if(!K.relations.some(Z=>Z.id===oe.id))throw new Ft("Выбранное отношение отсутствует в области.");return{packet:K}});if(!J.current)return;e.setGraph(J.value.packet),e.selectRelation(R.id,{rememberView:!1}),n.dataset.dataState="ready",x("")}catch(J){n.dataset.dataState="error",M(J,()=>v(R,null))}}function b(R,F,J){const B=mt(F==="node"?R.display.title:R.display.label,R.id),oe=_("",()=>F==="node"?P(R.id,J):v(R,J),"sc-result"),k=h("span","sc-result-label",B);if(F==="relation"){const K=mt(R.display.statement,R.from_id+" → "+R.to_id);k.append(h("span","sc-result-detail",K)),oe.setAttribute("aria-label",B+" · "+K)}return oe.append(k,h("small","",F==="node"?mt(R.display.kind_label):"Отношение")),oe}function w(R,F=0){clearTimeout(l),a.cancel("search"),f=F;const J=R.trim().slice(0,256),B=s(".sc-search-results");if(B.replaceChildren(),!J){B.append(h("div","sc-section-label","В ТЕКУЩЕЙ ОБЛАСТИ"));for(const oe of(e.packet?.nodes||[]).filter(k=>k.id===e.packet?.focus?.node_id||k.kind_id==="agent").slice(0,6))B.append(b(oe,"node",e.packet.source_revision));B.append(h("div","sc-empty","Введите имя, название или понятие для поиска во всём древе.")),e.cardChanged();return}B.append(h("div","sc-empty","Ищу в древе…")),e.cardChanged(),l=setTimeout(async()=>{try{const oe=await a.run("search",Z=>t.search(J,Z,F));if(!oe.current||s(".sc-search").hidden)return;const k=oe.value;B.replaceChildren(),n.dataset.searchQuery=J,n.dataset.searchRevision=k.source_revision;for(const Z of["node","relation"]){const Y=k[Z==="node"?"nodes":"relations"];if(Y.length){B.append(h("div","sc-section-label",Z==="node"?"УЗЛЫ":"ОТНОШЕНИЯ"));for(const ne of Y)B.append(b(ne,Z,k.source_revision))}}!k.nodes.length&&!k.relations.length&&B.append(h("div","sc-empty","По этому запросу ничего не найдено."));const K=document.createElement("div");K.className="sc-search-pager",F>0&&K.append(_("Ранее",()=>w(J,Math.max(0,F-6)))),Math.max(k.counts.matching_nodes,k.counts.matching_relations)>F+6&&K.append(_("Далее",()=>w(J,F+6))),B.append(K),e.announce("Результаты поиска обновлены."),e.cardChanged()}catch(oe){if(s(".sc-search").hidden)return;B.replaceChildren(h("div","sc-empty",oe.message),_("Повторить поиск",()=>w(J,f))),e.cardChanged()}},180)}function E(R,F){const J=s(".sc-provenance");J.replaceChildren();const B=F==="node"?R.display.summary_state:R.display.explanation_state,oe={authored:"Авторское описание","source-derived":"Описание из источника","metadata-synthesis":"Описание составлено из метаданных",missing:"Описание пока не зафиксировано"};J.append(h("p","sc-description-origin",oe[B]||"Происхождение описания не указано"));const k=F==="node"?R.display.summary:R.display.explanation;!k?.ru&&k?.default&&J.append(h("p","sc-description-origin","Показан исходный язык описания."));const K=document.createElement("details");K.className="sc-source-details",K.append(h("summary","",`Источники и статус · ${R.source_refs.length}`));const Z=R.epistemic||{};for(const[Y,ne]of[["Слой",Z.authority_layer],["Рассмотрение",Z.review_posture],["Канон",Z.canon_status]])K.append(h("p","sc-source-status",Y+": "+(!ne||ne==="not-recorded"?"не указан":ne)));for(const Y of R.source_refs)if(/^https?:\/\//i.test(Y))try{const ne=new URL(Y),H=h("a","sc-source-ref",ne.hostname+ne.pathname);H.href=ne.href,H.target="_blank",H.rel="noreferrer noopener",K.append(H)}catch{K.append(h("span","sc-source-ref",Y))}else K.append(h("span","sc-source-ref",Y));K.addEventListener("toggle",()=>e.cardChanged()),J.append(K)}function N(R){return mt(e.node(R)?.display?.title,R)}function G(R,F,J=[]){const B=JSON.stringify([e.packet?.source_revision,R,F.id,F.content_revision]);u.capture(),d.capture(),u.enter(B),d.enter(B),s(".sc-node-title").textContent=mt(R==="node"?F.display.title:F.display.label,F.id),s(".sc-node-original").textContent=R==="node"?F.display.title.original||F.display.title.en||"":mt(F.display.statement),s(".sc-kind").textContent=R==="node"?mt(F.display.kind_label,F.kind_id).toUpperCase():"ОТНОШЕНИЕ",s(".sc-description").textContent=mt(R==="node"?F.display.summary:F.display.explanation,"Описание пока не зафиксировано."),s(".sc-inspector").setAttribute("aria-label",R==="node"?"Выбранный узел":"Выбранное отношение"),n.dataset.inspectorKind=R,n.dataset.inspectorId=F.id;const oe=R==="node"?e.neighbors(F.id):[F];s("#so-relations-tab").firstChild.textContent=R==="node"?"Связи ":"Участники ",s(".sc-neighbor-count").textContent=String(R==="node"?oe.length:new Set([F.from_id,F.to_id]).size);const k=s(".sc-neighbors");if(k.replaceChildren(),R==="node"){for(const K of oe){const Z=K.from_id===F.id,Y=Z?K.to_id:K.from_id,ne=Z?mt(K.display.label):mt(K.display.inverse_label)||"← "+mt(K.display.label),H=_("",()=>e.selectRelation(K.id),"sc-neighbor sc-relation-row");H.append(h("small","",ne),h("span","",N(Y))),k.append(H)}oe.length||k.append(h("p","sc-empty","В этой области связи не показаны."))}else for(const[K,Z]of[["От",F.from_id],["К",F.to_id]]){const Y=J.find(H=>H.id===Z)||e.node(Z),ne=_(K+": "+mt(Y?.display.title,Z),()=>P(Z,e.packet.source_revision),"sc-neighbor sc-relation-row");k.append(ne)}E(F,R),s(".sc-provenance").append(_("Основания и прочтения",()=>n.dispatchEvent(new CustomEvent("sophia-evidence",{detail:{raw:F,kind:R}}))),_("Открыть источники",()=>n.dispatchEvent(new CustomEvent("sophia-sources",{detail:{raw:F,kind:R}}))),_(R==="node"?"Проложить маршрут":"Другой путь",()=>n.dispatchEvent(new CustomEvent("sophia-navigate",{detail:{raw:F,kind:R,tab:"paths"}})))),u.restore(),d.restore(),e.cardChanged()}async function W(R,F){m();const J=e.packet?.source_revision;if(!J)return;G(R,F),n.dataset.inspectorState="loading";const B=[J,R,F.id,F.content_revision].join("|");if(o.has(B)){const oe=o.get(B);G(R,oe.match,oe.packet.endpoints),n.dataset.inspectorState="ready";return}try{const oe=await a.run("inspect",k=>t.inspect(R,F.id,k,J,F.content_revision));if(!oe.current)return;L(B,oe.value),G(R,oe.value.match,oe.value.packet.endpoints),n.dataset.inspectorState="ready"}catch(oe){n.dataset.inspectorState="error",s(".sc-provenance").prepend(h("p","sc-inspection-error",oe.message)),oe instanceof Ei?M(oe,()=>A(e.packet.focus?.node_id||F.id,{selectFocus:!1})):s(".sc-provenance").prepend(_("Загрузить карточку ещё раз",()=>W(R,F))),e.cardChanged()}}s(".sc-open-neighborhood").addEventListener("click",()=>{const R=e.node(e.selection.nodeId);R&&n.dispatchEvent(new CustomEvent("sophia-navigate",{detail:{raw:R,kind:"node",tab:"neighbors"}}))}),s(".sc-read-selected").addEventListener("click",()=>{const R=e.selection,F=R.relationId?"relation":"node",J=F==="relation"?e.relation(R.relationId):e.node(R.nodeId);J&&n.dispatchEvent(new CustomEvent("sophia-read",{detail:{raw:J,kind:F}}))});async function I(){n.dataset.dataState="loading",x("Открываю линзу…");try{const R=await a.run("scene",async F=>{const J=jc(r),B=await nl(t,F);return il(t,J,B,F)});if(!R.current)return;e.setGraph(R.value,{initial:!0}),x(""),e.announce("Линза загружена. Узлов: "+R.value.nodes.length+". Связей: "+R.value.relations.length+".")}catch(R){n.dataset.dataState="error",M(R,I)}}return addEventListener("pagehide",C),{captureReading:()=>{u.capture(),d.capture()},restoreReading:()=>{u.restore(),d.restore()},loadFocus:A,chooseNode:P,chooseRelation:v,searchRow:b,search:w,showCard:W,cancelSearch:p,cancelInspector:m,cancelPending:C,willSelect:D,async start({skipScene:R=!1}={}){R||(r?I():A(i,{initial:!0,selectFocus:!1}));try{const F=await a.run("capabilities",J=>t.capabilities(J));F.current&&(n.dataset.explorationAvailable=String(F.value.available===!0))}catch{n.dataset.explorationAvailable="false"}}}}const al="185",Qu=0,Bl=1,ju=2,Ks=1,ef=2,ss=3,Gi=0,vn=1,gi=2,bi=0,Ur=1,kl=2,zl=3,Gl=4,ad=5,Bi=100,tf=101,nf=102,rf=103,sf=104,af=200,ao=201,of=202,od=203,oo=204,cs=205,lf=206,cf=207,df=208,uf=209,ff=210,hf=211,pf=212,mf=213,gf=214,lo=0,co=1,uo=2,kr=3,fo=4,ho=5,po=6,mo=7,ld=0,_f=1,vf=2,Vn=0,cd=1,dd=2,ud=3,fd=4,hd=5,pd=6,md=7,gd=300,ar=301,zr=302,ba=303,Sa=304,ua=306,go=1e3,xi=1001,_o=1002,on=1003,xf=1004,Ss=1005,ln=1006,Ma=1007,ir=1008,In=1009,_d=1010,vd=1011,ds=1012,ol=1013,ii=1014,ei=1015,wi=1016,ll=1017,cl=1018,us=1020,xd=35902,yd=35899,bd=1021,Sd=1022,Hn=1023,Ti=1026,rr=1027,Md=1028,dl=1029,or=1030,ul=1031,fl=1033,Zs=33776,Js=33777,Qs=33778,js=33779,vo=35840,xo=35841,yo=35842,bo=35843,So=36196,Mo=37492,Eo=37496,wo=37488,To=37489,ia=37490,Ao=37491,Ro=37808,Co=37809,Po=37810,Lo=37811,Io=37812,Do=37813,No=37814,Uo=37815,Fo=37816,Oo=37817,Bo=37818,ko=37819,zo=37820,Go=37821,Ho=36492,Vo=36494,Wo=36495,Xo=36283,qo=36284,ra=36285,Yo=36286,yf=3200,Hl=0,bf=1,_i="",Ln="srgb",fs="srgb-linear",sa="linear",Dt="srgb",_r=7680,Vl=519,Sf=512,Mf=513,Ef=514,hl=515,wf=516,Tf=517,pl=518,Af=519,$o=35044,Rf=35048,Ko="300 es",ti=2e3,aa=2001;function Cf(n){for(let e=n.length-1;e>=0;--e)if(n[e]>=65535)return!0;return!1}function oa(n){return document.createElementNS("http://www.w3.org/1999/xhtml",n)}function Pf(){const n=oa("canvas");return n.style.display="block",n}const Wl={};function la(...n){const e="THREE."+n.shift();console.log(e,...n)}function Ed(n){const e=n[0];if(typeof e=="string"&&e.startsWith("TSL:")){const t=n[1];t&&t.isStackTrace?n[0]+=" "+t.getLocation():n[1]='Stack trace not available. Enable "THREE.Node.captureStackTrace" to capture stack traces.'}return n}function st(...n){n=Ed(n);const e="THREE."+n.shift();{const t=n[0];t&&t.isStackTrace?console.warn(t.getError(e)):console.warn(e,...n)}}function Et(...n){n=Ed(n);const e="THREE."+n.shift();{const t=n[0];t&&t.isStackTrace?console.error(t.getError(e)):console.error(e,...n)}}function Fr(...n){const e=n.join(" ");e in Wl||(Wl[e]=!0,st(...n))}function Lf(n,e,t){return new Promise(function(i,r){function s(){switch(n.clientWaitSync(e,n.SYNC_FLUSH_COMMANDS_BIT,0)){case n.WAIT_FAILED:r();break;case n.TIMEOUT_EXPIRED:setTimeout(s,t);break;default:i()}}setTimeout(s,t)})}const If={[lo]:co,[uo]:po,[fo]:mo,[kr]:ho,[co]:lo,[po]:uo,[mo]:fo,[ho]:kr};class dr{addEventListener(e,t){this._listeners===void 0&&(this._listeners={});const i=this._listeners;i[e]===void 0&&(i[e]=[]),i[e].indexOf(t)===-1&&i[e].push(t)}hasEventListener(e,t){const i=this._listeners;return i===void 0?!1:i[e]!==void 0&&i[e].indexOf(t)!==-1}removeEventListener(e,t){const i=this._listeners;if(i===void 0)return;const r=i[e];if(r!==void 0){const s=r.indexOf(t);s!==-1&&r.splice(s,1)}}dispatchEvent(e){const t=this._listeners;if(t===void 0)return;const i=t[e.type];if(i!==void 0){e.target=this;const r=i.slice(0);for(let s=0,a=r.length;s<a;s++)r[s].call(this,e);e.target=null}}}const cn=["00","01","02","03","04","05","06","07","08","09","0a","0b","0c","0d","0e","0f","10","11","12","13","14","15","16","17","18","19","1a","1b","1c","1d","1e","1f","20","21","22","23","24","25","26","27","28","29","2a","2b","2c","2d","2e","2f","30","31","32","33","34","35","36","37","38","39","3a","3b","3c","3d","3e","3f","40","41","42","43","44","45","46","47","48","49","4a","4b","4c","4d","4e","4f","50","51","52","53","54","55","56","57","58","59","5a","5b","5c","5d","5e","5f","60","61","62","63","64","65","66","67","68","69","6a","6b","6c","6d","6e","6f","70","71","72","73","74","75","76","77","78","79","7a","7b","7c","7d","7e","7f","80","81","82","83","84","85","86","87","88","89","8a","8b","8c","8d","8e","8f","90","91","92","93","94","95","96","97","98","99","9a","9b","9c","9d","9e","9f","a0","a1","a2","a3","a4","a5","a6","a7","a8","a9","aa","ab","ac","ad","ae","af","b0","b1","b2","b3","b4","b5","b6","b7","b8","b9","ba","bb","bc","bd","be","bf","c0","c1","c2","c3","c4","c5","c6","c7","c8","c9","ca","cb","cc","cd","ce","cf","d0","d1","d2","d3","d4","d5","d6","d7","d8","d9","da","db","dc","dd","de","df","e0","e1","e2","e3","e4","e5","e6","e7","e8","e9","ea","eb","ec","ed","ee","ef","f0","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff"],Ea=Math.PI/180,Zo=180/Math.PI;function zi(){const n=Math.random()*4294967295|0,e=Math.random()*4294967295|0,t=Math.random()*4294967295|0,i=Math.random()*4294967295|0;return(cn[n&255]+cn[n>>8&255]+cn[n>>16&255]+cn[n>>24&255]+"-"+cn[e&255]+cn[e>>8&255]+"-"+cn[e>>16&15|64]+cn[e>>24&255]+"-"+cn[t&63|128]+cn[t>>8&255]+"-"+cn[t>>16&255]+cn[t>>24&255]+cn[i&255]+cn[i>>8&255]+cn[i>>16&255]+cn[i>>24&255]).toLowerCase()}function bt(n,e,t){return Math.max(e,Math.min(t,n))}function Df(n,e){return(n%e+e)%e}function wa(n,e,t){return(1-t)*n+t*e}function jn(n,e){switch(e.constructor){case Float32Array:return n;case Uint32Array:return n/4294967295;case Uint16Array:return n/65535;case Uint8Array:return n/255;case Int32Array:return Math.max(n/2147483647,-1);case Int16Array:return Math.max(n/32767,-1);case Int8Array:return Math.max(n/127,-1);default:throw new Error("THREE.MathUtils: Invalid component type.")}}function Ut(n,e){switch(e.constructor){case Float32Array:return n;case Uint32Array:return Math.round(n*4294967295);case Uint16Array:return Math.round(n*65535);case Uint8Array:return Math.round(n*255);case Int32Array:return Math.round(n*2147483647);case Int16Array:return Math.round(n*32767);case Int8Array:return Math.round(n*127);default:throw new Error("THREE.MathUtils: Invalid component type.")}}const xl=class xl{constructor(e=0,t=0){this.x=e,this.y=t}get width(){return this.x}set width(e){this.x=e}get height(){return this.y}set height(e){this.y=e}set(e,t){return this.x=e,this.y=t,this}setScalar(e){return this.x=e,this.y=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;default:throw new Error("THREE.Vector2: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;default:throw new Error("THREE.Vector2: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y)}copy(e){return this.x=e.x,this.y=e.y,this}add(e){return this.x+=e.x,this.y+=e.y,this}addScalar(e){return this.x+=e,this.y+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this}subScalar(e){return this.x-=e,this.y-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this}multiply(e){return this.x*=e.x,this.y*=e.y,this}multiplyScalar(e){return this.x*=e,this.y*=e,this}divide(e){return this.x/=e.x,this.y/=e.y,this}divideScalar(e){return this.multiplyScalar(1/e)}applyMatrix3(e){const t=this.x,i=this.y,r=e.elements;return this.x=r[0]*t+r[3]*i+r[6],this.y=r[1]*t+r[4]*i+r[7],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this}clamp(e,t){return this.x=bt(this.x,e.x,t.x),this.y=bt(this.y,e.y,t.y),this}clampScalar(e,t){return this.x=bt(this.x,e,t),this.y=bt(this.y,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(bt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this}negate(){return this.x=-this.x,this.y=-this.y,this}dot(e){return this.x*e.x+this.y*e.y}cross(e){return this.x*e.y-this.y*e.x}lengthSq(){return this.x*this.x+this.y*this.y}length(){return Math.sqrt(this.x*this.x+this.y*this.y)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)}normalize(){return this.divideScalar(this.length()||1)}angle(){return Math.atan2(-this.y,-this.x)+Math.PI}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const i=this.dot(e)/t;return Math.acos(bt(i,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,i=this.y-e.y;return t*t+i*i}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this}equals(e){return e.x===this.x&&e.y===this.y}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this}rotateAround(e,t){const i=Math.cos(t),r=Math.sin(t),s=this.x-e.x,a=this.y-e.y;return this.x=s*i-a*r+e.x,this.y=s*r+a*i+e.y,this}random(){return this.x=Math.random(),this.y=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y}};xl.prototype.isVector2=!0;let At=xl;class Wr{constructor(e=0,t=0,i=0,r=1){this.isQuaternion=!0,this._x=e,this._y=t,this._z=i,this._w=r}static slerpFlat(e,t,i,r,s,a,o){let l=i[r+0],c=i[r+1],f=i[r+2],u=i[r+3],d=s[a+0],h=s[a+1],_=s[a+2],x=s[a+3];if(u!==x||l!==d||c!==h||f!==_){let m=l*d+c*h+f*_+u*x;m<0&&(d=-d,h=-h,_=-_,x=-x,m=-m);let p=1-o;if(m<.9995){const C=Math.acos(m),D=Math.sin(C);p=Math.sin(p*C)/D,o=Math.sin(o*C)/D,l=l*p+d*o,c=c*p+h*o,f=f*p+_*o,u=u*p+x*o}else{l=l*p+d*o,c=c*p+h*o,f=f*p+_*o,u=u*p+x*o;const C=1/Math.sqrt(l*l+c*c+f*f+u*u);l*=C,c*=C,f*=C,u*=C}}e[t]=l,e[t+1]=c,e[t+2]=f,e[t+3]=u}static multiplyQuaternionsFlat(e,t,i,r,s,a){const o=i[r],l=i[r+1],c=i[r+2],f=i[r+3],u=s[a],d=s[a+1],h=s[a+2],_=s[a+3];return e[t]=o*_+f*u+l*h-c*d,e[t+1]=l*_+f*d+c*u-o*h,e[t+2]=c*_+f*h+o*d-l*u,e[t+3]=f*_-o*u-l*d-c*h,e}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get w(){return this._w}set w(e){this._w=e,this._onChangeCallback()}set(e,t,i,r){return this._x=e,this._y=t,this._z=i,this._w=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._w)}copy(e){return this._x=e.x,this._y=e.y,this._z=e.z,this._w=e.w,this._onChangeCallback(),this}setFromEuler(e,t=!0){const i=e._x,r=e._y,s=e._z,a=e._order,o=Math.cos,l=Math.sin,c=o(i/2),f=o(r/2),u=o(s/2),d=l(i/2),h=l(r/2),_=l(s/2);switch(a){case"XYZ":this._x=d*f*u+c*h*_,this._y=c*h*u-d*f*_,this._z=c*f*_+d*h*u,this._w=c*f*u-d*h*_;break;case"YXZ":this._x=d*f*u+c*h*_,this._y=c*h*u-d*f*_,this._z=c*f*_-d*h*u,this._w=c*f*u+d*h*_;break;case"ZXY":this._x=d*f*u-c*h*_,this._y=c*h*u+d*f*_,this._z=c*f*_+d*h*u,this._w=c*f*u-d*h*_;break;case"ZYX":this._x=d*f*u-c*h*_,this._y=c*h*u+d*f*_,this._z=c*f*_-d*h*u,this._w=c*f*u+d*h*_;break;case"YZX":this._x=d*f*u+c*h*_,this._y=c*h*u+d*f*_,this._z=c*f*_-d*h*u,this._w=c*f*u-d*h*_;break;case"XZY":this._x=d*f*u-c*h*_,this._y=c*h*u-d*f*_,this._z=c*f*_+d*h*u,this._w=c*f*u+d*h*_;break;default:st("Quaternion: .setFromEuler() encountered an unknown order: "+a)}return t===!0&&this._onChangeCallback(),this}setFromAxisAngle(e,t){const i=t/2,r=Math.sin(i);return this._x=e.x*r,this._y=e.y*r,this._z=e.z*r,this._w=Math.cos(i),this._onChangeCallback(),this}setFromRotationMatrix(e){const t=e.elements,i=t[0],r=t[4],s=t[8],a=t[1],o=t[5],l=t[9],c=t[2],f=t[6],u=t[10],d=i+o+u;if(d>0){const h=.5/Math.sqrt(d+1);this._w=.25/h,this._x=(f-l)*h,this._y=(s-c)*h,this._z=(a-r)*h}else if(i>o&&i>u){const h=2*Math.sqrt(1+i-o-u);this._w=(f-l)/h,this._x=.25*h,this._y=(r+a)/h,this._z=(s+c)/h}else if(o>u){const h=2*Math.sqrt(1+o-i-u);this._w=(s-c)/h,this._x=(r+a)/h,this._y=.25*h,this._z=(l+f)/h}else{const h=2*Math.sqrt(1+u-i-o);this._w=(a-r)/h,this._x=(s+c)/h,this._y=(l+f)/h,this._z=.25*h}return this._onChangeCallback(),this}setFromUnitVectors(e,t){let i=e.dot(t)+1;return i<1e-8?(i=0,Math.abs(e.x)>Math.abs(e.z)?(this._x=-e.y,this._y=e.x,this._z=0,this._w=i):(this._x=0,this._y=-e.z,this._z=e.y,this._w=i)):(this._x=e.y*t.z-e.z*t.y,this._y=e.z*t.x-e.x*t.z,this._z=e.x*t.y-e.y*t.x,this._w=i),this.normalize()}angleTo(e){return 2*Math.acos(Math.abs(bt(this.dot(e),-1,1)))}rotateTowards(e,t){const i=this.angleTo(e);if(i===0)return this;const r=Math.min(1,t/i);return this.slerp(e,r),this}identity(){return this.set(0,0,0,1)}invert(){return this.conjugate()}conjugate(){return this._x*=-1,this._y*=-1,this._z*=-1,this._onChangeCallback(),this}dot(e){return this._x*e._x+this._y*e._y+this._z*e._z+this._w*e._w}lengthSq(){return this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w}length(){return Math.sqrt(this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w)}normalize(){let e=this.length();return e===0?(this._x=0,this._y=0,this._z=0,this._w=1):(e=1/e,this._x=this._x*e,this._y=this._y*e,this._z=this._z*e,this._w=this._w*e),this._onChangeCallback(),this}multiply(e){return this.multiplyQuaternions(this,e)}premultiply(e){return this.multiplyQuaternions(e,this)}multiplyQuaternions(e,t){const i=e._x,r=e._y,s=e._z,a=e._w,o=t._x,l=t._y,c=t._z,f=t._w;return this._x=i*f+a*o+r*c-s*l,this._y=r*f+a*l+s*o-i*c,this._z=s*f+a*c+i*l-r*o,this._w=a*f-i*o-r*l-s*c,this._onChangeCallback(),this}slerp(e,t){let i=e._x,r=e._y,s=e._z,a=e._w,o=this.dot(e);o<0&&(i=-i,r=-r,s=-s,a=-a,o=-o);let l=1-t;if(o<.9995){const c=Math.acos(o),f=Math.sin(c);l=Math.sin(l*c)/f,t=Math.sin(t*c)/f,this._x=this._x*l+i*t,this._y=this._y*l+r*t,this._z=this._z*l+s*t,this._w=this._w*l+a*t,this._onChangeCallback()}else this._x=this._x*l+i*t,this._y=this._y*l+r*t,this._z=this._z*l+s*t,this._w=this._w*l+a*t,this.normalize();return this}slerpQuaternions(e,t,i){return this.copy(e).slerp(t,i)}random(){const e=2*Math.PI*Math.random(),t=2*Math.PI*Math.random(),i=Math.random(),r=Math.sqrt(1-i),s=Math.sqrt(i);return this.set(r*Math.sin(e),r*Math.cos(e),s*Math.sin(t),s*Math.cos(t))}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._w===this._w}fromArray(e,t=0){return this._x=e[t],this._y=e[t+1],this._z=e[t+2],this._w=e[t+3],this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._w,e}fromBufferAttribute(e,t){return this._x=e.getX(t),this._y=e.getY(t),this._z=e.getZ(t),this._w=e.getW(t),this._onChangeCallback(),this}toJSON(){return this.toArray()}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._w}}const yl=class yl{constructor(e=0,t=0,i=0){this.x=e,this.y=t,this.z=i}set(e,t,i){return i===void 0&&(i=this.z),this.x=e,this.y=t,this.z=i,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;default:throw new Error("THREE.Vector3: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;default:throw new Error("THREE.Vector3: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this}multiplyVectors(e,t){return this.x=e.x*t.x,this.y=e.y*t.y,this.z=e.z*t.z,this}applyEuler(e){return this.applyQuaternion(Xl.setFromEuler(e))}applyAxisAngle(e,t){return this.applyQuaternion(Xl.setFromAxisAngle(e,t))}applyMatrix3(e){const t=this.x,i=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[3]*i+s[6]*r,this.y=s[1]*t+s[4]*i+s[7]*r,this.z=s[2]*t+s[5]*i+s[8]*r,this}applyNormalMatrix(e){return this.applyMatrix3(e).normalize()}applyMatrix4(e){const t=this.x,i=this.y,r=this.z,s=e.elements,a=1/(s[3]*t+s[7]*i+s[11]*r+s[15]);return this.x=(s[0]*t+s[4]*i+s[8]*r+s[12])*a,this.y=(s[1]*t+s[5]*i+s[9]*r+s[13])*a,this.z=(s[2]*t+s[6]*i+s[10]*r+s[14])*a,this}applyQuaternion(e){const t=this.x,i=this.y,r=this.z,s=e.x,a=e.y,o=e.z,l=e.w,c=2*(a*r-o*i),f=2*(o*t-s*r),u=2*(s*i-a*t);return this.x=t+l*c+a*u-o*f,this.y=i+l*f+o*c-s*u,this.z=r+l*u+s*f-a*c,this}project(e){return this.applyMatrix4(e.matrixWorldInverse).applyMatrix4(e.projectionMatrix)}unproject(e){return this.applyMatrix4(e.projectionMatrixInverse).applyMatrix4(e.matrixWorld)}transformDirection(e){const t=this.x,i=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[4]*i+s[8]*r,this.y=s[1]*t+s[5]*i+s[9]*r,this.z=s[2]*t+s[6]*i+s[10]*r,this.normalize()}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this}divideScalar(e){return this.multiplyScalar(1/e)}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this}clamp(e,t){return this.x=bt(this.x,e.x,t.x),this.y=bt(this.y,e.y,t.y),this.z=bt(this.z,e.z,t.z),this}clampScalar(e,t){return this.x=bt(this.x,e,t),this.y=bt(this.y,e,t),this.z=bt(this.z,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(bt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this.z=e.z+(t.z-e.z)*i,this}cross(e){return this.crossVectors(this,e)}crossVectors(e,t){const i=e.x,r=e.y,s=e.z,a=t.x,o=t.y,l=t.z;return this.x=r*l-s*o,this.y=s*a-i*l,this.z=i*o-r*a,this}projectOnVector(e){const t=e.lengthSq();if(t===0)return this.set(0,0,0);const i=e.dot(this)/t;return this.copy(e).multiplyScalar(i)}projectOnPlane(e){return Ta.copy(this).projectOnVector(e),this.sub(Ta)}reflect(e){return this.sub(Ta.copy(e).multiplyScalar(2*this.dot(e)))}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const i=this.dot(e)/t;return Math.acos(bt(i,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,i=this.y-e.y,r=this.z-e.z;return t*t+i*i+r*r}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)+Math.abs(this.z-e.z)}setFromSpherical(e){return this.setFromSphericalCoords(e.radius,e.phi,e.theta)}setFromSphericalCoords(e,t,i){const r=Math.sin(t)*e;return this.x=r*Math.sin(i),this.y=Math.cos(t)*e,this.z=r*Math.cos(i),this}setFromCylindrical(e){return this.setFromCylindricalCoords(e.radius,e.theta,e.y)}setFromCylindricalCoords(e,t,i){return this.x=e*Math.sin(t),this.y=i,this.z=e*Math.cos(t),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this}setFromMatrixScale(e){const t=this.setFromMatrixColumn(e,0).length(),i=this.setFromMatrixColumn(e,1).length(),r=this.setFromMatrixColumn(e,2).length();return this.x=t,this.y=i,this.z=r,this}setFromMatrixColumn(e,t){return this.fromArray(e.elements,t*4)}setFromMatrix3Column(e,t){return this.fromArray(e.elements,t*3)}setFromEuler(e){return this.x=e._x,this.y=e._y,this.z=e._z,this}setFromColor(e){return this.x=e.r,this.y=e.g,this.z=e.b,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this}randomDirection(){const e=Math.random()*Math.PI*2,t=Math.random()*2-1,i=Math.sqrt(1-t*t);return this.x=i*Math.cos(e),this.y=t,this.z=i*Math.sin(e),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z}};yl.prototype.isVector3=!0;let le=yl;const Ta=new le,Xl=new Wr,bl=class bl{constructor(e,t,i,r,s,a,o,l,c){this.elements=[1,0,0,0,1,0,0,0,1],e!==void 0&&this.set(e,t,i,r,s,a,o,l,c)}set(e,t,i,r,s,a,o,l,c){const f=this.elements;return f[0]=e,f[1]=r,f[2]=o,f[3]=t,f[4]=s,f[5]=l,f[6]=i,f[7]=a,f[8]=c,this}identity(){return this.set(1,0,0,0,1,0,0,0,1),this}copy(e){const t=this.elements,i=e.elements;return t[0]=i[0],t[1]=i[1],t[2]=i[2],t[3]=i[3],t[4]=i[4],t[5]=i[5],t[6]=i[6],t[7]=i[7],t[8]=i[8],this}extractBasis(e,t,i){return e.setFromMatrix3Column(this,0),t.setFromMatrix3Column(this,1),i.setFromMatrix3Column(this,2),this}setFromMatrix4(e){const t=e.elements;return this.set(t[0],t[4],t[8],t[1],t[5],t[9],t[2],t[6],t[10]),this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const i=e.elements,r=t.elements,s=this.elements,a=i[0],o=i[3],l=i[6],c=i[1],f=i[4],u=i[7],d=i[2],h=i[5],_=i[8],x=r[0],m=r[3],p=r[6],C=r[1],D=r[4],M=r[7],L=r[2],A=r[5],P=r[8];return s[0]=a*x+o*C+l*L,s[3]=a*m+o*D+l*A,s[6]=a*p+o*M+l*P,s[1]=c*x+f*C+u*L,s[4]=c*m+f*D+u*A,s[7]=c*p+f*M+u*P,s[2]=d*x+h*C+_*L,s[5]=d*m+h*D+_*A,s[8]=d*p+h*M+_*P,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[3]*=e,t[6]*=e,t[1]*=e,t[4]*=e,t[7]*=e,t[2]*=e,t[5]*=e,t[8]*=e,this}determinant(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],f=e[8];return t*a*f-t*o*c-i*s*f+i*o*l+r*s*c-r*a*l}invert(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],f=e[8],u=f*a-o*c,d=o*l-f*s,h=c*s-a*l,_=t*u+i*d+r*h;if(_===0)return this.set(0,0,0,0,0,0,0,0,0);const x=1/_;return e[0]=u*x,e[1]=(r*c-f*i)*x,e[2]=(o*i-r*a)*x,e[3]=d*x,e[4]=(f*t-r*l)*x,e[5]=(r*s-o*t)*x,e[6]=h*x,e[7]=(i*l-c*t)*x,e[8]=(a*t-i*s)*x,this}transpose(){let e;const t=this.elements;return e=t[1],t[1]=t[3],t[3]=e,e=t[2],t[2]=t[6],t[6]=e,e=t[5],t[5]=t[7],t[7]=e,this}getNormalMatrix(e){return this.setFromMatrix4(e).invert().transpose()}transposeIntoArray(e){const t=this.elements;return e[0]=t[0],e[1]=t[3],e[2]=t[6],e[3]=t[1],e[4]=t[4],e[5]=t[7],e[6]=t[2],e[7]=t[5],e[8]=t[8],this}setUvTransform(e,t,i,r,s,a,o){const l=Math.cos(s),c=Math.sin(s);return this.set(i*l,i*c,-i*(l*a+c*o)+a+e,-r*c,r*l,-r*(-c*a+l*o)+o+t,0,0,1),this}scale(e,t){return Fr("Matrix3: .scale() is deprecated. Use .makeScale() instead."),this.premultiply(Aa.makeScale(e,t)),this}rotate(e){return Fr("Matrix3: .rotate() is deprecated. Use .makeRotation() instead."),this.premultiply(Aa.makeRotation(-e)),this}translate(e,t){return Fr("Matrix3: .translate() is deprecated. Use .makeTranslation() instead."),this.premultiply(Aa.makeTranslation(e,t)),this}makeTranslation(e,t){return e.isVector2?this.set(1,0,e.x,0,1,e.y,0,0,1):this.set(1,0,e,0,1,t,0,0,1),this}makeRotation(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,-i,0,i,t,0,0,0,1),this}makeScale(e,t){return this.set(e,0,0,0,t,0,0,0,1),this}equals(e){const t=this.elements,i=e.elements;for(let r=0;r<9;r++)if(t[r]!==i[r])return!1;return!0}fromArray(e,t=0){for(let i=0;i<9;i++)this.elements[i]=e[i+t];return this}toArray(e=[],t=0){const i=this.elements;return e[t]=i[0],e[t+1]=i[1],e[t+2]=i[2],e[t+3]=i[3],e[t+4]=i[4],e[t+5]=i[5],e[t+6]=i[6],e[t+7]=i[7],e[t+8]=i[8],e}clone(){return new this.constructor().fromArray(this.elements)}};bl.prototype.isMatrix3=!0;let ct=bl;const Aa=new ct,ql=new ct().set(.4123908,.3575843,.1804808,.212639,.7151687,.0721923,.0193308,.1191948,.9505322),Yl=new ct().set(3.2409699,-1.5373832,-.4986108,-.9692436,1.8759675,.0415551,.0556301,-.203977,1.0569715);function Nf(){const n={enabled:!0,workingColorSpace:fs,spaces:{},convert:function(r,s,a){return this.enabled===!1||s===a||!s||!a||(this.spaces[s].transfer===Dt&&(r.r=Si(r.r),r.g=Si(r.g),r.b=Si(r.b)),this.spaces[s].primaries!==this.spaces[a].primaries&&(r.applyMatrix3(this.spaces[s].toXYZ),r.applyMatrix3(this.spaces[a].fromXYZ)),this.spaces[a].transfer===Dt&&(r.r=Or(r.r),r.g=Or(r.g),r.b=Or(r.b))),r},workingToColorSpace:function(r,s){return this.convert(r,this.workingColorSpace,s)},colorSpaceToWorking:function(r,s){return this.convert(r,s,this.workingColorSpace)},getPrimaries:function(r){return this.spaces[r].primaries},getTransfer:function(r){return r===_i?sa:this.spaces[r].transfer},getToneMappingMode:function(r){return this.spaces[r].outputColorSpaceConfig.toneMappingMode||"standard"},getLuminanceCoefficients:function(r,s=this.workingColorSpace){return r.fromArray(this.spaces[s].luminanceCoefficients)},define:function(r){Object.assign(this.spaces,r)},_getMatrix:function(r,s,a){return r.copy(this.spaces[s].toXYZ).multiply(this.spaces[a].fromXYZ)},_getDrawingBufferColorSpace:function(r){return this.spaces[r].outputColorSpaceConfig.drawingBufferColorSpace},_getUnpackColorSpace:function(r=this.workingColorSpace){return this.spaces[r].workingColorSpaceConfig.unpackColorSpace},fromWorkingColorSpace:function(r,s){return Fr("ColorManagement: .fromWorkingColorSpace() has been renamed to .workingToColorSpace()."),n.workingToColorSpace(r,s)},toWorkingColorSpace:function(r,s){return Fr("ColorManagement: .toWorkingColorSpace() has been renamed to .colorSpaceToWorking()."),n.colorSpaceToWorking(r,s)}},e=[.64,.33,.3,.6,.15,.06],t=[.2126,.7152,.0722],i=[.3127,.329];return n.define({[fs]:{primaries:e,whitePoint:i,transfer:sa,toXYZ:ql,fromXYZ:Yl,luminanceCoefficients:t,workingColorSpaceConfig:{unpackColorSpace:Ln},outputColorSpaceConfig:{drawingBufferColorSpace:Ln}},[Ln]:{primaries:e,whitePoint:i,transfer:Dt,toXYZ:ql,fromXYZ:Yl,luminanceCoefficients:t,outputColorSpaceConfig:{drawingBufferColorSpace:Ln}}}),n}const yt=Nf();function Si(n){return n<.04045?n*.0773993808:Math.pow(n*.9478672986+.0521327014,2.4)}function Or(n){return n<.0031308?n*12.92:1.055*Math.pow(n,.41666)-.055}let vr;class Uf{static getDataURL(e,t="image/png"){if(/^data:/i.test(e.src)||typeof HTMLCanvasElement>"u")return e.src;let i;if(e instanceof HTMLCanvasElement)i=e;else{vr===void 0&&(vr=oa("canvas")),vr.width=e.width,vr.height=e.height;const r=vr.getContext("2d");e instanceof ImageData?r.putImageData(e,0,0):r.drawImage(e,0,0,e.width,e.height),i=vr}return i.toDataURL(t)}static sRGBToLinear(e){if(typeof HTMLImageElement<"u"&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&e instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&e instanceof ImageBitmap){const t=oa("canvas");t.width=e.width,t.height=e.height;const i=t.getContext("2d");i.drawImage(e,0,0,e.width,e.height);const r=i.getImageData(0,0,e.width,e.height),s=r.data;for(let a=0;a<s.length;a++)s[a]=Si(s[a]/255)*255;return i.putImageData(r,0,0),t}else if(e.data){const t=e.data.slice(0);for(let i=0;i<t.length;i++)t instanceof Uint8Array||t instanceof Uint8ClampedArray?t[i]=Math.floor(Si(t[i]/255)*255):t[i]=Si(t[i]);return{data:t,width:e.width,height:e.height}}else return st("ImageUtils.sRGBToLinear(): Unsupported image type. No color space conversion applied."),e}}let Ff=0;class ml{constructor(e=null){this.isSource=!0,Object.defineProperty(this,"id",{value:Ff++}),this.uuid=zi(),this.data=e,this.dataReady=!0,this.version=0}getSize(e){const t=this.data;return typeof HTMLVideoElement<"u"&&t instanceof HTMLVideoElement?e.set(t.videoWidth,t.videoHeight,0):typeof VideoFrame<"u"&&t instanceof VideoFrame?e.set(t.displayWidth,t.displayHeight,0):t!==null?e.set(t.width,t.height,t.depth||0):e.set(0,0,0),e}set needsUpdate(e){e===!0&&this.version++}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.images[this.uuid]!==void 0)return e.images[this.uuid];const i={uuid:this.uuid,url:""},r=this.data;if(r!==null){let s;if(Array.isArray(r)){s=[];for(let a=0,o=r.length;a<o;a++)r[a].isDataTexture?s.push(Ra(r[a].image)):s.push(Ra(r[a]))}else s=Ra(r);i.url=s}return t||(e.images[this.uuid]=i),i}}function Ra(n){return typeof HTMLImageElement<"u"&&n instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&n instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&n instanceof ImageBitmap?Uf.getDataURL(n):n.data?{data:Array.from(n.data),width:n.width,height:n.height,type:n.data.constructor.name}:(st("Texture: Unable to serialize Texture."),{})}let Of=0;const Ca=new le;class un extends dr{constructor(e=un.DEFAULT_IMAGE,t=un.DEFAULT_MAPPING,i=xi,r=xi,s=ln,a=ir,o=Hn,l=In,c=un.DEFAULT_ANISOTROPY,f=_i){super(),this.isTexture=!0,Object.defineProperty(this,"id",{value:Of++}),this.uuid=zi(),this.name="",this.source=new ml(e),this.mipmaps=[],this.mapping=t,this.channel=0,this.wrapS=i,this.wrapT=r,this.magFilter=s,this.minFilter=a,this.anisotropy=c,this.format=o,this.internalFormat=null,this.type=l,this.offset=new At(0,0),this.repeat=new At(1,1),this.center=new At(0,0),this.rotation=0,this.matrixAutoUpdate=!0,this.matrix=new ct,this.generateMipmaps=!0,this.premultiplyAlpha=!1,this.flipY=!0,this.unpackAlignment=4,this.colorSpace=f,this.userData={},this.updateRanges=[],this.version=0,this.onUpdate=null,this.renderTarget=null,this.isRenderTargetTexture=!1,this.isArrayTexture=!!(e&&e.depth&&e.depth>1),this.pmremVersion=0,this.normalized=!1}get width(){return this.source.getSize(Ca).x}get height(){return this.source.getSize(Ca).y}get depth(){return this.source.getSize(Ca).z}get image(){return this.source.data}set image(e){this.source.data=e}updateMatrix(){this.matrix.setUvTransform(this.offset.x,this.offset.y,this.repeat.x,this.repeat.y,this.rotation,this.center.x,this.center.y)}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}clone(){return new this.constructor().copy(this)}copy(e){return this.name=e.name,this.source=e.source,this.mipmaps=e.mipmaps.slice(0),this.mapping=e.mapping,this.channel=e.channel,this.wrapS=e.wrapS,this.wrapT=e.wrapT,this.magFilter=e.magFilter,this.minFilter=e.minFilter,this.anisotropy=e.anisotropy,this.format=e.format,this.internalFormat=e.internalFormat,this.type=e.type,this.normalized=e.normalized,this.offset.copy(e.offset),this.repeat.copy(e.repeat),this.center.copy(e.center),this.rotation=e.rotation,this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrix.copy(e.matrix),this.generateMipmaps=e.generateMipmaps,this.premultiplyAlpha=e.premultiplyAlpha,this.flipY=e.flipY,this.unpackAlignment=e.unpackAlignment,this.colorSpace=e.colorSpace,this.renderTarget=e.renderTarget,this.isRenderTargetTexture=e.isRenderTargetTexture,this.isArrayTexture=e.isArrayTexture,this.userData=JSON.parse(JSON.stringify(e.userData)),this.needsUpdate=!0,this}setValues(e){for(const t in e){const i=e[t];if(i===void 0){st(`Texture.setValues(): parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){st(`Texture.setValues(): property '${t}' does not exist.`);continue}r&&i&&r.isVector2&&i.isVector2||r&&i&&r.isVector3&&i.isVector3||r&&i&&r.isMatrix3&&i.isMatrix3?r.copy(i):this[t]=i}}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.textures[this.uuid]!==void 0)return e.textures[this.uuid];const i={metadata:{version:4.7,type:"Texture",generator:"Texture.toJSON"},uuid:this.uuid,name:this.name,image:this.source.toJSON(e).uuid,mapping:this.mapping,channel:this.channel,repeat:[this.repeat.x,this.repeat.y],offset:[this.offset.x,this.offset.y],center:[this.center.x,this.center.y],rotation:this.rotation,wrap:[this.wrapS,this.wrapT],format:this.format,internalFormat:this.internalFormat,type:this.type,normalized:this.normalized,colorSpace:this.colorSpace,minFilter:this.minFilter,magFilter:this.magFilter,anisotropy:this.anisotropy,flipY:this.flipY,generateMipmaps:this.generateMipmaps,premultiplyAlpha:this.premultiplyAlpha,unpackAlignment:this.unpackAlignment};return Object.keys(this.userData).length>0&&(i.userData=this.userData),t||(e.textures[this.uuid]=i),i}dispose(){this.dispatchEvent({type:"dispose"})}transformUv(e){if(this.mapping!==gd)return e;if(e.applyMatrix3(this.matrix),e.x<0||e.x>1)switch(this.wrapS){case go:e.x=e.x-Math.floor(e.x);break;case xi:e.x=e.x<0?0:1;break;case _o:Math.abs(Math.floor(e.x)%2)===1?e.x=Math.ceil(e.x)-e.x:e.x=e.x-Math.floor(e.x);break}if(e.y<0||e.y>1)switch(this.wrapT){case go:e.y=e.y-Math.floor(e.y);break;case xi:e.y=e.y<0?0:1;break;case _o:Math.abs(Math.floor(e.y)%2)===1?e.y=Math.ceil(e.y)-e.y:e.y=e.y-Math.floor(e.y);break}return this.flipY&&(e.y=1-e.y),e}set needsUpdate(e){e===!0&&(this.version++,this.source.needsUpdate=!0)}set needsPMREMUpdate(e){e===!0&&this.pmremVersion++}}un.DEFAULT_IMAGE=null;un.DEFAULT_MAPPING=gd;un.DEFAULT_ANISOTROPY=1;const Sl=class Sl{constructor(e=0,t=0,i=0,r=1){this.x=e,this.y=t,this.z=i,this.w=r}get width(){return this.z}set width(e){this.z=e}get height(){return this.w}set height(e){this.w=e}set(e,t,i,r){return this.x=e,this.y=t,this.z=i,this.w=r,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this.w=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setW(e){return this.w=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;case 3:this.w=t;break;default:throw new Error("THREE.Vector4: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;case 3:return this.w;default:throw new Error("THREE.Vector4: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z,this.w)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this.w=e.w!==void 0?e.w:1,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this.w+=e.w,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this.w+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this.w=e.w+t.w,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this.w+=e.w*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this.w-=e.w,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this.w-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this.w=e.w-t.w,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this.w*=e.w,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this.w*=e,this}applyMatrix4(e){const t=this.x,i=this.y,r=this.z,s=this.w,a=e.elements;return this.x=a[0]*t+a[4]*i+a[8]*r+a[12]*s,this.y=a[1]*t+a[5]*i+a[9]*r+a[13]*s,this.z=a[2]*t+a[6]*i+a[10]*r+a[14]*s,this.w=a[3]*t+a[7]*i+a[11]*r+a[15]*s,this}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this.w/=e.w,this}divideScalar(e){return this.multiplyScalar(1/e)}setAxisAngleFromQuaternion(e){this.w=2*Math.acos(e.w);const t=Math.sqrt(1-e.w*e.w);return t<1e-4?(this.x=1,this.y=0,this.z=0):(this.x=e.x/t,this.y=e.y/t,this.z=e.z/t),this}setAxisAngleFromRotationMatrix(e){let t,i,r,s;const l=e.elements,c=l[0],f=l[4],u=l[8],d=l[1],h=l[5],_=l[9],x=l[2],m=l[6],p=l[10];if(Math.abs(f-d)<.01&&Math.abs(u-x)<.01&&Math.abs(_-m)<.01){if(Math.abs(f+d)<.1&&Math.abs(u+x)<.1&&Math.abs(_+m)<.1&&Math.abs(c+h+p-3)<.1)return this.set(1,0,0,0),this;t=Math.PI;const D=(c+1)/2,M=(h+1)/2,L=(p+1)/2,A=(f+d)/4,P=(u+x)/4,v=(_+m)/4;return D>M&&D>L?D<.01?(i=0,r=.707106781,s=.707106781):(i=Math.sqrt(D),r=A/i,s=P/i):M>L?M<.01?(i=.707106781,r=0,s=.707106781):(r=Math.sqrt(M),i=A/r,s=v/r):L<.01?(i=.707106781,r=.707106781,s=0):(s=Math.sqrt(L),i=P/s,r=v/s),this.set(i,r,s,t),this}let C=Math.sqrt((m-_)*(m-_)+(u-x)*(u-x)+(d-f)*(d-f));return Math.abs(C)<.001&&(C=1),this.x=(m-_)/C,this.y=(u-x)/C,this.z=(d-f)/C,this.w=Math.acos((c+h+p-1)/2),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this.w=t[15],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this.w=Math.min(this.w,e.w),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this.w=Math.max(this.w,e.w),this}clamp(e,t){return this.x=bt(this.x,e.x,t.x),this.y=bt(this.y,e.y,t.y),this.z=bt(this.z,e.z,t.z),this.w=bt(this.w,e.w,t.w),this}clampScalar(e,t){return this.x=bt(this.x,e,t),this.y=bt(this.y,e,t),this.z=bt(this.z,e,t),this.w=bt(this.w,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(bt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this.w=Math.floor(this.w),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this.w=Math.ceil(this.w),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this.w=Math.round(this.w),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this.w=Math.trunc(this.w),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this.w=-this.w,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z+this.w*e.w}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)+Math.abs(this.w)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this.w+=(e.w-this.w)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this.z=e.z+(t.z-e.z)*i,this.w=e.w+(t.w-e.w)*i,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z&&e.w===this.w}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this.w=e[t+3],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e[t+3]=this.w,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this.w=e.getW(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this.w=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z,yield this.w}};Sl.prototype.isVector4=!0;let Xt=Sl;class Bf extends dr{constructor(e=1,t=1,i={}){super(),i=Object.assign({generateMipmaps:!1,internalFormat:null,minFilter:ln,depthBuffer:!0,stencilBuffer:!1,resolveDepthBuffer:!0,resolveStencilBuffer:!0,depthTexture:null,samples:0,count:1,depth:1,multiview:!1,useArrayDepthTexture:!1},i),this.isRenderTarget=!0,this.width=e,this.height=t,this.depth=i.depth,this.scissor=new Xt(0,0,e,t),this.scissorTest=!1,this.viewport=new Xt(0,0,e,t),this.textures=[];const r={width:e,height:t,depth:i.depth},s=new un(r),a=i.count;for(let o=0;o<a;o++)this.textures[o]=s.clone(),this.textures[o].isRenderTargetTexture=!0,this.textures[o].renderTarget=this;this._setTextureOptions(i),this.depthBuffer=i.depthBuffer,this.stencilBuffer=i.stencilBuffer,this.resolveDepthBuffer=i.resolveDepthBuffer,this.resolveStencilBuffer=i.resolveStencilBuffer,this._depthTexture=null,this.depthTexture=i.depthTexture,this.samples=i.samples,this.multiview=i.multiview,this.useArrayDepthTexture=i.useArrayDepthTexture}_setTextureOptions(e={}){const t={minFilter:ln,generateMipmaps:!1,flipY:!1,internalFormat:null};e.mapping!==void 0&&(t.mapping=e.mapping),e.wrapS!==void 0&&(t.wrapS=e.wrapS),e.wrapT!==void 0&&(t.wrapT=e.wrapT),e.wrapR!==void 0&&(t.wrapR=e.wrapR),e.magFilter!==void 0&&(t.magFilter=e.magFilter),e.minFilter!==void 0&&(t.minFilter=e.minFilter),e.format!==void 0&&(t.format=e.format),e.type!==void 0&&(t.type=e.type),e.anisotropy!==void 0&&(t.anisotropy=e.anisotropy),e.colorSpace!==void 0&&(t.colorSpace=e.colorSpace),e.flipY!==void 0&&(t.flipY=e.flipY),e.generateMipmaps!==void 0&&(t.generateMipmaps=e.generateMipmaps),e.internalFormat!==void 0&&(t.internalFormat=e.internalFormat);for(let i=0;i<this.textures.length;i++)this.textures[i].setValues(t)}get texture(){return this.textures[0]}set texture(e){this.textures[0]=e}set depthTexture(e){this._depthTexture!==null&&(this._depthTexture.renderTarget=null),e!==null&&(e.renderTarget=this),this._depthTexture=e}get depthTexture(){return this._depthTexture}setSize(e,t,i=1){if(this.width!==e||this.height!==t||this.depth!==i){this.width=e,this.height=t,this.depth=i;for(let r=0,s=this.textures.length;r<s;r++)this.textures[r].image.width=e,this.textures[r].image.height=t,this.textures[r].image.depth=i,this.textures[r].isData3DTexture!==!0&&(this.textures[r].isArrayTexture=this.textures[r].image.depth>1);this.dispose()}this.viewport.set(0,0,e,t),this.scissor.set(0,0,e,t)}clone(){return new this.constructor().copy(this)}copy(e){this.width=e.width,this.height=e.height,this.depth=e.depth,this.scissor.copy(e.scissor),this.scissorTest=e.scissorTest,this.viewport.copy(e.viewport),this.textures.length=0;for(let t=0,i=e.textures.length;t<i;t++){this.textures[t]=e.textures[t].clone(),this.textures[t].isRenderTargetTexture=!0,this.textures[t].renderTarget=this;const r=Object.assign({},e.textures[t].image);this.textures[t].source=new ml(r)}return this.depthBuffer=e.depthBuffer,this.stencilBuffer=e.stencilBuffer,this.resolveDepthBuffer=e.resolveDepthBuffer,this.resolveStencilBuffer=e.resolveStencilBuffer,e.depthTexture!==null&&(this.depthTexture=e.depthTexture.clone()),this.samples=e.samples,this.multiview=e.multiview,this.useArrayDepthTexture=e.useArrayDepthTexture,this}dispose(){this.dispatchEvent({type:"dispose"})}}class ni extends Bf{constructor(e=1,t=1,i={}){super(e,t,i),this.isWebGLRenderTarget=!0}}class wd extends un{constructor(e=null,t=1,i=1,r=1){super(null),this.isDataArrayTexture=!0,this.image={data:e,width:t,height:i,depth:r},this.magFilter=on,this.minFilter=on,this.wrapR=xi,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1,this.layerUpdates=new Set}addLayerUpdate(e){this.layerUpdates.add(e)}clearLayerUpdates(){this.layerUpdates.clear()}}class kf extends un{constructor(e=null,t=1,i=1,r=1){super(null),this.isData3DTexture=!0,this.image={data:e,width:t,height:i,depth:r},this.magFilter=on,this.minFilter=on,this.wrapR=xi,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const ca=class ca{constructor(e,t,i,r,s,a,o,l,c,f,u,d,h,_,x,m){this.elements=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],e!==void 0&&this.set(e,t,i,r,s,a,o,l,c,f,u,d,h,_,x,m)}set(e,t,i,r,s,a,o,l,c,f,u,d,h,_,x,m){const p=this.elements;return p[0]=e,p[4]=t,p[8]=i,p[12]=r,p[1]=s,p[5]=a,p[9]=o,p[13]=l,p[2]=c,p[6]=f,p[10]=u,p[14]=d,p[3]=h,p[7]=_,p[11]=x,p[15]=m,this}identity(){return this.set(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1),this}clone(){return new ca().fromArray(this.elements)}copy(e){const t=this.elements,i=e.elements;return t[0]=i[0],t[1]=i[1],t[2]=i[2],t[3]=i[3],t[4]=i[4],t[5]=i[5],t[6]=i[6],t[7]=i[7],t[8]=i[8],t[9]=i[9],t[10]=i[10],t[11]=i[11],t[12]=i[12],t[13]=i[13],t[14]=i[14],t[15]=i[15],this}copyPosition(e){const t=this.elements,i=e.elements;return t[12]=i[12],t[13]=i[13],t[14]=i[14],this}setFromMatrix3(e){const t=e.elements;return this.set(t[0],t[3],t[6],0,t[1],t[4],t[7],0,t[2],t[5],t[8],0,0,0,0,1),this}extractBasis(e,t,i){return this.determinantAffine()===0?(e.set(1,0,0),t.set(0,1,0),i.set(0,0,1),this):(e.setFromMatrixColumn(this,0),t.setFromMatrixColumn(this,1),i.setFromMatrixColumn(this,2),this)}makeBasis(e,t,i){return this.set(e.x,t.x,i.x,0,e.y,t.y,i.y,0,e.z,t.z,i.z,0,0,0,0,1),this}extractRotation(e){if(e.determinantAffine()===0)return this.identity();const t=this.elements,i=e.elements,r=1/xr.setFromMatrixColumn(e,0).length(),s=1/xr.setFromMatrixColumn(e,1).length(),a=1/xr.setFromMatrixColumn(e,2).length();return t[0]=i[0]*r,t[1]=i[1]*r,t[2]=i[2]*r,t[3]=0,t[4]=i[4]*s,t[5]=i[5]*s,t[6]=i[6]*s,t[7]=0,t[8]=i[8]*a,t[9]=i[9]*a,t[10]=i[10]*a,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromEuler(e){const t=this.elements,i=e.x,r=e.y,s=e.z,a=Math.cos(i),o=Math.sin(i),l=Math.cos(r),c=Math.sin(r),f=Math.cos(s),u=Math.sin(s);if(e.order==="XYZ"){const d=a*f,h=a*u,_=o*f,x=o*u;t[0]=l*f,t[4]=-l*u,t[8]=c,t[1]=h+_*c,t[5]=d-x*c,t[9]=-o*l,t[2]=x-d*c,t[6]=_+h*c,t[10]=a*l}else if(e.order==="YXZ"){const d=l*f,h=l*u,_=c*f,x=c*u;t[0]=d+x*o,t[4]=_*o-h,t[8]=a*c,t[1]=a*u,t[5]=a*f,t[9]=-o,t[2]=h*o-_,t[6]=x+d*o,t[10]=a*l}else if(e.order==="ZXY"){const d=l*f,h=l*u,_=c*f,x=c*u;t[0]=d-x*o,t[4]=-a*u,t[8]=_+h*o,t[1]=h+_*o,t[5]=a*f,t[9]=x-d*o,t[2]=-a*c,t[6]=o,t[10]=a*l}else if(e.order==="ZYX"){const d=a*f,h=a*u,_=o*f,x=o*u;t[0]=l*f,t[4]=_*c-h,t[8]=d*c+x,t[1]=l*u,t[5]=x*c+d,t[9]=h*c-_,t[2]=-c,t[6]=o*l,t[10]=a*l}else if(e.order==="YZX"){const d=a*l,h=a*c,_=o*l,x=o*c;t[0]=l*f,t[4]=x-d*u,t[8]=_*u+h,t[1]=u,t[5]=a*f,t[9]=-o*f,t[2]=-c*f,t[6]=h*u+_,t[10]=d-x*u}else if(e.order==="XZY"){const d=a*l,h=a*c,_=o*l,x=o*c;t[0]=l*f,t[4]=-u,t[8]=c*f,t[1]=d*u+x,t[5]=a*f,t[9]=h*u-_,t[2]=_*u-h,t[6]=o*f,t[10]=x*u+d}return t[3]=0,t[7]=0,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromQuaternion(e){return this.compose(zf,e,Gf)}lookAt(e,t,i){const r=this.elements;return Mn.subVectors(e,t),Mn.lengthSq()===0&&(Mn.z=1),Mn.normalize(),Li.crossVectors(i,Mn),Li.lengthSq()===0&&(Math.abs(i.z)===1?Mn.x+=1e-4:Mn.z+=1e-4,Mn.normalize(),Li.crossVectors(i,Mn)),Li.normalize(),Ms.crossVectors(Mn,Li),r[0]=Li.x,r[4]=Ms.x,r[8]=Mn.x,r[1]=Li.y,r[5]=Ms.y,r[9]=Mn.y,r[2]=Li.z,r[6]=Ms.z,r[10]=Mn.z,this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const i=e.elements,r=t.elements,s=this.elements,a=i[0],o=i[4],l=i[8],c=i[12],f=i[1],u=i[5],d=i[9],h=i[13],_=i[2],x=i[6],m=i[10],p=i[14],C=i[3],D=i[7],M=i[11],L=i[15],A=r[0],P=r[4],v=r[8],b=r[12],w=r[1],E=r[5],N=r[9],G=r[13],W=r[2],I=r[6],R=r[10],F=r[14],J=r[3],B=r[7],oe=r[11],k=r[15];return s[0]=a*A+o*w+l*W+c*J,s[4]=a*P+o*E+l*I+c*B,s[8]=a*v+o*N+l*R+c*oe,s[12]=a*b+o*G+l*F+c*k,s[1]=f*A+u*w+d*W+h*J,s[5]=f*P+u*E+d*I+h*B,s[9]=f*v+u*N+d*R+h*oe,s[13]=f*b+u*G+d*F+h*k,s[2]=_*A+x*w+m*W+p*J,s[6]=_*P+x*E+m*I+p*B,s[10]=_*v+x*N+m*R+p*oe,s[14]=_*b+x*G+m*F+p*k,s[3]=C*A+D*w+M*W+L*J,s[7]=C*P+D*E+M*I+L*B,s[11]=C*v+D*N+M*R+L*oe,s[15]=C*b+D*G+M*F+L*k,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[4]*=e,t[8]*=e,t[12]*=e,t[1]*=e,t[5]*=e,t[9]*=e,t[13]*=e,t[2]*=e,t[6]*=e,t[10]*=e,t[14]*=e,t[3]*=e,t[7]*=e,t[11]*=e,t[15]*=e,this}determinant(){const e=this.elements,t=e[0],i=e[4],r=e[8],s=e[12],a=e[1],o=e[5],l=e[9],c=e[13],f=e[2],u=e[6],d=e[10],h=e[14],_=e[3],x=e[7],m=e[11],p=e[15],C=l*h-c*d,D=o*h-c*u,M=o*d-l*u,L=a*h-c*f,A=a*d-l*f,P=a*u-o*f;return t*(x*C-m*D+p*M)-i*(_*C-m*L+p*A)+r*(_*D-x*L+p*P)-s*(_*M-x*A+m*P)}determinantAffine(){const e=this.elements,t=e[0],i=e[4],r=e[8],s=e[1],a=e[5],o=e[9],l=e[2],c=e[6],f=e[10];return t*(a*f-o*c)-i*(s*f-o*l)+r*(s*c-a*l)}transpose(){const e=this.elements;let t;return t=e[1],e[1]=e[4],e[4]=t,t=e[2],e[2]=e[8],e[8]=t,t=e[6],e[6]=e[9],e[9]=t,t=e[3],e[3]=e[12],e[12]=t,t=e[7],e[7]=e[13],e[13]=t,t=e[11],e[11]=e[14],e[14]=t,this}setPosition(e,t,i){const r=this.elements;return e.isVector3?(r[12]=e.x,r[13]=e.y,r[14]=e.z):(r[12]=e,r[13]=t,r[14]=i),this}invert(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],f=e[8],u=e[9],d=e[10],h=e[11],_=e[12],x=e[13],m=e[14],p=e[15],C=t*o-i*a,D=t*l-r*a,M=t*c-s*a,L=i*l-r*o,A=i*c-s*o,P=r*c-s*l,v=f*x-u*_,b=f*m-d*_,w=f*p-h*_,E=u*m-d*x,N=u*p-h*x,G=d*p-h*m,W=C*G-D*N+M*E+L*w-A*b+P*v;if(W===0)return this.set(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);const I=1/W;return e[0]=(o*G-l*N+c*E)*I,e[1]=(r*N-i*G-s*E)*I,e[2]=(x*P-m*A+p*L)*I,e[3]=(d*A-u*P-h*L)*I,e[4]=(l*w-a*G-c*b)*I,e[5]=(t*G-r*w+s*b)*I,e[6]=(m*M-_*P-p*D)*I,e[7]=(f*P-d*M+h*D)*I,e[8]=(a*N-o*w+c*v)*I,e[9]=(i*w-t*N-s*v)*I,e[10]=(_*A-x*M+p*C)*I,e[11]=(u*M-f*A-h*C)*I,e[12]=(o*b-a*E-l*v)*I,e[13]=(t*E-i*b+r*v)*I,e[14]=(x*D-_*L-m*C)*I,e[15]=(f*L-u*D+d*C)*I,this}scale(e){const t=this.elements,i=e.x,r=e.y,s=e.z;return t[0]*=i,t[4]*=r,t[8]*=s,t[1]*=i,t[5]*=r,t[9]*=s,t[2]*=i,t[6]*=r,t[10]*=s,t[3]*=i,t[7]*=r,t[11]*=s,this}getMaxScaleOnAxis(){const e=this.elements,t=e[0]*e[0]+e[1]*e[1]+e[2]*e[2],i=e[4]*e[4]+e[5]*e[5]+e[6]*e[6],r=e[8]*e[8]+e[9]*e[9]+e[10]*e[10];return Math.sqrt(Math.max(t,i,r))}makeTranslation(e,t,i){return e.isVector3?this.set(1,0,0,e.x,0,1,0,e.y,0,0,1,e.z,0,0,0,1):this.set(1,0,0,e,0,1,0,t,0,0,1,i,0,0,0,1),this}makeRotationX(e){const t=Math.cos(e),i=Math.sin(e);return this.set(1,0,0,0,0,t,-i,0,0,i,t,0,0,0,0,1),this}makeRotationY(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,0,i,0,0,1,0,0,-i,0,t,0,0,0,0,1),this}makeRotationZ(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,-i,0,0,i,t,0,0,0,0,1,0,0,0,0,1),this}makeRotationAxis(e,t){const i=Math.cos(t),r=Math.sin(t),s=1-i,a=e.x,o=e.y,l=e.z,c=s*a,f=s*o;return this.set(c*a+i,c*o-r*l,c*l+r*o,0,c*o+r*l,f*o+i,f*l-r*a,0,c*l-r*o,f*l+r*a,s*l*l+i,0,0,0,0,1),this}makeScale(e,t,i){return this.set(e,0,0,0,0,t,0,0,0,0,i,0,0,0,0,1),this}makeShear(e,t,i,r,s,a){return this.set(1,i,s,0,e,1,a,0,t,r,1,0,0,0,0,1),this}compose(e,t,i){const r=this.elements,s=t._x,a=t._y,o=t._z,l=t._w,c=s+s,f=a+a,u=o+o,d=s*c,h=s*f,_=s*u,x=a*f,m=a*u,p=o*u,C=l*c,D=l*f,M=l*u,L=i.x,A=i.y,P=i.z;return r[0]=(1-(x+p))*L,r[1]=(h+M)*L,r[2]=(_-D)*L,r[3]=0,r[4]=(h-M)*A,r[5]=(1-(d+p))*A,r[6]=(m+C)*A,r[7]=0,r[8]=(_+D)*P,r[9]=(m-C)*P,r[10]=(1-(d+x))*P,r[11]=0,r[12]=e.x,r[13]=e.y,r[14]=e.z,r[15]=1,this}decompose(e,t,i){const r=this.elements;e.x=r[12],e.y=r[13],e.z=r[14];const s=this.determinantAffine();if(s===0)return i.set(1,1,1),t.identity(),this;let a=xr.set(r[0],r[1],r[2]).length();const o=xr.set(r[4],r[5],r[6]).length(),l=xr.set(r[8],r[9],r[10]).length();s<0&&(a=-a),On.copy(this);const c=1/a,f=1/o,u=1/l;return On.elements[0]*=c,On.elements[1]*=c,On.elements[2]*=c,On.elements[4]*=f,On.elements[5]*=f,On.elements[6]*=f,On.elements[8]*=u,On.elements[9]*=u,On.elements[10]*=u,t.setFromRotationMatrix(On),i.x=a,i.y=o,i.z=l,this}makePerspective(e,t,i,r,s,a,o=ti,l=!1){const c=this.elements,f=2*s/(t-e),u=2*s/(i-r),d=(t+e)/(t-e),h=(i+r)/(i-r);let _,x;if(l)_=s/(a-s),x=a*s/(a-s);else if(o===ti)_=-(a+s)/(a-s),x=-2*a*s/(a-s);else if(o===aa)_=-a/(a-s),x=-a*s/(a-s);else throw new Error("THREE.Matrix4.makePerspective(): Invalid coordinate system: "+o);return c[0]=f,c[4]=0,c[8]=d,c[12]=0,c[1]=0,c[5]=u,c[9]=h,c[13]=0,c[2]=0,c[6]=0,c[10]=_,c[14]=x,c[3]=0,c[7]=0,c[11]=-1,c[15]=0,this}makeOrthographic(e,t,i,r,s,a,o=ti,l=!1){const c=this.elements,f=2/(t-e),u=2/(i-r),d=-(t+e)/(t-e),h=-(i+r)/(i-r);let _,x;if(l)_=1/(a-s),x=a/(a-s);else if(o===ti)_=-2/(a-s),x=-(a+s)/(a-s);else if(o===aa)_=-1/(a-s),x=-s/(a-s);else throw new Error("THREE.Matrix4.makeOrthographic(): Invalid coordinate system: "+o);return c[0]=f,c[4]=0,c[8]=0,c[12]=d,c[1]=0,c[5]=u,c[9]=0,c[13]=h,c[2]=0,c[6]=0,c[10]=_,c[14]=x,c[3]=0,c[7]=0,c[11]=0,c[15]=1,this}equals(e){const t=this.elements,i=e.elements;for(let r=0;r<16;r++)if(t[r]!==i[r])return!1;return!0}fromArray(e,t=0){for(let i=0;i<16;i++)this.elements[i]=e[i+t];return this}toArray(e=[],t=0){const i=this.elements;return e[t]=i[0],e[t+1]=i[1],e[t+2]=i[2],e[t+3]=i[3],e[t+4]=i[4],e[t+5]=i[5],e[t+6]=i[6],e[t+7]=i[7],e[t+8]=i[8],e[t+9]=i[9],e[t+10]=i[10],e[t+11]=i[11],e[t+12]=i[12],e[t+13]=i[13],e[t+14]=i[14],e[t+15]=i[15],e}};ca.prototype.isMatrix4=!0;let Zt=ca;const xr=new le,On=new Zt,zf=new le(0,0,0),Gf=new le(1,1,1),Li=new le,Ms=new le,Mn=new le,$l=new Zt,Kl=new Wr;class lr{constructor(e=0,t=0,i=0,r=lr.DEFAULT_ORDER){this.isEuler=!0,this._x=e,this._y=t,this._z=i,this._order=r}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get order(){return this._order}set order(e){this._order=e,this._onChangeCallback()}set(e,t,i,r=this._order){return this._x=e,this._y=t,this._z=i,this._order=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._order)}copy(e){return this._x=e._x,this._y=e._y,this._z=e._z,this._order=e._order,this._onChangeCallback(),this}setFromRotationMatrix(e,t=this._order,i=!0){const r=e.elements,s=r[0],a=r[4],o=r[8],l=r[1],c=r[5],f=r[9],u=r[2],d=r[6],h=r[10];switch(t){case"XYZ":this._y=Math.asin(bt(o,-1,1)),Math.abs(o)<.9999999?(this._x=Math.atan2(-f,h),this._z=Math.atan2(-a,s)):(this._x=Math.atan2(d,c),this._z=0);break;case"YXZ":this._x=Math.asin(-bt(f,-1,1)),Math.abs(f)<.9999999?(this._y=Math.atan2(o,h),this._z=Math.atan2(l,c)):(this._y=Math.atan2(-u,s),this._z=0);break;case"ZXY":this._x=Math.asin(bt(d,-1,1)),Math.abs(d)<.9999999?(this._y=Math.atan2(-u,h),this._z=Math.atan2(-a,c)):(this._y=0,this._z=Math.atan2(l,s));break;case"ZYX":this._y=Math.asin(-bt(u,-1,1)),Math.abs(u)<.9999999?(this._x=Math.atan2(d,h),this._z=Math.atan2(l,s)):(this._x=0,this._z=Math.atan2(-a,c));break;case"YZX":this._z=Math.asin(bt(l,-1,1)),Math.abs(l)<.9999999?(this._x=Math.atan2(-f,c),this._y=Math.atan2(-u,s)):(this._x=0,this._y=Math.atan2(o,h));break;case"XZY":this._z=Math.asin(-bt(a,-1,1)),Math.abs(a)<.9999999?(this._x=Math.atan2(d,c),this._y=Math.atan2(o,s)):(this._x=Math.atan2(-f,h),this._y=0);break;default:st("Euler: .setFromRotationMatrix() encountered an unknown order: "+t)}return this._order=t,i===!0&&this._onChangeCallback(),this}setFromQuaternion(e,t,i){return $l.makeRotationFromQuaternion(e),this.setFromRotationMatrix($l,t,i)}setFromVector3(e,t=this._order){return this.set(e.x,e.y,e.z,t)}reorder(e){return Kl.setFromEuler(this),this.setFromQuaternion(Kl,e)}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._order===this._order}fromArray(e){return this._x=e[0],this._y=e[1],this._z=e[2],e[3]!==void 0&&(this._order=e[3]),this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._order,e}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._order}}lr.DEFAULT_ORDER="XYZ";class Td{constructor(){this.mask=1}set(e){this.mask=(1<<e|0)>>>0}enable(e){this.mask|=1<<e|0}enableAll(){this.mask=-1}toggle(e){this.mask^=1<<e|0}disable(e){this.mask&=~(1<<e|0)}disableAll(){this.mask=0}test(e){return(this.mask&e.mask)!==0}isEnabled(e){return(this.mask&(1<<e|0))!==0}}let Hf=0;const Zl=new le,yr=new Wr,di=new Zt,Es=new le,Qr=new le,Vf=new le,Wf=new Wr,Jl=new le(1,0,0),Ql=new le(0,1,0),jl=new le(0,0,1),ec={type:"added"},Xf={type:"removed"},br={type:"childadded",child:null},Pa={type:"childremoved",child:null};class Tn extends dr{constructor(){super(),this.isObject3D=!0,Object.defineProperty(this,"id",{value:Hf++}),this.uuid=zi(),this.name="",this.type="Object3D",this.parent=null,this.children=[],this.up=Tn.DEFAULT_UP.clone();const e=new le,t=new lr,i=new Wr,r=new le(1,1,1);function s(){i.setFromEuler(t,!1)}function a(){t.setFromQuaternion(i,void 0,!1)}t._onChange(s),i._onChange(a),Object.defineProperties(this,{position:{configurable:!0,enumerable:!0,value:e},rotation:{configurable:!0,enumerable:!0,value:t},quaternion:{configurable:!0,enumerable:!0,value:i},scale:{configurable:!0,enumerable:!0,value:r},modelViewMatrix:{value:new Zt},normalMatrix:{value:new ct}}),this.matrix=new Zt,this.matrixWorld=new Zt,this.matrixAutoUpdate=Tn.DEFAULT_MATRIX_AUTO_UPDATE,this.matrixWorldAutoUpdate=Tn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE,this.matrixWorldNeedsUpdate=!1,this.layers=new Td,this.visible=!0,this.castShadow=!1,this.receiveShadow=!1,this.frustumCulled=!0,this.renderOrder=0,this.animations=[],this.customDepthMaterial=void 0,this.customDistanceMaterial=void 0,this.static=!1,this.userData={},this.pivot=null}onBeforeShadow(){}onAfterShadow(){}onBeforeRender(){}onAfterRender(){}applyMatrix4(e){this.matrixAutoUpdate&&this.updateMatrix(),this.matrix.premultiply(e),this.matrix.decompose(this.position,this.quaternion,this.scale)}applyQuaternion(e){return this.quaternion.premultiply(e),this}setRotationFromAxisAngle(e,t){this.quaternion.setFromAxisAngle(e,t)}setRotationFromEuler(e){this.quaternion.setFromEuler(e,!0)}setRotationFromMatrix(e){this.quaternion.setFromRotationMatrix(e)}setRotationFromQuaternion(e){this.quaternion.copy(e)}rotateOnAxis(e,t){return yr.setFromAxisAngle(e,t),this.quaternion.multiply(yr),this}rotateOnWorldAxis(e,t){return yr.setFromAxisAngle(e,t),this.quaternion.premultiply(yr),this}rotateX(e){return this.rotateOnAxis(Jl,e)}rotateY(e){return this.rotateOnAxis(Ql,e)}rotateZ(e){return this.rotateOnAxis(jl,e)}translateOnAxis(e,t){return Zl.copy(e).applyQuaternion(this.quaternion),this.position.add(Zl.multiplyScalar(t)),this}translateX(e){return this.translateOnAxis(Jl,e)}translateY(e){return this.translateOnAxis(Ql,e)}translateZ(e){return this.translateOnAxis(jl,e)}localToWorld(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(this.matrixWorld)}worldToLocal(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(di.copy(this.matrixWorld).invert())}lookAt(e,t,i){e.isVector3?Es.copy(e):Es.set(e,t,i);const r=this.parent;this.updateWorldMatrix(!0,!1),Qr.setFromMatrixPosition(this.matrixWorld),this.isCamera||this.isLight?di.lookAt(Qr,Es,this.up):di.lookAt(Es,Qr,this.up),this.quaternion.setFromRotationMatrix(di),r&&(di.extractRotation(r.matrixWorld),yr.setFromRotationMatrix(di),this.quaternion.premultiply(yr.invert()))}add(e){if(arguments.length>1){for(let t=0;t<arguments.length;t++)this.add(arguments[t]);return this}return e===this?(Et("Object3D.add: object can't be added as a child of itself.",e),this):(e&&e.isObject3D?(e.removeFromParent(),e.parent=this,this.children.push(e),e.dispatchEvent(ec),br.child=e,this.dispatchEvent(br),br.child=null):Et("Object3D.add: object not an instance of THREE.Object3D.",e),this)}remove(e){if(arguments.length>1){for(let i=0;i<arguments.length;i++)this.remove(arguments[i]);return this}const t=this.children.indexOf(e);return t!==-1&&(e.parent=null,this.children.splice(t,1),e.dispatchEvent(Xf),Pa.child=e,this.dispatchEvent(Pa),Pa.child=null),this}removeFromParent(){const e=this.parent;return e!==null&&e.remove(this),this}clear(){return this.remove(...this.children)}attach(e){return this.updateWorldMatrix(!0,!1),di.copy(this.matrixWorld).invert(),e.parent!==null&&(e.parent.updateWorldMatrix(!0,!1),di.multiply(e.parent.matrixWorld)),e.applyMatrix4(di),e.removeFromParent(),e.parent=this,this.children.push(e),e.updateWorldMatrix(!1,!0),e.dispatchEvent(ec),br.child=e,this.dispatchEvent(br),br.child=null,this}getObjectById(e){return this.getObjectByProperty("id",e)}getObjectByName(e){return this.getObjectByProperty("name",e)}getObjectByProperty(e,t){if(this[e]===t)return this;for(let i=0,r=this.children.length;i<r;i++){const a=this.children[i].getObjectByProperty(e,t);if(a!==void 0)return a}}getObjectsByProperty(e,t,i=[]){this[e]===t&&i.push(this);const r=this.children;for(let s=0,a=r.length;s<a;s++)r[s].getObjectsByProperty(e,t,i);return i}getWorldPosition(e){return this.updateWorldMatrix(!0,!1),e.setFromMatrixPosition(this.matrixWorld)}getWorldQuaternion(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Qr,e,Vf),e}getWorldScale(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Qr,Wf,e),e}getWorldDirection(e){this.updateWorldMatrix(!0,!1);const t=this.matrixWorld.elements;return e.set(t[8],t[9],t[10]).normalize()}raycast(){}traverse(e){e(this);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].traverse(e)}traverseVisible(e){if(this.visible===!1)return;e(this);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].traverseVisible(e)}traverseAncestors(e){const t=this.parent;t!==null&&(e(t),t.traverseAncestors(e))}updateMatrix(){this.matrix.compose(this.position,this.quaternion,this.scale);const e=this.pivot;if(e!==null){const t=e.x,i=e.y,r=e.z,s=this.matrix.elements;s[12]+=t-s[0]*t-s[4]*i-s[8]*r,s[13]+=i-s[1]*t-s[5]*i-s[9]*r,s[14]+=r-s[2]*t-s[6]*i-s[10]*r}this.matrixWorldNeedsUpdate=!0}updateMatrixWorld(e){this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||e)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,e=!0);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].updateMatrixWorld(e)}updateWorldMatrix(e,t,i=!1){const r=this.parent;if(e===!0&&r!==null&&r.updateWorldMatrix(!0,!1),this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||i)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,i=!0),t===!0){const s=this.children;for(let a=0,o=s.length;a<o;a++)s[a].updateWorldMatrix(!1,!0,i)}}toJSON(e){const t=e===void 0||typeof e=="string",i={};t&&(e={geometries:{},materials:{},textures:{},images:{},shapes:{},skeletons:{},animations:{},nodes:{}},i.metadata={version:4.7,type:"Object",generator:"Object3D.toJSON"});const r={};r.uuid=this.uuid,r.type=this.type,this.name!==""&&(r.name=this.name),this.castShadow===!0&&(r.castShadow=!0),this.receiveShadow===!0&&(r.receiveShadow=!0),this.visible===!1&&(r.visible=!1),this.frustumCulled===!1&&(r.frustumCulled=!1),this.renderOrder!==0&&(r.renderOrder=this.renderOrder),this.static!==!1&&(r.static=this.static),Object.keys(this.userData).length>0&&(r.userData=this.userData),r.layers=this.layers.mask,r.matrix=this.matrix.toArray(),r.up=this.up.toArray(),this.pivot!==null&&(r.pivot=this.pivot.toArray()),this.matrixAutoUpdate===!1&&(r.matrixAutoUpdate=!1),this.morphTargetDictionary!==void 0&&(r.morphTargetDictionary=Object.assign({},this.morphTargetDictionary)),this.morphTargetInfluences!==void 0&&(r.morphTargetInfluences=this.morphTargetInfluences.slice()),this.isInstancedMesh&&(r.type="InstancedMesh",r.count=this.count,r.instanceMatrix=this.instanceMatrix.toJSON(),this.instanceColor!==null&&(r.instanceColor=this.instanceColor.toJSON())),this.isBatchedMesh&&(r.type="BatchedMesh",r.perObjectFrustumCulled=this.perObjectFrustumCulled,r.sortObjects=this.sortObjects,r.drawRanges=this._drawRanges,r.reservedRanges=this._reservedRanges,r.geometryInfo=this._geometryInfo.map(o=>({...o,boundingBox:o.boundingBox?o.boundingBox.toJSON():void 0,boundingSphere:o.boundingSphere?o.boundingSphere.toJSON():void 0})),r.instanceInfo=this._instanceInfo.map(o=>({...o})),r.availableInstanceIds=this._availableInstanceIds.slice(),r.availableGeometryIds=this._availableGeometryIds.slice(),r.nextIndexStart=this._nextIndexStart,r.nextVertexStart=this._nextVertexStart,r.geometryCount=this._geometryCount,r.maxInstanceCount=this._maxInstanceCount,r.maxVertexCount=this._maxVertexCount,r.maxIndexCount=this._maxIndexCount,r.geometryInitialized=this._geometryInitialized,r.matricesTexture=this._matricesTexture.toJSON(e),r.indirectTexture=this._indirectTexture.toJSON(e),this._colorsTexture!==null&&(r.colorsTexture=this._colorsTexture.toJSON(e)),this.boundingSphere!==null&&(r.boundingSphere=this.boundingSphere.toJSON()),this.boundingBox!==null&&(r.boundingBox=this.boundingBox.toJSON()));function s(o,l){return o[l.uuid]===void 0&&(o[l.uuid]=l.toJSON(e)),l.uuid}if(this.isScene)this.background&&(this.background.isColor?r.background=this.background.toJSON():this.background.isTexture&&(r.background=this.background.toJSON(e).uuid)),this.environment&&this.environment.isTexture&&this.environment.isRenderTargetTexture!==!0&&(r.environment=this.environment.toJSON(e).uuid);else if(this.isMesh||this.isLine||this.isPoints){r.geometry=s(e.geometries,this.geometry);const o=this.geometry.parameters;if(o!==void 0&&o.shapes!==void 0){const l=o.shapes;if(Array.isArray(l))for(let c=0,f=l.length;c<f;c++){const u=l[c];s(e.shapes,u)}else s(e.shapes,l)}}if(this.isSkinnedMesh&&(r.bindMode=this.bindMode,r.bindMatrix=this.bindMatrix.toArray(),this.skeleton!==void 0&&(s(e.skeletons,this.skeleton),r.skeleton=this.skeleton.uuid)),this.material!==void 0)if(Array.isArray(this.material)){const o=[];for(let l=0,c=this.material.length;l<c;l++)o.push(s(e.materials,this.material[l]));r.material=o}else r.material=s(e.materials,this.material);if(this.children.length>0){r.children=[];for(let o=0;o<this.children.length;o++)r.children.push(this.children[o].toJSON(e).object)}if(this.animations.length>0){r.animations=[];for(let o=0;o<this.animations.length;o++){const l=this.animations[o];r.animations.push(s(e.animations,l))}}if(t){const o=a(e.geometries),l=a(e.materials),c=a(e.textures),f=a(e.images),u=a(e.shapes),d=a(e.skeletons),h=a(e.animations),_=a(e.nodes);o.length>0&&(i.geometries=o),l.length>0&&(i.materials=l),c.length>0&&(i.textures=c),f.length>0&&(i.images=f),u.length>0&&(i.shapes=u),d.length>0&&(i.skeletons=d),h.length>0&&(i.animations=h),_.length>0&&(i.nodes=_)}return i.object=r,i;function a(o){const l=[];for(const c in o){const f=o[c];delete f.metadata,l.push(f)}return l}}clone(e){return new this.constructor().copy(this,e)}copy(e,t=!0){if(this.name=e.name,this.up.copy(e.up),this.position.copy(e.position),this.rotation.order=e.rotation.order,this.quaternion.copy(e.quaternion),this.scale.copy(e.scale),this.pivot=e.pivot!==null?e.pivot.clone():null,this.matrix.copy(e.matrix),this.matrixWorld.copy(e.matrixWorld),this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrixWorldAutoUpdate=e.matrixWorldAutoUpdate,this.matrixWorldNeedsUpdate=e.matrixWorldNeedsUpdate,this.layers.mask=e.layers.mask,this.visible=e.visible,this.castShadow=e.castShadow,this.receiveShadow=e.receiveShadow,this.frustumCulled=e.frustumCulled,this.renderOrder=e.renderOrder,this.static=e.static,this.animations=e.animations.slice(),this.userData=JSON.parse(JSON.stringify(e.userData)),t===!0)for(let i=0;i<e.children.length;i++){const r=e.children[i];this.add(r.clone())}return this}}Tn.DEFAULT_UP=new le(0,1,0);Tn.DEFAULT_MATRIX_AUTO_UPDATE=!0;Tn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE=!0;class ws extends Tn{constructor(){super(),this.isGroup=!0,this.type="Group"}}const qf={type:"move"};class La{constructor(){this._targetRay=null,this._grip=null,this._hand=null}getHandSpace(){return this._hand===null&&(this._hand=new ws,this._hand.matrixAutoUpdate=!1,this._hand.visible=!1,this._hand.joints={},this._hand.inputState={pinching:!1}),this._hand}getTargetRaySpace(){return this._targetRay===null&&(this._targetRay=new ws,this._targetRay.matrixAutoUpdate=!1,this._targetRay.visible=!1,this._targetRay.hasLinearVelocity=!1,this._targetRay.linearVelocity=new le,this._targetRay.hasAngularVelocity=!1,this._targetRay.angularVelocity=new le),this._targetRay}getGripSpace(){return this._grip===null&&(this._grip=new ws,this._grip.matrixAutoUpdate=!1,this._grip.visible=!1,this._grip.hasLinearVelocity=!1,this._grip.linearVelocity=new le,this._grip.hasAngularVelocity=!1,this._grip.angularVelocity=new le,this._grip.eventsEnabled=!1),this._grip}dispatchEvent(e){return this._targetRay!==null&&this._targetRay.dispatchEvent(e),this._grip!==null&&this._grip.dispatchEvent(e),this._hand!==null&&this._hand.dispatchEvent(e),this}connect(e){if(e&&e.hand){const t=this._hand;if(t)for(const i of e.hand.values())this._getHandJoint(t,i)}return this.dispatchEvent({type:"connected",data:e}),this}disconnect(e){return this.dispatchEvent({type:"disconnected",data:e}),this._targetRay!==null&&(this._targetRay.visible=!1),this._grip!==null&&(this._grip.visible=!1),this._hand!==null&&(this._hand.visible=!1),this}update(e,t,i){let r=null,s=null,a=null;const o=this._targetRay,l=this._grip,c=this._hand;if(e&&t.session.visibilityState!=="visible-blurred"){if(c&&e.hand){a=!0;for(const x of e.hand.values()){const m=t.getJointPose(x,i),p=this._getHandJoint(c,x);m!==null&&(p.matrix.fromArray(m.transform.matrix),p.matrix.decompose(p.position,p.rotation,p.scale),p.matrixWorldNeedsUpdate=!0,p.jointRadius=m.radius),p.visible=m!==null}const f=c.joints["index-finger-tip"],u=c.joints["thumb-tip"],d=f.position.distanceTo(u.position),h=.02,_=.005;c.inputState.pinching&&d>h+_?(c.inputState.pinching=!1,this.dispatchEvent({type:"pinchend",handedness:e.handedness,target:this})):!c.inputState.pinching&&d<=h-_&&(c.inputState.pinching=!0,this.dispatchEvent({type:"pinchstart",handedness:e.handedness,target:this}))}else l!==null&&e.gripSpace&&(s=t.getPose(e.gripSpace,i),s!==null&&(l.matrix.fromArray(s.transform.matrix),l.matrix.decompose(l.position,l.rotation,l.scale),l.matrixWorldNeedsUpdate=!0,s.linearVelocity?(l.hasLinearVelocity=!0,l.linearVelocity.copy(s.linearVelocity)):l.hasLinearVelocity=!1,s.angularVelocity?(l.hasAngularVelocity=!0,l.angularVelocity.copy(s.angularVelocity)):l.hasAngularVelocity=!1,l.eventsEnabled&&l.dispatchEvent({type:"gripUpdated",data:e,target:this})));o!==null&&(r=t.getPose(e.targetRaySpace,i),r===null&&s!==null&&(r=s),r!==null&&(o.matrix.fromArray(r.transform.matrix),o.matrix.decompose(o.position,o.rotation,o.scale),o.matrixWorldNeedsUpdate=!0,r.linearVelocity?(o.hasLinearVelocity=!0,o.linearVelocity.copy(r.linearVelocity)):o.hasLinearVelocity=!1,r.angularVelocity?(o.hasAngularVelocity=!0,o.angularVelocity.copy(r.angularVelocity)):o.hasAngularVelocity=!1,this.dispatchEvent(qf)))}return o!==null&&(o.visible=r!==null),l!==null&&(l.visible=s!==null),c!==null&&(c.visible=a!==null),this}_getHandJoint(e,t){if(e.joints[t.jointName]===void 0){const i=new ws;i.matrixAutoUpdate=!1,i.visible=!1,e.joints[t.jointName]=i,e.add(i)}return e.joints[t.jointName]}}const Ad={aliceblue:15792383,antiquewhite:16444375,aqua:65535,aquamarine:8388564,azure:15794175,beige:16119260,bisque:16770244,black:0,blanchedalmond:16772045,blue:255,blueviolet:9055202,brown:10824234,burlywood:14596231,cadetblue:6266528,chartreuse:8388352,chocolate:13789470,coral:16744272,cornflowerblue:6591981,cornsilk:16775388,crimson:14423100,cyan:65535,darkblue:139,darkcyan:35723,darkgoldenrod:12092939,darkgray:11119017,darkgreen:25600,darkgrey:11119017,darkkhaki:12433259,darkmagenta:9109643,darkolivegreen:5597999,darkorange:16747520,darkorchid:10040012,darkred:9109504,darksalmon:15308410,darkseagreen:9419919,darkslateblue:4734347,darkslategray:3100495,darkslategrey:3100495,darkturquoise:52945,darkviolet:9699539,deeppink:16716947,deepskyblue:49151,dimgray:6908265,dimgrey:6908265,dodgerblue:2003199,firebrick:11674146,floralwhite:16775920,forestgreen:2263842,fuchsia:16711935,gainsboro:14474460,ghostwhite:16316671,gold:16766720,goldenrod:14329120,gray:8421504,green:32768,greenyellow:11403055,grey:8421504,honeydew:15794160,hotpink:16738740,indianred:13458524,indigo:4915330,ivory:16777200,khaki:15787660,lavender:15132410,lavenderblush:16773365,lawngreen:8190976,lemonchiffon:16775885,lightblue:11393254,lightcoral:15761536,lightcyan:14745599,lightgoldenrodyellow:16448210,lightgray:13882323,lightgreen:9498256,lightgrey:13882323,lightpink:16758465,lightsalmon:16752762,lightseagreen:2142890,lightskyblue:8900346,lightslategray:7833753,lightslategrey:7833753,lightsteelblue:11584734,lightyellow:16777184,lime:65280,limegreen:3329330,linen:16445670,magenta:16711935,maroon:8388608,mediumaquamarine:6737322,mediumblue:205,mediumorchid:12211667,mediumpurple:9662683,mediumseagreen:3978097,mediumslateblue:8087790,mediumspringgreen:64154,mediumturquoise:4772300,mediumvioletred:13047173,midnightblue:1644912,mintcream:16121850,mistyrose:16770273,moccasin:16770229,navajowhite:16768685,navy:128,oldlace:16643558,olive:8421376,olivedrab:7048739,orange:16753920,orangered:16729344,orchid:14315734,palegoldenrod:15657130,palegreen:10025880,paleturquoise:11529966,palevioletred:14381203,papayawhip:16773077,peachpuff:16767673,peru:13468991,pink:16761035,plum:14524637,powderblue:11591910,purple:8388736,rebeccapurple:6697881,red:16711680,rosybrown:12357519,royalblue:4286945,saddlebrown:9127187,salmon:16416882,sandybrown:16032864,seagreen:3050327,seashell:16774638,sienna:10506797,silver:12632256,skyblue:8900331,slateblue:6970061,slategray:7372944,slategrey:7372944,snow:16775930,springgreen:65407,steelblue:4620980,tan:13808780,teal:32896,thistle:14204888,tomato:16737095,turquoise:4251856,violet:15631086,wheat:16113331,white:16777215,whitesmoke:16119285,yellow:16776960,yellowgreen:10145074},Ii={h:0,s:0,l:0},Ts={h:0,s:0,l:0};function Ia(n,e,t){return t<0&&(t+=1),t>1&&(t-=1),t<1/6?n+(e-n)*6*t:t<1/2?e:t<2/3?n+(e-n)*6*(2/3-t):n}class It{constructor(e,t,i){return this.isColor=!0,this.r=1,this.g=1,this.b=1,this.set(e,t,i)}set(e,t,i){if(t===void 0&&i===void 0){const r=e;r&&r.isColor?this.copy(r):typeof r=="number"?this.setHex(r):typeof r=="string"&&this.setStyle(r)}else this.setRGB(e,t,i);return this}setScalar(e){return this.r=e,this.g=e,this.b=e,this}setHex(e,t=Ln){return e=Math.floor(e),this.r=(e>>16&255)/255,this.g=(e>>8&255)/255,this.b=(e&255)/255,yt.colorSpaceToWorking(this,t),this}setRGB(e,t,i,r=yt.workingColorSpace){return this.r=e,this.g=t,this.b=i,yt.colorSpaceToWorking(this,r),this}setHSL(e,t,i,r=yt.workingColorSpace){if(e=Df(e,1),t=bt(t,0,1),i=bt(i,0,1),t===0)this.r=this.g=this.b=i;else{const s=i<=.5?i*(1+t):i+t-i*t,a=2*i-s;this.r=Ia(a,s,e+1/3),this.g=Ia(a,s,e),this.b=Ia(a,s,e-1/3)}return yt.colorSpaceToWorking(this,r),this}setStyle(e,t=Ln){function i(s){s!==void 0&&parseFloat(s)<1&&st("Color: Alpha component of "+e+" will be ignored.")}let r;if(r=/^(\w+)\(([^\)]*)\)/.exec(e)){let s;const a=r[1],o=r[2];switch(a){case"rgb":case"rgba":if(s=/^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setRGB(Math.min(255,parseInt(s[1],10))/255,Math.min(255,parseInt(s[2],10))/255,Math.min(255,parseInt(s[3],10))/255,t);if(s=/^\s*(\d+)\%\s*,\s*(\d+)\%\s*,\s*(\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setRGB(Math.min(100,parseInt(s[1],10))/100,Math.min(100,parseInt(s[2],10))/100,Math.min(100,parseInt(s[3],10))/100,t);break;case"hsl":case"hsla":if(s=/^\s*(\d*\.?\d+)\s*,\s*(\d*\.?\d+)\%\s*,\s*(\d*\.?\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setHSL(parseFloat(s[1])/360,parseFloat(s[2])/100,parseFloat(s[3])/100,t);break;default:st("Color: Unknown color model "+e)}}else if(r=/^\#([A-Fa-f\d]+)$/.exec(e)){const s=r[1],a=s.length;if(a===3)return this.setRGB(parseInt(s.charAt(0),16)/15,parseInt(s.charAt(1),16)/15,parseInt(s.charAt(2),16)/15,t);if(a===6)return this.setHex(parseInt(s,16),t);st("Color: Invalid hex color "+e)}else if(e&&e.length>0)return this.setColorName(e,t);return this}setColorName(e,t=Ln){const i=Ad[e.toLowerCase()];return i!==void 0?this.setHex(i,t):st("Color: Unknown color "+e),this}clone(){return new this.constructor(this.r,this.g,this.b)}copy(e){return this.r=e.r,this.g=e.g,this.b=e.b,this}copySRGBToLinear(e){return this.r=Si(e.r),this.g=Si(e.g),this.b=Si(e.b),this}copyLinearToSRGB(e){return this.r=Or(e.r),this.g=Or(e.g),this.b=Or(e.b),this}convertSRGBToLinear(){return this.copySRGBToLinear(this),this}convertLinearToSRGB(){return this.copyLinearToSRGB(this),this}getHex(e=Ln){return yt.workingToColorSpace(dn.copy(this),e),Math.round(bt(dn.r*255,0,255))*65536+Math.round(bt(dn.g*255,0,255))*256+Math.round(bt(dn.b*255,0,255))}getHexString(e=Ln){return("000000"+this.getHex(e).toString(16)).slice(-6)}getHSL(e,t=yt.workingColorSpace){yt.workingToColorSpace(dn.copy(this),t);const i=dn.r,r=dn.g,s=dn.b,a=Math.max(i,r,s),o=Math.min(i,r,s);let l,c;const f=(o+a)/2;if(o===a)l=0,c=0;else{const u=a-o;switch(c=f<=.5?u/(a+o):u/(2-a-o),a){case i:l=(r-s)/u+(r<s?6:0);break;case r:l=(s-i)/u+2;break;case s:l=(i-r)/u+4;break}l/=6}return e.h=l,e.s=c,e.l=f,e}getRGB(e,t=yt.workingColorSpace){return yt.workingToColorSpace(dn.copy(this),t),e.r=dn.r,e.g=dn.g,e.b=dn.b,e}getStyle(e=Ln){yt.workingToColorSpace(dn.copy(this),e);const t=dn.r,i=dn.g,r=dn.b;return e!==Ln?`color(${e} ${t.toFixed(3)} ${i.toFixed(3)} ${r.toFixed(3)})`:`rgb(${Math.round(t*255)},${Math.round(i*255)},${Math.round(r*255)})`}offsetHSL(e,t,i){return this.getHSL(Ii),this.setHSL(Ii.h+e,Ii.s+t,Ii.l+i)}add(e){return this.r+=e.r,this.g+=e.g,this.b+=e.b,this}addColors(e,t){return this.r=e.r+t.r,this.g=e.g+t.g,this.b=e.b+t.b,this}addScalar(e){return this.r+=e,this.g+=e,this.b+=e,this}sub(e){return this.r=Math.max(0,this.r-e.r),this.g=Math.max(0,this.g-e.g),this.b=Math.max(0,this.b-e.b),this}multiply(e){return this.r*=e.r,this.g*=e.g,this.b*=e.b,this}multiplyScalar(e){return this.r*=e,this.g*=e,this.b*=e,this}lerp(e,t){return this.r+=(e.r-this.r)*t,this.g+=(e.g-this.g)*t,this.b+=(e.b-this.b)*t,this}lerpColors(e,t,i){return this.r=e.r+(t.r-e.r)*i,this.g=e.g+(t.g-e.g)*i,this.b=e.b+(t.b-e.b)*i,this}lerpHSL(e,t){this.getHSL(Ii),e.getHSL(Ts);const i=wa(Ii.h,Ts.h,t),r=wa(Ii.s,Ts.s,t),s=wa(Ii.l,Ts.l,t);return this.setHSL(i,r,s),this}setFromVector3(e){return this.r=e.x,this.g=e.y,this.b=e.z,this}applyMatrix3(e){const t=this.r,i=this.g,r=this.b,s=e.elements;return this.r=s[0]*t+s[3]*i+s[6]*r,this.g=s[1]*t+s[4]*i+s[7]*r,this.b=s[2]*t+s[5]*i+s[8]*r,this}equals(e){return e.r===this.r&&e.g===this.g&&e.b===this.b}fromArray(e,t=0){return this.r=e[t],this.g=e[t+1],this.b=e[t+2],this}toArray(e=[],t=0){return e[t]=this.r,e[t+1]=this.g,e[t+2]=this.b,e}fromBufferAttribute(e,t){return this.r=e.getX(t),this.g=e.getY(t),this.b=e.getZ(t),this}toJSON(){return this.getHex()}*[Symbol.iterator](){yield this.r,yield this.g,yield this.b}}const dn=new It;It.NAMES=Ad;class Yf extends Tn{constructor(){super(),this.isScene=!0,this.type="Scene",this.background=null,this.environment=null,this.fog=null,this.backgroundBlurriness=0,this.backgroundIntensity=1,this.backgroundRotation=new lr,this.environmentIntensity=1,this.environmentRotation=new lr,this.overrideMaterial=null,typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}copy(e,t){return super.copy(e,t),e.background!==null&&(this.background=e.background.clone()),e.environment!==null&&(this.environment=e.environment.clone()),e.fog!==null&&(this.fog=e.fog.clone()),this.backgroundBlurriness=e.backgroundBlurriness,this.backgroundIntensity=e.backgroundIntensity,this.backgroundRotation.copy(e.backgroundRotation),this.environmentIntensity=e.environmentIntensity,this.environmentRotation.copy(e.environmentRotation),e.overrideMaterial!==null&&(this.overrideMaterial=e.overrideMaterial.clone()),this.matrixAutoUpdate=e.matrixAutoUpdate,this}toJSON(e){const t=super.toJSON(e);return this.fog!==null&&(t.object.fog=this.fog.toJSON()),this.backgroundBlurriness>0&&(t.object.backgroundBlurriness=this.backgroundBlurriness),this.backgroundIntensity!==1&&(t.object.backgroundIntensity=this.backgroundIntensity),t.object.backgroundRotation=this.backgroundRotation.toArray(),this.environmentIntensity!==1&&(t.object.environmentIntensity=this.environmentIntensity),t.object.environmentRotation=this.environmentRotation.toArray(),t}}const Bn=new le,ui=new le,Da=new le,fi=new le,Sr=new le,Mr=new le,tc=new le,Na=new le,Ua=new le,Fa=new le,Oa=new Xt,Ba=new Xt,ka=new Xt;class Gn{constructor(e=new le,t=new le,i=new le){this.a=e,this.b=t,this.c=i}static getNormal(e,t,i,r){r.subVectors(i,t),Bn.subVectors(e,t),r.cross(Bn);const s=r.lengthSq();return s>0?r.multiplyScalar(1/Math.sqrt(s)):r.set(0,0,0)}static getBarycoord(e,t,i,r,s){Bn.subVectors(r,t),ui.subVectors(i,t),Da.subVectors(e,t);const a=Bn.dot(Bn),o=Bn.dot(ui),l=Bn.dot(Da),c=ui.dot(ui),f=ui.dot(Da),u=a*c-o*o;if(u===0)return s.set(0,0,0),null;const d=1/u,h=(c*l-o*f)*d,_=(a*f-o*l)*d;return s.set(1-h-_,_,h)}static containsPoint(e,t,i,r){return this.getBarycoord(e,t,i,r,fi)===null?!1:fi.x>=0&&fi.y>=0&&fi.x+fi.y<=1}static getInterpolation(e,t,i,r,s,a,o,l){return this.getBarycoord(e,t,i,r,fi)===null?(l.x=0,l.y=0,"z"in l&&(l.z=0),"w"in l&&(l.w=0),null):(l.setScalar(0),l.addScaledVector(s,fi.x),l.addScaledVector(a,fi.y),l.addScaledVector(o,fi.z),l)}static getInterpolatedAttribute(e,t,i,r,s,a){return Oa.setScalar(0),Ba.setScalar(0),ka.setScalar(0),Oa.fromBufferAttribute(e,t),Ba.fromBufferAttribute(e,i),ka.fromBufferAttribute(e,r),a.setScalar(0),a.addScaledVector(Oa,s.x),a.addScaledVector(Ba,s.y),a.addScaledVector(ka,s.z),a}static isFrontFacing(e,t,i,r){return Bn.subVectors(i,t),ui.subVectors(e,t),Bn.cross(ui).dot(r)<0}set(e,t,i){return this.a.copy(e),this.b.copy(t),this.c.copy(i),this}setFromPointsAndIndices(e,t,i,r){return this.a.copy(e[t]),this.b.copy(e[i]),this.c.copy(e[r]),this}setFromAttributeAndIndices(e,t,i,r){return this.a.fromBufferAttribute(e,t),this.b.fromBufferAttribute(e,i),this.c.fromBufferAttribute(e,r),this}clone(){return new this.constructor().copy(this)}copy(e){return this.a.copy(e.a),this.b.copy(e.b),this.c.copy(e.c),this}getArea(){return Bn.subVectors(this.c,this.b),ui.subVectors(this.a,this.b),Bn.cross(ui).length()*.5}getMidpoint(e){return e.addVectors(this.a,this.b).add(this.c).multiplyScalar(1/3)}getNormal(e){return Gn.getNormal(this.a,this.b,this.c,e)}getPlane(e){return e.setFromCoplanarPoints(this.a,this.b,this.c)}getBarycoord(e,t){return Gn.getBarycoord(e,this.a,this.b,this.c,t)}getInterpolation(e,t,i,r,s){return Gn.getInterpolation(e,this.a,this.b,this.c,t,i,r,s)}containsPoint(e){return Gn.containsPoint(e,this.a,this.b,this.c)}isFrontFacing(e){return Gn.isFrontFacing(this.a,this.b,this.c,e)}intersectsBox(e){return e.intersectsTriangle(this)}closestPointToPoint(e,t){const i=this.a,r=this.b,s=this.c;let a,o;Sr.subVectors(r,i),Mr.subVectors(s,i),Na.subVectors(e,i);const l=Sr.dot(Na),c=Mr.dot(Na);if(l<=0&&c<=0)return t.copy(i);Ua.subVectors(e,r);const f=Sr.dot(Ua),u=Mr.dot(Ua);if(f>=0&&u<=f)return t.copy(r);const d=l*u-f*c;if(d<=0&&l>=0&&f<=0)return a=l/(l-f),t.copy(i).addScaledVector(Sr,a);Fa.subVectors(e,s);const h=Sr.dot(Fa),_=Mr.dot(Fa);if(_>=0&&h<=_)return t.copy(s);const x=h*c-l*_;if(x<=0&&c>=0&&_<=0)return o=c/(c-_),t.copy(i).addScaledVector(Mr,o);const m=f*_-h*u;if(m<=0&&u-f>=0&&h-_>=0)return tc.subVectors(s,r),o=(u-f)/(u-f+(h-_)),t.copy(r).addScaledVector(tc,o);const p=1/(m+x+d);return a=x*p,o=d*p,t.copy(i).addScaledVector(Sr,a).addScaledVector(Mr,o)}equals(e){return e.a.equals(this.a)&&e.b.equals(this.b)&&e.c.equals(this.c)}}class ps{constructor(e=new le(1/0,1/0,1/0),t=new le(-1/0,-1/0,-1/0)){this.isBox3=!0,this.min=e,this.max=t}set(e,t){return this.min.copy(e),this.max.copy(t),this}setFromArray(e){this.makeEmpty();for(let t=0,i=e.length;t<i;t+=3)this.expandByPoint(kn.fromArray(e,t));return this}setFromBufferAttribute(e){this.makeEmpty();for(let t=0,i=e.count;t<i;t++)this.expandByPoint(kn.fromBufferAttribute(e,t));return this}setFromPoints(e){this.makeEmpty();for(let t=0,i=e.length;t<i;t++)this.expandByPoint(e[t]);return this}setFromCenterAndSize(e,t){const i=kn.copy(t).multiplyScalar(.5);return this.min.copy(e).sub(i),this.max.copy(e).add(i),this}setFromObject(e,t=!1){return this.makeEmpty(),this.expandByObject(e,t)}clone(){return new this.constructor().copy(this)}copy(e){return this.min.copy(e.min),this.max.copy(e.max),this}makeEmpty(){return this.min.x=this.min.y=this.min.z=1/0,this.max.x=this.max.y=this.max.z=-1/0,this}isEmpty(){return this.max.x<this.min.x||this.max.y<this.min.y||this.max.z<this.min.z}getCenter(e){return this.isEmpty()?e.set(0,0,0):e.addVectors(this.min,this.max).multiplyScalar(.5)}getSize(e){return this.isEmpty()?e.set(0,0,0):e.subVectors(this.max,this.min)}expandByPoint(e){return this.min.min(e),this.max.max(e),this}expandByVector(e){return this.min.sub(e),this.max.add(e),this}expandByScalar(e){return this.min.addScalar(-e),this.max.addScalar(e),this}expandByObject(e,t=!1){e.updateWorldMatrix(!1,!1);const i=e.geometry;if(i!==void 0){const s=i.getAttribute("position");if(t===!0&&s!==void 0&&e.isInstancedMesh!==!0)for(let a=0,o=s.count;a<o;a++)e.isMesh===!0?e.getVertexPosition(a,kn):kn.fromBufferAttribute(s,a),kn.applyMatrix4(e.matrixWorld),this.expandByPoint(kn);else e.boundingBox!==void 0?(e.boundingBox===null&&e.computeBoundingBox(),As.copy(e.boundingBox)):(i.boundingBox===null&&i.computeBoundingBox(),As.copy(i.boundingBox)),As.applyMatrix4(e.matrixWorld),this.union(As)}const r=e.children;for(let s=0,a=r.length;s<a;s++)this.expandByObject(r[s],t);return this}containsPoint(e){return e.x>=this.min.x&&e.x<=this.max.x&&e.y>=this.min.y&&e.y<=this.max.y&&e.z>=this.min.z&&e.z<=this.max.z}containsBox(e){return this.min.x<=e.min.x&&e.max.x<=this.max.x&&this.min.y<=e.min.y&&e.max.y<=this.max.y&&this.min.z<=e.min.z&&e.max.z<=this.max.z}getParameter(e,t){return t.set((e.x-this.min.x)/(this.max.x-this.min.x),(e.y-this.min.y)/(this.max.y-this.min.y),(e.z-this.min.z)/(this.max.z-this.min.z))}intersectsBox(e){return e.max.x>=this.min.x&&e.min.x<=this.max.x&&e.max.y>=this.min.y&&e.min.y<=this.max.y&&e.max.z>=this.min.z&&e.min.z<=this.max.z}intersectsSphere(e){return this.clampPoint(e.center,kn),kn.distanceToSquared(e.center)<=e.radius*e.radius}intersectsPlane(e){let t,i;return e.normal.x>0?(t=e.normal.x*this.min.x,i=e.normal.x*this.max.x):(t=e.normal.x*this.max.x,i=e.normal.x*this.min.x),e.normal.y>0?(t+=e.normal.y*this.min.y,i+=e.normal.y*this.max.y):(t+=e.normal.y*this.max.y,i+=e.normal.y*this.min.y),e.normal.z>0?(t+=e.normal.z*this.min.z,i+=e.normal.z*this.max.z):(t+=e.normal.z*this.max.z,i+=e.normal.z*this.min.z),t<=-e.constant&&i>=-e.constant}intersectsTriangle(e){if(this.isEmpty())return!1;this.getCenter(jr),Rs.subVectors(this.max,jr),Er.subVectors(e.a,jr),wr.subVectors(e.b,jr),Tr.subVectors(e.c,jr),Di.subVectors(wr,Er),Ni.subVectors(Tr,wr),$i.subVectors(Er,Tr);let t=[0,-Di.z,Di.y,0,-Ni.z,Ni.y,0,-$i.z,$i.y,Di.z,0,-Di.x,Ni.z,0,-Ni.x,$i.z,0,-$i.x,-Di.y,Di.x,0,-Ni.y,Ni.x,0,-$i.y,$i.x,0];return!za(t,Er,wr,Tr,Rs)||(t=[1,0,0,0,1,0,0,0,1],!za(t,Er,wr,Tr,Rs))?!1:(Cs.crossVectors(Di,Ni),t=[Cs.x,Cs.y,Cs.z],za(t,Er,wr,Tr,Rs))}clampPoint(e,t){return t.copy(e).clamp(this.min,this.max)}distanceToPoint(e){return this.clampPoint(e,kn).distanceTo(e)}getBoundingSphere(e){return this.isEmpty()?e.makeEmpty():(this.getCenter(e.center),e.radius=this.getSize(kn).length()*.5),e}intersect(e){return this.min.max(e.min),this.max.min(e.max),this.isEmpty()&&this.makeEmpty(),this}union(e){return this.min.min(e.min),this.max.max(e.max),this}applyMatrix4(e){return this.isEmpty()?this:(hi[0].set(this.min.x,this.min.y,this.min.z).applyMatrix4(e),hi[1].set(this.min.x,this.min.y,this.max.z).applyMatrix4(e),hi[2].set(this.min.x,this.max.y,this.min.z).applyMatrix4(e),hi[3].set(this.min.x,this.max.y,this.max.z).applyMatrix4(e),hi[4].set(this.max.x,this.min.y,this.min.z).applyMatrix4(e),hi[5].set(this.max.x,this.min.y,this.max.z).applyMatrix4(e),hi[6].set(this.max.x,this.max.y,this.min.z).applyMatrix4(e),hi[7].set(this.max.x,this.max.y,this.max.z).applyMatrix4(e),this.setFromPoints(hi),this)}translate(e){return this.min.add(e),this.max.add(e),this}equals(e){return e.min.equals(this.min)&&e.max.equals(this.max)}toJSON(){return{min:this.min.toArray(),max:this.max.toArray()}}fromJSON(e){return this.min.fromArray(e.min),this.max.fromArray(e.max),this}}const hi=[new le,new le,new le,new le,new le,new le,new le,new le],kn=new le,As=new ps,Er=new le,wr=new le,Tr=new le,Di=new le,Ni=new le,$i=new le,jr=new le,Rs=new le,Cs=new le,Ki=new le;function za(n,e,t,i,r){for(let s=0,a=n.length-3;s<=a;s+=3){Ki.fromArray(n,s);const o=r.x*Math.abs(Ki.x)+r.y*Math.abs(Ki.y)+r.z*Math.abs(Ki.z),l=e.dot(Ki),c=t.dot(Ki),f=i.dot(Ki);if(Math.max(-Math.max(l,c,f),Math.min(l,c,f))>o)return!1}return!0}const Kt=new le,Ps=new At;let $f=0;class Wn extends dr{constructor(e,t,i=!1){if(super(),Array.isArray(e))throw new TypeError("THREE.BufferAttribute: array should be a Typed Array.");this.isBufferAttribute=!0,Object.defineProperty(this,"id",{value:$f++}),this.name="",this.array=e,this.itemSize=t,this.count=e!==void 0?e.length/t:0,this.normalized=i,this.usage=$o,this.updateRanges=[],this.gpuType=ei,this.version=0}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.name=e.name,this.array=new e.array.constructor(e.array),this.itemSize=e.itemSize,this.count=e.count,this.normalized=e.normalized,this.usage=e.usage,this.gpuType=e.gpuType,this}copyAt(e,t,i){e*=this.itemSize,i*=t.itemSize;for(let r=0,s=this.itemSize;r<s;r++)this.array[e+r]=t.array[i+r];return this}copyArray(e){return this.array.set(e),this}applyMatrix3(e){if(this.itemSize===2)for(let t=0,i=this.count;t<i;t++)Ps.fromBufferAttribute(this,t),Ps.applyMatrix3(e),this.setXY(t,Ps.x,Ps.y);else if(this.itemSize===3)for(let t=0,i=this.count;t<i;t++)Kt.fromBufferAttribute(this,t),Kt.applyMatrix3(e),this.setXYZ(t,Kt.x,Kt.y,Kt.z);return this}applyMatrix4(e){for(let t=0,i=this.count;t<i;t++)Kt.fromBufferAttribute(this,t),Kt.applyMatrix4(e),this.setXYZ(t,Kt.x,Kt.y,Kt.z);return this}applyNormalMatrix(e){for(let t=0,i=this.count;t<i;t++)Kt.fromBufferAttribute(this,t),Kt.applyNormalMatrix(e),this.setXYZ(t,Kt.x,Kt.y,Kt.z);return this}transformDirection(e){for(let t=0,i=this.count;t<i;t++)Kt.fromBufferAttribute(this,t),Kt.transformDirection(e),this.setXYZ(t,Kt.x,Kt.y,Kt.z);return this}set(e,t=0){return this.array.set(e,t),this}getComponent(e,t){let i=this.array[e*this.itemSize+t];return this.normalized&&(i=jn(i,this.array)),i}setComponent(e,t,i){return this.normalized&&(i=Ut(i,this.array)),this.array[e*this.itemSize+t]=i,this}getX(e){let t=this.array[e*this.itemSize];return this.normalized&&(t=jn(t,this.array)),t}setX(e,t){return this.normalized&&(t=Ut(t,this.array)),this.array[e*this.itemSize]=t,this}getY(e){let t=this.array[e*this.itemSize+1];return this.normalized&&(t=jn(t,this.array)),t}setY(e,t){return this.normalized&&(t=Ut(t,this.array)),this.array[e*this.itemSize+1]=t,this}getZ(e){let t=this.array[e*this.itemSize+2];return this.normalized&&(t=jn(t,this.array)),t}setZ(e,t){return this.normalized&&(t=Ut(t,this.array)),this.array[e*this.itemSize+2]=t,this}getW(e){let t=this.array[e*this.itemSize+3];return this.normalized&&(t=jn(t,this.array)),t}setW(e,t){return this.normalized&&(t=Ut(t,this.array)),this.array[e*this.itemSize+3]=t,this}setXY(e,t,i){return e*=this.itemSize,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array)),this.array[e+0]=t,this.array[e+1]=i,this}setXYZ(e,t,i,r){return e*=this.itemSize,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array),r=Ut(r,this.array)),this.array[e+0]=t,this.array[e+1]=i,this.array[e+2]=r,this}setXYZW(e,t,i,r,s){return e*=this.itemSize,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array),r=Ut(r,this.array),s=Ut(s,this.array)),this.array[e+0]=t,this.array[e+1]=i,this.array[e+2]=r,this.array[e+3]=s,this}onUpload(e){return this.onUploadCallback=e,this}clone(){return new this.constructor(this.array,this.itemSize).copy(this)}toJSON(){const e={itemSize:this.itemSize,type:this.array.constructor.name,array:Array.from(this.array),normalized:this.normalized};return this.name!==""&&(e.name=this.name),this.usage!==$o&&(e.usage=this.usage),e}dispose(){this.dispatchEvent({type:"dispose"})}}class Rd extends Wn{constructor(e,t,i){super(new Uint16Array(e),t,i)}}class Cd extends Wn{constructor(e,t,i){super(new Uint32Array(e),t,i)}}class Mi extends Wn{constructor(e,t,i){super(new Float32Array(e),t,i)}}const Kf=new ps,es=new le,Ga=new le;class gl{constructor(e=new le,t=-1){this.isSphere=!0,this.center=e,this.radius=t}set(e,t){return this.center.copy(e),this.radius=t,this}setFromPoints(e,t){const i=this.center;t!==void 0?i.copy(t):Kf.setFromPoints(e).getCenter(i);let r=0;for(let s=0,a=e.length;s<a;s++)r=Math.max(r,i.distanceToSquared(e[s]));return this.radius=Math.sqrt(r),this}copy(e){return this.center.copy(e.center),this.radius=e.radius,this}isEmpty(){return this.radius<0}makeEmpty(){return this.center.set(0,0,0),this.radius=-1,this}containsPoint(e){return e.distanceToSquared(this.center)<=this.radius*this.radius}distanceToPoint(e){return e.distanceTo(this.center)-this.radius}intersectsSphere(e){const t=this.radius+e.radius;return e.center.distanceToSquared(this.center)<=t*t}intersectsBox(e){return e.intersectsSphere(this)}intersectsPlane(e){return Math.abs(e.distanceToPoint(this.center))<=this.radius}clampPoint(e,t){const i=this.center.distanceToSquared(e);return t.copy(e),i>this.radius*this.radius&&(t.sub(this.center).normalize(),t.multiplyScalar(this.radius).add(this.center)),t}getBoundingBox(e){return this.isEmpty()?(e.makeEmpty(),e):(e.set(this.center,this.center),e.expandByScalar(this.radius),e)}applyMatrix4(e){return this.center.applyMatrix4(e),this.radius=this.radius*e.getMaxScaleOnAxis(),this}translate(e){return this.center.add(e),this}expandByPoint(e){if(this.isEmpty())return this.center.copy(e),this.radius=0,this;es.subVectors(e,this.center);const t=es.lengthSq();if(t>this.radius*this.radius){const i=Math.sqrt(t),r=(i-this.radius)*.5;this.center.addScaledVector(es,r/i),this.radius+=r}return this}union(e){return e.isEmpty()?this:this.isEmpty()?(this.copy(e),this):(this.center.equals(e.center)===!0?this.radius=Math.max(this.radius,e.radius):(Ga.subVectors(e.center,this.center).setLength(e.radius),this.expandByPoint(es.copy(e.center).add(Ga)),this.expandByPoint(es.copy(e.center).sub(Ga))),this)}equals(e){return e.center.equals(this.center)&&e.radius===this.radius}clone(){return new this.constructor().copy(this)}toJSON(){return{radius:this.radius,center:this.center.toArray()}}fromJSON(e){return this.radius=e.radius,this.center.fromArray(e.center),this}}let Zf=0;const Cn=new Zt,Ha=new Tn,Ar=new le,En=new ps,ts=new ps,nn=new le;class ai extends dr{constructor(){super(),this.isBufferGeometry=!0,Object.defineProperty(this,"id",{value:Zf++}),this.uuid=zi(),this.name="",this.type="BufferGeometry",this.index=null,this.indirect=null,this.indirectOffset=0,this.attributes={},this.morphAttributes={},this.morphTargetsRelative=!1,this.groups=[],this.boundingBox=null,this.boundingSphere=null,this.drawRange={start:0,count:1/0},this.userData={},this._transformed=!1}getIndex(){return this.index}setIndex(e){return Array.isArray(e)?this.index=new(Cf(e)?Cd:Rd)(e,1):this.index=e,this}setIndirect(e,t=0){return this.indirect=e,this.indirectOffset=t,this}getIndirect(){return this.indirect}getAttribute(e){return this.attributes[e]}setAttribute(e,t){return this.attributes[e]=t,this}deleteAttribute(e){return delete this.attributes[e],this}hasAttribute(e){return this.attributes[e]!==void 0}addGroup(e,t,i=0){this.groups.push({start:e,count:t,materialIndex:i})}clearGroups(){this.groups=[]}setDrawRange(e,t){this.drawRange.start=e,this.drawRange.count=t}applyMatrix4(e){const t=this.attributes.position;t!==void 0&&(t.applyMatrix4(e),t.needsUpdate=!0);const i=this.attributes.normal;if(i!==void 0){const s=new ct().getNormalMatrix(e);i.applyNormalMatrix(s),i.needsUpdate=!0}const r=this.attributes.tangent;return r!==void 0&&(r.transformDirection(e),r.needsUpdate=!0),this.boundingBox!==null&&this.computeBoundingBox(),this.boundingSphere!==null&&this.computeBoundingSphere(),this._transformed=!0,this}applyQuaternion(e){return Cn.makeRotationFromQuaternion(e),this.applyMatrix4(Cn),this}rotateX(e){return Cn.makeRotationX(e),this.applyMatrix4(Cn),this}rotateY(e){return Cn.makeRotationY(e),this.applyMatrix4(Cn),this}rotateZ(e){return Cn.makeRotationZ(e),this.applyMatrix4(Cn),this}translate(e,t,i){return Cn.makeTranslation(e,t,i),this.applyMatrix4(Cn),this}scale(e,t,i){return Cn.makeScale(e,t,i),this.applyMatrix4(Cn),this}lookAt(e){return Ha.lookAt(e),Ha.updateMatrix(),this.applyMatrix4(Ha.matrix),this}center(){return this.computeBoundingBox(),this.boundingBox.getCenter(Ar).negate(),this.translate(Ar.x,Ar.y,Ar.z),this}setFromPoints(e){const t=this.getAttribute("position");if(t===void 0){const i=[];for(let r=0,s=e.length;r<s;r++){const a=e[r];i.push(a.x,a.y,a.z||0)}this.setAttribute("position",new Mi(i,3))}else{const i=Math.min(e.length,t.count);for(let r=0;r<i;r++){const s=e[r];t.setXYZ(r,s.x,s.y,s.z||0)}e.length>t.count&&st("BufferGeometry: Buffer size too small for points data. Use .dispose() and create a new geometry."),t.needsUpdate=!0}return this}computeBoundingBox(){this.boundingBox===null&&(this.boundingBox=new ps);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){Et("BufferGeometry.computeBoundingBox(): GLBufferAttribute requires a manual bounding box.",this),this.boundingBox.set(new le(-1/0,-1/0,-1/0),new le(1/0,1/0,1/0));return}if(e!==void 0){if(this.boundingBox.setFromBufferAttribute(e),t)for(let i=0,r=t.length;i<r;i++){const s=t[i];En.setFromBufferAttribute(s),this.morphTargetsRelative?(nn.addVectors(this.boundingBox.min,En.min),this.boundingBox.expandByPoint(nn),nn.addVectors(this.boundingBox.max,En.max),this.boundingBox.expandByPoint(nn)):(this.boundingBox.expandByPoint(En.min),this.boundingBox.expandByPoint(En.max))}}else this.boundingBox.makeEmpty();(isNaN(this.boundingBox.min.x)||isNaN(this.boundingBox.min.y)||isNaN(this.boundingBox.min.z))&&Et('BufferGeometry.computeBoundingBox(): Computed min/max have NaN values. The "position" attribute is likely to have NaN values.',this)}computeBoundingSphere(){this.boundingSphere===null&&(this.boundingSphere=new gl);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){Et("BufferGeometry.computeBoundingSphere(): GLBufferAttribute requires a manual bounding sphere.",this),this.boundingSphere.set(new le,1/0);return}if(e){const i=this.boundingSphere.center;if(En.setFromBufferAttribute(e),t)for(let s=0,a=t.length;s<a;s++){const o=t[s];ts.setFromBufferAttribute(o),this.morphTargetsRelative?(nn.addVectors(En.min,ts.min),En.expandByPoint(nn),nn.addVectors(En.max,ts.max),En.expandByPoint(nn)):(En.expandByPoint(ts.min),En.expandByPoint(ts.max))}En.getCenter(i);let r=0;for(let s=0,a=e.count;s<a;s++)nn.fromBufferAttribute(e,s),r=Math.max(r,i.distanceToSquared(nn));if(t)for(let s=0,a=t.length;s<a;s++){const o=t[s],l=this.morphTargetsRelative;for(let c=0,f=o.count;c<f;c++)nn.fromBufferAttribute(o,c),l&&(Ar.fromBufferAttribute(e,c),nn.add(Ar)),r=Math.max(r,i.distanceToSquared(nn))}this.boundingSphere.radius=Math.sqrt(r),isNaN(this.boundingSphere.radius)&&Et('BufferGeometry.computeBoundingSphere(): Computed radius is NaN. The "position" attribute is likely to have NaN values.',this)}}computeTangents(){const e=this.index,t=this.attributes;if(e===null||t.position===void 0||t.normal===void 0||t.uv===void 0){Et("BufferGeometry: .computeTangents() failed. Missing required attributes (index, position, normal or uv)");return}const i=t.position,r=t.normal,s=t.uv;let a=this.getAttribute("tangent");(a===void 0||a.count!==i.count)&&(a=new Wn(new Float32Array(4*i.count),4),this.setAttribute("tangent",a));const o=[],l=[];for(let v=0;v<i.count;v++)o[v]=new le,l[v]=new le;const c=new le,f=new le,u=new le,d=new At,h=new At,_=new At,x=new le,m=new le;function p(v,b,w){c.fromBufferAttribute(i,v),f.fromBufferAttribute(i,b),u.fromBufferAttribute(i,w),d.fromBufferAttribute(s,v),h.fromBufferAttribute(s,b),_.fromBufferAttribute(s,w),f.sub(c),u.sub(c),h.sub(d),_.sub(d);const E=1/(h.x*_.y-_.x*h.y);isFinite(E)&&(x.copy(f).multiplyScalar(_.y).addScaledVector(u,-h.y).multiplyScalar(E),m.copy(u).multiplyScalar(h.x).addScaledVector(f,-_.x).multiplyScalar(E),o[v].add(x),o[b].add(x),o[w].add(x),l[v].add(m),l[b].add(m),l[w].add(m))}let C=this.groups;C.length===0&&(C=[{start:0,count:e.count}]);for(let v=0,b=C.length;v<b;++v){const w=C[v],E=w.start,N=w.count;for(let G=E,W=E+N;G<W;G+=3)p(e.getX(G+0),e.getX(G+1),e.getX(G+2))}const D=new le,M=new le,L=new le,A=new le;function P(v){L.fromBufferAttribute(r,v),A.copy(L);const b=o[v];D.copy(b),D.sub(L.multiplyScalar(L.dot(b))).normalize(),M.crossVectors(A,b);const E=M.dot(l[v])<0?-1:1;a.setXYZW(v,D.x,D.y,D.z,E)}for(let v=0,b=C.length;v<b;++v){const w=C[v],E=w.start,N=w.count;for(let G=E,W=E+N;G<W;G+=3)P(e.getX(G+0)),P(e.getX(G+1)),P(e.getX(G+2))}this._transformed=!0}computeVertexNormals(){const e=this.index,t=this.getAttribute("position");if(t!==void 0){let i=this.getAttribute("normal");if(i===void 0||i.count!==t.count)i=new Wn(new Float32Array(t.count*3),3),this.setAttribute("normal",i);else for(let d=0,h=i.count;d<h;d++)i.setXYZ(d,0,0,0);const r=new le,s=new le,a=new le,o=new le,l=new le,c=new le,f=new le,u=new le;if(e)for(let d=0,h=e.count;d<h;d+=3){const _=e.getX(d+0),x=e.getX(d+1),m=e.getX(d+2);r.fromBufferAttribute(t,_),s.fromBufferAttribute(t,x),a.fromBufferAttribute(t,m),f.subVectors(a,s),u.subVectors(r,s),f.cross(u),o.fromBufferAttribute(i,_),l.fromBufferAttribute(i,x),c.fromBufferAttribute(i,m),o.add(f),l.add(f),c.add(f),i.setXYZ(_,o.x,o.y,o.z),i.setXYZ(x,l.x,l.y,l.z),i.setXYZ(m,c.x,c.y,c.z)}else for(let d=0,h=t.count;d<h;d+=3)r.fromBufferAttribute(t,d+0),s.fromBufferAttribute(t,d+1),a.fromBufferAttribute(t,d+2),f.subVectors(a,s),u.subVectors(r,s),f.cross(u),i.setXYZ(d+0,f.x,f.y,f.z),i.setXYZ(d+1,f.x,f.y,f.z),i.setXYZ(d+2,f.x,f.y,f.z);this.normalizeNormals(),i.needsUpdate=!0}}normalizeNormals(){const e=this.attributes.normal;for(let t=0,i=e.count;t<i;t++)nn.fromBufferAttribute(e,t),nn.normalize(),e.setXYZ(t,nn.x,nn.y,nn.z)}toNonIndexed(){function e(o,l){const c=o.array,f=o.itemSize,u=o.normalized,d=new c.constructor(l.length*f);let h=0,_=0;for(let x=0,m=l.length;x<m;x++){o.isInterleavedBufferAttribute?h=l[x]*o.data.stride+o.offset:h=l[x]*f;for(let p=0;p<f;p++)d[_++]=c[h++]}return new Wn(d,f,u)}if(this.index===null)return st("BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed."),this;const t=new ai,i=this.index.array,r=this.attributes;for(const o in r){const l=r[o],c=e(l,i);t.setAttribute(o,c)}const s=this.morphAttributes;for(const o in s){const l=[],c=s[o];for(let f=0,u=c.length;f<u;f++){const d=c[f],h=e(d,i);l.push(h)}t.morphAttributes[o]=l}t.morphTargetsRelative=this.morphTargetsRelative;const a=this.groups;for(let o=0,l=a.length;o<l;o++){const c=a[o];t.addGroup(c.start,c.count,c.materialIndex)}return t}toJSON(){const e={metadata:{version:4.7,type:"BufferGeometry",generator:"BufferGeometry.toJSON"}};if(e.uuid=this.uuid,e.type=this.parameters!==void 0&&this._transformed===!0?"BufferGeometry":this.type,this.name!==""&&(e.name=this.name),Object.keys(this.userData).length>0&&(e.userData=this.userData),this.parameters!==void 0&&this._transformed!==!0){const l=this.parameters;for(const c in l)l[c]!==void 0&&(e[c]=l[c]);return e}e.data={attributes:{}};const t=this.index;t!==null&&(e.data.index={type:t.array.constructor.name,array:Array.prototype.slice.call(t.array)});const i=this.attributes;for(const l in i){const c=i[l];e.data.attributes[l]=c.toJSON(e.data)}const r={};let s=!1;for(const l in this.morphAttributes){const c=this.morphAttributes[l],f=[];for(let u=0,d=c.length;u<d;u++){const h=c[u];f.push(h.toJSON(e.data))}f.length>0&&(r[l]=f,s=!0)}s&&(e.data.morphAttributes=r,e.data.morphTargetsRelative=this.morphTargetsRelative);const a=this.groups;a.length>0&&(e.data.groups=JSON.parse(JSON.stringify(a)));const o=this.boundingSphere;return o!==null&&(e.data.boundingSphere=o.toJSON()),e}clone(){return new this.constructor().copy(this)}copy(e){this.index=null,this.attributes={},this.morphAttributes={},this.groups=[],this.boundingBox=null,this.boundingSphere=null;const t={};this.name=e.name;const i=e.index;i!==null&&this.setIndex(i.clone());const r=e.attributes;for(const c in r){const f=r[c];this.setAttribute(c,f.clone(t))}const s=e.morphAttributes;for(const c in s){const f=[],u=s[c];for(let d=0,h=u.length;d<h;d++)f.push(u[d].clone(t));this.morphAttributes[c]=f}this.morphTargetsRelative=e.morphTargetsRelative;const a=e.groups;for(let c=0,f=a.length;c<f;c++){const u=a[c];this.addGroup(u.start,u.count,u.materialIndex)}const o=e.boundingBox;o!==null&&(this.boundingBox=o.clone());const l=e.boundingSphere;return l!==null&&(this.boundingSphere=l.clone()),this.drawRange.start=e.drawRange.start,this.drawRange.count=e.drawRange.count,this.userData=e.userData,this._transformed=e._transformed,this}dispose(){this.dispatchEvent({type:"dispose"})}}class Jf{constructor(e,t){this.isInterleavedBuffer=!0,this.array=e,this.stride=t,this.count=e!==void 0?e.length/t:0,this.usage=$o,this.updateRanges=[],this.version=0,this.uuid=zi()}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.array=new e.array.constructor(e.array),this.count=e.count,this.stride=e.stride,this.usage=e.usage,this}copyAt(e,t,i){e*=this.stride,i*=t.stride;for(let r=0,s=this.stride;r<s;r++)this.array[e+r]=t.array[i+r];return this}set(e,t=0){return this.array.set(e,t),this}clone(e){e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=zi()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=this.array.slice(0).buffer);const t=new this.array.constructor(e.arrayBuffers[this.array.buffer._uuid]),i=new this.constructor(t,this.stride);return i.setUsage(this.usage),i}onUpload(e){return this.onUploadCallback=e,this}toJSON(e){return e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=zi()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=Array.from(new Uint32Array(this.array.buffer))),{uuid:this.uuid,buffer:this.array.buffer._uuid,type:this.array.constructor.name,stride:this.stride}}}const fn=new le;class _l{constructor(e,t,i,r=!1){this.isInterleavedBufferAttribute=!0,this.name="",this.data=e,this.itemSize=t,this.offset=i,this.normalized=r}get count(){return this.data.count}get array(){return this.data.array}set needsUpdate(e){this.data.needsUpdate=e}applyMatrix4(e){for(let t=0,i=this.data.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.applyMatrix4(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}applyNormalMatrix(e){for(let t=0,i=this.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.applyNormalMatrix(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}transformDirection(e){for(let t=0,i=this.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.transformDirection(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}getComponent(e,t){let i=this.array[e*this.data.stride+this.offset+t];return this.normalized&&(i=jn(i,this.array)),i}setComponent(e,t,i){return this.normalized&&(i=Ut(i,this.array)),this.data.array[e*this.data.stride+this.offset+t]=i,this}setX(e,t){return this.normalized&&(t=Ut(t,this.array)),this.data.array[e*this.data.stride+this.offset]=t,this}setY(e,t){return this.normalized&&(t=Ut(t,this.array)),this.data.array[e*this.data.stride+this.offset+1]=t,this}setZ(e,t){return this.normalized&&(t=Ut(t,this.array)),this.data.array[e*this.data.stride+this.offset+2]=t,this}setW(e,t){return this.normalized&&(t=Ut(t,this.array)),this.data.array[e*this.data.stride+this.offset+3]=t,this}getX(e){let t=this.data.array[e*this.data.stride+this.offset];return this.normalized&&(t=jn(t,this.array)),t}getY(e){let t=this.data.array[e*this.data.stride+this.offset+1];return this.normalized&&(t=jn(t,this.array)),t}getZ(e){let t=this.data.array[e*this.data.stride+this.offset+2];return this.normalized&&(t=jn(t,this.array)),t}getW(e){let t=this.data.array[e*this.data.stride+this.offset+3];return this.normalized&&(t=jn(t,this.array)),t}setXY(e,t,i){return e=e*this.data.stride+this.offset,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this}setXYZ(e,t,i,r){return e=e*this.data.stride+this.offset,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array),r=Ut(r,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this.data.array[e+2]=r,this}setXYZW(e,t,i,r,s){return e=e*this.data.stride+this.offset,this.normalized&&(t=Ut(t,this.array),i=Ut(i,this.array),r=Ut(r,this.array),s=Ut(s,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this.data.array[e+2]=r,this.data.array[e+3]=s,this}clone(e){if(e===void 0){la("InterleavedBufferAttribute.clone(): Cloning an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let i=0;i<this.count;i++){const r=i*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return new Wn(new this.array.constructor(t),this.itemSize,this.normalized)}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.clone(e)),new _l(e.interleavedBuffers[this.data.uuid],this.itemSize,this.offset,this.normalized)}toJSON(e){if(e===void 0){la("InterleavedBufferAttribute.toJSON(): Serializing an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let i=0;i<this.count;i++){const r=i*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return{itemSize:this.itemSize,type:this.array.constructor.name,array:t,normalized:this.normalized}}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.toJSON(e)),{isInterleavedBufferAttribute:!0,itemSize:this.itemSize,data:this.data.uuid,offset:this.offset,normalized:this.normalized}}}let Qf=0;class fa extends dr{constructor(){super(),this.isMaterial=!0,Object.defineProperty(this,"id",{value:Qf++}),this.uuid=zi(),this.name="",this.type="Material",this.blending=Ur,this.side=Gi,this.vertexColors=!1,this.opacity=1,this.transparent=!1,this.alphaHash=!1,this.blendSrc=oo,this.blendDst=cs,this.blendEquation=Bi,this.blendSrcAlpha=null,this.blendDstAlpha=null,this.blendEquationAlpha=null,this.blendColor=new It(0,0,0),this.blendAlpha=0,this.depthFunc=kr,this.depthTest=!0,this.depthWrite=!0,this.stencilWriteMask=255,this.stencilFunc=Vl,this.stencilRef=0,this.stencilFuncMask=255,this.stencilFail=_r,this.stencilZFail=_r,this.stencilZPass=_r,this.stencilWrite=!1,this.clippingPlanes=null,this.clipIntersection=!1,this.clipShadows=!1,this.shadowSide=null,this.colorWrite=!0,this.precision=null,this.polygonOffset=!1,this.polygonOffsetFactor=0,this.polygonOffsetUnits=0,this.dithering=!1,this.alphaToCoverage=!1,this.premultipliedAlpha=!1,this.forceSinglePass=!1,this.allowOverride=!0,this.visible=!0,this.toneMapped=!0,this.userData={},this.version=0,this._alphaTest=0}get alphaTest(){return this._alphaTest}set alphaTest(e){this._alphaTest>0!=e>0&&this.version++,this._alphaTest=e}onBeforeRender(){}onBeforeCompile(){}customProgramCacheKey(){return this.onBeforeCompile.toString()}setValues(e){if(e!==void 0)for(const t in e){const i=e[t];if(i===void 0){st(`Material: parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){st(`Material: '${t}' is not a property of THREE.${this.type}.`);continue}r&&r.isColor?r.set(i):r&&r.isVector2&&i&&i.isVector2||r&&r.isEuler&&i&&i.isEuler||r&&r.isVector3&&i&&i.isVector3?r.copy(i):this[t]=i}}toJSON(e){const t=e===void 0||typeof e=="string";t&&(e={textures:{},images:{}});const i={metadata:{version:4.7,type:"Material",generator:"Material.toJSON"}};i.uuid=this.uuid,i.type=this.type,this.name!==""&&(i.name=this.name),this.color&&this.color.isColor&&(i.color=this.color.getHex()),this.roughness!==void 0&&(i.roughness=this.roughness),this.metalness!==void 0&&(i.metalness=this.metalness),this.sheen!==void 0&&(i.sheen=this.sheen),this.sheenColor&&this.sheenColor.isColor&&(i.sheenColor=this.sheenColor.getHex()),this.sheenRoughness!==void 0&&(i.sheenRoughness=this.sheenRoughness),this.emissive&&this.emissive.isColor&&(i.emissive=this.emissive.getHex()),this.emissiveIntensity!==void 0&&this.emissiveIntensity!==1&&(i.emissiveIntensity=this.emissiveIntensity),this.specular&&this.specular.isColor&&(i.specular=this.specular.getHex()),this.specularIntensity!==void 0&&(i.specularIntensity=this.specularIntensity),this.specularColor&&this.specularColor.isColor&&(i.specularColor=this.specularColor.getHex()),this.shininess!==void 0&&(i.shininess=this.shininess),this.clearcoat!==void 0&&(i.clearcoat=this.clearcoat),this.clearcoatRoughness!==void 0&&(i.clearcoatRoughness=this.clearcoatRoughness),this.clearcoatMap&&this.clearcoatMap.isTexture&&(i.clearcoatMap=this.clearcoatMap.toJSON(e).uuid),this.clearcoatRoughnessMap&&this.clearcoatRoughnessMap.isTexture&&(i.clearcoatRoughnessMap=this.clearcoatRoughnessMap.toJSON(e).uuid),this.clearcoatNormalMap&&this.clearcoatNormalMap.isTexture&&(i.clearcoatNormalMap=this.clearcoatNormalMap.toJSON(e).uuid,i.clearcoatNormalScale=this.clearcoatNormalScale.toArray()),this.sheenColorMap&&this.sheenColorMap.isTexture&&(i.sheenColorMap=this.sheenColorMap.toJSON(e).uuid),this.sheenRoughnessMap&&this.sheenRoughnessMap.isTexture&&(i.sheenRoughnessMap=this.sheenRoughnessMap.toJSON(e).uuid),this.dispersion!==void 0&&(i.dispersion=this.dispersion),this.iridescence!==void 0&&(i.iridescence=this.iridescence),this.iridescenceIOR!==void 0&&(i.iridescenceIOR=this.iridescenceIOR),this.iridescenceThicknessRange!==void 0&&(i.iridescenceThicknessRange=this.iridescenceThicknessRange),this.iridescenceMap&&this.iridescenceMap.isTexture&&(i.iridescenceMap=this.iridescenceMap.toJSON(e).uuid),this.iridescenceThicknessMap&&this.iridescenceThicknessMap.isTexture&&(i.iridescenceThicknessMap=this.iridescenceThicknessMap.toJSON(e).uuid),this.anisotropy!==void 0&&(i.anisotropy=this.anisotropy),this.anisotropyRotation!==void 0&&(i.anisotropyRotation=this.anisotropyRotation),this.anisotropyMap&&this.anisotropyMap.isTexture&&(i.anisotropyMap=this.anisotropyMap.toJSON(e).uuid),this.map&&this.map.isTexture&&(i.map=this.map.toJSON(e).uuid),this.matcap&&this.matcap.isTexture&&(i.matcap=this.matcap.toJSON(e).uuid),this.alphaMap&&this.alphaMap.isTexture&&(i.alphaMap=this.alphaMap.toJSON(e).uuid),this.lightMap&&this.lightMap.isTexture&&(i.lightMap=this.lightMap.toJSON(e).uuid,i.lightMapIntensity=this.lightMapIntensity),this.aoMap&&this.aoMap.isTexture&&(i.aoMap=this.aoMap.toJSON(e).uuid,i.aoMapIntensity=this.aoMapIntensity),this.bumpMap&&this.bumpMap.isTexture&&(i.bumpMap=this.bumpMap.toJSON(e).uuid,i.bumpScale=this.bumpScale),this.normalMap&&this.normalMap.isTexture&&(i.normalMap=this.normalMap.toJSON(e).uuid,i.normalMapType=this.normalMapType,i.normalScale=this.normalScale.toArray()),this.displacementMap&&this.displacementMap.isTexture&&(i.displacementMap=this.displacementMap.toJSON(e).uuid,i.displacementScale=this.displacementScale,i.displacementBias=this.displacementBias),this.roughnessMap&&this.roughnessMap.isTexture&&(i.roughnessMap=this.roughnessMap.toJSON(e).uuid),this.metalnessMap&&this.metalnessMap.isTexture&&(i.metalnessMap=this.metalnessMap.toJSON(e).uuid),this.emissiveMap&&this.emissiveMap.isTexture&&(i.emissiveMap=this.emissiveMap.toJSON(e).uuid),this.specularMap&&this.specularMap.isTexture&&(i.specularMap=this.specularMap.toJSON(e).uuid),this.specularIntensityMap&&this.specularIntensityMap.isTexture&&(i.specularIntensityMap=this.specularIntensityMap.toJSON(e).uuid),this.specularColorMap&&this.specularColorMap.isTexture&&(i.specularColorMap=this.specularColorMap.toJSON(e).uuid),this.envMap&&this.envMap.isTexture&&(i.envMap=this.envMap.toJSON(e).uuid,this.combine!==void 0&&(i.combine=this.combine)),this.envMapRotation!==void 0&&(i.envMapRotation=this.envMapRotation.toArray()),this.envMapIntensity!==void 0&&(i.envMapIntensity=this.envMapIntensity),this.reflectivity!==void 0&&(i.reflectivity=this.reflectivity),this.refractionRatio!==void 0&&(i.refractionRatio=this.refractionRatio),this.gradientMap&&this.gradientMap.isTexture&&(i.gradientMap=this.gradientMap.toJSON(e).uuid),this.transmission!==void 0&&(i.transmission=this.transmission),this.transmissionMap&&this.transmissionMap.isTexture&&(i.transmissionMap=this.transmissionMap.toJSON(e).uuid),this.thickness!==void 0&&(i.thickness=this.thickness),this.thicknessMap&&this.thicknessMap.isTexture&&(i.thicknessMap=this.thicknessMap.toJSON(e).uuid),this.attenuationDistance!==void 0&&this.attenuationDistance!==1/0&&(i.attenuationDistance=this.attenuationDistance),this.attenuationColor!==void 0&&(i.attenuationColor=this.attenuationColor.getHex()),this.size!==void 0&&(i.size=this.size),this.shadowSide!==null&&(i.shadowSide=this.shadowSide),this.sizeAttenuation!==void 0&&(i.sizeAttenuation=this.sizeAttenuation),this.blending!==Ur&&(i.blending=this.blending),this.side!==Gi&&(i.side=this.side),this.vertexColors===!0&&(i.vertexColors=!0),this.opacity<1&&(i.opacity=this.opacity),this.transparent===!0&&(i.transparent=!0),this.blendSrc!==oo&&(i.blendSrc=this.blendSrc),this.blendDst!==cs&&(i.blendDst=this.blendDst),this.blendEquation!==Bi&&(i.blendEquation=this.blendEquation),this.blendSrcAlpha!==null&&(i.blendSrcAlpha=this.blendSrcAlpha),this.blendDstAlpha!==null&&(i.blendDstAlpha=this.blendDstAlpha),this.blendEquationAlpha!==null&&(i.blendEquationAlpha=this.blendEquationAlpha),this.blendColor&&this.blendColor.isColor&&(i.blendColor=this.blendColor.getHex()),this.blendAlpha!==0&&(i.blendAlpha=this.blendAlpha),this.depthFunc!==kr&&(i.depthFunc=this.depthFunc),this.depthTest===!1&&(i.depthTest=this.depthTest),this.depthWrite===!1&&(i.depthWrite=this.depthWrite),this.colorWrite===!1&&(i.colorWrite=this.colorWrite),this.stencilWriteMask!==255&&(i.stencilWriteMask=this.stencilWriteMask),this.stencilFunc!==Vl&&(i.stencilFunc=this.stencilFunc),this.stencilRef!==0&&(i.stencilRef=this.stencilRef),this.stencilFuncMask!==255&&(i.stencilFuncMask=this.stencilFuncMask),this.stencilFail!==_r&&(i.stencilFail=this.stencilFail),this.stencilZFail!==_r&&(i.stencilZFail=this.stencilZFail),this.stencilZPass!==_r&&(i.stencilZPass=this.stencilZPass),this.stencilWrite===!0&&(i.stencilWrite=this.stencilWrite),this.rotation!==void 0&&this.rotation!==0&&(i.rotation=this.rotation),this.polygonOffset===!0&&(i.polygonOffset=!0),this.polygonOffsetFactor!==0&&(i.polygonOffsetFactor=this.polygonOffsetFactor),this.polygonOffsetUnits!==0&&(i.polygonOffsetUnits=this.polygonOffsetUnits),this.linewidth!==void 0&&this.linewidth!==1&&(i.linewidth=this.linewidth),this.dashSize!==void 0&&(i.dashSize=this.dashSize),this.gapSize!==void 0&&(i.gapSize=this.gapSize),this.scale!==void 0&&(i.scale=this.scale),this.dithering===!0&&(i.dithering=!0),this.alphaTest>0&&(i.alphaTest=this.alphaTest),this.alphaHash===!0&&(i.alphaHash=!0),this.alphaToCoverage===!0&&(i.alphaToCoverage=!0),this.premultipliedAlpha===!0&&(i.premultipliedAlpha=!0),this.forceSinglePass===!0&&(i.forceSinglePass=!0),this.allowOverride===!1&&(i.allowOverride=!1),this.wireframe===!0&&(i.wireframe=!0),this.wireframeLinewidth>1&&(i.wireframeLinewidth=this.wireframeLinewidth),this.wireframeLinecap!=="round"&&(i.wireframeLinecap=this.wireframeLinecap),this.wireframeLinejoin!=="round"&&(i.wireframeLinejoin=this.wireframeLinejoin),this.flatShading===!0&&(i.flatShading=!0),this.visible===!1&&(i.visible=!1),this.toneMapped===!1&&(i.toneMapped=!1),this.fog===!1&&(i.fog=!1),Object.keys(this.userData).length>0&&(i.userData=this.userData);function r(s){const a=[];for(const o in s){const l=s[o];delete l.metadata,a.push(l)}return a}if(t){const s=r(e.textures),a=r(e.images);s.length>0&&(i.textures=s),a.length>0&&(i.images=a)}return i}fromJSON(e,t){if(e.uuid!==void 0&&(this.uuid=e.uuid),e.name!==void 0&&(this.name=e.name),e.color!==void 0&&this.color!==void 0&&this.color.setHex(e.color),e.roughness!==void 0&&(this.roughness=e.roughness),e.metalness!==void 0&&(this.metalness=e.metalness),e.sheen!==void 0&&(this.sheen=e.sheen),e.sheenColor!==void 0&&(this.sheenColor=new It().setHex(e.sheenColor)),e.sheenRoughness!==void 0&&(this.sheenRoughness=e.sheenRoughness),e.emissive!==void 0&&this.emissive!==void 0&&this.emissive.setHex(e.emissive),e.specular!==void 0&&this.specular!==void 0&&this.specular.setHex(e.specular),e.specularIntensity!==void 0&&(this.specularIntensity=e.specularIntensity),e.specularColor!==void 0&&this.specularColor!==void 0&&this.specularColor.setHex(e.specularColor),e.shininess!==void 0&&(this.shininess=e.shininess),e.clearcoat!==void 0&&(this.clearcoat=e.clearcoat),e.clearcoatRoughness!==void 0&&(this.clearcoatRoughness=e.clearcoatRoughness),e.dispersion!==void 0&&(this.dispersion=e.dispersion),e.iridescence!==void 0&&(this.iridescence=e.iridescence),e.iridescenceIOR!==void 0&&(this.iridescenceIOR=e.iridescenceIOR),e.iridescenceThicknessRange!==void 0&&(this.iridescenceThicknessRange=e.iridescenceThicknessRange),e.transmission!==void 0&&(this.transmission=e.transmission),e.thickness!==void 0&&(this.thickness=e.thickness),e.attenuationDistance!==void 0&&(this.attenuationDistance=e.attenuationDistance),e.attenuationColor!==void 0&&this.attenuationColor!==void 0&&this.attenuationColor.setHex(e.attenuationColor),e.anisotropy!==void 0&&(this.anisotropy=e.anisotropy),e.anisotropyRotation!==void 0&&(this.anisotropyRotation=e.anisotropyRotation),e.fog!==void 0&&(this.fog=e.fog),e.flatShading!==void 0&&(this.flatShading=e.flatShading),e.blending!==void 0&&(this.blending=e.blending),e.combine!==void 0&&(this.combine=e.combine),e.side!==void 0&&(this.side=e.side),e.shadowSide!==void 0&&(this.shadowSide=e.shadowSide),e.opacity!==void 0&&(this.opacity=e.opacity),e.transparent!==void 0&&(this.transparent=e.transparent),e.alphaTest!==void 0&&(this.alphaTest=e.alphaTest),e.alphaHash!==void 0&&(this.alphaHash=e.alphaHash),e.depthFunc!==void 0&&(this.depthFunc=e.depthFunc),e.depthTest!==void 0&&(this.depthTest=e.depthTest),e.depthWrite!==void 0&&(this.depthWrite=e.depthWrite),e.colorWrite!==void 0&&(this.colorWrite=e.colorWrite),e.blendSrc!==void 0&&(this.blendSrc=e.blendSrc),e.blendDst!==void 0&&(this.blendDst=e.blendDst),e.blendEquation!==void 0&&(this.blendEquation=e.blendEquation),e.blendSrcAlpha!==void 0&&(this.blendSrcAlpha=e.blendSrcAlpha),e.blendDstAlpha!==void 0&&(this.blendDstAlpha=e.blendDstAlpha),e.blendEquationAlpha!==void 0&&(this.blendEquationAlpha=e.blendEquationAlpha),e.blendColor!==void 0&&this.blendColor!==void 0&&this.blendColor.setHex(e.blendColor),e.blendAlpha!==void 0&&(this.blendAlpha=e.blendAlpha),e.stencilWriteMask!==void 0&&(this.stencilWriteMask=e.stencilWriteMask),e.stencilFunc!==void 0&&(this.stencilFunc=e.stencilFunc),e.stencilRef!==void 0&&(this.stencilRef=e.stencilRef),e.stencilFuncMask!==void 0&&(this.stencilFuncMask=e.stencilFuncMask),e.stencilFail!==void 0&&(this.stencilFail=e.stencilFail),e.stencilZFail!==void 0&&(this.stencilZFail=e.stencilZFail),e.stencilZPass!==void 0&&(this.stencilZPass=e.stencilZPass),e.stencilWrite!==void 0&&(this.stencilWrite=e.stencilWrite),e.wireframe!==void 0&&(this.wireframe=e.wireframe),e.wireframeLinewidth!==void 0&&(this.wireframeLinewidth=e.wireframeLinewidth),e.wireframeLinecap!==void 0&&(this.wireframeLinecap=e.wireframeLinecap),e.wireframeLinejoin!==void 0&&(this.wireframeLinejoin=e.wireframeLinejoin),e.rotation!==void 0&&(this.rotation=e.rotation),e.linewidth!==void 0&&(this.linewidth=e.linewidth),e.dashSize!==void 0&&(this.dashSize=e.dashSize),e.gapSize!==void 0&&(this.gapSize=e.gapSize),e.scale!==void 0&&(this.scale=e.scale),e.polygonOffset!==void 0&&(this.polygonOffset=e.polygonOffset),e.polygonOffsetFactor!==void 0&&(this.polygonOffsetFactor=e.polygonOffsetFactor),e.polygonOffsetUnits!==void 0&&(this.polygonOffsetUnits=e.polygonOffsetUnits),e.dithering!==void 0&&(this.dithering=e.dithering),e.alphaToCoverage!==void 0&&(this.alphaToCoverage=e.alphaToCoverage),e.premultipliedAlpha!==void 0&&(this.premultipliedAlpha=e.premultipliedAlpha),e.forceSinglePass!==void 0&&(this.forceSinglePass=e.forceSinglePass),e.allowOverride!==void 0&&(this.allowOverride=e.allowOverride),e.visible!==void 0&&(this.visible=e.visible),e.toneMapped!==void 0&&(this.toneMapped=e.toneMapped),e.userData!==void 0&&(this.userData=e.userData),e.vertexColors!==void 0&&(typeof e.vertexColors=="number"?this.vertexColors=e.vertexColors>0:this.vertexColors=e.vertexColors),e.size!==void 0&&(this.size=e.size),e.sizeAttenuation!==void 0&&(this.sizeAttenuation=e.sizeAttenuation),e.map!==void 0&&(this.map=t[e.map]||null),e.matcap!==void 0&&(this.matcap=t[e.matcap]||null),e.alphaMap!==void 0&&(this.alphaMap=t[e.alphaMap]||null),e.bumpMap!==void 0&&(this.bumpMap=t[e.bumpMap]||null),e.bumpScale!==void 0&&(this.bumpScale=e.bumpScale),e.normalMap!==void 0&&(this.normalMap=t[e.normalMap]||null),e.normalMapType!==void 0&&(this.normalMapType=e.normalMapType),e.normalScale!==void 0){let i=e.normalScale;Array.isArray(i)===!1&&(i=[i,i]),this.normalScale=new At().fromArray(i)}return e.displacementMap!==void 0&&(this.displacementMap=t[e.displacementMap]||null),e.displacementScale!==void 0&&(this.displacementScale=e.displacementScale),e.displacementBias!==void 0&&(this.displacementBias=e.displacementBias),e.roughnessMap!==void 0&&(this.roughnessMap=t[e.roughnessMap]||null),e.metalnessMap!==void 0&&(this.metalnessMap=t[e.metalnessMap]||null),e.emissiveMap!==void 0&&(this.emissiveMap=t[e.emissiveMap]||null),e.emissiveIntensity!==void 0&&(this.emissiveIntensity=e.emissiveIntensity),e.specularMap!==void 0&&(this.specularMap=t[e.specularMap]||null),e.specularIntensityMap!==void 0&&(this.specularIntensityMap=t[e.specularIntensityMap]||null),e.specularColorMap!==void 0&&(this.specularColorMap=t[e.specularColorMap]||null),e.envMap!==void 0&&(this.envMap=t[e.envMap]||null),e.envMapRotation!==void 0&&this.envMapRotation.fromArray(e.envMapRotation),e.envMapIntensity!==void 0&&(this.envMapIntensity=e.envMapIntensity),e.reflectivity!==void 0&&(this.reflectivity=e.reflectivity),e.refractionRatio!==void 0&&(this.refractionRatio=e.refractionRatio),e.lightMap!==void 0&&(this.lightMap=t[e.lightMap]||null),e.lightMapIntensity!==void 0&&(this.lightMapIntensity=e.lightMapIntensity),e.aoMap!==void 0&&(this.aoMap=t[e.aoMap]||null),e.aoMapIntensity!==void 0&&(this.aoMapIntensity=e.aoMapIntensity),e.gradientMap!==void 0&&(this.gradientMap=t[e.gradientMap]||null),e.clearcoatMap!==void 0&&(this.clearcoatMap=t[e.clearcoatMap]||null),e.clearcoatRoughnessMap!==void 0&&(this.clearcoatRoughnessMap=t[e.clearcoatRoughnessMap]||null),e.clearcoatNormalMap!==void 0&&(this.clearcoatNormalMap=t[e.clearcoatNormalMap]||null),e.clearcoatNormalScale!==void 0&&(this.clearcoatNormalScale=new At().fromArray(e.clearcoatNormalScale)),e.iridescenceMap!==void 0&&(this.iridescenceMap=t[e.iridescenceMap]||null),e.iridescenceThicknessMap!==void 0&&(this.iridescenceThicknessMap=t[e.iridescenceThicknessMap]||null),e.transmissionMap!==void 0&&(this.transmissionMap=t[e.transmissionMap]||null),e.thicknessMap!==void 0&&(this.thicknessMap=t[e.thicknessMap]||null),e.anisotropyMap!==void 0&&(this.anisotropyMap=t[e.anisotropyMap]||null),e.sheenColorMap!==void 0&&(this.sheenColorMap=t[e.sheenColorMap]||null),e.sheenRoughnessMap!==void 0&&(this.sheenRoughnessMap=t[e.sheenRoughnessMap]||null),this}clone(){return new this.constructor().copy(this)}copy(e){this.name=e.name,this.blending=e.blending,this.side=e.side,this.vertexColors=e.vertexColors,this.opacity=e.opacity,this.transparent=e.transparent,this.blendSrc=e.blendSrc,this.blendDst=e.blendDst,this.blendEquation=e.blendEquation,this.blendSrcAlpha=e.blendSrcAlpha,this.blendDstAlpha=e.blendDstAlpha,this.blendEquationAlpha=e.blendEquationAlpha,this.blendColor.copy(e.blendColor),this.blendAlpha=e.blendAlpha,this.depthFunc=e.depthFunc,this.depthTest=e.depthTest,this.depthWrite=e.depthWrite,this.stencilWriteMask=e.stencilWriteMask,this.stencilFunc=e.stencilFunc,this.stencilRef=e.stencilRef,this.stencilFuncMask=e.stencilFuncMask,this.stencilFail=e.stencilFail,this.stencilZFail=e.stencilZFail,this.stencilZPass=e.stencilZPass,this.stencilWrite=e.stencilWrite;const t=e.clippingPlanes;let i=null;if(t!==null){const r=t.length;i=new Array(r);for(let s=0;s!==r;++s)i[s]=t[s].clone()}return this.clippingPlanes=i,this.clipIntersection=e.clipIntersection,this.clipShadows=e.clipShadows,this.shadowSide=e.shadowSide,this.colorWrite=e.colorWrite,this.precision=e.precision,this.polygonOffset=e.polygonOffset,this.polygonOffsetFactor=e.polygonOffsetFactor,this.polygonOffsetUnits=e.polygonOffsetUnits,this.dithering=e.dithering,this.alphaTest=e.alphaTest,this.alphaHash=e.alphaHash,this.alphaToCoverage=e.alphaToCoverage,this.premultipliedAlpha=e.premultipliedAlpha,this.forceSinglePass=e.forceSinglePass,this.allowOverride=e.allowOverride,this.visible=e.visible,this.toneMapped=e.toneMapped,this.userData=JSON.parse(JSON.stringify(e.userData)),this}dispose(){this.dispatchEvent({type:"dispose"})}set needsUpdate(e){e===!0&&this.version++}}const pi=new le,Va=new le,Ls=new le,Ui=new le,Wa=new le,Is=new le,Xa=new le;class jf{constructor(e=new le,t=new le(0,0,-1)){this.origin=e,this.direction=t}set(e,t){return this.origin.copy(e),this.direction.copy(t),this}copy(e){return this.origin.copy(e.origin),this.direction.copy(e.direction),this}at(e,t){return t.copy(this.origin).addScaledVector(this.direction,e)}lookAt(e){return this.direction.copy(e).sub(this.origin).normalize(),this}recast(e){return this.origin.copy(this.at(e,pi)),this}closestPointToPoint(e,t){t.subVectors(e,this.origin);const i=t.dot(this.direction);return i<0?t.copy(this.origin):t.copy(this.origin).addScaledVector(this.direction,i)}distanceToPoint(e){return Math.sqrt(this.distanceSqToPoint(e))}distanceSqToPoint(e){const t=pi.subVectors(e,this.origin).dot(this.direction);return t<0?this.origin.distanceToSquared(e):(pi.copy(this.origin).addScaledVector(this.direction,t),pi.distanceToSquared(e))}distanceSqToSegment(e,t,i,r){Va.copy(e).add(t).multiplyScalar(.5),Ls.copy(t).sub(e).normalize(),Ui.copy(this.origin).sub(Va);const s=e.distanceTo(t)*.5,a=-this.direction.dot(Ls),o=Ui.dot(this.direction),l=-Ui.dot(Ls),c=Ui.lengthSq(),f=Math.abs(1-a*a);let u,d,h,_;if(f>0)if(u=a*l-o,d=a*o-l,_=s*f,u>=0)if(d>=-_)if(d<=_){const x=1/f;u*=x,d*=x,h=u*(u+a*d+2*o)+d*(a*u+d+2*l)+c}else d=s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;else d=-s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;else d<=-_?(u=Math.max(0,-(-a*s+o)),d=u>0?-s:Math.min(Math.max(-s,-l),s),h=-u*u+d*(d+2*l)+c):d<=_?(u=0,d=Math.min(Math.max(-s,-l),s),h=d*(d+2*l)+c):(u=Math.max(0,-(a*s+o)),d=u>0?s:Math.min(Math.max(-s,-l),s),h=-u*u+d*(d+2*l)+c);else d=a>0?-s:s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;return i&&i.copy(this.origin).addScaledVector(this.direction,u),r&&r.copy(Va).addScaledVector(Ls,d),h}intersectSphere(e,t){pi.subVectors(e.center,this.origin);const i=pi.dot(this.direction),r=pi.dot(pi)-i*i,s=e.radius*e.radius;if(r>s)return null;const a=Math.sqrt(s-r),o=i-a,l=i+a;return l<0?null:o<0?this.at(l,t):this.at(o,t)}intersectsSphere(e){return e.radius<0?!1:this.distanceSqToPoint(e.center)<=e.radius*e.radius}distanceToPlane(e){const t=e.normal.dot(this.direction);if(t===0)return e.distanceToPoint(this.origin)===0?0:null;const i=-(this.origin.dot(e.normal)+e.constant)/t;return i>=0?i:null}intersectPlane(e,t){const i=this.distanceToPlane(e);return i===null?null:this.at(i,t)}intersectsPlane(e){const t=e.distanceToPoint(this.origin);return t===0||e.normal.dot(this.direction)*t<0}intersectBox(e,t){let i,r,s,a,o,l;const c=1/this.direction.x,f=1/this.direction.y,u=1/this.direction.z,d=this.origin;return c>=0?(i=(e.min.x-d.x)*c,r=(e.max.x-d.x)*c):(i=(e.max.x-d.x)*c,r=(e.min.x-d.x)*c),f>=0?(s=(e.min.y-d.y)*f,a=(e.max.y-d.y)*f):(s=(e.max.y-d.y)*f,a=(e.min.y-d.y)*f),i>a||s>r||((s>i||isNaN(i))&&(i=s),(a<r||isNaN(r))&&(r=a),u>=0?(o=(e.min.z-d.z)*u,l=(e.max.z-d.z)*u):(o=(e.max.z-d.z)*u,l=(e.min.z-d.z)*u),i>l||o>r)||((o>i||i!==i)&&(i=o),(l<r||r!==r)&&(r=l),r<0)?null:this.at(i>=0?i:r,t)}intersectsBox(e){return this.intersectBox(e,pi)!==null}intersectTriangle(e,t,i,r,s){Wa.subVectors(t,e),Is.subVectors(i,e),Xa.crossVectors(Wa,Is);let a=this.direction.dot(Xa),o;if(a>0){if(r)return null;o=1}else if(a<0)o=-1,a=-a;else return null;Ui.subVectors(this.origin,e);const l=o*this.direction.dot(Is.crossVectors(Ui,Is));if(l<0)return null;const c=o*this.direction.dot(Wa.cross(Ui));if(c<0||l+c>a)return null;const f=-o*Ui.dot(Xa);return f<0?null:this.at(f/a,s)}applyMatrix4(e){return this.origin.applyMatrix4(e),this.direction.transformDirection(e),this}equals(e){return e.origin.equals(this.origin)&&e.direction.equals(this.direction)}clone(){return new this.constructor().copy(this)}}class Pd extends fa{constructor(e){super(),this.isMeshBasicMaterial=!0,this.type="MeshBasicMaterial",this.color=new It(16777215),this.map=null,this.lightMap=null,this.lightMapIntensity=1,this.aoMap=null,this.aoMapIntensity=1,this.specularMap=null,this.alphaMap=null,this.envMap=null,this.envMapRotation=new lr,this.combine=ld,this.reflectivity=1,this.refractionRatio=.98,this.wireframe=!1,this.wireframeLinewidth=1,this.wireframeLinecap="round",this.wireframeLinejoin="round",this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.lightMap=e.lightMap,this.lightMapIntensity=e.lightMapIntensity,this.aoMap=e.aoMap,this.aoMapIntensity=e.aoMapIntensity,this.specularMap=e.specularMap,this.alphaMap=e.alphaMap,this.envMap=e.envMap,this.envMapRotation.copy(e.envMapRotation),this.combine=e.combine,this.reflectivity=e.reflectivity,this.refractionRatio=e.refractionRatio,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.wireframeLinecap=e.wireframeLinecap,this.wireframeLinejoin=e.wireframeLinejoin,this.fog=e.fog,this}}const nc=new Zt,Zi=new jf,Ds=new gl,ic=new le,Ns=new le,Us=new le,Fs=new le,qa=new le,Os=new le,rc=new le,Bs=new le;class ri extends Tn{constructor(e=new ai,t=new Pd){super(),this.isMesh=!0,this.type="Mesh",this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.count=1,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),e.morphTargetInfluences!==void 0&&(this.morphTargetInfluences=e.morphTargetInfluences.slice()),e.morphTargetDictionary!==void 0&&(this.morphTargetDictionary=Object.assign({},e.morphTargetDictionary)),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}updateMorphTargets(){const t=this.geometry.morphAttributes,i=Object.keys(t);if(i.length>0){const r=t[i[0]];if(r!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let s=0,a=r.length;s<a;s++){const o=r[s].name||String(s);this.morphTargetInfluences.push(0),this.morphTargetDictionary[o]=s}}}}getVertexPosition(e,t){const i=this.geometry,r=i.attributes.position,s=i.morphAttributes.position,a=i.morphTargetsRelative;t.fromBufferAttribute(r,e);const o=this.morphTargetInfluences;if(s&&o){Os.set(0,0,0);for(let l=0,c=s.length;l<c;l++){const f=o[l],u=s[l];f!==0&&(qa.fromBufferAttribute(u,e),a?Os.addScaledVector(qa,f):Os.addScaledVector(qa.sub(t),f))}t.add(Os)}return t}raycast(e,t){const i=this.geometry,r=this.material,s=this.matrixWorld;r!==void 0&&(i.boundingSphere===null&&i.computeBoundingSphere(),Ds.copy(i.boundingSphere),Ds.applyMatrix4(s),Zi.copy(e.ray).recast(e.near),!(Ds.containsPoint(Zi.origin)===!1&&(Zi.intersectSphere(Ds,ic)===null||Zi.origin.distanceToSquared(ic)>(e.far-e.near)**2))&&(nc.copy(s).invert(),Zi.copy(e.ray).applyMatrix4(nc),!(i.boundingBox!==null&&Zi.intersectsBox(i.boundingBox)===!1)&&this._computeIntersections(e,t,Zi)))}_computeIntersections(e,t,i){let r;const s=this.geometry,a=this.material,o=s.index,l=s.attributes.position,c=s.attributes.uv,f=s.attributes.uv1,u=s.attributes.normal,d=s.groups,h=s.drawRange;if(o!==null)if(Array.isArray(a))for(let _=0,x=d.length;_<x;_++){const m=d[_],p=a[m.materialIndex],C=Math.max(m.start,h.start),D=Math.min(o.count,Math.min(m.start+m.count,h.start+h.count));for(let M=C,L=D;M<L;M+=3){const A=o.getX(M),P=o.getX(M+1),v=o.getX(M+2);r=ks(this,p,e,i,c,f,u,A,P,v),r&&(r.faceIndex=Math.floor(M/3),r.face.materialIndex=m.materialIndex,t.push(r))}}else{const _=Math.max(0,h.start),x=Math.min(o.count,h.start+h.count);for(let m=_,p=x;m<p;m+=3){const C=o.getX(m),D=o.getX(m+1),M=o.getX(m+2);r=ks(this,a,e,i,c,f,u,C,D,M),r&&(r.faceIndex=Math.floor(m/3),t.push(r))}}else if(l!==void 0)if(Array.isArray(a))for(let _=0,x=d.length;_<x;_++){const m=d[_],p=a[m.materialIndex],C=Math.max(m.start,h.start),D=Math.min(l.count,Math.min(m.start+m.count,h.start+h.count));for(let M=C,L=D;M<L;M+=3){const A=M,P=M+1,v=M+2;r=ks(this,p,e,i,c,f,u,A,P,v),r&&(r.faceIndex=Math.floor(M/3),r.face.materialIndex=m.materialIndex,t.push(r))}}else{const _=Math.max(0,h.start),x=Math.min(l.count,h.start+h.count);for(let m=_,p=x;m<p;m+=3){const C=m,D=m+1,M=m+2;r=ks(this,a,e,i,c,f,u,C,D,M),r&&(r.faceIndex=Math.floor(m/3),t.push(r))}}}}function eh(n,e,t,i,r,s,a,o){let l;if(e.side===vn?l=i.intersectTriangle(a,s,r,!0,o):l=i.intersectTriangle(r,s,a,e.side===Gi,o),l===null)return null;Bs.copy(o),Bs.applyMatrix4(n.matrixWorld);const c=t.ray.origin.distanceTo(Bs);return c<t.near||c>t.far?null:{distance:c,point:Bs.clone(),object:n}}function ks(n,e,t,i,r,s,a,o,l,c){n.getVertexPosition(o,Ns),n.getVertexPosition(l,Us),n.getVertexPosition(c,Fs);const f=eh(n,e,t,i,Ns,Us,Fs,rc);if(f){const u=new le;Gn.getBarycoord(rc,Ns,Us,Fs,u),r&&(f.uv=Gn.getInterpolatedAttribute(r,o,l,c,u,new At)),s&&(f.uv1=Gn.getInterpolatedAttribute(s,o,l,c,u,new At)),a&&(f.normal=Gn.getInterpolatedAttribute(a,o,l,c,u,new le),f.normal.dot(i.direction)>0&&f.normal.multiplyScalar(-1));const d={a:o,b:l,c,normal:new le,materialIndex:0};Gn.getNormal(Ns,Us,Fs,d.normal),f.face=d,f.barycoord=u}return f}class th extends un{constructor(e=null,t=1,i=1,r,s,a,o,l,c=on,f=on,u,d){super(null,a,o,l,c,f,r,s,u,d),this.isDataTexture=!0,this.image={data:e,width:t,height:i},this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const Ya=new le,nh=new le,ih=new ct;class er{constructor(e=new le(1,0,0),t=0){this.isPlane=!0,this.normal=e,this.constant=t}set(e,t){return this.normal.copy(e),this.constant=t,this}setComponents(e,t,i,r){return this.normal.set(e,t,i),this.constant=r,this}setFromNormalAndCoplanarPoint(e,t){return this.normal.copy(e),this.constant=-t.dot(this.normal),this}setFromCoplanarPoints(e,t,i){const r=Ya.subVectors(i,t).cross(nh.subVectors(e,t)).normalize();return this.setFromNormalAndCoplanarPoint(r,e),this}copy(e){return this.normal.copy(e.normal),this.constant=e.constant,this}normalize(){const e=1/this.normal.length();return this.normal.multiplyScalar(e),this.constant*=e,this}negate(){return this.constant*=-1,this.normal.negate(),this}distanceToPoint(e){return this.normal.dot(e)+this.constant}distanceToSphere(e){return this.distanceToPoint(e.center)-e.radius}projectPoint(e,t){return t.copy(e).addScaledVector(this.normal,-this.distanceToPoint(e))}intersectLine(e,t,i=!0){const r=e.delta(Ya),s=this.normal.dot(r);if(s===0)return this.distanceToPoint(e.start)===0?t.copy(e.start):null;const a=-(e.start.dot(this.normal)+this.constant)/s;return i===!0&&(a<0||a>1)?null:t.copy(e.start).addScaledVector(r,a)}intersectsLine(e){const t=this.distanceToPoint(e.start),i=this.distanceToPoint(e.end);return t<0&&i>0||i<0&&t>0}intersectsBox(e){return e.intersectsPlane(this)}intersectsSphere(e){return e.intersectsPlane(this)}coplanarPoint(e){return e.copy(this.normal).multiplyScalar(-this.constant)}applyMatrix4(e,t){const i=t||ih.getNormalMatrix(e),r=this.coplanarPoint(Ya).applyMatrix4(e),s=this.normal.applyMatrix3(i).normalize();return this.constant=-r.dot(s),this}translate(e){return this.constant-=e.dot(this.normal),this}equals(e){return e.normal.equals(this.normal)&&e.constant===this.constant}clone(){return new this.constructor().copy(this)}}const Ji=new gl,rh=new At(.5,.5),zs=new le;class Ld{constructor(e=new er,t=new er,i=new er,r=new er,s=new er,a=new er){this.planes=[e,t,i,r,s,a]}set(e,t,i,r,s,a){const o=this.planes;return o[0].copy(e),o[1].copy(t),o[2].copy(i),o[3].copy(r),o[4].copy(s),o[5].copy(a),this}copy(e){const t=this.planes;for(let i=0;i<6;i++)t[i].copy(e.planes[i]);return this}setFromProjectionMatrix(e,t=ti,i=!1){const r=this.planes,s=e.elements,a=s[0],o=s[1],l=s[2],c=s[3],f=s[4],u=s[5],d=s[6],h=s[7],_=s[8],x=s[9],m=s[10],p=s[11],C=s[12],D=s[13],M=s[14],L=s[15];if(r[0].setComponents(c-a,h-f,p-_,L-C).normalize(),r[1].setComponents(c+a,h+f,p+_,L+C).normalize(),r[2].setComponents(c+o,h+u,p+x,L+D).normalize(),r[3].setComponents(c-o,h-u,p-x,L-D).normalize(),i)r[4].setComponents(l,d,m,M).normalize(),r[5].setComponents(c-l,h-d,p-m,L-M).normalize();else if(r[4].setComponents(c-l,h-d,p-m,L-M).normalize(),t===ti)r[5].setComponents(c+l,h+d,p+m,L+M).normalize();else if(t===aa)r[5].setComponents(l,d,m,M).normalize();else throw new Error("THREE.Frustum.setFromProjectionMatrix(): Invalid coordinate system: "+t);return this}intersectsObject(e){if(e.boundingSphere!==void 0)e.boundingSphere===null&&e.computeBoundingSphere(),Ji.copy(e.boundingSphere).applyMatrix4(e.matrixWorld);else{const t=e.geometry;t.boundingSphere===null&&t.computeBoundingSphere(),Ji.copy(t.boundingSphere).applyMatrix4(e.matrixWorld)}return this.intersectsSphere(Ji)}intersectsSprite(e){Ji.center.set(0,0,0);const t=rh.distanceTo(e.center);return Ji.radius=.7071067811865476+t,Ji.applyMatrix4(e.matrixWorld),this.intersectsSphere(Ji)}intersectsSphere(e){const t=this.planes,i=e.center,r=-e.radius;for(let s=0;s<6;s++)if(t[s].distanceToPoint(i)<r)return!1;return!0}intersectsBox(e){const t=this.planes;for(let i=0;i<6;i++){const r=t[i];if(zs.x=r.normal.x>0?e.max.x:e.min.x,zs.y=r.normal.y>0?e.max.y:e.min.y,zs.z=r.normal.z>0?e.max.z:e.min.z,r.distanceToPoint(zs)<0)return!1}return!0}containsPoint(e){const t=this.planes;for(let i=0;i<6;i++)if(t[i].distanceToPoint(e)<0)return!1;return!0}clone(){return new this.constructor().copy(this)}}class Id extends un{constructor(e=[],t=ar,i,r,s,a,o,l,c,f){super(e,t,i,r,s,a,o,l,c,f),this.isCubeTexture=!0,this.flipY=!1}get images(){return this.image}set images(e){this.image=e}}class sh extends un{constructor(e,t,i,r,s,a,o,l,c){super(e,t,i,r,s,a,o,l,c),this.isCanvasTexture=!0,this.needsUpdate=!0}}class Gr extends un{constructor(e,t,i=ii,r,s,a,o=on,l=on,c,f=Ti,u=1){if(f!==Ti&&f!==rr)throw new Error("THREE.DepthTexture: format must be either THREE.DepthFormat or THREE.DepthStencilFormat");const d={width:e,height:t,depth:u};super(d,r,s,a,o,l,f,i,c),this.isDepthTexture=!0,this.flipY=!1,this.generateMipmaps=!1,this.compareFunction=null}copy(e){return super.copy(e),this.source=new ml(Object.assign({},e.image)),this.compareFunction=e.compareFunction,this}toJSON(e){const t=super.toJSON(e);return this.compareFunction!==null&&(t.compareFunction=this.compareFunction),t}}class ah extends Gr{constructor(e,t=ii,i=ar,r,s,a=on,o=on,l,c=Ti){const f={width:e,height:e,depth:1},u=[f,f,f,f,f,f];super(e,e,t,i,r,s,a,o,l,c),this.image=u,this.isCubeDepthTexture=!0,this.isCubeTexture=!0}get images(){return this.image}set images(e){this.image=e}}class Dd extends un{constructor(e=null){super(),this.sourceTexture=e,this.isExternalTexture=!0}copy(e){return super.copy(e),this.sourceTexture=e.sourceTexture,this}}class ms extends ai{constructor(e=1,t=1,i=1,r=1,s=1,a=1){super(),this.type="BoxGeometry",this.parameters={width:e,height:t,depth:i,widthSegments:r,heightSegments:s,depthSegments:a};const o=this;r=Math.floor(r),s=Math.floor(s),a=Math.floor(a);const l=[],c=[],f=[],u=[];let d=0,h=0;_("z","y","x",-1,-1,i,t,e,a,s,0),_("z","y","x",1,-1,i,t,-e,a,s,1),_("x","z","y",1,1,e,i,t,r,a,2),_("x","z","y",1,-1,e,i,-t,r,a,3),_("x","y","z",1,-1,e,t,i,r,s,4),_("x","y","z",-1,-1,e,t,-i,r,s,5),this.setIndex(l),this.setAttribute("position",new Mi(c,3)),this.setAttribute("normal",new Mi(f,3)),this.setAttribute("uv",new Mi(u,2));function _(x,m,p,C,D,M,L,A,P,v,b){const w=M/P,E=L/v,N=M/2,G=L/2,W=A/2,I=P+1,R=v+1;let F=0,J=0;const B=new le;for(let oe=0;oe<R;oe++){const k=oe*E-G;for(let K=0;K<I;K++){const Z=K*w-N;B[x]=Z*C,B[m]=k*D,B[p]=W,c.push(B.x,B.y,B.z),B[x]=0,B[m]=0,B[p]=A>0?1:-1,f.push(B.x,B.y,B.z),u.push(K/P),u.push(1-oe/v),F+=1}}for(let oe=0;oe<v;oe++)for(let k=0;k<P;k++){const K=d+k+I*oe,Z=d+k+I*(oe+1),Y=d+(k+1)+I*(oe+1),ne=d+(k+1)+I*oe;l.push(K,Z,ne),l.push(Z,Y,ne),J+=6}o.addGroup(h,J,b),h+=J,d+=F}}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new ms(e.width,e.height,e.depth,e.widthSegments,e.heightSegments,e.depthSegments)}}class ha extends ai{constructor(e=1,t=1,i=1,r=1){super(),this.type="PlaneGeometry",this.parameters={width:e,height:t,widthSegments:i,heightSegments:r};const s=e/2,a=t/2,o=Math.floor(i),l=Math.floor(r),c=o+1,f=l+1,u=e/o,d=t/l,h=[],_=[],x=[],m=[];for(let p=0;p<f;p++){const C=p*d-a;for(let D=0;D<c;D++){const M=D*u-s;_.push(M,-C,0),x.push(0,0,1),m.push(D/o),m.push(1-p/l)}}for(let p=0;p<l;p++)for(let C=0;C<o;C++){const D=C+c*p,M=C+c*(p+1),L=C+1+c*(p+1),A=C+1+c*p;h.push(D,M,A),h.push(M,L,A)}this.setIndex(h),this.setAttribute("position",new Mi(_,3)),this.setAttribute("normal",new Mi(x,3)),this.setAttribute("uv",new Mi(m,2))}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new ha(e.width,e.height,e.widthSegments,e.heightSegments)}}function Hr(n){const e={};for(const t in n){e[t]={};for(const i in n[t]){const r=n[t][i];if(sc(r))r.isRenderTargetTexture?(st("UniformsUtils: Textures of render targets cannot be cloned via cloneUniforms() or mergeUniforms()."),e[t][i]=null):e[t][i]=r.clone();else if(Array.isArray(r))if(sc(r[0])){const s=[];for(let a=0,o=r.length;a<o;a++)s[a]=r[a].clone();e[t][i]=s}else e[t][i]=r.slice();else e[t][i]=r}}return e}function hn(n){const e={};for(let t=0;t<n.length;t++){const i=Hr(n[t]);for(const r in i)e[r]=i[r]}return e}function sc(n){return n&&(n.isColor||n.isMatrix3||n.isMatrix4||n.isVector2||n.isVector3||n.isVector4||n.isTexture||n.isQuaternion)}function oh(n){const e=[];for(let t=0;t<n.length;t++)e.push(n[t].clone());return e}function Nd(n){const e=n.getRenderTarget();return e===null?n.outputColorSpace:e.isXRRenderTarget===!0?e.texture.colorSpace:yt.workingColorSpace}const lh={clone:Hr,merge:hn};var ch=`void main() {
	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}`,dh=`void main() {
	gl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );
}`;class si extends fa{constructor(e){super(),this.isShaderMaterial=!0,this.type="ShaderMaterial",this.defines={},this.uniforms={},this.uniformsGroups=[],this.vertexShader=ch,this.fragmentShader=dh,this.linewidth=1,this.wireframe=!1,this.wireframeLinewidth=1,this.fog=!1,this.lights=!1,this.clipping=!1,this.forceSinglePass=!0,this.extensions={clipCullDistance:!1,multiDraw:!1},this.defaultAttributeValues={color:[1,1,1],uv:[0,0],uv1:[0,0]},this.index0AttributeName=void 0,this.uniformsNeedUpdate=!1,this.glslVersion=null,e!==void 0&&this.setValues(e)}copy(e){return super.copy(e),this.fragmentShader=e.fragmentShader,this.vertexShader=e.vertexShader,this.uniforms=Hr(e.uniforms),this.uniformsGroups=oh(e.uniformsGroups),this.defines=Object.assign({},e.defines),this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.fog=e.fog,this.lights=e.lights,this.clipping=e.clipping,this.extensions=Object.assign({},e.extensions),this.glslVersion=e.glslVersion,this.defaultAttributeValues=Object.assign({},e.defaultAttributeValues),this.index0AttributeName=e.index0AttributeName,this.uniformsNeedUpdate=e.uniformsNeedUpdate,this}toJSON(e){const t=super.toJSON(e);t.glslVersion=this.glslVersion,t.uniforms={};for(const r in this.uniforms){const a=this.uniforms[r].value;a&&a.isTexture?t.uniforms[r]={type:"t",value:a.toJSON(e).uuid}:a&&a.isColor?t.uniforms[r]={type:"c",value:a.getHex()}:a&&a.isVector2?t.uniforms[r]={type:"v2",value:a.toArray()}:a&&a.isVector3?t.uniforms[r]={type:"v3",value:a.toArray()}:a&&a.isVector4?t.uniforms[r]={type:"v4",value:a.toArray()}:a&&a.isMatrix3?t.uniforms[r]={type:"m3",value:a.toArray()}:a&&a.isMatrix4?t.uniforms[r]={type:"m4",value:a.toArray()}:t.uniforms[r]={value:a}}Object.keys(this.defines).length>0&&(t.defines=this.defines),t.vertexShader=this.vertexShader,t.fragmentShader=this.fragmentShader,t.lights=this.lights,t.clipping=this.clipping;const i={};for(const r in this.extensions)this.extensions[r]===!0&&(i[r]=!0);return Object.keys(i).length>0&&(t.extensions=i),t}fromJSON(e,t){if(super.fromJSON(e,t),e.uniforms!==void 0)for(const i in e.uniforms){const r=e.uniforms[i];switch(this.uniforms[i]={},r.type){case"t":this.uniforms[i].value=t[r.value]||null;break;case"c":this.uniforms[i].value=new It().setHex(r.value);break;case"v2":this.uniforms[i].value=new At().fromArray(r.value);break;case"v3":this.uniforms[i].value=new le().fromArray(r.value);break;case"v4":this.uniforms[i].value=new Xt().fromArray(r.value);break;case"m3":this.uniforms[i].value=new ct().fromArray(r.value);break;case"m4":this.uniforms[i].value=new Zt().fromArray(r.value);break;default:this.uniforms[i].value=r.value}}if(e.defines!==void 0&&(this.defines=e.defines),e.vertexShader!==void 0&&(this.vertexShader=e.vertexShader),e.fragmentShader!==void 0&&(this.fragmentShader=e.fragmentShader),e.glslVersion!==void 0&&(this.glslVersion=e.glslVersion),e.extensions!==void 0)for(const i in e.extensions)this.extensions[i]=e.extensions[i];return e.lights!==void 0&&(this.lights=e.lights),e.clipping!==void 0&&(this.clipping=e.clipping),this}}class Ud extends si{constructor(e){super(e),this.isRawShaderMaterial=!0,this.type="RawShaderMaterial"}}class uh extends fa{constructor(e){super(),this.isMeshDepthMaterial=!0,this.type="MeshDepthMaterial",this.depthPacking=yf,this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.wireframe=!1,this.wireframeLinewidth=1,this.setValues(e)}copy(e){return super.copy(e),this.depthPacking=e.depthPacking,this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this}}class fh extends fa{constructor(e){super(),this.isMeshDistanceMaterial=!0,this.type="MeshDistanceMaterial",this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.setValues(e)}copy(e){return super.copy(e),this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this}}const Gs=new le,Hs=new Wr,$n=new le;class vl extends Tn{constructor(){super(),this.isCamera=!0,this.type="Camera",this.matrixWorldInverse=new Zt,this.projectionMatrix=new Zt,this.projectionMatrixInverse=new Zt,this.coordinateSystem=ti,this._reversedDepth=!1}get reversedDepth(){return this._reversedDepth}copy(e,t){return super.copy(e,t),this.matrixWorldInverse.copy(e.matrixWorldInverse),this.projectionMatrix.copy(e.projectionMatrix),this.projectionMatrixInverse.copy(e.projectionMatrixInverse),this.coordinateSystem=e.coordinateSystem,this}getWorldDirection(e){return super.getWorldDirection(e).negate()}updateMatrixWorld(e){super.updateMatrixWorld(e),this.matrixWorld.decompose(Gs,Hs,$n),$n.x===1&&$n.y===1&&$n.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(Gs,Hs,$n.set(1,1,1)).invert()}updateWorldMatrix(e,t,i=!1){super.updateWorldMatrix(e,t,i),this.matrixWorld.decompose(Gs,Hs,$n),$n.x===1&&$n.y===1&&$n.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(Gs,Hs,$n.set(1,1,1)).invert()}clone(){return new this.constructor().copy(this)}}const Fi=new le,ac=new At,oc=new At;class zn extends vl{constructor(e=50,t=1,i=.1,r=2e3){super(),this.isPerspectiveCamera=!0,this.type="PerspectiveCamera",this.fov=e,this.zoom=1,this.near=i,this.far=r,this.focus=10,this.aspect=t,this.view=null,this.filmGauge=35,this.filmOffset=0,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.fov=e.fov,this.zoom=e.zoom,this.near=e.near,this.far=e.far,this.focus=e.focus,this.aspect=e.aspect,this.view=e.view===null?null:Object.assign({},e.view),this.filmGauge=e.filmGauge,this.filmOffset=e.filmOffset,this}setFocalLength(e){const t=.5*this.getFilmHeight()/e;this.fov=Zo*2*Math.atan(t),this.updateProjectionMatrix()}getFocalLength(){const e=Math.tan(Ea*.5*this.fov);return .5*this.getFilmHeight()/e}getEffectiveFOV(){return Zo*2*Math.atan(Math.tan(Ea*.5*this.fov)/this.zoom)}getFilmWidth(){return this.filmGauge*Math.min(this.aspect,1)}getFilmHeight(){return this.filmGauge/Math.max(this.aspect,1)}getViewBounds(e,t,i){Fi.set(-1,-1,.5).applyMatrix4(this.projectionMatrixInverse),t.set(Fi.x,Fi.y).multiplyScalar(-e/Fi.z),Fi.set(1,1,.5).applyMatrix4(this.projectionMatrixInverse),i.set(Fi.x,Fi.y).multiplyScalar(-e/Fi.z)}getViewSize(e,t){return this.getViewBounds(e,ac,oc),t.subVectors(oc,ac)}setViewOffset(e,t,i,r,s,a){this.aspect=e/t,this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=i,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=this.near;let t=e*Math.tan(Ea*.5*this.fov)/this.zoom,i=2*t,r=this.aspect*i,s=-.5*r;const a=this.view;if(this.view!==null&&this.view.enabled){const l=a.fullWidth,c=a.fullHeight;s+=a.offsetX*r/l,t-=a.offsetY*i/c,r*=a.width/l,i*=a.height/c}const o=this.filmOffset;o!==0&&(s+=e*o/this.getFilmWidth()),this.projectionMatrix.makePerspective(s,s+r,t,t-i,e,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.fov=this.fov,t.object.zoom=this.zoom,t.object.near=this.near,t.object.far=this.far,t.object.focus=this.focus,t.object.aspect=this.aspect,this.view!==null&&(t.object.view=Object.assign({},this.view)),t.object.filmGauge=this.filmGauge,t.object.filmOffset=this.filmOffset,t}}class Fd extends vl{constructor(e=-1,t=1,i=1,r=-1,s=.1,a=2e3){super(),this.isOrthographicCamera=!0,this.type="OrthographicCamera",this.zoom=1,this.view=null,this.left=e,this.right=t,this.top=i,this.bottom=r,this.near=s,this.far=a,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.left=e.left,this.right=e.right,this.top=e.top,this.bottom=e.bottom,this.near=e.near,this.far=e.far,this.zoom=e.zoom,this.view=e.view===null?null:Object.assign({},e.view),this}setViewOffset(e,t,i,r,s,a){this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=i,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=(this.right-this.left)/(2*this.zoom),t=(this.top-this.bottom)/(2*this.zoom),i=(this.right+this.left)/2,r=(this.top+this.bottom)/2;let s=i-e,a=i+e,o=r+t,l=r-t;if(this.view!==null&&this.view.enabled){const c=(this.right-this.left)/this.view.fullWidth/this.zoom,f=(this.top-this.bottom)/this.view.fullHeight/this.zoom;s+=c*this.view.offsetX,a=s+c*this.view.width,o-=f*this.view.offsetY,l=o-f*this.view.height}this.projectionMatrix.makeOrthographic(s,a,o,l,this.near,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.zoom=this.zoom,t.object.left=this.left,t.object.right=this.right,t.object.top=this.top,t.object.bottom=this.bottom,t.object.near=this.near,t.object.far=this.far,this.view!==null&&(t.object.view=Object.assign({},this.view)),t}}const Rr=-90,Cr=1;class hh extends Tn{constructor(e,t,i){super(),this.type="CubeCamera",this.renderTarget=i,this.coordinateSystem=null,this.activeMipmapLevel=0;const r=new zn(Rr,Cr,e,t);r.layers=this.layers,this.add(r);const s=new zn(Rr,Cr,e,t);s.layers=this.layers,this.add(s);const a=new zn(Rr,Cr,e,t);a.layers=this.layers,this.add(a);const o=new zn(Rr,Cr,e,t);o.layers=this.layers,this.add(o);const l=new zn(Rr,Cr,e,t);l.layers=this.layers,this.add(l);const c=new zn(Rr,Cr,e,t);c.layers=this.layers,this.add(c)}updateCoordinateSystem(){const e=this.coordinateSystem,t=this.children.concat(),[i,r,s,a,o,l]=t;for(const c of t)this.remove(c);if(e===ti)i.up.set(0,1,0),i.lookAt(1,0,0),r.up.set(0,1,0),r.lookAt(-1,0,0),s.up.set(0,0,-1),s.lookAt(0,1,0),a.up.set(0,0,1),a.lookAt(0,-1,0),o.up.set(0,1,0),o.lookAt(0,0,1),l.up.set(0,1,0),l.lookAt(0,0,-1);else if(e===aa)i.up.set(0,-1,0),i.lookAt(-1,0,0),r.up.set(0,-1,0),r.lookAt(1,0,0),s.up.set(0,0,1),s.lookAt(0,1,0),a.up.set(0,0,-1),a.lookAt(0,-1,0),o.up.set(0,-1,0),o.lookAt(0,0,1),l.up.set(0,-1,0),l.lookAt(0,0,-1);else throw new Error("THREE.CubeCamera.updateCoordinateSystem(): Invalid coordinate system: "+e);for(const c of t)this.add(c),c.updateMatrixWorld()}update(e,t){this.parent===null&&this.updateMatrixWorld();const{renderTarget:i,activeMipmapLevel:r}=this;this.coordinateSystem!==e.coordinateSystem&&(this.coordinateSystem=e.coordinateSystem,this.updateCoordinateSystem());const[s,a,o,l,c,f]=this.children,u=e.getRenderTarget(),d=e.getActiveCubeFace(),h=e.getActiveMipmapLevel(),_=e.xr.enabled;e.xr.enabled=!1;const x=i.texture.generateMipmaps;i.texture.generateMipmaps=!1;let m=!1;e.isWebGLRenderer===!0?m=e.state.buffers.depth.getReversed():m=e.reversedDepthBuffer,e.setRenderTarget(i,0,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,s),e.setRenderTarget(i,1,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,a),e.setRenderTarget(i,2,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,o),e.setRenderTarget(i,3,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,l),e.setRenderTarget(i,4,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,c),i.texture.generateMipmaps=x,e.setRenderTarget(i,5,r),m&&e.autoClear===!1&&e.clearDepth(),e.render(t,f),e.setRenderTarget(u,d,h),e.xr.enabled=_,i.texture.needsPMREMUpdate=!0}}class ph extends zn{constructor(e=[]){super(),this.isArrayCamera=!0,this.isMultiViewCamera=!1,this.cameras=e}}const Ml=class Ml{constructor(e,t,i,r){this.elements=[1,0,0,1],e!==void 0&&this.set(e,t,i,r)}identity(){return this.set(1,0,0,1),this}fromArray(e,t=0){for(let i=0;i<4;i++)this.elements[i]=e[i+t];return this}set(e,t,i,r){const s=this.elements;return s[0]=e,s[2]=t,s[1]=i,s[3]=r,this}};Ml.prototype.isMatrix2=!0;let lc=Ml;function cc(n,e,t,i){const r=mh(i);switch(t){case bd:return n*e;case Md:return n*e/r.components*r.byteLength;case dl:return n*e/r.components*r.byteLength;case or:return n*e*2/r.components*r.byteLength;case ul:return n*e*2/r.components*r.byteLength;case Sd:return n*e*3/r.components*r.byteLength;case Hn:return n*e*4/r.components*r.byteLength;case fl:return n*e*4/r.components*r.byteLength;case Zs:case Js:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*8;case Qs:case js:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case xo:case bo:return Math.max(n,16)*Math.max(e,8)/4;case vo:case yo:return Math.max(n,8)*Math.max(e,8)/2;case So:case Mo:case wo:case To:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*8;case Eo:case ia:case Ao:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case Ro:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case Co:return Math.floor((n+4)/5)*Math.floor((e+3)/4)*16;case Po:return Math.floor((n+4)/5)*Math.floor((e+4)/5)*16;case Lo:return Math.floor((n+5)/6)*Math.floor((e+4)/5)*16;case Io:return Math.floor((n+5)/6)*Math.floor((e+5)/6)*16;case Do:return Math.floor((n+7)/8)*Math.floor((e+4)/5)*16;case No:return Math.floor((n+7)/8)*Math.floor((e+5)/6)*16;case Uo:return Math.floor((n+7)/8)*Math.floor((e+7)/8)*16;case Fo:return Math.floor((n+9)/10)*Math.floor((e+4)/5)*16;case Oo:return Math.floor((n+9)/10)*Math.floor((e+5)/6)*16;case Bo:return Math.floor((n+9)/10)*Math.floor((e+7)/8)*16;case ko:return Math.floor((n+9)/10)*Math.floor((e+9)/10)*16;case zo:return Math.floor((n+11)/12)*Math.floor((e+9)/10)*16;case Go:return Math.floor((n+11)/12)*Math.floor((e+11)/12)*16;case Ho:case Vo:case Wo:return Math.ceil(n/4)*Math.ceil(e/4)*16;case Xo:case qo:return Math.ceil(n/4)*Math.ceil(e/4)*8;case ra:case Yo:return Math.ceil(n/4)*Math.ceil(e/4)*16}throw new Error(`Unable to determine texture byte length for ${t} format.`)}function mh(n){switch(n){case In:case _d:return{byteLength:1,components:1};case ds:case vd:case wi:return{byteLength:2,components:1};case ll:case cl:return{byteLength:2,components:4};case ii:case ol:case ei:return{byteLength:4,components:1};case xd:case yd:return{byteLength:4,components:3}}throw new Error(`THREE.TextureUtils: Unknown texture type ${n}.`)}typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("register",{detail:{revision:al}}));typeof window<"u"&&(window.__THREE__?st("WARNING: Multiple instances of Three.js being imported."):window.__THREE__=al);function Od(){let n=null,e=!1,t=null,i=null;function r(s,a){t(s,a),i=n.requestAnimationFrame(r)}return{start:function(){e!==!0&&t!==null&&n!==null&&(i=n.requestAnimationFrame(r),e=!0)},stop:function(){n!==null&&n.cancelAnimationFrame(i),e=!1},setAnimationLoop:function(s){t=s},setContext:function(s){n=s}}}function gh(n){const e=new WeakMap;function t(o,l){const c=o.array,f=o.usage,u=c.byteLength,d=n.createBuffer();n.bindBuffer(l,d),n.bufferData(l,c,f),o.onUploadCallback();let h;if(c instanceof Float32Array)h=n.FLOAT;else if(typeof Float16Array<"u"&&c instanceof Float16Array)h=n.HALF_FLOAT;else if(c instanceof Uint16Array)o.isFloat16BufferAttribute?h=n.HALF_FLOAT:h=n.UNSIGNED_SHORT;else if(c instanceof Int16Array)h=n.SHORT;else if(c instanceof Uint32Array)h=n.UNSIGNED_INT;else if(c instanceof Int32Array)h=n.INT;else if(c instanceof Int8Array)h=n.BYTE;else if(c instanceof Uint8Array)h=n.UNSIGNED_BYTE;else if(c instanceof Uint8ClampedArray)h=n.UNSIGNED_BYTE;else throw new Error("THREE.WebGLAttributes: Unsupported buffer data format: "+c);return{buffer:d,type:h,bytesPerElement:c.BYTES_PER_ELEMENT,version:o.version,size:u}}function i(o,l,c){const f=l.array,u=l.updateRanges;if(n.bindBuffer(c,o),u.length===0)n.bufferSubData(c,0,f);else{u.sort((h,_)=>h.start-_.start);let d=0;for(let h=1;h<u.length;h++){const _=u[d],x=u[h];x.start<=_.start+_.count+1?_.count=Math.max(_.count,x.start+x.count-_.start):(++d,u[d]=x)}u.length=d+1;for(let h=0,_=u.length;h<_;h++){const x=u[h];n.bufferSubData(c,x.start*f.BYTES_PER_ELEMENT,f,x.start,x.count)}l.clearUpdateRanges()}l.onUploadCallback()}function r(o){return o.isInterleavedBufferAttribute&&(o=o.data),e.get(o)}function s(o){o.isInterleavedBufferAttribute&&(o=o.data);const l=e.get(o);l&&(n.deleteBuffer(l.buffer),e.delete(o))}function a(o,l){if(o.isInterleavedBufferAttribute&&(o=o.data),o.isGLBufferAttribute){const f=e.get(o);(!f||f.version<o.version)&&e.set(o,{buffer:o.buffer,type:o.type,bytesPerElement:o.elementSize,version:o.version});return}const c=e.get(o);if(c===void 0)e.set(o,t(o,l));else if(c.version<o.version){if(c.size!==o.array.byteLength)throw new Error("THREE.WebGLAttributes: The size of the buffer attribute's array buffer does not match the original size. Resizing buffer attributes is not supported.");i(c.buffer,o,l),c.version=o.version}}return{get:r,remove:s,update:a}}var _h=`#ifdef USE_ALPHAHASH
	if ( diffuseColor.a < getAlphaHashThreshold( vPosition ) ) discard;
#endif`,vh=`#ifdef USE_ALPHAHASH
	const float ALPHA_HASH_SCALE = 0.05;
	float hash2D( vec2 value ) {
		return fract( 1.0e4 * sin( 17.0 * value.x + 0.1 * value.y ) * ( 0.1 + abs( sin( 13.0 * value.y + value.x ) ) ) );
	}
	float hash3D( vec3 value ) {
		return hash2D( vec2( hash2D( value.xy ), value.z ) );
	}
	float getAlphaHashThreshold( vec3 position ) {
		float maxDeriv = max(
			length( dFdx( position.xyz ) ),
			length( dFdy( position.xyz ) )
		);
		float pixScale = 1.0 / ( ALPHA_HASH_SCALE * maxDeriv );
		vec2 pixScales = vec2(
			exp2( floor( log2( pixScale ) ) ),
			exp2( ceil( log2( pixScale ) ) )
		);
		vec2 alpha = vec2(
			hash3D( floor( pixScales.x * position.xyz ) ),
			hash3D( floor( pixScales.y * position.xyz ) )
		);
		float lerpFactor = fract( log2( pixScale ) );
		float x = ( 1.0 - lerpFactor ) * alpha.x + lerpFactor * alpha.y;
		float a = min( lerpFactor, 1.0 - lerpFactor );
		vec3 cases = vec3(
			x * x / ( 2.0 * a * ( 1.0 - a ) ),
			( x - 0.5 * a ) / ( 1.0 - a ),
			1.0 - ( ( 1.0 - x ) * ( 1.0 - x ) / ( 2.0 * a * ( 1.0 - a ) ) )
		);
		float threshold = ( x < ( 1.0 - a ) )
			? ( ( x < a ) ? cases.x : cases.y )
			: cases.z;
		return clamp( threshold , 1.0e-6, 1.0 );
	}
#endif`,xh=`#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, vAlphaMapUv ).g;
#endif`,yh=`#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,bh=`#ifdef USE_ALPHATEST
	#ifdef ALPHA_TO_COVERAGE
	diffuseColor.a = smoothstep( alphaTest, alphaTest + fwidth( diffuseColor.a ), diffuseColor.a );
	if ( diffuseColor.a == 0.0 ) discard;
	#else
	if ( diffuseColor.a < alphaTest ) discard;
	#endif
#endif`,Sh=`#ifdef USE_ALPHATEST
	uniform float alphaTest;
#endif`,Mh=`#ifdef USE_AOMAP
	float ambientOcclusion = ( texture2D( aoMap, vAoMapUv ).r - 1.0 ) * aoMapIntensity + 1.0;
	reflectedLight.indirectDiffuse *= ambientOcclusion;
	#if defined( USE_CLEARCOAT ) 
		clearcoatSpecularIndirect *= ambientOcclusion;
	#endif
	#if defined( USE_SHEEN ) 
		sheenSpecularIndirect *= ambientOcclusion;
	#endif
	#if defined( USE_ENVMAP ) && defined( STANDARD )
		float dotNV = saturate( dot( geometryNormal, geometryViewDir ) );
		reflectedLight.indirectSpecular *= computeSpecularOcclusion( dotNV, ambientOcclusion, material.roughness );
	#endif
#endif`,Eh=`#ifdef USE_AOMAP
	uniform sampler2D aoMap;
	uniform float aoMapIntensity;
#endif`,wh=`#ifdef USE_BATCHING
	#if ! defined( GL_ANGLE_multi_draw )
	#define gl_DrawID _gl_DrawID
	uniform int _gl_DrawID;
	#endif
	uniform highp sampler2D batchingTexture;
	uniform highp usampler2D batchingIdTexture;
	mat4 getBatchingMatrix( const in float i ) {
		int size = textureSize( batchingTexture, 0 ).x;
		int j = int( i ) * 4;
		int x = j % size;
		int y = j / size;
		vec4 v1 = texelFetch( batchingTexture, ivec2( x, y ), 0 );
		vec4 v2 = texelFetch( batchingTexture, ivec2( x + 1, y ), 0 );
		vec4 v3 = texelFetch( batchingTexture, ivec2( x + 2, y ), 0 );
		vec4 v4 = texelFetch( batchingTexture, ivec2( x + 3, y ), 0 );
		return mat4( v1, v2, v3, v4 );
	}
	float getIndirectIndex( const in int i ) {
		int size = textureSize( batchingIdTexture, 0 ).x;
		int x = i % size;
		int y = i / size;
		return float( texelFetch( batchingIdTexture, ivec2( x, y ), 0 ).r );
	}
#endif
#ifdef USE_BATCHING_COLOR
	uniform sampler2D batchingColorTexture;
	vec4 getBatchingColor( const in float i ) {
		int size = textureSize( batchingColorTexture, 0 ).x;
		int j = int( i );
		int x = j % size;
		int y = j / size;
		return texelFetch( batchingColorTexture, ivec2( x, y ), 0 );
	}
#endif`,Th=`#ifdef USE_BATCHING
	mat4 batchingMatrix = getBatchingMatrix( getIndirectIndex( gl_DrawID ) );
#endif`,Ah=`vec3 transformed = vec3( position );
#ifdef USE_ALPHAHASH
	vPosition = vec3( position );
#endif`,Rh=`vec3 objectNormal = vec3( normal );
#ifdef USE_TANGENT
	vec3 objectTangent = vec3( tangent.xyz );
#endif`,Ch=`float G_BlinnPhong_Implicit( ) {
	return 0.25;
}
float D_BlinnPhong( const in float shininess, const in float dotNH ) {
	return RECIPROCAL_PI * ( shininess * 0.5 + 1.0 ) * pow( dotNH, shininess );
}
vec3 BRDF_BlinnPhong( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in vec3 specularColor, const in float shininess ) {
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNH = saturate( dot( normal, halfDir ) );
	float dotVH = saturate( dot( viewDir, halfDir ) );
	vec3 F = F_Schlick( specularColor, 1.0, dotVH );
	float G = G_BlinnPhong_Implicit( );
	float D = D_BlinnPhong( shininess, dotNH );
	return F * ( G * D );
} // validated`,Ph=`#ifdef USE_IRIDESCENCE
	const mat3 XYZ_TO_REC709 = mat3(
		 3.2404542, -0.9692660,  0.0556434,
		-1.5371385,  1.8760108, -0.2040259,
		-0.4985314,  0.0415560,  1.0572252
	);
	vec3 Fresnel0ToIor( vec3 fresnel0 ) {
		vec3 sqrtF0 = sqrt( fresnel0 );
		return ( vec3( 1.0 ) + sqrtF0 ) / ( vec3( 1.0 ) - sqrtF0 );
	}
	vec3 IorToFresnel0( vec3 transmittedIor, float incidentIor ) {
		return pow2( ( transmittedIor - vec3( incidentIor ) ) / ( transmittedIor + vec3( incidentIor ) ) );
	}
	float IorToFresnel0( float transmittedIor, float incidentIor ) {
		return pow2( ( transmittedIor - incidentIor ) / ( transmittedIor + incidentIor ));
	}
	vec3 evalSensitivity( float OPD, vec3 shift ) {
		float phase = 2.0 * PI * OPD * 1.0e-9;
		vec3 val = vec3( 5.4856e-13, 4.4201e-13, 5.2481e-13 );
		vec3 pos = vec3( 1.6810e+06, 1.7953e+06, 2.2084e+06 );
		vec3 var = vec3( 4.3278e+09, 9.3046e+09, 6.6121e+09 );
		vec3 xyz = val * sqrt( 2.0 * PI * var ) * cos( pos * phase + shift ) * exp( - pow2( phase ) * var );
		xyz.x += 9.7470e-14 * sqrt( 2.0 * PI * 4.5282e+09 ) * cos( 2.2399e+06 * phase + shift[ 0 ] ) * exp( - 4.5282e+09 * pow2( phase ) );
		xyz /= 1.0685e-7;
		vec3 rgb = XYZ_TO_REC709 * xyz;
		return rgb;
	}
	vec3 evalIridescence( float outsideIOR, float eta2, float cosTheta1, float thinFilmThickness, vec3 baseF0 ) {
		vec3 I;
		float iridescenceIOR = mix( outsideIOR, eta2, smoothstep( 0.0, 0.03, thinFilmThickness ) );
		float sinTheta2Sq = pow2( outsideIOR / iridescenceIOR ) * ( 1.0 - pow2( cosTheta1 ) );
		float cosTheta2Sq = 1.0 - sinTheta2Sq;
		if ( cosTheta2Sq < 0.0 ) {
			return vec3( 1.0 );
		}
		float cosTheta2 = sqrt( cosTheta2Sq );
		float R0 = IorToFresnel0( iridescenceIOR, outsideIOR );
		float R12 = F_Schlick( R0, 1.0, cosTheta1 );
		float T121 = 1.0 - R12;
		float phi12 = 0.0;
		if ( iridescenceIOR < outsideIOR ) phi12 = PI;
		float phi21 = PI - phi12;
		vec3 baseIOR = Fresnel0ToIor( clamp( baseF0, 0.0, 0.9999 ) );		vec3 R1 = IorToFresnel0( baseIOR, iridescenceIOR );
		vec3 R23 = F_Schlick( R1, 1.0, cosTheta2 );
		vec3 phi23 = vec3( 0.0 );
		if ( baseIOR[ 0 ] < iridescenceIOR ) phi23[ 0 ] = PI;
		if ( baseIOR[ 1 ] < iridescenceIOR ) phi23[ 1 ] = PI;
		if ( baseIOR[ 2 ] < iridescenceIOR ) phi23[ 2 ] = PI;
		float OPD = 2.0 * iridescenceIOR * thinFilmThickness * cosTheta2;
		vec3 phi = vec3( phi21 ) + phi23;
		vec3 R123 = clamp( R12 * R23, 1e-5, 0.9999 );
		vec3 r123 = sqrt( R123 );
		vec3 Rs = pow2( T121 ) * R23 / ( vec3( 1.0 ) - R123 );
		vec3 C0 = R12 + Rs;
		I = C0;
		vec3 Cm = Rs - T121;
		for ( int m = 1; m <= 2; ++ m ) {
			Cm *= r123;
			vec3 Sm = 2.0 * evalSensitivity( float( m ) * OPD, float( m ) * phi );
			I += Cm * Sm;
		}
		return max( I, vec3( 0.0 ) );
	}
#endif`,Lh=`#ifdef USE_BUMPMAP
	uniform sampler2D bumpMap;
	uniform float bumpScale;
	vec2 dHdxy_fwd() {
		vec2 dSTdx = dFdx( vBumpMapUv );
		vec2 dSTdy = dFdy( vBumpMapUv );
		float Hll = bumpScale * texture2D( bumpMap, vBumpMapUv ).x;
		float dBx = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdx ).x - Hll;
		float dBy = bumpScale * texture2D( bumpMap, vBumpMapUv + dSTdy ).x - Hll;
		return vec2( dBx, dBy );
	}
	vec3 perturbNormalArb( vec3 surf_pos, vec3 surf_norm, vec2 dHdxy, float faceDirection ) {
		vec3 vSigmaX = normalize( dFdx( surf_pos.xyz ) );
		vec3 vSigmaY = normalize( dFdy( surf_pos.xyz ) );
		vec3 vN = surf_norm;
		vec3 R1 = cross( vSigmaY, vN );
		vec3 R2 = cross( vN, vSigmaX );
		float fDet = dot( vSigmaX, R1 ) * faceDirection;
		vec3 vGrad = sign( fDet ) * ( dHdxy.x * R1 + dHdxy.y * R2 );
		return normalize( abs( fDet ) * surf_norm - vGrad );
	}
#endif`,Ih=`#if NUM_CLIPPING_PLANES > 0
	vec4 plane;
	#ifdef ALPHA_TO_COVERAGE
		float distanceToPlane, distanceGradient;
		float clipOpacity = 1.0;
		#pragma unroll_loop_start
		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {
			plane = clippingPlanes[ i ];
			distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;
			distanceGradient = fwidth( distanceToPlane ) / 2.0;
			clipOpacity *= smoothstep( - distanceGradient, distanceGradient, distanceToPlane );
			if ( clipOpacity == 0.0 ) discard;
		}
		#pragma unroll_loop_end
		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES
			float unionClipOpacity = 1.0;
			#pragma unroll_loop_start
			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {
				plane = clippingPlanes[ i ];
				distanceToPlane = - dot( vClipPosition, plane.xyz ) + plane.w;
				distanceGradient = fwidth( distanceToPlane ) / 2.0;
				unionClipOpacity *= 1.0 - smoothstep( - distanceGradient, distanceGradient, distanceToPlane );
			}
			#pragma unroll_loop_end
			clipOpacity *= 1.0 - unionClipOpacity;
		#endif
		diffuseColor.a *= clipOpacity;
		if ( diffuseColor.a == 0.0 ) discard;
	#else
		#pragma unroll_loop_start
		for ( int i = 0; i < UNION_CLIPPING_PLANES; i ++ ) {
			plane = clippingPlanes[ i ];
			if ( dot( vClipPosition, plane.xyz ) > plane.w ) discard;
		}
		#pragma unroll_loop_end
		#if UNION_CLIPPING_PLANES < NUM_CLIPPING_PLANES
			bool clipped = true;
			#pragma unroll_loop_start
			for ( int i = UNION_CLIPPING_PLANES; i < NUM_CLIPPING_PLANES; i ++ ) {
				plane = clippingPlanes[ i ];
				clipped = ( dot( vClipPosition, plane.xyz ) > plane.w ) && clipped;
			}
			#pragma unroll_loop_end
			if ( clipped ) discard;
		#endif
	#endif
#endif`,Dh=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
	uniform vec4 clippingPlanes[ NUM_CLIPPING_PLANES ];
#endif`,Nh=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
#endif`,Uh=`#if NUM_CLIPPING_PLANES > 0
	vClipPosition = - mvPosition.xyz;
#endif`,Fh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	diffuseColor *= vColor;
#endif`,Oh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	varying vec4 vColor;
#endif`,Bh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	varying vec4 vColor;
#endif`,kh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	vColor = vec4( 1.0 );
#endif
#ifdef USE_COLOR_ALPHA
	vColor *= color;
#elif defined( USE_COLOR )
	vColor.rgb *= color;
#endif
#ifdef USE_INSTANCING_COLOR
	vColor.rgb *= instanceColor.rgb;
#endif
#ifdef USE_BATCHING_COLOR
	vColor *= getBatchingColor( getIndirectIndex( gl_DrawID ) );
#endif`,zh=`#define PI 3.141592653589793
#define PI2 6.283185307179586
#define PI_HALF 1.5707963267948966
#define RECIPROCAL_PI 0.3183098861837907
#define RECIPROCAL_PI2 0.15915494309189535
#define EPSILON 1e-6
#ifndef saturate
#define saturate( a ) clamp( a, 0.0, 1.0 )
#endif
#define whiteComplement( a ) ( 1.0 - saturate( a ) )
float pow2( const in float x ) { return x*x; }
vec3 pow2( const in vec3 x ) { return x*x; }
float pow3( const in float x ) { return x*x*x; }
float pow4( const in float x ) { float x2 = x*x; return x2*x2; }
float max3( const in vec3 v ) { return max( max( v.x, v.y ), v.z ); }
float average( const in vec3 v ) { return dot( v, vec3( 0.3333333 ) ); }
highp float rand( const in vec2 uv ) {
	const highp float a = 12.9898, b = 78.233, c = 43758.5453;
	highp float dt = dot( uv.xy, vec2( a,b ) ), sn = mod( dt, PI );
	return fract( sin( sn ) * c );
}
#ifdef HIGH_PRECISION
	float precisionSafeLength( vec3 v ) { return length( v ); }
#else
	float precisionSafeLength( vec3 v ) {
		float maxComponent = max3( abs( v ) );
		return length( v / maxComponent ) * maxComponent;
	}
#endif
struct IncidentLight {
	vec3 color;
	vec3 direction;
	bool visible;
};
struct ReflectedLight {
	vec3 directDiffuse;
	vec3 directSpecular;
	vec3 indirectDiffuse;
	vec3 indirectSpecular;
};
#ifdef USE_ALPHAHASH
	varying vec3 vPosition;
#endif
vec3 transformDirection( in vec3 dir, in mat4 matrix ) {
	return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );
}
#define inverseTransformDirection transformDirectionByInverseViewMatrix
vec3 transformNormalByInverseViewMatrix( in vec3 normal, in mat4 viewMatrix ) {
	return normalize( ( vec4( normal, 0.0 ) * viewMatrix ).xyz );
}
vec3 transformDirectionByInverseViewMatrix( in vec3 dir, in mat4 viewMatrix ) {
	return normalize( ( vec4( dir, 0.0 ) * viewMatrix ).xyz );
}
bool isPerspectiveMatrix( mat4 m ) {
	return m[ 2 ][ 3 ] == - 1.0;
}
vec2 equirectUv( in vec3 dir ) {
	float u = atan( dir.z, dir.x ) * RECIPROCAL_PI2 + 0.5;
	float v = asin( clamp( dir.y, - 1.0, 1.0 ) ) * RECIPROCAL_PI + 0.5;
	return vec2( u, v );
}
vec3 BRDF_Lambert( const in vec3 diffuseColor ) {
	return RECIPROCAL_PI * diffuseColor;
}
vec3 F_Schlick( const in vec3 f0, const in float f90, const in float dotVH ) {
	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );
	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );
}
float F_Schlick( const in float f0, const in float f90, const in float dotVH ) {
	float fresnel = exp2( ( - 5.55473 * dotVH - 6.98316 ) * dotVH );
	return f0 * ( 1.0 - fresnel ) + ( f90 * fresnel );
} // validated`,Gh=`#ifdef ENVMAP_TYPE_CUBE_UV
	#define cubeUV_minMipLevel 4.0
	#define cubeUV_minTileSize 16.0
	float getFace( vec3 direction ) {
		vec3 absDirection = abs( direction );
		float face = - 1.0;
		if ( absDirection.x > absDirection.z ) {
			if ( absDirection.x > absDirection.y )
				face = direction.x > 0.0 ? 0.0 : 3.0;
			else
				face = direction.y > 0.0 ? 1.0 : 4.0;
		} else {
			if ( absDirection.z > absDirection.y )
				face = direction.z > 0.0 ? 2.0 : 5.0;
			else
				face = direction.y > 0.0 ? 1.0 : 4.0;
		}
		return face;
	}
	vec2 getUV( vec3 direction, float face ) {
		vec2 uv;
		if ( face == 0.0 ) {
			uv = vec2( direction.z, direction.y ) / abs( direction.x );
		} else if ( face == 1.0 ) {
			uv = vec2( - direction.x, - direction.z ) / abs( direction.y );
		} else if ( face == 2.0 ) {
			uv = vec2( - direction.x, direction.y ) / abs( direction.z );
		} else if ( face == 3.0 ) {
			uv = vec2( - direction.z, direction.y ) / abs( direction.x );
		} else if ( face == 4.0 ) {
			uv = vec2( - direction.x, direction.z ) / abs( direction.y );
		} else {
			uv = vec2( direction.x, direction.y ) / abs( direction.z );
		}
		return 0.5 * ( uv + 1.0 );
	}
	vec3 bilinearCubeUV( sampler2D envMap, vec3 direction, float mipInt ) {
		float face = getFace( direction );
		float filterInt = max( cubeUV_minMipLevel - mipInt, 0.0 );
		mipInt = max( mipInt, cubeUV_minMipLevel );
		float faceSize = exp2( mipInt );
		highp vec2 uv = getUV( direction, face ) * ( faceSize - 2.0 ) + 1.0;
		if ( face > 2.0 ) {
			uv.y += faceSize;
			face -= 3.0;
		}
		uv.x += face * faceSize;
		uv.x += filterInt * 3.0 * cubeUV_minTileSize;
		uv.y += 4.0 * ( exp2( CUBEUV_MAX_MIP ) - faceSize );
		uv.x *= CUBEUV_TEXEL_WIDTH;
		uv.y *= CUBEUV_TEXEL_HEIGHT;
		#ifdef texture2DGradEXT
			return texture2DGradEXT( envMap, uv, vec2( 0.0 ), vec2( 0.0 ) ).rgb;
		#else
			return texture2D( envMap, uv ).rgb;
		#endif
	}
	#define cubeUV_r0 1.0
	#define cubeUV_m0 - 2.0
	#define cubeUV_r1 0.8
	#define cubeUV_m1 - 1.0
	#define cubeUV_r4 0.4
	#define cubeUV_m4 2.0
	#define cubeUV_r5 0.305
	#define cubeUV_m5 3.0
	#define cubeUV_r6 0.21
	#define cubeUV_m6 4.0
	float roughnessToMip( float roughness ) {
		float mip = 0.0;
		if ( roughness >= cubeUV_r1 ) {
			mip = ( cubeUV_r0 - roughness ) * ( cubeUV_m1 - cubeUV_m0 ) / ( cubeUV_r0 - cubeUV_r1 ) + cubeUV_m0;
		} else if ( roughness >= cubeUV_r4 ) {
			mip = ( cubeUV_r1 - roughness ) * ( cubeUV_m4 - cubeUV_m1 ) / ( cubeUV_r1 - cubeUV_r4 ) + cubeUV_m1;
		} else if ( roughness >= cubeUV_r5 ) {
			mip = ( cubeUV_r4 - roughness ) * ( cubeUV_m5 - cubeUV_m4 ) / ( cubeUV_r4 - cubeUV_r5 ) + cubeUV_m4;
		} else if ( roughness >= cubeUV_r6 ) {
			mip = ( cubeUV_r5 - roughness ) * ( cubeUV_m6 - cubeUV_m5 ) / ( cubeUV_r5 - cubeUV_r6 ) + cubeUV_m5;
		} else {
			mip = - 2.0 * log2( 1.16 * roughness );		}
		return mip;
	}
	vec4 textureCubeUV( sampler2D envMap, vec3 sampleDir, float roughness ) {
		float mip = clamp( roughnessToMip( roughness ), cubeUV_m0, CUBEUV_MAX_MIP );
		float mipF = fract( mip );
		float mipInt = floor( mip );
		vec3 color0 = bilinearCubeUV( envMap, sampleDir, mipInt );
		if ( mipF == 0.0 ) {
			return vec4( color0, 1.0 );
		} else {
			vec3 color1 = bilinearCubeUV( envMap, sampleDir, mipInt + 1.0 );
			return vec4( mix( color0, color1, mipF ), 1.0 );
		}
	}
#endif`,Hh=`vec3 transformedNormal = objectNormal;
#ifdef USE_TANGENT
	vec3 transformedTangent = objectTangent;
#endif
#ifdef USE_BATCHING
	mat3 bm = mat3( batchingMatrix );
	transformedNormal /= vec3( dot( bm[ 0 ], bm[ 0 ] ), dot( bm[ 1 ], bm[ 1 ] ), dot( bm[ 2 ], bm[ 2 ] ) );
	transformedNormal = bm * transformedNormal;
	#ifdef USE_TANGENT
		transformedTangent = bm * transformedTangent;
	#endif
#endif
#ifdef USE_INSTANCING
	mat3 im = mat3( instanceMatrix );
	transformedNormal /= vec3( dot( im[ 0 ], im[ 0 ] ), dot( im[ 1 ], im[ 1 ] ), dot( im[ 2 ], im[ 2 ] ) );
	transformedNormal = im * transformedNormal;
	#ifdef USE_TANGENT
		transformedTangent = im * transformedTangent;
	#endif
#endif
transformedNormal = normalMatrix * transformedNormal;
#ifdef FLIP_SIDED
	transformedNormal = - transformedNormal;
#endif
#ifdef USE_TANGENT
	transformedTangent = ( modelViewMatrix * vec4( transformedTangent, 0.0 ) ).xyz;
#endif`,Vh=`#ifdef USE_DISPLACEMENTMAP
	uniform sampler2D displacementMap;
	uniform float displacementScale;
	uniform float displacementBias;
#endif`,Wh=`#ifdef USE_DISPLACEMENTMAP
	transformed += normalize( objectNormal ) * ( texture2D( displacementMap, vDisplacementMapUv ).x * displacementScale + displacementBias );
#endif`,Xh=`#ifdef USE_EMISSIVEMAP
	vec4 emissiveColor = texture2D( emissiveMap, vEmissiveMapUv );
	#ifdef DECODE_VIDEO_TEXTURE_EMISSIVE
		emissiveColor = sRGBTransferEOTF( emissiveColor );
	#endif
	totalEmissiveRadiance *= emissiveColor.rgb;
#endif`,qh=`#ifdef USE_EMISSIVEMAP
	uniform sampler2D emissiveMap;
#endif`,Yh="gl_FragColor = linearToOutputTexel( gl_FragColor );",$h=`vec4 LinearTransferOETF( in vec4 value ) {
	return value;
}
vec4 sRGBTransferEOTF( in vec4 value ) {
	return vec4( mix( pow( value.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), value.rgb * 0.0773993808, vec3( lessThanEqual( value.rgb, vec3( 0.04045 ) ) ) ), value.a );
}
vec4 sRGBTransferOETF( in vec4 value ) {
	return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
}`,Kh=`#ifdef USE_ENVMAP
	#ifdef ENV_WORLDPOS
		vec3 cameraToFrag;
		if ( isOrthographic ) {
			cameraToFrag = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );
		} else {
			cameraToFrag = normalize( vWorldPosition - cameraPosition );
		}
		vec3 worldNormal = transformNormalByInverseViewMatrix( normal, viewMatrix );
		#ifdef ENVMAP_MODE_REFLECTION
			vec3 reflectVec = reflect( cameraToFrag, worldNormal );
		#else
			vec3 reflectVec = refract( cameraToFrag, worldNormal, refractionRatio );
		#endif
	#else
		vec3 reflectVec = vReflect;
	#endif
	#ifdef ENVMAP_TYPE_CUBE
		vec4 envColor = textureCube( envMap, envMapRotation * reflectVec );
		#ifdef ENVMAP_BLENDING_MULTIPLY
			outgoingLight = mix( outgoingLight, outgoingLight * envColor.xyz, specularStrength * reflectivity );
		#elif defined( ENVMAP_BLENDING_MIX )
			outgoingLight = mix( outgoingLight, envColor.xyz, specularStrength * reflectivity );
		#elif defined( ENVMAP_BLENDING_ADD )
			outgoingLight += envColor.xyz * specularStrength * reflectivity;
		#endif
	#endif
#endif`,Zh=`#ifdef USE_ENVMAP
	uniform float envMapIntensity;
	uniform mat3 envMapRotation;
	#ifdef ENVMAP_TYPE_CUBE
		uniform samplerCube envMap;
	#else
		uniform sampler2D envMap;
	#endif
#endif`,Jh=`#ifdef USE_ENVMAP
	uniform float reflectivity;
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		varying vec3 vWorldPosition;
		uniform float refractionRatio;
	#else
		varying vec3 vReflect;
	#endif
#endif`,Qh=`#ifdef USE_ENVMAP
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		
		varying vec3 vWorldPosition;
	#else
		varying vec3 vReflect;
		uniform float refractionRatio;
	#endif
#endif`,jh=`#ifdef USE_ENVMAP
	#ifdef ENV_WORLDPOS
		vWorldPosition = worldPosition.xyz;
	#else
		vec3 cameraToVertex;
		if ( isOrthographic ) {
			cameraToVertex = normalize( vec3( - viewMatrix[ 0 ][ 2 ], - viewMatrix[ 1 ][ 2 ], - viewMatrix[ 2 ][ 2 ] ) );
		} else {
			cameraToVertex = normalize( worldPosition.xyz - cameraPosition );
		}
		vec3 worldNormal = transformNormalByInverseViewMatrix( transformedNormal, viewMatrix );
		#ifdef ENVMAP_MODE_REFLECTION
			vReflect = reflect( cameraToVertex, worldNormal );
		#else
			vReflect = refract( cameraToVertex, worldNormal, refractionRatio );
		#endif
	#endif
#endif`,ep=`#ifdef USE_FOG
	vFogDepth = - mvPosition.z;
#endif`,tp=`#ifdef USE_FOG
	varying float vFogDepth;
#endif`,np=`#ifdef USE_FOG
	#ifdef FOG_EXP2
		float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
	#else
		float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
	#endif
	gl_FragColor.rgb = mix( gl_FragColor.rgb, fogColor, fogFactor );
#endif`,ip=`#ifdef USE_FOG
	uniform vec3 fogColor;
	varying float vFogDepth;
	#ifdef FOG_EXP2
		uniform float fogDensity;
	#else
		uniform float fogNear;
		uniform float fogFar;
	#endif
#endif`,rp=`#ifdef USE_GRADIENTMAP
	uniform sampler2D gradientMap;
#endif
vec3 getGradientIrradiance( vec3 normal, vec3 lightDirection ) {
	float dotNL = dot( normal, lightDirection );
	vec2 coord = vec2( dotNL * 0.5 + 0.5, 0.0 );
	#ifdef USE_GRADIENTMAP
		return vec3( texture2D( gradientMap, coord ).r );
	#else
		vec2 fw = fwidth( coord ) * 0.5;
		return mix( vec3( 0.7 ), vec3( 1.0 ), smoothstep( 0.7 - fw.x, 0.7 + fw.x, coord.x ) );
	#endif
}`,sp=`#ifdef USE_LIGHTMAP
	uniform sampler2D lightMap;
	uniform float lightMapIntensity;
#endif`,ap=`LambertMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularStrength = specularStrength;`,op=`varying vec3 vViewPosition;
struct LambertMaterial {
	vec3 diffuseColor;
	float specularStrength;
};
void RE_Direct_Lambert( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
void RE_IndirectDiffuse_Lambert( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in LambertMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_Lambert
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Lambert`,lp=`uniform bool receiveShadow;
uniform vec3 ambientLightColor;
#if defined( USE_LIGHT_PROBES )
	uniform vec3 lightProbe[ 9 ];
#endif
vec3 shGetIrradianceAt( in vec3 normal, in vec3 shCoefficients[ 9 ] ) {
	float x = normal.x, y = normal.y, z = normal.z;
	vec3 result = shCoefficients[ 0 ] * 0.886227;
	result += shCoefficients[ 1 ] * 2.0 * 0.511664 * y;
	result += shCoefficients[ 2 ] * 2.0 * 0.511664 * z;
	result += shCoefficients[ 3 ] * 2.0 * 0.511664 * x;
	result += shCoefficients[ 4 ] * 2.0 * 0.429043 * x * y;
	result += shCoefficients[ 5 ] * 2.0 * 0.429043 * y * z;
	result += shCoefficients[ 6 ] * ( 0.743125 * z * z - 0.247708 );
	result += shCoefficients[ 7 ] * 2.0 * 0.429043 * x * z;
	result += shCoefficients[ 8 ] * 0.429043 * ( x * x - y * y );
	return result;
}
vec3 getLightProbeIrradiance( const in vec3 lightProbe[ 9 ], const in vec3 normal ) {
	vec3 worldNormal = transformNormalByInverseViewMatrix( normal, viewMatrix );
	vec3 irradiance = shGetIrradianceAt( worldNormal, lightProbe );
	return irradiance;
}
vec3 getAmbientLightIrradiance( const in vec3 ambientLightColor ) {
	vec3 irradiance = ambientLightColor;
	return irradiance;
}
float getDistanceAttenuation( const in float lightDistance, const in float cutoffDistance, const in float decayExponent ) {
	float distanceFalloff = 1.0 / max( pow( lightDistance, decayExponent ), 0.01 );
	if ( cutoffDistance > 0.0 ) {
		distanceFalloff *= pow2( saturate( 1.0 - pow4( lightDistance / cutoffDistance ) ) );
	}
	return distanceFalloff;
}
float getSpotAttenuation( const in float coneCosine, const in float penumbraCosine, const in float angleCosine ) {
	return smoothstep( coneCosine, penumbraCosine, angleCosine );
}
#if NUM_DIR_LIGHTS > 0
	struct DirectionalLight {
		vec3 direction;
		vec3 color;
	};
	uniform DirectionalLight directionalLights[ NUM_DIR_LIGHTS ];
	void getDirectionalLightInfo( const in DirectionalLight directionalLight, out IncidentLight light ) {
		light.color = directionalLight.color;
		light.direction = directionalLight.direction;
		light.visible = true;
	}
#endif
#if NUM_POINT_LIGHTS > 0
	struct PointLight {
		vec3 position;
		vec3 color;
		float distance;
		float decay;
	};
	uniform PointLight pointLights[ NUM_POINT_LIGHTS ];
	void getPointLightInfo( const in PointLight pointLight, const in vec3 geometryPosition, out IncidentLight light ) {
		vec3 lVector = pointLight.position - geometryPosition;
		light.direction = normalize( lVector );
		float lightDistance = length( lVector );
		light.color = pointLight.color;
		light.color *= getDistanceAttenuation( lightDistance, pointLight.distance, pointLight.decay );
		light.visible = ( light.color != vec3( 0.0 ) );
	}
#endif
#if NUM_SPOT_LIGHTS > 0
	struct SpotLight {
		vec3 position;
		vec3 direction;
		vec3 color;
		float distance;
		float decay;
		float coneCos;
		float penumbraCos;
	};
	uniform SpotLight spotLights[ NUM_SPOT_LIGHTS ];
	void getSpotLightInfo( const in SpotLight spotLight, const in vec3 geometryPosition, out IncidentLight light ) {
		vec3 lVector = spotLight.position - geometryPosition;
		light.direction = normalize( lVector );
		float angleCos = dot( light.direction, spotLight.direction );
		float spotAttenuation = getSpotAttenuation( spotLight.coneCos, spotLight.penumbraCos, angleCos );
		if ( spotAttenuation > 0.0 ) {
			float lightDistance = length( lVector );
			light.color = spotLight.color * spotAttenuation;
			light.color *= getDistanceAttenuation( lightDistance, spotLight.distance, spotLight.decay );
			light.visible = ( light.color != vec3( 0.0 ) );
		} else {
			light.color = vec3( 0.0 );
			light.visible = false;
		}
	}
#endif
#if NUM_RECT_AREA_LIGHTS > 0
	struct RectAreaLight {
		vec3 color;
		vec3 position;
		vec3 halfWidth;
		vec3 halfHeight;
	};
	uniform sampler2D ltc_1;	uniform sampler2D ltc_2;
	uniform RectAreaLight rectAreaLights[ NUM_RECT_AREA_LIGHTS ];
#endif
#if NUM_HEMI_LIGHTS > 0
	struct HemisphereLight {
		vec3 direction;
		vec3 skyColor;
		vec3 groundColor;
	};
	uniform HemisphereLight hemisphereLights[ NUM_HEMI_LIGHTS ];
	vec3 getHemisphereLightIrradiance( const in HemisphereLight hemiLight, const in vec3 normal ) {
		float dotNL = dot( normal, hemiLight.direction );
		float hemiDiffuseWeight = 0.5 * dotNL + 0.5;
		vec3 irradiance = mix( hemiLight.groundColor, hemiLight.skyColor, hemiDiffuseWeight );
		return irradiance;
	}
#endif
#include <lightprobes_pars_fragment>`,cp=`#ifdef USE_ENVMAP
	vec3 getIBLIrradiance( const in vec3 normal ) {
		#ifdef ENVMAP_TYPE_CUBE_UV
			vec3 worldNormal = transformNormalByInverseViewMatrix( normal, viewMatrix );
			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * worldNormal, 1.0 );
			return PI * envMapColor.rgb * envMapIntensity;
		#else
			return vec3( 0.0 );
		#endif
	}
	vec3 getIBLRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness ) {
		#ifdef ENVMAP_TYPE_CUBE_UV
			vec3 reflectVec = reflect( - viewDir, normal );
			reflectVec = normalize( mix( reflectVec, normal, pow4( roughness ) ) );
			reflectVec = transformDirectionByInverseViewMatrix( reflectVec, viewMatrix );
			vec4 envMapColor = textureCubeUV( envMap, envMapRotation * reflectVec, roughness );
			return envMapColor.rgb * envMapIntensity;
		#else
			return vec3( 0.0 );
		#endif
	}
	#ifdef USE_ANISOTROPY
		vec3 getIBLAnisotropyRadiance( const in vec3 viewDir, const in vec3 normal, const in float roughness, const in vec3 bitangent, const in float anisotropy ) {
			#ifdef ENVMAP_TYPE_CUBE_UV
				vec3 bentNormal = cross( bitangent, viewDir );
				bentNormal = normalize( cross( bentNormal, bitangent ) );
				bentNormal = normalize( mix( bentNormal, normal, pow2( pow2( 1.0 - anisotropy * ( 1.0 - roughness ) ) ) ) );
				return getIBLRadiance( viewDir, bentNormal, roughness );
			#else
				return vec3( 0.0 );
			#endif
		}
	#endif
#endif`,dp=`ToonMaterial material;
material.diffuseColor = diffuseColor.rgb;`,up=`varying vec3 vViewPosition;
struct ToonMaterial {
	vec3 diffuseColor;
};
void RE_Direct_Toon( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {
	vec3 irradiance = getGradientIrradiance( geometryNormal, directLight.direction ) * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
void RE_IndirectDiffuse_Toon( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in ToonMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_Toon
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Toon`,fp=`BlinnPhongMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularColor = specular;
material.specularShininess = shininess;
material.specularStrength = specularStrength;`,hp=`varying vec3 vViewPosition;
struct BlinnPhongMaterial {
	vec3 diffuseColor;
	vec3 specularColor;
	float specularShininess;
	float specularStrength;
};
void RE_Direct_BlinnPhong( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
	reflectedLight.directSpecular += irradiance * BRDF_BlinnPhong( directLight.direction, geometryViewDir, geometryNormal, material.specularColor, material.specularShininess ) * material.specularStrength;
}
void RE_IndirectDiffuse_BlinnPhong( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in BlinnPhongMaterial material, inout ReflectedLight reflectedLight ) {
	reflectedLight.indirectDiffuse += irradiance * BRDF_Lambert( material.diffuseColor );
}
#define RE_Direct				RE_Direct_BlinnPhong
#define RE_IndirectDiffuse		RE_IndirectDiffuse_BlinnPhong`,pp=`PhysicalMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.diffuseContribution = diffuseColor.rgb * ( 1.0 - metalnessFactor );
material.metalness = metalnessFactor;
vec3 dxy = max( abs( dFdx( nonPerturbedNormal ) ), abs( dFdy( nonPerturbedNormal ) ) );
float geometryRoughness = max( max( dxy.x, dxy.y ), dxy.z );
material.roughness = max( roughnessFactor, 0.0525 );material.roughness += geometryRoughness;
material.roughness = min( material.roughness, 1.0 );
#ifdef IOR
	material.ior = ior;
	#ifdef USE_SPECULAR
		float specularIntensityFactor = specularIntensity;
		vec3 specularColorFactor = specularColor;
		#ifdef USE_SPECULAR_COLORMAP
			specularColorFactor *= texture2D( specularColorMap, vSpecularColorMapUv ).rgb;
		#endif
		#ifdef USE_SPECULAR_INTENSITYMAP
			specularIntensityFactor *= texture2D( specularIntensityMap, vSpecularIntensityMapUv ).a;
		#endif
		material.specularF90 = mix( specularIntensityFactor, 1.0, metalnessFactor );
	#else
		float specularIntensityFactor = 1.0;
		vec3 specularColorFactor = vec3( 1.0 );
		material.specularF90 = 1.0;
	#endif
	material.specularColor = min( pow2( ( material.ior - 1.0 ) / ( material.ior + 1.0 ) ) * specularColorFactor, vec3( 1.0 ) ) * specularIntensityFactor;
	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );
#else
	material.specularColor = vec3( 0.04 );
	material.specularColorBlended = mix( material.specularColor, diffuseColor.rgb, metalnessFactor );
	material.specularF90 = 1.0;
#endif
#ifdef USE_CLEARCOAT
	material.clearcoat = clearcoat;
	material.clearcoatRoughness = clearcoatRoughness;
	material.clearcoatF0 = vec3( 0.04 );
	material.clearcoatF90 = 1.0;
	#ifdef USE_CLEARCOATMAP
		material.clearcoat *= texture2D( clearcoatMap, vClearcoatMapUv ).x;
	#endif
	#ifdef USE_CLEARCOAT_ROUGHNESSMAP
		material.clearcoatRoughness *= texture2D( clearcoatRoughnessMap, vClearcoatRoughnessMapUv ).y;
	#endif
	material.clearcoat = saturate( material.clearcoat );	material.clearcoatRoughness = max( material.clearcoatRoughness, 0.0525 );
	material.clearcoatRoughness += geometryRoughness;
	material.clearcoatRoughness = min( material.clearcoatRoughness, 1.0 );
#endif
#ifdef USE_DISPERSION
	material.dispersion = dispersion;
#endif
#ifdef USE_IRIDESCENCE
	material.iridescence = iridescence;
	material.iridescenceIOR = iridescenceIOR;
	#ifdef USE_IRIDESCENCEMAP
		material.iridescence *= texture2D( iridescenceMap, vIridescenceMapUv ).r;
	#endif
	#ifdef USE_IRIDESCENCE_THICKNESSMAP
		material.iridescenceThickness = (iridescenceThicknessMaximum - iridescenceThicknessMinimum) * texture2D( iridescenceThicknessMap, vIridescenceThicknessMapUv ).g + iridescenceThicknessMinimum;
	#else
		material.iridescenceThickness = iridescenceThicknessMaximum;
	#endif
#endif
#ifdef USE_SHEEN
	material.sheenColor = sheenColor;
	#ifdef USE_SHEEN_COLORMAP
		material.sheenColor *= texture2D( sheenColorMap, vSheenColorMapUv ).rgb;
	#endif
	material.sheenRoughness = clamp( sheenRoughness, 0.0001, 1.0 );
	#ifdef USE_SHEEN_ROUGHNESSMAP
		material.sheenRoughness *= texture2D( sheenRoughnessMap, vSheenRoughnessMapUv ).a;
	#endif
#endif
#ifdef USE_ANISOTROPY
	#ifdef USE_ANISOTROPYMAP
		mat2 anisotropyMat = mat2( anisotropyVector.x, anisotropyVector.y, - anisotropyVector.y, anisotropyVector.x );
		vec3 anisotropyPolar = texture2D( anisotropyMap, vAnisotropyMapUv ).rgb;
		vec2 anisotropyV = anisotropyMat * normalize( 2.0 * anisotropyPolar.rg - vec2( 1.0 ) ) * anisotropyPolar.b;
	#else
		vec2 anisotropyV = anisotropyVector;
	#endif
	material.anisotropy = length( anisotropyV );
	if( material.anisotropy == 0.0 ) {
		anisotropyV = vec2( 1.0, 0.0 );
	} else {
		anisotropyV /= material.anisotropy;
		material.anisotropy = saturate( material.anisotropy );
	}
	material.alphaT = mix( pow2( material.roughness ), 1.0, pow2( material.anisotropy ) );
	material.anisotropyT = tbn[ 0 ] * anisotropyV.x + tbn[ 1 ] * anisotropyV.y;
	material.anisotropyB = tbn[ 1 ] * anisotropyV.x - tbn[ 0 ] * anisotropyV.y;
#endif`,mp=`uniform sampler2D dfgLUT;
struct PhysicalMaterial {
	vec3 diffuseColor;
	vec3 diffuseContribution;
	vec3 specularColor;
	vec3 specularColorBlended;
	float roughness;
	float metalness;
	float specularF90;
	float dispersion;
	#ifdef USE_CLEARCOAT
		float clearcoat;
		float clearcoatRoughness;
		vec3 clearcoatF0;
		float clearcoatF90;
	#endif
	#ifdef USE_IRIDESCENCE
		float iridescence;
		float iridescenceIOR;
		float iridescenceThickness;
		vec3 iridescenceFresnel;
		vec3 iridescenceF0;
		vec3 iridescenceFresnelDielectric;
		vec3 iridescenceFresnelMetallic;
	#endif
	#ifdef USE_SHEEN
		vec3 sheenColor;
		float sheenRoughness;
	#endif
	#ifdef IOR
		float ior;
	#endif
	#ifdef USE_TRANSMISSION
		float transmission;
		float transmissionAlpha;
		float thickness;
		float attenuationDistance;
		vec3 attenuationColor;
	#endif
	#ifdef USE_ANISOTROPY
		float anisotropy;
		float alphaT;
		vec3 anisotropyT;
		vec3 anisotropyB;
	#endif
};
vec3 clearcoatSpecularDirect = vec3( 0.0 );
vec3 clearcoatSpecularIndirect = vec3( 0.0 );
vec3 sheenSpecularDirect = vec3( 0.0 );
vec3 sheenSpecularIndirect = vec3(0.0 );
vec3 Schlick_to_F0( const in vec3 f, const in float f90, const in float dotVH ) {
    float x = clamp( 1.0 - dotVH, 0.0, 1.0 );
    float x2 = x * x;
    float x5 = clamp( x * x2 * x2, 0.0, 0.9999 );
    return ( f - vec3( f90 ) * x5 ) / ( 1.0 - x5 );
}
float V_GGX_SmithCorrelated( const in float alpha, const in float dotNL, const in float dotNV ) {
	float a2 = pow2( alpha );
	float gv = dotNL * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNV ) );
	float gl = dotNV * sqrt( a2 + ( 1.0 - a2 ) * pow2( dotNL ) );
	return 0.5 / max( gv + gl, EPSILON );
}
float D_GGX( const in float alpha, const in float dotNH ) {
	float a2 = pow2( alpha );
	float denom = pow2( dotNH ) * ( a2 - 1.0 ) + 1.0;
	return RECIPROCAL_PI * a2 / pow2( denom );
}
#ifdef USE_ANISOTROPY
	float V_GGX_SmithCorrelated_Anisotropic( const in float alphaT, const in float alphaB, const in float dotTV, const in float dotBV, const in float dotTL, const in float dotBL, const in float dotNV, const in float dotNL ) {
		float gv = dotNL * length( vec3( alphaT * dotTV, alphaB * dotBV, dotNV ) );
		float gl = dotNV * length( vec3( alphaT * dotTL, alphaB * dotBL, dotNL ) );
		return 0.5 / max( gv + gl, EPSILON );
	}
	float D_GGX_Anisotropic( const in float alphaT, const in float alphaB, const in float dotNH, const in float dotTH, const in float dotBH ) {
		float a2 = alphaT * alphaB;
		highp vec3 v = vec3( alphaB * dotTH, alphaT * dotBH, a2 * dotNH );
		highp float v2 = dot( v, v );
		float w2 = a2 / v2;
		return RECIPROCAL_PI * a2 * pow2 ( w2 );
	}
#endif
#ifdef USE_CLEARCOAT
	vec3 BRDF_GGX_Clearcoat( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material) {
		vec3 f0 = material.clearcoatF0;
		float f90 = material.clearcoatF90;
		float roughness = material.clearcoatRoughness;
		float alpha = pow2( roughness );
		vec3 halfDir = normalize( lightDir + viewDir );
		float dotNL = saturate( dot( normal, lightDir ) );
		float dotNV = saturate( dot( normal, viewDir ) );
		float dotNH = saturate( dot( normal, halfDir ) );
		float dotVH = saturate( dot( viewDir, halfDir ) );
		vec3 F = F_Schlick( f0, f90, dotVH );
		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );
		float D = D_GGX( alpha, dotNH );
		return F * ( V * D );
	}
#endif
vec3 BRDF_GGX( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {
	vec3 f0 = material.specularColorBlended;
	float f90 = material.specularF90;
	float roughness = material.roughness;
	float alpha = pow2( roughness );
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	float dotNH = saturate( dot( normal, halfDir ) );
	float dotVH = saturate( dot( viewDir, halfDir ) );
	vec3 F = F_Schlick( f0, f90, dotVH );
	#ifdef USE_IRIDESCENCE
		F = mix( F, material.iridescenceFresnel, material.iridescence );
	#endif
	#ifdef USE_ANISOTROPY
		float dotTL = dot( material.anisotropyT, lightDir );
		float dotTV = dot( material.anisotropyT, viewDir );
		float dotTH = dot( material.anisotropyT, halfDir );
		float dotBL = dot( material.anisotropyB, lightDir );
		float dotBV = dot( material.anisotropyB, viewDir );
		float dotBH = dot( material.anisotropyB, halfDir );
		float V = V_GGX_SmithCorrelated_Anisotropic( material.alphaT, alpha, dotTV, dotBV, dotTL, dotBL, dotNV, dotNL );
		float D = D_GGX_Anisotropic( material.alphaT, alpha, dotNH, dotTH, dotBH );
	#else
		float V = V_GGX_SmithCorrelated( alpha, dotNL, dotNV );
		float D = D_GGX( alpha, dotNH );
	#endif
	return F * ( V * D );
}
vec2 LTC_Uv( const in vec3 N, const in vec3 V, const in float roughness ) {
	const float LUT_SIZE = 64.0;
	const float LUT_SCALE = ( LUT_SIZE - 1.0 ) / LUT_SIZE;
	const float LUT_BIAS = 0.5 / LUT_SIZE;
	float dotNV = saturate( dot( N, V ) );
	vec2 uv = vec2( roughness, sqrt( 1.0 - dotNV ) );
	uv = uv * LUT_SCALE + LUT_BIAS;
	return uv;
}
float LTC_ClippedSphereFormFactor( const in vec3 f ) {
	float l = length( f );
	return max( ( l * l + f.z ) / ( l + 1.0 ), 0.0 );
}
vec3 LTC_EdgeVectorFormFactor( const in vec3 v1, const in vec3 v2 ) {
	float x = dot( v1, v2 );
	float y = abs( x );
	float a = 0.8543985 + ( 0.4965155 + 0.0145206 * y ) * y;
	float b = 3.4175940 + ( 4.1616724 + y ) * y;
	float v = a / b;
	float theta_sintheta = ( x > 0.0 ) ? v : 0.5 * inversesqrt( max( 1.0 - x * x, 1e-7 ) ) - v;
	return cross( v1, v2 ) * theta_sintheta;
}
vec3 LTC_Evaluate( const in vec3 N, const in vec3 V, const in vec3 P, const in mat3 mInv, const in vec3 rectCoords[ 4 ] ) {
	vec3 v1 = rectCoords[ 1 ] - rectCoords[ 0 ];
	vec3 v2 = rectCoords[ 3 ] - rectCoords[ 0 ];
	vec3 lightNormal = cross( v1, v2 );
	if( dot( lightNormal, P - rectCoords[ 0 ] ) < 0.0 ) return vec3( 0.0 );
	vec3 T1, T2;
	T1 = normalize( V - N * dot( V, N ) );
	T2 = - cross( N, T1 );
	mat3 mat = mInv * transpose( mat3( T1, T2, N ) );
	vec3 coords[ 4 ];
	coords[ 0 ] = mat * ( rectCoords[ 0 ] - P );
	coords[ 1 ] = mat * ( rectCoords[ 1 ] - P );
	coords[ 2 ] = mat * ( rectCoords[ 2 ] - P );
	coords[ 3 ] = mat * ( rectCoords[ 3 ] - P );
	coords[ 0 ] = normalize( coords[ 0 ] );
	coords[ 1 ] = normalize( coords[ 1 ] );
	coords[ 2 ] = normalize( coords[ 2 ] );
	coords[ 3 ] = normalize( coords[ 3 ] );
	vec3 vectorFormFactor = vec3( 0.0 );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 0 ], coords[ 1 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 1 ], coords[ 2 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 2 ], coords[ 3 ] );
	vectorFormFactor += LTC_EdgeVectorFormFactor( coords[ 3 ], coords[ 0 ] );
	float result = LTC_ClippedSphereFormFactor( vectorFormFactor );
	return vec3( result );
}
#if defined( USE_SHEEN )
float D_Charlie( float roughness, float dotNH ) {
	float alpha = pow2( roughness );
	float invAlpha = 1.0 / alpha;
	float cos2h = dotNH * dotNH;
	float sin2h = max( 1.0 - cos2h, 0.0078125 );
	return ( 2.0 + invAlpha ) * pow( sin2h, invAlpha * 0.5 ) / ( 2.0 * PI );
}
float V_Neubelt( float dotNV, float dotNL ) {
	return saturate( 1.0 / ( 4.0 * ( dotNL + dotNV - dotNL * dotNV ) ) );
}
vec3 BRDF_Sheen( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, vec3 sheenColor, const in float sheenRoughness ) {
	vec3 halfDir = normalize( lightDir + viewDir );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	float dotNH = saturate( dot( normal, halfDir ) );
	float D = D_Charlie( sheenRoughness, dotNH );
	float V = V_Neubelt( dotNV, dotNL );
	return sheenColor * ( D * V );
}
#endif
float IBLSheenBRDF( const in vec3 normal, const in vec3 viewDir, const in float roughness ) {
	float dotNV = saturate( dot( normal, viewDir ) );
	float r2 = roughness * roughness;
	float rInv = 1.0 / ( roughness + 0.1 );
	float a = -1.9362 + 1.0678 * roughness + 0.4573 * r2 - 0.8469 * rInv;
	float b = -0.6014 + 0.5538 * roughness - 0.4670 * r2 - 0.1255 * rInv;
	float DG = exp( a * dotNV + b );
	return saturate( DG );
}
vec3 EnvironmentBRDF( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness ) {
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;
	return specularColor * fab.x + specularF90 * fab.y;
}
#ifdef USE_IRIDESCENCE
void computeMultiscatteringIridescence( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float iridescence, const in vec3 iridescenceF0, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {
#else
void computeMultiscattering( const in vec3 normal, const in vec3 viewDir, const in vec3 specularColor, const in float specularF90, const in float roughness, inout vec3 singleScatter, inout vec3 multiScatter ) {
#endif
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 fab = texture2D( dfgLUT, vec2( roughness, dotNV ) ).rg;
	#ifdef USE_IRIDESCENCE
		vec3 Fr = mix( specularColor, iridescenceF0, iridescence );
	#else
		vec3 Fr = specularColor;
	#endif
	vec3 FssEss = Fr * fab.x + specularF90 * fab.y;
	float Ess = fab.x + fab.y;
	float Ems = 1.0 - Ess;
	vec3 Favg = Fr + ( 1.0 - Fr ) * 0.047619;	vec3 Fms = FssEss * Favg / ( 1.0 - Ems * Favg );
	singleScatter += FssEss;
	multiScatter += Fms * Ems;
}
vec3 BRDF_GGX_Multiscatter( const in vec3 lightDir, const in vec3 viewDir, const in vec3 normal, const in PhysicalMaterial material ) {
	vec3 singleScatter = BRDF_GGX( lightDir, viewDir, normal, material );
	float dotNL = saturate( dot( normal, lightDir ) );
	float dotNV = saturate( dot( normal, viewDir ) );
	vec2 dfgV = texture2D( dfgLUT, vec2( material.roughness, dotNV ) ).rg;
	vec2 dfgL = texture2D( dfgLUT, vec2( material.roughness, dotNL ) ).rg;
	vec3 FssEss_V = material.specularColorBlended * dfgV.x + material.specularF90 * dfgV.y;
	vec3 FssEss_L = material.specularColorBlended * dfgL.x + material.specularF90 * dfgL.y;
	float Ess_V = dfgV.x + dfgV.y;
	float Ess_L = dfgL.x + dfgL.y;
	float Ems_V = 1.0 - Ess_V;
	float Ems_L = 1.0 - Ess_L;
	vec3 Favg = material.specularColorBlended + ( 1.0 - material.specularColorBlended ) * 0.047619;
	vec3 Fms = FssEss_V * FssEss_L * Favg / ( 1.0 - Ems_V * Ems_L * Favg + EPSILON );
	float compensationFactor = Ems_V * Ems_L;
	vec3 multiScatter = Fms * compensationFactor;
	return singleScatter + multiScatter;
}
#if NUM_RECT_AREA_LIGHTS > 0
	void RE_Direct_RectArea_Physical( const in RectAreaLight rectAreaLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
		vec3 normal = geometryNormal;
		vec3 viewDir = geometryViewDir;
		vec3 position = geometryPosition;
		vec3 lightPos = rectAreaLight.position;
		vec3 halfWidth = rectAreaLight.halfWidth;
		vec3 halfHeight = rectAreaLight.halfHeight;
		vec3 lightColor = rectAreaLight.color;
		float roughness = material.roughness;
		vec3 rectCoords[ 4 ];
		rectCoords[ 0 ] = lightPos + halfWidth - halfHeight;		rectCoords[ 1 ] = lightPos - halfWidth - halfHeight;
		rectCoords[ 2 ] = lightPos - halfWidth + halfHeight;
		rectCoords[ 3 ] = lightPos + halfWidth + halfHeight;
		vec2 uv = LTC_Uv( normal, viewDir, roughness );
		vec4 t1 = texture2D( ltc_1, uv );
		vec4 t2 = texture2D( ltc_2, uv );
		mat3 mInv = mat3(
			vec3( t1.x, 0, t1.y ),
			vec3(    0, 1,    0 ),
			vec3( t1.z, 0, t1.w )
		);
		vec3 fresnel = ( material.specularColorBlended * t2.x + ( material.specularF90 - material.specularColorBlended ) * t2.y );
		reflectedLight.directSpecular += lightColor * fresnel * LTC_Evaluate( normal, viewDir, position, mInv, rectCoords );
		reflectedLight.directDiffuse += lightColor * material.diffuseContribution * LTC_Evaluate( normal, viewDir, position, mat3( 1.0 ), rectCoords );
		#ifdef USE_CLEARCOAT
			vec3 Ncc = geometryClearcoatNormal;
			vec2 uvClearcoat = LTC_Uv( Ncc, viewDir, material.clearcoatRoughness );
			vec4 t1Clearcoat = texture2D( ltc_1, uvClearcoat );
			vec4 t2Clearcoat = texture2D( ltc_2, uvClearcoat );
			mat3 mInvClearcoat = mat3(
				vec3( t1Clearcoat.x, 0, t1Clearcoat.y ),
				vec3(             0, 1,             0 ),
				vec3( t1Clearcoat.z, 0, t1Clearcoat.w )
			);
			vec3 fresnelClearcoat = material.clearcoatF0 * t2Clearcoat.x + ( material.clearcoatF90 - material.clearcoatF0 ) * t2Clearcoat.y;
			clearcoatSpecularDirect += lightColor * fresnelClearcoat * LTC_Evaluate( Ncc, viewDir, position, mInvClearcoat, rectCoords );
		#endif
	}
#endif
void RE_Direct_Physical( const in IncidentLight directLight, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
	float dotNL = saturate( dot( geometryNormal, directLight.direction ) );
	vec3 irradiance = dotNL * directLight.color;
	#ifdef USE_CLEARCOAT
		float dotNLcc = saturate( dot( geometryClearcoatNormal, directLight.direction ) );
		vec3 ccIrradiance = dotNLcc * directLight.color;
		clearcoatSpecularDirect += ccIrradiance * BRDF_GGX_Clearcoat( directLight.direction, geometryViewDir, geometryClearcoatNormal, material );
	#endif
	#ifdef USE_SHEEN
 
 		sheenSpecularDirect += irradiance * BRDF_Sheen( directLight.direction, geometryViewDir, geometryNormal, material.sheenColor, material.sheenRoughness );
 
 		float sheenAlbedoV = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
 		float sheenAlbedoL = IBLSheenBRDF( geometryNormal, directLight.direction, material.sheenRoughness );
 
 		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * max( sheenAlbedoV, sheenAlbedoL );
 
 		irradiance *= sheenEnergyComp;
 
 	#endif
	reflectedLight.directSpecular += irradiance * BRDF_GGX_Multiscatter( directLight.direction, geometryViewDir, geometryNormal, material );
	reflectedLight.directDiffuse += irradiance * BRDF_Lambert( material.diffuseContribution );
}
void RE_IndirectDiffuse_Physical( const in vec3 irradiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight ) {
	vec3 diffuse = irradiance * BRDF_Lambert( material.diffuseContribution );
	#ifdef USE_SHEEN
		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;
		diffuse *= sheenEnergyComp;
	#endif
	reflectedLight.indirectDiffuse += diffuse;
}
void RE_IndirectSpecular_Physical( const in vec3 radiance, const in vec3 irradiance, const in vec3 clearcoatRadiance, const in vec3 geometryPosition, const in vec3 geometryNormal, const in vec3 geometryViewDir, const in vec3 geometryClearcoatNormal, const in PhysicalMaterial material, inout ReflectedLight reflectedLight) {
	#ifdef USE_CLEARCOAT
		clearcoatSpecularIndirect += clearcoatRadiance * EnvironmentBRDF( geometryClearcoatNormal, geometryViewDir, material.clearcoatF0, material.clearcoatF90, material.clearcoatRoughness );
	#endif
	#ifdef USE_SHEEN
		sheenSpecularIndirect += irradiance * material.sheenColor * IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness ) * RECIPROCAL_PI;
 	#endif
	vec3 singleScatteringDielectric = vec3( 0.0 );
	vec3 multiScatteringDielectric = vec3( 0.0 );
	vec3 singleScatteringMetallic = vec3( 0.0 );
	vec3 multiScatteringMetallic = vec3( 0.0 );
	#ifdef USE_IRIDESCENCE
		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.iridescence, material.iridescenceFresnelDielectric, material.roughness, singleScatteringDielectric, multiScatteringDielectric );
		computeMultiscatteringIridescence( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.iridescence, material.iridescenceFresnelMetallic, material.roughness, singleScatteringMetallic, multiScatteringMetallic );
	#else
		computeMultiscattering( geometryNormal, geometryViewDir, material.specularColor, material.specularF90, material.roughness, singleScatteringDielectric, multiScatteringDielectric );
		computeMultiscattering( geometryNormal, geometryViewDir, material.diffuseColor, material.specularF90, material.roughness, singleScatteringMetallic, multiScatteringMetallic );
	#endif
	vec3 singleScattering = mix( singleScatteringDielectric, singleScatteringMetallic, material.metalness );
	vec3 multiScattering = mix( multiScatteringDielectric, multiScatteringMetallic, material.metalness );
	vec3 totalScatteringDielectric = singleScatteringDielectric + multiScatteringDielectric;
	vec3 diffuse = material.diffuseContribution * ( 1.0 - totalScatteringDielectric );
	vec3 cosineWeightedIrradiance = irradiance * RECIPROCAL_PI;
	vec3 indirectSpecular = radiance * singleScattering;
	indirectSpecular += multiScattering * cosineWeightedIrradiance;
	vec3 indirectDiffuse = diffuse * cosineWeightedIrradiance;
	#ifdef USE_SHEEN
		float sheenAlbedo = IBLSheenBRDF( geometryNormal, geometryViewDir, material.sheenRoughness );
		float sheenEnergyComp = 1.0 - max3( material.sheenColor ) * sheenAlbedo;
		indirectSpecular *= sheenEnergyComp;
		indirectDiffuse *= sheenEnergyComp;
	#endif
	reflectedLight.indirectSpecular += indirectSpecular;
	reflectedLight.indirectDiffuse += indirectDiffuse;
}
#define RE_Direct				RE_Direct_Physical
#define RE_Direct_RectArea		RE_Direct_RectArea_Physical
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Physical
#define RE_IndirectSpecular		RE_IndirectSpecular_Physical
float computeSpecularOcclusion( const in float dotNV, const in float ambientOcclusion, const in float roughness ) {
	return saturate( pow( dotNV + ambientOcclusion, exp2( - 16.0 * roughness - 1.0 ) ) - 1.0 + ambientOcclusion );
}`,gp=`
vec3 geometryPosition = - vViewPosition;
vec3 geometryNormal = normal;
vec3 geometryViewDir = ( isOrthographic ) ? vec3( 0, 0, 1 ) : normalize( vViewPosition );
vec3 geometryClearcoatNormal = vec3( 0.0 );
#ifdef USE_CLEARCOAT
	geometryClearcoatNormal = clearcoatNormal;
#endif
#ifdef USE_IRIDESCENCE
	float dotNVi = saturate( dot( normal, geometryViewDir ) );
	if ( material.iridescenceThickness == 0.0 ) {
		material.iridescence = 0.0;
	} else {
		material.iridescence = saturate( material.iridescence );
	}
	if ( material.iridescence > 0.0 ) {
		material.iridescenceFresnelDielectric = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.specularColor );
		material.iridescenceFresnelMetallic = evalIridescence( 1.0, material.iridescenceIOR, dotNVi, material.iridescenceThickness, material.diffuseColor );
		material.iridescenceFresnel = mix( material.iridescenceFresnelDielectric, material.iridescenceFresnelMetallic, material.metalness );
		material.iridescenceF0 = Schlick_to_F0( material.iridescenceFresnel, 1.0, dotNVi );
	}
#endif
IncidentLight directLight;
#if ( NUM_POINT_LIGHTS > 0 ) && defined( RE_Direct )
	PointLight pointLight;
	#if defined( USE_SHADOWMAP ) && NUM_POINT_LIGHT_SHADOWS > 0
	PointLightShadow pointLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_POINT_LIGHTS; i ++ ) {
		pointLight = pointLights[ i ];
		getPointLightInfo( pointLight, geometryPosition, directLight );
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_POINT_LIGHT_SHADOWS ) && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )
		pointLightShadow = pointLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getPointShadow( pointShadowMap[ i ], pointLightShadow.shadowMapSize, pointLightShadow.shadowIntensity, pointLightShadow.shadowBias, pointLightShadow.shadowRadius, vPointShadowCoord[ i ], pointLightShadow.shadowCameraNear, pointLightShadow.shadowCameraFar ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_SPOT_LIGHTS > 0 ) && defined( RE_Direct )
	SpotLight spotLight;
	vec4 spotColor;
	vec3 spotLightCoord;
	bool inSpotLightMap;
	#if defined( USE_SHADOWMAP ) && NUM_SPOT_LIGHT_SHADOWS > 0
	SpotLightShadow spotLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHTS; i ++ ) {
		spotLight = spotLights[ i ];
		getSpotLightInfo( spotLight, geometryPosition, directLight );
		#if ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )
		#define SPOT_LIGHT_MAP_INDEX UNROLLED_LOOP_INDEX
		#elif ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
		#define SPOT_LIGHT_MAP_INDEX NUM_SPOT_LIGHT_MAPS
		#else
		#define SPOT_LIGHT_MAP_INDEX ( UNROLLED_LOOP_INDEX - NUM_SPOT_LIGHT_SHADOWS + NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS )
		#endif
		#if ( SPOT_LIGHT_MAP_INDEX < NUM_SPOT_LIGHT_MAPS )
			spotLightCoord = vSpotLightCoord[ i ].xyz / vSpotLightCoord[ i ].w;
			inSpotLightMap = all( lessThan( abs( spotLightCoord * 2. - 1. ), vec3( 1.0 ) ) );
			spotColor = texture2D( spotLightMap[ SPOT_LIGHT_MAP_INDEX ], spotLightCoord.xy );
			directLight.color = inSpotLightMap ? directLight.color * spotColor.rgb : directLight.color;
		#endif
		#undef SPOT_LIGHT_MAP_INDEX
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
		spotLightShadow = spotLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( spotShadowMap[ i ], spotLightShadow.shadowMapSize, spotLightShadow.shadowIntensity, spotLightShadow.shadowBias, spotLightShadow.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_DIR_LIGHTS > 0 ) && defined( RE_Direct )
	DirectionalLight directionalLight;
	#if defined( USE_SHADOWMAP ) && NUM_DIR_LIGHT_SHADOWS > 0
	DirectionalLightShadow directionalLightShadow;
	#endif
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_DIR_LIGHTS; i ++ ) {
		directionalLight = directionalLights[ i ];
		getDirectionalLightInfo( directionalLight, directLight );
		#if defined( USE_SHADOWMAP ) && ( UNROLLED_LOOP_INDEX < NUM_DIR_LIGHT_SHADOWS )
		directionalLightShadow = directionalLightShadows[ i ];
		directLight.color *= ( directLight.visible && receiveShadow ) ? getShadow( directionalShadowMap[ i ], directionalLightShadow.shadowMapSize, directionalLightShadow.shadowIntensity, directionalLightShadow.shadowBias, directionalLightShadow.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;
		#endif
		RE_Direct( directLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if ( NUM_RECT_AREA_LIGHTS > 0 ) && defined( RE_Direct_RectArea )
	RectAreaLight rectAreaLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_RECT_AREA_LIGHTS; i ++ ) {
		rectAreaLight = rectAreaLights[ i ];
		RE_Direct_RectArea( rectAreaLight, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
	}
	#pragma unroll_loop_end
#endif
#if defined( RE_IndirectDiffuse )
	vec3 iblIrradiance = vec3( 0.0 );
	vec3 irradiance = getAmbientLightIrradiance( ambientLightColor );
	#if defined( USE_LIGHT_PROBES )
		irradiance += getLightProbeIrradiance( lightProbe, geometryNormal );
	#endif
	#if ( NUM_HEMI_LIGHTS > 0 )
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_HEMI_LIGHTS; i ++ ) {
			irradiance += getHemisphereLightIrradiance( hemisphereLights[ i ], geometryNormal );
		}
		#pragma unroll_loop_end
	#endif
	#ifdef USE_LIGHT_PROBES_GRID
		vec3 probeWorldPos = ( ( vec4( geometryPosition, 1.0 ) - viewMatrix[ 3 ] ) * viewMatrix ).xyz;
		vec3 probeWorldNormal = transformNormalByInverseViewMatrix( geometryNormal, viewMatrix );
		irradiance += getLightProbeGridIrradiance( probeWorldPos, probeWorldNormal );
	#endif
#endif
#if defined( RE_IndirectSpecular )
	vec3 radiance = vec3( 0.0 );
	vec3 clearcoatRadiance = vec3( 0.0 );
#endif`,_p=`#if defined( RE_IndirectDiffuse )
	#ifdef USE_LIGHTMAP
		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );
		vec3 lightMapIrradiance = lightMapTexel.rgb * lightMapIntensity;
		irradiance += lightMapIrradiance;
	#endif
	#if defined( USE_ENVMAP ) && defined( ENVMAP_TYPE_CUBE_UV )
		#if defined( STANDARD ) || defined( LAMBERT ) || defined( PHONG )
			iblIrradiance += getIBLIrradiance( geometryNormal );
		#endif
	#endif
#endif
#if defined( USE_ENVMAP ) && defined( RE_IndirectSpecular )
	#ifdef USE_ANISOTROPY
		radiance += getIBLAnisotropyRadiance( geometryViewDir, geometryNormal, material.roughness, material.anisotropyB, material.anisotropy );
	#else
		radiance += getIBLRadiance( geometryViewDir, geometryNormal, material.roughness );
	#endif
	#ifdef USE_CLEARCOAT
		clearcoatRadiance += getIBLRadiance( geometryViewDir, geometryClearcoatNormal, material.clearcoatRoughness );
	#endif
#endif`,vp=`#if defined( RE_IndirectDiffuse )
	#if defined( LAMBERT ) || defined( PHONG )
		irradiance += iblIrradiance;
	#endif
	RE_IndirectDiffuse( irradiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif
#if defined( RE_IndirectSpecular )
	RE_IndirectSpecular( radiance, iblIrradiance, clearcoatRadiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif`,xp=`#ifdef USE_LIGHT_PROBES_GRID
uniform highp sampler3D probesSH;
uniform vec3 probesMin;
uniform vec3 probesMax;
uniform vec3 probesResolution;
vec3 getLightProbeGridIrradiance( vec3 worldPos, vec3 worldNormal ) {
	vec3 res = probesResolution;
	vec3 gridRange = probesMax - probesMin;
	vec3 resMinusOne = res - 1.0;
	vec3 probeSpacing = gridRange / resMinusOne;
	vec3 samplePos = worldPos + worldNormal * probeSpacing * 0.5;
	vec3 uvw = clamp( ( samplePos - probesMin ) / gridRange, 0.0, 1.0 );
	uvw = uvw * resMinusOne / res + 0.5 / res;
	float nz          = res.z;
	float paddedSlices = nz + 2.0;
	float atlasDepth  = 7.0 * paddedSlices;
	float uvZBase     = uvw.z * nz + 1.0;
	vec4 s0 = texture( probesSH, vec3( uvw.xy, ( uvZBase                       ) / atlasDepth ) );
	vec4 s1 = texture( probesSH, vec3( uvw.xy, ( uvZBase +       paddedSlices   ) / atlasDepth ) );
	vec4 s2 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 2.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s3 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 3.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s4 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 4.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s5 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 5.0 * paddedSlices   ) / atlasDepth ) );
	vec4 s6 = texture( probesSH, vec3( uvw.xy, ( uvZBase + 6.0 * paddedSlices   ) / atlasDepth ) );
	vec3 c0 = s0.xyz;
	vec3 c1 = vec3( s0.w, s1.xy );
	vec3 c2 = vec3( s1.zw, s2.x );
	vec3 c3 = s2.yzw;
	vec3 c4 = s3.xyz;
	vec3 c5 = vec3( s3.w, s4.xy );
	vec3 c6 = vec3( s4.zw, s5.x );
	vec3 c7 = s5.yzw;
	vec3 c8 = s6.xyz;
	float x = worldNormal.x, y = worldNormal.y, z = worldNormal.z;
	vec3 result = c0 * 0.886227;
	result += c1 * 2.0 * 0.511664 * y;
	result += c2 * 2.0 * 0.511664 * z;
	result += c3 * 2.0 * 0.511664 * x;
	result += c4 * 2.0 * 0.429043 * x * y;
	result += c5 * 2.0 * 0.429043 * y * z;
	result += c6 * ( 0.743125 * z * z - 0.247708 );
	result += c7 * 2.0 * 0.429043 * x * z;
	result += c8 * 0.429043 * ( x * x - y * y );
	return max( result, vec3( 0.0 ) );
}
#endif`,yp=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	gl_FragDepth = vIsPerspective == 0.0 ? gl_FragCoord.z : log2( vFragDepth ) * logDepthBufFC * 0.5;
#endif`,bp=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	uniform float logDepthBufFC;
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,Sp=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,Mp=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	vFragDepth = 1.0 + gl_Position.w;
	vIsPerspective = float( isPerspectiveMatrix( projectionMatrix ) );
#endif`,Ep=`#ifdef USE_MAP
	vec4 sampledDiffuseColor = texture2D( map, vMapUv );
	#ifdef DECODE_VIDEO_TEXTURE
		sampledDiffuseColor = sRGBTransferEOTF( sampledDiffuseColor );
	#endif
	diffuseColor *= sampledDiffuseColor;
#endif`,wp=`#ifdef USE_MAP
	uniform sampler2D map;
#endif`,Tp=`#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
	#if defined( USE_POINTS_UV )
		vec2 uv = vUv;
	#else
		vec2 uv = ( uvTransform * vec3( gl_PointCoord.x, 1.0 - gl_PointCoord.y, 1 ) ).xy;
	#endif
#endif
#ifdef USE_MAP
	diffuseColor *= texture2D( map, uv );
#endif
#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, uv ).g;
#endif`,Ap=`#if defined( USE_POINTS_UV )
	varying vec2 vUv;
#else
	#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
		uniform mat3 uvTransform;
	#endif
#endif
#ifdef USE_MAP
	uniform sampler2D map;
#endif
#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,Rp=`float metalnessFactor = metalness;
#ifdef USE_METALNESSMAP
	vec4 texelMetalness = texture2D( metalnessMap, vMetalnessMapUv );
	metalnessFactor *= texelMetalness.b;
#endif`,Cp=`#ifdef USE_METALNESSMAP
	uniform sampler2D metalnessMap;
#endif`,Pp=`#ifdef USE_INSTANCING_MORPH
	float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	float morphTargetBaseInfluence = texelFetch( morphTexture, ivec2( 0, gl_InstanceID ), 0 ).r;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		morphTargetInfluences[i] =  texelFetch( morphTexture, ivec2( i + 1, gl_InstanceID ), 0 ).r;
	}
#endif`,Lp=`#if defined( USE_MORPHCOLORS )
	vColor *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		#if defined( USE_COLOR_ALPHA )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ) * morphTargetInfluences[ i ];
		#elif defined( USE_COLOR )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ).rgb * morphTargetInfluences[ i ];
		#endif
	}
#endif`,Ip=`#ifdef USE_MORPHNORMALS
	objectNormal *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) objectNormal += getMorph( gl_VertexID, i, 1 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,Dp=`#ifdef USE_MORPHTARGETS
	#ifndef USE_INSTANCING_MORPH
		uniform float morphTargetBaseInfluence;
		uniform float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	#endif
	uniform sampler2DArray morphTargetsTexture;
	uniform ivec2 morphTargetsTextureSize;
	vec4 getMorph( const in int vertexIndex, const in int morphTargetIndex, const in int offset ) {
		int texelIndex = vertexIndex * MORPHTARGETS_TEXTURE_STRIDE + offset;
		int y = texelIndex / morphTargetsTextureSize.x;
		int x = texelIndex - y * morphTargetsTextureSize.x;
		ivec3 morphUV = ivec3( x, y, morphTargetIndex );
		return texelFetch( morphTargetsTexture, morphUV, 0 );
	}
#endif`,Np=`#ifdef USE_MORPHTARGETS
	transformed *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) transformed += getMorph( gl_VertexID, i, 0 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,Up=`float faceDirection = gl_FrontFacing ? 1.0 : - 1.0;
#ifdef FLAT_SHADED
	vec3 fdx = dFdx( vViewPosition );
	vec3 fdy = dFdy( vViewPosition );
	vec3 normal = normalize( cross( fdx, fdy ) );
#else
	vec3 normal = normalize( vNormal );
	#ifdef DOUBLE_SIDED
		normal *= faceDirection;
	#endif
#endif
#if defined( USE_NORMALMAP_TANGENTSPACE ) || defined( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY )
	#ifdef USE_TANGENT
		mat3 tbn = mat3( normalize( vTangent ), normalize( vBitangent ), normal );
	#else
		mat3 tbn = getTangentFrame( - vViewPosition, normal,
		#if defined( USE_NORMALMAP )
			vNormalMapUv
		#elif defined( USE_CLEARCOAT_NORMALMAP )
			vClearcoatNormalMapUv
		#else
			vUv
		#endif
		);
	#endif
	#ifdef DOUBLE_SIDED
		tbn[0] *= faceDirection;
		tbn[1] *= faceDirection;
	#endif
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	#ifdef USE_TANGENT
		mat3 tbn2 = mat3( normalize( vTangent ), normalize( vBitangent ), normal );
	#else
		mat3 tbn2 = getTangentFrame( - vViewPosition, normal, vClearcoatNormalMapUv );
	#endif
	#ifdef DOUBLE_SIDED
		tbn2[0] *= faceDirection;
		tbn2[1] *= faceDirection;
	#endif
#endif
vec3 nonPerturbedNormal = normal;`,Fp=`#ifdef USE_NORMALMAP_OBJECTSPACE
	normal = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;
	#ifdef FLIP_SIDED
		normal = - normal;
	#endif
	#ifdef DOUBLE_SIDED
		normal = normal * faceDirection;
	#endif
	normal = normalize( normalMatrix * normal );
#elif defined( USE_NORMALMAP_TANGENTSPACE )
	vec3 mapN = texture2D( normalMap, vNormalMapUv ).xyz * 2.0 - 1.0;
	#if defined( USE_PACKED_NORMALMAP )
		mapN = vec3( mapN.xy, sqrt( saturate( 1.0 - dot( mapN.xy, mapN.xy ) ) ) );
	#endif
	mapN.xy *= normalScale;
	normal = normalize( tbn * mapN );
#elif defined( USE_BUMPMAP )
	normal = perturbNormalArb( - vViewPosition, normal, dHdxy_fwd(), faceDirection );
#endif`,Op=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,Bp=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,kp=`#ifndef FLAT_SHADED
	vNormal = normalize( transformedNormal );
	#ifdef USE_TANGENT
		vTangent = normalize( transformedTangent );
		vBitangent = normalize( cross( vNormal, vTangent ) * tangent.w );
		#ifdef FLIP_SIDED
			vBitangent = - vBitangent;
		#endif
	#endif
#endif`,zp=`#ifdef USE_NORMALMAP
	uniform sampler2D normalMap;
	uniform vec2 normalScale;
#endif
#ifdef USE_NORMALMAP_OBJECTSPACE
	uniform mat3 normalMatrix;
#endif
#if ! defined ( USE_TANGENT ) && ( defined ( USE_NORMALMAP_TANGENTSPACE ) || defined ( USE_CLEARCOAT_NORMALMAP ) || defined( USE_ANISOTROPY ) )
	mat3 getTangentFrame( vec3 eye_pos, vec3 surf_norm, vec2 uv ) {
		vec3 q0 = dFdx( eye_pos.xyz );
		vec3 q1 = dFdy( eye_pos.xyz );
		vec2 st0 = dFdx( uv.st );
		vec2 st1 = dFdy( uv.st );
		vec3 N = surf_norm;
		vec3 q1perp = cross( q1, N );
		vec3 q0perp = cross( N, q0 );
		vec3 T = q1perp * st0.x + q0perp * st1.x;
		vec3 B = q1perp * st0.y + q0perp * st1.y;
		float det = max( dot( T, T ), dot( B, B ) );
		float scale = ( det == 0.0 ) ? 0.0 : inversesqrt( det );
		return mat3( T * scale, B * scale, N );
	}
#endif`,Gp=`#ifdef USE_CLEARCOAT
	vec3 clearcoatNormal = nonPerturbedNormal;
#endif`,Hp=`#ifdef USE_CLEARCOAT_NORMALMAP
	vec3 clearcoatMapN = texture2D( clearcoatNormalMap, vClearcoatNormalMapUv ).xyz * 2.0 - 1.0;
	clearcoatMapN.xy *= clearcoatNormalScale;
	clearcoatNormal = normalize( tbn2 * clearcoatMapN );
#endif`,Vp=`#ifdef USE_CLEARCOATMAP
	uniform sampler2D clearcoatMap;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform sampler2D clearcoatNormalMap;
	uniform vec2 clearcoatNormalScale;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform sampler2D clearcoatRoughnessMap;
#endif`,Wp=`#ifdef USE_IRIDESCENCEMAP
	uniform sampler2D iridescenceMap;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform sampler2D iridescenceThicknessMap;
#endif`,Xp=`#ifdef OPAQUE
diffuseColor.a = 1.0;
#endif
#ifdef USE_TRANSMISSION
diffuseColor.a *= material.transmissionAlpha;
#endif
gl_FragColor = vec4( outgoingLight, diffuseColor.a );`,qp=`vec3 packNormalToRGB( const in vec3 normal ) {
	return normalize( normal ) * 0.5 + 0.5;
}
vec3 unpackRGBToNormal( const in vec3 rgb ) {
	return 2.0 * rgb.xyz - 1.0;
}
const float PackUpscale = 256. / 255.;const float UnpackDownscale = 255. / 256.;const float ShiftRight8 = 1. / 256.;
const float Inv255 = 1. / 255.;
const vec4 PackFactors = vec4( 1.0, 256.0, 256.0 * 256.0, 256.0 * 256.0 * 256.0 );
const vec2 UnpackFactors2 = vec2( UnpackDownscale, 1.0 / PackFactors.g );
const vec3 UnpackFactors3 = vec3( UnpackDownscale / PackFactors.rg, 1.0 / PackFactors.b );
const vec4 UnpackFactors4 = vec4( UnpackDownscale / PackFactors.rgb, 1.0 / PackFactors.a );
vec4 packDepthToRGBA( const in float v ) {
	if( v <= 0.0 )
		return vec4( 0., 0., 0., 0. );
	if( v >= 1.0 )
		return vec4( 1., 1., 1., 1. );
	float vuf;
	float af = modf( v * PackFactors.a, vuf );
	float bf = modf( vuf * ShiftRight8, vuf );
	float gf = modf( vuf * ShiftRight8, vuf );
	return vec4( vuf * Inv255, gf * PackUpscale, bf * PackUpscale, af );
}
vec3 packDepthToRGB( const in float v ) {
	if( v <= 0.0 )
		return vec3( 0., 0., 0. );
	if( v >= 1.0 )
		return vec3( 1., 1., 1. );
	float vuf;
	float bf = modf( v * PackFactors.b, vuf );
	float gf = modf( vuf * ShiftRight8, vuf );
	return vec3( vuf * Inv255, gf * PackUpscale, bf );
}
vec2 packDepthToRG( const in float v ) {
	if( v <= 0.0 )
		return vec2( 0., 0. );
	if( v >= 1.0 )
		return vec2( 1., 1. );
	float vuf;
	float gf = modf( v * 256., vuf );
	return vec2( vuf * Inv255, gf );
}
float unpackRGBAToDepth( const in vec4 v ) {
	return dot( v, UnpackFactors4 );
}
float unpackRGBToDepth( const in vec3 v ) {
	return dot( v, UnpackFactors3 );
}
float unpackRGToDepth( const in vec2 v ) {
	return v.r * UnpackFactors2.r + v.g * UnpackFactors2.g;
}
vec4 pack2HalfToRGBA( const in vec2 v ) {
	vec4 r = vec4( v.x, fract( v.x * 255.0 ), v.y, fract( v.y * 255.0 ) );
	return vec4( r.x - r.y / 255.0, r.y, r.z - r.w / 255.0, r.w );
}
vec2 unpackRGBATo2Half( const in vec4 v ) {
	return vec2( v.x + ( v.y / 255.0 ), v.z + ( v.w / 255.0 ) );
}
float viewZToOrthographicDepth( const in float viewZ, const in float near, const in float far ) {
	return ( viewZ + near ) / ( near - far );
}
float orthographicDepthToViewZ( const in float depth, const in float near, const in float far ) {
	#ifdef USE_REVERSED_DEPTH_BUFFER
	
		return depth * ( far - near ) - far;
	#else
		return depth * ( near - far ) - near;
	#endif
}
float viewZToPerspectiveDepth( const in float viewZ, const in float near, const in float far ) {
	return ( ( near + viewZ ) * far ) / ( ( far - near ) * viewZ );
}
float perspectiveDepthToViewZ( const in float depth, const in float near, const in float far ) {
	
	#ifdef USE_REVERSED_DEPTH_BUFFER
		return ( near * far ) / ( ( near - far ) * depth - near );
	#else
		return ( near * far ) / ( ( far - near ) * depth - far );
	#endif
}`,Yp=`#ifdef PREMULTIPLIED_ALPHA
	gl_FragColor.rgb *= gl_FragColor.a;
#endif`,$p=`vec4 mvPosition = vec4( transformed, 1.0 );
#ifdef USE_BATCHING
	mvPosition = batchingMatrix * mvPosition;
#endif
#ifdef USE_INSTANCING
	mvPosition = instanceMatrix * mvPosition;
#endif
mvPosition = modelViewMatrix * mvPosition;
gl_Position = projectionMatrix * mvPosition;`,Kp=`#ifdef DITHERING
	gl_FragColor.rgb = dithering( gl_FragColor.rgb );
#endif`,Zp=`#ifdef DITHERING
	vec3 dithering( vec3 color ) {
		float grid_position = rand( gl_FragCoord.xy );
		vec3 dither_shift_RGB = vec3( 0.25 / 255.0, -0.25 / 255.0, 0.25 / 255.0 );
		dither_shift_RGB = mix( 2.0 * dither_shift_RGB, -2.0 * dither_shift_RGB, grid_position );
		return color + dither_shift_RGB;
	}
#endif`,Jp=`float roughnessFactor = roughness;
#ifdef USE_ROUGHNESSMAP
	vec4 texelRoughness = texture2D( roughnessMap, vRoughnessMapUv );
	roughnessFactor *= texelRoughness.g;
#endif`,Qp=`#ifdef USE_ROUGHNESSMAP
	uniform sampler2D roughnessMap;
#endif`,jp=`#if NUM_SPOT_LIGHT_COORDS > 0
	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];
#endif
#if NUM_SPOT_LIGHT_MAPS > 0
	uniform sampler2D spotLightMap[ NUM_SPOT_LIGHT_MAPS ];
#endif
#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform sampler2DShadow directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];
		#else
			uniform sampler2D directionalShadowMap[ NUM_DIR_LIGHT_SHADOWS ];
		#endif
		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];
		struct DirectionalLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform sampler2DShadow spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];
		#else
			uniform sampler2D spotShadowMap[ NUM_SPOT_LIGHT_SHADOWS ];
		#endif
		struct SpotLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		#if defined( SHADOWMAP_TYPE_PCF )
			uniform samplerCubeShadow pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];
		#elif defined( SHADOWMAP_TYPE_BASIC )
			uniform samplerCube pointShadowMap[ NUM_POINT_LIGHT_SHADOWS ];
		#endif
		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];
		struct PointLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
			float shadowCameraNear;
			float shadowCameraFar;
		};
		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];
	#endif
	#if defined( SHADOWMAP_TYPE_PCF )
		float interleavedGradientNoise( vec2 position ) {
			return fract( 52.9829189 * fract( dot( position, vec2( 0.06711056, 0.00583715 ) ) ) );
		}
		vec2 vogelDiskSample( int sampleIndex, int samplesCount, float phi ) {
			const float goldenAngle = 2.399963229728653;
			float r = sqrt( ( float( sampleIndex ) + 0.5 ) / float( samplesCount ) );
			float theta = float( sampleIndex ) * goldenAngle + phi;
			return vec2( cos( theta ), sin( theta ) ) * r;
		}
	#endif
	#if defined( SHADOWMAP_TYPE_PCF )
		float getShadow( sampler2DShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			shadowCoord.z += shadowBias;
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				vec2 texelSize = vec2( 1.0 ) / shadowMapSize;
				float radius = shadowRadius * texelSize.x;
				float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;
				shadow = (
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 0, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 1, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 2, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 3, 5, phi ) * radius, shadowCoord.z ) ) +
					texture( shadowMap, vec3( shadowCoord.xy + vogelDiskSample( 4, 5, phi ) * radius, shadowCoord.z ) )
				) * 0.2;
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#elif defined( SHADOWMAP_TYPE_VSM )
		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				shadowCoord.z -= shadowBias;
			#else
				shadowCoord.z += shadowBias;
			#endif
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				vec2 distribution = texture2D( shadowMap, shadowCoord.xy ).rg;
				float mean = distribution.x;
				float variance = distribution.y * distribution.y;
				#ifdef USE_REVERSED_DEPTH_BUFFER
					float hard_shadow = step( mean, shadowCoord.z );
				#else
					float hard_shadow = step( shadowCoord.z, mean );
				#endif
				
				if ( hard_shadow == 1.0 ) {
					shadow = 1.0;
				} else {
					variance = max( variance, 0.0000001 );
					float d = shadowCoord.z - mean;
					float p_max = variance / ( variance + d * d );
					p_max = clamp( ( p_max - 0.3 ) / 0.65, 0.0, 1.0 );
					shadow = max( hard_shadow, p_max );
				}
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#else
		float getShadow( sampler2D shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord ) {
			float shadow = 1.0;
			shadowCoord.xyz /= shadowCoord.w;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				shadowCoord.z -= shadowBias;
			#else
				shadowCoord.z += shadowBias;
			#endif
			bool inFrustum = shadowCoord.x >= 0.0 && shadowCoord.x <= 1.0 && shadowCoord.y >= 0.0 && shadowCoord.y <= 1.0;
			bool frustumTest = inFrustum && shadowCoord.z <= 1.0;
			if ( frustumTest ) {
				float depth = texture2D( shadowMap, shadowCoord.xy ).r;
				#ifdef USE_REVERSED_DEPTH_BUFFER
					shadow = step( depth, shadowCoord.z );
				#else
					shadow = step( shadowCoord.z, depth );
				#endif
			}
			return mix( 1.0, shadow, shadowIntensity );
		}
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
	#if defined( SHADOWMAP_TYPE_PCF )
	float getPointShadow( samplerCubeShadow shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {
		float shadow = 1.0;
		vec3 lightToPosition = shadowCoord.xyz;
		vec3 bd3D = normalize( lightToPosition );
		vec3 absVec = abs( lightToPosition );
		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );
		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {
			#ifdef USE_REVERSED_DEPTH_BUFFER
				float dp = ( shadowCameraNear * ( shadowCameraFar - viewSpaceZ ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
				dp -= shadowBias;
			#else
				float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
				dp += shadowBias;
			#endif
			float texelSize = shadowRadius / shadowMapSize.x;
			vec3 absDir = abs( bd3D );
			vec3 tangent = absDir.x > absDir.z ? vec3( 0.0, 1.0, 0.0 ) : vec3( 1.0, 0.0, 0.0 );
			tangent = normalize( cross( bd3D, tangent ) );
			vec3 bitangent = cross( bd3D, tangent );
			float phi = interleavedGradientNoise( gl_FragCoord.xy ) * PI2;
			vec2 sample0 = vogelDiskSample( 0, 5, phi );
			vec2 sample1 = vogelDiskSample( 1, 5, phi );
			vec2 sample2 = vogelDiskSample( 2, 5, phi );
			vec2 sample3 = vogelDiskSample( 3, 5, phi );
			vec2 sample4 = vogelDiskSample( 4, 5, phi );
			shadow = (
				texture( shadowMap, vec4( bd3D + ( tangent * sample0.x + bitangent * sample0.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample1.x + bitangent * sample1.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample2.x + bitangent * sample2.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample3.x + bitangent * sample3.y ) * texelSize, dp ) ) +
				texture( shadowMap, vec4( bd3D + ( tangent * sample4.x + bitangent * sample4.y ) * texelSize, dp ) )
			) * 0.2;
		}
		return mix( 1.0, shadow, shadowIntensity );
	}
	#elif defined( SHADOWMAP_TYPE_BASIC )
	float getPointShadow( samplerCube shadowMap, vec2 shadowMapSize, float shadowIntensity, float shadowBias, float shadowRadius, vec4 shadowCoord, float shadowCameraNear, float shadowCameraFar ) {
		float shadow = 1.0;
		vec3 lightToPosition = shadowCoord.xyz;
		vec3 absVec = abs( lightToPosition );
		float viewSpaceZ = max( max( absVec.x, absVec.y ), absVec.z );
		if ( viewSpaceZ - shadowCameraFar <= 0.0 && viewSpaceZ - shadowCameraNear >= 0.0 ) {
			float dp = ( shadowCameraFar * ( viewSpaceZ - shadowCameraNear ) ) / ( viewSpaceZ * ( shadowCameraFar - shadowCameraNear ) );
			dp += shadowBias;
			vec3 bd3D = normalize( lightToPosition );
			float depth = textureCube( shadowMap, bd3D ).r;
			#ifdef USE_REVERSED_DEPTH_BUFFER
				depth = 1.0 - depth;
			#endif
			shadow = step( dp, depth );
		}
		return mix( 1.0, shadow, shadowIntensity );
	}
	#endif
	#endif
#endif`,em=`#if NUM_SPOT_LIGHT_COORDS > 0
	uniform mat4 spotLightMatrix[ NUM_SPOT_LIGHT_COORDS ];
	varying vec4 vSpotLightCoord[ NUM_SPOT_LIGHT_COORDS ];
#endif
#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
		uniform mat4 directionalShadowMatrix[ NUM_DIR_LIGHT_SHADOWS ];
		varying vec4 vDirectionalShadowCoord[ NUM_DIR_LIGHT_SHADOWS ];
		struct DirectionalLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform DirectionalLightShadow directionalLightShadows[ NUM_DIR_LIGHT_SHADOWS ];
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
		struct SpotLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
		};
		uniform SpotLightShadow spotLightShadows[ NUM_SPOT_LIGHT_SHADOWS ];
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		uniform mat4 pointShadowMatrix[ NUM_POINT_LIGHT_SHADOWS ];
		varying vec4 vPointShadowCoord[ NUM_POINT_LIGHT_SHADOWS ];
		struct PointLightShadow {
			float shadowIntensity;
			float shadowBias;
			float shadowNormalBias;
			float shadowRadius;
			vec2 shadowMapSize;
			float shadowCameraNear;
			float shadowCameraFar;
		};
		uniform PointLightShadow pointLightShadows[ NUM_POINT_LIGHT_SHADOWS ];
	#endif
#endif`,tm=`#if ( defined( USE_SHADOWMAP ) && ( NUM_DIR_LIGHT_SHADOWS > 0 || NUM_POINT_LIGHT_SHADOWS > 0 ) ) || ( NUM_SPOT_LIGHT_COORDS > 0 )
	#ifdef HAS_NORMAL
		vec3 shadowWorldNormal = transformNormalByInverseViewMatrix( transformedNormal, viewMatrix );
	#else
		vec3 shadowWorldNormal = vec3( 0.0 );
	#endif
	vec4 shadowWorldPosition;
#endif
#if defined( USE_SHADOWMAP )
	#if NUM_DIR_LIGHT_SHADOWS > 0
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {
			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * directionalLightShadows[ i ].shadowNormalBias, 0 );
			vDirectionalShadowCoord[ i ] = directionalShadowMatrix[ i ] * shadowWorldPosition;
		}
		#pragma unroll_loop_end
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0
		#pragma unroll_loop_start
		for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {
			shadowWorldPosition = worldPosition + vec4( shadowWorldNormal * pointLightShadows[ i ].shadowNormalBias, 0 );
			vPointShadowCoord[ i ] = pointShadowMatrix[ i ] * shadowWorldPosition;
		}
		#pragma unroll_loop_end
	#endif
#endif
#if NUM_SPOT_LIGHT_COORDS > 0
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHT_COORDS; i ++ ) {
		shadowWorldPosition = worldPosition;
		#if ( defined( USE_SHADOWMAP ) && UNROLLED_LOOP_INDEX < NUM_SPOT_LIGHT_SHADOWS )
			shadowWorldPosition.xyz += shadowWorldNormal * spotLightShadows[ i ].shadowNormalBias;
		#endif
		vSpotLightCoord[ i ] = spotLightMatrix[ i ] * shadowWorldPosition;
	}
	#pragma unroll_loop_end
#endif`,nm=`float getShadowMask() {
	float shadow = 1.0;
	#ifdef USE_SHADOWMAP
	#if NUM_DIR_LIGHT_SHADOWS > 0
	DirectionalLightShadow directionalLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_DIR_LIGHT_SHADOWS; i ++ ) {
		directionalLight = directionalLightShadows[ i ];
		shadow *= receiveShadow ? getShadow( directionalShadowMap[ i ], directionalLight.shadowMapSize, directionalLight.shadowIntensity, directionalLight.shadowBias, directionalLight.shadowRadius, vDirectionalShadowCoord[ i ] ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#if NUM_SPOT_LIGHT_SHADOWS > 0
	SpotLightShadow spotLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_SPOT_LIGHT_SHADOWS; i ++ ) {
		spotLight = spotLightShadows[ i ];
		shadow *= receiveShadow ? getShadow( spotShadowMap[ i ], spotLight.shadowMapSize, spotLight.shadowIntensity, spotLight.shadowBias, spotLight.shadowRadius, vSpotLightCoord[ i ] ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#if NUM_POINT_LIGHT_SHADOWS > 0 && ( defined( SHADOWMAP_TYPE_PCF ) || defined( SHADOWMAP_TYPE_BASIC ) )
	PointLightShadow pointLight;
	#pragma unroll_loop_start
	for ( int i = 0; i < NUM_POINT_LIGHT_SHADOWS; i ++ ) {
		pointLight = pointLightShadows[ i ];
		shadow *= receiveShadow ? getPointShadow( pointShadowMap[ i ], pointLight.shadowMapSize, pointLight.shadowIntensity, pointLight.shadowBias, pointLight.shadowRadius, vPointShadowCoord[ i ], pointLight.shadowCameraNear, pointLight.shadowCameraFar ) : 1.0;
	}
	#pragma unroll_loop_end
	#endif
	#endif
	return shadow;
}`,im=`#ifdef USE_SKINNING
	mat4 boneMatX = getBoneMatrix( skinIndex.x );
	mat4 boneMatY = getBoneMatrix( skinIndex.y );
	mat4 boneMatZ = getBoneMatrix( skinIndex.z );
	mat4 boneMatW = getBoneMatrix( skinIndex.w );
#endif`,rm=`#ifdef USE_SKINNING
	uniform mat4 bindMatrix;
	uniform mat4 bindMatrixInverse;
	uniform highp sampler2D boneTexture;
	mat4 getBoneMatrix( const in float i ) {
		int size = textureSize( boneTexture, 0 ).x;
		int j = int( i ) * 4;
		int x = j % size;
		int y = j / size;
		vec4 v1 = texelFetch( boneTexture, ivec2( x, y ), 0 );
		vec4 v2 = texelFetch( boneTexture, ivec2( x + 1, y ), 0 );
		vec4 v3 = texelFetch( boneTexture, ivec2( x + 2, y ), 0 );
		vec4 v4 = texelFetch( boneTexture, ivec2( x + 3, y ), 0 );
		return mat4( v1, v2, v3, v4 );
	}
#endif`,sm=`#ifdef USE_SKINNING
	vec4 skinVertex = bindMatrix * vec4( transformed, 1.0 );
	vec4 skinned = vec4( 0.0 );
	skinned += boneMatX * skinVertex * skinWeight.x;
	skinned += boneMatY * skinVertex * skinWeight.y;
	skinned += boneMatZ * skinVertex * skinWeight.z;
	skinned += boneMatW * skinVertex * skinWeight.w;
	transformed = ( bindMatrixInverse * skinned ).xyz;
#endif`,am=`#ifdef USE_SKINNING
	mat4 skinMatrix = mat4( 0.0 );
	skinMatrix += skinWeight.x * boneMatX;
	skinMatrix += skinWeight.y * boneMatY;
	skinMatrix += skinWeight.z * boneMatZ;
	skinMatrix += skinWeight.w * boneMatW;
	skinMatrix = bindMatrixInverse * skinMatrix * bindMatrix;
	objectNormal = vec4( skinMatrix * vec4( objectNormal, 0.0 ) ).xyz;
	#ifdef USE_TANGENT
		objectTangent = vec4( skinMatrix * vec4( objectTangent, 0.0 ) ).xyz;
	#endif
#endif`,om=`float specularStrength;
#ifdef USE_SPECULARMAP
	vec4 texelSpecular = texture2D( specularMap, vSpecularMapUv );
	specularStrength = texelSpecular.r;
#else
	specularStrength = 1.0;
#endif`,lm=`#ifdef USE_SPECULARMAP
	uniform sampler2D specularMap;
#endif`,cm=`#if defined( TONE_MAPPING )
	gl_FragColor.rgb = toneMapping( gl_FragColor.rgb );
#endif`,dm=`#ifndef saturate
#define saturate( a ) clamp( a, 0.0, 1.0 )
#endif
uniform float toneMappingExposure;
vec3 LinearToneMapping( vec3 color ) {
	return saturate( toneMappingExposure * color );
}
vec3 ReinhardToneMapping( vec3 color ) {
	color *= toneMappingExposure;
	return saturate( color / ( vec3( 1.0 ) + color ) );
}
vec3 CineonToneMapping( vec3 color ) {
	color *= toneMappingExposure;
	color = max( vec3( 0.0 ), color - 0.004 );
	return pow( ( color * ( 6.2 * color + 0.5 ) ) / ( color * ( 6.2 * color + 1.7 ) + 0.06 ), vec3( 2.2 ) );
}
vec3 RRTAndODTFit( vec3 v ) {
	vec3 a = v * ( v + 0.0245786 ) - 0.000090537;
	vec3 b = v * ( 0.983729 * v + 0.4329510 ) + 0.238081;
	return a / b;
}
vec3 ACESFilmicToneMapping( vec3 color ) {
	const mat3 ACESInputMat = mat3(
		vec3( 0.59719, 0.07600, 0.02840 ),		vec3( 0.35458, 0.90834, 0.13383 ),
		vec3( 0.04823, 0.01566, 0.83777 )
	);
	const mat3 ACESOutputMat = mat3(
		vec3(  1.60475, -0.10208, -0.00327 ),		vec3( -0.53108,  1.10813, -0.07276 ),
		vec3( -0.07367, -0.00605,  1.07602 )
	);
	color *= toneMappingExposure / 0.6;
	color = ACESInputMat * color;
	color = RRTAndODTFit( color );
	color = ACESOutputMat * color;
	return saturate( color );
}
const mat3 LINEAR_REC2020_TO_LINEAR_SRGB = mat3(
	vec3( 1.6605, - 0.1246, - 0.0182 ),
	vec3( - 0.5876, 1.1329, - 0.1006 ),
	vec3( - 0.0728, - 0.0083, 1.1187 )
);
const mat3 LINEAR_SRGB_TO_LINEAR_REC2020 = mat3(
	vec3( 0.6274, 0.0691, 0.0164 ),
	vec3( 0.3293, 0.9195, 0.0880 ),
	vec3( 0.0433, 0.0113, 0.8956 )
);
vec3 agxDefaultContrastApprox( vec3 x ) {
	vec3 x2 = x * x;
	vec3 x4 = x2 * x2;
	return + 15.5 * x4 * x2
		- 40.14 * x4 * x
		+ 31.96 * x4
		- 6.868 * x2 * x
		+ 0.4298 * x2
		+ 0.1191 * x
		- 0.00232;
}
vec3 AgXToneMapping( vec3 color ) {
	const mat3 AgXInsetMatrix = mat3(
		vec3( 0.856627153315983, 0.137318972929847, 0.11189821299995 ),
		vec3( 0.0951212405381588, 0.761241990602591, 0.0767994186031903 ),
		vec3( 0.0482516061458583, 0.101439036467562, 0.811302368396859 )
	);
	const mat3 AgXOutsetMatrix = mat3(
		vec3( 1.1271005818144368, - 0.1413297634984383, - 0.14132976349843826 ),
		vec3( - 0.11060664309660323, 1.157823702216272, - 0.11060664309660294 ),
		vec3( - 0.016493938717834573, - 0.016493938717834257, 1.2519364065950405 )
	);
	const float AgxMinEv = - 12.47393;	const float AgxMaxEv = 4.026069;
	color *= toneMappingExposure;
	color = LINEAR_SRGB_TO_LINEAR_REC2020 * color;
	color = AgXInsetMatrix * color;
	color = max( color, 1e-10 );	color = log2( color );
	color = ( color - AgxMinEv ) / ( AgxMaxEv - AgxMinEv );
	color = clamp( color, 0.0, 1.0 );
	color = agxDefaultContrastApprox( color );
	color = AgXOutsetMatrix * color;
	color = pow( max( vec3( 0.0 ), color ), vec3( 2.2 ) );
	color = LINEAR_REC2020_TO_LINEAR_SRGB * color;
	color = clamp( color, 0.0, 1.0 );
	return color;
}
vec3 NeutralToneMapping( vec3 color ) {
	const float StartCompression = 0.8 - 0.04;
	const float Desaturation = 0.15;
	color *= toneMappingExposure;
	float x = min( color.r, min( color.g, color.b ) );
	float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
	color -= offset;
	float peak = max( color.r, max( color.g, color.b ) );
	if ( peak < StartCompression ) return color;
	float d = 1. - StartCompression;
	float newPeak = 1. - d * d / ( peak + d - StartCompression );
	color *= newPeak / peak;
	float g = 1. - 1. / ( Desaturation * ( peak - newPeak ) + 1. );
	return mix( color, vec3( newPeak ), g );
}
vec3 CustomToneMapping( vec3 color ) { return color; }`,um=`#ifdef USE_TRANSMISSION
	material.transmission = transmission;
	material.transmissionAlpha = 1.0;
	material.thickness = thickness;
	material.attenuationDistance = attenuationDistance;
	material.attenuationColor = attenuationColor;
	#ifdef USE_TRANSMISSIONMAP
		material.transmission *= texture2D( transmissionMap, vTransmissionMapUv ).r;
	#endif
	#ifdef USE_THICKNESSMAP
		material.thickness *= texture2D( thicknessMap, vThicknessMapUv ).g;
	#endif
	vec3 pos = vWorldPosition;
	vec3 v = normalize( cameraPosition - pos );
	vec3 n = transformNormalByInverseViewMatrix( normal, viewMatrix );
	vec4 transmitted = getIBLVolumeRefraction(
		n, v, material.roughness, material.diffuseContribution, material.specularColorBlended, material.specularF90,
		pos, modelMatrix, viewMatrix, projectionMatrix, material.dispersion, material.ior, material.thickness,
		material.attenuationColor, material.attenuationDistance );
	material.transmissionAlpha = mix( material.transmissionAlpha, transmitted.a, material.transmission );
	totalDiffuse = mix( totalDiffuse, transmitted.rgb, material.transmission );
#endif`,fm=`#ifdef USE_TRANSMISSION
	uniform float transmission;
	uniform float thickness;
	uniform float attenuationDistance;
	uniform vec3 attenuationColor;
	#ifdef USE_TRANSMISSIONMAP
		uniform sampler2D transmissionMap;
	#endif
	#ifdef USE_THICKNESSMAP
		uniform sampler2D thicknessMap;
	#endif
	uniform vec2 transmissionSamplerSize;
	uniform sampler2D transmissionSamplerMap;
	uniform mat4 modelMatrix;
	uniform mat4 projectionMatrix;
	varying vec3 vWorldPosition;
	float w0( float a ) {
		return ( 1.0 / 6.0 ) * ( a * ( a * ( - a + 3.0 ) - 3.0 ) + 1.0 );
	}
	float w1( float a ) {
		return ( 1.0 / 6.0 ) * ( a *  a * ( 3.0 * a - 6.0 ) + 4.0 );
	}
	float w2( float a ){
		return ( 1.0 / 6.0 ) * ( a * ( a * ( - 3.0 * a + 3.0 ) + 3.0 ) + 1.0 );
	}
	float w3( float a ) {
		return ( 1.0 / 6.0 ) * ( a * a * a );
	}
	float g0( float a ) {
		return w0( a ) + w1( a );
	}
	float g1( float a ) {
		return w2( a ) + w3( a );
	}
	float h0( float a ) {
		return - 1.0 + w1( a ) / ( w0( a ) + w1( a ) );
	}
	float h1( float a ) {
		return 1.0 + w3( a ) / ( w2( a ) + w3( a ) );
	}
	vec4 bicubic( sampler2D tex, vec2 uv, vec4 texelSize, float lod ) {
		uv = uv * texelSize.zw + 0.5;
		vec2 iuv = floor( uv );
		vec2 fuv = fract( uv );
		float g0x = g0( fuv.x );
		float g1x = g1( fuv.x );
		float h0x = h0( fuv.x );
		float h1x = h1( fuv.x );
		float h0y = h0( fuv.y );
		float h1y = h1( fuv.y );
		vec2 p0 = ( vec2( iuv.x + h0x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;
		vec2 p1 = ( vec2( iuv.x + h1x, iuv.y + h0y ) - 0.5 ) * texelSize.xy;
		vec2 p2 = ( vec2( iuv.x + h0x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;
		vec2 p3 = ( vec2( iuv.x + h1x, iuv.y + h1y ) - 0.5 ) * texelSize.xy;
		return g0( fuv.y ) * ( g0x * textureLod( tex, p0, lod ) + g1x * textureLod( tex, p1, lod ) ) +
			g1( fuv.y ) * ( g0x * textureLod( tex, p2, lod ) + g1x * textureLod( tex, p3, lod ) );
	}
	vec4 textureBicubic( sampler2D sampler, vec2 uv, float lod ) {
		vec2 fLodSize = vec2( textureSize( sampler, int( lod ) ) );
		vec2 cLodSize = vec2( textureSize( sampler, int( lod + 1.0 ) ) );
		vec2 fLodSizeInv = 1.0 / fLodSize;
		vec2 cLodSizeInv = 1.0 / cLodSize;
		vec4 fSample = bicubic( sampler, uv, vec4( fLodSizeInv, fLodSize ), floor( lod ) );
		vec4 cSample = bicubic( sampler, uv, vec4( cLodSizeInv, cLodSize ), ceil( lod ) );
		return mix( fSample, cSample, fract( lod ) );
	}
	vec3 getVolumeTransmissionRay( const in vec3 n, const in vec3 v, const in float thickness, const in float ior, const in mat4 modelMatrix ) {
		vec3 refractionVector = refract( - v, normalize( n ), 1.0 / ior );
		vec3 modelScale;
		modelScale.x = length( vec3( modelMatrix[ 0 ].xyz ) );
		modelScale.y = length( vec3( modelMatrix[ 1 ].xyz ) );
		modelScale.z = length( vec3( modelMatrix[ 2 ].xyz ) );
		return normalize( refractionVector ) * thickness * modelScale;
	}
	float applyIorToRoughness( const in float roughness, const in float ior ) {
		return roughness * clamp( ior * 2.0 - 2.0, 0.0, 1.0 );
	}
	vec4 getTransmissionSample( const in vec2 fragCoord, const in float roughness, const in float ior ) {
		float lod = log2( transmissionSamplerSize.x ) * applyIorToRoughness( roughness, ior );
		return textureBicubic( transmissionSamplerMap, fragCoord.xy, lod );
	}
	vec3 volumeAttenuation( const in float transmissionDistance, const in vec3 attenuationColor, const in float attenuationDistance ) {
		if ( isinf( attenuationDistance ) ) {
			return vec3( 1.0 );
		} else {
			vec3 attenuationCoefficient = -log( attenuationColor ) / attenuationDistance;
			vec3 transmittance = exp( - attenuationCoefficient * transmissionDistance );			return transmittance;
		}
	}
	vec4 getIBLVolumeRefraction( const in vec3 n, const in vec3 v, const in float roughness, const in vec3 diffuseColor,
		const in vec3 specularColor, const in float specularF90, const in vec3 position, const in mat4 modelMatrix,
		const in mat4 viewMatrix, const in mat4 projMatrix, const in float dispersion, const in float ior, const in float thickness,
		const in vec3 attenuationColor, const in float attenuationDistance ) {
		vec4 transmittedLight;
		vec3 transmittance;
		#ifdef USE_DISPERSION
			float halfSpread = ( ior - 1.0 ) * 0.025 * dispersion;
			vec3 iors = vec3( ior - halfSpread, ior, ior + halfSpread );
			for ( int i = 0; i < 3; i ++ ) {
				vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, iors[ i ], modelMatrix );
				vec3 refractedRayExit = position + transmissionRay;
				vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );
				vec2 refractionCoords = ndcPos.xy / ndcPos.w;
				refractionCoords += 1.0;
				refractionCoords /= 2.0;
				vec4 transmissionSample = getTransmissionSample( refractionCoords, roughness, iors[ i ] );
				transmittedLight[ i ] = transmissionSample[ i ];
				transmittedLight.a += transmissionSample.a;
				transmittance[ i ] = diffuseColor[ i ] * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance )[ i ];
			}
			transmittedLight.a /= 3.0;
		#else
			vec3 transmissionRay = getVolumeTransmissionRay( n, v, thickness, ior, modelMatrix );
			vec3 refractedRayExit = position + transmissionRay;
			vec4 ndcPos = projMatrix * viewMatrix * vec4( refractedRayExit, 1.0 );
			vec2 refractionCoords = ndcPos.xy / ndcPos.w;
			refractionCoords += 1.0;
			refractionCoords /= 2.0;
			transmittedLight = getTransmissionSample( refractionCoords, roughness, ior );
			transmittance = diffuseColor * volumeAttenuation( length( transmissionRay ), attenuationColor, attenuationDistance );
		#endif
		vec3 attenuatedColor = transmittance * transmittedLight.rgb;
		vec3 F = EnvironmentBRDF( n, v, specularColor, specularF90, roughness );
		float transmittanceFactor = ( transmittance.r + transmittance.g + transmittance.b ) / 3.0;
		return vec4( ( 1.0 - F ) * attenuatedColor, 1.0 - ( 1.0 - transmittedLight.a ) * transmittanceFactor );
	}
#endif`,hm=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	varying vec2 vUv;
#endif
#ifdef USE_MAP
	varying vec2 vMapUv;
#endif
#ifdef USE_ALPHAMAP
	varying vec2 vAlphaMapUv;
#endif
#ifdef USE_LIGHTMAP
	varying vec2 vLightMapUv;
#endif
#ifdef USE_AOMAP
	varying vec2 vAoMapUv;
#endif
#ifdef USE_BUMPMAP
	varying vec2 vBumpMapUv;
#endif
#ifdef USE_NORMALMAP
	varying vec2 vNormalMapUv;
#endif
#ifdef USE_EMISSIVEMAP
	varying vec2 vEmissiveMapUv;
#endif
#ifdef USE_METALNESSMAP
	varying vec2 vMetalnessMapUv;
#endif
#ifdef USE_ROUGHNESSMAP
	varying vec2 vRoughnessMapUv;
#endif
#ifdef USE_ANISOTROPYMAP
	varying vec2 vAnisotropyMapUv;
#endif
#ifdef USE_CLEARCOATMAP
	varying vec2 vClearcoatMapUv;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	varying vec2 vClearcoatNormalMapUv;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	varying vec2 vClearcoatRoughnessMapUv;
#endif
#ifdef USE_IRIDESCENCEMAP
	varying vec2 vIridescenceMapUv;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	varying vec2 vIridescenceThicknessMapUv;
#endif
#ifdef USE_SHEEN_COLORMAP
	varying vec2 vSheenColorMapUv;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	varying vec2 vSheenRoughnessMapUv;
#endif
#ifdef USE_SPECULARMAP
	varying vec2 vSpecularMapUv;
#endif
#ifdef USE_SPECULAR_COLORMAP
	varying vec2 vSpecularColorMapUv;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	varying vec2 vSpecularIntensityMapUv;
#endif
#ifdef USE_TRANSMISSIONMAP
	uniform mat3 transmissionMapTransform;
	varying vec2 vTransmissionMapUv;
#endif
#ifdef USE_THICKNESSMAP
	uniform mat3 thicknessMapTransform;
	varying vec2 vThicknessMapUv;
#endif`,pm=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	varying vec2 vUv;
#endif
#ifdef USE_MAP
	uniform mat3 mapTransform;
	varying vec2 vMapUv;
#endif
#ifdef USE_ALPHAMAP
	uniform mat3 alphaMapTransform;
	varying vec2 vAlphaMapUv;
#endif
#ifdef USE_LIGHTMAP
	uniform mat3 lightMapTransform;
	varying vec2 vLightMapUv;
#endif
#ifdef USE_AOMAP
	uniform mat3 aoMapTransform;
	varying vec2 vAoMapUv;
#endif
#ifdef USE_BUMPMAP
	uniform mat3 bumpMapTransform;
	varying vec2 vBumpMapUv;
#endif
#ifdef USE_NORMALMAP
	uniform mat3 normalMapTransform;
	varying vec2 vNormalMapUv;
#endif
#ifdef USE_DISPLACEMENTMAP
	uniform mat3 displacementMapTransform;
	varying vec2 vDisplacementMapUv;
#endif
#ifdef USE_EMISSIVEMAP
	uniform mat3 emissiveMapTransform;
	varying vec2 vEmissiveMapUv;
#endif
#ifdef USE_METALNESSMAP
	uniform mat3 metalnessMapTransform;
	varying vec2 vMetalnessMapUv;
#endif
#ifdef USE_ROUGHNESSMAP
	uniform mat3 roughnessMapTransform;
	varying vec2 vRoughnessMapUv;
#endif
#ifdef USE_ANISOTROPYMAP
	uniform mat3 anisotropyMapTransform;
	varying vec2 vAnisotropyMapUv;
#endif
#ifdef USE_CLEARCOATMAP
	uniform mat3 clearcoatMapTransform;
	varying vec2 vClearcoatMapUv;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform mat3 clearcoatNormalMapTransform;
	varying vec2 vClearcoatNormalMapUv;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform mat3 clearcoatRoughnessMapTransform;
	varying vec2 vClearcoatRoughnessMapUv;
#endif
#ifdef USE_SHEEN_COLORMAP
	uniform mat3 sheenColorMapTransform;
	varying vec2 vSheenColorMapUv;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	uniform mat3 sheenRoughnessMapTransform;
	varying vec2 vSheenRoughnessMapUv;
#endif
#ifdef USE_IRIDESCENCEMAP
	uniform mat3 iridescenceMapTransform;
	varying vec2 vIridescenceMapUv;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform mat3 iridescenceThicknessMapTransform;
	varying vec2 vIridescenceThicknessMapUv;
#endif
#ifdef USE_SPECULARMAP
	uniform mat3 specularMapTransform;
	varying vec2 vSpecularMapUv;
#endif
#ifdef USE_SPECULAR_COLORMAP
	uniform mat3 specularColorMapTransform;
	varying vec2 vSpecularColorMapUv;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	uniform mat3 specularIntensityMapTransform;
	varying vec2 vSpecularIntensityMapUv;
#endif
#ifdef USE_TRANSMISSIONMAP
	uniform mat3 transmissionMapTransform;
	varying vec2 vTransmissionMapUv;
#endif
#ifdef USE_THICKNESSMAP
	uniform mat3 thicknessMapTransform;
	varying vec2 vThicknessMapUv;
#endif`,mm=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
	vUv = vec3( uv, 1 ).xy;
#endif
#ifdef USE_MAP
	vMapUv = ( mapTransform * vec3( MAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ALPHAMAP
	vAlphaMapUv = ( alphaMapTransform * vec3( ALPHAMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_LIGHTMAP
	vLightMapUv = ( lightMapTransform * vec3( LIGHTMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_AOMAP
	vAoMapUv = ( aoMapTransform * vec3( AOMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_BUMPMAP
	vBumpMapUv = ( bumpMapTransform * vec3( BUMPMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_NORMALMAP
	vNormalMapUv = ( normalMapTransform * vec3( NORMALMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_DISPLACEMENTMAP
	vDisplacementMapUv = ( displacementMapTransform * vec3( DISPLACEMENTMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_EMISSIVEMAP
	vEmissiveMapUv = ( emissiveMapTransform * vec3( EMISSIVEMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_METALNESSMAP
	vMetalnessMapUv = ( metalnessMapTransform * vec3( METALNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ROUGHNESSMAP
	vRoughnessMapUv = ( roughnessMapTransform * vec3( ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_ANISOTROPYMAP
	vAnisotropyMapUv = ( anisotropyMapTransform * vec3( ANISOTROPYMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOATMAP
	vClearcoatMapUv = ( clearcoatMapTransform * vec3( CLEARCOATMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	vClearcoatNormalMapUv = ( clearcoatNormalMapTransform * vec3( CLEARCOAT_NORMALMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	vClearcoatRoughnessMapUv = ( clearcoatRoughnessMapTransform * vec3( CLEARCOAT_ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_IRIDESCENCEMAP
	vIridescenceMapUv = ( iridescenceMapTransform * vec3( IRIDESCENCEMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	vIridescenceThicknessMapUv = ( iridescenceThicknessMapTransform * vec3( IRIDESCENCE_THICKNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SHEEN_COLORMAP
	vSheenColorMapUv = ( sheenColorMapTransform * vec3( SHEEN_COLORMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SHEEN_ROUGHNESSMAP
	vSheenRoughnessMapUv = ( sheenRoughnessMapTransform * vec3( SHEEN_ROUGHNESSMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULARMAP
	vSpecularMapUv = ( specularMapTransform * vec3( SPECULARMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULAR_COLORMAP
	vSpecularColorMapUv = ( specularColorMapTransform * vec3( SPECULAR_COLORMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_SPECULAR_INTENSITYMAP
	vSpecularIntensityMapUv = ( specularIntensityMapTransform * vec3( SPECULAR_INTENSITYMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_TRANSMISSIONMAP
	vTransmissionMapUv = ( transmissionMapTransform * vec3( TRANSMISSIONMAP_UV, 1 ) ).xy;
#endif
#ifdef USE_THICKNESSMAP
	vThicknessMapUv = ( thicknessMapTransform * vec3( THICKNESSMAP_UV, 1 ) ).xy;
#endif`,gm=`#if defined( USE_ENVMAP ) || defined( DISTANCE ) || defined ( USE_SHADOWMAP ) || defined ( USE_TRANSMISSION ) || NUM_SPOT_LIGHT_COORDS > 0
	vec4 worldPosition = vec4( transformed, 1.0 );
	#ifdef USE_BATCHING
		worldPosition = batchingMatrix * worldPosition;
	#endif
	#ifdef USE_INSTANCING
		worldPosition = instanceMatrix * worldPosition;
	#endif
	worldPosition = modelMatrix * worldPosition;
#endif`;const _m=`varying vec2 vUv;
uniform mat3 uvTransform;
void main() {
	vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	gl_Position = vec4( position.xy, 1.0, 1.0 );
}`,vm=`uniform sampler2D t2D;
uniform float backgroundIntensity;
varying vec2 vUv;
void main() {
	vec4 texColor = texture2D( t2D, vUv );
	#ifdef DECODE_VIDEO_TEXTURE
		texColor = vec4( mix( pow( texColor.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), texColor.rgb * 0.0773993808, vec3( lessThanEqual( texColor.rgb, vec3( 0.04045 ) ) ) ), texColor.w );
	#endif
	texColor.rgb *= backgroundIntensity;
	gl_FragColor = texColor;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,xm=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,ym=`#ifdef ENVMAP_TYPE_CUBE
	uniform samplerCube envMap;
#elif defined( ENVMAP_TYPE_CUBE_UV )
	uniform sampler2D envMap;
#endif
uniform float backgroundBlurriness;
uniform float backgroundIntensity;
uniform mat3 backgroundRotation;
varying vec3 vWorldDirection;
#include <cube_uv_reflection_fragment>
void main() {
	#ifdef ENVMAP_TYPE_CUBE
		vec4 texColor = textureCube( envMap, backgroundRotation * vWorldDirection );
	#elif defined( ENVMAP_TYPE_CUBE_UV )
		vec4 texColor = textureCubeUV( envMap, backgroundRotation * vWorldDirection, backgroundBlurriness );
	#else
		vec4 texColor = vec4( 0.0, 0.0, 0.0, 1.0 );
	#endif
	texColor.rgb *= backgroundIntensity;
	gl_FragColor = texColor;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,bm=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,Sm=`uniform samplerCube tCube;
uniform float tFlip;
uniform float opacity;
varying vec3 vWorldDirection;
void main() {
	vec4 texColor = textureCube( tCube, vec3( tFlip * vWorldDirection.x, vWorldDirection.yz ) );
	gl_FragColor = texColor;
	gl_FragColor.a *= opacity;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,Mm=`#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
varying vec2 vHighPrecisionZW;
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <skinbase_vertex>
	#include <morphinstance_vertex>
	#ifdef USE_DISPLACEMENTMAP
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vHighPrecisionZW = gl_Position.zw;
}`,Em=`#if DEPTH_PACKING == 3200
	uniform float opacity;
#endif
#include <common>
#include <packing>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
varying vec2 vHighPrecisionZW;
void main() {
	vec4 diffuseColor = vec4( 1.0 );
	#include <clipping_planes_fragment>
	#if DEPTH_PACKING == 3200
		diffuseColor.a = opacity;
	#endif
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <logdepthbuf_fragment>
	#ifdef USE_REVERSED_DEPTH_BUFFER
		float fragCoordZ = vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ];
	#else
		float fragCoordZ = 0.5 * vHighPrecisionZW[ 0 ] / vHighPrecisionZW[ 1 ] + 0.5;
	#endif
	#if DEPTH_PACKING == 3200
		gl_FragColor = vec4( vec3( 1.0 - fragCoordZ ), opacity );
	#elif DEPTH_PACKING == 3201
		gl_FragColor = packDepthToRGBA( fragCoordZ );
	#elif DEPTH_PACKING == 3202
		gl_FragColor = vec4( packDepthToRGB( fragCoordZ ), 1.0 );
	#elif DEPTH_PACKING == 3203
		gl_FragColor = vec4( packDepthToRG( fragCoordZ ), 0.0, 1.0 );
	#endif
}`,wm=`#define DISTANCE
varying vec3 vWorldPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <skinbase_vertex>
	#include <morphinstance_vertex>
	#ifdef USE_DISPLACEMENTMAP
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <worldpos_vertex>
	#include <clipping_planes_vertex>
	vWorldPosition = worldPosition.xyz;
}`,Tm=`#define DISTANCE
uniform vec3 referencePosition;
uniform float nearDistance;
uniform float farDistance;
varying vec3 vWorldPosition;
#include <common>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( 1.0 );
	#include <clipping_planes_fragment>
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	float dist = length( vWorldPosition - referencePosition );
	dist = ( dist - nearDistance ) / ( farDistance - nearDistance );
	dist = saturate( dist );
	gl_FragColor = vec4( dist, 0.0, 0.0, 1.0 );
}`,Am=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
}`,Rm=`uniform sampler2D tEquirect;
varying vec3 vWorldDirection;
#include <common>
void main() {
	vec3 direction = normalize( vWorldDirection );
	vec2 sampleUV = equirectUv( direction );
	gl_FragColor = texture2D( tEquirect, sampleUV );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,Cm=`uniform float scale;
attribute float lineDistance;
varying float vLineDistance;
#include <common>
#include <uv_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	vLineDistance = scale * lineDistance;
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
}`,Pm=`uniform vec3 diffuse;
uniform float opacity;
uniform float dashSize;
uniform float totalSize;
varying float vLineDistance;
#include <common>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	if ( mod( vLineDistance, totalSize ) > dashSize ) {
		discard;
	}
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,Lm=`#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#if defined ( USE_ENVMAP ) || defined ( USE_SKINNING )
		#include <beginnormal_vertex>
		#include <morphnormal_vertex>
		#include <skinbase_vertex>
		#include <skinnormal_vertex>
		#include <defaultnormal_vertex>
	#endif
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <fog_vertex>
}`,Im=`uniform vec3 diffuse;
uniform float opacity;
#ifndef FLAT_SHADED
	varying vec3 vNormal;
#endif
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <fog_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	#ifdef USE_LIGHTMAP
		vec4 lightMapTexel = texture2D( lightMap, vLightMapUv );
		reflectedLight.indirectDiffuse += lightMapTexel.rgb * lightMapIntensity * RECIPROCAL_PI;
	#else
		reflectedLight.indirectDiffuse += vec3( 1.0 );
	#endif
	#include <aomap_fragment>
	reflectedLight.indirectDiffuse *= diffuseColor.rgb;
	vec3 outgoingLight = reflectedLight.indirectDiffuse;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Dm=`#define LAMBERT
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,Nm=`#define LAMBERT
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_lambert_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_lambert_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Um=`#define MATCAP
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <color_pars_vertex>
#include <displacementmap_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
	vViewPosition = - mvPosition.xyz;
}`,Fm=`#define MATCAP
uniform vec3 diffuse;
uniform float opacity;
uniform sampler2D matcap;
varying vec3 vViewPosition;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <normal_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	vec3 viewDir = normalize( vViewPosition );
	vec3 x = normalize( vec3( viewDir.z, 0.0, - viewDir.x ) );
	vec3 y = cross( viewDir, x );
	vec2 uv = vec2( dot( x, normal ), dot( y, normal ) ) * 0.495 + 0.5;
	#ifdef USE_MATCAP
		vec4 matcapColor = texture2D( matcap, uv );
	#else
		vec4 matcapColor = vec4( vec3( mix( 0.2, 0.8, uv.y ) ), 1.0 );
	#endif
	vec3 outgoingLight = diffuseColor.rgb * matcapColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Om=`#define NORMAL
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	varying vec3 vViewPosition;
#endif
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	vViewPosition = - mvPosition.xyz;
#endif
}`,Bm=`#define NORMAL
uniform float opacity;
#if defined( FLAT_SHADED ) || defined( USE_BUMPMAP ) || defined( USE_NORMALMAP_TANGENTSPACE )
	varying vec3 vViewPosition;
#endif
#include <uv_pars_fragment>
#include <normal_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( 0.0, 0.0, 0.0, opacity );
	#include <clipping_planes_fragment>
	#include <logdepthbuf_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	gl_FragColor = vec4( normalize( normal ) * 0.5 + 0.5, diffuseColor.a );
	#ifdef OPAQUE
		gl_FragColor.a = 1.0;
	#endif
}`,km=`#define PHONG
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <envmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <envmap_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,zm=`#define PHONG
uniform vec3 diffuse;
uniform vec3 emissive;
uniform vec3 specular;
uniform float shininess;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_phong_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <specularmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <specularmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_phong_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + reflectedLight.directSpecular + reflectedLight.indirectSpecular + totalEmissiveRadiance;
	#include <envmap_fragment>
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Gm=`#define STANDARD
varying vec3 vViewPosition;
#ifdef USE_TRANSMISSION
	varying vec3 vWorldPosition;
#endif
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
#ifdef USE_TRANSMISSION
	vWorldPosition = worldPosition.xyz;
#endif
}`,Hm=`#define STANDARD
#ifdef PHYSICAL
	#define IOR
	#define USE_SPECULAR
#endif
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float roughness;
uniform float metalness;
uniform float opacity;
#ifdef IOR
	uniform float ior;
#endif
#ifdef USE_SPECULAR
	uniform float specularIntensity;
	uniform vec3 specularColor;
	#ifdef USE_SPECULAR_COLORMAP
		uniform sampler2D specularColorMap;
	#endif
	#ifdef USE_SPECULAR_INTENSITYMAP
		uniform sampler2D specularIntensityMap;
	#endif
#endif
#ifdef USE_CLEARCOAT
	uniform float clearcoat;
	uniform float clearcoatRoughness;
#endif
#ifdef USE_DISPERSION
	uniform float dispersion;
#endif
#ifdef USE_IRIDESCENCE
	uniform float iridescence;
	uniform float iridescenceIOR;
	uniform float iridescenceThicknessMinimum;
	uniform float iridescenceThicknessMaximum;
#endif
#ifdef USE_SHEEN
	uniform vec3 sheenColor;
	uniform float sheenRoughness;
	#ifdef USE_SHEEN_COLORMAP
		uniform sampler2D sheenColorMap;
	#endif
	#ifdef USE_SHEEN_ROUGHNESSMAP
		uniform sampler2D sheenRoughnessMap;
	#endif
#endif
#ifdef USE_ANISOTROPY
	uniform vec2 anisotropyVector;
	#ifdef USE_ANISOTROPYMAP
		uniform sampler2D anisotropyMap;
	#endif
#endif
varying vec3 vViewPosition;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <iridescence_fragment>
#include <cube_uv_reflection_fragment>
#include <envmap_common_pars_fragment>
#include <envmap_physical_pars_fragment>
#include <fog_pars_fragment>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_physical_pars_fragment>
#include <transmission_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <clearcoat_pars_fragment>
#include <iridescence_pars_fragment>
#include <roughnessmap_pars_fragment>
#include <metalnessmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <roughnessmap_fragment>
	#include <metalnessmap_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <clearcoat_normal_fragment_begin>
	#include <clearcoat_normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_physical_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 totalDiffuse = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse;
	vec3 totalSpecular = reflectedLight.directSpecular + reflectedLight.indirectSpecular;
	#include <transmission_fragment>
	vec3 outgoingLight = totalDiffuse + totalSpecular + totalEmissiveRadiance;
	#ifdef USE_SHEEN
 
		outgoingLight = outgoingLight + sheenSpecularDirect + sheenSpecularIndirect;
 
 	#endif
	#ifdef USE_CLEARCOAT
		float dotNVcc = saturate( dot( geometryClearcoatNormal, geometryViewDir ) );
		vec3 Fcc = F_Schlick( material.clearcoatF0, material.clearcoatF90, dotNVcc );
		outgoingLight = outgoingLight * ( 1.0 - material.clearcoat * Fcc ) + ( clearcoatSpecularDirect + clearcoatSpecularIndirect ) * material.clearcoat;
	#endif
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Vm=`#define TOON
varying vec3 vViewPosition;
#include <common>
#include <batching_pars_vertex>
#include <uv_pars_vertex>
#include <displacementmap_pars_vertex>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <normal_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <shadowmap_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <normal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <displacementmap_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	vViewPosition = - mvPosition.xyz;
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,Wm=`#define TOON
uniform vec3 diffuse;
uniform vec3 emissive;
uniform float opacity;
#include <common>
#include <dithering_pars_fragment>
#include <color_pars_fragment>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <aomap_pars_fragment>
#include <lightmap_pars_fragment>
#include <emissivemap_pars_fragment>
#include <gradientmap_pars_fragment>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <normal_pars_fragment>
#include <lights_toon_pars_fragment>
#include <shadowmap_pars_fragment>
#include <bumpmap_pars_fragment>
#include <normalmap_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	ReflectedLight reflectedLight = ReflectedLight( vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ), vec3( 0.0 ) );
	vec3 totalEmissiveRadiance = emissive;
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <color_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	#include <normal_fragment_begin>
	#include <normal_fragment_maps>
	#include <emissivemap_fragment>
	#include <lights_toon_fragment>
	#include <lights_fragment_begin>
	#include <lights_fragment_maps>
	#include <lights_fragment_end>
	#include <aomap_fragment>
	vec3 outgoingLight = reflectedLight.directDiffuse + reflectedLight.indirectDiffuse + totalEmissiveRadiance;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
	#include <dithering_fragment>
}`,Xm=`uniform float size;
uniform float scale;
#include <common>
#include <color_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
#ifdef USE_POINTS_UV
	varying vec2 vUv;
	uniform mat3 uvTransform;
#endif
void main() {
	#ifdef USE_POINTS_UV
		vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	#endif
	#include <color_vertex>
	#include <morphinstance_vertex>
	#include <morphcolor_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <project_vertex>
	gl_PointSize = size;
	#ifdef USE_SIZEATTENUATION
		bool isPerspective = isPerspectiveMatrix( projectionMatrix );
		if ( isPerspective ) gl_PointSize *= ( scale / - mvPosition.z );
	#endif
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <worldpos_vertex>
	#include <fog_vertex>
}`,qm=`uniform vec3 diffuse;
uniform float opacity;
#include <common>
#include <color_pars_fragment>
#include <map_particle_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_particle_fragment>
	#include <color_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,Ym=`#include <common>
#include <batching_pars_vertex>
#include <fog_pars_vertex>
#include <morphtarget_pars_vertex>
#include <skinning_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <shadowmap_pars_vertex>
void main() {
	#include <batching_vertex>
	#include <beginnormal_vertex>
	#include <morphinstance_vertex>
	#include <morphnormal_vertex>
	#include <skinbase_vertex>
	#include <skinnormal_vertex>
	#include <defaultnormal_vertex>
	#include <begin_vertex>
	#include <morphtarget_vertex>
	#include <skinning_vertex>
	#include <project_vertex>
	#include <logdepthbuf_vertex>
	#include <worldpos_vertex>
	#include <shadowmap_vertex>
	#include <fog_vertex>
}`,$m=`uniform vec3 color;
uniform float opacity;
#include <common>
#include <fog_pars_fragment>
#include <bsdfs>
#include <lights_pars_begin>
#include <logdepthbuf_pars_fragment>
#include <shadowmap_pars_fragment>
#include <shadowmask_pars_fragment>
void main() {
	#include <logdepthbuf_fragment>
	gl_FragColor = vec4( color, opacity * ( 1.0 - getShadowMask() ) );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
	#include <premultiplied_alpha_fragment>
}`,Km=`uniform float rotation;
uniform vec2 center;
#include <common>
#include <uv_pars_vertex>
#include <fog_pars_vertex>
#include <logdepthbuf_pars_vertex>
#include <clipping_planes_pars_vertex>
void main() {
	#include <uv_vertex>
	vec4 mvPosition = modelViewMatrix[ 3 ];
	vec2 scale = vec2( length( modelMatrix[ 0 ].xyz ), length( modelMatrix[ 1 ].xyz ) );
	#ifndef USE_SIZEATTENUATION
		bool isPerspective = isPerspectiveMatrix( projectionMatrix );
		if ( isPerspective ) scale *= - mvPosition.z;
	#endif
	vec2 alignedPosition = ( position.xy - ( center - vec2( 0.5 ) ) ) * scale;
	vec2 rotatedPosition;
	rotatedPosition.x = cos( rotation ) * alignedPosition.x - sin( rotation ) * alignedPosition.y;
	rotatedPosition.y = sin( rotation ) * alignedPosition.x + cos( rotation ) * alignedPosition.y;
	mvPosition.xy += rotatedPosition;
	gl_Position = projectionMatrix * mvPosition;
	#include <logdepthbuf_vertex>
	#include <clipping_planes_vertex>
	#include <fog_vertex>
}`,Zm=`uniform vec3 diffuse;
uniform float opacity;
#include <common>
#include <uv_pars_fragment>
#include <map_pars_fragment>
#include <alphamap_pars_fragment>
#include <alphatest_pars_fragment>
#include <alphahash_pars_fragment>
#include <fog_pars_fragment>
#include <logdepthbuf_pars_fragment>
#include <clipping_planes_pars_fragment>
void main() {
	vec4 diffuseColor = vec4( diffuse, opacity );
	#include <clipping_planes_fragment>
	vec3 outgoingLight = vec3( 0.0 );
	#include <logdepthbuf_fragment>
	#include <map_fragment>
	#include <alphamap_fragment>
	#include <alphatest_fragment>
	#include <alphahash_fragment>
	outgoingLight = diffuseColor.rgb;
	#include <opaque_fragment>
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
	#include <fog_fragment>
}`,pt={alphahash_fragment:_h,alphahash_pars_fragment:vh,alphamap_fragment:xh,alphamap_pars_fragment:yh,alphatest_fragment:bh,alphatest_pars_fragment:Sh,aomap_fragment:Mh,aomap_pars_fragment:Eh,batching_pars_vertex:wh,batching_vertex:Th,begin_vertex:Ah,beginnormal_vertex:Rh,bsdfs:Ch,iridescence_fragment:Ph,bumpmap_pars_fragment:Lh,clipping_planes_fragment:Ih,clipping_planes_pars_fragment:Dh,clipping_planes_pars_vertex:Nh,clipping_planes_vertex:Uh,color_fragment:Fh,color_pars_fragment:Oh,color_pars_vertex:Bh,color_vertex:kh,common:zh,cube_uv_reflection_fragment:Gh,defaultnormal_vertex:Hh,displacementmap_pars_vertex:Vh,displacementmap_vertex:Wh,emissivemap_fragment:Xh,emissivemap_pars_fragment:qh,colorspace_fragment:Yh,colorspace_pars_fragment:$h,envmap_fragment:Kh,envmap_common_pars_fragment:Zh,envmap_pars_fragment:Jh,envmap_pars_vertex:Qh,envmap_physical_pars_fragment:cp,envmap_vertex:jh,fog_vertex:ep,fog_pars_vertex:tp,fog_fragment:np,fog_pars_fragment:ip,gradientmap_pars_fragment:rp,lightmap_pars_fragment:sp,lights_lambert_fragment:ap,lights_lambert_pars_fragment:op,lights_pars_begin:lp,lights_toon_fragment:dp,lights_toon_pars_fragment:up,lights_phong_fragment:fp,lights_phong_pars_fragment:hp,lights_physical_fragment:pp,lights_physical_pars_fragment:mp,lights_fragment_begin:gp,lights_fragment_maps:_p,lights_fragment_end:vp,lightprobes_pars_fragment:xp,logdepthbuf_fragment:yp,logdepthbuf_pars_fragment:bp,logdepthbuf_pars_vertex:Sp,logdepthbuf_vertex:Mp,map_fragment:Ep,map_pars_fragment:wp,map_particle_fragment:Tp,map_particle_pars_fragment:Ap,metalnessmap_fragment:Rp,metalnessmap_pars_fragment:Cp,morphinstance_vertex:Pp,morphcolor_vertex:Lp,morphnormal_vertex:Ip,morphtarget_pars_vertex:Dp,morphtarget_vertex:Np,normal_fragment_begin:Up,normal_fragment_maps:Fp,normal_pars_fragment:Op,normal_pars_vertex:Bp,normal_vertex:kp,normalmap_pars_fragment:zp,clearcoat_normal_fragment_begin:Gp,clearcoat_normal_fragment_maps:Hp,clearcoat_pars_fragment:Vp,iridescence_pars_fragment:Wp,opaque_fragment:Xp,packing:qp,premultiplied_alpha_fragment:Yp,project_vertex:$p,dithering_fragment:Kp,dithering_pars_fragment:Zp,roughnessmap_fragment:Jp,roughnessmap_pars_fragment:Qp,shadowmap_pars_fragment:jp,shadowmap_pars_vertex:em,shadowmap_vertex:tm,shadowmask_pars_fragment:nm,skinbase_vertex:im,skinning_pars_vertex:rm,skinning_vertex:sm,skinnormal_vertex:am,specularmap_fragment:om,specularmap_pars_fragment:lm,tonemapping_fragment:cm,tonemapping_pars_fragment:dm,transmission_fragment:um,transmission_pars_fragment:fm,uv_pars_fragment:hm,uv_pars_vertex:pm,uv_vertex:mm,worldpos_vertex:gm,background_vert:_m,background_frag:vm,backgroundCube_vert:xm,backgroundCube_frag:ym,cube_vert:bm,cube_frag:Sm,depth_vert:Mm,depth_frag:Em,distance_vert:wm,distance_frag:Tm,equirect_vert:Am,equirect_frag:Rm,linedashed_vert:Cm,linedashed_frag:Pm,meshbasic_vert:Lm,meshbasic_frag:Im,meshlambert_vert:Dm,meshlambert_frag:Nm,meshmatcap_vert:Um,meshmatcap_frag:Fm,meshnormal_vert:Om,meshnormal_frag:Bm,meshphong_vert:km,meshphong_frag:zm,meshphysical_vert:Gm,meshphysical_frag:Hm,meshtoon_vert:Vm,meshtoon_frag:Wm,points_vert:Xm,points_frag:qm,shadow_vert:Ym,shadow_frag:$m,sprite_vert:Km,sprite_frag:Zm},Ue={common:{diffuse:{value:new It(16777215)},opacity:{value:1},map:{value:null},mapTransform:{value:new ct},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0}},specularmap:{specularMap:{value:null},specularMapTransform:{value:new ct}},envmap:{envMap:{value:null},envMapRotation:{value:new ct},reflectivity:{value:1},ior:{value:1.5},refractionRatio:{value:.98},dfgLUT:{value:null}},aomap:{aoMap:{value:null},aoMapIntensity:{value:1},aoMapTransform:{value:new ct}},lightmap:{lightMap:{value:null},lightMapIntensity:{value:1},lightMapTransform:{value:new ct}},bumpmap:{bumpMap:{value:null},bumpMapTransform:{value:new ct},bumpScale:{value:1}},normalmap:{normalMap:{value:null},normalMapTransform:{value:new ct},normalScale:{value:new At(1,1)}},displacementmap:{displacementMap:{value:null},displacementMapTransform:{value:new ct},displacementScale:{value:1},displacementBias:{value:0}},emissivemap:{emissiveMap:{value:null},emissiveMapTransform:{value:new ct}},metalnessmap:{metalnessMap:{value:null},metalnessMapTransform:{value:new ct}},roughnessmap:{roughnessMap:{value:null},roughnessMapTransform:{value:new ct}},gradientmap:{gradientMap:{value:null}},fog:{fogDensity:{value:25e-5},fogNear:{value:1},fogFar:{value:2e3},fogColor:{value:new It(16777215)}},lights:{ambientLightColor:{value:[]},lightProbe:{value:[]},directionalLights:{value:[],properties:{direction:{},color:{}}},directionalLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},directionalShadowMatrix:{value:[]},spotLights:{value:[],properties:{color:{},position:{},direction:{},distance:{},coneCos:{},penumbraCos:{},decay:{}}},spotLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},spotLightMap:{value:[]},spotLightMatrix:{value:[]},pointLights:{value:[],properties:{color:{},position:{},decay:{},distance:{}}},pointLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{},shadowCameraNear:{},shadowCameraFar:{}}},pointShadowMatrix:{value:[]},hemisphereLights:{value:[],properties:{direction:{},skyColor:{},groundColor:{}}},rectAreaLights:{value:[],properties:{color:{},position:{},width:{},height:{}}},ltc_1:{value:null},ltc_2:{value:null},probesSH:{value:null},probesMin:{value:new le},probesMax:{value:new le},probesResolution:{value:new le}},points:{diffuse:{value:new It(16777215)},opacity:{value:1},size:{value:1},scale:{value:1},map:{value:null},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0},uvTransform:{value:new ct}},sprite:{diffuse:{value:new It(16777215)},opacity:{value:1},center:{value:new At(.5,.5)},rotation:{value:0},map:{value:null},mapTransform:{value:new ct},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0}}},Qn={basic:{uniforms:hn([Ue.common,Ue.specularmap,Ue.envmap,Ue.aomap,Ue.lightmap,Ue.fog]),vertexShader:pt.meshbasic_vert,fragmentShader:pt.meshbasic_frag},lambert:{uniforms:hn([Ue.common,Ue.specularmap,Ue.envmap,Ue.aomap,Ue.lightmap,Ue.emissivemap,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,Ue.fog,Ue.lights,{emissive:{value:new It(0)},envMapIntensity:{value:1}}]),vertexShader:pt.meshlambert_vert,fragmentShader:pt.meshlambert_frag},phong:{uniforms:hn([Ue.common,Ue.specularmap,Ue.envmap,Ue.aomap,Ue.lightmap,Ue.emissivemap,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,Ue.fog,Ue.lights,{emissive:{value:new It(0)},specular:{value:new It(1118481)},shininess:{value:30},envMapIntensity:{value:1}}]),vertexShader:pt.meshphong_vert,fragmentShader:pt.meshphong_frag},standard:{uniforms:hn([Ue.common,Ue.envmap,Ue.aomap,Ue.lightmap,Ue.emissivemap,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,Ue.roughnessmap,Ue.metalnessmap,Ue.fog,Ue.lights,{emissive:{value:new It(0)},roughness:{value:1},metalness:{value:0},envMapIntensity:{value:1}}]),vertexShader:pt.meshphysical_vert,fragmentShader:pt.meshphysical_frag},toon:{uniforms:hn([Ue.common,Ue.aomap,Ue.lightmap,Ue.emissivemap,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,Ue.gradientmap,Ue.fog,Ue.lights,{emissive:{value:new It(0)}}]),vertexShader:pt.meshtoon_vert,fragmentShader:pt.meshtoon_frag},matcap:{uniforms:hn([Ue.common,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,Ue.fog,{matcap:{value:null}}]),vertexShader:pt.meshmatcap_vert,fragmentShader:pt.meshmatcap_frag},points:{uniforms:hn([Ue.points,Ue.fog]),vertexShader:pt.points_vert,fragmentShader:pt.points_frag},dashed:{uniforms:hn([Ue.common,Ue.fog,{scale:{value:1},dashSize:{value:1},totalSize:{value:2}}]),vertexShader:pt.linedashed_vert,fragmentShader:pt.linedashed_frag},depth:{uniforms:hn([Ue.common,Ue.displacementmap]),vertexShader:pt.depth_vert,fragmentShader:pt.depth_frag},normal:{uniforms:hn([Ue.common,Ue.bumpmap,Ue.normalmap,Ue.displacementmap,{opacity:{value:1}}]),vertexShader:pt.meshnormal_vert,fragmentShader:pt.meshnormal_frag},sprite:{uniforms:hn([Ue.sprite,Ue.fog]),vertexShader:pt.sprite_vert,fragmentShader:pt.sprite_frag},background:{uniforms:{uvTransform:{value:new ct},t2D:{value:null},backgroundIntensity:{value:1}},vertexShader:pt.background_vert,fragmentShader:pt.background_frag},backgroundCube:{uniforms:{envMap:{value:null},backgroundBlurriness:{value:0},backgroundIntensity:{value:1},backgroundRotation:{value:new ct}},vertexShader:pt.backgroundCube_vert,fragmentShader:pt.backgroundCube_frag},cube:{uniforms:{tCube:{value:null},tFlip:{value:-1},opacity:{value:1}},vertexShader:pt.cube_vert,fragmentShader:pt.cube_frag},equirect:{uniforms:{tEquirect:{value:null}},vertexShader:pt.equirect_vert,fragmentShader:pt.equirect_frag},distance:{uniforms:hn([Ue.common,Ue.displacementmap,{referencePosition:{value:new le},nearDistance:{value:1},farDistance:{value:1e3}}]),vertexShader:pt.distance_vert,fragmentShader:pt.distance_frag},shadow:{uniforms:hn([Ue.lights,Ue.fog,{color:{value:new It(0)},opacity:{value:1}}]),vertexShader:pt.shadow_vert,fragmentShader:pt.shadow_frag}};Qn.physical={uniforms:hn([Qn.standard.uniforms,{clearcoat:{value:0},clearcoatMap:{value:null},clearcoatMapTransform:{value:new ct},clearcoatNormalMap:{value:null},clearcoatNormalMapTransform:{value:new ct},clearcoatNormalScale:{value:new At(1,1)},clearcoatRoughness:{value:0},clearcoatRoughnessMap:{value:null},clearcoatRoughnessMapTransform:{value:new ct},dispersion:{value:0},iridescence:{value:0},iridescenceMap:{value:null},iridescenceMapTransform:{value:new ct},iridescenceIOR:{value:1.3},iridescenceThicknessMinimum:{value:100},iridescenceThicknessMaximum:{value:400},iridescenceThicknessMap:{value:null},iridescenceThicknessMapTransform:{value:new ct},sheen:{value:0},sheenColor:{value:new It(0)},sheenColorMap:{value:null},sheenColorMapTransform:{value:new ct},sheenRoughness:{value:1},sheenRoughnessMap:{value:null},sheenRoughnessMapTransform:{value:new ct},transmission:{value:0},transmissionMap:{value:null},transmissionMapTransform:{value:new ct},transmissionSamplerSize:{value:new At},transmissionSamplerMap:{value:null},thickness:{value:0},thicknessMap:{value:null},thicknessMapTransform:{value:new ct},attenuationDistance:{value:0},attenuationColor:{value:new It(0)},specularColor:{value:new It(1,1,1)},specularColorMap:{value:null},specularColorMapTransform:{value:new ct},specularIntensity:{value:1},specularIntensityMap:{value:null},specularIntensityMapTransform:{value:new ct},anisotropyVector:{value:new At},anisotropyMap:{value:null},anisotropyMapTransform:{value:new ct}}]),vertexShader:pt.meshphysical_vert,fragmentShader:pt.meshphysical_frag};const Vs={r:0,b:0,g:0},Jm=new Zt,Bd=new ct;Bd.set(-1,0,0,0,1,0,0,0,1);function Qm(n,e,t,i,r,s){const a=new It(0);let o=r===!0?0:1,l,c,f=null,u=0,d=null;function h(C){let D=C.isScene===!0?C.background:null;if(D&&D.isTexture){const M=C.backgroundBlurriness>0;D=e.get(D,M)}return D}function _(C){let D=!1;const M=h(C);M===null?m(a,o):M&&M.isColor&&(m(M,1),D=!0);const L=n.xr.getEnvironmentBlendMode();L==="additive"?t.buffers.color.setClear(0,0,0,1,s):L==="alpha-blend"&&t.buffers.color.setClear(0,0,0,0,s),(n.autoClear||D)&&(t.buffers.depth.setTest(!0),t.buffers.depth.setMask(!0),t.buffers.color.setMask(!0),n.clear(n.autoClearColor,n.autoClearDepth,n.autoClearStencil))}function x(C,D){const M=h(D);M&&(M.isCubeTexture||M.mapping===ua)?(c===void 0&&(c=new ri(new ms(1,1,1),new si({name:"BackgroundCubeMaterial",uniforms:Hr(Qn.backgroundCube.uniforms),vertexShader:Qn.backgroundCube.vertexShader,fragmentShader:Qn.backgroundCube.fragmentShader,side:vn,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),c.geometry.deleteAttribute("normal"),c.geometry.deleteAttribute("uv"),c.onBeforeRender=function(L,A,P){this.matrixWorld.copyPosition(P.matrixWorld)},Object.defineProperty(c.material,"envMap",{get:function(){return this.uniforms.envMap.value}}),i.update(c)),c.material.uniforms.envMap.value=M,c.material.uniforms.backgroundBlurriness.value=D.backgroundBlurriness,c.material.uniforms.backgroundIntensity.value=D.backgroundIntensity,c.material.uniforms.backgroundRotation.value.setFromMatrix4(Jm.makeRotationFromEuler(D.backgroundRotation)).transpose(),M.isCubeTexture&&M.isRenderTargetTexture===!1&&c.material.uniforms.backgroundRotation.value.premultiply(Bd),c.material.toneMapped=yt.getTransfer(M.colorSpace)!==Dt,(f!==M||u!==M.version||d!==n.toneMapping)&&(c.material.needsUpdate=!0,f=M,u=M.version,d=n.toneMapping),c.layers.enableAll(),C.unshift(c,c.geometry,c.material,0,0,null)):M&&M.isTexture&&(l===void 0&&(l=new ri(new ha(2,2),new si({name:"BackgroundMaterial",uniforms:Hr(Qn.background.uniforms),vertexShader:Qn.background.vertexShader,fragmentShader:Qn.background.fragmentShader,side:Gi,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),l.geometry.deleteAttribute("normal"),Object.defineProperty(l.material,"map",{get:function(){return this.uniforms.t2D.value}}),i.update(l)),l.material.uniforms.t2D.value=M,l.material.uniforms.backgroundIntensity.value=D.backgroundIntensity,l.material.toneMapped=yt.getTransfer(M.colorSpace)!==Dt,M.matrixAutoUpdate===!0&&M.updateMatrix(),l.material.uniforms.uvTransform.value.copy(M.matrix),(f!==M||u!==M.version||d!==n.toneMapping)&&(l.material.needsUpdate=!0,f=M,u=M.version,d=n.toneMapping),l.layers.enableAll(),C.unshift(l,l.geometry,l.material,0,0,null))}function m(C,D){C.getRGB(Vs,Nd(n)),t.buffers.color.setClear(Vs.r,Vs.g,Vs.b,D,s)}function p(){c!==void 0&&(c.geometry.dispose(),c.material.dispose(),c=void 0),l!==void 0&&(l.geometry.dispose(),l.material.dispose(),l=void 0)}return{getClearColor:function(){return a},setClearColor:function(C,D=1){a.set(C),o=D,m(a,o)},getClearAlpha:function(){return o},setClearAlpha:function(C){o=C,m(a,o)},render:_,addToRenderList:x,dispose:p}}function jm(n,e){const t=n.getParameter(n.MAX_VERTEX_ATTRIBS),i={},r=d(null);let s=r,a=!1;function o(E,N,G,W,I){let R=!1;const F=u(E,W,G,N);s!==F&&(s=F,c(s.object)),R=h(E,W,G,I),R&&_(E,W,G,I),I!==null&&e.update(I,n.ELEMENT_ARRAY_BUFFER),(R||a)&&(a=!1,M(E,N,G,W),I!==null&&n.bindBuffer(n.ELEMENT_ARRAY_BUFFER,e.get(I).buffer))}function l(){return n.createVertexArray()}function c(E){return n.bindVertexArray(E)}function f(E){return n.deleteVertexArray(E)}function u(E,N,G,W){const I=W.wireframe===!0;let R=i[N.id];R===void 0&&(R={},i[N.id]=R);const F=E.isInstancedMesh===!0?E.id:0;let J=R[F];J===void 0&&(J={},R[F]=J);let B=J[G.id];B===void 0&&(B={},J[G.id]=B);let oe=B[I];return oe===void 0&&(oe=d(l()),B[I]=oe),oe}function d(E){const N=[],G=[],W=[];for(let I=0;I<t;I++)N[I]=0,G[I]=0,W[I]=0;return{geometry:null,program:null,wireframe:!1,newAttributes:N,enabledAttributes:G,attributeDivisors:W,object:E,attributes:{},index:null}}function h(E,N,G,W){const I=s.attributes,R=N.attributes;let F=0;const J=G.getAttributes();for(const B in J)if(J[B].location>=0){const k=I[B];let K=R[B];if(K===void 0&&(B==="instanceMatrix"&&E.instanceMatrix&&(K=E.instanceMatrix),B==="instanceColor"&&E.instanceColor&&(K=E.instanceColor)),k===void 0||k.attribute!==K||K&&k.data!==K.data)return!0;F++}return s.attributesNum!==F||s.index!==W}function _(E,N,G,W){const I={},R=N.attributes;let F=0;const J=G.getAttributes();for(const B in J)if(J[B].location>=0){let k=R[B];k===void 0&&(B==="instanceMatrix"&&E.instanceMatrix&&(k=E.instanceMatrix),B==="instanceColor"&&E.instanceColor&&(k=E.instanceColor));const K={};K.attribute=k,k&&k.data&&(K.data=k.data),I[B]=K,F++}s.attributes=I,s.attributesNum=F,s.index=W}function x(){const E=s.newAttributes;for(let N=0,G=E.length;N<G;N++)E[N]=0}function m(E){p(E,0)}function p(E,N){const G=s.newAttributes,W=s.enabledAttributes,I=s.attributeDivisors;G[E]=1,W[E]===0&&(n.enableVertexAttribArray(E),W[E]=1),I[E]!==N&&(n.vertexAttribDivisor(E,N),I[E]=N)}function C(){const E=s.newAttributes,N=s.enabledAttributes;for(let G=0,W=N.length;G<W;G++)N[G]!==E[G]&&(n.disableVertexAttribArray(G),N[G]=0)}function D(E,N,G,W,I,R,F){F===!0?n.vertexAttribIPointer(E,N,G,I,R):n.vertexAttribPointer(E,N,G,W,I,R)}function M(E,N,G,W){x();const I=W.attributes,R=G.getAttributes(),F=N.defaultAttributeValues;for(const J in R){const B=R[J];if(B.location>=0){let oe=I[J];if(oe===void 0&&(J==="instanceMatrix"&&E.instanceMatrix&&(oe=E.instanceMatrix),J==="instanceColor"&&E.instanceColor&&(oe=E.instanceColor)),oe!==void 0){const k=oe.normalized,K=oe.itemSize,Z=e.get(oe);if(Z===void 0)continue;const Y=Z.buffer,ne=Z.type,H=Z.bytesPerElement,te=ne===n.INT||ne===n.UNSIGNED_INT||oe.gpuType===ol;if(oe.isInterleavedBufferAttribute){const ie=oe.data,X=ie.stride,he=oe.offset;if(ie.isInstancedInterleavedBuffer){for(let _e=0;_e<B.locationSize;_e++)p(B.location+_e,ie.meshPerAttribute);E.isInstancedMesh!==!0&&W._maxInstanceCount===void 0&&(W._maxInstanceCount=ie.meshPerAttribute*ie.count)}else for(let _e=0;_e<B.locationSize;_e++)m(B.location+_e);n.bindBuffer(n.ARRAY_BUFFER,Y);for(let _e=0;_e<B.locationSize;_e++)D(B.location+_e,K/B.locationSize,ne,k,X*H,(he+K/B.locationSize*_e)*H,te)}else{if(oe.isInstancedBufferAttribute){for(let ie=0;ie<B.locationSize;ie++)p(B.location+ie,oe.meshPerAttribute);E.isInstancedMesh!==!0&&W._maxInstanceCount===void 0&&(W._maxInstanceCount=oe.meshPerAttribute*oe.count)}else for(let ie=0;ie<B.locationSize;ie++)m(B.location+ie);n.bindBuffer(n.ARRAY_BUFFER,Y);for(let ie=0;ie<B.locationSize;ie++)D(B.location+ie,K/B.locationSize,ne,k,K*H,K/B.locationSize*ie*H,te)}}else if(F!==void 0){const k=F[J];if(k!==void 0)switch(k.length){case 2:n.vertexAttrib2fv(B.location,k);break;case 3:n.vertexAttrib3fv(B.location,k);break;case 4:n.vertexAttrib4fv(B.location,k);break;default:n.vertexAttrib1fv(B.location,k)}}}}C()}function L(){b();for(const E in i){const N=i[E];for(const G in N){const W=N[G];for(const I in W){const R=W[I];for(const F in R)f(R[F].object),delete R[F];delete W[I]}}delete i[E]}}function A(E){if(i[E.id]===void 0)return;const N=i[E.id];for(const G in N){const W=N[G];for(const I in W){const R=W[I];for(const F in R)f(R[F].object),delete R[F];delete W[I]}}delete i[E.id]}function P(E){for(const N in i){const G=i[N];for(const W in G){const I=G[W];if(I[E.id]===void 0)continue;const R=I[E.id];for(const F in R)f(R[F].object),delete R[F];delete I[E.id]}}}function v(E){for(const N in i){const G=i[N],W=E.isInstancedMesh===!0?E.id:0,I=G[W];if(I!==void 0){for(const R in I){const F=I[R];for(const J in F)f(F[J].object),delete F[J];delete I[R]}delete G[W],Object.keys(G).length===0&&delete i[N]}}}function b(){w(),a=!0,s!==r&&(s=r,c(s.object))}function w(){r.geometry=null,r.program=null,r.wireframe=!1}return{setup:o,reset:b,resetDefaultState:w,dispose:L,releaseStatesOfGeometry:A,releaseStatesOfObject:v,releaseStatesOfProgram:P,initAttributes:x,enableAttribute:m,disableUnusedAttributes:C}}function eg(n,e,t){let i;function r(l){i=l}function s(l,c){n.drawArrays(i,l,c),t.update(c,i,1)}function a(l,c,f){f!==0&&(n.drawArraysInstanced(i,l,c,f),t.update(c,i,f))}function o(l,c,f){if(f===0)return;e.get("WEBGL_multi_draw").multiDrawArraysWEBGL(i,l,0,c,0,f);let d=0;for(let h=0;h<f;h++)d+=c[h];t.update(d,i,1)}this.setMode=r,this.render=s,this.renderInstances=a,this.renderMultiDraw=o}function tg(n,e,t,i){let r;function s(){if(r!==void 0)return r;if(e.has("EXT_texture_filter_anisotropic")===!0){const P=e.get("EXT_texture_filter_anisotropic");r=n.getParameter(P.MAX_TEXTURE_MAX_ANISOTROPY_EXT)}else r=0;return r}function a(P){return!(P!==Hn&&i.convert(P)!==n.getParameter(n.IMPLEMENTATION_COLOR_READ_FORMAT))}function o(P){const v=P===wi&&(e.has("EXT_color_buffer_half_float")||e.has("EXT_color_buffer_float"));return!(P!==In&&i.convert(P)!==n.getParameter(n.IMPLEMENTATION_COLOR_READ_TYPE)&&P!==ei&&!v)}function l(P){if(P==="highp"){if(n.getShaderPrecisionFormat(n.VERTEX_SHADER,n.HIGH_FLOAT).precision>0&&n.getShaderPrecisionFormat(n.FRAGMENT_SHADER,n.HIGH_FLOAT).precision>0)return"highp";P="mediump"}return P==="mediump"&&n.getShaderPrecisionFormat(n.VERTEX_SHADER,n.MEDIUM_FLOAT).precision>0&&n.getShaderPrecisionFormat(n.FRAGMENT_SHADER,n.MEDIUM_FLOAT).precision>0?"mediump":"lowp"}let c=t.precision!==void 0?t.precision:"highp";const f=l(c);f!==c&&(st("WebGLRenderer:",c,"not supported, using",f,"instead."),c=f);const u=t.logarithmicDepthBuffer===!0,d=t.reversedDepthBuffer===!0&&e.has("EXT_clip_control");t.reversedDepthBuffer===!0&&d===!1&&st("WebGLRenderer: Unable to use reversed depth buffer due to missing EXT_clip_control extension. Fallback to default depth buffer.");const h=n.getParameter(n.MAX_TEXTURE_IMAGE_UNITS),_=n.getParameter(n.MAX_VERTEX_TEXTURE_IMAGE_UNITS),x=n.getParameter(n.MAX_TEXTURE_SIZE),m=n.getParameter(n.MAX_CUBE_MAP_TEXTURE_SIZE),p=n.getParameter(n.MAX_VERTEX_ATTRIBS),C=n.getParameter(n.MAX_VERTEX_UNIFORM_VECTORS),D=n.getParameter(n.MAX_VARYING_VECTORS),M=n.getParameter(n.MAX_FRAGMENT_UNIFORM_VECTORS),L=n.getParameter(n.MAX_SAMPLES),A=n.getParameter(n.SAMPLES);return{isWebGL2:!0,getMaxAnisotropy:s,getMaxPrecision:l,textureFormatReadable:a,textureTypeReadable:o,precision:c,logarithmicDepthBuffer:u,reversedDepthBuffer:d,maxTextures:h,maxVertexTextures:_,maxTextureSize:x,maxCubemapSize:m,maxAttributes:p,maxVertexUniforms:C,maxVaryings:D,maxFragmentUniforms:M,maxSamples:L,samples:A}}function ng(n){const e=this;let t=null,i=0,r=!1,s=!1;const a=new er,o=new ct,l={value:null,needsUpdate:!1};this.uniform=l,this.numPlanes=0,this.numIntersection=0,this.init=function(u,d){const h=u.length!==0||d||i!==0||r;return r=d,i=u.length,h},this.beginShadows=function(){s=!0,f(null)},this.endShadows=function(){s=!1},this.setGlobalState=function(u,d){t=f(u,d,0)},this.setState=function(u,d,h){const _=u.clippingPlanes,x=u.clipIntersection,m=u.clipShadows,p=n.get(u);if(!r||_===null||_.length===0||s&&!m)s?f(null):c();else{const C=s?0:i,D=C*4;let M=p.clippingState||null;l.value=M,M=f(_,d,D,h);for(let L=0;L!==D;++L)M[L]=t[L];p.clippingState=M,this.numIntersection=x?this.numPlanes:0,this.numPlanes+=C}};function c(){l.value!==t&&(l.value=t,l.needsUpdate=i>0),e.numPlanes=i,e.numIntersection=0}function f(u,d,h,_){const x=u!==null?u.length:0;let m=null;if(x!==0){if(m=l.value,_!==!0||m===null){const p=h+x*4,C=d.matrixWorldInverse;o.getNormalMatrix(C),(m===null||m.length<p)&&(m=new Float32Array(p));for(let D=0,M=h;D!==x;++D,M+=4)a.copy(u[D]).applyMatrix4(C,o),a.normal.toArray(m,M),m[M+3]=a.constant}l.value=m,l.needsUpdate=!0}return e.numPlanes=x,e.numIntersection=0,m}}const ki=4,dc=[.125,.215,.35,.446,.526,.582],tr=20,ig=256,ns=new Fd,uc=new It;let $a=null,Ka=0,Za=0,Ja=!1;const rg=new le;class fc{constructor(e){this._renderer=e,this._pingPongRenderTarget=null,this._lodMax=0,this._cubeSize=0,this._sizeLods=[],this._sigmas=[],this._lodMeshes=[],this._backgroundBox=null,this._cubemapMaterial=null,this._equirectMaterial=null,this._blurMaterial=null,this._ggxMaterial=null}fromScene(e,t=0,i=.1,r=100,s={}){const{size:a=256,position:o=rg}=s;$a=this._renderer.getRenderTarget(),Ka=this._renderer.getActiveCubeFace(),Za=this._renderer.getActiveMipmapLevel(),Ja=this._renderer.xr.enabled,this._renderer.xr.enabled=!1,this._setSize(a);const l=this._allocateTargets();return l.depthBuffer=!0,this._sceneToCubeUV(e,i,r,l,o),t>0&&this._blur(l,0,0,t),this._applyPMREM(l),this._cleanup(l),l}fromEquirectangular(e,t=null){return this._fromTexture(e,t)}fromCubemap(e,t=null){return this._fromTexture(e,t)}compileCubemapShader(){this._cubemapMaterial===null&&(this._cubemapMaterial=mc(),this._compileMaterial(this._cubemapMaterial))}compileEquirectangularShader(){this._equirectMaterial===null&&(this._equirectMaterial=pc(),this._compileMaterial(this._equirectMaterial))}dispose(){this._dispose(),this._cubemapMaterial!==null&&this._cubemapMaterial.dispose(),this._equirectMaterial!==null&&this._equirectMaterial.dispose(),this._backgroundBox!==null&&(this._backgroundBox.geometry.dispose(),this._backgroundBox.material.dispose())}_setSize(e){this._lodMax=Math.floor(Math.log2(e)),this._cubeSize=Math.pow(2,this._lodMax)}_dispose(){this._blurMaterial!==null&&this._blurMaterial.dispose(),this._ggxMaterial!==null&&this._ggxMaterial.dispose(),this._pingPongRenderTarget!==null&&this._pingPongRenderTarget.dispose();for(let e=0;e<this._lodMeshes.length;e++)this._lodMeshes[e].geometry.dispose()}_cleanup(e){this._renderer.setRenderTarget($a,Ka,Za),this._renderer.xr.enabled=Ja,e.scissorTest=!1,Pr(e,0,0,e.width,e.height)}_fromTexture(e,t){e.mapping===ar||e.mapping===zr?this._setSize(e.image.length===0?16:e.image[0].width||e.image[0].image.width):this._setSize(e.image.width/4),$a=this._renderer.getRenderTarget(),Ka=this._renderer.getActiveCubeFace(),Za=this._renderer.getActiveMipmapLevel(),Ja=this._renderer.xr.enabled,this._renderer.xr.enabled=!1;const i=t||this._allocateTargets();return this._textureToCubeUV(e,i),this._applyPMREM(i),this._cleanup(i),i}_allocateTargets(){const e=3*Math.max(this._cubeSize,112),t=4*this._cubeSize,i={magFilter:ln,minFilter:ln,generateMipmaps:!1,type:wi,format:Hn,colorSpace:fs,depthBuffer:!1},r=hc(e,t,i);if(this._pingPongRenderTarget===null||this._pingPongRenderTarget.width!==e||this._pingPongRenderTarget.height!==t){this._pingPongRenderTarget!==null&&this._dispose(),this._pingPongRenderTarget=hc(e,t,i);const{_lodMax:s}=this;({lodMeshes:this._lodMeshes,sizeLods:this._sizeLods,sigmas:this._sigmas}=sg(s)),this._blurMaterial=og(s,e,t),this._ggxMaterial=ag(s,e,t)}return r}_compileMaterial(e){const t=new ri(new ai,e);this._renderer.compile(t,ns)}_sceneToCubeUV(e,t,i,r,s){const l=new zn(90,1,t,i),c=[1,-1,1,1,1,1],f=[1,1,1,-1,-1,-1],u=this._renderer,d=u.autoClear,h=u.toneMapping;u.getClearColor(uc),u.toneMapping=Vn,u.autoClear=!1,u.state.buffers.depth.getReversed()&&(u.setRenderTarget(r),u.clearDepth(),u.setRenderTarget(null)),this._backgroundBox===null&&(this._backgroundBox=new ri(new ms,new Pd({name:"PMREM.Background",side:vn,depthWrite:!1,depthTest:!1})));const x=this._backgroundBox,m=x.material;let p=!1;const C=e.background;C?C.isColor&&(m.color.copy(C),e.background=null,p=!0):(m.color.copy(uc),p=!0);for(let D=0;D<6;D++){const M=D%3;M===0?(l.up.set(0,c[D],0),l.position.set(s.x,s.y,s.z),l.lookAt(s.x+f[D],s.y,s.z)):M===1?(l.up.set(0,0,c[D]),l.position.set(s.x,s.y,s.z),l.lookAt(s.x,s.y+f[D],s.z)):(l.up.set(0,c[D],0),l.position.set(s.x,s.y,s.z),l.lookAt(s.x,s.y,s.z+f[D]));const L=this._cubeSize;Pr(r,M*L,D>2?L:0,L,L),u.setRenderTarget(r),p&&u.render(x,l),u.render(e,l)}u.toneMapping=h,u.autoClear=d,e.background=C}_textureToCubeUV(e,t){const i=this._renderer,r=e.mapping===ar||e.mapping===zr;r?(this._cubemapMaterial===null&&(this._cubemapMaterial=mc()),this._cubemapMaterial.uniforms.flipEnvMap.value=e.isRenderTargetTexture===!1?-1:1):this._equirectMaterial===null&&(this._equirectMaterial=pc());const s=r?this._cubemapMaterial:this._equirectMaterial,a=this._lodMeshes[0];a.material=s;const o=s.uniforms;o.envMap.value=e;const l=this._cubeSize;Pr(t,0,0,3*l,2*l),i.setRenderTarget(t),i.render(a,ns)}_applyPMREM(e){const t=this._renderer,i=t.autoClear;t.autoClear=!1;const r=this._lodMeshes.length;for(let s=1;s<r;s++)this._applyGGXFilter(e,s-1,s);t.autoClear=i}_applyGGXFilter(e,t,i){const r=this._renderer,s=this._pingPongRenderTarget,a=this._ggxMaterial,o=this._lodMeshes[i];o.material=a;const l=a.uniforms,c=i/(this._lodMeshes.length-1),f=t/(this._lodMeshes.length-1),u=Math.sqrt(c*c-f*f),d=0+c*1.25,h=u*d,{_lodMax:_}=this,x=this._sizeLods[i],m=3*x*(i>_-ki?i-_+ki:0),p=4*(this._cubeSize-x);l.envMap.value=e.texture,l.roughness.value=h,l.mipInt.value=_-t,Pr(s,m,p,3*x,2*x),r.setRenderTarget(s),r.render(o,ns),l.envMap.value=s.texture,l.roughness.value=0,l.mipInt.value=_-i,Pr(e,m,p,3*x,2*x),r.setRenderTarget(e),r.render(o,ns)}_blur(e,t,i,r,s){const a=this._pingPongRenderTarget;this._halfBlur(e,a,t,i,r,"latitudinal",s),this._halfBlur(a,e,i,i,r,"longitudinal",s)}_halfBlur(e,t,i,r,s,a,o){const l=this._renderer,c=this._blurMaterial;a!=="latitudinal"&&a!=="longitudinal"&&Et("blur direction must be either latitudinal or longitudinal!");const f=3,u=this._lodMeshes[r];u.material=c;const d=c.uniforms,h=this._sizeLods[i]-1,_=isFinite(s)?Math.PI/(2*h):2*Math.PI/(2*tr-1),x=s/_,m=isFinite(s)?1+Math.floor(f*x):tr;m>tr&&st(`sigmaRadians, ${s}, is too large and will clip, as it requested ${m} samples when the maximum is set to ${tr}`);const p=[];let C=0;for(let P=0;P<tr;++P){const v=P/x,b=Math.exp(-v*v/2);p.push(b),P===0?C+=b:P<m&&(C+=2*b)}for(let P=0;P<p.length;P++)p[P]=p[P]/C;d.envMap.value=e.texture,d.samples.value=m,d.weights.value=p,d.latitudinal.value=a==="latitudinal",o&&(d.poleAxis.value=o);const{_lodMax:D}=this;d.dTheta.value=_,d.mipInt.value=D-i;const M=this._sizeLods[r],L=3*M*(r>D-ki?r-D+ki:0),A=4*(this._cubeSize-M);Pr(t,L,A,3*M,2*M),l.setRenderTarget(t),l.render(u,ns)}}function sg(n){const e=[],t=[],i=[];let r=n;const s=n-ki+1+dc.length;for(let a=0;a<s;a++){const o=Math.pow(2,r);e.push(o);let l=1/o;a>n-ki?l=dc[a-n+ki-1]:a===0&&(l=0),t.push(l);const c=1/(o-2),f=-c,u=1+c,d=[f,f,u,f,u,u,f,f,u,u,f,u],h=6,_=6,x=3,m=2,p=1,C=new Float32Array(x*_*h),D=new Float32Array(m*_*h),M=new Float32Array(p*_*h);for(let A=0;A<h;A++){const P=A%3*2/3-1,v=A>2?0:-1,b=[P,v,0,P+2/3,v,0,P+2/3,v+1,0,P,v,0,P+2/3,v+1,0,P,v+1,0];C.set(b,x*_*A),D.set(d,m*_*A);const w=[A,A,A,A,A,A];M.set(w,p*_*A)}const L=new ai;L.setAttribute("position",new Wn(C,x)),L.setAttribute("uv",new Wn(D,m)),L.setAttribute("faceIndex",new Wn(M,p)),i.push(new ri(L,null)),r>ki&&r--}return{lodMeshes:i,sizeLods:e,sigmas:t}}function hc(n,e,t){const i=new ni(n,e,t);return i.texture.mapping=ua,i.texture.name="PMREM.cubeUv",i.scissorTest=!0,i}function Pr(n,e,t,i,r){n.viewport.set(e,t,i,r),n.scissor.set(e,t,i,r)}function ag(n,e,t){return new si({name:"PMREMGGXConvolution",defines:{GGX_SAMPLES:ig,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${n}.0`},uniforms:{envMap:{value:null},roughness:{value:0},mipInt:{value:0}},vertexShader:pa(),fragmentShader:`

			precision highp float;
			precision highp int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;
			uniform float roughness;
			uniform float mipInt;

			#define ENVMAP_TYPE_CUBE_UV
			#include <cube_uv_reflection_fragment>

			#define PI 3.14159265359

			// Van der Corput radical inverse
			float radicalInverse_VdC(uint bits) {
				bits = (bits << 16u) | (bits >> 16u);
				bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
				bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
				bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
				bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
				return float(bits) * 2.3283064365386963e-10; // / 0x100000000
			}

			// Hammersley sequence
			vec2 hammersley(uint i, uint N) {
				return vec2(float(i) / float(N), radicalInverse_VdC(i));
			}

			// GGX VNDF importance sampling (Eric Heitz 2018)
			// "Sampling the GGX Distribution of Visible Normals"
			// https://jcgt.org/published/0007/04/01/
			vec3 importanceSampleGGX_VNDF(vec2 Xi, vec3 V, float roughness) {
				float alpha = roughness * roughness;

				// Section 4.1: Orthonormal basis
				vec3 T1 = vec3(1.0, 0.0, 0.0);
				vec3 T2 = cross(V, T1);

				// Section 4.2: Parameterization of projected area
				float r = sqrt(Xi.x);
				float phi = 2.0 * PI * Xi.y;
				float t1 = r * cos(phi);
				float t2 = r * sin(phi);
				float s = 0.5 * (1.0 + V.z);
				t2 = (1.0 - s) * sqrt(1.0 - t1 * t1) + s * t2;

				// Section 4.3: Reprojection onto hemisphere
				vec3 Nh = t1 * T1 + t2 * T2 + sqrt(max(0.0, 1.0 - t1 * t1 - t2 * t2)) * V;

				// Section 3.4: Transform back to ellipsoid configuration
				return normalize(vec3(alpha * Nh.x, alpha * Nh.y, max(0.0, Nh.z)));
			}

			void main() {
				vec3 N = normalize(vOutputDirection);
				vec3 V = N; // Assume view direction equals normal for pre-filtering

				vec3 prefilteredColor = vec3(0.0);
				float totalWeight = 0.0;

				// For very low roughness, just sample the environment directly
				if (roughness < 0.001) {
					gl_FragColor = vec4(bilinearCubeUV(envMap, N, mipInt), 1.0);
					return;
				}

				// Tangent space basis for VNDF sampling
				vec3 up = abs(N.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
				vec3 tangent = normalize(cross(up, N));
				vec3 bitangent = cross(N, tangent);

				for(uint i = 0u; i < uint(GGX_SAMPLES); i++) {
					vec2 Xi = hammersley(i, uint(GGX_SAMPLES));

					// For PMREM, V = N, so in tangent space V is always (0, 0, 1)
					vec3 H_tangent = importanceSampleGGX_VNDF(Xi, vec3(0.0, 0.0, 1.0), roughness);

					// Transform H back to world space
					vec3 H = normalize(tangent * H_tangent.x + bitangent * H_tangent.y + N * H_tangent.z);
					vec3 L = normalize(2.0 * dot(V, H) * H - V);

					float NdotL = max(dot(N, L), 0.0);

					if(NdotL > 0.0) {
						// Sample environment at fixed mip level
						// VNDF importance sampling handles the distribution filtering
						vec3 sampleColor = bilinearCubeUV(envMap, L, mipInt);

						// Weight by NdotL for the split-sum approximation
						// VNDF PDF naturally accounts for the visible microfacet distribution
						prefilteredColor += sampleColor * NdotL;
						totalWeight += NdotL;
					}
				}

				if (totalWeight > 0.0) {
					prefilteredColor = prefilteredColor / totalWeight;
				}

				gl_FragColor = vec4(prefilteredColor, 1.0);
			}
		`,blending:bi,depthTest:!1,depthWrite:!1})}function og(n,e,t){const i=new Float32Array(tr),r=new le(0,1,0);return new si({name:"SphericalGaussianBlur",defines:{n:tr,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${n}.0`},uniforms:{envMap:{value:null},samples:{value:1},weights:{value:i},latitudinal:{value:!1},dTheta:{value:0},mipInt:{value:0},poleAxis:{value:r}},vertexShader:pa(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;
			uniform int samples;
			uniform float weights[ n ];
			uniform bool latitudinal;
			uniform float dTheta;
			uniform float mipInt;
			uniform vec3 poleAxis;

			#define ENVMAP_TYPE_CUBE_UV
			#include <cube_uv_reflection_fragment>

			vec3 getSample( float theta, vec3 axis ) {

				float cosTheta = cos( theta );
				// Rodrigues' axis-angle rotation
				vec3 sampleDirection = vOutputDirection * cosTheta
					+ cross( axis, vOutputDirection ) * sin( theta )
					+ axis * dot( axis, vOutputDirection ) * ( 1.0 - cosTheta );

				return bilinearCubeUV( envMap, sampleDirection, mipInt );

			}

			void main() {

				vec3 axis = latitudinal ? poleAxis : cross( poleAxis, vOutputDirection );

				if ( all( equal( axis, vec3( 0.0 ) ) ) ) {

					axis = vec3( vOutputDirection.z, 0.0, - vOutputDirection.x );

				}

				axis = normalize( axis );

				gl_FragColor = vec4( 0.0, 0.0, 0.0, 1.0 );
				gl_FragColor.rgb += weights[ 0 ] * getSample( 0.0, axis );

				for ( int i = 1; i < n; i++ ) {

					if ( i >= samples ) {

						break;

					}

					float theta = dTheta * float( i );
					gl_FragColor.rgb += weights[ i ] * getSample( -1.0 * theta, axis );
					gl_FragColor.rgb += weights[ i ] * getSample( theta, axis );

				}

			}
		`,blending:bi,depthTest:!1,depthWrite:!1})}function pc(){return new si({name:"EquirectangularToCubeUV",uniforms:{envMap:{value:null}},vertexShader:pa(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			varying vec3 vOutputDirection;

			uniform sampler2D envMap;

			#include <common>

			void main() {

				vec3 outputDirection = normalize( vOutputDirection );
				vec2 uv = equirectUv( outputDirection );

				gl_FragColor = vec4( texture2D ( envMap, uv ).rgb, 1.0 );

			}
		`,blending:bi,depthTest:!1,depthWrite:!1})}function mc(){return new si({name:"CubemapToCubeUV",uniforms:{envMap:{value:null},flipEnvMap:{value:-1}},vertexShader:pa(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			uniform float flipEnvMap;

			varying vec3 vOutputDirection;

			uniform samplerCube envMap;

			void main() {

				gl_FragColor = textureCube( envMap, vec3( flipEnvMap * vOutputDirection.x, vOutputDirection.yz ) );

			}
		`,blending:bi,depthTest:!1,depthWrite:!1})}function pa(){return`

		precision mediump float;
		precision mediump int;

		attribute float faceIndex;

		varying vec3 vOutputDirection;

		// RH coordinate system; PMREM face-indexing convention
		vec3 getDirection( vec2 uv, float face ) {

			uv = 2.0 * uv - 1.0;

			vec3 direction = vec3( uv, 1.0 );

			if ( face == 0.0 ) {

				direction = direction.zyx; // ( 1, v, u ) pos x

			} else if ( face == 1.0 ) {

				direction = direction.xzy;
				direction.xz *= -1.0; // ( -u, 1, -v ) pos y

			} else if ( face == 2.0 ) {

				direction.x *= -1.0; // ( -u, v, 1 ) pos z

			} else if ( face == 3.0 ) {

				direction = direction.zyx;
				direction.xz *= -1.0; // ( -1, v, -u ) neg x

			} else if ( face == 4.0 ) {

				direction = direction.xzy;
				direction.xy *= -1.0; // ( -u, -1, v ) neg y

			} else if ( face == 5.0 ) {

				direction.z *= -1.0; // ( u, v, -1 ) neg z

			}

			return direction;

		}

		void main() {

			vOutputDirection = getDirection( uv, faceIndex );
			gl_Position = vec4( position, 1.0 );

		}
	`}class kd extends ni{constructor(e=1,t={}){super(e,e,t),this.isWebGLCubeRenderTarget=!0;const i={width:e,height:e,depth:1},r=[i,i,i,i,i,i];this.texture=new Id(r),this._setTextureOptions(t),this.texture.isRenderTargetTexture=!0}fromEquirectangularTexture(e,t){this.texture.type=t.type,this.texture.colorSpace=t.colorSpace,this.texture.generateMipmaps=t.generateMipmaps,this.texture.minFilter=t.minFilter,this.texture.magFilter=t.magFilter;const i={uniforms:{tEquirect:{value:null}},vertexShader:`

				varying vec3 vWorldDirection;

				vec3 transformDirection( in vec3 dir, in mat4 matrix ) {

					return normalize( ( matrix * vec4( dir, 0.0 ) ).xyz );

				}

				void main() {

					vWorldDirection = transformDirection( position, modelMatrix );

					#include <begin_vertex>
					#include <project_vertex>

				}
			`,fragmentShader:`

				uniform sampler2D tEquirect;

				varying vec3 vWorldDirection;

				#include <common>

				void main() {

					vec3 direction = normalize( vWorldDirection );

					vec2 sampleUV = equirectUv( direction );

					gl_FragColor = texture2D( tEquirect, sampleUV );

				}
			`},r=new ms(5,5,5),s=new si({name:"CubemapFromEquirect",uniforms:Hr(i.uniforms),vertexShader:i.vertexShader,fragmentShader:i.fragmentShader,side:vn,blending:bi});s.uniforms.tEquirect.value=t;const a=new ri(r,s),o=t.minFilter;return t.minFilter===ir&&(t.minFilter=ln),new hh(1,10,this).update(e,a),t.minFilter=o,a.geometry.dispose(),a.material.dispose(),this}clear(e,t=!0,i=!0,r=!0){const s=e.getRenderTarget();for(let a=0;a<6;a++)e.setRenderTarget(this,a),e.clear(t,i,r);e.setRenderTarget(s)}}function lg(n){let e=new WeakMap,t=new WeakMap,i=null;function r(d,h=!1){return d==null?null:h?a(d):s(d)}function s(d){if(d&&d.isTexture){const h=d.mapping;if(h===ba||h===Sa)if(e.has(d)){const _=e.get(d).texture;return o(_,d.mapping)}else{const _=d.image;if(_&&_.height>0){const x=new kd(_.height);return x.fromEquirectangularTexture(n,d),e.set(d,x),d.addEventListener("dispose",c),o(x.texture,d.mapping)}else return null}}return d}function a(d){if(d&&d.isTexture){const h=d.mapping,_=h===ba||h===Sa,x=h===ar||h===zr;if(_||x){let m=t.get(d);const p=m!==void 0?m.texture.pmremVersion:0;if(d.isRenderTargetTexture&&d.pmremVersion!==p)return i===null&&(i=new fc(n)),m=_?i.fromEquirectangular(d,m):i.fromCubemap(d,m),m.texture.pmremVersion=d.pmremVersion,t.set(d,m),m.texture;if(m!==void 0)return m.texture;{const C=d.image;return _&&C&&C.height>0||x&&C&&l(C)?(i===null&&(i=new fc(n)),m=_?i.fromEquirectangular(d):i.fromCubemap(d),m.texture.pmremVersion=d.pmremVersion,t.set(d,m),d.addEventListener("dispose",f),m.texture):null}}}return d}function o(d,h){return h===ba?d.mapping=ar:h===Sa&&(d.mapping=zr),d}function l(d){let h=0;const _=6;for(let x=0;x<_;x++)d[x]!==void 0&&h++;return h===_}function c(d){const h=d.target;h.removeEventListener("dispose",c);const _=e.get(h);_!==void 0&&(e.delete(h),_.dispose())}function f(d){const h=d.target;h.removeEventListener("dispose",f);const _=t.get(h);_!==void 0&&(t.delete(h),_.dispose())}function u(){e=new WeakMap,t=new WeakMap,i!==null&&(i.dispose(),i=null)}return{get:r,dispose:u}}function cg(n){const e={};function t(i){if(e[i]!==void 0)return e[i];const r=n.getExtension(i);return e[i]=r,r}return{has:function(i){return t(i)!==null},init:function(){t("EXT_color_buffer_float"),t("WEBGL_clip_cull_distance"),t("OES_texture_float_linear"),t("EXT_color_buffer_half_float"),t("WEBGL_multisampled_render_to_texture"),t("WEBGL_render_shared_exponent")},get:function(i){const r=t(i);return r===null&&Fr("WebGLRenderer: "+i+" extension not supported."),r}}}function dg(n,e,t,i){const r={},s=new WeakMap;function a(u){const d=u.target;d.index!==null&&e.remove(d.index);for(const _ in d.attributes)e.remove(d.attributes[_]);d.removeEventListener("dispose",a),delete r[d.id];const h=s.get(d);h&&(e.remove(h),s.delete(d)),i.releaseStatesOfGeometry(d),d.isInstancedBufferGeometry===!0&&delete d._maxInstanceCount,t.memory.geometries--}function o(u,d){return r[d.id]===!0||(d.addEventListener("dispose",a),r[d.id]=!0,t.memory.geometries++),d}function l(u){const d=u.attributes;for(const h in d)e.update(d[h],n.ARRAY_BUFFER)}function c(u){const d=[],h=u.index,_=u.attributes.position;let x=0;if(_===void 0)return;if(h!==null){const C=h.array;x=h.version;for(let D=0,M=C.length;D<M;D+=3){const L=C[D+0],A=C[D+1],P=C[D+2];d.push(L,A,A,P,P,L)}}else{const C=_.array;x=_.version;for(let D=0,M=C.length/3-1;D<M;D+=3){const L=D+0,A=D+1,P=D+2;d.push(L,A,A,P,P,L)}}const m=new(_.count>=65535?Cd:Rd)(d,1);m.version=x;const p=s.get(u);p&&e.remove(p),s.set(u,m)}function f(u){const d=s.get(u);if(d){const h=u.index;h!==null&&d.version<h.version&&c(u)}else c(u);return s.get(u)}return{get:o,update:l,getWireframeAttribute:f}}function ug(n,e,t){let i;function r(u){i=u}let s,a;function o(u){s=u.type,a=u.bytesPerElement}function l(u,d){n.drawElements(i,d,s,u*a),t.update(d,i,1)}function c(u,d,h){h!==0&&(n.drawElementsInstanced(i,d,s,u*a,h),t.update(d,i,h))}function f(u,d,h){if(h===0)return;e.get("WEBGL_multi_draw").multiDrawElementsWEBGL(i,d,0,s,u,0,h);let x=0;for(let m=0;m<h;m++)x+=d[m];t.update(x,i,1)}this.setMode=r,this.setIndex=o,this.render=l,this.renderInstances=c,this.renderMultiDraw=f}function fg(n){const e={geometries:0,textures:0},t={frame:0,calls:0,triangles:0,points:0,lines:0};function i(s,a,o){switch(t.calls++,a){case n.TRIANGLES:t.triangles+=o*(s/3);break;case n.LINES:t.lines+=o*(s/2);break;case n.LINE_STRIP:t.lines+=o*(s-1);break;case n.LINE_LOOP:t.lines+=o*s;break;case n.POINTS:t.points+=o*s;break;default:Et("WebGLInfo: Unknown draw mode:",a);break}}function r(){t.calls=0,t.triangles=0,t.points=0,t.lines=0}return{memory:e,render:t,programs:null,autoReset:!0,reset:r,update:i}}function hg(n,e,t){const i=new WeakMap,r=new Xt;function s(a,o,l){const c=a.morphTargetInfluences,f=o.morphAttributes.position||o.morphAttributes.normal||o.morphAttributes.color,u=f!==void 0?f.length:0;let d=i.get(o);if(d===void 0||d.count!==u){let b=function(){P.dispose(),i.delete(o),o.removeEventListener("dispose",b)};d!==void 0&&d.texture.dispose();const h=o.morphAttributes.position!==void 0,_=o.morphAttributes.normal!==void 0,x=o.morphAttributes.color!==void 0,m=o.morphAttributes.position||[],p=o.morphAttributes.normal||[],C=o.morphAttributes.color||[];let D=0;h===!0&&(D=1),_===!0&&(D=2),x===!0&&(D=3);let M=o.attributes.position.count*D,L=1;M>e.maxTextureSize&&(L=Math.ceil(M/e.maxTextureSize),M=e.maxTextureSize);const A=new Float32Array(M*L*4*u),P=new wd(A,M,L,u);P.type=ei,P.needsUpdate=!0;const v=D*4;for(let w=0;w<u;w++){const E=m[w],N=p[w],G=C[w],W=M*L*4*w;for(let I=0;I<E.count;I++){const R=I*v;h===!0&&(r.fromBufferAttribute(E,I),A[W+R+0]=r.x,A[W+R+1]=r.y,A[W+R+2]=r.z,A[W+R+3]=0),_===!0&&(r.fromBufferAttribute(N,I),A[W+R+4]=r.x,A[W+R+5]=r.y,A[W+R+6]=r.z,A[W+R+7]=0),x===!0&&(r.fromBufferAttribute(G,I),A[W+R+8]=r.x,A[W+R+9]=r.y,A[W+R+10]=r.z,A[W+R+11]=G.itemSize===4?r.w:1)}}d={count:u,texture:P,size:new At(M,L)},i.set(o,d),o.addEventListener("dispose",b)}if(a.isInstancedMesh===!0&&a.morphTexture!==null)l.getUniforms().setValue(n,"morphTexture",a.morphTexture,t);else{let h=0;for(let x=0;x<c.length;x++)h+=c[x];const _=o.morphTargetsRelative?1:1-h;l.getUniforms().setValue(n,"morphTargetBaseInfluence",_),l.getUniforms().setValue(n,"morphTargetInfluences",c)}l.getUniforms().setValue(n,"morphTargetsTexture",d.texture,t),l.getUniforms().setValue(n,"morphTargetsTextureSize",d.size)}return{update:s}}function pg(n,e,t,i,r){let s=new WeakMap;function a(c){const f=r.render.frame,u=c.geometry,d=e.get(c,u);if(s.get(d)!==f&&(e.update(d),s.set(d,f)),c.isInstancedMesh&&(c.hasEventListener("dispose",l)===!1&&c.addEventListener("dispose",l),s.get(c)!==f&&(t.update(c.instanceMatrix,n.ARRAY_BUFFER),c.instanceColor!==null&&t.update(c.instanceColor,n.ARRAY_BUFFER),s.set(c,f))),c.isSkinnedMesh){const h=c.skeleton;s.get(h)!==f&&(h.update(),s.set(h,f))}return d}function o(){s=new WeakMap}function l(c){const f=c.target;f.removeEventListener("dispose",l),i.releaseStatesOfObject(f),t.remove(f.instanceMatrix),f.instanceColor!==null&&t.remove(f.instanceColor)}return{update:a,dispose:o}}const mg={[cd]:"LINEAR_TONE_MAPPING",[dd]:"REINHARD_TONE_MAPPING",[ud]:"CINEON_TONE_MAPPING",[fd]:"ACES_FILMIC_TONE_MAPPING",[pd]:"AGX_TONE_MAPPING",[md]:"NEUTRAL_TONE_MAPPING",[hd]:"CUSTOM_TONE_MAPPING"};function gg(n,e,t,i,r,s){const a=new ni(e,t,{type:n,depthBuffer:r,stencilBuffer:s,samples:i?4:0,depthTexture:r?new Gr(e,t):void 0}),o=new ni(e,t,{type:wi,depthBuffer:!1,stencilBuffer:!1}),l=new ai;l.setAttribute("position",new Mi([-1,3,0,-1,-1,0,3,-1,0],3)),l.setAttribute("uv",new Mi([0,2,0,0,2,0],2));const c=new Ud({uniforms:{tDiffuse:{value:null}},vertexShader:`
			precision highp float;

			uniform mat4 modelViewMatrix;
			uniform mat4 projectionMatrix;

			attribute vec3 position;
			attribute vec2 uv;

			varying vec2 vUv;

			void main() {
				vUv = uv;
				gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
			}`,fragmentShader:`
			precision highp float;

			uniform sampler2D tDiffuse;

			varying vec2 vUv;

			#include <tonemapping_pars_fragment>
			#include <colorspace_pars_fragment>

			void main() {
				gl_FragColor = texture2D( tDiffuse, vUv );

				#ifdef LINEAR_TONE_MAPPING
					gl_FragColor.rgb = LinearToneMapping( gl_FragColor.rgb );
				#elif defined( REINHARD_TONE_MAPPING )
					gl_FragColor.rgb = ReinhardToneMapping( gl_FragColor.rgb );
				#elif defined( CINEON_TONE_MAPPING )
					gl_FragColor.rgb = CineonToneMapping( gl_FragColor.rgb );
				#elif defined( ACES_FILMIC_TONE_MAPPING )
					gl_FragColor.rgb = ACESFilmicToneMapping( gl_FragColor.rgb );
				#elif defined( AGX_TONE_MAPPING )
					gl_FragColor.rgb = AgXToneMapping( gl_FragColor.rgb );
				#elif defined( NEUTRAL_TONE_MAPPING )
					gl_FragColor.rgb = NeutralToneMapping( gl_FragColor.rgb );
				#elif defined( CUSTOM_TONE_MAPPING )
					gl_FragColor.rgb = CustomToneMapping( gl_FragColor.rgb );
				#endif

				#ifdef SRGB_TRANSFER
					gl_FragColor = sRGBTransferOETF( gl_FragColor );
				#endif
			}`,depthTest:!1,depthWrite:!1}),f=new ri(l,c),u=new Fd(-1,1,1,-1,0,1);let d=null,h=null,_=!1,x,m=null,p=[],C=!1;this.setSize=function(D,M){a.setSize(D,M),o.setSize(D,M);for(let L=0;L<p.length;L++){const A=p[L];A.setSize&&A.setSize(D,M)}},this.setEffects=function(D){p=D,C=p.length>0&&p[0].isRenderPass===!0;const M=a.width,L=a.height;for(let A=0;A<p.length;A++){const P=p[A];P.setSize&&P.setSize(M,L)}},this.begin=function(D,M){if(_||D.toneMapping===Vn&&p.length===0)return!1;if(m=M,M!==null){const L=M.width,A=M.height;(a.width!==L||a.height!==A)&&this.setSize(L,A)}return C===!1&&D.setRenderTarget(a),x=D.toneMapping,D.toneMapping=Vn,!0},this.hasRenderPass=function(){return C},this.end=function(D,M){D.toneMapping=x,_=!0;let L=a,A=o;for(let P=0;P<p.length;P++){const v=p[P];if(v.enabled!==!1&&(v.render(D,A,L,M),v.needsSwap!==!1)){const b=L;L=A,A=b}}if(d!==D.outputColorSpace||h!==D.toneMapping){d=D.outputColorSpace,h=D.toneMapping,c.defines={},yt.getTransfer(d)===Dt&&(c.defines.SRGB_TRANSFER="");const P=mg[h];P&&(c.defines[P]=""),c.needsUpdate=!0}c.uniforms.tDiffuse.value=L.texture,D.setRenderTarget(m),D.render(f,u),m=null,_=!1},this.isCompositing=function(){return _},this.dispose=function(){a.depthTexture&&a.depthTexture.dispose(),a.dispose(),o.dispose(),l.dispose(),c.dispose()}}const zd=new un,Jo=new Gr(1,1),Gd=new wd,Hd=new kf,Vd=new Id,gc=[],_c=[],vc=new Float32Array(16),xc=new Float32Array(9),yc=new Float32Array(4);function Xr(n,e,t){const i=n[0];if(i<=0||i>0)return n;const r=e*t;let s=gc[r];if(s===void 0&&(s=new Float32Array(r),gc[r]=s),e!==0){i.toArray(s,0);for(let a=1,o=0;a!==e;++a)o+=t,n[a].toArray(s,o)}return s}function jt(n,e){if(n.length!==e.length)return!1;for(let t=0,i=n.length;t<i;t++)if(n[t]!==e[t])return!1;return!0}function en(n,e){for(let t=0,i=e.length;t<i;t++)n[t]=e[t]}function ma(n,e){let t=_c[e];t===void 0&&(t=new Int32Array(e),_c[e]=t);for(let i=0;i!==e;++i)t[i]=n.allocateTextureUnit();return t}function _g(n,e){const t=this.cache;t[0]!==e&&(n.uniform1f(this.addr,e),t[0]=e)}function vg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2f(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(jt(t,e))return;n.uniform2fv(this.addr,e),en(t,e)}}function xg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3f(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else if(e.r!==void 0)(t[0]!==e.r||t[1]!==e.g||t[2]!==e.b)&&(n.uniform3f(this.addr,e.r,e.g,e.b),t[0]=e.r,t[1]=e.g,t[2]=e.b);else{if(jt(t,e))return;n.uniform3fv(this.addr,e),en(t,e)}}function yg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4f(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(jt(t,e))return;n.uniform4fv(this.addr,e),en(t,e)}}function bg(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(jt(t,e))return;n.uniformMatrix2fv(this.addr,!1,e),en(t,e)}else{if(jt(t,i))return;yc.set(i),n.uniformMatrix2fv(this.addr,!1,yc),en(t,i)}}function Sg(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(jt(t,e))return;n.uniformMatrix3fv(this.addr,!1,e),en(t,e)}else{if(jt(t,i))return;xc.set(i),n.uniformMatrix3fv(this.addr,!1,xc),en(t,i)}}function Mg(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(jt(t,e))return;n.uniformMatrix4fv(this.addr,!1,e),en(t,e)}else{if(jt(t,i))return;vc.set(i),n.uniformMatrix4fv(this.addr,!1,vc),en(t,i)}}function Eg(n,e){const t=this.cache;t[0]!==e&&(n.uniform1i(this.addr,e),t[0]=e)}function wg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2i(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(jt(t,e))return;n.uniform2iv(this.addr,e),en(t,e)}}function Tg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3i(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(jt(t,e))return;n.uniform3iv(this.addr,e),en(t,e)}}function Ag(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4i(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(jt(t,e))return;n.uniform4iv(this.addr,e),en(t,e)}}function Rg(n,e){const t=this.cache;t[0]!==e&&(n.uniform1ui(this.addr,e),t[0]=e)}function Cg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2ui(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(jt(t,e))return;n.uniform2uiv(this.addr,e),en(t,e)}}function Pg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3ui(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(jt(t,e))return;n.uniform3uiv(this.addr,e),en(t,e)}}function Lg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4ui(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(jt(t,e))return;n.uniform4uiv(this.addr,e),en(t,e)}}function Ig(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r);let s;this.type===n.SAMPLER_2D_SHADOW?(Jo.compareFunction=t.isReversedDepthBuffer()?pl:hl,s=Jo):s=zd,t.setTexture2D(e||s,r)}function Dg(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTexture3D(e||Hd,r)}function Ng(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTextureCube(e||Vd,r)}function Ug(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTexture2DArray(e||Gd,r)}function Fg(n){switch(n){case 5126:return _g;case 35664:return vg;case 35665:return xg;case 35666:return yg;case 35674:return bg;case 35675:return Sg;case 35676:return Mg;case 5124:case 35670:return Eg;case 35667:case 35671:return wg;case 35668:case 35672:return Tg;case 35669:case 35673:return Ag;case 5125:return Rg;case 36294:return Cg;case 36295:return Pg;case 36296:return Lg;case 35678:case 36198:case 36298:case 36306:case 35682:return Ig;case 35679:case 36299:case 36307:return Dg;case 35680:case 36300:case 36308:case 36293:return Ng;case 36289:case 36303:case 36311:case 36292:return Ug}}function Og(n,e){n.uniform1fv(this.addr,e)}function Bg(n,e){const t=Xr(e,this.size,2);n.uniform2fv(this.addr,t)}function kg(n,e){const t=Xr(e,this.size,3);n.uniform3fv(this.addr,t)}function zg(n,e){const t=Xr(e,this.size,4);n.uniform4fv(this.addr,t)}function Gg(n,e){const t=Xr(e,this.size,4);n.uniformMatrix2fv(this.addr,!1,t)}function Hg(n,e){const t=Xr(e,this.size,9);n.uniformMatrix3fv(this.addr,!1,t)}function Vg(n,e){const t=Xr(e,this.size,16);n.uniformMatrix4fv(this.addr,!1,t)}function Wg(n,e){n.uniform1iv(this.addr,e)}function Xg(n,e){n.uniform2iv(this.addr,e)}function qg(n,e){n.uniform3iv(this.addr,e)}function Yg(n,e){n.uniform4iv(this.addr,e)}function $g(n,e){n.uniform1uiv(this.addr,e)}function Kg(n,e){n.uniform2uiv(this.addr,e)}function Zg(n,e){n.uniform3uiv(this.addr,e)}function Jg(n,e){n.uniform4uiv(this.addr,e)}function Qg(n,e,t){const i=this.cache,r=e.length,s=ma(t,r);jt(i,s)||(n.uniform1iv(this.addr,s),en(i,s));let a;this.type===n.SAMPLER_2D_SHADOW?a=Jo:a=zd;for(let o=0;o!==r;++o)t.setTexture2D(e[o]||a,s[o])}function jg(n,e,t){const i=this.cache,r=e.length,s=ma(t,r);jt(i,s)||(n.uniform1iv(this.addr,s),en(i,s));for(let a=0;a!==r;++a)t.setTexture3D(e[a]||Hd,s[a])}function e_(n,e,t){const i=this.cache,r=e.length,s=ma(t,r);jt(i,s)||(n.uniform1iv(this.addr,s),en(i,s));for(let a=0;a!==r;++a)t.setTextureCube(e[a]||Vd,s[a])}function t_(n,e,t){const i=this.cache,r=e.length,s=ma(t,r);jt(i,s)||(n.uniform1iv(this.addr,s),en(i,s));for(let a=0;a!==r;++a)t.setTexture2DArray(e[a]||Gd,s[a])}function n_(n){switch(n){case 5126:return Og;case 35664:return Bg;case 35665:return kg;case 35666:return zg;case 35674:return Gg;case 35675:return Hg;case 35676:return Vg;case 5124:case 35670:return Wg;case 35667:case 35671:return Xg;case 35668:case 35672:return qg;case 35669:case 35673:return Yg;case 5125:return $g;case 36294:return Kg;case 36295:return Zg;case 36296:return Jg;case 35678:case 36198:case 36298:case 36306:case 35682:return Qg;case 35679:case 36299:case 36307:return jg;case 35680:case 36300:case 36308:case 36293:return e_;case 36289:case 36303:case 36311:case 36292:return t_}}class i_{constructor(e,t,i){this.id=e,this.addr=i,this.cache=[],this.type=t.type,this.setValue=Fg(t.type)}}class r_{constructor(e,t,i){this.id=e,this.addr=i,this.cache=[],this.type=t.type,this.size=t.size,this.setValue=n_(t.type)}}class s_{constructor(e){this.id=e,this.seq=[],this.map={}}setValue(e,t,i){const r=this.seq;for(let s=0,a=r.length;s!==a;++s){const o=r[s];o.setValue(e,t[o.id],i)}}}const Qa=/(\w+)(\])?(\[|\.)?/g;function bc(n,e){n.seq.push(e),n.map[e.id]=e}function a_(n,e,t){const i=n.name,r=i.length;for(Qa.lastIndex=0;;){const s=Qa.exec(i),a=Qa.lastIndex;let o=s[1];const l=s[2]==="]",c=s[3];if(l&&(o=o|0),c===void 0||c==="["&&a+2===r){bc(t,c===void 0?new i_(o,n,e):new r_(o,n,e));break}else{let u=t.map[o];u===void 0&&(u=new s_(o),bc(t,u)),t=u}}}class ea{constructor(e,t){this.seq=[],this.map={};const i=e.getProgramParameter(t,e.ACTIVE_UNIFORMS);for(let a=0;a<i;++a){const o=e.getActiveUniform(t,a),l=e.getUniformLocation(t,o.name);a_(o,l,this)}const r=[],s=[];for(const a of this.seq)a.type===e.SAMPLER_2D_SHADOW||a.type===e.SAMPLER_CUBE_SHADOW||a.type===e.SAMPLER_2D_ARRAY_SHADOW?r.push(a):s.push(a);r.length>0&&(this.seq=r.concat(s))}setValue(e,t,i,r){const s=this.map[t];s!==void 0&&s.setValue(e,i,r)}setOptional(e,t,i){const r=t[i];r!==void 0&&this.setValue(e,i,r)}static upload(e,t,i,r){for(let s=0,a=t.length;s!==a;++s){const o=t[s],l=i[o.id];l.needsUpdate!==!1&&o.setValue(e,l.value,r)}}static seqWithValue(e,t){const i=[];for(let r=0,s=e.length;r!==s;++r){const a=e[r];a.id in t&&i.push(a)}return i}}function Sc(n,e,t){const i=n.createShader(e);return n.shaderSource(i,t),n.compileShader(i),i}const o_=37297;let l_=0;function c_(n,e){const t=n.split(`
`),i=[],r=Math.max(e-6,0),s=Math.min(e+6,t.length);for(let a=r;a<s;a++){const o=a+1;i.push(`${o===e?">":" "} ${o}: ${t[a]}`)}return i.join(`
`)}const Mc=new ct;function d_(n){yt._getMatrix(Mc,yt.workingColorSpace,n);const e=`mat3( ${Mc.elements.map(t=>t.toFixed(4))} )`;switch(yt.getTransfer(n)){case sa:return[e,"LinearTransferOETF"];case Dt:return[e,"sRGBTransferOETF"];default:return st("WebGLProgram: Unsupported color space: ",n),[e,"LinearTransferOETF"]}}function Ec(n,e,t){const i=n.getShaderParameter(e,n.COMPILE_STATUS),s=(n.getShaderInfoLog(e)||"").trim();if(i&&s==="")return"";const a=/ERROR: 0:(\d+)/.exec(s);if(a){const o=parseInt(a[1]);return t.toUpperCase()+`

`+s+`

`+c_(n.getShaderSource(e),o)}else return s}function u_(n,e){const t=d_(e);return[`vec4 ${n}( vec4 value ) {`,`	return ${t[1]}( vec4( value.rgb * ${t[0]}, value.a ) );`,"}"].join(`
`)}const f_={[cd]:"Linear",[dd]:"Reinhard",[ud]:"Cineon",[fd]:"ACESFilmic",[pd]:"AgX",[md]:"Neutral",[hd]:"Custom"};function h_(n,e){const t=f_[e];return t===void 0?(st("WebGLProgram: Unsupported toneMapping:",e),"vec3 "+n+"( vec3 color ) { return LinearToneMapping( color ); }"):"vec3 "+n+"( vec3 color ) { return "+t+"ToneMapping( color ); }"}const Ws=new le;function p_(){yt.getLuminanceCoefficients(Ws);const n=Ws.x.toFixed(4),e=Ws.y.toFixed(4),t=Ws.z.toFixed(4);return["float luminance( const in vec3 rgb ) {",`	const vec3 weights = vec3( ${n}, ${e}, ${t} );`,"	return dot( weights, rgb );","}"].join(`
`)}function m_(n){return[n.extensionClipCullDistance?"#extension GL_ANGLE_clip_cull_distance : require":"",n.extensionMultiDraw?"#extension GL_ANGLE_multi_draw : require":""].filter(as).join(`
`)}function g_(n){const e=[];for(const t in n){const i=n[t];i!==!1&&e.push("#define "+t+" "+i)}return e.join(`
`)}function __(n,e){const t={},i=n.getProgramParameter(e,n.ACTIVE_ATTRIBUTES);for(let r=0;r<i;r++){const s=n.getActiveAttrib(e,r),a=s.name;let o=1;s.type===n.FLOAT_MAT2&&(o=2),s.type===n.FLOAT_MAT3&&(o=3),s.type===n.FLOAT_MAT4&&(o=4),t[a]={type:s.type,location:n.getAttribLocation(e,a),locationSize:o}}return t}function as(n){return n!==""}function wc(n,e){const t=e.numSpotLightShadows+e.numSpotLightMaps-e.numSpotLightShadowsWithMaps;return n.replace(/NUM_DIR_LIGHTS/g,e.numDirLights).replace(/NUM_SPOT_LIGHTS/g,e.numSpotLights).replace(/NUM_SPOT_LIGHT_MAPS/g,e.numSpotLightMaps).replace(/NUM_SPOT_LIGHT_COORDS/g,t).replace(/NUM_RECT_AREA_LIGHTS/g,e.numRectAreaLights).replace(/NUM_POINT_LIGHTS/g,e.numPointLights).replace(/NUM_HEMI_LIGHTS/g,e.numHemiLights).replace(/NUM_DIR_LIGHT_SHADOWS/g,e.numDirLightShadows).replace(/NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS/g,e.numSpotLightShadowsWithMaps).replace(/NUM_SPOT_LIGHT_SHADOWS/g,e.numSpotLightShadows).replace(/NUM_POINT_LIGHT_SHADOWS/g,e.numPointLightShadows)}function Tc(n,e){return n.replace(/NUM_CLIPPING_PLANES/g,e.numClippingPlanes).replace(/UNION_CLIPPING_PLANES/g,e.numClippingPlanes-e.numClipIntersection)}const v_=/^[ \t]*#include +<([\w\d./]+)>/gm;function Qo(n){return n.replace(v_,y_)}const x_=new Map;function y_(n,e){let t=pt[e];if(t===void 0){const i=x_.get(e);if(i!==void 0)t=pt[i],st('WebGLRenderer: Shader chunk "%s" has been deprecated. Use "%s" instead.',e,i);else throw new Error("THREE.WebGLProgram: Can not resolve #include <"+e+">")}return Qo(t)}const b_=/#pragma unroll_loop_start\s+for\s*\(\s*int\s+i\s*=\s*(\d+)\s*;\s*i\s*<\s*(\d+)\s*;\s*i\s*\+\+\s*\)\s*{([\s\S]+?)}\s+#pragma unroll_loop_end/g;function Ac(n){return n.replace(b_,S_)}function S_(n,e,t,i){let r="";for(let s=parseInt(e);s<parseInt(t);s++)r+=i.replace(/\[\s*i\s*\]/g,"[ "+s+" ]").replace(/UNROLLED_LOOP_INDEX/g,s);return r}function Rc(n){let e=`precision ${n.precision} float;
	precision ${n.precision} int;
	precision ${n.precision} sampler2D;
	precision ${n.precision} samplerCube;
	precision ${n.precision} sampler3D;
	precision ${n.precision} sampler2DArray;
	precision ${n.precision} sampler2DShadow;
	precision ${n.precision} samplerCubeShadow;
	precision ${n.precision} sampler2DArrayShadow;
	precision ${n.precision} isampler2D;
	precision ${n.precision} isampler3D;
	precision ${n.precision} isamplerCube;
	precision ${n.precision} isampler2DArray;
	precision ${n.precision} usampler2D;
	precision ${n.precision} usampler3D;
	precision ${n.precision} usamplerCube;
	precision ${n.precision} usampler2DArray;
	`;return n.precision==="highp"?e+=`
#define HIGH_PRECISION`:n.precision==="mediump"?e+=`
#define MEDIUM_PRECISION`:n.precision==="lowp"&&(e+=`
#define LOW_PRECISION`),e}const M_={[Ks]:"SHADOWMAP_TYPE_PCF",[ss]:"SHADOWMAP_TYPE_VSM"};function E_(n){return M_[n.shadowMapType]||"SHADOWMAP_TYPE_BASIC"}const w_={[ar]:"ENVMAP_TYPE_CUBE",[zr]:"ENVMAP_TYPE_CUBE",[ua]:"ENVMAP_TYPE_CUBE_UV"};function T_(n){return n.envMap===!1?"ENVMAP_TYPE_CUBE":w_[n.envMapMode]||"ENVMAP_TYPE_CUBE"}const A_={[zr]:"ENVMAP_MODE_REFRACTION"};function R_(n){return n.envMap===!1?"ENVMAP_MODE_REFLECTION":A_[n.envMapMode]||"ENVMAP_MODE_REFLECTION"}const C_={[ld]:"ENVMAP_BLENDING_MULTIPLY",[_f]:"ENVMAP_BLENDING_MIX",[vf]:"ENVMAP_BLENDING_ADD"};function P_(n){return n.envMap===!1?"ENVMAP_BLENDING_NONE":C_[n.combine]||"ENVMAP_BLENDING_NONE"}function L_(n){const e=n.envMapCubeUVHeight;if(e===null)return null;const t=Math.log2(e)-2,i=1/e;return{texelWidth:1/(3*Math.max(Math.pow(2,t),112)),texelHeight:i,maxMip:t}}function I_(n,e,t,i){const r=n.getContext(),s=t.defines;let a=t.vertexShader,o=t.fragmentShader;const l=E_(t),c=T_(t),f=R_(t),u=P_(t),d=L_(t),h=m_(t),_=g_(s),x=r.createProgram();let m,p,C=t.glslVersion?"#version "+t.glslVersion+`
`:"";t.isRawShaderMaterial?(m=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_].filter(as).join(`
`),m.length>0&&(m+=`
`),p=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_].filter(as).join(`
`),p.length>0&&(p+=`
`)):(m=[Rc(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_,t.extensionClipCullDistance?"#define USE_CLIP_DISTANCE":"",t.batching?"#define USE_BATCHING":"",t.batchingColor?"#define USE_BATCHING_COLOR":"",t.instancing?"#define USE_INSTANCING":"",t.instancingColor?"#define USE_INSTANCING_COLOR":"",t.instancingMorph?"#define USE_INSTANCING_MORPH":"",t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.map?"#define USE_MAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+f:"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.displacementMap?"#define USE_DISPLACEMENTMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.mapUv?"#define MAP_UV "+t.mapUv:"",t.alphaMapUv?"#define ALPHAMAP_UV "+t.alphaMapUv:"",t.lightMapUv?"#define LIGHTMAP_UV "+t.lightMapUv:"",t.aoMapUv?"#define AOMAP_UV "+t.aoMapUv:"",t.emissiveMapUv?"#define EMISSIVEMAP_UV "+t.emissiveMapUv:"",t.bumpMapUv?"#define BUMPMAP_UV "+t.bumpMapUv:"",t.normalMapUv?"#define NORMALMAP_UV "+t.normalMapUv:"",t.displacementMapUv?"#define DISPLACEMENTMAP_UV "+t.displacementMapUv:"",t.metalnessMapUv?"#define METALNESSMAP_UV "+t.metalnessMapUv:"",t.roughnessMapUv?"#define ROUGHNESSMAP_UV "+t.roughnessMapUv:"",t.anisotropyMapUv?"#define ANISOTROPYMAP_UV "+t.anisotropyMapUv:"",t.clearcoatMapUv?"#define CLEARCOATMAP_UV "+t.clearcoatMapUv:"",t.clearcoatNormalMapUv?"#define CLEARCOAT_NORMALMAP_UV "+t.clearcoatNormalMapUv:"",t.clearcoatRoughnessMapUv?"#define CLEARCOAT_ROUGHNESSMAP_UV "+t.clearcoatRoughnessMapUv:"",t.iridescenceMapUv?"#define IRIDESCENCEMAP_UV "+t.iridescenceMapUv:"",t.iridescenceThicknessMapUv?"#define IRIDESCENCE_THICKNESSMAP_UV "+t.iridescenceThicknessMapUv:"",t.sheenColorMapUv?"#define SHEEN_COLORMAP_UV "+t.sheenColorMapUv:"",t.sheenRoughnessMapUv?"#define SHEEN_ROUGHNESSMAP_UV "+t.sheenRoughnessMapUv:"",t.specularMapUv?"#define SPECULARMAP_UV "+t.specularMapUv:"",t.specularColorMapUv?"#define SPECULAR_COLORMAP_UV "+t.specularColorMapUv:"",t.specularIntensityMapUv?"#define SPECULAR_INTENSITYMAP_UV "+t.specularIntensityMapUv:"",t.transmissionMapUv?"#define TRANSMISSIONMAP_UV "+t.transmissionMapUv:"",t.thicknessMapUv?"#define THICKNESSMAP_UV "+t.thicknessMapUv:"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexNormals?"#define HAS_NORMAL":"",t.vertexColors?"#define USE_COLOR":"",t.vertexAlphas?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.flatShading?"#define FLAT_SHADED":"",t.skinning?"#define USE_SKINNING":"",t.morphTargets?"#define USE_MORPHTARGETS":"",t.morphNormals&&t.flatShading===!1?"#define USE_MORPHNORMALS":"",t.morphColors?"#define USE_MORPHCOLORS":"",t.morphTargetsCount>0?"#define MORPHTARGETS_TEXTURE_STRIDE "+t.morphTextureStride:"",t.morphTargetsCount>0?"#define MORPHTARGETS_COUNT "+t.morphTargetsCount:"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+l:"",t.sizeAttenuation?"#define USE_SIZEATTENUATION":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 modelMatrix;","uniform mat4 modelViewMatrix;","uniform mat4 projectionMatrix;","uniform mat4 viewMatrix;","uniform mat3 normalMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;","#ifdef USE_INSTANCING","	attribute mat4 instanceMatrix;","#endif","#ifdef USE_INSTANCING_COLOR","	attribute vec3 instanceColor;","#endif","#ifdef USE_INSTANCING_MORPH","	uniform sampler2D morphTexture;","#endif","attribute vec3 position;","attribute vec3 normal;","attribute vec2 uv;","#ifdef USE_UV1","	attribute vec2 uv1;","#endif","#ifdef USE_UV2","	attribute vec2 uv2;","#endif","#ifdef USE_UV3","	attribute vec2 uv3;","#endif","#ifdef USE_TANGENT","	attribute vec4 tangent;","#endif","#if defined( USE_COLOR_ALPHA )","	attribute vec4 color;","#elif defined( USE_COLOR )","	attribute vec3 color;","#endif","#ifdef USE_SKINNING","	attribute vec4 skinIndex;","	attribute vec4 skinWeight;","#endif",`
`].filter(as).join(`
`),p=[Rc(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_,t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.alphaToCoverage?"#define ALPHA_TO_COVERAGE":"",t.map?"#define USE_MAP":"",t.matcap?"#define USE_MATCAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+c:"",t.envMap?"#define "+f:"",t.envMap?"#define "+u:"",d?"#define CUBEUV_TEXEL_WIDTH "+d.texelWidth:"",d?"#define CUBEUV_TEXEL_HEIGHT "+d.texelHeight:"",d?"#define CUBEUV_MAX_MIP "+d.maxMip+".0":"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.packedNormalMap?"#define USE_PACKED_NORMALMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoat?"#define USE_CLEARCOAT":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.dispersion?"#define USE_DISPERSION":"",t.iridescence?"#define USE_IRIDESCENCE":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaTest?"#define USE_ALPHATEST":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.sheen?"#define USE_SHEEN":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexColors||t.instancingColor?"#define USE_COLOR":"",t.vertexAlphas||t.batchingColor?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.gradientMap?"#define USE_GRADIENTMAP":"",t.flatShading?"#define FLAT_SHADED":"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+l:"",t.premultipliedAlpha?"#define PREMULTIPLIED_ALPHA":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.numLightProbeGrids>0?"#define USE_LIGHT_PROBES_GRID":"",t.decodeVideoTexture?"#define DECODE_VIDEO_TEXTURE":"",t.decodeVideoTextureEmissive?"#define DECODE_VIDEO_TEXTURE_EMISSIVE":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 viewMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;",t.toneMapping!==Vn?"#define TONE_MAPPING":"",t.toneMapping!==Vn?pt.tonemapping_pars_fragment:"",t.toneMapping!==Vn?h_("toneMapping",t.toneMapping):"",t.dithering?"#define DITHERING":"",t.opaque?"#define OPAQUE":"",pt.colorspace_pars_fragment,u_("linearToOutputTexel",t.outputColorSpace),p_(),t.useDepthPacking?"#define DEPTH_PACKING "+t.depthPacking:"",`
`].filter(as).join(`
`)),a=Qo(a),a=wc(a,t),a=Tc(a,t),o=Qo(o),o=wc(o,t),o=Tc(o,t),a=Ac(a),o=Ac(o),t.isRawShaderMaterial!==!0&&(C=`#version 300 es
`,m=[h,"#define attribute in","#define varying out","#define texture2D texture"].join(`
`)+`
`+m,p=["#define varying in",t.glslVersion===Ko?"":"layout(location = 0) out highp vec4 pc_fragColor;",t.glslVersion===Ko?"":"#define gl_FragColor pc_fragColor","#define gl_FragDepthEXT gl_FragDepth","#define texture2D texture","#define textureCube texture","#define texture2DProj textureProj","#define texture2DLodEXT textureLod","#define texture2DProjLodEXT textureProjLod","#define textureCubeLodEXT textureLod","#define texture2DGradEXT textureGrad","#define texture2DProjGradEXT textureProjGrad","#define textureCubeGradEXT textureGrad"].join(`
`)+`
`+p);const D=C+m+a,M=C+p+o,L=Sc(r,r.VERTEX_SHADER,D),A=Sc(r,r.FRAGMENT_SHADER,M);r.attachShader(x,L),r.attachShader(x,A),t.index0AttributeName!==void 0?r.bindAttribLocation(x,0,t.index0AttributeName):t.hasPositionAttribute===!0&&r.bindAttribLocation(x,0,"position"),r.linkProgram(x);function P(E){if(n.debug.checkShaderErrors){const N=r.getProgramInfoLog(x)||"",G=r.getShaderInfoLog(L)||"",W=r.getShaderInfoLog(A)||"",I=N.trim(),R=G.trim(),F=W.trim();let J=!0,B=!0;if(r.getProgramParameter(x,r.LINK_STATUS)===!1)if(J=!1,typeof n.debug.onShaderError=="function")n.debug.onShaderError(r,x,L,A);else{const oe=Ec(r,L,"vertex"),k=Ec(r,A,"fragment");Et("WebGLProgram: Shader Error "+r.getError()+" - VALIDATE_STATUS "+r.getProgramParameter(x,r.VALIDATE_STATUS)+`

Material Name: `+E.name+`
Material Type: `+E.type+`

Program Info Log: `+I+`
`+oe+`
`+k)}else I!==""?st("WebGLProgram: Program Info Log:",I):(R===""||F==="")&&(B=!1);B&&(E.diagnostics={runnable:J,programLog:I,vertexShader:{log:R,prefix:m},fragmentShader:{log:F,prefix:p}})}r.deleteShader(L),r.deleteShader(A),v=new ea(r,x),b=__(r,x)}let v;this.getUniforms=function(){return v===void 0&&P(this),v};let b;this.getAttributes=function(){return b===void 0&&P(this),b};let w=t.rendererExtensionParallelShaderCompile===!1;return this.isReady=function(){return w===!1&&(w=r.getProgramParameter(x,o_)),w},this.destroy=function(){i.releaseStatesOfProgram(this),r.deleteProgram(x),this.program=void 0},this.type=t.shaderType,this.name=t.shaderName,this.id=l_++,this.cacheKey=e,this.usedTimes=1,this.program=x,this.vertexShader=L,this.fragmentShader=A,this}let D_=0;class N_{constructor(){this.shaderCache=new Map,this.materialCache=new Map}update(e,t,i){const r=this._getShaderCacheForMaterial(e);return r.has(t)===!1&&(r.add(t),t.usedTimes++),r.has(i)===!1&&(r.add(i),i.usedTimes++),this}remove(e){const t=this.materialCache.get(e);for(const i of t)i.usedTimes--,i.usedTimes===0&&this.shaderCache.delete(i.code);return this.materialCache.delete(e),this}getVertexShaderStage(e){return this._getShaderStage(e.vertexShader)}getFragmentShaderStage(e){return this._getShaderStage(e.fragmentShader)}dispose(){this.shaderCache.clear(),this.materialCache.clear()}_getShaderCacheForMaterial(e){const t=this.materialCache;let i=t.get(e);return i===void 0&&(i=new Set,t.set(e,i)),i}_getShaderStage(e){const t=this.shaderCache;let i=t.get(e);return i===void 0&&(i=new U_(e),t.set(e,i)),i}}class U_{constructor(e){this.id=D_++,this.code=e,this.usedTimes=0}}function F_(n){return n===or||n===ia||n===ra}function O_(n,e,t,i,r,s){const a=new Td,o=new N_,l=new Set,c=[],f=new Map,u=i.logarithmicDepthBuffer;let d=i.precision;const h={MeshDepthMaterial:"depth",MeshDistanceMaterial:"distance",MeshNormalMaterial:"normal",MeshBasicMaterial:"basic",MeshLambertMaterial:"lambert",MeshPhongMaterial:"phong",MeshToonMaterial:"toon",MeshStandardMaterial:"physical",MeshPhysicalMaterial:"physical",MeshMatcapMaterial:"matcap",LineBasicMaterial:"basic",LineDashedMaterial:"dashed",PointsMaterial:"points",ShadowMaterial:"shadow",SpriteMaterial:"sprite"};function _(v){return l.add(v),v===0?"uv":`uv${v}`}function x(v,b,w,E,N,G){const W=E.fog,I=N.geometry,R=v.isMeshStandardMaterial||v.isMeshLambertMaterial||v.isMeshPhongMaterial?E.environment:null,F=v.isMeshStandardMaterial||v.isMeshLambertMaterial&&!v.envMap||v.isMeshPhongMaterial&&!v.envMap,J=e.get(v.envMap||R,F),B=J&&J.mapping===ua?J.image.height:null,oe=h[v.type];v.precision!==null&&(d=i.getMaxPrecision(v.precision),d!==v.precision&&st("WebGLProgram.getParameters:",v.precision,"not supported, using",d,"instead."));const k=I.morphAttributes.position||I.morphAttributes.normal||I.morphAttributes.color,K=k!==void 0?k.length:0;let Z=0;I.morphAttributes.position!==void 0&&(Z=1),I.morphAttributes.normal!==void 0&&(Z=2),I.morphAttributes.color!==void 0&&(Z=3);let Y,ne,H,te;if(oe){const We=Qn[oe];Y=We.vertexShader,ne=We.fragmentShader}else{Y=v.vertexShader,ne=v.fragmentShader;const We=o.getVertexShaderStage(v),Qe=o.getFragmentShaderStage(v);o.update(v,We,Qe),H=We.id,te=Qe.id}const ie=n.getRenderTarget(),X=n.state.buffers.depth.getReversed(),he=N.isInstancedMesh===!0,_e=N.isBatchedMesh===!0,we=!!v.map,ee=!!v.matcap,ce=!!J,$=!!v.aoMap,pe=!!v.lightMap,ye=!!v.bumpMap&&v.wireframe===!1,Te=!!v.normalMap,Pe=!!v.displacementMap,Ge=!!v.emissiveMap,Ee=!!v.metalnessMap,ve=!!v.roughnessMap,O=v.anisotropy>0,Xe=v.clearcoat>0,Ze=v.dispersion>0,y=v.iridescence>0,g=v.sheen>0,U=v.transmission>0,z=O&&!!v.anisotropyMap,Q=Xe&&!!v.clearcoatMap,ue=Xe&&!!v.clearcoatNormalMap,be=Xe&&!!v.clearcoatRoughnessMap,ae=y&&!!v.iridescenceMap,de=y&&!!v.iridescenceThicknessMap,Se=g&&!!v.sheenColorMap,ke=g&&!!v.sheenRoughnessMap,Ae=!!v.specularMap,Re=!!v.specularColorMap,He=!!v.specularIntensityMap,Ke=U&&!!v.transmissionMap,Je=U&&!!v.thicknessMap,q=!!v.gradientMap,Ce=!!v.alphaMap,xe=v.alphaTest>0,Le=!!v.alphaHash,Ne=!!v.extensions;let Me=Vn;v.toneMapped&&(ie===null||ie.isXRRenderTarget===!0)&&(Me=n.toneMapping);const qe={shaderID:oe,shaderType:v.type,shaderName:v.name,vertexShader:Y,fragmentShader:ne,defines:v.defines,customVertexShaderID:H,customFragmentShaderID:te,isRawShaderMaterial:v.isRawShaderMaterial===!0,glslVersion:v.glslVersion,precision:d,batching:_e,batchingColor:_e&&N._colorsTexture!==null,instancing:he,instancingColor:he&&N.instanceColor!==null,instancingMorph:he&&N.morphTexture!==null,outputColorSpace:ie===null?n.outputColorSpace:ie.isXRRenderTarget===!0?ie.texture.colorSpace:yt.workingColorSpace,alphaToCoverage:!!v.alphaToCoverage,map:we,matcap:ee,envMap:ce,envMapMode:ce&&J.mapping,envMapCubeUVHeight:B,aoMap:$,lightMap:pe,bumpMap:ye,normalMap:Te,displacementMap:Pe,emissiveMap:Ge,normalMapObjectSpace:Te&&v.normalMapType===bf,normalMapTangentSpace:Te&&v.normalMapType===Hl,packedNormalMap:Te&&v.normalMapType===Hl&&F_(v.normalMap.format),metalnessMap:Ee,roughnessMap:ve,anisotropy:O,anisotropyMap:z,clearcoat:Xe,clearcoatMap:Q,clearcoatNormalMap:ue,clearcoatRoughnessMap:be,dispersion:Ze,iridescence:y,iridescenceMap:ae,iridescenceThicknessMap:de,sheen:g,sheenColorMap:Se,sheenRoughnessMap:ke,specularMap:Ae,specularColorMap:Re,specularIntensityMap:He,transmission:U,transmissionMap:Ke,thicknessMap:Je,gradientMap:q,opaque:v.transparent===!1&&v.blending===Ur&&v.alphaToCoverage===!1,alphaMap:Ce,alphaTest:xe,alphaHash:Le,combine:v.combine,mapUv:we&&_(v.map.channel),aoMapUv:$&&_(v.aoMap.channel),lightMapUv:pe&&_(v.lightMap.channel),bumpMapUv:ye&&_(v.bumpMap.channel),normalMapUv:Te&&_(v.normalMap.channel),displacementMapUv:Pe&&_(v.displacementMap.channel),emissiveMapUv:Ge&&_(v.emissiveMap.channel),metalnessMapUv:Ee&&_(v.metalnessMap.channel),roughnessMapUv:ve&&_(v.roughnessMap.channel),anisotropyMapUv:z&&_(v.anisotropyMap.channel),clearcoatMapUv:Q&&_(v.clearcoatMap.channel),clearcoatNormalMapUv:ue&&_(v.clearcoatNormalMap.channel),clearcoatRoughnessMapUv:be&&_(v.clearcoatRoughnessMap.channel),iridescenceMapUv:ae&&_(v.iridescenceMap.channel),iridescenceThicknessMapUv:de&&_(v.iridescenceThicknessMap.channel),sheenColorMapUv:Se&&_(v.sheenColorMap.channel),sheenRoughnessMapUv:ke&&_(v.sheenRoughnessMap.channel),specularMapUv:Ae&&_(v.specularMap.channel),specularColorMapUv:Re&&_(v.specularColorMap.channel),specularIntensityMapUv:He&&_(v.specularIntensityMap.channel),transmissionMapUv:Ke&&_(v.transmissionMap.channel),thicknessMapUv:Je&&_(v.thicknessMap.channel),alphaMapUv:Ce&&_(v.alphaMap.channel),vertexTangents:!!I.attributes.tangent&&(Te||O),vertexNormals:!!I.attributes.normal,vertexColors:v.vertexColors,vertexAlphas:v.vertexColors===!0&&!!I.attributes.color&&I.attributes.color.itemSize===4,pointsUvs:N.isPoints===!0&&!!I.attributes.uv&&(we||Ce),fog:!!W,useFog:v.fog===!0,fogExp2:!!W&&W.isFogExp2,flatShading:v.wireframe===!1&&(v.flatShading===!0||I.attributes.normal===void 0&&Te===!1&&(v.isMeshLambertMaterial||v.isMeshPhongMaterial||v.isMeshStandardMaterial||v.isMeshPhysicalMaterial)),sizeAttenuation:v.sizeAttenuation===!0,logarithmicDepthBuffer:u,reversedDepthBuffer:X,skinning:N.isSkinnedMesh===!0,hasPositionAttribute:I.attributes.position!==void 0,morphTargets:I.morphAttributes.position!==void 0,morphNormals:I.morphAttributes.normal!==void 0,morphColors:I.morphAttributes.color!==void 0,morphTargetsCount:K,morphTextureStride:Z,numDirLights:b.directional.length,numPointLights:b.point.length,numSpotLights:b.spot.length,numSpotLightMaps:b.spotLightMap.length,numRectAreaLights:b.rectArea.length,numHemiLights:b.hemi.length,numDirLightShadows:b.directionalShadowMap.length,numPointLightShadows:b.pointShadowMap.length,numSpotLightShadows:b.spotShadowMap.length,numSpotLightShadowsWithMaps:b.numSpotLightShadowsWithMaps,numLightProbes:b.numLightProbes,numLightProbeGrids:G.length,numClippingPlanes:s.numPlanes,numClipIntersection:s.numIntersection,dithering:v.dithering,shadowMapEnabled:n.shadowMap.enabled&&w.length>0,shadowMapType:n.shadowMap.type,toneMapping:Me,decodeVideoTexture:we&&v.map.isVideoTexture===!0&&yt.getTransfer(v.map.colorSpace)===Dt,decodeVideoTextureEmissive:Ge&&v.emissiveMap.isVideoTexture===!0&&yt.getTransfer(v.emissiveMap.colorSpace)===Dt,premultipliedAlpha:v.premultipliedAlpha,doubleSided:v.side===gi,flipSided:v.side===vn,useDepthPacking:v.depthPacking>=0,depthPacking:v.depthPacking||0,index0AttributeName:v.index0AttributeName,extensionClipCullDistance:Ne&&v.extensions.clipCullDistance===!0&&t.has("WEBGL_clip_cull_distance"),extensionMultiDraw:(Ne&&v.extensions.multiDraw===!0||_e)&&t.has("WEBGL_multi_draw"),rendererExtensionParallelShaderCompile:t.has("KHR_parallel_shader_compile"),customProgramCacheKey:v.customProgramCacheKey()};return qe.vertexUv1s=l.has(1),qe.vertexUv2s=l.has(2),qe.vertexUv3s=l.has(3),l.clear(),qe}function m(v){const b=[];if(v.shaderID?b.push(v.shaderID):(b.push(v.customVertexShaderID),b.push(v.customFragmentShaderID)),v.defines!==void 0)for(const w in v.defines)b.push(w),b.push(v.defines[w]);return v.isRawShaderMaterial===!1&&(p(b,v),C(b,v),b.push(n.outputColorSpace)),b.push(v.customProgramCacheKey),b.join()}function p(v,b){v.push(b.precision),v.push(b.outputColorSpace),v.push(b.envMapMode),v.push(b.envMapCubeUVHeight),v.push(b.mapUv),v.push(b.alphaMapUv),v.push(b.lightMapUv),v.push(b.aoMapUv),v.push(b.bumpMapUv),v.push(b.normalMapUv),v.push(b.displacementMapUv),v.push(b.emissiveMapUv),v.push(b.metalnessMapUv),v.push(b.roughnessMapUv),v.push(b.anisotropyMapUv),v.push(b.clearcoatMapUv),v.push(b.clearcoatNormalMapUv),v.push(b.clearcoatRoughnessMapUv),v.push(b.iridescenceMapUv),v.push(b.iridescenceThicknessMapUv),v.push(b.sheenColorMapUv),v.push(b.sheenRoughnessMapUv),v.push(b.specularMapUv),v.push(b.specularColorMapUv),v.push(b.specularIntensityMapUv),v.push(b.transmissionMapUv),v.push(b.thicknessMapUv),v.push(b.combine),v.push(b.fogExp2),v.push(b.sizeAttenuation),v.push(b.morphTargetsCount),v.push(b.morphAttributeCount),v.push(b.numDirLights),v.push(b.numPointLights),v.push(b.numSpotLights),v.push(b.numSpotLightMaps),v.push(b.numHemiLights),v.push(b.numRectAreaLights),v.push(b.numDirLightShadows),v.push(b.numPointLightShadows),v.push(b.numSpotLightShadows),v.push(b.numSpotLightShadowsWithMaps),v.push(b.numLightProbes),v.push(b.shadowMapType),v.push(b.toneMapping),v.push(b.numClippingPlanes),v.push(b.numClipIntersection),v.push(b.depthPacking)}function C(v,b){a.disableAll(),b.instancing&&a.enable(0),b.instancingColor&&a.enable(1),b.instancingMorph&&a.enable(2),b.matcap&&a.enable(3),b.envMap&&a.enable(4),b.normalMapObjectSpace&&a.enable(5),b.normalMapTangentSpace&&a.enable(6),b.clearcoat&&a.enable(7),b.iridescence&&a.enable(8),b.alphaTest&&a.enable(9),b.vertexColors&&a.enable(10),b.vertexAlphas&&a.enable(11),b.vertexUv1s&&a.enable(12),b.vertexUv2s&&a.enable(13),b.vertexUv3s&&a.enable(14),b.vertexTangents&&a.enable(15),b.anisotropy&&a.enable(16),b.alphaHash&&a.enable(17),b.batching&&a.enable(18),b.dispersion&&a.enable(19),b.batchingColor&&a.enable(20),b.gradientMap&&a.enable(21),b.packedNormalMap&&a.enable(22),b.vertexNormals&&a.enable(23),v.push(a.mask),a.disableAll(),b.fog&&a.enable(0),b.useFog&&a.enable(1),b.flatShading&&a.enable(2),b.logarithmicDepthBuffer&&a.enable(3),b.reversedDepthBuffer&&a.enable(4),b.skinning&&a.enable(5),b.morphTargets&&a.enable(6),b.morphNormals&&a.enable(7),b.morphColors&&a.enable(8),b.premultipliedAlpha&&a.enable(9),b.shadowMapEnabled&&a.enable(10),b.doubleSided&&a.enable(11),b.flipSided&&a.enable(12),b.useDepthPacking&&a.enable(13),b.dithering&&a.enable(14),b.transmission&&a.enable(15),b.sheen&&a.enable(16),b.opaque&&a.enable(17),b.pointsUvs&&a.enable(18),b.decodeVideoTexture&&a.enable(19),b.decodeVideoTextureEmissive&&a.enable(20),b.alphaToCoverage&&a.enable(21),b.numLightProbeGrids>0&&a.enable(22),b.hasPositionAttribute&&a.enable(23),v.push(a.mask)}function D(v){const b=h[v.type];let w;if(b){const E=Qn[b];w=lh.clone(E.uniforms)}else w=v.uniforms;return w}function M(v,b){let w=f.get(b);return w!==void 0?++w.usedTimes:(w=new I_(n,b,v,r),c.push(w),f.set(b,w)),w}function L(v){if(--v.usedTimes===0){const b=c.indexOf(v);c[b]=c[c.length-1],c.pop(),f.delete(v.cacheKey),v.destroy()}}function A(v){o.remove(v)}function P(){o.dispose()}return{getParameters:x,getProgramCacheKey:m,getUniforms:D,acquireProgram:M,releaseProgram:L,releaseShaderCache:A,programs:c,dispose:P}}function B_(){let n=new WeakMap;function e(a){return n.has(a)}function t(a){let o=n.get(a);return o===void 0&&(o={},n.set(a,o)),o}function i(a){n.delete(a)}function r(a,o,l){n.get(a)[o]=l}function s(){n=new WeakMap}return{has:e,get:t,remove:i,update:r,dispose:s}}function k_(n,e){return n.groupOrder!==e.groupOrder?n.groupOrder-e.groupOrder:n.renderOrder!==e.renderOrder?n.renderOrder-e.renderOrder:n.material.id!==e.material.id?n.material.id-e.material.id:n.materialVariant!==e.materialVariant?n.materialVariant-e.materialVariant:n.z!==e.z?n.z-e.z:n.id-e.id}function Cc(n,e){return n.groupOrder!==e.groupOrder?n.groupOrder-e.groupOrder:n.renderOrder!==e.renderOrder?n.renderOrder-e.renderOrder:n.z!==e.z?e.z-n.z:n.id-e.id}function Pc(){const n=[];let e=0;const t=[],i=[],r=[];function s(){e=0,t.length=0,i.length=0,r.length=0}function a(d){let h=0;return d.isInstancedMesh&&(h+=2),d.isSkinnedMesh&&(h+=1),h}function o(d,h,_,x,m,p){let C=n[e];return C===void 0?(C={id:d.id,object:d,geometry:h,material:_,materialVariant:a(d),groupOrder:x,renderOrder:d.renderOrder,z:m,group:p},n[e]=C):(C.id=d.id,C.object=d,C.geometry=h,C.material=_,C.materialVariant=a(d),C.groupOrder=x,C.renderOrder=d.renderOrder,C.z=m,C.group=p),e++,C}function l(d,h,_,x,m,p){const C=o(d,h,_,x,m,p);_.transmission>0?i.push(C):_.transparent===!0?r.push(C):t.push(C)}function c(d,h,_,x,m,p){const C=o(d,h,_,x,m,p);_.transmission>0?i.unshift(C):_.transparent===!0?r.unshift(C):t.unshift(C)}function f(d,h,_){t.length>1&&t.sort(d||k_),i.length>1&&i.sort(h||Cc),r.length>1&&r.sort(h||Cc),_&&(t.reverse(),i.reverse(),r.reverse())}function u(){for(let d=e,h=n.length;d<h;d++){const _=n[d];if(_.id===null)break;_.id=null,_.object=null,_.geometry=null,_.material=null,_.group=null}}return{opaque:t,transmissive:i,transparent:r,init:s,push:l,unshift:c,finish:u,sort:f}}function z_(){let n=new WeakMap;function e(i,r){const s=n.get(i);let a;return s===void 0?(a=new Pc,n.set(i,[a])):r>=s.length?(a=new Pc,s.push(a)):a=s[r],a}function t(){n=new WeakMap}return{get:e,dispose:t}}function G_(){const n={};return{get:function(e){if(n[e.id]!==void 0)return n[e.id];let t;switch(e.type){case"DirectionalLight":t={direction:new le,color:new It};break;case"SpotLight":t={position:new le,direction:new le,color:new It,distance:0,coneCos:0,penumbraCos:0,decay:0};break;case"PointLight":t={position:new le,color:new It,distance:0,decay:0};break;case"HemisphereLight":t={direction:new le,skyColor:new It,groundColor:new It};break;case"RectAreaLight":t={color:new It,position:new le,halfWidth:new le,halfHeight:new le};break}return n[e.id]=t,t}}}function H_(){const n={};return{get:function(e){if(n[e.id]!==void 0)return n[e.id];let t;switch(e.type){case"DirectionalLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new At};break;case"SpotLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new At};break;case"PointLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new At,shadowCameraNear:1,shadowCameraFar:1e3};break}return n[e.id]=t,t}}}let V_=0;function W_(n,e){return(e.castShadow?2:0)-(n.castShadow?2:0)+(e.map?1:0)-(n.map?1:0)}function X_(n){const e=new G_,t=H_(),i={version:0,hash:{directionalLength:-1,pointLength:-1,spotLength:-1,rectAreaLength:-1,hemiLength:-1,numDirectionalShadows:-1,numPointShadows:-1,numSpotShadows:-1,numSpotMaps:-1,numLightProbes:-1},ambient:[0,0,0],probe:[],directional:[],directionalShadow:[],directionalShadowMap:[],directionalShadowMatrix:[],spot:[],spotLightMap:[],spotShadow:[],spotShadowMap:[],spotLightMatrix:[],rectArea:[],rectAreaLTC1:null,rectAreaLTC2:null,point:[],pointShadow:[],pointShadowMap:[],pointShadowMatrix:[],hemi:[],numSpotLightShadowsWithMaps:0,numLightProbes:0};for(let c=0;c<9;c++)i.probe.push(new le);const r=new le,s=new Zt,a=new Zt;function o(c){let f=0,u=0,d=0;for(let b=0;b<9;b++)i.probe[b].set(0,0,0);let h=0,_=0,x=0,m=0,p=0,C=0,D=0,M=0,L=0,A=0,P=0;c.sort(W_);for(let b=0,w=c.length;b<w;b++){const E=c[b],N=E.color,G=E.intensity,W=E.distance;let I=null;if(E.shadow&&E.shadow.map&&(E.shadow.map.texture.format===or?I=E.shadow.map.texture:I=E.shadow.map.depthTexture||E.shadow.map.texture),E.isAmbientLight)f+=N.r*G,u+=N.g*G,d+=N.b*G;else if(E.isLightProbe){for(let R=0;R<9;R++)i.probe[R].addScaledVector(E.sh.coefficients[R],G);P++}else if(E.isDirectionalLight){const R=e.get(E);if(R.color.copy(E.color).multiplyScalar(E.intensity),E.castShadow){const F=E.shadow,J=t.get(E);J.shadowIntensity=F.intensity,J.shadowBias=F.bias,J.shadowNormalBias=F.normalBias,J.shadowRadius=F.radius,J.shadowMapSize=F.mapSize,i.directionalShadow[h]=J,i.directionalShadowMap[h]=I,i.directionalShadowMatrix[h]=E.shadow.matrix,C++}i.directional[h]=R,h++}else if(E.isSpotLight){const R=e.get(E);R.position.setFromMatrixPosition(E.matrixWorld),R.color.copy(N).multiplyScalar(G),R.distance=W,R.coneCos=Math.cos(E.angle),R.penumbraCos=Math.cos(E.angle*(1-E.penumbra)),R.decay=E.decay,i.spot[x]=R;const F=E.shadow;if(E.map&&(i.spotLightMap[L]=E.map,L++,F.updateMatrices(E),E.castShadow&&A++),i.spotLightMatrix[x]=F.matrix,E.castShadow){const J=t.get(E);J.shadowIntensity=F.intensity,J.shadowBias=F.bias,J.shadowNormalBias=F.normalBias,J.shadowRadius=F.radius,J.shadowMapSize=F.mapSize,i.spotShadow[x]=J,i.spotShadowMap[x]=I,M++}x++}else if(E.isRectAreaLight){const R=e.get(E);R.color.copy(N).multiplyScalar(G),R.halfWidth.set(E.width*.5,0,0),R.halfHeight.set(0,E.height*.5,0),i.rectArea[m]=R,m++}else if(E.isPointLight){const R=e.get(E);if(R.color.copy(E.color).multiplyScalar(E.intensity),R.distance=E.distance,R.decay=E.decay,E.castShadow){const F=E.shadow,J=t.get(E);J.shadowIntensity=F.intensity,J.shadowBias=F.bias,J.shadowNormalBias=F.normalBias,J.shadowRadius=F.radius,J.shadowMapSize=F.mapSize,J.shadowCameraNear=F.camera.near,J.shadowCameraFar=F.camera.far,i.pointShadow[_]=J,i.pointShadowMap[_]=I,i.pointShadowMatrix[_]=E.shadow.matrix,D++}i.point[_]=R,_++}else if(E.isHemisphereLight){const R=e.get(E);R.skyColor.copy(E.color).multiplyScalar(G),R.groundColor.copy(E.groundColor).multiplyScalar(G),i.hemi[p]=R,p++}}m>0&&(n.has("OES_texture_float_linear")===!0?(i.rectAreaLTC1=Ue.LTC_FLOAT_1,i.rectAreaLTC2=Ue.LTC_FLOAT_2):(i.rectAreaLTC1=Ue.LTC_HALF_1,i.rectAreaLTC2=Ue.LTC_HALF_2)),i.ambient[0]=f,i.ambient[1]=u,i.ambient[2]=d;const v=i.hash;(v.directionalLength!==h||v.pointLength!==_||v.spotLength!==x||v.rectAreaLength!==m||v.hemiLength!==p||v.numDirectionalShadows!==C||v.numPointShadows!==D||v.numSpotShadows!==M||v.numSpotMaps!==L||v.numLightProbes!==P)&&(i.directional.length=h,i.spot.length=x,i.rectArea.length=m,i.point.length=_,i.hemi.length=p,i.directionalShadow.length=C,i.directionalShadowMap.length=C,i.pointShadow.length=D,i.pointShadowMap.length=D,i.spotShadow.length=M,i.spotShadowMap.length=M,i.directionalShadowMatrix.length=C,i.pointShadowMatrix.length=D,i.spotLightMatrix.length=M+L-A,i.spotLightMap.length=L,i.numSpotLightShadowsWithMaps=A,i.numLightProbes=P,v.directionalLength=h,v.pointLength=_,v.spotLength=x,v.rectAreaLength=m,v.hemiLength=p,v.numDirectionalShadows=C,v.numPointShadows=D,v.numSpotShadows=M,v.numSpotMaps=L,v.numLightProbes=P,i.version=V_++)}function l(c,f){let u=0,d=0,h=0,_=0,x=0;const m=f.matrixWorldInverse;for(let p=0,C=c.length;p<C;p++){const D=c[p];if(D.isDirectionalLight){const M=i.directional[u];M.direction.setFromMatrixPosition(D.matrixWorld),r.setFromMatrixPosition(D.target.matrixWorld),M.direction.sub(r),M.direction.transformDirection(m),u++}else if(D.isSpotLight){const M=i.spot[h];M.position.setFromMatrixPosition(D.matrixWorld),M.position.applyMatrix4(m),M.direction.setFromMatrixPosition(D.matrixWorld),r.setFromMatrixPosition(D.target.matrixWorld),M.direction.sub(r),M.direction.transformDirection(m),h++}else if(D.isRectAreaLight){const M=i.rectArea[_];M.position.setFromMatrixPosition(D.matrixWorld),M.position.applyMatrix4(m),a.identity(),s.copy(D.matrixWorld),s.premultiply(m),a.extractRotation(s),M.halfWidth.set(D.width*.5,0,0),M.halfHeight.set(0,D.height*.5,0),M.halfWidth.applyMatrix4(a),M.halfHeight.applyMatrix4(a),_++}else if(D.isPointLight){const M=i.point[d];M.position.setFromMatrixPosition(D.matrixWorld),M.position.applyMatrix4(m),d++}else if(D.isHemisphereLight){const M=i.hemi[x];M.direction.setFromMatrixPosition(D.matrixWorld),M.direction.transformDirection(m),x++}}}return{setup:o,setupView:l,state:i}}function Lc(n){const e=new X_(n),t=[],i=[],r=[];function s(d){u.camera=d,t.length=0,i.length=0,r.length=0}function a(d){t.push(d)}function o(d){i.push(d)}function l(d){r.push(d)}function c(){e.setup(t)}function f(d){e.setupView(t,d)}const u={lightsArray:t,shadowsArray:i,lightProbeGridArray:r,camera:null,lights:e,transmissionRenderTarget:{},textureUnits:0};return{init:s,state:u,setupLights:c,setupLightsView:f,pushLight:a,pushShadow:o,pushLightProbeGrid:l}}function q_(n){let e=new WeakMap;function t(r,s=0){const a=e.get(r);let o;return a===void 0?(o=new Lc(n),e.set(r,[o])):s>=a.length?(o=new Lc(n),a.push(o)):o=a[s],o}function i(){e=new WeakMap}return{get:t,dispose:i}}const Y_=`void main() {
	gl_Position = vec4( position, 1.0 );
}`,$_=`uniform sampler2D shadow_pass;
uniform vec2 resolution;
uniform float radius;
void main() {
	const float samples = float( VSM_SAMPLES );
	float mean = 0.0;
	float squared_mean = 0.0;
	float uvStride = samples <= 1.0 ? 0.0 : 2.0 / ( samples - 1.0 );
	float uvStart = samples <= 1.0 ? 0.0 : - 1.0;
	for ( float i = 0.0; i < samples; i ++ ) {
		float uvOffset = uvStart + i * uvStride;
		#ifdef HORIZONTAL_PASS
			vec2 distribution = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( uvOffset, 0.0 ) * radius ) / resolution ).rg;
			mean += distribution.x;
			squared_mean += distribution.y * distribution.y + distribution.x * distribution.x;
		#else
			float depth = texture2D( shadow_pass, ( gl_FragCoord.xy + vec2( 0.0, uvOffset ) * radius ) / resolution ).r;
			mean += depth;
			squared_mean += depth * depth;
		#endif
	}
	mean = mean / samples;
	squared_mean = squared_mean / samples;
	float std_dev = sqrt( max( 0.0, squared_mean - mean * mean ) );
	gl_FragColor = vec4( mean, std_dev, 0.0, 1.0 );
}`,K_=[new le(1,0,0),new le(-1,0,0),new le(0,1,0),new le(0,-1,0),new le(0,0,1),new le(0,0,-1)],Z_=[new le(0,-1,0),new le(0,-1,0),new le(0,0,1),new le(0,0,-1),new le(0,-1,0),new le(0,-1,0)],Ic=new Zt,is=new le,ja=new le;function J_(n,e,t){let i=new Ld;const r=new At,s=new At,a=new Xt,o=new uh,l=new fh,c={},f=t.maxTextureSize,u={[Gi]:vn,[vn]:Gi,[gi]:gi},d=new si({defines:{VSM_SAMPLES:8},uniforms:{shadow_pass:{value:null},resolution:{value:new At},radius:{value:4}},vertexShader:Y_,fragmentShader:$_}),h=d.clone();h.defines.HORIZONTAL_PASS=1;const _=new ai;_.setAttribute("position",new Wn(new Float32Array([-1,-1,.5,3,-1,.5,-1,3,.5]),3));const x=new ri(_,d),m=this;this.enabled=!1,this.autoUpdate=!0,this.needsUpdate=!1,this.type=Ks;let p=this.type;this.render=function(A,P,v){if(m.enabled===!1||m.autoUpdate===!1&&m.needsUpdate===!1||A.length===0)return;this.type===ef&&(st("WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead."),this.type=Ks);const b=n.getRenderTarget(),w=n.getActiveCubeFace(),E=n.getActiveMipmapLevel(),N=n.state;N.setBlending(bi),N.buffers.depth.getReversed()===!0?N.buffers.color.setClear(0,0,0,0):N.buffers.color.setClear(1,1,1,1),N.buffers.depth.setTest(!0),N.setScissorTest(!1);const G=p!==this.type;G&&P.traverse(function(W){W.material&&(Array.isArray(W.material)?W.material.forEach(I=>I.needsUpdate=!0):W.material.needsUpdate=!0)});for(let W=0,I=A.length;W<I;W++){const R=A[W],F=R.shadow;if(F===void 0){st("WebGLShadowMap:",R,"has no shadow.");continue}if(F.autoUpdate===!1&&F.needsUpdate===!1)continue;r.copy(F.mapSize);const J=F.getFrameExtents();r.multiply(J),s.copy(F.mapSize),(r.x>f||r.y>f)&&(r.x>f&&(s.x=Math.floor(f/J.x),r.x=s.x*J.x,F.mapSize.x=s.x),r.y>f&&(s.y=Math.floor(f/J.y),r.y=s.y*J.y,F.mapSize.y=s.y));const B=n.state.buffers.depth.getReversed();if(F.camera._reversedDepth=B,F.map===null||G===!0){if(F.map!==null&&(F.map.depthTexture!==null&&(F.map.depthTexture.dispose(),F.map.depthTexture=null),F.map.dispose()),this.type===ss){if(R.isPointLight){st("WebGLShadowMap: VSM shadow maps are not supported for PointLights. Use PCF or BasicShadowMap instead.");continue}F.map=new ni(r.x,r.y,{format:or,type:wi,minFilter:ln,magFilter:ln,generateMipmaps:!1}),F.map.texture.name=R.name+".shadowMap",F.map.depthTexture=new Gr(r.x,r.y,ei),F.map.depthTexture.name=R.name+".shadowMapDepth",F.map.depthTexture.format=Ti,F.map.depthTexture.compareFunction=null,F.map.depthTexture.minFilter=on,F.map.depthTexture.magFilter=on}else R.isPointLight?(F.map=new kd(r.x),F.map.depthTexture=new ah(r.x,ii)):(F.map=new ni(r.x,r.y),F.map.depthTexture=new Gr(r.x,r.y,ii)),F.map.depthTexture.name=R.name+".shadowMap",F.map.depthTexture.format=Ti,this.type===Ks?(F.map.depthTexture.compareFunction=B?pl:hl,F.map.depthTexture.minFilter=ln,F.map.depthTexture.magFilter=ln):(F.map.depthTexture.compareFunction=null,F.map.depthTexture.minFilter=on,F.map.depthTexture.magFilter=on);F.camera.updateProjectionMatrix()}const oe=F.map.isWebGLCubeRenderTarget?6:1;for(let k=0;k<oe;k++){if(F.map.isWebGLCubeRenderTarget)n.setRenderTarget(F.map,k),n.clear();else{k===0&&(n.setRenderTarget(F.map),n.clear());const K=F.getViewport(k);a.set(s.x*K.x,s.y*K.y,s.x*K.z,s.y*K.w),N.viewport(a)}if(R.isPointLight){const K=F.camera,Z=F.matrix,Y=R.distance||K.far;Y!==K.far&&(K.far=Y,K.updateProjectionMatrix()),is.setFromMatrixPosition(R.matrixWorld),K.position.copy(is),ja.copy(K.position),ja.add(K_[k]),K.up.copy(Z_[k]),K.lookAt(ja),K.updateMatrixWorld(),Z.makeTranslation(-is.x,-is.y,-is.z),Ic.multiplyMatrices(K.projectionMatrix,K.matrixWorldInverse),F._frustum.setFromProjectionMatrix(Ic,K.coordinateSystem,K.reversedDepth)}else F.updateMatrices(R);i=F.getFrustum(),M(P,v,F.camera,R,this.type)}F.isPointLightShadow!==!0&&this.type===ss&&C(F,v),F.needsUpdate=!1}p=this.type,m.needsUpdate=!1,n.setRenderTarget(b,w,E)};function C(A,P){const v=e.update(x);d.defines.VSM_SAMPLES!==A.blurSamples&&(d.defines.VSM_SAMPLES=A.blurSamples,h.defines.VSM_SAMPLES=A.blurSamples,d.needsUpdate=!0,h.needsUpdate=!0),A.mapPass===null&&(A.mapPass=new ni(r.x,r.y,{format:or,type:wi})),d.uniforms.shadow_pass.value=A.map.depthTexture,d.uniforms.resolution.value=A.mapSize,d.uniforms.radius.value=A.radius,n.setRenderTarget(A.mapPass),n.clear(),n.renderBufferDirect(P,null,v,d,x,null),h.uniforms.shadow_pass.value=A.mapPass.texture,h.uniforms.resolution.value=A.mapSize,h.uniforms.radius.value=A.radius,n.setRenderTarget(A.map),n.clear(),n.renderBufferDirect(P,null,v,h,x,null)}function D(A,P,v,b){let w=null;const E=v.isPointLight===!0?A.customDistanceMaterial:A.customDepthMaterial;if(E!==void 0)w=E;else if(w=v.isPointLight===!0?l:o,n.localClippingEnabled&&P.clipShadows===!0&&Array.isArray(P.clippingPlanes)&&P.clippingPlanes.length!==0||P.displacementMap&&P.displacementScale!==0||P.alphaMap&&P.alphaTest>0||P.map&&P.alphaTest>0||P.alphaToCoverage===!0){const N=w.uuid,G=P.uuid;let W=c[N];W===void 0&&(W={},c[N]=W);let I=W[G];I===void 0&&(I=w.clone(),W[G]=I,P.addEventListener("dispose",L)),w=I}if(w.visible=P.visible,w.wireframe=P.wireframe,b===ss?w.side=P.shadowSide!==null?P.shadowSide:P.side:w.side=P.shadowSide!==null?P.shadowSide:u[P.side],w.alphaMap=P.alphaMap,w.alphaTest=P.alphaToCoverage===!0?.5:P.alphaTest,w.map=P.map,w.clipShadows=P.clipShadows,w.clippingPlanes=P.clippingPlanes,w.clipIntersection=P.clipIntersection,w.displacementMap=P.displacementMap,w.displacementScale=P.displacementScale,w.displacementBias=P.displacementBias,w.wireframeLinewidth=P.wireframeLinewidth,w.linewidth=P.linewidth,v.isPointLight===!0&&w.isMeshDistanceMaterial===!0){const N=n.properties.get(w);N.light=v}return w}function M(A,P,v,b,w){if(A.visible===!1)return;if(A.layers.test(P.layers)&&(A.isMesh||A.isLine||A.isPoints)&&(A.castShadow||A.receiveShadow&&w===ss)&&(!A.frustumCulled||i.intersectsObject(A))){A.modelViewMatrix.multiplyMatrices(v.matrixWorldInverse,A.matrixWorld);const G=e.update(A),W=A.material;if(Array.isArray(W)){const I=G.groups;for(let R=0,F=I.length;R<F;R++){const J=I[R],B=W[J.materialIndex];if(B&&B.visible){const oe=D(A,B,b,w);A.onBeforeShadow(n,A,P,v,G,oe,J),n.renderBufferDirect(v,null,G,oe,A,J),A.onAfterShadow(n,A,P,v,G,oe,J)}}}else if(W.visible){const I=D(A,W,b,w);A.onBeforeShadow(n,A,P,v,G,I,null),n.renderBufferDirect(v,null,G,I,A,null),A.onAfterShadow(n,A,P,v,G,I,null)}}const N=A.children;for(let G=0,W=N.length;G<W;G++)M(N[G],P,v,b,w)}function L(A){A.target.removeEventListener("dispose",L);for(const v in c){const b=c[v],w=A.target.uuid;w in b&&(b[w].dispose(),delete b[w])}}}function Q_(n,e){function t(){let q=!1;const Ce=new Xt;let xe=null;const Le=new Xt(0,0,0,0);return{setMask:function(Ne){xe!==Ne&&!q&&(n.colorMask(Ne,Ne,Ne,Ne),xe=Ne)},setLocked:function(Ne){q=Ne},setClear:function(Ne,Me,qe,We,Qe){Qe===!0&&(Ne*=We,Me*=We,qe*=We),Ce.set(Ne,Me,qe,We),Le.equals(Ce)===!1&&(n.clearColor(Ne,Me,qe,We),Le.copy(Ce))},reset:function(){q=!1,xe=null,Le.set(-1,0,0,0)}}}function i(){let q=!1,Ce=!1,xe=null,Le=null,Ne=null;return{setReversed:function(Me){if(Ce!==Me){const qe=e.get("EXT_clip_control");Me?qe.clipControlEXT(qe.LOWER_LEFT_EXT,qe.ZERO_TO_ONE_EXT):qe.clipControlEXT(qe.LOWER_LEFT_EXT,qe.NEGATIVE_ONE_TO_ONE_EXT),Ce=Me;const We=Ne;Ne=null,this.setClear(We)}},getReversed:function(){return Ce},setTest:function(Me){Me?ie(n.DEPTH_TEST):X(n.DEPTH_TEST)},setMask:function(Me){xe!==Me&&!q&&(n.depthMask(Me),xe=Me)},setFunc:function(Me){if(Ce&&(Me=If[Me]),Le!==Me){switch(Me){case lo:n.depthFunc(n.NEVER);break;case co:n.depthFunc(n.ALWAYS);break;case uo:n.depthFunc(n.LESS);break;case kr:n.depthFunc(n.LEQUAL);break;case fo:n.depthFunc(n.EQUAL);break;case ho:n.depthFunc(n.GEQUAL);break;case po:n.depthFunc(n.GREATER);break;case mo:n.depthFunc(n.NOTEQUAL);break;default:n.depthFunc(n.LEQUAL)}Le=Me}},setLocked:function(Me){q=Me},setClear:function(Me){Ne!==Me&&(Ne=Me,Ce&&(Me=1-Me),n.clearDepth(Me))},reset:function(){q=!1,xe=null,Le=null,Ne=null,Ce=!1}}}function r(){let q=!1,Ce=null,xe=null,Le=null,Ne=null,Me=null,qe=null,We=null,Qe=null;return{setTest:function(wt){q||(wt?ie(n.STENCIL_TEST):X(n.STENCIL_TEST))},setMask:function(wt){Ce!==wt&&!q&&(n.stencilMask(wt),Ce=wt)},setFunc:function(wt,qt,St){(xe!==wt||Le!==qt||Ne!==St)&&(n.stencilFunc(wt,qt,St),xe=wt,Le=qt,Ne=St)},setOp:function(wt,qt,St){(Me!==wt||qe!==qt||We!==St)&&(n.stencilOp(wt,qt,St),Me=wt,qe=qt,We=St)},setLocked:function(wt){q=wt},setClear:function(wt){Qe!==wt&&(n.clearStencil(wt),Qe=wt)},reset:function(){q=!1,Ce=null,xe=null,Le=null,Ne=null,Me=null,qe=null,We=null,Qe=null}}}const s=new t,a=new i,o=new r,l=new WeakMap,c=new WeakMap;let f={},u={},d={},h=new WeakMap,_=[],x=null,m=!1,p=null,C=null,D=null,M=null,L=null,A=null,P=null,v=new It(0,0,0),b=0,w=!1,E=null,N=null,G=null,W=null,I=null;const R=n.getParameter(n.MAX_COMBINED_TEXTURE_IMAGE_UNITS);let F=!1,J=0;const B=n.getParameter(n.VERSION);B.indexOf("WebGL")!==-1?(J=parseFloat(/^WebGL (\d)/.exec(B)[1]),F=J>=1):B.indexOf("OpenGL ES")!==-1&&(J=parseFloat(/^OpenGL ES (\d)/.exec(B)[1]),F=J>=2);let oe=null,k={};const K=n.getParameter(n.SCISSOR_BOX),Z=n.getParameter(n.VIEWPORT),Y=new Xt().fromArray(K),ne=new Xt().fromArray(Z);function H(q,Ce,xe,Le){const Ne=new Uint8Array(4),Me=n.createTexture();n.bindTexture(q,Me),n.texParameteri(q,n.TEXTURE_MIN_FILTER,n.NEAREST),n.texParameteri(q,n.TEXTURE_MAG_FILTER,n.NEAREST);for(let qe=0;qe<xe;qe++)q===n.TEXTURE_3D||q===n.TEXTURE_2D_ARRAY?n.texImage3D(Ce,0,n.RGBA,1,1,Le,0,n.RGBA,n.UNSIGNED_BYTE,Ne):n.texImage2D(Ce+qe,0,n.RGBA,1,1,0,n.RGBA,n.UNSIGNED_BYTE,Ne);return Me}const te={};te[n.TEXTURE_2D]=H(n.TEXTURE_2D,n.TEXTURE_2D,1),te[n.TEXTURE_CUBE_MAP]=H(n.TEXTURE_CUBE_MAP,n.TEXTURE_CUBE_MAP_POSITIVE_X,6),te[n.TEXTURE_2D_ARRAY]=H(n.TEXTURE_2D_ARRAY,n.TEXTURE_2D_ARRAY,1,1),te[n.TEXTURE_3D]=H(n.TEXTURE_3D,n.TEXTURE_3D,1,1),s.setClear(0,0,0,1),a.setClear(1),o.setClear(0),ie(n.DEPTH_TEST),a.setFunc(kr),ye(!1),Te(Bl),ie(n.CULL_FACE),$(bi);function ie(q){f[q]!==!0&&(n.enable(q),f[q]=!0)}function X(q){f[q]!==!1&&(n.disable(q),f[q]=!1)}function he(q,Ce){return d[q]!==Ce?(n.bindFramebuffer(q,Ce),d[q]=Ce,q===n.DRAW_FRAMEBUFFER&&(d[n.FRAMEBUFFER]=Ce),q===n.FRAMEBUFFER&&(d[n.DRAW_FRAMEBUFFER]=Ce),!0):!1}function _e(q,Ce){let xe=_,Le=!1;if(q){xe=h.get(Ce),xe===void 0&&(xe=[],h.set(Ce,xe));const Ne=q.textures;if(xe.length!==Ne.length||xe[0]!==n.COLOR_ATTACHMENT0){for(let Me=0,qe=Ne.length;Me<qe;Me++)xe[Me]=n.COLOR_ATTACHMENT0+Me;xe.length=Ne.length,Le=!0}}else xe[0]!==n.BACK&&(xe[0]=n.BACK,Le=!0);Le&&n.drawBuffers(xe)}function we(q){return x!==q?(n.useProgram(q),x=q,!0):!1}const ee={[Bi]:n.FUNC_ADD,[tf]:n.FUNC_SUBTRACT,[nf]:n.FUNC_REVERSE_SUBTRACT};ee[rf]=n.MIN,ee[sf]=n.MAX;const ce={[af]:n.ZERO,[ao]:n.ONE,[of]:n.SRC_COLOR,[oo]:n.SRC_ALPHA,[ff]:n.SRC_ALPHA_SATURATE,[df]:n.DST_COLOR,[lf]:n.DST_ALPHA,[od]:n.ONE_MINUS_SRC_COLOR,[cs]:n.ONE_MINUS_SRC_ALPHA,[uf]:n.ONE_MINUS_DST_COLOR,[cf]:n.ONE_MINUS_DST_ALPHA,[hf]:n.CONSTANT_COLOR,[pf]:n.ONE_MINUS_CONSTANT_COLOR,[mf]:n.CONSTANT_ALPHA,[gf]:n.ONE_MINUS_CONSTANT_ALPHA};function $(q,Ce,xe,Le,Ne,Me,qe,We,Qe,wt){if(q===bi){m===!0&&(X(n.BLEND),m=!1);return}if(m===!1&&(ie(n.BLEND),m=!0),q!==ad){if(q!==p||wt!==w){if((C!==Bi||L!==Bi)&&(n.blendEquation(n.FUNC_ADD),C=Bi,L=Bi),wt)switch(q){case Ur:n.blendFuncSeparate(n.ONE,n.ONE_MINUS_SRC_ALPHA,n.ONE,n.ONE_MINUS_SRC_ALPHA);break;case kl:n.blendFunc(n.ONE,n.ONE);break;case zl:n.blendFuncSeparate(n.ZERO,n.ONE_MINUS_SRC_COLOR,n.ZERO,n.ONE);break;case Gl:n.blendFuncSeparate(n.DST_COLOR,n.ONE_MINUS_SRC_ALPHA,n.ZERO,n.ONE);break;default:Et("WebGLState: Invalid blending: ",q);break}else switch(q){case Ur:n.blendFuncSeparate(n.SRC_ALPHA,n.ONE_MINUS_SRC_ALPHA,n.ONE,n.ONE_MINUS_SRC_ALPHA);break;case kl:n.blendFuncSeparate(n.SRC_ALPHA,n.ONE,n.ONE,n.ONE);break;case zl:Et("WebGLState: SubtractiveBlending requires material.premultipliedAlpha = true");break;case Gl:Et("WebGLState: MultiplyBlending requires material.premultipliedAlpha = true");break;default:Et("WebGLState: Invalid blending: ",q);break}D=null,M=null,A=null,P=null,v.set(0,0,0),b=0,p=q,w=wt}return}Ne=Ne||Ce,Me=Me||xe,qe=qe||Le,(Ce!==C||Ne!==L)&&(n.blendEquationSeparate(ee[Ce],ee[Ne]),C=Ce,L=Ne),(xe!==D||Le!==M||Me!==A||qe!==P)&&(n.blendFuncSeparate(ce[xe],ce[Le],ce[Me],ce[qe]),D=xe,M=Le,A=Me,P=qe),(We.equals(v)===!1||Qe!==b)&&(n.blendColor(We.r,We.g,We.b,Qe),v.copy(We),b=Qe),p=q,w=!1}function pe(q,Ce){q.side===gi?X(n.CULL_FACE):ie(n.CULL_FACE);let xe=q.side===vn;Ce&&(xe=!xe),ye(xe),q.blending===Ur&&q.transparent===!1?$(bi):$(q.blending,q.blendEquation,q.blendSrc,q.blendDst,q.blendEquationAlpha,q.blendSrcAlpha,q.blendDstAlpha,q.blendColor,q.blendAlpha,q.premultipliedAlpha),a.setFunc(q.depthFunc),a.setTest(q.depthTest),a.setMask(q.depthWrite),s.setMask(q.colorWrite);const Le=q.stencilWrite;o.setTest(Le),Le&&(o.setMask(q.stencilWriteMask),o.setFunc(q.stencilFunc,q.stencilRef,q.stencilFuncMask),o.setOp(q.stencilFail,q.stencilZFail,q.stencilZPass)),Ge(q.polygonOffset,q.polygonOffsetFactor,q.polygonOffsetUnits),q.alphaToCoverage===!0?ie(n.SAMPLE_ALPHA_TO_COVERAGE):X(n.SAMPLE_ALPHA_TO_COVERAGE)}function ye(q){E!==q&&(q?n.frontFace(n.CW):n.frontFace(n.CCW),E=q)}function Te(q){q!==Qu?(ie(n.CULL_FACE),q!==N&&(q===Bl?n.cullFace(n.BACK):q===ju?n.cullFace(n.FRONT):n.cullFace(n.FRONT_AND_BACK))):X(n.CULL_FACE),N=q}function Pe(q){q!==G&&(F&&n.lineWidth(q),G=q)}function Ge(q,Ce,xe){q?(ie(n.POLYGON_OFFSET_FILL),(W!==Ce||I!==xe)&&(W=Ce,I=xe,a.getReversed()&&(Ce=-Ce),n.polygonOffset(Ce,xe))):X(n.POLYGON_OFFSET_FILL)}function Ee(q){q?ie(n.SCISSOR_TEST):X(n.SCISSOR_TEST)}function ve(q){q===void 0&&(q=n.TEXTURE0+R-1),oe!==q&&(n.activeTexture(q),oe=q)}function O(q,Ce,xe){xe===void 0&&(oe===null?xe=n.TEXTURE0+R-1:xe=oe);let Le=k[xe];Le===void 0&&(Le={type:void 0,texture:void 0},k[xe]=Le),(Le.type!==q||Le.texture!==Ce)&&(oe!==xe&&(n.activeTexture(xe),oe=xe),n.bindTexture(q,Ce||te[q]),Le.type=q,Le.texture=Ce)}function Xe(){const q=k[oe];q!==void 0&&q.type!==void 0&&(n.bindTexture(q.type,null),q.type=void 0,q.texture=void 0)}function Ze(){try{n.compressedTexImage2D(...arguments)}catch(q){Et("WebGLState:",q)}}function y(){try{n.compressedTexImage3D(...arguments)}catch(q){Et("WebGLState:",q)}}function g(){try{n.texSubImage2D(...arguments)}catch(q){Et("WebGLState:",q)}}function U(){try{n.texSubImage3D(...arguments)}catch(q){Et("WebGLState:",q)}}function z(){try{n.compressedTexSubImage2D(...arguments)}catch(q){Et("WebGLState:",q)}}function Q(){try{n.compressedTexSubImage3D(...arguments)}catch(q){Et("WebGLState:",q)}}function ue(){try{n.texStorage2D(...arguments)}catch(q){Et("WebGLState:",q)}}function be(){try{n.texStorage3D(...arguments)}catch(q){Et("WebGLState:",q)}}function ae(){try{n.texImage2D(...arguments)}catch(q){Et("WebGLState:",q)}}function de(){try{n.texImage3D(...arguments)}catch(q){Et("WebGLState:",q)}}function Se(q){return u[q]!==void 0?u[q]:n.getParameter(q)}function ke(q,Ce){u[q]!==Ce&&(n.pixelStorei(q,Ce),u[q]=Ce)}function Ae(q){Y.equals(q)===!1&&(n.scissor(q.x,q.y,q.z,q.w),Y.copy(q))}function Re(q){ne.equals(q)===!1&&(n.viewport(q.x,q.y,q.z,q.w),ne.copy(q))}function He(q,Ce){let xe=c.get(Ce);xe===void 0&&(xe=new WeakMap,c.set(Ce,xe));let Le=xe.get(q);Le===void 0&&(Le=n.getUniformBlockIndex(Ce,q.name),xe.set(q,Le))}function Ke(q,Ce){const Le=c.get(Ce).get(q);l.get(Ce)!==Le&&(n.uniformBlockBinding(Ce,Le,q.__bindingPointIndex),l.set(Ce,Le))}function Je(){n.disable(n.BLEND),n.disable(n.CULL_FACE),n.disable(n.DEPTH_TEST),n.disable(n.POLYGON_OFFSET_FILL),n.disable(n.SCISSOR_TEST),n.disable(n.STENCIL_TEST),n.disable(n.SAMPLE_ALPHA_TO_COVERAGE),n.blendEquation(n.FUNC_ADD),n.blendFunc(n.ONE,n.ZERO),n.blendFuncSeparate(n.ONE,n.ZERO,n.ONE,n.ZERO),n.blendColor(0,0,0,0),n.colorMask(!0,!0,!0,!0),n.clearColor(0,0,0,0),n.depthMask(!0),n.depthFunc(n.LESS),a.setReversed(!1),n.clearDepth(1),n.stencilMask(4294967295),n.stencilFunc(n.ALWAYS,0,4294967295),n.stencilOp(n.KEEP,n.KEEP,n.KEEP),n.clearStencil(0),n.cullFace(n.BACK),n.frontFace(n.CCW),n.polygonOffset(0,0),n.activeTexture(n.TEXTURE0),n.bindFramebuffer(n.FRAMEBUFFER,null),n.bindFramebuffer(n.DRAW_FRAMEBUFFER,null),n.bindFramebuffer(n.READ_FRAMEBUFFER,null),n.useProgram(null),n.lineWidth(1),n.scissor(0,0,n.canvas.width,n.canvas.height),n.viewport(0,0,n.canvas.width,n.canvas.height),n.pixelStorei(n.PACK_ALIGNMENT,4),n.pixelStorei(n.UNPACK_ALIGNMENT,4),n.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,!1),n.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,!1),n.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,n.BROWSER_DEFAULT_WEBGL),n.pixelStorei(n.PACK_ROW_LENGTH,0),n.pixelStorei(n.PACK_SKIP_PIXELS,0),n.pixelStorei(n.PACK_SKIP_ROWS,0),n.pixelStorei(n.UNPACK_ROW_LENGTH,0),n.pixelStorei(n.UNPACK_IMAGE_HEIGHT,0),n.pixelStorei(n.UNPACK_SKIP_PIXELS,0),n.pixelStorei(n.UNPACK_SKIP_ROWS,0),n.pixelStorei(n.UNPACK_SKIP_IMAGES,0),f={},u={},oe=null,k={},d={},h=new WeakMap,_=[],x=null,m=!1,p=null,C=null,D=null,M=null,L=null,A=null,P=null,v=new It(0,0,0),b=0,w=!1,E=null,N=null,G=null,W=null,I=null,Y.set(0,0,n.canvas.width,n.canvas.height),ne.set(0,0,n.canvas.width,n.canvas.height),s.reset(),a.reset(),o.reset()}return{buffers:{color:s,depth:a,stencil:o},enable:ie,disable:X,bindFramebuffer:he,drawBuffers:_e,useProgram:we,setBlending:$,setMaterial:pe,setFlipSided:ye,setCullFace:Te,setLineWidth:Pe,setPolygonOffset:Ge,setScissorTest:Ee,activeTexture:ve,bindTexture:O,unbindTexture:Xe,compressedTexImage2D:Ze,compressedTexImage3D:y,texImage2D:ae,texImage3D:de,pixelStorei:ke,getParameter:Se,updateUBOMapping:He,uniformBlockBinding:Ke,texStorage2D:ue,texStorage3D:be,texSubImage2D:g,texSubImage3D:U,compressedTexSubImage2D:z,compressedTexSubImage3D:Q,scissor:Ae,viewport:Re,reset:Je}}function j_(n,e,t,i,r,s,a){const o=e.has("WEBGL_multisampled_render_to_texture")?e.get("WEBGL_multisampled_render_to_texture"):null,l=typeof navigator>"u"?!1:/OculusBrowser/g.test(navigator.userAgent),c=new At,f=new WeakMap,u=new Set;let d;const h=new WeakMap;let _=!1;try{_=typeof OffscreenCanvas<"u"&&new OffscreenCanvas(1,1).getContext("2d")!==null}catch{}function x(y,g){return _?new OffscreenCanvas(y,g):oa("canvas")}function m(y,g,U){let z=1;const Q=Ze(y);if((Q.width>U||Q.height>U)&&(z=U/Math.max(Q.width,Q.height)),z<1)if(typeof HTMLImageElement<"u"&&y instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&y instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&y instanceof ImageBitmap||typeof VideoFrame<"u"&&y instanceof VideoFrame){const ue=Math.floor(z*Q.width),be=Math.floor(z*Q.height);d===void 0&&(d=x(ue,be));const ae=g?x(ue,be):d;return ae.width=ue,ae.height=be,ae.getContext("2d").drawImage(y,0,0,ue,be),st("WebGLRenderer: Texture has been resized from ("+Q.width+"x"+Q.height+") to ("+ue+"x"+be+")."),ae}else return"data"in y&&st("WebGLRenderer: Image in DataTexture is too big ("+Q.width+"x"+Q.height+")."),y;return y}function p(y){return y.generateMipmaps}function C(y){n.generateMipmap(y)}function D(y){return y.isWebGLCubeRenderTarget?n.TEXTURE_CUBE_MAP:y.isWebGL3DRenderTarget?n.TEXTURE_3D:y.isWebGLArrayRenderTarget||y.isCompressedArrayTexture?n.TEXTURE_2D_ARRAY:n.TEXTURE_2D}function M(y,g,U,z,Q,ue=!1){if(y!==null){if(n[y]!==void 0)return n[y];st("WebGLRenderer: Attempt to use non-existing WebGL internal format '"+y+"'")}let be;z&&(be=e.get("EXT_texture_norm16"),be||st("WebGLRenderer: Unable to use normalized textures without EXT_texture_norm16 extension"));let ae=g;if(g===n.RED&&(U===n.FLOAT&&(ae=n.R32F),U===n.HALF_FLOAT&&(ae=n.R16F),U===n.UNSIGNED_BYTE&&(ae=n.R8),U===n.UNSIGNED_SHORT&&be&&(ae=be.R16_EXT),U===n.SHORT&&be&&(ae=be.R16_SNORM_EXT)),g===n.RED_INTEGER&&(U===n.UNSIGNED_BYTE&&(ae=n.R8UI),U===n.UNSIGNED_SHORT&&(ae=n.R16UI),U===n.UNSIGNED_INT&&(ae=n.R32UI),U===n.BYTE&&(ae=n.R8I),U===n.SHORT&&(ae=n.R16I),U===n.INT&&(ae=n.R32I)),g===n.RG&&(U===n.FLOAT&&(ae=n.RG32F),U===n.HALF_FLOAT&&(ae=n.RG16F),U===n.UNSIGNED_BYTE&&(ae=n.RG8),U===n.UNSIGNED_SHORT&&be&&(ae=be.RG16_EXT),U===n.SHORT&&be&&(ae=be.RG16_SNORM_EXT)),g===n.RG_INTEGER&&(U===n.UNSIGNED_BYTE&&(ae=n.RG8UI),U===n.UNSIGNED_SHORT&&(ae=n.RG16UI),U===n.UNSIGNED_INT&&(ae=n.RG32UI),U===n.BYTE&&(ae=n.RG8I),U===n.SHORT&&(ae=n.RG16I),U===n.INT&&(ae=n.RG32I)),g===n.RGB_INTEGER&&(U===n.UNSIGNED_BYTE&&(ae=n.RGB8UI),U===n.UNSIGNED_SHORT&&(ae=n.RGB16UI),U===n.UNSIGNED_INT&&(ae=n.RGB32UI),U===n.BYTE&&(ae=n.RGB8I),U===n.SHORT&&(ae=n.RGB16I),U===n.INT&&(ae=n.RGB32I)),g===n.RGBA_INTEGER&&(U===n.UNSIGNED_BYTE&&(ae=n.RGBA8UI),U===n.UNSIGNED_SHORT&&(ae=n.RGBA16UI),U===n.UNSIGNED_INT&&(ae=n.RGBA32UI),U===n.BYTE&&(ae=n.RGBA8I),U===n.SHORT&&(ae=n.RGBA16I),U===n.INT&&(ae=n.RGBA32I)),g===n.RGB&&(U===n.UNSIGNED_SHORT&&be&&(ae=be.RGB16_EXT),U===n.SHORT&&be&&(ae=be.RGB16_SNORM_EXT),U===n.UNSIGNED_INT_5_9_9_9_REV&&(ae=n.RGB9_E5),U===n.UNSIGNED_INT_10F_11F_11F_REV&&(ae=n.R11F_G11F_B10F)),g===n.RGBA){const de=ue?sa:yt.getTransfer(Q);U===n.FLOAT&&(ae=n.RGBA32F),U===n.HALF_FLOAT&&(ae=n.RGBA16F),U===n.UNSIGNED_BYTE&&(ae=de===Dt?n.SRGB8_ALPHA8:n.RGBA8),U===n.UNSIGNED_SHORT&&be&&(ae=be.RGBA16_EXT),U===n.SHORT&&be&&(ae=be.RGBA16_SNORM_EXT),U===n.UNSIGNED_SHORT_4_4_4_4&&(ae=n.RGBA4),U===n.UNSIGNED_SHORT_5_5_5_1&&(ae=n.RGB5_A1)}return(ae===n.R16F||ae===n.R32F||ae===n.RG16F||ae===n.RG32F||ae===n.RGBA16F||ae===n.RGBA32F)&&e.get("EXT_color_buffer_float"),ae}function L(y,g){let U;return y?g===null||g===ii||g===us?U=n.DEPTH24_STENCIL8:g===ei?U=n.DEPTH32F_STENCIL8:g===ds&&(U=n.DEPTH24_STENCIL8,st("DepthTexture: 16 bit depth attachment is not supported with stencil. Using 24-bit attachment.")):g===null||g===ii||g===us?U=n.DEPTH_COMPONENT24:g===ei?U=n.DEPTH_COMPONENT32F:g===ds&&(U=n.DEPTH_COMPONENT16),U}function A(y,g){return p(y)===!0||y.isFramebufferTexture&&y.minFilter!==on&&y.minFilter!==ln?Math.log2(Math.max(g.width,g.height))+1:y.mipmaps!==void 0&&y.mipmaps.length>0?y.mipmaps.length:y.isCompressedTexture&&Array.isArray(y.image)?g.mipmaps.length:1}function P(y){const g=y.target;g.removeEventListener("dispose",P),b(g),g.isVideoTexture&&f.delete(g),g.isHTMLTexture&&u.delete(g)}function v(y){const g=y.target;g.removeEventListener("dispose",v),E(g)}function b(y){const g=i.get(y);if(g.__webglInit===void 0)return;const U=y.source,z=h.get(U);if(z){const Q=z[g.__cacheKey];Q.usedTimes--,Q.usedTimes===0&&w(y),Object.keys(z).length===0&&h.delete(U)}i.remove(y)}function w(y){const g=i.get(y);n.deleteTexture(g.__webglTexture);const U=y.source,z=h.get(U);delete z[g.__cacheKey],a.memory.textures--}function E(y){const g=i.get(y);if(y.depthTexture&&(y.depthTexture.dispose(),i.remove(y.depthTexture)),y.isWebGLCubeRenderTarget)for(let z=0;z<6;z++){if(Array.isArray(g.__webglFramebuffer[z]))for(let Q=0;Q<g.__webglFramebuffer[z].length;Q++)n.deleteFramebuffer(g.__webglFramebuffer[z][Q]);else n.deleteFramebuffer(g.__webglFramebuffer[z]);g.__webglDepthbuffer&&n.deleteRenderbuffer(g.__webglDepthbuffer[z])}else{if(Array.isArray(g.__webglFramebuffer))for(let z=0;z<g.__webglFramebuffer.length;z++)n.deleteFramebuffer(g.__webglFramebuffer[z]);else n.deleteFramebuffer(g.__webglFramebuffer);if(g.__webglDepthbuffer&&n.deleteRenderbuffer(g.__webglDepthbuffer),g.__webglMultisampledFramebuffer&&n.deleteFramebuffer(g.__webglMultisampledFramebuffer),g.__webglColorRenderbuffer)for(let z=0;z<g.__webglColorRenderbuffer.length;z++)g.__webglColorRenderbuffer[z]&&n.deleteRenderbuffer(g.__webglColorRenderbuffer[z]);g.__webglDepthRenderbuffer&&n.deleteRenderbuffer(g.__webglDepthRenderbuffer)}const U=y.textures;for(let z=0,Q=U.length;z<Q;z++){const ue=i.get(U[z]);ue.__webglTexture&&(n.deleteTexture(ue.__webglTexture),a.memory.textures--),i.remove(U[z])}i.remove(y)}let N=0;function G(){N=0}function W(){return N}function I(y){N=y}function R(){const y=N;return y>=r.maxTextures&&st("WebGLTextures: Trying to use "+y+" texture units while this GPU supports only "+r.maxTextures),N+=1,y}function F(y){const g=[];return g.push(y.wrapS),g.push(y.wrapT),g.push(y.wrapR||0),g.push(y.magFilter),g.push(y.minFilter),g.push(y.anisotropy),g.push(y.internalFormat),g.push(y.format),g.push(y.type),g.push(y.generateMipmaps),g.push(y.premultiplyAlpha),g.push(y.flipY),g.push(y.unpackAlignment),g.push(y.colorSpace),g.join()}function J(y,g){const U=i.get(y);if(y.isVideoTexture&&O(y),y.isRenderTargetTexture===!1&&y.isExternalTexture!==!0&&y.version>0&&U.__version!==y.version){const z=y.image;if(z===null)st("WebGLRenderer: Texture marked for update but no image data found.");else if(z.complete===!1)st("WebGLRenderer: Texture marked for update but image is incomplete");else{X(U,y,g);return}}else y.isExternalTexture&&(U.__webglTexture=y.sourceTexture?y.sourceTexture:null);t.bindTexture(n.TEXTURE_2D,U.__webglTexture,n.TEXTURE0+g)}function B(y,g){const U=i.get(y);if(y.isRenderTargetTexture===!1&&y.version>0&&U.__version!==y.version){X(U,y,g);return}else y.isExternalTexture&&(U.__webglTexture=y.sourceTexture?y.sourceTexture:null);t.bindTexture(n.TEXTURE_2D_ARRAY,U.__webglTexture,n.TEXTURE0+g)}function oe(y,g){const U=i.get(y);if(y.isRenderTargetTexture===!1&&y.version>0&&U.__version!==y.version){X(U,y,g);return}t.bindTexture(n.TEXTURE_3D,U.__webglTexture,n.TEXTURE0+g)}function k(y,g){const U=i.get(y);if(y.isCubeDepthTexture!==!0&&y.version>0&&U.__version!==y.version){he(U,y,g);return}t.bindTexture(n.TEXTURE_CUBE_MAP,U.__webglTexture,n.TEXTURE0+g)}const K={[go]:n.REPEAT,[xi]:n.CLAMP_TO_EDGE,[_o]:n.MIRRORED_REPEAT},Z={[on]:n.NEAREST,[xf]:n.NEAREST_MIPMAP_NEAREST,[Ss]:n.NEAREST_MIPMAP_LINEAR,[ln]:n.LINEAR,[Ma]:n.LINEAR_MIPMAP_NEAREST,[ir]:n.LINEAR_MIPMAP_LINEAR},Y={[Sf]:n.NEVER,[Af]:n.ALWAYS,[Mf]:n.LESS,[hl]:n.LEQUAL,[Ef]:n.EQUAL,[pl]:n.GEQUAL,[wf]:n.GREATER,[Tf]:n.NOTEQUAL};function ne(y,g){if(g.type===ei&&e.has("OES_texture_float_linear")===!1&&(g.magFilter===ln||g.magFilter===Ma||g.magFilter===Ss||g.magFilter===ir||g.minFilter===ln||g.minFilter===Ma||g.minFilter===Ss||g.minFilter===ir)&&st("WebGLRenderer: Unable to use linear filtering with floating point textures. OES_texture_float_linear not supported on this device."),n.texParameteri(y,n.TEXTURE_WRAP_S,K[g.wrapS]),n.texParameteri(y,n.TEXTURE_WRAP_T,K[g.wrapT]),(y===n.TEXTURE_3D||y===n.TEXTURE_2D_ARRAY)&&n.texParameteri(y,n.TEXTURE_WRAP_R,K[g.wrapR]),n.texParameteri(y,n.TEXTURE_MAG_FILTER,Z[g.magFilter]),n.texParameteri(y,n.TEXTURE_MIN_FILTER,Z[g.minFilter]),g.compareFunction&&(n.texParameteri(y,n.TEXTURE_COMPARE_MODE,n.COMPARE_REF_TO_TEXTURE),n.texParameteri(y,n.TEXTURE_COMPARE_FUNC,Y[g.compareFunction])),e.has("EXT_texture_filter_anisotropic")===!0){if(g.magFilter===on||g.minFilter!==Ss&&g.minFilter!==ir||g.type===ei&&e.has("OES_texture_float_linear")===!1)return;if(g.anisotropy>1||i.get(g).__currentAnisotropy){const U=e.get("EXT_texture_filter_anisotropic");n.texParameterf(y,U.TEXTURE_MAX_ANISOTROPY_EXT,Math.min(g.anisotropy,r.getMaxAnisotropy())),i.get(g).__currentAnisotropy=g.anisotropy}}}function H(y,g){let U=!1;y.__webglInit===void 0&&(y.__webglInit=!0,g.addEventListener("dispose",P));const z=g.source;let Q=h.get(z);Q===void 0&&(Q={},h.set(z,Q));const ue=F(g);if(ue!==y.__cacheKey){Q[ue]===void 0&&(Q[ue]={texture:n.createTexture(),usedTimes:0},a.memory.textures++,U=!0),Q[ue].usedTimes++;const be=Q[y.__cacheKey];be!==void 0&&(Q[y.__cacheKey].usedTimes--,be.usedTimes===0&&w(g)),y.__cacheKey=ue,y.__webglTexture=Q[ue].texture}return U}function te(y,g,U){return Math.floor(Math.floor(y/U)/g)}function ie(y,g,U,z){const ue=y.updateRanges;if(ue.length===0)t.texSubImage2D(n.TEXTURE_2D,0,0,0,g.width,g.height,U,z,g.data);else{ue.sort((ke,Ae)=>ke.start-Ae.start);let be=0;for(let ke=1;ke<ue.length;ke++){const Ae=ue[be],Re=ue[ke],He=Ae.start+Ae.count,Ke=te(Re.start,g.width,4),Je=te(Ae.start,g.width,4);Re.start<=He+1&&Ke===Je&&te(Re.start+Re.count-1,g.width,4)===Ke?Ae.count=Math.max(Ae.count,Re.start+Re.count-Ae.start):(++be,ue[be]=Re)}ue.length=be+1;const ae=t.getParameter(n.UNPACK_ROW_LENGTH),de=t.getParameter(n.UNPACK_SKIP_PIXELS),Se=t.getParameter(n.UNPACK_SKIP_ROWS);t.pixelStorei(n.UNPACK_ROW_LENGTH,g.width);for(let ke=0,Ae=ue.length;ke<Ae;ke++){const Re=ue[ke],He=Math.floor(Re.start/4),Ke=Math.ceil(Re.count/4),Je=He%g.width,q=Math.floor(He/g.width),Ce=Ke,xe=1;t.pixelStorei(n.UNPACK_SKIP_PIXELS,Je),t.pixelStorei(n.UNPACK_SKIP_ROWS,q),t.texSubImage2D(n.TEXTURE_2D,0,Je,q,Ce,xe,U,z,g.data)}y.clearUpdateRanges(),t.pixelStorei(n.UNPACK_ROW_LENGTH,ae),t.pixelStorei(n.UNPACK_SKIP_PIXELS,de),t.pixelStorei(n.UNPACK_SKIP_ROWS,Se)}}function X(y,g,U){let z=n.TEXTURE_2D;(g.isDataArrayTexture||g.isCompressedArrayTexture)&&(z=n.TEXTURE_2D_ARRAY),g.isData3DTexture&&(z=n.TEXTURE_3D);const Q=H(y,g),ue=g.source;t.bindTexture(z,y.__webglTexture,n.TEXTURE0+U);const be=i.get(ue);if(ue.version!==be.__version||Q===!0){if(t.activeTexture(n.TEXTURE0+U),(typeof ImageBitmap<"u"&&g.image instanceof ImageBitmap)===!1){const xe=yt.getPrimaries(yt.workingColorSpace),Le=g.colorSpace===_i?null:yt.getPrimaries(g.colorSpace),Ne=g.colorSpace===_i||xe===Le?n.NONE:n.BROWSER_DEFAULT_WEBGL;t.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,g.flipY),t.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,g.premultiplyAlpha),t.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,Ne)}t.pixelStorei(n.UNPACK_ALIGNMENT,g.unpackAlignment);let de=m(g.image,!1,r.maxTextureSize);de=Xe(g,de);const Se=s.convert(g.format,g.colorSpace),ke=s.convert(g.type);let Ae=M(g.internalFormat,Se,ke,g.normalized,g.colorSpace,g.isVideoTexture);ne(z,g);let Re;const He=g.mipmaps,Ke=g.isVideoTexture!==!0,Je=be.__version===void 0||Q===!0,q=ue.dataReady,Ce=A(g,de);if(g.isDepthTexture)Ae=L(g.format===rr,g.type),Je&&(Ke?t.texStorage2D(n.TEXTURE_2D,1,Ae,de.width,de.height):t.texImage2D(n.TEXTURE_2D,0,Ae,de.width,de.height,0,Se,ke,null));else if(g.isDataTexture)if(He.length>0){Ke&&Je&&t.texStorage2D(n.TEXTURE_2D,Ce,Ae,He[0].width,He[0].height);for(let xe=0,Le=He.length;xe<Le;xe++)Re=He[xe],Ke?q&&t.texSubImage2D(n.TEXTURE_2D,xe,0,0,Re.width,Re.height,Se,ke,Re.data):t.texImage2D(n.TEXTURE_2D,xe,Ae,Re.width,Re.height,0,Se,ke,Re.data);g.generateMipmaps=!1}else Ke?(Je&&t.texStorage2D(n.TEXTURE_2D,Ce,Ae,de.width,de.height),q&&ie(g,de,Se,ke)):t.texImage2D(n.TEXTURE_2D,0,Ae,de.width,de.height,0,Se,ke,de.data);else if(g.isCompressedTexture)if(g.isCompressedArrayTexture){Ke&&Je&&t.texStorage3D(n.TEXTURE_2D_ARRAY,Ce,Ae,He[0].width,He[0].height,de.depth);for(let xe=0,Le=He.length;xe<Le;xe++)if(Re=He[xe],g.format!==Hn)if(Se!==null)if(Ke){if(q)if(g.layerUpdates.size>0){const Ne=cc(Re.width,Re.height,g.format,g.type);for(const Me of g.layerUpdates){const qe=Re.data.subarray(Me*Ne/Re.data.BYTES_PER_ELEMENT,(Me+1)*Ne/Re.data.BYTES_PER_ELEMENT);t.compressedTexSubImage3D(n.TEXTURE_2D_ARRAY,xe,0,0,Me,Re.width,Re.height,1,Se,qe)}g.clearLayerUpdates()}else t.compressedTexSubImage3D(n.TEXTURE_2D_ARRAY,xe,0,0,0,Re.width,Re.height,de.depth,Se,Re.data)}else t.compressedTexImage3D(n.TEXTURE_2D_ARRAY,xe,Ae,Re.width,Re.height,de.depth,0,Re.data,0,0);else st("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()");else Ke?q&&t.texSubImage3D(n.TEXTURE_2D_ARRAY,xe,0,0,0,Re.width,Re.height,de.depth,Se,ke,Re.data):t.texImage3D(n.TEXTURE_2D_ARRAY,xe,Ae,Re.width,Re.height,de.depth,0,Se,ke,Re.data)}else{Ke&&Je&&t.texStorage2D(n.TEXTURE_2D,Ce,Ae,He[0].width,He[0].height);for(let xe=0,Le=He.length;xe<Le;xe++)Re=He[xe],g.format!==Hn?Se!==null?Ke?q&&t.compressedTexSubImage2D(n.TEXTURE_2D,xe,0,0,Re.width,Re.height,Se,Re.data):t.compressedTexImage2D(n.TEXTURE_2D,xe,Ae,Re.width,Re.height,0,Re.data):st("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()"):Ke?q&&t.texSubImage2D(n.TEXTURE_2D,xe,0,0,Re.width,Re.height,Se,ke,Re.data):t.texImage2D(n.TEXTURE_2D,xe,Ae,Re.width,Re.height,0,Se,ke,Re.data)}else if(g.isDataArrayTexture)if(Ke){if(Je&&t.texStorage3D(n.TEXTURE_2D_ARRAY,Ce,Ae,de.width,de.height,de.depth),q)if(g.layerUpdates.size>0){const xe=cc(de.width,de.height,g.format,g.type);for(const Le of g.layerUpdates){const Ne=de.data.subarray(Le*xe/de.data.BYTES_PER_ELEMENT,(Le+1)*xe/de.data.BYTES_PER_ELEMENT);t.texSubImage3D(n.TEXTURE_2D_ARRAY,0,0,0,Le,de.width,de.height,1,Se,ke,Ne)}g.clearLayerUpdates()}else t.texSubImage3D(n.TEXTURE_2D_ARRAY,0,0,0,0,de.width,de.height,de.depth,Se,ke,de.data)}else t.texImage3D(n.TEXTURE_2D_ARRAY,0,Ae,de.width,de.height,de.depth,0,Se,ke,de.data);else if(g.isData3DTexture)Ke?(Je&&t.texStorage3D(n.TEXTURE_3D,Ce,Ae,de.width,de.height,de.depth),q&&t.texSubImage3D(n.TEXTURE_3D,0,0,0,0,de.width,de.height,de.depth,Se,ke,de.data)):t.texImage3D(n.TEXTURE_3D,0,Ae,de.width,de.height,de.depth,0,Se,ke,de.data);else if(g.isFramebufferTexture){if(Je)if(Ke)t.texStorage2D(n.TEXTURE_2D,Ce,Ae,de.width,de.height);else{let xe=de.width,Le=de.height;for(let Ne=0;Ne<Ce;Ne++)t.texImage2D(n.TEXTURE_2D,Ne,Ae,xe,Le,0,Se,ke,null),xe>>=1,Le>>=1}}else if(g.isHTMLTexture){if("texElementImage2D"in n){const xe=n.canvas;if(xe.hasAttribute("layoutsubtree")||xe.setAttribute("layoutsubtree","true"),de.parentNode!==xe){xe.appendChild(de),u.add(g),xe.onpaint=Le=>{const Ne=Le.changedElements;for(const Me of u)Ne.includes(Me.image)&&(Me.needsUpdate=!0)},xe.requestPaint();return}if(n.texElementImage2D.length===3)n.texElementImage2D(n.TEXTURE_2D,n.RGBA8,de);else{const Ne=n.RGBA,Me=n.RGBA,qe=n.UNSIGNED_BYTE;n.texElementImage2D(n.TEXTURE_2D,0,Ne,Me,qe,de)}n.texParameteri(n.TEXTURE_2D,n.TEXTURE_MIN_FILTER,n.LINEAR),n.texParameteri(n.TEXTURE_2D,n.TEXTURE_WRAP_S,n.CLAMP_TO_EDGE),n.texParameteri(n.TEXTURE_2D,n.TEXTURE_WRAP_T,n.CLAMP_TO_EDGE)}}else if(He.length>0){if(Ke&&Je){const xe=Ze(He[0]);t.texStorage2D(n.TEXTURE_2D,Ce,Ae,xe.width,xe.height)}for(let xe=0,Le=He.length;xe<Le;xe++)Re=He[xe],Ke?q&&t.texSubImage2D(n.TEXTURE_2D,xe,0,0,Se,ke,Re):t.texImage2D(n.TEXTURE_2D,xe,Ae,Se,ke,Re);g.generateMipmaps=!1}else if(Ke){if(Je){const xe=Ze(de);t.texStorage2D(n.TEXTURE_2D,Ce,Ae,xe.width,xe.height)}q&&t.texSubImage2D(n.TEXTURE_2D,0,0,0,Se,ke,de)}else t.texImage2D(n.TEXTURE_2D,0,Ae,Se,ke,de);p(g)&&C(z),be.__version=ue.version,g.onUpdate&&g.onUpdate(g)}y.__version=g.version}function he(y,g,U){if(g.image.length!==6)return;const z=H(y,g),Q=g.source;t.bindTexture(n.TEXTURE_CUBE_MAP,y.__webglTexture,n.TEXTURE0+U);const ue=i.get(Q);if(Q.version!==ue.__version||z===!0){t.activeTexture(n.TEXTURE0+U);const be=yt.getPrimaries(yt.workingColorSpace),ae=g.colorSpace===_i?null:yt.getPrimaries(g.colorSpace),de=g.colorSpace===_i||be===ae?n.NONE:n.BROWSER_DEFAULT_WEBGL;t.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,g.flipY),t.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,g.premultiplyAlpha),t.pixelStorei(n.UNPACK_ALIGNMENT,g.unpackAlignment),t.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,de);const Se=g.isCompressedTexture||g.image[0].isCompressedTexture,ke=g.image[0]&&g.image[0].isDataTexture,Ae=[];for(let Me=0;Me<6;Me++)!Se&&!ke?Ae[Me]=m(g.image[Me],!0,r.maxCubemapSize):Ae[Me]=ke?g.image[Me].image:g.image[Me],Ae[Me]=Xe(g,Ae[Me]);const Re=Ae[0],He=s.convert(g.format,g.colorSpace),Ke=s.convert(g.type),Je=M(g.internalFormat,He,Ke,g.normalized,g.colorSpace),q=g.isVideoTexture!==!0,Ce=ue.__version===void 0||z===!0,xe=Q.dataReady;let Le=A(g,Re);ne(n.TEXTURE_CUBE_MAP,g);let Ne;if(Se){q&&Ce&&t.texStorage2D(n.TEXTURE_CUBE_MAP,Le,Je,Re.width,Re.height);for(let Me=0;Me<6;Me++){Ne=Ae[Me].mipmaps;for(let qe=0;qe<Ne.length;qe++){const We=Ne[qe];g.format!==Hn?He!==null?q?xe&&t.compressedTexSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe,0,0,We.width,We.height,He,We.data):t.compressedTexImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe,Je,We.width,We.height,0,We.data):st("WebGLRenderer: Attempt to load unsupported compressed texture format in .setTextureCube()"):q?xe&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe,0,0,We.width,We.height,He,Ke,We.data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe,Je,We.width,We.height,0,He,Ke,We.data)}}}else{if(Ne=g.mipmaps,q&&Ce){Ne.length>0&&Le++;const Me=Ze(Ae[0]);t.texStorage2D(n.TEXTURE_CUBE_MAP,Le,Je,Me.width,Me.height)}for(let Me=0;Me<6;Me++)if(ke){q?xe&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,0,0,0,Ae[Me].width,Ae[Me].height,He,Ke,Ae[Me].data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,0,Je,Ae[Me].width,Ae[Me].height,0,He,Ke,Ae[Me].data);for(let qe=0;qe<Ne.length;qe++){const Qe=Ne[qe].image[Me].image;q?xe&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe+1,0,0,Qe.width,Qe.height,He,Ke,Qe.data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe+1,Je,Qe.width,Qe.height,0,He,Ke,Qe.data)}}else{q?xe&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,0,0,0,He,Ke,Ae[Me]):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,0,Je,He,Ke,Ae[Me]);for(let qe=0;qe<Ne.length;qe++){const We=Ne[qe];q?xe&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe+1,0,0,He,Ke,We.image[Me]):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Me,qe+1,Je,He,Ke,We.image[Me])}}}p(g)&&C(n.TEXTURE_CUBE_MAP),ue.__version=Q.version,g.onUpdate&&g.onUpdate(g)}y.__version=g.version}function _e(y,g,U,z,Q,ue){const be=s.convert(U.format,U.colorSpace),ae=s.convert(U.type),de=M(U.internalFormat,be,ae,U.normalized,U.colorSpace),Se=i.get(g),ke=i.get(U);if(ke.__renderTarget=g,!Se.__hasExternalTextures){const Ae=Math.max(1,g.width>>ue),Re=Math.max(1,g.height>>ue);Q===n.TEXTURE_3D||Q===n.TEXTURE_2D_ARRAY?t.texImage3D(Q,ue,de,Ae,Re,g.depth,0,be,ae,null):t.texImage2D(Q,ue,de,Ae,Re,0,be,ae,null)}t.bindFramebuffer(n.FRAMEBUFFER,y),ve(g)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,z,Q,ke.__webglTexture,0,Ee(g)):(Q===n.TEXTURE_2D||Q>=n.TEXTURE_CUBE_MAP_POSITIVE_X&&Q<=n.TEXTURE_CUBE_MAP_NEGATIVE_Z)&&n.framebufferTexture2D(n.FRAMEBUFFER,z,Q,ke.__webglTexture,ue),t.bindFramebuffer(n.FRAMEBUFFER,null)}function we(y,g,U){if(n.bindRenderbuffer(n.RENDERBUFFER,y),g.depthBuffer){const z=g.depthTexture,Q=z&&z.isDepthTexture?z.type:null,ue=L(g.stencilBuffer,Q),be=g.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;ve(g)?o.renderbufferStorageMultisampleEXT(n.RENDERBUFFER,Ee(g),ue,g.width,g.height):U?n.renderbufferStorageMultisample(n.RENDERBUFFER,Ee(g),ue,g.width,g.height):n.renderbufferStorage(n.RENDERBUFFER,ue,g.width,g.height),n.framebufferRenderbuffer(n.FRAMEBUFFER,be,n.RENDERBUFFER,y)}else{const z=g.textures;for(let Q=0;Q<z.length;Q++){const ue=z[Q],be=s.convert(ue.format,ue.colorSpace),ae=s.convert(ue.type),de=M(ue.internalFormat,be,ae,ue.normalized,ue.colorSpace);ve(g)?o.renderbufferStorageMultisampleEXT(n.RENDERBUFFER,Ee(g),de,g.width,g.height):U?n.renderbufferStorageMultisample(n.RENDERBUFFER,Ee(g),de,g.width,g.height):n.renderbufferStorage(n.RENDERBUFFER,de,g.width,g.height)}}n.bindRenderbuffer(n.RENDERBUFFER,null)}function ee(y,g,U){const z=g.isWebGLCubeRenderTarget===!0;if(t.bindFramebuffer(n.FRAMEBUFFER,y),!(g.depthTexture&&g.depthTexture.isDepthTexture))throw new Error("THREE.WebGLTextures: renderTarget.depthTexture must be an instance of THREE.DepthTexture.");const Q=i.get(g.depthTexture);if(Q.__renderTarget=g,(!Q.__webglTexture||g.depthTexture.image.width!==g.width||g.depthTexture.image.height!==g.height)&&(g.depthTexture.image.width=g.width,g.depthTexture.image.height=g.height,g.depthTexture.needsUpdate=!0),z){if(Q.__webglInit===void 0&&(Q.__webglInit=!0,g.depthTexture.addEventListener("dispose",P)),Q.__webglTexture===void 0){Q.__webglTexture=n.createTexture(),t.bindTexture(n.TEXTURE_CUBE_MAP,Q.__webglTexture),ne(n.TEXTURE_CUBE_MAP,g.depthTexture);const Se=s.convert(g.depthTexture.format),ke=s.convert(g.depthTexture.type);let Ae;g.depthTexture.format===Ti?Ae=n.DEPTH_COMPONENT24:g.depthTexture.format===rr&&(Ae=n.DEPTH24_STENCIL8);for(let Re=0;Re<6;Re++)n.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Re,0,Ae,g.width,g.height,0,Se,ke,null)}}else J(g.depthTexture,0);const ue=Q.__webglTexture,be=Ee(g),ae=z?n.TEXTURE_CUBE_MAP_POSITIVE_X+U:n.TEXTURE_2D,de=g.depthTexture.format===rr?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;if(g.depthTexture.format===Ti)ve(g)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,de,ae,ue,0,be):n.framebufferTexture2D(n.FRAMEBUFFER,de,ae,ue,0);else if(g.depthTexture.format===rr)ve(g)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,de,ae,ue,0,be):n.framebufferTexture2D(n.FRAMEBUFFER,de,ae,ue,0);else throw new Error("THREE.WebGLTextures: Unknown depthTexture format.")}function ce(y){const g=i.get(y),U=y.isWebGLCubeRenderTarget===!0;if(g.__boundDepthTexture!==y.depthTexture){const z=y.depthTexture;if(g.__depthDisposeCallback&&g.__depthDisposeCallback(),z){const Q=()=>{delete g.__boundDepthTexture,delete g.__depthDisposeCallback,z.removeEventListener("dispose",Q)};z.addEventListener("dispose",Q),g.__depthDisposeCallback=Q}g.__boundDepthTexture=z}if(y.depthTexture&&!g.__autoAllocateDepthBuffer)if(U)for(let z=0;z<6;z++)ee(g.__webglFramebuffer[z],y,z);else{const z=y.texture.mipmaps;z&&z.length>0?ee(g.__webglFramebuffer[0],y,0):ee(g.__webglFramebuffer,y,0)}else if(U){g.__webglDepthbuffer=[];for(let z=0;z<6;z++)if(t.bindFramebuffer(n.FRAMEBUFFER,g.__webglFramebuffer[z]),g.__webglDepthbuffer[z]===void 0)g.__webglDepthbuffer[z]=n.createRenderbuffer(),we(g.__webglDepthbuffer[z],y,!1);else{const Q=y.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,ue=g.__webglDepthbuffer[z];n.bindRenderbuffer(n.RENDERBUFFER,ue),n.framebufferRenderbuffer(n.FRAMEBUFFER,Q,n.RENDERBUFFER,ue)}}else{const z=y.texture.mipmaps;if(z&&z.length>0?t.bindFramebuffer(n.FRAMEBUFFER,g.__webglFramebuffer[0]):t.bindFramebuffer(n.FRAMEBUFFER,g.__webglFramebuffer),g.__webglDepthbuffer===void 0)g.__webglDepthbuffer=n.createRenderbuffer(),we(g.__webglDepthbuffer,y,!1);else{const Q=y.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,ue=g.__webglDepthbuffer;n.bindRenderbuffer(n.RENDERBUFFER,ue),n.framebufferRenderbuffer(n.FRAMEBUFFER,Q,n.RENDERBUFFER,ue)}}t.bindFramebuffer(n.FRAMEBUFFER,null)}function $(y,g,U){const z=i.get(y);g!==void 0&&_e(z.__webglFramebuffer,y,y.texture,n.COLOR_ATTACHMENT0,n.TEXTURE_2D,0),U!==void 0&&ce(y)}function pe(y){const g=y.texture,U=i.get(y),z=i.get(g);y.addEventListener("dispose",v);const Q=y.textures,ue=y.isWebGLCubeRenderTarget===!0,be=Q.length>1;if(be||(z.__webglTexture===void 0&&(z.__webglTexture=n.createTexture()),z.__version=g.version,a.memory.textures++),ue){U.__webglFramebuffer=[];for(let ae=0;ae<6;ae++)if(g.mipmaps&&g.mipmaps.length>0){U.__webglFramebuffer[ae]=[];for(let de=0;de<g.mipmaps.length;de++)U.__webglFramebuffer[ae][de]=n.createFramebuffer()}else U.__webglFramebuffer[ae]=n.createFramebuffer()}else{if(g.mipmaps&&g.mipmaps.length>0){U.__webglFramebuffer=[];for(let ae=0;ae<g.mipmaps.length;ae++)U.__webglFramebuffer[ae]=n.createFramebuffer()}else U.__webglFramebuffer=n.createFramebuffer();if(be)for(let ae=0,de=Q.length;ae<de;ae++){const Se=i.get(Q[ae]);Se.__webglTexture===void 0&&(Se.__webglTexture=n.createTexture(),a.memory.textures++)}if(y.samples>0&&ve(y)===!1){U.__webglMultisampledFramebuffer=n.createFramebuffer(),U.__webglColorRenderbuffer=[],t.bindFramebuffer(n.FRAMEBUFFER,U.__webglMultisampledFramebuffer);for(let ae=0;ae<Q.length;ae++){const de=Q[ae];U.__webglColorRenderbuffer[ae]=n.createRenderbuffer(),n.bindRenderbuffer(n.RENDERBUFFER,U.__webglColorRenderbuffer[ae]);const Se=s.convert(de.format,de.colorSpace),ke=s.convert(de.type),Ae=M(de.internalFormat,Se,ke,de.normalized,de.colorSpace,y.isXRRenderTarget===!0),Re=Ee(y);n.renderbufferStorageMultisample(n.RENDERBUFFER,Re,Ae,y.width,y.height),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+ae,n.RENDERBUFFER,U.__webglColorRenderbuffer[ae])}n.bindRenderbuffer(n.RENDERBUFFER,null),y.depthBuffer&&(U.__webglDepthRenderbuffer=n.createRenderbuffer(),we(U.__webglDepthRenderbuffer,y,!0)),t.bindFramebuffer(n.FRAMEBUFFER,null)}}if(ue){t.bindTexture(n.TEXTURE_CUBE_MAP,z.__webglTexture),ne(n.TEXTURE_CUBE_MAP,g);for(let ae=0;ae<6;ae++)if(g.mipmaps&&g.mipmaps.length>0)for(let de=0;de<g.mipmaps.length;de++)_e(U.__webglFramebuffer[ae][de],y,g,n.COLOR_ATTACHMENT0,n.TEXTURE_CUBE_MAP_POSITIVE_X+ae,de);else _e(U.__webglFramebuffer[ae],y,g,n.COLOR_ATTACHMENT0,n.TEXTURE_CUBE_MAP_POSITIVE_X+ae,0);p(g)&&C(n.TEXTURE_CUBE_MAP),t.unbindTexture()}else if(be){for(let ae=0,de=Q.length;ae<de;ae++){const Se=Q[ae],ke=i.get(Se);let Ae=n.TEXTURE_2D;(y.isWebGL3DRenderTarget||y.isWebGLArrayRenderTarget)&&(Ae=y.isWebGL3DRenderTarget?n.TEXTURE_3D:n.TEXTURE_2D_ARRAY),t.bindTexture(Ae,ke.__webglTexture),ne(Ae,Se),_e(U.__webglFramebuffer,y,Se,n.COLOR_ATTACHMENT0+ae,Ae,0),p(Se)&&C(Ae)}t.unbindTexture()}else{let ae=n.TEXTURE_2D;if((y.isWebGL3DRenderTarget||y.isWebGLArrayRenderTarget)&&(ae=y.isWebGL3DRenderTarget?n.TEXTURE_3D:n.TEXTURE_2D_ARRAY),t.bindTexture(ae,z.__webglTexture),ne(ae,g),g.mipmaps&&g.mipmaps.length>0)for(let de=0;de<g.mipmaps.length;de++)_e(U.__webglFramebuffer[de],y,g,n.COLOR_ATTACHMENT0,ae,de);else _e(U.__webglFramebuffer,y,g,n.COLOR_ATTACHMENT0,ae,0);p(g)&&C(ae),t.unbindTexture()}y.depthBuffer&&ce(y)}function ye(y){const g=y.textures;for(let U=0,z=g.length;U<z;U++){const Q=g[U];if(p(Q)){const ue=D(y),be=i.get(Q).__webglTexture;t.bindTexture(ue,be),C(ue),t.unbindTexture()}}}const Te=[],Pe=[];function Ge(y){if(y.samples>0){if(ve(y)===!1){const g=y.textures,U=y.width,z=y.height;let Q=n.COLOR_BUFFER_BIT;const ue=y.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,be=i.get(y),ae=g.length>1;if(ae)for(let Se=0;Se<g.length;Se++)t.bindFramebuffer(n.FRAMEBUFFER,be.__webglMultisampledFramebuffer),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.RENDERBUFFER,null),t.bindFramebuffer(n.FRAMEBUFFER,be.__webglFramebuffer),n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.TEXTURE_2D,null,0);t.bindFramebuffer(n.READ_FRAMEBUFFER,be.__webglMultisampledFramebuffer);const de=y.texture.mipmaps;de&&de.length>0?t.bindFramebuffer(n.DRAW_FRAMEBUFFER,be.__webglFramebuffer[0]):t.bindFramebuffer(n.DRAW_FRAMEBUFFER,be.__webglFramebuffer);for(let Se=0;Se<g.length;Se++){if(y.resolveDepthBuffer&&(y.depthBuffer&&(Q|=n.DEPTH_BUFFER_BIT),y.stencilBuffer&&y.resolveStencilBuffer&&(Q|=n.STENCIL_BUFFER_BIT)),ae){n.framebufferRenderbuffer(n.READ_FRAMEBUFFER,n.COLOR_ATTACHMENT0,n.RENDERBUFFER,be.__webglColorRenderbuffer[Se]);const ke=i.get(g[Se]).__webglTexture;n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0,n.TEXTURE_2D,ke,0)}n.blitFramebuffer(0,0,U,z,0,0,U,z,Q,n.NEAREST),l===!0&&(Te.length=0,Pe.length=0,Te.push(n.COLOR_ATTACHMENT0+Se),y.depthBuffer&&y.resolveDepthBuffer===!1&&(Te.push(ue),Pe.push(ue),n.invalidateFramebuffer(n.DRAW_FRAMEBUFFER,Pe)),n.invalidateFramebuffer(n.READ_FRAMEBUFFER,Te))}if(t.bindFramebuffer(n.READ_FRAMEBUFFER,null),t.bindFramebuffer(n.DRAW_FRAMEBUFFER,null),ae)for(let Se=0;Se<g.length;Se++){t.bindFramebuffer(n.FRAMEBUFFER,be.__webglMultisampledFramebuffer),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.RENDERBUFFER,be.__webglColorRenderbuffer[Se]);const ke=i.get(g[Se]).__webglTexture;t.bindFramebuffer(n.FRAMEBUFFER,be.__webglFramebuffer),n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.TEXTURE_2D,ke,0)}t.bindFramebuffer(n.DRAW_FRAMEBUFFER,be.__webglMultisampledFramebuffer)}else if(y.depthBuffer&&y.resolveDepthBuffer===!1&&l){const g=y.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;n.invalidateFramebuffer(n.DRAW_FRAMEBUFFER,[g])}}}function Ee(y){return Math.min(r.maxSamples,y.samples)}function ve(y){const g=i.get(y);return y.samples>0&&e.has("WEBGL_multisampled_render_to_texture")===!0&&g.__useRenderToTexture!==!1}function O(y){const g=a.render.frame;f.get(y)!==g&&(f.set(y,g),y.update())}function Xe(y,g){const U=y.colorSpace,z=y.format,Q=y.type;return y.isCompressedTexture===!0||y.isVideoTexture===!0||U!==fs&&U!==_i&&(yt.getTransfer(U)===Dt?(z!==Hn||Q!==In)&&st("WebGLTextures: sRGB encoded textures have to use RGBAFormat and UnsignedByteType."):Et("WebGLTextures: Unsupported texture color space:",U)),g}function Ze(y){return typeof HTMLImageElement<"u"&&y instanceof HTMLImageElement?(c.width=y.naturalWidth||y.width,c.height=y.naturalHeight||y.height):typeof VideoFrame<"u"&&y instanceof VideoFrame?(c.width=y.displayWidth,c.height=y.displayHeight):(c.width=y.width,c.height=y.height),c}this.allocateTextureUnit=R,this.resetTextureUnits=G,this.getTextureUnits=W,this.setTextureUnits=I,this.setTexture2D=J,this.setTexture2DArray=B,this.setTexture3D=oe,this.setTextureCube=k,this.rebindTextures=$,this.setupRenderTarget=pe,this.updateRenderTargetMipmap=ye,this.updateMultisampleRenderTarget=Ge,this.setupDepthRenderbuffer=ce,this.setupFrameBufferTexture=_e,this.useMultisampledRTT=ve,this.isReversedDepthBuffer=function(){return t.buffers.depth.getReversed()}}function e0(n,e){function t(i,r=_i){let s;const a=yt.getTransfer(r);if(i===In)return n.UNSIGNED_BYTE;if(i===ll)return n.UNSIGNED_SHORT_4_4_4_4;if(i===cl)return n.UNSIGNED_SHORT_5_5_5_1;if(i===xd)return n.UNSIGNED_INT_5_9_9_9_REV;if(i===yd)return n.UNSIGNED_INT_10F_11F_11F_REV;if(i===_d)return n.BYTE;if(i===vd)return n.SHORT;if(i===ds)return n.UNSIGNED_SHORT;if(i===ol)return n.INT;if(i===ii)return n.UNSIGNED_INT;if(i===ei)return n.FLOAT;if(i===wi)return n.HALF_FLOAT;if(i===bd)return n.ALPHA;if(i===Sd)return n.RGB;if(i===Hn)return n.RGBA;if(i===Ti)return n.DEPTH_COMPONENT;if(i===rr)return n.DEPTH_STENCIL;if(i===Md)return n.RED;if(i===dl)return n.RED_INTEGER;if(i===or)return n.RG;if(i===ul)return n.RG_INTEGER;if(i===fl)return n.RGBA_INTEGER;if(i===Zs||i===Js||i===Qs||i===js)if(a===Dt)if(s=e.get("WEBGL_compressed_texture_s3tc_srgb"),s!==null){if(i===Zs)return s.COMPRESSED_SRGB_S3TC_DXT1_EXT;if(i===Js)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT;if(i===Qs)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT;if(i===js)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT}else return null;else if(s=e.get("WEBGL_compressed_texture_s3tc"),s!==null){if(i===Zs)return s.COMPRESSED_RGB_S3TC_DXT1_EXT;if(i===Js)return s.COMPRESSED_RGBA_S3TC_DXT1_EXT;if(i===Qs)return s.COMPRESSED_RGBA_S3TC_DXT3_EXT;if(i===js)return s.COMPRESSED_RGBA_S3TC_DXT5_EXT}else return null;if(i===vo||i===xo||i===yo||i===bo)if(s=e.get("WEBGL_compressed_texture_pvrtc"),s!==null){if(i===vo)return s.COMPRESSED_RGB_PVRTC_4BPPV1_IMG;if(i===xo)return s.COMPRESSED_RGB_PVRTC_2BPPV1_IMG;if(i===yo)return s.COMPRESSED_RGBA_PVRTC_4BPPV1_IMG;if(i===bo)return s.COMPRESSED_RGBA_PVRTC_2BPPV1_IMG}else return null;if(i===So||i===Mo||i===Eo||i===wo||i===To||i===ia||i===Ao)if(s=e.get("WEBGL_compressed_texture_etc"),s!==null){if(i===So||i===Mo)return a===Dt?s.COMPRESSED_SRGB8_ETC2:s.COMPRESSED_RGB8_ETC2;if(i===Eo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ETC2_EAC:s.COMPRESSED_RGBA8_ETC2_EAC;if(i===wo)return s.COMPRESSED_R11_EAC;if(i===To)return s.COMPRESSED_SIGNED_R11_EAC;if(i===ia)return s.COMPRESSED_RG11_EAC;if(i===Ao)return s.COMPRESSED_SIGNED_RG11_EAC}else return null;if(i===Ro||i===Co||i===Po||i===Lo||i===Io||i===Do||i===No||i===Uo||i===Fo||i===Oo||i===Bo||i===ko||i===zo||i===Go)if(s=e.get("WEBGL_compressed_texture_astc"),s!==null){if(i===Ro)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_4x4_KHR:s.COMPRESSED_RGBA_ASTC_4x4_KHR;if(i===Co)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x4_KHR:s.COMPRESSED_RGBA_ASTC_5x4_KHR;if(i===Po)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x5_KHR:s.COMPRESSED_RGBA_ASTC_5x5_KHR;if(i===Lo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x5_KHR:s.COMPRESSED_RGBA_ASTC_6x5_KHR;if(i===Io)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x6_KHR:s.COMPRESSED_RGBA_ASTC_6x6_KHR;if(i===Do)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x5_KHR:s.COMPRESSED_RGBA_ASTC_8x5_KHR;if(i===No)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x6_KHR:s.COMPRESSED_RGBA_ASTC_8x6_KHR;if(i===Uo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x8_KHR:s.COMPRESSED_RGBA_ASTC_8x8_KHR;if(i===Fo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x5_KHR:s.COMPRESSED_RGBA_ASTC_10x5_KHR;if(i===Oo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x6_KHR:s.COMPRESSED_RGBA_ASTC_10x6_KHR;if(i===Bo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x8_KHR:s.COMPRESSED_RGBA_ASTC_10x8_KHR;if(i===ko)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x10_KHR:s.COMPRESSED_RGBA_ASTC_10x10_KHR;if(i===zo)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x10_KHR:s.COMPRESSED_RGBA_ASTC_12x10_KHR;if(i===Go)return a===Dt?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x12_KHR:s.COMPRESSED_RGBA_ASTC_12x12_KHR}else return null;if(i===Ho||i===Vo||i===Wo)if(s=e.get("EXT_texture_compression_bptc"),s!==null){if(i===Ho)return a===Dt?s.COMPRESSED_SRGB_ALPHA_BPTC_UNORM_EXT:s.COMPRESSED_RGBA_BPTC_UNORM_EXT;if(i===Vo)return s.COMPRESSED_RGB_BPTC_SIGNED_FLOAT_EXT;if(i===Wo)return s.COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT_EXT}else return null;if(i===Xo||i===qo||i===ra||i===Yo)if(s=e.get("EXT_texture_compression_rgtc"),s!==null){if(i===Xo)return s.COMPRESSED_RED_RGTC1_EXT;if(i===qo)return s.COMPRESSED_SIGNED_RED_RGTC1_EXT;if(i===ra)return s.COMPRESSED_RED_GREEN_RGTC2_EXT;if(i===Yo)return s.COMPRESSED_SIGNED_RED_GREEN_RGTC2_EXT}else return null;return i===us?n.UNSIGNED_INT_24_8:n[i]!==void 0?n[i]:null}return{convert:t}}const t0=`
void main() {

	gl_Position = vec4( position, 1.0 );

}`,n0=`
uniform sampler2DArray depthColor;
uniform float depthWidth;
uniform float depthHeight;

void main() {

	vec2 coord = vec2( gl_FragCoord.x / depthWidth, gl_FragCoord.y / depthHeight );

	if ( coord.x >= 1.0 ) {

		gl_FragDepth = texture( depthColor, vec3( coord.x - 1.0, coord.y, 1 ) ).r;

	} else {

		gl_FragDepth = texture( depthColor, vec3( coord.x, coord.y, 0 ) ).r;

	}

}`;class i0{constructor(){this.texture=null,this.mesh=null,this.depthNear=0,this.depthFar=0}init(e,t){if(this.texture===null){const i=new Dd(e.texture);(e.depthNear!==t.depthNear||e.depthFar!==t.depthFar)&&(this.depthNear=e.depthNear,this.depthFar=e.depthFar),this.texture=i}}getMesh(e){if(this.texture!==null&&this.mesh===null){const t=e.cameras[0].viewport,i=new si({vertexShader:t0,fragmentShader:n0,uniforms:{depthColor:{value:this.texture},depthWidth:{value:t.z},depthHeight:{value:t.w}}});this.mesh=new ri(new ha(20,20),i)}return this.mesh}reset(){this.texture=null,this.mesh=null}getDepthTexture(){return this.texture}}class r0 extends dr{constructor(e,t){super();const i=this;let r=null,s=1,a=null,o="local-floor",l=1,c=null,f=null,u=null,d=null,h=null,_=null;const x=typeof XRWebGLBinding<"u",m=new i0,p={},C=t.getContextAttributes();let D=null,M=null;const L=[],A=[],P=new At;let v=null;const b=new zn;b.viewport=new Xt;const w=new zn;w.viewport=new Xt;const E=[b,w],N=new ph;let G=null,W=null;this.cameraAutoUpdate=!0,this.enabled=!1,this.isPresenting=!1,this.getController=function(H){let te=L[H];return te===void 0&&(te=new La,L[H]=te),te.getTargetRaySpace()},this.getControllerGrip=function(H){let te=L[H];return te===void 0&&(te=new La,L[H]=te),te.getGripSpace()},this.getHand=function(H){let te=L[H];return te===void 0&&(te=new La,L[H]=te),te.getHandSpace()};function I(H){const te=A.indexOf(H.inputSource);if(te===-1)return;const ie=L[te];ie!==void 0&&(ie.update(H.inputSource,H.frame,c||a),ie.dispatchEvent({type:H.type,data:H.inputSource}))}function R(){r.removeEventListener("select",I),r.removeEventListener("selectstart",I),r.removeEventListener("selectend",I),r.removeEventListener("squeeze",I),r.removeEventListener("squeezestart",I),r.removeEventListener("squeezeend",I),r.removeEventListener("end",R),r.removeEventListener("inputsourceschange",F);for(let H=0;H<L.length;H++){const te=A[H];te!==null&&(A[H]=null,L[H].disconnect(te))}G=null,W=null,m.reset();for(const H in p)delete p[H];e.setRenderTarget(D),h=null,d=null,u=null,r=null,M=null,ne.stop(),i.isPresenting=!1,e.setPixelRatio(v),e.setSize(P.width,P.height,!1),i.dispatchEvent({type:"sessionend"})}this.setFramebufferScaleFactor=function(H){s=H,i.isPresenting===!0&&st("WebXRManager: Cannot change framebuffer scale while presenting.")},this.setReferenceSpaceType=function(H){o=H,i.isPresenting===!0&&st("WebXRManager: Cannot change reference space type while presenting.")},this.getReferenceSpace=function(){return c||a},this.setReferenceSpace=function(H){c=H},this.getBaseLayer=function(){return d!==null?d:h},this.getBinding=function(){return u===null&&x&&(u=new XRWebGLBinding(r,t)),u},this.getFrame=function(){return _},this.getSession=function(){return r},this.setSession=async function(H){if(r=H,r!==null){if(D=e.getRenderTarget(),r.addEventListener("select",I),r.addEventListener("selectstart",I),r.addEventListener("selectend",I),r.addEventListener("squeeze",I),r.addEventListener("squeezestart",I),r.addEventListener("squeezeend",I),r.addEventListener("end",R),r.addEventListener("inputsourceschange",F),C.xrCompatible!==!0&&await t.makeXRCompatible(),v=e.getPixelRatio(),e.getSize(P),x&&"createProjectionLayer"in XRWebGLBinding.prototype){let ie=null,X=null,he=null;C.depth&&(he=C.stencil?t.DEPTH24_STENCIL8:t.DEPTH_COMPONENT24,ie=C.stencil?rr:Ti,X=C.stencil?us:ii);const _e={colorFormat:t.RGBA8,depthFormat:he,scaleFactor:s};u=this.getBinding(),d=u.createProjectionLayer(_e),r.updateRenderState({layers:[d]}),e.setPixelRatio(1),e.setSize(d.textureWidth,d.textureHeight,!1),M=new ni(d.textureWidth,d.textureHeight,{format:Hn,type:In,depthTexture:new Gr(d.textureWidth,d.textureHeight,X,void 0,void 0,void 0,void 0,void 0,void 0,ie),stencilBuffer:C.stencil,colorSpace:e.outputColorSpace,samples:C.antialias?4:0,resolveDepthBuffer:d.ignoreDepthValues===!1,resolveStencilBuffer:d.ignoreDepthValues===!1})}else{const ie={antialias:C.antialias,alpha:!0,depth:C.depth,stencil:C.stencil,framebufferScaleFactor:s};h=new XRWebGLLayer(r,t,ie),r.updateRenderState({baseLayer:h}),e.setPixelRatio(1),e.setSize(h.framebufferWidth,h.framebufferHeight,!1),M=new ni(h.framebufferWidth,h.framebufferHeight,{format:Hn,type:In,colorSpace:e.outputColorSpace,stencilBuffer:C.stencil,resolveDepthBuffer:h.ignoreDepthValues===!1,resolveStencilBuffer:h.ignoreDepthValues===!1})}M.isXRRenderTarget=!0,this.setFoveation(l),c=null,a=await r.requestReferenceSpace(o),ne.setContext(r),ne.start(),i.isPresenting=!0,i.dispatchEvent({type:"sessionstart"})}},this.getEnvironmentBlendMode=function(){if(r!==null)return r.environmentBlendMode},this.getDepthTexture=function(){return m.getDepthTexture()};function F(H){for(let te=0;te<H.removed.length;te++){const ie=H.removed[te],X=A.indexOf(ie);X>=0&&(A[X]=null,L[X].disconnect(ie))}for(let te=0;te<H.added.length;te++){const ie=H.added[te];let X=A.indexOf(ie);if(X===-1){for(let _e=0;_e<L.length;_e++)if(_e>=A.length){A.push(ie),X=_e;break}else if(A[_e]===null){A[_e]=ie,X=_e;break}if(X===-1)break}const he=L[X];he&&he.connect(ie)}}const J=new le,B=new le;function oe(H,te,ie){J.setFromMatrixPosition(te.matrixWorld),B.setFromMatrixPosition(ie.matrixWorld);const X=J.distanceTo(B),he=te.projectionMatrix.elements,_e=ie.projectionMatrix.elements,we=he[14]/(he[10]-1),ee=he[14]/(he[10]+1),ce=(he[9]+1)/he[5],$=(he[9]-1)/he[5],pe=(he[8]-1)/he[0],ye=(_e[8]+1)/_e[0],Te=we*pe,Pe=we*ye,Ge=X/(-pe+ye),Ee=Ge*-pe;if(te.matrixWorld.decompose(H.position,H.quaternion,H.scale),H.translateX(Ee),H.translateZ(Ge),H.matrixWorld.compose(H.position,H.quaternion,H.scale),H.matrixWorldInverse.copy(H.matrixWorld).invert(),he[10]===-1)H.projectionMatrix.copy(te.projectionMatrix),H.projectionMatrixInverse.copy(te.projectionMatrixInverse);else{const ve=we+Ge,O=ee+Ge,Xe=Te-Ee,Ze=Pe+(X-Ee),y=ce*ee/O*ve,g=$*ee/O*ve;H.projectionMatrix.makePerspective(Xe,Ze,y,g,ve,O),H.projectionMatrixInverse.copy(H.projectionMatrix).invert()}}function k(H,te){te===null?H.matrixWorld.copy(H.matrix):H.matrixWorld.multiplyMatrices(te.matrixWorld,H.matrix),H.matrixWorldInverse.copy(H.matrixWorld).invert()}this.updateCamera=function(H){if(r===null)return;let te=H.near,ie=H.far;m.texture!==null&&(m.depthNear>0&&(te=m.depthNear),m.depthFar>0&&(ie=m.depthFar)),N.near=w.near=b.near=te,N.far=w.far=b.far=ie,(G!==N.near||W!==N.far)&&(r.updateRenderState({depthNear:N.near,depthFar:N.far}),G=N.near,W=N.far),N.layers.mask=H.layers.mask|6,b.layers.mask=N.layers.mask&-5,w.layers.mask=N.layers.mask&-3;const X=H.parent,he=N.cameras;k(N,X);for(let _e=0;_e<he.length;_e++)k(he[_e],X);he.length===2?oe(N,b,w):N.projectionMatrix.copy(b.projectionMatrix),K(H,N,X)};function K(H,te,ie){ie===null?H.matrix.copy(te.matrixWorld):(H.matrix.copy(ie.matrixWorld),H.matrix.invert(),H.matrix.multiply(te.matrixWorld)),H.matrix.decompose(H.position,H.quaternion,H.scale),H.updateMatrixWorld(!0),H.projectionMatrix.copy(te.projectionMatrix),H.projectionMatrixInverse.copy(te.projectionMatrixInverse),H.isPerspectiveCamera&&(H.fov=Zo*2*Math.atan(1/H.projectionMatrix.elements[5]),H.zoom=1)}this.getCamera=function(){return N},this.getFoveation=function(){if(!(d===null&&h===null))return l},this.setFoveation=function(H){l=H,d!==null&&(d.fixedFoveation=H),h!==null&&h.fixedFoveation!==void 0&&(h.fixedFoveation=H)},this.hasDepthSensing=function(){return m.texture!==null},this.getDepthSensingMesh=function(){return m.getMesh(N)},this.getCameraTexture=function(H){return p[H]};let Z=null;function Y(H,te){if(f=te.getViewerPose(c||a),_=te,f!==null){const ie=f.views;h!==null&&(e.setRenderTargetFramebuffer(M,h.framebuffer),e.setRenderTarget(M));let X=!1;ie.length!==N.cameras.length&&(N.cameras.length=0,X=!0);for(let ee=0;ee<ie.length;ee++){const ce=ie[ee];let $=null;if(h!==null)$=h.getViewport(ce);else{const ye=u.getViewSubImage(d,ce);$=ye.viewport,ee===0&&(e.setRenderTargetTextures(M,ye.colorTexture,ye.depthStencilTexture),e.setRenderTarget(M))}let pe=E[ee];pe===void 0&&(pe=new zn,pe.layers.enable(ee),pe.viewport=new Xt,E[ee]=pe),pe.matrix.fromArray(ce.transform.matrix),pe.matrix.decompose(pe.position,pe.quaternion,pe.scale),pe.projectionMatrix.fromArray(ce.projectionMatrix),pe.projectionMatrixInverse.copy(pe.projectionMatrix).invert(),pe.viewport.set($.x,$.y,$.width,$.height),ee===0&&(N.matrix.copy(pe.matrix),N.matrix.decompose(N.position,N.quaternion,N.scale)),X===!0&&N.cameras.push(pe)}const he=r.enabledFeatures;if(he&&he.includes("depth-sensing")&&r.depthUsage=="gpu-optimized"&&x){u=i.getBinding();const ee=u.getDepthInformation(ie[0]);ee&&ee.isValid&&ee.texture&&m.init(ee,r.renderState)}if(he&&he.includes("camera-access")&&x){e.state.unbindTexture(),u=i.getBinding();for(let ee=0;ee<ie.length;ee++){const ce=ie[ee].camera;if(ce){let $=p[ce];$||($=new Dd,p[ce]=$);const pe=u.getCameraImage(ce);$.sourceTexture=pe}}}}for(let ie=0;ie<L.length;ie++){const X=A[ie],he=L[ie];X!==null&&he!==void 0&&he.update(X,te,c||a)}Z&&Z(H,te),te.detectedPlanes&&i.dispatchEvent({type:"planesdetected",data:te}),_=null}const ne=new Od;ne.setAnimationLoop(Y),this.setAnimationLoop=function(H){Z=H},this.dispose=function(){}}}const s0=new Zt,Wd=new ct;Wd.set(-1,0,0,0,1,0,0,0,1);function a0(n,e){function t(m,p){m.matrixAutoUpdate===!0&&m.updateMatrix(),p.value.copy(m.matrix)}function i(m,p){p.color.getRGB(m.fogColor.value,Nd(n)),p.isFog?(m.fogNear.value=p.near,m.fogFar.value=p.far):p.isFogExp2&&(m.fogDensity.value=p.density)}function r(m,p,C,D,M){p.isNodeMaterial?p.uniformsNeedUpdate=!1:p.isMeshBasicMaterial?s(m,p):p.isMeshLambertMaterial?(s(m,p),p.envMap&&(m.envMapIntensity.value=p.envMapIntensity)):p.isMeshToonMaterial?(s(m,p),u(m,p)):p.isMeshPhongMaterial?(s(m,p),f(m,p),p.envMap&&(m.envMapIntensity.value=p.envMapIntensity)):p.isMeshStandardMaterial?(s(m,p),d(m,p),p.isMeshPhysicalMaterial&&h(m,p,M)):p.isMeshMatcapMaterial?(s(m,p),_(m,p)):p.isMeshDepthMaterial?s(m,p):p.isMeshDistanceMaterial?(s(m,p),x(m,p)):p.isMeshNormalMaterial?s(m,p):p.isLineBasicMaterial?(a(m,p),p.isLineDashedMaterial&&o(m,p)):p.isPointsMaterial?l(m,p,C,D):p.isSpriteMaterial?c(m,p):p.isShadowMaterial?(m.color.value.copy(p.color),m.opacity.value=p.opacity):p.isShaderMaterial&&(p.uniformsNeedUpdate=!1)}function s(m,p){m.opacity.value=p.opacity,p.color&&m.diffuse.value.copy(p.color),p.emissive&&m.emissive.value.copy(p.emissive).multiplyScalar(p.emissiveIntensity),p.map&&(m.map.value=p.map,t(p.map,m.mapTransform)),p.alphaMap&&(m.alphaMap.value=p.alphaMap,t(p.alphaMap,m.alphaMapTransform)),p.bumpMap&&(m.bumpMap.value=p.bumpMap,t(p.bumpMap,m.bumpMapTransform),m.bumpScale.value=p.bumpScale,p.side===vn&&(m.bumpScale.value*=-1)),p.normalMap&&(m.normalMap.value=p.normalMap,t(p.normalMap,m.normalMapTransform),m.normalScale.value.copy(p.normalScale),p.side===vn&&m.normalScale.value.negate()),p.displacementMap&&(m.displacementMap.value=p.displacementMap,t(p.displacementMap,m.displacementMapTransform),m.displacementScale.value=p.displacementScale,m.displacementBias.value=p.displacementBias),p.emissiveMap&&(m.emissiveMap.value=p.emissiveMap,t(p.emissiveMap,m.emissiveMapTransform)),p.specularMap&&(m.specularMap.value=p.specularMap,t(p.specularMap,m.specularMapTransform)),p.alphaTest>0&&(m.alphaTest.value=p.alphaTest);const C=e.get(p),D=C.envMap,M=C.envMapRotation;D&&(m.envMap.value=D,m.envMapRotation.value.setFromMatrix4(s0.makeRotationFromEuler(M)).transpose(),D.isCubeTexture&&D.isRenderTargetTexture===!1&&m.envMapRotation.value.premultiply(Wd),m.reflectivity.value=p.reflectivity,m.ior.value=p.ior,m.refractionRatio.value=p.refractionRatio),p.lightMap&&(m.lightMap.value=p.lightMap,m.lightMapIntensity.value=p.lightMapIntensity,t(p.lightMap,m.lightMapTransform)),p.aoMap&&(m.aoMap.value=p.aoMap,m.aoMapIntensity.value=p.aoMapIntensity,t(p.aoMap,m.aoMapTransform))}function a(m,p){m.diffuse.value.copy(p.color),m.opacity.value=p.opacity,p.map&&(m.map.value=p.map,t(p.map,m.mapTransform))}function o(m,p){m.dashSize.value=p.dashSize,m.totalSize.value=p.dashSize+p.gapSize,m.scale.value=p.scale}function l(m,p,C,D){m.diffuse.value.copy(p.color),m.opacity.value=p.opacity,m.size.value=p.size*C,m.scale.value=D*.5,p.map&&(m.map.value=p.map,t(p.map,m.uvTransform)),p.alphaMap&&(m.alphaMap.value=p.alphaMap,t(p.alphaMap,m.alphaMapTransform)),p.alphaTest>0&&(m.alphaTest.value=p.alphaTest)}function c(m,p){m.diffuse.value.copy(p.color),m.opacity.value=p.opacity,m.rotation.value=p.rotation,p.map&&(m.map.value=p.map,t(p.map,m.mapTransform)),p.alphaMap&&(m.alphaMap.value=p.alphaMap,t(p.alphaMap,m.alphaMapTransform)),p.alphaTest>0&&(m.alphaTest.value=p.alphaTest)}function f(m,p){m.specular.value.copy(p.specular),m.shininess.value=Math.max(p.shininess,1e-4)}function u(m,p){p.gradientMap&&(m.gradientMap.value=p.gradientMap)}function d(m,p){m.metalness.value=p.metalness,p.metalnessMap&&(m.metalnessMap.value=p.metalnessMap,t(p.metalnessMap,m.metalnessMapTransform)),m.roughness.value=p.roughness,p.roughnessMap&&(m.roughnessMap.value=p.roughnessMap,t(p.roughnessMap,m.roughnessMapTransform)),p.envMap&&(m.envMapIntensity.value=p.envMapIntensity)}function h(m,p,C){m.ior.value=p.ior,p.sheen>0&&(m.sheenColor.value.copy(p.sheenColor).multiplyScalar(p.sheen),m.sheenRoughness.value=p.sheenRoughness,p.sheenColorMap&&(m.sheenColorMap.value=p.sheenColorMap,t(p.sheenColorMap,m.sheenColorMapTransform)),p.sheenRoughnessMap&&(m.sheenRoughnessMap.value=p.sheenRoughnessMap,t(p.sheenRoughnessMap,m.sheenRoughnessMapTransform))),p.clearcoat>0&&(m.clearcoat.value=p.clearcoat,m.clearcoatRoughness.value=p.clearcoatRoughness,p.clearcoatMap&&(m.clearcoatMap.value=p.clearcoatMap,t(p.clearcoatMap,m.clearcoatMapTransform)),p.clearcoatRoughnessMap&&(m.clearcoatRoughnessMap.value=p.clearcoatRoughnessMap,t(p.clearcoatRoughnessMap,m.clearcoatRoughnessMapTransform)),p.clearcoatNormalMap&&(m.clearcoatNormalMap.value=p.clearcoatNormalMap,t(p.clearcoatNormalMap,m.clearcoatNormalMapTransform),m.clearcoatNormalScale.value.copy(p.clearcoatNormalScale),p.side===vn&&m.clearcoatNormalScale.value.negate())),p.dispersion>0&&(m.dispersion.value=p.dispersion),p.iridescence>0&&(m.iridescence.value=p.iridescence,m.iridescenceIOR.value=p.iridescenceIOR,m.iridescenceThicknessMinimum.value=p.iridescenceThicknessRange[0],m.iridescenceThicknessMaximum.value=p.iridescenceThicknessRange[1],p.iridescenceMap&&(m.iridescenceMap.value=p.iridescenceMap,t(p.iridescenceMap,m.iridescenceMapTransform)),p.iridescenceThicknessMap&&(m.iridescenceThicknessMap.value=p.iridescenceThicknessMap,t(p.iridescenceThicknessMap,m.iridescenceThicknessMapTransform))),p.transmission>0&&(m.transmission.value=p.transmission,m.transmissionSamplerMap.value=C.texture,m.transmissionSamplerSize.value.set(C.width,C.height),p.transmissionMap&&(m.transmissionMap.value=p.transmissionMap,t(p.transmissionMap,m.transmissionMapTransform)),m.thickness.value=p.thickness,p.thicknessMap&&(m.thicknessMap.value=p.thicknessMap,t(p.thicknessMap,m.thicknessMapTransform)),m.attenuationDistance.value=p.attenuationDistance,m.attenuationColor.value.copy(p.attenuationColor)),p.anisotropy>0&&(m.anisotropyVector.value.set(p.anisotropy*Math.cos(p.anisotropyRotation),p.anisotropy*Math.sin(p.anisotropyRotation)),p.anisotropyMap&&(m.anisotropyMap.value=p.anisotropyMap,t(p.anisotropyMap,m.anisotropyMapTransform))),m.specularIntensity.value=p.specularIntensity,m.specularColor.value.copy(p.specularColor),p.specularColorMap&&(m.specularColorMap.value=p.specularColorMap,t(p.specularColorMap,m.specularColorMapTransform)),p.specularIntensityMap&&(m.specularIntensityMap.value=p.specularIntensityMap,t(p.specularIntensityMap,m.specularIntensityMapTransform))}function _(m,p){p.matcap&&(m.matcap.value=p.matcap)}function x(m,p){const C=e.get(p).light;m.referencePosition.value.setFromMatrixPosition(C.matrixWorld),m.nearDistance.value=C.shadow.camera.near,m.farDistance.value=C.shadow.camera.far}return{refreshFogUniforms:i,refreshMaterialUniforms:r}}function o0(n,e,t,i){let r={},s={},a=[];const o=n.getParameter(n.MAX_UNIFORM_BUFFER_BINDINGS);function l(M,L){const A=L.program;i.uniformBlockBinding(M,A)}function c(M,L){let A=r[M.id];A===void 0&&(m(M),A=f(M),r[M.id]=A,M.addEventListener("dispose",C));const P=L.program;i.updateUBOMapping(M,P);const v=e.render.frame;s[M.id]!==v&&(d(M),s[M.id]=v)}function f(M){const L=u();M.__bindingPointIndex=L;const A=n.createBuffer(),P=M.__size,v=M.usage;return n.bindBuffer(n.UNIFORM_BUFFER,A),n.bufferData(n.UNIFORM_BUFFER,P,v),n.bindBuffer(n.UNIFORM_BUFFER,null),n.bindBufferBase(n.UNIFORM_BUFFER,L,A),A}function u(){for(let M=0;M<o;M++)if(a.indexOf(M)===-1)return a.push(M),M;return Et("WebGLRenderer: Maximum number of simultaneously usable uniforms groups reached."),0}function d(M){const L=r[M.id],A=M.uniforms,P=M.__cache;n.bindBuffer(n.UNIFORM_BUFFER,L);for(let v=0,b=A.length;v<b;v++){const w=A[v];if(Array.isArray(w))for(let E=0,N=w.length;E<N;E++)h(w[E],v,E,P);else h(w,v,0,P)}n.bindBuffer(n.UNIFORM_BUFFER,null)}function h(M,L,A,P){if(x(M,L,A,P)===!0){const v=M.__offset,b=M.value;if(Array.isArray(b)){let w=0;for(let E=0;E<b.length;E++){const N=b[E],G=p(N);_(N,M.__data,w),typeof N!="number"&&typeof N!="boolean"&&!N.isMatrix3&&!ArrayBuffer.isView(N)&&(w+=G.storage/Float32Array.BYTES_PER_ELEMENT)}}else _(b,M.__data,0);n.bufferSubData(n.UNIFORM_BUFFER,v,M.__data)}}function _(M,L,A){typeof M=="number"||typeof M=="boolean"?L[0]=M:M.isMatrix3?(L[0]=M.elements[0],L[1]=M.elements[1],L[2]=M.elements[2],L[3]=0,L[4]=M.elements[3],L[5]=M.elements[4],L[6]=M.elements[5],L[7]=0,L[8]=M.elements[6],L[9]=M.elements[7],L[10]=M.elements[8],L[11]=0):ArrayBuffer.isView(M)?L.set(new M.constructor(M.buffer,M.byteOffset,L.length)):M.toArray(L,A)}function x(M,L,A,P){const v=M.value,b=L+"_"+A;if(P[b]===void 0)return typeof v=="number"||typeof v=="boolean"?P[b]=v:ArrayBuffer.isView(v)?P[b]=v.slice():P[b]=v.clone(),!0;{const w=P[b];if(typeof v=="number"||typeof v=="boolean"){if(w!==v)return P[b]=v,!0}else{if(ArrayBuffer.isView(v))return!0;if(w.equals(v)===!1)return w.copy(v),!0}}return!1}function m(M){const L=M.uniforms;let A=0;const P=16;for(let b=0,w=L.length;b<w;b++){const E=Array.isArray(L[b])?L[b]:[L[b]];for(let N=0,G=E.length;N<G;N++){const W=E[N],I=Array.isArray(W.value)?W.value:[W.value];for(let R=0,F=I.length;R<F;R++){const J=I[R],B=p(J),oe=A%P,k=oe%B.boundary,K=oe+k;A+=k,K!==0&&P-K<B.storage&&(A+=P-K),W.__data=new Float32Array(B.storage/Float32Array.BYTES_PER_ELEMENT),W.__offset=A,A+=B.storage}}}const v=A%P;return v>0&&(A+=P-v),M.__size=A,M.__cache={},this}function p(M){const L={boundary:0,storage:0};return typeof M=="number"||typeof M=="boolean"?(L.boundary=4,L.storage=4):M.isVector2?(L.boundary=8,L.storage=8):M.isVector3||M.isColor?(L.boundary=16,L.storage=12):M.isVector4?(L.boundary=16,L.storage=16):M.isMatrix3?(L.boundary=48,L.storage=48):M.isMatrix4?(L.boundary=64,L.storage=64):M.isTexture?st("WebGLRenderer: Texture samplers can not be part of an uniforms group."):ArrayBuffer.isView(M)?(L.boundary=16,L.storage=M.byteLength):st("WebGLRenderer: Unsupported uniform value type.",M),L}function C(M){const L=M.target;L.removeEventListener("dispose",C);const A=a.indexOf(L.__bindingPointIndex);a.splice(A,1),n.deleteBuffer(r[L.id]),delete r[L.id],delete s[L.id]}function D(){for(const M in r)n.deleteBuffer(r[M]);a=[],r={},s={}}return{bind:l,update:c,dispose:D}}const l0=new Uint16Array([12469,15057,12620,14925,13266,14620,13807,14376,14323,13990,14545,13625,14713,13328,14840,12882,14931,12528,14996,12233,15039,11829,15066,11525,15080,11295,15085,10976,15082,10705,15073,10495,13880,14564,13898,14542,13977,14430,14158,14124,14393,13732,14556,13410,14702,12996,14814,12596,14891,12291,14937,11834,14957,11489,14958,11194,14943,10803,14921,10506,14893,10278,14858,9960,14484,14039,14487,14025,14499,13941,14524,13740,14574,13468,14654,13106,14743,12678,14818,12344,14867,11893,14889,11509,14893,11180,14881,10751,14852,10428,14812,10128,14765,9754,14712,9466,14764,13480,14764,13475,14766,13440,14766,13347,14769,13070,14786,12713,14816,12387,14844,11957,14860,11549,14868,11215,14855,10751,14825,10403,14782,10044,14729,9651,14666,9352,14599,9029,14967,12835,14966,12831,14963,12804,14954,12723,14936,12564,14917,12347,14900,11958,14886,11569,14878,11247,14859,10765,14828,10401,14784,10011,14727,9600,14660,9289,14586,8893,14508,8533,15111,12234,15110,12234,15104,12216,15092,12156,15067,12010,15028,11776,14981,11500,14942,11205,14902,10752,14861,10393,14812,9991,14752,9570,14682,9252,14603,8808,14519,8445,14431,8145,15209,11449,15208,11451,15202,11451,15190,11438,15163,11384,15117,11274,15055,10979,14994,10648,14932,10343,14871,9936,14803,9532,14729,9218,14645,8742,14556,8381,14461,8020,14365,7603,15273,10603,15272,10607,15267,10619,15256,10631,15231,10614,15182,10535,15118,10389,15042,10167,14963,9787,14883,9447,14800,9115,14710,8665,14615,8318,14514,7911,14411,7507,14279,7198,15314,9675,15313,9683,15309,9712,15298,9759,15277,9797,15229,9773,15166,9668,15084,9487,14995,9274,14898,8910,14800,8539,14697,8234,14590,7790,14479,7409,14367,7067,14178,6621,15337,8619,15337,8631,15333,8677,15325,8769,15305,8871,15264,8940,15202,8909,15119,8775,15022,8565,14916,8328,14804,8009,14688,7614,14569,7287,14448,6888,14321,6483,14088,6171,15350,7402,15350,7419,15347,7480,15340,7613,15322,7804,15287,7973,15229,8057,15148,8012,15046,7846,14933,7611,14810,7357,14682,7069,14552,6656,14421,6316,14251,5948,14007,5528,15356,5942,15356,5977,15353,6119,15348,6294,15332,6551,15302,6824,15249,7044,15171,7122,15070,7050,14949,6861,14818,6611,14679,6349,14538,6067,14398,5651,14189,5311,13935,4958,15359,4123,15359,4153,15356,4296,15353,4646,15338,5160,15311,5508,15263,5829,15188,6042,15088,6094,14966,6001,14826,5796,14678,5543,14527,5287,14377,4985,14133,4586,13869,4257,15360,1563,15360,1642,15358,2076,15354,2636,15341,3350,15317,4019,15273,4429,15203,4732,15105,4911,14981,4932,14836,4818,14679,4621,14517,4386,14359,4156,14083,3795,13808,3437,15360,122,15360,137,15358,285,15355,636,15344,1274,15322,2177,15281,2765,15215,3223,15120,3451,14995,3569,14846,3567,14681,3466,14511,3305,14344,3121,14037,2800,13753,2467,15360,0,15360,1,15359,21,15355,89,15346,253,15325,479,15287,796,15225,1148,15133,1492,15008,1749,14856,1882,14685,1886,14506,1783,14324,1608,13996,1398,13702,1183]);let Kn=null;function c0(){return Kn===null&&(Kn=new th(l0,16,16,or,wi),Kn.name="DFG_LUT",Kn.minFilter=ln,Kn.magFilter=ln,Kn.wrapS=xi,Kn.wrapT=xi,Kn.generateMipmaps=!1,Kn.needsUpdate=!0),Kn}class d0{constructor(e={}){const{canvas:t=Pf(),context:i=null,depth:r=!0,stencil:s=!1,alpha:a=!1,antialias:o=!1,premultipliedAlpha:l=!0,preserveDrawingBuffer:c=!1,powerPreference:f="default",failIfMajorPerformanceCaveat:u=!1,reversedDepthBuffer:d=!1,outputBufferType:h=In}=e;this.isWebGLRenderer=!0;let _;if(i!==null){if(typeof WebGLRenderingContext<"u"&&i instanceof WebGLRenderingContext)throw new Error("THREE.WebGLRenderer: WebGL 1 is not supported since r163.");_=i.getContextAttributes().alpha}else _=a;const x=h,m=new Set([fl,ul,dl]),p=new Set([In,ii,ds,us,ll,cl]),C=new Uint32Array(4),D=new Int32Array(4),M=new le;let L=null,A=null;const P=[],v=[];let b=null;this.domElement=t,this.debug={checkShaderErrors:!0,onShaderError:null},this.autoClear=!0,this.autoClearColor=!0,this.autoClearDepth=!0,this.autoClearStencil=!0,this.sortObjects=!0,this.clippingPlanes=[],this.localClippingEnabled=!1,this.toneMapping=Vn,this.toneMappingExposure=1,this.transmissionResolutionScale=1;const w=this;let E=!1,N=null,G=null,W=null,I=null;this._outputColorSpace=Ln;let R=0,F=0,J=null,B=-1,oe=null;const k=new Xt,K=new Xt;let Z=null;const Y=new It(0);let ne=0,H=t.width,te=t.height,ie=1,X=null,he=null;const _e=new Xt(0,0,H,te),we=new Xt(0,0,H,te);let ee=!1;const ce=new Ld;let $=!1,pe=!1;const ye=new Zt,Te=new le,Pe=new Xt,Ge={background:null,fog:null,environment:null,overrideMaterial:null,isScene:!0};let Ee=!1;function ve(){return J===null?ie:1}let O=i;function Xe(S,j){return t.getContext(S,j)}try{const S={alpha:!0,depth:r,stencil:s,antialias:o,premultipliedAlpha:l,preserveDrawingBuffer:c,powerPreference:f,failIfMajorPerformanceCaveat:u};if("setAttribute"in t&&t.setAttribute("data-engine",`three.js r${al}`),t.addEventListener("webglcontextlost",Qe,!1),t.addEventListener("webglcontextrestored",wt,!1),t.addEventListener("webglcontextcreationerror",qt,!1),O===null){const j="webgl2";if(O=Xe(j,S),O===null)throw Xe(j)?new Error("THREE.WebGLRenderer: Error creating WebGL context with your selected attributes."):new Error("THREE.WebGLRenderer: Error creating WebGL context.")}}catch(S){throw Et("WebGLRenderer: "+S.message),S}let Ze,y,g,U,z,Q,ue,be,ae,de,Se,ke,Ae,Re,He,Ke,Je,q,Ce,xe,Le,Ne,Me;function qe(){Ze=new cg(O),Ze.init(),Le=new e0(O,Ze),y=new tg(O,Ze,e,Le),g=new Q_(O,Ze),y.reversedDepthBuffer&&d&&g.buffers.depth.setReversed(!0),G=O.createFramebuffer(),W=O.createFramebuffer(),I=O.createFramebuffer(),U=new fg(O),z=new B_,Q=new j_(O,Ze,g,z,y,Le,U),ue=new lg(w),be=new gh(O),Ne=new jm(O,be),ae=new dg(O,be,U,Ne),de=new pg(O,ae,be,Ne,U),q=new hg(O,y,Q),He=new ng(z),Se=new O_(w,ue,Ze,y,Ne,He),ke=new a0(w,z),Ae=new z_,Re=new q_(Ze),Je=new Qm(w,ue,g,de,_,l),Ke=new J_(w,de,y),Me=new o0(O,U,y,g),Ce=new eg(O,Ze,U),xe=new ug(O,Ze,U),U.programs=Se.programs,w.capabilities=y,w.extensions=Ze,w.properties=z,w.renderLists=Ae,w.shadowMap=Ke,w.state=g,w.info=U}qe(),x!==In&&(b=new gg(x,t.width,t.height,o,r,s));const We=new r0(w,O);this.xr=We,this.getContext=function(){return O},this.getContextAttributes=function(){return O.getContextAttributes()},this.forceContextLoss=function(){const S=Ze.get("WEBGL_lose_context");S&&S.loseContext()},this.forceContextRestore=function(){const S=Ze.get("WEBGL_lose_context");S&&S.restoreContext()},this.getPixelRatio=function(){return ie},this.setPixelRatio=function(S){S!==void 0&&(ie=S,this.setSize(H,te,!1))},this.getSize=function(S){return S.set(H,te)},this.setSize=function(S,j,fe=!0){if(We.isPresenting){st("WebGLRenderer: Can't change size while VR device is presenting.");return}H=S,te=j,t.width=Math.floor(S*ie),t.height=Math.floor(j*ie),fe===!0&&(t.style.width=S+"px",t.style.height=j+"px"),b!==null&&b.setSize(t.width,t.height),this.setViewport(0,0,S,j)},this.getDrawingBufferSize=function(S){return S.set(H*ie,te*ie).floor()},this.setDrawingBufferSize=function(S,j,fe){H=S,te=j,ie=fe,t.width=Math.floor(S*fe),t.height=Math.floor(j*fe),this.setViewport(0,0,S,j)},this.setEffects=function(S){if(x===In){Et("WebGLRenderer: setEffects() requires outputBufferType set to HalfFloatType or FloatType.");return}if(S){for(let j=0;j<S.length;j++)if(S[j].isOutputPass===!0){st("WebGLRenderer: OutputPass is not needed in setEffects(). Tone mapping and color space conversion are applied automatically.");break}}b.setEffects(S||[])},this.getCurrentViewport=function(S){return S.copy(k)},this.getViewport=function(S){return S.copy(_e)},this.setViewport=function(S,j,fe,re){S.isVector4?_e.set(S.x,S.y,S.z,S.w):_e.set(S,j,fe,re),g.viewport(k.copy(_e).multiplyScalar(ie).round())},this.getScissor=function(S){return S.copy(we)},this.setScissor=function(S,j,fe,re){S.isVector4?we.set(S.x,S.y,S.z,S.w):we.set(S,j,fe,re),g.scissor(K.copy(we).multiplyScalar(ie).round())},this.getScissorTest=function(){return ee},this.setScissorTest=function(S){g.setScissorTest(ee=S)},this.setOpaqueSort=function(S){X=S},this.setTransparentSort=function(S){he=S},this.getClearColor=function(S){return S.copy(Je.getClearColor())},this.setClearColor=function(){Je.setClearColor(...arguments)},this.getClearAlpha=function(){return Je.getClearAlpha()},this.setClearAlpha=function(){Je.setClearAlpha(...arguments)},this.clear=function(S=!0,j=!0,fe=!0){let re=0;if(S){let se=!1;if(J!==null){const De=J.texture.format;se=m.has(De)}if(se){const De=J.texture.type,Oe=p.has(De),Ie=Je.getClearColor(),Be=Je.getClearAlpha(),ze=Ie.r,rt=Ie.g,ft=Ie.b;Oe?(C[0]=ze,C[1]=rt,C[2]=ft,C[3]=Be,O.clearBufferuiv(O.COLOR,0,C)):(D[0]=ze,D[1]=rt,D[2]=ft,D[3]=Be,O.clearBufferiv(O.COLOR,0,D))}else re|=O.COLOR_BUFFER_BIT}j&&(re|=O.DEPTH_BUFFER_BIT,this.state.buffers.depth.setMask(!0)),fe&&(re|=O.STENCIL_BUFFER_BIT,this.state.buffers.stencil.setMask(4294967295)),re!==0&&O.clear(re)},this.clearColor=function(){this.clear(!0,!1,!1)},this.clearDepth=function(){this.clear(!1,!0,!1)},this.clearStencil=function(){this.clear(!1,!1,!0)},this.setNodesHandler=function(S){S.setRenderer(this),N=S},this.dispose=function(){t.removeEventListener("webglcontextlost",Qe,!1),t.removeEventListener("webglcontextrestored",wt,!1),t.removeEventListener("webglcontextcreationerror",qt,!1),Je.dispose(),Ae.dispose(),Re.dispose(),z.dispose(),ue.dispose(),de.dispose(),Ne.dispose(),Me.dispose(),Se.dispose(),We.dispose(),We.removeEventListener("sessionstart",gs),We.removeEventListener("sessionend",Yr),An.stop()};function Qe(S){S.preventDefault(),la("WebGLRenderer: Context Lost."),E=!0}function wt(){la("WebGLRenderer: Context Restored."),E=!1;const S=U.autoReset,j=Ke.enabled,fe=Ke.autoUpdate,re=Ke.needsUpdate,se=Ke.type;qe(),U.autoReset=S,Ke.enabled=j,Ke.autoUpdate=fe,Ke.needsUpdate=re,Ke.type=se}function qt(S){Et("WebGLRenderer: A WebGL context could not be created. Reason: ",S.statusMessage)}function St(S){const j=S.target;j.removeEventListener("dispose",St),Ai(j)}function Ai(S){sn(S),z.remove(S)}function sn(S){const j=z.get(S).programs;j!==void 0&&(j.forEach(function(fe){Se.releaseProgram(fe)}),S.isShaderMaterial&&Se.releaseShaderCache(S))}this.renderBufferDirect=function(S,j,fe,re,se,De){j===null&&(j=Ge);const Oe=se.isMesh&&se.matrixWorld.determinantAffine()<0,Ie=xn(S,j,fe,re,se);g.setMaterial(re,Oe);let Be=fe.index,ze=1;if(re.wireframe===!0){if(Be=ae.getWireframeAttribute(fe),Be===void 0)return;ze=2}const rt=fe.drawRange,ft=fe.attributes.position;let Ye=rt.start*ze,Tt=(rt.start+rt.count)*ze;De!==null&&(Ye=Math.max(Ye,De.start*ze),Tt=Math.min(Tt,(De.start+De.count)*ze)),Be!==null?(Ye=Math.max(Ye,0),Tt=Math.min(Tt,Be.count)):ft!=null&&(Ye=Math.max(Ye,0),Tt=Math.min(Tt,ft.count));const Gt=Tt-Ye;if(Gt<0||Gt===1/0)return;Ne.setup(se,re,Ie,fe,Be);let Bt,_t=Ce;if(Be!==null&&(Bt=be.get(Be),_t=xe,_t.setIndex(Bt)),se.isMesh)re.wireframe===!0?(g.setLineWidth(re.wireframeLinewidth*ve()),_t.setMode(O.LINES)):_t.setMode(O.TRIANGLES);else if(se.isLine){let Jt=re.linewidth;Jt===void 0&&(Jt=1),g.setLineWidth(Jt*ve()),se.isLineSegments?_t.setMode(O.LINES):se.isLineLoop?_t.setMode(O.LINE_LOOP):_t.setMode(O.LINE_STRIP)}else se.isPoints?_t.setMode(O.POINTS):se.isSprite&&_t.setMode(O.TRIANGLES);if(se.isBatchedMesh)if(Ze.get("WEBGL_multi_draw"))_t.renderMultiDraw(se._multiDrawStarts,se._multiDrawCounts,se._multiDrawCount);else{const Jt=se._multiDrawStarts,Fe=se._multiDrawCounts,an=se._multiDrawCount,vt=Be?be.get(Be).bytesPerElement:1,Ht=z.get(re).currentProgram.getUniforms();for(let yn=0;yn<an;yn++)Ht.setValue(O,"_gl_DrawID",yn),_t.render(Jt[yn]/vt,Fe[yn])}else if(se.isInstancedMesh)_t.renderInstances(Ye,Gt,se.count);else if(fe.isInstancedBufferGeometry){const Jt=fe._maxInstanceCount!==void 0?fe._maxInstanceCount:1/0,Fe=Math.min(fe.instanceCount,Jt);_t.renderInstances(Ye,Gt,Fe)}else _t.render(Ye,Gt)};function qr(S,j,fe){S.transparent===!0&&S.side===gi&&S.forceSinglePass===!1?(S.side=vn,S.needsUpdate=!0,oi(S,j,fe),S.side=Gi,S.needsUpdate=!0,oi(S,j,fe),S.side=gi):oi(S,j,fe)}this.compile=function(S,j,fe=null){fe===null&&(fe=S),A=Re.get(fe),A.init(j),v.push(A),fe.traverseVisible(function(se){se.isLight&&se.layers.test(j.layers)&&(A.pushLight(se),se.castShadow&&A.pushShadow(se))}),S!==fe&&S.traverseVisible(function(se){se.isLight&&se.layers.test(j.layers)&&(A.pushLight(se),se.castShadow&&A.pushShadow(se))}),A.setupLights();const re=new Set;return S.traverse(function(se){if(!(se.isMesh||se.isPoints||se.isLine||se.isSprite))return;const De=se.material;if(De)if(Array.isArray(De))for(let Oe=0;Oe<De.length;Oe++){const Ie=De[Oe];qr(Ie,fe,se),re.add(Ie)}else qr(De,fe,se),re.add(De)}),A=v.pop(),re},this.compileAsync=function(S,j,fe=null){const re=this.compile(S,j,fe);return new Promise(se=>{function De(){if(re.forEach(function(Oe){z.get(Oe).currentProgram.isReady()&&re.delete(Oe)}),re.size===0){se(S);return}setTimeout(De,10)}Ze.get("KHR_parallel_shader_compile")!==null?De():setTimeout(De,10)})};let Vi=null;function ga(S){Vi&&Vi(S)}function gs(){An.stop()}function Yr(){An.start()}const An=new Od;An.setAnimationLoop(ga),typeof self<"u"&&An.setContext(self),this.setAnimationLoop=function(S){Vi=S,We.setAnimationLoop(S),S===null?An.stop():An.start()},We.addEventListener("sessionstart",gs),We.addEventListener("sessionend",Yr),this.render=function(S,j){if(j!==void 0&&j.isCamera!==!0){Et("WebGLRenderer.render: camera is not an instance of THREE.Camera.");return}if(E===!0)return;N!==null&&N.renderStart(S,j);const fe=We.enabled===!0&&We.isPresenting===!0,re=b!==null&&(J===null||fe)&&b.begin(w,J);if(S.matrixWorldAutoUpdate===!0&&S.updateMatrixWorld(),j.parent===null&&j.matrixWorldAutoUpdate===!0&&j.updateMatrixWorld(),We.enabled===!0&&We.isPresenting===!0&&(b===null||b.isCompositing()===!1)&&(We.cameraAutoUpdate===!0&&We.updateCamera(j),j=We.getCamera()),S.isScene===!0&&S.onBeforeRender(w,S,j,J),A=Re.get(S,v.length),A.init(j),A.state.textureUnits=Q.getTextureUnits(),v.push(A),ye.multiplyMatrices(j.projectionMatrix,j.matrixWorldInverse),ce.setFromProjectionMatrix(ye,ti,j.reversedDepth),pe=this.localClippingEnabled,$=He.init(this.clippingPlanes,pe),L=Ae.get(S,P.length),L.init(),P.push(L),We.enabled===!0&&We.isPresenting===!0){const Oe=w.xr.getDepthSensingMesh();Oe!==null&&ur(Oe,j,-1/0,w.sortObjects)}ur(S,j,0,w.sortObjects),L.finish(),w.sortObjects===!0&&L.sort(X,he,j.reversedDepth),Ee=We.enabled===!1||We.isPresenting===!1||We.hasDepthSensing()===!1,Ee&&Je.addToRenderList(L,S),this.info.render.frame++,this.info.autoReset===!0&&this.info.reset(),$===!0&&He.beginShadows();const se=A.state.shadowsArray;if(Ke.render(se,S,j),$===!0&&He.endShadows(),(re&&b.hasRenderPass())===!1){const Oe=L.opaque,Ie=L.transmissive;if(A.setupLights(),j.isArrayCamera){const Be=j.cameras;if(Ie.length>0)for(let ze=0,rt=Be.length;ze<rt;ze++){const ft=Be[ze];qn(Oe,Ie,S,ft)}Ee&&Je.render(S);for(let ze=0,rt=Be.length;ze<rt;ze++){const ft=Be[ze];$r(L,S,ft,ft.viewport)}}else Ie.length>0&&qn(Oe,Ie,S,j),Ee&&Je.render(S),$r(L,S,j)}J!==null&&F===0&&(Q.updateMultisampleRenderTarget(J),Q.updateRenderTargetMipmap(J)),re&&b.end(w),S.isScene===!0&&S.onAfterRender(w,S,j),Ne.resetDefaultState(),B=-1,oe=null,v.pop(),v.length>0?(A=v[v.length-1],Q.setTextureUnits(A.state.textureUnits),$===!0&&He.setGlobalState(w.clippingPlanes,A.state.camera)):A=null,P.pop(),P.length>0?L=P[P.length-1]:L=null,N!==null&&N.renderEnd()};function ur(S,j,fe,re){if(S.visible===!1)return;if(S.layers.test(j.layers)){if(S.isGroup)fe=S.renderOrder;else if(S.isLOD)S.autoUpdate===!0&&S.update(j);else if(S.isLightProbeGrid)A.pushLightProbeGrid(S);else if(S.isLight)A.pushLight(S),S.castShadow&&A.pushShadow(S);else if(S.isSprite){if(!S.frustumCulled||ce.intersectsSprite(S)){re&&Pe.setFromMatrixPosition(S.matrixWorld).applyMatrix4(ye);const Oe=de.update(S),Ie=S.material;Ie.visible&&L.push(S,Oe,Ie,fe,Pe.z,null)}}else if((S.isMesh||S.isLine||S.isPoints)&&(!S.frustumCulled||ce.intersectsObject(S))){const Oe=de.update(S),Ie=S.material;if(re&&(S.boundingSphere!==void 0?(S.boundingSphere===null&&S.computeBoundingSphere(),Pe.copy(S.boundingSphere.center)):(Oe.boundingSphere===null&&Oe.computeBoundingSphere(),Pe.copy(Oe.boundingSphere.center)),Pe.applyMatrix4(S.matrixWorld).applyMatrix4(ye)),Array.isArray(Ie)){const Be=Oe.groups;for(let ze=0,rt=Be.length;ze<rt;ze++){const ft=Be[ze],Ye=Ie[ft.materialIndex];Ye&&Ye.visible&&L.push(S,Oe,Ye,fe,Pe.z,ft)}}else Ie.visible&&L.push(S,Oe,Ie,fe,Pe.z,null)}}const De=S.children;for(let Oe=0,Ie=De.length;Oe<Ie;Oe++)ur(De[Oe],j,fe,re)}function $r(S,j,fe,re){const{opaque:se,transmissive:De,transparent:Oe}=S;A.setupLightsView(fe),$===!0&&He.setGlobalState(w.clippingPlanes,fe),re&&g.viewport(k.copy(re)),se.length>0&&Dn(se,j,fe),De.length>0&&Dn(De,j,fe),Oe.length>0&&Dn(Oe,j,fe),g.buffers.depth.setTest(!0),g.buffers.depth.setMask(!0),g.buffers.color.setMask(!0),g.setPolygonOffset(!1)}function qn(S,j,fe,re){if((fe.isScene===!0?fe.overrideMaterial:null)!==null)return;if(A.state.transmissionRenderTarget[re.id]===void 0){const Ye=Ze.has("EXT_color_buffer_half_float")||Ze.has("EXT_color_buffer_float");A.state.transmissionRenderTarget[re.id]=new ni(1,1,{generateMipmaps:!0,type:Ye?wi:In,minFilter:ir,samples:Math.max(4,y.samples),stencilBuffer:s,resolveDepthBuffer:!1,resolveStencilBuffer:!1,colorSpace:yt.workingColorSpace})}const De=A.state.transmissionRenderTarget[re.id],Oe=re.viewport||k;De.setSize(Oe.z*w.transmissionResolutionScale,Oe.w*w.transmissionResolutionScale);const Ie=w.getRenderTarget(),Be=w.getActiveCubeFace(),ze=w.getActiveMipmapLevel();w.setRenderTarget(De),w.getClearColor(Y),ne=w.getClearAlpha(),ne<1&&w.setClearColor(16777215,.5),w.clear(),Ee&&Je.render(fe);const rt=w.toneMapping;w.toneMapping=Vn;const ft=re.viewport;if(re.viewport!==void 0&&(re.viewport=void 0),A.setupLightsView(re),$===!0&&He.setGlobalState(w.clippingPlanes,re),Dn(S,fe,re),Q.updateMultisampleRenderTarget(De),Q.updateRenderTargetMipmap(De),Ze.has("WEBGL_multisampled_render_to_texture")===!1){let Ye=!1;for(let Tt=0,Gt=j.length;Tt<Gt;Tt++){const Bt=j[Tt],{object:_t,geometry:Jt,material:Fe,group:an}=Bt;if(Fe.side===gi&&_t.layers.test(re.layers)){const vt=Fe.side;Fe.side=vn,Fe.needsUpdate=!0,Kr(_t,fe,re,Jt,Fe,an),Fe.side=vt,Fe.needsUpdate=!0,Ye=!0}}Ye===!0&&(Q.updateMultisampleRenderTarget(De),Q.updateRenderTargetMipmap(De))}w.setRenderTarget(Ie,Be,ze),w.setClearColor(Y,ne),ft!==void 0&&(re.viewport=ft),w.toneMapping=rt}function Dn(S,j,fe){const re=j.isScene===!0?j.overrideMaterial:null;for(let se=0,De=S.length;se<De;se++){const Oe=S[se],{object:Ie,geometry:Be,group:ze}=Oe;let rt=Oe.material;rt.allowOverride===!0&&re!==null&&(rt=re),Ie.layers.test(fe.layers)&&Kr(Ie,j,fe,Be,rt,ze)}}function Kr(S,j,fe,re,se,De){S.onBeforeRender(w,j,fe,re,se,De),S.modelViewMatrix.multiplyMatrices(fe.matrixWorldInverse,S.matrixWorld),S.normalMatrix.getNormalMatrix(S.modelViewMatrix),se.onBeforeRender(w,j,fe,re,S,De),se.transparent===!0&&se.side===gi&&se.forceSinglePass===!1?(se.side=vn,se.needsUpdate=!0,w.renderBufferDirect(fe,j,re,se,S,De),se.side=Gi,se.needsUpdate=!0,w.renderBufferDirect(fe,j,re,se,S,De),se.side=gi):w.renderBufferDirect(fe,j,re,se,S,De),S.onAfterRender(w,j,fe,re,se,De)}function oi(S,j,fe){j.isScene!==!0&&(j=Ge);const re=z.get(S),se=A.state.lights,De=A.state.shadowsArray,Oe=se.state.version,Ie=Se.getParameters(S,se.state,De,j,fe,A.state.lightProbeGridArray),Be=Se.getProgramCacheKey(Ie);let ze=re.programs;re.environment=S.isMeshStandardMaterial||S.isMeshLambertMaterial||S.isMeshPhongMaterial?j.environment:null,re.fog=j.fog;const rt=S.isMeshStandardMaterial||S.isMeshLambertMaterial&&!S.envMap||S.isMeshPhongMaterial&&!S.envMap;re.envMap=ue.get(S.envMap||re.environment,rt),re.envMapRotation=re.environment!==null&&S.envMap===null?j.environmentRotation:S.envMapRotation,ze===void 0&&(S.addEventListener("dispose",St),ze=new Map,re.programs=ze);let ft=ze.get(Be);if(ft!==void 0){if(re.currentProgram===ft&&re.lightsStateVersion===Oe)return Ri(S,Ie),ft}else Ie.uniforms=Se.getUniforms(S),N!==null&&S.isNodeMaterial&&N.build(S,fe,Ie),S.onBeforeCompile(Ie,w),ft=Se.acquireProgram(Ie,Be),ze.set(Be,ft),re.uniforms=Ie.uniforms;const Ye=re.uniforms;return(!S.isShaderMaterial&&!S.isRawShaderMaterial||S.clipping===!0)&&(Ye.clippingPlanes=He.uniform),Ri(S,Ie),re.needsLights=_s(S),re.lightsStateVersion=Oe,re.needsLights&&(Ye.ambientLightColor.value=se.state.ambient,Ye.lightProbe.value=se.state.probe,Ye.directionalLights.value=se.state.directional,Ye.directionalLightShadows.value=se.state.directionalShadow,Ye.spotLights.value=se.state.spot,Ye.spotLightShadows.value=se.state.spotShadow,Ye.rectAreaLights.value=se.state.rectArea,Ye.ltc_1.value=se.state.rectAreaLTC1,Ye.ltc_2.value=se.state.rectAreaLTC2,Ye.pointLights.value=se.state.point,Ye.pointLightShadows.value=se.state.pointShadow,Ye.hemisphereLights.value=se.state.hemi,Ye.directionalShadowMatrix.value=se.state.directionalShadowMatrix,Ye.spotLightMatrix.value=se.state.spotLightMatrix,Ye.spotLightMap.value=se.state.spotLightMap,Ye.pointShadowMatrix.value=se.state.pointShadowMatrix),re.lightProbeGrid=A.state.lightProbeGridArray.length>0,re.currentProgram=ft,re.uniformsList=null,ft}function Wi(S){if(S.uniformsList===null){const j=S.currentProgram.getUniforms();S.uniformsList=ea.seqWithValue(j.seq,S.uniforms)}return S.uniformsList}function Ri(S,j){const fe=z.get(S);fe.outputColorSpace=j.outputColorSpace,fe.batching=j.batching,fe.batchingColor=j.batchingColor,fe.instancing=j.instancing,fe.instancingColor=j.instancingColor,fe.instancingMorph=j.instancingMorph,fe.skinning=j.skinning,fe.morphTargets=j.morphTargets,fe.morphNormals=j.morphNormals,fe.morphColors=j.morphColors,fe.morphTargetsCount=j.morphTargetsCount,fe.numClippingPlanes=j.numClippingPlanes,fe.numIntersection=j.numClipIntersection,fe.vertexAlphas=j.vertexAlphas,fe.vertexTangents=j.vertexTangents,fe.toneMapping=j.toneMapping}function _a(S,j){if(S.length===0)return null;if(S.length===1)return S[0].texture!==null?S[0]:null;M.setFromMatrixPosition(j.matrixWorld);for(let fe=0,re=S.length;fe<re;fe++){const se=S[fe];if(se.texture!==null&&se.boundingBox.containsPoint(M))return se}return null}function xn(S,j,fe,re,se){j.isScene!==!0&&(j=Ge),Q.resetTextureUnits();const De=j.fog,Oe=re.isMeshStandardMaterial||re.isMeshLambertMaterial||re.isMeshPhongMaterial?j.environment:null,Ie=J===null?w.outputColorSpace:J.isXRRenderTarget===!0?J.texture.colorSpace:yt.workingColorSpace,Be=re.isMeshStandardMaterial||re.isMeshLambertMaterial&&!re.envMap||re.isMeshPhongMaterial&&!re.envMap,ze=ue.get(re.envMap||Oe,Be),rt=re.vertexColors===!0&&!!fe.attributes.color&&fe.attributes.color.itemSize===4,ft=!!fe.attributes.tangent&&(!!re.normalMap||re.anisotropy>0),Ye=!!fe.morphAttributes.position,Tt=!!fe.morphAttributes.normal,Gt=!!fe.morphAttributes.color;let Bt=Vn;re.toneMapped&&(J===null||J.isXRRenderTarget===!0)&&(Bt=w.toneMapping);const _t=fe.morphAttributes.position||fe.morphAttributes.normal||fe.morphAttributes.color,Jt=_t!==void 0?_t.length:0,Fe=z.get(re),an=A.state.lights;if($===!0&&(pe===!0||S!==oe)){const Nt=S===oe&&re.id===B;He.setState(re,S,Nt)}let vt=!1;re.version===Fe.__version?(Fe.needsLights&&Fe.lightsStateVersion!==an.state.version||Fe.outputColorSpace!==Ie||se.isBatchedMesh&&Fe.batching===!1||!se.isBatchedMesh&&Fe.batching===!0||se.isBatchedMesh&&Fe.batchingColor===!0&&se.colorTexture===null||se.isBatchedMesh&&Fe.batchingColor===!1&&se.colorTexture!==null||se.isInstancedMesh&&Fe.instancing===!1||!se.isInstancedMesh&&Fe.instancing===!0||se.isSkinnedMesh&&Fe.skinning===!1||!se.isSkinnedMesh&&Fe.skinning===!0||se.isInstancedMesh&&Fe.instancingColor===!0&&se.instanceColor===null||se.isInstancedMesh&&Fe.instancingColor===!1&&se.instanceColor!==null||se.isInstancedMesh&&Fe.instancingMorph===!0&&se.morphTexture===null||se.isInstancedMesh&&Fe.instancingMorph===!1&&se.morphTexture!==null||Fe.envMap!==ze||re.fog===!0&&Fe.fog!==De||Fe.numClippingPlanes!==void 0&&(Fe.numClippingPlanes!==He.numPlanes||Fe.numIntersection!==He.numIntersection)||Fe.vertexAlphas!==rt||Fe.vertexTangents!==ft||Fe.morphTargets!==Ye||Fe.morphNormals!==Tt||Fe.morphColors!==Gt||Fe.toneMapping!==Bt||Fe.morphTargetsCount!==Jt||!!Fe.lightProbeGrid!=A.state.lightProbeGridArray.length>0)&&(vt=!0):(vt=!0,Fe.__version=re.version);let Ht=Fe.currentProgram;vt===!0&&(Ht=oi(re,j,se),N&&re.isNodeMaterial&&N.onUpdateProgram(re,Ht,Fe));let yn=!1,Rn=!1,li=!1;const Rt=Ht.getUniforms(),kt=Fe.uniforms;if(g.useProgram(Ht.program)&&(yn=!0,Rn=!0,li=!0),re.id!==B&&(B=re.id,Rn=!0),Fe.needsLights){const Nt=_a(A.state.lightProbeGridArray,se);Fe.lightProbeGrid!==Nt&&(Fe.lightProbeGrid=Nt,Rn=!0)}if(yn||oe!==S){g.buffers.depth.getReversed()&&S.reversedDepth!==!0&&(S._reversedDepth=!0,S.updateProjectionMatrix()),Rt.setValue(O,"projectionMatrix",S.projectionMatrix),Rt.setValue(O,"viewMatrix",S.matrixWorldInverse);const Yn=Rt.map.cameraPosition;Yn!==void 0&&Yn.setValue(O,Te.setFromMatrixPosition(S.matrixWorld)),y.logarithmicDepthBuffer&&Rt.setValue(O,"logDepthBufFC",2/(Math.log(S.far+1)/Math.LN2)),(re.isMeshPhongMaterial||re.isMeshToonMaterial||re.isMeshLambertMaterial||re.isMeshBasicMaterial||re.isMeshStandardMaterial||re.isShaderMaterial)&&Rt.setValue(O,"isOrthographic",S.isOrthographicCamera===!0),oe!==S&&(oe=S,Rn=!0,li=!0)}if(Fe.needsLights&&(an.state.directionalShadowMap.length>0&&Rt.setValue(O,"directionalShadowMap",an.state.directionalShadowMap,Q),an.state.spotShadowMap.length>0&&Rt.setValue(O,"spotShadowMap",an.state.spotShadowMap,Q),an.state.pointShadowMap.length>0&&Rt.setValue(O,"pointShadowMap",an.state.pointShadowMap,Q)),se.isSkinnedMesh){Rt.setOptional(O,se,"bindMatrix"),Rt.setOptional(O,se,"bindMatrixInverse");const Nt=se.skeleton;Nt&&(Nt.boneTexture===null&&Nt.computeBoneTexture(),Rt.setValue(O,"boneTexture",Nt.boneTexture,Q))}se.isBatchedMesh&&(Rt.setOptional(O,se,"batchingTexture"),Rt.setValue(O,"batchingTexture",se._matricesTexture,Q),Rt.setOptional(O,se,"batchingIdTexture"),Rt.setValue(O,"batchingIdTexture",se._indirectTexture,Q),Rt.setOptional(O,se,"batchingColorTexture"),se._colorsTexture!==null&&Rt.setValue(O,"batchingColorTexture",se._colorsTexture,Q));const bn=fe.morphAttributes;if((bn.position!==void 0||bn.normal!==void 0||bn.color!==void 0)&&q.update(se,fe,Ht),(Rn||Fe.receiveShadow!==se.receiveShadow)&&(Fe.receiveShadow=se.receiveShadow,Rt.setValue(O,"receiveShadow",se.receiveShadow)),(re.isMeshStandardMaterial||re.isMeshLambertMaterial||re.isMeshPhongMaterial)&&re.envMap===null&&j.environment!==null&&(kt.envMapIntensity.value=j.environmentIntensity),kt.dfgLUT!==void 0&&(kt.dfgLUT.value=c0()),Rn){if(Rt.setValue(O,"toneMappingExposure",w.toneMappingExposure),Fe.needsLights&&Nn(kt,li),De&&re.fog===!0&&ke.refreshFogUniforms(kt,De),ke.refreshMaterialUniforms(kt,re,ie,te,A.state.transmissionRenderTarget[S.id]),Fe.needsLights&&Fe.lightProbeGrid){const Nt=Fe.lightProbeGrid;kt.probesSH.value=Nt.texture,kt.probesMin.value.copy(Nt.boundingBox.min),kt.probesMax.value.copy(Nt.boundingBox.max),kt.probesResolution.value.copy(Nt.resolution)}ea.upload(O,Wi(Fe),kt,Q)}if(re.isShaderMaterial&&re.uniformsNeedUpdate===!0&&(ea.upload(O,Wi(Fe),kt,Q),re.uniformsNeedUpdate=!1),re.isSpriteMaterial&&Rt.setValue(O,"center",se.center),Rt.setValue(O,"modelViewMatrix",se.modelViewMatrix),Rt.setValue(O,"normalMatrix",se.normalMatrix),Rt.setValue(O,"modelMatrix",se.matrixWorld),re.uniformsGroups!==void 0){const Nt=re.uniformsGroups;for(let Yn=0,Ci=Nt.length;Yn<Ci;Yn++){const vs=Nt[Yn];Me.update(vs,Ht),Me.bind(vs,Ht)}}return Ht}function Nn(S,j){S.ambientLightColor.needsUpdate=j,S.lightProbe.needsUpdate=j,S.directionalLights.needsUpdate=j,S.directionalLightShadows.needsUpdate=j,S.pointLights.needsUpdate=j,S.pointLightShadows.needsUpdate=j,S.spotLights.needsUpdate=j,S.spotLightShadows.needsUpdate=j,S.rectAreaLights.needsUpdate=j,S.hemisphereLights.needsUpdate=j}function _s(S){return S.isMeshLambertMaterial||S.isMeshToonMaterial||S.isMeshPhongMaterial||S.isMeshStandardMaterial||S.isShadowMaterial||S.isShaderMaterial&&S.lights===!0}this.getActiveCubeFace=function(){return R},this.getActiveMipmapLevel=function(){return F},this.getRenderTarget=function(){return J},this.setRenderTargetTextures=function(S,j,fe){const re=z.get(S);re.__autoAllocateDepthBuffer=S.resolveDepthBuffer===!1,re.__autoAllocateDepthBuffer===!1&&(re.__useRenderToTexture=!1),z.get(S.texture).__webglTexture=j,z.get(S.depthTexture).__webglTexture=re.__autoAllocateDepthBuffer?void 0:fe,re.__hasExternalTextures=!0},this.setRenderTargetFramebuffer=function(S,j){const fe=z.get(S);fe.__webglFramebuffer=j,fe.__useDefaultFramebuffer=j===void 0},this.setRenderTarget=function(S,j=0,fe=0){J=S,R=j,F=fe;let re=null,se=!1,De=!1;if(S){const Ie=z.get(S);if(Ie.__useDefaultFramebuffer!==void 0){g.bindFramebuffer(O.FRAMEBUFFER,Ie.__webglFramebuffer),k.copy(S.viewport),K.copy(S.scissor),Z=S.scissorTest,g.viewport(k),g.scissor(K),g.setScissorTest(Z),B=-1;return}else if(Ie.__webglFramebuffer===void 0)Q.setupRenderTarget(S);else if(Ie.__hasExternalTextures)Q.rebindTextures(S,z.get(S.texture).__webglTexture,z.get(S.depthTexture).__webglTexture);else if(S.depthBuffer){const rt=S.depthTexture;if(Ie.__boundDepthTexture!==rt){if(rt!==null&&z.has(rt)&&(S.width!==rt.image.width||S.height!==rt.image.height))throw new Error("THREE.WebGLRenderer: Attached DepthTexture is initialized to the incorrect size.");Q.setupDepthRenderbuffer(S)}}const Be=S.texture;(Be.isData3DTexture||Be.isDataArrayTexture||Be.isCompressedArrayTexture)&&(De=!0);const ze=z.get(S).__webglFramebuffer;S.isWebGLCubeRenderTarget?(Array.isArray(ze[j])?re=ze[j][fe]:re=ze[j],se=!0):S.samples>0&&Q.useMultisampledRTT(S)===!1?re=z.get(S).__webglMultisampledFramebuffer:Array.isArray(ze)?re=ze[fe]:re=ze,k.copy(S.viewport),K.copy(S.scissor),Z=S.scissorTest}else k.copy(_e).multiplyScalar(ie).floor(),K.copy(we).multiplyScalar(ie).floor(),Z=ee;if(fe!==0&&(re=G),g.bindFramebuffer(O.FRAMEBUFFER,re)&&g.drawBuffers(S,re),g.viewport(k),g.scissor(K),g.setScissorTest(Z),se){const Ie=z.get(S.texture);O.framebufferTexture2D(O.FRAMEBUFFER,O.COLOR_ATTACHMENT0,O.TEXTURE_CUBE_MAP_POSITIVE_X+j,Ie.__webglTexture,fe)}else if(De){const Ie=j;for(let Be=0;Be<S.textures.length;Be++){const ze=z.get(S.textures[Be]);O.framebufferTextureLayer(O.FRAMEBUFFER,O.COLOR_ATTACHMENT0+Be,ze.__webglTexture,fe,Ie)}}else if(S!==null&&fe!==0){const Ie=z.get(S.texture);O.framebufferTexture2D(O.FRAMEBUFFER,O.COLOR_ATTACHMENT0,O.TEXTURE_2D,Ie.__webglTexture,fe)}B=-1},this.readRenderTargetPixels=function(S,j,fe,re,se,De,Oe,Ie=0){if(!(S&&S.isWebGLRenderTarget)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");return}let Be=z.get(S).__webglFramebuffer;if(S.isWebGLCubeRenderTarget&&Oe!==void 0&&(Be=Be[Oe]),Be){g.bindFramebuffer(O.FRAMEBUFFER,Be);try{const ze=S.textures[Ie],rt=ze.format,ft=ze.type;if(S.textures.length>1&&O.readBuffer(O.COLOR_ATTACHMENT0+Ie),!y.textureFormatReadable(rt)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not in RGBA or implementation defined format.");return}if(!y.textureTypeReadable(ft)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not in UnsignedByteType or implementation defined type.");return}j>=0&&j<=S.width-re&&fe>=0&&fe<=S.height-se&&O.readPixels(j,fe,re,se,Le.convert(rt),Le.convert(ft),De)}finally{const ze=J!==null?z.get(J).__webglFramebuffer:null;g.bindFramebuffer(O.FRAMEBUFFER,ze)}}},this.readRenderTargetPixelsAsync=async function(S,j,fe,re,se,De,Oe,Ie=0){if(!(S&&S.isWebGLRenderTarget))throw new Error("THREE.WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");let Be=z.get(S).__webglFramebuffer;if(S.isWebGLCubeRenderTarget&&Oe!==void 0&&(Be=Be[Oe]),Be)if(j>=0&&j<=S.width-re&&fe>=0&&fe<=S.height-se){g.bindFramebuffer(O.FRAMEBUFFER,Be);const ze=S.textures[Ie],rt=ze.format,ft=ze.type;if(S.textures.length>1&&O.readBuffer(O.COLOR_ATTACHMENT0+Ie),!y.textureFormatReadable(rt))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in RGBA or implementation defined format.");if(!y.textureTypeReadable(ft))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in UnsignedByteType or implementation defined type.");const Ye=O.createBuffer();O.bindBuffer(O.PIXEL_PACK_BUFFER,Ye),O.bufferData(O.PIXEL_PACK_BUFFER,De.byteLength,O.STREAM_READ),O.readPixels(j,fe,re,se,Le.convert(rt),Le.convert(ft),0);const Tt=J!==null?z.get(J).__webglFramebuffer:null;g.bindFramebuffer(O.FRAMEBUFFER,Tt);const Gt=O.fenceSync(O.SYNC_GPU_COMMANDS_COMPLETE,0);return O.flush(),await Lf(O,Gt,4),O.bindBuffer(O.PIXEL_PACK_BUFFER,Ye),O.getBufferSubData(O.PIXEL_PACK_BUFFER,0,De),O.deleteBuffer(Ye),O.deleteSync(Gt),De}else throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: requested read bounds are out of range.")},this.copyFramebufferToTexture=function(S,j=null,fe=0){const re=Math.pow(2,-fe),se=Math.floor(S.image.width*re),De=Math.floor(S.image.height*re),Oe=j!==null?j.x:0,Ie=j!==null?j.y:0;Q.setTexture2D(S,0),O.copyTexSubImage2D(O.TEXTURE_2D,fe,0,0,Oe,Ie,se,De),g.unbindTexture()},this.copyTextureToTexture=function(S,j,fe=null,re=null,se=0,De=0){let Oe,Ie,Be,ze,rt,ft,Ye,Tt,Gt;const Bt=S.isCompressedTexture?S.mipmaps[De]:S.image;if(fe!==null)Oe=fe.max.x-fe.min.x,Ie=fe.max.y-fe.min.y,Be=fe.isBox3?fe.max.z-fe.min.z:1,ze=fe.min.x,rt=fe.min.y,ft=fe.isBox3?fe.min.z:0;else{const kt=Math.pow(2,-se);Oe=Math.floor(Bt.width*kt),Ie=Math.floor(Bt.height*kt),S.isDataArrayTexture?Be=Bt.depth:S.isData3DTexture?Be=Math.floor(Bt.depth*kt):Be=1,ze=0,rt=0,ft=0}re!==null?(Ye=re.x,Tt=re.y,Gt=re.z):(Ye=0,Tt=0,Gt=0);const _t=Le.convert(j.format),Jt=Le.convert(j.type);let Fe;j.isData3DTexture?(Q.setTexture3D(j,0),Fe=O.TEXTURE_3D):j.isDataArrayTexture||j.isCompressedArrayTexture?(Q.setTexture2DArray(j,0),Fe=O.TEXTURE_2D_ARRAY):(Q.setTexture2D(j,0),Fe=O.TEXTURE_2D),g.activeTexture(O.TEXTURE0),g.pixelStorei(O.UNPACK_FLIP_Y_WEBGL,j.flipY),g.pixelStorei(O.UNPACK_PREMULTIPLY_ALPHA_WEBGL,j.premultiplyAlpha),g.pixelStorei(O.UNPACK_ALIGNMENT,j.unpackAlignment);const an=g.getParameter(O.UNPACK_ROW_LENGTH),vt=g.getParameter(O.UNPACK_IMAGE_HEIGHT),Ht=g.getParameter(O.UNPACK_SKIP_PIXELS),yn=g.getParameter(O.UNPACK_SKIP_ROWS),Rn=g.getParameter(O.UNPACK_SKIP_IMAGES);g.pixelStorei(O.UNPACK_ROW_LENGTH,Bt.width),g.pixelStorei(O.UNPACK_IMAGE_HEIGHT,Bt.height),g.pixelStorei(O.UNPACK_SKIP_PIXELS,ze),g.pixelStorei(O.UNPACK_SKIP_ROWS,rt),g.pixelStorei(O.UNPACK_SKIP_IMAGES,ft);const li=S.isDataArrayTexture||S.isData3DTexture,Rt=j.isDataArrayTexture||j.isData3DTexture;if(S.isDepthTexture){const kt=z.get(S),bn=z.get(j),Nt=z.get(kt.__renderTarget),Yn=z.get(bn.__renderTarget);g.bindFramebuffer(O.READ_FRAMEBUFFER,Nt.__webglFramebuffer),g.bindFramebuffer(O.DRAW_FRAMEBUFFER,Yn.__webglFramebuffer);for(let Ci=0;Ci<Be;Ci++)li&&(O.framebufferTextureLayer(O.READ_FRAMEBUFFER,O.COLOR_ATTACHMENT0,z.get(S).__webglTexture,se,ft+Ci),O.framebufferTextureLayer(O.DRAW_FRAMEBUFFER,O.COLOR_ATTACHMENT0,z.get(j).__webglTexture,De,Gt+Ci)),O.blitFramebuffer(ze,rt,Oe,Ie,Ye,Tt,Oe,Ie,O.DEPTH_BUFFER_BIT,O.NEAREST);g.bindFramebuffer(O.READ_FRAMEBUFFER,null),g.bindFramebuffer(O.DRAW_FRAMEBUFFER,null)}else if(se!==0||S.isRenderTargetTexture||z.has(S)){const kt=z.get(S),bn=z.get(j);g.bindFramebuffer(O.READ_FRAMEBUFFER,W),g.bindFramebuffer(O.DRAW_FRAMEBUFFER,I);for(let Nt=0;Nt<Be;Nt++)li?O.framebufferTextureLayer(O.READ_FRAMEBUFFER,O.COLOR_ATTACHMENT0,kt.__webglTexture,se,ft+Nt):O.framebufferTexture2D(O.READ_FRAMEBUFFER,O.COLOR_ATTACHMENT0,O.TEXTURE_2D,kt.__webglTexture,se),Rt?O.framebufferTextureLayer(O.DRAW_FRAMEBUFFER,O.COLOR_ATTACHMENT0,bn.__webglTexture,De,Gt+Nt):O.framebufferTexture2D(O.DRAW_FRAMEBUFFER,O.COLOR_ATTACHMENT0,O.TEXTURE_2D,bn.__webglTexture,De),se!==0?O.blitFramebuffer(ze,rt,Oe,Ie,Ye,Tt,Oe,Ie,O.COLOR_BUFFER_BIT,O.NEAREST):Rt?O.copyTexSubImage3D(Fe,De,Ye,Tt,Gt+Nt,ze,rt,Oe,Ie):O.copyTexSubImage2D(Fe,De,Ye,Tt,ze,rt,Oe,Ie);g.bindFramebuffer(O.READ_FRAMEBUFFER,null),g.bindFramebuffer(O.DRAW_FRAMEBUFFER,null)}else Rt?S.isDataTexture||S.isData3DTexture?O.texSubImage3D(Fe,De,Ye,Tt,Gt,Oe,Ie,Be,_t,Jt,Bt.data):j.isCompressedArrayTexture?O.compressedTexSubImage3D(Fe,De,Ye,Tt,Gt,Oe,Ie,Be,_t,Bt.data):O.texSubImage3D(Fe,De,Ye,Tt,Gt,Oe,Ie,Be,_t,Jt,Bt):S.isDataTexture?O.texSubImage2D(O.TEXTURE_2D,De,Ye,Tt,Oe,Ie,_t,Jt,Bt.data):S.isCompressedTexture?O.compressedTexSubImage2D(O.TEXTURE_2D,De,Ye,Tt,Bt.width,Bt.height,_t,Bt.data):O.texSubImage2D(O.TEXTURE_2D,De,Ye,Tt,Oe,Ie,_t,Jt,Bt);g.pixelStorei(O.UNPACK_ROW_LENGTH,an),g.pixelStorei(O.UNPACK_IMAGE_HEIGHT,vt),g.pixelStorei(O.UNPACK_SKIP_PIXELS,Ht),g.pixelStorei(O.UNPACK_SKIP_ROWS,yn),g.pixelStorei(O.UNPACK_SKIP_IMAGES,Rn),De===0&&j.generateMipmaps&&O.generateMipmap(Fe),g.unbindTexture()},this.initRenderTarget=function(S){z.get(S).__webglFramebuffer===void 0&&Q.setupRenderTarget(S)},this.initTexture=function(S){S.isCubeTexture?Q.setTextureCube(S,0):S.isData3DTexture?Q.setTexture3D(S,0):S.isDataArrayTexture||S.isCompressedArrayTexture?Q.setTexture2DArray(S,0):Q.setTexture2D(S,0),g.unbindTexture()},this.resetState=function(){R=0,F=0,J=null,g.reset(),Ne.reset()},typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}get coordinateSystem(){return ti}get outputColorSpace(){return this._outputColorSpace}set outputColorSpace(e){this._outputColorSpace=e;const t=this.getContext();t.drawingBufferColorSpace=yt._getDrawingBufferColorSpace(e),t.unpackColorSpace=yt._getUnpackColorSpace()}}const Xs=11,u0=[0,2,1,0,3,2],f0=`
precision highp float;
in vec2 aPosition;
in vec2 aLocal;
in vec4 aColor;
in vec3 aShape;
uniform vec2 uResolution;
out vec2 vLocal;
out vec4 vColor;
out vec3 vShape;
void main() {
  gl_Position = vec4(aPosition.x/uResolution.x*2.-1., 1.-aPosition.y/uResolution.y*2., 0., 1.);
  vLocal=aLocal; vColor=aColor; vShape=aShape;
}`,h0=`
precision highp float;
uniform sampler2D uAtlas;
in vec2 vLocal;
in vec4 vColor;
in vec3 vShape;
out vec4 outColor;
float halfPlaneCoverage(float d,vec2 derivative) {
  vec2 n=abs(derivative);
  float a=max(n.x,n.y),b=min(n.x,n.y);
  if(b<.0001)return clamp(d/max(a,.0001)+.5,0.,1.);
  float z=clamp(d+(a+b)*.5,0.,a+b);
  if(z<b)return z*z/(2.*a*b);
  if(z<a)return (z-b*.5)/a;
  float tail=a+b-z;
  return 1.-tail*tail/(2.*a*b);
}
float stripCoverage(float coordinate,float halfExtent,vec2 derivative) {
  return halfPlaneCoverage(halfExtent-coordinate,derivative)-halfPlaneCoverage(-halfExtent-coordinate,derivative);
}
void main() {
  if(vShape.x<.5) {
    // Atlas bytes and vertex colors deliberately remain encoded sRGB to match
    // Canvas2D blending. Texture samples are premultiplied before interpolation.
    outColor=texture(uAtlas,vLocal)*vColor;
  } else {
    float coverage;
    if(vShape.x<1.5) {
      // Integrate a thin line across the pixel square. A linear fwidth ramp
      // dims diagonals relative to the accepted Canvas painter.
      coverage=stripCoverage(vLocal.x,vShape.y,vec2(dFdx(vLocal.x),dFdy(vLocal.x)))
        *stripCoverage(vLocal.y,vShape.z,vec2(dFdx(vLocal.y),dFdy(vLocal.y)));
    } else if(vShape.x<2.5) {
      float dist=length(vLocal)-vShape.y;
      coverage=clamp(.5-dist,0.,1.);
    } else if(vShape.x<3.5) {
      // The focus marker has mitered L corners. Draw their union once, retaining
      // the outer corner and avoiding alpha accumulation between the two legs.
      float len=vShape.y,hw=vShape.z,center=(len-hw)*.5,extent=(len+hw)*.5;
      vec2 dx=vec2(dFdx(vLocal.x),dFdy(vLocal.x));
      vec2 dy=vec2(dFdx(vLocal.y),dFdy(vLocal.y));
      float sx=stripCoverage(vLocal.x,hw,dx),sy=stripCoverage(vLocal.y,hw,dy);
      coverage=stripCoverage(vLocal.x-center,extent,dx)*sy
        +sx*stripCoverage(vLocal.y-center,extent,dy)-sx*sy;
    } else {
      vec2 dx=vec2(dFdx(vLocal.x),dFdy(vLocal.x));
      vec2 dy=vec2(dFdx(vLocal.y),dFdy(vLocal.y));
      float sx=stripCoverage(vLocal.x,vShape.z,dx),sy=stripCoverage(vLocal.y,vShape.z,dy);
      coverage=stripCoverage(vLocal.x,vShape.y,dx)*sy
        +sx*stripCoverage(vLocal.y,vShape.y,dy)-sx*sy;
    }
    outColor=vColor*coverage;
  }
}`,eo=new Map;function Xd(n){if(eo.has(n))return eo.get(n);const e=n.replace("#","");if(![3,4,6,8].includes(e.length)||!/^[a-f0-9]+$/i.test(e))throw new Error("Unsupported scene color: "+n);const t=e.length<5?[...e].map(r=>r+r).join(""):e,i=[0,2,4].map(r=>parseInt(t.slice(r,r+2),16)/255);return i.push(t.length===8?parseInt(t.slice(6,8),16)/255:1),eo.set(n,i),i}class Dc{constructor(e,t){this.a=e,this.b=t,this.stops=[]}addColorStop(e,t){this.stops.push([e,Xd(t)]),this.stops.sort((i,r)=>i[0]-r[0])}at(e,t){const i=this.b[0]-this.a[0],r=this.b[1]-this.a[1],s=Math.max(0,Math.min(1,((e-this.a[0])*i+(t-this.a[1])*r)/(i*i+r*r||1))),a=this.stops[0],o=this.stops.at(-1);return a[1].map((l,c)=>l+(o[1][c]-l)*s)}}class p0{constructor(e){this.canvas=e,this.renderer=new d0({canvas:e,alpha:!1,antialias:!1,depth:!1,stencil:!1,premultipliedAlpha:!0}),this.renderer.setClearColor(0,1),this.renderer.outputColorSpace=fs,this.renderer.toneMapping=Vn,this.renderer.sortObjects=!1,this.measure=document.createElement("canvas").getContext("2d"),this.atlas=document.createElement("canvas"),this.atlas.width=this.atlas.height=2048,this.atlasContext=this.atlas.getContext("2d"),this.atlasTexture=new sh(this.atlas),this.atlasTexture.colorSpace=_i,this.atlasTexture.premultiplyAlpha=!0,this.atlasTexture.generateMipmaps=!1,this.atlasTexture.minFilter=this.atlasTexture.magFilter=ln,this.images=new Map,this.shelfX=2,this.shelfY=2,this.shelfHeight=0,this.resolution=new At(1,1),this.materials=["source-over","screen"].map(t=>new Ud({name:"Sophia "+t,vertexShader:f0,fragmentShader:h0,glslVersion:Ko,uniforms:{uResolution:{value:this.resolution},uAtlas:{value:this.atlasTexture}},transparent:!0,depthTest:!1,depthWrite:!1,toneMapped:!1,blending:ad,blendEquation:Bi,blendSrc:ao,blendDst:t==="screen"?od:cs,blendSrcAlpha:ao,blendDstAlpha:cs})),this.scene=new Yf,this.camera=new vl,this.capacity=0,this.mesh=null,this.allocate(16384),this.matrix=[1,0,0,1,0,0],this.stack=[],this.path=[],this.quadPoints=Array.from({length:4},()=>[0,0]),this.quadLocals=Array.from({length:4},()=>[0,0]),this.quadShape=[0,0,0],this.globalAlpha=1,this.globalCompositeOperation="source-over",this.fillStyle="#000000",this.strokeStyle="#000000",this.lineWidth=1,this.lost=!1,this.disposed=!1,e.addEventListener("webglcontextlost",t=>{t.preventDefault(),this.lost=!0,e.dataset.context="lost"}),e.addEventListener("webglcontextrestored",()=>{this.lost=!1,this.atlasTexture.needsUpdate=!0,e.dataset.context="restored",e.dispatchEvent(new CustomEvent("sophia-renderer-restored"))}),addEventListener("pagehide",t=>{t.persisted||this.dispose()})}get font(){return this.measure.font}set font(e){this.measure.font=e}measureText(e){return this.measure.measureText(e)}point(e,t){const i=this.matrix;return[i[0]*e+i[2]*t+i[4],i[1]*e+i[3]*t+i[5]]}setTransform(...e){this.matrix=e}translate(e,t){const i=this.point(e,t);this.matrix[4]=i[0],this.matrix[5]=i[1]}rotate(e){const[t,i,r,s,a,o]=this.matrix,l=Math.cos(e),c=Math.sin(e);this.matrix=[t*l+r*c,i*l+s*c,r*l-t*c,s*l-i*c,a,o]}save(){this.stack.push({matrix:this.matrix.slice(),globalAlpha:this.globalAlpha,globalCompositeOperation:this.globalCompositeOperation,fillStyle:this.fillStyle,strokeStyle:this.strokeStyle,lineWidth:this.lineWidth})}restore(){const e=this.stack.pop();e&&Object.assign(this,e)}createLinearGradient(e,t,i,r){return new Dc(this.point(e,t),this.point(i,r))}allocate(e){const t=this.data;this.geometry?.dispose(),this.capacity=e,this.data=new Float32Array(e*Xs),t&&this.data.set(t),this.buffer=new Jf(this.data,Xs).setUsage(Rf),this.geometry=new ai;for(const[i,r,s]of[["aPosition",2,0],["aLocal",2,2],["aColor",4,4],["aShape",3,8]])this.geometry.setAttribute(i,new _l(this.buffer,r,s));this.mesh?this.mesh.geometry=this.geometry:(this.mesh=new ri(this.geometry,this.materials),this.mesh.frustumCulled=!1,this.scene.add(this.mesh))}beginFrame(){this.used=0,this.groups=[],(this.resolution.x!==this.canvas.width||this.resolution.y!==this.canvas.height)&&(this.resolution.set(this.canvas.width,this.canvas.height),this.renderer.setSize(this.canvas.width,this.canvas.height,!1))}endFrame(){this.lost||this.disposed||(this.geometry.clearGroups(),this.groups.forEach(e=>this.geometry.addGroup(e.start,e.count,e.material)),this.geometry.setDrawRange(0,this.used),this.buffer.clearUpdateRanges(),this.buffer.addUpdateRange(0,this.used*Xs),this.buffer.needsUpdate=!0,this.renderer.render(this.scene,this.camera),this.canvas.dataset.drawCalls=String(this.renderer.info.render.calls),this.canvas.dataset.gpuVertices=String(this.used),this.canvas.dataset.gpuTextures=String(this.renderer.info.memory.textures))}quad(e,t,i,r,s=this.globalAlpha){this.used+6>this.capacity&&this.allocate(this.capacity*2);const a=this.globalCompositeOperation==="screen"?1:0;let o=this.groups.at(-1);(!o||o.material!==a)&&(o={start:this.used,count:0,material:a},this.groups.push(o)),o.count+=6;const l=r instanceof Dc?null:Xd(r);for(const c of u0){const f=e[c],u=t[c],d=l||r.at(...f),h=d[3]*s,_=this.used++*Xs;this.data[_]=f[0],this.data[_+1]=f[1],this.data[_+2]=u[0],this.data[_+3]=u[1],this.data[_+4]=d[0]*h,this.data[_+5]=d[1]*h,this.data[_+6]=d[2]*h,this.data[_+7]=h,this.data[_+8]=i[0],this.data[_+9]=i[1],this.data[_+10]=i[2]}}atlasEntry(e){if(this.images.has(e))return this.images.get(e);const t=e.width,i=e.height;if(this.shelfX+t+2>2048&&(this.shelfX=2,this.shelfY+=this.shelfHeight+4,this.shelfHeight=0),this.shelfY+i+2>2048)throw new Error("Scene texture atlas budget exceeded");const r={x:this.shelfX,y:this.shelfY,width:t,height:i};return this.shelfX+=t+4,this.shelfHeight=Math.max(this.shelfHeight,i),this.images.set(e,r),this.invalidateImage(e),r}invalidateImage(e){const t=this.images.get(e);if(!t)return;const i=this.atlasContext,{x:r,y:s,width:a,height:o}=t;i.clearRect(r-1,s-1,a+2,o+2),i.drawImage(e,r,s),i.drawImage(e,0,0,a,1,r,s-1,a,1),i.drawImage(e,0,o-1,a,1,r,s+o,a,1),i.drawImage(e,0,0,1,o,r-1,s,1,o),i.drawImage(e,a-1,0,1,o,r+a,s,1,o);for(const[l,c,f,u]of[[0,0,r-1,s-1],[a-1,0,r+a,s-1],[0,o-1,r-1,s+o],[a-1,o-1,r+a,s+o]])i.drawImage(e,l,c,1,1,f,u,1,1);this.atlasTexture.needsUpdate=!0}drawImage(e,t,i,r,s){const a=this.atlasEntry(e),o=a.x/2048,l=1-a.y/2048,c=a.width/2048,f=a.height/2048,u=this.matrix,d=this.quadPoints,h=this.quadLocals,_=this.quadShape;for(let x=0;x<4;x++){const m=x===1||x===2,p=x>=2,C=m?t+r:t,D=p?i+s:i;d[x][0]=u[0]*C+u[2]*D+u[4],d[x][1]=u[1]*C+u[3]*D+u[5],h[x][0]=m?o+c:o,h[x][1]=p?l-f:l}_[0]=_[1]=_[2]=0,this.quad(d,h,_,"#ffffff")}rectAt(e,t,i,r,s,a,o){const l=s+1,c=a+1,f=this.quadPoints,u=this.quadLocals,d=this.quadShape;for(let h=0;h<4;h++){const _=h===1||h===2?l:-l,x=h>=2?c:-c;u[h][0]=_,u[h][1]=x,f[h][0]=e+i*_-r*x,f[h][1]=t+r*_+i*x}d[0]=1,d[1]=s,d[2]=a,this.quad(f,u,d,o)}fillRect(e,t,i,r){const s=this.matrix,a=Math.hypot(s[0],s[1]),o=Math.hypot(s[2],s[3]),l=e+i/2,c=t+r/2;this.rectAt(s[0]*l+s[2]*c+s[4],s[1]*l+s[3]*c+s[5],s[0]/a,s[1]/a,Math.abs(i*a/2),Math.abs(r*o/2),this.fillStyle)}beginPath(){this.path=[],this.current=null}moveTo(e,t){this.current=this.point(e,t)}lineTo(e,t){const i=this.point(e,t);this.current&&this.path.push({type:"line",a:this.current,b:i}),this.current=i}arc(e,t,i,r,s){if(Math.abs(s-r-Math.PI*2)>1e-4)throw new Error("Only full scene discs are supported");this.path.push({type:"disc",center:this.point(e,t),radius:i*Math.hypot(this.matrix[0],this.matrix[1])})}fill(){for(const e of this.path){if(e.type!=="disc")throw new Error("Only scene discs may be filled");const t=e.radius+1,i=[[-t,-t],[t,-t],[t,t],[-t,t]];this.quad(i.map(([r,s])=>[r+e.center[0],s+e.center[1]]),i,[2,e.radius,e.radius],this.fillStyle)}}stroke(){const e=this.lineWidth*Math.hypot(this.matrix[0],this.matrix[1])/2;if(this.path.length===2){const[t,i]=this.path;if(t.type==="line"&&i.type==="line"){const r=t.b[0]-t.a[0],s=t.b[1]-t.a[1],a=i.b[0]-i.a[0],o=i.b[1]-i.a[1],l=Math.hypot(r,s);if(l>0&&Math.abs(l-Math.hypot(a,o))<1e-5&&Math.abs(r*a+s*o)<1e-5&&Math.abs(t.a[0]+t.b[0]-i.a[0]-i.b[0])<1e-5&&Math.abs(t.a[1]+t.b[1]-i.a[1]-i.b[1])<1e-5){const c=(t.a[0]+t.b[0])/2,f=(t.a[1]+t.b[1])/2,u=r/l,d=s/l,h=l/2+1,_=[[-h,-h],[h,-h],[h,h],[-h,h]];this.quad(_.map(([x,m])=>[c+u*x-d*m,f+d*x+u*m]),_,[4,l/2,e],this.strokeStyle);return}}}for(let t=0;t<this.path.length;t++){const i=this.path[t],r=this.path[t+1];if(i.type!=="line")throw new Error("Only scene line segments may be stroked");const s=i.b[0]-i.a[0],a=i.b[1]-i.a[1],o=Math.hypot(s,a);if(r?.type==="line"&&i.b[0]===r.a[0]&&i.b[1]===r.a[1]){const l=r.b[0]-r.a[0],c=r.b[1]-r.a[1],f=Math.hypot(l,c);if(o>0&&Math.abs(o-f)<1e-5&&Math.abs(s*l+a*c)<1e-5){const u=[-s/o,-a/o],d=[l/o,c/o],h=-e-1,_=o+1;let x=[[h,h],[_,h],[_,_],[h,_]];u[0]*d[1]-u[1]*d[0]<0&&(x=[x[0],x[3],x[2],x[1]]),this.quad(x.map(([m,p])=>[i.b[0]+u[0]*m+d[0]*p,i.b[1]+u[1]*m+d[1]*p]),x,[3,o,e],this.strokeStyle),t++;continue}}o>0&&this.rectAt((i.a[0]+i.b[0])/2,(i.a[1]+i.b[1])/2,s/o,a/o,o/2,e,this.strokeStyle)}}dispose(){this.disposed||(this.disposed=!0,this.geometry.dispose(),this.materials.forEach(e=>e.dispose()),this.atlasTexture.dispose(),this.images.clear(),this.renderer.dispose())}}function m0(n,{client:e,onChange:t=()=>{},initialFocus:i,initialLens:r,autoStart:s=!0}={}){const a=T=>n.querySelector(T);n.dataset.rendererBuild="c8d4575cf448b8707cdf55ae779d2743482a02f7348ae5e66f4972983bfe18cb";let o=a(".sc-sky"),l;try{l=new p0(o),n.dataset.renderer="three-webgl2"}catch(T){const V=o.cloneNode(!0);o.replaceWith(V),o=V,l=o.getContext("2d",{alpha:!1}),n.dataset.renderer="canvas-fallback",console.warn("GPU renderer unavailable; accepted Canvas painter retained.",T)}if(!l)return;const c=matchMedia("(prefers-reduced-motion: reduce)"),f={density:.85,pixel:.5,depth:1.25,liveliness:1,playing:!c.matches},u=1250;let d=[],h=[];const _=["#89accc","#dfc28f","#a49fdb"],x=new Map,m=a(".sc-nodes");let p=7239;const C=()=>(p=p*1664525+1013904223>>>0,p/4294967296),D=(T,V,me)=>{const ge=Math.max(0,Math.min(1,(me-T)/(V-T)));return ge*ge*(3-2*ge)},M=Array.from({length:2200},(T,V)=>{const me=V%25===0?2:V%5<2?1:0,ge=(C()-.5)*2.75,$e=C()<.58&&me===0?ge*.34+(C()+C()+C()-1.5)*.54:(C()-.5)*2.7,it=[-2400,-920,220][me],Ve=[1530,1150,570][me],ht=C()*Math.PI*2;return{layer:me,sx:ge,sy:$e,minZ:it,range:Ve,offset:C(),cycle:[230,135,92][me]*(.8+C()*.5),size:me===2?1.25+C()*1.5:me===1?.65+C()*.85:.38+C()*.65,phase:ht,freq:.55+C()*1.5,tone:Math.floor(C()*6),alpha:[.3,.58,.36][me]*(.55+C()*.65),flare:C()>.95,drift:(C()-.5)*26,x:0,y:0}}),L=["#bdd5ed","#95bde0","#e3eaf5","#dfcfb5","#aeb8e7","#f0e3c9"],A=new Map;function P(T){if(A.has(T))return A.get(T);const V=document.createElement("canvas");V.width=V.height=128;const me=V.getContext("2d"),ge=me.createRadialGradient(64,64,0,64,64,64);return ge.addColorStop(0,T+"c0"),ge.addColorStop(.05,T+"80"),ge.addColorStop(.16,T+"30"),ge.addColorStop(.45,T+"0c"),ge.addColorStop(1,T+"00"),me.fillStyle=ge,me.fillRect(0,0,128,128),A.set(T,V),V}const v=document.createElement("canvas");v.width=1100,v.height=780;const b=v.getContext("2d");function w(){b.fillStyle="#04070e",b.fillRect(0,0,1100,780);const T=[[210,260,390,"#111c33"],[620,410,375,"#142c31"],[930,555,330,"#231d38"],[585,405,170,"#242523"]];for(const[V,me,ge,$e]of T){const it=b.createRadialGradient(V,me,0,V,me,ge);it.addColorStop(0,$e+"b0"),it.addColorStop(.5,$e+"65"),it.addColorStop(1,$e+"00"),b.fillStyle=it,b.fillRect(0,0,1100,780)}l.invalidateImage?.(v)}function E(T,V,me){const ge=(Vt,Yt)=>{let zt=Math.imul(Vt,374761393)+Math.imul(Yt,668265263)+Math.imul(me,1442695041);return zt=Math.imul(zt^zt>>>13,1274126177),((zt^zt>>>16)>>>0)/4294967295},$e=Math.floor(T),it=Math.floor(V),Ve=T-$e,ht=V-it,tt=Ve*Ve*(3-2*Ve),dt=ht*ht*(3-2*ht),ot=ge($e,it),lt=ge($e+1,it),Ct=ge($e,it+1),Ot=ge($e+1,it+1);return(ot+(lt-ot)*tt)*(1-dt)+(Ct+(Ot-Ct)*tt)*dt}function N(T,V){const me=document.createElement("canvas");me.width=320,me.height=240;const ge=me.getContext("2d"),$e=ge.createImageData(me.width,me.height),it=$e.data;for(let Ve=0;Ve<me.height;Ve++)for(let ht=0;ht<me.width;ht++){const tt=ht/me.width,dt=Ve/me.height,ot=E(tt*3,dt*3,V);let lt=0,Ct=.55,Ot=3;for(let Lt=0;Lt<4;Lt++)lt+=E(tt*Ot+ot*.8,dt*Ot,V+Lt)*Ct,Ct*=.5,Ot*=2;const Vt=Math.exp(-Math.pow((dt-(.43+Math.sin(tt*4.4+V)*.14))/.23,2)),Yt=Math.sin(Math.PI*tt)*Math.sin(Math.PI*dt),zt=Math.max(0,lt-.34)*Vt*Yt,et=(Ve*me.width+ht)*4;it[et]=T[0],it[et+1]=T[1],it[et+2]=T[2],it[et+3]=Math.min(130,Math.round(zt*220))}return ge.putImageData($e,0,0),me}const G=N([76,102,151],17),W=N([111,91,136],53);let I=1,R=740,F=1,J=1,B=-1,oe=-1,k="constellations",K=0,Z=-.055,Y=0,ne=-.055,H=1,te=1,ie={x:0,y:0},X={x:0,y:0},he=0,_e=0,we=0,ee=!0,ce=null,$=0,pe=0,ye=0,Te=0,Pe=0,Ge="about",Ee=!0,ve=!1,O=null,Xe=null,Ze=-1/0,y=0,g="",U=0,z=0,Q="trackpad",ue=null,be=-1/0;const ae=.55,de=22;let Se={x:0,y:0,manual:!1},ke=[];const Ae=new Map,Re=[],He=new Map,Ke=(T,V,me)=>Math.max(V,Math.min(me,T)),Je={x:0,y:0,tx:0,ty:0};let q=1,Ce=0,xe=1,Le=0,Ne=1,Me=0,qe=1,We=0;const Qe=a(".sc-inspector");let wt=null,qt=[],St=null,Ai=!1,sn=null;function qr(){return{packet:wt,vertices:d.map(T=>({id:T.id,slot:T.slot,p:T.p.slice(),sourcePosition:T.sourcePosition.slice(),volumeZ:T.volumeZ,pos:T.target.slice(),target:T.target.slice()})),selectedRelation:St}}function Vi(T,V=d,{restore:me=!1}={}){const ge=d[B]?.id,$e=new Map(d.map(Ve=>[Ve.id,Ve]));if(d=ou(T,V).map(Ve=>{const ht=$e.get(Ve.id);let tt=ht?.el,dt=ht?.label;if(!tt){tt=document.createElement("button"),tt.type="button",tt.className="sc-node",tt.dataset.id=Ve.id,dt=document.createElement("span"),dt.className="sc-node-label",tt.append(dt);const ot=()=>x.get(Ve.id);tt.addEventListener("click",lt=>{lt.detail<2&&ot()!==void 0&&Oe(ot())}),tt.addEventListener("dblclick",()=>{ot()!==void 0&&(S(),re(ot()))}),tt.addEventListener("pointerenter",()=>{oe=ot()??-1,Mt()}),tt.addEventListener("pointerleave",()=>{oe=-1,Mt()}),tt.addEventListener("focus",()=>{oe=ot()??-1,Mt()}),tt.addEventListener("blur",()=>{oe=-1,Mt()}),tt.addEventListener("keydown",lt=>{ot()!==void 0&&vt(lt,ot())})}return tt.dataset.main=String(Ve.main),tt.setAttribute("aria-label",Ve.fullName+" — "+Ve.kind),dt.textContent=Ve.name,{...Ve,el:tt,label:dt,screen:$r(...Ve.pos)}}),x.clear(),d.forEach((Ve,ht)=>x.set(Ve.id,ht)),m.replaceChildren(...d.map(Ve=>Ve.el)),wt=T,qt=T.relations,h=qt.map(Ve=>[x.get(Ve.from_id),x.get(Ve.to_id)]),k!=="constellations"){const Ve=me?new Set(V.map(ot=>ot.id)):null,ht=x.get(T.focus?.node_id)??0,tt=Kr(ht),dt=d.map((ot,lt)=>lt).filter(ot=>ot!==ht&&!tt.includes(ot));d.forEach((ot,lt)=>{if(!Ve?.has(ot.id))if(k==="plane")ot.target=[ot.sourcePosition[0],ot.sourcePosition[1],0];else if(lt===ht)ot.target=[0,0,0];else{const Ct=tt.includes(lt)?tt:dt,Ot=Ct.indexOf(lt)/Ct.length*Math.PI*2-.8,Vt=Ct===tt?175:380;ot.target=[Math.cos(Ot)*Vt,Math.sin(Ot)*Vt*.68,Math.sin(Ot*2)*140*f.depth]}})}B=x.get(ge)??-1,oe=-1,qt.some(Ve=>Ve.id===St)||(St=null),n.dataset.graphRevision=T.source_revision,n.dataset.graphFocus=T.focus?.node_id||"",n.dataset.nodeCount=String(d.length),n.dataset.relationCount=String(h.length),n.dataset.dataState="ready",oi(),se(),Nn(),Ee=!0,$=1,Mt()}function ga(T){T?.packet&&(sn?.cancelPending(),Vi(T.packet,T.vertices,{restore:!0}),St=T.selectedRelation)}function gs(T){if(!h.length)return!1;const V=n.getBoundingClientRect(),me=(T.clientX-V.left)*I/V.width,ge=(T.clientY-V.top)*R/V.height;let $e=-1,it=7;for(let Ve=0;Ve<h.length;Ve++){const[ht,tt]=h[Ve],dt=d[ht].screen,ot=d[tt].screen;if(!dt.s||!ot.s)continue;const lt=ot.x-dt.x,Ct=ot.y-dt.y,Ot=lt*lt+Ct*Ct,Vt=Ot?Ke(((me-dt.x)*lt+(ge-dt.y)*Ct)/Ot,0,1):0;if(Vt<.04||Vt>.96)continue;const Yt=Math.hypot(me-dt.x-Vt*lt,ge-dt.y-Vt*Ct);Yt<it&&(it=Yt,$e=Ve)}return $e<0?!1:(Yr(qt[$e].id),!0)}function Yr(T){const V=qt.find(me=>me.id===T);V&&(sn?.willSelect(),(St!==T||Qe.hidden)&&S(),Be(!1,!1),ze(!1,!1),St=T,B=x.get(V.from_id)??-1,Qe.hidden=!1,De(),Ye(Ae.get(T)||"about"),se(),Nn(),xn(!0),Ee=!0,Mt(),qn("Отношение: "+a(".sc-node-title").textContent),a("#so-about-tab").focus())}const An={get packet(){return wt},get selection(){return{nodeId:d[B]?.id||null,relationId:St}},node:T=>d.find(V=>V.id===T)?.raw,relation:T=>qt.find(V=>V.id===T),neighbors:T=>qt.filter(V=>V.from_id===T||V.to_id===T),captureView:()=>_s(),capturePlace:()=>so({lens:k,yaw:Y,pitch:ne,zoom:te,pan:X,selectedId:d[B]?.id||null,relationId:St,panelOpen:!Qe.hidden,cardTab:Ge,vertices:qr().vertices}),restorePlace(T){const V=so(T);_t(),k=V.lens,Vi(wt,V.vertices,{restore:!0}),B=x.get(V.selectedId)??-1,St=qt.some(me=>me.id===V.relationId)?V.relationId:null,Y=V.yaw,ne=V.pitch,te=V.zoom,X={...V.pan},Tt(),B>=0||St?(De(),Ye(V.cardTab,{preserveCamera:!0}),Qe.hidden=!V.panelOpen,xn(!1)):Qe.hidden=!0,se(),Nn(),$=1,Mt()},refreshTypography(){oi(),Ee=!0,Mt()},restoreView(T){T?.graph?.packet&&(S(),fe(T))},setGraph(T,{selectFocus:V=!1,initial:me=!1}={}){if(me||S(),sn?.cancelInspector(),Vi(T),V&&T.focus){Ai=!0;try{Oe(x.get(T.focus.node_id))}finally{Ai=!1}}else B>=0||St?De():(Qe.hidden=!0,Nn())},selectNode(T){const V=x.get(T);return V===void 0?!1:(Oe(V),!0)},selectRelation(T,{rememberView:V=!0}={}){const me=Ai;Ai=!V||me;try{Yr(T)}finally{Ai=me}},cardChanged(){xn(!1),Ee=!0,Mt()},announce:qn};function ur(){const T=I;I=n.clientWidth,R=n.clientHeight,F=Math.min(devicePixelRatio||1,1.5),o.width=Math.round(I*F),o.height=Math.round(R*F),J=I<=540?I/720:Math.min(I/1190,1.06),M.forEach(V=>{const me=V.minZ+V.range*.5,ge=(u-me)/u;V.x=V.sx*I*.5/J*ge,V.y=V.sy*R*.5/J*ge}),oi(),xn(T!==I),B>=0&&!Qe.hidden&&re(B,!1),Ee=!0,Mt()}function $r(T,V,me,ge=!1){const $e=ge?Ne:q,it=ge?Me:Ce,Ve=ge?qe:xe,ht=ge?We:Le,tt=T*$e-me*it,dt=T*it+me*$e,ot=V*Ve-dt*ht,lt=V*ht+dt*Ve;if(lt>u-110)return{x:-9999,y:-9999,s:0,depth:0,z:lt};const Ct=u/(u-lt),Ot=J*(ge?1:H)*Ct;return{x:I*.5+tt*Ot+(ge?ie.x*.18+Je.x*Ct*23:ie.x),y:R*.54+ot*Ot+(ge?ie.y*.18+Je.y*Ct*17:ie.y),s:Ot,depth:Ct,z:lt}}function qn(T){a(".sc-announcement").textContent=T}function Dn(T,V,me){T.style[V]!==String(me)&&(T.style[V]=me)}function Kr(T){return h.filter(V=>V.includes(T)).map(V=>V[0]===T?V[1]:V[0])}function oi(){d.forEach(T=>{const V=getComputedStyle(T.label);l.font=V.font;const me=parseFloat(V.letterSpacing)||0;T.labelWidth=Math.ceil(l.measureText(T.name).width+me*T.name.length)+2,T.labelHeight=Math.ceil(parseFloat(V.fontSize)*1.5)})}function Wi(T){const V=T.getBoundingClientRect(),me=n.getBoundingClientRect();return{x:V.left-me.left,y:V.top-me.top,w:V.width,h:V.height}}function Ri(T,V,me=0){return T.x<V.x+V.w+me&&T.x+T.w+me>V.x&&T.y<V.y+V.h+me&&T.y+T.h+me>V.y}function _a(){ke=[];for(const T of n.querySelectorAll(".sc-header,.sc-context,.sc-footer,.sc-label-west,.sc-label-east,.sc-panel"))if(!T.hidden){const V=Wi(T);V.w&&V.h&&ke.push(V)}Qe.hidden||Wi(Qe),Ee=!1}function xn(T=!1){if(!Qe.hidden){if(a(".sc-window-handle").disabled=I<=540,I<=540)for(const V of["left","right","top","bottom","transform"])Qe.style[V]="";else{const V=Qe.offsetWidth,me=Qe.offsetHeight;if(T&&!Se.manual&&B>=0){const ge=d[B].screen;Se.x=ge.x+V+60<I?ge.x+46:ge.x-V-46,Se.y=ge.y-100}Se.x=Ke(Se.x,16,I-V-16),Se.y=Ke(Se.y,190,Math.max(190,R-me-98)),Qe.style.left="0",Qe.style.top="0",Qe.style.right="auto",Qe.style.bottom="auto",Qe.style.transform="translate3d("+Math.round(Se.x)+"px,"+Math.round(Se.y)+"px,0)"}Ee=!0,Mt()}}function Nn(){a(".sc-back").hidden=Re.length===0,a(".sc-context-sub").textContent=St?"Выбрана связь":B>=0?"Фокус: "+d[B].name:wt?d.length+" звёзд · "+h.length+" связей":"Загружаем область…",n.dataset.history=String(Re.length),Ee=!0,t(An)}function _s(){return{graph:qr(),selected:B,panelOpen:!Qe.hidden||ve,lens:k,targets:d.map(T=>T.target.slice()),yaw:Y,pitch:ne,zoom:te,pan:{...X},windowPosition:{...Se},cardTab:Ge}}function S(){if(Ai)return;const T=_s();JSON.stringify(T)!==JSON.stringify(Re.at(-1))&&(Re.push(T),Re.length>24&&Re.shift()),Nn()}function j(){const T=Re.pop();T&&fe(T)}function fe(T){_t(),Be(!1,!1),ze(!1,!1),ga(T.graph),B=T.selected,oe=-1,k=T.lens,d.forEach((V,me)=>V.target=T.targets[me].slice()),Y=T.yaw,ne=T.pitch,te=T.zoom,X={...T.pan},Se={...T.windowPosition},Tt(),B>=0||St?(De(),Ye(T.cardTab,{preserveCamera:!0}),Qe.hidden=!T.panelOpen,xn(!1)):Qe.hidden=!0,se(),Nn(),qn(B>=0?"Возврат: "+d[B].name:"Возврат к предыдущему виду"),Re.length||a(".sc-overview").focus(),$=1,Mt()}function re(T,V=!0,{gentle:me=!1}={}){_t();const ge=d[T].target,$e=Math.cos(Y),it=Math.sin(Y),Ve=Math.cos(ne),ht=Math.sin(ne),tt=ge[0]*$e-ge[2]*it,dt=ge[0]*it+ge[2]*$e,ot=ge[1]*Ve-dt*ht,lt=ge[1]*ht+dt*Ve;let Ct=I*.5,Ot=R*.5;if(!Qe.hidden){const Yt=Wi(Qe);if(I<=540){const zt=Wi(a(".sc-context"));Ot=(zt.y+zt.h+35+Yt.y-38)*.5}else Ct=Yt.x>I*.44?Math.max(I*.3,Yt.x*.55):Math.min(I*.76,(Yt.x+Yt.w+I)*.5),Ot=Ke(Yt.y+Yt.h*.44,250,R-150)}te=me?Ke(te*1.08,.65,2.2):V?Math.max(I<=540?1.25:1.45,te):I<=540?1.12:Math.max(1.1,te),te=Math.min(2.2,te);const Vt=J*te*u/(u-lt);X.x=Ct-I*.5-tt*Vt,X.y=Ot-R*.54-ot*Vt,$=1,Mt()}function se(){n.dataset.selected=B>=0?d[B].id:"",d.forEach((T,V)=>T.el.setAttribute("aria-pressed",String(V===B)))}function De(){const T=St?An.relation(St):d[B]?.raw;T&&sn?.showCard(St?"relation":"node",T)}function Oe(T,{fly:V=!1,keepWindow:me=!1}={}){sn?.willSelect();const ge=B!==T||!!St;(ge||Qe.hidden)&&S(),St=null;const $e=!Qe.hidden;Be(!1,!1),ze(!1,!1),B=T,De(),Ye(Ae.get(d[T].id)||"about"),Qe.hidden=!1,sn?.restoreReading(),se(),Nn(),xn(!me&&(ge||!$e)),V?re(T,!0):ge||!$e?re(T,!1,{gentle:!0}):I<=540&&re(T,!1),qn(d[T].kind+": "+d[T].name),Ee=!0,Mt()}function Ie(T=!0,V=!1){sn?.captureReading(),sn?.cancelInspector(),Qe.hidden=!0,ve=!1,T&&B>=0&&!d[B].el.hidden&&d[B].el.focus(),V&&(St=null,B=-1,oe=-1,se(),Nn()),Ee=!0,Mt()}function Be(T=!0,V=!0){sn?.cancelSearch();const me=!a(".sc-search").hidden;a(".sc-search").hidden=!0,a(".sc-search-open").setAttribute("aria-expanded","false"),me&&(V&&ve&&B>=0&&(Qe.hidden=!1,xn(!1)),ve=!1),T&&a(".sc-search-open").focus(),Ee=!0,Mt()}function ze(T=!0,V=!0){const me=!a(".sc-lenses").hidden;a(".sc-lenses").hidden=!0,a(".sc-lenses-open").setAttribute("aria-expanded","false"),me&&(V&&ve&&B>=0&&(Qe.hidden=!1,xn(!1)),ve=!1),T&&a(".sc-lenses-open").focus(),Ee=!0,Mt()}function rt(T){const V=a(".sc-"+T);if(!V.hidden){T==="search"?Be():ze();return}const me=!Qe.hidden||ve;Be(!1,!1),ze(!1,!1),ve=me,Qe.hidden=!0,V.hidden=!1,a(".sc-"+T+"-open").setAttribute("aria-expanded","true"),T==="search"?(ft(),a("#sc-query").focus()):a('.sc-lens[aria-pressed="true"]').focus(),Ee=!0,Mt()}function ft(){sn?.search(a("#sc-query").value)}function Ye(T,{preserveCamera:V=!1}={}){sn?.captureReading(),Ge=T;const me=St||d[B]?.id;me&&(Ae.delete(me),Ae.set(me,T),Ae.size>64&&Ae.delete(Ae.keys().next().value)),n.querySelectorAll(".sc-card-tab").forEach(ge=>{const $e=ge.id==="so-"+T+"-tab";ge.setAttribute("aria-selected",String($e)),ge.tabIndex=$e?0:-1}),a("#so-about").hidden=T!=="about",a("#so-relations").hidden=T!=="relations",sn?.restoreReading(),xn(!1),!V&&I<=540&&B>=0&&!Qe.hidden&&re(B,!1),Ee=!0,Mt()}function Tt(){n.dataset.lens=k,a(".sc-context h2").textContent=k==="orbits"?"Орбиты мысли":k==="plane"?"Карта связей":"Созвездия мысли",a(".sc-label-west").hidden=!0,a(".sc-label-east").hidden=!0,n.querySelectorAll(".sc-lens").forEach(T=>T.setAttribute("aria-pressed",String(T.dataset.lens===k))),Ee=!0}function Gt(T){if(T===k){ze();return}_t(),S(),k=T;const V=B>=0?B:0,me=Kr(V),ge=d.map(($e,it)=>it).filter($e=>$e!==V&&!me.includes($e));d.forEach(($e,it)=>{if(T==="constellations")$e.target=$e.p.slice();else if(T==="plane")$e.target=[$e.sourcePosition[0],$e.sourcePosition[1],0];else if(it===V)$e.target=[0,0,0];else{const Ve=me.includes(it)?me:ge,ht=Ve.indexOf(it)/Ve.length*Math.PI*2-.8,tt=Ve===me?175:380;$e.target=[Math.cos(ht)*tt,Math.sin(ht)*tt*.68,Math.sin(ht*2)*140*f.depth]}}),Tt(),Y=0,ne=T==="plane"?0:-.055,X={x:0,y:0},te=1,ze(),B>=0&&!Qe.hidden&&re(B,!1),qn("Линза: "+a(".sc-context h2").textContent),$=1,Mt()}function Bt(){_t(),S(),Be(!1,!1),ze(!1,!1),Y=0,ne=k==="plane"?0:-.055,X={x:0,y:0},te=1,$=1,Ie(!1,!0),Se.manual=!1,qn("Общий вид"),Mt()}function _t(){U=0,Ze=-1/0,y=0,g="",ue=null,be=-1/0}function Jt(){const T=Q==="trackpad",V=a(".sc-input-mode");n.dataset.inputMode=Q,V.setAttribute("aria-label",T?"Управление: тачпад. Переключить на мышь":"Управление: мышь. Переключить на тачпад"),V.setAttribute("data-tooltip",T?"Тачпад · два пальца — сдвиг, щипок — полёт":"Мышь · колесо — масштаб, перетаскивание — вращение"),V.innerHTML=T?'<i data-lucide="touchpad" aria-hidden="true"></i>':'<i data-lucide="mouse" aria-hidden="true"></i>',a(".sc-gesture").textContent=T?"Два пальца — сдвиг · щипок — полёт":"Колесо — масштаб · Shift + перетаскивание — сдвиг",yi(),Ee=!0}function Fe(T,V=I*.5,me=R*.54){T=Ke(T,.65,2.2);const ge=T/te;X.x=V-I*.5-(V-I*.5-X.x)*ge,X.y=me-R*.54-(me-R*.54-X.y)*ge,te=T,$=1,Mt()}function an(){n.dataset.motion=f.playing?"running":"paused",a(".sc-motion").setAttribute("aria-pressed",String(!f.playing)),a(".sc-motion").setAttribute("aria-label",f.playing?"Приостановить движение":"Включить движение"),a(".sc-motion").innerHTML=f.playing?'<i data-lucide="pause" aria-hidden="true"></i>':'<i data-lucide="play" aria-hidden="true"></i>',yi(),Mt()}function vt(T,V){const me={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[T.key];if(!me)return;T.preventDefault();const ge=d[V].screen;let $e=-1,it=1/0;d.forEach((Ve,ht)=>{if(ht===V||Ve.el.hidden)return;const tt=Ve.screen.x-ge.x,dt=Ve.screen.y-ge.y,ot=tt*me[0]+dt*me[1];if(ot<=0)return;const lt=tt*tt+dt*dt,Ct=lt/Math.max(1,ot)+Math.abs(tt*me[1]-dt*me[0])*1.5;Ct<it&&(it=Ct,$e=ht)}),$e>=0&&d[$e].el.focus()}a(".sc-back").addEventListener("click",j),a(".sc-close").addEventListener("click",()=>Ie()),a(".sc-focus").addEventListener("click",()=>{B>=0&&(S(),re(B))}),a(".sc-search-open").addEventListener("click",()=>rt("search")),a(".sc-search-close").addEventListener("click",()=>Be()),a("#sc-query").addEventListener("input",ft),a(".sc-lenses-open").addEventListener("click",()=>rt("lenses")),a(".sc-lenses-close").addEventListener("click",()=>ze()),n.querySelectorAll(".sc-lens").forEach(T=>T.addEventListener("click",()=>Gt(T.dataset.lens))),a(".sc-overview").addEventListener("click",Bt),a(".sc-plus").addEventListener("click",()=>{_t(),S(),Fe(te*1.18)}),a(".sc-minus").addEventListener("click",()=>{_t(),S(),Fe(te/1.18)}),a(".sc-motion").addEventListener("click",()=>{f.playing=!f.playing,an()}),a(".sc-input-mode").addEventListener("click",()=>{_t(),Q=Q==="trackpad"?"mouse":"trackpad",Jt(),qn(Q==="trackpad"?"Тачпад: два пальца — перемещение, щипок — приближение":"Мышь: колесо — масштаб, перетаскивание — вращение"),Mt()}),n.querySelectorAll(".sc-card-tab").forEach((T,V)=>{T.addEventListener("click",()=>Ye(V?"relations":"about")),T.addEventListener("keydown",me=>{if(["ArrowLeft","ArrowRight","Home","End"].includes(me.key)){me.preventDefault();const ge=me.key==="Home"?"about":me.key==="End"||Ge==="about"?"relations":"about";Ye(ge),a("#so-"+ge+"-tab").focus()}})}),a(".sc-search").addEventListener("keydown",T=>{const V=[...n.querySelectorAll(".sc-result")],me=V.indexOf(T.target);if(T.target===a("#sc-query")&&T.key==="Enter"&&V[0])T.preventDefault(),V[0].click();else if(T.key==="ArrowDown"||T.key==="ArrowUp"){T.preventDefault();const ge=me+(T.key==="ArrowDown"?1:-1);ge<0?a("#sc-query").focus():V[Math.min(ge,V.length-1)]?.focus()}}),n.addEventListener("keydown",T=>{T.key==="Escape"&&(a(".sc-search").hidden?a(".sc-lenses").hidden?Qe.hidden||Ie():ze():Be(),T.stopPropagation()),T.key==="/"&&!T.target.closest("input,textarea,select,[contenteditable]")&&(T.preventDefault(),rt("search"))});const Ht=a(".sc-window-handle");Ht.addEventListener("pointerdown",T=>{I<=540||T.button!==0||(O={id:T.pointerId,x:T.clientX,y:T.clientY,origin:{...Se}},Ht.setPointerCapture(T.pointerId),T.preventDefault())}),Ht.addEventListener("pointermove",T=>{!O||T.pointerId!==O.id||(Se={x:O.origin.x+T.clientX-O.x,y:O.origin.y+T.clientY-O.y,manual:!0},xn(!1))});const yn=T=>{O?.id===T.pointerId&&(O=null,Ht.hasPointerCapture(T.pointerId)&&Ht.releasePointerCapture(T.pointerId))};Ht.addEventListener("pointerup",yn),Ht.addEventListener("pointercancel",yn),Ht.addEventListener("lostpointercapture",()=>O=null),Ht.addEventListener("keydown",T=>{if(I<=540)return;const V={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[T.key];if(!V)return;T.preventDefault();const me=T.shiftKey?40:16;Se.x+=V[0]*me,Se.y+=V[1]*me,Se.manual=!0,xn(!1)}),o.addEventListener("pointerdown",T=>{if(T.button===0){if(He.set(T.pointerId,{x:T.clientX,y:T.clientY}),o.setPointerCapture(T.pointerId),He.size===1)_t(),ce={id:T.pointerId,x:T.clientX,y:T.clientY,yaw:Y,pitch:ne,pan:{...X},shift:T.shiftKey,moved:!1};else if(He.size===2){const[V,me]=[...He.values()],ge=n.getBoundingClientRect();te=H,X={...ie},Xe={distance:Math.hypot(me.x-V.x,me.y-V.y),x:((V.x+me.x)*.5-ge.left)*I/ge.width,y:((V.y+me.y)*.5-ge.top)*R/ge.height},ce&&(ce.moved=!0)}}}),o.addEventListener("pointermove",T=>{if(He.has(T.pointerId)){if(He.set(T.pointerId,{x:T.clientX,y:T.clientY}),Xe&&He.size>=2){const[V,me]=[...He.values()],ge=n.getBoundingClientRect(),$e=Math.hypot(me.x-V.x,me.y-V.y),it=((V.x+me.x)*.5-ge.left)*I/ge.width,Ve=((V.y+me.y)*.5-ge.top)*R/ge.height;X.x+=it-Xe.x,X.y+=Ve-Xe.y,Fe(te*$e/Math.max(1,Xe.distance),it,Ve),Xe={distance:$e,x:it,y:Ve},g="pinch",U=performance.now()+180,bn()}else if(ce&&T.pointerId===ce.id){const V=T.clientX-ce.x,me=T.clientY-ce.y;if(ce.moved=ce.moved||Math.abs(V)+Math.abs(me)>4,ce.shift){const ge=n.getBoundingClientRect();X.x=ce.pan.x+V*I/ge.width*ae,X.y=ce.pan.y+me*R/ge.height*ae,g="pan",U=performance.now()+180,bn()}else Y=ce.yaw+V*.0022,ne=Ke(ce.pitch+me*.0018,-.7,.7)}$=1,Mt()}});function Rn(T){if(!He.has(T.pointerId))return;const V=ce?.moved||Xe||T.type!=="pointerup";if(He.delete(T.pointerId),Xe=null,He.size){const[me,ge]=[...He.entries()][0];ce={id:me,x:ge.x,y:ge.y,yaw:Y,pitch:ne,pan:{...X},shift:!1,moved:!0}}else ce=null,!V&&!gs(T)&&(B>=0&&S(),Be(!1,!1),ze(!1,!1),Ie(!1,!0));o.hasPointerCapture(T.pointerId)&&o.releasePointerCapture(T.pointerId)}o.addEventListener("pointerup",Rn),o.addEventListener("pointercancel",Rn),o.addEventListener("lostpointercapture",Rn);function li(T){const V=T.target instanceof Element?T.target:T.target.parentElement;return!!(V&&!V.closest('.sc-panel, .sc-navigation, button:not(.sc-node), input, textarea, select, a, [contenteditable="true"]'))}function Rt(T,V={x:I*.5,y:R*.54}){const me=n.getBoundingClientRect();return{x:Number.isFinite(T.clientX)?(T.clientX-me.left)*I/me.width:V.x,y:Number.isFinite(T.clientY)?(T.clientY-me.top)*R/me.height:V.y}}function kt(T,V){(V-Ze>220||T!==g)&&(te=H,X={...ie}),Ze=V,g=T,U=V+180}function bn(){n.dataset.wheelKind=g,n.dataset.zoomTarget=te.toFixed(6),n.dataset.panTarget=[X.x.toFixed(3),X.y.toFixed(3)].join(",")}function Nt(T){if(!li(T))return;T.cancelable&&T.preventDefault(),T.stopPropagation(),n.dataset.wheelEvents=String(++z);const V=T.deltaMode,me=T.deltaX,ge=T.deltaY,$e=performance.now();if(!Number.isFinite(me)||!Number.isFinite(ge)||ue||T.ctrlKey&&$e<be||me===0&&ge===0)return;const it=T.ctrlKey?"pinch":Q==="trackpad"?"pan":"wheel",Ve=V===1?32:V===2?R:1,ht=Math.sign(-ge);if(it==="wheel"&&g===it&&ht!==y&&(te=H,X={...ie}),kt(it,$e),y=ht,it==="pan"){const tt=n.getBoundingClientRect();X.x-=me*Ve*I/tt.width*ae,X.y-=ge*Ve*R/tt.height*ae,$=1,Mt()}else{const tt=it==="pinch"?Ke(-ge*Ve*.006,-Math.log(4),Math.log(4)):Ke(-ge*Ve*.0016,-.18,.18),dt=Rt(T);Fe(te*Math.exp(tt),dt.x,dt.y)}bn()}n.addEventListener("wheel",Nt,{passive:!1,capture:!0});function Yn(T){if(!li(T)||!Number.isFinite(T.scale)||T.scale<=0)return;T.cancelable&&T.preventDefault(),T.stopPropagation(),te=H,X={...ie};const V=Rt(T);ue={scale:T.scale,...V},kt("pinch",performance.now())}function Ci(T){if(!ue||!Number.isFinite(T.scale)||T.scale<=0)return;T.cancelable&&T.preventDefault(),T.stopPropagation();const V=Rt(T,ue);X.x+=V.x-ue.x,X.y+=V.y-ue.y,Fe(te*Math.pow(T.scale/ue.scale,.6),V.x,V.y),ue={scale:T.scale,...V},Ze=performance.now(),g="pinch",U=Ze+180,bn()}function vs(T){ue&&(T.cancelable&&T.preventDefault(),T.stopPropagation(),ue=null,be=performance.now()+80)}n.addEventListener("gesturestart",Yn,{passive:!1,capture:!0}),n.addEventListener("gesturechange",Ci,{passive:!1,capture:!0}),n.addEventListener("gestureend",vs,{passive:!1,capture:!0}),window.addEventListener("blur",()=>{ue=null,be=-1/0,He.clear(),ce=null,Xe=null,_t()}),n.addEventListener("pointermove",T=>{if(T.pointerType!=="mouse"||!f.playing)return;const V=n.getBoundingClientRect();Je.tx=Ke(((T.clientX-V.left)/I-.5)*2,-1,1),Je.ty=Ke(((T.clientY-V.top)/R-.5)*2,-1,1),Mt()}),n.addEventListener("pointerleave",()=>{Je.tx=0,Je.ty=0,Mt()});function El(T,V,me){l.save(),l.globalCompositeOperation="screen",l.globalAlpha=me,l.translate(I*.5+Math.sin(he*.025+V)*I*.026+Je.x*16-K*27,R*.51+Math.cos(he*.019+V)*R*.024+Je.y*12),l.rotate(Math.sin(he*.013+V)*.055-K*.035),l.drawImage(T,-I*.72,-R*.68,I*1.44,R*1.36),l.restore()}function va(T,V){for(let me=0;me<V;me++){const ge=M[me];if(ge.layer!==T)continue;const $e=(ge.offset+he/ge.cycle)%1,it=ge.minZ+$e*ge.range,Ve=D(0,.065,$e)*(1-D(.9,1,$e));if(Ve<.002)continue;const ht=Math.sin(he*.035+ge.phase)*ge.drift,tt=ge.x+ht,dt=ge.y+Math.cos(he*.029+ge.phase)*ge.drift*.65,ot=tt*Ne-it*Me,lt=tt*Me+it*Ne,Ct=dt*qe-lt*We,Ot=dt*We+lt*qe;if(Ot>u-130)continue;const Vt=u/(u-Ot),Yt=J*Vt,zt=I*.5+ot*Yt+ie.x*.18+Je.x*Vt*23,et=R*.54+Ct*Yt+ie.y*.18+Je.y*Vt*17;if(zt<-30||et<-30||zt>I+30||et>R+30)continue;const Lt=Math.sin(he*ge.freq+ge.phase)*.26+Math.sin(he*ge.freq*.43+ge.phase*2.13)*.16,ut=ge.flare?Math.pow(Math.max(0,Math.sin(he*.23+ge.phase*1.7)),28):0,Wt=Math.min(.92,ge.alpha*Ve*(.66+Lt+ut*.8))*(1-Pe*.16),$t=Math.min(T===2?3.3:2.2,Math.max(.38,ge.size*Vt*(.64+J*.32))),pn=L[ge.tone];if(l.fillStyle=pn,l.globalAlpha=Wt,T===2){const Qt=17+$t*8;l.drawImage(P(pn),zt-Qt/2,et-Qt/2,Qt,Qt),l.globalAlpha=Wt*.7,l.beginPath(),l.arc(zt,et,$t*.62,0,Math.PI*2),l.fill()}else if(l.fillRect(zt-$t/2,et-$t/2,$t,$t),ge.flare||$t>1.5){const Qt=12+$t*5+ut*17;l.globalAlpha=Wt*.6,l.drawImage(P(pn),zt-Qt/2,et-Qt/2,Qt,Qt)}if(ge.flare&&ut>.25&&T!==0){l.globalAlpha=Wt*ut*.52;const Qt=3+ut*7;l.lineWidth=.6,l.strokeStyle=pn,l.beginPath(),l.moveTo(zt-Qt,et),l.lineTo(zt+Qt,et),l.moveTo(zt,et-Qt),l.lineTo(zt,et+Qt),l.stroke()}me===25&&(n.dataset.starProbe=[zt.toFixed(2),et.toFixed(2),Wt.toFixed(3)].join(","))}l.globalAlpha=1}function wl(T){if(we=0,!n.isConnected||!ee||document.hidden){_e=0;return}const V=performance.now();l.beginFrame?.();const me=_e?(T-_e)/1e3:1/60,ge=Math.min(me,.05);_e=T;const $e=c.matches?1:1-Math.exp(-ge*14),it=c.matches?1:T<U?1-Math.exp(-ge*(g==="pan"?de:30)):$e,Ve=Qe.hidden?0:1;if(Pe+=(Ve-Pe)*(c.matches?1:1-Math.exp(-ge*3)),f.playing){he+=ge*f.liveliness*(1-Pe*.65);const et=1-Math.exp(-ge*4);Je.x+=(Je.tx-Je.x)*et,Je.y+=(Je.ty-Je.y)*et}K+=(Y-K)*$e,Z+=(ne-Z)*$e,H+=(te-H)*it,ie.x+=(X.x-ie.x)*it,ie.y+=(X.y-ie.y)*it;let ht=Math.abs(Y-K)+Math.abs(ne-Z)+Math.abs(te-H)+Math.abs(ie.x-X.x)*.01+Math.abs(ie.y-X.y)*.01+Math.abs(Pe-Ve);Ee&&_a(),q=Math.cos(K),Ce=Math.sin(K),xe=Math.cos(Z),Le=Math.sin(Z);const tt=K+Math.sin(he*.021)*.011,dt=Z+Math.cos(he*.017)*.008;Ne=Math.cos(tt),Me=Math.sin(tt),qe=Math.cos(dt),We=Math.sin(dt),l.setTransform(F,0,0,F,0,0),l.globalAlpha=1,l.globalCompositeOperation="source-over",l.drawImage(v,0,0,I,R);const ot=Math.round(M.length*f.density);va(0,ot),El(G,.2,.85),va(1,ot),El(W,2.4,.46),d.forEach(et=>{for(let Lt=0;Lt<3;Lt++)et.pos[Lt]+=(et.target[Lt]-et.pos[Lt])*$e,ht+=Math.abs(et.target[Lt]-et.pos[Lt])*.001;et.screen=$r(...et.pos)});const lt=oe>=0?oe:B,Ct=new Set(lt<0?[]:h.filter(et=>et.includes(lt)).flat());l.lineWidth=.7,h.forEach(([et,Lt],ut)=>{const Wt=d[et].screen,$t=d[Lt].screen;if(!Wt.s||!$t.s)return;const pn=St?qt[ut].id===St:lt>=0&&(et===lt||Lt===lt),Qt=d[et].group===d[Lt].group,Zr=pn?"#e8c88d":Qt?_[d[et].group]:"#8ea8c3",fr=pn?.76:lt>=0?.07:Qt?.39:.17,xs=Math.max(.18,Math.min(1,Wt.depth*.75)),hr=Math.max(.18,Math.min(1,$t.depth*.75)),Pi=l.createLinearGradient(Wt.x,Wt.y,$t.x,$t.y);Pi.addColorStop(0,Zr+Math.round(fr*xs*255).toString(16).padStart(2,"0")),Pi.addColorStop(1,Zr+Math.round(fr*hr*255).toString(16).padStart(2,"0")),l.strokeStyle=Pi,l.lineWidth=(pn?1:.7)*Math.max(.6,Math.min(1.35,(Wt.depth+$t.depth)/2)),l.beginPath(),l.moveTo(Wt.x,Wt.y),l.lineTo($t.x,$t.y),l.stroke()}),l.globalAlpha=1;const Ot=[],Vt=[];let Yt=0;const zt=d.map((et,Lt)=>({n:et,i:Lt})).sort((et,Lt)=>(Lt.i===lt?20:Ct.has(Lt.i)?8:Lt.n.main?3:0)-(et.i===lt?20:Ct.has(et.i)?8:et.n.main?3:0));for(const{n:et,i:Lt}of zt){const ut=et.screen,Wt=Lt===lt,$t=Ct.has(Lt),pn=et.main,Qt=Math.max(.52,Math.min(1,ut.depth*.86)),Zr=(lt<0||Wt||$t?1:.36)*Qt,fr=_[et.group],xs=(Wt?5:Lt===0?4.7:pn?3.1:1.9)*Math.max(.55,Math.min(ut.s,1.45));l.globalAlpha=Zr;const hr=(Wt?115:Lt===0?145:pn?90:42)*Math.max(.55,ut.s);l.drawImage(P(fr),ut.x-hr/2,ut.y-hr/2,hr,hr),l.fillStyle=fr,l.beginPath(),l.arc(ut.x,ut.y,xs,0,Math.PI*2),l.fill(),l.fillStyle="#fff9ea";const Pi=Math.max(1,xs*.45);if(l.fillRect(Math.round(ut.x-Pi/2),Math.round(ut.y-Pi/2),Pi,Pi),pn||Wt){const tn=(Wt?18:Lt===0?16:10)*Math.max(.7,ut.s);l.globalAlpha=Zr*(.45+f.pixel*.45),l.strokeStyle=fr,l.lineWidth=.65,l.beginPath(),l.moveTo(ut.x-tn,ut.y),l.lineTo(ut.x+tn,ut.y),l.moveTo(ut.x,ut.y-tn),l.lineTo(ut.x,ut.y+tn),l.stroke()}if(Wt){l.globalAlpha=.85,l.strokeStyle="#d8bc84",l.lineWidth=1;const tn=20,Jr=5;l.beginPath();for(const[mr,gr]of[[-1,-1],[-1,1],[1,-1],[1,1]])l.moveTo(ut.x+mr*tn,ut.y+gr*(tn-Jr)),l.lineTo(ut.x+mr*tn,ut.y+gr*tn),l.lineTo(ut.x+mr*(tn-Jr),ut.y+gr*tn);l.stroke()}l.globalAlpha=1;const ys=I<=540?44:38,Sn={x:ut.x-ys/2,y:ut.y-ys/2,w:ys,h:ys};Dn(et.el,"transform","translate3d("+Math.round(Sn.x)+"px,"+Math.round(Sn.y)+"px,0)"),Dn(et.el,"opacity",lt<0||Wt||$t?"1":".46");const $d=ut.s>0&&Sn.x>10&&Sn.x+Sn.w<I-10&&Sn.y>92&&Sn.y+Sn.h<R-90,Kd=ke.some(tn=>Ri(Sn,tn,2)),Zd=Vt.some(tn=>Ri(Sn,tn,0));et.el.hidden=!$d||Kd||Zd,et.el.hidden||Vt.push(Sn);const Jd=Wt||$t||lt<0&&(pn||H>(I<=540?1.5:1.28))||pn&&H>1.8;let pr=null;const Xi=et.labelWidth,qi=et.labelHeight;if(!et.el.hidden&&Jd){const tn={x:ut.x-Xi/2,y:ut.y+22,w:Xi,h:qi},Jr={x:ut.x-Xi/2,y:ut.y-23-qi,w:Xi,h:qi},mr={x:ut.x+28,y:ut.y-qi/2,w:Xi,h:qi},gr={x:ut.x-28-Xi,y:ut.y-qi/2,w:Xi,h:qi};pr=(et.above?[Jr,tn,mr,gr]:[tn,Jr,mr,gr]).find(ci=>ci.x>12&&ci.x+ci.w<I-12&&ci.y>90&&ci.y+ci.h<R-86&&!ke.some(Yi=>Ri(ci,Yi,7))&&!Ot.some(Yi=>Ri(ci,Yi,7))&&!d.some((Yi,Qd)=>Qd!==Lt&&Yi.screen.s>0&&Ri(ci,{x:Yi.screen.x-8,y:Yi.screen.y-8,w:16,h:16},3)))}Dn(et.label,"visibility",pr?"visible":"hidden"),pr&&(Dn(et.label,"left",Math.round(pr.x-Sn.x)+"px"),Dn(et.label,"top",Math.round(pr.y-Sn.y)+"px"),Dn(et.label,"transform","none"),Ot.push(pr),Yt++)}va(2,ot),l.globalAlpha=1,l.endFrame?.(),pe++,ye+=performance.now()-V,Te+=me*1e3,pe%90===0&&(n.dataset.drawMs=(ye/90).toFixed(2),n.dataset.frameMs=(Te/90).toFixed(2),n.dataset.stars=String(ot),n.dataset.frames=String(pe),ye=0,Te=0),n.dataset.skyClock=he.toFixed(3),n.dataset.rotation=K.toFixed(3),n.dataset.zoom=H.toFixed(3),n.dataset.labels=String(Yt),n.dataset.calm=Pe.toFixed(2),n.dataset.camera=[K.toFixed(3),Z.toFixed(3),H.toFixed(3),ie.x.toFixed(1),ie.y.toFixed(1)].join(","),$=ht>(T<U?1e-5:.005)?1:0,f.playing||$||ce?we=requestAnimationFrame(wl):_e=0}o.addEventListener("sophia-renderer-restored",()=>{$=1,Mt()});function Mt(){!we&&ee&&!document.hidden&&(we=requestAnimationFrame(wl))}return document.addEventListener("visibilitychange",()=>{document.hidden?(cancelAnimationFrame(we),we=0,_e=0):Mt()}),new IntersectionObserver(T=>{ee=T[0].isIntersecting,ee?Mt():(cancelAnimationFrame(we),we=0,_e=0)}).observe(n),new ResizeObserver(ur).observe(n),c.addEventListener("change",T=>{f.playing=!T.matches,an()}),sn=Ju(n,An,{client:e,initialFocus:i,initialLens:r}),w(),ur(),an(),Jt(),n.dataset.lens=k,Nn(),s&&sn.start(),document.fonts?.ready.then(()=>{oi(),Mt()}),yi(),{port:An,ui:sn,openSearch:()=>rt("search"),overview:Bt,closeInspector:Ie,invalidate:()=>{Ee=!0,Mt()}}}function g0(n,e){const t={id:e.parentHypothesisId,title:e.statement.slice(0,100),body:e.statement,targetId:e.targetId,fromId:e.fromId,toId:e.toId},i=Gc({persistence:!1});i.importPacket(n.exportPacket()),i.addHypothesis(t);const r={...e,createdAt:e.createdAt??new Date().toISOString(),baseWorkspaceRevision:i.summary().revision};return i.stageProposal(r),n.addHypothesis(t),n.stageProposal(r)}const nt=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Pn=(n,e)=>{const t=nt("button",n);return t.type="button",t.addEventListener("click",e),t};function qs(n){if(/^https?:\/\//i.test(n))try{const e=new URL(n),t=nt("a",e.hostname+e.pathname,"sc-source-ref");return t.href=e.href,t.target="_blank",t.rel="noreferrer noopener",t}catch{}return nt("span",n,"sc-source-ref")}function _0(n,e,{data:{queries:t},selected:i,panels:r,onChange:s}){let a=!1;try{a=jd(localStorage,"tos-research-workspace-v1")}catch{}const o=Gc({sessionId:"tos-local-research",persistence:a}),l=new cr,c=new Map;let f=null;const u=Pn("",()=>G("notes"));u.className="sc-control sc-workspace-open",u.setAttribute("aria-label","Исследование"),u.setAttribute("aria-expanded","false"),u.innerHTML='<i data-lucide="notebook-pen" aria-hidden="true"></i><span>Исследование</span>',n.querySelector(".sc-header-actions").append(u);const d=nt("section","","sc-panel sc-workspace");d.hidden=!0,d.setAttribute("aria-label","Исследовательская панель"),d.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-workspace-close" aria-label="Закрыть исследование"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Исследование</h3><div class="sc-workspace-tabs" role="tablist" aria-label="Инструменты исследования"></div><div class="sc-workspace-body" role="tabpanel" id="sc-tool-content"></div><div class="sc-tool-status" role="status"></div><div class="sc-workspace-footer"></div>',n.append(d),r.register("workspace",d,()=>{p.capture(),l.cancelAll(),u.setAttribute("aria-expanded","false")});const h=d.querySelector(".sc-workspace-body"),_=d.querySelector(".sc-tool-status"),x=d.querySelector(".sc-workspace-tabs"),m=d.querySelector(".sc-workspace-footer"),p=Br(h);let C="notes",D=null,M=null,L="node",A=u,P="",v="note",b=null;const w={notes:"Записи",sources:"Источники",analysis:"Разбор"},E=Object.entries(w).map(([ee,ce])=>{const $=Pn(ce,()=>W(ee));return $.id="sc-tool-"+ee,$.setAttribute("role","tab"),$.setAttribute("aria-controls","sc-tool-content"),x.append($),$});x.addEventListener("keydown",ee=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(ee.key))return;ee.preventDefault();const ce=E.indexOf(ee.target),$=ee.key==="Home"?0:ee.key==="End"?2:(ce+(ee.key==="ArrowLeft"?2:1))%3;W(Object.keys(w)[$]),E[$].focus()});function N(){r.close("workspace"),(A?.isConnected&&!A.closest("[hidden]")?A:u).focus()}d.querySelector(".sc-workspace-close").addEventListener("click",N),d.addEventListener("keydown",ee=>{ee.key==="Escape"&&(ee.preventDefault(),ee.stopPropagation(),N())});function G(ee="notes",ce){p.capture(),A=document.activeElement instanceof HTMLElement?document.activeElement:u,D=ce?{id:ce.raw.id,label:mt(ce.raw.display.title||ce.raw.display.label),kind:ce.kind==="relation"?"edge":"node",source_refs:ce.raw.source_refs}:i(),M=ce?.raw||(D?.kind==="edge"?e.port.relation(D.id):e.port.node(D?.id)),L=ce?.kind||(D?.kind==="edge"?"relation":"node"),r.open("workspace"),u.setAttribute("aria-expanded","true"),W(ee),E[Object.keys(w).indexOf(ee)].focus()}function W(ee){p.capture(),l.cancelAll(),h.setAttribute("aria-busy","false"),C=ee,p.enter(JSON.stringify([e.port.packet?.source_revision,D?.id,ee])),d.querySelector("h3").textContent={notes:"Исследование",sources:"Источники",analysis:"Разбор текста"}[ee],_.textContent="";for(const[ce,$]of E.entries()){const pe=Object.keys(w)[ce]===ee;$.setAttribute("aria-selected",String(pe)),$.tabIndex=pe?0:-1}h.setAttribute("aria-labelledby","sc-tool-"+ee),h.replaceChildren(),ee==="notes"?Y():ee==="sources"?H():te(),p.restore(),e.invalidate()}r.configure("workspace",{onResume:()=>{p.restore(),W(C)}}),n.addEventListener("sophia-sources",ee=>G("sources",ee.detail));function I(ee){ee?.name!=="AbortError"&&(_.textContent=ee.message||"Не удалось выполнить действие.",e.invalidate())}function R(ee){try{return ee()}catch(ce){I(ce)}}function F(...ee){const ce=nt("div","","sc-tool-actions");return ce.append(...ee),ce}function J(){h.append(nt("p",D?"Для: "+D.label:"Общие записи исследования","sc-muted"))}function B(ee){return`${ee}:${crypto.randomUUID()}`}function oe(ee,ce){return o.addNote({id:B("note"),body:ee,targetId:ce}),{added:!0,summary:o.summary()}}function k(ee,ce=D){return o.addHypothesis({id:B("hypothesis"),title:ee.slice(0,100),body:ee,targetId:ce?.id,...ce?.from_id&&ce?.to_id?{fromId:ce.from_id,toId:ce.to_id}:{}})}function K(ee,ce=D,$={}){if(!ce?.id)throw new Error("Сначала выберите звезду или отношение.");const pe=$.source_refs||ce.source_refs||[];if(!pe.length)throw new Error("Для предложения нужен хотя бы один источник.");const ye=$.kind||"interpretation",Te=$.from_id||ce.from_id,Pe=$.to_id||ce.to_id;if(["relation","source_route"].includes(ye)&&(!Te||!Pe))throw new Error("Для предложения о связи нужны оба участника.");return g0(o,{id:B("proposal"),kind:ye,parentHypothesisId:B("hypothesis"),targetId:ce.id,...Te&&Pe?{fromId:Te,toId:Pe}:{},statement:ee,sourceRefs:pe,evidenceRefs:$.evidence_refs?.length?$.evidence_refs:pe,confidencePosture:{value:$.confidence||"unknown",meaning:"maker_declared_uncertainty_not_truth_probability"},actorOrigin:$.actor_origin==="agent"?"agent":"human",basePageRevision:$.context_revision||0,dataFingerprint:e.port.packet?.source_revision||"unavailable"})}function Z(){const ee=URL.createObjectURL(new Blob([o.exportPacket()],{type:"application/json"})),ce=nt("a");ce.href=ee,ce.download="sophia-research.json",ce.click(),setTimeout(()=>URL.revokeObjectURL(ee),1e3)}function Y(){const ee=P?b:D;h.append(nt("p",ee?"Для: "+ee.label:"Общие записи исследования","sc-muted")),h.append(nt("p","Записи и гипотезы сохраняются в этом браузере. Предложения остаются черновиками до рассмотрения.","sc-muted"));const ce=nt("form"),$=nt("label","Новая запись");$.htmlFor="sc-note-text";const pe=nt("textarea");pe.id="sc-note-text",pe.maxLength=2e3,pe.placeholder="Мысль, вопрос или наблюдение…",pe.value=P,pe.addEventListener("input",()=>{P||(b=D?{...D}:null),P=pe.value});const ye=nt("label","Тип записи");ye.htmlFor="sc-note-kind";const Te=nt("select");Te.id="sc-note-kind";for(const[Xe,Ze]of[["note","Заметка"],["hypothesis","Гипотеза"],["proposal","Предложение к рассмотрению"]]){const y=nt("option",Ze);y.value=Xe,Te.append(y)}Te.value=v,Te.addEventListener("change",()=>v=Te.value);const Pe=nt("button","Сохранить запись");Pe.type="submit",ce.append($,pe,ye,Te,F(Pe)),ce.addEventListener("submit",Xe=>{Xe.preventDefault(),R(()=>{const Ze=pe.value.trim();if(!Ze)throw new Error("Напишите текст записи.");const y=P?b:D;Te.value==="hypothesis"?k(Ze,y):Te.value==="proposal"?K(Ze,y):oe(Ze,y?.id),P="",b=null,W("notes"),_.textContent="Запись сохранена."})}),h.append(ce);const Ge=Pn("Отменить",()=>{o.undo(),W("notes")}),Ee=Pn("Повторить",()=>{o.redo(),W("notes")});Ge.disabled=!o.canUndo(),Ee.disabled=!o.canRedo();const ve=nt("input");ve.type="file",ve.accept=".json,application/json",ve.hidden=!0,ve.setAttribute("aria-label","Импорт исследования"),ve.addEventListener("change",async()=>{const Xe=ve.files?.[0];if(Xe){if(Xe.size>1e6){I(new Error("Файл превышает 1 МБ."));return}try{const Ze=await Xe.text();o.importPacket(Ze),W("notes"),_.textContent="Исследование импортировано."}catch(Ze){I(Ze)}}}),h.append(F(Ge,Ee,Pn("Экспорт",Z),Pn("Импорт",()=>ve.click())),ve);const O=o.getState();for(const[Xe,Ze]of[["Заметка",O.notes],["Гипотеза",O.hypotheses],["Предложение · ожидает рассмотрения",O.proposals]])for(const y of Ze.slice().reverse()){const g=nt("article","","sc-entry");g.append(nt("small",Xe),nt("p",y.body||y.statement)),y.targetId&&g.append(nt("small",y.targetId)),Xe==="Заметка"&&g.append(F(Pn("Удалить",()=>{o.removeNote(y.id),W("notes")}))),h.append(g)}!O.notes.length&&!O.hypotheses.length&&!O.proposals.length&&h.append(nt("p","Здесь появятся ваши записи.","sc-muted")),o.persistenceError()&&(_.textContent="Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием.")}function ne(ee){const ce=nt("article","","sc-entry");ce.append(nt("h4",ee.label||ee.preferred_label||ee.node_id));const $=ee.properties||{};($.description||$.notes)&&ce.append(nt("p",$.description||$.notes,"sc-source-text"));for(const pe of[...new Set([...ee.source_refs||[],$.url,$.locator,$.source_url].filter(ye=>typeof ye=="string"))])ce.append(qs(pe));return ce}async function H(){if(J(),!M){h.append(nt("p","Выберите звезду или отношение, чтобы увидеть источники."));return}const ee=M;h.append(nt("p",mt(L==="relation"?ee.display.explanation:ee.display.summary,"Описание пока не зафиксировано."),"sc-source-text"));const ce=nt("details");ce.append(nt("summary","Происхождение и статус"));for(const[ye,Te]of[["Слой",ee.epistemic?.authority_layer],["Рассмотрение",ee.epistemic?.review_posture],["Канон",ee.epistemic?.canon_status]])ce.append(nt("p",ye+": "+(Te&&Te!=="not-recorded"?Te:"не указан"),"sc-muted"));for(const ye of ee.source_refs)ce.append(qs(ye));if(h.append(ce),L==="relation")return;const $=ee.native_id;if(!$)return;const pe=nt("div");h.append(pe),pe.append(nt("p","Получаю досье источников…","sc-muted")),h.setAttribute("aria-busy","true");try{const ye=await l.run("source",Ge=>t.invoke("tos.dossier.inspect",{object_id:$,limit:40},{signal:Ge}));if(!ye.current||d.hidden||C!=="sources")return;pe.replaceChildren();const Te=ye.value;for(const[Ge,Ee]of[["work","Произведение"],["expression","Редакции и переводы"],["edition","Издания"],["file","Файлы"],["item","Экземпляры"],["link","Ссылки"]]){const ve=Te.chain?.[Ge]||[];if(!ve.length)continue;const O=nt("details");O.append(nt("summary",Ee+" · "+ve.length));for(const Xe of ve)O.append(ne(Xe));pe.append(O)}const Pe=Te.agent_summary;Pe&&pe.append(nt("p","Доступность ссылки и право использования — отдельные сведения. Статус прав: "+(Pe.rights_posture&&Pe.rights_posture!=="unknown"?Pe.rights_posture:"не указан")+".","sc-muted")),pe.children.length||pe.append(nt("p","Дополнительные маршруты источников пока не записаны.","sc-muted")),h.setAttribute("aria-busy","false"),p.restore(),e.invalidate()}catch(ye){h.setAttribute("aria-busy","false"),pe.replaceChildren(nt("p",ye.message,"sc-muted")),pe.append(F(Pn("Повторить",()=>W("sources")))),e.invalidate()}}function te(){h.append(nt("p","Ищите недостающие источники или подготовьте разбор слова в «Заратустре».","sc-muted"));const ee=nt("label","Запрос");ee.htmlFor="sc-analysis-query";const ce=nt("input");ce.id="sc-analysis-query",ce.placeholder="Название источника или слово…",ce.maxLength=256;const $=nt("div");h.append(ee,ce,F(Pn("Пробелы в источниках",()=>ie("gaps",ce.value,$).catch(()=>{})),Pn("Разобрать слово",()=>ie("word",ce.value,$).catch(()=>{}))),$)}async function ie(ee,ce,$,pe,ye={}){_.textContent="Получаю материал…",$.replaceChildren();try{const Te=ee==="gaps"?"tos.source-gaps.search":"tos.zarathustra.word-analysis.prepare",Pe=ee==="gaps"?{query:ce,limit:ye.limit??12}:{query:ce,language:ye.language||"ru",rank:ye.rank??1,include_semantic_neighbors:ye.include_semantic_neighbors===!0},Ge=await l.run("analysis",ve=>t.invoke(Te,Pe,{signal:pe?AbortSignal.any([ve,pe]):ve}));if(!Ge.current||d.hidden||C!=="analysis")throw pe?.throwIfAborted(),new DOMException("Panel closed","AbortError");pe?.throwIfAborted();const Ee=Ge.value;if(_.textContent="",ee==="gaps"){c.clear();for(const ve of Ee.gaps||[]){c.set(ve.edge_id,ve);const O=nt("article","","sc-entry");O.append(nt("h4",ve.to_label||ve.label),nt("p",ve.properties?.public_summary_en||ve.summary||""),nt("small","Доступ: "+ve.access_status+" · Запрос: "+ve.request_status));for(const Xe of ve.source_refs||[])O.append(qs(Xe));O.append(F(Pn("Рассмотреть",()=>he(ve.edge_id)))),$.append(O)}Ee.gaps?.length||$.append(nt("p","По этому запросу пробелов не найдено."))}else if(Ee.available!==!0)$.append(nt("p","Разбор для этого запроса сейчас недоступен."),nt("p",Ee.reason||"","sc-muted"));else{const ve=Ee.task?.source||{};$.append(nt("h4",ve.surface||ve.text||ce),nt("p",ve.context||ve.excerpt||ve.sentence||"")),ve.source_ref&&$.append(qs(ve.source_ref)),$.append(nt("p","Подготовлен разбор по исходному тексту. Результат требует рассмотрения.","sc-muted")),$.append(F(Pn("Сохранить задание",()=>{const O=URL.createObjectURL(new Blob([JSON.stringify(Ee,null,2)],{type:"application/json"})),Xe=nt("a");Xe.href=O,Xe.download="sophia-word-analysis.json",Xe.click(),setTimeout(()=>URL.revokeObjectURL(O),1e3)})))}return e.invalidate(),Ee}catch(Te){throw I(Te),Te}}function X(){}function he(ee){const ce=c.get(ee);return ce?(f={id:ce.edge_id,kind:"edge",semantic_kind:"source_access_gap",label:ce.to_label,from_id:ce.from_id,to_id:ce.to_id,predicate_id:ce.predicate_id,source_refs:ce.source_refs,authority_posture:ce.authority_posture,review_posture:ce.review_posture,canon_status:ce.canon_status,reroutable:!1},D=f,r.open("workspace"),u.setAttribute("aria-expanded","true"),W("notes"),s(),!0):!1}const _e={"tos.page.research-workspace":()=>({packet:JSON.parse(o.exportPacket()),summary:o.summary()}),"tos.page.add-research-note":ee=>oe(String(ee.text||""),ee.target_id?String(ee.target_id):void 0),"tos.page.add-session-hypothesis":ee=>({hypothesis:k(String(ee.statement||""),i()),summary:o.summary()}),"tos.page.stage-proposal":ee=>({proposal:K(String(ee.statement||""),i(),ee),summary:o.summary()}),"tos.page.workspace-undo":()=>({changed:o.undo(),summary:o.summary()}),"tos.page.workspace-redo":()=>({changed:o.redo(),summary:o.summary()}),"tos.page.workspace-export":()=>({packet:JSON.parse(o.exportPacket())}),"tos.page.workspace-import":ee=>({imported:o.importPacket(typeof ee.packet=="string"?ee.packet:JSON.stringify(ee.packet)),summary:o.summary()}),"tos.page.find-source-gaps":async(ee,{signal:ce})=>{G("analysis");const $=nt("div");h.append($);const pe=await ie("gaps",String(ee.query||""),$,ce,ee);return{...pe,gaps:pe.gaps.map(ye=>({...ye,id:ye.edge_id,label:ye.to_label,summary:ye.properties?.public_summary_en}))}},"tos.page.prepare-word-analysis":async(ee,{signal:ce})=>{G("analysis");const $=nt("div");return h.append($),ie("word",String(ee.query||""),$,ce,ee)}};let we=!1;return o.subscribe(()=>{s(),!we&&(we=!0,queueMicrotask(()=>{if(we=!1,d.hidden||C!=="notes")return;const ee=document.activeElement,ce=h.querySelector("textarea"),$=ee===ce,pe=ce?.selectionStart,ye=ce?.selectionEnd;if(h.replaceChildren(),Y(),$){const Te=h.querySelector("textarea");Te.focus(),Te.setSelectionRange(pe,ye)}e.invalidate()}))}),yi(),window.addEventListener("pagehide",()=>l.cancelAll(),{once:!0}),{workspace:o,handlers:_e,selectionChanged:X,chooseGap:he,get auxiliarySelection(){return f},clearAuxiliarySelection(){f=null},agentStatus(ee){m.textContent=ee.registered?"Агент подключён к текущему пространству":ee.supported?"Подключаю агента…":"Записи хранятся локально"}}}function v0(n,e,{onUserAction:t=()=>{}}={}){const i=new Map,r=new Map,s=[];let a=structuredClone(sl),o=null,l="",c=!1;try{o=localStorage,a=qu(o)}catch(b){l=b.message}const f=()=>JSON.stringify([e.port.packet?.source_revision,e.port.packet?.fingerprint,e.port.selection]),u=b=>b==="inspector"?"К карточке":{evidence:"К основаниям",workspace:"К источникам",navigation:"К маршруту",builder:"К линзе",studio:"К рабочему месту",reader:"К чтению"}[b]||"Назад";function d(){for(const[b,w]of i)if(!w.element.hidden)return b;return null}function h(){try{o?.setItem(rd,JSON.stringify(a)),l=o?"":"Настройки действуют до закрытия страницы: хранилище недоступно."}catch{l="Браузер не сохранил настройки. Они действуют до закрытия страницы."}}function _(){if(a.dock!=="auto")return a.dock;const b=n.getBoundingClientRect(),w=b.left+b.width/2,E=n.querySelector(".sc-inspector");if(!E.hidden&&b.width>650){const G=E.getBoundingClientRect();return G.left+G.width/2<w?"left":"right"}for(const{element:G}of i.values())if(!G.hidden&&G.dataset.dock)return G.dataset.dock;const N=[...n.querySelectorAll(".sc-node")].find(G=>G.dataset.id===n.dataset.selected&&!G.hidden);if(N){const G=N.getBoundingClientRect();if(G.width)return G.left+G.width/2>w?"left":"right"}return"right"}function x(b){const w=i.get(b);!w||w.element.hidden||(w.onHide(),w.element.hidden=!0,e.invalidate())}const m=new ResizeObserver(()=>{e.port.cardChanged(),e.invalidate()});function p(b){const w=i.get(b),E=a.sizes[b];if(w)for(const N of["width","height"])E?w.element.style.setProperty("--sc-panel-"+N,E[N]+"px"):w.element.style.removeProperty("--sc-panel-"+N)}function C(b,w,E){a.sizes[b]={width:Math.max(280,Math.min(760,w)),height:Math.max(240,Math.min(800,E))},p(b),e.invalidate()}function D(){t();const b=f();let w;for(;s.length;){const N=s.pop();if(N.key===b){w=N;break}}if(!w){M();return}c=!0,L(w.id,!1),c=!1,i.get(w.id).onResume?.();const E=w.focus;E?.isConnected&&!E.closest("[hidden]")?E.focus():i.get(w.id).element.querySelector("button")?.focus()}function M(){for(const b of i.values()){const w=[...s].reverse().find(E=>E.key===f());b.back.hidden=!w,b.back.textContent=w?"← "+u(w.id):""}}function L(b,w=!0){if(!i.has(b))throw new Error("Unknown panel: "+b);const E=d(),N=_();w&&!c&&E&&E!==b&&(s.push({id:E,key:f(),focus:document.activeElement}),s.length>8&&s.shift());for(const W of i.keys())W!==b&&x(W);b!=="inspector"&&(e.closeInspector(!1),n.querySelector(".sc-search-close").click(),n.querySelector(".sc-lenses-close").click());const G=i.get(b).element;if(b!=="inspector"){const W=n.querySelector(".sc-context").getBoundingClientRect();G.style.setProperty("--sc-tool-top",`${W.bottom-n.getBoundingClientRect().top+18}px`),G.dataset.dock=N}G.hidden=!1,p(b),M(),e.invalidate()}function A(b,w,E=()=>{},N={}){if(i.has(b))throw new Error("Duplicate panel: "+b);w.dataset.panelId=b;const G=document.createElement("div");G.className="sc-reading-return";const W=document.createElement("button");W.type="button",W.hidden=!0,W.addEventListener("click",D),G.append(W),w.querySelector(".sc-panel-top").after(G);const I=document.createElement("button");I.type="button",I.className="sc-panel-size",I.textContent="↔",I.setAttribute("aria-label","Изменить размер окна"),I.title="Размер окна: обычный / просторный",I.addEventListener("click",()=>{t();const B=a.sizes[b];C(b,B?.width>=560?420:640,B?.width>=560?440:620),h()}),w.querySelector(".sc-panel-top").insertBefore(I,w.querySelector(".sc-panel-top").lastElementChild);const R=document.createElement("button");R.type="button",R.className="sc-panel-resize",R.textContent="⌟",R.setAttribute("aria-label","Размер окна: стрелки меняют ширину и высоту"),R.title="Потяните угол; стрелки меняют размер, Home сбрасывает";let F=null;R.addEventListener("pointerdown",B=>{if(B.button!==0)return;t();const oe=w.getBoundingClientRect();F={id:B.pointerId,x:B.clientX,y:B.clientY,w:oe.width,h:oe.height},R.setPointerCapture(B.pointerId),B.preventDefault()}),R.addEventListener("pointermove",B=>{F?.id===B.pointerId&&C(b,F.w+(B.clientX-F.x)*(w.dataset.dock==="right"?-1:1),F.h+B.clientY-F.y)});const J=()=>{F&&(F=null,h())};R.addEventListener("pointerup",J),R.addEventListener("pointercancel",J),R.addEventListener("lostpointercapture",J),R.addEventListener("keydown",B=>{const oe={ArrowLeft:[-20,0],ArrowRight:[20,0],ArrowUp:[0,-20],ArrowDown:[0,20]}[B.key];if(B.key==="Home")t(),B.preventDefault(),delete a.sizes[b],p(b),h();else if(oe){t(),B.preventDefault();const k=w.getBoundingClientRect();C(b,k.width+oe[0],k.height+oe[1]),h()}}),w.append(R),i.set(b,{element:w,onHide:E,...N,back:W}),p(b),m.observe(w)}A("inspector",n.querySelector(".sc-inspector"),()=>{e.ui.captureReading(),e.ui.cancelInspector()},{onResume:()=>e.ui.restoreReading()});const P=new MutationObserver(()=>{if([".sc-inspector",".sc-search",".sc-lenses"].some(b=>!n.querySelector(b).hidden))for(const[b,w]of i)b!=="inspector"&&!w.element.hidden&&x(b);M()});for(const b of[".sc-inspector",".sc-search",".sc-lenses"])P.observe(n.querySelector(b),{attributes:!0,attributeFilter:["hidden"]});P.observe(n,{attributes:!0,attributeFilter:["data-graph-revision","data-selected"]});function v(){n.dataset.readingSize=a.text,n.dataset.labelSize=a.labels;for(const[w,E]of r)E.opener.hidden=!a.pinned.includes(w),a.pinned.includes(w)&&n.querySelector(".sc-header-actions").append(E.opener);for(const w of a.pinned){const E=r.get(w);E&&n.querySelector(".sc-header-actions").append(E.opener)}const b=n.querySelector(".sc-studio-open");b&&n.querySelector(".sc-header-actions").append(b);for(const[w,E]of i)p(w),w!=="inspector"&&!E.element.hidden&&a.dock!=="auto"&&(E.element.dataset.dock=a.dock);e.port.refreshTypography?.(),e.invalidate()}return v(),{register:A,open:L,close:x,back:D,configure(b,w){Object.assign(i.get(b),w)},get preferences(){return structuredClone(a)},get storageError(){return l},setPreferences(b){a=sd(b),h(),v()},addTool(b,w){if(r.has(b))throw new Error("Duplicate tool: "+b);r.set(b,w),v()},toolList:()=>[...r].map(([b,w])=>({id:b,title:w.title,available:w.available?.()!==!1})),launch(b){const w=r.get(b);if(!w||w.available?.()===!1)throw new Error("Сначала выберите звезду или связь.");w.launch()}}}const Hi=n=>[...new Set([n?.source_ref,...n?.source_refs||[]].filter(e=>typeof e=="string"&&e))];function qd(n){return n?.native_id?n.source_graph==="philosophy"?{mode:"philosophy",item_id:n.native_id}:n.source_graph==="canon"?{mode:"corpus",item_id:n.native_id,view_id:"route-graph"}:null:null}function x0(n,e,t,i){const r=n?.selection,s=n?.authority_boundary;if(n?.schema!=="tos_evidence_lens_packet_v1"||n.mode!==i.mode||n.item_id!==e.native_id||r?.[t==="relation"?"edge_id":"node_id"]!==e.native_id||!["is_source","is_canon","is_semantic_truth","is_rights_clearance"].every(a=>s?.[a]===!1)||!Hi(r).some(a=>Hi(e).includes(a))||!["challenge_relations","context_relations","neighbor_nodes","source_refs","routes","source_anchors","gaps"].every(a=>Array.isArray(n[a]))||typeof n.conclusion?.can_conclude!="boolean")throw new Ft("Основания не удалось связать с выбранным объектом. Обновите область.");return n}async function y0(n,e,t,{client:i,queries:r,signal:s,limit:a=60}){const{match:o}=await i.inspect(e,n.id,s,t);if(s?.throwIfAborted(),o.content_revision!==n.content_revision)throw new Ei;const l=qd(o);if(!l)return{raw:o,availability:"not_connected",packet:null};let c;try{c=await r.invoke("tos.epistemic.inspect",{...l,limit:a},{signal:s})}catch(u){if(u.status!==404)throw u;return s?.throwIfAborted(),{raw:o,availability:"outside_route",packet:null}}s?.throwIfAborted(),x0(c,o,e,l);const f=await i.inspect(e,n.id,s,t);if(s?.throwIfAborted(),f.match.content_revision!==o.content_revision)throw new Ei;return{raw:o,availability:"available",packet:c,binding:{knowledge_id:n.id,native_id:o.native_id,source_graph:o.source_graph,source_revision:t,content_revision:o.content_revision}}}function b0(n,e=[]){const t=n.properties||{},i=n.edge_id||n.node_id,r=s=>{const a=e.find(o=>o.node_id===s);return a?.label_ru||a?.preferred_label||a?.label||s};return{id:i,label:n.label_ru||n.label||t.relation_label||n.predicate_id||i,statement:n.summary_ru||n.summary||t.comment||t.description||"",route:n.from_id&&n.to_id?r(n.from_id)+" → "+r(n.to_id):"",predicate_id:n.predicate_id,from_id:n.from_id,to_id:n.to_id,source_refs:Hi(n),authority_posture:t.authority_posture,canon_status:t.canon_status,review_posture:t.review_posture,confidence:t.confidence||t.master_confidence}}function Nc(n,e){const t=n.packet;if(!t)throw new Error("Для этого объекта сравнение прочтений пока не подключено.");const i=a=>a.filter(o=>(o.edge_id||o.node_id)!==t.item_id).map(o=>b0(o,[t.selection,...t.neighbor_nodes])),r=i(t.challenge_relations),s=i(t.context_relations);return{schema:"tos_interpretation_comparison_v1",selection:e,binding:n.binding,posture:t.posture,can_conclude:t.conclusion.can_conclude===!0,competing_reading_count:r.length,competing_readings:r,contextual_readings:s,coverage:t.coverage,gaps:t.gaps_ru||t.gaps,authority_note:t.authority_note}}function S0(n,e){return{id:n.id,kind:e==="relation"?"edge":"node",label:mt(n.display.title||n.display.label,n.id),source_refs:Hi(n)}}const at=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Ys=(n,e,t="")=>{const i=at("button",n,t);return i.type="button",i.addEventListener("click",e),i},M0={"pre-canon":"До канона",canon:"Канон","derived-export":"Проекция источников",prepared_research_candidate:"Исследовательский кандидат",prepared_branch_candidate:"Кандидат ветви",contested_review_required:"Требует рассмотрения",pending_human_review:"Ожидает рассмотрения","not-recorded":"Не указан",unresolved:"Не разрешено",review_status_unresolved:"Статус рассмотрения не установлен",contested_by:"Оспаривается",uncertain_relation:"Неопределённая связь",polemicizes_with:"Полемизирует с"},Qi=n=>M0[n]||n||"Не указан";function E0(n,e,t,{data:{client:i,queries:r},selected:s,onUserAction:a}){const o=new cr,l=at("section","","sc-panel sc-evidence");l.hidden=!0,l.setAttribute("aria-label","Основания и прочтения"),l.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ЛИСТ ИССЛЕДОВАНИЯ</span><button type="button" class="sc-icon sc-evidence-close" aria-label="Закрыть основания"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-evidence-heading"><span class="sc-evidence-symbol" aria-hidden="true">✧</span><div><p class="sc-evidence-kind"></p><h3></h3></div></div><div class="sc-evidence-tabs" role="tablist" aria-label="Основания и сравнение"></div><div class="sc-evidence-body" id="sc-evidence-content" role="tabpanel" tabindex="0"></div><div class="sc-evidence-status" role="status"></div><div class="sc-evidence-footer"><span>От мысли — к источнику</span></div>',n.append(l);const c=l.querySelector(".sc-evidence-body"),f=l.querySelector(".sc-evidence-status"),u=l.querySelector(".sc-evidence-tabs"),d=Br(c),h=new Map;let _=0,x=null,m=null,p=null,C="grounds",D=null,M="",L=0;t.register("evidence",l,()=>{d.capture(),o.cancelAll(),c.setAttribute("aria-busy","false")});function A(){t.close("evidence"),a();const Z=[...n.querySelectorAll(".sc-node")].find(Y=>Y.dataset.id===x?.raw.id&&!Y.hidden);(D?.isConnected&&!D.closest("[hidden]")?D:Z||n.querySelector(".sc-overview")).focus()}l.querySelector(".sc-evidence-close").addEventListener("click",A),l.addEventListener("keydown",Z=>{Z.key==="Escape"&&(Z.preventDefault(),Z.stopPropagation(),A())});const P=["grounds","compare"],v=["Основания","Сравнение"].map((Z,Y)=>{const ne=Ys(Z,()=>{a(),b(P[Y])});return ne.id="sc-evidence-"+P[Y],ne.setAttribute("role","tab"),ne.setAttribute("aria-controls",c.id),u.append(ne),ne});u.addEventListener("keydown",Z=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(Z.key))return;Z.preventDefault();const Y=Z.key==="Home"?0:Z.key==="End"?1:1-v.indexOf(Z.target);a(),b(P[Y]),v[Y].focus()});function b(Z){d.capture(),C=Z,h.set(M,Z),h.size>48&&h.delete(h.keys().next().value),l.dataset.tab=Z,v.forEach((Y,ne)=>{Y.setAttribute("aria-selected",String(P[ne]===Z)),Y.tabIndex=P[ne]===Z?0:-1}),c.setAttribute("aria-labelledby","sc-evidence-"+Z),B()}function w(Z,Y,ne=""){const H=at("section","","sc-evidence-section "+ne);H.append(at("h4",Z));for(const te of Y)H.append(typeof te=="string"?at("p",te):te);return H}function E(Z,Y="Источники"){const ne=at("details","","sc-evidence-refs");ne.append(at("summary",Y+" · "+Z.length));for(const H of Z){const te=at("div","","sc-evidence-ref");if(/^https?:\/\//i.test(H))try{const ie=new URL(H),X=at("a",ie.hostname+ie.pathname);X.href=ie.href,X.target="_blank",X.rel="noopener noreferrer",te.append(X)}catch{te.append(at("span",H))}else te.append(at("span",H));te.append(Ys("Копировать",async()=>{try{await navigator.clipboard.writeText(H),f.textContent="Ссылка на источник скопирована."}catch{f.textContent="Выделите и скопируйте путь к источнику."}},"sc-evidence-copy")),ne.append(te)}return ne}function N(Z){const Y=at("dl","","sc-evidence-posture");for(const[ne,H]of[["Слой",Z.epistemic?.authority_layer],["Рассмотрение",Z.epistemic?.review_posture],["Канон",Z.epistemic?.canon_status]]){const te=at("div");te.append(at("dt",ne),at("dd",Qi(H))),Y.append(te)}return Y}function G(Z,Y,ne=""){if(!Y?.length)return null;const H=at("ul");for(const te of Y)H.append(at("li",te));return w(Z,[H],ne)}function W(...Z){c.append(...Z.filter(Boolean))}function I(){const Z=m?.raw||x.raw;W(N(Z),E(Hi(Z),"Происхождение объекта"));const Y=Ys("Открыть досье источников",()=>{a(),n.dispatchEvent(new CustomEvent("sophia-sources",{detail:{raw:Z,kind:x.kind}}))},"sc-evidence-source");c.append(Y)}function R(){const Z=m.raw,Y=m.packet;if(!Y){W(w("Происхождение",[mt(x.kind==="relation"?Z.display.explanation:Z.display.summary,"Описание пока не записано.")],"sc-evidence-finding")),W(w("Маршрут оснований",[m.availability==="outside_route"?"Объект не найден в доступной области Evidence Lens. Это не означает, что у него нет оснований.":"Для этого слоя отдельный маршрут оснований ещё не подключён. Здесь показаны сведения из карточки и её источники."])),I();return}W(w("Что установлено",[Y.finding_ru||Y.finding],"sc-evidence-finding"));const ne=at("div","","sc-evidence-conclusions");for(const H of[G("Можно утверждать",Y.conclusion.allowed_ru||Y.conclusion.allowed,"sc-evidence-allowed"),G("Вывод пока не следует",Y.conclusion.not_allowed_ru||Y.conclusion.not_allowed,"sc-evidence-limits")])H&&ne.append(H);if(W(ne),Y.conclusion.can_conclude!==!0&&W(at("p","Материала недостаточно для окончательного вывода в указанной области.","sc-evidence-note")),W(G("Открытые вопросы",Y.gaps_ru||Y.gaps)),Y.source_anchors.length){const H=at("details","","sc-evidence-refs");H.append(at("summary","Точные фрагменты · "+Y.source_anchors.length));for(const te of Y.source_anchors){const ie=at("div","","sc-evidence-ref");ie.append(at("p",(te.anchor_segment_ids||[]).join(" · ")),at("small",te.witness_scope||"")),te.relation_ref&&ie.append(E([te.relation_ref],"Запись связи")),H.append(ie)}W(H)}if(Y.routes.length){const H=at("details","","sc-evidence-refs");H.append(at("summary","Маршруты к основаниям · "+Y.routes.length));for(const te of Y.routes){const ie=at("div","","sc-evidence-ref");ie.append(at("small",Qi(te.route_kind)+" · "+Qi(te.status))),te.ref&&ie.append(E([te.ref],"Открыть путь")),H.append(ie)}W(H)}W(E(Y.source_refs,"Источники поля оснований")),I()}function F(Z,Y,ne=!1){const H=at("article","","sc-reading"+(ne?" sc-reading-selected":""));return H.append(at("span",Y,"sc-reading-caption"),at("h4",Qi(Z.label))),Z.route&&H.append(at("p",Z.route,"sc-reading-route")),H.append(at("p",Z.statement||"Описание этого прочтения пока не записано.","sc-reading-text")),(Z.review_posture||Z.canon_status)&&H.append(at("p",[Z.review_posture,Z.canon_status].filter(Boolean).map(Qi).join(" · "),"sc-evidence-note")),H.append(E(Z.source_refs)),H}function J(){if(!m.packet){W(w("Сопоставление ещё не подключено",[m.availability==="outside_route"?"Объект находится за пределами доступного маршрута оснований.":"В этом слое пока нет подключённого поля прочтений. Соседство на карте само по себе не означает разногласия."])),I();return}const Z=Nc(m,S0(m.raw,x.kind)),Y=m.packet,ne=m.raw;W(at("p","Сопоставление показывает записанные связи и вопросы к ним. Их истинность определяется рассмотрением источников.","sc-evidence-note"));const H=Z.competing_readings,te={label:mt(ne.display.title||ne.display.label),statement:mt(x.kind==="relation"?ne.display.explanation:ne.display.summary),route:mt(ne.display.statement),source_refs:Hi(ne),review_posture:ne.epistemic?.review_posture,canon_status:ne.epistemic?.canon_status},ie=at("div","","sc-reading-spread");if(ie.append(F(te,"ВЫБРАНО",!0)),H.length){const he=at("div","","sc-reading-alternative"),_e=at("label","Сопоставить с");_e.htmlFor="sc-reading-choice";const we=at("select");we.id=_e.htmlFor;for(const[$,pe]of H.entries()){const ye=at("option",`${$+1}. ${Qi(pe.label)}${pe.route?" · "+pe.route:""}`);ye.value=String($),we.append(ye)}we.value=String(Math.min(_,H.length-1));const ee=at("div"),ce=()=>{ee.replaceChildren(F(H[Number(we.value)],"ДРУГОЕ ПРОЧТЕНИЕ")),e.invalidate()};we.addEventListener("change",()=>{a(),_=Number(we.value),ce()}),he.append(_e,we,ee),ce(),ie.append(he)}else ie.append(w("Других прочтений не показано",["В полученной области нет других оспаривающих связей. Это не означает согласия или доказанности."],"sc-reading-empty"));W(ie);const X=Y.coverage;if(W(at("p",`Получено оспаривающих связей: ${X.returned_challenge_relations??Y.challenge_relations.length} из ${X.available_challenge_relations??Y.challenge_relations.length}.`+(x.kind==="relation"?" Выбранная связь исключена из второго столбца.":""),"sc-evidence-note")),Z.contextual_readings.length){const he=at("details","","sc-evidence-refs");he.append(at("summary","Контекст · "+Z.contextual_readings.length));for(const _e of Z.contextual_readings)he.append(F(_e,"СВЯЗЬ КОНТЕКСТА"));W(he)}W(G("Что остаётся открытым",Z.gaps))}function B(){d.capture(),d.enter(M+"|"+C),c.replaceChildren(),f.textContent="",c.setAttribute("aria-busy",String(!m&&!p)),p?W(w("Не удалось прочитать основания",[p.message]),Ys("Повторить",()=>{a(),oe(x,C).catch(()=>{})},"sc-evidence-source")):m?C==="grounds"?R():J():W(at("div","Собираю источники и прочтения…","sc-evidence-loading")),d.restore(),e.invalidate()}async function oe(Z,Y=null,{signal:ne,limit:H=60}={}){if(!Z?.raw)throw new Error("Сначала выберите звезду или отношение.");const te=++L;d.capture();const ie=e.port.packet?.source_revision,X=JSON.stringify([Z.raw.id,Z.kind,ie]);X!==M&&(_=0),D=document.activeElement,t.open("evidence"),x=Z,m=null,p=null,M=X,Y=Y||h.get(M)||"grounds",l.dataset.itemId=Z.raw.id,l.querySelector("h3").textContent=Qi(mt(Z.raw.display.title||Z.raw.display.label)),l.querySelector(".sc-evidence-kind").textContent=Z.kind==="relation"?"ОТНОШЕНИЕ":mt(Z.raw.display.kind_label,"УЗЕЛ").toUpperCase(),b(Y),v[P.indexOf(Y)].focus();try{const he=await o.run("evidence",_e=>y0(Z.raw,Z.kind,ie,{client:i,queries:r,limit:H,signal:ne?AbortSignal.any([ne,_e]):_e}));if(ne?.throwIfAborted(),!he.current||l.hidden)throw new DOMException("Panel closed","AbortError");return m=he.value,B(),m}catch(he){throw te===L&&!l.hidden&&(p=he.name==="AbortError"?new Error("Чтение прервано. Можно повторить запрос."):he,B()),he}}n.addEventListener("sophia-evidence",Z=>{a(),oe(Z.detail).catch(()=>{})});function k(){const Z=s();!l.hidden&&M!==JSON.stringify([Z?.id,Z?.kind==="edge"?"relation":"node",e.port.packet?.source_revision])&&t.close("evidence")}async function K(Z,Y,{signal:ne}){const H=s(),te=H?.kind==="edge"?"relation":"node",ie=te==="relation"?e.port.relation(H?.id):e.port.node(H?.id),X=await oe({raw:ie,kind:te},Z,{signal:ne,limit:Number(Y.limit)||60});if(!X.packet)throw new Error("Для этого объекта маршрут Evidence Lens сейчас недоступен.");return Z==="compare"?Nc(X,H):{...X.packet,binding:X.binding,agent_summary:{...X.packet.agent_summary,selection:ie.id}}}return t.configure("evidence",{onResume:()=>{m?d.restore():oe(x,C).catch(()=>{})}}),yi(),window.addEventListener("pagehide",()=>o.cancelAll()),{selectionChanged:k,handlers:{"tos.page.inspect-epistemic":(Z,Y)=>K("grounds",Z,Y),"tos.page.compare-readings":(Z,Y)=>K("compare",Z,Y)}}}const wn=n=>n?.source_graph==="philosophy"&&typeof n.native_id=="string"&&!!n.native_id,vi=()=>{throw new Ft("Маршрут не удалось связать с текущими данными. Обновите область.")};function w0(n,{depth:e=2,direction:t="either",profile:i="overview"}={}){return{focus_node_id:n,max_depth:e,direction:t,profile:i,page_nodes:10,page_relations:14}}function T0(n){return{schema_version:"tos_lens_spec_v1",lens_id:"observatory-path-search",sources:["philosophy"],language:"ru",detail:"compact",seed:{text_query:n},traversal:{depth:0,profile:"all"},relation_query:{enabled:!1},limits:{nodes:6,relations:0,groups:1}}}function A0(n,e,t,{direction:i,maxDepth:r,alternativeLimit:s,excluded:a}){(n?.schema!=="tos_philosophy_mcp_path_v2"||n.from_id!==e.native_id||n.to_id!==t.native_id||n.direction!==i||n.max_depth!==r||n.alternative_limit!==s||!Array.isArray(n.paths)||n.paths.length>s||n.path_count!==n.paths.length||n.found!==n.paths.length>0||typeof n.exploration_truncated!="boolean"||JSON.stringify([...n.excluded_edge_ids||[]].sort())!==JSON.stringify(a.map(l=>l.native_id).sort()))&&vi();const o=new Set;for(const l of n.paths){const c=l.node_ids,f=l.edge_ids;(!Array.isArray(c)||!Array.isArray(f)||!Array.isArray(l.nodes)||!Array.isArray(l.edges)||!Array.isArray(l.traversal)||c[0]!==e.native_id||c.at(-1)!==t.native_id||c.length!==f.length+1||f.length>r||new Set(c).size!==c.length||new Set(f).size!==f.length||l.nodes.length!==c.length||l.edges.length!==f.length||l.traversal.length!==f.length)&&vi();const u=JSON.stringify(f);o.has(u)&&vi(),o.add(u);for(let d=0;d<c.length;d++)l.nodes[d].node_id!==c[d]&&vi();for(let d=0;d<f.length;d++){const h=l.edges[d],_=l.traversal[d],x=h.from_id===c[d]&&h.to_id===c[d+1],m=h.to_id===c[d]&&h.from_id===c[d+1];(h.edge_id!==f[d]||a.some(p=>p.native_id===f[d])||!x&&!m||i==="outgoing"&&!x||i==="incoming"&&!m||_.edge_id!==f[d]||_.from_node_id!==c[d]||_.to_node_id!==c[d+1]||_.edge_direction!==(x?"forward":"reverse"))&&vi()}}return n}function R0(n,e){return{schema_version:"tos_lens_spec_v1",lens_id:"observatory-path",sources:["philosophy"],language:"ru",detail:"compact",seed:{focus_node_id:e.id},node_query:{filters:[{field:"native_id",op:"in",value:n.node_ids}]},relation_query:{filters:[{field:"native_id",op:"in",value:n.edge_ids}]},traversal:{depth:0,profile:"all"},limits:{nodes:9,relations:8,groups:1}}}function C0(n,e,t,i){function r(o,l,c){o.length!==l.length&&vi();const f=new Map;for(const u of o){const d=l.find(h=>h[c==="node"?"node_id":"edge_id"]===u.native_id);(!wn(u)||f.has(u.native_id)||!d||!Hi(d).some(h=>Hi(u).includes(h)))&&vi(),f.set(u.native_id,u)}return f}const s=r(e.nodes,n.nodes,"node"),a=r(e.relations,n.edges,"relation");(s.get(t.native_id)?.id!==t.id||s.get(i.native_id)?.id!==i.id)&&vi();for(const o of n.edges){const l=a.get(o.edge_id);(l.from_id!==s.get(o.from_id)?.id||l.to_id!==s.get(o.to_id)?.id)&&vi()}return{packet:e,node_ids:n.node_ids.map(o=>s.get(o).id),edge_ids:n.edge_ids.map(o=>a.get(o).id),nodes:n.node_ids.map(o=>s.get(o)),edges:n.edge_ids.map(o=>a.get(o)),traversal:n.traversal.map(o=>({edge_id:a.get(o.edge_id).id,from_node_id:s.get(o.from_node_id).id,to_node_id:s.get(o.to_node_id).id,edge_direction:o.edge_direction}))}}async function P0(n,e,t,{client:i,queries:r,signal:s,direction:a="either",maxDepth:o=6,alternativeLimit:l=3,excluded:c=[]}){if(!wn(n)||!wn(e)||c.some(x=>!wn(x)))throw new Error("Маршруты пока доступны между объектами философского графа.");if(n.id===e.id)throw new Error("Выберите две разные звезды.");if(!["outgoing","incoming","either"].includes(a)||!Number.isInteger(o)||o<1||o>8||!Number.isInteger(l)||l<1||l>5||c.length>64)throw new Error("Выберите глубину от 1 до 8 и до 5 вариантов.");if(c.some(x=>x.native_id.includes(",")))throw new Error("Эту связь пока нельзя исключить через доступный маршрут поиска.");const f=[[n,"node"],[e,"node"],...c.map(x=>[x,"relation"])],u=()=>Promise.all(f.map(([x,m])=>i.inspect(m,x.id,s,t,x.content_revision)));await u(),s?.throwIfAborted();const d={direction:a,maxDepth:o,alternativeLimit:l,excluded:c},h=A0(await r.invoke("tos.path.find",{from_id:n.native_id,to_id:e.native_id,direction:a,max_depth:o,alternative_limit:l,excluded_edge_ids:c.map(x=>x.native_id)},{signal:s}),n,e,d),_=await Promise.all(h.paths.map(async x=>C0(x,await i.compile(R0(x,n),s,t),n,e)));await u(),s?.throwIfAborted();for(const x of _)for(const m of[n,e])if(x.nodes.find(p=>p.id===m.id)?.content_revision!==m.content_revision)throw new Ei;return{schema:"tos_observatory_paths_v1",from_id:n.id,to_id:e.id,source_revision:t,found:_.length>0,path_count:_.length,paths:_,direction:a,max_depth:o,alternative_limit:l,excluded_edge_ids:c.map(x=>x.id),exploration_truncated:h.exploration_truncated,next_actions:_.length?["inspect a route node or relation","try excluding a relation"]:["change direction or depth","restore an excluded relation"]}}const xt=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},mn=(n,e,t="sc-nav-button")=>{const i=xt("button",n,t);return i.type="button",i.addEventListener("click",e),i},Zn=n=>mt(n?.display?.title||n?.display?.label,"Выбрать звезду");function L0(n,e,t,{data:{client:i,queries:r},selected:s,commit:a,onUserAction:o}){const l=new cr;let c="neighbors",f=null,u=null,d=null,h=null,_=null,x=null,m=null,p=0,C=2,D="either",M="overview",L=6,A=3,P=[],v=!1,b=null,w="end",E=0,N=0,G=!1,W=null;const I=xt("section","","sc-panel sc-navigation-panel");I.hidden=!0,I.setAttribute("aria-label","Связи и маршруты"),I.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">АТЛАС ПЕРЕХОДОВ</span><button type="button" class="sc-icon sc-nav-close" aria-label="Закрыть маршруты"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-nav-heading"><span aria-hidden="true">⟡</span><h3>Продолжить мысль</h3></div><div class="sc-nav-tabs" role="tablist" aria-label="Способ исследования"></div><div class="sc-nav-body" id="sc-nav-body" role="tabpanel" tabindex="0"></div><div class="sc-nav-status" role="status"></div><div class="sc-nav-footer"></div>',n.append(I);const R=I.querySelector(".sc-nav-body"),F=I.querySelector(".sc-nav-status"),J=I.querySelector(".sc-nav-footer"),B=Br(R);function oe(){B.capture(),E++,l.cancelAll(),clearTimeout(N),v=!1}t.register("navigation",I,oe);const k=mn("",()=>{o(),H()},"sc-control sc-navigation-open");k.setAttribute("aria-label","Связи и маршруты"),k.innerHTML='<i data-lucide="route" aria-hidden="true"></i><span>Маршруты</span>',n.querySelector(".sc-header-actions").append(k);function K(){t.close("navigation"),(W?.isConnected&&!W.closest("[hidden]")?W:k).focus()}I.querySelector(".sc-nav-close").addEventListener("click",()=>{o(),K()}),I.addEventListener("keydown",y=>{y.key==="Escape"&&(y.preventDefault(),y.stopPropagation(),o(),K())});const Z=["neighbors","paths"].map((y,g)=>{const U=mn(g?"Маршрут":"Связи",()=>{o(),oe(),c=y,b=null,g&&!u&&wn(Y())&&(u=Y()),ve()},"");return U.id="sc-nav-tab-"+y,U.setAttribute("role","tab"),U.setAttribute("aria-controls","sc-nav-body"),U.addEventListener("keydown",z=>{["ArrowLeft","ArrowRight","Home","End"].includes(z.key)&&(z.preventDefault(),Z[z.key==="Home"?0:z.key==="End"?1:1-g].click(),Z[c==="paths"?1:0].focus())}),I.querySelector(".sc-nav-tabs").append(U),U}),Y=()=>{const y=s();return y?.kind==="node"?e.port.node(y.id):null};function ne(){const y=e.port.packet?.source_revision;return y!==h?(oe(),h=y,u=null,d=null,f=null,m=null,x=null,P=[],_=null,!0):!1}function H(y=c){ne(),!_&&e.port.packet&&(_=e.port.captureView()),W=document.activeElement,t.open("navigation"),c=y,f=Y()||f,x&&x.focus.node_id!==f?.id&&(x=null),c==="paths"&&!u&&wn(Y())&&(u=Y()),ve(),Z[c==="paths"?1:0].focus()}function te(y,{command:g=!1}={}){_||(_=e.port.captureView()),G=!0;try{g?a(()=>e.port.setGraph(y)):e.port.setGraph(y)}finally{G=!1}}async function ie(y,g,U){e.ui.cancelPending(),b=null,v=!0;const z=++E;ve();try{const Q=await l.run("navigation",ue=>y(U?AbortSignal.any([ue,U]):ue));if(U?.throwIfAborted(),!Q.current||z!==E||I.hidden)throw new DOMException("Navigation cancelled","AbortError");return g(Q.value),Q.value}catch(Q){throw z===E&&!I.hidden&&(b=Q.name==="AbortError"?new Error("Поиск прерван. Можно повторить."):Q),Q}finally{z===E&&(v=!1,ve())}}const X=y=>()=>{o(),y().catch(()=>{})};function he(y){const g=R.querySelector(y);g&&!I.hidden&&(R.scrollTop+=g.getBoundingClientRect().top-R.getBoundingClientRect().top,e.invalidate())}async function _e(y=!1,g){if(!f)throw new Error("Сначала выберите звезду.");const U=y?x:null,z=f,Q=await ie(async ue=>{await i.inspect("node",z.id,ue,h,z.content_revision);const be=await i.explore(U?{cursor:U.page.next_cursor}:w0(z.id,{depth:C,direction:D,profile:M}),ue,h,U);if(be.focus.node_id!==z.id||be.nodes.find(ae=>ae.id===z.id)?.content_revision!==z.content_revision)throw new Ei;return be},ue=>{x=ue,te(ue,{command:!!g})},g);return he(".sc-nav-page"),Q}async function we(y){if(!u||!d)throw new Error("Выберите начало и конец маршрута.");const g=await ie(U=>P0(u,d,h,{client:i,queries:r,signal:U,direction:D,maxDepth:L,alternativeLimit:A,excluded:P}),U=>{m=U,p=0,U.paths.length&&te(U.paths[0].packet,{command:!!y})},y);return he(g.found?".sc-nav-variants":".sc-nav-empty"),g}function ee(y,g,U,z){const Q=xt("label","","sc-nav-field");Q.append(xt("span",y));const ue=xt("select");for(const[be,ae]of U){const de=xt("option",ae);de.value=be,ue.append(de)}return ue.value=g,ue.disabled=v,ue.addEventListener("change",()=>{o(),oe(),b=null,z(ue.value),ve()}),Q.append(ue),Q}function ce(y){const g=xt("div","","sc-nav-options");return g.append(ee("Направление",D,[["either","В обе стороны"],["outgoing","По связям →"],["incoming","Против связей ←"]],U=>{D=U,m=null,x=null})),g.append(ee("Глубина",String(y?L:C),Array.from({length:y?8:3},(U,z)=>[String(z+1),String(z+1)+" "+(z===0?"шаг":z<4?"шага":"шагов")]),U=>{y?(L=Number(U),m=null):(C=Number(U),x=null)})),y?g.append(ee("Варианты",String(A),[1,2,3,4,5].map(U=>[String(U),"До "+U]),U=>{A=Number(U),m=null})):g.append(ee("Содержание",M,[["overview","Обзор связей"],["all","Все типы связей"]],U=>{M=U,x=null})),g}function $(y){return xt("p",y,"sc-nav-note")}function pe(y,g="node"){o(),!(g==="node"?e.port.node(y.id):e.port.relation(y.id))&&m?.paths[p]&&te(m.paths[p].packet),t.close("navigation"),g==="node"?e.port.selectNode(y.id):e.port.selectRelation(y.id)}function ye(){if(R.append(xt("div","ОТ ВЫБРАННОЙ ЗВЕЗДЫ","sc-nav-caption"),xt("h4",f?Zn(f):"Выберите звезду в пространстве","sc-nav-origin")),!f){R.append($("Откройте её карточку, затем «Окрестность»."));return}R.append(ce(!1));const y=mn(v?"Раскрываю…":"Раскрыть связи",X(()=>_e()),"sc-nav-primary");if(y.disabled=v,R.append(y),x){const g=xt("div","","sc-nav-page");if(g.append(xt("span","ОБЛАСТЬ "+String(x.page.number).padStart(2,"0"),"sc-nav-caption"),xt("p",`Звёзд: ${x.nodes.length} · связей: ${x.relations.length}`)),R.append(g),R.append($(`За этот обход обнаружено узлов: ${x.counts.discovered_nodes}; показано связей: ${x.counts.emitted_relations}. На экране — текущая порция.`)),x.status==="paused"){const z=mn("Продолжить раскрытие →",X(()=>_e(!0)));z.disabled=v,R.append(z)}else R.append($(x.status==="complete"?"Обход завершён в выбранных пределах.":"Достигнут предел обхода. Выберите более близкий центр."));const U=xt("div","","sc-nav-discoveries");for(const z of x.page.primary_node_ids){const Q=x.nodes.find(ue=>ue.id===z);z!==f.id&&U.append(mn(Zn(Q),()=>pe(Q),"sc-nav-discovery"))}R.append(U)}else R.append($("Связи раскрываются небольшими областями. Сохраняются положение камеры и места уже знакомых звёзд."))}function Te(y){w=y,m=null,b=null,ve(),R.querySelector("input")?.focus()}function Pe(y,g,U){const z=mn("",()=>{o(),Te(U)},"sc-nav-endpoint");return z.disabled=v,z.append(xt("small",y),xt("span",Zn(g))),z.dataset.empty=String(!g),z}function Ge(){const y=xt("div","","sc-nav-search"),g=xt("label",w==="start"?"Найти начало":"Найти конечную звезду");g.htmlFor="sc-nav-query";const U=xt("input");U.id="sc-nav-query",U.type="search",U.placeholder="Имя или понятие…",U.autocomplete="off",U.disabled=v;const z=xt("div","","sc-nav-search-results");z.setAttribute("aria-live","polite"),y.append(g,U,z),R.append(y),U.addEventListener("input",()=>{o(),clearTimeout(N),l.cancel("search"),z.replaceChildren();const Q=U.value.trim().slice(0,256);if(!Q)return;const ue=w;z.append($("Ищу…")),N=setTimeout(async()=>{try{const be=await l.run("search",de=>i.compile(T0(Q),de,h));if(!be.current||!U.isConnected||I.hidden)return;z.replaceChildren();const ae=be.value.nodes.filter(wn);for(const de of ae){const Se=mn("",()=>{o(),oe(),ue==="start"?(u=de,d||(w="end")):d=de,m=null,P=[],b=null,ve()},"sc-nav-search-result");Se.dataset.itemId=de.id,Se.append(xt("span",Zn(de)),xt("small",mt(de.display.kind_label)),xt("span",mt(de.display.summary),"sc-nav-search-context"),xt("small",de.native_id,"sc-nav-search-identity")),z.append(Se)}z.append($(ae.length===6?"Первые 6 совпадений. Уточните название для более точного поиска.":ae.length?"Объекты философского графа":"Совпадений в философском графе нет.")),e.invalidate()}catch(be){U.isConnected&&!I.hidden&&(z.replaceChildren($(be.message)),e.invalidate())}},200)})}function Ee(){const y=xt("div","","sc-nav-endpoints");if(y.append(Pe("01 / НАЧАЛО",u,"start"),xt("span","↓","sc-nav-connector"),Pe("02 / НАЗНАЧЕНИЕ",d,"end")),R.append(y),R.append($("Поиск между объектами философского графа. Связность сама по себе не означает согласия.")),m||Ge(),R.append(ce(!0)),P.length){const ue=xt("div","","sc-nav-exclusions");ue.append(xt("span","Обходим связи:","sc-nav-caption"));for(const be of P){const ae=mn(Zn(be)+" · вернуть",X(async()=>{P=P.filter(de=>de.id!==be.id),await we()}));ae.disabled=v,ue.append(ae)}R.append(ue)}const g=mn(v?"Ищу пути…":m?"Найти заново":"Найти пути",X(()=>we()),"sc-nav-primary");if(g.disabled=v||!u||!d||u.id===d.id,R.append(g),!m||(m.found||R.append(xt("h4","Здесь путь не найден","sc-nav-origin sc-nav-empty"),$("В пределах выбранного направления, глубины и исключений. Можно изменить условия и повторить поиск.")),m.exploration_truncated&&R.append($("Поиск достиг вычислительного предела; другие пути могли остаться за ним.")),!m.found))return;const U=xt("div","","sc-nav-variants");U.setAttribute("role","group"),U.setAttribute("aria-label","Варианты маршрута"),m.paths.forEach((ue,be)=>{const ae=mn(String(be+1).padStart(2,"0")+" · "+ue.edges.length+" св.",()=>{o(),p=be,te(ue.packet),ve()},"sc-nav-variant");ae.setAttribute("aria-pressed",String(be===p)),ae.disabled=v,U.append(ae)}),R.append(U),R.append($(`Путь ${p+1} из ${m.path_count} · поиск до ${m.alternative_limit} вариантов.`));const z=m.paths[p],Q=xt("ol","","sc-nav-itinerary");z.nodes.forEach((ue,be)=>{const ae=xt("li");if(ae.append(mn(Zn(ue),()=>pe(ue),"sc-nav-stop")),z.edges[be]){const de=z.edges[be],Se=xt("div","","sc-nav-hop");Se.append(mn(Zn(de),()=>pe(de,"relation"),"sc-nav-relation")),z.traversal[be].edge_direction==="reverse"&&Se.append(xt("small","← против направления связи"));const ke=mn("Обойти",X(async()=>{P.some(Ae=>Ae.id===de.id)||(P=[...P,de]),await we()}),"sc-nav-skip");ke.setAttribute("aria-label","Обойти связь: "+Zn(de)),ke.disabled=v,Se.append(ke),ae.append(Se)}Q.append(ae)}),R.append(Q)}function ve(){B.capture(),B.enter(JSON.stringify([h,f?.id,c,p])),Z.forEach((y,g)=>{const U=c===(g?"paths":"neighbors");y.setAttribute("aria-selected",String(U)),y.tabIndex=U?0:-1}),R.setAttribute("aria-labelledby","sc-nav-tab-"+c),R.setAttribute("aria-busy",String(v)),R.replaceChildren(),c==="paths"?Ee():ye(),F.textContent=b?.message||"",I.dataset.state=v?"loading":b?"error":"ready",J.replaceChildren(),_?J.append(mn("↶ К исходному виду",()=>{o();const y=_;_=null,t.close("navigation"),G=!0;try{e.port.restoreView(y)}finally{G=!1}},"sc-nav-return")):J.append(xt("span","От звезды — к созвездию")),B.restore(),e.invalidate()}async function O({raw:y,kind:g,tab:U}){if(ne(),_||(_=e.port.captureView()),H(U||"paths"),g==="node"){f=y,c==="paths"&&wn(y)?(u&&u.id!==y.id?d=y:u=y,m=null,P=[]):c==="paths"&&(u=null,d=null,m=null,P=[],b=new Error("Для этой звезды поиск путей пока не подключён. Можно раскрыть её связи или выбрать объекты философского графа.")),ve();return}if(!wn(y)){b=new Error("Поиск обходного пути пока доступен для связей философского графа."),ve();return}await ie(async z=>(await Promise.all([y.from_id,y.to_id].map(ue=>i.inspect("node",ue,z,h)))).map(ue=>ue.match),z=>{[u,d]=z,P=[y],m=null})}n.addEventListener("sophia-navigate",y=>{o(),O(y.detail).catch(()=>{})});function Xe(){G||(ne()&&!I.hidden&&(b=new Error("Область обновилась. Выберите звезду для нового исследования."),ve()),v&&(oe(),I.hidden||(b=new Error("Выбор изменился. Повторите поиск из нужной звезды."),ve())))}async function Ze(y,{signal:g},U=!1){if(y.constrain_to_view)throw new Error("Ограничение маршрута текущим видом пока не подключено. Доступен философский граф целиком.");const z=s(),Q=U?e.port.relation(z?.id):Y();if(!Q||!wn(Q))throw new Error("Выберите объект философского графа.");if(ne(),_||(_=e.port.captureView()),H("paths"),U){const ue=await Promise.all([Q.from_id,Q.to_id].map(be=>i.inspect("node",be,g,h)));g.throwIfAborted(),[u,d]=ue.map(be=>be.match),P=[Q]}else{if(!u)throw new Error("Сначала задайте начало маршрута.");d=Q,P=[]}return y.excluded_edge_ids?.length&&(P=(await Promise.all(y.excluded_edge_ids.map(ue=>i.inspect("relation",ue,g,h)))).map(ue=>ue.match)),D=y.direction||"outgoing",L=Number(y.max_depth)||6,A=Number(y.alternative_limit)||3,we(g)}return t.configure("navigation",{onResume:()=>{B.restore(),ve()}}),yi(),window.addEventListener("pagehide",oe),{selectionChanged:Xe,get startId(){return u?.id||null},handlers:{"tos.page.start-path":()=>{const y=Y();if(!wn(y))throw new Error("Выберите звезду философского графа.");return ne(),_||(_=e.port.captureView()),u=y,d=null,m=null,P=[],H("paths"),{path_start_node_id:u.id}},"tos.page.find-path":(y,g)=>Ze(y,g),"tos.page.reroute-without-selection":(y,g)=>Ze(y,g,!0),"tos.page.show-neighborhood":async(y,{signal:g})=>{if(ne(),f=Y(),!f)throw new Error("Сначала выберите звезду.");_||(_=e.port.captureView()),H("neighbors"),C=Math.max(1,Math.min(3,Number(y.depth)||1));const U=await _e(!1,g);return{node:{node_id:f.id,label:Zn(f)},neighbors:U.nodes.filter(z=>z.id!==f.id).map(z=>({node_id:z.id,label:Zn(z)})),edges:U.relations.map(z=>({edge_id:z.id})),page:U.page,status:U.status}}}}}const Yd={meaning:"Понятия и смыслы",identity:"Люди и произведения",history:"История и традиции",authorship:"Авторство и передача",evidence:"Источники и свидетельства",structure:"Структура источников",other:"Другие типы",technical:"Технические типы",unavailable:"Выбрано вне этих источников"},I0=Object.keys(Yd),Uc=new Intl.Collator("ru",{numeric:!0,sensitivity:"base"});function D0(n,e){const t=e==="predicates",i=n.semantic_registries||{},r=new Map((i.entity_types?.entries||[]).map(l=>[l.type_id,l])),s=t?new Map((i.relation_types?.entries||[]).map(l=>[l.relation_type_id,l])):r,a=t?"predicate_id":"kind_id",o=t?"source_predicate_id":"source_kind_id";return(t?n.predicates:n.node_kinds).map(l=>{const c=(l[t?"relation_type_ids":"type_ids"]||[]).map(m=>s.get(m)),f=c.flatMap(m=>(m?.source_mappings||[]).filter(p=>p[o]===l[a]&&(!t||p.scope==="edge"))),u=[...new Set(f.map(m=>m.source_graph).filter(m=>typeof m=="string"))],d=c.length>0&&c.every(Boolean)&&u.length>0&&!l.mapping_statuses?.includes("unmapped");function h(m){if(!m)return"other";if(!t)return{semantic:"meaning",identity:"identity",navigation:"history",evidence:"evidence",assertion:"evidence",activity:"evidence",literal:"evidence",projection:"technical"}[m.object_role]||"other";const p=m.domain_type_ids||[],C=m.range_type_ids||[];return m.assertion_mode==="derived-projection"||[...p,...C].length>0&&[...p,...C].every(D=>r.get(D)?.object_role==="projection")?"technical":m.parent_relation_type_ids?.includes("tos.relation.responsibility")?"authorship":m.assertion_mode==="structural"?"structure":m.assertion_mode==="reified-claim"||m.parent_relation_type_ids?.includes("tos.relation.claim-structure")?"evidence":m.assertion_mode==="direct"?"meaning":"other"}const _=c.map(h),x=_.find(m=>m!=="technical")||_[0]||"other";return{id:l[a],title:mt(l.display,l[a]),group:x,sources:u,sourceKnown:d,count:l.count||0}})}function N0(n,{sources:e,selected:t=[],query:i="",sort:r="alphabet"}={}){const s=i.trim().toLocaleLowerCase("ru"),a=new Set(t),o=new Map,l=[...n];for(const c of a)n.some(f=>f.id===c)||l.push({id:c,title:c,group:"other",sources:[],sourceKnown:!1,count:0});for(const c of l){const f=!c.sourceKnown||c.sources.some(d=>e.includes(d));if(!f&&!a.has(c.id)||s&&!(c.title+" "+c.id).toLocaleLowerCase("ru").includes(s))continue;const u=f?c.group:"unavailable";o.has(u)||o.set(u,[]),o.get(u).push({...c,selected:a.has(c.id),available:f})}return I0.filter(c=>o.has(c)).map(c=>({key:c,title:Yd[c],items:o.get(c).sort((f,u)=>Number(u.selected)-Number(f.selected)||(r==="frequency"?u.count-f.count:0)||Uc.compare(f.title,u.title)||Uc.compare(f.id,u.id))}))}const Pt=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Oi=(n,e)=>{const t=Pt("option",e);return t.value=n,t},Fc=(n,e)=>{const t=Pt("button",n,"sc-builder-link");return t.type="button",t.addEventListener("click",e),t},Oc=(n,e)=>{const t=Pt("label","","sc-builder-field");return t.append(Pt("span",n),e),t},Bc=n=>/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(n.trim())&&Number.isFinite(Number(n))?Number(n):n;let U0=0;function kc({draft:n,context:e,kind:t,onChange:i}){const r=na(e,t),s=n.conditions[t],a=t==="nodes",o=Pt("section","","sc-conditions");o.setAttribute("aria-label",a?"Условия исходных узлов":"Условия связей"),o.append(Pt("h4",a?"Условия исходных узлов":"Условия связей"));const l=a?n.scope==="focus":!n.relations;o.append(Pt("p",l?a?"Сохранены, но не действуют при выборе явного центра.":"Сохранены, но не действуют, пока связи выключены.":"Все условия этого раздела действуют одновременно.","sc-builder-note")),r.length||o.append(Pt("p","Сервер пока не объявил совместимые свойства и операции.","sc-builder-note"));function c(){o.querySelectorAll(".sc-condition,.sc-condition-add").forEach(u=>u.remove()),s.forEach((u,d)=>{const h=Pt("fieldset","","sc-condition");h.append(Pt("legend",`Условие ${d+1}`));const _=Pt("div","","sc-condition-controls"),x=Pt("input");x.type="search",x.placeholder="Найти свойство…",x.setAttribute("aria-label",`Найти свойство условия ${d+1}`);const m=Pt("select");m.setAttribute("aria-label",`Свойство условия ${d+1}`);const p=Pt("div","","sc-condition-detail");function C(){const M=x.value.toLocaleLowerCase("ru"),L=Jn(u),A=r.find(b=>Jn(b)===L);m.replaceChildren();const P=r.filter(b=>(b.title+" "+b.id).toLocaleLowerCase("ru").includes(M)),v=P.slice(0,100);A&&!v.includes(A)&&v.unshift(A),A||m.append(Oi(L,"Недоступно: "+u.id));for(const[b,w]of[["property_id","Свойства сущностей"],["field","Поля представления"]]){const E=Pt("optgroup");E.label=w;const N=new Map;for(const G of v)N.set(G.title,(N.get(G.title)||0)+1);for(const G of v.filter(W=>W.selector===b))E.append(Oi(Jn(G),G.title+(N.get(G.title)>1?" · "+G.id:"")));E.children.length&&m.append(E)}if(m.value=L,P.length>100){const b=Oi("","Показаны первые 100 — уточните поиск");b.disabled=!0,m.append(b)}}function D(){p.replaceChildren();const M=r.find(E=>Jn(E)===Jn(u));if(!M){p.append(Pt("p",ro(u,[]),"sc-builder-note"),Pt("p","Это условие сохранено. Выберите доступное свойство или удалите условие.","sc-builder-warning"));return}const L=Pt("select");L.setAttribute("aria-label",`Операция условия ${d+1}`),M.operators.includes(u.op)||L.append(Oi(u.op,"Недоступно: "+(ls[u.op]||u.op)));for(const E of M.operators)L.append(Oi(E,ls[E]));L.value=u.op,L.addEventListener("change",()=>{u.op=L.value,u.op==="exists"||M.valueType==="boolean"?u.value=!0:(Array.isArray(u.value)||typeof u.value=="boolean")&&(u.value=""),D(),i(),p.querySelector("input,textarea,select:last-child")?.focus()});const A=u.op==="exists"||M.valueType==="boolean",P=u.op==="in"||u.op==="contains"&&M.valueType==="string-array";let v;A?(v=Pt("select"),v.append(Oi("true","Да"),Oi("false","Нет")),v.value=String(u.value)):P?(v=Pt("textarea"),v.rows=2,v.placeholder="По одному значению на строку",v.value=Array.isArray(u.value)?u.value.join(`
`):String(u.value),v.maxLength=12e3):(v=Pt("input"),v.type="text",v.maxLength=1024,v.value=String(u.value),M.valueType==="number"&&(v.inputMode="decimal")),v.setAttribute("aria-label",`Значение условия ${d+1}`);const b=(E={})=>{u.value=A?v.value==="true":P?v.value.split(`
`).map(N=>M.valueType==="number"?Bc(N):N):M.valueType==="number"?Bc(v.value):v.value,i({composing:E.isComposing===!0})};v.addEventListener(A?"change":"input",b),v.addEventListener("compositionstart",()=>i({composing:!0})),v.addEventListener("compositionend",()=>b());const w=Pt("div","","sc-condition-value");if(w.append(Oc("Операция",L),Oc(A?"Выберите значение":P?"Значения · по одному на строку":M.valueType==="number"?"Число":"Значение",v)),p.append(w),M.suggestions.length&&!A&&!P){const E=Pt("datalist");E.id="sc-condition-values-"+ ++U0;for(const N of M.suggestions.slice(0,100))E.append(Oi(String(N),String(N)));v.setAttribute("list",E.id),p.append(E)}if(M.selector==="property_id"){const E=Pt("details","","sc-condition-definition");E.append(Pt("summary","Смысл и область свойства")),M.definition&&E.append(Pt("p",M.definition));const N=e.catalog.semantic_registries?.entity_types?.entries||[];E.append(Pt("p","Применимо к: "+M.appliesTo.map(G=>{const W=N.find(I=>I.type_id===G);return W?.labels?.ru||W?.labels?.default||G}).join(", ")+(M.inherited?" и их подтипам.":"."))),M.unit&&E.append(Pt("p","Единица: "+M.unit)),M.language&&E.append(Pt("p","Язык значения: "+M.language)),M.valueType.startsWith("string")&&E.append(Pt("p","Значение передаётся без изменения регистра, языка или символов. Правила сравнения задаёт сервер.")),E.append(Pt("code",M.id)),p.append(E)}}x.addEventListener("input",C),m.addEventListener("change",()=>{const M=r.find(L=>Jn(L)===m.value);M&&(Object.assign(u,Nl(M)),D(),i())}),C(),D(),_.append(x,m),h.append(_,p,Fc("Удалить условие "+(d+1),()=>{s.splice(d,1),c(),i(),o.querySelector(".sc-condition-add")?.focus()})),o.append(h)});const f=Fc(a?"＋ Условие узла":"＋ Условие связи",()=>{s.push(Nl(r[0])),c(),i(),o.querySelector(".sc-condition:last-of-type input")?.focus()});f.classList.add("sc-condition-add"),f.disabled=l||!r.length||s.length>=Kc,o.append(f)}return c(),o}const je=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},gn=(n,e,t="sc-builder-button")=>{const i=je("button",n,t);return i.type="button",i.addEventListener("click",e),i},F0=new Intl.PluralRules("ru"),rs=(n,e,t,i)=>`${n} ${{one:e,few:t,many:i}[F0.select(n)]||i}`,zc={philosophy:"Философский атлас",canon:"Канон","candidate-intake":"Исследовательские кандидаты","source-navigation":"Произведения и источники","source-claims":"Утверждения источников","semantic-interchange":"Понятия и типы",repository:"Карта проекта"};function O0(n,e,t,{data:{client:i},onUserAction:r}){const s=new cr;let a=null,o=null,l=null,c=null,f=null,u=null,d=!1,h="",_="",x=!1,m=!1,p=[],C=null,D=0,M=!1,L="",A=null,P=!1,v=null;const b=new Map,w=new Map,E=je("section","","sc-panel sc-builder");E.hidden=!0,E.setAttribute("aria-label","Конструктор линз"),E.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ОПТИКА МЫСЛИ</span><button class="sc-icon sc-builder-close" type="button" aria-label="Закрыть конструктор"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-builder-heading"><span aria-hidden="true">◈</span><div><h3>Собрать линзу</h3><p>Настройки сразу меняют пространство.</p></div></div><div class="sc-builder-body"></div><div class="sc-builder-result" aria-live="polite"></div><div class="sc-builder-status" role="status"></div><div class="sc-builder-footer"></div>',n.append(E);const N=E.querySelector(".sc-builder-body"),G=E.querySelector(".sc-builder-result"),W=E.querySelector(".sc-builder-status"),I=E.querySelector(".sc-builder-footer"),R=gn("Конструктор линз",()=>{r(),Y()},"sc-builder-open");n.querySelector(".sc-lenses").append(R);const F=n.querySelector(".sc-lenses-open"),J=gn("",()=>{r(),Y()},"sc-active-query");J.hidden=!0,J.setAttribute("aria-label","Условия отображённой области"),n.querySelector(".sc-context").append(J);function B($,pe=a){const ye=pe?na(pe,"nodes"):[],Te=pe?na(pe,"relations"):[],Pe=[$.scope==="area"?`Из исходной области · ${$.nodeIds.length}`:$.scope==="focus"?"От выбранной звезды":"По всему древу"];if(Pe.push("Источники: "+$.sources.map(Ge=>zc[Ge]||Ge).join(", ")),$.scope!=="focus"){$.query&&Pe.push("Поиск: «"+$.query+"»"),$.kinds.length&&Pe.push("Типы узлов: "+$.kinds.map(Ge=>mt(pe?.catalog.node_kinds.find(Ee=>Ee.kind_id===Ge)?.display,Ge)).join(", "));for(const Ge of $.conditions.nodes)Pe.push(ro(Ge,ye))}if($.relations){$.predicates.length&&Pe.push("Типы связей: "+$.predicates.map(Ge=>mt(pe?.catalog.predicates.find(Ee=>Ee.predicate_id===Ge)?.display,Ge)).join(", "));for(const Ge of $.conditions.relations)Pe.push("Связь: "+ro(Ge,Te));Pe.push(`Окружение: ${$.depth} · ${{either:"в обе стороны",outgoing:"по связям",incoming:"против связей"}[$.direction]} · ${$.profile==="all"?"все типы":"обзор"}`)}else Pe.push("Без связей");return Pe.push("До "+$.limit+" звёзд"),Pe}function oe(){const $=Nr(e.port.packet);J.hidden=!$,$&&(J.textContent="Линза: "+$.name+" · условия",J.title=B($,a?.catalog.source_revision===e.port.packet.source_revision?a:null).join(`
`))}const k=()=>{D++,clearTimeout(A),A=null,P=!1,s.cancelAll(),d=!1};t.register("builder",E,k);function K(){t.close("builder"),(C?.isConnected&&!C.closest("[hidden]")?C:F).focus()}E.querySelector(".sc-builder-close").addEventListener("click",()=>{r(),K()}),E.addEventListener("keydown",$=>{$.key==="Escape"&&($.stopPropagation(),$.preventDefault(),r(),K())});function Z(){f=e.port.packet,u=f,c=f?e.port.captureView():null,l=null,x=!1,o=null,M=!1,v=null,b.clear()}async function Y(){C=document.activeElement,(M||!o||e.port.packet!==u)&&Z(),t.open("builder"),h="",L="";try{p=nd(localStorage),_=""}catch($){p=[],_=$.message||"Локальное хранилище недоступно."}if(a&&a.catalog.source_revision===e.port.packet?.source_revision){ne(),_e();return}await H()}function ne(){o||(o=structuredClone(Nr(e.port.packet)||Bu(f,a)));const $=e.port.selection.nodeId;o.scope==="area"&&$&&f?.nodes.some(pe=>pe.id===$)&&(o.focusId=$)}async function H(){r(),k();const $=D;d=!0,a=null,L="",_e();try{const pe=await s.run("catalog",ye=>nl(i,ye));if(!pe.current||$!==D||E.hidden)return;a=pe.value,ne(),h=""}catch(pe){$===D&&(h=pe.message||"Не удалось загрузить словарь данных.")}finally{$===D&&(d=!1,_e())}}function te({composing:$=!1}={}){r(),k(),h="",L="",l=null,x=!1,v=null,a&&!M&&!$&&(P=!0,A=setTimeout(()=>{A=null,ee()},400)),we()}function ie($,pe){const ye=je("label","","sc-builder-field");return ye.append(je("span",$),pe),ye}function X($,pe,ye){ye.some(([Pe])=>String(Pe)===String(o[pe]))||(ye=[...ye,[o[pe],String(o[pe])]]);const Te=je("select");for(const[Pe,Ge]of ye){const Ee=je("option",Ge);Ee.value=String(Pe),Te.append(Ee)}return Te.value=String(o[pe]),Te.addEventListener("change",()=>{o[pe]=["limit","depth"].includes(pe)?Number(Te.value):Te.value,te(),pe==="scope"&&_e()}),ie($,Te)}function he($,pe){const ye=b.get(pe)||{open:!1,query:"",sort:"alphabet",expanded:new Map,limits:new Map};b.set(pe,ye);const Te=je("details","","sc-builder-choices"),Pe=je("summary"),Ge=je("div","","sc-builder-options"),Ee=je("input");Te.open=ye.open,Te.addEventListener("toggle",()=>ye.open=Te.open),Ee.type="search",Ee.placeholder="Найти в списке…",Ee.setAttribute("aria-label","Найти: "+$),Ee.value=ye.query;const ve=je("select");ve.setAttribute("aria-label","Порядок: "+$);for(const[U,z]of[["alphabet","По алфавиту"],["frequency","Сначала частые"]]){const Q=je("option",z);Q.value=U,ve.append(Q)}ve.value=ye.sort;const O=D0(a.catalog,pe),Xe=new Map;for(const U of O)Xe.set(U.title,(Xe.get(U.title)||0)+1);const Ze=()=>Pe.textContent=$+" · "+(o[pe].length?o[pe].length+" выбрано":"любые");Ze();function y(){const U=Ge.scrollTop;Ge.replaceChildren();const z=N0(O,{sources:o.sources,selected:o[pe],query:ye.query,sort:ye.sort});for(const Q of z){const ue=je("details","","sc-builder-choice-group"),be=je("summary",Q.title+" · "+Q.items.length);ue.dataset.group=Q.key,ue.open=ye.query.trim()!==""||Q.items.some(de=>de.selected)||ye.expanded.get(Q.key)===!0,ue.addEventListener("toggle",()=>ye.expanded.set(Q.key,ue.open)),ue.append(be),Q.key==="unavailable"&&ue.append(je("p","Эти условия сохраняются. Снимите их или включите соответствующий источник.","sc-builder-note"));const ae=ye.limits.get(Q.key)||40;for(const de of Q.items.slice(0,ae)){const Se=je("input");Se.type="checkbox",Se.value=de.id,Se.checked=de.selected,Se.addEventListener("change",()=>{o[pe]=Se.checked?[...o[pe],de.id]:o[pe].filter(Re=>Re!==de.id),Ze(),te()});const ke=je("label","","sc-builder-choice"),Ae=je("span",de.title);Xe.get(de.title)>1&&Ae.append(je("small",de.id)),ke.append(Se,Ae),ue.append(ke)}Q.items.length>ae&&ue.append(gn("Ещё варианты · "+(Q.items.length-ae),()=>{ye.limits.set(Q.key,ae+40),y()},"sc-builder-link")),Ge.append(ue)}z.length||Ge.append(je("p","Нет вариантов для этих источников и поиска.","sc-builder-note")),Ge.scrollTop=U}Ee.addEventListener("input",()=>{ye.query=Ee.value,y()}),ve.addEventListener("change",()=>{ye.sort=ve.value,y()}),w.set(pe,y),y();const g=je("div","","sc-builder-choice-controls");return g.append(Ee,ve),Te.append(Pe,g,Ge,je("p","Типы для выбранных источников. Технические — в отдельной группе.","sc-builder-note"),gn("Сбросить выбор",()=>{o[pe]=[],Ze(),y(),te()},"sc-builder-link")),Te}function _e(){const $=N.scrollTop;if(N.replaceChildren(),w.clear(),!a){N.append(je("p",d?"Загружаю доступные источники и условия…":"Словарь данных пока недоступен.","sc-builder-note")),d||N.append(gn("Повторить загрузку",()=>{H()})),we();return}const pe=je("input");if(pe.type="text",pe.maxLength=64,pe.value=o.name,pe.addEventListener("input",()=>{o.name=pe.value,te()}),N.append(ie("Название линзы",pe)),p.length){const Ee=je("select");Ee.append(je("option","Выбрать сохранённую…"));for(const ve of p){const O=je("option",ve.name);O.value=ve.name,Ee.append(O)}Ee.addEventListener("change",()=>{const ve=p.find(O=>O.name===Ee.value);ve&&(o=structuredClone(ve),te(),_e())}),N.append(ie("Мои линзы",Ee))}if(N.append(X("Отправная точка","scope",[["area","Из исходной области"],["focus","От выбранной звезды"],["all","По всему древу"]])),o.scope==="area"&&N.append(je("p",`Исходная область: ${rs(o.nodeIds.length,"звезда","звезды","звёзд")}. Фильтры выбирают начало; глубина добавляет окружение.`,"sc-builder-note")),o.scope==="focus"){const Ee=e.port.node(o.focusId)||f?.nodes.find(O=>O.id===o.focusId);N.append(je("p",Ee?mt(Ee.display.title):o.focusId?"Звезда из сохранённой линзы.":"Сначала выберите звезду в пространстве.","sc-builder-focus"));const ve=e.port.selection.nodeId;ve&&ve!==o.focusId&&N.append(gn("Взять выбранную звезду",()=>{o.focusId=ve,te(),_e()},"sc-builder-link")),N.append(je("p","Центр остаётся в области. Условия ниже выбирают связи вокруг него.","sc-builder-note"))}const ye=je("fieldset","","sc-builder-sources");ye.append(je("legend","Источники"));for(const Ee of a.catalog.capabilities.sources){const ve=je("input");ve.type="checkbox",ve.checked=o.sources.includes(Ee),ve.addEventListener("change",()=>{o.sources=ve.checked?[...o.sources,Ee]:o.sources.filter(Xe=>Xe!==Ee),te();for(const Xe of w.values())Xe()});const O=je("label");O.append(ve,je("span",zc[Ee]||Ee)),ye.append(O)}if(N.append(ye),o.scope!=="focus"){const Ee=je("input");Ee.type="search",Ee.maxLength=256,Ee.value=o.query,Ee.placeholder="Имя, произведение, понятие…",Ee.addEventListener("input",()=>{o.query=Ee.value,te()}),N.append(ie("Слова в исходных узлах",Ee)),N.append(he("Типы узлов","kinds"))}(o.scope!=="focus"||o.conditions.nodes.length)&&N.append(kc({draft:o,context:a,kind:"nodes",onChange:te}));const Te=je("input");Te.type="checkbox",Te.checked=o.relations,Te.addEventListener("change",()=>{o.relations=Te.checked,te(),_e()});const Pe=je("label","","sc-builder-toggle");Pe.append(Te,je("span","Показывать связи и окружение")),N.append(Pe),o.relations&&N.append(he("Типы связей","predicates")),(o.relations||o.conditions.relations.length)&&N.append(kc({draft:o,context:a,kind:"relations",onChange:te}));const Ge=je("div","","sc-builder-grid");o.relations&&Ge.append(X("Глубина","depth",[[0,"Только исходные"],[1,"1 шаг"],[2,"2 шага"],[3,"3 шага"]]),X("Направление","direction",[["either","В обе стороны"],["outgoing","По связям →"],["incoming","Против связей ←"]]),X("Подробность связей","profile",[["all","Все типы, включая текст"],["overview","Обзор без структуры текста"]])),Ge.append(X("Звёзд в области","limit",[[10,"До 10"],[20,"До 20"],[40,"До 40"]])),N.append(Ge),N.append(je("p","Линза меняет способ просмотра. Фильтры не меняют источники, связи или их статус.","sc-builder-note")),we(),N.scrollTop=$}function we(){if(G.parentElement!==N&&N.append(G),G.replaceChildren(),I.replaceChildren(),W.textContent=M?"Область изменилась. Откройте конструктор заново для нового вида.":h||_||L||(P?"Обновлю пространство…":d?"Обновляю пространство…":x?v&&Object.values(v).every(ye=>ye.added===0&&ye.removed===0)?"Условия применены. Состав этой области совпал.":`В пространстве: ${rs(l.nodes.length,"звезда","звезды","звёзд")} · ${rs(l.relations.length,"связь","связи","связей")}`:l&&!l.nodes.length?"Ничего не найдено. Предыдущий вид сохранён.":"Измените условие — результат появится в пространстве."),a){const ye=je("details","","sc-query-description");ye.append(je("summary","Редактируемые условия"));const Te=je("ul");for(const ve of B(o))Te.append(je("li",ve));ye.append(Te),G.append(ye);const Pe=Nr(e.port.packet);if(Pe&&(JSON.stringify(Pe)!==JSON.stringify(o)||a.catalog.source_revision!==e.port.packet.source_revision)){const ve=je("details","","sc-query-description");ve.append(je("summary","Сейчас в пространстве: "+Pe.name));const O=a.catalog.source_revision===e.port.packet.source_revision;O||ve.append(je("p","Эта область получена из предыдущего снимка. Названия свойств из нового каталога к ней не применяются."));const Xe=je("ul");for(const Ze of B(Pe,O?a:null))Xe.append(je("li",Ze));ve.append(Xe),G.append(ve)}if(l){const ve=td(l);if(G.append(je("strong",`${rs(ve.nodes,"звезда","звезды","звёзд")} · ${rs(ve.relations,"связь","связи","связей")}`)),G.append(je("p",`Условиями выбрано: ${ve.matched}. Из показанных узлов добавлено окружением: ${ve.context}.`)),ve.limited&&G.append(je("p","Результат ограничен размером области. Для другого среза уточните условия.","sc-builder-warning")),v&&G.append(je("p",`Изменение области: звёзды +${v.nodes.added} / −${v.nodes.removed}; связи +${v.relations.added} / −${v.relations.removed}.`)),!ve.nodes){G.append(je("p","В выбранных источниках и области совпадений нет. Это не означает, что таких материалов нет во всём древе."));const O=je("div","","sc-empty-actions");o.scope==="area"&&O.append(gn("Искать по всему древу",()=>{o.scope="all",te(),_e()})),o.query&&O.append(gn("Убрать текстовый поиск",()=>{o.query="",te(),_e()}));const Xe=o.scope!=="focus"&&o.conditions.nodes.length?"nodes":o.relations&&o.conditions.relations.length?"relations":null;Xe&&O.append(gn("Убрать последнее условие "+(Xe==="nodes"?"узла":"связи"),()=>{o.conditions[Xe].pop(),te(),_e()})),G.append(O)}}else G.append(je("p","Изменения применяются автоматически; исходный вид можно вернуть."));const Ge=gn(d?"Обновляю…":"Обновить пространство",()=>{ee()},"sc-builder-primary");Ge.disabled=d||M,I.append(Ge);const Ee=gn("Сохранить линзу",()=>{r();try{ed(o,a),p=ku(localStorage,o),_="",h="",L="Линза сохранена в этом браузере.",_e()}catch(ve){h=ve.message||"Не удалось сохранить линзу.",we()}});Ee.disabled=d||P||M,I.append(Ee),h&&I.append(gn("Обновить каталог",()=>{H()},"sc-builder-link"))}const $=gn("← Предыдущий вид",()=>{r(),k(),n.querySelector(".sc-back").click(),Z(),K()},"sc-builder-link");$.disabled=n.dataset.history==="0",I.append($);const pe=gn("↶ К исходному виду",()=>{r(),k(),m=!0;try{c&&e.port.restoreView(c),Z()}finally{m=!1}K()},"sc-builder-link");pe.disabled=!c,I.append(pe),oe(),e.invalidate()}async function ee(){if(M)return;r(),k(),e.ui.cancelPending(),h="",L="",x=!1,v=null;const $=D;d=!0,we();try{tl(o);const pe=structuredClone(o),ye=await s.run("compile",Te=>il(i,pe,a,Te));if(!ye.current||$!==D||E.hidden)return;if(l=ye.value,l.nodes.length){v=zu(e.port.packet,l),m=!0;try{e.port.setGraph(l),u=e.port.packet,x=!0}finally{m=!1}}}catch(pe){$===D&&(h=pe.message||"Не удалось собрать линзу.",l=null)}finally{$===D&&(d=!1,we(),e.invalidate())}}function ce(){oe(),!(m||E.hidden||e.port.packet===u)&&(k(),l=null,M=!0,we())}return t.configure("builder",{onResume:()=>{a?we():H()}}),addEventListener("pagehide",k),yi(),{selectionChanged:ce}}function B0({fetcher:n,timeoutMs:e=6e4}={}){const t=new Tl({fetcher:n,timeoutMs:e}),i=new Tl({fetcher:n,timeoutMs:e,base:""}),r=eu(async(s,a)=>{try{return await i.request(s,a)}catch(o){throw o instanceof Dr&&o.status===404?new Dr(404,"Этот материал пока недоступен в выбранном способе просмотра."):o}});return Object.freeze({client:t,queries:r})}function H0({host:n=document.getElementById("app"),data:e=B0(),initialRoute:t=location.search}={}){if(!n)throw new Error("Observatory mount point is missing.");n.innerHTML=iu;const i=n.firstElementChild;let r,s,a,o,l,c,f,u,d=!1,h="";const{client:_}=e;function x(){if(s?.auxiliarySelection)return s.auxiliarySelection;const w=r?.port.selection;if(!w)return null;const E=w.relationId?r.port.relation(w.relationId):r.port.node(w.nodeId);return E?{id:E.id,kind:w.relationId?"edge":"node",semantic_kind:E.kind_id||"relation",label:mt(E.display.title||E.display.label),subtitle:mt(E.display.statement),from_id:E.from_id,to_id:E.to_id,predicate_id:E.predicate_id,source_refs:E.source_refs,authority_posture:E.epistemic?.authority_layer,review_posture:E.epistemic?.review_posture,canon_status:E.epistemic?.canon_status,reroutable:wn(E),path_available:wn(E),evidence_available:!!qd(E)}:null}function m(){if(!r)return;const w=x(),E=r.port.packet,N=new URL(location.href);if(E?.focus?.node_id?N.searchParams.set("focus",E.focus.node_id):E&&N.searchParams.delete("focus"),E){const W=Nr(E);W?N.searchParams.set("lens",tl(W)):N.searchParams.delete("lens")}w&&!s?.auxiliarySelection?N.searchParams.set("selection",w.id):N.searchParams.delete("selection"),history.replaceState(null,"",N);const G=JSON.stringify([E?.source_revision,E?.fingerprint,E?.focus,w,i.dataset.lens,o?.startId]);G!==h&&(h=G,d||u?.notifyStateChange()),s?.selectionChanged(),a?.selectionChanged(),o?.selectionChanged(),l?.selectionChanged(),c?.selectionChanged(),f?.selectionChanged()}const p=w=>{d=!0;try{return w()}finally{d=!1,m()}};r=m0(i,{client:_,autoStart:!1,initialFocus:new URLSearchParams(t).get("focus")||to,initialLens:new URLSearchParams(t).get("lens"),onChange:()=>{s?.clearAuxiliarySelection(),m()}});const C=v0(i,r,{onUserAction:()=>u?.notifyStateChange()});s=_0(i,r,{data:e,selected:x,panels:C,onChange:()=>{d||u?.notifyStateChange(),m()}}),a=E0(i,r,C,{data:e,selected:x,onUserAction:()=>u?.notifyStateChange()}),o=L0(i,r,C,{data:e,selected:x,commit:p,onUserAction:()=>u?.notifyStateChange()}),l=O0(i,r,C,{data:e,onUserAction:()=>u?.notifyStateChange()});const D=()=>u?.notifyStateChange();for(const[w,E,N]of[["search","Поиск",".sc-search-open"],["lenses","Линзы",".sc-lenses-open"],["workspace","Исследование",".sc-workspace-open"],["navigation","Маршруты",".sc-navigation-open"]]){const G=i.querySelector(N);C.addTool(w,{title:E,opener:G,launch:()=>G.click()})}function M(){const w=r.port.selection,E=w.relationId?"relation":"node",N=E==="relation"?r.port.relation(w.relationId):r.port.node(w.nodeId);return N?{raw:N,kind:E}:null}for(const[w,E,N,G]of[["builder","Конструктор линз","◈",null],["evidence","Основания","✧","sophia-evidence"],["sources","Источники","◇","sophia-sources"]]){const W=document.createElement("button");W.type="button",W.className="sc-control sc-tool-shortcut",W.setAttribute("aria-label",E);const I=document.createElement("b");I.textContent=N;const R=document.createElement("span");R.textContent=E,W.append(I,R),i.querySelector(".sc-header-actions").append(W);const F=()=>{if(D(),w==="builder")i.querySelector(".sc-builder-open").click();else{const J=M();J?i.dispatchEvent(new CustomEvent(G,{detail:J})):r.port.announce("Сначала выберите звезду или связь.")}};W.addEventListener("click",F),C.addTool(w,{title:E,opener:W,launch:F,available:()=>w==="builder"||!!M()})}f=Du(i,r,C,{data:e,onUserAction:D}),c=Yu(i,r,C,{data:e,initialRoute:t,onUserAction:D}),Zu(i,r);const L={...s.handlers,...a.handlers,"tos.page.inspect-selection":()=>x(),"tos.page.open-view":async(w,{signal:E})=>{if(r.ui.cancelPending(),w.mode!=="philosophy"||w.graph_mode&&w.graph_mode!=="nodes"||!["constellations","observatory"].includes(String(w.view_id)))throw new Error("Эта линза открывается в расширенном исследовательском режиме.");const N=await _.compile(ta(String(w.focus_id||to)),E);return E.throwIfAborted(),p(()=>r.port.setGraph(N,{selectFocus:!!w.focus_id})),{view_id:"observatory"}},"tos.page.select":async(w,{signal:E})=>{r.ui.cancelPending();const N=String(w.item_id||"");if(p(()=>s.chooseGap(N)))return x();if(r.port.node(N))return p(()=>r.port.selectNode(N)),x();if(r.port.relation(N))return p(()=>r.port.selectRelation(N)),x();const G=A.get(N);if(!G)throw new Error("Выберите объект из текущей области или результатов поиска.");const W=await _.compile(G.from_id?Vc(G):ta(N),E,P);return E.throwIfAborted(),p(()=>{r.port.setGraph(W,{selectFocus:!G.from_id}),G.from_id&&r.port.selectRelation(N,{rememberView:!1})}),x()},"tos.page.search":async(w,{signal:E})=>{r.ui.cancelPending();const N=String(w.query||"").trim().slice(0,256),G=await _.search(N,E);E.throwIfAborted(),r.openSearch(),r.ui.cancelSearch(),i.querySelector("#sc-query").value=N;const W=i.querySelector(".sc-search-results");W.replaceChildren(),A.clear(),P=G.source_revision;for(const[I,R]of[["node",G.nodes],["relation",G.relations]])for(const F of R)A.set(F.id,F),W.append(r.ui.searchRow(F,I,G.source_revision));return r.invalidate(),{query:N,result_count:G.counts.matching_nodes+G.counts.matching_relations,results:[...A.values()].map(I=>({id:I.id,label:mt(I.display.title||I.display.label),kind:I.kind_id||"relation",summary:mt(I.display.summary||I.display.statement)}))}},...o.handlers,"tos.page.clear-focus":()=>p(()=>(r.closeInspector(!1,!0),r.overview(),{cleared:!0}))},A=new Map;let P=null;for(const w of Object.keys(s.handlers)){const E=L[w];L[w]=(N,G)=>p(()=>E(N,G))}u=nu(()=>{const w=Nr(r.port.packet);return{mode:"philosophy",view_id:"observatory",graph_mode:"nodes",selected:x(),path_start_node_id:o.startId,active_layers:w?[...w.sources]:["knowledge"],active_predicates:w?w.relations?w.predicates.length?[...w.predicates]:[...new Set(r.port.packet.relations.map(E=>E.predicate_id))]:[]:["overview"],deep_link:location.href,research_workspace:s.workspace.summary()}},L);const v=new Set(["tos.page.context","tos.page.cancel",...Object.keys(L)]);i.querySelector("#sc-query").addEventListener("input",()=>u.notifyStateChange());const b=tu(u,document,v);return b.subscribeStatus(w=>s.agentStatus(w)),b.start(),window.addEventListener("pagehide",()=>b.stop()),window.addEventListener("pageshow",w=>{w.persisted&&b.start()}),c.start().then(w=>{if(w)return;const E=new URLSearchParams(t).get("selection");if(!E)return;const N=()=>{r.port.packet&&(G.disconnect(),p(()=>{r.port.node(E)?r.port.selectNode(E):r.port.relation(E)&&r.port.selectRelation(E)}))},G=new MutationObserver(N);G.observe(i,{attributes:!0,attributeFilter:["data-graph-revision"]}),N()}),{root:i,scene:r}}export{H0 as mountObservatory};
