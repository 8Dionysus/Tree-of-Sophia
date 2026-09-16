import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {createResearchWorkspace,createLocalStoragePersistence} from '../research-workspace';
import {localized,displayTitle,RequestSlots} from './knowledge-client.mjs';
import {refreshIcons} from './icons';
import {stageObservation} from './research-actions';
import {readExactSource,exactSourceRepresentations,sourceReadExport} from './exact-source-read.mjs';
import {sourceLinkLabel,rawDataDownload} from './human-presentation.mjs';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
const button=(label,action)=>{const b=el('button',label);b.type='button';b.addEventListener('click',action);return b;};
export function sourceReferenceLink(ref){
  const label=sourceLinkLabel(ref);
  if(/^https?:\/\//i.test(ref)){try{const url=new URL(ref);const a=el('a',label,'sc-source-ref');a.href=url.href;a.target='_blank';a.rel='noreferrer noopener';return a;}catch{/* Render invalid references as a human source label. */}}
  const name=String(ref).split(/[\\/]/).filter(Boolean).at(-1)||String(ui('Источник'));
  const download=rawDataDownload({source_ref:ref},ui('Скачать ссылку: {0}',[name]),'sophia-source-reference.json');download.classList.add('sc-source-ref');return download;
}
const link=sourceReferenceLink;
const statusLabels={
  'pre-canon':ui("До канона"),canon:ui("Канон"),'derived-export':ui("Проекция источников"),
  prepared_research_candidate:ui("Исследовательский кандидат"),prepared_branch_candidate:ui("Кандидат ветви"),
  contested_review_required:ui("Требует рассмотрения"),pending_human_review:ui("Ожидает рассмотрения"),
  'not-recorded':ui("Не указан"),unresolved:ui("Не разрешено"),review_status_unresolved:ui("Статус рассмотрения не установлен"),
  unknown:ui("Не указан"),available:ui("Доступно"),missing:ui("Не найдено"),stale:ui("Устарело"),corrupt:ui("Повреждено"),
  'access-restricted':ui("Доступ ограничен"),'over-budget':ui("Слишком большой объём"),unsupported:ui("Не поддерживается"),
  'public-domain':ui('Общественное достояние'),'public-domain-reviewed':ui('Общественное достояние: проверено'),
  'open-licensed':ui('Открытая лицензия'),'permission-granted':ui('Разрешение получено'),
  'research-only':ui('Только для исследования'),restricted:ui('Доступ ограничен'),'rights-unknown':ui('Права не установлены'),rejected:ui('Отклонено'),
  'draft-not-sent':ui('Черновик запроса'),'awaiting-human-send-approval':ui('Ожидает разрешения на отправку'),
  sent:ui('Запрос отправлен'),'response-received':ui('Ответ получен'),'permission-denied':ui('В разрешении отказано'),
  expired:ui('Срок истёк'),withdrawn:ui('Запрос отозван'),
  source:ui("Источник"),projection:ui("Проекция"),runtime:ui("Рабочий слой"),pending:ui("Ожидает"),
};
export const humanSourceStatus=value=>Object.hasOwn(statusLabels,value)?statusLabels[value]:(typeof value==='string'&&value?ui('Статус источника: {0}',[value]):ui('Не указан'));
const humanStatus=humanSourceStatus;
export function createTools(root,scene,{data:{queries,client},selected,panels,onChange}){
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
    target=source?{id:source.raw.id,label:displayTitle(source.raw,ui("Материал без названия")),kind:source.kind==='relation'?'edge':'node',source_refs:source.raw.source_refs}:selected();
    rawSource=source?.raw||(target?.kind==='edge'?scene.port.relation(target.id):scene.port.node(target?.id));sourceKind=source?.kind||(target?.kind==='edge'?'relation':'node');
    panels.open('workspace');uiAttribute(open, 'aria-expanded', 'true');switchTab(tab);tabButtons[Object.keys(names).indexOf(tab)].focus();
  }
  function switchTab(tab){reading.capture();requests.cancelAll();uiAttribute(body, 'aria-busy', 'false');active=tab;reading.enter(JSON.stringify([scene.port.packet?.source_revision,target?.id,tab]));uiText(panel.querySelector('h3'), {notes:ui("Исследование"),sources:ui("Источники"),analysis:ui("Разбор текста")}[tab]);uiText(status, '');for(const [i,b]of tabButtons.entries()){const isActive=Object.keys(names)[i]===tab;uiAttribute(b, 'aria-selected', String(isActive));b.tabIndex=isActive?0:-1;}uiAttribute(body, 'aria-labelledby', 'sc-tool-'+tab);uiChildren(body, "replaceChildren");
    if(tab==='notes')renderNotes();else if(tab==='sources')void renderSources();else renderAnalysis();reading.restore();scene.invalidate();
  }
  panels.configure('workspace',{onResume:()=>{reading.restore();switchTab(active);}});
  root.addEventListener('sophia-sources',e=>show('sources',e.detail));
  function report(error,fallback=ui("Не удалось выполнить действие. Повторите запрос.")){if(error?.name==='AbortError')return;uiText(status,fallback);scene.invalidate();}
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
  function renderNotes(){
    const noteTarget=notebookDraft?draftTarget:target;uiChildren(body, "append", el('p',noteTarget?ui("Для: {0}", [noteTarget.label]):ui("Общие записи исследования"),'sc-muted'));
    const form=el('form'),label=el('label',ui("Новая запись"));label.htmlFor='sc-note-text';const input=el('textarea');input.id='sc-note-text';input.maxLength=2000;uiAttribute(input, "placeholder", ui("Мысль, вопрос или наблюдение…"));input.value=notebookDraft;
    input.addEventListener('input',()=>{if(!notebookDraft)draftTarget=target?{...target}:null;notebookDraft=input.value;});
    const kindLabel=el('label',ui("Тип записи"));kindLabel.htmlFor='sc-note-kind';const kind=el('select');kind.id='sc-note-kind';for(const [value,text]of [['note',ui("Заметка")],['hypothesis',ui("Гипотеза")],['proposal',ui("Предложение к рассмотрению")]]){const opt=el('option',text);opt.value=value;uiChildren(kind, "append", opt);}kind.value=draftKind;kind.addEventListener('change',()=>draftKind=kind.value);
    const submit=el('button',ui("Сохранить запись"));submit.type='submit';uiChildren(form, "append", label, input, kindLabel, kind, actions(submit));
    form.addEventListener('submit',e=>{e.preventDefault();safe(()=>{const text=input.value.trim();if(!text)throw new Error(ui("Напишите текст записи."));const bound=notebookDraft?draftTarget:target;if(kind.value==='hypothesis')hypothesis(text,bound);else if(kind.value==='proposal')stage(text,bound);else addNote(text,bound?.id);notebookDraft='';draftTarget=null;switchTab('notes');uiText(status, ui("Запись сохранена."));});});uiChildren(body, "append", form);
    const undo=button(ui("Отменить"),()=>{workspace.undo();switchTab('notes');}),redo=button(ui("Повторить действие"),()=>{workspace.redo();switchTab('notes');});undo.disabled=!workspace.canUndo();redo.disabled=!workspace.canRedo();
    const upload=el('input');upload.type='file';upload.accept='.json,application/json';upload.hidden=true;uiAttribute(upload, 'aria-label', ui("Импорт исследования"));
    upload.addEventListener('change',async()=>{const file=upload.files?.[0];if(!file)return;if(file.size>1000000){uiText(status,ui("Файл превышает 1 МБ."));return;}try{const packet=await file.text();workspace.importPacket(packet);switchTab('notes');uiText(status, ui("Исследование импортировано."));}catch(error){report(error,ui("Не удалось импортировать исследование. Проверьте файл и повторите."));}});
    const exportButton=rawDataDownload(JSON.parse(workspace.exportPacket()),ui("Экспорт записей"),'sophia-research.json');
    uiChildren(body, "append", actions(undo,redo,exportButton,button(ui("Импорт записей"),()=>upload.click()),button(ui("Полная копия исследования"),()=>root.dispatchEvent(new CustomEvent('sophia-workspace-copy')))), upload);
    const state=workspace.getState();
    for(const [type,items]of [[ui("Заметка"),state.notes],[ui("Гипотеза"),state.hypotheses],[ui("Предложение · ожидает рассмотрения"),state.proposals]])for(const item of items.slice().reverse()){
      const entry=el('article','','sc-entry');uiChildren(entry, "append", el('small',type), el('p',item.body||item.statement));
      if(type==='Заметка')uiChildren(entry, "append", actions(button(ui("Удалить"),()=>{workspace.removeNote(item.id);switchTab('notes');})));
      uiChildren(body, "append", entry);
    }
    if(!state.notes.length&&!state.hypotheses.length&&!state.proposals.length)uiChildren(body, "append", el('p',ui("Здесь появятся ваши записи."),'sc-muted'));
    if(workspace.persistenceError())uiText(status, ui("Браузер не смог сохранить записи на диск. Экспортируйте исследование перед закрытием."));
  }
  function sourceRecord(record){
    const entry=el('article','','sc-entry');uiChildren(entry, "append", el('h4',record.label||record.preferred_label||ui("Материал без названия")));
    const properties=record.properties||{};if(properties.description||properties.notes)uiChildren(entry, "append", el('p',properties.description||properties.notes,'sc-source-text'));
    for(const ref of [...new Set([...(record.source_refs||[]),properties.url,properties.locator,properties.source_url].filter(v=>typeof v==='string'))])uiChildren(entry, "append", link(ref));
    return entry;
  }
  async function renderSources(){
    context();if(!rawSource){uiChildren(body, "append", el('p',ui("Выберите звезду или отношение, чтобы увидеть источники.")));return;}
    const source=rawSource;uiChildren(body, "append", el('p',localized(sourceKind==='relation'?source.display.explanation:source.display.summary,ui("Описание пока не зафиксировано.")),'sc-source-text'));
    const provenance=el('details');uiChildren(provenance, "append", el('summary',ui("Происхождение и статус")));
    for(const [label,value]of [[ui("Слой"),source.epistemic?.authority_layer],[ui("Рассмотрение"),source.epistemic?.review_posture],[ui("Канон"),source.epistemic?.canon_status]])uiChildren(provenance, "append", el('p',`${String(label)}: ${String(humanStatus(value))}`,'sc-muted'));
    for(const ref of source.source_refs||[])uiChildren(provenance, "append", link(ref));uiChildren(body, "append", provenance);
    const sourceRevision=scene.port.packet?.source_revision,kind=sourceKind;
    const exact=el('section','','sc-exact-source');
    const isCurrent=()=>!panel.hidden&&active==='sources'&&rawSource===source&&scene.port.packet?.source_revision===sourceRevision;
    async function openExact(representation='record',output=exact){
      if(representation!=='record'){
        if(isCurrent())root.dispatchEvent(new CustomEvent('sophia-read-native',{detail:{selection:{kind,id:source.id,source_revision:sourceRevision,content_revision:source.content_revision},representation}}));
        return;
      }
      output.dataset.sourceReadStatus='loading';
      uiChildren(output,"replaceChildren",el('p',ui("Читаю точную исходную запись…"),'sc-muted'));
      try{
        const response=await requests.run('exact-source-'+representation,signal=>readExactSource(client,
          {kind,id:source.id,source_revision:sourceRevision,content_revision:source.content_revision},{signal,representation}));
        if(!response.current||!isCurrent()||!output.isConnected)return;
        const read=response.value;output.dataset.sourceReadStatus=read.status;
        uiChildren(output,"replaceChildren");
        if(read.status!=='available'){
          uiChildren(output,"append",el('p',ui("Запись источника сейчас недоступна. Попробуйте открыть её позже."),'sc-muted'));scene.invalidate();return;
        }
        {
          if(typeof read.record.preferred_label==='string')uiChildren(exact,"append",el('h4',read.record.preferred_label));
          const notes=read.layer==='authored_csv_record'?read.record.note:read.record.notes;
          if(typeof notes==='string'&&notes.trim())uiChildren(exact,"append",el('p',notes,'sc-source-text'));
          uiChildren(exact,"append",rawDataDownload(sourceReadExport(read),ui("Скачать запись"),'sophia-source-record.json'));
          if(read.record.native_text_binding){
            const choices=el('div','','sc-native-source-choices');uiChildren(exact,"append",choices);
            const response=await requests.run('source-representations',signal=>exactSourceRepresentations(client,sourceRevision,signal));
            if(!response.current||!isCurrent()||!choices.isConnected)return;
            for(const representation of response.value){
              const native=el('section','','sc-native-source');native.dataset.representation=representation;
              const label=representation==='native_local_unit'?ui("Открыть точный текст на локальных условиях"):ui("Открыть точный публичный текст");
              uiChildren(choices,"append",actions(button(label,()=>void openExact(representation,native))),native);
            }
          }
        }
        reading.restore();scene.invalidate();
      }catch(error){if(!isCurrent()||!output.isConnected)return;
        if(output.dataset.sourceReadStatus==='loading'){output.dataset.sourceReadStatus='error';uiChildren(output,"replaceChildren",el('p',ui("Не удалось прочитать запись источника. Попробуйте ещё раз."),'sc-muted'));}
        else uiChildren(output,"append",el('p',ui("Не удалось прочитать запись источника. Попробуйте ещё раз."),'sc-muted'));
        scene.invalidate();}
    }
    uiChildren(body,"append",actions(button(ui("Открыть исходную запись"),()=>void openExact())),exact);
    if(sourceKind==='relation')return;
    // The access packet advertises the exact source-navigation owner handle.
    // A transport-native id (for example identity:tos.expression...) is not a
    // dossier route and must never be repaired in the browser by stripping a
    // prefix or guessing from the entity id.
    const dossierRef=source.source_dossier_ref;
    if(!dossierRef)return;
    const result=el('div');uiChildren(body, "append", result);uiChildren(result, "append", el('p',ui("Получаю досье источников…"),'sc-muted'));
    uiAttribute(body, 'aria-busy', 'true');
    try{
      const response=await requests.run('source',signal=>queries.invoke('tos.dossier.inspect',{object_id:dossierRef,limit:40},{signal}));if(!response.current||panel.hidden||active!=='sources')return;
      uiChildren(result, "replaceChildren");const packet=response.value;
      for(const [key,title]of [['work',ui("Произведение")],['expression',ui("Редакции и переводы")],['edition',ui("Издания")],['file',ui("Файлы")],['item',ui("Экземпляры")],['link',ui("Ссылки")]]){
        const items=packet.chain?.[key]||[];if(!items.length)continue;const group=el('details');uiChildren(group, "append", el('summary',title+' · '+items.length));for(const item of items)uiChildren(group, "append", sourceRecord(item));uiChildren(result, "append", group);
      }
      const boundary=packet.agent_summary;if(boundary)uiChildren(result, "append", el('p',ui("Права: {0}.",[String(humanStatus(boundary.rights_posture))]),'sc-muted'));
      if(!result.children.length)uiChildren(result, "append", el('p',ui("Дополнительные маршруты источников пока не записаны."),'sc-muted'));
      uiAttribute(body, 'aria-busy', 'false');reading.restore();scene.invalidate();
    }catch(error){uiAttribute(body, 'aria-busy', 'false');uiChildren(result, "replaceChildren", el('p',ui("Не удалось загрузить досье источников. Попробуйте ещё раз."),'sc-muted'));uiChildren(result, "append", actions(button(ui("Повторить"),()=>switchTab('sources'))));scene.invalidate();}
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
        gapHits.clear();for(const gap of packet.gaps||[]){gapHits.set(gap.edge_id,gap);const entry=el('article','','sc-entry');uiChildren(entry, "append", el('h4',gap.to_label||gap.label||ui("Источник без названия")), el('p',gap.properties?.public_summary_en||gap.summary||ui("Описание пока недоступно.")), el('small',ui("Доступ: {0} · Запрос: {1}", [String(humanStatus(gap.access_status)), String(humanStatus(gap.request_status))])));for(const ref of gap.source_refs||[])uiChildren(entry, "append", link(ref));uiChildren(entry, "append", actions(button(ui("Рассмотреть"),()=>chooseGap(gap.edge_id))));uiChildren(out, "append", entry);}
        if(!packet.gaps?.length)uiChildren(out, "append", el('p',ui("По этому запросу пробелов не найдено.")));
      }else if(packet.available!==true){uiChildren(out, "append", el('p',ui("Разбор для этого запроса сейчас недоступен. Попробуйте другой запрос.")));}
      else{
        const source=packet.task?.source||{};uiChildren(out, "append", el('h4',source.surface||source.text||query), el('p',source.context||source.excerpt||source.sentence||''));if(source.source_ref)uiChildren(out, "append", link(source.source_ref));
        uiChildren(out, "append", el('p',ui("Подготовлен разбор по исходному тексту. Результат требует рассмотрения."),'sc-muted'));
        // Preserve the full source-bound analysis task for the agent and local export.
        uiChildren(out, "append", actions(rawDataDownload(packet,ui("Сохранить задание"),'sophia-word-analysis.json')));
      }
      scene.invalidate();return packet;
    }catch(error){report(error,ui("Не удалось выполнить исследование. Повторите запрос."));throw error;}
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
