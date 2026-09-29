import {createRouteCenterSession,routeRelationMember,createLensSortSession,createLensNodeSession,lensDegreeNext,lensSlotPosition} from './lens-projection-rules.mjs';
import {browserFocusSpec,browserRelationSpec} from './lens-spec-rules.mjs';
import {createClientPacketSession,createClientMaterialSession,packetKindUnits,packetMissing,createClientJsonSession,createClientSelectorSession,packetSetKey,packetString} from './client-packet-rules.mjs';
import {createSourceDossierSession,dossierTextUnits,dossierPredicate} from './source-dossier-rules.mjs';
import {createClientInspectionSession,createClientItemsSession,inspectionRefInvalid,inspectionRevisionText,inspectionRevisionUnits} from './client-inspection-rules.mjs';
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
export function isSourceDossierRef(value){
  if(!dossierPredicate('reference_size',typeof value==='string',typeof value==='string'?value.length:0))return false;
  return dossierPredicate('reference_value',dossierTextUnits(value));
}
// Transport bounds are not a license to draw or retain every delivered page.
export const EXPLORATION_BUDGET = Object.freeze({nodes:302,relations:101});
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
  const session=createClientJsonSession();let keys,result;
  try{while(true){switch(session.need()){
    case 'identity':session.observe(left===right);break;
    case 'left-null':session.observe(left===null);break;
    case 'right-null':session.observe(right===null);break;
    case 'left-object':session.observe(typeof left==='object');break;
    case 'right-object':session.observe(typeof right==='object');break;
    case 'left-array-first':case 'left-array-second':session.observe(Array.isArray(left));break;
    case 'right-array-first':case 'right-array-second':session.observe(Array.isArray(right));break;
    case 'array-length':session.observe(left.length===right.length);break;
    case 'array-every':result=left.every((value,index)=>sameJson(value,right[index]));session.observe(Boolean(result));break;
    case 'object-keys':keys=Object.keys(left);session.observe(true);break;
    case 'object-key-count':session.observe(keys.length===Object.keys(right).length);break;
    case 'object-every':result=keys.every(key=>{
      if(packetMissing(Object.hasOwn(right,key)))return false;
      return sameJson(left[key],right[key]);
    });session.observe(Boolean(result));break;
    case 'done':return session.result()===2?result:session.result()===1;
    default:throw new Error('Invalid exact JSON Rust need');
  }}}finally{session.free();}
}
// The original request remains on the wire; only comparison treats the two
// owner-normalized selector fields as sets.
export function explorationRequestMatches(normalized,requested){
  if(packetMissing(Boolean(normalized)))return false;
  return Object.entries(requested).every(([key,value])=>{
    if(packetMissing(packetSetKey(key)))return sameJson(normalized[key],value);
    const actual=normalized[key];
    return validateRequestSelectorSet(value,actual);
  });
}
function validateRequestSelectorSet(value,actual){
  const session=createClientSelectorSession();let result;
  try{while(true){switch(session.need()){
    case 'requested-array':session.observe(Array.isArray(value));break;
    case 'actual-array':session.observe(Array.isArray(actual));break;
    case 'strings':result=value.every(packetString);session.observe(Boolean(result));break;
    case 'actual-unique':session.observe(new Set(actual).size===actual.length);break;
    case 'requested-count':session.observe(new Set(value).size===actual.length);break;
    case 'members':result=value.every(item=>actual.includes(item));session.observe(Boolean(result));break;
    case 'done':return session.result()===2?result:session.result()===1;
    default:throw new Error('Invalid selector comparison Rust need');
  }}}finally{session.free();}
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
  const session=createRouteCenterSession();let identity,packet,error,relationError=null;
  const observe=value=>{try{session.observe(value);}catch(failure){
    if(failure==='route-id')throw new ContractError(t("Не указан центр области."));
    if(failure==='route-presence')throw new ContractError(t("Выбранное отношение отсутствует в области."));
    throw failure;
  }};
  try{while(true){const phase=session.need();
    try{switch(phase){
      case 'id-type':observe(typeof id==='string');break;
      case 'id-trim':observe(Boolean(id.trim()));break;
      case 'relation-inspect':identity=await client.inspect('relation',id,signal,expected);observe(true);break;
      case 'relation-compile':{const revision=identity.packet.source_revision;packet=await client.compile(relationSpec(identity.match),signal,revision);observe(true);break;}
      case 'relation-presence':observe(Boolean(packet.relations.some(item=>routeRelationMember(item.id===id))));break;
      case 'relation-return':return {packet,kind:'relation',relation:identity.match};
      case 'relation-error-instance':observe(error instanceof RequestError);break;
      case 'relation-error-status':observe(error.status===404);break;
      case 'node-compile':packet=await client.compile(focusSpec(id,{depth}),signal,expected);observe(true);break;
      case 'node-return':return {packet,kind:'node'};
      case 'node-error-prior':observe(Boolean(relationError));break;
      case 'node-error-instance':observe(error instanceof RequestError);break;
      case 'node-error-status':observe(error.status===404);break;
      case 'throw-current':throw error;
      case 'throw-relation':throw relationError;
      default:throw new Error('Invalid route center Rust need');
    }}catch(failure){
      if(phase==='relation-inspect'||phase==='relation-compile'||phase==='relation-presence'||phase==='relation-return'){
        relationError=error=failure;session.failed();
      }else if(phase==='node-compile'||phase==='node-return'){error=failure;session.failed();}
      else throw failure;
    }
  }}finally{session.free();}
}
export function checkRevision(packet,expected){return validateClientPacket(packet,0,expected);}
function dossierMethod(session,method,...values){
  try{return session[method](...values);}catch(error){
    const messages={request:'Неверная ссылка на досье источника.',dossier:'Неподдерживаемое досье источника.',chain:'Неполная цепочка источника.'};
    if(typeof error==='string'&&Object.hasOwn(messages,error))throw new ContractError(t(messages[error]));
    throw error;
  }
}
function dossierReference(session,value){
  dossierMethod(session,'reference_length',typeof value==='string',typeof value==='string'?value.length:0);
  dossierMethod(session,'reference',dossierTextUnits(value));
}
function validateDossierRequest(objectId,limit){
  const session=createSourceDossierSession(true);
  try{dossierReference(session,objectId);dossierMethod(session,'request_limit',typeof limit==='number',typeof limit==='number'?limit:0);}
  finally{session.free();}
}
export function validateSourceDossier(packet,expected){
  const summary=packet?.agent_summary,object=packet?.object,session=createSourceDossierSession();
  const run=(method,...values)=>dossierMethod(session,method,...values);
  let array;
  const recordObservation=value=>{
    if(!run('record_type',value!==null,typeof value==='object'))return false;
    return run('record_array',Array.isArray(value));
  };
  const recordElement=value=>{
    if(!dossierPredicate('record_element_type',value!==null,typeof value==='object'))return false;
    return dossierPredicate('record_element_array',Array.isArray(value));
  };
  const text=value=>{run('text_length',typeof value==='string',typeof value==='string'?value.length:0);run('text',dossierTextUnits(value));};
  try{
    while(true){
      switch(session.need()){
        case 'schema':text(packet?.schema);break;
        case 'expected-ref':dossierReference(session,expected);break;
        case 'object-id':run('observe',packet.object_id===expected);break;
        case 'object':recordObservation(object);break;
        case 'node-id':run('observe',object.node_id===expected);break;
        case 'kind':text(object.node_kind);break;
        case 'summary':run('observe',Boolean(summary));break;
        case 'technical-type':run('observe',typeof summary.technical_access==='string');break;
        case 'technical-truthy':run('observe',Boolean(summary.technical_access));break;
        case 'posture-type':run('observe',typeof summary.rights_posture==='string');break;
        case 'posture-truthy':run('observe',Boolean(summary.rights_posture));break;
        case 'review-type':run('observe',typeof summary.human_review_required==='boolean');break;
        case 'legal-type':run('observe',typeof summary.can_conclude_legal_openness==='boolean');break;
        case 'license-false':{const value=summary.availability_is_license;run('license',typeof value==='boolean',Boolean(value));break;}
        case 'array-value':{
          const field=session.array_field();array=session.array_location()==='summary'?summary[field]:packet[field];
          run('array_type',Array.isArray(array));break;
        }
        case 'array-length':run('array_length',Number(array.length));break;
        case 'array-every':{
          const callback=session.element_kind()==='string'
            ?value=>dossierPredicate('string_element',typeof value==='string',typeof value==='string'?value.length:0):recordElement;
          run('observe',Boolean(array.every(callback)));break;
        }
        case 'truncated':run('observe',typeof packet.truncated==='boolean');break;
        case 'authority-type':run('observe',typeof packet.authority_note==='string');break;
        case 'authority-length':run('authority_length',Number(packet.authority_note.length));break;
        case 'authority-record':recordObservation(packet.authority_note);break;
        case 'chain-truthy':run('observe',Boolean(packet.chain));break;
        case 'chain-type':run('observe',typeof packet.chain==='object');break;
        case 'chain-array':run('observe',Array.isArray(packet.chain));break;
        case 'chain-values':run('observe',Boolean(Object.values(packet.chain).some(value=>{
          if(!dossierPredicate('chain_array_type',Array.isArray(value)))return dossierPredicate('chain_result',false);
          if(!dossierPredicate('chain_array_length',Number(value.length)))return dossierPredicate('chain_result',false);
          return dossierPredicate('chain_result',Boolean(value.every(recordElement)));
        })));break;
        case 'done':return packet;
        default:throw new Error('Invalid source dossier Rust need');
      }
    }
  }finally{session.free();}
}
function packetMethod(session,method,...values){
  try{return session[method](...values);}catch(error){
    if(error==='revision')throw new RevisionError();
    const messages={missing_revision:'Ответ не содержит версию данных.',lens:'Неподдерживаемый контракт линзы.',
      area:'Неподдерживаемый контракт области.',budget:'Область превышает бюджет отображения.',
      endpoints:'Связь не содержит оба конца в области.',focus:'Центр отсутствует в области.',
      exploration:'Неполная страница раскрытия связей.',request_area:'Сервер вернул другую область раскрытия.',search:'Неподдерживаемый ответ поиска.'};
    if(typeof error==='string'&&Object.hasOwn(messages,error))throw new ContractError(t(messages[error]));
    throw error;
  }
}
function checkItems(items,kind){
  const session=createClientItemsSession(kind==='node');let ids;
  const run=(method,...values)=>{
    try{return session[method](...values);}catch(error){
      if(error==='items')throw new ContractError(t('Неверный список объектов.'));
      if(error==='item')throw new ContractError(t('Неполный или повторяющийся объект.'));
      throw error;
    }
  };
  try{
    run('observe',Array.isArray(items));ids=new Set();
    for(const item of items){
      run('observe',true);
      while(session.need()!=='item-next'){
        switch(session.need()){
          case 'item-truthy':run('observe',Boolean(item));break;
          case 'id-type':run('observe',typeof item.id==='string');break;
          case 'id-truthy':run('observe',Boolean(item.id));break;
          case 'id-duplicate':run('observe',ids.has(item.id));break;
          case 'display':run('observe',Boolean(item.display));break;
          case 'localized-title':run('observe',Boolean(localized(item.display.title)));break;
          case 'localized-label':run('observe',Boolean(localized(item.display.label)));break;
          case 'content-revision':{
            const text=inspectionRevisionText(item.content_revision||'');run('text_length',text.length);run('text',inspectionRevisionUnits(text));break;
          }
          case 'refs-array':run('observe',Array.isArray(item.source_refs));break;
          case 'refs-length':run('observe',Boolean(item.source_refs.length));break;
          case 'refs':run('observe',Boolean(item.source_refs.some(ref=>inspectionRefInvalid(typeof ref==='string',Boolean(ref)))));break;
          case 'add-id':ids.add(item.id);run('observe',true);break;
          default:throw new Error('Invalid client item Rust need');
        }
      }
    }
    run('observe',false);return ids;
  }finally{session.free();}
}
function validateClientPacket(packet,mode,expected=null,previous=null,options={}){
  const session=createClientPacketSession(mode,Boolean(expected),Boolean(previous),options.mode==='indexed');
  const run=(method,...values)=>packetMethod(session,method,...values);
  let ids,nodes,relations,page,origin,query,primary,context,items,inclusion,item,endpoint,node,scene;
  const text=value=>{run('text_length',typeof value==='string',typeof value==='string'?value.length:0);run('text',inspectionRevisionUnits(value));};
  const digest=value=>{
    run('digest_type',typeof value==='string');
    const valueText=session.strict_digest()?value:inspectionRevisionText(value);
    run('text_length',true,valueText.length);run('text',inspectionRevisionUnits(valueText));
  };
  try{
    while(true){
      const need=session.need();
      switch(need){
        case 'lens-schema':text(packet?.schema);break;
        case 'exploration-schema':run('select_v2',packet?.schema==='tos_exploration_result_v2');break;
        case 'revision':digest(packet?.source_revision||'');break;
        case 'expected':run('observe',packet.source_revision===expected);break;
        case 'authority-source':run('observe',packet.authority_boundary?.is_source===false);break;
        case 'authority-canon':run('observe',packet.authority_boundary?.is_canon===false);break;
        case 'authority-writes':run('observe',packet.authority_boundary?.writes_to_tree===false);break;
        case 'nodes-array':run('observe',Array.isArray(packet.nodes));break;
        case 'relations-array':run('observe',Array.isArray(packet.relations));break;
        case 'nodes-length':run('number',Number(packet.nodes.length));break;
        case 'relations-length':run('number',Number(packet.relations.length));break;
        case 'node-items':ids=checkItems(packet.nodes,'node');run('observe',true);break;
        case 'relation-items':checkItems(packet.relations,'relation');run('observe',true);break;
        case 'area-endpoints':{const areaIds=ids;run('observe',Boolean(packet.relations.some(relation=>{
          if(packetMissing(areaIds.has(relation.from_id)))return true;
          return packetMissing(areaIds.has(relation.to_id));
        })));break;}
        case 'area-focus':run('observe',Boolean(packet.focus));break;
        case 'area-focus-member':run('observe',ids.has(packet.focus.node_id));break;
        case 'exploration-snapshots':{
          if(session.v2()){
            nodes=new Map(packet.nodes.map(item=>[item.id,item]));relations=new Map(packet.relations.map(item=>[item.id,item]));
            origin=packet.origin;page=packet.page;query=packet.query;
          }else{page=packet.page;ids=new Set(packet.nodes.map(node=>node.id));}
          run('observe',true);break;
        }
        case 'v1-schema':text(packet.schema);break;
        case 'writes':run('observe',packet.writes_to_tree===false);break;
        case 'snapshot':digest(session.v2()?packet.snapshot_revision:packet.snapshot_revision||'');break;
        case 'execution':text(packet.execution_version);break;
        case 'status':text(packet.status);break;
        case 'v1-focus':run('observe',Boolean(packet.focus));break;
        case 'origin':run('observe',Boolean(origin));break;
        case 'origin-kind':text(origin.kind);break;
        case 'origin-revision':digest(origin.content_revision);break;
        case 'origin-id-type':run('observe',typeof origin.id==='string');break;
        case 'origin-id-truthy':run('observe',Boolean(origin.id));break;
        case 'query':run('observe',Boolean(query));break;
        case 'query-schema':text(query.schema_version);break;
        case 'query-revision':run('observe',query.source_revision===packet.source_revision);break;
        case 'query-origin':run('observe',Boolean(query.origin));break;
        case 'query-origin-fields':run('observe',Boolean(['kind','id','content_revision'].some(key=>packetMissing(query.origin[key]===origin[key]))));break;
        case 'focus-own':run('observe',Object.hasOwn(packet,'focus'));break;
        case 'page-integer':{const value=page?.number;run('integer',typeof value==='number',typeof value==='number'?value:0);break;}
        case 'page-positive':run('number',Number(page.number));break;
        case 'page-scope':text(page.scope);break;
        case 'returned-nodes':run('observe',page.returned_nodes===(session.v2()?nodes.size:packet.nodes.length));break;
        case 'returned-relations':run('observe',page.returned_relations===(session.v2()?relations.size:packet.relations.length));break;
        case 'work-integer':{const value=page.work_units;run('integer',typeof value==='number',typeof value==='number'?value:0);break;}
        case 'work-lower':run('number',Number(page.work_units));break;
        case 'work-upper':run('number',Number(page.work_units));break;
        case 'v1-primary-array':run('observe',Array.isArray(page.primary_node_ids));break;
        case 'v1-context-array':run('observe',Array.isArray(page.context_node_ids));break;
        case 'v1-total':run('observe',page.primary_node_ids.length+page.context_node_ids.length===ids.size);break;
        case 'v1-unique':run('observe',new Set([...page.primary_node_ids,...page.context_node_ids]).size===ids.size);break;
        case 'v1-members':run('observe',Boolean([...page.primary_node_ids,...page.context_node_ids].some(id=>packetMissing(ids.has(id)))));break;
        case 'counts-scope':text(packet.counts?.scope);break;
        case 'inclusion-authority':text(packet.inclusion?.authority);break;
        case 'cursor-status':run('observe',packet.status==='paused');break;
        case 'cursor-digest':digest(session.v2()?page.next_cursor:page.next_cursor||'');break;
        case 'cursor-null-value':run('observe',page.next_cursor===null);break;
        case 'limit-status':run('observe',packet.status==='limit_reached');break;
        case 'limit-reason':text(packet.limit_reason);break;
        case 'limit-null':run('observe',packet.limit_reason===null);break;
        case 'partition-start':{
          const kind=session.relation_partition()?'relation':'node';items=session.relation_partition()?relations:nodes;
          primary=page[`primary_${kind}_ids`];context=page[`context_${kind}_ids`];run('observe',true);break;
        }
        case 'primary-array':run('observe',Array.isArray(primary));break;
        case 'context-array':run('observe',Array.isArray(context));break;
        case 'partition-total':run('observe',primary.length+context.length===items.size);break;
        case 'partition-unique':run('observe',new Set([...primary,...context]).size===items.size);break;
        case 'partition-members':{const partitionItems=items;run('observe',Boolean([...primary,...context].some(id=>packetMissing(partitionItems.has(id)))));break;}
        case 'partition-inclusion':inclusion=packet.inclusion[session.relation_partition()?'relations':'nodes'];run('observe',Boolean(inclusion));break;
        case 'inclusion-count':run('observe',Object.keys(inclusion).length===items.size);break;
        case 'inclusion-keys':{const partitionInclusion=inclusion;run('observe',Boolean([...items.keys()].some(id=>packetMissing(Object.hasOwn(partitionInclusion,id)))));break;}
        case 'partition-next':run('observe',true);break;
        case 'origin-item':item=(origin.kind==='node'?nodes:relations).get(origin.id);run('observe',Boolean(item));break;
        case 'origin-item-revision':run('observe',item.content_revision===origin.content_revision);break;
        case 'origin-branch':run('observe',origin.kind==='node');break;
        case 'origin-no-endpoints':run('observe',Object.hasOwn(origin,'endpoints'));break;
        case 'context-relations-empty':run('observe',Boolean(page.context_relation_ids.length));break;
        case 'origin-node-context':run('observe',Boolean(page.context_node_ids.includes(origin.id)));break;
        case 'context-relation-count':run('observe',page.context_relation_ids.length===1);break;
        case 'context-relation-id':run('observe',page.context_relation_ids[0]===origin.id);break;
        case 'origin-inclusion':text(packet.inclusion[session.origin_relation()?'relations':'nodes'][origin.id]?.kind);break;
        case 'endpoint-start':{
          const side=session.endpoint_to()?'to':'from';endpoint=origin.endpoints?.[side];node=nodes.get(item[`${side}_id`]);run('observe',true);break;
        }
        case 'endpoint':run('observe',Boolean(endpoint));break;
        case 'endpoint-node':run('observe',Boolean(node));break;
        case 'endpoint-id':run('observe',endpoint.node_id===node.id);break;
        case 'endpoint-revision':run('observe',endpoint.content_revision===node.content_revision);break;
        case 'endpoint-entity':run('observe',endpoint.entity_id===node.entity_id);break;
        case 'endpoint-context':run('observe',Boolean(page.context_node_ids.includes(node.id)));break;
        case 'endpoint-inclusion':text(packet.inclusion.nodes[node.id]?.kind);break;
        case 'endpoint-next':run('observe',true);break;
        case 'previous':run('observe',true);break;
        case 'prior-schema':run('observe',previous.schema===packet.schema);break;
        case 'prior-status':text(previous.status);break;
        case 'prior-source':run('observe',packet.source_revision===previous.source_revision);break;
        case 'prior-snapshot':run('observe',packet.snapshot_revision===previous.snapshot_revision);break;
        case 'prior-execution':run('observe',packet.execution_version===previous.execution_version);break;
        case 'prior-focus':run('observe',packet.focus.node_id===previous.focus.node_id);break;
        case 'prior-page':run('observe',page.number===previous.page.number+1);break;
        case 'prior-cursor-status':run('observe',packet.status==='paused');break;
        case 'prior-cursor-different':run('observe',page.next_cursor===previous.page.next_cursor);break;
        case 'prior-origin':run('observe',sameJson(packet.origin,previous.origin));break;
        case 'prior-query':run('observe',sameJson(packet.query,previous.query));break;
        case 'prior-query-json':run('observe',JSON.stringify(packet.query)===JSON.stringify(previous.query));break;
        case 'scene':{
          const focusNode=origin.kind==='node'?origin.id:null;
          let success=true;try{scene=knowledgeScene(packet.nodes,packet.relations,focusNode,origin.kind==='relation'?origin.id:null);}catch{success=false;}
          run('observe',success);break;
        }
        case 'scene-equal':run('observe',sameJson(packet.scene,scene));break;
        case 'search-schema':text(packet.schema);break;
        case 'search-nodes-length':run('search_length',Number(packet.nodes?.length),options.limit);break;
        case 'search-relations-length':run('search_length',Number(packet.relations?.length),options.limit);break;
        case 'search-cursor':run('observe',packet.page?.cursor===options.cursor);break;
        case 'search-limit':run('observe',packet.page.limit_per_kind===options.limit);break;
        case 'search-more-type':run('observe',typeof packet.page.has_more==='boolean');break;
        case 'search-more':run('observe',Boolean(packet.page.has_more));break;
        case 'search-next-type':run('observe',typeof packet.page.next_cursor==='string');break;
        case 'search-next-truthy':run('observe',Boolean(packet.page.next_cursor));break;
        case 'search-next-null':run('observe',packet.page.next_cursor===null);break;
        case 'search-writes':run('observe',packet.authority_boundary?.writes_to_tree===false);break;
        case 'done':return packet;
        default:throw new Error('Invalid client packet Rust need');
      }
    }
  }finally{session.free();}
}
function validateArea(packet,expected=null){return validateClientPacket(packet,1,expected);}
export function validateLens(packet,expected=null){return validateClientPacket(packet,2,expected);}
export function validateExploration(packet,expected=null,previous=null){return validateClientPacket(packet,3,expected,previous);}
function validateExplorationRequestPacket(packet,query,previous){
  const session=createClientPacketSession(6,false,Boolean(previous),false);
  try{while(true){switch(session.need()){
    case 'initial-request':packetMethod(session,'observe',true);break;
    case 'request-match':packetMethod(session,'observe',Boolean(explorationRequestMatches(packet.query,query)));break;
    case 'done':return packet;
    default:throw new Error('Invalid exploration request identity Rust need');
  }}}finally{session.free();}
}
function materialMethod(session,method,...values){
  try{return session[method](...values);}catch(error){
    if(error==='revision')throw new RevisionError();
    if(error==='form')throw new FormContractError();
    const messages={material_request:'Неверный запрос материала.',match:'Не найден точный идентификатор карточки.',material_scope:'Ответ вышел за границы выбранного материала.'};
    if(typeof error==='string'&&Object.hasOwn(messages,error))throw new ContractError(t(messages[error]));
    throw error;
  }
}
function validateSearchRequest(cursor,search_mode,limit){
  const session=createClientPacketSession(4,false,false,false),run=(method,...values)=>packetMethod(session,method,...values);
  try{while(true){switch(session.need()){
    case 'cursor-null':run('observe',cursor===null);break;
    case 'request-cursor-type':run('observe',typeof cursor==='string');break;
    case 'request-cursor-length':run('number',cursor.length);break;
    case 'request-mode':run('observe',Boolean(search_mode));break;
    case 'request-limit-type':run('integer',typeof limit==='number',typeof limit==='number'?limit:0);break;
    case 'request-limit':run('number',limit);break;
    case 'done':return;
    default:throw new Error('Invalid search request Rust need');
  }}}finally{session.free();}
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
  const session=createClientInspectionSession(kind==='node',kind==='relation',Boolean(expected),Boolean(contentRevision));
  const run=(method,...values)=>{
    // Source arguments evaluate outside the direct WASM refusal boundary.
    try{return session[method](...values);}catch(error){
      if(error==='revision')throw new RevisionError();
      const messages={missing_revision:'Ответ не содержит версию данных.',schema:'Неверная карточка.',
        items:'Неверный список объектов.',item:'Неполный или повторяющийся объект.',
        match:'Не найден точный идентификатор карточки.',endpoints:'Неполные концы связи.'};
      if(typeof error==='string'&&Object.hasOwn(messages,error))throw new ContractError(t(messages[error]));
      throw error;
    }
  };
  let item,match,ids;
  const digest=value=>{
    const text=inspectionRevisionText(value);run('text_length',text.length);run('text',inspectionRevisionUnits(text));
  };
  const observeItem=need=>{
    switch(need){
        case 'item-truthy':run('observe',Boolean(item));break;
        case 'id-type':run('observe',typeof item.id==='string');break;
        case 'id-truthy':run('observe',Boolean(item.id));break;
        case 'id-duplicate':run('observe',ids.has(item.id));break;
        case 'display':run('observe',Boolean(item.display));break;
        case 'localized-title':run('observe',Boolean(localized(item.display.title)));break;
        case 'localized-label':run('observe',Boolean(localized(item.display.label)));break;
        case 'content-revision':digest(item.content_revision||'');break;
        case 'refs-array':run('observe',Array.isArray(item.source_refs));break;
        case 'refs-length':run('observe',Boolean(item.source_refs.length));break;
        case 'refs':run('observe',Boolean(item.source_refs.some(ref=>{
          return inspectionRefInvalid(typeof ref==='string',Boolean(ref));
        })));break;
        case 'add-id':ids.add(item.id);run('observe',true);break;
      default:throw new Error('Invalid client inspection item need');
    }
  };
  try{
    while(true){
      const need=session.need();
      switch(need){
        case 'revision':digest(packet?.source_revision||'');break;
        case 'expected':run('observe',packet.source_revision===expected);break;
        case 'schema':{
          const value=packet.schema;run('schema_length',typeof value==='string',typeof value==='string'?value.length:0);
          run('text',inspectionRevisionUnits(value));break;
        }
        case 'matches':case 'endpoints':{
          const values=need==='matches'?packet.matches:packet.endpoints;
          run('observe',Array.isArray(values));ids=new Set();
          // Native for-of owns IteratorClose and exception precedence.
          for(const value of values){
            item=value;run('observe',true);
            while(session.need()!=='item-next')observeItem(session.need());
          }
          run('observe',false);break;
        }
        case 'exact':match=packet.matches.find(value=>value.id===id);run('observe',Boolean(match));break;
        case 'content-expected':run('observe',match.content_revision===contentRevision);break;
        case 'from-endpoint':run('observe',ids.has(match.from_id));break;
        case 'to-endpoint':run('observe',ids.has(match.to_id));break;
        case 'done':return match;
        default:throw new Error('Invalid client inspection Rust need');
      }
    }
  }finally{session.free();}
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
    const session=createClientMaterialSession(false,false),run=(method,...values)=>materialMethod(session,method,...values);
    let spec,revision=expected,packet,match,allowed;
    try{while(true){switch(session.need()){
      case 'kind':run('kind',typeof kind==='string',packetKindUnits(kind));break;
      case 'id-type':run('observe',typeof id==='string');break;
      case 'id-truthy':run('observe',Boolean(id));break;
      case 'language':run('observe',Boolean(contentLanguage(language)));break;
      case 'relation-truthy':run('observe',Boolean(relation));break;
      case 'relation-id':run('observe',relation.id===id);break;
      case 'expected':run('observe',Boolean(expected));break;
      case 'inspect':{
        const identity=await this.inspect(kind,id,signal,expected,contentRevision);
        relation=identity.match;revision=identity.packet.source_revision;run('observe',true);break;
      }
      case 'spec':{
        if(session.relation())spec=relationSpec(relation);
        else{spec=focusSpec(id,{depth:0});spec.traversal.profile='all';spec.limits={nodes:1,relations:0,groups:1};}
        spec={...spec,lens_id:'sophia-observatory-material',language,detail:'full',explain:false};run('observe',true);break;
      }
      case 'compile':packet=await this.compile(spec,signal,revision);run('observe',true);break;
      case 'match':match=packet[session.relation()?'relations':'nodes'].find(item=>item.id===id);run('observe',Boolean(match));break;
      case 'content-required':run('observe',Boolean(contentRevision));break;
      case 'content-equal':run('observe',match.content_revision===contentRevision);break;
      case 'allowed':allowed=new Set(session.relation()?[match.from_id,match.to_id]:[id]);run('observe',true);break;
      case 'scope-node-count':run('observe',packet.nodes.length===allowed.size);break;
      case 'scope-node-members':run('observe',Boolean(packet.nodes.some(node=>packetMissing(allowed.has(node.id)))));break;
      case 'scope-relation-count':run('observe',packet.relations.length===(session.relation()?1:0));break;
      case 'forms':for(const item of [...packet.nodes,...packet.relations]){validateHumanForms(item,language);await verifyReadableContext(item);}run('observe',true);break;
      case 'done':return {packet,match,endpoints:session.relation()?packet.nodes:[]};
      default:throw new Error('Invalid material Rust need');
    }}}finally{session.free();}
  }
  async readClaimMaterial(scene,path,signal,{language='ru'}={}){
    validateArea(scene);
    return this.readClaimReference(claimMaterialReference(scene,path),signal,
      {language,expected:scene.source_revision,versions:materialVersions(scene)});
  }
  async readClaimReference(reference,signal,{language='ru',expected=null,versions=null}={}){
    const session=createClientMaterialSession(true,Boolean(versions)),run=(method,...values)=>materialMethod(session,method,...values);
    let ref,closure,spec,packet,selected,returned,closureNodes,closureRelations,nodeItems,relationItems;
    try{while(true){switch(session.need()){
      case 'language':run('observe',Boolean(contentLanguage(language)));break;
      case 'reference':ref=validateClaimReference(reference,reference?.claimId);closure={nodeIds:ref.closureNodeIds,relationIds:[...ref.relationIds,...ref.detailRelationIds]};run('observe',true);break;
      case 'spec':{
        spec={...focusSpec(ref.nodeIds[0],{depth:0}),seed:{},lens_id:'sophia-observatory-claim-material',language,detail:'full',explain:false,
          node_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.nodeIds}]},
          relation_query:{enabled:true,filters:[{field:'id',op:'in',value:closure.relationIds}]},
          traversal:{depth:0,direction:'either',profile:'all'},
          limits:{nodes:closure.nodeIds.length,relations:closure.relationIds.length,groups:closure.nodeIds.length}};
        run('observe',true);break;
      }
      case 'compile':packet=await this.compile(spec,signal,expected);run('observe',true);break;
      case 'claim-nodes-count':nodeItems=packet.nodes;run('observe',nodeItems.length===closure.nodeIds.length);break;
      case 'claim-nodes-every':run('observe',Boolean(nodeItems.every(item=>closure.nodeIds.includes(item.id))));break;
      case 'claim-relations-count':relationItems=packet.relations;run('observe',relationItems.length===closure.relationIds.length);break;
      case 'claim-relations-every':run('observe',Boolean(relationItems.every(item=>closure.relationIds.includes(item.id))));break;
      case 'claim-items':{
        for(const kind of ['nodes','relations'])for(const item of packet[kind]){
          run('begin_item');
          while(session.need()!=='claim-items')switch(session.need()){
            case 'item-version':run('observe',item.content_revision===versions[kind]?.[item.id]);break;
            case 'item-human':validateHumanForms(item,language);run('observe',true);break;
            case 'item-readable':await verifyReadableContext(item);run('observe',true);break;
            default:throw new Error('Invalid Claim material item Rust need');
          }
        }
        run('finish_items');break;
      }
      case 'path':selected=claimPathFor(packet,ref.claimId);run('observe',Boolean(selected));break;
      case 'closure':returned=claimPathClosure(packet,selected);run('observe',true);break;
      case 'path-id':run('observe',selected.id===ref.pathId);break;
      case 'path-relation-type':run('observe',selected.relation_type_id===ref.relationType);break;
      case 'path-nodes-json':run('observe',JSON.stringify(selected.node_ids)===JSON.stringify(ref.nodeIds));break;
      case 'path-relations-json':run('observe',JSON.stringify(selected.relation_ids)===JSON.stringify(ref.relationIds));break;
      case 'closure-nodes-count':closureNodes=returned.nodeIds.map(id=>({id}));run('observe',closureNodes.length===closure.nodeIds.length);break;
      case 'closure-nodes-every':run('observe',Boolean(closureNodes.every(item=>closure.nodeIds.includes(item.id))));break;
      case 'closure-relations-count':closureRelations=returned.relationIds.map(id=>({id}));run('observe',closureRelations.length===closure.relationIds.length);break;
      case 'closure-relations-every':run('observe',Boolean(closureRelations.every(item=>closure.relationIds.includes(item.id))));break;
      case 'done':return {packet,match:returned.node,endpoints:[],path:selected};
      default:throw new Error('Invalid Claim material Rust need');
    }}}finally{session.free();}
  }
  capabilities(signal){return this.request('/explore/capabilities',{signal});}
}

