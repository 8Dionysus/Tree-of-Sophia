import {createReadingMemory} from './reading-state.mjs';
import {createResearchWorkspace,createLocalStoragePersistence} from '../research-workspace';
import {localized,RequestSlots} from './knowledge-client.mjs';
import {refreshIcons} from './icons';
import {stageObservation} from './research-actions';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(label,action)=>{const b=el('button',label);b.type='button';b.addEventListener('click',action);return b;};
function link(ref){
  if(/^https?:\/\//i.test(ref)){try{const url=new URL(ref);const a=el('a',url.hostname+url.pathname,'sc-source-ref');a.href=url.href;a.target='_blank';a.rel='noreferrer noopener';return a;}catch{/* Render invalid references as text. */}}
  return el('span',ref,'sc-source-ref');
}
export function createTools(root,scene,{data:{queries},selected,panels,onChange}){
  let persistence=false;
  try{persistence=createLocalStoragePersistence(localStorage,'tos-research-workspace-v1');}catch{/* Workspace remains usable in memory. */}
  const workspace=createResearchWorkspace({sessionId:'tos-local-research',persistence});
  const requests=new RequestSlots(),gapHits=new Map();let gapSelection=null;
  const open=button('',()=>show('notes'));open.className='sc-control sc-workspace-open';open.setAttribute('aria-label','Исследование');open.setAttribute('aria-expanded','false');
  open.innerHTML='<i data-lucide="notebook-pen" aria-hidden="true"></i><span>Исследование</span>';
  root.querySelector('.sc-header-actions').append(open);
  const panel=el('section','','sc-panel sc-workspace');panel.hidden=true;panel.setAttribute('aria-label','Исследовательская панель');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-workspace-close" aria-label="Закрыть исследование"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Исследование</h3><div class="sc-workspace-tabs" role="tablist" aria-label="Инструменты исследования"></div><div class="sc-workspace-body" role="tabpanel" id="sc-tool-content"></div><div class="sc-tool-status" role="status"></div><div class="sc-workspace-footer"></div>';
  root.append(panel);
  panels.register('workspace',panel,()=>{reading.capture();requests.cancelAll();open.setAttribute('aria-expanded','false');});
  const body=panel.querySelector('.sc-workspace-body'),status=panel.querySelector('.sc-tool-status'),tabs=panel.querySelector('.sc-workspace-tabs'),footer=panel.querySelector('.sc-workspace-footer');
  const reading=createReadingMemory(body);
  let active='notes',target=null,rawSource=null,sourceKind='node',focusReturn=open,notebookDraft='',draftKind='note',draftTarget=null;
  const names={notes:'Записи',sources:'Источники',analysis:'Разбор'};
  const tabButtons=Object.entries(names).map(([id,name])=>{const b=button(name,()=>switchTab(id));b.id='sc-tool-'+id;b.setAttribute('role','tab');b.setAttribute('aria-controls','sc-tool-content');tabs.append(b);return b;});
  tabs.addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;e.preventDefault();const index=tabButtons.indexOf(e.target),next=e.key==='Home'?0:e.key==='End'?2:(index+(e.key==='ArrowLeft'?2:1))%3;switchTab(Object.keys(names)[next]);tabButtons[next].focus();});
  function close(){panels.close('workspace');(focusReturn?.isConnected&&!focusReturn.closest('[hidden]')?focusReturn:open).focus();}
  panel.querySelector('.sc-workspace-close').addEventListener('click',close);
  panel.addEventListener('keydown',e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}});
  function show(tab='notes',source){
    reading.capture();focusReturn=document.activeElement instanceof HTMLElement?document.activeElement:open;
    target=source?{id:source.raw.id,label:localized(source.raw.display.title||source.raw.display.label),kind:source.kind==='relation'?'edge':'node',source_refs:source.raw.source_refs}:selected();
    rawSource=source?.raw||(target?.kind==='edge'?scene.port.relation(target.id):scene.port.node(target?.id));sourceKind=source?.kind||(target?.kind==='edge'?'relation':'node');
    panels.open('workspace');open.setAttribute('aria-expanded','true');switchTab(tab);tabButtons[Object.keys(names).indexOf(tab)].focus();
  }
  function switchTab(tab){reading.capture();requests.cancelAll();body.setAttribute('aria-busy','false');active=tab;reading.enter(JSON.stringify([scene.port.packet?.source_revision,target?.id,tab]));panel.querySelector('h3').textContent={notes:'Исследование',sources:'Источники',analysis:'Разбор текста'}[tab];status.textContent='';for(const [i,b]of tabButtons.entries()){const isActive=Object.keys(names)[i]===tab;b.setAttribute('aria-selected',String(isActive));b.tabIndex=isActive?0:-1;}body.setAttribute('aria-labelledby','sc-tool-'+tab);body.replaceChildren();
    if(tab==='notes')renderNotes();else if(tab==='sources')void renderSources();else renderAnalysis();reading.restore();scene.invalidate();
  }
  panels.configure('workspace',{onResume:()=>{reading.restore();switchTab(active);}});
  root.addEventListener('sophia-sources',e=>show('sources',e.detail));
  function report(error){if(error?.name==='AbortError')return;status.textContent=error.message||'Не удалось выполнить действие.';scene.invalidate();}
  function safe(action){try{return action();}catch(error){report(error);}}
  function actions(...items){const row=el('div','','sc-tool-actions');row.append(...items);return row;}
  function context(){body.append(el('p',target?'Для: '+target.label:'Общие записи исследования','sc-muted'));}
  function newId(kind){return `${kind}:${crypto.randomUUID()}`;}
  function addNote(text,targetId){workspace.addNote({id:newId('note'),body:text,targetId});return {added:true,summary:workspace.summary()};}
  function hypothesis(text,selection=target){
    return workspace.addHypothesis({id:newId('hypothesis'),title:text.slice(0,100),body:text,targetId:selection?.id,
      ...(selection?.from_id&&selection?.to_id?{fromId:selection.from_id,toId:selection.to_id}:{})});
  }
  function stage(text,selection=target,input={}){
    if(!selection?.id)throw new Error('Сначала выберите звезду или отношение.');
    const refs=input.source_refs||selection.source_refs||[];if(!refs.length)throw new Error('Для предложения нужен хотя бы один источник.');
    const kind=input.kind||'interpretation',fromId=input.from_id||selection.from_id,toId=input.to_id||selection.to_id;
    if(['relation','source_route'].includes(kind)&&(!fromId||!toId))throw new Error('Для предложения о связи нужны оба участника.');
    return stageObservation(workspace,{id:newId('proposal'),kind,parentHypothesisId:newId('hypothesis'),targetId:selection.id,
      ...(fromId&&toId?{fromId,toId}:{}),statement:text,sourceRefs:refs,evidenceRefs:input.evidence_refs?.length?input.evidence_refs:refs,
      confidencePosture:{value:input.confidence||'unknown',meaning:'maker_declared_uncertainty_not_truth_probability'},
      actorOrigin:input.actor_origin==='agent'?'agent':'human',basePageRevision:input.context_revision||0,
      dataFingerprint:scene.port.packet?.source_revision||'unavailable'});
  }
  function download(){const url=URL.createObjectURL(new Blob([workspace.exportPacket()],{type:'application/json'}));const a=el('a');a.href=url;a.download='sophia-research.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
  function renderNotes(){
    const noteTarget=notebookDraft?draftTarget:target;body.append(el('p',noteTarget?'Для: '+noteTarget.label:'Общие записи исследования','sc-muted'));body.append(el('p','Записи и гипотезы сохраняются в этом браузере. Предложения остаются черновиками до рассмотрения.','sc-muted'));
    const form=el('form'),label=el('label','Новая запись');label.htmlFor='sc-note-text';const input=el('textarea');input.id='sc-note-text';input.maxLength=2000;input.placeholder='Мысль, вопрос или наблюдение…';input.value=notebookDraft;
    input.addEventListener('input',()=>{if(!notebookDraft)draftTarget=target?{...target}:null;notebookDraft=input.value;});
    const kindLabel=el('label','Тип записи');kindLabel.htmlFor='sc-note-kind';const kind=el('select');kind.id='sc-note-kind';for(const [value,text]of [['note','Заметка'],['hypothesis','Гипотеза'],['proposal','Предложение к рассмотрению']]){const opt=el('option',text);opt.value=value;kind.append(opt);}kind.value=draftKind;kind.addEventListener('change',()=>draftKind=kind.value);
    const submit=el('button','Сохранить запись');submit.type='submit';form.append(label,input,kindLabel,kind,actions(submit));
    form.addEventListener('submit',e=>{e.preventDefault();safe(()=>{const text=input.value.trim();if(!text)throw new Error('Напишите текст записи.');const bound=notebookDraft?draftTarget:target;if(kind.value==='hypothesis')hypothesis(text,bound);else if(kind.value==='proposal')stage(text,bound);else addNote(text,bound?.id);notebookDraft='';draftTarget=null;switchTab('notes');status.textContent='Запись сохранена.';});});body.append(form);
    const undo=button('Отменить',()=>{workspace.undo();switchTab('notes');}),redo=button('Повторить',()=>{workspace.redo();switchTab('notes');});undo.disabled=!workspace.canUndo();redo.disabled=!workspace.canRedo();
    const upload=el('input');upload.type='file';upload.accept='.json,application/json';upload.hidden=true;upload.setAttribute('aria-label','Импорт исследования');
    upload.addEventListener('change',async()=>{const file=upload.files?.[0];if(!file)return;if(file.size>1000000){report(new Error('Файл превышает 1 МБ.'));return;}try{const packet=await file.text();workspace.importPacket(packet);switchTab('notes');status.textContent='Исследование импортировано.';}catch(error){report(error);}});
    body.append(actions(undo,redo,button('Экспорт записей',download),button('Импорт записей',()=>upload.click()),button('Полная копия исследования',()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy')))),upload);
    const state=workspace.getState();
    for(const [type,items]of [['Заметка',state.notes],['Гипотеза',state.hypotheses],['Предложение · ожидает рассмотрения',state.proposals]])for(const item of items.slice().reverse()){
      const entry=el('article','','sc-entry');entry.append(el('small',type),el('p',item.body||item.statement));if(item.targetId)entry.append(el('small',item.targetId));
      if(type==='Заметка')entry.append(actions(button('Удалить',()=>{workspace.removeNote(item.id);switchTab('notes');})));
      body.append(entry);
    }
    if(!state.notes.length&&!state.hypotheses.length&&!state.proposals.length)body.append(el('p','Здесь появятся ваши записи.','sc-muted'));
    if(workspace.persistenceError())status.textContent='Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием.';
  }
  function sourceRecord(record){
    const entry=el('article','','sc-entry');entry.append(el('h4',record.label||record.preferred_label||record.node_id));
    const properties=record.properties||{};if(properties.description||properties.notes)entry.append(el('p',properties.description||properties.notes,'sc-source-text'));
    for(const ref of [...new Set([...(record.source_refs||[]),properties.url,properties.locator,properties.source_url].filter(v=>typeof v==='string'))])entry.append(link(ref));
    return entry;
  }
  async function renderSources(){
    context();if(!rawSource){body.append(el('p','Выберите звезду или отношение, чтобы увидеть источники.'));return;}
    const source=rawSource;body.append(el('p',localized(sourceKind==='relation'?source.display.explanation:source.display.summary,'Описание пока не зафиксировано.'),'sc-source-text'));
    const provenance=el('details');provenance.append(el('summary','Происхождение и статус'));
    for(const [label,value]of [['Слой',source.epistemic?.authority_layer],['Рассмотрение',source.epistemic?.review_posture],['Канон',source.epistemic?.canon_status]])provenance.append(el('p',label+': '+(value&&value!=='not-recorded'?value:'не указан'),'sc-muted'));
    for(const ref of source.source_refs)provenance.append(link(ref));body.append(provenance);
    if(sourceKind==='relation')return;
    const nativeId=source.native_id; // Explicit owner identity; never split or guess an opaque knowledge ID.
    if(!nativeId)return;
    const result=el('div');body.append(result);result.append(el('p','Получаю досье источников…','sc-muted'));
    body.setAttribute('aria-busy','true');
    try{
      const response=await requests.run('source',signal=>queries.invoke('tos.dossier.inspect',{object_id:nativeId,limit:40},{signal}));if(!response.current||panel.hidden||active!=='sources')return;
      result.replaceChildren();const packet=response.value;
      for(const [key,title]of [['work','Произведение'],['expression','Редакции и переводы'],['edition','Издания'],['file','Файлы'],['item','Экземпляры'],['link','Ссылки']]){
        const items=packet.chain?.[key]||[];if(!items.length)continue;const group=el('details');group.append(el('summary',title+' · '+items.length));for(const item of items)group.append(sourceRecord(item));result.append(group);
      }
      const boundary=packet.agent_summary;if(boundary)result.append(el('p','Доступность ссылки и право использования — отдельные сведения. Статус прав: '+(boundary.rights_posture&&boundary.rights_posture!=='unknown'?boundary.rights_posture:'не указан')+'.','sc-muted'));
      if(!result.children.length)result.append(el('p','Дополнительные маршруты источников пока не записаны.','sc-muted'));
      body.setAttribute('aria-busy','false');reading.restore();scene.invalidate();
    }catch(error){body.setAttribute('aria-busy','false');result.replaceChildren(el('p',error.message,'sc-muted'));result.append(actions(button('Повторить',()=>switchTab('sources'))));scene.invalidate();}
  }
  function renderAnalysis(){
    body.append(el('p','Ищите недостающие источники или подготовьте разбор слова в «Заратустре».','sc-muted'));
    const label=el('label','Запрос');label.htmlFor='sc-analysis-query';const input=el('input');input.id='sc-analysis-query';input.placeholder='Название источника или слово…';input.maxLength=256;
    const result=el('div');body.append(label,input,actions(button('Пробелы в источниках',()=>runTool('gaps',input.value,result).catch(()=>{})),button('Разобрать слово',()=>runTool('word',input.value,result).catch(()=>{}))),result);
  }
  async function runTool(kind,query,out,externalSignal,options={}){
    status.textContent='Получаю материал…';out.replaceChildren();
    try{
      const operation=kind==='gaps'?'tos.source-gaps.search':'tos.zarathustra.word-analysis.prepare';
      const input=kind==='gaps'?{query,limit:options.limit??12}:{query,language:options.language||'ru',rank:options.rank??1,include_semantic_neighbors:options.include_semantic_neighbors===true};
      const response=await requests.run('analysis',signal=>queries.invoke(operation,input,{signal:externalSignal?AbortSignal.any([signal,externalSignal]):signal}));
      if(!response.current||panel.hidden||active!=='analysis'){externalSignal?.throwIfAborted();throw new DOMException('Panel closed','AbortError');}externalSignal?.throwIfAborted();const packet=response.value;status.textContent='';
      if(kind==='gaps'){
        gapHits.clear();for(const gap of packet.gaps||[]){gapHits.set(gap.edge_id,gap);const entry=el('article','','sc-entry');entry.append(el('h4',gap.to_label||gap.label),el('p',gap.properties?.public_summary_en||gap.summary||''),el('small','Доступ: '+gap.access_status+' · Запрос: '+gap.request_status));for(const ref of gap.source_refs||[])entry.append(link(ref));entry.append(actions(button('Рассмотреть',()=>chooseGap(gap.edge_id))));out.append(entry);}
        if(!packet.gaps?.length)out.append(el('p','По этому запросу пробелов не найдено.'));
      }else if(packet.available!==true){out.append(el('p','Разбор для этого запроса сейчас недоступен.'),el('p',packet.reason||'','sc-muted'));}
      else{
        const source=packet.task?.source||{};out.append(el('h4',source.surface||source.text||query),el('p',source.context||source.excerpt||source.sentence||''));if(source.source_ref)out.append(link(source.source_ref));
        out.append(el('p','Подготовлен разбор по исходному тексту. Результат требует рассмотрения.','sc-muted'));
        // Preserve the full source-bound analysis task for the agent and local export.
        out.append(actions(button('Сохранить задание',()=>{const url=URL.createObjectURL(new Blob([JSON.stringify(packet,null,2)],{type:'application/json'}));const a=el('a');a.href=url;a.download='sophia-word-analysis.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);})));
      }
      scene.invalidate();return packet;
    }catch(error){report(error);throw error;}
  }
  function selectionChanged(){/* Drafts remain anchored to their explicit target. */}
  function chooseGap(id){
    const gap=gapHits.get(id);if(!gap)return false;
    gapSelection={id:gap.edge_id,kind:'edge',semantic_kind:'source_access_gap',label:gap.to_label,from_id:gap.from_id,to_id:gap.to_id,predicate_id:gap.predicate_id,
      source_refs:gap.source_refs,authority_posture:gap.authority_posture,review_posture:gap.review_posture,canon_status:gap.canon_status,reroutable:false};
    target=gapSelection;panels.open('workspace');open.setAttribute('aria-expanded','true');switchTab('notes');
    onChange();return true;
  }
  const handlers={
    'tos.page.research-workspace':()=>({packet:JSON.parse(workspace.exportPacket()),summary:workspace.summary()}),
    'tos.page.add-research-note':input=>addNote(String(input.text||''),input.target_id?String(input.target_id):undefined),
    'tos.page.add-session-hypothesis':input=>({hypothesis:hypothesis(String(input.statement||''),selected()),summary:workspace.summary()}),
    'tos.page.stage-proposal':input=>({proposal:stage(String(input.statement||''),selected(),input),summary:workspace.summary()}),
    'tos.page.workspace-undo':()=>({changed:workspace.undo(),summary:workspace.summary()}),
    'tos.page.workspace-redo':()=>({changed:workspace.redo(),summary:workspace.summary()}),
    'tos.page.workspace-export':()=>({packet:JSON.parse(workspace.exportPacket())}),
    'tos.page.workspace-import':input=>({imported:workspace.importPacket(typeof input.packet==='string'?input.packet:JSON.stringify(input.packet)),summary:workspace.summary()}),
    'tos.page.find-source-gaps':async(input,{signal})=>{show('analysis');const out=el('div');body.append(out);const packet=await runTool('gaps',String(input.query||''),out,signal,input);return {...packet,gaps:packet.gaps.map(gap=>({...gap,id:gap.edge_id,label:gap.to_label,summary:gap.properties?.public_summary_en}))};},
    'tos.page.prepare-word-analysis':async(input,{signal})=>{show('analysis');const out=el('div');body.append(out);return runTool('word',String(input.query||''),out,signal,input);},
  };
  let pendingRender=false;
  workspace.subscribe(()=>{
    onChange();if(pendingRender)return;pendingRender=true;
    queueMicrotask(()=>{pendingRender=false;if(panel.hidden||active!=='notes')return;
      const focused=document.activeElement,input=body.querySelector('textarea');
      const editing=focused===input,start=input?.selectionStart,end=input?.selectionEnd;
      body.replaceChildren();renderNotes();if(editing){const next=body.querySelector('textarea');next.focus();next.setSelectionRange(start,end);}scene.invalidate();
    });
  });
  refreshIcons();
  window.addEventListener('pagehide',()=>requests.cancelAll(),{once:true});
  return {workspace,handlers,selectionChanged,chooseGap,get auxiliarySelection(){return gapSelection;},clearAuxiliarySelection(){gapSelection=null;},agentStatus(state){footer.textContent=state.registered?'Агент подключён к текущему пространству':state.supported?'Подключаю агента…':'Записи хранятся локально';}};
}
