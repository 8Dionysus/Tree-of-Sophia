import{a as Uc,b as Yd,e as jo,c as Kd,d as $d}from"./webmcp-BcZzX6i6.js";const Zd=`<div id="sophia-gestures" aria-label="Древо Софии — область исследования">
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
    <div class="sc-panel-bottom"><button type="button" class="sc-open-neighborhood">Окрестность</button><button type="button" class="sc-focus">Приблизиться <span aria-hidden="true">↗</span></button></div>
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
</div>`,el="tos.work.friedrich-nietzsche.also-sprach-zarathustra",Gn=Object.freeze({nodes:40,relations:80}),Nc=new WeakMap,Jd=n=>Nc.get(n)||null;class Wt extends Error{}class Ti extends Error{constructor(){super("Данные изменились. Обновите область, чтобы продолжить.")}}class Ys extends Error{constructor(e,t){super(t),this.status=e}}function vt(n,e=""){return[n?.ru,n?.default,n?.original,n?.en].find(t=>typeof t=="string"&&t.trim())||e}function ta(n,{depth:e=1}={}){return{schema_version:"tos_lens_spec_v1",lens_id:"sophia-observatory-focus",language:"ru",detail:"compact",explain:!0,seed:{focus_node_id:n},node_query:{enabled:!1},traversal:{depth:e,direction:"either",profile:"overview"},limits:{...Gn,groups:8}}}function Fc(n){const e=ta(n.from_id,{depth:0});return e.node_query={enabled:!0,filters:[{field:"id",op:"in",value:[n.from_id,n.to_id]}]},e.traversal.profile="all",e.relation_query={filters:[{field:"id",op:"eq",value:n.id}]},e.limits={nodes:2,relations:1,groups:2},e}function na(n,e){if(!/^[a-f0-9]{64}$/.test(n?.source_revision||""))throw new Wt("Ответ не содержит версию данных.");if(e&&n.source_revision!==e)throw new Ti;return n}function Ur(n,e){if(!Array.isArray(n))throw new Wt("Неверный список объектов.");const t=new Set;for(const i of n){if(!i||typeof i.id!="string"||!i.id||t.has(i.id)||!i.display||!vt(e==="node"?i.display.title:i.display.label)||!/^[a-f0-9]{64}$/.test(i.content_revision||"")||!Array.isArray(i.source_refs)||!i.source_refs.length||i.source_refs.some(r=>typeof r!="string"||!r))throw new Wt("Неполный или повторяющийся объект.");t.add(i.id)}return t}function Oc(n,e=null){if(na(n,e),n.authority_boundary?.is_source!==!1||n.authority_boundary?.is_canon!==!1||n.authority_boundary?.writes_to_tree!==!1)throw new Wt("Неподдерживаемый контракт области.");if(!Array.isArray(n.nodes)||!Array.isArray(n.relations)||n.nodes.length>Gn.nodes||n.relations.length>Gn.relations)throw new Wt("Область превышает бюджет отображения.");const t=Ur(n.nodes,"node");if(Ur(n.relations,"relation"),n.relations.some(i=>!t.has(i.from_id)||!t.has(i.to_id)))throw new Wt("Связь не содержит оба конца в области.");if(n.focus&&!t.has(n.focus.node_id))throw new Wt("Центр отсутствует в области.");return n}function fa(n,e=null){if(n?.schema!=="tos_lens_result_v1")throw new Wt("Неподдерживаемый контракт линзы.");return Oc(n,e)}function tl(n,e=null,t=null){Oc(n,e);const i=n.page,r=new Set(n.nodes.map(s=>s.id));if(n.schema!=="tos_exploration_result_v1"||n.writes_to_tree!==!1||!/^[a-f0-9]{64}$/.test(n.snapshot_revision||"")||!["paused","complete","limit_reached"].includes(n.status)||!n.focus||!Number.isInteger(i?.number)||i.number<1||i.scope!=="resumable-neighborhood"||i.returned_nodes!==n.nodes.length||i.returned_relations!==n.relations.length||!Array.isArray(i.primary_node_ids)||!Array.isArray(i.context_node_ids)||i.primary_node_ids.length+i.context_node_ids.length!==r.size||new Set([...i.primary_node_ids,...i.context_node_ids]).size!==r.size||[...i.primary_node_ids,...i.context_node_ids].some(s=>!r.has(s))||n.counts?.scope!=="cumulative-discovered-not-global-total"||n.inclusion?.authority!=="query-execution-not-semantic-proof"||(n.status==="paused"?!/^[a-f0-9]{64}$/.test(i.next_cursor||""):i.next_cursor!==null))throw new Wt("Неполная страница раскрытия связей.");if(t&&(n.snapshot_revision!==t.snapshot_revision||n.focus.node_id!==t.focus.node_id||i.number!==t.page.number+1||JSON.stringify(n.query)!==JSON.stringify(t.query)))throw new Ti;return n}class Hr{constructor(){this.slots=new Map}cancel(e){this.slots.get(e)?.abort(),this.slots.delete(e)}cancelAll(){for(const e of this.slots.keys())this.cancel(e)}async run(e,t){this.cancel(e);const i=new AbortController;this.slots.set(e,i);try{const r=await t(i.signal);return this.slots.get(e)===i?{current:!0,value:r}:{current:!1}}catch(r){if(this.slots.get(e)!==i||i.signal.aborted)return{current:!1};throw r}finally{this.slots.get(e)===i&&this.slots.delete(e)}}}class or{constructor({fetcher:e=globalThis.fetch.bind(globalThis),base:t="/api/knowledge",timeoutMs:i=6e4}={}){this.fetcher=e,this.base=t,this.timeoutMs=i}async request(e,{signal:t,body:i}={}){const r=new AbortController;let s=!1;const a=()=>r.abort(t.reason);t?.aborted?a():t?.addEventListener("abort",a,{once:!0});const o=setTimeout(()=>{s=!0,r.abort()},this.timeoutMs);try{const l=await this.fetcher(this.base+e,{signal:r.signal,method:i?"POST":"GET",headers:i?{"Content-Type":"application/json"}:{},...i?{body:JSON.stringify(i)}:{}});if(!l.ok)throw l.status===409?new Ti:new Ys(l.status,{400:"Запрос не удалось исполнить.",404:"Объект больше не доступен.",410:"Срок сохранённого обхода истёк.",413:"Область слишком велика. Выберите более узкий центр.",503:"Этот способ просмотра пока не доступен."}[l.status]||"Не удалось получить данные. Попробуйте ещё раз.");const c=await l.json();if(!c||typeof c!="object")throw new Wt("Неверный ответ сервера.");return c}catch(l){throw s?new Ys(504,"Сервер отвечает дольше обычного. Попробуйте ещё раз."):!r.signal.aborted&&(l instanceof TypeError||l?.name==="NetworkError")?new Ys(0,"Нет связи с данными. Проверьте соединение и повторите запрос."):l instanceof SyntaxError?new Wt("Сервер вернул нечитаемый ответ. Повторите запрос."):l}finally{clearTimeout(o),t?.removeEventListener("abort",a)}}async search(e,t,i=0){const r=na(await this.request("/search?"+new URLSearchParams({query:e,limit:6,offset:i}),{signal:t}));if(r.schema!=="tos_knowledge_search_v1"||r.nodes?.length>6||r.relations?.length>6)throw new Wt("Неподдерживаемый ответ поиска.");return Ur(r.nodes,"node"),Ur(r.relations,"relation"),r}async compile(e,t,i=null){const r=structuredClone(e),s=fa(await this.request("/lenses/compile",{signal:t,body:r}),i);return Nc.set(s,r),s}async explore(e,t,i,r=null){const s=tl(await this.request("/explore",{signal:t,body:e}),i,r);if(!r&&Object.entries(e).some(([a,o])=>JSON.stringify(s.query?.[a])!==JSON.stringify(o)))throw new Wt("Сервер вернул другую область раскрытия.");return s}async inspect(e,t,i,r,s){const a=na(await this.request("/"+(e==="node"?"nodes/":"relations/")+encodeURIComponent(t)+(e==="node"?"?relation_limit=0":""),{signal:i}),r);if(a.schema!==(e==="node"?"tos_knowledge_node_packet_v1":"tos_knowledge_relation_packet_v1"))throw new Wt("Неверная карточка.");Ur(a.matches,e);const o=a.matches.find(l=>l.id===t);if(!o)throw new Wt("Не найден точный идентификатор карточки.");if(s&&o.content_revision!==s)throw new Ti;if(e==="relation"){const l=Ur(a.endpoints,"node");if(!l.has(o.from_id)||!l.has(o.to_id))throw new Wt("Неполные концы связи.")}return{packet:a,match:o}}capabilities(e){return this.request("/explore/capabilities",{signal:e})}}const Qd=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];function jd(n){let e=2166136261;for(const t of n)e=Math.imul(e^t.codePointAt(0),16777619);return e>>>0}function eu(n,e=[]){n.schema==="tos_exploration_result_v1"?tl(n):fa(n);const t=new Map(e.map(c=>[c.id,c])),i=new Map;for(const c of n.relations)i.set(c.from_id,(i.get(c.from_id)||0)+1),i.set(c.to_id,(i.get(c.to_id)||0)+1);const r=n.focus?.node_id,s=n.nodes.slice().sort((c,p)=>(p.id===r)-(c.id===r)||(i.get(p.id)||0)-(i.get(c.id)||0)||c.id.localeCompare(p.id,"en")),a=new Set(n.nodes.map(c=>c.id)),o=new Set(e.filter(c=>a.has(c.id)).map(c=>c.slot));let l=0;return s.map((c,p)=>{const u=t.get(c.id);for(;o.has(l);)l++;const d=u?.slot??l++;o.add(d);const h=vt(c.display.title,c.id),_=h.length>46?h.slice(0,43)+"…":h,x=jd(c.id),g=d*2.399963229728653,m=Qd[d]?.slice()||[Math.cos(g)*(260+d%4*58),Math.sin(g)*(160+d%3*47),-480+x%740];return{id:c.id,raw:c,name:_,fullName:h,original:c.display.title.original||c.display.title.en||"",kind:vt(c.display.kind_label,c.kind_id),description:vt(c.display.summary),main:p<8,above:p%3===1,group:c.id===r?1:c.kind_id==="agent"?0:c.kind_id==="expression"?2:1,slot:d,p:u?.p?.slice()||m,sourcePosition:u?.sourcePosition?.slice()||m.slice(),volumeZ:u?.volumeZ??m[2],pos:u?.pos?.slice()||m.slice(),target:u?.target?.slice()||m.slice()}})}const tu="observatory-custom",Bc="tos-observatory-lenses-v1",En=n=>{throw new Wt(n)},ir=(n,e,t=1024)=>Array.isArray(n)&&n.length<=e&&new Set(n).size===n.length&&n.every(i=>typeof i=="string"&&i.length>0&&i.length<=t),kc=new WeakMap,ha=n=>kc.get(n)||null;function Vr(n){return(!n||n.v!==1||typeof n.name!="string"||!n.name.trim()||n.name.length>64||!["area","focus","all"].includes(n.scope)||!ir(n.sources,7)||!n.sources.length||!ir(n.nodeIds,Gn.nodes)||!ir(n.kinds,100)||!ir(n.predicates,100)||typeof n.query!="string"||n.query.length>256||!(n.focusId===null||typeof n.focusId=="string"&&n.focusId.length>0&&n.focusId.length<=1024)||!Number.isInteger(n.depth)||n.depth<0||n.depth>3||!["either","outgoing","incoming"].includes(n.direction)||!["overview","all"].includes(n.profile)||!Number.isInteger(n.limit)||n.limit<1||n.limit>Gn.nodes||typeof n.relations!="boolean")&&En("Настройки линзы неполны или превышают допустимый размер."),n.scope==="area"&&!n.nodeIds.length&&En("Исходная область пуста. Выберите поиск по древу."),n.scope==="focus"&&!n.focusId&&En("Сначала выберите звезду."),{v:1,name:n.name.trim(),scope:n.scope,sources:[...n.sources],nodeIds:[...n.nodeIds],focusId:n.focusId,query:n.query,kinds:[...n.kinds],predicates:[...n.predicates],depth:n.depth,direction:n.direction,profile:n.profile,limit:n.limit,relations:n.relations}}function nl(n){const e=JSON.stringify(Vr(n));return e.length>12e3&&En("Описание линзы слишком велико для ссылки. Сузьте исходную область."),e}function zc(n){(typeof n!="string"||n.length>12e3)&&En("Ссылка на линзу слишком велика.");try{return Vr(JSON.parse(n))}catch(e){if(e instanceof Wt)throw e;En("Не удалось прочитать настройки линзы.")}}async function il(n,e){const[t,i]=await Promise.all([n.request("/catalog",{signal:e}),n.request("/contracts",{signal:e})]);na(t);const r=t.capabilities,s=i?.contracts?.lens_spec;return(t.schema!=="tos_knowledge_catalog_v1"||t.authority_boundary?.is_source!==!1||t.authority_boundary?.is_canon!==!1||t.authority_boundary?.writes_to_tree!==!1||i?.schema!=="tos_knowledge_contract_bundle_v1"||i.authority_boundary?.writes_to_tree!==!1||s?.properties?.schema_version?.const!=="tos_lens_spec_v1"||!ir(r?.sources,7)||!r.sources.length||!["in"].every(a=>r.filter_operators?.includes(a))||!r.node_fields?.includes("kind_id")||!r.relation_fields?.includes("predicate_id")||!Array.isArray(t.node_kinds)||!Array.isArray(t.predicates)||t.node_kinds.length>5e3||t.predicates.length>5e3||!ir(t.node_kinds.map(a=>a?.kind_id),5e3)||!ir(t.predicates.map(a=>a?.predicate_id),5e3)||!Array.isArray(s.properties.sources?.items?.enum)||!r.sources.every(a=>s.properties.sources.items.enum.includes(a))||!["nodes","relations","groups","traversal_depth"].every(a=>Number.isInteger(r.maximums?.[a])&&r.maximums[a]>=1)||!r.neighborhood_profiles?.some(a=>a.profile==="all")||!r.neighborhood_profiles?.some(a=>a.profile==="overview")||!["nodes","relations","groups"].every(a=>Number.isInteger(s.properties.limits?.properties?.[a]?.maximum)&&s.properties.limits.properties[a].maximum>=1)||!Number.isInteger(s.properties.traversal?.properties?.depth?.maximum)||r.inclusion?.authority!=="query-execution-not-semantic-proof")&&En("Сервер пока не предоставляет совместимый конструктор линз."),{catalog:t,schema:s}}function nu(n,e){return{v:1,name:"Моя линза",scope:n?.nodes?.length?"area":"all",sources:[...e.catalog.capabilities.sources],nodeIds:(n?.nodes||[]).map(t=>t.id),focusId:n?.focus?.node_id||null,query:"",kinds:[],predicates:[],depth:0,direction:"either",profile:"all",limit:Gn.nodes,relations:!0}}function Gc(n,{catalog:e,schema:t}){const i=Vr(n),r=e.capabilities,s=(l,c)=>l.every(p=>c.includes(p));(!s(i.sources,r.sources)||!s(i.kinds,e.node_kinds.map(l=>l?.kind_id))||!s(i.predicates,e.predicates.map(l=>l.predicate_id)))&&En("Словарь данных изменился. Обновите каталог и проверьте выбранные условия.");const a=Math.min(Gn.nodes,r.maximums.nodes,t.properties.limits.properties.nodes.maximum);(i.limit>a||i.depth>Math.min(r.maximums.traversal_depth,t.properties.traversal.properties.depth.maximum))&&En("Сервер не поддерживает выбранный размер области.");const o={schema_version:"tos_lens_spec_v1",lens_id:tu,title:i.name,language:"ru",detail:"compact",explain:!0,sources:i.sources,seed:i.scope==="focus"?{focus_node_id:i.focusId}:{text_query:i.query,...i.scope==="area"?{node_ids:i.nodeIds}:{}},node_query:{enabled:i.scope!=="focus",filters:i.kinds.length?[{field:"kind_id",op:"in",value:i.kinds}]:[]},relation_query:{enabled:i.relations,filters:i.predicates.length?[{field:"predicate_id",op:"in",value:i.predicates}]:[]},traversal:{depth:i.depth,direction:i.direction,profile:i.profile},composition:{endpoint_policy:"both"},limits:{nodes:i.limit,relations:i.relations?Math.min(Gn.relations,r.maximums.relations,t.properties.limits.properties.relations.maximum):0,groups:Math.min(8,r.maximums.groups,t.properties.limits.properties.groups.maximum)}};return i.scope==="focus"&&(o.node_query.filters=[]),o}function Hc(n){fa(n);const e=n.counts;return(!/^[a-f0-9]{64}$/.test(n.fingerprint||"")||n.inclusion?.authority!=="query-execution-not-semantic-proof"||!["matched_nodes","eligible_relations","truncated_nodes","truncated_relations"].every(t=>Number.isInteger(e?.[t])&&e[t]>=0)||e.nodes!==n.nodes.length||e.relations!==n.relations.length)&&En("Сервер не подтвердил состав линзы."),{nodes:n.nodes.length,relations:n.relations.length,matched:e.matched_nodes,context:n.nodes.filter(t=>["traversal","endpoint"].includes(n.inclusion.nodes?.[t.id]?.kind)).length,limited:e.truncated_nodes>0||e.truncated_relations>0}}async function rl(n,e,t,i){const r=await n.compile(Gc(e,t),i,t.catalog.source_revision);return Hc(r),kc.set(r,Vr(e)),r}function Vc(n){const e=n.getItem(Bc);if(!e)return[];e.length>15e4&&En("Сохранённые линзы превышают размер локального хранилища.");try{const t=JSON.parse(e);if(!Array.isArray(t)||t.length>12)throw new Error;return t.map(Vr)}catch{En("Сохранённые линзы не удалось прочитать. Они остались в хранилище без изменений.")}}function iu(n,e){const t=zc(nl(e)),i=Vc(n),r=i.findIndex(s=>s.name===t.name);return r<0?(i.length>=12&&En("Уже сохранено 12 линз. Дайте этой линзе имя одной из существующих, чтобы обновить её."),i.push(t)):i[r]=t,n.setItem(Bc,JSON.stringify(i)),i}function ru(n,e){const t=i=>{const r=new Set((n?.[i]||[]).map(a=>a.id)),s=new Set(e[i].map(a=>a.id));return{added:[...s].filter(a=>!r.has(a)).length,removed:[...r].filter(a=>!s.has(a)).length}};return{nodes:t("nodes"),relations:t("relations")}}const er=(n,e,t)=>typeof n=="number"&&Number.isFinite(n)&&n>=e&&n<=t,Ma=n=>n===null||typeof n=="string"&&n.length>0&&n.length<=1024,ba=n=>Array.isArray(n)&&n.length===3&&n.every(e=>er(e,-1e4,1e4));function so(n){if(!n||!["constellations","plane","orbits"].includes(n.lens)||!er(n.yaw,-1e5,1e5)||!er(n.pitch,-2,2)||!er(n.zoom,.1,5)||!er(n.pan?.x,-1e5,1e5)||!er(n.pan?.y,-1e5,1e5)||!Ma(n.selectedId)||!Ma(n.relationId)||typeof n.panelOpen!="boolean"||!["about","relations"].includes(n.cardTab)||!Array.isArray(n.vertices)||n.vertices.length>40)throw new Error("Не удалось прочитать положение сохранённого места.");const e=new Set,t=n.vertices.map(i=>{if(!Ma(i.id)||!i.id||e.has(i.id)||!Number.isInteger(i.slot)||i.slot<0||i.slot>1e3||!ba(i.p)||!ba(i.sourcePosition)||!ba(i.target)||!er(i.volumeZ,-1e4,1e4))throw new Error("Сохранённое расположение звёзд повреждено.");return e.add(i.id),{id:i.id,slot:i.slot,p:[...i.p],sourcePosition:[...i.sourcePosition],target:[...i.target],pos:[...i.target],volumeZ:i.volumeZ}});return{lens:n.lens,yaw:n.yaw,pitch:n.pitch,zoom:n.zoom,pan:{x:n.pan.x,y:n.pan.y},selectedId:n.selectedId,relationId:n.relationId,panelOpen:n.panelOpen,cardTab:n.cardTab,vertices:t}}const sl="tos-observatory-places-v1",Ks="tos-observatory-resume-v1",ar=()=>{throw new Error("Сохранённое место не удалось прочитать. Запись осталась в браузере.")};function su(n){return(!n||JSON.stringify(n).length>24e3||n.schema_version!=="tos_lens_spec_v1"||!Number.isInteger(n.limits?.nodes)||n.limits.nodes<1||n.limits.nodes>Gn.nodes||!Number.isInteger(n.limits?.relations)||n.limits.relations<0||n.limits.relations>Gn.relations||!Number.isInteger(n.limits?.groups)||n.limits.groups<0||n.limits.groups>8||n.traversal?.depth!==void 0&&(!Number.isInteger(n.traversal.depth)||n.traversal.depth<0||n.traversal.depth>3))&&ar(),Object.fromEntries(["schema_version","lens_id","title","language","detail","explain","sources","seed","node_query","relation_query","traversal","composition","limits"].filter(t=>n[t]!==void 0).map(t=>[t,structuredClone(n[t])]))}function fs(n){(n?.v!==1||typeof n.name!="string"||!n.name.trim()||n.name.length>64||typeof n.id!="string"||!n.id||n.id.length>80||!Number.isFinite(n.savedAt)||!/^[a-f0-9]{64}$/.test(n.sourceRevision||"")||typeof n.route!="string"||n.route.length>5e4)&&ar();const e={v:1,id:n.id,name:n.name.trim(),savedAt:n.savedAt,sourceRevision:n.sourceRevision,route:n.route,spec:su(n.spec),draft:n.draft?Vr(n.draft):null,pose:so(n.pose)};return JSON.stringify(e).length>65e3&&ar(),e}function au(n,e,{name:t,id:i,route:r,savedAt:s=Date.now()}){n.schema==="tos_exploration_result_v1"?tl(n):fa(n);const a=Jd(n)||{schema_version:"tos_lens_spec_v1",lens_id:"observatory-saved-area",language:"ru",detail:"compact",explain:!0,seed:{node_ids:n.nodes.map(o=>o.id)},node_query:{enabled:!0},relation_query:{enabled:!!n.relations.length,filters:n.relations.length?[{field:"id",op:"in",value:n.relations.map(o=>o.id)}]:[]},traversal:{depth:0,profile:"all"},limits:{...Gn,groups:8}};return fs({v:1,id:i,name:t,savedAt:s,route:r,sourceRevision:n.source_revision,spec:a,draft:ha(n),pose:e})}function Wc(n){const e=n.getItem(sl);if(!e)return[];e.length>8e5&&ar();const t=JSON.parse(e);(!Array.isArray(t)||t.length>12)&&ar();const i=t.map(fs);return new Set(i.map(r=>r.id)).size!==i.length&&ar(),i}function Cl(n,e){const t=fs(e),i=Wc(n),r=i.findIndex(s=>s.id===t.id);if(r<0){if(i.length>=12)throw new Error("Сохранено 12 мест. Удалите ненужное место, чтобы добавить новое.");i.unshift(t)}else i[r]=t;return n.setItem(sl,JSON.stringify(i)),i}function ou(n,e){const t=n.getItem(Ks);if(!t)return null;t.length>65e3&&ar();const i=fs(JSON.parse(t));return!e||e===i.route?i:null}async function lu(n,e,t){const i=fs(e),r=i.draft?await rl(n,i.draft,await il(n,t),t):await n.compile({...i.spec,explain:!0},t);if(!r.nodes.length)throw new Error("В этом месте больше нет доступных звёзд. Предыдущий вид сохранён.");return{packet:r,pose:i.pose,changed:r.source_revision!==i.sourceRevision}}const Xc="tos-observatory-interface-v1",Pl=["search","lenses","workspace","navigation","builder","evidence","sources"],al={v:1,pinned:["search","lenses","workspace","navigation"],dock:"auto",text:"comfortable",labels:"normal",sizes:{}},cu=["inspector","workspace","evidence","navigation","builder","studio"];function qc(n){if(n?.v!==1||!Array.isArray(n.pinned)||n.pinned.length>Pl.length||new Set(n.pinned).size!==n.pinned.length||n.pinned.some(t=>!Pl.includes(t))||!["auto","left","right"].includes(n.dock)||!["comfortable","large"].includes(n.text)||!["normal","large"].includes(n.labels)||!n.sizes||typeof n.sizes!="object")throw new Error("Настройки интерфейса не удалось прочитать.");const e={};for(const t of cu){const i=n.sizes[t];if(i){if(!Number.isFinite(i.width)||i.width<280||i.width>760||!Number.isFinite(i.height)||i.height<240||i.height>800)throw new Error("Сохранённый размер окна повреждён.");e[t]={width:i.width,height:i.height}}}return{v:1,pinned:[...n.pinned],dock:n.dock,text:n.text,labels:n.labels,sizes:e}}function du(n){const e=n?.getItem(Xc);if(!e)return structuredClone(al);if(e.length>5e3)throw new Error("Настройки интерфейса слишком велики.");return qc(JSON.parse(e))}const Yc={xmlns:"http://www.w3.org/2000/svg",width:24,height:24,viewBox:"0 0 24 24",fill:"none",stroke:"currentColor","stroke-width":2,"stroke-linecap":"round","stroke-linejoin":"round"};const Kc=([n,e,t])=>{const i=document.createElementNS("http://www.w3.org/2000/svg",n);return Object.keys(e).forEach(r=>{i.setAttribute(r,String(e[r]))}),t?.length&&t.forEach(r=>{const s=Kc(r);i.appendChild(s)}),i},uu=(n,e={})=>{const i={...Yc,...e};return Kc(["svg",i,n])};const fu=n=>{for(const e in n)if(e.startsWith("aria-")||e==="role"||e==="title")return!0;return!1};const hu=(...n)=>n.filter((e,t,i)=>!!e&&e.trim()!==""&&i.indexOf(e)===t).join(" ").trim();const pu=n=>n.replace(/^([A-Z])|[\s-_]+(\w)/g,(e,t,i)=>i?i.toUpperCase():t.toLowerCase());const mu=n=>{const e=pu(n);return e.charAt(0).toUpperCase()+e.slice(1)};const gu=n=>Array.from(n.attributes).reduce((e,t)=>(e[t.name]=t.value,e),{}),Ll=n=>typeof n=="string"?n:!n||!n.class?"":n.class&&typeof n.class=="string"?n.class.split(" "):n.class&&Array.isArray(n.class)?n.class:"",Il=(n,{nameAttr:e,icons:t,attrs:i})=>{const r=n.getAttribute(e);if(r==null)return;const s=mu(r),a=t[s];if(!a)return console.warn(`${n.outerHTML} icon name was not found in the provided icons object.`);const o=gu(n),l=fu(o)?{}:{"aria-hidden":"true"},c={...Yc,"data-lucide":r,...l,...i,...o},p=Ll(o),u=Ll(i),d=hu("lucide",`lucide-${r}`,...p,...u);d&&Object.assign(c,{class:d});const h=uu(a,c);return n.parentNode?.replaceChild(h,n)};const _u=[["path",{d:"m12 19-7-7 7-7"}],["path",{d:"M19 12H5"}]];const vu=[["path",{d:"M17 3a2 2 0 0 1 2 2v15a1 1 0 0 1-1.496.868l-4.512-2.578a2 2 0 0 0-1.984 0l-4.512 2.578A1 1 0 0 1 5 20V5a2 2 0 0 1 2-2z"}]];const xu=[["circle",{cx:"12",cy:"9",r:"1"}],["circle",{cx:"19",cy:"9",r:"1"}],["circle",{cx:"5",cy:"9",r:"1"}],["circle",{cx:"12",cy:"15",r:"1"}],["circle",{cx:"19",cy:"15",r:"1"}],["circle",{cx:"5",cy:"15",r:"1"}]];const yu=[["path",{d:"M13 13.74a2 2 0 0 1-2 0L2.5 8.87a1 1 0 0 1 0-1.74L11 2.26a2 2 0 0 1 2 0l8.5 4.87a1 1 0 0 1 0 1.74z"}],["path",{d:"m20 14.285 1.5.845a1 1 0 0 1 0 1.74L13 21.74a2 2 0 0 1-2 0l-8.5-4.87a1 1 0 0 1 0-1.74l1.5-.845"}]];const Su=[["path",{d:"M5 12h14"}]];const Mu=[["rect",{x:"5",y:"2",width:"14",height:"20",rx:"7"}],["path",{d:"M12 6v4"}]];const bu=[["path",{d:"M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4"}],["path",{d:"M2 6h4"}],["path",{d:"M2 10h4"}],["path",{d:"M2 14h4"}],["path",{d:"M2 18h4"}],["path",{d:"M21.378 5.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z"}]];const Eu=[["rect",{x:"14",y:"3",width:"5",height:"18",rx:"1"}],["rect",{x:"5",y:"3",width:"5",height:"18",rx:"1"}]];const Tu=[["path",{d:"M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"}]];const wu=[["path",{d:"M5 12h14"}],["path",{d:"M12 5v14"}]];const Au=[["circle",{cx:"6",cy:"19",r:"3"}],["path",{d:"M9 19h8.5a3.5 3.5 0 0 0 0-7h-11a3.5 3.5 0 0 1 0-7H15"}],["circle",{cx:"18",cy:"5",r:"3"}]];const Ru=[["path",{d:"m21 21-4.34-4.34"}],["circle",{cx:"11",cy:"11",r:"8"}]];const Cu=[["rect",{width:"20",height:"16",x:"2",y:"4",rx:"2"}],["path",{d:"M2 14h20"}],["path",{d:"M12 20v-6"}]];const Pu=[["path",{d:"M18 6 6 18"}],["path",{d:"m6 6 12 12"}]];const $c=({icons:n={},nameAttr:e="data-lucide",attrs:t={},root:i=document,inTemplates:r}={})=>{if(!Object.values(n).length)throw new Error(`Please provide an icons object.
If you want to use all the icons you can import it like:
 \`import { createIcons, icons } from 'lucide';
lucide.createIcons({icons});\``);if(typeof i>"u")throw new Error("`createIcons()` only works in a browser environment.");if(Array.from(i.querySelectorAll(`[${e}]`)).forEach(a=>Il(a,{nameAttr:e,icons:n,attrs:t})),r&&Array.from(i.querySelectorAll("template")).forEach(o=>$c({icons:n,nameAttr:e,attrs:t,root:o.content,inTemplates:r})),e==="data-lucide"){const a=i.querySelectorAll("[icon-name]");a.length>0&&(console.warn("[Lucide] Some icons were found with the now deprecated icon-name attribute. These will still be replaced for backwards compatibility, but will no longer be supported in v1.0 and you should switch to data-lucide"),Array.from(a).forEach(o=>Il(o,{nameAttr:"icon-name",icons:n,attrs:t})))}};function ki(){$c({icons:{Search:Ru,Layers2:yu,ArrowLeft:_u,GripHorizontal:xu,X:Pu,Minus:Su,Plus:wu,Pause:Eu,Play:Tu,Touchpad:Cu,Mouse:Mu,NotebookPen:bu,Route:Au,Bookmark:vu}})}const en=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},In=(n,e)=>{const t=en("button",n);return t.type="button",t.addEventListener("click",e),t};function Lu(n,e,t,{initialRoute:i,onUserAction:r}){const s=new or,a=new Hr;let o=null,l=[],c="",p="",u="places",d=!1,h=!1,_=!1,x=!0,g=null,m=null,A=null,P=null,M=0;try{o=localStorage,l=Wc(o)}catch(se){c=se.message}const I=In("",()=>{r(),Z()});I.className="sc-control sc-studio-open",I.setAttribute("aria-label","Места и инструменты"),I.setAttribute("aria-expanded","false"),I.innerHTML='<i data-lucide="bookmark" aria-hidden="true"></i><span>Моё пространство</span>',n.querySelector(".sc-header-actions").append(I);const T=en("section","","sc-panel sc-studio");T.hidden=!0,T.setAttribute("aria-label","Места и инструменты"),T.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">МОЁ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-studio-close" aria-label="Закрыть места и инструменты"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Места мысли</h3><div class="sc-studio-tabs" role="tablist" aria-label="Рабочее окружение"></div><div class="sc-studio-body" role="tabpanel" tabindex="0"></div><p class="sc-studio-status" role="status"></p>',n.append(T);const D=T.querySelector(".sc-studio-body"),v=T.querySelector(".sc-studio-status");D.id="sc-studio-content";const S=In("Повторить открытие места",()=>{r(),de(P)});S.className="sc-studio-retry",S.hidden=!0,v.after(S);function w(){M++,a.cancelAll(),d=!1}t.register("studio",T,()=>{w(),I.setAttribute("aria-expanded","false")}),t.configure("studio",{onResume:$});const L=["places","tools"].map((se,_e)=>{const Me=In(_e?"Инструменты":"Места",()=>{r(),w(),u=se,$()});return Me.id="sc-studio-"+se,Me.setAttribute("role","tab"),Me.setAttribute("aria-controls",D.id),T.querySelector(".sc-studio-tabs").append(Me),Me.addEventListener("keydown",ve=>{if(["ArrowLeft","ArrowRight","Home","End"].includes(ve.key)){ve.preventDefault();const F=ve.key==="Home"?0:ve.key==="End"?1:1-_e;L[F].click(),L[F].focus()}}),Me});function V(){t.close("studio"),I.focus()}T.querySelector(".sc-studio-close").addEventListener("click",V),T.addEventListener("keydown",se=>{se.key==="Escape"&&(se.preventDefault(),se.stopPropagation(),V())});function Z(){t.open("studio"),I.setAttribute("aria-expanded","true"),$(),L[u==="places"?0:1].focus()}function k(se){c=se.message||"Не удалось выполнить действие.",ce()}function z(se){try{r(),se()}catch(_e){k(_e)}}const C=()=>vt(e.port.node(e.port.selection.nodeId||e.port.packet?.focus?.node_id)?.display.title,"Моё место");function R(se,_e){if(!e.port.packet?.nodes.length)throw new Error("Сначала дождитесь загрузки области.");return au(e.port.packet,e.port.capturePlace(),{name:se.slice(0,64),id:_e,route:location.search})}function G(se,_e){if(!o)throw new Error("Локальное хранилище недоступно.");l=Cl(o,R(se,_e)),c="",p="Место сохранено в этом браузере.",$()}function Y(){if(!(!_||!x||h||d||!o||!e.port.packet?.nodes.length))try{const se=JSON.stringify(R(C(),"resume"));o.getItem(Ks)!==se&&o.setItem(Ks,se)}catch(se){c="Не удалось сохранить последний вид. "+se.message,ce()}}function ne(){clearTimeout(g),g=setTimeout(Y,800)}async function de(se,{initial:_e=!1}={}){w();const Me=M;e.ui.cancelPending(),d=!0,P=se,c="",p="Возвращаюсь к месту…",ce();try{const ve=await a.run("place",F=>lu(s,se,F));if(!ve.current||Me!==M)return!1;h=!0;try{e.port.setGraph(ve.value.packet,{initial:_e}),e.port.restorePlace(ve.value.pose)}finally{h=!1}return p=ve.value.changed?"Место открыто. Данные обновились с последнего посещения.":"Место открыто.",P=null,x=!0,d=!1,ne(),!_e&&!T.hidden&&$(),e.port.announce(p),!0}catch(ve){return Me===M&&k(ve),!1}finally{Me===M&&(d=!1,ce())}}function ce(){v.textContent=c||p||t.storageError||"",T.setAttribute("aria-busy",String(d)),D.querySelectorAll("[data-place-action]").forEach(se=>se.disabled=d),S.hidden=u!=="places"||!P||!c,S.disabled=d}function J(se,_e){const Me=en("label",se,"sc-studio-field");return Me.append(_e),Me}function ue(){D.append(en("p","Сохраните область, линзу и ракурс. При возвращении данные проверяются заново.","sc-studio-note"));const se=en("form"),_e=en("input");_e.type="text",_e.maxLength=64,_e.value=C().slice(0,64),_e.required=!0,_e.setAttribute("aria-label","Название места");const Me=en("button","Сохранить текущее место");Me.type="submit",Me.dataset.placeAction="save",Me.disabled=d||!e.port.packet,se.append(J("Название места",_e),Me),se.addEventListener("submit",F=>{F.preventDefault(),z(()=>G(_e.value.trim(),crypto.randomUUID()))}),D.append(se);for(const F of l){const K=en("article","","sc-place"),ae=In(F.name,()=>{r(),de(F)});ae.className="sc-place-open",ae.dataset.placeAction="open",K.append(ae,en("small",new Date(F.savedAt).toLocaleDateString("ru",{day:"numeric",month:"long"})+" · "+F.pose.vertices.length+" звёзд при сохранении"));const xe=en("div","","sc-place-actions");xe.append(In("Обновить этим видом",()=>z(()=>G(F.name,F.id))),In("Удалить",()=>z(()=>{A=F,l=l.filter(Ae=>Ae.id!==F.id),o.setItem(sl,JSON.stringify(l)),p="Место удалено.",$()}))),K.append(xe),D.append(K)}A&&D.append(In("Вернуть удалённое место",()=>z(()=>{l=Cl(o,A),A=null,p="Место восстановлено.",$()}))),l.length||D.append(en("p","Здесь появятся места, к которым хочется вернуться.","sc-studio-empty"));const ve=In("Забыть последний вид",()=>z(()=>{o?.removeItem(Ks),x=!1,p="Автовозврат отключён до следующего открытия страницы.",ce()}));ve.className="sc-studio-subtle",D.append(en("p","Последний вид запоминается автоматически в этом браузере.","sc-studio-note"),ve)}function me(se,_e,Me){const ve=en("select");for(const[F,K]of Me){const ae=en("option",K);ae.value=F,ve.append(ae)}return ve.value=t.preferences[_e],ve.addEventListener("change",()=>z(()=>{t.setPreferences({...t.preferences,[_e]:ve.value}),ce()})),J(se,ve)}function B(){D.append(en("p","Закрепите нужное в верхней панели. Все инструменты доступны и отсюда.","sc-studio-note"));const se=t.preferences,_e=t.toolList(),Me=[...se.pinned,..._e.map(ve=>ve.id).filter(ve=>!se.pinned.includes(ve))];for(const ve of Me){const F=_e.find(Ae=>Ae.id===ve);if(!F)continue;const K=en("div","","sc-tool-choice"),ae=en("input");ae.type="checkbox",ae.checked=se.pinned.includes(ve),ae.setAttribute("aria-label","Закрепить: "+F.title),ae.addEventListener("change",()=>z(()=>{const Ae=t.preferences;t.setPreferences({...Ae,pinned:ae.checked?[...Ae.pinned,ve]:Ae.pinned.filter(be=>be!==ve)}),oe(ve)}));const xe=In(F.title,()=>z(()=>t.launch(ve)));if(xe.disabled=!F.available,K.append(ae,xe),ae.checked){const Ae=In("↑",()=>z(()=>{const be=t.preferences,ke=be.pinned.indexOf(ve);ke>0&&([be.pinned[ke-1],be.pinned[ke]]=[be.pinned[ke],be.pinned[ke-1]],t.setPreferences(be),oe(ve))}));Ae.setAttribute("aria-label","Выше: "+F.title),Ae.disabled=se.pinned[0]===ve,K.append(Ae)}D.append(K)}D.append(me("Сторона окна","dock",[["auto","По свободному месту"],["left","Слева"],["right","Справа"]]),me("Текст для чтения","text",[["comfortable","Обычный"],["large","Крупнее"]]),me("Подписи звёзд","labels",[["normal","Обычные"],["large","Крупнее"]])),D.append(In("Восстановить исходное расположение",()=>z(()=>{t.setPreferences(structuredClone(al)),p="Исходные настройки восстановлены.",$()}))),D.append(en("p","Размер окна меняется кнопкой ↔ или за угол. На клавиатуре: стрелки на уголке, Home — сброс.","sc-studio-note"))}function oe(se){$(),[...D.querySelectorAll("input")].find(_e=>_e.getAttribute("aria-label")==="Закрепить: "+t.toolList().find(Me=>Me.id===se)?.title)?.focus()}function $(){const se=D.scrollTop;D.replaceChildren(),T.querySelector("h3").textContent=u==="places"?"Места мысли":"Мои инструменты",L.forEach((_e,Me)=>{const ve=u===(Me?"tools":"places");_e.setAttribute("aria-selected",String(ve)),_e.tabIndex=ve?0:-1}),D.setAttribute("aria-labelledby","sc-studio-"+u),u==="places"?ue():B(),D.scrollTop=se,ce(),e.invalidate()}return n.addEventListener("pointerdown",se=>{d&&!T.contains(se.target)&&(w(),p="Возвращение прервано вашим действием.",ce())},{capture:!0}),n.addEventListener("wheel",se=>{se.target.closest(".sc-panel")||(d&&(w(),p="Возвращение прервано вашим действием.",ce()),ne())},{passive:!0}),n.addEventListener("keydown",se=>{d&&!T.contains(se.target)&&!["Tab","Shift","Control","Meta","Alt"].includes(se.key)&&(w(),p="Возвращение прервано вашим действием.",ce())},{capture:!0}),n.addEventListener("pointerup",ne,{passive:!0}),n.addEventListener("keyup",ne,{passive:!0}),window.addEventListener("pagehide",()=>{clearTimeout(g),Y(),w()}),document.addEventListener("visibilitychange",()=>{document.hidden&&Y()}),ki(),{selectionChanged(){!h&&e.port.packet!==m&&(d&&w(),m=e.port.packet),ne()},async start(){let se=null;try{o&&(se=ou(o,i))}catch(_e){x=!1,k(_e)}if(se){const _e=await de(se,{initial:!0});if(_=!0,_e)return e.ui.start({skipScene:!0}),ne(),!0;x=!1,Z()}return _=!0,e.ui.start(),!1}}}function Iu(n){const e=new Map;if(n?.inclusion?.authority!=="query-execution-not-semantic-proof")return e;for(const t of n.nodes){const i=n.inclusion.nodes?.[t.id]?.kind,r=i==="selector"?"matched":i==="focus"?"focus":["traversal","endpoint"].includes(i)?"context":null;r&&e.set(t.id,r)}return e}function Du(n,e){const t=document.createElement("div");t.className="sc-inclusion-legend",t.hidden=!0,t.title="Причина появления в области; не оценка истинности или значимости",n.querySelector(".sc-context").append(t);let i=null,r=null,s=null,a=0;const o={matched:"Эта звезда соответствует условиям линзы.",focus:"Центр выбранной области.",context:"Эта звезда добавлена через связи с выбранной областью."};function l(){s&&(s.querySelector(".sc-inclusion-hint").hidden=!0,delete s.dataset.hintOpen,s=null)}function c(d){if(!d||d.hidden||d===s)return;l();const h=d.querySelector(".sc-inclusion-hint");if(!h)return;s=d,h.hidden=!1,d.dataset.hintOpen="true";const _=n.getBoundingClientRect(),x=d.getBoundingClientRect(),g=h.getBoundingClientRect(),m=Math.max(_.left+12,Math.min(x.left+(x.width-g.width)/2,_.right-g.width-12)),A=x.bottom+22,P=A+g.height<_.bottom-90?A:x.top-g.height-22;h.style.left=m-x.left+"px",h.style.top=Math.max(_.top+90,P)-x.top+"px"}n.addEventListener("pointerover",d=>c(d.target.closest?.(".sc-node"))),n.addEventListener("pointerout",d=>{s?.contains(d.target)&&!s.contains(d.relatedTarget)&&!s.contains(document.activeElement)&&l()}),n.addEventListener("focusin",d=>c(d.target.closest?.(".sc-node"))),n.addEventListener("focusout",d=>{s?.contains(d.target)&&!s.contains(d.relatedTarget)&&l()}),window.addEventListener("keydown",d=>{d.key==="Escape"&&s&&(l(),d.preventDefault(),d.stopPropagation())},{capture:!0}),n.addEventListener("pointerdown",l,{capture:!0}),n.addEventListener("click",l),n.addEventListener("wheel",l,{passive:!0}),window.addEventListener("resize",l);function p(){const d=e.port.packet;if(!d||d===i)return;l(),clearTimeout(r);const h=new Set(i?.nodes.map(g=>g.id)||[]),_=Iu(d),x={matched:0,focus:0,context:0};for(const g of n.querySelectorAll(".sc-node")){const m=_.get(g.dataset.id);g.dataset.inclusion=m||"",g.dataset.entering=String(!!(i&&!h.has(g.dataset.id)));let A=g.querySelector(".sc-inclusion-hint");m?(x[m]++,A||(A=document.createElement("span"),A.className="sc-inclusion-hint",A.id="sc-inclusion-hint-"+ ++a,A.setAttribute("role","tooltip"),g.append(A)),A.hidden=!0,A.textContent=o[m],g.setAttribute("aria-describedby",A.id)):(A?.remove(),g.removeAttribute("aria-describedby"))}t.replaceChildren();for(const[g,m]of[["matched","По условиям"],["focus","Центр"],["context","Окружение"]])if(x[g]){const A=document.createElement("span");A.dataset.role=g,A.textContent=m+" "+x[g],t.append(A)}t.hidden=!_.size,i=d,e.invalidate(),r=setTimeout(()=>{for(const g of n.querySelectorAll('.sc-node[data-entering="true"]'))g.dataset.entering="false"},1600)}return new MutationObserver(p).observe(n,{attributes:!0,attributeFilter:["data-graph-revision","data-node-count","data-relation-count"]}),p(),window.addEventListener("pagehide",()=>{l(),clearTimeout(r)}),{update:p}}function as(n,{limit:e=48}={}){const t=new Map;let i=null,r=!1;const s=(l,c)=>l.dataset.readingKey||l.querySelector("summary")?.textContent?.trim()||String(c);function a(){if(!i||r||n.getAttribute("aria-busy")==="true"||!n.isConnected||!n.getClientRects().length)return;const l=[...n.querySelectorAll("details")].map((d,h)=>[s(d,h),d.open]),c=n.getBoundingClientRect(),u=[...n.querySelectorAll("h4,article,p")].find(d=>d.getBoundingClientRect().bottom>c.top+4);t.delete(i),t.set(i,{top:n.scrollTop,details:l,anchor:u?{text:u.textContent.slice(0,180),offset:u.getBoundingClientRect().top-c.top}:null}),t.size>e&&t.delete(t.keys().next().value)}function o(){const l=t.get(i);if(!l||!n.getClientRects().length)return;r=!0;const c=new Map(l.details);if([...n.querySelectorAll("details")].forEach((p,u)=>{c.has(s(p,u))&&(p.open=c.get(s(p,u)))}),n.scrollTop=l.top,l.anchor){const p=[...n.querySelectorAll("h4,article,p")].find(u=>u.textContent.slice(0,180)===l.anchor.text);p&&(n.scrollTop+=p.getBoundingClientRect().top-n.getBoundingClientRect().top-l.anchor.offset)}r=!1}return n.addEventListener("scroll",a,{passive:!0}),{capture:a,restore:o,enter(l){l!==i&&(a(),i=l)},get key(){return i}}}function Uu(n,e,{client:t=new or,initialFocus:i=el,initialLens:r}={}){const s=C=>n.querySelector(C),a=new Hr,o=new Map;let l=0,c=null,p=0;const u=as(s("#so-about")),d=as(s("#so-relations")),h=(C,R,G)=>{const Y=document.createElement(C);return Y.className=R,Y.textContent=G,Y};function _(C,R,G="sc-neighbor"){const Y=h("button",G,C);return Y.type="button",Y.addEventListener("click",R),Y}function x(C,R=null){s(".sc-data-notice").hidden=!C,s(".sc-data-notice span").textContent=C,s(".sc-retry").hidden=!R,c=R}s(".sc-retry").addEventListener("click",()=>c?.());function g(){a.cancel("inspect")}function m(){clearTimeout(l),a.cancel("search")}function A(){clearTimeout(l),a.cancelAll(),x("")}function P(){a.cancel("scene"),g(),x(""),e.packet&&(n.dataset.dataState="ready")}function M(C,R){x(C.message||"Связь с данными прервалась.",R),e.announce(C.message)}function I(C,R){o.delete(C),o.set(C,R),o.size>48&&o.delete(o.keys().next().value)}async function T(C,{expected:R=null,initial:G=!1,depth:Y=1,selectFocus:ne=!0}={}){const de=ta(C,{depth:Y});x("Получаю окрестность…"),n.dataset.dataState="loading";try{const ce=await a.run("scene",J=>t.compile(de,J,R));if(!ce.current)return;e.setGraph(ce.value,{initial:G,selectFocus:ne}),n.dataset.dataState="ready",x(""),ne&&s("#so-about-tab").focus(),e.announce("Область загружена. Узлов: "+ce.value.nodes.length+". Связей: "+ce.value.relations.length+".")}catch(ce){n.dataset.dataState="error",M(ce,()=>T(C,{initial:G,depth:Y,selectFocus:ne}))}}function D(C,R){if(R===e.packet?.source_revision&&e.node(C)){e.selectNode(C),s("#so-about-tab").focus();return}T(C,{expected:R,selectFocus:!0})}async function v(C,R){if(R===e.packet?.source_revision&&e.relation(C.id)){e.selectRelation(C.id);return}x("Открываю отношение…"),n.dataset.dataState="loading";try{const G=await a.run("scene",async Y=>{const{match:ne}=await t.inspect("relation",C.id,Y,R,C.content_revision),de=Fc(ne),ce=await t.compile(de,Y,R);if(!ce.relations.some(J=>J.id===ne.id))throw new Wt("Выбранное отношение отсутствует в области.");return{packet:ce}});if(!G.current)return;e.setGraph(G.value.packet),e.selectRelation(C.id,{rememberView:!1}),n.dataset.dataState="ready",x("")}catch(G){n.dataset.dataState="error",M(G,()=>v(C,null))}}function S(C,R,G){const Y=vt(R==="node"?C.display.title:C.display.label,C.id),ne=_("",()=>R==="node"?D(C.id,G):v(C,G),"sc-result"),de=h("span","sc-result-label",Y);if(R==="relation"){const ce=vt(C.display.statement,C.from_id+" → "+C.to_id);de.append(h("span","sc-result-detail",ce)),ne.setAttribute("aria-label",Y+" · "+ce)}return ne.append(de,h("small","",R==="node"?vt(C.display.kind_label):"Отношение")),ne}function w(C,R=0){clearTimeout(l),a.cancel("search"),p=R;const G=C.trim().slice(0,256),Y=s(".sc-search-results");if(Y.replaceChildren(),!G){Y.append(h("div","sc-section-label","В ТЕКУЩЕЙ ОБЛАСТИ"));for(const ne of(e.packet?.nodes||[]).filter(de=>de.id===e.packet?.focus?.node_id||de.kind_id==="agent").slice(0,6))Y.append(S(ne,"node",e.packet.source_revision));Y.append(h("div","sc-empty","Введите имя, название или понятие для поиска во всём древе.")),e.cardChanged();return}Y.append(h("div","sc-empty","Ищу в древе…")),e.cardChanged(),l=setTimeout(async()=>{try{const ne=await a.run("search",J=>t.search(G,J,R));if(!ne.current||s(".sc-search").hidden)return;const de=ne.value;Y.replaceChildren(),n.dataset.searchQuery=G,n.dataset.searchRevision=de.source_revision;for(const J of["node","relation"]){const ue=de[J==="node"?"nodes":"relations"];if(ue.length){Y.append(h("div","sc-section-label",J==="node"?"УЗЛЫ":"ОТНОШЕНИЯ"));for(const me of ue)Y.append(S(me,J,de.source_revision))}}!de.nodes.length&&!de.relations.length&&Y.append(h("div","sc-empty","По этому запросу ничего не найдено."));const ce=document.createElement("div");ce.className="sc-search-pager",R>0&&ce.append(_("Ранее",()=>w(G,Math.max(0,R-6)))),Math.max(de.counts.matching_nodes,de.counts.matching_relations)>R+6&&ce.append(_("Далее",()=>w(G,R+6))),Y.append(ce),e.announce("Результаты поиска обновлены."),e.cardChanged()}catch(ne){if(s(".sc-search").hidden)return;Y.replaceChildren(h("div","sc-empty",ne.message),_("Повторить поиск",()=>w(G,p))),e.cardChanged()}},180)}function L(C,R){const G=s(".sc-provenance");G.replaceChildren();const Y=R==="node"?C.display.summary_state:C.display.explanation_state,ne={authored:"Авторское описание","source-derived":"Описание из источника","metadata-synthesis":"Описание составлено из метаданных",missing:"Описание пока не зафиксировано"};G.append(h("p","sc-description-origin",ne[Y]||"Происхождение описания не указано"));const de=R==="node"?C.display.summary:C.display.explanation;!de?.ru&&de?.default&&G.append(h("p","sc-description-origin","Показан исходный язык описания."));const ce=document.createElement("details");ce.className="sc-source-details",ce.append(h("summary","",`Источники и статус · ${C.source_refs.length}`));const J=C.epistemic||{};for(const[ue,me]of[["Слой",J.authority_layer],["Рассмотрение",J.review_posture],["Канон",J.canon_status]])ce.append(h("p","sc-source-status",ue+": "+(!me||me==="not-recorded"?"не указан":me)));for(const ue of C.source_refs)if(/^https?:\/\//i.test(ue))try{const me=new URL(ue),B=h("a","sc-source-ref",me.hostname+me.pathname);B.href=me.href,B.target="_blank",B.rel="noreferrer noopener",ce.append(B)}catch{ce.append(h("span","sc-source-ref",ue))}else ce.append(h("span","sc-source-ref",ue));ce.addEventListener("toggle",()=>e.cardChanged()),G.append(ce)}function V(C){return vt(e.node(C)?.display?.title,C)}function Z(C,R,G=[]){const Y=JSON.stringify([e.packet?.source_revision,C,R.id,R.content_revision]);u.capture(),d.capture(),u.enter(Y),d.enter(Y),s(".sc-node-title").textContent=vt(C==="node"?R.display.title:R.display.label,R.id),s(".sc-node-original").textContent=C==="node"?R.display.title.original||R.display.title.en||"":vt(R.display.statement),s(".sc-kind").textContent=C==="node"?vt(R.display.kind_label,R.kind_id).toUpperCase():"ОТНОШЕНИЕ",s(".sc-description").textContent=vt(C==="node"?R.display.summary:R.display.explanation,"Описание пока не зафиксировано."),s(".sc-inspector").setAttribute("aria-label",C==="node"?"Выбранный узел":"Выбранное отношение"),n.dataset.inspectorKind=C,n.dataset.inspectorId=R.id;const ne=C==="node"?e.neighbors(R.id):[R];s("#so-relations-tab").firstChild.textContent=C==="node"?"Связи ":"Участники ",s(".sc-neighbor-count").textContent=String(C==="node"?ne.length:new Set([R.from_id,R.to_id]).size);const de=s(".sc-neighbors");if(de.replaceChildren(),C==="node"){for(const ce of ne){const J=ce.from_id===R.id,ue=J?ce.to_id:ce.from_id,me=J?vt(ce.display.label):vt(ce.display.inverse_label)||"← "+vt(ce.display.label),B=_("",()=>e.selectRelation(ce.id),"sc-neighbor sc-relation-row");B.append(h("small","",me),h("span","",V(ue))),de.append(B)}ne.length||de.append(h("p","sc-empty","В этой области связи не показаны."))}else for(const[ce,J]of[["От",R.from_id],["К",R.to_id]]){const ue=G.find(B=>B.id===J)||e.node(J),me=_(ce+": "+vt(ue?.display.title,J),()=>D(J,e.packet.source_revision),"sc-neighbor sc-relation-row");de.append(me)}L(R,C),s(".sc-provenance").append(_("Основания и прочтения",()=>n.dispatchEvent(new CustomEvent("sophia-evidence",{detail:{raw:R,kind:C}}))),_("Открыть источники",()=>n.dispatchEvent(new CustomEvent("sophia-sources",{detail:{raw:R,kind:C}}))),_(C==="node"?"Проложить маршрут":"Другой путь",()=>n.dispatchEvent(new CustomEvent("sophia-navigate",{detail:{raw:R,kind:C,tab:"paths"}})))),u.restore(),d.restore(),e.cardChanged()}async function k(C,R){g();const G=e.packet?.source_revision;if(!G)return;Z(C,R),n.dataset.inspectorState="loading";const Y=[G,C,R.id,R.content_revision].join("|");if(o.has(Y)){const ne=o.get(Y);Z(C,ne.match,ne.packet.endpoints),n.dataset.inspectorState="ready";return}try{const ne=await a.run("inspect",de=>t.inspect(C,R.id,de,G,R.content_revision));if(!ne.current)return;I(Y,ne.value),Z(C,ne.value.match,ne.value.packet.endpoints),n.dataset.inspectorState="ready"}catch(ne){n.dataset.inspectorState="error",s(".sc-provenance").prepend(h("p","sc-inspection-error",ne.message)),ne instanceof Ti?M(ne,()=>T(e.packet.focus?.node_id||R.id,{selectFocus:!1})):s(".sc-provenance").prepend(_("Загрузить карточку ещё раз",()=>k(C,R))),e.cardChanged()}}s(".sc-open-neighborhood").addEventListener("click",()=>{const C=e.node(e.selection.nodeId);C&&n.dispatchEvent(new CustomEvent("sophia-navigate",{detail:{raw:C,kind:"node",tab:"neighbors"}}))});async function z(){n.dataset.dataState="loading",x("Открываю линзу…");try{const C=await a.run("scene",async R=>{const G=zc(r),Y=await il(t,R);return rl(t,G,Y,R)});if(!C.current)return;e.setGraph(C.value,{initial:!0}),x(""),e.announce("Линза загружена. Узлов: "+C.value.nodes.length+". Связей: "+C.value.relations.length+".")}catch(C){n.dataset.dataState="error",M(C,z)}}return addEventListener("pagehide",A),{captureReading:()=>{u.capture(),d.capture()},restoreReading:()=>{u.restore(),d.restore()},loadFocus:T,chooseNode:D,chooseRelation:v,searchRow:S,search:w,showCard:k,cancelSearch:m,cancelInspector:g,cancelPending:A,willSelect:P,async start({skipScene:C=!1}={}){C||(r?z():T(i,{initial:!0,selectFocus:!1}));try{const R=await a.run("capabilities",G=>t.capabilities(G));R.current&&(n.dataset.explorationAvailable=String(R.value.available===!0))}catch{n.dataset.explorationAvailable="false"}}}}const ol="185",Nu=0,Dl=1,Fu=2,$s=1,Ou=2,rs=3,Gi=0,_n=1,vi=2,Mi=0,Nr=1,Ul=2,Nl=3,Fl=4,Zc=5,Oi=100,Bu=101,ku=102,zu=103,Gu=104,Hu=200,ao=201,Vu=202,Jc=203,oo=204,os=205,Wu=206,Xu=207,qu=208,Yu=209,Ku=210,$u=211,Zu=212,Ju=213,Qu=214,lo=0,co=1,uo=2,Br=3,fo=4,ho=5,po=6,mo=7,Qc=0,ju=1,ef=2,kn=0,jc=1,ed=2,td=3,nd=4,id=5,rd=6,sd=7,ad=300,lr=301,kr=302,Ea=303,Ta=304,pa=306,go=1e3,Si=1001,_o=1002,rn=1003,tf=1004,ys=1005,sn=1006,wa=1007,rr=1008,Cn=1009,od=1010,ld=1011,ls=1012,ll=1013,ii=1014,ei=1015,wi=1016,cl=1017,dl=1018,cs=1020,cd=35902,dd=35899,ud=1021,fd=1022,Bn=1023,Ai=1026,sr=1027,hd=1028,ul=1029,cr=1030,fl=1031,hl=1033,Zs=33776,Js=33777,Qs=33778,js=33779,vo=35840,xo=35841,yo=35842,So=35843,Mo=36196,bo=37492,Eo=37496,To=37488,wo=37489,ia=37490,Ao=37491,Ro=37808,Co=37809,Po=37810,Lo=37811,Io=37812,Do=37813,Uo=37814,No=37815,Fo=37816,Oo=37817,Bo=37818,ko=37819,zo=37820,Go=37821,Ho=36492,Vo=36494,Wo=36495,Xo=36283,qo=36284,ra=36285,Yo=36286,nf=3200,Ol=0,rf=1,xi="",Rn="srgb",ds="srgb-linear",sa="linear",It="srgb",yr=7680,Bl=519,sf=512,af=513,of=514,pl=515,lf=516,cf=517,ml=518,df=519,Ko=35044,uf=35048,$o="300 es",ti=2e3,aa=2001;function ff(n){for(let e=n.length-1;e>=0;--e)if(n[e]>=65535)return!0;return!1}function oa(n){return document.createElementNS("http://www.w3.org/1999/xhtml",n)}function hf(){const n=oa("canvas");return n.style.display="block",n}const kl={};function la(...n){const e="THREE."+n.shift();console.log(e,...n)}function pd(n){const e=n[0];if(typeof e=="string"&&e.startsWith("TSL:")){const t=n[1];t&&t.isStackTrace?n[0]+=" "+t.getLocation():n[1]='Stack trace not available. Enable "THREE.Node.captureStackTrace" to capture stack traces.'}return n}function tt(...n){n=pd(n);const e="THREE."+n.shift();{const t=n[0];t&&t.isStackTrace?console.warn(t.getError(e)):console.warn(e,...n)}}function Et(...n){n=pd(n);const e="THREE."+n.shift();{const t=n[0];t&&t.isStackTrace?console.error(t.getError(e)):console.error(e,...n)}}function Fr(...n){const e=n.join(" ");e in kl||(kl[e]=!0,tt(...n))}function pf(n,e,t){return new Promise(function(i,r){function s(){switch(n.clientWaitSync(e,n.SYNC_FLUSH_COMMANDS_BIT,0)){case n.WAIT_FAILED:r();break;case n.TIMEOUT_EXPIRED:setTimeout(s,t);break;default:i()}}setTimeout(s,t)})}const mf={[lo]:co,[uo]:po,[fo]:mo,[Br]:ho,[co]:lo,[po]:uo,[mo]:fo,[ho]:Br};class ur{addEventListener(e,t){this._listeners===void 0&&(this._listeners={});const i=this._listeners;i[e]===void 0&&(i[e]=[]),i[e].indexOf(t)===-1&&i[e].push(t)}hasEventListener(e,t){const i=this._listeners;return i===void 0?!1:i[e]!==void 0&&i[e].indexOf(t)!==-1}removeEventListener(e,t){const i=this._listeners;if(i===void 0)return;const r=i[e];if(r!==void 0){const s=r.indexOf(t);s!==-1&&r.splice(s,1)}}dispatchEvent(e){const t=this._listeners;if(t===void 0)return;const i=t[e.type];if(i!==void 0){e.target=this;const r=i.slice(0);for(let s=0,a=r.length;s<a;s++)r[s].call(this,e);e.target=null}}}const on=["00","01","02","03","04","05","06","07","08","09","0a","0b","0c","0d","0e","0f","10","11","12","13","14","15","16","17","18","19","1a","1b","1c","1d","1e","1f","20","21","22","23","24","25","26","27","28","29","2a","2b","2c","2d","2e","2f","30","31","32","33","34","35","36","37","38","39","3a","3b","3c","3d","3e","3f","40","41","42","43","44","45","46","47","48","49","4a","4b","4c","4d","4e","4f","50","51","52","53","54","55","56","57","58","59","5a","5b","5c","5d","5e","5f","60","61","62","63","64","65","66","67","68","69","6a","6b","6c","6d","6e","6f","70","71","72","73","74","75","76","77","78","79","7a","7b","7c","7d","7e","7f","80","81","82","83","84","85","86","87","88","89","8a","8b","8c","8d","8e","8f","90","91","92","93","94","95","96","97","98","99","9a","9b","9c","9d","9e","9f","a0","a1","a2","a3","a4","a5","a6","a7","a8","a9","aa","ab","ac","ad","ae","af","b0","b1","b2","b3","b4","b5","b6","b7","b8","b9","ba","bb","bc","bd","be","bf","c0","c1","c2","c3","c4","c5","c6","c7","c8","c9","ca","cb","cc","cd","ce","cf","d0","d1","d2","d3","d4","d5","d6","d7","d8","d9","da","db","dc","dd","de","df","e0","e1","e2","e3","e4","e5","e6","e7","e8","e9","ea","eb","ec","ed","ee","ef","f0","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff"],Aa=Math.PI/180,Zo=180/Math.PI;function zi(){const n=Math.random()*4294967295|0,e=Math.random()*4294967295|0,t=Math.random()*4294967295|0,i=Math.random()*4294967295|0;return(on[n&255]+on[n>>8&255]+on[n>>16&255]+on[n>>24&255]+"-"+on[e&255]+on[e>>8&255]+"-"+on[e>>16&15|64]+on[e>>24&255]+"-"+on[t&63|128]+on[t>>8&255]+"-"+on[t>>16&255]+on[t>>24&255]+on[i&255]+on[i>>8&255]+on[i>>16&255]+on[i>>24&255]).toLowerCase()}function yt(n,e,t){return Math.max(e,Math.min(t,n))}function gf(n,e){return(n%e+e)%e}function Ra(n,e,t){return(1-t)*n+t*e}function Qn(n,e){switch(e.constructor){case Float32Array:return n;case Uint32Array:return n/4294967295;case Uint16Array:return n/65535;case Uint8Array:return n/255;case Int32Array:return Math.max(n/2147483647,-1);case Int16Array:return Math.max(n/32767,-1);case Int8Array:return Math.max(n/127,-1);default:throw new Error("THREE.MathUtils: Invalid component type.")}}function Nt(n,e){switch(e.constructor){case Float32Array:return n;case Uint32Array:return Math.round(n*4294967295);case Uint16Array:return Math.round(n*65535);case Uint8Array:return Math.round(n*255);case Int32Array:return Math.round(n*2147483647);case Int16Array:return Math.round(n*32767);case Int8Array:return Math.round(n*127);default:throw new Error("THREE.MathUtils: Invalid component type.")}}const bl=class bl{constructor(e=0,t=0){this.x=e,this.y=t}get width(){return this.x}set width(e){this.x=e}get height(){return this.y}set height(e){this.y=e}set(e,t){return this.x=e,this.y=t,this}setScalar(e){return this.x=e,this.y=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;default:throw new Error("THREE.Vector2: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;default:throw new Error("THREE.Vector2: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y)}copy(e){return this.x=e.x,this.y=e.y,this}add(e){return this.x+=e.x,this.y+=e.y,this}addScalar(e){return this.x+=e,this.y+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this}subScalar(e){return this.x-=e,this.y-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this}multiply(e){return this.x*=e.x,this.y*=e.y,this}multiplyScalar(e){return this.x*=e,this.y*=e,this}divide(e){return this.x/=e.x,this.y/=e.y,this}divideScalar(e){return this.multiplyScalar(1/e)}applyMatrix3(e){const t=this.x,i=this.y,r=e.elements;return this.x=r[0]*t+r[3]*i+r[6],this.y=r[1]*t+r[4]*i+r[7],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this}clamp(e,t){return this.x=yt(this.x,e.x,t.x),this.y=yt(this.y,e.y,t.y),this}clampScalar(e,t){return this.x=yt(this.x,e,t),this.y=yt(this.y,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(yt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this}negate(){return this.x=-this.x,this.y=-this.y,this}dot(e){return this.x*e.x+this.y*e.y}cross(e){return this.x*e.y-this.y*e.x}lengthSq(){return this.x*this.x+this.y*this.y}length(){return Math.sqrt(this.x*this.x+this.y*this.y)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)}normalize(){return this.divideScalar(this.length()||1)}angle(){return Math.atan2(-this.y,-this.x)+Math.PI}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const i=this.dot(e)/t;return Math.acos(yt(i,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,i=this.y-e.y;return t*t+i*i}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this}equals(e){return e.x===this.x&&e.y===this.y}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this}rotateAround(e,t){const i=Math.cos(t),r=Math.sin(t),s=this.x-e.x,a=this.y-e.y;return this.x=s*i-a*r+e.x,this.y=s*r+a*i+e.y,this}random(){return this.x=Math.random(),this.y=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y}};bl.prototype.isVector2=!0;let Tt=bl;class Wr{constructor(e=0,t=0,i=0,r=1){this.isQuaternion=!0,this._x=e,this._y=t,this._z=i,this._w=r}static slerpFlat(e,t,i,r,s,a,o){let l=i[r+0],c=i[r+1],p=i[r+2],u=i[r+3],d=s[a+0],h=s[a+1],_=s[a+2],x=s[a+3];if(u!==x||l!==d||c!==h||p!==_){let g=l*d+c*h+p*_+u*x;g<0&&(d=-d,h=-h,_=-_,x=-x,g=-g);let m=1-o;if(g<.9995){const A=Math.acos(g),P=Math.sin(A);m=Math.sin(m*A)/P,o=Math.sin(o*A)/P,l=l*m+d*o,c=c*m+h*o,p=p*m+_*o,u=u*m+x*o}else{l=l*m+d*o,c=c*m+h*o,p=p*m+_*o,u=u*m+x*o;const A=1/Math.sqrt(l*l+c*c+p*p+u*u);l*=A,c*=A,p*=A,u*=A}}e[t]=l,e[t+1]=c,e[t+2]=p,e[t+3]=u}static multiplyQuaternionsFlat(e,t,i,r,s,a){const o=i[r],l=i[r+1],c=i[r+2],p=i[r+3],u=s[a],d=s[a+1],h=s[a+2],_=s[a+3];return e[t]=o*_+p*u+l*h-c*d,e[t+1]=l*_+p*d+c*u-o*h,e[t+2]=c*_+p*h+o*d-l*u,e[t+3]=p*_-o*u-l*d-c*h,e}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get w(){return this._w}set w(e){this._w=e,this._onChangeCallback()}set(e,t,i,r){return this._x=e,this._y=t,this._z=i,this._w=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._w)}copy(e){return this._x=e.x,this._y=e.y,this._z=e.z,this._w=e.w,this._onChangeCallback(),this}setFromEuler(e,t=!0){const i=e._x,r=e._y,s=e._z,a=e._order,o=Math.cos,l=Math.sin,c=o(i/2),p=o(r/2),u=o(s/2),d=l(i/2),h=l(r/2),_=l(s/2);switch(a){case"XYZ":this._x=d*p*u+c*h*_,this._y=c*h*u-d*p*_,this._z=c*p*_+d*h*u,this._w=c*p*u-d*h*_;break;case"YXZ":this._x=d*p*u+c*h*_,this._y=c*h*u-d*p*_,this._z=c*p*_-d*h*u,this._w=c*p*u+d*h*_;break;case"ZXY":this._x=d*p*u-c*h*_,this._y=c*h*u+d*p*_,this._z=c*p*_+d*h*u,this._w=c*p*u-d*h*_;break;case"ZYX":this._x=d*p*u-c*h*_,this._y=c*h*u+d*p*_,this._z=c*p*_-d*h*u,this._w=c*p*u+d*h*_;break;case"YZX":this._x=d*p*u+c*h*_,this._y=c*h*u+d*p*_,this._z=c*p*_-d*h*u,this._w=c*p*u-d*h*_;break;case"XZY":this._x=d*p*u-c*h*_,this._y=c*h*u-d*p*_,this._z=c*p*_+d*h*u,this._w=c*p*u+d*h*_;break;default:tt("Quaternion: .setFromEuler() encountered an unknown order: "+a)}return t===!0&&this._onChangeCallback(),this}setFromAxisAngle(e,t){const i=t/2,r=Math.sin(i);return this._x=e.x*r,this._y=e.y*r,this._z=e.z*r,this._w=Math.cos(i),this._onChangeCallback(),this}setFromRotationMatrix(e){const t=e.elements,i=t[0],r=t[4],s=t[8],a=t[1],o=t[5],l=t[9],c=t[2],p=t[6],u=t[10],d=i+o+u;if(d>0){const h=.5/Math.sqrt(d+1);this._w=.25/h,this._x=(p-l)*h,this._y=(s-c)*h,this._z=(a-r)*h}else if(i>o&&i>u){const h=2*Math.sqrt(1+i-o-u);this._w=(p-l)/h,this._x=.25*h,this._y=(r+a)/h,this._z=(s+c)/h}else if(o>u){const h=2*Math.sqrt(1+o-i-u);this._w=(s-c)/h,this._x=(r+a)/h,this._y=.25*h,this._z=(l+p)/h}else{const h=2*Math.sqrt(1+u-i-o);this._w=(a-r)/h,this._x=(s+c)/h,this._y=(l+p)/h,this._z=.25*h}return this._onChangeCallback(),this}setFromUnitVectors(e,t){let i=e.dot(t)+1;return i<1e-8?(i=0,Math.abs(e.x)>Math.abs(e.z)?(this._x=-e.y,this._y=e.x,this._z=0,this._w=i):(this._x=0,this._y=-e.z,this._z=e.y,this._w=i)):(this._x=e.y*t.z-e.z*t.y,this._y=e.z*t.x-e.x*t.z,this._z=e.x*t.y-e.y*t.x,this._w=i),this.normalize()}angleTo(e){return 2*Math.acos(Math.abs(yt(this.dot(e),-1,1)))}rotateTowards(e,t){const i=this.angleTo(e);if(i===0)return this;const r=Math.min(1,t/i);return this.slerp(e,r),this}identity(){return this.set(0,0,0,1)}invert(){return this.conjugate()}conjugate(){return this._x*=-1,this._y*=-1,this._z*=-1,this._onChangeCallback(),this}dot(e){return this._x*e._x+this._y*e._y+this._z*e._z+this._w*e._w}lengthSq(){return this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w}length(){return Math.sqrt(this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w)}normalize(){let e=this.length();return e===0?(this._x=0,this._y=0,this._z=0,this._w=1):(e=1/e,this._x=this._x*e,this._y=this._y*e,this._z=this._z*e,this._w=this._w*e),this._onChangeCallback(),this}multiply(e){return this.multiplyQuaternions(this,e)}premultiply(e){return this.multiplyQuaternions(e,this)}multiplyQuaternions(e,t){const i=e._x,r=e._y,s=e._z,a=e._w,o=t._x,l=t._y,c=t._z,p=t._w;return this._x=i*p+a*o+r*c-s*l,this._y=r*p+a*l+s*o-i*c,this._z=s*p+a*c+i*l-r*o,this._w=a*p-i*o-r*l-s*c,this._onChangeCallback(),this}slerp(e,t){let i=e._x,r=e._y,s=e._z,a=e._w,o=this.dot(e);o<0&&(i=-i,r=-r,s=-s,a=-a,o=-o);let l=1-t;if(o<.9995){const c=Math.acos(o),p=Math.sin(c);l=Math.sin(l*c)/p,t=Math.sin(t*c)/p,this._x=this._x*l+i*t,this._y=this._y*l+r*t,this._z=this._z*l+s*t,this._w=this._w*l+a*t,this._onChangeCallback()}else this._x=this._x*l+i*t,this._y=this._y*l+r*t,this._z=this._z*l+s*t,this._w=this._w*l+a*t,this.normalize();return this}slerpQuaternions(e,t,i){return this.copy(e).slerp(t,i)}random(){const e=2*Math.PI*Math.random(),t=2*Math.PI*Math.random(),i=Math.random(),r=Math.sqrt(1-i),s=Math.sqrt(i);return this.set(r*Math.sin(e),r*Math.cos(e),s*Math.sin(t),s*Math.cos(t))}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._w===this._w}fromArray(e,t=0){return this._x=e[t],this._y=e[t+1],this._z=e[t+2],this._w=e[t+3],this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._w,e}fromBufferAttribute(e,t){return this._x=e.getX(t),this._y=e.getY(t),this._z=e.getZ(t),this._w=e.getW(t),this._onChangeCallback(),this}toJSON(){return this.toArray()}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._w}}const El=class El{constructor(e=0,t=0,i=0){this.x=e,this.y=t,this.z=i}set(e,t,i){return i===void 0&&(i=this.z),this.x=e,this.y=t,this.z=i,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;default:throw new Error("THREE.Vector3: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;default:throw new Error("THREE.Vector3: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this}multiplyVectors(e,t){return this.x=e.x*t.x,this.y=e.y*t.y,this.z=e.z*t.z,this}applyEuler(e){return this.applyQuaternion(zl.setFromEuler(e))}applyAxisAngle(e,t){return this.applyQuaternion(zl.setFromAxisAngle(e,t))}applyMatrix3(e){const t=this.x,i=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[3]*i+s[6]*r,this.y=s[1]*t+s[4]*i+s[7]*r,this.z=s[2]*t+s[5]*i+s[8]*r,this}applyNormalMatrix(e){return this.applyMatrix3(e).normalize()}applyMatrix4(e){const t=this.x,i=this.y,r=this.z,s=e.elements,a=1/(s[3]*t+s[7]*i+s[11]*r+s[15]);return this.x=(s[0]*t+s[4]*i+s[8]*r+s[12])*a,this.y=(s[1]*t+s[5]*i+s[9]*r+s[13])*a,this.z=(s[2]*t+s[6]*i+s[10]*r+s[14])*a,this}applyQuaternion(e){const t=this.x,i=this.y,r=this.z,s=e.x,a=e.y,o=e.z,l=e.w,c=2*(a*r-o*i),p=2*(o*t-s*r),u=2*(s*i-a*t);return this.x=t+l*c+a*u-o*p,this.y=i+l*p+o*c-s*u,this.z=r+l*u+s*p-a*c,this}project(e){return this.applyMatrix4(e.matrixWorldInverse).applyMatrix4(e.projectionMatrix)}unproject(e){return this.applyMatrix4(e.projectionMatrixInverse).applyMatrix4(e.matrixWorld)}transformDirection(e){const t=this.x,i=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[4]*i+s[8]*r,this.y=s[1]*t+s[5]*i+s[9]*r,this.z=s[2]*t+s[6]*i+s[10]*r,this.normalize()}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this}divideScalar(e){return this.multiplyScalar(1/e)}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this}clamp(e,t){return this.x=yt(this.x,e.x,t.x),this.y=yt(this.y,e.y,t.y),this.z=yt(this.z,e.z,t.z),this}clampScalar(e,t){return this.x=yt(this.x,e,t),this.y=yt(this.y,e,t),this.z=yt(this.z,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(yt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this.z=e.z+(t.z-e.z)*i,this}cross(e){return this.crossVectors(this,e)}crossVectors(e,t){const i=e.x,r=e.y,s=e.z,a=t.x,o=t.y,l=t.z;return this.x=r*l-s*o,this.y=s*a-i*l,this.z=i*o-r*a,this}projectOnVector(e){const t=e.lengthSq();if(t===0)return this.set(0,0,0);const i=e.dot(this)/t;return this.copy(e).multiplyScalar(i)}projectOnPlane(e){return Ca.copy(this).projectOnVector(e),this.sub(Ca)}reflect(e){return this.sub(Ca.copy(e).multiplyScalar(2*this.dot(e)))}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const i=this.dot(e)/t;return Math.acos(yt(i,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,i=this.y-e.y,r=this.z-e.z;return t*t+i*i+r*r}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)+Math.abs(this.z-e.z)}setFromSpherical(e){return this.setFromSphericalCoords(e.radius,e.phi,e.theta)}setFromSphericalCoords(e,t,i){const r=Math.sin(t)*e;return this.x=r*Math.sin(i),this.y=Math.cos(t)*e,this.z=r*Math.cos(i),this}setFromCylindrical(e){return this.setFromCylindricalCoords(e.radius,e.theta,e.y)}setFromCylindricalCoords(e,t,i){return this.x=e*Math.sin(t),this.y=i,this.z=e*Math.cos(t),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this}setFromMatrixScale(e){const t=this.setFromMatrixColumn(e,0).length(),i=this.setFromMatrixColumn(e,1).length(),r=this.setFromMatrixColumn(e,2).length();return this.x=t,this.y=i,this.z=r,this}setFromMatrixColumn(e,t){return this.fromArray(e.elements,t*4)}setFromMatrix3Column(e,t){return this.fromArray(e.elements,t*3)}setFromEuler(e){return this.x=e._x,this.y=e._y,this.z=e._z,this}setFromColor(e){return this.x=e.r,this.y=e.g,this.z=e.b,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this}randomDirection(){const e=Math.random()*Math.PI*2,t=Math.random()*2-1,i=Math.sqrt(1-t*t);return this.x=i*Math.cos(e),this.y=t,this.z=i*Math.sin(e),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z}};El.prototype.isVector3=!0;let re=El;const Ca=new re,zl=new Wr,Tl=class Tl{constructor(e,t,i,r,s,a,o,l,c){this.elements=[1,0,0,0,1,0,0,0,1],e!==void 0&&this.set(e,t,i,r,s,a,o,l,c)}set(e,t,i,r,s,a,o,l,c){const p=this.elements;return p[0]=e,p[1]=r,p[2]=o,p[3]=t,p[4]=s,p[5]=l,p[6]=i,p[7]=a,p[8]=c,this}identity(){return this.set(1,0,0,0,1,0,0,0,1),this}copy(e){const t=this.elements,i=e.elements;return t[0]=i[0],t[1]=i[1],t[2]=i[2],t[3]=i[3],t[4]=i[4],t[5]=i[5],t[6]=i[6],t[7]=i[7],t[8]=i[8],this}extractBasis(e,t,i){return e.setFromMatrix3Column(this,0),t.setFromMatrix3Column(this,1),i.setFromMatrix3Column(this,2),this}setFromMatrix4(e){const t=e.elements;return this.set(t[0],t[4],t[8],t[1],t[5],t[9],t[2],t[6],t[10]),this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const i=e.elements,r=t.elements,s=this.elements,a=i[0],o=i[3],l=i[6],c=i[1],p=i[4],u=i[7],d=i[2],h=i[5],_=i[8],x=r[0],g=r[3],m=r[6],A=r[1],P=r[4],M=r[7],I=r[2],T=r[5],D=r[8];return s[0]=a*x+o*A+l*I,s[3]=a*g+o*P+l*T,s[6]=a*m+o*M+l*D,s[1]=c*x+p*A+u*I,s[4]=c*g+p*P+u*T,s[7]=c*m+p*M+u*D,s[2]=d*x+h*A+_*I,s[5]=d*g+h*P+_*T,s[8]=d*m+h*M+_*D,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[3]*=e,t[6]*=e,t[1]*=e,t[4]*=e,t[7]*=e,t[2]*=e,t[5]*=e,t[8]*=e,this}determinant(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],p=e[8];return t*a*p-t*o*c-i*s*p+i*o*l+r*s*c-r*a*l}invert(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],p=e[8],u=p*a-o*c,d=o*l-p*s,h=c*s-a*l,_=t*u+i*d+r*h;if(_===0)return this.set(0,0,0,0,0,0,0,0,0);const x=1/_;return e[0]=u*x,e[1]=(r*c-p*i)*x,e[2]=(o*i-r*a)*x,e[3]=d*x,e[4]=(p*t-r*l)*x,e[5]=(r*s-o*t)*x,e[6]=h*x,e[7]=(i*l-c*t)*x,e[8]=(a*t-i*s)*x,this}transpose(){let e;const t=this.elements;return e=t[1],t[1]=t[3],t[3]=e,e=t[2],t[2]=t[6],t[6]=e,e=t[5],t[5]=t[7],t[7]=e,this}getNormalMatrix(e){return this.setFromMatrix4(e).invert().transpose()}transposeIntoArray(e){const t=this.elements;return e[0]=t[0],e[1]=t[3],e[2]=t[6],e[3]=t[1],e[4]=t[4],e[5]=t[7],e[6]=t[2],e[7]=t[5],e[8]=t[8],this}setUvTransform(e,t,i,r,s,a,o){const l=Math.cos(s),c=Math.sin(s);return this.set(i*l,i*c,-i*(l*a+c*o)+a+e,-r*c,r*l,-r*(-c*a+l*o)+o+t,0,0,1),this}scale(e,t){return Fr("Matrix3: .scale() is deprecated. Use .makeScale() instead."),this.premultiply(Pa.makeScale(e,t)),this}rotate(e){return Fr("Matrix3: .rotate() is deprecated. Use .makeRotation() instead."),this.premultiply(Pa.makeRotation(-e)),this}translate(e,t){return Fr("Matrix3: .translate() is deprecated. Use .makeTranslation() instead."),this.premultiply(Pa.makeTranslation(e,t)),this}makeTranslation(e,t){return e.isVector2?this.set(1,0,e.x,0,1,e.y,0,0,1):this.set(1,0,e,0,1,t,0,0,1),this}makeRotation(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,-i,0,i,t,0,0,0,1),this}makeScale(e,t){return this.set(e,0,0,0,t,0,0,0,1),this}equals(e){const t=this.elements,i=e.elements;for(let r=0;r<9;r++)if(t[r]!==i[r])return!1;return!0}fromArray(e,t=0){for(let i=0;i<9;i++)this.elements[i]=e[i+t];return this}toArray(e=[],t=0){const i=this.elements;return e[t]=i[0],e[t+1]=i[1],e[t+2]=i[2],e[t+3]=i[3],e[t+4]=i[4],e[t+5]=i[5],e[t+6]=i[6],e[t+7]=i[7],e[t+8]=i[8],e}clone(){return new this.constructor().fromArray(this.elements)}};Tl.prototype.isMatrix3=!0;let ct=Tl;const Pa=new ct,Gl=new ct().set(.4123908,.3575843,.1804808,.212639,.7151687,.0721923,.0193308,.1191948,.9505322),Hl=new ct().set(3.2409699,-1.5373832,-.4986108,-.9692436,1.8759675,.0415551,.0556301,-.203977,1.0569715);function _f(){const n={enabled:!0,workingColorSpace:ds,spaces:{},convert:function(r,s,a){return this.enabled===!1||s===a||!s||!a||(this.spaces[s].transfer===It&&(r.r=bi(r.r),r.g=bi(r.g),r.b=bi(r.b)),this.spaces[s].primaries!==this.spaces[a].primaries&&(r.applyMatrix3(this.spaces[s].toXYZ),r.applyMatrix3(this.spaces[a].fromXYZ)),this.spaces[a].transfer===It&&(r.r=Or(r.r),r.g=Or(r.g),r.b=Or(r.b))),r},workingToColorSpace:function(r,s){return this.convert(r,this.workingColorSpace,s)},colorSpaceToWorking:function(r,s){return this.convert(r,s,this.workingColorSpace)},getPrimaries:function(r){return this.spaces[r].primaries},getTransfer:function(r){return r===xi?sa:this.spaces[r].transfer},getToneMappingMode:function(r){return this.spaces[r].outputColorSpaceConfig.toneMappingMode||"standard"},getLuminanceCoefficients:function(r,s=this.workingColorSpace){return r.fromArray(this.spaces[s].luminanceCoefficients)},define:function(r){Object.assign(this.spaces,r)},_getMatrix:function(r,s,a){return r.copy(this.spaces[s].toXYZ).multiply(this.spaces[a].fromXYZ)},_getDrawingBufferColorSpace:function(r){return this.spaces[r].outputColorSpaceConfig.drawingBufferColorSpace},_getUnpackColorSpace:function(r=this.workingColorSpace){return this.spaces[r].workingColorSpaceConfig.unpackColorSpace},fromWorkingColorSpace:function(r,s){return Fr("ColorManagement: .fromWorkingColorSpace() has been renamed to .workingToColorSpace()."),n.workingToColorSpace(r,s)},toWorkingColorSpace:function(r,s){return Fr("ColorManagement: .toWorkingColorSpace() has been renamed to .colorSpaceToWorking()."),n.colorSpaceToWorking(r,s)}},e=[.64,.33,.3,.6,.15,.06],t=[.2126,.7152,.0722],i=[.3127,.329];return n.define({[ds]:{primaries:e,whitePoint:i,transfer:sa,toXYZ:Gl,fromXYZ:Hl,luminanceCoefficients:t,workingColorSpaceConfig:{unpackColorSpace:Rn},outputColorSpaceConfig:{drawingBufferColorSpace:Rn}},[Rn]:{primaries:e,whitePoint:i,transfer:It,toXYZ:Gl,fromXYZ:Hl,luminanceCoefficients:t,outputColorSpaceConfig:{drawingBufferColorSpace:Rn}}}),n}const xt=_f();function bi(n){return n<.04045?n*.0773993808:Math.pow(n*.9478672986+.0521327014,2.4)}function Or(n){return n<.0031308?n*12.92:1.055*Math.pow(n,.41666)-.055}let Sr;class vf{static getDataURL(e,t="image/png"){if(/^data:/i.test(e.src)||typeof HTMLCanvasElement>"u")return e.src;let i;if(e instanceof HTMLCanvasElement)i=e;else{Sr===void 0&&(Sr=oa("canvas")),Sr.width=e.width,Sr.height=e.height;const r=Sr.getContext("2d");e instanceof ImageData?r.putImageData(e,0,0):r.drawImage(e,0,0,e.width,e.height),i=Sr}return i.toDataURL(t)}static sRGBToLinear(e){if(typeof HTMLImageElement<"u"&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&e instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&e instanceof ImageBitmap){const t=oa("canvas");t.width=e.width,t.height=e.height;const i=t.getContext("2d");i.drawImage(e,0,0,e.width,e.height);const r=i.getImageData(0,0,e.width,e.height),s=r.data;for(let a=0;a<s.length;a++)s[a]=bi(s[a]/255)*255;return i.putImageData(r,0,0),t}else if(e.data){const t=e.data.slice(0);for(let i=0;i<t.length;i++)t instanceof Uint8Array||t instanceof Uint8ClampedArray?t[i]=Math.floor(bi(t[i]/255)*255):t[i]=bi(t[i]);return{data:t,width:e.width,height:e.height}}else return tt("ImageUtils.sRGBToLinear(): Unsupported image type. No color space conversion applied."),e}}let xf=0;class gl{constructor(e=null){this.isSource=!0,Object.defineProperty(this,"id",{value:xf++}),this.uuid=zi(),this.data=e,this.dataReady=!0,this.version=0}getSize(e){const t=this.data;return typeof HTMLVideoElement<"u"&&t instanceof HTMLVideoElement?e.set(t.videoWidth,t.videoHeight,0):typeof VideoFrame<"u"&&t instanceof VideoFrame?e.set(t.displayWidth,t.displayHeight,0):t!==null?e.set(t.width,t.height,t.depth||0):e.set(0,0,0),e}set needsUpdate(e){e===!0&&this.version++}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.images[this.uuid]!==void 0)return e.images[this.uuid];const i={uuid:this.uuid,url:""},r=this.data;if(r!==null){let s;if(Array.isArray(r)){s=[];for(let a=0,o=r.length;a<o;a++)r[a].isDataTexture?s.push(La(r[a].image)):s.push(La(r[a]))}else s=La(r);i.url=s}return t||(e.images[this.uuid]=i),i}}function La(n){return typeof HTMLImageElement<"u"&&n instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&n instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&n instanceof ImageBitmap?vf.getDataURL(n):n.data?{data:Array.from(n.data),width:n.width,height:n.height,type:n.data.constructor.name}:(tt("Texture: Unable to serialize Texture."),{})}let yf=0;const Ia=new re;class cn extends ur{constructor(e=cn.DEFAULT_IMAGE,t=cn.DEFAULT_MAPPING,i=Si,r=Si,s=sn,a=rr,o=Bn,l=Cn,c=cn.DEFAULT_ANISOTROPY,p=xi){super(),this.isTexture=!0,Object.defineProperty(this,"id",{value:yf++}),this.uuid=zi(),this.name="",this.source=new gl(e),this.mipmaps=[],this.mapping=t,this.channel=0,this.wrapS=i,this.wrapT=r,this.magFilter=s,this.minFilter=a,this.anisotropy=c,this.format=o,this.internalFormat=null,this.type=l,this.offset=new Tt(0,0),this.repeat=new Tt(1,1),this.center=new Tt(0,0),this.rotation=0,this.matrixAutoUpdate=!0,this.matrix=new ct,this.generateMipmaps=!0,this.premultiplyAlpha=!1,this.flipY=!0,this.unpackAlignment=4,this.colorSpace=p,this.userData={},this.updateRanges=[],this.version=0,this.onUpdate=null,this.renderTarget=null,this.isRenderTargetTexture=!1,this.isArrayTexture=!!(e&&e.depth&&e.depth>1),this.pmremVersion=0,this.normalized=!1}get width(){return this.source.getSize(Ia).x}get height(){return this.source.getSize(Ia).y}get depth(){return this.source.getSize(Ia).z}get image(){return this.source.data}set image(e){this.source.data=e}updateMatrix(){this.matrix.setUvTransform(this.offset.x,this.offset.y,this.repeat.x,this.repeat.y,this.rotation,this.center.x,this.center.y)}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}clone(){return new this.constructor().copy(this)}copy(e){return this.name=e.name,this.source=e.source,this.mipmaps=e.mipmaps.slice(0),this.mapping=e.mapping,this.channel=e.channel,this.wrapS=e.wrapS,this.wrapT=e.wrapT,this.magFilter=e.magFilter,this.minFilter=e.minFilter,this.anisotropy=e.anisotropy,this.format=e.format,this.internalFormat=e.internalFormat,this.type=e.type,this.normalized=e.normalized,this.offset.copy(e.offset),this.repeat.copy(e.repeat),this.center.copy(e.center),this.rotation=e.rotation,this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrix.copy(e.matrix),this.generateMipmaps=e.generateMipmaps,this.premultiplyAlpha=e.premultiplyAlpha,this.flipY=e.flipY,this.unpackAlignment=e.unpackAlignment,this.colorSpace=e.colorSpace,this.renderTarget=e.renderTarget,this.isRenderTargetTexture=e.isRenderTargetTexture,this.isArrayTexture=e.isArrayTexture,this.userData=JSON.parse(JSON.stringify(e.userData)),this.needsUpdate=!0,this}setValues(e){for(const t in e){const i=e[t];if(i===void 0){tt(`Texture.setValues(): parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){tt(`Texture.setValues(): property '${t}' does not exist.`);continue}r&&i&&r.isVector2&&i.isVector2||r&&i&&r.isVector3&&i.isVector3||r&&i&&r.isMatrix3&&i.isMatrix3?r.copy(i):this[t]=i}}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.textures[this.uuid]!==void 0)return e.textures[this.uuid];const i={metadata:{version:4.7,type:"Texture",generator:"Texture.toJSON"},uuid:this.uuid,name:this.name,image:this.source.toJSON(e).uuid,mapping:this.mapping,channel:this.channel,repeat:[this.repeat.x,this.repeat.y],offset:[this.offset.x,this.offset.y],center:[this.center.x,this.center.y],rotation:this.rotation,wrap:[this.wrapS,this.wrapT],format:this.format,internalFormat:this.internalFormat,type:this.type,normalized:this.normalized,colorSpace:this.colorSpace,minFilter:this.minFilter,magFilter:this.magFilter,anisotropy:this.anisotropy,flipY:this.flipY,generateMipmaps:this.generateMipmaps,premultiplyAlpha:this.premultiplyAlpha,unpackAlignment:this.unpackAlignment};return Object.keys(this.userData).length>0&&(i.userData=this.userData),t||(e.textures[this.uuid]=i),i}dispose(){this.dispatchEvent({type:"dispose"})}transformUv(e){if(this.mapping!==ad)return e;if(e.applyMatrix3(this.matrix),e.x<0||e.x>1)switch(this.wrapS){case go:e.x=e.x-Math.floor(e.x);break;case Si:e.x=e.x<0?0:1;break;case _o:Math.abs(Math.floor(e.x)%2)===1?e.x=Math.ceil(e.x)-e.x:e.x=e.x-Math.floor(e.x);break}if(e.y<0||e.y>1)switch(this.wrapT){case go:e.y=e.y-Math.floor(e.y);break;case Si:e.y=e.y<0?0:1;break;case _o:Math.abs(Math.floor(e.y)%2)===1?e.y=Math.ceil(e.y)-e.y:e.y=e.y-Math.floor(e.y);break}return this.flipY&&(e.y=1-e.y),e}set needsUpdate(e){e===!0&&(this.version++,this.source.needsUpdate=!0)}set needsPMREMUpdate(e){e===!0&&this.pmremVersion++}}cn.DEFAULT_IMAGE=null;cn.DEFAULT_MAPPING=ad;cn.DEFAULT_ANISOTROPY=1;const wl=class wl{constructor(e=0,t=0,i=0,r=1){this.x=e,this.y=t,this.z=i,this.w=r}get width(){return this.z}set width(e){this.z=e}get height(){return this.w}set height(e){this.w=e}set(e,t,i,r){return this.x=e,this.y=t,this.z=i,this.w=r,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this.w=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setW(e){return this.w=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;case 3:this.w=t;break;default:throw new Error("THREE.Vector4: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;case 3:return this.w;default:throw new Error("THREE.Vector4: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z,this.w)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this.w=e.w!==void 0?e.w:1,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this.w+=e.w,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this.w+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this.w=e.w+t.w,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this.w+=e.w*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this.w-=e.w,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this.w-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this.w=e.w-t.w,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this.w*=e.w,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this.w*=e,this}applyMatrix4(e){const t=this.x,i=this.y,r=this.z,s=this.w,a=e.elements;return this.x=a[0]*t+a[4]*i+a[8]*r+a[12]*s,this.y=a[1]*t+a[5]*i+a[9]*r+a[13]*s,this.z=a[2]*t+a[6]*i+a[10]*r+a[14]*s,this.w=a[3]*t+a[7]*i+a[11]*r+a[15]*s,this}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this.w/=e.w,this}divideScalar(e){return this.multiplyScalar(1/e)}setAxisAngleFromQuaternion(e){this.w=2*Math.acos(e.w);const t=Math.sqrt(1-e.w*e.w);return t<1e-4?(this.x=1,this.y=0,this.z=0):(this.x=e.x/t,this.y=e.y/t,this.z=e.z/t),this}setAxisAngleFromRotationMatrix(e){let t,i,r,s;const l=e.elements,c=l[0],p=l[4],u=l[8],d=l[1],h=l[5],_=l[9],x=l[2],g=l[6],m=l[10];if(Math.abs(p-d)<.01&&Math.abs(u-x)<.01&&Math.abs(_-g)<.01){if(Math.abs(p+d)<.1&&Math.abs(u+x)<.1&&Math.abs(_+g)<.1&&Math.abs(c+h+m-3)<.1)return this.set(1,0,0,0),this;t=Math.PI;const P=(c+1)/2,M=(h+1)/2,I=(m+1)/2,T=(p+d)/4,D=(u+x)/4,v=(_+g)/4;return P>M&&P>I?P<.01?(i=0,r=.707106781,s=.707106781):(i=Math.sqrt(P),r=T/i,s=D/i):M>I?M<.01?(i=.707106781,r=0,s=.707106781):(r=Math.sqrt(M),i=T/r,s=v/r):I<.01?(i=.707106781,r=.707106781,s=0):(s=Math.sqrt(I),i=D/s,r=v/s),this.set(i,r,s,t),this}let A=Math.sqrt((g-_)*(g-_)+(u-x)*(u-x)+(d-p)*(d-p));return Math.abs(A)<.001&&(A=1),this.x=(g-_)/A,this.y=(u-x)/A,this.z=(d-p)/A,this.w=Math.acos((c+h+m-1)/2),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this.w=t[15],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this.w=Math.min(this.w,e.w),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this.w=Math.max(this.w,e.w),this}clamp(e,t){return this.x=yt(this.x,e.x,t.x),this.y=yt(this.y,e.y,t.y),this.z=yt(this.z,e.z,t.z),this.w=yt(this.w,e.w,t.w),this}clampScalar(e,t){return this.x=yt(this.x,e,t),this.y=yt(this.y,e,t),this.z=yt(this.z,e,t),this.w=yt(this.w,e,t),this}clampLength(e,t){const i=this.length();return this.divideScalar(i||1).multiplyScalar(yt(i,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this.w=Math.floor(this.w),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this.w=Math.ceil(this.w),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this.w=Math.round(this.w),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this.w=Math.trunc(this.w),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this.w=-this.w,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z+this.w*e.w}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)+Math.abs(this.w)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this.w+=(e.w-this.w)*t,this}lerpVectors(e,t,i){return this.x=e.x+(t.x-e.x)*i,this.y=e.y+(t.y-e.y)*i,this.z=e.z+(t.z-e.z)*i,this.w=e.w+(t.w-e.w)*i,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z&&e.w===this.w}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this.w=e[t+3],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e[t+3]=this.w,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this.w=e.getW(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this.w=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z,yield this.w}};wl.prototype.isVector4=!0;let Vt=wl;class Sf extends ur{constructor(e=1,t=1,i={}){super(),i=Object.assign({generateMipmaps:!1,internalFormat:null,minFilter:sn,depthBuffer:!0,stencilBuffer:!1,resolveDepthBuffer:!0,resolveStencilBuffer:!0,depthTexture:null,samples:0,count:1,depth:1,multiview:!1,useArrayDepthTexture:!1},i),this.isRenderTarget=!0,this.width=e,this.height=t,this.depth=i.depth,this.scissor=new Vt(0,0,e,t),this.scissorTest=!1,this.viewport=new Vt(0,0,e,t),this.textures=[];const r={width:e,height:t,depth:i.depth},s=new cn(r),a=i.count;for(let o=0;o<a;o++)this.textures[o]=s.clone(),this.textures[o].isRenderTargetTexture=!0,this.textures[o].renderTarget=this;this._setTextureOptions(i),this.depthBuffer=i.depthBuffer,this.stencilBuffer=i.stencilBuffer,this.resolveDepthBuffer=i.resolveDepthBuffer,this.resolveStencilBuffer=i.resolveStencilBuffer,this._depthTexture=null,this.depthTexture=i.depthTexture,this.samples=i.samples,this.multiview=i.multiview,this.useArrayDepthTexture=i.useArrayDepthTexture}_setTextureOptions(e={}){const t={minFilter:sn,generateMipmaps:!1,flipY:!1,internalFormat:null};e.mapping!==void 0&&(t.mapping=e.mapping),e.wrapS!==void 0&&(t.wrapS=e.wrapS),e.wrapT!==void 0&&(t.wrapT=e.wrapT),e.wrapR!==void 0&&(t.wrapR=e.wrapR),e.magFilter!==void 0&&(t.magFilter=e.magFilter),e.minFilter!==void 0&&(t.minFilter=e.minFilter),e.format!==void 0&&(t.format=e.format),e.type!==void 0&&(t.type=e.type),e.anisotropy!==void 0&&(t.anisotropy=e.anisotropy),e.colorSpace!==void 0&&(t.colorSpace=e.colorSpace),e.flipY!==void 0&&(t.flipY=e.flipY),e.generateMipmaps!==void 0&&(t.generateMipmaps=e.generateMipmaps),e.internalFormat!==void 0&&(t.internalFormat=e.internalFormat);for(let i=0;i<this.textures.length;i++)this.textures[i].setValues(t)}get texture(){return this.textures[0]}set texture(e){this.textures[0]=e}set depthTexture(e){this._depthTexture!==null&&(this._depthTexture.renderTarget=null),e!==null&&(e.renderTarget=this),this._depthTexture=e}get depthTexture(){return this._depthTexture}setSize(e,t,i=1){if(this.width!==e||this.height!==t||this.depth!==i){this.width=e,this.height=t,this.depth=i;for(let r=0,s=this.textures.length;r<s;r++)this.textures[r].image.width=e,this.textures[r].image.height=t,this.textures[r].image.depth=i,this.textures[r].isData3DTexture!==!0&&(this.textures[r].isArrayTexture=this.textures[r].image.depth>1);this.dispose()}this.viewport.set(0,0,e,t),this.scissor.set(0,0,e,t)}clone(){return new this.constructor().copy(this)}copy(e){this.width=e.width,this.height=e.height,this.depth=e.depth,this.scissor.copy(e.scissor),this.scissorTest=e.scissorTest,this.viewport.copy(e.viewport),this.textures.length=0;for(let t=0,i=e.textures.length;t<i;t++){this.textures[t]=e.textures[t].clone(),this.textures[t].isRenderTargetTexture=!0,this.textures[t].renderTarget=this;const r=Object.assign({},e.textures[t].image);this.textures[t].source=new gl(r)}return this.depthBuffer=e.depthBuffer,this.stencilBuffer=e.stencilBuffer,this.resolveDepthBuffer=e.resolveDepthBuffer,this.resolveStencilBuffer=e.resolveStencilBuffer,e.depthTexture!==null&&(this.depthTexture=e.depthTexture.clone()),this.samples=e.samples,this.multiview=e.multiview,this.useArrayDepthTexture=e.useArrayDepthTexture,this}dispose(){this.dispatchEvent({type:"dispose"})}}class ni extends Sf{constructor(e=1,t=1,i={}){super(e,t,i),this.isWebGLRenderTarget=!0}}class md extends cn{constructor(e=null,t=1,i=1,r=1){super(null),this.isDataArrayTexture=!0,this.image={data:e,width:t,height:i,depth:r},this.magFilter=rn,this.minFilter=rn,this.wrapR=Si,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1,this.layerUpdates=new Set}addLayerUpdate(e){this.layerUpdates.add(e)}clearLayerUpdates(){this.layerUpdates.clear()}}class Mf extends cn{constructor(e=null,t=1,i=1,r=1){super(null),this.isData3DTexture=!0,this.image={data:e,width:t,height:i,depth:r},this.magFilter=rn,this.minFilter=rn,this.wrapR=Si,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const ua=class ua{constructor(e,t,i,r,s,a,o,l,c,p,u,d,h,_,x,g){this.elements=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],e!==void 0&&this.set(e,t,i,r,s,a,o,l,c,p,u,d,h,_,x,g)}set(e,t,i,r,s,a,o,l,c,p,u,d,h,_,x,g){const m=this.elements;return m[0]=e,m[4]=t,m[8]=i,m[12]=r,m[1]=s,m[5]=a,m[9]=o,m[13]=l,m[2]=c,m[6]=p,m[10]=u,m[14]=d,m[3]=h,m[7]=_,m[11]=x,m[15]=g,this}identity(){return this.set(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1),this}clone(){return new ua().fromArray(this.elements)}copy(e){const t=this.elements,i=e.elements;return t[0]=i[0],t[1]=i[1],t[2]=i[2],t[3]=i[3],t[4]=i[4],t[5]=i[5],t[6]=i[6],t[7]=i[7],t[8]=i[8],t[9]=i[9],t[10]=i[10],t[11]=i[11],t[12]=i[12],t[13]=i[13],t[14]=i[14],t[15]=i[15],this}copyPosition(e){const t=this.elements,i=e.elements;return t[12]=i[12],t[13]=i[13],t[14]=i[14],this}setFromMatrix3(e){const t=e.elements;return this.set(t[0],t[3],t[6],0,t[1],t[4],t[7],0,t[2],t[5],t[8],0,0,0,0,1),this}extractBasis(e,t,i){return this.determinantAffine()===0?(e.set(1,0,0),t.set(0,1,0),i.set(0,0,1),this):(e.setFromMatrixColumn(this,0),t.setFromMatrixColumn(this,1),i.setFromMatrixColumn(this,2),this)}makeBasis(e,t,i){return this.set(e.x,t.x,i.x,0,e.y,t.y,i.y,0,e.z,t.z,i.z,0,0,0,0,1),this}extractRotation(e){if(e.determinantAffine()===0)return this.identity();const t=this.elements,i=e.elements,r=1/Mr.setFromMatrixColumn(e,0).length(),s=1/Mr.setFromMatrixColumn(e,1).length(),a=1/Mr.setFromMatrixColumn(e,2).length();return t[0]=i[0]*r,t[1]=i[1]*r,t[2]=i[2]*r,t[3]=0,t[4]=i[4]*s,t[5]=i[5]*s,t[6]=i[6]*s,t[7]=0,t[8]=i[8]*a,t[9]=i[9]*a,t[10]=i[10]*a,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromEuler(e){const t=this.elements,i=e.x,r=e.y,s=e.z,a=Math.cos(i),o=Math.sin(i),l=Math.cos(r),c=Math.sin(r),p=Math.cos(s),u=Math.sin(s);if(e.order==="XYZ"){const d=a*p,h=a*u,_=o*p,x=o*u;t[0]=l*p,t[4]=-l*u,t[8]=c,t[1]=h+_*c,t[5]=d-x*c,t[9]=-o*l,t[2]=x-d*c,t[6]=_+h*c,t[10]=a*l}else if(e.order==="YXZ"){const d=l*p,h=l*u,_=c*p,x=c*u;t[0]=d+x*o,t[4]=_*o-h,t[8]=a*c,t[1]=a*u,t[5]=a*p,t[9]=-o,t[2]=h*o-_,t[6]=x+d*o,t[10]=a*l}else if(e.order==="ZXY"){const d=l*p,h=l*u,_=c*p,x=c*u;t[0]=d-x*o,t[4]=-a*u,t[8]=_+h*o,t[1]=h+_*o,t[5]=a*p,t[9]=x-d*o,t[2]=-a*c,t[6]=o,t[10]=a*l}else if(e.order==="ZYX"){const d=a*p,h=a*u,_=o*p,x=o*u;t[0]=l*p,t[4]=_*c-h,t[8]=d*c+x,t[1]=l*u,t[5]=x*c+d,t[9]=h*c-_,t[2]=-c,t[6]=o*l,t[10]=a*l}else if(e.order==="YZX"){const d=a*l,h=a*c,_=o*l,x=o*c;t[0]=l*p,t[4]=x-d*u,t[8]=_*u+h,t[1]=u,t[5]=a*p,t[9]=-o*p,t[2]=-c*p,t[6]=h*u+_,t[10]=d-x*u}else if(e.order==="XZY"){const d=a*l,h=a*c,_=o*l,x=o*c;t[0]=l*p,t[4]=-u,t[8]=c*p,t[1]=d*u+x,t[5]=a*p,t[9]=h*u-_,t[2]=_*u-h,t[6]=o*p,t[10]=x*u+d}return t[3]=0,t[7]=0,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromQuaternion(e){return this.compose(bf,e,Ef)}lookAt(e,t,i){const r=this.elements;return Sn.subVectors(e,t),Sn.lengthSq()===0&&(Sn.z=1),Sn.normalize(),Li.crossVectors(i,Sn),Li.lengthSq()===0&&(Math.abs(i.z)===1?Sn.x+=1e-4:Sn.z+=1e-4,Sn.normalize(),Li.crossVectors(i,Sn)),Li.normalize(),Ss.crossVectors(Sn,Li),r[0]=Li.x,r[4]=Ss.x,r[8]=Sn.x,r[1]=Li.y,r[5]=Ss.y,r[9]=Sn.y,r[2]=Li.z,r[6]=Ss.z,r[10]=Sn.z,this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const i=e.elements,r=t.elements,s=this.elements,a=i[0],o=i[4],l=i[8],c=i[12],p=i[1],u=i[5],d=i[9],h=i[13],_=i[2],x=i[6],g=i[10],m=i[14],A=i[3],P=i[7],M=i[11],I=i[15],T=r[0],D=r[4],v=r[8],S=r[12],w=r[1],L=r[5],V=r[9],Z=r[13],k=r[2],z=r[6],C=r[10],R=r[14],G=r[3],Y=r[7],ne=r[11],de=r[15];return s[0]=a*T+o*w+l*k+c*G,s[4]=a*D+o*L+l*z+c*Y,s[8]=a*v+o*V+l*C+c*ne,s[12]=a*S+o*Z+l*R+c*de,s[1]=p*T+u*w+d*k+h*G,s[5]=p*D+u*L+d*z+h*Y,s[9]=p*v+u*V+d*C+h*ne,s[13]=p*S+u*Z+d*R+h*de,s[2]=_*T+x*w+g*k+m*G,s[6]=_*D+x*L+g*z+m*Y,s[10]=_*v+x*V+g*C+m*ne,s[14]=_*S+x*Z+g*R+m*de,s[3]=A*T+P*w+M*k+I*G,s[7]=A*D+P*L+M*z+I*Y,s[11]=A*v+P*V+M*C+I*ne,s[15]=A*S+P*Z+M*R+I*de,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[4]*=e,t[8]*=e,t[12]*=e,t[1]*=e,t[5]*=e,t[9]*=e,t[13]*=e,t[2]*=e,t[6]*=e,t[10]*=e,t[14]*=e,t[3]*=e,t[7]*=e,t[11]*=e,t[15]*=e,this}determinant(){const e=this.elements,t=e[0],i=e[4],r=e[8],s=e[12],a=e[1],o=e[5],l=e[9],c=e[13],p=e[2],u=e[6],d=e[10],h=e[14],_=e[3],x=e[7],g=e[11],m=e[15],A=l*h-c*d,P=o*h-c*u,M=o*d-l*u,I=a*h-c*p,T=a*d-l*p,D=a*u-o*p;return t*(x*A-g*P+m*M)-i*(_*A-g*I+m*T)+r*(_*P-x*I+m*D)-s*(_*M-x*T+g*D)}determinantAffine(){const e=this.elements,t=e[0],i=e[4],r=e[8],s=e[1],a=e[5],o=e[9],l=e[2],c=e[6],p=e[10];return t*(a*p-o*c)-i*(s*p-o*l)+r*(s*c-a*l)}transpose(){const e=this.elements;let t;return t=e[1],e[1]=e[4],e[4]=t,t=e[2],e[2]=e[8],e[8]=t,t=e[6],e[6]=e[9],e[9]=t,t=e[3],e[3]=e[12],e[12]=t,t=e[7],e[7]=e[13],e[13]=t,t=e[11],e[11]=e[14],e[14]=t,this}setPosition(e,t,i){const r=this.elements;return e.isVector3?(r[12]=e.x,r[13]=e.y,r[14]=e.z):(r[12]=e,r[13]=t,r[14]=i),this}invert(){const e=this.elements,t=e[0],i=e[1],r=e[2],s=e[3],a=e[4],o=e[5],l=e[6],c=e[7],p=e[8],u=e[9],d=e[10],h=e[11],_=e[12],x=e[13],g=e[14],m=e[15],A=t*o-i*a,P=t*l-r*a,M=t*c-s*a,I=i*l-r*o,T=i*c-s*o,D=r*c-s*l,v=p*x-u*_,S=p*g-d*_,w=p*m-h*_,L=u*g-d*x,V=u*m-h*x,Z=d*m-h*g,k=A*Z-P*V+M*L+I*w-T*S+D*v;if(k===0)return this.set(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);const z=1/k;return e[0]=(o*Z-l*V+c*L)*z,e[1]=(r*V-i*Z-s*L)*z,e[2]=(x*D-g*T+m*I)*z,e[3]=(d*T-u*D-h*I)*z,e[4]=(l*w-a*Z-c*S)*z,e[5]=(t*Z-r*w+s*S)*z,e[6]=(g*M-_*D-m*P)*z,e[7]=(p*D-d*M+h*P)*z,e[8]=(a*V-o*w+c*v)*z,e[9]=(i*w-t*V-s*v)*z,e[10]=(_*T-x*M+m*A)*z,e[11]=(u*M-p*T-h*A)*z,e[12]=(o*S-a*L-l*v)*z,e[13]=(t*L-i*S+r*v)*z,e[14]=(x*P-_*I-g*A)*z,e[15]=(p*I-u*P+d*A)*z,this}scale(e){const t=this.elements,i=e.x,r=e.y,s=e.z;return t[0]*=i,t[4]*=r,t[8]*=s,t[1]*=i,t[5]*=r,t[9]*=s,t[2]*=i,t[6]*=r,t[10]*=s,t[3]*=i,t[7]*=r,t[11]*=s,this}getMaxScaleOnAxis(){const e=this.elements,t=e[0]*e[0]+e[1]*e[1]+e[2]*e[2],i=e[4]*e[4]+e[5]*e[5]+e[6]*e[6],r=e[8]*e[8]+e[9]*e[9]+e[10]*e[10];return Math.sqrt(Math.max(t,i,r))}makeTranslation(e,t,i){return e.isVector3?this.set(1,0,0,e.x,0,1,0,e.y,0,0,1,e.z,0,0,0,1):this.set(1,0,0,e,0,1,0,t,0,0,1,i,0,0,0,1),this}makeRotationX(e){const t=Math.cos(e),i=Math.sin(e);return this.set(1,0,0,0,0,t,-i,0,0,i,t,0,0,0,0,1),this}makeRotationY(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,0,i,0,0,1,0,0,-i,0,t,0,0,0,0,1),this}makeRotationZ(e){const t=Math.cos(e),i=Math.sin(e);return this.set(t,-i,0,0,i,t,0,0,0,0,1,0,0,0,0,1),this}makeRotationAxis(e,t){const i=Math.cos(t),r=Math.sin(t),s=1-i,a=e.x,o=e.y,l=e.z,c=s*a,p=s*o;return this.set(c*a+i,c*o-r*l,c*l+r*o,0,c*o+r*l,p*o+i,p*l-r*a,0,c*l-r*o,p*l+r*a,s*l*l+i,0,0,0,0,1),this}makeScale(e,t,i){return this.set(e,0,0,0,0,t,0,0,0,0,i,0,0,0,0,1),this}makeShear(e,t,i,r,s,a){return this.set(1,i,s,0,e,1,a,0,t,r,1,0,0,0,0,1),this}compose(e,t,i){const r=this.elements,s=t._x,a=t._y,o=t._z,l=t._w,c=s+s,p=a+a,u=o+o,d=s*c,h=s*p,_=s*u,x=a*p,g=a*u,m=o*u,A=l*c,P=l*p,M=l*u,I=i.x,T=i.y,D=i.z;return r[0]=(1-(x+m))*I,r[1]=(h+M)*I,r[2]=(_-P)*I,r[3]=0,r[4]=(h-M)*T,r[5]=(1-(d+m))*T,r[6]=(g+A)*T,r[7]=0,r[8]=(_+P)*D,r[9]=(g-A)*D,r[10]=(1-(d+x))*D,r[11]=0,r[12]=e.x,r[13]=e.y,r[14]=e.z,r[15]=1,this}decompose(e,t,i){const r=this.elements;e.x=r[12],e.y=r[13],e.z=r[14];const s=this.determinantAffine();if(s===0)return i.set(1,1,1),t.identity(),this;let a=Mr.set(r[0],r[1],r[2]).length();const o=Mr.set(r[4],r[5],r[6]).length(),l=Mr.set(r[8],r[9],r[10]).length();s<0&&(a=-a),Dn.copy(this);const c=1/a,p=1/o,u=1/l;return Dn.elements[0]*=c,Dn.elements[1]*=c,Dn.elements[2]*=c,Dn.elements[4]*=p,Dn.elements[5]*=p,Dn.elements[6]*=p,Dn.elements[8]*=u,Dn.elements[9]*=u,Dn.elements[10]*=u,t.setFromRotationMatrix(Dn),i.x=a,i.y=o,i.z=l,this}makePerspective(e,t,i,r,s,a,o=ti,l=!1){const c=this.elements,p=2*s/(t-e),u=2*s/(i-r),d=(t+e)/(t-e),h=(i+r)/(i-r);let _,x;if(l)_=s/(a-s),x=a*s/(a-s);else if(o===ti)_=-(a+s)/(a-s),x=-2*a*s/(a-s);else if(o===aa)_=-a/(a-s),x=-a*s/(a-s);else throw new Error("THREE.Matrix4.makePerspective(): Invalid coordinate system: "+o);return c[0]=p,c[4]=0,c[8]=d,c[12]=0,c[1]=0,c[5]=u,c[9]=h,c[13]=0,c[2]=0,c[6]=0,c[10]=_,c[14]=x,c[3]=0,c[7]=0,c[11]=-1,c[15]=0,this}makeOrthographic(e,t,i,r,s,a,o=ti,l=!1){const c=this.elements,p=2/(t-e),u=2/(i-r),d=-(t+e)/(t-e),h=-(i+r)/(i-r);let _,x;if(l)_=1/(a-s),x=a/(a-s);else if(o===ti)_=-2/(a-s),x=-(a+s)/(a-s);else if(o===aa)_=-1/(a-s),x=-s/(a-s);else throw new Error("THREE.Matrix4.makeOrthographic(): Invalid coordinate system: "+o);return c[0]=p,c[4]=0,c[8]=0,c[12]=d,c[1]=0,c[5]=u,c[9]=0,c[13]=h,c[2]=0,c[6]=0,c[10]=_,c[14]=x,c[3]=0,c[7]=0,c[11]=0,c[15]=1,this}equals(e){const t=this.elements,i=e.elements;for(let r=0;r<16;r++)if(t[r]!==i[r])return!1;return!0}fromArray(e,t=0){for(let i=0;i<16;i++)this.elements[i]=e[i+t];return this}toArray(e=[],t=0){const i=this.elements;return e[t]=i[0],e[t+1]=i[1],e[t+2]=i[2],e[t+3]=i[3],e[t+4]=i[4],e[t+5]=i[5],e[t+6]=i[6],e[t+7]=i[7],e[t+8]=i[8],e[t+9]=i[9],e[t+10]=i[10],e[t+11]=i[11],e[t+12]=i[12],e[t+13]=i[13],e[t+14]=i[14],e[t+15]=i[15],e}};ua.prototype.isMatrix4=!0;let Kt=ua;const Mr=new re,Dn=new Kt,bf=new re(0,0,0),Ef=new re(1,1,1),Li=new re,Ss=new re,Sn=new re,Vl=new Kt,Wl=new Wr;class dr{constructor(e=0,t=0,i=0,r=dr.DEFAULT_ORDER){this.isEuler=!0,this._x=e,this._y=t,this._z=i,this._order=r}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get order(){return this._order}set order(e){this._order=e,this._onChangeCallback()}set(e,t,i,r=this._order){return this._x=e,this._y=t,this._z=i,this._order=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._order)}copy(e){return this._x=e._x,this._y=e._y,this._z=e._z,this._order=e._order,this._onChangeCallback(),this}setFromRotationMatrix(e,t=this._order,i=!0){const r=e.elements,s=r[0],a=r[4],o=r[8],l=r[1],c=r[5],p=r[9],u=r[2],d=r[6],h=r[10];switch(t){case"XYZ":this._y=Math.asin(yt(o,-1,1)),Math.abs(o)<.9999999?(this._x=Math.atan2(-p,h),this._z=Math.atan2(-a,s)):(this._x=Math.atan2(d,c),this._z=0);break;case"YXZ":this._x=Math.asin(-yt(p,-1,1)),Math.abs(p)<.9999999?(this._y=Math.atan2(o,h),this._z=Math.atan2(l,c)):(this._y=Math.atan2(-u,s),this._z=0);break;case"ZXY":this._x=Math.asin(yt(d,-1,1)),Math.abs(d)<.9999999?(this._y=Math.atan2(-u,h),this._z=Math.atan2(-a,c)):(this._y=0,this._z=Math.atan2(l,s));break;case"ZYX":this._y=Math.asin(-yt(u,-1,1)),Math.abs(u)<.9999999?(this._x=Math.atan2(d,h),this._z=Math.atan2(l,s)):(this._x=0,this._z=Math.atan2(-a,c));break;case"YZX":this._z=Math.asin(yt(l,-1,1)),Math.abs(l)<.9999999?(this._x=Math.atan2(-p,c),this._y=Math.atan2(-u,s)):(this._x=0,this._y=Math.atan2(o,h));break;case"XZY":this._z=Math.asin(-yt(a,-1,1)),Math.abs(a)<.9999999?(this._x=Math.atan2(d,c),this._y=Math.atan2(o,s)):(this._x=Math.atan2(-p,h),this._y=0);break;default:tt("Euler: .setFromRotationMatrix() encountered an unknown order: "+t)}return this._order=t,i===!0&&this._onChangeCallback(),this}setFromQuaternion(e,t,i){return Vl.makeRotationFromQuaternion(e),this.setFromRotationMatrix(Vl,t,i)}setFromVector3(e,t=this._order){return this.set(e.x,e.y,e.z,t)}reorder(e){return Wl.setFromEuler(this),this.setFromQuaternion(Wl,e)}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._order===this._order}fromArray(e){return this._x=e[0],this._y=e[1],this._z=e[2],e[3]!==void 0&&(this._order=e[3]),this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._order,e}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._order}}dr.DEFAULT_ORDER="XYZ";class gd{constructor(){this.mask=1}set(e){this.mask=(1<<e|0)>>>0}enable(e){this.mask|=1<<e|0}enableAll(){this.mask=-1}toggle(e){this.mask^=1<<e|0}disable(e){this.mask&=~(1<<e|0)}disableAll(){this.mask=0}test(e){return(this.mask&e.mask)!==0}isEnabled(e){return(this.mask&(1<<e|0))!==0}}let Tf=0;const Xl=new re,br=new Wr,ui=new Kt,Ms=new re,Jr=new re,wf=new re,Af=new Wr,ql=new re(1,0,0),Yl=new re(0,1,0),Kl=new re(0,0,1),$l={type:"added"},Rf={type:"removed"},Er={type:"childadded",child:null},Da={type:"childremoved",child:null};class Tn extends ur{constructor(){super(),this.isObject3D=!0,Object.defineProperty(this,"id",{value:Tf++}),this.uuid=zi(),this.name="",this.type="Object3D",this.parent=null,this.children=[],this.up=Tn.DEFAULT_UP.clone();const e=new re,t=new dr,i=new Wr,r=new re(1,1,1);function s(){i.setFromEuler(t,!1)}function a(){t.setFromQuaternion(i,void 0,!1)}t._onChange(s),i._onChange(a),Object.defineProperties(this,{position:{configurable:!0,enumerable:!0,value:e},rotation:{configurable:!0,enumerable:!0,value:t},quaternion:{configurable:!0,enumerable:!0,value:i},scale:{configurable:!0,enumerable:!0,value:r},modelViewMatrix:{value:new Kt},normalMatrix:{value:new ct}}),this.matrix=new Kt,this.matrixWorld=new Kt,this.matrixAutoUpdate=Tn.DEFAULT_MATRIX_AUTO_UPDATE,this.matrixWorldAutoUpdate=Tn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE,this.matrixWorldNeedsUpdate=!1,this.layers=new gd,this.visible=!0,this.castShadow=!1,this.receiveShadow=!1,this.frustumCulled=!0,this.renderOrder=0,this.animations=[],this.customDepthMaterial=void 0,this.customDistanceMaterial=void 0,this.static=!1,this.userData={},this.pivot=null}onBeforeShadow(){}onAfterShadow(){}onBeforeRender(){}onAfterRender(){}applyMatrix4(e){this.matrixAutoUpdate&&this.updateMatrix(),this.matrix.premultiply(e),this.matrix.decompose(this.position,this.quaternion,this.scale)}applyQuaternion(e){return this.quaternion.premultiply(e),this}setRotationFromAxisAngle(e,t){this.quaternion.setFromAxisAngle(e,t)}setRotationFromEuler(e){this.quaternion.setFromEuler(e,!0)}setRotationFromMatrix(e){this.quaternion.setFromRotationMatrix(e)}setRotationFromQuaternion(e){this.quaternion.copy(e)}rotateOnAxis(e,t){return br.setFromAxisAngle(e,t),this.quaternion.multiply(br),this}rotateOnWorldAxis(e,t){return br.setFromAxisAngle(e,t),this.quaternion.premultiply(br),this}rotateX(e){return this.rotateOnAxis(ql,e)}rotateY(e){return this.rotateOnAxis(Yl,e)}rotateZ(e){return this.rotateOnAxis(Kl,e)}translateOnAxis(e,t){return Xl.copy(e).applyQuaternion(this.quaternion),this.position.add(Xl.multiplyScalar(t)),this}translateX(e){return this.translateOnAxis(ql,e)}translateY(e){return this.translateOnAxis(Yl,e)}translateZ(e){return this.translateOnAxis(Kl,e)}localToWorld(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(this.matrixWorld)}worldToLocal(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(ui.copy(this.matrixWorld).invert())}lookAt(e,t,i){e.isVector3?Ms.copy(e):Ms.set(e,t,i);const r=this.parent;this.updateWorldMatrix(!0,!1),Jr.setFromMatrixPosition(this.matrixWorld),this.isCamera||this.isLight?ui.lookAt(Jr,Ms,this.up):ui.lookAt(Ms,Jr,this.up),this.quaternion.setFromRotationMatrix(ui),r&&(ui.extractRotation(r.matrixWorld),br.setFromRotationMatrix(ui),this.quaternion.premultiply(br.invert()))}add(e){if(arguments.length>1){for(let t=0;t<arguments.length;t++)this.add(arguments[t]);return this}return e===this?(Et("Object3D.add: object can't be added as a child of itself.",e),this):(e&&e.isObject3D?(e.removeFromParent(),e.parent=this,this.children.push(e),e.dispatchEvent($l),Er.child=e,this.dispatchEvent(Er),Er.child=null):Et("Object3D.add: object not an instance of THREE.Object3D.",e),this)}remove(e){if(arguments.length>1){for(let i=0;i<arguments.length;i++)this.remove(arguments[i]);return this}const t=this.children.indexOf(e);return t!==-1&&(e.parent=null,this.children.splice(t,1),e.dispatchEvent(Rf),Da.child=e,this.dispatchEvent(Da),Da.child=null),this}removeFromParent(){const e=this.parent;return e!==null&&e.remove(this),this}clear(){return this.remove(...this.children)}attach(e){return this.updateWorldMatrix(!0,!1),ui.copy(this.matrixWorld).invert(),e.parent!==null&&(e.parent.updateWorldMatrix(!0,!1),ui.multiply(e.parent.matrixWorld)),e.applyMatrix4(ui),e.removeFromParent(),e.parent=this,this.children.push(e),e.updateWorldMatrix(!1,!0),e.dispatchEvent($l),Er.child=e,this.dispatchEvent(Er),Er.child=null,this}getObjectById(e){return this.getObjectByProperty("id",e)}getObjectByName(e){return this.getObjectByProperty("name",e)}getObjectByProperty(e,t){if(this[e]===t)return this;for(let i=0,r=this.children.length;i<r;i++){const a=this.children[i].getObjectByProperty(e,t);if(a!==void 0)return a}}getObjectsByProperty(e,t,i=[]){this[e]===t&&i.push(this);const r=this.children;for(let s=0,a=r.length;s<a;s++)r[s].getObjectsByProperty(e,t,i);return i}getWorldPosition(e){return this.updateWorldMatrix(!0,!1),e.setFromMatrixPosition(this.matrixWorld)}getWorldQuaternion(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Jr,e,wf),e}getWorldScale(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Jr,Af,e),e}getWorldDirection(e){this.updateWorldMatrix(!0,!1);const t=this.matrixWorld.elements;return e.set(t[8],t[9],t[10]).normalize()}raycast(){}traverse(e){e(this);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].traverse(e)}traverseVisible(e){if(this.visible===!1)return;e(this);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].traverseVisible(e)}traverseAncestors(e){const t=this.parent;t!==null&&(e(t),t.traverseAncestors(e))}updateMatrix(){this.matrix.compose(this.position,this.quaternion,this.scale);const e=this.pivot;if(e!==null){const t=e.x,i=e.y,r=e.z,s=this.matrix.elements;s[12]+=t-s[0]*t-s[4]*i-s[8]*r,s[13]+=i-s[1]*t-s[5]*i-s[9]*r,s[14]+=r-s[2]*t-s[6]*i-s[10]*r}this.matrixWorldNeedsUpdate=!0}updateMatrixWorld(e){this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||e)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,e=!0);const t=this.children;for(let i=0,r=t.length;i<r;i++)t[i].updateMatrixWorld(e)}updateWorldMatrix(e,t,i=!1){const r=this.parent;if(e===!0&&r!==null&&r.updateWorldMatrix(!0,!1),this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||i)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,i=!0),t===!0){const s=this.children;for(let a=0,o=s.length;a<o;a++)s[a].updateWorldMatrix(!1,!0,i)}}toJSON(e){const t=e===void 0||typeof e=="string",i={};t&&(e={geometries:{},materials:{},textures:{},images:{},shapes:{},skeletons:{},animations:{},nodes:{}},i.metadata={version:4.7,type:"Object",generator:"Object3D.toJSON"});const r={};r.uuid=this.uuid,r.type=this.type,this.name!==""&&(r.name=this.name),this.castShadow===!0&&(r.castShadow=!0),this.receiveShadow===!0&&(r.receiveShadow=!0),this.visible===!1&&(r.visible=!1),this.frustumCulled===!1&&(r.frustumCulled=!1),this.renderOrder!==0&&(r.renderOrder=this.renderOrder),this.static!==!1&&(r.static=this.static),Object.keys(this.userData).length>0&&(r.userData=this.userData),r.layers=this.layers.mask,r.matrix=this.matrix.toArray(),r.up=this.up.toArray(),this.pivot!==null&&(r.pivot=this.pivot.toArray()),this.matrixAutoUpdate===!1&&(r.matrixAutoUpdate=!1),this.morphTargetDictionary!==void 0&&(r.morphTargetDictionary=Object.assign({},this.morphTargetDictionary)),this.morphTargetInfluences!==void 0&&(r.morphTargetInfluences=this.morphTargetInfluences.slice()),this.isInstancedMesh&&(r.type="InstancedMesh",r.count=this.count,r.instanceMatrix=this.instanceMatrix.toJSON(),this.instanceColor!==null&&(r.instanceColor=this.instanceColor.toJSON())),this.isBatchedMesh&&(r.type="BatchedMesh",r.perObjectFrustumCulled=this.perObjectFrustumCulled,r.sortObjects=this.sortObjects,r.drawRanges=this._drawRanges,r.reservedRanges=this._reservedRanges,r.geometryInfo=this._geometryInfo.map(o=>({...o,boundingBox:o.boundingBox?o.boundingBox.toJSON():void 0,boundingSphere:o.boundingSphere?o.boundingSphere.toJSON():void 0})),r.instanceInfo=this._instanceInfo.map(o=>({...o})),r.availableInstanceIds=this._availableInstanceIds.slice(),r.availableGeometryIds=this._availableGeometryIds.slice(),r.nextIndexStart=this._nextIndexStart,r.nextVertexStart=this._nextVertexStart,r.geometryCount=this._geometryCount,r.maxInstanceCount=this._maxInstanceCount,r.maxVertexCount=this._maxVertexCount,r.maxIndexCount=this._maxIndexCount,r.geometryInitialized=this._geometryInitialized,r.matricesTexture=this._matricesTexture.toJSON(e),r.indirectTexture=this._indirectTexture.toJSON(e),this._colorsTexture!==null&&(r.colorsTexture=this._colorsTexture.toJSON(e)),this.boundingSphere!==null&&(r.boundingSphere=this.boundingSphere.toJSON()),this.boundingBox!==null&&(r.boundingBox=this.boundingBox.toJSON()));function s(o,l){return o[l.uuid]===void 0&&(o[l.uuid]=l.toJSON(e)),l.uuid}if(this.isScene)this.background&&(this.background.isColor?r.background=this.background.toJSON():this.background.isTexture&&(r.background=this.background.toJSON(e).uuid)),this.environment&&this.environment.isTexture&&this.environment.isRenderTargetTexture!==!0&&(r.environment=this.environment.toJSON(e).uuid);else if(this.isMesh||this.isLine||this.isPoints){r.geometry=s(e.geometries,this.geometry);const o=this.geometry.parameters;if(o!==void 0&&o.shapes!==void 0){const l=o.shapes;if(Array.isArray(l))for(let c=0,p=l.length;c<p;c++){const u=l[c];s(e.shapes,u)}else s(e.shapes,l)}}if(this.isSkinnedMesh&&(r.bindMode=this.bindMode,r.bindMatrix=this.bindMatrix.toArray(),this.skeleton!==void 0&&(s(e.skeletons,this.skeleton),r.skeleton=this.skeleton.uuid)),this.material!==void 0)if(Array.isArray(this.material)){const o=[];for(let l=0,c=this.material.length;l<c;l++)o.push(s(e.materials,this.material[l]));r.material=o}else r.material=s(e.materials,this.material);if(this.children.length>0){r.children=[];for(let o=0;o<this.children.length;o++)r.children.push(this.children[o].toJSON(e).object)}if(this.animations.length>0){r.animations=[];for(let o=0;o<this.animations.length;o++){const l=this.animations[o];r.animations.push(s(e.animations,l))}}if(t){const o=a(e.geometries),l=a(e.materials),c=a(e.textures),p=a(e.images),u=a(e.shapes),d=a(e.skeletons),h=a(e.animations),_=a(e.nodes);o.length>0&&(i.geometries=o),l.length>0&&(i.materials=l),c.length>0&&(i.textures=c),p.length>0&&(i.images=p),u.length>0&&(i.shapes=u),d.length>0&&(i.skeletons=d),h.length>0&&(i.animations=h),_.length>0&&(i.nodes=_)}return i.object=r,i;function a(o){const l=[];for(const c in o){const p=o[c];delete p.metadata,l.push(p)}return l}}clone(e){return new this.constructor().copy(this,e)}copy(e,t=!0){if(this.name=e.name,this.up.copy(e.up),this.position.copy(e.position),this.rotation.order=e.rotation.order,this.quaternion.copy(e.quaternion),this.scale.copy(e.scale),this.pivot=e.pivot!==null?e.pivot.clone():null,this.matrix.copy(e.matrix),this.matrixWorld.copy(e.matrixWorld),this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrixWorldAutoUpdate=e.matrixWorldAutoUpdate,this.matrixWorldNeedsUpdate=e.matrixWorldNeedsUpdate,this.layers.mask=e.layers.mask,this.visible=e.visible,this.castShadow=e.castShadow,this.receiveShadow=e.receiveShadow,this.frustumCulled=e.frustumCulled,this.renderOrder=e.renderOrder,this.static=e.static,this.animations=e.animations.slice(),this.userData=JSON.parse(JSON.stringify(e.userData)),t===!0)for(let i=0;i<e.children.length;i++){const r=e.children[i];this.add(r.clone())}return this}}Tn.DEFAULT_UP=new re(0,1,0);Tn.DEFAULT_MATRIX_AUTO_UPDATE=!0;Tn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE=!0;class bs extends Tn{constructor(){super(),this.isGroup=!0,this.type="Group"}}const Cf={type:"move"};class Ua{constructor(){this._targetRay=null,this._grip=null,this._hand=null}getHandSpace(){return this._hand===null&&(this._hand=new bs,this._hand.matrixAutoUpdate=!1,this._hand.visible=!1,this._hand.joints={},this._hand.inputState={pinching:!1}),this._hand}getTargetRaySpace(){return this._targetRay===null&&(this._targetRay=new bs,this._targetRay.matrixAutoUpdate=!1,this._targetRay.visible=!1,this._targetRay.hasLinearVelocity=!1,this._targetRay.linearVelocity=new re,this._targetRay.hasAngularVelocity=!1,this._targetRay.angularVelocity=new re),this._targetRay}getGripSpace(){return this._grip===null&&(this._grip=new bs,this._grip.matrixAutoUpdate=!1,this._grip.visible=!1,this._grip.hasLinearVelocity=!1,this._grip.linearVelocity=new re,this._grip.hasAngularVelocity=!1,this._grip.angularVelocity=new re,this._grip.eventsEnabled=!1),this._grip}dispatchEvent(e){return this._targetRay!==null&&this._targetRay.dispatchEvent(e),this._grip!==null&&this._grip.dispatchEvent(e),this._hand!==null&&this._hand.dispatchEvent(e),this}connect(e){if(e&&e.hand){const t=this._hand;if(t)for(const i of e.hand.values())this._getHandJoint(t,i)}return this.dispatchEvent({type:"connected",data:e}),this}disconnect(e){return this.dispatchEvent({type:"disconnected",data:e}),this._targetRay!==null&&(this._targetRay.visible=!1),this._grip!==null&&(this._grip.visible=!1),this._hand!==null&&(this._hand.visible=!1),this}update(e,t,i){let r=null,s=null,a=null;const o=this._targetRay,l=this._grip,c=this._hand;if(e&&t.session.visibilityState!=="visible-blurred"){if(c&&e.hand){a=!0;for(const x of e.hand.values()){const g=t.getJointPose(x,i),m=this._getHandJoint(c,x);g!==null&&(m.matrix.fromArray(g.transform.matrix),m.matrix.decompose(m.position,m.rotation,m.scale),m.matrixWorldNeedsUpdate=!0,m.jointRadius=g.radius),m.visible=g!==null}const p=c.joints["index-finger-tip"],u=c.joints["thumb-tip"],d=p.position.distanceTo(u.position),h=.02,_=.005;c.inputState.pinching&&d>h+_?(c.inputState.pinching=!1,this.dispatchEvent({type:"pinchend",handedness:e.handedness,target:this})):!c.inputState.pinching&&d<=h-_&&(c.inputState.pinching=!0,this.dispatchEvent({type:"pinchstart",handedness:e.handedness,target:this}))}else l!==null&&e.gripSpace&&(s=t.getPose(e.gripSpace,i),s!==null&&(l.matrix.fromArray(s.transform.matrix),l.matrix.decompose(l.position,l.rotation,l.scale),l.matrixWorldNeedsUpdate=!0,s.linearVelocity?(l.hasLinearVelocity=!0,l.linearVelocity.copy(s.linearVelocity)):l.hasLinearVelocity=!1,s.angularVelocity?(l.hasAngularVelocity=!0,l.angularVelocity.copy(s.angularVelocity)):l.hasAngularVelocity=!1,l.eventsEnabled&&l.dispatchEvent({type:"gripUpdated",data:e,target:this})));o!==null&&(r=t.getPose(e.targetRaySpace,i),r===null&&s!==null&&(r=s),r!==null&&(o.matrix.fromArray(r.transform.matrix),o.matrix.decompose(o.position,o.rotation,o.scale),o.matrixWorldNeedsUpdate=!0,r.linearVelocity?(o.hasLinearVelocity=!0,o.linearVelocity.copy(r.linearVelocity)):o.hasLinearVelocity=!1,r.angularVelocity?(o.hasAngularVelocity=!0,o.angularVelocity.copy(r.angularVelocity)):o.hasAngularVelocity=!1,this.dispatchEvent(Cf)))}return o!==null&&(o.visible=r!==null),l!==null&&(l.visible=s!==null),c!==null&&(c.visible=a!==null),this}_getHandJoint(e,t){if(e.joints[t.jointName]===void 0){const i=new bs;i.matrixAutoUpdate=!1,i.visible=!1,e.joints[t.jointName]=i,e.add(i)}return e.joints[t.jointName]}}const _d={aliceblue:15792383,antiquewhite:16444375,aqua:65535,aquamarine:8388564,azure:15794175,beige:16119260,bisque:16770244,black:0,blanchedalmond:16772045,blue:255,blueviolet:9055202,brown:10824234,burlywood:14596231,cadetblue:6266528,chartreuse:8388352,chocolate:13789470,coral:16744272,cornflowerblue:6591981,cornsilk:16775388,crimson:14423100,cyan:65535,darkblue:139,darkcyan:35723,darkgoldenrod:12092939,darkgray:11119017,darkgreen:25600,darkgrey:11119017,darkkhaki:12433259,darkmagenta:9109643,darkolivegreen:5597999,darkorange:16747520,darkorchid:10040012,darkred:9109504,darksalmon:15308410,darkseagreen:9419919,darkslateblue:4734347,darkslategray:3100495,darkslategrey:3100495,darkturquoise:52945,darkviolet:9699539,deeppink:16716947,deepskyblue:49151,dimgray:6908265,dimgrey:6908265,dodgerblue:2003199,firebrick:11674146,floralwhite:16775920,forestgreen:2263842,fuchsia:16711935,gainsboro:14474460,ghostwhite:16316671,gold:16766720,goldenrod:14329120,gray:8421504,green:32768,greenyellow:11403055,grey:8421504,honeydew:15794160,hotpink:16738740,indianred:13458524,indigo:4915330,ivory:16777200,khaki:15787660,lavender:15132410,lavenderblush:16773365,lawngreen:8190976,lemonchiffon:16775885,lightblue:11393254,lightcoral:15761536,lightcyan:14745599,lightgoldenrodyellow:16448210,lightgray:13882323,lightgreen:9498256,lightgrey:13882323,lightpink:16758465,lightsalmon:16752762,lightseagreen:2142890,lightskyblue:8900346,lightslategray:7833753,lightslategrey:7833753,lightsteelblue:11584734,lightyellow:16777184,lime:65280,limegreen:3329330,linen:16445670,magenta:16711935,maroon:8388608,mediumaquamarine:6737322,mediumblue:205,mediumorchid:12211667,mediumpurple:9662683,mediumseagreen:3978097,mediumslateblue:8087790,mediumspringgreen:64154,mediumturquoise:4772300,mediumvioletred:13047173,midnightblue:1644912,mintcream:16121850,mistyrose:16770273,moccasin:16770229,navajowhite:16768685,navy:128,oldlace:16643558,olive:8421376,olivedrab:7048739,orange:16753920,orangered:16729344,orchid:14315734,palegoldenrod:15657130,palegreen:10025880,paleturquoise:11529966,palevioletred:14381203,papayawhip:16773077,peachpuff:16767673,peru:13468991,pink:16761035,plum:14524637,powderblue:11591910,purple:8388736,rebeccapurple:6697881,red:16711680,rosybrown:12357519,royalblue:4286945,saddlebrown:9127187,salmon:16416882,sandybrown:16032864,seagreen:3050327,seashell:16774638,sienna:10506797,silver:12632256,skyblue:8900331,slateblue:6970061,slategray:7372944,slategrey:7372944,snow:16775930,springgreen:65407,steelblue:4620980,tan:13808780,teal:32896,thistle:14204888,tomato:16737095,turquoise:4251856,violet:15631086,wheat:16113331,white:16777215,whitesmoke:16119285,yellow:16776960,yellowgreen:10145074},Ii={h:0,s:0,l:0},Es={h:0,s:0,l:0};function Na(n,e,t){return t<0&&(t+=1),t>1&&(t-=1),t<1/6?n+(e-n)*6*t:t<1/2?e:t<2/3?n+(e-n)*6*(2/3-t):n}class Lt{constructor(e,t,i){return this.isColor=!0,this.r=1,this.g=1,this.b=1,this.set(e,t,i)}set(e,t,i){if(t===void 0&&i===void 0){const r=e;r&&r.isColor?this.copy(r):typeof r=="number"?this.setHex(r):typeof r=="string"&&this.setStyle(r)}else this.setRGB(e,t,i);return this}setScalar(e){return this.r=e,this.g=e,this.b=e,this}setHex(e,t=Rn){return e=Math.floor(e),this.r=(e>>16&255)/255,this.g=(e>>8&255)/255,this.b=(e&255)/255,xt.colorSpaceToWorking(this,t),this}setRGB(e,t,i,r=xt.workingColorSpace){return this.r=e,this.g=t,this.b=i,xt.colorSpaceToWorking(this,r),this}setHSL(e,t,i,r=xt.workingColorSpace){if(e=gf(e,1),t=yt(t,0,1),i=yt(i,0,1),t===0)this.r=this.g=this.b=i;else{const s=i<=.5?i*(1+t):i+t-i*t,a=2*i-s;this.r=Na(a,s,e+1/3),this.g=Na(a,s,e),this.b=Na(a,s,e-1/3)}return xt.colorSpaceToWorking(this,r),this}setStyle(e,t=Rn){function i(s){s!==void 0&&parseFloat(s)<1&&tt("Color: Alpha component of "+e+" will be ignored.")}let r;if(r=/^(\w+)\(([^\)]*)\)/.exec(e)){let s;const a=r[1],o=r[2];switch(a){case"rgb":case"rgba":if(s=/^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setRGB(Math.min(255,parseInt(s[1],10))/255,Math.min(255,parseInt(s[2],10))/255,Math.min(255,parseInt(s[3],10))/255,t);if(s=/^\s*(\d+)\%\s*,\s*(\d+)\%\s*,\s*(\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setRGB(Math.min(100,parseInt(s[1],10))/100,Math.min(100,parseInt(s[2],10))/100,Math.min(100,parseInt(s[3],10))/100,t);break;case"hsl":case"hsla":if(s=/^\s*(\d*\.?\d+)\s*,\s*(\d*\.?\d+)\%\s*,\s*(\d*\.?\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return i(s[4]),this.setHSL(parseFloat(s[1])/360,parseFloat(s[2])/100,parseFloat(s[3])/100,t);break;default:tt("Color: Unknown color model "+e)}}else if(r=/^\#([A-Fa-f\d]+)$/.exec(e)){const s=r[1],a=s.length;if(a===3)return this.setRGB(parseInt(s.charAt(0),16)/15,parseInt(s.charAt(1),16)/15,parseInt(s.charAt(2),16)/15,t);if(a===6)return this.setHex(parseInt(s,16),t);tt("Color: Invalid hex color "+e)}else if(e&&e.length>0)return this.setColorName(e,t);return this}setColorName(e,t=Rn){const i=_d[e.toLowerCase()];return i!==void 0?this.setHex(i,t):tt("Color: Unknown color "+e),this}clone(){return new this.constructor(this.r,this.g,this.b)}copy(e){return this.r=e.r,this.g=e.g,this.b=e.b,this}copySRGBToLinear(e){return this.r=bi(e.r),this.g=bi(e.g),this.b=bi(e.b),this}copyLinearToSRGB(e){return this.r=Or(e.r),this.g=Or(e.g),this.b=Or(e.b),this}convertSRGBToLinear(){return this.copySRGBToLinear(this),this}convertLinearToSRGB(){return this.copyLinearToSRGB(this),this}getHex(e=Rn){return xt.workingToColorSpace(ln.copy(this),e),Math.round(yt(ln.r*255,0,255))*65536+Math.round(yt(ln.g*255,0,255))*256+Math.round(yt(ln.b*255,0,255))}getHexString(e=Rn){return("000000"+this.getHex(e).toString(16)).slice(-6)}getHSL(e,t=xt.workingColorSpace){xt.workingToColorSpace(ln.copy(this),t);const i=ln.r,r=ln.g,s=ln.b,a=Math.max(i,r,s),o=Math.min(i,r,s);let l,c;const p=(o+a)/2;if(o===a)l=0,c=0;else{const u=a-o;switch(c=p<=.5?u/(a+o):u/(2-a-o),a){case i:l=(r-s)/u+(r<s?6:0);break;case r:l=(s-i)/u+2;break;case s:l=(i-r)/u+4;break}l/=6}return e.h=l,e.s=c,e.l=p,e}getRGB(e,t=xt.workingColorSpace){return xt.workingToColorSpace(ln.copy(this),t),e.r=ln.r,e.g=ln.g,e.b=ln.b,e}getStyle(e=Rn){xt.workingToColorSpace(ln.copy(this),e);const t=ln.r,i=ln.g,r=ln.b;return e!==Rn?`color(${e} ${t.toFixed(3)} ${i.toFixed(3)} ${r.toFixed(3)})`:`rgb(${Math.round(t*255)},${Math.round(i*255)},${Math.round(r*255)})`}offsetHSL(e,t,i){return this.getHSL(Ii),this.setHSL(Ii.h+e,Ii.s+t,Ii.l+i)}add(e){return this.r+=e.r,this.g+=e.g,this.b+=e.b,this}addColors(e,t){return this.r=e.r+t.r,this.g=e.g+t.g,this.b=e.b+t.b,this}addScalar(e){return this.r+=e,this.g+=e,this.b+=e,this}sub(e){return this.r=Math.max(0,this.r-e.r),this.g=Math.max(0,this.g-e.g),this.b=Math.max(0,this.b-e.b),this}multiply(e){return this.r*=e.r,this.g*=e.g,this.b*=e.b,this}multiplyScalar(e){return this.r*=e,this.g*=e,this.b*=e,this}lerp(e,t){return this.r+=(e.r-this.r)*t,this.g+=(e.g-this.g)*t,this.b+=(e.b-this.b)*t,this}lerpColors(e,t,i){return this.r=e.r+(t.r-e.r)*i,this.g=e.g+(t.g-e.g)*i,this.b=e.b+(t.b-e.b)*i,this}lerpHSL(e,t){this.getHSL(Ii),e.getHSL(Es);const i=Ra(Ii.h,Es.h,t),r=Ra(Ii.s,Es.s,t),s=Ra(Ii.l,Es.l,t);return this.setHSL(i,r,s),this}setFromVector3(e){return this.r=e.x,this.g=e.y,this.b=e.z,this}applyMatrix3(e){const t=this.r,i=this.g,r=this.b,s=e.elements;return this.r=s[0]*t+s[3]*i+s[6]*r,this.g=s[1]*t+s[4]*i+s[7]*r,this.b=s[2]*t+s[5]*i+s[8]*r,this}equals(e){return e.r===this.r&&e.g===this.g&&e.b===this.b}fromArray(e,t=0){return this.r=e[t],this.g=e[t+1],this.b=e[t+2],this}toArray(e=[],t=0){return e[t]=this.r,e[t+1]=this.g,e[t+2]=this.b,e}fromBufferAttribute(e,t){return this.r=e.getX(t),this.g=e.getY(t),this.b=e.getZ(t),this}toJSON(){return this.getHex()}*[Symbol.iterator](){yield this.r,yield this.g,yield this.b}}const ln=new Lt;Lt.NAMES=_d;class Pf extends Tn{constructor(){super(),this.isScene=!0,this.type="Scene",this.background=null,this.environment=null,this.fog=null,this.backgroundBlurriness=0,this.backgroundIntensity=1,this.backgroundRotation=new dr,this.environmentIntensity=1,this.environmentRotation=new dr,this.overrideMaterial=null,typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}copy(e,t){return super.copy(e,t),e.background!==null&&(this.background=e.background.clone()),e.environment!==null&&(this.environment=e.environment.clone()),e.fog!==null&&(this.fog=e.fog.clone()),this.backgroundBlurriness=e.backgroundBlurriness,this.backgroundIntensity=e.backgroundIntensity,this.backgroundRotation.copy(e.backgroundRotation),this.environmentIntensity=e.environmentIntensity,this.environmentRotation.copy(e.environmentRotation),e.overrideMaterial!==null&&(this.overrideMaterial=e.overrideMaterial.clone()),this.matrixAutoUpdate=e.matrixAutoUpdate,this}toJSON(e){const t=super.toJSON(e);return this.fog!==null&&(t.object.fog=this.fog.toJSON()),this.backgroundBlurriness>0&&(t.object.backgroundBlurriness=this.backgroundBlurriness),this.backgroundIntensity!==1&&(t.object.backgroundIntensity=this.backgroundIntensity),t.object.backgroundRotation=this.backgroundRotation.toArray(),this.environmentIntensity!==1&&(t.object.environmentIntensity=this.environmentIntensity),t.object.environmentRotation=this.environmentRotation.toArray(),t}}const Un=new re,fi=new re,Fa=new re,hi=new re,Tr=new re,wr=new re,Zl=new re,Oa=new re,Ba=new re,ka=new re,za=new Vt,Ga=new Vt,Ha=new Vt;class On{constructor(e=new re,t=new re,i=new re){this.a=e,this.b=t,this.c=i}static getNormal(e,t,i,r){r.subVectors(i,t),Un.subVectors(e,t),r.cross(Un);const s=r.lengthSq();return s>0?r.multiplyScalar(1/Math.sqrt(s)):r.set(0,0,0)}static getBarycoord(e,t,i,r,s){Un.subVectors(r,t),fi.subVectors(i,t),Fa.subVectors(e,t);const a=Un.dot(Un),o=Un.dot(fi),l=Un.dot(Fa),c=fi.dot(fi),p=fi.dot(Fa),u=a*c-o*o;if(u===0)return s.set(0,0,0),null;const d=1/u,h=(c*l-o*p)*d,_=(a*p-o*l)*d;return s.set(1-h-_,_,h)}static containsPoint(e,t,i,r){return this.getBarycoord(e,t,i,r,hi)===null?!1:hi.x>=0&&hi.y>=0&&hi.x+hi.y<=1}static getInterpolation(e,t,i,r,s,a,o,l){return this.getBarycoord(e,t,i,r,hi)===null?(l.x=0,l.y=0,"z"in l&&(l.z=0),"w"in l&&(l.w=0),null):(l.setScalar(0),l.addScaledVector(s,hi.x),l.addScaledVector(a,hi.y),l.addScaledVector(o,hi.z),l)}static getInterpolatedAttribute(e,t,i,r,s,a){return za.setScalar(0),Ga.setScalar(0),Ha.setScalar(0),za.fromBufferAttribute(e,t),Ga.fromBufferAttribute(e,i),Ha.fromBufferAttribute(e,r),a.setScalar(0),a.addScaledVector(za,s.x),a.addScaledVector(Ga,s.y),a.addScaledVector(Ha,s.z),a}static isFrontFacing(e,t,i,r){return Un.subVectors(i,t),fi.subVectors(e,t),Un.cross(fi).dot(r)<0}set(e,t,i){return this.a.copy(e),this.b.copy(t),this.c.copy(i),this}setFromPointsAndIndices(e,t,i,r){return this.a.copy(e[t]),this.b.copy(e[i]),this.c.copy(e[r]),this}setFromAttributeAndIndices(e,t,i,r){return this.a.fromBufferAttribute(e,t),this.b.fromBufferAttribute(e,i),this.c.fromBufferAttribute(e,r),this}clone(){return new this.constructor().copy(this)}copy(e){return this.a.copy(e.a),this.b.copy(e.b),this.c.copy(e.c),this}getArea(){return Un.subVectors(this.c,this.b),fi.subVectors(this.a,this.b),Un.cross(fi).length()*.5}getMidpoint(e){return e.addVectors(this.a,this.b).add(this.c).multiplyScalar(1/3)}getNormal(e){return On.getNormal(this.a,this.b,this.c,e)}getPlane(e){return e.setFromCoplanarPoints(this.a,this.b,this.c)}getBarycoord(e,t){return On.getBarycoord(e,this.a,this.b,this.c,t)}getInterpolation(e,t,i,r,s){return On.getInterpolation(e,this.a,this.b,this.c,t,i,r,s)}containsPoint(e){return On.containsPoint(e,this.a,this.b,this.c)}isFrontFacing(e){return On.isFrontFacing(this.a,this.b,this.c,e)}intersectsBox(e){return e.intersectsTriangle(this)}closestPointToPoint(e,t){const i=this.a,r=this.b,s=this.c;let a,o;Tr.subVectors(r,i),wr.subVectors(s,i),Oa.subVectors(e,i);const l=Tr.dot(Oa),c=wr.dot(Oa);if(l<=0&&c<=0)return t.copy(i);Ba.subVectors(e,r);const p=Tr.dot(Ba),u=wr.dot(Ba);if(p>=0&&u<=p)return t.copy(r);const d=l*u-p*c;if(d<=0&&l>=0&&p<=0)return a=l/(l-p),t.copy(i).addScaledVector(Tr,a);ka.subVectors(e,s);const h=Tr.dot(ka),_=wr.dot(ka);if(_>=0&&h<=_)return t.copy(s);const x=h*c-l*_;if(x<=0&&c>=0&&_<=0)return o=c/(c-_),t.copy(i).addScaledVector(wr,o);const g=p*_-h*u;if(g<=0&&u-p>=0&&h-_>=0)return Zl.subVectors(s,r),o=(u-p)/(u-p+(h-_)),t.copy(r).addScaledVector(Zl,o);const m=1/(g+x+d);return a=x*m,o=d*m,t.copy(i).addScaledVector(Tr,a).addScaledVector(wr,o)}equals(e){return e.a.equals(this.a)&&e.b.equals(this.b)&&e.c.equals(this.c)}}class hs{constructor(e=new re(1/0,1/0,1/0),t=new re(-1/0,-1/0,-1/0)){this.isBox3=!0,this.min=e,this.max=t}set(e,t){return this.min.copy(e),this.max.copy(t),this}setFromArray(e){this.makeEmpty();for(let t=0,i=e.length;t<i;t+=3)this.expandByPoint(Nn.fromArray(e,t));return this}setFromBufferAttribute(e){this.makeEmpty();for(let t=0,i=e.count;t<i;t++)this.expandByPoint(Nn.fromBufferAttribute(e,t));return this}setFromPoints(e){this.makeEmpty();for(let t=0,i=e.length;t<i;t++)this.expandByPoint(e[t]);return this}setFromCenterAndSize(e,t){const i=Nn.copy(t).multiplyScalar(.5);return this.min.copy(e).sub(i),this.max.copy(e).add(i),this}setFromObject(e,t=!1){return this.makeEmpty(),this.expandByObject(e,t)}clone(){return new this.constructor().copy(this)}copy(e){return this.min.copy(e.min),this.max.copy(e.max),this}makeEmpty(){return this.min.x=this.min.y=this.min.z=1/0,this.max.x=this.max.y=this.max.z=-1/0,this}isEmpty(){return this.max.x<this.min.x||this.max.y<this.min.y||this.max.z<this.min.z}getCenter(e){return this.isEmpty()?e.set(0,0,0):e.addVectors(this.min,this.max).multiplyScalar(.5)}getSize(e){return this.isEmpty()?e.set(0,0,0):e.subVectors(this.max,this.min)}expandByPoint(e){return this.min.min(e),this.max.max(e),this}expandByVector(e){return this.min.sub(e),this.max.add(e),this}expandByScalar(e){return this.min.addScalar(-e),this.max.addScalar(e),this}expandByObject(e,t=!1){e.updateWorldMatrix(!1,!1);const i=e.geometry;if(i!==void 0){const s=i.getAttribute("position");if(t===!0&&s!==void 0&&e.isInstancedMesh!==!0)for(let a=0,o=s.count;a<o;a++)e.isMesh===!0?e.getVertexPosition(a,Nn):Nn.fromBufferAttribute(s,a),Nn.applyMatrix4(e.matrixWorld),this.expandByPoint(Nn);else e.boundingBox!==void 0?(e.boundingBox===null&&e.computeBoundingBox(),Ts.copy(e.boundingBox)):(i.boundingBox===null&&i.computeBoundingBox(),Ts.copy(i.boundingBox)),Ts.applyMatrix4(e.matrixWorld),this.union(Ts)}const r=e.children;for(let s=0,a=r.length;s<a;s++)this.expandByObject(r[s],t);return this}containsPoint(e){return e.x>=this.min.x&&e.x<=this.max.x&&e.y>=this.min.y&&e.y<=this.max.y&&e.z>=this.min.z&&e.z<=this.max.z}containsBox(e){return this.min.x<=e.min.x&&e.max.x<=this.max.x&&this.min.y<=e.min.y&&e.max.y<=this.max.y&&this.min.z<=e.min.z&&e.max.z<=this.max.z}getParameter(e,t){return t.set((e.x-this.min.x)/(this.max.x-this.min.x),(e.y-this.min.y)/(this.max.y-this.min.y),(e.z-this.min.z)/(this.max.z-this.min.z))}intersectsBox(e){return e.max.x>=this.min.x&&e.min.x<=this.max.x&&e.max.y>=this.min.y&&e.min.y<=this.max.y&&e.max.z>=this.min.z&&e.min.z<=this.max.z}intersectsSphere(e){return this.clampPoint(e.center,Nn),Nn.distanceToSquared(e.center)<=e.radius*e.radius}intersectsPlane(e){let t,i;return e.normal.x>0?(t=e.normal.x*this.min.x,i=e.normal.x*this.max.x):(t=e.normal.x*this.max.x,i=e.normal.x*this.min.x),e.normal.y>0?(t+=e.normal.y*this.min.y,i+=e.normal.y*this.max.y):(t+=e.normal.y*this.max.y,i+=e.normal.y*this.min.y),e.normal.z>0?(t+=e.normal.z*this.min.z,i+=e.normal.z*this.max.z):(t+=e.normal.z*this.max.z,i+=e.normal.z*this.min.z),t<=-e.constant&&i>=-e.constant}intersectsTriangle(e){if(this.isEmpty())return!1;this.getCenter(Qr),ws.subVectors(this.max,Qr),Ar.subVectors(e.a,Qr),Rr.subVectors(e.b,Qr),Cr.subVectors(e.c,Qr),Di.subVectors(Rr,Ar),Ui.subVectors(Cr,Rr),$i.subVectors(Ar,Cr);let t=[0,-Di.z,Di.y,0,-Ui.z,Ui.y,0,-$i.z,$i.y,Di.z,0,-Di.x,Ui.z,0,-Ui.x,$i.z,0,-$i.x,-Di.y,Di.x,0,-Ui.y,Ui.x,0,-$i.y,$i.x,0];return!Va(t,Ar,Rr,Cr,ws)||(t=[1,0,0,0,1,0,0,0,1],!Va(t,Ar,Rr,Cr,ws))?!1:(As.crossVectors(Di,Ui),t=[As.x,As.y,As.z],Va(t,Ar,Rr,Cr,ws))}clampPoint(e,t){return t.copy(e).clamp(this.min,this.max)}distanceToPoint(e){return this.clampPoint(e,Nn).distanceTo(e)}getBoundingSphere(e){return this.isEmpty()?e.makeEmpty():(this.getCenter(e.center),e.radius=this.getSize(Nn).length()*.5),e}intersect(e){return this.min.max(e.min),this.max.min(e.max),this.isEmpty()&&this.makeEmpty(),this}union(e){return this.min.min(e.min),this.max.max(e.max),this}applyMatrix4(e){return this.isEmpty()?this:(pi[0].set(this.min.x,this.min.y,this.min.z).applyMatrix4(e),pi[1].set(this.min.x,this.min.y,this.max.z).applyMatrix4(e),pi[2].set(this.min.x,this.max.y,this.min.z).applyMatrix4(e),pi[3].set(this.min.x,this.max.y,this.max.z).applyMatrix4(e),pi[4].set(this.max.x,this.min.y,this.min.z).applyMatrix4(e),pi[5].set(this.max.x,this.min.y,this.max.z).applyMatrix4(e),pi[6].set(this.max.x,this.max.y,this.min.z).applyMatrix4(e),pi[7].set(this.max.x,this.max.y,this.max.z).applyMatrix4(e),this.setFromPoints(pi),this)}translate(e){return this.min.add(e),this.max.add(e),this}equals(e){return e.min.equals(this.min)&&e.max.equals(this.max)}toJSON(){return{min:this.min.toArray(),max:this.max.toArray()}}fromJSON(e){return this.min.fromArray(e.min),this.max.fromArray(e.max),this}}const pi=[new re,new re,new re,new re,new re,new re,new re,new re],Nn=new re,Ts=new hs,Ar=new re,Rr=new re,Cr=new re,Di=new re,Ui=new re,$i=new re,Qr=new re,ws=new re,As=new re,Zi=new re;function Va(n,e,t,i,r){for(let s=0,a=n.length-3;s<=a;s+=3){Zi.fromArray(n,s);const o=r.x*Math.abs(Zi.x)+r.y*Math.abs(Zi.y)+r.z*Math.abs(Zi.z),l=e.dot(Zi),c=t.dot(Zi),p=i.dot(Zi);if(Math.max(-Math.max(l,c,p),Math.min(l,c,p))>o)return!1}return!0}const Yt=new re,Rs=new Tt;let Lf=0;class zn extends ur{constructor(e,t,i=!1){if(super(),Array.isArray(e))throw new TypeError("THREE.BufferAttribute: array should be a Typed Array.");this.isBufferAttribute=!0,Object.defineProperty(this,"id",{value:Lf++}),this.name="",this.array=e,this.itemSize=t,this.count=e!==void 0?e.length/t:0,this.normalized=i,this.usage=Ko,this.updateRanges=[],this.gpuType=ei,this.version=0}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.name=e.name,this.array=new e.array.constructor(e.array),this.itemSize=e.itemSize,this.count=e.count,this.normalized=e.normalized,this.usage=e.usage,this.gpuType=e.gpuType,this}copyAt(e,t,i){e*=this.itemSize,i*=t.itemSize;for(let r=0,s=this.itemSize;r<s;r++)this.array[e+r]=t.array[i+r];return this}copyArray(e){return this.array.set(e),this}applyMatrix3(e){if(this.itemSize===2)for(let t=0,i=this.count;t<i;t++)Rs.fromBufferAttribute(this,t),Rs.applyMatrix3(e),this.setXY(t,Rs.x,Rs.y);else if(this.itemSize===3)for(let t=0,i=this.count;t<i;t++)Yt.fromBufferAttribute(this,t),Yt.applyMatrix3(e),this.setXYZ(t,Yt.x,Yt.y,Yt.z);return this}applyMatrix4(e){for(let t=0,i=this.count;t<i;t++)Yt.fromBufferAttribute(this,t),Yt.applyMatrix4(e),this.setXYZ(t,Yt.x,Yt.y,Yt.z);return this}applyNormalMatrix(e){for(let t=0,i=this.count;t<i;t++)Yt.fromBufferAttribute(this,t),Yt.applyNormalMatrix(e),this.setXYZ(t,Yt.x,Yt.y,Yt.z);return this}transformDirection(e){for(let t=0,i=this.count;t<i;t++)Yt.fromBufferAttribute(this,t),Yt.transformDirection(e),this.setXYZ(t,Yt.x,Yt.y,Yt.z);return this}set(e,t=0){return this.array.set(e,t),this}getComponent(e,t){let i=this.array[e*this.itemSize+t];return this.normalized&&(i=Qn(i,this.array)),i}setComponent(e,t,i){return this.normalized&&(i=Nt(i,this.array)),this.array[e*this.itemSize+t]=i,this}getX(e){let t=this.array[e*this.itemSize];return this.normalized&&(t=Qn(t,this.array)),t}setX(e,t){return this.normalized&&(t=Nt(t,this.array)),this.array[e*this.itemSize]=t,this}getY(e){let t=this.array[e*this.itemSize+1];return this.normalized&&(t=Qn(t,this.array)),t}setY(e,t){return this.normalized&&(t=Nt(t,this.array)),this.array[e*this.itemSize+1]=t,this}getZ(e){let t=this.array[e*this.itemSize+2];return this.normalized&&(t=Qn(t,this.array)),t}setZ(e,t){return this.normalized&&(t=Nt(t,this.array)),this.array[e*this.itemSize+2]=t,this}getW(e){let t=this.array[e*this.itemSize+3];return this.normalized&&(t=Qn(t,this.array)),t}setW(e,t){return this.normalized&&(t=Nt(t,this.array)),this.array[e*this.itemSize+3]=t,this}setXY(e,t,i){return e*=this.itemSize,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array)),this.array[e+0]=t,this.array[e+1]=i,this}setXYZ(e,t,i,r){return e*=this.itemSize,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array),r=Nt(r,this.array)),this.array[e+0]=t,this.array[e+1]=i,this.array[e+2]=r,this}setXYZW(e,t,i,r,s){return e*=this.itemSize,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array),r=Nt(r,this.array),s=Nt(s,this.array)),this.array[e+0]=t,this.array[e+1]=i,this.array[e+2]=r,this.array[e+3]=s,this}onUpload(e){return this.onUploadCallback=e,this}clone(){return new this.constructor(this.array,this.itemSize).copy(this)}toJSON(){const e={itemSize:this.itemSize,type:this.array.constructor.name,array:Array.from(this.array),normalized:this.normalized};return this.name!==""&&(e.name=this.name),this.usage!==Ko&&(e.usage=this.usage),e}dispose(){this.dispatchEvent({type:"dispose"})}}class vd extends zn{constructor(e,t,i){super(new Uint16Array(e),t,i)}}class xd extends zn{constructor(e,t,i){super(new Uint32Array(e),t,i)}}class Ei extends zn{constructor(e,t,i){super(new Float32Array(e),t,i)}}const If=new hs,jr=new re,Wa=new re;class _l{constructor(e=new re,t=-1){this.isSphere=!0,this.center=e,this.radius=t}set(e,t){return this.center.copy(e),this.radius=t,this}setFromPoints(e,t){const i=this.center;t!==void 0?i.copy(t):If.setFromPoints(e).getCenter(i);let r=0;for(let s=0,a=e.length;s<a;s++)r=Math.max(r,i.distanceToSquared(e[s]));return this.radius=Math.sqrt(r),this}copy(e){return this.center.copy(e.center),this.radius=e.radius,this}isEmpty(){return this.radius<0}makeEmpty(){return this.center.set(0,0,0),this.radius=-1,this}containsPoint(e){return e.distanceToSquared(this.center)<=this.radius*this.radius}distanceToPoint(e){return e.distanceTo(this.center)-this.radius}intersectsSphere(e){const t=this.radius+e.radius;return e.center.distanceToSquared(this.center)<=t*t}intersectsBox(e){return e.intersectsSphere(this)}intersectsPlane(e){return Math.abs(e.distanceToPoint(this.center))<=this.radius}clampPoint(e,t){const i=this.center.distanceToSquared(e);return t.copy(e),i>this.radius*this.radius&&(t.sub(this.center).normalize(),t.multiplyScalar(this.radius).add(this.center)),t}getBoundingBox(e){return this.isEmpty()?(e.makeEmpty(),e):(e.set(this.center,this.center),e.expandByScalar(this.radius),e)}applyMatrix4(e){return this.center.applyMatrix4(e),this.radius=this.radius*e.getMaxScaleOnAxis(),this}translate(e){return this.center.add(e),this}expandByPoint(e){if(this.isEmpty())return this.center.copy(e),this.radius=0,this;jr.subVectors(e,this.center);const t=jr.lengthSq();if(t>this.radius*this.radius){const i=Math.sqrt(t),r=(i-this.radius)*.5;this.center.addScaledVector(jr,r/i),this.radius+=r}return this}union(e){return e.isEmpty()?this:this.isEmpty()?(this.copy(e),this):(this.center.equals(e.center)===!0?this.radius=Math.max(this.radius,e.radius):(Wa.subVectors(e.center,this.center).setLength(e.radius),this.expandByPoint(jr.copy(e.center).add(Wa)),this.expandByPoint(jr.copy(e.center).sub(Wa))),this)}equals(e){return e.center.equals(this.center)&&e.radius===this.radius}clone(){return new this.constructor().copy(this)}toJSON(){return{radius:this.radius,center:this.center.toArray()}}fromJSON(e){return this.radius=e.radius,this.center.fromArray(e.center),this}}let Df=0;const wn=new Kt,Xa=new Tn,Pr=new re,Mn=new hs,es=new hs,jt=new re;class ai extends ur{constructor(){super(),this.isBufferGeometry=!0,Object.defineProperty(this,"id",{value:Df++}),this.uuid=zi(),this.name="",this.type="BufferGeometry",this.index=null,this.indirect=null,this.indirectOffset=0,this.attributes={},this.morphAttributes={},this.morphTargetsRelative=!1,this.groups=[],this.boundingBox=null,this.boundingSphere=null,this.drawRange={start:0,count:1/0},this.userData={},this._transformed=!1}getIndex(){return this.index}setIndex(e){return Array.isArray(e)?this.index=new(ff(e)?xd:vd)(e,1):this.index=e,this}setIndirect(e,t=0){return this.indirect=e,this.indirectOffset=t,this}getIndirect(){return this.indirect}getAttribute(e){return this.attributes[e]}setAttribute(e,t){return this.attributes[e]=t,this}deleteAttribute(e){return delete this.attributes[e],this}hasAttribute(e){return this.attributes[e]!==void 0}addGroup(e,t,i=0){this.groups.push({start:e,count:t,materialIndex:i})}clearGroups(){this.groups=[]}setDrawRange(e,t){this.drawRange.start=e,this.drawRange.count=t}applyMatrix4(e){const t=this.attributes.position;t!==void 0&&(t.applyMatrix4(e),t.needsUpdate=!0);const i=this.attributes.normal;if(i!==void 0){const s=new ct().getNormalMatrix(e);i.applyNormalMatrix(s),i.needsUpdate=!0}const r=this.attributes.tangent;return r!==void 0&&(r.transformDirection(e),r.needsUpdate=!0),this.boundingBox!==null&&this.computeBoundingBox(),this.boundingSphere!==null&&this.computeBoundingSphere(),this._transformed=!0,this}applyQuaternion(e){return wn.makeRotationFromQuaternion(e),this.applyMatrix4(wn),this}rotateX(e){return wn.makeRotationX(e),this.applyMatrix4(wn),this}rotateY(e){return wn.makeRotationY(e),this.applyMatrix4(wn),this}rotateZ(e){return wn.makeRotationZ(e),this.applyMatrix4(wn),this}translate(e,t,i){return wn.makeTranslation(e,t,i),this.applyMatrix4(wn),this}scale(e,t,i){return wn.makeScale(e,t,i),this.applyMatrix4(wn),this}lookAt(e){return Xa.lookAt(e),Xa.updateMatrix(),this.applyMatrix4(Xa.matrix),this}center(){return this.computeBoundingBox(),this.boundingBox.getCenter(Pr).negate(),this.translate(Pr.x,Pr.y,Pr.z),this}setFromPoints(e){const t=this.getAttribute("position");if(t===void 0){const i=[];for(let r=0,s=e.length;r<s;r++){const a=e[r];i.push(a.x,a.y,a.z||0)}this.setAttribute("position",new Ei(i,3))}else{const i=Math.min(e.length,t.count);for(let r=0;r<i;r++){const s=e[r];t.setXYZ(r,s.x,s.y,s.z||0)}e.length>t.count&&tt("BufferGeometry: Buffer size too small for points data. Use .dispose() and create a new geometry."),t.needsUpdate=!0}return this}computeBoundingBox(){this.boundingBox===null&&(this.boundingBox=new hs);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){Et("BufferGeometry.computeBoundingBox(): GLBufferAttribute requires a manual bounding box.",this),this.boundingBox.set(new re(-1/0,-1/0,-1/0),new re(1/0,1/0,1/0));return}if(e!==void 0){if(this.boundingBox.setFromBufferAttribute(e),t)for(let i=0,r=t.length;i<r;i++){const s=t[i];Mn.setFromBufferAttribute(s),this.morphTargetsRelative?(jt.addVectors(this.boundingBox.min,Mn.min),this.boundingBox.expandByPoint(jt),jt.addVectors(this.boundingBox.max,Mn.max),this.boundingBox.expandByPoint(jt)):(this.boundingBox.expandByPoint(Mn.min),this.boundingBox.expandByPoint(Mn.max))}}else this.boundingBox.makeEmpty();(isNaN(this.boundingBox.min.x)||isNaN(this.boundingBox.min.y)||isNaN(this.boundingBox.min.z))&&Et('BufferGeometry.computeBoundingBox(): Computed min/max have NaN values. The "position" attribute is likely to have NaN values.',this)}computeBoundingSphere(){this.boundingSphere===null&&(this.boundingSphere=new _l);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){Et("BufferGeometry.computeBoundingSphere(): GLBufferAttribute requires a manual bounding sphere.",this),this.boundingSphere.set(new re,1/0);return}if(e){const i=this.boundingSphere.center;if(Mn.setFromBufferAttribute(e),t)for(let s=0,a=t.length;s<a;s++){const o=t[s];es.setFromBufferAttribute(o),this.morphTargetsRelative?(jt.addVectors(Mn.min,es.min),Mn.expandByPoint(jt),jt.addVectors(Mn.max,es.max),Mn.expandByPoint(jt)):(Mn.expandByPoint(es.min),Mn.expandByPoint(es.max))}Mn.getCenter(i);let r=0;for(let s=0,a=e.count;s<a;s++)jt.fromBufferAttribute(e,s),r=Math.max(r,i.distanceToSquared(jt));if(t)for(let s=0,a=t.length;s<a;s++){const o=t[s],l=this.morphTargetsRelative;for(let c=0,p=o.count;c<p;c++)jt.fromBufferAttribute(o,c),l&&(Pr.fromBufferAttribute(e,c),jt.add(Pr)),r=Math.max(r,i.distanceToSquared(jt))}this.boundingSphere.radius=Math.sqrt(r),isNaN(this.boundingSphere.radius)&&Et('BufferGeometry.computeBoundingSphere(): Computed radius is NaN. The "position" attribute is likely to have NaN values.',this)}}computeTangents(){const e=this.index,t=this.attributes;if(e===null||t.position===void 0||t.normal===void 0||t.uv===void 0){Et("BufferGeometry: .computeTangents() failed. Missing required attributes (index, position, normal or uv)");return}const i=t.position,r=t.normal,s=t.uv;let a=this.getAttribute("tangent");(a===void 0||a.count!==i.count)&&(a=new zn(new Float32Array(4*i.count),4),this.setAttribute("tangent",a));const o=[],l=[];for(let v=0;v<i.count;v++)o[v]=new re,l[v]=new re;const c=new re,p=new re,u=new re,d=new Tt,h=new Tt,_=new Tt,x=new re,g=new re;function m(v,S,w){c.fromBufferAttribute(i,v),p.fromBufferAttribute(i,S),u.fromBufferAttribute(i,w),d.fromBufferAttribute(s,v),h.fromBufferAttribute(s,S),_.fromBufferAttribute(s,w),p.sub(c),u.sub(c),h.sub(d),_.sub(d);const L=1/(h.x*_.y-_.x*h.y);isFinite(L)&&(x.copy(p).multiplyScalar(_.y).addScaledVector(u,-h.y).multiplyScalar(L),g.copy(u).multiplyScalar(h.x).addScaledVector(p,-_.x).multiplyScalar(L),o[v].add(x),o[S].add(x),o[w].add(x),l[v].add(g),l[S].add(g),l[w].add(g))}let A=this.groups;A.length===0&&(A=[{start:0,count:e.count}]);for(let v=0,S=A.length;v<S;++v){const w=A[v],L=w.start,V=w.count;for(let Z=L,k=L+V;Z<k;Z+=3)m(e.getX(Z+0),e.getX(Z+1),e.getX(Z+2))}const P=new re,M=new re,I=new re,T=new re;function D(v){I.fromBufferAttribute(r,v),T.copy(I);const S=o[v];P.copy(S),P.sub(I.multiplyScalar(I.dot(S))).normalize(),M.crossVectors(T,S);const L=M.dot(l[v])<0?-1:1;a.setXYZW(v,P.x,P.y,P.z,L)}for(let v=0,S=A.length;v<S;++v){const w=A[v],L=w.start,V=w.count;for(let Z=L,k=L+V;Z<k;Z+=3)D(e.getX(Z+0)),D(e.getX(Z+1)),D(e.getX(Z+2))}this._transformed=!0}computeVertexNormals(){const e=this.index,t=this.getAttribute("position");if(t!==void 0){let i=this.getAttribute("normal");if(i===void 0||i.count!==t.count)i=new zn(new Float32Array(t.count*3),3),this.setAttribute("normal",i);else for(let d=0,h=i.count;d<h;d++)i.setXYZ(d,0,0,0);const r=new re,s=new re,a=new re,o=new re,l=new re,c=new re,p=new re,u=new re;if(e)for(let d=0,h=e.count;d<h;d+=3){const _=e.getX(d+0),x=e.getX(d+1),g=e.getX(d+2);r.fromBufferAttribute(t,_),s.fromBufferAttribute(t,x),a.fromBufferAttribute(t,g),p.subVectors(a,s),u.subVectors(r,s),p.cross(u),o.fromBufferAttribute(i,_),l.fromBufferAttribute(i,x),c.fromBufferAttribute(i,g),o.add(p),l.add(p),c.add(p),i.setXYZ(_,o.x,o.y,o.z),i.setXYZ(x,l.x,l.y,l.z),i.setXYZ(g,c.x,c.y,c.z)}else for(let d=0,h=t.count;d<h;d+=3)r.fromBufferAttribute(t,d+0),s.fromBufferAttribute(t,d+1),a.fromBufferAttribute(t,d+2),p.subVectors(a,s),u.subVectors(r,s),p.cross(u),i.setXYZ(d+0,p.x,p.y,p.z),i.setXYZ(d+1,p.x,p.y,p.z),i.setXYZ(d+2,p.x,p.y,p.z);this.normalizeNormals(),i.needsUpdate=!0}}normalizeNormals(){const e=this.attributes.normal;for(let t=0,i=e.count;t<i;t++)jt.fromBufferAttribute(e,t),jt.normalize(),e.setXYZ(t,jt.x,jt.y,jt.z)}toNonIndexed(){function e(o,l){const c=o.array,p=o.itemSize,u=o.normalized,d=new c.constructor(l.length*p);let h=0,_=0;for(let x=0,g=l.length;x<g;x++){o.isInterleavedBufferAttribute?h=l[x]*o.data.stride+o.offset:h=l[x]*p;for(let m=0;m<p;m++)d[_++]=c[h++]}return new zn(d,p,u)}if(this.index===null)return tt("BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed."),this;const t=new ai,i=this.index.array,r=this.attributes;for(const o in r){const l=r[o],c=e(l,i);t.setAttribute(o,c)}const s=this.morphAttributes;for(const o in s){const l=[],c=s[o];for(let p=0,u=c.length;p<u;p++){const d=c[p],h=e(d,i);l.push(h)}t.morphAttributes[o]=l}t.morphTargetsRelative=this.morphTargetsRelative;const a=this.groups;for(let o=0,l=a.length;o<l;o++){const c=a[o];t.addGroup(c.start,c.count,c.materialIndex)}return t}toJSON(){const e={metadata:{version:4.7,type:"BufferGeometry",generator:"BufferGeometry.toJSON"}};if(e.uuid=this.uuid,e.type=this.parameters!==void 0&&this._transformed===!0?"BufferGeometry":this.type,this.name!==""&&(e.name=this.name),Object.keys(this.userData).length>0&&(e.userData=this.userData),this.parameters!==void 0&&this._transformed!==!0){const l=this.parameters;for(const c in l)l[c]!==void 0&&(e[c]=l[c]);return e}e.data={attributes:{}};const t=this.index;t!==null&&(e.data.index={type:t.array.constructor.name,array:Array.prototype.slice.call(t.array)});const i=this.attributes;for(const l in i){const c=i[l];e.data.attributes[l]=c.toJSON(e.data)}const r={};let s=!1;for(const l in this.morphAttributes){const c=this.morphAttributes[l],p=[];for(let u=0,d=c.length;u<d;u++){const h=c[u];p.push(h.toJSON(e.data))}p.length>0&&(r[l]=p,s=!0)}s&&(e.data.morphAttributes=r,e.data.morphTargetsRelative=this.morphTargetsRelative);const a=this.groups;a.length>0&&(e.data.groups=JSON.parse(JSON.stringify(a)));const o=this.boundingSphere;return o!==null&&(e.data.boundingSphere=o.toJSON()),e}clone(){return new this.constructor().copy(this)}copy(e){this.index=null,this.attributes={},this.morphAttributes={},this.groups=[],this.boundingBox=null,this.boundingSphere=null;const t={};this.name=e.name;const i=e.index;i!==null&&this.setIndex(i.clone());const r=e.attributes;for(const c in r){const p=r[c];this.setAttribute(c,p.clone(t))}const s=e.morphAttributes;for(const c in s){const p=[],u=s[c];for(let d=0,h=u.length;d<h;d++)p.push(u[d].clone(t));this.morphAttributes[c]=p}this.morphTargetsRelative=e.morphTargetsRelative;const a=e.groups;for(let c=0,p=a.length;c<p;c++){const u=a[c];this.addGroup(u.start,u.count,u.materialIndex)}const o=e.boundingBox;o!==null&&(this.boundingBox=o.clone());const l=e.boundingSphere;return l!==null&&(this.boundingSphere=l.clone()),this.drawRange.start=e.drawRange.start,this.drawRange.count=e.drawRange.count,this.userData=e.userData,this._transformed=e._transformed,this}dispose(){this.dispatchEvent({type:"dispose"})}}class Uf{constructor(e,t){this.isInterleavedBuffer=!0,this.array=e,this.stride=t,this.count=e!==void 0?e.length/t:0,this.usage=Ko,this.updateRanges=[],this.version=0,this.uuid=zi()}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.array=new e.array.constructor(e.array),this.count=e.count,this.stride=e.stride,this.usage=e.usage,this}copyAt(e,t,i){e*=this.stride,i*=t.stride;for(let r=0,s=this.stride;r<s;r++)this.array[e+r]=t.array[i+r];return this}set(e,t=0){return this.array.set(e,t),this}clone(e){e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=zi()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=this.array.slice(0).buffer);const t=new this.array.constructor(e.arrayBuffers[this.array.buffer._uuid]),i=new this.constructor(t,this.stride);return i.setUsage(this.usage),i}onUpload(e){return this.onUploadCallback=e,this}toJSON(e){return e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=zi()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=Array.from(new Uint32Array(this.array.buffer))),{uuid:this.uuid,buffer:this.array.buffer._uuid,type:this.array.constructor.name,stride:this.stride}}}const fn=new re;class vl{constructor(e,t,i,r=!1){this.isInterleavedBufferAttribute=!0,this.name="",this.data=e,this.itemSize=t,this.offset=i,this.normalized=r}get count(){return this.data.count}get array(){return this.data.array}set needsUpdate(e){this.data.needsUpdate=e}applyMatrix4(e){for(let t=0,i=this.data.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.applyMatrix4(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}applyNormalMatrix(e){for(let t=0,i=this.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.applyNormalMatrix(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}transformDirection(e){for(let t=0,i=this.count;t<i;t++)fn.fromBufferAttribute(this,t),fn.transformDirection(e),this.setXYZ(t,fn.x,fn.y,fn.z);return this}getComponent(e,t){let i=this.array[e*this.data.stride+this.offset+t];return this.normalized&&(i=Qn(i,this.array)),i}setComponent(e,t,i){return this.normalized&&(i=Nt(i,this.array)),this.data.array[e*this.data.stride+this.offset+t]=i,this}setX(e,t){return this.normalized&&(t=Nt(t,this.array)),this.data.array[e*this.data.stride+this.offset]=t,this}setY(e,t){return this.normalized&&(t=Nt(t,this.array)),this.data.array[e*this.data.stride+this.offset+1]=t,this}setZ(e,t){return this.normalized&&(t=Nt(t,this.array)),this.data.array[e*this.data.stride+this.offset+2]=t,this}setW(e,t){return this.normalized&&(t=Nt(t,this.array)),this.data.array[e*this.data.stride+this.offset+3]=t,this}getX(e){let t=this.data.array[e*this.data.stride+this.offset];return this.normalized&&(t=Qn(t,this.array)),t}getY(e){let t=this.data.array[e*this.data.stride+this.offset+1];return this.normalized&&(t=Qn(t,this.array)),t}getZ(e){let t=this.data.array[e*this.data.stride+this.offset+2];return this.normalized&&(t=Qn(t,this.array)),t}getW(e){let t=this.data.array[e*this.data.stride+this.offset+3];return this.normalized&&(t=Qn(t,this.array)),t}setXY(e,t,i){return e=e*this.data.stride+this.offset,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this}setXYZ(e,t,i,r){return e=e*this.data.stride+this.offset,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array),r=Nt(r,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this.data.array[e+2]=r,this}setXYZW(e,t,i,r,s){return e=e*this.data.stride+this.offset,this.normalized&&(t=Nt(t,this.array),i=Nt(i,this.array),r=Nt(r,this.array),s=Nt(s,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=i,this.data.array[e+2]=r,this.data.array[e+3]=s,this}clone(e){if(e===void 0){la("InterleavedBufferAttribute.clone(): Cloning an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let i=0;i<this.count;i++){const r=i*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return new zn(new this.array.constructor(t),this.itemSize,this.normalized)}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.clone(e)),new vl(e.interleavedBuffers[this.data.uuid],this.itemSize,this.offset,this.normalized)}toJSON(e){if(e===void 0){la("InterleavedBufferAttribute.toJSON(): Serializing an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let i=0;i<this.count;i++){const r=i*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return{itemSize:this.itemSize,type:this.array.constructor.name,array:t,normalized:this.normalized}}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.toJSON(e)),{isInterleavedBufferAttribute:!0,itemSize:this.itemSize,data:this.data.uuid,offset:this.offset,normalized:this.normalized}}}let Nf=0;class ma extends ur{constructor(){super(),this.isMaterial=!0,Object.defineProperty(this,"id",{value:Nf++}),this.uuid=zi(),this.name="",this.type="Material",this.blending=Nr,this.side=Gi,this.vertexColors=!1,this.opacity=1,this.transparent=!1,this.alphaHash=!1,this.blendSrc=oo,this.blendDst=os,this.blendEquation=Oi,this.blendSrcAlpha=null,this.blendDstAlpha=null,this.blendEquationAlpha=null,this.blendColor=new Lt(0,0,0),this.blendAlpha=0,this.depthFunc=Br,this.depthTest=!0,this.depthWrite=!0,this.stencilWriteMask=255,this.stencilFunc=Bl,this.stencilRef=0,this.stencilFuncMask=255,this.stencilFail=yr,this.stencilZFail=yr,this.stencilZPass=yr,this.stencilWrite=!1,this.clippingPlanes=null,this.clipIntersection=!1,this.clipShadows=!1,this.shadowSide=null,this.colorWrite=!0,this.precision=null,this.polygonOffset=!1,this.polygonOffsetFactor=0,this.polygonOffsetUnits=0,this.dithering=!1,this.alphaToCoverage=!1,this.premultipliedAlpha=!1,this.forceSinglePass=!1,this.allowOverride=!0,this.visible=!0,this.toneMapped=!0,this.userData={},this.version=0,this._alphaTest=0}get alphaTest(){return this._alphaTest}set alphaTest(e){this._alphaTest>0!=e>0&&this.version++,this._alphaTest=e}onBeforeRender(){}onBeforeCompile(){}customProgramCacheKey(){return this.onBeforeCompile.toString()}setValues(e){if(e!==void 0)for(const t in e){const i=e[t];if(i===void 0){tt(`Material: parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){tt(`Material: '${t}' is not a property of THREE.${this.type}.`);continue}r&&r.isColor?r.set(i):r&&r.isVector2&&i&&i.isVector2||r&&r.isEuler&&i&&i.isEuler||r&&r.isVector3&&i&&i.isVector3?r.copy(i):this[t]=i}}toJSON(e){const t=e===void 0||typeof e=="string";t&&(e={textures:{},images:{}});const i={metadata:{version:4.7,type:"Material",generator:"Material.toJSON"}};i.uuid=this.uuid,i.type=this.type,this.name!==""&&(i.name=this.name),this.color&&this.color.isColor&&(i.color=this.color.getHex()),this.roughness!==void 0&&(i.roughness=this.roughness),this.metalness!==void 0&&(i.metalness=this.metalness),this.sheen!==void 0&&(i.sheen=this.sheen),this.sheenColor&&this.sheenColor.isColor&&(i.sheenColor=this.sheenColor.getHex()),this.sheenRoughness!==void 0&&(i.sheenRoughness=this.sheenRoughness),this.emissive&&this.emissive.isColor&&(i.emissive=this.emissive.getHex()),this.emissiveIntensity!==void 0&&this.emissiveIntensity!==1&&(i.emissiveIntensity=this.emissiveIntensity),this.specular&&this.specular.isColor&&(i.specular=this.specular.getHex()),this.specularIntensity!==void 0&&(i.specularIntensity=this.specularIntensity),this.specularColor&&this.specularColor.isColor&&(i.specularColor=this.specularColor.getHex()),this.shininess!==void 0&&(i.shininess=this.shininess),this.clearcoat!==void 0&&(i.clearcoat=this.clearcoat),this.clearcoatRoughness!==void 0&&(i.clearcoatRoughness=this.clearcoatRoughness),this.clearcoatMap&&this.clearcoatMap.isTexture&&(i.clearcoatMap=this.clearcoatMap.toJSON(e).uuid),this.clearcoatRoughnessMap&&this.clearcoatRoughnessMap.isTexture&&(i.clearcoatRoughnessMap=this.clearcoatRoughnessMap.toJSON(e).uuid),this.clearcoatNormalMap&&this.clearcoatNormalMap.isTexture&&(i.clearcoatNormalMap=this.clearcoatNormalMap.toJSON(e).uuid,i.clearcoatNormalScale=this.clearcoatNormalScale.toArray()),this.sheenColorMap&&this.sheenColorMap.isTexture&&(i.sheenColorMap=this.sheenColorMap.toJSON(e).uuid),this.sheenRoughnessMap&&this.sheenRoughnessMap.isTexture&&(i.sheenRoughnessMap=this.sheenRoughnessMap.toJSON(e).uuid),this.dispersion!==void 0&&(i.dispersion=this.dispersion),this.iridescence!==void 0&&(i.iridescence=this.iridescence),this.iridescenceIOR!==void 0&&(i.iridescenceIOR=this.iridescenceIOR),this.iridescenceThicknessRange!==void 0&&(i.iridescenceThicknessRange=this.iridescenceThicknessRange),this.iridescenceMap&&this.iridescenceMap.isTexture&&(i.iridescenceMap=this.iridescenceMap.toJSON(e).uuid),this.iridescenceThicknessMap&&this.iridescenceThicknessMap.isTexture&&(i.iridescenceThicknessMap=this.iridescenceThicknessMap.toJSON(e).uuid),this.anisotropy!==void 0&&(i.anisotropy=this.anisotropy),this.anisotropyRotation!==void 0&&(i.anisotropyRotation=this.anisotropyRotation),this.anisotropyMap&&this.anisotropyMap.isTexture&&(i.anisotropyMap=this.anisotropyMap.toJSON(e).uuid),this.map&&this.map.isTexture&&(i.map=this.map.toJSON(e).uuid),this.matcap&&this.matcap.isTexture&&(i.matcap=this.matcap.toJSON(e).uuid),this.alphaMap&&this.alphaMap.isTexture&&(i.alphaMap=this.alphaMap.toJSON(e).uuid),this.lightMap&&this.lightMap.isTexture&&(i.lightMap=this.lightMap.toJSON(e).uuid,i.lightMapIntensity=this.lightMapIntensity),this.aoMap&&this.aoMap.isTexture&&(i.aoMap=this.aoMap.toJSON(e).uuid,i.aoMapIntensity=this.aoMapIntensity),this.bumpMap&&this.bumpMap.isTexture&&(i.bumpMap=this.bumpMap.toJSON(e).uuid,i.bumpScale=this.bumpScale),this.normalMap&&this.normalMap.isTexture&&(i.normalMap=this.normalMap.toJSON(e).uuid,i.normalMapType=this.normalMapType,i.normalScale=this.normalScale.toArray()),this.displacementMap&&this.displacementMap.isTexture&&(i.displacementMap=this.displacementMap.toJSON(e).uuid,i.displacementScale=this.displacementScale,i.displacementBias=this.displacementBias),this.roughnessMap&&this.roughnessMap.isTexture&&(i.roughnessMap=this.roughnessMap.toJSON(e).uuid),this.metalnessMap&&this.metalnessMap.isTexture&&(i.metalnessMap=this.metalnessMap.toJSON(e).uuid),this.emissiveMap&&this.emissiveMap.isTexture&&(i.emissiveMap=this.emissiveMap.toJSON(e).uuid),this.specularMap&&this.specularMap.isTexture&&(i.specularMap=this.specularMap.toJSON(e).uuid),this.specularIntensityMap&&this.specularIntensityMap.isTexture&&(i.specularIntensityMap=this.specularIntensityMap.toJSON(e).uuid),this.specularColorMap&&this.specularColorMap.isTexture&&(i.specularColorMap=this.specularColorMap.toJSON(e).uuid),this.envMap&&this.envMap.isTexture&&(i.envMap=this.envMap.toJSON(e).uuid,this.combine!==void 0&&(i.combine=this.combine)),this.envMapRotation!==void 0&&(i.envMapRotation=this.envMapRotation.toArray()),this.envMapIntensity!==void 0&&(i.envMapIntensity=this.envMapIntensity),this.reflectivity!==void 0&&(i.reflectivity=this.reflectivity),this.refractionRatio!==void 0&&(i.refractionRatio=this.refractionRatio),this.gradientMap&&this.gradientMap.isTexture&&(i.gradientMap=this.gradientMap.toJSON(e).uuid),this.transmission!==void 0&&(i.transmission=this.transmission),this.transmissionMap&&this.transmissionMap.isTexture&&(i.transmissionMap=this.transmissionMap.toJSON(e).uuid),this.thickness!==void 0&&(i.thickness=this.thickness),this.thicknessMap&&this.thicknessMap.isTexture&&(i.thicknessMap=this.thicknessMap.toJSON(e).uuid),this.attenuationDistance!==void 0&&this.attenuationDistance!==1/0&&(i.attenuationDistance=this.attenuationDistance),this.attenuationColor!==void 0&&(i.attenuationColor=this.attenuationColor.getHex()),this.size!==void 0&&(i.size=this.size),this.shadowSide!==null&&(i.shadowSide=this.shadowSide),this.sizeAttenuation!==void 0&&(i.sizeAttenuation=this.sizeAttenuation),this.blending!==Nr&&(i.blending=this.blending),this.side!==Gi&&(i.side=this.side),this.vertexColors===!0&&(i.vertexColors=!0),this.opacity<1&&(i.opacity=this.opacity),this.transparent===!0&&(i.transparent=!0),this.blendSrc!==oo&&(i.blendSrc=this.blendSrc),this.blendDst!==os&&(i.blendDst=this.blendDst),this.blendEquation!==Oi&&(i.blendEquation=this.blendEquation),this.blendSrcAlpha!==null&&(i.blendSrcAlpha=this.blendSrcAlpha),this.blendDstAlpha!==null&&(i.blendDstAlpha=this.blendDstAlpha),this.blendEquationAlpha!==null&&(i.blendEquationAlpha=this.blendEquationAlpha),this.blendColor&&this.blendColor.isColor&&(i.blendColor=this.blendColor.getHex()),this.blendAlpha!==0&&(i.blendAlpha=this.blendAlpha),this.depthFunc!==Br&&(i.depthFunc=this.depthFunc),this.depthTest===!1&&(i.depthTest=this.depthTest),this.depthWrite===!1&&(i.depthWrite=this.depthWrite),this.colorWrite===!1&&(i.colorWrite=this.colorWrite),this.stencilWriteMask!==255&&(i.stencilWriteMask=this.stencilWriteMask),this.stencilFunc!==Bl&&(i.stencilFunc=this.stencilFunc),this.stencilRef!==0&&(i.stencilRef=this.stencilRef),this.stencilFuncMask!==255&&(i.stencilFuncMask=this.stencilFuncMask),this.stencilFail!==yr&&(i.stencilFail=this.stencilFail),this.stencilZFail!==yr&&(i.stencilZFail=this.stencilZFail),this.stencilZPass!==yr&&(i.stencilZPass=this.stencilZPass),this.stencilWrite===!0&&(i.stencilWrite=this.stencilWrite),this.rotation!==void 0&&this.rotation!==0&&(i.rotation=this.rotation),this.polygonOffset===!0&&(i.polygonOffset=!0),this.polygonOffsetFactor!==0&&(i.polygonOffsetFactor=this.polygonOffsetFactor),this.polygonOffsetUnits!==0&&(i.polygonOffsetUnits=this.polygonOffsetUnits),this.linewidth!==void 0&&this.linewidth!==1&&(i.linewidth=this.linewidth),this.dashSize!==void 0&&(i.dashSize=this.dashSize),this.gapSize!==void 0&&(i.gapSize=this.gapSize),this.scale!==void 0&&(i.scale=this.scale),this.dithering===!0&&(i.dithering=!0),this.alphaTest>0&&(i.alphaTest=this.alphaTest),this.alphaHash===!0&&(i.alphaHash=!0),this.alphaToCoverage===!0&&(i.alphaToCoverage=!0),this.premultipliedAlpha===!0&&(i.premultipliedAlpha=!0),this.forceSinglePass===!0&&(i.forceSinglePass=!0),this.allowOverride===!1&&(i.allowOverride=!1),this.wireframe===!0&&(i.wireframe=!0),this.wireframeLinewidth>1&&(i.wireframeLinewidth=this.wireframeLinewidth),this.wireframeLinecap!=="round"&&(i.wireframeLinecap=this.wireframeLinecap),this.wireframeLinejoin!=="round"&&(i.wireframeLinejoin=this.wireframeLinejoin),this.flatShading===!0&&(i.flatShading=!0),this.visible===!1&&(i.visible=!1),this.toneMapped===!1&&(i.toneMapped=!1),this.fog===!1&&(i.fog=!1),Object.keys(this.userData).length>0&&(i.userData=this.userData);function r(s){const a=[];for(const o in s){const l=s[o];delete l.metadata,a.push(l)}return a}if(t){const s=r(e.textures),a=r(e.images);s.length>0&&(i.textures=s),a.length>0&&(i.images=a)}return i}fromJSON(e,t){if(e.uuid!==void 0&&(this.uuid=e.uuid),e.name!==void 0&&(this.name=e.name),e.color!==void 0&&this.color!==void 0&&this.color.setHex(e.color),e.roughness!==void 0&&(this.roughness=e.roughness),e.metalness!==void 0&&(this.metalness=e.metalness),e.sheen!==void 0&&(this.sheen=e.sheen),e.sheenColor!==void 0&&(this.sheenColor=new Lt().setHex(e.sheenColor)),e.sheenRoughness!==void 0&&(this.sheenRoughness=e.sheenRoughness),e.emissive!==void 0&&this.emissive!==void 0&&this.emissive.setHex(e.emissive),e.specular!==void 0&&this.specular!==void 0&&this.specular.setHex(e.specular),e.specularIntensity!==void 0&&(this.specularIntensity=e.specularIntensity),e.specularColor!==void 0&&this.specularColor!==void 0&&this.specularColor.setHex(e.specularColor),e.shininess!==void 0&&(this.shininess=e.shininess),e.clearcoat!==void 0&&(this.clearcoat=e.clearcoat),e.clearcoatRoughness!==void 0&&(this.clearcoatRoughness=e.clearcoatRoughness),e.dispersion!==void 0&&(this.dispersion=e.dispersion),e.iridescence!==void 0&&(this.iridescence=e.iridescence),e.iridescenceIOR!==void 0&&(this.iridescenceIOR=e.iridescenceIOR),e.iridescenceThicknessRange!==void 0&&(this.iridescenceThicknessRange=e.iridescenceThicknessRange),e.transmission!==void 0&&(this.transmission=e.transmission),e.thickness!==void 0&&(this.thickness=e.thickness),e.attenuationDistance!==void 0&&(this.attenuationDistance=e.attenuationDistance),e.attenuationColor!==void 0&&this.attenuationColor!==void 0&&this.attenuationColor.setHex(e.attenuationColor),e.anisotropy!==void 0&&(this.anisotropy=e.anisotropy),e.anisotropyRotation!==void 0&&(this.anisotropyRotation=e.anisotropyRotation),e.fog!==void 0&&(this.fog=e.fog),e.flatShading!==void 0&&(this.flatShading=e.flatShading),e.blending!==void 0&&(this.blending=e.blending),e.combine!==void 0&&(this.combine=e.combine),e.side!==void 0&&(this.side=e.side),e.shadowSide!==void 0&&(this.shadowSide=e.shadowSide),e.opacity!==void 0&&(this.opacity=e.opacity),e.transparent!==void 0&&(this.transparent=e.transparent),e.alphaTest!==void 0&&(this.alphaTest=e.alphaTest),e.alphaHash!==void 0&&(this.alphaHash=e.alphaHash),e.depthFunc!==void 0&&(this.depthFunc=e.depthFunc),e.depthTest!==void 0&&(this.depthTest=e.depthTest),e.depthWrite!==void 0&&(this.depthWrite=e.depthWrite),e.colorWrite!==void 0&&(this.colorWrite=e.colorWrite),e.blendSrc!==void 0&&(this.blendSrc=e.blendSrc),e.blendDst!==void 0&&(this.blendDst=e.blendDst),e.blendEquation!==void 0&&(this.blendEquation=e.blendEquation),e.blendSrcAlpha!==void 0&&(this.blendSrcAlpha=e.blendSrcAlpha),e.blendDstAlpha!==void 0&&(this.blendDstAlpha=e.blendDstAlpha),e.blendEquationAlpha!==void 0&&(this.blendEquationAlpha=e.blendEquationAlpha),e.blendColor!==void 0&&this.blendColor!==void 0&&this.blendColor.setHex(e.blendColor),e.blendAlpha!==void 0&&(this.blendAlpha=e.blendAlpha),e.stencilWriteMask!==void 0&&(this.stencilWriteMask=e.stencilWriteMask),e.stencilFunc!==void 0&&(this.stencilFunc=e.stencilFunc),e.stencilRef!==void 0&&(this.stencilRef=e.stencilRef),e.stencilFuncMask!==void 0&&(this.stencilFuncMask=e.stencilFuncMask),e.stencilFail!==void 0&&(this.stencilFail=e.stencilFail),e.stencilZFail!==void 0&&(this.stencilZFail=e.stencilZFail),e.stencilZPass!==void 0&&(this.stencilZPass=e.stencilZPass),e.stencilWrite!==void 0&&(this.stencilWrite=e.stencilWrite),e.wireframe!==void 0&&(this.wireframe=e.wireframe),e.wireframeLinewidth!==void 0&&(this.wireframeLinewidth=e.wireframeLinewidth),e.wireframeLinecap!==void 0&&(this.wireframeLinecap=e.wireframeLinecap),e.wireframeLinejoin!==void 0&&(this.wireframeLinejoin=e.wireframeLinejoin),e.rotation!==void 0&&(this.rotation=e.rotation),e.linewidth!==void 0&&(this.linewidth=e.linewidth),e.dashSize!==void 0&&(this.dashSize=e.dashSize),e.gapSize!==void 0&&(this.gapSize=e.gapSize),e.scale!==void 0&&(this.scale=e.scale),e.polygonOffset!==void 0&&(this.polygonOffset=e.polygonOffset),e.polygonOffsetFactor!==void 0&&(this.polygonOffsetFactor=e.polygonOffsetFactor),e.polygonOffsetUnits!==void 0&&(this.polygonOffsetUnits=e.polygonOffsetUnits),e.dithering!==void 0&&(this.dithering=e.dithering),e.alphaToCoverage!==void 0&&(this.alphaToCoverage=e.alphaToCoverage),e.premultipliedAlpha!==void 0&&(this.premultipliedAlpha=e.premultipliedAlpha),e.forceSinglePass!==void 0&&(this.forceSinglePass=e.forceSinglePass),e.allowOverride!==void 0&&(this.allowOverride=e.allowOverride),e.visible!==void 0&&(this.visible=e.visible),e.toneMapped!==void 0&&(this.toneMapped=e.toneMapped),e.userData!==void 0&&(this.userData=e.userData),e.vertexColors!==void 0&&(typeof e.vertexColors=="number"?this.vertexColors=e.vertexColors>0:this.vertexColors=e.vertexColors),e.size!==void 0&&(this.size=e.size),e.sizeAttenuation!==void 0&&(this.sizeAttenuation=e.sizeAttenuation),e.map!==void 0&&(this.map=t[e.map]||null),e.matcap!==void 0&&(this.matcap=t[e.matcap]||null),e.alphaMap!==void 0&&(this.alphaMap=t[e.alphaMap]||null),e.bumpMap!==void 0&&(this.bumpMap=t[e.bumpMap]||null),e.bumpScale!==void 0&&(this.bumpScale=e.bumpScale),e.normalMap!==void 0&&(this.normalMap=t[e.normalMap]||null),e.normalMapType!==void 0&&(this.normalMapType=e.normalMapType),e.normalScale!==void 0){let i=e.normalScale;Array.isArray(i)===!1&&(i=[i,i]),this.normalScale=new Tt().fromArray(i)}return e.displacementMap!==void 0&&(this.displacementMap=t[e.displacementMap]||null),e.displacementScale!==void 0&&(this.displacementScale=e.displacementScale),e.displacementBias!==void 0&&(this.displacementBias=e.displacementBias),e.roughnessMap!==void 0&&(this.roughnessMap=t[e.roughnessMap]||null),e.metalnessMap!==void 0&&(this.metalnessMap=t[e.metalnessMap]||null),e.emissiveMap!==void 0&&(this.emissiveMap=t[e.emissiveMap]||null),e.emissiveIntensity!==void 0&&(this.emissiveIntensity=e.emissiveIntensity),e.specularMap!==void 0&&(this.specularMap=t[e.specularMap]||null),e.specularIntensityMap!==void 0&&(this.specularIntensityMap=t[e.specularIntensityMap]||null),e.specularColorMap!==void 0&&(this.specularColorMap=t[e.specularColorMap]||null),e.envMap!==void 0&&(this.envMap=t[e.envMap]||null),e.envMapRotation!==void 0&&this.envMapRotation.fromArray(e.envMapRotation),e.envMapIntensity!==void 0&&(this.envMapIntensity=e.envMapIntensity),e.reflectivity!==void 0&&(this.reflectivity=e.reflectivity),e.refractionRatio!==void 0&&(this.refractionRatio=e.refractionRatio),e.lightMap!==void 0&&(this.lightMap=t[e.lightMap]||null),e.lightMapIntensity!==void 0&&(this.lightMapIntensity=e.lightMapIntensity),e.aoMap!==void 0&&(this.aoMap=t[e.aoMap]||null),e.aoMapIntensity!==void 0&&(this.aoMapIntensity=e.aoMapIntensity),e.gradientMap!==void 0&&(this.gradientMap=t[e.gradientMap]||null),e.clearcoatMap!==void 0&&(this.clearcoatMap=t[e.clearcoatMap]||null),e.clearcoatRoughnessMap!==void 0&&(this.clearcoatRoughnessMap=t[e.clearcoatRoughnessMap]||null),e.clearcoatNormalMap!==void 0&&(this.clearcoatNormalMap=t[e.clearcoatNormalMap]||null),e.clearcoatNormalScale!==void 0&&(this.clearcoatNormalScale=new Tt().fromArray(e.clearcoatNormalScale)),e.iridescenceMap!==void 0&&(this.iridescenceMap=t[e.iridescenceMap]||null),e.iridescenceThicknessMap!==void 0&&(this.iridescenceThicknessMap=t[e.iridescenceThicknessMap]||null),e.transmissionMap!==void 0&&(this.transmissionMap=t[e.transmissionMap]||null),e.thicknessMap!==void 0&&(this.thicknessMap=t[e.thicknessMap]||null),e.anisotropyMap!==void 0&&(this.anisotropyMap=t[e.anisotropyMap]||null),e.sheenColorMap!==void 0&&(this.sheenColorMap=t[e.sheenColorMap]||null),e.sheenRoughnessMap!==void 0&&(this.sheenRoughnessMap=t[e.sheenRoughnessMap]||null),this}clone(){return new this.constructor().copy(this)}copy(e){this.name=e.name,this.blending=e.blending,this.side=e.side,this.vertexColors=e.vertexColors,this.opacity=e.opacity,this.transparent=e.transparent,this.blendSrc=e.blendSrc,this.blendDst=e.blendDst,this.blendEquation=e.blendEquation,this.blendSrcAlpha=e.blendSrcAlpha,this.blendDstAlpha=e.blendDstAlpha,this.blendEquationAlpha=e.blendEquationAlpha,this.blendColor.copy(e.blendColor),this.blendAlpha=e.blendAlpha,this.depthFunc=e.depthFunc,this.depthTest=e.depthTest,this.depthWrite=e.depthWrite,this.stencilWriteMask=e.stencilWriteMask,this.stencilFunc=e.stencilFunc,this.stencilRef=e.stencilRef,this.stencilFuncMask=e.stencilFuncMask,this.stencilFail=e.stencilFail,this.stencilZFail=e.stencilZFail,this.stencilZPass=e.stencilZPass,this.stencilWrite=e.stencilWrite;const t=e.clippingPlanes;let i=null;if(t!==null){const r=t.length;i=new Array(r);for(let s=0;s!==r;++s)i[s]=t[s].clone()}return this.clippingPlanes=i,this.clipIntersection=e.clipIntersection,this.clipShadows=e.clipShadows,this.shadowSide=e.shadowSide,this.colorWrite=e.colorWrite,this.precision=e.precision,this.polygonOffset=e.polygonOffset,this.polygonOffsetFactor=e.polygonOffsetFactor,this.polygonOffsetUnits=e.polygonOffsetUnits,this.dithering=e.dithering,this.alphaTest=e.alphaTest,this.alphaHash=e.alphaHash,this.alphaToCoverage=e.alphaToCoverage,this.premultipliedAlpha=e.premultipliedAlpha,this.forceSinglePass=e.forceSinglePass,this.allowOverride=e.allowOverride,this.visible=e.visible,this.toneMapped=e.toneMapped,this.userData=JSON.parse(JSON.stringify(e.userData)),this}dispose(){this.dispatchEvent({type:"dispose"})}set needsUpdate(e){e===!0&&this.version++}}const mi=new re,qa=new re,Cs=new re,Ni=new re,Ya=new re,Ps=new re,Ka=new re;class Ff{constructor(e=new re,t=new re(0,0,-1)){this.origin=e,this.direction=t}set(e,t){return this.origin.copy(e),this.direction.copy(t),this}copy(e){return this.origin.copy(e.origin),this.direction.copy(e.direction),this}at(e,t){return t.copy(this.origin).addScaledVector(this.direction,e)}lookAt(e){return this.direction.copy(e).sub(this.origin).normalize(),this}recast(e){return this.origin.copy(this.at(e,mi)),this}closestPointToPoint(e,t){t.subVectors(e,this.origin);const i=t.dot(this.direction);return i<0?t.copy(this.origin):t.copy(this.origin).addScaledVector(this.direction,i)}distanceToPoint(e){return Math.sqrt(this.distanceSqToPoint(e))}distanceSqToPoint(e){const t=mi.subVectors(e,this.origin).dot(this.direction);return t<0?this.origin.distanceToSquared(e):(mi.copy(this.origin).addScaledVector(this.direction,t),mi.distanceToSquared(e))}distanceSqToSegment(e,t,i,r){qa.copy(e).add(t).multiplyScalar(.5),Cs.copy(t).sub(e).normalize(),Ni.copy(this.origin).sub(qa);const s=e.distanceTo(t)*.5,a=-this.direction.dot(Cs),o=Ni.dot(this.direction),l=-Ni.dot(Cs),c=Ni.lengthSq(),p=Math.abs(1-a*a);let u,d,h,_;if(p>0)if(u=a*l-o,d=a*o-l,_=s*p,u>=0)if(d>=-_)if(d<=_){const x=1/p;u*=x,d*=x,h=u*(u+a*d+2*o)+d*(a*u+d+2*l)+c}else d=s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;else d=-s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;else d<=-_?(u=Math.max(0,-(-a*s+o)),d=u>0?-s:Math.min(Math.max(-s,-l),s),h=-u*u+d*(d+2*l)+c):d<=_?(u=0,d=Math.min(Math.max(-s,-l),s),h=d*(d+2*l)+c):(u=Math.max(0,-(a*s+o)),d=u>0?s:Math.min(Math.max(-s,-l),s),h=-u*u+d*(d+2*l)+c);else d=a>0?-s:s,u=Math.max(0,-(a*d+o)),h=-u*u+d*(d+2*l)+c;return i&&i.copy(this.origin).addScaledVector(this.direction,u),r&&r.copy(qa).addScaledVector(Cs,d),h}intersectSphere(e,t){mi.subVectors(e.center,this.origin);const i=mi.dot(this.direction),r=mi.dot(mi)-i*i,s=e.radius*e.radius;if(r>s)return null;const a=Math.sqrt(s-r),o=i-a,l=i+a;return l<0?null:o<0?this.at(l,t):this.at(o,t)}intersectsSphere(e){return e.radius<0?!1:this.distanceSqToPoint(e.center)<=e.radius*e.radius}distanceToPlane(e){const t=e.normal.dot(this.direction);if(t===0)return e.distanceToPoint(this.origin)===0?0:null;const i=-(this.origin.dot(e.normal)+e.constant)/t;return i>=0?i:null}intersectPlane(e,t){const i=this.distanceToPlane(e);return i===null?null:this.at(i,t)}intersectsPlane(e){const t=e.distanceToPoint(this.origin);return t===0||e.normal.dot(this.direction)*t<0}intersectBox(e,t){let i,r,s,a,o,l;const c=1/this.direction.x,p=1/this.direction.y,u=1/this.direction.z,d=this.origin;return c>=0?(i=(e.min.x-d.x)*c,r=(e.max.x-d.x)*c):(i=(e.max.x-d.x)*c,r=(e.min.x-d.x)*c),p>=0?(s=(e.min.y-d.y)*p,a=(e.max.y-d.y)*p):(s=(e.max.y-d.y)*p,a=(e.min.y-d.y)*p),i>a||s>r||((s>i||isNaN(i))&&(i=s),(a<r||isNaN(r))&&(r=a),u>=0?(o=(e.min.z-d.z)*u,l=(e.max.z-d.z)*u):(o=(e.max.z-d.z)*u,l=(e.min.z-d.z)*u),i>l||o>r)||((o>i||i!==i)&&(i=o),(l<r||r!==r)&&(r=l),r<0)?null:this.at(i>=0?i:r,t)}intersectsBox(e){return this.intersectBox(e,mi)!==null}intersectTriangle(e,t,i,r,s){Ya.subVectors(t,e),Ps.subVectors(i,e),Ka.crossVectors(Ya,Ps);let a=this.direction.dot(Ka),o;if(a>0){if(r)return null;o=1}else if(a<0)o=-1,a=-a;else return null;Ni.subVectors(this.origin,e);const l=o*this.direction.dot(Ps.crossVectors(Ni,Ps));if(l<0)return null;const c=o*this.direction.dot(Ya.cross(Ni));if(c<0||l+c>a)return null;const p=-o*Ni.dot(Ka);return p<0?null:this.at(p/a,s)}applyMatrix4(e){return this.origin.applyMatrix4(e),this.direction.transformDirection(e),this}equals(e){return e.origin.equals(this.origin)&&e.direction.equals(this.direction)}clone(){return new this.constructor().copy(this)}}class yd extends ma{constructor(e){super(),this.isMeshBasicMaterial=!0,this.type="MeshBasicMaterial",this.color=new Lt(16777215),this.map=null,this.lightMap=null,this.lightMapIntensity=1,this.aoMap=null,this.aoMapIntensity=1,this.specularMap=null,this.alphaMap=null,this.envMap=null,this.envMapRotation=new dr,this.combine=Qc,this.reflectivity=1,this.refractionRatio=.98,this.wireframe=!1,this.wireframeLinewidth=1,this.wireframeLinecap="round",this.wireframeLinejoin="round",this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.lightMap=e.lightMap,this.lightMapIntensity=e.lightMapIntensity,this.aoMap=e.aoMap,this.aoMapIntensity=e.aoMapIntensity,this.specularMap=e.specularMap,this.alphaMap=e.alphaMap,this.envMap=e.envMap,this.envMapRotation.copy(e.envMapRotation),this.combine=e.combine,this.reflectivity=e.reflectivity,this.refractionRatio=e.refractionRatio,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.wireframeLinecap=e.wireframeLinecap,this.wireframeLinejoin=e.wireframeLinejoin,this.fog=e.fog,this}}const Jl=new Kt,Ji=new Ff,Ls=new _l,Ql=new re,Is=new re,Ds=new re,Us=new re,$a=new re,Ns=new re,jl=new re,Fs=new re;class ri extends Tn{constructor(e=new ai,t=new yd){super(),this.isMesh=!0,this.type="Mesh",this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.count=1,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),e.morphTargetInfluences!==void 0&&(this.morphTargetInfluences=e.morphTargetInfluences.slice()),e.morphTargetDictionary!==void 0&&(this.morphTargetDictionary=Object.assign({},e.morphTargetDictionary)),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}updateMorphTargets(){const t=this.geometry.morphAttributes,i=Object.keys(t);if(i.length>0){const r=t[i[0]];if(r!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let s=0,a=r.length;s<a;s++){const o=r[s].name||String(s);this.morphTargetInfluences.push(0),this.morphTargetDictionary[o]=s}}}}getVertexPosition(e,t){const i=this.geometry,r=i.attributes.position,s=i.morphAttributes.position,a=i.morphTargetsRelative;t.fromBufferAttribute(r,e);const o=this.morphTargetInfluences;if(s&&o){Ns.set(0,0,0);for(let l=0,c=s.length;l<c;l++){const p=o[l],u=s[l];p!==0&&($a.fromBufferAttribute(u,e),a?Ns.addScaledVector($a,p):Ns.addScaledVector($a.sub(t),p))}t.add(Ns)}return t}raycast(e,t){const i=this.geometry,r=this.material,s=this.matrixWorld;r!==void 0&&(i.boundingSphere===null&&i.computeBoundingSphere(),Ls.copy(i.boundingSphere),Ls.applyMatrix4(s),Ji.copy(e.ray).recast(e.near),!(Ls.containsPoint(Ji.origin)===!1&&(Ji.intersectSphere(Ls,Ql)===null||Ji.origin.distanceToSquared(Ql)>(e.far-e.near)**2))&&(Jl.copy(s).invert(),Ji.copy(e.ray).applyMatrix4(Jl),!(i.boundingBox!==null&&Ji.intersectsBox(i.boundingBox)===!1)&&this._computeIntersections(e,t,Ji)))}_computeIntersections(e,t,i){let r;const s=this.geometry,a=this.material,o=s.index,l=s.attributes.position,c=s.attributes.uv,p=s.attributes.uv1,u=s.attributes.normal,d=s.groups,h=s.drawRange;if(o!==null)if(Array.isArray(a))for(let _=0,x=d.length;_<x;_++){const g=d[_],m=a[g.materialIndex],A=Math.max(g.start,h.start),P=Math.min(o.count,Math.min(g.start+g.count,h.start+h.count));for(let M=A,I=P;M<I;M+=3){const T=o.getX(M),D=o.getX(M+1),v=o.getX(M+2);r=Os(this,m,e,i,c,p,u,T,D,v),r&&(r.faceIndex=Math.floor(M/3),r.face.materialIndex=g.materialIndex,t.push(r))}}else{const _=Math.max(0,h.start),x=Math.min(o.count,h.start+h.count);for(let g=_,m=x;g<m;g+=3){const A=o.getX(g),P=o.getX(g+1),M=o.getX(g+2);r=Os(this,a,e,i,c,p,u,A,P,M),r&&(r.faceIndex=Math.floor(g/3),t.push(r))}}else if(l!==void 0)if(Array.isArray(a))for(let _=0,x=d.length;_<x;_++){const g=d[_],m=a[g.materialIndex],A=Math.max(g.start,h.start),P=Math.min(l.count,Math.min(g.start+g.count,h.start+h.count));for(let M=A,I=P;M<I;M+=3){const T=M,D=M+1,v=M+2;r=Os(this,m,e,i,c,p,u,T,D,v),r&&(r.faceIndex=Math.floor(M/3),r.face.materialIndex=g.materialIndex,t.push(r))}}else{const _=Math.max(0,h.start),x=Math.min(l.count,h.start+h.count);for(let g=_,m=x;g<m;g+=3){const A=g,P=g+1,M=g+2;r=Os(this,a,e,i,c,p,u,A,P,M),r&&(r.faceIndex=Math.floor(g/3),t.push(r))}}}}function Of(n,e,t,i,r,s,a,o){let l;if(e.side===_n?l=i.intersectTriangle(a,s,r,!0,o):l=i.intersectTriangle(r,s,a,e.side===Gi,o),l===null)return null;Fs.copy(o),Fs.applyMatrix4(n.matrixWorld);const c=t.ray.origin.distanceTo(Fs);return c<t.near||c>t.far?null:{distance:c,point:Fs.clone(),object:n}}function Os(n,e,t,i,r,s,a,o,l,c){n.getVertexPosition(o,Is),n.getVertexPosition(l,Ds),n.getVertexPosition(c,Us);const p=Of(n,e,t,i,Is,Ds,Us,jl);if(p){const u=new re;On.getBarycoord(jl,Is,Ds,Us,u),r&&(p.uv=On.getInterpolatedAttribute(r,o,l,c,u,new Tt)),s&&(p.uv1=On.getInterpolatedAttribute(s,o,l,c,u,new Tt)),a&&(p.normal=On.getInterpolatedAttribute(a,o,l,c,u,new re),p.normal.dot(i.direction)>0&&p.normal.multiplyScalar(-1));const d={a:o,b:l,c,normal:new re,materialIndex:0};On.getNormal(Is,Ds,Us,d.normal),p.face=d,p.barycoord=u}return p}class Bf extends cn{constructor(e=null,t=1,i=1,r,s,a,o,l,c=rn,p=rn,u,d){super(null,a,o,l,c,p,r,s,u,d),this.isDataTexture=!0,this.image={data:e,width:t,height:i},this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const Za=new re,kf=new re,zf=new ct;class tr{constructor(e=new re(1,0,0),t=0){this.isPlane=!0,this.normal=e,this.constant=t}set(e,t){return this.normal.copy(e),this.constant=t,this}setComponents(e,t,i,r){return this.normal.set(e,t,i),this.constant=r,this}setFromNormalAndCoplanarPoint(e,t){return this.normal.copy(e),this.constant=-t.dot(this.normal),this}setFromCoplanarPoints(e,t,i){const r=Za.subVectors(i,t).cross(kf.subVectors(e,t)).normalize();return this.setFromNormalAndCoplanarPoint(r,e),this}copy(e){return this.normal.copy(e.normal),this.constant=e.constant,this}normalize(){const e=1/this.normal.length();return this.normal.multiplyScalar(e),this.constant*=e,this}negate(){return this.constant*=-1,this.normal.negate(),this}distanceToPoint(e){return this.normal.dot(e)+this.constant}distanceToSphere(e){return this.distanceToPoint(e.center)-e.radius}projectPoint(e,t){return t.copy(e).addScaledVector(this.normal,-this.distanceToPoint(e))}intersectLine(e,t,i=!0){const r=e.delta(Za),s=this.normal.dot(r);if(s===0)return this.distanceToPoint(e.start)===0?t.copy(e.start):null;const a=-(e.start.dot(this.normal)+this.constant)/s;return i===!0&&(a<0||a>1)?null:t.copy(e.start).addScaledVector(r,a)}intersectsLine(e){const t=this.distanceToPoint(e.start),i=this.distanceToPoint(e.end);return t<0&&i>0||i<0&&t>0}intersectsBox(e){return e.intersectsPlane(this)}intersectsSphere(e){return e.intersectsPlane(this)}coplanarPoint(e){return e.copy(this.normal).multiplyScalar(-this.constant)}applyMatrix4(e,t){const i=t||zf.getNormalMatrix(e),r=this.coplanarPoint(Za).applyMatrix4(e),s=this.normal.applyMatrix3(i).normalize();return this.constant=-r.dot(s),this}translate(e){return this.constant-=e.dot(this.normal),this}equals(e){return e.normal.equals(this.normal)&&e.constant===this.constant}clone(){return new this.constructor().copy(this)}}const Qi=new _l,Gf=new Tt(.5,.5),Bs=new re;class Sd{constructor(e=new tr,t=new tr,i=new tr,r=new tr,s=new tr,a=new tr){this.planes=[e,t,i,r,s,a]}set(e,t,i,r,s,a){const o=this.planes;return o[0].copy(e),o[1].copy(t),o[2].copy(i),o[3].copy(r),o[4].copy(s),o[5].copy(a),this}copy(e){const t=this.planes;for(let i=0;i<6;i++)t[i].copy(e.planes[i]);return this}setFromProjectionMatrix(e,t=ti,i=!1){const r=this.planes,s=e.elements,a=s[0],o=s[1],l=s[2],c=s[3],p=s[4],u=s[5],d=s[6],h=s[7],_=s[8],x=s[9],g=s[10],m=s[11],A=s[12],P=s[13],M=s[14],I=s[15];if(r[0].setComponents(c-a,h-p,m-_,I-A).normalize(),r[1].setComponents(c+a,h+p,m+_,I+A).normalize(),r[2].setComponents(c+o,h+u,m+x,I+P).normalize(),r[3].setComponents(c-o,h-u,m-x,I-P).normalize(),i)r[4].setComponents(l,d,g,M).normalize(),r[5].setComponents(c-l,h-d,m-g,I-M).normalize();else if(r[4].setComponents(c-l,h-d,m-g,I-M).normalize(),t===ti)r[5].setComponents(c+l,h+d,m+g,I+M).normalize();else if(t===aa)r[5].setComponents(l,d,g,M).normalize();else throw new Error("THREE.Frustum.setFromProjectionMatrix(): Invalid coordinate system: "+t);return this}intersectsObject(e){if(e.boundingSphere!==void 0)e.boundingSphere===null&&e.computeBoundingSphere(),Qi.copy(e.boundingSphere).applyMatrix4(e.matrixWorld);else{const t=e.geometry;t.boundingSphere===null&&t.computeBoundingSphere(),Qi.copy(t.boundingSphere).applyMatrix4(e.matrixWorld)}return this.intersectsSphere(Qi)}intersectsSprite(e){Qi.center.set(0,0,0);const t=Gf.distanceTo(e.center);return Qi.radius=.7071067811865476+t,Qi.applyMatrix4(e.matrixWorld),this.intersectsSphere(Qi)}intersectsSphere(e){const t=this.planes,i=e.center,r=-e.radius;for(let s=0;s<6;s++)if(t[s].distanceToPoint(i)<r)return!1;return!0}intersectsBox(e){const t=this.planes;for(let i=0;i<6;i++){const r=t[i];if(Bs.x=r.normal.x>0?e.max.x:e.min.x,Bs.y=r.normal.y>0?e.max.y:e.min.y,Bs.z=r.normal.z>0?e.max.z:e.min.z,r.distanceToPoint(Bs)<0)return!1}return!0}containsPoint(e){const t=this.planes;for(let i=0;i<6;i++)if(t[i].distanceToPoint(e)<0)return!1;return!0}clone(){return new this.constructor().copy(this)}}class Md extends cn{constructor(e=[],t=lr,i,r,s,a,o,l,c,p){super(e,t,i,r,s,a,o,l,c,p),this.isCubeTexture=!0,this.flipY=!1}get images(){return this.image}set images(e){this.image=e}}class Hf extends cn{constructor(e,t,i,r,s,a,o,l,c){super(e,t,i,r,s,a,o,l,c),this.isCanvasTexture=!0,this.needsUpdate=!0}}class zr extends cn{constructor(e,t,i=ii,r,s,a,o=rn,l=rn,c,p=Ai,u=1){if(p!==Ai&&p!==sr)throw new Error("THREE.DepthTexture: format must be either THREE.DepthFormat or THREE.DepthStencilFormat");const d={width:e,height:t,depth:u};super(d,r,s,a,o,l,p,i,c),this.isDepthTexture=!0,this.flipY=!1,this.generateMipmaps=!1,this.compareFunction=null}copy(e){return super.copy(e),this.source=new gl(Object.assign({},e.image)),this.compareFunction=e.compareFunction,this}toJSON(e){const t=super.toJSON(e);return this.compareFunction!==null&&(t.compareFunction=this.compareFunction),t}}class Vf extends zr{constructor(e,t=ii,i=lr,r,s,a=rn,o=rn,l,c=Ai){const p={width:e,height:e,depth:1},u=[p,p,p,p,p,p];super(e,e,t,i,r,s,a,o,l,c),this.image=u,this.isCubeDepthTexture=!0,this.isCubeTexture=!0}get images(){return this.image}set images(e){this.image=e}}class bd extends cn{constructor(e=null){super(),this.sourceTexture=e,this.isExternalTexture=!0}copy(e){return super.copy(e),this.sourceTexture=e.sourceTexture,this}}class ps extends ai{constructor(e=1,t=1,i=1,r=1,s=1,a=1){super(),this.type="BoxGeometry",this.parameters={width:e,height:t,depth:i,widthSegments:r,heightSegments:s,depthSegments:a};const o=this;r=Math.floor(r),s=Math.floor(s),a=Math.floor(a);const l=[],c=[],p=[],u=[];let d=0,h=0;_("z","y","x",-1,-1,i,t,e,a,s,0),_("z","y","x",1,-1,i,t,-e,a,s,1),_("x","z","y",1,1,e,i,t,r,a,2),_("x","z","y",1,-1,e,i,-t,r,a,3),_("x","y","z",1,-1,e,t,i,r,s,4),_("x","y","z",-1,-1,e,t,-i,r,s,5),this.setIndex(l),this.setAttribute("position",new Ei(c,3)),this.setAttribute("normal",new Ei(p,3)),this.setAttribute("uv",new Ei(u,2));function _(x,g,m,A,P,M,I,T,D,v,S){const w=M/D,L=I/v,V=M/2,Z=I/2,k=T/2,z=D+1,C=v+1;let R=0,G=0;const Y=new re;for(let ne=0;ne<C;ne++){const de=ne*L-Z;for(let ce=0;ce<z;ce++){const J=ce*w-V;Y[x]=J*A,Y[g]=de*P,Y[m]=k,c.push(Y.x,Y.y,Y.z),Y[x]=0,Y[g]=0,Y[m]=T>0?1:-1,p.push(Y.x,Y.y,Y.z),u.push(ce/D),u.push(1-ne/v),R+=1}}for(let ne=0;ne<v;ne++)for(let de=0;de<D;de++){const ce=d+de+z*ne,J=d+de+z*(ne+1),ue=d+(de+1)+z*(ne+1),me=d+(de+1)+z*ne;l.push(ce,J,me),l.push(J,ue,me),G+=6}o.addGroup(h,G,S),h+=G,d+=R}}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new ps(e.width,e.height,e.depth,e.widthSegments,e.heightSegments,e.depthSegments)}}class ga extends ai{constructor(e=1,t=1,i=1,r=1){super(),this.type="PlaneGeometry",this.parameters={width:e,height:t,widthSegments:i,heightSegments:r};const s=e/2,a=t/2,o=Math.floor(i),l=Math.floor(r),c=o+1,p=l+1,u=e/o,d=t/l,h=[],_=[],x=[],g=[];for(let m=0;m<p;m++){const A=m*d-a;for(let P=0;P<c;P++){const M=P*u-s;_.push(M,-A,0),x.push(0,0,1),g.push(P/o),g.push(1-m/l)}}for(let m=0;m<l;m++)for(let A=0;A<o;A++){const P=A+c*m,M=A+c*(m+1),I=A+1+c*(m+1),T=A+1+c*m;h.push(P,M,T),h.push(M,I,T)}this.setIndex(h),this.setAttribute("position",new Ei(_,3)),this.setAttribute("normal",new Ei(x,3)),this.setAttribute("uv",new Ei(g,2))}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new ga(e.width,e.height,e.widthSegments,e.heightSegments)}}function Gr(n){const e={};for(const t in n){e[t]={};for(const i in n[t]){const r=n[t][i];if(ec(r))r.isRenderTargetTexture?(tt("UniformsUtils: Textures of render targets cannot be cloned via cloneUniforms() or mergeUniforms()."),e[t][i]=null):e[t][i]=r.clone();else if(Array.isArray(r))if(ec(r[0])){const s=[];for(let a=0,o=r.length;a<o;a++)s[a]=r[a].clone();e[t][i]=s}else e[t][i]=r.slice();else e[t][i]=r}}return e}function hn(n){const e={};for(let t=0;t<n.length;t++){const i=Gr(n[t]);for(const r in i)e[r]=i[r]}return e}function ec(n){return n&&(n.isColor||n.isMatrix3||n.isMatrix4||n.isVector2||n.isVector3||n.isVector4||n.isTexture||n.isQuaternion)}function Wf(n){const e=[];for(let t=0;t<n.length;t++)e.push(n[t].clone());return e}function Ed(n){const e=n.getRenderTarget();return e===null?n.outputColorSpace:e.isXRRenderTarget===!0?e.texture.colorSpace:xt.workingColorSpace}const Xf={clone:Gr,merge:hn};var qf=`void main() {
	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}`,Yf=`void main() {
	gl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );
}`;class si extends ma{constructor(e){super(),this.isShaderMaterial=!0,this.type="ShaderMaterial",this.defines={},this.uniforms={},this.uniformsGroups=[],this.vertexShader=qf,this.fragmentShader=Yf,this.linewidth=1,this.wireframe=!1,this.wireframeLinewidth=1,this.fog=!1,this.lights=!1,this.clipping=!1,this.forceSinglePass=!0,this.extensions={clipCullDistance:!1,multiDraw:!1},this.defaultAttributeValues={color:[1,1,1],uv:[0,0],uv1:[0,0]},this.index0AttributeName=void 0,this.uniformsNeedUpdate=!1,this.glslVersion=null,e!==void 0&&this.setValues(e)}copy(e){return super.copy(e),this.fragmentShader=e.fragmentShader,this.vertexShader=e.vertexShader,this.uniforms=Gr(e.uniforms),this.uniformsGroups=Wf(e.uniformsGroups),this.defines=Object.assign({},e.defines),this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.fog=e.fog,this.lights=e.lights,this.clipping=e.clipping,this.extensions=Object.assign({},e.extensions),this.glslVersion=e.glslVersion,this.defaultAttributeValues=Object.assign({},e.defaultAttributeValues),this.index0AttributeName=e.index0AttributeName,this.uniformsNeedUpdate=e.uniformsNeedUpdate,this}toJSON(e){const t=super.toJSON(e);t.glslVersion=this.glslVersion,t.uniforms={};for(const r in this.uniforms){const a=this.uniforms[r].value;a&&a.isTexture?t.uniforms[r]={type:"t",value:a.toJSON(e).uuid}:a&&a.isColor?t.uniforms[r]={type:"c",value:a.getHex()}:a&&a.isVector2?t.uniforms[r]={type:"v2",value:a.toArray()}:a&&a.isVector3?t.uniforms[r]={type:"v3",value:a.toArray()}:a&&a.isVector4?t.uniforms[r]={type:"v4",value:a.toArray()}:a&&a.isMatrix3?t.uniforms[r]={type:"m3",value:a.toArray()}:a&&a.isMatrix4?t.uniforms[r]={type:"m4",value:a.toArray()}:t.uniforms[r]={value:a}}Object.keys(this.defines).length>0&&(t.defines=this.defines),t.vertexShader=this.vertexShader,t.fragmentShader=this.fragmentShader,t.lights=this.lights,t.clipping=this.clipping;const i={};for(const r in this.extensions)this.extensions[r]===!0&&(i[r]=!0);return Object.keys(i).length>0&&(t.extensions=i),t}fromJSON(e,t){if(super.fromJSON(e,t),e.uniforms!==void 0)for(const i in e.uniforms){const r=e.uniforms[i];switch(this.uniforms[i]={},r.type){case"t":this.uniforms[i].value=t[r.value]||null;break;case"c":this.uniforms[i].value=new Lt().setHex(r.value);break;case"v2":this.uniforms[i].value=new Tt().fromArray(r.value);break;case"v3":this.uniforms[i].value=new re().fromArray(r.value);break;case"v4":this.uniforms[i].value=new Vt().fromArray(r.value);break;case"m3":this.uniforms[i].value=new ct().fromArray(r.value);break;case"m4":this.uniforms[i].value=new Kt().fromArray(r.value);break;default:this.uniforms[i].value=r.value}}if(e.defines!==void 0&&(this.defines=e.defines),e.vertexShader!==void 0&&(this.vertexShader=e.vertexShader),e.fragmentShader!==void 0&&(this.fragmentShader=e.fragmentShader),e.glslVersion!==void 0&&(this.glslVersion=e.glslVersion),e.extensions!==void 0)for(const i in e.extensions)this.extensions[i]=e.extensions[i];return e.lights!==void 0&&(this.lights=e.lights),e.clipping!==void 0&&(this.clipping=e.clipping),this}}class Td extends si{constructor(e){super(e),this.isRawShaderMaterial=!0,this.type="RawShaderMaterial"}}class Kf extends ma{constructor(e){super(),this.isMeshDepthMaterial=!0,this.type="MeshDepthMaterial",this.depthPacking=nf,this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.wireframe=!1,this.wireframeLinewidth=1,this.setValues(e)}copy(e){return super.copy(e),this.depthPacking=e.depthPacking,this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this}}class $f extends ma{constructor(e){super(),this.isMeshDistanceMaterial=!0,this.type="MeshDistanceMaterial",this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.setValues(e)}copy(e){return super.copy(e),this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this}}const ks=new re,zs=new Wr,Kn=new re;class xl extends Tn{constructor(){super(),this.isCamera=!0,this.type="Camera",this.matrixWorldInverse=new Kt,this.projectionMatrix=new Kt,this.projectionMatrixInverse=new Kt,this.coordinateSystem=ti,this._reversedDepth=!1}get reversedDepth(){return this._reversedDepth}copy(e,t){return super.copy(e,t),this.matrixWorldInverse.copy(e.matrixWorldInverse),this.projectionMatrix.copy(e.projectionMatrix),this.projectionMatrixInverse.copy(e.projectionMatrixInverse),this.coordinateSystem=e.coordinateSystem,this}getWorldDirection(e){return super.getWorldDirection(e).negate()}updateMatrixWorld(e){super.updateMatrixWorld(e),this.matrixWorld.decompose(ks,zs,Kn),Kn.x===1&&Kn.y===1&&Kn.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(ks,zs,Kn.set(1,1,1)).invert()}updateWorldMatrix(e,t,i=!1){super.updateWorldMatrix(e,t,i),this.matrixWorld.decompose(ks,zs,Kn),Kn.x===1&&Kn.y===1&&Kn.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(ks,zs,Kn.set(1,1,1)).invert()}clone(){return new this.constructor().copy(this)}}const Fi=new re,tc=new Tt,nc=new Tt;class Fn extends xl{constructor(e=50,t=1,i=.1,r=2e3){super(),this.isPerspectiveCamera=!0,this.type="PerspectiveCamera",this.fov=e,this.zoom=1,this.near=i,this.far=r,this.focus=10,this.aspect=t,this.view=null,this.filmGauge=35,this.filmOffset=0,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.fov=e.fov,this.zoom=e.zoom,this.near=e.near,this.far=e.far,this.focus=e.focus,this.aspect=e.aspect,this.view=e.view===null?null:Object.assign({},e.view),this.filmGauge=e.filmGauge,this.filmOffset=e.filmOffset,this}setFocalLength(e){const t=.5*this.getFilmHeight()/e;this.fov=Zo*2*Math.atan(t),this.updateProjectionMatrix()}getFocalLength(){const e=Math.tan(Aa*.5*this.fov);return .5*this.getFilmHeight()/e}getEffectiveFOV(){return Zo*2*Math.atan(Math.tan(Aa*.5*this.fov)/this.zoom)}getFilmWidth(){return this.filmGauge*Math.min(this.aspect,1)}getFilmHeight(){return this.filmGauge/Math.max(this.aspect,1)}getViewBounds(e,t,i){Fi.set(-1,-1,.5).applyMatrix4(this.projectionMatrixInverse),t.set(Fi.x,Fi.y).multiplyScalar(-e/Fi.z),Fi.set(1,1,.5).applyMatrix4(this.projectionMatrixInverse),i.set(Fi.x,Fi.y).multiplyScalar(-e/Fi.z)}getViewSize(e,t){return this.getViewBounds(e,tc,nc),t.subVectors(nc,tc)}setViewOffset(e,t,i,r,s,a){this.aspect=e/t,this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=i,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=this.near;let t=e*Math.tan(Aa*.5*this.fov)/this.zoom,i=2*t,r=this.aspect*i,s=-.5*r;const a=this.view;if(this.view!==null&&this.view.enabled){const l=a.fullWidth,c=a.fullHeight;s+=a.offsetX*r/l,t-=a.offsetY*i/c,r*=a.width/l,i*=a.height/c}const o=this.filmOffset;o!==0&&(s+=e*o/this.getFilmWidth()),this.projectionMatrix.makePerspective(s,s+r,t,t-i,e,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.fov=this.fov,t.object.zoom=this.zoom,t.object.near=this.near,t.object.far=this.far,t.object.focus=this.focus,t.object.aspect=this.aspect,this.view!==null&&(t.object.view=Object.assign({},this.view)),t.object.filmGauge=this.filmGauge,t.object.filmOffset=this.filmOffset,t}}class wd extends xl{constructor(e=-1,t=1,i=1,r=-1,s=.1,a=2e3){super(),this.isOrthographicCamera=!0,this.type="OrthographicCamera",this.zoom=1,this.view=null,this.left=e,this.right=t,this.top=i,this.bottom=r,this.near=s,this.far=a,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.left=e.left,this.right=e.right,this.top=e.top,this.bottom=e.bottom,this.near=e.near,this.far=e.far,this.zoom=e.zoom,this.view=e.view===null?null:Object.assign({},e.view),this}setViewOffset(e,t,i,r,s,a){this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=i,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=(this.right-this.left)/(2*this.zoom),t=(this.top-this.bottom)/(2*this.zoom),i=(this.right+this.left)/2,r=(this.top+this.bottom)/2;let s=i-e,a=i+e,o=r+t,l=r-t;if(this.view!==null&&this.view.enabled){const c=(this.right-this.left)/this.view.fullWidth/this.zoom,p=(this.top-this.bottom)/this.view.fullHeight/this.zoom;s+=c*this.view.offsetX,a=s+c*this.view.width,o-=p*this.view.offsetY,l=o-p*this.view.height}this.projectionMatrix.makeOrthographic(s,a,o,l,this.near,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.zoom=this.zoom,t.object.left=this.left,t.object.right=this.right,t.object.top=this.top,t.object.bottom=this.bottom,t.object.near=this.near,t.object.far=this.far,this.view!==null&&(t.object.view=Object.assign({},this.view)),t}}const Lr=-90,Ir=1;class Zf extends Tn{constructor(e,t,i){super(),this.type="CubeCamera",this.renderTarget=i,this.coordinateSystem=null,this.activeMipmapLevel=0;const r=new Fn(Lr,Ir,e,t);r.layers=this.layers,this.add(r);const s=new Fn(Lr,Ir,e,t);s.layers=this.layers,this.add(s);const a=new Fn(Lr,Ir,e,t);a.layers=this.layers,this.add(a);const o=new Fn(Lr,Ir,e,t);o.layers=this.layers,this.add(o);const l=new Fn(Lr,Ir,e,t);l.layers=this.layers,this.add(l);const c=new Fn(Lr,Ir,e,t);c.layers=this.layers,this.add(c)}updateCoordinateSystem(){const e=this.coordinateSystem,t=this.children.concat(),[i,r,s,a,o,l]=t;for(const c of t)this.remove(c);if(e===ti)i.up.set(0,1,0),i.lookAt(1,0,0),r.up.set(0,1,0),r.lookAt(-1,0,0),s.up.set(0,0,-1),s.lookAt(0,1,0),a.up.set(0,0,1),a.lookAt(0,-1,0),o.up.set(0,1,0),o.lookAt(0,0,1),l.up.set(0,1,0),l.lookAt(0,0,-1);else if(e===aa)i.up.set(0,-1,0),i.lookAt(-1,0,0),r.up.set(0,-1,0),r.lookAt(1,0,0),s.up.set(0,0,1),s.lookAt(0,1,0),a.up.set(0,0,-1),a.lookAt(0,-1,0),o.up.set(0,-1,0),o.lookAt(0,0,1),l.up.set(0,-1,0),l.lookAt(0,0,-1);else throw new Error("THREE.CubeCamera.updateCoordinateSystem(): Invalid coordinate system: "+e);for(const c of t)this.add(c),c.updateMatrixWorld()}update(e,t){this.parent===null&&this.updateMatrixWorld();const{renderTarget:i,activeMipmapLevel:r}=this;this.coordinateSystem!==e.coordinateSystem&&(this.coordinateSystem=e.coordinateSystem,this.updateCoordinateSystem());const[s,a,o,l,c,p]=this.children,u=e.getRenderTarget(),d=e.getActiveCubeFace(),h=e.getActiveMipmapLevel(),_=e.xr.enabled;e.xr.enabled=!1;const x=i.texture.generateMipmaps;i.texture.generateMipmaps=!1;let g=!1;e.isWebGLRenderer===!0?g=e.state.buffers.depth.getReversed():g=e.reversedDepthBuffer,e.setRenderTarget(i,0,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,s),e.setRenderTarget(i,1,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,a),e.setRenderTarget(i,2,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,o),e.setRenderTarget(i,3,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,l),e.setRenderTarget(i,4,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,c),i.texture.generateMipmaps=x,e.setRenderTarget(i,5,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,p),e.setRenderTarget(u,d,h),e.xr.enabled=_,i.texture.needsPMREMUpdate=!0}}class Jf extends Fn{constructor(e=[]){super(),this.isArrayCamera=!0,this.isMultiViewCamera=!1,this.cameras=e}}const Al=class Al{constructor(e,t,i,r){this.elements=[1,0,0,1],e!==void 0&&this.set(e,t,i,r)}identity(){return this.set(1,0,0,1),this}fromArray(e,t=0){for(let i=0;i<4;i++)this.elements[i]=e[i+t];return this}set(e,t,i,r){const s=this.elements;return s[0]=e,s[2]=t,s[1]=i,s[3]=r,this}};Al.prototype.isMatrix2=!0;let ic=Al;function rc(n,e,t,i){const r=Qf(i);switch(t){case ud:return n*e;case hd:return n*e/r.components*r.byteLength;case ul:return n*e/r.components*r.byteLength;case cr:return n*e*2/r.components*r.byteLength;case fl:return n*e*2/r.components*r.byteLength;case fd:return n*e*3/r.components*r.byteLength;case Bn:return n*e*4/r.components*r.byteLength;case hl:return n*e*4/r.components*r.byteLength;case Zs:case Js:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*8;case Qs:case js:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case xo:case So:return Math.max(n,16)*Math.max(e,8)/4;case vo:case yo:return Math.max(n,8)*Math.max(e,8)/2;case Mo:case bo:case To:case wo:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*8;case Eo:case ia:case Ao:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case Ro:return Math.floor((n+3)/4)*Math.floor((e+3)/4)*16;case Co:return Math.floor((n+4)/5)*Math.floor((e+3)/4)*16;case Po:return Math.floor((n+4)/5)*Math.floor((e+4)/5)*16;case Lo:return Math.floor((n+5)/6)*Math.floor((e+4)/5)*16;case Io:return Math.floor((n+5)/6)*Math.floor((e+5)/6)*16;case Do:return Math.floor((n+7)/8)*Math.floor((e+4)/5)*16;case Uo:return Math.floor((n+7)/8)*Math.floor((e+5)/6)*16;case No:return Math.floor((n+7)/8)*Math.floor((e+7)/8)*16;case Fo:return Math.floor((n+9)/10)*Math.floor((e+4)/5)*16;case Oo:return Math.floor((n+9)/10)*Math.floor((e+5)/6)*16;case Bo:return Math.floor((n+9)/10)*Math.floor((e+7)/8)*16;case ko:return Math.floor((n+9)/10)*Math.floor((e+9)/10)*16;case zo:return Math.floor((n+11)/12)*Math.floor((e+9)/10)*16;case Go:return Math.floor((n+11)/12)*Math.floor((e+11)/12)*16;case Ho:case Vo:case Wo:return Math.ceil(n/4)*Math.ceil(e/4)*16;case Xo:case qo:return Math.ceil(n/4)*Math.ceil(e/4)*8;case ra:case Yo:return Math.ceil(n/4)*Math.ceil(e/4)*16}throw new Error(`Unable to determine texture byte length for ${t} format.`)}function Qf(n){switch(n){case Cn:case od:return{byteLength:1,components:1};case ls:case ld:case wi:return{byteLength:2,components:1};case cl:case dl:return{byteLength:2,components:4};case ii:case ll:case ei:return{byteLength:4,components:1};case cd:case dd:return{byteLength:4,components:3}}throw new Error(`THREE.TextureUtils: Unknown texture type ${n}.`)}typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("register",{detail:{revision:ol}}));typeof window<"u"&&(window.__THREE__?tt("WARNING: Multiple instances of Three.js being imported."):window.__THREE__=ol);function Ad(){let n=null,e=!1,t=null,i=null;function r(s,a){t(s,a),i=n.requestAnimationFrame(r)}return{start:function(){e!==!0&&t!==null&&n!==null&&(i=n.requestAnimationFrame(r),e=!0)},stop:function(){n!==null&&n.cancelAnimationFrame(i),e=!1},setAnimationLoop:function(s){t=s},setContext:function(s){n=s}}}function jf(n){const e=new WeakMap;function t(o,l){const c=o.array,p=o.usage,u=c.byteLength,d=n.createBuffer();n.bindBuffer(l,d),n.bufferData(l,c,p),o.onUploadCallback();let h;if(c instanceof Float32Array)h=n.FLOAT;else if(typeof Float16Array<"u"&&c instanceof Float16Array)h=n.HALF_FLOAT;else if(c instanceof Uint16Array)o.isFloat16BufferAttribute?h=n.HALF_FLOAT:h=n.UNSIGNED_SHORT;else if(c instanceof Int16Array)h=n.SHORT;else if(c instanceof Uint32Array)h=n.UNSIGNED_INT;else if(c instanceof Int32Array)h=n.INT;else if(c instanceof Int8Array)h=n.BYTE;else if(c instanceof Uint8Array)h=n.UNSIGNED_BYTE;else if(c instanceof Uint8ClampedArray)h=n.UNSIGNED_BYTE;else throw new Error("THREE.WebGLAttributes: Unsupported buffer data format: "+c);return{buffer:d,type:h,bytesPerElement:c.BYTES_PER_ELEMENT,version:o.version,size:u}}function i(o,l,c){const p=l.array,u=l.updateRanges;if(n.bindBuffer(c,o),u.length===0)n.bufferSubData(c,0,p);else{u.sort((h,_)=>h.start-_.start);let d=0;for(let h=1;h<u.length;h++){const _=u[d],x=u[h];x.start<=_.start+_.count+1?_.count=Math.max(_.count,x.start+x.count-_.start):(++d,u[d]=x)}u.length=d+1;for(let h=0,_=u.length;h<_;h++){const x=u[h];n.bufferSubData(c,x.start*p.BYTES_PER_ELEMENT,p,x.start,x.count)}l.clearUpdateRanges()}l.onUploadCallback()}function r(o){return o.isInterleavedBufferAttribute&&(o=o.data),e.get(o)}function s(o){o.isInterleavedBufferAttribute&&(o=o.data);const l=e.get(o);l&&(n.deleteBuffer(l.buffer),e.delete(o))}function a(o,l){if(o.isInterleavedBufferAttribute&&(o=o.data),o.isGLBufferAttribute){const p=e.get(o);(!p||p.version<o.version)&&e.set(o,{buffer:o.buffer,type:o.type,bytesPerElement:o.elementSize,version:o.version});return}const c=e.get(o);if(c===void 0)e.set(o,t(o,l));else if(c.version<o.version){if(c.size!==o.array.byteLength)throw new Error("THREE.WebGLAttributes: The size of the buffer attribute's array buffer does not match the original size. Resizing buffer attributes is not supported.");i(c.buffer,o,l),c.version=o.version}}return{get:r,remove:s,update:a}}var eh=`#ifdef USE_ALPHAHASH
	if ( diffuseColor.a < getAlphaHashThreshold( vPosition ) ) discard;
#endif`,th=`#ifdef USE_ALPHAHASH
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
#endif`,nh=`#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, vAlphaMapUv ).g;
#endif`,ih=`#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,rh=`#ifdef USE_ALPHATEST
	#ifdef ALPHA_TO_COVERAGE
	diffuseColor.a = smoothstep( alphaTest, alphaTest + fwidth( diffuseColor.a ), diffuseColor.a );
	if ( diffuseColor.a == 0.0 ) discard;
	#else
	if ( diffuseColor.a < alphaTest ) discard;
	#endif
#endif`,sh=`#ifdef USE_ALPHATEST
	uniform float alphaTest;
#endif`,ah=`#ifdef USE_AOMAP
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
#endif`,oh=`#ifdef USE_AOMAP
	uniform sampler2D aoMap;
	uniform float aoMapIntensity;
#endif`,lh=`#ifdef USE_BATCHING
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
#endif`,ch=`#ifdef USE_BATCHING
	mat4 batchingMatrix = getBatchingMatrix( getIndirectIndex( gl_DrawID ) );
#endif`,dh=`vec3 transformed = vec3( position );
#ifdef USE_ALPHAHASH
	vPosition = vec3( position );
#endif`,uh=`vec3 objectNormal = vec3( normal );
#ifdef USE_TANGENT
	vec3 objectTangent = vec3( tangent.xyz );
#endif`,fh=`float G_BlinnPhong_Implicit( ) {
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
} // validated`,hh=`#ifdef USE_IRIDESCENCE
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
#endif`,ph=`#ifdef USE_BUMPMAP
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
#endif`,mh=`#if NUM_CLIPPING_PLANES > 0
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
#endif`,gh=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
	uniform vec4 clippingPlanes[ NUM_CLIPPING_PLANES ];
#endif`,_h=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
#endif`,vh=`#if NUM_CLIPPING_PLANES > 0
	vClipPosition = - mvPosition.xyz;
#endif`,xh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	diffuseColor *= vColor;
#endif`,yh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	varying vec4 vColor;
#endif`,Sh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	varying vec4 vColor;
#endif`,Mh=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
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
#endif`,bh=`#define PI 3.141592653589793
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
} // validated`,Eh=`#ifdef ENVMAP_TYPE_CUBE_UV
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
#endif`,Th=`vec3 transformedNormal = objectNormal;
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
#endif`,wh=`#ifdef USE_DISPLACEMENTMAP
	uniform sampler2D displacementMap;
	uniform float displacementScale;
	uniform float displacementBias;
#endif`,Ah=`#ifdef USE_DISPLACEMENTMAP
	transformed += normalize( objectNormal ) * ( texture2D( displacementMap, vDisplacementMapUv ).x * displacementScale + displacementBias );
#endif`,Rh=`#ifdef USE_EMISSIVEMAP
	vec4 emissiveColor = texture2D( emissiveMap, vEmissiveMapUv );
	#ifdef DECODE_VIDEO_TEXTURE_EMISSIVE
		emissiveColor = sRGBTransferEOTF( emissiveColor );
	#endif
	totalEmissiveRadiance *= emissiveColor.rgb;
#endif`,Ch=`#ifdef USE_EMISSIVEMAP
	uniform sampler2D emissiveMap;
#endif`,Ph="gl_FragColor = linearToOutputTexel( gl_FragColor );",Lh=`vec4 LinearTransferOETF( in vec4 value ) {
	return value;
}
vec4 sRGBTransferEOTF( in vec4 value ) {
	return vec4( mix( pow( value.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), value.rgb * 0.0773993808, vec3( lessThanEqual( value.rgb, vec3( 0.04045 ) ) ) ), value.a );
}
vec4 sRGBTransferOETF( in vec4 value ) {
	return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
}`,Ih=`#ifdef USE_ENVMAP
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
#endif`,Dh=`#ifdef USE_ENVMAP
	uniform float envMapIntensity;
	uniform mat3 envMapRotation;
	#ifdef ENVMAP_TYPE_CUBE
		uniform samplerCube envMap;
	#else
		uniform sampler2D envMap;
	#endif
#endif`,Uh=`#ifdef USE_ENVMAP
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
#endif`,Nh=`#ifdef USE_ENVMAP
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		
		varying vec3 vWorldPosition;
	#else
		varying vec3 vReflect;
		uniform float refractionRatio;
	#endif
#endif`,Fh=`#ifdef USE_ENVMAP
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
#endif`,Oh=`#ifdef USE_FOG
	vFogDepth = - mvPosition.z;
#endif`,Bh=`#ifdef USE_FOG
	varying float vFogDepth;
#endif`,kh=`#ifdef USE_FOG
	#ifdef FOG_EXP2
		float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
	#else
		float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
	#endif
	gl_FragColor.rgb = mix( gl_FragColor.rgb, fogColor, fogFactor );
#endif`,zh=`#ifdef USE_FOG
	uniform vec3 fogColor;
	varying float vFogDepth;
	#ifdef FOG_EXP2
		uniform float fogDensity;
	#else
		uniform float fogNear;
		uniform float fogFar;
	#endif
#endif`,Gh=`#ifdef USE_GRADIENTMAP
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
}`,Hh=`#ifdef USE_LIGHTMAP
	uniform sampler2D lightMap;
	uniform float lightMapIntensity;
#endif`,Vh=`LambertMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularStrength = specularStrength;`,Wh=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Lambert`,Xh=`uniform bool receiveShadow;
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
#include <lightprobes_pars_fragment>`,qh=`#ifdef USE_ENVMAP
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
#endif`,Yh=`ToonMaterial material;
material.diffuseColor = diffuseColor.rgb;`,Kh=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Toon`,$h=`BlinnPhongMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularColor = specular;
material.specularShininess = shininess;
material.specularStrength = specularStrength;`,Zh=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_BlinnPhong`,Jh=`PhysicalMaterial material;
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
#endif`,Qh=`uniform sampler2D dfgLUT;
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
}`,jh=`
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
#endif`,ep=`#if defined( RE_IndirectDiffuse )
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
#endif`,tp=`#if defined( RE_IndirectDiffuse )
	#if defined( LAMBERT ) || defined( PHONG )
		irradiance += iblIrradiance;
	#endif
	RE_IndirectDiffuse( irradiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif
#if defined( RE_IndirectSpecular )
	RE_IndirectSpecular( radiance, iblIrradiance, clearcoatRadiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif`,np=`#ifdef USE_LIGHT_PROBES_GRID
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
#endif`,ip=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	gl_FragDepth = vIsPerspective == 0.0 ? gl_FragCoord.z : log2( vFragDepth ) * logDepthBufFC * 0.5;
#endif`,rp=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	uniform float logDepthBufFC;
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,sp=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,ap=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	vFragDepth = 1.0 + gl_Position.w;
	vIsPerspective = float( isPerspectiveMatrix( projectionMatrix ) );
#endif`,op=`#ifdef USE_MAP
	vec4 sampledDiffuseColor = texture2D( map, vMapUv );
	#ifdef DECODE_VIDEO_TEXTURE
		sampledDiffuseColor = sRGBTransferEOTF( sampledDiffuseColor );
	#endif
	diffuseColor *= sampledDiffuseColor;
#endif`,lp=`#ifdef USE_MAP
	uniform sampler2D map;
#endif`,cp=`#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
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
#endif`,dp=`#if defined( USE_POINTS_UV )
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
#endif`,up=`float metalnessFactor = metalness;
#ifdef USE_METALNESSMAP
	vec4 texelMetalness = texture2D( metalnessMap, vMetalnessMapUv );
	metalnessFactor *= texelMetalness.b;
#endif`,fp=`#ifdef USE_METALNESSMAP
	uniform sampler2D metalnessMap;
#endif`,hp=`#ifdef USE_INSTANCING_MORPH
	float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	float morphTargetBaseInfluence = texelFetch( morphTexture, ivec2( 0, gl_InstanceID ), 0 ).r;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		morphTargetInfluences[i] =  texelFetch( morphTexture, ivec2( i + 1, gl_InstanceID ), 0 ).r;
	}
#endif`,pp=`#if defined( USE_MORPHCOLORS )
	vColor *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		#if defined( USE_COLOR_ALPHA )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ) * morphTargetInfluences[ i ];
		#elif defined( USE_COLOR )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ).rgb * morphTargetInfluences[ i ];
		#endif
	}
#endif`,mp=`#ifdef USE_MORPHNORMALS
	objectNormal *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) objectNormal += getMorph( gl_VertexID, i, 1 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,gp=`#ifdef USE_MORPHTARGETS
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
#endif`,_p=`#ifdef USE_MORPHTARGETS
	transformed *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) transformed += getMorph( gl_VertexID, i, 0 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,vp=`float faceDirection = gl_FrontFacing ? 1.0 : - 1.0;
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
vec3 nonPerturbedNormal = normal;`,xp=`#ifdef USE_NORMALMAP_OBJECTSPACE
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
#endif`,yp=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,Sp=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,Mp=`#ifndef FLAT_SHADED
	vNormal = normalize( transformedNormal );
	#ifdef USE_TANGENT
		vTangent = normalize( transformedTangent );
		vBitangent = normalize( cross( vNormal, vTangent ) * tangent.w );
		#ifdef FLIP_SIDED
			vBitangent = - vBitangent;
		#endif
	#endif
#endif`,bp=`#ifdef USE_NORMALMAP
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
#endif`,Ep=`#ifdef USE_CLEARCOAT
	vec3 clearcoatNormal = nonPerturbedNormal;
#endif`,Tp=`#ifdef USE_CLEARCOAT_NORMALMAP
	vec3 clearcoatMapN = texture2D( clearcoatNormalMap, vClearcoatNormalMapUv ).xyz * 2.0 - 1.0;
	clearcoatMapN.xy *= clearcoatNormalScale;
	clearcoatNormal = normalize( tbn2 * clearcoatMapN );
#endif`,wp=`#ifdef USE_CLEARCOATMAP
	uniform sampler2D clearcoatMap;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform sampler2D clearcoatNormalMap;
	uniform vec2 clearcoatNormalScale;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform sampler2D clearcoatRoughnessMap;
#endif`,Ap=`#ifdef USE_IRIDESCENCEMAP
	uniform sampler2D iridescenceMap;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform sampler2D iridescenceThicknessMap;
#endif`,Rp=`#ifdef OPAQUE
diffuseColor.a = 1.0;
#endif
#ifdef USE_TRANSMISSION
diffuseColor.a *= material.transmissionAlpha;
#endif
gl_FragColor = vec4( outgoingLight, diffuseColor.a );`,Cp=`vec3 packNormalToRGB( const in vec3 normal ) {
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
}`,Pp=`#ifdef PREMULTIPLIED_ALPHA
	gl_FragColor.rgb *= gl_FragColor.a;
#endif`,Lp=`vec4 mvPosition = vec4( transformed, 1.0 );
#ifdef USE_BATCHING
	mvPosition = batchingMatrix * mvPosition;
#endif
#ifdef USE_INSTANCING
	mvPosition = instanceMatrix * mvPosition;
#endif
mvPosition = modelViewMatrix * mvPosition;
gl_Position = projectionMatrix * mvPosition;`,Ip=`#ifdef DITHERING
	gl_FragColor.rgb = dithering( gl_FragColor.rgb );
#endif`,Dp=`#ifdef DITHERING
	vec3 dithering( vec3 color ) {
		float grid_position = rand( gl_FragCoord.xy );
		vec3 dither_shift_RGB = vec3( 0.25 / 255.0, -0.25 / 255.0, 0.25 / 255.0 );
		dither_shift_RGB = mix( 2.0 * dither_shift_RGB, -2.0 * dither_shift_RGB, grid_position );
		return color + dither_shift_RGB;
	}
#endif`,Up=`float roughnessFactor = roughness;
#ifdef USE_ROUGHNESSMAP
	vec4 texelRoughness = texture2D( roughnessMap, vRoughnessMapUv );
	roughnessFactor *= texelRoughness.g;
#endif`,Np=`#ifdef USE_ROUGHNESSMAP
	uniform sampler2D roughnessMap;
#endif`,Fp=`#if NUM_SPOT_LIGHT_COORDS > 0
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
#endif`,Op=`#if NUM_SPOT_LIGHT_COORDS > 0
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
#endif`,Bp=`#if ( defined( USE_SHADOWMAP ) && ( NUM_DIR_LIGHT_SHADOWS > 0 || NUM_POINT_LIGHT_SHADOWS > 0 ) ) || ( NUM_SPOT_LIGHT_COORDS > 0 )
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
#endif`,kp=`float getShadowMask() {
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
}`,zp=`#ifdef USE_SKINNING
	mat4 boneMatX = getBoneMatrix( skinIndex.x );
	mat4 boneMatY = getBoneMatrix( skinIndex.y );
	mat4 boneMatZ = getBoneMatrix( skinIndex.z );
	mat4 boneMatW = getBoneMatrix( skinIndex.w );
#endif`,Gp=`#ifdef USE_SKINNING
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
#endif`,Hp=`#ifdef USE_SKINNING
	vec4 skinVertex = bindMatrix * vec4( transformed, 1.0 );
	vec4 skinned = vec4( 0.0 );
	skinned += boneMatX * skinVertex * skinWeight.x;
	skinned += boneMatY * skinVertex * skinWeight.y;
	skinned += boneMatZ * skinVertex * skinWeight.z;
	skinned += boneMatW * skinVertex * skinWeight.w;
	transformed = ( bindMatrixInverse * skinned ).xyz;
#endif`,Vp=`#ifdef USE_SKINNING
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
#endif`,Wp=`float specularStrength;
#ifdef USE_SPECULARMAP
	vec4 texelSpecular = texture2D( specularMap, vSpecularMapUv );
	specularStrength = texelSpecular.r;
#else
	specularStrength = 1.0;
#endif`,Xp=`#ifdef USE_SPECULARMAP
	uniform sampler2D specularMap;
#endif`,qp=`#if defined( TONE_MAPPING )
	gl_FragColor.rgb = toneMapping( gl_FragColor.rgb );
#endif`,Yp=`#ifndef saturate
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
vec3 CustomToneMapping( vec3 color ) { return color; }`,Kp=`#ifdef USE_TRANSMISSION
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
#endif`,$p=`#ifdef USE_TRANSMISSION
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
#endif`,Zp=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,Jp=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,Qp=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,jp=`#if defined( USE_ENVMAP ) || defined( DISTANCE ) || defined ( USE_SHADOWMAP ) || defined ( USE_TRANSMISSION ) || NUM_SPOT_LIGHT_COORDS > 0
	vec4 worldPosition = vec4( transformed, 1.0 );
	#ifdef USE_BATCHING
		worldPosition = batchingMatrix * worldPosition;
	#endif
	#ifdef USE_INSTANCING
		worldPosition = instanceMatrix * worldPosition;
	#endif
	worldPosition = modelMatrix * worldPosition;
#endif`;const em=`varying vec2 vUv;
uniform mat3 uvTransform;
void main() {
	vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	gl_Position = vec4( position.xy, 1.0, 1.0 );
}`,tm=`uniform sampler2D t2D;
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
}`,nm=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,im=`#ifdef ENVMAP_TYPE_CUBE
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
}`,rm=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,sm=`uniform samplerCube tCube;
uniform float tFlip;
uniform float opacity;
varying vec3 vWorldDirection;
void main() {
	vec4 texColor = textureCube( tCube, vec3( tFlip * vWorldDirection.x, vWorldDirection.yz ) );
	gl_FragColor = texColor;
	gl_FragColor.a *= opacity;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,am=`#include <common>
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
}`,om=`#if DEPTH_PACKING == 3200
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
}`,lm=`#define DISTANCE
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
}`,cm=`#define DISTANCE
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
}`,dm=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
}`,um=`uniform sampler2D tEquirect;
varying vec3 vWorldDirection;
#include <common>
void main() {
	vec3 direction = normalize( vWorldDirection );
	vec2 sampleUV = equirectUv( direction );
	gl_FragColor = texture2D( tEquirect, sampleUV );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,fm=`uniform float scale;
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
}`,hm=`uniform vec3 diffuse;
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
}`,pm=`#include <common>
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
}`,mm=`uniform vec3 diffuse;
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
}`,gm=`#define LAMBERT
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
}`,_m=`#define LAMBERT
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
}`,vm=`#define MATCAP
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
}`,xm=`#define MATCAP
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
}`,ym=`#define NORMAL
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
}`,Sm=`#define NORMAL
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
}`,Mm=`#define PHONG
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
}`,bm=`#define PHONG
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
}`,Em=`#define STANDARD
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
}`,Tm=`#define STANDARD
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
}`,wm=`#define TOON
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
}`,Am=`#define TOON
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
}`,Rm=`uniform float size;
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
}`,Cm=`uniform vec3 diffuse;
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
}`,Pm=`#include <common>
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
}`,Lm=`uniform vec3 color;
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
}`,Im=`uniform float rotation;
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
}`,Dm=`uniform vec3 diffuse;
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
}`,pt={alphahash_fragment:eh,alphahash_pars_fragment:th,alphamap_fragment:nh,alphamap_pars_fragment:ih,alphatest_fragment:rh,alphatest_pars_fragment:sh,aomap_fragment:ah,aomap_pars_fragment:oh,batching_pars_vertex:lh,batching_vertex:ch,begin_vertex:dh,beginnormal_vertex:uh,bsdfs:fh,iridescence_fragment:hh,bumpmap_pars_fragment:ph,clipping_planes_fragment:mh,clipping_planes_pars_fragment:gh,clipping_planes_pars_vertex:_h,clipping_planes_vertex:vh,color_fragment:xh,color_pars_fragment:yh,color_pars_vertex:Sh,color_vertex:Mh,common:bh,cube_uv_reflection_fragment:Eh,defaultnormal_vertex:Th,displacementmap_pars_vertex:wh,displacementmap_vertex:Ah,emissivemap_fragment:Rh,emissivemap_pars_fragment:Ch,colorspace_fragment:Ph,colorspace_pars_fragment:Lh,envmap_fragment:Ih,envmap_common_pars_fragment:Dh,envmap_pars_fragment:Uh,envmap_pars_vertex:Nh,envmap_physical_pars_fragment:qh,envmap_vertex:Fh,fog_vertex:Oh,fog_pars_vertex:Bh,fog_fragment:kh,fog_pars_fragment:zh,gradientmap_pars_fragment:Gh,lightmap_pars_fragment:Hh,lights_lambert_fragment:Vh,lights_lambert_pars_fragment:Wh,lights_pars_begin:Xh,lights_toon_fragment:Yh,lights_toon_pars_fragment:Kh,lights_phong_fragment:$h,lights_phong_pars_fragment:Zh,lights_physical_fragment:Jh,lights_physical_pars_fragment:Qh,lights_fragment_begin:jh,lights_fragment_maps:ep,lights_fragment_end:tp,lightprobes_pars_fragment:np,logdepthbuf_fragment:ip,logdepthbuf_pars_fragment:rp,logdepthbuf_pars_vertex:sp,logdepthbuf_vertex:ap,map_fragment:op,map_pars_fragment:lp,map_particle_fragment:cp,map_particle_pars_fragment:dp,metalnessmap_fragment:up,metalnessmap_pars_fragment:fp,morphinstance_vertex:hp,morphcolor_vertex:pp,morphnormal_vertex:mp,morphtarget_pars_vertex:gp,morphtarget_vertex:_p,normal_fragment_begin:vp,normal_fragment_maps:xp,normal_pars_fragment:yp,normal_pars_vertex:Sp,normal_vertex:Mp,normalmap_pars_fragment:bp,clearcoat_normal_fragment_begin:Ep,clearcoat_normal_fragment_maps:Tp,clearcoat_pars_fragment:wp,iridescence_pars_fragment:Ap,opaque_fragment:Rp,packing:Cp,premultiplied_alpha_fragment:Pp,project_vertex:Lp,dithering_fragment:Ip,dithering_pars_fragment:Dp,roughnessmap_fragment:Up,roughnessmap_pars_fragment:Np,shadowmap_pars_fragment:Fp,shadowmap_pars_vertex:Op,shadowmap_vertex:Bp,shadowmask_pars_fragment:kp,skinbase_vertex:zp,skinning_pars_vertex:Gp,skinning_vertex:Hp,skinnormal_vertex:Vp,specularmap_fragment:Wp,specularmap_pars_fragment:Xp,tonemapping_fragment:qp,tonemapping_pars_fragment:Yp,transmission_fragment:Kp,transmission_pars_fragment:$p,uv_pars_fragment:Zp,uv_pars_vertex:Jp,uv_vertex:Qp,worldpos_vertex:jp,background_vert:em,background_frag:tm,backgroundCube_vert:nm,backgroundCube_frag:im,cube_vert:rm,cube_frag:sm,depth_vert:am,depth_frag:om,distance_vert:lm,distance_frag:cm,equirect_vert:dm,equirect_frag:um,linedashed_vert:fm,linedashed_frag:hm,meshbasic_vert:pm,meshbasic_frag:mm,meshlambert_vert:gm,meshlambert_frag:_m,meshmatcap_vert:vm,meshmatcap_frag:xm,meshnormal_vert:ym,meshnormal_frag:Sm,meshphong_vert:Mm,meshphong_frag:bm,meshphysical_vert:Em,meshphysical_frag:Tm,meshtoon_vert:wm,meshtoon_frag:Am,points_vert:Rm,points_frag:Cm,shadow_vert:Pm,shadow_frag:Lm,sprite_vert:Im,sprite_frag:Dm},De={common:{diffuse:{value:new Lt(16777215)},opacity:{value:1},map:{value:null},mapTransform:{value:new ct},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0}},specularmap:{specularMap:{value:null},specularMapTransform:{value:new ct}},envmap:{envMap:{value:null},envMapRotation:{value:new ct},reflectivity:{value:1},ior:{value:1.5},refractionRatio:{value:.98},dfgLUT:{value:null}},aomap:{aoMap:{value:null},aoMapIntensity:{value:1},aoMapTransform:{value:new ct}},lightmap:{lightMap:{value:null},lightMapIntensity:{value:1},lightMapTransform:{value:new ct}},bumpmap:{bumpMap:{value:null},bumpMapTransform:{value:new ct},bumpScale:{value:1}},normalmap:{normalMap:{value:null},normalMapTransform:{value:new ct},normalScale:{value:new Tt(1,1)}},displacementmap:{displacementMap:{value:null},displacementMapTransform:{value:new ct},displacementScale:{value:1},displacementBias:{value:0}},emissivemap:{emissiveMap:{value:null},emissiveMapTransform:{value:new ct}},metalnessmap:{metalnessMap:{value:null},metalnessMapTransform:{value:new ct}},roughnessmap:{roughnessMap:{value:null},roughnessMapTransform:{value:new ct}},gradientmap:{gradientMap:{value:null}},fog:{fogDensity:{value:25e-5},fogNear:{value:1},fogFar:{value:2e3},fogColor:{value:new Lt(16777215)}},lights:{ambientLightColor:{value:[]},lightProbe:{value:[]},directionalLights:{value:[],properties:{direction:{},color:{}}},directionalLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},directionalShadowMatrix:{value:[]},spotLights:{value:[],properties:{color:{},position:{},direction:{},distance:{},coneCos:{},penumbraCos:{},decay:{}}},spotLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},spotLightMap:{value:[]},spotLightMatrix:{value:[]},pointLights:{value:[],properties:{color:{},position:{},decay:{},distance:{}}},pointLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{},shadowCameraNear:{},shadowCameraFar:{}}},pointShadowMatrix:{value:[]},hemisphereLights:{value:[],properties:{direction:{},skyColor:{},groundColor:{}}},rectAreaLights:{value:[],properties:{color:{},position:{},width:{},height:{}}},ltc_1:{value:null},ltc_2:{value:null},probesSH:{value:null},probesMin:{value:new re},probesMax:{value:new re},probesResolution:{value:new re}},points:{diffuse:{value:new Lt(16777215)},opacity:{value:1},size:{value:1},scale:{value:1},map:{value:null},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0},uvTransform:{value:new ct}},sprite:{diffuse:{value:new Lt(16777215)},opacity:{value:1},center:{value:new Tt(.5,.5)},rotation:{value:0},map:{value:null},mapTransform:{value:new ct},alphaMap:{value:null},alphaMapTransform:{value:new ct},alphaTest:{value:0}}},Jn={basic:{uniforms:hn([De.common,De.specularmap,De.envmap,De.aomap,De.lightmap,De.fog]),vertexShader:pt.meshbasic_vert,fragmentShader:pt.meshbasic_frag},lambert:{uniforms:hn([De.common,De.specularmap,De.envmap,De.aomap,De.lightmap,De.emissivemap,De.bumpmap,De.normalmap,De.displacementmap,De.fog,De.lights,{emissive:{value:new Lt(0)},envMapIntensity:{value:1}}]),vertexShader:pt.meshlambert_vert,fragmentShader:pt.meshlambert_frag},phong:{uniforms:hn([De.common,De.specularmap,De.envmap,De.aomap,De.lightmap,De.emissivemap,De.bumpmap,De.normalmap,De.displacementmap,De.fog,De.lights,{emissive:{value:new Lt(0)},specular:{value:new Lt(1118481)},shininess:{value:30},envMapIntensity:{value:1}}]),vertexShader:pt.meshphong_vert,fragmentShader:pt.meshphong_frag},standard:{uniforms:hn([De.common,De.envmap,De.aomap,De.lightmap,De.emissivemap,De.bumpmap,De.normalmap,De.displacementmap,De.roughnessmap,De.metalnessmap,De.fog,De.lights,{emissive:{value:new Lt(0)},roughness:{value:1},metalness:{value:0},envMapIntensity:{value:1}}]),vertexShader:pt.meshphysical_vert,fragmentShader:pt.meshphysical_frag},toon:{uniforms:hn([De.common,De.aomap,De.lightmap,De.emissivemap,De.bumpmap,De.normalmap,De.displacementmap,De.gradientmap,De.fog,De.lights,{emissive:{value:new Lt(0)}}]),vertexShader:pt.meshtoon_vert,fragmentShader:pt.meshtoon_frag},matcap:{uniforms:hn([De.common,De.bumpmap,De.normalmap,De.displacementmap,De.fog,{matcap:{value:null}}]),vertexShader:pt.meshmatcap_vert,fragmentShader:pt.meshmatcap_frag},points:{uniforms:hn([De.points,De.fog]),vertexShader:pt.points_vert,fragmentShader:pt.points_frag},dashed:{uniforms:hn([De.common,De.fog,{scale:{value:1},dashSize:{value:1},totalSize:{value:2}}]),vertexShader:pt.linedashed_vert,fragmentShader:pt.linedashed_frag},depth:{uniforms:hn([De.common,De.displacementmap]),vertexShader:pt.depth_vert,fragmentShader:pt.depth_frag},normal:{uniforms:hn([De.common,De.bumpmap,De.normalmap,De.displacementmap,{opacity:{value:1}}]),vertexShader:pt.meshnormal_vert,fragmentShader:pt.meshnormal_frag},sprite:{uniforms:hn([De.sprite,De.fog]),vertexShader:pt.sprite_vert,fragmentShader:pt.sprite_frag},background:{uniforms:{uvTransform:{value:new ct},t2D:{value:null},backgroundIntensity:{value:1}},vertexShader:pt.background_vert,fragmentShader:pt.background_frag},backgroundCube:{uniforms:{envMap:{value:null},backgroundBlurriness:{value:0},backgroundIntensity:{value:1},backgroundRotation:{value:new ct}},vertexShader:pt.backgroundCube_vert,fragmentShader:pt.backgroundCube_frag},cube:{uniforms:{tCube:{value:null},tFlip:{value:-1},opacity:{value:1}},vertexShader:pt.cube_vert,fragmentShader:pt.cube_frag},equirect:{uniforms:{tEquirect:{value:null}},vertexShader:pt.equirect_vert,fragmentShader:pt.equirect_frag},distance:{uniforms:hn([De.common,De.displacementmap,{referencePosition:{value:new re},nearDistance:{value:1},farDistance:{value:1e3}}]),vertexShader:pt.distance_vert,fragmentShader:pt.distance_frag},shadow:{uniforms:hn([De.lights,De.fog,{color:{value:new Lt(0)},opacity:{value:1}}]),vertexShader:pt.shadow_vert,fragmentShader:pt.shadow_frag}};Jn.physical={uniforms:hn([Jn.standard.uniforms,{clearcoat:{value:0},clearcoatMap:{value:null},clearcoatMapTransform:{value:new ct},clearcoatNormalMap:{value:null},clearcoatNormalMapTransform:{value:new ct},clearcoatNormalScale:{value:new Tt(1,1)},clearcoatRoughness:{value:0},clearcoatRoughnessMap:{value:null},clearcoatRoughnessMapTransform:{value:new ct},dispersion:{value:0},iridescence:{value:0},iridescenceMap:{value:null},iridescenceMapTransform:{value:new ct},iridescenceIOR:{value:1.3},iridescenceThicknessMinimum:{value:100},iridescenceThicknessMaximum:{value:400},iridescenceThicknessMap:{value:null},iridescenceThicknessMapTransform:{value:new ct},sheen:{value:0},sheenColor:{value:new Lt(0)},sheenColorMap:{value:null},sheenColorMapTransform:{value:new ct},sheenRoughness:{value:1},sheenRoughnessMap:{value:null},sheenRoughnessMapTransform:{value:new ct},transmission:{value:0},transmissionMap:{value:null},transmissionMapTransform:{value:new ct},transmissionSamplerSize:{value:new Tt},transmissionSamplerMap:{value:null},thickness:{value:0},thicknessMap:{value:null},thicknessMapTransform:{value:new ct},attenuationDistance:{value:0},attenuationColor:{value:new Lt(0)},specularColor:{value:new Lt(1,1,1)},specularColorMap:{value:null},specularColorMapTransform:{value:new ct},specularIntensity:{value:1},specularIntensityMap:{value:null},specularIntensityMapTransform:{value:new ct},anisotropyVector:{value:new Tt},anisotropyMap:{value:null},anisotropyMapTransform:{value:new ct}}]),vertexShader:pt.meshphysical_vert,fragmentShader:pt.meshphysical_frag};const Gs={r:0,b:0,g:0},Um=new Kt,Rd=new ct;Rd.set(-1,0,0,0,1,0,0,0,1);function Nm(n,e,t,i,r,s){const a=new Lt(0);let o=r===!0?0:1,l,c,p=null,u=0,d=null;function h(A){let P=A.isScene===!0?A.background:null;if(P&&P.isTexture){const M=A.backgroundBlurriness>0;P=e.get(P,M)}return P}function _(A){let P=!1;const M=h(A);M===null?g(a,o):M&&M.isColor&&(g(M,1),P=!0);const I=n.xr.getEnvironmentBlendMode();I==="additive"?t.buffers.color.setClear(0,0,0,1,s):I==="alpha-blend"&&t.buffers.color.setClear(0,0,0,0,s),(n.autoClear||P)&&(t.buffers.depth.setTest(!0),t.buffers.depth.setMask(!0),t.buffers.color.setMask(!0),n.clear(n.autoClearColor,n.autoClearDepth,n.autoClearStencil))}function x(A,P){const M=h(P);M&&(M.isCubeTexture||M.mapping===pa)?(c===void 0&&(c=new ri(new ps(1,1,1),new si({name:"BackgroundCubeMaterial",uniforms:Gr(Jn.backgroundCube.uniforms),vertexShader:Jn.backgroundCube.vertexShader,fragmentShader:Jn.backgroundCube.fragmentShader,side:_n,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),c.geometry.deleteAttribute("normal"),c.geometry.deleteAttribute("uv"),c.onBeforeRender=function(I,T,D){this.matrixWorld.copyPosition(D.matrixWorld)},Object.defineProperty(c.material,"envMap",{get:function(){return this.uniforms.envMap.value}}),i.update(c)),c.material.uniforms.envMap.value=M,c.material.uniforms.backgroundBlurriness.value=P.backgroundBlurriness,c.material.uniforms.backgroundIntensity.value=P.backgroundIntensity,c.material.uniforms.backgroundRotation.value.setFromMatrix4(Um.makeRotationFromEuler(P.backgroundRotation)).transpose(),M.isCubeTexture&&M.isRenderTargetTexture===!1&&c.material.uniforms.backgroundRotation.value.premultiply(Rd),c.material.toneMapped=xt.getTransfer(M.colorSpace)!==It,(p!==M||u!==M.version||d!==n.toneMapping)&&(c.material.needsUpdate=!0,p=M,u=M.version,d=n.toneMapping),c.layers.enableAll(),A.unshift(c,c.geometry,c.material,0,0,null)):M&&M.isTexture&&(l===void 0&&(l=new ri(new ga(2,2),new si({name:"BackgroundMaterial",uniforms:Gr(Jn.background.uniforms),vertexShader:Jn.background.vertexShader,fragmentShader:Jn.background.fragmentShader,side:Gi,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),l.geometry.deleteAttribute("normal"),Object.defineProperty(l.material,"map",{get:function(){return this.uniforms.t2D.value}}),i.update(l)),l.material.uniforms.t2D.value=M,l.material.uniforms.backgroundIntensity.value=P.backgroundIntensity,l.material.toneMapped=xt.getTransfer(M.colorSpace)!==It,M.matrixAutoUpdate===!0&&M.updateMatrix(),l.material.uniforms.uvTransform.value.copy(M.matrix),(p!==M||u!==M.version||d!==n.toneMapping)&&(l.material.needsUpdate=!0,p=M,u=M.version,d=n.toneMapping),l.layers.enableAll(),A.unshift(l,l.geometry,l.material,0,0,null))}function g(A,P){A.getRGB(Gs,Ed(n)),t.buffers.color.setClear(Gs.r,Gs.g,Gs.b,P,s)}function m(){c!==void 0&&(c.geometry.dispose(),c.material.dispose(),c=void 0),l!==void 0&&(l.geometry.dispose(),l.material.dispose(),l=void 0)}return{getClearColor:function(){return a},setClearColor:function(A,P=1){a.set(A),o=P,g(a,o)},getClearAlpha:function(){return o},setClearAlpha:function(A){o=A,g(a,o)},render:_,addToRenderList:x,dispose:m}}function Fm(n,e){const t=n.getParameter(n.MAX_VERTEX_ATTRIBS),i={},r=d(null);let s=r,a=!1;function o(L,V,Z,k,z){let C=!1;const R=u(L,k,Z,V);s!==R&&(s=R,c(s.object)),C=h(L,k,Z,z),C&&_(L,k,Z,z),z!==null&&e.update(z,n.ELEMENT_ARRAY_BUFFER),(C||a)&&(a=!1,M(L,V,Z,k),z!==null&&n.bindBuffer(n.ELEMENT_ARRAY_BUFFER,e.get(z).buffer))}function l(){return n.createVertexArray()}function c(L){return n.bindVertexArray(L)}function p(L){return n.deleteVertexArray(L)}function u(L,V,Z,k){const z=k.wireframe===!0;let C=i[V.id];C===void 0&&(C={},i[V.id]=C);const R=L.isInstancedMesh===!0?L.id:0;let G=C[R];G===void 0&&(G={},C[R]=G);let Y=G[Z.id];Y===void 0&&(Y={},G[Z.id]=Y);let ne=Y[z];return ne===void 0&&(ne=d(l()),Y[z]=ne),ne}function d(L){const V=[],Z=[],k=[];for(let z=0;z<t;z++)V[z]=0,Z[z]=0,k[z]=0;return{geometry:null,program:null,wireframe:!1,newAttributes:V,enabledAttributes:Z,attributeDivisors:k,object:L,attributes:{},index:null}}function h(L,V,Z,k){const z=s.attributes,C=V.attributes;let R=0;const G=Z.getAttributes();for(const Y in G)if(G[Y].location>=0){const de=z[Y];let ce=C[Y];if(ce===void 0&&(Y==="instanceMatrix"&&L.instanceMatrix&&(ce=L.instanceMatrix),Y==="instanceColor"&&L.instanceColor&&(ce=L.instanceColor)),de===void 0||de.attribute!==ce||ce&&de.data!==ce.data)return!0;R++}return s.attributesNum!==R||s.index!==k}function _(L,V,Z,k){const z={},C=V.attributes;let R=0;const G=Z.getAttributes();for(const Y in G)if(G[Y].location>=0){let de=C[Y];de===void 0&&(Y==="instanceMatrix"&&L.instanceMatrix&&(de=L.instanceMatrix),Y==="instanceColor"&&L.instanceColor&&(de=L.instanceColor));const ce={};ce.attribute=de,de&&de.data&&(ce.data=de.data),z[Y]=ce,R++}s.attributes=z,s.attributesNum=R,s.index=k}function x(){const L=s.newAttributes;for(let V=0,Z=L.length;V<Z;V++)L[V]=0}function g(L){m(L,0)}function m(L,V){const Z=s.newAttributes,k=s.enabledAttributes,z=s.attributeDivisors;Z[L]=1,k[L]===0&&(n.enableVertexAttribArray(L),k[L]=1),z[L]!==V&&(n.vertexAttribDivisor(L,V),z[L]=V)}function A(){const L=s.newAttributes,V=s.enabledAttributes;for(let Z=0,k=V.length;Z<k;Z++)V[Z]!==L[Z]&&(n.disableVertexAttribArray(Z),V[Z]=0)}function P(L,V,Z,k,z,C,R){R===!0?n.vertexAttribIPointer(L,V,Z,z,C):n.vertexAttribPointer(L,V,Z,k,z,C)}function M(L,V,Z,k){x();const z=k.attributes,C=Z.getAttributes(),R=V.defaultAttributeValues;for(const G in C){const Y=C[G];if(Y.location>=0){let ne=z[G];if(ne===void 0&&(G==="instanceMatrix"&&L.instanceMatrix&&(ne=L.instanceMatrix),G==="instanceColor"&&L.instanceColor&&(ne=L.instanceColor)),ne!==void 0){const de=ne.normalized,ce=ne.itemSize,J=e.get(ne);if(J===void 0)continue;const ue=J.buffer,me=J.type,B=J.bytesPerElement,oe=me===n.INT||me===n.UNSIGNED_INT||ne.gpuType===ll;if(ne.isInterleavedBufferAttribute){const $=ne.data,se=$.stride,_e=ne.offset;if($.isInstancedInterleavedBuffer){for(let Me=0;Me<Y.locationSize;Me++)m(Y.location+Me,$.meshPerAttribute);L.isInstancedMesh!==!0&&k._maxInstanceCount===void 0&&(k._maxInstanceCount=$.meshPerAttribute*$.count)}else for(let Me=0;Me<Y.locationSize;Me++)g(Y.location+Me);n.bindBuffer(n.ARRAY_BUFFER,ue);for(let Me=0;Me<Y.locationSize;Me++)P(Y.location+Me,ce/Y.locationSize,me,de,se*B,(_e+ce/Y.locationSize*Me)*B,oe)}else{if(ne.isInstancedBufferAttribute){for(let $=0;$<Y.locationSize;$++)m(Y.location+$,ne.meshPerAttribute);L.isInstancedMesh!==!0&&k._maxInstanceCount===void 0&&(k._maxInstanceCount=ne.meshPerAttribute*ne.count)}else for(let $=0;$<Y.locationSize;$++)g(Y.location+$);n.bindBuffer(n.ARRAY_BUFFER,ue);for(let $=0;$<Y.locationSize;$++)P(Y.location+$,ce/Y.locationSize,me,de,ce*B,ce/Y.locationSize*$*B,oe)}}else if(R!==void 0){const de=R[G];if(de!==void 0)switch(de.length){case 2:n.vertexAttrib2fv(Y.location,de);break;case 3:n.vertexAttrib3fv(Y.location,de);break;case 4:n.vertexAttrib4fv(Y.location,de);break;default:n.vertexAttrib1fv(Y.location,de)}}}}A()}function I(){S();for(const L in i){const V=i[L];for(const Z in V){const k=V[Z];for(const z in k){const C=k[z];for(const R in C)p(C[R].object),delete C[R];delete k[z]}}delete i[L]}}function T(L){if(i[L.id]===void 0)return;const V=i[L.id];for(const Z in V){const k=V[Z];for(const z in k){const C=k[z];for(const R in C)p(C[R].object),delete C[R];delete k[z]}}delete i[L.id]}function D(L){for(const V in i){const Z=i[V];for(const k in Z){const z=Z[k];if(z[L.id]===void 0)continue;const C=z[L.id];for(const R in C)p(C[R].object),delete C[R];delete z[L.id]}}}function v(L){for(const V in i){const Z=i[V],k=L.isInstancedMesh===!0?L.id:0,z=Z[k];if(z!==void 0){for(const C in z){const R=z[C];for(const G in R)p(R[G].object),delete R[G];delete z[C]}delete Z[k],Object.keys(Z).length===0&&delete i[V]}}}function S(){w(),a=!0,s!==r&&(s=r,c(s.object))}function w(){r.geometry=null,r.program=null,r.wireframe=!1}return{setup:o,reset:S,resetDefaultState:w,dispose:I,releaseStatesOfGeometry:T,releaseStatesOfObject:v,releaseStatesOfProgram:D,initAttributes:x,enableAttribute:g,disableUnusedAttributes:A}}function Om(n,e,t){let i;function r(l){i=l}function s(l,c){n.drawArrays(i,l,c),t.update(c,i,1)}function a(l,c,p){p!==0&&(n.drawArraysInstanced(i,l,c,p),t.update(c,i,p))}function o(l,c,p){if(p===0)return;e.get("WEBGL_multi_draw").multiDrawArraysWEBGL(i,l,0,c,0,p);let d=0;for(let h=0;h<p;h++)d+=c[h];t.update(d,i,1)}this.setMode=r,this.render=s,this.renderInstances=a,this.renderMultiDraw=o}function Bm(n,e,t,i){let r;function s(){if(r!==void 0)return r;if(e.has("EXT_texture_filter_anisotropic")===!0){const D=e.get("EXT_texture_filter_anisotropic");r=n.getParameter(D.MAX_TEXTURE_MAX_ANISOTROPY_EXT)}else r=0;return r}function a(D){return!(D!==Bn&&i.convert(D)!==n.getParameter(n.IMPLEMENTATION_COLOR_READ_FORMAT))}function o(D){const v=D===wi&&(e.has("EXT_color_buffer_half_float")||e.has("EXT_color_buffer_float"));return!(D!==Cn&&i.convert(D)!==n.getParameter(n.IMPLEMENTATION_COLOR_READ_TYPE)&&D!==ei&&!v)}function l(D){if(D==="highp"){if(n.getShaderPrecisionFormat(n.VERTEX_SHADER,n.HIGH_FLOAT).precision>0&&n.getShaderPrecisionFormat(n.FRAGMENT_SHADER,n.HIGH_FLOAT).precision>0)return"highp";D="mediump"}return D==="mediump"&&n.getShaderPrecisionFormat(n.VERTEX_SHADER,n.MEDIUM_FLOAT).precision>0&&n.getShaderPrecisionFormat(n.FRAGMENT_SHADER,n.MEDIUM_FLOAT).precision>0?"mediump":"lowp"}let c=t.precision!==void 0?t.precision:"highp";const p=l(c);p!==c&&(tt("WebGLRenderer:",c,"not supported, using",p,"instead."),c=p);const u=t.logarithmicDepthBuffer===!0,d=t.reversedDepthBuffer===!0&&e.has("EXT_clip_control");t.reversedDepthBuffer===!0&&d===!1&&tt("WebGLRenderer: Unable to use reversed depth buffer due to missing EXT_clip_control extension. Fallback to default depth buffer.");const h=n.getParameter(n.MAX_TEXTURE_IMAGE_UNITS),_=n.getParameter(n.MAX_VERTEX_TEXTURE_IMAGE_UNITS),x=n.getParameter(n.MAX_TEXTURE_SIZE),g=n.getParameter(n.MAX_CUBE_MAP_TEXTURE_SIZE),m=n.getParameter(n.MAX_VERTEX_ATTRIBS),A=n.getParameter(n.MAX_VERTEX_UNIFORM_VECTORS),P=n.getParameter(n.MAX_VARYING_VECTORS),M=n.getParameter(n.MAX_FRAGMENT_UNIFORM_VECTORS),I=n.getParameter(n.MAX_SAMPLES),T=n.getParameter(n.SAMPLES);return{isWebGL2:!0,getMaxAnisotropy:s,getMaxPrecision:l,textureFormatReadable:a,textureTypeReadable:o,precision:c,logarithmicDepthBuffer:u,reversedDepthBuffer:d,maxTextures:h,maxVertexTextures:_,maxTextureSize:x,maxCubemapSize:g,maxAttributes:m,maxVertexUniforms:A,maxVaryings:P,maxFragmentUniforms:M,maxSamples:I,samples:T}}function km(n){const e=this;let t=null,i=0,r=!1,s=!1;const a=new tr,o=new ct,l={value:null,needsUpdate:!1};this.uniform=l,this.numPlanes=0,this.numIntersection=0,this.init=function(u,d){const h=u.length!==0||d||i!==0||r;return r=d,i=u.length,h},this.beginShadows=function(){s=!0,p(null)},this.endShadows=function(){s=!1},this.setGlobalState=function(u,d){t=p(u,d,0)},this.setState=function(u,d,h){const _=u.clippingPlanes,x=u.clipIntersection,g=u.clipShadows,m=n.get(u);if(!r||_===null||_.length===0||s&&!g)s?p(null):c();else{const A=s?0:i,P=A*4;let M=m.clippingState||null;l.value=M,M=p(_,d,P,h);for(let I=0;I!==P;++I)M[I]=t[I];m.clippingState=M,this.numIntersection=x?this.numPlanes:0,this.numPlanes+=A}};function c(){l.value!==t&&(l.value=t,l.needsUpdate=i>0),e.numPlanes=i,e.numIntersection=0}function p(u,d,h,_){const x=u!==null?u.length:0;let g=null;if(x!==0){if(g=l.value,_!==!0||g===null){const m=h+x*4,A=d.matrixWorldInverse;o.getNormalMatrix(A),(g===null||g.length<m)&&(g=new Float32Array(m));for(let P=0,M=h;P!==x;++P,M+=4)a.copy(u[P]).applyMatrix4(A,o),a.normal.toArray(g,M),g[M+3]=a.constant}l.value=g,l.needsUpdate=!0}return e.numPlanes=x,e.numIntersection=0,g}}const Bi=4,sc=[.125,.215,.35,.446,.526,.582],nr=20,zm=256,ts=new wd,ac=new Lt;let Ja=null,Qa=0,ja=0,eo=!1;const Gm=new re;class oc{constructor(e){this._renderer=e,this._pingPongRenderTarget=null,this._lodMax=0,this._cubeSize=0,this._sizeLods=[],this._sigmas=[],this._lodMeshes=[],this._backgroundBox=null,this._cubemapMaterial=null,this._equirectMaterial=null,this._blurMaterial=null,this._ggxMaterial=null}fromScene(e,t=0,i=.1,r=100,s={}){const{size:a=256,position:o=Gm}=s;Ja=this._renderer.getRenderTarget(),Qa=this._renderer.getActiveCubeFace(),ja=this._renderer.getActiveMipmapLevel(),eo=this._renderer.xr.enabled,this._renderer.xr.enabled=!1,this._setSize(a);const l=this._allocateTargets();return l.depthBuffer=!0,this._sceneToCubeUV(e,i,r,l,o),t>0&&this._blur(l,0,0,t),this._applyPMREM(l),this._cleanup(l),l}fromEquirectangular(e,t=null){return this._fromTexture(e,t)}fromCubemap(e,t=null){return this._fromTexture(e,t)}compileCubemapShader(){this._cubemapMaterial===null&&(this._cubemapMaterial=dc(),this._compileMaterial(this._cubemapMaterial))}compileEquirectangularShader(){this._equirectMaterial===null&&(this._equirectMaterial=cc(),this._compileMaterial(this._equirectMaterial))}dispose(){this._dispose(),this._cubemapMaterial!==null&&this._cubemapMaterial.dispose(),this._equirectMaterial!==null&&this._equirectMaterial.dispose(),this._backgroundBox!==null&&(this._backgroundBox.geometry.dispose(),this._backgroundBox.material.dispose())}_setSize(e){this._lodMax=Math.floor(Math.log2(e)),this._cubeSize=Math.pow(2,this._lodMax)}_dispose(){this._blurMaterial!==null&&this._blurMaterial.dispose(),this._ggxMaterial!==null&&this._ggxMaterial.dispose(),this._pingPongRenderTarget!==null&&this._pingPongRenderTarget.dispose();for(let e=0;e<this._lodMeshes.length;e++)this._lodMeshes[e].geometry.dispose()}_cleanup(e){this._renderer.setRenderTarget(Ja,Qa,ja),this._renderer.xr.enabled=eo,e.scissorTest=!1,Dr(e,0,0,e.width,e.height)}_fromTexture(e,t){e.mapping===lr||e.mapping===kr?this._setSize(e.image.length===0?16:e.image[0].width||e.image[0].image.width):this._setSize(e.image.width/4),Ja=this._renderer.getRenderTarget(),Qa=this._renderer.getActiveCubeFace(),ja=this._renderer.getActiveMipmapLevel(),eo=this._renderer.xr.enabled,this._renderer.xr.enabled=!1;const i=t||this._allocateTargets();return this._textureToCubeUV(e,i),this._applyPMREM(i),this._cleanup(i),i}_allocateTargets(){const e=3*Math.max(this._cubeSize,112),t=4*this._cubeSize,i={magFilter:sn,minFilter:sn,generateMipmaps:!1,type:wi,format:Bn,colorSpace:ds,depthBuffer:!1},r=lc(e,t,i);if(this._pingPongRenderTarget===null||this._pingPongRenderTarget.width!==e||this._pingPongRenderTarget.height!==t){this._pingPongRenderTarget!==null&&this._dispose(),this._pingPongRenderTarget=lc(e,t,i);const{_lodMax:s}=this;({lodMeshes:this._lodMeshes,sizeLods:this._sizeLods,sigmas:this._sigmas}=Hm(s)),this._blurMaterial=Wm(s,e,t),this._ggxMaterial=Vm(s,e,t)}return r}_compileMaterial(e){const t=new ri(new ai,e);this._renderer.compile(t,ts)}_sceneToCubeUV(e,t,i,r,s){const l=new Fn(90,1,t,i),c=[1,-1,1,1,1,1],p=[1,1,1,-1,-1,-1],u=this._renderer,d=u.autoClear,h=u.toneMapping;u.getClearColor(ac),u.toneMapping=kn,u.autoClear=!1,u.state.buffers.depth.getReversed()&&(u.setRenderTarget(r),u.clearDepth(),u.setRenderTarget(null)),this._backgroundBox===null&&(this._backgroundBox=new ri(new ps,new yd({name:"PMREM.Background",side:_n,depthWrite:!1,depthTest:!1})));const x=this._backgroundBox,g=x.material;let m=!1;const A=e.background;A?A.isColor&&(g.color.copy(A),e.background=null,m=!0):(g.color.copy(ac),m=!0);for(let P=0;P<6;P++){const M=P%3;M===0?(l.up.set(0,c[P],0),l.position.set(s.x,s.y,s.z),l.lookAt(s.x+p[P],s.y,s.z)):M===1?(l.up.set(0,0,c[P]),l.position.set(s.x,s.y,s.z),l.lookAt(s.x,s.y+p[P],s.z)):(l.up.set(0,c[P],0),l.position.set(s.x,s.y,s.z),l.lookAt(s.x,s.y,s.z+p[P]));const I=this._cubeSize;Dr(r,M*I,P>2?I:0,I,I),u.setRenderTarget(r),m&&u.render(x,l),u.render(e,l)}u.toneMapping=h,u.autoClear=d,e.background=A}_textureToCubeUV(e,t){const i=this._renderer,r=e.mapping===lr||e.mapping===kr;r?(this._cubemapMaterial===null&&(this._cubemapMaterial=dc()),this._cubemapMaterial.uniforms.flipEnvMap.value=e.isRenderTargetTexture===!1?-1:1):this._equirectMaterial===null&&(this._equirectMaterial=cc());const s=r?this._cubemapMaterial:this._equirectMaterial,a=this._lodMeshes[0];a.material=s;const o=s.uniforms;o.envMap.value=e;const l=this._cubeSize;Dr(t,0,0,3*l,2*l),i.setRenderTarget(t),i.render(a,ts)}_applyPMREM(e){const t=this._renderer,i=t.autoClear;t.autoClear=!1;const r=this._lodMeshes.length;for(let s=1;s<r;s++)this._applyGGXFilter(e,s-1,s);t.autoClear=i}_applyGGXFilter(e,t,i){const r=this._renderer,s=this._pingPongRenderTarget,a=this._ggxMaterial,o=this._lodMeshes[i];o.material=a;const l=a.uniforms,c=i/(this._lodMeshes.length-1),p=t/(this._lodMeshes.length-1),u=Math.sqrt(c*c-p*p),d=0+c*1.25,h=u*d,{_lodMax:_}=this,x=this._sizeLods[i],g=3*x*(i>_-Bi?i-_+Bi:0),m=4*(this._cubeSize-x);l.envMap.value=e.texture,l.roughness.value=h,l.mipInt.value=_-t,Dr(s,g,m,3*x,2*x),r.setRenderTarget(s),r.render(o,ts),l.envMap.value=s.texture,l.roughness.value=0,l.mipInt.value=_-i,Dr(e,g,m,3*x,2*x),r.setRenderTarget(e),r.render(o,ts)}_blur(e,t,i,r,s){const a=this._pingPongRenderTarget;this._halfBlur(e,a,t,i,r,"latitudinal",s),this._halfBlur(a,e,i,i,r,"longitudinal",s)}_halfBlur(e,t,i,r,s,a,o){const l=this._renderer,c=this._blurMaterial;a!=="latitudinal"&&a!=="longitudinal"&&Et("blur direction must be either latitudinal or longitudinal!");const p=3,u=this._lodMeshes[r];u.material=c;const d=c.uniforms,h=this._sizeLods[i]-1,_=isFinite(s)?Math.PI/(2*h):2*Math.PI/(2*nr-1),x=s/_,g=isFinite(s)?1+Math.floor(p*x):nr;g>nr&&tt(`sigmaRadians, ${s}, is too large and will clip, as it requested ${g} samples when the maximum is set to ${nr}`);const m=[];let A=0;for(let D=0;D<nr;++D){const v=D/x,S=Math.exp(-v*v/2);m.push(S),D===0?A+=S:D<g&&(A+=2*S)}for(let D=0;D<m.length;D++)m[D]=m[D]/A;d.envMap.value=e.texture,d.samples.value=g,d.weights.value=m,d.latitudinal.value=a==="latitudinal",o&&(d.poleAxis.value=o);const{_lodMax:P}=this;d.dTheta.value=_,d.mipInt.value=P-i;const M=this._sizeLods[r],I=3*M*(r>P-Bi?r-P+Bi:0),T=4*(this._cubeSize-M);Dr(t,I,T,3*M,2*M),l.setRenderTarget(t),l.render(u,ts)}}function Hm(n){const e=[],t=[],i=[];let r=n;const s=n-Bi+1+sc.length;for(let a=0;a<s;a++){const o=Math.pow(2,r);e.push(o);let l=1/o;a>n-Bi?l=sc[a-n+Bi-1]:a===0&&(l=0),t.push(l);const c=1/(o-2),p=-c,u=1+c,d=[p,p,u,p,u,u,p,p,u,u,p,u],h=6,_=6,x=3,g=2,m=1,A=new Float32Array(x*_*h),P=new Float32Array(g*_*h),M=new Float32Array(m*_*h);for(let T=0;T<h;T++){const D=T%3*2/3-1,v=T>2?0:-1,S=[D,v,0,D+2/3,v,0,D+2/3,v+1,0,D,v,0,D+2/3,v+1,0,D,v+1,0];A.set(S,x*_*T),P.set(d,g*_*T);const w=[T,T,T,T,T,T];M.set(w,m*_*T)}const I=new ai;I.setAttribute("position",new zn(A,x)),I.setAttribute("uv",new zn(P,g)),I.setAttribute("faceIndex",new zn(M,m)),i.push(new ri(I,null)),r>Bi&&r--}return{lodMeshes:i,sizeLods:e,sigmas:t}}function lc(n,e,t){const i=new ni(n,e,t);return i.texture.mapping=pa,i.texture.name="PMREM.cubeUv",i.scissorTest=!0,i}function Dr(n,e,t,i,r){n.viewport.set(e,t,i,r),n.scissor.set(e,t,i,r)}function Vm(n,e,t){return new si({name:"PMREMGGXConvolution",defines:{GGX_SAMPLES:zm,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${n}.0`},uniforms:{envMap:{value:null},roughness:{value:0},mipInt:{value:0}},vertexShader:_a(),fragmentShader:`

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
		`,blending:Mi,depthTest:!1,depthWrite:!1})}function Wm(n,e,t){const i=new Float32Array(nr),r=new re(0,1,0);return new si({name:"SphericalGaussianBlur",defines:{n:nr,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${n}.0`},uniforms:{envMap:{value:null},samples:{value:1},weights:{value:i},latitudinal:{value:!1},dTheta:{value:0},mipInt:{value:0},poleAxis:{value:r}},vertexShader:_a(),fragmentShader:`

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
		`,blending:Mi,depthTest:!1,depthWrite:!1})}function cc(){return new si({name:"EquirectangularToCubeUV",uniforms:{envMap:{value:null}},vertexShader:_a(),fragmentShader:`

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
		`,blending:Mi,depthTest:!1,depthWrite:!1})}function dc(){return new si({name:"CubemapToCubeUV",uniforms:{envMap:{value:null},flipEnvMap:{value:-1}},vertexShader:_a(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			uniform float flipEnvMap;

			varying vec3 vOutputDirection;

			uniform samplerCube envMap;

			void main() {

				gl_FragColor = textureCube( envMap, vec3( flipEnvMap * vOutputDirection.x, vOutputDirection.yz ) );

			}
		`,blending:Mi,depthTest:!1,depthWrite:!1})}function _a(){return`

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
	`}class Cd extends ni{constructor(e=1,t={}){super(e,e,t),this.isWebGLCubeRenderTarget=!0;const i={width:e,height:e,depth:1},r=[i,i,i,i,i,i];this.texture=new Md(r),this._setTextureOptions(t),this.texture.isRenderTargetTexture=!0}fromEquirectangularTexture(e,t){this.texture.type=t.type,this.texture.colorSpace=t.colorSpace,this.texture.generateMipmaps=t.generateMipmaps,this.texture.minFilter=t.minFilter,this.texture.magFilter=t.magFilter;const i={uniforms:{tEquirect:{value:null}},vertexShader:`

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
			`},r=new ps(5,5,5),s=new si({name:"CubemapFromEquirect",uniforms:Gr(i.uniforms),vertexShader:i.vertexShader,fragmentShader:i.fragmentShader,side:_n,blending:Mi});s.uniforms.tEquirect.value=t;const a=new ri(r,s),o=t.minFilter;return t.minFilter===rr&&(t.minFilter=sn),new Zf(1,10,this).update(e,a),t.minFilter=o,a.geometry.dispose(),a.material.dispose(),this}clear(e,t=!0,i=!0,r=!0){const s=e.getRenderTarget();for(let a=0;a<6;a++)e.setRenderTarget(this,a),e.clear(t,i,r);e.setRenderTarget(s)}}function Xm(n){let e=new WeakMap,t=new WeakMap,i=null;function r(d,h=!1){return d==null?null:h?a(d):s(d)}function s(d){if(d&&d.isTexture){const h=d.mapping;if(h===Ea||h===Ta)if(e.has(d)){const _=e.get(d).texture;return o(_,d.mapping)}else{const _=d.image;if(_&&_.height>0){const x=new Cd(_.height);return x.fromEquirectangularTexture(n,d),e.set(d,x),d.addEventListener("dispose",c),o(x.texture,d.mapping)}else return null}}return d}function a(d){if(d&&d.isTexture){const h=d.mapping,_=h===Ea||h===Ta,x=h===lr||h===kr;if(_||x){let g=t.get(d);const m=g!==void 0?g.texture.pmremVersion:0;if(d.isRenderTargetTexture&&d.pmremVersion!==m)return i===null&&(i=new oc(n)),g=_?i.fromEquirectangular(d,g):i.fromCubemap(d,g),g.texture.pmremVersion=d.pmremVersion,t.set(d,g),g.texture;if(g!==void 0)return g.texture;{const A=d.image;return _&&A&&A.height>0||x&&A&&l(A)?(i===null&&(i=new oc(n)),g=_?i.fromEquirectangular(d):i.fromCubemap(d),g.texture.pmremVersion=d.pmremVersion,t.set(d,g),d.addEventListener("dispose",p),g.texture):null}}}return d}function o(d,h){return h===Ea?d.mapping=lr:h===Ta&&(d.mapping=kr),d}function l(d){let h=0;const _=6;for(let x=0;x<_;x++)d[x]!==void 0&&h++;return h===_}function c(d){const h=d.target;h.removeEventListener("dispose",c);const _=e.get(h);_!==void 0&&(e.delete(h),_.dispose())}function p(d){const h=d.target;h.removeEventListener("dispose",p);const _=t.get(h);_!==void 0&&(t.delete(h),_.dispose())}function u(){e=new WeakMap,t=new WeakMap,i!==null&&(i.dispose(),i=null)}return{get:r,dispose:u}}function qm(n){const e={};function t(i){if(e[i]!==void 0)return e[i];const r=n.getExtension(i);return e[i]=r,r}return{has:function(i){return t(i)!==null},init:function(){t("EXT_color_buffer_float"),t("WEBGL_clip_cull_distance"),t("OES_texture_float_linear"),t("EXT_color_buffer_half_float"),t("WEBGL_multisampled_render_to_texture"),t("WEBGL_render_shared_exponent")},get:function(i){const r=t(i);return r===null&&Fr("WebGLRenderer: "+i+" extension not supported."),r}}}function Ym(n,e,t,i){const r={},s=new WeakMap;function a(u){const d=u.target;d.index!==null&&e.remove(d.index);for(const _ in d.attributes)e.remove(d.attributes[_]);d.removeEventListener("dispose",a),delete r[d.id];const h=s.get(d);h&&(e.remove(h),s.delete(d)),i.releaseStatesOfGeometry(d),d.isInstancedBufferGeometry===!0&&delete d._maxInstanceCount,t.memory.geometries--}function o(u,d){return r[d.id]===!0||(d.addEventListener("dispose",a),r[d.id]=!0,t.memory.geometries++),d}function l(u){const d=u.attributes;for(const h in d)e.update(d[h],n.ARRAY_BUFFER)}function c(u){const d=[],h=u.index,_=u.attributes.position;let x=0;if(_===void 0)return;if(h!==null){const A=h.array;x=h.version;for(let P=0,M=A.length;P<M;P+=3){const I=A[P+0],T=A[P+1],D=A[P+2];d.push(I,T,T,D,D,I)}}else{const A=_.array;x=_.version;for(let P=0,M=A.length/3-1;P<M;P+=3){const I=P+0,T=P+1,D=P+2;d.push(I,T,T,D,D,I)}}const g=new(_.count>=65535?xd:vd)(d,1);g.version=x;const m=s.get(u);m&&e.remove(m),s.set(u,g)}function p(u){const d=s.get(u);if(d){const h=u.index;h!==null&&d.version<h.version&&c(u)}else c(u);return s.get(u)}return{get:o,update:l,getWireframeAttribute:p}}function Km(n,e,t){let i;function r(u){i=u}let s,a;function o(u){s=u.type,a=u.bytesPerElement}function l(u,d){n.drawElements(i,d,s,u*a),t.update(d,i,1)}function c(u,d,h){h!==0&&(n.drawElementsInstanced(i,d,s,u*a,h),t.update(d,i,h))}function p(u,d,h){if(h===0)return;e.get("WEBGL_multi_draw").multiDrawElementsWEBGL(i,d,0,s,u,0,h);let x=0;for(let g=0;g<h;g++)x+=d[g];t.update(x,i,1)}this.setMode=r,this.setIndex=o,this.render=l,this.renderInstances=c,this.renderMultiDraw=p}function $m(n){const e={geometries:0,textures:0},t={frame:0,calls:0,triangles:0,points:0,lines:0};function i(s,a,o){switch(t.calls++,a){case n.TRIANGLES:t.triangles+=o*(s/3);break;case n.LINES:t.lines+=o*(s/2);break;case n.LINE_STRIP:t.lines+=o*(s-1);break;case n.LINE_LOOP:t.lines+=o*s;break;case n.POINTS:t.points+=o*s;break;default:Et("WebGLInfo: Unknown draw mode:",a);break}}function r(){t.calls=0,t.triangles=0,t.points=0,t.lines=0}return{memory:e,render:t,programs:null,autoReset:!0,reset:r,update:i}}function Zm(n,e,t){const i=new WeakMap,r=new Vt;function s(a,o,l){const c=a.morphTargetInfluences,p=o.morphAttributes.position||o.morphAttributes.normal||o.morphAttributes.color,u=p!==void 0?p.length:0;let d=i.get(o);if(d===void 0||d.count!==u){let S=function(){D.dispose(),i.delete(o),o.removeEventListener("dispose",S)};d!==void 0&&d.texture.dispose();const h=o.morphAttributes.position!==void 0,_=o.morphAttributes.normal!==void 0,x=o.morphAttributes.color!==void 0,g=o.morphAttributes.position||[],m=o.morphAttributes.normal||[],A=o.morphAttributes.color||[];let P=0;h===!0&&(P=1),_===!0&&(P=2),x===!0&&(P=3);let M=o.attributes.position.count*P,I=1;M>e.maxTextureSize&&(I=Math.ceil(M/e.maxTextureSize),M=e.maxTextureSize);const T=new Float32Array(M*I*4*u),D=new md(T,M,I,u);D.type=ei,D.needsUpdate=!0;const v=P*4;for(let w=0;w<u;w++){const L=g[w],V=m[w],Z=A[w],k=M*I*4*w;for(let z=0;z<L.count;z++){const C=z*v;h===!0&&(r.fromBufferAttribute(L,z),T[k+C+0]=r.x,T[k+C+1]=r.y,T[k+C+2]=r.z,T[k+C+3]=0),_===!0&&(r.fromBufferAttribute(V,z),T[k+C+4]=r.x,T[k+C+5]=r.y,T[k+C+6]=r.z,T[k+C+7]=0),x===!0&&(r.fromBufferAttribute(Z,z),T[k+C+8]=r.x,T[k+C+9]=r.y,T[k+C+10]=r.z,T[k+C+11]=Z.itemSize===4?r.w:1)}}d={count:u,texture:D,size:new Tt(M,I)},i.set(o,d),o.addEventListener("dispose",S)}if(a.isInstancedMesh===!0&&a.morphTexture!==null)l.getUniforms().setValue(n,"morphTexture",a.morphTexture,t);else{let h=0;for(let x=0;x<c.length;x++)h+=c[x];const _=o.morphTargetsRelative?1:1-h;l.getUniforms().setValue(n,"morphTargetBaseInfluence",_),l.getUniforms().setValue(n,"morphTargetInfluences",c)}l.getUniforms().setValue(n,"morphTargetsTexture",d.texture,t),l.getUniforms().setValue(n,"morphTargetsTextureSize",d.size)}return{update:s}}function Jm(n,e,t,i,r){let s=new WeakMap;function a(c){const p=r.render.frame,u=c.geometry,d=e.get(c,u);if(s.get(d)!==p&&(e.update(d),s.set(d,p)),c.isInstancedMesh&&(c.hasEventListener("dispose",l)===!1&&c.addEventListener("dispose",l),s.get(c)!==p&&(t.update(c.instanceMatrix,n.ARRAY_BUFFER),c.instanceColor!==null&&t.update(c.instanceColor,n.ARRAY_BUFFER),s.set(c,p))),c.isSkinnedMesh){const h=c.skeleton;s.get(h)!==p&&(h.update(),s.set(h,p))}return d}function o(){s=new WeakMap}function l(c){const p=c.target;p.removeEventListener("dispose",l),i.releaseStatesOfObject(p),t.remove(p.instanceMatrix),p.instanceColor!==null&&t.remove(p.instanceColor)}return{update:a,dispose:o}}const Qm={[jc]:"LINEAR_TONE_MAPPING",[ed]:"REINHARD_TONE_MAPPING",[td]:"CINEON_TONE_MAPPING",[nd]:"ACES_FILMIC_TONE_MAPPING",[rd]:"AGX_TONE_MAPPING",[sd]:"NEUTRAL_TONE_MAPPING",[id]:"CUSTOM_TONE_MAPPING"};function jm(n,e,t,i,r,s){const a=new ni(e,t,{type:n,depthBuffer:r,stencilBuffer:s,samples:i?4:0,depthTexture:r?new zr(e,t):void 0}),o=new ni(e,t,{type:wi,depthBuffer:!1,stencilBuffer:!1}),l=new ai;l.setAttribute("position",new Ei([-1,3,0,-1,-1,0,3,-1,0],3)),l.setAttribute("uv",new Ei([0,2,0,0,2,0],2));const c=new Td({uniforms:{tDiffuse:{value:null}},vertexShader:`
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
			}`,depthTest:!1,depthWrite:!1}),p=new ri(l,c),u=new wd(-1,1,1,-1,0,1);let d=null,h=null,_=!1,x,g=null,m=[],A=!1;this.setSize=function(P,M){a.setSize(P,M),o.setSize(P,M);for(let I=0;I<m.length;I++){const T=m[I];T.setSize&&T.setSize(P,M)}},this.setEffects=function(P){m=P,A=m.length>0&&m[0].isRenderPass===!0;const M=a.width,I=a.height;for(let T=0;T<m.length;T++){const D=m[T];D.setSize&&D.setSize(M,I)}},this.begin=function(P,M){if(_||P.toneMapping===kn&&m.length===0)return!1;if(g=M,M!==null){const I=M.width,T=M.height;(a.width!==I||a.height!==T)&&this.setSize(I,T)}return A===!1&&P.setRenderTarget(a),x=P.toneMapping,P.toneMapping=kn,!0},this.hasRenderPass=function(){return A},this.end=function(P,M){P.toneMapping=x,_=!0;let I=a,T=o;for(let D=0;D<m.length;D++){const v=m[D];if(v.enabled!==!1&&(v.render(P,T,I,M),v.needsSwap!==!1)){const S=I;I=T,T=S}}if(d!==P.outputColorSpace||h!==P.toneMapping){d=P.outputColorSpace,h=P.toneMapping,c.defines={},xt.getTransfer(d)===It&&(c.defines.SRGB_TRANSFER="");const D=Qm[h];D&&(c.defines[D]=""),c.needsUpdate=!0}c.uniforms.tDiffuse.value=I.texture,P.setRenderTarget(g),P.render(p,u),g=null,_=!1},this.isCompositing=function(){return _},this.dispose=function(){a.depthTexture&&a.depthTexture.dispose(),a.dispose(),o.dispose(),l.dispose(),c.dispose()}}const Pd=new cn,Jo=new zr(1,1),Ld=new md,Id=new Mf,Dd=new Md,uc=[],fc=[],hc=new Float32Array(16),pc=new Float32Array(9),mc=new Float32Array(4);function Xr(n,e,t){const i=n[0];if(i<=0||i>0)return n;const r=e*t;let s=uc[r];if(s===void 0&&(s=new Float32Array(r),uc[r]=s),e!==0){i.toArray(s,0);for(let a=1,o=0;a!==e;++a)o+=t,n[a].toArray(s,o)}return s}function Zt(n,e){if(n.length!==e.length)return!1;for(let t=0,i=n.length;t<i;t++)if(n[t]!==e[t])return!1;return!0}function Jt(n,e){for(let t=0,i=e.length;t<i;t++)n[t]=e[t]}function va(n,e){let t=fc[e];t===void 0&&(t=new Int32Array(e),fc[e]=t);for(let i=0;i!==e;++i)t[i]=n.allocateTextureUnit();return t}function eg(n,e){const t=this.cache;t[0]!==e&&(n.uniform1f(this.addr,e),t[0]=e)}function tg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2f(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Zt(t,e))return;n.uniform2fv(this.addr,e),Jt(t,e)}}function ng(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3f(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else if(e.r!==void 0)(t[0]!==e.r||t[1]!==e.g||t[2]!==e.b)&&(n.uniform3f(this.addr,e.r,e.g,e.b),t[0]=e.r,t[1]=e.g,t[2]=e.b);else{if(Zt(t,e))return;n.uniform3fv(this.addr,e),Jt(t,e)}}function ig(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4f(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Zt(t,e))return;n.uniform4fv(this.addr,e),Jt(t,e)}}function rg(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(Zt(t,e))return;n.uniformMatrix2fv(this.addr,!1,e),Jt(t,e)}else{if(Zt(t,i))return;mc.set(i),n.uniformMatrix2fv(this.addr,!1,mc),Jt(t,i)}}function sg(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(Zt(t,e))return;n.uniformMatrix3fv(this.addr,!1,e),Jt(t,e)}else{if(Zt(t,i))return;pc.set(i),n.uniformMatrix3fv(this.addr,!1,pc),Jt(t,i)}}function ag(n,e){const t=this.cache,i=e.elements;if(i===void 0){if(Zt(t,e))return;n.uniformMatrix4fv(this.addr,!1,e),Jt(t,e)}else{if(Zt(t,i))return;hc.set(i),n.uniformMatrix4fv(this.addr,!1,hc),Jt(t,i)}}function og(n,e){const t=this.cache;t[0]!==e&&(n.uniform1i(this.addr,e),t[0]=e)}function lg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2i(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Zt(t,e))return;n.uniform2iv(this.addr,e),Jt(t,e)}}function cg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3i(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(Zt(t,e))return;n.uniform3iv(this.addr,e),Jt(t,e)}}function dg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4i(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Zt(t,e))return;n.uniform4iv(this.addr,e),Jt(t,e)}}function ug(n,e){const t=this.cache;t[0]!==e&&(n.uniform1ui(this.addr,e),t[0]=e)}function fg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(n.uniform2ui(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Zt(t,e))return;n.uniform2uiv(this.addr,e),Jt(t,e)}}function hg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(n.uniform3ui(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(Zt(t,e))return;n.uniform3uiv(this.addr,e),Jt(t,e)}}function pg(n,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(n.uniform4ui(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Zt(t,e))return;n.uniform4uiv(this.addr,e),Jt(t,e)}}function mg(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r);let s;this.type===n.SAMPLER_2D_SHADOW?(Jo.compareFunction=t.isReversedDepthBuffer()?ml:pl,s=Jo):s=Pd,t.setTexture2D(e||s,r)}function gg(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTexture3D(e||Id,r)}function _g(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTextureCube(e||Dd,r)}function vg(n,e,t){const i=this.cache,r=t.allocateTextureUnit();i[0]!==r&&(n.uniform1i(this.addr,r),i[0]=r),t.setTexture2DArray(e||Ld,r)}function xg(n){switch(n){case 5126:return eg;case 35664:return tg;case 35665:return ng;case 35666:return ig;case 35674:return rg;case 35675:return sg;case 35676:return ag;case 5124:case 35670:return og;case 35667:case 35671:return lg;case 35668:case 35672:return cg;case 35669:case 35673:return dg;case 5125:return ug;case 36294:return fg;case 36295:return hg;case 36296:return pg;case 35678:case 36198:case 36298:case 36306:case 35682:return mg;case 35679:case 36299:case 36307:return gg;case 35680:case 36300:case 36308:case 36293:return _g;case 36289:case 36303:case 36311:case 36292:return vg}}function yg(n,e){n.uniform1fv(this.addr,e)}function Sg(n,e){const t=Xr(e,this.size,2);n.uniform2fv(this.addr,t)}function Mg(n,e){const t=Xr(e,this.size,3);n.uniform3fv(this.addr,t)}function bg(n,e){const t=Xr(e,this.size,4);n.uniform4fv(this.addr,t)}function Eg(n,e){const t=Xr(e,this.size,4);n.uniformMatrix2fv(this.addr,!1,t)}function Tg(n,e){const t=Xr(e,this.size,9);n.uniformMatrix3fv(this.addr,!1,t)}function wg(n,e){const t=Xr(e,this.size,16);n.uniformMatrix4fv(this.addr,!1,t)}function Ag(n,e){n.uniform1iv(this.addr,e)}function Rg(n,e){n.uniform2iv(this.addr,e)}function Cg(n,e){n.uniform3iv(this.addr,e)}function Pg(n,e){n.uniform4iv(this.addr,e)}function Lg(n,e){n.uniform1uiv(this.addr,e)}function Ig(n,e){n.uniform2uiv(this.addr,e)}function Dg(n,e){n.uniform3uiv(this.addr,e)}function Ug(n,e){n.uniform4uiv(this.addr,e)}function Ng(n,e,t){const i=this.cache,r=e.length,s=va(t,r);Zt(i,s)||(n.uniform1iv(this.addr,s),Jt(i,s));let a;this.type===n.SAMPLER_2D_SHADOW?a=Jo:a=Pd;for(let o=0;o!==r;++o)t.setTexture2D(e[o]||a,s[o])}function Fg(n,e,t){const i=this.cache,r=e.length,s=va(t,r);Zt(i,s)||(n.uniform1iv(this.addr,s),Jt(i,s));for(let a=0;a!==r;++a)t.setTexture3D(e[a]||Id,s[a])}function Og(n,e,t){const i=this.cache,r=e.length,s=va(t,r);Zt(i,s)||(n.uniform1iv(this.addr,s),Jt(i,s));for(let a=0;a!==r;++a)t.setTextureCube(e[a]||Dd,s[a])}function Bg(n,e,t){const i=this.cache,r=e.length,s=va(t,r);Zt(i,s)||(n.uniform1iv(this.addr,s),Jt(i,s));for(let a=0;a!==r;++a)t.setTexture2DArray(e[a]||Ld,s[a])}function kg(n){switch(n){case 5126:return yg;case 35664:return Sg;case 35665:return Mg;case 35666:return bg;case 35674:return Eg;case 35675:return Tg;case 35676:return wg;case 5124:case 35670:return Ag;case 35667:case 35671:return Rg;case 35668:case 35672:return Cg;case 35669:case 35673:return Pg;case 5125:return Lg;case 36294:return Ig;case 36295:return Dg;case 36296:return Ug;case 35678:case 36198:case 36298:case 36306:case 35682:return Ng;case 35679:case 36299:case 36307:return Fg;case 35680:case 36300:case 36308:case 36293:return Og;case 36289:case 36303:case 36311:case 36292:return Bg}}class zg{constructor(e,t,i){this.id=e,this.addr=i,this.cache=[],this.type=t.type,this.setValue=xg(t.type)}}class Gg{constructor(e,t,i){this.id=e,this.addr=i,this.cache=[],this.type=t.type,this.size=t.size,this.setValue=kg(t.type)}}class Hg{constructor(e){this.id=e,this.seq=[],this.map={}}setValue(e,t,i){const r=this.seq;for(let s=0,a=r.length;s!==a;++s){const o=r[s];o.setValue(e,t[o.id],i)}}}const to=/(\w+)(\])?(\[|\.)?/g;function gc(n,e){n.seq.push(e),n.map[e.id]=e}function Vg(n,e,t){const i=n.name,r=i.length;for(to.lastIndex=0;;){const s=to.exec(i),a=to.lastIndex;let o=s[1];const l=s[2]==="]",c=s[3];if(l&&(o=o|0),c===void 0||c==="["&&a+2===r){gc(t,c===void 0?new zg(o,n,e):new Gg(o,n,e));break}else{let u=t.map[o];u===void 0&&(u=new Hg(o),gc(t,u)),t=u}}}class ea{constructor(e,t){this.seq=[],this.map={};const i=e.getProgramParameter(t,e.ACTIVE_UNIFORMS);for(let a=0;a<i;++a){const o=e.getActiveUniform(t,a),l=e.getUniformLocation(t,o.name);Vg(o,l,this)}const r=[],s=[];for(const a of this.seq)a.type===e.SAMPLER_2D_SHADOW||a.type===e.SAMPLER_CUBE_SHADOW||a.type===e.SAMPLER_2D_ARRAY_SHADOW?r.push(a):s.push(a);r.length>0&&(this.seq=r.concat(s))}setValue(e,t,i,r){const s=this.map[t];s!==void 0&&s.setValue(e,i,r)}setOptional(e,t,i){const r=t[i];r!==void 0&&this.setValue(e,i,r)}static upload(e,t,i,r){for(let s=0,a=t.length;s!==a;++s){const o=t[s],l=i[o.id];l.needsUpdate!==!1&&o.setValue(e,l.value,r)}}static seqWithValue(e,t){const i=[];for(let r=0,s=e.length;r!==s;++r){const a=e[r];a.id in t&&i.push(a)}return i}}function _c(n,e,t){const i=n.createShader(e);return n.shaderSource(i,t),n.compileShader(i),i}const Wg=37297;let Xg=0;function qg(n,e){const t=n.split(`
`),i=[],r=Math.max(e-6,0),s=Math.min(e+6,t.length);for(let a=r;a<s;a++){const o=a+1;i.push(`${o===e?">":" "} ${o}: ${t[a]}`)}return i.join(`
`)}const vc=new ct;function Yg(n){xt._getMatrix(vc,xt.workingColorSpace,n);const e=`mat3( ${vc.elements.map(t=>t.toFixed(4))} )`;switch(xt.getTransfer(n)){case sa:return[e,"LinearTransferOETF"];case It:return[e,"sRGBTransferOETF"];default:return tt("WebGLProgram: Unsupported color space: ",n),[e,"LinearTransferOETF"]}}function xc(n,e,t){const i=n.getShaderParameter(e,n.COMPILE_STATUS),s=(n.getShaderInfoLog(e)||"").trim();if(i&&s==="")return"";const a=/ERROR: 0:(\d+)/.exec(s);if(a){const o=parseInt(a[1]);return t.toUpperCase()+`

`+s+`

`+qg(n.getShaderSource(e),o)}else return s}function Kg(n,e){const t=Yg(e);return[`vec4 ${n}( vec4 value ) {`,`	return ${t[1]}( vec4( value.rgb * ${t[0]}, value.a ) );`,"}"].join(`
`)}const $g={[jc]:"Linear",[ed]:"Reinhard",[td]:"Cineon",[nd]:"ACESFilmic",[rd]:"AgX",[sd]:"Neutral",[id]:"Custom"};function Zg(n,e){const t=$g[e];return t===void 0?(tt("WebGLProgram: Unsupported toneMapping:",e),"vec3 "+n+"( vec3 color ) { return LinearToneMapping( color ); }"):"vec3 "+n+"( vec3 color ) { return "+t+"ToneMapping( color ); }"}const Hs=new re;function Jg(){xt.getLuminanceCoefficients(Hs);const n=Hs.x.toFixed(4),e=Hs.y.toFixed(4),t=Hs.z.toFixed(4);return["float luminance( const in vec3 rgb ) {",`	const vec3 weights = vec3( ${n}, ${e}, ${t} );`,"	return dot( weights, rgb );","}"].join(`
`)}function Qg(n){return[n.extensionClipCullDistance?"#extension GL_ANGLE_clip_cull_distance : require":"",n.extensionMultiDraw?"#extension GL_ANGLE_multi_draw : require":""].filter(ss).join(`
`)}function jg(n){const e=[];for(const t in n){const i=n[t];i!==!1&&e.push("#define "+t+" "+i)}return e.join(`
`)}function e_(n,e){const t={},i=n.getProgramParameter(e,n.ACTIVE_ATTRIBUTES);for(let r=0;r<i;r++){const s=n.getActiveAttrib(e,r),a=s.name;let o=1;s.type===n.FLOAT_MAT2&&(o=2),s.type===n.FLOAT_MAT3&&(o=3),s.type===n.FLOAT_MAT4&&(o=4),t[a]={type:s.type,location:n.getAttribLocation(e,a),locationSize:o}}return t}function ss(n){return n!==""}function yc(n,e){const t=e.numSpotLightShadows+e.numSpotLightMaps-e.numSpotLightShadowsWithMaps;return n.replace(/NUM_DIR_LIGHTS/g,e.numDirLights).replace(/NUM_SPOT_LIGHTS/g,e.numSpotLights).replace(/NUM_SPOT_LIGHT_MAPS/g,e.numSpotLightMaps).replace(/NUM_SPOT_LIGHT_COORDS/g,t).replace(/NUM_RECT_AREA_LIGHTS/g,e.numRectAreaLights).replace(/NUM_POINT_LIGHTS/g,e.numPointLights).replace(/NUM_HEMI_LIGHTS/g,e.numHemiLights).replace(/NUM_DIR_LIGHT_SHADOWS/g,e.numDirLightShadows).replace(/NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS/g,e.numSpotLightShadowsWithMaps).replace(/NUM_SPOT_LIGHT_SHADOWS/g,e.numSpotLightShadows).replace(/NUM_POINT_LIGHT_SHADOWS/g,e.numPointLightShadows)}function Sc(n,e){return n.replace(/NUM_CLIPPING_PLANES/g,e.numClippingPlanes).replace(/UNION_CLIPPING_PLANES/g,e.numClippingPlanes-e.numClipIntersection)}const t_=/^[ \t]*#include +<([\w\d./]+)>/gm;function Qo(n){return n.replace(t_,i_)}const n_=new Map;function i_(n,e){let t=pt[e];if(t===void 0){const i=n_.get(e);if(i!==void 0)t=pt[i],tt('WebGLRenderer: Shader chunk "%s" has been deprecated. Use "%s" instead.',e,i);else throw new Error("THREE.WebGLProgram: Can not resolve #include <"+e+">")}return Qo(t)}const r_=/#pragma unroll_loop_start\s+for\s*\(\s*int\s+i\s*=\s*(\d+)\s*;\s*i\s*<\s*(\d+)\s*;\s*i\s*\+\+\s*\)\s*{([\s\S]+?)}\s+#pragma unroll_loop_end/g;function Mc(n){return n.replace(r_,s_)}function s_(n,e,t,i){let r="";for(let s=parseInt(e);s<parseInt(t);s++)r+=i.replace(/\[\s*i\s*\]/g,"[ "+s+" ]").replace(/UNROLLED_LOOP_INDEX/g,s);return r}function bc(n){let e=`precision ${n.precision} float;
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
#define LOW_PRECISION`),e}const a_={[$s]:"SHADOWMAP_TYPE_PCF",[rs]:"SHADOWMAP_TYPE_VSM"};function o_(n){return a_[n.shadowMapType]||"SHADOWMAP_TYPE_BASIC"}const l_={[lr]:"ENVMAP_TYPE_CUBE",[kr]:"ENVMAP_TYPE_CUBE",[pa]:"ENVMAP_TYPE_CUBE_UV"};function c_(n){return n.envMap===!1?"ENVMAP_TYPE_CUBE":l_[n.envMapMode]||"ENVMAP_TYPE_CUBE"}const d_={[kr]:"ENVMAP_MODE_REFRACTION"};function u_(n){return n.envMap===!1?"ENVMAP_MODE_REFLECTION":d_[n.envMapMode]||"ENVMAP_MODE_REFLECTION"}const f_={[Qc]:"ENVMAP_BLENDING_MULTIPLY",[ju]:"ENVMAP_BLENDING_MIX",[ef]:"ENVMAP_BLENDING_ADD"};function h_(n){return n.envMap===!1?"ENVMAP_BLENDING_NONE":f_[n.combine]||"ENVMAP_BLENDING_NONE"}function p_(n){const e=n.envMapCubeUVHeight;if(e===null)return null;const t=Math.log2(e)-2,i=1/e;return{texelWidth:1/(3*Math.max(Math.pow(2,t),112)),texelHeight:i,maxMip:t}}function m_(n,e,t,i){const r=n.getContext(),s=t.defines;let a=t.vertexShader,o=t.fragmentShader;const l=o_(t),c=c_(t),p=u_(t),u=h_(t),d=p_(t),h=Qg(t),_=jg(s),x=r.createProgram();let g,m,A=t.glslVersion?"#version "+t.glslVersion+`
`:"";t.isRawShaderMaterial?(g=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_].filter(ss).join(`
`),g.length>0&&(g+=`
`),m=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_].filter(ss).join(`
`),m.length>0&&(m+=`
`)):(g=[bc(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_,t.extensionClipCullDistance?"#define USE_CLIP_DISTANCE":"",t.batching?"#define USE_BATCHING":"",t.batchingColor?"#define USE_BATCHING_COLOR":"",t.instancing?"#define USE_INSTANCING":"",t.instancingColor?"#define USE_INSTANCING_COLOR":"",t.instancingMorph?"#define USE_INSTANCING_MORPH":"",t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.map?"#define USE_MAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+p:"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.displacementMap?"#define USE_DISPLACEMENTMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.mapUv?"#define MAP_UV "+t.mapUv:"",t.alphaMapUv?"#define ALPHAMAP_UV "+t.alphaMapUv:"",t.lightMapUv?"#define LIGHTMAP_UV "+t.lightMapUv:"",t.aoMapUv?"#define AOMAP_UV "+t.aoMapUv:"",t.emissiveMapUv?"#define EMISSIVEMAP_UV "+t.emissiveMapUv:"",t.bumpMapUv?"#define BUMPMAP_UV "+t.bumpMapUv:"",t.normalMapUv?"#define NORMALMAP_UV "+t.normalMapUv:"",t.displacementMapUv?"#define DISPLACEMENTMAP_UV "+t.displacementMapUv:"",t.metalnessMapUv?"#define METALNESSMAP_UV "+t.metalnessMapUv:"",t.roughnessMapUv?"#define ROUGHNESSMAP_UV "+t.roughnessMapUv:"",t.anisotropyMapUv?"#define ANISOTROPYMAP_UV "+t.anisotropyMapUv:"",t.clearcoatMapUv?"#define CLEARCOATMAP_UV "+t.clearcoatMapUv:"",t.clearcoatNormalMapUv?"#define CLEARCOAT_NORMALMAP_UV "+t.clearcoatNormalMapUv:"",t.clearcoatRoughnessMapUv?"#define CLEARCOAT_ROUGHNESSMAP_UV "+t.clearcoatRoughnessMapUv:"",t.iridescenceMapUv?"#define IRIDESCENCEMAP_UV "+t.iridescenceMapUv:"",t.iridescenceThicknessMapUv?"#define IRIDESCENCE_THICKNESSMAP_UV "+t.iridescenceThicknessMapUv:"",t.sheenColorMapUv?"#define SHEEN_COLORMAP_UV "+t.sheenColorMapUv:"",t.sheenRoughnessMapUv?"#define SHEEN_ROUGHNESSMAP_UV "+t.sheenRoughnessMapUv:"",t.specularMapUv?"#define SPECULARMAP_UV "+t.specularMapUv:"",t.specularColorMapUv?"#define SPECULAR_COLORMAP_UV "+t.specularColorMapUv:"",t.specularIntensityMapUv?"#define SPECULAR_INTENSITYMAP_UV "+t.specularIntensityMapUv:"",t.transmissionMapUv?"#define TRANSMISSIONMAP_UV "+t.transmissionMapUv:"",t.thicknessMapUv?"#define THICKNESSMAP_UV "+t.thicknessMapUv:"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexNormals?"#define HAS_NORMAL":"",t.vertexColors?"#define USE_COLOR":"",t.vertexAlphas?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.flatShading?"#define FLAT_SHADED":"",t.skinning?"#define USE_SKINNING":"",t.morphTargets?"#define USE_MORPHTARGETS":"",t.morphNormals&&t.flatShading===!1?"#define USE_MORPHNORMALS":"",t.morphColors?"#define USE_MORPHCOLORS":"",t.morphTargetsCount>0?"#define MORPHTARGETS_TEXTURE_STRIDE "+t.morphTextureStride:"",t.morphTargetsCount>0?"#define MORPHTARGETS_COUNT "+t.morphTargetsCount:"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+l:"",t.sizeAttenuation?"#define USE_SIZEATTENUATION":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 modelMatrix;","uniform mat4 modelViewMatrix;","uniform mat4 projectionMatrix;","uniform mat4 viewMatrix;","uniform mat3 normalMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;","#ifdef USE_INSTANCING","	attribute mat4 instanceMatrix;","#endif","#ifdef USE_INSTANCING_COLOR","	attribute vec3 instanceColor;","#endif","#ifdef USE_INSTANCING_MORPH","	uniform sampler2D morphTexture;","#endif","attribute vec3 position;","attribute vec3 normal;","attribute vec2 uv;","#ifdef USE_UV1","	attribute vec2 uv1;","#endif","#ifdef USE_UV2","	attribute vec2 uv2;","#endif","#ifdef USE_UV3","	attribute vec2 uv3;","#endif","#ifdef USE_TANGENT","	attribute vec4 tangent;","#endif","#if defined( USE_COLOR_ALPHA )","	attribute vec4 color;","#elif defined( USE_COLOR )","	attribute vec3 color;","#endif","#ifdef USE_SKINNING","	attribute vec4 skinIndex;","	attribute vec4 skinWeight;","#endif",`
`].filter(ss).join(`
`),m=[bc(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,_,t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.alphaToCoverage?"#define ALPHA_TO_COVERAGE":"",t.map?"#define USE_MAP":"",t.matcap?"#define USE_MATCAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+c:"",t.envMap?"#define "+p:"",t.envMap?"#define "+u:"",d?"#define CUBEUV_TEXEL_WIDTH "+d.texelWidth:"",d?"#define CUBEUV_TEXEL_HEIGHT "+d.texelHeight:"",d?"#define CUBEUV_MAX_MIP "+d.maxMip+".0":"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.packedNormalMap?"#define USE_PACKED_NORMALMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoat?"#define USE_CLEARCOAT":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.dispersion?"#define USE_DISPERSION":"",t.iridescence?"#define USE_IRIDESCENCE":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaTest?"#define USE_ALPHATEST":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.sheen?"#define USE_SHEEN":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexColors||t.instancingColor?"#define USE_COLOR":"",t.vertexAlphas||t.batchingColor?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.gradientMap?"#define USE_GRADIENTMAP":"",t.flatShading?"#define FLAT_SHADED":"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+l:"",t.premultipliedAlpha?"#define PREMULTIPLIED_ALPHA":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.numLightProbeGrids>0?"#define USE_LIGHT_PROBES_GRID":"",t.decodeVideoTexture?"#define DECODE_VIDEO_TEXTURE":"",t.decodeVideoTextureEmissive?"#define DECODE_VIDEO_TEXTURE_EMISSIVE":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 viewMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;",t.toneMapping!==kn?"#define TONE_MAPPING":"",t.toneMapping!==kn?pt.tonemapping_pars_fragment:"",t.toneMapping!==kn?Zg("toneMapping",t.toneMapping):"",t.dithering?"#define DITHERING":"",t.opaque?"#define OPAQUE":"",pt.colorspace_pars_fragment,Kg("linearToOutputTexel",t.outputColorSpace),Jg(),t.useDepthPacking?"#define DEPTH_PACKING "+t.depthPacking:"",`
`].filter(ss).join(`
`)),a=Qo(a),a=yc(a,t),a=Sc(a,t),o=Qo(o),o=yc(o,t),o=Sc(o,t),a=Mc(a),o=Mc(o),t.isRawShaderMaterial!==!0&&(A=`#version 300 es
`,g=[h,"#define attribute in","#define varying out","#define texture2D texture"].join(`
`)+`
`+g,m=["#define varying in",t.glslVersion===$o?"":"layout(location = 0) out highp vec4 pc_fragColor;",t.glslVersion===$o?"":"#define gl_FragColor pc_fragColor","#define gl_FragDepthEXT gl_FragDepth","#define texture2D texture","#define textureCube texture","#define texture2DProj textureProj","#define texture2DLodEXT textureLod","#define texture2DProjLodEXT textureProjLod","#define textureCubeLodEXT textureLod","#define texture2DGradEXT textureGrad","#define texture2DProjGradEXT textureProjGrad","#define textureCubeGradEXT textureGrad"].join(`
`)+`
`+m);const P=A+g+a,M=A+m+o,I=_c(r,r.VERTEX_SHADER,P),T=_c(r,r.FRAGMENT_SHADER,M);r.attachShader(x,I),r.attachShader(x,T),t.index0AttributeName!==void 0?r.bindAttribLocation(x,0,t.index0AttributeName):t.hasPositionAttribute===!0&&r.bindAttribLocation(x,0,"position"),r.linkProgram(x);function D(L){if(n.debug.checkShaderErrors){const V=r.getProgramInfoLog(x)||"",Z=r.getShaderInfoLog(I)||"",k=r.getShaderInfoLog(T)||"",z=V.trim(),C=Z.trim(),R=k.trim();let G=!0,Y=!0;if(r.getProgramParameter(x,r.LINK_STATUS)===!1)if(G=!1,typeof n.debug.onShaderError=="function")n.debug.onShaderError(r,x,I,T);else{const ne=xc(r,I,"vertex"),de=xc(r,T,"fragment");Et("WebGLProgram: Shader Error "+r.getError()+" - VALIDATE_STATUS "+r.getProgramParameter(x,r.VALIDATE_STATUS)+`

Material Name: `+L.name+`
Material Type: `+L.type+`

Program Info Log: `+z+`
`+ne+`
`+de)}else z!==""?tt("WebGLProgram: Program Info Log:",z):(C===""||R==="")&&(Y=!1);Y&&(L.diagnostics={runnable:G,programLog:z,vertexShader:{log:C,prefix:g},fragmentShader:{log:R,prefix:m}})}r.deleteShader(I),r.deleteShader(T),v=new ea(r,x),S=e_(r,x)}let v;this.getUniforms=function(){return v===void 0&&D(this),v};let S;this.getAttributes=function(){return S===void 0&&D(this),S};let w=t.rendererExtensionParallelShaderCompile===!1;return this.isReady=function(){return w===!1&&(w=r.getProgramParameter(x,Wg)),w},this.destroy=function(){i.releaseStatesOfProgram(this),r.deleteProgram(x),this.program=void 0},this.type=t.shaderType,this.name=t.shaderName,this.id=Xg++,this.cacheKey=e,this.usedTimes=1,this.program=x,this.vertexShader=I,this.fragmentShader=T,this}let g_=0;class __{constructor(){this.shaderCache=new Map,this.materialCache=new Map}update(e,t,i){const r=this._getShaderCacheForMaterial(e);return r.has(t)===!1&&(r.add(t),t.usedTimes++),r.has(i)===!1&&(r.add(i),i.usedTimes++),this}remove(e){const t=this.materialCache.get(e);for(const i of t)i.usedTimes--,i.usedTimes===0&&this.shaderCache.delete(i.code);return this.materialCache.delete(e),this}getVertexShaderStage(e){return this._getShaderStage(e.vertexShader)}getFragmentShaderStage(e){return this._getShaderStage(e.fragmentShader)}dispose(){this.shaderCache.clear(),this.materialCache.clear()}_getShaderCacheForMaterial(e){const t=this.materialCache;let i=t.get(e);return i===void 0&&(i=new Set,t.set(e,i)),i}_getShaderStage(e){const t=this.shaderCache;let i=t.get(e);return i===void 0&&(i=new v_(e),t.set(e,i)),i}}class v_{constructor(e){this.id=g_++,this.code=e,this.usedTimes=0}}function x_(n){return n===cr||n===ia||n===ra}function y_(n,e,t,i,r,s){const a=new gd,o=new __,l=new Set,c=[],p=new Map,u=i.logarithmicDepthBuffer;let d=i.precision;const h={MeshDepthMaterial:"depth",MeshDistanceMaterial:"distance",MeshNormalMaterial:"normal",MeshBasicMaterial:"basic",MeshLambertMaterial:"lambert",MeshPhongMaterial:"phong",MeshToonMaterial:"toon",MeshStandardMaterial:"physical",MeshPhysicalMaterial:"physical",MeshMatcapMaterial:"matcap",LineBasicMaterial:"basic",LineDashedMaterial:"dashed",PointsMaterial:"points",ShadowMaterial:"shadow",SpriteMaterial:"sprite"};function _(v){return l.add(v),v===0?"uv":`uv${v}`}function x(v,S,w,L,V,Z){const k=L.fog,z=V.geometry,C=v.isMeshStandardMaterial||v.isMeshLambertMaterial||v.isMeshPhongMaterial?L.environment:null,R=v.isMeshStandardMaterial||v.isMeshLambertMaterial&&!v.envMap||v.isMeshPhongMaterial&&!v.envMap,G=e.get(v.envMap||C,R),Y=G&&G.mapping===pa?G.image.height:null,ne=h[v.type];v.precision!==null&&(d=i.getMaxPrecision(v.precision),d!==v.precision&&tt("WebGLProgram.getParameters:",v.precision,"not supported, using",d,"instead."));const de=z.morphAttributes.position||z.morphAttributes.normal||z.morphAttributes.color,ce=de!==void 0?de.length:0;let J=0;z.morphAttributes.position!==void 0&&(J=1),z.morphAttributes.normal!==void 0&&(J=2),z.morphAttributes.color!==void 0&&(J=3);let ue,me,B,oe;if(ne){const Ee=Jn[ne];ue=Ee.vertexShader,me=Ee.fragmentShader}else{ue=v.vertexShader,me=v.fragmentShader;const Ee=o.getVertexShaderStage(v),Dt=o.getFragmentShaderStage(v);o.update(v,Ee,Dt),B=Ee.id,oe=Dt.id}const $=n.getRenderTarget(),se=n.state.buffers.depth.getReversed(),_e=V.isInstancedMesh===!0,Me=V.isBatchedMesh===!0,ve=!!v.map,F=!!v.matcap,K=!!G,ae=!!v.aoMap,xe=!!v.lightMap,Ae=!!v.bumpMap&&v.wireframe===!1,be=!!v.normalMap,ke=!!v.displacementMap,Xe=!!v.emissiveMap,Ze=!!v.metalnessMap,Ne=!!v.roughnessMap,N=v.anisotropy>0,at=v.clearcoat>0,$e=v.dispersion>0,E=v.iridescence>0,f=v.sheen>0,U=v.transmission>0,O=N&&!!v.anisotropyMap,H=at&&!!v.clearcoatMap,fe=at&&!!v.clearcoatNormalMap,he=at&&!!v.clearcoatRoughnessMap,ee=E&&!!v.iridescenceMap,te=E&&!!v.iridescenceThicknessMap,Se=f&&!!v.sheenColorMap,Ue=f&&!!v.sheenRoughnessMap,we=!!v.specularMap,Te=!!v.specularColorMap,He=!!v.specularIntensityMap,Ge=U&&!!v.transmissionMap,it=U&&!!v.thicknessMap,X=!!v.gradientMap,Ce=!!v.alphaMap,ge=v.alphaTest>0,Pe=!!v.alphaHash,Le=!!v.extensions;let ye=kn;v.toneMapped&&($===null||$.isXRRenderTarget===!0)&&(ye=n.toneMapping);const Ve={shaderID:ne,shaderType:v.type,shaderName:v.name,vertexShader:ue,fragmentShader:me,defines:v.defines,customVertexShaderID:B,customFragmentShaderID:oe,isRawShaderMaterial:v.isRawShaderMaterial===!0,glslVersion:v.glslVersion,precision:d,batching:Me,batchingColor:Me&&V._colorsTexture!==null,instancing:_e,instancingColor:_e&&V.instanceColor!==null,instancingMorph:_e&&V.morphTexture!==null,outputColorSpace:$===null?n.outputColorSpace:$.isXRRenderTarget===!0?$.texture.colorSpace:xt.workingColorSpace,alphaToCoverage:!!v.alphaToCoverage,map:ve,matcap:F,envMap:K,envMapMode:K&&G.mapping,envMapCubeUVHeight:Y,aoMap:ae,lightMap:xe,bumpMap:Ae,normalMap:be,displacementMap:ke,emissiveMap:Xe,normalMapObjectSpace:be&&v.normalMapType===rf,normalMapTangentSpace:be&&v.normalMapType===Ol,packedNormalMap:be&&v.normalMapType===Ol&&x_(v.normalMap.format),metalnessMap:Ze,roughnessMap:Ne,anisotropy:N,anisotropyMap:O,clearcoat:at,clearcoatMap:H,clearcoatNormalMap:fe,clearcoatRoughnessMap:he,dispersion:$e,iridescence:E,iridescenceMap:ee,iridescenceThicknessMap:te,sheen:f,sheenColorMap:Se,sheenRoughnessMap:Ue,specularMap:we,specularColorMap:Te,specularIntensityMap:He,transmission:U,transmissionMap:Ge,thicknessMap:it,gradientMap:X,opaque:v.transparent===!1&&v.blending===Nr&&v.alphaToCoverage===!1,alphaMap:Ce,alphaTest:ge,alphaHash:Pe,combine:v.combine,mapUv:ve&&_(v.map.channel),aoMapUv:ae&&_(v.aoMap.channel),lightMapUv:xe&&_(v.lightMap.channel),bumpMapUv:Ae&&_(v.bumpMap.channel),normalMapUv:be&&_(v.normalMap.channel),displacementMapUv:ke&&_(v.displacementMap.channel),emissiveMapUv:Xe&&_(v.emissiveMap.channel),metalnessMapUv:Ze&&_(v.metalnessMap.channel),roughnessMapUv:Ne&&_(v.roughnessMap.channel),anisotropyMapUv:O&&_(v.anisotropyMap.channel),clearcoatMapUv:H&&_(v.clearcoatMap.channel),clearcoatNormalMapUv:fe&&_(v.clearcoatNormalMap.channel),clearcoatRoughnessMapUv:he&&_(v.clearcoatRoughnessMap.channel),iridescenceMapUv:ee&&_(v.iridescenceMap.channel),iridescenceThicknessMapUv:te&&_(v.iridescenceThicknessMap.channel),sheenColorMapUv:Se&&_(v.sheenColorMap.channel),sheenRoughnessMapUv:Ue&&_(v.sheenRoughnessMap.channel),specularMapUv:we&&_(v.specularMap.channel),specularColorMapUv:Te&&_(v.specularColorMap.channel),specularIntensityMapUv:He&&_(v.specularIntensityMap.channel),transmissionMapUv:Ge&&_(v.transmissionMap.channel),thicknessMapUv:it&&_(v.thicknessMap.channel),alphaMapUv:Ce&&_(v.alphaMap.channel),vertexTangents:!!z.attributes.tangent&&(be||N),vertexNormals:!!z.attributes.normal,vertexColors:v.vertexColors,vertexAlphas:v.vertexColors===!0&&!!z.attributes.color&&z.attributes.color.itemSize===4,pointsUvs:V.isPoints===!0&&!!z.attributes.uv&&(ve||Ce),fog:!!k,useFog:v.fog===!0,fogExp2:!!k&&k.isFogExp2,flatShading:v.wireframe===!1&&(v.flatShading===!0||z.attributes.normal===void 0&&be===!1&&(v.isMeshLambertMaterial||v.isMeshPhongMaterial||v.isMeshStandardMaterial||v.isMeshPhysicalMaterial)),sizeAttenuation:v.sizeAttenuation===!0,logarithmicDepthBuffer:u,reversedDepthBuffer:se,skinning:V.isSkinnedMesh===!0,hasPositionAttribute:z.attributes.position!==void 0,morphTargets:z.morphAttributes.position!==void 0,morphNormals:z.morphAttributes.normal!==void 0,morphColors:z.morphAttributes.color!==void 0,morphTargetsCount:ce,morphTextureStride:J,numDirLights:S.directional.length,numPointLights:S.point.length,numSpotLights:S.spot.length,numSpotLightMaps:S.spotLightMap.length,numRectAreaLights:S.rectArea.length,numHemiLights:S.hemi.length,numDirLightShadows:S.directionalShadowMap.length,numPointLightShadows:S.pointShadowMap.length,numSpotLightShadows:S.spotShadowMap.length,numSpotLightShadowsWithMaps:S.numSpotLightShadowsWithMaps,numLightProbes:S.numLightProbes,numLightProbeGrids:Z.length,numClippingPlanes:s.numPlanes,numClipIntersection:s.numIntersection,dithering:v.dithering,shadowMapEnabled:n.shadowMap.enabled&&w.length>0,shadowMapType:n.shadowMap.type,toneMapping:ye,decodeVideoTexture:ve&&v.map.isVideoTexture===!0&&xt.getTransfer(v.map.colorSpace)===It,decodeVideoTextureEmissive:Xe&&v.emissiveMap.isVideoTexture===!0&&xt.getTransfer(v.emissiveMap.colorSpace)===It,premultipliedAlpha:v.premultipliedAlpha,doubleSided:v.side===vi,flipSided:v.side===_n,useDepthPacking:v.depthPacking>=0,depthPacking:v.depthPacking||0,index0AttributeName:v.index0AttributeName,extensionClipCullDistance:Le&&v.extensions.clipCullDistance===!0&&t.has("WEBGL_clip_cull_distance"),extensionMultiDraw:(Le&&v.extensions.multiDraw===!0||Me)&&t.has("WEBGL_multi_draw"),rendererExtensionParallelShaderCompile:t.has("KHR_parallel_shader_compile"),customProgramCacheKey:v.customProgramCacheKey()};return Ve.vertexUv1s=l.has(1),Ve.vertexUv2s=l.has(2),Ve.vertexUv3s=l.has(3),l.clear(),Ve}function g(v){const S=[];if(v.shaderID?S.push(v.shaderID):(S.push(v.customVertexShaderID),S.push(v.customFragmentShaderID)),v.defines!==void 0)for(const w in v.defines)S.push(w),S.push(v.defines[w]);return v.isRawShaderMaterial===!1&&(m(S,v),A(S,v),S.push(n.outputColorSpace)),S.push(v.customProgramCacheKey),S.join()}function m(v,S){v.push(S.precision),v.push(S.outputColorSpace),v.push(S.envMapMode),v.push(S.envMapCubeUVHeight),v.push(S.mapUv),v.push(S.alphaMapUv),v.push(S.lightMapUv),v.push(S.aoMapUv),v.push(S.bumpMapUv),v.push(S.normalMapUv),v.push(S.displacementMapUv),v.push(S.emissiveMapUv),v.push(S.metalnessMapUv),v.push(S.roughnessMapUv),v.push(S.anisotropyMapUv),v.push(S.clearcoatMapUv),v.push(S.clearcoatNormalMapUv),v.push(S.clearcoatRoughnessMapUv),v.push(S.iridescenceMapUv),v.push(S.iridescenceThicknessMapUv),v.push(S.sheenColorMapUv),v.push(S.sheenRoughnessMapUv),v.push(S.specularMapUv),v.push(S.specularColorMapUv),v.push(S.specularIntensityMapUv),v.push(S.transmissionMapUv),v.push(S.thicknessMapUv),v.push(S.combine),v.push(S.fogExp2),v.push(S.sizeAttenuation),v.push(S.morphTargetsCount),v.push(S.morphAttributeCount),v.push(S.numDirLights),v.push(S.numPointLights),v.push(S.numSpotLights),v.push(S.numSpotLightMaps),v.push(S.numHemiLights),v.push(S.numRectAreaLights),v.push(S.numDirLightShadows),v.push(S.numPointLightShadows),v.push(S.numSpotLightShadows),v.push(S.numSpotLightShadowsWithMaps),v.push(S.numLightProbes),v.push(S.shadowMapType),v.push(S.toneMapping),v.push(S.numClippingPlanes),v.push(S.numClipIntersection),v.push(S.depthPacking)}function A(v,S){a.disableAll(),S.instancing&&a.enable(0),S.instancingColor&&a.enable(1),S.instancingMorph&&a.enable(2),S.matcap&&a.enable(3),S.envMap&&a.enable(4),S.normalMapObjectSpace&&a.enable(5),S.normalMapTangentSpace&&a.enable(6),S.clearcoat&&a.enable(7),S.iridescence&&a.enable(8),S.alphaTest&&a.enable(9),S.vertexColors&&a.enable(10),S.vertexAlphas&&a.enable(11),S.vertexUv1s&&a.enable(12),S.vertexUv2s&&a.enable(13),S.vertexUv3s&&a.enable(14),S.vertexTangents&&a.enable(15),S.anisotropy&&a.enable(16),S.alphaHash&&a.enable(17),S.batching&&a.enable(18),S.dispersion&&a.enable(19),S.batchingColor&&a.enable(20),S.gradientMap&&a.enable(21),S.packedNormalMap&&a.enable(22),S.vertexNormals&&a.enable(23),v.push(a.mask),a.disableAll(),S.fog&&a.enable(0),S.useFog&&a.enable(1),S.flatShading&&a.enable(2),S.logarithmicDepthBuffer&&a.enable(3),S.reversedDepthBuffer&&a.enable(4),S.skinning&&a.enable(5),S.morphTargets&&a.enable(6),S.morphNormals&&a.enable(7),S.morphColors&&a.enable(8),S.premultipliedAlpha&&a.enable(9),S.shadowMapEnabled&&a.enable(10),S.doubleSided&&a.enable(11),S.flipSided&&a.enable(12),S.useDepthPacking&&a.enable(13),S.dithering&&a.enable(14),S.transmission&&a.enable(15),S.sheen&&a.enable(16),S.opaque&&a.enable(17),S.pointsUvs&&a.enable(18),S.decodeVideoTexture&&a.enable(19),S.decodeVideoTextureEmissive&&a.enable(20),S.alphaToCoverage&&a.enable(21),S.numLightProbeGrids>0&&a.enable(22),S.hasPositionAttribute&&a.enable(23),v.push(a.mask)}function P(v){const S=h[v.type];let w;if(S){const L=Jn[S];w=Xf.clone(L.uniforms)}else w=v.uniforms;return w}function M(v,S){let w=p.get(S);return w!==void 0?++w.usedTimes:(w=new m_(n,S,v,r),c.push(w),p.set(S,w)),w}function I(v){if(--v.usedTimes===0){const S=c.indexOf(v);c[S]=c[c.length-1],c.pop(),p.delete(v.cacheKey),v.destroy()}}function T(v){o.remove(v)}function D(){o.dispose()}return{getParameters:x,getProgramCacheKey:g,getUniforms:P,acquireProgram:M,releaseProgram:I,releaseShaderCache:T,programs:c,dispose:D}}function S_(){let n=new WeakMap;function e(a){return n.has(a)}function t(a){let o=n.get(a);return o===void 0&&(o={},n.set(a,o)),o}function i(a){n.delete(a)}function r(a,o,l){n.get(a)[o]=l}function s(){n=new WeakMap}return{has:e,get:t,remove:i,update:r,dispose:s}}function M_(n,e){return n.groupOrder!==e.groupOrder?n.groupOrder-e.groupOrder:n.renderOrder!==e.renderOrder?n.renderOrder-e.renderOrder:n.material.id!==e.material.id?n.material.id-e.material.id:n.materialVariant!==e.materialVariant?n.materialVariant-e.materialVariant:n.z!==e.z?n.z-e.z:n.id-e.id}function Ec(n,e){return n.groupOrder!==e.groupOrder?n.groupOrder-e.groupOrder:n.renderOrder!==e.renderOrder?n.renderOrder-e.renderOrder:n.z!==e.z?e.z-n.z:n.id-e.id}function Tc(){const n=[];let e=0;const t=[],i=[],r=[];function s(){e=0,t.length=0,i.length=0,r.length=0}function a(d){let h=0;return d.isInstancedMesh&&(h+=2),d.isSkinnedMesh&&(h+=1),h}function o(d,h,_,x,g,m){let A=n[e];return A===void 0?(A={id:d.id,object:d,geometry:h,material:_,materialVariant:a(d),groupOrder:x,renderOrder:d.renderOrder,z:g,group:m},n[e]=A):(A.id=d.id,A.object=d,A.geometry=h,A.material=_,A.materialVariant=a(d),A.groupOrder=x,A.renderOrder=d.renderOrder,A.z=g,A.group=m),e++,A}function l(d,h,_,x,g,m){const A=o(d,h,_,x,g,m);_.transmission>0?i.push(A):_.transparent===!0?r.push(A):t.push(A)}function c(d,h,_,x,g,m){const A=o(d,h,_,x,g,m);_.transmission>0?i.unshift(A):_.transparent===!0?r.unshift(A):t.unshift(A)}function p(d,h,_){t.length>1&&t.sort(d||M_),i.length>1&&i.sort(h||Ec),r.length>1&&r.sort(h||Ec),_&&(t.reverse(),i.reverse(),r.reverse())}function u(){for(let d=e,h=n.length;d<h;d++){const _=n[d];if(_.id===null)break;_.id=null,_.object=null,_.geometry=null,_.material=null,_.group=null}}return{opaque:t,transmissive:i,transparent:r,init:s,push:l,unshift:c,finish:u,sort:p}}function b_(){let n=new WeakMap;function e(i,r){const s=n.get(i);let a;return s===void 0?(a=new Tc,n.set(i,[a])):r>=s.length?(a=new Tc,s.push(a)):a=s[r],a}function t(){n=new WeakMap}return{get:e,dispose:t}}function E_(){const n={};return{get:function(e){if(n[e.id]!==void 0)return n[e.id];let t;switch(e.type){case"DirectionalLight":t={direction:new re,color:new Lt};break;case"SpotLight":t={position:new re,direction:new re,color:new Lt,distance:0,coneCos:0,penumbraCos:0,decay:0};break;case"PointLight":t={position:new re,color:new Lt,distance:0,decay:0};break;case"HemisphereLight":t={direction:new re,skyColor:new Lt,groundColor:new Lt};break;case"RectAreaLight":t={color:new Lt,position:new re,halfWidth:new re,halfHeight:new re};break}return n[e.id]=t,t}}}function T_(){const n={};return{get:function(e){if(n[e.id]!==void 0)return n[e.id];let t;switch(e.type){case"DirectionalLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Tt};break;case"SpotLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Tt};break;case"PointLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Tt,shadowCameraNear:1,shadowCameraFar:1e3};break}return n[e.id]=t,t}}}let w_=0;function A_(n,e){return(e.castShadow?2:0)-(n.castShadow?2:0)+(e.map?1:0)-(n.map?1:0)}function R_(n){const e=new E_,t=T_(),i={version:0,hash:{directionalLength:-1,pointLength:-1,spotLength:-1,rectAreaLength:-1,hemiLength:-1,numDirectionalShadows:-1,numPointShadows:-1,numSpotShadows:-1,numSpotMaps:-1,numLightProbes:-1},ambient:[0,0,0],probe:[],directional:[],directionalShadow:[],directionalShadowMap:[],directionalShadowMatrix:[],spot:[],spotLightMap:[],spotShadow:[],spotShadowMap:[],spotLightMatrix:[],rectArea:[],rectAreaLTC1:null,rectAreaLTC2:null,point:[],pointShadow:[],pointShadowMap:[],pointShadowMatrix:[],hemi:[],numSpotLightShadowsWithMaps:0,numLightProbes:0};for(let c=0;c<9;c++)i.probe.push(new re);const r=new re,s=new Kt,a=new Kt;function o(c){let p=0,u=0,d=0;for(let S=0;S<9;S++)i.probe[S].set(0,0,0);let h=0,_=0,x=0,g=0,m=0,A=0,P=0,M=0,I=0,T=0,D=0;c.sort(A_);for(let S=0,w=c.length;S<w;S++){const L=c[S],V=L.color,Z=L.intensity,k=L.distance;let z=null;if(L.shadow&&L.shadow.map&&(L.shadow.map.texture.format===cr?z=L.shadow.map.texture:z=L.shadow.map.depthTexture||L.shadow.map.texture),L.isAmbientLight)p+=V.r*Z,u+=V.g*Z,d+=V.b*Z;else if(L.isLightProbe){for(let C=0;C<9;C++)i.probe[C].addScaledVector(L.sh.coefficients[C],Z);D++}else if(L.isDirectionalLight){const C=e.get(L);if(C.color.copy(L.color).multiplyScalar(L.intensity),L.castShadow){const R=L.shadow,G=t.get(L);G.shadowIntensity=R.intensity,G.shadowBias=R.bias,G.shadowNormalBias=R.normalBias,G.shadowRadius=R.radius,G.shadowMapSize=R.mapSize,i.directionalShadow[h]=G,i.directionalShadowMap[h]=z,i.directionalShadowMatrix[h]=L.shadow.matrix,A++}i.directional[h]=C,h++}else if(L.isSpotLight){const C=e.get(L);C.position.setFromMatrixPosition(L.matrixWorld),C.color.copy(V).multiplyScalar(Z),C.distance=k,C.coneCos=Math.cos(L.angle),C.penumbraCos=Math.cos(L.angle*(1-L.penumbra)),C.decay=L.decay,i.spot[x]=C;const R=L.shadow;if(L.map&&(i.spotLightMap[I]=L.map,I++,R.updateMatrices(L),L.castShadow&&T++),i.spotLightMatrix[x]=R.matrix,L.castShadow){const G=t.get(L);G.shadowIntensity=R.intensity,G.shadowBias=R.bias,G.shadowNormalBias=R.normalBias,G.shadowRadius=R.radius,G.shadowMapSize=R.mapSize,i.spotShadow[x]=G,i.spotShadowMap[x]=z,M++}x++}else if(L.isRectAreaLight){const C=e.get(L);C.color.copy(V).multiplyScalar(Z),C.halfWidth.set(L.width*.5,0,0),C.halfHeight.set(0,L.height*.5,0),i.rectArea[g]=C,g++}else if(L.isPointLight){const C=e.get(L);if(C.color.copy(L.color).multiplyScalar(L.intensity),C.distance=L.distance,C.decay=L.decay,L.castShadow){const R=L.shadow,G=t.get(L);G.shadowIntensity=R.intensity,G.shadowBias=R.bias,G.shadowNormalBias=R.normalBias,G.shadowRadius=R.radius,G.shadowMapSize=R.mapSize,G.shadowCameraNear=R.camera.near,G.shadowCameraFar=R.camera.far,i.pointShadow[_]=G,i.pointShadowMap[_]=z,i.pointShadowMatrix[_]=L.shadow.matrix,P++}i.point[_]=C,_++}else if(L.isHemisphereLight){const C=e.get(L);C.skyColor.copy(L.color).multiplyScalar(Z),C.groundColor.copy(L.groundColor).multiplyScalar(Z),i.hemi[m]=C,m++}}g>0&&(n.has("OES_texture_float_linear")===!0?(i.rectAreaLTC1=De.LTC_FLOAT_1,i.rectAreaLTC2=De.LTC_FLOAT_2):(i.rectAreaLTC1=De.LTC_HALF_1,i.rectAreaLTC2=De.LTC_HALF_2)),i.ambient[0]=p,i.ambient[1]=u,i.ambient[2]=d;const v=i.hash;(v.directionalLength!==h||v.pointLength!==_||v.spotLength!==x||v.rectAreaLength!==g||v.hemiLength!==m||v.numDirectionalShadows!==A||v.numPointShadows!==P||v.numSpotShadows!==M||v.numSpotMaps!==I||v.numLightProbes!==D)&&(i.directional.length=h,i.spot.length=x,i.rectArea.length=g,i.point.length=_,i.hemi.length=m,i.directionalShadow.length=A,i.directionalShadowMap.length=A,i.pointShadow.length=P,i.pointShadowMap.length=P,i.spotShadow.length=M,i.spotShadowMap.length=M,i.directionalShadowMatrix.length=A,i.pointShadowMatrix.length=P,i.spotLightMatrix.length=M+I-T,i.spotLightMap.length=I,i.numSpotLightShadowsWithMaps=T,i.numLightProbes=D,v.directionalLength=h,v.pointLength=_,v.spotLength=x,v.rectAreaLength=g,v.hemiLength=m,v.numDirectionalShadows=A,v.numPointShadows=P,v.numSpotShadows=M,v.numSpotMaps=I,v.numLightProbes=D,i.version=w_++)}function l(c,p){let u=0,d=0,h=0,_=0,x=0;const g=p.matrixWorldInverse;for(let m=0,A=c.length;m<A;m++){const P=c[m];if(P.isDirectionalLight){const M=i.directional[u];M.direction.setFromMatrixPosition(P.matrixWorld),r.setFromMatrixPosition(P.target.matrixWorld),M.direction.sub(r),M.direction.transformDirection(g),u++}else if(P.isSpotLight){const M=i.spot[h];M.position.setFromMatrixPosition(P.matrixWorld),M.position.applyMatrix4(g),M.direction.setFromMatrixPosition(P.matrixWorld),r.setFromMatrixPosition(P.target.matrixWorld),M.direction.sub(r),M.direction.transformDirection(g),h++}else if(P.isRectAreaLight){const M=i.rectArea[_];M.position.setFromMatrixPosition(P.matrixWorld),M.position.applyMatrix4(g),a.identity(),s.copy(P.matrixWorld),s.premultiply(g),a.extractRotation(s),M.halfWidth.set(P.width*.5,0,0),M.halfHeight.set(0,P.height*.5,0),M.halfWidth.applyMatrix4(a),M.halfHeight.applyMatrix4(a),_++}else if(P.isPointLight){const M=i.point[d];M.position.setFromMatrixPosition(P.matrixWorld),M.position.applyMatrix4(g),d++}else if(P.isHemisphereLight){const M=i.hemi[x];M.direction.setFromMatrixPosition(P.matrixWorld),M.direction.transformDirection(g),x++}}}return{setup:o,setupView:l,state:i}}function wc(n){const e=new R_(n),t=[],i=[],r=[];function s(d){u.camera=d,t.length=0,i.length=0,r.length=0}function a(d){t.push(d)}function o(d){i.push(d)}function l(d){r.push(d)}function c(){e.setup(t)}function p(d){e.setupView(t,d)}const u={lightsArray:t,shadowsArray:i,lightProbeGridArray:r,camera:null,lights:e,transmissionRenderTarget:{},textureUnits:0};return{init:s,state:u,setupLights:c,setupLightsView:p,pushLight:a,pushShadow:o,pushLightProbeGrid:l}}function C_(n){let e=new WeakMap;function t(r,s=0){const a=e.get(r);let o;return a===void 0?(o=new wc(n),e.set(r,[o])):s>=a.length?(o=new wc(n),a.push(o)):o=a[s],o}function i(){e=new WeakMap}return{get:t,dispose:i}}const P_=`void main() {
	gl_Position = vec4( position, 1.0 );
}`,L_=`uniform sampler2D shadow_pass;
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
}`,I_=[new re(1,0,0),new re(-1,0,0),new re(0,1,0),new re(0,-1,0),new re(0,0,1),new re(0,0,-1)],D_=[new re(0,-1,0),new re(0,-1,0),new re(0,0,1),new re(0,0,-1),new re(0,-1,0),new re(0,-1,0)],Ac=new Kt,ns=new re,no=new re;function U_(n,e,t){let i=new Sd;const r=new Tt,s=new Tt,a=new Vt,o=new Kf,l=new $f,c={},p=t.maxTextureSize,u={[Gi]:_n,[_n]:Gi,[vi]:vi},d=new si({defines:{VSM_SAMPLES:8},uniforms:{shadow_pass:{value:null},resolution:{value:new Tt},radius:{value:4}},vertexShader:P_,fragmentShader:L_}),h=d.clone();h.defines.HORIZONTAL_PASS=1;const _=new ai;_.setAttribute("position",new zn(new Float32Array([-1,-1,.5,3,-1,.5,-1,3,.5]),3));const x=new ri(_,d),g=this;this.enabled=!1,this.autoUpdate=!0,this.needsUpdate=!1,this.type=$s;let m=this.type;this.render=function(T,D,v){if(g.enabled===!1||g.autoUpdate===!1&&g.needsUpdate===!1||T.length===0)return;this.type===Ou&&(tt("WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead."),this.type=$s);const S=n.getRenderTarget(),w=n.getActiveCubeFace(),L=n.getActiveMipmapLevel(),V=n.state;V.setBlending(Mi),V.buffers.depth.getReversed()===!0?V.buffers.color.setClear(0,0,0,0):V.buffers.color.setClear(1,1,1,1),V.buffers.depth.setTest(!0),V.setScissorTest(!1);const Z=m!==this.type;Z&&D.traverse(function(k){k.material&&(Array.isArray(k.material)?k.material.forEach(z=>z.needsUpdate=!0):k.material.needsUpdate=!0)});for(let k=0,z=T.length;k<z;k++){const C=T[k],R=C.shadow;if(R===void 0){tt("WebGLShadowMap:",C,"has no shadow.");continue}if(R.autoUpdate===!1&&R.needsUpdate===!1)continue;r.copy(R.mapSize);const G=R.getFrameExtents();r.multiply(G),s.copy(R.mapSize),(r.x>p||r.y>p)&&(r.x>p&&(s.x=Math.floor(p/G.x),r.x=s.x*G.x,R.mapSize.x=s.x),r.y>p&&(s.y=Math.floor(p/G.y),r.y=s.y*G.y,R.mapSize.y=s.y));const Y=n.state.buffers.depth.getReversed();if(R.camera._reversedDepth=Y,R.map===null||Z===!0){if(R.map!==null&&(R.map.depthTexture!==null&&(R.map.depthTexture.dispose(),R.map.depthTexture=null),R.map.dispose()),this.type===rs){if(C.isPointLight){tt("WebGLShadowMap: VSM shadow maps are not supported for PointLights. Use PCF or BasicShadowMap instead.");continue}R.map=new ni(r.x,r.y,{format:cr,type:wi,minFilter:sn,magFilter:sn,generateMipmaps:!1}),R.map.texture.name=C.name+".shadowMap",R.map.depthTexture=new zr(r.x,r.y,ei),R.map.depthTexture.name=C.name+".shadowMapDepth",R.map.depthTexture.format=Ai,R.map.depthTexture.compareFunction=null,R.map.depthTexture.minFilter=rn,R.map.depthTexture.magFilter=rn}else C.isPointLight?(R.map=new Cd(r.x),R.map.depthTexture=new Vf(r.x,ii)):(R.map=new ni(r.x,r.y),R.map.depthTexture=new zr(r.x,r.y,ii)),R.map.depthTexture.name=C.name+".shadowMap",R.map.depthTexture.format=Ai,this.type===$s?(R.map.depthTexture.compareFunction=Y?ml:pl,R.map.depthTexture.minFilter=sn,R.map.depthTexture.magFilter=sn):(R.map.depthTexture.compareFunction=null,R.map.depthTexture.minFilter=rn,R.map.depthTexture.magFilter=rn);R.camera.updateProjectionMatrix()}const ne=R.map.isWebGLCubeRenderTarget?6:1;for(let de=0;de<ne;de++){if(R.map.isWebGLCubeRenderTarget)n.setRenderTarget(R.map,de),n.clear();else{de===0&&(n.setRenderTarget(R.map),n.clear());const ce=R.getViewport(de);a.set(s.x*ce.x,s.y*ce.y,s.x*ce.z,s.y*ce.w),V.viewport(a)}if(C.isPointLight){const ce=R.camera,J=R.matrix,ue=C.distance||ce.far;ue!==ce.far&&(ce.far=ue,ce.updateProjectionMatrix()),ns.setFromMatrixPosition(C.matrixWorld),ce.position.copy(ns),no.copy(ce.position),no.add(I_[de]),ce.up.copy(D_[de]),ce.lookAt(no),ce.updateMatrixWorld(),J.makeTranslation(-ns.x,-ns.y,-ns.z),Ac.multiplyMatrices(ce.projectionMatrix,ce.matrixWorldInverse),R._frustum.setFromProjectionMatrix(Ac,ce.coordinateSystem,ce.reversedDepth)}else R.updateMatrices(C);i=R.getFrustum(),M(D,v,R.camera,C,this.type)}R.isPointLightShadow!==!0&&this.type===rs&&A(R,v),R.needsUpdate=!1}m=this.type,g.needsUpdate=!1,n.setRenderTarget(S,w,L)};function A(T,D){const v=e.update(x);d.defines.VSM_SAMPLES!==T.blurSamples&&(d.defines.VSM_SAMPLES=T.blurSamples,h.defines.VSM_SAMPLES=T.blurSamples,d.needsUpdate=!0,h.needsUpdate=!0),T.mapPass===null&&(T.mapPass=new ni(r.x,r.y,{format:cr,type:wi})),d.uniforms.shadow_pass.value=T.map.depthTexture,d.uniforms.resolution.value=T.mapSize,d.uniforms.radius.value=T.radius,n.setRenderTarget(T.mapPass),n.clear(),n.renderBufferDirect(D,null,v,d,x,null),h.uniforms.shadow_pass.value=T.mapPass.texture,h.uniforms.resolution.value=T.mapSize,h.uniforms.radius.value=T.radius,n.setRenderTarget(T.map),n.clear(),n.renderBufferDirect(D,null,v,h,x,null)}function P(T,D,v,S){let w=null;const L=v.isPointLight===!0?T.customDistanceMaterial:T.customDepthMaterial;if(L!==void 0)w=L;else if(w=v.isPointLight===!0?l:o,n.localClippingEnabled&&D.clipShadows===!0&&Array.isArray(D.clippingPlanes)&&D.clippingPlanes.length!==0||D.displacementMap&&D.displacementScale!==0||D.alphaMap&&D.alphaTest>0||D.map&&D.alphaTest>0||D.alphaToCoverage===!0){const V=w.uuid,Z=D.uuid;let k=c[V];k===void 0&&(k={},c[V]=k);let z=k[Z];z===void 0&&(z=w.clone(),k[Z]=z,D.addEventListener("dispose",I)),w=z}if(w.visible=D.visible,w.wireframe=D.wireframe,S===rs?w.side=D.shadowSide!==null?D.shadowSide:D.side:w.side=D.shadowSide!==null?D.shadowSide:u[D.side],w.alphaMap=D.alphaMap,w.alphaTest=D.alphaToCoverage===!0?.5:D.alphaTest,w.map=D.map,w.clipShadows=D.clipShadows,w.clippingPlanes=D.clippingPlanes,w.clipIntersection=D.clipIntersection,w.displacementMap=D.displacementMap,w.displacementScale=D.displacementScale,w.displacementBias=D.displacementBias,w.wireframeLinewidth=D.wireframeLinewidth,w.linewidth=D.linewidth,v.isPointLight===!0&&w.isMeshDistanceMaterial===!0){const V=n.properties.get(w);V.light=v}return w}function M(T,D,v,S,w){if(T.visible===!1)return;if(T.layers.test(D.layers)&&(T.isMesh||T.isLine||T.isPoints)&&(T.castShadow||T.receiveShadow&&w===rs)&&(!T.frustumCulled||i.intersectsObject(T))){T.modelViewMatrix.multiplyMatrices(v.matrixWorldInverse,T.matrixWorld);const Z=e.update(T),k=T.material;if(Array.isArray(k)){const z=Z.groups;for(let C=0,R=z.length;C<R;C++){const G=z[C],Y=k[G.materialIndex];if(Y&&Y.visible){const ne=P(T,Y,S,w);T.onBeforeShadow(n,T,D,v,Z,ne,G),n.renderBufferDirect(v,null,Z,ne,T,G),T.onAfterShadow(n,T,D,v,Z,ne,G)}}}else if(k.visible){const z=P(T,k,S,w);T.onBeforeShadow(n,T,D,v,Z,z,null),n.renderBufferDirect(v,null,Z,z,T,null),T.onAfterShadow(n,T,D,v,Z,z,null)}}const V=T.children;for(let Z=0,k=V.length;Z<k;Z++)M(V[Z],D,v,S,w)}function I(T){T.target.removeEventListener("dispose",I);for(const v in c){const S=c[v],w=T.target.uuid;w in S&&(S[w].dispose(),delete S[w])}}}function N_(n,e){function t(){let X=!1;const Ce=new Vt;let ge=null;const Pe=new Vt(0,0,0,0);return{setMask:function(Le){ge!==Le&&!X&&(n.colorMask(Le,Le,Le,Le),ge=Le)},setLocked:function(Le){X=Le},setClear:function(Le,ye,Ve,Ee,Dt){Dt===!0&&(Le*=Ee,ye*=Ee,Ve*=Ee),Ce.set(Le,ye,Ve,Ee),Pe.equals(Ce)===!1&&(n.clearColor(Le,ye,Ve,Ee),Pe.copy(Ce))},reset:function(){X=!1,ge=null,Pe.set(-1,0,0,0)}}}function i(){let X=!1,Ce=!1,ge=null,Pe=null,Le=null;return{setReversed:function(ye){if(Ce!==ye){const Ve=e.get("EXT_clip_control");ye?Ve.clipControlEXT(Ve.LOWER_LEFT_EXT,Ve.ZERO_TO_ONE_EXT):Ve.clipControlEXT(Ve.LOWER_LEFT_EXT,Ve.NEGATIVE_ONE_TO_ONE_EXT),Ce=ye;const Ee=Le;Le=null,this.setClear(Ee)}},getReversed:function(){return Ce},setTest:function(ye){ye?$(n.DEPTH_TEST):se(n.DEPTH_TEST)},setMask:function(ye){ge!==ye&&!X&&(n.depthMask(ye),ge=ye)},setFunc:function(ye){if(Ce&&(ye=mf[ye]),Pe!==ye){switch(ye){case lo:n.depthFunc(n.NEVER);break;case co:n.depthFunc(n.ALWAYS);break;case uo:n.depthFunc(n.LESS);break;case Br:n.depthFunc(n.LEQUAL);break;case fo:n.depthFunc(n.EQUAL);break;case ho:n.depthFunc(n.GEQUAL);break;case po:n.depthFunc(n.GREATER);break;case mo:n.depthFunc(n.NOTEQUAL);break;default:n.depthFunc(n.LEQUAL)}Pe=ye}},setLocked:function(ye){X=ye},setClear:function(ye){Le!==ye&&(Le=ye,Ce&&(ye=1-ye),n.clearDepth(ye))},reset:function(){X=!1,ge=null,Pe=null,Le=null,Ce=!1}}}function r(){let X=!1,Ce=null,ge=null,Pe=null,Le=null,ye=null,Ve=null,Ee=null,Dt=null;return{setTest:function(gt){X||(gt?$(n.STENCIL_TEST):se(n.STENCIL_TEST))},setMask:function(gt){Ce!==gt&&!X&&(n.stencilMask(gt),Ce=gt)},setFunc:function(gt,St,tn){(ge!==gt||Pe!==St||Le!==tn)&&(n.stencilFunc(gt,St,tn),ge=gt,Pe=St,Le=tn)},setOp:function(gt,St,tn){(ye!==gt||Ve!==St||Ee!==tn)&&(n.stencilOp(gt,St,tn),ye=gt,Ve=St,Ee=tn)},setLocked:function(gt){X=gt},setClear:function(gt){Dt!==gt&&(n.clearStencil(gt),Dt=gt)},reset:function(){X=!1,Ce=null,ge=null,Pe=null,Le=null,ye=null,Ve=null,Ee=null,Dt=null}}}const s=new t,a=new i,o=new r,l=new WeakMap,c=new WeakMap;let p={},u={},d={},h=new WeakMap,_=[],x=null,g=!1,m=null,A=null,P=null,M=null,I=null,T=null,D=null,v=new Lt(0,0,0),S=0,w=!1,L=null,V=null,Z=null,k=null,z=null;const C=n.getParameter(n.MAX_COMBINED_TEXTURE_IMAGE_UNITS);let R=!1,G=0;const Y=n.getParameter(n.VERSION);Y.indexOf("WebGL")!==-1?(G=parseFloat(/^WebGL (\d)/.exec(Y)[1]),R=G>=1):Y.indexOf("OpenGL ES")!==-1&&(G=parseFloat(/^OpenGL ES (\d)/.exec(Y)[1]),R=G>=2);let ne=null,de={};const ce=n.getParameter(n.SCISSOR_BOX),J=n.getParameter(n.VIEWPORT),ue=new Vt().fromArray(ce),me=new Vt().fromArray(J);function B(X,Ce,ge,Pe){const Le=new Uint8Array(4),ye=n.createTexture();n.bindTexture(X,ye),n.texParameteri(X,n.TEXTURE_MIN_FILTER,n.NEAREST),n.texParameteri(X,n.TEXTURE_MAG_FILTER,n.NEAREST);for(let Ve=0;Ve<ge;Ve++)X===n.TEXTURE_3D||X===n.TEXTURE_2D_ARRAY?n.texImage3D(Ce,0,n.RGBA,1,1,Pe,0,n.RGBA,n.UNSIGNED_BYTE,Le):n.texImage2D(Ce+Ve,0,n.RGBA,1,1,0,n.RGBA,n.UNSIGNED_BYTE,Le);return ye}const oe={};oe[n.TEXTURE_2D]=B(n.TEXTURE_2D,n.TEXTURE_2D,1),oe[n.TEXTURE_CUBE_MAP]=B(n.TEXTURE_CUBE_MAP,n.TEXTURE_CUBE_MAP_POSITIVE_X,6),oe[n.TEXTURE_2D_ARRAY]=B(n.TEXTURE_2D_ARRAY,n.TEXTURE_2D_ARRAY,1,1),oe[n.TEXTURE_3D]=B(n.TEXTURE_3D,n.TEXTURE_3D,1,1),s.setClear(0,0,0,1),a.setClear(1),o.setClear(0),$(n.DEPTH_TEST),a.setFunc(Br),Ae(!1),be(Dl),$(n.CULL_FACE),ae(Mi);function $(X){p[X]!==!0&&(n.enable(X),p[X]=!0)}function se(X){p[X]!==!1&&(n.disable(X),p[X]=!1)}function _e(X,Ce){return d[X]!==Ce?(n.bindFramebuffer(X,Ce),d[X]=Ce,X===n.DRAW_FRAMEBUFFER&&(d[n.FRAMEBUFFER]=Ce),X===n.FRAMEBUFFER&&(d[n.DRAW_FRAMEBUFFER]=Ce),!0):!1}function Me(X,Ce){let ge=_,Pe=!1;if(X){ge=h.get(Ce),ge===void 0&&(ge=[],h.set(Ce,ge));const Le=X.textures;if(ge.length!==Le.length||ge[0]!==n.COLOR_ATTACHMENT0){for(let ye=0,Ve=Le.length;ye<Ve;ye++)ge[ye]=n.COLOR_ATTACHMENT0+ye;ge.length=Le.length,Pe=!0}}else ge[0]!==n.BACK&&(ge[0]=n.BACK,Pe=!0);Pe&&n.drawBuffers(ge)}function ve(X){return x!==X?(n.useProgram(X),x=X,!0):!1}const F={[Oi]:n.FUNC_ADD,[Bu]:n.FUNC_SUBTRACT,[ku]:n.FUNC_REVERSE_SUBTRACT};F[zu]=n.MIN,F[Gu]=n.MAX;const K={[Hu]:n.ZERO,[ao]:n.ONE,[Vu]:n.SRC_COLOR,[oo]:n.SRC_ALPHA,[Ku]:n.SRC_ALPHA_SATURATE,[qu]:n.DST_COLOR,[Wu]:n.DST_ALPHA,[Jc]:n.ONE_MINUS_SRC_COLOR,[os]:n.ONE_MINUS_SRC_ALPHA,[Yu]:n.ONE_MINUS_DST_COLOR,[Xu]:n.ONE_MINUS_DST_ALPHA,[$u]:n.CONSTANT_COLOR,[Zu]:n.ONE_MINUS_CONSTANT_COLOR,[Ju]:n.CONSTANT_ALPHA,[Qu]:n.ONE_MINUS_CONSTANT_ALPHA};function ae(X,Ce,ge,Pe,Le,ye,Ve,Ee,Dt,gt){if(X===Mi){g===!0&&(se(n.BLEND),g=!1);return}if(g===!1&&($(n.BLEND),g=!0),X!==Zc){if(X!==m||gt!==w){if((A!==Oi||I!==Oi)&&(n.blendEquation(n.FUNC_ADD),A=Oi,I=Oi),gt)switch(X){case Nr:n.blendFuncSeparate(n.ONE,n.ONE_MINUS_SRC_ALPHA,n.ONE,n.ONE_MINUS_SRC_ALPHA);break;case Ul:n.blendFunc(n.ONE,n.ONE);break;case Nl:n.blendFuncSeparate(n.ZERO,n.ONE_MINUS_SRC_COLOR,n.ZERO,n.ONE);break;case Fl:n.blendFuncSeparate(n.DST_COLOR,n.ONE_MINUS_SRC_ALPHA,n.ZERO,n.ONE);break;default:Et("WebGLState: Invalid blending: ",X);break}else switch(X){case Nr:n.blendFuncSeparate(n.SRC_ALPHA,n.ONE_MINUS_SRC_ALPHA,n.ONE,n.ONE_MINUS_SRC_ALPHA);break;case Ul:n.blendFuncSeparate(n.SRC_ALPHA,n.ONE,n.ONE,n.ONE);break;case Nl:Et("WebGLState: SubtractiveBlending requires material.premultipliedAlpha = true");break;case Fl:Et("WebGLState: MultiplyBlending requires material.premultipliedAlpha = true");break;default:Et("WebGLState: Invalid blending: ",X);break}P=null,M=null,T=null,D=null,v.set(0,0,0),S=0,m=X,w=gt}return}Le=Le||Ce,ye=ye||ge,Ve=Ve||Pe,(Ce!==A||Le!==I)&&(n.blendEquationSeparate(F[Ce],F[Le]),A=Ce,I=Le),(ge!==P||Pe!==M||ye!==T||Ve!==D)&&(n.blendFuncSeparate(K[ge],K[Pe],K[ye],K[Ve]),P=ge,M=Pe,T=ye,D=Ve),(Ee.equals(v)===!1||Dt!==S)&&(n.blendColor(Ee.r,Ee.g,Ee.b,Dt),v.copy(Ee),S=Dt),m=X,w=!1}function xe(X,Ce){X.side===vi?se(n.CULL_FACE):$(n.CULL_FACE);let ge=X.side===_n;Ce&&(ge=!ge),Ae(ge),X.blending===Nr&&X.transparent===!1?ae(Mi):ae(X.blending,X.blendEquation,X.blendSrc,X.blendDst,X.blendEquationAlpha,X.blendSrcAlpha,X.blendDstAlpha,X.blendColor,X.blendAlpha,X.premultipliedAlpha),a.setFunc(X.depthFunc),a.setTest(X.depthTest),a.setMask(X.depthWrite),s.setMask(X.colorWrite);const Pe=X.stencilWrite;o.setTest(Pe),Pe&&(o.setMask(X.stencilWriteMask),o.setFunc(X.stencilFunc,X.stencilRef,X.stencilFuncMask),o.setOp(X.stencilFail,X.stencilZFail,X.stencilZPass)),Xe(X.polygonOffset,X.polygonOffsetFactor,X.polygonOffsetUnits),X.alphaToCoverage===!0?$(n.SAMPLE_ALPHA_TO_COVERAGE):se(n.SAMPLE_ALPHA_TO_COVERAGE)}function Ae(X){L!==X&&(X?n.frontFace(n.CW):n.frontFace(n.CCW),L=X)}function be(X){X!==Nu?($(n.CULL_FACE),X!==V&&(X===Dl?n.cullFace(n.BACK):X===Fu?n.cullFace(n.FRONT):n.cullFace(n.FRONT_AND_BACK))):se(n.CULL_FACE),V=X}function ke(X){X!==Z&&(R&&n.lineWidth(X),Z=X)}function Xe(X,Ce,ge){X?($(n.POLYGON_OFFSET_FILL),(k!==Ce||z!==ge)&&(k=Ce,z=ge,a.getReversed()&&(Ce=-Ce),n.polygonOffset(Ce,ge))):se(n.POLYGON_OFFSET_FILL)}function Ze(X){X?$(n.SCISSOR_TEST):se(n.SCISSOR_TEST)}function Ne(X){X===void 0&&(X=n.TEXTURE0+C-1),ne!==X&&(n.activeTexture(X),ne=X)}function N(X,Ce,ge){ge===void 0&&(ne===null?ge=n.TEXTURE0+C-1:ge=ne);let Pe=de[ge];Pe===void 0&&(Pe={type:void 0,texture:void 0},de[ge]=Pe),(Pe.type!==X||Pe.texture!==Ce)&&(ne!==ge&&(n.activeTexture(ge),ne=ge),n.bindTexture(X,Ce||oe[X]),Pe.type=X,Pe.texture=Ce)}function at(){const X=de[ne];X!==void 0&&X.type!==void 0&&(n.bindTexture(X.type,null),X.type=void 0,X.texture=void 0)}function $e(){try{n.compressedTexImage2D(...arguments)}catch(X){Et("WebGLState:",X)}}function E(){try{n.compressedTexImage3D(...arguments)}catch(X){Et("WebGLState:",X)}}function f(){try{n.texSubImage2D(...arguments)}catch(X){Et("WebGLState:",X)}}function U(){try{n.texSubImage3D(...arguments)}catch(X){Et("WebGLState:",X)}}function O(){try{n.compressedTexSubImage2D(...arguments)}catch(X){Et("WebGLState:",X)}}function H(){try{n.compressedTexSubImage3D(...arguments)}catch(X){Et("WebGLState:",X)}}function fe(){try{n.texStorage2D(...arguments)}catch(X){Et("WebGLState:",X)}}function he(){try{n.texStorage3D(...arguments)}catch(X){Et("WebGLState:",X)}}function ee(){try{n.texImage2D(...arguments)}catch(X){Et("WebGLState:",X)}}function te(){try{n.texImage3D(...arguments)}catch(X){Et("WebGLState:",X)}}function Se(X){return u[X]!==void 0?u[X]:n.getParameter(X)}function Ue(X,Ce){u[X]!==Ce&&(n.pixelStorei(X,Ce),u[X]=Ce)}function we(X){ue.equals(X)===!1&&(n.scissor(X.x,X.y,X.z,X.w),ue.copy(X))}function Te(X){me.equals(X)===!1&&(n.viewport(X.x,X.y,X.z,X.w),me.copy(X))}function He(X,Ce){let ge=c.get(Ce);ge===void 0&&(ge=new WeakMap,c.set(Ce,ge));let Pe=ge.get(X);Pe===void 0&&(Pe=n.getUniformBlockIndex(Ce,X.name),ge.set(X,Pe))}function Ge(X,Ce){const Pe=c.get(Ce).get(X);l.get(Ce)!==Pe&&(n.uniformBlockBinding(Ce,Pe,X.__bindingPointIndex),l.set(Ce,Pe))}function it(){n.disable(n.BLEND),n.disable(n.CULL_FACE),n.disable(n.DEPTH_TEST),n.disable(n.POLYGON_OFFSET_FILL),n.disable(n.SCISSOR_TEST),n.disable(n.STENCIL_TEST),n.disable(n.SAMPLE_ALPHA_TO_COVERAGE),n.blendEquation(n.FUNC_ADD),n.blendFunc(n.ONE,n.ZERO),n.blendFuncSeparate(n.ONE,n.ZERO,n.ONE,n.ZERO),n.blendColor(0,0,0,0),n.colorMask(!0,!0,!0,!0),n.clearColor(0,0,0,0),n.depthMask(!0),n.depthFunc(n.LESS),a.setReversed(!1),n.clearDepth(1),n.stencilMask(4294967295),n.stencilFunc(n.ALWAYS,0,4294967295),n.stencilOp(n.KEEP,n.KEEP,n.KEEP),n.clearStencil(0),n.cullFace(n.BACK),n.frontFace(n.CCW),n.polygonOffset(0,0),n.activeTexture(n.TEXTURE0),n.bindFramebuffer(n.FRAMEBUFFER,null),n.bindFramebuffer(n.DRAW_FRAMEBUFFER,null),n.bindFramebuffer(n.READ_FRAMEBUFFER,null),n.useProgram(null),n.lineWidth(1),n.scissor(0,0,n.canvas.width,n.canvas.height),n.viewport(0,0,n.canvas.width,n.canvas.height),n.pixelStorei(n.PACK_ALIGNMENT,4),n.pixelStorei(n.UNPACK_ALIGNMENT,4),n.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,!1),n.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,!1),n.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,n.BROWSER_DEFAULT_WEBGL),n.pixelStorei(n.PACK_ROW_LENGTH,0),n.pixelStorei(n.PACK_SKIP_PIXELS,0),n.pixelStorei(n.PACK_SKIP_ROWS,0),n.pixelStorei(n.UNPACK_ROW_LENGTH,0),n.pixelStorei(n.UNPACK_IMAGE_HEIGHT,0),n.pixelStorei(n.UNPACK_SKIP_PIXELS,0),n.pixelStorei(n.UNPACK_SKIP_ROWS,0),n.pixelStorei(n.UNPACK_SKIP_IMAGES,0),p={},u={},ne=null,de={},d={},h=new WeakMap,_=[],x=null,g=!1,m=null,A=null,P=null,M=null,I=null,T=null,D=null,v=new Lt(0,0,0),S=0,w=!1,L=null,V=null,Z=null,k=null,z=null,ue.set(0,0,n.canvas.width,n.canvas.height),me.set(0,0,n.canvas.width,n.canvas.height),s.reset(),a.reset(),o.reset()}return{buffers:{color:s,depth:a,stencil:o},enable:$,disable:se,bindFramebuffer:_e,drawBuffers:Me,useProgram:ve,setBlending:ae,setMaterial:xe,setFlipSided:Ae,setCullFace:be,setLineWidth:ke,setPolygonOffset:Xe,setScissorTest:Ze,activeTexture:Ne,bindTexture:N,unbindTexture:at,compressedTexImage2D:$e,compressedTexImage3D:E,texImage2D:ee,texImage3D:te,pixelStorei:Ue,getParameter:Se,updateUBOMapping:He,uniformBlockBinding:Ge,texStorage2D:fe,texStorage3D:he,texSubImage2D:f,texSubImage3D:U,compressedTexSubImage2D:O,compressedTexSubImage3D:H,scissor:we,viewport:Te,reset:it}}function F_(n,e,t,i,r,s,a){const o=e.has("WEBGL_multisampled_render_to_texture")?e.get("WEBGL_multisampled_render_to_texture"):null,l=typeof navigator>"u"?!1:/OculusBrowser/g.test(navigator.userAgent),c=new Tt,p=new WeakMap,u=new Set;let d;const h=new WeakMap;let _=!1;try{_=typeof OffscreenCanvas<"u"&&new OffscreenCanvas(1,1).getContext("2d")!==null}catch{}function x(E,f){return _?new OffscreenCanvas(E,f):oa("canvas")}function g(E,f,U){let O=1;const H=$e(E);if((H.width>U||H.height>U)&&(O=U/Math.max(H.width,H.height)),O<1)if(typeof HTMLImageElement<"u"&&E instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&E instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&E instanceof ImageBitmap||typeof VideoFrame<"u"&&E instanceof VideoFrame){const fe=Math.floor(O*H.width),he=Math.floor(O*H.height);d===void 0&&(d=x(fe,he));const ee=f?x(fe,he):d;return ee.width=fe,ee.height=he,ee.getContext("2d").drawImage(E,0,0,fe,he),tt("WebGLRenderer: Texture has been resized from ("+H.width+"x"+H.height+") to ("+fe+"x"+he+")."),ee}else return"data"in E&&tt("WebGLRenderer: Image in DataTexture is too big ("+H.width+"x"+H.height+")."),E;return E}function m(E){return E.generateMipmaps}function A(E){n.generateMipmap(E)}function P(E){return E.isWebGLCubeRenderTarget?n.TEXTURE_CUBE_MAP:E.isWebGL3DRenderTarget?n.TEXTURE_3D:E.isWebGLArrayRenderTarget||E.isCompressedArrayTexture?n.TEXTURE_2D_ARRAY:n.TEXTURE_2D}function M(E,f,U,O,H,fe=!1){if(E!==null){if(n[E]!==void 0)return n[E];tt("WebGLRenderer: Attempt to use non-existing WebGL internal format '"+E+"'")}let he;O&&(he=e.get("EXT_texture_norm16"),he||tt("WebGLRenderer: Unable to use normalized textures without EXT_texture_norm16 extension"));let ee=f;if(f===n.RED&&(U===n.FLOAT&&(ee=n.R32F),U===n.HALF_FLOAT&&(ee=n.R16F),U===n.UNSIGNED_BYTE&&(ee=n.R8),U===n.UNSIGNED_SHORT&&he&&(ee=he.R16_EXT),U===n.SHORT&&he&&(ee=he.R16_SNORM_EXT)),f===n.RED_INTEGER&&(U===n.UNSIGNED_BYTE&&(ee=n.R8UI),U===n.UNSIGNED_SHORT&&(ee=n.R16UI),U===n.UNSIGNED_INT&&(ee=n.R32UI),U===n.BYTE&&(ee=n.R8I),U===n.SHORT&&(ee=n.R16I),U===n.INT&&(ee=n.R32I)),f===n.RG&&(U===n.FLOAT&&(ee=n.RG32F),U===n.HALF_FLOAT&&(ee=n.RG16F),U===n.UNSIGNED_BYTE&&(ee=n.RG8),U===n.UNSIGNED_SHORT&&he&&(ee=he.RG16_EXT),U===n.SHORT&&he&&(ee=he.RG16_SNORM_EXT)),f===n.RG_INTEGER&&(U===n.UNSIGNED_BYTE&&(ee=n.RG8UI),U===n.UNSIGNED_SHORT&&(ee=n.RG16UI),U===n.UNSIGNED_INT&&(ee=n.RG32UI),U===n.BYTE&&(ee=n.RG8I),U===n.SHORT&&(ee=n.RG16I),U===n.INT&&(ee=n.RG32I)),f===n.RGB_INTEGER&&(U===n.UNSIGNED_BYTE&&(ee=n.RGB8UI),U===n.UNSIGNED_SHORT&&(ee=n.RGB16UI),U===n.UNSIGNED_INT&&(ee=n.RGB32UI),U===n.BYTE&&(ee=n.RGB8I),U===n.SHORT&&(ee=n.RGB16I),U===n.INT&&(ee=n.RGB32I)),f===n.RGBA_INTEGER&&(U===n.UNSIGNED_BYTE&&(ee=n.RGBA8UI),U===n.UNSIGNED_SHORT&&(ee=n.RGBA16UI),U===n.UNSIGNED_INT&&(ee=n.RGBA32UI),U===n.BYTE&&(ee=n.RGBA8I),U===n.SHORT&&(ee=n.RGBA16I),U===n.INT&&(ee=n.RGBA32I)),f===n.RGB&&(U===n.UNSIGNED_SHORT&&he&&(ee=he.RGB16_EXT),U===n.SHORT&&he&&(ee=he.RGB16_SNORM_EXT),U===n.UNSIGNED_INT_5_9_9_9_REV&&(ee=n.RGB9_E5),U===n.UNSIGNED_INT_10F_11F_11F_REV&&(ee=n.R11F_G11F_B10F)),f===n.RGBA){const te=fe?sa:xt.getTransfer(H);U===n.FLOAT&&(ee=n.RGBA32F),U===n.HALF_FLOAT&&(ee=n.RGBA16F),U===n.UNSIGNED_BYTE&&(ee=te===It?n.SRGB8_ALPHA8:n.RGBA8),U===n.UNSIGNED_SHORT&&he&&(ee=he.RGBA16_EXT),U===n.SHORT&&he&&(ee=he.RGBA16_SNORM_EXT),U===n.UNSIGNED_SHORT_4_4_4_4&&(ee=n.RGBA4),U===n.UNSIGNED_SHORT_5_5_5_1&&(ee=n.RGB5_A1)}return(ee===n.R16F||ee===n.R32F||ee===n.RG16F||ee===n.RG32F||ee===n.RGBA16F||ee===n.RGBA32F)&&e.get("EXT_color_buffer_float"),ee}function I(E,f){let U;return E?f===null||f===ii||f===cs?U=n.DEPTH24_STENCIL8:f===ei?U=n.DEPTH32F_STENCIL8:f===ls&&(U=n.DEPTH24_STENCIL8,tt("DepthTexture: 16 bit depth attachment is not supported with stencil. Using 24-bit attachment.")):f===null||f===ii||f===cs?U=n.DEPTH_COMPONENT24:f===ei?U=n.DEPTH_COMPONENT32F:f===ls&&(U=n.DEPTH_COMPONENT16),U}function T(E,f){return m(E)===!0||E.isFramebufferTexture&&E.minFilter!==rn&&E.minFilter!==sn?Math.log2(Math.max(f.width,f.height))+1:E.mipmaps!==void 0&&E.mipmaps.length>0?E.mipmaps.length:E.isCompressedTexture&&Array.isArray(E.image)?f.mipmaps.length:1}function D(E){const f=E.target;f.removeEventListener("dispose",D),S(f),f.isVideoTexture&&p.delete(f),f.isHTMLTexture&&u.delete(f)}function v(E){const f=E.target;f.removeEventListener("dispose",v),L(f)}function S(E){const f=i.get(E);if(f.__webglInit===void 0)return;const U=E.source,O=h.get(U);if(O){const H=O[f.__cacheKey];H.usedTimes--,H.usedTimes===0&&w(E),Object.keys(O).length===0&&h.delete(U)}i.remove(E)}function w(E){const f=i.get(E);n.deleteTexture(f.__webglTexture);const U=E.source,O=h.get(U);delete O[f.__cacheKey],a.memory.textures--}function L(E){const f=i.get(E);if(E.depthTexture&&(E.depthTexture.dispose(),i.remove(E.depthTexture)),E.isWebGLCubeRenderTarget)for(let O=0;O<6;O++){if(Array.isArray(f.__webglFramebuffer[O]))for(let H=0;H<f.__webglFramebuffer[O].length;H++)n.deleteFramebuffer(f.__webglFramebuffer[O][H]);else n.deleteFramebuffer(f.__webglFramebuffer[O]);f.__webglDepthbuffer&&n.deleteRenderbuffer(f.__webglDepthbuffer[O])}else{if(Array.isArray(f.__webglFramebuffer))for(let O=0;O<f.__webglFramebuffer.length;O++)n.deleteFramebuffer(f.__webglFramebuffer[O]);else n.deleteFramebuffer(f.__webglFramebuffer);if(f.__webglDepthbuffer&&n.deleteRenderbuffer(f.__webglDepthbuffer),f.__webglMultisampledFramebuffer&&n.deleteFramebuffer(f.__webglMultisampledFramebuffer),f.__webglColorRenderbuffer)for(let O=0;O<f.__webglColorRenderbuffer.length;O++)f.__webglColorRenderbuffer[O]&&n.deleteRenderbuffer(f.__webglColorRenderbuffer[O]);f.__webglDepthRenderbuffer&&n.deleteRenderbuffer(f.__webglDepthRenderbuffer)}const U=E.textures;for(let O=0,H=U.length;O<H;O++){const fe=i.get(U[O]);fe.__webglTexture&&(n.deleteTexture(fe.__webglTexture),a.memory.textures--),i.remove(U[O])}i.remove(E)}let V=0;function Z(){V=0}function k(){return V}function z(E){V=E}function C(){const E=V;return E>=r.maxTextures&&tt("WebGLTextures: Trying to use "+E+" texture units while this GPU supports only "+r.maxTextures),V+=1,E}function R(E){const f=[];return f.push(E.wrapS),f.push(E.wrapT),f.push(E.wrapR||0),f.push(E.magFilter),f.push(E.minFilter),f.push(E.anisotropy),f.push(E.internalFormat),f.push(E.format),f.push(E.type),f.push(E.generateMipmaps),f.push(E.premultiplyAlpha),f.push(E.flipY),f.push(E.unpackAlignment),f.push(E.colorSpace),f.join()}function G(E,f){const U=i.get(E);if(E.isVideoTexture&&N(E),E.isRenderTargetTexture===!1&&E.isExternalTexture!==!0&&E.version>0&&U.__version!==E.version){const O=E.image;if(O===null)tt("WebGLRenderer: Texture marked for update but no image data found.");else if(O.complete===!1)tt("WebGLRenderer: Texture marked for update but image is incomplete");else{se(U,E,f);return}}else E.isExternalTexture&&(U.__webglTexture=E.sourceTexture?E.sourceTexture:null);t.bindTexture(n.TEXTURE_2D,U.__webglTexture,n.TEXTURE0+f)}function Y(E,f){const U=i.get(E);if(E.isRenderTargetTexture===!1&&E.version>0&&U.__version!==E.version){se(U,E,f);return}else E.isExternalTexture&&(U.__webglTexture=E.sourceTexture?E.sourceTexture:null);t.bindTexture(n.TEXTURE_2D_ARRAY,U.__webglTexture,n.TEXTURE0+f)}function ne(E,f){const U=i.get(E);if(E.isRenderTargetTexture===!1&&E.version>0&&U.__version!==E.version){se(U,E,f);return}t.bindTexture(n.TEXTURE_3D,U.__webglTexture,n.TEXTURE0+f)}function de(E,f){const U=i.get(E);if(E.isCubeDepthTexture!==!0&&E.version>0&&U.__version!==E.version){_e(U,E,f);return}t.bindTexture(n.TEXTURE_CUBE_MAP,U.__webglTexture,n.TEXTURE0+f)}const ce={[go]:n.REPEAT,[Si]:n.CLAMP_TO_EDGE,[_o]:n.MIRRORED_REPEAT},J={[rn]:n.NEAREST,[tf]:n.NEAREST_MIPMAP_NEAREST,[ys]:n.NEAREST_MIPMAP_LINEAR,[sn]:n.LINEAR,[wa]:n.LINEAR_MIPMAP_NEAREST,[rr]:n.LINEAR_MIPMAP_LINEAR},ue={[sf]:n.NEVER,[df]:n.ALWAYS,[af]:n.LESS,[pl]:n.LEQUAL,[of]:n.EQUAL,[ml]:n.GEQUAL,[lf]:n.GREATER,[cf]:n.NOTEQUAL};function me(E,f){if(f.type===ei&&e.has("OES_texture_float_linear")===!1&&(f.magFilter===sn||f.magFilter===wa||f.magFilter===ys||f.magFilter===rr||f.minFilter===sn||f.minFilter===wa||f.minFilter===ys||f.minFilter===rr)&&tt("WebGLRenderer: Unable to use linear filtering with floating point textures. OES_texture_float_linear not supported on this device."),n.texParameteri(E,n.TEXTURE_WRAP_S,ce[f.wrapS]),n.texParameteri(E,n.TEXTURE_WRAP_T,ce[f.wrapT]),(E===n.TEXTURE_3D||E===n.TEXTURE_2D_ARRAY)&&n.texParameteri(E,n.TEXTURE_WRAP_R,ce[f.wrapR]),n.texParameteri(E,n.TEXTURE_MAG_FILTER,J[f.magFilter]),n.texParameteri(E,n.TEXTURE_MIN_FILTER,J[f.minFilter]),f.compareFunction&&(n.texParameteri(E,n.TEXTURE_COMPARE_MODE,n.COMPARE_REF_TO_TEXTURE),n.texParameteri(E,n.TEXTURE_COMPARE_FUNC,ue[f.compareFunction])),e.has("EXT_texture_filter_anisotropic")===!0){if(f.magFilter===rn||f.minFilter!==ys&&f.minFilter!==rr||f.type===ei&&e.has("OES_texture_float_linear")===!1)return;if(f.anisotropy>1||i.get(f).__currentAnisotropy){const U=e.get("EXT_texture_filter_anisotropic");n.texParameterf(E,U.TEXTURE_MAX_ANISOTROPY_EXT,Math.min(f.anisotropy,r.getMaxAnisotropy())),i.get(f).__currentAnisotropy=f.anisotropy}}}function B(E,f){let U=!1;E.__webglInit===void 0&&(E.__webglInit=!0,f.addEventListener("dispose",D));const O=f.source;let H=h.get(O);H===void 0&&(H={},h.set(O,H));const fe=R(f);if(fe!==E.__cacheKey){H[fe]===void 0&&(H[fe]={texture:n.createTexture(),usedTimes:0},a.memory.textures++,U=!0),H[fe].usedTimes++;const he=H[E.__cacheKey];he!==void 0&&(H[E.__cacheKey].usedTimes--,he.usedTimes===0&&w(f)),E.__cacheKey=fe,E.__webglTexture=H[fe].texture}return U}function oe(E,f,U){return Math.floor(Math.floor(E/U)/f)}function $(E,f,U,O){const fe=E.updateRanges;if(fe.length===0)t.texSubImage2D(n.TEXTURE_2D,0,0,0,f.width,f.height,U,O,f.data);else{fe.sort((Ue,we)=>Ue.start-we.start);let he=0;for(let Ue=1;Ue<fe.length;Ue++){const we=fe[he],Te=fe[Ue],He=we.start+we.count,Ge=oe(Te.start,f.width,4),it=oe(we.start,f.width,4);Te.start<=He+1&&Ge===it&&oe(Te.start+Te.count-1,f.width,4)===Ge?we.count=Math.max(we.count,Te.start+Te.count-we.start):(++he,fe[he]=Te)}fe.length=he+1;const ee=t.getParameter(n.UNPACK_ROW_LENGTH),te=t.getParameter(n.UNPACK_SKIP_PIXELS),Se=t.getParameter(n.UNPACK_SKIP_ROWS);t.pixelStorei(n.UNPACK_ROW_LENGTH,f.width);for(let Ue=0,we=fe.length;Ue<we;Ue++){const Te=fe[Ue],He=Math.floor(Te.start/4),Ge=Math.ceil(Te.count/4),it=He%f.width,X=Math.floor(He/f.width),Ce=Ge,ge=1;t.pixelStorei(n.UNPACK_SKIP_PIXELS,it),t.pixelStorei(n.UNPACK_SKIP_ROWS,X),t.texSubImage2D(n.TEXTURE_2D,0,it,X,Ce,ge,U,O,f.data)}E.clearUpdateRanges(),t.pixelStorei(n.UNPACK_ROW_LENGTH,ee),t.pixelStorei(n.UNPACK_SKIP_PIXELS,te),t.pixelStorei(n.UNPACK_SKIP_ROWS,Se)}}function se(E,f,U){let O=n.TEXTURE_2D;(f.isDataArrayTexture||f.isCompressedArrayTexture)&&(O=n.TEXTURE_2D_ARRAY),f.isData3DTexture&&(O=n.TEXTURE_3D);const H=B(E,f),fe=f.source;t.bindTexture(O,E.__webglTexture,n.TEXTURE0+U);const he=i.get(fe);if(fe.version!==he.__version||H===!0){if(t.activeTexture(n.TEXTURE0+U),(typeof ImageBitmap<"u"&&f.image instanceof ImageBitmap)===!1){const ge=xt.getPrimaries(xt.workingColorSpace),Pe=f.colorSpace===xi?null:xt.getPrimaries(f.colorSpace),Le=f.colorSpace===xi||ge===Pe?n.NONE:n.BROWSER_DEFAULT_WEBGL;t.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,f.flipY),t.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,f.premultiplyAlpha),t.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,Le)}t.pixelStorei(n.UNPACK_ALIGNMENT,f.unpackAlignment);let te=g(f.image,!1,r.maxTextureSize);te=at(f,te);const Se=s.convert(f.format,f.colorSpace),Ue=s.convert(f.type);let we=M(f.internalFormat,Se,Ue,f.normalized,f.colorSpace,f.isVideoTexture);me(O,f);let Te;const He=f.mipmaps,Ge=f.isVideoTexture!==!0,it=he.__version===void 0||H===!0,X=fe.dataReady,Ce=T(f,te);if(f.isDepthTexture)we=I(f.format===sr,f.type),it&&(Ge?t.texStorage2D(n.TEXTURE_2D,1,we,te.width,te.height):t.texImage2D(n.TEXTURE_2D,0,we,te.width,te.height,0,Se,Ue,null));else if(f.isDataTexture)if(He.length>0){Ge&&it&&t.texStorage2D(n.TEXTURE_2D,Ce,we,He[0].width,He[0].height);for(let ge=0,Pe=He.length;ge<Pe;ge++)Te=He[ge],Ge?X&&t.texSubImage2D(n.TEXTURE_2D,ge,0,0,Te.width,Te.height,Se,Ue,Te.data):t.texImage2D(n.TEXTURE_2D,ge,we,Te.width,Te.height,0,Se,Ue,Te.data);f.generateMipmaps=!1}else Ge?(it&&t.texStorage2D(n.TEXTURE_2D,Ce,we,te.width,te.height),X&&$(f,te,Se,Ue)):t.texImage2D(n.TEXTURE_2D,0,we,te.width,te.height,0,Se,Ue,te.data);else if(f.isCompressedTexture)if(f.isCompressedArrayTexture){Ge&&it&&t.texStorage3D(n.TEXTURE_2D_ARRAY,Ce,we,He[0].width,He[0].height,te.depth);for(let ge=0,Pe=He.length;ge<Pe;ge++)if(Te=He[ge],f.format!==Bn)if(Se!==null)if(Ge){if(X)if(f.layerUpdates.size>0){const Le=rc(Te.width,Te.height,f.format,f.type);for(const ye of f.layerUpdates){const Ve=Te.data.subarray(ye*Le/Te.data.BYTES_PER_ELEMENT,(ye+1)*Le/Te.data.BYTES_PER_ELEMENT);t.compressedTexSubImage3D(n.TEXTURE_2D_ARRAY,ge,0,0,ye,Te.width,Te.height,1,Se,Ve)}f.clearLayerUpdates()}else t.compressedTexSubImage3D(n.TEXTURE_2D_ARRAY,ge,0,0,0,Te.width,Te.height,te.depth,Se,Te.data)}else t.compressedTexImage3D(n.TEXTURE_2D_ARRAY,ge,we,Te.width,Te.height,te.depth,0,Te.data,0,0);else tt("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()");else Ge?X&&t.texSubImage3D(n.TEXTURE_2D_ARRAY,ge,0,0,0,Te.width,Te.height,te.depth,Se,Ue,Te.data):t.texImage3D(n.TEXTURE_2D_ARRAY,ge,we,Te.width,Te.height,te.depth,0,Se,Ue,Te.data)}else{Ge&&it&&t.texStorage2D(n.TEXTURE_2D,Ce,we,He[0].width,He[0].height);for(let ge=0,Pe=He.length;ge<Pe;ge++)Te=He[ge],f.format!==Bn?Se!==null?Ge?X&&t.compressedTexSubImage2D(n.TEXTURE_2D,ge,0,0,Te.width,Te.height,Se,Te.data):t.compressedTexImage2D(n.TEXTURE_2D,ge,we,Te.width,Te.height,0,Te.data):tt("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()"):Ge?X&&t.texSubImage2D(n.TEXTURE_2D,ge,0,0,Te.width,Te.height,Se,Ue,Te.data):t.texImage2D(n.TEXTURE_2D,ge,we,Te.width,Te.height,0,Se,Ue,Te.data)}else if(f.isDataArrayTexture)if(Ge){if(it&&t.texStorage3D(n.TEXTURE_2D_ARRAY,Ce,we,te.width,te.height,te.depth),X)if(f.layerUpdates.size>0){const ge=rc(te.width,te.height,f.format,f.type);for(const Pe of f.layerUpdates){const Le=te.data.subarray(Pe*ge/te.data.BYTES_PER_ELEMENT,(Pe+1)*ge/te.data.BYTES_PER_ELEMENT);t.texSubImage3D(n.TEXTURE_2D_ARRAY,0,0,0,Pe,te.width,te.height,1,Se,Ue,Le)}f.clearLayerUpdates()}else t.texSubImage3D(n.TEXTURE_2D_ARRAY,0,0,0,0,te.width,te.height,te.depth,Se,Ue,te.data)}else t.texImage3D(n.TEXTURE_2D_ARRAY,0,we,te.width,te.height,te.depth,0,Se,Ue,te.data);else if(f.isData3DTexture)Ge?(it&&t.texStorage3D(n.TEXTURE_3D,Ce,we,te.width,te.height,te.depth),X&&t.texSubImage3D(n.TEXTURE_3D,0,0,0,0,te.width,te.height,te.depth,Se,Ue,te.data)):t.texImage3D(n.TEXTURE_3D,0,we,te.width,te.height,te.depth,0,Se,Ue,te.data);else if(f.isFramebufferTexture){if(it)if(Ge)t.texStorage2D(n.TEXTURE_2D,Ce,we,te.width,te.height);else{let ge=te.width,Pe=te.height;for(let Le=0;Le<Ce;Le++)t.texImage2D(n.TEXTURE_2D,Le,we,ge,Pe,0,Se,Ue,null),ge>>=1,Pe>>=1}}else if(f.isHTMLTexture){if("texElementImage2D"in n){const ge=n.canvas;if(ge.hasAttribute("layoutsubtree")||ge.setAttribute("layoutsubtree","true"),te.parentNode!==ge){ge.appendChild(te),u.add(f),ge.onpaint=Pe=>{const Le=Pe.changedElements;for(const ye of u)Le.includes(ye.image)&&(ye.needsUpdate=!0)},ge.requestPaint();return}if(n.texElementImage2D.length===3)n.texElementImage2D(n.TEXTURE_2D,n.RGBA8,te);else{const Le=n.RGBA,ye=n.RGBA,Ve=n.UNSIGNED_BYTE;n.texElementImage2D(n.TEXTURE_2D,0,Le,ye,Ve,te)}n.texParameteri(n.TEXTURE_2D,n.TEXTURE_MIN_FILTER,n.LINEAR),n.texParameteri(n.TEXTURE_2D,n.TEXTURE_WRAP_S,n.CLAMP_TO_EDGE),n.texParameteri(n.TEXTURE_2D,n.TEXTURE_WRAP_T,n.CLAMP_TO_EDGE)}}else if(He.length>0){if(Ge&&it){const ge=$e(He[0]);t.texStorage2D(n.TEXTURE_2D,Ce,we,ge.width,ge.height)}for(let ge=0,Pe=He.length;ge<Pe;ge++)Te=He[ge],Ge?X&&t.texSubImage2D(n.TEXTURE_2D,ge,0,0,Se,Ue,Te):t.texImage2D(n.TEXTURE_2D,ge,we,Se,Ue,Te);f.generateMipmaps=!1}else if(Ge){if(it){const ge=$e(te);t.texStorage2D(n.TEXTURE_2D,Ce,we,ge.width,ge.height)}X&&t.texSubImage2D(n.TEXTURE_2D,0,0,0,Se,Ue,te)}else t.texImage2D(n.TEXTURE_2D,0,we,Se,Ue,te);m(f)&&A(O),he.__version=fe.version,f.onUpdate&&f.onUpdate(f)}E.__version=f.version}function _e(E,f,U){if(f.image.length!==6)return;const O=B(E,f),H=f.source;t.bindTexture(n.TEXTURE_CUBE_MAP,E.__webglTexture,n.TEXTURE0+U);const fe=i.get(H);if(H.version!==fe.__version||O===!0){t.activeTexture(n.TEXTURE0+U);const he=xt.getPrimaries(xt.workingColorSpace),ee=f.colorSpace===xi?null:xt.getPrimaries(f.colorSpace),te=f.colorSpace===xi||he===ee?n.NONE:n.BROWSER_DEFAULT_WEBGL;t.pixelStorei(n.UNPACK_FLIP_Y_WEBGL,f.flipY),t.pixelStorei(n.UNPACK_PREMULTIPLY_ALPHA_WEBGL,f.premultiplyAlpha),t.pixelStorei(n.UNPACK_ALIGNMENT,f.unpackAlignment),t.pixelStorei(n.UNPACK_COLORSPACE_CONVERSION_WEBGL,te);const Se=f.isCompressedTexture||f.image[0].isCompressedTexture,Ue=f.image[0]&&f.image[0].isDataTexture,we=[];for(let ye=0;ye<6;ye++)!Se&&!Ue?we[ye]=g(f.image[ye],!0,r.maxCubemapSize):we[ye]=Ue?f.image[ye].image:f.image[ye],we[ye]=at(f,we[ye]);const Te=we[0],He=s.convert(f.format,f.colorSpace),Ge=s.convert(f.type),it=M(f.internalFormat,He,Ge,f.normalized,f.colorSpace),X=f.isVideoTexture!==!0,Ce=fe.__version===void 0||O===!0,ge=H.dataReady;let Pe=T(f,Te);me(n.TEXTURE_CUBE_MAP,f);let Le;if(Se){X&&Ce&&t.texStorage2D(n.TEXTURE_CUBE_MAP,Pe,it,Te.width,Te.height);for(let ye=0;ye<6;ye++){Le=we[ye].mipmaps;for(let Ve=0;Ve<Le.length;Ve++){const Ee=Le[Ve];f.format!==Bn?He!==null?X?ge&&t.compressedTexSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve,0,0,Ee.width,Ee.height,He,Ee.data):t.compressedTexImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve,it,Ee.width,Ee.height,0,Ee.data):tt("WebGLRenderer: Attempt to load unsupported compressed texture format in .setTextureCube()"):X?ge&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve,0,0,Ee.width,Ee.height,He,Ge,Ee.data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve,it,Ee.width,Ee.height,0,He,Ge,Ee.data)}}}else{if(Le=f.mipmaps,X&&Ce){Le.length>0&&Pe++;const ye=$e(we[0]);t.texStorage2D(n.TEXTURE_CUBE_MAP,Pe,it,ye.width,ye.height)}for(let ye=0;ye<6;ye++)if(Ue){X?ge&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,0,0,0,we[ye].width,we[ye].height,He,Ge,we[ye].data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,0,it,we[ye].width,we[ye].height,0,He,Ge,we[ye].data);for(let Ve=0;Ve<Le.length;Ve++){const Dt=Le[Ve].image[ye].image;X?ge&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve+1,0,0,Dt.width,Dt.height,He,Ge,Dt.data):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve+1,it,Dt.width,Dt.height,0,He,Ge,Dt.data)}}else{X?ge&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,0,0,0,He,Ge,we[ye]):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,0,it,He,Ge,we[ye]);for(let Ve=0;Ve<Le.length;Ve++){const Ee=Le[Ve];X?ge&&t.texSubImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve+1,0,0,He,Ge,Ee.image[ye]):t.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+ye,Ve+1,it,He,Ge,Ee.image[ye])}}}m(f)&&A(n.TEXTURE_CUBE_MAP),fe.__version=H.version,f.onUpdate&&f.onUpdate(f)}E.__version=f.version}function Me(E,f,U,O,H,fe){const he=s.convert(U.format,U.colorSpace),ee=s.convert(U.type),te=M(U.internalFormat,he,ee,U.normalized,U.colorSpace),Se=i.get(f),Ue=i.get(U);if(Ue.__renderTarget=f,!Se.__hasExternalTextures){const we=Math.max(1,f.width>>fe),Te=Math.max(1,f.height>>fe);H===n.TEXTURE_3D||H===n.TEXTURE_2D_ARRAY?t.texImage3D(H,fe,te,we,Te,f.depth,0,he,ee,null):t.texImage2D(H,fe,te,we,Te,0,he,ee,null)}t.bindFramebuffer(n.FRAMEBUFFER,E),Ne(f)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,O,H,Ue.__webglTexture,0,Ze(f)):(H===n.TEXTURE_2D||H>=n.TEXTURE_CUBE_MAP_POSITIVE_X&&H<=n.TEXTURE_CUBE_MAP_NEGATIVE_Z)&&n.framebufferTexture2D(n.FRAMEBUFFER,O,H,Ue.__webglTexture,fe),t.bindFramebuffer(n.FRAMEBUFFER,null)}function ve(E,f,U){if(n.bindRenderbuffer(n.RENDERBUFFER,E),f.depthBuffer){const O=f.depthTexture,H=O&&O.isDepthTexture?O.type:null,fe=I(f.stencilBuffer,H),he=f.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;Ne(f)?o.renderbufferStorageMultisampleEXT(n.RENDERBUFFER,Ze(f),fe,f.width,f.height):U?n.renderbufferStorageMultisample(n.RENDERBUFFER,Ze(f),fe,f.width,f.height):n.renderbufferStorage(n.RENDERBUFFER,fe,f.width,f.height),n.framebufferRenderbuffer(n.FRAMEBUFFER,he,n.RENDERBUFFER,E)}else{const O=f.textures;for(let H=0;H<O.length;H++){const fe=O[H],he=s.convert(fe.format,fe.colorSpace),ee=s.convert(fe.type),te=M(fe.internalFormat,he,ee,fe.normalized,fe.colorSpace);Ne(f)?o.renderbufferStorageMultisampleEXT(n.RENDERBUFFER,Ze(f),te,f.width,f.height):U?n.renderbufferStorageMultisample(n.RENDERBUFFER,Ze(f),te,f.width,f.height):n.renderbufferStorage(n.RENDERBUFFER,te,f.width,f.height)}}n.bindRenderbuffer(n.RENDERBUFFER,null)}function F(E,f,U){const O=f.isWebGLCubeRenderTarget===!0;if(t.bindFramebuffer(n.FRAMEBUFFER,E),!(f.depthTexture&&f.depthTexture.isDepthTexture))throw new Error("THREE.WebGLTextures: renderTarget.depthTexture must be an instance of THREE.DepthTexture.");const H=i.get(f.depthTexture);if(H.__renderTarget=f,(!H.__webglTexture||f.depthTexture.image.width!==f.width||f.depthTexture.image.height!==f.height)&&(f.depthTexture.image.width=f.width,f.depthTexture.image.height=f.height,f.depthTexture.needsUpdate=!0),O){if(H.__webglInit===void 0&&(H.__webglInit=!0,f.depthTexture.addEventListener("dispose",D)),H.__webglTexture===void 0){H.__webglTexture=n.createTexture(),t.bindTexture(n.TEXTURE_CUBE_MAP,H.__webglTexture),me(n.TEXTURE_CUBE_MAP,f.depthTexture);const Se=s.convert(f.depthTexture.format),Ue=s.convert(f.depthTexture.type);let we;f.depthTexture.format===Ai?we=n.DEPTH_COMPONENT24:f.depthTexture.format===sr&&(we=n.DEPTH24_STENCIL8);for(let Te=0;Te<6;Te++)n.texImage2D(n.TEXTURE_CUBE_MAP_POSITIVE_X+Te,0,we,f.width,f.height,0,Se,Ue,null)}}else G(f.depthTexture,0);const fe=H.__webglTexture,he=Ze(f),ee=O?n.TEXTURE_CUBE_MAP_POSITIVE_X+U:n.TEXTURE_2D,te=f.depthTexture.format===sr?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;if(f.depthTexture.format===Ai)Ne(f)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,te,ee,fe,0,he):n.framebufferTexture2D(n.FRAMEBUFFER,te,ee,fe,0);else if(f.depthTexture.format===sr)Ne(f)?o.framebufferTexture2DMultisampleEXT(n.FRAMEBUFFER,te,ee,fe,0,he):n.framebufferTexture2D(n.FRAMEBUFFER,te,ee,fe,0);else throw new Error("THREE.WebGLTextures: Unknown depthTexture format.")}function K(E){const f=i.get(E),U=E.isWebGLCubeRenderTarget===!0;if(f.__boundDepthTexture!==E.depthTexture){const O=E.depthTexture;if(f.__depthDisposeCallback&&f.__depthDisposeCallback(),O){const H=()=>{delete f.__boundDepthTexture,delete f.__depthDisposeCallback,O.removeEventListener("dispose",H)};O.addEventListener("dispose",H),f.__depthDisposeCallback=H}f.__boundDepthTexture=O}if(E.depthTexture&&!f.__autoAllocateDepthBuffer)if(U)for(let O=0;O<6;O++)F(f.__webglFramebuffer[O],E,O);else{const O=E.texture.mipmaps;O&&O.length>0?F(f.__webglFramebuffer[0],E,0):F(f.__webglFramebuffer,E,0)}else if(U){f.__webglDepthbuffer=[];for(let O=0;O<6;O++)if(t.bindFramebuffer(n.FRAMEBUFFER,f.__webglFramebuffer[O]),f.__webglDepthbuffer[O]===void 0)f.__webglDepthbuffer[O]=n.createRenderbuffer(),ve(f.__webglDepthbuffer[O],E,!1);else{const H=E.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,fe=f.__webglDepthbuffer[O];n.bindRenderbuffer(n.RENDERBUFFER,fe),n.framebufferRenderbuffer(n.FRAMEBUFFER,H,n.RENDERBUFFER,fe)}}else{const O=E.texture.mipmaps;if(O&&O.length>0?t.bindFramebuffer(n.FRAMEBUFFER,f.__webglFramebuffer[0]):t.bindFramebuffer(n.FRAMEBUFFER,f.__webglFramebuffer),f.__webglDepthbuffer===void 0)f.__webglDepthbuffer=n.createRenderbuffer(),ve(f.__webglDepthbuffer,E,!1);else{const H=E.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,fe=f.__webglDepthbuffer;n.bindRenderbuffer(n.RENDERBUFFER,fe),n.framebufferRenderbuffer(n.FRAMEBUFFER,H,n.RENDERBUFFER,fe)}}t.bindFramebuffer(n.FRAMEBUFFER,null)}function ae(E,f,U){const O=i.get(E);f!==void 0&&Me(O.__webglFramebuffer,E,E.texture,n.COLOR_ATTACHMENT0,n.TEXTURE_2D,0),U!==void 0&&K(E)}function xe(E){const f=E.texture,U=i.get(E),O=i.get(f);E.addEventListener("dispose",v);const H=E.textures,fe=E.isWebGLCubeRenderTarget===!0,he=H.length>1;if(he||(O.__webglTexture===void 0&&(O.__webglTexture=n.createTexture()),O.__version=f.version,a.memory.textures++),fe){U.__webglFramebuffer=[];for(let ee=0;ee<6;ee++)if(f.mipmaps&&f.mipmaps.length>0){U.__webglFramebuffer[ee]=[];for(let te=0;te<f.mipmaps.length;te++)U.__webglFramebuffer[ee][te]=n.createFramebuffer()}else U.__webglFramebuffer[ee]=n.createFramebuffer()}else{if(f.mipmaps&&f.mipmaps.length>0){U.__webglFramebuffer=[];for(let ee=0;ee<f.mipmaps.length;ee++)U.__webglFramebuffer[ee]=n.createFramebuffer()}else U.__webglFramebuffer=n.createFramebuffer();if(he)for(let ee=0,te=H.length;ee<te;ee++){const Se=i.get(H[ee]);Se.__webglTexture===void 0&&(Se.__webglTexture=n.createTexture(),a.memory.textures++)}if(E.samples>0&&Ne(E)===!1){U.__webglMultisampledFramebuffer=n.createFramebuffer(),U.__webglColorRenderbuffer=[],t.bindFramebuffer(n.FRAMEBUFFER,U.__webglMultisampledFramebuffer);for(let ee=0;ee<H.length;ee++){const te=H[ee];U.__webglColorRenderbuffer[ee]=n.createRenderbuffer(),n.bindRenderbuffer(n.RENDERBUFFER,U.__webglColorRenderbuffer[ee]);const Se=s.convert(te.format,te.colorSpace),Ue=s.convert(te.type),we=M(te.internalFormat,Se,Ue,te.normalized,te.colorSpace,E.isXRRenderTarget===!0),Te=Ze(E);n.renderbufferStorageMultisample(n.RENDERBUFFER,Te,we,E.width,E.height),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+ee,n.RENDERBUFFER,U.__webglColorRenderbuffer[ee])}n.bindRenderbuffer(n.RENDERBUFFER,null),E.depthBuffer&&(U.__webglDepthRenderbuffer=n.createRenderbuffer(),ve(U.__webglDepthRenderbuffer,E,!0)),t.bindFramebuffer(n.FRAMEBUFFER,null)}}if(fe){t.bindTexture(n.TEXTURE_CUBE_MAP,O.__webglTexture),me(n.TEXTURE_CUBE_MAP,f);for(let ee=0;ee<6;ee++)if(f.mipmaps&&f.mipmaps.length>0)for(let te=0;te<f.mipmaps.length;te++)Me(U.__webglFramebuffer[ee][te],E,f,n.COLOR_ATTACHMENT0,n.TEXTURE_CUBE_MAP_POSITIVE_X+ee,te);else Me(U.__webglFramebuffer[ee],E,f,n.COLOR_ATTACHMENT0,n.TEXTURE_CUBE_MAP_POSITIVE_X+ee,0);m(f)&&A(n.TEXTURE_CUBE_MAP),t.unbindTexture()}else if(he){for(let ee=0,te=H.length;ee<te;ee++){const Se=H[ee],Ue=i.get(Se);let we=n.TEXTURE_2D;(E.isWebGL3DRenderTarget||E.isWebGLArrayRenderTarget)&&(we=E.isWebGL3DRenderTarget?n.TEXTURE_3D:n.TEXTURE_2D_ARRAY),t.bindTexture(we,Ue.__webglTexture),me(we,Se),Me(U.__webglFramebuffer,E,Se,n.COLOR_ATTACHMENT0+ee,we,0),m(Se)&&A(we)}t.unbindTexture()}else{let ee=n.TEXTURE_2D;if((E.isWebGL3DRenderTarget||E.isWebGLArrayRenderTarget)&&(ee=E.isWebGL3DRenderTarget?n.TEXTURE_3D:n.TEXTURE_2D_ARRAY),t.bindTexture(ee,O.__webglTexture),me(ee,f),f.mipmaps&&f.mipmaps.length>0)for(let te=0;te<f.mipmaps.length;te++)Me(U.__webglFramebuffer[te],E,f,n.COLOR_ATTACHMENT0,ee,te);else Me(U.__webglFramebuffer,E,f,n.COLOR_ATTACHMENT0,ee,0);m(f)&&A(ee),t.unbindTexture()}E.depthBuffer&&K(E)}function Ae(E){const f=E.textures;for(let U=0,O=f.length;U<O;U++){const H=f[U];if(m(H)){const fe=P(E),he=i.get(H).__webglTexture;t.bindTexture(fe,he),A(fe),t.unbindTexture()}}}const be=[],ke=[];function Xe(E){if(E.samples>0){if(Ne(E)===!1){const f=E.textures,U=E.width,O=E.height;let H=n.COLOR_BUFFER_BIT;const fe=E.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT,he=i.get(E),ee=f.length>1;if(ee)for(let Se=0;Se<f.length;Se++)t.bindFramebuffer(n.FRAMEBUFFER,he.__webglMultisampledFramebuffer),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.RENDERBUFFER,null),t.bindFramebuffer(n.FRAMEBUFFER,he.__webglFramebuffer),n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.TEXTURE_2D,null,0);t.bindFramebuffer(n.READ_FRAMEBUFFER,he.__webglMultisampledFramebuffer);const te=E.texture.mipmaps;te&&te.length>0?t.bindFramebuffer(n.DRAW_FRAMEBUFFER,he.__webglFramebuffer[0]):t.bindFramebuffer(n.DRAW_FRAMEBUFFER,he.__webglFramebuffer);for(let Se=0;Se<f.length;Se++){if(E.resolveDepthBuffer&&(E.depthBuffer&&(H|=n.DEPTH_BUFFER_BIT),E.stencilBuffer&&E.resolveStencilBuffer&&(H|=n.STENCIL_BUFFER_BIT)),ee){n.framebufferRenderbuffer(n.READ_FRAMEBUFFER,n.COLOR_ATTACHMENT0,n.RENDERBUFFER,he.__webglColorRenderbuffer[Se]);const Ue=i.get(f[Se]).__webglTexture;n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0,n.TEXTURE_2D,Ue,0)}n.blitFramebuffer(0,0,U,O,0,0,U,O,H,n.NEAREST),l===!0&&(be.length=0,ke.length=0,be.push(n.COLOR_ATTACHMENT0+Se),E.depthBuffer&&E.resolveDepthBuffer===!1&&(be.push(fe),ke.push(fe),n.invalidateFramebuffer(n.DRAW_FRAMEBUFFER,ke)),n.invalidateFramebuffer(n.READ_FRAMEBUFFER,be))}if(t.bindFramebuffer(n.READ_FRAMEBUFFER,null),t.bindFramebuffer(n.DRAW_FRAMEBUFFER,null),ee)for(let Se=0;Se<f.length;Se++){t.bindFramebuffer(n.FRAMEBUFFER,he.__webglMultisampledFramebuffer),n.framebufferRenderbuffer(n.FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.RENDERBUFFER,he.__webglColorRenderbuffer[Se]);const Ue=i.get(f[Se]).__webglTexture;t.bindFramebuffer(n.FRAMEBUFFER,he.__webglFramebuffer),n.framebufferTexture2D(n.DRAW_FRAMEBUFFER,n.COLOR_ATTACHMENT0+Se,n.TEXTURE_2D,Ue,0)}t.bindFramebuffer(n.DRAW_FRAMEBUFFER,he.__webglMultisampledFramebuffer)}else if(E.depthBuffer&&E.resolveDepthBuffer===!1&&l){const f=E.stencilBuffer?n.DEPTH_STENCIL_ATTACHMENT:n.DEPTH_ATTACHMENT;n.invalidateFramebuffer(n.DRAW_FRAMEBUFFER,[f])}}}function Ze(E){return Math.min(r.maxSamples,E.samples)}function Ne(E){const f=i.get(E);return E.samples>0&&e.has("WEBGL_multisampled_render_to_texture")===!0&&f.__useRenderToTexture!==!1}function N(E){const f=a.render.frame;p.get(E)!==f&&(p.set(E,f),E.update())}function at(E,f){const U=E.colorSpace,O=E.format,H=E.type;return E.isCompressedTexture===!0||E.isVideoTexture===!0||U!==ds&&U!==xi&&(xt.getTransfer(U)===It?(O!==Bn||H!==Cn)&&tt("WebGLTextures: sRGB encoded textures have to use RGBAFormat and UnsignedByteType."):Et("WebGLTextures: Unsupported texture color space:",U)),f}function $e(E){return typeof HTMLImageElement<"u"&&E instanceof HTMLImageElement?(c.width=E.naturalWidth||E.width,c.height=E.naturalHeight||E.height):typeof VideoFrame<"u"&&E instanceof VideoFrame?(c.width=E.displayWidth,c.height=E.displayHeight):(c.width=E.width,c.height=E.height),c}this.allocateTextureUnit=C,this.resetTextureUnits=Z,this.getTextureUnits=k,this.setTextureUnits=z,this.setTexture2D=G,this.setTexture2DArray=Y,this.setTexture3D=ne,this.setTextureCube=de,this.rebindTextures=ae,this.setupRenderTarget=xe,this.updateRenderTargetMipmap=Ae,this.updateMultisampleRenderTarget=Xe,this.setupDepthRenderbuffer=K,this.setupFrameBufferTexture=Me,this.useMultisampledRTT=Ne,this.isReversedDepthBuffer=function(){return t.buffers.depth.getReversed()}}function O_(n,e){function t(i,r=xi){let s;const a=xt.getTransfer(r);if(i===Cn)return n.UNSIGNED_BYTE;if(i===cl)return n.UNSIGNED_SHORT_4_4_4_4;if(i===dl)return n.UNSIGNED_SHORT_5_5_5_1;if(i===cd)return n.UNSIGNED_INT_5_9_9_9_REV;if(i===dd)return n.UNSIGNED_INT_10F_11F_11F_REV;if(i===od)return n.BYTE;if(i===ld)return n.SHORT;if(i===ls)return n.UNSIGNED_SHORT;if(i===ll)return n.INT;if(i===ii)return n.UNSIGNED_INT;if(i===ei)return n.FLOAT;if(i===wi)return n.HALF_FLOAT;if(i===ud)return n.ALPHA;if(i===fd)return n.RGB;if(i===Bn)return n.RGBA;if(i===Ai)return n.DEPTH_COMPONENT;if(i===sr)return n.DEPTH_STENCIL;if(i===hd)return n.RED;if(i===ul)return n.RED_INTEGER;if(i===cr)return n.RG;if(i===fl)return n.RG_INTEGER;if(i===hl)return n.RGBA_INTEGER;if(i===Zs||i===Js||i===Qs||i===js)if(a===It)if(s=e.get("WEBGL_compressed_texture_s3tc_srgb"),s!==null){if(i===Zs)return s.COMPRESSED_SRGB_S3TC_DXT1_EXT;if(i===Js)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT;if(i===Qs)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT;if(i===js)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT}else return null;else if(s=e.get("WEBGL_compressed_texture_s3tc"),s!==null){if(i===Zs)return s.COMPRESSED_RGB_S3TC_DXT1_EXT;if(i===Js)return s.COMPRESSED_RGBA_S3TC_DXT1_EXT;if(i===Qs)return s.COMPRESSED_RGBA_S3TC_DXT3_EXT;if(i===js)return s.COMPRESSED_RGBA_S3TC_DXT5_EXT}else return null;if(i===vo||i===xo||i===yo||i===So)if(s=e.get("WEBGL_compressed_texture_pvrtc"),s!==null){if(i===vo)return s.COMPRESSED_RGB_PVRTC_4BPPV1_IMG;if(i===xo)return s.COMPRESSED_RGB_PVRTC_2BPPV1_IMG;if(i===yo)return s.COMPRESSED_RGBA_PVRTC_4BPPV1_IMG;if(i===So)return s.COMPRESSED_RGBA_PVRTC_2BPPV1_IMG}else return null;if(i===Mo||i===bo||i===Eo||i===To||i===wo||i===ia||i===Ao)if(s=e.get("WEBGL_compressed_texture_etc"),s!==null){if(i===Mo||i===bo)return a===It?s.COMPRESSED_SRGB8_ETC2:s.COMPRESSED_RGB8_ETC2;if(i===Eo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ETC2_EAC:s.COMPRESSED_RGBA8_ETC2_EAC;if(i===To)return s.COMPRESSED_R11_EAC;if(i===wo)return s.COMPRESSED_SIGNED_R11_EAC;if(i===ia)return s.COMPRESSED_RG11_EAC;if(i===Ao)return s.COMPRESSED_SIGNED_RG11_EAC}else return null;if(i===Ro||i===Co||i===Po||i===Lo||i===Io||i===Do||i===Uo||i===No||i===Fo||i===Oo||i===Bo||i===ko||i===zo||i===Go)if(s=e.get("WEBGL_compressed_texture_astc"),s!==null){if(i===Ro)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_4x4_KHR:s.COMPRESSED_RGBA_ASTC_4x4_KHR;if(i===Co)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x4_KHR:s.COMPRESSED_RGBA_ASTC_5x4_KHR;if(i===Po)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x5_KHR:s.COMPRESSED_RGBA_ASTC_5x5_KHR;if(i===Lo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x5_KHR:s.COMPRESSED_RGBA_ASTC_6x5_KHR;if(i===Io)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x6_KHR:s.COMPRESSED_RGBA_ASTC_6x6_KHR;if(i===Do)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x5_KHR:s.COMPRESSED_RGBA_ASTC_8x5_KHR;if(i===Uo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x6_KHR:s.COMPRESSED_RGBA_ASTC_8x6_KHR;if(i===No)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x8_KHR:s.COMPRESSED_RGBA_ASTC_8x8_KHR;if(i===Fo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x5_KHR:s.COMPRESSED_RGBA_ASTC_10x5_KHR;if(i===Oo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x6_KHR:s.COMPRESSED_RGBA_ASTC_10x6_KHR;if(i===Bo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x8_KHR:s.COMPRESSED_RGBA_ASTC_10x8_KHR;if(i===ko)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x10_KHR:s.COMPRESSED_RGBA_ASTC_10x10_KHR;if(i===zo)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x10_KHR:s.COMPRESSED_RGBA_ASTC_12x10_KHR;if(i===Go)return a===It?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x12_KHR:s.COMPRESSED_RGBA_ASTC_12x12_KHR}else return null;if(i===Ho||i===Vo||i===Wo)if(s=e.get("EXT_texture_compression_bptc"),s!==null){if(i===Ho)return a===It?s.COMPRESSED_SRGB_ALPHA_BPTC_UNORM_EXT:s.COMPRESSED_RGBA_BPTC_UNORM_EXT;if(i===Vo)return s.COMPRESSED_RGB_BPTC_SIGNED_FLOAT_EXT;if(i===Wo)return s.COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT_EXT}else return null;if(i===Xo||i===qo||i===ra||i===Yo)if(s=e.get("EXT_texture_compression_rgtc"),s!==null){if(i===Xo)return s.COMPRESSED_RED_RGTC1_EXT;if(i===qo)return s.COMPRESSED_SIGNED_RED_RGTC1_EXT;if(i===ra)return s.COMPRESSED_RED_GREEN_RGTC2_EXT;if(i===Yo)return s.COMPRESSED_SIGNED_RED_GREEN_RGTC2_EXT}else return null;return i===cs?n.UNSIGNED_INT_24_8:n[i]!==void 0?n[i]:null}return{convert:t}}const B_=`
void main() {

	gl_Position = vec4( position, 1.0 );

}`,k_=`
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

}`;class z_{constructor(){this.texture=null,this.mesh=null,this.depthNear=0,this.depthFar=0}init(e,t){if(this.texture===null){const i=new bd(e.texture);(e.depthNear!==t.depthNear||e.depthFar!==t.depthFar)&&(this.depthNear=e.depthNear,this.depthFar=e.depthFar),this.texture=i}}getMesh(e){if(this.texture!==null&&this.mesh===null){const t=e.cameras[0].viewport,i=new si({vertexShader:B_,fragmentShader:k_,uniforms:{depthColor:{value:this.texture},depthWidth:{value:t.z},depthHeight:{value:t.w}}});this.mesh=new ri(new ga(20,20),i)}return this.mesh}reset(){this.texture=null,this.mesh=null}getDepthTexture(){return this.texture}}class G_ extends ur{constructor(e,t){super();const i=this;let r=null,s=1,a=null,o="local-floor",l=1,c=null,p=null,u=null,d=null,h=null,_=null;const x=typeof XRWebGLBinding<"u",g=new z_,m={},A=t.getContextAttributes();let P=null,M=null;const I=[],T=[],D=new Tt;let v=null;const S=new Fn;S.viewport=new Vt;const w=new Fn;w.viewport=new Vt;const L=[S,w],V=new Jf;let Z=null,k=null;this.cameraAutoUpdate=!0,this.enabled=!1,this.isPresenting=!1,this.getController=function(B){let oe=I[B];return oe===void 0&&(oe=new Ua,I[B]=oe),oe.getTargetRaySpace()},this.getControllerGrip=function(B){let oe=I[B];return oe===void 0&&(oe=new Ua,I[B]=oe),oe.getGripSpace()},this.getHand=function(B){let oe=I[B];return oe===void 0&&(oe=new Ua,I[B]=oe),oe.getHandSpace()};function z(B){const oe=T.indexOf(B.inputSource);if(oe===-1)return;const $=I[oe];$!==void 0&&($.update(B.inputSource,B.frame,c||a),$.dispatchEvent({type:B.type,data:B.inputSource}))}function C(){r.removeEventListener("select",z),r.removeEventListener("selectstart",z),r.removeEventListener("selectend",z),r.removeEventListener("squeeze",z),r.removeEventListener("squeezestart",z),r.removeEventListener("squeezeend",z),r.removeEventListener("end",C),r.removeEventListener("inputsourceschange",R);for(let B=0;B<I.length;B++){const oe=T[B];oe!==null&&(T[B]=null,I[B].disconnect(oe))}Z=null,k=null,g.reset();for(const B in m)delete m[B];e.setRenderTarget(P),h=null,d=null,u=null,r=null,M=null,me.stop(),i.isPresenting=!1,e.setPixelRatio(v),e.setSize(D.width,D.height,!1),i.dispatchEvent({type:"sessionend"})}this.setFramebufferScaleFactor=function(B){s=B,i.isPresenting===!0&&tt("WebXRManager: Cannot change framebuffer scale while presenting.")},this.setReferenceSpaceType=function(B){o=B,i.isPresenting===!0&&tt("WebXRManager: Cannot change reference space type while presenting.")},this.getReferenceSpace=function(){return c||a},this.setReferenceSpace=function(B){c=B},this.getBaseLayer=function(){return d!==null?d:h},this.getBinding=function(){return u===null&&x&&(u=new XRWebGLBinding(r,t)),u},this.getFrame=function(){return _},this.getSession=function(){return r},this.setSession=async function(B){if(r=B,r!==null){if(P=e.getRenderTarget(),r.addEventListener("select",z),r.addEventListener("selectstart",z),r.addEventListener("selectend",z),r.addEventListener("squeeze",z),r.addEventListener("squeezestart",z),r.addEventListener("squeezeend",z),r.addEventListener("end",C),r.addEventListener("inputsourceschange",R),A.xrCompatible!==!0&&await t.makeXRCompatible(),v=e.getPixelRatio(),e.getSize(D),x&&"createProjectionLayer"in XRWebGLBinding.prototype){let $=null,se=null,_e=null;A.depth&&(_e=A.stencil?t.DEPTH24_STENCIL8:t.DEPTH_COMPONENT24,$=A.stencil?sr:Ai,se=A.stencil?cs:ii);const Me={colorFormat:t.RGBA8,depthFormat:_e,scaleFactor:s};u=this.getBinding(),d=u.createProjectionLayer(Me),r.updateRenderState({layers:[d]}),e.setPixelRatio(1),e.setSize(d.textureWidth,d.textureHeight,!1),M=new ni(d.textureWidth,d.textureHeight,{format:Bn,type:Cn,depthTexture:new zr(d.textureWidth,d.textureHeight,se,void 0,void 0,void 0,void 0,void 0,void 0,$),stencilBuffer:A.stencil,colorSpace:e.outputColorSpace,samples:A.antialias?4:0,resolveDepthBuffer:d.ignoreDepthValues===!1,resolveStencilBuffer:d.ignoreDepthValues===!1})}else{const $={antialias:A.antialias,alpha:!0,depth:A.depth,stencil:A.stencil,framebufferScaleFactor:s};h=new XRWebGLLayer(r,t,$),r.updateRenderState({baseLayer:h}),e.setPixelRatio(1),e.setSize(h.framebufferWidth,h.framebufferHeight,!1),M=new ni(h.framebufferWidth,h.framebufferHeight,{format:Bn,type:Cn,colorSpace:e.outputColorSpace,stencilBuffer:A.stencil,resolveDepthBuffer:h.ignoreDepthValues===!1,resolveStencilBuffer:h.ignoreDepthValues===!1})}M.isXRRenderTarget=!0,this.setFoveation(l),c=null,a=await r.requestReferenceSpace(o),me.setContext(r),me.start(),i.isPresenting=!0,i.dispatchEvent({type:"sessionstart"})}},this.getEnvironmentBlendMode=function(){if(r!==null)return r.environmentBlendMode},this.getDepthTexture=function(){return g.getDepthTexture()};function R(B){for(let oe=0;oe<B.removed.length;oe++){const $=B.removed[oe],se=T.indexOf($);se>=0&&(T[se]=null,I[se].disconnect($))}for(let oe=0;oe<B.added.length;oe++){const $=B.added[oe];let se=T.indexOf($);if(se===-1){for(let Me=0;Me<I.length;Me++)if(Me>=T.length){T.push($),se=Me;break}else if(T[Me]===null){T[Me]=$,se=Me;break}if(se===-1)break}const _e=I[se];_e&&_e.connect($)}}const G=new re,Y=new re;function ne(B,oe,$){G.setFromMatrixPosition(oe.matrixWorld),Y.setFromMatrixPosition($.matrixWorld);const se=G.distanceTo(Y),_e=oe.projectionMatrix.elements,Me=$.projectionMatrix.elements,ve=_e[14]/(_e[10]-1),F=_e[14]/(_e[10]+1),K=(_e[9]+1)/_e[5],ae=(_e[9]-1)/_e[5],xe=(_e[8]-1)/_e[0],Ae=(Me[8]+1)/Me[0],be=ve*xe,ke=ve*Ae,Xe=se/(-xe+Ae),Ze=Xe*-xe;if(oe.matrixWorld.decompose(B.position,B.quaternion,B.scale),B.translateX(Ze),B.translateZ(Xe),B.matrixWorld.compose(B.position,B.quaternion,B.scale),B.matrixWorldInverse.copy(B.matrixWorld).invert(),_e[10]===-1)B.projectionMatrix.copy(oe.projectionMatrix),B.projectionMatrixInverse.copy(oe.projectionMatrixInverse);else{const Ne=ve+Xe,N=F+Xe,at=be-Ze,$e=ke+(se-Ze),E=K*F/N*Ne,f=ae*F/N*Ne;B.projectionMatrix.makePerspective(at,$e,E,f,Ne,N),B.projectionMatrixInverse.copy(B.projectionMatrix).invert()}}function de(B,oe){oe===null?B.matrixWorld.copy(B.matrix):B.matrixWorld.multiplyMatrices(oe.matrixWorld,B.matrix),B.matrixWorldInverse.copy(B.matrixWorld).invert()}this.updateCamera=function(B){if(r===null)return;let oe=B.near,$=B.far;g.texture!==null&&(g.depthNear>0&&(oe=g.depthNear),g.depthFar>0&&($=g.depthFar)),V.near=w.near=S.near=oe,V.far=w.far=S.far=$,(Z!==V.near||k!==V.far)&&(r.updateRenderState({depthNear:V.near,depthFar:V.far}),Z=V.near,k=V.far),V.layers.mask=B.layers.mask|6,S.layers.mask=V.layers.mask&-5,w.layers.mask=V.layers.mask&-3;const se=B.parent,_e=V.cameras;de(V,se);for(let Me=0;Me<_e.length;Me++)de(_e[Me],se);_e.length===2?ne(V,S,w):V.projectionMatrix.copy(S.projectionMatrix),ce(B,V,se)};function ce(B,oe,$){$===null?B.matrix.copy(oe.matrixWorld):(B.matrix.copy($.matrixWorld),B.matrix.invert(),B.matrix.multiply(oe.matrixWorld)),B.matrix.decompose(B.position,B.quaternion,B.scale),B.updateMatrixWorld(!0),B.projectionMatrix.copy(oe.projectionMatrix),B.projectionMatrixInverse.copy(oe.projectionMatrixInverse),B.isPerspectiveCamera&&(B.fov=Zo*2*Math.atan(1/B.projectionMatrix.elements[5]),B.zoom=1)}this.getCamera=function(){return V},this.getFoveation=function(){if(!(d===null&&h===null))return l},this.setFoveation=function(B){l=B,d!==null&&(d.fixedFoveation=B),h!==null&&h.fixedFoveation!==void 0&&(h.fixedFoveation=B)},this.hasDepthSensing=function(){return g.texture!==null},this.getDepthSensingMesh=function(){return g.getMesh(V)},this.getCameraTexture=function(B){return m[B]};let J=null;function ue(B,oe){if(p=oe.getViewerPose(c||a),_=oe,p!==null){const $=p.views;h!==null&&(e.setRenderTargetFramebuffer(M,h.framebuffer),e.setRenderTarget(M));let se=!1;$.length!==V.cameras.length&&(V.cameras.length=0,se=!0);for(let F=0;F<$.length;F++){const K=$[F];let ae=null;if(h!==null)ae=h.getViewport(K);else{const Ae=u.getViewSubImage(d,K);ae=Ae.viewport,F===0&&(e.setRenderTargetTextures(M,Ae.colorTexture,Ae.depthStencilTexture),e.setRenderTarget(M))}let xe=L[F];xe===void 0&&(xe=new Fn,xe.layers.enable(F),xe.viewport=new Vt,L[F]=xe),xe.matrix.fromArray(K.transform.matrix),xe.matrix.decompose(xe.position,xe.quaternion,xe.scale),xe.projectionMatrix.fromArray(K.projectionMatrix),xe.projectionMatrixInverse.copy(xe.projectionMatrix).invert(),xe.viewport.set(ae.x,ae.y,ae.width,ae.height),F===0&&(V.matrix.copy(xe.matrix),V.matrix.decompose(V.position,V.quaternion,V.scale)),se===!0&&V.cameras.push(xe)}const _e=r.enabledFeatures;if(_e&&_e.includes("depth-sensing")&&r.depthUsage=="gpu-optimized"&&x){u=i.getBinding();const F=u.getDepthInformation($[0]);F&&F.isValid&&F.texture&&g.init(F,r.renderState)}if(_e&&_e.includes("camera-access")&&x){e.state.unbindTexture(),u=i.getBinding();for(let F=0;F<$.length;F++){const K=$[F].camera;if(K){let ae=m[K];ae||(ae=new bd,m[K]=ae);const xe=u.getCameraImage(K);ae.sourceTexture=xe}}}}for(let $=0;$<I.length;$++){const se=T[$],_e=I[$];se!==null&&_e!==void 0&&_e.update(se,oe,c||a)}J&&J(B,oe),oe.detectedPlanes&&i.dispatchEvent({type:"planesdetected",data:oe}),_=null}const me=new Ad;me.setAnimationLoop(ue),this.setAnimationLoop=function(B){J=B},this.dispose=function(){}}}const H_=new Kt,Ud=new ct;Ud.set(-1,0,0,0,1,0,0,0,1);function V_(n,e){function t(g,m){g.matrixAutoUpdate===!0&&g.updateMatrix(),m.value.copy(g.matrix)}function i(g,m){m.color.getRGB(g.fogColor.value,Ed(n)),m.isFog?(g.fogNear.value=m.near,g.fogFar.value=m.far):m.isFogExp2&&(g.fogDensity.value=m.density)}function r(g,m,A,P,M){m.isNodeMaterial?m.uniformsNeedUpdate=!1:m.isMeshBasicMaterial?s(g,m):m.isMeshLambertMaterial?(s(g,m),m.envMap&&(g.envMapIntensity.value=m.envMapIntensity)):m.isMeshToonMaterial?(s(g,m),u(g,m)):m.isMeshPhongMaterial?(s(g,m),p(g,m),m.envMap&&(g.envMapIntensity.value=m.envMapIntensity)):m.isMeshStandardMaterial?(s(g,m),d(g,m),m.isMeshPhysicalMaterial&&h(g,m,M)):m.isMeshMatcapMaterial?(s(g,m),_(g,m)):m.isMeshDepthMaterial?s(g,m):m.isMeshDistanceMaterial?(s(g,m),x(g,m)):m.isMeshNormalMaterial?s(g,m):m.isLineBasicMaterial?(a(g,m),m.isLineDashedMaterial&&o(g,m)):m.isPointsMaterial?l(g,m,A,P):m.isSpriteMaterial?c(g,m):m.isShadowMaterial?(g.color.value.copy(m.color),g.opacity.value=m.opacity):m.isShaderMaterial&&(m.uniformsNeedUpdate=!1)}function s(g,m){g.opacity.value=m.opacity,m.color&&g.diffuse.value.copy(m.color),m.emissive&&g.emissive.value.copy(m.emissive).multiplyScalar(m.emissiveIntensity),m.map&&(g.map.value=m.map,t(m.map,g.mapTransform)),m.alphaMap&&(g.alphaMap.value=m.alphaMap,t(m.alphaMap,g.alphaMapTransform)),m.bumpMap&&(g.bumpMap.value=m.bumpMap,t(m.bumpMap,g.bumpMapTransform),g.bumpScale.value=m.bumpScale,m.side===_n&&(g.bumpScale.value*=-1)),m.normalMap&&(g.normalMap.value=m.normalMap,t(m.normalMap,g.normalMapTransform),g.normalScale.value.copy(m.normalScale),m.side===_n&&g.normalScale.value.negate()),m.displacementMap&&(g.displacementMap.value=m.displacementMap,t(m.displacementMap,g.displacementMapTransform),g.displacementScale.value=m.displacementScale,g.displacementBias.value=m.displacementBias),m.emissiveMap&&(g.emissiveMap.value=m.emissiveMap,t(m.emissiveMap,g.emissiveMapTransform)),m.specularMap&&(g.specularMap.value=m.specularMap,t(m.specularMap,g.specularMapTransform)),m.alphaTest>0&&(g.alphaTest.value=m.alphaTest);const A=e.get(m),P=A.envMap,M=A.envMapRotation;P&&(g.envMap.value=P,g.envMapRotation.value.setFromMatrix4(H_.makeRotationFromEuler(M)).transpose(),P.isCubeTexture&&P.isRenderTargetTexture===!1&&g.envMapRotation.value.premultiply(Ud),g.reflectivity.value=m.reflectivity,g.ior.value=m.ior,g.refractionRatio.value=m.refractionRatio),m.lightMap&&(g.lightMap.value=m.lightMap,g.lightMapIntensity.value=m.lightMapIntensity,t(m.lightMap,g.lightMapTransform)),m.aoMap&&(g.aoMap.value=m.aoMap,g.aoMapIntensity.value=m.aoMapIntensity,t(m.aoMap,g.aoMapTransform))}function a(g,m){g.diffuse.value.copy(m.color),g.opacity.value=m.opacity,m.map&&(g.map.value=m.map,t(m.map,g.mapTransform))}function o(g,m){g.dashSize.value=m.dashSize,g.totalSize.value=m.dashSize+m.gapSize,g.scale.value=m.scale}function l(g,m,A,P){g.diffuse.value.copy(m.color),g.opacity.value=m.opacity,g.size.value=m.size*A,g.scale.value=P*.5,m.map&&(g.map.value=m.map,t(m.map,g.uvTransform)),m.alphaMap&&(g.alphaMap.value=m.alphaMap,t(m.alphaMap,g.alphaMapTransform)),m.alphaTest>0&&(g.alphaTest.value=m.alphaTest)}function c(g,m){g.diffuse.value.copy(m.color),g.opacity.value=m.opacity,g.rotation.value=m.rotation,m.map&&(g.map.value=m.map,t(m.map,g.mapTransform)),m.alphaMap&&(g.alphaMap.value=m.alphaMap,t(m.alphaMap,g.alphaMapTransform)),m.alphaTest>0&&(g.alphaTest.value=m.alphaTest)}function p(g,m){g.specular.value.copy(m.specular),g.shininess.value=Math.max(m.shininess,1e-4)}function u(g,m){m.gradientMap&&(g.gradientMap.value=m.gradientMap)}function d(g,m){g.metalness.value=m.metalness,m.metalnessMap&&(g.metalnessMap.value=m.metalnessMap,t(m.metalnessMap,g.metalnessMapTransform)),g.roughness.value=m.roughness,m.roughnessMap&&(g.roughnessMap.value=m.roughnessMap,t(m.roughnessMap,g.roughnessMapTransform)),m.envMap&&(g.envMapIntensity.value=m.envMapIntensity)}function h(g,m,A){g.ior.value=m.ior,m.sheen>0&&(g.sheenColor.value.copy(m.sheenColor).multiplyScalar(m.sheen),g.sheenRoughness.value=m.sheenRoughness,m.sheenColorMap&&(g.sheenColorMap.value=m.sheenColorMap,t(m.sheenColorMap,g.sheenColorMapTransform)),m.sheenRoughnessMap&&(g.sheenRoughnessMap.value=m.sheenRoughnessMap,t(m.sheenRoughnessMap,g.sheenRoughnessMapTransform))),m.clearcoat>0&&(g.clearcoat.value=m.clearcoat,g.clearcoatRoughness.value=m.clearcoatRoughness,m.clearcoatMap&&(g.clearcoatMap.value=m.clearcoatMap,t(m.clearcoatMap,g.clearcoatMapTransform)),m.clearcoatRoughnessMap&&(g.clearcoatRoughnessMap.value=m.clearcoatRoughnessMap,t(m.clearcoatRoughnessMap,g.clearcoatRoughnessMapTransform)),m.clearcoatNormalMap&&(g.clearcoatNormalMap.value=m.clearcoatNormalMap,t(m.clearcoatNormalMap,g.clearcoatNormalMapTransform),g.clearcoatNormalScale.value.copy(m.clearcoatNormalScale),m.side===_n&&g.clearcoatNormalScale.value.negate())),m.dispersion>0&&(g.dispersion.value=m.dispersion),m.iridescence>0&&(g.iridescence.value=m.iridescence,g.iridescenceIOR.value=m.iridescenceIOR,g.iridescenceThicknessMinimum.value=m.iridescenceThicknessRange[0],g.iridescenceThicknessMaximum.value=m.iridescenceThicknessRange[1],m.iridescenceMap&&(g.iridescenceMap.value=m.iridescenceMap,t(m.iridescenceMap,g.iridescenceMapTransform)),m.iridescenceThicknessMap&&(g.iridescenceThicknessMap.value=m.iridescenceThicknessMap,t(m.iridescenceThicknessMap,g.iridescenceThicknessMapTransform))),m.transmission>0&&(g.transmission.value=m.transmission,g.transmissionSamplerMap.value=A.texture,g.transmissionSamplerSize.value.set(A.width,A.height),m.transmissionMap&&(g.transmissionMap.value=m.transmissionMap,t(m.transmissionMap,g.transmissionMapTransform)),g.thickness.value=m.thickness,m.thicknessMap&&(g.thicknessMap.value=m.thicknessMap,t(m.thicknessMap,g.thicknessMapTransform)),g.attenuationDistance.value=m.attenuationDistance,g.attenuationColor.value.copy(m.attenuationColor)),m.anisotropy>0&&(g.anisotropyVector.value.set(m.anisotropy*Math.cos(m.anisotropyRotation),m.anisotropy*Math.sin(m.anisotropyRotation)),m.anisotropyMap&&(g.anisotropyMap.value=m.anisotropyMap,t(m.anisotropyMap,g.anisotropyMapTransform))),g.specularIntensity.value=m.specularIntensity,g.specularColor.value.copy(m.specularColor),m.specularColorMap&&(g.specularColorMap.value=m.specularColorMap,t(m.specularColorMap,g.specularColorMapTransform)),m.specularIntensityMap&&(g.specularIntensityMap.value=m.specularIntensityMap,t(m.specularIntensityMap,g.specularIntensityMapTransform))}function _(g,m){m.matcap&&(g.matcap.value=m.matcap)}function x(g,m){const A=e.get(m).light;g.referencePosition.value.setFromMatrixPosition(A.matrixWorld),g.nearDistance.value=A.shadow.camera.near,g.farDistance.value=A.shadow.camera.far}return{refreshFogUniforms:i,refreshMaterialUniforms:r}}function W_(n,e,t,i){let r={},s={},a=[];const o=n.getParameter(n.MAX_UNIFORM_BUFFER_BINDINGS);function l(M,I){const T=I.program;i.uniformBlockBinding(M,T)}function c(M,I){let T=r[M.id];T===void 0&&(g(M),T=p(M),r[M.id]=T,M.addEventListener("dispose",A));const D=I.program;i.updateUBOMapping(M,D);const v=e.render.frame;s[M.id]!==v&&(d(M),s[M.id]=v)}function p(M){const I=u();M.__bindingPointIndex=I;const T=n.createBuffer(),D=M.__size,v=M.usage;return n.bindBuffer(n.UNIFORM_BUFFER,T),n.bufferData(n.UNIFORM_BUFFER,D,v),n.bindBuffer(n.UNIFORM_BUFFER,null),n.bindBufferBase(n.UNIFORM_BUFFER,I,T),T}function u(){for(let M=0;M<o;M++)if(a.indexOf(M)===-1)return a.push(M),M;return Et("WebGLRenderer: Maximum number of simultaneously usable uniforms groups reached."),0}function d(M){const I=r[M.id],T=M.uniforms,D=M.__cache;n.bindBuffer(n.UNIFORM_BUFFER,I);for(let v=0,S=T.length;v<S;v++){const w=T[v];if(Array.isArray(w))for(let L=0,V=w.length;L<V;L++)h(w[L],v,L,D);else h(w,v,0,D)}n.bindBuffer(n.UNIFORM_BUFFER,null)}function h(M,I,T,D){if(x(M,I,T,D)===!0){const v=M.__offset,S=M.value;if(Array.isArray(S)){let w=0;for(let L=0;L<S.length;L++){const V=S[L],Z=m(V);_(V,M.__data,w),typeof V!="number"&&typeof V!="boolean"&&!V.isMatrix3&&!ArrayBuffer.isView(V)&&(w+=Z.storage/Float32Array.BYTES_PER_ELEMENT)}}else _(S,M.__data,0);n.bufferSubData(n.UNIFORM_BUFFER,v,M.__data)}}function _(M,I,T){typeof M=="number"||typeof M=="boolean"?I[0]=M:M.isMatrix3?(I[0]=M.elements[0],I[1]=M.elements[1],I[2]=M.elements[2],I[3]=0,I[4]=M.elements[3],I[5]=M.elements[4],I[6]=M.elements[5],I[7]=0,I[8]=M.elements[6],I[9]=M.elements[7],I[10]=M.elements[8],I[11]=0):ArrayBuffer.isView(M)?I.set(new M.constructor(M.buffer,M.byteOffset,I.length)):M.toArray(I,T)}function x(M,I,T,D){const v=M.value,S=I+"_"+T;if(D[S]===void 0)return typeof v=="number"||typeof v=="boolean"?D[S]=v:ArrayBuffer.isView(v)?D[S]=v.slice():D[S]=v.clone(),!0;{const w=D[S];if(typeof v=="number"||typeof v=="boolean"){if(w!==v)return D[S]=v,!0}else{if(ArrayBuffer.isView(v))return!0;if(w.equals(v)===!1)return w.copy(v),!0}}return!1}function g(M){const I=M.uniforms;let T=0;const D=16;for(let S=0,w=I.length;S<w;S++){const L=Array.isArray(I[S])?I[S]:[I[S]];for(let V=0,Z=L.length;V<Z;V++){const k=L[V],z=Array.isArray(k.value)?k.value:[k.value];for(let C=0,R=z.length;C<R;C++){const G=z[C],Y=m(G),ne=T%D,de=ne%Y.boundary,ce=ne+de;T+=de,ce!==0&&D-ce<Y.storage&&(T+=D-ce),k.__data=new Float32Array(Y.storage/Float32Array.BYTES_PER_ELEMENT),k.__offset=T,T+=Y.storage}}}const v=T%D;return v>0&&(T+=D-v),M.__size=T,M.__cache={},this}function m(M){const I={boundary:0,storage:0};return typeof M=="number"||typeof M=="boolean"?(I.boundary=4,I.storage=4):M.isVector2?(I.boundary=8,I.storage=8):M.isVector3||M.isColor?(I.boundary=16,I.storage=12):M.isVector4?(I.boundary=16,I.storage=16):M.isMatrix3?(I.boundary=48,I.storage=48):M.isMatrix4?(I.boundary=64,I.storage=64):M.isTexture?tt("WebGLRenderer: Texture samplers can not be part of an uniforms group."):ArrayBuffer.isView(M)?(I.boundary=16,I.storage=M.byteLength):tt("WebGLRenderer: Unsupported uniform value type.",M),I}function A(M){const I=M.target;I.removeEventListener("dispose",A);const T=a.indexOf(I.__bindingPointIndex);a.splice(T,1),n.deleteBuffer(r[I.id]),delete r[I.id],delete s[I.id]}function P(){for(const M in r)n.deleteBuffer(r[M]);a=[],r={},s={}}return{bind:l,update:c,dispose:P}}const X_=new Uint16Array([12469,15057,12620,14925,13266,14620,13807,14376,14323,13990,14545,13625,14713,13328,14840,12882,14931,12528,14996,12233,15039,11829,15066,11525,15080,11295,15085,10976,15082,10705,15073,10495,13880,14564,13898,14542,13977,14430,14158,14124,14393,13732,14556,13410,14702,12996,14814,12596,14891,12291,14937,11834,14957,11489,14958,11194,14943,10803,14921,10506,14893,10278,14858,9960,14484,14039,14487,14025,14499,13941,14524,13740,14574,13468,14654,13106,14743,12678,14818,12344,14867,11893,14889,11509,14893,11180,14881,10751,14852,10428,14812,10128,14765,9754,14712,9466,14764,13480,14764,13475,14766,13440,14766,13347,14769,13070,14786,12713,14816,12387,14844,11957,14860,11549,14868,11215,14855,10751,14825,10403,14782,10044,14729,9651,14666,9352,14599,9029,14967,12835,14966,12831,14963,12804,14954,12723,14936,12564,14917,12347,14900,11958,14886,11569,14878,11247,14859,10765,14828,10401,14784,10011,14727,9600,14660,9289,14586,8893,14508,8533,15111,12234,15110,12234,15104,12216,15092,12156,15067,12010,15028,11776,14981,11500,14942,11205,14902,10752,14861,10393,14812,9991,14752,9570,14682,9252,14603,8808,14519,8445,14431,8145,15209,11449,15208,11451,15202,11451,15190,11438,15163,11384,15117,11274,15055,10979,14994,10648,14932,10343,14871,9936,14803,9532,14729,9218,14645,8742,14556,8381,14461,8020,14365,7603,15273,10603,15272,10607,15267,10619,15256,10631,15231,10614,15182,10535,15118,10389,15042,10167,14963,9787,14883,9447,14800,9115,14710,8665,14615,8318,14514,7911,14411,7507,14279,7198,15314,9675,15313,9683,15309,9712,15298,9759,15277,9797,15229,9773,15166,9668,15084,9487,14995,9274,14898,8910,14800,8539,14697,8234,14590,7790,14479,7409,14367,7067,14178,6621,15337,8619,15337,8631,15333,8677,15325,8769,15305,8871,15264,8940,15202,8909,15119,8775,15022,8565,14916,8328,14804,8009,14688,7614,14569,7287,14448,6888,14321,6483,14088,6171,15350,7402,15350,7419,15347,7480,15340,7613,15322,7804,15287,7973,15229,8057,15148,8012,15046,7846,14933,7611,14810,7357,14682,7069,14552,6656,14421,6316,14251,5948,14007,5528,15356,5942,15356,5977,15353,6119,15348,6294,15332,6551,15302,6824,15249,7044,15171,7122,15070,7050,14949,6861,14818,6611,14679,6349,14538,6067,14398,5651,14189,5311,13935,4958,15359,4123,15359,4153,15356,4296,15353,4646,15338,5160,15311,5508,15263,5829,15188,6042,15088,6094,14966,6001,14826,5796,14678,5543,14527,5287,14377,4985,14133,4586,13869,4257,15360,1563,15360,1642,15358,2076,15354,2636,15341,3350,15317,4019,15273,4429,15203,4732,15105,4911,14981,4932,14836,4818,14679,4621,14517,4386,14359,4156,14083,3795,13808,3437,15360,122,15360,137,15358,285,15355,636,15344,1274,15322,2177,15281,2765,15215,3223,15120,3451,14995,3569,14846,3567,14681,3466,14511,3305,14344,3121,14037,2800,13753,2467,15360,0,15360,1,15359,21,15355,89,15346,253,15325,479,15287,796,15225,1148,15133,1492,15008,1749,14856,1882,14685,1886,14506,1783,14324,1608,13996,1398,13702,1183]);let $n=null;function q_(){return $n===null&&($n=new Bf(X_,16,16,cr,wi),$n.name="DFG_LUT",$n.minFilter=sn,$n.magFilter=sn,$n.wrapS=Si,$n.wrapT=Si,$n.generateMipmaps=!1,$n.needsUpdate=!0),$n}class Y_{constructor(e={}){const{canvas:t=hf(),context:i=null,depth:r=!0,stencil:s=!1,alpha:a=!1,antialias:o=!1,premultipliedAlpha:l=!0,preserveDrawingBuffer:c=!1,powerPreference:p="default",failIfMajorPerformanceCaveat:u=!1,reversedDepthBuffer:d=!1,outputBufferType:h=Cn}=e;this.isWebGLRenderer=!0;let _;if(i!==null){if(typeof WebGLRenderingContext<"u"&&i instanceof WebGLRenderingContext)throw new Error("THREE.WebGLRenderer: WebGL 1 is not supported since r163.");_=i.getContextAttributes().alpha}else _=a;const x=h,g=new Set([hl,fl,ul]),m=new Set([Cn,ii,ls,cs,cl,dl]),A=new Uint32Array(4),P=new Int32Array(4),M=new re;let I=null,T=null;const D=[],v=[];let S=null;this.domElement=t,this.debug={checkShaderErrors:!0,onShaderError:null},this.autoClear=!0,this.autoClearColor=!0,this.autoClearDepth=!0,this.autoClearStencil=!0,this.sortObjects=!0,this.clippingPlanes=[],this.localClippingEnabled=!1,this.toneMapping=kn,this.toneMappingExposure=1,this.transmissionResolutionScale=1;const w=this;let L=!1,V=null,Z=null,k=null,z=null;this._outputColorSpace=Rn;let C=0,R=0,G=null,Y=-1,ne=null;const de=new Vt,ce=new Vt;let J=null;const ue=new Lt(0);let me=0,B=t.width,oe=t.height,$=1,se=null,_e=null;const Me=new Vt(0,0,B,oe),ve=new Vt(0,0,B,oe);let F=!1;const K=new Sd;let ae=!1,xe=!1;const Ae=new Kt,be=new re,ke=new Vt,Xe={background:null,fog:null,environment:null,overrideMaterial:null,isScene:!0};let Ze=!1;function Ne(){return G===null?$:1}let N=i;function at(y,q){return t.getContext(y,q)}try{const y={alpha:!0,depth:r,stencil:s,antialias:o,premultipliedAlpha:l,preserveDrawingBuffer:c,powerPreference:p,failIfMajorPerformanceCaveat:u};if("setAttribute"in t&&t.setAttribute("data-engine",`three.js r${ol}`),t.addEventListener("webglcontextlost",Dt,!1),t.addEventListener("webglcontextrestored",gt,!1),t.addEventListener("webglcontextcreationerror",St,!1),N===null){const q="webgl2";if(N=at(q,y),N===null)throw at(q)?new Error("THREE.WebGLRenderer: Error creating WebGL context with your selected attributes."):new Error("THREE.WebGLRenderer: Error creating WebGL context.")}}catch(y){throw Et("WebGLRenderer: "+y.message),y}let $e,E,f,U,O,H,fe,he,ee,te,Se,Ue,we,Te,He,Ge,it,X,Ce,ge,Pe,Le,ye;function Ve(){$e=new qm(N),$e.init(),Pe=new O_(N,$e),E=new Bm(N,$e,e,Pe),f=new N_(N,$e),E.reversedDepthBuffer&&d&&f.buffers.depth.setReversed(!0),Z=N.createFramebuffer(),k=N.createFramebuffer(),z=N.createFramebuffer(),U=new $m(N),O=new S_,H=new F_(N,$e,f,O,E,Pe,U),fe=new Xm(w),he=new jf(N),Le=new Fm(N,he),ee=new Ym(N,he,U,Le),te=new Jm(N,ee,he,Le,U),X=new Zm(N,E,H),He=new km(O),Se=new y_(w,fe,$e,E,Le,He),Ue=new V_(w,O),we=new b_,Te=new C_($e),it=new Nm(w,fe,f,te,_,l),Ge=new U_(w,te,E),ye=new W_(N,U,E,f),Ce=new Om(N,$e,U),ge=new Km(N,$e,U),U.programs=Se.programs,w.capabilities=E,w.extensions=$e,w.properties=O,w.renderLists=we,w.shadowMap=Ge,w.state=f,w.info=U}Ve(),x!==Cn&&(S=new jm(x,t.width,t.height,o,r,s));const Ee=new G_(w,N);this.xr=Ee,this.getContext=function(){return N},this.getContextAttributes=function(){return N.getContextAttributes()},this.forceContextLoss=function(){const y=$e.get("WEBGL_lose_context");y&&y.loseContext()},this.forceContextRestore=function(){const y=$e.get("WEBGL_lose_context");y&&y.restoreContext()},this.getPixelRatio=function(){return $},this.setPixelRatio=function(y){y!==void 0&&($=y,this.setSize(B,oe,!1))},this.getSize=function(y){return y.set(B,oe)},this.setSize=function(y,q,ie=!0){if(Ee.isPresenting){tt("WebGLRenderer: Can't change size while VR device is presenting.");return}B=y,oe=q,t.width=Math.floor(y*$),t.height=Math.floor(q*$),ie===!0&&(t.style.width=y+"px",t.style.height=q+"px"),S!==null&&S.setSize(t.width,t.height),this.setViewport(0,0,y,q)},this.getDrawingBufferSize=function(y){return y.set(B*$,oe*$).floor()},this.setDrawingBufferSize=function(y,q,ie){B=y,oe=q,$=ie,t.width=Math.floor(y*ie),t.height=Math.floor(q*ie),this.setViewport(0,0,y,q)},this.setEffects=function(y){if(x===Cn){Et("WebGLRenderer: setEffects() requires outputBufferType set to HalfFloatType or FloatType.");return}if(y){for(let q=0;q<y.length;q++)if(y[q].isOutputPass===!0){tt("WebGLRenderer: OutputPass is not needed in setEffects(). Tone mapping and color space conversion are applied automatically.");break}}S.setEffects(y||[])},this.getCurrentViewport=function(y){return y.copy(de)},this.getViewport=function(y){return y.copy(Me)},this.setViewport=function(y,q,ie,Q){y.isVector4?Me.set(y.x,y.y,y.z,y.w):Me.set(y,q,ie,Q),f.viewport(de.copy(Me).multiplyScalar($).round())},this.getScissor=function(y){return y.copy(ve)},this.setScissor=function(y,q,ie,Q){y.isVector4?ve.set(y.x,y.y,y.z,y.w):ve.set(y,q,ie,Q),f.scissor(ce.copy(ve).multiplyScalar($).round())},this.getScissorTest=function(){return F},this.setScissorTest=function(y){f.setScissorTest(F=y)},this.setOpaqueSort=function(y){se=y},this.setTransparentSort=function(y){_e=y},this.getClearColor=function(y){return y.copy(it.getClearColor())},this.setClearColor=function(){it.setClearColor(...arguments)},this.getClearAlpha=function(){return it.getClearAlpha()},this.setClearAlpha=function(){it.setClearAlpha(...arguments)},this.clear=function(y=!0,q=!0,ie=!0){let Q=0;if(y){let j=!1;if(G!==null){const Ie=G.texture.format;j=g.has(Ie)}if(j){const Ie=G.texture.type,Fe=m.has(Ie),Re=it.getClearColor(),Oe=it.getClearAlpha(),qe=Re.r,rt=Re.g,nt=Re.b;Fe?(A[0]=qe,A[1]=rt,A[2]=nt,A[3]=Oe,N.clearBufferuiv(N.COLOR,0,A)):(P[0]=qe,P[1]=rt,P[2]=nt,P[3]=Oe,N.clearBufferiv(N.COLOR,0,P))}else Q|=N.COLOR_BUFFER_BIT}q&&(Q|=N.DEPTH_BUFFER_BIT,this.state.buffers.depth.setMask(!0)),ie&&(Q|=N.STENCIL_BUFFER_BIT,this.state.buffers.stencil.setMask(4294967295)),Q!==0&&N.clear(Q)},this.clearColor=function(){this.clear(!0,!1,!1)},this.clearDepth=function(){this.clear(!1,!0,!1)},this.clearStencil=function(){this.clear(!1,!1,!0)},this.setNodesHandler=function(y){y.setRenderer(this),V=y},this.dispose=function(){t.removeEventListener("webglcontextlost",Dt,!1),t.removeEventListener("webglcontextrestored",gt,!1),t.removeEventListener("webglcontextcreationerror",St,!1),it.dispose(),we.dispose(),Te.dispose(),O.dispose(),fe.dispose(),te.dispose(),Le.dispose(),ye.dispose(),Se.dispose(),Ee.dispose(),Ee.removeEventListener("sessionstart",Yr),Ee.removeEventListener("sessionend",Vi),Vn.stop()};function Dt(y){y.preventDefault(),la("WebGLRenderer: Context Lost."),L=!0}function gt(){la("WebGLRenderer: Context Restored."),L=!1;const y=U.autoReset,q=Ge.enabled,ie=Ge.autoUpdate,Q=Ge.needsUpdate,j=Ge.type;Ve(),U.autoReset=y,Ge.enabled=q,Ge.autoUpdate=ie,Ge.needsUpdate=Q,Ge.type=j}function St(y){Et("WebGLRenderer: A WebGL context could not be created. Reason: ",y.statusMessage)}function tn(y){const q=y.target;q.removeEventListener("dispose",tn),nn(q)}function nn(y){ms(y),O.remove(y)}function ms(y){const q=O.get(y).programs;q!==void 0&&(q.forEach(function(ie){Se.releaseProgram(ie)}),y.isShaderMaterial&&Se.releaseShaderCache(y))}this.renderBufferDirect=function(y,q,ie,Q,j,Ie){q===null&&(q=Xe);const Fe=j.isMesh&&j.matrixWorld.determinantAffine()<0,Re=Pn(y,q,ie,Q,j);f.setMaterial(Q,Fe);let Oe=ie.index,qe=1;if(Q.wireframe===!0){if(Oe=ee.getWireframeAttribute(ie),Oe===void 0)return;qe=2}const rt=ie.drawRange,nt=ie.attributes.position;let Ye=rt.start*qe,At=(rt.start+rt.count)*qe;Ie!==null&&(Ye=Math.max(Ye,Ie.start*qe),At=Math.min(At,(Ie.start+Ie.count)*qe)),Oe!==null?(Ye=Math.max(Ye,0),At=Math.min(At,Oe.count)):nt!=null&&(Ye=Math.max(Ye,0),At=Math.min(At,nt.count));const Bt=At-Ye;if(Bt<0||Bt===1/0)return;Le.setup(j,Q,Re,ie,Oe);let Mt,Rt=Ce;if(Oe!==null&&(Mt=he.get(Oe),Rt=ge,Rt.setIndex(Mt)),j.isMesh)Q.wireframe===!0?(f.setLineWidth(Q.wireframeLinewidth*Ne()),Rt.setMode(N.LINES)):Rt.setMode(N.TRIANGLES);else if(j.isLine){let zt=Q.linewidth;zt===void 0&&(zt=1),f.setLineWidth(zt*Ne()),j.isLineSegments?Rt.setMode(N.LINES):j.isLineLoop?Rt.setMode(N.LINE_LOOP):Rt.setMode(N.LINE_STRIP)}else j.isPoints?Rt.setMode(N.POINTS):j.isSprite&&Rt.setMode(N.TRIANGLES);if(j.isBatchedMesh)if($e.get("WEBGL_multi_draw"))Rt.renderMultiDraw(j._multiDrawStarts,j._multiDrawCounts,j._multiDrawCount);else{const zt=j._multiDrawStarts,Be=j._multiDrawCounts,dn=j._multiDrawCount,dt=Oe?he.get(Oe).bytesPerElement:1,un=O.get(Q).currentProgram.getUniforms();for(let pn=0;pn<dn;pn++)un.setValue(N,"_gl_DrawID",pn),Rt.render(zt[pn]/dt,Be[pn])}else if(j.isInstancedMesh)Rt.renderInstances(Ye,Bt,j.count);else if(ie.isInstancedBufferGeometry){const zt=ie._maxInstanceCount!==void 0?ie._maxInstanceCount:1/0,Be=Math.min(ie.instanceCount,zt);Rt.renderInstances(Ye,Bt,Be)}else Rt.render(Ye,Bt)};function hr(y,q,ie){y.transparent===!0&&y.side===vi&&y.forceSinglePass===!1?(y.side=_n,y.needsUpdate=!0,ci(y,q,ie),y.side=Gi,y.needsUpdate=!0,ci(y,q,ie),y.side=vi):ci(y,q,ie)}this.compile=function(y,q,ie=null){ie===null&&(ie=y),T=Te.get(ie),T.init(q),v.push(T),ie.traverseVisible(function(j){j.isLight&&j.layers.test(q.layers)&&(T.pushLight(j),j.castShadow&&T.pushShadow(j))}),y!==ie&&y.traverseVisible(function(j){j.isLight&&j.layers.test(q.layers)&&(T.pushLight(j),j.castShadow&&T.pushShadow(j))}),T.setupLights();const Q=new Set;return y.traverse(function(j){if(!(j.isMesh||j.isPoints||j.isLine||j.isSprite))return;const Ie=j.material;if(Ie)if(Array.isArray(Ie))for(let Fe=0;Fe<Ie.length;Fe++){const Re=Ie[Fe];hr(Re,ie,j),Q.add(Re)}else hr(Ie,ie,j),Q.add(Ie)}),T=v.pop(),Q},this.compileAsync=function(y,q,ie=null){const Q=this.compile(y,q,ie);return new Promise(j=>{function Ie(){if(Q.forEach(function(Fe){O.get(Fe).currentProgram.isReady()&&Q.delete(Fe)}),Q.size===0){j(y);return}setTimeout(Ie,10)}$e.get("KHR_parallel_shader_compile")!==null?Ie():setTimeout(Ie,10)})};let qr=null;function ya(y){qr&&qr(y)}function Yr(){Vn.stop()}function Vi(){Vn.start()}const Vn=new Ad;Vn.setAnimationLoop(ya),typeof self<"u"&&Vn.setContext(self),this.setAnimationLoop=function(y){qr=y,Ee.setAnimationLoop(y),y===null?Vn.stop():Vn.start()},Ee.addEventListener("sessionstart",Yr),Ee.addEventListener("sessionend",Vi),this.render=function(y,q){if(q!==void 0&&q.isCamera!==!0){Et("WebGLRenderer.render: camera is not an instance of THREE.Camera.");return}if(L===!0)return;V!==null&&V.renderStart(y,q);const ie=Ee.enabled===!0&&Ee.isPresenting===!0,Q=S!==null&&(G===null||ie)&&S.begin(w,G);if(y.matrixWorldAutoUpdate===!0&&y.updateMatrixWorld(),q.parent===null&&q.matrixWorldAutoUpdate===!0&&q.updateMatrixWorld(),Ee.enabled===!0&&Ee.isPresenting===!0&&(S===null||S.isCompositing()===!1)&&(Ee.cameraAutoUpdate===!0&&Ee.updateCamera(q),q=Ee.getCamera()),y.isScene===!0&&y.onBeforeRender(w,y,q,G),T=Te.get(y,v.length),T.init(q),T.state.textureUnits=H.getTextureUnits(),v.push(T),Ae.multiplyMatrices(q.projectionMatrix,q.matrixWorldInverse),K.setFromProjectionMatrix(Ae,ti,q.reversedDepth),xe=this.localClippingEnabled,ae=He.init(this.clippingPlanes,xe),I=we.get(y,D.length),I.init(),D.push(I),Ee.enabled===!0&&Ee.isPresenting===!0){const Fe=w.xr.getDepthSensingMesh();Fe!==null&&pr(Fe,q,-1/0,w.sortObjects)}pr(y,q,0,w.sortObjects),I.finish(),w.sortObjects===!0&&I.sort(se,_e,q.reversedDepth),Ze=Ee.enabled===!1||Ee.isPresenting===!1||Ee.hasDepthSensing()===!1,Ze&&it.addToRenderList(I,y),this.info.render.frame++,this.info.autoReset===!0&&this.info.reset(),ae===!0&&He.beginShadows();const j=T.state.shadowsArray;if(Ge.render(j,y,q),ae===!0&&He.endShadows(),(Q&&S.hasRenderPass())===!1){const Fe=I.opaque,Re=I.transmissive;if(T.setupLights(),q.isArrayCamera){const Oe=q.cameras;if(Re.length>0)for(let qe=0,rt=Oe.length;qe<rt;qe++){const nt=Oe[qe];li(Fe,Re,y,nt)}Ze&&it.render(y);for(let qe=0,rt=Oe.length;qe<rt;qe++){const nt=Oe[qe];Wn(I,y,nt,nt.viewport)}}else Re.length>0&&li(Fe,Re,y,q),Ze&&it.render(y),Wn(I,y,q)}G!==null&&R===0&&(H.updateMultisampleRenderTarget(G),H.updateRenderTargetMipmap(G)),Q&&S.end(w),y.isScene===!0&&y.onAfterRender(w,y,q),Le.resetDefaultState(),Y=-1,ne=null,v.pop(),v.length>0?(T=v[v.length-1],H.setTextureUnits(T.state.textureUnits),ae===!0&&He.setGlobalState(w.clippingPlanes,T.state.camera)):T=null,D.pop(),D.length>0?I=D[D.length-1]:I=null,V!==null&&V.renderEnd()};function pr(y,q,ie,Q){if(y.visible===!1)return;if(y.layers.test(q.layers)){if(y.isGroup)ie=y.renderOrder;else if(y.isLOD)y.autoUpdate===!0&&y.update(q);else if(y.isLightProbeGrid)T.pushLightProbeGrid(y);else if(y.isLight)T.pushLight(y),y.castShadow&&T.pushShadow(y);else if(y.isSprite){if(!y.frustumCulled||K.intersectsSprite(y)){Q&&ke.setFromMatrixPosition(y.matrixWorld).applyMatrix4(Ae);const Fe=te.update(y),Re=y.material;Re.visible&&I.push(y,Fe,Re,ie,ke.z,null)}}else if((y.isMesh||y.isLine||y.isPoints)&&(!y.frustumCulled||K.intersectsObject(y))){const Fe=te.update(y),Re=y.material;if(Q&&(y.boundingSphere!==void 0?(y.boundingSphere===null&&y.computeBoundingSphere(),ke.copy(y.boundingSphere.center)):(Fe.boundingSphere===null&&Fe.computeBoundingSphere(),ke.copy(Fe.boundingSphere.center)),ke.applyMatrix4(y.matrixWorld).applyMatrix4(Ae)),Array.isArray(Re)){const Oe=Fe.groups;for(let qe=0,rt=Oe.length;qe<rt;qe++){const nt=Oe[qe],Ye=Re[nt.materialIndex];Ye&&Ye.visible&&I.push(y,Fe,Ye,ie,ke.z,nt)}}else Re.visible&&I.push(y,Fe,Re,ie,ke.z,null)}}const Ie=y.children;for(let Fe=0,Re=Ie.length;Fe<Re;Fe++)pr(Ie[Fe],q,ie,Q)}function Wn(y,q,ie,Q){const{opaque:j,transmissive:Ie,transparent:Fe}=y;T.setupLightsView(ie),ae===!0&&He.setGlobalState(w.clippingPlanes,ie),Q&&f.viewport(de.copy(Q)),j.length>0&&Wi(j,q,ie),Ie.length>0&&Wi(Ie,q,ie),Fe.length>0&&Wi(Fe,q,ie),f.buffers.depth.setTest(!0),f.buffers.depth.setMask(!0),f.buffers.color.setMask(!0),f.setPolygonOffset(!1)}function li(y,q,ie,Q){if((ie.isScene===!0?ie.overrideMaterial:null)!==null)return;if(T.state.transmissionRenderTarget[Q.id]===void 0){const Ye=$e.has("EXT_color_buffer_half_float")||$e.has("EXT_color_buffer_float");T.state.transmissionRenderTarget[Q.id]=new ni(1,1,{generateMipmaps:!0,type:Ye?wi:Cn,minFilter:rr,samples:Math.max(4,E.samples),stencilBuffer:s,resolveDepthBuffer:!1,resolveStencilBuffer:!1,colorSpace:xt.workingColorSpace})}const Ie=T.state.transmissionRenderTarget[Q.id],Fe=Q.viewport||de;Ie.setSize(Fe.z*w.transmissionResolutionScale,Fe.w*w.transmissionResolutionScale);const Re=w.getRenderTarget(),Oe=w.getActiveCubeFace(),qe=w.getActiveMipmapLevel();w.setRenderTarget(Ie),w.getClearColor(ue),me=w.getClearAlpha(),me<1&&w.setClearColor(16777215,.5),w.clear(),Ze&&it.render(ie);const rt=w.toneMapping;w.toneMapping=kn;const nt=Q.viewport;if(Q.viewport!==void 0&&(Q.viewport=void 0),T.setupLightsView(Q),ae===!0&&He.setGlobalState(w.clippingPlanes,Q),Wi(y,ie,Q),H.updateMultisampleRenderTarget(Ie),H.updateRenderTargetMipmap(Ie),$e.has("WEBGL_multisampled_render_to_texture")===!1){let Ye=!1;for(let At=0,Bt=q.length;At<Bt;At++){const Mt=q[At],{object:Rt,geometry:zt,material:Be,group:dn}=Mt;if(Be.side===vi&&Rt.layers.test(Q.layers)){const dt=Be.side;Be.side=_n,Be.needsUpdate=!0,Xi(Rt,ie,Q,zt,Be,dn),Be.side=dt,Be.needsUpdate=!0,Ye=!0}}Ye===!0&&(H.updateMultisampleRenderTarget(Ie),H.updateRenderTargetMipmap(Ie))}w.setRenderTarget(Re,Oe,qe),w.setClearColor(ue,me),nt!==void 0&&(Q.viewport=nt),w.toneMapping=rt}function Wi(y,q,ie){const Q=q.isScene===!0?q.overrideMaterial:null;for(let j=0,Ie=y.length;j<Ie;j++){const Fe=y[j],{object:Re,geometry:Oe,group:qe}=Fe;let rt=Fe.material;rt.allowOverride===!0&&Q!==null&&(rt=Q),Re.layers.test(ie.layers)&&Xi(Re,q,ie,Oe,rt,qe)}}function Xi(y,q,ie,Q,j,Ie){y.onBeforeRender(w,q,ie,Q,j,Ie),y.modelViewMatrix.multiplyMatrices(ie.matrixWorldInverse,y.matrixWorld),y.normalMatrix.getNormalMatrix(y.modelViewMatrix),j.onBeforeRender(w,q,ie,Q,y,Ie),j.transparent===!0&&j.side===vi&&j.forceSinglePass===!1?(j.side=_n,j.needsUpdate=!0,w.renderBufferDirect(ie,q,Q,j,y,Ie),j.side=Gi,j.needsUpdate=!0,w.renderBufferDirect(ie,q,Q,j,y,Ie),j.side=vi):w.renderBufferDirect(ie,q,Q,j,y,Ie),y.onAfterRender(w,q,ie,Q,j,Ie)}function ci(y,q,ie){q.isScene!==!0&&(q=Xe);const Q=O.get(y),j=T.state.lights,Ie=T.state.shadowsArray,Fe=j.state.version,Re=Se.getParameters(y,j.state,Ie,q,ie,T.state.lightProbeGridArray),Oe=Se.getProgramCacheKey(Re);let qe=Q.programs;Q.environment=y.isMeshStandardMaterial||y.isMeshLambertMaterial||y.isMeshPhongMaterial?q.environment:null,Q.fog=q.fog;const rt=y.isMeshStandardMaterial||y.isMeshLambertMaterial&&!y.envMap||y.isMeshPhongMaterial&&!y.envMap;Q.envMap=fe.get(y.envMap||Q.environment,rt),Q.envMapRotation=Q.environment!==null&&y.envMap===null?q.environmentRotation:y.envMapRotation,qe===void 0&&(y.addEventListener("dispose",tn),qe=new Map,Q.programs=qe);let nt=qe.get(Oe);if(nt!==void 0){if(Q.currentProgram===nt&&Q.lightsStateVersion===Fe)return gs(y,Re),nt}else Re.uniforms=Se.getUniforms(y),V!==null&&y.isNodeMaterial&&V.build(y,ie,Re),y.onBeforeCompile(Re,w),nt=Se.acquireProgram(Re,Oe),qe.set(Oe,nt),Q.uniforms=Re.uniforms;const Ye=Q.uniforms;return(!y.isShaderMaterial&&!y.isRawShaderMaterial||y.clipping===!0)&&(Ye.clippingPlanes=He.uniform),gs(y,Re),Q.needsLights=xn(y),Q.lightsStateVersion=Fe,Q.needsLights&&(Ye.ambientLightColor.value=j.state.ambient,Ye.lightProbe.value=j.state.probe,Ye.directionalLights.value=j.state.directional,Ye.directionalLightShadows.value=j.state.directionalShadow,Ye.spotLights.value=j.state.spot,Ye.spotLightShadows.value=j.state.spotShadow,Ye.rectAreaLights.value=j.state.rectArea,Ye.ltc_1.value=j.state.rectAreaLTC1,Ye.ltc_2.value=j.state.rectAreaLTC2,Ye.pointLights.value=j.state.point,Ye.pointLightShadows.value=j.state.pointShadow,Ye.hemisphereLights.value=j.state.hemi,Ye.directionalShadowMatrix.value=j.state.directionalShadowMatrix,Ye.spotLightMatrix.value=j.state.spotLightMatrix,Ye.spotLightMap.value=j.state.spotLightMap,Ye.pointShadowMatrix.value=j.state.pointShadowMatrix),Q.lightProbeGrid=T.state.lightProbeGridArray.length>0,Q.currentProgram=nt,Q.uniformsList=null,nt}function Ri(y){if(y.uniformsList===null){const q=y.currentProgram.getUniforms();y.uniformsList=ea.seqWithValue(q.seq,y.uniforms)}return y.uniformsList}function gs(y,q){const ie=O.get(y);ie.outputColorSpace=q.outputColorSpace,ie.batching=q.batching,ie.batchingColor=q.batchingColor,ie.instancing=q.instancing,ie.instancingColor=q.instancingColor,ie.instancingMorph=q.instancingMorph,ie.skinning=q.skinning,ie.morphTargets=q.morphTargets,ie.morphNormals=q.morphNormals,ie.morphColors=q.morphColors,ie.morphTargetsCount=q.morphTargetsCount,ie.numClippingPlanes=q.numClippingPlanes,ie.numIntersection=q.numClipIntersection,ie.vertexAlphas=q.vertexAlphas,ie.vertexTangents=q.vertexTangents,ie.toneMapping=q.toneMapping}function vn(y,q){if(y.length===0)return null;if(y.length===1)return y[0].texture!==null?y[0]:null;M.setFromMatrixPosition(q.matrixWorld);for(let ie=0,Q=y.length;ie<Q;ie++){const j=y[ie];if(j.texture!==null&&j.boundingBox.containsPoint(M))return j}return null}function Pn(y,q,ie,Q,j){q.isScene!==!0&&(q=Xe),H.resetTextureUnits();const Ie=q.fog,Fe=Q.isMeshStandardMaterial||Q.isMeshLambertMaterial||Q.isMeshPhongMaterial?q.environment:null,Re=G===null?w.outputColorSpace:G.isXRRenderTarget===!0?G.texture.colorSpace:xt.workingColorSpace,Oe=Q.isMeshStandardMaterial||Q.isMeshLambertMaterial&&!Q.envMap||Q.isMeshPhongMaterial&&!Q.envMap,qe=fe.get(Q.envMap||Fe,Oe),rt=Q.vertexColors===!0&&!!ie.attributes.color&&ie.attributes.color.itemSize===4,nt=!!ie.attributes.tangent&&(!!Q.normalMap||Q.anisotropy>0),Ye=!!ie.morphAttributes.position,At=!!ie.morphAttributes.normal,Bt=!!ie.morphAttributes.color;let Mt=kn;Q.toneMapped&&(G===null||G.isXRRenderTarget===!0)&&(Mt=w.toneMapping);const Rt=ie.morphAttributes.position||ie.morphAttributes.normal||ie.morphAttributes.color,zt=Rt!==void 0?Rt.length:0,Be=O.get(Q),dn=T.state.lights;if(ae===!0&&(xe===!0||y!==ne)){const Ut=y===ne&&Q.id===Y;He.setState(Q,y,Ut)}let dt=!1;Q.version===Be.__version?(Be.needsLights&&Be.lightsStateVersion!==dn.state.version||Be.outputColorSpace!==Re||j.isBatchedMesh&&Be.batching===!1||!j.isBatchedMesh&&Be.batching===!0||j.isBatchedMesh&&Be.batchingColor===!0&&j.colorTexture===null||j.isBatchedMesh&&Be.batchingColor===!1&&j.colorTexture!==null||j.isInstancedMesh&&Be.instancing===!1||!j.isInstancedMesh&&Be.instancing===!0||j.isSkinnedMesh&&Be.skinning===!1||!j.isSkinnedMesh&&Be.skinning===!0||j.isInstancedMesh&&Be.instancingColor===!0&&j.instanceColor===null||j.isInstancedMesh&&Be.instancingColor===!1&&j.instanceColor!==null||j.isInstancedMesh&&Be.instancingMorph===!0&&j.morphTexture===null||j.isInstancedMesh&&Be.instancingMorph===!1&&j.morphTexture!==null||Be.envMap!==qe||Q.fog===!0&&Be.fog!==Ie||Be.numClippingPlanes!==void 0&&(Be.numClippingPlanes!==He.numPlanes||Be.numIntersection!==He.numIntersection)||Be.vertexAlphas!==rt||Be.vertexTangents!==nt||Be.morphTargets!==Ye||Be.morphNormals!==At||Be.morphColors!==Bt||Be.toneMapping!==Mt||Be.morphTargetsCount!==zt||!!Be.lightProbeGrid!=T.state.lightProbeGridArray.length>0)&&(dt=!0):(dt=!0,Be.__version=Q.version);let un=Be.currentProgram;dt===!0&&(un=ci(Q,q,j),V&&Q.isNodeMaterial&&V.onUpdateProgram(Q,un,Be));let pn=!1,Ln=!1,Xn=!1;const Ct=un.getUniforms(),Ft=Be.uniforms;if(f.useProgram(un.program)&&(pn=!0,Ln=!0,Xn=!0),Q.id!==Y&&(Y=Q.id,Ln=!0),Be.needsLights){const Ut=vn(T.state.lightProbeGridArray,j);Be.lightProbeGrid!==Ut&&(Be.lightProbeGrid=Ut,Ln=!0)}if(pn||ne!==y){f.buffers.depth.getReversed()&&y.reversedDepth!==!0&&(y._reversedDepth=!0,y.updateProjectionMatrix()),Ct.setValue(N,"projectionMatrix",y.projectionMatrix),Ct.setValue(N,"viewMatrix",y.matrixWorldInverse);const Yn=Ct.map.cameraPosition;Yn!==void 0&&Yn.setValue(N,be.setFromMatrixPosition(y.matrixWorld)),E.logarithmicDepthBuffer&&Ct.setValue(N,"logDepthBufFC",2/(Math.log(y.far+1)/Math.LN2)),(Q.isMeshPhongMaterial||Q.isMeshToonMaterial||Q.isMeshLambertMaterial||Q.isMeshBasicMaterial||Q.isMeshStandardMaterial||Q.isShaderMaterial)&&Ct.setValue(N,"isOrthographic",y.isOrthographicCamera===!0),ne!==y&&(ne=y,Ln=!0,Xn=!0)}if(Be.needsLights&&(dn.state.directionalShadowMap.length>0&&Ct.setValue(N,"directionalShadowMap",dn.state.directionalShadowMap,H),dn.state.spotShadowMap.length>0&&Ct.setValue(N,"spotShadowMap",dn.state.spotShadowMap,H),dn.state.pointShadowMap.length>0&&Ct.setValue(N,"pointShadowMap",dn.state.pointShadowMap,H)),j.isSkinnedMesh){Ct.setOptional(N,j,"bindMatrix"),Ct.setOptional(N,j,"bindMatrixInverse");const Ut=j.skeleton;Ut&&(Ut.boneTexture===null&&Ut.computeBoneTexture(),Ct.setValue(N,"boneTexture",Ut.boneTexture,H))}j.isBatchedMesh&&(Ct.setOptional(N,j,"batchingTexture"),Ct.setValue(N,"batchingTexture",j._matricesTexture,H),Ct.setOptional(N,j,"batchingIdTexture"),Ct.setValue(N,"batchingIdTexture",j._indirectTexture,H),Ct.setOptional(N,j,"batchingColorTexture"),j._colorsTexture!==null&&Ct.setValue(N,"batchingColorTexture",j._colorsTexture,H));const qn=ie.morphAttributes;if((qn.position!==void 0||qn.normal!==void 0||qn.color!==void 0)&&X.update(j,ie,un),(Ln||Be.receiveShadow!==j.receiveShadow)&&(Be.receiveShadow=j.receiveShadow,Ct.setValue(N,"receiveShadow",j.receiveShadow)),(Q.isMeshStandardMaterial||Q.isMeshLambertMaterial||Q.isMeshPhongMaterial)&&Q.envMap===null&&q.environment!==null&&(Ft.envMapIntensity.value=q.environmentIntensity),Ft.dfgLUT!==void 0&&(Ft.dfgLUT.value=q_()),Ln){if(Ct.setValue(N,"toneMappingExposure",w.toneMappingExposure),Be.needsLights&&_s(Ft,Xn),Ie&&Q.fog===!0&&Ue.refreshFogUniforms(Ft,Ie),Ue.refreshMaterialUniforms(Ft,Q,$,oe,T.state.transmissionRenderTarget[y.id]),Be.needsLights&&Be.lightProbeGrid){const Ut=Be.lightProbeGrid;Ft.probesSH.value=Ut.texture,Ft.probesMin.value.copy(Ut.boundingBox.min),Ft.probesMax.value.copy(Ut.boundingBox.max),Ft.probesResolution.value.copy(Ut.resolution)}ea.upload(N,Ri(Be),Ft,H)}if(Q.isShaderMaterial&&Q.uniformsNeedUpdate===!0&&(ea.upload(N,Ri(Be),Ft,H),Q.uniformsNeedUpdate=!1),Q.isSpriteMaterial&&Ct.setValue(N,"center",j.center),Ct.setValue(N,"modelViewMatrix",j.modelViewMatrix),Ct.setValue(N,"normalMatrix",j.normalMatrix),Ct.setValue(N,"modelMatrix",j.matrixWorld),Q.uniformsGroups!==void 0){const Ut=Q.uniformsGroups;for(let Yn=0,Ci=Ut.length;Yn<Ci;Yn++){const Kr=Ut[Yn];ye.update(Kr,un),ye.bind(Kr,un)}}return un}function _s(y,q){y.ambientLightColor.needsUpdate=q,y.lightProbe.needsUpdate=q,y.directionalLights.needsUpdate=q,y.directionalLightShadows.needsUpdate=q,y.pointLights.needsUpdate=q,y.pointLightShadows.needsUpdate=q,y.spotLights.needsUpdate=q,y.spotLightShadows.needsUpdate=q,y.rectAreaLights.needsUpdate=q,y.hemisphereLights.needsUpdate=q}function xn(y){return y.isMeshLambertMaterial||y.isMeshToonMaterial||y.isMeshPhongMaterial||y.isMeshStandardMaterial||y.isShadowMaterial||y.isShaderMaterial&&y.lights===!0}this.getActiveCubeFace=function(){return C},this.getActiveMipmapLevel=function(){return R},this.getRenderTarget=function(){return G},this.setRenderTargetTextures=function(y,q,ie){const Q=O.get(y);Q.__autoAllocateDepthBuffer=y.resolveDepthBuffer===!1,Q.__autoAllocateDepthBuffer===!1&&(Q.__useRenderToTexture=!1),O.get(y.texture).__webglTexture=q,O.get(y.depthTexture).__webglTexture=Q.__autoAllocateDepthBuffer?void 0:ie,Q.__hasExternalTextures=!0},this.setRenderTargetFramebuffer=function(y,q){const ie=O.get(y);ie.__webglFramebuffer=q,ie.__useDefaultFramebuffer=q===void 0},this.setRenderTarget=function(y,q=0,ie=0){G=y,C=q,R=ie;let Q=null,j=!1,Ie=!1;if(y){const Re=O.get(y);if(Re.__useDefaultFramebuffer!==void 0){f.bindFramebuffer(N.FRAMEBUFFER,Re.__webglFramebuffer),de.copy(y.viewport),ce.copy(y.scissor),J=y.scissorTest,f.viewport(de),f.scissor(ce),f.setScissorTest(J),Y=-1;return}else if(Re.__webglFramebuffer===void 0)H.setupRenderTarget(y);else if(Re.__hasExternalTextures)H.rebindTextures(y,O.get(y.texture).__webglTexture,O.get(y.depthTexture).__webglTexture);else if(y.depthBuffer){const rt=y.depthTexture;if(Re.__boundDepthTexture!==rt){if(rt!==null&&O.has(rt)&&(y.width!==rt.image.width||y.height!==rt.image.height))throw new Error("THREE.WebGLRenderer: Attached DepthTexture is initialized to the incorrect size.");H.setupDepthRenderbuffer(y)}}const Oe=y.texture;(Oe.isData3DTexture||Oe.isDataArrayTexture||Oe.isCompressedArrayTexture)&&(Ie=!0);const qe=O.get(y).__webglFramebuffer;y.isWebGLCubeRenderTarget?(Array.isArray(qe[q])?Q=qe[q][ie]:Q=qe[q],j=!0):y.samples>0&&H.useMultisampledRTT(y)===!1?Q=O.get(y).__webglMultisampledFramebuffer:Array.isArray(qe)?Q=qe[ie]:Q=qe,de.copy(y.viewport),ce.copy(y.scissor),J=y.scissorTest}else de.copy(Me).multiplyScalar($).floor(),ce.copy(ve).multiplyScalar($).floor(),J=F;if(ie!==0&&(Q=Z),f.bindFramebuffer(N.FRAMEBUFFER,Q)&&f.drawBuffers(y,Q),f.viewport(de),f.scissor(ce),f.setScissorTest(J),j){const Re=O.get(y.texture);N.framebufferTexture2D(N.FRAMEBUFFER,N.COLOR_ATTACHMENT0,N.TEXTURE_CUBE_MAP_POSITIVE_X+q,Re.__webglTexture,ie)}else if(Ie){const Re=q;for(let Oe=0;Oe<y.textures.length;Oe++){const qe=O.get(y.textures[Oe]);N.framebufferTextureLayer(N.FRAMEBUFFER,N.COLOR_ATTACHMENT0+Oe,qe.__webglTexture,ie,Re)}}else if(y!==null&&ie!==0){const Re=O.get(y.texture);N.framebufferTexture2D(N.FRAMEBUFFER,N.COLOR_ATTACHMENT0,N.TEXTURE_2D,Re.__webglTexture,ie)}Y=-1},this.readRenderTargetPixels=function(y,q,ie,Q,j,Ie,Fe,Re=0){if(!(y&&y.isWebGLRenderTarget)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");return}let Oe=O.get(y).__webglFramebuffer;if(y.isWebGLCubeRenderTarget&&Fe!==void 0&&(Oe=Oe[Fe]),Oe){f.bindFramebuffer(N.FRAMEBUFFER,Oe);try{const qe=y.textures[Re],rt=qe.format,nt=qe.type;if(y.textures.length>1&&N.readBuffer(N.COLOR_ATTACHMENT0+Re),!E.textureFormatReadable(rt)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not in RGBA or implementation defined format.");return}if(!E.textureTypeReadable(nt)){Et("WebGLRenderer.readRenderTargetPixels: renderTarget is not in UnsignedByteType or implementation defined type.");return}q>=0&&q<=y.width-Q&&ie>=0&&ie<=y.height-j&&N.readPixels(q,ie,Q,j,Pe.convert(rt),Pe.convert(nt),Ie)}finally{const qe=G!==null?O.get(G).__webglFramebuffer:null;f.bindFramebuffer(N.FRAMEBUFFER,qe)}}},this.readRenderTargetPixelsAsync=async function(y,q,ie,Q,j,Ie,Fe,Re=0){if(!(y&&y.isWebGLRenderTarget))throw new Error("THREE.WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");let Oe=O.get(y).__webglFramebuffer;if(y.isWebGLCubeRenderTarget&&Fe!==void 0&&(Oe=Oe[Fe]),Oe)if(q>=0&&q<=y.width-Q&&ie>=0&&ie<=y.height-j){f.bindFramebuffer(N.FRAMEBUFFER,Oe);const qe=y.textures[Re],rt=qe.format,nt=qe.type;if(y.textures.length>1&&N.readBuffer(N.COLOR_ATTACHMENT0+Re),!E.textureFormatReadable(rt))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in RGBA or implementation defined format.");if(!E.textureTypeReadable(nt))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in UnsignedByteType or implementation defined type.");const Ye=N.createBuffer();N.bindBuffer(N.PIXEL_PACK_BUFFER,Ye),N.bufferData(N.PIXEL_PACK_BUFFER,Ie.byteLength,N.STREAM_READ),N.readPixels(q,ie,Q,j,Pe.convert(rt),Pe.convert(nt),0);const At=G!==null?O.get(G).__webglFramebuffer:null;f.bindFramebuffer(N.FRAMEBUFFER,At);const Bt=N.fenceSync(N.SYNC_GPU_COMMANDS_COMPLETE,0);return N.flush(),await pf(N,Bt,4),N.bindBuffer(N.PIXEL_PACK_BUFFER,Ye),N.getBufferSubData(N.PIXEL_PACK_BUFFER,0,Ie),N.deleteBuffer(Ye),N.deleteSync(Bt),Ie}else throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: requested read bounds are out of range.")},this.copyFramebufferToTexture=function(y,q=null,ie=0){const Q=Math.pow(2,-ie),j=Math.floor(y.image.width*Q),Ie=Math.floor(y.image.height*Q),Fe=q!==null?q.x:0,Re=q!==null?q.y:0;H.setTexture2D(y,0),N.copyTexSubImage2D(N.TEXTURE_2D,ie,0,0,Fe,Re,j,Ie),f.unbindTexture()},this.copyTextureToTexture=function(y,q,ie=null,Q=null,j=0,Ie=0){let Fe,Re,Oe,qe,rt,nt,Ye,At,Bt;const Mt=y.isCompressedTexture?y.mipmaps[Ie]:y.image;if(ie!==null)Fe=ie.max.x-ie.min.x,Re=ie.max.y-ie.min.y,Oe=ie.isBox3?ie.max.z-ie.min.z:1,qe=ie.min.x,rt=ie.min.y,nt=ie.isBox3?ie.min.z:0;else{const Ft=Math.pow(2,-j);Fe=Math.floor(Mt.width*Ft),Re=Math.floor(Mt.height*Ft),y.isDataArrayTexture?Oe=Mt.depth:y.isData3DTexture?Oe=Math.floor(Mt.depth*Ft):Oe=1,qe=0,rt=0,nt=0}Q!==null?(Ye=Q.x,At=Q.y,Bt=Q.z):(Ye=0,At=0,Bt=0);const Rt=Pe.convert(q.format),zt=Pe.convert(q.type);let Be;q.isData3DTexture?(H.setTexture3D(q,0),Be=N.TEXTURE_3D):q.isDataArrayTexture||q.isCompressedArrayTexture?(H.setTexture2DArray(q,0),Be=N.TEXTURE_2D_ARRAY):(H.setTexture2D(q,0),Be=N.TEXTURE_2D),f.activeTexture(N.TEXTURE0),f.pixelStorei(N.UNPACK_FLIP_Y_WEBGL,q.flipY),f.pixelStorei(N.UNPACK_PREMULTIPLY_ALPHA_WEBGL,q.premultiplyAlpha),f.pixelStorei(N.UNPACK_ALIGNMENT,q.unpackAlignment);const dn=f.getParameter(N.UNPACK_ROW_LENGTH),dt=f.getParameter(N.UNPACK_IMAGE_HEIGHT),un=f.getParameter(N.UNPACK_SKIP_PIXELS),pn=f.getParameter(N.UNPACK_SKIP_ROWS),Ln=f.getParameter(N.UNPACK_SKIP_IMAGES);f.pixelStorei(N.UNPACK_ROW_LENGTH,Mt.width),f.pixelStorei(N.UNPACK_IMAGE_HEIGHT,Mt.height),f.pixelStorei(N.UNPACK_SKIP_PIXELS,qe),f.pixelStorei(N.UNPACK_SKIP_ROWS,rt),f.pixelStorei(N.UNPACK_SKIP_IMAGES,nt);const Xn=y.isDataArrayTexture||y.isData3DTexture,Ct=q.isDataArrayTexture||q.isData3DTexture;if(y.isDepthTexture){const Ft=O.get(y),qn=O.get(q),Ut=O.get(Ft.__renderTarget),Yn=O.get(qn.__renderTarget);f.bindFramebuffer(N.READ_FRAMEBUFFER,Ut.__webglFramebuffer),f.bindFramebuffer(N.DRAW_FRAMEBUFFER,Yn.__webglFramebuffer);for(let Ci=0;Ci<Oe;Ci++)Xn&&(N.framebufferTextureLayer(N.READ_FRAMEBUFFER,N.COLOR_ATTACHMENT0,O.get(y).__webglTexture,j,nt+Ci),N.framebufferTextureLayer(N.DRAW_FRAMEBUFFER,N.COLOR_ATTACHMENT0,O.get(q).__webglTexture,Ie,Bt+Ci)),N.blitFramebuffer(qe,rt,Fe,Re,Ye,At,Fe,Re,N.DEPTH_BUFFER_BIT,N.NEAREST);f.bindFramebuffer(N.READ_FRAMEBUFFER,null),f.bindFramebuffer(N.DRAW_FRAMEBUFFER,null)}else if(j!==0||y.isRenderTargetTexture||O.has(y)){const Ft=O.get(y),qn=O.get(q);f.bindFramebuffer(N.READ_FRAMEBUFFER,k),f.bindFramebuffer(N.DRAW_FRAMEBUFFER,z);for(let Ut=0;Ut<Oe;Ut++)Xn?N.framebufferTextureLayer(N.READ_FRAMEBUFFER,N.COLOR_ATTACHMENT0,Ft.__webglTexture,j,nt+Ut):N.framebufferTexture2D(N.READ_FRAMEBUFFER,N.COLOR_ATTACHMENT0,N.TEXTURE_2D,Ft.__webglTexture,j),Ct?N.framebufferTextureLayer(N.DRAW_FRAMEBUFFER,N.COLOR_ATTACHMENT0,qn.__webglTexture,Ie,Bt+Ut):N.framebufferTexture2D(N.DRAW_FRAMEBUFFER,N.COLOR_ATTACHMENT0,N.TEXTURE_2D,qn.__webglTexture,Ie),j!==0?N.blitFramebuffer(qe,rt,Fe,Re,Ye,At,Fe,Re,N.COLOR_BUFFER_BIT,N.NEAREST):Ct?N.copyTexSubImage3D(Be,Ie,Ye,At,Bt+Ut,qe,rt,Fe,Re):N.copyTexSubImage2D(Be,Ie,Ye,At,qe,rt,Fe,Re);f.bindFramebuffer(N.READ_FRAMEBUFFER,null),f.bindFramebuffer(N.DRAW_FRAMEBUFFER,null)}else Ct?y.isDataTexture||y.isData3DTexture?N.texSubImage3D(Be,Ie,Ye,At,Bt,Fe,Re,Oe,Rt,zt,Mt.data):q.isCompressedArrayTexture?N.compressedTexSubImage3D(Be,Ie,Ye,At,Bt,Fe,Re,Oe,Rt,Mt.data):N.texSubImage3D(Be,Ie,Ye,At,Bt,Fe,Re,Oe,Rt,zt,Mt):y.isDataTexture?N.texSubImage2D(N.TEXTURE_2D,Ie,Ye,At,Fe,Re,Rt,zt,Mt.data):y.isCompressedTexture?N.compressedTexSubImage2D(N.TEXTURE_2D,Ie,Ye,At,Mt.width,Mt.height,Rt,Mt.data):N.texSubImage2D(N.TEXTURE_2D,Ie,Ye,At,Fe,Re,Rt,zt,Mt);f.pixelStorei(N.UNPACK_ROW_LENGTH,dn),f.pixelStorei(N.UNPACK_IMAGE_HEIGHT,dt),f.pixelStorei(N.UNPACK_SKIP_PIXELS,un),f.pixelStorei(N.UNPACK_SKIP_ROWS,pn),f.pixelStorei(N.UNPACK_SKIP_IMAGES,Ln),Ie===0&&q.generateMipmaps&&N.generateMipmap(Be),f.unbindTexture()},this.initRenderTarget=function(y){O.get(y).__webglFramebuffer===void 0&&H.setupRenderTarget(y)},this.initTexture=function(y){y.isCubeTexture?H.setTextureCube(y,0):y.isData3DTexture?H.setTexture3D(y,0):y.isDataArrayTexture||y.isCompressedArrayTexture?H.setTexture2DArray(y,0):H.setTexture2D(y,0),f.unbindTexture()},this.resetState=function(){C=0,R=0,G=null,f.reset(),Le.reset()},typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}get coordinateSystem(){return ti}get outputColorSpace(){return this._outputColorSpace}set outputColorSpace(e){this._outputColorSpace=e;const t=this.getContext();t.drawingBufferColorSpace=xt._getDrawingBufferColorSpace(e),t.unpackColorSpace=xt._getUnpackColorSpace()}}const Vs=11,K_=[0,2,1,0,3,2],$_=`
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
}`,Z_=`
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
}`,io=new Map;function Nd(n){if(io.has(n))return io.get(n);const e=n.replace("#","");if(![3,4,6,8].includes(e.length)||!/^[a-f0-9]+$/i.test(e))throw new Error("Unsupported scene color: "+n);const t=e.length<5?[...e].map(r=>r+r).join(""):e,i=[0,2,4].map(r=>parseInt(t.slice(r,r+2),16)/255);return i.push(t.length===8?parseInt(t.slice(6,8),16)/255:1),io.set(n,i),i}class Rc{constructor(e,t){this.a=e,this.b=t,this.stops=[]}addColorStop(e,t){this.stops.push([e,Nd(t)]),this.stops.sort((i,r)=>i[0]-r[0])}at(e,t){const i=this.b[0]-this.a[0],r=this.b[1]-this.a[1],s=Math.max(0,Math.min(1,((e-this.a[0])*i+(t-this.a[1])*r)/(i*i+r*r||1))),a=this.stops[0],o=this.stops.at(-1);return a[1].map((l,c)=>l+(o[1][c]-l)*s)}}class J_{constructor(e){this.canvas=e,this.renderer=new Y_({canvas:e,alpha:!1,antialias:!1,depth:!1,stencil:!1,premultipliedAlpha:!0}),this.renderer.setClearColor(0,1),this.renderer.outputColorSpace=ds,this.renderer.toneMapping=kn,this.renderer.sortObjects=!1,this.measure=document.createElement("canvas").getContext("2d"),this.atlas=document.createElement("canvas"),this.atlas.width=this.atlas.height=2048,this.atlasContext=this.atlas.getContext("2d"),this.atlasTexture=new Hf(this.atlas),this.atlasTexture.colorSpace=xi,this.atlasTexture.premultiplyAlpha=!0,this.atlasTexture.generateMipmaps=!1,this.atlasTexture.minFilter=this.atlasTexture.magFilter=sn,this.images=new Map,this.shelfX=2,this.shelfY=2,this.shelfHeight=0,this.resolution=new Tt(1,1),this.materials=["source-over","screen"].map(t=>new Td({name:"Sophia "+t,vertexShader:$_,fragmentShader:Z_,glslVersion:$o,uniforms:{uResolution:{value:this.resolution},uAtlas:{value:this.atlasTexture}},transparent:!0,depthTest:!1,depthWrite:!1,toneMapped:!1,blending:Zc,blendEquation:Oi,blendSrc:ao,blendDst:t==="screen"?Jc:os,blendSrcAlpha:ao,blendDstAlpha:os})),this.scene=new Pf,this.camera=new xl,this.capacity=0,this.mesh=null,this.allocate(16384),this.matrix=[1,0,0,1,0,0],this.stack=[],this.path=[],this.quadPoints=Array.from({length:4},()=>[0,0]),this.quadLocals=Array.from({length:4},()=>[0,0]),this.quadShape=[0,0,0],this.globalAlpha=1,this.globalCompositeOperation="source-over",this.fillStyle="#000000",this.strokeStyle="#000000",this.lineWidth=1,this.lost=!1,this.disposed=!1,e.addEventListener("webglcontextlost",t=>{t.preventDefault(),this.lost=!0,e.dataset.context="lost"}),e.addEventListener("webglcontextrestored",()=>{this.lost=!1,this.atlasTexture.needsUpdate=!0,e.dataset.context="restored",e.dispatchEvent(new CustomEvent("sophia-renderer-restored"))}),addEventListener("pagehide",t=>{t.persisted||this.dispose()})}get font(){return this.measure.font}set font(e){this.measure.font=e}measureText(e){return this.measure.measureText(e)}point(e,t){const i=this.matrix;return[i[0]*e+i[2]*t+i[4],i[1]*e+i[3]*t+i[5]]}setTransform(...e){this.matrix=e}translate(e,t){const i=this.point(e,t);this.matrix[4]=i[0],this.matrix[5]=i[1]}rotate(e){const[t,i,r,s,a,o]=this.matrix,l=Math.cos(e),c=Math.sin(e);this.matrix=[t*l+r*c,i*l+s*c,r*l-t*c,s*l-i*c,a,o]}save(){this.stack.push({matrix:this.matrix.slice(),globalAlpha:this.globalAlpha,globalCompositeOperation:this.globalCompositeOperation,fillStyle:this.fillStyle,strokeStyle:this.strokeStyle,lineWidth:this.lineWidth})}restore(){const e=this.stack.pop();e&&Object.assign(this,e)}createLinearGradient(e,t,i,r){return new Rc(this.point(e,t),this.point(i,r))}allocate(e){const t=this.data;this.geometry?.dispose(),this.capacity=e,this.data=new Float32Array(e*Vs),t&&this.data.set(t),this.buffer=new Uf(this.data,Vs).setUsage(uf),this.geometry=new ai;for(const[i,r,s]of[["aPosition",2,0],["aLocal",2,2],["aColor",4,4],["aShape",3,8]])this.geometry.setAttribute(i,new vl(this.buffer,r,s));this.mesh?this.mesh.geometry=this.geometry:(this.mesh=new ri(this.geometry,this.materials),this.mesh.frustumCulled=!1,this.scene.add(this.mesh))}beginFrame(){this.used=0,this.groups=[],(this.resolution.x!==this.canvas.width||this.resolution.y!==this.canvas.height)&&(this.resolution.set(this.canvas.width,this.canvas.height),this.renderer.setSize(this.canvas.width,this.canvas.height,!1))}endFrame(){this.lost||this.disposed||(this.geometry.clearGroups(),this.groups.forEach(e=>this.geometry.addGroup(e.start,e.count,e.material)),this.geometry.setDrawRange(0,this.used),this.buffer.clearUpdateRanges(),this.buffer.addUpdateRange(0,this.used*Vs),this.buffer.needsUpdate=!0,this.renderer.render(this.scene,this.camera),this.canvas.dataset.drawCalls=String(this.renderer.info.render.calls),this.canvas.dataset.gpuVertices=String(this.used),this.canvas.dataset.gpuTextures=String(this.renderer.info.memory.textures))}quad(e,t,i,r,s=this.globalAlpha){this.used+6>this.capacity&&this.allocate(this.capacity*2);const a=this.globalCompositeOperation==="screen"?1:0;let o=this.groups.at(-1);(!o||o.material!==a)&&(o={start:this.used,count:0,material:a},this.groups.push(o)),o.count+=6;const l=r instanceof Rc?null:Nd(r);for(const c of K_){const p=e[c],u=t[c],d=l||r.at(...p),h=d[3]*s,_=this.used++*Vs;this.data[_]=p[0],this.data[_+1]=p[1],this.data[_+2]=u[0],this.data[_+3]=u[1],this.data[_+4]=d[0]*h,this.data[_+5]=d[1]*h,this.data[_+6]=d[2]*h,this.data[_+7]=h,this.data[_+8]=i[0],this.data[_+9]=i[1],this.data[_+10]=i[2]}}atlasEntry(e){if(this.images.has(e))return this.images.get(e);const t=e.width,i=e.height;if(this.shelfX+t+2>2048&&(this.shelfX=2,this.shelfY+=this.shelfHeight+4,this.shelfHeight=0),this.shelfY+i+2>2048)throw new Error("Scene texture atlas budget exceeded");const r={x:this.shelfX,y:this.shelfY,width:t,height:i};return this.shelfX+=t+4,this.shelfHeight=Math.max(this.shelfHeight,i),this.images.set(e,r),this.invalidateImage(e),r}invalidateImage(e){const t=this.images.get(e);if(!t)return;const i=this.atlasContext,{x:r,y:s,width:a,height:o}=t;i.clearRect(r-1,s-1,a+2,o+2),i.drawImage(e,r,s),i.drawImage(e,0,0,a,1,r,s-1,a,1),i.drawImage(e,0,o-1,a,1,r,s+o,a,1),i.drawImage(e,0,0,1,o,r-1,s,1,o),i.drawImage(e,a-1,0,1,o,r+a,s,1,o);for(const[l,c,p,u]of[[0,0,r-1,s-1],[a-1,0,r+a,s-1],[0,o-1,r-1,s+o],[a-1,o-1,r+a,s+o]])i.drawImage(e,l,c,1,1,p,u,1,1);this.atlasTexture.needsUpdate=!0}drawImage(e,t,i,r,s){const a=this.atlasEntry(e),o=a.x/2048,l=1-a.y/2048,c=a.width/2048,p=a.height/2048,u=this.matrix,d=this.quadPoints,h=this.quadLocals,_=this.quadShape;for(let x=0;x<4;x++){const g=x===1||x===2,m=x>=2,A=g?t+r:t,P=m?i+s:i;d[x][0]=u[0]*A+u[2]*P+u[4],d[x][1]=u[1]*A+u[3]*P+u[5],h[x][0]=g?o+c:o,h[x][1]=m?l-p:l}_[0]=_[1]=_[2]=0,this.quad(d,h,_,"#ffffff")}rectAt(e,t,i,r,s,a,o){const l=s+1,c=a+1,p=this.quadPoints,u=this.quadLocals,d=this.quadShape;for(let h=0;h<4;h++){const _=h===1||h===2?l:-l,x=h>=2?c:-c;u[h][0]=_,u[h][1]=x,p[h][0]=e+i*_-r*x,p[h][1]=t+r*_+i*x}d[0]=1,d[1]=s,d[2]=a,this.quad(p,u,d,o)}fillRect(e,t,i,r){const s=this.matrix,a=Math.hypot(s[0],s[1]),o=Math.hypot(s[2],s[3]),l=e+i/2,c=t+r/2;this.rectAt(s[0]*l+s[2]*c+s[4],s[1]*l+s[3]*c+s[5],s[0]/a,s[1]/a,Math.abs(i*a/2),Math.abs(r*o/2),this.fillStyle)}beginPath(){this.path=[],this.current=null}moveTo(e,t){this.current=this.point(e,t)}lineTo(e,t){const i=this.point(e,t);this.current&&this.path.push({type:"line",a:this.current,b:i}),this.current=i}arc(e,t,i,r,s){if(Math.abs(s-r-Math.PI*2)>1e-4)throw new Error("Only full scene discs are supported");this.path.push({type:"disc",center:this.point(e,t),radius:i*Math.hypot(this.matrix[0],this.matrix[1])})}fill(){for(const e of this.path){if(e.type!=="disc")throw new Error("Only scene discs may be filled");const t=e.radius+1,i=[[-t,-t],[t,-t],[t,t],[-t,t]];this.quad(i.map(([r,s])=>[r+e.center[0],s+e.center[1]]),i,[2,e.radius,e.radius],this.fillStyle)}}stroke(){const e=this.lineWidth*Math.hypot(this.matrix[0],this.matrix[1])/2;if(this.path.length===2){const[t,i]=this.path;if(t.type==="line"&&i.type==="line"){const r=t.b[0]-t.a[0],s=t.b[1]-t.a[1],a=i.b[0]-i.a[0],o=i.b[1]-i.a[1],l=Math.hypot(r,s);if(l>0&&Math.abs(l-Math.hypot(a,o))<1e-5&&Math.abs(r*a+s*o)<1e-5&&Math.abs(t.a[0]+t.b[0]-i.a[0]-i.b[0])<1e-5&&Math.abs(t.a[1]+t.b[1]-i.a[1]-i.b[1])<1e-5){const c=(t.a[0]+t.b[0])/2,p=(t.a[1]+t.b[1])/2,u=r/l,d=s/l,h=l/2+1,_=[[-h,-h],[h,-h],[h,h],[-h,h]];this.quad(_.map(([x,g])=>[c+u*x-d*g,p+d*x+u*g]),_,[4,l/2,e],this.strokeStyle);return}}}for(let t=0;t<this.path.length;t++){const i=this.path[t],r=this.path[t+1];if(i.type!=="line")throw new Error("Only scene line segments may be stroked");const s=i.b[0]-i.a[0],a=i.b[1]-i.a[1],o=Math.hypot(s,a);if(r?.type==="line"&&i.b[0]===r.a[0]&&i.b[1]===r.a[1]){const l=r.b[0]-r.a[0],c=r.b[1]-r.a[1],p=Math.hypot(l,c);if(o>0&&Math.abs(o-p)<1e-5&&Math.abs(s*l+a*c)<1e-5){const u=[-s/o,-a/o],d=[l/o,c/o],h=-e-1,_=o+1;let x=[[h,h],[_,h],[_,_],[h,_]];u[0]*d[1]-u[1]*d[0]<0&&(x=[x[0],x[3],x[2],x[1]]),this.quad(x.map(([g,m])=>[i.b[0]+u[0]*g+d[0]*m,i.b[1]+u[1]*g+d[1]*m]),x,[3,o,e],this.strokeStyle),t++;continue}}o>0&&this.rectAt((i.a[0]+i.b[0])/2,(i.a[1]+i.b[1])/2,s/o,a/o,o/2,e,this.strokeStyle)}}dispose(){this.disposed||(this.disposed=!0,this.geometry.dispose(),this.materials.forEach(e=>e.dispose()),this.atlasTexture.dispose(),this.images.clear(),this.renderer.dispose())}}function Q_(n,{onChange:e=()=>{},initialFocus:t,initialLens:i,autoStart:r=!0}={}){const s=b=>n.querySelector(b);n.dataset.rendererBuild="c8d4575cf448b8707cdf55ae779d2743482a02f7348ae5e66f4972983bfe18cb";let a=s(".sc-sky"),o;try{o=new J_(a),n.dataset.renderer="three-webgl2"}catch(b){const W=a.cloneNode(!0);a.replaceWith(W),a=W,o=a.getContext("2d",{alpha:!1}),n.dataset.renderer="canvas-fallback",console.warn("GPU renderer unavailable; accepted Canvas painter retained.",b)}if(!o)return;const l=matchMedia("(prefers-reduced-motion: reduce)"),c={density:.85,pixel:.5,depth:1.25,liveliness:1,playing:!l.matches},p=1250;let u=[],d=[];const h=["#89accc","#dfc28f","#a49fdb"],_=new Map,x=s(".sc-nodes");let g=7239;const m=()=>(g=g*1664525+1013904223>>>0,g/4294967296),A=(b,W,le)=>{const pe=Math.max(0,Math.min(1,(le-b)/(W-b)));return pe*pe*(3-2*pe)},P=Array.from({length:2200},(b,W)=>{const le=W%25===0?2:W%5<2?1:0,pe=(m()-.5)*2.75,We=m()<.58&&le===0?pe*.34+(m()+m()+m()-1.5)*.54:(m()-.5)*2.7,je=[-2400,-920,220][le],ze=[1530,1150,570][le],ht=m()*Math.PI*2;return{layer:le,sx:pe,sy:We,minZ:je,range:ze,offset:m(),cycle:[230,135,92][le]*(.8+m()*.5),size:le===2?1.25+m()*1.5:le===1?.65+m()*.85:.38+m()*.65,phase:ht,freq:.55+m()*1.5,tone:Math.floor(m()*6),alpha:[.3,.58,.36][le]*(.55+m()*.65),flare:m()>.95,drift:(m()-.5)*26,x:0,y:0}}),M=["#bdd5ed","#95bde0","#e3eaf5","#dfcfb5","#aeb8e7","#f0e3c9"],I=new Map;function T(b){if(I.has(b))return I.get(b);const W=document.createElement("canvas");W.width=W.height=128;const le=W.getContext("2d"),pe=le.createRadialGradient(64,64,0,64,64,64);return pe.addColorStop(0,b+"c0"),pe.addColorStop(.05,b+"80"),pe.addColorStop(.16,b+"30"),pe.addColorStop(.45,b+"0c"),pe.addColorStop(1,b+"00"),le.fillStyle=pe,le.fillRect(0,0,128,128),I.set(b,W),W}const D=document.createElement("canvas");D.width=1100,D.height=780;const v=D.getContext("2d");function S(){v.fillStyle="#04070e",v.fillRect(0,0,1100,780);const b=[[210,260,390,"#111c33"],[620,410,375,"#142c31"],[930,555,330,"#231d38"],[585,405,170,"#242523"]];for(const[W,le,pe,We]of b){const je=v.createRadialGradient(W,le,0,W,le,pe);je.addColorStop(0,We+"b0"),je.addColorStop(.5,We+"65"),je.addColorStop(1,We+"00"),v.fillStyle=je,v.fillRect(0,0,1100,780)}o.invalidateImage?.(D)}function w(b,W,le){const pe=(Gt,Xt)=>{let kt=Math.imul(Gt,374761393)+Math.imul(Xt,668265263)+Math.imul(le,1442695041);return kt=Math.imul(kt^kt>>>13,1274126177),((kt^kt>>>16)>>>0)/4294967295},We=Math.floor(b),je=Math.floor(W),ze=b-We,ht=W-je,Je=ze*ze*(3-2*ze),ut=ht*ht*(3-2*ht),ot=pe(We,je),lt=pe(We+1,je),wt=pe(We,je+1),Ot=pe(We+1,je+1);return(ot+(lt-ot)*Je)*(1-ut)+(wt+(Ot-wt)*Je)*ut}function L(b,W){const le=document.createElement("canvas");le.width=320,le.height=240;const pe=le.getContext("2d"),We=pe.createImageData(le.width,le.height),je=We.data;for(let ze=0;ze<le.height;ze++)for(let ht=0;ht<le.width;ht++){const Je=ht/le.width,ut=ze/le.height,ot=w(Je*3,ut*3,W);let lt=0,wt=.55,Ot=3;for(let Pt=0;Pt<4;Pt++)lt+=w(Je*Ot+ot*.8,ut*Ot,W+Pt)*wt,wt*=.5,Ot*=2;const Gt=Math.exp(-Math.pow((ut-(.43+Math.sin(Je*4.4+W)*.14))/.23,2)),Xt=Math.sin(Math.PI*Je)*Math.sin(Math.PI*ut),kt=Math.max(0,lt-.34)*Gt*Xt,Ke=(ze*le.width+ht)*4;je[Ke]=b[0],je[Ke+1]=b[1],je[Ke+2]=b[2],je[Ke+3]=Math.min(130,Math.round(kt*220))}return pe.putImageData(We,0,0),le}const V=L([76,102,151],17),Z=L([111,91,136],53);let k=1,z=740,C=1,R=1,G=-1,Y=-1,ne="constellations",de=0,ce=-.055,J=0,ue=-.055,me=1,B=1,oe={x:0,y:0},$={x:0,y:0},se=0,_e=0,Me=0,ve=!0,F=null,K=0,ae=0,xe=0,Ae=0,be=0,ke="about",Xe=!0,Ze=!1,Ne=null,N=null,at=-1/0,$e=0,E="",f=0,U=0,O="trackpad",H=null,fe=-1/0;const he=.55,ee=22;let te={x:0,y:0,manual:!1},Se=[];const Ue=new Map,we=[],Te=new Map,He=(b,W,le)=>Math.max(W,Math.min(le,b)),Ge={x:0,y:0,tx:0,ty:0};let it=1,X=0,Ce=1,ge=0,Pe=1,Le=0,ye=1,Ve=0;const Ee=s(".sc-inspector");let Dt=null,gt=[],St=null,tn=!1,nn=null;function ms(){return{packet:Dt,vertices:u.map(b=>({id:b.id,slot:b.slot,p:b.p.slice(),sourcePosition:b.sourcePosition.slice(),volumeZ:b.volumeZ,pos:b.target.slice(),target:b.target.slice()})),selectedRelation:St}}function hr(b,W=u,{restore:le=!1}={}){const pe=u[G]?.id,We=new Map(u.map(ze=>[ze.id,ze]));if(u=eu(b,W).map(ze=>{const ht=We.get(ze.id);let Je=ht?.el,ut=ht?.label;if(!Je){Je=document.createElement("button"),Je.type="button",Je.className="sc-node",Je.dataset.id=ze.id,ut=document.createElement("span"),ut.className="sc-node-label",Je.append(ut);const ot=()=>_.get(ze.id);Je.addEventListener("click",lt=>{lt.detail<2&&ot()!==void 0&&Ie(ot())}),Je.addEventListener("dblclick",()=>{ot()!==void 0&&(xn(),ie(ot()))}),Je.addEventListener("pointerenter",()=>{Y=ot()??-1,bt()}),Je.addEventListener("pointerleave",()=>{Y=-1,bt()}),Je.addEventListener("focus",()=>{Y=ot()??-1,bt()}),Je.addEventListener("blur",()=>{Y=-1,bt()}),Je.addEventListener("keydown",lt=>{ot()!==void 0&&dn(lt,ot())})}return Je.dataset.main=String(ze.main),Je.setAttribute("aria-label",ze.fullName+" — "+ze.kind),ut.textContent=ze.name,{...ze,el:Je,label:ut,screen:pr(...ze.pos)}}),_.clear(),u.forEach((ze,ht)=>_.set(ze.id,ht)),x.replaceChildren(...u.map(ze=>ze.el)),Dt=b,gt=b.relations,d=gt.map(ze=>[_.get(ze.from_id),_.get(ze.to_id)]),ne!=="constellations"){const ze=le?new Set(W.map(ot=>ot.id)):null,ht=_.get(b.focus?.node_id)??0,Je=Wi(ht),ut=u.map((ot,lt)=>lt).filter(ot=>ot!==ht&&!Je.includes(ot));u.forEach((ot,lt)=>{if(!ze?.has(ot.id))if(ne==="plane")ot.target=[ot.sourcePosition[0],ot.sourcePosition[1],0];else if(lt===ht)ot.target=[0,0,0];else{const wt=Je.includes(lt)?Je:ut,Ot=wt.indexOf(lt)/wt.length*Math.PI*2-.8,Gt=wt===Je?175:380;ot.target=[Math.cos(Ot)*Gt,Math.sin(Ot)*Gt*.68,Math.sin(Ot*2)*140*c.depth]}})}G=_.get(pe)??-1,Y=-1,gt.some(ze=>ze.id===St)||(St=null),n.dataset.graphRevision=b.source_revision,n.dataset.graphFocus=b.focus?.node_id||"",n.dataset.nodeCount=String(u.length),n.dataset.relationCount=String(d.length),n.dataset.dataState="ready",Xi(),Q(),Pn(),Xe=!0,K=1,bt()}function qr(b){b?.packet&&(nn?.cancelPending(),hr(b.packet,b.vertices,{restore:!0}),St=b.selectedRelation)}function ya(b){if(!d.length)return!1;const W=n.getBoundingClientRect(),le=(b.clientX-W.left)*k/W.width,pe=(b.clientY-W.top)*z/W.height;let We=-1,je=7;for(let ze=0;ze<d.length;ze++){const[ht,Je]=d[ze],ut=u[ht].screen,ot=u[Je].screen;if(!ut.s||!ot.s)continue;const lt=ot.x-ut.x,wt=ot.y-ut.y,Ot=lt*lt+wt*wt,Gt=Ot?He(((le-ut.x)*lt+(pe-ut.y)*wt)/Ot,0,1):0;if(Gt<.04||Gt>.96)continue;const Xt=Math.hypot(le-ut.x-Gt*lt,pe-ut.y-Gt*wt);Xt<je&&(je=Xt,We=ze)}return We<0?!1:(Yr(gt[We].id),!0)}function Yr(b){const W=gt.find(le=>le.id===b);W&&(nn?.willSelect(),(St!==b||Ee.hidden)&&xn(),Re(!1,!1),Oe(!1,!1),St=b,G=_.get(W.from_id)??-1,Ee.hidden=!1,j(),nt(Ue.get(b)||"about"),Q(),Pn(),vn(!0),Xe=!0,bt(),Wn("Отношение: "+s(".sc-node-title").textContent),s("#so-about-tab").focus())}const Vi={get packet(){return Dt},get selection(){return{nodeId:u[G]?.id||null,relationId:St}},node:b=>u.find(W=>W.id===b)?.raw,relation:b=>gt.find(W=>W.id===b),neighbors:b=>gt.filter(W=>W.from_id===b||W.to_id===b),captureView:()=>_s(),capturePlace:()=>so({lens:ne,yaw:J,pitch:ue,zoom:B,pan:$,selectedId:u[G]?.id||null,relationId:St,panelOpen:!Ee.hidden,cardTab:ke,vertices:ms().vertices}),restorePlace(b){const W=so(b);Mt(),ne=W.lens,hr(Dt,W.vertices,{restore:!0}),G=_.get(W.selectedId)??-1,St=gt.some(le=>le.id===W.relationId)?W.relationId:null,J=W.yaw,ue=W.pitch,B=W.zoom,$={...W.pan},Ye(),G>=0||St?(j(),nt(W.cardTab,{preserveCamera:!0}),Ee.hidden=!W.panelOpen,vn(!1)):Ee.hidden=!0,Q(),Pn(),K=1,bt()},refreshTypography(){Xi(),Xe=!0,bt()},restoreView(b){b?.graph?.packet&&(xn(),q(b))},setGraph(b,{selectFocus:W=!1,initial:le=!1}={}){if(le||xn(),nn?.cancelInspector(),hr(b),W&&b.focus){tn=!0;try{Ie(_.get(b.focus.node_id))}finally{tn=!1}}else G>=0||St?j():(Ee.hidden=!0,Pn())},selectNode(b){const W=_.get(b);return W===void 0?!1:(Ie(W),!0)},selectRelation(b,{rememberView:W=!0}={}){const le=tn;tn=!W||le;try{Yr(b)}finally{tn=le}},cardChanged(){vn(!1),Xe=!0,bt()},announce:Wn};function Vn(){const b=k;k=n.clientWidth,z=n.clientHeight,C=Math.min(devicePixelRatio||1,1.5),a.width=Math.round(k*C),a.height=Math.round(z*C),R=k<=540?k/720:Math.min(k/1190,1.06),P.forEach(W=>{const le=W.minZ+W.range*.5,pe=(p-le)/p;W.x=W.sx*k*.5/R*pe,W.y=W.sy*z*.5/R*pe}),Xi(),vn(b!==k),G>=0&&!Ee.hidden&&ie(G,!1),Xe=!0,bt()}function pr(b,W,le,pe=!1){const We=pe?Pe:it,je=pe?Le:X,ze=pe?ye:Ce,ht=pe?Ve:ge,Je=b*We-le*je,ut=b*je+le*We,ot=W*ze-ut*ht,lt=W*ht+ut*ze;if(lt>p-110)return{x:-9999,y:-9999,s:0,depth:0,z:lt};const wt=p/(p-lt),Ot=R*(pe?1:me)*wt;return{x:k*.5+Je*Ot+(pe?oe.x*.18+Ge.x*wt*23:oe.x),y:z*.54+ot*Ot+(pe?oe.y*.18+Ge.y*wt*17:oe.y),s:Ot,depth:wt,z:lt}}function Wn(b){s(".sc-announcement").textContent=b}function li(b,W,le){b.style[W]!==String(le)&&(b.style[W]=le)}function Wi(b){return d.filter(W=>W.includes(b)).map(W=>W[0]===b?W[1]:W[0])}function Xi(){u.forEach(b=>{const W=getComputedStyle(b.label);o.font=W.font;const le=parseFloat(W.letterSpacing)||0;b.labelWidth=Math.ceil(o.measureText(b.name).width+le*b.name.length)+2,b.labelHeight=Math.ceil(parseFloat(W.fontSize)*1.5)})}function ci(b){const W=b.getBoundingClientRect(),le=n.getBoundingClientRect();return{x:W.left-le.left,y:W.top-le.top,w:W.width,h:W.height}}function Ri(b,W,le=0){return b.x<W.x+W.w+le&&b.x+b.w+le>W.x&&b.y<W.y+W.h+le&&b.y+b.h+le>W.y}function gs(){Se=[];for(const b of n.querySelectorAll(".sc-header,.sc-context,.sc-footer,.sc-label-west,.sc-label-east,.sc-panel"))if(!b.hidden){const W=ci(b);W.w&&W.h&&Se.push(W)}Ee.hidden||ci(Ee),Xe=!1}function vn(b=!1){if(!Ee.hidden){if(s(".sc-window-handle").disabled=k<=540,k<=540)for(const W of["left","right","top","bottom","transform"])Ee.style[W]="";else{const W=Ee.offsetWidth,le=Ee.offsetHeight;if(b&&!te.manual&&G>=0){const pe=u[G].screen;te.x=pe.x+W+60<k?pe.x+46:pe.x-W-46,te.y=pe.y-100}te.x=He(te.x,16,k-W-16),te.y=He(te.y,190,Math.max(190,z-le-98)),Ee.style.left="0",Ee.style.top="0",Ee.style.right="auto",Ee.style.bottom="auto",Ee.style.transform="translate3d("+Math.round(te.x)+"px,"+Math.round(te.y)+"px,0)"}Xe=!0,bt()}}function Pn(){s(".sc-back").hidden=we.length===0,s(".sc-context-sub").textContent=St?"Выбрана связь":G>=0?"Фокус: "+u[G].name:Dt?u.length+" звёзд · "+d.length+" связей":"Загружаем область…",n.dataset.history=String(we.length),Xe=!0,e(Vi)}function _s(){return{graph:ms(),selected:G,panelOpen:!Ee.hidden||Ze,lens:ne,targets:u.map(b=>b.target.slice()),yaw:J,pitch:ue,zoom:B,pan:{...$},windowPosition:{...te},cardTab:ke}}function xn(){if(tn)return;const b=_s();JSON.stringify(b)!==JSON.stringify(we.at(-1))&&(we.push(b),we.length>24&&we.shift()),Pn()}function y(){const b=we.pop();b&&q(b)}function q(b){Mt(),Re(!1,!1),Oe(!1,!1),qr(b.graph),G=b.selected,Y=-1,ne=b.lens,u.forEach((W,le)=>W.target=b.targets[le].slice()),J=b.yaw,ue=b.pitch,B=b.zoom,$={...b.pan},te={...b.windowPosition},Ye(),G>=0||St?(j(),nt(b.cardTab),Ee.hidden=!b.panelOpen,vn(!1)):Ee.hidden=!0,Q(),Pn(),Wn(G>=0?"Возврат: "+u[G].name:"Возврат к предыдущему виду"),we.length||s(".sc-overview").focus(),K=1,bt()}function ie(b,W=!0,{gentle:le=!1}={}){Mt();const pe=u[b].target,We=Math.cos(J),je=Math.sin(J),ze=Math.cos(ue),ht=Math.sin(ue),Je=pe[0]*We-pe[2]*je,ut=pe[0]*je+pe[2]*We,ot=pe[1]*ze-ut*ht,lt=pe[1]*ht+ut*ze;let wt=k*.5,Ot=z*.5;if(!Ee.hidden){const Xt=ci(Ee);if(k<=540){const kt=ci(s(".sc-context"));Ot=(kt.y+kt.h+35+Xt.y-38)*.5}else wt=Xt.x>k*.44?Math.max(k*.3,Xt.x*.55):Math.min(k*.76,(Xt.x+Xt.w+k)*.5),Ot=He(Xt.y+Xt.h*.44,250,z-150)}B=le?He(B*1.08,.65,2.2):W?Math.max(k<=540?1.25:1.45,B):k<=540?1.12:Math.max(1.1,B),B=Math.min(2.2,B);const Gt=R*B*p/(p-lt);$.x=wt-k*.5-Je*Gt,$.y=Ot-z*.54-ot*Gt,K=1,bt()}function Q(){n.dataset.selected=G>=0?u[G].id:"",u.forEach((b,W)=>b.el.setAttribute("aria-pressed",String(W===G)))}function j(){const b=St?Vi.relation(St):u[G]?.raw;b&&nn?.showCard(St?"relation":"node",b)}function Ie(b,{fly:W=!1,keepWindow:le=!1}={}){nn?.willSelect();const pe=G!==b||!!St;(pe||Ee.hidden)&&xn(),St=null;const We=!Ee.hidden;Re(!1,!1),Oe(!1,!1),G=b,j(),nt(Ue.get(u[b].id)||"about"),Ee.hidden=!1,nn?.restoreReading(),Q(),Pn(),vn(!le&&(pe||!We)),W?ie(b,!0):pe||!We?ie(b,!1,{gentle:!0}):k<=540&&ie(b,!1),Wn(u[b].kind+": "+u[b].name),Xe=!0,bt()}function Fe(b=!0,W=!1){nn?.captureReading(),nn?.cancelInspector(),Ee.hidden=!0,Ze=!1,b&&G>=0&&!u[G].el.hidden&&u[G].el.focus(),W&&(St=null,G=-1,Y=-1,Q(),Pn()),Xe=!0,bt()}function Re(b=!0,W=!0){nn?.cancelSearch();const le=!s(".sc-search").hidden;s(".sc-search").hidden=!0,s(".sc-search-open").setAttribute("aria-expanded","false"),le&&(W&&Ze&&G>=0&&(Ee.hidden=!1,vn(!1)),Ze=!1),b&&s(".sc-search-open").focus(),Xe=!0,bt()}function Oe(b=!0,W=!0){const le=!s(".sc-lenses").hidden;s(".sc-lenses").hidden=!0,s(".sc-lenses-open").setAttribute("aria-expanded","false"),le&&(W&&Ze&&G>=0&&(Ee.hidden=!1,vn(!1)),Ze=!1),b&&s(".sc-lenses-open").focus(),Xe=!0,bt()}function qe(b){const W=s(".sc-"+b);if(!W.hidden){b==="search"?Re():Oe();return}const le=!Ee.hidden||Ze;Re(!1,!1),Oe(!1,!1),Ze=le,Ee.hidden=!0,W.hidden=!1,s(".sc-"+b+"-open").setAttribute("aria-expanded","true"),b==="search"?(rt(),s("#sc-query").focus()):s('.sc-lens[aria-pressed="true"]').focus(),Xe=!0,bt()}function rt(){nn?.search(s("#sc-query").value)}function nt(b,{preserveCamera:W=!1}={}){nn?.captureReading(),ke=b;const le=St||u[G]?.id;le&&(Ue.delete(le),Ue.set(le,b),Ue.size>64&&Ue.delete(Ue.keys().next().value)),n.querySelectorAll(".sc-card-tab").forEach(pe=>{const We=pe.id==="so-"+b+"-tab";pe.setAttribute("aria-selected",String(We)),pe.tabIndex=We?0:-1}),s("#so-about").hidden=b!=="about",s("#so-relations").hidden=b!=="relations",nn?.restoreReading(),vn(!1),!W&&k<=540&&G>=0&&!Ee.hidden&&ie(G,!1),Xe=!0,bt()}function Ye(){n.dataset.lens=ne,s(".sc-context h2").textContent=ne==="orbits"?"Орбиты мысли":ne==="plane"?"Карта связей":"Созвездия мысли",s(".sc-label-west").hidden=!0,s(".sc-label-east").hidden=!0,n.querySelectorAll(".sc-lens").forEach(b=>b.setAttribute("aria-pressed",String(b.dataset.lens===ne))),Xe=!0}function At(b){if(b===ne){Oe();return}Mt(),xn(),ne=b;const W=G>=0?G:0,le=Wi(W),pe=u.map((We,je)=>je).filter(We=>We!==W&&!le.includes(We));u.forEach((We,je)=>{if(b==="constellations")We.target=We.p.slice();else if(b==="plane")We.target=[We.sourcePosition[0],We.sourcePosition[1],0];else if(je===W)We.target=[0,0,0];else{const ze=le.includes(je)?le:pe,ht=ze.indexOf(je)/ze.length*Math.PI*2-.8,Je=ze===le?175:380;We.target=[Math.cos(ht)*Je,Math.sin(ht)*Je*.68,Math.sin(ht*2)*140*c.depth]}}),Ye(),J=0,ue=b==="plane"?0:-.055,$={x:0,y:0},B=1,Oe(),G>=0&&!Ee.hidden&&ie(G,!1),Wn("Линза: "+s(".sc-context h2").textContent),K=1,bt()}function Bt(){Mt(),xn(),Re(!1,!1),Oe(!1,!1),J=0,ue=ne==="plane"?0:-.055,$={x:0,y:0},B=1,K=1,Fe(!1,!0),te.manual=!1,Wn("Общий вид"),bt()}function Mt(){f=0,at=-1/0,$e=0,E="",H=null,fe=-1/0}function Rt(){const b=O==="trackpad",W=s(".sc-input-mode");n.dataset.inputMode=O,W.setAttribute("aria-label",b?"Управление: тачпад. Переключить на мышь":"Управление: мышь. Переключить на тачпад"),W.setAttribute("data-tooltip",b?"Тачпад · два пальца — сдвиг, щипок — полёт":"Мышь · колесо — масштаб, перетаскивание — вращение"),W.innerHTML=b?'<i data-lucide="touchpad" aria-hidden="true"></i>':'<i data-lucide="mouse" aria-hidden="true"></i>',s(".sc-gesture").textContent=b?"Два пальца — сдвиг · щипок — полёт":"Колесо — масштаб · Shift + перетаскивание — сдвиг",ki(),Xe=!0}function zt(b,W=k*.5,le=z*.54){b=He(b,.65,2.2);const pe=b/B;$.x=W-k*.5-(W-k*.5-$.x)*pe,$.y=le-z*.54-(le-z*.54-$.y)*pe,B=b,K=1,bt()}function Be(){n.dataset.motion=c.playing?"running":"paused",s(".sc-motion").setAttribute("aria-pressed",String(!c.playing)),s(".sc-motion").setAttribute("aria-label",c.playing?"Приостановить движение":"Включить движение"),s(".sc-motion").innerHTML=c.playing?'<i data-lucide="pause" aria-hidden="true"></i>':'<i data-lucide="play" aria-hidden="true"></i>',ki(),bt()}function dn(b,W){const le={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[b.key];if(!le)return;b.preventDefault();const pe=u[W].screen;let We=-1,je=1/0;u.forEach((ze,ht)=>{if(ht===W||ze.el.hidden)return;const Je=ze.screen.x-pe.x,ut=ze.screen.y-pe.y,ot=Je*le[0]+ut*le[1];if(ot<=0)return;const lt=Je*Je+ut*ut,wt=lt/Math.max(1,ot)+Math.abs(Je*le[1]-ut*le[0])*1.5;wt<je&&(je=wt,We=ht)}),We>=0&&u[We].el.focus()}s(".sc-back").addEventListener("click",y),s(".sc-close").addEventListener("click",()=>Fe()),s(".sc-focus").addEventListener("click",()=>{G>=0&&(xn(),ie(G))}),s(".sc-search-open").addEventListener("click",()=>qe("search")),s(".sc-search-close").addEventListener("click",()=>Re()),s("#sc-query").addEventListener("input",rt),s(".sc-lenses-open").addEventListener("click",()=>qe("lenses")),s(".sc-lenses-close").addEventListener("click",()=>Oe()),n.querySelectorAll(".sc-lens").forEach(b=>b.addEventListener("click",()=>At(b.dataset.lens))),s(".sc-overview").addEventListener("click",Bt),s(".sc-plus").addEventListener("click",()=>{Mt(),xn(),zt(B*1.18)}),s(".sc-minus").addEventListener("click",()=>{Mt(),xn(),zt(B/1.18)}),s(".sc-motion").addEventListener("click",()=>{c.playing=!c.playing,Be()}),s(".sc-input-mode").addEventListener("click",()=>{Mt(),O=O==="trackpad"?"mouse":"trackpad",Rt(),Wn(O==="trackpad"?"Тачпад: два пальца — перемещение, щипок — приближение":"Мышь: колесо — масштаб, перетаскивание — вращение"),bt()}),n.querySelectorAll(".sc-card-tab").forEach((b,W)=>{b.addEventListener("click",()=>nt(W?"relations":"about")),b.addEventListener("keydown",le=>{if(["ArrowLeft","ArrowRight","Home","End"].includes(le.key)){le.preventDefault();const pe=le.key==="Home"?"about":le.key==="End"||ke==="about"?"relations":"about";nt(pe),s("#so-"+pe+"-tab").focus()}})}),s(".sc-search").addEventListener("keydown",b=>{const W=[...n.querySelectorAll(".sc-result")],le=W.indexOf(b.target);if(b.target===s("#sc-query")&&b.key==="Enter"&&W[0])b.preventDefault(),W[0].click();else if(b.key==="ArrowDown"||b.key==="ArrowUp"){b.preventDefault();const pe=le+(b.key==="ArrowDown"?1:-1);pe<0?s("#sc-query").focus():W[Math.min(pe,W.length-1)]?.focus()}}),n.addEventListener("keydown",b=>{b.key==="Escape"&&(s(".sc-search").hidden?s(".sc-lenses").hidden?Ee.hidden||Fe():Oe():Re(),b.stopPropagation()),b.key==="/"&&!b.target.closest("input,textarea,select,[contenteditable]")&&(b.preventDefault(),qe("search"))});const dt=s(".sc-window-handle");dt.addEventListener("pointerdown",b=>{k<=540||b.button!==0||(Ne={id:b.pointerId,x:b.clientX,y:b.clientY,origin:{...te}},dt.setPointerCapture(b.pointerId),b.preventDefault())}),dt.addEventListener("pointermove",b=>{!Ne||b.pointerId!==Ne.id||(te={x:Ne.origin.x+b.clientX-Ne.x,y:Ne.origin.y+b.clientY-Ne.y,manual:!0},vn(!1))});const un=b=>{Ne?.id===b.pointerId&&(Ne=null,dt.hasPointerCapture(b.pointerId)&&dt.releasePointerCapture(b.pointerId))};dt.addEventListener("pointerup",un),dt.addEventListener("pointercancel",un),dt.addEventListener("lostpointercapture",()=>Ne=null),dt.addEventListener("keydown",b=>{if(k<=540)return;const W={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[b.key];if(!W)return;b.preventDefault();const le=b.shiftKey?40:16;te.x+=W[0]*le,te.y+=W[1]*le,te.manual=!0,vn(!1)}),a.addEventListener("pointerdown",b=>{if(b.button===0){if(Te.set(b.pointerId,{x:b.clientX,y:b.clientY}),a.setPointerCapture(b.pointerId),Te.size===1)Mt(),F={id:b.pointerId,x:b.clientX,y:b.clientY,yaw:J,pitch:ue,pan:{...$},shift:b.shiftKey,moved:!1};else if(Te.size===2){const[W,le]=[...Te.values()],pe=n.getBoundingClientRect();B=me,$={...oe},N={distance:Math.hypot(le.x-W.x,le.y-W.y),x:((W.x+le.x)*.5-pe.left)*k/pe.width,y:((W.y+le.y)*.5-pe.top)*z/pe.height},F&&(F.moved=!0)}}}),a.addEventListener("pointermove",b=>{if(Te.has(b.pointerId)){if(Te.set(b.pointerId,{x:b.clientX,y:b.clientY}),N&&Te.size>=2){const[W,le]=[...Te.values()],pe=n.getBoundingClientRect(),We=Math.hypot(le.x-W.x,le.y-W.y),je=((W.x+le.x)*.5-pe.left)*k/pe.width,ze=((W.y+le.y)*.5-pe.top)*z/pe.height;$.x+=je-N.x,$.y+=ze-N.y,zt(B*We/Math.max(1,N.distance),je,ze),N={distance:We,x:je,y:ze},E="pinch",f=performance.now()+180,Ft()}else if(F&&b.pointerId===F.id){const W=b.clientX-F.x,le=b.clientY-F.y;if(F.moved=F.moved||Math.abs(W)+Math.abs(le)>4,F.shift){const pe=n.getBoundingClientRect();$.x=F.pan.x+W*k/pe.width*he,$.y=F.pan.y+le*z/pe.height*he,E="pan",f=performance.now()+180,Ft()}else J=F.yaw+W*.0022,ue=He(F.pitch+le*.0018,-.7,.7)}K=1,bt()}});function pn(b){if(!Te.has(b.pointerId))return;const W=F?.moved||N||b.type!=="pointerup";if(Te.delete(b.pointerId),N=null,Te.size){const[le,pe]=[...Te.entries()][0];F={id:le,x:pe.x,y:pe.y,yaw:J,pitch:ue,pan:{...$},shift:!1,moved:!0}}else F=null,!W&&!ya(b)&&(G>=0&&xn(),Re(!1,!1),Oe(!1,!1),Fe(!1,!0));a.hasPointerCapture(b.pointerId)&&a.releasePointerCapture(b.pointerId)}a.addEventListener("pointerup",pn),a.addEventListener("pointercancel",pn),a.addEventListener("lostpointercapture",pn);function Ln(b){const W=b.target instanceof Element?b.target:b.target.parentElement;return!!(W&&!W.closest('.sc-panel, .sc-navigation, button:not(.sc-node), input, textarea, select, a, [contenteditable="true"]'))}function Xn(b,W={x:k*.5,y:z*.54}){const le=n.getBoundingClientRect();return{x:Number.isFinite(b.clientX)?(b.clientX-le.left)*k/le.width:W.x,y:Number.isFinite(b.clientY)?(b.clientY-le.top)*z/le.height:W.y}}function Ct(b,W){(W-at>220||b!==E)&&(B=me,$={...oe}),at=W,E=b,f=W+180}function Ft(){n.dataset.wheelKind=E,n.dataset.zoomTarget=B.toFixed(6),n.dataset.panTarget=[$.x.toFixed(3),$.y.toFixed(3)].join(",")}function qn(b){if(!Ln(b))return;b.cancelable&&b.preventDefault(),b.stopPropagation(),n.dataset.wheelEvents=String(++U);const W=b.deltaMode,le=b.deltaX,pe=b.deltaY,We=performance.now();if(!Number.isFinite(le)||!Number.isFinite(pe)||H||b.ctrlKey&&We<fe||le===0&&pe===0)return;const je=b.ctrlKey?"pinch":O==="trackpad"?"pan":"wheel",ze=W===1?32:W===2?z:1,ht=Math.sign(-pe);if(je==="wheel"&&E===je&&ht!==$e&&(B=me,$={...oe}),Ct(je,We),$e=ht,je==="pan"){const Je=n.getBoundingClientRect();$.x-=le*ze*k/Je.width*he,$.y-=pe*ze*z/Je.height*he,K=1,bt()}else{const Je=je==="pinch"?He(-pe*ze*.006,-Math.log(4),Math.log(4)):He(-pe*ze*.0016,-.18,.18),ut=Xn(b);zt(B*Math.exp(Je),ut.x,ut.y)}Ft()}n.addEventListener("wheel",qn,{passive:!1,capture:!0});function Ut(b){if(!Ln(b)||!Number.isFinite(b.scale)||b.scale<=0)return;b.cancelable&&b.preventDefault(),b.stopPropagation(),B=me,$={...oe};const W=Xn(b);H={scale:b.scale,...W},Ct("pinch",performance.now())}function Yn(b){if(!H||!Number.isFinite(b.scale)||b.scale<=0)return;b.cancelable&&b.preventDefault(),b.stopPropagation();const W=Xn(b,H);$.x+=W.x-H.x,$.y+=W.y-H.y,zt(B*Math.pow(b.scale/H.scale,.6),W.x,W.y),H={scale:b.scale,...W},at=performance.now(),E="pinch",f=at+180,Ft()}function Ci(b){H&&(b.cancelable&&b.preventDefault(),b.stopPropagation(),H=null,fe=performance.now()+80)}n.addEventListener("gesturestart",Ut,{passive:!1,capture:!0}),n.addEventListener("gesturechange",Yn,{passive:!1,capture:!0}),n.addEventListener("gestureend",Ci,{passive:!1,capture:!0}),window.addEventListener("blur",()=>{H=null,fe=-1/0,Te.clear(),F=null,N=null,Mt()}),n.addEventListener("pointermove",b=>{if(b.pointerType!=="mouse"||!c.playing)return;const W=n.getBoundingClientRect();Ge.tx=He(((b.clientX-W.left)/k-.5)*2,-1,1),Ge.ty=He(((b.clientY-W.top)/z-.5)*2,-1,1),bt()}),n.addEventListener("pointerleave",()=>{Ge.tx=0,Ge.ty=0,bt()});function Kr(b,W,le){o.save(),o.globalCompositeOperation="screen",o.globalAlpha=le,o.translate(k*.5+Math.sin(se*.025+W)*k*.026+Ge.x*16-de*27,z*.51+Math.cos(se*.019+W)*z*.024+Ge.y*12),o.rotate(Math.sin(se*.013+W)*.055-de*.035),o.drawImage(b,-k*.72,-z*.68,k*1.44,z*1.36),o.restore()}function Sa(b,W){for(let le=0;le<W;le++){const pe=P[le];if(pe.layer!==b)continue;const We=(pe.offset+se/pe.cycle)%1,je=pe.minZ+We*pe.range,ze=A(0,.065,We)*(1-A(.9,1,We));if(ze<.002)continue;const ht=Math.sin(se*.035+pe.phase)*pe.drift,Je=pe.x+ht,ut=pe.y+Math.cos(se*.029+pe.phase)*pe.drift*.65,ot=Je*Pe-je*Le,lt=Je*Le+je*Pe,wt=ut*ye-lt*Ve,Ot=ut*Ve+lt*ye;if(Ot>p-130)continue;const Gt=p/(p-Ot),Xt=R*Gt,kt=k*.5+ot*Xt+oe.x*.18+Ge.x*Gt*23,Ke=z*.54+wt*Xt+oe.y*.18+Ge.y*Gt*17;if(kt<-30||Ke<-30||kt>k+30||Ke>z+30)continue;const Pt=Math.sin(se*pe.freq+pe.phase)*.26+Math.sin(se*pe.freq*.43+pe.phase*2.13)*.16,ft=pe.flare?Math.pow(Math.max(0,Math.sin(se*.23+pe.phase*1.7)),28):0,Ht=Math.min(.92,pe.alpha*ze*(.66+Pt+ft*.8))*(1-be*.16),qt=Math.min(b===2?3.3:2.2,Math.max(.38,pe.size*Gt*(.64+R*.32))),mn=M[pe.tone];if(o.fillStyle=mn,o.globalAlpha=Ht,b===2){const $t=17+qt*8;o.drawImage(T(mn),kt-$t/2,Ke-$t/2,$t,$t),o.globalAlpha=Ht*.7,o.beginPath(),o.arc(kt,Ke,qt*.62,0,Math.PI*2),o.fill()}else if(o.fillRect(kt-qt/2,Ke-qt/2,qt,qt),pe.flare||qt>1.5){const $t=12+qt*5+ft*17;o.globalAlpha=Ht*.6,o.drawImage(T(mn),kt-$t/2,Ke-$t/2,$t,$t)}if(pe.flare&&ft>.25&&b!==0){o.globalAlpha=Ht*ft*.52;const $t=3+ft*7;o.lineWidth=.6,o.strokeStyle=mn,o.beginPath(),o.moveTo(kt-$t,Ke),o.lineTo(kt+$t,Ke),o.moveTo(kt,Ke-$t),o.lineTo(kt,Ke+$t),o.stroke()}le===25&&(n.dataset.starProbe=[kt.toFixed(2),Ke.toFixed(2),Ht.toFixed(3)].join(","))}o.globalAlpha=1}function Rl(b){if(Me=0,!n.isConnected||!ve||document.hidden){_e=0;return}const W=performance.now();o.beginFrame?.();const le=_e?(b-_e)/1e3:1/60,pe=Math.min(le,.05);_e=b;const We=l.matches?1:1-Math.exp(-pe*14),je=l.matches?1:b<f?1-Math.exp(-pe*(E==="pan"?ee:30)):We,ze=Ee.hidden?0:1;if(be+=(ze-be)*(l.matches?1:1-Math.exp(-pe*3)),c.playing){se+=pe*c.liveliness*(1-be*.65);const Ke=1-Math.exp(-pe*4);Ge.x+=(Ge.tx-Ge.x)*Ke,Ge.y+=(Ge.ty-Ge.y)*Ke}de+=(J-de)*We,ce+=(ue-ce)*We,me+=(B-me)*je,oe.x+=($.x-oe.x)*je,oe.y+=($.y-oe.y)*je;let ht=Math.abs(J-de)+Math.abs(ue-ce)+Math.abs(B-me)+Math.abs(oe.x-$.x)*.01+Math.abs(oe.y-$.y)*.01+Math.abs(be-ze);Xe&&gs(),it=Math.cos(de),X=Math.sin(de),Ce=Math.cos(ce),ge=Math.sin(ce);const Je=de+Math.sin(se*.021)*.011,ut=ce+Math.cos(se*.017)*.008;Pe=Math.cos(Je),Le=Math.sin(Je),ye=Math.cos(ut),Ve=Math.sin(ut),o.setTransform(C,0,0,C,0,0),o.globalAlpha=1,o.globalCompositeOperation="source-over",o.drawImage(D,0,0,k,z);const ot=Math.round(P.length*c.density);Sa(0,ot),Kr(V,.2,.85),Sa(1,ot),Kr(Z,2.4,.46),u.forEach(Ke=>{for(let Pt=0;Pt<3;Pt++)Ke.pos[Pt]+=(Ke.target[Pt]-Ke.pos[Pt])*We,ht+=Math.abs(Ke.target[Pt]-Ke.pos[Pt])*.001;Ke.screen=pr(...Ke.pos)});const lt=Y>=0?Y:G,wt=new Set(lt<0?[]:d.filter(Ke=>Ke.includes(lt)).flat());o.lineWidth=.7,d.forEach(([Ke,Pt],ft)=>{const Ht=u[Ke].screen,qt=u[Pt].screen;if(!Ht.s||!qt.s)return;const mn=St?gt[ft].id===St:lt>=0&&(Ke===lt||Pt===lt),$t=u[Ke].group===u[Pt].group,$r=mn?"#e8c88d":$t?h[u[Ke].group]:"#8ea8c3",mr=mn?.76:lt>=0?.07:$t?.39:.17,vs=Math.max(.18,Math.min(1,Ht.depth*.75)),gr=Math.max(.18,Math.min(1,qt.depth*.75)),Pi=o.createLinearGradient(Ht.x,Ht.y,qt.x,qt.y);Pi.addColorStop(0,$r+Math.round(mr*vs*255).toString(16).padStart(2,"0")),Pi.addColorStop(1,$r+Math.round(mr*gr*255).toString(16).padStart(2,"0")),o.strokeStyle=Pi,o.lineWidth=(mn?1:.7)*Math.max(.6,Math.min(1.35,(Ht.depth+qt.depth)/2)),o.beginPath(),o.moveTo(Ht.x,Ht.y),o.lineTo(qt.x,qt.y),o.stroke()}),o.globalAlpha=1;const Ot=[],Gt=[];let Xt=0;const kt=u.map((Ke,Pt)=>({n:Ke,i:Pt})).sort((Ke,Pt)=>(Pt.i===lt?20:wt.has(Pt.i)?8:Pt.n.main?3:0)-(Ke.i===lt?20:wt.has(Ke.i)?8:Ke.n.main?3:0));for(const{n:Ke,i:Pt}of kt){const ft=Ke.screen,Ht=Pt===lt,qt=wt.has(Pt),mn=Ke.main,$t=Math.max(.52,Math.min(1,ft.depth*.86)),$r=(lt<0||Ht||qt?1:.36)*$t,mr=h[Ke.group],vs=(Ht?5:Pt===0?4.7:mn?3.1:1.9)*Math.max(.55,Math.min(ft.s,1.45));o.globalAlpha=$r;const gr=(Ht?115:Pt===0?145:mn?90:42)*Math.max(.55,ft.s);o.drawImage(T(mr),ft.x-gr/2,ft.y-gr/2,gr,gr),o.fillStyle=mr,o.beginPath(),o.arc(ft.x,ft.y,vs,0,Math.PI*2),o.fill(),o.fillStyle="#fff9ea";const Pi=Math.max(1,vs*.45);if(o.fillRect(Math.round(ft.x-Pi/2),Math.round(ft.y-Pi/2),Pi,Pi),mn||Ht){const Qt=(Ht?18:Pt===0?16:10)*Math.max(.7,ft.s);o.globalAlpha=$r*(.45+c.pixel*.45),o.strokeStyle=mr,o.lineWidth=.65,o.beginPath(),o.moveTo(ft.x-Qt,ft.y),o.lineTo(ft.x+Qt,ft.y),o.moveTo(ft.x,ft.y-Qt),o.lineTo(ft.x,ft.y+Qt),o.stroke()}if(Ht){o.globalAlpha=.85,o.strokeStyle="#d8bc84",o.lineWidth=1;const Qt=20,Zr=5;o.beginPath();for(const[vr,xr]of[[-1,-1],[-1,1],[1,-1],[1,1]])o.moveTo(ft.x+vr*Qt,ft.y+xr*(Qt-Zr)),o.lineTo(ft.x+vr*Qt,ft.y+xr*Qt),o.lineTo(ft.x+vr*(Qt-Zr),ft.y+xr*Qt);o.stroke()}o.globalAlpha=1;const xs=k<=540?44:38,yn={x:ft.x-xs/2,y:ft.y-xs/2,w:xs,h:xs};li(Ke.el,"transform","translate3d("+Math.round(yn.x)+"px,"+Math.round(yn.y)+"px,0)"),li(Ke.el,"opacity",lt<0||Ht||qt?"1":".46");const Hd=ft.s>0&&yn.x>10&&yn.x+yn.w<k-10&&yn.y>92&&yn.y+yn.h<z-90,Vd=Se.some(Qt=>Ri(yn,Qt,2)),Wd=Gt.some(Qt=>Ri(yn,Qt,0));Ke.el.hidden=!Hd||Vd||Wd,Ke.el.hidden||Gt.push(yn);const Xd=Ht||qt||lt<0&&(mn||me>(k<=540?1.5:1.28))||mn&&me>1.8;let _r=null;const qi=Ke.labelWidth,Yi=Ke.labelHeight;if(!Ke.el.hidden&&Xd){const Qt={x:ft.x-qi/2,y:ft.y+22,w:qi,h:Yi},Zr={x:ft.x-qi/2,y:ft.y-23-Yi,w:qi,h:Yi},vr={x:ft.x+28,y:ft.y-Yi/2,w:qi,h:Yi},xr={x:ft.x-28-qi,y:ft.y-Yi/2,w:qi,h:Yi};_r=(Ke.above?[Zr,Qt,vr,xr]:[Qt,Zr,vr,xr]).find(di=>di.x>12&&di.x+di.w<k-12&&di.y>90&&di.y+di.h<z-86&&!Se.some(Ki=>Ri(di,Ki,7))&&!Ot.some(Ki=>Ri(di,Ki,7))&&!u.some((Ki,qd)=>qd!==Pt&&Ki.screen.s>0&&Ri(di,{x:Ki.screen.x-8,y:Ki.screen.y-8,w:16,h:16},3)))}li(Ke.label,"visibility",_r?"visible":"hidden"),_r&&(li(Ke.label,"left",Math.round(_r.x-yn.x)+"px"),li(Ke.label,"top",Math.round(_r.y-yn.y)+"px"),li(Ke.label,"transform","none"),Ot.push(_r),Xt++)}Sa(2,ot),o.globalAlpha=1,o.endFrame?.(),ae++,xe+=performance.now()-W,Ae+=le*1e3,ae%90===0&&(n.dataset.drawMs=(xe/90).toFixed(2),n.dataset.frameMs=(Ae/90).toFixed(2),n.dataset.stars=String(ot),n.dataset.frames=String(ae),xe=0,Ae=0),n.dataset.skyClock=se.toFixed(3),n.dataset.rotation=de.toFixed(3),n.dataset.zoom=me.toFixed(3),n.dataset.labels=String(Xt),n.dataset.calm=be.toFixed(2),n.dataset.camera=[de.toFixed(3),ce.toFixed(3),me.toFixed(3),oe.x.toFixed(1),oe.y.toFixed(1)].join(","),K=ht>(b<f?1e-5:.005)?1:0,c.playing||K||F?Me=requestAnimationFrame(Rl):_e=0}a.addEventListener("sophia-renderer-restored",()=>{K=1,bt()});function bt(){!Me&&ve&&!document.hidden&&(Me=requestAnimationFrame(Rl))}return document.addEventListener("visibilitychange",()=>{document.hidden?(cancelAnimationFrame(Me),Me=0,_e=0):bt()}),new IntersectionObserver(b=>{ve=b[0].isIntersecting,ve?bt():(cancelAnimationFrame(Me),Me=0,_e=0)}).observe(n),new ResizeObserver(Vn).observe(n),l.addEventListener("change",b=>{c.playing=!b.matches,Be()}),nn=Uu(n,Vi,{initialFocus:t,initialLens:i}),S(),Vn(),Be(),Rt(),n.dataset.lens=ne,Pn(),r&&nn.start(),document.fonts?.ready.then(()=>{Xi(),bt()}),ki(),{port:Vi,ui:nn,openSearch:()=>qe("search"),overview:Bt,closeInspector:Fe,invalidate:()=>{Xe=!0,bt()}}}function j_(n,e){const t={id:e.parentHypothesisId,title:e.statement.slice(0,100),body:e.statement,targetId:e.targetId,fromId:e.fromId,toId:e.toId},i=Uc({persistence:!1});i.importPacket(n.exportPacket()),i.addHypothesis(t);const r={...e,createdAt:e.createdAt??new Date().toISOString(),baseWorkspaceRevision:i.summary().revision};return i.stageProposal(r),n.addHypothesis(t),n.stageProposal(r)}const Qe=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},An=(n,e)=>{const t=Qe("button",n);return t.type="button",t.addEventListener("click",e),t};function Ws(n){if(/^https?:\/\//i.test(n))try{const e=new URL(n),t=Qe("a",e.hostname+e.pathname,"sc-source-ref");return t.href=e.href,t.target="_blank",t.rel="noreferrer noopener",t}catch{}return Qe("span",n,"sc-source-ref")}function e0(n,e,{selected:t,panels:i,onChange:r}){let s=!1;try{s=Yd(localStorage,"tos-research-workspace-v1")}catch{}const a=Uc({sessionId:"tos-local-research",persistence:s}),o=new Hr,l=new Map;let c=null;const p=jo(async(F,K={})=>{const ae=AbortSignal.any([K.signal||new AbortController().signal,AbortSignal.timeout(6e4)]),xe=await fetch(F,{...K,signal:ae});if(!xe.ok)throw new Error(xe.status===404?"Для этого объекта отдельное досье пока не подготовлено.":"Не удалось загрузить материал. Попробуйте ещё раз.");return xe.json()}),u=An("",()=>Z("notes"));u.className="sc-control sc-workspace-open",u.setAttribute("aria-label","Исследование"),u.setAttribute("aria-expanded","false"),u.innerHTML='<i data-lucide="notebook-pen" aria-hidden="true"></i><span>Исследование</span>',n.querySelector(".sc-header-actions").append(u);const d=Qe("section","","sc-panel sc-workspace");d.hidden=!0,d.setAttribute("aria-label","Исследовательская панель"),d.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-workspace-close" aria-label="Закрыть исследование"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Исследование</h3><div class="sc-workspace-tabs" role="tablist" aria-label="Инструменты исследования"></div><div class="sc-workspace-body" role="tabpanel" id="sc-tool-content"></div><div class="sc-tool-status" role="status"></div><div class="sc-workspace-footer"></div>',n.append(d),i.register("workspace",d,()=>{m.capture(),o.cancelAll(),u.setAttribute("aria-expanded","false")});const h=d.querySelector(".sc-workspace-body"),_=d.querySelector(".sc-tool-status"),x=d.querySelector(".sc-workspace-tabs"),g=d.querySelector(".sc-workspace-footer"),m=as(h);let A="notes",P=null,M=null,I="node",T=u,D="",v="note",S=null;const w={notes:"Записи",sources:"Источники",analysis:"Разбор"},L=Object.entries(w).map(([F,K])=>{const ae=An(K,()=>k(F));return ae.id="sc-tool-"+F,ae.setAttribute("role","tab"),ae.setAttribute("aria-controls","sc-tool-content"),x.append(ae),ae});x.addEventListener("keydown",F=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(F.key))return;F.preventDefault();const K=L.indexOf(F.target),ae=F.key==="Home"?0:F.key==="End"?2:(K+(F.key==="ArrowLeft"?2:1))%3;k(Object.keys(w)[ae]),L[ae].focus()});function V(){i.close("workspace"),(T?.isConnected&&!T.closest("[hidden]")?T:u).focus()}d.querySelector(".sc-workspace-close").addEventListener("click",V),d.addEventListener("keydown",F=>{F.key==="Escape"&&(F.preventDefault(),F.stopPropagation(),V())});function Z(F="notes",K){m.capture(),T=document.activeElement instanceof HTMLElement?document.activeElement:u,P=K?{id:K.raw.id,label:vt(K.raw.display.title||K.raw.display.label),kind:K.kind==="relation"?"edge":"node",source_refs:K.raw.source_refs}:t(),M=K?.raw||(P?.kind==="edge"?e.port.relation(P.id):e.port.node(P?.id)),I=K?.kind||(P?.kind==="edge"?"relation":"node"),i.open("workspace"),u.setAttribute("aria-expanded","true"),k(F),L[Object.keys(w).indexOf(F)].focus()}function k(F){m.capture(),o.cancelAll(),h.setAttribute("aria-busy","false"),A=F,m.enter(JSON.stringify([e.port.packet?.source_revision,P?.id,F])),d.querySelector("h3").textContent={notes:"Исследование",sources:"Источники",analysis:"Разбор текста"}[F],_.textContent="";for(const[K,ae]of L.entries()){const xe=Object.keys(w)[K]===F;ae.setAttribute("aria-selected",String(xe)),ae.tabIndex=xe?0:-1}h.setAttribute("aria-labelledby","sc-tool-"+F),h.replaceChildren(),F==="notes"?ue():F==="sources"?B():oe(),m.restore(),e.invalidate()}i.configure("workspace",{onResume:()=>{m.restore(),k(A)}}),n.addEventListener("sophia-sources",F=>Z("sources",F.detail));function z(F){F?.name!=="AbortError"&&(_.textContent=F.message||"Не удалось выполнить действие.",e.invalidate())}function C(F){try{return F()}catch(K){z(K)}}function R(...F){const K=Qe("div","","sc-tool-actions");return K.append(...F),K}function G(){h.append(Qe("p",P?"Для: "+P.label:"Общие записи исследования","sc-muted"))}function Y(F){return`${F}:${crypto.randomUUID()}`}function ne(F,K){return a.addNote({id:Y("note"),body:F,targetId:K}),{added:!0,summary:a.summary()}}function de(F,K=P){return a.addHypothesis({id:Y("hypothesis"),title:F.slice(0,100),body:F,targetId:K?.id,...K?.from_id&&K?.to_id?{fromId:K.from_id,toId:K.to_id}:{}})}function ce(F,K=P,ae={}){if(!K?.id)throw new Error("Сначала выберите звезду или отношение.");const xe=ae.source_refs||K.source_refs||[];if(!xe.length)throw new Error("Для предложения нужен хотя бы один источник.");const Ae=ae.kind||"interpretation",be=ae.from_id||K.from_id,ke=ae.to_id||K.to_id;if(["relation","source_route"].includes(Ae)&&(!be||!ke))throw new Error("Для предложения о связи нужны оба участника.");return j_(a,{id:Y("proposal"),kind:Ae,parentHypothesisId:Y("hypothesis"),targetId:K.id,...be&&ke?{fromId:be,toId:ke}:{},statement:F,sourceRefs:xe,evidenceRefs:ae.evidence_refs?.length?ae.evidence_refs:xe,confidencePosture:{value:ae.confidence||"unknown",meaning:"maker_declared_uncertainty_not_truth_probability"},actorOrigin:ae.actor_origin==="agent"?"agent":"human",basePageRevision:ae.context_revision||0,dataFingerprint:e.port.packet?.source_revision||"unavailable"})}function J(){const F=URL.createObjectURL(new Blob([a.exportPacket()],{type:"application/json"})),K=Qe("a");K.href=F,K.download="sophia-research.json",K.click(),setTimeout(()=>URL.revokeObjectURL(F),1e3)}function ue(){const F=D?S:P;h.append(Qe("p",F?"Для: "+F.label:"Общие записи исследования","sc-muted")),h.append(Qe("p","Записи и гипотезы сохраняются в этом браузере. Предложения остаются черновиками до рассмотрения.","sc-muted"));const K=Qe("form"),ae=Qe("label","Новая запись");ae.htmlFor="sc-note-text";const xe=Qe("textarea");xe.id="sc-note-text",xe.maxLength=2e3,xe.placeholder="Мысль, вопрос или наблюдение…",xe.value=D,xe.addEventListener("input",()=>{D||(S=P?{...P}:null),D=xe.value});const Ae=Qe("label","Тип записи");Ae.htmlFor="sc-note-kind";const be=Qe("select");be.id="sc-note-kind";for(const[at,$e]of[["note","Заметка"],["hypothesis","Гипотеза"],["proposal","Предложение к рассмотрению"]]){const E=Qe("option",$e);E.value=at,be.append(E)}be.value=v,be.addEventListener("change",()=>v=be.value);const ke=Qe("button","Сохранить запись");ke.type="submit",K.append(ae,xe,Ae,be,R(ke)),K.addEventListener("submit",at=>{at.preventDefault(),C(()=>{const $e=xe.value.trim();if(!$e)throw new Error("Напишите текст записи.");const E=D?S:P;be.value==="hypothesis"?de($e,E):be.value==="proposal"?ce($e,E):ne($e,E?.id),D="",S=null,k("notes"),_.textContent="Запись сохранена."})}),h.append(K);const Xe=An("Отменить",()=>{a.undo(),k("notes")}),Ze=An("Повторить",()=>{a.redo(),k("notes")});Xe.disabled=!a.canUndo(),Ze.disabled=!a.canRedo();const Ne=Qe("input");Ne.type="file",Ne.accept=".json,application/json",Ne.hidden=!0,Ne.setAttribute("aria-label","Импорт исследования"),Ne.addEventListener("change",async()=>{const at=Ne.files?.[0];if(at){if(at.size>1e6){z(new Error("Файл превышает 1 МБ."));return}try{const $e=await at.text();a.importPacket($e),k("notes"),_.textContent="Исследование импортировано."}catch($e){z($e)}}}),h.append(R(Xe,Ze,An("Экспорт",J),An("Импорт",()=>Ne.click())),Ne);const N=a.getState();for(const[at,$e]of[["Заметка",N.notes],["Гипотеза",N.hypotheses],["Предложение · ожидает рассмотрения",N.proposals]])for(const E of $e.slice().reverse()){const f=Qe("article","","sc-entry");f.append(Qe("small",at),Qe("p",E.body||E.statement)),E.targetId&&f.append(Qe("small",E.targetId)),at==="Заметка"&&f.append(R(An("Удалить",()=>{a.removeNote(E.id),k("notes")}))),h.append(f)}!N.notes.length&&!N.hypotheses.length&&!N.proposals.length&&h.append(Qe("p","Здесь появятся ваши записи.","sc-muted")),a.persistenceError()&&(_.textContent="Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием.")}function me(F){const K=Qe("article","","sc-entry");K.append(Qe("h4",F.label||F.preferred_label||F.node_id));const ae=F.properties||{};(ae.description||ae.notes)&&K.append(Qe("p",ae.description||ae.notes,"sc-source-text"));for(const xe of[...new Set([...F.source_refs||[],ae.url,ae.locator,ae.source_url].filter(Ae=>typeof Ae=="string"))])K.append(Ws(xe));return K}async function B(){if(G(),!M){h.append(Qe("p","Выберите звезду или отношение, чтобы увидеть источники."));return}const F=M;h.append(Qe("p",vt(I==="relation"?F.display.explanation:F.display.summary,"Описание пока не зафиксировано."),"sc-source-text"));const K=Qe("details");K.append(Qe("summary","Происхождение и статус"));for(const[Ae,be]of[["Слой",F.epistemic?.authority_layer],["Рассмотрение",F.epistemic?.review_posture],["Канон",F.epistemic?.canon_status]])K.append(Qe("p",Ae+": "+(be&&be!=="not-recorded"?be:"не указан"),"sc-muted"));for(const Ae of F.source_refs)K.append(Ws(Ae));if(h.append(K),I==="relation")return;const ae=F.native_id;if(!ae)return;const xe=Qe("div");h.append(xe),xe.append(Qe("p","Получаю досье источников…","sc-muted")),h.setAttribute("aria-busy","true");try{const Ae=await o.run("source",Xe=>p.invoke("tos.dossier.inspect",{object_id:ae,limit:40},{signal:Xe}));if(!Ae.current||d.hidden||A!=="sources")return;xe.replaceChildren();const be=Ae.value;for(const[Xe,Ze]of[["work","Произведение"],["expression","Редакции и переводы"],["edition","Издания"],["file","Файлы"],["item","Экземпляры"],["link","Ссылки"]]){const Ne=be.chain?.[Xe]||[];if(!Ne.length)continue;const N=Qe("details");N.append(Qe("summary",Ze+" · "+Ne.length));for(const at of Ne)N.append(me(at));xe.append(N)}const ke=be.agent_summary;ke&&xe.append(Qe("p","Доступность ссылки и право использования — отдельные сведения. Статус прав: "+(ke.rights_posture&&ke.rights_posture!=="unknown"?ke.rights_posture:"не указан")+".","sc-muted")),xe.children.length||xe.append(Qe("p","Дополнительные маршруты источников пока не записаны.","sc-muted")),h.setAttribute("aria-busy","false"),m.restore(),e.invalidate()}catch(Ae){h.setAttribute("aria-busy","false"),xe.replaceChildren(Qe("p",Ae.message,"sc-muted")),xe.append(R(An("Повторить",()=>k("sources")))),e.invalidate()}}function oe(){h.append(Qe("p","Ищите недостающие источники или подготовьте разбор слова в «Заратустре».","sc-muted"));const F=Qe("label","Запрос");F.htmlFor="sc-analysis-query";const K=Qe("input");K.id="sc-analysis-query",K.placeholder="Название источника или слово…",K.maxLength=256;const ae=Qe("div");h.append(F,K,R(An("Пробелы в источниках",()=>$("gaps",K.value,ae).catch(()=>{})),An("Разобрать слово",()=>$("word",K.value,ae).catch(()=>{}))),ae)}async function $(F,K,ae,xe,Ae={}){_.textContent="Получаю материал…",ae.replaceChildren();try{const be=F==="gaps"?"tos.source-gaps.search":"tos.zarathustra.word-analysis.prepare",ke=F==="gaps"?{query:K,limit:Ae.limit??12}:{query:K,language:Ae.language||"ru",rank:Ae.rank??1,include_semantic_neighbors:Ae.include_semantic_neighbors===!0},Xe=await o.run("analysis",Ne=>p.invoke(be,ke,{signal:xe?AbortSignal.any([Ne,xe]):Ne}));if(!Xe.current||d.hidden||A!=="analysis")throw xe?.throwIfAborted(),new DOMException("Panel closed","AbortError");xe?.throwIfAborted();const Ze=Xe.value;if(_.textContent="",F==="gaps"){l.clear();for(const Ne of Ze.gaps||[]){l.set(Ne.edge_id,Ne);const N=Qe("article","","sc-entry");N.append(Qe("h4",Ne.to_label||Ne.label),Qe("p",Ne.properties?.public_summary_en||Ne.summary||""),Qe("small","Доступ: "+Ne.access_status+" · Запрос: "+Ne.request_status));for(const at of Ne.source_refs||[])N.append(Ws(at));N.append(R(An("Рассмотреть",()=>_e(Ne.edge_id)))),ae.append(N)}Ze.gaps?.length||ae.append(Qe("p","По этому запросу пробелов не найдено."))}else if(Ze.available!==!0)ae.append(Qe("p","Разбор для этого запроса сейчас недоступен."),Qe("p",Ze.reason||"","sc-muted"));else{const Ne=Ze.task?.source||{};ae.append(Qe("h4",Ne.surface||Ne.text||K),Qe("p",Ne.context||Ne.excerpt||Ne.sentence||"")),Ne.source_ref&&ae.append(Ws(Ne.source_ref)),ae.append(Qe("p","Подготовлен разбор по исходному тексту. Результат требует рассмотрения.","sc-muted")),ae.append(R(An("Сохранить задание",()=>{const N=URL.createObjectURL(new Blob([JSON.stringify(Ze,null,2)],{type:"application/json"})),at=Qe("a");at.href=N,at.download="sophia-word-analysis.json",at.click(),setTimeout(()=>URL.revokeObjectURL(N),1e3)})))}return e.invalidate(),Ze}catch(be){throw z(be),be}}function se(){}function _e(F){const K=l.get(F);return K?(c={id:K.edge_id,kind:"edge",semantic_kind:"source_access_gap",label:K.to_label,from_id:K.from_id,to_id:K.to_id,predicate_id:K.predicate_id,source_refs:K.source_refs,authority_posture:K.authority_posture,review_posture:K.review_posture,canon_status:K.canon_status,reroutable:!1},P=c,i.open("workspace"),u.setAttribute("aria-expanded","true"),k("notes"),r(),!0):!1}const Me={"tos.page.research-workspace":()=>({packet:JSON.parse(a.exportPacket()),summary:a.summary()}),"tos.page.add-research-note":F=>ne(String(F.text||""),F.target_id?String(F.target_id):void 0),"tos.page.add-session-hypothesis":F=>({hypothesis:de(String(F.statement||""),t()),summary:a.summary()}),"tos.page.stage-proposal":F=>({proposal:ce(String(F.statement||""),t(),F),summary:a.summary()}),"tos.page.workspace-undo":()=>({changed:a.undo(),summary:a.summary()}),"tos.page.workspace-redo":()=>({changed:a.redo(),summary:a.summary()}),"tos.page.workspace-export":()=>({packet:JSON.parse(a.exportPacket())}),"tos.page.workspace-import":F=>({imported:a.importPacket(typeof F.packet=="string"?F.packet:JSON.stringify(F.packet)),summary:a.summary()}),"tos.page.find-source-gaps":async(F,{signal:K})=>{Z("analysis");const ae=Qe("div");h.append(ae);const xe=await $("gaps",String(F.query||""),ae,K,F);return{...xe,gaps:xe.gaps.map(Ae=>({...Ae,id:Ae.edge_id,label:Ae.to_label,summary:Ae.properties?.public_summary_en}))}},"tos.page.prepare-word-analysis":async(F,{signal:K})=>{Z("analysis");const ae=Qe("div");return h.append(ae),$("word",String(F.query||""),ae,K,F)}};let ve=!1;return a.subscribe(()=>{r(),!ve&&(ve=!0,queueMicrotask(()=>{if(ve=!1,d.hidden||A!=="notes")return;const F=document.activeElement,K=h.querySelector("textarea"),ae=F===K,xe=K?.selectionStart,Ae=K?.selectionEnd;if(h.replaceChildren(),ue(),ae){const be=h.querySelector("textarea");be.focus(),be.setSelectionRange(xe,Ae)}e.invalidate()}))}),ki(),window.addEventListener("pagehide",()=>o.cancelAll(),{once:!0}),{workspace:a,handlers:Me,selectionChanged:se,chooseGap:_e,get auxiliarySelection(){return c},clearAuxiliarySelection(){c=null},agentStatus(F){g.textContent=F.registered?"Агент подключён к текущему пространству":F.supported?"Подключаю агента…":"Записи хранятся локально"}}}function t0(n,e,{onUserAction:t=()=>{}}={}){const i=new Map,r=new Map,s=[];let a=structuredClone(al),o=null,l="",c=!1;try{o=localStorage,a=du(o)}catch(S){l=S.message}const p=()=>JSON.stringify([e.port.packet?.source_revision,e.port.packet?.fingerprint,e.port.selection]),u=S=>S==="inspector"?"К карточке":{evidence:"К основаниям",workspace:"К источникам",navigation:"К маршруту",builder:"К линзе",studio:"К рабочему месту"}[S]||"Назад";function d(){for(const[S,w]of i)if(!w.element.hidden)return S;return null}function h(){try{o?.setItem(Xc,JSON.stringify(a)),l=o?"":"Настройки действуют до закрытия страницы: хранилище недоступно."}catch{l="Браузер не сохранил настройки. Они действуют до закрытия страницы."}}function _(){if(a.dock!=="auto")return a.dock;const S=n.getBoundingClientRect(),w=S.left+S.width/2,L=n.querySelector(".sc-inspector");if(!L.hidden&&S.width>650){const Z=L.getBoundingClientRect();return Z.left+Z.width/2<w?"left":"right"}for(const{element:Z}of i.values())if(!Z.hidden&&Z.dataset.dock)return Z.dataset.dock;const V=[...n.querySelectorAll(".sc-node")].find(Z=>Z.dataset.id===n.dataset.selected&&!Z.hidden);if(V){const Z=V.getBoundingClientRect();if(Z.width)return Z.left+Z.width/2>w?"left":"right"}return"right"}function x(S){const w=i.get(S);!w||w.element.hidden||(w.onHide(),w.element.hidden=!0,e.invalidate())}const g=new ResizeObserver(()=>{e.port.cardChanged(),e.invalidate()});function m(S){const w=i.get(S),L=a.sizes[S];if(w)for(const V of["width","height"])L?w.element.style.setProperty("--sc-panel-"+V,L[V]+"px"):w.element.style.removeProperty("--sc-panel-"+V)}function A(S,w,L){a.sizes[S]={width:Math.max(280,Math.min(760,w)),height:Math.max(240,Math.min(800,L))},m(S),e.invalidate()}function P(){t();const S=p();let w;for(;s.length;){const V=s.pop();if(V.key===S){w=V;break}}if(!w){M();return}c=!0,I(w.id,!1),c=!1,i.get(w.id).onResume?.();const L=w.focus;L?.isConnected&&!L.closest("[hidden]")?L.focus():i.get(w.id).element.querySelector("button")?.focus()}function M(){for(const S of i.values()){const w=[...s].reverse().find(L=>L.key===p());S.back.hidden=!w,S.back.textContent=w?"← "+u(w.id):""}}function I(S,w=!0){if(!i.has(S))throw new Error("Unknown panel: "+S);const L=d(),V=_();w&&!c&&L&&L!==S&&(s.push({id:L,key:p(),focus:document.activeElement}),s.length>8&&s.shift());for(const k of i.keys())k!==S&&x(k);S!=="inspector"&&(e.closeInspector(!1),n.querySelector(".sc-search-close").click(),n.querySelector(".sc-lenses-close").click());const Z=i.get(S).element;if(S!=="inspector"){const k=n.querySelector(".sc-context").getBoundingClientRect();Z.style.setProperty("--sc-tool-top",`${k.bottom-n.getBoundingClientRect().top+18}px`),Z.dataset.dock=V}Z.hidden=!1,m(S),M(),e.invalidate()}function T(S,w,L=()=>{},V={}){if(i.has(S))throw new Error("Duplicate panel: "+S);w.dataset.panelId=S;const Z=document.createElement("div");Z.className="sc-reading-return";const k=document.createElement("button");k.type="button",k.hidden=!0,k.addEventListener("click",P),Z.append(k),w.querySelector(".sc-panel-top").after(Z);const z=document.createElement("button");z.type="button",z.className="sc-panel-size",z.textContent="↔",z.setAttribute("aria-label","Изменить размер окна"),z.title="Размер окна: обычный / просторный",z.addEventListener("click",()=>{t();const Y=a.sizes[S];A(S,Y?.width>=560?420:640,Y?.width>=560?440:620),h()}),w.querySelector(".sc-panel-top").insertBefore(z,w.querySelector(".sc-panel-top").lastElementChild);const C=document.createElement("button");C.type="button",C.className="sc-panel-resize",C.textContent="⌟",C.setAttribute("aria-label","Размер окна: стрелки меняют ширину и высоту"),C.title="Потяните угол; стрелки меняют размер, Home сбрасывает";let R=null;C.addEventListener("pointerdown",Y=>{if(Y.button!==0)return;t();const ne=w.getBoundingClientRect();R={id:Y.pointerId,x:Y.clientX,y:Y.clientY,w:ne.width,h:ne.height},C.setPointerCapture(Y.pointerId),Y.preventDefault()}),C.addEventListener("pointermove",Y=>{R?.id===Y.pointerId&&A(S,R.w+(Y.clientX-R.x)*(w.dataset.dock==="right"?-1:1),R.h+Y.clientY-R.y)});const G=()=>{R&&(R=null,h())};C.addEventListener("pointerup",G),C.addEventListener("pointercancel",G),C.addEventListener("lostpointercapture",G),C.addEventListener("keydown",Y=>{const ne={ArrowLeft:[-20,0],ArrowRight:[20,0],ArrowUp:[0,-20],ArrowDown:[0,20]}[Y.key];if(Y.key==="Home")t(),Y.preventDefault(),delete a.sizes[S],m(S),h();else if(ne){t(),Y.preventDefault();const de=w.getBoundingClientRect();A(S,de.width+ne[0],de.height+ne[1]),h()}}),w.append(C),i.set(S,{element:w,onHide:L,...V,back:k}),m(S),g.observe(w)}T("inspector",n.querySelector(".sc-inspector"),()=>{e.ui.captureReading(),e.ui.cancelInspector()},{onResume:()=>e.ui.restoreReading()});const D=new MutationObserver(()=>{if([".sc-inspector",".sc-search",".sc-lenses"].some(S=>!n.querySelector(S).hidden))for(const[S,w]of i)S!=="inspector"&&!w.element.hidden&&x(S);M()});for(const S of[".sc-inspector",".sc-search",".sc-lenses"])D.observe(n.querySelector(S),{attributes:!0,attributeFilter:["hidden"]});D.observe(n,{attributes:!0,attributeFilter:["data-graph-revision","data-selected"]});function v(){n.dataset.readingSize=a.text,n.dataset.labelSize=a.labels;for(const[w,L]of r)L.opener.hidden=!a.pinned.includes(w),a.pinned.includes(w)&&n.querySelector(".sc-header-actions").append(L.opener);for(const w of a.pinned){const L=r.get(w);L&&n.querySelector(".sc-header-actions").append(L.opener)}const S=n.querySelector(".sc-studio-open");S&&n.querySelector(".sc-header-actions").append(S);for(const[w,L]of i)m(w),w!=="inspector"&&!L.element.hidden&&a.dock!=="auto"&&(L.element.dataset.dock=a.dock);e.port.refreshTypography?.(),e.invalidate()}return v(),{register:T,open:I,close:x,back:P,configure(S,w){Object.assign(i.get(S),w)},get preferences(){return structuredClone(a)},get storageError(){return l},setPreferences(S){a=qc(S),h(),v()},addTool(S,w){if(r.has(S))throw new Error("Duplicate tool: "+S);r.set(S,w),v()},toolList:()=>[...r].map(([S,w])=>({id:S,title:w.title,available:w.available?.()!==!1})),launch(S){const w=r.get(S);if(!w||w.available?.()===!1)throw new Error("Сначала выберите звезду или связь.");w.launch()}}}const Hi=n=>[...new Set([n?.source_ref,...n?.source_refs||[]].filter(e=>typeof e=="string"&&e))];function Fd(n){return n?.native_id?n.source_graph==="philosophy"?{mode:"philosophy",item_id:n.native_id}:n.source_graph==="canon"?{mode:"corpus",item_id:n.native_id,view_id:"route-graph"}:null:null}function n0(n,e,t,i){const r=n?.selection,s=n?.authority_boundary;if(n?.schema!=="tos_evidence_lens_packet_v1"||n.mode!==i.mode||n.item_id!==e.native_id||r?.[t==="relation"?"edge_id":"node_id"]!==e.native_id||!["is_source","is_canon","is_semantic_truth","is_rights_clearance"].every(a=>s?.[a]===!1)||!Hi(r).some(a=>Hi(e).includes(a))||!["challenge_relations","context_relations","neighbor_nodes","source_refs","routes","source_anchors","gaps"].every(a=>Array.isArray(n[a]))||typeof n.conclusion?.can_conclude!="boolean")throw new Wt("Основания не удалось связать с выбранным объектом. Обновите область.");return n}async function i0(n,e,t,{client:i,queries:r,signal:s,limit:a=60}){const{match:o}=await i.inspect(e,n.id,s,t);if(s?.throwIfAborted(),o.content_revision!==n.content_revision)throw new Ti;const l=Fd(o);if(!l)return{raw:o,availability:"not_connected",packet:null};let c;try{c=await r.invoke("tos.epistemic.inspect",{...l,limit:a},{signal:s})}catch(u){if(u.status!==404)throw u;return s?.throwIfAborted(),{raw:o,availability:"outside_route",packet:null}}s?.throwIfAborted(),n0(c,o,e,l);const p=await i.inspect(e,n.id,s,t);if(s?.throwIfAborted(),p.match.content_revision!==o.content_revision)throw new Ti;return{raw:o,availability:"available",packet:c,binding:{knowledge_id:n.id,native_id:o.native_id,source_graph:o.source_graph,source_revision:t,content_revision:o.content_revision}}}function r0(n,e=[]){const t=n.properties||{},i=n.edge_id||n.node_id,r=s=>{const a=e.find(o=>o.node_id===s);return a?.label_ru||a?.preferred_label||a?.label||s};return{id:i,label:n.label_ru||n.label||t.relation_label||n.predicate_id||i,statement:n.summary_ru||n.summary||t.comment||t.description||"",route:n.from_id&&n.to_id?r(n.from_id)+" → "+r(n.to_id):"",predicate_id:n.predicate_id,from_id:n.from_id,to_id:n.to_id,source_refs:Hi(n),authority_posture:t.authority_posture,canon_status:t.canon_status,review_posture:t.review_posture,confidence:t.confidence||t.master_confidence}}function Cc(n,e){const t=n.packet;if(!t)throw new Error("Для этого объекта сравнение прочтений пока не подключено.");const i=a=>a.filter(o=>(o.edge_id||o.node_id)!==t.item_id).map(o=>r0(o,[t.selection,...t.neighbor_nodes])),r=i(t.challenge_relations),s=i(t.context_relations);return{schema:"tos_interpretation_comparison_v1",selection:e,binding:n.binding,posture:t.posture,can_conclude:t.conclusion.can_conclude===!0,competing_reading_count:r.length,competing_readings:r,contextual_readings:s,coverage:t.coverage,gaps:t.gaps_ru||t.gaps,authority_note:t.authority_note}}function s0(n,e){return{id:n.id,kind:e==="relation"?"edge":"node",label:vt(n.display.title||n.display.label,n.id),source_refs:Hi(n)}}const st=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},Xs=(n,e,t="")=>{const i=st("button",n,t);return i.type="button",i.addEventListener("click",e),i},a0={"pre-canon":"До канона",canon:"Канон","derived-export":"Проекция источников",prepared_research_candidate:"Исследовательский кандидат",prepared_branch_candidate:"Кандидат ветви",contested_review_required:"Требует рассмотрения",pending_human_review:"Ожидает рассмотрения","not-recorded":"Не указан",unresolved:"Не разрешено",review_status_unresolved:"Статус рассмотрения не установлен",contested_by:"Оспаривается",uncertain_relation:"Неопределённая связь",polemicizes_with:"Полемизирует с"},ji=n=>a0[n]||n||"Не указан";function o0(n,e,t,{selected:i,onUserAction:r}){const s=new or,a=new Hr,o=jo(async(J,ue={})=>{const me=AbortSignal.any([ue.signal||new AbortController().signal,AbortSignal.timeout(6e4)]);try{const B=await fetch(J,{...ue,signal:me});if(!B.ok)throw new Ys(B.status,"Не удалось получить основания. Попробуйте ещё раз.");return await B.json()}catch(B){throw me.aborted?me.reason:B instanceof TypeError?new Error("Нет связи с данными. Повторите запрос после подключения."):B}}),l=st("section","","sc-panel sc-evidence");l.hidden=!0,l.setAttribute("aria-label","Основания и прочтения"),l.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ЛИСТ ИССЛЕДОВАНИЯ</span><button type="button" class="sc-icon sc-evidence-close" aria-label="Закрыть основания"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-evidence-heading"><span class="sc-evidence-symbol" aria-hidden="true">✧</span><div><p class="sc-evidence-kind"></p><h3></h3></div></div><div class="sc-evidence-tabs" role="tablist" aria-label="Основания и сравнение"></div><div class="sc-evidence-body" id="sc-evidence-content" role="tabpanel" tabindex="0"></div><div class="sc-evidence-status" role="status"></div><div class="sc-evidence-footer"><span>От мысли — к источнику</span></div>',n.append(l);const c=l.querySelector(".sc-evidence-body"),p=l.querySelector(".sc-evidence-status"),u=l.querySelector(".sc-evidence-tabs"),d=as(c),h=new Map;let _=0,x=null,g=null,m=null,A="grounds",P=null,M="",I=0;t.register("evidence",l,()=>{d.capture(),a.cancelAll(),c.setAttribute("aria-busy","false")});function T(){t.close("evidence"),r();const J=[...n.querySelectorAll(".sc-node")].find(ue=>ue.dataset.id===x?.raw.id&&!ue.hidden);(P?.isConnected&&!P.closest("[hidden]")?P:J||n.querySelector(".sc-overview")).focus()}l.querySelector(".sc-evidence-close").addEventListener("click",T),l.addEventListener("keydown",J=>{J.key==="Escape"&&(J.preventDefault(),J.stopPropagation(),T())});const D=["grounds","compare"],v=["Основания","Сравнение"].map((J,ue)=>{const me=Xs(J,()=>{r(),S(D[ue])});return me.id="sc-evidence-"+D[ue],me.setAttribute("role","tab"),me.setAttribute("aria-controls",c.id),u.append(me),me});u.addEventListener("keydown",J=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(J.key))return;J.preventDefault();const ue=J.key==="Home"?0:J.key==="End"?1:1-v.indexOf(J.target);r(),S(D[ue]),v[ue].focus()});function S(J){d.capture(),A=J,h.set(M,J),h.size>48&&h.delete(h.keys().next().value),l.dataset.tab=J,v.forEach((ue,me)=>{ue.setAttribute("aria-selected",String(D[me]===J)),ue.tabIndex=D[me]===J?0:-1}),c.setAttribute("aria-labelledby","sc-evidence-"+J),Y()}function w(J,ue,me=""){const B=st("section","","sc-evidence-section "+me);B.append(st("h4",J));for(const oe of ue)B.append(typeof oe=="string"?st("p",oe):oe);return B}function L(J,ue="Источники"){const me=st("details","","sc-evidence-refs");me.append(st("summary",ue+" · "+J.length));for(const B of J){const oe=st("div","","sc-evidence-ref");if(/^https?:\/\//i.test(B))try{const $=new URL(B),se=st("a",$.hostname+$.pathname);se.href=$.href,se.target="_blank",se.rel="noopener noreferrer",oe.append(se)}catch{oe.append(st("span",B))}else oe.append(st("span",B));oe.append(Xs("Копировать",async()=>{try{await navigator.clipboard.writeText(B),p.textContent="Ссылка на источник скопирована."}catch{p.textContent="Выделите и скопируйте путь к источнику."}},"sc-evidence-copy")),me.append(oe)}return me}function V(J){const ue=st("dl","","sc-evidence-posture");for(const[me,B]of[["Слой",J.epistemic?.authority_layer],["Рассмотрение",J.epistemic?.review_posture],["Канон",J.epistemic?.canon_status]]){const oe=st("div");oe.append(st("dt",me),st("dd",ji(B))),ue.append(oe)}return ue}function Z(J,ue,me=""){if(!ue?.length)return null;const B=st("ul");for(const oe of ue)B.append(st("li",oe));return w(J,[B],me)}function k(...J){c.append(...J.filter(Boolean))}function z(){const J=g?.raw||x.raw;k(V(J),L(Hi(J),"Происхождение объекта"));const ue=Xs("Открыть досье источников",()=>{r(),n.dispatchEvent(new CustomEvent("sophia-sources",{detail:{raw:J,kind:x.kind}}))},"sc-evidence-source");c.append(ue)}function C(){const J=g.raw,ue=g.packet;if(!ue){k(w("Происхождение",[vt(x.kind==="relation"?J.display.explanation:J.display.summary,"Описание пока не записано.")],"sc-evidence-finding")),k(w("Маршрут оснований",[g.availability==="outside_route"?"Объект не найден в доступной области Evidence Lens. Это не означает, что у него нет оснований.":"Для этого слоя отдельный маршрут оснований ещё не подключён. Здесь показаны сведения из карточки и её источники."])),z();return}k(w("Что установлено",[ue.finding_ru||ue.finding],"sc-evidence-finding"));const me=st("div","","sc-evidence-conclusions");for(const B of[Z("Можно утверждать",ue.conclusion.allowed_ru||ue.conclusion.allowed,"sc-evidence-allowed"),Z("Вывод пока не следует",ue.conclusion.not_allowed_ru||ue.conclusion.not_allowed,"sc-evidence-limits")])B&&me.append(B);if(k(me),ue.conclusion.can_conclude!==!0&&k(st("p","Материала недостаточно для окончательного вывода в указанной области.","sc-evidence-note")),k(Z("Открытые вопросы",ue.gaps_ru||ue.gaps)),ue.source_anchors.length){const B=st("details","","sc-evidence-refs");B.append(st("summary","Точные фрагменты · "+ue.source_anchors.length));for(const oe of ue.source_anchors){const $=st("div","","sc-evidence-ref");$.append(st("p",(oe.anchor_segment_ids||[]).join(" · ")),st("small",oe.witness_scope||"")),oe.relation_ref&&$.append(L([oe.relation_ref],"Запись связи")),B.append($)}k(B)}if(ue.routes.length){const B=st("details","","sc-evidence-refs");B.append(st("summary","Маршруты к основаниям · "+ue.routes.length));for(const oe of ue.routes){const $=st("div","","sc-evidence-ref");$.append(st("small",ji(oe.route_kind)+" · "+ji(oe.status))),oe.ref&&$.append(L([oe.ref],"Открыть путь")),B.append($)}k(B)}k(L(ue.source_refs,"Источники поля оснований")),z()}function R(J,ue,me=!1){const B=st("article","","sc-reading"+(me?" sc-reading-selected":""));return B.append(st("span",ue,"sc-reading-caption"),st("h4",ji(J.label))),J.route&&B.append(st("p",J.route,"sc-reading-route")),B.append(st("p",J.statement||"Описание этого прочтения пока не записано.","sc-reading-text")),(J.review_posture||J.canon_status)&&B.append(st("p",[J.review_posture,J.canon_status].filter(Boolean).map(ji).join(" · "),"sc-evidence-note")),B.append(L(J.source_refs)),B}function G(){if(!g.packet){k(w("Сопоставление ещё не подключено",[g.availability==="outside_route"?"Объект находится за пределами доступного маршрута оснований.":"В этом слое пока нет подключённого поля прочтений. Соседство на карте само по себе не означает разногласия."])),z();return}const J=Cc(g,s0(g.raw,x.kind)),ue=g.packet,me=g.raw;k(st("p","Сопоставление показывает записанные связи и вопросы к ним. Их истинность определяется рассмотрением источников.","sc-evidence-note"));const B=J.competing_readings,oe={label:vt(me.display.title||me.display.label),statement:vt(x.kind==="relation"?me.display.explanation:me.display.summary),route:vt(me.display.statement),source_refs:Hi(me),review_posture:me.epistemic?.review_posture,canon_status:me.epistemic?.canon_status},$=st("div","","sc-reading-spread");if($.append(R(oe,"ВЫБРАНО",!0)),B.length){const _e=st("div","","sc-reading-alternative"),Me=st("label","Сопоставить с");Me.htmlFor="sc-reading-choice";const ve=st("select");ve.id=Me.htmlFor;for(const[ae,xe]of B.entries()){const Ae=st("option",`${ae+1}. ${ji(xe.label)}${xe.route?" · "+xe.route:""}`);Ae.value=String(ae),ve.append(Ae)}ve.value=String(Math.min(_,B.length-1));const F=st("div"),K=()=>{F.replaceChildren(R(B[Number(ve.value)],"ДРУГОЕ ПРОЧТЕНИЕ")),e.invalidate()};ve.addEventListener("change",()=>{r(),_=Number(ve.value),K()}),_e.append(Me,ve,F),K(),$.append(_e)}else $.append(w("Других прочтений не показано",["В полученной области нет других оспаривающих связей. Это не означает согласия или доказанности."],"sc-reading-empty"));k($);const se=ue.coverage;if(k(st("p",`Получено оспаривающих связей: ${se.returned_challenge_relations??ue.challenge_relations.length} из ${se.available_challenge_relations??ue.challenge_relations.length}.`+(x.kind==="relation"?" Выбранная связь исключена из второго столбца.":""),"sc-evidence-note")),J.contextual_readings.length){const _e=st("details","","sc-evidence-refs");_e.append(st("summary","Контекст · "+J.contextual_readings.length));for(const Me of J.contextual_readings)_e.append(R(Me,"СВЯЗЬ КОНТЕКСТА"));k(_e)}k(Z("Что остаётся открытым",J.gaps))}function Y(){d.capture(),d.enter(M+"|"+A),c.replaceChildren(),p.textContent="",c.setAttribute("aria-busy",String(!g&&!m)),m?k(w("Не удалось прочитать основания",[m.message]),Xs("Повторить",()=>{r(),ne(x,A).catch(()=>{})},"sc-evidence-source")):g?A==="grounds"?C():G():k(st("div","Собираю источники и прочтения…","sc-evidence-loading")),d.restore(),e.invalidate()}async function ne(J,ue=null,{signal:me,limit:B=60}={}){if(!J?.raw)throw new Error("Сначала выберите звезду или отношение.");const oe=++I;d.capture();const $=e.port.packet?.source_revision,se=JSON.stringify([J.raw.id,J.kind,$]);se!==M&&(_=0),P=document.activeElement,t.open("evidence"),x=J,g=null,m=null,M=se,ue=ue||h.get(M)||"grounds",l.dataset.itemId=J.raw.id,l.querySelector("h3").textContent=ji(vt(J.raw.display.title||J.raw.display.label)),l.querySelector(".sc-evidence-kind").textContent=J.kind==="relation"?"ОТНОШЕНИЕ":vt(J.raw.display.kind_label,"УЗЕЛ").toUpperCase(),S(ue),v[D.indexOf(ue)].focus();try{const _e=await a.run("evidence",Me=>i0(J.raw,J.kind,$,{client:s,queries:o,limit:B,signal:me?AbortSignal.any([me,Me]):Me}));if(me?.throwIfAborted(),!_e.current||l.hidden)throw new DOMException("Panel closed","AbortError");return g=_e.value,Y(),g}catch(_e){throw oe===I&&!l.hidden&&(m=_e.name==="AbortError"?new Error("Чтение прервано. Можно повторить запрос."):_e,Y()),_e}}n.addEventListener("sophia-evidence",J=>{r(),ne(J.detail).catch(()=>{})});function de(){const J=i();!l.hidden&&M!==JSON.stringify([J?.id,J?.kind==="edge"?"relation":"node",e.port.packet?.source_revision])&&t.close("evidence")}async function ce(J,ue,{signal:me}){const B=i(),oe=B?.kind==="edge"?"relation":"node",$=oe==="relation"?e.port.relation(B?.id):e.port.node(B?.id),se=await ne({raw:$,kind:oe},J,{signal:me,limit:Number(ue.limit)||60});if(!se.packet)throw new Error("Для этого объекта маршрут Evidence Lens сейчас недоступен.");return J==="compare"?Cc(se,B):{...se.packet,binding:se.binding,agent_summary:{...se.packet.agent_summary,selection:$.id}}}return t.configure("evidence",{onResume:()=>{g?d.restore():ne(x,A).catch(()=>{})}}),ki(),window.addEventListener("pagehide",()=>a.cancelAll()),{selectionChanged:de,handlers:{"tos.page.inspect-epistemic":(J,ue)=>ce("grounds",J,ue),"tos.page.compare-readings":(J,ue)=>ce("compare",J,ue)}}}const bn=n=>n?.source_graph==="philosophy"&&typeof n.native_id=="string"&&!!n.native_id,yi=()=>{throw new Wt("Маршрут не удалось связать с текущими данными. Обновите область.")};function l0(n,{depth:e=2,direction:t="either",profile:i="overview"}={}){return{focus_node_id:n,max_depth:e,direction:t,profile:i,page_nodes:10,page_relations:14}}function c0(n){return{schema_version:"tos_lens_spec_v1",lens_id:"observatory-path-search",sources:["philosophy"],language:"ru",detail:"compact",seed:{text_query:n},traversal:{depth:0,profile:"all"},relation_query:{enabled:!1},limits:{nodes:6,relations:0,groups:1}}}function d0(n,e,t,{direction:i,maxDepth:r,alternativeLimit:s,excluded:a}){(n?.schema!=="tos_philosophy_mcp_path_v2"||n.from_id!==e.native_id||n.to_id!==t.native_id||n.direction!==i||n.max_depth!==r||n.alternative_limit!==s||!Array.isArray(n.paths)||n.paths.length>s||n.path_count!==n.paths.length||n.found!==n.paths.length>0||typeof n.exploration_truncated!="boolean"||JSON.stringify([...n.excluded_edge_ids||[]].sort())!==JSON.stringify(a.map(l=>l.native_id).sort()))&&yi();const o=new Set;for(const l of n.paths){const c=l.node_ids,p=l.edge_ids;(!Array.isArray(c)||!Array.isArray(p)||!Array.isArray(l.nodes)||!Array.isArray(l.edges)||!Array.isArray(l.traversal)||c[0]!==e.native_id||c.at(-1)!==t.native_id||c.length!==p.length+1||p.length>r||new Set(c).size!==c.length||new Set(p).size!==p.length||l.nodes.length!==c.length||l.edges.length!==p.length||l.traversal.length!==p.length)&&yi();const u=JSON.stringify(p);o.has(u)&&yi(),o.add(u);for(let d=0;d<c.length;d++)l.nodes[d].node_id!==c[d]&&yi();for(let d=0;d<p.length;d++){const h=l.edges[d],_=l.traversal[d],x=h.from_id===c[d]&&h.to_id===c[d+1],g=h.to_id===c[d]&&h.from_id===c[d+1];(h.edge_id!==p[d]||a.some(m=>m.native_id===p[d])||!x&&!g||i==="outgoing"&&!x||i==="incoming"&&!g||_.edge_id!==p[d]||_.from_node_id!==c[d]||_.to_node_id!==c[d+1]||_.edge_direction!==(x?"forward":"reverse"))&&yi()}}return n}function u0(n,e){return{schema_version:"tos_lens_spec_v1",lens_id:"observatory-path",sources:["philosophy"],language:"ru",detail:"compact",seed:{focus_node_id:e.id},node_query:{filters:[{field:"native_id",op:"in",value:n.node_ids}]},relation_query:{filters:[{field:"native_id",op:"in",value:n.edge_ids}]},traversal:{depth:0,profile:"all"},limits:{nodes:9,relations:8,groups:1}}}function f0(n,e,t,i){function r(o,l,c){o.length!==l.length&&yi();const p=new Map;for(const u of o){const d=l.find(h=>h[c==="node"?"node_id":"edge_id"]===u.native_id);(!bn(u)||p.has(u.native_id)||!d||!Hi(d).some(h=>Hi(u).includes(h)))&&yi(),p.set(u.native_id,u)}return p}const s=r(e.nodes,n.nodes,"node"),a=r(e.relations,n.edges,"relation");(s.get(t.native_id)?.id!==t.id||s.get(i.native_id)?.id!==i.id)&&yi();for(const o of n.edges){const l=a.get(o.edge_id);(l.from_id!==s.get(o.from_id)?.id||l.to_id!==s.get(o.to_id)?.id)&&yi()}return{packet:e,node_ids:n.node_ids.map(o=>s.get(o).id),edge_ids:n.edge_ids.map(o=>a.get(o).id),nodes:n.node_ids.map(o=>s.get(o)),edges:n.edge_ids.map(o=>a.get(o)),traversal:n.traversal.map(o=>({edge_id:a.get(o.edge_id).id,from_node_id:s.get(o.from_node_id).id,to_node_id:s.get(o.to_node_id).id,edge_direction:o.edge_direction}))}}async function h0(n,e,t,{client:i,queries:r,signal:s,direction:a="either",maxDepth:o=6,alternativeLimit:l=3,excluded:c=[]}){if(!bn(n)||!bn(e)||c.some(x=>!bn(x)))throw new Error("Маршруты пока доступны между объектами философского графа.");if(n.id===e.id)throw new Error("Выберите две разные звезды.");if(!["outgoing","incoming","either"].includes(a)||!Number.isInteger(o)||o<1||o>8||!Number.isInteger(l)||l<1||l>5||c.length>64)throw new Error("Выберите глубину от 1 до 8 и до 5 вариантов.");if(c.some(x=>x.native_id.includes(",")))throw new Error("Эту связь пока нельзя исключить через доступный маршрут поиска.");const p=[[n,"node"],[e,"node"],...c.map(x=>[x,"relation"])],u=()=>Promise.all(p.map(([x,g])=>i.inspect(g,x.id,s,t,x.content_revision)));await u(),s?.throwIfAborted();const d={direction:a,maxDepth:o,alternativeLimit:l,excluded:c},h=d0(await r.invoke("tos.path.find",{from_id:n.native_id,to_id:e.native_id,direction:a,max_depth:o,alternative_limit:l,excluded_edge_ids:c.map(x=>x.native_id)},{signal:s}),n,e,d),_=await Promise.all(h.paths.map(async x=>f0(x,await i.compile(u0(x,n),s,t),n,e)));await u(),s?.throwIfAborted();for(const x of _)for(const g of[n,e])if(x.nodes.find(m=>m.id===g.id)?.content_revision!==g.content_revision)throw new Ti;return{schema:"tos_observatory_paths_v1",from_id:n.id,to_id:e.id,source_revision:t,found:_.length>0,path_count:_.length,paths:_,direction:a,max_depth:o,alternative_limit:l,excluded_edge_ids:c.map(x=>x.id),exploration_truncated:h.exploration_truncated,next_actions:_.length?["inspect a route node or relation","try excluding a relation"]:["change direction or depth","restore an excluded relation"]}}const _t=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},gn=(n,e,t="sc-nav-button")=>{const i=_t("button",n,t);return i.type="button",i.addEventListener("click",e),i},Zn=n=>vt(n?.display?.title||n?.display?.label,"Выбрать звезду");function p0(n,e,t,{selected:i,commit:r,onUserAction:s}){const a=new or,o=new or({base:""}),l=new Hr,c=jo((f,U)=>o.request(f,U));let p="neighbors",u=null,d=null,h=null,_=null,x=null,g=null,m=null,A=0,P=2,M="either",I="overview",T=6,D=3,v=[],S=!1,w=null,L="end",V=0,Z=0,k=!1,z=null;const C=_t("section","","sc-panel sc-navigation-panel");C.hidden=!0,C.setAttribute("aria-label","Связи и маршруты"),C.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">АТЛАС ПЕРЕХОДОВ</span><button type="button" class="sc-icon sc-nav-close" aria-label="Закрыть маршруты"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-nav-heading"><span aria-hidden="true">⟡</span><h3>Продолжить мысль</h3></div><div class="sc-nav-tabs" role="tablist" aria-label="Способ исследования"></div><div class="sc-nav-body" id="sc-nav-body" role="tabpanel" tabindex="0"></div><div class="sc-nav-status" role="status"></div><div class="sc-nav-footer"></div>',n.append(C);const R=C.querySelector(".sc-nav-body"),G=C.querySelector(".sc-nav-status"),Y=C.querySelector(".sc-nav-footer"),ne=as(R);function de(){ne.capture(),V++,l.cancelAll(),clearTimeout(Z),S=!1}t.register("navigation",C,de);const ce=gn("",()=>{s(),oe()},"sc-control sc-navigation-open");ce.setAttribute("aria-label","Связи и маршруты"),ce.innerHTML='<i data-lucide="route" aria-hidden="true"></i><span>Маршруты</span>',n.querySelector(".sc-header-actions").append(ce);function J(){t.close("navigation"),(z?.isConnected&&!z.closest("[hidden]")?z:ce).focus()}C.querySelector(".sc-nav-close").addEventListener("click",()=>{s(),J()}),C.addEventListener("keydown",f=>{f.key==="Escape"&&(f.preventDefault(),f.stopPropagation(),s(),J())});const ue=["neighbors","paths"].map((f,U)=>{const O=gn(U?"Маршрут":"Связи",()=>{s(),de(),p=f,w=null,U&&!d&&bn(me())&&(d=me()),N()},"");return O.id="sc-nav-tab-"+f,O.setAttribute("role","tab"),O.setAttribute("aria-controls","sc-nav-body"),O.addEventListener("keydown",H=>{["ArrowLeft","ArrowRight","Home","End"].includes(H.key)&&(H.preventDefault(),ue[H.key==="Home"?0:H.key==="End"?1:1-U].click(),ue[p==="paths"?1:0].focus())}),C.querySelector(".sc-nav-tabs").append(O),O}),me=()=>{const f=i();return f?.kind==="node"?e.port.node(f.id):null};function B(){const f=e.port.packet?.source_revision;return f!==_?(de(),_=f,d=null,h=null,u=null,m=null,g=null,v=[],x=null,!0):!1}function oe(f=p){B(),!x&&e.port.packet&&(x=e.port.captureView()),z=document.activeElement,t.open("navigation"),p=f,u=me()||u,g&&g.focus.node_id!==u?.id&&(g=null),p==="paths"&&!d&&bn(me())&&(d=me()),N(),ue[p==="paths"?1:0].focus()}function $(f,{command:U=!1}={}){x||(x=e.port.captureView()),k=!0;try{U?r(()=>e.port.setGraph(f)):e.port.setGraph(f)}finally{k=!1}}async function se(f,U,O){e.ui.cancelPending(),w=null,S=!0;const H=++V;N();try{const fe=await l.run("navigation",he=>f(O?AbortSignal.any([he,O]):he));if(O?.throwIfAborted(),!fe.current||H!==V||C.hidden)throw new DOMException("Navigation cancelled","AbortError");return U(fe.value),fe.value}catch(fe){throw H===V&&!C.hidden&&(w=fe.name==="AbortError"?new Error("Поиск прерван. Можно повторить."):fe),fe}finally{H===V&&(S=!1,N())}}const _e=f=>()=>{s(),f().catch(()=>{})};function Me(f){const U=R.querySelector(f);U&&!C.hidden&&(R.scrollTop+=U.getBoundingClientRect().top-R.getBoundingClientRect().top,e.invalidate())}async function ve(f=!1,U){if(!u)throw new Error("Сначала выберите звезду.");const O=f?g:null,H=u,fe=await se(async he=>{await a.inspect("node",H.id,he,_,H.content_revision);const ee=await a.explore(O?{cursor:O.page.next_cursor}:l0(H.id,{depth:P,direction:M,profile:I}),he,_,O);if(ee.focus.node_id!==H.id||ee.nodes.find(te=>te.id===H.id)?.content_revision!==H.content_revision)throw new Ti;return ee},he=>{g=he,$(he,{command:!!U})},U);return Me(".sc-nav-page"),fe}async function F(f){if(!d||!h)throw new Error("Выберите начало и конец маршрута.");const U=await se(O=>h0(d,h,_,{client:a,queries:c,signal:O,direction:M,maxDepth:T,alternativeLimit:D,excluded:v}),O=>{m=O,A=0,O.paths.length&&$(O.paths[0].packet,{command:!!f})},f);return Me(U.found?".sc-nav-variants":".sc-nav-empty"),U}function K(f,U,O,H){const fe=_t("label","","sc-nav-field");fe.append(_t("span",f));const he=_t("select");for(const[ee,te]of O){const Se=_t("option",te);Se.value=ee,he.append(Se)}return he.value=U,he.disabled=S,he.addEventListener("change",()=>{s(),de(),w=null,H(he.value),N()}),fe.append(he),fe}function ae(f){const U=_t("div","","sc-nav-options");return U.append(K("Направление",M,[["either","В обе стороны"],["outgoing","По связям →"],["incoming","Против связей ←"]],O=>{M=O,m=null,g=null})),U.append(K("Глубина",String(f?T:P),Array.from({length:f?8:3},(O,H)=>[String(H+1),String(H+1)+" "+(H===0?"шаг":H<4?"шага":"шагов")]),O=>{f?(T=Number(O),m=null):(P=Number(O),g=null)})),f?U.append(K("Варианты",String(D),[1,2,3,4,5].map(O=>[String(O),"До "+O]),O=>{D=Number(O),m=null})):U.append(K("Содержание",I,[["overview","Обзор связей"],["all","Все типы связей"]],O=>{I=O,g=null})),U}function xe(f){return _t("p",f,"sc-nav-note")}function Ae(f,U="node"){s(),!(U==="node"?e.port.node(f.id):e.port.relation(f.id))&&m?.paths[A]&&$(m.paths[A].packet),t.close("navigation"),U==="node"?e.port.selectNode(f.id):e.port.selectRelation(f.id)}function be(){if(R.append(_t("div","ОТ ВЫБРАННОЙ ЗВЕЗДЫ","sc-nav-caption"),_t("h4",u?Zn(u):"Выберите звезду в пространстве","sc-nav-origin")),!u){R.append(xe("Откройте её карточку, затем «Окрестность»."));return}R.append(ae(!1));const f=gn(S?"Раскрываю…":"Раскрыть связи",_e(()=>ve()),"sc-nav-primary");if(f.disabled=S,R.append(f),g){const U=_t("div","","sc-nav-page");if(U.append(_t("span","ОБЛАСТЬ "+String(g.page.number).padStart(2,"0"),"sc-nav-caption"),_t("p",`Звёзд: ${g.nodes.length} · связей: ${g.relations.length}`)),R.append(U),R.append(xe(`За этот обход обнаружено узлов: ${g.counts.discovered_nodes}; показано связей: ${g.counts.emitted_relations}. На экране — текущая порция.`)),g.status==="paused"){const H=gn("Продолжить раскрытие →",_e(()=>ve(!0)));H.disabled=S,R.append(H)}else R.append(xe(g.status==="complete"?"Обход завершён в выбранных пределах.":"Достигнут предел обхода. Выберите более близкий центр."));const O=_t("div","","sc-nav-discoveries");for(const H of g.page.primary_node_ids){const fe=g.nodes.find(he=>he.id===H);H!==u.id&&O.append(gn(Zn(fe),()=>Ae(fe),"sc-nav-discovery"))}R.append(O)}else R.append(xe("Связи раскрываются небольшими областями. Сохраняются положение камеры и места уже знакомых звёзд."))}function ke(f){L=f,m=null,w=null,N(),R.querySelector("input")?.focus()}function Xe(f,U,O){const H=gn("",()=>{s(),ke(O)},"sc-nav-endpoint");return H.disabled=S,H.append(_t("small",f),_t("span",Zn(U))),H.dataset.empty=String(!U),H}function Ze(){const f=_t("div","","sc-nav-search"),U=_t("label",L==="start"?"Найти начало":"Найти конечную звезду");U.htmlFor="sc-nav-query";const O=_t("input");O.id="sc-nav-query",O.type="search",O.placeholder="Имя или понятие…",O.autocomplete="off",O.disabled=S;const H=_t("div","","sc-nav-search-results");H.setAttribute("aria-live","polite"),f.append(U,O,H),R.append(f),O.addEventListener("input",()=>{s(),clearTimeout(Z),l.cancel("search"),H.replaceChildren();const fe=O.value.trim().slice(0,256);if(!fe)return;const he=L;H.append(xe("Ищу…")),Z=setTimeout(async()=>{try{const ee=await l.run("search",Se=>a.compile(c0(fe),Se,_));if(!ee.current||!O.isConnected||C.hidden)return;H.replaceChildren();const te=ee.value.nodes.filter(bn);for(const Se of te){const Ue=gn("",()=>{s(),de(),he==="start"?(d=Se,h||(L="end")):h=Se,m=null,v=[],w=null,N()},"sc-nav-search-result");Ue.dataset.itemId=Se.id,Ue.append(_t("span",Zn(Se)),_t("small",vt(Se.display.kind_label)),_t("span",vt(Se.display.summary),"sc-nav-search-context"),_t("small",Se.native_id,"sc-nav-search-identity")),H.append(Ue)}H.append(xe(te.length===6?"Первые 6 совпадений. Уточните название для более точного поиска.":te.length?"Объекты философского графа":"Совпадений в философском графе нет.")),e.invalidate()}catch(ee){O.isConnected&&!C.hidden&&(H.replaceChildren(xe(ee.message)),e.invalidate())}},200)})}function Ne(){const f=_t("div","","sc-nav-endpoints");if(f.append(Xe("01 / НАЧАЛО",d,"start"),_t("span","↓","sc-nav-connector"),Xe("02 / НАЗНАЧЕНИЕ",h,"end")),R.append(f),R.append(xe("Поиск между объектами философского графа. Связность сама по себе не означает согласия.")),m||Ze(),R.append(ae(!0)),v.length){const he=_t("div","","sc-nav-exclusions");he.append(_t("span","Обходим связи:","sc-nav-caption"));for(const ee of v){const te=gn(Zn(ee)+" · вернуть",_e(async()=>{v=v.filter(Se=>Se.id!==ee.id),await F()}));te.disabled=S,he.append(te)}R.append(he)}const U=gn(S?"Ищу пути…":m?"Найти заново":"Найти пути",_e(()=>F()),"sc-nav-primary");if(U.disabled=S||!d||!h||d.id===h.id,R.append(U),!m||(m.found||R.append(_t("h4","Здесь путь не найден","sc-nav-origin sc-nav-empty"),xe("В пределах выбранного направления, глубины и исключений. Можно изменить условия и повторить поиск.")),m.exploration_truncated&&R.append(xe("Поиск достиг вычислительного предела; другие пути могли остаться за ним.")),!m.found))return;const O=_t("div","","sc-nav-variants");O.setAttribute("role","group"),O.setAttribute("aria-label","Варианты маршрута"),m.paths.forEach((he,ee)=>{const te=gn(String(ee+1).padStart(2,"0")+" · "+he.edges.length+" св.",()=>{s(),A=ee,$(he.packet),N()},"sc-nav-variant");te.setAttribute("aria-pressed",String(ee===A)),te.disabled=S,O.append(te)}),R.append(O),R.append(xe(`Путь ${A+1} из ${m.path_count} · поиск до ${m.alternative_limit} вариантов.`));const H=m.paths[A],fe=_t("ol","","sc-nav-itinerary");H.nodes.forEach((he,ee)=>{const te=_t("li");if(te.append(gn(Zn(he),()=>Ae(he),"sc-nav-stop")),H.edges[ee]){const Se=H.edges[ee],Ue=_t("div","","sc-nav-hop");Ue.append(gn(Zn(Se),()=>Ae(Se,"relation"),"sc-nav-relation")),H.traversal[ee].edge_direction==="reverse"&&Ue.append(_t("small","← против направления связи"));const we=gn("Обойти",_e(async()=>{v.some(Te=>Te.id===Se.id)||(v=[...v,Se]),await F()}),"sc-nav-skip");we.setAttribute("aria-label","Обойти связь: "+Zn(Se)),we.disabled=S,Ue.append(we),te.append(Ue)}fe.append(te)}),R.append(fe)}function N(){ne.capture(),ne.enter(JSON.stringify([_,u?.id,p,A])),ue.forEach((f,U)=>{const O=p===(U?"paths":"neighbors");f.setAttribute("aria-selected",String(O)),f.tabIndex=O?0:-1}),R.setAttribute("aria-labelledby","sc-nav-tab-"+p),R.setAttribute("aria-busy",String(S)),R.replaceChildren(),p==="paths"?Ne():be(),G.textContent=w?.message||"",C.dataset.state=S?"loading":w?"error":"ready",Y.replaceChildren(),x?Y.append(gn("↶ К исходному виду",()=>{s();const f=x;x=null,t.close("navigation"),k=!0;try{e.port.restoreView(f)}finally{k=!1}},"sc-nav-return")):Y.append(_t("span","От звезды — к созвездию")),ne.restore(),e.invalidate()}async function at({raw:f,kind:U,tab:O}){if(B(),x||(x=e.port.captureView()),oe(O||"paths"),U==="node"){u=f,p==="paths"&&bn(f)?(d&&d.id!==f.id?h=f:d=f,m=null,v=[]):p==="paths"&&(d=null,h=null,m=null,v=[],w=new Error("Для этой звезды поиск путей пока не подключён. Можно раскрыть её связи или выбрать объекты философского графа.")),N();return}if(!bn(f)){w=new Error("Поиск обходного пути пока доступен для связей философского графа."),N();return}await se(async H=>(await Promise.all([f.from_id,f.to_id].map(he=>a.inspect("node",he,H,_)))).map(he=>he.match),H=>{[d,h]=H,v=[f],m=null})}n.addEventListener("sophia-navigate",f=>{s(),at(f.detail).catch(()=>{})});function $e(){k||(B()&&!C.hidden&&(w=new Error("Область обновилась. Выберите звезду для нового исследования."),N()),S&&(de(),C.hidden||(w=new Error("Выбор изменился. Повторите поиск из нужной звезды."),N())))}async function E(f,{signal:U},O=!1){if(f.constrain_to_view)throw new Error("Ограничение маршрута текущим видом пока не подключено. Доступен философский граф целиком.");const H=i(),fe=O?e.port.relation(H?.id):me();if(!fe||!bn(fe))throw new Error("Выберите объект философского графа.");if(B(),x||(x=e.port.captureView()),oe("paths"),O){const he=await Promise.all([fe.from_id,fe.to_id].map(ee=>a.inspect("node",ee,U,_)));U.throwIfAborted(),[d,h]=he.map(ee=>ee.match),v=[fe]}else{if(!d)throw new Error("Сначала задайте начало маршрута.");h=fe,v=[]}return f.excluded_edge_ids?.length&&(v=(await Promise.all(f.excluded_edge_ids.map(he=>a.inspect("relation",he,U,_)))).map(he=>he.match)),M=f.direction||"outgoing",T=Number(f.max_depth)||6,D=Number(f.alternative_limit)||3,F(U)}return t.configure("navigation",{onResume:()=>{ne.restore(),N()}}),ki(),window.addEventListener("pagehide",de),{selectionChanged:$e,get startId(){return d?.id||null},handlers:{"tos.page.start-path":()=>{const f=me();if(!bn(f))throw new Error("Выберите звезду философского графа.");return B(),x||(x=e.port.captureView()),d=f,h=null,m=null,v=[],oe("paths"),{path_start_node_id:d.id}},"tos.page.find-path":(f,U)=>E(f,U),"tos.page.reroute-without-selection":(f,U)=>E(f,U,!0),"tos.page.show-neighborhood":async(f,{signal:U})=>{if(B(),u=me(),!u)throw new Error("Сначала выберите звезду.");x||(x=e.port.captureView()),oe("neighbors"),P=Math.max(1,Math.min(3,Number(f.depth)||1));const O=await ve(!1,U);return{node:{node_id:u.id,label:Zn(u)},neighbors:O.nodes.filter(H=>H.id!==u.id).map(H=>({node_id:H.id,label:Zn(H)})),edges:O.relations.map(H=>({edge_id:H.id})),page:O.page,status:O.status}}}}}const Od={meaning:"Понятия и смыслы",identity:"Люди и произведения",history:"История и традиции",authorship:"Авторство и передача",evidence:"Источники и свидетельства",structure:"Структура источников",other:"Другие типы",technical:"Технические типы",unavailable:"Выбрано вне этих источников"},m0=Object.keys(Od),Pc=new Intl.Collator("ru",{numeric:!0,sensitivity:"base"});function g0(n,e){const t=e==="predicates",i=n.semantic_registries||{},r=new Map((i.entity_types?.entries||[]).map(l=>[l.type_id,l])),s=t?new Map((i.relation_types?.entries||[]).map(l=>[l.relation_type_id,l])):r,a=t?"predicate_id":"kind_id",o=t?"source_predicate_id":"source_kind_id";return(t?n.predicates:n.node_kinds).map(l=>{const c=(l[t?"relation_type_ids":"type_ids"]||[]).map(g=>s.get(g)),p=c.flatMap(g=>(g?.source_mappings||[]).filter(m=>m[o]===l[a]&&(!t||m.scope==="edge"))),u=[...new Set(p.map(g=>g.source_graph).filter(g=>typeof g=="string"))],d=c.length>0&&c.every(Boolean)&&u.length>0&&!l.mapping_statuses?.includes("unmapped");function h(g){if(!g)return"other";if(!t)return{semantic:"meaning",identity:"identity",navigation:"history",evidence:"evidence",assertion:"evidence",activity:"evidence",literal:"evidence",projection:"technical"}[g.object_role]||"other";const m=g.domain_type_ids||[],A=g.range_type_ids||[];return g.assertion_mode==="derived-projection"||[...m,...A].length>0&&[...m,...A].every(P=>r.get(P)?.object_role==="projection")?"technical":g.parent_relation_type_ids?.includes("tos.relation.responsibility")?"authorship":g.assertion_mode==="structural"?"structure":g.assertion_mode==="reified-claim"||g.parent_relation_type_ids?.includes("tos.relation.claim-structure")?"evidence":g.assertion_mode==="direct"?"meaning":"other"}const _=c.map(h),x=_.find(g=>g!=="technical")||_[0]||"other";return{id:l[a],title:vt(l.display,l[a]),group:x,sources:u,sourceKnown:d,count:l.count||0}})}function _0(n,{sources:e,selected:t=[],query:i="",sort:r="alphabet"}={}){const s=i.trim().toLocaleLowerCase("ru"),a=new Set(t),o=new Map,l=[...n];for(const c of a)n.some(p=>p.id===c)||l.push({id:c,title:c,group:"other",sources:[],sourceKnown:!1,count:0});for(const c of l){const p=!c.sourceKnown||c.sources.some(d=>e.includes(d));if(!p&&!a.has(c.id)||s&&!(c.title+" "+c.id).toLocaleLowerCase("ru").includes(s))continue;const u=p?c.group:"unavailable";o.has(u)||o.set(u,[]),o.get(u).push({...c,selected:a.has(c.id),available:p})}return m0.filter(c=>o.has(c)).map(c=>({key:c,title:Od[c],items:o.get(c).sort((p,u)=>Number(u.selected)-Number(p.selected)||(r==="frequency"?u.count-p.count:0)||Pc.compare(p.title,u.title)||Pc.compare(p.id,u.id))}))}const et=(n,e="",t="")=>{const i=document.createElement(n);return i.textContent=e,i.className=t,i},gi=(n,e,t="sc-builder-button")=>{const i=et("button",n,t);return i.type="button",i.addEventListener("click",e),i},v0=new Intl.PluralRules("ru"),is=(n,e,t,i)=>`${n} ${{one:e,few:t,many:i}[v0.select(n)]||i}`,x0={philosophy:"Философский атлас",canon:"Канон","candidate-intake":"Исследовательские кандидаты","source-navigation":"Произведения и источники","source-claims":"Утверждения источников","semantic-interchange":"Понятия и типы",repository:"Карта проекта"};function y0(n,e,t,{onUserAction:i}){const r=new or,s=new Hr;let a=null,o=null,l=null,c=null,p=null,u=null,d=!1,h="",_="",x=!1,g=!1,m=[],A=null,P=0,M=!1,I="",T=null,D=!1,v=null;const S=new Map,w=new Map,L=et("section","","sc-panel sc-builder");L.hidden=!0,L.setAttribute("aria-label","Конструктор линз"),L.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ОПТИКА МЫСЛИ</span><button class="sc-icon sc-builder-close" type="button" aria-label="Закрыть конструктор"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-builder-heading"><span aria-hidden="true">◈</span><div><h3>Собрать линзу</h3><p>Настройки сразу меняют пространство.</p></div></div><div class="sc-builder-body"></div><div class="sc-builder-result" aria-live="polite"></div><div class="sc-builder-status" role="status"></div><div class="sc-builder-footer"></div>',n.append(L);const V=L.querySelector(".sc-builder-body"),Z=L.querySelector(".sc-builder-result"),k=L.querySelector(".sc-builder-status"),z=L.querySelector(".sc-builder-footer"),C=gi("Конструктор линз",()=>{i(),de()},"sc-builder-open");n.querySelector(".sc-lenses").append(C);const R=n.querySelector(".sc-lenses-open"),G=()=>{P++,clearTimeout(T),T=null,D=!1,s.cancelAll(),d=!1};t.register("builder",L,G);function Y(){t.close("builder"),(A?.isConnected&&!A.closest("[hidden]")?A:R).focus()}L.querySelector(".sc-builder-close").addEventListener("click",()=>{i(),Y()}),L.addEventListener("keydown",ve=>{ve.key==="Escape"&&(ve.stopPropagation(),ve.preventDefault(),i(),Y())});function ne(){p=e.port.packet,u=p,c=p?e.port.captureView():null,l=null,x=!1,o=null,M=!1,v=null,S.clear()}async function de(){A=document.activeElement,(M||!o||e.port.packet!==u)&&ne(),t.open("builder"),h="",I="";try{m=Vc(localStorage),_=""}catch(ve){m=[],_=ve.message||"Локальное хранилище недоступно."}if(a&&a.catalog.source_revision===e.port.packet?.source_revision){ce(),$();return}await J()}function ce(){o||(o=structuredClone(ha(e.port.packet)||nu(p,a)));const ve=e.port.selection.nodeId;o.scope==="area"&&ve&&p?.nodes.some(F=>F.id===ve)&&(o.focusId=ve)}async function J(){i(),G();const ve=P;d=!0,a=null,I="",$();try{const F=await s.run("catalog",K=>il(r,K));if(!F.current||ve!==P||L.hidden)return;a=F.value,ce(),h=""}catch(F){ve===P&&(h=F.message||"Не удалось загрузить словарь данных.")}finally{ve===P&&(d=!1,$())}}function ue(){i(),G(),h="",I="",l=null,x=!1,v=null,a&&!M&&(D=!0,T=setTimeout(()=>{T=null,_e()},400)),se()}function me(ve,F){const K=et("label","","sc-builder-field");return K.append(et("span",ve),F),K}function B(ve,F,K){K.some(([xe])=>String(xe)===String(o[F]))||(K=[...K,[o[F],String(o[F])]]);const ae=et("select");for(const[xe,Ae]of K){const be=et("option",Ae);be.value=String(xe),ae.append(be)}return ae.value=String(o[F]),ae.addEventListener("change",()=>{o[F]=["limit","depth"].includes(F)?Number(ae.value):ae.value,ue(),F==="scope"&&$()}),me(ve,ae)}function oe(ve,F){const K=S.get(F)||{open:!1,query:"",sort:"alphabet",expanded:new Map,limits:new Map};S.set(F,K);const ae=et("details","","sc-builder-choices"),xe=et("summary"),Ae=et("div","","sc-builder-options"),be=et("input");ae.open=K.open,ae.addEventListener("toggle",()=>K.open=ae.open),be.type="search",be.placeholder="Найти в списке…",be.setAttribute("aria-label","Найти: "+ve),be.value=K.query;const ke=et("select");ke.setAttribute("aria-label","Порядок: "+ve);for(const[$e,E]of[["alphabet","По алфавиту"],["frequency","Сначала частые"]]){const f=et("option",E);f.value=$e,ke.append(f)}ke.value=K.sort;const Xe=g0(a.catalog,F),Ze=new Map;for(const $e of Xe)Ze.set($e.title,(Ze.get($e.title)||0)+1);const Ne=()=>xe.textContent=ve+" · "+(o[F].length?o[F].length+" выбрано":"любые");Ne();function N(){const $e=Ae.scrollTop;Ae.replaceChildren();const E=_0(Xe,{sources:o.sources,selected:o[F],query:K.query,sort:K.sort});for(const f of E){const U=et("details","","sc-builder-choice-group"),O=et("summary",f.title+" · "+f.items.length);U.dataset.group=f.key,U.open=K.query.trim()!==""||f.items.some(fe=>fe.selected)||K.expanded.get(f.key)===!0,U.addEventListener("toggle",()=>K.expanded.set(f.key,U.open)),U.append(O),f.key==="unavailable"&&U.append(et("p","Эти условия сохраняются. Снимите их или включите соответствующий источник.","sc-builder-note"));const H=K.limits.get(f.key)||40;for(const fe of f.items.slice(0,H)){const he=et("input");he.type="checkbox",he.value=fe.id,he.checked=fe.selected,he.addEventListener("change",()=>{o[F]=he.checked?[...o[F],fe.id]:o[F].filter(Se=>Se!==fe.id),Ne(),ue()});const ee=et("label","","sc-builder-choice"),te=et("span",fe.title);Ze.get(fe.title)>1&&te.append(et("small",fe.id)),ee.append(he,te),U.append(ee)}f.items.length>H&&U.append(gi("Ещё варианты · "+(f.items.length-H),()=>{K.limits.set(f.key,H+40),N()},"sc-builder-link")),Ae.append(U)}E.length||Ae.append(et("p","Нет вариантов для этих источников и поиска.","sc-builder-note")),Ae.scrollTop=$e}be.addEventListener("input",()=>{K.query=be.value,N()}),ke.addEventListener("change",()=>{K.sort=ke.value,N()}),w.set(F,N),N();const at=et("div","","sc-builder-choice-controls");return at.append(be,ke),ae.append(xe,at,Ae,et("p","Типы для выбранных источников. Технические — в отдельной группе.","sc-builder-note"),gi("Сбросить выбор",()=>{o[F]=[],Ne(),N(),ue()},"sc-builder-link")),ae}function $(){const ve=V.scrollTop;if(V.replaceChildren(),w.clear(),!a){V.append(et("p",d?"Загружаю доступные источники и условия…":"Словарь данных пока недоступен.","sc-builder-note")),d||V.append(gi("Повторить загрузку",()=>{J()})),se();return}const F=et("input");if(F.type="text",F.maxLength=64,F.value=o.name,F.addEventListener("input",()=>{o.name=F.value,ue()}),V.append(me("Название линзы",F)),m.length){const be=et("select");be.append(et("option","Выбрать сохранённую…"));for(const ke of m){const Xe=et("option",ke.name);Xe.value=ke.name,be.append(Xe)}be.addEventListener("change",()=>{const ke=m.find(Xe=>Xe.name===be.value);ke&&(o=structuredClone(ke),ue(),$())}),V.append(me("Мои линзы",be))}if(V.append(B("Отправная точка","scope",[["area","Из исходной области"],["focus","От выбранной звезды"],["all","По всему древу"]])),o.scope==="area"&&V.append(et("p",`Исходная область: ${is(o.nodeIds.length,"звезда","звезды","звёзд")}. Фильтры выбирают начало; глубина добавляет окружение.`,"sc-builder-note")),o.scope==="focus"){const be=e.port.node(o.focusId)||p?.nodes.find(Xe=>Xe.id===o.focusId);V.append(et("p",be?vt(be.display.title):o.focusId?"Звезда из сохранённой линзы.":"Сначала выберите звезду в пространстве.","sc-builder-focus"));const ke=e.port.selection.nodeId;ke&&ke!==o.focusId&&V.append(gi("Взять выбранную звезду",()=>{o.focusId=ke,ue(),$()},"sc-builder-link")),V.append(et("p","Центр остаётся в области. Условия ниже выбирают связи вокруг него.","sc-builder-note"))}const K=et("fieldset","","sc-builder-sources");K.append(et("legend","Источники"));for(const be of a.catalog.capabilities.sources){const ke=et("input");ke.type="checkbox",ke.checked=o.sources.includes(be),ke.addEventListener("change",()=>{o.sources=ke.checked?[...o.sources,be]:o.sources.filter(Ze=>Ze!==be),ue();for(const Ze of w.values())Ze()});const Xe=et("label");Xe.append(ke,et("span",x0[be]||be)),K.append(Xe)}if(V.append(K),o.scope!=="focus"){const be=et("input");be.type="search",be.maxLength=256,be.value=o.query,be.placeholder="Имя, произведение, понятие…",be.addEventListener("input",()=>{o.query=be.value,ue()}),V.append(me("Слова в исходных узлах",be)),V.append(oe("Типы узлов","kinds"))}const ae=et("input");ae.type="checkbox",ae.checked=o.relations,ae.addEventListener("change",()=>{o.relations=ae.checked,ue(),$()});const xe=et("label","","sc-builder-toggle");xe.append(ae,et("span","Показывать связи и окружение")),V.append(xe),o.relations&&V.append(oe("Типы связей","predicates"));const Ae=et("div","","sc-builder-grid");o.relations&&Ae.append(B("Глубина","depth",[[0,"Только исходные"],[1,"1 шаг"],[2,"2 шага"],[3,"3 шага"]]),B("Направление","direction",[["either","В обе стороны"],["outgoing","По связям →"],["incoming","Против связей ←"]]),B("Подробность связей","profile",[["all","Все типы, включая текст"],["overview","Обзор без структуры текста"]])),Ae.append(B("Звёзд в области","limit",[[10,"До 10"],[20,"До 20"],[40,"До 40"]])),V.append(Ae),V.append(et("p","Линза меняет способ просмотра. Фильтры не меняют источники, связи или их статус.","sc-builder-note")),se(),V.scrollTop=ve}function se(){if(Z.parentElement!==V&&V.append(Z),Z.replaceChildren(),z.replaceChildren(),k.textContent=M?"Область изменилась. Откройте конструктор заново для нового вида.":h||_||I||(D?"Обновлю пространство…":d?"Обновляю пространство…":x?v&&Object.values(v).every(F=>F.added===0&&F.removed===0)?"Условия применены. Состав этой области совпал.":`В пространстве: ${is(l.nodes.length,"звезда","звезды","звёзд")} · ${is(l.relations.length,"связь","связи","связей")}`:l&&!l.nodes.length?"Ничего не найдено. Предыдущий вид сохранён.":"Измените условие — результат появится в пространстве."),a){if(l){const ae=Hc(l);Z.append(et("strong",`${is(ae.nodes,"звезда","звезды","звёзд")} · ${is(ae.relations,"связь","связи","связей")}`)),Z.append(et("p",`Условиями выбрано: ${ae.matched}. Из показанных узлов добавлено окружением: ${ae.context}.`)),ae.limited&&Z.append(et("p","Результат ограничен размером области. Для другого среза уточните условия.","sc-builder-warning")),v&&Z.append(et("p",`Изменение области: звёзды +${v.nodes.added} / −${v.nodes.removed}; связи +${v.relations.added} / −${v.relations.removed}.`)),ae.nodes||Z.append(et("p","Ничего не найдено. Пространство сохранено. Измените условия."))}else Z.append(et("p","Изменения применяются автоматически; исходный вид можно вернуть."));const F=gi(d?"Обновляю…":"Обновить пространство",()=>{_e()},"sc-builder-primary");F.disabled=d||M,z.append(F);const K=gi("Сохранить линзу",()=>{i();try{Gc(o,a),m=iu(localStorage,o),_="",h="",I="Линза сохранена в этом браузере.",$()}catch(ae){h=ae.message||"Не удалось сохранить линзу.",se()}});K.disabled=d||D||M,z.append(K),h&&z.append(gi("Обновить каталог",()=>{J()},"sc-builder-link"))}const ve=gi("↶ К исходному виду",()=>{i(),G(),g=!0;try{c&&e.port.restoreView(c),u=e.port.packet,x=!1}finally{g=!1}Y()},"sc-builder-link");ve.disabled=!c,z.append(ve),e.invalidate()}async function _e(){if(M)return;i(),G(),e.ui.cancelPending(),h="",I="",x=!1,v=null;const ve=P;d=!0,se();try{nl(o);const F=structuredClone(o),K=await s.run("compile",ae=>rl(r,F,a,ae));if(!K.current||ve!==P||L.hidden)return;if(l=K.value,l.nodes.length){v=ru(e.port.packet,l),g=!0;try{e.port.setGraph(l),u=e.port.packet,x=!0}finally{g=!1}}}catch(F){ve===P&&(h=F.message||"Не удалось собрать линзу.",l=null)}finally{ve===P&&(d=!1,se(),e.invalidate())}}function Me(){g||L.hidden||e.port.packet===u||(G(),l=null,M=!0,se())}return t.configure("builder",{onResume:()=>{a?se():J()}}),addEventListener("pagehide",G),ki(),{selectionChanged:Me}}const Bd=document.getElementById("app");Bd.innerHTML=Zd;const an=Bd.firstElementChild,kd=location.search;let mt,Hn,yl,us,zd,Sl,oi,ca=!1,Lc="";const ro=new or;function jn(){if(Hn?.auxiliarySelection)return Hn.auxiliarySelection;const n=mt?.port.selection;if(!n)return null;const e=n.relationId?mt.port.relation(n.relationId):mt.port.node(n.nodeId);return e?{id:e.id,kind:n.relationId?"edge":"node",semantic_kind:e.kind_id||"relation",label:vt(e.display.title||e.display.label),subtitle:vt(e.display.statement),from_id:e.from_id,to_id:e.to_id,predicate_id:e.predicate_id,source_refs:e.source_refs,authority_posture:e.epistemic?.authority_layer,review_posture:e.epistemic?.review_posture,canon_status:e.epistemic?.canon_status,reroutable:bn(e),path_available:bn(e),evidence_available:!!Fd(e)}:null}function Ml(){if(!mt)return;const n=jn(),e=mt.port.packet,t=new URL(location.href);if(e?.focus?.node_id?t.searchParams.set("focus",e.focus.node_id):e&&t.searchParams.delete("focus"),e){const r=ha(e);r?t.searchParams.set("lens",nl(r)):t.searchParams.delete("lens")}n&&!Hn?.auxiliarySelection?t.searchParams.set("selection",n.id):t.searchParams.delete("selection"),history.replaceState(null,"",t);const i=JSON.stringify([e?.source_revision,e?.fingerprint,e?.focus,n,an.dataset.lens,us?.startId]);i!==Lc&&(Lc=i,ca||oi?.notifyStateChange()),Hn?.selectionChanged(),yl?.selectionChanged(),us?.selectionChanged(),zd?.selectionChanged(),Sl?.selectionChanged()}const _i=n=>{ca=!0;try{return n()}finally{ca=!1,Ml()}};mt=Q_(an,{autoStart:!1,initialFocus:new URLSearchParams(location.search).get("focus")||el,initialLens:new URLSearchParams(location.search).get("lens"),onChange:()=>{Hn?.clearAuxiliarySelection(),Ml()}});const fr=t0(an,mt,{onUserAction:()=>oi?.notifyStateChange()});Hn=e0(an,mt,{selected:jn,panels:fr,onChange:()=>{ca||oi?.notifyStateChange(),Ml()}});yl=o0(an,mt,fr,{selected:jn,onUserAction:()=>oi?.notifyStateChange()});us=p0(an,mt,fr,{selected:jn,commit:_i,onUserAction:()=>oi?.notifyStateChange()});zd=y0(an,mt,fr,{onUserAction:()=>oi?.notifyStateChange()});const Gd=()=>oi?.notifyStateChange();for(const[n,e,t]of[["search","Поиск",".sc-search-open"],["lenses","Линзы",".sc-lenses-open"],["workspace","Исследование",".sc-workspace-open"],["navigation","Маршруты",".sc-navigation-open"]]){const i=an.querySelector(t);fr.addTool(n,{title:e,opener:i,launch:()=>i.click()})}function Ic(){const n=mt.port.selection,e=n.relationId?"relation":"node",t=e==="relation"?mt.port.relation(n.relationId):mt.port.node(n.nodeId);return t?{raw:t,kind:e}:null}for(const[n,e,t,i]of[["builder","Конструктор линз","◈",null],["evidence","Основания","✧","sophia-evidence"],["sources","Источники","◇","sophia-sources"]]){const r=document.createElement("button");r.type="button",r.className="sc-control sc-tool-shortcut",r.setAttribute("aria-label",e);const s=document.createElement("b");s.textContent=t;const a=document.createElement("span");a.textContent=e,r.append(s,a),an.querySelector(".sc-header-actions").append(r);const o=()=>{if(Gd(),n==="builder")an.querySelector(".sc-builder-open").click();else{const l=Ic();l?an.dispatchEvent(new CustomEvent(i,{detail:l})):mt.port.announce("Сначала выберите звезду или связь.")}};r.addEventListener("click",o),fr.addTool(n,{title:e,opener:r,launch:o,available:()=>n==="builder"||!!Ic()})}Sl=Lu(an,mt,fr,{initialRoute:kd,onUserAction:Gd});Du(an,mt);const da={...Hn.handlers,...yl.handlers,"tos.page.inspect-selection":()=>jn(),"tos.page.open-view":async(n,{signal:e})=>{if(mt.ui.cancelPending(),n.mode!=="philosophy"||n.graph_mode&&n.graph_mode!=="nodes"||!["constellations","observatory"].includes(String(n.view_id)))throw new Error("Эта линза открывается в расширенном исследовательском режиме.");const t=await ro.compile(ta(String(n.focus_id||el)),e);return e.throwIfAborted(),_i(()=>mt.port.setGraph(t,{selectFocus:!!n.focus_id})),{view_id:"observatory"}},"tos.page.select":async(n,{signal:e})=>{mt.ui.cancelPending();const t=String(n.item_id||"");if(_i(()=>Hn.chooseGap(t)))return jn();if(mt.port.node(t))return _i(()=>mt.port.selectNode(t)),jn();if(mt.port.relation(t))return _i(()=>mt.port.selectRelation(t)),jn();const i=qs.get(t);if(!i)throw new Error("Выберите объект из текущей области или результатов поиска.");const r=await ro.compile(i.from_id?Fc(i):ta(t),e,Dc);return e.throwIfAborted(),_i(()=>{mt.port.setGraph(r,{selectFocus:!i.from_id}),i.from_id&&mt.port.selectRelation(t,{rememberView:!1})}),jn()},"tos.page.search":async(n,{signal:e})=>{mt.ui.cancelPending();const t=String(n.query||"").trim().slice(0,256),i=await ro.search(t,e);e.throwIfAborted(),mt.openSearch(),mt.ui.cancelSearch(),an.querySelector("#sc-query").value=t;const r=an.querySelector(".sc-search-results");r.replaceChildren(),qs.clear(),Dc=i.source_revision;for(const[s,a]of[["node",i.nodes],["relation",i.relations]])for(const o of a)qs.set(o.id,o),r.append(mt.ui.searchRow(o,s,i.source_revision));return mt.invalidate(),{query:t,result_count:i.counts.matching_nodes+i.counts.matching_relations,results:[...qs.values()].map(s=>({id:s.id,label:vt(s.display.title||s.display.label),kind:s.kind_id||"relation",summary:vt(s.display.summary||s.display.statement)}))}},...us.handlers,"tos.page.clear-focus":()=>_i(()=>(mt.closeInspector(!1,!0),mt.overview(),{cleared:!0}))},qs=new Map;let Dc=null;for(const n of Object.keys(Hn.handlers)){const e=da[n];da[n]=(t,i)=>_i(()=>e(t,i))}oi=$d(()=>{const n=ha(mt.port.packet);return{mode:"philosophy",view_id:"observatory",graph_mode:"nodes",selected:jn(),path_start_node_id:us.startId,active_layers:n?[...n.sources]:["knowledge"],active_predicates:n?n.relations?n.predicates.length?[...n.predicates]:[...new Set(mt.port.packet.relations.map(e=>e.predicate_id))]:[]:["overview"],deep_link:location.href,research_workspace:Hn.workspace.summary()}},da);const S0=new Set(["tos.page.context","tos.page.cancel",...Object.keys(da)]);an.querySelector("#sc-query").addEventListener("input",()=>oi.notifyStateChange());const xa=Kd(oi,document,S0);xa.subscribeStatus(n=>Hn.agentStatus(n));xa.start();window.addEventListener("pagehide",()=>xa.stop());window.addEventListener("pageshow",n=>{n.persisted&&xa.start()});Sl.start().then(n=>{if(n)return;const e=new URLSearchParams(kd).get("selection");if(!e)return;const t=()=>{mt.port.packet&&(i.disconnect(),_i(()=>{mt.port.node(e)?mt.port.selectNode(e):mt.port.relation(e)&&mt.port.selectRelation(e)}))},i=new MutationObserver(t);i.observe(an,{attributes:!0,attributeFilter:["data-graph-revision"]}),t()});