// Pure presentation mapping. Opaque IDs never merge through entity_id or title.
// Existing positions survive replacement; they express UI layout, not meaning.
function compareLensNodes(a,b,focus,degree){
  const session=createLensSortSession();
  try{while(true){switch(session.need()){
    case 'b-focus':session.focus(b.id===focus);break;
    case 'a-focus':session.focus(a.id===focus);break;
    case 'b-degree':{const value=degree.get(b.id);session.degree(Boolean(value),typeof value==='number'?value:0);break;}
    case 'a-degree':{const value=degree.get(a.id);session.degree(Boolean(value),typeof value==='number'?value:0);break;}
    case 'locale':return a.id.localeCompare(b.id,'en');
    case 'done':return session.result();
    default:throw new Error('Invalid lens sort Rust need');
  }}}finally{session.free();}
}
export function projectLens(packet,previous=[]) {
  if(packet.schema==='tos_exploration_result_v1')validateExploration(packet);else validateLens(packet);
  const existing=new Map(previous.map(node=>[node.id,node])),degree=new Map();
  for(const relation of packet.relations){
    const from=relation.from_id,fromCount=degree.get(relation.from_id);degree.set(from,lensDegreeNext(fromCount));
    const to=relation.to_id,toCount=degree.get(relation.to_id);degree.set(to,lensDegreeNext(toCount));
  }
  const focus=packet.focus?.node_id;
  const ordered=packet.nodes.slice().sort((a,b)=>compareLensNodes(a,b,focus,degree));
  const currentIds=new Set(packet.nodes.map(node=>node.id));
  const occupied=new Set(previous.filter(node=>currentIds.has(node.id)).map(node=>node.slot));
  let nextSlot=0;
  return ordered.map((raw,index)=>{
    const old=existing.get(raw.id),session=createLensNodeSession(nextSlot);
    let slot,angle,position,x,y,prefix,main,above,pValue,sourcePosition,volumeZ,pos,target;
    try{while(true){switch(session.need()){
      case 'scan-slot':session.scan_slot(occupied.has(session.next_slot()));nextSlot=session.next_slot();break;
      case 'old-slot':{
        const value=old?.slot,allocated=session.old_slot(value===null,value===undefined);
        nextSlot=session.next_slot();slot=allocated?session.allocated_slot():value;break;
      }
      case 'occupied-add':occupied.add(slot);session.advance();break;
      case 'hash':for(const char of raw.id)session.hash_word(session.hash_value()^char.codePointAt(0));session.hash_done();break;
      case 'angle':angle=slot*session.angle_multiplier();session.advance();break;
      case 'table':position=lensSlotPosition(slot);session.table_candidate(Boolean(position));break;
      case 'fallback-x':x=Math.cos(angle)*(session.x_base()+(slot%session.x_cycle())*session.x_spacing());session.advance();break;
      case 'fallback-y':y=Math.sin(angle)*(session.y_base()+(slot%session.y_cycle())*session.y_spacing());session.advance();break;
      case 'fallback-z':position=[x,y,session.fallback_z()];session.advance();break;
      case 'prefix':prefix={id:raw.id,raw,...nodeLabels(raw)};session.advance();break;
      case 'main':main=session.main_flag(Number(index));break;
      case 'above':above=session.above_flag(index%session.above_cycle());break;
      case 'group-id':session.group_focus(raw.id===focus);break;
      case 'kind-agent':case 'kind-expression':{
        const value=raw.kind_id;
        if(session.kind_length(typeof value==='string',typeof value==='string'?value.length:0))session.kind(inspectionRevisionUnits(value));
        else session.kind_mismatch();break;
      }
      case 'restore-p':{const value=old?.p?.slice();pValue=session.restore_candidate(Boolean(value))===0?value:position;break;}
      case 'restore-source':{const value=old?.sourcePosition?.slice();if(session.restore_candidate(Boolean(value))===0)sourcePosition=value;break;}
      case 'clone-source':sourcePosition=position.slice();session.advance();break;
      case 'volume':{const value=old?.volumeZ;volumeZ=session.volume_candidate(value===null,value===undefined)?value:position[2];break;}
      case 'restore-pos':{const value=old?.pos?.slice();if(session.restore_candidate(Boolean(value))===0)pos=value;break;}
      case 'clone-pos':pos=position.slice();session.advance();break;
      case 'restore-target':{const value=old?.target?.slice();if(session.restore_candidate(Boolean(value))===0)target=value;break;}
      case 'clone-target':target=position.slice();session.advance();break;
      case 'done':return {...prefix,main,above,group:session.group(),slot,p:pValue,sourcePosition,volumeZ,pos,target};
      default:throw new Error('Invalid lens projection Rust need');
    }}}finally{session.free();}
  });
}
