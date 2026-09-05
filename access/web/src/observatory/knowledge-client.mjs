// The browser consumes the access contract; it never authors ToS relationships.
export const DEFAULT_FOCUS = 'tos.work.friedrich-nietzsche.also-sprach-zarathustra';
export const BUDGET = Object.freeze({nodes:40,relations:80});
export class ContractError extends Error {}
export class RevisionError extends Error {
  constructor(){super('Данные изменились. Обновите область, чтобы продолжить.');}
}
export class RequestError extends Error {
  constructor(status,message){super(message);this.status=status;}
}
export function localized(value,fallback='') {
  return [value?.ru,value?.default,value?.original,value?.en].find(v=>typeof v==='string'&&v.trim())||fallback;
}
export function focusSpec(id,{depth=1}={}) {
  return {schema_version:'tos_lens_spec_v1',lens_id:'sophia-observatory-focus',language:'ru',detail:'compact',
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
  if(!/^[a-f0-9]{64}$/.test(packet?.source_revision||''))throw new ContractError('Ответ не содержит версию данных.');
  if(expected&&packet.source_revision!==expected)throw new RevisionError();
  return packet;
}
function checkItems(items,kind) {
  if(!Array.isArray(items))throw new ContractError('Неверный список объектов.');
  const ids=new Set();
  for(const item of items) {
    if(!item||typeof item.id!=='string'||!item.id||ids.has(item.id)||!item.display
      ||!localized(kind==='node'?item.display.title:item.display.label)
      ||!/^[a-f0-9]{64}$/.test(item.content_revision||'')
      ||!Array.isArray(item.source_refs)||!item.source_refs.length
      ||item.source_refs.some(ref=>typeof ref!=='string'||!ref))throw new ContractError('Неполный или повторяющийся объект.');
    ids.add(item.id);
  }
  return ids;
}
export function validateLens(packet,expected=null) {
  checkRevision(packet,expected);
  if(packet.schema!=='tos_lens_result_v1'||packet.authority_boundary?.is_source!==false
    ||packet.authority_boundary?.is_canon!==false
    ||packet.authority_boundary?.writes_to_tree!==false)throw new ContractError('Неподдерживаемый контракт области.');
  if(!Array.isArray(packet.nodes)||!Array.isArray(packet.relations)
    ||packet.nodes.length>BUDGET.nodes||packet.relations.length>BUDGET.relations)throw new ContractError('Область превышает бюджет отображения.');
  const ids=checkItems(packet.nodes,'node');checkItems(packet.relations,'relation');
  if(packet.relations.some(r=>!ids.has(r.from_id)||!ids.has(r.to_id)))throw new ContractError('Связь не содержит оба конца в области.');
  if(packet.focus&&!ids.has(packet.focus.node_id))throw new ContractError('Центр отсутствует в области.');
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
      throw new RequestError(response.status,({400:'Запрос не удалось исполнить.',404:'Объект больше не доступен.',410:'Срок сохранённого обхода истёк.',413:'Область слишком велика. Выберите более узкий центр.',503:'Этот способ просмотра пока не доступен.'})[response.status]||'Не удалось получить данные. Попробуйте ещё раз.');
    }
    const packet=await response.json();
    if(!packet||typeof packet!=='object')throw new ContractError('Неверный ответ сервера.');
    return packet;
    } catch(error) {
      if(timedOut)throw new RequestError(504,'Сервер отвечает дольше обычного. Попробуйте ещё раз.');
      if(!controller.signal.aborted&&(error instanceof TypeError||error?.name==='NetworkError'))throw new RequestError(0,'Нет связи с данными. Проверьте соединение и повторите запрос.');
      if(error instanceof SyntaxError)throw new ContractError('Сервер вернул нечитаемый ответ. Повторите запрос.');
      throw error;
    } finally {clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  async search(query,signal,offset=0) {
    const packet=checkRevision(await this.request('/search?'+new URLSearchParams({query,limit:6,offset}),{signal}));
    if(packet.schema!=='tos_knowledge_search_v1'||packet.nodes?.length>6||packet.relations?.length>6)throw new ContractError('Неподдерживаемый ответ поиска.');
    checkItems(packet.nodes,'node');checkItems(packet.relations,'relation');return packet;
  }
  async compile(spec,signal,expected=null){return validateLens(await this.request('/lenses/compile',{signal,body:spec}),expected);}
  async inspect(kind,id,signal,expected) {
    const packet=checkRevision(await this.request('/'+(kind==='node'?'nodes/':'relations/')+encodeURIComponent(id)+(kind==='node'?'?relation_limit=0':''),{signal}),expected);
    if(packet.schema!==(kind==='node'?'tos_knowledge_node_packet_v1':'tos_knowledge_relation_packet_v1'))throw new ContractError('Неверная карточка.');
    checkItems(packet.matches,kind);
    const match=packet.matches.find(item=>item.id===id);
    if(!match)throw new ContractError('Не найден точный идентификатор карточки.');
    if(kind==='relation'){const ids=checkItems(packet.endpoints,'node');if(!ids.has(match.from_id)||!ids.has(match.to_id))throw new ContractError('Неполные концы связи.');}
    return {packet,match};
  }
  capabilities(signal){return this.request('/explore/capabilities',{signal});}
}

// Pure presentation mapping. Opaque IDs never merge through entity_id or title.
// Existing positions survive replacement; they express UI layout, not meaning.
const slots=[[0,5,70],[-124,-134,-190],[143,-97,210],[151,91,-45],[-106,135,185],[-230,-47,95],[-54,-70,300],[63,157,245],[-29,74,-225],[-403,100,-460],[-474,-2,-335],[-309,184,-350],[-506,141,-560],[-365,-18,-420],[346,17,-140],[412,-98,-315],[490,65,-275],[344,157,120],[470,202,-410]];
function hash(id){let value=2166136261;for(const c of id)value=Math.imul(value^c.codePointAt(0),16777619);return value>>>0;}
export function projectLens(packet,previous=[]) {
  validateLens(packet);const existing=new Map(previous.map(n=>[n.id,n])),degree=new Map();
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
    const fullName=localized(raw.display.title,raw.id),name=fullName.length>46?fullName.slice(0,43)+'…':fullName;
    const h=hash(raw.id),angle=slot*2.399963229728653;
    const p=slots[slot]?.slice()||[Math.cos(angle)*(260+(slot%4)*58),Math.sin(angle)*(160+(slot%3)*47),-480+(h%740)];
    return {id:raw.id,raw,name,fullName,original:raw.display.title.original||raw.display.title.en||'',
      kind:localized(raw.display.kind_label,raw.kind_id),description:localized(raw.display.summary),
      main:index<8,above:index%3===1,group:raw.id===focus?1:raw.kind_id==='agent'?0:raw.kind_id==='expression'?2:1,
      slot,p:old?.p?.slice()||p,sourcePosition:old?.sourcePosition?.slice()||p.slice(),volumeZ:old?.volumeZ??p[2],
      pos:old?.pos?.slice()||p.slice(),target:old?.target?.slice()||p.slice()};
  });
}
