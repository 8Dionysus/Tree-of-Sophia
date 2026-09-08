import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {createResearchWorkspace,createLocalStoragePersistence} from '../research-workspace';
import {localized,RequestSlots} from './knowledge-client.mjs';
import {refreshIcons} from './icons';
import {stageObservation} from './research-actions';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
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
  const open=button('',()=>show('notes'));open.className='sc-control sc-workspace-open';uiAttribute(open, 'aria-label', ui("Исследование"));uiAttribute(open, 'aria-expanded', 'false');
  uiHTML(open, '<i data-lucide="notebook-pen" aria-hidden="true"></i><span>Исследование</span>');
  uiChildren(root.querySelector('.sc-header-actions'), "append", open);
  const panel=el('section','','sc-panel sc-workspace');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Исследовательская панель"));
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">РАБОЧЕЕ ПРОСТРАНСТВО</span><button type="button" class="sc-icon sc-workspace-close" aria-label="Закрыть исследование"><i data-lucide="x" aria-hidden="true"></i></button></div><h3>Исследование</h3><div class="sc-workspace-tabs" role="tablist" aria-label="Инструменты исследования"></div><div class="sc-workspace-body" role="tabpanel" id="sc-tool-content"></div><div class="sc-tool-status" role="status"></div><div class="sc-workspace-footer"></div>');
  uiChildren(root, "append", panel);
  panels.register('workspace',panel,()=>{reading.capture();requests.cancelAll();uiAttribute(open, 'aria-expanded', 'false');});
  const body=panel.querySelector('.sc-workspace-body'),status=panel.querySelector('.sc-tool-status'),tabs=panel.querySelector('.sc-workspace-tabs'),footer=panel.querySelector('.sc-workspace-footer');
  const reading=createReadingMemory(body);
  let active='notes',target=null,rawSource=null,sourceKind='node',focusReturn=open,notebookDraft='',draftKind='note',draftTarget=null;
  const names={notes:ui("Записи"),sources:ui("Источники"),analysis:ui("Разбор")};
  const tabButtons=Object.entries(names).map(([id,name])=>{const b=button(name,()=>switchTab(id));b.id='sc-tool-'+id;uiAttribute(b, 'role', 'tab');uiAttribute(b, 'aria-controls', 'sc-tool-content');uiChildren(tabs, "append", b);return b;});
  tabs.addEventListener('keydown',e=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(e.key))return;e.preventDefault();const index=tabButtons.indexOf(e.target),next=e.key==='Home'?0:e.key==='End'?2:(index+(e.key==='ArrowLeft'?2:1))%3;switchTab(Object.keys(names)[next]);tabButtons[next].focus();});
  function close(){panels.close('workspace');(focusReturn?.isConnected&&!focusReturn.closest('[hidden]')?focusReturn:open).focus();}
  panel.querySelector('.sc-workspace-close').addEventListener('click',close);
  panel.addEventListener('keydown',e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}});
  function show(tab='notes',source){
    reading.capture();focusReturn=document.activeElement instanceof HTMLElement?document.activeElement:open;
    target=source?{id:source.raw.id,label:localized(source.raw.display.title||source.raw.display.label),kind:source.kind==='relation'?'edge':'node',source_refs:source.raw.source_refs}:selected();
    rawSource=source?.raw||(target?.kind==='edge'?scene.port.relation(target.id):scene.port.node(target?.id));sourceKind=source?.kind||(target?.kind==='edge'?'relation':'node');
    panels.open('workspace');uiAttribute(open, 'aria-expanded', 'true');switchTab(tab);tabButtons[Object.keys(names).indexOf(tab)].focus();
  }
  function switchTab(tab){reading.capture();requests.cancelAll();uiAttribute(body, 'aria-busy', 'false');active=tab;reading.enter(JSON.stringify([scene.port.packet?.source_revision,target?.id,tab]));uiText(panel.querySelector('h3'), {notes:ui("Исследование"),sources:ui("Источники"),analysis:ui("Разбор текста")}[tab]);uiText(status, '');for(const [i,b]of tabButtons.entries()){const isActive=Object.keys(names)[i]===tab;uiAttribute(b, 'aria-selected', String(isActive));b.tabIndex=isActive?0:-1;}uiAttribute(body, 'aria-labelledby', 'sc-tool-'+tab);uiChildren(body, "replaceChildren");
    if(tab==='notes')renderNotes();else if(tab==='sources')void renderSources();else renderAnalysis();reading.restore();scene.invalidate();
  }
  panels.configure('workspace',{onResume:()=>{reading.restore();switchTab(active);}});
  root.addEventListener('sophia-sources',e=>show('sources',e.detail));
  function report(error){if(error?.name==='AbortError')return;uiText(status, error.message||ui("Не удалось выполнить действие."));scene.invalidate();}
  function safe(action){try{return action();}catch(error){report(error);}}
  function actions(...items){const row=el('div','','sc-tool-actions');uiChildren(row, "append", ...items);return row;}
  function context(){uiChildren(body, "append", el('p',target?ui("Для: {0}", [target.label]):ui("Общие записи исследования"),'sc-muted'));}
  function newId(kind){return `${kind}:${crypto.randomUUID()}`;}
  function addNote(text,targetId){workspace.addNote({id:newId('note'),body:text,targetId});return {added:true,summary:workspace.summary()};}
  function hypothesis(text,selection=target){
    return workspace.addHypothesis({id:newId('hypothesis'),title:text.slice(0,100),body:text,targetId:selection?.id,
      ...(selection?.from_id&&selection?.to_id?{fromId:selection.from_id,toId:selection.to_id}:{})});
  }
  function stage(text,selection=target,input={}){
    if(!selection?.id)throw new Error(ui("Сначала выберите звезду или отношение."));
    const refs=input.source_refs||selection.source_refs||[];if(!refs.length)throw new Error(ui("Для предложения нужен хотя бы один источник."));
    const kind=input.kind||'interpretation',fromId=input.from_id||selection.from_id,toId=input.to_id||selection.to_id;
    if(['relation','source_route'].includes(kind)&&(!fromId||!toId))throw new Error(ui("Для предложения о связи нужны оба участника."));
    return stageObservation(workspace,{id:newId('proposal'),kind,parentHypothesisId:newId('hypothesis'),targetId:selection.id,
      ...(fromId&&toId?{fromId,toId}:{}),statement:text,sourceRefs:refs,evidenceRefs:input.evidence_refs?.length?input.evidence_refs:refs,
      confidencePosture:{value:input.confidence||'unknown',meaning:'maker_declared_uncertainty_not_truth_probability'},
      actorOrigin:input.actor_origin==='agent'?'agent':'human',basePageRevision:input.context_revision||0,
      dataFingerprint:scene.port.packet?.source_revision||'unavailable'});
  }
  function download(){const url=URL.createObjectURL(new Blob([workspace.exportPacket()],{type:'application/json'}));const a=el('a');a.href=url;a.download='sophia-research.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
  function renderNotes(){
    const noteTarget=notebookDraft?draftTarget:target;uiChildren(body, "append", el('p',noteTarget?ui("Для: {0}", [noteTarget.label]):ui("Общие записи исследования"),'sc-muted'));uiChildren(body, "append", el('p',ui("Записи и гипотезы сохраняются в этом браузере. Предложения остаются черновиками до рассмотрения."),'sc-muted'));
    const form=el('form'),label=el('label',ui("Новая запись"));label.htmlFor='sc-note-text';const input=el('textarea');input.id='sc-note-text';input.maxLength=2000;uiAttribute(input, "placeholder", ui("Мысль, вопрос или наблюдение…"));input.value=notebookDraft;
    input.addEventListener('input',()=>{if(!notebookDraft)draftTarget=target?{...target}:null;notebookDraft=input.value;});
    const kindLabel=el('label',ui("Тип записи"));kindLabel.htmlFor='sc-note-kind';const kind=el('select');kind.id='sc-note-kind';for(const [value,text]of [['note',ui("Заметка")],['hypothesis',ui("Гипотеза")],['proposal',ui("Предложение к рассмотрению")]]){const opt=el('option',text);opt.value=value;uiChildren(kind, "append", opt);}kind.value=draftKind;kind.addEventListener('change',()=>draftKind=kind.value);
    const submit=el('button',ui("Сохранить запись"));submit.type='submit';uiChildren(form, "append", label, input, kindLabel, kind, actions(submit));
    form.addEventListener('submit',e=>{e.preventDefault();safe(()=>{const text=input.value.trim();if(!text)throw new Error(ui("Напишите текст записи."));const bound=notebookDraft?draftTarget:target;if(kind.value==='hypothesis')hypothesis(text,bound);else if(kind.value==='proposal')stage(text,bound);else addNote(text,bound?.id);notebookDraft='';draftTarget=null;switchTab('notes');uiText(status, ui("Запись сохранена."));});});uiChildren(body, "append", form);
    const undo=button(ui("Отменить"),()=>{workspace.undo();switchTab('notes');}),redo=button(ui("Повторить действие"),()=>{workspace.redo();switchTab('notes');});undo.disabled=!workspace.canUndo();redo.disabled=!workspace.canRedo();
    const upload=el('input');upload.type='file';upload.accept='.json,application/json';upload.hidden=true;uiAttribute(upload, 'aria-label', ui("Импорт исследования"));
    upload.addEventListener('change',async()=>{const file=upload.files?.[0];if(!file)return;if(file.size>1000000){report(new Error(ui("Файл превышает 1 МБ.")));return;}try{const packet=await file.text();workspace.importPacket(packet);switchTab('notes');uiText(status, ui("Исследование импортировано."));}catch(error){report(error);}});
    uiChildren(body, "append", actions(undo,redo,button(ui("Экспорт записей"),download),button(ui("Импорт записей"),()=>upload.click()),button(ui("Полная копия исследования"),()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy')))), upload);
    const state=workspace.getState();
    for(const [type,items]of [[ui("Заметка"),state.notes],[ui("Гипотеза"),state.hypotheses],[ui("Предложение · ожидает рассмотрения"),state.proposals]])for(const item of items.slice().reverse()){
      const entry=el('article','','sc-entry');uiChildren(entry, "append", el('small',type), el('p',item.body||item.statement));if(item.targetId)uiChildren(entry, "append", el('small',item.targetId));
      if(type==='Заметка')uiChildren(entry, "append", actions(button(ui("Удалить"),()=>{workspace.removeNote(item.id);switchTab('notes');})));
      uiChildren(body, "append", entry);
    }
    if(!state.notes.length&&!state.hypotheses.length&&!state.proposals.length)uiChildren(body, "append", el('p',ui("Здесь появятся ваши записи."),'sc-muted'));
    if(workspace.persistenceError())uiText(status, ui("Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием."));
  }
  function sourceRecord(record){
    const entry=el('article','','sc-entry');uiChildren(entry, "append", el('h4',record.label||record.preferred_label||record.node_id));
    const properties=record.properties||{};if(properties.description||properties.notes)uiChildren(entry, "append", el('p',properties.description||properties.notes,'sc-source-text'));
    for(const ref of [...new Set([...(record.source_refs||[]),properties.url,properties.locator,properties.source_url].filter(v=>typeof v==='string'))])uiChildren(entry, "append", link(ref));
    return entry;
  }
  async function renderSources(){
    context();if(!rawSource){uiChildren(body, "append", el('p',ui("Выберите звезду или отношение, чтобы увидеть источники.")));return;}
    const source=rawSource;uiChildren(body, "append", el('p',localized(sourceKind==='relation'?source.display.explanation:source.display.summary,ui("Описание пока не зафиксировано.")),'sc-source-text'));
    const provenance=el('details');uiChildren(provenance, "append", el('summary',ui("Происхождение и статус")));
    for(const [label,value]of [[ui("Слой"),source.epistemic?.authority_layer],[ui("Рассмотрение"),source.epistemic?.review_posture],[ui("Канон"),source.epistemic?.canon_status]])uiChildren(provenance, "append", el('p',label+': '+(value&&value!=='not-recorded'?value:ui("не указан")),'sc-muted'));
    for(const ref of source.source_refs)uiChildren(provenance, "append", link(ref));uiChildren(body, "append", provenance);
    if(sourceKind==='relation')return;
    const nativeId=source.native_id; // Explicit owner identity; never split or guess an opaque knowledge ID.
    if(!nativeId)return;
    const result=el('div');uiChildren(body, "append", result);uiChildren(result, "append", el('p',ui("Получаю досье источников…"),'sc-muted'));
    uiAttribute(body, 'aria-busy', 'true');
    try{
      const response=await requests.run('source',signal=>queries.invoke('tos.dossier.inspect',{object_id:nativeId,limit:40},{signal}));if(!response.current||panel.hidden||active!=='sources')return;
      uiChildren(result, "replaceChildren");const packet=response.value;
      for(const [key,title]of [['work',ui("Произведение")],['expression',ui("Редакции и переводы")],['edition',ui("Издания")],['file',ui("Файлы")],['item',ui("Экземпляры")],['link',ui("Ссылки")]]){
        const items=packet.chain?.[key]||[];if(!items.length)continue;const group=el('details');uiChildren(group, "append", el('summary',title+' · '+items.length));for(const item of items)uiChildren(group, "append", sourceRecord(item));uiChildren(result, "append", group);
      }
      const boundary=packet.agent_summary;if(boundary)uiChildren(result, "append", el('p',ui("Доступность ссылки и право использования — отдельные сведения. Статус прав: {0}.", [(boundary.rights_posture&&boundary.rights_posture!=='unknown'?boundary.rights_posture:ui("не указан"))]),'sc-muted'));
      if(!result.children.length)uiChildren(result, "append", el('p',ui("Дополнительные маршруты источников пока не записаны."),'sc-muted'));
      uiAttribute(body, 'aria-busy', 'false');reading.restore();scene.invalidate();
    }catch(error){uiAttribute(body, 'aria-busy', 'false');uiChildren(result, "replaceChildren", el('p',error.message,'sc-muted'));uiChildren(result, "append", actions(button(ui("Повторить"),()=>switchTab('sources'))));scene.invalidate();}
  }
  function renderAnalysis(){
    uiChildren(body, "append", el('p',ui("Ищите недостающие источники или подготовьте разбор слова в «Заратустре»."),'sc-muted'));
    const label=el('label',ui("Запрос"));label.htmlFor='sc-analysis-query';const input=el('input');input.id='sc-analysis-query';uiAttribute(input, "placeholder", ui("Название источника или слово…"));input.maxLength=256;
    const result=el('div');uiChildren(body, "append", label, input, actions(button(ui("Пробелы в источниках"),()=>runTool('gaps',input.value,result).catch(()=>{})),button(ui("Разобрать слово"),()=>runTool('word',input.value,result).catch(()=>{}))), result);
  }
  async function runTool(kind,query,out,externalSignal,options={}){
    uiText(status, ui("Получаю материал…"));uiChildren(out, "replaceChildren");
    try{
      const operation=kind==='gaps'?'tos.source-gaps.search':'tos.zarathustra.word-analysis.prepare';
      const input=kind==='gaps'?{query,limit:options.limit??12}:{query,language:options.language||'ru',rank:options.rank??1,include_semantic_neighbors:options.include_semantic_neighbors===true};
      const response=await requests.run('analysis',signal=>queries.invoke(operation,input,{signal:externalSignal?AbortSignal.any([signal,externalSignal]):signal}));
      if(!response.current||panel.hidden||active!=='analysis'){externalSignal?.throwIfAborted();throw new DOMException('Panel closed','AbortError');}externalSignal?.throwIfAborted();const packet=response.value;uiText(status, '');
      if(kind==='gaps'){
        gapHits.clear();for(const gap of packet.gaps||[]){gapHits.set(gap.edge_id,gap);const entry=el('article','','sc-entry');uiChildren(entry, "append", el('h4',gap.to_label||gap.label), el('p',gap.properties?.public_summary_en||gap.summary||''), el('small',ui("Доступ: {0} · Запрос: {1}", [gap.access_status, gap.request_status])));for(const ref of gap.source_refs||[])uiChildren(entry, "append", link(ref));uiChildren(entry, "append", actions(button(ui("Рассмотреть"),()=>chooseGap(gap.edge_id))));uiChildren(out, "append", entry);}
        if(!packet.gaps?.length)uiChildren(out, "append", el('p',ui("По этому запросу пробелов не найдено.")));
      }else if(packet.available!==true){uiChildren(out, "append", el('p',ui("Разбор для этого запроса сейчас недоступен.")), el('p',packet.reason||'','sc-muted'));}
      else{
        const source=packet.task?.source||{};uiChildren(out, "append", el('h4',source.surface||source.text||query), el('p',source.context||source.excerpt||source.sentence||''));if(source.source_ref)uiChildren(out, "append", link(source.source_ref));
        uiChildren(out, "append", el('p',ui("Подготовлен разбор по исходному тексту. Результат требует рассмотрения."),'sc-muted'));
        // Preserve the full source-bound analysis task for the agent and local export.
        uiChildren(out, "append", actions(button(ui("Сохранить задание"),()=>{const url=URL.createObjectURL(new Blob([JSON.stringify(packet,null,2)],{type:'application/json'}));const a=el('a');a.href=url;a.download='sophia-word-analysis.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);})));
      }
      scene.invalidate();return packet;
    }catch(error){report(error);throw error;}
  }
  function selectionChanged(){/* Drafts remain anchored to their explicit target. */}
  function chooseGap(id){
    const gap=gapHits.get(id);if(!gap)return false;
    gapSelection={id:gap.edge_id,kind:'edge',semantic_kind:'source_access_gap',label:gap.to_label,from_id:gap.from_id,to_id:gap.to_id,predicate_id:gap.predicate_id,
      source_refs:gap.source_refs,authority_posture:gap.authority_posture,review_posture:gap.review_posture,canon_status:gap.canon_status,reroutable:false};
    target=gapSelection;panels.open('workspace');uiAttribute(open, 'aria-expanded', 'true');switchTab('notes');
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
    'tos.page.find-source-gaps':async(input,{signal})=>{show('analysis');const out=el('div');uiChildren(body, "append", out);const packet=await runTool('gaps',String(input.query||''),out,signal,input);return {...packet,gaps:packet.gaps.map(gap=>({...gap,id:gap.edge_id,label:gap.to_label,summary:gap.properties?.public_summary_en}))};},
    'tos.page.prepare-word-analysis':async(input,{signal})=>{show('analysis');const out=el('div');uiChildren(body, "append", out);return runTool('word',String(input.query||''),out,signal,input);},
  };
  let pendingRender=false;
  workspace.subscribe(()=>{
    onChange();if(pendingRender)return;pendingRender=true;
    queueMicrotask(()=>{pendingRender=false;if(panel.hidden||active!=='notes')return;
      const focused=document.activeElement,input=body.querySelector('textarea');
      const editing=focused===input,start=input?.selectionStart,end=input?.selectionEnd;
      uiChildren(body, "replaceChildren");renderNotes();if(editing){const next=body.querySelector('textarea');next.focus();next.setSelectionRange(start,end);}scene.invalidate();
    });
  });
  refreshIcons();
  window.addEventListener('pagehide',()=>requests.cancelAll(),{once:true});
  return {workspace,handlers,selectionChanged,chooseGap,get auxiliarySelection(){return gapSelection;},clearAuxiliarySelection(){gapSelection=null;},agentStatus(state){uiText(footer, state.registered?ui("Агент подключён к текущему пространству"):state.supported?ui("Подключаю агента…"):ui("Записи хранятся локально"));}};
}
