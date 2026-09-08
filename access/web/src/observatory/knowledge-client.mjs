import {t,uiLanguage} from './ui-i18n.mjs';
import {displayForm} from './display-language.mjs';
import {contentLanguage,validateHumanForms,claimPathFor,claimPathClosure,FormContractError} from './human-forms.mjs';
// The browser consumes the access contract; it never authors ToS relationships.
export const DEFAULT_FOCUS = 'tos.work.friedrich-nietzsche.also-sprach-zarathustra';
export const BUDGET = Object.freeze({nodes:40,relations:80});
const executedSpecs=new WeakMap();
export const specForPacket=packet=>executedSpecs.get(packet)||null;
export class ContractError extends Error {}
export class RevisionError extends Error {
  constructor(){super(t("Данные изменились. Обновите область, чтобы продолжить."));}
}
export class RequestError extends Error {
  constructor(status,message){super(message);this.status=status;}
}
// Durable reading stores selectors only. The backend must supply and validate
// the complete current path again; saved IDs never stand in for source text.
export function validateClaimReference(value,claimId){
  const id=value=>typeof value==='string'&&value.length>0&&value.length<=2048;
  const ids=(value,limit)=>Array.isArray(value)&&value.length<=limit&&value.every(id);
  if(!value||value.claimId!==claimId||!id(claimId)||!id(value.pathId)||!id(value.relationType)
    ||!ids(value.nodeIds,3)||value.nodeIds.length!==3||value.nodeIds[1]!==claimId
    ||!ids(value.relationIds,2)||value.relationIds.length!==2
    ||!ids(value.detailRelationIds,BUDGET.relations-2)
    ||!ids(value.closureNodeIds,BUDGET.nodes)||!value.closureNodeIds.length
    ||new Set(value.closureNodeIds).size!==value.closureNodeIds.length
    ||!value.nodeIds.every(nodeId=>value.closureNodeIds.includes(nodeId))
    ||new Set([...value.relationIds,...value.detailRelationIds]).size!==value.relationIds.length+value.detailRelationIds.length)
    throw new FormContractError();
  return Object.fromEntries(['claimId','pathId','relationType','nodeIds','relationIds','detailRelationIds','closureNodeIds']
    .map(key=>[key,structuredClone(value[key])]));
}
export function claimMaterialReference(packet,path){
  const closure=claimPathClosure(packet,path);
  return validateClaimReference({claimId:path.claim_node_id,pathId:path.id,relationType:path.relation_type_id,
    nodeIds:path.node_ids,relationIds:path.relation_ids,detailRelationIds:path.detail_relation_ids,closureNodeIds:closure.nodeIds},path.claim_node_id);
}
export const materialVersions=packet=>Object.fromEntries(['nodes','relations'].map(kind=>
  [kind,Object.fromEntries(packet[kind].map(item=>[item.id,item.content_revision]))]));
