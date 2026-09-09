import {t,ui,uiComputed} from './ui-i18n.mjs';
import {ContractError,RequestSlots,RequestError,RevisionError,checkRevision,displayTitleForm,materialDisplayForm,claimMaterialReference,materialVersions} from './knowledge-client.mjs';
import {essentialContext} from './record-context.mjs';
import {displayForm as readingForm,displayLanguageKey as languageKey} from './display-language.mjs';
export {readingForm};
import {FORM_ROLES,FormContractError,validateHumanForms,inspectExactHumanForms,formLanguages,formIdentity,claimPathFor,resolveClaimReading} from './human-forms.mjs';

const present=value=>typeof value==='string'&&Boolean(value.trim());
const FORM_ROLES_WITH_EXACT=selection=>FORM_ROLES.filter(role=>selection.roles[role]?.state==='over-budget'
  &&selection.roles[role]?.reason==='inspect-exact-form'&&selection.roles[role]?.form);
export const readingKey=(kind,id)=>JSON.stringify([kind,id]);
export function formLabel(key){
  return uiComputed(()=>{
    const labels={ru:t("Русский"),en:'English',es:'Español',auto:t('Автоматически'),original:t("Исходная форма"),default:t("Форма по умолчанию")};
    return Object.hasOwn(labels,key)?labels[key]:String(key??'');
  });
}
export function formLanguageNote(form){
  return uiComputed(()=>(form.fallback?t('Выбранная форма отсутствует. Показана: '):t('Показана: '))
    +formLabel(form.lang||form.key)+(form.lang?'.':t('. Язык в этой форме не указан.')));
}
function readingTitle(raw,preferred='ru'){
  const form=displayTitleForm(raw,preferred,true);
  // Only the missing-title placeholder follows UI language. Supplied wording
  // and the item's selected content language remain unchanged.
  return form?.unavailable?{...form,text:uiComputed(()=>displayTitleForm(raw,preferred).text)}:form;
}
export function readingLanguages(snapshot){
  if(snapshot.raw.human_form_selection)return [...new Set(['ru','en','es',...formLanguages(snapshot.raw)])];
  const display=snapshot.raw.display,fields=['title','label','kind_label','statement','summary','explanation'];
  return [...new Set(fields.flatMap(field=>Object.keys(display[field]||{})))].filter(key=>
    (languageKey(key)||['original','default'].includes(key))&&fields.some(field=>present(display[field]?.[key])));
}
function readerRecord(raw){
  // This is a bounded browser reading copy, not a lossless corpus export.
  // Opaque IDs are never merged by title or entity_id.
  const fields=['id','entity_id','kind_id','from_id','to_id','predicate_id','source_graph','native_id','source_dossier_ref','content_revision','display','epistemic','semantics','source_refs','human_form_selection','display_selection'];
  return Object.fromEntries(fields.filter(key=>raw[key]!==undefined).map(key=>[key,structuredClone(raw[key])]));
}
export function readingSnapshot({packet,match,endpoints:materialEndpoints,path},kind){
  checkRevision(packet);
  if(!['node','relation'].includes(kind)||!present(match?.id)||!match.display
    ||!/^[a-f0-9]{64}$/.test(match.content_revision||'')||!Array.isArray(match.source_refs)||!match.source_refs.length)
    throw new ContractError(t("Материал не содержит точной версии и источника."));
  const selection=validateHumanForms(match);
  const exactForms=inspectExactHumanForms(match);
  if(selection&&FORM_ROLES_WITH_EXACT(selection).some(role=>!exactForms?.[role]))throw new FormContractError();
  const endpoints=kind==='relation'?(materialEndpoints||packet.endpoints||[]):[];
  if(kind==='relation'&&![match.from_id,match.to_id].every(id=>endpoints.some(node=>node.id===id)))
    throw new ContractError(t("Для чтения связи нужны оба её участника."));
  const selectedEndpoints=kind==='relation'?[...new Set([match.from_id,match.to_id])].map(id=>endpoints.find(node=>node.id===id)):[];
  const snapshot={kind,sourceRevision:packet.source_revision,raw:readerRecord(match),endpoints:selectedEndpoints.map(readerRecord),essentialContext:essentialContext(match),
    ...(exactForms?{exactForms}: {})};
  if(path){
    if(kind!=='node'||path.claim_node_id!==match.id)throw new FormContractError();
    snapshot.claimReference=claimMaterialReference(packet,path);
    snapshot.claimReading=resolveClaimReading(packet,path.reading);
    snapshot.claimNodes=structuredClone(packet.nodes);
  }else if(kind==='node'&&match.semantics?.claim?.predicate_mapping_status==='mapped'){
    snapshot.claimContextUnavailable=true;
  }
  if(JSON.stringify(snapshot).length>500000)throw new ContractError(t("Материал слишком велик для закреплённой карточки. Откройте его источник."));
  return snapshot;
}
export function readingPositionKey(key,snapshot,preferred){
  const identity=formIdentity(snapshot.raw),parts=[key,snapshot.sourceRevision,snapshot.raw.content_revision,preferred];
  if(identity)parts.push(identity);return JSON.stringify(parts);
}
export function readingDocument(snapshot,language='ru'){
  const {raw,kind}=snapshot,display=raw.display;
  const state=kind==='node'?display.summary_state:display.explanation_state;
  const blocks=[];
  const humanForms=validateHumanForms(raw);
  if(!humanForms){
    if(kind==='relation')blocks.push({id:'statement',title:ui("Формулировка связи"),form:materialDisplayForm(raw,'statement',language)});
    blocks.push({id:'description',title:kind==='node'?ui("Описание"):ui("Пояснение"),state,
      form:state==='missing'?null:materialDisplayForm(raw,kind==='node'?'summary':'explanation',language)});
  }
  return {title:readingTitle(raw,language),claimContextUnavailable:Boolean(snapshot.claimContextUnavailable),
    kind:kind==='node'?materialDisplayForm(raw,'kind_label',language):null,blocks,humanForms,
    essentialContext:snapshot.essentialContext||essentialContext(raw),
    participants:kind==='relation'?[[ui("От"),raw.from_id],[ui("К"),raw.to_id]].map(([role,id])=>({id,role,
      form:readingTitle(snapshot.endpoints.find(node=>node.id===id),language)})):[],
    sourceRefs:[...raw.source_refs],posture:raw.epistemic||{}};
}

