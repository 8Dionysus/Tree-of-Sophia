import {lensPosition} from './lens-projection-rules.mjs';
import {browserFocusSpec,browserRelationSpec} from './lens-spec-rules.mjs';
import {checkClientPacket,checkClientSearchRequest,checkClientExplorationRequest,checkClientMaterialRequest,checkClientMaterialPacket,checkClientClaimScope,checkClientClaimPath,packetSetKey} from './client-packet-rules.mjs';
import {sourceDossierReference,sourceDossierRequest,sourceDossierCheck} from './source-dossier-rules.mjs';
import {checkClientInspection} from './client-inspection-rules.mjs';
import {t,uiLanguage} from './ui-i18n.mjs';
import {relationLabel,fileLabel,sourceLinkLabel,languageName,readableTitleForm,isReadablePresentationTitle} from './human-presentation.mjs';
import {chooseKnowledgeSearchMode} from '../knowledge-search.ts';
import {displayForm} from './display-language.mjs';
import {contentLanguage,validateHumanForms,claimPathFor,claimPathClosure,FormContractError} from './human-forms.mjs';
import {normalizeClaimReference} from './claim-reference-rust.mjs';
import {verifyReadableContext} from './readable-context.mjs';
import {knowledgeScene} from '../../../shared/knowledge-scene.ts';
import {DEFAULT_RESPONSE_BYTES,validateResponseLimit,ResponseLimitError,withAbort,cancelResponseBody,readBoundedJSON} from './bounded-response.mjs';
// The browser consumes the access contract; it never authors ToS relationships.
export const DEFAULT_FOCUS = 'tos.work.friedrich-nietzsche.also-sprach-zarathustra';
export const BUDGET = Object.freeze({nodes:40,relations:80});
// Source dossiers are a metadata-only bridge from a delivered carrier to its
// owner-provided bibliographic route.  Keep the browser window narrower than
// the backend contract; truncation remains an honest dossier field.
export const SOURCE_DOSSIER_LIMIT = 64;
export const isSourceDossierRef=sourceDossierReference;
const executedSpecs=new WeakMap();
export const specForPacket=packet=>executedSpecs.get(packet)||null;
export class ContractError extends Error {}
export class RevisionError extends Error {
  constructor(){super(t("Данные изменились. Обновите область, чтобы продолжить."));}
}
export class RequestError extends Error {
  constructor(status,message){super(message);this.status=status;}
}
// JSON objects have no meaningful property order; array order remains exact.
export function sameJson(left,right){
  if(left===right)return true;
  if(left===null||right===null||typeof left!=='object'||typeof right!=='object')return false;
  if(Array.isArray(left)||Array.isArray(right)){
    return Array.isArray(left)&&Array.isArray(right)&&left.length===right.length
      &&left.every((value,index)=>sameJson(value,right[index]));
  }
  const keys=Object.keys(left);
  return keys.length===Object.keys(right).length
    &&keys.every(key=>Object.hasOwn(right,key)&&sameJson(left[key],right[key]));
}
// The original request remains on the wire; only comparison treats the two
// owner-normalized selector fields as sets.
export function explorationRequestMatches(normalized,requested){
  if(!normalized)return false;
  return Object.entries(requested).every(([key,value])=>{
    if(!packetSetKey(key))return sameJson(normalized[key],value);
    const actual=normalized[key];
    if(!Array.isArray(value)||!Array.isArray(actual))return false;
    const strings=value.every(item=>typeof item==='string');
    if(!strings)return strings;
    return new Set(actual).size===actual.length&&new Set(value).size===actual.length
      &&value.every(item=>actual.includes(item));
  });
}
// Durable reading stores selectors only. The backend must supply and validate
// the complete current path again; saved IDs never stand in for source text.
export function validateClaimReference(value,claimId){
  try{return normalizeClaimReference(value,claimId);}
  catch{throw new FormContractError();}
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
  if(raw?.predicate_id)return {text:relationLabel(raw,preferred),key:preferred,lang:preferred,fallback:false};
  const provenance=raw?.display?.provenance||{};
  // Text-layer navigation currently supplies a generated packet filename,
  // not the title of a passage. Use only declared type/language here.
  if(raw?.kind_id==='text-layer'&&provenance.title==='projected-label'&&!raw.display.title?.[preferred]){
    const name={ru:'Текстовый слой',en:'Text layer',es:'Capa de texto'}[preferred]??'Text layer';
    const language=raw.attributes?.language;
    return {text:name+(language?' · '+languageName(language,preferred):''),key:preferred,lang:preferred,fallback:false,navigationOnly:true};
  }

  const field=raw?.display?.title?'title':'label';
  const administrative=['item','file','link','anchor','text-unit'].includes(raw?.kind_id)
    ||provenance.title==='projected-path';
  const selected=material?materialDisplayForm(raw,field,preferred):null;
  // A full material may carry an owner-selected form. Keep that selection and
  // preserve its exact source wording. Compact administrative navigation can
  // still choose the first readable explicit source form.
  const form=material&&raw?.display_selection!==undefined
    ? selected&&(!administrative||isReadablePresentationTitle(selected.text))?selected:null
    : readableTitleForm(raw?.display?.[field],preferred,{guard:administrative});
  // A generated claim descriptor combines transport posture, endpoints and
  // status prose. It is a navigation carrier, not an authored material title.
  if(raw?.kind_id==='claim'&&(missingReadableTitle(raw)
    ||provenance.title==='navigation-template'&&provenance.source_title_available!==true)){
    const predicate=raw.semantics?.claim?.source_predicate_id;
    const claimLabel={ru:'Утверждение',en:'Claim',es:'Afirmación'}[preferred]??'Claim';
    const predicateLabel=relationLabel({predicate_id:predicate},preferred);
    const knownPredicate=predicateLabel!==relationLabel({},preferred);
    const text=predicate==='provision_activity'?predicateLabel:knownPredicate?`${claimLabel} · ${predicateLabel}`:claimLabel;
    return {text,key:preferred,lang:preferred,fallback:false,navigationOnly:true};
  }
  if(missingReadableTitle(raw))return {text:localized(raw.display.kind_label,t('Материал'),preferred),key:null,lang:null,fallback:false,unavailable:true};
  if(form)return provenance.title==='navigation-template'?{...form,navigationOnly:true}:form;
  // Administrative carrier descriptions are source text, not interface titles.
  // A localized kind is a navigation label; the original name stays in Sources.
  if(administrative){
    const names={
      'text-unit':{ru:'Фрагмент',en:'Passage',es:'Pasaje'},anchor:{ru:'Место в тексте',en:'Text location',es:'Lugar del texto'},
      'repository-branch_manifest':{ru:'Описание раздела',en:'Section description',es:'Descripción de la sección'},
      'repository-manifest':{ru:'Описание раздела',en:'Section description',es:'Descripción de la sección'},
      'repository-research_packet':{ru:'Исследование',en:'Research',es:'Investigación'},
      'repository-source_witness':{ru:'Сведения об источнике',en:'Source information',es:'Información de la fuente'}};
    let text=names[raw.kind_id]?.[preferred]??localized(raw.display.kind_label,t('Материал'),preferred);
    if(raw.kind_id==='file')text=fileLabel(raw.attributes,preferred,raw.display.title?.default);
    if(raw.kind_id==='item'||raw.kind_id==='link'){
      const refs=[...(raw.attributes?.source_refs??[]),...(raw.source_refs??[])];
      const url=typeof raw.attributes?.uri==='string'?raw.attributes.uri:refs.find(ref=>/^https?:\/\//i.test(ref));
      if(url)text+=' · '+sourceLinkLabel(url,preferred);
    }
    return {text,key:preferred,lang:preferred,fallback:false,navigationOnly:true};
  }
  return null;
}
export function displayTitle(raw,fallback='',preferred=uiLanguage()){
  return displayTitleForm(raw,preferred)?.text||fallback;
}
export function sourceOriginalTitle(raw){
  return missingReadableTitle(raw)||raw?.display?.provenance?.source_title_available===false?'':raw?.display?.title?.original||'';
}
export function nodeLabels(raw,preferred=uiLanguage()){
  const form=displayTitleForm(raw,preferred),fullName=form?.text||raw.id;
  return {name:fullName.length>46?fullName.slice(0,43)+'…':fullName,
    fullName,original:sourceOriginalTitle(raw),labelLanguage:form?.lang||null,
    kind:localized(raw.display.kind_label,raw.kind_id,preferred),description:localized(raw.display.summary,'',preferred)};
}
export function focusSpec(id,{depth=1}={}){return browserFocusSpec(id,depth);}
export function relationSpec(relation){return browserRelationSpec(relation);}

// A public route may name either a node or a relation.  Relation identities
// are intentionally opaque, so the route resolver asks the owner relation
// endpoint first and falls back to the node lens only for an actual 404.  A
// network, permission, revision, or contract failure must remain visible and
// must never be mistaken for a node route.
export async function compileRouteCenter(client,id,signal,expected=null,{depth=1}={}) {
  if(typeof id!=='string'||!id.trim())throw new ContractError(t("Не указан центр области."));
  let relationError;
  try{
    const identity=await client.inspect('relation',id,signal,expected);
    const revision=identity.packet.source_revision;
    const packet=await client.compile(relationSpec(identity.match),signal,revision);
    if(!packet.relations.some(item=>item.id===id))throw new ContractError(t("Выбранное отношение отсутствует в области."));
    return {packet,kind:'relation',relation:identity.match};
  }catch(error){
    if(!(error instanceof RequestError)||error.status!==404)throw error;
    relationError=error;
  }
  try{
    const packet=await client.compile(focusSpec(id,{depth}),signal,expected);
    return {packet,kind:'node'};
  }catch(error){
    if(error instanceof RequestError&&error.status===404)throw relationError;
    throw error;
  }
}

export function checkRevision(packet,expected){return validateClientPacket(packet,0,expected);}
function validateDossierRequest(objectId,limit){
  if(!sourceDossierRequest(objectId,limit))throw new ContractError(t('Неверная ссылка на досье источника.'));
}
export function validateSourceDossier(packet,expected){
  const code=sourceDossierCheck(packet,expected);
  if(code)throw new ContractError(t(code==='chain'?'Неполная цепочка источника.':'Неподдерживаемое досье источника.'));
  return packet;
}
function packetRefusal(code){
  if(code==='revision')throw new RevisionError();
  if(code==='items'||code==='item')inspectionRefusal(code);
  const messages={missing_revision:'Ответ не содержит версию данных.',lens:'Неподдерживаемый контракт линзы.',
    area:'Неподдерживаемый контракт области.',budget:'Область превышает бюджет отображения.',
    endpoints:'Связь не содержит оба конца в области.',focus:'Центр отсутствует в области.',
    exploration:'Неполная страница раскрытия связей.',request_area:'Сервер вернул другую область раскрытия.',search:'Неподдерживаемый ответ поиска.'};
  throw new ContractError(t(messages[code]));
}
function inspectionRefusal(code){
  if(code==='revision')throw new RevisionError();
  const messages={missing_revision:'Ответ не содержит версию данных.',schema:'Неверная карточка.',
    items:'Неверный список объектов.',item:'Неполный или повторяющийся объект.',
    match:'Не найден точный идентификатор карточки.',endpoints:'Неполные концы связи.'};
  throw new ContractError(t(messages[code]));
}
const packetHost={localized,sameJson,scene:knowledgeScene,add:(left,right)=>left+right,requestMatches:explorationRequestMatches,claimPathFor,claimPathClosure};
function validateClientPacket(packet,mode,expected=null,previous=null,options={}){
  const code=checkClientPacket(packet,mode,expected,previous,options,packetHost);
  if(code)packetRefusal(code);
  return packet;
}
function validateArea(packet,expected=null){return validateClientPacket(packet,1,expected);}
export function validateLens(packet,expected=null){return validateClientPacket(packet,2,expected);}
export function validateExploration(packet,expected=null,previous=null){return validateClientPacket(packet,3,expected,previous);}
function validateExplorationRequestPacket(packet,query,previous){
  const code=checkClientExplorationRequest(packet,query,previous,packetHost);
  if(code)packetRefusal(code);
  return packet;
}
function materialResult(result){
  if(typeof result!=='string')return result;
  if(result==='revision')throw new RevisionError();
  if(result==='form')throw new FormContractError();
  const messages={material_request:'Неверный запрос материала.',match:'Не найден точный идентификатор карточки.',material_scope:'Ответ вышел за границы выбранного материала.'};
  throw new ContractError(t(messages[result]));
}
function validateSearchRequest(cursor,search_mode,limit){
  const code=checkClientSearchRequest(cursor,search_mode,limit);
  if(code)packetRefusal(code);
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
function validateClientInspection(packet,kind,id,expected,contentRevision){
  const result=checkClientInspection(packet,kind,id,expected,contentRevision,localized);
  if(typeof result==='string')inspectionRefusal(result);
  return result.match;
}
export class KnowledgeClient {
  constructor({fetcher=globalThis.fetch.bind(globalThis),base='/api/knowledge',timeoutMs=60000,maxResponseBytes=DEFAULT_RESPONSE_BYTES}={}){
    validateResponseLimit(maxResponseBytes);
    this.fetcher=fetcher;this.base=base;this.timeoutMs=timeoutMs;this.maxResponseBytes=maxResponseBytes;
  }
  async request(path,{signal,body,maxResponseBytes=this.maxResponseBytes}={}) {
    validateResponseLimit(maxResponseBytes);
    if(maxResponseBytes>this.maxResponseBytes)throw new RangeError('A request cannot widen the client response budget.');
    const limitMessage=()=>['/catalog','/contracts'].includes(path.split('?')[0])
      ?t("Словарь данных слишком велик для загрузки. Обратитесь к оператору сервиса.")
      :t("Область слишком велика. Выберите более узкий центр.");
    const controller=new AbortController();let timedOut=false;
    const abort=()=>controller.abort(signal.reason);
    if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
    const timer=setTimeout(()=>{timedOut=true;controller.abort();},this.timeoutMs);
    try {
    controller.signal.throwIfAborted();
    // A source route is an explicit same-origin API path, never a caller-
    // supplied host or filesystem base.  Knowledge routes remain relative to
    // the configured knowledge prefix.
    const endpoint=path.startsWith('/api/source/')?path:this.base+path;
    const response=await withAbort(Promise.resolve(this.fetcher(endpoint,{signal:controller.signal,method:body?'POST':'GET',
      headers:body?{'Content-Type':'application/json'}:{},...(body?{body:JSON.stringify(body)}:{})})).then(response=>{
        if(controller.signal.aborted){cancelResponseBody(response,controller.signal.reason);controller.signal.throwIfAborted();}
        return response;
      }),controller.signal);
    if(!response.ok) {
      cancelResponseBody(response);
      if(response.status===409)throw new RevisionError();
      throw new RequestError(response.status,({400:t("Запрос не удалось исполнить."),403:t("Доступ к материалу ограничен."),404:t("Объект больше не доступен."),410:t("Срок сохранённого обхода истёк."),413:limitMessage(),503:t("Этот способ просмотра пока не доступен.")})[response.status]||t("Не удалось получить данные. Попробуйте ещё раз."));
    }
    const packet=await readBoundedJSON(response,maxResponseBytes,controller.signal);
    if(!packet||typeof packet!=='object'||Array.isArray(packet))throw new ContractError(t("Неверный ответ сервера."));
    return packet;
    } catch(error) {
      if(timedOut)throw new RequestError(504,t("Сервер отвечает дольше обычного. Попробуйте ещё раз."));
      if(controller.signal.aborted)throw error;
      if(error instanceof ResponseLimitError)throw new RequestError(413,limitMessage());
      if(!controller.signal.aborted&&(error instanceof TypeError||error?.name==='NetworkError'))throw new RequestError(0,t("Нет связи с данными. Проверьте соединение и повторите запрос."));
      if(error instanceof SyntaxError)throw new ContractError(t("Сервер вернул нечитаемый ответ. Повторите запрос."));
      throw error;
    } finally {clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
  async search(query,signal,{cursor=null,search_mode,source_revision,limit=6}={}) {
    validateSearchRequest(cursor,search_mode,limit);
    const capabilities=await this.request('/search/capabilities',{signal});
    const mode=chooseKnowledgeSearchMode(capabilities,search_mode,query);
    const packet=validateClientPacket(await this.request('/search?'+new URLSearchParams({query,limit,mode,...(cursor!==null?{cursor}:{})}),{signal}),5,source_revision,null,{mode,cursor,limit});
    return {...packet,search_mode:mode};
  }
  async compile(spec,signal,expected=null){const owned=structuredClone(spec),packet=validateLens(await this.request('/lenses/compile',{signal,body:owned}),expected);executedSpecs.set(packet,owned);return packet;}
  async explore(query,signal,expected,previous=null){
    const packet=validateExploration(await this.request('/explore',{signal,body:query}),expected,previous);
    validateExplorationRequestPacket(packet,query,previous);
    return packet;
  }
  async inspect(kind,id,signal,expected,contentRevision) {
    const packet=await this.request('/'+(kind==='node'?'nodes/':'relations/')+encodeURIComponent(id)+(kind==='node'?'?relation_limit=0':''),{signal});
    const match=validateClientInspection(packet,kind,id,expected,contentRevision);
    return {packet,match};
  }
  async sourceDossier(objectId,signal,{limit=SOURCE_DOSSIER_LIMIT}={}) {
    validateDossierRequest(objectId,limit);
    // This is the fixed same-origin source-navigation route.  It is not a
    // configurable endpoint and never becomes an arbitrary filesystem URL.
    const packet=await this.request('/api/source/dossiers/'+encodeURIComponent(objectId)+'?'+new URLSearchParams({limit:String(limit)}),{signal});
    return validateSourceDossier(packet,objectId);
  }
  async readMaterial(kind,id,signal,expected,contentRevision,{language='ru',relation=null}={}) {
    const inspect=materialResult(checkClientMaterialRequest(kind,id,language,relation,expected));
    let revision=expected;
    if(inspect){
      const identity=await this.inspect(kind,id,signal,expected,contentRevision);
      relation=identity.match;revision=identity.packet.source_revision;
    }
    let spec;
    if(kind==='relation')spec=relationSpec(relation);
    else{spec=focusSpec(id,{depth:0});spec.traversal.profile='all';spec.limits={nodes:1,relations:0,groups:1};}
    spec={...spec,lens_id:'sophia-observatory-material',language,detail:'full',explain:false};
    const packet=await this.compile(spec,signal,revision);
    const {match}=materialResult(checkClientMaterialPacket(packet,kind,id,contentRevision));
    for(const item of [...packet.nodes,...packet.relations]){validateHumanForms(item,language);await verifyReadableContext(item);}
    return {packet,match,endpoints:kind==='relation'?packet.nodes:[]};
  }
  async readClaimMaterial(scene,path,signal,{language='ru'}={}){
    validateArea(scene);
    return this.readClaimReference(claimMaterialReference(scene,path),signal,
      {language,expected:scene.source_revision,versions:materialVersions(scene)});
  }
  async readClaimReference(reference,signal,{language='ru',expected=null,versions=null}={}){
    if(!contentLanguage(language))materialResult('material_request');
    const ref=validateClaimReference(reference,reference?.claimId);
    const closure={nodeIds:ref.closureNodeIds,relationIds:[...ref.relationIds,...ref.detailRelationIds]};
    const spec={...focusSpec(ref.nodeIds[0],{depth:0}),seed:{},lens_id:'sophia-observatory-claim-material',language,detail:'full',explain:false,
      node_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.nodeIds}]},
      relation_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.relationIds}]},
      traversal:{depth:0,direction:'either',profile:'all'},
      limits:{nodes:closure.nodeIds.length,relations:closure.relationIds.length,groups:closure.nodeIds.length}};
    const packet=await this.compile(spec,signal,expected);
    materialResult(checkClientClaimScope(packet,closure));
    for(const kind of ['nodes','relations'])for(const item of packet[kind]){
      if(versions&&item.content_revision!==versions[kind]?.[item.id])throw new RevisionError();
      validateHumanForms(item,language);await verifyReadableContext(item);
    }
    const {selected,returned}=materialResult(checkClientClaimPath(packet,ref,closure,packetHost));
    return {packet,match:returned.node,endpoints:[],path:selected};
  }
  capabilities(signal){return this.request('/explore/capabilities',{signal});}
}

