import {ui,uiAttribute,uiChildren,uiText} from './ui-i18n.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {RequestSlots,RevisionError,ContractError,localized,focusSpec,relationSpec,DEFAULT_FOCUS} from './knowledge-client.mjs';
import {decodeDraft,constructorCatalog,previewDraft} from './lens-model.mjs';
import {formIdentity,formLanguages,validateHumanForms,claimPathFor,resolveClaimReading} from './human-forms.mjs';
import {renderHumanForms,renderClaimContext} from './human-forms-view.mjs';
import {formLabel} from './reader-model.mjs';

export async function readInspectorMaterial({client,scene,kind,raw,language,signal}){
  const path=kind==='node'?claimPathFor(scene,raw.id):null;
  if(path){
    if(path.reading?.content_revision!==raw.content_revision)throw new RevisionError();
    const found=await client.readClaimMaterial(scene,path,signal,{language});
    return {...found,claimReading:resolveClaimReading(found.packet,found.path.reading)};
  }
  return client.readMaterial(kind,raw.id,signal,scene.source_revision,raw.content_revision,{language,relation:kind==='relation'?raw:null});
}

export function attachKnowledgeUI(root,port,{client,initialFocus=DEFAULT_FOCUS,initialLens}={}) {
  const q=s=>root.querySelector(s),slots=new RequestSlots();
  let searchTimer=0,retryAction=null,searchOffset=0;
  let cardLanguage='ru';
  const forms=document.createElement('div');forms.className='sc-card-forms';q('.sc-description').after(forms);
  const language=document.createElement('select'),languageLabel=document.createElement('label');
  uiText(languageLabel,ui('Язык материала'));uiAttribute(language,'aria-label',ui('Язык материала'));languageLabel.append(language);forms.before(languageLabel);
  language.addEventListener('change',()=>{
    cardLanguage=language.value;root.dataset.materialLanguage=cardLanguage;
    const selected=port.selection,kind=selected.relationId?'relation':'node',raw=kind==='relation'?port.relation(selected.relationId):port.node(selected.nodeId);
    if(raw)void showCard(kind,raw);
  });
  const cardReading=createReadingMemory(q('#so-about')),relationsReading=createReadingMemory(q('#so-relations'));
  const text=(tag,className,value)=>{const el=document.createElement(tag);el.className=className;uiText(el, value);return el;};
  function button(label,action,className='sc-neighbor'){
    const b=text('button',className,label);b.type='button';b.addEventListener('click',action);return b;
  }
  function notice(message,action=null){
    q('.sc-data-notice').hidden=!message;uiText(q('.sc-data-notice span'), message);
    q('.sc-retry').hidden=!action;retryAction=action;
  }
  q('.sc-retry').addEventListener('click',()=>retryAction?.());
  function cancelInspector(){slots.cancel('inspect');}
  function cancelSearch(){clearTimeout(searchTimer);slots.cancel('search');}
  function cancelPending(){clearTimeout(searchTimer);slots.cancelAll();notice('');}
  function willSelect(){slots.cancel('scene');cancelInspector();notice('');if(port.packet)root.dataset.dataState='ready';}
  function notifyFailure(error,retry){notice(error.message||ui("Связь с данными прервалась."),retry);port.announce(error.message);}
  async function loadFocus(id,{expected=null,initial=false,depth=1,selectFocus=true}={}){
    const spec=focusSpec(id,{depth});notice(ui("Получаю окрестность…"));root.dataset.dataState='loading';
    try{
      const result=await slots.run('scene',signal=>client.compile(spec,signal,expected));if(!result.current)return;
      port.setGraph(result.value,{initial,selectFocus});root.dataset.dataState='ready';notice('');
      if(selectFocus)q('#so-about-tab').focus();
      port.announce(ui("Область загружена. Узлов: {0}. Связей: {1}.", [result.value.nodes.length, result.value.relations.length]));
    }catch(error){root.dataset.dataState='error';notifyFailure(error,()=>loadFocus(id,{initial,depth,selectFocus}));}
  }
  function chooseNode(id,expected){
    if(expected===port.packet?.source_revision&&port.node(id)){port.selectNode(id);q('#so-about-tab').focus();return;}
    loadFocus(id,{expected,selectFocus:true});
  }
  async function chooseRelation(raw,expected){
    if(expected===port.packet?.source_revision&&port.relation(raw.id)){port.selectRelation(raw.id);return;}
    notice(ui("Открываю отношение…"));root.dataset.dataState='loading';
    try{
      const result=await slots.run('scene',async signal=>{
        const {match}=await client.inspect('relation',raw.id,signal,expected,raw.content_revision);
        const spec=relationSpec(match),packet=await client.compile(spec,signal,expected);
        if(!packet.relations.some(r=>r.id===match.id))throw new ContractError(ui("Выбранное отношение отсутствует в области."));
        return {packet};
      });
      if(!result.current)return;
      port.setGraph(result.value.packet);port.selectRelation(raw.id,{rememberView:false});
      root.dataset.dataState='ready';notice('');
    }catch(error){root.dataset.dataState='error';notifyFailure(error,()=>chooseRelation(raw,null));}
  }
  function searchRow(raw,kind,revision){
    const title=localized(kind==='node'?raw.display.title:raw.display.label,raw.id);
    const row=button('',()=>kind==='node'?chooseNode(raw.id,revision):chooseRelation(raw,revision),'sc-result');
    const label=text('span','sc-result-label',title);
    if(kind==='relation'){
      const statement=localized(raw.display.statement,raw.from_id+' → '+raw.to_id);
      uiChildren(label, "append", text('span','sc-result-detail',statement));uiAttribute(row, 'aria-label', title+' · '+statement);
    }
    uiChildren(row, "append", label, text('small','',kind==='node'?localized(raw.display.kind_label):ui("Отношение")));
    return row;
  }
  function search(value,offset=0){
    clearTimeout(searchTimer);slots.cancel('search');searchOffset=offset;
    const query=value.trim().slice(0,256),results=q('.sc-search-results');uiChildren(results, "replaceChildren");
    if(!query){
      uiChildren(results, "append", text('div','sc-section-label',ui("В ТЕКУЩЕЙ ОБЛАСТИ")));
      for(const raw of (port.packet?.nodes||[]).filter(n=>n.id===port.packet?.focus?.node_id||n.kind_id==='agent').slice(0,6))uiChildren(results, "append", searchRow(raw,'node',port.packet.source_revision));
      uiChildren(results, "append", text('div','sc-empty',ui("Введите имя, название или понятие для поиска во всём древе.")));
      port.cardChanged();return;
    }
    uiChildren(results, "append", text('div','sc-empty',ui("Ищу в древе…")));port.cardChanged();
    searchTimer=setTimeout(async()=>{
      try{
        const found=await slots.run('search',signal=>client.search(query,signal,offset));
        if(!found.current||q('.sc-search').hidden)return;
        const packet=found.value;uiChildren(results, "replaceChildren");root.dataset.searchQuery=query;root.dataset.searchRevision=packet.source_revision;
        for(const kind of ['node','relation']){
          const items=packet[kind==='node'?'nodes':'relations'];if(!items.length)continue;
          uiChildren(results, "append", text('div','sc-section-label',kind==='node'?ui("УЗЛЫ"):ui("ОТНОШЕНИЯ")));
          for(const raw of items)uiChildren(results, "append", searchRow(raw,kind,packet.source_revision));
        }
        if(!packet.nodes.length&&!packet.relations.length)uiChildren(results, "append", text('div','sc-empty',ui("По этому запросу ничего не найдено.")));
        const pager=document.createElement('div');pager.className='sc-search-pager';
        if(offset>0)uiChildren(pager, "append", button(ui("Ранее"),()=>search(query,Math.max(0,offset-6))));
        if(Math.max(packet.counts.matching_nodes,packet.counts.matching_relations)>offset+6)uiChildren(pager, "append", button(ui("Далее"),()=>search(query,offset+6)));
        uiChildren(results, "append", pager);port.announce(ui("Результаты поиска обновлены."));port.cardChanged();
      }catch(error){if(q('.sc-search').hidden)return;uiChildren(results, "replaceChildren", text('div','sc-empty',error.message), button(ui("Повторить поиск"),()=>search(query,searchOffset)));port.cardChanged();}
    },180);
  }
  function sourceDetails(raw,kind){
    const out=q('.sc-provenance');uiChildren(out, "replaceChildren");
    const state=kind==='node'?raw.display.summary_state:raw.display.explanation_state;
    const labels={authored:ui("Авторское описание"),'source-derived':ui("Описание из источника"),'metadata-synthesis':ui("Описание составлено из метаданных"),missing:ui("Описание пока не зафиксировано")};
    uiChildren(out, "append", text('p','sc-description-origin',labels[state]||ui("Происхождение описания не указано")));
    const summary=kind==='node'?raw.display.summary:raw.display.explanation;
    if(!summary?.ru&&summary?.default)uiChildren(out, "append", text('p','sc-description-origin',ui("Показан исходный язык описания.")));
    const details=document.createElement('details');details.className='sc-source-details';
    uiChildren(details, "append", text('summary','',ui("Источники и статус · {0}", [raw.source_refs.length])));
    const posture=raw.epistemic||{};
    for(const [label,value]of [[ui("Слой"),posture.authority_layer],[ui("Рассмотрение"),posture.review_posture],[ui("Канон"),posture.canon_status]]){
      uiChildren(details, "append", text('p','sc-source-status',label+': '+(!value||value==='not-recorded'?ui("не указан"):value)));
    }
    for(const ref of raw.source_refs){
      if(/^https?:\/\//i.test(ref)){
        try{const url=new URL(ref);const a=text('a','sc-source-ref',url.hostname+url.pathname);a.href=url.href;a.target='_blank';a.rel='noreferrer noopener';uiChildren(details, "append", a);}catch{uiChildren(details, "append", text('span','sc-source-ref',ref));}
      }else uiChildren(details, "append", text('span','sc-source-ref',ref));
    }
    details.addEventListener('toggle',()=>port.cardChanged());uiChildren(out, "append", details);
  }
  function endpointName(id){return localized(port.node(id)?.display?.title,id);}
  function renderCard(kind,raw,endpoints=[]){
    const selection=validateHumanForms(raw),identity=formIdentity(raw);
    const readingKey=JSON.stringify([port.packet?.source_revision,kind,raw.id,raw.content_revision,cardLanguage,identity]);cardReading.capture();relationsReading.capture();cardReading.enter(readingKey);relationsReading.enter(readingKey);
    uiText(q('.sc-node-title'), localized(kind==='node'?raw.display.title:raw.display.label,raw.id));
    uiText(q('.sc-node-original'), kind==='node'?(raw.display.title.original||raw.display.title.en||''):localized(raw.display.statement));
    uiText(q('.sc-kind'), kind==='node'?localized(raw.display.kind_label,raw.kind_id).toUpperCase():ui("ОТНОШЕНИЕ"));
    uiText(q('.sc-description'), localized(kind==='node'?raw.display.summary:raw.display.explanation,ui("Описание пока не зафиксировано.")));
    q('.sc-description').hidden=Boolean(selection);forms.replaceChildren(renderHumanForms(raw));
    const languages=[...new Set(['ru','en','es',...formLanguages(raw),cardLanguage])];
    language.replaceChildren(...languages.map(value=>{const option=document.createElement('option');option.value=value;option.textContent=formLabel(value);return option;}));language.value=cardLanguage;
    uiAttribute(q('.sc-inspector'), 'aria-label', kind==='node'?ui("Выбранный узел"):ui("Выбранное отношение"));
    root.dataset.inspectorKind=kind;root.dataset.inspectorId=raw.id;
    const relationships=kind==='node'?port.neighbors(raw.id):[raw];
    uiText(q('#so-relations-tab').firstChild, kind==='node'?ui("Связи "):ui("Участники "));
    uiText(q('.sc-neighbor-count'), String(kind==='node'?relationships.length:new Set([raw.from_id,raw.to_id]).size));
    const list=q('.sc-neighbors');uiChildren(list, "replaceChildren");
    if(kind==='node'){
      for(const relation of relationships){
        const outgoing=relation.from_id===raw.id,other=outgoing?relation.to_id:relation.from_id;
        const label=outgoing?localized(relation.display.label):localized(relation.display.inverse_label)||'← '+localized(relation.display.label);
        const b=button('',()=>port.selectRelation(relation.id),'sc-neighbor sc-relation-row');uiAttribute(b, "data-tooltip", localized(relation.display.statement,ui("Открыть связь и её основания.")).slice(0,260));
        uiChildren(b, "append", text('small','',label), text('span','',endpointName(other)));uiChildren(list, "append", b);
      }
      if(!relationships.length)uiChildren(list, "append", text('p','sc-empty',ui("В этой области связи не показаны.")));
    }else{
      for(const [label,id]of [[ui("От"),raw.from_id],[ui("К"),raw.to_id]]){
        const node=endpoints.find(n=>n.id===id)||port.node(id);
        const b=button(label+': '+localized(node?.display.title,id),()=>chooseNode(id,port.packet.source_revision),'sc-neighbor sc-relation-row');uiChildren(list, "append", b);
      }
    }
    sourceDetails(raw,kind);
    uiChildren(q('.sc-provenance'), "append", button(ui("Основания и прочтения"),()=>root.dispatchEvent(new CustomEvent('sophia-evidence',{detail:{raw,kind}}))), button(ui("Открыть источники"),()=>root.dispatchEvent(new CustomEvent('sophia-sources',{detail:{raw,kind}}))), button(kind==='node'?ui("Проложить маршрут"):ui("Другой путь"),()=>root.dispatchEvent(new CustomEvent('sophia-navigate',{detail:{raw,kind,tab:'paths'}}))));cardReading.restore();relationsReading.restore();port.cardChanged();
  }
  async function showCard(kind,raw){
    cancelInspector();const scene=port.packet,language=cardLanguage;if(!scene?.source_revision)return;
    root.dataset.inspectorState='loading';
    uiText(q('.sc-node-title'),localized(kind==='node'?raw.display.title:raw.display.label,raw.id));
    uiText(q('.sc-node-original'),'');q('.sc-provenance').replaceChildren();q('.sc-neighbors').replaceChildren();
    try{
      renderCard(kind,raw);
      forms.replaceChildren(text('p','sc-form-status',ui('Обновляю формы…')));q('.sc-description').hidden=true;
      const found=await slots.run('inspect',signal=>readInspectorMaterial({client,scene,kind,raw,language,signal}));
      if(!found.current||port.packet!==scene||cardLanguage!==language
        ||(kind==='relation'?port.selection.relationId:port.selection.nodeId)!==raw.id)return;
      renderCard(kind,found.value.match,found.value.endpoints);
      if(found.value.claimReading)forms.append(renderClaimContext(found.value.claimReading));
      root.dataset.inspectorState='ready';
    }catch(error){
      root.dataset.inspectorState='error';
      forms.replaceChildren(text('p','sc-form-status',error.message));q('.sc-description').hidden=true;
      uiChildren(q('.sc-provenance'), "prepend", text('p','sc-inspection-error',error.message));
      if(error instanceof RevisionError)notifyFailure(error,()=>loadFocus(port.packet.focus?.node_id||raw.id,{selectFocus:false}));
      else uiChildren(q('.sc-provenance'), "prepend", button(ui("Загрузить карточку ещё раз"),()=>showCard(kind,raw)));
      port.cardChanged();
    }
  }
  q('.sc-open-neighborhood').addEventListener('click',()=>{
    const raw=port.node(port.selection.nodeId);if(raw)root.dispatchEvent(new CustomEvent('sophia-navigate',{detail:{raw,kind:'node',tab:'neighbors'}}));
  });
  q('.sc-read-selected').addEventListener('click',()=>{
    const selection=port.selection,kind=selection.relationId?'relation':'node',raw=kind==='relation'?port.relation(selection.relationId):port.node(selection.nodeId);
    if(raw)root.dispatchEvent(new CustomEvent('sophia-read',{detail:{raw,kind,preferred:cardLanguage}}));
  });
  async function loadLensLink(){
    root.dataset.dataState='loading';notice(ui("Открываю линзу…"));
    try{const result=await slots.run('scene',async signal=>{const draft=decodeDraft(initialLens),context=await constructorCatalog(client,signal);return previewDraft(client,draft,context,signal);});
      if(!result.current)return;port.setGraph(result.value,{initial:true});notice('');port.announce(ui("Линза загружена. Узлов: {0}. Связей: {1}.", [result.value.nodes.length, result.value.relations.length]));
    }catch(error){root.dataset.dataState='error';notifyFailure(error,loadLensLink);}
  }
  addEventListener('pagehide',cancelPending);
  return {captureReading:()=>{cardReading.capture();relationsReading.capture();},restoreReading:()=>{cardReading.restore();relationsReading.restore();},loadFocus,chooseNode,chooseRelation,searchRow,search,showCard,cancelSearch,cancelInspector,cancelPending,willSelect,
    async start({skipScene=false}={}){
      if(!skipScene){if(initialLens)void loadLensLink();else loadFocus(initialFocus,{initial:true,selectFocus:false});}
      try{const result=await slots.run('capabilities',signal=>client.capabilities(signal));if(result.current)root.dataset.explorationAvailable=String(result.value.available===true);}catch{root.dataset.explorationAvailable='false';}
    },
  };
}
