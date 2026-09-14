import {ui,uiAttribute,uiChildren,uiHTML,uiText} from './ui-i18n.mjs';
import {createReadingMemory} from './reading-state.mjs';
import {RequestSlots,localized} from './knowledge-client.mjs';
import {loadEvidence,compareEvidence,sourceRefs,selectionSummary} from './evidence-model.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);uiText(node, text);node.className=className;return node;};
const button=(label,action,className='')=>{const node=el('button',label,className);node.type='button';node.addEventListener('click',action);return node;};
const labels={
  'pre-canon':ui("До канона"),'canon':ui("Канон"),'derived-export':ui("Проекция источников"),
  prepared_research_candidate:ui("Исследовательский кандидат"),prepared_branch_candidate:ui("Кандидат ветви"),
  contested_review_required:ui("Требует рассмотрения"),pending_human_review:ui("Ожидает рассмотрения"),
  'not-recorded':ui("Не указан"),unresolved:ui("Не разрешено"),review_status_unresolved:ui("Статус рассмотрения не установлен"),
  contested_by:ui("Оспаривается"),uncertain_relation:ui("Неопределённая связь"),polemicizes_with:ui("Полемизирует с"),
};
const human=value=>labels[value]||value||ui("Не указан");

export function createEvidencePanel(root,scene,panels,{data:{client,queries},selected,onUserAction}){
  const requests=new RequestSlots();
  const panel=el('section','','sc-panel sc-evidence');panel.hidden=true;uiAttribute(panel, 'aria-label', ui("Основания и прочтения"));
  uiHTML(panel, '<div class="sc-panel-top"><span class="sc-eyebrow">ЛИСТ ИССЛЕДОВАНИЯ</span><button type="button" class="sc-icon sc-evidence-close" aria-label="Закрыть основания"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-evidence-heading"><span class="sc-evidence-symbol" aria-hidden="true">✧</span><div><p class="sc-evidence-kind"></p><h3></h3></div></div><div class="sc-evidence-tabs" role="tablist" aria-label="Основания и сравнение"></div><div class="sc-evidence-body" id="sc-evidence-content" role="tabpanel" tabindex="0"></div><div class="sc-evidence-status" role="status"></div><div class="sc-evidence-footer"><span>От мысли — к источнику</span></div>');
  uiChildren(root, "append", panel);
  const body=panel.querySelector('.sc-evidence-body'),status=panel.querySelector('.sc-evidence-status'),tabs=panel.querySelector('.sc-evidence-tabs');
  const reading=createReadingMemory(body),rememberedTabs=new Map();let comparisonChoice=0;
  let target=null,result=null,failure=null,active='grounds',focusReturn=null,viewKey='',requestId=0;
  panels.register('evidence',panel,()=>{reading.capture();requests.cancelAll();uiAttribute(body, 'aria-busy', 'false');});
  function close(){panels.close('evidence');onUserAction();
    const star=[...root.querySelectorAll('.sc-node')].find(node=>node.dataset.id===target?.raw.id&&!node.hidden);
    (focusReturn?.isConnected&&!focusReturn.closest('[hidden]')?focusReturn:star||root.querySelector('.sc-overview')).focus();
  }
  panel.querySelector('.sc-evidence-close').addEventListener('click',close);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  const tabIds=['grounds','compare'];
  const tabButtons=[ui("Основания"),ui("Сравнение")].map((label,index)=>{
    const tab=button(label,()=>{onUserAction();switchTab(tabIds[index]);});tab.id='sc-evidence-'+tabIds[index];uiAttribute(tab, 'role', 'tab');uiAttribute(tab, 'aria-controls', body.id);uiChildren(tabs, "append", tab);return tab;
  });
  tabs.addEventListener('keydown',event=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();
    const next=event.key==='Home'?0:event.key==='End'?1:1-tabButtons.indexOf(event.target);onUserAction();switchTab(tabIds[next]);tabButtons[next].focus();
  });
  function switchTab(tab){reading.capture();active=tab;rememberedTabs.set(viewKey,tab);if(rememberedTabs.size>48)rememberedTabs.delete(rememberedTabs.keys().next().value);panel.dataset.tab=tab;
    tabButtons.forEach((button,index)=>{uiAttribute(button, 'aria-selected', String(tabIds[index]===tab));button.tabIndex=tabIds[index]===tab?0:-1;});
    uiAttribute(body, 'aria-labelledby', 'sc-evidence-'+tab);render();
  }
  function section(title,content,className=''){
    const node=el('section','','sc-evidence-section '+className);uiChildren(node, "append", el('h4',title));for(const item of content)uiChildren(node, "append", typeof item==='string'?el('p',item):item);return node;
  }
  function refs(references,title=ui("Источники")){
    const details=el('details','','sc-evidence-refs');uiChildren(details, "append", el('summary',title+' · '+references.length));
    for(const ref of references){const row=el('div','','sc-evidence-ref');
      if(/^https?:\/\//i.test(ref)){try{const url=new URL(ref),a=el('a',url.hostname+url.pathname);a.href=url.href;a.target='_blank';a.rel='noopener noreferrer';uiChildren(row, "append", a);}catch{uiChildren(row, "append", el('span',ref));}}
      else uiChildren(row, "append", el('span',ref));
      uiChildren(row, "append", button(ui("Копировать"),async()=>{try{await navigator.clipboard.writeText(ref);uiText(status, ui("Ссылка на источник скопирована."));}catch{uiText(status, ui("Выделите и скопируйте путь к источнику."));}},'sc-evidence-copy'));uiChildren(details, "append", row);
    }
    return details;
  }
  function posture(raw){
    const row=el('dl','','sc-evidence-posture');for(const [label,value]of [[ui("Слой"),raw.epistemic?.authority_layer],[ui("Рассмотрение"),raw.epistemic?.review_posture],[ui("Канон"),raw.epistemic?.canon_status]]){const pair=el('div');uiChildren(pair, "append", el('dt',label), el('dd',human(value)));uiChildren(row, "append", pair);}return row;
  }
  function points(title,values,className=''){
    if(!values?.length)return null;const list=el('ul');for(const value of values)uiChildren(list, "append", el('li',value));return section(title,[list],className);
  }
  function append(...nodes){uiChildren(body, "append", ...nodes.filter(Boolean));}
  function provenance(){
    const raw=result?.raw||target.raw;
    append(posture(raw),refs(sourceRefs(raw),ui("Происхождение объекта")));
    const route=button(ui("Открыть досье источников"),()=>{onUserAction();root.dispatchEvent(new CustomEvent('sophia-sources',{detail:{raw,kind:target.kind}}));},'sc-evidence-source');uiChildren(body, "append", route);
  }
  function renderGrounds(){
    const raw=result.raw,packet=result.packet;
    if(!packet){
      append(section(ui("Происхождение"), [localized(target.kind==='relation'?raw.display.explanation:raw.display.summary,ui("Описание пока не записано."))],'sc-evidence-finding'));
      append(section(ui("Маршрут оснований"), [result.availability==='outside_route'?ui("Объект не найден в доступной области Evidence Lens. Это не означает, что у него нет оснований."):ui("Для этого слоя отдельный маршрут оснований ещё не подключён. Здесь показаны сведения из карточки и её источники.")]));provenance();return;
    }
    append(section(ui("Что установлено"), [packet.finding_ru||packet.finding],'sc-evidence-finding'));
    const limits=el('div','','sc-evidence-conclusions');
    for(const node of [points(ui("Можно утверждать"),packet.conclusion.allowed_ru||packet.conclusion.allowed,'sc-evidence-allowed'),points(ui("Вывод пока не следует"),packet.conclusion.not_allowed_ru||packet.conclusion.not_allowed,'sc-evidence-limits')])if(node)uiChildren(limits, "append", node);
    append(limits);
    if(packet.conclusion.can_conclude!==true)append(el('p',ui("Материала недостаточно для окончательного вывода в указанной области."),'sc-evidence-note'));
    append(points(ui("Открытые вопросы"),packet.gaps_ru||packet.gaps));
    if(packet.source_anchors.length){const anchors=el('details','','sc-evidence-refs');uiChildren(anchors, "append", el('summary',ui("Точные фрагменты · {0}", [packet.source_anchors.length])));for(const anchor of packet.source_anchors){const item=el('div','','sc-evidence-ref');uiChildren(item, "append", el('p',(anchor.anchor_segment_ids||[]).join(' · ')), el('small',anchor.witness_scope||''));if(anchor.relation_ref)uiChildren(item, "append", refs([anchor.relation_ref],ui("Запись связи")));uiChildren(anchors, "append", item);}append(anchors);}
    if(packet.routes.length){const routes=el('details','','sc-evidence-refs');uiChildren(routes, "append", el('summary',ui("Маршруты к основаниям · {0}", [packet.routes.length])));for(const route of packet.routes){const item=el('div','','sc-evidence-ref');uiChildren(item, "append", el('small',human(route.route_kind)+' · '+human(route.status)));if(route.ref)uiChildren(item, "append", refs([route.ref],ui("Открыть путь")));uiChildren(routes, "append", item);}append(routes);}
    append(refs(packet.source_refs,ui("Источники поля оснований")));provenance();
  }
  function readingCard(reading,caption,isSelected=false){
    const card=el('article','','sc-reading'+(isSelected?' sc-reading-selected':''));
    uiChildren(card, "append", el('span',caption,'sc-reading-caption'), el('h4',human(reading.label)));
    if(reading.route)uiChildren(card, "append", el('p',reading.route,'sc-reading-route'));
    uiChildren(card, "append", el('p',reading.statement||ui("Описание этого прочтения пока не записано."),'sc-reading-text'));
    if(reading.review_posture||reading.canon_status)uiChildren(card, "append", el('p',[reading.review_posture,reading.canon_status].filter(Boolean).map(human).join(' · '),'sc-evidence-note'));
    uiChildren(card, "append", refs(reading.source_refs));return card;
  }
  function renderComparison(){
    if(!result.packet){append(section(ui("Сопоставление ещё не подключено"),[result.availability==='outside_route'?ui("Объект находится за пределами доступного маршрута оснований."):ui("В этом слое пока нет подключённого поля прочтений. Соседство на карте само по себе не означает разногласия.")]));provenance();return;}
    const comparison=compareEvidence(result,selectionSummary(result.raw,target.kind)),packet=result.packet,raw=result.raw;
    append(el('p',ui("Сопоставление показывает записанные связи и вопросы к ним. Их истинность определяется рассмотрением источников."),'sc-evidence-note'));
    const readings=comparison.competing_readings;
    const chosen={label:localized(raw.display.title||raw.display.label),statement:localized(target.kind==='relation'?raw.display.explanation:raw.display.summary),route:localized(raw.display.statement),source_refs:sourceRefs(raw),review_posture:raw.epistemic?.review_posture,canon_status:raw.epistemic?.canon_status};
    const spread=el('div','','sc-reading-spread');uiChildren(spread, "append", readingCard(chosen,ui("ВЫБРАНО"),true));
    if(readings.length){
      const right=el('div','','sc-reading-alternative');
      const label=el('label',ui("Сопоставить с"));label.htmlFor='sc-reading-choice';const select=el('select');select.id=label.htmlFor;
      for(const [index,reading]of readings.entries()){const option=el('option',`${index+1}. ${human(reading.label)}${reading.route?' · '+reading.route:''}`);option.value=String(index);uiChildren(select, "append", option);}
      select.value=String(Math.min(comparisonChoice,readings.length-1));
      const card=el('div');const update=()=>{uiChildren(card, "replaceChildren", readingCard(readings[Number(select.value)],ui("ДРУГОЕ ПРОЧТЕНИЕ")));scene.invalidate();};
      select.addEventListener('change',()=>{onUserAction();comparisonChoice=Number(select.value);update();});uiChildren(right, "append", label, select, card);update();uiChildren(spread, "append", right);
    }else uiChildren(spread, "append", section(ui("Других прочтений не показано"),[ui("В полученной области нет других оспаривающих связей. Это не означает согласия или доказанности.")],'sc-reading-empty'));
    append(spread);
    const coverage=packet.coverage;
    append(el('p',ui("Получено оспаривающих связей: {0} из {1}.", [coverage.returned_challenge_relations??packet.challenge_relations.length, coverage.available_challenge_relations??packet.challenge_relations.length])+(target.kind==='relation'?ui(" Выбранная связь исключена из второго столбца."):''),'sc-evidence-note'));
    if(comparison.contextual_readings.length){const context=el('details','','sc-evidence-refs');uiChildren(context, "append", el('summary',ui("Контекст · {0}", [comparison.contextual_readings.length])));for(const reading of comparison.contextual_readings)uiChildren(context, "append", readingCard(reading,ui("СВЯЗЬ КОНТЕКСТА")));append(context);}
    append(points(ui("Что остаётся открытым"),comparison.gaps));
  }
  function render(){
    reading.capture();reading.enter(viewKey+'|'+active);uiChildren(body, "replaceChildren");uiText(status, '');uiAttribute(body, 'aria-busy', String(!result&&!failure));
    if(failure){append(section(ui("Не удалось прочитать основания"),[failure.message]),button(ui("Повторить"),()=>{onUserAction();void open(target,active).catch(()=>{});},'sc-evidence-source'));}
    else if(!result){append(el('div',ui("Собираю источники и прочтения…"),'sc-evidence-loading'));}
    else if(active==='grounds')renderGrounds();else renderComparison();
    reading.restore();scene.invalidate();
  }
  async function open(source,tab=null,{signal,limit=60}={}){
    if(!source?.raw)throw new Error(ui("Сначала выберите звезду или отношение."));
    const ticket=++requestId;
    reading.capture();const revision=scene.port.packet?.source_revision,nextKey=JSON.stringify([source.raw.id,source.kind,revision]);
    if(nextKey!==viewKey)comparisonChoice=0;
    focusReturn=document.activeElement;panels.open('evidence');target=source;result=null;failure=null;viewKey=nextKey;tab=tab||rememberedTabs.get(viewKey)||'grounds';
    panel.dataset.itemId=source.raw.id;uiText(panel.querySelector('h3'), human(localized(source.raw.display.title||source.raw.display.label)));
    uiText(panel.querySelector('.sc-evidence-kind'), source.kind==='relation'?ui("ОТНОШЕНИЕ"):localized(source.raw.display.kind_label,ui("УЗЕЛ")).toUpperCase());
    switchTab(tab);tabButtons[tabIds.indexOf(tab)].focus();
    try{
      const response=await requests.run('evidence',ownSignal=>loadEvidence(source.raw,source.kind,revision,{client,queries,limit,signal:signal?AbortSignal.any([signal,ownSignal]):ownSignal}));
      signal?.throwIfAborted();if(!response.current||panel.hidden)throw new DOMException('Panel closed','AbortError');
      result=response.value;render();return result;
    }catch(error){
      if(ticket===requestId&&!panel.hidden){failure=error.name==='AbortError'?new Error(ui("Чтение прервано. Можно повторить запрос.")):error;render();}
      throw error;
    }
  }
  root.addEventListener('sophia-evidence',event=>{onUserAction();void open(event.detail).catch(()=>{});});
  function selectionChanged(){
    const selection=selected();
    if(!panel.hidden&&viewKey!==JSON.stringify([selection?.id,selection?.kind==='edge'?'relation':'node',scene.port.packet?.source_revision]))panels.close('evidence');
  }
  async function command(tab,input,{signal}){
    const selection=selected(),kind=selection?.kind==='edge'?'relation':'node';
    const raw=kind==='relation'?scene.port.relation(selection?.id):scene.port.node(selection?.id);
    const loaded=await open({raw,kind},tab,{signal,limit:Number(input.limit)||60});
    if(!loaded.packet)throw new Error(ui("Для этого объекта маршрут Evidence Lens сейчас недоступен."));
    return tab==='compare'?compareEvidence(loaded,selection):{...loaded.packet,binding:loaded.binding,agent_summary:{...loaded.packet.agent_summary,selection:raw.id}};
  }
  panels.configure('evidence',{onResume:()=>{if(!result)void open(target,active).catch(()=>{});else reading.restore();}});
  refreshIcons();window.addEventListener('pagehide',()=>requests.cancelAll());
  return {selectionChanged,handlers:{'tos.page.inspect-epistemic':(input,execution)=>command('grounds',input,execution),'tos.page.compare-readings':(input,execution)=>command('compare',input,execution)}};
}