function readingFailure(entry){
  if(!entry.error)return null;
  switch(entry.failure){
    case 'revision':return t("Версия материала изменилась. Обновите материал.");
    case 'contract':return t("Не удалось подготовить материал для чтения. Обновите материал.");
    case 'restricted':return t("Доступ к материалу ограничен.");
    case 'unavailable':return t("Материал пока недоступен.");
    case 'load':return t("Загрузка не удалась. Повторите попытку.");
    default:return null;
  }
}
export function readingStatus(entry,sceneRevision){
  // Local status follows the interface language. An error supplied by the
  // client stays verbatim, even if it happens to match a translated UI label.
  const mismatch=Boolean(entry.snapshot&&sceneRevision&&entry.sourceRevision!==sceneRevision);
  return uiComputed(()=>[entry.loading?t("Обновляю материал…"):null,readingFailure(entry),entry.error,
    entry.changed?t("Данные изменились: чтение начато с начала актуального материала."):null,
    mismatch?t("Материал и сцена относятся к разным снимкам."):null,
    entry.snapshot&&entry.error?t("Показан ранее закреплённый материал."):null].filter(Boolean).join(' '));
}

// Source copies and graph bookmarks stay in page memory. Durable reading uses
// exact references only and always reinspects the current backend on restore.
export function createReadingShelf({client,onChange=()=>{}}){
  const items=new Map(),requests=new RequestSlots();let sceneRevision=null;
  const update=(key,patch)=>{if(items.has(key)){items.set(key,{...items.get(key),...patch});onChange();}};
  async function load(key,latest=false){
    const entry=items.get(key);if(!entry)return;
    update(key,{loading:true,error:null,failure:null,retryable:false,retryLatest:latest});
    try{
      const language=entry.preferred==='default'?'auto':entry.preferred||'ru';
      const response=await requests.run(key,signal=>entry.claimReference
        ?client.readClaimReference(entry.claimReference,signal,{language,expected:latest?null:entry.sourceRevision,versions:latest?null:entry.claimVersions})
        :client.readMaterial(entry.kind,entry.id,signal,latest?null:entry.sourceRevision,latest?undefined:entry.contentRevision,{language}));
      if(!response.current||!items.has(key))return;
      const snapshot=readingSnapshot(response.value,entry.kind);
      if(snapshot.raw.id!==entry.id)throw new ContractError(t("Сервер вернул другой предмет для чтения."));
      if(entry.claimReference&&!snapshot.claimReference)throw new FormContractError();
      const bookmark=entry.bookmark?.graph?.packet?.source_revision===snapshot.sourceRevision?entry.bookmark:null;
      const changed=entry.sourceRevision!==snapshot.sourceRevision||entry.contentRevision!==snapshot.raw.content_revision;
      update(key,{snapshot,bookmark,sourceRevision:snapshot.sourceRevision,contentRevision:snapshot.raw.content_revision,loading:false,error:null,
        ...(snapshot.claimReference?{claimReference:snapshot.claimReference,claimVersions:materialVersions(response.value.packet)}:{}),
        changed:changed||entry.changed});
    }catch(error){
      const unavailable=error instanceof FormContractError||error instanceof ContractError||error instanceof RevisionError||error instanceof RequestError&&[403,404,410].includes(error.status);
      const failure=error instanceof RevisionError?'revision':error instanceof FormContractError||error instanceof ContractError?'contract'
        :error instanceof RequestError&&error.status===403?'restricted':error instanceof RequestError&&[404,410].includes(error.status)?'unavailable':'load';
      update(key,{loading:false,error:error.message||uiComputed(()=>t("Материал не удалось загрузить.")),failure,retryable:!unavailable,
        ...(unavailable?{snapshot:null,bookmark:null}:{} )});
    }
  }
  return {
    get entries(){return [...items.values()];},
    get sceneRevision(){return sceneRevision;},
    observeRevision(revision){if(sceneRevision!==revision){sceneRevision=revision;onChange();}},
    pin({raw,kind,sourceRevision,bookmark,preferred}){
      const key=readingKey(kind,raw.id),existing=items.get(key);
      if(!existing&&items.size>=2)throw new Error(t("Уже закреплены два материала. Уберите один из них, чтобы добавить другой."));
      if(!['node','relation'].includes(kind)||!/^[a-f0-9]{64}$/.test(sourceRevision||''))
        throw new ContractError(t("Сначала дождитесь загрузки выбранного материала."));
      const scene=bookmark?.graph?.packet,path=kind==='node'?claimPathFor(scene,raw.id):null;
      if(path&&(scene.source_revision!==sourceRevision||path.reading?.content_revision!==raw.content_revision))throw new RevisionError();
      const reference=path?claimMaterialReference(scene,path):null;
      if(existing&&(!reference||existing.snapshot&&existing.sourceRevision===sourceRevision
        &&existing.contentRevision===raw.content_revision&&JSON.stringify(existing.claimReference)===JSON.stringify(reference)))return {key,existing:true};
      items.set(key,{key,kind,id:raw.id,title:readingTitle(raw,preferred),
        ...(reference?{claimReference:reference,claimVersions:materialVersions(scene)}:{}),
        changed:Boolean(existing&&(existing.changed||existing.sourceRevision!==sourceRevision||existing.contentRevision!==raw.content_revision)),
        sourceRevision,contentRevision:raw.content_revision,preferred:existing?.preferred||preferred||raw.human_form_selection?.requested_language||'ru',bookmark,snapshot:null,loading:true,error:null});
      onChange();void load(key);return {key,existing:Boolean(existing)};
    },
    refresh:key=>load(key,true),
    // A transient retry preserves the failed request's exact/current intent;
    // obtaining a newer version still belongs to the explicit refresh action.
    retry(key){const entry=items.get(key);return entry?.retryable?load(key,entry.retryLatest):Promise.resolve();},
    language(key,preferred){update(key,{preferred});return load(key);},
    restore(references){
      requests.cancelAll();items.clear();
      for(const entry of references){const key=readingKey(entry.kind,entry.id);items.set(key,{key,kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,
        ...(entry.claimReference?{claimReference:entry.claimReference}:{}),
        contentRevision:entry.contentRevision,preferred:entry.preferred||'ru',title:null,bookmark:null,snapshot:null,loading:true,error:null,changed:false});}
      onChange();for(const key of items.keys())void load(key,true);
    },
    remove(key){requests.cancel(key);items.delete(key);onChange();},
    suspend(){requests.cancelAll();for(const [key,entry]of items)if(entry.loading)items.set(key,{...entry,loading:false,retryable:true,error:uiComputed(()=>t("Загрузка прервана. Обновите материал, чтобы продолжить."))});},
    dispose(){requests.cancelAll();items.clear();},
  };
}
