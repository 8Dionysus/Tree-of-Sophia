import{a as kl,b as kc,e as Gc,c as Vc,d as Hc}from"./webmcp-DXqrHjbE.js";const Wc=`<div id="sophia-gestures" aria-label="Древо Софии — область исследования">
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
</div>`;const Gl={xmlns:"http://www.w3.org/2000/svg",width:24,height:24,viewBox:"0 0 24 24",fill:"none",stroke:"currentColor","stroke-width":2,"stroke-linecap":"round","stroke-linejoin":"round"};const Vl=([i,e,t])=>{const n=document.createElementNS("http://www.w3.org/2000/svg",i);return Object.keys(e).forEach(r=>{n.setAttribute(r,String(e[r]))}),t?.length&&t.forEach(r=>{const s=Vl(r);n.appendChild(s)}),n},Xc=(i,e={})=>{const n={...Gl,...e};return Vl(["svg",n,i])};const qc=i=>{for(const e in i)if(e.startsWith("aria-")||e==="role"||e==="title")return!0;return!1};const Yc=(...i)=>i.filter((e,t,n)=>!!e&&e.trim()!==""&&n.indexOf(e)===t).join(" ").trim();const Kc=i=>i.replace(/^([A-Z])|[\s-_]+(\w)/g,(e,t,n)=>n?n.toUpperCase():t.toLowerCase());const Zc=i=>{const e=Kc(i);return e.charAt(0).toUpperCase()+e.slice(1)};const $c=i=>Array.from(i.attributes).reduce((e,t)=>(e[t.name]=t.value,e),{}),ko=i=>typeof i=="string"?i:!i||!i.class?"":i.class&&typeof i.class=="string"?i.class.split(" "):i.class&&Array.isArray(i.class)?i.class:"",Go=(i,{nameAttr:e,icons:t,attrs:n})=>{const r=i.getAttribute(e);if(r==null)return;const s=Zc(r),a=t[s];if(!a)return console.warn(`${i.outerHTML} icon name was not found in the provided icons object.`);const o=$c(i),c=qc(o)?{}:{"aria-hidden":"true"},l={...Gl,"data-lucide":r,...c,...n,...o},u=ko(o),f=ko(n),d=Yc("lucide",`lucide-${r}`,...u,...f);d&&Object.assign(l,{class:d});const m=Xc(a,l);return i.parentNode?.replaceChild(m,i)};const Jc=[["path",{d:"m12 19-7-7 7-7"}],["path",{d:"M19 12H5"}]];const Qc=[["circle",{cx:"12",cy:"9",r:"1"}],["circle",{cx:"19",cy:"9",r:"1"}],["circle",{cx:"5",cy:"9",r:"1"}],["circle",{cx:"12",cy:"15",r:"1"}],["circle",{cx:"19",cy:"15",r:"1"}],["circle",{cx:"5",cy:"15",r:"1"}]];const jc=[["path",{d:"M13 13.74a2 2 0 0 1-2 0L2.5 8.87a1 1 0 0 1 0-1.74L11 2.26a2 2 0 0 1 2 0l8.5 4.87a1 1 0 0 1 0 1.74z"}],["path",{d:"m20 14.285 1.5.845a1 1 0 0 1 0 1.74L13 21.74a2 2 0 0 1-2 0l-8.5-4.87a1 1 0 0 1 0-1.74l1.5-.845"}]];const ed=[["path",{d:"M5 12h14"}]];const td=[["rect",{x:"5",y:"2",width:"14",height:"20",rx:"7"}],["path",{d:"M12 6v4"}]];const nd=[["path",{d:"M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4"}],["path",{d:"M2 6h4"}],["path",{d:"M2 10h4"}],["path",{d:"M2 14h4"}],["path",{d:"M2 18h4"}],["path",{d:"M21.378 5.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z"}]];const id=[["rect",{x:"14",y:"3",width:"5",height:"18",rx:"1"}],["rect",{x:"5",y:"3",width:"5",height:"18",rx:"1"}]];const rd=[["path",{d:"M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z"}]];const sd=[["path",{d:"M5 12h14"}],["path",{d:"M12 5v14"}]];const ad=[["path",{d:"m21 21-4.34-4.34"}],["circle",{cx:"11",cy:"11",r:"8"}]];const od=[["rect",{width:"20",height:"16",x:"2",y:"4",rx:"2"}],["path",{d:"M2 14h20"}],["path",{d:"M12 20v-6"}]];const ld=[["path",{d:"M18 6 6 18"}],["path",{d:"m6 6 12 12"}]];const Hl=({icons:i={},nameAttr:e="data-lucide",attrs:t={},root:n=document,inTemplates:r}={})=>{if(!Object.values(i).length)throw new Error(`Please provide an icons object.
If you want to use all the icons you can import it like:
 \`import { createIcons, icons } from 'lucide';
lucide.createIcons({icons});\``);if(typeof n>"u")throw new Error("`createIcons()` only works in a browser environment.");if(Array.from(n.querySelectorAll(`[${e}]`)).forEach(a=>Go(a,{nameAttr:e,icons:i,attrs:t})),r&&Array.from(n.querySelectorAll("template")).forEach(o=>Hl({icons:i,nameAttr:e,attrs:t,root:o.content,inTemplates:r})),e==="data-lucide"){const a=n.querySelectorAll("[icon-name]");a.length>0&&(console.warn("[Lucide] Some icons were found with the now deprecated icon-name attribute. These will still be replaced for backwards compatibility, but will no longer be supported in v1.0 and you should switch to data-lucide"),Array.from(a).forEach(o=>Go(o,{nameAttr:"icon-name",icons:i,attrs:t})))}};function xs(){Hl({icons:{Search:ad,Layers2:jc,ArrowLeft:Jc,GripHorizontal:Qc,X:ld,Minus:ed,Plus:sd,Pause:id,Play:rd,Touchpad:od,Mouse:td,NotebookPen:nd}})}const vo="tos.work.friedrich-nietzsche.also-sprach-zarathustra",Ea=Object.freeze({nodes:40,relations:80});class ln extends Error{}class Mo extends Error{constructor(){super("Данные изменились. Обновите область, чтобы продолжить.")}}class Hs extends Error{constructor(e,t){super(t),this.status=e}}function Nt(i,e=""){return[i?.ru,i?.default,i?.original,i?.en].find(t=>typeof t=="string"&&t.trim())||e}function Ir(i,{depth:e=1}={}){return{schema_version:"tos_lens_spec_v1",lens_id:"sophia-observatory-focus",language:"ru",detail:"compact",seed:{focus_node_id:i},node_query:{enabled:!1},traversal:{depth:e,direction:"either",profile:"overview"},limits:{...Ea,groups:8}}}function Wl(i){const e=Ir(i.from_id,{depth:0});return e.node_query={enabled:!0,filters:[{field:"id",op:"in",value:[i.from_id,i.to_id]}]},e.traversal.profile="all",e.relation_query={filters:[{field:"id",op:"eq",value:i.id}]},e.limits={nodes:2,relations:1,groups:2},e}function Ta(i,e){if(!/^[a-f0-9]{64}$/.test(i?.source_revision||""))throw new ln("Ответ не содержит версию данных.");if(e&&i.source_revision!==e)throw new Mo;return i}function dr(i,e){if(!Array.isArray(i))throw new ln("Неверный список объектов.");const t=new Set;for(const n of i){if(!n||typeof n.id!="string"||!n.id||t.has(n.id)||!n.display||!Nt(e==="node"?n.display.title:n.display.label)||!/^[a-f0-9]{64}$/.test(n.content_revision||"")||!Array.isArray(n.source_refs)||!n.source_refs.length||n.source_refs.some(r=>typeof r!="string"||!r))throw new ln("Неполный или повторяющийся объект.");t.add(n.id)}return t}function Xl(i,e=null){if(Ta(i,e),i.schema!=="tos_lens_result_v1"||i.authority_boundary?.is_source!==!1||i.authority_boundary?.is_canon!==!1||i.authority_boundary?.writes_to_tree!==!1)throw new ln("Неподдерживаемый контракт области.");if(!Array.isArray(i.nodes)||!Array.isArray(i.relations)||i.nodes.length>Ea.nodes||i.relations.length>Ea.relations)throw new ln("Область превышает бюджет отображения.");const t=dr(i.nodes,"node");if(dr(i.relations,"relation"),i.relations.some(n=>!t.has(n.from_id)||!t.has(n.to_id)))throw new ln("Связь не содержит оба конца в области.");if(i.focus&&!t.has(i.focus.node_id))throw new ln("Центр отсутствует в области.");return i}class ql{constructor(){this.slots=new Map}cancel(e){this.slots.get(e)?.abort(),this.slots.delete(e)}cancelAll(){for(const e of this.slots.keys())this.cancel(e)}async run(e,t){this.cancel(e);const n=new AbortController;this.slots.set(e,n);try{const r=await t(n.signal);return this.slots.get(e)===n?{current:!0,value:r}:{current:!1}}catch(r){if(this.slots.get(e)!==n||n.signal.aborted)return{current:!1};throw r}finally{this.slots.get(e)===n&&this.slots.delete(e)}}}class Yl{constructor({fetcher:e=globalThis.fetch.bind(globalThis),base:t="/api/knowledge",timeoutMs:n=6e4}={}){this.fetcher=e,this.base=t,this.timeoutMs=n}async request(e,{signal:t,body:n}={}){const r=new AbortController;let s=!1;const a=()=>r.abort(t.reason);t?.aborted?a():t?.addEventListener("abort",a,{once:!0});const o=setTimeout(()=>{s=!0,r.abort()},this.timeoutMs);try{const c=await this.fetcher(this.base+e,{signal:r.signal,method:n?"POST":"GET",headers:n?{"Content-Type":"application/json"}:{},...n?{body:JSON.stringify(n)}:{}});if(!c.ok)throw c.status===409?new Mo:new Hs(c.status,{400:"Запрос не удалось исполнить.",404:"Объект больше не доступен.",410:"Срок сохранённого обхода истёк.",413:"Область слишком велика. Выберите более узкий центр.",503:"Этот способ просмотра пока не доступен."}[c.status]||"Не удалось получить данные. Попробуйте ещё раз.");const l=await c.json();if(!l||typeof l!="object")throw new ln("Неверный ответ сервера.");return l}catch(c){throw s?new Hs(504,"Сервер отвечает дольше обычного. Попробуйте ещё раз."):!r.signal.aborted&&(c instanceof TypeError||c?.name==="NetworkError")?new Hs(0,"Нет связи с данными. Проверьте соединение и повторите запрос."):c instanceof SyntaxError?new ln("Сервер вернул нечитаемый ответ. Повторите запрос."):c}finally{clearTimeout(o),t?.removeEventListener("abort",a)}}async search(e,t,n=0){const r=Ta(await this.request("/search?"+new URLSearchParams({query:e,limit:6,offset:n}),{signal:t}));if(r.schema!=="tos_knowledge_search_v1"||r.nodes?.length>6||r.relations?.length>6)throw new ln("Неподдерживаемый ответ поиска.");return dr(r.nodes,"node"),dr(r.relations,"relation"),r}async compile(e,t,n=null){return Xl(await this.request("/lenses/compile",{signal:t,body:e}),n)}async inspect(e,t,n,r){const s=Ta(await this.request("/"+(e==="node"?"nodes/":"relations/")+encodeURIComponent(t)+(e==="node"?"?relation_limit=0":""),{signal:n}),r);if(s.schema!==(e==="node"?"tos_knowledge_node_packet_v1":"tos_knowledge_relation_packet_v1"))throw new ln("Неверная карточка.");dr(s.matches,e);const a=s.matches.find(o=>o.id===t);if(!a)throw new ln("Не найден точный идентификатор карточки.");if(e==="relation"){const o=dr(s.endpoints,"node");if(!o.has(a.from_id)||!o.has(a.to_id))throw new ln("Неполные концы связи.")}return{packet:s,match:a}}capabilities(e){return this.request("/explore/capabilities",{signal:e})}}const cd=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];function dd(i){let e=2166136261;for(const t of i)e=Math.imul(e^t.codePointAt(0),16777619);return e>>>0}function ud(i,e=[]){Xl(i);const t=new Map(e.map(l=>[l.id,l])),n=new Map;for(const l of i.relations)n.set(l.from_id,(n.get(l.from_id)||0)+1),n.set(l.to_id,(n.get(l.to_id)||0)+1);const r=i.focus?.node_id,s=i.nodes.slice().sort((l,u)=>(u.id===r)-(l.id===r)||(n.get(u.id)||0)-(n.get(l.id)||0)||l.id.localeCompare(u.id,"en")),a=new Set(i.nodes.map(l=>l.id)),o=new Set(e.filter(l=>a.has(l.id)).map(l=>l.slot));let c=0;return s.map((l,u)=>{const f=t.get(l.id);for(;o.has(c);)c++;const d=f?.slot??c++;o.add(d);const m=Nt(l.display.title,l.id),x=m.length>46?m.slice(0,43)+"…":m,S=dd(l.id),g=d*2.399963229728653,h=cd[d]?.slice()||[Math.cos(g)*(260+d%4*58),Math.sin(g)*(160+d%3*47),-480+S%740];return{id:l.id,raw:l,name:x,fullName:m,original:l.display.title.original||l.display.title.en||"",kind:Nt(l.display.kind_label,l.kind_id),description:Nt(l.display.summary),main:u<8,above:u%3===1,group:l.id===r?1:l.kind_id==="agent"?0:l.kind_id==="expression"?2:1,slot:d,p:f?.p?.slice()||h,sourcePosition:f?.sourcePosition?.slice()||h.slice(),volumeZ:f?.volumeZ??h[2],pos:f?.pos?.slice()||h.slice(),target:f?.target?.slice()||h.slice()}})}function hd(i,e,{client:t=new Yl,initialFocus:n=vo}={}){const r=b=>i.querySelector(b),s=new ql,a=new Map;let o=0,c=null,l=0;const u=(b,P,W)=>{const N=document.createElement(b);return N.className=P,N.textContent=W,N};function f(b,P,W="sc-neighbor"){const N=u("button",W,b);return N.type="button",N.addEventListener("click",P),N}function d(b,P=null){r(".sc-data-notice").hidden=!b,r(".sc-data-notice span").textContent=b,r(".sc-retry").hidden=!P,c=P}r(".sc-retry").addEventListener("click",()=>c?.());function m(){s.cancel("inspect")}function x(){clearTimeout(o),s.cancel("search")}function S(){clearTimeout(o),s.cancelAll(),d("")}function g(){s.cancel("scene"),m(),d(""),e.packet&&(i.dataset.dataState="ready")}function h(b,P){d(b.message||"Связь с данными прервалась.",P),e.announce(b.message)}function R(b,P){a.delete(b),a.set(b,P),a.size>48&&a.delete(a.keys().next().value)}async function C(b,{expected:P=null,initial:W=!1,depth:N=1,selectFocus:D=!0}={}){const G=Ir(b,{depth:N});d("Получаю окрестность…"),i.dataset.dataState="loading";try{const H=await s.run("scene",ne=>t.compile(G,ne,P));if(!H.current)return;e.setGraph(H.value,{initial:W,selectFocus:D}),i.dataset.dataState="ready",d(""),D&&r("#so-about-tab").focus(),e.announce("Область загружена. Узлов: "+H.value.nodes.length+". Связей: "+H.value.relations.length+".")}catch(H){i.dataset.dataState="error",h(H,()=>C(b,{initial:W,depth:N,selectFocus:D}))}}function y(b,P){if(P===e.packet?.source_revision&&e.node(b)){e.selectNode(b),r("#so-about-tab").focus();return}C(b,{expected:P,selectFocus:!0})}async function w(b,P){if(P===e.packet?.source_revision&&e.relation(b.id)){e.selectRelation(b.id);return}d("Открываю отношение…"),i.dataset.dataState="loading";try{const W=await s.run("scene",async N=>{const{match:D}=await t.inspect("relation",b.id,N,P),G=Wl(D),H=await t.compile(G,N,P);if(!H.relations.some(ne=>ne.id===D.id))throw new ln("Выбранное отношение отсутствует в области.");return{packet:H}});if(!W.current)return;e.setGraph(W.value.packet),e.selectRelation(b.id,{rememberView:!1}),i.dataset.dataState="ready",d("")}catch(W){i.dataset.dataState="error",h(W,()=>w(b,null))}}function T(b,P,W){const N=Nt(P==="node"?b.display.title:b.display.label,b.id),D=f("",()=>P==="node"?y(b.id,W):w(b,W),"sc-result"),G=u("span","sc-result-label",N);if(P==="relation"){const H=Nt(b.display.statement,b.from_id+" → "+b.to_id);G.append(u("span","sc-result-detail",H)),D.setAttribute("aria-label",N+" · "+H)}return D.append(G,u("small","",P==="node"?Nt(b.display.kind_label):"Отношение")),D}function I(b,P=0){clearTimeout(o),s.cancel("search"),l=P;const W=b.trim().slice(0,256),N=r(".sc-search-results");if(N.replaceChildren(),!W){N.append(u("div","sc-section-label","В ТЕКУЩЕЙ ОБЛАСТИ"));for(const D of(e.packet?.nodes||[]).filter(G=>G.id===e.packet?.focus?.node_id||G.kind_id==="agent").slice(0,6))N.append(T(D,"node",e.packet.source_revision));N.append(u("div","sc-empty","Введите имя, название или понятие для поиска во всём древе.")),e.cardChanged();return}N.append(u("div","sc-empty","Ищу в древе…")),e.cardChanged(),o=setTimeout(async()=>{try{const D=await s.run("search",ne=>t.search(W,ne,P));if(!D.current||r(".sc-search").hidden)return;const G=D.value;N.replaceChildren(),i.dataset.searchQuery=W,i.dataset.searchRevision=G.source_revision;for(const ne of["node","relation"]){const ce=G[ne==="node"?"nodes":"relations"];if(ce.length){N.append(u("div","sc-section-label",ne==="node"?"УЗЛЫ":"ОТНОШЕНИЯ"));for(const oe of ce)N.append(T(oe,ne,G.source_revision))}}!G.nodes.length&&!G.relations.length&&N.append(u("div","sc-empty","По этому запросу ничего не найдено."));const H=document.createElement("div");H.className="sc-search-pager",P>0&&H.append(f("Ранее",()=>I(W,Math.max(0,P-6)))),Math.max(G.counts.matching_nodes,G.counts.matching_relations)>P+6&&H.append(f("Далее",()=>I(W,P+6))),N.append(H),e.announce("Результаты поиска обновлены."),e.cardChanged()}catch(D){if(r(".sc-search").hidden)return;N.replaceChildren(u("div","sc-empty",D.message),f("Повторить поиск",()=>I(W,l))),e.cardChanged()}},180)}function _(b,P){const W=r(".sc-provenance");W.replaceChildren();const N=P==="node"?b.display.summary_state:b.display.explanation_state,D={authored:"Авторское описание","source-derived":"Описание из источника","metadata-synthesis":"Описание составлено из метаданных",missing:"Описание пока не зафиксировано"};W.append(u("p","sc-description-origin",D[N]||"Происхождение описания не указано"));const G=P==="node"?b.display.summary:b.display.explanation;!G?.ru&&G?.default&&W.append(u("p","sc-description-origin","Показан исходный язык описания."));const H=document.createElement("details");H.className="sc-source-details",H.append(u("summary","",`Источники и статус · ${b.source_refs.length}`));const ne=b.epistemic||{};for(const[ce,oe]of[["Слой",ne.authority_layer],["Рассмотрение",ne.review_posture],["Канон",ne.canon_status]])H.append(u("p","sc-source-status",ce+": "+(!oe||oe==="not-recorded"?"не указан":oe)));for(const ce of b.source_refs)if(/^https?:\/\//i.test(ce))try{const oe=new URL(ce),_e=u("a","sc-source-ref",oe.hostname+oe.pathname);_e.href=oe.href,_e.target="_blank",_e.rel="noreferrer noopener",H.append(_e)}catch{H.append(u("span","sc-source-ref",ce))}else H.append(u("span","sc-source-ref",ce));H.addEventListener("toggle",()=>e.cardChanged()),W.append(H)}function A(b){return Nt(e.node(b)?.display?.title,b)}function k(b,P,W=[]){r(".sc-node-title").textContent=Nt(b==="node"?P.display.title:P.display.label,P.id),r(".sc-node-original").textContent=b==="node"?P.display.title.original||P.display.title.en||"":Nt(P.display.statement),r(".sc-kind").textContent=b==="node"?Nt(P.display.kind_label,P.kind_id).toUpperCase():"ОТНОШЕНИЕ",r(".sc-description").textContent=Nt(b==="node"?P.display.summary:P.display.explanation,"Описание пока не зафиксировано."),r(".sc-inspector").setAttribute("aria-label",b==="node"?"Выбранный узел":"Выбранное отношение"),i.dataset.inspectorKind=b,i.dataset.inspectorId=P.id;const N=b==="node"?e.neighbors(P.id):[P];r("#so-relations-tab").firstChild.textContent=b==="node"?"Связи ":"Участники ",r(".sc-neighbor-count").textContent=String(b==="node"?N.length:new Set([P.from_id,P.to_id]).size);const D=r(".sc-neighbors");if(D.replaceChildren(),b==="node"){for(const G of N){const H=G.from_id===P.id,ne=H?G.to_id:G.from_id,ce=H?Nt(G.display.label):Nt(G.display.inverse_label)||"← "+Nt(G.display.label),oe=f("",()=>e.selectRelation(G.id),"sc-neighbor sc-relation-row");oe.append(u("small","",ce),u("span","",A(ne))),D.append(oe)}N.length||D.append(u("p","sc-empty","В этой области связи не показаны."))}else for(const[G,H]of[["От",P.from_id],["К",P.to_id]]){const ne=W.find(oe=>oe.id===H)||e.node(H),ce=f(G+": "+Nt(ne?.display.title,H),()=>y(H,e.packet.source_revision),"sc-neighbor sc-relation-row");D.append(ce)}_(P,b),r(".sc-provenance").append(f("Открыть источники",()=>i.dispatchEvent(new CustomEvent("sophia-sources",{detail:{raw:P,kind:b}})))),e.cardChanged()}async function F(b,P){m();const W=e.packet?.source_revision;if(!W)return;k(b,P),i.dataset.inspectorState="loading";const N=[W,b,P.id,P.content_revision].join("|");if(a.has(N)){const D=a.get(N);k(b,D.match,D.packet.endpoints),i.dataset.inspectorState="ready";return}try{const D=await s.run("inspect",G=>t.inspect(b,P.id,G,W));if(!D.current)return;R(N,D.value),k(b,D.value.match,D.value.packet.endpoints),i.dataset.inspectorState="ready"}catch(D){i.dataset.inspectorState="error",r(".sc-provenance").prepend(u("p","sc-inspection-error",D.message)),D instanceof Mo?h(D,()=>C(e.packet.focus?.node_id||P.id,{selectFocus:!1})):r(".sc-provenance").prepend(f("Загрузить карточку ещё раз",()=>F(b,P))),e.cardChanged()}}return r(".sc-open-neighborhood").addEventListener("click",()=>{const b=e.selection.nodeId;b&&C(b,{expected:e.packet.source_revision,depth:1})}),addEventListener("pagehide",S),{loadFocus:C,chooseNode:y,chooseRelation:w,searchRow:T,search:I,showCard:F,cancelSearch:x,cancelInspector:m,cancelPending:S,willSelect:g,async start(){C(n,{initial:!0,selectFocus:!1});try{const b=await s.run("capabilities",P=>t.capabilities(P));b.current&&(i.dataset.explorationAvailable=String(b.value.available===!0))}catch{i.dataset.explorationAvailable="false"}}}}const So="185",fd=0,Vo=1,pd=2,vs=1,md=2,Lr=3,yi=0,cn=1,ni=2,si=0,ur=1,Ho=2,Wo=3,Xo=4,Kl=5,vi=100,gd=101,_d=102,xd=103,vd=104,Md=200,Aa=201,Sd=202,Zl=203,wa=204,Ur=205,yd=206,bd=207,Ed=208,Td=209,Ad=210,wd=211,Rd=212,Cd=213,Pd=214,Ra=0,Ca=1,Pa=2,pr=3,La=4,Da=5,Ia=6,Ua=7,$l=0,Ld=1,Dd=2,Pn=0,Jl=1,Ql=2,jl=3,ec=4,tc=5,nc=6,ic=7,rc=300,Oi=301,mr=302,Ws=303,Xs=304,Us=306,Na=1e3,ri=1001,Fa=1002,Jt=1003,Id=1004,Xr=1005,Qt=1006,qs=1007,Ni=1008,yn=1009,sc=1010,ac=1011,Nr=1012,yo=1013,Xn=1014,Vn=1015,li=1016,bo=1017,Eo=1018,Fr=1020,oc=35902,lc=35899,cc=1021,dc=1022,Cn=1023,ci=1026,Fi=1027,uc=1028,To=1029,Bi=1030,Ao=1031,wo=1033,Ms=33776,Ss=33777,ys=33778,bs=33779,Oa=35840,Ba=35841,za=35842,ka=35843,Ga=36196,Va=37492,Ha=37496,Wa=37488,Xa=37489,Ts=37490,qa=37491,Ya=37808,Ka=37809,Za=37810,$a=37811,Ja=37812,Qa=37813,ja=37814,eo=37815,to=37816,no=37817,io=37818,ro=37819,so=37820,ao=37821,oo=36492,lo=36494,co=36495,uo=36283,ho=36284,As=36285,fo=36286,Ud=3200,qo=0,Nd=1,ii="",Sn="srgb",Or="srgb-linear",ws="linear",Ct="srgb",$i=7680,Yo=519,Fd=512,Od=513,Bd=514,Ro=515,zd=516,kd=517,Co=518,Gd=519,po=35044,Vd=35048,mo="300 es",Hn=2e3,Rs=2001;function Hd(i){for(let e=i.length-1;e>=0;--e)if(i[e]>=65535)return!0;return!1}function Cs(i){return document.createElementNS("http://www.w3.org/1999/xhtml",i)}function Wd(){const i=Cs("canvas");return i.style.display="block",i}const Ko={};function Ps(...i){const e="THREE."+i.shift();console.log(e,...i)}function hc(i){const e=i[0];if(typeof e=="string"&&e.startsWith("TSL:")){const t=i[1];t&&t.isStackTrace?i[0]+=" "+t.getLocation():i[1]='Stack trace not available. Enable "THREE.Node.captureStackTrace" to capture stack traces.'}return i}function je(...i){i=hc(i);const e="THREE."+i.shift();{const t=i[0];t&&t.isStackTrace?console.warn(t.getError(e)):console.warn(e,...i)}}function _t(...i){i=hc(i);const e="THREE."+i.shift();{const t=i[0];t&&t.isStackTrace?console.error(t.getError(e)):console.error(e,...i)}}function hr(...i){const e=i.join(" ");e in Ko||(Ko[e]=!0,je(...i))}function Xd(i,e,t){return new Promise(function(n,r){function s(){switch(i.clientWaitSync(e,i.SYNC_FLUSH_COMMANDS_BIT,0)){case i.WAIT_FAILED:r();break;case i.TIMEOUT_EXPIRED:setTimeout(s,t);break;default:n()}}setTimeout(s,t)})}const qd={[Ra]:Ca,[Pa]:Ia,[La]:Ua,[pr]:Da,[Ca]:Ra,[Ia]:Pa,[Ua]:La,[Da]:pr};class Gi{addEventListener(e,t){this._listeners===void 0&&(this._listeners={});const n=this._listeners;n[e]===void 0&&(n[e]=[]),n[e].indexOf(t)===-1&&n[e].push(t)}hasEventListener(e,t){const n=this._listeners;return n===void 0?!1:n[e]!==void 0&&n[e].indexOf(t)!==-1}removeEventListener(e,t){const n=this._listeners;if(n===void 0)return;const r=n[e];if(r!==void 0){const s=r.indexOf(t);s!==-1&&r.splice(s,1)}}dispatchEvent(e){const t=this._listeners;if(t===void 0)return;const n=t[e.type];if(n!==void 0){e.target=this;const r=n.slice(0);for(let s=0,a=r.length;s<a;s++)r[s].call(this,e);e.target=null}}}const jt=["00","01","02","03","04","05","06","07","08","09","0a","0b","0c","0d","0e","0f","10","11","12","13","14","15","16","17","18","19","1a","1b","1c","1d","1e","1f","20","21","22","23","24","25","26","27","28","29","2a","2b","2c","2d","2e","2f","30","31","32","33","34","35","36","37","38","39","3a","3b","3c","3d","3e","3f","40","41","42","43","44","45","46","47","48","49","4a","4b","4c","4d","4e","4f","50","51","52","53","54","55","56","57","58","59","5a","5b","5c","5d","5e","5f","60","61","62","63","64","65","66","67","68","69","6a","6b","6c","6d","6e","6f","70","71","72","73","74","75","76","77","78","79","7a","7b","7c","7d","7e","7f","80","81","82","83","84","85","86","87","88","89","8a","8b","8c","8d","8e","8f","90","91","92","93","94","95","96","97","98","99","9a","9b","9c","9d","9e","9f","a0","a1","a2","a3","a4","a5","a6","a7","a8","a9","aa","ab","ac","ad","ae","af","b0","b1","b2","b3","b4","b5","b6","b7","b8","b9","ba","bb","bc","bd","be","bf","c0","c1","c2","c3","c4","c5","c6","c7","c8","c9","ca","cb","cc","cd","ce","cf","d0","d1","d2","d3","d4","d5","d6","d7","d8","d9","da","db","dc","dd","de","df","e0","e1","e2","e3","e4","e5","e6","e7","e8","e9","ea","eb","ec","ed","ee","ef","f0","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff"],Ys=Math.PI/180,go=180/Math.PI;function Si(){const i=Math.random()*4294967295|0,e=Math.random()*4294967295|0,t=Math.random()*4294967295|0,n=Math.random()*4294967295|0;return(jt[i&255]+jt[i>>8&255]+jt[i>>16&255]+jt[i>>24&255]+"-"+jt[e&255]+jt[e>>8&255]+"-"+jt[e>>16&15|64]+jt[e>>24&255]+"-"+jt[t&63|128]+jt[t>>8&255]+"-"+jt[t>>16&255]+jt[t>>24&255]+jt[n&255]+jt[n>>8&255]+jt[n>>16&255]+jt[n>>24&255]).toLowerCase()}function gt(i,e,t){return Math.max(e,Math.min(t,i))}function Yd(i,e){return(i%e+e)%e}function Ks(i,e,t){return(1-t)*i+t*e}function Gn(i,e){switch(e.constructor){case Float32Array:return i;case Uint32Array:return i/4294967295;case Uint16Array:return i/65535;case Uint8Array:return i/255;case Int32Array:return Math.max(i/2147483647,-1);case Int16Array:return Math.max(i/32767,-1);case Int8Array:return Math.max(i/127,-1);default:throw new Error("THREE.MathUtils: Invalid component type.")}}function Pt(i,e){switch(e.constructor){case Float32Array:return i;case Uint32Array:return Math.round(i*4294967295);case Uint16Array:return Math.round(i*65535);case Uint8Array:return Math.round(i*255);case Int32Array:return Math.round(i*2147483647);case Int16Array:return Math.round(i*32767);case Int8Array:return Math.round(i*127);default:throw new Error("THREE.MathUtils: Invalid component type.")}}const No=class No{constructor(e=0,t=0){this.x=e,this.y=t}get width(){return this.x}set width(e){this.x=e}get height(){return this.y}set height(e){this.y=e}set(e,t){return this.x=e,this.y=t,this}setScalar(e){return this.x=e,this.y=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;default:throw new Error("THREE.Vector2: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;default:throw new Error("THREE.Vector2: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y)}copy(e){return this.x=e.x,this.y=e.y,this}add(e){return this.x+=e.x,this.y+=e.y,this}addScalar(e){return this.x+=e,this.y+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this}subScalar(e){return this.x-=e,this.y-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this}multiply(e){return this.x*=e.x,this.y*=e.y,this}multiplyScalar(e){return this.x*=e,this.y*=e,this}divide(e){return this.x/=e.x,this.y/=e.y,this}divideScalar(e){return this.multiplyScalar(1/e)}applyMatrix3(e){const t=this.x,n=this.y,r=e.elements;return this.x=r[0]*t+r[3]*n+r[6],this.y=r[1]*t+r[4]*n+r[7],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this}clamp(e,t){return this.x=gt(this.x,e.x,t.x),this.y=gt(this.y,e.y,t.y),this}clampScalar(e,t){return this.x=gt(this.x,e,t),this.y=gt(this.y,e,t),this}clampLength(e,t){const n=this.length();return this.divideScalar(n||1).multiplyScalar(gt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this}negate(){return this.x=-this.x,this.y=-this.y,this}dot(e){return this.x*e.x+this.y*e.y}cross(e){return this.x*e.y-this.y*e.x}lengthSq(){return this.x*this.x+this.y*this.y}length(){return Math.sqrt(this.x*this.x+this.y*this.y)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)}normalize(){return this.divideScalar(this.length()||1)}angle(){return Math.atan2(-this.y,-this.x)+Math.PI}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const n=this.dot(e)/t;return Math.acos(gt(n,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,n=this.y-e.y;return t*t+n*n}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this}equals(e){return e.x===this.x&&e.y===this.y}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this}rotateAround(e,t){const n=Math.cos(t),r=Math.sin(t),s=this.x-e.x,a=this.y-e.y;return this.x=s*n-a*r+e.x,this.y=s*r+a*n+e.y,this}random(){return this.x=Math.random(),this.y=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y}};No.prototype.isVector2=!0;let Mt=No;class xr{constructor(e=0,t=0,n=0,r=1){this.isQuaternion=!0,this._x=e,this._y=t,this._z=n,this._w=r}static slerpFlat(e,t,n,r,s,a,o){let c=n[r+0],l=n[r+1],u=n[r+2],f=n[r+3],d=s[a+0],m=s[a+1],x=s[a+2],S=s[a+3];if(f!==S||c!==d||l!==m||u!==x){let g=c*d+l*m+u*x+f*S;g<0&&(d=-d,m=-m,x=-x,S=-S,g=-g);let h=1-o;if(g<.9995){const R=Math.acos(g),C=Math.sin(R);h=Math.sin(h*R)/C,o=Math.sin(o*R)/C,c=c*h+d*o,l=l*h+m*o,u=u*h+x*o,f=f*h+S*o}else{c=c*h+d*o,l=l*h+m*o,u=u*h+x*o,f=f*h+S*o;const R=1/Math.sqrt(c*c+l*l+u*u+f*f);c*=R,l*=R,u*=R,f*=R}}e[t]=c,e[t+1]=l,e[t+2]=u,e[t+3]=f}static multiplyQuaternionsFlat(e,t,n,r,s,a){const o=n[r],c=n[r+1],l=n[r+2],u=n[r+3],f=s[a],d=s[a+1],m=s[a+2],x=s[a+3];return e[t]=o*x+u*f+c*m-l*d,e[t+1]=c*x+u*d+l*f-o*m,e[t+2]=l*x+u*m+o*d-c*f,e[t+3]=u*x-o*f-c*d-l*m,e}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get w(){return this._w}set w(e){this._w=e,this._onChangeCallback()}set(e,t,n,r){return this._x=e,this._y=t,this._z=n,this._w=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._w)}copy(e){return this._x=e.x,this._y=e.y,this._z=e.z,this._w=e.w,this._onChangeCallback(),this}setFromEuler(e,t=!0){const n=e._x,r=e._y,s=e._z,a=e._order,o=Math.cos,c=Math.sin,l=o(n/2),u=o(r/2),f=o(s/2),d=c(n/2),m=c(r/2),x=c(s/2);switch(a){case"XYZ":this._x=d*u*f+l*m*x,this._y=l*m*f-d*u*x,this._z=l*u*x+d*m*f,this._w=l*u*f-d*m*x;break;case"YXZ":this._x=d*u*f+l*m*x,this._y=l*m*f-d*u*x,this._z=l*u*x-d*m*f,this._w=l*u*f+d*m*x;break;case"ZXY":this._x=d*u*f-l*m*x,this._y=l*m*f+d*u*x,this._z=l*u*x+d*m*f,this._w=l*u*f-d*m*x;break;case"ZYX":this._x=d*u*f-l*m*x,this._y=l*m*f+d*u*x,this._z=l*u*x-d*m*f,this._w=l*u*f+d*m*x;break;case"YZX":this._x=d*u*f+l*m*x,this._y=l*m*f+d*u*x,this._z=l*u*x-d*m*f,this._w=l*u*f-d*m*x;break;case"XZY":this._x=d*u*f-l*m*x,this._y=l*m*f-d*u*x,this._z=l*u*x+d*m*f,this._w=l*u*f+d*m*x;break;default:je("Quaternion: .setFromEuler() encountered an unknown order: "+a)}return t===!0&&this._onChangeCallback(),this}setFromAxisAngle(e,t){const n=t/2,r=Math.sin(n);return this._x=e.x*r,this._y=e.y*r,this._z=e.z*r,this._w=Math.cos(n),this._onChangeCallback(),this}setFromRotationMatrix(e){const t=e.elements,n=t[0],r=t[4],s=t[8],a=t[1],o=t[5],c=t[9],l=t[2],u=t[6],f=t[10],d=n+o+f;if(d>0){const m=.5/Math.sqrt(d+1);this._w=.25/m,this._x=(u-c)*m,this._y=(s-l)*m,this._z=(a-r)*m}else if(n>o&&n>f){const m=2*Math.sqrt(1+n-o-f);this._w=(u-c)/m,this._x=.25*m,this._y=(r+a)/m,this._z=(s+l)/m}else if(o>f){const m=2*Math.sqrt(1+o-n-f);this._w=(s-l)/m,this._x=(r+a)/m,this._y=.25*m,this._z=(c+u)/m}else{const m=2*Math.sqrt(1+f-n-o);this._w=(a-r)/m,this._x=(s+l)/m,this._y=(c+u)/m,this._z=.25*m}return this._onChangeCallback(),this}setFromUnitVectors(e,t){let n=e.dot(t)+1;return n<1e-8?(n=0,Math.abs(e.x)>Math.abs(e.z)?(this._x=-e.y,this._y=e.x,this._z=0,this._w=n):(this._x=0,this._y=-e.z,this._z=e.y,this._w=n)):(this._x=e.y*t.z-e.z*t.y,this._y=e.z*t.x-e.x*t.z,this._z=e.x*t.y-e.y*t.x,this._w=n),this.normalize()}angleTo(e){return 2*Math.acos(Math.abs(gt(this.dot(e),-1,1)))}rotateTowards(e,t){const n=this.angleTo(e);if(n===0)return this;const r=Math.min(1,t/n);return this.slerp(e,r),this}identity(){return this.set(0,0,0,1)}invert(){return this.conjugate()}conjugate(){return this._x*=-1,this._y*=-1,this._z*=-1,this._onChangeCallback(),this}dot(e){return this._x*e._x+this._y*e._y+this._z*e._z+this._w*e._w}lengthSq(){return this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w}length(){return Math.sqrt(this._x*this._x+this._y*this._y+this._z*this._z+this._w*this._w)}normalize(){let e=this.length();return e===0?(this._x=0,this._y=0,this._z=0,this._w=1):(e=1/e,this._x=this._x*e,this._y=this._y*e,this._z=this._z*e,this._w=this._w*e),this._onChangeCallback(),this}multiply(e){return this.multiplyQuaternions(this,e)}premultiply(e){return this.multiplyQuaternions(e,this)}multiplyQuaternions(e,t){const n=e._x,r=e._y,s=e._z,a=e._w,o=t._x,c=t._y,l=t._z,u=t._w;return this._x=n*u+a*o+r*l-s*c,this._y=r*u+a*c+s*o-n*l,this._z=s*u+a*l+n*c-r*o,this._w=a*u-n*o-r*c-s*l,this._onChangeCallback(),this}slerp(e,t){let n=e._x,r=e._y,s=e._z,a=e._w,o=this.dot(e);o<0&&(n=-n,r=-r,s=-s,a=-a,o=-o);let c=1-t;if(o<.9995){const l=Math.acos(o),u=Math.sin(l);c=Math.sin(c*l)/u,t=Math.sin(t*l)/u,this._x=this._x*c+n*t,this._y=this._y*c+r*t,this._z=this._z*c+s*t,this._w=this._w*c+a*t,this._onChangeCallback()}else this._x=this._x*c+n*t,this._y=this._y*c+r*t,this._z=this._z*c+s*t,this._w=this._w*c+a*t,this.normalize();return this}slerpQuaternions(e,t,n){return this.copy(e).slerp(t,n)}random(){const e=2*Math.PI*Math.random(),t=2*Math.PI*Math.random(),n=Math.random(),r=Math.sqrt(1-n),s=Math.sqrt(n);return this.set(r*Math.sin(e),r*Math.cos(e),s*Math.sin(t),s*Math.cos(t))}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._w===this._w}fromArray(e,t=0){return this._x=e[t],this._y=e[t+1],this._z=e[t+2],this._w=e[t+3],this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._w,e}fromBufferAttribute(e,t){return this._x=e.getX(t),this._y=e.getY(t),this._z=e.getZ(t),this._w=e.getW(t),this._onChangeCallback(),this}toJSON(){return this.toArray()}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._w}}const Fo=class Fo{constructor(e=0,t=0,n=0){this.x=e,this.y=t,this.z=n}set(e,t,n){return n===void 0&&(n=this.z),this.x=e,this.y=t,this.z=n,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;default:throw new Error("THREE.Vector3: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;default:throw new Error("THREE.Vector3: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this}multiplyVectors(e,t){return this.x=e.x*t.x,this.y=e.y*t.y,this.z=e.z*t.z,this}applyEuler(e){return this.applyQuaternion(Zo.setFromEuler(e))}applyAxisAngle(e,t){return this.applyQuaternion(Zo.setFromAxisAngle(e,t))}applyMatrix3(e){const t=this.x,n=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[3]*n+s[6]*r,this.y=s[1]*t+s[4]*n+s[7]*r,this.z=s[2]*t+s[5]*n+s[8]*r,this}applyNormalMatrix(e){return this.applyMatrix3(e).normalize()}applyMatrix4(e){const t=this.x,n=this.y,r=this.z,s=e.elements,a=1/(s[3]*t+s[7]*n+s[11]*r+s[15]);return this.x=(s[0]*t+s[4]*n+s[8]*r+s[12])*a,this.y=(s[1]*t+s[5]*n+s[9]*r+s[13])*a,this.z=(s[2]*t+s[6]*n+s[10]*r+s[14])*a,this}applyQuaternion(e){const t=this.x,n=this.y,r=this.z,s=e.x,a=e.y,o=e.z,c=e.w,l=2*(a*r-o*n),u=2*(o*t-s*r),f=2*(s*n-a*t);return this.x=t+c*l+a*f-o*u,this.y=n+c*u+o*l-s*f,this.z=r+c*f+s*u-a*l,this}project(e){return this.applyMatrix4(e.matrixWorldInverse).applyMatrix4(e.projectionMatrix)}unproject(e){return this.applyMatrix4(e.projectionMatrixInverse).applyMatrix4(e.matrixWorld)}transformDirection(e){const t=this.x,n=this.y,r=this.z,s=e.elements;return this.x=s[0]*t+s[4]*n+s[8]*r,this.y=s[1]*t+s[5]*n+s[9]*r,this.z=s[2]*t+s[6]*n+s[10]*r,this.normalize()}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this}divideScalar(e){return this.multiplyScalar(1/e)}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this}clamp(e,t){return this.x=gt(this.x,e.x,t.x),this.y=gt(this.y,e.y,t.y),this.z=gt(this.z,e.z,t.z),this}clampScalar(e,t){return this.x=gt(this.x,e,t),this.y=gt(this.y,e,t),this.z=gt(this.z,e,t),this}clampLength(e,t){const n=this.length();return this.divideScalar(n||1).multiplyScalar(gt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this.z=e.z+(t.z-e.z)*n,this}cross(e){return this.crossVectors(this,e)}crossVectors(e,t){const n=e.x,r=e.y,s=e.z,a=t.x,o=t.y,c=t.z;return this.x=r*c-s*o,this.y=s*a-n*c,this.z=n*o-r*a,this}projectOnVector(e){const t=e.lengthSq();if(t===0)return this.set(0,0,0);const n=e.dot(this)/t;return this.copy(e).multiplyScalar(n)}projectOnPlane(e){return Zs.copy(this).projectOnVector(e),this.sub(Zs)}reflect(e){return this.sub(Zs.copy(e).multiplyScalar(2*this.dot(e)))}angleTo(e){const t=Math.sqrt(this.lengthSq()*e.lengthSq());if(t===0)return Math.PI/2;const n=this.dot(e)/t;return Math.acos(gt(n,-1,1))}distanceTo(e){return Math.sqrt(this.distanceToSquared(e))}distanceToSquared(e){const t=this.x-e.x,n=this.y-e.y,r=this.z-e.z;return t*t+n*n+r*r}manhattanDistanceTo(e){return Math.abs(this.x-e.x)+Math.abs(this.y-e.y)+Math.abs(this.z-e.z)}setFromSpherical(e){return this.setFromSphericalCoords(e.radius,e.phi,e.theta)}setFromSphericalCoords(e,t,n){const r=Math.sin(t)*e;return this.x=r*Math.sin(n),this.y=Math.cos(t)*e,this.z=r*Math.cos(n),this}setFromCylindrical(e){return this.setFromCylindricalCoords(e.radius,e.theta,e.y)}setFromCylindricalCoords(e,t,n){return this.x=e*Math.sin(t),this.y=n,this.z=e*Math.cos(t),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this}setFromMatrixScale(e){const t=this.setFromMatrixColumn(e,0).length(),n=this.setFromMatrixColumn(e,1).length(),r=this.setFromMatrixColumn(e,2).length();return this.x=t,this.y=n,this.z=r,this}setFromMatrixColumn(e,t){return this.fromArray(e.elements,t*4)}setFromMatrix3Column(e,t){return this.fromArray(e.elements,t*3)}setFromEuler(e){return this.x=e._x,this.y=e._y,this.z=e._z,this}setFromColor(e){return this.x=e.r,this.y=e.g,this.z=e.b,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this}randomDirection(){const e=Math.random()*Math.PI*2,t=Math.random()*2-1,n=Math.sqrt(1-t*t);return this.x=n*Math.cos(e),this.y=t,this.z=n*Math.sin(e),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z}};Fo.prototype.isVector3=!0;let K=Fo;const Zs=new K,Zo=new xr,Oo=class Oo{constructor(e,t,n,r,s,a,o,c,l){this.elements=[1,0,0,0,1,0,0,0,1],e!==void 0&&this.set(e,t,n,r,s,a,o,c,l)}set(e,t,n,r,s,a,o,c,l){const u=this.elements;return u[0]=e,u[1]=r,u[2]=o,u[3]=t,u[4]=s,u[5]=c,u[6]=n,u[7]=a,u[8]=l,this}identity(){return this.set(1,0,0,0,1,0,0,0,1),this}copy(e){const t=this.elements,n=e.elements;return t[0]=n[0],t[1]=n[1],t[2]=n[2],t[3]=n[3],t[4]=n[4],t[5]=n[5],t[6]=n[6],t[7]=n[7],t[8]=n[8],this}extractBasis(e,t,n){return e.setFromMatrix3Column(this,0),t.setFromMatrix3Column(this,1),n.setFromMatrix3Column(this,2),this}setFromMatrix4(e){const t=e.elements;return this.set(t[0],t[4],t[8],t[1],t[5],t[9],t[2],t[6],t[10]),this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const n=e.elements,r=t.elements,s=this.elements,a=n[0],o=n[3],c=n[6],l=n[1],u=n[4],f=n[7],d=n[2],m=n[5],x=n[8],S=r[0],g=r[3],h=r[6],R=r[1],C=r[4],y=r[7],w=r[2],T=r[5],I=r[8];return s[0]=a*S+o*R+c*w,s[3]=a*g+o*C+c*T,s[6]=a*h+o*y+c*I,s[1]=l*S+u*R+f*w,s[4]=l*g+u*C+f*T,s[7]=l*h+u*y+f*I,s[2]=d*S+m*R+x*w,s[5]=d*g+m*C+x*T,s[8]=d*h+m*y+x*I,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[3]*=e,t[6]*=e,t[1]*=e,t[4]*=e,t[7]*=e,t[2]*=e,t[5]*=e,t[8]*=e,this}determinant(){const e=this.elements,t=e[0],n=e[1],r=e[2],s=e[3],a=e[4],o=e[5],c=e[6],l=e[7],u=e[8];return t*a*u-t*o*l-n*s*u+n*o*c+r*s*l-r*a*c}invert(){const e=this.elements,t=e[0],n=e[1],r=e[2],s=e[3],a=e[4],o=e[5],c=e[6],l=e[7],u=e[8],f=u*a-o*l,d=o*c-u*s,m=l*s-a*c,x=t*f+n*d+r*m;if(x===0)return this.set(0,0,0,0,0,0,0,0,0);const S=1/x;return e[0]=f*S,e[1]=(r*l-u*n)*S,e[2]=(o*n-r*a)*S,e[3]=d*S,e[4]=(u*t-r*c)*S,e[5]=(r*s-o*t)*S,e[6]=m*S,e[7]=(n*c-l*t)*S,e[8]=(a*t-n*s)*S,this}transpose(){let e;const t=this.elements;return e=t[1],t[1]=t[3],t[3]=e,e=t[2],t[2]=t[6],t[6]=e,e=t[5],t[5]=t[7],t[7]=e,this}getNormalMatrix(e){return this.setFromMatrix4(e).invert().transpose()}transposeIntoArray(e){const t=this.elements;return e[0]=t[0],e[1]=t[3],e[2]=t[6],e[3]=t[1],e[4]=t[4],e[5]=t[7],e[6]=t[2],e[7]=t[5],e[8]=t[8],this}setUvTransform(e,t,n,r,s,a,o){const c=Math.cos(s),l=Math.sin(s);return this.set(n*c,n*l,-n*(c*a+l*o)+a+e,-r*l,r*c,-r*(-l*a+c*o)+o+t,0,0,1),this}scale(e,t){return hr("Matrix3: .scale() is deprecated. Use .makeScale() instead."),this.premultiply($s.makeScale(e,t)),this}rotate(e){return hr("Matrix3: .rotate() is deprecated. Use .makeRotation() instead."),this.premultiply($s.makeRotation(-e)),this}translate(e,t){return hr("Matrix3: .translate() is deprecated. Use .makeTranslation() instead."),this.premultiply($s.makeTranslation(e,t)),this}makeTranslation(e,t){return e.isVector2?this.set(1,0,e.x,0,1,e.y,0,0,1):this.set(1,0,e,0,1,t,0,0,1),this}makeRotation(e){const t=Math.cos(e),n=Math.sin(e);return this.set(t,-n,0,n,t,0,0,0,1),this}makeScale(e,t){return this.set(e,0,0,0,t,0,0,0,1),this}equals(e){const t=this.elements,n=e.elements;for(let r=0;r<9;r++)if(t[r]!==n[r])return!1;return!0}fromArray(e,t=0){for(let n=0;n<9;n++)this.elements[n]=e[n+t];return this}toArray(e=[],t=0){const n=this.elements;return e[t]=n[0],e[t+1]=n[1],e[t+2]=n[2],e[t+3]=n[3],e[t+4]=n[4],e[t+5]=n[5],e[t+6]=n[6],e[t+7]=n[7],e[t+8]=n[8],e}clone(){return new this.constructor().fromArray(this.elements)}};Oo.prototype.isMatrix3=!0;let nt=Oo;const $s=new nt,$o=new nt().set(.4123908,.3575843,.1804808,.212639,.7151687,.0721923,.0193308,.1191948,.9505322),Jo=new nt().set(3.2409699,-1.5373832,-.4986108,-.9692436,1.8759675,.0415551,.0556301,-.203977,1.0569715);function Kd(){const i={enabled:!0,workingColorSpace:Or,spaces:{},convert:function(r,s,a){return this.enabled===!1||s===a||!s||!a||(this.spaces[s].transfer===Ct&&(r.r=ai(r.r),r.g=ai(r.g),r.b=ai(r.b)),this.spaces[s].primaries!==this.spaces[a].primaries&&(r.applyMatrix3(this.spaces[s].toXYZ),r.applyMatrix3(this.spaces[a].fromXYZ)),this.spaces[a].transfer===Ct&&(r.r=fr(r.r),r.g=fr(r.g),r.b=fr(r.b))),r},workingToColorSpace:function(r,s){return this.convert(r,this.workingColorSpace,s)},colorSpaceToWorking:function(r,s){return this.convert(r,s,this.workingColorSpace)},getPrimaries:function(r){return this.spaces[r].primaries},getTransfer:function(r){return r===ii?ws:this.spaces[r].transfer},getToneMappingMode:function(r){return this.spaces[r].outputColorSpaceConfig.toneMappingMode||"standard"},getLuminanceCoefficients:function(r,s=this.workingColorSpace){return r.fromArray(this.spaces[s].luminanceCoefficients)},define:function(r){Object.assign(this.spaces,r)},_getMatrix:function(r,s,a){return r.copy(this.spaces[s].toXYZ).multiply(this.spaces[a].fromXYZ)},_getDrawingBufferColorSpace:function(r){return this.spaces[r].outputColorSpaceConfig.drawingBufferColorSpace},_getUnpackColorSpace:function(r=this.workingColorSpace){return this.spaces[r].workingColorSpaceConfig.unpackColorSpace},fromWorkingColorSpace:function(r,s){return hr("ColorManagement: .fromWorkingColorSpace() has been renamed to .workingToColorSpace()."),i.workingToColorSpace(r,s)},toWorkingColorSpace:function(r,s){return hr("ColorManagement: .toWorkingColorSpace() has been renamed to .colorSpaceToWorking()."),i.colorSpaceToWorking(r,s)}},e=[.64,.33,.3,.6,.15,.06],t=[.2126,.7152,.0722],n=[.3127,.329];return i.define({[Or]:{primaries:e,whitePoint:n,transfer:ws,toXYZ:$o,fromXYZ:Jo,luminanceCoefficients:t,workingColorSpaceConfig:{unpackColorSpace:Sn},outputColorSpaceConfig:{drawingBufferColorSpace:Sn}},[Sn]:{primaries:e,whitePoint:n,transfer:Ct,toXYZ:$o,fromXYZ:Jo,luminanceCoefficients:t,outputColorSpaceConfig:{drawingBufferColorSpace:Sn}}}),i}const mt=Kd();function ai(i){return i<.04045?i*.0773993808:Math.pow(i*.9478672986+.0521327014,2.4)}function fr(i){return i<.0031308?i*12.92:1.055*Math.pow(i,.41666)-.055}let Ji;class Zd{static getDataURL(e,t="image/png"){if(/^data:/i.test(e.src)||typeof HTMLCanvasElement>"u")return e.src;let n;if(e instanceof HTMLCanvasElement)n=e;else{Ji===void 0&&(Ji=Cs("canvas")),Ji.width=e.width,Ji.height=e.height;const r=Ji.getContext("2d");e instanceof ImageData?r.putImageData(e,0,0):r.drawImage(e,0,0,e.width,e.height),n=Ji}return n.toDataURL(t)}static sRGBToLinear(e){if(typeof HTMLImageElement<"u"&&e instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&e instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&e instanceof ImageBitmap){const t=Cs("canvas");t.width=e.width,t.height=e.height;const n=t.getContext("2d");n.drawImage(e,0,0,e.width,e.height);const r=n.getImageData(0,0,e.width,e.height),s=r.data;for(let a=0;a<s.length;a++)s[a]=ai(s[a]/255)*255;return n.putImageData(r,0,0),t}else if(e.data){const t=e.data.slice(0);for(let n=0;n<t.length;n++)t instanceof Uint8Array||t instanceof Uint8ClampedArray?t[n]=Math.floor(ai(t[n]/255)*255):t[n]=ai(t[n]);return{data:t,width:e.width,height:e.height}}else return je("ImageUtils.sRGBToLinear(): Unsupported image type. No color space conversion applied."),e}}let $d=0;class Po{constructor(e=null){this.isSource=!0,Object.defineProperty(this,"id",{value:$d++}),this.uuid=Si(),this.data=e,this.dataReady=!0,this.version=0}getSize(e){const t=this.data;return typeof HTMLVideoElement<"u"&&t instanceof HTMLVideoElement?e.set(t.videoWidth,t.videoHeight,0):typeof VideoFrame<"u"&&t instanceof VideoFrame?e.set(t.displayWidth,t.displayHeight,0):t!==null?e.set(t.width,t.height,t.depth||0):e.set(0,0,0),e}set needsUpdate(e){e===!0&&this.version++}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.images[this.uuid]!==void 0)return e.images[this.uuid];const n={uuid:this.uuid,url:""},r=this.data;if(r!==null){let s;if(Array.isArray(r)){s=[];for(let a=0,o=r.length;a<o;a++)r[a].isDataTexture?s.push(Js(r[a].image)):s.push(Js(r[a]))}else s=Js(r);n.url=s}return t||(e.images[this.uuid]=n),n}}function Js(i){return typeof HTMLImageElement<"u"&&i instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&i instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&i instanceof ImageBitmap?Zd.getDataURL(i):i.data?{data:Array.from(i.data),width:i.width,height:i.height,type:i.data.constructor.name}:(je("Texture: Unable to serialize Texture."),{})}let Jd=0;const Qs=new K;class tn extends Gi{constructor(e=tn.DEFAULT_IMAGE,t=tn.DEFAULT_MAPPING,n=ri,r=ri,s=Qt,a=Ni,o=Cn,c=yn,l=tn.DEFAULT_ANISOTROPY,u=ii){super(),this.isTexture=!0,Object.defineProperty(this,"id",{value:Jd++}),this.uuid=Si(),this.name="",this.source=new Po(e),this.mipmaps=[],this.mapping=t,this.channel=0,this.wrapS=n,this.wrapT=r,this.magFilter=s,this.minFilter=a,this.anisotropy=l,this.format=o,this.internalFormat=null,this.type=c,this.offset=new Mt(0,0),this.repeat=new Mt(1,1),this.center=new Mt(0,0),this.rotation=0,this.matrixAutoUpdate=!0,this.matrix=new nt,this.generateMipmaps=!0,this.premultiplyAlpha=!1,this.flipY=!0,this.unpackAlignment=4,this.colorSpace=u,this.userData={},this.updateRanges=[],this.version=0,this.onUpdate=null,this.renderTarget=null,this.isRenderTargetTexture=!1,this.isArrayTexture=!!(e&&e.depth&&e.depth>1),this.pmremVersion=0,this.normalized=!1}get width(){return this.source.getSize(Qs).x}get height(){return this.source.getSize(Qs).y}get depth(){return this.source.getSize(Qs).z}get image(){return this.source.data}set image(e){this.source.data=e}updateMatrix(){this.matrix.setUvTransform(this.offset.x,this.offset.y,this.repeat.x,this.repeat.y,this.rotation,this.center.x,this.center.y)}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}clone(){return new this.constructor().copy(this)}copy(e){return this.name=e.name,this.source=e.source,this.mipmaps=e.mipmaps.slice(0),this.mapping=e.mapping,this.channel=e.channel,this.wrapS=e.wrapS,this.wrapT=e.wrapT,this.magFilter=e.magFilter,this.minFilter=e.minFilter,this.anisotropy=e.anisotropy,this.format=e.format,this.internalFormat=e.internalFormat,this.type=e.type,this.normalized=e.normalized,this.offset.copy(e.offset),this.repeat.copy(e.repeat),this.center.copy(e.center),this.rotation=e.rotation,this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrix.copy(e.matrix),this.generateMipmaps=e.generateMipmaps,this.premultiplyAlpha=e.premultiplyAlpha,this.flipY=e.flipY,this.unpackAlignment=e.unpackAlignment,this.colorSpace=e.colorSpace,this.renderTarget=e.renderTarget,this.isRenderTargetTexture=e.isRenderTargetTexture,this.isArrayTexture=e.isArrayTexture,this.userData=JSON.parse(JSON.stringify(e.userData)),this.needsUpdate=!0,this}setValues(e){for(const t in e){const n=e[t];if(n===void 0){je(`Texture.setValues(): parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){je(`Texture.setValues(): property '${t}' does not exist.`);continue}r&&n&&r.isVector2&&n.isVector2||r&&n&&r.isVector3&&n.isVector3||r&&n&&r.isMatrix3&&n.isMatrix3?r.copy(n):this[t]=n}}toJSON(e){const t=e===void 0||typeof e=="string";if(!t&&e.textures[this.uuid]!==void 0)return e.textures[this.uuid];const n={metadata:{version:4.7,type:"Texture",generator:"Texture.toJSON"},uuid:this.uuid,name:this.name,image:this.source.toJSON(e).uuid,mapping:this.mapping,channel:this.channel,repeat:[this.repeat.x,this.repeat.y],offset:[this.offset.x,this.offset.y],center:[this.center.x,this.center.y],rotation:this.rotation,wrap:[this.wrapS,this.wrapT],format:this.format,internalFormat:this.internalFormat,type:this.type,normalized:this.normalized,colorSpace:this.colorSpace,minFilter:this.minFilter,magFilter:this.magFilter,anisotropy:this.anisotropy,flipY:this.flipY,generateMipmaps:this.generateMipmaps,premultiplyAlpha:this.premultiplyAlpha,unpackAlignment:this.unpackAlignment};return Object.keys(this.userData).length>0&&(n.userData=this.userData),t||(e.textures[this.uuid]=n),n}dispose(){this.dispatchEvent({type:"dispose"})}transformUv(e){if(this.mapping!==rc)return e;if(e.applyMatrix3(this.matrix),e.x<0||e.x>1)switch(this.wrapS){case Na:e.x=e.x-Math.floor(e.x);break;case ri:e.x=e.x<0?0:1;break;case Fa:Math.abs(Math.floor(e.x)%2)===1?e.x=Math.ceil(e.x)-e.x:e.x=e.x-Math.floor(e.x);break}if(e.y<0||e.y>1)switch(this.wrapT){case Na:e.y=e.y-Math.floor(e.y);break;case ri:e.y=e.y<0?0:1;break;case Fa:Math.abs(Math.floor(e.y)%2)===1?e.y=Math.ceil(e.y)-e.y:e.y=e.y-Math.floor(e.y);break}return this.flipY&&(e.y=1-e.y),e}set needsUpdate(e){e===!0&&(this.version++,this.source.needsUpdate=!0)}set needsPMREMUpdate(e){e===!0&&this.pmremVersion++}}tn.DEFAULT_IMAGE=null;tn.DEFAULT_MAPPING=rc;tn.DEFAULT_ANISOTROPY=1;const Bo=class Bo{constructor(e=0,t=0,n=0,r=1){this.x=e,this.y=t,this.z=n,this.w=r}get width(){return this.z}set width(e){this.z=e}get height(){return this.w}set height(e){this.w=e}set(e,t,n,r){return this.x=e,this.y=t,this.z=n,this.w=r,this}setScalar(e){return this.x=e,this.y=e,this.z=e,this.w=e,this}setX(e){return this.x=e,this}setY(e){return this.y=e,this}setZ(e){return this.z=e,this}setW(e){return this.w=e,this}setComponent(e,t){switch(e){case 0:this.x=t;break;case 1:this.y=t;break;case 2:this.z=t;break;case 3:this.w=t;break;default:throw new Error("THREE.Vector4: index is out of range: "+e)}return this}getComponent(e){switch(e){case 0:return this.x;case 1:return this.y;case 2:return this.z;case 3:return this.w;default:throw new Error("THREE.Vector4: index is out of range: "+e)}}clone(){return new this.constructor(this.x,this.y,this.z,this.w)}copy(e){return this.x=e.x,this.y=e.y,this.z=e.z,this.w=e.w!==void 0?e.w:1,this}add(e){return this.x+=e.x,this.y+=e.y,this.z+=e.z,this.w+=e.w,this}addScalar(e){return this.x+=e,this.y+=e,this.z+=e,this.w+=e,this}addVectors(e,t){return this.x=e.x+t.x,this.y=e.y+t.y,this.z=e.z+t.z,this.w=e.w+t.w,this}addScaledVector(e,t){return this.x+=e.x*t,this.y+=e.y*t,this.z+=e.z*t,this.w+=e.w*t,this}sub(e){return this.x-=e.x,this.y-=e.y,this.z-=e.z,this.w-=e.w,this}subScalar(e){return this.x-=e,this.y-=e,this.z-=e,this.w-=e,this}subVectors(e,t){return this.x=e.x-t.x,this.y=e.y-t.y,this.z=e.z-t.z,this.w=e.w-t.w,this}multiply(e){return this.x*=e.x,this.y*=e.y,this.z*=e.z,this.w*=e.w,this}multiplyScalar(e){return this.x*=e,this.y*=e,this.z*=e,this.w*=e,this}applyMatrix4(e){const t=this.x,n=this.y,r=this.z,s=this.w,a=e.elements;return this.x=a[0]*t+a[4]*n+a[8]*r+a[12]*s,this.y=a[1]*t+a[5]*n+a[9]*r+a[13]*s,this.z=a[2]*t+a[6]*n+a[10]*r+a[14]*s,this.w=a[3]*t+a[7]*n+a[11]*r+a[15]*s,this}divide(e){return this.x/=e.x,this.y/=e.y,this.z/=e.z,this.w/=e.w,this}divideScalar(e){return this.multiplyScalar(1/e)}setAxisAngleFromQuaternion(e){this.w=2*Math.acos(e.w);const t=Math.sqrt(1-e.w*e.w);return t<1e-4?(this.x=1,this.y=0,this.z=0):(this.x=e.x/t,this.y=e.y/t,this.z=e.z/t),this}setAxisAngleFromRotationMatrix(e){let t,n,r,s;const c=e.elements,l=c[0],u=c[4],f=c[8],d=c[1],m=c[5],x=c[9],S=c[2],g=c[6],h=c[10];if(Math.abs(u-d)<.01&&Math.abs(f-S)<.01&&Math.abs(x-g)<.01){if(Math.abs(u+d)<.1&&Math.abs(f+S)<.1&&Math.abs(x+g)<.1&&Math.abs(l+m+h-3)<.1)return this.set(1,0,0,0),this;t=Math.PI;const C=(l+1)/2,y=(m+1)/2,w=(h+1)/2,T=(u+d)/4,I=(f+S)/4,_=(x+g)/4;return C>y&&C>w?C<.01?(n=0,r=.707106781,s=.707106781):(n=Math.sqrt(C),r=T/n,s=I/n):y>w?y<.01?(n=.707106781,r=0,s=.707106781):(r=Math.sqrt(y),n=T/r,s=_/r):w<.01?(n=.707106781,r=.707106781,s=0):(s=Math.sqrt(w),n=I/s,r=_/s),this.set(n,r,s,t),this}let R=Math.sqrt((g-x)*(g-x)+(f-S)*(f-S)+(d-u)*(d-u));return Math.abs(R)<.001&&(R=1),this.x=(g-x)/R,this.y=(f-S)/R,this.z=(d-u)/R,this.w=Math.acos((l+m+h-1)/2),this}setFromMatrixPosition(e){const t=e.elements;return this.x=t[12],this.y=t[13],this.z=t[14],this.w=t[15],this}min(e){return this.x=Math.min(this.x,e.x),this.y=Math.min(this.y,e.y),this.z=Math.min(this.z,e.z),this.w=Math.min(this.w,e.w),this}max(e){return this.x=Math.max(this.x,e.x),this.y=Math.max(this.y,e.y),this.z=Math.max(this.z,e.z),this.w=Math.max(this.w,e.w),this}clamp(e,t){return this.x=gt(this.x,e.x,t.x),this.y=gt(this.y,e.y,t.y),this.z=gt(this.z,e.z,t.z),this.w=gt(this.w,e.w,t.w),this}clampScalar(e,t){return this.x=gt(this.x,e,t),this.y=gt(this.y,e,t),this.z=gt(this.z,e,t),this.w=gt(this.w,e,t),this}clampLength(e,t){const n=this.length();return this.divideScalar(n||1).multiplyScalar(gt(n,e,t))}floor(){return this.x=Math.floor(this.x),this.y=Math.floor(this.y),this.z=Math.floor(this.z),this.w=Math.floor(this.w),this}ceil(){return this.x=Math.ceil(this.x),this.y=Math.ceil(this.y),this.z=Math.ceil(this.z),this.w=Math.ceil(this.w),this}round(){return this.x=Math.round(this.x),this.y=Math.round(this.y),this.z=Math.round(this.z),this.w=Math.round(this.w),this}roundToZero(){return this.x=Math.trunc(this.x),this.y=Math.trunc(this.y),this.z=Math.trunc(this.z),this.w=Math.trunc(this.w),this}negate(){return this.x=-this.x,this.y=-this.y,this.z=-this.z,this.w=-this.w,this}dot(e){return this.x*e.x+this.y*e.y+this.z*e.z+this.w*e.w}lengthSq(){return this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w}length(){return Math.sqrt(this.x*this.x+this.y*this.y+this.z*this.z+this.w*this.w)}manhattanLength(){return Math.abs(this.x)+Math.abs(this.y)+Math.abs(this.z)+Math.abs(this.w)}normalize(){return this.divideScalar(this.length()||1)}setLength(e){return this.normalize().multiplyScalar(e)}lerp(e,t){return this.x+=(e.x-this.x)*t,this.y+=(e.y-this.y)*t,this.z+=(e.z-this.z)*t,this.w+=(e.w-this.w)*t,this}lerpVectors(e,t,n){return this.x=e.x+(t.x-e.x)*n,this.y=e.y+(t.y-e.y)*n,this.z=e.z+(t.z-e.z)*n,this.w=e.w+(t.w-e.w)*n,this}equals(e){return e.x===this.x&&e.y===this.y&&e.z===this.z&&e.w===this.w}fromArray(e,t=0){return this.x=e[t],this.y=e[t+1],this.z=e[t+2],this.w=e[t+3],this}toArray(e=[],t=0){return e[t]=this.x,e[t+1]=this.y,e[t+2]=this.z,e[t+3]=this.w,e}fromBufferAttribute(e,t){return this.x=e.getX(t),this.y=e.getY(t),this.z=e.getZ(t),this.w=e.getW(t),this}random(){return this.x=Math.random(),this.y=Math.random(),this.z=Math.random(),this.w=Math.random(),this}*[Symbol.iterator](){yield this.x,yield this.y,yield this.z,yield this.w}};Bo.prototype.isVector4=!0;let Ot=Bo;class Qd extends Gi{constructor(e=1,t=1,n={}){super(),n=Object.assign({generateMipmaps:!1,internalFormat:null,minFilter:Qt,depthBuffer:!0,stencilBuffer:!1,resolveDepthBuffer:!0,resolveStencilBuffer:!0,depthTexture:null,samples:0,count:1,depth:1,multiview:!1,useArrayDepthTexture:!1},n),this.isRenderTarget=!0,this.width=e,this.height=t,this.depth=n.depth,this.scissor=new Ot(0,0,e,t),this.scissorTest=!1,this.viewport=new Ot(0,0,e,t),this.textures=[];const r={width:e,height:t,depth:n.depth},s=new tn(r),a=n.count;for(let o=0;o<a;o++)this.textures[o]=s.clone(),this.textures[o].isRenderTargetTexture=!0,this.textures[o].renderTarget=this;this._setTextureOptions(n),this.depthBuffer=n.depthBuffer,this.stencilBuffer=n.stencilBuffer,this.resolveDepthBuffer=n.resolveDepthBuffer,this.resolveStencilBuffer=n.resolveStencilBuffer,this._depthTexture=null,this.depthTexture=n.depthTexture,this.samples=n.samples,this.multiview=n.multiview,this.useArrayDepthTexture=n.useArrayDepthTexture}_setTextureOptions(e={}){const t={minFilter:Qt,generateMipmaps:!1,flipY:!1,internalFormat:null};e.mapping!==void 0&&(t.mapping=e.mapping),e.wrapS!==void 0&&(t.wrapS=e.wrapS),e.wrapT!==void 0&&(t.wrapT=e.wrapT),e.wrapR!==void 0&&(t.wrapR=e.wrapR),e.magFilter!==void 0&&(t.magFilter=e.magFilter),e.minFilter!==void 0&&(t.minFilter=e.minFilter),e.format!==void 0&&(t.format=e.format),e.type!==void 0&&(t.type=e.type),e.anisotropy!==void 0&&(t.anisotropy=e.anisotropy),e.colorSpace!==void 0&&(t.colorSpace=e.colorSpace),e.flipY!==void 0&&(t.flipY=e.flipY),e.generateMipmaps!==void 0&&(t.generateMipmaps=e.generateMipmaps),e.internalFormat!==void 0&&(t.internalFormat=e.internalFormat);for(let n=0;n<this.textures.length;n++)this.textures[n].setValues(t)}get texture(){return this.textures[0]}set texture(e){this.textures[0]=e}set depthTexture(e){this._depthTexture!==null&&(this._depthTexture.renderTarget=null),e!==null&&(e.renderTarget=this),this._depthTexture=e}get depthTexture(){return this._depthTexture}setSize(e,t,n=1){if(this.width!==e||this.height!==t||this.depth!==n){this.width=e,this.height=t,this.depth=n;for(let r=0,s=this.textures.length;r<s;r++)this.textures[r].image.width=e,this.textures[r].image.height=t,this.textures[r].image.depth=n,this.textures[r].isData3DTexture!==!0&&(this.textures[r].isArrayTexture=this.textures[r].image.depth>1);this.dispose()}this.viewport.set(0,0,e,t),this.scissor.set(0,0,e,t)}clone(){return new this.constructor().copy(this)}copy(e){this.width=e.width,this.height=e.height,this.depth=e.depth,this.scissor.copy(e.scissor),this.scissorTest=e.scissorTest,this.viewport.copy(e.viewport),this.textures.length=0;for(let t=0,n=e.textures.length;t<n;t++){this.textures[t]=e.textures[t].clone(),this.textures[t].isRenderTargetTexture=!0,this.textures[t].renderTarget=this;const r=Object.assign({},e.textures[t].image);this.textures[t].source=new Po(r)}return this.depthBuffer=e.depthBuffer,this.stencilBuffer=e.stencilBuffer,this.resolveDepthBuffer=e.resolveDepthBuffer,this.resolveStencilBuffer=e.resolveStencilBuffer,e.depthTexture!==null&&(this.depthTexture=e.depthTexture.clone()),this.samples=e.samples,this.multiview=e.multiview,this.useArrayDepthTexture=e.useArrayDepthTexture,this}dispose(){this.dispatchEvent({type:"dispose"})}}class Wn extends Qd{constructor(e=1,t=1,n={}){super(e,t,n),this.isWebGLRenderTarget=!0}}class fc extends tn{constructor(e=null,t=1,n=1,r=1){super(null),this.isDataArrayTexture=!0,this.image={data:e,width:t,height:n,depth:r},this.magFilter=Jt,this.minFilter=Jt,this.wrapR=ri,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1,this.layerUpdates=new Set}addLayerUpdate(e){this.layerUpdates.add(e)}clearLayerUpdates(){this.layerUpdates.clear()}}class jd extends tn{constructor(e=null,t=1,n=1,r=1){super(null),this.isData3DTexture=!0,this.image={data:e,width:t,height:n,depth:r},this.magFilter=Jt,this.minFilter=Jt,this.wrapR=ri,this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const Is=class Is{constructor(e,t,n,r,s,a,o,c,l,u,f,d,m,x,S,g){this.elements=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],e!==void 0&&this.set(e,t,n,r,s,a,o,c,l,u,f,d,m,x,S,g)}set(e,t,n,r,s,a,o,c,l,u,f,d,m,x,S,g){const h=this.elements;return h[0]=e,h[4]=t,h[8]=n,h[12]=r,h[1]=s,h[5]=a,h[9]=o,h[13]=c,h[2]=l,h[6]=u,h[10]=f,h[14]=d,h[3]=m,h[7]=x,h[11]=S,h[15]=g,this}identity(){return this.set(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1),this}clone(){return new Is().fromArray(this.elements)}copy(e){const t=this.elements,n=e.elements;return t[0]=n[0],t[1]=n[1],t[2]=n[2],t[3]=n[3],t[4]=n[4],t[5]=n[5],t[6]=n[6],t[7]=n[7],t[8]=n[8],t[9]=n[9],t[10]=n[10],t[11]=n[11],t[12]=n[12],t[13]=n[13],t[14]=n[14],t[15]=n[15],this}copyPosition(e){const t=this.elements,n=e.elements;return t[12]=n[12],t[13]=n[13],t[14]=n[14],this}setFromMatrix3(e){const t=e.elements;return this.set(t[0],t[3],t[6],0,t[1],t[4],t[7],0,t[2],t[5],t[8],0,0,0,0,1),this}extractBasis(e,t,n){return this.determinantAffine()===0?(e.set(1,0,0),t.set(0,1,0),n.set(0,0,1),this):(e.setFromMatrixColumn(this,0),t.setFromMatrixColumn(this,1),n.setFromMatrixColumn(this,2),this)}makeBasis(e,t,n){return this.set(e.x,t.x,n.x,0,e.y,t.y,n.y,0,e.z,t.z,n.z,0,0,0,0,1),this}extractRotation(e){if(e.determinantAffine()===0)return this.identity();const t=this.elements,n=e.elements,r=1/Qi.setFromMatrixColumn(e,0).length(),s=1/Qi.setFromMatrixColumn(e,1).length(),a=1/Qi.setFromMatrixColumn(e,2).length();return t[0]=n[0]*r,t[1]=n[1]*r,t[2]=n[2]*r,t[3]=0,t[4]=n[4]*s,t[5]=n[5]*s,t[6]=n[6]*s,t[7]=0,t[8]=n[8]*a,t[9]=n[9]*a,t[10]=n[10]*a,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromEuler(e){const t=this.elements,n=e.x,r=e.y,s=e.z,a=Math.cos(n),o=Math.sin(n),c=Math.cos(r),l=Math.sin(r),u=Math.cos(s),f=Math.sin(s);if(e.order==="XYZ"){const d=a*u,m=a*f,x=o*u,S=o*f;t[0]=c*u,t[4]=-c*f,t[8]=l,t[1]=m+x*l,t[5]=d-S*l,t[9]=-o*c,t[2]=S-d*l,t[6]=x+m*l,t[10]=a*c}else if(e.order==="YXZ"){const d=c*u,m=c*f,x=l*u,S=l*f;t[0]=d+S*o,t[4]=x*o-m,t[8]=a*l,t[1]=a*f,t[5]=a*u,t[9]=-o,t[2]=m*o-x,t[6]=S+d*o,t[10]=a*c}else if(e.order==="ZXY"){const d=c*u,m=c*f,x=l*u,S=l*f;t[0]=d-S*o,t[4]=-a*f,t[8]=x+m*o,t[1]=m+x*o,t[5]=a*u,t[9]=S-d*o,t[2]=-a*l,t[6]=o,t[10]=a*c}else if(e.order==="ZYX"){const d=a*u,m=a*f,x=o*u,S=o*f;t[0]=c*u,t[4]=x*l-m,t[8]=d*l+S,t[1]=c*f,t[5]=S*l+d,t[9]=m*l-x,t[2]=-l,t[6]=o*c,t[10]=a*c}else if(e.order==="YZX"){const d=a*c,m=a*l,x=o*c,S=o*l;t[0]=c*u,t[4]=S-d*f,t[8]=x*f+m,t[1]=f,t[5]=a*u,t[9]=-o*u,t[2]=-l*u,t[6]=m*f+x,t[10]=d-S*f}else if(e.order==="XZY"){const d=a*c,m=a*l,x=o*c,S=o*l;t[0]=c*u,t[4]=-f,t[8]=l*u,t[1]=d*f+S,t[5]=a*u,t[9]=m*f-x,t[2]=x*f-m,t[6]=o*u,t[10]=S*f+d}return t[3]=0,t[7]=0,t[11]=0,t[12]=0,t[13]=0,t[14]=0,t[15]=1,this}makeRotationFromQuaternion(e){return this.compose(eu,e,tu)}lookAt(e,t,n){const r=this.elements;return fn.subVectors(e,t),fn.lengthSq()===0&&(fn.z=1),fn.normalize(),fi.crossVectors(n,fn),fi.lengthSq()===0&&(Math.abs(n.z)===1?fn.x+=1e-4:fn.z+=1e-4,fn.normalize(),fi.crossVectors(n,fn)),fi.normalize(),qr.crossVectors(fn,fi),r[0]=fi.x,r[4]=qr.x,r[8]=fn.x,r[1]=fi.y,r[5]=qr.y,r[9]=fn.y,r[2]=fi.z,r[6]=qr.z,r[10]=fn.z,this}multiply(e){return this.multiplyMatrices(this,e)}premultiply(e){return this.multiplyMatrices(e,this)}multiplyMatrices(e,t){const n=e.elements,r=t.elements,s=this.elements,a=n[0],o=n[4],c=n[8],l=n[12],u=n[1],f=n[5],d=n[9],m=n[13],x=n[2],S=n[6],g=n[10],h=n[14],R=n[3],C=n[7],y=n[11],w=n[15],T=r[0],I=r[4],_=r[8],A=r[12],k=r[1],F=r[5],b=r[9],P=r[13],W=r[2],N=r[6],D=r[10],G=r[14],H=r[3],ne=r[7],ce=r[11],oe=r[15];return s[0]=a*T+o*k+c*W+l*H,s[4]=a*I+o*F+c*N+l*ne,s[8]=a*_+o*b+c*D+l*ce,s[12]=a*A+o*P+c*G+l*oe,s[1]=u*T+f*k+d*W+m*H,s[5]=u*I+f*F+d*N+m*ne,s[9]=u*_+f*b+d*D+m*ce,s[13]=u*A+f*P+d*G+m*oe,s[2]=x*T+S*k+g*W+h*H,s[6]=x*I+S*F+g*N+h*ne,s[10]=x*_+S*b+g*D+h*ce,s[14]=x*A+S*P+g*G+h*oe,s[3]=R*T+C*k+y*W+w*H,s[7]=R*I+C*F+y*N+w*ne,s[11]=R*_+C*b+y*D+w*ce,s[15]=R*A+C*P+y*G+w*oe,this}multiplyScalar(e){const t=this.elements;return t[0]*=e,t[4]*=e,t[8]*=e,t[12]*=e,t[1]*=e,t[5]*=e,t[9]*=e,t[13]*=e,t[2]*=e,t[6]*=e,t[10]*=e,t[14]*=e,t[3]*=e,t[7]*=e,t[11]*=e,t[15]*=e,this}determinant(){const e=this.elements,t=e[0],n=e[4],r=e[8],s=e[12],a=e[1],o=e[5],c=e[9],l=e[13],u=e[2],f=e[6],d=e[10],m=e[14],x=e[3],S=e[7],g=e[11],h=e[15],R=c*m-l*d,C=o*m-l*f,y=o*d-c*f,w=a*m-l*u,T=a*d-c*u,I=a*f-o*u;return t*(S*R-g*C+h*y)-n*(x*R-g*w+h*T)+r*(x*C-S*w+h*I)-s*(x*y-S*T+g*I)}determinantAffine(){const e=this.elements,t=e[0],n=e[4],r=e[8],s=e[1],a=e[5],o=e[9],c=e[2],l=e[6],u=e[10];return t*(a*u-o*l)-n*(s*u-o*c)+r*(s*l-a*c)}transpose(){const e=this.elements;let t;return t=e[1],e[1]=e[4],e[4]=t,t=e[2],e[2]=e[8],e[8]=t,t=e[6],e[6]=e[9],e[9]=t,t=e[3],e[3]=e[12],e[12]=t,t=e[7],e[7]=e[13],e[13]=t,t=e[11],e[11]=e[14],e[14]=t,this}setPosition(e,t,n){const r=this.elements;return e.isVector3?(r[12]=e.x,r[13]=e.y,r[14]=e.z):(r[12]=e,r[13]=t,r[14]=n),this}invert(){const e=this.elements,t=e[0],n=e[1],r=e[2],s=e[3],a=e[4],o=e[5],c=e[6],l=e[7],u=e[8],f=e[9],d=e[10],m=e[11],x=e[12],S=e[13],g=e[14],h=e[15],R=t*o-n*a,C=t*c-r*a,y=t*l-s*a,w=n*c-r*o,T=n*l-s*o,I=r*l-s*c,_=u*S-f*x,A=u*g-d*x,k=u*h-m*x,F=f*g-d*S,b=f*h-m*S,P=d*h-m*g,W=R*P-C*b+y*F+w*k-T*A+I*_;if(W===0)return this.set(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);const N=1/W;return e[0]=(o*P-c*b+l*F)*N,e[1]=(r*b-n*P-s*F)*N,e[2]=(S*I-g*T+h*w)*N,e[3]=(d*T-f*I-m*w)*N,e[4]=(c*k-a*P-l*A)*N,e[5]=(t*P-r*k+s*A)*N,e[6]=(g*y-x*I-h*C)*N,e[7]=(u*I-d*y+m*C)*N,e[8]=(a*b-o*k+l*_)*N,e[9]=(n*k-t*b-s*_)*N,e[10]=(x*T-S*y+h*R)*N,e[11]=(f*y-u*T-m*R)*N,e[12]=(o*A-a*F-c*_)*N,e[13]=(t*F-n*A+r*_)*N,e[14]=(S*C-x*w-g*R)*N,e[15]=(u*w-f*C+d*R)*N,this}scale(e){const t=this.elements,n=e.x,r=e.y,s=e.z;return t[0]*=n,t[4]*=r,t[8]*=s,t[1]*=n,t[5]*=r,t[9]*=s,t[2]*=n,t[6]*=r,t[10]*=s,t[3]*=n,t[7]*=r,t[11]*=s,this}getMaxScaleOnAxis(){const e=this.elements,t=e[0]*e[0]+e[1]*e[1]+e[2]*e[2],n=e[4]*e[4]+e[5]*e[5]+e[6]*e[6],r=e[8]*e[8]+e[9]*e[9]+e[10]*e[10];return Math.sqrt(Math.max(t,n,r))}makeTranslation(e,t,n){return e.isVector3?this.set(1,0,0,e.x,0,1,0,e.y,0,0,1,e.z,0,0,0,1):this.set(1,0,0,e,0,1,0,t,0,0,1,n,0,0,0,1),this}makeRotationX(e){const t=Math.cos(e),n=Math.sin(e);return this.set(1,0,0,0,0,t,-n,0,0,n,t,0,0,0,0,1),this}makeRotationY(e){const t=Math.cos(e),n=Math.sin(e);return this.set(t,0,n,0,0,1,0,0,-n,0,t,0,0,0,0,1),this}makeRotationZ(e){const t=Math.cos(e),n=Math.sin(e);return this.set(t,-n,0,0,n,t,0,0,0,0,1,0,0,0,0,1),this}makeRotationAxis(e,t){const n=Math.cos(t),r=Math.sin(t),s=1-n,a=e.x,o=e.y,c=e.z,l=s*a,u=s*o;return this.set(l*a+n,l*o-r*c,l*c+r*o,0,l*o+r*c,u*o+n,u*c-r*a,0,l*c-r*o,u*c+r*a,s*c*c+n,0,0,0,0,1),this}makeScale(e,t,n){return this.set(e,0,0,0,0,t,0,0,0,0,n,0,0,0,0,1),this}makeShear(e,t,n,r,s,a){return this.set(1,n,s,0,e,1,a,0,t,r,1,0,0,0,0,1),this}compose(e,t,n){const r=this.elements,s=t._x,a=t._y,o=t._z,c=t._w,l=s+s,u=a+a,f=o+o,d=s*l,m=s*u,x=s*f,S=a*u,g=a*f,h=o*f,R=c*l,C=c*u,y=c*f,w=n.x,T=n.y,I=n.z;return r[0]=(1-(S+h))*w,r[1]=(m+y)*w,r[2]=(x-C)*w,r[3]=0,r[4]=(m-y)*T,r[5]=(1-(d+h))*T,r[6]=(g+R)*T,r[7]=0,r[8]=(x+C)*I,r[9]=(g-R)*I,r[10]=(1-(d+S))*I,r[11]=0,r[12]=e.x,r[13]=e.y,r[14]=e.z,r[15]=1,this}decompose(e,t,n){const r=this.elements;e.x=r[12],e.y=r[13],e.z=r[14];const s=this.determinantAffine();if(s===0)return n.set(1,1,1),t.identity(),this;let a=Qi.set(r[0],r[1],r[2]).length();const o=Qi.set(r[4],r[5],r[6]).length(),c=Qi.set(r[8],r[9],r[10]).length();s<0&&(a=-a),En.copy(this);const l=1/a,u=1/o,f=1/c;return En.elements[0]*=l,En.elements[1]*=l,En.elements[2]*=l,En.elements[4]*=u,En.elements[5]*=u,En.elements[6]*=u,En.elements[8]*=f,En.elements[9]*=f,En.elements[10]*=f,t.setFromRotationMatrix(En),n.x=a,n.y=o,n.z=c,this}makePerspective(e,t,n,r,s,a,o=Hn,c=!1){const l=this.elements,u=2*s/(t-e),f=2*s/(n-r),d=(t+e)/(t-e),m=(n+r)/(n-r);let x,S;if(c)x=s/(a-s),S=a*s/(a-s);else if(o===Hn)x=-(a+s)/(a-s),S=-2*a*s/(a-s);else if(o===Rs)x=-a/(a-s),S=-a*s/(a-s);else throw new Error("THREE.Matrix4.makePerspective(): Invalid coordinate system: "+o);return l[0]=u,l[4]=0,l[8]=d,l[12]=0,l[1]=0,l[5]=f,l[9]=m,l[13]=0,l[2]=0,l[6]=0,l[10]=x,l[14]=S,l[3]=0,l[7]=0,l[11]=-1,l[15]=0,this}makeOrthographic(e,t,n,r,s,a,o=Hn,c=!1){const l=this.elements,u=2/(t-e),f=2/(n-r),d=-(t+e)/(t-e),m=-(n+r)/(n-r);let x,S;if(c)x=1/(a-s),S=a/(a-s);else if(o===Hn)x=-2/(a-s),S=-(a+s)/(a-s);else if(o===Rs)x=-1/(a-s),S=-s/(a-s);else throw new Error("THREE.Matrix4.makeOrthographic(): Invalid coordinate system: "+o);return l[0]=u,l[4]=0,l[8]=0,l[12]=d,l[1]=0,l[5]=f,l[9]=0,l[13]=m,l[2]=0,l[6]=0,l[10]=x,l[14]=S,l[3]=0,l[7]=0,l[11]=0,l[15]=1,this}equals(e){const t=this.elements,n=e.elements;for(let r=0;r<16;r++)if(t[r]!==n[r])return!1;return!0}fromArray(e,t=0){for(let n=0;n<16;n++)this.elements[n]=e[n+t];return this}toArray(e=[],t=0){const n=this.elements;return e[t]=n[0],e[t+1]=n[1],e[t+2]=n[2],e[t+3]=n[3],e[t+4]=n[4],e[t+5]=n[5],e[t+6]=n[6],e[t+7]=n[7],e[t+8]=n[8],e[t+9]=n[9],e[t+10]=n[10],e[t+11]=n[11],e[t+12]=n[12],e[t+13]=n[13],e[t+14]=n[14],e[t+15]=n[15],e}};Is.prototype.isMatrix4=!0;let Vt=Is;const Qi=new K,En=new Vt,eu=new K(0,0,0),tu=new K(1,1,1),fi=new K,qr=new K,fn=new K,Qo=new Vt,jo=new xr;class zi{constructor(e=0,t=0,n=0,r=zi.DEFAULT_ORDER){this.isEuler=!0,this._x=e,this._y=t,this._z=n,this._order=r}get x(){return this._x}set x(e){this._x=e,this._onChangeCallback()}get y(){return this._y}set y(e){this._y=e,this._onChangeCallback()}get z(){return this._z}set z(e){this._z=e,this._onChangeCallback()}get order(){return this._order}set order(e){this._order=e,this._onChangeCallback()}set(e,t,n,r=this._order){return this._x=e,this._y=t,this._z=n,this._order=r,this._onChangeCallback(),this}clone(){return new this.constructor(this._x,this._y,this._z,this._order)}copy(e){return this._x=e._x,this._y=e._y,this._z=e._z,this._order=e._order,this._onChangeCallback(),this}setFromRotationMatrix(e,t=this._order,n=!0){const r=e.elements,s=r[0],a=r[4],o=r[8],c=r[1],l=r[5],u=r[9],f=r[2],d=r[6],m=r[10];switch(t){case"XYZ":this._y=Math.asin(gt(o,-1,1)),Math.abs(o)<.9999999?(this._x=Math.atan2(-u,m),this._z=Math.atan2(-a,s)):(this._x=Math.atan2(d,l),this._z=0);break;case"YXZ":this._x=Math.asin(-gt(u,-1,1)),Math.abs(u)<.9999999?(this._y=Math.atan2(o,m),this._z=Math.atan2(c,l)):(this._y=Math.atan2(-f,s),this._z=0);break;case"ZXY":this._x=Math.asin(gt(d,-1,1)),Math.abs(d)<.9999999?(this._y=Math.atan2(-f,m),this._z=Math.atan2(-a,l)):(this._y=0,this._z=Math.atan2(c,s));break;case"ZYX":this._y=Math.asin(-gt(f,-1,1)),Math.abs(f)<.9999999?(this._x=Math.atan2(d,m),this._z=Math.atan2(c,s)):(this._x=0,this._z=Math.atan2(-a,l));break;case"YZX":this._z=Math.asin(gt(c,-1,1)),Math.abs(c)<.9999999?(this._x=Math.atan2(-u,l),this._y=Math.atan2(-f,s)):(this._x=0,this._y=Math.atan2(o,m));break;case"XZY":this._z=Math.asin(-gt(a,-1,1)),Math.abs(a)<.9999999?(this._x=Math.atan2(d,l),this._y=Math.atan2(o,s)):(this._x=Math.atan2(-u,m),this._y=0);break;default:je("Euler: .setFromRotationMatrix() encountered an unknown order: "+t)}return this._order=t,n===!0&&this._onChangeCallback(),this}setFromQuaternion(e,t,n){return Qo.makeRotationFromQuaternion(e),this.setFromRotationMatrix(Qo,t,n)}setFromVector3(e,t=this._order){return this.set(e.x,e.y,e.z,t)}reorder(e){return jo.setFromEuler(this),this.setFromQuaternion(jo,e)}equals(e){return e._x===this._x&&e._y===this._y&&e._z===this._z&&e._order===this._order}fromArray(e){return this._x=e[0],this._y=e[1],this._z=e[2],e[3]!==void 0&&(this._order=e[3]),this._onChangeCallback(),this}toArray(e=[],t=0){return e[t]=this._x,e[t+1]=this._y,e[t+2]=this._z,e[t+3]=this._order,e}_onChange(e){return this._onChangeCallback=e,this}_onChangeCallback(){}*[Symbol.iterator](){yield this._x,yield this._y,yield this._z,yield this._order}}zi.DEFAULT_ORDER="XYZ";class pc{constructor(){this.mask=1}set(e){this.mask=(1<<e|0)>>>0}enable(e){this.mask|=1<<e|0}enableAll(){this.mask=-1}toggle(e){this.mask^=1<<e|0}disable(e){this.mask&=~(1<<e|0)}disableAll(){this.mask=0}test(e){return(this.mask&e.mask)!==0}isEnabled(e){return(this.mask&(1<<e|0))!==0}}let nu=0;const el=new K,ji=new xr,$n=new Vt,Yr=new K,Er=new K,iu=new K,ru=new xr,tl=new K(1,0,0),nl=new K(0,1,0),il=new K(0,0,1),rl={type:"added"},su={type:"removed"},er={type:"childadded",child:null},js={type:"childremoved",child:null};class mn extends Gi{constructor(){super(),this.isObject3D=!0,Object.defineProperty(this,"id",{value:nu++}),this.uuid=Si(),this.name="",this.type="Object3D",this.parent=null,this.children=[],this.up=mn.DEFAULT_UP.clone();const e=new K,t=new zi,n=new xr,r=new K(1,1,1);function s(){n.setFromEuler(t,!1)}function a(){t.setFromQuaternion(n,void 0,!1)}t._onChange(s),n._onChange(a),Object.defineProperties(this,{position:{configurable:!0,enumerable:!0,value:e},rotation:{configurable:!0,enumerable:!0,value:t},quaternion:{configurable:!0,enumerable:!0,value:n},scale:{configurable:!0,enumerable:!0,value:r},modelViewMatrix:{value:new Vt},normalMatrix:{value:new nt}}),this.matrix=new Vt,this.matrixWorld=new Vt,this.matrixAutoUpdate=mn.DEFAULT_MATRIX_AUTO_UPDATE,this.matrixWorldAutoUpdate=mn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE,this.matrixWorldNeedsUpdate=!1,this.layers=new pc,this.visible=!0,this.castShadow=!1,this.receiveShadow=!1,this.frustumCulled=!0,this.renderOrder=0,this.animations=[],this.customDepthMaterial=void 0,this.customDistanceMaterial=void 0,this.static=!1,this.userData={},this.pivot=null}onBeforeShadow(){}onAfterShadow(){}onBeforeRender(){}onAfterRender(){}applyMatrix4(e){this.matrixAutoUpdate&&this.updateMatrix(),this.matrix.premultiply(e),this.matrix.decompose(this.position,this.quaternion,this.scale)}applyQuaternion(e){return this.quaternion.premultiply(e),this}setRotationFromAxisAngle(e,t){this.quaternion.setFromAxisAngle(e,t)}setRotationFromEuler(e){this.quaternion.setFromEuler(e,!0)}setRotationFromMatrix(e){this.quaternion.setFromRotationMatrix(e)}setRotationFromQuaternion(e){this.quaternion.copy(e)}rotateOnAxis(e,t){return ji.setFromAxisAngle(e,t),this.quaternion.multiply(ji),this}rotateOnWorldAxis(e,t){return ji.setFromAxisAngle(e,t),this.quaternion.premultiply(ji),this}rotateX(e){return this.rotateOnAxis(tl,e)}rotateY(e){return this.rotateOnAxis(nl,e)}rotateZ(e){return this.rotateOnAxis(il,e)}translateOnAxis(e,t){return el.copy(e).applyQuaternion(this.quaternion),this.position.add(el.multiplyScalar(t)),this}translateX(e){return this.translateOnAxis(tl,e)}translateY(e){return this.translateOnAxis(nl,e)}translateZ(e){return this.translateOnAxis(il,e)}localToWorld(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4(this.matrixWorld)}worldToLocal(e){return this.updateWorldMatrix(!0,!1),e.applyMatrix4($n.copy(this.matrixWorld).invert())}lookAt(e,t,n){e.isVector3?Yr.copy(e):Yr.set(e,t,n);const r=this.parent;this.updateWorldMatrix(!0,!1),Er.setFromMatrixPosition(this.matrixWorld),this.isCamera||this.isLight?$n.lookAt(Er,Yr,this.up):$n.lookAt(Yr,Er,this.up),this.quaternion.setFromRotationMatrix($n),r&&($n.extractRotation(r.matrixWorld),ji.setFromRotationMatrix($n),this.quaternion.premultiply(ji.invert()))}add(e){if(arguments.length>1){for(let t=0;t<arguments.length;t++)this.add(arguments[t]);return this}return e===this?(_t("Object3D.add: object can't be added as a child of itself.",e),this):(e&&e.isObject3D?(e.removeFromParent(),e.parent=this,this.children.push(e),e.dispatchEvent(rl),er.child=e,this.dispatchEvent(er),er.child=null):_t("Object3D.add: object not an instance of THREE.Object3D.",e),this)}remove(e){if(arguments.length>1){for(let n=0;n<arguments.length;n++)this.remove(arguments[n]);return this}const t=this.children.indexOf(e);return t!==-1&&(e.parent=null,this.children.splice(t,1),e.dispatchEvent(su),js.child=e,this.dispatchEvent(js),js.child=null),this}removeFromParent(){const e=this.parent;return e!==null&&e.remove(this),this}clear(){return this.remove(...this.children)}attach(e){return this.updateWorldMatrix(!0,!1),$n.copy(this.matrixWorld).invert(),e.parent!==null&&(e.parent.updateWorldMatrix(!0,!1),$n.multiply(e.parent.matrixWorld)),e.applyMatrix4($n),e.removeFromParent(),e.parent=this,this.children.push(e),e.updateWorldMatrix(!1,!0),e.dispatchEvent(rl),er.child=e,this.dispatchEvent(er),er.child=null,this}getObjectById(e){return this.getObjectByProperty("id",e)}getObjectByName(e){return this.getObjectByProperty("name",e)}getObjectByProperty(e,t){if(this[e]===t)return this;for(let n=0,r=this.children.length;n<r;n++){const a=this.children[n].getObjectByProperty(e,t);if(a!==void 0)return a}}getObjectsByProperty(e,t,n=[]){this[e]===t&&n.push(this);const r=this.children;for(let s=0,a=r.length;s<a;s++)r[s].getObjectsByProperty(e,t,n);return n}getWorldPosition(e){return this.updateWorldMatrix(!0,!1),e.setFromMatrixPosition(this.matrixWorld)}getWorldQuaternion(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Er,e,iu),e}getWorldScale(e){return this.updateWorldMatrix(!0,!1),this.matrixWorld.decompose(Er,ru,e),e}getWorldDirection(e){this.updateWorldMatrix(!0,!1);const t=this.matrixWorld.elements;return e.set(t[8],t[9],t[10]).normalize()}raycast(){}traverse(e){e(this);const t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].traverse(e)}traverseVisible(e){if(this.visible===!1)return;e(this);const t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].traverseVisible(e)}traverseAncestors(e){const t=this.parent;t!==null&&(e(t),t.traverseAncestors(e))}updateMatrix(){this.matrix.compose(this.position,this.quaternion,this.scale);const e=this.pivot;if(e!==null){const t=e.x,n=e.y,r=e.z,s=this.matrix.elements;s[12]+=t-s[0]*t-s[4]*n-s[8]*r,s[13]+=n-s[1]*t-s[5]*n-s[9]*r,s[14]+=r-s[2]*t-s[6]*n-s[10]*r}this.matrixWorldNeedsUpdate=!0}updateMatrixWorld(e){this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||e)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,e=!0);const t=this.children;for(let n=0,r=t.length;n<r;n++)t[n].updateMatrixWorld(e)}updateWorldMatrix(e,t,n=!1){const r=this.parent;if(e===!0&&r!==null&&r.updateWorldMatrix(!0,!1),this.matrixAutoUpdate&&this.updateMatrix(),(this.matrixWorldNeedsUpdate||n)&&(this.matrixWorldAutoUpdate===!0&&(this.parent===null?this.matrixWorld.copy(this.matrix):this.matrixWorld.multiplyMatrices(this.parent.matrixWorld,this.matrix)),this.matrixWorldNeedsUpdate=!1,n=!0),t===!0){const s=this.children;for(let a=0,o=s.length;a<o;a++)s[a].updateWorldMatrix(!1,!0,n)}}toJSON(e){const t=e===void 0||typeof e=="string",n={};t&&(e={geometries:{},materials:{},textures:{},images:{},shapes:{},skeletons:{},animations:{},nodes:{}},n.metadata={version:4.7,type:"Object",generator:"Object3D.toJSON"});const r={};r.uuid=this.uuid,r.type=this.type,this.name!==""&&(r.name=this.name),this.castShadow===!0&&(r.castShadow=!0),this.receiveShadow===!0&&(r.receiveShadow=!0),this.visible===!1&&(r.visible=!1),this.frustumCulled===!1&&(r.frustumCulled=!1),this.renderOrder!==0&&(r.renderOrder=this.renderOrder),this.static!==!1&&(r.static=this.static),Object.keys(this.userData).length>0&&(r.userData=this.userData),r.layers=this.layers.mask,r.matrix=this.matrix.toArray(),r.up=this.up.toArray(),this.pivot!==null&&(r.pivot=this.pivot.toArray()),this.matrixAutoUpdate===!1&&(r.matrixAutoUpdate=!1),this.morphTargetDictionary!==void 0&&(r.morphTargetDictionary=Object.assign({},this.morphTargetDictionary)),this.morphTargetInfluences!==void 0&&(r.morphTargetInfluences=this.morphTargetInfluences.slice()),this.isInstancedMesh&&(r.type="InstancedMesh",r.count=this.count,r.instanceMatrix=this.instanceMatrix.toJSON(),this.instanceColor!==null&&(r.instanceColor=this.instanceColor.toJSON())),this.isBatchedMesh&&(r.type="BatchedMesh",r.perObjectFrustumCulled=this.perObjectFrustumCulled,r.sortObjects=this.sortObjects,r.drawRanges=this._drawRanges,r.reservedRanges=this._reservedRanges,r.geometryInfo=this._geometryInfo.map(o=>({...o,boundingBox:o.boundingBox?o.boundingBox.toJSON():void 0,boundingSphere:o.boundingSphere?o.boundingSphere.toJSON():void 0})),r.instanceInfo=this._instanceInfo.map(o=>({...o})),r.availableInstanceIds=this._availableInstanceIds.slice(),r.availableGeometryIds=this._availableGeometryIds.slice(),r.nextIndexStart=this._nextIndexStart,r.nextVertexStart=this._nextVertexStart,r.geometryCount=this._geometryCount,r.maxInstanceCount=this._maxInstanceCount,r.maxVertexCount=this._maxVertexCount,r.maxIndexCount=this._maxIndexCount,r.geometryInitialized=this._geometryInitialized,r.matricesTexture=this._matricesTexture.toJSON(e),r.indirectTexture=this._indirectTexture.toJSON(e),this._colorsTexture!==null&&(r.colorsTexture=this._colorsTexture.toJSON(e)),this.boundingSphere!==null&&(r.boundingSphere=this.boundingSphere.toJSON()),this.boundingBox!==null&&(r.boundingBox=this.boundingBox.toJSON()));function s(o,c){return o[c.uuid]===void 0&&(o[c.uuid]=c.toJSON(e)),c.uuid}if(this.isScene)this.background&&(this.background.isColor?r.background=this.background.toJSON():this.background.isTexture&&(r.background=this.background.toJSON(e).uuid)),this.environment&&this.environment.isTexture&&this.environment.isRenderTargetTexture!==!0&&(r.environment=this.environment.toJSON(e).uuid);else if(this.isMesh||this.isLine||this.isPoints){r.geometry=s(e.geometries,this.geometry);const o=this.geometry.parameters;if(o!==void 0&&o.shapes!==void 0){const c=o.shapes;if(Array.isArray(c))for(let l=0,u=c.length;l<u;l++){const f=c[l];s(e.shapes,f)}else s(e.shapes,c)}}if(this.isSkinnedMesh&&(r.bindMode=this.bindMode,r.bindMatrix=this.bindMatrix.toArray(),this.skeleton!==void 0&&(s(e.skeletons,this.skeleton),r.skeleton=this.skeleton.uuid)),this.material!==void 0)if(Array.isArray(this.material)){const o=[];for(let c=0,l=this.material.length;c<l;c++)o.push(s(e.materials,this.material[c]));r.material=o}else r.material=s(e.materials,this.material);if(this.children.length>0){r.children=[];for(let o=0;o<this.children.length;o++)r.children.push(this.children[o].toJSON(e).object)}if(this.animations.length>0){r.animations=[];for(let o=0;o<this.animations.length;o++){const c=this.animations[o];r.animations.push(s(e.animations,c))}}if(t){const o=a(e.geometries),c=a(e.materials),l=a(e.textures),u=a(e.images),f=a(e.shapes),d=a(e.skeletons),m=a(e.animations),x=a(e.nodes);o.length>0&&(n.geometries=o),c.length>0&&(n.materials=c),l.length>0&&(n.textures=l),u.length>0&&(n.images=u),f.length>0&&(n.shapes=f),d.length>0&&(n.skeletons=d),m.length>0&&(n.animations=m),x.length>0&&(n.nodes=x)}return n.object=r,n;function a(o){const c=[];for(const l in o){const u=o[l];delete u.metadata,c.push(u)}return c}}clone(e){return new this.constructor().copy(this,e)}copy(e,t=!0){if(this.name=e.name,this.up.copy(e.up),this.position.copy(e.position),this.rotation.order=e.rotation.order,this.quaternion.copy(e.quaternion),this.scale.copy(e.scale),this.pivot=e.pivot!==null?e.pivot.clone():null,this.matrix.copy(e.matrix),this.matrixWorld.copy(e.matrixWorld),this.matrixAutoUpdate=e.matrixAutoUpdate,this.matrixWorldAutoUpdate=e.matrixWorldAutoUpdate,this.matrixWorldNeedsUpdate=e.matrixWorldNeedsUpdate,this.layers.mask=e.layers.mask,this.visible=e.visible,this.castShadow=e.castShadow,this.receiveShadow=e.receiveShadow,this.frustumCulled=e.frustumCulled,this.renderOrder=e.renderOrder,this.static=e.static,this.animations=e.animations.slice(),this.userData=JSON.parse(JSON.stringify(e.userData)),t===!0)for(let n=0;n<e.children.length;n++){const r=e.children[n];this.add(r.clone())}return this}}mn.DEFAULT_UP=new K(0,1,0);mn.DEFAULT_MATRIX_AUTO_UPDATE=!0;mn.DEFAULT_MATRIX_WORLD_AUTO_UPDATE=!0;class Kr extends mn{constructor(){super(),this.isGroup=!0,this.type="Group"}}const au={type:"move"};class ea{constructor(){this._targetRay=null,this._grip=null,this._hand=null}getHandSpace(){return this._hand===null&&(this._hand=new Kr,this._hand.matrixAutoUpdate=!1,this._hand.visible=!1,this._hand.joints={},this._hand.inputState={pinching:!1}),this._hand}getTargetRaySpace(){return this._targetRay===null&&(this._targetRay=new Kr,this._targetRay.matrixAutoUpdate=!1,this._targetRay.visible=!1,this._targetRay.hasLinearVelocity=!1,this._targetRay.linearVelocity=new K,this._targetRay.hasAngularVelocity=!1,this._targetRay.angularVelocity=new K),this._targetRay}getGripSpace(){return this._grip===null&&(this._grip=new Kr,this._grip.matrixAutoUpdate=!1,this._grip.visible=!1,this._grip.hasLinearVelocity=!1,this._grip.linearVelocity=new K,this._grip.hasAngularVelocity=!1,this._grip.angularVelocity=new K,this._grip.eventsEnabled=!1),this._grip}dispatchEvent(e){return this._targetRay!==null&&this._targetRay.dispatchEvent(e),this._grip!==null&&this._grip.dispatchEvent(e),this._hand!==null&&this._hand.dispatchEvent(e),this}connect(e){if(e&&e.hand){const t=this._hand;if(t)for(const n of e.hand.values())this._getHandJoint(t,n)}return this.dispatchEvent({type:"connected",data:e}),this}disconnect(e){return this.dispatchEvent({type:"disconnected",data:e}),this._targetRay!==null&&(this._targetRay.visible=!1),this._grip!==null&&(this._grip.visible=!1),this._hand!==null&&(this._hand.visible=!1),this}update(e,t,n){let r=null,s=null,a=null;const o=this._targetRay,c=this._grip,l=this._hand;if(e&&t.session.visibilityState!=="visible-blurred"){if(l&&e.hand){a=!0;for(const S of e.hand.values()){const g=t.getJointPose(S,n),h=this._getHandJoint(l,S);g!==null&&(h.matrix.fromArray(g.transform.matrix),h.matrix.decompose(h.position,h.rotation,h.scale),h.matrixWorldNeedsUpdate=!0,h.jointRadius=g.radius),h.visible=g!==null}const u=l.joints["index-finger-tip"],f=l.joints["thumb-tip"],d=u.position.distanceTo(f.position),m=.02,x=.005;l.inputState.pinching&&d>m+x?(l.inputState.pinching=!1,this.dispatchEvent({type:"pinchend",handedness:e.handedness,target:this})):!l.inputState.pinching&&d<=m-x&&(l.inputState.pinching=!0,this.dispatchEvent({type:"pinchstart",handedness:e.handedness,target:this}))}else c!==null&&e.gripSpace&&(s=t.getPose(e.gripSpace,n),s!==null&&(c.matrix.fromArray(s.transform.matrix),c.matrix.decompose(c.position,c.rotation,c.scale),c.matrixWorldNeedsUpdate=!0,s.linearVelocity?(c.hasLinearVelocity=!0,c.linearVelocity.copy(s.linearVelocity)):c.hasLinearVelocity=!1,s.angularVelocity?(c.hasAngularVelocity=!0,c.angularVelocity.copy(s.angularVelocity)):c.hasAngularVelocity=!1,c.eventsEnabled&&c.dispatchEvent({type:"gripUpdated",data:e,target:this})));o!==null&&(r=t.getPose(e.targetRaySpace,n),r===null&&s!==null&&(r=s),r!==null&&(o.matrix.fromArray(r.transform.matrix),o.matrix.decompose(o.position,o.rotation,o.scale),o.matrixWorldNeedsUpdate=!0,r.linearVelocity?(o.hasLinearVelocity=!0,o.linearVelocity.copy(r.linearVelocity)):o.hasLinearVelocity=!1,r.angularVelocity?(o.hasAngularVelocity=!0,o.angularVelocity.copy(r.angularVelocity)):o.hasAngularVelocity=!1,this.dispatchEvent(au)))}return o!==null&&(o.visible=r!==null),c!==null&&(c.visible=s!==null),l!==null&&(l.visible=a!==null),this}_getHandJoint(e,t){if(e.joints[t.jointName]===void 0){const n=new Kr;n.matrixAutoUpdate=!1,n.visible=!1,e.joints[t.jointName]=n,e.add(n)}return e.joints[t.jointName]}}const mc={aliceblue:15792383,antiquewhite:16444375,aqua:65535,aquamarine:8388564,azure:15794175,beige:16119260,bisque:16770244,black:0,blanchedalmond:16772045,blue:255,blueviolet:9055202,brown:10824234,burlywood:14596231,cadetblue:6266528,chartreuse:8388352,chocolate:13789470,coral:16744272,cornflowerblue:6591981,cornsilk:16775388,crimson:14423100,cyan:65535,darkblue:139,darkcyan:35723,darkgoldenrod:12092939,darkgray:11119017,darkgreen:25600,darkgrey:11119017,darkkhaki:12433259,darkmagenta:9109643,darkolivegreen:5597999,darkorange:16747520,darkorchid:10040012,darkred:9109504,darksalmon:15308410,darkseagreen:9419919,darkslateblue:4734347,darkslategray:3100495,darkslategrey:3100495,darkturquoise:52945,darkviolet:9699539,deeppink:16716947,deepskyblue:49151,dimgray:6908265,dimgrey:6908265,dodgerblue:2003199,firebrick:11674146,floralwhite:16775920,forestgreen:2263842,fuchsia:16711935,gainsboro:14474460,ghostwhite:16316671,gold:16766720,goldenrod:14329120,gray:8421504,green:32768,greenyellow:11403055,grey:8421504,honeydew:15794160,hotpink:16738740,indianred:13458524,indigo:4915330,ivory:16777200,khaki:15787660,lavender:15132410,lavenderblush:16773365,lawngreen:8190976,lemonchiffon:16775885,lightblue:11393254,lightcoral:15761536,lightcyan:14745599,lightgoldenrodyellow:16448210,lightgray:13882323,lightgreen:9498256,lightgrey:13882323,lightpink:16758465,lightsalmon:16752762,lightseagreen:2142890,lightskyblue:8900346,lightslategray:7833753,lightslategrey:7833753,lightsteelblue:11584734,lightyellow:16777184,lime:65280,limegreen:3329330,linen:16445670,magenta:16711935,maroon:8388608,mediumaquamarine:6737322,mediumblue:205,mediumorchid:12211667,mediumpurple:9662683,mediumseagreen:3978097,mediumslateblue:8087790,mediumspringgreen:64154,mediumturquoise:4772300,mediumvioletred:13047173,midnightblue:1644912,mintcream:16121850,mistyrose:16770273,moccasin:16770229,navajowhite:16768685,navy:128,oldlace:16643558,olive:8421376,olivedrab:7048739,orange:16753920,orangered:16729344,orchid:14315734,palegoldenrod:15657130,palegreen:10025880,paleturquoise:11529966,palevioletred:14381203,papayawhip:16773077,peachpuff:16767673,peru:13468991,pink:16761035,plum:14524637,powderblue:11591910,purple:8388736,rebeccapurple:6697881,red:16711680,rosybrown:12357519,royalblue:4286945,saddlebrown:9127187,salmon:16416882,sandybrown:16032864,seagreen:3050327,seashell:16774638,sienna:10506797,silver:12632256,skyblue:8900331,slateblue:6970061,slategray:7372944,slategrey:7372944,snow:16775930,springgreen:65407,steelblue:4620980,tan:13808780,teal:32896,thistle:14204888,tomato:16737095,turquoise:4251856,violet:15631086,wheat:16113331,white:16777215,whitesmoke:16119285,yellow:16776960,yellowgreen:10145074},pi={h:0,s:0,l:0},Zr={h:0,s:0,l:0};function ta(i,e,t){return t<0&&(t+=1),t>1&&(t-=1),t<1/6?i+(e-i)*6*t:t<1/2?e:t<2/3?i+(e-i)*6*(2/3-t):i}class Tt{constructor(e,t,n){return this.isColor=!0,this.r=1,this.g=1,this.b=1,this.set(e,t,n)}set(e,t,n){if(t===void 0&&n===void 0){const r=e;r&&r.isColor?this.copy(r):typeof r=="number"?this.setHex(r):typeof r=="string"&&this.setStyle(r)}else this.setRGB(e,t,n);return this}setScalar(e){return this.r=e,this.g=e,this.b=e,this}setHex(e,t=Sn){return e=Math.floor(e),this.r=(e>>16&255)/255,this.g=(e>>8&255)/255,this.b=(e&255)/255,mt.colorSpaceToWorking(this,t),this}setRGB(e,t,n,r=mt.workingColorSpace){return this.r=e,this.g=t,this.b=n,mt.colorSpaceToWorking(this,r),this}setHSL(e,t,n,r=mt.workingColorSpace){if(e=Yd(e,1),t=gt(t,0,1),n=gt(n,0,1),t===0)this.r=this.g=this.b=n;else{const s=n<=.5?n*(1+t):n+t-n*t,a=2*n-s;this.r=ta(a,s,e+1/3),this.g=ta(a,s,e),this.b=ta(a,s,e-1/3)}return mt.colorSpaceToWorking(this,r),this}setStyle(e,t=Sn){function n(s){s!==void 0&&parseFloat(s)<1&&je("Color: Alpha component of "+e+" will be ignored.")}let r;if(r=/^(\w+)\(([^\)]*)\)/.exec(e)){let s;const a=r[1],o=r[2];switch(a){case"rgb":case"rgba":if(s=/^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(s[4]),this.setRGB(Math.min(255,parseInt(s[1],10))/255,Math.min(255,parseInt(s[2],10))/255,Math.min(255,parseInt(s[3],10))/255,t);if(s=/^\s*(\d+)\%\s*,\s*(\d+)\%\s*,\s*(\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(s[4]),this.setRGB(Math.min(100,parseInt(s[1],10))/100,Math.min(100,parseInt(s[2],10))/100,Math.min(100,parseInt(s[3],10))/100,t);break;case"hsl":case"hsla":if(s=/^\s*(\d*\.?\d+)\s*,\s*(\d*\.?\d+)\%\s*,\s*(\d*\.?\d+)\%\s*(?:,\s*(\d*\.?\d+)\s*)?$/.exec(o))return n(s[4]),this.setHSL(parseFloat(s[1])/360,parseFloat(s[2])/100,parseFloat(s[3])/100,t);break;default:je("Color: Unknown color model "+e)}}else if(r=/^\#([A-Fa-f\d]+)$/.exec(e)){const s=r[1],a=s.length;if(a===3)return this.setRGB(parseInt(s.charAt(0),16)/15,parseInt(s.charAt(1),16)/15,parseInt(s.charAt(2),16)/15,t);if(a===6)return this.setHex(parseInt(s,16),t);je("Color: Invalid hex color "+e)}else if(e&&e.length>0)return this.setColorName(e,t);return this}setColorName(e,t=Sn){const n=mc[e.toLowerCase()];return n!==void 0?this.setHex(n,t):je("Color: Unknown color "+e),this}clone(){return new this.constructor(this.r,this.g,this.b)}copy(e){return this.r=e.r,this.g=e.g,this.b=e.b,this}copySRGBToLinear(e){return this.r=ai(e.r),this.g=ai(e.g),this.b=ai(e.b),this}copyLinearToSRGB(e){return this.r=fr(e.r),this.g=fr(e.g),this.b=fr(e.b),this}convertSRGBToLinear(){return this.copySRGBToLinear(this),this}convertLinearToSRGB(){return this.copyLinearToSRGB(this),this}getHex(e=Sn){return mt.workingToColorSpace(en.copy(this),e),Math.round(gt(en.r*255,0,255))*65536+Math.round(gt(en.g*255,0,255))*256+Math.round(gt(en.b*255,0,255))}getHexString(e=Sn){return("000000"+this.getHex(e).toString(16)).slice(-6)}getHSL(e,t=mt.workingColorSpace){mt.workingToColorSpace(en.copy(this),t);const n=en.r,r=en.g,s=en.b,a=Math.max(n,r,s),o=Math.min(n,r,s);let c,l;const u=(o+a)/2;if(o===a)c=0,l=0;else{const f=a-o;switch(l=u<=.5?f/(a+o):f/(2-a-o),a){case n:c=(r-s)/f+(r<s?6:0);break;case r:c=(s-n)/f+2;break;case s:c=(n-r)/f+4;break}c/=6}return e.h=c,e.s=l,e.l=u,e}getRGB(e,t=mt.workingColorSpace){return mt.workingToColorSpace(en.copy(this),t),e.r=en.r,e.g=en.g,e.b=en.b,e}getStyle(e=Sn){mt.workingToColorSpace(en.copy(this),e);const t=en.r,n=en.g,r=en.b;return e!==Sn?`color(${e} ${t.toFixed(3)} ${n.toFixed(3)} ${r.toFixed(3)})`:`rgb(${Math.round(t*255)},${Math.round(n*255)},${Math.round(r*255)})`}offsetHSL(e,t,n){return this.getHSL(pi),this.setHSL(pi.h+e,pi.s+t,pi.l+n)}add(e){return this.r+=e.r,this.g+=e.g,this.b+=e.b,this}addColors(e,t){return this.r=e.r+t.r,this.g=e.g+t.g,this.b=e.b+t.b,this}addScalar(e){return this.r+=e,this.g+=e,this.b+=e,this}sub(e){return this.r=Math.max(0,this.r-e.r),this.g=Math.max(0,this.g-e.g),this.b=Math.max(0,this.b-e.b),this}multiply(e){return this.r*=e.r,this.g*=e.g,this.b*=e.b,this}multiplyScalar(e){return this.r*=e,this.g*=e,this.b*=e,this}lerp(e,t){return this.r+=(e.r-this.r)*t,this.g+=(e.g-this.g)*t,this.b+=(e.b-this.b)*t,this}lerpColors(e,t,n){return this.r=e.r+(t.r-e.r)*n,this.g=e.g+(t.g-e.g)*n,this.b=e.b+(t.b-e.b)*n,this}lerpHSL(e,t){this.getHSL(pi),e.getHSL(Zr);const n=Ks(pi.h,Zr.h,t),r=Ks(pi.s,Zr.s,t),s=Ks(pi.l,Zr.l,t);return this.setHSL(n,r,s),this}setFromVector3(e){return this.r=e.x,this.g=e.y,this.b=e.z,this}applyMatrix3(e){const t=this.r,n=this.g,r=this.b,s=e.elements;return this.r=s[0]*t+s[3]*n+s[6]*r,this.g=s[1]*t+s[4]*n+s[7]*r,this.b=s[2]*t+s[5]*n+s[8]*r,this}equals(e){return e.r===this.r&&e.g===this.g&&e.b===this.b}fromArray(e,t=0){return this.r=e[t],this.g=e[t+1],this.b=e[t+2],this}toArray(e=[],t=0){return e[t]=this.r,e[t+1]=this.g,e[t+2]=this.b,e}fromBufferAttribute(e,t){return this.r=e.getX(t),this.g=e.getY(t),this.b=e.getZ(t),this}toJSON(){return this.getHex()}*[Symbol.iterator](){yield this.r,yield this.g,yield this.b}}const en=new Tt;Tt.NAMES=mc;class ou extends mn{constructor(){super(),this.isScene=!0,this.type="Scene",this.background=null,this.environment=null,this.fog=null,this.backgroundBlurriness=0,this.backgroundIntensity=1,this.backgroundRotation=new zi,this.environmentIntensity=1,this.environmentRotation=new zi,this.overrideMaterial=null,typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}copy(e,t){return super.copy(e,t),e.background!==null&&(this.background=e.background.clone()),e.environment!==null&&(this.environment=e.environment.clone()),e.fog!==null&&(this.fog=e.fog.clone()),this.backgroundBlurriness=e.backgroundBlurriness,this.backgroundIntensity=e.backgroundIntensity,this.backgroundRotation.copy(e.backgroundRotation),this.environmentIntensity=e.environmentIntensity,this.environmentRotation.copy(e.environmentRotation),e.overrideMaterial!==null&&(this.overrideMaterial=e.overrideMaterial.clone()),this.matrixAutoUpdate=e.matrixAutoUpdate,this}toJSON(e){const t=super.toJSON(e);return this.fog!==null&&(t.object.fog=this.fog.toJSON()),this.backgroundBlurriness>0&&(t.object.backgroundBlurriness=this.backgroundBlurriness),this.backgroundIntensity!==1&&(t.object.backgroundIntensity=this.backgroundIntensity),t.object.backgroundRotation=this.backgroundRotation.toArray(),this.environmentIntensity!==1&&(t.object.environmentIntensity=this.environmentIntensity),t.object.environmentRotation=this.environmentRotation.toArray(),t}}const Tn=new K,Jn=new K,na=new K,Qn=new K,tr=new K,nr=new K,sl=new K,ia=new K,ra=new K,sa=new K,aa=new Ot,oa=new Ot,la=new Ot;class Rn{constructor(e=new K,t=new K,n=new K){this.a=e,this.b=t,this.c=n}static getNormal(e,t,n,r){r.subVectors(n,t),Tn.subVectors(e,t),r.cross(Tn);const s=r.lengthSq();return s>0?r.multiplyScalar(1/Math.sqrt(s)):r.set(0,0,0)}static getBarycoord(e,t,n,r,s){Tn.subVectors(r,t),Jn.subVectors(n,t),na.subVectors(e,t);const a=Tn.dot(Tn),o=Tn.dot(Jn),c=Tn.dot(na),l=Jn.dot(Jn),u=Jn.dot(na),f=a*l-o*o;if(f===0)return s.set(0,0,0),null;const d=1/f,m=(l*c-o*u)*d,x=(a*u-o*c)*d;return s.set(1-m-x,x,m)}static containsPoint(e,t,n,r){return this.getBarycoord(e,t,n,r,Qn)===null?!1:Qn.x>=0&&Qn.y>=0&&Qn.x+Qn.y<=1}static getInterpolation(e,t,n,r,s,a,o,c){return this.getBarycoord(e,t,n,r,Qn)===null?(c.x=0,c.y=0,"z"in c&&(c.z=0),"w"in c&&(c.w=0),null):(c.setScalar(0),c.addScaledVector(s,Qn.x),c.addScaledVector(a,Qn.y),c.addScaledVector(o,Qn.z),c)}static getInterpolatedAttribute(e,t,n,r,s,a){return aa.setScalar(0),oa.setScalar(0),la.setScalar(0),aa.fromBufferAttribute(e,t),oa.fromBufferAttribute(e,n),la.fromBufferAttribute(e,r),a.setScalar(0),a.addScaledVector(aa,s.x),a.addScaledVector(oa,s.y),a.addScaledVector(la,s.z),a}static isFrontFacing(e,t,n,r){return Tn.subVectors(n,t),Jn.subVectors(e,t),Tn.cross(Jn).dot(r)<0}set(e,t,n){return this.a.copy(e),this.b.copy(t),this.c.copy(n),this}setFromPointsAndIndices(e,t,n,r){return this.a.copy(e[t]),this.b.copy(e[n]),this.c.copy(e[r]),this}setFromAttributeAndIndices(e,t,n,r){return this.a.fromBufferAttribute(e,t),this.b.fromBufferAttribute(e,n),this.c.fromBufferAttribute(e,r),this}clone(){return new this.constructor().copy(this)}copy(e){return this.a.copy(e.a),this.b.copy(e.b),this.c.copy(e.c),this}getArea(){return Tn.subVectors(this.c,this.b),Jn.subVectors(this.a,this.b),Tn.cross(Jn).length()*.5}getMidpoint(e){return e.addVectors(this.a,this.b).add(this.c).multiplyScalar(1/3)}getNormal(e){return Rn.getNormal(this.a,this.b,this.c,e)}getPlane(e){return e.setFromCoplanarPoints(this.a,this.b,this.c)}getBarycoord(e,t){return Rn.getBarycoord(e,this.a,this.b,this.c,t)}getInterpolation(e,t,n,r,s){return Rn.getInterpolation(e,this.a,this.b,this.c,t,n,r,s)}containsPoint(e){return Rn.containsPoint(e,this.a,this.b,this.c)}isFrontFacing(e){return Rn.isFrontFacing(this.a,this.b,this.c,e)}intersectsBox(e){return e.intersectsTriangle(this)}closestPointToPoint(e,t){const n=this.a,r=this.b,s=this.c;let a,o;tr.subVectors(r,n),nr.subVectors(s,n),ia.subVectors(e,n);const c=tr.dot(ia),l=nr.dot(ia);if(c<=0&&l<=0)return t.copy(n);ra.subVectors(e,r);const u=tr.dot(ra),f=nr.dot(ra);if(u>=0&&f<=u)return t.copy(r);const d=c*f-u*l;if(d<=0&&c>=0&&u<=0)return a=c/(c-u),t.copy(n).addScaledVector(tr,a);sa.subVectors(e,s);const m=tr.dot(sa),x=nr.dot(sa);if(x>=0&&m<=x)return t.copy(s);const S=m*l-c*x;if(S<=0&&l>=0&&x<=0)return o=l/(l-x),t.copy(n).addScaledVector(nr,o);const g=u*x-m*f;if(g<=0&&f-u>=0&&m-x>=0)return sl.subVectors(s,r),o=(f-u)/(f-u+(m-x)),t.copy(r).addScaledVector(sl,o);const h=1/(g+S+d);return a=S*h,o=d*h,t.copy(n).addScaledVector(tr,a).addScaledVector(nr,o)}equals(e){return e.a.equals(this.a)&&e.b.equals(this.b)&&e.c.equals(this.c)}}class Br{constructor(e=new K(1/0,1/0,1/0),t=new K(-1/0,-1/0,-1/0)){this.isBox3=!0,this.min=e,this.max=t}set(e,t){return this.min.copy(e),this.max.copy(t),this}setFromArray(e){this.makeEmpty();for(let t=0,n=e.length;t<n;t+=3)this.expandByPoint(An.fromArray(e,t));return this}setFromBufferAttribute(e){this.makeEmpty();for(let t=0,n=e.count;t<n;t++)this.expandByPoint(An.fromBufferAttribute(e,t));return this}setFromPoints(e){this.makeEmpty();for(let t=0,n=e.length;t<n;t++)this.expandByPoint(e[t]);return this}setFromCenterAndSize(e,t){const n=An.copy(t).multiplyScalar(.5);return this.min.copy(e).sub(n),this.max.copy(e).add(n),this}setFromObject(e,t=!1){return this.makeEmpty(),this.expandByObject(e,t)}clone(){return new this.constructor().copy(this)}copy(e){return this.min.copy(e.min),this.max.copy(e.max),this}makeEmpty(){return this.min.x=this.min.y=this.min.z=1/0,this.max.x=this.max.y=this.max.z=-1/0,this}isEmpty(){return this.max.x<this.min.x||this.max.y<this.min.y||this.max.z<this.min.z}getCenter(e){return this.isEmpty()?e.set(0,0,0):e.addVectors(this.min,this.max).multiplyScalar(.5)}getSize(e){return this.isEmpty()?e.set(0,0,0):e.subVectors(this.max,this.min)}expandByPoint(e){return this.min.min(e),this.max.max(e),this}expandByVector(e){return this.min.sub(e),this.max.add(e),this}expandByScalar(e){return this.min.addScalar(-e),this.max.addScalar(e),this}expandByObject(e,t=!1){e.updateWorldMatrix(!1,!1);const n=e.geometry;if(n!==void 0){const s=n.getAttribute("position");if(t===!0&&s!==void 0&&e.isInstancedMesh!==!0)for(let a=0,o=s.count;a<o;a++)e.isMesh===!0?e.getVertexPosition(a,An):An.fromBufferAttribute(s,a),An.applyMatrix4(e.matrixWorld),this.expandByPoint(An);else e.boundingBox!==void 0?(e.boundingBox===null&&e.computeBoundingBox(),$r.copy(e.boundingBox)):(n.boundingBox===null&&n.computeBoundingBox(),$r.copy(n.boundingBox)),$r.applyMatrix4(e.matrixWorld),this.union($r)}const r=e.children;for(let s=0,a=r.length;s<a;s++)this.expandByObject(r[s],t);return this}containsPoint(e){return e.x>=this.min.x&&e.x<=this.max.x&&e.y>=this.min.y&&e.y<=this.max.y&&e.z>=this.min.z&&e.z<=this.max.z}containsBox(e){return this.min.x<=e.min.x&&e.max.x<=this.max.x&&this.min.y<=e.min.y&&e.max.y<=this.max.y&&this.min.z<=e.min.z&&e.max.z<=this.max.z}getParameter(e,t){return t.set((e.x-this.min.x)/(this.max.x-this.min.x),(e.y-this.min.y)/(this.max.y-this.min.y),(e.z-this.min.z)/(this.max.z-this.min.z))}intersectsBox(e){return e.max.x>=this.min.x&&e.min.x<=this.max.x&&e.max.y>=this.min.y&&e.min.y<=this.max.y&&e.max.z>=this.min.z&&e.min.z<=this.max.z}intersectsSphere(e){return this.clampPoint(e.center,An),An.distanceToSquared(e.center)<=e.radius*e.radius}intersectsPlane(e){let t,n;return e.normal.x>0?(t=e.normal.x*this.min.x,n=e.normal.x*this.max.x):(t=e.normal.x*this.max.x,n=e.normal.x*this.min.x),e.normal.y>0?(t+=e.normal.y*this.min.y,n+=e.normal.y*this.max.y):(t+=e.normal.y*this.max.y,n+=e.normal.y*this.min.y),e.normal.z>0?(t+=e.normal.z*this.min.z,n+=e.normal.z*this.max.z):(t+=e.normal.z*this.max.z,n+=e.normal.z*this.min.z),t<=-e.constant&&n>=-e.constant}intersectsTriangle(e){if(this.isEmpty())return!1;this.getCenter(Tr),Jr.subVectors(this.max,Tr),ir.subVectors(e.a,Tr),rr.subVectors(e.b,Tr),sr.subVectors(e.c,Tr),mi.subVectors(rr,ir),gi.subVectors(sr,rr),Ci.subVectors(ir,sr);let t=[0,-mi.z,mi.y,0,-gi.z,gi.y,0,-Ci.z,Ci.y,mi.z,0,-mi.x,gi.z,0,-gi.x,Ci.z,0,-Ci.x,-mi.y,mi.x,0,-gi.y,gi.x,0,-Ci.y,Ci.x,0];return!ca(t,ir,rr,sr,Jr)||(t=[1,0,0,0,1,0,0,0,1],!ca(t,ir,rr,sr,Jr))?!1:(Qr.crossVectors(mi,gi),t=[Qr.x,Qr.y,Qr.z],ca(t,ir,rr,sr,Jr))}clampPoint(e,t){return t.copy(e).clamp(this.min,this.max)}distanceToPoint(e){return this.clampPoint(e,An).distanceTo(e)}getBoundingSphere(e){return this.isEmpty()?e.makeEmpty():(this.getCenter(e.center),e.radius=this.getSize(An).length()*.5),e}intersect(e){return this.min.max(e.min),this.max.min(e.max),this.isEmpty()&&this.makeEmpty(),this}union(e){return this.min.min(e.min),this.max.max(e.max),this}applyMatrix4(e){return this.isEmpty()?this:(jn[0].set(this.min.x,this.min.y,this.min.z).applyMatrix4(e),jn[1].set(this.min.x,this.min.y,this.max.z).applyMatrix4(e),jn[2].set(this.min.x,this.max.y,this.min.z).applyMatrix4(e),jn[3].set(this.min.x,this.max.y,this.max.z).applyMatrix4(e),jn[4].set(this.max.x,this.min.y,this.min.z).applyMatrix4(e),jn[5].set(this.max.x,this.min.y,this.max.z).applyMatrix4(e),jn[6].set(this.max.x,this.max.y,this.min.z).applyMatrix4(e),jn[7].set(this.max.x,this.max.y,this.max.z).applyMatrix4(e),this.setFromPoints(jn),this)}translate(e){return this.min.add(e),this.max.add(e),this}equals(e){return e.min.equals(this.min)&&e.max.equals(this.max)}toJSON(){return{min:this.min.toArray(),max:this.max.toArray()}}fromJSON(e){return this.min.fromArray(e.min),this.max.fromArray(e.max),this}}const jn=[new K,new K,new K,new K,new K,new K,new K,new K],An=new K,$r=new Br,ir=new K,rr=new K,sr=new K,mi=new K,gi=new K,Ci=new K,Tr=new K,Jr=new K,Qr=new K,Pi=new K;function ca(i,e,t,n,r){for(let s=0,a=i.length-3;s<=a;s+=3){Pi.fromArray(i,s);const o=r.x*Math.abs(Pi.x)+r.y*Math.abs(Pi.y)+r.z*Math.abs(Pi.z),c=e.dot(Pi),l=t.dot(Pi),u=n.dot(Pi);if(Math.max(-Math.max(c,l,u),Math.min(c,l,u))>o)return!1}return!0}const Gt=new K,jr=new Mt;let lu=0;class Ln extends Gi{constructor(e,t,n=!1){if(super(),Array.isArray(e))throw new TypeError("THREE.BufferAttribute: array should be a Typed Array.");this.isBufferAttribute=!0,Object.defineProperty(this,"id",{value:lu++}),this.name="",this.array=e,this.itemSize=t,this.count=e!==void 0?e.length/t:0,this.normalized=n,this.usage=po,this.updateRanges=[],this.gpuType=Vn,this.version=0}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.name=e.name,this.array=new e.array.constructor(e.array),this.itemSize=e.itemSize,this.count=e.count,this.normalized=e.normalized,this.usage=e.usage,this.gpuType=e.gpuType,this}copyAt(e,t,n){e*=this.itemSize,n*=t.itemSize;for(let r=0,s=this.itemSize;r<s;r++)this.array[e+r]=t.array[n+r];return this}copyArray(e){return this.array.set(e),this}applyMatrix3(e){if(this.itemSize===2)for(let t=0,n=this.count;t<n;t++)jr.fromBufferAttribute(this,t),jr.applyMatrix3(e),this.setXY(t,jr.x,jr.y);else if(this.itemSize===3)for(let t=0,n=this.count;t<n;t++)Gt.fromBufferAttribute(this,t),Gt.applyMatrix3(e),this.setXYZ(t,Gt.x,Gt.y,Gt.z);return this}applyMatrix4(e){for(let t=0,n=this.count;t<n;t++)Gt.fromBufferAttribute(this,t),Gt.applyMatrix4(e),this.setXYZ(t,Gt.x,Gt.y,Gt.z);return this}applyNormalMatrix(e){for(let t=0,n=this.count;t<n;t++)Gt.fromBufferAttribute(this,t),Gt.applyNormalMatrix(e),this.setXYZ(t,Gt.x,Gt.y,Gt.z);return this}transformDirection(e){for(let t=0,n=this.count;t<n;t++)Gt.fromBufferAttribute(this,t),Gt.transformDirection(e),this.setXYZ(t,Gt.x,Gt.y,Gt.z);return this}set(e,t=0){return this.array.set(e,t),this}getComponent(e,t){let n=this.array[e*this.itemSize+t];return this.normalized&&(n=Gn(n,this.array)),n}setComponent(e,t,n){return this.normalized&&(n=Pt(n,this.array)),this.array[e*this.itemSize+t]=n,this}getX(e){let t=this.array[e*this.itemSize];return this.normalized&&(t=Gn(t,this.array)),t}setX(e,t){return this.normalized&&(t=Pt(t,this.array)),this.array[e*this.itemSize]=t,this}getY(e){let t=this.array[e*this.itemSize+1];return this.normalized&&(t=Gn(t,this.array)),t}setY(e,t){return this.normalized&&(t=Pt(t,this.array)),this.array[e*this.itemSize+1]=t,this}getZ(e){let t=this.array[e*this.itemSize+2];return this.normalized&&(t=Gn(t,this.array)),t}setZ(e,t){return this.normalized&&(t=Pt(t,this.array)),this.array[e*this.itemSize+2]=t,this}getW(e){let t=this.array[e*this.itemSize+3];return this.normalized&&(t=Gn(t,this.array)),t}setW(e,t){return this.normalized&&(t=Pt(t,this.array)),this.array[e*this.itemSize+3]=t,this}setXY(e,t,n){return e*=this.itemSize,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array)),this.array[e+0]=t,this.array[e+1]=n,this}setXYZ(e,t,n,r){return e*=this.itemSize,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array),r=Pt(r,this.array)),this.array[e+0]=t,this.array[e+1]=n,this.array[e+2]=r,this}setXYZW(e,t,n,r,s){return e*=this.itemSize,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array),r=Pt(r,this.array),s=Pt(s,this.array)),this.array[e+0]=t,this.array[e+1]=n,this.array[e+2]=r,this.array[e+3]=s,this}onUpload(e){return this.onUploadCallback=e,this}clone(){return new this.constructor(this.array,this.itemSize).copy(this)}toJSON(){const e={itemSize:this.itemSize,type:this.array.constructor.name,array:Array.from(this.array),normalized:this.normalized};return this.name!==""&&(e.name=this.name),this.usage!==po&&(e.usage=this.usage),e}dispose(){this.dispatchEvent({type:"dispose"})}}class gc extends Ln{constructor(e,t,n){super(new Uint16Array(e),t,n)}}class _c extends Ln{constructor(e,t,n){super(new Uint32Array(e),t,n)}}class oi extends Ln{constructor(e,t,n){super(new Float32Array(e),t,n)}}const cu=new Br,Ar=new K,da=new K;class Lo{constructor(e=new K,t=-1){this.isSphere=!0,this.center=e,this.radius=t}set(e,t){return this.center.copy(e),this.radius=t,this}setFromPoints(e,t){const n=this.center;t!==void 0?n.copy(t):cu.setFromPoints(e).getCenter(n);let r=0;for(let s=0,a=e.length;s<a;s++)r=Math.max(r,n.distanceToSquared(e[s]));return this.radius=Math.sqrt(r),this}copy(e){return this.center.copy(e.center),this.radius=e.radius,this}isEmpty(){return this.radius<0}makeEmpty(){return this.center.set(0,0,0),this.radius=-1,this}containsPoint(e){return e.distanceToSquared(this.center)<=this.radius*this.radius}distanceToPoint(e){return e.distanceTo(this.center)-this.radius}intersectsSphere(e){const t=this.radius+e.radius;return e.center.distanceToSquared(this.center)<=t*t}intersectsBox(e){return e.intersectsSphere(this)}intersectsPlane(e){return Math.abs(e.distanceToPoint(this.center))<=this.radius}clampPoint(e,t){const n=this.center.distanceToSquared(e);return t.copy(e),n>this.radius*this.radius&&(t.sub(this.center).normalize(),t.multiplyScalar(this.radius).add(this.center)),t}getBoundingBox(e){return this.isEmpty()?(e.makeEmpty(),e):(e.set(this.center,this.center),e.expandByScalar(this.radius),e)}applyMatrix4(e){return this.center.applyMatrix4(e),this.radius=this.radius*e.getMaxScaleOnAxis(),this}translate(e){return this.center.add(e),this}expandByPoint(e){if(this.isEmpty())return this.center.copy(e),this.radius=0,this;Ar.subVectors(e,this.center);const t=Ar.lengthSq();if(t>this.radius*this.radius){const n=Math.sqrt(t),r=(n-this.radius)*.5;this.center.addScaledVector(Ar,r/n),this.radius+=r}return this}union(e){return e.isEmpty()?this:this.isEmpty()?(this.copy(e),this):(this.center.equals(e.center)===!0?this.radius=Math.max(this.radius,e.radius):(da.subVectors(e.center,this.center).setLength(e.radius),this.expandByPoint(Ar.copy(e.center).add(da)),this.expandByPoint(Ar.copy(e.center).sub(da))),this)}equals(e){return e.center.equals(this.center)&&e.radius===this.radius}clone(){return new this.constructor().copy(this)}toJSON(){return{radius:this.radius,center:this.center.toArray()}}fromJSON(e){return this.radius=e.radius,this.center.fromArray(e.center),this}}let du=0;const vn=new Vt,ua=new mn,ar=new K,pn=new Br,wr=new Br,Kt=new K;class Kn extends Gi{constructor(){super(),this.isBufferGeometry=!0,Object.defineProperty(this,"id",{value:du++}),this.uuid=Si(),this.name="",this.type="BufferGeometry",this.index=null,this.indirect=null,this.indirectOffset=0,this.attributes={},this.morphAttributes={},this.morphTargetsRelative=!1,this.groups=[],this.boundingBox=null,this.boundingSphere=null,this.drawRange={start:0,count:1/0},this.userData={},this._transformed=!1}getIndex(){return this.index}setIndex(e){return Array.isArray(e)?this.index=new(Hd(e)?_c:gc)(e,1):this.index=e,this}setIndirect(e,t=0){return this.indirect=e,this.indirectOffset=t,this}getIndirect(){return this.indirect}getAttribute(e){return this.attributes[e]}setAttribute(e,t){return this.attributes[e]=t,this}deleteAttribute(e){return delete this.attributes[e],this}hasAttribute(e){return this.attributes[e]!==void 0}addGroup(e,t,n=0){this.groups.push({start:e,count:t,materialIndex:n})}clearGroups(){this.groups=[]}setDrawRange(e,t){this.drawRange.start=e,this.drawRange.count=t}applyMatrix4(e){const t=this.attributes.position;t!==void 0&&(t.applyMatrix4(e),t.needsUpdate=!0);const n=this.attributes.normal;if(n!==void 0){const s=new nt().getNormalMatrix(e);n.applyNormalMatrix(s),n.needsUpdate=!0}const r=this.attributes.tangent;return r!==void 0&&(r.transformDirection(e),r.needsUpdate=!0),this.boundingBox!==null&&this.computeBoundingBox(),this.boundingSphere!==null&&this.computeBoundingSphere(),this._transformed=!0,this}applyQuaternion(e){return vn.makeRotationFromQuaternion(e),this.applyMatrix4(vn),this}rotateX(e){return vn.makeRotationX(e),this.applyMatrix4(vn),this}rotateY(e){return vn.makeRotationY(e),this.applyMatrix4(vn),this}rotateZ(e){return vn.makeRotationZ(e),this.applyMatrix4(vn),this}translate(e,t,n){return vn.makeTranslation(e,t,n),this.applyMatrix4(vn),this}scale(e,t,n){return vn.makeScale(e,t,n),this.applyMatrix4(vn),this}lookAt(e){return ua.lookAt(e),ua.updateMatrix(),this.applyMatrix4(ua.matrix),this}center(){return this.computeBoundingBox(),this.boundingBox.getCenter(ar).negate(),this.translate(ar.x,ar.y,ar.z),this}setFromPoints(e){const t=this.getAttribute("position");if(t===void 0){const n=[];for(let r=0,s=e.length;r<s;r++){const a=e[r];n.push(a.x,a.y,a.z||0)}this.setAttribute("position",new oi(n,3))}else{const n=Math.min(e.length,t.count);for(let r=0;r<n;r++){const s=e[r];t.setXYZ(r,s.x,s.y,s.z||0)}e.length>t.count&&je("BufferGeometry: Buffer size too small for points data. Use .dispose() and create a new geometry."),t.needsUpdate=!0}return this}computeBoundingBox(){this.boundingBox===null&&(this.boundingBox=new Br);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){_t("BufferGeometry.computeBoundingBox(): GLBufferAttribute requires a manual bounding box.",this),this.boundingBox.set(new K(-1/0,-1/0,-1/0),new K(1/0,1/0,1/0));return}if(e!==void 0){if(this.boundingBox.setFromBufferAttribute(e),t)for(let n=0,r=t.length;n<r;n++){const s=t[n];pn.setFromBufferAttribute(s),this.morphTargetsRelative?(Kt.addVectors(this.boundingBox.min,pn.min),this.boundingBox.expandByPoint(Kt),Kt.addVectors(this.boundingBox.max,pn.max),this.boundingBox.expandByPoint(Kt)):(this.boundingBox.expandByPoint(pn.min),this.boundingBox.expandByPoint(pn.max))}}else this.boundingBox.makeEmpty();(isNaN(this.boundingBox.min.x)||isNaN(this.boundingBox.min.y)||isNaN(this.boundingBox.min.z))&&_t('BufferGeometry.computeBoundingBox(): Computed min/max have NaN values. The "position" attribute is likely to have NaN values.',this)}computeBoundingSphere(){this.boundingSphere===null&&(this.boundingSphere=new Lo);const e=this.attributes.position,t=this.morphAttributes.position;if(e&&e.isGLBufferAttribute){_t("BufferGeometry.computeBoundingSphere(): GLBufferAttribute requires a manual bounding sphere.",this),this.boundingSphere.set(new K,1/0);return}if(e){const n=this.boundingSphere.center;if(pn.setFromBufferAttribute(e),t)for(let s=0,a=t.length;s<a;s++){const o=t[s];wr.setFromBufferAttribute(o),this.morphTargetsRelative?(Kt.addVectors(pn.min,wr.min),pn.expandByPoint(Kt),Kt.addVectors(pn.max,wr.max),pn.expandByPoint(Kt)):(pn.expandByPoint(wr.min),pn.expandByPoint(wr.max))}pn.getCenter(n);let r=0;for(let s=0,a=e.count;s<a;s++)Kt.fromBufferAttribute(e,s),r=Math.max(r,n.distanceToSquared(Kt));if(t)for(let s=0,a=t.length;s<a;s++){const o=t[s],c=this.morphTargetsRelative;for(let l=0,u=o.count;l<u;l++)Kt.fromBufferAttribute(o,l),c&&(ar.fromBufferAttribute(e,l),Kt.add(ar)),r=Math.max(r,n.distanceToSquared(Kt))}this.boundingSphere.radius=Math.sqrt(r),isNaN(this.boundingSphere.radius)&&_t('BufferGeometry.computeBoundingSphere(): Computed radius is NaN. The "position" attribute is likely to have NaN values.',this)}}computeTangents(){const e=this.index,t=this.attributes;if(e===null||t.position===void 0||t.normal===void 0||t.uv===void 0){_t("BufferGeometry: .computeTangents() failed. Missing required attributes (index, position, normal or uv)");return}const n=t.position,r=t.normal,s=t.uv;let a=this.getAttribute("tangent");(a===void 0||a.count!==n.count)&&(a=new Ln(new Float32Array(4*n.count),4),this.setAttribute("tangent",a));const o=[],c=[];for(let _=0;_<n.count;_++)o[_]=new K,c[_]=new K;const l=new K,u=new K,f=new K,d=new Mt,m=new Mt,x=new Mt,S=new K,g=new K;function h(_,A,k){l.fromBufferAttribute(n,_),u.fromBufferAttribute(n,A),f.fromBufferAttribute(n,k),d.fromBufferAttribute(s,_),m.fromBufferAttribute(s,A),x.fromBufferAttribute(s,k),u.sub(l),f.sub(l),m.sub(d),x.sub(d);const F=1/(m.x*x.y-x.x*m.y);isFinite(F)&&(S.copy(u).multiplyScalar(x.y).addScaledVector(f,-m.y).multiplyScalar(F),g.copy(f).multiplyScalar(m.x).addScaledVector(u,-x.x).multiplyScalar(F),o[_].add(S),o[A].add(S),o[k].add(S),c[_].add(g),c[A].add(g),c[k].add(g))}let R=this.groups;R.length===0&&(R=[{start:0,count:e.count}]);for(let _=0,A=R.length;_<A;++_){const k=R[_],F=k.start,b=k.count;for(let P=F,W=F+b;P<W;P+=3)h(e.getX(P+0),e.getX(P+1),e.getX(P+2))}const C=new K,y=new K,w=new K,T=new K;function I(_){w.fromBufferAttribute(r,_),T.copy(w);const A=o[_];C.copy(A),C.sub(w.multiplyScalar(w.dot(A))).normalize(),y.crossVectors(T,A);const F=y.dot(c[_])<0?-1:1;a.setXYZW(_,C.x,C.y,C.z,F)}for(let _=0,A=R.length;_<A;++_){const k=R[_],F=k.start,b=k.count;for(let P=F,W=F+b;P<W;P+=3)I(e.getX(P+0)),I(e.getX(P+1)),I(e.getX(P+2))}this._transformed=!0}computeVertexNormals(){const e=this.index,t=this.getAttribute("position");if(t!==void 0){let n=this.getAttribute("normal");if(n===void 0||n.count!==t.count)n=new Ln(new Float32Array(t.count*3),3),this.setAttribute("normal",n);else for(let d=0,m=n.count;d<m;d++)n.setXYZ(d,0,0,0);const r=new K,s=new K,a=new K,o=new K,c=new K,l=new K,u=new K,f=new K;if(e)for(let d=0,m=e.count;d<m;d+=3){const x=e.getX(d+0),S=e.getX(d+1),g=e.getX(d+2);r.fromBufferAttribute(t,x),s.fromBufferAttribute(t,S),a.fromBufferAttribute(t,g),u.subVectors(a,s),f.subVectors(r,s),u.cross(f),o.fromBufferAttribute(n,x),c.fromBufferAttribute(n,S),l.fromBufferAttribute(n,g),o.add(u),c.add(u),l.add(u),n.setXYZ(x,o.x,o.y,o.z),n.setXYZ(S,c.x,c.y,c.z),n.setXYZ(g,l.x,l.y,l.z)}else for(let d=0,m=t.count;d<m;d+=3)r.fromBufferAttribute(t,d+0),s.fromBufferAttribute(t,d+1),a.fromBufferAttribute(t,d+2),u.subVectors(a,s),f.subVectors(r,s),u.cross(f),n.setXYZ(d+0,u.x,u.y,u.z),n.setXYZ(d+1,u.x,u.y,u.z),n.setXYZ(d+2,u.x,u.y,u.z);this.normalizeNormals(),n.needsUpdate=!0}}normalizeNormals(){const e=this.attributes.normal;for(let t=0,n=e.count;t<n;t++)Kt.fromBufferAttribute(e,t),Kt.normalize(),e.setXYZ(t,Kt.x,Kt.y,Kt.z)}toNonIndexed(){function e(o,c){const l=o.array,u=o.itemSize,f=o.normalized,d=new l.constructor(c.length*u);let m=0,x=0;for(let S=0,g=c.length;S<g;S++){o.isInterleavedBufferAttribute?m=c[S]*o.data.stride+o.offset:m=c[S]*u;for(let h=0;h<u;h++)d[x++]=l[m++]}return new Ln(d,u,f)}if(this.index===null)return je("BufferGeometry.toNonIndexed(): BufferGeometry is already non-indexed."),this;const t=new Kn,n=this.index.array,r=this.attributes;for(const o in r){const c=r[o],l=e(c,n);t.setAttribute(o,l)}const s=this.morphAttributes;for(const o in s){const c=[],l=s[o];for(let u=0,f=l.length;u<f;u++){const d=l[u],m=e(d,n);c.push(m)}t.morphAttributes[o]=c}t.morphTargetsRelative=this.morphTargetsRelative;const a=this.groups;for(let o=0,c=a.length;o<c;o++){const l=a[o];t.addGroup(l.start,l.count,l.materialIndex)}return t}toJSON(){const e={metadata:{version:4.7,type:"BufferGeometry",generator:"BufferGeometry.toJSON"}};if(e.uuid=this.uuid,e.type=this.parameters!==void 0&&this._transformed===!0?"BufferGeometry":this.type,this.name!==""&&(e.name=this.name),Object.keys(this.userData).length>0&&(e.userData=this.userData),this.parameters!==void 0&&this._transformed!==!0){const c=this.parameters;for(const l in c)c[l]!==void 0&&(e[l]=c[l]);return e}e.data={attributes:{}};const t=this.index;t!==null&&(e.data.index={type:t.array.constructor.name,array:Array.prototype.slice.call(t.array)});const n=this.attributes;for(const c in n){const l=n[c];e.data.attributes[c]=l.toJSON(e.data)}const r={};let s=!1;for(const c in this.morphAttributes){const l=this.morphAttributes[c],u=[];for(let f=0,d=l.length;f<d;f++){const m=l[f];u.push(m.toJSON(e.data))}u.length>0&&(r[c]=u,s=!0)}s&&(e.data.morphAttributes=r,e.data.morphTargetsRelative=this.morphTargetsRelative);const a=this.groups;a.length>0&&(e.data.groups=JSON.parse(JSON.stringify(a)));const o=this.boundingSphere;return o!==null&&(e.data.boundingSphere=o.toJSON()),e}clone(){return new this.constructor().copy(this)}copy(e){this.index=null,this.attributes={},this.morphAttributes={},this.groups=[],this.boundingBox=null,this.boundingSphere=null;const t={};this.name=e.name;const n=e.index;n!==null&&this.setIndex(n.clone());const r=e.attributes;for(const l in r){const u=r[l];this.setAttribute(l,u.clone(t))}const s=e.morphAttributes;for(const l in s){const u=[],f=s[l];for(let d=0,m=f.length;d<m;d++)u.push(f[d].clone(t));this.morphAttributes[l]=u}this.morphTargetsRelative=e.morphTargetsRelative;const a=e.groups;for(let l=0,u=a.length;l<u;l++){const f=a[l];this.addGroup(f.start,f.count,f.materialIndex)}const o=e.boundingBox;o!==null&&(this.boundingBox=o.clone());const c=e.boundingSphere;return c!==null&&(this.boundingSphere=c.clone()),this.drawRange.start=e.drawRange.start,this.drawRange.count=e.drawRange.count,this.userData=e.userData,this._transformed=e._transformed,this}dispose(){this.dispatchEvent({type:"dispose"})}}class uu{constructor(e,t){this.isInterleavedBuffer=!0,this.array=e,this.stride=t,this.count=e!==void 0?e.length/t:0,this.usage=po,this.updateRanges=[],this.version=0,this.uuid=Si()}onUploadCallback(){}set needsUpdate(e){e===!0&&this.version++}setUsage(e){return this.usage=e,this}addUpdateRange(e,t){this.updateRanges.push({start:e,count:t})}clearUpdateRanges(){this.updateRanges.length=0}copy(e){return this.array=new e.array.constructor(e.array),this.count=e.count,this.stride=e.stride,this.usage=e.usage,this}copyAt(e,t,n){e*=this.stride,n*=t.stride;for(let r=0,s=this.stride;r<s;r++)this.array[e+r]=t.array[n+r];return this}set(e,t=0){return this.array.set(e,t),this}clone(e){e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=Si()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=this.array.slice(0).buffer);const t=new this.array.constructor(e.arrayBuffers[this.array.buffer._uuid]),n=new this.constructor(t,this.stride);return n.setUsage(this.usage),n}onUpload(e){return this.onUploadCallback=e,this}toJSON(e){return e.arrayBuffers===void 0&&(e.arrayBuffers={}),this.array.buffer._uuid===void 0&&(this.array.buffer._uuid=Si()),e.arrayBuffers[this.array.buffer._uuid]===void 0&&(e.arrayBuffers[this.array.buffer._uuid]=Array.from(new Uint32Array(this.array.buffer))),{uuid:this.uuid,buffer:this.array.buffer._uuid,type:this.array.constructor.name,stride:this.stride}}}const rn=new K;class Do{constructor(e,t,n,r=!1){this.isInterleavedBufferAttribute=!0,this.name="",this.data=e,this.itemSize=t,this.offset=n,this.normalized=r}get count(){return this.data.count}get array(){return this.data.array}set needsUpdate(e){this.data.needsUpdate=e}applyMatrix4(e){for(let t=0,n=this.data.count;t<n;t++)rn.fromBufferAttribute(this,t),rn.applyMatrix4(e),this.setXYZ(t,rn.x,rn.y,rn.z);return this}applyNormalMatrix(e){for(let t=0,n=this.count;t<n;t++)rn.fromBufferAttribute(this,t),rn.applyNormalMatrix(e),this.setXYZ(t,rn.x,rn.y,rn.z);return this}transformDirection(e){for(let t=0,n=this.count;t<n;t++)rn.fromBufferAttribute(this,t),rn.transformDirection(e),this.setXYZ(t,rn.x,rn.y,rn.z);return this}getComponent(e,t){let n=this.array[e*this.data.stride+this.offset+t];return this.normalized&&(n=Gn(n,this.array)),n}setComponent(e,t,n){return this.normalized&&(n=Pt(n,this.array)),this.data.array[e*this.data.stride+this.offset+t]=n,this}setX(e,t){return this.normalized&&(t=Pt(t,this.array)),this.data.array[e*this.data.stride+this.offset]=t,this}setY(e,t){return this.normalized&&(t=Pt(t,this.array)),this.data.array[e*this.data.stride+this.offset+1]=t,this}setZ(e,t){return this.normalized&&(t=Pt(t,this.array)),this.data.array[e*this.data.stride+this.offset+2]=t,this}setW(e,t){return this.normalized&&(t=Pt(t,this.array)),this.data.array[e*this.data.stride+this.offset+3]=t,this}getX(e){let t=this.data.array[e*this.data.stride+this.offset];return this.normalized&&(t=Gn(t,this.array)),t}getY(e){let t=this.data.array[e*this.data.stride+this.offset+1];return this.normalized&&(t=Gn(t,this.array)),t}getZ(e){let t=this.data.array[e*this.data.stride+this.offset+2];return this.normalized&&(t=Gn(t,this.array)),t}getW(e){let t=this.data.array[e*this.data.stride+this.offset+3];return this.normalized&&(t=Gn(t,this.array)),t}setXY(e,t,n){return e=e*this.data.stride+this.offset,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=n,this}setXYZ(e,t,n,r){return e=e*this.data.stride+this.offset,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array),r=Pt(r,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=n,this.data.array[e+2]=r,this}setXYZW(e,t,n,r,s){return e=e*this.data.stride+this.offset,this.normalized&&(t=Pt(t,this.array),n=Pt(n,this.array),r=Pt(r,this.array),s=Pt(s,this.array)),this.data.array[e+0]=t,this.data.array[e+1]=n,this.data.array[e+2]=r,this.data.array[e+3]=s,this}clone(e){if(e===void 0){Ps("InterleavedBufferAttribute.clone(): Cloning an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let n=0;n<this.count;n++){const r=n*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return new Ln(new this.array.constructor(t),this.itemSize,this.normalized)}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.clone(e)),new Do(e.interleavedBuffers[this.data.uuid],this.itemSize,this.offset,this.normalized)}toJSON(e){if(e===void 0){Ps("InterleavedBufferAttribute.toJSON(): Serializing an interleaved buffer attribute will de-interleave buffer data.");const t=[];for(let n=0;n<this.count;n++){const r=n*this.data.stride+this.offset;for(let s=0;s<this.itemSize;s++)t.push(this.data.array[r+s])}return{itemSize:this.itemSize,type:this.array.constructor.name,array:t,normalized:this.normalized}}else return e.interleavedBuffers===void 0&&(e.interleavedBuffers={}),e.interleavedBuffers[this.data.uuid]===void 0&&(e.interleavedBuffers[this.data.uuid]=this.data.toJSON(e)),{isInterleavedBufferAttribute:!0,itemSize:this.itemSize,data:this.data.uuid,offset:this.offset,normalized:this.normalized}}}let hu=0;class Ns extends Gi{constructor(){super(),this.isMaterial=!0,Object.defineProperty(this,"id",{value:hu++}),this.uuid=Si(),this.name="",this.type="Material",this.blending=ur,this.side=yi,this.vertexColors=!1,this.opacity=1,this.transparent=!1,this.alphaHash=!1,this.blendSrc=wa,this.blendDst=Ur,this.blendEquation=vi,this.blendSrcAlpha=null,this.blendDstAlpha=null,this.blendEquationAlpha=null,this.blendColor=new Tt(0,0,0),this.blendAlpha=0,this.depthFunc=pr,this.depthTest=!0,this.depthWrite=!0,this.stencilWriteMask=255,this.stencilFunc=Yo,this.stencilRef=0,this.stencilFuncMask=255,this.stencilFail=$i,this.stencilZFail=$i,this.stencilZPass=$i,this.stencilWrite=!1,this.clippingPlanes=null,this.clipIntersection=!1,this.clipShadows=!1,this.shadowSide=null,this.colorWrite=!0,this.precision=null,this.polygonOffset=!1,this.polygonOffsetFactor=0,this.polygonOffsetUnits=0,this.dithering=!1,this.alphaToCoverage=!1,this.premultipliedAlpha=!1,this.forceSinglePass=!1,this.allowOverride=!0,this.visible=!0,this.toneMapped=!0,this.userData={},this.version=0,this._alphaTest=0}get alphaTest(){return this._alphaTest}set alphaTest(e){this._alphaTest>0!=e>0&&this.version++,this._alphaTest=e}onBeforeRender(){}onBeforeCompile(){}customProgramCacheKey(){return this.onBeforeCompile.toString()}setValues(e){if(e!==void 0)for(const t in e){const n=e[t];if(n===void 0){je(`Material: parameter '${t}' has value of undefined.`);continue}const r=this[t];if(r===void 0){je(`Material: '${t}' is not a property of THREE.${this.type}.`);continue}r&&r.isColor?r.set(n):r&&r.isVector2&&n&&n.isVector2||r&&r.isEuler&&n&&n.isEuler||r&&r.isVector3&&n&&n.isVector3?r.copy(n):this[t]=n}}toJSON(e){const t=e===void 0||typeof e=="string";t&&(e={textures:{},images:{}});const n={metadata:{version:4.7,type:"Material",generator:"Material.toJSON"}};n.uuid=this.uuid,n.type=this.type,this.name!==""&&(n.name=this.name),this.color&&this.color.isColor&&(n.color=this.color.getHex()),this.roughness!==void 0&&(n.roughness=this.roughness),this.metalness!==void 0&&(n.metalness=this.metalness),this.sheen!==void 0&&(n.sheen=this.sheen),this.sheenColor&&this.sheenColor.isColor&&(n.sheenColor=this.sheenColor.getHex()),this.sheenRoughness!==void 0&&(n.sheenRoughness=this.sheenRoughness),this.emissive&&this.emissive.isColor&&(n.emissive=this.emissive.getHex()),this.emissiveIntensity!==void 0&&this.emissiveIntensity!==1&&(n.emissiveIntensity=this.emissiveIntensity),this.specular&&this.specular.isColor&&(n.specular=this.specular.getHex()),this.specularIntensity!==void 0&&(n.specularIntensity=this.specularIntensity),this.specularColor&&this.specularColor.isColor&&(n.specularColor=this.specularColor.getHex()),this.shininess!==void 0&&(n.shininess=this.shininess),this.clearcoat!==void 0&&(n.clearcoat=this.clearcoat),this.clearcoatRoughness!==void 0&&(n.clearcoatRoughness=this.clearcoatRoughness),this.clearcoatMap&&this.clearcoatMap.isTexture&&(n.clearcoatMap=this.clearcoatMap.toJSON(e).uuid),this.clearcoatRoughnessMap&&this.clearcoatRoughnessMap.isTexture&&(n.clearcoatRoughnessMap=this.clearcoatRoughnessMap.toJSON(e).uuid),this.clearcoatNormalMap&&this.clearcoatNormalMap.isTexture&&(n.clearcoatNormalMap=this.clearcoatNormalMap.toJSON(e).uuid,n.clearcoatNormalScale=this.clearcoatNormalScale.toArray()),this.sheenColorMap&&this.sheenColorMap.isTexture&&(n.sheenColorMap=this.sheenColorMap.toJSON(e).uuid),this.sheenRoughnessMap&&this.sheenRoughnessMap.isTexture&&(n.sheenRoughnessMap=this.sheenRoughnessMap.toJSON(e).uuid),this.dispersion!==void 0&&(n.dispersion=this.dispersion),this.iridescence!==void 0&&(n.iridescence=this.iridescence),this.iridescenceIOR!==void 0&&(n.iridescenceIOR=this.iridescenceIOR),this.iridescenceThicknessRange!==void 0&&(n.iridescenceThicknessRange=this.iridescenceThicknessRange),this.iridescenceMap&&this.iridescenceMap.isTexture&&(n.iridescenceMap=this.iridescenceMap.toJSON(e).uuid),this.iridescenceThicknessMap&&this.iridescenceThicknessMap.isTexture&&(n.iridescenceThicknessMap=this.iridescenceThicknessMap.toJSON(e).uuid),this.anisotropy!==void 0&&(n.anisotropy=this.anisotropy),this.anisotropyRotation!==void 0&&(n.anisotropyRotation=this.anisotropyRotation),this.anisotropyMap&&this.anisotropyMap.isTexture&&(n.anisotropyMap=this.anisotropyMap.toJSON(e).uuid),this.map&&this.map.isTexture&&(n.map=this.map.toJSON(e).uuid),this.matcap&&this.matcap.isTexture&&(n.matcap=this.matcap.toJSON(e).uuid),this.alphaMap&&this.alphaMap.isTexture&&(n.alphaMap=this.alphaMap.toJSON(e).uuid),this.lightMap&&this.lightMap.isTexture&&(n.lightMap=this.lightMap.toJSON(e).uuid,n.lightMapIntensity=this.lightMapIntensity),this.aoMap&&this.aoMap.isTexture&&(n.aoMap=this.aoMap.toJSON(e).uuid,n.aoMapIntensity=this.aoMapIntensity),this.bumpMap&&this.bumpMap.isTexture&&(n.bumpMap=this.bumpMap.toJSON(e).uuid,n.bumpScale=this.bumpScale),this.normalMap&&this.normalMap.isTexture&&(n.normalMap=this.normalMap.toJSON(e).uuid,n.normalMapType=this.normalMapType,n.normalScale=this.normalScale.toArray()),this.displacementMap&&this.displacementMap.isTexture&&(n.displacementMap=this.displacementMap.toJSON(e).uuid,n.displacementScale=this.displacementScale,n.displacementBias=this.displacementBias),this.roughnessMap&&this.roughnessMap.isTexture&&(n.roughnessMap=this.roughnessMap.toJSON(e).uuid),this.metalnessMap&&this.metalnessMap.isTexture&&(n.metalnessMap=this.metalnessMap.toJSON(e).uuid),this.emissiveMap&&this.emissiveMap.isTexture&&(n.emissiveMap=this.emissiveMap.toJSON(e).uuid),this.specularMap&&this.specularMap.isTexture&&(n.specularMap=this.specularMap.toJSON(e).uuid),this.specularIntensityMap&&this.specularIntensityMap.isTexture&&(n.specularIntensityMap=this.specularIntensityMap.toJSON(e).uuid),this.specularColorMap&&this.specularColorMap.isTexture&&(n.specularColorMap=this.specularColorMap.toJSON(e).uuid),this.envMap&&this.envMap.isTexture&&(n.envMap=this.envMap.toJSON(e).uuid,this.combine!==void 0&&(n.combine=this.combine)),this.envMapRotation!==void 0&&(n.envMapRotation=this.envMapRotation.toArray()),this.envMapIntensity!==void 0&&(n.envMapIntensity=this.envMapIntensity),this.reflectivity!==void 0&&(n.reflectivity=this.reflectivity),this.refractionRatio!==void 0&&(n.refractionRatio=this.refractionRatio),this.gradientMap&&this.gradientMap.isTexture&&(n.gradientMap=this.gradientMap.toJSON(e).uuid),this.transmission!==void 0&&(n.transmission=this.transmission),this.transmissionMap&&this.transmissionMap.isTexture&&(n.transmissionMap=this.transmissionMap.toJSON(e).uuid),this.thickness!==void 0&&(n.thickness=this.thickness),this.thicknessMap&&this.thicknessMap.isTexture&&(n.thicknessMap=this.thicknessMap.toJSON(e).uuid),this.attenuationDistance!==void 0&&this.attenuationDistance!==1/0&&(n.attenuationDistance=this.attenuationDistance),this.attenuationColor!==void 0&&(n.attenuationColor=this.attenuationColor.getHex()),this.size!==void 0&&(n.size=this.size),this.shadowSide!==null&&(n.shadowSide=this.shadowSide),this.sizeAttenuation!==void 0&&(n.sizeAttenuation=this.sizeAttenuation),this.blending!==ur&&(n.blending=this.blending),this.side!==yi&&(n.side=this.side),this.vertexColors===!0&&(n.vertexColors=!0),this.opacity<1&&(n.opacity=this.opacity),this.transparent===!0&&(n.transparent=!0),this.blendSrc!==wa&&(n.blendSrc=this.blendSrc),this.blendDst!==Ur&&(n.blendDst=this.blendDst),this.blendEquation!==vi&&(n.blendEquation=this.blendEquation),this.blendSrcAlpha!==null&&(n.blendSrcAlpha=this.blendSrcAlpha),this.blendDstAlpha!==null&&(n.blendDstAlpha=this.blendDstAlpha),this.blendEquationAlpha!==null&&(n.blendEquationAlpha=this.blendEquationAlpha),this.blendColor&&this.blendColor.isColor&&(n.blendColor=this.blendColor.getHex()),this.blendAlpha!==0&&(n.blendAlpha=this.blendAlpha),this.depthFunc!==pr&&(n.depthFunc=this.depthFunc),this.depthTest===!1&&(n.depthTest=this.depthTest),this.depthWrite===!1&&(n.depthWrite=this.depthWrite),this.colorWrite===!1&&(n.colorWrite=this.colorWrite),this.stencilWriteMask!==255&&(n.stencilWriteMask=this.stencilWriteMask),this.stencilFunc!==Yo&&(n.stencilFunc=this.stencilFunc),this.stencilRef!==0&&(n.stencilRef=this.stencilRef),this.stencilFuncMask!==255&&(n.stencilFuncMask=this.stencilFuncMask),this.stencilFail!==$i&&(n.stencilFail=this.stencilFail),this.stencilZFail!==$i&&(n.stencilZFail=this.stencilZFail),this.stencilZPass!==$i&&(n.stencilZPass=this.stencilZPass),this.stencilWrite===!0&&(n.stencilWrite=this.stencilWrite),this.rotation!==void 0&&this.rotation!==0&&(n.rotation=this.rotation),this.polygonOffset===!0&&(n.polygonOffset=!0),this.polygonOffsetFactor!==0&&(n.polygonOffsetFactor=this.polygonOffsetFactor),this.polygonOffsetUnits!==0&&(n.polygonOffsetUnits=this.polygonOffsetUnits),this.linewidth!==void 0&&this.linewidth!==1&&(n.linewidth=this.linewidth),this.dashSize!==void 0&&(n.dashSize=this.dashSize),this.gapSize!==void 0&&(n.gapSize=this.gapSize),this.scale!==void 0&&(n.scale=this.scale),this.dithering===!0&&(n.dithering=!0),this.alphaTest>0&&(n.alphaTest=this.alphaTest),this.alphaHash===!0&&(n.alphaHash=!0),this.alphaToCoverage===!0&&(n.alphaToCoverage=!0),this.premultipliedAlpha===!0&&(n.premultipliedAlpha=!0),this.forceSinglePass===!0&&(n.forceSinglePass=!0),this.allowOverride===!1&&(n.allowOverride=!1),this.wireframe===!0&&(n.wireframe=!0),this.wireframeLinewidth>1&&(n.wireframeLinewidth=this.wireframeLinewidth),this.wireframeLinecap!=="round"&&(n.wireframeLinecap=this.wireframeLinecap),this.wireframeLinejoin!=="round"&&(n.wireframeLinejoin=this.wireframeLinejoin),this.flatShading===!0&&(n.flatShading=!0),this.visible===!1&&(n.visible=!1),this.toneMapped===!1&&(n.toneMapped=!1),this.fog===!1&&(n.fog=!1),Object.keys(this.userData).length>0&&(n.userData=this.userData);function r(s){const a=[];for(const o in s){const c=s[o];delete c.metadata,a.push(c)}return a}if(t){const s=r(e.textures),a=r(e.images);s.length>0&&(n.textures=s),a.length>0&&(n.images=a)}return n}fromJSON(e,t){if(e.uuid!==void 0&&(this.uuid=e.uuid),e.name!==void 0&&(this.name=e.name),e.color!==void 0&&this.color!==void 0&&this.color.setHex(e.color),e.roughness!==void 0&&(this.roughness=e.roughness),e.metalness!==void 0&&(this.metalness=e.metalness),e.sheen!==void 0&&(this.sheen=e.sheen),e.sheenColor!==void 0&&(this.sheenColor=new Tt().setHex(e.sheenColor)),e.sheenRoughness!==void 0&&(this.sheenRoughness=e.sheenRoughness),e.emissive!==void 0&&this.emissive!==void 0&&this.emissive.setHex(e.emissive),e.specular!==void 0&&this.specular!==void 0&&this.specular.setHex(e.specular),e.specularIntensity!==void 0&&(this.specularIntensity=e.specularIntensity),e.specularColor!==void 0&&this.specularColor!==void 0&&this.specularColor.setHex(e.specularColor),e.shininess!==void 0&&(this.shininess=e.shininess),e.clearcoat!==void 0&&(this.clearcoat=e.clearcoat),e.clearcoatRoughness!==void 0&&(this.clearcoatRoughness=e.clearcoatRoughness),e.dispersion!==void 0&&(this.dispersion=e.dispersion),e.iridescence!==void 0&&(this.iridescence=e.iridescence),e.iridescenceIOR!==void 0&&(this.iridescenceIOR=e.iridescenceIOR),e.iridescenceThicknessRange!==void 0&&(this.iridescenceThicknessRange=e.iridescenceThicknessRange),e.transmission!==void 0&&(this.transmission=e.transmission),e.thickness!==void 0&&(this.thickness=e.thickness),e.attenuationDistance!==void 0&&(this.attenuationDistance=e.attenuationDistance),e.attenuationColor!==void 0&&this.attenuationColor!==void 0&&this.attenuationColor.setHex(e.attenuationColor),e.anisotropy!==void 0&&(this.anisotropy=e.anisotropy),e.anisotropyRotation!==void 0&&(this.anisotropyRotation=e.anisotropyRotation),e.fog!==void 0&&(this.fog=e.fog),e.flatShading!==void 0&&(this.flatShading=e.flatShading),e.blending!==void 0&&(this.blending=e.blending),e.combine!==void 0&&(this.combine=e.combine),e.side!==void 0&&(this.side=e.side),e.shadowSide!==void 0&&(this.shadowSide=e.shadowSide),e.opacity!==void 0&&(this.opacity=e.opacity),e.transparent!==void 0&&(this.transparent=e.transparent),e.alphaTest!==void 0&&(this.alphaTest=e.alphaTest),e.alphaHash!==void 0&&(this.alphaHash=e.alphaHash),e.depthFunc!==void 0&&(this.depthFunc=e.depthFunc),e.depthTest!==void 0&&(this.depthTest=e.depthTest),e.depthWrite!==void 0&&(this.depthWrite=e.depthWrite),e.colorWrite!==void 0&&(this.colorWrite=e.colorWrite),e.blendSrc!==void 0&&(this.blendSrc=e.blendSrc),e.blendDst!==void 0&&(this.blendDst=e.blendDst),e.blendEquation!==void 0&&(this.blendEquation=e.blendEquation),e.blendSrcAlpha!==void 0&&(this.blendSrcAlpha=e.blendSrcAlpha),e.blendDstAlpha!==void 0&&(this.blendDstAlpha=e.blendDstAlpha),e.blendEquationAlpha!==void 0&&(this.blendEquationAlpha=e.blendEquationAlpha),e.blendColor!==void 0&&this.blendColor!==void 0&&this.blendColor.setHex(e.blendColor),e.blendAlpha!==void 0&&(this.blendAlpha=e.blendAlpha),e.stencilWriteMask!==void 0&&(this.stencilWriteMask=e.stencilWriteMask),e.stencilFunc!==void 0&&(this.stencilFunc=e.stencilFunc),e.stencilRef!==void 0&&(this.stencilRef=e.stencilRef),e.stencilFuncMask!==void 0&&(this.stencilFuncMask=e.stencilFuncMask),e.stencilFail!==void 0&&(this.stencilFail=e.stencilFail),e.stencilZFail!==void 0&&(this.stencilZFail=e.stencilZFail),e.stencilZPass!==void 0&&(this.stencilZPass=e.stencilZPass),e.stencilWrite!==void 0&&(this.stencilWrite=e.stencilWrite),e.wireframe!==void 0&&(this.wireframe=e.wireframe),e.wireframeLinewidth!==void 0&&(this.wireframeLinewidth=e.wireframeLinewidth),e.wireframeLinecap!==void 0&&(this.wireframeLinecap=e.wireframeLinecap),e.wireframeLinejoin!==void 0&&(this.wireframeLinejoin=e.wireframeLinejoin),e.rotation!==void 0&&(this.rotation=e.rotation),e.linewidth!==void 0&&(this.linewidth=e.linewidth),e.dashSize!==void 0&&(this.dashSize=e.dashSize),e.gapSize!==void 0&&(this.gapSize=e.gapSize),e.scale!==void 0&&(this.scale=e.scale),e.polygonOffset!==void 0&&(this.polygonOffset=e.polygonOffset),e.polygonOffsetFactor!==void 0&&(this.polygonOffsetFactor=e.polygonOffsetFactor),e.polygonOffsetUnits!==void 0&&(this.polygonOffsetUnits=e.polygonOffsetUnits),e.dithering!==void 0&&(this.dithering=e.dithering),e.alphaToCoverage!==void 0&&(this.alphaToCoverage=e.alphaToCoverage),e.premultipliedAlpha!==void 0&&(this.premultipliedAlpha=e.premultipliedAlpha),e.forceSinglePass!==void 0&&(this.forceSinglePass=e.forceSinglePass),e.allowOverride!==void 0&&(this.allowOverride=e.allowOverride),e.visible!==void 0&&(this.visible=e.visible),e.toneMapped!==void 0&&(this.toneMapped=e.toneMapped),e.userData!==void 0&&(this.userData=e.userData),e.vertexColors!==void 0&&(typeof e.vertexColors=="number"?this.vertexColors=e.vertexColors>0:this.vertexColors=e.vertexColors),e.size!==void 0&&(this.size=e.size),e.sizeAttenuation!==void 0&&(this.sizeAttenuation=e.sizeAttenuation),e.map!==void 0&&(this.map=t[e.map]||null),e.matcap!==void 0&&(this.matcap=t[e.matcap]||null),e.alphaMap!==void 0&&(this.alphaMap=t[e.alphaMap]||null),e.bumpMap!==void 0&&(this.bumpMap=t[e.bumpMap]||null),e.bumpScale!==void 0&&(this.bumpScale=e.bumpScale),e.normalMap!==void 0&&(this.normalMap=t[e.normalMap]||null),e.normalMapType!==void 0&&(this.normalMapType=e.normalMapType),e.normalScale!==void 0){let n=e.normalScale;Array.isArray(n)===!1&&(n=[n,n]),this.normalScale=new Mt().fromArray(n)}return e.displacementMap!==void 0&&(this.displacementMap=t[e.displacementMap]||null),e.displacementScale!==void 0&&(this.displacementScale=e.displacementScale),e.displacementBias!==void 0&&(this.displacementBias=e.displacementBias),e.roughnessMap!==void 0&&(this.roughnessMap=t[e.roughnessMap]||null),e.metalnessMap!==void 0&&(this.metalnessMap=t[e.metalnessMap]||null),e.emissiveMap!==void 0&&(this.emissiveMap=t[e.emissiveMap]||null),e.emissiveIntensity!==void 0&&(this.emissiveIntensity=e.emissiveIntensity),e.specularMap!==void 0&&(this.specularMap=t[e.specularMap]||null),e.specularIntensityMap!==void 0&&(this.specularIntensityMap=t[e.specularIntensityMap]||null),e.specularColorMap!==void 0&&(this.specularColorMap=t[e.specularColorMap]||null),e.envMap!==void 0&&(this.envMap=t[e.envMap]||null),e.envMapRotation!==void 0&&this.envMapRotation.fromArray(e.envMapRotation),e.envMapIntensity!==void 0&&(this.envMapIntensity=e.envMapIntensity),e.reflectivity!==void 0&&(this.reflectivity=e.reflectivity),e.refractionRatio!==void 0&&(this.refractionRatio=e.refractionRatio),e.lightMap!==void 0&&(this.lightMap=t[e.lightMap]||null),e.lightMapIntensity!==void 0&&(this.lightMapIntensity=e.lightMapIntensity),e.aoMap!==void 0&&(this.aoMap=t[e.aoMap]||null),e.aoMapIntensity!==void 0&&(this.aoMapIntensity=e.aoMapIntensity),e.gradientMap!==void 0&&(this.gradientMap=t[e.gradientMap]||null),e.clearcoatMap!==void 0&&(this.clearcoatMap=t[e.clearcoatMap]||null),e.clearcoatRoughnessMap!==void 0&&(this.clearcoatRoughnessMap=t[e.clearcoatRoughnessMap]||null),e.clearcoatNormalMap!==void 0&&(this.clearcoatNormalMap=t[e.clearcoatNormalMap]||null),e.clearcoatNormalScale!==void 0&&(this.clearcoatNormalScale=new Mt().fromArray(e.clearcoatNormalScale)),e.iridescenceMap!==void 0&&(this.iridescenceMap=t[e.iridescenceMap]||null),e.iridescenceThicknessMap!==void 0&&(this.iridescenceThicknessMap=t[e.iridescenceThicknessMap]||null),e.transmissionMap!==void 0&&(this.transmissionMap=t[e.transmissionMap]||null),e.thicknessMap!==void 0&&(this.thicknessMap=t[e.thicknessMap]||null),e.anisotropyMap!==void 0&&(this.anisotropyMap=t[e.anisotropyMap]||null),e.sheenColorMap!==void 0&&(this.sheenColorMap=t[e.sheenColorMap]||null),e.sheenRoughnessMap!==void 0&&(this.sheenRoughnessMap=t[e.sheenRoughnessMap]||null),this}clone(){return new this.constructor().copy(this)}copy(e){this.name=e.name,this.blending=e.blending,this.side=e.side,this.vertexColors=e.vertexColors,this.opacity=e.opacity,this.transparent=e.transparent,this.blendSrc=e.blendSrc,this.blendDst=e.blendDst,this.blendEquation=e.blendEquation,this.blendSrcAlpha=e.blendSrcAlpha,this.blendDstAlpha=e.blendDstAlpha,this.blendEquationAlpha=e.blendEquationAlpha,this.blendColor.copy(e.blendColor),this.blendAlpha=e.blendAlpha,this.depthFunc=e.depthFunc,this.depthTest=e.depthTest,this.depthWrite=e.depthWrite,this.stencilWriteMask=e.stencilWriteMask,this.stencilFunc=e.stencilFunc,this.stencilRef=e.stencilRef,this.stencilFuncMask=e.stencilFuncMask,this.stencilFail=e.stencilFail,this.stencilZFail=e.stencilZFail,this.stencilZPass=e.stencilZPass,this.stencilWrite=e.stencilWrite;const t=e.clippingPlanes;let n=null;if(t!==null){const r=t.length;n=new Array(r);for(let s=0;s!==r;++s)n[s]=t[s].clone()}return this.clippingPlanes=n,this.clipIntersection=e.clipIntersection,this.clipShadows=e.clipShadows,this.shadowSide=e.shadowSide,this.colorWrite=e.colorWrite,this.precision=e.precision,this.polygonOffset=e.polygonOffset,this.polygonOffsetFactor=e.polygonOffsetFactor,this.polygonOffsetUnits=e.polygonOffsetUnits,this.dithering=e.dithering,this.alphaTest=e.alphaTest,this.alphaHash=e.alphaHash,this.alphaToCoverage=e.alphaToCoverage,this.premultipliedAlpha=e.premultipliedAlpha,this.forceSinglePass=e.forceSinglePass,this.allowOverride=e.allowOverride,this.visible=e.visible,this.toneMapped=e.toneMapped,this.userData=JSON.parse(JSON.stringify(e.userData)),this}dispose(){this.dispatchEvent({type:"dispose"})}set needsUpdate(e){e===!0&&this.version++}}const ei=new K,ha=new K,es=new K,_i=new K,fa=new K,ts=new K,pa=new K;class fu{constructor(e=new K,t=new K(0,0,-1)){this.origin=e,this.direction=t}set(e,t){return this.origin.copy(e),this.direction.copy(t),this}copy(e){return this.origin.copy(e.origin),this.direction.copy(e.direction),this}at(e,t){return t.copy(this.origin).addScaledVector(this.direction,e)}lookAt(e){return this.direction.copy(e).sub(this.origin).normalize(),this}recast(e){return this.origin.copy(this.at(e,ei)),this}closestPointToPoint(e,t){t.subVectors(e,this.origin);const n=t.dot(this.direction);return n<0?t.copy(this.origin):t.copy(this.origin).addScaledVector(this.direction,n)}distanceToPoint(e){return Math.sqrt(this.distanceSqToPoint(e))}distanceSqToPoint(e){const t=ei.subVectors(e,this.origin).dot(this.direction);return t<0?this.origin.distanceToSquared(e):(ei.copy(this.origin).addScaledVector(this.direction,t),ei.distanceToSquared(e))}distanceSqToSegment(e,t,n,r){ha.copy(e).add(t).multiplyScalar(.5),es.copy(t).sub(e).normalize(),_i.copy(this.origin).sub(ha);const s=e.distanceTo(t)*.5,a=-this.direction.dot(es),o=_i.dot(this.direction),c=-_i.dot(es),l=_i.lengthSq(),u=Math.abs(1-a*a);let f,d,m,x;if(u>0)if(f=a*c-o,d=a*o-c,x=s*u,f>=0)if(d>=-x)if(d<=x){const S=1/u;f*=S,d*=S,m=f*(f+a*d+2*o)+d*(a*f+d+2*c)+l}else d=s,f=Math.max(0,-(a*d+o)),m=-f*f+d*(d+2*c)+l;else d=-s,f=Math.max(0,-(a*d+o)),m=-f*f+d*(d+2*c)+l;else d<=-x?(f=Math.max(0,-(-a*s+o)),d=f>0?-s:Math.min(Math.max(-s,-c),s),m=-f*f+d*(d+2*c)+l):d<=x?(f=0,d=Math.min(Math.max(-s,-c),s),m=d*(d+2*c)+l):(f=Math.max(0,-(a*s+o)),d=f>0?s:Math.min(Math.max(-s,-c),s),m=-f*f+d*(d+2*c)+l);else d=a>0?-s:s,f=Math.max(0,-(a*d+o)),m=-f*f+d*(d+2*c)+l;return n&&n.copy(this.origin).addScaledVector(this.direction,f),r&&r.copy(ha).addScaledVector(es,d),m}intersectSphere(e,t){ei.subVectors(e.center,this.origin);const n=ei.dot(this.direction),r=ei.dot(ei)-n*n,s=e.radius*e.radius;if(r>s)return null;const a=Math.sqrt(s-r),o=n-a,c=n+a;return c<0?null:o<0?this.at(c,t):this.at(o,t)}intersectsSphere(e){return e.radius<0?!1:this.distanceSqToPoint(e.center)<=e.radius*e.radius}distanceToPlane(e){const t=e.normal.dot(this.direction);if(t===0)return e.distanceToPoint(this.origin)===0?0:null;const n=-(this.origin.dot(e.normal)+e.constant)/t;return n>=0?n:null}intersectPlane(e,t){const n=this.distanceToPlane(e);return n===null?null:this.at(n,t)}intersectsPlane(e){const t=e.distanceToPoint(this.origin);return t===0||e.normal.dot(this.direction)*t<0}intersectBox(e,t){let n,r,s,a,o,c;const l=1/this.direction.x,u=1/this.direction.y,f=1/this.direction.z,d=this.origin;return l>=0?(n=(e.min.x-d.x)*l,r=(e.max.x-d.x)*l):(n=(e.max.x-d.x)*l,r=(e.min.x-d.x)*l),u>=0?(s=(e.min.y-d.y)*u,a=(e.max.y-d.y)*u):(s=(e.max.y-d.y)*u,a=(e.min.y-d.y)*u),n>a||s>r||((s>n||isNaN(n))&&(n=s),(a<r||isNaN(r))&&(r=a),f>=0?(o=(e.min.z-d.z)*f,c=(e.max.z-d.z)*f):(o=(e.max.z-d.z)*f,c=(e.min.z-d.z)*f),n>c||o>r)||((o>n||n!==n)&&(n=o),(c<r||r!==r)&&(r=c),r<0)?null:this.at(n>=0?n:r,t)}intersectsBox(e){return this.intersectBox(e,ei)!==null}intersectTriangle(e,t,n,r,s){fa.subVectors(t,e),ts.subVectors(n,e),pa.crossVectors(fa,ts);let a=this.direction.dot(pa),o;if(a>0){if(r)return null;o=1}else if(a<0)o=-1,a=-a;else return null;_i.subVectors(this.origin,e);const c=o*this.direction.dot(ts.crossVectors(_i,ts));if(c<0)return null;const l=o*this.direction.dot(fa.cross(_i));if(l<0||c+l>a)return null;const u=-o*_i.dot(pa);return u<0?null:this.at(u/a,s)}applyMatrix4(e){return this.origin.applyMatrix4(e),this.direction.transformDirection(e),this}equals(e){return e.origin.equals(this.origin)&&e.direction.equals(this.direction)}clone(){return new this.constructor().copy(this)}}class xc extends Ns{constructor(e){super(),this.isMeshBasicMaterial=!0,this.type="MeshBasicMaterial",this.color=new Tt(16777215),this.map=null,this.lightMap=null,this.lightMapIntensity=1,this.aoMap=null,this.aoMapIntensity=1,this.specularMap=null,this.alphaMap=null,this.envMap=null,this.envMapRotation=new zi,this.combine=$l,this.reflectivity=1,this.refractionRatio=.98,this.wireframe=!1,this.wireframeLinewidth=1,this.wireframeLinecap="round",this.wireframeLinejoin="round",this.fog=!0,this.setValues(e)}copy(e){return super.copy(e),this.color.copy(e.color),this.map=e.map,this.lightMap=e.lightMap,this.lightMapIntensity=e.lightMapIntensity,this.aoMap=e.aoMap,this.aoMapIntensity=e.aoMapIntensity,this.specularMap=e.specularMap,this.alphaMap=e.alphaMap,this.envMap=e.envMap,this.envMapRotation.copy(e.envMapRotation),this.combine=e.combine,this.reflectivity=e.reflectivity,this.refractionRatio=e.refractionRatio,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.wireframeLinecap=e.wireframeLinecap,this.wireframeLinejoin=e.wireframeLinejoin,this.fog=e.fog,this}}const al=new Vt,Li=new fu,ns=new Lo,ol=new K,is=new K,rs=new K,ss=new K,ma=new K,as=new K,ll=new K,os=new K;class qn extends mn{constructor(e=new Kn,t=new xc){super(),this.isMesh=!0,this.type="Mesh",this.geometry=e,this.material=t,this.morphTargetDictionary=void 0,this.morphTargetInfluences=void 0,this.count=1,this.updateMorphTargets()}copy(e,t){return super.copy(e,t),e.morphTargetInfluences!==void 0&&(this.morphTargetInfluences=e.morphTargetInfluences.slice()),e.morphTargetDictionary!==void 0&&(this.morphTargetDictionary=Object.assign({},e.morphTargetDictionary)),this.material=Array.isArray(e.material)?e.material.slice():e.material,this.geometry=e.geometry,this}updateMorphTargets(){const t=this.geometry.morphAttributes,n=Object.keys(t);if(n.length>0){const r=t[n[0]];if(r!==void 0){this.morphTargetInfluences=[],this.morphTargetDictionary={};for(let s=0,a=r.length;s<a;s++){const o=r[s].name||String(s);this.morphTargetInfluences.push(0),this.morphTargetDictionary[o]=s}}}}getVertexPosition(e,t){const n=this.geometry,r=n.attributes.position,s=n.morphAttributes.position,a=n.morphTargetsRelative;t.fromBufferAttribute(r,e);const o=this.morphTargetInfluences;if(s&&o){as.set(0,0,0);for(let c=0,l=s.length;c<l;c++){const u=o[c],f=s[c];u!==0&&(ma.fromBufferAttribute(f,e),a?as.addScaledVector(ma,u):as.addScaledVector(ma.sub(t),u))}t.add(as)}return t}raycast(e,t){const n=this.geometry,r=this.material,s=this.matrixWorld;r!==void 0&&(n.boundingSphere===null&&n.computeBoundingSphere(),ns.copy(n.boundingSphere),ns.applyMatrix4(s),Li.copy(e.ray).recast(e.near),!(ns.containsPoint(Li.origin)===!1&&(Li.intersectSphere(ns,ol)===null||Li.origin.distanceToSquared(ol)>(e.far-e.near)**2))&&(al.copy(s).invert(),Li.copy(e.ray).applyMatrix4(al),!(n.boundingBox!==null&&Li.intersectsBox(n.boundingBox)===!1)&&this._computeIntersections(e,t,Li)))}_computeIntersections(e,t,n){let r;const s=this.geometry,a=this.material,o=s.index,c=s.attributes.position,l=s.attributes.uv,u=s.attributes.uv1,f=s.attributes.normal,d=s.groups,m=s.drawRange;if(o!==null)if(Array.isArray(a))for(let x=0,S=d.length;x<S;x++){const g=d[x],h=a[g.materialIndex],R=Math.max(g.start,m.start),C=Math.min(o.count,Math.min(g.start+g.count,m.start+m.count));for(let y=R,w=C;y<w;y+=3){const T=o.getX(y),I=o.getX(y+1),_=o.getX(y+2);r=ls(this,h,e,n,l,u,f,T,I,_),r&&(r.faceIndex=Math.floor(y/3),r.face.materialIndex=g.materialIndex,t.push(r))}}else{const x=Math.max(0,m.start),S=Math.min(o.count,m.start+m.count);for(let g=x,h=S;g<h;g+=3){const R=o.getX(g),C=o.getX(g+1),y=o.getX(g+2);r=ls(this,a,e,n,l,u,f,R,C,y),r&&(r.faceIndex=Math.floor(g/3),t.push(r))}}else if(c!==void 0)if(Array.isArray(a))for(let x=0,S=d.length;x<S;x++){const g=d[x],h=a[g.materialIndex],R=Math.max(g.start,m.start),C=Math.min(c.count,Math.min(g.start+g.count,m.start+m.count));for(let y=R,w=C;y<w;y+=3){const T=y,I=y+1,_=y+2;r=ls(this,h,e,n,l,u,f,T,I,_),r&&(r.faceIndex=Math.floor(y/3),r.face.materialIndex=g.materialIndex,t.push(r))}}else{const x=Math.max(0,m.start),S=Math.min(c.count,m.start+m.count);for(let g=x,h=S;g<h;g+=3){const R=g,C=g+1,y=g+2;r=ls(this,a,e,n,l,u,f,R,C,y),r&&(r.faceIndex=Math.floor(g/3),t.push(r))}}}}function pu(i,e,t,n,r,s,a,o){let c;if(e.side===cn?c=n.intersectTriangle(a,s,r,!0,o):c=n.intersectTriangle(r,s,a,e.side===yi,o),c===null)return null;os.copy(o),os.applyMatrix4(i.matrixWorld);const l=t.ray.origin.distanceTo(os);return l<t.near||l>t.far?null:{distance:l,point:os.clone(),object:i}}function ls(i,e,t,n,r,s,a,o,c,l){i.getVertexPosition(o,is),i.getVertexPosition(c,rs),i.getVertexPosition(l,ss);const u=pu(i,e,t,n,is,rs,ss,ll);if(u){const f=new K;Rn.getBarycoord(ll,is,rs,ss,f),r&&(u.uv=Rn.getInterpolatedAttribute(r,o,c,l,f,new Mt)),s&&(u.uv1=Rn.getInterpolatedAttribute(s,o,c,l,f,new Mt)),a&&(u.normal=Rn.getInterpolatedAttribute(a,o,c,l,f,new K),u.normal.dot(n.direction)>0&&u.normal.multiplyScalar(-1));const d={a:o,b:c,c:l,normal:new K,materialIndex:0};Rn.getNormal(is,rs,ss,d.normal),u.face=d,u.barycoord=f}return u}class mu extends tn{constructor(e=null,t=1,n=1,r,s,a,o,c,l=Jt,u=Jt,f,d){super(null,a,o,c,l,u,r,s,f,d),this.isDataTexture=!0,this.image={data:e,width:t,height:n},this.generateMipmaps=!1,this.flipY=!1,this.unpackAlignment=1}}const ga=new K,gu=new K,_u=new nt;class Ii{constructor(e=new K(1,0,0),t=0){this.isPlane=!0,this.normal=e,this.constant=t}set(e,t){return this.normal.copy(e),this.constant=t,this}setComponents(e,t,n,r){return this.normal.set(e,t,n),this.constant=r,this}setFromNormalAndCoplanarPoint(e,t){return this.normal.copy(e),this.constant=-t.dot(this.normal),this}setFromCoplanarPoints(e,t,n){const r=ga.subVectors(n,t).cross(gu.subVectors(e,t)).normalize();return this.setFromNormalAndCoplanarPoint(r,e),this}copy(e){return this.normal.copy(e.normal),this.constant=e.constant,this}normalize(){const e=1/this.normal.length();return this.normal.multiplyScalar(e),this.constant*=e,this}negate(){return this.constant*=-1,this.normal.negate(),this}distanceToPoint(e){return this.normal.dot(e)+this.constant}distanceToSphere(e){return this.distanceToPoint(e.center)-e.radius}projectPoint(e,t){return t.copy(e).addScaledVector(this.normal,-this.distanceToPoint(e))}intersectLine(e,t,n=!0){const r=e.delta(ga),s=this.normal.dot(r);if(s===0)return this.distanceToPoint(e.start)===0?t.copy(e.start):null;const a=-(e.start.dot(this.normal)+this.constant)/s;return n===!0&&(a<0||a>1)?null:t.copy(e.start).addScaledVector(r,a)}intersectsLine(e){const t=this.distanceToPoint(e.start),n=this.distanceToPoint(e.end);return t<0&&n>0||n<0&&t>0}intersectsBox(e){return e.intersectsPlane(this)}intersectsSphere(e){return e.intersectsPlane(this)}coplanarPoint(e){return e.copy(this.normal).multiplyScalar(-this.constant)}applyMatrix4(e,t){const n=t||_u.getNormalMatrix(e),r=this.coplanarPoint(ga).applyMatrix4(e),s=this.normal.applyMatrix3(n).normalize();return this.constant=-r.dot(s),this}translate(e){return this.constant-=e.dot(this.normal),this}equals(e){return e.normal.equals(this.normal)&&e.constant===this.constant}clone(){return new this.constructor().copy(this)}}const Di=new Lo,xu=new Mt(.5,.5),cs=new K;class vc{constructor(e=new Ii,t=new Ii,n=new Ii,r=new Ii,s=new Ii,a=new Ii){this.planes=[e,t,n,r,s,a]}set(e,t,n,r,s,a){const o=this.planes;return o[0].copy(e),o[1].copy(t),o[2].copy(n),o[3].copy(r),o[4].copy(s),o[5].copy(a),this}copy(e){const t=this.planes;for(let n=0;n<6;n++)t[n].copy(e.planes[n]);return this}setFromProjectionMatrix(e,t=Hn,n=!1){const r=this.planes,s=e.elements,a=s[0],o=s[1],c=s[2],l=s[3],u=s[4],f=s[5],d=s[6],m=s[7],x=s[8],S=s[9],g=s[10],h=s[11],R=s[12],C=s[13],y=s[14],w=s[15];if(r[0].setComponents(l-a,m-u,h-x,w-R).normalize(),r[1].setComponents(l+a,m+u,h+x,w+R).normalize(),r[2].setComponents(l+o,m+f,h+S,w+C).normalize(),r[3].setComponents(l-o,m-f,h-S,w-C).normalize(),n)r[4].setComponents(c,d,g,y).normalize(),r[5].setComponents(l-c,m-d,h-g,w-y).normalize();else if(r[4].setComponents(l-c,m-d,h-g,w-y).normalize(),t===Hn)r[5].setComponents(l+c,m+d,h+g,w+y).normalize();else if(t===Rs)r[5].setComponents(c,d,g,y).normalize();else throw new Error("THREE.Frustum.setFromProjectionMatrix(): Invalid coordinate system: "+t);return this}intersectsObject(e){if(e.boundingSphere!==void 0)e.boundingSphere===null&&e.computeBoundingSphere(),Di.copy(e.boundingSphere).applyMatrix4(e.matrixWorld);else{const t=e.geometry;t.boundingSphere===null&&t.computeBoundingSphere(),Di.copy(t.boundingSphere).applyMatrix4(e.matrixWorld)}return this.intersectsSphere(Di)}intersectsSprite(e){Di.center.set(0,0,0);const t=xu.distanceTo(e.center);return Di.radius=.7071067811865476+t,Di.applyMatrix4(e.matrixWorld),this.intersectsSphere(Di)}intersectsSphere(e){const t=this.planes,n=e.center,r=-e.radius;for(let s=0;s<6;s++)if(t[s].distanceToPoint(n)<r)return!1;return!0}intersectsBox(e){const t=this.planes;for(let n=0;n<6;n++){const r=t[n];if(cs.x=r.normal.x>0?e.max.x:e.min.x,cs.y=r.normal.y>0?e.max.y:e.min.y,cs.z=r.normal.z>0?e.max.z:e.min.z,r.distanceToPoint(cs)<0)return!1}return!0}containsPoint(e){const t=this.planes;for(let n=0;n<6;n++)if(t[n].distanceToPoint(e)<0)return!1;return!0}clone(){return new this.constructor().copy(this)}}class Mc extends tn{constructor(e=[],t=Oi,n,r,s,a,o,c,l,u){super(e,t,n,r,s,a,o,c,l,u),this.isCubeTexture=!0,this.flipY=!1}get images(){return this.image}set images(e){this.image=e}}class vu extends tn{constructor(e,t,n,r,s,a,o,c,l){super(e,t,n,r,s,a,o,c,l),this.isCanvasTexture=!0,this.needsUpdate=!0}}class gr extends tn{constructor(e,t,n=Xn,r,s,a,o=Jt,c=Jt,l,u=ci,f=1){if(u!==ci&&u!==Fi)throw new Error("THREE.DepthTexture: format must be either THREE.DepthFormat or THREE.DepthStencilFormat");const d={width:e,height:t,depth:f};super(d,r,s,a,o,c,u,n,l),this.isDepthTexture=!0,this.flipY=!1,this.generateMipmaps=!1,this.compareFunction=null}copy(e){return super.copy(e),this.source=new Po(Object.assign({},e.image)),this.compareFunction=e.compareFunction,this}toJSON(e){const t=super.toJSON(e);return this.compareFunction!==null&&(t.compareFunction=this.compareFunction),t}}class Mu extends gr{constructor(e,t=Xn,n=Oi,r,s,a=Jt,o=Jt,c,l=ci){const u={width:e,height:e,depth:1},f=[u,u,u,u,u,u];super(e,e,t,n,r,s,a,o,c,l),this.image=f,this.isCubeDepthTexture=!0,this.isCubeTexture=!0}get images(){return this.image}set images(e){this.image=e}}class Sc extends tn{constructor(e=null){super(),this.sourceTexture=e,this.isExternalTexture=!0}copy(e){return super.copy(e),this.sourceTexture=e.sourceTexture,this}}class zr extends Kn{constructor(e=1,t=1,n=1,r=1,s=1,a=1){super(),this.type="BoxGeometry",this.parameters={width:e,height:t,depth:n,widthSegments:r,heightSegments:s,depthSegments:a};const o=this;r=Math.floor(r),s=Math.floor(s),a=Math.floor(a);const c=[],l=[],u=[],f=[];let d=0,m=0;x("z","y","x",-1,-1,n,t,e,a,s,0),x("z","y","x",1,-1,n,t,-e,a,s,1),x("x","z","y",1,1,e,n,t,r,a,2),x("x","z","y",1,-1,e,n,-t,r,a,3),x("x","y","z",1,-1,e,t,n,r,s,4),x("x","y","z",-1,-1,e,t,-n,r,s,5),this.setIndex(c),this.setAttribute("position",new oi(l,3)),this.setAttribute("normal",new oi(u,3)),this.setAttribute("uv",new oi(f,2));function x(S,g,h,R,C,y,w,T,I,_,A){const k=y/I,F=w/_,b=y/2,P=w/2,W=T/2,N=I+1,D=_+1;let G=0,H=0;const ne=new K;for(let ce=0;ce<D;ce++){const oe=ce*F-P;for(let _e=0;_e<N;_e++){const Qe=_e*k-b;ne[S]=Qe*R,ne[g]=oe*C,ne[h]=W,l.push(ne.x,ne.y,ne.z),ne[S]=0,ne[g]=0,ne[h]=T>0?1:-1,u.push(ne.x,ne.y,ne.z),f.push(_e/I),f.push(1-ce/_),G+=1}}for(let ce=0;ce<_;ce++)for(let oe=0;oe<I;oe++){const _e=d+oe+N*ce,Qe=d+oe+N*(ce+1),Ze=d+(oe+1)+N*(ce+1),He=d+(oe+1)+N*ce;c.push(_e,Qe,He),c.push(Qe,Ze,He),H+=6}o.addGroup(m,H,A),m+=H,d+=G}}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new zr(e.width,e.height,e.depth,e.widthSegments,e.heightSegments,e.depthSegments)}}class Fs extends Kn{constructor(e=1,t=1,n=1,r=1){super(),this.type="PlaneGeometry",this.parameters={width:e,height:t,widthSegments:n,heightSegments:r};const s=e/2,a=t/2,o=Math.floor(n),c=Math.floor(r),l=o+1,u=c+1,f=e/o,d=t/c,m=[],x=[],S=[],g=[];for(let h=0;h<u;h++){const R=h*d-a;for(let C=0;C<l;C++){const y=C*f-s;x.push(y,-R,0),S.push(0,0,1),g.push(C/o),g.push(1-h/c)}}for(let h=0;h<c;h++)for(let R=0;R<o;R++){const C=R+l*h,y=R+l*(h+1),w=R+1+l*(h+1),T=R+1+l*h;m.push(C,y,T),m.push(y,w,T)}this.setIndex(m),this.setAttribute("position",new oi(x,3)),this.setAttribute("normal",new oi(S,3)),this.setAttribute("uv",new oi(g,2))}copy(e){return super.copy(e),this.parameters=Object.assign({},e.parameters),this}static fromJSON(e){return new Fs(e.width,e.height,e.widthSegments,e.heightSegments)}}function _r(i){const e={};for(const t in i){e[t]={};for(const n in i[t]){const r=i[t][n];if(cl(r))r.isRenderTargetTexture?(je("UniformsUtils: Textures of render targets cannot be cloned via cloneUniforms() or mergeUniforms()."),e[t][n]=null):e[t][n]=r.clone();else if(Array.isArray(r))if(cl(r[0])){const s=[];for(let a=0,o=r.length;a<o;a++)s[a]=r[a].clone();e[t][n]=s}else e[t][n]=r.slice();else e[t][n]=r}}return e}function sn(i){const e={};for(let t=0;t<i.length;t++){const n=_r(i[t]);for(const r in n)e[r]=n[r]}return e}function cl(i){return i&&(i.isColor||i.isMatrix3||i.isMatrix4||i.isVector2||i.isVector3||i.isVector4||i.isTexture||i.isQuaternion)}function Su(i){const e=[];for(let t=0;t<i.length;t++)e.push(i[t].clone());return e}function yc(i){const e=i.getRenderTarget();return e===null?i.outputColorSpace:e.isXRRenderTarget===!0?e.texture.colorSpace:mt.workingColorSpace}const yu={clone:_r,merge:sn};var bu=`void main() {
	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}`,Eu=`void main() {
	gl_FragColor = vec4( 1.0, 0.0, 0.0, 1.0 );
}`;class Yn extends Ns{constructor(e){super(),this.isShaderMaterial=!0,this.type="ShaderMaterial",this.defines={},this.uniforms={},this.uniformsGroups=[],this.vertexShader=bu,this.fragmentShader=Eu,this.linewidth=1,this.wireframe=!1,this.wireframeLinewidth=1,this.fog=!1,this.lights=!1,this.clipping=!1,this.forceSinglePass=!0,this.extensions={clipCullDistance:!1,multiDraw:!1},this.defaultAttributeValues={color:[1,1,1],uv:[0,0],uv1:[0,0]},this.index0AttributeName=void 0,this.uniformsNeedUpdate=!1,this.glslVersion=null,e!==void 0&&this.setValues(e)}copy(e){return super.copy(e),this.fragmentShader=e.fragmentShader,this.vertexShader=e.vertexShader,this.uniforms=_r(e.uniforms),this.uniformsGroups=Su(e.uniformsGroups),this.defines=Object.assign({},e.defines),this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this.fog=e.fog,this.lights=e.lights,this.clipping=e.clipping,this.extensions=Object.assign({},e.extensions),this.glslVersion=e.glslVersion,this.defaultAttributeValues=Object.assign({},e.defaultAttributeValues),this.index0AttributeName=e.index0AttributeName,this.uniformsNeedUpdate=e.uniformsNeedUpdate,this}toJSON(e){const t=super.toJSON(e);t.glslVersion=this.glslVersion,t.uniforms={};for(const r in this.uniforms){const a=this.uniforms[r].value;a&&a.isTexture?t.uniforms[r]={type:"t",value:a.toJSON(e).uuid}:a&&a.isColor?t.uniforms[r]={type:"c",value:a.getHex()}:a&&a.isVector2?t.uniforms[r]={type:"v2",value:a.toArray()}:a&&a.isVector3?t.uniforms[r]={type:"v3",value:a.toArray()}:a&&a.isVector4?t.uniforms[r]={type:"v4",value:a.toArray()}:a&&a.isMatrix3?t.uniforms[r]={type:"m3",value:a.toArray()}:a&&a.isMatrix4?t.uniforms[r]={type:"m4",value:a.toArray()}:t.uniforms[r]={value:a}}Object.keys(this.defines).length>0&&(t.defines=this.defines),t.vertexShader=this.vertexShader,t.fragmentShader=this.fragmentShader,t.lights=this.lights,t.clipping=this.clipping;const n={};for(const r in this.extensions)this.extensions[r]===!0&&(n[r]=!0);return Object.keys(n).length>0&&(t.extensions=n),t}fromJSON(e,t){if(super.fromJSON(e,t),e.uniforms!==void 0)for(const n in e.uniforms){const r=e.uniforms[n];switch(this.uniforms[n]={},r.type){case"t":this.uniforms[n].value=t[r.value]||null;break;case"c":this.uniforms[n].value=new Tt().setHex(r.value);break;case"v2":this.uniforms[n].value=new Mt().fromArray(r.value);break;case"v3":this.uniforms[n].value=new K().fromArray(r.value);break;case"v4":this.uniforms[n].value=new Ot().fromArray(r.value);break;case"m3":this.uniforms[n].value=new nt().fromArray(r.value);break;case"m4":this.uniforms[n].value=new Vt().fromArray(r.value);break;default:this.uniforms[n].value=r.value}}if(e.defines!==void 0&&(this.defines=e.defines),e.vertexShader!==void 0&&(this.vertexShader=e.vertexShader),e.fragmentShader!==void 0&&(this.fragmentShader=e.fragmentShader),e.glslVersion!==void 0&&(this.glslVersion=e.glslVersion),e.extensions!==void 0)for(const n in e.extensions)this.extensions[n]=e.extensions[n];return e.lights!==void 0&&(this.lights=e.lights),e.clipping!==void 0&&(this.clipping=e.clipping),this}}class bc extends Yn{constructor(e){super(e),this.isRawShaderMaterial=!0,this.type="RawShaderMaterial"}}class Tu extends Ns{constructor(e){super(),this.isMeshDepthMaterial=!0,this.type="MeshDepthMaterial",this.depthPacking=Ud,this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.wireframe=!1,this.wireframeLinewidth=1,this.setValues(e)}copy(e){return super.copy(e),this.depthPacking=e.depthPacking,this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this.wireframe=e.wireframe,this.wireframeLinewidth=e.wireframeLinewidth,this}}class Au extends Ns{constructor(e){super(),this.isMeshDistanceMaterial=!0,this.type="MeshDistanceMaterial",this.map=null,this.alphaMap=null,this.displacementMap=null,this.displacementScale=1,this.displacementBias=0,this.setValues(e)}copy(e){return super.copy(e),this.map=e.map,this.alphaMap=e.alphaMap,this.displacementMap=e.displacementMap,this.displacementScale=e.displacementScale,this.displacementBias=e.displacementBias,this}}const ds=new K,us=new xr,On=new K;class Io extends mn{constructor(){super(),this.isCamera=!0,this.type="Camera",this.matrixWorldInverse=new Vt,this.projectionMatrix=new Vt,this.projectionMatrixInverse=new Vt,this.coordinateSystem=Hn,this._reversedDepth=!1}get reversedDepth(){return this._reversedDepth}copy(e,t){return super.copy(e,t),this.matrixWorldInverse.copy(e.matrixWorldInverse),this.projectionMatrix.copy(e.projectionMatrix),this.projectionMatrixInverse.copy(e.projectionMatrixInverse),this.coordinateSystem=e.coordinateSystem,this}getWorldDirection(e){return super.getWorldDirection(e).negate()}updateMatrixWorld(e){super.updateMatrixWorld(e),this.matrixWorld.decompose(ds,us,On),On.x===1&&On.y===1&&On.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(ds,us,On.set(1,1,1)).invert()}updateWorldMatrix(e,t,n=!1){super.updateWorldMatrix(e,t,n),this.matrixWorld.decompose(ds,us,On),On.x===1&&On.y===1&&On.z===1?this.matrixWorldInverse.copy(this.matrixWorld).invert():this.matrixWorldInverse.compose(ds,us,On.set(1,1,1)).invert()}clone(){return new this.constructor().copy(this)}}const xi=new K,dl=new Mt,ul=new Mt;class wn extends Io{constructor(e=50,t=1,n=.1,r=2e3){super(),this.isPerspectiveCamera=!0,this.type="PerspectiveCamera",this.fov=e,this.zoom=1,this.near=n,this.far=r,this.focus=10,this.aspect=t,this.view=null,this.filmGauge=35,this.filmOffset=0,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.fov=e.fov,this.zoom=e.zoom,this.near=e.near,this.far=e.far,this.focus=e.focus,this.aspect=e.aspect,this.view=e.view===null?null:Object.assign({},e.view),this.filmGauge=e.filmGauge,this.filmOffset=e.filmOffset,this}setFocalLength(e){const t=.5*this.getFilmHeight()/e;this.fov=go*2*Math.atan(t),this.updateProjectionMatrix()}getFocalLength(){const e=Math.tan(Ys*.5*this.fov);return .5*this.getFilmHeight()/e}getEffectiveFOV(){return go*2*Math.atan(Math.tan(Ys*.5*this.fov)/this.zoom)}getFilmWidth(){return this.filmGauge*Math.min(this.aspect,1)}getFilmHeight(){return this.filmGauge/Math.max(this.aspect,1)}getViewBounds(e,t,n){xi.set(-1,-1,.5).applyMatrix4(this.projectionMatrixInverse),t.set(xi.x,xi.y).multiplyScalar(-e/xi.z),xi.set(1,1,.5).applyMatrix4(this.projectionMatrixInverse),n.set(xi.x,xi.y).multiplyScalar(-e/xi.z)}getViewSize(e,t){return this.getViewBounds(e,dl,ul),t.subVectors(ul,dl)}setViewOffset(e,t,n,r,s,a){this.aspect=e/t,this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=n,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=this.near;let t=e*Math.tan(Ys*.5*this.fov)/this.zoom,n=2*t,r=this.aspect*n,s=-.5*r;const a=this.view;if(this.view!==null&&this.view.enabled){const c=a.fullWidth,l=a.fullHeight;s+=a.offsetX*r/c,t-=a.offsetY*n/l,r*=a.width/c,n*=a.height/l}const o=this.filmOffset;o!==0&&(s+=e*o/this.getFilmWidth()),this.projectionMatrix.makePerspective(s,s+r,t,t-n,e,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.fov=this.fov,t.object.zoom=this.zoom,t.object.near=this.near,t.object.far=this.far,t.object.focus=this.focus,t.object.aspect=this.aspect,this.view!==null&&(t.object.view=Object.assign({},this.view)),t.object.filmGauge=this.filmGauge,t.object.filmOffset=this.filmOffset,t}}class Ec extends Io{constructor(e=-1,t=1,n=1,r=-1,s=.1,a=2e3){super(),this.isOrthographicCamera=!0,this.type="OrthographicCamera",this.zoom=1,this.view=null,this.left=e,this.right=t,this.top=n,this.bottom=r,this.near=s,this.far=a,this.updateProjectionMatrix()}copy(e,t){return super.copy(e,t),this.left=e.left,this.right=e.right,this.top=e.top,this.bottom=e.bottom,this.near=e.near,this.far=e.far,this.zoom=e.zoom,this.view=e.view===null?null:Object.assign({},e.view),this}setViewOffset(e,t,n,r,s,a){this.view===null&&(this.view={enabled:!0,fullWidth:1,fullHeight:1,offsetX:0,offsetY:0,width:1,height:1}),this.view.enabled=!0,this.view.fullWidth=e,this.view.fullHeight=t,this.view.offsetX=n,this.view.offsetY=r,this.view.width=s,this.view.height=a,this.updateProjectionMatrix()}clearViewOffset(){this.view!==null&&(this.view.enabled=!1),this.updateProjectionMatrix()}updateProjectionMatrix(){const e=(this.right-this.left)/(2*this.zoom),t=(this.top-this.bottom)/(2*this.zoom),n=(this.right+this.left)/2,r=(this.top+this.bottom)/2;let s=n-e,a=n+e,o=r+t,c=r-t;if(this.view!==null&&this.view.enabled){const l=(this.right-this.left)/this.view.fullWidth/this.zoom,u=(this.top-this.bottom)/this.view.fullHeight/this.zoom;s+=l*this.view.offsetX,a=s+l*this.view.width,o-=u*this.view.offsetY,c=o-u*this.view.height}this.projectionMatrix.makeOrthographic(s,a,o,c,this.near,this.far,this.coordinateSystem,this.reversedDepth),this.projectionMatrixInverse.copy(this.projectionMatrix).invert()}toJSON(e){const t=super.toJSON(e);return t.object.zoom=this.zoom,t.object.left=this.left,t.object.right=this.right,t.object.top=this.top,t.object.bottom=this.bottom,t.object.near=this.near,t.object.far=this.far,this.view!==null&&(t.object.view=Object.assign({},this.view)),t}}const or=-90,lr=1;class wu extends mn{constructor(e,t,n){super(),this.type="CubeCamera",this.renderTarget=n,this.coordinateSystem=null,this.activeMipmapLevel=0;const r=new wn(or,lr,e,t);r.layers=this.layers,this.add(r);const s=new wn(or,lr,e,t);s.layers=this.layers,this.add(s);const a=new wn(or,lr,e,t);a.layers=this.layers,this.add(a);const o=new wn(or,lr,e,t);o.layers=this.layers,this.add(o);const c=new wn(or,lr,e,t);c.layers=this.layers,this.add(c);const l=new wn(or,lr,e,t);l.layers=this.layers,this.add(l)}updateCoordinateSystem(){const e=this.coordinateSystem,t=this.children.concat(),[n,r,s,a,o,c]=t;for(const l of t)this.remove(l);if(e===Hn)n.up.set(0,1,0),n.lookAt(1,0,0),r.up.set(0,1,0),r.lookAt(-1,0,0),s.up.set(0,0,-1),s.lookAt(0,1,0),a.up.set(0,0,1),a.lookAt(0,-1,0),o.up.set(0,1,0),o.lookAt(0,0,1),c.up.set(0,1,0),c.lookAt(0,0,-1);else if(e===Rs)n.up.set(0,-1,0),n.lookAt(-1,0,0),r.up.set(0,-1,0),r.lookAt(1,0,0),s.up.set(0,0,1),s.lookAt(0,1,0),a.up.set(0,0,-1),a.lookAt(0,-1,0),o.up.set(0,-1,0),o.lookAt(0,0,1),c.up.set(0,-1,0),c.lookAt(0,0,-1);else throw new Error("THREE.CubeCamera.updateCoordinateSystem(): Invalid coordinate system: "+e);for(const l of t)this.add(l),l.updateMatrixWorld()}update(e,t){this.parent===null&&this.updateMatrixWorld();const{renderTarget:n,activeMipmapLevel:r}=this;this.coordinateSystem!==e.coordinateSystem&&(this.coordinateSystem=e.coordinateSystem,this.updateCoordinateSystem());const[s,a,o,c,l,u]=this.children,f=e.getRenderTarget(),d=e.getActiveCubeFace(),m=e.getActiveMipmapLevel(),x=e.xr.enabled;e.xr.enabled=!1;const S=n.texture.generateMipmaps;n.texture.generateMipmaps=!1;let g=!1;e.isWebGLRenderer===!0?g=e.state.buffers.depth.getReversed():g=e.reversedDepthBuffer,e.setRenderTarget(n,0,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,s),e.setRenderTarget(n,1,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,a),e.setRenderTarget(n,2,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,o),e.setRenderTarget(n,3,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,c),e.setRenderTarget(n,4,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,l),n.texture.generateMipmaps=S,e.setRenderTarget(n,5,r),g&&e.autoClear===!1&&e.clearDepth(),e.render(t,u),e.setRenderTarget(f,d,m),e.xr.enabled=x,n.texture.needsPMREMUpdate=!0}}class Ru extends wn{constructor(e=[]){super(),this.isArrayCamera=!0,this.isMultiViewCamera=!1,this.cameras=e}}const zo=class zo{constructor(e,t,n,r){this.elements=[1,0,0,1],e!==void 0&&this.set(e,t,n,r)}identity(){return this.set(1,0,0,1),this}fromArray(e,t=0){for(let n=0;n<4;n++)this.elements[n]=e[n+t];return this}set(e,t,n,r){const s=this.elements;return s[0]=e,s[2]=t,s[1]=n,s[3]=r,this}};zo.prototype.isMatrix2=!0;let hl=zo;function fl(i,e,t,n){const r=Cu(n);switch(t){case cc:return i*e;case uc:return i*e/r.components*r.byteLength;case To:return i*e/r.components*r.byteLength;case Bi:return i*e*2/r.components*r.byteLength;case Ao:return i*e*2/r.components*r.byteLength;case dc:return i*e*3/r.components*r.byteLength;case Cn:return i*e*4/r.components*r.byteLength;case wo:return i*e*4/r.components*r.byteLength;case Ms:case Ss:return Math.floor((i+3)/4)*Math.floor((e+3)/4)*8;case ys:case bs:return Math.floor((i+3)/4)*Math.floor((e+3)/4)*16;case Ba:case ka:return Math.max(i,16)*Math.max(e,8)/4;case Oa:case za:return Math.max(i,8)*Math.max(e,8)/2;case Ga:case Va:case Wa:case Xa:return Math.floor((i+3)/4)*Math.floor((e+3)/4)*8;case Ha:case Ts:case qa:return Math.floor((i+3)/4)*Math.floor((e+3)/4)*16;case Ya:return Math.floor((i+3)/4)*Math.floor((e+3)/4)*16;case Ka:return Math.floor((i+4)/5)*Math.floor((e+3)/4)*16;case Za:return Math.floor((i+4)/5)*Math.floor((e+4)/5)*16;case $a:return Math.floor((i+5)/6)*Math.floor((e+4)/5)*16;case Ja:return Math.floor((i+5)/6)*Math.floor((e+5)/6)*16;case Qa:return Math.floor((i+7)/8)*Math.floor((e+4)/5)*16;case ja:return Math.floor((i+7)/8)*Math.floor((e+5)/6)*16;case eo:return Math.floor((i+7)/8)*Math.floor((e+7)/8)*16;case to:return Math.floor((i+9)/10)*Math.floor((e+4)/5)*16;case no:return Math.floor((i+9)/10)*Math.floor((e+5)/6)*16;case io:return Math.floor((i+9)/10)*Math.floor((e+7)/8)*16;case ro:return Math.floor((i+9)/10)*Math.floor((e+9)/10)*16;case so:return Math.floor((i+11)/12)*Math.floor((e+9)/10)*16;case ao:return Math.floor((i+11)/12)*Math.floor((e+11)/12)*16;case oo:case lo:case co:return Math.ceil(i/4)*Math.ceil(e/4)*16;case uo:case ho:return Math.ceil(i/4)*Math.ceil(e/4)*8;case As:case fo:return Math.ceil(i/4)*Math.ceil(e/4)*16}throw new Error(`Unable to determine texture byte length for ${t} format.`)}function Cu(i){switch(i){case yn:case sc:return{byteLength:1,components:1};case Nr:case ac:case li:return{byteLength:2,components:1};case bo:case Eo:return{byteLength:2,components:4};case Xn:case yo:case Vn:return{byteLength:4,components:1};case oc:case lc:return{byteLength:4,components:3}}throw new Error(`THREE.TextureUtils: Unknown texture type ${i}.`)}typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("register",{detail:{revision:So}}));typeof window<"u"&&(window.__THREE__?je("WARNING: Multiple instances of Three.js being imported."):window.__THREE__=So);function Tc(){let i=null,e=!1,t=null,n=null;function r(s,a){t(s,a),n=i.requestAnimationFrame(r)}return{start:function(){e!==!0&&t!==null&&i!==null&&(n=i.requestAnimationFrame(r),e=!0)},stop:function(){i!==null&&i.cancelAnimationFrame(n),e=!1},setAnimationLoop:function(s){t=s},setContext:function(s){i=s}}}function Pu(i){const e=new WeakMap;function t(o,c){const l=o.array,u=o.usage,f=l.byteLength,d=i.createBuffer();i.bindBuffer(c,d),i.bufferData(c,l,u),o.onUploadCallback();let m;if(l instanceof Float32Array)m=i.FLOAT;else if(typeof Float16Array<"u"&&l instanceof Float16Array)m=i.HALF_FLOAT;else if(l instanceof Uint16Array)o.isFloat16BufferAttribute?m=i.HALF_FLOAT:m=i.UNSIGNED_SHORT;else if(l instanceof Int16Array)m=i.SHORT;else if(l instanceof Uint32Array)m=i.UNSIGNED_INT;else if(l instanceof Int32Array)m=i.INT;else if(l instanceof Int8Array)m=i.BYTE;else if(l instanceof Uint8Array)m=i.UNSIGNED_BYTE;else if(l instanceof Uint8ClampedArray)m=i.UNSIGNED_BYTE;else throw new Error("THREE.WebGLAttributes: Unsupported buffer data format: "+l);return{buffer:d,type:m,bytesPerElement:l.BYTES_PER_ELEMENT,version:o.version,size:f}}function n(o,c,l){const u=c.array,f=c.updateRanges;if(i.bindBuffer(l,o),f.length===0)i.bufferSubData(l,0,u);else{f.sort((m,x)=>m.start-x.start);let d=0;for(let m=1;m<f.length;m++){const x=f[d],S=f[m];S.start<=x.start+x.count+1?x.count=Math.max(x.count,S.start+S.count-x.start):(++d,f[d]=S)}f.length=d+1;for(let m=0,x=f.length;m<x;m++){const S=f[m];i.bufferSubData(l,S.start*u.BYTES_PER_ELEMENT,u,S.start,S.count)}c.clearUpdateRanges()}c.onUploadCallback()}function r(o){return o.isInterleavedBufferAttribute&&(o=o.data),e.get(o)}function s(o){o.isInterleavedBufferAttribute&&(o=o.data);const c=e.get(o);c&&(i.deleteBuffer(c.buffer),e.delete(o))}function a(o,c){if(o.isInterleavedBufferAttribute&&(o=o.data),o.isGLBufferAttribute){const u=e.get(o);(!u||u.version<o.version)&&e.set(o,{buffer:o.buffer,type:o.type,bytesPerElement:o.elementSize,version:o.version});return}const l=e.get(o);if(l===void 0)e.set(o,t(o,c));else if(l.version<o.version){if(l.size!==o.array.byteLength)throw new Error("THREE.WebGLAttributes: The size of the buffer attribute's array buffer does not match the original size. Resizing buffer attributes is not supported.");n(l.buffer,o,c),l.version=o.version}}return{get:r,remove:s,update:a}}var Lu=`#ifdef USE_ALPHAHASH
	if ( diffuseColor.a < getAlphaHashThreshold( vPosition ) ) discard;
#endif`,Du=`#ifdef USE_ALPHAHASH
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
#endif`,Iu=`#ifdef USE_ALPHAMAP
	diffuseColor.a *= texture2D( alphaMap, vAlphaMapUv ).g;
#endif`,Uu=`#ifdef USE_ALPHAMAP
	uniform sampler2D alphaMap;
#endif`,Nu=`#ifdef USE_ALPHATEST
	#ifdef ALPHA_TO_COVERAGE
	diffuseColor.a = smoothstep( alphaTest, alphaTest + fwidth( diffuseColor.a ), diffuseColor.a );
	if ( diffuseColor.a == 0.0 ) discard;
	#else
	if ( diffuseColor.a < alphaTest ) discard;
	#endif
#endif`,Fu=`#ifdef USE_ALPHATEST
	uniform float alphaTest;
#endif`,Ou=`#ifdef USE_AOMAP
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
#endif`,Bu=`#ifdef USE_AOMAP
	uniform sampler2D aoMap;
	uniform float aoMapIntensity;
#endif`,zu=`#ifdef USE_BATCHING
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
#endif`,ku=`#ifdef USE_BATCHING
	mat4 batchingMatrix = getBatchingMatrix( getIndirectIndex( gl_DrawID ) );
#endif`,Gu=`vec3 transformed = vec3( position );
#ifdef USE_ALPHAHASH
	vPosition = vec3( position );
#endif`,Vu=`vec3 objectNormal = vec3( normal );
#ifdef USE_TANGENT
	vec3 objectTangent = vec3( tangent.xyz );
#endif`,Hu=`float G_BlinnPhong_Implicit( ) {
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
} // validated`,Wu=`#ifdef USE_IRIDESCENCE
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
#endif`,Xu=`#ifdef USE_BUMPMAP
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
#endif`,qu=`#if NUM_CLIPPING_PLANES > 0
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
#endif`,Yu=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
	uniform vec4 clippingPlanes[ NUM_CLIPPING_PLANES ];
#endif`,Ku=`#if NUM_CLIPPING_PLANES > 0
	varying vec3 vClipPosition;
#endif`,Zu=`#if NUM_CLIPPING_PLANES > 0
	vClipPosition = - mvPosition.xyz;
#endif`,$u=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	diffuseColor *= vColor;
#endif`,Ju=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA )
	varying vec4 vColor;
#endif`,Qu=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
	varying vec4 vColor;
#endif`,ju=`#if defined( USE_COLOR ) || defined( USE_COLOR_ALPHA ) || defined( USE_INSTANCING_COLOR ) || defined( USE_BATCHING_COLOR )
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
#endif`,eh=`#define PI 3.141592653589793
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
} // validated`,th=`#ifdef ENVMAP_TYPE_CUBE_UV
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
#endif`,nh=`vec3 transformedNormal = objectNormal;
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
#endif`,ih=`#ifdef USE_DISPLACEMENTMAP
	uniform sampler2D displacementMap;
	uniform float displacementScale;
	uniform float displacementBias;
#endif`,rh=`#ifdef USE_DISPLACEMENTMAP
	transformed += normalize( objectNormal ) * ( texture2D( displacementMap, vDisplacementMapUv ).x * displacementScale + displacementBias );
#endif`,sh=`#ifdef USE_EMISSIVEMAP
	vec4 emissiveColor = texture2D( emissiveMap, vEmissiveMapUv );
	#ifdef DECODE_VIDEO_TEXTURE_EMISSIVE
		emissiveColor = sRGBTransferEOTF( emissiveColor );
	#endif
	totalEmissiveRadiance *= emissiveColor.rgb;
#endif`,ah=`#ifdef USE_EMISSIVEMAP
	uniform sampler2D emissiveMap;
#endif`,oh="gl_FragColor = linearToOutputTexel( gl_FragColor );",lh=`vec4 LinearTransferOETF( in vec4 value ) {
	return value;
}
vec4 sRGBTransferEOTF( in vec4 value ) {
	return vec4( mix( pow( value.rgb * 0.9478672986 + vec3( 0.0521327014 ), vec3( 2.4 ) ), value.rgb * 0.0773993808, vec3( lessThanEqual( value.rgb, vec3( 0.04045 ) ) ) ), value.a );
}
vec4 sRGBTransferOETF( in vec4 value ) {
	return vec4( mix( pow( value.rgb, vec3( 0.41666 ) ) * 1.055 - vec3( 0.055 ), value.rgb * 12.92, vec3( lessThanEqual( value.rgb, vec3( 0.0031308 ) ) ) ), value.a );
}`,ch=`#ifdef USE_ENVMAP
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
#endif`,dh=`#ifdef USE_ENVMAP
	uniform float envMapIntensity;
	uniform mat3 envMapRotation;
	#ifdef ENVMAP_TYPE_CUBE
		uniform samplerCube envMap;
	#else
		uniform sampler2D envMap;
	#endif
#endif`,uh=`#ifdef USE_ENVMAP
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
#endif`,hh=`#ifdef USE_ENVMAP
	#if defined( USE_BUMPMAP ) || defined( USE_NORMALMAP ) || defined( PHONG ) || defined( LAMBERT )
		#define ENV_WORLDPOS
	#endif
	#ifdef ENV_WORLDPOS
		
		varying vec3 vWorldPosition;
	#else
		varying vec3 vReflect;
		uniform float refractionRatio;
	#endif
#endif`,fh=`#ifdef USE_ENVMAP
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
#endif`,ph=`#ifdef USE_FOG
	vFogDepth = - mvPosition.z;
#endif`,mh=`#ifdef USE_FOG
	varying float vFogDepth;
#endif`,gh=`#ifdef USE_FOG
	#ifdef FOG_EXP2
		float fogFactor = 1.0 - exp( - fogDensity * fogDensity * vFogDepth * vFogDepth );
	#else
		float fogFactor = smoothstep( fogNear, fogFar, vFogDepth );
	#endif
	gl_FragColor.rgb = mix( gl_FragColor.rgb, fogColor, fogFactor );
#endif`,_h=`#ifdef USE_FOG
	uniform vec3 fogColor;
	varying float vFogDepth;
	#ifdef FOG_EXP2
		uniform float fogDensity;
	#else
		uniform float fogNear;
		uniform float fogFar;
	#endif
#endif`,xh=`#ifdef USE_GRADIENTMAP
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
}`,vh=`#ifdef USE_LIGHTMAP
	uniform sampler2D lightMap;
	uniform float lightMapIntensity;
#endif`,Mh=`LambertMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularStrength = specularStrength;`,Sh=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Lambert`,yh=`uniform bool receiveShadow;
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
#include <lightprobes_pars_fragment>`,bh=`#ifdef USE_ENVMAP
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
#endif`,Eh=`ToonMaterial material;
material.diffuseColor = diffuseColor.rgb;`,Th=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_Toon`,Ah=`BlinnPhongMaterial material;
material.diffuseColor = diffuseColor.rgb;
material.specularColor = specular;
material.specularShininess = shininess;
material.specularStrength = specularStrength;`,wh=`varying vec3 vViewPosition;
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
#define RE_IndirectDiffuse		RE_IndirectDiffuse_BlinnPhong`,Rh=`PhysicalMaterial material;
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
#endif`,Ch=`uniform sampler2D dfgLUT;
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
}`,Ph=`
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
#endif`,Lh=`#if defined( RE_IndirectDiffuse )
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
#endif`,Dh=`#if defined( RE_IndirectDiffuse )
	#if defined( LAMBERT ) || defined( PHONG )
		irradiance += iblIrradiance;
	#endif
	RE_IndirectDiffuse( irradiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif
#if defined( RE_IndirectSpecular )
	RE_IndirectSpecular( radiance, iblIrradiance, clearcoatRadiance, geometryPosition, geometryNormal, geometryViewDir, geometryClearcoatNormal, material, reflectedLight );
#endif`,Ih=`#ifdef USE_LIGHT_PROBES_GRID
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
#endif`,Uh=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	gl_FragDepth = vIsPerspective == 0.0 ? gl_FragCoord.z : log2( vFragDepth ) * logDepthBufFC * 0.5;
#endif`,Nh=`#if defined( USE_LOGARITHMIC_DEPTH_BUFFER )
	uniform float logDepthBufFC;
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,Fh=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	varying float vFragDepth;
	varying float vIsPerspective;
#endif`,Oh=`#ifdef USE_LOGARITHMIC_DEPTH_BUFFER
	vFragDepth = 1.0 + gl_Position.w;
	vIsPerspective = float( isPerspectiveMatrix( projectionMatrix ) );
#endif`,Bh=`#ifdef USE_MAP
	vec4 sampledDiffuseColor = texture2D( map, vMapUv );
	#ifdef DECODE_VIDEO_TEXTURE
		sampledDiffuseColor = sRGBTransferEOTF( sampledDiffuseColor );
	#endif
	diffuseColor *= sampledDiffuseColor;
#endif`,zh=`#ifdef USE_MAP
	uniform sampler2D map;
#endif`,kh=`#if defined( USE_MAP ) || defined( USE_ALPHAMAP )
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
#endif`,Gh=`#if defined( USE_POINTS_UV )
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
#endif`,Vh=`float metalnessFactor = metalness;
#ifdef USE_METALNESSMAP
	vec4 texelMetalness = texture2D( metalnessMap, vMetalnessMapUv );
	metalnessFactor *= texelMetalness.b;
#endif`,Hh=`#ifdef USE_METALNESSMAP
	uniform sampler2D metalnessMap;
#endif`,Wh=`#ifdef USE_INSTANCING_MORPH
	float morphTargetInfluences[ MORPHTARGETS_COUNT ];
	float morphTargetBaseInfluence = texelFetch( morphTexture, ivec2( 0, gl_InstanceID ), 0 ).r;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		morphTargetInfluences[i] =  texelFetch( morphTexture, ivec2( i + 1, gl_InstanceID ), 0 ).r;
	}
#endif`,Xh=`#if defined( USE_MORPHCOLORS )
	vColor *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		#if defined( USE_COLOR_ALPHA )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ) * morphTargetInfluences[ i ];
		#elif defined( USE_COLOR )
			if ( morphTargetInfluences[ i ] != 0.0 ) vColor += getMorph( gl_VertexID, i, 2 ).rgb * morphTargetInfluences[ i ];
		#endif
	}
#endif`,qh=`#ifdef USE_MORPHNORMALS
	objectNormal *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) objectNormal += getMorph( gl_VertexID, i, 1 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,Yh=`#ifdef USE_MORPHTARGETS
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
#endif`,Kh=`#ifdef USE_MORPHTARGETS
	transformed *= morphTargetBaseInfluence;
	for ( int i = 0; i < MORPHTARGETS_COUNT; i ++ ) {
		if ( morphTargetInfluences[ i ] != 0.0 ) transformed += getMorph( gl_VertexID, i, 0 ).xyz * morphTargetInfluences[ i ];
	}
#endif`,Zh=`float faceDirection = gl_FrontFacing ? 1.0 : - 1.0;
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
vec3 nonPerturbedNormal = normal;`,$h=`#ifdef USE_NORMALMAP_OBJECTSPACE
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
#endif`,Jh=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,Qh=`#ifndef FLAT_SHADED
	varying vec3 vNormal;
	#ifdef USE_TANGENT
		varying vec3 vTangent;
		varying vec3 vBitangent;
	#endif
#endif`,jh=`#ifndef FLAT_SHADED
	vNormal = normalize( transformedNormal );
	#ifdef USE_TANGENT
		vTangent = normalize( transformedTangent );
		vBitangent = normalize( cross( vNormal, vTangent ) * tangent.w );
		#ifdef FLIP_SIDED
			vBitangent = - vBitangent;
		#endif
	#endif
#endif`,ef=`#ifdef USE_NORMALMAP
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
#endif`,tf=`#ifdef USE_CLEARCOAT
	vec3 clearcoatNormal = nonPerturbedNormal;
#endif`,nf=`#ifdef USE_CLEARCOAT_NORMALMAP
	vec3 clearcoatMapN = texture2D( clearcoatNormalMap, vClearcoatNormalMapUv ).xyz * 2.0 - 1.0;
	clearcoatMapN.xy *= clearcoatNormalScale;
	clearcoatNormal = normalize( tbn2 * clearcoatMapN );
#endif`,rf=`#ifdef USE_CLEARCOATMAP
	uniform sampler2D clearcoatMap;
#endif
#ifdef USE_CLEARCOAT_NORMALMAP
	uniform sampler2D clearcoatNormalMap;
	uniform vec2 clearcoatNormalScale;
#endif
#ifdef USE_CLEARCOAT_ROUGHNESSMAP
	uniform sampler2D clearcoatRoughnessMap;
#endif`,sf=`#ifdef USE_IRIDESCENCEMAP
	uniform sampler2D iridescenceMap;
#endif
#ifdef USE_IRIDESCENCE_THICKNESSMAP
	uniform sampler2D iridescenceThicknessMap;
#endif`,af=`#ifdef OPAQUE
diffuseColor.a = 1.0;
#endif
#ifdef USE_TRANSMISSION
diffuseColor.a *= material.transmissionAlpha;
#endif
gl_FragColor = vec4( outgoingLight, diffuseColor.a );`,of=`vec3 packNormalToRGB( const in vec3 normal ) {
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
}`,lf=`#ifdef PREMULTIPLIED_ALPHA
	gl_FragColor.rgb *= gl_FragColor.a;
#endif`,cf=`vec4 mvPosition = vec4( transformed, 1.0 );
#ifdef USE_BATCHING
	mvPosition = batchingMatrix * mvPosition;
#endif
#ifdef USE_INSTANCING
	mvPosition = instanceMatrix * mvPosition;
#endif
mvPosition = modelViewMatrix * mvPosition;
gl_Position = projectionMatrix * mvPosition;`,df=`#ifdef DITHERING
	gl_FragColor.rgb = dithering( gl_FragColor.rgb );
#endif`,uf=`#ifdef DITHERING
	vec3 dithering( vec3 color ) {
		float grid_position = rand( gl_FragCoord.xy );
		vec3 dither_shift_RGB = vec3( 0.25 / 255.0, -0.25 / 255.0, 0.25 / 255.0 );
		dither_shift_RGB = mix( 2.0 * dither_shift_RGB, -2.0 * dither_shift_RGB, grid_position );
		return color + dither_shift_RGB;
	}
#endif`,hf=`float roughnessFactor = roughness;
#ifdef USE_ROUGHNESSMAP
	vec4 texelRoughness = texture2D( roughnessMap, vRoughnessMapUv );
	roughnessFactor *= texelRoughness.g;
#endif`,ff=`#ifdef USE_ROUGHNESSMAP
	uniform sampler2D roughnessMap;
#endif`,pf=`#if NUM_SPOT_LIGHT_COORDS > 0
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
#endif`,mf=`#if NUM_SPOT_LIGHT_COORDS > 0
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
#endif`,gf=`#if ( defined( USE_SHADOWMAP ) && ( NUM_DIR_LIGHT_SHADOWS > 0 || NUM_POINT_LIGHT_SHADOWS > 0 ) ) || ( NUM_SPOT_LIGHT_COORDS > 0 )
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
#endif`,_f=`float getShadowMask() {
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
}`,xf=`#ifdef USE_SKINNING
	mat4 boneMatX = getBoneMatrix( skinIndex.x );
	mat4 boneMatY = getBoneMatrix( skinIndex.y );
	mat4 boneMatZ = getBoneMatrix( skinIndex.z );
	mat4 boneMatW = getBoneMatrix( skinIndex.w );
#endif`,vf=`#ifdef USE_SKINNING
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
#endif`,Mf=`#ifdef USE_SKINNING
	vec4 skinVertex = bindMatrix * vec4( transformed, 1.0 );
	vec4 skinned = vec4( 0.0 );
	skinned += boneMatX * skinVertex * skinWeight.x;
	skinned += boneMatY * skinVertex * skinWeight.y;
	skinned += boneMatZ * skinVertex * skinWeight.z;
	skinned += boneMatW * skinVertex * skinWeight.w;
	transformed = ( bindMatrixInverse * skinned ).xyz;
#endif`,Sf=`#ifdef USE_SKINNING
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
#endif`,yf=`float specularStrength;
#ifdef USE_SPECULARMAP
	vec4 texelSpecular = texture2D( specularMap, vSpecularMapUv );
	specularStrength = texelSpecular.r;
#else
	specularStrength = 1.0;
#endif`,bf=`#ifdef USE_SPECULARMAP
	uniform sampler2D specularMap;
#endif`,Ef=`#if defined( TONE_MAPPING )
	gl_FragColor.rgb = toneMapping( gl_FragColor.rgb );
#endif`,Tf=`#ifndef saturate
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
vec3 CustomToneMapping( vec3 color ) { return color; }`,Af=`#ifdef USE_TRANSMISSION
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
#endif`,wf=`#ifdef USE_TRANSMISSION
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
#endif`,Rf=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,Cf=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,Pf=`#if defined( USE_UV ) || defined( USE_ANISOTROPY )
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
#endif`,Lf=`#if defined( USE_ENVMAP ) || defined( DISTANCE ) || defined ( USE_SHADOWMAP ) || defined ( USE_TRANSMISSION ) || NUM_SPOT_LIGHT_COORDS > 0
	vec4 worldPosition = vec4( transformed, 1.0 );
	#ifdef USE_BATCHING
		worldPosition = batchingMatrix * worldPosition;
	#endif
	#ifdef USE_INSTANCING
		worldPosition = instanceMatrix * worldPosition;
	#endif
	worldPosition = modelMatrix * worldPosition;
#endif`;const Df=`varying vec2 vUv;
uniform mat3 uvTransform;
void main() {
	vUv = ( uvTransform * vec3( uv, 1 ) ).xy;
	gl_Position = vec4( position.xy, 1.0, 1.0 );
}`,If=`uniform sampler2D t2D;
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
}`,Uf=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,Nf=`#ifdef ENVMAP_TYPE_CUBE
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
}`,Ff=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
	gl_Position.z = gl_Position.w;
}`,Of=`uniform samplerCube tCube;
uniform float tFlip;
uniform float opacity;
varying vec3 vWorldDirection;
void main() {
	vec4 texColor = textureCube( tCube, vec3( tFlip * vWorldDirection.x, vWorldDirection.yz ) );
	gl_FragColor = texColor;
	gl_FragColor.a *= opacity;
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,Bf=`#include <common>
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
}`,zf=`#if DEPTH_PACKING == 3200
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
}`,kf=`#define DISTANCE
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
}`,Gf=`#define DISTANCE
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
}`,Vf=`varying vec3 vWorldDirection;
#include <common>
void main() {
	vWorldDirection = transformDirection( position, modelMatrix );
	#include <begin_vertex>
	#include <project_vertex>
}`,Hf=`uniform sampler2D tEquirect;
varying vec3 vWorldDirection;
#include <common>
void main() {
	vec3 direction = normalize( vWorldDirection );
	vec2 sampleUV = equirectUv( direction );
	gl_FragColor = texture2D( tEquirect, sampleUV );
	#include <tonemapping_fragment>
	#include <colorspace_fragment>
}`,Wf=`uniform float scale;
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
}`,Xf=`uniform vec3 diffuse;
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
}`,qf=`#include <common>
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
}`,Yf=`uniform vec3 diffuse;
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
}`,Kf=`#define LAMBERT
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
}`,Zf=`#define LAMBERT
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
}`,$f=`#define MATCAP
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
}`,Jf=`#define MATCAP
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
}`,Qf=`#define NORMAL
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
}`,jf=`#define NORMAL
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
}`,ep=`#define PHONG
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
}`,tp=`#define PHONG
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
}`,np=`#define STANDARD
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
}`,ip=`#define STANDARD
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
}`,rp=`#define TOON
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
}`,sp=`#define TOON
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
}`,ap=`uniform float size;
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
}`,op=`uniform vec3 diffuse;
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
}`,lp=`#include <common>
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
}`,cp=`uniform vec3 color;
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
}`,dp=`uniform float rotation;
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
}`,up=`uniform vec3 diffuse;
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
}`,ct={alphahash_fragment:Lu,alphahash_pars_fragment:Du,alphamap_fragment:Iu,alphamap_pars_fragment:Uu,alphatest_fragment:Nu,alphatest_pars_fragment:Fu,aomap_fragment:Ou,aomap_pars_fragment:Bu,batching_pars_vertex:zu,batching_vertex:ku,begin_vertex:Gu,beginnormal_vertex:Vu,bsdfs:Hu,iridescence_fragment:Wu,bumpmap_pars_fragment:Xu,clipping_planes_fragment:qu,clipping_planes_pars_fragment:Yu,clipping_planes_pars_vertex:Ku,clipping_planes_vertex:Zu,color_fragment:$u,color_pars_fragment:Ju,color_pars_vertex:Qu,color_vertex:ju,common:eh,cube_uv_reflection_fragment:th,defaultnormal_vertex:nh,displacementmap_pars_vertex:ih,displacementmap_vertex:rh,emissivemap_fragment:sh,emissivemap_pars_fragment:ah,colorspace_fragment:oh,colorspace_pars_fragment:lh,envmap_fragment:ch,envmap_common_pars_fragment:dh,envmap_pars_fragment:uh,envmap_pars_vertex:hh,envmap_physical_pars_fragment:bh,envmap_vertex:fh,fog_vertex:ph,fog_pars_vertex:mh,fog_fragment:gh,fog_pars_fragment:_h,gradientmap_pars_fragment:xh,lightmap_pars_fragment:vh,lights_lambert_fragment:Mh,lights_lambert_pars_fragment:Sh,lights_pars_begin:yh,lights_toon_fragment:Eh,lights_toon_pars_fragment:Th,lights_phong_fragment:Ah,lights_phong_pars_fragment:wh,lights_physical_fragment:Rh,lights_physical_pars_fragment:Ch,lights_fragment_begin:Ph,lights_fragment_maps:Lh,lights_fragment_end:Dh,lightprobes_pars_fragment:Ih,logdepthbuf_fragment:Uh,logdepthbuf_pars_fragment:Nh,logdepthbuf_pars_vertex:Fh,logdepthbuf_vertex:Oh,map_fragment:Bh,map_pars_fragment:zh,map_particle_fragment:kh,map_particle_pars_fragment:Gh,metalnessmap_fragment:Vh,metalnessmap_pars_fragment:Hh,morphinstance_vertex:Wh,morphcolor_vertex:Xh,morphnormal_vertex:qh,morphtarget_pars_vertex:Yh,morphtarget_vertex:Kh,normal_fragment_begin:Zh,normal_fragment_maps:$h,normal_pars_fragment:Jh,normal_pars_vertex:Qh,normal_vertex:jh,normalmap_pars_fragment:ef,clearcoat_normal_fragment_begin:tf,clearcoat_normal_fragment_maps:nf,clearcoat_pars_fragment:rf,iridescence_pars_fragment:sf,opaque_fragment:af,packing:of,premultiplied_alpha_fragment:lf,project_vertex:cf,dithering_fragment:df,dithering_pars_fragment:uf,roughnessmap_fragment:hf,roughnessmap_pars_fragment:ff,shadowmap_pars_fragment:pf,shadowmap_pars_vertex:mf,shadowmap_vertex:gf,shadowmask_pars_fragment:_f,skinbase_vertex:xf,skinning_pars_vertex:vf,skinning_vertex:Mf,skinnormal_vertex:Sf,specularmap_fragment:yf,specularmap_pars_fragment:bf,tonemapping_fragment:Ef,tonemapping_pars_fragment:Tf,transmission_fragment:Af,transmission_pars_fragment:wf,uv_pars_fragment:Rf,uv_pars_vertex:Cf,uv_vertex:Pf,worldpos_vertex:Lf,background_vert:Df,background_frag:If,backgroundCube_vert:Uf,backgroundCube_frag:Nf,cube_vert:Ff,cube_frag:Of,depth_vert:Bf,depth_frag:zf,distance_vert:kf,distance_frag:Gf,equirect_vert:Vf,equirect_frag:Hf,linedashed_vert:Wf,linedashed_frag:Xf,meshbasic_vert:qf,meshbasic_frag:Yf,meshlambert_vert:Kf,meshlambert_frag:Zf,meshmatcap_vert:$f,meshmatcap_frag:Jf,meshnormal_vert:Qf,meshnormal_frag:jf,meshphong_vert:ep,meshphong_frag:tp,meshphysical_vert:np,meshphysical_frag:ip,meshtoon_vert:rp,meshtoon_frag:sp,points_vert:ap,points_frag:op,shadow_vert:lp,shadow_frag:cp,sprite_vert:dp,sprite_frag:up},Ee={common:{diffuse:{value:new Tt(16777215)},opacity:{value:1},map:{value:null},mapTransform:{value:new nt},alphaMap:{value:null},alphaMapTransform:{value:new nt},alphaTest:{value:0}},specularmap:{specularMap:{value:null},specularMapTransform:{value:new nt}},envmap:{envMap:{value:null},envMapRotation:{value:new nt},reflectivity:{value:1},ior:{value:1.5},refractionRatio:{value:.98},dfgLUT:{value:null}},aomap:{aoMap:{value:null},aoMapIntensity:{value:1},aoMapTransform:{value:new nt}},lightmap:{lightMap:{value:null},lightMapIntensity:{value:1},lightMapTransform:{value:new nt}},bumpmap:{bumpMap:{value:null},bumpMapTransform:{value:new nt},bumpScale:{value:1}},normalmap:{normalMap:{value:null},normalMapTransform:{value:new nt},normalScale:{value:new Mt(1,1)}},displacementmap:{displacementMap:{value:null},displacementMapTransform:{value:new nt},displacementScale:{value:1},displacementBias:{value:0}},emissivemap:{emissiveMap:{value:null},emissiveMapTransform:{value:new nt}},metalnessmap:{metalnessMap:{value:null},metalnessMapTransform:{value:new nt}},roughnessmap:{roughnessMap:{value:null},roughnessMapTransform:{value:new nt}},gradientmap:{gradientMap:{value:null}},fog:{fogDensity:{value:25e-5},fogNear:{value:1},fogFar:{value:2e3},fogColor:{value:new Tt(16777215)}},lights:{ambientLightColor:{value:[]},lightProbe:{value:[]},directionalLights:{value:[],properties:{direction:{},color:{}}},directionalLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},directionalShadowMatrix:{value:[]},spotLights:{value:[],properties:{color:{},position:{},direction:{},distance:{},coneCos:{},penumbraCos:{},decay:{}}},spotLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{}}},spotLightMap:{value:[]},spotLightMatrix:{value:[]},pointLights:{value:[],properties:{color:{},position:{},decay:{},distance:{}}},pointLightShadows:{value:[],properties:{shadowIntensity:1,shadowBias:{},shadowNormalBias:{},shadowRadius:{},shadowMapSize:{},shadowCameraNear:{},shadowCameraFar:{}}},pointShadowMatrix:{value:[]},hemisphereLights:{value:[],properties:{direction:{},skyColor:{},groundColor:{}}},rectAreaLights:{value:[],properties:{color:{},position:{},width:{},height:{}}},ltc_1:{value:null},ltc_2:{value:null},probesSH:{value:null},probesMin:{value:new K},probesMax:{value:new K},probesResolution:{value:new K}},points:{diffuse:{value:new Tt(16777215)},opacity:{value:1},size:{value:1},scale:{value:1},map:{value:null},alphaMap:{value:null},alphaMapTransform:{value:new nt},alphaTest:{value:0},uvTransform:{value:new nt}},sprite:{diffuse:{value:new Tt(16777215)},opacity:{value:1},center:{value:new Mt(.5,.5)},rotation:{value:0},map:{value:null},mapTransform:{value:new nt},alphaMap:{value:null},alphaMapTransform:{value:new nt},alphaTest:{value:0}}},kn={basic:{uniforms:sn([Ee.common,Ee.specularmap,Ee.envmap,Ee.aomap,Ee.lightmap,Ee.fog]),vertexShader:ct.meshbasic_vert,fragmentShader:ct.meshbasic_frag},lambert:{uniforms:sn([Ee.common,Ee.specularmap,Ee.envmap,Ee.aomap,Ee.lightmap,Ee.emissivemap,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,Ee.fog,Ee.lights,{emissive:{value:new Tt(0)},envMapIntensity:{value:1}}]),vertexShader:ct.meshlambert_vert,fragmentShader:ct.meshlambert_frag},phong:{uniforms:sn([Ee.common,Ee.specularmap,Ee.envmap,Ee.aomap,Ee.lightmap,Ee.emissivemap,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,Ee.fog,Ee.lights,{emissive:{value:new Tt(0)},specular:{value:new Tt(1118481)},shininess:{value:30},envMapIntensity:{value:1}}]),vertexShader:ct.meshphong_vert,fragmentShader:ct.meshphong_frag},standard:{uniforms:sn([Ee.common,Ee.envmap,Ee.aomap,Ee.lightmap,Ee.emissivemap,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,Ee.roughnessmap,Ee.metalnessmap,Ee.fog,Ee.lights,{emissive:{value:new Tt(0)},roughness:{value:1},metalness:{value:0},envMapIntensity:{value:1}}]),vertexShader:ct.meshphysical_vert,fragmentShader:ct.meshphysical_frag},toon:{uniforms:sn([Ee.common,Ee.aomap,Ee.lightmap,Ee.emissivemap,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,Ee.gradientmap,Ee.fog,Ee.lights,{emissive:{value:new Tt(0)}}]),vertexShader:ct.meshtoon_vert,fragmentShader:ct.meshtoon_frag},matcap:{uniforms:sn([Ee.common,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,Ee.fog,{matcap:{value:null}}]),vertexShader:ct.meshmatcap_vert,fragmentShader:ct.meshmatcap_frag},points:{uniforms:sn([Ee.points,Ee.fog]),vertexShader:ct.points_vert,fragmentShader:ct.points_frag},dashed:{uniforms:sn([Ee.common,Ee.fog,{scale:{value:1},dashSize:{value:1},totalSize:{value:2}}]),vertexShader:ct.linedashed_vert,fragmentShader:ct.linedashed_frag},depth:{uniforms:sn([Ee.common,Ee.displacementmap]),vertexShader:ct.depth_vert,fragmentShader:ct.depth_frag},normal:{uniforms:sn([Ee.common,Ee.bumpmap,Ee.normalmap,Ee.displacementmap,{opacity:{value:1}}]),vertexShader:ct.meshnormal_vert,fragmentShader:ct.meshnormal_frag},sprite:{uniforms:sn([Ee.sprite,Ee.fog]),vertexShader:ct.sprite_vert,fragmentShader:ct.sprite_frag},background:{uniforms:{uvTransform:{value:new nt},t2D:{value:null},backgroundIntensity:{value:1}},vertexShader:ct.background_vert,fragmentShader:ct.background_frag},backgroundCube:{uniforms:{envMap:{value:null},backgroundBlurriness:{value:0},backgroundIntensity:{value:1},backgroundRotation:{value:new nt}},vertexShader:ct.backgroundCube_vert,fragmentShader:ct.backgroundCube_frag},cube:{uniforms:{tCube:{value:null},tFlip:{value:-1},opacity:{value:1}},vertexShader:ct.cube_vert,fragmentShader:ct.cube_frag},equirect:{uniforms:{tEquirect:{value:null}},vertexShader:ct.equirect_vert,fragmentShader:ct.equirect_frag},distance:{uniforms:sn([Ee.common,Ee.displacementmap,{referencePosition:{value:new K},nearDistance:{value:1},farDistance:{value:1e3}}]),vertexShader:ct.distance_vert,fragmentShader:ct.distance_frag},shadow:{uniforms:sn([Ee.lights,Ee.fog,{color:{value:new Tt(0)},opacity:{value:1}}]),vertexShader:ct.shadow_vert,fragmentShader:ct.shadow_frag}};kn.physical={uniforms:sn([kn.standard.uniforms,{clearcoat:{value:0},clearcoatMap:{value:null},clearcoatMapTransform:{value:new nt},clearcoatNormalMap:{value:null},clearcoatNormalMapTransform:{value:new nt},clearcoatNormalScale:{value:new Mt(1,1)},clearcoatRoughness:{value:0},clearcoatRoughnessMap:{value:null},clearcoatRoughnessMapTransform:{value:new nt},dispersion:{value:0},iridescence:{value:0},iridescenceMap:{value:null},iridescenceMapTransform:{value:new nt},iridescenceIOR:{value:1.3},iridescenceThicknessMinimum:{value:100},iridescenceThicknessMaximum:{value:400},iridescenceThicknessMap:{value:null},iridescenceThicknessMapTransform:{value:new nt},sheen:{value:0},sheenColor:{value:new Tt(0)},sheenColorMap:{value:null},sheenColorMapTransform:{value:new nt},sheenRoughness:{value:1},sheenRoughnessMap:{value:null},sheenRoughnessMapTransform:{value:new nt},transmission:{value:0},transmissionMap:{value:null},transmissionMapTransform:{value:new nt},transmissionSamplerSize:{value:new Mt},transmissionSamplerMap:{value:null},thickness:{value:0},thicknessMap:{value:null},thicknessMapTransform:{value:new nt},attenuationDistance:{value:0},attenuationColor:{value:new Tt(0)},specularColor:{value:new Tt(1,1,1)},specularColorMap:{value:null},specularColorMapTransform:{value:new nt},specularIntensity:{value:1},specularIntensityMap:{value:null},specularIntensityMapTransform:{value:new nt},anisotropyVector:{value:new Mt},anisotropyMap:{value:null},anisotropyMapTransform:{value:new nt}}]),vertexShader:ct.meshphysical_vert,fragmentShader:ct.meshphysical_frag};const hs={r:0,b:0,g:0},hp=new Vt,Ac=new nt;Ac.set(-1,0,0,0,1,0,0,0,1);function fp(i,e,t,n,r,s){const a=new Tt(0);let o=r===!0?0:1,c,l,u=null,f=0,d=null;function m(R){let C=R.isScene===!0?R.background:null;if(C&&C.isTexture){const y=R.backgroundBlurriness>0;C=e.get(C,y)}return C}function x(R){let C=!1;const y=m(R);y===null?g(a,o):y&&y.isColor&&(g(y,1),C=!0);const w=i.xr.getEnvironmentBlendMode();w==="additive"?t.buffers.color.setClear(0,0,0,1,s):w==="alpha-blend"&&t.buffers.color.setClear(0,0,0,0,s),(i.autoClear||C)&&(t.buffers.depth.setTest(!0),t.buffers.depth.setMask(!0),t.buffers.color.setMask(!0),i.clear(i.autoClearColor,i.autoClearDepth,i.autoClearStencil))}function S(R,C){const y=m(C);y&&(y.isCubeTexture||y.mapping===Us)?(l===void 0&&(l=new qn(new zr(1,1,1),new Yn({name:"BackgroundCubeMaterial",uniforms:_r(kn.backgroundCube.uniforms),vertexShader:kn.backgroundCube.vertexShader,fragmentShader:kn.backgroundCube.fragmentShader,side:cn,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),l.geometry.deleteAttribute("normal"),l.geometry.deleteAttribute("uv"),l.onBeforeRender=function(w,T,I){this.matrixWorld.copyPosition(I.matrixWorld)},Object.defineProperty(l.material,"envMap",{get:function(){return this.uniforms.envMap.value}}),n.update(l)),l.material.uniforms.envMap.value=y,l.material.uniforms.backgroundBlurriness.value=C.backgroundBlurriness,l.material.uniforms.backgroundIntensity.value=C.backgroundIntensity,l.material.uniforms.backgroundRotation.value.setFromMatrix4(hp.makeRotationFromEuler(C.backgroundRotation)).transpose(),y.isCubeTexture&&y.isRenderTargetTexture===!1&&l.material.uniforms.backgroundRotation.value.premultiply(Ac),l.material.toneMapped=mt.getTransfer(y.colorSpace)!==Ct,(u!==y||f!==y.version||d!==i.toneMapping)&&(l.material.needsUpdate=!0,u=y,f=y.version,d=i.toneMapping),l.layers.enableAll(),R.unshift(l,l.geometry,l.material,0,0,null)):y&&y.isTexture&&(c===void 0&&(c=new qn(new Fs(2,2),new Yn({name:"BackgroundMaterial",uniforms:_r(kn.background.uniforms),vertexShader:kn.background.vertexShader,fragmentShader:kn.background.fragmentShader,side:yi,depthTest:!1,depthWrite:!1,fog:!1,allowOverride:!1})),c.geometry.deleteAttribute("normal"),Object.defineProperty(c.material,"map",{get:function(){return this.uniforms.t2D.value}}),n.update(c)),c.material.uniforms.t2D.value=y,c.material.uniforms.backgroundIntensity.value=C.backgroundIntensity,c.material.toneMapped=mt.getTransfer(y.colorSpace)!==Ct,y.matrixAutoUpdate===!0&&y.updateMatrix(),c.material.uniforms.uvTransform.value.copy(y.matrix),(u!==y||f!==y.version||d!==i.toneMapping)&&(c.material.needsUpdate=!0,u=y,f=y.version,d=i.toneMapping),c.layers.enableAll(),R.unshift(c,c.geometry,c.material,0,0,null))}function g(R,C){R.getRGB(hs,yc(i)),t.buffers.color.setClear(hs.r,hs.g,hs.b,C,s)}function h(){l!==void 0&&(l.geometry.dispose(),l.material.dispose(),l=void 0),c!==void 0&&(c.geometry.dispose(),c.material.dispose(),c=void 0)}return{getClearColor:function(){return a},setClearColor:function(R,C=1){a.set(R),o=C,g(a,o)},getClearAlpha:function(){return o},setClearAlpha:function(R){o=R,g(a,o)},render:x,addToRenderList:S,dispose:h}}function pp(i,e){const t=i.getParameter(i.MAX_VERTEX_ATTRIBS),n={},r=d(null);let s=r,a=!1;function o(F,b,P,W,N){let D=!1;const G=f(F,W,P,b);s!==G&&(s=G,l(s.object)),D=m(F,W,P,N),D&&x(F,W,P,N),N!==null&&e.update(N,i.ELEMENT_ARRAY_BUFFER),(D||a)&&(a=!1,y(F,b,P,W),N!==null&&i.bindBuffer(i.ELEMENT_ARRAY_BUFFER,e.get(N).buffer))}function c(){return i.createVertexArray()}function l(F){return i.bindVertexArray(F)}function u(F){return i.deleteVertexArray(F)}function f(F,b,P,W){const N=W.wireframe===!0;let D=n[b.id];D===void 0&&(D={},n[b.id]=D);const G=F.isInstancedMesh===!0?F.id:0;let H=D[G];H===void 0&&(H={},D[G]=H);let ne=H[P.id];ne===void 0&&(ne={},H[P.id]=ne);let ce=ne[N];return ce===void 0&&(ce=d(c()),ne[N]=ce),ce}function d(F){const b=[],P=[],W=[];for(let N=0;N<t;N++)b[N]=0,P[N]=0,W[N]=0;return{geometry:null,program:null,wireframe:!1,newAttributes:b,enabledAttributes:P,attributeDivisors:W,object:F,attributes:{},index:null}}function m(F,b,P,W){const N=s.attributes,D=b.attributes;let G=0;const H=P.getAttributes();for(const ne in H)if(H[ne].location>=0){const oe=N[ne];let _e=D[ne];if(_e===void 0&&(ne==="instanceMatrix"&&F.instanceMatrix&&(_e=F.instanceMatrix),ne==="instanceColor"&&F.instanceColor&&(_e=F.instanceColor)),oe===void 0||oe.attribute!==_e||_e&&oe.data!==_e.data)return!0;G++}return s.attributesNum!==G||s.index!==W}function x(F,b,P,W){const N={},D=b.attributes;let G=0;const H=P.getAttributes();for(const ne in H)if(H[ne].location>=0){let oe=D[ne];oe===void 0&&(ne==="instanceMatrix"&&F.instanceMatrix&&(oe=F.instanceMatrix),ne==="instanceColor"&&F.instanceColor&&(oe=F.instanceColor));const _e={};_e.attribute=oe,oe&&oe.data&&(_e.data=oe.data),N[ne]=_e,G++}s.attributes=N,s.attributesNum=G,s.index=W}function S(){const F=s.newAttributes;for(let b=0,P=F.length;b<P;b++)F[b]=0}function g(F){h(F,0)}function h(F,b){const P=s.newAttributes,W=s.enabledAttributes,N=s.attributeDivisors;P[F]=1,W[F]===0&&(i.enableVertexAttribArray(F),W[F]=1),N[F]!==b&&(i.vertexAttribDivisor(F,b),N[F]=b)}function R(){const F=s.newAttributes,b=s.enabledAttributes;for(let P=0,W=b.length;P<W;P++)b[P]!==F[P]&&(i.disableVertexAttribArray(P),b[P]=0)}function C(F,b,P,W,N,D,G){G===!0?i.vertexAttribIPointer(F,b,P,N,D):i.vertexAttribPointer(F,b,P,W,N,D)}function y(F,b,P,W){S();const N=W.attributes,D=P.getAttributes(),G=b.defaultAttributeValues;for(const H in D){const ne=D[H];if(ne.location>=0){let ce=N[H];if(ce===void 0&&(H==="instanceMatrix"&&F.instanceMatrix&&(ce=F.instanceMatrix),H==="instanceColor"&&F.instanceColor&&(ce=F.instanceColor)),ce!==void 0){const oe=ce.normalized,_e=ce.itemSize,Qe=e.get(ce);if(Qe===void 0)continue;const Ze=Qe.buffer,He=Qe.type,Z=Qe.bytesPerElement,de=He===i.INT||He===i.UNSIGNED_INT||ce.gpuType===yo;if(ce.isInterleavedBufferAttribute){const le=ce.data,Oe=le.stride,Je=ce.offset;if(le.isInstancedInterleavedBuffer){for(let Te=0;Te<ne.locationSize;Te++)h(ne.location+Te,le.meshPerAttribute);F.isInstancedMesh!==!0&&W._maxInstanceCount===void 0&&(W._maxInstanceCount=le.meshPerAttribute*le.count)}else for(let Te=0;Te<ne.locationSize;Te++)g(ne.location+Te);i.bindBuffer(i.ARRAY_BUFFER,Ze);for(let Te=0;Te<ne.locationSize;Te++)C(ne.location+Te,_e/ne.locationSize,He,oe,Oe*Z,(Je+_e/ne.locationSize*Te)*Z,de)}else{if(ce.isInstancedBufferAttribute){for(let le=0;le<ne.locationSize;le++)h(ne.location+le,ce.meshPerAttribute);F.isInstancedMesh!==!0&&W._maxInstanceCount===void 0&&(W._maxInstanceCount=ce.meshPerAttribute*ce.count)}else for(let le=0;le<ne.locationSize;le++)g(ne.location+le);i.bindBuffer(i.ARRAY_BUFFER,Ze);for(let le=0;le<ne.locationSize;le++)C(ne.location+le,_e/ne.locationSize,He,oe,_e*Z,_e/ne.locationSize*le*Z,de)}}else if(G!==void 0){const oe=G[H];if(oe!==void 0)switch(oe.length){case 2:i.vertexAttrib2fv(ne.location,oe);break;case 3:i.vertexAttrib3fv(ne.location,oe);break;case 4:i.vertexAttrib4fv(ne.location,oe);break;default:i.vertexAttrib1fv(ne.location,oe)}}}}R()}function w(){A();for(const F in n){const b=n[F];for(const P in b){const W=b[P];for(const N in W){const D=W[N];for(const G in D)u(D[G].object),delete D[G];delete W[N]}}delete n[F]}}function T(F){if(n[F.id]===void 0)return;const b=n[F.id];for(const P in b){const W=b[P];for(const N in W){const D=W[N];for(const G in D)u(D[G].object),delete D[G];delete W[N]}}delete n[F.id]}function I(F){for(const b in n){const P=n[b];for(const W in P){const N=P[W];if(N[F.id]===void 0)continue;const D=N[F.id];for(const G in D)u(D[G].object),delete D[G];delete N[F.id]}}}function _(F){for(const b in n){const P=n[b],W=F.isInstancedMesh===!0?F.id:0,N=P[W];if(N!==void 0){for(const D in N){const G=N[D];for(const H in G)u(G[H].object),delete G[H];delete N[D]}delete P[W],Object.keys(P).length===0&&delete n[b]}}}function A(){k(),a=!0,s!==r&&(s=r,l(s.object))}function k(){r.geometry=null,r.program=null,r.wireframe=!1}return{setup:o,reset:A,resetDefaultState:k,dispose:w,releaseStatesOfGeometry:T,releaseStatesOfObject:_,releaseStatesOfProgram:I,initAttributes:S,enableAttribute:g,disableUnusedAttributes:R}}function mp(i,e,t){let n;function r(c){n=c}function s(c,l){i.drawArrays(n,c,l),t.update(l,n,1)}function a(c,l,u){u!==0&&(i.drawArraysInstanced(n,c,l,u),t.update(l,n,u))}function o(c,l,u){if(u===0)return;e.get("WEBGL_multi_draw").multiDrawArraysWEBGL(n,c,0,l,0,u);let d=0;for(let m=0;m<u;m++)d+=l[m];t.update(d,n,1)}this.setMode=r,this.render=s,this.renderInstances=a,this.renderMultiDraw=o}function gp(i,e,t,n){let r;function s(){if(r!==void 0)return r;if(e.has("EXT_texture_filter_anisotropic")===!0){const I=e.get("EXT_texture_filter_anisotropic");r=i.getParameter(I.MAX_TEXTURE_MAX_ANISOTROPY_EXT)}else r=0;return r}function a(I){return!(I!==Cn&&n.convert(I)!==i.getParameter(i.IMPLEMENTATION_COLOR_READ_FORMAT))}function o(I){const _=I===li&&(e.has("EXT_color_buffer_half_float")||e.has("EXT_color_buffer_float"));return!(I!==yn&&n.convert(I)!==i.getParameter(i.IMPLEMENTATION_COLOR_READ_TYPE)&&I!==Vn&&!_)}function c(I){if(I==="highp"){if(i.getShaderPrecisionFormat(i.VERTEX_SHADER,i.HIGH_FLOAT).precision>0&&i.getShaderPrecisionFormat(i.FRAGMENT_SHADER,i.HIGH_FLOAT).precision>0)return"highp";I="mediump"}return I==="mediump"&&i.getShaderPrecisionFormat(i.VERTEX_SHADER,i.MEDIUM_FLOAT).precision>0&&i.getShaderPrecisionFormat(i.FRAGMENT_SHADER,i.MEDIUM_FLOAT).precision>0?"mediump":"lowp"}let l=t.precision!==void 0?t.precision:"highp";const u=c(l);u!==l&&(je("WebGLRenderer:",l,"not supported, using",u,"instead."),l=u);const f=t.logarithmicDepthBuffer===!0,d=t.reversedDepthBuffer===!0&&e.has("EXT_clip_control");t.reversedDepthBuffer===!0&&d===!1&&je("WebGLRenderer: Unable to use reversed depth buffer due to missing EXT_clip_control extension. Fallback to default depth buffer.");const m=i.getParameter(i.MAX_TEXTURE_IMAGE_UNITS),x=i.getParameter(i.MAX_VERTEX_TEXTURE_IMAGE_UNITS),S=i.getParameter(i.MAX_TEXTURE_SIZE),g=i.getParameter(i.MAX_CUBE_MAP_TEXTURE_SIZE),h=i.getParameter(i.MAX_VERTEX_ATTRIBS),R=i.getParameter(i.MAX_VERTEX_UNIFORM_VECTORS),C=i.getParameter(i.MAX_VARYING_VECTORS),y=i.getParameter(i.MAX_FRAGMENT_UNIFORM_VECTORS),w=i.getParameter(i.MAX_SAMPLES),T=i.getParameter(i.SAMPLES);return{isWebGL2:!0,getMaxAnisotropy:s,getMaxPrecision:c,textureFormatReadable:a,textureTypeReadable:o,precision:l,logarithmicDepthBuffer:f,reversedDepthBuffer:d,maxTextures:m,maxVertexTextures:x,maxTextureSize:S,maxCubemapSize:g,maxAttributes:h,maxVertexUniforms:R,maxVaryings:C,maxFragmentUniforms:y,maxSamples:w,samples:T}}function _p(i){const e=this;let t=null,n=0,r=!1,s=!1;const a=new Ii,o=new nt,c={value:null,needsUpdate:!1};this.uniform=c,this.numPlanes=0,this.numIntersection=0,this.init=function(f,d){const m=f.length!==0||d||n!==0||r;return r=d,n=f.length,m},this.beginShadows=function(){s=!0,u(null)},this.endShadows=function(){s=!1},this.setGlobalState=function(f,d){t=u(f,d,0)},this.setState=function(f,d,m){const x=f.clippingPlanes,S=f.clipIntersection,g=f.clipShadows,h=i.get(f);if(!r||x===null||x.length===0||s&&!g)s?u(null):l();else{const R=s?0:n,C=R*4;let y=h.clippingState||null;c.value=y,y=u(x,d,C,m);for(let w=0;w!==C;++w)y[w]=t[w];h.clippingState=y,this.numIntersection=S?this.numPlanes:0,this.numPlanes+=R}};function l(){c.value!==t&&(c.value=t,c.needsUpdate=n>0),e.numPlanes=n,e.numIntersection=0}function u(f,d,m,x){const S=f!==null?f.length:0;let g=null;if(S!==0){if(g=c.value,x!==!0||g===null){const h=m+S*4,R=d.matrixWorldInverse;o.getNormalMatrix(R),(g===null||g.length<h)&&(g=new Float32Array(h));for(let C=0,y=m;C!==S;++C,y+=4)a.copy(f[C]).applyMatrix4(R,o),a.normal.toArray(g,y),g[y+3]=a.constant}c.value=g,c.needsUpdate=!0}return e.numPlanes=S,e.numIntersection=0,g}}const Mi=4,pl=[.125,.215,.35,.446,.526,.582],Ui=20,xp=256,Rr=new Ec,ml=new Tt;let _a=null,xa=0,va=0,Ma=!1;const vp=new K;class gl{constructor(e){this._renderer=e,this._pingPongRenderTarget=null,this._lodMax=0,this._cubeSize=0,this._sizeLods=[],this._sigmas=[],this._lodMeshes=[],this._backgroundBox=null,this._cubemapMaterial=null,this._equirectMaterial=null,this._blurMaterial=null,this._ggxMaterial=null}fromScene(e,t=0,n=.1,r=100,s={}){const{size:a=256,position:o=vp}=s;_a=this._renderer.getRenderTarget(),xa=this._renderer.getActiveCubeFace(),va=this._renderer.getActiveMipmapLevel(),Ma=this._renderer.xr.enabled,this._renderer.xr.enabled=!1,this._setSize(a);const c=this._allocateTargets();return c.depthBuffer=!0,this._sceneToCubeUV(e,n,r,c,o),t>0&&this._blur(c,0,0,t),this._applyPMREM(c),this._cleanup(c),c}fromEquirectangular(e,t=null){return this._fromTexture(e,t)}fromCubemap(e,t=null){return this._fromTexture(e,t)}compileCubemapShader(){this._cubemapMaterial===null&&(this._cubemapMaterial=vl(),this._compileMaterial(this._cubemapMaterial))}compileEquirectangularShader(){this._equirectMaterial===null&&(this._equirectMaterial=xl(),this._compileMaterial(this._equirectMaterial))}dispose(){this._dispose(),this._cubemapMaterial!==null&&this._cubemapMaterial.dispose(),this._equirectMaterial!==null&&this._equirectMaterial.dispose(),this._backgroundBox!==null&&(this._backgroundBox.geometry.dispose(),this._backgroundBox.material.dispose())}_setSize(e){this._lodMax=Math.floor(Math.log2(e)),this._cubeSize=Math.pow(2,this._lodMax)}_dispose(){this._blurMaterial!==null&&this._blurMaterial.dispose(),this._ggxMaterial!==null&&this._ggxMaterial.dispose(),this._pingPongRenderTarget!==null&&this._pingPongRenderTarget.dispose();for(let e=0;e<this._lodMeshes.length;e++)this._lodMeshes[e].geometry.dispose()}_cleanup(e){this._renderer.setRenderTarget(_a,xa,va),this._renderer.xr.enabled=Ma,e.scissorTest=!1,cr(e,0,0,e.width,e.height)}_fromTexture(e,t){e.mapping===Oi||e.mapping===mr?this._setSize(e.image.length===0?16:e.image[0].width||e.image[0].image.width):this._setSize(e.image.width/4),_a=this._renderer.getRenderTarget(),xa=this._renderer.getActiveCubeFace(),va=this._renderer.getActiveMipmapLevel(),Ma=this._renderer.xr.enabled,this._renderer.xr.enabled=!1;const n=t||this._allocateTargets();return this._textureToCubeUV(e,n),this._applyPMREM(n),this._cleanup(n),n}_allocateTargets(){const e=3*Math.max(this._cubeSize,112),t=4*this._cubeSize,n={magFilter:Qt,minFilter:Qt,generateMipmaps:!1,type:li,format:Cn,colorSpace:Or,depthBuffer:!1},r=_l(e,t,n);if(this._pingPongRenderTarget===null||this._pingPongRenderTarget.width!==e||this._pingPongRenderTarget.height!==t){this._pingPongRenderTarget!==null&&this._dispose(),this._pingPongRenderTarget=_l(e,t,n);const{_lodMax:s}=this;({lodMeshes:this._lodMeshes,sizeLods:this._sizeLods,sigmas:this._sigmas}=Mp(s)),this._blurMaterial=yp(s,e,t),this._ggxMaterial=Sp(s,e,t)}return r}_compileMaterial(e){const t=new qn(new Kn,e);this._renderer.compile(t,Rr)}_sceneToCubeUV(e,t,n,r,s){const c=new wn(90,1,t,n),l=[1,-1,1,1,1,1],u=[1,1,1,-1,-1,-1],f=this._renderer,d=f.autoClear,m=f.toneMapping;f.getClearColor(ml),f.toneMapping=Pn,f.autoClear=!1,f.state.buffers.depth.getReversed()&&(f.setRenderTarget(r),f.clearDepth(),f.setRenderTarget(null)),this._backgroundBox===null&&(this._backgroundBox=new qn(new zr,new xc({name:"PMREM.Background",side:cn,depthWrite:!1,depthTest:!1})));const S=this._backgroundBox,g=S.material;let h=!1;const R=e.background;R?R.isColor&&(g.color.copy(R),e.background=null,h=!0):(g.color.copy(ml),h=!0);for(let C=0;C<6;C++){const y=C%3;y===0?(c.up.set(0,l[C],0),c.position.set(s.x,s.y,s.z),c.lookAt(s.x+u[C],s.y,s.z)):y===1?(c.up.set(0,0,l[C]),c.position.set(s.x,s.y,s.z),c.lookAt(s.x,s.y+u[C],s.z)):(c.up.set(0,l[C],0),c.position.set(s.x,s.y,s.z),c.lookAt(s.x,s.y,s.z+u[C]));const w=this._cubeSize;cr(r,y*w,C>2?w:0,w,w),f.setRenderTarget(r),h&&f.render(S,c),f.render(e,c)}f.toneMapping=m,f.autoClear=d,e.background=R}_textureToCubeUV(e,t){const n=this._renderer,r=e.mapping===Oi||e.mapping===mr;r?(this._cubemapMaterial===null&&(this._cubemapMaterial=vl()),this._cubemapMaterial.uniforms.flipEnvMap.value=e.isRenderTargetTexture===!1?-1:1):this._equirectMaterial===null&&(this._equirectMaterial=xl());const s=r?this._cubemapMaterial:this._equirectMaterial,a=this._lodMeshes[0];a.material=s;const o=s.uniforms;o.envMap.value=e;const c=this._cubeSize;cr(t,0,0,3*c,2*c),n.setRenderTarget(t),n.render(a,Rr)}_applyPMREM(e){const t=this._renderer,n=t.autoClear;t.autoClear=!1;const r=this._lodMeshes.length;for(let s=1;s<r;s++)this._applyGGXFilter(e,s-1,s);t.autoClear=n}_applyGGXFilter(e,t,n){const r=this._renderer,s=this._pingPongRenderTarget,a=this._ggxMaterial,o=this._lodMeshes[n];o.material=a;const c=a.uniforms,l=n/(this._lodMeshes.length-1),u=t/(this._lodMeshes.length-1),f=Math.sqrt(l*l-u*u),d=0+l*1.25,m=f*d,{_lodMax:x}=this,S=this._sizeLods[n],g=3*S*(n>x-Mi?n-x+Mi:0),h=4*(this._cubeSize-S);c.envMap.value=e.texture,c.roughness.value=m,c.mipInt.value=x-t,cr(s,g,h,3*S,2*S),r.setRenderTarget(s),r.render(o,Rr),c.envMap.value=s.texture,c.roughness.value=0,c.mipInt.value=x-n,cr(e,g,h,3*S,2*S),r.setRenderTarget(e),r.render(o,Rr)}_blur(e,t,n,r,s){const a=this._pingPongRenderTarget;this._halfBlur(e,a,t,n,r,"latitudinal",s),this._halfBlur(a,e,n,n,r,"longitudinal",s)}_halfBlur(e,t,n,r,s,a,o){const c=this._renderer,l=this._blurMaterial;a!=="latitudinal"&&a!=="longitudinal"&&_t("blur direction must be either latitudinal or longitudinal!");const u=3,f=this._lodMeshes[r];f.material=l;const d=l.uniforms,m=this._sizeLods[n]-1,x=isFinite(s)?Math.PI/(2*m):2*Math.PI/(2*Ui-1),S=s/x,g=isFinite(s)?1+Math.floor(u*S):Ui;g>Ui&&je(`sigmaRadians, ${s}, is too large and will clip, as it requested ${g} samples when the maximum is set to ${Ui}`);const h=[];let R=0;for(let I=0;I<Ui;++I){const _=I/S,A=Math.exp(-_*_/2);h.push(A),I===0?R+=A:I<g&&(R+=2*A)}for(let I=0;I<h.length;I++)h[I]=h[I]/R;d.envMap.value=e.texture,d.samples.value=g,d.weights.value=h,d.latitudinal.value=a==="latitudinal",o&&(d.poleAxis.value=o);const{_lodMax:C}=this;d.dTheta.value=x,d.mipInt.value=C-n;const y=this._sizeLods[r],w=3*y*(r>C-Mi?r-C+Mi:0),T=4*(this._cubeSize-y);cr(t,w,T,3*y,2*y),c.setRenderTarget(t),c.render(f,Rr)}}function Mp(i){const e=[],t=[],n=[];let r=i;const s=i-Mi+1+pl.length;for(let a=0;a<s;a++){const o=Math.pow(2,r);e.push(o);let c=1/o;a>i-Mi?c=pl[a-i+Mi-1]:a===0&&(c=0),t.push(c);const l=1/(o-2),u=-l,f=1+l,d=[u,u,f,u,f,f,u,u,f,f,u,f],m=6,x=6,S=3,g=2,h=1,R=new Float32Array(S*x*m),C=new Float32Array(g*x*m),y=new Float32Array(h*x*m);for(let T=0;T<m;T++){const I=T%3*2/3-1,_=T>2?0:-1,A=[I,_,0,I+2/3,_,0,I+2/3,_+1,0,I,_,0,I+2/3,_+1,0,I,_+1,0];R.set(A,S*x*T),C.set(d,g*x*T);const k=[T,T,T,T,T,T];y.set(k,h*x*T)}const w=new Kn;w.setAttribute("position",new Ln(R,S)),w.setAttribute("uv",new Ln(C,g)),w.setAttribute("faceIndex",new Ln(y,h)),n.push(new qn(w,null)),r>Mi&&r--}return{lodMeshes:n,sizeLods:e,sigmas:t}}function _l(i,e,t){const n=new Wn(i,e,t);return n.texture.mapping=Us,n.texture.name="PMREM.cubeUv",n.scissorTest=!0,n}function cr(i,e,t,n,r){i.viewport.set(e,t,n,r),i.scissor.set(e,t,n,r)}function Sp(i,e,t){return new Yn({name:"PMREMGGXConvolution",defines:{GGX_SAMPLES:xp,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${i}.0`},uniforms:{envMap:{value:null},roughness:{value:0},mipInt:{value:0}},vertexShader:Os(),fragmentShader:`

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
		`,blending:si,depthTest:!1,depthWrite:!1})}function yp(i,e,t){const n=new Float32Array(Ui),r=new K(0,1,0);return new Yn({name:"SphericalGaussianBlur",defines:{n:Ui,CUBEUV_TEXEL_WIDTH:1/e,CUBEUV_TEXEL_HEIGHT:1/t,CUBEUV_MAX_MIP:`${i}.0`},uniforms:{envMap:{value:null},samples:{value:1},weights:{value:n},latitudinal:{value:!1},dTheta:{value:0},mipInt:{value:0},poleAxis:{value:r}},vertexShader:Os(),fragmentShader:`

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
		`,blending:si,depthTest:!1,depthWrite:!1})}function xl(){return new Yn({name:"EquirectangularToCubeUV",uniforms:{envMap:{value:null}},vertexShader:Os(),fragmentShader:`

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
		`,blending:si,depthTest:!1,depthWrite:!1})}function vl(){return new Yn({name:"CubemapToCubeUV",uniforms:{envMap:{value:null},flipEnvMap:{value:-1}},vertexShader:Os(),fragmentShader:`

			precision mediump float;
			precision mediump int;

			uniform float flipEnvMap;

			varying vec3 vOutputDirection;

			uniform samplerCube envMap;

			void main() {

				gl_FragColor = textureCube( envMap, vec3( flipEnvMap * vOutputDirection.x, vOutputDirection.yz ) );

			}
		`,blending:si,depthTest:!1,depthWrite:!1})}function Os(){return`

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
	`}class wc extends Wn{constructor(e=1,t={}){super(e,e,t),this.isWebGLCubeRenderTarget=!0;const n={width:e,height:e,depth:1},r=[n,n,n,n,n,n];this.texture=new Mc(r),this._setTextureOptions(t),this.texture.isRenderTargetTexture=!0}fromEquirectangularTexture(e,t){this.texture.type=t.type,this.texture.colorSpace=t.colorSpace,this.texture.generateMipmaps=t.generateMipmaps,this.texture.minFilter=t.minFilter,this.texture.magFilter=t.magFilter;const n={uniforms:{tEquirect:{value:null}},vertexShader:`

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
			`},r=new zr(5,5,5),s=new Yn({name:"CubemapFromEquirect",uniforms:_r(n.uniforms),vertexShader:n.vertexShader,fragmentShader:n.fragmentShader,side:cn,blending:si});s.uniforms.tEquirect.value=t;const a=new qn(r,s),o=t.minFilter;return t.minFilter===Ni&&(t.minFilter=Qt),new wu(1,10,this).update(e,a),t.minFilter=o,a.geometry.dispose(),a.material.dispose(),this}clear(e,t=!0,n=!0,r=!0){const s=e.getRenderTarget();for(let a=0;a<6;a++)e.setRenderTarget(this,a),e.clear(t,n,r);e.setRenderTarget(s)}}function bp(i){let e=new WeakMap,t=new WeakMap,n=null;function r(d,m=!1){return d==null?null:m?a(d):s(d)}function s(d){if(d&&d.isTexture){const m=d.mapping;if(m===Ws||m===Xs)if(e.has(d)){const x=e.get(d).texture;return o(x,d.mapping)}else{const x=d.image;if(x&&x.height>0){const S=new wc(x.height);return S.fromEquirectangularTexture(i,d),e.set(d,S),d.addEventListener("dispose",l),o(S.texture,d.mapping)}else return null}}return d}function a(d){if(d&&d.isTexture){const m=d.mapping,x=m===Ws||m===Xs,S=m===Oi||m===mr;if(x||S){let g=t.get(d);const h=g!==void 0?g.texture.pmremVersion:0;if(d.isRenderTargetTexture&&d.pmremVersion!==h)return n===null&&(n=new gl(i)),g=x?n.fromEquirectangular(d,g):n.fromCubemap(d,g),g.texture.pmremVersion=d.pmremVersion,t.set(d,g),g.texture;if(g!==void 0)return g.texture;{const R=d.image;return x&&R&&R.height>0||S&&R&&c(R)?(n===null&&(n=new gl(i)),g=x?n.fromEquirectangular(d):n.fromCubemap(d),g.texture.pmremVersion=d.pmremVersion,t.set(d,g),d.addEventListener("dispose",u),g.texture):null}}}return d}function o(d,m){return m===Ws?d.mapping=Oi:m===Xs&&(d.mapping=mr),d}function c(d){let m=0;const x=6;for(let S=0;S<x;S++)d[S]!==void 0&&m++;return m===x}function l(d){const m=d.target;m.removeEventListener("dispose",l);const x=e.get(m);x!==void 0&&(e.delete(m),x.dispose())}function u(d){const m=d.target;m.removeEventListener("dispose",u);const x=t.get(m);x!==void 0&&(t.delete(m),x.dispose())}function f(){e=new WeakMap,t=new WeakMap,n!==null&&(n.dispose(),n=null)}return{get:r,dispose:f}}function Ep(i){const e={};function t(n){if(e[n]!==void 0)return e[n];const r=i.getExtension(n);return e[n]=r,r}return{has:function(n){return t(n)!==null},init:function(){t("EXT_color_buffer_float"),t("WEBGL_clip_cull_distance"),t("OES_texture_float_linear"),t("EXT_color_buffer_half_float"),t("WEBGL_multisampled_render_to_texture"),t("WEBGL_render_shared_exponent")},get:function(n){const r=t(n);return r===null&&hr("WebGLRenderer: "+n+" extension not supported."),r}}}function Tp(i,e,t,n){const r={},s=new WeakMap;function a(f){const d=f.target;d.index!==null&&e.remove(d.index);for(const x in d.attributes)e.remove(d.attributes[x]);d.removeEventListener("dispose",a),delete r[d.id];const m=s.get(d);m&&(e.remove(m),s.delete(d)),n.releaseStatesOfGeometry(d),d.isInstancedBufferGeometry===!0&&delete d._maxInstanceCount,t.memory.geometries--}function o(f,d){return r[d.id]===!0||(d.addEventListener("dispose",a),r[d.id]=!0,t.memory.geometries++),d}function c(f){const d=f.attributes;for(const m in d)e.update(d[m],i.ARRAY_BUFFER)}function l(f){const d=[],m=f.index,x=f.attributes.position;let S=0;if(x===void 0)return;if(m!==null){const R=m.array;S=m.version;for(let C=0,y=R.length;C<y;C+=3){const w=R[C+0],T=R[C+1],I=R[C+2];d.push(w,T,T,I,I,w)}}else{const R=x.array;S=x.version;for(let C=0,y=R.length/3-1;C<y;C+=3){const w=C+0,T=C+1,I=C+2;d.push(w,T,T,I,I,w)}}const g=new(x.count>=65535?_c:gc)(d,1);g.version=S;const h=s.get(f);h&&e.remove(h),s.set(f,g)}function u(f){const d=s.get(f);if(d){const m=f.index;m!==null&&d.version<m.version&&l(f)}else l(f);return s.get(f)}return{get:o,update:c,getWireframeAttribute:u}}function Ap(i,e,t){let n;function r(f){n=f}let s,a;function o(f){s=f.type,a=f.bytesPerElement}function c(f,d){i.drawElements(n,d,s,f*a),t.update(d,n,1)}function l(f,d,m){m!==0&&(i.drawElementsInstanced(n,d,s,f*a,m),t.update(d,n,m))}function u(f,d,m){if(m===0)return;e.get("WEBGL_multi_draw").multiDrawElementsWEBGL(n,d,0,s,f,0,m);let S=0;for(let g=0;g<m;g++)S+=d[g];t.update(S,n,1)}this.setMode=r,this.setIndex=o,this.render=c,this.renderInstances=l,this.renderMultiDraw=u}function wp(i){const e={geometries:0,textures:0},t={frame:0,calls:0,triangles:0,points:0,lines:0};function n(s,a,o){switch(t.calls++,a){case i.TRIANGLES:t.triangles+=o*(s/3);break;case i.LINES:t.lines+=o*(s/2);break;case i.LINE_STRIP:t.lines+=o*(s-1);break;case i.LINE_LOOP:t.lines+=o*s;break;case i.POINTS:t.points+=o*s;break;default:_t("WebGLInfo: Unknown draw mode:",a);break}}function r(){t.calls=0,t.triangles=0,t.points=0,t.lines=0}return{memory:e,render:t,programs:null,autoReset:!0,reset:r,update:n}}function Rp(i,e,t){const n=new WeakMap,r=new Ot;function s(a,o,c){const l=a.morphTargetInfluences,u=o.morphAttributes.position||o.morphAttributes.normal||o.morphAttributes.color,f=u!==void 0?u.length:0;let d=n.get(o);if(d===void 0||d.count!==f){let A=function(){I.dispose(),n.delete(o),o.removeEventListener("dispose",A)};d!==void 0&&d.texture.dispose();const m=o.morphAttributes.position!==void 0,x=o.morphAttributes.normal!==void 0,S=o.morphAttributes.color!==void 0,g=o.morphAttributes.position||[],h=o.morphAttributes.normal||[],R=o.morphAttributes.color||[];let C=0;m===!0&&(C=1),x===!0&&(C=2),S===!0&&(C=3);let y=o.attributes.position.count*C,w=1;y>e.maxTextureSize&&(w=Math.ceil(y/e.maxTextureSize),y=e.maxTextureSize);const T=new Float32Array(y*w*4*f),I=new fc(T,y,w,f);I.type=Vn,I.needsUpdate=!0;const _=C*4;for(let k=0;k<f;k++){const F=g[k],b=h[k],P=R[k],W=y*w*4*k;for(let N=0;N<F.count;N++){const D=N*_;m===!0&&(r.fromBufferAttribute(F,N),T[W+D+0]=r.x,T[W+D+1]=r.y,T[W+D+2]=r.z,T[W+D+3]=0),x===!0&&(r.fromBufferAttribute(b,N),T[W+D+4]=r.x,T[W+D+5]=r.y,T[W+D+6]=r.z,T[W+D+7]=0),S===!0&&(r.fromBufferAttribute(P,N),T[W+D+8]=r.x,T[W+D+9]=r.y,T[W+D+10]=r.z,T[W+D+11]=P.itemSize===4?r.w:1)}}d={count:f,texture:I,size:new Mt(y,w)},n.set(o,d),o.addEventListener("dispose",A)}if(a.isInstancedMesh===!0&&a.morphTexture!==null)c.getUniforms().setValue(i,"morphTexture",a.morphTexture,t);else{let m=0;for(let S=0;S<l.length;S++)m+=l[S];const x=o.morphTargetsRelative?1:1-m;c.getUniforms().setValue(i,"morphTargetBaseInfluence",x),c.getUniforms().setValue(i,"morphTargetInfluences",l)}c.getUniforms().setValue(i,"morphTargetsTexture",d.texture,t),c.getUniforms().setValue(i,"morphTargetsTextureSize",d.size)}return{update:s}}function Cp(i,e,t,n,r){let s=new WeakMap;function a(l){const u=r.render.frame,f=l.geometry,d=e.get(l,f);if(s.get(d)!==u&&(e.update(d),s.set(d,u)),l.isInstancedMesh&&(l.hasEventListener("dispose",c)===!1&&l.addEventListener("dispose",c),s.get(l)!==u&&(t.update(l.instanceMatrix,i.ARRAY_BUFFER),l.instanceColor!==null&&t.update(l.instanceColor,i.ARRAY_BUFFER),s.set(l,u))),l.isSkinnedMesh){const m=l.skeleton;s.get(m)!==u&&(m.update(),s.set(m,u))}return d}function o(){s=new WeakMap}function c(l){const u=l.target;u.removeEventListener("dispose",c),n.releaseStatesOfObject(u),t.remove(u.instanceMatrix),u.instanceColor!==null&&t.remove(u.instanceColor)}return{update:a,dispose:o}}const Pp={[Jl]:"LINEAR_TONE_MAPPING",[Ql]:"REINHARD_TONE_MAPPING",[jl]:"CINEON_TONE_MAPPING",[ec]:"ACES_FILMIC_TONE_MAPPING",[nc]:"AGX_TONE_MAPPING",[ic]:"NEUTRAL_TONE_MAPPING",[tc]:"CUSTOM_TONE_MAPPING"};function Lp(i,e,t,n,r,s){const a=new Wn(e,t,{type:i,depthBuffer:r,stencilBuffer:s,samples:n?4:0,depthTexture:r?new gr(e,t):void 0}),o=new Wn(e,t,{type:li,depthBuffer:!1,stencilBuffer:!1}),c=new Kn;c.setAttribute("position",new oi([-1,3,0,-1,-1,0,3,-1,0],3)),c.setAttribute("uv",new oi([0,2,0,0,2,0],2));const l=new bc({uniforms:{tDiffuse:{value:null}},vertexShader:`
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
			}`,depthTest:!1,depthWrite:!1}),u=new qn(c,l),f=new Ec(-1,1,1,-1,0,1);let d=null,m=null,x=!1,S,g=null,h=[],R=!1;this.setSize=function(C,y){a.setSize(C,y),o.setSize(C,y);for(let w=0;w<h.length;w++){const T=h[w];T.setSize&&T.setSize(C,y)}},this.setEffects=function(C){h=C,R=h.length>0&&h[0].isRenderPass===!0;const y=a.width,w=a.height;for(let T=0;T<h.length;T++){const I=h[T];I.setSize&&I.setSize(y,w)}},this.begin=function(C,y){if(x||C.toneMapping===Pn&&h.length===0)return!1;if(g=y,y!==null){const w=y.width,T=y.height;(a.width!==w||a.height!==T)&&this.setSize(w,T)}return R===!1&&C.setRenderTarget(a),S=C.toneMapping,C.toneMapping=Pn,!0},this.hasRenderPass=function(){return R},this.end=function(C,y){C.toneMapping=S,x=!0;let w=a,T=o;for(let I=0;I<h.length;I++){const _=h[I];if(_.enabled!==!1&&(_.render(C,T,w,y),_.needsSwap!==!1)){const A=w;w=T,T=A}}if(d!==C.outputColorSpace||m!==C.toneMapping){d=C.outputColorSpace,m=C.toneMapping,l.defines={},mt.getTransfer(d)===Ct&&(l.defines.SRGB_TRANSFER="");const I=Pp[m];I&&(l.defines[I]=""),l.needsUpdate=!0}l.uniforms.tDiffuse.value=w.texture,C.setRenderTarget(g),C.render(u,f),g=null,x=!1},this.isCompositing=function(){return x},this.dispose=function(){a.depthTexture&&a.depthTexture.dispose(),a.dispose(),o.dispose(),c.dispose(),l.dispose()}}const Rc=new tn,_o=new gr(1,1),Cc=new fc,Pc=new jd,Lc=new Mc,Ml=[],Sl=[],yl=new Float32Array(16),bl=new Float32Array(9),El=new Float32Array(4);function vr(i,e,t){const n=i[0];if(n<=0||n>0)return i;const r=e*t;let s=Ml[r];if(s===void 0&&(s=new Float32Array(r),Ml[r]=s),e!==0){n.toArray(s,0);for(let a=1,o=0;a!==e;++a)o+=t,i[a].toArray(s,o)}return s}function Xt(i,e){if(i.length!==e.length)return!1;for(let t=0,n=i.length;t<n;t++)if(i[t]!==e[t])return!1;return!0}function qt(i,e){for(let t=0,n=e.length;t<n;t++)i[t]=e[t]}function Bs(i,e){let t=Sl[e];t===void 0&&(t=new Int32Array(e),Sl[e]=t);for(let n=0;n!==e;++n)t[n]=i.allocateTextureUnit();return t}function Dp(i,e){const t=this.cache;t[0]!==e&&(i.uniform1f(this.addr,e),t[0]=e)}function Ip(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(i.uniform2f(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Xt(t,e))return;i.uniform2fv(this.addr,e),qt(t,e)}}function Up(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(i.uniform3f(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else if(e.r!==void 0)(t[0]!==e.r||t[1]!==e.g||t[2]!==e.b)&&(i.uniform3f(this.addr,e.r,e.g,e.b),t[0]=e.r,t[1]=e.g,t[2]=e.b);else{if(Xt(t,e))return;i.uniform3fv(this.addr,e),qt(t,e)}}function Np(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(i.uniform4f(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Xt(t,e))return;i.uniform4fv(this.addr,e),qt(t,e)}}function Fp(i,e){const t=this.cache,n=e.elements;if(n===void 0){if(Xt(t,e))return;i.uniformMatrix2fv(this.addr,!1,e),qt(t,e)}else{if(Xt(t,n))return;El.set(n),i.uniformMatrix2fv(this.addr,!1,El),qt(t,n)}}function Op(i,e){const t=this.cache,n=e.elements;if(n===void 0){if(Xt(t,e))return;i.uniformMatrix3fv(this.addr,!1,e),qt(t,e)}else{if(Xt(t,n))return;bl.set(n),i.uniformMatrix3fv(this.addr,!1,bl),qt(t,n)}}function Bp(i,e){const t=this.cache,n=e.elements;if(n===void 0){if(Xt(t,e))return;i.uniformMatrix4fv(this.addr,!1,e),qt(t,e)}else{if(Xt(t,n))return;yl.set(n),i.uniformMatrix4fv(this.addr,!1,yl),qt(t,n)}}function zp(i,e){const t=this.cache;t[0]!==e&&(i.uniform1i(this.addr,e),t[0]=e)}function kp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(i.uniform2i(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Xt(t,e))return;i.uniform2iv(this.addr,e),qt(t,e)}}function Gp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(i.uniform3i(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(Xt(t,e))return;i.uniform3iv(this.addr,e),qt(t,e)}}function Vp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(i.uniform4i(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Xt(t,e))return;i.uniform4iv(this.addr,e),qt(t,e)}}function Hp(i,e){const t=this.cache;t[0]!==e&&(i.uniform1ui(this.addr,e),t[0]=e)}function Wp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y)&&(i.uniform2ui(this.addr,e.x,e.y),t[0]=e.x,t[1]=e.y);else{if(Xt(t,e))return;i.uniform2uiv(this.addr,e),qt(t,e)}}function Xp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z)&&(i.uniform3ui(this.addr,e.x,e.y,e.z),t[0]=e.x,t[1]=e.y,t[2]=e.z);else{if(Xt(t,e))return;i.uniform3uiv(this.addr,e),qt(t,e)}}function qp(i,e){const t=this.cache;if(e.x!==void 0)(t[0]!==e.x||t[1]!==e.y||t[2]!==e.z||t[3]!==e.w)&&(i.uniform4ui(this.addr,e.x,e.y,e.z,e.w),t[0]=e.x,t[1]=e.y,t[2]=e.z,t[3]=e.w);else{if(Xt(t,e))return;i.uniform4uiv(this.addr,e),qt(t,e)}}function Yp(i,e,t){const n=this.cache,r=t.allocateTextureUnit();n[0]!==r&&(i.uniform1i(this.addr,r),n[0]=r);let s;this.type===i.SAMPLER_2D_SHADOW?(_o.compareFunction=t.isReversedDepthBuffer()?Co:Ro,s=_o):s=Rc,t.setTexture2D(e||s,r)}function Kp(i,e,t){const n=this.cache,r=t.allocateTextureUnit();n[0]!==r&&(i.uniform1i(this.addr,r),n[0]=r),t.setTexture3D(e||Pc,r)}function Zp(i,e,t){const n=this.cache,r=t.allocateTextureUnit();n[0]!==r&&(i.uniform1i(this.addr,r),n[0]=r),t.setTextureCube(e||Lc,r)}function $p(i,e,t){const n=this.cache,r=t.allocateTextureUnit();n[0]!==r&&(i.uniform1i(this.addr,r),n[0]=r),t.setTexture2DArray(e||Cc,r)}function Jp(i){switch(i){case 5126:return Dp;case 35664:return Ip;case 35665:return Up;case 35666:return Np;case 35674:return Fp;case 35675:return Op;case 35676:return Bp;case 5124:case 35670:return zp;case 35667:case 35671:return kp;case 35668:case 35672:return Gp;case 35669:case 35673:return Vp;case 5125:return Hp;case 36294:return Wp;case 36295:return Xp;case 36296:return qp;case 35678:case 36198:case 36298:case 36306:case 35682:return Yp;case 35679:case 36299:case 36307:return Kp;case 35680:case 36300:case 36308:case 36293:return Zp;case 36289:case 36303:case 36311:case 36292:return $p}}function Qp(i,e){i.uniform1fv(this.addr,e)}function jp(i,e){const t=vr(e,this.size,2);i.uniform2fv(this.addr,t)}function em(i,e){const t=vr(e,this.size,3);i.uniform3fv(this.addr,t)}function tm(i,e){const t=vr(e,this.size,4);i.uniform4fv(this.addr,t)}function nm(i,e){const t=vr(e,this.size,4);i.uniformMatrix2fv(this.addr,!1,t)}function im(i,e){const t=vr(e,this.size,9);i.uniformMatrix3fv(this.addr,!1,t)}function rm(i,e){const t=vr(e,this.size,16);i.uniformMatrix4fv(this.addr,!1,t)}function sm(i,e){i.uniform1iv(this.addr,e)}function am(i,e){i.uniform2iv(this.addr,e)}function om(i,e){i.uniform3iv(this.addr,e)}function lm(i,e){i.uniform4iv(this.addr,e)}function cm(i,e){i.uniform1uiv(this.addr,e)}function dm(i,e){i.uniform2uiv(this.addr,e)}function um(i,e){i.uniform3uiv(this.addr,e)}function hm(i,e){i.uniform4uiv(this.addr,e)}function fm(i,e,t){const n=this.cache,r=e.length,s=Bs(t,r);Xt(n,s)||(i.uniform1iv(this.addr,s),qt(n,s));let a;this.type===i.SAMPLER_2D_SHADOW?a=_o:a=Rc;for(let o=0;o!==r;++o)t.setTexture2D(e[o]||a,s[o])}function pm(i,e,t){const n=this.cache,r=e.length,s=Bs(t,r);Xt(n,s)||(i.uniform1iv(this.addr,s),qt(n,s));for(let a=0;a!==r;++a)t.setTexture3D(e[a]||Pc,s[a])}function mm(i,e,t){const n=this.cache,r=e.length,s=Bs(t,r);Xt(n,s)||(i.uniform1iv(this.addr,s),qt(n,s));for(let a=0;a!==r;++a)t.setTextureCube(e[a]||Lc,s[a])}function gm(i,e,t){const n=this.cache,r=e.length,s=Bs(t,r);Xt(n,s)||(i.uniform1iv(this.addr,s),qt(n,s));for(let a=0;a!==r;++a)t.setTexture2DArray(e[a]||Cc,s[a])}function _m(i){switch(i){case 5126:return Qp;case 35664:return jp;case 35665:return em;case 35666:return tm;case 35674:return nm;case 35675:return im;case 35676:return rm;case 5124:case 35670:return sm;case 35667:case 35671:return am;case 35668:case 35672:return om;case 35669:case 35673:return lm;case 5125:return cm;case 36294:return dm;case 36295:return um;case 36296:return hm;case 35678:case 36198:case 36298:case 36306:case 35682:return fm;case 35679:case 36299:case 36307:return pm;case 35680:case 36300:case 36308:case 36293:return mm;case 36289:case 36303:case 36311:case 36292:return gm}}class xm{constructor(e,t,n){this.id=e,this.addr=n,this.cache=[],this.type=t.type,this.setValue=Jp(t.type)}}class vm{constructor(e,t,n){this.id=e,this.addr=n,this.cache=[],this.type=t.type,this.size=t.size,this.setValue=_m(t.type)}}class Mm{constructor(e){this.id=e,this.seq=[],this.map={}}setValue(e,t,n){const r=this.seq;for(let s=0,a=r.length;s!==a;++s){const o=r[s];o.setValue(e,t[o.id],n)}}}const Sa=/(\w+)(\])?(\[|\.)?/g;function Tl(i,e){i.seq.push(e),i.map[e.id]=e}function Sm(i,e,t){const n=i.name,r=n.length;for(Sa.lastIndex=0;;){const s=Sa.exec(n),a=Sa.lastIndex;let o=s[1];const c=s[2]==="]",l=s[3];if(c&&(o=o|0),l===void 0||l==="["&&a+2===r){Tl(t,l===void 0?new xm(o,i,e):new vm(o,i,e));break}else{let f=t.map[o];f===void 0&&(f=new Mm(o),Tl(t,f)),t=f}}}class Es{constructor(e,t){this.seq=[],this.map={};const n=e.getProgramParameter(t,e.ACTIVE_UNIFORMS);for(let a=0;a<n;++a){const o=e.getActiveUniform(t,a),c=e.getUniformLocation(t,o.name);Sm(o,c,this)}const r=[],s=[];for(const a of this.seq)a.type===e.SAMPLER_2D_SHADOW||a.type===e.SAMPLER_CUBE_SHADOW||a.type===e.SAMPLER_2D_ARRAY_SHADOW?r.push(a):s.push(a);r.length>0&&(this.seq=r.concat(s))}setValue(e,t,n,r){const s=this.map[t];s!==void 0&&s.setValue(e,n,r)}setOptional(e,t,n){const r=t[n];r!==void 0&&this.setValue(e,n,r)}static upload(e,t,n,r){for(let s=0,a=t.length;s!==a;++s){const o=t[s],c=n[o.id];c.needsUpdate!==!1&&o.setValue(e,c.value,r)}}static seqWithValue(e,t){const n=[];for(let r=0,s=e.length;r!==s;++r){const a=e[r];a.id in t&&n.push(a)}return n}}function Al(i,e,t){const n=i.createShader(e);return i.shaderSource(n,t),i.compileShader(n),n}const ym=37297;let bm=0;function Em(i,e){const t=i.split(`
`),n=[],r=Math.max(e-6,0),s=Math.min(e+6,t.length);for(let a=r;a<s;a++){const o=a+1;n.push(`${o===e?">":" "} ${o}: ${t[a]}`)}return n.join(`
`)}const wl=new nt;function Tm(i){mt._getMatrix(wl,mt.workingColorSpace,i);const e=`mat3( ${wl.elements.map(t=>t.toFixed(4))} )`;switch(mt.getTransfer(i)){case ws:return[e,"LinearTransferOETF"];case Ct:return[e,"sRGBTransferOETF"];default:return je("WebGLProgram: Unsupported color space: ",i),[e,"LinearTransferOETF"]}}function Rl(i,e,t){const n=i.getShaderParameter(e,i.COMPILE_STATUS),s=(i.getShaderInfoLog(e)||"").trim();if(n&&s==="")return"";const a=/ERROR: 0:(\d+)/.exec(s);if(a){const o=parseInt(a[1]);return t.toUpperCase()+`

`+s+`

`+Em(i.getShaderSource(e),o)}else return s}function Am(i,e){const t=Tm(e);return[`vec4 ${i}( vec4 value ) {`,`	return ${t[1]}( vec4( value.rgb * ${t[0]}, value.a ) );`,"}"].join(`
`)}const wm={[Jl]:"Linear",[Ql]:"Reinhard",[jl]:"Cineon",[ec]:"ACESFilmic",[nc]:"AgX",[ic]:"Neutral",[tc]:"Custom"};function Rm(i,e){const t=wm[e];return t===void 0?(je("WebGLProgram: Unsupported toneMapping:",e),"vec3 "+i+"( vec3 color ) { return LinearToneMapping( color ); }"):"vec3 "+i+"( vec3 color ) { return "+t+"ToneMapping( color ); }"}const fs=new K;function Cm(){mt.getLuminanceCoefficients(fs);const i=fs.x.toFixed(4),e=fs.y.toFixed(4),t=fs.z.toFixed(4);return["float luminance( const in vec3 rgb ) {",`	const vec3 weights = vec3( ${i}, ${e}, ${t} );`,"	return dot( weights, rgb );","}"].join(`
`)}function Pm(i){return[i.extensionClipCullDistance?"#extension GL_ANGLE_clip_cull_distance : require":"",i.extensionMultiDraw?"#extension GL_ANGLE_multi_draw : require":""].filter(Dr).join(`
`)}function Lm(i){const e=[];for(const t in i){const n=i[t];n!==!1&&e.push("#define "+t+" "+n)}return e.join(`
`)}function Dm(i,e){const t={},n=i.getProgramParameter(e,i.ACTIVE_ATTRIBUTES);for(let r=0;r<n;r++){const s=i.getActiveAttrib(e,r),a=s.name;let o=1;s.type===i.FLOAT_MAT2&&(o=2),s.type===i.FLOAT_MAT3&&(o=3),s.type===i.FLOAT_MAT4&&(o=4),t[a]={type:s.type,location:i.getAttribLocation(e,a),locationSize:o}}return t}function Dr(i){return i!==""}function Cl(i,e){const t=e.numSpotLightShadows+e.numSpotLightMaps-e.numSpotLightShadowsWithMaps;return i.replace(/NUM_DIR_LIGHTS/g,e.numDirLights).replace(/NUM_SPOT_LIGHTS/g,e.numSpotLights).replace(/NUM_SPOT_LIGHT_MAPS/g,e.numSpotLightMaps).replace(/NUM_SPOT_LIGHT_COORDS/g,t).replace(/NUM_RECT_AREA_LIGHTS/g,e.numRectAreaLights).replace(/NUM_POINT_LIGHTS/g,e.numPointLights).replace(/NUM_HEMI_LIGHTS/g,e.numHemiLights).replace(/NUM_DIR_LIGHT_SHADOWS/g,e.numDirLightShadows).replace(/NUM_SPOT_LIGHT_SHADOWS_WITH_MAPS/g,e.numSpotLightShadowsWithMaps).replace(/NUM_SPOT_LIGHT_SHADOWS/g,e.numSpotLightShadows).replace(/NUM_POINT_LIGHT_SHADOWS/g,e.numPointLightShadows)}function Pl(i,e){return i.replace(/NUM_CLIPPING_PLANES/g,e.numClippingPlanes).replace(/UNION_CLIPPING_PLANES/g,e.numClippingPlanes-e.numClipIntersection)}const Im=/^[ \t]*#include +<([\w\d./]+)>/gm;function xo(i){return i.replace(Im,Nm)}const Um=new Map;function Nm(i,e){let t=ct[e];if(t===void 0){const n=Um.get(e);if(n!==void 0)t=ct[n],je('WebGLRenderer: Shader chunk "%s" has been deprecated. Use "%s" instead.',e,n);else throw new Error("THREE.WebGLProgram: Can not resolve #include <"+e+">")}return xo(t)}const Fm=/#pragma unroll_loop_start\s+for\s*\(\s*int\s+i\s*=\s*(\d+)\s*;\s*i\s*<\s*(\d+)\s*;\s*i\s*\+\+\s*\)\s*{([\s\S]+?)}\s+#pragma unroll_loop_end/g;function Ll(i){return i.replace(Fm,Om)}function Om(i,e,t,n){let r="";for(let s=parseInt(e);s<parseInt(t);s++)r+=n.replace(/\[\s*i\s*\]/g,"[ "+s+" ]").replace(/UNROLLED_LOOP_INDEX/g,s);return r}function Dl(i){let e=`precision ${i.precision} float;
	precision ${i.precision} int;
	precision ${i.precision} sampler2D;
	precision ${i.precision} samplerCube;
	precision ${i.precision} sampler3D;
	precision ${i.precision} sampler2DArray;
	precision ${i.precision} sampler2DShadow;
	precision ${i.precision} samplerCubeShadow;
	precision ${i.precision} sampler2DArrayShadow;
	precision ${i.precision} isampler2D;
	precision ${i.precision} isampler3D;
	precision ${i.precision} isamplerCube;
	precision ${i.precision} isampler2DArray;
	precision ${i.precision} usampler2D;
	precision ${i.precision} usampler3D;
	precision ${i.precision} usamplerCube;
	precision ${i.precision} usampler2DArray;
	`;return i.precision==="highp"?e+=`
#define HIGH_PRECISION`:i.precision==="mediump"?e+=`
#define MEDIUM_PRECISION`:i.precision==="lowp"&&(e+=`
#define LOW_PRECISION`),e}const Bm={[vs]:"SHADOWMAP_TYPE_PCF",[Lr]:"SHADOWMAP_TYPE_VSM"};function zm(i){return Bm[i.shadowMapType]||"SHADOWMAP_TYPE_BASIC"}const km={[Oi]:"ENVMAP_TYPE_CUBE",[mr]:"ENVMAP_TYPE_CUBE",[Us]:"ENVMAP_TYPE_CUBE_UV"};function Gm(i){return i.envMap===!1?"ENVMAP_TYPE_CUBE":km[i.envMapMode]||"ENVMAP_TYPE_CUBE"}const Vm={[mr]:"ENVMAP_MODE_REFRACTION"};function Hm(i){return i.envMap===!1?"ENVMAP_MODE_REFLECTION":Vm[i.envMapMode]||"ENVMAP_MODE_REFLECTION"}const Wm={[$l]:"ENVMAP_BLENDING_MULTIPLY",[Ld]:"ENVMAP_BLENDING_MIX",[Dd]:"ENVMAP_BLENDING_ADD"};function Xm(i){return i.envMap===!1?"ENVMAP_BLENDING_NONE":Wm[i.combine]||"ENVMAP_BLENDING_NONE"}function qm(i){const e=i.envMapCubeUVHeight;if(e===null)return null;const t=Math.log2(e)-2,n=1/e;return{texelWidth:1/(3*Math.max(Math.pow(2,t),112)),texelHeight:n,maxMip:t}}function Ym(i,e,t,n){const r=i.getContext(),s=t.defines;let a=t.vertexShader,o=t.fragmentShader;const c=zm(t),l=Gm(t),u=Hm(t),f=Xm(t),d=qm(t),m=Pm(t),x=Lm(s),S=r.createProgram();let g,h,R=t.glslVersion?"#version "+t.glslVersion+`
`:"";t.isRawShaderMaterial?(g=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,x].filter(Dr).join(`
`),g.length>0&&(g+=`
`),h=["#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,x].filter(Dr).join(`
`),h.length>0&&(h+=`
`)):(g=[Dl(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,x,t.extensionClipCullDistance?"#define USE_CLIP_DISTANCE":"",t.batching?"#define USE_BATCHING":"",t.batchingColor?"#define USE_BATCHING_COLOR":"",t.instancing?"#define USE_INSTANCING":"",t.instancingColor?"#define USE_INSTANCING_COLOR":"",t.instancingMorph?"#define USE_INSTANCING_MORPH":"",t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.map?"#define USE_MAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+u:"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.displacementMap?"#define USE_DISPLACEMENTMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.mapUv?"#define MAP_UV "+t.mapUv:"",t.alphaMapUv?"#define ALPHAMAP_UV "+t.alphaMapUv:"",t.lightMapUv?"#define LIGHTMAP_UV "+t.lightMapUv:"",t.aoMapUv?"#define AOMAP_UV "+t.aoMapUv:"",t.emissiveMapUv?"#define EMISSIVEMAP_UV "+t.emissiveMapUv:"",t.bumpMapUv?"#define BUMPMAP_UV "+t.bumpMapUv:"",t.normalMapUv?"#define NORMALMAP_UV "+t.normalMapUv:"",t.displacementMapUv?"#define DISPLACEMENTMAP_UV "+t.displacementMapUv:"",t.metalnessMapUv?"#define METALNESSMAP_UV "+t.metalnessMapUv:"",t.roughnessMapUv?"#define ROUGHNESSMAP_UV "+t.roughnessMapUv:"",t.anisotropyMapUv?"#define ANISOTROPYMAP_UV "+t.anisotropyMapUv:"",t.clearcoatMapUv?"#define CLEARCOATMAP_UV "+t.clearcoatMapUv:"",t.clearcoatNormalMapUv?"#define CLEARCOAT_NORMALMAP_UV "+t.clearcoatNormalMapUv:"",t.clearcoatRoughnessMapUv?"#define CLEARCOAT_ROUGHNESSMAP_UV "+t.clearcoatRoughnessMapUv:"",t.iridescenceMapUv?"#define IRIDESCENCEMAP_UV "+t.iridescenceMapUv:"",t.iridescenceThicknessMapUv?"#define IRIDESCENCE_THICKNESSMAP_UV "+t.iridescenceThicknessMapUv:"",t.sheenColorMapUv?"#define SHEEN_COLORMAP_UV "+t.sheenColorMapUv:"",t.sheenRoughnessMapUv?"#define SHEEN_ROUGHNESSMAP_UV "+t.sheenRoughnessMapUv:"",t.specularMapUv?"#define SPECULARMAP_UV "+t.specularMapUv:"",t.specularColorMapUv?"#define SPECULAR_COLORMAP_UV "+t.specularColorMapUv:"",t.specularIntensityMapUv?"#define SPECULAR_INTENSITYMAP_UV "+t.specularIntensityMapUv:"",t.transmissionMapUv?"#define TRANSMISSIONMAP_UV "+t.transmissionMapUv:"",t.thicknessMapUv?"#define THICKNESSMAP_UV "+t.thicknessMapUv:"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexNormals?"#define HAS_NORMAL":"",t.vertexColors?"#define USE_COLOR":"",t.vertexAlphas?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.flatShading?"#define FLAT_SHADED":"",t.skinning?"#define USE_SKINNING":"",t.morphTargets?"#define USE_MORPHTARGETS":"",t.morphNormals&&t.flatShading===!1?"#define USE_MORPHNORMALS":"",t.morphColors?"#define USE_MORPHCOLORS":"",t.morphTargetsCount>0?"#define MORPHTARGETS_TEXTURE_STRIDE "+t.morphTextureStride:"",t.morphTargetsCount>0?"#define MORPHTARGETS_COUNT "+t.morphTargetsCount:"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+c:"",t.sizeAttenuation?"#define USE_SIZEATTENUATION":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 modelMatrix;","uniform mat4 modelViewMatrix;","uniform mat4 projectionMatrix;","uniform mat4 viewMatrix;","uniform mat3 normalMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;","#ifdef USE_INSTANCING","	attribute mat4 instanceMatrix;","#endif","#ifdef USE_INSTANCING_COLOR","	attribute vec3 instanceColor;","#endif","#ifdef USE_INSTANCING_MORPH","	uniform sampler2D morphTexture;","#endif","attribute vec3 position;","attribute vec3 normal;","attribute vec2 uv;","#ifdef USE_UV1","	attribute vec2 uv1;","#endif","#ifdef USE_UV2","	attribute vec2 uv2;","#endif","#ifdef USE_UV3","	attribute vec2 uv3;","#endif","#ifdef USE_TANGENT","	attribute vec4 tangent;","#endif","#if defined( USE_COLOR_ALPHA )","	attribute vec4 color;","#elif defined( USE_COLOR )","	attribute vec3 color;","#endif","#ifdef USE_SKINNING","	attribute vec4 skinIndex;","	attribute vec4 skinWeight;","#endif",`
`].filter(Dr).join(`
`),h=[Dl(t),"#define SHADER_TYPE "+t.shaderType,"#define SHADER_NAME "+t.shaderName,x,t.useFog&&t.fog?"#define USE_FOG":"",t.useFog&&t.fogExp2?"#define FOG_EXP2":"",t.alphaToCoverage?"#define ALPHA_TO_COVERAGE":"",t.map?"#define USE_MAP":"",t.matcap?"#define USE_MATCAP":"",t.envMap?"#define USE_ENVMAP":"",t.envMap?"#define "+l:"",t.envMap?"#define "+u:"",t.envMap?"#define "+f:"",d?"#define CUBEUV_TEXEL_WIDTH "+d.texelWidth:"",d?"#define CUBEUV_TEXEL_HEIGHT "+d.texelHeight:"",d?"#define CUBEUV_MAX_MIP "+d.maxMip+".0":"",t.lightMap?"#define USE_LIGHTMAP":"",t.aoMap?"#define USE_AOMAP":"",t.bumpMap?"#define USE_BUMPMAP":"",t.normalMap?"#define USE_NORMALMAP":"",t.normalMapObjectSpace?"#define USE_NORMALMAP_OBJECTSPACE":"",t.normalMapTangentSpace?"#define USE_NORMALMAP_TANGENTSPACE":"",t.packedNormalMap?"#define USE_PACKED_NORMALMAP":"",t.emissiveMap?"#define USE_EMISSIVEMAP":"",t.anisotropy?"#define USE_ANISOTROPY":"",t.anisotropyMap?"#define USE_ANISOTROPYMAP":"",t.clearcoat?"#define USE_CLEARCOAT":"",t.clearcoatMap?"#define USE_CLEARCOATMAP":"",t.clearcoatRoughnessMap?"#define USE_CLEARCOAT_ROUGHNESSMAP":"",t.clearcoatNormalMap?"#define USE_CLEARCOAT_NORMALMAP":"",t.dispersion?"#define USE_DISPERSION":"",t.iridescence?"#define USE_IRIDESCENCE":"",t.iridescenceMap?"#define USE_IRIDESCENCEMAP":"",t.iridescenceThicknessMap?"#define USE_IRIDESCENCE_THICKNESSMAP":"",t.specularMap?"#define USE_SPECULARMAP":"",t.specularColorMap?"#define USE_SPECULAR_COLORMAP":"",t.specularIntensityMap?"#define USE_SPECULAR_INTENSITYMAP":"",t.roughnessMap?"#define USE_ROUGHNESSMAP":"",t.metalnessMap?"#define USE_METALNESSMAP":"",t.alphaMap?"#define USE_ALPHAMAP":"",t.alphaTest?"#define USE_ALPHATEST":"",t.alphaHash?"#define USE_ALPHAHASH":"",t.sheen?"#define USE_SHEEN":"",t.sheenColorMap?"#define USE_SHEEN_COLORMAP":"",t.sheenRoughnessMap?"#define USE_SHEEN_ROUGHNESSMAP":"",t.transmission?"#define USE_TRANSMISSION":"",t.transmissionMap?"#define USE_TRANSMISSIONMAP":"",t.thicknessMap?"#define USE_THICKNESSMAP":"",t.vertexTangents&&t.flatShading===!1?"#define USE_TANGENT":"",t.vertexColors||t.instancingColor?"#define USE_COLOR":"",t.vertexAlphas||t.batchingColor?"#define USE_COLOR_ALPHA":"",t.vertexUv1s?"#define USE_UV1":"",t.vertexUv2s?"#define USE_UV2":"",t.vertexUv3s?"#define USE_UV3":"",t.pointsUvs?"#define USE_POINTS_UV":"",t.gradientMap?"#define USE_GRADIENTMAP":"",t.flatShading?"#define FLAT_SHADED":"",t.doubleSided?"#define DOUBLE_SIDED":"",t.flipSided?"#define FLIP_SIDED":"",t.shadowMapEnabled?"#define USE_SHADOWMAP":"",t.shadowMapEnabled?"#define "+c:"",t.premultipliedAlpha?"#define PREMULTIPLIED_ALPHA":"",t.numLightProbes>0?"#define USE_LIGHT_PROBES":"",t.numLightProbeGrids>0?"#define USE_LIGHT_PROBES_GRID":"",t.decodeVideoTexture?"#define DECODE_VIDEO_TEXTURE":"",t.decodeVideoTextureEmissive?"#define DECODE_VIDEO_TEXTURE_EMISSIVE":"",t.logarithmicDepthBuffer?"#define USE_LOGARITHMIC_DEPTH_BUFFER":"",t.reversedDepthBuffer?"#define USE_REVERSED_DEPTH_BUFFER":"","uniform mat4 viewMatrix;","uniform vec3 cameraPosition;","uniform bool isOrthographic;",t.toneMapping!==Pn?"#define TONE_MAPPING":"",t.toneMapping!==Pn?ct.tonemapping_pars_fragment:"",t.toneMapping!==Pn?Rm("toneMapping",t.toneMapping):"",t.dithering?"#define DITHERING":"",t.opaque?"#define OPAQUE":"",ct.colorspace_pars_fragment,Am("linearToOutputTexel",t.outputColorSpace),Cm(),t.useDepthPacking?"#define DEPTH_PACKING "+t.depthPacking:"",`
`].filter(Dr).join(`
`)),a=xo(a),a=Cl(a,t),a=Pl(a,t),o=xo(o),o=Cl(o,t),o=Pl(o,t),a=Ll(a),o=Ll(o),t.isRawShaderMaterial!==!0&&(R=`#version 300 es
`,g=[m,"#define attribute in","#define varying out","#define texture2D texture"].join(`
`)+`
`+g,h=["#define varying in",t.glslVersion===mo?"":"layout(location = 0) out highp vec4 pc_fragColor;",t.glslVersion===mo?"":"#define gl_FragColor pc_fragColor","#define gl_FragDepthEXT gl_FragDepth","#define texture2D texture","#define textureCube texture","#define texture2DProj textureProj","#define texture2DLodEXT textureLod","#define texture2DProjLodEXT textureProjLod","#define textureCubeLodEXT textureLod","#define texture2DGradEXT textureGrad","#define texture2DProjGradEXT textureProjGrad","#define textureCubeGradEXT textureGrad"].join(`
`)+`
`+h);const C=R+g+a,y=R+h+o,w=Al(r,r.VERTEX_SHADER,C),T=Al(r,r.FRAGMENT_SHADER,y);r.attachShader(S,w),r.attachShader(S,T),t.index0AttributeName!==void 0?r.bindAttribLocation(S,0,t.index0AttributeName):t.hasPositionAttribute===!0&&r.bindAttribLocation(S,0,"position"),r.linkProgram(S);function I(F){if(i.debug.checkShaderErrors){const b=r.getProgramInfoLog(S)||"",P=r.getShaderInfoLog(w)||"",W=r.getShaderInfoLog(T)||"",N=b.trim(),D=P.trim(),G=W.trim();let H=!0,ne=!0;if(r.getProgramParameter(S,r.LINK_STATUS)===!1)if(H=!1,typeof i.debug.onShaderError=="function")i.debug.onShaderError(r,S,w,T);else{const ce=Rl(r,w,"vertex"),oe=Rl(r,T,"fragment");_t("WebGLProgram: Shader Error "+r.getError()+" - VALIDATE_STATUS "+r.getProgramParameter(S,r.VALIDATE_STATUS)+`

Material Name: `+F.name+`
Material Type: `+F.type+`

Program Info Log: `+N+`
`+ce+`
`+oe)}else N!==""?je("WebGLProgram: Program Info Log:",N):(D===""||G==="")&&(ne=!1);ne&&(F.diagnostics={runnable:H,programLog:N,vertexShader:{log:D,prefix:g},fragmentShader:{log:G,prefix:h}})}r.deleteShader(w),r.deleteShader(T),_=new Es(r,S),A=Dm(r,S)}let _;this.getUniforms=function(){return _===void 0&&I(this),_};let A;this.getAttributes=function(){return A===void 0&&I(this),A};let k=t.rendererExtensionParallelShaderCompile===!1;return this.isReady=function(){return k===!1&&(k=r.getProgramParameter(S,ym)),k},this.destroy=function(){n.releaseStatesOfProgram(this),r.deleteProgram(S),this.program=void 0},this.type=t.shaderType,this.name=t.shaderName,this.id=bm++,this.cacheKey=e,this.usedTimes=1,this.program=S,this.vertexShader=w,this.fragmentShader=T,this}let Km=0;class Zm{constructor(){this.shaderCache=new Map,this.materialCache=new Map}update(e,t,n){const r=this._getShaderCacheForMaterial(e);return r.has(t)===!1&&(r.add(t),t.usedTimes++),r.has(n)===!1&&(r.add(n),n.usedTimes++),this}remove(e){const t=this.materialCache.get(e);for(const n of t)n.usedTimes--,n.usedTimes===0&&this.shaderCache.delete(n.code);return this.materialCache.delete(e),this}getVertexShaderStage(e){return this._getShaderStage(e.vertexShader)}getFragmentShaderStage(e){return this._getShaderStage(e.fragmentShader)}dispose(){this.shaderCache.clear(),this.materialCache.clear()}_getShaderCacheForMaterial(e){const t=this.materialCache;let n=t.get(e);return n===void 0&&(n=new Set,t.set(e,n)),n}_getShaderStage(e){const t=this.shaderCache;let n=t.get(e);return n===void 0&&(n=new $m(e),t.set(e,n)),n}}class $m{constructor(e){this.id=Km++,this.code=e,this.usedTimes=0}}function Jm(i){return i===Bi||i===Ts||i===As}function Qm(i,e,t,n,r,s){const a=new pc,o=new Zm,c=new Set,l=[],u=new Map,f=n.logarithmicDepthBuffer;let d=n.precision;const m={MeshDepthMaterial:"depth",MeshDistanceMaterial:"distance",MeshNormalMaterial:"normal",MeshBasicMaterial:"basic",MeshLambertMaterial:"lambert",MeshPhongMaterial:"phong",MeshToonMaterial:"toon",MeshStandardMaterial:"physical",MeshPhysicalMaterial:"physical",MeshMatcapMaterial:"matcap",LineBasicMaterial:"basic",LineDashedMaterial:"dashed",PointsMaterial:"points",ShadowMaterial:"shadow",SpriteMaterial:"sprite"};function x(_){return c.add(_),_===0?"uv":`uv${_}`}function S(_,A,k,F,b,P){const W=F.fog,N=b.geometry,D=_.isMeshStandardMaterial||_.isMeshLambertMaterial||_.isMeshPhongMaterial?F.environment:null,G=_.isMeshStandardMaterial||_.isMeshLambertMaterial&&!_.envMap||_.isMeshPhongMaterial&&!_.envMap,H=e.get(_.envMap||D,G),ne=H&&H.mapping===Us?H.image.height:null,ce=m[_.type];_.precision!==null&&(d=n.getMaxPrecision(_.precision),d!==_.precision&&je("WebGLProgram.getParameters:",_.precision,"not supported, using",d,"instead."));const oe=N.morphAttributes.position||N.morphAttributes.normal||N.morphAttributes.color,_e=oe!==void 0?oe.length:0;let Qe=0;N.morphAttributes.position!==void 0&&(Qe=1),N.morphAttributes.normal!==void 0&&(Qe=2),N.morphAttributes.color!==void 0&&(Qe=3);let Ze,He,Z,de;if(ce){const Me=kn[ce];Ze=Me.vertexShader,He=Me.fragmentShader}else{Ze=_.vertexShader,He=_.fragmentShader;const Me=o.getVertexShaderStage(_),At=o.getFragmentShaderStage(_);o.update(_,Me,At),Z=Me.id,de=At.id}const le=i.getRenderTarget(),Oe=i.state.buffers.depth.getReversed(),Je=b.isInstancedMesh===!0,Te=b.isBatchedMesh===!0,Q=!!_.map,te=!!_.matcap,pe=!!H,Se=!!_.aoMap,Ce=!!_.lightMap,We=!!_.bumpMap&&_.wireframe===!1,qe=!!_.normalMap,ut=!!_.displacementMap,ot=!!_.emissiveMap,Le=!!_.metalnessMap,et=!!_.roughnessMap,L=_.anisotropy>0,ht=_.clearcoat>0,$e=_.dispersion>0,E=_.iridescence>0,p=_.sheen>0,z=_.transmission>0,q=L&&!!_.anisotropyMap,J=ht&&!!_.clearcoatMap,me=ht&&!!_.clearcoatNormalMap,ue=ht&&!!_.clearcoatRoughnessMap,j=E&&!!_.iridescenceMap,ee=E&&!!_.iridescenceThicknessMap,fe=p&&!!_.sheenColorMap,Pe=p&&!!_.sheenRoughnessMap,he=!!_.specularMap,ve=!!_.specularColorMap,Be=!!_.specularIntensityMap,Ge=z&&!!_.transmissionMap,tt=z&&!!_.thicknessMap,U=!!_.gradientMap,ge=!!_.alphaMap,ie=_.alphaTest>0,xe=!!_.alphaHash,ae=!!_.extensions;let se=Pn;_.toneMapped&&(le===null||le.isXRRenderTarget===!0)&&(se=i.toneMapping);const De={shaderID:ce,shaderType:_.type,shaderName:_.name,vertexShader:Ze,fragmentShader:He,defines:_.defines,customVertexShaderID:Z,customFragmentShaderID:de,isRawShaderMaterial:_.isRawShaderMaterial===!0,glslVersion:_.glslVersion,precision:d,batching:Te,batchingColor:Te&&b._colorsTexture!==null,instancing:Je,instancingColor:Je&&b.instanceColor!==null,instancingMorph:Je&&b.morphTexture!==null,outputColorSpace:le===null?i.outputColorSpace:le.isXRRenderTarget===!0?le.texture.colorSpace:mt.workingColorSpace,alphaToCoverage:!!_.alphaToCoverage,map:Q,matcap:te,envMap:pe,envMapMode:pe&&H.mapping,envMapCubeUVHeight:ne,aoMap:Se,lightMap:Ce,bumpMap:We,normalMap:qe,displacementMap:ut,emissiveMap:ot,normalMapObjectSpace:qe&&_.normalMapType===Nd,normalMapTangentSpace:qe&&_.normalMapType===qo,packedNormalMap:qe&&_.normalMapType===qo&&Jm(_.normalMap.format),metalnessMap:Le,roughnessMap:et,anisotropy:L,anisotropyMap:q,clearcoat:ht,clearcoatMap:J,clearcoatNormalMap:me,clearcoatRoughnessMap:ue,dispersion:$e,iridescence:E,iridescenceMap:j,iridescenceThicknessMap:ee,sheen:p,sheenColorMap:fe,sheenRoughnessMap:Pe,specularMap:he,specularColorMap:ve,specularIntensityMap:Be,transmission:z,transmissionMap:Ge,thicknessMap:tt,gradientMap:U,opaque:_.transparent===!1&&_.blending===ur&&_.alphaToCoverage===!1,alphaMap:ge,alphaTest:ie,alphaHash:xe,combine:_.combine,mapUv:Q&&x(_.map.channel),aoMapUv:Se&&x(_.aoMap.channel),lightMapUv:Ce&&x(_.lightMap.channel),bumpMapUv:We&&x(_.bumpMap.channel),normalMapUv:qe&&x(_.normalMap.channel),displacementMapUv:ut&&x(_.displacementMap.channel),emissiveMapUv:ot&&x(_.emissiveMap.channel),metalnessMapUv:Le&&x(_.metalnessMap.channel),roughnessMapUv:et&&x(_.roughnessMap.channel),anisotropyMapUv:q&&x(_.anisotropyMap.channel),clearcoatMapUv:J&&x(_.clearcoatMap.channel),clearcoatNormalMapUv:me&&x(_.clearcoatNormalMap.channel),clearcoatRoughnessMapUv:ue&&x(_.clearcoatRoughnessMap.channel),iridescenceMapUv:j&&x(_.iridescenceMap.channel),iridescenceThicknessMapUv:ee&&x(_.iridescenceThicknessMap.channel),sheenColorMapUv:fe&&x(_.sheenColorMap.channel),sheenRoughnessMapUv:Pe&&x(_.sheenRoughnessMap.channel),specularMapUv:he&&x(_.specularMap.channel),specularColorMapUv:ve&&x(_.specularColorMap.channel),specularIntensityMapUv:Be&&x(_.specularIntensityMap.channel),transmissionMapUv:Ge&&x(_.transmissionMap.channel),thicknessMapUv:tt&&x(_.thicknessMap.channel),alphaMapUv:ge&&x(_.alphaMap.channel),vertexTangents:!!N.attributes.tangent&&(qe||L),vertexNormals:!!N.attributes.normal,vertexColors:_.vertexColors,vertexAlphas:_.vertexColors===!0&&!!N.attributes.color&&N.attributes.color.itemSize===4,pointsUvs:b.isPoints===!0&&!!N.attributes.uv&&(Q||ge),fog:!!W,useFog:_.fog===!0,fogExp2:!!W&&W.isFogExp2,flatShading:_.wireframe===!1&&(_.flatShading===!0||N.attributes.normal===void 0&&qe===!1&&(_.isMeshLambertMaterial||_.isMeshPhongMaterial||_.isMeshStandardMaterial||_.isMeshPhysicalMaterial)),sizeAttenuation:_.sizeAttenuation===!0,logarithmicDepthBuffer:f,reversedDepthBuffer:Oe,skinning:b.isSkinnedMesh===!0,hasPositionAttribute:N.attributes.position!==void 0,morphTargets:N.morphAttributes.position!==void 0,morphNormals:N.morphAttributes.normal!==void 0,morphColors:N.morphAttributes.color!==void 0,morphTargetsCount:_e,morphTextureStride:Qe,numDirLights:A.directional.length,numPointLights:A.point.length,numSpotLights:A.spot.length,numSpotLightMaps:A.spotLightMap.length,numRectAreaLights:A.rectArea.length,numHemiLights:A.hemi.length,numDirLightShadows:A.directionalShadowMap.length,numPointLightShadows:A.pointShadowMap.length,numSpotLightShadows:A.spotShadowMap.length,numSpotLightShadowsWithMaps:A.numSpotLightShadowsWithMaps,numLightProbes:A.numLightProbes,numLightProbeGrids:P.length,numClippingPlanes:s.numPlanes,numClipIntersection:s.numIntersection,dithering:_.dithering,shadowMapEnabled:i.shadowMap.enabled&&k.length>0,shadowMapType:i.shadowMap.type,toneMapping:se,decodeVideoTexture:Q&&_.map.isVideoTexture===!0&&mt.getTransfer(_.map.colorSpace)===Ct,decodeVideoTextureEmissive:ot&&_.emissiveMap.isVideoTexture===!0&&mt.getTransfer(_.emissiveMap.colorSpace)===Ct,premultipliedAlpha:_.premultipliedAlpha,doubleSided:_.side===ni,flipSided:_.side===cn,useDepthPacking:_.depthPacking>=0,depthPacking:_.depthPacking||0,index0AttributeName:_.index0AttributeName,extensionClipCullDistance:ae&&_.extensions.clipCullDistance===!0&&t.has("WEBGL_clip_cull_distance"),extensionMultiDraw:(ae&&_.extensions.multiDraw===!0||Te)&&t.has("WEBGL_multi_draw"),rendererExtensionParallelShaderCompile:t.has("KHR_parallel_shader_compile"),customProgramCacheKey:_.customProgramCacheKey()};return De.vertexUv1s=c.has(1),De.vertexUv2s=c.has(2),De.vertexUv3s=c.has(3),c.clear(),De}function g(_){const A=[];if(_.shaderID?A.push(_.shaderID):(A.push(_.customVertexShaderID),A.push(_.customFragmentShaderID)),_.defines!==void 0)for(const k in _.defines)A.push(k),A.push(_.defines[k]);return _.isRawShaderMaterial===!1&&(h(A,_),R(A,_),A.push(i.outputColorSpace)),A.push(_.customProgramCacheKey),A.join()}function h(_,A){_.push(A.precision),_.push(A.outputColorSpace),_.push(A.envMapMode),_.push(A.envMapCubeUVHeight),_.push(A.mapUv),_.push(A.alphaMapUv),_.push(A.lightMapUv),_.push(A.aoMapUv),_.push(A.bumpMapUv),_.push(A.normalMapUv),_.push(A.displacementMapUv),_.push(A.emissiveMapUv),_.push(A.metalnessMapUv),_.push(A.roughnessMapUv),_.push(A.anisotropyMapUv),_.push(A.clearcoatMapUv),_.push(A.clearcoatNormalMapUv),_.push(A.clearcoatRoughnessMapUv),_.push(A.iridescenceMapUv),_.push(A.iridescenceThicknessMapUv),_.push(A.sheenColorMapUv),_.push(A.sheenRoughnessMapUv),_.push(A.specularMapUv),_.push(A.specularColorMapUv),_.push(A.specularIntensityMapUv),_.push(A.transmissionMapUv),_.push(A.thicknessMapUv),_.push(A.combine),_.push(A.fogExp2),_.push(A.sizeAttenuation),_.push(A.morphTargetsCount),_.push(A.morphAttributeCount),_.push(A.numDirLights),_.push(A.numPointLights),_.push(A.numSpotLights),_.push(A.numSpotLightMaps),_.push(A.numHemiLights),_.push(A.numRectAreaLights),_.push(A.numDirLightShadows),_.push(A.numPointLightShadows),_.push(A.numSpotLightShadows),_.push(A.numSpotLightShadowsWithMaps),_.push(A.numLightProbes),_.push(A.shadowMapType),_.push(A.toneMapping),_.push(A.numClippingPlanes),_.push(A.numClipIntersection),_.push(A.depthPacking)}function R(_,A){a.disableAll(),A.instancing&&a.enable(0),A.instancingColor&&a.enable(1),A.instancingMorph&&a.enable(2),A.matcap&&a.enable(3),A.envMap&&a.enable(4),A.normalMapObjectSpace&&a.enable(5),A.normalMapTangentSpace&&a.enable(6),A.clearcoat&&a.enable(7),A.iridescence&&a.enable(8),A.alphaTest&&a.enable(9),A.vertexColors&&a.enable(10),A.vertexAlphas&&a.enable(11),A.vertexUv1s&&a.enable(12),A.vertexUv2s&&a.enable(13),A.vertexUv3s&&a.enable(14),A.vertexTangents&&a.enable(15),A.anisotropy&&a.enable(16),A.alphaHash&&a.enable(17),A.batching&&a.enable(18),A.dispersion&&a.enable(19),A.batchingColor&&a.enable(20),A.gradientMap&&a.enable(21),A.packedNormalMap&&a.enable(22),A.vertexNormals&&a.enable(23),_.push(a.mask),a.disableAll(),A.fog&&a.enable(0),A.useFog&&a.enable(1),A.flatShading&&a.enable(2),A.logarithmicDepthBuffer&&a.enable(3),A.reversedDepthBuffer&&a.enable(4),A.skinning&&a.enable(5),A.morphTargets&&a.enable(6),A.morphNormals&&a.enable(7),A.morphColors&&a.enable(8),A.premultipliedAlpha&&a.enable(9),A.shadowMapEnabled&&a.enable(10),A.doubleSided&&a.enable(11),A.flipSided&&a.enable(12),A.useDepthPacking&&a.enable(13),A.dithering&&a.enable(14),A.transmission&&a.enable(15),A.sheen&&a.enable(16),A.opaque&&a.enable(17),A.pointsUvs&&a.enable(18),A.decodeVideoTexture&&a.enable(19),A.decodeVideoTextureEmissive&&a.enable(20),A.alphaToCoverage&&a.enable(21),A.numLightProbeGrids>0&&a.enable(22),A.hasPositionAttribute&&a.enable(23),_.push(a.mask)}function C(_){const A=m[_.type];let k;if(A){const F=kn[A];k=yu.clone(F.uniforms)}else k=_.uniforms;return k}function y(_,A){let k=u.get(A);return k!==void 0?++k.usedTimes:(k=new Ym(i,A,_,r),l.push(k),u.set(A,k)),k}function w(_){if(--_.usedTimes===0){const A=l.indexOf(_);l[A]=l[l.length-1],l.pop(),u.delete(_.cacheKey),_.destroy()}}function T(_){o.remove(_)}function I(){o.dispose()}return{getParameters:S,getProgramCacheKey:g,getUniforms:C,acquireProgram:y,releaseProgram:w,releaseShaderCache:T,programs:l,dispose:I}}function jm(){let i=new WeakMap;function e(a){return i.has(a)}function t(a){let o=i.get(a);return o===void 0&&(o={},i.set(a,o)),o}function n(a){i.delete(a)}function r(a,o,c){i.get(a)[o]=c}function s(){i=new WeakMap}return{has:e,get:t,remove:n,update:r,dispose:s}}function eg(i,e){return i.groupOrder!==e.groupOrder?i.groupOrder-e.groupOrder:i.renderOrder!==e.renderOrder?i.renderOrder-e.renderOrder:i.material.id!==e.material.id?i.material.id-e.material.id:i.materialVariant!==e.materialVariant?i.materialVariant-e.materialVariant:i.z!==e.z?i.z-e.z:i.id-e.id}function Il(i,e){return i.groupOrder!==e.groupOrder?i.groupOrder-e.groupOrder:i.renderOrder!==e.renderOrder?i.renderOrder-e.renderOrder:i.z!==e.z?e.z-i.z:i.id-e.id}function Ul(){const i=[];let e=0;const t=[],n=[],r=[];function s(){e=0,t.length=0,n.length=0,r.length=0}function a(d){let m=0;return d.isInstancedMesh&&(m+=2),d.isSkinnedMesh&&(m+=1),m}function o(d,m,x,S,g,h){let R=i[e];return R===void 0?(R={id:d.id,object:d,geometry:m,material:x,materialVariant:a(d),groupOrder:S,renderOrder:d.renderOrder,z:g,group:h},i[e]=R):(R.id=d.id,R.object=d,R.geometry=m,R.material=x,R.materialVariant=a(d),R.groupOrder=S,R.renderOrder=d.renderOrder,R.z=g,R.group=h),e++,R}function c(d,m,x,S,g,h){const R=o(d,m,x,S,g,h);x.transmission>0?n.push(R):x.transparent===!0?r.push(R):t.push(R)}function l(d,m,x,S,g,h){const R=o(d,m,x,S,g,h);x.transmission>0?n.unshift(R):x.transparent===!0?r.unshift(R):t.unshift(R)}function u(d,m,x){t.length>1&&t.sort(d||eg),n.length>1&&n.sort(m||Il),r.length>1&&r.sort(m||Il),x&&(t.reverse(),n.reverse(),r.reverse())}function f(){for(let d=e,m=i.length;d<m;d++){const x=i[d];if(x.id===null)break;x.id=null,x.object=null,x.geometry=null,x.material=null,x.group=null}}return{opaque:t,transmissive:n,transparent:r,init:s,push:c,unshift:l,finish:f,sort:u}}function tg(){let i=new WeakMap;function e(n,r){const s=i.get(n);let a;return s===void 0?(a=new Ul,i.set(n,[a])):r>=s.length?(a=new Ul,s.push(a)):a=s[r],a}function t(){i=new WeakMap}return{get:e,dispose:t}}function ng(){const i={};return{get:function(e){if(i[e.id]!==void 0)return i[e.id];let t;switch(e.type){case"DirectionalLight":t={direction:new K,color:new Tt};break;case"SpotLight":t={position:new K,direction:new K,color:new Tt,distance:0,coneCos:0,penumbraCos:0,decay:0};break;case"PointLight":t={position:new K,color:new Tt,distance:0,decay:0};break;case"HemisphereLight":t={direction:new K,skyColor:new Tt,groundColor:new Tt};break;case"RectAreaLight":t={color:new Tt,position:new K,halfWidth:new K,halfHeight:new K};break}return i[e.id]=t,t}}}function ig(){const i={};return{get:function(e){if(i[e.id]!==void 0)return i[e.id];let t;switch(e.type){case"DirectionalLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Mt};break;case"SpotLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Mt};break;case"PointLight":t={shadowIntensity:1,shadowBias:0,shadowNormalBias:0,shadowRadius:1,shadowMapSize:new Mt,shadowCameraNear:1,shadowCameraFar:1e3};break}return i[e.id]=t,t}}}let rg=0;function sg(i,e){return(e.castShadow?2:0)-(i.castShadow?2:0)+(e.map?1:0)-(i.map?1:0)}function ag(i){const e=new ng,t=ig(),n={version:0,hash:{directionalLength:-1,pointLength:-1,spotLength:-1,rectAreaLength:-1,hemiLength:-1,numDirectionalShadows:-1,numPointShadows:-1,numSpotShadows:-1,numSpotMaps:-1,numLightProbes:-1},ambient:[0,0,0],probe:[],directional:[],directionalShadow:[],directionalShadowMap:[],directionalShadowMatrix:[],spot:[],spotLightMap:[],spotShadow:[],spotShadowMap:[],spotLightMatrix:[],rectArea:[],rectAreaLTC1:null,rectAreaLTC2:null,point:[],pointShadow:[],pointShadowMap:[],pointShadowMatrix:[],hemi:[],numSpotLightShadowsWithMaps:0,numLightProbes:0};for(let l=0;l<9;l++)n.probe.push(new K);const r=new K,s=new Vt,a=new Vt;function o(l){let u=0,f=0,d=0;for(let A=0;A<9;A++)n.probe[A].set(0,0,0);let m=0,x=0,S=0,g=0,h=0,R=0,C=0,y=0,w=0,T=0,I=0;l.sort(sg);for(let A=0,k=l.length;A<k;A++){const F=l[A],b=F.color,P=F.intensity,W=F.distance;let N=null;if(F.shadow&&F.shadow.map&&(F.shadow.map.texture.format===Bi?N=F.shadow.map.texture:N=F.shadow.map.depthTexture||F.shadow.map.texture),F.isAmbientLight)u+=b.r*P,f+=b.g*P,d+=b.b*P;else if(F.isLightProbe){for(let D=0;D<9;D++)n.probe[D].addScaledVector(F.sh.coefficients[D],P);I++}else if(F.isDirectionalLight){const D=e.get(F);if(D.color.copy(F.color).multiplyScalar(F.intensity),F.castShadow){const G=F.shadow,H=t.get(F);H.shadowIntensity=G.intensity,H.shadowBias=G.bias,H.shadowNormalBias=G.normalBias,H.shadowRadius=G.radius,H.shadowMapSize=G.mapSize,n.directionalShadow[m]=H,n.directionalShadowMap[m]=N,n.directionalShadowMatrix[m]=F.shadow.matrix,R++}n.directional[m]=D,m++}else if(F.isSpotLight){const D=e.get(F);D.position.setFromMatrixPosition(F.matrixWorld),D.color.copy(b).multiplyScalar(P),D.distance=W,D.coneCos=Math.cos(F.angle),D.penumbraCos=Math.cos(F.angle*(1-F.penumbra)),D.decay=F.decay,n.spot[S]=D;const G=F.shadow;if(F.map&&(n.spotLightMap[w]=F.map,w++,G.updateMatrices(F),F.castShadow&&T++),n.spotLightMatrix[S]=G.matrix,F.castShadow){const H=t.get(F);H.shadowIntensity=G.intensity,H.shadowBias=G.bias,H.shadowNormalBias=G.normalBias,H.shadowRadius=G.radius,H.shadowMapSize=G.mapSize,n.spotShadow[S]=H,n.spotShadowMap[S]=N,y++}S++}else if(F.isRectAreaLight){const D=e.get(F);D.color.copy(b).multiplyScalar(P),D.halfWidth.set(F.width*.5,0,0),D.halfHeight.set(0,F.height*.5,0),n.rectArea[g]=D,g++}else if(F.isPointLight){const D=e.get(F);if(D.color.copy(F.color).multiplyScalar(F.intensity),D.distance=F.distance,D.decay=F.decay,F.castShadow){const G=F.shadow,H=t.get(F);H.shadowIntensity=G.intensity,H.shadowBias=G.bias,H.shadowNormalBias=G.normalBias,H.shadowRadius=G.radius,H.shadowMapSize=G.mapSize,H.shadowCameraNear=G.camera.near,H.shadowCameraFar=G.camera.far,n.pointShadow[x]=H,n.pointShadowMap[x]=N,n.pointShadowMatrix[x]=F.shadow.matrix,C++}n.point[x]=D,x++}else if(F.isHemisphereLight){const D=e.get(F);D.skyColor.copy(F.color).multiplyScalar(P),D.groundColor.copy(F.groundColor).multiplyScalar(P),n.hemi[h]=D,h++}}g>0&&(i.has("OES_texture_float_linear")===!0?(n.rectAreaLTC1=Ee.LTC_FLOAT_1,n.rectAreaLTC2=Ee.LTC_FLOAT_2):(n.rectAreaLTC1=Ee.LTC_HALF_1,n.rectAreaLTC2=Ee.LTC_HALF_2)),n.ambient[0]=u,n.ambient[1]=f,n.ambient[2]=d;const _=n.hash;(_.directionalLength!==m||_.pointLength!==x||_.spotLength!==S||_.rectAreaLength!==g||_.hemiLength!==h||_.numDirectionalShadows!==R||_.numPointShadows!==C||_.numSpotShadows!==y||_.numSpotMaps!==w||_.numLightProbes!==I)&&(n.directional.length=m,n.spot.length=S,n.rectArea.length=g,n.point.length=x,n.hemi.length=h,n.directionalShadow.length=R,n.directionalShadowMap.length=R,n.pointShadow.length=C,n.pointShadowMap.length=C,n.spotShadow.length=y,n.spotShadowMap.length=y,n.directionalShadowMatrix.length=R,n.pointShadowMatrix.length=C,n.spotLightMatrix.length=y+w-T,n.spotLightMap.length=w,n.numSpotLightShadowsWithMaps=T,n.numLightProbes=I,_.directionalLength=m,_.pointLength=x,_.spotLength=S,_.rectAreaLength=g,_.hemiLength=h,_.numDirectionalShadows=R,_.numPointShadows=C,_.numSpotShadows=y,_.numSpotMaps=w,_.numLightProbes=I,n.version=rg++)}function c(l,u){let f=0,d=0,m=0,x=0,S=0;const g=u.matrixWorldInverse;for(let h=0,R=l.length;h<R;h++){const C=l[h];if(C.isDirectionalLight){const y=n.directional[f];y.direction.setFromMatrixPosition(C.matrixWorld),r.setFromMatrixPosition(C.target.matrixWorld),y.direction.sub(r),y.direction.transformDirection(g),f++}else if(C.isSpotLight){const y=n.spot[m];y.position.setFromMatrixPosition(C.matrixWorld),y.position.applyMatrix4(g),y.direction.setFromMatrixPosition(C.matrixWorld),r.setFromMatrixPosition(C.target.matrixWorld),y.direction.sub(r),y.direction.transformDirection(g),m++}else if(C.isRectAreaLight){const y=n.rectArea[x];y.position.setFromMatrixPosition(C.matrixWorld),y.position.applyMatrix4(g),a.identity(),s.copy(C.matrixWorld),s.premultiply(g),a.extractRotation(s),y.halfWidth.set(C.width*.5,0,0),y.halfHeight.set(0,C.height*.5,0),y.halfWidth.applyMatrix4(a),y.halfHeight.applyMatrix4(a),x++}else if(C.isPointLight){const y=n.point[d];y.position.setFromMatrixPosition(C.matrixWorld),y.position.applyMatrix4(g),d++}else if(C.isHemisphereLight){const y=n.hemi[S];y.direction.setFromMatrixPosition(C.matrixWorld),y.direction.transformDirection(g),S++}}}return{setup:o,setupView:c,state:n}}function Nl(i){const e=new ag(i),t=[],n=[],r=[];function s(d){f.camera=d,t.length=0,n.length=0,r.length=0}function a(d){t.push(d)}function o(d){n.push(d)}function c(d){r.push(d)}function l(){e.setup(t)}function u(d){e.setupView(t,d)}const f={lightsArray:t,shadowsArray:n,lightProbeGridArray:r,camera:null,lights:e,transmissionRenderTarget:{},textureUnits:0};return{init:s,state:f,setupLights:l,setupLightsView:u,pushLight:a,pushShadow:o,pushLightProbeGrid:c}}function og(i){let e=new WeakMap;function t(r,s=0){const a=e.get(r);let o;return a===void 0?(o=new Nl(i),e.set(r,[o])):s>=a.length?(o=new Nl(i),a.push(o)):o=a[s],o}function n(){e=new WeakMap}return{get:t,dispose:n}}const lg=`void main() {
	gl_Position = vec4( position, 1.0 );
}`,cg=`uniform sampler2D shadow_pass;
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
}`,dg=[new K(1,0,0),new K(-1,0,0),new K(0,1,0),new K(0,-1,0),new K(0,0,1),new K(0,0,-1)],ug=[new K(0,-1,0),new K(0,-1,0),new K(0,0,1),new K(0,0,-1),new K(0,-1,0),new K(0,-1,0)],Fl=new Vt,Cr=new K,ya=new K;function hg(i,e,t){let n=new vc;const r=new Mt,s=new Mt,a=new Ot,o=new Tu,c=new Au,l={},u=t.maxTextureSize,f={[yi]:cn,[cn]:yi,[ni]:ni},d=new Yn({defines:{VSM_SAMPLES:8},uniforms:{shadow_pass:{value:null},resolution:{value:new Mt},radius:{value:4}},vertexShader:lg,fragmentShader:cg}),m=d.clone();m.defines.HORIZONTAL_PASS=1;const x=new Kn;x.setAttribute("position",new Ln(new Float32Array([-1,-1,.5,3,-1,.5,-1,3,.5]),3));const S=new qn(x,d),g=this;this.enabled=!1,this.autoUpdate=!0,this.needsUpdate=!1,this.type=vs;let h=this.type;this.render=function(T,I,_){if(g.enabled===!1||g.autoUpdate===!1&&g.needsUpdate===!1||T.length===0)return;this.type===md&&(je("WebGLShadowMap: PCFSoftShadowMap has been deprecated. Using PCFShadowMap instead."),this.type=vs);const A=i.getRenderTarget(),k=i.getActiveCubeFace(),F=i.getActiveMipmapLevel(),b=i.state;b.setBlending(si),b.buffers.depth.getReversed()===!0?b.buffers.color.setClear(0,0,0,0):b.buffers.color.setClear(1,1,1,1),b.buffers.depth.setTest(!0),b.setScissorTest(!1);const P=h!==this.type;P&&I.traverse(function(W){W.material&&(Array.isArray(W.material)?W.material.forEach(N=>N.needsUpdate=!0):W.material.needsUpdate=!0)});for(let W=0,N=T.length;W<N;W++){const D=T[W],G=D.shadow;if(G===void 0){je("WebGLShadowMap:",D,"has no shadow.");continue}if(G.autoUpdate===!1&&G.needsUpdate===!1)continue;r.copy(G.mapSize);const H=G.getFrameExtents();r.multiply(H),s.copy(G.mapSize),(r.x>u||r.y>u)&&(r.x>u&&(s.x=Math.floor(u/H.x),r.x=s.x*H.x,G.mapSize.x=s.x),r.y>u&&(s.y=Math.floor(u/H.y),r.y=s.y*H.y,G.mapSize.y=s.y));const ne=i.state.buffers.depth.getReversed();if(G.camera._reversedDepth=ne,G.map===null||P===!0){if(G.map!==null&&(G.map.depthTexture!==null&&(G.map.depthTexture.dispose(),G.map.depthTexture=null),G.map.dispose()),this.type===Lr){if(D.isPointLight){je("WebGLShadowMap: VSM shadow maps are not supported for PointLights. Use PCF or BasicShadowMap instead.");continue}G.map=new Wn(r.x,r.y,{format:Bi,type:li,minFilter:Qt,magFilter:Qt,generateMipmaps:!1}),G.map.texture.name=D.name+".shadowMap",G.map.depthTexture=new gr(r.x,r.y,Vn),G.map.depthTexture.name=D.name+".shadowMapDepth",G.map.depthTexture.format=ci,G.map.depthTexture.compareFunction=null,G.map.depthTexture.minFilter=Jt,G.map.depthTexture.magFilter=Jt}else D.isPointLight?(G.map=new wc(r.x),G.map.depthTexture=new Mu(r.x,Xn)):(G.map=new Wn(r.x,r.y),G.map.depthTexture=new gr(r.x,r.y,Xn)),G.map.depthTexture.name=D.name+".shadowMap",G.map.depthTexture.format=ci,this.type===vs?(G.map.depthTexture.compareFunction=ne?Co:Ro,G.map.depthTexture.minFilter=Qt,G.map.depthTexture.magFilter=Qt):(G.map.depthTexture.compareFunction=null,G.map.depthTexture.minFilter=Jt,G.map.depthTexture.magFilter=Jt);G.camera.updateProjectionMatrix()}const ce=G.map.isWebGLCubeRenderTarget?6:1;for(let oe=0;oe<ce;oe++){if(G.map.isWebGLCubeRenderTarget)i.setRenderTarget(G.map,oe),i.clear();else{oe===0&&(i.setRenderTarget(G.map),i.clear());const _e=G.getViewport(oe);a.set(s.x*_e.x,s.y*_e.y,s.x*_e.z,s.y*_e.w),b.viewport(a)}if(D.isPointLight){const _e=G.camera,Qe=G.matrix,Ze=D.distance||_e.far;Ze!==_e.far&&(_e.far=Ze,_e.updateProjectionMatrix()),Cr.setFromMatrixPosition(D.matrixWorld),_e.position.copy(Cr),ya.copy(_e.position),ya.add(dg[oe]),_e.up.copy(ug[oe]),_e.lookAt(ya),_e.updateMatrixWorld(),Qe.makeTranslation(-Cr.x,-Cr.y,-Cr.z),Fl.multiplyMatrices(_e.projectionMatrix,_e.matrixWorldInverse),G._frustum.setFromProjectionMatrix(Fl,_e.coordinateSystem,_e.reversedDepth)}else G.updateMatrices(D);n=G.getFrustum(),y(I,_,G.camera,D,this.type)}G.isPointLightShadow!==!0&&this.type===Lr&&R(G,_),G.needsUpdate=!1}h=this.type,g.needsUpdate=!1,i.setRenderTarget(A,k,F)};function R(T,I){const _=e.update(S);d.defines.VSM_SAMPLES!==T.blurSamples&&(d.defines.VSM_SAMPLES=T.blurSamples,m.defines.VSM_SAMPLES=T.blurSamples,d.needsUpdate=!0,m.needsUpdate=!0),T.mapPass===null&&(T.mapPass=new Wn(r.x,r.y,{format:Bi,type:li})),d.uniforms.shadow_pass.value=T.map.depthTexture,d.uniforms.resolution.value=T.mapSize,d.uniforms.radius.value=T.radius,i.setRenderTarget(T.mapPass),i.clear(),i.renderBufferDirect(I,null,_,d,S,null),m.uniforms.shadow_pass.value=T.mapPass.texture,m.uniforms.resolution.value=T.mapSize,m.uniforms.radius.value=T.radius,i.setRenderTarget(T.map),i.clear(),i.renderBufferDirect(I,null,_,m,S,null)}function C(T,I,_,A){let k=null;const F=_.isPointLight===!0?T.customDistanceMaterial:T.customDepthMaterial;if(F!==void 0)k=F;else if(k=_.isPointLight===!0?c:o,i.localClippingEnabled&&I.clipShadows===!0&&Array.isArray(I.clippingPlanes)&&I.clippingPlanes.length!==0||I.displacementMap&&I.displacementScale!==0||I.alphaMap&&I.alphaTest>0||I.map&&I.alphaTest>0||I.alphaToCoverage===!0){const b=k.uuid,P=I.uuid;let W=l[b];W===void 0&&(W={},l[b]=W);let N=W[P];N===void 0&&(N=k.clone(),W[P]=N,I.addEventListener("dispose",w)),k=N}if(k.visible=I.visible,k.wireframe=I.wireframe,A===Lr?k.side=I.shadowSide!==null?I.shadowSide:I.side:k.side=I.shadowSide!==null?I.shadowSide:f[I.side],k.alphaMap=I.alphaMap,k.alphaTest=I.alphaToCoverage===!0?.5:I.alphaTest,k.map=I.map,k.clipShadows=I.clipShadows,k.clippingPlanes=I.clippingPlanes,k.clipIntersection=I.clipIntersection,k.displacementMap=I.displacementMap,k.displacementScale=I.displacementScale,k.displacementBias=I.displacementBias,k.wireframeLinewidth=I.wireframeLinewidth,k.linewidth=I.linewidth,_.isPointLight===!0&&k.isMeshDistanceMaterial===!0){const b=i.properties.get(k);b.light=_}return k}function y(T,I,_,A,k){if(T.visible===!1)return;if(T.layers.test(I.layers)&&(T.isMesh||T.isLine||T.isPoints)&&(T.castShadow||T.receiveShadow&&k===Lr)&&(!T.frustumCulled||n.intersectsObject(T))){T.modelViewMatrix.multiplyMatrices(_.matrixWorldInverse,T.matrixWorld);const P=e.update(T),W=T.material;if(Array.isArray(W)){const N=P.groups;for(let D=0,G=N.length;D<G;D++){const H=N[D],ne=W[H.materialIndex];if(ne&&ne.visible){const ce=C(T,ne,A,k);T.onBeforeShadow(i,T,I,_,P,ce,H),i.renderBufferDirect(_,null,P,ce,T,H),T.onAfterShadow(i,T,I,_,P,ce,H)}}}else if(W.visible){const N=C(T,W,A,k);T.onBeforeShadow(i,T,I,_,P,N,null),i.renderBufferDirect(_,null,P,N,T,null),T.onAfterShadow(i,T,I,_,P,N,null)}}const b=T.children;for(let P=0,W=b.length;P<W;P++)y(b[P],I,_,A,k)}function w(T){T.target.removeEventListener("dispose",w);for(const _ in l){const A=l[_],k=T.target.uuid;k in A&&(A[k].dispose(),delete A[k])}}}function fg(i,e){function t(){let U=!1;const ge=new Ot;let ie=null;const xe=new Ot(0,0,0,0);return{setMask:function(ae){ie!==ae&&!U&&(i.colorMask(ae,ae,ae,ae),ie=ae)},setLocked:function(ae){U=ae},setClear:function(ae,se,De,Me,At){At===!0&&(ae*=Me,se*=Me,De*=Me),ge.set(ae,se,De,Me),xe.equals(ge)===!1&&(i.clearColor(ae,se,De,Me),xe.copy(ge))},reset:function(){U=!1,ie=null,xe.set(-1,0,0,0)}}}function n(){let U=!1,ge=!1,ie=null,xe=null,ae=null;return{setReversed:function(se){if(ge!==se){const De=e.get("EXT_clip_control");se?De.clipControlEXT(De.LOWER_LEFT_EXT,De.ZERO_TO_ONE_EXT):De.clipControlEXT(De.LOWER_LEFT_EXT,De.NEGATIVE_ONE_TO_ONE_EXT),ge=se;const Me=ae;ae=null,this.setClear(Me)}},getReversed:function(){return ge},setTest:function(se){se?le(i.DEPTH_TEST):Oe(i.DEPTH_TEST)},setMask:function(se){ie!==se&&!U&&(i.depthMask(se),ie=se)},setFunc:function(se){if(ge&&(se=qd[se]),xe!==se){switch(se){case Ra:i.depthFunc(i.NEVER);break;case Ca:i.depthFunc(i.ALWAYS);break;case Pa:i.depthFunc(i.LESS);break;case pr:i.depthFunc(i.LEQUAL);break;case La:i.depthFunc(i.EQUAL);break;case Da:i.depthFunc(i.GEQUAL);break;case Ia:i.depthFunc(i.GREATER);break;case Ua:i.depthFunc(i.NOTEQUAL);break;default:i.depthFunc(i.LEQUAL)}xe=se}},setLocked:function(se){U=se},setClear:function(se){ae!==se&&(ae=se,ge&&(se=1-se),i.clearDepth(se))},reset:function(){U=!1,ie=null,xe=null,ae=null,ge=!1}}}function r(){let U=!1,ge=null,ie=null,xe=null,ae=null,se=null,De=null,Me=null,At=null;return{setTest:function(dt){U||(dt?le(i.STENCIL_TEST):Oe(i.STENCIL_TEST))},setMask:function(dt){ge!==dt&&!U&&(i.stencilMask(dt),ge=dt)},setFunc:function(dt,gn,dn){(ie!==dt||xe!==gn||ae!==dn)&&(i.stencilFunc(dt,gn,dn),ie=dt,xe=gn,ae=dn)},setOp:function(dt,gn,dn){(se!==dt||De!==gn||Me!==dn)&&(i.stencilOp(dt,gn,dn),se=dt,De=gn,Me=dn)},setLocked:function(dt){U=dt},setClear:function(dt){At!==dt&&(i.clearStencil(dt),At=dt)},reset:function(){U=!1,ge=null,ie=null,xe=null,ae=null,se=null,De=null,Me=null,At=null}}}const s=new t,a=new n,o=new r,c=new WeakMap,l=new WeakMap;let u={},f={},d={},m=new WeakMap,x=[],S=null,g=!1,h=null,R=null,C=null,y=null,w=null,T=null,I=null,_=new Tt(0,0,0),A=0,k=!1,F=null,b=null,P=null,W=null,N=null;const D=i.getParameter(i.MAX_COMBINED_TEXTURE_IMAGE_UNITS);let G=!1,H=0;const ne=i.getParameter(i.VERSION);ne.indexOf("WebGL")!==-1?(H=parseFloat(/^WebGL (\d)/.exec(ne)[1]),G=H>=1):ne.indexOf("OpenGL ES")!==-1&&(H=parseFloat(/^OpenGL ES (\d)/.exec(ne)[1]),G=H>=2);let ce=null,oe={};const _e=i.getParameter(i.SCISSOR_BOX),Qe=i.getParameter(i.VIEWPORT),Ze=new Ot().fromArray(_e),He=new Ot().fromArray(Qe);function Z(U,ge,ie,xe){const ae=new Uint8Array(4),se=i.createTexture();i.bindTexture(U,se),i.texParameteri(U,i.TEXTURE_MIN_FILTER,i.NEAREST),i.texParameteri(U,i.TEXTURE_MAG_FILTER,i.NEAREST);for(let De=0;De<ie;De++)U===i.TEXTURE_3D||U===i.TEXTURE_2D_ARRAY?i.texImage3D(ge,0,i.RGBA,1,1,xe,0,i.RGBA,i.UNSIGNED_BYTE,ae):i.texImage2D(ge+De,0,i.RGBA,1,1,0,i.RGBA,i.UNSIGNED_BYTE,ae);return se}const de={};de[i.TEXTURE_2D]=Z(i.TEXTURE_2D,i.TEXTURE_2D,1),de[i.TEXTURE_CUBE_MAP]=Z(i.TEXTURE_CUBE_MAP,i.TEXTURE_CUBE_MAP_POSITIVE_X,6),de[i.TEXTURE_2D_ARRAY]=Z(i.TEXTURE_2D_ARRAY,i.TEXTURE_2D_ARRAY,1,1),de[i.TEXTURE_3D]=Z(i.TEXTURE_3D,i.TEXTURE_3D,1,1),s.setClear(0,0,0,1),a.setClear(1),o.setClear(0),le(i.DEPTH_TEST),a.setFunc(pr),We(!1),qe(Vo),le(i.CULL_FACE),Se(si);function le(U){u[U]!==!0&&(i.enable(U),u[U]=!0)}function Oe(U){u[U]!==!1&&(i.disable(U),u[U]=!1)}function Je(U,ge){return d[U]!==ge?(i.bindFramebuffer(U,ge),d[U]=ge,U===i.DRAW_FRAMEBUFFER&&(d[i.FRAMEBUFFER]=ge),U===i.FRAMEBUFFER&&(d[i.DRAW_FRAMEBUFFER]=ge),!0):!1}function Te(U,ge){let ie=x,xe=!1;if(U){ie=m.get(ge),ie===void 0&&(ie=[],m.set(ge,ie));const ae=U.textures;if(ie.length!==ae.length||ie[0]!==i.COLOR_ATTACHMENT0){for(let se=0,De=ae.length;se<De;se++)ie[se]=i.COLOR_ATTACHMENT0+se;ie.length=ae.length,xe=!0}}else ie[0]!==i.BACK&&(ie[0]=i.BACK,xe=!0);xe&&i.drawBuffers(ie)}function Q(U){return S!==U?(i.useProgram(U),S=U,!0):!1}const te={[vi]:i.FUNC_ADD,[gd]:i.FUNC_SUBTRACT,[_d]:i.FUNC_REVERSE_SUBTRACT};te[xd]=i.MIN,te[vd]=i.MAX;const pe={[Md]:i.ZERO,[Aa]:i.ONE,[Sd]:i.SRC_COLOR,[wa]:i.SRC_ALPHA,[Ad]:i.SRC_ALPHA_SATURATE,[Ed]:i.DST_COLOR,[yd]:i.DST_ALPHA,[Zl]:i.ONE_MINUS_SRC_COLOR,[Ur]:i.ONE_MINUS_SRC_ALPHA,[Td]:i.ONE_MINUS_DST_COLOR,[bd]:i.ONE_MINUS_DST_ALPHA,[wd]:i.CONSTANT_COLOR,[Rd]:i.ONE_MINUS_CONSTANT_COLOR,[Cd]:i.CONSTANT_ALPHA,[Pd]:i.ONE_MINUS_CONSTANT_ALPHA};function Se(U,ge,ie,xe,ae,se,De,Me,At,dt){if(U===si){g===!0&&(Oe(i.BLEND),g=!1);return}if(g===!1&&(le(i.BLEND),g=!0),U!==Kl){if(U!==h||dt!==k){if((R!==vi||w!==vi)&&(i.blendEquation(i.FUNC_ADD),R=vi,w=vi),dt)switch(U){case ur:i.blendFuncSeparate(i.ONE,i.ONE_MINUS_SRC_ALPHA,i.ONE,i.ONE_MINUS_SRC_ALPHA);break;case Ho:i.blendFunc(i.ONE,i.ONE);break;case Wo:i.blendFuncSeparate(i.ZERO,i.ONE_MINUS_SRC_COLOR,i.ZERO,i.ONE);break;case Xo:i.blendFuncSeparate(i.DST_COLOR,i.ONE_MINUS_SRC_ALPHA,i.ZERO,i.ONE);break;default:_t("WebGLState: Invalid blending: ",U);break}else switch(U){case ur:i.blendFuncSeparate(i.SRC_ALPHA,i.ONE_MINUS_SRC_ALPHA,i.ONE,i.ONE_MINUS_SRC_ALPHA);break;case Ho:i.blendFuncSeparate(i.SRC_ALPHA,i.ONE,i.ONE,i.ONE);break;case Wo:_t("WebGLState: SubtractiveBlending requires material.premultipliedAlpha = true");break;case Xo:_t("WebGLState: MultiplyBlending requires material.premultipliedAlpha = true");break;default:_t("WebGLState: Invalid blending: ",U);break}C=null,y=null,T=null,I=null,_.set(0,0,0),A=0,h=U,k=dt}return}ae=ae||ge,se=se||ie,De=De||xe,(ge!==R||ae!==w)&&(i.blendEquationSeparate(te[ge],te[ae]),R=ge,w=ae),(ie!==C||xe!==y||se!==T||De!==I)&&(i.blendFuncSeparate(pe[ie],pe[xe],pe[se],pe[De]),C=ie,y=xe,T=se,I=De),(Me.equals(_)===!1||At!==A)&&(i.blendColor(Me.r,Me.g,Me.b,At),_.copy(Me),A=At),h=U,k=!1}function Ce(U,ge){U.side===ni?Oe(i.CULL_FACE):le(i.CULL_FACE);let ie=U.side===cn;ge&&(ie=!ie),We(ie),U.blending===ur&&U.transparent===!1?Se(si):Se(U.blending,U.blendEquation,U.blendSrc,U.blendDst,U.blendEquationAlpha,U.blendSrcAlpha,U.blendDstAlpha,U.blendColor,U.blendAlpha,U.premultipliedAlpha),a.setFunc(U.depthFunc),a.setTest(U.depthTest),a.setMask(U.depthWrite),s.setMask(U.colorWrite);const xe=U.stencilWrite;o.setTest(xe),xe&&(o.setMask(U.stencilWriteMask),o.setFunc(U.stencilFunc,U.stencilRef,U.stencilFuncMask),o.setOp(U.stencilFail,U.stencilZFail,U.stencilZPass)),ot(U.polygonOffset,U.polygonOffsetFactor,U.polygonOffsetUnits),U.alphaToCoverage===!0?le(i.SAMPLE_ALPHA_TO_COVERAGE):Oe(i.SAMPLE_ALPHA_TO_COVERAGE)}function We(U){F!==U&&(U?i.frontFace(i.CW):i.frontFace(i.CCW),F=U)}function qe(U){U!==fd?(le(i.CULL_FACE),U!==b&&(U===Vo?i.cullFace(i.BACK):U===pd?i.cullFace(i.FRONT):i.cullFace(i.FRONT_AND_BACK))):Oe(i.CULL_FACE),b=U}function ut(U){U!==P&&(G&&i.lineWidth(U),P=U)}function ot(U,ge,ie){U?(le(i.POLYGON_OFFSET_FILL),(W!==ge||N!==ie)&&(W=ge,N=ie,a.getReversed()&&(ge=-ge),i.polygonOffset(ge,ie))):Oe(i.POLYGON_OFFSET_FILL)}function Le(U){U?le(i.SCISSOR_TEST):Oe(i.SCISSOR_TEST)}function et(U){U===void 0&&(U=i.TEXTURE0+D-1),ce!==U&&(i.activeTexture(U),ce=U)}function L(U,ge,ie){ie===void 0&&(ce===null?ie=i.TEXTURE0+D-1:ie=ce);let xe=oe[ie];xe===void 0&&(xe={type:void 0,texture:void 0},oe[ie]=xe),(xe.type!==U||xe.texture!==ge)&&(ce!==ie&&(i.activeTexture(ie),ce=ie),i.bindTexture(U,ge||de[U]),xe.type=U,xe.texture=ge)}function ht(){const U=oe[ce];U!==void 0&&U.type!==void 0&&(i.bindTexture(U.type,null),U.type=void 0,U.texture=void 0)}function $e(){try{i.compressedTexImage2D(...arguments)}catch(U){_t("WebGLState:",U)}}function E(){try{i.compressedTexImage3D(...arguments)}catch(U){_t("WebGLState:",U)}}function p(){try{i.texSubImage2D(...arguments)}catch(U){_t("WebGLState:",U)}}function z(){try{i.texSubImage3D(...arguments)}catch(U){_t("WebGLState:",U)}}function q(){try{i.compressedTexSubImage2D(...arguments)}catch(U){_t("WebGLState:",U)}}function J(){try{i.compressedTexSubImage3D(...arguments)}catch(U){_t("WebGLState:",U)}}function me(){try{i.texStorage2D(...arguments)}catch(U){_t("WebGLState:",U)}}function ue(){try{i.texStorage3D(...arguments)}catch(U){_t("WebGLState:",U)}}function j(){try{i.texImage2D(...arguments)}catch(U){_t("WebGLState:",U)}}function ee(){try{i.texImage3D(...arguments)}catch(U){_t("WebGLState:",U)}}function fe(U){return f[U]!==void 0?f[U]:i.getParameter(U)}function Pe(U,ge){f[U]!==ge&&(i.pixelStorei(U,ge),f[U]=ge)}function he(U){Ze.equals(U)===!1&&(i.scissor(U.x,U.y,U.z,U.w),Ze.copy(U))}function ve(U){He.equals(U)===!1&&(i.viewport(U.x,U.y,U.z,U.w),He.copy(U))}function Be(U,ge){let ie=l.get(ge);ie===void 0&&(ie=new WeakMap,l.set(ge,ie));let xe=ie.get(U);xe===void 0&&(xe=i.getUniformBlockIndex(ge,U.name),ie.set(U,xe))}function Ge(U,ge){const xe=l.get(ge).get(U);c.get(ge)!==xe&&(i.uniformBlockBinding(ge,xe,U.__bindingPointIndex),c.set(ge,xe))}function tt(){i.disable(i.BLEND),i.disable(i.CULL_FACE),i.disable(i.DEPTH_TEST),i.disable(i.POLYGON_OFFSET_FILL),i.disable(i.SCISSOR_TEST),i.disable(i.STENCIL_TEST),i.disable(i.SAMPLE_ALPHA_TO_COVERAGE),i.blendEquation(i.FUNC_ADD),i.blendFunc(i.ONE,i.ZERO),i.blendFuncSeparate(i.ONE,i.ZERO,i.ONE,i.ZERO),i.blendColor(0,0,0,0),i.colorMask(!0,!0,!0,!0),i.clearColor(0,0,0,0),i.depthMask(!0),i.depthFunc(i.LESS),a.setReversed(!1),i.clearDepth(1),i.stencilMask(4294967295),i.stencilFunc(i.ALWAYS,0,4294967295),i.stencilOp(i.KEEP,i.KEEP,i.KEEP),i.clearStencil(0),i.cullFace(i.BACK),i.frontFace(i.CCW),i.polygonOffset(0,0),i.activeTexture(i.TEXTURE0),i.bindFramebuffer(i.FRAMEBUFFER,null),i.bindFramebuffer(i.DRAW_FRAMEBUFFER,null),i.bindFramebuffer(i.READ_FRAMEBUFFER,null),i.useProgram(null),i.lineWidth(1),i.scissor(0,0,i.canvas.width,i.canvas.height),i.viewport(0,0,i.canvas.width,i.canvas.height),i.pixelStorei(i.PACK_ALIGNMENT,4),i.pixelStorei(i.UNPACK_ALIGNMENT,4),i.pixelStorei(i.UNPACK_FLIP_Y_WEBGL,!1),i.pixelStorei(i.UNPACK_PREMULTIPLY_ALPHA_WEBGL,!1),i.pixelStorei(i.UNPACK_COLORSPACE_CONVERSION_WEBGL,i.BROWSER_DEFAULT_WEBGL),i.pixelStorei(i.PACK_ROW_LENGTH,0),i.pixelStorei(i.PACK_SKIP_PIXELS,0),i.pixelStorei(i.PACK_SKIP_ROWS,0),i.pixelStorei(i.UNPACK_ROW_LENGTH,0),i.pixelStorei(i.UNPACK_IMAGE_HEIGHT,0),i.pixelStorei(i.UNPACK_SKIP_PIXELS,0),i.pixelStorei(i.UNPACK_SKIP_ROWS,0),i.pixelStorei(i.UNPACK_SKIP_IMAGES,0),u={},f={},ce=null,oe={},d={},m=new WeakMap,x=[],S=null,g=!1,h=null,R=null,C=null,y=null,w=null,T=null,I=null,_=new Tt(0,0,0),A=0,k=!1,F=null,b=null,P=null,W=null,N=null,Ze.set(0,0,i.canvas.width,i.canvas.height),He.set(0,0,i.canvas.width,i.canvas.height),s.reset(),a.reset(),o.reset()}return{buffers:{color:s,depth:a,stencil:o},enable:le,disable:Oe,bindFramebuffer:Je,drawBuffers:Te,useProgram:Q,setBlending:Se,setMaterial:Ce,setFlipSided:We,setCullFace:qe,setLineWidth:ut,setPolygonOffset:ot,setScissorTest:Le,activeTexture:et,bindTexture:L,unbindTexture:ht,compressedTexImage2D:$e,compressedTexImage3D:E,texImage2D:j,texImage3D:ee,pixelStorei:Pe,getParameter:fe,updateUBOMapping:Be,uniformBlockBinding:Ge,texStorage2D:me,texStorage3D:ue,texSubImage2D:p,texSubImage3D:z,compressedTexSubImage2D:q,compressedTexSubImage3D:J,scissor:he,viewport:ve,reset:tt}}function pg(i,e,t,n,r,s,a){const o=e.has("WEBGL_multisampled_render_to_texture")?e.get("WEBGL_multisampled_render_to_texture"):null,c=typeof navigator>"u"?!1:/OculusBrowser/g.test(navigator.userAgent),l=new Mt,u=new WeakMap,f=new Set;let d;const m=new WeakMap;let x=!1;try{x=typeof OffscreenCanvas<"u"&&new OffscreenCanvas(1,1).getContext("2d")!==null}catch{}function S(E,p){return x?new OffscreenCanvas(E,p):Cs("canvas")}function g(E,p,z){let q=1;const J=$e(E);if((J.width>z||J.height>z)&&(q=z/Math.max(J.width,J.height)),q<1)if(typeof HTMLImageElement<"u"&&E instanceof HTMLImageElement||typeof HTMLCanvasElement<"u"&&E instanceof HTMLCanvasElement||typeof ImageBitmap<"u"&&E instanceof ImageBitmap||typeof VideoFrame<"u"&&E instanceof VideoFrame){const me=Math.floor(q*J.width),ue=Math.floor(q*J.height);d===void 0&&(d=S(me,ue));const j=p?S(me,ue):d;return j.width=me,j.height=ue,j.getContext("2d").drawImage(E,0,0,me,ue),je("WebGLRenderer: Texture has been resized from ("+J.width+"x"+J.height+") to ("+me+"x"+ue+")."),j}else return"data"in E&&je("WebGLRenderer: Image in DataTexture is too big ("+J.width+"x"+J.height+")."),E;return E}function h(E){return E.generateMipmaps}function R(E){i.generateMipmap(E)}function C(E){return E.isWebGLCubeRenderTarget?i.TEXTURE_CUBE_MAP:E.isWebGL3DRenderTarget?i.TEXTURE_3D:E.isWebGLArrayRenderTarget||E.isCompressedArrayTexture?i.TEXTURE_2D_ARRAY:i.TEXTURE_2D}function y(E,p,z,q,J,me=!1){if(E!==null){if(i[E]!==void 0)return i[E];je("WebGLRenderer: Attempt to use non-existing WebGL internal format '"+E+"'")}let ue;q&&(ue=e.get("EXT_texture_norm16"),ue||je("WebGLRenderer: Unable to use normalized textures without EXT_texture_norm16 extension"));let j=p;if(p===i.RED&&(z===i.FLOAT&&(j=i.R32F),z===i.HALF_FLOAT&&(j=i.R16F),z===i.UNSIGNED_BYTE&&(j=i.R8),z===i.UNSIGNED_SHORT&&ue&&(j=ue.R16_EXT),z===i.SHORT&&ue&&(j=ue.R16_SNORM_EXT)),p===i.RED_INTEGER&&(z===i.UNSIGNED_BYTE&&(j=i.R8UI),z===i.UNSIGNED_SHORT&&(j=i.R16UI),z===i.UNSIGNED_INT&&(j=i.R32UI),z===i.BYTE&&(j=i.R8I),z===i.SHORT&&(j=i.R16I),z===i.INT&&(j=i.R32I)),p===i.RG&&(z===i.FLOAT&&(j=i.RG32F),z===i.HALF_FLOAT&&(j=i.RG16F),z===i.UNSIGNED_BYTE&&(j=i.RG8),z===i.UNSIGNED_SHORT&&ue&&(j=ue.RG16_EXT),z===i.SHORT&&ue&&(j=ue.RG16_SNORM_EXT)),p===i.RG_INTEGER&&(z===i.UNSIGNED_BYTE&&(j=i.RG8UI),z===i.UNSIGNED_SHORT&&(j=i.RG16UI),z===i.UNSIGNED_INT&&(j=i.RG32UI),z===i.BYTE&&(j=i.RG8I),z===i.SHORT&&(j=i.RG16I),z===i.INT&&(j=i.RG32I)),p===i.RGB_INTEGER&&(z===i.UNSIGNED_BYTE&&(j=i.RGB8UI),z===i.UNSIGNED_SHORT&&(j=i.RGB16UI),z===i.UNSIGNED_INT&&(j=i.RGB32UI),z===i.BYTE&&(j=i.RGB8I),z===i.SHORT&&(j=i.RGB16I),z===i.INT&&(j=i.RGB32I)),p===i.RGBA_INTEGER&&(z===i.UNSIGNED_BYTE&&(j=i.RGBA8UI),z===i.UNSIGNED_SHORT&&(j=i.RGBA16UI),z===i.UNSIGNED_INT&&(j=i.RGBA32UI),z===i.BYTE&&(j=i.RGBA8I),z===i.SHORT&&(j=i.RGBA16I),z===i.INT&&(j=i.RGBA32I)),p===i.RGB&&(z===i.UNSIGNED_SHORT&&ue&&(j=ue.RGB16_EXT),z===i.SHORT&&ue&&(j=ue.RGB16_SNORM_EXT),z===i.UNSIGNED_INT_5_9_9_9_REV&&(j=i.RGB9_E5),z===i.UNSIGNED_INT_10F_11F_11F_REV&&(j=i.R11F_G11F_B10F)),p===i.RGBA){const ee=me?ws:mt.getTransfer(J);z===i.FLOAT&&(j=i.RGBA32F),z===i.HALF_FLOAT&&(j=i.RGBA16F),z===i.UNSIGNED_BYTE&&(j=ee===Ct?i.SRGB8_ALPHA8:i.RGBA8),z===i.UNSIGNED_SHORT&&ue&&(j=ue.RGBA16_EXT),z===i.SHORT&&ue&&(j=ue.RGBA16_SNORM_EXT),z===i.UNSIGNED_SHORT_4_4_4_4&&(j=i.RGBA4),z===i.UNSIGNED_SHORT_5_5_5_1&&(j=i.RGB5_A1)}return(j===i.R16F||j===i.R32F||j===i.RG16F||j===i.RG32F||j===i.RGBA16F||j===i.RGBA32F)&&e.get("EXT_color_buffer_float"),j}function w(E,p){let z;return E?p===null||p===Xn||p===Fr?z=i.DEPTH24_STENCIL8:p===Vn?z=i.DEPTH32F_STENCIL8:p===Nr&&(z=i.DEPTH24_STENCIL8,je("DepthTexture: 16 bit depth attachment is not supported with stencil. Using 24-bit attachment.")):p===null||p===Xn||p===Fr?z=i.DEPTH_COMPONENT24:p===Vn?z=i.DEPTH_COMPONENT32F:p===Nr&&(z=i.DEPTH_COMPONENT16),z}function T(E,p){return h(E)===!0||E.isFramebufferTexture&&E.minFilter!==Jt&&E.minFilter!==Qt?Math.log2(Math.max(p.width,p.height))+1:E.mipmaps!==void 0&&E.mipmaps.length>0?E.mipmaps.length:E.isCompressedTexture&&Array.isArray(E.image)?p.mipmaps.length:1}function I(E){const p=E.target;p.removeEventListener("dispose",I),A(p),p.isVideoTexture&&u.delete(p),p.isHTMLTexture&&f.delete(p)}function _(E){const p=E.target;p.removeEventListener("dispose",_),F(p)}function A(E){const p=n.get(E);if(p.__webglInit===void 0)return;const z=E.source,q=m.get(z);if(q){const J=q[p.__cacheKey];J.usedTimes--,J.usedTimes===0&&k(E),Object.keys(q).length===0&&m.delete(z)}n.remove(E)}function k(E){const p=n.get(E);i.deleteTexture(p.__webglTexture);const z=E.source,q=m.get(z);delete q[p.__cacheKey],a.memory.textures--}function F(E){const p=n.get(E);if(E.depthTexture&&(E.depthTexture.dispose(),n.remove(E.depthTexture)),E.isWebGLCubeRenderTarget)for(let q=0;q<6;q++){if(Array.isArray(p.__webglFramebuffer[q]))for(let J=0;J<p.__webglFramebuffer[q].length;J++)i.deleteFramebuffer(p.__webglFramebuffer[q][J]);else i.deleteFramebuffer(p.__webglFramebuffer[q]);p.__webglDepthbuffer&&i.deleteRenderbuffer(p.__webglDepthbuffer[q])}else{if(Array.isArray(p.__webglFramebuffer))for(let q=0;q<p.__webglFramebuffer.length;q++)i.deleteFramebuffer(p.__webglFramebuffer[q]);else i.deleteFramebuffer(p.__webglFramebuffer);if(p.__webglDepthbuffer&&i.deleteRenderbuffer(p.__webglDepthbuffer),p.__webglMultisampledFramebuffer&&i.deleteFramebuffer(p.__webglMultisampledFramebuffer),p.__webglColorRenderbuffer)for(let q=0;q<p.__webglColorRenderbuffer.length;q++)p.__webglColorRenderbuffer[q]&&i.deleteRenderbuffer(p.__webglColorRenderbuffer[q]);p.__webglDepthRenderbuffer&&i.deleteRenderbuffer(p.__webglDepthRenderbuffer)}const z=E.textures;for(let q=0,J=z.length;q<J;q++){const me=n.get(z[q]);me.__webglTexture&&(i.deleteTexture(me.__webglTexture),a.memory.textures--),n.remove(z[q])}n.remove(E)}let b=0;function P(){b=0}function W(){return b}function N(E){b=E}function D(){const E=b;return E>=r.maxTextures&&je("WebGLTextures: Trying to use "+E+" texture units while this GPU supports only "+r.maxTextures),b+=1,E}function G(E){const p=[];return p.push(E.wrapS),p.push(E.wrapT),p.push(E.wrapR||0),p.push(E.magFilter),p.push(E.minFilter),p.push(E.anisotropy),p.push(E.internalFormat),p.push(E.format),p.push(E.type),p.push(E.generateMipmaps),p.push(E.premultiplyAlpha),p.push(E.flipY),p.push(E.unpackAlignment),p.push(E.colorSpace),p.join()}function H(E,p){const z=n.get(E);if(E.isVideoTexture&&L(E),E.isRenderTargetTexture===!1&&E.isExternalTexture!==!0&&E.version>0&&z.__version!==E.version){const q=E.image;if(q===null)je("WebGLRenderer: Texture marked for update but no image data found.");else if(q.complete===!1)je("WebGLRenderer: Texture marked for update but image is incomplete");else{Oe(z,E,p);return}}else E.isExternalTexture&&(z.__webglTexture=E.sourceTexture?E.sourceTexture:null);t.bindTexture(i.TEXTURE_2D,z.__webglTexture,i.TEXTURE0+p)}function ne(E,p){const z=n.get(E);if(E.isRenderTargetTexture===!1&&E.version>0&&z.__version!==E.version){Oe(z,E,p);return}else E.isExternalTexture&&(z.__webglTexture=E.sourceTexture?E.sourceTexture:null);t.bindTexture(i.TEXTURE_2D_ARRAY,z.__webglTexture,i.TEXTURE0+p)}function ce(E,p){const z=n.get(E);if(E.isRenderTargetTexture===!1&&E.version>0&&z.__version!==E.version){Oe(z,E,p);return}t.bindTexture(i.TEXTURE_3D,z.__webglTexture,i.TEXTURE0+p)}function oe(E,p){const z=n.get(E);if(E.isCubeDepthTexture!==!0&&E.version>0&&z.__version!==E.version){Je(z,E,p);return}t.bindTexture(i.TEXTURE_CUBE_MAP,z.__webglTexture,i.TEXTURE0+p)}const _e={[Na]:i.REPEAT,[ri]:i.CLAMP_TO_EDGE,[Fa]:i.MIRRORED_REPEAT},Qe={[Jt]:i.NEAREST,[Id]:i.NEAREST_MIPMAP_NEAREST,[Xr]:i.NEAREST_MIPMAP_LINEAR,[Qt]:i.LINEAR,[qs]:i.LINEAR_MIPMAP_NEAREST,[Ni]:i.LINEAR_MIPMAP_LINEAR},Ze={[Fd]:i.NEVER,[Gd]:i.ALWAYS,[Od]:i.LESS,[Ro]:i.LEQUAL,[Bd]:i.EQUAL,[Co]:i.GEQUAL,[zd]:i.GREATER,[kd]:i.NOTEQUAL};function He(E,p){if(p.type===Vn&&e.has("OES_texture_float_linear")===!1&&(p.magFilter===Qt||p.magFilter===qs||p.magFilter===Xr||p.magFilter===Ni||p.minFilter===Qt||p.minFilter===qs||p.minFilter===Xr||p.minFilter===Ni)&&je("WebGLRenderer: Unable to use linear filtering with floating point textures. OES_texture_float_linear not supported on this device."),i.texParameteri(E,i.TEXTURE_WRAP_S,_e[p.wrapS]),i.texParameteri(E,i.TEXTURE_WRAP_T,_e[p.wrapT]),(E===i.TEXTURE_3D||E===i.TEXTURE_2D_ARRAY)&&i.texParameteri(E,i.TEXTURE_WRAP_R,_e[p.wrapR]),i.texParameteri(E,i.TEXTURE_MAG_FILTER,Qe[p.magFilter]),i.texParameteri(E,i.TEXTURE_MIN_FILTER,Qe[p.minFilter]),p.compareFunction&&(i.texParameteri(E,i.TEXTURE_COMPARE_MODE,i.COMPARE_REF_TO_TEXTURE),i.texParameteri(E,i.TEXTURE_COMPARE_FUNC,Ze[p.compareFunction])),e.has("EXT_texture_filter_anisotropic")===!0){if(p.magFilter===Jt||p.minFilter!==Xr&&p.minFilter!==Ni||p.type===Vn&&e.has("OES_texture_float_linear")===!1)return;if(p.anisotropy>1||n.get(p).__currentAnisotropy){const z=e.get("EXT_texture_filter_anisotropic");i.texParameterf(E,z.TEXTURE_MAX_ANISOTROPY_EXT,Math.min(p.anisotropy,r.getMaxAnisotropy())),n.get(p).__currentAnisotropy=p.anisotropy}}}function Z(E,p){let z=!1;E.__webglInit===void 0&&(E.__webglInit=!0,p.addEventListener("dispose",I));const q=p.source;let J=m.get(q);J===void 0&&(J={},m.set(q,J));const me=G(p);if(me!==E.__cacheKey){J[me]===void 0&&(J[me]={texture:i.createTexture(),usedTimes:0},a.memory.textures++,z=!0),J[me].usedTimes++;const ue=J[E.__cacheKey];ue!==void 0&&(J[E.__cacheKey].usedTimes--,ue.usedTimes===0&&k(p)),E.__cacheKey=me,E.__webglTexture=J[me].texture}return z}function de(E,p,z){return Math.floor(Math.floor(E/z)/p)}function le(E,p,z,q){const me=E.updateRanges;if(me.length===0)t.texSubImage2D(i.TEXTURE_2D,0,0,0,p.width,p.height,z,q,p.data);else{me.sort((Pe,he)=>Pe.start-he.start);let ue=0;for(let Pe=1;Pe<me.length;Pe++){const he=me[ue],ve=me[Pe],Be=he.start+he.count,Ge=de(ve.start,p.width,4),tt=de(he.start,p.width,4);ve.start<=Be+1&&Ge===tt&&de(ve.start+ve.count-1,p.width,4)===Ge?he.count=Math.max(he.count,ve.start+ve.count-he.start):(++ue,me[ue]=ve)}me.length=ue+1;const j=t.getParameter(i.UNPACK_ROW_LENGTH),ee=t.getParameter(i.UNPACK_SKIP_PIXELS),fe=t.getParameter(i.UNPACK_SKIP_ROWS);t.pixelStorei(i.UNPACK_ROW_LENGTH,p.width);for(let Pe=0,he=me.length;Pe<he;Pe++){const ve=me[Pe],Be=Math.floor(ve.start/4),Ge=Math.ceil(ve.count/4),tt=Be%p.width,U=Math.floor(Be/p.width),ge=Ge,ie=1;t.pixelStorei(i.UNPACK_SKIP_PIXELS,tt),t.pixelStorei(i.UNPACK_SKIP_ROWS,U),t.texSubImage2D(i.TEXTURE_2D,0,tt,U,ge,ie,z,q,p.data)}E.clearUpdateRanges(),t.pixelStorei(i.UNPACK_ROW_LENGTH,j),t.pixelStorei(i.UNPACK_SKIP_PIXELS,ee),t.pixelStorei(i.UNPACK_SKIP_ROWS,fe)}}function Oe(E,p,z){let q=i.TEXTURE_2D;(p.isDataArrayTexture||p.isCompressedArrayTexture)&&(q=i.TEXTURE_2D_ARRAY),p.isData3DTexture&&(q=i.TEXTURE_3D);const J=Z(E,p),me=p.source;t.bindTexture(q,E.__webglTexture,i.TEXTURE0+z);const ue=n.get(me);if(me.version!==ue.__version||J===!0){if(t.activeTexture(i.TEXTURE0+z),(typeof ImageBitmap<"u"&&p.image instanceof ImageBitmap)===!1){const ie=mt.getPrimaries(mt.workingColorSpace),xe=p.colorSpace===ii?null:mt.getPrimaries(p.colorSpace),ae=p.colorSpace===ii||ie===xe?i.NONE:i.BROWSER_DEFAULT_WEBGL;t.pixelStorei(i.UNPACK_FLIP_Y_WEBGL,p.flipY),t.pixelStorei(i.UNPACK_PREMULTIPLY_ALPHA_WEBGL,p.premultiplyAlpha),t.pixelStorei(i.UNPACK_COLORSPACE_CONVERSION_WEBGL,ae)}t.pixelStorei(i.UNPACK_ALIGNMENT,p.unpackAlignment);let ee=g(p.image,!1,r.maxTextureSize);ee=ht(p,ee);const fe=s.convert(p.format,p.colorSpace),Pe=s.convert(p.type);let he=y(p.internalFormat,fe,Pe,p.normalized,p.colorSpace,p.isVideoTexture);He(q,p);let ve;const Be=p.mipmaps,Ge=p.isVideoTexture!==!0,tt=ue.__version===void 0||J===!0,U=me.dataReady,ge=T(p,ee);if(p.isDepthTexture)he=w(p.format===Fi,p.type),tt&&(Ge?t.texStorage2D(i.TEXTURE_2D,1,he,ee.width,ee.height):t.texImage2D(i.TEXTURE_2D,0,he,ee.width,ee.height,0,fe,Pe,null));else if(p.isDataTexture)if(Be.length>0){Ge&&tt&&t.texStorage2D(i.TEXTURE_2D,ge,he,Be[0].width,Be[0].height);for(let ie=0,xe=Be.length;ie<xe;ie++)ve=Be[ie],Ge?U&&t.texSubImage2D(i.TEXTURE_2D,ie,0,0,ve.width,ve.height,fe,Pe,ve.data):t.texImage2D(i.TEXTURE_2D,ie,he,ve.width,ve.height,0,fe,Pe,ve.data);p.generateMipmaps=!1}else Ge?(tt&&t.texStorage2D(i.TEXTURE_2D,ge,he,ee.width,ee.height),U&&le(p,ee,fe,Pe)):t.texImage2D(i.TEXTURE_2D,0,he,ee.width,ee.height,0,fe,Pe,ee.data);else if(p.isCompressedTexture)if(p.isCompressedArrayTexture){Ge&&tt&&t.texStorage3D(i.TEXTURE_2D_ARRAY,ge,he,Be[0].width,Be[0].height,ee.depth);for(let ie=0,xe=Be.length;ie<xe;ie++)if(ve=Be[ie],p.format!==Cn)if(fe!==null)if(Ge){if(U)if(p.layerUpdates.size>0){const ae=fl(ve.width,ve.height,p.format,p.type);for(const se of p.layerUpdates){const De=ve.data.subarray(se*ae/ve.data.BYTES_PER_ELEMENT,(se+1)*ae/ve.data.BYTES_PER_ELEMENT);t.compressedTexSubImage3D(i.TEXTURE_2D_ARRAY,ie,0,0,se,ve.width,ve.height,1,fe,De)}p.clearLayerUpdates()}else t.compressedTexSubImage3D(i.TEXTURE_2D_ARRAY,ie,0,0,0,ve.width,ve.height,ee.depth,fe,ve.data)}else t.compressedTexImage3D(i.TEXTURE_2D_ARRAY,ie,he,ve.width,ve.height,ee.depth,0,ve.data,0,0);else je("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()");else Ge?U&&t.texSubImage3D(i.TEXTURE_2D_ARRAY,ie,0,0,0,ve.width,ve.height,ee.depth,fe,Pe,ve.data):t.texImage3D(i.TEXTURE_2D_ARRAY,ie,he,ve.width,ve.height,ee.depth,0,fe,Pe,ve.data)}else{Ge&&tt&&t.texStorage2D(i.TEXTURE_2D,ge,he,Be[0].width,Be[0].height);for(let ie=0,xe=Be.length;ie<xe;ie++)ve=Be[ie],p.format!==Cn?fe!==null?Ge?U&&t.compressedTexSubImage2D(i.TEXTURE_2D,ie,0,0,ve.width,ve.height,fe,ve.data):t.compressedTexImage2D(i.TEXTURE_2D,ie,he,ve.width,ve.height,0,ve.data):je("WebGLRenderer: Attempt to load unsupported compressed texture format in .uploadTexture()"):Ge?U&&t.texSubImage2D(i.TEXTURE_2D,ie,0,0,ve.width,ve.height,fe,Pe,ve.data):t.texImage2D(i.TEXTURE_2D,ie,he,ve.width,ve.height,0,fe,Pe,ve.data)}else if(p.isDataArrayTexture)if(Ge){if(tt&&t.texStorage3D(i.TEXTURE_2D_ARRAY,ge,he,ee.width,ee.height,ee.depth),U)if(p.layerUpdates.size>0){const ie=fl(ee.width,ee.height,p.format,p.type);for(const xe of p.layerUpdates){const ae=ee.data.subarray(xe*ie/ee.data.BYTES_PER_ELEMENT,(xe+1)*ie/ee.data.BYTES_PER_ELEMENT);t.texSubImage3D(i.TEXTURE_2D_ARRAY,0,0,0,xe,ee.width,ee.height,1,fe,Pe,ae)}p.clearLayerUpdates()}else t.texSubImage3D(i.TEXTURE_2D_ARRAY,0,0,0,0,ee.width,ee.height,ee.depth,fe,Pe,ee.data)}else t.texImage3D(i.TEXTURE_2D_ARRAY,0,he,ee.width,ee.height,ee.depth,0,fe,Pe,ee.data);else if(p.isData3DTexture)Ge?(tt&&t.texStorage3D(i.TEXTURE_3D,ge,he,ee.width,ee.height,ee.depth),U&&t.texSubImage3D(i.TEXTURE_3D,0,0,0,0,ee.width,ee.height,ee.depth,fe,Pe,ee.data)):t.texImage3D(i.TEXTURE_3D,0,he,ee.width,ee.height,ee.depth,0,fe,Pe,ee.data);else if(p.isFramebufferTexture){if(tt)if(Ge)t.texStorage2D(i.TEXTURE_2D,ge,he,ee.width,ee.height);else{let ie=ee.width,xe=ee.height;for(let ae=0;ae<ge;ae++)t.texImage2D(i.TEXTURE_2D,ae,he,ie,xe,0,fe,Pe,null),ie>>=1,xe>>=1}}else if(p.isHTMLTexture){if("texElementImage2D"in i){const ie=i.canvas;if(ie.hasAttribute("layoutsubtree")||ie.setAttribute("layoutsubtree","true"),ee.parentNode!==ie){ie.appendChild(ee),f.add(p),ie.onpaint=xe=>{const ae=xe.changedElements;for(const se of f)ae.includes(se.image)&&(se.needsUpdate=!0)},ie.requestPaint();return}if(i.texElementImage2D.length===3)i.texElementImage2D(i.TEXTURE_2D,i.RGBA8,ee);else{const ae=i.RGBA,se=i.RGBA,De=i.UNSIGNED_BYTE;i.texElementImage2D(i.TEXTURE_2D,0,ae,se,De,ee)}i.texParameteri(i.TEXTURE_2D,i.TEXTURE_MIN_FILTER,i.LINEAR),i.texParameteri(i.TEXTURE_2D,i.TEXTURE_WRAP_S,i.CLAMP_TO_EDGE),i.texParameteri(i.TEXTURE_2D,i.TEXTURE_WRAP_T,i.CLAMP_TO_EDGE)}}else if(Be.length>0){if(Ge&&tt){const ie=$e(Be[0]);t.texStorage2D(i.TEXTURE_2D,ge,he,ie.width,ie.height)}for(let ie=0,xe=Be.length;ie<xe;ie++)ve=Be[ie],Ge?U&&t.texSubImage2D(i.TEXTURE_2D,ie,0,0,fe,Pe,ve):t.texImage2D(i.TEXTURE_2D,ie,he,fe,Pe,ve);p.generateMipmaps=!1}else if(Ge){if(tt){const ie=$e(ee);t.texStorage2D(i.TEXTURE_2D,ge,he,ie.width,ie.height)}U&&t.texSubImage2D(i.TEXTURE_2D,0,0,0,fe,Pe,ee)}else t.texImage2D(i.TEXTURE_2D,0,he,fe,Pe,ee);h(p)&&R(q),ue.__version=me.version,p.onUpdate&&p.onUpdate(p)}E.__version=p.version}function Je(E,p,z){if(p.image.length!==6)return;const q=Z(E,p),J=p.source;t.bindTexture(i.TEXTURE_CUBE_MAP,E.__webglTexture,i.TEXTURE0+z);const me=n.get(J);if(J.version!==me.__version||q===!0){t.activeTexture(i.TEXTURE0+z);const ue=mt.getPrimaries(mt.workingColorSpace),j=p.colorSpace===ii?null:mt.getPrimaries(p.colorSpace),ee=p.colorSpace===ii||ue===j?i.NONE:i.BROWSER_DEFAULT_WEBGL;t.pixelStorei(i.UNPACK_FLIP_Y_WEBGL,p.flipY),t.pixelStorei(i.UNPACK_PREMULTIPLY_ALPHA_WEBGL,p.premultiplyAlpha),t.pixelStorei(i.UNPACK_ALIGNMENT,p.unpackAlignment),t.pixelStorei(i.UNPACK_COLORSPACE_CONVERSION_WEBGL,ee);const fe=p.isCompressedTexture||p.image[0].isCompressedTexture,Pe=p.image[0]&&p.image[0].isDataTexture,he=[];for(let se=0;se<6;se++)!fe&&!Pe?he[se]=g(p.image[se],!0,r.maxCubemapSize):he[se]=Pe?p.image[se].image:p.image[se],he[se]=ht(p,he[se]);const ve=he[0],Be=s.convert(p.format,p.colorSpace),Ge=s.convert(p.type),tt=y(p.internalFormat,Be,Ge,p.normalized,p.colorSpace),U=p.isVideoTexture!==!0,ge=me.__version===void 0||q===!0,ie=J.dataReady;let xe=T(p,ve);He(i.TEXTURE_CUBE_MAP,p);let ae;if(fe){U&&ge&&t.texStorage2D(i.TEXTURE_CUBE_MAP,xe,tt,ve.width,ve.height);for(let se=0;se<6;se++){ae=he[se].mipmaps;for(let De=0;De<ae.length;De++){const Me=ae[De];p.format!==Cn?Be!==null?U?ie&&t.compressedTexSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De,0,0,Me.width,Me.height,Be,Me.data):t.compressedTexImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De,tt,Me.width,Me.height,0,Me.data):je("WebGLRenderer: Attempt to load unsupported compressed texture format in .setTextureCube()"):U?ie&&t.texSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De,0,0,Me.width,Me.height,Be,Ge,Me.data):t.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De,tt,Me.width,Me.height,0,Be,Ge,Me.data)}}}else{if(ae=p.mipmaps,U&&ge){ae.length>0&&xe++;const se=$e(he[0]);t.texStorage2D(i.TEXTURE_CUBE_MAP,xe,tt,se.width,se.height)}for(let se=0;se<6;se++)if(Pe){U?ie&&t.texSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,0,0,0,he[se].width,he[se].height,Be,Ge,he[se].data):t.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,0,tt,he[se].width,he[se].height,0,Be,Ge,he[se].data);for(let De=0;De<ae.length;De++){const At=ae[De].image[se].image;U?ie&&t.texSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De+1,0,0,At.width,At.height,Be,Ge,At.data):t.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De+1,tt,At.width,At.height,0,Be,Ge,At.data)}}else{U?ie&&t.texSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,0,0,0,Be,Ge,he[se]):t.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,0,tt,Be,Ge,he[se]);for(let De=0;De<ae.length;De++){const Me=ae[De];U?ie&&t.texSubImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De+1,0,0,Be,Ge,Me.image[se]):t.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+se,De+1,tt,Be,Ge,Me.image[se])}}}h(p)&&R(i.TEXTURE_CUBE_MAP),me.__version=J.version,p.onUpdate&&p.onUpdate(p)}E.__version=p.version}function Te(E,p,z,q,J,me){const ue=s.convert(z.format,z.colorSpace),j=s.convert(z.type),ee=y(z.internalFormat,ue,j,z.normalized,z.colorSpace),fe=n.get(p),Pe=n.get(z);if(Pe.__renderTarget=p,!fe.__hasExternalTextures){const he=Math.max(1,p.width>>me),ve=Math.max(1,p.height>>me);J===i.TEXTURE_3D||J===i.TEXTURE_2D_ARRAY?t.texImage3D(J,me,ee,he,ve,p.depth,0,ue,j,null):t.texImage2D(J,me,ee,he,ve,0,ue,j,null)}t.bindFramebuffer(i.FRAMEBUFFER,E),et(p)?o.framebufferTexture2DMultisampleEXT(i.FRAMEBUFFER,q,J,Pe.__webglTexture,0,Le(p)):(J===i.TEXTURE_2D||J>=i.TEXTURE_CUBE_MAP_POSITIVE_X&&J<=i.TEXTURE_CUBE_MAP_NEGATIVE_Z)&&i.framebufferTexture2D(i.FRAMEBUFFER,q,J,Pe.__webglTexture,me),t.bindFramebuffer(i.FRAMEBUFFER,null)}function Q(E,p,z){if(i.bindRenderbuffer(i.RENDERBUFFER,E),p.depthBuffer){const q=p.depthTexture,J=q&&q.isDepthTexture?q.type:null,me=w(p.stencilBuffer,J),ue=p.stencilBuffer?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT;et(p)?o.renderbufferStorageMultisampleEXT(i.RENDERBUFFER,Le(p),me,p.width,p.height):z?i.renderbufferStorageMultisample(i.RENDERBUFFER,Le(p),me,p.width,p.height):i.renderbufferStorage(i.RENDERBUFFER,me,p.width,p.height),i.framebufferRenderbuffer(i.FRAMEBUFFER,ue,i.RENDERBUFFER,E)}else{const q=p.textures;for(let J=0;J<q.length;J++){const me=q[J],ue=s.convert(me.format,me.colorSpace),j=s.convert(me.type),ee=y(me.internalFormat,ue,j,me.normalized,me.colorSpace);et(p)?o.renderbufferStorageMultisampleEXT(i.RENDERBUFFER,Le(p),ee,p.width,p.height):z?i.renderbufferStorageMultisample(i.RENDERBUFFER,Le(p),ee,p.width,p.height):i.renderbufferStorage(i.RENDERBUFFER,ee,p.width,p.height)}}i.bindRenderbuffer(i.RENDERBUFFER,null)}function te(E,p,z){const q=p.isWebGLCubeRenderTarget===!0;if(t.bindFramebuffer(i.FRAMEBUFFER,E),!(p.depthTexture&&p.depthTexture.isDepthTexture))throw new Error("THREE.WebGLTextures: renderTarget.depthTexture must be an instance of THREE.DepthTexture.");const J=n.get(p.depthTexture);if(J.__renderTarget=p,(!J.__webglTexture||p.depthTexture.image.width!==p.width||p.depthTexture.image.height!==p.height)&&(p.depthTexture.image.width=p.width,p.depthTexture.image.height=p.height,p.depthTexture.needsUpdate=!0),q){if(J.__webglInit===void 0&&(J.__webglInit=!0,p.depthTexture.addEventListener("dispose",I)),J.__webglTexture===void 0){J.__webglTexture=i.createTexture(),t.bindTexture(i.TEXTURE_CUBE_MAP,J.__webglTexture),He(i.TEXTURE_CUBE_MAP,p.depthTexture);const fe=s.convert(p.depthTexture.format),Pe=s.convert(p.depthTexture.type);let he;p.depthTexture.format===ci?he=i.DEPTH_COMPONENT24:p.depthTexture.format===Fi&&(he=i.DEPTH24_STENCIL8);for(let ve=0;ve<6;ve++)i.texImage2D(i.TEXTURE_CUBE_MAP_POSITIVE_X+ve,0,he,p.width,p.height,0,fe,Pe,null)}}else H(p.depthTexture,0);const me=J.__webglTexture,ue=Le(p),j=q?i.TEXTURE_CUBE_MAP_POSITIVE_X+z:i.TEXTURE_2D,ee=p.depthTexture.format===Fi?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT;if(p.depthTexture.format===ci)et(p)?o.framebufferTexture2DMultisampleEXT(i.FRAMEBUFFER,ee,j,me,0,ue):i.framebufferTexture2D(i.FRAMEBUFFER,ee,j,me,0);else if(p.depthTexture.format===Fi)et(p)?o.framebufferTexture2DMultisampleEXT(i.FRAMEBUFFER,ee,j,me,0,ue):i.framebufferTexture2D(i.FRAMEBUFFER,ee,j,me,0);else throw new Error("THREE.WebGLTextures: Unknown depthTexture format.")}function pe(E){const p=n.get(E),z=E.isWebGLCubeRenderTarget===!0;if(p.__boundDepthTexture!==E.depthTexture){const q=E.depthTexture;if(p.__depthDisposeCallback&&p.__depthDisposeCallback(),q){const J=()=>{delete p.__boundDepthTexture,delete p.__depthDisposeCallback,q.removeEventListener("dispose",J)};q.addEventListener("dispose",J),p.__depthDisposeCallback=J}p.__boundDepthTexture=q}if(E.depthTexture&&!p.__autoAllocateDepthBuffer)if(z)for(let q=0;q<6;q++)te(p.__webglFramebuffer[q],E,q);else{const q=E.texture.mipmaps;q&&q.length>0?te(p.__webglFramebuffer[0],E,0):te(p.__webglFramebuffer,E,0)}else if(z){p.__webglDepthbuffer=[];for(let q=0;q<6;q++)if(t.bindFramebuffer(i.FRAMEBUFFER,p.__webglFramebuffer[q]),p.__webglDepthbuffer[q]===void 0)p.__webglDepthbuffer[q]=i.createRenderbuffer(),Q(p.__webglDepthbuffer[q],E,!1);else{const J=E.stencilBuffer?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT,me=p.__webglDepthbuffer[q];i.bindRenderbuffer(i.RENDERBUFFER,me),i.framebufferRenderbuffer(i.FRAMEBUFFER,J,i.RENDERBUFFER,me)}}else{const q=E.texture.mipmaps;if(q&&q.length>0?t.bindFramebuffer(i.FRAMEBUFFER,p.__webglFramebuffer[0]):t.bindFramebuffer(i.FRAMEBUFFER,p.__webglFramebuffer),p.__webglDepthbuffer===void 0)p.__webglDepthbuffer=i.createRenderbuffer(),Q(p.__webglDepthbuffer,E,!1);else{const J=E.stencilBuffer?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT,me=p.__webglDepthbuffer;i.bindRenderbuffer(i.RENDERBUFFER,me),i.framebufferRenderbuffer(i.FRAMEBUFFER,J,i.RENDERBUFFER,me)}}t.bindFramebuffer(i.FRAMEBUFFER,null)}function Se(E,p,z){const q=n.get(E);p!==void 0&&Te(q.__webglFramebuffer,E,E.texture,i.COLOR_ATTACHMENT0,i.TEXTURE_2D,0),z!==void 0&&pe(E)}function Ce(E){const p=E.texture,z=n.get(E),q=n.get(p);E.addEventListener("dispose",_);const J=E.textures,me=E.isWebGLCubeRenderTarget===!0,ue=J.length>1;if(ue||(q.__webglTexture===void 0&&(q.__webglTexture=i.createTexture()),q.__version=p.version,a.memory.textures++),me){z.__webglFramebuffer=[];for(let j=0;j<6;j++)if(p.mipmaps&&p.mipmaps.length>0){z.__webglFramebuffer[j]=[];for(let ee=0;ee<p.mipmaps.length;ee++)z.__webglFramebuffer[j][ee]=i.createFramebuffer()}else z.__webglFramebuffer[j]=i.createFramebuffer()}else{if(p.mipmaps&&p.mipmaps.length>0){z.__webglFramebuffer=[];for(let j=0;j<p.mipmaps.length;j++)z.__webglFramebuffer[j]=i.createFramebuffer()}else z.__webglFramebuffer=i.createFramebuffer();if(ue)for(let j=0,ee=J.length;j<ee;j++){const fe=n.get(J[j]);fe.__webglTexture===void 0&&(fe.__webglTexture=i.createTexture(),a.memory.textures++)}if(E.samples>0&&et(E)===!1){z.__webglMultisampledFramebuffer=i.createFramebuffer(),z.__webglColorRenderbuffer=[],t.bindFramebuffer(i.FRAMEBUFFER,z.__webglMultisampledFramebuffer);for(let j=0;j<J.length;j++){const ee=J[j];z.__webglColorRenderbuffer[j]=i.createRenderbuffer(),i.bindRenderbuffer(i.RENDERBUFFER,z.__webglColorRenderbuffer[j]);const fe=s.convert(ee.format,ee.colorSpace),Pe=s.convert(ee.type),he=y(ee.internalFormat,fe,Pe,ee.normalized,ee.colorSpace,E.isXRRenderTarget===!0),ve=Le(E);i.renderbufferStorageMultisample(i.RENDERBUFFER,ve,he,E.width,E.height),i.framebufferRenderbuffer(i.FRAMEBUFFER,i.COLOR_ATTACHMENT0+j,i.RENDERBUFFER,z.__webglColorRenderbuffer[j])}i.bindRenderbuffer(i.RENDERBUFFER,null),E.depthBuffer&&(z.__webglDepthRenderbuffer=i.createRenderbuffer(),Q(z.__webglDepthRenderbuffer,E,!0)),t.bindFramebuffer(i.FRAMEBUFFER,null)}}if(me){t.bindTexture(i.TEXTURE_CUBE_MAP,q.__webglTexture),He(i.TEXTURE_CUBE_MAP,p);for(let j=0;j<6;j++)if(p.mipmaps&&p.mipmaps.length>0)for(let ee=0;ee<p.mipmaps.length;ee++)Te(z.__webglFramebuffer[j][ee],E,p,i.COLOR_ATTACHMENT0,i.TEXTURE_CUBE_MAP_POSITIVE_X+j,ee);else Te(z.__webglFramebuffer[j],E,p,i.COLOR_ATTACHMENT0,i.TEXTURE_CUBE_MAP_POSITIVE_X+j,0);h(p)&&R(i.TEXTURE_CUBE_MAP),t.unbindTexture()}else if(ue){for(let j=0,ee=J.length;j<ee;j++){const fe=J[j],Pe=n.get(fe);let he=i.TEXTURE_2D;(E.isWebGL3DRenderTarget||E.isWebGLArrayRenderTarget)&&(he=E.isWebGL3DRenderTarget?i.TEXTURE_3D:i.TEXTURE_2D_ARRAY),t.bindTexture(he,Pe.__webglTexture),He(he,fe),Te(z.__webglFramebuffer,E,fe,i.COLOR_ATTACHMENT0+j,he,0),h(fe)&&R(he)}t.unbindTexture()}else{let j=i.TEXTURE_2D;if((E.isWebGL3DRenderTarget||E.isWebGLArrayRenderTarget)&&(j=E.isWebGL3DRenderTarget?i.TEXTURE_3D:i.TEXTURE_2D_ARRAY),t.bindTexture(j,q.__webglTexture),He(j,p),p.mipmaps&&p.mipmaps.length>0)for(let ee=0;ee<p.mipmaps.length;ee++)Te(z.__webglFramebuffer[ee],E,p,i.COLOR_ATTACHMENT0,j,ee);else Te(z.__webglFramebuffer,E,p,i.COLOR_ATTACHMENT0,j,0);h(p)&&R(j),t.unbindTexture()}E.depthBuffer&&pe(E)}function We(E){const p=E.textures;for(let z=0,q=p.length;z<q;z++){const J=p[z];if(h(J)){const me=C(E),ue=n.get(J).__webglTexture;t.bindTexture(me,ue),R(me),t.unbindTexture()}}}const qe=[],ut=[];function ot(E){if(E.samples>0){if(et(E)===!1){const p=E.textures,z=E.width,q=E.height;let J=i.COLOR_BUFFER_BIT;const me=E.stencilBuffer?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT,ue=n.get(E),j=p.length>1;if(j)for(let fe=0;fe<p.length;fe++)t.bindFramebuffer(i.FRAMEBUFFER,ue.__webglMultisampledFramebuffer),i.framebufferRenderbuffer(i.FRAMEBUFFER,i.COLOR_ATTACHMENT0+fe,i.RENDERBUFFER,null),t.bindFramebuffer(i.FRAMEBUFFER,ue.__webglFramebuffer),i.framebufferTexture2D(i.DRAW_FRAMEBUFFER,i.COLOR_ATTACHMENT0+fe,i.TEXTURE_2D,null,0);t.bindFramebuffer(i.READ_FRAMEBUFFER,ue.__webglMultisampledFramebuffer);const ee=E.texture.mipmaps;ee&&ee.length>0?t.bindFramebuffer(i.DRAW_FRAMEBUFFER,ue.__webglFramebuffer[0]):t.bindFramebuffer(i.DRAW_FRAMEBUFFER,ue.__webglFramebuffer);for(let fe=0;fe<p.length;fe++){if(E.resolveDepthBuffer&&(E.depthBuffer&&(J|=i.DEPTH_BUFFER_BIT),E.stencilBuffer&&E.resolveStencilBuffer&&(J|=i.STENCIL_BUFFER_BIT)),j){i.framebufferRenderbuffer(i.READ_FRAMEBUFFER,i.COLOR_ATTACHMENT0,i.RENDERBUFFER,ue.__webglColorRenderbuffer[fe]);const Pe=n.get(p[fe]).__webglTexture;i.framebufferTexture2D(i.DRAW_FRAMEBUFFER,i.COLOR_ATTACHMENT0,i.TEXTURE_2D,Pe,0)}i.blitFramebuffer(0,0,z,q,0,0,z,q,J,i.NEAREST),c===!0&&(qe.length=0,ut.length=0,qe.push(i.COLOR_ATTACHMENT0+fe),E.depthBuffer&&E.resolveDepthBuffer===!1&&(qe.push(me),ut.push(me),i.invalidateFramebuffer(i.DRAW_FRAMEBUFFER,ut)),i.invalidateFramebuffer(i.READ_FRAMEBUFFER,qe))}if(t.bindFramebuffer(i.READ_FRAMEBUFFER,null),t.bindFramebuffer(i.DRAW_FRAMEBUFFER,null),j)for(let fe=0;fe<p.length;fe++){t.bindFramebuffer(i.FRAMEBUFFER,ue.__webglMultisampledFramebuffer),i.framebufferRenderbuffer(i.FRAMEBUFFER,i.COLOR_ATTACHMENT0+fe,i.RENDERBUFFER,ue.__webglColorRenderbuffer[fe]);const Pe=n.get(p[fe]).__webglTexture;t.bindFramebuffer(i.FRAMEBUFFER,ue.__webglFramebuffer),i.framebufferTexture2D(i.DRAW_FRAMEBUFFER,i.COLOR_ATTACHMENT0+fe,i.TEXTURE_2D,Pe,0)}t.bindFramebuffer(i.DRAW_FRAMEBUFFER,ue.__webglMultisampledFramebuffer)}else if(E.depthBuffer&&E.resolveDepthBuffer===!1&&c){const p=E.stencilBuffer?i.DEPTH_STENCIL_ATTACHMENT:i.DEPTH_ATTACHMENT;i.invalidateFramebuffer(i.DRAW_FRAMEBUFFER,[p])}}}function Le(E){return Math.min(r.maxSamples,E.samples)}function et(E){const p=n.get(E);return E.samples>0&&e.has("WEBGL_multisampled_render_to_texture")===!0&&p.__useRenderToTexture!==!1}function L(E){const p=a.render.frame;u.get(E)!==p&&(u.set(E,p),E.update())}function ht(E,p){const z=E.colorSpace,q=E.format,J=E.type;return E.isCompressedTexture===!0||E.isVideoTexture===!0||z!==Or&&z!==ii&&(mt.getTransfer(z)===Ct?(q!==Cn||J!==yn)&&je("WebGLTextures: sRGB encoded textures have to use RGBAFormat and UnsignedByteType."):_t("WebGLTextures: Unsupported texture color space:",z)),p}function $e(E){return typeof HTMLImageElement<"u"&&E instanceof HTMLImageElement?(l.width=E.naturalWidth||E.width,l.height=E.naturalHeight||E.height):typeof VideoFrame<"u"&&E instanceof VideoFrame?(l.width=E.displayWidth,l.height=E.displayHeight):(l.width=E.width,l.height=E.height),l}this.allocateTextureUnit=D,this.resetTextureUnits=P,this.getTextureUnits=W,this.setTextureUnits=N,this.setTexture2D=H,this.setTexture2DArray=ne,this.setTexture3D=ce,this.setTextureCube=oe,this.rebindTextures=Se,this.setupRenderTarget=Ce,this.updateRenderTargetMipmap=We,this.updateMultisampleRenderTarget=ot,this.setupDepthRenderbuffer=pe,this.setupFrameBufferTexture=Te,this.useMultisampledRTT=et,this.isReversedDepthBuffer=function(){return t.buffers.depth.getReversed()}}function mg(i,e){function t(n,r=ii){let s;const a=mt.getTransfer(r);if(n===yn)return i.UNSIGNED_BYTE;if(n===bo)return i.UNSIGNED_SHORT_4_4_4_4;if(n===Eo)return i.UNSIGNED_SHORT_5_5_5_1;if(n===oc)return i.UNSIGNED_INT_5_9_9_9_REV;if(n===lc)return i.UNSIGNED_INT_10F_11F_11F_REV;if(n===sc)return i.BYTE;if(n===ac)return i.SHORT;if(n===Nr)return i.UNSIGNED_SHORT;if(n===yo)return i.INT;if(n===Xn)return i.UNSIGNED_INT;if(n===Vn)return i.FLOAT;if(n===li)return i.HALF_FLOAT;if(n===cc)return i.ALPHA;if(n===dc)return i.RGB;if(n===Cn)return i.RGBA;if(n===ci)return i.DEPTH_COMPONENT;if(n===Fi)return i.DEPTH_STENCIL;if(n===uc)return i.RED;if(n===To)return i.RED_INTEGER;if(n===Bi)return i.RG;if(n===Ao)return i.RG_INTEGER;if(n===wo)return i.RGBA_INTEGER;if(n===Ms||n===Ss||n===ys||n===bs)if(a===Ct)if(s=e.get("WEBGL_compressed_texture_s3tc_srgb"),s!==null){if(n===Ms)return s.COMPRESSED_SRGB_S3TC_DXT1_EXT;if(n===Ss)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT1_EXT;if(n===ys)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT3_EXT;if(n===bs)return s.COMPRESSED_SRGB_ALPHA_S3TC_DXT5_EXT}else return null;else if(s=e.get("WEBGL_compressed_texture_s3tc"),s!==null){if(n===Ms)return s.COMPRESSED_RGB_S3TC_DXT1_EXT;if(n===Ss)return s.COMPRESSED_RGBA_S3TC_DXT1_EXT;if(n===ys)return s.COMPRESSED_RGBA_S3TC_DXT3_EXT;if(n===bs)return s.COMPRESSED_RGBA_S3TC_DXT5_EXT}else return null;if(n===Oa||n===Ba||n===za||n===ka)if(s=e.get("WEBGL_compressed_texture_pvrtc"),s!==null){if(n===Oa)return s.COMPRESSED_RGB_PVRTC_4BPPV1_IMG;if(n===Ba)return s.COMPRESSED_RGB_PVRTC_2BPPV1_IMG;if(n===za)return s.COMPRESSED_RGBA_PVRTC_4BPPV1_IMG;if(n===ka)return s.COMPRESSED_RGBA_PVRTC_2BPPV1_IMG}else return null;if(n===Ga||n===Va||n===Ha||n===Wa||n===Xa||n===Ts||n===qa)if(s=e.get("WEBGL_compressed_texture_etc"),s!==null){if(n===Ga||n===Va)return a===Ct?s.COMPRESSED_SRGB8_ETC2:s.COMPRESSED_RGB8_ETC2;if(n===Ha)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ETC2_EAC:s.COMPRESSED_RGBA8_ETC2_EAC;if(n===Wa)return s.COMPRESSED_R11_EAC;if(n===Xa)return s.COMPRESSED_SIGNED_R11_EAC;if(n===Ts)return s.COMPRESSED_RG11_EAC;if(n===qa)return s.COMPRESSED_SIGNED_RG11_EAC}else return null;if(n===Ya||n===Ka||n===Za||n===$a||n===Ja||n===Qa||n===ja||n===eo||n===to||n===no||n===io||n===ro||n===so||n===ao)if(s=e.get("WEBGL_compressed_texture_astc"),s!==null){if(n===Ya)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_4x4_KHR:s.COMPRESSED_RGBA_ASTC_4x4_KHR;if(n===Ka)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x4_KHR:s.COMPRESSED_RGBA_ASTC_5x4_KHR;if(n===Za)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_5x5_KHR:s.COMPRESSED_RGBA_ASTC_5x5_KHR;if(n===$a)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x5_KHR:s.COMPRESSED_RGBA_ASTC_6x5_KHR;if(n===Ja)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_6x6_KHR:s.COMPRESSED_RGBA_ASTC_6x6_KHR;if(n===Qa)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x5_KHR:s.COMPRESSED_RGBA_ASTC_8x5_KHR;if(n===ja)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x6_KHR:s.COMPRESSED_RGBA_ASTC_8x6_KHR;if(n===eo)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_8x8_KHR:s.COMPRESSED_RGBA_ASTC_8x8_KHR;if(n===to)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x5_KHR:s.COMPRESSED_RGBA_ASTC_10x5_KHR;if(n===no)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x6_KHR:s.COMPRESSED_RGBA_ASTC_10x6_KHR;if(n===io)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x8_KHR:s.COMPRESSED_RGBA_ASTC_10x8_KHR;if(n===ro)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_10x10_KHR:s.COMPRESSED_RGBA_ASTC_10x10_KHR;if(n===so)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x10_KHR:s.COMPRESSED_RGBA_ASTC_12x10_KHR;if(n===ao)return a===Ct?s.COMPRESSED_SRGB8_ALPHA8_ASTC_12x12_KHR:s.COMPRESSED_RGBA_ASTC_12x12_KHR}else return null;if(n===oo||n===lo||n===co)if(s=e.get("EXT_texture_compression_bptc"),s!==null){if(n===oo)return a===Ct?s.COMPRESSED_SRGB_ALPHA_BPTC_UNORM_EXT:s.COMPRESSED_RGBA_BPTC_UNORM_EXT;if(n===lo)return s.COMPRESSED_RGB_BPTC_SIGNED_FLOAT_EXT;if(n===co)return s.COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT_EXT}else return null;if(n===uo||n===ho||n===As||n===fo)if(s=e.get("EXT_texture_compression_rgtc"),s!==null){if(n===uo)return s.COMPRESSED_RED_RGTC1_EXT;if(n===ho)return s.COMPRESSED_SIGNED_RED_RGTC1_EXT;if(n===As)return s.COMPRESSED_RED_GREEN_RGTC2_EXT;if(n===fo)return s.COMPRESSED_SIGNED_RED_GREEN_RGTC2_EXT}else return null;return n===Fr?i.UNSIGNED_INT_24_8:i[n]!==void 0?i[n]:null}return{convert:t}}const gg=`
void main() {

	gl_Position = vec4( position, 1.0 );

}`,_g=`
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

}`;class xg{constructor(){this.texture=null,this.mesh=null,this.depthNear=0,this.depthFar=0}init(e,t){if(this.texture===null){const n=new Sc(e.texture);(e.depthNear!==t.depthNear||e.depthFar!==t.depthFar)&&(this.depthNear=e.depthNear,this.depthFar=e.depthFar),this.texture=n}}getMesh(e){if(this.texture!==null&&this.mesh===null){const t=e.cameras[0].viewport,n=new Yn({vertexShader:gg,fragmentShader:_g,uniforms:{depthColor:{value:this.texture},depthWidth:{value:t.z},depthHeight:{value:t.w}}});this.mesh=new qn(new Fs(20,20),n)}return this.mesh}reset(){this.texture=null,this.mesh=null}getDepthTexture(){return this.texture}}class vg extends Gi{constructor(e,t){super();const n=this;let r=null,s=1,a=null,o="local-floor",c=1,l=null,u=null,f=null,d=null,m=null,x=null;const S=typeof XRWebGLBinding<"u",g=new xg,h={},R=t.getContextAttributes();let C=null,y=null;const w=[],T=[],I=new Mt;let _=null;const A=new wn;A.viewport=new Ot;const k=new wn;k.viewport=new Ot;const F=[A,k],b=new Ru;let P=null,W=null;this.cameraAutoUpdate=!0,this.enabled=!1,this.isPresenting=!1,this.getController=function(Z){let de=w[Z];return de===void 0&&(de=new ea,w[Z]=de),de.getTargetRaySpace()},this.getControllerGrip=function(Z){let de=w[Z];return de===void 0&&(de=new ea,w[Z]=de),de.getGripSpace()},this.getHand=function(Z){let de=w[Z];return de===void 0&&(de=new ea,w[Z]=de),de.getHandSpace()};function N(Z){const de=T.indexOf(Z.inputSource);if(de===-1)return;const le=w[de];le!==void 0&&(le.update(Z.inputSource,Z.frame,l||a),le.dispatchEvent({type:Z.type,data:Z.inputSource}))}function D(){r.removeEventListener("select",N),r.removeEventListener("selectstart",N),r.removeEventListener("selectend",N),r.removeEventListener("squeeze",N),r.removeEventListener("squeezestart",N),r.removeEventListener("squeezeend",N),r.removeEventListener("end",D),r.removeEventListener("inputsourceschange",G);for(let Z=0;Z<w.length;Z++){const de=T[Z];de!==null&&(T[Z]=null,w[Z].disconnect(de))}P=null,W=null,g.reset();for(const Z in h)delete h[Z];e.setRenderTarget(C),m=null,d=null,f=null,r=null,y=null,He.stop(),n.isPresenting=!1,e.setPixelRatio(_),e.setSize(I.width,I.height,!1),n.dispatchEvent({type:"sessionend"})}this.setFramebufferScaleFactor=function(Z){s=Z,n.isPresenting===!0&&je("WebXRManager: Cannot change framebuffer scale while presenting.")},this.setReferenceSpaceType=function(Z){o=Z,n.isPresenting===!0&&je("WebXRManager: Cannot change reference space type while presenting.")},this.getReferenceSpace=function(){return l||a},this.setReferenceSpace=function(Z){l=Z},this.getBaseLayer=function(){return d!==null?d:m},this.getBinding=function(){return f===null&&S&&(f=new XRWebGLBinding(r,t)),f},this.getFrame=function(){return x},this.getSession=function(){return r},this.setSession=async function(Z){if(r=Z,r!==null){if(C=e.getRenderTarget(),r.addEventListener("select",N),r.addEventListener("selectstart",N),r.addEventListener("selectend",N),r.addEventListener("squeeze",N),r.addEventListener("squeezestart",N),r.addEventListener("squeezeend",N),r.addEventListener("end",D),r.addEventListener("inputsourceschange",G),R.xrCompatible!==!0&&await t.makeXRCompatible(),_=e.getPixelRatio(),e.getSize(I),S&&"createProjectionLayer"in XRWebGLBinding.prototype){let le=null,Oe=null,Je=null;R.depth&&(Je=R.stencil?t.DEPTH24_STENCIL8:t.DEPTH_COMPONENT24,le=R.stencil?Fi:ci,Oe=R.stencil?Fr:Xn);const Te={colorFormat:t.RGBA8,depthFormat:Je,scaleFactor:s};f=this.getBinding(),d=f.createProjectionLayer(Te),r.updateRenderState({layers:[d]}),e.setPixelRatio(1),e.setSize(d.textureWidth,d.textureHeight,!1),y=new Wn(d.textureWidth,d.textureHeight,{format:Cn,type:yn,depthTexture:new gr(d.textureWidth,d.textureHeight,Oe,void 0,void 0,void 0,void 0,void 0,void 0,le),stencilBuffer:R.stencil,colorSpace:e.outputColorSpace,samples:R.antialias?4:0,resolveDepthBuffer:d.ignoreDepthValues===!1,resolveStencilBuffer:d.ignoreDepthValues===!1})}else{const le={antialias:R.antialias,alpha:!0,depth:R.depth,stencil:R.stencil,framebufferScaleFactor:s};m=new XRWebGLLayer(r,t,le),r.updateRenderState({baseLayer:m}),e.setPixelRatio(1),e.setSize(m.framebufferWidth,m.framebufferHeight,!1),y=new Wn(m.framebufferWidth,m.framebufferHeight,{format:Cn,type:yn,colorSpace:e.outputColorSpace,stencilBuffer:R.stencil,resolveDepthBuffer:m.ignoreDepthValues===!1,resolveStencilBuffer:m.ignoreDepthValues===!1})}y.isXRRenderTarget=!0,this.setFoveation(c),l=null,a=await r.requestReferenceSpace(o),He.setContext(r),He.start(),n.isPresenting=!0,n.dispatchEvent({type:"sessionstart"})}},this.getEnvironmentBlendMode=function(){if(r!==null)return r.environmentBlendMode},this.getDepthTexture=function(){return g.getDepthTexture()};function G(Z){for(let de=0;de<Z.removed.length;de++){const le=Z.removed[de],Oe=T.indexOf(le);Oe>=0&&(T[Oe]=null,w[Oe].disconnect(le))}for(let de=0;de<Z.added.length;de++){const le=Z.added[de];let Oe=T.indexOf(le);if(Oe===-1){for(let Te=0;Te<w.length;Te++)if(Te>=T.length){T.push(le),Oe=Te;break}else if(T[Te]===null){T[Te]=le,Oe=Te;break}if(Oe===-1)break}const Je=w[Oe];Je&&Je.connect(le)}}const H=new K,ne=new K;function ce(Z,de,le){H.setFromMatrixPosition(de.matrixWorld),ne.setFromMatrixPosition(le.matrixWorld);const Oe=H.distanceTo(ne),Je=de.projectionMatrix.elements,Te=le.projectionMatrix.elements,Q=Je[14]/(Je[10]-1),te=Je[14]/(Je[10]+1),pe=(Je[9]+1)/Je[5],Se=(Je[9]-1)/Je[5],Ce=(Je[8]-1)/Je[0],We=(Te[8]+1)/Te[0],qe=Q*Ce,ut=Q*We,ot=Oe/(-Ce+We),Le=ot*-Ce;if(de.matrixWorld.decompose(Z.position,Z.quaternion,Z.scale),Z.translateX(Le),Z.translateZ(ot),Z.matrixWorld.compose(Z.position,Z.quaternion,Z.scale),Z.matrixWorldInverse.copy(Z.matrixWorld).invert(),Je[10]===-1)Z.projectionMatrix.copy(de.projectionMatrix),Z.projectionMatrixInverse.copy(de.projectionMatrixInverse);else{const et=Q+ot,L=te+ot,ht=qe-Le,$e=ut+(Oe-Le),E=pe*te/L*et,p=Se*te/L*et;Z.projectionMatrix.makePerspective(ht,$e,E,p,et,L),Z.projectionMatrixInverse.copy(Z.projectionMatrix).invert()}}function oe(Z,de){de===null?Z.matrixWorld.copy(Z.matrix):Z.matrixWorld.multiplyMatrices(de.matrixWorld,Z.matrix),Z.matrixWorldInverse.copy(Z.matrixWorld).invert()}this.updateCamera=function(Z){if(r===null)return;let de=Z.near,le=Z.far;g.texture!==null&&(g.depthNear>0&&(de=g.depthNear),g.depthFar>0&&(le=g.depthFar)),b.near=k.near=A.near=de,b.far=k.far=A.far=le,(P!==b.near||W!==b.far)&&(r.updateRenderState({depthNear:b.near,depthFar:b.far}),P=b.near,W=b.far),b.layers.mask=Z.layers.mask|6,A.layers.mask=b.layers.mask&-5,k.layers.mask=b.layers.mask&-3;const Oe=Z.parent,Je=b.cameras;oe(b,Oe);for(let Te=0;Te<Je.length;Te++)oe(Je[Te],Oe);Je.length===2?ce(b,A,k):b.projectionMatrix.copy(A.projectionMatrix),_e(Z,b,Oe)};function _e(Z,de,le){le===null?Z.matrix.copy(de.matrixWorld):(Z.matrix.copy(le.matrixWorld),Z.matrix.invert(),Z.matrix.multiply(de.matrixWorld)),Z.matrix.decompose(Z.position,Z.quaternion,Z.scale),Z.updateMatrixWorld(!0),Z.projectionMatrix.copy(de.projectionMatrix),Z.projectionMatrixInverse.copy(de.projectionMatrixInverse),Z.isPerspectiveCamera&&(Z.fov=go*2*Math.atan(1/Z.projectionMatrix.elements[5]),Z.zoom=1)}this.getCamera=function(){return b},this.getFoveation=function(){if(!(d===null&&m===null))return c},this.setFoveation=function(Z){c=Z,d!==null&&(d.fixedFoveation=Z),m!==null&&m.fixedFoveation!==void 0&&(m.fixedFoveation=Z)},this.hasDepthSensing=function(){return g.texture!==null},this.getDepthSensingMesh=function(){return g.getMesh(b)},this.getCameraTexture=function(Z){return h[Z]};let Qe=null;function Ze(Z,de){if(u=de.getViewerPose(l||a),x=de,u!==null){const le=u.views;m!==null&&(e.setRenderTargetFramebuffer(y,m.framebuffer),e.setRenderTarget(y));let Oe=!1;le.length!==b.cameras.length&&(b.cameras.length=0,Oe=!0);for(let te=0;te<le.length;te++){const pe=le[te];let Se=null;if(m!==null)Se=m.getViewport(pe);else{const We=f.getViewSubImage(d,pe);Se=We.viewport,te===0&&(e.setRenderTargetTextures(y,We.colorTexture,We.depthStencilTexture),e.setRenderTarget(y))}let Ce=F[te];Ce===void 0&&(Ce=new wn,Ce.layers.enable(te),Ce.viewport=new Ot,F[te]=Ce),Ce.matrix.fromArray(pe.transform.matrix),Ce.matrix.decompose(Ce.position,Ce.quaternion,Ce.scale),Ce.projectionMatrix.fromArray(pe.projectionMatrix),Ce.projectionMatrixInverse.copy(Ce.projectionMatrix).invert(),Ce.viewport.set(Se.x,Se.y,Se.width,Se.height),te===0&&(b.matrix.copy(Ce.matrix),b.matrix.decompose(b.position,b.quaternion,b.scale)),Oe===!0&&b.cameras.push(Ce)}const Je=r.enabledFeatures;if(Je&&Je.includes("depth-sensing")&&r.depthUsage=="gpu-optimized"&&S){f=n.getBinding();const te=f.getDepthInformation(le[0]);te&&te.isValid&&te.texture&&g.init(te,r.renderState)}if(Je&&Je.includes("camera-access")&&S){e.state.unbindTexture(),f=n.getBinding();for(let te=0;te<le.length;te++){const pe=le[te].camera;if(pe){let Se=h[pe];Se||(Se=new Sc,h[pe]=Se);const Ce=f.getCameraImage(pe);Se.sourceTexture=Ce}}}}for(let le=0;le<w.length;le++){const Oe=T[le],Je=w[le];Oe!==null&&Je!==void 0&&Je.update(Oe,de,l||a)}Qe&&Qe(Z,de),de.detectedPlanes&&n.dispatchEvent({type:"planesdetected",data:de}),x=null}const He=new Tc;He.setAnimationLoop(Ze),this.setAnimationLoop=function(Z){Qe=Z},this.dispose=function(){}}}const Mg=new Vt,Dc=new nt;Dc.set(-1,0,0,0,1,0,0,0,1);function Sg(i,e){function t(g,h){g.matrixAutoUpdate===!0&&g.updateMatrix(),h.value.copy(g.matrix)}function n(g,h){h.color.getRGB(g.fogColor.value,yc(i)),h.isFog?(g.fogNear.value=h.near,g.fogFar.value=h.far):h.isFogExp2&&(g.fogDensity.value=h.density)}function r(g,h,R,C,y){h.isNodeMaterial?h.uniformsNeedUpdate=!1:h.isMeshBasicMaterial?s(g,h):h.isMeshLambertMaterial?(s(g,h),h.envMap&&(g.envMapIntensity.value=h.envMapIntensity)):h.isMeshToonMaterial?(s(g,h),f(g,h)):h.isMeshPhongMaterial?(s(g,h),u(g,h),h.envMap&&(g.envMapIntensity.value=h.envMapIntensity)):h.isMeshStandardMaterial?(s(g,h),d(g,h),h.isMeshPhysicalMaterial&&m(g,h,y)):h.isMeshMatcapMaterial?(s(g,h),x(g,h)):h.isMeshDepthMaterial?s(g,h):h.isMeshDistanceMaterial?(s(g,h),S(g,h)):h.isMeshNormalMaterial?s(g,h):h.isLineBasicMaterial?(a(g,h),h.isLineDashedMaterial&&o(g,h)):h.isPointsMaterial?c(g,h,R,C):h.isSpriteMaterial?l(g,h):h.isShadowMaterial?(g.color.value.copy(h.color),g.opacity.value=h.opacity):h.isShaderMaterial&&(h.uniformsNeedUpdate=!1)}function s(g,h){g.opacity.value=h.opacity,h.color&&g.diffuse.value.copy(h.color),h.emissive&&g.emissive.value.copy(h.emissive).multiplyScalar(h.emissiveIntensity),h.map&&(g.map.value=h.map,t(h.map,g.mapTransform)),h.alphaMap&&(g.alphaMap.value=h.alphaMap,t(h.alphaMap,g.alphaMapTransform)),h.bumpMap&&(g.bumpMap.value=h.bumpMap,t(h.bumpMap,g.bumpMapTransform),g.bumpScale.value=h.bumpScale,h.side===cn&&(g.bumpScale.value*=-1)),h.normalMap&&(g.normalMap.value=h.normalMap,t(h.normalMap,g.normalMapTransform),g.normalScale.value.copy(h.normalScale),h.side===cn&&g.normalScale.value.negate()),h.displacementMap&&(g.displacementMap.value=h.displacementMap,t(h.displacementMap,g.displacementMapTransform),g.displacementScale.value=h.displacementScale,g.displacementBias.value=h.displacementBias),h.emissiveMap&&(g.emissiveMap.value=h.emissiveMap,t(h.emissiveMap,g.emissiveMapTransform)),h.specularMap&&(g.specularMap.value=h.specularMap,t(h.specularMap,g.specularMapTransform)),h.alphaTest>0&&(g.alphaTest.value=h.alphaTest);const R=e.get(h),C=R.envMap,y=R.envMapRotation;C&&(g.envMap.value=C,g.envMapRotation.value.setFromMatrix4(Mg.makeRotationFromEuler(y)).transpose(),C.isCubeTexture&&C.isRenderTargetTexture===!1&&g.envMapRotation.value.premultiply(Dc),g.reflectivity.value=h.reflectivity,g.ior.value=h.ior,g.refractionRatio.value=h.refractionRatio),h.lightMap&&(g.lightMap.value=h.lightMap,g.lightMapIntensity.value=h.lightMapIntensity,t(h.lightMap,g.lightMapTransform)),h.aoMap&&(g.aoMap.value=h.aoMap,g.aoMapIntensity.value=h.aoMapIntensity,t(h.aoMap,g.aoMapTransform))}function a(g,h){g.diffuse.value.copy(h.color),g.opacity.value=h.opacity,h.map&&(g.map.value=h.map,t(h.map,g.mapTransform))}function o(g,h){g.dashSize.value=h.dashSize,g.totalSize.value=h.dashSize+h.gapSize,g.scale.value=h.scale}function c(g,h,R,C){g.diffuse.value.copy(h.color),g.opacity.value=h.opacity,g.size.value=h.size*R,g.scale.value=C*.5,h.map&&(g.map.value=h.map,t(h.map,g.uvTransform)),h.alphaMap&&(g.alphaMap.value=h.alphaMap,t(h.alphaMap,g.alphaMapTransform)),h.alphaTest>0&&(g.alphaTest.value=h.alphaTest)}function l(g,h){g.diffuse.value.copy(h.color),g.opacity.value=h.opacity,g.rotation.value=h.rotation,h.map&&(g.map.value=h.map,t(h.map,g.mapTransform)),h.alphaMap&&(g.alphaMap.value=h.alphaMap,t(h.alphaMap,g.alphaMapTransform)),h.alphaTest>0&&(g.alphaTest.value=h.alphaTest)}function u(g,h){g.specular.value.copy(h.specular),g.shininess.value=Math.max(h.shininess,1e-4)}function f(g,h){h.gradientMap&&(g.gradientMap.value=h.gradientMap)}function d(g,h){g.metalness.value=h.metalness,h.metalnessMap&&(g.metalnessMap.value=h.metalnessMap,t(h.metalnessMap,g.metalnessMapTransform)),g.roughness.value=h.roughness,h.roughnessMap&&(g.roughnessMap.value=h.roughnessMap,t(h.roughnessMap,g.roughnessMapTransform)),h.envMap&&(g.envMapIntensity.value=h.envMapIntensity)}function m(g,h,R){g.ior.value=h.ior,h.sheen>0&&(g.sheenColor.value.copy(h.sheenColor).multiplyScalar(h.sheen),g.sheenRoughness.value=h.sheenRoughness,h.sheenColorMap&&(g.sheenColorMap.value=h.sheenColorMap,t(h.sheenColorMap,g.sheenColorMapTransform)),h.sheenRoughnessMap&&(g.sheenRoughnessMap.value=h.sheenRoughnessMap,t(h.sheenRoughnessMap,g.sheenRoughnessMapTransform))),h.clearcoat>0&&(g.clearcoat.value=h.clearcoat,g.clearcoatRoughness.value=h.clearcoatRoughness,h.clearcoatMap&&(g.clearcoatMap.value=h.clearcoatMap,t(h.clearcoatMap,g.clearcoatMapTransform)),h.clearcoatRoughnessMap&&(g.clearcoatRoughnessMap.value=h.clearcoatRoughnessMap,t(h.clearcoatRoughnessMap,g.clearcoatRoughnessMapTransform)),h.clearcoatNormalMap&&(g.clearcoatNormalMap.value=h.clearcoatNormalMap,t(h.clearcoatNormalMap,g.clearcoatNormalMapTransform),g.clearcoatNormalScale.value.copy(h.clearcoatNormalScale),h.side===cn&&g.clearcoatNormalScale.value.negate())),h.dispersion>0&&(g.dispersion.value=h.dispersion),h.iridescence>0&&(g.iridescence.value=h.iridescence,g.iridescenceIOR.value=h.iridescenceIOR,g.iridescenceThicknessMinimum.value=h.iridescenceThicknessRange[0],g.iridescenceThicknessMaximum.value=h.iridescenceThicknessRange[1],h.iridescenceMap&&(g.iridescenceMap.value=h.iridescenceMap,t(h.iridescenceMap,g.iridescenceMapTransform)),h.iridescenceThicknessMap&&(g.iridescenceThicknessMap.value=h.iridescenceThicknessMap,t(h.iridescenceThicknessMap,g.iridescenceThicknessMapTransform))),h.transmission>0&&(g.transmission.value=h.transmission,g.transmissionSamplerMap.value=R.texture,g.transmissionSamplerSize.value.set(R.width,R.height),h.transmissionMap&&(g.transmissionMap.value=h.transmissionMap,t(h.transmissionMap,g.transmissionMapTransform)),g.thickness.value=h.thickness,h.thicknessMap&&(g.thicknessMap.value=h.thicknessMap,t(h.thicknessMap,g.thicknessMapTransform)),g.attenuationDistance.value=h.attenuationDistance,g.attenuationColor.value.copy(h.attenuationColor)),h.anisotropy>0&&(g.anisotropyVector.value.set(h.anisotropy*Math.cos(h.anisotropyRotation),h.anisotropy*Math.sin(h.anisotropyRotation)),h.anisotropyMap&&(g.anisotropyMap.value=h.anisotropyMap,t(h.anisotropyMap,g.anisotropyMapTransform))),g.specularIntensity.value=h.specularIntensity,g.specularColor.value.copy(h.specularColor),h.specularColorMap&&(g.specularColorMap.value=h.specularColorMap,t(h.specularColorMap,g.specularColorMapTransform)),h.specularIntensityMap&&(g.specularIntensityMap.value=h.specularIntensityMap,t(h.specularIntensityMap,g.specularIntensityMapTransform))}function x(g,h){h.matcap&&(g.matcap.value=h.matcap)}function S(g,h){const R=e.get(h).light;g.referencePosition.value.setFromMatrixPosition(R.matrixWorld),g.nearDistance.value=R.shadow.camera.near,g.farDistance.value=R.shadow.camera.far}return{refreshFogUniforms:n,refreshMaterialUniforms:r}}function yg(i,e,t,n){let r={},s={},a=[];const o=i.getParameter(i.MAX_UNIFORM_BUFFER_BINDINGS);function c(y,w){const T=w.program;n.uniformBlockBinding(y,T)}function l(y,w){let T=r[y.id];T===void 0&&(g(y),T=u(y),r[y.id]=T,y.addEventListener("dispose",R));const I=w.program;n.updateUBOMapping(y,I);const _=e.render.frame;s[y.id]!==_&&(d(y),s[y.id]=_)}function u(y){const w=f();y.__bindingPointIndex=w;const T=i.createBuffer(),I=y.__size,_=y.usage;return i.bindBuffer(i.UNIFORM_BUFFER,T),i.bufferData(i.UNIFORM_BUFFER,I,_),i.bindBuffer(i.UNIFORM_BUFFER,null),i.bindBufferBase(i.UNIFORM_BUFFER,w,T),T}function f(){for(let y=0;y<o;y++)if(a.indexOf(y)===-1)return a.push(y),y;return _t("WebGLRenderer: Maximum number of simultaneously usable uniforms groups reached."),0}function d(y){const w=r[y.id],T=y.uniforms,I=y.__cache;i.bindBuffer(i.UNIFORM_BUFFER,w);for(let _=0,A=T.length;_<A;_++){const k=T[_];if(Array.isArray(k))for(let F=0,b=k.length;F<b;F++)m(k[F],_,F,I);else m(k,_,0,I)}i.bindBuffer(i.UNIFORM_BUFFER,null)}function m(y,w,T,I){if(S(y,w,T,I)===!0){const _=y.__offset,A=y.value;if(Array.isArray(A)){let k=0;for(let F=0;F<A.length;F++){const b=A[F],P=h(b);x(b,y.__data,k),typeof b!="number"&&typeof b!="boolean"&&!b.isMatrix3&&!ArrayBuffer.isView(b)&&(k+=P.storage/Float32Array.BYTES_PER_ELEMENT)}}else x(A,y.__data,0);i.bufferSubData(i.UNIFORM_BUFFER,_,y.__data)}}function x(y,w,T){typeof y=="number"||typeof y=="boolean"?w[0]=y:y.isMatrix3?(w[0]=y.elements[0],w[1]=y.elements[1],w[2]=y.elements[2],w[3]=0,w[4]=y.elements[3],w[5]=y.elements[4],w[6]=y.elements[5],w[7]=0,w[8]=y.elements[6],w[9]=y.elements[7],w[10]=y.elements[8],w[11]=0):ArrayBuffer.isView(y)?w.set(new y.constructor(y.buffer,y.byteOffset,w.length)):y.toArray(w,T)}function S(y,w,T,I){const _=y.value,A=w+"_"+T;if(I[A]===void 0)return typeof _=="number"||typeof _=="boolean"?I[A]=_:ArrayBuffer.isView(_)?I[A]=_.slice():I[A]=_.clone(),!0;{const k=I[A];if(typeof _=="number"||typeof _=="boolean"){if(k!==_)return I[A]=_,!0}else{if(ArrayBuffer.isView(_))return!0;if(k.equals(_)===!1)return k.copy(_),!0}}return!1}function g(y){const w=y.uniforms;let T=0;const I=16;for(let A=0,k=w.length;A<k;A++){const F=Array.isArray(w[A])?w[A]:[w[A]];for(let b=0,P=F.length;b<P;b++){const W=F[b],N=Array.isArray(W.value)?W.value:[W.value];for(let D=0,G=N.length;D<G;D++){const H=N[D],ne=h(H),ce=T%I,oe=ce%ne.boundary,_e=ce+oe;T+=oe,_e!==0&&I-_e<ne.storage&&(T+=I-_e),W.__data=new Float32Array(ne.storage/Float32Array.BYTES_PER_ELEMENT),W.__offset=T,T+=ne.storage}}}const _=T%I;return _>0&&(T+=I-_),y.__size=T,y.__cache={},this}function h(y){const w={boundary:0,storage:0};return typeof y=="number"||typeof y=="boolean"?(w.boundary=4,w.storage=4):y.isVector2?(w.boundary=8,w.storage=8):y.isVector3||y.isColor?(w.boundary=16,w.storage=12):y.isVector4?(w.boundary=16,w.storage=16):y.isMatrix3?(w.boundary=48,w.storage=48):y.isMatrix4?(w.boundary=64,w.storage=64):y.isTexture?je("WebGLRenderer: Texture samplers can not be part of an uniforms group."):ArrayBuffer.isView(y)?(w.boundary=16,w.storage=y.byteLength):je("WebGLRenderer: Unsupported uniform value type.",y),w}function R(y){const w=y.target;w.removeEventListener("dispose",R);const T=a.indexOf(w.__bindingPointIndex);a.splice(T,1),i.deleteBuffer(r[w.id]),delete r[w.id],delete s[w.id]}function C(){for(const y in r)i.deleteBuffer(r[y]);a=[],r={},s={}}return{bind:c,update:l,dispose:C}}const bg=new Uint16Array([12469,15057,12620,14925,13266,14620,13807,14376,14323,13990,14545,13625,14713,13328,14840,12882,14931,12528,14996,12233,15039,11829,15066,11525,15080,11295,15085,10976,15082,10705,15073,10495,13880,14564,13898,14542,13977,14430,14158,14124,14393,13732,14556,13410,14702,12996,14814,12596,14891,12291,14937,11834,14957,11489,14958,11194,14943,10803,14921,10506,14893,10278,14858,9960,14484,14039,14487,14025,14499,13941,14524,13740,14574,13468,14654,13106,14743,12678,14818,12344,14867,11893,14889,11509,14893,11180,14881,10751,14852,10428,14812,10128,14765,9754,14712,9466,14764,13480,14764,13475,14766,13440,14766,13347,14769,13070,14786,12713,14816,12387,14844,11957,14860,11549,14868,11215,14855,10751,14825,10403,14782,10044,14729,9651,14666,9352,14599,9029,14967,12835,14966,12831,14963,12804,14954,12723,14936,12564,14917,12347,14900,11958,14886,11569,14878,11247,14859,10765,14828,10401,14784,10011,14727,9600,14660,9289,14586,8893,14508,8533,15111,12234,15110,12234,15104,12216,15092,12156,15067,12010,15028,11776,14981,11500,14942,11205,14902,10752,14861,10393,14812,9991,14752,9570,14682,9252,14603,8808,14519,8445,14431,8145,15209,11449,15208,11451,15202,11451,15190,11438,15163,11384,15117,11274,15055,10979,14994,10648,14932,10343,14871,9936,14803,9532,14729,9218,14645,8742,14556,8381,14461,8020,14365,7603,15273,10603,15272,10607,15267,10619,15256,10631,15231,10614,15182,10535,15118,10389,15042,10167,14963,9787,14883,9447,14800,9115,14710,8665,14615,8318,14514,7911,14411,7507,14279,7198,15314,9675,15313,9683,15309,9712,15298,9759,15277,9797,15229,9773,15166,9668,15084,9487,14995,9274,14898,8910,14800,8539,14697,8234,14590,7790,14479,7409,14367,7067,14178,6621,15337,8619,15337,8631,15333,8677,15325,8769,15305,8871,15264,8940,15202,8909,15119,8775,15022,8565,14916,8328,14804,8009,14688,7614,14569,7287,14448,6888,14321,6483,14088,6171,15350,7402,15350,7419,15347,7480,15340,7613,15322,7804,15287,7973,15229,8057,15148,8012,15046,7846,14933,7611,14810,7357,14682,7069,14552,6656,14421,6316,14251,5948,14007,5528,15356,5942,15356,5977,15353,6119,15348,6294,15332,6551,15302,6824,15249,7044,15171,7122,15070,7050,14949,6861,14818,6611,14679,6349,14538,6067,14398,5651,14189,5311,13935,4958,15359,4123,15359,4153,15356,4296,15353,4646,15338,5160,15311,5508,15263,5829,15188,6042,15088,6094,14966,6001,14826,5796,14678,5543,14527,5287,14377,4985,14133,4586,13869,4257,15360,1563,15360,1642,15358,2076,15354,2636,15341,3350,15317,4019,15273,4429,15203,4732,15105,4911,14981,4932,14836,4818,14679,4621,14517,4386,14359,4156,14083,3795,13808,3437,15360,122,15360,137,15358,285,15355,636,15344,1274,15322,2177,15281,2765,15215,3223,15120,3451,14995,3569,14846,3567,14681,3466,14511,3305,14344,3121,14037,2800,13753,2467,15360,0,15360,1,15359,21,15355,89,15346,253,15325,479,15287,796,15225,1148,15133,1492,15008,1749,14856,1882,14685,1886,14506,1783,14324,1608,13996,1398,13702,1183]);let Bn=null;function Eg(){return Bn===null&&(Bn=new mu(bg,16,16,Bi,li),Bn.name="DFG_LUT",Bn.minFilter=Qt,Bn.magFilter=Qt,Bn.wrapS=ri,Bn.wrapT=ri,Bn.generateMipmaps=!1,Bn.needsUpdate=!0),Bn}class Tg{constructor(e={}){const{canvas:t=Wd(),context:n=null,depth:r=!0,stencil:s=!1,alpha:a=!1,antialias:o=!1,premultipliedAlpha:c=!0,preserveDrawingBuffer:l=!1,powerPreference:u="default",failIfMajorPerformanceCaveat:f=!1,reversedDepthBuffer:d=!1,outputBufferType:m=yn}=e;this.isWebGLRenderer=!0;let x;if(n!==null){if(typeof WebGLRenderingContext<"u"&&n instanceof WebGLRenderingContext)throw new Error("THREE.WebGLRenderer: WebGL 1 is not supported since r163.");x=n.getContextAttributes().alpha}else x=a;const S=m,g=new Set([wo,Ao,To]),h=new Set([yn,Xn,Nr,Fr,bo,Eo]),R=new Uint32Array(4),C=new Int32Array(4),y=new K;let w=null,T=null;const I=[],_=[];let A=null;this.domElement=t,this.debug={checkShaderErrors:!0,onShaderError:null},this.autoClear=!0,this.autoClearColor=!0,this.autoClearDepth=!0,this.autoClearStencil=!0,this.sortObjects=!0,this.clippingPlanes=[],this.localClippingEnabled=!1,this.toneMapping=Pn,this.toneMappingExposure=1,this.transmissionResolutionScale=1;const k=this;let F=!1,b=null,P=null,W=null,N=null;this._outputColorSpace=Sn;let D=0,G=0,H=null,ne=-1,ce=null;const oe=new Ot,_e=new Ot;let Qe=null;const Ze=new Tt(0);let He=0,Z=t.width,de=t.height,le=1,Oe=null,Je=null;const Te=new Ot(0,0,Z,de),Q=new Ot(0,0,Z,de);let te=!1;const pe=new vc;let Se=!1,Ce=!1;const We=new Vt,qe=new K,ut=new Ot,ot={background:null,fog:null,environment:null,overrideMaterial:null,isScene:!0};let Le=!1;function et(){return H===null?le:1}let L=n;function ht(v,B){return t.getContext(v,B)}try{const v={alpha:!0,depth:r,stencil:s,antialias:o,premultipliedAlpha:c,preserveDrawingBuffer:l,powerPreference:u,failIfMajorPerformanceCaveat:f};if("setAttribute"in t&&t.setAttribute("data-engine",`three.js r${So}`),t.addEventListener("webglcontextlost",At,!1),t.addEventListener("webglcontextrestored",dt,!1),t.addEventListener("webglcontextcreationerror",gn,!1),L===null){const B="webgl2";if(L=ht(B,v),L===null)throw ht(B)?new Error("THREE.WebGLRenderer: Error creating WebGL context with your selected attributes."):new Error("THREE.WebGLRenderer: Error creating WebGL context.")}}catch(v){throw _t("WebGLRenderer: "+v.message),v}let $e,E,p,z,q,J,me,ue,j,ee,fe,Pe,he,ve,Be,Ge,tt,U,ge,ie,xe,ae,se;function De(){$e=new Ep(L),$e.init(),xe=new mg(L,$e),E=new gp(L,$e,e,xe),p=new fg(L,$e),E.reversedDepthBuffer&&d&&p.buffers.depth.setReversed(!0),P=L.createFramebuffer(),W=L.createFramebuffer(),N=L.createFramebuffer(),z=new wp(L),q=new jm,J=new pg(L,$e,p,q,E,xe,z),me=new bp(k),ue=new Pu(L),ae=new pp(L,ue),j=new Tp(L,ue,z,ae),ee=new Cp(L,j,ue,ae,z),U=new Rp(L,E,J),Be=new _p(q),fe=new Qm(k,me,$e,E,ae,Be),Pe=new Sg(k,q),he=new tg,ve=new og($e),tt=new fp(k,me,p,ee,x,c),Ge=new hg(k,ee,E),se=new yg(L,z,E,p),ge=new mp(L,$e,z),ie=new Ap(L,$e,z),z.programs=fe.programs,k.capabilities=E,k.extensions=$e,k.properties=q,k.renderLists=he,k.shadowMap=Ge,k.state=p,k.info=z}De(),S!==yn&&(A=new Lp(S,t.width,t.height,o,r,s));const Me=new vg(k,L);this.xr=Me,this.getContext=function(){return L},this.getContextAttributes=function(){return L.getContextAttributes()},this.forceContextLoss=function(){const v=$e.get("WEBGL_lose_context");v&&v.loseContext()},this.forceContextRestore=function(){const v=$e.get("WEBGL_lose_context");v&&v.restoreContext()},this.getPixelRatio=function(){return le},this.setPixelRatio=function(v){v!==void 0&&(le=v,this.setSize(Z,de,!1))},this.getSize=function(v){return v.set(Z,de)},this.setSize=function(v,B,Y=!0){if(Me.isPresenting){je("WebGLRenderer: Can't change size while VR device is presenting.");return}Z=v,de=B,t.width=Math.floor(v*le),t.height=Math.floor(B*le),Y===!0&&(t.style.width=v+"px",t.style.height=B+"px"),A!==null&&A.setSize(t.width,t.height),this.setViewport(0,0,v,B)},this.getDrawingBufferSize=function(v){return v.set(Z*le,de*le).floor()},this.setDrawingBufferSize=function(v,B,Y){Z=v,de=B,le=Y,t.width=Math.floor(v*Y),t.height=Math.floor(B*Y),this.setViewport(0,0,v,B)},this.setEffects=function(v){if(S===yn){_t("WebGLRenderer: setEffects() requires outputBufferType set to HalfFloatType or FloatType.");return}if(v){for(let B=0;B<v.length;B++)if(v[B].isOutputPass===!0){je("WebGLRenderer: OutputPass is not needed in setEffects(). Tone mapping and color space conversion are applied automatically.");break}}A.setEffects(v||[])},this.getCurrentViewport=function(v){return v.copy(oe)},this.getViewport=function(v){return v.copy(Te)},this.setViewport=function(v,B,Y,V){v.isVector4?Te.set(v.x,v.y,v.z,v.w):Te.set(v,B,Y,V),p.viewport(oe.copy(Te).multiplyScalar(le).round())},this.getScissor=function(v){return v.copy(Q)},this.setScissor=function(v,B,Y,V){v.isVector4?Q.set(v.x,v.y,v.z,v.w):Q.set(v,B,Y,V),p.scissor(_e.copy(Q).multiplyScalar(le).round())},this.getScissorTest=function(){return te},this.setScissorTest=function(v){p.setScissorTest(te=v)},this.setOpaqueSort=function(v){Oe=v},this.setTransparentSort=function(v){Je=v},this.getClearColor=function(v){return v.copy(tt.getClearColor())},this.setClearColor=function(){tt.setClearColor(...arguments)},this.getClearAlpha=function(){return tt.getClearAlpha()},this.setClearAlpha=function(){tt.setClearAlpha(...arguments)},this.clear=function(v=!0,B=!0,Y=!0){let V=0;if(v){let X=!1;if(H!==null){const be=H.texture.format;X=g.has(be)}if(X){const be=H.texture.type,Ae=h.has(be),ye=tt.getClearColor(),Ie=tt.getClearAlpha(),Ne=ye.r,Xe=ye.g,st=ye.b;Ae?(R[0]=Ne,R[1]=Xe,R[2]=st,R[3]=Ie,L.clearBufferuiv(L.COLOR,0,R)):(C[0]=Ne,C[1]=Xe,C[2]=st,C[3]=Ie,L.clearBufferiv(L.COLOR,0,C))}else V|=L.COLOR_BUFFER_BIT}B&&(V|=L.DEPTH_BUFFER_BIT,this.state.buffers.depth.setMask(!0)),Y&&(V|=L.STENCIL_BUFFER_BIT,this.state.buffers.stencil.setMask(4294967295)),V!==0&&L.clear(V)},this.clearColor=function(){this.clear(!0,!1,!1)},this.clearDepth=function(){this.clear(!1,!0,!1)},this.clearStencil=function(){this.clear(!1,!1,!0)},this.setNodesHandler=function(v){v.setRenderer(this),b=v},this.dispose=function(){t.removeEventListener("webglcontextlost",At,!1),t.removeEventListener("webglcontextrestored",dt,!1),t.removeEventListener("webglcontextcreationerror",gn,!1),tt.dispose(),he.dispose(),ve.dispose(),q.dispose(),me.dispose(),ee.dispose(),ae.dispose(),se.dispose(),fe.dispose(),Me.dispose(),Me.removeEventListener("sessionstart",Sr),Me.removeEventListener("sessionend",In),an.stop()};function At(v){v.preventDefault(),Ps("WebGLRenderer: Context Lost."),F=!0}function dt(){Ps("WebGLRenderer: Context Restored."),F=!1;const v=z.autoReset,B=Ge.enabled,Y=Ge.autoUpdate,V=Ge.needsUpdate,X=Ge.type;De(),z.autoReset=v,Ge.enabled=B,Ge.autoUpdate=Y,Ge.needsUpdate=V,Ge.type=X}function gn(v){_t("WebGLRenderer: A WebGL context could not be created. Reason: ",v.statusMessage)}function dn(v){const B=v.target;B.removeEventListener("dispose",dn),ks(B)}function ks(v){Gs(v),q.remove(v)}function Gs(v){const B=q.get(v).programs;B!==void 0&&(B.forEach(function(Y){fe.releaseProgram(Y)}),v.isShaderMaterial&&fe.releaseShaderCache(v))}this.renderBufferDirect=function(v,B,Y,V,X,be){B===null&&(B=ot);const Ae=X.isMesh&&X.matrixWorld.determinantAffine()<0,ye=Nn(v,B,Y,V,X);p.setMaterial(V,Ae);let Ie=Y.index,Ne=1;if(V.wireframe===!0){if(Ie=j.getWireframeAttribute(Y),Ie===void 0)return;Ne=2}const Xe=Y.drawRange,st=Y.attributes.position;let Ue=Xe.start*Ne,xt=(Xe.start+Xe.count)*Ne;be!==null&&(Ue=Math.max(Ue,be.start*Ne),xt=Math.min(xt,(be.start+be.count)*Ne)),Ie!==null?(Ue=Math.max(Ue,0),xt=Math.min(xt,Ie.count)):st!=null&&(Ue=Math.max(Ue,0),xt=Math.min(xt,st.count));const Ut=xt-Ue;if(Ut<0||Ut===1/0)return;ae.setup(X,V,ye,Y,Ie);let vt,bt=ge;if(Ie!==null&&(vt=ue.get(Ie),bt=ie,bt.setIndex(vt)),X.isMesh)V.wireframe===!0?(p.setLineWidth(V.wireframeLinewidth*et()),bt.setMode(L.LINES)):bt.setMode(L.TRIANGLES);else if(X.isLine){let Bt=V.linewidth;Bt===void 0&&(Bt=1),p.setLineWidth(Bt*et()),X.isLineSegments?bt.setMode(L.LINES):X.isLineLoop?bt.setMode(L.LINE_LOOP):bt.setMode(L.LINE_STRIP)}else X.isPoints?bt.setMode(L.POINTS):X.isSprite&&bt.setMode(L.TRIANGLES);if(X.isBatchedMesh)if($e.get("WEBGL_multi_draw"))bt.renderMultiDraw(X._multiDrawStarts,X._multiDrawCounts,X._multiDrawCount);else{const Bt=X._multiDrawStarts,we=X._multiDrawCounts,Zt=X._multiDrawCount,pt=Ie?ue.get(Ie).bytesPerElement:1,$t=q.get(V).currentProgram.getUniforms();for(let _n=0;_n<Zt;_n++)$t.setValue(L,"_gl_DrawID",_n),bt.render(Bt[_n]/pt,we[_n])}else if(X.isInstancedMesh)bt.renderInstances(Ue,Ut,X.count);else if(Y.isInstancedBufferGeometry){const Bt=Y._maxInstanceCount!==void 0?Y._maxInstanceCount:1/0,we=Math.min(Y.instanceCount,Bt);bt.renderInstances(Ue,Ut,we)}else bt.render(Ue,Ut)};function Mr(v,B,Y){v.transparent===!0&&v.side===ni&&v.forceSinglePass===!1?(v.side=cn,v.needsUpdate=!0,nn(v,B,Y),v.side=yi,v.needsUpdate=!0,nn(v,B,Y),v.side=ni):nn(v,B,Y)}this.compile=function(v,B,Y=null){Y===null&&(Y=v),T=ve.get(Y),T.init(B),_.push(T),Y.traverseVisible(function(X){X.isLight&&X.layers.test(B.layers)&&(T.pushLight(X),X.castShadow&&T.pushShadow(X))}),v!==Y&&v.traverseVisible(function(X){X.isLight&&X.layers.test(B.layers)&&(T.pushLight(X),X.castShadow&&T.pushShadow(X))}),T.setupLights();const V=new Set;return v.traverse(function(X){if(!(X.isMesh||X.isPoints||X.isLine||X.isSprite))return;const be=X.material;if(be)if(Array.isArray(be))for(let Ae=0;Ae<be.length;Ae++){const ye=be[Ae];Mr(ye,Y,X),V.add(ye)}else Mr(be,Y,X),V.add(be)}),T=_.pop(),V},this.compileAsync=function(v,B,Y=null){const V=this.compile(v,B,Y);return new Promise(X=>{function be(){if(V.forEach(function(Ae){q.get(Ae).currentProgram.isReady()&&V.delete(Ae)}),V.size===0){X(v);return}setTimeout(be,10)}$e.get("KHR_parallel_shader_compile")!==null?be():setTimeout(be,10)})};let di=null;function Gr(v){di&&di(v)}function Sr(){an.stop()}function In(){an.start()}const an=new Tc;an.setAnimationLoop(Gr),typeof self<"u"&&an.setContext(self),this.setAnimationLoop=function(v){di=v,Me.setAnimationLoop(v),v===null?an.stop():an.start()},Me.addEventListener("sessionstart",Sr),Me.addEventListener("sessionend",In),this.render=function(v,B){if(B!==void 0&&B.isCamera!==!0){_t("WebGLRenderer.render: camera is not an instance of THREE.Camera.");return}if(F===!0)return;b!==null&&b.renderStart(v,B);const Y=Me.enabled===!0&&Me.isPresenting===!0,V=A!==null&&(H===null||Y)&&A.begin(k,H);if(v.matrixWorldAutoUpdate===!0&&v.updateMatrixWorld(),B.parent===null&&B.matrixWorldAutoUpdate===!0&&B.updateMatrixWorld(),Me.enabled===!0&&Me.isPresenting===!0&&(A===null||A.isCompositing()===!1)&&(Me.cameraAutoUpdate===!0&&Me.updateCamera(B),B=Me.getCamera()),v.isScene===!0&&v.onBeforeRender(k,v,B,H),T=ve.get(v,_.length),T.init(B),T.state.textureUnits=J.getTextureUnits(),_.push(T),We.multiplyMatrices(B.projectionMatrix,B.matrixWorldInverse),pe.setFromProjectionMatrix(We,Hn,B.reversedDepth),Ce=this.localClippingEnabled,Se=Be.init(this.clippingPlanes,Ce),w=he.get(v,I.length),w.init(),I.push(w),Me.enabled===!0&&Me.isPresenting===!0){const Ae=k.xr.getDepthSensingMesh();Ae!==null&&Vi(Ae,B,-1/0,k.sortObjects)}Vi(v,B,0,k.sortObjects),w.finish(),k.sortObjects===!0&&w.sort(Oe,Je,B.reversedDepth),Le=Me.enabled===!1||Me.isPresenting===!1||Me.hasDepthSensing()===!1,Le&&tt.addToRenderList(w,v),this.info.render.frame++,this.info.autoReset===!0&&this.info.reset(),Se===!0&&Be.beginShadows();const X=T.state.shadowsArray;if(Ge.render(X,v,B),Se===!0&&Be.endShadows(),(V&&A.hasRenderPass())===!1){const Ae=w.opaque,ye=w.transmissive;if(T.setupLights(),B.isArrayCamera){const Ie=B.cameras;if(ye.length>0)for(let Ne=0,Xe=Ie.length;Ne<Xe;Ne++){const st=Ie[Ne];bi(Ae,ye,v,st)}Le&&tt.render(v);for(let Ne=0,Xe=Ie.length;Ne<Xe;Ne++){const st=Ie[Ne];Hi(w,v,st,st.viewport)}}else ye.length>0&&bi(Ae,ye,v,B),Le&&tt.render(v),Hi(w,v,B)}H!==null&&G===0&&(J.updateMultisampleRenderTarget(H),J.updateRenderTargetMipmap(H)),V&&A.end(k),v.isScene===!0&&v.onAfterRender(k,v,B),ae.resetDefaultState(),ne=-1,ce=null,_.pop(),_.length>0?(T=_[_.length-1],J.setTextureUnits(T.state.textureUnits),Se===!0&&Be.setGlobalState(k.clippingPlanes,T.state.camera)):T=null,I.pop(),I.length>0?w=I[I.length-1]:w=null,b!==null&&b.renderEnd()};function Vi(v,B,Y,V){if(v.visible===!1)return;if(v.layers.test(B.layers)){if(v.isGroup)Y=v.renderOrder;else if(v.isLOD)v.autoUpdate===!0&&v.update(B);else if(v.isLightProbeGrid)T.pushLightProbeGrid(v);else if(v.isLight)T.pushLight(v),v.castShadow&&T.pushShadow(v);else if(v.isSprite){if(!v.frustumCulled||pe.intersectsSprite(v)){V&&ut.setFromMatrixPosition(v.matrixWorld).applyMatrix4(We);const Ae=ee.update(v),ye=v.material;ye.visible&&w.push(v,Ae,ye,Y,ut.z,null)}}else if((v.isMesh||v.isLine||v.isPoints)&&(!v.frustumCulled||pe.intersectsObject(v))){const Ae=ee.update(v),ye=v.material;if(V&&(v.boundingSphere!==void 0?(v.boundingSphere===null&&v.computeBoundingSphere(),ut.copy(v.boundingSphere.center)):(Ae.boundingSphere===null&&Ae.computeBoundingSphere(),ut.copy(Ae.boundingSphere.center)),ut.applyMatrix4(v.matrixWorld).applyMatrix4(We)),Array.isArray(ye)){const Ie=Ae.groups;for(let Ne=0,Xe=Ie.length;Ne<Xe;Ne++){const st=Ie[Ne],Ue=ye[st.materialIndex];Ue&&Ue.visible&&w.push(v,Ae,Ue,Y,ut.z,st)}}else ye.visible&&w.push(v,Ae,ye,Y,ut.z,null)}}const be=v.children;for(let Ae=0,ye=be.length;Ae<ye;Ae++)Vi(be[Ae],B,Y,V)}function Hi(v,B,Y,V){const{opaque:X,transmissive:be,transparent:Ae}=v;T.setupLightsView(Y),Se===!0&&Be.setGlobalState(k.clippingPlanes,Y),V&&p.viewport(oe.copy(V)),X.length>0&&Un(X,B,Y),be.length>0&&Un(be,B,Y),Ae.length>0&&Un(Ae,B,Y),p.buffers.depth.setTest(!0),p.buffers.depth.setMask(!0),p.buffers.color.setMask(!0),p.setPolygonOffset(!1)}function bi(v,B,Y,V){if((Y.isScene===!0?Y.overrideMaterial:null)!==null)return;if(T.state.transmissionRenderTarget[V.id]===void 0){const Ue=$e.has("EXT_color_buffer_half_float")||$e.has("EXT_color_buffer_float");T.state.transmissionRenderTarget[V.id]=new Wn(1,1,{generateMipmaps:!0,type:Ue?li:yn,minFilter:Ni,samples:Math.max(4,E.samples),stencilBuffer:s,resolveDepthBuffer:!1,resolveStencilBuffer:!1,colorSpace:mt.workingColorSpace})}const be=T.state.transmissionRenderTarget[V.id],Ae=V.viewport||oe;be.setSize(Ae.z*k.transmissionResolutionScale,Ae.w*k.transmissionResolutionScale);const ye=k.getRenderTarget(),Ie=k.getActiveCubeFace(),Ne=k.getActiveMipmapLevel();k.setRenderTarget(be),k.getClearColor(Ze),He=k.getClearAlpha(),He<1&&k.setClearColor(16777215,.5),k.clear(),Le&&tt.render(Y);const Xe=k.toneMapping;k.toneMapping=Pn;const st=V.viewport;if(V.viewport!==void 0&&(V.viewport=void 0),T.setupLightsView(V),Se===!0&&Be.setGlobalState(k.clippingPlanes,V),Un(v,Y,V),J.updateMultisampleRenderTarget(be),J.updateRenderTargetMipmap(be),$e.has("WEBGL_multisampled_render_to_texture")===!1){let Ue=!1;for(let xt=0,Ut=B.length;xt<Ut;xt++){const vt=B[xt],{object:bt,geometry:Bt,material:we,group:Zt}=vt;if(we.side===ni&&bt.layers.test(V.layers)){const pt=we.side;we.side=cn,we.needsUpdate=!0,Vr(bt,Y,V,Bt,we,Zt),we.side=pt,we.needsUpdate=!0,Ue=!0}}Ue===!0&&(J.updateMultisampleRenderTarget(be),J.updateRenderTargetMipmap(be))}k.setRenderTarget(ye,Ie,Ne),k.setClearColor(Ze,He),st!==void 0&&(V.viewport=st),k.toneMapping=Xe}function Un(v,B,Y){const V=B.isScene===!0?B.overrideMaterial:null;for(let X=0,be=v.length;X<be;X++){const Ae=v[X],{object:ye,geometry:Ie,group:Ne}=Ae;let Xe=Ae.material;Xe.allowOverride===!0&&V!==null&&(Xe=V),ye.layers.test(Y.layers)&&Vr(ye,B,Y,Ie,Xe,Ne)}}function Vr(v,B,Y,V,X,be){v.onBeforeRender(k,B,Y,V,X,be),v.modelViewMatrix.multiplyMatrices(Y.matrixWorldInverse,v.matrixWorld),v.normalMatrix.getNormalMatrix(v.modelViewMatrix),X.onBeforeRender(k,B,Y,V,v,be),X.transparent===!0&&X.side===ni&&X.forceSinglePass===!1?(X.side=cn,X.needsUpdate=!0,k.renderBufferDirect(Y,B,V,X,v,be),X.side=yi,X.needsUpdate=!0,k.renderBufferDirect(Y,B,V,X,v,be),X.side=ni):k.renderBufferDirect(Y,B,V,X,v,be),v.onAfterRender(k,B,Y,V,X,be)}function nn(v,B,Y){B.isScene!==!0&&(B=ot);const V=q.get(v),X=T.state.lights,be=T.state.shadowsArray,Ae=X.state.version,ye=fe.getParameters(v,X.state,be,B,Y,T.state.lightProbeGridArray),Ie=fe.getProgramCacheKey(ye);let Ne=V.programs;V.environment=v.isMeshStandardMaterial||v.isMeshLambertMaterial||v.isMeshPhongMaterial?B.environment:null,V.fog=B.fog;const Xe=v.isMeshStandardMaterial||v.isMeshLambertMaterial&&!v.envMap||v.isMeshPhongMaterial&&!v.envMap;V.envMap=me.get(v.envMap||V.environment,Xe),V.envMapRotation=V.environment!==null&&v.envMap===null?B.environmentRotation:v.envMapRotation,Ne===void 0&&(v.addEventListener("dispose",dn),Ne=new Map,V.programs=Ne);let st=Ne.get(Ie);if(st!==void 0){if(V.currentProgram===st&&V.lightsStateVersion===Ae)return un(v,ye),st}else ye.uniforms=fe.getUniforms(v),b!==null&&v.isNodeMaterial&&b.build(v,Y,ye),v.onBeforeCompile(ye,k),st=fe.acquireProgram(ye,Ie),Ne.set(Ie,st),V.uniforms=ye.uniforms;const Ue=V.uniforms;return(!v.isShaderMaterial&&!v.isRawShaderMaterial||v.clipping===!0)&&(Ue.clippingPlanes=Be.uniform),un(v,ye),V.needsLights=Wi(v),V.lightsStateVersion=Ae,V.needsLights&&(Ue.ambientLightColor.value=X.state.ambient,Ue.lightProbe.value=X.state.probe,Ue.directionalLights.value=X.state.directional,Ue.directionalLightShadows.value=X.state.directionalShadow,Ue.spotLights.value=X.state.spot,Ue.spotLightShadows.value=X.state.spotShadow,Ue.rectAreaLights.value=X.state.rectArea,Ue.ltc_1.value=X.state.rectAreaLTC1,Ue.ltc_2.value=X.state.rectAreaLTC2,Ue.pointLights.value=X.state.point,Ue.pointLightShadows.value=X.state.pointShadow,Ue.hemisphereLights.value=X.state.hemi,Ue.directionalShadowMatrix.value=X.state.directionalShadowMatrix,Ue.spotLightMatrix.value=X.state.spotLightMatrix,Ue.spotLightMap.value=X.state.spotLightMap,Ue.pointShadowMatrix.value=X.state.pointShadowMatrix),V.lightProbeGrid=T.state.lightProbeGridArray.length>0,V.currentProgram=st,V.uniformsList=null,st}function bn(v){if(v.uniformsList===null){const B=v.currentProgram.getUniforms();v.uniformsList=Es.seqWithValue(B.seq,v.uniforms)}return v.uniformsList}function un(v,B){const Y=q.get(v);Y.outputColorSpace=B.outputColorSpace,Y.batching=B.batching,Y.batchingColor=B.batchingColor,Y.instancing=B.instancing,Y.instancingColor=B.instancingColor,Y.instancingMorph=B.instancingMorph,Y.skinning=B.skinning,Y.morphTargets=B.morphTargets,Y.morphNormals=B.morphNormals,Y.morphColors=B.morphColors,Y.morphTargetsCount=B.morphTargetsCount,Y.numClippingPlanes=B.numClippingPlanes,Y.numIntersection=B.numClipIntersection,Y.vertexAlphas=B.vertexAlphas,Y.vertexTangents=B.vertexTangents,Y.toneMapping=B.toneMapping}function Vs(v,B){if(v.length===0)return null;if(v.length===1)return v[0].texture!==null?v[0]:null;y.setFromMatrixPosition(B.matrixWorld);for(let Y=0,V=v.length;Y<V;Y++){const X=v[Y];if(X.texture!==null&&X.boundingBox.containsPoint(y))return X}return null}function Nn(v,B,Y,V,X){B.isScene!==!0&&(B=ot),J.resetTextureUnits();const be=B.fog,Ae=V.isMeshStandardMaterial||V.isMeshLambertMaterial||V.isMeshPhongMaterial?B.environment:null,ye=H===null?k.outputColorSpace:H.isXRRenderTarget===!0?H.texture.colorSpace:mt.workingColorSpace,Ie=V.isMeshStandardMaterial||V.isMeshLambertMaterial&&!V.envMap||V.isMeshPhongMaterial&&!V.envMap,Ne=me.get(V.envMap||Ae,Ie),Xe=V.vertexColors===!0&&!!Y.attributes.color&&Y.attributes.color.itemSize===4,st=!!Y.attributes.tangent&&(!!V.normalMap||V.anisotropy>0),Ue=!!Y.morphAttributes.position,xt=!!Y.morphAttributes.normal,Ut=!!Y.morphAttributes.color;let vt=Pn;V.toneMapped&&(H===null||H.isXRRenderTarget===!0)&&(vt=k.toneMapping);const bt=Y.morphAttributes.position||Y.morphAttributes.normal||Y.morphAttributes.color,Bt=bt!==void 0?bt.length:0,we=q.get(V),Zt=T.state.lights;if(Se===!0&&(Ce===!0||v!==ce)){const Rt=v===ce&&V.id===ne;Be.setState(V,v,Rt)}let pt=!1;V.version===we.__version?(we.needsLights&&we.lightsStateVersion!==Zt.state.version||we.outputColorSpace!==ye||X.isBatchedMesh&&we.batching===!1||!X.isBatchedMesh&&we.batching===!0||X.isBatchedMesh&&we.batchingColor===!0&&X.colorTexture===null||X.isBatchedMesh&&we.batchingColor===!1&&X.colorTexture!==null||X.isInstancedMesh&&we.instancing===!1||!X.isInstancedMesh&&we.instancing===!0||X.isSkinnedMesh&&we.skinning===!1||!X.isSkinnedMesh&&we.skinning===!0||X.isInstancedMesh&&we.instancingColor===!0&&X.instanceColor===null||X.isInstancedMesh&&we.instancingColor===!1&&X.instanceColor!==null||X.isInstancedMesh&&we.instancingMorph===!0&&X.morphTexture===null||X.isInstancedMesh&&we.instancingMorph===!1&&X.morphTexture!==null||we.envMap!==Ne||V.fog===!0&&we.fog!==be||we.numClippingPlanes!==void 0&&(we.numClippingPlanes!==Be.numPlanes||we.numIntersection!==Be.numIntersection)||we.vertexAlphas!==Xe||we.vertexTangents!==st||we.morphTargets!==Ue||we.morphNormals!==xt||we.morphColors!==Ut||we.toneMapping!==vt||we.morphTargetsCount!==Bt||!!we.lightProbeGrid!=T.state.lightProbeGridArray.length>0)&&(pt=!0):(pt=!0,we.__version=V.version);let $t=we.currentProgram;pt===!0&&($t=nn(V,B,X),b&&V.isNodeMaterial&&b.onUpdateProgram(V,$t,we));let _n=!1,Fn=!1,ui=!1;const wt=$t.getUniforms(),Dt=we.uniforms;if(p.useProgram($t.program)&&(_n=!0,Fn=!0,ui=!0),V.id!==ne&&(ne=V.id,Fn=!0),we.needsLights){const Rt=Vs(T.state.lightProbeGridArray,X);we.lightProbeGrid!==Rt&&(we.lightProbeGrid=Rt,Fn=!0)}if(_n||ce!==v){p.buffers.depth.getReversed()&&v.reversedDepth!==!0&&(v._reversedDepth=!0,v.updateProjectionMatrix()),wt.setValue(L,"projectionMatrix",v.projectionMatrix),wt.setValue(L,"viewMatrix",v.matrixWorldInverse);const lt=wt.map.cameraPosition;lt!==void 0&&lt.setValue(L,qe.setFromMatrixPosition(v.matrixWorld)),E.logarithmicDepthBuffer&&wt.setValue(L,"logDepthBufFC",2/(Math.log(v.far+1)/Math.LN2)),(V.isMeshPhongMaterial||V.isMeshToonMaterial||V.isMeshLambertMaterial||V.isMeshBasicMaterial||V.isMeshStandardMaterial||V.isShaderMaterial)&&wt.setValue(L,"isOrthographic",v.isOrthographicCamera===!0),ce!==v&&(ce=v,Fn=!0,ui=!0)}if(we.needsLights&&(Zt.state.directionalShadowMap.length>0&&wt.setValue(L,"directionalShadowMap",Zt.state.directionalShadowMap,J),Zt.state.spotShadowMap.length>0&&wt.setValue(L,"spotShadowMap",Zt.state.spotShadowMap,J),Zt.state.pointShadowMap.length>0&&wt.setValue(L,"pointShadowMap",Zt.state.pointShadowMap,J)),X.isSkinnedMesh){wt.setOptional(L,X,"bindMatrix"),wt.setOptional(L,X,"bindMatrixInverse");const Rt=X.skeleton;Rt&&(Rt.boneTexture===null&&Rt.computeBoneTexture(),wt.setValue(L,"boneTexture",Rt.boneTexture,J))}X.isBatchedMesh&&(wt.setOptional(L,X,"batchingTexture"),wt.setValue(L,"batchingTexture",X._matricesTexture,J),wt.setOptional(L,X,"batchingIdTexture"),wt.setValue(L,"batchingIdTexture",X._indirectTexture,J),wt.setOptional(L,X,"batchingColorTexture"),X._colorsTexture!==null&&wt.setValue(L,"batchingColorTexture",X._colorsTexture,J));const xn=Y.morphAttributes;if((xn.position!==void 0||xn.normal!==void 0||xn.color!==void 0)&&U.update(X,Y,$t),(Fn||we.receiveShadow!==X.receiveShadow)&&(we.receiveShadow=X.receiveShadow,wt.setValue(L,"receiveShadow",X.receiveShadow)),(V.isMeshStandardMaterial||V.isMeshLambertMaterial||V.isMeshPhongMaterial)&&V.envMap===null&&B.environment!==null&&(Dt.envMapIntensity.value=B.environmentIntensity),Dt.dfgLUT!==void 0&&(Dt.dfgLUT.value=Eg()),Fn){if(wt.setValue(L,"toneMappingExposure",k.toneMappingExposure),we.needsLights&&Ei(Dt,ui),be&&V.fog===!0&&Pe.refreshFogUniforms(Dt,be),Pe.refreshMaterialUniforms(Dt,V,le,de,T.state.transmissionRenderTarget[v.id]),we.needsLights&&we.lightProbeGrid){const Rt=we.lightProbeGrid;Dt.probesSH.value=Rt.texture,Dt.probesMin.value.copy(Rt.boundingBox.min),Dt.probesMax.value.copy(Rt.boundingBox.max),Dt.probesResolution.value.copy(Rt.resolution)}Es.upload(L,bn(we),Dt,J)}if(V.isShaderMaterial&&V.uniformsNeedUpdate===!0&&(Es.upload(L,bn(we),Dt,J),V.uniformsNeedUpdate=!1),V.isSpriteMaterial&&wt.setValue(L,"center",X.center),wt.setValue(L,"modelViewMatrix",X.modelViewMatrix),wt.setValue(L,"normalMatrix",X.normalMatrix),wt.setValue(L,"modelMatrix",X.matrixWorld),V.uniformsGroups!==void 0){const Rt=V.uniformsGroups;for(let lt=0,Ti=Rt.length;lt<Ti;lt++){const M=Rt[lt];se.update(M,$t),se.bind(M,$t)}}return $t}function Ei(v,B){v.ambientLightColor.needsUpdate=B,v.lightProbe.needsUpdate=B,v.directionalLights.needsUpdate=B,v.directionalLightShadows.needsUpdate=B,v.pointLights.needsUpdate=B,v.pointLightShadows.needsUpdate=B,v.spotLights.needsUpdate=B,v.spotLightShadows.needsUpdate=B,v.rectAreaLights.needsUpdate=B,v.hemisphereLights.needsUpdate=B}function Wi(v){return v.isMeshLambertMaterial||v.isMeshToonMaterial||v.isMeshPhongMaterial||v.isMeshStandardMaterial||v.isShadowMaterial||v.isShaderMaterial&&v.lights===!0}this.getActiveCubeFace=function(){return D},this.getActiveMipmapLevel=function(){return G},this.getRenderTarget=function(){return H},this.setRenderTargetTextures=function(v,B,Y){const V=q.get(v);V.__autoAllocateDepthBuffer=v.resolveDepthBuffer===!1,V.__autoAllocateDepthBuffer===!1&&(V.__useRenderToTexture=!1),q.get(v.texture).__webglTexture=B,q.get(v.depthTexture).__webglTexture=V.__autoAllocateDepthBuffer?void 0:Y,V.__hasExternalTextures=!0},this.setRenderTargetFramebuffer=function(v,B){const Y=q.get(v);Y.__webglFramebuffer=B,Y.__useDefaultFramebuffer=B===void 0},this.setRenderTarget=function(v,B=0,Y=0){H=v,D=B,G=Y;let V=null,X=!1,be=!1;if(v){const ye=q.get(v);if(ye.__useDefaultFramebuffer!==void 0){p.bindFramebuffer(L.FRAMEBUFFER,ye.__webglFramebuffer),oe.copy(v.viewport),_e.copy(v.scissor),Qe=v.scissorTest,p.viewport(oe),p.scissor(_e),p.setScissorTest(Qe),ne=-1;return}else if(ye.__webglFramebuffer===void 0)J.setupRenderTarget(v);else if(ye.__hasExternalTextures)J.rebindTextures(v,q.get(v.texture).__webglTexture,q.get(v.depthTexture).__webglTexture);else if(v.depthBuffer){const Xe=v.depthTexture;if(ye.__boundDepthTexture!==Xe){if(Xe!==null&&q.has(Xe)&&(v.width!==Xe.image.width||v.height!==Xe.image.height))throw new Error("THREE.WebGLRenderer: Attached DepthTexture is initialized to the incorrect size.");J.setupDepthRenderbuffer(v)}}const Ie=v.texture;(Ie.isData3DTexture||Ie.isDataArrayTexture||Ie.isCompressedArrayTexture)&&(be=!0);const Ne=q.get(v).__webglFramebuffer;v.isWebGLCubeRenderTarget?(Array.isArray(Ne[B])?V=Ne[B][Y]:V=Ne[B],X=!0):v.samples>0&&J.useMultisampledRTT(v)===!1?V=q.get(v).__webglMultisampledFramebuffer:Array.isArray(Ne)?V=Ne[Y]:V=Ne,oe.copy(v.viewport),_e.copy(v.scissor),Qe=v.scissorTest}else oe.copy(Te).multiplyScalar(le).floor(),_e.copy(Q).multiplyScalar(le).floor(),Qe=te;if(Y!==0&&(V=P),p.bindFramebuffer(L.FRAMEBUFFER,V)&&p.drawBuffers(v,V),p.viewport(oe),p.scissor(_e),p.setScissorTest(Qe),X){const ye=q.get(v.texture);L.framebufferTexture2D(L.FRAMEBUFFER,L.COLOR_ATTACHMENT0,L.TEXTURE_CUBE_MAP_POSITIVE_X+B,ye.__webglTexture,Y)}else if(be){const ye=B;for(let Ie=0;Ie<v.textures.length;Ie++){const Ne=q.get(v.textures[Ie]);L.framebufferTextureLayer(L.FRAMEBUFFER,L.COLOR_ATTACHMENT0+Ie,Ne.__webglTexture,Y,ye)}}else if(v!==null&&Y!==0){const ye=q.get(v.texture);L.framebufferTexture2D(L.FRAMEBUFFER,L.COLOR_ATTACHMENT0,L.TEXTURE_2D,ye.__webglTexture,Y)}ne=-1},this.readRenderTargetPixels=function(v,B,Y,V,X,be,Ae,ye=0){if(!(v&&v.isWebGLRenderTarget)){_t("WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");return}let Ie=q.get(v).__webglFramebuffer;if(v.isWebGLCubeRenderTarget&&Ae!==void 0&&(Ie=Ie[Ae]),Ie){p.bindFramebuffer(L.FRAMEBUFFER,Ie);try{const Ne=v.textures[ye],Xe=Ne.format,st=Ne.type;if(v.textures.length>1&&L.readBuffer(L.COLOR_ATTACHMENT0+ye),!E.textureFormatReadable(Xe)){_t("WebGLRenderer.readRenderTargetPixels: renderTarget is not in RGBA or implementation defined format.");return}if(!E.textureTypeReadable(st)){_t("WebGLRenderer.readRenderTargetPixels: renderTarget is not in UnsignedByteType or implementation defined type.");return}B>=0&&B<=v.width-V&&Y>=0&&Y<=v.height-X&&L.readPixels(B,Y,V,X,xe.convert(Xe),xe.convert(st),be)}finally{const Ne=H!==null?q.get(H).__webglFramebuffer:null;p.bindFramebuffer(L.FRAMEBUFFER,Ne)}}},this.readRenderTargetPixelsAsync=async function(v,B,Y,V,X,be,Ae,ye=0){if(!(v&&v.isWebGLRenderTarget))throw new Error("THREE.WebGLRenderer.readRenderTargetPixels: renderTarget is not THREE.WebGLRenderTarget.");let Ie=q.get(v).__webglFramebuffer;if(v.isWebGLCubeRenderTarget&&Ae!==void 0&&(Ie=Ie[Ae]),Ie)if(B>=0&&B<=v.width-V&&Y>=0&&Y<=v.height-X){p.bindFramebuffer(L.FRAMEBUFFER,Ie);const Ne=v.textures[ye],Xe=Ne.format,st=Ne.type;if(v.textures.length>1&&L.readBuffer(L.COLOR_ATTACHMENT0+ye),!E.textureFormatReadable(Xe))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in RGBA or implementation defined format.");if(!E.textureTypeReadable(st))throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: renderTarget is not in UnsignedByteType or implementation defined type.");const Ue=L.createBuffer();L.bindBuffer(L.PIXEL_PACK_BUFFER,Ue),L.bufferData(L.PIXEL_PACK_BUFFER,be.byteLength,L.STREAM_READ),L.readPixels(B,Y,V,X,xe.convert(Xe),xe.convert(st),0);const xt=H!==null?q.get(H).__webglFramebuffer:null;p.bindFramebuffer(L.FRAMEBUFFER,xt);const Ut=L.fenceSync(L.SYNC_GPU_COMMANDS_COMPLETE,0);return L.flush(),await Xd(L,Ut,4),L.bindBuffer(L.PIXEL_PACK_BUFFER,Ue),L.getBufferSubData(L.PIXEL_PACK_BUFFER,0,be),L.deleteBuffer(Ue),L.deleteSync(Ut),be}else throw new Error("THREE.WebGLRenderer.readRenderTargetPixelsAsync: requested read bounds are out of range.")},this.copyFramebufferToTexture=function(v,B=null,Y=0){const V=Math.pow(2,-Y),X=Math.floor(v.image.width*V),be=Math.floor(v.image.height*V),Ae=B!==null?B.x:0,ye=B!==null?B.y:0;J.setTexture2D(v,0),L.copyTexSubImage2D(L.TEXTURE_2D,Y,0,0,Ae,ye,X,be),p.unbindTexture()},this.copyTextureToTexture=function(v,B,Y=null,V=null,X=0,be=0){let Ae,ye,Ie,Ne,Xe,st,Ue,xt,Ut;const vt=v.isCompressedTexture?v.mipmaps[be]:v.image;if(Y!==null)Ae=Y.max.x-Y.min.x,ye=Y.max.y-Y.min.y,Ie=Y.isBox3?Y.max.z-Y.min.z:1,Ne=Y.min.x,Xe=Y.min.y,st=Y.isBox3?Y.min.z:0;else{const Dt=Math.pow(2,-X);Ae=Math.floor(vt.width*Dt),ye=Math.floor(vt.height*Dt),v.isDataArrayTexture?Ie=vt.depth:v.isData3DTexture?Ie=Math.floor(vt.depth*Dt):Ie=1,Ne=0,Xe=0,st=0}V!==null?(Ue=V.x,xt=V.y,Ut=V.z):(Ue=0,xt=0,Ut=0);const bt=xe.convert(B.format),Bt=xe.convert(B.type);let we;B.isData3DTexture?(J.setTexture3D(B,0),we=L.TEXTURE_3D):B.isDataArrayTexture||B.isCompressedArrayTexture?(J.setTexture2DArray(B,0),we=L.TEXTURE_2D_ARRAY):(J.setTexture2D(B,0),we=L.TEXTURE_2D),p.activeTexture(L.TEXTURE0),p.pixelStorei(L.UNPACK_FLIP_Y_WEBGL,B.flipY),p.pixelStorei(L.UNPACK_PREMULTIPLY_ALPHA_WEBGL,B.premultiplyAlpha),p.pixelStorei(L.UNPACK_ALIGNMENT,B.unpackAlignment);const Zt=p.getParameter(L.UNPACK_ROW_LENGTH),pt=p.getParameter(L.UNPACK_IMAGE_HEIGHT),$t=p.getParameter(L.UNPACK_SKIP_PIXELS),_n=p.getParameter(L.UNPACK_SKIP_ROWS),Fn=p.getParameter(L.UNPACK_SKIP_IMAGES);p.pixelStorei(L.UNPACK_ROW_LENGTH,vt.width),p.pixelStorei(L.UNPACK_IMAGE_HEIGHT,vt.height),p.pixelStorei(L.UNPACK_SKIP_PIXELS,Ne),p.pixelStorei(L.UNPACK_SKIP_ROWS,Xe),p.pixelStorei(L.UNPACK_SKIP_IMAGES,st);const ui=v.isDataArrayTexture||v.isData3DTexture,wt=B.isDataArrayTexture||B.isData3DTexture;if(v.isDepthTexture){const Dt=q.get(v),xn=q.get(B),Rt=q.get(Dt.__renderTarget),lt=q.get(xn.__renderTarget);p.bindFramebuffer(L.READ_FRAMEBUFFER,Rt.__webglFramebuffer),p.bindFramebuffer(L.DRAW_FRAMEBUFFER,lt.__webglFramebuffer);for(let Ti=0;Ti<Ie;Ti++)ui&&(L.framebufferTextureLayer(L.READ_FRAMEBUFFER,L.COLOR_ATTACHMENT0,q.get(v).__webglTexture,X,st+Ti),L.framebufferTextureLayer(L.DRAW_FRAMEBUFFER,L.COLOR_ATTACHMENT0,q.get(B).__webglTexture,be,Ut+Ti)),L.blitFramebuffer(Ne,Xe,Ae,ye,Ue,xt,Ae,ye,L.DEPTH_BUFFER_BIT,L.NEAREST);p.bindFramebuffer(L.READ_FRAMEBUFFER,null),p.bindFramebuffer(L.DRAW_FRAMEBUFFER,null)}else if(X!==0||v.isRenderTargetTexture||q.has(v)){const Dt=q.get(v),xn=q.get(B);p.bindFramebuffer(L.READ_FRAMEBUFFER,W),p.bindFramebuffer(L.DRAW_FRAMEBUFFER,N);for(let Rt=0;Rt<Ie;Rt++)ui?L.framebufferTextureLayer(L.READ_FRAMEBUFFER,L.COLOR_ATTACHMENT0,Dt.__webglTexture,X,st+Rt):L.framebufferTexture2D(L.READ_FRAMEBUFFER,L.COLOR_ATTACHMENT0,L.TEXTURE_2D,Dt.__webglTexture,X),wt?L.framebufferTextureLayer(L.DRAW_FRAMEBUFFER,L.COLOR_ATTACHMENT0,xn.__webglTexture,be,Ut+Rt):L.framebufferTexture2D(L.DRAW_FRAMEBUFFER,L.COLOR_ATTACHMENT0,L.TEXTURE_2D,xn.__webglTexture,be),X!==0?L.blitFramebuffer(Ne,Xe,Ae,ye,Ue,xt,Ae,ye,L.COLOR_BUFFER_BIT,L.NEAREST):wt?L.copyTexSubImage3D(we,be,Ue,xt,Ut+Rt,Ne,Xe,Ae,ye):L.copyTexSubImage2D(we,be,Ue,xt,Ne,Xe,Ae,ye);p.bindFramebuffer(L.READ_FRAMEBUFFER,null),p.bindFramebuffer(L.DRAW_FRAMEBUFFER,null)}else wt?v.isDataTexture||v.isData3DTexture?L.texSubImage3D(we,be,Ue,xt,Ut,Ae,ye,Ie,bt,Bt,vt.data):B.isCompressedArrayTexture?L.compressedTexSubImage3D(we,be,Ue,xt,Ut,Ae,ye,Ie,bt,vt.data):L.texSubImage3D(we,be,Ue,xt,Ut,Ae,ye,Ie,bt,Bt,vt):v.isDataTexture?L.texSubImage2D(L.TEXTURE_2D,be,Ue,xt,Ae,ye,bt,Bt,vt.data):v.isCompressedTexture?L.compressedTexSubImage2D(L.TEXTURE_2D,be,Ue,xt,vt.width,vt.height,bt,vt.data):L.texSubImage2D(L.TEXTURE_2D,be,Ue,xt,Ae,ye,bt,Bt,vt);p.pixelStorei(L.UNPACK_ROW_LENGTH,Zt),p.pixelStorei(L.UNPACK_IMAGE_HEIGHT,pt),p.pixelStorei(L.UNPACK_SKIP_PIXELS,$t),p.pixelStorei(L.UNPACK_SKIP_ROWS,_n),p.pixelStorei(L.UNPACK_SKIP_IMAGES,Fn),be===0&&B.generateMipmaps&&L.generateMipmap(we),p.unbindTexture()},this.initRenderTarget=function(v){q.get(v).__webglFramebuffer===void 0&&J.setupRenderTarget(v)},this.initTexture=function(v){v.isCubeTexture?J.setTextureCube(v,0):v.isData3DTexture?J.setTexture3D(v,0):v.isDataArrayTexture||v.isCompressedArrayTexture?J.setTexture2DArray(v,0):J.setTexture2D(v,0),p.unbindTexture()},this.resetState=function(){D=0,G=0,H=null,p.reset(),ae.reset()},typeof __THREE_DEVTOOLS__<"u"&&__THREE_DEVTOOLS__.dispatchEvent(new CustomEvent("observe",{detail:this}))}get coordinateSystem(){return Hn}get outputColorSpace(){return this._outputColorSpace}set outputColorSpace(e){this._outputColorSpace=e;const t=this.getContext();t.drawingBufferColorSpace=mt._getDrawingBufferColorSpace(e),t.unpackColorSpace=mt._getUnpackColorSpace()}}const ps=11,Ag=[0,2,1,0,3,2],wg=`
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
}`,Rg=`
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
}`,ba=new Map;function Ic(i){if(ba.has(i))return ba.get(i);const e=i.replace("#","");if(![3,4,6,8].includes(e.length)||!/^[a-f0-9]+$/i.test(e))throw new Error("Unsupported scene color: "+i);const t=e.length<5?[...e].map(r=>r+r).join(""):e,n=[0,2,4].map(r=>parseInt(t.slice(r,r+2),16)/255);return n.push(t.length===8?parseInt(t.slice(6,8),16)/255:1),ba.set(i,n),n}class Ol{constructor(e,t){this.a=e,this.b=t,this.stops=[]}addColorStop(e,t){this.stops.push([e,Ic(t)]),this.stops.sort((n,r)=>n[0]-r[0])}at(e,t){const n=this.b[0]-this.a[0],r=this.b[1]-this.a[1],s=Math.max(0,Math.min(1,((e-this.a[0])*n+(t-this.a[1])*r)/(n*n+r*r||1))),a=this.stops[0],o=this.stops.at(-1);return a[1].map((c,l)=>c+(o[1][l]-c)*s)}}class Cg{constructor(e){this.canvas=e,this.renderer=new Tg({canvas:e,alpha:!1,antialias:!1,depth:!1,stencil:!1,premultipliedAlpha:!0}),this.renderer.setClearColor(0,1),this.renderer.outputColorSpace=Or,this.renderer.toneMapping=Pn,this.renderer.sortObjects=!1,this.measure=document.createElement("canvas").getContext("2d"),this.atlas=document.createElement("canvas"),this.atlas.width=this.atlas.height=2048,this.atlasContext=this.atlas.getContext("2d"),this.atlasTexture=new vu(this.atlas),this.atlasTexture.colorSpace=ii,this.atlasTexture.premultiplyAlpha=!0,this.atlasTexture.generateMipmaps=!1,this.atlasTexture.minFilter=this.atlasTexture.magFilter=Qt,this.images=new Map,this.shelfX=2,this.shelfY=2,this.shelfHeight=0,this.resolution=new Mt(1,1),this.materials=["source-over","screen"].map(t=>new bc({name:"Sophia "+t,vertexShader:wg,fragmentShader:Rg,glslVersion:mo,uniforms:{uResolution:{value:this.resolution},uAtlas:{value:this.atlasTexture}},transparent:!0,depthTest:!1,depthWrite:!1,toneMapped:!1,blending:Kl,blendEquation:vi,blendSrc:Aa,blendDst:t==="screen"?Zl:Ur,blendSrcAlpha:Aa,blendDstAlpha:Ur})),this.scene=new ou,this.camera=new Io,this.capacity=0,this.mesh=null,this.allocate(16384),this.matrix=[1,0,0,1,0,0],this.stack=[],this.path=[],this.quadPoints=Array.from({length:4},()=>[0,0]),this.quadLocals=Array.from({length:4},()=>[0,0]),this.quadShape=[0,0,0],this.globalAlpha=1,this.globalCompositeOperation="source-over",this.fillStyle="#000000",this.strokeStyle="#000000",this.lineWidth=1,this.lost=!1,this.disposed=!1,e.addEventListener("webglcontextlost",t=>{t.preventDefault(),this.lost=!0,e.dataset.context="lost"}),e.addEventListener("webglcontextrestored",()=>{this.lost=!1,this.atlasTexture.needsUpdate=!0,e.dataset.context="restored",e.dispatchEvent(new CustomEvent("sophia-renderer-restored"))}),addEventListener("pagehide",t=>{t.persisted||this.dispose()})}get font(){return this.measure.font}set font(e){this.measure.font=e}measureText(e){return this.measure.measureText(e)}point(e,t){const n=this.matrix;return[n[0]*e+n[2]*t+n[4],n[1]*e+n[3]*t+n[5]]}setTransform(...e){this.matrix=e}translate(e,t){const n=this.point(e,t);this.matrix[4]=n[0],this.matrix[5]=n[1]}rotate(e){const[t,n,r,s,a,o]=this.matrix,c=Math.cos(e),l=Math.sin(e);this.matrix=[t*c+r*l,n*c+s*l,r*c-t*l,s*c-n*l,a,o]}save(){this.stack.push({matrix:this.matrix.slice(),globalAlpha:this.globalAlpha,globalCompositeOperation:this.globalCompositeOperation,fillStyle:this.fillStyle,strokeStyle:this.strokeStyle,lineWidth:this.lineWidth})}restore(){const e=this.stack.pop();e&&Object.assign(this,e)}createLinearGradient(e,t,n,r){return new Ol(this.point(e,t),this.point(n,r))}allocate(e){const t=this.data;this.geometry?.dispose(),this.capacity=e,this.data=new Float32Array(e*ps),t&&this.data.set(t),this.buffer=new uu(this.data,ps).setUsage(Vd),this.geometry=new Kn;for(const[n,r,s]of[["aPosition",2,0],["aLocal",2,2],["aColor",4,4],["aShape",3,8]])this.geometry.setAttribute(n,new Do(this.buffer,r,s));this.mesh?this.mesh.geometry=this.geometry:(this.mesh=new qn(this.geometry,this.materials),this.mesh.frustumCulled=!1,this.scene.add(this.mesh))}beginFrame(){this.used=0,this.groups=[],(this.resolution.x!==this.canvas.width||this.resolution.y!==this.canvas.height)&&(this.resolution.set(this.canvas.width,this.canvas.height),this.renderer.setSize(this.canvas.width,this.canvas.height,!1))}endFrame(){this.lost||this.disposed||(this.geometry.clearGroups(),this.groups.forEach(e=>this.geometry.addGroup(e.start,e.count,e.material)),this.geometry.setDrawRange(0,this.used),this.buffer.clearUpdateRanges(),this.buffer.addUpdateRange(0,this.used*ps),this.buffer.needsUpdate=!0,this.renderer.render(this.scene,this.camera),this.canvas.dataset.drawCalls=String(this.renderer.info.render.calls),this.canvas.dataset.gpuVertices=String(this.used),this.canvas.dataset.gpuTextures=String(this.renderer.info.memory.textures))}quad(e,t,n,r,s=this.globalAlpha){this.used+6>this.capacity&&this.allocate(this.capacity*2);const a=this.globalCompositeOperation==="screen"?1:0;let o=this.groups.at(-1);(!o||o.material!==a)&&(o={start:this.used,count:0,material:a},this.groups.push(o)),o.count+=6;const c=r instanceof Ol?null:Ic(r);for(const l of Ag){const u=e[l],f=t[l],d=c||r.at(...u),m=d[3]*s,x=this.used++*ps;this.data[x]=u[0],this.data[x+1]=u[1],this.data[x+2]=f[0],this.data[x+3]=f[1],this.data[x+4]=d[0]*m,this.data[x+5]=d[1]*m,this.data[x+6]=d[2]*m,this.data[x+7]=m,this.data[x+8]=n[0],this.data[x+9]=n[1],this.data[x+10]=n[2]}}atlasEntry(e){if(this.images.has(e))return this.images.get(e);const t=e.width,n=e.height;if(this.shelfX+t+2>2048&&(this.shelfX=2,this.shelfY+=this.shelfHeight+4,this.shelfHeight=0),this.shelfY+n+2>2048)throw new Error("Scene texture atlas budget exceeded");const r={x:this.shelfX,y:this.shelfY,width:t,height:n};return this.shelfX+=t+4,this.shelfHeight=Math.max(this.shelfHeight,n),this.images.set(e,r),this.invalidateImage(e),r}invalidateImage(e){const t=this.images.get(e);if(!t)return;const n=this.atlasContext,{x:r,y:s,width:a,height:o}=t;n.clearRect(r-1,s-1,a+2,o+2),n.drawImage(e,r,s),n.drawImage(e,0,0,a,1,r,s-1,a,1),n.drawImage(e,0,o-1,a,1,r,s+o,a,1),n.drawImage(e,0,0,1,o,r-1,s,1,o),n.drawImage(e,a-1,0,1,o,r+a,s,1,o);for(const[c,l,u,f]of[[0,0,r-1,s-1],[a-1,0,r+a,s-1],[0,o-1,r-1,s+o],[a-1,o-1,r+a,s+o]])n.drawImage(e,c,l,1,1,u,f,1,1);this.atlasTexture.needsUpdate=!0}drawImage(e,t,n,r,s){const a=this.atlasEntry(e),o=a.x/2048,c=1-a.y/2048,l=a.width/2048,u=a.height/2048,f=this.matrix,d=this.quadPoints,m=this.quadLocals,x=this.quadShape;for(let S=0;S<4;S++){const g=S===1||S===2,h=S>=2,R=g?t+r:t,C=h?n+s:n;d[S][0]=f[0]*R+f[2]*C+f[4],d[S][1]=f[1]*R+f[3]*C+f[5],m[S][0]=g?o+l:o,m[S][1]=h?c-u:c}x[0]=x[1]=x[2]=0,this.quad(d,m,x,"#ffffff")}rectAt(e,t,n,r,s,a,o){const c=s+1,l=a+1,u=this.quadPoints,f=this.quadLocals,d=this.quadShape;for(let m=0;m<4;m++){const x=m===1||m===2?c:-c,S=m>=2?l:-l;f[m][0]=x,f[m][1]=S,u[m][0]=e+n*x-r*S,u[m][1]=t+r*x+n*S}d[0]=1,d[1]=s,d[2]=a,this.quad(u,f,d,o)}fillRect(e,t,n,r){const s=this.matrix,a=Math.hypot(s[0],s[1]),o=Math.hypot(s[2],s[3]),c=e+n/2,l=t+r/2;this.rectAt(s[0]*c+s[2]*l+s[4],s[1]*c+s[3]*l+s[5],s[0]/a,s[1]/a,Math.abs(n*a/2),Math.abs(r*o/2),this.fillStyle)}beginPath(){this.path=[],this.current=null}moveTo(e,t){this.current=this.point(e,t)}lineTo(e,t){const n=this.point(e,t);this.current&&this.path.push({type:"line",a:this.current,b:n}),this.current=n}arc(e,t,n,r,s){if(Math.abs(s-r-Math.PI*2)>1e-4)throw new Error("Only full scene discs are supported");this.path.push({type:"disc",center:this.point(e,t),radius:n*Math.hypot(this.matrix[0],this.matrix[1])})}fill(){for(const e of this.path){if(e.type!=="disc")throw new Error("Only scene discs may be filled");const t=e.radius+1,n=[[-t,-t],[t,-t],[t,t],[-t,t]];this.quad(n.map(([r,s])=>[r+e.center[0],s+e.center[1]]),n,[2,e.radius,e.radius],this.fillStyle)}}stroke(){const e=this.lineWidth*Math.hypot(this.matrix[0],this.matrix[1])/2;if(this.path.length===2){const[t,n]=this.path;if(t.type==="line"&&n.type==="line"){const r=t.b[0]-t.a[0],s=t.b[1]-t.a[1],a=n.b[0]-n.a[0],o=n.b[1]-n.a[1],c=Math.hypot(r,s);if(c>0&&Math.abs(c-Math.hypot(a,o))<1e-5&&Math.abs(r*a+s*o)<1e-5&&Math.abs(t.a[0]+t.b[0]-n.a[0]-n.b[0])<1e-5&&Math.abs(t.a[1]+t.b[1]-n.a[1]-n.b[1])<1e-5){const l=(t.a[0]+t.b[0])/2,u=(t.a[1]+t.b[1])/2,f=r/c,d=s/c,m=c/2+1,x=[[-m,-m],[m,-m],[m,m],[-m,m]];this.quad(x.map(([S,g])=>[l+f*S-d*g,u+d*S+f*g]),x,[4,c/2,e],this.strokeStyle);return}}}for(let t=0;t<this.path.length;t++){const n=this.path[t],r=this.path[t+1];if(n.type!=="line")throw new Error("Only scene line segments may be stroked");const s=n.b[0]-n.a[0],a=n.b[1]-n.a[1],o=Math.hypot(s,a);if(r?.type==="line"&&n.b[0]===r.a[0]&&n.b[1]===r.a[1]){const c=r.b[0]-r.a[0],l=r.b[1]-r.a[1],u=Math.hypot(c,l);if(o>0&&Math.abs(o-u)<1e-5&&Math.abs(s*c+a*l)<1e-5){const f=[-s/o,-a/o],d=[c/o,l/o],m=-e-1,x=o+1;let S=[[m,m],[x,m],[x,x],[m,x]];f[0]*d[1]-f[1]*d[0]<0&&(S=[S[0],S[3],S[2],S[1]]),this.quad(S.map(([g,h])=>[n.b[0]+f[0]*g+d[0]*h,n.b[1]+f[1]*g+d[1]*h]),S,[3,o,e],this.strokeStyle),t++;continue}}o>0&&this.rectAt((n.a[0]+n.b[0])/2,(n.a[1]+n.b[1])/2,s/o,a/o,o/2,e,this.strokeStyle)}}dispose(){this.disposed||(this.disposed=!0,this.geometry.dispose(),this.materials.forEach(e=>e.dispose()),this.atlasTexture.dispose(),this.images.clear(),this.renderer.dispose())}}function Pg(i,{onChange:e=()=>{},initialFocus:t}={}){const n=M=>i.querySelector(M);i.dataset.rendererBuild="c8d4575cf448b8707cdf55ae779d2743482a02f7348ae5e66f4972983bfe18cb";let r=n(".sc-sky"),s;try{s=new Cg(r),i.dataset.renderer="three-webgl2"}catch(M){const O=r.cloneNode(!0);r.replaceWith(O),r=O,s=r.getContext("2d",{alpha:!1}),i.dataset.renderer="canvas-fallback",console.warn("GPU renderer unavailable; accepted Canvas painter retained.",M)}if(!s)return;const a=matchMedia("(prefers-reduced-motion: reduce)"),o={density:.85,pixel:.5,depth:1.25,liveliness:1,playing:!a.matches},c=1250;let l=[],u=[];const f=["#89accc","#dfc28f","#a49fdb"],d=new Map,m=n(".sc-nodes");let x=7239;const S=()=>(x=x*1664525+1013904223>>>0,x/4294967296),g=(M,O,$)=>{const re=Math.max(0,Math.min(1,($-M)/(O-M)));return re*re*(3-2*re)},h=Array.from({length:2200},(M,O)=>{const $=O%25===0?2:O%5<2?1:0,re=(S()-.5)*2.75,Fe=S()<.58&&$===0?re*.34+(S()+S()+S()-1.5)*.54:(S()-.5)*2.7,Ve=[-2400,-920,220][$],Re=[1530,1150,570][$],it=S()*Math.PI*2;return{layer:$,sx:re,sy:Fe,minZ:Ve,range:Re,offset:S(),cycle:[230,135,92][$]*(.8+S()*.5),size:$===2?1.25+S()*1.5:$===1?.65+S()*.85:.38+S()*.65,phase:it,freq:.55+S()*1.5,tone:Math.floor(S()*6),alpha:[.3,.58,.36][$]*(.55+S()*.65),flare:S()>.95,drift:(S()-.5)*26,x:0,y:0}}),R=["#bdd5ed","#95bde0","#e3eaf5","#dfcfb5","#aeb8e7","#f0e3c9"],C=new Map;function y(M){if(C.has(M))return C.get(M);const O=document.createElement("canvas");O.width=O.height=128;const $=O.getContext("2d"),re=$.createRadialGradient(64,64,0,64,64,64);return re.addColorStop(0,M+"c0"),re.addColorStop(.05,M+"80"),re.addColorStop(.16,M+"30"),re.addColorStop(.45,M+"0c"),re.addColorStop(1,M+"00"),$.fillStyle=re,$.fillRect(0,0,128,128),C.set(M,O),O}const w=document.createElement("canvas");w.width=1100,w.height=780;const T=w.getContext("2d");function I(){T.fillStyle="#04070e",T.fillRect(0,0,1100,780);const M=[[210,260,390,"#111c33"],[620,410,375,"#142c31"],[930,555,330,"#231d38"],[585,405,170,"#242523"]];for(const[O,$,re,Fe]of M){const Ve=T.createRadialGradient(O,$,0,O,$,re);Ve.addColorStop(0,Fe+"b0"),Ve.addColorStop(.5,Fe+"65"),Ve.addColorStop(1,Fe+"00"),T.fillStyle=Ve,T.fillRect(0,0,1100,780)}s.invalidateImage?.(w)}function _(M,O,$){const re=(Ht,zt)=>{let It=Math.imul(Ht,374761393)+Math.imul(zt,668265263)+Math.imul($,1442695041);return It=Math.imul(It^It>>>13,1274126177),((It^It>>>16)>>>0)/4294967295},Fe=Math.floor(M),Ve=Math.floor(O),Re=M-Fe,it=O-Ve,Ye=Re*Re*(3-2*Re),Ke=it*it*(3-2*it),ft=re(Fe,Ve),at=re(Fe+1,Ve),St=re(Fe,Ve+1),Lt=re(Fe+1,Ve+1);return(ft+(at-ft)*Ye)*(1-Ke)+(St+(Lt-St)*Ye)*Ke}function A(M,O){const $=document.createElement("canvas");$.width=320,$.height=240;const re=$.getContext("2d"),Fe=re.createImageData($.width,$.height),Ve=Fe.data;for(let Re=0;Re<$.height;Re++)for(let it=0;it<$.width;it++){const Ye=it/$.width,Ke=Re/$.height,ft=_(Ye*3,Ke*3,O);let at=0,St=.55,Lt=3;for(let Et=0;Et<4;Et++)at+=_(Ye*Lt+ft*.8,Ke*Lt,O+Et)*St,St*=.5,Lt*=2;const Ht=Math.exp(-Math.pow((Ke-(.43+Math.sin(Ye*4.4+O)*.14))/.23,2)),zt=Math.sin(Math.PI*Ye)*Math.sin(Math.PI*Ke),It=Math.max(0,at-.34)*Ht*zt,ze=(Re*$.width+it)*4;Ve[ze]=M[0],Ve[ze+1]=M[1],Ve[ze+2]=M[2],Ve[ze+3]=Math.min(130,Math.round(It*220))}return re.putImageData(Fe,0,0),$}const k=A([76,102,151],17),F=A([111,91,136],53);let b=1,P=740,W=1,N=1,D=-1,G=-1,H="constellations",ne=0,ce=-.055,oe=0,_e=-.055,Qe=1,Ze=1,He={x:0,y:0},Z={x:0,y:0},de=0,le=0,Oe=0,Je=!0,Te=null,Q=0,te=0,pe=0,Se=0,Ce=0,We="about",qe=!0,ut=!1,ot=null,Le=null,et=-1/0,L=0,ht="",$e=0,E=0,p="trackpad",z=null,q=-1/0;const J=.55,me=22;let ue={x:0,y:0,manual:!1},j=[];const ee=[],fe=new Map,Pe=(M,O,$)=>Math.max(O,Math.min($,M)),he={x:0,y:0,tx:0,ty:0};let ve=1,Be=0,Ge=1,tt=0,U=1,ge=0,ie=1,xe=0;const ae=n(".sc-inspector");let se=null,De=[],Me=null,At=!1,dt=null;function gn(){return{packet:se,vertices:l.map(M=>({id:M.id,slot:M.slot,p:M.p.slice(),sourcePosition:M.sourcePosition.slice(),volumeZ:M.volumeZ,pos:M.target.slice(),target:M.target.slice()})),selectedRelation:Me}}function dn(M,O=l,{restore:$=!1}={}){const re=l[D]?.id,Fe=new Map(l.map(Re=>[Re.id,Re]));if(l=ud(M,O).map(Re=>{const it=Fe.get(Re.id);let Ye=it?.el,Ke=it?.label;if(!Ye){Ye=document.createElement("button"),Ye.type="button",Ye.className="sc-node",Ye.dataset.id=Re.id,Ke=document.createElement("span"),Ke.className="sc-node-label",Ye.append(Ke);const ft=()=>d.get(Re.id);Ye.addEventListener("click",at=>{at.detail<2&&ft()!==void 0&&v(ft())}),Ye.addEventListener("dblclick",()=>{ft()!==void 0&&(un(),Nn(ft()))}),Ye.addEventListener("pointerenter",()=>{G=ft()??-1,lt()}),Ye.addEventListener("pointerleave",()=>{G=-1,lt()}),Ye.addEventListener("focus",()=>{G=ft()??-1,lt()}),Ye.addEventListener("blur",()=>{G=-1,lt()}),Ye.addEventListener("keydown",at=>{ft()!==void 0&&Ut(at,ft())})}return Ye.dataset.main=String(Re.main),Ye.setAttribute("aria-label",Re.fullName+" — "+Re.kind),Ke.textContent=Re.name,{...Re,el:Ye,label:Ke,screen:Sr(...Re.pos)}}),d.clear(),l.forEach((Re,it)=>d.set(Re.id,it)),m.replaceChildren(...l.map(Re=>Re.el)),se=M,De=M.relations,u=De.map(Re=>[d.get(Re.from_id),d.get(Re.to_id)]),!$&&H!=="constellations"){const Re=d.get(M.focus?.node_id)??0,it=Vi(Re),Ye=l.map((Ke,ft)=>ft).filter(Ke=>Ke!==Re&&!it.includes(Ke));l.forEach((Ke,ft)=>{if(H==="plane")Ke.target=[Ke.sourcePosition[0],Ke.sourcePosition[1],0];else if(ft===Re)Ke.target=[0,0,0];else{const at=it.includes(ft)?it:Ye,St=at.indexOf(ft)/at.length*Math.PI*2-.8,Lt=at===it?175:380;Ke.target=[Math.cos(St)*Lt,Math.sin(St)*Lt*.68,Math.sin(St*2)*140*o.depth]}})}D=d.get(re)??-1,G=-1,De.some(Re=>Re.id===Me)||(Me=null),i.dataset.graphRevision=M.source_revision,i.dataset.graphFocus=M.focus?.node_id||"",i.dataset.nodeCount=String(l.length),i.dataset.relationCount=String(u.length),i.dataset.dataState="ready",Hi(),Ei(),bn(),qe=!0,Q=1,lt()}function ks(M){M?.packet&&(dt?.cancelPending(),dn(M.packet,M.vertices,{restore:!0}),Me=M.selectedRelation)}function Gs(M){if(!u.length)return!1;const O=i.getBoundingClientRect(),$=(M.clientX-O.left)*b/O.width,re=(M.clientY-O.top)*P/O.height;let Fe=-1,Ve=7;for(let Re=0;Re<u.length;Re++){const[it,Ye]=u[Re],Ke=l[it].screen,ft=l[Ye].screen;if(!Ke.s||!ft.s)continue;const at=ft.x-Ke.x,St=ft.y-Ke.y,Lt=at*at+St*St,Ht=Lt?Pe((($-Ke.x)*at+(re-Ke.y)*St)/Lt,0,1):0;if(Ht<.04||Ht>.96)continue;const zt=Math.hypot($-Ke.x-Ht*at,re-Ke.y-Ht*St);zt<Ve&&(Ve=zt,Fe=Re)}return Fe<0?!1:(Mr(De[Fe].id),!0)}function Mr(M){const O=De.find($=>$.id===M);O&&(dt?.willSelect(),(Me!==M||ae.hidden)&&un(),Y(!1,!1),V(!1,!1),Me=M,D=d.get(O.from_id)??-1,ae.hidden=!1,Wi(),Ae("about"),Ei(),bn(),nn(!0),qe=!0,lt(),In("Отношение: "+n(".sc-node-title").textContent),n("#so-about-tab").focus())}const di={get packet(){return se},get selection(){return{nodeId:l[D]?.id||null,relationId:Me}},node:M=>l.find(O=>O.id===M)?.raw,relation:M=>De.find(O=>O.id===M),neighbors:M=>De.filter(O=>O.from_id===M||O.to_id===M),setGraph(M,{selectFocus:O=!1,initial:$=!1}={}){if($||un(),dt?.cancelInspector(),dn(M),O&&M.focus){At=!0;try{v(d.get(M.focus.node_id))}finally{At=!1}}else D>=0||Me?Wi():(ae.hidden=!0,bn())},selectNode(M){const O=d.get(M);return O===void 0?!1:(v(O),!0)},selectRelation(M,{rememberView:O=!0}={}){const $=At;At=!O||$;try{Mr(M)}finally{At=$}},cardChanged(){nn(!1),qe=!0,lt()},announce:In};function Gr(){const M=b;b=i.clientWidth,P=i.clientHeight,W=Math.min(devicePixelRatio||1,1.5),r.width=Math.round(b*W),r.height=Math.round(P*W),N=b<=540?b/720:Math.min(b/1190,1.06),h.forEach(O=>{const $=O.minZ+O.range*.5,re=(c-$)/c;O.x=O.sx*b*.5/N*re,O.y=O.sy*P*.5/N*re}),Hi(),nn(M!==b),D>=0&&!ae.hidden&&Nn(D,!1),qe=!0,lt()}function Sr(M,O,$,re=!1){const Fe=re?U:ve,Ve=re?ge:Be,Re=re?ie:Ge,it=re?xe:tt,Ye=M*Fe-$*Ve,Ke=M*Ve+$*Fe,ft=O*Re-Ke*it,at=O*it+Ke*Re;if(at>c-110)return{x:-9999,y:-9999,s:0,depth:0,z:at};const St=c/(c-at),Lt=N*(re?1:Qe)*St;return{x:b*.5+Ye*Lt+(re?He.x*.18+he.x*St*23:He.x),y:P*.54+ft*Lt+(re?He.y*.18+he.y*St*17:He.y),s:Lt,depth:St,z:at}}function In(M){n(".sc-announcement").textContent=M}function an(M,O,$){M.style[O]!==String($)&&(M.style[O]=$)}function Vi(M){return u.filter(O=>O.includes(M)).map(O=>O[0]===M?O[1]:O[0])}function Hi(){l.forEach(M=>{const O=getComputedStyle(M.label);s.font=O.font;const $=parseFloat(O.letterSpacing)||0;M.labelWidth=Math.ceil(s.measureText(M.name).width+$*M.name.length)+2,M.labelHeight=Math.ceil(parseFloat(O.fontSize)*1.5)})}function bi(M){const O=M.getBoundingClientRect(),$=i.getBoundingClientRect();return{x:O.left-$.left,y:O.top-$.top,w:O.width,h:O.height}}function Un(M,O,$=0){return M.x<O.x+O.w+$&&M.x+M.w+$>O.x&&M.y<O.y+O.h+$&&M.y+M.h+$>O.y}function Vr(){j=[];for(const M of[".sc-header",".sc-context",".sc-footer",".sc-label-west",".sc-label-east",".sc-inspector",".sc-search",".sc-lenses",".sc-workspace"]){const O=n(M);if(!O.hidden){const $=bi(O);$.w&&$.h&&j.push($)}}ae.hidden||bi(ae),qe=!1}function nn(M=!1){if(!ae.hidden){if(n(".sc-window-handle").disabled=b<=540,b<=540)for(const O of["left","right","top","bottom","transform"])ae.style[O]="";else{const O=ae.offsetWidth,$=ae.offsetHeight;if(M&&!ue.manual&&D>=0){const re=l[D].screen;ue.x=re.x+O+60<b?re.x+46:re.x-O-46,ue.y=re.y-100}ue.x=Pe(ue.x,16,b-O-16),ue.y=Pe(ue.y,190,Math.max(190,P-$-98)),ae.style.left="0",ae.style.top="0",ae.style.right="auto",ae.style.bottom="auto",ae.style.transform="translate3d("+Math.round(ue.x)+"px,"+Math.round(ue.y)+"px,0)"}qe=!0,lt()}}function bn(){n(".sc-back").hidden=ee.length===0,n(".sc-context-sub").textContent=Me?"Выбрана связь":D>=0?"Фокус: "+l[D].name:se?l.length+" звёзд · "+u.length+" связей":"Загружаем область…",i.dataset.history=String(ee.length),qe=!0,e(di)}function un(){if(At)return;const M={graph:gn(),selected:D,panelOpen:!ae.hidden||ut,lens:H,targets:l.map(O=>O.target.slice()),yaw:oe,pitch:_e,zoom:Ze,pan:{...Z},windowPosition:{...ue},cardTab:We};JSON.stringify(M)!==JSON.stringify(ee.at(-1))&&(ee.push(M),ee.length>24&&ee.shift()),bn()}function Vs(){const M=ee.pop();M&&(Xe(),Y(!1,!1),V(!1,!1),ks(M.graph),D=M.selected,G=-1,H=M.lens,l.forEach((O,$)=>O.target=M.targets[$].slice()),oe=M.yaw,_e=M.pitch,Ze=M.zoom,Z={...M.pan},ue={...M.windowPosition},ye(),D>=0||Me?(Wi(),Ae(M.cardTab),ae.hidden=!M.panelOpen,nn(!1)):ae.hidden=!0,Ei(),bn(),In(D>=0?"Возврат: "+l[D].name:"Возврат к предыдущему виду"),ee.length||n(".sc-overview").focus(),Q=1,lt())}function Nn(M,O=!0,{gentle:$=!1}={}){Xe();const re=l[M].target,Fe=Math.cos(oe),Ve=Math.sin(oe),Re=Math.cos(_e),it=Math.sin(_e),Ye=re[0]*Fe-re[2]*Ve,Ke=re[0]*Ve+re[2]*Fe,ft=re[1]*Re-Ke*it,at=re[1]*it+Ke*Re;let St=b*.5,Lt=P*.5;if(!ae.hidden){const zt=bi(ae);if(b<=540){const It=bi(n(".sc-context"));Lt=(It.y+It.h+35+zt.y-38)*.5}else St=zt.x>b*.44?Math.max(b*.3,zt.x*.55):Math.min(b*.76,(zt.x+zt.w+b)*.5),Lt=Pe(zt.y+zt.h*.44,250,P-150)}Ze=$?Pe(Ze*1.08,.65,2.2):O?Math.max(b<=540?1.25:1.45,Ze):b<=540?1.12:Math.max(1.1,Ze),Ze=Math.min(2.2,Ze);const Ht=N*Ze*c/(c-at);Z.x=St-b*.5-Ye*Ht,Z.y=Lt-P*.54-ft*Ht,Q=1,lt()}function Ei(){i.dataset.selected=D>=0?l[D].id:"",l.forEach((M,O)=>M.el.setAttribute("aria-pressed",String(O===D)))}function Wi(){const M=Me?di.relation(Me):l[D]?.raw;M&&dt?.showCard(Me?"relation":"node",M)}function v(M,{fly:O=!1,keepWindow:$=!1}={}){dt?.willSelect();const re=D!==M||!!Me;(re||ae.hidden)&&un(),Me=null;const Fe=!ae.hidden;Y(!1,!1),V(!1,!1),D=M,Wi(),Ae("about"),ae.hidden=!1,Ei(),bn(),nn(!$&&(re||!Fe)),O?Nn(M,!0):re||!Fe?Nn(M,!1,{gentle:!0}):b<=540&&Nn(M,!1),In(l[M].kind+": "+l[M].name),qe=!0,lt()}function B(M=!0,O=!1){dt?.cancelInspector(),ae.hidden=!0,ut=!1,M&&D>=0&&!l[D].el.hidden&&l[D].el.focus(),O&&(Me=null,D=-1,G=-1,Ei(),bn()),qe=!0,lt()}function Y(M=!0,O=!0){dt?.cancelSearch();const $=!n(".sc-search").hidden;n(".sc-search").hidden=!0,n(".sc-search-open").setAttribute("aria-expanded","false"),$&&(O&&ut&&D>=0&&(ae.hidden=!1,nn(!1)),ut=!1),M&&n(".sc-search-open").focus(),qe=!0,lt()}function V(M=!0,O=!0){const $=!n(".sc-lenses").hidden;n(".sc-lenses").hidden=!0,n(".sc-lenses-open").setAttribute("aria-expanded","false"),$&&(O&&ut&&D>=0&&(ae.hidden=!1,nn(!1)),ut=!1),M&&n(".sc-lenses-open").focus(),qe=!0,lt()}function X(M){const O=n(".sc-"+M);if(!O.hidden){M==="search"?Y():V();return}const $=!ae.hidden||ut;Y(!1,!1),V(!1,!1),ut=$,ae.hidden=!0,O.hidden=!1,n(".sc-"+M+"-open").setAttribute("aria-expanded","true"),M==="search"?(be(),n("#sc-query").focus()):n('.sc-lens[aria-pressed="true"]').focus(),qe=!0,lt()}function be(){dt?.search(n("#sc-query").value)}function Ae(M){We=M,i.querySelectorAll(".sc-card-tab").forEach(O=>O.setAttribute("aria-selected",String(O.id==="so-"+M+"-tab"))),n("#so-about").hidden=M!=="about",n("#so-relations").hidden=M!=="relations",nn(!1),b<=540&&D>=0&&!ae.hidden&&Nn(D,!1),qe=!0,lt()}function ye(){i.dataset.lens=H,n(".sc-context h2").textContent=H==="orbits"?"Орбиты мысли":H==="plane"?"Карта связей":"Созвездия мысли",n(".sc-label-west").hidden=!0,n(".sc-label-east").hidden=!0,i.querySelectorAll(".sc-lens").forEach(M=>M.setAttribute("aria-pressed",String(M.dataset.lens===H))),qe=!0}function Ie(M){if(M===H){V();return}Xe(),un(),H=M;const O=D>=0?D:0,$=Vi(O),re=l.map((Fe,Ve)=>Ve).filter(Fe=>Fe!==O&&!$.includes(Fe));l.forEach((Fe,Ve)=>{if(M==="constellations")Fe.target=Fe.p.slice();else if(M==="plane")Fe.target=[Fe.sourcePosition[0],Fe.sourcePosition[1],0];else if(Ve===O)Fe.target=[0,0,0];else{const Re=$.includes(Ve)?$:re,it=Re.indexOf(Ve)/Re.length*Math.PI*2-.8,Ye=Re===$?175:380;Fe.target=[Math.cos(it)*Ye,Math.sin(it)*Ye*.68,Math.sin(it*2)*140*o.depth]}}),ye(),oe=0,_e=M==="plane"?0:-.055,Z={x:0,y:0},Ze=1,V(),D>=0&&!ae.hidden&&Nn(D,!1),In("Линза: "+n(".sc-context h2").textContent),Q=1,lt()}function Ne(){Xe(),un(),Y(!1,!1),V(!1,!1),oe=0,_e=H==="plane"?0:-.055,Z={x:0,y:0},Ze=1,Q=1,B(!1,!0),ue.manual=!1,In("Общий вид"),lt()}function Xe(){$e=0,et=-1/0,L=0,ht="",z=null,q=-1/0}function st(){const M=p==="trackpad",O=n(".sc-input-mode");i.dataset.inputMode=p,O.setAttribute("aria-label",M?"Управление: тачпад. Переключить на мышь":"Управление: мышь. Переключить на тачпад"),O.setAttribute("data-tooltip",M?"Тачпад · два пальца — сдвиг, щипок — полёт":"Мышь · колесо — масштаб, перетаскивание — вращение"),O.innerHTML=M?'<i data-lucide="touchpad" aria-hidden="true"></i>':'<i data-lucide="mouse" aria-hidden="true"></i>',n(".sc-gesture").textContent=M?"Два пальца — сдвиг · щипок — полёт":"Колесо — масштаб · Shift + перетаскивание — сдвиг",xs(),qe=!0}function Ue(M,O=b*.5,$=P*.54){M=Pe(M,.65,2.2);const re=M/Ze;Z.x=O-b*.5-(O-b*.5-Z.x)*re,Z.y=$-P*.54-($-P*.54-Z.y)*re,Ze=M,Q=1,lt()}function xt(){i.dataset.motion=o.playing?"running":"paused",n(".sc-motion").setAttribute("aria-pressed",String(!o.playing)),n(".sc-motion").setAttribute("aria-label",o.playing?"Приостановить движение":"Включить движение"),n(".sc-motion").innerHTML=o.playing?'<i data-lucide="pause" aria-hidden="true"></i>':'<i data-lucide="play" aria-hidden="true"></i>',xs(),lt()}function Ut(M,O){const $={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[M.key];if(!$)return;M.preventDefault();const re=l[O].screen;let Fe=-1,Ve=1/0;l.forEach((Re,it)=>{if(it===O||Re.el.hidden)return;const Ye=Re.screen.x-re.x,Ke=Re.screen.y-re.y,ft=Ye*$[0]+Ke*$[1];if(ft<=0)return;const at=Ye*Ye+Ke*Ke,St=at/Math.max(1,ft)+Math.abs(Ye*$[1]-Ke*$[0])*1.5;St<Ve&&(Ve=St,Fe=it)}),Fe>=0&&l[Fe].el.focus()}n(".sc-back").addEventListener("click",Vs),n(".sc-close").addEventListener("click",()=>B()),n(".sc-focus").addEventListener("click",()=>{D>=0&&(un(),Nn(D))}),n(".sc-search-open").addEventListener("click",()=>X("search")),n(".sc-search-close").addEventListener("click",()=>Y()),n("#sc-query").addEventListener("input",be),n(".sc-lenses-open").addEventListener("click",()=>X("lenses")),n(".sc-lenses-close").addEventListener("click",()=>V()),i.querySelectorAll(".sc-lens").forEach(M=>M.addEventListener("click",()=>Ie(M.dataset.lens))),n(".sc-overview").addEventListener("click",Ne),n(".sc-plus").addEventListener("click",()=>{Xe(),un(),Ue(Ze*1.18)}),n(".sc-minus").addEventListener("click",()=>{Xe(),un(),Ue(Ze/1.18)}),n(".sc-motion").addEventListener("click",()=>{o.playing=!o.playing,xt()}),n(".sc-input-mode").addEventListener("click",()=>{Xe(),p=p==="trackpad"?"mouse":"trackpad",st(),In(p==="trackpad"?"Тачпад: два пальца — перемещение, щипок — приближение":"Мышь: колесо — масштаб, перетаскивание — вращение"),lt()}),i.querySelectorAll(".sc-card-tab").forEach((M,O)=>{M.addEventListener("click",()=>Ae(O?"relations":"about")),M.addEventListener("keydown",$=>{if(["ArrowLeft","ArrowRight","Home","End"].includes($.key)){$.preventDefault();const re=$.key==="Home"?"about":$.key==="End"||We==="about"?"relations":"about";Ae(re),n("#so-"+re+"-tab").focus()}})}),n(".sc-search").addEventListener("keydown",M=>{const O=[...i.querySelectorAll(".sc-result")],$=O.indexOf(M.target);if(M.target===n("#sc-query")&&M.key==="Enter"&&O[0])M.preventDefault(),O[0].click();else if(M.key==="ArrowDown"||M.key==="ArrowUp"){M.preventDefault();const re=$+(M.key==="ArrowDown"?1:-1);re<0?n("#sc-query").focus():O[Math.min(re,O.length-1)]?.focus()}}),i.addEventListener("keydown",M=>{M.key==="Escape"&&(n(".sc-search").hidden?n(".sc-lenses").hidden?ae.hidden||B():V():Y(),M.stopPropagation()),M.key==="/"&&!M.target.closest("input,textarea,select,[contenteditable]")&&(M.preventDefault(),X("search"))});const vt=n(".sc-window-handle");vt.addEventListener("pointerdown",M=>{b<=540||M.button!==0||(ot={id:M.pointerId,x:M.clientX,y:M.clientY,origin:{...ue}},vt.setPointerCapture(M.pointerId),M.preventDefault())}),vt.addEventListener("pointermove",M=>{!ot||M.pointerId!==ot.id||(ue={x:ot.origin.x+M.clientX-ot.x,y:ot.origin.y+M.clientY-ot.y,manual:!0},nn(!1))});const bt=M=>{ot?.id===M.pointerId&&(ot=null,vt.hasPointerCapture(M.pointerId)&&vt.releasePointerCapture(M.pointerId))};vt.addEventListener("pointerup",bt),vt.addEventListener("pointercancel",bt),vt.addEventListener("lostpointercapture",()=>ot=null),vt.addEventListener("keydown",M=>{if(b<=540)return;const O={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]}[M.key];if(!O)return;M.preventDefault();const $=M.shiftKey?40:16;ue.x+=O[0]*$,ue.y+=O[1]*$,ue.manual=!0,nn(!1)}),r.addEventListener("pointerdown",M=>{if(M.button===0){if(fe.set(M.pointerId,{x:M.clientX,y:M.clientY}),r.setPointerCapture(M.pointerId),fe.size===1)Xe(),Te={id:M.pointerId,x:M.clientX,y:M.clientY,yaw:oe,pitch:_e,pan:{...Z},shift:M.shiftKey,moved:!1};else if(fe.size===2){const[O,$]=[...fe.values()],re=i.getBoundingClientRect();Ze=Qe,Z={...He},Le={distance:Math.hypot($.x-O.x,$.y-O.y),x:((O.x+$.x)*.5-re.left)*b/re.width,y:((O.y+$.y)*.5-re.top)*P/re.height},Te&&(Te.moved=!0)}}}),r.addEventListener("pointermove",M=>{if(fe.has(M.pointerId)){if(fe.set(M.pointerId,{x:M.clientX,y:M.clientY}),Le&&fe.size>=2){const[O,$]=[...fe.values()],re=i.getBoundingClientRect(),Fe=Math.hypot($.x-O.x,$.y-O.y),Ve=((O.x+$.x)*.5-re.left)*b/re.width,Re=((O.y+$.y)*.5-re.top)*P/re.height;Z.x+=Ve-Le.x,Z.y+=Re-Le.y,Ue(Ze*Fe/Math.max(1,Le.distance),Ve,Re),Le={distance:Fe,x:Ve,y:Re},ht="pinch",$e=performance.now()+180,$t()}else if(Te&&M.pointerId===Te.id){const O=M.clientX-Te.x,$=M.clientY-Te.y;if(Te.moved=Te.moved||Math.abs(O)+Math.abs($)>4,Te.shift){const re=i.getBoundingClientRect();Z.x=Te.pan.x+O*b/re.width*J,Z.y=Te.pan.y+$*P/re.height*J,ht="pan",$e=performance.now()+180,$t()}else oe=Te.yaw+O*.0022,_e=Pe(Te.pitch+$*.0018,-.7,.7)}Q=1,lt()}});function Bt(M){if(!fe.has(M.pointerId))return;const O=Te?.moved||Le||M.type!=="pointerup";if(fe.delete(M.pointerId),Le=null,fe.size){const[$,re]=[...fe.entries()][0];Te={id:$,x:re.x,y:re.y,yaw:oe,pitch:_e,pan:{...Z},shift:!1,moved:!0}}else Te=null,!O&&!Gs(M)&&(D>=0&&un(),Y(!1,!1),V(!1,!1),B(!1,!0));r.hasPointerCapture(M.pointerId)&&r.releasePointerCapture(M.pointerId)}r.addEventListener("pointerup",Bt),r.addEventListener("pointercancel",Bt),r.addEventListener("lostpointercapture",Bt);function we(M){const O=M.target instanceof Element?M.target:M.target.parentElement;return!!(O&&!O.closest('.sc-panel, .sc-navigation, button:not(.sc-node), input, textarea, select, a, [contenteditable="true"]'))}function Zt(M,O={x:b*.5,y:P*.54}){const $=i.getBoundingClientRect();return{x:Number.isFinite(M.clientX)?(M.clientX-$.left)*b/$.width:O.x,y:Number.isFinite(M.clientY)?(M.clientY-$.top)*P/$.height:O.y}}function pt(M,O){(O-et>220||M!==ht)&&(Ze=Qe,Z={...He}),et=O,ht=M,$e=O+180}function $t(){i.dataset.wheelKind=ht,i.dataset.zoomTarget=Ze.toFixed(6),i.dataset.panTarget=[Z.x.toFixed(3),Z.y.toFixed(3)].join(",")}function _n(M){if(!we(M))return;M.cancelable&&M.preventDefault(),M.stopPropagation(),i.dataset.wheelEvents=String(++E);const O=M.deltaMode,$=M.deltaX,re=M.deltaY,Fe=performance.now();if(!Number.isFinite($)||!Number.isFinite(re)||z||M.ctrlKey&&Fe<q||$===0&&re===0)return;const Ve=M.ctrlKey?"pinch":p==="trackpad"?"pan":"wheel",Re=O===1?32:O===2?P:1,it=Math.sign(-re);if(Ve==="wheel"&&ht===Ve&&it!==L&&(Ze=Qe,Z={...He}),pt(Ve,Fe),L=it,Ve==="pan"){const Ye=i.getBoundingClientRect();Z.x-=$*Re*b/Ye.width*J,Z.y-=re*Re*P/Ye.height*J,Q=1,lt()}else{const Ye=Ve==="pinch"?Pe(-re*Re*.006,-Math.log(4),Math.log(4)):Pe(-re*Re*.0016,-.18,.18),Ke=Zt(M);Ue(Ze*Math.exp(Ye),Ke.x,Ke.y)}$t()}i.addEventListener("wheel",_n,{passive:!1,capture:!0});function Fn(M){if(!we(M)||!Number.isFinite(M.scale)||M.scale<=0)return;M.cancelable&&M.preventDefault(),M.stopPropagation(),Ze=Qe,Z={...He};const O=Zt(M);z={scale:M.scale,...O},pt("pinch",performance.now())}function ui(M){if(!z||!Number.isFinite(M.scale)||M.scale<=0)return;M.cancelable&&M.preventDefault(),M.stopPropagation();const O=Zt(M,z);Z.x+=O.x-z.x,Z.y+=O.y-z.y,Ue(Ze*Math.pow(M.scale/z.scale,.6),O.x,O.y),z={scale:M.scale,...O},et=performance.now(),ht="pinch",$e=et+180,$t()}function wt(M){z&&(M.cancelable&&M.preventDefault(),M.stopPropagation(),z=null,q=performance.now()+80)}i.addEventListener("gesturestart",Fn,{passive:!1,capture:!0}),i.addEventListener("gesturechange",ui,{passive:!1,capture:!0}),i.addEventListener("gestureend",wt,{passive:!1,capture:!0}),window.addEventListener("blur",()=>{z=null,q=-1/0,fe.clear(),Te=null,Le=null,Xe()}),i.addEventListener("pointermove",M=>{if(M.pointerType!=="mouse"||!o.playing)return;const O=i.getBoundingClientRect();he.tx=Pe(((M.clientX-O.left)/b-.5)*2,-1,1),he.ty=Pe(((M.clientY-O.top)/P-.5)*2,-1,1),lt()}),i.addEventListener("pointerleave",()=>{he.tx=0,he.ty=0,lt()});function Dt(M,O,$){s.save(),s.globalCompositeOperation="screen",s.globalAlpha=$,s.translate(b*.5+Math.sin(de*.025+O)*b*.026+he.x*16-ne*27,P*.51+Math.cos(de*.019+O)*P*.024+he.y*12),s.rotate(Math.sin(de*.013+O)*.055-ne*.035),s.drawImage(M,-b*.72,-P*.68,b*1.44,P*1.36),s.restore()}function xn(M,O){for(let $=0;$<O;$++){const re=h[$];if(re.layer!==M)continue;const Fe=(re.offset+de/re.cycle)%1,Ve=re.minZ+Fe*re.range,Re=g(0,.065,Fe)*(1-g(.9,1,Fe));if(Re<.002)continue;const it=Math.sin(de*.035+re.phase)*re.drift,Ye=re.x+it,Ke=re.y+Math.cos(de*.029+re.phase)*re.drift*.65,ft=Ye*U-Ve*ge,at=Ye*ge+Ve*U,St=Ke*ie-at*xe,Lt=Ke*xe+at*ie;if(Lt>c-130)continue;const Ht=c/(c-Lt),zt=N*Ht,It=b*.5+ft*zt+He.x*.18+he.x*Ht*23,ze=P*.54+St*zt+He.y*.18+he.y*Ht*17;if(It<-30||ze<-30||It>b+30||ze>P+30)continue;const Et=Math.sin(de*re.freq+re.phase)*.26+Math.sin(de*re.freq*.43+re.phase*2.13)*.16,rt=re.flare?Math.pow(Math.max(0,Math.sin(de*.23+re.phase*1.7)),28):0,Ft=Math.min(.92,re.alpha*Re*(.66+Et+rt*.8))*(1-Ce*.16),kt=Math.min(M===2?3.3:2.2,Math.max(.38,re.size*Ht*(.64+N*.32))),on=R[re.tone];if(s.fillStyle=on,s.globalAlpha=Ft,M===2){const Wt=17+kt*8;s.drawImage(y(on),It-Wt/2,ze-Wt/2,Wt,Wt),s.globalAlpha=Ft*.7,s.beginPath(),s.arc(It,ze,kt*.62,0,Math.PI*2),s.fill()}else if(s.fillRect(It-kt/2,ze-kt/2,kt,kt),re.flare||kt>1.5){const Wt=12+kt*5+rt*17;s.globalAlpha=Ft*.6,s.drawImage(y(on),It-Wt/2,ze-Wt/2,Wt,Wt)}if(re.flare&&rt>.25&&M!==0){s.globalAlpha=Ft*rt*.52;const Wt=3+rt*7;s.lineWidth=.6,s.strokeStyle=on,s.beginPath(),s.moveTo(It-Wt,ze),s.lineTo(It+Wt,ze),s.moveTo(It,ze-Wt),s.lineTo(It,ze+Wt),s.stroke()}$===25&&(i.dataset.starProbe=[It.toFixed(2),ze.toFixed(2),Ft.toFixed(3)].join(","))}s.globalAlpha=1}function Rt(M){if(Oe=0,!i.isConnected||!Je||document.hidden){le=0;return}const O=performance.now();s.beginFrame?.();const $=le?(M-le)/1e3:1/60,re=Math.min($,.05);le=M;const Fe=a.matches?1:1-Math.exp(-re*14),Ve=a.matches?1:M<$e?1-Math.exp(-re*(ht==="pan"?me:30)):Fe,Re=ae.hidden?0:1;if(Ce+=(Re-Ce)*(a.matches?1:1-Math.exp(-re*3)),o.playing){de+=re*o.liveliness*(1-Ce*.65);const ze=1-Math.exp(-re*4);he.x+=(he.tx-he.x)*ze,he.y+=(he.ty-he.y)*ze}ne+=(oe-ne)*Fe,ce+=(_e-ce)*Fe,Qe+=(Ze-Qe)*Ve,He.x+=(Z.x-He.x)*Ve,He.y+=(Z.y-He.y)*Ve;let it=Math.abs(oe-ne)+Math.abs(_e-ce)+Math.abs(Ze-Qe)+Math.abs(He.x-Z.x)*.01+Math.abs(He.y-Z.y)*.01+Math.abs(Ce-Re);qe&&Vr(),ve=Math.cos(ne),Be=Math.sin(ne),Ge=Math.cos(ce),tt=Math.sin(ce);const Ye=ne+Math.sin(de*.021)*.011,Ke=ce+Math.cos(de*.017)*.008;U=Math.cos(Ye),ge=Math.sin(Ye),ie=Math.cos(Ke),xe=Math.sin(Ke),s.setTransform(W,0,0,W,0,0),s.globalAlpha=1,s.globalCompositeOperation="source-over",s.drawImage(w,0,0,b,P);const ft=Math.round(h.length*o.density);xn(0,ft),Dt(k,.2,.85),xn(1,ft),Dt(F,2.4,.46),l.forEach(ze=>{for(let Et=0;Et<3;Et++)ze.pos[Et]+=(ze.target[Et]-ze.pos[Et])*Fe,it+=Math.abs(ze.target[Et]-ze.pos[Et])*.001;ze.screen=Sr(...ze.pos)});const at=G>=0?G:D,St=new Set(at<0?[]:u.filter(ze=>ze.includes(at)).flat());s.lineWidth=.7,u.forEach(([ze,Et],rt)=>{const Ft=l[ze].screen,kt=l[Et].screen;if(!Ft.s||!kt.s)return;const on=Me?De[rt].id===Me:at>=0&&(ze===at||Et===at),Wt=l[ze].group===l[Et].group,yr=on?"#e8c88d":Wt?f[l[ze].group]:"#8ea8c3",Xi=on?.76:at>=0?.07:Wt?.39:.17,Hr=Math.max(.18,Math.min(1,Ft.depth*.75)),qi=Math.max(.18,Math.min(1,kt.depth*.75)),hi=s.createLinearGradient(Ft.x,Ft.y,kt.x,kt.y);hi.addColorStop(0,yr+Math.round(Xi*Hr*255).toString(16).padStart(2,"0")),hi.addColorStop(1,yr+Math.round(Xi*qi*255).toString(16).padStart(2,"0")),s.strokeStyle=hi,s.lineWidth=(on?1:.7)*Math.max(.6,Math.min(1.35,(Ft.depth+kt.depth)/2)),s.beginPath(),s.moveTo(Ft.x,Ft.y),s.lineTo(kt.x,kt.y),s.stroke()}),s.globalAlpha=1;const Lt=[],Ht=[];let zt=0;const It=l.map((ze,Et)=>({n:ze,i:Et})).sort((ze,Et)=>(Et.i===at?20:St.has(Et.i)?8:Et.n.main?3:0)-(ze.i===at?20:St.has(ze.i)?8:ze.n.main?3:0));for(const{n:ze,i:Et}of It){const rt=ze.screen,Ft=Et===at,kt=St.has(Et),on=ze.main,Wt=Math.max(.52,Math.min(1,rt.depth*.86)),yr=(at<0||Ft||kt?1:.36)*Wt,Xi=f[ze.group],Hr=(Ft?5:Et===0?4.7:on?3.1:1.9)*Math.max(.55,Math.min(rt.s,1.45));s.globalAlpha=yr;const qi=(Ft?115:Et===0?145:on?90:42)*Math.max(.55,rt.s);s.drawImage(y(Xi),rt.x-qi/2,rt.y-qi/2,qi,qi),s.fillStyle=Xi,s.beginPath(),s.arc(rt.x,rt.y,Hr,0,Math.PI*2),s.fill(),s.fillStyle="#fff9ea";const hi=Math.max(1,Hr*.45);if(s.fillRect(Math.round(rt.x-hi/2),Math.round(rt.y-hi/2),hi,hi),on||Ft){const Yt=(Ft?18:Et===0?16:10)*Math.max(.7,rt.s);s.globalAlpha=yr*(.45+o.pixel*.45),s.strokeStyle=Xi,s.lineWidth=.65,s.beginPath(),s.moveTo(rt.x-Yt,rt.y),s.lineTo(rt.x+Yt,rt.y),s.moveTo(rt.x,rt.y-Yt),s.lineTo(rt.x,rt.y+Yt),s.stroke()}if(Ft){s.globalAlpha=.85,s.strokeStyle="#d8bc84",s.lineWidth=1;const Yt=20,br=5;s.beginPath();for(const[Ki,Zi]of[[-1,-1],[-1,1],[1,-1],[1,1]])s.moveTo(rt.x+Ki*Yt,rt.y+Zi*(Yt-br)),s.lineTo(rt.x+Ki*Yt,rt.y+Zi*Yt),s.lineTo(rt.x+Ki*(Yt-br),rt.y+Zi*Yt);s.stroke()}s.globalAlpha=1;const Wr=b<=540?44:38,hn={x:rt.x-Wr/2,y:rt.y-Wr/2,w:Wr,h:Wr};an(ze.el,"transform","translate3d("+Math.round(hn.x)+"px,"+Math.round(hn.y)+"px,0)"),an(ze.el,"opacity",at<0||Ft||kt?"1":".46");const Nc=rt.s>0&&hn.x>10&&hn.x+hn.w<b-10&&hn.y>92&&hn.y+hn.h<P-90,Fc=j.some(Yt=>Un(hn,Yt,2)),Oc=Ht.some(Yt=>Un(hn,Yt,0));ze.el.hidden=!Nc||Fc||Oc,ze.el.hidden||Ht.push(hn);const Bc=Ft||kt||at<0&&(on||Qe>(b<=540?1.5:1.28))||on&&Qe>1.8;let Yi=null;const Ai=ze.labelWidth,wi=ze.labelHeight;if(!ze.el.hidden&&Bc){const Yt={x:rt.x-Ai/2,y:rt.y+22,w:Ai,h:wi},br={x:rt.x-Ai/2,y:rt.y-23-wi,w:Ai,h:wi},Ki={x:rt.x+28,y:rt.y-wi/2,w:Ai,h:wi},Zi={x:rt.x-28-Ai,y:rt.y-wi/2,w:Ai,h:wi};Yi=(ze.above?[br,Yt,Ki,Zi]:[Yt,br,Ki,Zi]).find(Zn=>Zn.x>12&&Zn.x+Zn.w<b-12&&Zn.y>90&&Zn.y+Zn.h<P-86&&!j.some(Ri=>Un(Zn,Ri,7))&&!Lt.some(Ri=>Un(Zn,Ri,7))&&!l.some((Ri,zc)=>zc!==Et&&Ri.screen.s>0&&Un(Zn,{x:Ri.screen.x-8,y:Ri.screen.y-8,w:16,h:16},3)))}an(ze.label,"visibility",Yi?"visible":"hidden"),Yi&&(an(ze.label,"left",Math.round(Yi.x-hn.x)+"px"),an(ze.label,"top",Math.round(Yi.y-hn.y)+"px"),an(ze.label,"transform","none"),Lt.push(Yi),zt++)}xn(2,ft),s.globalAlpha=1,s.endFrame?.(),te++,pe+=performance.now()-O,Se+=$*1e3,te%90===0&&(i.dataset.drawMs=(pe/90).toFixed(2),i.dataset.frameMs=(Se/90).toFixed(2),i.dataset.stars=String(ft),i.dataset.frames=String(te),pe=0,Se=0),i.dataset.skyClock=de.toFixed(3),i.dataset.rotation=ne.toFixed(3),i.dataset.zoom=Qe.toFixed(3),i.dataset.labels=String(zt),i.dataset.calm=Ce.toFixed(2),i.dataset.camera=[ne.toFixed(3),ce.toFixed(3),Qe.toFixed(3),He.x.toFixed(1),He.y.toFixed(1)].join(","),Q=it>(M<$e?1e-5:.005)?1:0,o.playing||Q||Te?Oe=requestAnimationFrame(Rt):le=0}r.addEventListener("sophia-renderer-restored",()=>{Q=1,lt()});function lt(){!Oe&&Je&&!document.hidden&&(Oe=requestAnimationFrame(Rt))}return document.addEventListener("visibilitychange",()=>{document.hidden?(cancelAnimationFrame(Oe),Oe=0,le=0):lt()}),new IntersectionObserver(M=>{Je=M[0].isIntersecting,Je?lt():(cancelAnimationFrame(Oe),Oe=0,le=0)}).observe(i),new ResizeObserver(Gr).observe(i),a.addEventListener("change",M=>{o.playing=!M.matches,xt()}),dt=hd(i,di,{initialFocus:t}),I(),Gr(),xt(),st(),i.dataset.lens=H,bn(),dt.start(),document.fonts?.ready.then(()=>{Hi(),lt()}),xs(),{port:di,ui:dt,openSearch:()=>X("search"),overview:Ne,closeInspector:B,invalidate:()=>{qe=!0,lt()}}}function Lg(i,e){const t={id:e.parentHypothesisId,title:e.statement.slice(0,100),body:e.statement,targetId:e.targetId,fromId:e.fromId,toId:e.toId},n=kl({persistence:!1});n.importPacket(i.exportPacket()),n.addHypothesis(t);const r={...e,createdAt:e.createdAt??new Date().toISOString(),baseWorkspaceRevision:n.summary().revision};return n.stageProposal(r),i.addHypothesis(t),i.stageProposal(r)}const ke=(i,e="",t="")=>{const n=document.createElement(i);return n.textContent=e,n.className=t,n},Mn=(i,e)=>{const t=ke("button",i);return t.type="button",t.addEventListener("click",e),t};function ms(i){if(/^https?:\/\//i.test(i))try{const e=new URL(i),t=ke("a",e.hostname+e.pathname,"sc-source-ref");return t.href=e.href,t.target="_blank",t.rel="noreferrer noopener",t}catch{}return ke("span",i,"sc-source-ref")}function Dg(i,e,{selected:t,onChange:n}){let r=!1;try{r=kc(localStorage,"tos-research-workspace-v1")}catch{}const s=kl({sessionId:"tos-local-research",persistence:r}),a=new ql,o=new Map;let c=null;const l=Gc(async(Q,te={})=>{const pe=AbortSignal.any([te.signal||new AbortController().signal,AbortSignal.timeout(6e4)]),Se=await fetch(Q,{...te,signal:pe});if(!Se.ok)throw new Error(Se.status===404?"Для этого объекта отдельное досье пока не подготовлено.":"Не удалось загрузить материал. Попробуйте ещё раз.");return Se.json()}),u=Mn("",()=>F("notes"));u.className="sc-control sc-workspace-open",u.setAttribute("aria-label","Исследование"),u.setAttribute("aria-expanded","false"),u.innerHTML='<i data-lucide="notebook-pen" aria-hidden="true"></i><span>Исследование</span>',i.querySelector(".sc-header-actions").append(u);const f=ke("section","","sc-panel sc-workspace");f.hidden=!0,f.setAttribute("aria-label","Исследовательская панель"),f.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-workspace-close" aria-label="Закрыть исследование"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Исследование</h3><div class="sc-workspace-tabs" role="tablist" aria-label="Инструменты исследования"></div><div class="sc-workspace-body" role="tabpanel" id="sc-tool-content"></div><div class="sc-tool-status" role="status"></div><div class="sc-workspace-footer"></div>',i.append(f);const d=f.querySelector(".sc-workspace-body"),m=f.querySelector(".sc-tool-status"),x=f.querySelector(".sc-workspace-tabs"),S=f.querySelector(".sc-workspace-footer");let g="notes",h=null,R=null,C="node",y=u,w="",T="note",I=null;const _={notes:"Записи",sources:"Источники",analysis:"Разбор"},A=Object.entries(_).map(([Q,te])=>{const pe=Mn(te,()=>b(Q));return pe.id="sc-tool-"+Q,pe.setAttribute("role","tab"),pe.setAttribute("aria-controls","sc-tool-content"),x.append(pe),pe});x.addEventListener("keydown",Q=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(Q.key))return;Q.preventDefault();const te=A.indexOf(Q.target),pe=Q.key==="Home"?0:Q.key==="End"?2:(te+(Q.key==="ArrowLeft"?2:1))%3;b(Object.keys(_)[pe]),A[pe].focus()});function k(){a.cancelAll(),f.hidden=!0,u.setAttribute("aria-expanded","false"),e.invalidate(),(y?.isConnected&&!y.closest("[hidden]")?y:u).focus()}f.querySelector(".sc-workspace-close").addEventListener("click",k),f.addEventListener("keydown",Q=>{Q.key==="Escape"&&(Q.preventDefault(),Q.stopPropagation(),k())});function F(Q="notes",te){y=document.activeElement instanceof HTMLElement?document.activeElement:u,h=te?{id:te.raw.id,label:Nt(te.raw.display.title||te.raw.display.label),kind:te.kind==="relation"?"edge":"node",source_refs:te.raw.source_refs}:t(),R=te?.raw||(h?.kind==="edge"?e.port.relation(h.id):e.port.node(h?.id)),C=te?.kind||(h?.kind==="edge"?"relation":"node"),e.closeInspector(!1),i.querySelector(".sc-search-close").click(),i.querySelector(".sc-lenses-close").click(),f.hidden=!1,u.setAttribute("aria-expanded","true"),b(Q),A[Object.keys(_).indexOf(Q)].focus()}function b(Q){a.cancelAll(),g=Q,f.querySelector("h3").textContent={notes:"Исследование",sources:"Источники",analysis:"Разбор текста"}[Q],m.textContent="";for(const[te,pe]of A.entries()){const Se=Object.keys(_)[te]===Q;pe.setAttribute("aria-selected",String(Se)),pe.tabIndex=Se?0:-1}d.setAttribute("aria-labelledby","sc-tool-"+Q),d.replaceChildren(),Q==="notes"?Qe():Q==="sources"?He():Z(),e.invalidate()}const P=new MutationObserver(()=>{(!i.querySelector(".sc-search").hidden||!i.querySelector(".sc-lenses").hidden)&&(f.hidden=!0,u.setAttribute("aria-expanded","false"),a.cancelAll(),e.invalidate())});for(const Q of[".sc-search",".sc-lenses"])P.observe(i.querySelector(Q),{attributes:!0,attributeFilter:["hidden"]});i.addEventListener("sophia-sources",Q=>F("sources",Q.detail));function W(Q){Q?.name!=="AbortError"&&(m.textContent=Q.message||"Не удалось выполнить действие.",e.invalidate())}function N(Q){try{return Q()}catch(te){W(te)}}function D(...Q){const te=ke("div","","sc-tool-actions");return te.append(...Q),te}function G(){d.append(ke("p",h?"Для: "+h.label:"Общие записи исследования","sc-muted"))}function H(Q){return`${Q}:${crypto.randomUUID()}`}function ne(Q,te){return s.addNote({id:H("note"),body:Q,targetId:te}),{added:!0,summary:s.summary()}}function ce(Q,te=h){return s.addHypothesis({id:H("hypothesis"),title:Q.slice(0,100),body:Q,targetId:te?.id,...te?.from_id&&te?.to_id?{fromId:te.from_id,toId:te.to_id}:{}})}function oe(Q,te=h,pe={}){if(!te?.id)throw new Error("Сначала выберите звезду или отношение.");const Se=pe.source_refs||te.source_refs||[];if(!Se.length)throw new Error("Для предложения нужен хотя бы один источник.");const Ce=pe.kind||"interpretation",We=pe.from_id||te.from_id,qe=pe.to_id||te.to_id;if(["relation","source_route"].includes(Ce)&&(!We||!qe))throw new Error("Для предложения о связи нужны оба участника.");return Lg(s,{id:H("proposal"),kind:Ce,parentHypothesisId:H("hypothesis"),targetId:te.id,...We&&qe?{fromId:We,toId:qe}:{},statement:Q,sourceRefs:Se,evidenceRefs:pe.evidence_refs?.length?pe.evidence_refs:Se,confidencePosture:{value:pe.confidence||"unknown",meaning:"maker_declared_uncertainty_not_truth_probability"},actorOrigin:pe.actor_origin==="agent"?"agent":"human",basePageRevision:pe.context_revision||0,dataFingerprint:e.port.packet?.source_revision||"unavailable"})}function _e(){const Q=URL.createObjectURL(new Blob([s.exportPacket()],{type:"application/json"})),te=ke("a");te.href=Q,te.download="sophia-research.json",te.click(),setTimeout(()=>URL.revokeObjectURL(Q),1e3)}function Qe(){const Q=w?I:h;d.append(ke("p",Q?"Для: "+Q.label:"Общие записи исследования","sc-muted")),d.append(ke("p","Записи и гипотезы сохраняются в этом браузере. Предложения остаются черновиками до рассмотрения.","sc-muted"));const te=ke("form"),pe=ke("label","Новая запись");pe.htmlFor="sc-note-text";const Se=ke("textarea");Se.id="sc-note-text",Se.maxLength=2e3,Se.placeholder="Мысль, вопрос или наблюдение…",Se.value=w,Se.addEventListener("input",()=>{w||(I=h?{...h}:null),w=Se.value});const Ce=ke("label","Тип записи");Ce.htmlFor="sc-note-kind";const We=ke("select");We.id="sc-note-kind";for(const[L,ht]of[["note","Заметка"],["hypothesis","Гипотеза"],["proposal","Предложение к рассмотрению"]]){const $e=ke("option",ht);$e.value=L,We.append($e)}We.value=T,We.addEventListener("change",()=>T=We.value);const qe=ke("button","Сохранить запись");qe.type="submit",te.append(pe,Se,Ce,We,D(qe)),te.addEventListener("submit",L=>{L.preventDefault(),N(()=>{const ht=Se.value.trim();if(!ht)throw new Error("Напишите текст записи.");const $e=w?I:h;We.value==="hypothesis"?ce(ht,$e):We.value==="proposal"?oe(ht,$e):ne(ht,$e?.id),w="",I=null,b("notes"),m.textContent="Запись сохранена."})}),d.append(te);const ut=Mn("Отменить",()=>{s.undo(),b("notes")}),ot=Mn("Повторить",()=>{s.redo(),b("notes")});ut.disabled=!s.canUndo(),ot.disabled=!s.canRedo();const Le=ke("input");Le.type="file",Le.accept=".json,application/json",Le.hidden=!0,Le.setAttribute("aria-label","Импорт исследования"),Le.addEventListener("change",async()=>{const L=Le.files?.[0];if(L){if(L.size>1e6){W(new Error("Файл превышает 1 МБ."));return}try{const ht=await L.text();s.importPacket(ht),b("notes"),m.textContent="Исследование импортировано."}catch(ht){W(ht)}}}),d.append(D(ut,ot,Mn("Экспорт",_e),Mn("Импорт",()=>Le.click())),Le);const et=s.getState();for(const[L,ht]of[["Заметка",et.notes],["Гипотеза",et.hypotheses],["Предложение · ожидает рассмотрения",et.proposals]])for(const $e of ht.slice().reverse()){const E=ke("article","","sc-entry");E.append(ke("small",L),ke("p",$e.body||$e.statement)),$e.targetId&&E.append(ke("small",$e.targetId)),L==="Заметка"&&E.append(D(Mn("Удалить",()=>{s.removeNote($e.id),b("notes")}))),d.append(E)}!et.notes.length&&!et.hypotheses.length&&!et.proposals.length&&d.append(ke("p","Здесь появятся ваши записи.","sc-muted")),s.persistenceError()&&(m.textContent="Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием.")}function Ze(Q){const te=ke("article","","sc-entry");te.append(ke("h4",Q.label||Q.preferred_label||Q.node_id));const pe=Q.properties||{};(pe.description||pe.notes)&&te.append(ke("p",pe.description||pe.notes));for(const Se of[...new Set([...Q.source_refs||[],pe.url,pe.locator,pe.source_url].filter(Ce=>typeof Ce=="string"))])te.append(ms(Se));return te}async function He(){if(G(),!R){d.append(ke("p","Выберите звезду или отношение, чтобы увидеть источники."));return}const Q=R;d.append(ke("p",Nt(C==="relation"?Q.display.explanation:Q.display.summary,"Описание пока не зафиксировано.")));const te=ke("details");te.append(ke("summary","Происхождение и статус"));for(const[Ce,We]of[["Слой",Q.epistemic?.authority_layer],["Рассмотрение",Q.epistemic?.review_posture],["Канон",Q.epistemic?.canon_status]])te.append(ke("p",Ce+": "+(We&&We!=="not-recorded"?We:"не указан"),"sc-muted"));for(const Ce of Q.source_refs)te.append(ms(Ce));if(d.append(te),C==="relation")return;const pe=Q.native_id;if(!pe)return;const Se=ke("div");d.append(Se),Se.append(ke("p","Получаю досье источников…","sc-muted"));try{const Ce=await a.run("source",ut=>l.invoke("tos.dossier.inspect",{object_id:pe,limit:40},{signal:ut}));if(!Ce.current||f.hidden||g!=="sources")return;Se.replaceChildren();const We=Ce.value;for(const[ut,ot]of[["work","Произведение"],["expression","Редакции и переводы"],["edition","Издания"],["file","Файлы"],["item","Экземпляры"],["link","Ссылки"]]){const Le=We.chain?.[ut]||[];if(!Le.length)continue;const et=ke("details");et.append(ke("summary",ot+" · "+Le.length));for(const L of Le)et.append(Ze(L));Se.append(et)}const qe=We.agent_summary;qe&&Se.append(ke("p","Доступность ссылки и право использования — отдельные сведения. Статус прав: "+(qe.rights_posture&&qe.rights_posture!=="unknown"?qe.rights_posture:"не указан")+".","sc-muted")),Se.children.length||Se.append(ke("p","Дополнительные маршруты источников пока не записаны.","sc-muted")),e.invalidate()}catch(Ce){Se.replaceChildren(ke("p",Ce.message,"sc-muted")),Se.append(D(Mn("Повторить",()=>b("sources")))),e.invalidate()}}function Z(){d.append(ke("p","Ищите недостающие источники или подготовьте разбор слова в «Заратустре».","sc-muted"));const Q=ke("label","Запрос");Q.htmlFor="sc-analysis-query";const te=ke("input");te.id="sc-analysis-query",te.placeholder="Название источника или слово…",te.maxLength=256;const pe=ke("div");d.append(Q,te,D(Mn("Пробелы в источниках",()=>de("gaps",te.value,pe).catch(()=>{})),Mn("Разобрать слово",()=>de("word",te.value,pe).catch(()=>{}))),pe)}async function de(Q,te,pe,Se,Ce={}){m.textContent="Получаю материал…",pe.replaceChildren();try{const We=Q==="gaps"?"tos.source-gaps.search":"tos.zarathustra.word-analysis.prepare",qe=Q==="gaps"?{query:te,limit:Ce.limit??12}:{query:te,language:Ce.language||"ru",rank:Ce.rank??1,include_semantic_neighbors:Ce.include_semantic_neighbors===!0},ut=await a.run("analysis",Le=>l.invoke(We,qe,{signal:Se?AbortSignal.any([Le,Se]):Le}));if(!ut.current||f.hidden||g!=="analysis")throw Se?.throwIfAborted(),new DOMException("Panel closed","AbortError");Se?.throwIfAborted();const ot=ut.value;if(m.textContent="",Q==="gaps"){o.clear();for(const Le of ot.gaps||[]){o.set(Le.edge_id,Le);const et=ke("article","","sc-entry");et.append(ke("h4",Le.to_label||Le.label),ke("p",Le.properties?.public_summary_en||Le.summary||""),ke("small","Доступ: "+Le.access_status+" · Запрос: "+Le.request_status));for(const L of Le.source_refs||[])et.append(ms(L));et.append(D(Mn("Рассмотреть",()=>Oe(Le.edge_id)))),pe.append(et)}ot.gaps?.length||pe.append(ke("p","По этому запросу пробелов не найдено."))}else if(ot.available!==!0)pe.append(ke("p","Разбор для этого запроса сейчас недоступен."),ke("p",ot.reason||"","sc-muted"));else{const Le=ot.task?.source||{};pe.append(ke("h4",Le.surface||Le.text||te),ke("p",Le.context||Le.excerpt||Le.sentence||"")),Le.source_ref&&pe.append(ms(Le.source_ref)),pe.append(ke("p","Подготовлен разбор по исходному тексту. Результат требует рассмотрения.","sc-muted")),pe.append(D(Mn("Сохранить задание",()=>{const et=URL.createObjectURL(new Blob([JSON.stringify(ot,null,2)],{type:"application/json"})),L=ke("a");L.href=et,L.download="sophia-word-analysis.json",L.click(),setTimeout(()=>URL.revokeObjectURL(et),1e3)})))}return e.invalidate(),ot}catch(We){throw W(We),We}}function le(){}function Oe(Q){const te=o.get(Q);return te?(c={id:te.edge_id,kind:"edge",semantic_kind:"source_access_gap",label:te.to_label,from_id:te.from_id,to_id:te.to_id,predicate_id:te.predicate_id,source_refs:te.source_refs,authority_posture:te.authority_posture,review_posture:te.review_posture,canon_status:te.canon_status,reroutable:!1},h=c,f.hidden=!1,u.setAttribute("aria-expanded","true"),b("notes"),n(),!0):!1}const Je={"tos.page.research-workspace":()=>({packet:JSON.parse(s.exportPacket()),summary:s.summary()}),"tos.page.add-research-note":Q=>ne(String(Q.text||""),Q.target_id?String(Q.target_id):void 0),"tos.page.add-session-hypothesis":Q=>({hypothesis:ce(String(Q.statement||""),t()),summary:s.summary()}),"tos.page.stage-proposal":Q=>({proposal:oe(String(Q.statement||""),t(),Q),summary:s.summary()}),"tos.page.workspace-undo":()=>({changed:s.undo(),summary:s.summary()}),"tos.page.workspace-redo":()=>({changed:s.redo(),summary:s.summary()}),"tos.page.workspace-export":()=>({packet:JSON.parse(s.exportPacket())}),"tos.page.workspace-import":Q=>({imported:s.importPacket(typeof Q.packet=="string"?Q.packet:JSON.stringify(Q.packet)),summary:s.summary()}),"tos.page.find-source-gaps":async(Q,{signal:te})=>{F("analysis");const pe=ke("div");d.append(pe);const Se=await de("gaps",String(Q.query||""),pe,te,Q);return{...Se,gaps:Se.gaps.map(Ce=>({...Ce,id:Ce.edge_id,label:Ce.to_label,summary:Ce.properties?.public_summary_en}))}},"tos.page.prepare-word-analysis":async(Q,{signal:te})=>{F("analysis");const pe=ke("div");return d.append(pe),de("word",String(Q.query||""),pe,te,Q)}};let Te=!1;return s.subscribe(()=>{n(),!Te&&(Te=!0,queueMicrotask(()=>{if(Te=!1,f.hidden||g!=="notes")return;const Q=document.activeElement,te=d.querySelector("textarea"),pe=Q===te,Se=te?.selectionStart,Ce=te?.selectionEnd;if(d.replaceChildren(),Qe(),pe){const We=d.querySelector("textarea");We.focus(),We.setSelectionRange(Se,Ce)}e.invalidate()}))}),xs(),window.addEventListener("pagehide",()=>a.cancelAll(),{once:!0}),{workspace:s,handlers:Je,selectionChanged:le,chooseGap:Oe,get auxiliarySelection(){return c},clearAuxiliarySelection(){c=null},agentStatus(Q){S.textContent=Q.registered?"Агент подключён к текущему пространству":Q.supported?"Подключаю агента…":"Записи хранятся локально"}}}const Uc=document.getElementById("app");Uc.innerHTML=Wc;const ki=Uc.firstElementChild;let yt,Dn,kr,Ls=!1,Bl="";const gs=new Yl;function zn(){if(Dn?.auxiliarySelection)return Dn.auxiliarySelection;const i=yt?.port.selection;if(!i)return null;const e=i.relationId?yt.port.relation(i.relationId):yt.port.node(i.nodeId);return e?{id:e.id,kind:i.relationId?"edge":"node",semantic_kind:e.kind_id||"relation",label:Nt(e.display.title||e.display.label),subtitle:Nt(e.display.statement),from_id:e.from_id,to_id:e.to_id,predicate_id:e.predicate_id,source_refs:e.source_refs,authority_posture:e.epistemic?.authority_layer,review_posture:e.epistemic?.review_posture,canon_status:e.epistemic?.canon_status,reroutable:!1}:null}function Uo(){if(!yt)return;const i=zn(),e=yt.port.packet,t=new URL(location.href);e?.focus?.node_id&&t.searchParams.set("focus",e.focus.node_id),i&&!Dn?.auxiliarySelection?t.searchParams.set("selection",i.id):t.searchParams.delete("selection"),history.replaceState(null,"",t);const n=JSON.stringify([e?.source_revision,e?.focus,i,ki.dataset.lens]);n!==Bl&&(Bl=n,Ls||kr?.notifyStateChange()),Dn?.selectionChanged()}const ti=i=>{Ls=!0;try{return i()}finally{Ls=!1,Uo()}};yt=Pg(ki,{initialFocus:new URLSearchParams(location.search).get("focus")||vo,onChange:()=>{Dn?.clearAuxiliarySelection(),Uo()}});Dn=Dg(ki,yt,{selected:zn,onChange:()=>{Ls||kr?.notifyStateChange(),Uo()}});const Ds={...Dn.handlers,"tos.page.inspect-selection":()=>zn(),"tos.page.open-view":async(i,{signal:e})=>{if(yt.ui.cancelPending(),i.mode!=="philosophy"||i.graph_mode&&i.graph_mode!=="nodes"||!["constellations","observatory"].includes(String(i.view_id)))throw new Error("Эта линза открывается в расширенном исследовательском режиме.");const t=await gs.compile(Ir(String(i.focus_id||vo)),e);return e.throwIfAborted(),ti(()=>yt.port.setGraph(t,{selectFocus:!!i.focus_id})),{view_id:"observatory"}},"tos.page.select":async(i,{signal:e})=>{yt.ui.cancelPending();const t=String(i.item_id||"");if(ti(()=>Dn.chooseGap(t)))return zn();if(yt.port.node(t))return ti(()=>yt.port.selectNode(t)),zn();if(yt.port.relation(t))return ti(()=>yt.port.selectRelation(t)),zn();const n=_s.get(t);if(!n)throw new Error("Выберите объект из текущей области или результатов поиска.");const r=await gs.compile(n.from_id?Wl(n):Ir(t),e,zl);return e.throwIfAborted(),ti(()=>{yt.port.setGraph(r,{selectFocus:!n.from_id}),n.from_id&&yt.port.selectRelation(t,{rememberView:!1})}),zn()},"tos.page.search":async(i,{signal:e})=>{yt.ui.cancelPending();const t=String(i.query||"").trim().slice(0,256),n=await gs.search(t,e);e.throwIfAborted(),yt.openSearch(),yt.ui.cancelSearch(),ki.querySelector("#sc-query").value=t;const r=ki.querySelector(".sc-search-results");r.replaceChildren(),_s.clear(),zl=n.source_revision;for(const[s,a]of[["node",n.nodes],["relation",n.relations]])for(const o of a)_s.set(o.id,o),r.append(yt.ui.searchRow(o,s,n.source_revision));return yt.invalidate(),{query:t,result_count:n.counts.matching_nodes+n.counts.matching_relations,results:[..._s.values()].map(s=>({id:s.id,label:Nt(s.display.title||s.display.label),kind:s.kind_id||"relation",summary:Nt(s.display.summary||s.display.statement)}))}},"tos.page.show-neighborhood":async(i,{signal:e})=>{yt.ui.cancelPending();const t=zn()?.id;if(!t||zn().kind!=="node")throw new Error("Сначала выберите звезду.");const n=await gs.compile(Ir(t,{depth:Math.max(1,Math.min(3,Number(i.depth)||1))}),e,yt.port.packet.source_revision);return e.throwIfAborted(),ti(()=>yt.port.setGraph(n,{selectFocus:!0})),{node:{node_id:t},neighbors:n.nodes.map(r=>({node_id:r.id,label:Nt(r.display.title)})),edges:n.relations.map(r=>({edge_id:r.id}))}},"tos.page.clear-focus":()=>ti(()=>(yt.closeInspector(!1,!0),yt.overview(),{cleared:!0}))},_s=new Map;let zl=null;for(const i of Object.keys(Dn.handlers)){const e=Ds[i];Ds[i]=(t,n)=>ti(()=>e(t,n))}kr=Hc(()=>({mode:"philosophy",view_id:"observatory",graph_mode:"nodes",selected:zn(),path_start_node_id:null,active_layers:["knowledge"],active_predicates:["overview"],deep_link:location.href,research_workspace:Dn.workspace.summary()}),Ds);const Ig=new Set(["tos.page.context","tos.page.cancel",...Object.keys(Ds)]);ki.querySelector("#sc-query").addEventListener("input",()=>kr.notifyStateChange());const zs=Vc(kr,document,Ig);zs.subscribeStatus(i=>Dn.agentStatus(i));zs.start();window.addEventListener("pagehide",()=>zs.stop());window.addEventListener("pageshow",i=>{i.persisted&&zs.start()});const Pr=new URLSearchParams(location.search).get("selection");if(Pr){const i=new MutationObserver(()=>{yt.port.packet&&(i.disconnect(),ti(()=>{yt.port.node(Pr)?yt.port.selectNode(Pr):yt.port.relation(Pr)&&yt.port.selectRelation(Pr)}))});i.observe(ki,{attributes:!0,attributeFilter:["data-graph-revision"]})}