export function localized(value,fallback='',preferred='ru') {
  return displayForm(value,preferred)?.text||fallback;
}
export const missingReadableTitle=raw=>raw?.display?.provenance?.title==='identifier-fallback';
// Only the owner's explicit provenance marks an identifier fallback. Never
// infer a title, statement or type from an opaque ID or a human form.
export function materialDisplayForm(raw,field,preferred='ru'){
  const selection=raw?.display_selection;
  if(selection===undefined)return displayForm(raw?.display?.[field],preferred);
  if(selection?.schema_version!=='tos_display_selection_v1'||selection.content_revision!==raw?.content_revision
    ||!selection.fields||!Object.hasOwn(selection.fields,field)||!selection.fields[field]
    ||typeof selection.fields[field]!=='object'||Array.isArray(selection.fields[field]))return null;
  return displayForm(raw?.display?.[field],preferred,selection.fields[field]);
}
export function displayTitleForm(raw,preferred=uiLanguage(),material=false){
  if(missingReadableTitle(raw))return {text:[localized(raw.display.kind_label,raw.kind_id,preferred),t('Нет читаемого названия')].filter(Boolean).join(' · '),key:null,lang:null,fallback:false,unavailable:true};
  const field=raw?.display?.title?'title':'label';
  const form=material?materialDisplayForm(raw,field,preferred):displayForm(raw?.display?.[field],preferred);
  return form&&raw?.display?.provenance?.title==='navigation-template'?{...form,navigationOnly:true}:form;
}
export function displayTitle(raw,fallback='',preferred=uiLanguage()){
  return displayTitleForm(raw,preferred)?.text||fallback;
}
export function sourceOriginalTitle(raw){
  return missingReadableTitle(raw)||raw?.display?.provenance?.source_title_available===false?'':raw?.display?.title?.original||'';
}
export function nodeLabels(raw,preferred=uiLanguage()){
  const form=displayTitleForm(raw,preferred),fullName=form?.text||raw.id;
  return {name:missingReadableTitle(raw)?t('Нет читаемого названия'):fullName.length>46?fullName.slice(0,43)+'…':fullName,
    fullName,original:sourceOriginalTitle(raw),labelLanguage:form?.lang||null,
    kind:localized(raw.display.kind_label,raw.kind_id,preferred),description:localized(raw.display.summary,'',preferred)};
}
export function focusSpec(id,{depth=1}={}) {
  return {schema_version:'tos_lens_spec_v1',lens_id:'sophia-observatory-focus',language:'ru',detail:'compact',explain:true,
    seed:{focus_node_id:id},node_query:{enabled:false},
    traversal:{depth,direction:'either',profile:'overview'},limits:{...BUDGET,groups:8}};
}
export function relationSpec(relation){
  const spec=focusSpec(relation.from_id,{depth:0});
  spec.node_query={enabled:true,filters:[{field:'id',op:'in',value:[relation.from_id,relation.to_id]}]};
  // An explicitly selected relation is not subject to overview omissions.
  spec.traversal.profile='all';
  spec.relation_query={filters:[{field:'id',op:'eq',value:relation.id}]};
  spec.limits={nodes:2,relations:1,groups:2};return spec;
}
export function checkRevision(packet,expected) {
  if(!/^[a-f0-9]{64}$/.test(packet?.source_revision||''))throw new ContractError(t("Ответ не содержит версию данных."));
  if(expected&&packet.source_revision!==expected)throw new RevisionError();
  return packet;
}
function checkItems(items,kind) {
  if(!Array.isArray(items))throw new ContractError(t("Неверный список объектов."));
  const ids=new Set();
  for(const item of items) {
    if(!item||typeof item.id!=='string'||!item.id||ids.has(item.id)||!item.display
      ||!localized(kind==='node'?item.display.title:item.display.label)
      ||!/^[a-f0-9]{64}$/.test(item.content_revision||'')
      ||!Array.isArray(item.source_refs)||!item.source_refs.length
      ||item.source_refs.some(ref=>typeof ref!=='string'||!ref))throw new ContractError(t("Неполный или повторяющийся объект."));
    ids.add(item.id);
  }
  return ids;
}
function validateArea(packet,expected=null) {
  checkRevision(packet,expected);
  if(packet.authority_boundary?.is_source!==false
    ||packet.authority_boundary?.is_canon!==false
    ||packet.authority_boundary?.writes_to_tree!==false)throw new ContractError(t("Неподдерживаемый контракт области."));
  if(!Array.isArray(packet.nodes)||!Array.isArray(packet.relations)
    ||packet.nodes.length>BUDGET.nodes||packet.relations.length>BUDGET.relations)throw new ContractError(t("Область превышает бюджет отображения."));
  const ids=checkItems(packet.nodes,'node');checkItems(packet.relations,'relation');
  if(packet.relations.some(r=>!ids.has(r.from_id)||!ids.has(r.to_id)))throw new ContractError(t("Связь не содержит оба конца в области."));
  if(packet.focus&&!ids.has(packet.focus.node_id))throw new ContractError(t("Центр отсутствует в области."));
  return packet;
}
export function validateLens(packet,expected=null) {
  if(packet?.schema!=='tos_lens_result_v1')throw new ContractError(t("Неподдерживаемый контракт линзы."));
  return validateArea(packet,expected);
}
// Exploration pages remain exploration packets; they are never relabelled as a LensResult.
export function validateExploration(packet,expected=null,previous=null) {
  validateArea(packet,expected);
  const page=packet.page,ids=new Set(packet.nodes.map(n=>n.id));
  if(packet.schema!=='tos_exploration_result_v1'||packet.writes_to_tree!==false
    ||!/^[a-f0-9]{64}$/.test(packet.snapshot_revision||'')
    ||!['paused','complete','limit_reached'].includes(packet.status)
    ||!packet.focus||!Number.isInteger(page?.number)||page.number<1
    ||page.scope!=='resumable-neighborhood'||page.returned_nodes!==packet.nodes.length
    ||page.returned_relations!==packet.relations.length
    ||!Array.isArray(page.primary_node_ids)||!Array.isArray(page.context_node_ids)
    ||page.primary_node_ids.length+page.context_node_ids.length!==ids.size
    ||new Set([...page.primary_node_ids,...page.context_node_ids]).size!==ids.size
    ||[...page.primary_node_ids,...page.context_node_ids].some(id=>!ids.has(id))
    ||packet.counts?.scope!=='cumulative-discovered-not-global-total'
    ||packet.inclusion?.authority!=='query-execution-not-semantic-proof'
    ||(packet.status==='paused'?!/^[a-f0-9]{64}$/.test(page.next_cursor||''):page.next_cursor!==null))throw new ContractError(t("Неполная страница раскрытия связей."));
  if(previous&&(packet.snapshot_revision!==previous.snapshot_revision
    ||packet.focus.node_id!==previous.focus.node_id
    ||page.number!==previous.page.number+1
    ||JSON.stringify(packet.query)!==JSON.stringify(previous.query)))throw new RevisionError();
  return packet;
}
// Abort and generation checking are both needed: a completed response can race
// cancellation, and transports used in tests or future caches may ignore abort.
export class RequestSlots {
  constructor(){this.slots=new Map();}
  cancel(name){this.slots.get(name)?.abort();this.slots.delete(name);}
  cancelAll(){for(const name of this.slots.keys())this.cancel(name);}
  async run(name,work){
    this.cancel(name);const controller=new AbortController();this.slots.set(name,controller);
    try {
      const value=await work(controller.signal);
      return this.slots.get(name)===controller?{current:true,value}:{current:false};
    } catch(error) {
      if(this.slots.get(name)!==controller||controller.signal.aborted)return {current:false};
      throw error;
    } finally {if(this.slots.get(name)===controller)this.slots.delete(name);}
  }
}
export class KnowledgeClient {
  constructor({fetcher=globalThis.fetch.bind(globalThis),base='/api/knowledge',timeoutMs=60000}={}){this.fetcher=fetcher;this.base=base;this.timeoutMs=timeoutMs;}
  async request(path,{signal,body}={}) {
    const controller=new AbortController();let timedOut=false;
    const abort=()=>controller.abort(signal.reason);
    if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
    const timer=setTimeout(()=>{timedOut=true;controller.abort();},this.timeoutMs);
    try {
    const response=await this.fetcher(this.base+path,{signal:controller.signal,method:body?'POST':'GET',
      headers:body?{'Content-Type':'application/json'}:{},...(body?{body:JSON.stringify(body)}:{})});
    if(!response.ok) {
      if(response.status===409)throw new RevisionError();
      throw new RequestError(response.status,({400:t("Запрос не удалось исполнить."),403:t("Доступ к материалу ограничен."),404:t("Объект больше не доступен."),410:t("Срок сохранённого обхода истёк."),413:t("Область слишком велика. Выберите более узкий центр."),503:t("Этот способ просмотра пока не доступен.")})[response.status]||t("Не удалось получить данные. Попробуйте ещё раз."));
    }
    const packet=await response.json();
    if(!packet||typeof packet!=='object')throw new ContractError(t("Неверный ответ сервера."));
    return packet;
    } catch(error) {
      if(timedOut)throw new RequestError(504,t("Сервер отвечает дольше обычного. Попробуйте ещё раз."));
      if(!controller.signal.aborted&&(error instanceof TypeError||error?.name==='NetworkError'))throw new RequestError(0,t("Нет связи с данными. Проверьте соединение и повторите запрос."));
      if(error instanceof SyntaxError)throw new ContractError(t("Сервер вернул нечитаемый ответ. Повторите запрос."));
      throw error;
    } finally {clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  async search(query,signal,offset=0) {
    const packet=checkRevision(await this.request('/search?'+new URLSearchParams({query,limit:6,offset}),{signal}));
    if(packet.schema!=='tos_knowledge_search_v1'||packet.nodes?.length>6||packet.relations?.length>6)throw new ContractError(t("Неподдерживаемый ответ поиска."));
    checkItems(packet.nodes,'node');checkItems(packet.relations,'relation');return packet;
  }
  async compile(spec,signal,expected=null){const owned=structuredClone(spec),packet=validateLens(await this.request('/lenses/compile',{signal,body:owned}),expected);executedSpecs.set(packet,owned);return packet;}
  async explore(query,signal,expected,previous=null){
    const packet=validateExploration(await this.request('/explore',{signal,body:query}),expected,previous);
    if(!previous&&Object.entries(query).some(([key,value])=>JSON.stringify(packet.query?.[key])!==JSON.stringify(value)))throw new ContractError(t("Сервер вернул другую область раскрытия."));
    return packet;
  }
  async inspect(kind,id,signal,expected,contentRevision) {
    const packet=checkRevision(await this.request('/'+(kind==='node'?'nodes/':'relations/')+encodeURIComponent(id)+(kind==='node'?'?relation_limit=0':''),{signal}),expected);
    if(packet.schema!==(kind==='node'?'tos_knowledge_node_packet_v1':'tos_knowledge_relation_packet_v1'))throw new ContractError(t("Неверная карточка."));
    checkItems(packet.matches,kind);
    const match=packet.matches.find(item=>item.id===id);
    if(!match)throw new ContractError(t("Не найден точный идентификатор карточки."));
    if(contentRevision&&match.content_revision!==contentRevision)throw new RevisionError();
    if(kind==='relation'){const ids=checkItems(packet.endpoints,'node');if(!ids.has(match.from_id)||!ids.has(match.to_id))throw new ContractError(t("Неполные концы связи."));}
    return {packet,match};
  }
  async readMaterial(kind,id,signal,expected,contentRevision,{language='ru',relation=null}={}) {
    if(!['node','relation'].includes(kind)||typeof id!=='string'||!id||!contentLanguage(language))throw new ContractError(t('Неверный запрос материала.'));
    let spec,revision=expected;
    if(kind==='relation'){
      // Restore stores only identities. Inspect discovers the endpoints, then
      // one real full LensResult supplies every displayed field at that version.
      if(!relation||relation.id!==id||!expected){
        const identity=await this.inspect(kind,id,signal,expected,contentRevision);
        relation=identity.match;revision=identity.packet.source_revision;
      }
      spec=relationSpec(relation);
    }else{
      spec=focusSpec(id,{depth:0});spec.traversal.profile='all';spec.limits={nodes:1,relations:0,groups:1};
    }
    spec={...spec,lens_id:'sophia-observatory-material',language,detail:'full',explain:false};
    const packet=await this.compile(spec,signal,revision);
    const match=packet[kind==='node'?'nodes':'relations'].find(item=>item.id===id);
    if(!match)throw new ContractError(t('Не найден точный идентификатор карточки.'));
    if(contentRevision&&match.content_revision!==contentRevision)throw new RevisionError();
    const allowed=new Set(kind==='node'?[id]:[match.from_id,match.to_id]);
    if(packet.nodes.length!==allowed.size||packet.nodes.some(node=>!allowed.has(node.id))
      ||packet.relations.length!==(kind==='node'?0:1))throw new ContractError(t('Ответ вышел за границы выбранного материала.'));
    for(const item of [...packet.nodes,...packet.relations])validateHumanForms(item,language);
    // This UI envelope is not an invented inspect packet. Keep the original
    // LensResult and its schema intact for validation, revision and provenance.
    return {packet,match,endpoints:kind==='relation'?packet.nodes:[]};
  }
  async readClaimMaterial(scene,path,signal,{language='ru'}={}){
    validateArea(scene);
    return this.readClaimReference(claimMaterialReference(scene,path),signal,
      {language,expected:scene.source_revision,versions:materialVersions(scene)});
  }
  async readClaimReference(reference,signal,{language='ru',expected=null,versions=null}={}){
    if(!contentLanguage(language))throw new ContractError(t('Неверный запрос материала.'));
    const ref=validateClaimReference(reference,reference?.claimId);
    const closure={nodeIds:ref.closureNodeIds,relationIds:[...ref.relationIds,...ref.detailRelationIds]};
    // Exact selectors supply the closure. A focus can suppress a valid compact
    // path when that node is the Claim or is also referenced as its grounds.
    const spec={...focusSpec(ref.nodeIds[0],{depth:0}),seed:{},lens_id:'sophia-observatory-claim-material',language,detail:'full',explain:false,
      node_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.nodeIds}]},
      relation_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.relationIds}]},
      traversal:{depth:0,direction:'either',profile:'all'},
      limits:{nodes:closure.nodeIds.length,relations:closure.relationIds.length,groups:closure.nodeIds.length}};
    const packet=await this.compile(spec,signal,expected);
    const exact=(items,ids)=>items.length===ids.length&&items.every(item=>ids.includes(item.id));
    if(!exact(packet.nodes,closure.nodeIds)||!exact(packet.relations,closure.relationIds))
      throw new ContractError(t('Ответ вышел за границы выбранного материала.'));
    for(const kind of ['nodes','relations'])for(const item of packet[kind]){
      if(versions&&item.content_revision!==versions[kind]?.[item.id])throw new RevisionError();
      validateHumanForms(item,language);
    }
    const selected=claimPathFor(packet,ref.claimId);if(!selected)throw new FormContractError();
    const returned=claimPathClosure(packet,selected);
    if(selected.id!==ref.pathId||selected.relation_type_id!==ref.relationType
      ||JSON.stringify(selected.node_ids)!==JSON.stringify(ref.nodeIds)
      ||JSON.stringify(selected.relation_ids)!==JSON.stringify(ref.relationIds)
      ||!exact(returned.nodeIds.map(id=>({id})),closure.nodeIds)
      ||!exact(returned.relationIds.map(id=>({id})),closure.relationIds))throw new FormContractError();
    return {packet,match:returned.node,endpoints:[],path:selected};
  }
  capabilities(signal){return this.request('/explore/capabilities',{signal});}
}

