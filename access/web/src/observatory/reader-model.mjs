import {t} from './ui-i18n.mjs';
import {ContractError,RequestSlots,RequestError,RevisionError,checkRevision,displayTitle,missingReadableTitle} from './knowledge-client.mjs';
import {FormContractError,validateHumanForms,formLanguages,formIdentity} from './human-forms.mjs';

const present=value=>typeof value==='string'&&Boolean(value.trim());
const languageKey=key=>!['default','original'].includes(key)&&/^[a-z]{2,8}(?:-[a-z0-9]{1,8})*$/i.test(key);
export const readingKey=(kind,id)=>JSON.stringify([kind,id]);
export function formLabel(key){
  return ({ru:t("Русский"),en:'English',es:'Español',auto:t('Автоматически'),original:t("Исходная форма"),default:t("Форма по умолчанию")})[key]||key;
}
// Preserve the delivered wording and the actual selected field. In particular,
// `default` and `original` are not language tags and do not imply a translation.
export function readingForm(value,preferred='ru'){
  const keys=[preferred,'default','original','ru','en',...Object.keys(value||{}).filter(languageKey)];
  const key=keys.find(key=>present(value?.[key]));
  return key?{text:value[key],key,lang:languageKey(key)?key:null,fallback:key!==preferred}:null;
}
function readingTitle(raw,preferred='ru'){
  return missingReadableTitle(raw)?{text:displayTitle(raw),key:null,lang:null,fallback:false,unavailable:true}
    :readingForm(raw.display.title||raw.display.label,preferred);
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
  const fields=['id','entity_id','kind_id','from_id','to_id','predicate_id','source_graph','native_id','content_revision','display','epistemic','semantics','source_refs','human_form_selection','display_selection'];
  return Object.fromEntries(fields.filter(key=>raw[key]!==undefined).map(key=>[key,structuredClone(raw[key])]));
}
export function readingSnapshot({packet,match,endpoints:materialEndpoints},kind){
  checkRevision(packet);
  if(!['node','relation'].includes(kind)||!present(match?.id)||!match.display
    ||!/^[a-f0-9]{64}$/.test(match.content_revision||'')||!Array.isArray(match.source_refs)||!match.source_refs.length)
    throw new ContractError(t("Материал не содержит точной версии и источника."));
  validateHumanForms(match);
  const endpoints=kind==='relation'?(materialEndpoints||packet.endpoints||[]):[];
  if(kind==='relation'&&![match.from_id,match.to_id].every(id=>endpoints.some(node=>node.id===id)))
    throw new ContractError(t("Для чтения связи нужны оба её участника."));
  const selectedEndpoints=kind==='relation'?[...new Set([match.from_id,match.to_id])].map(id=>endpoints.find(node=>node.id===id)):[];
  const snapshot={kind,sourceRevision:packet.source_revision,raw:readerRecord(match),endpoints:selectedEndpoints.map(readerRecord)};
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
    if(kind==='relation')blocks.push({id:'statement',title:t("Формулировка связи"),form:readingForm(display.statement,language)});
    blocks.push({id:'description',title:kind==='node'?t("Описание"):t("Пояснение"),state,
      form:state==='missing'?null:readingForm(kind==='node'?display.summary:display.explanation,language)});
  }
  return {title:readingTitle(raw,language),
    kind:kind==='node'?readingForm(display.kind_label,language):null,blocks,humanForms,
    participants:kind==='relation'?[[t("От"),raw.from_id],[t("К"),raw.to_id]].map(([role,id])=>({id,role,
      form:readingTitle(snapshot.endpoints.find(node=>node.id===id),language)})):[],
    sourceRefs:[...raw.source_refs],posture:raw.epistemic||{}};
}

// Source copies and graph bookmarks stay in page memory. Durable reading uses
// exact references only and always reinspects the current backend on restore.
export function createReadingShelf({client,onChange=()=>{}}){
  const items=new Map(),requests=new RequestSlots();let sceneRevision=null;
  const update=(key,patch)=>{if(items.has(key)){items.set(key,{...items.get(key),...patch});onChange();}};
  async function load(key,latest=false){
    const entry=items.get(key);if(!entry)return;
    update(key,{loading:true,error:null});
    try{
      const response=await requests.run(key,signal=>client.readMaterial(entry.kind,entry.id,signal,
        latest?null:entry.sourceRevision,latest?undefined:entry.contentRevision,{language:entry.preferred==='default'?'auto':entry.preferred||'ru'}));
      if(!response.current||!items.has(key))return;
      const snapshot=readingSnapshot(response.value,entry.kind);
      if(snapshot.raw.id!==entry.id)throw new ContractError(t("Сервер вернул другой предмет для чтения."));
      const bookmark=entry.bookmark?.graph?.packet?.source_revision===snapshot.sourceRevision?entry.bookmark:null;
      const changed=entry.sourceRevision!==snapshot.sourceRevision||entry.contentRevision!==snapshot.raw.content_revision;
      update(key,{snapshot,bookmark,sourceRevision:snapshot.sourceRevision,contentRevision:snapshot.raw.content_revision,loading:false,error:null,
        changed:changed||entry.changed});
    }catch(error){
      const unavailable=error instanceof FormContractError||error instanceof ContractError||error instanceof RevisionError||error instanceof RequestError&&[403,404,410].includes(error.status);
      update(key,{loading:false,error:error.message||t("Материал не удалось загрузить."),
        ...(unavailable?{snapshot:null,bookmark:null}:{} )});
    }
  }
  return {
    get entries(){return [...items.values()];},
    get sceneRevision(){return sceneRevision;},
    observeRevision(revision){if(sceneRevision!==revision){sceneRevision=revision;onChange();}},
    pin({raw,kind,sourceRevision,bookmark,preferred}){
      const key=readingKey(kind,raw.id);
      if(items.has(key))return {key,existing:true};
      if(items.size>=2)throw new Error(t("Уже закреплены два материала. Уберите один из них, чтобы добавить другой."));
      if(!['node','relation'].includes(kind)||!/^[a-f0-9]{64}$/.test(sourceRevision||''))
        throw new ContractError(t("Сначала дождитесь загрузки выбранного материала."));
      items.set(key,{key,kind,id:raw.id,title:readingTitle(raw,preferred),
        sourceRevision,contentRevision:raw.content_revision,preferred:preferred||raw.human_form_selection?.requested_language||'ru',bookmark,snapshot:null,loading:true,error:null});
      onChange();void load(key);return {key,existing:false};
    },
    refresh:key=>load(key,true),
    language(key,preferred){update(key,{preferred});return load(key);},
    restore(references){
      requests.cancelAll();items.clear();
      for(const entry of references){const key=readingKey(entry.kind,entry.id);items.set(key,{key,kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,
        contentRevision:entry.contentRevision,preferred:entry.preferred||'ru',title:null,bookmark:null,snapshot:null,loading:true,error:null,changed:false});}
      onChange();for(const key of items.keys())void load(key,true);
    },
    remove(key){requests.cancel(key);items.delete(key);onChange();},
    suspend(){requests.cancelAll();for(const [key,entry]of items)if(entry.loading)items.set(key,{...entry,loading:false,error:t("Загрузка прервана. Обновите материал, чтобы продолжить.")});},
    dispose(){requests.cancelAll();items.clear();},
  };
}
