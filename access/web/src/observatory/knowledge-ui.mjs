import {KnowledgeClient,RequestSlots,RevisionError,ContractError,localized,focusSpec,relationSpec,DEFAULT_FOCUS} from './knowledge-client.mjs';
import {decodeDraft,constructorCatalog,previewDraft} from './lens-model.mjs';

export function attachKnowledgeUI(root,port,{client=new KnowledgeClient(),initialFocus=DEFAULT_FOCUS,initialLens}={}) {
  const q=s=>root.querySelector(s),slots=new RequestSlots(),cache=new Map();
  let searchTimer=0,retryAction=null,searchOffset=0;
  const text=(tag,className,value)=>{const el=document.createElement(tag);el.className=className;el.textContent=value;return el;};
  function button(label,action,className='sc-neighbor'){
    const b=text('button',className,label);b.type='button';b.addEventListener('click',action);return b;
  }
  function notice(message,action=null){
    q('.sc-data-notice').hidden=!message;q('.sc-data-notice span').textContent=message;
    q('.sc-retry').hidden=!action;retryAction=action;
  }
  q('.sc-retry').addEventListener('click',()=>retryAction?.());
  function cancelInspector(){slots.cancel('inspect');}
  function cancelSearch(){clearTimeout(searchTimer);slots.cancel('search');}
  function cancelPending(){clearTimeout(searchTimer);slots.cancelAll();notice('');}
  function willSelect(){slots.cancel('scene');cancelInspector();notice('');if(port.packet)root.dataset.dataState='ready';}
  function notifyFailure(error,retry){notice(error.message||'Связь с данными прервалась.',retry);port.announce(error.message);}
  function saveCache(key,value){cache.delete(key);cache.set(key,value);if(cache.size>48)cache.delete(cache.keys().next().value);}
  async function loadFocus(id,{expected=null,initial=false,depth=1,selectFocus=true}={}){
    const spec=focusSpec(id,{depth});notice('Получаю окрестность…');root.dataset.dataState='loading';
    try{
      const result=await slots.run('scene',signal=>client.compile(spec,signal,expected));if(!result.current)return;
      port.setGraph(result.value,{initial,selectFocus});root.dataset.dataState='ready';notice('');
      if(selectFocus)q('#so-about-tab').focus();
      port.announce('Область загружена. Узлов: '+result.value.nodes.length+'. Связей: '+result.value.relations.length+'.');
    }catch(error){root.dataset.dataState='error';notifyFailure(error,()=>loadFocus(id,{initial,depth,selectFocus}));}
  }
  function chooseNode(id,expected){
    if(expected===port.packet?.source_revision&&port.node(id)){port.selectNode(id);q('#so-about-tab').focus();return;}
    loadFocus(id,{expected,selectFocus:true});
  }
  async function chooseRelation(raw,expected){
    if(expected===port.packet?.source_revision&&port.relation(raw.id)){port.selectRelation(raw.id);return;}
    notice('Открываю отношение…');root.dataset.dataState='loading';
    try{
      const result=await slots.run('scene',async signal=>{
        const {match}=await client.inspect('relation',raw.id,signal,expected,raw.content_revision);
        const spec=relationSpec(match),packet=await client.compile(spec,signal,expected);
        if(!packet.relations.some(r=>r.id===match.id))throw new ContractError('Выбранное отношение отсутствует в области.');
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
      label.append(text('span','sc-result-detail',statement));row.setAttribute('aria-label',title+' · '+statement);
    }
    row.append(label,text('small','',kind==='node'?localized(raw.display.kind_label):'Отношение'));
    return row;
  }
  function search(value,offset=0){
    clearTimeout(searchTimer);slots.cancel('search');searchOffset=offset;
    const query=value.trim().slice(0,256),results=q('.sc-search-results');results.replaceChildren();
    if(!query){
      results.append(text('div','sc-section-label','В ТЕКУЩЕЙ ОБЛАСТИ'));
      for(const raw of (port.packet?.nodes||[]).filter(n=>n.id===port.packet?.focus?.node_id||n.kind_id==='agent').slice(0,6))results.append(searchRow(raw,'node',port.packet.source_revision));
      results.append(text('div','sc-empty','Введите имя, название или понятие для поиска во всём древе.'));
      port.cardChanged();return;
    }
    results.append(text('div','sc-empty','Ищу в древе…'));port.cardChanged();
    searchTimer=setTimeout(async()=>{
      try{
        const found=await slots.run('search',signal=>client.search(query,signal,offset));
        if(!found.current||q('.sc-search').hidden)return;
        const packet=found.value;results.replaceChildren();root.dataset.searchQuery=query;root.dataset.searchRevision=packet.source_revision;
        for(const kind of ['node','relation']){
          const items=packet[kind==='node'?'nodes':'relations'];if(!items.length)continue;
          results.append(text('div','sc-section-label',kind==='node'?'УЗЛЫ':'ОТНОШЕНИЯ'));
          for(const raw of items)results.append(searchRow(raw,kind,packet.source_revision));
        }
        if(!packet.nodes.length&&!packet.relations.length)results.append(text('div','sc-empty','По этому запросу ничего не найдено.'));
        const pager=document.createElement('div');pager.className='sc-search-pager';
        if(offset>0)pager.append(button('Ранее',()=>search(query,Math.max(0,offset-6))));
        if(Math.max(packet.counts.matching_nodes,packet.counts.matching_relations)>offset+6)pager.append(button('Далее',()=>search(query,offset+6)));
        results.append(pager);port.announce('Результаты поиска обновлены.');port.cardChanged();
      }catch(error){if(q('.sc-search').hidden)return;results.replaceChildren(text('div','sc-empty',error.message),button('Повторить поиск',()=>search(query,searchOffset)));port.cardChanged();}
    },180);
  }
  function sourceDetails(raw,kind){
    const out=q('.sc-provenance');out.replaceChildren();
    const state=kind==='node'?raw.display.summary_state:raw.display.explanation_state;
    const labels={authored:'Авторское описание','source-derived':'Описание из источника','metadata-synthesis':'Описание составлено из метаданных',missing:'Описание пока не зафиксировано'};
    out.append(text('p','sc-description-origin',labels[state]||'Происхождение описания не указано'));
    const summary=kind==='node'?raw.display.summary:raw.display.explanation;
    if(!summary?.ru&&summary?.default)out.append(text('p','sc-description-origin','Показан исходный язык описания.'));
    const details=document.createElement('details');details.className='sc-source-details';
    details.append(text('summary','',`Источники и статус · ${raw.source_refs.length}`));
    const posture=raw.epistemic||{};
    for(const [label,value]of [['Слой',posture.authority_layer],['Рассмотрение',posture.review_posture],['Канон',posture.canon_status]]){
      details.append(text('p','sc-source-status',label+': '+(!value||value==='not-recorded'?'не указан':value)));
    }
    for(const ref of raw.source_refs){
      if(/^https?:\/\//i.test(ref)){
        try{const url=new URL(ref);const a=text('a','sc-source-ref',url.hostname+url.pathname);a.href=url.href;a.target='_blank';a.rel='noreferrer noopener';details.append(a);}catch{details.append(text('span','sc-source-ref',ref));}
      }else details.append(text('span','sc-source-ref',ref));
    }
    details.addEventListener('toggle',()=>port.cardChanged());out.append(details);
  }
  function endpointName(id){return localized(port.node(id)?.display?.title,id);}
  function renderCard(kind,raw,endpoints=[]){
    q('.sc-node-title').textContent=localized(kind==='node'?raw.display.title:raw.display.label,raw.id);
    q('.sc-node-original').textContent=kind==='node'?(raw.display.title.original||raw.display.title.en||''):localized(raw.display.statement);
    q('.sc-kind').textContent=kind==='node'?localized(raw.display.kind_label,raw.kind_id).toUpperCase():'ОТНОШЕНИЕ';
    q('.sc-description').textContent=localized(kind==='node'?raw.display.summary:raw.display.explanation,'Описание пока не зафиксировано.');
    q('.sc-inspector').setAttribute('aria-label',kind==='node'?'Выбранный узел':'Выбранное отношение');
    root.dataset.inspectorKind=kind;root.dataset.inspectorId=raw.id;
    const relationships=kind==='node'?port.neighbors(raw.id):[raw];
    q('#so-relations-tab').firstChild.textContent=kind==='node'?'Связи ':'Участники ';
    q('.sc-neighbor-count').textContent=String(kind==='node'?relationships.length:new Set([raw.from_id,raw.to_id]).size);
    const list=q('.sc-neighbors');list.replaceChildren();
    if(kind==='node'){
      for(const relation of relationships){
        const outgoing=relation.from_id===raw.id,other=outgoing?relation.to_id:relation.from_id;
        const label=outgoing?localized(relation.display.label):localized(relation.display.inverse_label)||'← '+localized(relation.display.label);
        const b=button('',()=>port.selectRelation(relation.id),'sc-neighbor sc-relation-row');
        b.append(text('small','',label),text('span','',endpointName(other)));list.append(b);
      }
      if(!relationships.length)list.append(text('p','sc-empty','В этой области связи не показаны.'));
    }else{
      for(const [label,id]of [['От',raw.from_id],['К',raw.to_id]]){
        const node=endpoints.find(n=>n.id===id)||port.node(id);
        const b=button(label+': '+localized(node?.display.title,id),()=>chooseNode(id,port.packet.source_revision),'sc-neighbor sc-relation-row');list.append(b);
      }
    }
    sourceDetails(raw,kind);q('.sc-provenance').append(button('Основания и прочтения',()=>root.dispatchEvent(new CustomEvent('sophia-evidence',{detail:{raw,kind}}))),button('Открыть источники',()=>root.dispatchEvent(new CustomEvent('sophia-sources',{detail:{raw,kind}}))),button(kind==='node'?'Проложить маршрут':'Другой путь',()=>root.dispatchEvent(new CustomEvent('sophia-navigate',{detail:{raw,kind,tab:'paths'}}))));port.cardChanged();
  }
  async function showCard(kind,raw){
    cancelInspector();const revision=port.packet?.source_revision;if(!revision)return;
    renderCard(kind,raw);root.dataset.inspectorState='loading';
    const key=[revision,kind,raw.id,raw.content_revision].join('|');
    if(cache.has(key)){const saved=cache.get(key);renderCard(kind,saved.match,saved.packet.endpoints);root.dataset.inspectorState='ready';return;}
    try{
      const found=await slots.run('inspect',signal=>client.inspect(kind,raw.id,signal,revision,raw.content_revision));if(!found.current)return;
      saveCache(key,found.value);renderCard(kind,found.value.match,found.value.packet.endpoints);root.dataset.inspectorState='ready';
    }catch(error){
      root.dataset.inspectorState='error';
      q('.sc-provenance').prepend(text('p','sc-inspection-error',error.message));
      if(error instanceof RevisionError)notifyFailure(error,()=>loadFocus(port.packet.focus?.node_id||raw.id,{selectFocus:false}));
      else q('.sc-provenance').prepend(button('Загрузить карточку ещё раз',()=>showCard(kind,raw)));
      port.cardChanged();
    }
  }
  q('.sc-open-neighborhood').addEventListener('click',()=>{
    const raw=port.node(port.selection.nodeId);if(raw)root.dispatchEvent(new CustomEvent('sophia-navigate',{detail:{raw,kind:'node',tab:'neighbors'}}));
  });
  async function loadLensLink(){
    root.dataset.dataState='loading';notice('Открываю линзу…');
    try{const result=await slots.run('scene',async signal=>{const draft=decodeDraft(initialLens),context=await constructorCatalog(client,signal);return previewDraft(client,draft,context,signal);});
      if(!result.current)return;port.setGraph(result.value,{initial:true});notice('');port.announce('Линза загружена. Узлов: '+result.value.nodes.length+'. Связей: '+result.value.relations.length+'.');
    }catch(error){root.dataset.dataState='error';notifyFailure(error,loadLensLink);}
  }
  addEventListener('pagehide',cancelPending);
  return {loadFocus,chooseNode,chooseRelation,searchRow,search,showCard,cancelSearch,cancelInspector,cancelPending,willSelect,
    async start(){
      if(initialLens)void loadLensLink();else loadFocus(initialFocus,{initial:true,selectFocus:false});
      try{const result=await slots.run('capabilities',signal=>client.capabilities(signal));if(result.current)root.dataset.explorationAvailable=String(result.value.available===true);}catch{root.dataset.explorationAvailable='false';}
    },
  };
}