// Pure presentation mapping. Opaque IDs never merge through entity_id or title.
// Existing positions survive replacement; they express UI layout, not meaning.
const slots=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];
function hash(id){let value=2166136261;for(const c of id)value=Math.imul(value^c.codePointAt(0),16777619);return value>>>0;}
export function projectLens(packet,previous=[]) {
  if(packet.schema==='tos_exploration_result_v1')validateExploration(packet);else validateLens(packet);
  const existing=new Map(previous.map(n=>[n.id,n])),degree=new Map();
  for(const r of packet.relations){degree.set(r.from_id,(degree.get(r.from_id)||0)+1);degree.set(r.to_id,(degree.get(r.to_id)||0)+1);}
  const focus=packet.focus?.node_id;
  const ordered=packet.nodes.slice().sort((a,b)=>(b.id===focus)-(a.id===focus)
    ||(degree.get(b.id)||0)-(degree.get(a.id)||0)||a.id.localeCompare(b.id,'en'));
  const currentIds=new Set(packet.nodes.map(n=>n.id));
  const occupied=new Set(previous.filter(n=>currentIds.has(n.id)).map(n=>n.slot));
  let nextSlot=0;
  return ordered.map((raw,index)=>{
    const old=existing.get(raw.id);while(occupied.has(nextSlot))nextSlot++;
    const slot=old?.slot??nextSlot++;occupied.add(slot);
    const h=hash(raw.id),angle=slot*2.399963229728653;
    const p=slots[slot]?.slice()||[Math.cos(angle)*(260+(slot%4)*58),Math.sin(angle)*(160+(slot%3)*47),-480+(h%740)];
    return {id:raw.id,raw,...nodeLabels(raw),
      main:index<8,above:index%3===1,group:raw.id===focus?1:raw.kind_id==='agent'?0:raw.kind_id==='expression'?2:1,
      slot,p:old?.p?.slice()||p,sourcePosition:old?.sourcePosition?.slice()||p.slice(),volumeZ:old?.volumeZ??p[2],
      pos:old?.pos?.slice()||p.slice(),target:old?.target?.slice()||p.slice()};
  });
}