// Pure presentation mapping. Opaque IDs never merge through entity_id or title.
// Existing positions survive replacement; they express UI layout, not meaning.
function compareLensNodes(a,b,focus,degree){
  return Number(b.id===focus)-Number(a.id===focus)
    ||(degree.get(b.id)||0)-(degree.get(a.id)||0)
    ||a.id.localeCompare(b.id,'en');
}

export function projectLens(packet,previous=[]) {
  if(packet.schema==='tos_exploration_result_v1')validateExploration(packet);else validateLens(packet);
  const existing=new Map(previous.map(node=>[node.id,node])),degree=new Map();
  for(const relation of packet.relations){
    const from=relation.from_id,fromCount=degree.get(relation.from_id);degree.set(from,(fromCount||0)+1);
    const to=relation.to_id,toCount=degree.get(relation.to_id);degree.set(to,(toCount||0)+1);
  }
  const focus=packet.focus?.node_id;
  const ordered=packet.nodes.slice().sort((a,b)=>compareLensNodes(a,b,focus,degree));
  const currentIds=new Set(packet.nodes.map(node=>node.id));
  const occupied=new Set(previous.filter(node=>currentIds.has(node.id)).map(node=>node.slot));
  let nextSlot=0;
  return ordered.map((raw,index)=>{
    const old=existing.get(raw.id);
    while(occupied.has(nextSlot))nextSlot++;
    const slot=old?.slot??nextSlot++;
    occupied.add(slot);
    const position=lensPosition(raw.id,slot);
    const prefix={id:raw.id,raw,...nodeLabels(raw)};
    const main=index<8,above=index%3===1;
    const group=raw.id===focus?1:raw.kind_id==='agent'?0:raw.kind_id==='expression'?2:1;
    return {...prefix,main,above,group,slot,
      p:old?.p?.slice()||position,
      sourcePosition:old?.sourcePosition?.slice()||position.slice(),
      volumeZ:old?.volumeZ??position[2],
      pos:old?.pos?.slice()||position.slice(),
      target:old?.target?.slice()||position.slice()};
  });
}
