import {ui,uiAttribute,uiChildren,uiHTML,uiNode,uiText} from './ui-i18n.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {createReadingShelf,readingDocument,readingLanguages,readingKey,readingPositionKey,formLabel,formLanguageNote} from './reader-model.mjs';
import {renderHumanForms,renderClaimContext,renderEssentialContext} from './human-forms-view.mjs';
import './human-forms.css';
import {READING_KEY,readReading,emptyReading,validateReading} from './reading-resume.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
const button=(label,action,className='')=>{const node=el('button',label,className);node.type='button';node.addEventListener('click',action);return node;};
const originLabels={authored:ui("Авторское описание"),'source-derived':ui("Описание из источника"),'metadata-synthesis':ui("Описание составлено из метаданных")};
const postureLabels={disputed:ui("Оспаривается"),rejected:ui("Отклонено"),accepted:ui("Принято в указанной источником области"),
  contested_review_required:ui("Требует рассмотрения"),unresolved:ui("Не разрешено"),review_status_unresolved:ui("Статус рассмотрения не установлен"),
  pending_human_review:ui("Ожидает рассмотрения"),unreviewed:ui("Не рассмотрено"),'not-recorded':ui("Не указан")};

export function createReaderPanel(root,scene,panels,{data:{client},onUserAction=()=>{}}){
  const views=new Map();let activeKey=null,returnFocus=null,notice='',wide=false;
  let storage=null,savedText=null,saveTimer=null,storageError='',writable=true,initial=emptyReading();
  const storageKey=READING_KEY+':'+location.pathname;
  try{storage=localStorage;savedText=storage.getItem(storageKey);initial=readReading(storage,storageKey);}catch(error){storageError=error.message;writable=false;}
  const restoredViews=new Map(initial.entries.map(entry=>[readingKey(entry.kind,entry.id),entry]));
  const opener=button('',()=>show(),'sc-control sc-reader-open');
  uiAttribute(opener, 'aria-label', ui("Чтение и сопоставление"));uiAttribute(opener, 'aria-expanded', 'false');
  uiHTML(opener, '<i data-lucide="book-open" aria-hidden="true"></i><span>Чтение</span>');
  uiChildren(root.querySelector('.sc-header-actions'), "append", opener);
  const resume=button('',()=>show(),'sc-reader-resume');resume.hidden=true;
  uiChildren(root.querySelector('.sc-context'), "append", resume);
  const panel=el('section','','sc-panel sc-reader');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Чтение и сопоставление"));
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">ЧТЕНИЕ</span><button type="button" class="sc-icon sc-reader-close" aria-label="Закрыть чтение"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-reader-heading"><h3 tabindex="-1">Удержать мысль</h3></div><div class="sc-reader-toolbar"></div><div class="sc-reader-tabs" role="tablist" aria-label="Закреплённые материалы"></div><div class="sc-reader-columns"></div><p class="sc-reader-notice" role="status"></p>');
  uiChildren(root, "append", panel);
  const columns=panel.querySelector('.sc-reader-columns'),tabs=panel.querySelector('.sc-reader-tabs'),status=panel.querySelector('.sc-reader-notice');
  const add=button(ui("Добавить выбранное"),()=>pinSelection(),'sc-reader-add');uiChildren(panel.querySelector('.sc-reader-toolbar'), "append", add);
  const empty=el('p',ui("Оставьте здесь предмет или связь, чтобы читать, переходить к основаниям и сопоставлять с другим материалом."),'sc-reader-empty');
  const shelf=createReadingShelf({client,onChange:render});
  const current=()=>{
    const selection=scene.port.selection,kind=selection.relationId?'relation':'node';
    const raw=kind==='relation'?scene.port.relation(selection.relationId):scene.port.node(selection.nodeId);
    return raw?{raw,kind,sourceRevision:scene.port.packet?.source_revision,preferred:root.dataset.materialLanguage||'ru'}:null;
  };
  const entryFor=key=>shelf.entries.find(entry=>entry.key===key);
  function capture(){for(const view of views.values())view.reading.capture();}
  function exportState(){
    capture();return validateReading({v:1,activeKey,entries:shelf.entries.map(entry=>{
      const view=views.get(entry.key),prefix=JSON.stringify([entry.key,entry.sourceRevision,entry.contentRevision]).slice(0,-1)+',';
      return {kind:entry.kind,id:entry.id,sourceRevision:entry.sourceRevision,contentRevision:entry.contentRevision,preferred:view?.preferred||'ru',
        ...(entry.claimReference?{claimReference:entry.claimReference}:{}),
        positions:(view?.reading.exportPositions()||[]).filter(([key])=>key.startsWith(prefix))};
    })});
  }
  function persist(){
    clearTimeout(saveTimer);if(!writable)return;
    try{
      if(!storage)throw new Error(ui("Чтение сохраняется только до закрытия страницы: хранилище недоступно."));
      if(storage.getItem(storageKey)!==savedText){writable=false;throw new Error(ui("Чтение изменено в другой вкладке. Здесь новые изменения пока не сохранены."));}
      const text=JSON.stringify(exportState());clearTimeout(saveTimer);storage.setItem(storageKey,text);savedText=text;storageError='';
    }catch(error){storageError=error.message;}
    uiText(status, storageError||notice||ui("Пара материалов, формы и позиции чтения сохраняются в этом браузере."));status.dataset.important=String(Boolean(storageError));
  }
  function scheduleSave(){clearTimeout(saveTimer);saveTimer=setTimeout(persist,500);}
  function restore(){for(const view of views.values())if(!view.article.hidden)view.reading.restore();}
  panels.register('reader',panel,()=>{capture();uiAttribute(opener, 'aria-expanded', 'false');});
  panels.configure('reader',{onResume:()=>{uiAttribute(opener, 'aria-expanded', 'true');render();}});
  panels.addTool('reader',{title:ui("Чтение и сопоставление"),opener,launch:()=>show()});
  function show(){
    onUserAction();returnFocus=document.activeElement;panels.open('reader');uiAttribute(opener, 'aria-expanded', 'true');render();
    (views.get(activeKey)?.body||panel.querySelector('h3')).focus();
  }
  function close(){
    onUserAction();panels.close('reader');
    const target=returnFocus?.isConnected&&!returnFocus.closest('[hidden]')?returnFocus:!resume.hidden?resume:root.querySelector('.sc-studio-open');
    target?.focus();
  }
  panel.querySelector('.sc-reader-close').addEventListener('click',close);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  function pinSelection(source=current()){
    if(!source){notice=ui("Сначала выберите звезду или связь.");show();return;}
    onUserAction();scene.ui.captureReading();
    try{
      const result=shelf.pin({...source,bookmark:scene.port.captureView()});activeKey=result.key;
      notice=result.existing?ui("Этот материал уже оставлен для чтения."):ui("Материал оставлен для продолжения чтения.");
    }catch(error){notice=error.message;}
    show();
  }
  function remove(key){
    onUserAction();capture();shelf.remove(key);notice=ui("Материал убран из чтения.");render();
    (views.get(activeKey)?.tab||add).focus();
  }
  function switchTo(key){onUserAction();capture();activeKey=key;layout();restore();scheduleSave();}
  function returnTo(key){
    const entry=entryFor(key);if(!entry?.bookmark)return;
    if(entry.bookmark.graph.packet.source_revision!==scene.port.packet?.source_revision){notice=ui("Данные изменились. Откройте актуальный материал в пространстве.");render();return;}
    onUserAction();capture();scene.ui.cancelPending();scene.port.restoreView(entry.bookmark);
  }
  function openInScene(key){
    const entry=entryFor(key);if(!entry?.snapshot)return;
    onUserAction();capture();
    if(entry.kind==='relation')void scene.ui.chooseRelation(entry.snapshot.raw,entry.sourceRevision);
    else scene.ui.chooseNode(entry.id,entry.sourceRevision);
  }
  function handoff(key,event){
    const entry=entryFor(key);if(!entry?.snapshot||entry.sourceRevision!==scene.port.packet?.source_revision)return;
    onUserAction();capture();root.dispatchEvent(new CustomEvent(event,{detail:{raw:entry.snapshot.raw,kind:entry.kind}}));
  }
  function makeView(entry){
    const key=entry.key,article=el('article','','sc-reader-article');article.dataset.readingId=entry.id;
    const head=el('div','','sc-reader-item-head'),title=el('h4'),kind=el('p','','sc-reader-kind');
    const removeButton=button(ui("Убрать"),()=>remove(key),'sc-reader-remove');
    uiChildren(head, "append", kind, title, removeButton);
    const controls=el('div','','sc-reader-item-controls'),language=el('select');uiAttribute(language, 'aria-label', ui("Язык или форма материала"));
    const languageLabel=el('label',ui("Форма"));uiChildren(languageLabel, "append", language);
    const refresh=button(ui("Обновить"),()=>{onUserAction();void shelf.refresh(key);});uiChildren(controls, "append", languageLabel, refresh);
    const state=el('p','','sc-reader-state');uiAttribute(state, 'role', 'status');
    const body=el('div','','sc-reader-body');body.tabIndex=0;
    const actions=el('div','','sc-reader-actions');
    const back=button(ui("К месту"),()=>returnTo(key)),open=button(ui("В пространстве"),()=>openInScene(key));
    uiAttribute(back, 'aria-label', ui("Вернуться к месту закрепления"));uiAttribute(open, 'aria-label', ui("Открыть материал в пространстве"));
    const evidence=button(ui("Основания"),()=>handoff(key,'sophia-evidence')),sources=button(ui("Источники"),()=>handoff(key,'sophia-sources'));
    uiChildren(actions, "append", back, open, evidence, sources);uiChildren(article, "append", head, controls, state, body, actions);
    const tab=button('',()=>switchTo(key));uiAttribute(tab, 'role', 'tab');tab.id='sc-reader-tab-'+crypto.randomUUID();
    article.id='sc-reader-item-'+crypto.randomUUID();uiAttribute(tab, 'aria-controls', article.id);
    tab.addEventListener('keydown',event=>{
      if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
      event.preventDefault();const entries=shelf.entries,index=entries.findIndex(item=>item.key===key);
      const next=entries[event.key==='Home'?0:event.key==='End'?entries.length-1:(index+1)%entries.length];
      switchTo(next.key);views.get(next.key).tab.focus();
    });
    const remembered=restoredViews.get(key);restoredViews.delete(key);
    const view={article,head,title,kind,language,refresh,state,body,back,open,evidence,sources,tab,reading:createReadingMemory(body,{limit:8,onCapture:scheduleSave}),preferred:remembered?.preferred||entry.preferred||'ru',snapshot:null};
    if(remembered)view.reading.importPositions(remembered.positions);
    language.addEventListener('change',()=>{onUserAction();view.reading.capture();view.preferred=language.value;void shelf.language(key,view.preferred);});
    uiChildren(columns, "append", article);uiChildren(tabs, "append", tab);return view;
  }
  function appendForm(parent,form,blockId){
    const text=el('div','','sc-reader-text');if(form.lang)text.lang=form.lang;
    // Paragraph anchors are page-local reading positions, never invented ToS
    // text addresses. Keep all delivered characters, including separators.
    form.text.split(/(\r?\n[\t ]*\r?\n)/).forEach((part,index)=>{
      if(index%2){uiChildren(text, "append", uiNode(part));return;}
      const p=el('p',part);p.dir='auto';p.dataset.readingAnchor=blockId+':'+index;uiChildren(text, "append", p);
    });uiChildren(parent, "append", text);
    if(form.fallback||!form.lang){
      uiChildren(parent, "append", el('p',formLanguageNote(form),'sc-reader-language-note'));
    }
  }
  function renderDocument(view,entry){
    const snapshot=entry.snapshot,doc=readingDocument(snapshot,view.preferred);
    view.reading.capture();view.reading.enter(readingPositionKey(entry.key,snapshot,snapshot.raw.human_form_selection?.requested_language||view.preferred));
    uiChildren(view.body, "replaceChildren");view.body.scrollTop=0;view.titleText=uiText(view.title, doc.title?.text||ui("Материал"));
    view.title.dir='auto';if(doc.title?.lang)view.title.lang=doc.title.lang;else view.title.removeAttribute('lang');
    uiText(view.kind, entry.kind==='relation'?ui("Связь"):doc.kind?.text||ui("Предмет"));
    if(doc.claimContextUnavailable)uiChildren(view.body,'append',el('p',ui('Связанный контекст утверждения не закреплён. Для полного чтения закрепите его из области, где этот контекст доступен.'),'sc-reader-gap'));
    if(!doc.title?.unavailable&&(doc.title?.fallback||!doc.title?.lang))uiChildren(view.body, "append", el('p',ui("Название: {0}{1}", [formLabel(doc.title?.key||ui("не указана")), (doc.title?.lang?'':ui("; язык не указан"))]),'sc-reader-language-note'));
    for(const block of doc.blocks){
      const section=el('section','','sc-reader-section');uiChildren(section, "append", el('h5',block.title));
      if(block.form)appendForm(section,block.form,block.id);
      else uiChildren(section, "append", el('p',block.id==='statement'?ui("Формулировка пока не предоставлена."):ui("Описание пока отсутствует."),'sc-reader-gap'));
      if(originLabels[block.state])uiChildren(section, "append", el('p',originLabels[block.state],'sc-reader-origin'));
      uiChildren(view.body, "append", section);
    }
    if(doc.humanForms)uiChildren(view.body,'append',renderHumanForms(snapshot.raw));
    uiChildren(view.body,'append',renderEssentialContext(doc.essentialContext));
    if(snapshot.claimReading)uiChildren(view.body,'append',renderClaimContext(snapshot.claimReading));
    if(doc.participants.length){
      const section=el('section','','sc-reader-section');uiChildren(section, "append", el('h5',ui("Участники связи")));
      for(const participant of doc.participants){
        const row=button('',()=>{onUserAction();capture();scene.ui.chooseNode(participant.id,snapshot.sourceRevision);},'sc-reader-participant');
        uiChildren(row, "append", el('small',participant.role), el('span',participant.form?.text||ui("Участник")));uiChildren(section, "append", row);
      }
      uiChildren(view.body, "append", section);
    }
    const review=doc.posture.review_posture;
    if(postureLabels[review]&&review!=='not-recorded')uiChildren(view.body, "append", el('p',ui("Рассмотрение: {0}.", [postureLabels[review]]),'sc-reader-assessment'));
    const refs=el('details','','sc-reader-section');refs.dataset.readingKey='sources';uiChildren(refs, "append", el('summary',ui("Источники · {0}", [doc.sourceRefs.length])));
    for(const ref of doc.sourceRefs){
      let link=el('span',ref,'sc-source-ref');
      try{const url=new URL(ref);if(['http:','https:'].includes(url.protocol)){link=el('a',ref,'sc-source-ref');link.href=url.href;link.target='_blank';link.rel='noopener noreferrer';}}catch{/* Local source references stay selectable text. */}
      uiChildren(refs, "append", link);
    }
    uiChildren(view.body, "append", refs);
    const technical=el('details','','sc-reader-technical');technical.dataset.readingKey='identity';uiChildren(technical, "append", el('summary',ui("Точные сведения о материале")));
    const details=el('dl');
    for(const [label,value]of [[ui("Идентификатор"),entry.id],[ui("Снимок данных"),snapshot.sourceRevision],[ui("Версия материала"),snapshot.raw.content_revision],[ui("Слой"),doc.posture.authority_layer],[ui("Рассмотрение"),review],[ui("Канон"),doc.posture.canon_status],[ui("Уверенность, как передана источником"),doc.posture.confidence]]){
      const row=el('div');uiChildren(row, "append", el('dt',label), el('dd',value===null||value===undefined||value==='not-recorded'?ui("Не указан"):String(value)));uiChildren(details, "append", row);
    }
    uiChildren(technical, "append", details);uiChildren(view.body, "append", technical);view.snapshot=snapshot;
  }
  function layout(){
    const entries=shelf.entries,twoColumns=wide&&entries.length===2;
    panel.dataset.columns=twoColumns?'2':'1';tabs.hidden=entries.length<2||twoColumns;
    for(const entry of entries){const view=views.get(entry.key),selected=entry.key===activeKey;
      view.article.hidden=!twoColumns&&!selected;uiAttribute(view.tab, 'aria-selected', String(selected));view.tab.tabIndex=selected?0:-1;
      if(!tabs.hidden){uiAttribute(view.article, 'role', 'tabpanel');uiAttribute(view.article, 'aria-labelledby', view.tab.id);}
      else{view.article.removeAttribute('role');view.article.removeAttribute('aria-labelledby');}
    }
    scene.invalidate();
  }
  function render(){
    if(!panel.hidden)wide=panel.clientWidth>=580;
    const entries=shelf.entries;
    for(const [key,view]of views)if(!entries.some(entry=>entry.key===key)){view.article.remove();view.tab.remove();views.delete(key);}
    if(!entries.some(entry=>entry.key===activeKey))activeKey=entries[0]?.key||null;
    for(const entry of entries){
      let view=views.get(entry.key);if(!view){view=makeView(entry);views.set(entry.key,view);}
      if(entry.snapshot&&view.snapshot!==entry.snapshot){
        const languages=readingLanguages(entry.snapshot);uiChildren(view.language, "replaceChildren", ...languages.map(key=>{const option=el('option',formLabel(key));option.value=key;return option;}));
        if(!languages.includes(view.preferred)){const option=el('option',ui("{0} — недоступна", [formLabel(view.preferred)]));option.value=view.preferred;uiChildren(view.language, "prepend", option);}
        view.language.value=view.preferred;renderDocument(view,entry);
      }else if(!entry.snapshot){
        view.snapshot=null;view.titleText=uiText(view.title, entry.title?.text||ui("Материал"));uiChildren(view.body, "replaceChildren", el('p',entry.loading?ui("Получаю материал…"):ui("Материал пока недоступен."),'sc-reader-gap'));
      }
      uiText(view.tab, view.titleText);uiAttribute(view.body, 'aria-label', ui("Чтение: {0}", [view.titleText]));
      view.language.disabled=!entry.snapshot;view.refresh.disabled=entry.loading;
      const mismatch=Boolean(entry.snapshot&&shelf.sceneRevision&&entry.sourceRevision!==shelf.sceneRevision);
      const states=[entry.loading?ui("Обновляю материал…"):null,entry.error,
        entry.changed?ui("Данные изменились: чтение начато с начала актуального материала."):null,
        mismatch?ui("Материал и сцена относятся к разным снимкам."):null,
        entry.snapshot&&entry.error?ui("Показан ранее закреплённый материал."):null];
      uiText(view.state, states.filter(Boolean).join(' '));view.state.hidden=!view.state.textContent;
      view.back.hidden=!entry.bookmark;view.back.disabled=entry.bookmark?.graph?.packet?.source_revision!==scene.port.packet?.source_revision;
      view.open.disabled=!entry.snapshot||mismatch;view.evidence.disabled=!entry.snapshot||mismatch;view.sources.disabled=!entry.snapshot||mismatch;
      view.article.dataset.snapshot=entry.snapshot?.sourceRevision||'';
    }
    empty.hidden=Boolean(entries.length);if(!empty.isConnected)uiChildren(columns, "append", empty);
    const selection=current(),already=selection&&entries.some(entry=>entry.key===readingKey(selection.kind,selection.raw.id));
    add.disabled=!selection||(!already&&entries.length>=2);uiText(add, already?ui("Читать выбранное"):ui("Добавить выбранное"));
    panel.querySelector('.sc-reader-toolbar').hidden=Boolean(already)||entries.length===2;
    resume.hidden=!entries.length;uiText(resume, ui("К чтению · {0}", [entries.length]));
    panel.dataset.count=String(entries.length);uiText(status, storageError||notice||ui("Пара материалов, формы и позиции чтения сохраняются в этом браузере."));status.dataset.important=String(Boolean(storageError));
    layout();restore();scheduleSave();
  }
  const resize=new ResizeObserver(()=>{
    if(panel.hidden)return;
    const next=panel.clientWidth>=580;
    if(next!==wide){wide=next;layout();}
    // Reflow already happened by this callback. Keep the reading anchor saved
    // before the width changed instead of capturing the reflowed scroll offset.
    restore();
  });resize.observe(panel);
  root.addEventListener('sophia-read',event=>pinSelection({...event.detail,sourceRevision:scene.port.packet?.source_revision}));
  root.addEventListener('sophia-workspace-replacing',()=>{writable=false;clearTimeout(saveTimer);shelf.suspend();});
  window.addEventListener('pagehide',event=>{capture();persist();clearTimeout(saveTimer);if(event.persisted)shelf.suspend();else shelf.dispose();});
  document.addEventListener('visibilitychange',()=>{if(document.hidden)persist();});
  window.addEventListener('pageshow',event=>{if(event.persisted)render();});
  activeKey=initial.activeKey;shelf.restore(initial.entries);
  refreshIcons();render();
  return {exportState,flush:persist,
    selectionChanged(){shelf.observeRevision(scene.port.packet?.source_revision);if(!panel.hidden)render();}};
}
