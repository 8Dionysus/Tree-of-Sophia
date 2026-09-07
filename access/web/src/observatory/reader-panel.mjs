import {createReadingMemory} from './reading-state.mjs';
import {createReadingShelf,readingDocument,readingLanguages,readingKey,formLabel} from './reader-model.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(label,action,className='')=>{const node=el('button',label,className);node.type='button';node.addEventListener('click',action);return node;};
const originLabels={authored:'Авторское описание','source-derived':'Описание из источника','metadata-synthesis':'Описание составлено из метаданных'};
const postureLabels={disputed:'Оспаривается',rejected:'Отклонено',accepted:'Принято в указанной источником области',
  contested_review_required:'Требует рассмотрения',unresolved:'Не разрешено',review_status_unresolved:'Статус рассмотрения не установлен',
  pending_human_review:'Ожидает рассмотрения',unreviewed:'Не рассмотрено','not-recorded':'Не указан'};

export function createReaderPanel(root,scene,panels,{data:{client},onUserAction=()=>{}}){
  const views=new Map();let activeKey=null,returnFocus=null,notice='',wide=false;
  const opener=button('',()=>show(),'sc-control sc-reader-open');
  opener.setAttribute('aria-label','Чтение и сопоставление');opener.setAttribute('aria-expanded','false');
  opener.innerHTML='<i data-lucide="book-open" aria-hidden="true"></i><span>Чтение</span>';
  root.querySelector('.sc-header-actions').append(opener);
  const resume=button('',()=>show(),'sc-reader-resume');resume.hidden=true;
  root.querySelector('.sc-context').append(resume);
  const panel=el('section','','sc-panel sc-reader');panel.hidden=true;panel.setAttribute('aria-label','Чтение и сопоставление');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ЧТЕНИЕ</span><button type="button" class="sc-icon sc-reader-close" aria-label="Закрыть чтение"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-reader-heading"><h3 tabindex="-1">Удержать мысль</h3></div><div class="sc-reader-toolbar"></div><div class="sc-reader-tabs" role="tablist" aria-label="Закреплённые материалы"></div><div class="sc-reader-columns"></div><p class="sc-reader-notice" role="status"></p>';
  root.append(panel);
  const columns=panel.querySelector('.sc-reader-columns'),tabs=panel.querySelector('.sc-reader-tabs'),status=panel.querySelector('.sc-reader-notice');
  const add=button('Добавить выбранное',()=>pinSelection(),'sc-reader-add');panel.querySelector('.sc-reader-toolbar').append(add);
  const empty=el('p','Оставьте здесь предмет или связь, чтобы читать, переходить к основаниям и сопоставлять с другим материалом.','sc-reader-empty');
  const shelf=createReadingShelf({client,onChange:render});
  const current=()=>{
    const selection=scene.port.selection,kind=selection.relationId?'relation':'node';
    const raw=kind==='relation'?scene.port.relation(selection.relationId):scene.port.node(selection.nodeId);
    return raw?{raw,kind,sourceRevision:scene.port.packet?.source_revision}:null;
  };
  const entryFor=key=>shelf.entries.find(entry=>entry.key===key);
  function capture(){for(const view of views.values())view.reading.capture();}
  function restore(){for(const view of views.values())if(!view.article.hidden)view.reading.restore();}
  panels.register('reader',panel,()=>{capture();opener.setAttribute('aria-expanded','false');});
  panels.configure('reader',{onResume:()=>{opener.setAttribute('aria-expanded','true');render();}});
  panels.addTool('reader',{title:'Чтение и сопоставление',opener,launch:()=>show()});
  function show(){
    onUserAction();returnFocus=document.activeElement;panels.open('reader');opener.setAttribute('aria-expanded','true');render();
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
    if(!source){notice='Сначала выберите звезду или связь.';show();return;}
    onUserAction();scene.ui.captureReading();
    try{
      const result=shelf.pin({...source,bookmark:scene.port.captureView()});activeKey=result.key;
      notice=result.existing?'Этот материал уже оставлен для чтения.':'Материал оставлен для чтения в этой вкладке.';
    }catch(error){notice=error.message;}
    show();
  }
  function remove(key){
    onUserAction();capture();shelf.remove(key);notice='Материал убран из чтения.';render();
    (views.get(activeKey)?.tab||add).focus();
  }
  function switchTo(key){onUserAction();capture();activeKey=key;layout();restore();}
  function returnTo(key){
    const entry=entryFor(key);if(!entry?.bookmark)return;
    if(entry.bookmark.graph.packet.source_revision!==scene.port.packet?.source_revision){notice='Данные изменились. Откройте актуальный материал в пространстве.';render();return;}
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
    const removeButton=button('Убрать',()=>remove(key),'sc-reader-remove');
    head.append(kind,title,removeButton);
    const controls=el('div','','sc-reader-item-controls'),language=el('select');language.setAttribute('aria-label','Язык или форма материала');
    const languageLabel=el('label','Форма');languageLabel.append(language);
    const refresh=button('Обновить',()=>{onUserAction();void shelf.refresh(key);});controls.append(languageLabel,refresh);
    const state=el('p','','sc-reader-state');state.setAttribute('role','status');
    const body=el('div','','sc-reader-body');body.tabIndex=0;
    const actions=el('div','','sc-reader-actions');
    const back=button('К месту',()=>returnTo(key)),open=button('В пространстве',()=>openInScene(key));
    back.setAttribute('aria-label','Вернуться к месту закрепления');open.setAttribute('aria-label','Открыть материал в пространстве');
    const evidence=button('Основания',()=>handoff(key,'sophia-evidence')),sources=button('Источники',()=>handoff(key,'sophia-sources'));
    actions.append(back,open,evidence,sources);article.append(head,controls,state,body,actions);
    const tab=button('',()=>switchTo(key));tab.setAttribute('role','tab');tab.id='sc-reader-tab-'+crypto.randomUUID();
    article.id='sc-reader-item-'+crypto.randomUUID();tab.setAttribute('aria-controls',article.id);
    tab.addEventListener('keydown',event=>{
      if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
      event.preventDefault();const entries=shelf.entries,index=entries.findIndex(item=>item.key===key);
      const next=entries[event.key==='Home'?0:event.key==='End'?entries.length-1:(index+1)%entries.length];
      switchTo(next.key);views.get(next.key).tab.focus();
    });
    const view={article,head,title,kind,language,refresh,state,body,back,open,evidence,sources,tab,reading:createReadingMemory(body,{limit:8}),preferred:'ru',snapshot:null};
    language.addEventListener('change',()=>{onUserAction();view.reading.capture();view.preferred=language.value;view.snapshot=null;render();});
    columns.append(article);tabs.append(tab);return view;
  }
  function appendForm(parent,form,blockId){
    const text=el('div','','sc-reader-text');if(form.lang)text.lang=form.lang;
    // Paragraph anchors are page-local reading positions, never invented ToS
    // text addresses. Keep all delivered characters, including separators.
    form.text.split(/(\r?\n[\t ]*\r?\n)/).forEach((part,index)=>{
      if(index%2){text.append(document.createTextNode(part));return;}
      const p=el('p',part);p.dir='auto';p.dataset.readingAnchor=blockId+':'+index;text.append(p);
    });parent.append(text);
    if(form.fallback||!form.lang){
      const caption=(form.fallback?'Выбранная форма отсутствует. Показана: ':'Показана: ')+formLabel(form.key)+(form.lang?'.':'. Язык в этой форме не указан.');
      parent.append(el('p',caption,'sc-reader-language-note'));
    }
  }
  function renderDocument(view,entry){
    const snapshot=entry.snapshot,doc=readingDocument(snapshot,view.preferred);
    view.reading.capture();view.reading.enter(JSON.stringify([entry.key,snapshot.sourceRevision,snapshot.raw.content_revision,view.preferred]));
    view.body.replaceChildren();view.body.scrollTop=0;view.title.textContent=doc.title?.text||'Материал';
    view.title.dir='auto';if(doc.title?.lang)view.title.lang=doc.title.lang;else view.title.removeAttribute('lang');
    view.kind.textContent=entry.kind==='relation'?'Связь':doc.kind?.text||'Предмет';
    if(doc.title?.fallback||!doc.title?.lang)view.body.append(el('p','Название: '+formLabel(doc.title?.key||'не указана')+(doc.title?.lang?'':'; язык не указан'),'sc-reader-language-note'));
    for(const block of doc.blocks){
      const section=el('section','','sc-reader-section');section.append(el('h5',block.title));
      if(block.form)appendForm(section,block.form,block.id);
      else section.append(el('p',block.id==='statement'?'Формулировка пока не предоставлена.':'Описание пока отсутствует.','sc-reader-gap'));
      if(originLabels[block.state])section.append(el('p',originLabels[block.state],'sc-reader-origin'));
      view.body.append(section);
    }
    if(doc.participants.length){
      const section=el('section','','sc-reader-section');section.append(el('h5','Участники связи'));
      for(const participant of doc.participants){
        const row=button('',()=>{onUserAction();capture();scene.ui.chooseNode(participant.id,snapshot.sourceRevision);},'sc-reader-participant');
        row.append(el('small',participant.role),el('span',participant.form?.text||'Участник'));section.append(row);
      }
      view.body.append(section);
    }
    const review=doc.posture.review_posture;
    if(postureLabels[review]&&review!=='not-recorded')view.body.append(el('p','Рассмотрение: '+postureLabels[review]+'.','sc-reader-assessment'));
    const refs=el('details','','sc-reader-section');refs.dataset.readingKey='sources';refs.append(el('summary','Источники · '+doc.sourceRefs.length));
    for(const ref of doc.sourceRefs){
      let link=el('span',ref,'sc-source-ref');
      try{const url=new URL(ref);if(['http:','https:'].includes(url.protocol)){link=el('a',ref,'sc-source-ref');link.href=url.href;link.target='_blank';link.rel='noopener noreferrer';}}catch{/* Local source references stay selectable text. */}
      refs.append(link);
    }
    view.body.append(refs);
    const technical=el('details','','sc-reader-technical');technical.dataset.readingKey='identity';technical.append(el('summary','Точные сведения о материале'));
    const details=el('dl');
    for(const [label,value]of [['Идентификатор',entry.id],['Снимок данных',snapshot.sourceRevision],['Версия материала',snapshot.raw.content_revision],['Слой',doc.posture.authority_layer],['Рассмотрение',review],['Канон',doc.posture.canon_status],['Уверенность, как передана источником',doc.posture.confidence]]){
      const row=el('div');row.append(el('dt',label),el('dd',value===null||value===undefined||value==='not-recorded'?'Не указан':String(value)));details.append(row);
    }
    technical.append(details);view.body.append(technical);view.snapshot=snapshot;
  }
  function layout(){
    const entries=shelf.entries,twoColumns=wide&&entries.length===2;
    panel.dataset.columns=twoColumns?'2':'1';tabs.hidden=entries.length<2||twoColumns;
    for(const entry of entries){const view=views.get(entry.key),selected=entry.key===activeKey;
      view.article.hidden=!twoColumns&&!selected;view.tab.setAttribute('aria-selected',String(selected));view.tab.tabIndex=selected?0:-1;
      if(!tabs.hidden){view.article.setAttribute('role','tabpanel');view.article.setAttribute('aria-labelledby',view.tab.id);}
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
        const languages=readingLanguages(entry.snapshot);view.language.replaceChildren(...languages.map(key=>{const option=el('option',formLabel(key));option.value=key;return option;}));
        if(!languages.includes(view.preferred)){const option=el('option',formLabel(view.preferred)+' — недоступна');option.value=view.preferred;view.language.prepend(option);}
        view.language.value=view.preferred;renderDocument(view,entry);
      }else if(!entry.snapshot){
        view.snapshot=null;view.title.textContent=entry.title?.text||'Материал';view.body.replaceChildren(el('p',entry.loading?'Получаю материал…':'Материал пока недоступен.','sc-reader-gap'));
      }
      view.tab.textContent=view.title.textContent;view.body.setAttribute('aria-label','Чтение: '+view.title.textContent);
      view.language.disabled=!entry.snapshot;view.refresh.disabled=entry.loading;
      const mismatch=Boolean(entry.snapshot&&shelf.sceneRevision&&entry.sourceRevision!==shelf.sceneRevision);
      const states=[entry.loading?'Обновляю материал…':null,entry.error,
        mismatch?'Материал и сцена относятся к разным снимкам.':null,
        entry.snapshot&&entry.error?'Показан ранее закреплённый материал.':null];
      view.state.textContent=states.filter(Boolean).join(' ');view.state.hidden=!view.state.textContent;
      view.back.hidden=!entry.bookmark;view.back.disabled=entry.bookmark?.graph?.packet?.source_revision!==scene.port.packet?.source_revision;
      view.open.disabled=!entry.snapshot||mismatch;view.evidence.disabled=!entry.snapshot||mismatch;view.sources.disabled=!entry.snapshot||mismatch;
      view.article.dataset.snapshot=entry.snapshot?.sourceRevision||'';
    }
    empty.hidden=Boolean(entries.length);if(!empty.isConnected)columns.append(empty);
    const selection=current(),already=selection&&entries.some(entry=>entry.key===readingKey(selection.kind,selection.raw.id));
    add.disabled=!selection||(!already&&entries.length>=2);add.textContent=already?'Читать выбранное':'Добавить выбранное';
    panel.querySelector('.sc-reader-toolbar').hidden=Boolean(already)||entries.length===2;
    resume.hidden=!entries.length;resume.textContent='К чтению · '+entries.length;
    panel.dataset.count=String(entries.length);status.textContent=notice||'Материалы сохраняются для чтения до закрытия вкладки.';
    layout();restore();
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
  window.addEventListener('pagehide',event=>{if(event.persisted)shelf.suspend();else shelf.dispose();});
  window.addEventListener('pageshow',event=>{if(event.persisted)render();});
  refreshIcons();render();
  return {selectionChanged(){shelf.observeRevision(scene.port.packet?.source_revision);if(!panel.hidden)render();}};
}
